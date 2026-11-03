//! VN 背景庫（plans/vn-mode-pkg3.md）的 Tauri 邊界：排入這幕缺的背景、讀圖、狀態、重生成。
use crate::generated_image::png_data_url;
use crate::vn_backgrounds::{self, Backdrop, Library};
use crate::vn_sprites::queue::{self, BackgroundStatus, BgSlot};
use crate::{chat_turn, data, data_root};

/// 排入這幕用到但庫裡沒有的背景（冪等；scene 沒給＝目前這幕）。回這幕用到的背景，依出現順序
#[tauri::command]
pub(crate) fn vn_backgrounds_fill(
    app: tauri::AppHandle,
    world_id: String,
    scene: Option<u64>,
) -> Result<Vec<Backdrop>, String> {
    let root = data_root(&app)?;
    let scene = match scene {
        Some(scene) => scene,
        None => {
            data::read_state(&root, &world_id)
                .map_err(|error| error.to_string())?
                .current_scene
        }
    };
    let (_, used) = chat_turn::read_scene(&root, &world_id, scene)?;
    vn_backgrounds::fill(&app, &world_id, &used)?;
    Ok(used)
}

/// 一張背景（data URL）；庫裡沒有回 None
#[tauri::command]
pub(crate) fn vn_background_read(
    app: tauri::AppHandle,
    world_id: String,
    slug: String,
    period: String,
) -> Result<Option<String>, String> {
    let period = vn_backgrounds::known_period(&period).ok_or("VN_BACKGROUND_KEY")?;
    if !vn_backgrounds::valid_slug(&slug) {
        return Err("VN_BACKGROUND_KEY".to_owned());
    }
    let library = Library::of(&data_root(&app)?, &world_id)?;
    Ok(library.read(&slug, period).map(|png| png_data_url(&png)))
}

#[tauri::command]
pub(crate) fn vn_backgrounds_status(world_id: String) -> Vec<BackgroundStatus> {
    queue::background_snapshot(&world_id)
}

/// 重生成一張背景（玩家按工具列）：庫裡已有也覆寫
#[tauri::command]
pub(crate) fn vn_background_regenerate(
    app: tauri::AppHandle,
    world_id: String,
    location: String,
    period: String,
) -> Result<(), String> {
    let period = vn_backgrounds::known_period(&period).ok_or("VN_BACKGROUND_KEY")?;
    if location.trim().is_empty() {
        return Err("VN_BACKGROUND_KEY".to_owned());
    }
    data::read_state(&data_root(&app)?, &world_id).map_err(|error| error.to_string())?;
    let backdrop = Backdrop::new(&location, None, Some(period));
    queue::regenerate_background(&app, BgSlot::new(&world_id, backdrop, true));
    Ok(())
}
