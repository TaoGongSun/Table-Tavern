//! app 內的 CLI 更新鈕（plans/cli-version-gate.md 包 2 第 3 點）。大宗玩家唯一的升版路徑，必須可靠：
//! 擋板擋住新呼叫 → 寫啟動復原標記 → 跑官方更新指令（程序群組、5 分鐘上限）→ 以 `find_binary` 那支的
//! `--version` 驗證 → 不符就照還原契約還原 → 還原也驗證；還原失敗才請玩家重新安裝。

pub(crate) mod gate;
pub(crate) mod marker;
pub(crate) mod recipe;
pub(crate) mod tree;

use super::tested_versions;
use recipe::{Recipes, Restore, Step, Target};
use semver::Version;
use serde::Serialize;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 更新與還原各自的上限；逾時殺整組。
const STEP_LIMIT: Duration = Duration::from_secs(5 * 60);
/// `--version` 的上限。
const VERSION_LIMIT: Duration = Duration::from_secs(30);

pub(crate) struct Ctx {
    pub data_root: PathBuf,
    pub recipes: Box<dyn Recipes>,
    /// app 平常呼叫的那支（正式＝`find_binary`）
    pub locate: Box<dyn Fn(&str) -> Option<PathBuf> + Send + Sync>,
    pub step_limit: Duration,
}

impl Ctx {
    pub(crate) fn official(
        data_root: PathBuf,
        home: PathBuf,
        agy_envs: Vec<(String, String)>,
    ) -> Self {
        Self {
            data_root,
            recipes: Box::new(recipe::Official { home, agy_envs }),
            locate: Box::new(super::find_binary),
            step_limit: STEP_LIMIT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Mode {
    /// 先裝已測版（agy 一律最新版）
    Tested,
    Latest,
}

/// 更新鈕的結果（前端依 kind 出文案與下一顆按鈕）。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Outcome {
    /// 已更新到 version：請再送一次（不自動重送，不替玩家花額度）
    Updated {
        version: String,
    },
    /// 已測版不比目前新（或伺服器要求的版本比已測版高）：同一位置直接給「裝最新版」
    TestedNotNewer,
    /// 最新版就是目前這版：更新沒有解決問題
    AlreadyLatest {
        version: String,
    },
    /// 更新失敗、仍是原本的 version（還原驗證過）；`tested`＝這次裝的是已測版，可改給「裝最新版」
    Failed {
        version: String,
        tested: bool,
    },
    /// 還原也失敗：CLI 無法執行，請按重新安裝
    Broken,
    /// 更新或還原的程序停不下來：擋板留著，請重新啟動 app（啟動復原會再處理）
    Stuck,
    /// 還有在途呼叫：等目前這輪結束
    Busy,
    /// 已經有一個更新在跑
    AlreadyUpdating,
    NotFound,
}

fn parse_line(line: &str) -> Option<Version> {
    tested_versions::parse(line)
}

/// 更新或還原的程序（整組）確認不了已停：不能往下走（還在寫檔的程序會跟還原打架），擋板也不撤。
#[derive(Debug)]
pub(crate) struct Unstopped;

fn note(log: &mut Option<File>, text: &[u8]) {
    if let Some(file) = log.as_mut() {
        let _ = file.write_all(text);
    }
}

/// 標記檔記下這一步的程序群組（啟動復原先確認它停了才判定）。
fn record_group(
    data_root: &Path,
    pending: &mut marker::Marker,
    group: Option<tree::Group>,
) -> std::io::Result<()> {
    pending.group = group;
    marker::write(data_root, pending)
}

/// 照更新模式的環境跑一步，輸出寫進安裝 log；spawn 後先把群組交給 `on_spawn` 記進標記。
/// 記不進去（當機後無從確認這組停了沒）：立刻停掉整組，這一步算失敗，交給呼叫端照常驗證。
/// 回 Ok(成功與否)；整組確認不了已停回 Err。
async fn run_step(
    step: &Step,
    limit: Duration,
    log: &mut Option<File>,
    on_spawn: &mut (dyn FnMut(Option<tree::Group>) -> std::io::Result<()> + Send),
) -> Result<bool, Unstopped> {
    let Some((program, args)) = step.argv.split_first() else {
        return Ok(false);
    };
    let mut command = tokio::process::Command::new(program);
    super::proxy::apply_system_proxy(&mut command);
    for key in recipe::REMOVED_ENVS {
        command.env_remove(key);
    }
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("ANTHROPIC_") {
            command.env_remove(&key);
        }
    }
    command
        .args(args)
        .envs(
            step.envs
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        )
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    note(log, format!("\n$ {}\n", step.argv.join(" ")).as_bytes());
    let child = match super::spawn::spawn_update(&mut command) {
        Ok(child) => child,
        Err(error) => {
            note(log, format!("spawn failed: {error}\n").as_bytes());
            return Ok(false);
        }
    };
    if let Err(error) = on_spawn(child.group()) {
        log::warn!("寫 CLI 更新標記失敗，停掉這一步：{error}");
        note(
            log,
            format!("marker write failed ({error}): stopping this step\n").as_bytes(),
        );
        if !child.finish(Duration::ZERO).await.stopped {
            note(log, b"process group did not stop\n");
            return Err(Unstopped);
        }
        return Ok(false);
    }
    let finished = child.finish(limit).await;
    match &finished.output {
        Some(output) => {
            note(log, &output.stdout);
            note(log, &output.stderr);
            note(log, format!("exit: {}\n", output.status).as_bytes());
        }
        None => note(
            log,
            format!("timed out (or wait failed) after {}s\n", limit.as_secs()).as_bytes(),
        ),
    }
    if !finished.stopped {
        note(log, b"process group did not stop\n");
        return Err(Unstopped);
    }
    Ok(finished
        .output
        .is_some_and(|output| output.status.success()))
}

/// `<binary> --version` 第一行（更新模式跑，不過擋板）；跑不起來回 Ok(None)。
async fn version_line(ctx: &Ctx, cli: &str, binary: &Path) -> Result<Option<String>, Unstopped> {
    let mut command = tokio::process::Command::new(binary);
    command
        .arg("--version")
        .envs(super::spawn::auto_update_off(cli).iter().copied())
        .envs(
            ctx.recipes
                .probe_envs(cli)
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        )
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let Ok(child) = super::spawn::spawn_update(&mut command) else {
        return Ok(None);
    };
    let finished = child.finish(VERSION_LIMIT).await;
    if !finished.stopped {
        return Err(Unstopped);
    }
    Ok(finished
        .output
        .filter(|output| output.status.success())
        .and_then(|output| {
            let line = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned();
            (!line.is_empty()).then_some(line)
        }))
}

/// 現在 app 會用的那支的 `--version` 原文與解析出的版號（重新 `find_binary`，裝到別處而這支沒變也看得出來）。
type Current = Option<(String, Option<Version>)>;

async fn current(ctx: &Ctx, cli: &str) -> Result<Current, Unstopped> {
    let Some(binary) = (ctx.locate)(cli) else {
        return Ok(None);
    };
    Ok(version_line(ctx, cli, &binary).await?.map(|line| {
        let version = parse_line(&line);
        (line, version)
    }))
}

/// 還是原本那版：版號相同，或（版號解析不出來時）`--version` 原文相同。
fn is_original(before_line: &str, before: Option<&Version>, now: &Current) -> bool {
    match now {
        Some((line, after)) => {
            line == before_line
                || matches!((before, after), (Some(before), Some(after)) if before == after)
        }
        None => false,
    }
}

/// 驗證規則：指定版要等於目標；最新版要比原本新（原本的版號解析不出來時，新的解析得出來就算）。
fn verified(target: &Target, before: Option<&Version>, after: Option<&Version>) -> bool {
    let Some(after) = after else {
        return false;
    };
    match target {
        Target::Version(version) => Version::parse(version).is_ok_and(|target| &target == after),
        Target::Latest => before.is_none_or(|before| after > before),
    }
}

fn open_log(data_root: &Path, cli: &str) -> Option<File> {
    let directory = data_root.join("install-logs");
    std::fs::create_dir_all(&directory).ok()?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or_default();
    std::fs::OpenOptions::new()
        .create_new(true)
        .append(true)
        .open(directory.join(format!("update-{cli}-{stamp}.log")))
        .ok()
}

/// 更新鈕。`required`＝伺服器要求的最低版本（`CliTooOld.required`），高於已測版就跳過已測版。
pub(crate) async fn run(
    ctx: &Ctx,
    cli: &'static str,
    mode: Mode,
    required: Option<&str>,
) -> Outcome {
    let Some(binary) = (ctx.locate)(cli) else {
        return Outcome::NotFound;
    };
    let barrier = match gate::try_raise(cli, gate::pid_alive) {
        gate::Raise::Raised(barrier) => barrier,
        gate::Raise::Busy => return Outcome::Busy,
        gate::Raise::AlreadyUpdating => return Outcome::AlreadyUpdating,
    };
    match run_raised(ctx, cli, &binary, mode, required).await {
        Ok(outcome) => {
            drop(barrier);
            outcome
        }
        Err(Unstopped) => {
            log::warn!("CLI 更新程序停不下來（{cli}）：擋板留到 app 重新啟動");
            gate::hold(barrier);
            Outcome::Stuck
        }
    }
}

async fn run_raised(
    ctx: &Ctx,
    cli: &'static str,
    binary: &Path,
    mode: Mode,
    required: Option<&str>,
) -> Result<Outcome, Unstopped> {
    // 原版本跑不起來就無從還原：直接請玩家重新安裝
    let Some(before_line) = version_line(ctx, cli, binary).await? else {
        return Ok(Outcome::Broken);
    };
    // 跑得起來但版號解析不出來（輸出格式改了）照樣放行，比照 below_minimum；判定改比對 `--version` 原文
    let before = parse_line(&before_line);
    let target = match (cli, mode) {
        ("agy", _) | (_, Mode::Latest) => Target::Latest,
        (_, Mode::Tested) => {
            let tested = tested_versions::tested(cli).and_then(|v| Version::parse(v).ok());
            let required = required.and_then(parse_line);
            match tested {
                Some(tested)
                    if before.as_ref().is_none_or(|before| &tested > before)
                        && required.as_ref().is_none_or(|r| r <= &tested) =>
                {
                    Target::Version(tested.to_string())
                }
                _ => return Ok(Outcome::TestedNotNewer),
            }
        }
    };
    let backup = match cli {
        "agy" => {
            let backup = marker::backup_path(&ctx.data_root, cli);
            let copied = backup
                .parent()
                .map(std::fs::create_dir_all)
                .unwrap_or(Ok(()))
                .and_then(|()| std::fs::copy(binary, &backup));
            if let Err(error) = copied {
                // 沒有備份就沒有退路：不更新
                log::warn!("agy 更新前備份失敗：{error}");
                return Ok(Outcome::Failed {
                    version: before_line,
                    tested: false,
                });
            }
            Some(backup)
        }
        _ => None,
    };
    let mut pending = marker::Marker {
        cli: cli.to_owned(),
        path: binary.to_path_buf(),
        version: before_line.clone(),
        backup,
        target: target.clone(),
        restore_failed: false,
        group: None,
    };
    if let Err(error) = marker::write(&ctx.data_root, &pending) {
        // 寫不了標記＝當機後無從復原：不更新
        log::warn!("寫 CLI 更新標記失敗（{cli}）：{error}");
        marker::clear(&ctx.data_root, cli);
        return Ok(Outcome::Failed {
            version: before_line,
            tested: false,
        });
    }
    let mut log = open_log(&ctx.data_root, cli);
    let step = ctx.recipes.install(cli, binary, &target);
    let data_root = ctx.data_root.as_path();
    let ran = run_step(&step, ctx.step_limit, &mut log, &mut |group| {
        record_group(data_root, &mut pending, group)
    })
    .await?;
    let tested = matches!(target, Target::Version(_));
    let now = current(ctx, cli).await?;
    if let Some((line, after)) = &now {
        if verified(&target, before.as_ref(), after.as_ref()) {
            marker::clear(&ctx.data_root, cli);
            return Ok(Outcome::Updated {
                version: line.clone(),
            });
        }
    }
    // 這支沒變（裝到別處、已是最新版、或下載失敗沒動到檔）：不用還原
    if is_original(&before_line, before.as_ref(), &now) {
        marker::clear(&ctx.data_root, cli);
        let version = now.map(|(line, _)| line).unwrap_or_default();
        return Ok(match (target, ran) {
            (Target::Latest, true) => Outcome::AlreadyLatest { version },
            _ => Outcome::Failed { version, tested },
        });
    }
    if restore(ctx, &mut log, &mut pending, before.as_ref()).await? {
        marker::clear(&ctx.data_root, cli);
        return Ok(Outcome::Failed {
            version: before_line,
            tested,
        });
    }
    pending.restore_failed = true;
    if let Err(error) = marker::write(&ctx.data_root, &pending) {
        log::warn!("寫 CLI 還原失敗標記失敗（{cli}）：{error}");
    }
    Ok(Outcome::Broken)
}

/// 照四家還原契約依序試（本機留存的原版優先、官方腳本殿後），每試一個就以 `find_binary` 那支的
/// `--version` 驗證，等於原版本才算成功。成功後清掉留在下載目錄、已經沒人指向的新版檔（grok）。
async fn restore(
    ctx: &Ctx,
    log: &mut Option<File>,
    pending: &mut marker::Marker,
    before: Option<&Version>,
) -> Result<bool, Unstopped> {
    let cli = pending.cli.clone();
    let before_line = pending.version.clone();
    if let Some(backup) = pending.backup.clone() {
        let copied = copy_back(&backup, &pending.path);
        note(
            log,
            format!(
                "\nrestore: copy {} -> {} ({})\n",
                backup.display(),
                pending.path.display(),
                if copied { "ok" } else { "failed" }
            )
            .as_bytes(),
        );
        return Ok(is_original(
            &before_line,
            before,
            &current(ctx, &cli).await?,
        ));
    }
    let Some(before) = before else {
        note(
            log,
            b"\nrestore: original version unknown, nothing to restore\n",
        );
        return Ok(false);
    };
    let stale = ctx
        .recipes
        .stale_after_restore(&cli)
        .and_then(|(link, directory)| {
            let file = std::fs::canonicalize(&link).ok()?;
            let directory = std::fs::canonicalize(directory).ok()?;
            file.starts_with(&directory).then_some((link, file))
        });
    let data_root = ctx.data_root.clone();
    for candidate in ctx.recipes.restore(&cli, before) {
        match candidate {
            Restore::Relink { links, dest } => {
                for link in &links {
                    let linked = relink(link, &dest);
                    note(
                        log,
                        format!(
                            "\nrestore: relink {} -> {} ({})\n",
                            link.display(),
                            dest.display(),
                            if linked { "ok" } else { "failed" }
                        )
                        .as_bytes(),
                    );
                }
            }
            Restore::Run(step) => {
                let ran = run_step(&step, ctx.step_limit, log, &mut |group| {
                    record_group(&data_root, pending, group)
                })
                .await?;
                if !ran {
                    log::warn!("CLI 還原指令失敗（{cli}）");
                }
            }
        }
        // 指令回報失敗也驗一次：有些腳本裝好了才在最後一步報錯
        if is_original(&before_line, Some(before), &current(ctx, &cli).await?) {
            if let Some((link, file)) = &stale {
                remove_stale(log, link, file);
            }
            return Ok(true);
        }
    }
    Ok(false)
}

/// 還原後連結已不指向它的新版檔（grok 的下載目錄）：刪掉，免得留著壞掉的新版。
fn remove_stale(log: &mut Option<File>, link: &Path, file: &Path) {
    if std::fs::canonicalize(link).is_ok_and(|now| now == file) {
        return;
    }
    let removed = std::fs::remove_file(file);
    note(
        log,
        format!(
            "restore: removed {} ({})\n",
            file.display(),
            match &removed {
                Ok(()) => "ok".to_owned(),
                Err(error) => error.to_string(),
            }
        )
        .as_bytes(),
    );
}

/// agy：把更新前的副本複製回原路徑（先寫旁邊再 rename，不留半截檔）。
fn copy_back(backup: &Path, dest: &Path) -> bool {
    let temp = dest.with_extension("tt-restore");
    let result = std::fs::copy(backup, &temp).and_then(|_| std::fs::rename(&temp, dest));
    if let Err(error) = &result {
        log::warn!("agy 還原副本失敗：{error}");
        let _ = std::fs::remove_file(&temp);
    }
    result.is_ok()
}

#[cfg(unix)]
fn relink(link: &Path, dest: &Path) -> bool {
    let temp = link.with_extension("tt-restore");
    let _ = std::fs::remove_file(&temp);
    let result =
        std::os::unix::fs::symlink(dest, &temp).and_then(|()| std::fs::rename(&temp, link));
    if let Err(error) = &result {
        log::warn!("連結指回原版本失敗（{}）：{error}", link.display());
        let _ = std::fs::remove_file(&temp);
    }
    result.is_ok()
}

#[cfg(not(unix))]
fn relink(_link: &Path, _dest: &Path) -> bool {
    false
}

/// 這支 CLI 的更新狀態（設定頁用）：程序停不下來＝stuck（請重新啟動 app）；擋板立著＝更新中；
/// 標記是 restore_failed＝無法執行、請重新安裝。
pub(crate) fn state(data_root: &Path, cli: &str) -> Option<&'static str> {
    if gate::is_stuck(cli) {
        return Some("stuck");
    }
    if gate::is_raised(cli) {
        return Some("updating");
    }
    marker::read(data_root, cli)
        .filter(|marker| marker.restore_failed)
        .map(|_| "broken")
}

/// 重新安裝後 `--version` 驗證通過：清掉 restore_failed 的標記。
pub(crate) fn clear_after_reinstall(data_root: &Path, cli: &str) {
    if marker::read(data_root, cli).is_some_and(|marker| marker.restore_failed) {
        marker::clear(data_root, cli);
    }
}

/// 偵測時用：`find_binary` 那支的 `--version` 跑得起來、版號解析得出來＝能用了，清掉 restore_failed。
/// mac 的安裝在 app 外的終端機跑，app 只能從結果判斷。
pub(crate) fn clear_if_runs(data_root: &Path, cli: &str, version_parsed: bool) {
    if version_parsed {
        clear_after_reinstall(data_root, cli);
    }
}

/// 有沒有 restore_failed 的標記（mac 的安裝腳本據此強制重裝，不看「已經裝了」）。
pub(crate) fn needs_reinstall(data_root: &Path, cli: &str) -> bool {
    marker::read(data_root, cli).is_some_and(|marker| marker.restore_failed)
}

const CLIS: [&str; 4] = ["claude", "codex", "agy", "grok"];

/// app 啟動時同步呼叫（排在 `home_cleanup::spawn_startup_sweep` 之前）：看到標記就立起該 CLI 的擋板，
/// 回傳要在背景處理的那幾家；`restore_failed` 的不處理、不立擋板（玩家按重新安裝不能被擋）。
pub(crate) fn raise_pending(data_root: &Path) -> Vec<(marker::Marker, gate::Barrier)> {
    CLIS.into_iter()
        .filter_map(|cli| {
            let pending = marker::read(data_root, cli)?;
            if pending.restore_failed {
                return None;
            }
            Some((pending, gate::raise_now(cli)?))
        })
        .collect()
}

/// 背景處理一份標記（處理期間擋板保持立起，結束時撤掉；程序停不下來就留著）：
/// 先確認上一輪的更新程序那組已經停了，再判定——版本等於原版本＝沒動過；符合驗證規則＝其實更新成功了；
/// 其他照契約還原，失敗記 restore_failed。
pub(crate) async fn recover(ctx: &Ctx, mut pending: marker::Marker, barrier: gate::Barrier) {
    match recover_raised(ctx, &mut pending).await {
        Ok(()) => drop(barrier),
        Err(Unstopped) => {
            log::warn!(
                "CLI 更新復原：上一輪的程序停不下來（{}），擋板留到 app 重新啟動",
                pending.cli
            );
            gate::hold(barrier);
        }
    }
}

async fn recover_raised(ctx: &Ctx, pending: &mut marker::Marker) -> Result<(), Unstopped> {
    if let Some(group) = pending.group {
        if !tree::ensure_gone(&group).await {
            return Err(Unstopped);
        }
    }
    let cli = pending.cli.clone();
    let before = parse_line(&pending.version);
    let now = current(ctx, &cli).await?;
    let updated = now
        .as_ref()
        .is_some_and(|(_, after)| verified(&pending.target, before.as_ref(), after.as_ref()));
    if updated || is_original(&pending.version, before.as_ref(), &now) {
        marker::clear(&ctx.data_root, &cli);
        return Ok(());
    }
    let mut log = open_log(&ctx.data_root, &cli);
    if restore(ctx, &mut log, pending, before.as_ref()).await? {
        marker::clear(&ctx.data_root, &cli);
    } else {
        pending.restore_failed = true;
        if let Err(error) = marker::write(&ctx.data_root, pending) {
            log::warn!("寫 CLI 還原失敗標記失敗（{cli}）：{error}");
        }
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests;
