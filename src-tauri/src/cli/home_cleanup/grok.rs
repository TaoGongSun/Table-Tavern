//! grok-home 的清理：prompt 歷史、搜尋索引、自帶 TTL 設定（grok 1.0.70 實測，見計畫檔）。
//! 閒置的 session 由 grok 的 `storage.cleanup_ttl_days` 回收；單發與撤線的 session 由 `remove_session` 即刪。

use super::{real_dir, remove_entry};
use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, Write};
use std::path::Path;
use std::time::{Duration, Instant};

/// 寫進 grok-home `config.toml` 的閒置天數〔作者裁決 2026-10-28：7 天〕。
pub(crate) const TTL_DAYS: i64 = 7;

/// 刪掉 `sessions/<群組>/<id>/`。連結一律不跟隨：sessions 或群組是連結就不進去找，`<id>` 是連結不算
/// session（都回 0、不刪）；home 是連結回錯。
/// 回刪掉的個數：0＝本來就沒有（當已刪）；多個一個都不刪（不知道哪個才是），回錯。
pub(crate) fn remove_session(home: &Path, session_id: &str) -> Result<usize, String> {
    if session_id.is_empty() || session_id.contains(['/', '\\']) || session_id.starts_with('.') {
        return Err(format!("session id 不合法：{session_id:?}"));
    }
    let Some(home) = real_dir(home, &[])? else {
        return Ok(0);
    };
    let sessions = home.join("sessions");
    match std::fs::symlink_metadata(&sessions) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => return Ok(0), // 連結或不是資料夾：沒有可刪的 session
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(format!("{}：{error}", sessions.display())),
    }
    let entries =
        std::fs::read_dir(&sessions).map_err(|error| format!("{}：{error}", sessions.display()))?;
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("{}：{error}", sessions.display()))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        // 連結與一般檔（session_search.sqlite 等）不是群組，可略過；讀不到的一律停——
        // 少看一個群組就可能把「多個同 id」誤判成唯一
        let kind = entry
            .file_type()
            .map_err(|error| format!("{}：{error}", entry.path().display()))?;
        if !kind.is_dir() {
            continue;
        }
        let Some(group) = real_dir(&sessions, &[name])? else {
            continue;
        };
        let candidate = group.join(session_id);
        match std::fs::symlink_metadata(&candidate) {
            Ok(meta) if meta.is_dir() => found.push(candidate),
            Ok(_) => {} // 連結或檔案：不是 session
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("{}：{error}", candidate.display())),
        }
    }
    match found.as_slice() {
        [] => Ok(0),
        [dir] => remove_entry(dir)
            .map(|()| 1)
            .map_err(|error| format!("無法刪除 {}：{error}", dir.display())),
        _ => Err(format!(
            "session {session_id} 有 {} 個目錄，一個都不刪",
            found.len()
        )),
    }
}

/// 每個群組的 `prompt_history.jsonl`：每次呼叫追加 prompt 全文（含私設），app 用不到。
/// 只認這個檔名；`sessions`、群組任一層是連結就跳過那一層。單純追加的檔，grok 在跑也能刪。
pub(crate) fn remove_prompt_history(home: &Path) -> Result<(), String> {
    let Some(sessions) = real_dir(home, &["sessions"])? else {
        return Ok(());
    };
    let entries =
        std::fs::read_dir(&sessions).map_err(|error| format!("{}：{error}", sessions.display()))?;
    let mut failure = None;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let group = match real_dir(&sessions, &[name]) {
            Ok(Some(group)) => group,
            // 連結或不是目錄（例如 session_search.sqlite）：不是群組
            _ => continue,
        };
        let target = group.join("prompt_history.jsonl");
        if let Err(error) = remove_entry(&target) {
            failure = Some(format!("{}：{error}", target.display()));
        }
    }
    failure.map_or(Ok(()), Err)
}

/// 搜尋索引整組：依序 `-journal`、`-wal`、`-shm`、主檔，再刪 grok 改名留下的
/// `session_search.sqlite*.corrupt.*`；`session_search.sqlite.lock` 不動。
/// 只能在沒有 grok 開著它時呼叫（由 `grok_gate` 保證）。
pub(crate) fn remove_search_index(home: &Path) -> Result<(), String> {
    let Some(sessions) = real_dir(home, &["sessions"])? else {
        return Ok(());
    };
    const MAIN: &str = "session_search.sqlite";
    let mut targets: Vec<_> = ["-journal", "-wal", "-shm", ""]
        .iter()
        .map(|suffix| sessions.join(format!("{MAIN}{suffix}")))
        .collect();
    let entries =
        std::fs::read_dir(&sessions).map_err(|error| format!("{}：{error}", sessions.display()))?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name
            .to_str()
            .is_some_and(|name| name.starts_with(MAIN) && name.contains(".corrupt."))
        {
            targets.push(entry.path());
        }
    }
    for target in targets {
        remove_entry(&target).map_err(|error| format!("{}：{error}", target.display()))?;
    }
    Ok(())
}

/// TTL 設定寫入的結果；三種都算這個行程做過了，不再重試。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TtlOutcome {
    Written,
    AlreadySet,
    /// 拿不到 grok 的初始化鎖（grok 自己正在寫設定）
    LockBusy,
}

const LOCK_WAIT: Duration = Duration::from_secs(2);

/// 把 `[storage] cleanup_ttl_days = TTL_DAYS` 寫進 grok-home 的 `config.toml`。
/// 照 grok 的鎖協議：持有 `.config-init.lock`（grok 等 1 秒拿不到就放棄自己的寫入；與 flock 互斥，實測）
/// 才讀改寫。檔案不存在用 `create_new` 建；存在就以 `toml_edit` 只改這個鍵、其他段落原樣保留，
/// 臨時檔＋rename、保留原權限。`storage` 不是 table、解析失敗、是連結都不寫、回錯。
pub(crate) fn ensure_ttl(home: &Path) -> Result<TtlOutcome, String> {
    let Some(home) = real_dir(home, &[])? else {
        return Err(format!("{} 不存在", home.display()));
    };
    let lock_path = home.join(".config-init.lock");
    if std::fs::symlink_metadata(&lock_path).is_ok_and(|meta| !meta.is_file()) {
        return Err(format!("{} 不是一般檔", lock_path.display()));
    }
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .map_err(|error| format!("{}：{error}", lock_path.display()))?;
    if !acquire(&lock)? {
        return Ok(TtlOutcome::LockBusy);
    }
    let path = home.join("config.toml");
    let fresh = format!("[storage]\ncleanup_ttl_days = {TTL_DAYS}\n");
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            file.write_all(fresh.as_bytes())
                .and_then(|()| file.sync_all())
                .map_err(|error| format!("{}：{error}", path.display()))?;
            return Ok(TtlOutcome::Written);
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(format!("{}：{error}", path.display())),
    }
    let meta =
        std::fs::symlink_metadata(&path).map_err(|error| format!("{}：{error}", path.display()))?;
    if !meta.is_file() {
        return Err(format!("{} 不是一般檔，不寫", path.display()));
    }
    let text =
        std::fs::read_to_string(&path).map_err(|error| format!("{}：{error}", path.display()))?;
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|error| format!("{} 解析失敗，不寫：{error}", path.display()))?;
    let storage = doc
        .entry("storage")
        .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()));
    let Some(storage) = storage.as_table_mut() else {
        return Err(format!("{} 的 storage 不是 table，不寫", path.display()));
    };
    if storage
        .get("cleanup_ttl_days")
        .and_then(toml_edit::Item::as_integer)
        == Some(TTL_DAYS)
    {
        return Ok(TtlOutcome::AlreadySet);
    }
    storage["cleanup_ttl_days"] = toml_edit::value(TTL_DAYS);
    write_replace(&path, doc.to_string().as_bytes(), meta.permissions())?;
    Ok(TtlOutcome::Written)
}

/// 最多等 `LOCK_WAIT` 拿獨占鎖；鎖跟著 `File` 走，呼叫端握著 `lock` 期間都有效。
fn acquire(lock: &File) -> Result<bool, String> {
    let deadline = Instant::now() + LOCK_WAIT;
    loop {
        match lock.try_lock() {
            Ok(()) => return Ok(true),
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(TryLockError::WouldBlock) => return Ok(false),
            Err(TryLockError::Error(error)) => return Err(error.to_string()),
        }
    }
}

/// 同目錄臨時檔寫好、套原權限、fsync 再 rename 蓋上；失敗就刪臨時檔。
fn write_replace(
    path: &Path,
    bytes: &[u8],
    permissions: std::fs::Permissions,
) -> Result<(), String> {
    let mut nonce = [0u8; 8];
    getrandom::fill(&mut nonce).map_err(|error| error.to_string())?;
    let temp = path.with_file_name(format!(
        ".config.toml.tt-{}",
        nonce.iter().map(|b| format!("{b:02x}")).collect::<String>()
    ));
    let result = (|| -> io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.set_permissions(permissions)?;
        file.sync_all()?;
        std::fs::rename(&temp, path)
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("{}：{error}", path.display()));
    }
    Ok(())
}
