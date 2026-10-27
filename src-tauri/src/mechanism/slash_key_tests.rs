//! flat-field-reporting：鍵名本身含「/」（`剧情阶段/要塞事件`）時 GM 照原字寫成兩段也對得回去；
//! insert 只差字形且對到登記欄位時不長第二份；狀態圍欄寫到樹上已有的欄位走 replace。

use super::apply::apply_updates;
use super::ledger::apply_block;
use super::test_support::{mechanism_with, rule, tree_from, world_with};
use super::types::{Patch, PatchOp, RecordKind};
use crate::data::{self, FieldKind, StateNode};
use crate::transport::StateBlock;
use std::collections::BTreeMap;

fn patch(op: PatchOp, path: &[&str], from: &[&str], value: &str) -> Patch {
    Patch {
        op,
        path: path.iter().map(|s| (*s).to_owned()).collect(),
        from: from.iter().map(|s| (*s).to_owned()).collect(),
        value: Some(serde_json::json!(value)),
    }
}

fn leaf(tree: &BTreeMap<String, StateNode>, path: &[&str]) -> Option<String> {
    let segments: Vec<String> = path.iter().map(|s| (*s).to_owned()).collect();
    match data::node_at(tree, &segments) {
        Some(StateNode::Leaf(value)) => Some(value.clone()),
        _ => None,
    }
}

fn rejected(outcome: &super::types::Outcome) -> usize {
    outcome
        .records
        .iter()
        .filter(|record| record.kind == RecordKind::Rejected)
        .count()
}

#[test]
fn rejoin_takes_the_longest_match_across_lengths() {
    let mut tree = tree_from(&[("a/b", "短"), ("a/b/c", "長")]);
    let outcome = apply_updates(
        &mut tree,
        &mechanism_with(&[]),
        &[patch(PatchOp::Replace, &["a", "b", "c"], &[], "新")],
    );
    assert_eq!(rejected(&outcome), 0, "{:?}", outcome.records);
    assert_eq!(leaf(&tree, &["a/b/c"]).as_deref(), Some("新"));
    assert_eq!(leaf(&tree, &["a/b"]).as_deref(), Some("短"));
}

#[test]
fn rejoin_glyph_folds_but_rejects_same_length_ambiguity() {
    let mut tree = tree_from(&[("剧情阶段/要塞事件", "初")]);
    let outcome = apply_updates(
        &mut tree,
        &mechanism_with(&[]),
        &[patch(
            PatchOp::Replace,
            &["劇情階段", "要塞事件"],
            &[],
            "二",
        )],
    );
    assert_eq!(rejected(&outcome), 0);
    assert_eq!(leaf(&tree, &["剧情阶段/要塞事件"]).as_deref(), Some("二"));

    let mut tree = tree_from(&[("沈郁/x", "1"), ("沈鬱/x", "2")]);
    let before = tree.clone();
    let outcome = apply_updates(
        &mut tree,
        &mechanism_with(&[]),
        &[patch(PatchOp::Replace, &["瀋鬱", "x"], &[], "3")],
    );
    assert_eq!(rejected(&outcome), 1);
    assert_eq!(tree, before);
}

#[test]
fn a_real_branch_wins_over_rejoin() {
    let mut tree = tree_from(&[("剧情阶段.要塞事件", "枝"), ("剧情阶段/要塞事件", "鍵")]);
    apply_updates(
        &mut tree,
        &mechanism_with(&[]),
        &[patch(
            PatchOp::Replace,
            &["剧情阶段", "要塞事件"],
            &[],
            "新",
        )],
    );
    assert_eq!(
        leaf(&tree, &["剧情阶段", "要塞事件"]).as_deref(),
        Some("新")
    );
    assert_eq!(leaf(&tree, &["剧情阶段/要塞事件"]).as_deref(), Some("鍵"));
}

#[test]
fn insert_onto_a_rejoined_key_replaces_and_move_keeps_moving() {
    let mut tree = tree_from(&[("剧情阶段/要塞事件", "初"), ("驻留/交涉角色", "托伦")]);
    let outcome = apply_updates(
        &mut tree,
        &mechanism_with(&[]),
        &[
            patch(PatchOp::Insert, &["剧情阶段", "要塞事件"], &[], "二"),
            patch(PatchOp::Move, &["备份"], &["驻留", "交涉角色"], ""),
        ],
    );
    assert_eq!(rejected(&outcome), 0, "{:?}", outcome.records);
    assert_eq!(leaf(&tree, &["剧情阶段/要塞事件"]).as_deref(), Some("二"));
    assert!(!tree.contains_key("剧情阶段"));
    assert!(!tree.contains_key("驻留/交涉角色"));
    assert_eq!(leaf(&tree, &["备份"]).as_deref(), Some("托伦"));
}

/// 例外二：insert 末段只差字形，對到「精確登記規則」的葉子才接；只有萬用規則命中照原字新建；move 不折。
#[test]
fn glyph_insert_joins_only_exactly_registered_leaves() {
    let text = rule(FieldKind::Text, None, None);
    let mut tree = tree_from(&[("当前环境", "城门")]);
    apply_updates(
        &mut tree,
        &mechanism_with(&[("当前环境", text.clone())]),
        &[patch(PatchOp::Insert, &["當前環境"], &[], "食堂")],
    );
    assert_eq!(leaf(&tree, &["当前环境"]).as_deref(), Some("食堂"));
    assert!(!tree.contains_key("當前環境"));

    let mut tree = tree_from(&[("当前环境", "城门")]);
    apply_updates(
        &mut tree,
        &mechanism_with(&[("*", text.clone())]),
        &[patch(PatchOp::Insert, &["當前環境"], &[], "食堂")],
    );
    assert_eq!(leaf(&tree, &["当前环境"]).as_deref(), Some("城门"));
    assert_eq!(leaf(&tree, &["當前環境"]).as_deref(), Some("食堂"));

    let mut tree = tree_from(&[("当前环境", "城门"), ("舊地點", "北坡")]);
    apply_updates(
        &mut tree,
        &mechanism_with(&[("当前环境", text)]),
        &[patch(PatchOp::Move, &["當前環境"], &["舊地點"], "")],
    );
    assert_eq!(leaf(&tree, &["当前环境"]).as_deref(), Some("城门"));
    assert_eq!(leaf(&tree, &["當前環境"]).as_deref(), Some("北坡"));
}

fn fence(fields: &[(&str, &str)]) -> StateBlock {
    StateBlock {
        fields: fields
            .iter()
            .map(|(key, value)| (vec![(*key).to_owned()], (*value).to_owned()))
            .collect(),
        updates: Vec::new(),
        display: String::new(),
        lists: Vec::new(),
        maps: Vec::new(),
        fenced: None,
    }
}

/// 圍欄：增量桌命中樹上頂層葉子就走 replace 的實際結果（字形唯一也算），多候選拒收、不寫平欄；
/// 固定三鍵與樹上沒有的鍵照舊寫平欄。
#[test]
fn fence_fields_hitting_tree_leaves_go_through_replace() {
    let mut mechanism = mechanism_with(&[
        ("当前环境", rule(FieldKind::Text, None, None)),
        ("驻守天数", rule(FieldKind::Counter, Some(0.0), None)),
        ("体力", rule(FieldKind::Pair, None, None)),
    ]);
    mechanism.incremental = true;
    let mut world = world_with(
        &[
            ("当前环境", "城门"),
            ("驻守天数", "1"),
            ("体力", "5/10"),
            ("沈郁", "甲"),
            ("沈鬱", "乙"),
        ],
        mechanism,
    );
    let outcome = apply_block(
        &mut world,
        &fence(&[
            ("當前環境", "食堂"),
            ("驻守天数", "9"),
            ("体力", "5/12"),
            ("瀋鬱", "丙"),
            ("place", "北坡"),
            ("天气", "小雪"),
        ]),
        "玩家",
    );
    let tree = &world.state.tree;
    assert_eq!(leaf(tree, &["当前环境"]).as_deref(), Some("食堂"));
    assert_eq!(
        leaf(tree, &["驻守天数"]).as_deref(),
        Some("1"),
        "數字欄照 replace 守門"
    );
    assert_eq!(
        leaf(tree, &["体力"]).as_deref(),
        Some("5/12"),
        "Pair 改上限照收"
    );
    assert_eq!(leaf(tree, &["沈郁"]).as_deref(), Some("甲"));
    assert_eq!(leaf(tree, &["沈鬱"]).as_deref(), Some("乙"));
    assert_eq!(rejected(&outcome), 2, "{:?}", outcome.records);
    let table: Vec<&str> = world.state.table.keys().map(String::as_str).collect();
    assert_eq!(table, vec!["place", "天气"]);
}

/// 例外二只限單段 insert：巢狀路徑只差字形照原字新建，不覆寫已登記的舊葉子。
#[test]
fn nested_glyph_insert_keeps_the_registered_leaf() {
    let mut tree = tree_from(&[("人物.当前环境", "城门")]);
    apply_updates(
        &mut tree,
        &mechanism_with(&[("人物.当前环境", rule(FieldKind::Text, None, None))]),
        &[patch(PatchOp::Insert, &["人物", "當前環境"], &[], "食堂")],
    );
    assert_eq!(leaf(&tree, &["人物", "当前环境"]).as_deref(), Some("城门"));
    assert_eq!(leaf(&tree, &["人物", "當前環境"]).as_deref(), Some("食堂"));
}

/// 例外二多個合格候選（都只差字形、都有精確規則）＝歧義拒收。
#[test]
fn glyph_insert_with_several_registered_leaves_is_rejected() {
    let text = rule(FieldKind::Text, None, None);
    let mut tree = tree_from(&[("沈郁", "甲"), ("沈鬱", "乙")]);
    let before = tree.clone();
    let outcome = apply_updates(
        &mut tree,
        &mechanism_with(&[("沈郁", text.clone()), ("沈鬱", text)]),
        &[patch(PatchOp::Insert, &["瀋鬱"], &[], "丙")],
    );
    assert_eq!(rejected(&outcome), 1, "{:?}", outcome.records);
    assert_eq!(tree, before);
}

/// move 目的地照原字寫成兩段也接回含「/」的既有鍵，不另長 `剧情阶段` 枝。
#[test]
fn move_destination_rejoins_the_slash_key() {
    let mut tree = tree_from(&[("剧情阶段/要塞事件", "初"), ("草稿", "二")]);
    let outcome = apply_updates(
        &mut tree,
        &mechanism_with(&[]),
        &[patch(
            PatchOp::Move,
            &["剧情阶段", "要塞事件"],
            &["草稿"],
            "",
        )],
    );
    assert_eq!(rejected(&outcome), 0, "{:?}", outcome.records);
    assert!(!tree.contains_key("剧情阶段"), "{tree:?}");
    assert!(!tree.contains_key("草稿"));
    assert_eq!(leaf(&tree, &["剧情阶段/要塞事件"]).as_deref(), Some("二"));
}
