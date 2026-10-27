use crate::data::{self, FieldKind, FieldRule, Mechanism, NumericUpdate, StateNode, UpdateMode};
use std::collections::BTreeMap;

use super::list_keys::{BatchKeys, Prepared};
use super::note::{build_notes, shown_pointer, snippet};
use super::rules::rule_for;
use super::tree::{
    canonical_path, format_num, insert_node, json_to_node, leaf_value, split_pair, take_node,
    value_as_f64,
};
use super::types::{Outcome, Parsed, Patch, PatchOp, Record, RecordKind};
use super::upstream_set::{is_vwd, upstream_set, TypedView, Val};

mod array;
mod hint;

// ---------------------------------------------------------------------
// 套用：依欄位規則把 Patch 套進狀態樹
// ---------------------------------------------------------------------

/// 依欄位規則把更新套進狀態樹，回傳記帳、下一輪要給模型的自癒回饋句、以及這一輪真的改到樹的變動。
#[cfg(test)]
pub fn apply_updates(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patches: &[Patch],
) -> Outcome {
    let mut view = TypedView::for_table(mechanism, None, tree, tree);
    let parsed: Vec<Parsed> = patches.iter().cloned().map(Parsed::Patch).collect();
    apply_updates_typed(tree, mechanism, &parsed, &mut view)
}

/// 同 `apply_updates`，`view` 是這一批的帶型別視圖（沒重構的 MVU 卡才有作用，見 `upstream_set`）。
/// 解析時就拒收的項目照原位置記進記錄，回饋順序＝區塊裡的原順序。
pub(super) fn apply_updates_typed(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    parsed: &[Parsed],
    view: &mut TypedView,
) -> Outcome {
    let mut records = Vec::new();
    let mut changes = BTreeMap::new();
    let patches: Vec<&Patch> = parsed
        .iter()
        .filter_map(|item| match item {
            Parsed::Patch(patch) => Some(patch),
            Parsed::Note(_) => None,
        })
        .collect();
    let list_keys = BatchKeys::new(tree, mechanism, &patches);
    for item in parsed {
        let patch = match item {
            Parsed::Patch(patch) => patch,
            Parsed::Note(record) => {
                records.push(record.clone());
                continue;
            }
        };
        let canonical = match canonicalize(tree, mechanism, patch) {
            Ok(canonical) => canonical,
            Err(record) => {
                records.push(record);
                continue;
            }
        };
        let prepared = list_keys.prepare(mechanism, &canonical);
        let patch = prepared
            .as_ref()
            .map_or(&canonical, |prepared| &prepared.patch);
        if !view.is_active() {
            apply_one(tree, mechanism, patch, view, &mut records, &mut changes);
            record_list_keys(tree, prepared.as_ref(), &changes, &mut records);
            continue;
        }
        let before = tree.clone();
        let moved = (patch.op == PatchOp::Move)
            .then(|| view.get(&patch.from))
            .flatten();
        let move_done = apply_one(tree, mechanism, patch, view, &mut records, &mut changes);
        let keys: Vec<&String> = patch
            .path
            .first()
            .into_iter()
            .chain(patch.from.first())
            .collect();
        view.after_command(&keys, &before, tree);
        // move 搬的是原值：只有這一筆 move 明確搬成功才照原型別寫回目的欄；拒收、失敗都不動視圖
        if let (true, Some(moved)) = (move_done, moved) {
            view.overwrite(&patch.path, &moved);
        }
        record_list_keys(tree, prepared.as_ref(), &changes, &mut records);
    }
    let notes = build_notes(&records);
    Outcome {
        records,
        notes,
        changes,
        typed: None,
    }
}

/// 清單欄鍵名校正：真的寫進樹（這一批有變動標記、葉子就是校正後的值）才記帳。
fn record_list_keys(
    tree: &BTreeMap<String, StateNode>,
    prepared: Option<&Prepared>,
    changes: &BTreeMap<String, String>,
    records: &mut Vec<Record>,
) {
    let Some(prepared) = prepared else {
        return;
    };
    let written = changes.contains_key(&prepared.patch.path.join("."))
        && data::node_at(tree, &prepared.patch.path).and_then(leaf_value)
            == prepared
                .patch
                .value
                .as_ref()
                .and_then(|value| value.as_str());
    if written {
        records.push(prepared.record());
    }
}

/// 路徑對回樹上的寫法（模型把「驻守天数」寫成「駐守天數」也對得上）：精確優先、同層唯一折疊兜底，
/// 歧義整筆拒收。insert 與 move 目的地的最後一段是新鍵，不折；move 來源整條都折。
/// 單段 insert 例外：只差字形的是「有精確登記規則的頂層葉子」（系統管的欄位）就接到它，免得長出第二份。
fn canonicalize(
    tree: &BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patch: &Patch,
) -> Result<Patch, Record> {
    let ambiguous = |path: &[String], keys: Vec<String>| {
        Record::new(
            RecordKind::Rejected,
            path.join("."),
            format!(
                "{} 對到好幾個只差字形的欄位（{}），請照狀態裡的原字寫路徑。",
                shown_pointer(path),
                keys.join("、")
            ),
        )
    };
    let new_key = matches!(patch.op, PatchOp::Insert | PatchOp::Move);
    let mut path =
        canonical_path(tree, &patch.path, !new_key).map_err(|keys| ambiguous(&patch.path, keys))?;
    if patch.op == PatchOp::Insert && patch.path.len() == 1 {
        if let Some(key) = registered_glyph_leaf(tree, mechanism, &path[0])
            .map_err(|keys| ambiguous(&patch.path, keys))?
        {
            path = vec![key];
        }
    }
    let from = if patch.op == PatchOp::Move {
        canonical_path(tree, &patch.from, true).map_err(|keys| ambiguous(&patch.from, keys))?
    } else {
        patch.from.clone()
    };
    Ok(Patch {
        op: patch.op,
        path,
        from,
        value: patch.value.clone(),
    })
}

/// 單段 insert 目標不存在時：頂層只差字形、且 `mechanism.rules` 精確登記了該鍵（不認萬用 `*`）的葉子。
/// 唯一回 Some，多個回 Err（候選鍵），沒有回 None。巢狀路徑不在這裡處理（照原字當新鍵）。
fn registered_glyph_leaf(
    tree: &BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    key: &str,
) -> Result<Option<String>, Vec<String>> {
    if tree.contains_key(key) {
        return Ok(None);
    }
    let candidates: Vec<String> = tree
        .iter()
        .filter(|(name, node)| {
            matches!(node, StateNode::Leaf(_))
                && data::glyph_equal(name, key)
                && mechanism.rules.contains_key(*name)
        })
        .map(|(name, _)| name.clone())
        .collect();
    match candidates.len() {
        0 => Ok(None),
        1 => Ok(candidates.into_iter().next()),
        _ => Err(candidates),
    }
}

/// 回傳 true＝這一筆是 move 而且搬成功（其餘一律 false）。
fn apply_one(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patch: &Patch,
    view: &mut TypedView,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) -> bool {
    if let Some(offending) = readonly_violation(patch) {
        records.push(Record::new(
            RecordKind::Rejected,
            offending.join("."),
            format!(
                "{} 是唯讀欄位（底線開頭），不接受更新。",
                shown_pointer(offending)
            ),
        ));
        return false;
    }
    // 沒重構 MVU 卡：路徑進到陣列（`/HP/0`、`/清單/-`）照上游在帶型別視圖上操作，整個陣列寫回葉子
    if patch.op != PatchOp::Move {
        if let Some(base) = array::array_field(tree, view, &patch.path) {
            array::apply(tree, mechanism, patch, base, view, records, changes);
            return false;
        }
    }
    match patch.op {
        PatchOp::Delta => apply_delta(tree, mechanism, patch, view, records, changes),
        PatchOp::Replace => apply_replace(tree, mechanism, patch, view, records, changes),
        PatchOp::Insert => apply_insert(tree, mechanism, patch, view, records, changes),
        PatchOp::Remove => apply_remove(tree, mechanism, patch, records, changes),
        PatchOp::Move => return apply_move(tree, mechanism, patch, view, records, changes),
    }
    false
}

/// delta 的變動標記：帶號數字，正數補 `+`（負數 `format_num` 本身就帶 `-`）。
/// 標記照原始 delta 寫——被夾邊界也算改到，記的是模型要求的量，不是夾完的量。
pub(super) fn signed_delta_mark(delta: f64) -> String {
    if delta >= 0.0 {
        format!("+{}", format_num(delta))
    } else {
        format_num(delta)
    }
}

/// delta 的增量：數字或數字字串（比上游寬鬆），非有限數（"NaN"、"inf"）不收。
pub(super) fn finite_delta(value: Option<&serde_json::Value>) -> Option<f64> {
    value_as_f64(value).filter(|delta| delta.is_finite())
}

/// 上游 add 的 `parseFloat((a + b).toPrecision(12))`。
pub(super) fn to_precision_12(value: f64) -> f64 {
    if !value.is_finite() || value == 0.0 {
        return value;
    }
    format!("{value:.11e}").parse().unwrap_or(value)
}

/// `[值, 說明]` 的 delta 算出來的結果：新值、增量、夾值前後。
pub(super) struct VwdDelta {
    pub(super) value: Val,
    pub(super) delta: f64,
    raw: f64,
    next: f64,
}

impl VwdDelta {
    /// 被夾到範圍內時的記錄（結果是非有限數不算夾值）。
    pub(super) fn clamped(&self, path_str: &str) -> Option<Record> {
        (self.next != self.raw && self.raw.is_finite()).then(|| {
            Record::new(
                RecordKind::Clamped,
                path_str.to_owned(),
                format!(
                    "{path_str} 已夾在範圍內，目前值 {}。",
                    format_num(self.next)
                ),
            )
        })
    }
}

/// `[值, 說明]` 的 delta 計算：第 0 項加增量、說明不動；明確規則有範圍就夾（與本 app 數字欄一致）。
/// `shown` 是句中印的路徑；擋下時回拒收句。
pub(super) fn vwd_delta_value(
    rule: &FieldRule,
    shown: &str,
    items: &[Val],
    value: Option<&serde_json::Value>,
) -> Result<VwdDelta, String> {
    if let Some(detail) = hint::guard_refusal(rule, shown) {
        return Err(detail);
    }
    let Val::Num(first) = items[0] else {
        return Err(format!(
            "{shown} 的第 0 項不是數字，不能 delta；replace 它只會改第 0 項（目前「{}」）。",
            hint::first_item_text(&items[0])
        ));
    };
    let Some(delta) = finite_delta(value) else {
        return Err(format!(
            "{shown} 是數字欄，delta 的值要是純數字增減量（例 5、-10）。"
        ));
    };
    let raw = to_precision_12(first + delta);
    let mut next = raw;
    if let Some(min) = rule.min {
        next = next.max(min);
    }
    if let Some(max) = rule.max {
        next = next.min(max);
    }
    Ok(VwdDelta {
        value: Val::Arr(vec![Val::Num(next), items[1].clone()]),
        delta,
        raw,
        next,
    })
}

#[allow(clippy::too_many_arguments)]
fn vwd_delta(
    tree: &mut BTreeMap<String, StateNode>,
    patch: &Patch,
    path_str: &str,
    rule: &FieldRule,
    items: &[Val],
    view: &mut TypedView,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) {
    let shown = shown_pointer(&patch.path);
    let result = match vwd_delta_value(rule, &shown, items, patch.value.as_ref()) {
        Ok(result) => result,
        Err(detail) => {
            records.push(Record::new(
                RecordKind::Rejected,
                path_str.to_owned(),
                detail,
            ));
            return;
        }
    };
    if insert_node(tree, &patch.path, result.value.node()).is_err() {
        return;
    }
    view.set(&patch.path, &result.value);
    changes.insert(path_str.to_owned(), signed_delta_mark(result.delta));
    records.extend(result.clamped(path_str));
}

fn readonly_violation(patch: &Patch) -> Option<&[String]> {
    if patch.path.iter().any(|segment| segment.starts_with('_')) {
        return Some(&patch.path);
    }
    if patch.op == PatchOp::Move && patch.from.iter().any(|segment| segment.starts_with('_')) {
        return Some(&patch.from);
    }
    None
}

fn apply_delta(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patch: &Patch,
    view: &mut TypedView,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) {
    let path_str = patch.path.join(".");
    let current = match data::node_at(tree, &patch.path) {
        None => {
            records.push(hint::missing(tree, mechanism, &patch.path, PatchOp::Delta));
            return;
        }
        Some(StateNode::Branch(children)) => {
            records.push(hint::branch(mechanism, &patch.path, children, "做增減"));
            return;
        }
        Some(StateNode::Leaf(value)) => value.clone(),
    };
    let rule = rule_for(mechanism, &patch.path, Some(&current));
    // 沒重構 MVU 卡的 `[值, 說明]`：照上游加在第 0 項（骰值／唯讀規則照樣先擋）。
    if view.is_active() {
        if let Some(Val::Arr(items)) = view.get(&patch.path) {
            if is_vwd(&items) {
                vwd_delta(
                    tree, patch, &path_str, &rule, &items, view, records, changes,
                );
                return;
            }
        }
    }
    // 先看欄位收不收 delta，再解析值：用錯操作要回饋給模型改，所以記拒收（進下一輪自癒回饋）。
    let shown = shown_pointer(&patch.path);
    if let Some(detail) = delta_refusal(&rule, &shown) {
        records.push(Record::new(RecordKind::Rejected, path_str.clone(), detail));
        return;
    }
    let Some(delta) = finite_delta(patch.value.as_ref()) else {
        let detail = if rule.kind == FieldKind::Pair {
            format!("{shown} 是「現值/上限」欄，delta 只給現值的增減量（純數字，例 -10）。")
        } else {
            format!("{shown} 是數字欄，delta 的值要是純數字增減量（例 5、-10）。")
        };
        records.push(Record::new(RecordKind::Rejected, path_str.clone(), detail));
        return;
    };
    match rule.kind {
        FieldKind::Pair => {
            let Some((current_value, max)) = split_pair(&current) else {
                records.push(Record::new(
                    RecordKind::Error,
                    path_str.clone(),
                    format!("{path_str} 現值格式不是「現值/上限」。"),
                ));
                return;
            };
            let min = rule.min.unwrap_or(0.0);
            let raw_next = current_value + delta;
            let next = raw_next.clamp(min, max);
            data::set_tree_value(
                tree,
                &patch.path,
                &format!("{}/{}", format_num(next), format_num(max)),
            );
            changes.insert(path_str.clone(), signed_delta_mark(delta));
            if next != raw_next {
                records.push(Record::new(
                    RecordKind::Clamped,
                    path_str.clone(),
                    format!(
                        "{path_str} 已夾在範圍內，目前值 {}/{}。",
                        format_num(next),
                        format_num(max)
                    ),
                ));
            }
        }
        FieldKind::Number | FieldKind::Counter => {
            let Ok(current_value) = current.trim().parse::<f64>() else {
                records.push(Record::new(
                    RecordKind::Error,
                    path_str.clone(),
                    format!("{path_str} 現值不是數字，無法增減。"),
                ));
                return;
            };
            let mut next = current_value + delta;
            let mut clamped = false;
            if let Some(min) = rule.min {
                if next < min {
                    next = min;
                    clamped = true;
                }
            }
            if let Some(max) = rule.max {
                if next > max {
                    next = max;
                    clamped = true;
                }
            }
            data::set_tree_value(tree, &patch.path, &format_num(next));
            changes.insert(path_str.clone(), signed_delta_mark(delta));
            if clamped {
                records.push(Record::new(
                    RecordKind::Clamped,
                    path_str.clone(),
                    format!("{path_str} 已夾在範圍內，目前值 {}。", format_num(next)),
                ));
            }
        }
        // delta_refusal 已擋下
        FieldKind::Text
        | FieldKind::List
        | FieldKind::Roll
        | FieldKind::ReadOnly
        | FieldKind::Derived => {}
    }
}

/// 這一欄不收 delta 時回傳要回饋給模型的拒收句（`shown`＝提示用路徑）：骰值、唯讀（含衍生值）沿用既有訊息，
/// 文字與清單欄指明改用 replace。數字類欄回 None，交給後面解析增減量。
fn delta_refusal(rule: &FieldRule, shown: &str) -> Option<String> {
    if let Some(detail) = hint::guard_refusal(rule, shown) {
        return Some(detail);
    }
    match rule.kind {
        FieldKind::List => Some(list_refusal(shown)),
        FieldKind::Text => Some(format!(
            "{shown} 是文字欄，不能用 delta；請用 replace 寫新值。"
        )),
        _ => None,
    }
}

fn list_refusal(shown: &str) -> String {
    format!(
        "{shown} 是清單欄，不能用 delta；改任何一行都用 replace 寫出整個欄位的完整字串（沒變的行也照抄）。"
    )
}

fn apply_replace(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patch: &Patch,
    view: &mut TypedView,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) {
    let path_str = patch.path.join(".");
    let Some(node) = data::node_at(tree, &patch.path) else {
        records.push(hint::missing(
            tree,
            mechanism,
            &patch.path,
            PatchOp::Replace,
        ));
        return;
    };
    let current_leaf = leaf_value(node).map(str::to_owned);
    replace_existing(
        tree,
        mechanism,
        &patch.path,
        path_str,
        current_leaf,
        patch.value.as_ref(),
        view,
        records,
        changes,
    );
}

/// Replace 與「Insert 目標已存在」共用的規則：一律照 Replace 的語意走。
#[allow(clippy::too_many_arguments)]
fn replace_existing(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    path: &[String],
    path_str: String,
    current_leaf: Option<String>,
    value: Option<&serde_json::Value>,
    view: &mut TypedView,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) {
    let rule = rule_for(mechanism, path, current_leaf.as_deref());
    let shown = shown_pointer(path);
    // 沒重構的 MVU 卡：replace 一律照上游 set（文字欄也是，型別才對得上），數字欄解除 delta 限制；
    // 本地擲骰、唯讀照舊擋（落到下面的分支）。
    let numeric = matches!(
        rule.kind,
        FieldKind::Number | FieldKind::Counter | FieldKind::Pair
    );
    let upstream = mechanism.numeric_update == NumericUpdate::Upstream
        && (rule.update == UpdateMode::Replace
            || (rule.update == UpdateMode::Delta && numeric && current_leaf.is_some()));
    if upstream {
        if let Some(old) = view.get(path) {
            let new = Val::from_serde(value.unwrap_or(&serde_json::Value::Null));
            let next = upstream_set(&old, new);
            if insert_node(tree, path, next.node()).is_ok() {
                view.set(path, &next);
                changes.insert(path_str, "更新".to_owned());
            }
            return;
        }
    }
    match rule.update {
        UpdateMode::Replace => {
            let node = json_to_node(value.unwrap_or(&serde_json::Value::Null));
            if insert_node(tree, path, node).is_ok() {
                changes.insert(path_str.clone(), "更新".to_owned());
            }
        }
        UpdateMode::Local => {
            records.push(Record::new(
                RecordKind::Rejected,
                path_str.clone(),
                format!("{shown} 由系統本地擲骰，請勿更新。"),
            ));
        }
        UpdateMode::Reject => {
            records.push(Record::new(
                RecordKind::Rejected,
                path_str.clone(),
                format!("{shown} 是唯讀欄位，不接受更新。"),
            ));
        }
        UpdateMode::Delta if current_leaf.is_none() => {
            if let Some(StateNode::Branch(children)) = data::node_at(tree, path) {
                records.push(hint::branch(mechanism, path, children, "整個替換成單一值"));
            }
        }
        UpdateMode::Delta if rule.kind == FieldKind::Pair => {
            let Some(current) = current_leaf else {
                return;
            };
            let Some((current_value, max)) = split_pair(&current) else {
                records.push(Record::new(
                    RecordKind::Error,
                    path_str.clone(),
                    format!("{path_str} 現值格式不是「現值/上限」。"),
                ));
                return;
            };
            let Some((new_value, new_max)) = value.and_then(|v| v.as_str()).and_then(split_pair)
            else {
                records.push(Record::new(
                    RecordKind::Rejected,
                    path_str.clone(),
                    format!("{shown} 新值不是「現值/上限」格式，已忽略。"),
                ));
                return;
            };
            if new_value != current_value {
                records.push(Record::new(
                    RecordKind::Rejected,
                    path_str.clone(),
                    format!(
                        "{shown} 現值 {}/{}，請用增減量（delta）而不是絕對值。",
                        format_num(current_value),
                        format_num(max)
                    ),
                ));
            }
            if new_max != max {
                data::set_tree_value(
                    tree,
                    path,
                    &format!("{}/{}", format_num(current_value), format_num(new_max)),
                );
                changes.insert(path_str.clone(), "更新".to_owned());
            }
        }
        UpdateMode::Delta => {
            let current_display = snippet(&current_leaf.unwrap_or_default(), 40);
            records.push(Record::new(
                RecordKind::Rejected,
                path_str.clone(),
                format!("{shown} 現值 {current_display}，請用增減量（delta）而不是絕對值。"),
            ));
        }
    }
}

fn apply_insert(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patch: &Patch,
    view: &mut TypedView,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) {
    let path_str = patch.path.join(".");
    if let Some(node) = data::node_at(tree, &patch.path) {
        let current_leaf = leaf_value(node).map(str::to_owned);
        replace_existing(
            tree,
            mechanism,
            &patch.path,
            path_str,
            current_leaf,
            patch.value.as_ref(),
            view,
            records,
            changes,
        );
        return;
    }
    // 沒重構 MVU 卡：新值照 stat_data 投影（陣列是一片葉子、之後走得了索引路徑），標記照上游剝掉
    let typed = view
        .is_active()
        .then(|| array::gm_value(patch.value.as_ref()));
    let node = match &typed {
        Some(json) => array::json_node(json),
        None => json_to_node(patch.value.as_ref().unwrap_or(&serde_json::Value::Null)),
    };
    if let Ok(key) = insert_node(tree, &patch.path, node) {
        // 末段 `-` 在樹上落成實際的鍵（目前沒被用掉的最小整數），視圖與變動標記照實際的鍵寫
        let mut written = patch.path.clone();
        if let Some(last) = written.last_mut() {
            *last = key;
        }
        if let Some(json) = typed {
            view.replace_json(&written, json, Vec::new());
        }
        changes.insert(written.join("."), "更新".to_owned());
    } else {
        // 中途撞到葉子（往清單欄 `/神經狀態/-` 插一筆、在單一欄位底下建子欄位）：告訴 GM 該怎麼寫
        records.push(hint::missing(tree, mechanism, &patch.path, PatchOp::Insert));
    }
}

fn apply_remove(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patch: &Patch,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) {
    let path_str = patch.path.join(".");
    if data::node_at(tree, &patch.path).is_none() {
        records.push(hint::missing(tree, mechanism, &patch.path, PatchOp::Remove));
        return;
    }
    // 空字串＝刪除，並沿用 set_tree_value 既有的「因此變空的父分支一併剪掉」行為。
    data::set_tree_value(tree, &patch.path, "");
    changes.insert(path_str, "移除".to_owned());
}

/// 回傳 true＝搬成功。
fn apply_move(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patch: &Patch,
    view: &TypedView,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) -> bool {
    if let Some(record) = array::move_refusal(tree, view, patch) {
        records.push(record);
        return false;
    }
    let Some(node) = take_node(tree, &patch.from) else {
        records.push(hint::missing(tree, mechanism, &patch.from, PatchOp::Move));
        return false;
    };
    let path_str = patch.path.join(".");
    if insert_node(tree, &patch.path, node.clone()).is_ok() {
        changes.insert(path_str, "搬移".to_owned());
        true
    } else {
        let _ = insert_node(tree, &patch.from, node); // 寫不進去就放回原位，不憑空丟資料
        records.push(hint::missing(tree, mechanism, &patch.path, PatchOp::Move));
        false
    }
}

#[cfg(test)]
mod array_tests;
#[cfg(test)]
mod delta_refusal_tests;
#[cfg(test)]
mod hint_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod upstream_tests;
