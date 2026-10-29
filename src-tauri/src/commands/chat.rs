//! 聊天回合的 Tauri 邊界：接參數、轉呼叫 `chat_turn`。
use crate::chat_turn::{self, ChatReply, GmNarration};
use crate::inflight;

/// 角色接話（流程見 `chat_turn::chat_with_character`）。
#[tauri::command]
pub(crate) async fn chat_with_character(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    turn_id: String,
    on_delta: tauri::ipc::Channel<String>,
    action_id: Option<String>,
) -> Result<ChatReply, String> {
    chat_turn::chat_with_character(app, world_id, character_id, turn_id, on_delta, action_id).await
}

/// GM 旁白＋點名（流程見 `chat_turn::gm_narrate`）。
#[tauri::command]
pub(crate) async fn gm_narrate(
    app: tauri::AppHandle,
    world_id: String,
    turn_id: String,
    on_delta: tauri::ipc::Channel<String>,
    action_id: Option<String>,
) -> Result<GmNarration, String> {
    chat_turn::gm_narrate(app, world_id, turn_id, on_delta, action_id).await
}

/// 中止這一輪對話。只打 `turn_id` 對得上的那一筆；晚到的舊 id 不會波及下一輪，也不會打到重構。
#[tauri::command]
pub(crate) fn chat_abort(world_id: String, turn_id: String) {
    inflight::abort_turn(&world_id, &turn_id);
}
