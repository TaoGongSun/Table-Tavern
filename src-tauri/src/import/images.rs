use super::card_io::{base64_encode, PNG_MAGIC};
use crate::data::{self, DataResult};
use std::fs;
use std::path::Path;

/// 世界書匯入的檔案若是 PNG 卡，整張圖存成 GM 卡的圖（worlds/<world_id>/gm.png）；
/// 純 JSON 世界書不動舊圖——換書不該讓 GM 卡突然變回內建書本圖。
pub fn save_gm_image(root: &Path, world_id: &str, bytes: &[u8]) -> bool {
    if !bytes.starts_with(PNG_MAGIC) {
        return false;
    }
    let Ok(path) = data::gm_image_path(root, world_id) else {
        return false;
    };
    fs::write(path, bytes).is_ok()
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

fn save_character_png(
    root: &Path,
    world_id: &str,
    character_id: &str,
    bytes: &[u8],
    extension: &str,
) -> DataResult<()> {
    if !bytes.starts_with(PNG_MAGIC) {
        return Err(data::invalid_data("圖片必須是 PNG"));
    }
    let path = data::character_path(root, world_id, character_id)?;
    if !path.exists() {
        return Err(data::invalid_data(format!("角色 {character_id} 不存在")));
    }
    fs::write(path.with_extension(extension), bytes)?;
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
        fs::remove_file(path)?;
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
            "圖片必須是 PNG"
        );
        assert_eq!(
            save_character_avatar(root.path(), &world_id, &missing_id, PNG_MAGIC)
                .unwrap_err()
                .to_string(),
            format!("角色 {missing_id} 不存在")
        );
    }

    #[test]
    fn delete_character_images_is_idempotent() {
        let root = TestRoot::new("delete-images");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let png = minimal_png(r#"{"data":{"name":"凱恩"}}"#);
        let meta = import_character(root.path(), &world_id, &png, "#111111").unwrap();

        delete_character_image(root.path(), &world_id, &meta.id).unwrap();
        delete_character_image(root.path(), &world_id, &meta.id).unwrap();
        delete_character_avatar(root.path(), &world_id, &meta.id).unwrap();
        delete_character_avatar(root.path(), &world_id, &meta.id).unwrap();
    }
}
