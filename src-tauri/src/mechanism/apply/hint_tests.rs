//! mechanism-error-feedback：路徑寫錯（不存在、打到分支、在單一欄位底下寫子路徑）改記拒收並回饋 GM 怎麼改。
use super::*;
use crate::data::{FieldRule, InjectLevel};
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

fn move_patch(from: &str, to: &str) -> Patch {
    Patch {
        op: PatchOp::Move,
        path: segments(to),
        from: segments(from),
        value: None,
    }
}

fn upstream(rules: &[(&str, FieldRule)]) -> Mechanism {
    let mut mechanism = mechanism_with(rules);
    mechanism.numeric_update = NumericUpdate::Upstream;
    mechanism
}

fn world_tree() -> BTreeMap<String, StateNode> {
    tree_from(&[("World.HP", "10"), ("World.MP", "5"), ("World._秘", "x")])
}

fn only_note(outcome: &Outcome) -> &str {
    assert_eq!(outcome.notes.len(), 1, "{:?}", outcome.records);
    &outcome.notes[0]
}

// ---- 路徑不存在 ----

#[test]
fn missing_last_segment_lists_siblings_and_suggests_insert_for_replace_and_delta() {
    let mechanism = Mechanism::default();
    for op in [
        replace("/World/Mana", serde_json::json!(3)),
        delta("/World/Mana", serde_json::json!(3)),
    ] {
        let mut tree = world_tree();
        let outcome = apply_updates(&mut tree, &mechanism, &[op]);
        assert_eq!(outcome.records[0].kind, RecordKind::Rejected);
        assert_eq!(
            only_note(&outcome),
            "路徑不存在：/World/Mana。上一層現有：HP、MP。若要新增請用 insert。"
        );
        assert_eq!(tree, world_tree());
    }
}

#[test]
fn missing_middle_segment_prints_the_prefix_without_insert_hint() {
    let mut tree = world_tree();
    let outcome = apply_updates(
        &mut tree,
        &Mechanism::default(),
        &[replace("/World/Stats/STR", serde_json::json!(3))],
    );
    assert_eq!(
        only_note(&outcome),
        "路徑不存在：/World/Stats。上一層現有：HP、MP。"
    );
    assert_eq!(outcome.records[0].path, "World.Stats.STR");
}

#[test]
fn remove_and_move_from_missing_paths_never_suggest_insert() {
    let mut tree = world_tree();
    let outcome = apply_updates(
        &mut tree,
        &Mechanism::default(),
        &[
            patch(PatchOp::Remove, "/World/Mana", None),
            move_patch("/Nope", "/World/Gone"),
        ],
    );
    assert_eq!(
        outcome.notes,
        vec![
            "路徑不存在：/World/Mana。上一層現有：HP、MP。".to_owned(),
            "路徑不存在：/Nope。頂層現有：World。".to_owned(),
        ]
    );
    assert!(outcome
        .records
        .iter()
        .all(|r| r.kind == RecordKind::Rejected));
}

#[test]
fn same_missing_prefix_merges_into_one_note_but_keeps_every_ledger_record() {
    let mut tree = tree_from(&[("角色.甲.好感度", "10")]);
    let outcome = apply_updates(
        &mut tree,
        &Mechanism::default(),
        &[
            replace("/角色/新NPC/好感度", serde_json::json!(1)),
            replace("/角色/新NPC/心情", serde_json::json!("好")),
            replace("/角色/新NPC/位置", serde_json::json!("酒館")),
        ],
    );
    assert_eq!(outcome.records.len(), 3);
    assert_eq!(
        only_note(&outcome),
        "路徑不存在：/角色/新NPC。上一層現有：甲。（本輪 3 筆未套用：好感度、心情、位置）"
    );
}

#[test]
fn explicit_readonly_rule_on_the_intended_path_answers_with_readonly_wording() {
    let mut tree = tree_from(&[("角色.甲.秘密", "x")]);
    let mechanism = mechanism_with(&[("角色.*.秘密", rule(FieldKind::ReadOnly, None, None))]);
    let outcome = apply_updates(
        &mut tree,
        &mechanism,
        &[replace("/角色/乙/秘密", serde_json::json!("y"))],
    );
    assert_eq!(
        only_note(&outcome),
        "/角色/乙/秘密 是唯讀欄位，不接受更新。"
    );
}

// ---- 路徑中途撞到葉子 ----

#[test]
fn index_path_into_a_list_field_gets_the_list_guidance() {
    let mut tree = tree_from(&[("神經狀態", "neuro|疲勞|20")]);
    let mechanism = mechanism_with(&[("神經狀態", rule(FieldKind::List, None, None))]);
    let outcome = apply_updates(
        &mut tree,
        &mechanism,
        &[replace("/神經狀態/0", serde_json::json!("neuro|疲勞|30"))],
    );
    assert!(only_note(&outcome).starts_with("/神經狀態 是清單欄，不能逐項改；"));
    assert_eq!(outcome.records[0].path, "神經狀態");
}

#[test]
fn child_path_under_a_single_field_explains_the_right_op_for_that_field() {
    let mut tree = tree_from(&[("HP", "50"), ("地點", "港口"), ("骰", "3"), ("戰力", "9")]);
    let mut derived = rule(FieldKind::Derived, None, None);
    derived.formula = Some("1".to_owned());
    let mechanism = mechanism_with(&[("骰", rule(FieldKind::Roll, None, None)), ("戰力", derived)]);
    let outcome = apply_updates(
        &mut tree,
        &mechanism,
        &[
            replace("/HP/current", serde_json::json!(40)),
            replace("/地點/名稱", serde_json::json!("酒館")),
            replace("/骰/值", serde_json::json!(6)),
            patch(PatchOp::Insert, "/戰力/x", Some(serde_json::json!(1))),
        ],
    );
    assert_eq!(
        outcome.notes,
        vec![
            "/HP 是單一欄位，沒有子欄位；增減請 delta 它給增減量。".to_owned(),
            "/地點 是單一欄位，沒有子欄位；請 replace 它寫新值。".to_owned(),
            "/骰 由系統本地擲骰，請勿更新。".to_owned(),
            "/戰力 是唯讀欄位，不接受更新。".to_owned(),
        ]
    );
    assert!(outcome
        .notes
        .iter()
        .all(|note| !note.contains("請直接更新")));
}

#[test]
fn move_into_a_single_field_is_rejected_and_the_value_goes_back() {
    let mut tree = tree_from(&[("src", "劍"), ("blk", "5")]);
    let outcome = apply_updates(
        &mut tree,
        &Mechanism::default(),
        &[move_patch("/src", "/blk/x")],
    );
    assert_eq!(outcome.records[0].kind, RecordKind::Rejected);
    assert!(only_note(&outcome).starts_with("/blk 是單一欄位"));
    assert_eq!(tree, tree_from(&[("src", "劍"), ("blk", "5")]));
}

// ---- 分支 ----

#[test]
fn delta_or_single_value_replace_on_a_branch_lists_children() {
    let mechanism = mechanism_with(&[("World", rule(FieldKind::Pair, None, None))]);
    let mut tree = world_tree();
    let outcome = apply_updates(
        &mut tree,
        &mechanism,
        &[
            delta("/World", serde_json::json!(1)),
            replace("/World", serde_json::json!("3/5")),
        ],
    );
    assert_eq!(
        outcome.notes,
        vec![
            "/World 是分支，不能做增減；請改底下的欄位：HP、MP。".to_owned(),
            "/World 是分支，不能整個替換成單一值；請改底下的欄位：HP、MP。".to_owned(),
        ]
    );
    // Delta 規則（數字）打到分支也是同一句
    let number = mechanism_with(&[("World", rule(FieldKind::Number, None, None))]);
    let outcome = apply_updates(
        &mut tree,
        &number,
        &[replace("/World", serde_json::json!(3))],
    );
    assert_eq!(
        only_note(&outcome),
        "/World 是分支，不能整個替換成單一值；請改底下的欄位：HP、MP。"
    );
    assert_eq!(tree, world_tree());
}

#[test]
fn delta_rule_leaf_replace_keeps_the_current_value_wording() {
    let mut tree = tree_from(&[("HP", "10")]);
    let mechanism = mechanism_with(&[("HP", rule(FieldKind::Number, None, None))]);
    let outcome = apply_updates(
        &mut tree,
        &mechanism,
        &[replace("/HP", serde_json::json!(3))],
    );
    assert_eq!(
        only_note(&outcome),
        "/HP 現值 10，請用增減量（delta）而不是絕對值。"
    );
}

#[test]
fn replace_rule_still_swaps_a_whole_branch() {
    let mut tree = world_tree();
    let outcome = apply_updates(
        &mut tree,
        &Mechanism::default(),
        &[replace("/World", serde_json::json!({"HP": 1}))],
    );
    assert!(outcome.records.is_empty());
    assert_eq!(tree, tree_from(&[("World.HP", "1")]));
}

// ---- 列鍵 ----

#[test]
fn branch_listing_caps_at_eight_and_skips_underscore_keys() {
    let pairs: Vec<(String, String)> = (0..12)
        .map(|i| (format!("B.k{i:02}"), "1".to_owned()))
        .collect();
    let mut pairs: Vec<(&str, &str)> = pairs
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    pairs.push(("B._x", "1"));
    let mut tree = tree_from(&pairs);
    let outcome = apply_updates(
        &mut tree,
        &Mechanism::default(),
        &[delta("/B", serde_json::json!(1))],
    );
    assert_eq!(
        only_note(&outcome),
        "/B 是分支，不能做增減；請改底下的欄位：k00、k01、k02、k03、k04、k05、k06、k07 等 12 項。"
    );
}

#[test]
fn incremental_listing_hides_what_the_gm_never_sees() {
    let pairs = [
        ("A.HP", "1"),
        ("A.戰力", "2"),
        ("A.內部", "3"),
        ("A.隱藏支.x", "4"),
        ("A.地點", "港"),
    ];
    let mut derived = rule(FieldKind::Derived, None, None);
    derived.formula = Some("1".to_owned());
    let mut rare = rule(FieldKind::Number, None, None);
    rare.inject = InjectLevel::Rare;
    let rules = [
        ("A.戰力", derived),
        ("A.內部", rare.clone()),
        ("A.隱藏支.x", rare),
    ];
    let mut mechanism = mechanism_with(&rules);
    mechanism.incremental = true;
    let mut tree = tree_from(&pairs);
    let outcome = apply_updates(&mut tree, &mechanism, &[delta("/A", serde_json::json!(1))]);
    assert_eq!(
        only_note(&outcome),
        "/A 是分支，不能做增減；請改底下的欄位：HP、地點。"
    );
    // 全量桌照舊全列
    mechanism.incremental = false;
    let outcome = apply_updates(&mut tree, &mechanism, &[delta("/A", serde_json::json!(1))]);
    assert_eq!(
        only_note(&outcome),
        "/A 是分支，不能做增減；請改底下的欄位：HP、內部、地點、戰力、隱藏支。"
    );
}

// ---- 長度與路徑寫法 ----

#[test]
fn long_paths_and_many_long_keys_stay_under_the_cap_with_path_and_instruction_intact() {
    let long = "長".repeat(118); // 指標 "/" + 118 字 = 119 字
    let keys: Vec<String> = (0..30)
        .map(|i| format!("{}{i:02}", "鍵".repeat(30)))
        .collect();
    let pairs: Vec<(String, &str)> = keys.iter().map(|k| (k.clone(), "1")).collect();
    let pairs: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    let mut tree = tree_from(&pairs);
    let pointer = format!("/{long}");
    let outcome = apply_updates(
        &mut tree,
        &Mechanism::default(),
        &[
            replace(&format!("{pointer}/a"), serde_json::json!(1)),
            replace(&format!("{pointer}/b"), serde_json::json!(1)),
        ],
    );
    let note = only_note(&outcome);
    assert!(note.chars().count() <= NOTE_MAX, "{}", note.chars().count());
    assert_eq!(note.matches(&pointer).count(), 1, "{note}");
    assert!(
        note.starts_with(&format!("路徑不存在：{pointer}。頂層現有：")),
        "{note}"
    );
    assert!(note.ends_with("（本輪 2 筆未套用：a、b）"), "{note}");
    assert!(note.contains(" 等 30 項"), "{note}");

    // 超長路徑中段省略，仍不超過上限
    let huge = format!("/{}", "長".repeat(300));
    let outcome = apply_updates(
        &mut tree,
        &Mechanism::default(),
        &[replace(&huge, serde_json::json!(1))],
    );
    let note = only_note(&outcome);
    assert!(
        note.chars().count() <= NOTE_MAX && note.contains('…'),
        "{note}"
    );
    assert!(note.ends_with("若要新增請用 insert。"), "{note}");
}

#[test]
fn hint_pointer_escapes_and_round_trips_through_the_parser() {
    let mut tree = tree_from(&[("a/b~c", "5")]);
    let outcome = apply_updates(
        &mut tree,
        &Mechanism::default(),
        &[Patch {
            op: PatchOp::Replace,
            path: vec!["a/b~c".to_owned(), "子".to_owned()],
            from: Vec::new(),
            value: Some(serde_json::json!(1)),
        }],
    );
    let note = only_note(&outcome);
    let pointer = note.split(' ').next().unwrap();
    assert_eq!(pointer, "/a~1b~0c");
    let patches = parse_updates(&format!(
        r#"[{{"op":"delta","path":"{pointer}","value":1}}]"#
    ));
    assert_eq!(patches[0].path, vec!["a/b~c".to_owned()]);
}

// ---- 沒重構 MVU 卡（Upstream）的陣列 ----

#[test]
fn upstream_index_paths_explain_vwd_and_plain_arrays() {
    let mut tree = tree_from(&[
        ("HP", r#"[50,"生命"]"#),
        ("稱號", r#"["勇者","目前稱號"]"#),
        ("物品", r#"["劍","盾","弓"]"#),
    ]);
    let mechanism = upstream(&[("物品", rule(FieldKind::List, None, None))]);
    let before = tree.clone();
    let outcome = apply_updates(
        &mut tree,
        &mechanism,
        &[
            replace("/HP/0", serde_json::json!(40)),
            replace("/稱號/0", serde_json::json!("魔王")),
            replace("/物品/1", serde_json::json!("斧")),
        ],
    );
    assert_eq!(
        outcome.notes,
        vec![
            "/HP 是 [值, 說明] 欄，不能用索引路徑；replace 它只會改第 0 項（值只寫新數字），增減用 delta 它。".to_owned(),
            "/稱號 依上游規則視為 [值, 說明]：replace 它只會改第 0 項（目前「勇者」）；增刪項目目前不支援。".to_owned(),
            "/物品 是陣列，目前不支援逐項路徑；請 replace 它，值寫成整個 JSON 陣列（不要加引號）。"
                .to_owned(),
        ]
    );
    assert!(outcome.notes[..2]
        .iter()
        .all(|note| !note.contains("整個陣列")));
    assert!(!outcome.notes[1].contains("delta") && !outcome.notes[1].contains("只寫新值"));
    assert_eq!(tree, before);
}

#[test]
fn upstream_index_path_into_a_roll_array_answers_with_roll_wording() {
    let mut tree = tree_from(&[("骰", r#"[3,"本輪骰值"]"#)]);
    let mechanism = upstream(&[("骰", rule(FieldKind::Roll, None, None))]);
    let outcome = apply_updates(
        &mut tree,
        &mechanism,
        &[replace("/骰/0", serde_json::json!(6))],
    );
    assert_eq!(only_note(&outcome), "/骰 由系統本地擲骰，請勿更新。");
}

#[test]
fn upstream_missing_remove_and_move_are_rejected_and_the_batch_goes_on() {
    let mut tree = tree_from(&[("HP", "10"), ("名", "甲")]);
    let mechanism = upstream(&[]);
    let outcome = apply_updates(
        &mut tree,
        &mechanism,
        &[
            patch(PatchOp::Remove, "/沒有", None),
            move_patch("/也沒有", "/名2"),
            replace("/新欄", serde_json::json!(1)),
            replace("/名", serde_json::json!("乙")),
        ],
    );
    let kinds: Vec<RecordKind> = outcome.records.iter().map(|r| r.kind).collect();
    assert_eq!(kinds, vec![RecordKind::Rejected; 3]);
    assert_eq!(tree, tree_from(&[("HP", "10"), ("名", "乙")]));
}

// ---- VWD delta 守門與非有限輸入 ----

#[test]
fn vwd_delta_respects_roll_and_readonly_rules() {
    for (kind, wording) in [
        (FieldKind::Roll, "/HP 由系統本地擲骰，請勿更新。"),
        (FieldKind::ReadOnly, "/HP 是唯讀欄位，不接受更新。"),
    ] {
        let mut tree = tree_from(&[("HP", r#"[50,"生命"]"#)]);
        let mechanism = upstream(&[("HP", rule(kind, None, None))]);
        let outcome = apply_updates(
            &mut tree,
            &mechanism,
            &[delta("/HP", serde_json::json!(-5))],
        );
        assert_eq!(only_note(&outcome), wording);
        assert_eq!(tree, tree_from(&[("HP", r#"[50,"生命"]"#)]));
    }
}

#[test]
fn non_finite_delta_strings_are_rejected_everywhere() {
    for bad in ["NaN", "inf", "-inf"] {
        let mut tree = tree_from(&[("HP", r#"[50,"生命"]"#), ("MP", "5")]);
        let outcome = apply_updates(
            &mut tree,
            &upstream(&[]),
            &[
                delta("/HP", serde_json::json!(bad)),
                delta("/MP", serde_json::json!(bad)),
            ],
        );
        assert_eq!(outcome.notes.len(), 2, "{bad}: {:?}", outcome.records);
        assert_eq!(
            tree,
            tree_from(&[("HP", r#"[50,"生命"]"#), ("MP", "5")]),
            "{bad}"
        );
    }
}

#[test]
fn vwd_delta_with_a_string_value_is_rejected() {
    let mut tree = tree_from(&[("稱號", r#"["勇者","目前稱號"]"#)]);
    let outcome = apply_updates(
        &mut tree,
        &upstream(&[]),
        &[delta("/稱號", serde_json::json!(1))],
    );
    assert_eq!(
        only_note(&outcome),
        "/稱號 的第 0 項不是數字，不能 delta；replace 它只會改第 0 項（目前「勇者」）。"
    );
}

/// 恰好兩個字串的清單照上游當 [值, 說明]：索引、逐項插入、刪除都照實說 replace 只會改第 0 項。
#[test]
fn two_string_list_counts_as_vwd_and_the_hint_says_replace_only_touches_the_first_item() {
    let stat = r#"["劍","盾"]"#;
    for bad in [
        replace("/背包/0", serde_json::json!("弓")),
        replace("/背包/1", serde_json::json!("弓")),
        patch(PatchOp::Insert, "/背包/-", Some(serde_json::json!("弓"))),
        patch(PatchOp::Remove, "/背包/0", None),
    ] {
        let mut tree = tree_from(&[("背包", stat)]);
        let outcome = apply_updates(&mut tree, &upstream(&[]), &[bad]);
        let note = only_note(&outcome);
        assert!(
            note.contains("只會改第 0 項（目前「劍」）") && !note.contains("只寫新值"),
            "{note}"
        );
        assert_eq!(tree, tree_from(&[("背包", stat)]));
    }
}

#[test]
fn new_keys_under_the_same_parent_merge_into_one_note() {
    let mut tree = tree_from(&[("背包.劍", "1")]);
    let outcome = apply_updates(
        &mut tree,
        &Mechanism::default(),
        &[
            replace("/背包/弓", serde_json::json!(1)),
            replace("/背包/藥", serde_json::json!(2)),
            patch(PatchOp::Remove, "/背包/盾", None),
        ],
    );
    assert_eq!(
        outcome.notes,
        vec![
            "路徑不存在：/背包 底下沒有這些欄位。現有：劍。若要新增請用 insert。（本輪 2 筆未套用：弓、藥）".to_owned(),
            "路徑不存在：/背包/盾。上一層現有：劍。".to_owned(),
        ]
    );
}

#[test]
fn vwd_rejections_with_long_paths_stay_under_the_cap() {
    let long = "長".repeat(200);
    let pairs = [
        (format!("{long}.甲"), r#"["勇者","稱號"]"#.to_owned()),
        (format!("{long}.乙"), r#"[1,"數"]"#.to_owned()),
    ];
    let pairs: Vec<(&str, &str)> = pairs
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let mut tree = tree_from(&pairs);
    let outcome = apply_updates(
        &mut tree,
        &upstream(&[]),
        &[
            delta(&format!("/{long}/甲"), serde_json::json!(1)),
            delta(&format!("/{long}/乙"), serde_json::json!("很多")),
        ],
    );
    assert_eq!(outcome.notes.len(), 2, "{:?}", outcome.records);
    for note in &outcome.notes {
        assert!(note.chars().count() <= NOTE_MAX, "{}", note.chars().count());
        assert!(note.contains('…'), "{note}");
    }
    assert!(outcome.notes[0].ends_with("replace 它只會改第 0 項（目前「勇者」）。"));
    assert!(outcome.notes[1].ends_with("delta 的值要是純數字增減量（例 5、-10）。"));
}

/// 同一個新鍵寫了兩次：合併尾綴的鍵名去重，筆數照實。
#[test]
fn merged_suffix_dedupes_key_names_but_counts_every_record() {
    let mut tree = tree_from(&[("背包.劍", "1")]);
    let outcome = apply_updates(
        &mut tree,
        &Mechanism::default(),
        &[
            replace("/背包/x", serde_json::json!(1)),
            replace("/背包/x", serde_json::json!(2)),
        ],
    );
    assert_eq!(outcome.records.len(), 2);
    assert!(
        only_note(&outcome).ends_with("（本輪 2 筆未套用：x）"),
        "{}",
        outcome.notes[0]
    );
}
