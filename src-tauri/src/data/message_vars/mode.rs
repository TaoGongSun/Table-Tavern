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

/// 這桌該不該用卡片變數模式：有載入 MVU（或帶 zod 變數 schema）的卡、沒有重構骨架、不是角色優先桌（與前端 `pickCardShell`
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
        .find(|card| (card.mvu || card.zod.is_some()) && card.unsupported.is_none())
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
    ensure_active_seeded(tx, macros, None)
}

/// 同 `ensure_active`；要新建種子時 `seed` 有值就直接用它（zod 卡由引擎建、過了 schema 的保序種子），
/// 不從狀態樹物化。
pub fn ensure_active_seeded(
    tx: &CommitTx<'_>,
    macros: Option<&Macros>,
    seed: Option<&Json>,
) -> DataResult<bool> {
    let mut control = read_control(tx.root, tx.world_id)?;
    let cache = read_state_cache(tx.root, tx.world_id)?;
    let scene = cache.current_scene;
    if control.active(scene).is_some() {
        return Ok(true);
    }
    let Some((macros, previous)) = seed_basis(tx, &control, &cache, macros)? else {
        return Ok(false);
    };
    let seed = match seed {
        Some(seed) => seed.clone(),
        None => materialize(
            &cache.state.tree,
            &cache.mechanism.value_types,
            macros.as_ref(),
            previous.as_ref(),
        ),
    };
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

/// 這一幕要新建種子時、`ensure_active` 不給種子會物化出的那張表（只讀不發布）：zod 桌卡上沒有 `[initvar]` 時，
/// 鎖外先拿它的 stat_data 過 schema 再當種子。已啟用或不符合條件回 None。
pub fn tree_seed(tx: &CommitTx<'_>, macros: Option<&Macros>) -> DataResult<Option<Json>> {
    Ok(tree_seed_with_macros(tx, macros)?.map(|(table, _)| table))
}

/// 同 `tree_seed`，另回建種子時生效的巨集（`{{user}}`／`{{char}}` 的有效值，同 `seed_basis`）。
pub fn tree_seed_with_macros(
    tx: &CommitTx<'_>,
    macros: Option<&Macros>,
) -> DataResult<Option<(Json, Option<Macros>)>> {
    let control = read_control(tx.root, tx.world_id)?;
    let cache = read_state_cache(tx.root, tx.world_id)?;
    if control.active(cache.current_scene).is_some() {
        return Ok(None);
    }
    Ok(
        seed_basis(tx, &control, &cache, macros)?.map(|(macros, previous)| {
            let table = materialize(
                &cache.state.tree,
                &cache.mechanism.value_types,
                macros.as_ref(),
                previous.as_ref(),
            );
            (table, macros)
        }),
    )
}

/// 非 zod 桌開場白（initvar-nonzod-overwrite）：這一幕還沒啟用、`override_stat` 有值時，以提交當下樹物化的表、
/// stat_data 整份換成它當種子啟用，回 true（這次發布了覆寫種子，呼叫端要把樹換成新投影）；其餘同 `ensure_active`、回 false。
pub fn ensure_active_overridden(
    tx: &CommitTx<'_>,
    macros: Option<&Macros>,
    override_stat: Option<&Json>,
) -> DataResult<bool> {
    if let Some(stat) = override_stat {
        if let Some((mut table, _)) = tree_seed_with_macros(tx, macros)? {
            derive_display(&mut table, stat.clone());
            ensure_active_seeded(tx, macros, Some(&table))?;
            return Ok(true);
        }
    }
    ensure_active(tx, macros)?;
    Ok(false)
}

/// 新建種子的依據：這桌該不該啟用（不該回 None）、生效的巨集、上一份完整表（這一幕以前的 epoch 的初始化來源，
/// 重構往返後退回、或模式切回來時沿用 schema 等鍵）。
fn seed_basis(
    tx: &CommitTx<'_>,
    control: &super::control::Control,
    cache: &crate::data::WorldState,
    macros: Option<&Macros>,
) -> DataResult<Option<(Option<Macros>, Option<Json>)>> {
    let scene = cache.current_scene;
    let char_name = match control.mode {
        Mode::Events => None,
        Mode::Tree => match eligible_char(tx.root, tx.world_id, cache.refactor_mode.as_deref()) {
            Some(name) => Some(name),
            None => return Ok(None),
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
    let previous = match control.scene(scene) {
        Some(vars) => Some(
            init_source(tx.root, tx.world_id, scene, vars)?
                .table()
                .clone(),
        ),
        None => None,
    };
    Ok(Some((macros, previous)))
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
