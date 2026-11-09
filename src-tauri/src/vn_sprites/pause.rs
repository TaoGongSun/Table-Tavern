//! 來源額度暫停僅在記憶體；唯一生圖預檢由 generate_png 在 gate 內呼叫。
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tauri::Emitter;

const QUOTA: &str = "AI_IMAGE_QUOTA:";
pub(crate) const PAUSED: &str = "AI_IMAGE_PAUSED";
pub(crate) const EVENT: &str = "vn-art-flow-status";

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Pause {
    pub reset: Option<i64>,
}
struct Entry {
    pause: Pause,
    timer: Option<tokio::task::JoinHandle<()>>,
}
fn state() -> std::sync::MutexGuard<'static, HashMap<String, Entry>> {
    static STATE: OnceLock<Mutex<HashMap<String, Entry>>> = OnceLock::new();
    STATE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}
pub(crate) fn snapshot(source: &str) -> Option<Pause> {
    state().get(source).map(|e| e.pause.clone())
}
/// 未授權的桌不顯示其他桌造成的來源暫停；手動按鈕仍照常收到 generate_png 的錯誤。
pub(crate) fn for_art(source: &str, started: bool) -> Option<Pause> {
    started.then(|| snapshot(source)).flatten()
}
fn recheck_delay(deadline: i64, now: i64) -> std::time::Duration {
    std::time::Duration::from_millis((deadline - now).clamp(1000, 60_000) as u64)
}
pub(crate) fn is_pause(error: &str) -> bool {
    error.starts_with(QUOTA) || error == PAUSED
}
pub(crate) fn record(app: &tauri::AppHandle, source: &str, error: &str) {
    let Some(reset) = error.strip_prefix(QUOTA) else {
        return;
    };
    let reset = reset.parse::<i64>().ok();
    clear(source);
    let timer = reset.map(|deadline| {
        let app = app.clone();
        let source = source.to_owned();
        tokio::spawn(async move {
            // 避免供應商回過期時間造成忙迴圈；期限本身仍如實顯示。
            loop {
                tokio::time::sleep(recheck_delay(
                    deadline,
                    chrono::Utc::now().timestamp_millis(),
                ))
                .await;
                if chrono::Utc::now().timestamp_millis() >= deadline {
                    break;
                }
            }
            {
                let _gate = super::source_gate().lock().await;
                let mut held = state();
                if !held
                    .get(&source)
                    .and_then(|e| e.pause.reset)
                    .is_some_and(|at| at <= chrono::Utc::now().timestamp_millis())
                {
                    return;
                }
                held.remove(&source); // 不 abort 自己
            }
            resume(&app);
        })
    });
    state().insert(
        source.to_owned(),
        Entry {
            pause: Pause { reset },
            timer,
        },
    );
    let _ = app.emit(EVENT, ());
}
pub(crate) fn clear(source: &str) {
    if let Some(entry) = state().remove(source) {
        if let Some(timer) = entry.timer {
            timer.abort();
        }
    }
}
pub(crate) fn resume(app: &tauri::AppHandle) {
    super::queue::start_worker(app);
    super::prepare::resume_pending(app);
    let _ = app.emit(EVENT, ());
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wake_checks_wall_clock_at_most_a_minute_later() {
        assert_eq!(recheck_delay(86_400_000, 0).as_millis(), 60_000);
        assert_eq!(recheck_delay(86_400_000, 86_399_000).as_millis(), 1000);
        assert_eq!(recheck_delay(86_400_000, 86_400_001).as_millis(), 1000);
    }
    #[tokio::test]
    async fn clearing_pause_cancels_its_only_timer() {
        let timer = tokio::spawn(std::future::pending::<()>());
        let abort = timer.abort_handle();
        state().insert(
            "test-source".into(),
            Entry {
                pause: Pause { reset: None },
                timer: Some(timer),
            },
        );
        assert!(snapshot("test-source").unwrap().reset.is_none());
        assert!(for_art("test-source", false).is_none());
        assert!(for_art("test-source", true).is_some());
        clear("test-source");
        tokio::task::yield_now().await;
        assert!(abort.is_finished());
        assert!(snapshot("test-source").is_none());
        assert!(is_pause("AI_IMAGE_QUOTA:1"));
        assert!(is_pause(PAUSED));
        assert!(!is_pause("story mentions AI_IMAGE_QUOTA:"));
    }
}
