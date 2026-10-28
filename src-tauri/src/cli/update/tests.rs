//! 更新流程以假 CLI 腳本驗（plans/cli-version-gate.md 包 2 第 3 點「測試」）。
//! 擋板表在測試裡是每條執行緒一張（`gate::table`），各測試互不干擾。

use super::*;
use std::os::unix::fs::PermissionsExt;

const GOOD: &str = "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then cat \"$(dirname \"$0\")/../ver\"; exit 0; fi\nexit 0\n";

struct Fake {
    dir: PathBuf,
}

impl Fake {
    fn new(cli: &str, version: &str) -> Self {
        let mut nonce = [0u8; 6];
        getrandom::fill(&mut nonce).unwrap();
        let dir = std::env::temp_dir().join(format!(
            "tt-update-{cli}-{}",
            nonce.iter().map(|b| format!("{b:02x}")).collect::<String>()
        ));
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::create_dir_all(dir.join("root")).unwrap();
        let fake = Self { dir };
        fake.write_binary(cli, GOOD);
        fake.set_version(version);
        fake
    }

    fn binary(&self, cli: &str) -> PathBuf {
        self.dir.join("bin").join(cli)
    }

    fn write_binary(&self, cli: &str, body: &str) {
        let path = self.binary(cli);
        std::fs::write(&path, body).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn set_version(&self, version: &str) {
        std::fs::write(self.dir.join("ver"), format!("{version}\n")).unwrap();
    }

    fn root(&self) -> PathBuf {
        self.dir.join("root")
    }

    /// install／restore 是 sh 腳本，`$DIR` 指向這個假環境、`$BIN` 是假 CLI。
    fn ctx(&self, cli: &str, install: &str, restore: Option<&str>) -> Ctx {
        self.ctx_with(cli, install, restore, None, None)
    }

    /// `local`：先試的本機連結還原 (連結, 原版檔)；`stale`：還原後要清的 (連結, 目錄)。
    fn ctx_with(
        &self,
        cli: &str,
        install: &str,
        restore: Option<&str>,
        local: Option<(PathBuf, PathBuf)>,
        stale: Option<(PathBuf, PathBuf)>,
    ) -> Ctx {
        let binary = self.binary(cli);
        let located = binary.clone();
        Ctx {
            data_root: self.root(),
            recipes: Box::new(Scripts {
                dir: self.dir.clone(),
                binary,
                install: install.to_owned(),
                restore: restore.map(str::to_owned),
                local,
                stale,
                lock_root: None,
            }),
            locate: Box::new(move |_| located.is_file().then(|| located.clone())),
            step_limit: Duration::from_secs(2),
        }
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(self.root(), std::fs::Permissions::from_mode(0o755));
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

struct Scripts {
    dir: PathBuf,
    binary: PathBuf,
    install: String,
    restore: Option<String>,
    local: Option<(PathBuf, PathBuf)>,
    stale: Option<(PathBuf, PathBuf)>,
    /// 注入寫入失敗：開始安裝時把資料根目錄改成唯讀（標記寫不進去）
    lock_root: Option<PathBuf>,
}

impl Scripts {
    fn step(&self, script: &str) -> Step {
        Step {
            argv: vec!["sh".to_owned(), "-c".to_owned(), script.to_owned()],
            envs: vec![
                ("DIR".to_owned(), self.dir.to_string_lossy().into_owned()),
                ("BIN".to_owned(), self.binary.to_string_lossy().into_owned()),
            ],
        }
    }
}

impl Recipes for Scripts {
    fn install(&self, _cli: &str, _binary: &Path, target: &Target) -> Step {
        if let Some(root) = &self.lock_root {
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o555)).unwrap();
        }
        let mut step = self.step(&self.install);
        if let Target::Version(version) = target {
            step.envs.push(("TARGET".to_owned(), version.clone()));
        }
        step
    }

    fn restore(&self, _cli: &str, original: &Version) -> Vec<Restore> {
        let mut candidates = Vec::new();
        if let Some((link, dest)) = &self.local {
            candidates.push(Restore::Relink {
                links: vec![link.clone()],
                dest: dest.clone(),
            });
        }
        if let Some(script) = &self.restore {
            let mut step = self.step(script);
            step.envs
                .push(("ORIGINAL".to_owned(), original.to_string()));
            candidates.push(Restore::Run(step));
        }
        candidates
    }

    fn stale_after_restore(&self, _cli: &str) -> Option<(PathBuf, PathBuf)> {
        self.stale.clone()
    }

    fn probe_envs(&self, _cli: &str) -> Vec<(String, String)> {
        Vec::new()
    }
}

/// 把假 CLI 換成跑不起來的版本
const BREAK: &str = "printf '#!/bin/sh\\nexit 1\\n' > \"$BIN\"";
/// 還原腳本：寫回能跑的假 CLI 與原版本（不呼叫 CLI 本身）
const RESTORE_OK: &str = "printf '#!/bin/sh\\nif [ \"$1\" = \"--version\" ]; then cat \"$(dirname \"$0\")/../ver\"; exit 0; fi\\nexit 0\\n' > \"$BIN\"; chmod +x \"$BIN\"; echo \"$ORIGINAL (Claude Code)\" > \"$DIR/ver\"";

#[tokio::test]
async fn tested_version_installs_and_clears_the_marker() {
    let fake = Fake::new("claude", "2.1.300 (Claude Code)");
    let ctx = fake.ctx(
        "claude",
        "echo \"$TARGET (Claude Code)\" > \"$DIR/ver\"",
        None,
    );
    let outcome = run(&ctx, "claude", Mode::Tested, None).await;
    assert_eq!(
        outcome,
        Outcome::Updated {
            version: "2.1.311 (Claude Code)".to_owned()
        }
    );
    assert!(marker::read(&fake.root(), "claude").is_none());
    assert!(!gate::is_raised("claude"));
    // 安裝 log 有寫
    assert!(
        std::fs::read_dir(fake.root().join("install-logs"))
            .unwrap()
            .count()
            >= 1
    );
}

#[tokio::test]
async fn wrong_version_is_restored_and_reported_as_failed() {
    let fake = Fake::new("claude", "2.1.300 (Claude Code)");
    let ctx = fake.ctx(
        "claude",
        "echo '2.1.305 (Claude Code)' > \"$DIR/ver\"",
        Some(RESTORE_OK),
    );
    let outcome = run(&ctx, "claude", Mode::Tested, None).await;
    assert_eq!(
        outcome,
        Outcome::Failed {
            version: "2.1.300 (Claude Code)".to_owned(),
            tested: true
        }
    );
    assert!(marker::read(&fake.root(), "claude").is_none());
}

/// 已測版下載失敗：這支沒動過，不用還原；`tested` 讓前端改給「裝最新版」。
#[tokio::test]
async fn tested_download_failure_offers_latest_without_restoring() {
    let fake = Fake::new("claude", "2.1.300 (Claude Code)");
    let ctx = fake.ctx("claude", "exit 22", Some("touch \"$DIR/restored\"; exit 1"));
    let outcome = run(&ctx, "claude", Mode::Tested, None).await;
    assert_eq!(
        outcome,
        Outcome::Failed {
            version: "2.1.300 (Claude Code)".to_owned(),
            tested: true
        }
    );
    assert!(!fake.dir.join("restored").exists());
    // 接著按「裝最新版」
    let ctx = fake.ctx(
        "claude",
        "echo '2.1.400 (Claude Code)' > \"$DIR/ver\"",
        None,
    );
    assert_eq!(
        run(&ctx, "claude", Mode::Latest, None).await,
        Outcome::Updated {
            version: "2.1.400 (Claude Code)".to_owned()
        }
    );
}

#[tokio::test]
async fn broken_new_binary_is_restored_by_the_official_script() {
    let fake = Fake::new("claude", "2.1.300 (Claude Code)");
    let ctx = fake.ctx("claude", BREAK, Some(RESTORE_OK));
    let outcome = run(&ctx, "claude", Mode::Tested, None).await;
    assert_eq!(
        outcome,
        Outcome::Failed {
            version: "2.1.300 (Claude Code)".to_owned(),
            tested: true
        }
    );
    assert!(marker::read(&fake.root(), "claude").is_none());
}

#[tokio::test]
async fn failed_restore_keeps_the_marker_lowers_the_barrier_and_reinstall_clears_it() {
    let fake = Fake::new("claude", "2.1.300 (Claude Code)");
    let ctx = fake.ctx("claude", BREAK, Some("exit 1"));
    assert_eq!(
        run(&ctx, "claude", Mode::Tested, None).await,
        Outcome::Broken
    );
    let kept = marker::read(&fake.root(), "claude").unwrap();
    assert!(kept.restore_failed);
    // 擋板已撤：按重新安裝不會被 CliUpdating 擋住
    assert!(!gate::is_raised("claude"));
    assert_eq!(state(&fake.root(), "claude"), Some("broken"));
    assert!(needs_reinstall(&fake.root(), "claude"));
    // 啟動時看到 restore_failed：不再重跑還原、不立擋板
    assert!(raise_pending(&fake.root()).is_empty());
    // 重新安裝後 --version 驗證通過才清
    clear_after_reinstall(&fake.root(), "claude");
    assert!(marker::read(&fake.root(), "claude").is_none());
    assert_eq!(state(&fake.root(), "claude"), None);
}

/// 裝到別處、app 用的這支沒變：判失敗，不還原。
#[tokio::test]
async fn install_elsewhere_counts_as_failure() {
    let fake = Fake::new("grok", "grok 1.0.60 (x) [stable]");
    let ctx = fake.ctx(
        "grok",
        "mkdir -p \"$DIR/elsewhere\" && echo 'grok 1.0.70' > \"$DIR/elsewhere/ver\"",
        Some("touch \"$DIR/restored\""),
    );
    let outcome = run(&ctx, "grok", Mode::Tested, None).await;
    assert_eq!(
        outcome,
        Outcome::Failed {
            version: "grok 1.0.60 (x) [stable]".to_owned(),
            tested: true
        }
    );
    assert!(!fake.dir.join("restored").exists());
}

fn pid_from(path: &Path) -> u32 {
    for _ in 0..50 {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(pid) = text.trim().parse() {
                return pid;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("孫程序沒寫 pid");
}

/// 更新逾時殺整組，含孫程序。
#[tokio::test]
async fn timed_out_update_kills_the_whole_tree() {
    let fake = Fake::new("codex", "codex-cli 0.160.0");
    let ctx = fake.ctx(
        "codex",
        "sh -c 'echo $$ > \"$DIR/grandchild\"; exec sleep 30' & wait",
        Some("touch \"$DIR/restored\""),
    );
    let started = std::time::Instant::now();
    let outcome = run(&ctx, "codex", Mode::Tested, None).await;
    assert!(started.elapsed() < std::time::Duration::from_secs(20));
    // 這支沒動過：失敗、不還原
    assert_eq!(
        outcome,
        Outcome::Failed {
            version: "codex-cli 0.160.0".to_owned(),
            tested: true
        }
    );
    assert!(!gate::pid_alive(pid_from(&fake.dir.join("grandchild"))));
}

#[tokio::test]
async fn timed_out_restore_reports_broken() {
    let fake = Fake::new("codex", "codex-cli 0.160.0");
    let ctx = fake.ctx("codex", BREAK, Some("sleep 30"));
    assert_eq!(
        run(&ctx, "codex", Mode::Tested, None).await,
        Outcome::Broken
    );
    assert!(marker::read(&fake.root(), "codex").unwrap().restore_failed);
}

#[tokio::test]
async fn latest_that_changes_nothing_is_already_latest() {
    let fake = Fake::new("grok", "grok 1.0.80 (x) [stable]");
    let ctx = fake.ctx("grok", "echo up to date", None);
    assert_eq!(
        run(&ctx, "grok", Mode::Latest, None).await,
        Outcome::AlreadyLatest {
            version: "grok 1.0.80 (x) [stable]".to_owned()
        }
    );
}

/// 已測版不比目前新，或伺服器要求的版本比已測版高：跳過已測版。
#[tokio::test]
async fn tested_not_newer_skips_straight_to_latest() {
    let fake = Fake::new("claude", "2.1.311 (Claude Code)");
    let ctx = fake.ctx("claude", "touch \"$DIR/ran\"", None);
    assert_eq!(
        run(&ctx, "claude", Mode::Tested, None).await,
        Outcome::TestedNotNewer
    );
    fake.set_version("2.1.300 (Claude Code)");
    assert_eq!(
        run(&ctx, "claude", Mode::Tested, Some("2.2.0")).await,
        Outcome::TestedNotNewer
    );
    assert!(!fake.dir.join("ran").exists());
    assert!(marker::read(&fake.root(), "claude").is_none());
}

/// agy 一律裝最新版；新檔壞掉就把更新前的副本複製回去。
#[tokio::test]
async fn agy_installs_latest_and_restores_from_its_backup() {
    let fake = Fake::new("agy", "1.4.9");
    let ctx = fake.ctx(
        "agy",
        &format!("[ -z \"$TARGET\" ] || exit 9; {BREAK}"),
        None,
    );
    assert_eq!(
        run(&ctx, "agy", Mode::Tested, None).await,
        Outcome::Failed {
            version: "1.4.9".to_owned(),
            tested: false
        }
    );
    // 副本已複製回去、跑得起來；備份與標記都清掉
    assert_eq!(std::fs::read_to_string(fake.binary("agy")).unwrap(), GOOD);
    assert!(!marker::backup_path(&fake.root(), "agy").exists());
    assert!(marker::read(&fake.root(), "agy").is_none());
}

#[tokio::test]
async fn live_inflight_call_refuses_and_dead_one_is_cleared() {
    let fake = Fake::new("grok", "grok 1.0.60 (x) [stable]");
    let ctx = fake.ctx("grok", "echo \"grok $TARGET\" > \"$DIR/ver\"", None);
    let slot = gate::enter("grok").unwrap();
    slot.set_pid(Some(std::process::id()));
    assert_eq!(run(&ctx, "grok", Mode::Tested, None).await, Outcome::Busy);
    // 死掉的 pid（沒收屍的殘影）不會讓按鈕永久停用
    slot.set_pid(Some(999_999_999));
    assert!(matches!(
        run(&ctx, "grok", Mode::Tested, None).await,
        Outcome::Updated { .. }
    ));
    drop(slot);
}

/// 擋板期間其他經 spawn 的呼叫（含 --version）立即回 CliUpdating。
#[tokio::test]
async fn calls_during_an_update_get_cli_updating() {
    let fake = Fake::new("codex", "codex-cli 0.160.0");
    let ctx = fake.ctx(
        "codex",
        "touch \"$DIR/started\"; sleep 1; echo \"codex-cli $TARGET\" > \"$DIR/ver\"",
        None,
    );
    let binary = fake.binary("codex");
    let started = fake.dir.join("started");
    let probe = async {
        while !started.exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let mut command = tokio::process::Command::new(&binary);
        command.arg("--version");
        let error = super::super::spawn::spawn(&mut command, &[]).err().unwrap();
        assert!(error.to_string().contains("cli_updating"), "{error}");
        assert_eq!(state(&fake.root(), "codex"), Some("updating"));
    };
    let (outcome, ()) = tokio::join!(run(&ctx, "codex", Mode::Tested, None), probe);
    assert!(matches!(outcome, Outcome::Updated { .. }));
    let mut command = tokio::process::Command::new(&binary);
    command.arg("--version");
    assert!(super::super::spawn::spawn(&mut command, &[]).is_ok());
}

/// 啟動復原：依目標版本判斷，該還原的才還原；擋板處理完才撤。
#[tokio::test]
async fn startup_recovery_follows_the_marker() {
    let fake = Fake::new("claude", "2.1.300 (Claude Code)");
    let base = marker::Marker {
        cli: "claude".to_owned(),
        path: fake.binary("claude"),
        version: "2.1.300 (Claude Code)".to_owned(),
        backup: None,
        target: Target::Version("2.1.311".to_owned()),
        restore_failed: false,
        group: None,
    };
    let ctx = fake.ctx("claude", "true", Some(RESTORE_OK));
    // 1. 版本等於原版本：沒動過，直接刪標記
    marker::write(&fake.root(), &base).unwrap();
    let mut pending = raise_pending(&fake.root());
    assert_eq!(pending.len(), 1);
    assert!(gate::is_raised("claude"));
    let (found, barrier) = pending.pop().unwrap();
    recover(&ctx, found, barrier).await;
    assert!(marker::read(&fake.root(), "claude").is_none());
    assert!(!gate::is_raised("claude"));
    // 2. 已經是目標版本：更新其實成功了
    fake.set_version("2.1.311 (Claude Code)");
    marker::write(&fake.root(), &base).unwrap();
    let (found, barrier) = raise_pending(&fake.root()).pop().unwrap();
    recover(&ctx, found, barrier).await;
    assert!(marker::read(&fake.root(), "claude").is_none());
    assert_eq!(
        std::fs::read_to_string(fake.dir.join("ver"))
            .unwrap()
            .trim(),
        "2.1.311 (Claude Code)"
    );
    // 3. 半升級（跑不起來）：照契約還原
    fake.write_binary("claude", "#!/bin/sh\nexit 1\n");
    marker::write(&fake.root(), &base).unwrap();
    let (found, barrier) = raise_pending(&fake.root()).pop().unwrap();
    recover(&ctx, found, barrier).await;
    assert!(marker::read(&fake.root(), "claude").is_none());
    assert_eq!(
        std::fs::read_to_string(fake.dir.join("ver"))
            .unwrap()
            .trim(),
        "2.1.300 (Claude Code)"
    );
    // 4. 還原也失敗：記 restore_failed、擋板撤掉
    fake.write_binary("claude", "#!/bin/sh\nexit 1\n");
    marker::write(&fake.root(), &base).unwrap();
    let ctx = fake.ctx("claude", "true", Some("exit 1"));
    let (found, barrier) = raise_pending(&fake.root()).pop().unwrap();
    recover(&ctx, found, barrier).await;
    assert!(marker::read(&fake.root(), "claude").unwrap().restore_failed);
    assert!(!gate::is_raised("claude"));
}

/// 兩家同時更新：各自的標記與擋板互不影響。
/// 兩支假安裝都停在柵欄（go 檔）上，看到兩份標記同時存在才放行，不靠時間猜。
#[tokio::test]
async fn two_clis_update_concurrently_with_separate_markers() {
    let claude = Fake::new("claude", "2.1.300 (Claude Code)");
    let grok = Fake::new("grok", "grok 1.0.60 (x) [stable]");
    const WAIT_GO: &str = "while [ ! -e \"$DIR/go\" ]; do sleep 0.02; done; ";
    // 兩家共用同一個資料根目錄
    let mut claude_ctx = claude.ctx(
        "claude",
        &format!("{WAIT_GO}echo \"$TARGET (Claude Code)\" > \"$DIR/ver\""),
        None,
    );
    let mut grok_ctx = grok.ctx(
        "grok",
        &format!("{WAIT_GO}echo \"grok $TARGET\" > \"$DIR/ver\""),
        None,
    );
    claude_ctx.data_root = claude.root();
    grok_ctx.data_root = claude.root();
    // 等柵欄的時間不能吃掉更新上限
    claude_ctx.step_limit = Duration::from_secs(60);
    grok_ctx.step_limit = Duration::from_secs(60);
    let watch = async {
        while marker::read(&claude.root(), "claude").is_none()
            || marker::read(&claude.root(), "grok").is_none()
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        std::fs::write(claude.dir.join("go"), "").unwrap();
        std::fs::write(grok.dir.join("go"), "").unwrap();
    };
    let (a, b, ()) = tokio::time::timeout(Duration::from_secs(60), async {
        tokio::join!(
            run(&claude_ctx, "claude", Mode::Tested, None),
            run(&grok_ctx, "grok", Mode::Tested, None),
            watch
        )
    })
    .await
    .expect("兩家的標記沒有同時存在（柵欄沒放行）");
    assert!(matches!(a, Outcome::Updated { .. }));
    assert!(matches!(b, Outcome::Updated { .. }));
    assert!(marker::read(&claude.root(), "claude").is_none());
    assert!(marker::read(&claude.root(), "grok").is_none());
}

fn read_logs(root: &Path) -> String {
    std::fs::read_dir(root.join("install-logs"))
        .unwrap()
        .flatten()
        .map(|entry| std::fs::read_to_string(entry.path()).unwrap_or_default())
        .collect()
}

/// 本機還留著原版：先把連結指回去（不跑下載腳本），log 記下走的路徑；指不到的新版檔清掉。
#[tokio::test]
async fn restore_relinks_the_local_original_before_downloading() {
    let fake = Fake::new("claude", "2.1.300 (Claude Code)");
    let versions = fake.dir.join("versions");
    std::fs::create_dir_all(&versions).unwrap();
    let original = versions.join("2.1.300");
    let new = versions.join("2.1.311");
    std::fs::rename(fake.binary("claude"), &original).unwrap();
    std::fs::copy(&original, &new).unwrap();
    std::os::unix::fs::symlink(&new, fake.binary("claude")).unwrap();
    // 更新把新版檔弄壞（經連結寫進 versions/2.1.311）
    let ctx = fake.ctx_with(
        "claude",
        BREAK,
        Some("touch \"$DIR/script-ran\"; exit 1"),
        Some((fake.binary("claude"), original.clone())),
        Some((fake.binary("claude"), versions.clone())),
    );
    assert_eq!(
        run(&ctx, "claude", Mode::Tested, None).await,
        Outcome::Failed {
            version: "2.1.300 (Claude Code)".to_owned(),
            tested: true
        }
    );
    assert!(!fake.dir.join("script-ran").exists());
    assert_eq!(std::fs::read_link(fake.binary("claude")).unwrap(), original);
    assert!(!new.exists(), "壞掉的新版檔應清掉");
    assert!(original.exists());
    let logs = read_logs(&fake.root());
    assert!(logs.contains("restore: relink"), "{logs}");
    assert!(logs.contains("2.1.300 (ok)"), "{logs}");
}

/// 本機那份指回去還是不對：接著跑官方腳本。
#[tokio::test]
async fn restore_falls_back_to_the_script_when_the_local_copy_is_wrong() {
    let fake = Fake::new("claude", "2.1.300 (Claude Code)");
    let wrong = fake.dir.join("wrong");
    std::fs::write(&wrong, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&wrong, std::fs::Permissions::from_mode(0o755)).unwrap();
    let ctx = fake.ctx_with(
        "claude",
        BREAK,
        Some(RESTORE_OK),
        Some((fake.binary("claude"), wrong)),
        None,
    );
    assert_eq!(
        run(&ctx, "claude", Mode::Tested, None).await,
        Outcome::Failed {
            version: "2.1.300 (Claude Code)".to_owned(),
            tested: true
        }
    );
}

/// `--version` 跑得起來但版號解析不出來：照樣放行更新，不判無法執行。
#[tokio::test]
async fn unparsable_version_still_updates() {
    let fake = Fake::new("claude", "Claude Code nightly build");
    let ctx = fake.ctx(
        "claude",
        "echo \"$TARGET (Claude Code)\" > \"$DIR/ver\"",
        None,
    );
    assert!(matches!(
        run(&ctx, "claude", Mode::Tested, None).await,
        Outcome::Updated { .. }
    ));
    // 沒動到：依原文判定沒變，不還原
    fake.set_version("Claude Code nightly build");
    let ctx = fake.ctx("claude", "exit 1", Some("touch \"$DIR/restored\""));
    assert_eq!(
        run(&ctx, "claude", Mode::Latest, None).await,
        Outcome::Failed {
            version: "Claude Code nightly build".to_owned(),
            tested: false
        }
    );
    assert!(!fake.dir.join("restored").exists());
}

/// 還原失敗標記：偵測到版號解析得出來就清（不看修改時間）。
#[tokio::test]
async fn restore_failed_clears_once_the_version_parses() {
    let fake = Fake::new("claude", "2.1.300 (Claude Code)");
    let ctx = fake.ctx("claude", BREAK, Some("exit 1"));
    assert_eq!(
        run(&ctx, "claude", Mode::Tested, None).await,
        Outcome::Broken
    );
    clear_if_runs(&fake.root(), "claude", false);
    assert!(needs_reinstall(&fake.root(), "claude"));
    clear_if_runs(&fake.root(), "claude", true);
    assert!(marker::read(&fake.root(), "claude").is_none());
}

/// 標記記下更新程序的群組；啟動復原先殺掉還活著的那組，再判定。
#[tokio::test]
async fn startup_recovery_kills_the_previous_group_first() {
    let fake = Fake::new("grok", "grok 1.0.60 (x) [stable]");
    // 更新時標記裡要有群組
    let ctx = fake.ctx(
        "grok",
        "cp \"$DIR/root/cli-update-pending-grok.json\" \"$DIR/seen.json\"; echo \"grok $TARGET\" > \"$DIR/ver\"",
        None,
    );
    assert!(matches!(
        run(&ctx, "grok", Mode::Tested, None).await,
        Outcome::Updated { .. }
    ));
    let seen: marker::Marker =
        serde_json::from_str(&std::fs::read_to_string(fake.dir.join("seen.json")).unwrap())
            .unwrap();
    let group = seen.group.expect("標記應記下群組");
    assert!(group.started.is_some());
    // 上一輪留下一組還活著的程序（自成群組）
    let mut command = tokio::process::Command::new("sleep");
    command.arg("30").process_group(0);
    let mut child = command.spawn().unwrap();
    let pid = child.id().unwrap() as i32;
    let base = marker::Marker {
        cli: "grok".to_owned(),
        path: fake.binary("grok"),
        version: "grok 1.0.70 (x) [stable]".to_owned(),
        backup: None,
        target: Target::Latest,
        restore_failed: false,
        group: Some(tree::Group {
            pgid: pid,
            started: tree::start_time_of(pid),
        }),
    };
    marker::write(&fake.root(), &base).unwrap();
    let (found, barrier) = raise_pending(&fake.root()).pop().unwrap();
    recover(&ctx, found, barrier).await;
    let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
    assert!(!gate::pid_alive(pid as u32), "上一輪那組應被殺掉");
    assert!(marker::read(&fake.root(), "grok").is_none());
    assert!(!gate::is_raised("grok"));
}

/// 確認不了上一輪那組是不是我們的（組長還在、身分對不上之外的情況）：擋板不撤、設定頁顯示停不下來。
#[tokio::test]
async fn unconfirmed_group_keeps_the_barrier_and_reports_stuck() {
    let fake = Fake::new("grok", "grok 1.0.60 (x) [stable]");
    let mut command = tokio::process::Command::new("sleep");
    command.arg("30").process_group(0);
    let mut child = command.spawn().unwrap();
    let pid = child.id().unwrap() as i32;
    let base = marker::Marker {
        cli: "grok".to_owned(),
        path: fake.binary("grok"),
        version: "grok 1.0.60 (x) [stable]".to_owned(),
        backup: None,
        target: Target::Latest,
        restore_failed: false,
        // 沒有啟動時間：無法確認組長是不是當初那支
        group: Some(tree::Group {
            pgid: pid,
            started: None,
        }),
    };
    marker::write(&fake.root(), &base).unwrap();
    let ctx = fake.ctx("grok", "true", None);
    let (found, barrier) = raise_pending(&fake.root()).pop().unwrap();
    recover(&ctx, found, barrier).await;
    assert!(gate::is_raised("grok"));
    assert_eq!(state(&fake.root(), "grok"), Some("stuck"));
    assert!(gate::pid_alive(pid as u32), "身分不明的不殺");
    assert!(marker::read(&fake.root(), "grok").is_some());
    gate::release_held("grok");
    let _ = child.kill().await;
}

/// 組長的 pid 已被別的程序重用（啟動時間對不上）：我們那組早就沒了，不殺、照常判定。
#[tokio::test]
async fn reused_leader_pid_is_not_killed() {
    let fake = Fake::new("grok", "grok 1.0.60 (x) [stable]");
    let mut command = tokio::process::Command::new("sleep");
    command.arg("30").process_group(0);
    let mut child = command.spawn().unwrap();
    let pid = child.id().unwrap() as i32;
    let started = tree::start_time_of(pid).unwrap();
    let base = marker::Marker {
        cli: "grok".to_owned(),
        path: fake.binary("grok"),
        version: "grok 1.0.60 (x) [stable]".to_owned(),
        backup: None,
        target: Target::Latest,
        restore_failed: false,
        group: Some(tree::Group {
            pgid: pid,
            started: Some(started + 1000),
        }),
    };
    marker::write(&fake.root(), &base).unwrap();
    let ctx = fake.ctx("grok", "true", None);
    let (found, barrier) = raise_pending(&fake.root()).pop().unwrap();
    recover(&ctx, found, barrier).await;
    assert!(gate::pid_alive(pid as u32));
    assert!(!gate::is_raised("grok"));
    assert!(marker::read(&fake.root(), "grok").is_none());
    let _ = child.kill().await;
}

/// 正常結束也等整組消失：安裝腳本丟到背景的程序一併收掉，才往下判定、撤擋板。
#[tokio::test]
async fn normal_exit_still_stops_leftover_background_processes() {
    let fake = Fake::new("codex", "codex-cli 0.160.0");
    let ctx = fake.ctx(
        "codex",
        "sh -c 'echo $$ > \"$DIR/grandchild\"; exec sleep 30' >/dev/null 2>&1 & while [ ! -s \"$DIR/grandchild\" ]; do sleep 0.05; done; echo \"codex-cli $TARGET\" > \"$DIR/ver\"",
        None,
    );
    assert!(matches!(
        run(&ctx, "codex", Mode::Tested, None).await,
        Outcome::Updated { .. }
    ));
    assert!(!gate::pid_alive(pid_from(&fake.dir.join("grandchild"))));
    assert!(!gate::is_raised("codex"));
}

/// spawn 後群組身分寫不進標記：立刻停掉整組（不讓它跑完），照常驗證後判失敗、擋板撤掉。
#[tokio::test]
async fn group_write_failure_stops_the_step_and_fails() {
    let fake = Fake::new("claude", "2.1.300 (Claude Code)");
    let install = "sleep 1; echo \"$TARGET (Claude Code)\" > \"$DIR/ver\"";
    let ctx = fake.ctx("claude", install, None);
    let ctx = Ctx {
        recipes: Box::new(Scripts {
            dir: fake.dir.clone(),
            binary: fake.binary("claude"),
            install: install.to_owned(),
            restore: None,
            local: None,
            stale: None,
            lock_root: Some(fake.root()),
        }),
        step_limit: Duration::from_secs(30),
        ..ctx
    };
    let started = std::time::Instant::now();
    let outcome = run(&ctx, "claude", Mode::Tested, None).await;
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "應立刻停掉、不是等逾時"
    );
    assert_eq!(
        outcome,
        Outcome::Failed {
            version: "2.1.300 (Claude Code)".to_owned(),
            tested: true
        }
    );
    // 安裝程序沒跑完：版本沒動
    std::thread::sleep(std::time::Duration::from_millis(1500));
    assert_eq!(
        std::fs::read_to_string(fake.dir.join("ver"))
            .unwrap()
            .trim(),
        "2.1.300 (Claude Code)"
    );
    assert!(!gate::is_raised("claude"));
    assert!(!gate::is_stuck("claude"));
    // 唯讀目錄刪不掉標記：留著的是 spawn 前那份（沒有群組），下次啟動依實際版本判定沒動過
    let kept = marker::read(&fake.root(), "claude").unwrap();
    assert!(kept.group.is_none());
    assert!(read_logs(&fake.root()).contains("marker write failed"));
}
