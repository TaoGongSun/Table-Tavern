//! 聊天回合的 Tauri 邊界：接參數、轉呼叫 `chat_turn`。
use crate::chat_turn::{self, RelayReport};
use crate::inflight;

/// 單人桌的一條接力（旁白、接話、推進、點名；規則見 `chat_turn::relay`）。`relay_id` 由前端給，
/// 停止（`chat_abort`）與換桌（`chat_relay_release`）都認它。
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn chat_relay(
    app: tauri::AppHandle,
    world_id: String,
    relay_id: String,
    mode: String,
    card_id: Option<String>,
    narration_id: Option<String>,
    action_id: String,
    player_name: String,
    on_event: tauri::ipc::Channel<serde_json::Value>,
) -> Result<RelayReport, String> {
    // 房間模式期間劇情由房間執行端推進（它走 domain、不經這裡），桌面的鈕一律拒收
    crate::room::refuse_if_active(&world_id)?;
    chat_turn::chat_relay(
        app,
        world_id,
        relay_id,
        mode,
        card_id,
        narration_id,
        action_id,
        player_name,
        on_event,
    )
    .await
}

/// 換桌換幕：這條接力在途那段照常落完，之後不再接。
#[tauri::command]
pub(crate) fn chat_relay_release(world_id: String, relay_id: String) {
    chat_turn::release_relay(&world_id, &relay_id);
}

/// 中止這一輪對話。只打 `turn_id` 對得上的那一筆（含一條接力的 id：整條不再接、在途那段中止）；
/// 晚到的舊 id 不會波及下一輪，也不會打到重構。
#[tauri::command]
pub(crate) fn chat_abort(world_id: String, turn_id: String) {
    inflight::abort_turn(&world_id, &turn_id);
    chat_turn::interrupt_relay(&world_id, &turn_id);
}
