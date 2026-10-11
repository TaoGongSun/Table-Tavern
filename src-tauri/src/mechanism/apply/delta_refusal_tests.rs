//! takeover-text-field-delta：不收 delta 的欄位記拒收、進下一輪自癒回饋。
use super::*;
use crate::mechanism::parse::parse_updates;
use crate::mechanism::test_support::{mechanism_with, rule, tree_from};

const NEURO: &str = "neuro|疲勞|20|低度疲勞狀態\nneuro|壓力|35|中度壓力\nneuro|痛苦|10|輕微疼痛\nneuro|恐懼|5|極低的恐懼感";

fn leaf(tree: &BTreeMap<String, StateNode>, path: &str) -> Option<String> {
    let segments: Vec<String> = path.split('.').map(str::to_owned).collect();
    data::node_at(tree, &segments).and_then(|node| leaf_value(node).map(str::to_owned))
}

fn delta(path: &str, value: serde_json::Value) -> Patch {
    Patch {
        op: PatchOp::Delta,
        path: path.split('.').map(str::to_owned).collect(),
        from: Vec::new(),
        value: Some(value),
    }
}

/// Transfur-safe 接管桌的縮小版：清單欄、文字欄、數字欄、「現值/上限」欄。
fn transfur_table() -> (BTreeMap<String, StateNode>, Mechanism) {
    let tree = tree_from(&[
        ("神經狀態", NEURO),
        ("時間", "1991-03-15 08:00"),
        ("地點", "團地區"),
        ("貨幣", "currency|2150|KC|約2.15kg"),
        ("生命值.當前值", "100"),
        ("經驗值.進度", "0/100"),
    ]);
    let mut mechanism = mechanism_with(&[
        ("神經狀態", rule(FieldKind::List, None, None)),
        ("時間", rule(FieldKind::Text, None, None)),
        ("地點", rule(FieldKind::Text, None, None)),
        ("貨幣", rule(FieldKind::List, None, None)),
        ("生命值.當前值", rule(FieldKind::Number, Some(0.0), None)),
        ("經驗值.進度", rule(FieldKind::Pair, None, None)),
    ]);
    mechanism.incremental = true;
    mechanism.numeric_update = NumericUpdate::DeltaOnly;
    (tree, mechanism)
}

#[test]
fn delta_on_list_field_is_rejected_with_replace_guidance() {
    let (mut tree, mechanism) = transfur_table();
    let outcome = apply_updates(
        &mut tree,
        &mechanism,
        &[delta("神經狀態", serde_json::json!("neuro|壓力|10|升高"))],
    );
    assert_eq!(outcome.records.len(), 1);
    let record = &outcome.records[0];
    assert_eq!(record.kind, RecordKind::Rejected);
    assert!(record.detail.contains("清單欄"));
    assert!(record.detail.contains("replace"));
    assert!(record.detail.contains("沒變的行也照抄"));
    assert_eq!(outcome.notes, vec![record.detail.clone()]);
    assert_eq!(leaf(&tree, "神經狀態").as_deref(), Some(NEURO));
    assert!(outcome.changes.is_empty());
}

#[test]
fn delta_on_text_field_with_number_is_rejected() {
    let (mut tree, mechanism) = transfur_table();
    let outcome = apply_updates(
        &mut tree,
        &mechanism,
        &[delta("地點", serde_json::json!(5))],
    );
    assert_eq!(outcome.records[0].kind, RecordKind::Rejected);
    assert!(outcome.records[0].detail.contains("文字欄"));
    assert_eq!(leaf(&tree, "地點").as_deref(), Some("團地區"));
}

#[test]
fn non_numeric_delta_on_number_and_pair_fields_is_rejected() {
    let (mut tree, mechanism) = transfur_table();
    let outcome = apply_updates(
        &mut tree,
        &mechanism,
        &[
            delta("生命值.當前值", serde_json::json!("-10%")),
            delta("經驗值.進度", serde_json::json!("5點")),
        ],
    );
    assert_eq!(outcome.records.len(), 2);
    assert!(outcome
        .records
        .iter()
        .all(|r| r.kind == RecordKind::Rejected));
    assert!(outcome.records[0].detail.contains("是數字欄"));
    assert!(outcome.records[1].detail.contains("「現值/上限」欄"));
    assert_eq!(outcome.notes.len(), 2);
    assert_eq!(leaf(&tree, "生命值.當前值").as_deref(), Some("100"));
    assert_eq!(leaf(&tree, "經驗值.進度").as_deref(), Some("0/100"));
}

#[test]
fn broken_current_value_on_number_field_stays_an_error() {
    let mut tree = tree_from(&[("HP", "滿")]);
    let mechanism = mechanism_with(&[("HP", rule(FieldKind::Number, None, None))]);
    let outcome = apply_updates(&mut tree, &mechanism, &[delta("HP", serde_json::json!(5))]);
    assert_eq!(outcome.records[0].kind, RecordKind::Error);
    assert!(outcome.notes.is_empty());
}

#[test]
fn readonly_roll_and_derived_fields_reject_non_numeric_delta_with_existing_wording() {
    let mut tree = tree_from(&[("RO", "1"), ("Dice", "4"), ("Total", "9")]);
    let mechanism = mechanism_with(&[
        ("RO", rule(FieldKind::ReadOnly, None, None)),
        ("Dice", rule(FieldKind::Roll, None, None)),
        ("Total", rule(FieldKind::Derived, None, None)),
    ]);
    let outcome = apply_updates(
        &mut tree,
        &mechanism,
        &[
            delta("RO", serde_json::json!("加一")),
            delta("Dice", serde_json::json!("加一")),
            delta("Total", serde_json::json!("加一")),
        ],
    );
    let kinds: Vec<_> = outcome.records.iter().map(|r| r.kind).collect();
    assert_eq!(kinds, vec![RecordKind::Rejected; 3]);
    assert_eq!(outcome.records[0].detail, "RO 是唯讀欄位，不接受更新。");
    assert_eq!(outcome.records[1].detail, "Dice 由系統本地擲骰，請勿更新。");
    assert_eq!(outcome.records[2].detail, "Total 是唯讀欄位，不接受更新。");
    assert_eq!(leaf(&tree, "Total").as_deref(), Some("9"));
}

#[test]
fn upstream_table_inferred_text_field_rejects_delta() {
    let mut tree = tree_from(&[("糧草", "账载四百四十石 实存待查")]);
    let mut mechanism = Mechanism::default();
    mechanism.numeric_update = NumericUpdate::Upstream;
    let outcome = apply_updates(
        &mut tree,
        &mechanism,
        &[delta("糧草", serde_json::json!(-40))],
    );
    assert_eq!(outcome.records[0].kind, RecordKind::Rejected);
    assert!(outcome.records[0].detail.contains("文字欄"));
    assert_eq!(
        leaf(&tree, "糧草").as_deref(),
        Some("账载四百四十石 实存待查")
    );
}

#[test]
fn insert_under_a_list_leaf_is_rejected_with_list_guidance() {
    let (mut tree, mechanism) = transfur_table();
    let patch = Patch {
        op: PatchOp::Insert,
        path: vec!["神經狀態".to_owned(), "-".to_owned()],
        from: Vec::new(),
        value: Some(serde_json::json!("neuro|恐懼|15|不安")),
    };
    let outcome = apply_updates(&mut tree, &mechanism, &[patch]);
    assert_eq!(outcome.records.len(), 1);
    assert_eq!(outcome.records[0].kind, RecordKind::Rejected);
    assert_eq!(outcome.records[0].path, "神經狀態");
    assert!(outcome.records[0].detail.contains("清單欄，不能逐行插入"));
    assert_eq!(leaf(&tree, "神經狀態").as_deref(), Some(NEURO));
}

/// 前次真模型實測三回合原始更新區塊的縮小版（Transfur-safe 接管桌，Haiku）：
/// 三筆 `神經狀態` delta 都進回饋，同批其他 patch 照常套用。
#[test]
fn replay_of_recorded_transfur_turns_feeds_every_list_delta_back() {
    let turns = [
        r#"[
  {"op": "replace", "path": "/地點", "value": "卡斯特羅鎮YMCA旅社 301房間"},
  {"op": "replace", "path": "/時間", "value": "1991-03-15 08:15"},
  {"op": "delta", "path": "/神經狀態", "value": "neuro|疲勞|15|輕度疲勞，長途旅行後略感倦怠\nneuro|壓力|30|中度壓力，即將開始新生活的焦慮"},
  {"op": "replace", "path": "/貨幣", "value": "currency|2150|KC|約2.15kg\ncurrency|450|美元|約12g"}
]"#,
        r#"[
  {"op": "replace", "path": "/時間", "value": "1991-03-15 08:45"},
  {"op": "replace", "path": "/地點", "value": "卡斯特羅鎮YMCA旅社 一樓大廳"},
  {"op": "delta", "path": "/神經狀態", "value": "neuro|壓力|10|中度壓力升至高度不安"}
]"#,
        r#"[
  {"op": "replace", "path": "/時間", "value": "1991-03-15 10:30"},
  {"op": "replace", "path": "/地點", "value": "團地區 赤星公寓 七樓走廊"},
  {"op": "delta", "path": "/神經狀態", "value": "neuro|壓力|15|壓力增加至高度，出現懸念感\nneuro|恐懼|10|警覺性提升，感到不安"}
]"#,
    ];
    let (mut tree, mechanism) = transfur_table();
    let expected_place = [
        "卡斯特羅鎮YMCA旅社 301房間",
        "卡斯特羅鎮YMCA旅社 一樓大廳",
        "團地區 赤星公寓 七樓走廊",
    ];
    for (turn, place) in turns.iter().zip(expected_place) {
        let patches = parse_updates(&format!(
            "<UpdateVariable><JSONPatch>{turn}</JSONPatch></UpdateVariable>"
        ));
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert_eq!(outcome.notes.len(), 1, "{:?}", outcome.records);
        assert!(outcome.notes[0].starts_with("神經狀態 是清單欄"));
        assert_eq!(leaf(&tree, "地點").as_deref(), Some(place));
        assert_eq!(leaf(&tree, "神經狀態").as_deref(), Some(NEURO));
    }
    assert_eq!(leaf(&tree, "時間").as_deref(), Some("1991-03-15 10:30"));
    assert!(leaf(&tree, "貨幣").unwrap().contains("美元"));
}

/// 清單／文字欄另設骰值或唯讀時，delta 與 insert 都回那個訊息，不回「改用 replace」。
#[test]
fn list_and_text_with_local_or_reject_answer_with_readonly_wording() {
    let mut tree = tree_from(&[("清單甲", "a|1"), ("清單乙", "b|2"), ("文字甲", "x")]);
    let with = |kind, update| FieldRule {
        update,
        ..rule(kind, None, None)
    };
    let mechanism = mechanism_with(&[
        ("清單甲", with(FieldKind::List, UpdateMode::Reject)),
        ("清單乙", with(FieldKind::List, UpdateMode::Local)),
        ("文字甲", with(FieldKind::Text, UpdateMode::Reject)),
    ]);
    let insert = Patch {
        op: PatchOp::Insert,
        path: vec!["清單甲".to_owned(), "-".to_owned()],
        from: Vec::new(),
        value: Some(serde_json::json!("a|2")),
    };
    let outcome = apply_updates(
        &mut tree,
        &mechanism,
        &[
            delta("清單甲", serde_json::json!("a|3")),
            delta("清單乙", serde_json::json!("b|3")),
            delta("文字甲", serde_json::json!(1)),
            insert,
        ],
    );
    let details: Vec<&str> = outcome.records.iter().map(|r| r.detail.as_str()).collect();
    assert_eq!(
        details,
        vec![
            "清單甲 是唯讀欄位，不接受更新。",
            "清單乙 由系統本地擲骰，請勿更新。",
            "文字甲 是唯讀欄位，不接受更新。",
            "清單甲 是唯讀欄位，不接受更新。",
        ]
    );
    assert!(outcome
        .records
        .iter()
        .all(|r| r.kind == RecordKind::Rejected));
    assert_eq!(leaf(&tree, "清單甲").as_deref(), Some("a|1"));
}
