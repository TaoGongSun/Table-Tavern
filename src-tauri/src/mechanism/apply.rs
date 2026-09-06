use crate::data::{self, FieldKind, Mechanism, StateNode, UpdateMode};
use std::collections::BTreeMap;

use super::rules::rule_for;
use super::tree::{
    build_notes, format_num, insert_node, json_to_node, leaf_value, split_pair, take_node,
    value_as_f64,
};
use super::types::{Outcome, Patch, PatchOp, Record, RecordKind};

// ---------------------------------------------------------------------
// 套用：依欄位規則把 Patch 套進狀態樹
// ---------------------------------------------------------------------

/// 依欄位規則把更新套進狀態樹，回傳記帳、下一輪要給模型的自癒回饋句、以及這一輪真的改到樹的變動。
pub fn apply_updates(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patches: &[Patch],
) -> Outcome {
    let mut records = Vec::new();
    let mut changes = BTreeMap::new();
    for patch in patches {
        apply_one(tree, mechanism, patch, &mut records, &mut changes);
    }
    let notes = build_notes(&records);
    Outcome {
        records,
        notes,
        changes,
    }
}

fn apply_one(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patch: &Patch,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) {
    if let Some(offending) = readonly_violation(patch) {
        records.push(Record::new(
            RecordKind::Rejected,
            offending.clone(),
            format!("{offending} 是唯讀欄位（底線開頭），不接受更新。"),
        ));
        return;
    }
    match patch.op {
        PatchOp::Delta => apply_delta(tree, mechanism, patch, records, changes),
        PatchOp::Replace => apply_replace(tree, mechanism, patch, records, changes),
        PatchOp::Insert => apply_insert(tree, mechanism, patch, records, changes),
        PatchOp::Remove => apply_remove(tree, patch, records, changes),
        PatchOp::Move => apply_move(tree, patch, records, changes),
    }
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

fn readonly_violation(patch: &Patch) -> Option<String> {
    if patch.path.iter().any(|segment| segment.starts_with('_')) {
        return Some(patch.path.join("."));
    }
    if patch.op == PatchOp::Move && patch.from.iter().any(|segment| segment.starts_with('_')) {
        return Some(patch.from.join("."));
    }
    None
}

fn apply_delta(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patch: &Patch,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) {
    let path_str = patch.path.join(".");
    let Some(node) = data::node_at(tree, &patch.path) else {
        records.push(Record::new(
            RecordKind::Error,
            path_str.clone(),
            format!("路徑不存在：{path_str}"),
        ));
        return;
    };
    let Some(current) = leaf_value(node).map(str::to_owned) else {
        records.push(Record::new(
            RecordKind::Error,
            path_str.clone(),
            format!("{path_str} 是分支，不能做增減。"),
        ));
        return;
    };
    let Some(delta) = value_as_f64(patch.value.as_ref()) else {
        records.push(Record::new(
            RecordKind::Error,
            path_str.clone(),
            format!("{path_str} 的更新值不是數字。"),
        ));
        return;
    };
    let rule = rule_for(mechanism, &patch.path, Some(&current));
    match rule.update {
        UpdateMode::Local => {
            records.push(Record::new(
                RecordKind::Rejected,
                path_str.clone(),
                format!("{path_str} 由系統本地擲骰，請勿更新。"),
            ));
            return;
        }
        UpdateMode::Reject => {
            records.push(Record::new(
                RecordKind::Rejected,
                path_str.clone(),
                format!("{path_str} 是唯讀欄位，不接受更新。"),
            ));
            return;
        }
        UpdateMode::Delta | UpdateMode::Replace => {}
    }
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
        FieldKind::Text
        | FieldKind::List
        | FieldKind::Roll
        | FieldKind::ReadOnly
        | FieldKind::Derived => {
            records.push(Record::new(
                RecordKind::Error,
                path_str.clone(),
                format!("{path_str} 是文字欄位，不能做增減。"),
            ));
        }
    }
}

fn apply_replace(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patch: &Patch,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) {
    let path_str = patch.path.join(".");
    let Some(node) = data::node_at(tree, &patch.path) else {
        records.push(Record::new(
            RecordKind::Error,
            path_str.clone(),
            format!("路徑不存在：{path_str}"),
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
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) {
    let rule = rule_for(mechanism, path, current_leaf.as_deref());
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
                format!("{path_str} 由系統本地擲骰，請勿更新。"),
            ));
        }
        UpdateMode::Reject => {
            records.push(Record::new(
                RecordKind::Rejected,
                path_str.clone(),
                format!("{path_str} 是唯讀欄位，不接受更新。"),
            ));
        }
        UpdateMode::Delta if rule.kind == FieldKind::Pair => {
            let Some(current) = current_leaf else {
                records.push(Record::new(
                    RecordKind::Error,
                    path_str.clone(),
                    format!("{path_str} 是分支，不能替換。"),
                ));
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
                    format!("{path_str} 新值不是「現值/上限」格式，已忽略。"),
                ));
                return;
            };
            if new_value != current_value {
                records.push(Record::new(
                    RecordKind::Rejected,
                    path_str.clone(),
                    format!(
                        "{path_str} 現值 {}/{}，請用增減量（delta）而不是絕對值。",
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
            let current_display = current_leaf.unwrap_or_default();
            records.push(Record::new(
                RecordKind::Rejected,
                path_str.clone(),
                format!("{path_str} 現值 {current_display}，請用增減量（delta）而不是絕對值。"),
            ));
        }
    }
}

fn apply_insert(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patch: &Patch,
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
            records,
            changes,
        );
        return;
    }
    let node = json_to_node(patch.value.as_ref().unwrap_or(&serde_json::Value::Null));
    if insert_node(tree, &patch.path, node).is_ok() {
        changes.insert(path_str, "更新".to_owned());
    } else {
        records.push(Record::new(
            RecordKind::Error,
            path_str.clone(),
            format!("{path_str} 中間層已被其他欄位占用，無法建立。"),
        ));
    }
}

fn apply_remove(
    tree: &mut BTreeMap<String, StateNode>,
    patch: &Patch,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) {
    let path_str = patch.path.join(".");
    if data::node_at(tree, &patch.path).is_none() {
        records.push(Record::new(
            RecordKind::Error,
            path_str.clone(),
            format!("路徑不存在，無法刪除：{path_str}"),
        ));
        return;
    }
    // 空字串＝刪除，並沿用 set_tree_value 既有的「因此變空的父分支一併剪掉」行為。
    data::set_tree_value(tree, &patch.path, "");
    changes.insert(path_str, "移除".to_owned());
}

fn apply_move(
    tree: &mut BTreeMap<String, StateNode>,
    patch: &Patch,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) {
    let from_str = patch.from.join(".");
    let Some(node) = take_node(tree, &patch.from) else {
        records.push(Record::new(
            RecordKind::Error,
            from_str.clone(),
            format!("路徑不存在：{from_str}"),
        ));
        return;
    };
    let path_str = patch.path.join(".");
    if insert_node(tree, &patch.path, node.clone()).is_ok() {
        changes.insert(path_str, "搬移".to_owned());
    } else {
        let _ = insert_node(tree, &patch.from, node); // 寫不進去就放回原位，不憑空丟資料
        records.push(Record::new(
            RecordKind::Error,
            path_str.clone(),
            format!("{path_str} 中間層已被其他欄位占用，搬移失敗。"),
        ));
    }
}

#[cfg(test)]
mod tests;
