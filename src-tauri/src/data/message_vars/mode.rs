//! 模式交接（計畫 8.1）與狀態快取重建。交接都在短提交鎖內、看目前模式決定要不要做，所以重做無害。
use super::control::{read_control, write_control, Mode, SceneVars};
use super::convert::{keep_previous, tree_to_stat, Macros};
use super::json::Json;
use super::source::{init_source, projected_tree};
use super::VarsTable;
use crate::data::state::{read_state_cache, write_state};
use crate::data::state_commit::CommitTx;
use crate::data::DataResult;
use std::path::Path;

pub fn new_token() -> String {
    ulid::Ulid::generate().to_string()
}

/// 這桌該不該用卡片變數模式：有載入 MVU 的卡、沒有重構骨架、不是角色優先桌（與前端 `pickCardShell`
/// 給 MVU 快照的條件同一依據）。是的話回那張卡的名字（`{{char}}` 代換用）。
pub fn eligible_char(root: &Path, world_id: &str, refactor_mode: Option<&str>) -> Option<String> {
    if refactor_mode == Some("characters") {
        return None;
    }
    let has_shell = crate::data::read_interface_shell(root, world_id)
        .ok()
        .flatten()
        .is_some_and(|shell| !shell.trim().is_empty());
    if has_shell {
        return None;
    }
    crate::import::read_card_interfaces(root, world_id)
        .ok()?
        .into_iter()
        .find(|card| card.mvu && card.unsupported.is_none())
        .map(|card| card.character_name)
}

/// 由狀態樹物化一張新表：沿用上一份完整表的其他鍵（`schema`、自訂鍵…），`stat_data` 換成樹的物化值
/// （同路徑同值的沿用原 JSON 值），`display_data`／`delta_data` 重新衍生。
pub fn materialize(
    tree: &crate::data::message_vars::convert::Tree,
    types: &std::collections::BTreeMap<String, String>,
    macros: Option<&Macros>,
    previous: Option<&Json>,
) -> Json {
    let fresh = tree_to_stat(tree, types, macros);
    let stat = keep_previous(fresh, previous.and_then(|table| table.get("stat_data")));
    let mut table = match previous {
        Some(table @ Json::Object(_)) => table.clone(),
        _ => Json::empty_object(),
    };
    derive_display(&mut table, stat);
    table
}

/// 建新表時衍生：`display_data`＝stat_data 拷貝、`delta_data`＝空物件。
pub fn derive_display(table: &mut Json, stat: Json) {
    table.insert("stat_data", stat.clone());
    table.insert("display_data", stat);
    table.insert("delta_data", Json::empty_object());
}

/// 確保目前這一幕在變數模式下有 epoch 與種子；回 true＝這一幕走事件。
/// - 已是變數模式且這一幕有種子：不動。
/// - 變數模式但這一幕還沒有種子（例如退回啟用前的父幕），或樹模式而這桌符合條件：以當下狀態樹物化
///   新種子、換新 epoch，一次寫入控制檔即發布。
/// - 樹模式且不符合條件：不動、回 false。停用腳本、讀不到卡片介面都不會把變數模式切回去。
pub fn ensure_active(tx: &CommitTx<'_>, macros: Option<&Macros>) -> DataResult<bool> {
    let mut control = read_control(tx.root, tx.world_id)?;
    let cache = read_state_cache(tx.root, tx.world_id)?;
    let scene = cache.current_scene;
    if control.active(scene).is_some() {
        return Ok(true);
    }
    let char_name = match control.mode {
        Mode::Events => None,
        Mode::Tree => match eligible_char(tx.root, tx.world_id, cache.refactor_mode.as_deref()) {
            Some(name) => Some(name),
            None => return Ok(false),
        },
    };
    let macros = match (macros, &control.macros) {
        (Some(given), _) => Some(Macros {
            user: given.user.clone(),
            char: given.char.clone().or(char_name),
        }),
        (None, Some(stored)) => Some(stored.clone()),
        (None, None) => None,
    };
    // 上一份完整表：這一幕以前的 epoch 的初始化來源（重構往返後退回、或模式切回來時沿用 schema 等鍵）
    let previous = match control.scene(scene) {
        Some(vars) => Some(
            init_source(tx.root, tx.world_id, scene, vars)?
                .table()
                .clone(),
        ),
        None => None,
    };
    let seed = materialize(
        &cache.state.tree,
        &cache.mechanism.value_types,
        macros.as_ref(),
        previous.as_ref(),
    );
    control.mode = Mode::Events;
    if macros.is_some() {
        control.macros = macros;
    }
    control.scenes.insert(
        scene.to_string(),
        SceneVars {
            epoch: new_token(),
            seed: VarsTable::from_json(&seed),
        },
    );
    write_control(tx, &control)?;
    Ok(true)
}

/// 重構套用（events → tree）：先把有效 stat_data 投影寫進 state.json 樹並確認寫成，再寫控制檔發布
/// `tree`。中途崩潰時控制檔仍是 events，有效狀態照舊從事件來，重做即可。
pub fn handover_to_tree(tx: &CommitTx<'_>) -> DataResult<()> {
    let mut control = read_control(tx.root, tx.world_id)?;
    if control.mode != Mode::Events {
        return Ok(());
    }
    let mut cache = read_state_cache(tx.root, tx.world_id)?;
    if let Some(tree) = projected_tree(tx.root, tx.world_id, cache.current_scene)? {
        if cache.state.tree != tree {
            cache.state.tree = tree;
            write_state(tx.root, tx.world_id, &cache)?;
        }
    }
    control.mode = Mode::Tree;
    write_control(tx, &control)
}

/// 變數模式時把 state.json 的樹換成目前投影。快取寫失敗不影響已完成的提交，下次重建。
pub fn refresh_cache(tx: &CommitTx<'_>) {
    let Ok(mut cache) = read_state_cache(tx.root, tx.world_id) else {
        return;
    };
    if let Ok(Some(tree)) = projected_tree(tx.root, tx.world_id, cache.current_scene) {
        if cache.state.tree != tree {
            cache.state.tree = tree;
            let _ = write_state(tx.root, tx.world_id, &cache);
        }
    }
}
