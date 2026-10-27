//! zod 卡的引擎路徑（mvu-zod-semantics 規格 2.4 節）在變數層的兩段鎖：段 1 固定輸入、段 3 重驗才寫；
//! 中間鎖外跑引擎由 `mvu_engine::wiring` 做。另附表與樹之間的轉換（後處理用），全程保序。
use super::convert::{merge_tree_change, stat_to_tree, NewValues, Tree};
use super::json::Json;
use super::mode::ensure_active_seeded;
use super::source::current_source;
use super::turn::PendingMain;
use super::write::{live_turn, submit, GmCommit, TurnSide};
use crate::data::state::{read_state, WorldState};
use crate::data::state_commit::CommitTx;
use crate::data::{invalid_data, DataResult};
use std::collections::BTreeMap;

/// 段 1（鎖內）：回合還對得上才固定輸入。樹模式開始的回合在這裡啟用變數模式——`seed` 有值（zod 卡由引擎建的
/// 保序種子）就用它，否則照舊從樹物化；固定的輸入存回回合紀錄。回合不符回 None；啟用不了變數模式回錯
/// （引擎路徑必須有表）。
pub fn engine_turn_input(
    tx: &CommitTx<'_>,
    turn_id: &str,
    seed: Option<&Json>,
) -> DataResult<Option<Json>> {
    let Some(record) = live_turn(tx, turn_id)? else {
        return Ok(None);
    };
    if let Some(input) = record.input {
        return Ok(Some(input));
    }
    if !ensure_active_seeded(tx, record.macros.as_ref(), seed)? {
        return Err(invalid_data("mvu-engine-needs-variable-mode"));
    }
    let input = current_source(tx.root, tx.world_id, record.scene)?
        .map(|source| source.table().clone())
        .ok_or_else(|| invalid_data("mvu-engine-needs-variable-mode"))?;
    super::turn::update_turn(tx, turn_id, |stored| stored.input = Some(input.clone()));
    Ok(Some(input))
}

/// 這一幕要不要建種子（還沒啟用變數模式）：給段 1 之前在鎖外先算種子用。
pub fn needs_seed(tx: &CommitTx<'_>) -> DataResult<bool> {
    let scene = crate::data::state::read_state_cache(tx.root, tx.world_id)?.current_scene;
    Ok(super::control::read_control(tx.root, tx.world_id)?
        .active(scene)
        .is_none())
}

/// 開場白用：這一幕還沒啟用就以給定的種子啟用變數模式，回目前這一幕的初始化來源（完整表）。
pub fn activate_for_opening(
    tx: &CommitTx<'_>,
    macros: &super::convert::Macros,
    seed: Option<&Json>,
) -> DataResult<Option<Json>> {
    if !ensure_active_seeded(tx, Some(macros), seed)? {
        return Ok(None);
    }
    let scene = crate::data::state::read_state_cache(tx.root, tx.world_id)?.current_scene;
    Ok(current_source(tx.root, tx.world_id, scene)?.map(|source| source.table().clone()))
}

/// 目前這一幕已啟用時的初始化來源（完整表），不啟用、不物化；開場白鎖內核對用。
pub fn active_table(tx: &CommitTx<'_>) -> DataResult<Option<Json>> {
    if needs_seed(tx)? {
        return Ok(None);
    }
    let scene = crate::data::state::read_state_cache(tx.root, tx.world_id)?.current_scene;
    Ok(current_source(tx.root, tx.world_id, scene)?.map(|source| source.table().clone()))
}

/// 段 3（鎖內）：回合仍對得上、回合紀錄的輸入就是段 1 那份（`expected`；建種子失敗、沒走到段 1 時是 None）才寫，
/// 否則回 None（過期結果不提交）。schema 憑證由呼叫端在同一把鎖內先核對。
/// `apply` 拿最新狀態、回結果與新表；新表 None＝整次失敗：狀態不寫、正文照常落檔且不帶表。
pub fn apply_engine_turn<R>(
    tx: &CommitTx<'_>,
    turn_id: &str,
    expected: Option<&Json>,
    main: PendingMain,
    apply: impl FnOnce(&mut WorldState) -> (R, Option<Json>),
    sides: impl FnOnce(&WorldState, &R) -> Vec<TurnSide>,
) -> DataResult<Option<GmCommit<R>>> {
    let Some(record) = live_turn(tx, turn_id)? else {
        return Ok(None);
    };
    if record.input.as_ref() != expected {
        return Ok(None);
    }
    let mut state = read_state(tx.root, tx.world_id)?;
    let (result, pending) = apply(&mut state);
    let write = pending.is_some();
    submit(
        tx,
        turn_id,
        record.input,
        pending,
        main,
        state,
        result,
        sides,
        write,
    )
}

/// 表的 stat_data 投影成狀態樹（後處理在樹上做）
pub fn table_tree(table: &Json) -> Tree {
    stat_to_tree(table.get("stat_data"))
}

/// 後處理改到樹：合回表的 stat_data——沒動到的路徑保留原 JSON 值與鍵順序，只換變了的葉。
pub fn merge_post_process(
    table: &Json,
    before: &Tree,
    after: &Tree,
    types: &BTreeMap<String, String>,
) -> Json {
    let rules = NewValues {
        types,
        keep_strings: false,
        clean: None,
    };
    let stat = match table.get("stat_data") {
        Some(stat) => stat.clone(),
        None => Json::empty_object(),
    };
    let merged = merge_tree_change(&stat, before, after, &rules);
    let mut table = table.clone();
    table.insert("stat_data", merged);
    table
}
