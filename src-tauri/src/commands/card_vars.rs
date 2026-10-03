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
    let config = data::read_config(&config_root(&app)?).unwrap_or_default();
    let lang = transport::ui_language(&config);
    let user = data::read_player_card(&root, &world_id)
        .ok()
        .flatten()
        .map(|card| card.name)
        .unwrap_or_else(|| transport::player_fallback_name(&lang).to_owned());
    let macros = data::message_vars::Macros { user, char: None };
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
