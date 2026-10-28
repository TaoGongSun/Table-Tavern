//! agy-home 的對話刪除與啟動回收。一個對話的檔都在 `.gemini/antigravity-cli/` 下：
//! `conversations/<id>.db(-wal,-shm)`、`brain/<id>/`、`annotations/<id>.pbtxt`、`presence/<id>.lock`
//! （agy 1.4.8 實測；`implicit/<uuid>.pb` 的 uuid 不是對話 id，不屬於對話）。

use super::{is_uuid, newest_mtime, real_dir, remove_entry, RETENTION};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const BASE: [&str; 2] = [".gemini", "antigravity-cli"];

/// 每個子目錄認的後綴；`brain` 是以 id 命名的資料夾（空後綴）。不認得的後綴一律跳過。
const LAYOUT: [(&str, &[&str]); 4] = [
    ("conversations", &[".db", ".db-wal", ".db-shm"]),
    ("brain", &[""]),
    ("annotations", &[".pbtxt"]),
    ("presence", &[".lock"]),
];

/// 名稱拆成（id, 後綴）：前 36 字是 UUID、剩下的在白名單裡才算。
fn parse_name<'a>(name: &'a str, suffixes: &[&str]) -> Option<&'a str> {
    let (id, suffix) = name.split_at_checked(36)?;
    (is_uuid(id) && suffixes.contains(&suffix)).then_some(id)
}

/// 刪掉一個對話的所有檔。家目錄、`.gemini`、`antigravity-cli`、各子目錄任一層是連結就整個不動。
pub(crate) fn remove_conversation(home: &Path, id: &str) -> Result<(), String> {
    if !is_uuid(id) {
        return Err(format!("agy 對話 id 不合法：{id:?}"));
    }
    let Some(base) = real_dir(home, &BASE)? else {
        return Ok(());
    };
    let mut targets = Vec::new();
    for (sub, suffixes) in LAYOUT {
        let Some(dir) = real_dir(&base, &[sub])? else {
            continue;
        };
        targets.extend(
            suffixes
                .iter()
                .map(|suffix| dir.join(format!("{id}{suffix}"))),
        );
    }
    for target in targets {
        remove_entry(&target).map_err(|error| format!("{}：{error}", target.display()))?;
    }
    Ok(())
}

/// 啟動回收：不在引用集、而且組內所有項目都 7 天沒動的對話才刪；`log/cli-*.log` 同樣 7 天。
/// 順序：先列候選、再讀引用集（`mark`，失敗整輪停）、每組刪前重新 stat。回傳刪掉的對話數。
pub(crate) fn sweep_agy(
    home: &Path,
    mark: impl FnOnce() -> Result<HashSet<String>, String>,
    now: SystemTime,
) -> Result<usize, String> {
    let Some(base) = real_dir(home, &BASE)? else {
        return Ok(0);
    };
    let cutoff = now.checked_sub(RETENTION).unwrap_or(SystemTime::UNIX_EPOCH);
    let mut groups: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for (sub, suffixes) in LAYOUT {
        let Some(dir) = real_dir(&base, &[sub])? else {
            continue;
        };
        let entries =
            std::fs::read_dir(&dir).map_err(|error| format!("{}：{error}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|error| format!("{}：{error}", dir.display()))?;
            let name = entry.file_name();
            let Some(id) = name.to_str().and_then(|name| parse_name(name, suffixes)) else {
                continue;
            };
            groups.entry(id.to_owned()).or_default().push(entry.path());
        }
    }
    let idle = |paths: &[PathBuf]| -> bool {
        paths
            .iter()
            .map(|path| newest_mtime(path))
            .try_fold(true, |all_old, mtime| {
                mtime.map(|mtime| all_old && mtime < cutoff)
            })
            .unwrap_or(false) // 任何一個讀不到：整組保留
    };
    let candidates: Vec<_> = groups
        .into_iter()
        .filter(|(_, paths)| idle(paths))
        .collect();
    let referenced = mark()?;
    let mut removed = 0;
    for (id, paths) in candidates {
        // 引用集讀完後再 stat 一次：候選列出後才被動到的不刪
        if referenced.contains(&id) || !idle(&paths) {
            continue;
        }
        match remove_conversation(home, &id) {
            Ok(()) => removed += 1,
            Err(error) => log::warn!("agy 對話 {id} 回收失敗：{error}"),
        }
    }
    sweep_logs(&base, cutoff);
    Ok(removed)
}

/// `log/cli-<時間>.log` 早於門檻的一般檔；`cli.log` 連結與它指向的那份不動。
fn sweep_logs(base: &Path, cutoff: SystemTime) {
    let dir = match real_dir(base, &["log"]) {
        Ok(Some(dir)) => dir,
        Ok(None) => return,
        Err(error) => {
            log::warn!("agy log 清理略過：{error}");
            return;
        }
    };
    // agy 1.4.8 實測：`cli.log` 在 antigravity-cli 這層，指向 `log/cli-<時間>.log`
    let current = std::fs::read_link(base.join("cli.log"))
        .ok()
        .and_then(|target| target.file_name().map(|name| name.to_owned()));
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(text) = name.to_str() else { continue };
        if !(text.starts_with("cli-") && text.ends_with(".log")) || current.as_ref() == Some(&name)
        {
            continue;
        }
        let path = entry.path();
        let old = std::fs::symlink_metadata(&path)
            .ok()
            .filter(|meta| meta.is_file())
            .and_then(|meta| meta.modified().ok())
            .is_some_and(|mtime| mtime < cutoff);
        if old {
            if let Err(error) = remove_entry(&path) {
                log::warn!("agy log {} 刪除失敗：{error}", path.display());
            }
        }
    }
}
