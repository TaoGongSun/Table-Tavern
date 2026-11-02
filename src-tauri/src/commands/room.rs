//! 房間模式的 Tauri 邊界：轉呼叫 `room`（多人房間 GM 執行端）。
use crate::{config_root, room};

#[tauri::command]
pub(crate) fn room_status(app: tauri::AppHandle) -> Result<room::Status, String> {
    Ok(room::status(&config_root(&app)?))
}

/// 進入房間模式：同桌有憑證就接回原房間，否則向 `server` 建新房
#[tauri::command]
pub(crate) async fn room_enter(
    app: tauri::AppHandle,
    world_id: String,
    server: String,
) -> Result<room::Status, String> {
    room::enter(room::AppEngine::shared(&app)?, world_id, server).await
}

/// 退出房間模式：斷線、解除防休眠、還原單人桌；憑證保留
#[tauri::command]
pub(crate) async fn room_exit() -> Result<(), String> {
    room::exit().await
}

/// 面板「重設房主」：發一張新的 grantHost 配對碼
#[tauri::command]
pub(crate) fn room_pair_code() -> Result<(), String> {
    room::request_pair_code()
}
