use crate::data::CharacterMeta;
use crate::{config_root, data, data_root, import, receipts, transport};

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
    let _permit = data::world_write_permit(&world_id)?;
    data::reclaim_world_if_empty(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn delete_world(app: tauri::AppHandle, world_id: String) -> Result<(), String> {
    // 獨占在 data::delete_world（try_world_exclusive）。這裡不再拿共用鎖，避免跟自己搶、也避免跟在途寫入交錯。
    data::delete_world(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn rename_world(
    app: tauri::AppHandle,
    world_id: String,
    new_name: String,
) -> Result<(), String> {
    let _permit = data::world_write_permit(&world_id)?;
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
    let _permit = data::world_write_permit(&world_id)?;
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
    let _permit = data::world_write_permit(&world_id)?;
    data::upsert_worldbook_entry(&data_root(&app)?, &world_id, entry)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn reorder_worldbook_entries(
    app: tauri::AppHandle,
    world_id: String,
    uids: Vec<u64>,
) -> Result<(), String> {
    let _permit = data::world_write_permit(&world_id)?;
    data::reorder_worldbook_entries(&data_root(&app)?, &world_id, &uids)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn delete_worldbook_entry(
    app: tauri::AppHandle,
    world_id: String,
    uid: u64,
) -> Result<(), String> {
    let _permit = data::world_write_permit(&world_id)?;
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
    let _permit = data::world_write_permit(&world_id)?;
    data::worldbook_entry_to_character(&data_root(&app)?, &world_id, uid, color, as_player)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn character_to_worldbook_entry(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<receipts::CharacterDeleteOutcome, String> {
    // 獨占在 receipts::character_to_worldbook_entry_and_clean（try_world_exclusive），這裡不拿共用許可，免得跟自己搶。
    // 私有段標照介面語系寫進條目內文；設定讀不到就用預設語系，不擋轉換
    let config = data::read_config(&config_root(&app)?).unwrap_or_default();
    let lang = transport::ui_language(&config);
    receipts::character_to_worldbook_entry_and_clean(
        &data_root(&app)?,
        &world_id,
        &character_id,
        &lang,
    )
    .map_err(|error| error.to_string())
}

/// 狀態列是否顯示：沒有匯入狀態列規則的桌，整條狀態列不掛上去。
#[tauri::command]
pub(crate) fn world_has_state_bar(app: tauri::AppHandle, world_id: String) -> Result<bool, String> {
    data::world_has_state_bar(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

/// 整段持整桌獨占（排隊等在途寫入放開）：快照、匯入、記帳之間不讓別的寫入插進來。
#[tauri::command]
pub(crate) async fn import_worldbook(
    app: tauri::AppHandle,
    world_id: String,
    data: Vec<u8>,
    label: String,
) -> Result<WorldbookImportResult, String> {
    let held = data::world_exclusive_async(&world_id).await?;
    let root = data_root(&app)?;
    let imported = import::import_worldbook_file(&root, &world_id, &data, &label, &held)
        .map_err(|error| error.to_string())?;
    Ok(WorldbookImportResult {
        book: imported.value,
        source: imported.source,
        image_dropped: imported.image_dropped,
    })
}

/// 世界書匯入的結果：收編數字＋這次匯入的原檔識別（貼開場白時帶回，開場白才掛得到這筆匯入）。
#[derive(serde::Serialize)]
pub(crate) struct WorldbookImportResult {
    #[serde(flatten)]
    book: data::WorldbookImport,
    source: Option<String>,
    /// PNG 世界書卡的圖救不回、GM 圖沒存成
    image_dropped: bool,
}

/// 選項以中性求值換好巨集給玩家挑（不寫變數）；貼出時後端從原檔重新完整求值（`post_opening`）。
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
    let config = data::read_config(&crate::config_root(&app)?).unwrap_or_default();
    let path = crate::scene_budget::path_limits(
        &crate::config_root(&app)?,
        &root,
        &config,
        transport::gm_tier(&config),
    );
    let macros = crate::world_scan::opening::OpeningMacros::load(
        &root, &world_id, &name, &path, &lang, false,
    );
    Ok(openings
        .iter()
        .map(|opening| macros.display(opening))
        .collect())
}

#[tauri::command]
pub(crate) fn dedupe_worldbook(app: tauri::AppHandle, world_id: String) -> Result<usize, String> {
    let _permit = data::world_write_permit(&world_id)?;
    data::dedupe_worldbook(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn open_world(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<data::OpenWorld, String> {
    data::open_world(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn read_world_readonly(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<data::ReadonlyWorld, String> {
    data::read_world_readonly(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn restore_world_backup(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<data::OpenWorld, String> {
    data::restore_world_backup(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

// 存檔位置由前端的「另存新檔」對話框決定
#[tauri::command]
pub(crate) fn export_worldbook(
    app: tauri::AppHandle,
    world_id: String,
    path: String,
) -> Result<(), String> {
    crate::data::refuse_if_updating()?;
    data::export_worldbook(&data_root(&app)?, &world_id, std::path::Path::new(&path))
        .map_err(|error| error.to_string())
}
