//! 實送的落地呼叫點（方案三之 4、三之 8）：掃描前結算、交給傳輸層那刻落地（計時＋變數副作用）、確定失敗當場撤回。
//! 量測不走這裡（唯讀，見 `scene_budget::measure`）。

use super::{macros, WorldScan};
use crate::data::card_vars::{self, Layer, LayerWrite};
use crate::data::message_vars;
use crate::data::state_commit::with_commit;
use crate::data::world_info_store::{self as store, Perspective, Report, VarConflict, VarIntent};
use crate::st_macros::variables::{ScopeKind, VarOp, VarOpKind, VarScope, Variables};
use crate::ui_msg::UiMsg;
use crate::world_info::timed::WiTimed;
use std::path::Path;

/// 變數層 compare-and-set 撞到別人寫入（`Stale`）時最多重試幾次（三之 8「變數衝突」）。
const VAR_ATTEMPTS: usize = 3;

/// 實送掃描之前（持整桌寫入許可、回合交接 `settle_previous_turn` 之後，否則「GM 已提交、正文還沒落檔」的
/// 回合會被誤判成失敗）：結算這一幕的 pending，再讀這個視角的計時表。失敗回 `WorldInfoSettleFailed`。
/// 同桌還有別的對話輪在途就擋下（`WorldBusy`）：整桌寫入許可是共用讀鎖，GM 與角色回合在後端不互斥，
/// 並行時結算會把對方已送出、還沒落檔的落地誤撤。
pub fn before_scan(
    root: &Path,
    world_id: &str,
    scene: u64,
    turn_id: &str,
    perspective: &Perspective,
) -> Result<WiTimed, String> {
    if crate::inflight::other_turn_in_flight(world_id, turn_id) {
        return Err(UiMsg::WorldBusy.to_string());
    }
    store::settle_pending(root, world_id, scene).map_err(|error| error.to_string())?;
    store::read_timed(root, world_id, scene, perspective).map_err(|error| {
        UiMsg::WorldInfoSettleFailed {
            error: error.to_string(),
        }
        .to_string()
    })
}

/// 交給傳輸層那一刻：計時表落地（寫入中）→ 變數層照「意圖→寫層→記結果」逐層寫（chat，再 global）→ 已送出。
/// 中途失敗就撤回、不送出；撤回時保住別人之後寫的值，連同原錯一起回報。成功才把本輪 outlet 記成下一輪的
/// 「上一輪 outlet」。
pub fn land(
    root: &Path,
    world_id: &str,
    scene: u64,
    turn_id: &str,
    perspective: &Perspective,
    scan: &WorldScan,
) -> Result<(), String> {
    if let Err(error) =
        store::begin_landing(root, world_id, scene, turn_id, perspective, &scan.timed)
    {
        fail(root, world_id, scene, turn_id, Report::Notices);
        return Err(error.to_string());
    }
    let landed = land_vars(root, world_id, scene, turn_id, &scan.var_ops).and_then(|()| {
        store::mark_sent(root, world_id, scene, turn_id).map_err(|error| error.to_string())
    });
    if let Err(error) = landed {
        let kept = fail(root, world_id, scene, turn_id, Report::Inline);
        return Err(with_kept(error, &kept));
    }
    macros::remember_outlets(world_id, perspective, &scan.outlets);
    Ok(())
}

/// 原錯加上沒還原的變數層（有的話）。
pub fn with_kept(error: String, kept: &[VarConflict]) -> String {
    if kept.is_empty() {
        return error;
    }
    UiMsg::WorldInfoVarsKept {
        error,
        layers: kept
            .iter()
            .map(|conflict| layer_name(conflict.layer))
            .collect::<Vec<_>>()
            .join(", "),
    }
    .to_string()
}

/// 持許可當場判得出的確定失敗（落地中途出錯、角色呼叫回錯、角色中止沒有半截）：撤回這回合的落地，
/// 回傳沒還原的變數寫入（`Report::Inline` 交回呼叫端連同原錯回報，`Report::Notices` 寫進待回報檔）。
/// 撤回本身失敗只記 log，留給下一次實送前的結算（那裡失敗會明確回報）。
pub fn fail(
    root: &Path,
    world_id: &str,
    scene: u64,
    turn_id: &str,
    report: Report,
) -> Vec<VarConflict> {
    store::fail_turn(root, world_id, scene, turn_id, report).unwrap_or_else(|error| {
        log::warn!("world-info: 撤回回合 {turn_id} 的落地失敗，留給下一次結算：{error}");
        Vec::new()
    })
}

fn scope_layer(scope: ScopeKind) -> Layer {
    match scope {
        ScopeKind::Local => Layer::Chat,
        ScopeKind::Global => Layer::Global,
    }
}

/// 變數副作用逐層落地（chat，再 global）。
pub(crate) fn land_vars(
    root: &Path,
    world_id: &str,
    scene: u64,
    turn_id: &str,
    ops: &[VarOp],
) -> Result<(), String> {
    for scope in [ScopeKind::Local, ScopeKind::Global] {
        let ops: Vec<VarOp> = ops.iter().filter(|op| op.scope == scope).cloned().collect();
        if !ops.is_empty() {
            land_layer(root, world_id, scene, turn_id, scope, &ops)?;
        }
    }
    Ok(())
}

/// 在最新的表上重放這一層的操作序列，回寫入後的 JSON。
pub(crate) fn replayed(vars: &str, scope: ScopeKind, ops: &[VarOp]) -> Result<String, String> {
    let table = VarScope::from_json_text(vars)?;
    let mut variables = match scope {
        ScopeKind::Local => Variables::new(table, VarScope::default()),
        ScopeKind::Global => Variables::new(VarScope::default(), table),
    };
    variables.replay(ops);
    Ok(variables.scope(scope).to_json_text())
}

/// 操作序列的留證格式（結算不讀）：`[{op, name, value?, index?}]`，值是 `JSON.stringify` 的原文。
pub(crate) fn ops_json(ops: &[VarOp]) -> serde_json::Value {
    ops.iter()
        .map(|op| {
            let mut item = serde_json::Map::new();
            item.insert("name".to_owned(), op.name.clone().into());
            match &op.kind {
                VarOpKind::Set { value, index } => {
                    item.insert("op".to_owned(), "set".into());
                    item.insert("value".to_owned(), value.stringify().into());
                    if let Some(index) = index {
                        item.insert("index".to_owned(), index.clone().into());
                    }
                }
                VarOpKind::Add { value } => {
                    item.insert("op".to_owned(), "add".into());
                    item.insert("value".to_owned(), value.stringify().into());
                }
                VarOpKind::Delete => {
                    item.insert("op".to_owned(), "delete".into());
                }
            }
            serde_json::Value::Object(item)
        })
        .collect()
}

/// 一層：讀最新的表與 rev → 重放 → 記意圖（重試前改成這次讀到的前像）→ compare-and-set → 記結果。
/// `Stale` 重讀重放，最多 `VAR_ATTEMPTS` 次；`Rejected` 或重試用完＝落地失敗。
fn land_layer(
    root: &Path,
    world_id: &str,
    scene: u64,
    turn_id: &str,
    scope: ScopeKind,
    ops: &[VarOp],
) -> Result<(), String> {
    let layer = scope_layer(scope);
    let mut index: Option<usize> = None;
    for _ in 0..VAR_ATTEMPTS {
        let doc = card_vars::read_layer(root, world_id, layer, None)
            .map_err(|error| error.to_string())?;
        let after = replayed(&doc.vars, scope, ops)?;
        let at = match index {
            None => {
                let at = store::push_var_intent(
                    root,
                    world_id,
                    scene,
                    turn_id,
                    VarIntent {
                        layer,
                        id: None,
                        expected_rev: doc.rev.clone(),
                        before: doc.vars.clone(),
                        ops: ops_json(ops),
                        after_rev: None,
                        restore_rev: None,
                    },
                )
                .map_err(|error| error.to_string())?;
                index = Some(at);
                at
            }
            Some(at) => {
                store::update_var_intent(root, world_id, scene, turn_id, at, |intent| {
                    intent.expected_rev = doc.rev.clone();
                    intent.before = doc.vars.clone();
                })
                .map_err(|error| error.to_string())?;
                at
            }
        };
        let generation = with_commit(root, world_id, message_vars::generation);
        match card_vars::write_layer(
            root,
            world_id,
            layer,
            None,
            generation,
            doc.rev.as_deref(),
            &after,
        )
        .map_err(|error| error.to_string())?
        {
            LayerWrite::LayerOk { rev } => {
                return store::update_var_intent(root, world_id, scene, turn_id, at, |intent| {
                    intent.after_rev = Some(rev);
                })
                .map_err(|error| error.to_string());
            }
            LayerWrite::Stale { .. } => continue,
            LayerWrite::Rejected { code, .. } => {
                return Err(format!("card-vars {}: {code}", layer_name(layer)))
            }
        }
    }
    Err(format!(
        "card-vars {}: stale after {VAR_ATTEMPTS} attempts",
        layer_name(layer)
    ))
}

fn layer_name(layer: Layer) -> &'static str {
    match layer {
        Layer::Global => "global",
        _ => "chat",
    }
}
