//! mvu-array-index-path：沒重構 MVU 卡的陣列索引路徑——守門、拒收句、變動標記、夾值、樹模式投影。
//! 與前端墊片逐項對照的套用結果在 `mvu-replace-parity.json`（`replace_parity_tests`）。
use super::*;
use crate::data::FieldRule;
use crate::mechanism::note::NOTE_MAX;
use crate::mechanism::parse::parse_updates;
use crate::mechanism::test_support::{mechanism_with, rule, tree_from};

fn segments(pointer: &str) -> Vec<String> {
    pointer
        .trim_start_matches('/')
        .split('/')
        .map(str::to_owned)
        .collect()
}

fn patch(op: PatchOp, pointer: &str, value: Option<serde_json::Value>) -> Patch {
    Patch {
        op,
        path: segments(pointer),
        from: Vec::new(),
        value,
    }
}

fn replace(pointer: &str, value: serde_json::Value) -> Patch {
    patch(PatchOp::Replace, pointer, Some(value))
}

fn delta(pointer: &str, value: serde_json::Value) -> Patch {
    patch(PatchOp::Delta, pointer, Some(value))
}

fn upstream(rules: &[(&str, FieldRule)]) -> Mechanism {
    let mut mechanism = mechanism_with(rules);
    mechanism.numeric_update = NumericUpdate::Upstream;
    mechanism
}

fn run(
    leaves: &[(&str, &str)],
    mechanism: &Mechanism,
    patches: &[Patch],
) -> (BTreeMap<String, StateNode>, Outcome) {
    let mut tree = tree_from(leaves);
    let outcome = apply_updates(&mut tree, mechanism, patches);
    (tree, outcome)
}

fn only_note(outcome: &Outcome) -> &str {
    assert_eq!(outcome.notes.len(), 1, "{:?}", outcome.records);
    &outcome.notes[0]
}

#[test]
fn vwd_first_item_delta_clamps_by_the_field_rule_like_delta_on_the_field() {
    let mut ranged = rule(FieldKind::Number, Some(0.0), Some(100.0));
    ranged.update = UpdateMode::Delta;
    let (tree, outcome) = run(
        &[("HP", r#"[95,"生命"]"#)],
        &upstream(&[("HP", ranged)]),
        &[delta("/HP/0", serde_json::json!(10))],
    );
    assert_eq!(tree, tree_from(&[("HP", r#"[100,"生命"]"#)]));
    assert_eq!(outcome.changes.get("HP").map(String::as_str), Some("+10"));
    assert_eq!(outcome.records.len(), 1);
    assert_eq!(outcome.records[0].kind, RecordKind::Clamped);
    assert_eq!(outcome.records[0].path, "HP");
}

/// 巢在陣列元素裡的 [值, 說明]（`delta /A/0/HP/0`）：辨認實際的父 X（A.0.HP），套 X 的萬用規則夾值、記在 X，寫回外層陣列。
#[test]
fn nested_vwd_first_item_delta_uses_the_rule_of_its_own_field() {
    let (tree, outcome) = run(
        &[("A", r#"[{"HP":[95,"生命"],"名":"甲"}]"#)],
        &upstream(&[("A.*.HP", rule(FieldKind::Number, Some(0.0), Some(100.0)))]),
        &[delta("/A/0/HP/0", serde_json::json!(10))],
    );
    assert_eq!(
        tree,
        tree_from(&[("A", r#"[{"HP":[100,"生命"],"名":"甲"}]"#)])
    );
    assert_eq!(outcome.changes.get("A").map(String::as_str), Some("更新"));
    assert_eq!(outcome.records.len(), 1);
    assert_eq!(outcome.records[0].kind, RecordKind::Clamped);
    assert_eq!(outcome.records[0].path, "A.0.HP");
}

#[test]
fn nested_vwd_first_item_delta_answers_guards_of_the_wildcard_rule() {
    for (kind, expected) in [
        (FieldKind::Roll, "/A/0/HP/0 由系統本地擲骰，請勿更新。"),
        (FieldKind::ReadOnly, "/A/0/HP/0 是唯讀欄位，不接受更新。"),
    ] {
        let leaves = [("A", r#"[{"HP":[95,"生命"]}]"#)];
        let (tree, outcome) = run(
            &leaves,
            &upstream(&[("A.*.HP", rule(kind, None, None))]),
            &[delta("/A/0/HP/0", serde_json::json!(1))],
        );
        assert_eq!(tree, tree_from(&leaves));
        assert_eq!(only_note(&outcome), expected);
        assert_eq!(outcome.records[0].path, "A.0.HP.0");
    }
}

#[test]
fn element_vwd_and_plain_elements_add_without_clamping() {
    let (tree, outcome) = run(
        &[("列表", r#"[[10,"x"],7]"#)],
        &upstream(&[("列表", rule(FieldKind::Number, Some(0.0), Some(5.0)))]),
        &[
            delta("/列表/0", serde_json::json!(2)),
            delta("/列表/1", serde_json::json!(100)),
        ],
    );
    assert!(outcome.records.is_empty(), "{:?}", outcome.records);
    assert_eq!(tree, tree_from(&[("列表", r#"[[12,"x"],107]"#)]));
    assert_eq!(
        outcome.changes.get("列表").map(String::as_str),
        Some("更新")
    );
}

#[test]
fn move_into_or_out_of_an_array_is_refused_and_nothing_moves() {
    let leaves = [("物品", r#"["劍","盾"]"#), ("其他", "x")];
    for (from, to) in [("/物品/0", "/新"), ("/其他", "/物品/1")] {
        let (tree, outcome) = run(
            &leaves,
            &upstream(&[]),
            &[Patch {
                op: PatchOp::Move,
                path: segments(to),
                from: segments(from),
                value: None,
            }],
        );
        assert_eq!(tree, tree_from(&leaves));
        let note = only_note(&outcome);
        assert!(
            note.contains(
                "是陣列項目，不能 move；請 remove 後 insert，或 replace /物品 寫出整個陣列"
            ),
            "{note}"
        );
    }
}

#[test]
fn insert_with_a_non_index_last_segment_is_refused() {
    let (tree, outcome) = run(
        &[("物品", r#"["劍"]"#)],
        &upstream(&[]),
        &[patch(
            PatchOp::Insert,
            "/物品/abc",
            Some(serde_json::json!("盾")),
        )],
    );
    assert_eq!(tree, tree_from(&[("物品", r#"["劍"]"#)]));
    assert_eq!(
        only_note(&outcome),
        "/物品 是陣列，insert 的最後一段要寫索引或 -（例 /物品/-）。"
    );
    assert_eq!(outcome.records[0].path, "物品.abc");
}

#[test]
fn guards_on_the_array_field_answer_first_and_list_rules_still_apply() {
    for (kind, expected) in [
        (FieldKind::Roll, "/骰/0 由系統本地擲骰，請勿更新。"),
        (FieldKind::ReadOnly, "/骰/0 是唯讀欄位，不接受更新。"),
        (FieldKind::Derived, "/骰/0 是唯讀欄位，不接受更新。"),
    ] {
        let (_, outcome) = run(
            &[("骰", "[3,4]")],
            &upstream(&[("骰", rule(kind, None, None))]),
            &[replace("/骰/0", serde_json::json!(6))],
        );
        assert_eq!(only_note(&outcome), expected.replace("/骰/0", "/骰"));
    }
    let (tree, outcome) = run(
        &[("物品", r#"["劍"]"#)],
        &upstream(&[("物品", rule(FieldKind::List, None, None))]),
        &[patch(
            PatchOp::Insert,
            "/物品/-",
            Some(serde_json::json!("盾")),
        )],
    );
    assert!(outcome.records.is_empty(), "{:?}", outcome.records);
    assert_eq!(tree, tree_from(&[("物品", r#"["劍","盾"]"#)]));
}

#[test]
fn missing_index_says_how_many_items_and_offers_the_whole_array() {
    let (_, outcome) = run(
        &[("物品", r#"["劍","盾","弓"]"#), ("空", "[]")],
        &upstream(&[]),
        &[
            patch(PatchOp::Remove, "/物品/5", None),
            replace("/物品/3", serde_json::json!("斧")),
            delta("/空/0", serde_json::json!(1)),
            replace("/物品/01", serde_json::json!("斧")),
        ],
    );
    assert_eq!(
        outcome.notes,
        vec![
            "/物品/5 不存在：/物品 目前 3 項（索引 0–2）；改項目請 replace /物品 寫出整個陣列。".to_owned(),
            "/物品/3 不存在：/物品 目前 3 項（索引 0–2）；新增請用 insert /物品/-，或 replace /物品 寫出整個陣列。".to_owned(),
            "/空/0 不存在：/空 目前是空的；改項目請 replace /空 寫出整個陣列。".to_owned(),
            "/物品/01 不存在：/物品 目前 3 項（索引 0–2）；改項目請 replace /物品 寫出整個陣列。".to_owned(),
        ]
    );
    assert_eq!(outcome.records[0].path, "物品.5");
}

#[test]
fn element_misses_name_the_spot_and_how_to_fix_it() {
    let (_, outcome) = run(
        &[("角色", r#"[{"名":"甲","HP":3,"_秘":1},"單值"]"#)],
        &upstream(&[]),
        &[
            replace("/角色/0/MP", serde_json::json!(1)),
            replace("/角色/1/HP", serde_json::json!(1)),
            delta("/角色/0/名", serde_json::json!(1)),
            delta("/角色/0/HP", serde_json::json!("很多")),
        ],
    );
    assert_eq!(
        outcome.notes,
        vec![
            "路徑不存在：/角色/0/MP。上一層現有：名、HP。若要新增請用 insert。".to_owned(),
            "/角色/1 是單一值，沒有子欄位；請 replace /角色/1。".to_owned(),
            "/角色/0/名 的值不是數字，不能 delta；請 replace /角色/0/名 寫新值。".to_owned(),
            "/角色/0/HP 是數字，delta 的值要是純數字增減量（例 5、-10）。".to_owned(),
        ]
    );
}

#[test]
fn rejection_pointers_escape_and_stay_under_the_cap() {
    let key = format!("a/b~{}", "長".repeat(150));
    let mut tree = BTreeMap::new();
    tree.insert(key.clone(), StateNode::Leaf("[1]".to_owned()));
    let outcome = apply_updates(
        &mut tree,
        &upstream(&[]),
        &[Patch {
            op: PatchOp::Remove,
            path: vec![key.clone(), "4".to_owned()],
            from: Vec::new(),
            value: None,
        }],
    );
    let note = only_note(&outcome);
    assert!(note.chars().count() <= NOTE_MAX, "{note}");
    let short = "a/b~c".to_owned();
    let mut tree = BTreeMap::new();
    tree.insert(short.clone(), StateNode::Leaf("[1]".to_owned()));
    let outcome = apply_updates(
        &mut tree,
        &upstream(&[]),
        &[Patch {
            op: PatchOp::Remove,
            path: vec![short.clone(), "4".to_owned()],
            from: Vec::new(),
            value: None,
        }],
    );
    let pointer = only_note(&outcome).split(' ').next().unwrap();
    let parsed = parse_updates(&format!(r#"[{{"op":"remove","path":"{pointer}"}}]"#));
    assert_eq!(parsed[0].path, vec![short, "4".to_owned()]);
}

#[test]
fn delta_only_tables_keep_the_old_single_field_hint() {
    let (tree, outcome) = run(
        &[("物品", r#"["劍"]"#)],
        &Mechanism::default(),
        &[replace("/物品/0", serde_json::json!("盾"))],
    );
    assert_eq!(tree, tree_from(&[("物品", r#"["劍"]"#)]));
    assert!(only_note(&outcome).contains("是單一欄位，沒有子欄位"));
}

#[test]
fn upstream_insert_of_a_new_key_keeps_arrays_and_null_as_leaves() {
    let (tree, outcome) = run(
        &[("R.x", "1")],
        &upstream(&[]),
        &[
            patch(
                PatchOp::Insert,
                "/R/清單",
                Some(serde_json::json!(["a", "$__META_EXTENSIBLE__$", "b"])),
            ),
            patch(PatchOp::Insert, "/R/空", Some(serde_json::Value::Null)),
            replace("/R/清單/1", serde_json::json!("c")),
        ],
    );
    assert!(outcome.records.is_empty(), "{:?}", outcome.records);
    assert_eq!(
        tree,
        tree_from(&[("R.x", "1"), ("R.清單", r#"["a","c"]"#), ("R.空", "null")])
    );
}
