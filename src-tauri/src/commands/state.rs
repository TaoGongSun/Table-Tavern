use crate::commands::character::load_active_cards;
use crate::data::WorldState;
use crate::{data, data_root, mechanism, transport};
use serde::Serialize;

/// 世界書分頁「機制帳本」面板：哪些條目被本地機制接管／跳過，供玩家切回「照原文送模型」。
#[tauri::command]
pub(crate) fn mechanism_ledger(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<mechanism::Ledger, String> {
    Ok(mechanism::read_ledger(&data_root(&app)?, &world_id))
}

#[tauri::command]
pub(crate) fn read_state(app: tauri::AppHandle, world_id: String) -> Result<WorldState, String> {
    data::read_state(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn write_state(
    app: tauri::AppHandle,
    world_id: String,
    state: WorldState,
) -> Result<(), String> {
    data::write_state(&data_root(&app)?, &world_id, &state).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn set_table_state(
    app: tauri::AppHandle,
    world_id: String,
    fields: std::collections::BTreeMap<String, String>,
) -> Result<(), String> {
    let root = data_root(&app)?;
    let mut state = data::read_state(&root, &world_id).map_err(|error| error.to_string())?;
    for (key, value) in fields {
        if value.is_empty() {
            state.state.table.remove(&key);
        } else {
            state.state.table.insert(key, value);
        }
    }
    data::write_state(&root, &world_id, &state).map_err(|error| error.to_string())?;
    data::set_last_transcript_state(&root, &world_id, state.current_scene, &state.state)
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub(crate) async fn set_state_path(
    app: tauri::AppHandle,
    world_id: String,
    path: Vec<String>,
    value: String,
) -> Result<(), String> {
    let root = data_root(&app)?;
    let mut state = data::read_state(&root, &world_id).map_err(|error| error.to_string())?;
    if !data::set_tree_value(&mut state.state.tree, &path, &value) {
        return Ok(());
    }
    data::write_state(&root, &world_id, &state).map_err(|error| error.to_string())?;
    data::set_last_transcript_state(&root, &world_id, state.current_scene, &state.state)
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// 面板指認：把角色卡綁到狀態樹的某個分支；path 為 None／空陣列＝解除綁定。
/// 一支分支只屬於一個角色，換綁時把指到同一條路徑的舊綁定一併移除。
/// branch_bindings 在 WorldState 上、不在 TableState 裡，不需要同步 transcript 快照。
#[tauri::command]
pub(crate) fn set_branch_binding(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    path: Option<Vec<String>>,
) -> Result<(), String> {
    let root = data_root(&app)?;
    let mut state = data::read_state(&root, &world_id).map_err(|error| error.to_string())?;
    match path.filter(|path| !path.is_empty()) {
        Some(path) => {
            // 一支分支只屬於一個角色：先清掉其他卡指到同一條路徑的舊綁定。
            state
                .branch_bindings
                .retain(|other_id, bound| *other_id == character_id || *bound != path);
            state.branch_bindings.insert(character_id, path);
        }
        None => {
            state.branch_bindings.remove(&character_id);
        }
    }
    data::write_state(&root, &world_id, &state).map_err(|error| error.to_string())
}

/// 面板記號：玩家把某欄標成計數器（例如卡片自訂的「第 N 天」，時間跳躍是那張卡的明文
/// 功能），以後全量桌跳動比對不再對它示警。寫一條 Counter 規則釘死，並清掉這一輪
/// 已經標出來的那筆警示（不然要等下一輪重算才會消失）。
#[tauri::command]
pub(crate) fn mark_state_counter(
    app: tauri::AppHandle,
    world_id: String,
    path: Vec<String>,
) -> Result<(), String> {
    let root = data_root(&app)?;
    let mut state = data::read_state(&root, &world_id).map_err(|error| error.to_string())?;
    let Some(first) = path.first() else {
        return Ok(());
    };
    let mut rule = data::FieldRule::for_kind(data::FieldKind::Counter);
    rule.branch = Some(first.clone());
    let key = path.join(".");
    state.mechanism.rules.insert(key.clone(), rule);
    state.state.jumps.remove(&key);
    data::write_state(&root, &world_id, &state).map_err(|error| error.to_string())
}

/// 面板要畫的有效綁定（含自動同名比對的結果）；解析不到分支的卡不進清單。
#[tauri::command]
pub(crate) fn branch_bindings(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<Vec<BranchBinding>, String> {
    let root = data_root(&app)?;
    let state = data::read_state(&root, &world_id).map_err(|error| error.to_string())?;
    let cards = load_active_cards(&root, &world_id)?;
    Ok(cards
        .into_iter()
        .filter_map(|card| {
            let path = transport::resolve_branch(
                &state.state.tree,
                &state.branch_bindings,
                &card.id,
                &card.name,
            )?;
            let auto = state.branch_bindings.get(&card.id) != Some(&path);
            Some(BranchBinding {
                path,
                character_id: card.id,
                character_name: card.name,
                auto,
            })
        })
        .collect())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BranchBinding {
    /// 狀態樹路徑
    path: Vec<String>,
    character_id: String,
    character_name: String,
    /// true＝同名自動比對的結果（沒存進 state.json）
    auto: bool,
}
