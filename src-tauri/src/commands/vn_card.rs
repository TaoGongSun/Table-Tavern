//! 初始 VN 卡（plans/vn-mode-pkg9.md）的 Tauri 邊界：匯入建一張新 VN 桌、VN 桌匯出成卡。
use crate::{config_root, data, data_root, import, transport};

/// 匯入 VN 卡：整桌原子建好一張新 VN 桌（失敗不留半桌），回新桌與夾斷提示。
#[tauri::command]
pub(crate) async fn import_vn_card(
    app: tauri::AppHandle,
    data: Vec<u8>,
    spoiler_lock: Option<bool>,
) -> Result<import::VnCardImported, String> {
    let root = data_root(&app)?;
    tokio::task::spawn_blocking(move || {
        import::import_vn_card(&root, &data, spoiler_lock.unwrap_or(true))
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

/// 匯出成 VN 卡：存檔位置由前端的另存新檔決定，副檔名決定 PNG 或 JSON；`cover` 是桌上沒有封面時玩家挑的 PNG。
#[tauri::command]
pub(crate) fn export_vn_card(
    app: tauri::AppHandle,
    world_id: String,
    path: String,
    cover: Option<String>,
) -> Result<(), String> {
    let config = data::read_config(&config_root(&app)?).unwrap_or_default();
    import::export_vn_card(
        &data_root(&app)?,
        &world_id,
        std::path::Path::new(&path),
        cover.as_deref().map(std::path::Path::new),
        &transport::ui_language(&config),
    )
    .map_err(|error| error.to_string())
}
