//! VN 立繪（plans/vn-mode-pkg7.md）的 Tauri 邊界：讀立繪、生候選、挑定、改框、重骰、上傳、背景補表情。
use crate::data::vn::CardOrigin;
use crate::data::vn_art::ArtEntry;
use crate::generated_image::{self, png_data_url, GeneratedImage};
use crate::vn_sprites::queue::{self, SlotStatus};
use crate::vn_sprites::store::{self, FaceBox, SpriteDir, SpriteMeta};
use crate::{data, data_root, vn_sprites};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
pub(crate) struct SpriteView {
    meta: SpriteMeta,
    width: u32,
    height: u32,
    /// 定稿（data URL）
    body: String,
    /// 已生好的臉層：表情 → data URL
    faces: BTreeMap<String, String>,
    /// 變體鍵 → 身體圖與獨立表情組的臉層
    variant_layers: BTreeMap<String, VariantLayers>,
    /// VN 卡宣告的額外表情，依卡上順序（臉層照 faces 的鍵）
    expressions: Vec<ArtLabel>,
    /// VN 卡宣告的變體（還沒生成的立繪分頁顯示成空格）
    declared_variants: Vec<ArtLabel>,
    /// 舞台身高比例＝本角色 height_cm ÷ 名單中有填者的最高值；沒填＝None（照統一比例）
    scale: Option<f64>,
}

#[derive(Serialize)]
pub(crate) struct ArtLabel {
    key: String,
    label: String,
}

fn labels(entries: &[ArtEntry]) -> Vec<ArtLabel> {
    entries
        .iter()
        .map(|entry| ArtLabel {
            key: entry.key.clone(),
            label: entry.label.clone(),
        })
        .collect()
}

#[derive(Serialize)]
pub(crate) struct VariantLayers {
    body: String,
    width: u32,
    height: u32,
    faces: BTreeMap<String, String>,
}

fn faces(dir: &SpriteDir, set: Option<&str>, keys: &[String]) -> BTreeMap<String, String> {
    keys.iter()
        .filter_map(|expression| {
            let png = std::fs::read(dir.face_path(set, expression)).ok()?;
            Some((expression.to_owned(), png_data_url(&png)))
        })
        .collect()
}

/// `card`＝這桌的 VN 卡原件（呼叫端讀一次）：表情清單、卡宣告變體與身高比例都從它取
fn read_view(dir: &SpriteDir, card: Option<&CardOrigin>, character: &str) -> Option<SpriteView> {
    let art = card
        .and_then(|card| card.cast.get(character))
        .map(|cast| cast.art.clone())
        .unwrap_or_default();
    let keys = &vn_sprites::face_list(&art);
    let store::Loaded {
        meta,
        size: (width, height),
        body,
    } = dir.read()?;
    let variant_layers = meta
        .variants
        .iter()
        .filter_map(|variant| {
            let (body, (width, height)) = dir.variant_body(&variant.key)?;
            let set = variant.face.is_some().then_some(variant.key.as_str());
            let faces = set
                .map(|set| faces(dir, Some(set), keys))
                .unwrap_or_default();
            let body = png_data_url(&body);
            Some((
                variant.key.clone(),
                VariantLayers {
                    body,
                    width,
                    height,
                    faces,
                },
            ))
        })
        .collect();
    Some(SpriteView {
        faces: faces(dir, None, keys),
        meta,
        width,
        height,
        body: png_data_url(&body),
        variant_layers,
        expressions: labels(&art.expressions),
        declared_variants: labels(&art.variants),
        scale: vn_sprites::height_scale(card, character),
    })
}

fn faces_of(app: &tauri::AppHandle, world: &str, character: &str) -> Result<Vec<String>, String> {
    Ok(vn_sprites::faces_of(&data_root(app)?, world, character))
}

/// 前端傳來的變體鍵先驗格式（路徑由鍵組成）
fn checked_key(key: Option<&str>) -> Result<(), String> {
    match key {
        Some(key) if !store::valid_variant_key(key) => Err("VN_SPRITE_VARIANT".to_owned()),
        _ => Ok(()),
    }
}

fn character_ids(root: &std::path::Path, world_id: &str) -> Result<Vec<String>, String> {
    Ok(data::list_characters(root, world_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|meta| meta.id)
        .collect())
}

/// 舞台用：這桌所有有定稿的角色
#[tauri::command]
pub(crate) fn vn_sprites_read_all(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<BTreeMap<String, SpriteView>, String> {
    let root = data_root(&app)?;
    let card = data::vn::read_card(&root, &world_id);
    Ok(character_ids(&root, &world_id)?
        .into_iter()
        .filter_map(|id| {
            let view = read_view(
                &SpriteDir::of(&root, &world_id, &id).ok()?,
                card.as_ref(),
                &id,
            )?;
            Some((id, view))
        })
        .collect())
}

#[derive(Serialize)]
pub(crate) struct SpriteState {
    sprite: Option<SpriteView>,
    /// 定稿候選檔名，新的在前
    candidates: Vec<String>,
}

#[tauri::command]
pub(crate) fn vn_sprite_read(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<SpriteState, String> {
    let root = data_root(&app)?;
    let dir = SpriteDir::of(&root, &world_id, &character_id)?;
    let card = data::vn::read_card(&root, &world_id);
    Ok(SpriteState {
        sprite: read_view(&dir, card.as_ref(), &character_id),
        candidates: dir.candidates(),
    })
}

#[tauri::command]
pub(crate) fn vn_sprite_candidate(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    file: String,
) -> Result<String, String> {
    let dir = SpriteDir::of(&data_root(&app)?, &world_id, &character_id)?;
    Ok(png_data_url(&dir.candidate_bytes(&file)?))
}

#[tauri::command]
pub(crate) async fn vn_sprite_generate(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<String, String> {
    crate::data::vn::refuse_if_replay(&data_root(&app)?, &world_id)?;
    vn_sprites::generate_candidate(&app, &world_id, &character_id).await
}

#[tauri::command]
pub(crate) async fn vn_sprite_pick(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    file: String,
) -> Result<(), String> {
    let dir = SpriteDir::of(&data_root(&app)?, &world_id, &character_id)?;
    let png = dir.candidate_bytes(&file)?;
    vn_sprites::install_base(&app, &world_id, &character_id, png, Some(&file)).await
}

/// 上傳替換：expression 為 None＝換定稿（不透明圖會先去背），否則換該組（variant＝獨立表情組）的表情臉層
#[tauri::command]
pub(crate) async fn vn_sprite_upload(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    variant: Option<String>,
    expression: Option<String>,
    data: Vec<u8>,
) -> Result<(), String> {
    checked_key(variant.as_deref())?;
    let png = generated_image::normalize(GeneratedImage::Bytes(data)).await?;
    match expression {
        None => vn_sprites::install_base(&app, &world_id, &character_id, png, None).await,
        Some(expression) if faces_of(&app, &world_id, &character_id)?.contains(&expression) => {
            let _permit = data::world_write_permit_async(&world_id).await?;
            vn_sprites::upload_face(
                &data_root(&app)?,
                &world_id,
                &character_id,
                variant.as_deref(),
                &expression,
                &png,
            )
        }
        Some(_) => Err("VN_SPRITE_EXPRESSION".to_owned()),
    }
}

#[tauri::command]
pub(crate) fn vn_sprite_set_face(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    variant: Option<String>,
    face: FaceBox,
) -> Result<(), String> {
    checked_key(variant.as_deref())?;
    let _permit = data::world_write_permit(&world_id)?;
    let faces = faces_of(&app, &world_id, &character_id)?;
    SpriteDir::of(&data_root(&app)?, &world_id, &character_id)?.set_face(
        variant.as_deref(),
        face,
        &faces,
    )?;
    queue::forget(&world_id, &character_id);
    Ok(())
}

/// 重骰一格表情：刪掉舊臉層，前端接著 fill 排入
#[tauri::command]
pub(crate) fn vn_sprite_reroll(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    variant: Option<String>,
    expression: String,
) -> Result<(), String> {
    crate::data::vn::refuse_if_replay(&data_root(&app)?, &world_id)?;
    if !faces_of(&app, &world_id, &character_id)?.contains(&expression) {
        return Err("VN_SPRITE_EXPRESSION".to_owned());
    }
    let set = variant.as_deref();
    checked_key(set)?;
    let _permit = data::world_write_permit(&world_id)?;
    SpriteDir::of(&data_root(&app)?, &world_id, &character_id)?.drop_face(set, &expression)?;
    queue::forget_slot(&world_id, &character_id, set, &expression);
    Ok(())
}

/// 刪整份立繪：舞台退回角色卡原圖
#[tauri::command]
pub(crate) fn vn_sprite_delete(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<(), String> {
    let _permit = data::world_write_permit(&world_id)?;
    vn_sprites::supersede_base(&world_id, &character_id);
    SpriteDir::of(&data_root(&app)?, &world_id, &character_id)?.remove_all()?;
    queue::forget(&world_id, &character_id);
    Ok(())
}

/// 背景補表情：排入這桌有定稿但缺的臉層（冪等；開演、進桌、立繪分頁變動後呼叫）
#[tauri::command]
pub(crate) fn vn_sprites_fill(app: tauri::AppHandle, world_id: String) -> Result<(), String> {
    let characters = character_ids(&data_root(&app)?, &world_id)?;
    queue::fill(&app, &world_id, &characters);
    Ok(())
}

/// 自動定稿（plans/vn-setup-wizard.md §2 ③）：沒定稿的角色在背景生候選並裝成定稿（冪等）。
/// `adopt`＝按開始時正在再骰的角色：再骰完用新圖
#[tauri::command]
pub(crate) fn vn_sprites_prepare(
    app: tauri::AppHandle,
    world_id: String,
    adopt: Vec<String>,
) -> Result<(), String> {
    vn_sprites::prepare::start(&app, &world_id, &adopt)
}

#[tauri::command]
pub(crate) fn vn_sprites_status(world_id: String) -> Vec<SlotStatus> {
    let mut list = queue::snapshot(&world_id);
    list.extend(vn_sprites::prepare::snapshot(&world_id));
    list
}

/// 新增或重骰變體（key 有值＝重骰那個；label 就是服裝／姿態描述）
#[tauri::command]
pub(crate) async fn vn_sprite_variant_generate(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    key: Option<String>,
    label: String,
) -> Result<(), String> {
    crate::data::vn::refuse_if_replay(&data_root(&app)?, &world_id)?;
    checked_key(key.as_deref())?;
    vn_sprites::generate_variant(&app, &world_id, &character_id, key, label.trim().to_owned()).await
}

/// 上傳變體身體（不透明會先去背；一律獨立表情組）
#[tauri::command]
pub(crate) async fn vn_sprite_variant_upload(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    key: Option<String>,
    label: String,
    data: Vec<u8>,
) -> Result<(), String> {
    checked_key(key.as_deref())?;
    let png = generated_image::normalize(GeneratedImage::Bytes(data)).await?;
    let label = label.trim().to_owned();
    vn_sprites::upload_variant(&app, &world_id, &character_id, key, label, png).await
}

#[tauri::command]
pub(crate) fn vn_sprite_variant_delete(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    key: String,
) -> Result<(), String> {
    checked_key(Some(&key))?;
    let _permit = data::world_write_permit(&world_id)?;
    vn_sprites::delete_variant(&data_root(&app)?, &world_id, &character_id, &key)
}
