//! codex-home 的 thread 刪除與啟動回收（plans/agy-home-cleanup.md 包 2）。刪 thread 一律走官方
//! `codex delete <id> --force`：它清 rollout、threads 列、thread_history 等（codex 0.164.0 實測）；
//! 自己刪 rollout 會留下 SQLite 裡的 prompt 原文。生圖 `generated_images/<thread>/` 官方不清，由這裡刪。

use super::{is_uuid, newest_mtime, real_dir, remove_entry, RETENTION};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime};

pub(crate) fn codex_home_dir(config_root: &Path) -> PathBuf {
    config_root.join("codex-home")
}

const DELETE_TIMEOUT: Duration = Duration::from_secs(10);

/// app 行程內所有 codex delete 串行（codex 的 session index 跨程序沒有鎖）。
fn delete_lock() -> Arc<tokio::sync::Mutex<()>> {
    static LOCK: OnceLock<Arc<tokio::sync::Mutex<()>>> = OnceLock::new();
    LOCK.get_or_init(Default::default).clone()
}

/// 刪一條 codex thread。codex-home 不在就不呼叫。整個刪除在獨立任務裡持鎖做完（含逾時後的
/// kill＋wait 收屍）：呼叫端 future 被取消也不中斷，確認程序退出前下一個刪除不會開始。
/// spawn 失敗、逾時、rc≠0 都回錯（codex 連「不存在」也只回 rc=1，分不出來），呼叫端只記 warn。
pub(crate) async fn delete_thread(
    program: &Path,
    home: &Path,
    cwd: &Path,
    id: &str,
) -> Result<(), String> {
    delete_thread_within(program, home, cwd, id, DELETE_TIMEOUT).await
}

async fn delete_thread_within(
    program: &Path,
    home: &Path,
    cwd: &Path,
    id: &str,
    limit: Duration,
) -> Result<(), String> {
    if !is_uuid(id) {
        return Err(format!("codex thread id 不合法：{id:?}"));
    }
    if real_dir(home, &[])?.is_none() {
        return Ok(());
    }
    let (program, home, cwd, id) = (
        program.to_path_buf(),
        home.to_path_buf(),
        cwd.to_path_buf(),
        id.to_owned(),
    );
    let lock = delete_lock();
    tokio::spawn(async move {
        let _held = lock.lock_owned().await;
        run_delete(&program, &home, &cwd, &id, limit).await
    })
    .await
    .map_err(|error| format!("codex delete 任務中止：{error}"))?
}

async fn run_delete(
    program: &Path,
    home: &Path,
    cwd: &Path,
    id: &str,
    limit: Duration,
) -> Result<(), String> {
    let envs = crate::cli::codex_envs(home);
    let mut command = tokio::process::Command::new(program);
    command
        .args(["delete", id, "--force"])
        .current_dir(cwd)
        .envs(
            envs.iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        )
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(target_os = "windows")]
    command.creation_flags(0x08000000);
    let mut spawned = super::super::spawn::spawn(&mut command, &envs)
        .map_err(|error| format!("codex delete 啟動失敗：{error}"))?;
    match tokio::time::timeout(limit, spawned.wait_with_output()).await {
        Ok(Ok(output)) if output.status.success() => Ok(()),
        Ok(Ok(output)) => Err(format!(
            "codex delete rc={:?}：{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        Ok(Err(error)) => {
            spawned.kill_and_wait().await;
            Err(format!("codex delete 讀取失敗：{error}"))
        }
        Err(_) => {
            spawned.kill_and_wait().await;
            Err(format!("codex delete 超過 {} 秒", limit.as_secs()))
        }
    }
}

/// rollout 檔名 `rollout-<時間>-<uuid>.jsonl`（或 `.jsonl.zst`）拆出 thread id。
fn rollout_id(name: &str) -> Option<&str> {
    let stem = name
        .strip_suffix(".jsonl")
        .or_else(|| name.strip_suffix(".jsonl.zst"))?;
    if !stem.starts_with("rollout-") || stem.len() < 36 + 9 {
        return None;
    }
    let id = &stem[stem.len() - 36..];
    (is_uuid(id) && stem[..stem.len() - 36].ends_with('-')).then_some(id)
}

/// `sessions/<YYYY>/<MM>/<DD>/` 下的 rollout，依 thread id 分組。日期層任一層是連結就跳過那層。
fn list_rollouts(home: &Path) -> Result<HashMap<String, Vec<PathBuf>>, String> {
    let mut groups: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let Some(sessions) = real_dir(home, &["sessions"])? else {
        return Ok(groups);
    };
    let mut days = vec![sessions];
    for _ in 0..3 {
        let mut next = Vec::new();
        for dir in days {
            for entry in read_dir(&dir)? {
                let meta = entry
                    .metadata()
                    .map_err(|error| format!("{}：{error}", entry.path().display()))?;
                if meta.is_dir() && !meta.file_type().is_symlink() {
                    next.push(entry.path());
                }
            }
        }
        days = next;
    }
    for dir in days {
        for entry in read_dir(&dir)? {
            let name = entry.file_name();
            if let Some(id) = name.to_str().and_then(rollout_id) {
                groups.entry(id.to_owned()).or_default().push(entry.path());
            }
        }
    }
    Ok(groups)
}

/// `generated_images/<uuid>/`：單發生圖的 thread（ephemeral，不會在 lanes.json）。
fn list_images(home: &Path) -> Result<HashMap<String, Vec<PathBuf>>, String> {
    let mut groups = HashMap::new();
    let Some(dir) = real_dir(home, &["generated_images"])? else {
        return Ok(groups);
    };
    for entry in read_dir(&dir)? {
        if let Some(id) = entry.file_name().to_str().filter(|name| is_uuid(name)) {
            groups.insert(id.to_owned(), vec![entry.path()]);
        }
    }
    Ok(groups)
}

fn read_dir(dir: &Path) -> Result<Vec<std::fs::DirEntry>, String> {
    std::fs::read_dir(dir)
        .and_then(|entries| entries.collect::<std::io::Result<Vec<_>>>())
        .map_err(|error| format!("{}：{error}", dir.display()))
}

/// 啟動回收：不在引用集、組內所有項目 7 天沒動的 thread 交給 `delete`（正式碼是 `codex delete`），
/// 生圖目錄直接刪。順序同 agy：先列候選、再讀引用集（失敗整輪停）、刪前重新 stat。
/// 回傳（刪掉的 thread 數, 刪掉的生圖目錄數）。
pub(crate) fn sweep_codex(
    home: &Path,
    mark: impl FnOnce() -> Result<HashSet<String>, String>,
    now: SystemTime,
    mut delete: impl FnMut(&str) -> Result<(), String>,
) -> Result<(usize, usize), String> {
    let cutoff = now.checked_sub(RETENTION).unwrap_or(SystemTime::UNIX_EPOCH);
    let idle = |paths: &[PathBuf]| -> bool {
        paths
            .iter()
            .map(|path| newest_mtime(path))
            .try_fold(true, |all_old, mtime| {
                mtime.map(|mtime| all_old && mtime < cutoff)
            })
            .unwrap_or(false) // 任何一個讀不到：整組保留
    };
    let keep_idle = |groups: HashMap<String, Vec<PathBuf>>| -> Vec<(String, Vec<PathBuf>)> {
        groups
            .into_iter()
            .filter(|(_, paths)| idle(paths))
            .collect()
    };
    let rollouts = keep_idle(list_rollouts(home)?);
    let images = keep_idle(list_images(home)?);
    if rollouts.is_empty() && images.is_empty() {
        return Ok((0, 0));
    }
    let referenced = mark()?;
    let mut threads = 0;
    for (id, paths) in rollouts {
        if referenced.contains(&id) || !idle(&paths) {
            continue;
        }
        match delete(&id) {
            Ok(()) => threads += 1,
            Err(error) => log::warn!("codex thread {id} 回收失敗：{error}"),
        }
    }
    let mut removed_images = 0;
    for (id, paths) in images {
        if referenced.contains(&id) || !idle(&paths) {
            continue;
        }
        match paths.iter().try_for_each(|path| remove_entry(path)) {
            Ok(()) => removed_images += 1,
            Err(error) => log::warn!("codex 生圖 {id} 回收失敗：{error}"),
        }
    }
    Ok((threads, removed_images))
}

#[cfg(all(test, unix))]
mod tests;
