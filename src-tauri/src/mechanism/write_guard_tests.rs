//! vn-mode 包 4 §2.3：合法值與受保護根擋住模型更新、面板手改與一次性旗標三個入口。
use super::apply::apply_updates;
use super::evaluate_triggers;
use super::types::{Patch, PatchOp};
use super::{write_refusal, Write};
use crate::data::vn::{normalize, parse_bible, LOCKED_ROOTS};
use crate::data::{self, Mechanism, StateNode, Trigger, TriggerCase, TriggerMode};
use serde_json::json;
use std::collections::BTreeMap;

fn vn_mechanism() -> (Mechanism, BTreeMap<String, StateNode>) {
    let bible = normalize(
        parse_bible(&json!({
            "flags": [
                {"id": "met", "type": "bool"},
                {"id": "trust", "type": "counter", "min": 0, "max": 5},
                {"id": "route", "type": "route", "values": ["a", "b"]}
            ],
            "secrets": [{"id": "s1"}]
        }))
        .unwrap(),
        &[],
    );
    let mut mechanism = Mechanism {
        rules: bible.rules(),
        incremental: true,
        locked_roots: LOCKED_ROOTS.map(str::to_owned).to_vec(),
        ..Mechanism::default()
    };
    mechanism.triggers.clear();
    let mut tree = BTreeMap::new();
    data::vn::fill_missing(&mut tree, &bible.seeds());
    (mechanism, tree)
}

fn patch(op: PatchOp, path: &str, value: Option<serde_json::Value>) -> Patch {
    let split = |path: &str| path.split('.').map(str::to_owned).collect::<Vec<_>>();
    Patch {
        op,
        path: split(path),
        from: if op == PatchOp::Move {
            split("flags.met")
        } else {
            Vec::new()
        },
        value,
        dice: None,
    }
}

/// 合法值內的 replace、範圍內的 delta 收；其餘拒收並在回饋附可用值，樹不動
#[test]
fn allowed_values_and_locked_roots_refuse_with_feedback() {
    let (mechanism, mut tree) = vn_mechanism();
    let outcome = apply_updates(
        &mut tree,
        &mechanism,
        &[
            patch(PatchOp::Replace, "flags.met", Some(json!(true))),
            patch(PatchOp::Delta, "flags.trust", Some(json!(2))),
            patch(PatchOp::Replace, "flags.route", Some(json!("c"))),
            patch(PatchOp::Delta, "revealed.s1", Some(json!(1))),
            patch(PatchOp::Insert, "flags.extra", Some(json!("1"))),
            patch(PatchOp::Remove, "flags.route", None),
            patch(PatchOp::Replace, "flags", Some(json!({}))),
            patch(PatchOp::Move, "other", None),
            patch(PatchOp::Remove, "chapter", None),
        ],
    );
    let leaf = |path: &[&str]| {
        let path: Vec<String> = path.iter().map(|part| part.to_string()).collect();
        data::node_at(&tree, &path).cloned()
    };
    assert_eq!(
        leaf(&["flags", "met"]),
        Some(StateNode::Leaf("true".to_owned()))
    );
    assert_eq!(
        leaf(&["flags", "trust"]),
        Some(StateNode::Leaf("2".to_owned()))
    );
    assert_eq!(
        leaf(&["flags", "route"]),
        Some(StateNode::Leaf("a".to_owned()))
    );
    assert_eq!(leaf(&["chapter"]), Some(StateNode::Leaf("1".to_owned())));
    assert_eq!(leaf(&["flags", "extra"]), None);
    assert_eq!(leaf(&["other"]), None);
    let notes = outcome.notes.join("\n");
    let rejected: Vec<&str> = outcome
        .records
        .iter()
        .filter(|record| record.kind == super::RecordKind::Rejected)
        .map(|record| record.path.as_str())
        .collect();
    assert_eq!(
        rejected,
        [
            "flags.route",
            "revealed.s1",
            "flags.extra",
            "flags.route",
            "flags",
            "flags.met",
            "chapter"
        ]
    );
    assert!(notes.contains("「c」不在可用值內。可用值：a、b"), "{notes}");
    assert!(notes.contains("revealed.s1 只能用 replace"), "{notes}");
    assert!(notes.contains("flags.extra 不是表內欄位"), "{notes}");
    assert!(notes.contains("flags.route 不能刪除或搬移"), "{notes}");
}

/// 面板手改走同一道檢查；沒有保護根的一般桌不受影響
#[test]
fn panel_writes_share_the_check() {
    let (mechanism, _) = vn_mechanism();
    let path = |text: &str| text.split('.').map(str::to_owned).collect::<Vec<_>>();
    assert!(write_refusal(&mechanism, &path("flags.met"), Write::Replace("maybe")).is_some());
    assert!(write_refusal(&mechanism, &path("flags.met"), Write::Other).is_some());
    assert!(write_refusal(&mechanism, &path("flags.met"), Write::Replace("false")).is_none());
    assert!(write_refusal(&Mechanism::default(), &path("flags.x"), Write::Other).is_none());
}

/// 一次性旗標指到不收 true 的欄位時改釘 `_once.<id>`
#[test]
fn once_flag_onto_refused_path_falls_back() {
    let (mut mechanism, tree) = vn_mechanism();
    mechanism.triggers.push(Trigger {
        id: "初見".to_owned(),
        title: "初見".to_owned(),
        mode: TriggerMode::Once,
        cases: vec![TriggerCase {
            when: Vec::new(),
            text: "相遇".to_owned(),
        }],
        preamble: String::new(),
        scope: Vec::new(),
        flag: Some("flags.route".to_owned()),
    });
    let outcome = evaluate_triggers(&tree, &mechanism, "玩家");
    assert_eq!(outcome.flags, ["_once.初見"]);
}

/// ```state 圍欄同一道防線：巢狀非法值、整支清空、唯讀章號都不落地，保留原值並回饋
#[test]
fn fence_fields_and_lists_go_through_the_guard() {
    let (mechanism, tree) = vn_mechanism();
    let mut world = super::test_support::world_with(&[], mechanism);
    world.state.tree = tree;
    let split = |path: &str| path.split('.').map(str::to_owned).collect::<Vec<_>>();
    let block = crate::transport::StateBlock {
        fields: vec![
            (split("flags.met"), "maybe".to_owned()),
            (split("flags.route"), "b".to_owned()),
            (split("chapter"), "5".to_owned()),
        ],
        lists: vec![split("revealed")],
        ..Default::default()
    };
    let outcome = super::apply_block(&mut world, &block, "玩家");
    let leaf = |path: &str| data::node_at(&world.state.tree, &split(path)).cloned();
    assert_eq!(leaf("flags.met"), Some(StateNode::Leaf("false".to_owned())));
    assert_eq!(leaf("flags.route"), Some(StateNode::Leaf("b".to_owned())));
    assert_eq!(leaf("chapter"), Some(StateNode::Leaf("1".to_owned())));
    assert_eq!(
        leaf("revealed.s1"),
        Some(StateNode::Leaf("false".to_owned()))
    );
    let notes = outcome.notes.join("\n");
    assert!(
        notes.contains("「maybe」不在可用值內") && notes.contains("revealed 不是表內欄位"),
        "{notes}"
    );
}
