use std::path::{Path, PathBuf};
use super::{DataResult, invalid_data};

/// 定址代碼格式：26 字 Crockford base32（ulid crate 輸出的格式），擋掉一切路徑逃逸。
/// 所有用 id 組路徑的地方都先過這關。
pub(crate) fn validate_id(id: &str) -> DataResult<()> {
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    if id.len() != 26 || !id.bytes().all(|byte| ALPHABET.contains(&byte)) {
        return Err(invalid_data(format!("invalid id: {id:?}")));
    }
    Ok(())
}

pub(crate) fn validate_single_line(field: &str, value: &str) -> DataResult<()> {
    if value.contains('\n') || value.contains('\r') {
        return Err(invalid_data(format!("{field} must be a single line")));
    }
    Ok(())
}

pub(super) fn worlds_dir(root: &Path) -> PathBuf {
    root.join("worlds")
}

pub(super) fn world_dir(root: &Path, world_id: &str) -> DataResult<PathBuf> {
    validate_id(world_id)?;
    Ok(worlds_dir(root).join(world_id))
}

pub(crate) fn character_path(
    root: &Path,
    world_id: &str,
    character_id: &str,
) -> DataResult<PathBuf> {
    validate_id(character_id)?;
    Ok(world_dir(root, world_id)?
        .join("characters")
        .join(format!("{character_id}.md")))
}

/// claude lane 續聊狀態檔（prompt-cache-optimization 包 2）：worlds/<world_id>/lanes.json。
/// 本機工具狀態，壞檔或缺檔都只是重開續聊線，不影響正典資料。
pub(crate) fn lanes_path(root: &Path, world_id: &str) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?.join("lanes.json"))
}

/// 機制記帳落檔：worlds/<world_id>/mechanism-log.jsonl。
pub(crate) fn mechanism_log_path(root: &Path, world_id: &str) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?.join("mechanism-log.jsonl"))
}

/// 匯入收據落檔：worlds/<world_id>/import-receipts.json（JSON 陣列，append）。
pub(crate) fn import_receipts_path(root: &Path, world_id: &str) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?.join("import-receipts.json"))
}

/// 世界書路徑匯入的原始卡檔：worlds/<world_id>/source-card.<png|import.json>。
/// 卡片自帶介面要靠它，角色卡路徑則是留在角色檔旁邊。
pub(crate) fn world_card_path(root: &Path, world_id: &str, extension: &str) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?.join(format!("source-card.{extension}")))
}

/// GM 卡的圖：worlds/<world_id>/gm.png。世界書匯入的若是 PNG 卡就存這張，側欄 GM 卡改用它
/// 取代內建書本圖；沒有這檔就回退書本圖。
pub(crate) fn gm_image_path(root: &Path, world_id: &str) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?.join("gm.png"))
}

/// 介面渲染殼檔：worlds/<world_id>/interface-shell.html。AI 卡重構展開介面規則時，除了狀態樹
/// 初始值（state_fields）還可能多產一份自包含 HTML 殼；前端拿狀態樹的值替換殼內 `{{路徑}}`
/// 佔位符後塞進既有卡片沙盒 iframe（interface-card.ts buildShellDocument，下一包串接）。
pub(crate) fn interface_shell_path(root: &Path, world_id: &str) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?.join("interface-shell.html"))
}

/// AI 卡重構產物存檔：worlds/<world_id>/refactor-outcome.json。套用成功後落一份完整產物，供
/// 玩家之後從世界書工具列直接匯出重玩，不必重燒 AI 額度重新展開同一張卡；二次套用直接覆寫。
pub(crate) fn refactor_outcome_path(root: &Path, world_id: &str) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?.join("refactor-outcome.json"))
}

/// 生成圖庫目錄，落在世界目錄內：worlds/<world_id>/gen-gallery/<character_id>。
pub(crate) fn gallery_dir(root: &Path, world_id: &str, character_id: &str) -> DataResult<PathBuf> {
    validate_id(character_id)?;
    Ok(world_dir(root, world_id)?
        .join("gen-gallery")
        .join(character_id))
}

#[cfg(test)]
mod tests {
    use crate::data::*;
    use crate::data::test_support::*;

    /// 測試清單 #8：顯示名放行含 /、..、開頭 .、保留字 GM；含換行仍擋
    #[test]
    fn display_names_allow_special_characters_but_reject_newlines() {
        let root = TestRoot::new("display-names");
        let world_id = create_world(root.path(), "世界").unwrap();
        for name in ["../evil", "a/b", ".hidden", "", "GM"] {
            let card = character_card(&new_id(), name);
            write_character(root.path(), &world_id, &card).unwrap();
            let read_back = read_character(root.path(), &world_id, &card.id).unwrap();
            assert_eq!(read_back.name, name);
        }

        let mut newline_card = character_card(&new_id(), "壞名字");
        newline_card.name = "含換行\n的名字".to_owned();
        assert!(write_character(root.path(), &world_id, &newline_card).is_err());

        // 世界名同樣只擋換行
        let odd_world = create_world(root.path(), "../also/fine").unwrap();
        assert_eq!(
            read_state(root.path(), &odd_world).unwrap().name,
            "../also/fine"
        );
    }

    /// 測試清單 #9：world_id／character_id 路徑逃逸一律被 validate_id 擋
    #[test]
    fn validate_id_rejects_path_escaping_ids() {
        let root = TestRoot::new("escape");
        for bad_id in ["../x", "a/b", "", "short", &"A".repeat(27)] {
            assert!(
                read_world_md(root.path(), bad_id).is_err(),
                "accepted world id {bad_id:?}"
            );
        }

        let world_id = create_world(root.path(), "世界").unwrap();
        for bad_id in ["../x", "a/b", "", "short"] {
            assert!(
                read_character(root.path(), &world_id, bad_id).is_err(),
                "accepted character id {bad_id:?}"
            );
            let mut card = character_card(&new_id(), "角色");
            card.id = bad_id.to_owned();
            assert!(
                write_character(root.path(), &world_id, &card).is_err(),
                "accepted character id {bad_id:?} on write"
            );
        }
    }

}
