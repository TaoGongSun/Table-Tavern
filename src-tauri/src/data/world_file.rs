//! 桌目錄的唯一寫入口。資料層與 command 層要改桌裡的檔，都走這裡；
//! 寫入當下再查一次格式標記。掃描測試會擋掉這支以外的直接寫檔。
#[cfg(test)]
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fs::File;
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use super::format::marker::{self, FormatVersion};
use super::paths::{validate_id, world_dir, worlds_dir};
use super::DataResult;
use crate::ui_msg::UiMsg;

#[derive(Debug)]
pub(crate) struct RenameError {
    message: String,
}

impl std::fmt::Display for RenameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RenameError {}

fn rename_error(message: String) -> DataResult<()> {
    Err(Box::new(RenameError { message }))
}

#[cfg(test)]
thread_local! {
    static RENAME_SKIPS: Cell<u32> = const { Cell::new(0) };
    static RENAME_FAIL_MATCH: RefCell<Option<String>> = const { RefCell::new(None) };
    static RENAME_FAILS: Cell<u32> = const { Cell::new(0) };
    static RENAME_ATTEMPT_IO_FAILS: Cell<u32> = const { Cell::new(0) };
    static RENAME_ATTEMPT_COUNT: Cell<u32> = const { Cell::new(0) };
    static RENAME_WAIT_COUNT: Cell<u32> = const { Cell::new(0) };
    static REMOVE_FAILS: Cell<u32> = const { Cell::new(0) };
    static APPEND_PARTIAL_FAILS: Cell<u32> = const { Cell::new(0) };
    static WRITE_PARTIAL_FAILS: Cell<u32> = const { Cell::new(0) };
    static TRUNCATE_FAILS: Cell<u32> = const { Cell::new(0) };
    static TRUNCATE_CUT_THEN_FAIL: Cell<bool> = const { Cell::new(false) };
    static LEN_FAILS: Cell<u32> = const { Cell::new(0) };
    static REMOVE_FAIL_MATCH: RefCell<Option<String>> = const { RefCell::new(None) };
    static WRITE_FAIL_MATCH: RefCell<Option<String>> = const { RefCell::new(None) };
    static WRITE_PARTIAL_SKIP: Cell<u32> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) struct RenameFailGuard;

#[cfg(test)]
impl RenameFailGuard {
    pub(crate) fn fail(times: u32) -> Self {
        Self::fail_after(0, times)
    }

    /// 先放行 `skip` 次改名，再讓接下來 `times` 次失敗：用來打在交換流程的指定階段。
    pub(crate) fn fail_after(skip: u32, times: u32) -> Self {
        RENAME_SKIPS.with(|cell| cell.set(skip));
        RENAME_FAILS.with(|cell| cell.set(times));
        Self
    }

    /// 只讓目標路徑以 `suffix` 結尾的改名失敗：打在流程裡某個特定檔的替換，不受途中其他改名的次數影響。
    pub(crate) fn fail_ending(suffix: &str, times: u32) -> Self {
        RENAME_FAIL_MATCH.with(|cell| *cell.borrow_mut() = Some(suffix.to_owned()));
        Self::fail(times)
    }
}

#[cfg(test)]
impl Drop for RenameFailGuard {
    fn drop(&mut self) {
        RENAME_SKIPS.with(|cell| cell.set(0));
        RENAME_FAILS.with(|cell| cell.set(0));
        RENAME_FAIL_MATCH.with(|cell| *cell.borrow_mut() = None);
    }
}

/// 下 `times` 次改名「嘗試」回 IO 錯，並把嘗試數與等待數歸零計起：測 `rename_path` 的重試本身。
#[cfg(test)]
pub(crate) struct RenameAttemptGuard;

#[cfg(test)]
impl RenameAttemptGuard {
    pub(crate) fn fail_io(times: u32) -> Self {
        RENAME_ATTEMPT_IO_FAILS.with(|cell| cell.set(times));
        RENAME_ATTEMPT_COUNT.with(|cell| cell.set(0));
        RENAME_WAIT_COUNT.with(|cell| cell.set(0));
        Self
    }

    /// （嘗試數, 等待數）
    pub(crate) fn counts(&self) -> (u32, u32) {
        (
            RENAME_ATTEMPT_COUNT.with(Cell::get),
            RENAME_WAIT_COUNT.with(Cell::get),
        )
    }
}

#[cfg(test)]
impl Drop for RenameAttemptGuard {
    fn drop(&mut self) {
        RENAME_ATTEMPT_IO_FAILS.with(|cell| cell.set(0));
    }
}

/// 下幾次「目標存在」的刪除直接失敗，用來測恢復途中的 IO 錯。
#[cfg(test)]
pub(crate) struct RemoveFailGuard;

#[cfg(test)]
impl RemoveFailGuard {
    pub(crate) fn fail(times: u32) -> Self {
        REMOVE_FAILS.with(|cell| cell.set(times));
        Self
    }

    /// 只讓路徑以 `suffix` 結尾的刪除失敗：打在流程裡某個特定檔或目錄，不受途中其他刪除的次數影響。
    pub(crate) fn fail_ending(suffix: &str, times: u32) -> Self {
        REMOVE_FAIL_MATCH.with(|cell| *cell.borrow_mut() = Some(suffix.to_owned()));
        Self::fail(times)
    }
}

#[cfg(test)]
impl Drop for RemoveFailGuard {
    fn drop(&mut self) {
        REMOVE_FAILS.with(|cell| cell.set(0));
        REMOVE_FAIL_MATCH.with(|cell| *cell.borrow_mut() = None);
    }
}

/// 下幾次追加只寫進前半段就失敗，用來測「部分追加後失敗」的回復。
#[cfg(test)]
pub(crate) struct AppendFailGuard;

#[cfg(test)]
impl AppendFailGuard {
    pub(crate) fn partial(times: u32) -> Self {
        APPEND_PARTIAL_FAILS.with(|cell| cell.set(times));
        Self
    }
}

#[cfg(test)]
impl Drop for AppendFailGuard {
    fn drop(&mut self) {
        APPEND_PARTIAL_FAILS.with(|cell| cell.set(0));
    }
}

/// 下幾次整檔寫入只寫進前半段就失敗（檔案停在半截），用來測覆寫途中失敗。
#[cfg(test)]
pub(crate) struct WriteFailGuard;

#[cfg(test)]
impl WriteFailGuard {
    pub(crate) fn partial(times: u32) -> Self {
        WRITE_PARTIAL_FAILS.with(|cell| cell.set(times));
        Self
    }

    /// 只讓路徑以 `suffix` 結尾的整檔寫入停在半截（例如 `state.json.tmp`）：打在流程裡某支特定檔。
    pub(crate) fn partial_ending(suffix: &str, times: u32) -> Self {
        WRITE_FAIL_MATCH.with(|cell| *cell.borrow_mut() = Some(suffix.to_owned()));
        Self::partial(times)
    }

    /// 同 partial_ending，但先放行 skip 次符合的寫入（打在流程中第 N 支同類檔）。
    pub(crate) fn partial_ending_after(suffix: &str, skip: u32, times: u32) -> Self {
        WRITE_PARTIAL_SKIP.with(|cell| cell.set(skip));
        Self::partial_ending(suffix, times)
    }
}

#[cfg(test)]
impl Drop for WriteFailGuard {
    fn drop(&mut self) {
        WRITE_PARTIAL_FAILS.with(|cell| cell.set(0));
        WRITE_FAIL_MATCH.with(|cell| *cell.borrow_mut() = None);
        WRITE_PARTIAL_SKIP.with(|cell| cell.set(0));
    }
}

fn injected_partial_write(path: &Path) -> bool {
    #[cfg(test)]
    {
        let targeted = WRITE_FAIL_MATCH.with(|cell| {
            cell.borrow()
                .as_ref()
                .is_none_or(|suffix| path.to_string_lossy().ends_with(suffix.as_str()))
        });
        if !targeted {
            return false;
        }
        if WRITE_PARTIAL_FAILS.with(Cell::get) > 0 {
            let skip = WRITE_PARTIAL_SKIP.with(Cell::get);
            if skip > 0 {
                WRITE_PARTIAL_SKIP.with(|cell| cell.set(skip - 1));
                return false;
            }
        }
        WRITE_PARTIAL_FAILS.with(|cell| {
            let left = cell.get();
            if left == 0 {
                return false;
            }
            cell.set(left - 1);
            true
        })
    }
    #[cfg(not(test))]
    {
        let _ = path;
        false
    }
}

/// 下幾次截檔回錯：預設檔案不動；`cut_then_fail` 是真的截了才回錯（平台不保證失敗不變的那種）。
/// `len_fails` 讓之後量長度也失敗。測追加失敗截不回、收回時截檔結果的判定。
#[cfg(test)]
pub(crate) struct TruncateFailGuard;

#[cfg(test)]
impl TruncateFailGuard {
    pub(crate) fn fail(times: u32) -> Self {
        TRUNCATE_FAILS.with(|cell| cell.set(times));
        Self
    }

    pub(crate) fn cut_then_fail(times: u32) -> Self {
        TRUNCATE_CUT_THEN_FAIL.with(|cell| cell.set(true));
        Self::fail(times)
    }

    pub(crate) fn len_fails(self, times: u32) -> Self {
        LEN_FAILS.with(|cell| cell.set(times));
        self
    }
}

#[cfg(test)]
impl Drop for TruncateFailGuard {
    fn drop(&mut self) {
        TRUNCATE_FAILS.with(|cell| cell.set(0));
        TRUNCATE_CUT_THEN_FAIL.with(|cell| cell.set(false));
        LEN_FAILS.with(|cell| cell.set(0));
    }
}

fn injected_len_failure() -> bool {
    #[cfg(test)]
    {
        LEN_FAILS.with(|cell| {
            let left = cell.get();
            if left == 0 {
                return false;
            }
            cell.set(left - 1);
            true
        })
    }
    #[cfg(not(test))]
    {
        false
    }
}

fn injected_cut_then_fail() -> bool {
    #[cfg(test)]
    {
        TRUNCATE_CUT_THEN_FAIL.with(Cell::get)
    }
    #[cfg(not(test))]
    {
        false
    }
}

fn injected_truncate_failure() -> bool {
    #[cfg(test)]
    {
        TRUNCATE_FAILS.with(|cell| {
            let left = cell.get();
            if left == 0 {
                return false;
            }
            cell.set(left - 1);
            true
        })
    }
    #[cfg(not(test))]
    {
        false
    }
}

fn injected_partial_append() -> bool {
    #[cfg(test)]
    {
        APPEND_PARTIAL_FAILS.with(|cell| {
            let left = cell.get();
            if left == 0 {
                return false;
            }
            cell.set(left - 1);
            true
        })
    }
    #[cfg(not(test))]
    {
        false
    }
}

/// 測試用寫入控制點：指定路徑的每次寫入／追加在動檔前呼叫，測試可以在資料寫入點停住一方，
/// 驗另一方的寫入進不來。全域的，用到它的測試要彼此排隊。
#[cfg(test)]
pub(crate) mod write_hook {
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    type Hook = Arc<dyn Fn() + Send + Sync>;

    static HOOK: Mutex<Option<(PathBuf, Hook)>> = Mutex::new(None);

    pub(crate) struct HookGuard;

    impl Drop for HookGuard {
        fn drop(&mut self) {
            *HOOK.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
        }
    }

    pub(crate) fn install(path: PathBuf, hook: impl Fn() + Send + Sync + 'static) -> HookGuard {
        *HOOK.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) =
            Some((path, Arc::new(hook)));
        HookGuard
    }

    pub(super) fn fire(path: &Path) {
        let hook = HOOK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .filter(|(target, _)| target == path)
            .map(|(_, hook)| hook.clone());
        if let Some(hook) = hook {
            hook();
        }
    }
}

fn injected_remove_failure(path: &Path) -> bool {
    #[cfg(test)]
    {
        let targeted = REMOVE_FAIL_MATCH.with(|cell| {
            cell.borrow()
                .as_ref()
                .is_none_or(|suffix| path.to_string_lossy().ends_with(suffix.as_str()))
        });
        if !targeted {
            return false;
        }
        REMOVE_FAILS.with(|cell| {
            let left = cell.get();
            if left == 0 {
                return false;
            }
            cell.set(left - 1);
            true
        })
    }
    #[cfg(not(test))]
    {
        let _ = path;
        false
    }
}

fn injected_rename_failure(to: &Path) -> bool {
    #[cfg(test)]
    {
        let targeted = RENAME_FAIL_MATCH.with(|cell| {
            cell.borrow()
                .as_ref()
                .is_none_or(|suffix| to.to_string_lossy().ends_with(suffix.as_str()))
        });
        if !targeted {
            return false;
        }
        let skipped = RENAME_SKIPS.with(|cell| {
            let left = cell.get();
            if left == 0 {
                return false;
            }
            cell.set(left - 1);
            true
        });
        if skipped {
            return false;
        }
        RENAME_FAILS.with(|cell| {
            let left = cell.get();
            if left == 0 {
                return false;
            }
            cell.set(left - 1);
            true
        })
    }
    #[cfg(not(test))]
    {
        let _ = to;
        false
    }
}

/// Windows 上改名常被防毒或索引暫時佔用，失敗就短暫重試。其他平台只試一次。
pub(crate) fn rename_path(from: &Path, to: &Path) -> DataResult<()> {
    if injected_rename_failure(to) {
        return rename_error(rename_failed(from, to));
    }
    let attempts = RENAME_ATTEMPTS;
    let mut wait = Duration::from_millis(20);
    for attempt in 1..=attempts {
        match attempt_rename(from, to) {
            Ok(()) => return Ok(()),
            Err(_) if attempt < attempts => {
                note_rename_wait();
                std::thread::sleep(wait);
                wait = (wait * 2).min(Duration::from_millis(200));
            }
            Err(error) => {
                return rename_error(
                    UiMsg::RenameFailedIo {
                        from: from.display().to_string(),
                        to: to.display().to_string(),
                        error: error.to_string(),
                    }
                    .to_string(),
                );
            }
        }
    }
    rename_error(rename_failed(from, to))
}

const RENAME_ATTEMPTS: u32 = if cfg!(windows) { 8 } else { 1 };

/// 每次真的嘗試改名都經過這裡；測試在這裡計數並模擬 IO 錯。
fn attempt_rename(from: &Path, to: &Path) -> std::io::Result<()> {
    #[cfg(test)]
    {
        RENAME_ATTEMPT_COUNT.with(|cell| cell.set(cell.get() + 1));
        let fail = RENAME_ATTEMPT_IO_FAILS.with(|cell| {
            let left = cell.get();
            if left == 0 {
                return false;
            }
            cell.set(left - 1);
            true
        });
        if fail {
            return Err(std::io::Error::other("injected rename io error"));
        }
    }
    fs::rename(from, to)
}

fn note_rename_wait() {
    #[cfg(test)]
    RENAME_WAIT_COUNT.with(|cell| cell.set(cell.get() + 1));
}

fn rename_failed(from: &Path, to: &Path) -> String {
    UiMsg::RenameFailed {
        from: from.display().to_string(),
        to: to.display().to_string(),
    }
    .to_string()
}

pub(crate) fn write_bytes_raw(path: &Path, bytes: &[u8]) -> DataResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if injected_partial_write(path) {
        fs::write(path, &bytes[..bytes.len() / 2])?;
        return Err(std::io::Error::other("injected partial write").into());
    }
    fs::write(path, bytes)?;
    Ok(())
}

pub(crate) fn fsync_file(path: &Path) -> DataResult<()> {
    let file = OpenOptions::new().write(true).open(path)?;
    file.sync_all()?;
    Ok(())
}

/// 目錄 fsync：Unix 開目錄 handle 後 sync。Windows 開目錄會失敗，這裡略過。
pub(crate) fn fsync_dir(path: &Path) -> DataResult<()> {
    #[cfg(unix)]
    {
        let file = File::open(path)?;
        file.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

pub(crate) fn fsync_tree(dir: &Path) -> DataResult<()> {
    let entries = fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            fsync_tree(&path)?;
        } else if path.is_file() {
            fsync_file(&path)?;
        }
    }
    fsync_dir(dir)
}

pub(crate) fn remove_path_raw(path: &Path) -> DataResult<()> {
    if !path.exists() {
        return Ok(());
    }
    if injected_remove_failure(path) {
        return Err(UiMsg::RemoveFailed {
            path: path.display().to_string(),
        }
        .into_error());
    }
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub(crate) fn copy_dir(from: &Path, to: &Path) -> DataResult<()> {
    if to.exists() {
        return Err(UiMsg::TargetExists {
            path: to.display().to_string(),
        }
        .into_error());
    }
    fs::create_dir_all(to)?;
    copy_children(from, to)?;
    fsync_tree(to)?;
    Ok(())
}

fn copy_children(from: &Path, to: &Path) -> DataResult<()> {
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let source = entry.path();
        let dest = to.join(entry.file_name());
        if source.is_dir() {
            fs::create_dir(&dest)?;
            copy_children(&source, &dest)?;
        } else {
            fs::copy(&source, &dest)?;
        }
    }
    Ok(())
}

/// 從路徑往上找到 `worlds/<合法 id>`。sidecar（`.tt-*`）不是合法 id，不會被認成桌。
pub(crate) fn locate_world(path: &Path) -> DataResult<(PathBuf, String)> {
    let mut current = Some(path);
    while let Some(node) = current {
        if let Some(parent) = node.parent() {
            if parent.file_name().is_some_and(|name| name == "worlds") {
                if let Some(id) = node.file_name().and_then(|name| name.to_str()) {
                    if validate_id(id).is_ok() {
                        let root = parent.parent().ok_or_else(|| {
                            UiMsg::DataRootNotFound {
                                path: path.display().to_string(),
                            }
                            .into_error()
                        })?;
                        return Ok((root.to_path_buf(), id.to_owned()));
                    }
                }
            }
        }
        current = node.parent();
    }
    Err(UiMsg::PathOutsideWorld {
        path: path.display().to_string(),
    }
    .into_error())
}

fn combo_blocks_write(root: &Path, world_id: &str) -> bool {
    let worlds = worlds_dir(root);
    let log = worlds.join(format!(".tt-op-{world_id}.json"));
    let staging = worlds.join(format!(".tt-staging-{world_id}"));
    let trash = worlds.join(format!(".tt-trash-{world_id}"));
    log.exists() || staging.exists() || trash.exists()
}

pub(crate) fn ensure_writable(root: &Path, world_id: &str) -> DataResult<()> {
    let dir = world_dir(root, world_id)?;
    if !dir.is_dir() {
        return Err(UiMsg::WorldNotFound.into_error());
    }
    if combo_blocks_write(root, world_id) {
        return Err(UiMsg::WorldConverting.into_error());
    }
    match marker::read_format(&dir).version {
        FormatVersion::Known(version) if version == marker::current_format() => Ok(()),
        _ => Err(UiMsg::WorldReadOnly.into_error()),
    }
}

// 同檔鎖：同一行程內，同一支桌檔的寫入（含讀改寫、追加失敗截回）整段排隊，讀者也在鎖內一次讀完。
// 鍵是呼叫端組出來的路徑（固定 root＋受控名稱），不做 canonicalize；跨行程與 symlink 別名不保證共鎖。
// 鎖順序：先桌的 world_write_permit，再這把；只在同步程式內短暫持有，不跨 await、不巢狀拿第二把。
fn file_lock(path: &Path) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
    let mut table = LOCKS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    table.entry(path.to_path_buf()).or_default().clone()
}

/// 拿著某支檔的同檔鎖做事。鎖內只能用 `LockedFile` 的方法，不可再呼叫會取鎖的
/// `commit_world_*`／`read_transcript`，否則自鎖。
pub(crate) fn with_file_lock<T>(path: &Path, work: impl FnOnce(&LockedFile<'_>) -> T) -> T {
    let lock = file_lock(path);
    let _held = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    work(&LockedFile { path, root: false })
}

/// 資料根目錄下的檔（不在 `worlds/` 裡，例如跨桌的卡片變數檔）：同樣同檔鎖，寫入前查更新閘門
/// （沒有桌的格式標記可查）。路徑由呼叫端用受控名稱組成，不收外部字串。
pub(crate) fn with_root_file_lock<T>(path: &Path, work: impl FnOnce(&LockedFile<'_>) -> T) -> T {
    let lock = file_lock(path);
    let _held = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    work(&LockedFile { path, root: true })
}

/// 持有同檔鎖期間對該檔的操作。
pub(crate) struct LockedFile<'a> {
    path: &'a Path,
    /// 資料根目錄下的檔：不查桌的格式標記，改查更新閘門
    root: bool,
}

impl LockedFile<'_> {
    fn writable(&self) -> DataResult<()> {
        if self.root {
            super::world_lock::refuse_if_updating()?;
        } else {
            let (root, id) = locate_world(self.path)?;
            ensure_writable(&root, &id)?;
        }
        #[cfg(test)]
        write_hook::fire(self.path);
        Ok(())
    }

    /// 整檔讀；檔案不存在回 None。
    pub(crate) fn read(&self) -> DataResult<Option<Vec<u8>>> {
        match fs::read(self.path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub(crate) fn len(&self) -> DataResult<u64> {
        if injected_len_failure() {
            return Err(std::io::Error::other("injected len failure").into());
        }
        Ok(fs::metadata(self.path)?.len())
    }

    pub(crate) fn write(&self, bytes: &[u8]) -> DataResult<()> {
        self.writable()?;
        write_bytes_raw(self.path, bytes)
    }

    /// 整檔原子寫：同目錄暫存檔寫好、fsync 後改名蓋上。中途失敗原檔不動。
    pub(crate) fn write_atomic(&self, bytes: &[u8]) -> DataResult<()> {
        self.writable()?;
        let mut name = self.path.file_name().unwrap_or_default().to_os_string();
        name.push(".tmp");
        let temp = self.path.with_file_name(name);
        let committed = write_bytes_raw(&temp, bytes)
            .and_then(|()| fsync_file(&temp))
            .and_then(|()| rename_path(&temp, self.path));
        if committed.is_err() {
            let _ = remove_path_raw(&temp);
        }
        committed
    }

    pub(crate) fn remove(&self) -> DataResult<()> {
        self.writable()?;
        remove_path_raw(self.path)
    }

    pub(crate) fn truncate(&self, len: u64) -> DataResult<()> {
        self.writable()?;
        truncate_raw(self.path, len)
    }

    /// 追加一段（呼叫端給完整的行，含結尾換行），回傳這段的起始位元組（追加收據）。
    /// 前一行缺換行（上次追加失敗又截不回）先補一個，免得兩筆黏成一行；
    /// 寫入失敗就盡力截回原長度，截不回就放著，回原本的錯。
    pub(crate) fn append(&self, bytes: &[u8]) -> DataResult<u64> {
        self.writable()?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let original = match fs::metadata(self.path) {
            Ok(meta) => meta.len(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
            Err(error) => return Err(error.into()),
        };
        let needs_newline = original > 0 && {
            let mut file = File::open(self.path)?;
            file.seek(SeekFrom::Start(original - 1))?;
            let mut last = [0u8; 1];
            file.read_exact(&mut last)?;
            last[0] != b'\n'
        };
        let mut payload = Vec::with_capacity(bytes.len() + 1);
        if needs_newline {
            payload.push(b'\n');
        }
        payload.extend_from_slice(bytes);
        let written = (|| -> DataResult<()> {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.path)?;
            if injected_partial_append() {
                file.write_all(&payload[..payload.len() / 2])?;
                return Err(std::io::Error::other("injected partial append").into());
            }
            file.write_all(&payload)?;
            Ok(())
        })();
        match written {
            Ok(()) => Ok(original + u64::from(needs_newline)),
            Err(error) => {
                let _ = truncate_raw(self.path, original);
                Err(error)
            }
        }
    }
}

fn truncate_raw(path: &Path, len: u64) -> DataResult<()> {
    if injected_truncate_failure() {
        if injected_cut_then_fail() {
            OpenOptions::new().write(true).open(path)?.set_len(len)?;
        }
        return Err(std::io::Error::other("injected truncate failure").into());
    }
    OpenOptions::new().write(true).open(path)?.set_len(len)?;
    Ok(())
}

pub(crate) fn commit_world_write(path: &Path, bytes: &[u8]) -> DataResult<()> {
    with_file_lock(path, |file| file.write(bytes))
}

pub(crate) fn commit_world_write_atomic(path: &Path, bytes: &[u8]) -> DataResult<()> {
    with_file_lock(path, |file| file.write_atomic(bytes))
}

/// 追加並回傳收據（這段的起始位元組）。
pub(crate) fn commit_world_append(path: &Path, bytes: &[u8]) -> DataResult<u64> {
    with_file_lock(path, |file| file.append(bytes))
}

pub(crate) fn commit_world_remove(path: &Path) -> DataResult<()> {
    with_file_lock(path, |file| file.remove())
}

/// 建桌：先落格式標記，後面的 state／world.md 才過得了閘門。
pub(crate) fn prepare_new_world(root: &Path, id: &str) -> DataResult<PathBuf> {
    validate_id(id)?;
    let worlds = worlds_dir(root);
    fs::create_dir_all(&worlds)?;
    let dir = worlds.join(id);
    fs::create_dir(&dir)?;
    fs::create_dir(dir.join("characters"))?;
    fs::create_dir(dir.join("transcript"))?;
    marker::write_marker(&dir, marker::current_format())?;
    Ok(dir)
}

/// 玩家確認刪桌：主目錄與這桌的 sidecar 一起清掉，避免留下「只有備份」的修復列。
pub(crate) fn delete_world_tree(root: &Path, world_id: &str) -> DataResult<()> {
    let dir = world_dir(root, world_id)?;
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    let worlds = worlds_dir(root);
    for name in [
        format!(".tt-staging-{world_id}"),
        format!(".tt-pre-{world_id}"),
        format!(".tt-trash-{world_id}"),
        format!(".tt-newer-{world_id}"),
        format!(".tt-op-{world_id}.json"),
        format!(".tt-op-{world_id}.json.tmp"),
        format!(".tt-reset-{world_id}"),
    ] {
        remove_path_raw(&worlds.join(name))?;
    }
    Ok(())
}

#[cfg(test)]
mod scan_tests {
    /// 資料層與 command 層不得在共用寫入函式之外直接寫檔。
    /// 豁免註解必須緊貼上一行，而且理由要寫明不是桌目錄。
    const NEEDLES: &[&str] = &[
        "fs::write",
        "write_all",
        "OpenOptions",
        "File::create",
        "fs::rename",
        "remove_file",
        "remove_dir",
        "fs::copy",
        "fs::create_dir",
    ];

    #[test]
    fn direct_world_writes_go_through_the_shared_helpers() {
        let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut violations = Vec::new();
        let mut exemptions = 0usize;
        for folder in ["src/data", "src/commands"] {
            walk(&manifest.join(folder), &mut |path| {
                if skip_file(path) {
                    return;
                }
                let text = std::fs::read_to_string(path).unwrap();
                let stripped = strip_cfg_test(&text);
                let lines: Vec<&str> = stripped.lines().collect();
                for (index, line) in lines.iter().enumerate() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("//") || is_use_line(trimmed) {
                        continue;
                    }
                    if !NEEDLES.iter().any(|needle| line.contains(needle)) {
                        continue;
                    }
                    let previous = index
                        .checked_sub(1)
                        .and_then(|i| lines.get(i))
                        .copied()
                        .unwrap_or("");
                    if exemption_ok(previous) {
                        exemptions += 1;
                    } else {
                        violations.push(format!("{}:{}: {}", path.display(), index + 1, trimmed));
                    }
                }
            });
        }
        assert!(
            violations.is_empty(),
            "直接寫檔沒有豁免：\n{}",
            violations.join("\n")
        );
        assert!(
            exemptions >= 15,
            "掃描沒有看到已知的非桌目錄豁免：{exemptions}"
        );
    }

    fn skip_file(path: &std::path::Path) -> bool {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if name == "tests.rs" || name == "test_support.rs" || name == "world_file.rs" {
            return true;
        }
        path.components()
            .any(|component| component.as_os_str() == "tests")
    }

    fn is_use_line(trimmed: &str) -> bool {
        trimmed.starts_with("use ")
            || trimmed.starts_with("pub use ")
            || trimmed.starts_with("pub(crate) use ")
            || trimmed.starts_with("pub(super) use ")
    }

    fn exemption_ok(previous: &str) -> bool {
        let trimmed = previous.trim();
        trimmed.starts_with("// world-write-exempt:") && trimmed.contains("不是桌目錄")
    }

    fn walk(dir: &std::path::Path, visit: &mut dyn FnMut(&std::path::Path)) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, visit);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                visit(&path);
            }
        }
    }

    fn strip_cfg_test(source: &str) -> String {
        let bytes = source.as_bytes();
        let mut out = source.to_owned().into_bytes();
        let mut i = 0;
        let mut state = Lex::Code;
        while i < bytes.len() {
            if at_line_start(&bytes, i) {
                if let Some(after_attr) = match_cfg_test(&bytes, i) {
                    let end = skip_item(&bytes, after_attr);
                    blank(&mut out, i, end);
                    i = end;
                    state = Lex::Code;
                    continue;
                }
            }
            i = advance(&bytes, i, &mut state);
        }
        String::from_utf8(out).unwrap()
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Lex {
        Code,
        Line,
        Block(u32),
        String,
        Raw(u8),
    }

    fn at_line_start(bytes: &[u8], index: usize) -> bool {
        bytes[..index]
            .iter()
            .rev()
            .take_while(|byte| **byte != b'\n')
            .all(|byte| byte.is_ascii_whitespace())
    }

    fn match_cfg_test(bytes: &[u8], index: usize) -> Option<usize> {
        let rest = &bytes[index..];
        if !rest.starts_with(b"#[") {
            return None;
        }
        let mut j = index + 2;
        j = skip_ws(bytes, j);
        if !bytes[j..].starts_with(b"cfg") {
            return None;
        }
        j += 3;
        j = skip_ws(bytes, j);
        if bytes.get(j) != Some(&b'(') {
            return None;
        }
        j += 1;
        j = skip_ws(bytes, j);
        if !bytes[j..].starts_with(b"test") {
            return None;
        }
        j += 4;
        j = skip_ws(bytes, j);
        if bytes.get(j) != Some(&b')') {
            return None;
        }
        j += 1;
        j = skip_ws(bytes, j);
        if bytes.get(j) != Some(&b']') {
            return None;
        }
        Some(j + 1)
    }

    fn skip_ws(bytes: &[u8], mut index: usize) -> usize {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        index
    }

    fn skip_item(bytes: &[u8], mut index: usize) -> usize {
        let mut paren = 0i32;
        let mut bracket = 0i32;
        let mut state = Lex::Code;
        while index < bytes.len() {
            if state == Lex::Code && paren == 0 && bracket == 0 && bytes[index] == b'{' {
                return skip_brace(bytes, index);
            }
            if state == Lex::Code && paren == 0 && bracket == 0 && bytes[index] == b';' {
                return index + 1;
            }
            if state == Lex::Code {
                match bytes[index] {
                    b'(' => paren += 1,
                    b')' => paren -= 1,
                    b'[' => bracket += 1,
                    b']' => bracket -= 1,
                    _ => {}
                }
            }
            index = advance(bytes, index, &mut state);
        }
        index
    }

    fn skip_brace(bytes: &[u8], open: usize) -> usize {
        let mut depth = 0i32;
        let mut index = open;
        let mut state = Lex::Code;
        while index < bytes.len() {
            if state == Lex::Code {
                match bytes[index] {
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            return index + 1;
                        }
                    }
                    _ => {}
                }
            }
            index = advance(bytes, index, &mut state);
        }
        index
    }

    fn advance(bytes: &[u8], index: usize, state: &mut Lex) -> usize {
        let byte = bytes[index];
        match *state {
            Lex::Line => {
                if byte == b'\n' {
                    *state = Lex::Code;
                }
                index + 1
            }
            Lex::Block(depth) => {
                if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
                    *state = Lex::Block(depth + 1);
                    return index + 2;
                }
                if byte == b'*' && bytes.get(index + 1) == Some(&b'/') {
                    *state = if depth == 1 {
                        Lex::Code
                    } else {
                        Lex::Block(depth - 1)
                    };
                    return index + 2;
                }
                index + 1
            }
            Lex::String => {
                if byte == b'\\' {
                    return index + 2;
                }
                if byte == b'"' {
                    *state = Lex::Code;
                }
                index + 1
            }
            Lex::Raw(hashes) => {
                if byte == b'"' && bytes[index + 1..].starts_with(&vec![b'#'; hashes as usize]) {
                    *state = Lex::Code;
                    return index + 1 + hashes as usize;
                }
                index + 1
            }
            Lex::Code => {
                if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
                    *state = Lex::Line;
                    return index + 2;
                }
                if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
                    *state = Lex::Block(1);
                    return index + 2;
                }
                if byte == b'"' {
                    *state = Lex::String;
                    return index + 1;
                }
                // 只有 'x'、'\n'、'\u{..}' 這種字元常值才整段跳過。'static 這類生命週期沒有配對引號，
                // 若進字元狀態會吞到下一個 '，cfg(test) 空白化就可能越界把後面的寫檔藏起來。
                if byte == b'\'' {
                    if let Some(end) = char_literal_end(bytes, index) {
                        return end;
                    }
                    return index + 1;
                }
                if let Some((hashes, start)) = raw_string_at(bytes, index) {
                    *state = Lex::Raw(hashes);
                    return start;
                }
                index + 1
            }
        }
    }

    /// 開引號在 `open`。認得出字元常值就回結束引號的下一個位置。
    fn char_literal_end(bytes: &[u8], open: usize) -> Option<usize> {
        let body = open + 1;
        if bytes.get(body) == Some(&b'\\') {
            let after = skip_char_escape(bytes, body)?;
            if bytes.get(after) == Some(&b'\'') {
                return Some(after + 1);
            }
            return None;
        }
        let width = utf8_width(*bytes.get(body)?);
        let close = body + width;
        if width > 0 && bytes.get(close) == Some(&b'\'') {
            return Some(close + 1);
        }
        None
    }

    fn skip_char_escape(bytes: &[u8], slash: usize) -> Option<usize> {
        match *bytes.get(slash + 1)? {
            b'n' | b'r' | b't' | b'\\' | b'0' | b'\'' | b'"' => Some(slash + 2),
            b'x' => {
                let ok = bytes.get(slash + 2).is_some_and(u8::is_ascii_hexdigit)
                    && bytes.get(slash + 3).is_some_and(u8::is_ascii_hexdigit);
                if ok {
                    Some(slash + 4)
                } else {
                    None
                }
            }
            b'u' => {
                if bytes.get(slash + 2) != Some(&b'{') {
                    return None;
                }
                let mut index = slash + 3;
                let start = index;
                while index < bytes.len() && bytes[index].is_ascii_hexdigit() && index - start < 6 {
                    index += 1;
                }
                if index == start || bytes.get(index) != Some(&b'}') {
                    return None;
                }
                Some(index + 1)
            }
            _ => None,
        }
    }

    fn utf8_width(first: u8) -> usize {
        if first < 0x80 {
            1
        } else if first & 0xE0 == 0xC0 {
            2
        } else if first & 0xF0 == 0xE0 {
            3
        } else if first & 0xF8 == 0xF0 {
            4
        } else {
            0
        }
    }

    fn raw_string_at(bytes: &[u8], index: usize) -> Option<(u8, usize)> {
        let mut i = index;
        if bytes.get(i) == Some(&b'b') || bytes.get(i) == Some(&b'c') {
            i += 1;
        }
        if bytes.get(i) != Some(&b'r') {
            return None;
        }
        i += 1;
        let mut hashes = 0u8;
        while bytes.get(i) == Some(&b'#') {
            hashes += 1;
            i += 1;
        }
        if bytes.get(i) != Some(&b'"') {
            return None;
        }
        Some((hashes, i + 1))
    }

    #[test]
    fn lifetime_inside_cfg_test_does_not_hide_a_later_write() {
        let source = "\
#[cfg(test)]
fn helper<'static>() {
    let _ = 1;
}

fn ship() {
    fs::write(path, data);
}
";
        let stripped = strip_cfg_test(source);
        let lines: Vec<&str> = stripped.lines().collect();
        let caught = lines.iter().enumerate().any(|(index, line)| {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || is_use_line(trimmed) || !line.contains("fs::write") {
                return false;
            }
            let previous = index
                .checked_sub(1)
                .and_then(|i| lines.get(i))
                .copied()
                .unwrap_or("");
            !exemption_ok(previous)
        });
        assert!(caught, "fs::write 沒被抓到：\n{stripped}");
    }

    fn blank(buf: &mut [u8], start: usize, end: usize) {
        for byte in &mut buf[start..end] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    }
}

#[cfg(test)]
mod rename_tests {
    use super::*;

    struct Dir(PathBuf);

    impl Dir {
        fn new(label: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("tt-rename-{label}-{}", ulid::Ulid::generate()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_transient_failure_is_retried_where_the_platform_retries() {
        let dir = Dir::new("transient");
        let from = dir.0.join("a");
        let to = dir.0.join("b");
        fs::write(&from, b"x").unwrap();
        let guard = RenameAttemptGuard::fail_io(1);
        let result = rename_path(&from, &to);
        if RENAME_ATTEMPTS > 1 {
            result.unwrap();
            assert_eq!(guard.counts(), (2, 1));
            assert!(to.is_file() && !from.exists());
        } else {
            assert!(result.is_err());
            assert_eq!(guard.counts(), (1, 0));
            assert!(from.is_file() && !to.exists());
        }
    }

    #[test]
    fn persistent_failure_tries_every_attempt_and_moves_nothing() {
        let dir = Dir::new("persistent");
        let from = dir.0.join("a");
        let to = dir.0.join("b");
        fs::write(&from, b"x").unwrap();
        let guard = RenameAttemptGuard::fail_io(u32::MAX);
        let error = rename_path(&from, &to).unwrap_err().to_string();
        let expected = UiMsg::RenameFailedIo {
            from: from.display().to_string(),
            to: to.display().to_string(),
            error: "injected rename io error".to_owned(),
        }
        .to_string();
        assert_eq!(error, expected);
        assert_eq!(guard.counts(), (RENAME_ATTEMPTS, RENAME_ATTEMPTS - 1));
        assert_eq!(RENAME_ATTEMPTS, if cfg!(windows) { 8 } else { 1 });
        assert_eq!(fs::read(&from).unwrap(), b"x");
        assert!(!to.exists());
    }

    #[test]
    fn an_injected_call_failure_skips_the_attempts_entirely() {
        let dir = Dir::new("injected");
        let from = dir.0.join("a");
        let to = dir.0.join("b");
        fs::write(&from, b"x").unwrap();
        let attempts = RenameAttemptGuard::fail_io(0);
        let fails = RenameFailGuard::fail_after(1, 1);
        rename_path(&from, &to).unwrap();
        assert!(rename_path(&to, &from).is_err());
        rename_path(&to, &from).unwrap();
        drop(fails);
        assert_eq!(attempts.counts(), (2, 0), "被注入的那次呼叫不進重試迴圈");
        assert!(from.is_file());
    }
}
