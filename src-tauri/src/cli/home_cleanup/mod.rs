//! app 專用 CLI 家目錄（agy-home、grok-home、codex-home）的清理（plans/agy-home-cleanup.md）。
//! 共用規則：家目錄只由 config_root 推出；從家目錄起逐層驗證是實體目錄（任一層是連結就不動）；
//! 只刪固定前綴下、名稱合格的項目，項目本身是連結只刪連結、不解析目標。

mod agy;
mod codex;
mod grok;
#[cfg(test)]
mod tests;

pub(crate) use agy::{remove_conversation, sweep_agy};
pub(crate) use codex::{codex_home_dir, delete_thread as delete_codex_thread, sweep_codex};
pub(crate) use grok::{
    ensure_ttl, remove_prompt_history, remove_search_index, remove_session, TtlOutcome,
};

use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// 保留天數〔作者裁決 2026-10-28〕：閒置超過就回收（agy 由 app 掃、grok 交給 CLI 自帶的 TTL）。
pub(crate) const RETENTION: Duration = Duration::from_secs(60 * 60 * 24 * 7);

pub(crate) fn agy_home_dir(config_root: &Path) -> PathBuf {
    config_root.join("agy-home")
}

pub(crate) fn grok_home_dir(config_root: &Path) -> PathBuf {
    config_root.join("grok-home")
}

/// 標準 36 字元 UUID（小寫十六進位＋四個連字號）。agy 的對話 id、app 給 grok 的 session id 都是這個形狀。
pub(crate) fn is_uuid(name: &str) -> bool {
    name.len() == 36
        && name.char_indices().all(|(index, ch)| match index {
            8 | 13 | 18 | 23 => ch == '-',
            _ => ch.is_ascii_digit() || ('a'..='f').contains(&ch),
        })
}

/// 從 `home` 起逐層驗：`home` 本身與每一段都必須是實體目錄（`symlink_metadata`，不跟隨連結）。
/// 任一層不存在回 `Ok(None)`（沒東西可清）；是連結或不是目錄回錯，呼叫端整個不動。
pub(crate) fn real_dir(home: &Path, segments: &[&str]) -> Result<Option<PathBuf>, String> {
    let mut path = home.to_path_buf();
    for (depth, segment) in std::iter::once("")
        .chain(segments.iter().copied())
        .enumerate()
    {
        if depth > 0 {
            path.push(segment);
        }
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(format!("{} 是連結，不清理", path.display()))
            }
            Ok(meta) if !meta.is_dir() => {
                return Err(format!("{} 不是資料夾，不清理", path.display()))
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("{}：{error}", path.display())),
        }
    }
    Ok(Some(path))
}

/// 刪一個項目，不跟隨連結：連結只刪連結本身；目錄用 std `remove_dir_all`（文件明載不跟隨連結，
/// unix 以 openat／unlinkat 防 TOCTOU）；檔案 `remove_file`。不存在當已刪。
pub(crate) fn remove_entry(path: &Path) -> io::Result<()> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let result = if meta.file_type().is_symlink() {
        // Windows 的目錄連結要用 remove_dir 刪，remove_file 會失敗
        std::fs::remove_file(path).or_else(|_| std::fs::remove_dir(path))
    } else if meta.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    match result {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// 項目（目錄則遞迴、不跟隨連結）裡最新的 mtime。任何一步讀不到就回錯，呼叫端整組保留。
pub(crate) fn newest_mtime(path: &Path) -> io::Result<SystemTime> {
    let meta = std::fs::symlink_metadata(path)?;
    let mut newest = meta.modified()?;
    if meta.is_dir() {
        for entry in std::fs::read_dir(path)? {
            newest = newest.max(newest_mtime(&entry?.path())?);
        }
    }
    Ok(newest)
}

/// app 啟動時背景跑一次：agy、codex 依引用集＋7 天回收；grok 刪 prompt 歷史與（沒有 grok 在跑時）
/// 搜尋索引，session 交給它自帶的 `storage.cleanup_ttl_days`。失敗只記 warn。
/// 同步檔案操作留在這條 std thread；只有 codex delete 用 tauri 的 async runtime 跑（spawn helper 是 tokio）。
pub(crate) fn spawn_startup_sweep(config_root: PathBuf, data_root: PathBuf) {
    std::thread::spawn(move || {
        let agy = agy_home_dir(&config_root);
        match sweep_agy(
            &agy,
            || crate::lanes::referenced_session_ids(&data_root),
            SystemTime::now(),
        ) {
            Ok(removed) if removed > 0 => log::info!("agy-home 回收 {removed} 個對話"),
            Ok(_) => {}
            Err(error) => log::warn!("agy-home 回收略過：{error}"),
        }
        let grok = grok_home_dir(&config_root);
        if let Err(error) = remove_prompt_history(&grok) {
            log::warn!("grok prompt 歷史清理失敗：{error}");
        }
        super::grok_gate::clear_if_idle(&grok);
        sweep_codex_home(&config_root, &data_root);
    });
}

fn sweep_codex_home(config_root: &Path, data_root: &Path) {
    let home = codex_home_dir(config_root);
    let workspace = config_root.join("cli-workspace");
    let cwd = if workspace.is_dir() {
        workspace
    } else {
        config_root.to_path_buf()
    };
    let program = super::find_binary("codex");
    let delete = |id: &str| match &program {
        Some(program) => {
            tauri::async_runtime::block_on(delete_codex_thread(program, &home, &cwd, id))
        }
        None => Err("找不到 codex".to_owned()),
    };
    match sweep_codex(
        &home,
        || crate::lanes::referenced_session_ids(data_root),
        SystemTime::now(),
        delete,
    ) {
        Ok((0, 0)) => {}
        Ok((threads, images)) => {
            log::info!("codex-home 回收 {threads} 個 thread、{images} 個生圖目錄")
        }
        Err(error) => log::warn!("codex-home 回收略過：{error}"),
    }
}

/// agy 文字單發的善後：結束（正常、出錯、future 被丟棄）就刪這次的對話。被中止時程序可能
/// 還在退場、晚於刪除又寫回檔，留給啟動回收（已知限制）。
pub(crate) struct AgyOneshot<'a> {
    pub home: PathBuf,
    pub slot: &'a std::sync::Mutex<Option<String>>,
}

impl Drop for AgyOneshot<'_> {
    fn drop(&mut self) {
        let id = self
            .slot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(id) = id {
            if let Err(error) = remove_conversation(&self.home, &id) {
                log::warn!("agy 單發對話 {id} 刪除失敗：{error}");
            }
        }
    }
}

/// grok 文字單發的善後：刪這次的 session（id 由 app 用 `-s` 指定）。呼叫被中止時程序可能還沒收屍，
/// 交給 grok 閘等在途歸零才刪，不在 grok 還在寫的時候刪。
pub(crate) struct GrokOneshot {
    pub home: PathBuf,
    pub session: String,
}

impl Drop for GrokOneshot {
    fn drop(&mut self) {
        super::grok_gate::remove_session_when_idle(&self.home, &self.session);
    }
}
