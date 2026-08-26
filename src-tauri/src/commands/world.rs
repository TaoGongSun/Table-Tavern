use crate::data::CharacterMeta;
use crate::{data, data_root, import, receipts, transport};

#[tauri::command]
pub(crate) fn list_worlds(app: tauri::AppHandle) -> Result<Vec<data::WorldMeta>, String> {
    data::list_worlds(&data_root(&app)?).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn create_world(app: tauri::AppHandle, name: String) -> Result<String, String> {
    data::create_world(&data_root(&app)?, &name).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn create_sample_world(app: tauri::AppHandle, lang: String) -> Result<String, String> {
    data::create_sample_world(&data_root(&app)?, &lang).map_err(|error| error.to_string())
}

/// 前端建立新世界／新角色前先要一個代碼：草稿期生圖就能落在正確的路徑，存檔用同一個 id
#[tauri::command]
pub(crate) fn new_id() -> String {
    data::new_id()
}

#[tauri::command]
pub(crate) fn reclaim_world_if_empty(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<bool, String> {
    data::reclaim_world_if_empty(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn delete_world(app: tauri::AppHandle, world_id: String) -> Result<(), String> {
    data::delete_world(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn rename_world(
    app: tauri::AppHandle,
    world_id: String,
    new_name: String,
) -> Result<(), String> {
    data::rename_world(&data_root(&app)?, &world_id, &new_name).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn read_world_md(app: tauri::AppHandle, world_id: String) -> Result<String, String> {
    data::read_world_md(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn write_world_md(
    app: tauri::AppHandle,
    world_id: String,
    content: String,
) -> Result<(), String> {
    data::write_world_md(&data_root(&app)?, &world_id, &content).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn read_worldbook(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<Vec<data::WorldbookEntry>, String> {
    data::read_worldbook(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn upsert_worldbook_entry(
    app: tauri::AppHandle,
    world_id: String,
    entry: data::WorldbookEntry,
) -> Result<u64, String> {
    data::upsert_worldbook_entry(&data_root(&app)?, &world_id, entry)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn reorder_worldbook_entries(
    app: tauri::AppHandle,
    world_id: String,
    uids: Vec<u64>,
) -> Result<(), String> {
    data::reorder_worldbook_entries(&data_root(&app)?, &world_id, &uids)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn delete_worldbook_entry(
    app: tauri::AppHandle,
    world_id: String,
    uid: u64,
) -> Result<(), String> {
    data::delete_worldbook_entry(&data_root(&app)?, &world_id, uid)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn worldbook_entry_to_character(
    app: tauri::AppHandle,
    world_id: String,
    uid: u64,
    color: String,
    as_player: bool,
) -> Result<CharacterMeta, String> {
    data::worldbook_entry_to_character(&data_root(&app)?, &world_id, uid, color, as_player)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn character_to_worldbook_entry(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<(), String> {
    data::character_to_worldbook_entry(&data_root(&app)?, &world_id, &character_id)
        .map_err(|error| error.to_string())
}

/// 狀態列是否顯示：沒有匯入狀態列規則的桌，整條狀態列不掛上去。
#[tauri::command]
pub(crate) fn world_has_state_bar(app: tauri::AppHandle, world_id: String) -> Result<bool, String> {
    data::world_has_state_bar(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn import_worldbook(
    app: tauri::AppHandle,
    world_id: String,
    data: Vec<u8>,
    label: String,
) -> Result<data::WorldbookImport, String> {
    let json_text = import::worldbook_json(&data).map_err(|error| error.to_string())?;
    let root = data_root(&app)?;
    let before = receipts::snapshot(&root, &world_id);
    let result =
        data::import_worldbook(&root, &world_id, &json_text).map_err(|error| error.to_string())?;
    import::save_world_card(&root, &world_id, &data);
    import::save_gm_image(&root, &world_id, &data);
    if let Ok(book) = serde_json::from_str(&json_text) {
        import::import_mechanism(&root, &world_id, &book);
    }
    import::import_card_extension(&root, &world_id, &label, &data);
    receipts::record_worldbook_import(&root, &world_id, &label, before);
    Ok(result)
}

/// 選項要先換成當桌實名，前端貼入逐字稿時才不會留下卡片巨集。
#[tauri::command]
pub(crate) fn card_openings(
    app: tauri::AppHandle,
    world_id: String,
    data: Vec<u8>,
    lang: String,
) -> Result<Vec<String>, String> {
    let Some((name, openings)) = import::card_openings(&data) else {
        return Ok(Vec::new());
    };
    let root = data_root(&app)?;
    let player = data::read_player_card(&root, &world_id).map_err(|error| error.to_string())?;
    Ok(openings
        .iter()
        .map(|opening| {
            transport::resolve_display_macros(
                opening,
                player.as_ref().map(|card| card.name.as_str()),
                &name,
                &lang,
            )
        })
        .collect())
}

#[tauri::command]
pub(crate) fn dedupe_worldbook(app: tauri::AppHandle, world_id: String) -> Result<usize, String> {
    data::dedupe_worldbook(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

// 存檔位置由前端的「另存新檔」對話框決定
#[tauri::command]
pub(crate) fn export_worldbook(
    app: tauri::AppHandle,
    world_id: String,
    path: String,
) -> Result<(), String> {
    data::export_worldbook(&data_root(&app)?, &world_id, std::path::Path::new(&path))
        .map_err(|error| error.to_string())
}
