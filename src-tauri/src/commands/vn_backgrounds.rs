//! VN 背景庫（plans/vn-mode-pkg3.md、vn-setup-wizard.md §3）的 Tauri 邊界：排入這幕缺的基底、串流中提早排、
//! 讀圖（專屬圖或基底）、狀態、重生成、生專屬時段圖；美術一覽的地點清單與上傳替換。
use crate::generated_image::{self, png_data_url, GeneratedImage};
use crate::vn_backgrounds::{self, gallery, Backdrop, Library, Places};
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
    gallery::checked(&Places::load(root, world_id), slug, period)
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

/// 美術一覽：整桌地點、各自的時段清單、基底與有專屬圖的時段（含佇列中、失敗的地點；狀態另走 `vn_backgrounds_status`）
#[tauri::command]
pub(crate) fn vn_backgrounds_list(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<Vec<gallery::Place>, String> {
    let root = data_root(&app)?;
    let pending = queue::background_jobs(&world_id);
    Ok(Library::of(&root, &world_id)?.places(&Places::load(&root, &world_id), &pending))
}

/// 上傳替換一張背景：解碼重編 PNG（不去背、不管比例），持寫入許可重驗後原子寫入，發 done 讓舞台重讀；
/// 地點還缺基底就接著排基底（時段清單第一項）
#[tauri::command]
pub(crate) async fn vn_background_upload(
    app: tauri::AppHandle,
    world_id: String,
    slug: String,
    period: String,
    data: Vec<u8>,
) -> Result<(), String> {
    let root = data_root(&app)?;
    data::vn::refuse_if_replay(&root, &world_id)?;
    checked(&root, &world_id, &slug, &period)?;
    let png = generated_image::normalize(GeneratedImage::Bytes(data)).await?;
    let backdrop = {
        let _permit = data::world_write_permit_async(&world_id).await?;
        let places = Places::load(&root, &world_id);
        let pending = queue::background_jobs(&world_id);
        gallery::commit_upload(&root, &world_id, &places, &pending, &slug, &period, &png)?
    };
    let slot = BgSlot::new(&world_id, backdrop.clone(), BgMode::Dedicated);
    queue::announce_background(&app, &slot, "done");
    vn_backgrounds::fill(&app, &world_id, &[backdrop])
}

/// 重生成一張背景（美術一覽：那一格實際的圖，基底或專屬圖；失敗的重試）：庫裡已有也覆寫
#[tauri::command]
pub(crate) fn vn_background_regenerate(
    app: tauri::AppHandle,
    world_id: String,
    slug: String,
    period: String,
) -> Result<(), String> {
    enqueue(&app, &world_id, &slug, &period, BgMode::Regenerate)
}

/// 生這個地點這個時段的專屬圖（以同地點既有的圖當參考改光線，蓋過遮罩；美術一覽用）
#[tauri::command]
pub(crate) fn vn_background_generate_period(
    app: tauri::AppHandle,
    world_id: String,
    slug: String,
    period: String,
) -> Result<(), String> {
    enqueue(&app, &world_id, &slug, &period, BgMode::Dedicated)
}

/// 取消排隊中的一張背景（在跑的那張不動）
#[tauri::command]
pub(crate) fn vn_background_cancel(
    app: tauri::AppHandle,
    world_id: String,
    slug: String,
    period: String,
) -> Result<(), String> {
    let root = data_root(&app)?;
    data::vn::refuse_if_replay(&root, &world_id)?;
    checked(&root, &world_id, &slug, &period)?;
    queue::cancel_background(&world_id, &slug, &period);
    let backdrop = Backdrop {
        location: String::new(),
        slug,
        period,
    };
    queue::announce_background(
        &app,
        &BgSlot::new(&world_id, backdrop, BgMode::Dedicated),
        "cancelled",
    );
    Ok(())
}

fn enqueue(
    app: &tauri::AppHandle,
    world_id: &str,
    slug: &str,
    period: &str,
    mode: BgMode,
) -> Result<(), String> {
    let root = data_root(app)?;
    data::vn::refuse_if_replay(&root, world_id)?;
    let pending = queue::background_jobs(world_id);
    let backdrop = Library::of(&root, world_id)?.target(
        &Places::load(&root, world_id),
        &pending,
        slug,
        period,
    )?;
    queue::regenerate_background(app, BgSlot::new(world_id, backdrop, mode));
    Ok(())
}
