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
mod tests {
    use super::*;
    use crate::data::{FieldRule, InjectLevel};
    use crate::mechanism::parse::parse_updates;
    use crate::mechanism::test_support::{mechanism_with, rule, tree_from};

    // ---- apply_updates：五種 op 各一條 ----

    #[test]
    fn delta_adds_to_current_value_within_bounds() {
        let mut tree = tree_from(&[("World.HP", "10")]);
        let mechanism =
            mechanism_with(&[("World.HP", rule(FieldKind::Number, Some(0.0), Some(100.0)))]);
        let patches = vec![Patch {
            op: PatchOp::Delta,
            path: vec!["World".to_owned(), "HP".to_owned()],
            from: Vec::new(),
            value: Some(serde_json::json!(5)),
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert!(outcome.records.is_empty());
        assert_eq!(tree.get("World"), {
            let mut inner = BTreeMap::new();
            inner.insert("HP".to_owned(), StateNode::Leaf("15".to_owned()));
            Some(&StateNode::Branch(inner))
        });
    }

    #[test]
    fn replace_overwrites_text_field() {
        let mut tree = tree_from(&[("World.Location", "舊城")]);
        let mechanism = Mechanism::default();
        let patches = vec![Patch {
            op: PatchOp::Replace,
            path: vec!["World".to_owned(), "Location".to_owned()],
            from: Vec::new(),
            value: Some(serde_json::json!("晨港")),
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert!(outcome.records.is_empty());
        assert_eq!(
            tree.get("World"),
            Some(&StateNode::Branch(BTreeMap::from([(
                "Location".to_owned(),
                StateNode::Leaf("晨港".to_owned())
            )])))
        );
    }

    #[test]
    fn insert_creates_missing_branches_and_leaf() {
        let mut tree: BTreeMap<String, StateNode> = BTreeMap::new();
        let mechanism = Mechanism::default();
        let patches = vec![Patch {
            op: PatchOp::Insert,
            path: vec![
                "Player".to_owned(),
                "Inventory".to_owned(),
                "藥水".to_owned(),
            ],
            from: Vec::new(),
            value: Some(serde_json::json!({ "數量": 2 })),
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert!(outcome.records.is_empty());
        let StateNode::Branch(player) = tree.get("Player").unwrap() else {
            panic!("Player 應該是分支");
        };
        let StateNode::Branch(inventory) = player.get("Inventory").unwrap() else {
            panic!("Inventory 應該是分支");
        };
        assert!(inventory.contains_key("藥水"));
    }

    #[test]
    fn remove_deletes_leaf_and_prunes_empty_parent() {
        let mut tree = tree_from(&[("Player.Inventory.舊劍", "鏽")]);
        let mechanism = Mechanism::default();
        let patches = vec![Patch {
            op: PatchOp::Remove,
            path: vec![
                "Player".to_owned(),
                "Inventory".to_owned(),
                "舊劍".to_owned(),
            ],
            from: Vec::new(),
            value: None,
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert!(outcome.records.is_empty());
        // Inventory 底下只有這一項，刪掉後 Inventory、Player 都該一併被剪掉。
        assert!(!tree.contains_key("Player"));
    }

    #[test]
    fn move_relocates_subtree_without_recording_anything() {
        let mut tree = tree_from(&[("NPCs.A.HP", "10")]);
        let mechanism = Mechanism::default();
        let patches = vec![Patch {
            op: PatchOp::Move,
            path: vec!["Heroes".to_owned(), "鴉".to_owned()],
            from: vec!["NPCs".to_owned(), "A".to_owned()],
            value: None,
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert!(outcome.records.is_empty());
        assert!(!tree.contains_key("NPCs"));
        let StateNode::Branch(heroes) = tree.get("Heroes").unwrap() else {
            panic!("Heroes 應該是分支");
        };
        assert!(heroes.contains_key("鴉"));
    }

    // ---- 拒收與夾邊界 ----

    #[test]
    fn replace_absolute_value_on_number_field_is_rejected_and_local_state_unchanged() {
        let mut tree = tree_from(&[("World.HP", "80")]);
        let mechanism =
            mechanism_with(&[("World.HP", rule(FieldKind::Number, Some(0.0), Some(100.0)))]);
        let patches = vec![Patch {
            op: PatchOp::Replace,
            path: vec!["World".to_owned(), "HP".to_owned()],
            from: Vec::new(),
            value: Some(serde_json::json!(999)),
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert_eq!(outcome.records.len(), 1);
        assert_eq!(outcome.records[0].kind, RecordKind::Rejected);
        let StateNode::Branch(world) = tree.get("World").unwrap() else {
            panic!("World 應該是分支");
        };
        assert_eq!(world.get("HP"), Some(&StateNode::Leaf("80".to_owned())));
    }

    #[test]
    fn delta_clamps_to_max_and_records_clamped() {
        let mut tree = tree_from(&[("World.HP", "95")]);
        let mechanism =
            mechanism_with(&[("World.HP", rule(FieldKind::Number, Some(0.0), Some(100.0)))]);
        let patches = vec![Patch {
            op: PatchOp::Delta,
            path: vec!["World".to_owned(), "HP".to_owned()],
            from: Vec::new(),
            value: Some(serde_json::json!(50)),
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert_eq!(outcome.records.len(), 1);
        assert_eq!(outcome.records[0].kind, RecordKind::Clamped);
        let StateNode::Branch(world) = tree.get("World").unwrap() else {
            panic!("World 應該是分支");
        };
        assert_eq!(world.get("HP"), Some(&StateNode::Leaf("100".to_owned())));
    }

    #[test]
    fn pair_delta_moves_current_value_and_replace_only_changes_cap() {
        let mut tree = tree_from(&[("亚瑟·晨光.HP", "480/500")]);
        let mechanism = mechanism_with(&[(
            "亚瑟·晨光.HP",
            rule(FieldKind::Pair, Some(0.0), Some(500.0)),
        )]);
        let delta_patch = Patch {
            op: PatchOp::Delta,
            path: vec!["亚瑟·晨光".to_owned(), "HP".to_owned()],
            from: Vec::new(),
            value: Some(serde_json::json!(-30)),
        };
        let outcome = apply_updates(&mut tree, &mechanism, std::slice::from_ref(&delta_patch));
        assert!(outcome.records.is_empty());
        let StateNode::Branch(character) = tree.get("亚瑟·晨光").unwrap() else {
            panic!("角色應該是分支");
        };
        assert_eq!(
            character.get("HP"),
            Some(&StateNode::Leaf("450/500".to_owned()))
        );

        // replace 只認上限：升級成 450/600（現值不變才收，現值變了要記一筆）。
        let upgrade_patch = Patch {
            op: PatchOp::Replace,
            path: vec!["亚瑟·晨光".to_owned(), "HP".to_owned()],
            from: Vec::new(),
            value: Some(serde_json::json!("450/600")),
        };
        let outcome = apply_updates(&mut tree, &mechanism, std::slice::from_ref(&upgrade_patch));
        assert!(outcome.records.is_empty());
        let StateNode::Branch(character) = tree.get("亚瑟·晨光").unwrap() else {
            panic!("角色應該是分支");
        };
        assert_eq!(
            character.get("HP"),
            Some(&StateNode::Leaf("450/600".to_owned()))
        );

        // replace 想連現值一起改：上限照改，但現值變動被拒收，本地帳沿用舊現值。
        let bad_patch = Patch {
            op: PatchOp::Replace,
            path: vec!["亚瑟·晨光".to_owned(), "HP".to_owned()],
            from: Vec::new(),
            value: Some(serde_json::json!("999/700")),
        };
        let outcome = apply_updates(&mut tree, &mechanism, std::slice::from_ref(&bad_patch));
        assert_eq!(outcome.records.len(), 1);
        assert_eq!(outcome.records[0].kind, RecordKind::Rejected);
        assert_eq!(
            outcome.records[0].detail,
            "亚瑟·晨光.HP 現值 450/600，請用增減量（delta）而不是絕對值。"
        );
        let StateNode::Branch(character) = tree.get("亚瑟·晨光").unwrap() else {
            panic!("角色應該是分支");
        };
        assert_eq!(
            character.get("HP"),
            Some(&StateNode::Leaf("450/700".to_owned()))
        );
    }

    #[test]
    fn delta_on_missing_path_is_a_hard_error() {
        let mut tree: BTreeMap<String, StateNode> = BTreeMap::new();
        let mechanism = Mechanism::default();
        let patches = vec![Patch {
            op: PatchOp::Delta,
            path: vec!["World".to_owned(), "HP".to_owned()],
            from: Vec::new(),
            value: Some(serde_json::json!(5)),
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert_eq!(outcome.records.len(), 1);
        assert_eq!(outcome.records[0].kind, RecordKind::Error);
    }

    #[test]
    fn delta_on_text_field_is_a_hard_error() {
        let mut tree = tree_from(&[("World.Location", "晨港")]);
        let mechanism = Mechanism::default();
        let patches = vec![Patch {
            op: PatchOp::Delta,
            path: vec!["World".to_owned(), "Location".to_owned()],
            from: Vec::new(),
            value: Some(serde_json::json!(5)),
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert_eq!(outcome.records.len(), 1);
        assert_eq!(outcome.records[0].kind, RecordKind::Error);
        let StateNode::Branch(world) = tree.get("World").unwrap() else {
            panic!("World 應該是分支");
        };
        assert_eq!(
            world.get("Location"),
            Some(&StateNode::Leaf("晨港".to_owned()))
        );
    }

    #[test]
    fn remove_on_missing_path_is_a_hard_error() {
        let mut tree: BTreeMap<String, StateNode> = BTreeMap::new();
        let mechanism = Mechanism::default();
        let patches = vec![Patch {
            op: PatchOp::Remove,
            path: vec![
                "Player".to_owned(),
                "Inventory".to_owned(),
                "舊劍".to_owned(),
            ],
            from: Vec::new(),
            value: None,
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert_eq!(outcome.records.len(), 1);
        assert_eq!(outcome.records[0].kind, RecordKind::Error);
    }

    #[test]
    fn underscore_prefixed_field_is_rejected_and_untouched() {
        let mut tree = tree_from(&[("World._Secret", "隱藏")]);
        let mechanism = Mechanism::default();
        let patches = vec![Patch {
            op: PatchOp::Replace,
            path: vec!["World".to_owned(), "_Secret".to_owned()],
            from: Vec::new(),
            value: Some(serde_json::json!("洩漏")),
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert_eq!(outcome.records.len(), 1);
        assert_eq!(outcome.records[0].kind, RecordKind::Rejected);
        let StateNode::Branch(world) = tree.get("World").unwrap() else {
            panic!("World 應該是分支");
        };
        assert_eq!(
            world.get("_Secret"),
            Some(&StateNode::Leaf("隱藏".to_owned()))
        );
    }

    #[test]
    fn wildcard_rule_clamps_any_hero_affection_between_0_and_200() {
        let mut tree = tree_from(&[("Heroes.亚瑟·晨光.Affection", "190")]);
        let mechanism = mechanism_with(&[(
            "Heroes.*.Affection",
            rule(FieldKind::Number, Some(0.0), Some(200.0)),
        )]);
        let patches = vec![Patch {
            op: PatchOp::Delta,
            path: vec![
                "Heroes".to_owned(),
                "亚瑟·晨光".to_owned(),
                "Affection".to_owned(),
            ],
            from: Vec::new(),
            value: Some(serde_json::json!(50)),
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert_eq!(outcome.records.len(), 1);
        assert_eq!(outcome.records[0].kind, RecordKind::Clamped);
        let StateNode::Branch(heroes) = tree.get("Heroes").unwrap() else {
            panic!("Heroes 應該是分支");
        };
        let StateNode::Branch(hero) = heroes.get("亚瑟·晨光").unwrap() else {
            panic!("角色應該是分支");
        };
        assert_eq!(
            hero.get("Affection"),
            Some(&StateNode::Leaf("200".to_owned()))
        );
    }

    // ---- notes：只從 Rejected 產生 ----

    #[test]
    fn notes_are_deduped_and_capped_at_five() {
        let mut tree = tree_from(&[("World.Roll100", "42")]);
        let mechanism = mechanism_with(&[(
            "World.Roll100",
            FieldRule {
                kind: FieldKind::Roll,
                min: Some(1.0),
                max: Some(100.0),
                update: crate::data::UpdateMode::Local,
                inject: InjectLevel::Turn,
                branch: None,
                formula: None,
            },
        )]);
        let patch = Patch {
            op: PatchOp::Delta,
            path: vec!["World".to_owned(), "Roll100".to_owned()],
            from: Vec::new(),
            value: Some(serde_json::json!(1)),
        };
        let patches = vec![patch.clone(), patch.clone(), patch];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert_eq!(outcome.records.len(), 3);
        assert_eq!(
            outcome.notes,
            vec!["World.Roll100 由系統本地擲骰，請勿更新。".to_owned()]
        );
    }

    // ---- 整合測試：從一段完整的 <UpdateVariable> 原文一路跑到樹被更新 ----

    #[test]
    fn full_update_variable_block_parses_and_applies_to_the_tree() {
        let mut tree = tree_from(&[
            ("World.Location", "舊港"),
            ("Heroes.亚瑟·晨光.Affection", "10"),
            ("Player.Inventory.舊劍", "鏽蝕"),
        ]);
        let mechanism = mechanism_with(&[(
            "Heroes.*.Affection",
            rule(FieldKind::Number, Some(0.0), Some(200.0)),
        )]);
        let block = r#"<Analysis>Some english reasoning about the scene.</Analysis>
<JSONPatch>
[
  { "op": "replace", "path": "/World/Location", "value": "晨港" },
  { "op": "delta",   "path": "/Heroes/亚瑟·晨光/Affection", "value": 5 },
  { "op": "insert",  "path": "/Player/Inventory/藥水", "value": { "描述": "紅色的", "數量": 2 } },
  { "op": "remove",  "path": "/Player/Inventory/舊劍" }
]
</JSONPatch>"#;
        let patches = parse_updates(block);
        assert_eq!(patches.len(), 4);
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert!(outcome.records.is_empty());

        let StateNode::Branch(world) = tree.get("World").unwrap() else {
            panic!("World 應該是分支");
        };
        assert_eq!(
            world.get("Location"),
            Some(&StateNode::Leaf("晨港".to_owned()))
        );

        let StateNode::Branch(heroes) = tree.get("Heroes").unwrap() else {
            panic!("Heroes 應該是分支");
        };
        let StateNode::Branch(hero) = heroes.get("亚瑟·晨光").unwrap() else {
            panic!("角色應該是分支");
        };
        assert_eq!(
            hero.get("Affection"),
            Some(&StateNode::Leaf("15".to_owned()))
        );

        let StateNode::Branch(player) = tree.get("Player").unwrap() else {
            panic!("Player 應該是分支");
        };
        let StateNode::Branch(inventory) = player.get("Inventory").unwrap() else {
            panic!("Inventory 應該是分支");
        };
        assert!(inventory.contains_key("藥水"));
        assert!(!inventory.contains_key("舊劍"));
    }

    // ---- outcome.changes：只有真的改到樹的更新才記帳（狀態欄二期包 5）----

    #[test]
    fn accepted_delta_records_signed_mark_but_rejection_and_error_do_not() {
        let mut tree = tree_from(&[("World.HP", "80"), ("World.Location", "舊城")]);
        let mechanism =
            mechanism_with(&[("World.HP", rule(FieldKind::Number, Some(0.0), Some(100.0)))]);
        let patches = vec![
            // 收下的 delta：標記照原始量寫，即使之後會被夾邊界。
            Patch {
                op: PatchOp::Delta,
                path: vec!["World".to_owned(), "HP".to_owned()],
                from: Vec::new(),
                value: Some(serde_json::json!(50)),
            },
            // 絕對值頂替 delta 欄＝拒收，不進 changes。
            Patch {
                op: PatchOp::Replace,
                path: vec!["World".to_owned(), "HP".to_owned()],
                from: Vec::new(),
                value: Some(serde_json::json!(999)),
            },
            // 路徑不存在＝硬錯誤，不進 changes。
            Patch {
                op: PatchOp::Delta,
                path: vec!["World".to_owned(), "沒有這欄".to_owned()],
                from: Vec::new(),
                value: Some(serde_json::json!(1)),
            },
        ];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert_eq!(
            outcome.changes.get("World.HP"),
            Some(&"+50".to_owned()),
            "標記要帶原始 delta（+50），不是夾完的 +20"
        );
        assert_eq!(outcome.changes.len(), 1, "拒收與硬錯誤都不該進 changes");
    }

    #[test]
    fn negative_delta_and_replace_insert_remove_move_each_record_the_documented_mark() {
        let mut tree = tree_from(&[
            ("World.HP", "480/500"),
            ("World.Location", "舊城"),
            ("Player.Inventory.舊劍", "鏽"),
            ("NPCs.A.HP", "10"),
        ]);
        let mechanism =
            mechanism_with(&[("World.HP", rule(FieldKind::Pair, Some(0.0), Some(500.0)))]);
        let patches = vec![
            // pair delta：負數本身帶「-」，標記不再重複加號。
            Patch {
                op: PatchOp::Delta,
                path: vec!["World".to_owned(), "HP".to_owned()],
                from: Vec::new(),
                value: Some(serde_json::json!(-80)),
            },
            // replace：文字欄收下＝「更新」。
            Patch {
                op: PatchOp::Replace,
                path: vec!["World".to_owned(), "Location".to_owned()],
                from: Vec::new(),
                value: Some(serde_json::json!("晨港")),
            },
            // insert：新建路徑＝「更新」。
            Patch {
                op: PatchOp::Insert,
                path: vec![
                    "Player".to_owned(),
                    "Inventory".to_owned(),
                    "藥水".to_owned(),
                ],
                from: Vec::new(),
                value: Some(serde_json::json!({ "數量": 2 })),
            },
            // remove：刪除既有路徑＝「移除」。
            Patch {
                op: PatchOp::Remove,
                path: vec![
                    "Player".to_owned(),
                    "Inventory".to_owned(),
                    "舊劍".to_owned(),
                ],
                from: Vec::new(),
                value: None,
            },
            // move：搬移成功＝「搬移」，記在目的地路徑。
            Patch {
                op: PatchOp::Move,
                path: vec!["Heroes".to_owned(), "鴉".to_owned()],
                from: vec!["NPCs".to_owned(), "A".to_owned()],
                value: None,
            },
        ];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert_eq!(outcome.changes.get("World.HP"), Some(&"-80".to_owned()));
        assert_eq!(
            outcome.changes.get("World.Location"),
            Some(&"更新".to_owned())
        );
        assert_eq!(
            outcome.changes.get("Player.Inventory.藥水"),
            Some(&"更新".to_owned())
        );
        assert_eq!(
            outcome.changes.get("Player.Inventory.舊劍"),
            Some(&"移除".to_owned())
        );
        assert_eq!(outcome.changes.get("Heroes.鴉"), Some(&"搬移".to_owned()));
    }
}
