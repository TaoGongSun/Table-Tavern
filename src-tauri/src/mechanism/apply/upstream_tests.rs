//! mvu-replace-numeric：數字欄 replace 依更新策略放行或拒收。
use super::*;
use crate::data::FieldRule;
use crate::mechanism::parse::parse_updates;
use crate::mechanism::test_support::{mechanism_with, rule, tree_from};

fn with_policy(mut mechanism: Mechanism, numeric: NumericUpdate) -> Mechanism {
    mechanism.numeric_update = numeric;
    mechanism
}

fn leaf(tree: &BTreeMap<String, StateNode>, path: &str) -> Option<String> {
    let segments: Vec<String> = path.split('.').map(str::to_owned).collect();
    data::node_at(tree, &segments).and_then(|node| leaf_value(node).map(str::to_owned))
}

fn replace(path: &str, value: serde_json::Value) -> Patch {
    Patch {
        op: PatchOp::Replace,
        path: path.split('.').map(str::to_owned).collect(),
        from: Vec::new(),
        value: Some(value),
    }
}

fn rejected(outcome: &Outcome) -> Vec<&str> {
    outcome
        .records
        .iter()
        .filter(|record| record.kind == RecordKind::Rejected)
        .map(|record| record.path.as_str())
        .collect()
}

/// 上游放行：明確規則帶範圍的數字欄 replace 照寫、不夾範圍、不留拒收，變動記成「更新」。
#[test]
fn upstream_replace_writes_number_without_clamping() {
    let mut tree = tree_from(&[("World.HP", "10")]);
    let mechanism = with_policy(
        mechanism_with(&[("World.HP", rule(FieldKind::Number, Some(0.0), Some(100.0)))]),
        NumericUpdate::Upstream,
    );
    let outcome = apply_updates(
        &mut tree,
        &mechanism,
        &[replace("World.HP", serde_json::json!(150))],
    );
    assert!(outcome.records.is_empty(), "{:?}", outcome.records);
    assert_eq!(leaf(&tree, "World.HP").as_deref(), Some("150"));
    assert_eq!(
        outcome.changes.get("World.HP").map(String::as_str),
        Some("更新")
    );
}

/// 原規則（重構卡）：同一筆 replace 照舊擋下、值不動，下一輪回饋要求改用 delta。
#[test]
fn delta_only_replace_is_still_rejected() {
    let mut tree = tree_from(&[("World.HP", "10")]);
    let mechanism =
        mechanism_with(&[("World.HP", rule(FieldKind::Number, Some(0.0), Some(100.0)))]);
    let outcome = apply_updates(
        &mut tree,
        &mechanism,
        &[replace("World.HP", serde_json::json!(50))],
    );
    assert_eq!(rejected(&outcome), ["World.HP"]);
    assert_eq!(leaf(&tree, "World.HP").as_deref(), Some("10"));
    assert!(outcome.notes[0].contains("請用增減量（delta）"));
    assert!(outcome.changes.is_empty());
}

/// 沒規則表、靠現值推成數字欄：上游放行，原文 `<JSONPatch>` 解析後一樣通（實機現象「現值 100000」那條）。
#[test]
fn upstream_replace_on_inferred_number_from_json_patch_text() {
    let block = r#"<JSONPatch>[{"op":"replace","path":"/主角/金錢","value":99000}]</JSONPatch>"#;
    let patches = parse_updates(block);
    let mut strict_tree = tree_from(&[("主角.金錢", "100000")]);
    let strict = apply_updates(&mut strict_tree, &Mechanism::default(), &patches);
    assert_eq!(rejected(&strict), ["主角.金錢"]);
    assert_eq!(leaf(&strict_tree, "主角.金錢").as_deref(), Some("100000"));

    let mut tree = tree_from(&[("主角.金錢", "100000")]);
    let mechanism = with_policy(Mechanism::default(), NumericUpdate::Upstream);
    let outcome = apply_updates(&mut tree, &mechanism, &patches);
    assert!(outcome.records.is_empty(), "{:?}", outcome.records);
    assert_eq!(leaf(&tree, "主角.金錢").as_deref(), Some("99000"));
}

#[test]
fn upstream_replace_covers_counter_fields() {
    let mut tree = tree_from(&[("World.Day", "3")]);
    let rules = [("World.Day", rule(FieldKind::Counter, Some(0.0), Some(5.0)))];
    let mechanism = with_policy(mechanism_with(&rules), NumericUpdate::Upstream);
    let outcome = apply_updates(
        &mut tree,
        &mechanism,
        &[replace("World.Day", serde_json::json!(9))],
    );
    assert!(outcome.records.is_empty());
    assert_eq!(leaf(&tree, "World.Day").as_deref(), Some("9"));

    let mut strict_tree = tree_from(&[("World.Day", "3")]);
    let strict = apply_updates(
        &mut strict_tree,
        &mechanism_with(&rules),
        &[replace("World.Day", serde_json::json!(9))],
    );
    assert_eq!(rejected(&strict), ["World.Day"]);
}

/// Pair：上游整值改寫（現值與上限一起換）；原規則維持「現值拒改、上限可改」。
#[test]
fn pair_replace_whole_value_upstream_but_only_max_when_delta_only() {
    let patch = replace("Player.HP", serde_json::json!("450/600"));
    let mut tree = tree_from(&[("Player.HP", "480/500")]);
    let upstream = with_policy(Mechanism::default(), NumericUpdate::Upstream);
    let outcome = apply_updates(&mut tree, &upstream, std::slice::from_ref(&patch));
    assert!(outcome.records.is_empty());
    assert_eq!(leaf(&tree, "Player.HP").as_deref(), Some("450/600"));

    let mut strict_tree = tree_from(&[("Player.HP", "480/500")]);
    let strict = apply_updates(&mut strict_tree, &Mechanism::default(), &[patch]);
    assert_eq!(rejected(&strict), ["Player.HP"]);
    assert_eq!(leaf(&strict_tree, "Player.HP").as_deref(), Some("480/600"));
}

/// 上游 set 的值轉換：數字字串轉數字（含前後空白、空字串、十六進位）、轉不成的變 null、null 照寫 null。
#[test]
fn upstream_replace_converts_number_strings_and_accepts_null() {
    let mechanism = with_policy(Mechanism::default(), NumericUpdate::Upstream);
    for (value, expected) in [
        (serde_json::json!("85"), "85"),
        (serde_json::json!(" 12.5 "), "12.5"),
        (serde_json::json!(""), "0"),
        (serde_json::json!("0x10"), "16"),
        (serde_json::json!("1e3"), "1000"),
        (serde_json::json!("abc"), "null"),
        (serde_json::json!("Infinity"), "null"),
        (serde_json::Value::Null, "null"),
        (serde_json::json!(true), "true"),
        (serde_json::json!(-3.5), "-3.5"),
    ] {
        let mut tree = tree_from(&[("A.n", "5")]);
        let outcome = apply_updates(&mut tree, &mechanism, &[replace("A.n", value.clone())]);
        assert!(outcome.records.is_empty(), "{value}");
        assert_eq!(leaf(&tree, "A.n").as_deref(), Some(expected), "{value}");
    }
    // Pair 舊值不是數字（上游是字串）：字串照寫，null 一樣寫 null
    let mut tree = tree_from(&[("A.hp", "3/9")]);
    apply_updates(
        &mut tree,
        &mechanism,
        &[replace("A.hp", serde_json::Value::Null)],
    );
    assert_eq!(leaf(&tree, "A.hp").as_deref(), Some("null"));
}

/// insert 命中既有數字欄走同一條規則：上游放行、原規則擋下。
#[test]
fn insert_on_existing_number_follows_the_policy() {
    let patch = Patch {
        op: PatchOp::Insert,
        path: vec!["A".to_owned(), "n".to_owned()],
        from: Vec::new(),
        value: Some(serde_json::json!(42)),
    };
    let mut tree = tree_from(&[("A.n", "5")]);
    let upstream = with_policy(Mechanism::default(), NumericUpdate::Upstream);
    assert!(
        apply_updates(&mut tree, &upstream, std::slice::from_ref(&patch))
            .records
            .is_empty()
    );
    assert_eq!(leaf(&tree, "A.n").as_deref(), Some("42"));

    let mut strict_tree = tree_from(&[("A.n", "5")]);
    let strict = apply_updates(&mut strict_tree, &Mechanism::default(), &[patch]);
    assert_eq!(rejected(&strict), ["A.n"]);
    assert_eq!(leaf(&strict_tree, "A.n").as_deref(), Some("5"));
}

/// 上游策略只解除 Delta 限制：本地擲骰、唯讀規則、底線欄照舊拒收。
#[test]
fn upstream_keeps_local_readonly_and_underscore_rejections() {
    let mechanism = with_policy(
        mechanism_with(&[
            ("A.roll", FieldRule::for_kind(FieldKind::Roll)),
            ("A.flag", FieldRule::for_kind(FieldKind::ReadOnly)),
        ]),
        NumericUpdate::Upstream,
    );
    let mut tree = tree_from(&[("A.roll", "4"), ("A.flag", "1"), ("A._secret", "7")]);
    let outcome = apply_updates(
        &mut tree,
        &mechanism,
        &[
            replace("A.roll", serde_json::json!(6)),
            replace("A.flag", serde_json::json!(0)),
            replace("A._secret", serde_json::json!(9)),
        ],
    );
    assert_eq!(rejected(&outcome), ["A.roll", "A.flag", "A._secret"]);
    assert_eq!(leaf(&tree, "A.roll").as_deref(), Some("4"));
    assert_eq!(leaf(&tree, "A.flag").as_deref(), Some("1"));
    assert_eq!(leaf(&tree, "A._secret").as_deref(), Some("7"));
}

#[test]
fn js_number_matches_javascript_number_for_strings() {
    use crate::mechanism::upstream_set::js_number;
    for (text, expected) in [
        ("42", 42.0),
        ("  -7 ", -7.0),
        ("+1.5", 1.5),
        (".5", 0.5),
        ("5.", 5.0),
        ("2E-2", 0.02),
        ("0b101", 5.0),
        ("0O17", 15.0),
        ("0XfF", 255.0),
        ("", 0.0),
        ("\u{feff}85", 85.0),
        ("\u{3000}\u{a0}12\u{2028}", 12.0),
        ("0x10000000000000000", 18_446_744_073_709_551_616.0),
        ("-Infinity", f64::NEG_INFINITY),
    ] {
        assert_eq!(js_number(text), expected, "{text:?}");
    }
    // 超過 128 位元的十六進位照就近捨入：2^200＋1 捨成 2^200
    let huge = format!("0x1{}1", "0".repeat(49));
    assert_eq!(js_number(&huge), 2f64.powi(200));
    for text in [
        "abc", "inf", "NaN", "nan", "1e", ".", "1,000", "12abc", "-0x10", "infinity", "0x",
        "\u{85}5",
    ] {
        assert!(js_number(text).is_nan(), "{text:?}");
    }
}
