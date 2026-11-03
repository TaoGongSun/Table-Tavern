//! 房間模式的 Tauri 邊界：轉呼叫 `room`（多人房間 GM 執行端）。
use crate::{config_root, room};

#[tauri::command]
pub(crate) fn room_status(
    app: tauri::AppHandle,
    world_id: Option<String>,
) -> Result<room::Status, String> {
    Ok(room::status(&config_root(&app)?, world_id.as_deref()))
}

/// 開過房的桌：`[桌 id, 房間碼]`（大廳桌卡的「重開房間」）
#[tauri::command]
pub(crate) fn room_tables(app: tauri::AppHandle) -> Result<Vec<(String, String)>, String> {
    Ok(room::room_tables(&config_root(&app)?))
}

/// 開房／重開房間：這張桌有憑證就接回原房間，否則向 `server` 建新房
#[tauri::command]
pub(crate) async fn room_enter(
    app: tauri::AppHandle,
    world_id: String,
    server: String,
) -> Result<room::Status, String> {
    room::enter(room::AppEngine::shared(&app)?, world_id, server).await
}

/// 關房（暫停）：斷線、解除防休眠、還原單人桌；憑證保留，之後可重開
#[tauri::command]
pub(crate) async fn room_exit() -> Result<(), String> {
    room::exit().await
}

/// 發一張新的 grantHost 配對碼：`phone` 為真＝交給手機，否則＝桌面拿回房主
#[tauri::command]
pub(crate) fn room_pair_code(phone: bool) -> Result<(), String> {
    room::request_pair_code(phone)
}

/// 桌卡「結束並刪除房間」
#[tauri::command]
pub(crate) async fn room_close(app: tauri::AppHandle, world_id: String) -> Result<(), String> {
    room::close(room::AppEngine::shared(&app)?, world_id, false).await
}

/// 房主在房間畫面改了卡：重送公開名冊
#[tauri::command]
pub(crate) fn room_roster_refresh() -> Result<(), String> {
    room::refresh_roster()
}

/// 桌面成員連線的裝置碼與邀請連結
#[tauri::command]
pub(crate) fn room_identity() -> Result<room::Identity, String> {
    room::identity()
}

/// 房間畫面的 WebSocket（經 Rust 轉手）：開、送、關
#[tauri::command]
pub(crate) fn room_member_open(
    on_event: tauri::ipc::Channel<serde_json::Value>,
) -> Result<u32, String> {
    room::member_open(std::sync::Arc::new(move |event| {
        let _ = on_event.send(event);
    }))
}

#[tauri::command]
pub(crate) fn room_member_send(id: u32, text: String) -> Result<(), String> {
    room::member_send(id, text)
}

#[tauri::command]
pub(crate) fn room_member_close(id: u32, code: u16) {
    room::member_close(id, code)
}
