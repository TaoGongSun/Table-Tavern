use crate::data::{CharacterCard, CharacterMeta};
use crate::{config_root, data, data_root, import, receipts, transport};

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
    let _permit = data::world_write_permit(&world_id)?;
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
    let _permit = data::world_write_permit(&world_id)?;
    data::write_character(&data_root(&app)?, &world_id, &card).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn set_character_archived(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    archived: bool,
) -> Result<(), String> {
    let _permit = data::world_write_permit(&world_id)?;
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
    let _permit = data::world_write_permit(&world_id)?;
    data::set_character_auto_hidden(&data_root(&app)?, &world_id, &character_id, auto_hidden)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn delete_character(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<(), String> {
    let _permit = data::world_write_permit(&world_id)?;
    data::delete_character(&data_root(&app)?, &world_id, &character_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn import_character(
    app: tauri::AppHandle,
    world_id: String,
    data: Vec<u8>,
    color: String,
) -> Result<CharacterImport, String> {
    // 整段持整桌獨占：快照、匯入、記帳之間不讓別的寫入插進來
    let held = data::world_exclusive_async(&world_id).await?;
    let root = data_root(&app)?;
    let entries_before = data::read_worldbook(&root, &world_id).map_or(0, |entries| entries.len());
    // 段標照介面語系寫進卡片內文；設定讀不到就用預設語系，不擋匯入
    let config = data::read_config(&config_root(&app)?).unwrap_or_default();
    let lang = transport::ui_language(&config);
    let imported_card =
        import::import_character_file(&root, &world_id, &data, &color, &lang, &held)
            .map_err(|error| error.to_string())?;
    // 卡片隨身的世界書條目也要跟世界書路徑一樣回報進來幾條、重複跳過幾條
    let imported =
        data::read_worldbook(&root, &world_id).map_or(0, |entries| entries.len() - entries_before);
    let skipped = import::probe_import(&data)
        .book_entries
        .saturating_sub(imported);
    Ok(CharacterImport {
        meta: imported_card.value,
        book: data::WorldbookImport { imported, skipped },
        source: imported_card.source,
        image_dropped: imported_card.image_dropped,
    })
}

/// 匯入網頁存檔（桌檔契約 v1）：開一張新桌照存檔接著玩。失敗不留半桌；成功回新桌與要交給前端的卡片 storage。
#[tauri::command]
pub(crate) async fn import_web_save(
    app: tauri::AppHandle,
    data: Vec<u8>,
) -> Result<import::WebSaveImported, String> {
    let root = data_root(&app)?;
    let config = data::read_config(&config_root(&app)?).unwrap_or_default();
    let lang = transport::ui_language(&config);
    tokio::task::spawn_blocking(move || {
        import::import_web_save(&root, &data, &lang).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

/// 網頁存檔匯入完成（前端寫好卡片 storage、要進桌）：刪掉新桌的未確認記錄。
#[tauri::command]
pub(crate) fn confirm_web_save_import(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<(), String> {
    let _permit = data::world_write_permit(&world_id)?;
    import::confirm_web_save_import(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

/// 放棄還沒確認的網頁存檔匯入：照記錄撤回跨桌層再刪新桌（獨占在 import 層）。`reason` 是放棄的原因，
/// 清理沒做完時放進回報。
#[tauri::command]
pub(crate) async fn discard_web_save_import(
    app: tauri::AppHandle,
    world_id: String,
    reason: String,
) -> Result<(), String> {
    let root = data_root(&app)?;
    tokio::task::spawn_blocking(move || {
        import::discard_web_save_import(&root, &world_id, &reason)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

/// 角色卡匯入的完整結果：新角色本體＋卡片隨身世界書的收編數字＋這次匯入的原檔識別（貼開場白時帶回）。
#[derive(serde::Serialize)]
pub(crate) struct CharacterImport {
    meta: CharacterMeta,
    book: data::WorldbookImport,
    source: Option<String>,
    /// PNG 卡的圖救不回、沒存成（卡照常匯入）
    image_dropped: bool,
}

/// 角色編輯器儲存前先驗待存的圖：不合格整個儲存取消，什麼都不寫。
#[tauri::command]
pub(crate) fn check_character_image(data: Vec<u8>) -> Result<(), String> {
    import::check_character_image(&data).map_err(|error| error.to_string())
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
pub(crate) async fn undo_last_import(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<receipts::UndoReport, String> {
    let held = data::world_exclusive_async(&world_id).await?;
    receipts::undo_last_import(&data_root(&app)?, &world_id, &held)
        .map_err(|error| error.to_string())
}

/// adoptImportName 改名成功後呼叫：把舊桌名補進最後一筆收據，undo 才能把桌名退回去。
#[tauri::command]
pub(crate) async fn record_import_rename(
    app: tauri::AppHandle,
    world_id: String,
    old_name: String,
) -> Result<(), String> {
    let held = data::world_exclusive_async(&world_id).await?;
    receipts::record_last_import_rename(&data_root(&app)?, &world_id, &old_name, &held);
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
    crate::data::refuse_if_updating()?;
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
    crate::chat_assembly::active_cards(root, world_id)
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
