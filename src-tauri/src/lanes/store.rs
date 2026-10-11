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
