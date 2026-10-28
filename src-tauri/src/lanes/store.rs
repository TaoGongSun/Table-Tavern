//! lanes.json 的讀寫、每桌互斥鎖與丟線帳本（log_drop／log_drop_failure）。
use super::*;

pub(super) fn read_store(path: &Path) -> LaneStore {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub(super) fn write_store(path: &Path, store: &LaneStore) -> Result<(), String> {
    let text = serde_json::to_string_pretty(store).map_err(|error| error.to_string())?;
    crate::data::commit_world_write(path, text.as_bytes()).map_err(|error| {
        UiMsg::LaneStateWriteFailed {
            path: path.display().to_string(),
            error: error.to_string(),
        }
        .to_string()
    })
}

/// 每桌一把 lane 鎖（以 lanes.json 路徑為鍵）。run_turn 從讀 store 到最終落檔全程持有。
pub(super) fn lane_lock(store_path: &Path) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    static LOCKS: std::sync::OnceLock<
        std::sync::Mutex<
            std::collections::HashMap<PathBuf, std::sync::Arc<tokio::sync::Mutex<()>>>,
        >,
    > = std::sync::OnceLock::new();
    let mut table = LOCKS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    table.entry(store_path.to_path_buf()).or_default().clone()
}

/// 丟線落帳（沒有 stage／detail 的那種）。`transport` 記實際 provider。
pub(super) fn log_drop(call: &LaneCall, world_id: &str, key: &str, reason: &str) {
    if let Some(path) = call.usage_log.as_deref() {
        usage_log::append_event(
            path,
            call.provider.as_str(),
            Some(world_id),
            key,
            usage_log::Event::DropLane,
            reason,
        );
    }
}

/// 丟線落帳，多記壞在哪一步與原錯誤字串（先遮掉 `paths` 裡的路徑再截斷）。
pub(super) fn log_drop_failure(
    call: &LaneCall,
    world_id: &str,
    key: &str,
    reason: &str,
    failure: &RewriteFailure,
    paths: &[(String, String)],
) {
    if let Some(log) = call.usage_log.as_deref() {
        let detail = rewrite_failure::mask_detail(&failure.detail, paths);
        usage_log::append_drop(
            log,
            call.provider.as_str(),
            Some(world_id),
            key,
            reason,
            failure.stage,
            &detail,
        );
    }
}

/// 背景回收（cli::home_cleanup）用的引用集：所有桌（本體、副本、重設暫存）lanes.json 裡的全部
/// session_id，不分 provider。讀前後各拍一次唯讀快照，兩次不同、有桌待修復、任何一份讀不到或
/// 解析不了都回錯——引用集不完整寧可這輪不刪。只有 NotFound 算空。不用 `read_store`（它讀失敗回空）。
pub(crate) fn referenced_session_ids(
    root: &Path,
) -> Result<std::collections::HashSet<String>, String> {
    let before = crate::data::readonly_lane_scan(root)?;
    let mut ids = std::collections::HashSet::new();
    for dir in &before.dirs {
        let path = dir.join("lanes.json");
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("{}：{error}", path.display())),
        };
        let store: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&text)
            .map_err(|error| format!("{} 解析失敗：{error}", path.display()))?;
        for (key, state) in store {
            let id = state
                .get("session_id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| format!("{} 的 {key} 沒有 session_id", path.display()))?;
            ids.insert(id.to_owned());
        }
    }
    let after = crate::data::readonly_lane_scan(root)?;
    if before != after {
        return Err("讀 lanes.json 期間桌子有變動".to_owned());
    }
    Ok(ids)
}
