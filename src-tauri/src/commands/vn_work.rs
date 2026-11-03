//! VN 作品包（plans/vn-mode-pkg8.md）的 Tauri 邊界：匯出、探測、匯入（路徑由前端的系統對話框給，Rust 自己讀，
//! 不經 IPC 傳整包）、重播桌換幕。
use crate::{config_root, data, data_root, transport, vn_work};
use std::path::PathBuf;

/// 匯出成作品包：存檔位置由前端另存新檔決定；`cover` 是桌上沒有封面時玩家挑的 PNG。
#[tauri::command]
pub(crate) async fn export_vn_work(
    app: tauri::AppHandle,
    world_id: String,
    path: String,
    cover: Option<String>,
    settings: bool,
) -> Result<vn_work::WorkExported, String> {
    let _permit = data::world_write_permit_async(&world_id).await?;
    let root = data_root(&app)?;
    let config = data::read_config(&config_root(&app)?).unwrap_or_default();
    let lang = transport::ui_language(&config);
    tokio::task::spawn_blocking(move || {
        vn_work::export_work(
            &root,
            &world_id,
            &PathBuf::from(path),
            cover.as_deref().map(std::path::Path::new),
            &lang,
            settings,
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

/// 只讀 manifest：匯入前給玩家確認標題與分級。
#[tauri::command]
pub(crate) async fn probe_vn_work(path: String) -> Result<vn_work::WorkProbe, String> {
    tokio::task::spawn_blocking(move || vn_work::probe_work(&PathBuf::from(path)))
        .await
        .map_err(|error| error.to_string())?
}

/// 匯入成重播桌或可續寫的桌（後者需包內附設定）：整包驗完才建桌，失敗不留半桌；回新桌與夾斷提示。
#[tauri::command]
pub(crate) async fn import_vn_work(
    app: tauri::AppHandle,
    path: String,
    mode: vn_work::ImportMode,
    spoiler_lock: bool,
) -> Result<vn_work::WorkImported, String> {
    let root = data_root(&app)?;
    tokio::task::spawn_blocking(move || {
        vn_work::import_work(&root, &PathBuf::from(path), mode, spoiler_lock)
    })
    .await
    .map_err(|error| error.to_string())?
}

/// 重播桌換到任一已發布幕（章頭、讀檔、選項點跳分支、章末接下一幕），回目標幕號。
#[tauri::command]
pub(crate) fn vn_replay_goto(
    app: tauri::AppHandle,
    world_id: String,
    scene: u64,
) -> Result<u64, String> {
    let _permit = data::world_write_permit(&world_id)?;
    let scene = data::replay_goto(&data_root(&app)?, &world_id, scene)
        .map_err(|error| error.to_string())?;
    crate::world_scan::macros::forget_outlets(&world_id);
    Ok(scene)
}
