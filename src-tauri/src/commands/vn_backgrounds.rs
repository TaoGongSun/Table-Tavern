//! VN 背景庫（plans/vn-mode-pkg3.md、vn-setup-wizard.md §3）的 Tauri 邊界：排入這幕缺的基底、串流中提早排、
//! 讀圖（專屬圖或基底）、狀態、重生成、生專屬時段圖。
use crate::generated_image::png_data_url;
use crate::vn_backgrounds::{self, Backdrop, Library, Places};
use crate::vn_sprites::queue::{self, BackgroundStatus, BgMode, BgSlot};
use crate::{chat_turn, data, data_root};
use serde::Serialize;

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

/// 串流中寫完的 `[bg]` 行一出現就排（前端只在串流進行中送；只看換行結尾的行）。去重同 `vn_backgrounds_fill`
#[tauri::command]
pub(crate) fn vn_backgrounds_prefetch(
    app: tauri::AppHandle,
    world_id: String,
    scene: u64,
    source: chat_turn::VnSource,
    text: String,
) -> Result<Vec<Backdrop>, String> {
    let root = data_root(&app)?;
    let roster = chat_turn::VnRoster::load(
        &root,
        &world_id,
        data::vn::read_card(&root, &world_id).as_ref(),
    )?;
    let (mut formal, _) = chat_turn::read_scene(&root, &world_id, scene)?;
    let used = vn_backgrounds::streamed(&mut formal, &text, &source, &roster);
    vn_backgrounds::fill(&app, &world_id, &used)?;
    Ok(used)
}

/// 舞台的一張背景：src＝data URL，period＝這張圖實際的時段（與要的不同＝基底，前端疊遮罩）
#[derive(Serialize)]
pub(crate) struct BackgroundView {
    src: String,
    period: String,
}

fn checked(
    root: &std::path::Path,
    world_id: &str,
    slug: &str,
    period: &str,
) -> Result<Vec<String>, String> {
    // 時段要在這個 slug 對應地點的有效清單內（卡地點用卡上的時段鍵）
    let periods = Places::load(root, world_id).periods(slug);
    match vn_backgrounds::valid_slug(slug) && periods.iter().any(|known| known == period) {
        true => Ok(periods),
        false => Err("VN_BACKGROUND_KEY".to_owned()),
    }
}

/// 一張背景：該時段專屬圖 → 同地點基底；地點還沒有任何圖回 None
#[tauri::command]
pub(crate) fn vn_background_read(
    app: tauri::AppHandle,
    world_id: String,
    slug: String,
    period: String,
) -> Result<Option<BackgroundView>, String> {
    let root = data_root(&app)?;
    let periods = checked(&root, &world_id, &slug, &period)?;
    let library = Library::of(&root, &world_id)?;
    Ok(library
        .shown(&slug, &period, &periods)
        .map(|shown| BackgroundView {
            src: png_data_url(&shown.png),
            period: shown.period,
        }))
}

#[tauri::command]
pub(crate) fn vn_backgrounds_status(world_id: String) -> Vec<BackgroundStatus> {
    queue::background_snapshot(&world_id)
}

/// 重生成一張背景（玩家按工具列：舞台目前顯示的那張，基底或專屬圖）：庫裡已有也覆寫
#[tauri::command]
pub(crate) fn vn_background_regenerate(
    app: tauri::AppHandle,
    world_id: String,
    location: String,
    period: String,
) -> Result<(), String> {
    enqueue(&app, &world_id, &location, &period, BgMode::Regenerate)
}

/// 生這個地點這個時段的專屬圖（以同地點既有的圖當參考改光線，蓋過遮罩；美術一覽用）
#[tauri::command]
pub(crate) fn vn_background_generate_period(
    app: tauri::AppHandle,
    world_id: String,
    location: String,
    period: String,
) -> Result<(), String> {
    enqueue(&app, &world_id, &location, &period, BgMode::Dedicated)
}

fn enqueue(
    app: &tauri::AppHandle,
    world_id: &str,
    location: &str,
    period: &str,
    mode: BgMode,
) -> Result<(), String> {
    let root = data_root(app)?;
    data::vn::refuse_if_replay(&root, world_id)?;
    let backdrop = Backdrop::new(location, None, Some(period), &Places::load(&root, world_id));
    // 時段不在該地點的有效清單內會被換成清單第一項：當成壞鍵拒收
    if location.trim().is_empty() || backdrop.period != period {
        return Err("VN_BACKGROUND_KEY".to_owned());
    }
    queue::regenerate_background(app, BgSlot::new(world_id, backdrop, mode));
    Ok(())
}
