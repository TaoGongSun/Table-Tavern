use crate::data::{CharacterCard, CharacterMeta};
use crate::{data, data_root, import, receipts};

#[tauri::command]
pub(crate) fn list_characters(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<Vec<CharacterMeta>, String> {
    data::list_characters(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn reorder_characters(
    app: tauri::AppHandle,
    world_id: String,
    ids: Vec<String>,
) -> Result<(), String> {
    data::reorder_characters(&data_root(&app)?, &world_id, &ids).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn read_character(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<CharacterCard, String> {
    data::read_character(&data_root(&app)?, &world_id, &character_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn write_character(
    app: tauri::AppHandle,
    world_id: String,
    card: CharacterCard,
) -> Result<(), String> {
    data::write_character(&data_root(&app)?, &world_id, &card).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn set_character_archived(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    archived: bool,
) -> Result<(), String> {
    data::set_character_archived(&data_root(&app)?, &world_id, &character_id, archived)
        .map_err(|error| error.to_string())
}

/// 玩家從隱藏區手動拉回自動隱藏的卡（或手動收進去）。玩家意志優先於自動結算，
/// 幕中按下快取代價玩家自付——與 set_character_archived 同款語意。
#[tauri::command]
pub(crate) fn set_character_auto_hidden(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    auto_hidden: bool,
) -> Result<(), String> {
    data::set_character_auto_hidden(&data_root(&app)?, &world_id, &character_id, auto_hidden)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn delete_character(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<(), String> {
    data::delete_character(&data_root(&app)?, &world_id, &character_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn import_character(
    app: tauri::AppHandle,
    world_id: String,
    data: Vec<u8>,
    color: String,
) -> Result<CharacterImport, String> {
    let root = data_root(&app)?;
    let before = receipts::snapshot(&root, &world_id);
    let entries_before = data::read_worldbook(&root, &world_id).map_or(0, |entries| entries.len());
    let meta = import::import_character(&root, &world_id, &data, &color)
        .map_err(|error| error.to_string())?;
    receipts::record_character_import(&root, &world_id, &meta.id, &meta.name, before);
    // 卡片隨身的世界書條目也要跟世界書路徑一樣回報進來幾條、重複跳過幾條
    let imported =
        data::read_worldbook(&root, &world_id).map_or(0, |entries| entries.len() - entries_before);
    let skipped = import::probe_import(&data).book_entries.saturating_sub(imported);
    Ok(CharacterImport {
        meta,
        book: data::WorldbookImport { imported, skipped },
    })
}

/// 角色卡匯入的完整結果：新角色本體＋卡片隨身世界書的收編數字。
#[derive(serde::Serialize)]
pub(crate) struct CharacterImport {
    meta: CharacterMeta,
    book: data::WorldbookImport,
}

#[tauri::command]
pub(crate) fn probe_import(data: Vec<u8>) -> Result<import::ImportProbe, String> {
    Ok(import::probe_import(&data))
}

/// 側欄按鈕判斷要不要顯示「復原上次匯入」；未來路由框也靠這份摘要判身分。
#[tauri::command]
pub(crate) fn list_import_receipts(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<Vec<receipts::ImportReceiptSummary>, String> {
    Ok(receipts::list_import_receipts(&data_root(&app)?, &world_id))
}

/// 逆向最後一筆匯入收據：刪角色、刪未經玩家修改的世界書條目、退回機制寫入與桌名。
#[tauri::command]
pub(crate) fn undo_last_import(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<receipts::UndoReport, String> {
    receipts::undo_last_import(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

/// adoptImportName 改名成功後呼叫：把舊桌名補進最後一筆收據，undo 才能把桌名退回去。
#[tauri::command]
pub(crate) fn record_import_rename(
    app: tauri::AppHandle,
    world_id: String,
    old_name: String,
) -> Result<(), String> {
    receipts::record_last_import_rename(&data_root(&app)?, &world_id, &old_name);
    Ok(())
}

// 存檔位置由前端的「另存新檔」對話框決定；副檔名決定 PNG 或 JSON
#[tauri::command]
pub(crate) fn export_character(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    path: String,
) -> Result<(), String> {
    import::export_character(
        &data_root(&app)?,
        &world_id,
        &character_id,
        std::path::Path::new(&path),
    )
    .map_err(|error| error.to_string())
}

/// 這一桌未封存、也沒被自動隱藏的角色卡（GM 上下文與 chars 續聊線的快照都要全卡）；
/// auto_hidden 的卡在別桌上場前先不進凍結快照，見 record_card_arrivals／load_hidden_cards。
pub(super) fn load_active_cards(
    root: &std::path::Path,
    world_id: &str,
) -> Result<Vec<data::CharacterCard>, String> {
    data::list_characters(root, world_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|meta| !meta.archived && !meta.auto_hidden)
        .map(|meta| {
            data::read_character(root, world_id, &meta.id).map_err(|error| error.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::load_active_cards;
    use crate::commands::{character_card, NEXT_TEMP_ID};
    use crate::data;
    use std::sync::atomic::Ordering;

    /// AI 卡重構包 4b：load_active_cards 濾掉 auto_hidden（跟既有的 archived 並列），
    /// 只有沒被隱藏、也沒被封存的卡才進 GM／chars 凍結快照。
    #[test]
    fn load_active_cards_filters_auto_hidden_and_archived() {
        let root = std::env::temp_dir().join(format!(
            "table-tavern-load-active-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let world_id = data::create_world(&root, "測試桌").unwrap();

        let visible = character_card(&data::new_id(), "在場");
        let hidden = character_card(&data::new_id(), "隱藏");
        let archived = character_card(&data::new_id(), "封存");
        data::write_character(&root, &world_id, &visible).unwrap();
        data::write_character(&root, &world_id, &hidden).unwrap();
        data::write_character(&root, &world_id, &archived).unwrap();
        data::set_character_auto_hidden(&root, &world_id, &hidden.id, true).unwrap();
        data::set_character_archived(&root, &world_id, &archived.id, true).unwrap();

        let active = load_active_cards(&root, &world_id).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, visible.id);

        std::fs::remove_dir_all(&root).unwrap();
    }
}
