//! CLI 子程序唯一的 spawn 入口（plans/agy-home-cleanup.md、plans/cli-version-gate.md 包 2）。
//! - 四家 CLI 一律注入關閉自動更新的 env（版本由 app 掌控，方案拍板 B）。
//! - 四家 CLI 先過更新擋板（`update::gate`）：擋板立著立即回 `CliUpdating`；否則在途表寫一筆佔位，
//!   spawn 成功回填 pid，收屍後才移除。
//! - envs 帶 `GROK_HOME` 的呼叫另進 grok 閘拿票。
//! 票與佔位跟著 `Spawned` 走、程序真的 wait 收屍後才交回：正常路徑在 `wait`／`wait_with_output`／
//! `kill_and_wait` 回來時交回；`Spawned` 沒收屍就被丟棄（future 被取消、逾時、提早返回）時，
//! Drop 把 Child 交給 tokio 任務 kill＋wait，收屍完才交回。`kill_on_drop` 送出終止不算退出。
//! 更新模式（`spawn_update`）不過擋板、不注入關閉變數，放進獨立程序群組。
//! cli/ 底下的程序一律走這裡（本檔測試掃原始碼把關）。

use super::grok_gate::{self, Ticket};
use super::update::gate::{self, Slot};
use super::update::tree::{self, TreeChild};
use crate::ui_msg::UiMsg;
use std::io;
use std::path::Path;
use std::process::{ExitStatus, Output};
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};

pub(crate) struct Spawned {
    child: Option<Child>,
    ticket: Option<Ticket>,
    slot: Option<Slot>,
}

/// 四家關閉自動更新的 env（方案「前提」表）。codex `exec` 本來就不觸發，不帶。
/// claude 不用 `DISABLE_UPDATES`（會連更新鈕的 `claude update` 一起擋）；agy 只認 "true"。
pub(crate) fn auto_update_off(cli: &str) -> &'static [(&'static str, &'static str)] {
    match cli {
        "claude" => &[("DISABLE_AUTOUPDATER", "1")],
        "agy" => &[("AGY_CLI_DISABLE_AUTO_UPDATE", "true")],
        "grok" => &[("GROK_DISABLE_AUTOUPDATER", "1")],
        _ => &[],
    }
}

/// spawn 失敗時票與佔位立刻交回（在這裡 drop）。
pub(crate) fn spawn(command: &mut Command, envs: &[(String, String)]) -> io::Result<Spawned> {
    let cli = gate::cli_of(command.as_std().get_program());
    let slot = match cli {
        Some(cli) => {
            command.envs(auto_update_off(cli).iter().copied());
            Some(gate::enter(cli).map_err(|()| {
                io::Error::other(
                    UiMsg::CliUpdating {
                        cli: cli.to_owned(),
                    }
                    .to_string(),
                )
            })?)
        }
        None => None,
    };
    let ticket = envs
        .iter()
        .rev()
        .find(|(key, _)| key == "GROK_HOME")
        .map(|(_, home)| grok_gate::enter(Path::new(home)));
    command.kill_on_drop(true);
    let child = command.spawn()?;
    if let Some(slot) = &slot {
        slot.set_pid(child.id());
    }
    Ok(Spawned {
        child: Some(child),
        ticket,
        slot,
    })
}

/// 更新模式：CLI 的更新／還原指令與安裝腳本。不過擋板（擋板就是更新流程自己立的），
/// 不注入關閉自動更新的變數，放進獨立程序群組（逾時殺整組）。環境由呼叫端組好。
pub(crate) fn spawn_update(command: &mut Command) -> io::Result<TreeChild> {
    tree::prepare(command);
    command.kill_on_drop(true);
    tree::adopt(command.spawn()?)
}

impl Spawned {
    pub(crate) fn child(&mut self) -> &mut Child {
        self.child.as_mut().expect("child 在 Spawned 存活期間都在")
    }

    pub(crate) fn id(&self) -> Option<u32> {
        self.child.as_ref().and_then(Child::id)
    }

    /// 收屍完成：交回票與在途佔位。
    fn reaped(&mut self) {
        self.ticket.take();
        self.slot.take();
    }

    pub(crate) async fn wait(&mut self) -> io::Result<ExitStatus> {
        let status = self.child().wait().await?;
        self.reaped();
        Ok(status)
    }

    /// 先送殺再等它退出；已經死掉時 `start_kill` 會失敗，`wait` 仍負責收屍。
    pub(crate) async fn kill_and_wait(&mut self) {
        let _ = self.child().start_kill();
        if self.child().wait().await.is_ok() {
            self.reaped();
        }
    }

    /// 等同 tokio 的 `wait_with_output`，但 Child 留在 `Spawned` 裡：讀到一半被丟棄時，
    /// Drop 照樣能收屍再交回票。stdout／stderr 沒設成 piped 的那邊回空。
    pub(crate) async fn wait_with_output(&mut self) -> io::Result<Output> {
        let child = self.child();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let (status, stdout, stderr) =
            tokio::try_join!(child.wait(), read_all(stdout), read_all(stderr))?;
        self.reaped();
        Ok(Output {
            status,
            stdout,
            stderr,
        })
    }
}

async fn read_all(pipe: Option<impl AsyncReadExt + Unpin>) -> io::Result<Vec<u8>> {
    let mut buffer = Vec::new();
    if let Some(mut pipe) = pipe {
        pipe.read_to_end(&mut buffer).await?;
    }
    Ok(buffer)
}

impl Drop for Spawned {
    fn drop(&mut self) {
        if self.ticket.is_none() && self.slot.is_none() {
            return; // 不是四家 CLI 或已收屍：kill_on_drop 處理剩下的
        }
        let Some(mut child) = self.child.take() else {
            return;
        };
        let _ = child.start_kill();
        let ticket = self.ticket.take();
        let slot = self.slot.take();
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            // 沒有 runtime 就沒辦法等它退出：票不交回；佔位隨這裡返回一併移除（程序已送殺）
            if let Some(ticket) = ticket {
                ticket.leak();
            }
            return;
        };
        // 收屍任務被 runtime 中止時，Unreaped 的 Drop 也不交回票
        let ticket = Unreaped(ticket);
        handle.spawn(async move {
            let mut ticket = ticket;
            let waited = child.wait().await;
            drop(slot);
            if let Some(ticket) = ticket.0.take() {
                settle(ticket, waited);
            }
        });
    }
}

/// 收屍結果決定票的去留：確認退出才交回，`wait` 回錯就不交回。
fn settle(ticket: Ticket, waited: io::Result<ExitStatus>) {
    match waited {
        Ok(_) => drop(ticket),
        Err(error) => {
            log::warn!("grok 程序收屍失敗：{error}");
            ticket.leak();
        }
    }
}

/// 收屍任務手上的票：任務沒跑完就被丟棄（runtime 收尾中止）時不交回。
struct Unreaped(Option<Ticket>);

impl Drop for Unreaped {
    fn drop(&mut self) {
        if let Some(ticket) = self.0.take() {
            ticket.leak();
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Stdio;
    use std::time::Duration;

    fn grok_home(tag: &str) -> PathBuf {
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce).unwrap();
        let dir = std::env::temp_dir().join(format!(
            "tt-spawn-{tag}-{}",
            nonce.iter().map(|b| format!("{b:02x}")).collect::<String>()
        ));
        std::fs::create_dir_all(dir.join("sessions")).unwrap();
        dir
    }

    fn envs(home: &Path) -> Vec<(String, String)> {
        vec![("GROK_HOME".to_owned(), home.to_string_lossy().into_owned())]
    }

    async fn until_zero(home: &Path) {
        for _ in 0..100 {
            if grok_gate::running(home) == 0 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("票沒交回");
    }

    /// 四家一律帶關閉自動更新的 env（codex exec 本來不觸發、不帶）；安裝腳本之類的程式不帶。
    #[tokio::test]
    async fn cli_spawns_carry_auto_update_switches() {
        let env = |command: &Command, key: &str| {
            command
                .as_std()
                .get_envs()
                .find(|(name, _)| *name == key)
                .and_then(|(_, value)| value)
                .map(|value| value.to_string_lossy().into_owned())
        };
        for (cli, key, value) in [
            ("claude", "DISABLE_AUTOUPDATER", "1"),
            ("agy", "AGY_CLI_DISABLE_AUTO_UPDATE", "true"),
            ("grok", "GROK_DISABLE_AUTOUPDATER", "1"),
        ] {
            let mut command = Command::new(format!("/nonexistent-tt/{cli}"));
            assert!(spawn(&mut command, &[]).is_err());
            assert_eq!(env(&command, key).as_deref(), Some(value), "{cli}");
        }
        let mut codex = Command::new("/nonexistent-tt/codex");
        let _ = spawn(&mut codex, &[]);
        assert_eq!(codex.as_std().get_envs().count(), 0);
        let mut bash = Command::new("/nonexistent-tt/bash");
        let _ = spawn(&mut bash, &[]);
        assert_eq!(env(&bash, "DISABLE_AUTOUPDATER"), None);
        // spawn 失敗：在途佔位不留
        assert_eq!(super::gate::inflight_count("claude"), 0);
    }

    #[tokio::test]
    async fn spawn_failure_returns_ticket_at_once() {
        let home = grok_home("fail");
        let mut command = Command::new(home.join("no-such-program"));
        assert!(spawn(&mut command, &envs(&home)).is_err());
        assert_eq!(grok_gate::running(&home), 0);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[tokio::test]
    async fn non_grok_spawn_takes_no_ticket() {
        let home = grok_home("plain");
        let mut command = Command::new("true");
        let mut spawned = spawn(&mut command, &[]).unwrap();
        assert_eq!(grok_gate::running(&home), 0);
        spawned.wait().await.unwrap();
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[tokio::test]
    async fn ticket_held_until_wait_returns() {
        let home = grok_home("wait");
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 0.2"]);
        let mut spawned = spawn(&mut command, &envs(&home)).unwrap();
        assert_eq!(grok_gate::running(&home), 1);
        spawned.wait().await.unwrap();
        assert_eq!(grok_gate::running(&home), 0);
        std::fs::remove_dir_all(&home).unwrap();
    }

    /// 收屍回錯：票不交回，在途數不歸零、索引不刪。
    #[test]
    fn failed_wait_keeps_ticket_out() {
        let home = grok_home("wait-error");
        std::fs::write(home.join("sessions/session_search.sqlite"), "x").unwrap();
        settle(
            grok_gate::enter(&home),
            Err(io::Error::other("wait failed")),
        );
        assert_eq!(grok_gate::running(&home), 1);
        assert!(home.join("sessions/session_search.sqlite").exists());
        std::fs::remove_dir_all(&home).unwrap();
    }

    /// 讀輸出讀到一半 future 被丟棄（逾時）：收屍完成前票不交回、收屍後才交回。
    #[tokio::test]
    async fn dropped_mid_output_releases_only_after_reap() {
        let home = grok_home("drop");
        let mut command = Command::new("sh");
        // 先吐一行、忽略 TERM 撐一下再走，模擬收屍要花時間
        command
            .args(["-c", "trap '' TERM; echo partial; sleep 0.4"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut spawned = spawn(&mut command, &envs(&home)).unwrap();
        let timed_out =
            tokio::time::timeout(Duration::from_millis(50), spawned.wait_with_output()).await;
        assert!(timed_out.is_err());
        drop(spawned);
        // SIGKILL 由 start_kill 送出：trap 擋不了，收屍很快完成；在那之前在途數仍是 1
        assert_eq!(grok_gate::running(&home), 1);
        until_zero(&home).await;
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[tokio::test]
    async fn wait_with_output_collects_both_pipes() {
        let home = grok_home("output");
        let mut command = Command::new("sh");
        command
            .args(["-c", "echo out; echo err >&2"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut spawned = spawn(&mut command, &envs(&home)).unwrap();
        let output = spawned.wait_with_output().await.unwrap();
        assert_eq!(output.stdout, b"out\n");
        assert_eq!(output.stderr, b"err\n");
        assert_eq!(grok_gate::running(&home), 0);
        std::fs::remove_dir_all(&home).unwrap();
    }
}

/// 掃原始碼的結構檢查不限 unix：Windows CI 也要擋。
#[cfg(test)]
mod scan_tests {
    use std::path::Path;

    /// 子程序一律走本檔：cli/ 底下不准直接 spawn／output／status；cli/ 以外不准建 `Command`，
    /// 只放行不帶 GROK_HOME 的固定例外（行程管理的 kill、更新的 codesign、測試通道、登入開終端機）。
    #[test]
    fn spawns_go_through_helper() {
        // 整檔放行的例外；cli_setup 只放行開終端機的 `open` 與它的 import
        const OUTSIDE_EXCEPTIONS: [&str; 3] = ["inflight.rs", "updater/", "harness/"];
        const CLI_SETUP_ALLOWED: [&str; 2] =
            ["Command::new(\"open\")", "use std::process::Command;"];
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders = Vec::new();
        let mut stack = vec![src.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                // 以 components 組成「/」分隔的相對路徑，Windows 上同一套比對也成立
                let relative = path
                    .strip_prefix(&src)
                    .unwrap()
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("");
                // 測試檔與檔尾的測試模組不算：測試用 std 的同步程序搭假 CLI
                let test_file = name == "tests.rs"
                    || name.ends_with("_tests.rs")
                    || path.components().any(|part| part.as_os_str() == "tests");
                if !name.ends_with(".rs") || test_file || relative == "cli/spawn.rs" {
                    continue;
                }
                let in_cli = relative.starts_with("cli/");
                if !in_cli
                    && OUTSIDE_EXCEPTIONS
                        .iter()
                        .any(|exception| relative.starts_with(exception))
                {
                    continue;
                }
                let patterns: &[&str] = if in_cli {
                    &[
                        ".spawn()",
                        ".output()",
                        ".status()",
                        "std::process::Command",
                    ]
                } else {
                    &["Command::new(", "process::Command"]
                };
                let text = std::fs::read_to_string(&path)
                    .unwrap()
                    .replace("\r\n", "\n");
                let production = text
                    .split("\n#[cfg(test)]\nmod ")
                    .next()
                    .unwrap_or_default()
                    .split("\n#[cfg(all(test")
                    .next()
                    .unwrap_or_default();
                for (number, line) in production.lines().enumerate() {
                    let code = line.split("//").next().unwrap_or_default();
                    let allowed = relative == "commands/cli_setup.rs"
                        && CLI_SETUP_ALLOWED.iter().any(|ok| code.contains(ok));
                    if !allowed && patterns.iter().any(|pattern| code.contains(pattern)) {
                        offenders.push(format!("{relative}:{}", number + 1));
                    }
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "未走 cli::spawn 的呼叫：{offenders:?}"
        );
    }
}
