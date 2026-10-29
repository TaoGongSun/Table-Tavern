//! takeover-list-structured：集合的寫入（形狀把關、具名項目與 `_序`、序列補位、改名、寫入邊界、觸發條件）。
use super::apply::apply_updates;
use super::evaluate_triggers;
use super::test_support::rule;
use super::types::{Patch, PatchOp, RecordKind};
use crate::data::{
    self, Condition, FieldKind, Mechanism, StateNode, Trigger, TriggerCase, TriggerMode,
};
use serde_json::json;
use std::collections::BTreeMap;

/// 北境驿站接管桌：在場人物（具名、物件項目）、行動選項（序列、純量項目）、營地.守衛（巢狀具名）。
fn mechanism() -> Mechanism {
    let mut mechanism = Mechanism {
        incremental: true,
        incremental_by_refactor: true,
        ..Mechanism::default()
    };
    for (path, kind) in [
        ("在場人物", FieldKind::List),
        ("在場人物.*.身份", FieldKind::Text),
        ("在場人物.*.行动", FieldKind::Text),
        ("在場人物.*.体力", FieldKind::Number),
        ("行動選項", FieldKind::List),
        ("行動選項.*", FieldKind::Text),
        ("營地.守衛", FieldKind::List),
        ("營地.守衛.*.職位", FieldKind::Text),
    ] {
        mechanism
            .rules
            .insert(path.to_owned(), rule(kind, Some(0.0), Some(100.0)));
    }
    mechanism.collections = serde_json::from_value(json!({
        "在場人物": { "keyed": true, "item": "object",
                      "fields": { "身份": "text", "行动": "text", "体力": "number" } },
        "行動選項": { "keyed": false, "item": "scalar", "type": "text" },
        "營地.守衛": { "keyed": true, "item": "object", "fields": { "職位": "text" } },
    }))
    .unwrap();
    mechanism
}

fn start() -> BTreeMap<String, StateNode> {
    serde_json::from_value(json!({
        "在場人物": {
            "林远": { "_序": "0", "身份": "新任驿丞", "行动": "牵马", "体力": "55" },
            "贺铮": { "_序": "1", "身份": "老驿长", "行动": "抽烟斗", "体力": "70" },
        },
        "行動選項": { "0": "甲", "1": "乙", "2": "丙" },
        "營地": { "名稱": "北營", "守衛": {} },
        "地點": "前院",
    }))
    .unwrap()
}

fn patch(op: PatchOp, path: &str, value: Option<serde_json::Value>) -> Patch {
    Patch {
        op,
        path: path.split('/').map(str::to_owned).collect(),
        from: Vec::new(),
        value,
    }
}

fn moved(from: &str, to: &str) -> Patch {
    Patch {
        op: PatchOp::Move,
        path: to.split('/').map(str::to_owned).collect(),
        from: from.split('/').map(str::to_owned).collect(),
        value: None,
    }
}

fn node(tree: &BTreeMap<String, StateNode>, path: &str) -> Option<serde_json::Value> {
    let segments: Vec<String> = path.split('.').map(str::to_owned).collect();
    data::node_at(tree, &segments).map(|node| serde_json::to_value(node).unwrap())
}

fn rejected(outcome: &super::Outcome) -> Vec<String> {
    outcome
        .records
        .iter()
        .filter(|record| record.kind == RecordKind::Rejected)
        .map(|record| record.detail.clone())
        .collect()
}

#[test]
fn a_field_change_goes_through_the_normal_rules_with_glyph_folded_names() {
    let mut tree = start();
    let outcome = apply_updates(
        &mut tree,
        &mechanism(),
        &[
            patch(PatchOp::Replace, "在場人物/林遠/行動", Some(json!("核账"))),
            patch(PatchOp::Delta, "在場人物/贺铮/体力", Some(json!(-5))),
        ],
    );
    assert!(rejected(&outcome).is_empty(), "{:?}", outcome.records);
    assert_eq!(node(&tree, "在場人物.林远.行动"), Some(json!("核账")));
    assert_eq!(node(&tree, "在場人物.贺铮.体力"), Some(json!("65")));
    assert!(outcome.changes.contains_key("在場人物.林远.行动"));
}

#[test]
fn new_items_go_last_and_input_order_leaves_and_unknown_fields_are_dropped() {
    let mut tree = start();
    let outcome = apply_updates(
        &mut tree,
        &mechanism(),
        &[
            patch(
                PatchOp::Insert,
                "在場人物/周掌柜",
                Some(
                    json!({ "身份": "军需官", "行動": "捧茶", "体力": 80, "_序": "-9", "外號": "老周" }),
                ),
            ),
            patch(
                PatchOp::Insert,
                "在場人物/商队头领",
                Some(json!({ "身份": "商人" })),
            ),
        ],
    );
    assert_eq!(
        node(&tree, "在場人物.周掌柜"),
        Some(json!({ "_序": "2", "身份": "军需官", "行动": "捧茶", "体力": "80" }))
    );
    assert_eq!(node(&tree, "在場人物.商队头领._序"), Some(json!("3")));
    let notes = rejected(&outcome);
    assert!(
        notes
            .iter()
            .any(|note| note.contains("外號") && note.contains("不是這個集合的欄位")),
        "{notes:?}"
    );
    assert!(
        notes.iter().any(|note| note.contains("缺欄位 体力、行动")),
        "{notes:?}"
    );
}

#[test]
fn inserting_a_glyph_variant_of_an_existing_name_replaces_it_and_keeps_its_place() {
    let mut tree = start();
    apply_updates(
        &mut tree,
        &mechanism(),
        &[patch(
            PatchOp::Insert,
            "在場人物/林遠",
            Some(json!({ "身份": "驿丞", "行动": "看账", "体力": 50, "_序": "7" })),
        )],
    );
    let people = node(&tree, "在場人物").unwrap();
    assert_eq!(people.as_object().unwrap().len(), 2);
    assert_eq!(node(&tree, "在場人物.林远._序"), Some(json!("0")));
    assert_eq!(node(&tree, "在場人物.林远.身份"), Some(json!("驿丞")));
}

/// 既有項目整項寫入（insert 與整份換）只帶部分欄位：沒帶的沿用舊值、不記缺欄位（playdata-loss-fixes #10）。
#[test]
fn rewriting_an_existing_item_keeps_the_fields_it_left_out() {
    let mut tree = start();
    let outcome = apply_updates(
        &mut tree,
        &mechanism(),
        &[
            patch(
                PatchOp::Insert,
                "在場人物/林遠",
                Some(json!({ "行动": "看账" })),
            ),
            patch(
                PatchOp::Replace,
                "在場人物",
                Some(json!({ "贺铮": { "体力": 60 }, "林远": { "身份": "驿丞" } })),
            ),
        ],
    );
    assert_eq!(
        node(&tree, "在場人物.林远"),
        Some(json!({ "_序": "1", "身份": "驿丞", "行动": "看账", "体力": "55" }))
    );
    assert_eq!(
        node(&tree, "在場人物.贺铮"),
        Some(json!({ "_序": "0", "身份": "老驿长", "行动": "抽烟斗", "体力": "60" }))
    );
    assert!(
        !rejected(&outcome)
            .iter()
            .any(|note| note.contains("缺欄位")),
        "{:?}",
        outcome.records
    );
}

#[test]
fn whole_replace_renumbers_in_json_order_and_strings_are_refused() {
    let mut tree = start();
    let outcome = apply_updates(
        &mut tree,
        &mechanism(),
        &[
            patch(
                PatchOp::Replace,
                "在場人物",
                Some(json!({
                    "贺铮": { "身份": "老驿长", "行动": "离开", "体力": 70, "_序": "0" },
                    "林远": { "身份": "驿丞", "行动": "送客", "体力": 55 },
                })),
            ),
            patch(
                PatchOp::Replace,
                "在場人物",
                Some(json!("    - 人物:\n        名字: \"👤 林远\"")),
            ),
        ],
    );
    assert_eq!(node(&tree, "在場人物.贺铮._序"), Some(json!("0")));
    assert_eq!(node(&tree, "在場人物.林远._序"), Some(json!("1")));
    let notes = rejected(&outcome);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("是集合，不是字串"));
}

#[test]
fn removing_the_last_item_keeps_an_empty_collection_that_can_be_refilled() {
    let mut tree = start();
    let mechanism = mechanism();
    apply_updates(
        &mut tree,
        &mechanism,
        &[
            patch(PatchOp::Remove, "在場人物/林远", None),
            patch(PatchOp::Remove, "在場人物/贺铮", None),
        ],
    );
    assert_eq!(node(&tree, "在場人物"), Some(json!({})));
    // 存檔重載：空分支照常序列化、讀回
    let mut tree: BTreeMap<String, StateNode> =
        serde_json::from_str(&serde_json::to_string(&tree).unwrap()).unwrap();
    apply_updates(
        &mut tree,
        &mechanism,
        &[patch(
            PatchOp::Insert,
            "在場人物/林远",
            Some(json!({ "身份": "a", "行动": "b", "体力": 1 })),
        )],
    );
    assert_eq!(node(&tree, "在場人物.林远._序"), Some(json!("0")));
    // 整支被剪掉的集合也能直接新增
    tree.remove("在場人物");
    apply_updates(
        &mut tree,
        &mechanism,
        &[patch(
            PatchOp::Insert,
            "在場人物/贺铮",
            Some(json!({ "身份": "a", "行动": "b", "体力": 1 })),
        )],
    );
    assert_eq!(node(&tree, "在場人物.贺铮._序"), Some(json!("0")));
}

#[test]
fn sequences_shift_on_insert_and_remove_and_sort_past_ten() {
    let mut tree = start();
    let mechanism = mechanism();
    apply_updates(
        &mut tree,
        &mechanism,
        &[
            patch(PatchOp::Remove, "行動選項/0", None),
            patch(PatchOp::Insert, "行動選項/1", Some(json!("插"))),
            patch(PatchOp::Insert, "行動選項/-", Some(json!("尾"))),
        ],
    );
    assert_eq!(
        node(&tree, "行動選項"),
        Some(json!({ "0": "乙", "1": "插", "2": "丙", "3": "尾" }))
    );
    let many: Vec<String> = (0..12).map(|index| format!("選{index}")).collect();
    let outcome = apply_updates(
        &mut tree,
        &mechanism,
        &[
            patch(PatchOp::Replace, "行動選項", Some(json!(many))),
            patch(PatchOp::Remove, "行動選項/9", None),
            patch(PatchOp::Replace, "行動選項/20", Some(json!("x"))),
            patch(PatchOp::Insert, "行動選項/0", Some(json!({ "a": 1 }))),
        ],
    );
    assert_eq!(node(&tree, "行動選項.9"), Some(json!("選10")));
    assert_eq!(node(&tree, "行動選項.10"), Some(json!("選11")));
    assert_eq!(node(&tree, "行動選項.11"), None);
    assert_eq!(rejected(&outcome).len(), 2, "{:?}", outcome.records);
}

#[test]
fn rename_carries_the_order_and_cross_collection_moves_are_refused_without_losing_the_source() {
    let mut tree = start();
    let outcome = apply_updates(
        &mut tree,
        &mechanism(),
        &[
            moved("在場人物/贺铮", "在場人物/贺老"),
            moved("在場人物/林远", "營地/守衛/林远"),
            moved("地點", "在場人物/地點"),
            moved("在場人物/林远", "在場人物/贺老"),
        ],
    );
    assert_eq!(node(&tree, "在場人物.贺老._序"), Some(json!("1")));
    assert_eq!(node(&tree, "在場人物.贺铮"), None);
    assert_eq!(node(&tree, "在場人物.林远._序"), Some(json!("0")));
    assert_eq!(node(&tree, "地點"), Some(json!("前院")));
    assert_eq!(node(&tree, "營地.守衛"), Some(json!({})));
    assert_eq!(rejected(&outcome).len(), 3, "{:?}", outcome.records);
}

#[test]
fn unknown_fields_ancestors_and_missing_items_are_refused() {
    let mut tree = start();
    let before = tree.clone();
    let outcome = apply_updates(
        &mut tree,
        &mechanism(),
        &[
            patch(PatchOp::Replace, "在場人物/林远/未知", Some(json!("x"))),
            patch(PatchOp::Replace, "營地", Some(json!({ "名稱": "南營" }))),
            patch(PatchOp::Remove, "營地", None),
            patch(PatchOp::Replace, "在場人物/周掌柜/行动", Some(json!("x"))),
            patch(
                PatchOp::Replace,
                "在場人物/周掌柜",
                Some(json!({ "身份": "x" })),
            ),
            patch(PatchOp::Insert, "在場人物/-", Some(json!({ "身份": "x" }))),
            patch(
                PatchOp::Insert,
                "在場人物/Dr. Li",
                Some(json!({ "身份": "x" })),
            ),
            patch(PatchOp::Delta, "在場人物", Some(json!(1))),
            patch(PatchOp::Replace, "在場人物/林远/身份/深", Some(json!("x"))),
        ],
    );
    assert_eq!(tree, before);
    let notes = rejected(&outcome);
    assert_eq!(notes.len(), 9, "{notes:?}");
    assert!(notes[1].contains("底下有集合 營地.守衛"), "{notes:?}");
    assert!(notes[3].contains("新項目請用 insert"), "{notes:?}");
    // 祖先底下不是集合的欄位照常可寫
    let outcome = apply_updates(
        &mut tree,
        &mechanism(),
        &[patch(PatchOp::Replace, "營地/名稱", Some(json!("南營")))],
    );
    assert!(rejected(&outcome).is_empty());
    assert_eq!(node(&tree, "營地.名稱"), Some(json!("南營")));
}

#[test]
fn contains_on_a_collection_matches_item_names_and_values_but_not_field_names() {
    let mut mechanism = mechanism();
    let when = |needle: &str| Condition::Contains {
        path: "在場人物".to_owned(),
        any: vec![needle.to_owned()],
    };
    mechanism.triggers = ["贺铮", "抽烟斗", "身份"]
        .iter()
        .map(|needle| Trigger {
            id: (*needle).to_owned(),
            title: (*needle).to_owned(),
            mode: TriggerMode::Range,
            cases: vec![TriggerCase {
                when: vec![when(needle)],
                text: format!("命中{needle}"),
            }],
            preamble: String::new(),
            scope: Vec::new(),
            flag: None,
        })
        .collect();
    let hits = evaluate_triggers(&start(), &mechanism, "阿濤").hits;
    assert_eq!(
        hits.keys().cloned().collect::<Vec<_>>(),
        vec!["抽烟斗", "贺铮"]
    );
}

#[test]
fn a_missing_field_can_be_filled_next_turn_with_replace_as_the_feedback_says() {
    let mechanism = mechanism();
    let mut tree = start();
    let first = apply_updates(
        &mut tree,
        &mechanism,
        &[patch(
            PatchOp::Insert,
            "在場人物/周掌柜",
            Some(json!({ "身份": "军需官" })),
        )],
    );
    let note = rejected(&first).join("");
    assert!(
        note.contains("請用 replace 補上（例 /在場人物/周掌柜/体力）"),
        "{note}"
    );
    let second = apply_updates(
        &mut tree,
        &mechanism,
        &[
            patch(
                PatchOp::Replace,
                "在場人物/周掌柜/行动",
                Some(json!("捧茶")),
            ),
            patch(PatchOp::Replace, "在場人物/周掌柜/体力", Some(json!(80))),
        ],
    );
    assert!(rejected(&second).is_empty(), "{:?}", second.records);
    assert_eq!(node(&tree, "在場人物.周掌柜.行动"), Some(json!("捧茶")));
    assert_eq!(node(&tree, "在場人物.周掌柜.体力"), Some(json!("80")));
}

#[test]
fn a_field_takes_only_a_single_value() {
    let mut tree = start();
    let before = tree.clone();
    let outcome = apply_updates(
        &mut tree,
        &mechanism(),
        &[
            patch(
                PatchOp::Replace,
                "在場人物/林远/行动",
                Some(json!({ "深": "x" })),
            ),
            patch(PatchOp::Insert, "在場人物/林远/身份", Some(json!(["x"]))),
        ],
    );
    assert_eq!(tree, before);
    assert_eq!(rejected(&outcome).len(), 2);
}

#[test]
fn a_whole_replace_with_any_fatal_item_is_refused_and_leaves_the_tree_alone() {
    let cases = [
        json!({ "林远": "牵马" }),
        json!({ "-": { "身份": "x" } }),
        json!({ "林远": { "身份": "x" }, "Dr. Li": { "身份": "y" } }),
        json!({ "林远": { "身份": "x" }, "林遠": { "身份": "y" } }),
    ];
    for value in cases {
        let mut tree = start();
        let before = tree.clone();
        let outcome = apply_updates(
            &mut tree,
            &mechanism(),
            &[patch(PatchOp::Replace, "在場人物", Some(value.clone()))],
        );
        assert_eq!(tree, before, "{value}");
        let notes = rejected(&outcome);
        assert!(
            notes.iter().any(|note| note.contains("整份沒有套用")),
            "{value} {notes:?}"
        );
    }
    let mut tree = start();
    let outcome = apply_updates(
        &mut tree,
        &mechanism(),
        &[patch(
            PatchOp::Replace,
            "在場人物",
            Some(json!({ "林远": { "身份": "x" }, "林遠": { "身份": "y" } })),
        )],
    );
    assert!(rejected(&outcome)
        .iter()
        .any(|note| note.contains("只差字形")));
    let mut tree = start();
    let before = tree.clone();
    apply_updates(
        &mut tree,
        &mechanism(),
        &[patch(
            PatchOp::Replace,
            "行動選項",
            Some(json!(["a", { "b": 1 }])),
        )],
    );
    assert_eq!(tree, before);
}
