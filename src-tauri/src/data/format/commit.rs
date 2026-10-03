//! 可恢復的整桌提交：轉換、改用備份、以及照目錄組合表接續或退回。
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::super::paths::{validate_id, world_dir, worlds_dir};
use super::super::scene::TranscriptEvent;
use super::super::state::WorldState;
use super::super::world_file::{self, rename_path, RenameError};
use super::super::world_lock::try_world_exclusive;
use super::super::DataResult;
use super::marker::{self, FormatVersion};
use crate::ui_msg::UiMsg;

/// 需修復的原因；畫面文字由前端 `needsRepair_<reason>` 翻譯。值會送到前端，不得改名。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairReason {
    /// 目錄組合對不上恢復表，無法自動修復；原檔都還在。
    Outside,
    /// 修復時改名失敗（檔案可能被佔用），沒有刪任何東西，下次開啟再試。
    Rename,
    /// 格式轉換沒完成，原桌未改動，下次開啟再試。
    Convert,
    /// 主資料夾不見，只剩備份或未完成的操作。
    Missing,
    /// 修復時讀寫失敗，原桌、備份與另存都還在；`error` 帶系統錯誤原文。
    Io,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum OpenWorld {
    Ready,
    Migrated {
        from: u64,
        to: u64,
    },
    ReadOnly {
        format_version: Option<u64>,
        app_version: Option<String>,
        backup_available: bool,
    },
    NeedsRepair {
        reason: RepairReason,
        /// 只有 `io` 帶，是系統錯誤原文（可能是另一則 TTMSG）。
        error: Option<String>,
        /// 給「打開資料夾」。優先序：主目錄、轉換前備份、暫存、另存、操作日誌、worlds/。
        directory: String,
    },
    Busy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadonlyLine {
    pub speaker_name: String,
    pub text: String,
    pub kind: String,
    /// 事件標頭代碼原樣帶出、不驗；形狀由前端顯示時檢查，畸形或未知就只顯示本文。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marker: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadonlyWorld {
    pub scene: u64,
    pub events: Vec<ReadonlyLine>,
    pub skipped: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpLog {
    op: String,
    stage: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    from: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    to: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    had_pre: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    had_newer: Option<bool>,
}

#[derive(Clone, Copy)]
struct Presence {
    i: bool,
    s: bool,
    p: bool,
    t: bool,
    n: bool,
}

pub(crate) fn live_dir(root: &Path, id: &str) -> PathBuf {
    worlds_dir(root).join(id)
}
fn staging_dir(root: &Path, id: &str) -> PathBuf {
    worlds_dir(root).join(format!(".tt-staging-{id}"))
}
pub(crate) fn pre_dir(root: &Path, id: &str) -> PathBuf {
    worlds_dir(root).join(format!(".tt-pre-{id}"))
}
fn trash_dir(root: &Path, id: &str) -> PathBuf {
    worlds_dir(root).join(format!(".tt-trash-{id}"))
}
pub(crate) fn newer_dir(root: &Path, id: &str) -> PathBuf {
    worlds_dir(root).join(format!(".tt-newer-{id}"))
}
/// 重新重構時重建整桌用的臨時資料根：worlds/.tt-reset-<id>/worlds/<id>。不是側車名稱，桌清單看不到；
/// 重建完才搬成 staging 交換（replace_world_from_build），失敗或殘留都整個刪掉，原桌不受影響。
pub(crate) fn reset_build_root(root: &Path, id: &str) -> PathBuf {
    worlds_dir(root).join(format!(".tt-reset-{id}"))
}

/// 刪掉重設用的臨時根（不存在就當成功）。
pub(crate) fn remove_reset_build_root(root: &Path, id: &str) -> DataResult<()> {
    world_file::remove_path_raw(&reset_build_root(root, id))
}
fn log_path(root: &Path, id: &str) -> PathBuf {
    worlds_dir(root).join(format!(".tt-op-{id}.json"))
}
fn log_tmp_path(root: &Path, id: &str) -> PathBuf {
    worlds_dir(root).join(format!(".tt-op-{id}.json.tmp"))
}

fn presence(root: &Path, id: &str) -> Presence {
    Presence {
        i: live_dir(root, id).exists(),
        s: staging_dir(root, id).exists(),
        p: pre_dir(root, id).exists(),
        t: trash_dir(root, id).exists(),
        n: newer_dir(root, id).exists(),
    }
}

/// 先寫暫存、fsync、再改名蓋過正式日誌。換階段時中斷不會把正式檔截成半份。
/// 讀日誌只看正式檔，暫存檔不當操作紀錄。
fn write_log(root: &Path, id: &str, log: &OpLog) -> DataResult<()> {
    let bytes = serde_json::to_vec_pretty(log)?;
    let tmp = log_tmp_path(root, id);
    world_file::write_bytes_raw(&tmp, &bytes)?;
    world_file::fsync_file(&tmp)?;
    rename_path(&tmp, &log_path(root, id))?;
    world_file::fsync_dir(&worlds_dir(root))?;
    Ok(())
}

fn read_log(path: &Path) -> DataResult<OpLog> {
    parse_log(&fs::read_to_string(path)?)
}

fn parse_log(text: &str) -> DataResult<OpLog> {
    let log: OpLog = serde_json::from_str(text)?;
    match log.op.as_str() {
        "migrate" => {
            if log.had_pre.is_none() || log.from.is_none() || log.to.is_none() {
                return Err(op_log_invalid("migrate log is missing fields"));
            }
        }
        "restore" => {
            if log.had_newer.is_none() {
                return Err(op_log_invalid("restore log is missing fields"));
            }
        }
        // 重新重構的整桌交換：只有 swap／cleanup 兩階段，不帶額外欄位
        "reset" => {
            if log.stage == "build" {
                return Err(op_log_invalid("reset log has no build stage"));
            }
        }
        other => return Err(op_log_invalid(&format!("unknown op {other:?}"))),
    }
    if !matches!(log.stage.as_str(), "build" | "swap" | "cleanup") {
        return Err(op_log_invalid(&format!("unknown stage {:?}", log.stage)));
    }
    Ok(log)
}

fn op_log_invalid(detail: &str) -> Box<dyn std::error::Error + Send + Sync> {
    UiMsg::OpLogInvalid {
        detail: detail.to_owned(),
    }
    .into_error()
}

fn flag(value: Option<bool>) -> bool {
    value.unwrap_or(false)
}

/// h／n：存在與否必須等於旗標。≤flag：旗標為真可有可無，為假必須不在。
fn eq_flag(present: bool, flag: bool) -> bool {
    present == flag
}
fn leq_flag(present: bool, flag: bool) -> bool {
    if flag {
        true
    } else {
        !present
    }
}

#[derive(Clone, Copy)]
enum Action {
    /// 刪 S（若在）、刪日誌。原桌未動。
    DropStagingAndLog,
    DropLog,
    /// T→P、刪 S、刪日誌。
    RollbackAfterR1,
    /// S 驗過則 S→I 並進 cleanup；否則 P→I、（h 真）T→P、刪 S、刪日誌。
    ContinueOrRollbackSwap,
    EnterCleanup,
    FinishCleanup,
    /// T→N、刪日誌。
    RollbackRestore,
    /// P 的格式 ≤ 本版才 P→I 並進 cleanup；否則表外。
    ContinueRestore,
    /// reset：S 驗過則 S→I 並進 cleanup；否則 T→I、刪 S、刪日誌（退回原桌）。
    ContinueReset,
}

fn match_action(log: &OpLog, here: Presence) -> Option<Action> {
    let h = flag(log.had_pre);
    let n = flag(log.had_newer);
    let migrate = log.op == "migrate";
    let restore = log.op == "restore";
    let reset = log.op == "reset";
    let stage = log.stage.as_str();

    // reset（重新重構清回原卡）：S 是已驗過的重建桌，交換＝I→T、S→I，備份 P 不動。
    if reset && stage == "swap" && here.i && here.s && !here.t {
        return Some(Action::DropStagingAndLog);
    }
    if reset && stage == "swap" && !here.i && here.s && here.t {
        return Some(Action::ContinueReset);
    }
    if reset && stage == "swap" && here.i && !here.s && here.t {
        return Some(Action::EnterCleanup);
    }
    if reset && stage == "swap" && here.i && !here.s && !here.t {
        return Some(Action::DropLog);
    }
    if reset && stage == "cleanup" && here.i && !here.s {
        return Some(Action::FinishCleanup);
    }

    if migrate && stage == "build" && here.i && eq_flag(here.p, h) && !here.t {
        return Some(Action::DropStagingAndLog);
    }
    if migrate && stage == "swap" && here.i && here.s && eq_flag(here.p, h) && !here.t {
        return Some(Action::DropStagingAndLog);
    }
    if migrate && stage == "swap" && here.i && !here.s && eq_flag(here.p, h) && !here.t {
        return Some(Action::DropLog);
    }
    if migrate && stage == "swap" && h && here.i && here.s && !here.p && here.t {
        return Some(Action::RollbackAfterR1);
    }
    if migrate && stage == "swap" && !here.i && here.s && here.p && eq_flag(here.t, h) {
        return Some(Action::ContinueOrRollbackSwap);
    }
    if migrate && stage == "swap" && here.i && !here.s && here.p && eq_flag(here.t, h) {
        return Some(Action::EnterCleanup);
    }
    if migrate && stage == "cleanup" && here.i && !here.s && here.p && leq_flag(here.t, h) {
        return Some(Action::FinishCleanup);
    }
    if restore && stage == "swap" && here.i && !here.s && here.p && !here.t && eq_flag(here.n, n) {
        return Some(Action::DropLog);
    }
    if restore && stage == "swap" && n && here.i && !here.s && here.p && here.t && !here.n {
        return Some(Action::RollbackRestore);
    }
    if restore && stage == "swap" && !here.i && !here.s && here.p && eq_flag(here.t, n) && here.n {
        return Some(Action::ContinueRestore);
    }
    if restore && stage == "swap" && here.i && !here.s && !here.p && eq_flag(here.t, n) && here.n {
        return Some(Action::EnterCleanup);
    }
    if restore
        && stage == "cleanup"
        && here.i
        && !here.s
        && !here.p
        && leq_flag(here.t, n)
        && here.n
    {
        return Some(Action::FinishCleanup);
    }
    None
}

fn world_reads_fully(dir: &Path) -> DataResult<()> {
    let state_text = fs::read_to_string(dir.join("state.json"))?;
    let _: WorldState = serde_json::from_str(&state_text)?;
    let worldbook = dir.join("worldbook.json");
    if worldbook.is_file() {
        let value: Value = serde_json::from_str(&fs::read_to_string(&worldbook)?)?;
        if !value.get("entries").is_some_and(Value::is_object) {
            return Err(UiMsg::WorldDataInvalid {
                detail: "worldbook.json entries is not an object".to_owned(),
            }
            .into_error());
        }
    }
    let characters = dir.join("characters");
    if characters.is_dir() {
        for entry in fs::read_dir(&characters)? {
            let entry = entry?;
            if entry.path().extension().is_some_and(|ext| ext == "md") {
                let text = fs::read_to_string(entry.path())?;
                super::super::character::character_file_parses(&text)?;
            }
        }
    }
    let transcript = dir.join("transcript");
    if transcript.is_dir() {
        for entry in fs::read_dir(&transcript)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if !name.ends_with(".jsonl") {
                continue;
            }
            let text = fs::read_to_string(entry.path())?;
            for line in text.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                let _: TranscriptEvent = serde_json::from_str(line)?;
            }
        }
    }
    Ok(())
}

fn staging_verified(dir: &Path, target: u64) -> bool {
    match marker::read_format(dir).version {
        FormatVersion::Known(version) if version == target => world_reads_fully(dir).is_ok(),
        _ => false,
    }
}

enum StepError {
    /// 帶 world_file 的改名錯誤原樣往上傳，畫面上看得到是哪個路徑。
    Rename(Box<dyn std::error::Error + Send + Sync>),
    BackupNewer,
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl From<Box<dyn std::error::Error + Send + Sync>> for StepError {
    fn from(error: Box<dyn std::error::Error + Send + Sync>) -> Self {
        if error.downcast_ref::<RenameError>().is_some() {
            StepError::Rename(error)
        } else {
            StepError::Other(error)
        }
    }
}

fn step_to_data(error: StepError) -> Box<dyn std::error::Error + Send + Sync> {
    match error {
        StepError::Rename(error) | StepError::Other(error) => error,
        StepError::BackupNewer => UiMsg::BackupNewer.into_error(),
    }
}

fn repair_io(error: impl std::fmt::Display) -> Recovered {
    Recovered::Repair(RepairReason::Io, Some(error.to_string()))
}

fn apply_action(root: &Path, id: &str, log: &OpLog) -> Result<(), StepError> {
    let action = match_action(log, presence(root, id))
        .ok_or_else(|| StepError::Other(UiMsg::WorldComboUnexpected.into_error()))?;
    match action {
        Action::DropStagingAndLog => {
            world_file::remove_path_raw(&staging_dir(root, id))?;
            world_file::remove_path_raw(&log_path(root, id))?;
        }
        Action::DropLog => {
            world_file::remove_path_raw(&log_path(root, id))?;
        }
        Action::RollbackAfterR1 => {
            rename_path(&trash_dir(root, id), &pre_dir(root, id))?;
            world_file::remove_path_raw(&staging_dir(root, id))?;
            world_file::remove_path_raw(&log_path(root, id))?;
        }
        Action::ContinueOrRollbackSwap => {
            let target = log.to.unwrap_or(0);
            if staging_verified(&staging_dir(root, id), target) {
                rename_path(&staging_dir(root, id), &live_dir(root, id))?;
                enter_cleanup(root, id, log)?;
            } else {
                rename_path(&pre_dir(root, id), &live_dir(root, id))?;
                if flag(log.had_pre) {
                    rename_path(&trash_dir(root, id), &pre_dir(root, id))?;
                }
                world_file::remove_path_raw(&staging_dir(root, id))?;
                world_file::remove_path_raw(&log_path(root, id))?;
            }
        }
        Action::EnterCleanup => enter_cleanup(root, id, log)?,
        Action::FinishCleanup => finish_cleanup(root, id)?,
        Action::RollbackRestore => {
            rename_path(&trash_dir(root, id), &newer_dir(root, id))?;
            world_file::remove_path_raw(&log_path(root, id))?;
        }
        Action::ContinueReset => {
            let target = marker::current_format();
            if staging_verified(&staging_dir(root, id), target) {
                rename_path(&staging_dir(root, id), &live_dir(root, id))?;
                enter_cleanup(root, id, log)?;
            } else {
                rename_path(&trash_dir(root, id), &live_dir(root, id))?;
                world_file::remove_path_raw(&staging_dir(root, id))?;
                world_file::remove_path_raw(&log_path(root, id))?;
            }
        }
        Action::ContinueRestore => {
            if !marker::version_playable(&pre_dir(root, id)) {
                return Err(StepError::BackupNewer);
            }
            rename_path(&pre_dir(root, id), &live_dir(root, id))?;
            enter_cleanup(root, id, log)?;
        }
    }
    Ok(())
}

fn enter_cleanup(root: &Path, id: &str, log: &OpLog) -> Result<(), StepError> {
    let mut next = log.clone();
    next.stage = "cleanup".to_owned();
    write_log(root, id, &next)?;
    finish_cleanup(root, id)
}

fn finish_cleanup(root: &Path, id: &str) -> Result<(), StepError> {
    world_file::remove_path_raw(&trash_dir(root, id))?;
    world_file::remove_path_raw(&log_path(root, id))?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Recovered {
    Clean,
    Repair(RepairReason, Option<String>),
}

fn recover_locked(root: &Path, id: &str) -> Recovered {
    // 殘留暫存不當日誌。清不掉就停，不接著刪主目錄、備份或另存。
    if let Err(error) = world_file::remove_path_raw(&log_tmp_path(root, id)) {
        return repair_io(error);
    }
    let log_file = log_path(root, id);
    if !log_file.exists() {
        return recover_without_log(root, id);
    }
    if !log_file.is_file() {
        return Recovered::Repair(RepairReason::Outside, None);
    }
    let text = match fs::read_to_string(&log_file) {
        Ok(text) => text,
        Err(error) => return repair_io(error),
    };
    let log = match parse_log(&text) {
        Ok(log) => log,
        Err(_) => return Recovered::Repair(RepairReason::Outside, None),
    };
    if match_action(&log, presence(root, id)).is_none() {
        return Recovered::Repair(RepairReason::Outside, None);
    }
    match apply_action(root, id, &log) {
        Ok(()) => Recovered::Clean,
        Err(StepError::Rename(_)) => Recovered::Repair(RepairReason::Rename, None),
        Err(StepError::BackupNewer) => Recovered::Repair(RepairReason::Outside, None),
        Err(StepError::Other(error)) => repair_io(error),
    }
}

fn recover_without_log(root: &Path, id: &str) -> Recovered {
    let here = presence(root, id);
    if here.t {
        return Recovered::Repair(RepairReason::Outside, None);
    }
    if here.s {
        if here.i || here.p {
            return match world_file::remove_path_raw(&staging_dir(root, id)) {
                Ok(()) => Recovered::Clean,
                Err(error) => repair_io(error),
            };
        }
        return Recovered::Repair(RepairReason::Outside, None);
    }
    if !here.i && (here.p || here.n) {
        return Recovered::Repair(RepairReason::Missing, None);
    }
    Recovered::Clean
}

pub(crate) fn combo_clean(root: &Path, id: &str) -> bool {
    let here = presence(root, id);
    here.i && !here.s && !here.t && !log_path(root, id).exists()
}

fn backup_available(root: &Path, id: &str) -> bool {
    combo_clean(root, id)
        && pre_dir(root, id).is_dir()
        && marker::version_playable(&pre_dir(root, id))
}

fn best_directory(root: &Path, id: &str) -> PathBuf {
    for dir in [
        live_dir(root, id),
        pre_dir(root, id),
        staging_dir(root, id),
        newer_dir(root, id),
    ] {
        if dir.exists() {
            return dir;
        }
    }
    let log = log_path(root, id);
    if log.exists() {
        return log;
    }
    worlds_dir(root)
}

fn repair(root: &Path, id: &str, reason: RepairReason, error: Option<String>) -> OpenWorld {
    OpenWorld::NeedsRepair {
        reason,
        error,
        directory: best_directory(root, id).display().to_string(),
    }
}

fn classify_open(root: &Path, id: &str) -> OpenWorld {
    let dir = live_dir(root, id);
    if !dir.is_dir() {
        return repair(root, id, RepairReason::Missing, None);
    }
    let info = marker::read_format(&dir);
    match info.version {
        FormatVersion::Known(version) if version == marker::current_format() => OpenWorld::Ready,
        FormatVersion::Known(version) if version < marker::current_format() => OpenWorld::Ready,
        FormatVersion::Known(version) => OpenWorld::ReadOnly {
            format_version: Some(version),
            app_version: info.app_version,
            backup_available: backup_available(root, id),
        },
        FormatVersion::Unknown => OpenWorld::ReadOnly {
            format_version: None,
            app_version: info.app_version,
            backup_available: backup_available(root, id),
        },
    }
}

/// 版本比本版舊時 classify 先回 Ready 的佔位，呼叫端改走轉換。
fn needs_migration(root: &Path, id: &str) -> Option<u64> {
    let dir = live_dir(root, id);
    match marker::read_format(&dir).version {
        FormatVersion::Known(version) if version < marker::current_format() => Some(version),
        _ => None,
    }
}

#[cfg(test)]
thread_local! {
    static TEST_STEPS: std::cell::RefCell<Vec<(u64, u64)>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
pub(crate) struct StepOverride {
    previous: Vec<(u64, u64)>,
}

#[cfg(test)]
impl StepOverride {
    pub(crate) fn set(steps: Vec<(u64, u64)>) -> Self {
        let previous = TEST_STEPS.with(|cell| cell.replace(steps));
        Self { previous }
    }
}

#[cfg(test)]
impl Drop for StepOverride {
    fn drop(&mut self) {
        let previous = std::mem::take(&mut self.previous);
        TEST_STEPS.with(|cell| *cell.borrow_mut() = previous);
    }
}

fn apply_migration_step(dir: &Path, from: u64, to: u64) -> DataResult<()> {
    #[cfg(test)]
    {
        let registered = TEST_STEPS.with(|cell| cell.borrow().contains(&(from, to)));
        if registered {
            let path = dir.join("migration-log.txt");
            let mut text = fs::read_to_string(&path).unwrap_or_default();
            text.push_str(&format!("{from}->{to}\n"));
            world_file::write_bytes_raw(&path, text.as_bytes())?;
            return Ok(());
        }
    }
    let _ = (dir, from, to);
    Err(UiMsg::NoMigrationPath { from, to }.into_error())
}

fn migrate_tree(dir: &Path, from: u64, to: u64) -> DataResult<()> {
    let mut version = from;
    while version < to {
        let next = version + 1;
        apply_migration_step(dir, version, next)?;
        version = next;
    }
    Ok(())
}

fn run_migrate(root: &Path, id: &str, from: u64) -> DataResult<()> {
    let to = marker::current_format();
    if !combo_clean(root, id) {
        return Err(UiMsg::WorldComboDirty.into_error());
    }
    let had_pre = pre_dir(root, id).exists();
    write_log(
        root,
        id,
        &OpLog {
            op: "migrate".to_owned(),
            stage: "build".to_owned(),
            from: Some(from),
            to: Some(to),
            had_pre: Some(had_pre),
            had_newer: None,
        },
    )?;
    let staging = staging_dir(root, id);
    if let Err(error) = (|| -> DataResult<()> {
        world_file::copy_dir(&live_dir(root, id), &staging)?;
        migrate_tree(&staging, from, to)?;
        world_reads_fully(&staging)?;
        marker::write_marker(&staging, to)?;
        world_file::fsync_tree(&staging)?;
        Ok(())
    })() {
        let _ = world_file::remove_path_raw(&staging);
        let _ = world_file::remove_path_raw(&log_path(root, id));
        return Err(error);
    }
    let mut log = read_log(&log_path(root, id))?;
    log.stage = "swap".to_owned();
    write_log(root, id, &log)?;
    if had_pre {
        rename_path(&pre_dir(root, id), &trash_dir(root, id))?;
    }
    rename_path(&live_dir(root, id), &pre_dir(root, id))?;
    rename_path(&staging_dir(root, id), &live_dir(root, id))?;
    enter_cleanup(root, id, &read_log(&log_path(root, id))?).map_err(step_to_data)?;
    Ok(())
}

/// 重新重構：用 reset_build_root 裡已完整重建的桌換掉原桌。先驗重建桌讀得完整、搬成 staging，
/// 寫日誌後 I→T、S→I，最後刪 T 與日誌。交換前任何一步失敗都退回原桌、回錯；交換中斷（當機）由
/// 下次開桌照恢復表接續或退回。呼叫端持有這桌的寫入許可。
pub(crate) fn replace_world_from_build(root: &Path, id: &str) -> DataResult<()> {
    if !combo_clean(root, id) {
        return Err(UiMsg::WorldComboDirty.into_error());
    }
    let built = reset_build_root(root, id).join("worlds").join(id);
    world_reads_fully(&built)?;
    world_file::fsync_tree(&built)?;
    let staging = staging_dir(root, id);
    rename_path(&built, &staging)?;
    let log = OpLog {
        op: "reset".to_owned(),
        stage: "swap".to_owned(),
        from: None,
        to: None,
        had_pre: None,
        had_newer: None,
    };
    if let Err(error) = write_log(root, id, &log) {
        let _ = world_file::remove_path_raw(&staging);
        return Err(error);
    }
    if let Err(error) = rename_path(&live_dir(root, id), &trash_dir(root, id)) {
        let _ = world_file::remove_path_raw(&staging);
        let _ = world_file::remove_path_raw(&log_path(root, id));
        return Err(error);
    }
    if let Err(error) = rename_path(&staging, &live_dir(root, id)) {
        // 退回原桌；連退回都失敗就留給下次開桌照日誌接續（S 已驗過，會換成重建桌）
        if rename_path(&trash_dir(root, id), &live_dir(root, id)).is_ok() {
            let _ = world_file::remove_path_raw(&staging);
            let _ = world_file::remove_path_raw(&log_path(root, id));
        }
        return Err(error);
    }
    // 已換成重建桌；清不掉的 T 與日誌留給下次開桌的 cleanup
    let _ = enter_cleanup(root, id, &log);
    Ok(())
}

fn run_restore(root: &Path, id: &str) -> DataResult<()> {
    let had_newer = newer_dir(root, id).exists();
    let log = OpLog {
        op: "restore".to_owned(),
        stage: "swap".to_owned(),
        from: None,
        to: None,
        had_pre: None,
        had_newer: Some(had_newer),
    };
    write_log(root, id, &log)?;
    if had_newer {
        rename_path(&newer_dir(root, id), &trash_dir(root, id))?;
    }
    rename_path(&live_dir(root, id), &newer_dir(root, id))?;
    rename_path(&pre_dir(root, id), &live_dir(root, id))?;
    enter_cleanup(root, id, &read_log(&log_path(root, id))?).map_err(step_to_data)?;
    Ok(())
}

fn finish_after_recover(root: &Path, id: &str, recovered: Recovered) -> DataResult<OpenWorld> {
    if let Recovered::Repair(reason, error) = recovered {
        return Ok(repair(root, id, reason, error));
    }
    if !live_dir(root, id).is_dir() {
        if pre_dir(root, id).exists() || log_path(root, id).exists() {
            return Ok(repair(root, id, RepairReason::Missing, None));
        }
        return Err(UiMsg::WorldNotFound.into_error());
    }
    if let Some(from) = needs_migration(root, id) {
        if !combo_clean(root, id) {
            return Ok(repair(root, id, RepairReason::Outside, None));
        }
        return migrate_then_classify(root, id, from);
    }
    Ok(classify_open(root, id))
}

fn migrate_then_classify(root: &Path, id: &str, from: u64) -> DataResult<OpenWorld> {
    let to = marker::current_format();
    match run_migrate(root, id, from) {
        Ok(()) => Ok(OpenWorld::Migrated { from, to }),
        Err(_) => match recover_locked(root, id) {
            Recovered::Repair(reason, error) => Ok(repair(root, id, reason, error)),
            Recovered::Clean => {
                if live_dir(root, id).is_dir()
                    && matches!(
                        marker::read_format(&live_dir(root, id)).version,
                        FormatVersion::Known(version) if version == to
                    )
                {
                    Ok(OpenWorld::Migrated { from, to })
                } else if combo_clean(root, id) {
                    Ok(repair(root, id, RepairReason::Convert, None))
                } else {
                    Ok(repair(root, id, RepairReason::Outside, None))
                }
            }
        },
    }
}

pub fn open_world(root: &Path, world_id: &str) -> DataResult<OpenWorld> {
    validate_id(world_id)?;
    let Some(_lock) = try_world_exclusive(world_id) else {
        return Ok(OpenWorld::Busy);
    };
    // 重設交換已提交、但當時臨時根沒清掉：持獨占時順手清，清不掉下次再試
    let _ = remove_reset_build_root(root, world_id);
    let recovered = recover_locked(root, world_id);
    finish_after_recover(root, world_id, recovered)
}

pub fn restore_world_backup(root: &Path, world_id: &str) -> DataResult<OpenWorld> {
    validate_id(world_id)?;
    let Some(_lock) = try_world_exclusive(world_id) else {
        return Ok(OpenWorld::Busy);
    };
    let recovered = recover_locked(root, world_id);
    if let Recovered::Repair(reason, error) = recovered {
        return Ok(repair(root, world_id, reason, error));
    }
    let opened = classify_open(root, world_id);
    let read_only = matches!(opened, OpenWorld::ReadOnly { .. });
    if !read_only {
        return Err(UiMsg::WorldNotReadOnly.into_error());
    }
    if !backup_available(root, world_id) {
        return Err(UiMsg::NoPreMigrationBackup.into_error());
    }
    if let Err(error) = run_restore(root, world_id) {
        return match recover_locked(root, world_id) {
            Recovered::Repair(reason, error) => Ok(repair(root, world_id, reason, error)),
            Recovered::Clean => Err(error),
        };
    }
    // 還原後的桌可能比本版舊，再走一次格式判讀（含轉換）。
    finish_after_recover(root, world_id, Recovered::Clean)
}

pub fn read_world_readonly(root: &Path, world_id: &str) -> DataResult<ReadonlyWorld> {
    validate_id(world_id)?;
    let dir = world_dir(root, world_id)?;
    let scene = loose_scene(&dir).or_else(|| max_transcript_scene(&dir));
    let Some(scene) = scene else {
        return Ok(ReadonlyWorld {
            scene: 0,
            events: Vec::new(),
            skipped: 0,
        });
    };
    let path = dir.join("transcript").join(format!("{scene}.jsonl"));
    if !path.is_file() {
        if let Some(fallback) = max_transcript_scene(&dir) {
            if fallback != scene {
                return read_scene_file(&dir, fallback);
            }
        }
        return Ok(ReadonlyWorld {
            scene,
            events: Vec::new(),
            skipped: 0,
        });
    }
    read_scene_file(&dir, scene)
}

fn read_scene_file(dir: &Path, scene: u64) -> DataResult<ReadonlyWorld> {
    let path = dir.join("transcript").join(format!("{scene}.jsonl"));
    let text = fs::read_to_string(&path).unwrap_or_default();
    let mut events = Vec::new();
    let mut skipped = 0u64;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(line) {
            Ok(value) => {
                let speaker = value.get("speaker_name").and_then(Value::as_str);
                let text = value.get("text").and_then(Value::as_str);
                let kind = value.get("kind").and_then(Value::as_str);
                if let (Some(speaker_name), Some(text), Some(kind)) = (speaker, text, kind) {
                    events.push(ReadonlyLine {
                        speaker_name: speaker_name.to_owned(),
                        text: text.to_owned(),
                        kind: kind.to_owned(),
                        marker: value
                            .get("marker")
                            .filter(|marker| !marker.is_null())
                            .cloned(),
                    });
                } else {
                    skipped += 1;
                }
            }
            Err(_) => skipped += 1,
        }
    }
    Ok(ReadonlyWorld {
        scene,
        events,
        skipped,
    })
}

fn loose_scene(dir: &Path) -> Option<u64> {
    let text = fs::read_to_string(dir.join("state.json")).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    value.get("current_scene").and_then(Value::as_u64)
}

fn max_transcript_scene(dir: &Path) -> Option<u64> {
    let mut best = None;
    let entries = fs::read_dir(dir.join("transcript")).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(stem) = name.strip_suffix(".jsonl") else {
            continue;
        };
        if let Ok(number) = stem.parse::<u64>() {
            best = Some(best.map_or(number, |current: u64| current.max(number)));
        }
    }
    best
}

#[derive(Debug, Clone)]
pub struct ListedWorld {
    pub id: String,
    pub name_dir: Option<PathBuf>,
    pub sort_dir: Option<PathBuf>,
    pub read_only: bool,
    pub needs_repair: bool,
}

pub fn discover_ids(root: &Path) -> DataResult<Vec<String>> {
    let directory = worlds_dir(root);
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut ids = std::collections::BTreeSet::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if validate_id(name).is_ok() && entry.file_type()?.is_dir() {
            ids.insert(name.to_owned());
            continue;
        }
        if let Some(id) = sidecar_id(name) {
            ids.insert(id);
        }
    }
    Ok(ids.into_iter().collect())
}

fn sidecar_id(name: &str) -> Option<String> {
    for prefix in [".tt-staging-", ".tt-pre-", ".tt-trash-", ".tt-newer-"] {
        if let Some(rest) = name.strip_prefix(prefix) {
            if validate_id(rest).is_ok() {
                return Some(rest.to_owned());
            }
        }
    }
    let rest = name.strip_prefix(".tt-op-")?.strip_suffix(".json")?;
    validate_id(rest).ok()?;
    Some(rest.to_owned())
}

pub fn recover_for_list(root: &Path, id: &str) -> DataResult<ListedWorld> {
    let mut forced_repair = false;
    if let Some(_guard) = try_world_exclusive(id) {
        if let Recovered::Repair(..) = recover_locked(root, id) {
            forced_repair = true;
        }
    }
    let mut listed = assess(root, id);
    if forced_repair {
        listed.needs_repair = true;
    }
    Ok(listed)
}

fn assess(root: &Path, id: &str) -> ListedWorld {
    let here = presence(root, id);
    let log_bad = log_path(root, id).exists()
        && read_log(&log_path(root, id))
            .ok()
            .and_then(|log| match_action(&log, here).map(|_| log))
            .is_none();
    let outside = if log_path(root, id).exists() {
        log_bad
    } else if here.t || (here.s && !(here.i || here.p)) || (!here.i && (here.p || here.n)) {
        true
    } else {
        false
    };
    let name_dir = [
        live_dir(root, id),
        pre_dir(root, id),
        staging_dir(root, id),
        newer_dir(root, id),
    ]
    .into_iter()
    .find(|dir| dir.is_dir());
    let read_only = live_dir(root, id).is_dir()
        && match marker::read_format(&live_dir(root, id)).version {
            FormatVersion::Known(version) => version > marker::current_format(),
            FormatVersion::Unknown => true,
        };
    ListedWorld {
        id: id.to_owned(),
        sort_dir: name_dir.clone(),
        name_dir,
        read_only,
        needs_repair: outside || !here.i,
    }
}
