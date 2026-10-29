//! CSP 沙盒負面探測（.ai/plans/desktop-csp.md）：探測卡在卡片沙盒內試打 `harness_probe`，
//! 宿主先自己打一次當正向對照，最後讀 `harness_probe_count` 必須只剩宿主那一次。

use std::sync::atomic::{AtomicU32, Ordering};

static HITS: AtomicU32 = AtomicU32::new(0);

#[tauri::command]
pub(crate) fn harness_probe() -> u32 {
    HITS.fetch_add(1, Ordering::SeqCst) + 1
}

#[tauri::command]
pub(crate) fn harness_probe_count() -> u32 {
    HITS.load(Ordering::SeqCst)
}
