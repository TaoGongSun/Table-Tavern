//! 卡片變數（MVU message 層）的 Tauri 邊界：沙盒卡寫經宿主佇列送到這裡（計畫 8.5、8.8）。
//! 規則都在 `data::message_vars`，這裡只接參數、驗上限、整理回傳。
use crate::{config_root, data, data_root, transport};
use serde::Serialize;

/// 宿主組沙盒快照與送卡寫要的這桌現況。
#[derive(Serialize)]
pub(crate) struct CardVarsState {
    /// 桌世代：寫入都帶著它，整桌交換／還原後舊值一律 stale
    generation: u64,
    /// 目前這一幕走卡片變數（逐樓表）；false＝狀態樹是權威（還沒啟用、重構接管）
    active: bool,
    scene: u64,
}

#[tauri::command]
pub(crate) fn card_vars_state(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<CardVarsState, String> {
    let root = data_root(&app)?;
    data::state_commit::with_commit(&root, &world_id, |tx| -> data::DataResult<CardVarsState> {
        let scene = data::read_state(tx.root, tx.world_id)?.current_scene;
        Ok(CardVarsState {
            generation: data::message_vars::generation(tx),
            active: data::message_vars::read_control(tx.root, tx.world_id)?
                .active(scene)
                .is_some(),
            scene,
        })
    })
    .map_err(|error| error.to_string())
}

/// 卡寫一樓的整張表（JSON 文字，保留鍵順序）。上限不符整批拒絕；目標的世代、場、事件、版本不符回
/// stale 附權威值；GM 回合進行中回 busy。
#[tauri::command]
pub(crate) fn card_vars_write(
    app: tauri::AppHandle,
    world_id: String,
    generation: u64,
    scene: u64,
    target: data::message_vars::CardWriteTarget,
    expected_rev: Option<String>,
    vars_json: String,
) -> Result<data::message_vars::CardWrite, String> {
    let _permit = data::world_write_permit(&world_id)?;
    let root = data_root(&app)?;
    let table = match data::message_vars::parse_table(&vars_json) {
        Ok(table) => table,
        Err(limit) => {
            return Ok(data::message_vars::CardWrite::Rejected {
                code: limit.code.to_owned(),
                found: false,
                rev: None,
                table: None,
            })
        }
    };
    let macros = card_macros(&app, &root, &world_id)?;
    data::state_commit::with_commit(&root, &world_id, |tx| {
        data::message_vars::card_write(
            tx,
            Some(&macros),
            generation,
            scene,
            &target,
            expected_rev.as_deref(),
            &table,
        )
    })
    .map_err(|error| error.to_string())
}

/// 卡片變數的巨集代換值：同 GM 回合（`{{user}}`＝玩家卡名，沒有就照介面語系的稱呼；`{{char}}` 由下游決定）
fn card_macros(
    app: &tauri::AppHandle,
    root: &std::path::Path,
    world_id: &str,
) -> Result<data::message_vars::Macros, String> {
    let config = data::read_config(&config_root(app)?).unwrap_or_default();
    let lang = transport::ui_language(&config);
    let user = data::read_player_card(root, world_id)
        .ok()
        .flatten()
        .map(|card| card.name)
        .unwrap_or_else(|| transport::player_fallback_name(&lang).to_owned());
    Ok(data::message_vars::Macros { user, char: None })
}

/// `card_vars_parse` 的回傳：`zod` false＝確認不是 zod 桌（沙盒自己跑）；true 時 `table` 是結果表的 JSON 文字（保留鍵序）
#[derive(Serialize)]
pub(crate) struct CardParse {
    zod: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    table: Option<String>,
}

/// 卡片前端自己呼叫 parseMessage（card-shim-no-zod）：zod 桌交同一台 zod 引擎只算不寫（不碰 state、不記帳）。
/// 判定失敗或整次失敗回 Err，沙盒據以 reject。
#[tauri::command]
pub(crate) async fn card_vars_parse(
    app: tauri::AppHandle,
    world_id: String,
    message: String,
    old_json: String,
) -> Result<CardParse, String> {
    let root = data_root(&app)?;
    let macros = card_macros(&app, &root, &world_id)?;
    let table = tokio::task::spawn_blocking(move || {
        crate::mvu_engine::wiring::card_parse(&root, &world_id, &message, &old_json, &macros)
    })
    .await
    .map_err(|error| format!("crashed: {error}"))??;
    Ok(CardParse {
        zod: table.is_some(),
        table,
    })
}

/// 設定／清掉玩家卡：只改 `player_card_id` 那一欄（取代前端讀整份 state 再整份寫回）。
#[tauri::command]
pub(crate) fn set_player_card(
    app: tauri::AppHandle,
    world_id: String,
    card_id: Option<String>,
) -> Result<(), String> {
    let _permit = data::world_write_permit(&world_id)?;
    data::set_player_card(&data_root(&app)?, &world_id, card_id).map_err(|error| error.to_string())
}

/// 面板掛載時讀回非 message 層的現況（chat、這張卡的 character、global、preset、全部 script 與 extension）。
#[tauri::command]
pub(crate) fn card_layers(
    app: tauri::AppHandle,
    world_id: String,
    character_id: Option<String>,
) -> Result<Vec<data::card_vars::LayerEntry>, String> {
    let root = data_root(&app)?;
    data::card_vars::load_layers(&root, &world_id, character_id.as_deref())
        .map_err(|error| error.to_string())
}

/// 寫非 message 層的整張表：`expected_rev` 與檔案現況不符回 stale 附權威值；桌內的層另核對桌世代。
#[tauri::command]
pub(crate) fn card_layer_write(
    app: tauri::AppHandle,
    world_id: String,
    layer: data::card_vars::Layer,
    id: Option<String>,
    generation: u64,
    expected_rev: Option<String>,
    vars_json: String,
) -> Result<data::card_vars::LayerWrite, String> {
    let _permit = data::world_write_permit(&world_id)?;
    let root = data_root(&app)?;
    data::card_vars::write_layer(
        &root,
        &world_id,
        layer,
        id.as_deref(),
        generation,
        expected_rev.as_deref(),
        &vars_json,
    )
    .map_err(|error| error.to_string())
}
