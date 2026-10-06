use super::card_io::{base64_encode, png_invalid, PNG_MAGIC};
use super::png_clean;
use super::png_image::{validate_png_image, STORED_IMAGE_LIMITS};
use crate::data::{self, DataResult};
use crate::ui_msg::UiMsg;
use std::fs;
use std::path::Path;

/// GM 卡的圖這次匯入的結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GmImage {
    Saved,
    /// 純 JSON 世界書：不動舊圖
    NotPng,
    /// PNG 但圖救不回：不動舊圖，匯入結果提示
    Dropped,
}

/// 世界書匯入的檔案若是 PNG 卡，圖存成 GM 卡的圖（worlds/<world_id>/gm.png）；
/// 純 JSON 世界書不動舊圖——換書不該讓 GM 卡突然變回內建書本圖。
/// 圖走救圖版（截到 IEND、剝動畫、重編），只存像素，不搬卡文字；原子寫，寫檔失敗回錯，不當成略過。
pub fn save_gm_image(root: &Path, world_id: &str, bytes: &[u8]) -> DataResult<GmImage> {
    if !bytes.starts_with(PNG_MAGIC) {
        return Ok(GmImage::NotPng);
    }
    let Some(stored) = png_clean::salvage_stored_png(bytes) else {
        return Ok(GmImage::Dropped);
    };
    let path = data::gm_image_path(root, world_id)?;
    data::commit_world_write_atomic(&path, stored.bytes(bytes))?;
    Ok(GmImage::Saved)
}

/// GM 卡的圖；沒有回 None，前端拿 base64 組 data URL 顯示，比照 character_image
pub fn gm_image(root: &Path, world_id: &str) -> DataResult<Option<String>> {
    let path = data::gm_image_path(root, world_id)?;
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(base64_encode(&fs::read(path)?)))
}

/// 匯入時存下的原 PNG（characters/<id>.png）；沒有圖回 None，前端拿 base64 組 data URL 顯示
pub fn character_image(
    root: &Path,
    world_id: &str,
    character_id: &str,
) -> DataResult<Option<String>> {
    let path = data::character_path(root, world_id, character_id)?.with_extension("png");
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(base64_encode(&fs::read(path)?)))
}

pub fn save_character_image(
    root: &Path,
    world_id: &str,
    character_id: &str,
    bytes: &[u8],
) -> DataResult<()> {
    save_character_png(root, world_id, character_id, bytes, "png")
}

pub fn delete_character_image(root: &Path, world_id: &str, character_id: &str) -> DataResult<()> {
    delete_character_png(root, world_id, character_id, "png")
}

pub fn character_avatar(
    root: &Path,
    world_id: &str,
    character_id: &str,
) -> DataResult<Option<String>> {
    let path = data::character_path(root, world_id, character_id)?.with_extension("avatar.png");
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(base64_encode(&fs::read(path)?)))
}

pub fn save_character_avatar(
    root: &Path,
    world_id: &str,
    character_id: &str,
    bytes: &[u8],
) -> DataResult<()> {
    save_character_png(root, world_id, character_id, bytes, "avatar.png")
}

pub fn delete_character_avatar(root: &Path, world_id: &str, character_id: &str) -> DataResult<()> {
    delete_character_png(root, world_id, character_id, "avatar.png")
}

/// 角色圖／頭像要存之前的嚴驗（不寫檔）：編輯器儲存前先跑，不合格整個儲存取消。
pub fn check_character_image(bytes: &[u8]) -> DataResult<()> {
    if !bytes.starts_with(PNG_MAGIC) {
        return Err(UiMsg::ImageNotPng.into_error());
    }
    validate_png_image(bytes, STORED_IMAGE_LIMITS).map_err(|detail| png_invalid(&detail))?;
    Ok(())
}

fn save_character_png(
    root: &Path,
    world_id: &str,
    character_id: &str,
    bytes: &[u8],
    extension: &str,
) -> DataResult<()> {
    check_character_image(bytes)?;
    let path = data::character_path(root, world_id, character_id)?;
    if !path.exists() {
        return Err(UiMsg::CharacterNotFound {
            id: character_id.to_owned(),
        }
        .into_error());
    }
    // 原子寫：寫一半失敗時舊圖原樣留著，不會留下半張圖
    data::commit_world_write_atomic(&path.with_extension(extension), bytes)?;
    Ok(())
}

fn delete_character_png(
    root: &Path,
    world_id: &str,
    character_id: &str,
    extension: &str,
) -> DataResult<()> {
    let path = data::character_path(root, world_id, character_id)?.with_extension(extension);
    if path.exists() {
        data::commit_world_remove(&path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data;
    use crate::import::import_character;
    use crate::import::test_support::{minimal_png, TestRoot};

    #[test]
    fn save_character_images_reject_invalid_png_and_missing_character() {
        let root = TestRoot::new("reject-images");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let missing_id = data::new_id();

        assert_eq!(
            save_character_image(root.path(), &world_id, &missing_id, b"not png")
                .unwrap_err()
                .to_string(),
            UiMsg::ImageNotPng.to_string()
        );
        assert_eq!(
            save_character_avatar(
                root.path(),
                &world_id,
                &missing_id,
                &crate::import::png_image::test_png::real_png(2, 2)
            )
            .unwrap_err()
            .to_string(),
            UiMsg::CharacterNotFound { id: missing_id }.to_string()
        );
    }

    /// 手動上傳的防線：結構壞的 PNG 回 PngInvalid、不寫檔；預驗只驗不寫。
    #[test]
    fn broken_png_is_refused_and_check_writes_nothing() {
        use crate::import::png_clean::tests::{break_crc, with_trailing};
        use crate::import::png_image::test_png::real_png;
        let root = TestRoot::new("strict-images");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let png = minimal_png(r#"{"data":{"name":"凱恩"}}"#);
        let meta = import_character(root.path(), &world_id, &png, "#111111", "zh-TW").unwrap();
        let image_path = data::character_path(root.path(), &world_id, &meta.id)
            .unwrap()
            .with_extension("png");
        assert!(!image_path.exists());
        for broken in [
            break_crc(&real_png(4, 3), b"IDAT"),
            with_trailing(real_png(4, 3)),
            real_png(8193, 1),
        ] {
            let error = save_character_image(root.path(), &world_id, &meta.id, &broken)
                .unwrap_err()
                .to_string();
            assert!(error.contains("\"code\":\"png_invalid\""), "{error}");
            assert!(check_character_image(&broken).is_err());
            assert!(!image_path.exists());
        }
        assert_eq!(
            check_character_image(b"not png").unwrap_err().to_string(),
            UiMsg::ImageNotPng.to_string()
        );
        check_character_image(&real_png(4, 3)).unwrap();
        assert!(!image_path.exists());
    }

    #[test]
    fn delete_character_images_is_idempotent() {
        let root = TestRoot::new("delete-images");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let png = minimal_png(r#"{"data":{"name":"凱恩"}}"#);
        let meta = import_character(root.path(), &world_id, &png, "#111111", "zh-TW").unwrap();

        delete_character_image(root.path(), &world_id, &meta.id).unwrap();
        delete_character_image(root.path(), &world_id, &meta.id).unwrap();
        delete_character_avatar(root.path(), &world_id, &meta.id).unwrap();
        delete_character_avatar(root.path(), &world_id, &meta.id).unwrap();
    }
}
