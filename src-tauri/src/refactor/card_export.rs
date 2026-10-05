//! 重構卡匯出：副檔名 .json 寫對外封套、.png 寫重構卡 PNG（#2 只有封套；#3 再附角色圖），
//! 其他副檔名拒絕；含角色圖只收 .png。讀圖一律有上限，讀取中檔案變大也擋得住。
//! #3 的圖靠桌內存檔的套用映射找角色：各角色目前的全身圖＋裁切頭像，排除 gen-gallery；
//! 卡內容凍結為 outcome 的角色，套用後手動新增的角色不進卡。

use super::card_file::{parse_card, RefactorApplied, RefactorCardFile};
use super::card_png::{encode, AssetKind, CardAsset, CARD_LIMITS};
use super::types::RefactorOutcome;
use crate::data::{self, DataResult};
use crate::import::card_io::blank_png;
use crate::import::png_image::validate_png_image;
use crate::ui_msg::UiMsg;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

/// 桌內沒有重構卡存檔：前端比對這個字串顯示「這桌還沒有重構卡」。
pub const EXPORT_NONE: &str = "refactor-export-none";

#[derive(PartialEq)]
enum Target {
    Json,
    Png,
}

fn target(path: &Path) -> DataResult<Target> {
    let extension = path.extension().and_then(|extension| extension.to_str());
    match extension.map(str::to_ascii_lowercase).as_deref() {
        Some("json") => Ok(Target::Json),
        Some("png") => Ok(Target::Png),
        _ => Err(UiMsg::RefactorExportNeedPngOrJson.into_error()),
    }
}

/// 有上限的讀檔：最多讀 max＋1 位元組，超過回 None（讀取途中檔案變大一樣擋住，不先整份讀進來）。
fn read_capped(path: &Path, max: usize) -> std::io::Result<Option<Vec<u8>>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(max as u64 + 1)
        .read_to_end(&mut bytes)?;
    Ok((bytes.len() <= max).then_some(bytes))
}

fn usable_image(bytes: &[u8]) -> Result<(), String> {
    validate_png_image(bytes, CARD_LIMITS.pixels).map(|_| ())
}

/// 封面：GM 圖（匯入原卡的封面）→ 第一位已建卡角色的全身圖 → 1×1 透明圖；
/// 過不了完整驗證或超過單張上限的候選直接跳過。
fn pick_cover(root: &Path, world_id: &str, applied: Option<&RefactorApplied>) -> Vec<u8> {
    let mut candidates = Vec::new();
    if let Ok(path) = data::gm_image_path(root, world_id) {
        candidates.push(path);
    }
    for item in applied
        .map(|applied| applied.characters.as_slice())
        .unwrap_or(&[])
    {
        if let Some(path) = item
            .character_id
            .as_deref()
            .and_then(|id| data::character_path(root, world_id, id).ok())
        {
            candidates.push(path.with_extension("png"));
        }
    }
    candidates
        .into_iter()
        .filter(|path| path.is_file())
        .filter_map(|path| read_capped(&path, CARD_LIMITS.cover).ok().flatten())
        .find(|bytes| usable_image(bytes).is_ok())
        .unwrap_or_else(blank_png)
}

/// #3 素材：映射到的角色若已被刪就略過；任一張圖過不了驗證或超上限，整個匯出拒絕並點名角色。
fn collect_assets(
    root: &Path,
    world_id: &str,
    applied: &RefactorApplied,
) -> DataResult<Vec<CardAsset>> {
    let mut assets = Vec::new();
    let mut total = 0usize;
    for item in &applied.characters {
        let Some(id) = item.character_id.as_deref() else {
            continue;
        };
        let path = data::character_path(root, world_id, id)?;
        if !path.is_file() {
            continue;
        }
        for (kind, extension) in [
            (AssetKind::Portrait, "png"),
            (AssetKind::Avatar, "avatar.png"),
        ] {
            let image = path.with_extension(extension);
            if !image.is_file() {
                continue;
            }
            let checked = match read_capped(&image, CARD_LIMITS.image)? {
                Some(bytes) => usable_image(&bytes).map(|()| bytes),
                None => Err("image too large".to_owned()),
            };
            let bytes = match checked {
                Ok(bytes) => bytes,
                Err(detail) => {
                    let name = data::read_character(root, world_id, id)?.name;
                    return Err(UiMsg::RefactorExportImageInvalid { name, detail }.into_error());
                }
            };
            total += bytes.len();
            if total > CARD_LIMITS.assets_total {
                return Err(UiMsg::RefactorExportTooLarge.into_error());
            }
            assets.push(CardAsset {
                outcome_index: item.outcome_index,
                kind,
                bytes,
            });
        }
    }
    Ok(assets)
}

fn write_card(
    root: &Path,
    world_id: &str,
    card: &RefactorCardFile,
    assets: &[CardAsset],
    path: &Path,
) -> DataResult<u64> {
    let bytes = if target(path)? == Target::Json {
        serde_json::to_vec_pretty(&card.for_export())?
    } else {
        let cover = pick_cover(root, world_id, card.applied.as_ref());
        let size = cover.len() + assets.iter().map(|asset| asset.bytes.len()).sum::<usize>();
        if size > CARD_LIMITS.file - CARD_LIMITS.manifest {
            return Err(UiMsg::RefactorExportTooLarge.into_error());
        }
        encode(card, assets, &cover)?
    };
    // world-write-exempt: 寫到玩家選定的匯出路徑，不是桌目錄
    fs::write(path, &bytes)?;
    Ok(bytes.len() as u64)
}

/// 結果卡上還沒套用（或剛套用完）的產物：沒有映射，只出 .json 或 #2。
pub fn export_outcome(
    root: &Path,
    world_id: &str,
    outcome: &RefactorOutcome,
    path: &Path,
) -> DataResult<u64> {
    let card = RefactorCardFile::new(outcome.clone(), None);
    write_card(root, world_id, &card, &[], path)
}

/// 桌內存檔：.json／#2／#3（with_images）。含圖只收 PNG 路徑；舊裸存檔沒有映射附不了圖。
pub fn export_saved(
    root: &Path,
    world_id: &str,
    path: &Path,
    with_images: bool,
) -> DataResult<u64> {
    if with_images && target(path).ok() != Some(Target::Png) {
        return Err(UiMsg::RefactorExportImagesNeedPng.into_error());
    }
    target(path)?;
    let content = data::read_refactor_outcome(root, world_id)?
        .ok_or_else(|| data::invalid_data(EXPORT_NONE))?;
    let card = parse_card(&content)?;
    let assets = if with_images {
        let applied = card
            .applied
            .as_ref()
            .ok_or_else(|| UiMsg::RefactorExportNoMap.into_error())?;
        collect_assets(root, world_id, applied)?
    } else {
        Vec::new()
    };
    write_card(root, world_id, &card, &assets, path)
}

#[cfg(test)]
mod tests;
