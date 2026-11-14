//! 戰鬥欄的模型寫入（ttrpg-combat-tracker §2.4–2.8）：HP 專用驗證（初始化、增量、改上限、敵人 insert 四入口）、
//! HP 增量綁本輪骰池、敵人整筆只准新增／移除、輪到誰須在先攻順序裡，以及戰鬥欄的拒收標記。
//! 先攻順序與狀態效果是一般清單欄，照預設狀態欄的守門與預算走。
use crate::data::default_panel::combat::{
    drop_from_order, in_order, list_values, COMBAT, CONDITIONS, CONDITIONS_MAX, CONDITION_CHARS,
    FOES, FOES_MAX, HP, HP_MAX, NAME_CHARS, ORDER, TURN,
};
use crate::data::default_panel::CHARACTERS;
use crate::data::{self, Mechanism, RefusedMark, StateNode};
use std::collections::{BTreeMap, BTreeSet};

use super::apply::add_delta_mark;
use super::note::{shown_pointer, snippet};
use super::types::{Patch, PatchOp, Record, RecordKind};

/// 合法 HP：`現值/上限` 兩個非負整數，`0 ≤ 現值 ≤ 上限`、`1 ≤ 上限 ≤ 99999`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Hp {
    current: i64,
    max: i64,
}

impl Hp {
    fn parse(text: &str) -> Option<Self> {
        let (current, max) = text.split_once('/')?;
        let number = |part: &str| -> Option<i64> {
            let part = part.trim();
            (!part.is_empty() && part.len() <= 5 && part.bytes().all(|b| b.is_ascii_digit()))
                .then(|| part.parse().ok())
                .flatten()
        };
        let hp = Self {
            current: number(current)?,
            max: number(max)?,
        };
        (hp.max >= 1 && hp.max <= HP_MAX && hp.current <= hp.max).then_some(hp)
    }

    fn text(self) -> String {
        format!("{}/{}", self.current, self.max)
    }
}

fn cell_exists(mechanism: &Mechanism, key: &str) -> bool {
    mechanism.panel_cells.values().any(|cell| cell == key)
}

/// 這條路徑是不是戰鬥 HP：角色格的 `hp`（玩家自訂過同名精確規則的格不算）或敵人的 `hp`。
pub(super) fn is_hp(mechanism: &Mechanism, path: &[String]) -> bool {
    if !mechanism.combat_panel {
        return false;
    }
    match path {
        [root, cell, leaf] if root == CHARACTERS && leaf == HP => {
            cell_exists(mechanism, cell) && !mechanism.rules.contains_key(&path.join("."))
        }
        [root, foes, _, leaf] => root == COMBAT && foes == FOES && leaf == HP,
        _ => false,
    }
}

/// 拒收標記收哪些路徑：兩種 HP、兩種狀態效果、`combat` 底下全部。
fn is_field(mechanism: &Mechanism, path: &str) -> bool {
    let segments: Vec<String> = path.split('.').map(str::to_owned).collect();
    if !mechanism.combat_panel {
        return false;
    }
    match segments.as_slice() {
        [root, ..] if root == COMBAT => true,
        [root, cell, leaf] if root == CHARACTERS && leaf == CONDITIONS => {
            cell_exists(mechanism, cell) && !mechanism.rules.contains_key(path)
        }
        _ => is_hp(mechanism, &segments),
    }
}

fn value_text(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

/// 拒收一筆，帶拒收標記（GM 寫了什麼）。
fn refuse(patch: &Patch, detail: String) -> Record {
    let mut record = Record::new(RecordKind::Rejected, patch.path.join("."), detail);
    let wrote = value_text(patch.value.as_ref());
    record.mark = Some(RefusedMark {
        wrote: (!wrote.is_empty()).then(|| snippet(&wrote, 40)),
        ..Default::default()
    });
    record
}

/// 拒收並附原因代碼，玩家看到的是白話原因而不是 GM 寫的原文。
fn refuse_why(patch: &Patch, detail: String, reason: &str) -> Record {
    let mut record = refuse(patch, detail);
    if let Some(mark) = record.mark.as_mut() {
        mark.reason = Some(reason.to_owned());
    }
    record
}

/// 增量：有限整數、絕對值 ≤ 99999（數字或數字字串）。
fn integer_delta(value: Option<&serde_json::Value>) -> Option<i64> {
    let number = match value? {
        serde_json::Value::Number(number) => number.as_f64()?,
        serde_json::Value::String(text) => text.trim().parse::<f64>().ok()?,
        _ => return None,
    };
    (number.is_finite() && number.fract() == 0.0 && number.abs() <= HP_MAX as f64)
        .then_some(number as i64)
}

/// 補丁入口（`apply_updates_typed` 對齊路徑之後、預算之前）：戰鬥 HP、敵人整筆、輪到誰在這裡處理。
/// 回 true＝已處理（套了或記了拒收），呼叫端跳過一般流程；false＝交回一般流程。
/// `used`＝本則回覆已用過的骰號，跨本則所有更新區塊共用、不落檔。
pub(super) fn patch(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patch: &Patch,
    used: &mut BTreeSet<crate::dice::DieRef>,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) -> bool {
    if !mechanism.combat_panel {
        return false;
    }
    match patch.path.as_slice() {
        [root, foes, _] if root == COMBAT && foes == FOES => {
            foe_entry(tree, patch, records, changes);
            true
        }
        [root, turn] if root == COMBAT && turn == TURN => {
            let name = value_text(patch.value.as_ref());
            let allowed = !matches!(patch.op, PatchOp::Replace | PatchOp::Insert)
                || name.is_empty()
                || in_order(tree, &name);
            if !allowed {
                records.push(refuse_why(
                    patch,
                    format!(
                        "{} 只能寫行動順序（/combat/order）裡的名字；「{}」不在順序裡，未套用。",
                        shown_pointer(&patch.path),
                        snippet(&name, 24)
                    ),
                    "not_in_order",
                ));
            }
            !allowed
        }
        // 敵人子欄：敵人格要已存在，新敵人只能整筆 insert（才過得了名字、上限與 HP 檢查）
        [root, foes, name, _, ..]
            if root == COMBAT
                && foes == FOES
                && data::node_at(tree, &patch.path[..3]).is_none() =>
        {
            records.push(refuse_why(
                patch,
                format!(
                    "沒有敵人「{name}」；新敵人請整筆 insert {}，值 {{\"hp\":\"現值/上限\"}}。",
                    shown_pointer(&patch.path[..3])
                ),
                "missing",
            ));
            true
        }
        _ if is_hp(mechanism, &patch.path) => {
            hp(tree, mechanism, patch, used, records, changes);
            true
        }
        _ => false,
    }
}

/// 敵人整筆：insert 只准新增不存在的名字（已存在就整筆拒收，免得 insert 轉替換蓋掉扣過的 HP）、remove 移除，其餘拒收。
fn foe_entry(
    tree: &mut BTreeMap<String, StateNode>,
    patch: &Patch,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) {
    let name = &patch.path[2];
    let shown = shown_pointer(&patch.path);
    let no_order =
        list_values(data::node_at(tree, &[COMBAT.to_owned(), ORDER.to_owned()])).is_empty();
    let Some(StateNode::Branch(foes)) = data::node_at_mut(tree, &patch.path[..2]) else {
        records.push(refuse(
            patch,
            format!("{shown}：戰鬥欄還沒準備好，未套用。"),
        ));
        return;
    };
    match patch.op {
        PatchOp::Insert => {}
        PatchOp::Remove => {
            if foes.remove(name).is_some() {
                changes.insert(patch.path.join("."), "移除".to_owned());
                if drop_from_order(tree, name) {
                    changes.insert(format!("{COMBAT}.{ORDER}"), "更新".to_owned());
                }
            } else {
                records.push(refuse_why(
                    patch,
                    format!("沒有敵人「{name}」，不用移除。"),
                    "missing",
                ));
            }
            return;
        }
        _ => {
            records.push(refuse(
                patch,
                format!("{shown} 是整筆敵人，只能 insert 新增或 remove 移除；扣血用 delta 寫 {shown}/hp。"),
            ));
            return;
        }
    }
    let refusal = if no_order {
        Some(("no_order", "還沒有行動順序，敵人加不進來；開打時先 replace /combat/order 排好順序（含敵人名字），再 insert 敵人。".to_owned()))
    } else if name.trim().is_empty()
        || name.chars().count() > NAME_CHARS as usize
        || name.contains(['.', '*'])
        || name.starts_with('_')
        || name == "-"
    {
        Some((
            "bad_name",
            format!("敵人名字要是 1–{NAME_CHARS} 字、不含「.」「*」、不以底線開頭，未套用。"),
        ))
    } else if foes.contains_key(name) || foes.keys().any(|key| data::glyph_equal(key, name)) {
        Some(("duplicate", format!("已有敵人「{name}」，扣血請用 delta 寫 {shown}/hp；新敵人請取不同名字（例 哥布林A、哥布林B）。")))
    } else if foes.len() >= FOES_MAX {
        Some((
            "too_many",
            format!("敵人最多 {FOES_MAX} 筆，未套用；倒下或逃走的請先 remove。"),
        ))
    } else {
        None
    };
    if let Some((reason, detail)) = refusal {
        records.push(refuse_why(patch, detail, reason));
        return;
    }
    let object = patch.value.as_ref().and_then(serde_json::Value::as_object);
    let hp = object
        .and_then(|object| object.get(HP))
        .and_then(serde_json::Value::as_str)
        .and_then(Hp::parse);
    let conditions: Option<Vec<String>> = match object.and_then(|object| object.get(CONDITIONS)) {
        None | Some(serde_json::Value::Null) => Some(Vec::new()),
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(|text| text.chars().take(CONDITION_CHARS as usize).collect())
            })
            .collect(),
        Some(_) => None,
    };
    let (Some(hp), Some(mut conditions)) = (hp, conditions) else {
        records.push(refuse_why(
            patch,
            format!("新增敵人的值要是 {{\"hp\":\"現值/上限\"}}（整數、上限 1–{HP_MAX}），可加 \"conditions\":[字串]；未套用。"),
            "bad_hp",
        ));
        return;
    };
    conditions.truncate(CONDITIONS_MAX as usize);
    let entry = BTreeMap::from([
        (HP.to_owned(), StateNode::Leaf(hp.text())),
        (
            CONDITIONS.to_owned(),
            StateNode::Branch(
                conditions
                    .into_iter()
                    .enumerate()
                    .map(|(index, item)| (index.to_string(), StateNode::Leaf(item)))
                    .collect(),
            ),
        ),
    ]);
    foes.insert(name.clone(), StateNode::Branch(entry));
    changes.insert(patch.path.join("."), "更新".to_owned());
}

/// 戰鬥 HP 的四個入口（敵人 insert 在 `foe_entry`）：初始化、增量、改上限；刪除與搬移拒收。
fn hp(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patch: &Patch,
    used: &mut BTreeSet<crate::dice::DieRef>,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) {
    let path_str = patch.path.join(".");
    let shown = shown_pointer(&patch.path);
    let Some(StateNode::Leaf(current)) = data::node_at(tree, &patch.path).cloned() else {
        records.push(refuse(patch, format!("{shown} 不存在，未套用。")));
        return;
    };
    let current_hp = Hp::parse(&current);
    match patch.op {
        PatchOp::Delta => {}
        PatchOp::Replace | PatchOp::Insert => {
            let wrote = value_text(patch.value.as_ref());
            let Some(next) = Hp::parse(&wrote) else {
                records.push(refuse(
                    patch,
                    format!("{shown} 要寫成「現值/上限」兩個整數（0 ≤ 現值 ≤ 上限、上限 1–{HP_MAX}），未套用。"),
                ));
                return;
            };
            match current_hp {
                // 還沒有 HP（或玩家手改留下的非法值）：整個寫入
                None => {}
                Some(hp) if hp == next => return,
                Some(hp) if hp.current == next.current => {}
                Some(hp) => {
                    records.push(refuse(
                        patch,
                        format!(
                            "{shown} 現值 {}，請用 delta 報增減量；改上限時現值照抄（例 {}/{}）。",
                            hp.text(),
                            hp.current,
                            next.max.max(hp.current)
                        ),
                    ));
                    return;
                }
            }
            data::set_tree_value(tree, &patch.path, &next.text());
            changes.insert(path_str, "更新".to_owned());
            return;
        }
        PatchOp::Remove | PatchOp::Move => {
            records.push(refuse(
                patch,
                format!("{shown} 不能刪除或搬移；扣血補血用 delta。"),
            ));
            return;
        }
    }
    let Some(reported) = integer_delta(patch.value.as_ref()) else {
        records.push(refuse(
            patch,
            format!("{shown} 的 delta 要是整數增減量（絕對值 ≤ {HP_MAX}，例 -7），未套用。"),
        ));
        return;
    };
    let mut delta = reported;
    let mut adjusted = None;
    let mut spent = Vec::new();
    if let Some(formula) = &patch.dice {
        let Some(pool) = mechanism.turn_dice.as_deref() else {
            records.push(refuse(
                patch,
                "本輪沒有骰池，dice 不能用；不帶 dice 直接報增減量。".to_owned(),
            ));
            return;
        };
        let (total, dice) = match crate::dice::resolve(pool, formula) {
            Ok(resolved) => resolved,
            Err(detail) => {
                records.push(refuse(patch, detail));
                return;
            }
        };
        if let Some((sides, index)) = dice.iter().find(|die| used.contains(*die)) {
            records.push(refuse(
                patch,
                format!("d{sides}#{index} 本則已經用過，一顆骰只能用一次。"),
            ));
            return;
        }
        if total < 0 {
            records.push(refuse(
                patch,
                format!("dice「{formula}」算出 {total}，傷害或治療量不能是負數，未套用。"),
            ));
            return;
        }
        if total != 0 && reported == 0 {
            records.push(refuse(patch, format!("dice「{formula}」算出 {total}，但 delta 是 0，分不出是扣血還是補血；扣血寫負數、補血寫正數。")));
            return;
        }
        let signed = if reported < 0 { -total } else { total };
        if signed.abs() > HP_MAX {
            records.push(refuse(
                patch,
                format!("dice「{formula}」算出的量超過 {HP_MAX}，未套用。"),
            ));
            return;
        }
        if signed != reported {
            adjusted = Some(signed);
        }
        delta = signed;
        spent = dice;
    }
    let Some(hp) = current_hp else {
        let detail = if current.trim().is_empty() {
            format!("{shown} 還沒有 HP，先依角色設定自己定一個合理值，用 replace 寫「現值/上限」（例 24/24）再報增減；不要停下來問玩家。")
        } else {
            format!(
                "{shown} 現值「{}」不是「現值/上限」格式，請玩家修正後再報增減；未套用。",
                snippet(&current, 20)
            )
        };
        records.push(refuse(patch, detail));
        return;
    };
    let raw = hp.current + delta;
    let next = raw.clamp(0, hp.max);
    data::set_tree_value(
        tree,
        &patch.path,
        &Hp {
            current: next,
            max: hp.max,
        }
        .text(),
    );
    used.extend(spent);
    add_delta_mark(changes, &path_str, (next - hp.current) as f64);
    if let Some(applied) = adjusted {
        let mut record = Record::new(
            RecordKind::Rejected,
            path_str.clone(),
            format!("{shown}：你報 {reported:+}，依骰面套 {applied:+}（已套用）；dice 的骰面總和就是增減量，有減半等調整就不要帶 dice。"),
        );
        record.mark = Some(RefusedMark {
            wrote: Some(format!("{reported:+}")),
            applied: Some(format!("{applied:+}")),
            ..Default::default()
        });
        records.push(record);
    }
    if next != raw {
        records.push(Record::new(
            RecordKind::Clamped,
            path_str.clone(),
            format!("{path_str} 已夾在範圍內，目前值 {next}/{}。", hp.max),
        ));
    }
}

/// 套用收尾：戰鬥欄的拒收記錄收成玩家看的標記（後面的蓋前面的）。依骰面改套的那筆帶實際增減。
pub(super) fn refused(mechanism: &Mechanism, records: &[Record]) -> BTreeMap<String, RefusedMark> {
    records
        .iter()
        .filter(|record| record.kind == RecordKind::Rejected && is_field(mechanism, &record.path))
        .map(|record| (record.path.clone(), record.mark.clone().unwrap_or_default()))
        .collect()
}

/// 圍欄入口的戰鬥欄拒收句：HP 與敵人只走更新區塊（補丁）；其餘回 None 照一般守門。
pub(super) fn fence_refusal(mechanism: &Mechanism, path: &[String]) -> Option<String> {
    let shown = shown_pointer(path);
    if is_hp(mechanism, path) {
        return Some(format!(
            "{shown} 不在狀態區塊裡寫；扣血補血用更新區塊的 delta，第一次用 replace 寫「現值/上限」。"
        ));
    }
    match path {
        [root, foes, ..] if mechanism.combat_panel && root == COMBAT && foes == FOES => Some(
            format!("{shown}：敵人只在更新區塊寫（insert 新增、remove 移除、hp 用 delta）。"),
        ),
        _ => None,
    }
}
