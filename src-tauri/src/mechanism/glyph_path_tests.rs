//! 模型把路徑換了簡繁字形（`/駐守天數` 對樹上的 `驻守天数`）：精確優先、同層唯一折疊兜底、歧義拒收；
//! insert／move 目的地的最後一段是新鍵，不折。

use super::apply::apply_updates;
use super::test_support::{mechanism_with, rule, tree_from};
use super::types::{Patch, PatchOp, RecordKind};
use crate::data::{self, FieldKind};

fn patch(op: PatchOp, path: &[&str], from: &[&str], value: serde_json::Value) -> Patch {
    Patch {
        op,
        path: path.iter().map(|s| (*s).to_owned()).collect(),
        from: from.iter().map(|s| (*s).to_owned()).collect(),
        value: Some(value),
    }
}

fn leaf(tree: &std::collections::BTreeMap<String, data::StateNode>, path: &str) -> Option<String> {
    let segments: Vec<String> = path.split('.').map(str::to_owned).collect();
    match data::node_at(tree, &segments) {
        Some(data::StateNode::Leaf(value)) => Some(value.clone()),
        _ => None,
    }
}

#[test]
fn folded_path_lands_on_the_existing_key_at_any_depth() {
    let mut tree = tree_from(&[("驻守天数", "1"), ("世界.地点", "城门")]);
    let mechanism = mechanism_with(&[("驻守天数", rule(FieldKind::Counter, Some(0.0), None))]);
    let outcome = apply_updates(
        &mut tree,
        &mechanism,
        &[
            patch(PatchOp::Delta, &["駐守天數"], &[], serde_json::json!(6)),
            patch(
                PatchOp::Replace,
                &["世界", "地點"],
                &[],
                serde_json::json!("冬營地"),
            ),
        ],
    );
    assert!(outcome
        .records
        .iter()
        .all(|record| record.kind != RecordKind::Rejected));
    assert_eq!(leaf(&tree, "驻守天数").as_deref(), Some("7"));
    assert_eq!(leaf(&tree, "世界.地点").as_deref(), Some("冬營地"));
    assert!(leaf(&tree, "駐守天數").is_none() && leaf(&tree, "世界.地點").is_none());
    assert!(outcome.changes.contains_key("驻守天数"));
}

#[test]
fn exact_key_wins_and_fold_collisions_are_rejected() {
    let mut tree = tree_from(&[("沈郁", "1"), ("沈鬱", "2")]);
    let mechanism = mechanism_with(&[]);
    let outcome = apply_updates(
        &mut tree,
        &mechanism,
        &[
            patch(PatchOp::Delta, &["沈鬱"], &[], serde_json::json!(1)),
            patch(PatchOp::Delta, &["瀋鬱"], &[], serde_json::json!(5)),
        ],
    );
    assert_eq!(
        leaf(&tree, "沈鬱").as_deref(),
        Some("3"),
        "原文精確命中不看折疊"
    );
    assert_eq!(
        leaf(&tree, "沈郁").as_deref(),
        Some("1"),
        "歧義不碰任何一個"
    );
    assert!(outcome
        .records
        .iter()
        .any(|record| record.kind == RecordKind::Rejected && record.path == "瀋鬱"));
}

#[test]
fn insert_and_move_targets_keep_their_new_key_but_move_source_folds() {
    let mut tree = tree_from(&[("驻守天数", "1"), ("甲.发带", "红")]);
    let mechanism = mechanism_with(&[]);
    apply_updates(
        &mut tree,
        &mechanism,
        &[
            patch(
                PatchOp::Insert,
                &["駐守天數"],
                &[],
                serde_json::json!("另一欄"),
            ),
            patch(
                PatchOp::Move,
                &["乙", "髮帶"],
                &["甲", "髮帶"],
                serde_json::Value::Null,
            ),
        ],
    );
    assert_eq!(
        leaf(&tree, "驻守天数").as_deref(),
        Some("1"),
        "insert 的新鍵不當成拼錯的舊鍵"
    );
    assert_eq!(leaf(&tree, "駐守天數").as_deref(), Some("另一欄"));
    assert_eq!(
        leaf(&tree, "乙.髮帶").as_deref(),
        Some("红"),
        "move 來源折疊找到、目的地照寫"
    );
    assert!(leaf(&tree, "甲.发带").is_none());
}
