//! 回合串流的共用接線：增量緩衝、API 路徑與取消賽跑、劇情續聊線的 CLI 呼叫素材。
use crate::transport::dispatch::prepare_lane_call;
use crate::{data, inflight, lanes};
use tauri::Emitter;

pub(super) struct Spoken {
    pub(super) text: String,
    pub(super) aborted: bool,
}

/// 增量先寫進這支緩衝再送前端。中止贏的那個當下，緩衝裡就是玩家已經看到的字。
pub(super) fn push_delta(
    buffer: &std::sync::Mutex<String>,
    on_delta: &tauri::ipc::Channel<String>,
    delta: &str,
) {
    buffer.lock().expect("delta buffer").push_str(delta);
    let _ = on_delta.send(delta.to_owned());
}

fn take_delta(buffer: &std::sync::Mutex<String>) -> String {
    std::mem::take(&mut *buffer.lock().expect("delta buffer"))
}

/// API 路徑在呼叫處跟取消賽跑。完成的分支排前面：兩邊同時就緒時交回完整結果。
pub(super) async fn take_abort_or_finish(
    cancel: &mut inflight::CancelSignal,
    buffer: &std::sync::Mutex<String>,
    call: impl std::future::Future<Output = Result<String, String>>,
) -> Result<Spoken, String> {
    tokio::pin!(call);
    let finished = tokio::select! {
        biased;
        result = call.as_mut() => Some(result),
        _ = cancel.cancelled() => None,
    };
    match finished {
        Some(result) => Ok(Spoken {
            text: result?,
            aborted: false,
        }),
        None => {
            drop(call);
            Ok(Spoken {
                text: take_delta(buffer),
                aborted: true,
            })
        }
    }
}

/// 劇情續聊線（角色線、GM 線）的 CLI 呼叫素材：claude 快取釘 1 小時，
/// 有嘗試落在訂閱超額就通知前端一次（`claude-overage`，app 層只提示一次）。
pub(super) async fn prepare_play_lane_call(
    app: &tauri::AppHandle,
    config: &data::AppConfig,
    tier: data::Tier,
    provider: lanes::LaneProvider,
) -> Result<lanes::LaneCall, String> {
    let mut call = prepare_lane_call(app, config, tier, provider).await?;
    lanes::pin_lane_cache_ttl(&mut call);
    let app = app.clone();
    call.on_overage = Some(std::sync::Arc::new(
        move |observed: lanes::CacheWriteObserved| {
            let _ = app.emit(
                "claude-overage",
                serde_json::json!({
                    "eventId": ulid::Ulid::generate().to_string(),
                    "observed": observed.as_str(),
                }),
            );
        },
    ));
    Ok(call)
}
