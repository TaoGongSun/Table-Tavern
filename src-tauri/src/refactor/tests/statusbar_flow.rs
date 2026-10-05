//! refactor-statusbar-skeleton：狀態欄型（playable: no）的新產物走「匯出→匯入→套用→undo」整條路，
//! 來源條目、殼、增量協定、卡專屬規則與指引都要套上，undo 後全部回到套用前。
use super::super::card_file::parse_card;
use super::super::test_support::*;
use super::super::*;
use crate::data::{self, FieldKind, FieldRule, InjectLevel, UpdateMode};
use crate::receipts;
use std::collections::BTreeMap;

const SHELL: &str = "<Status_block>\n状态栏:\n  地点: \"📍 {{状态栏.地点}}\"\n  粮草: \"🌾 {{状态栏.粮草}}\"\n</Status_block>";

fn rule(kind: FieldKind, update: UpdateMode) -> FieldRule {
    FieldRule {
        kind,
        min: None,
        max: None,
        update,
        inject: InjectLevel::Turn,
        branch: None,
        formula: None,
    }
}

#[test]
fn statusbar_outcome_round_trips_through_export_apply_and_undo() {
    let root = TestRoot::new("statusbar-flow");
    let world_id = data::create_world(root.path(), "驛站").unwrap();
    let source_uid = seed_entry(
        root.path(),
        &world_id,
        "美化状态栏",
        "每回合輸出 <Status_block> YAML",
    );
    let before = data::read_state(root.path(), &world_id).unwrap();

    let outcome = RefactorOutcome {
        mode: Some("interface".to_owned()),
        characters: Vec::new(),
        interface: Some(RefactorInterface {
            state_fields: serde_json::json!({ "状态栏": { "地点": "北境驿站", "粮草": "320" } }),
            source_uids: vec![source_uid.to_string()],
            raw: "状态栏散文".to_owned(),
            shell: Some(SHELL.to_owned()),
            rules: BTreeMap::from([
                (
                    "状态栏.地点".to_owned(),
                    rule(FieldKind::Text, UpdateMode::Replace),
                ),
                (
                    "状态栏.粮草".to_owned(),
                    rule(FieldKind::Number, UpdateMode::Delta),
                ),
            ]),
            guide: "地点每回合必報；粮草變動才報。".to_owned(),
        }),
        mechanisms: Vec::new(),
        entries: Vec::new(),
        deletable_shared_uids: Vec::new(),
        dropped: Vec::new(),
        unabsorbed: Vec::new(),
        audit: Vec::new(),
        preserve_source_uids: Vec::new(),
    };
    // 匯出成檔再匯入：產物格式不分 kind，殼／規則／指引都在
    let exported = serde_json::to_string_pretty(&outcome).unwrap();
    let imported: RefactorOutcome = serde_json::from_str(&exported).unwrap();
    let selection = RefactorSelection {
        apply_interface: true,
        ..no_player_selection(Vec::new())
    };
    apply_recorded(root.path(), &world_id, &imported, &selection);

    let state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(
        data::read_interface_shell(root.path(), &world_id)
            .unwrap()
            .as_deref(),
        Some(SHELL)
    );
    assert!(state.mechanism.incremental);
    assert_eq!(state.mechanism.guide, "地点每回合必報；粮草變動才報。");
    assert!(state.mechanism.rules.contains_key("状态栏.粮草"));
    assert_eq!(state.refactor_mode.as_deref(), Some("interface"));
    assert!(!data::read_worldbook(root.path(), &world_id)
        .unwrap()
        .iter()
        .any(|entry| entry.uid == source_uid));
    assert_eq!(
        parse_card(
            &data::read_refactor_outcome(root.path(), &world_id)
                .unwrap()
                .unwrap()
        )
        .unwrap()
        .outcome,
        outcome
    );

    receipts::undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    let undone = data::read_state(root.path(), &world_id).unwrap();
    assert!(data::read_interface_shell(root.path(), &world_id)
        .unwrap()
        .is_none());
    assert_eq!(undone.mechanism.incremental, before.mechanism.incremental);
    assert_eq!(undone.mechanism.guide, before.mechanism.guide);
    assert!(!undone.mechanism.rules.contains_key("状态栏.粮草"));
    assert_eq!(undone.refactor_mode, before.refactor_mode);
    assert_eq!(undone.state.tree, before.state.tree);
    assert!(data::read_worldbook(root.path(), &world_id)
        .unwrap()
        .iter()
        .any(|entry| entry.content == "每回合輸出 <Status_block> YAML"));
}

/// 指引被第二輪重構換掉後 undo，寫回的是第一輪的指引，不是清空。
#[test]
fn undo_restores_previous_guide_not_empty() {
    let root = TestRoot::new("statusbar-guide");
    let world_id = data::create_world(root.path(), "驛站").unwrap();
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    state.mechanism.guide = "第一輪指引".to_owned();
    data::write_state(root.path(), &world_id, &state).unwrap();

    let outcome = RefactorOutcome {
        mode: Some("interface".to_owned()),
        characters: Vec::new(),
        interface: Some(RefactorInterface {
            state_fields: serde_json::json!({ "状态栏": { "地点": "驿站" } }),
            source_uids: Vec::new(),
            raw: String::new(),
            shell: Some("<Status_block>\n地点: \"{{状态栏.地点}}\"\n</Status_block>".to_owned()),
            rules: BTreeMap::from([(
                "状态栏.地点".to_owned(),
                rule(FieldKind::Text, UpdateMode::Replace),
            )]),
            guide: "第二輪指引".to_owned(),
        }),
        mechanisms: Vec::new(),
        entries: Vec::new(),
        deletable_shared_uids: Vec::new(),
        dropped: Vec::new(),
        unabsorbed: Vec::new(),
        audit: Vec::new(),
        preserve_source_uids: Vec::new(),
    };
    let selection = RefactorSelection {
        apply_interface: true,
        ..no_player_selection(Vec::new())
    };
    apply_recorded(root.path(), &world_id, &outcome, &selection);
    assert_eq!(
        data::read_state(root.path(), &world_id)
            .unwrap()
            .mechanism
            .guide,
        "第二輪指引"
    );
    receipts::undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert_eq!(
        data::read_state(root.path(), &world_id)
            .unwrap()
            .mechanism
            .guide,
        "第一輪指引"
    );
}

/// 混合條目：同一條來源的設定段套用成功，但狀態欄段失敗（介面合併衝突，產物不帶介面）；失敗 uid 列在
/// preserve_source_uids，套用後來源條目必須還在——否則狀態欄那部分內容憑空消失。
#[test]
fn preserved_source_survives_when_another_part_of_it_failed() {
    let root = TestRoot::new("statusbar-preserve");
    let world_id = data::create_world(root.path(), "驛站").unwrap();
    let source_uid = seed_entry(root.path(), &world_id, "混合條目", "設定段＋狀態欄段");
    let outcome = |preserve: Vec<String>| RefactorOutcome {
        mode: Some("interface".to_owned()),
        characters: Vec::new(),
        interface: None,
        mechanisms: Vec::new(),
        entries: vec![crate::refactor_ai::RefactorNewEntry {
            title: "設定".to_owned(),
            kind: "setting".to_owned(),
            content: "重寫的設定段".to_owned(),
            source_uids: vec![source_uid.to_string()],
            rules: BTreeMap::new(),
            triggers: Vec::new(),
            meta: None,
        }],
        deletable_shared_uids: Vec::new(),
        dropped: Vec::new(),
        unabsorbed: Vec::new(),
        audit: Vec::new(),
        preserve_source_uids: preserve,
    };
    let selection = RefactorSelection {
        entry_indices: vec![0],
        ..no_player_selection(Vec::new())
    };
    let has_source = |root: &TestRoot| {
        data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .iter()
            .any(|entry| entry.uid == source_uid && !entry.disabled)
    };

    apply(
        root.path(),
        &world_id,
        &outcome(vec![source_uid.to_string()]),
        &selection,
    )
    .unwrap();
    assert!(has_source(&root));
    assert!(data::read_worldbook(root.path(), &world_id)
        .unwrap()
        .iter()
        .any(|entry| entry.content == "重寫的設定段"));

    // 對照：沒有失敗時同一條來源照常被消耗
    let other = TestRoot::new("statusbar-preserve-control");
    let world_id = data::create_world(other.path(), "驛站").unwrap();
    let control_uid = seed_entry(other.path(), &world_id, "混合條目", "設定段＋狀態欄段");
    assert_eq!(control_uid, source_uid);
    apply(other.path(), &world_id, &outcome(Vec::new()), &selection).unwrap();
    assert!(!has_source(&other));
}

/// 套用時記下原卡 STATE 的數字／布林欄位型別：狀態樹的葉子都轉成字串，填骨架時靠這份決定卡讀回的型別。
#[test]
fn apply_records_original_value_types_for_the_skeleton() {
    let root = TestRoot::new("statusbar-types");
    let world_id = data::create_world(root.path(), "驛站").unwrap();
    let outcome = RefactorOutcome {
        mode: Some("interface".to_owned()),
        characters: Vec::new(),
        interface: Some(RefactorInterface {
            state_fields: serde_json::json!({ "状态栏": { "粮草": 320, "开仓": true, "地点": "驿站", "人物": "林远, 周掌柜" } }),
            source_uids: Vec::new(),
            raw: String::new(),
            shell: Some("<Status_block>\n粮草: {{状态栏.粮草}}\n开仓: {{状态栏.开仓}}\n地点: {{状态栏.地点}}\n人物: [{{状态栏.人物}}]\n</Status_block>".to_owned()),
            rules: BTreeMap::from([
                ("状态栏.粮草".to_owned(), rule(FieldKind::Number, UpdateMode::Delta)),
                ("状态栏.开仓".to_owned(), rule(FieldKind::Text, UpdateMode::Replace)),
                ("状态栏.地点".to_owned(), rule(FieldKind::Text, UpdateMode::Replace)),
                ("状态栏.人物".to_owned(), rule(FieldKind::List, UpdateMode::Replace)),
            ]),
            guide: "粮草變動才報".to_owned(),
        }),
        mechanisms: Vec::new(),
        entries: Vec::new(),
        deletable_shared_uids: Vec::new(),
        dropped: Vec::new(),
        unabsorbed: Vec::new(),
        audit: Vec::new(),
        preserve_source_uids: Vec::new(),
    };
    let selection = RefactorSelection {
        apply_interface: true,
        ..no_player_selection(Vec::new())
    };
    apply(root.path(), &world_id, &outcome, &selection).unwrap();
    let types = data::read_state(root.path(), &world_id)
        .unwrap()
        .mechanism
        .value_types;
    assert_eq!(
        types,
        BTreeMap::from([
            ("状态栏.粮草".to_owned(), "number".to_owned()),
            ("状态栏.开仓".to_owned(), "bool".to_owned()),
            ("状态栏.人物".to_owned(), "list".to_owned()),
        ])
    );
}

fn typed_outcome(grain: serde_json::Value) -> RefactorOutcome {
    RefactorOutcome {
        mode: Some("interface".to_owned()),
        characters: Vec::new(),
        interface: Some(RefactorInterface {
            state_fields: serde_json::json!({ "状态栏": { "粮草": grain } }),
            source_uids: Vec::new(),
            raw: String::new(),
            shell: Some("<Status_block>\n粮草: {{状态栏.粮草}}\n</Status_block>".to_owned()),
            rules: BTreeMap::from([(
                "状态栏.粮草".to_owned(),
                rule(FieldKind::Text, UpdateMode::Replace),
            )]),
            guide: "粮草變動才報".to_owned(),
        }),
        mechanisms: Vec::new(),
        entries: Vec::new(),
        deletable_shared_uids: Vec::new(),
        dropped: Vec::new(),
        unabsorbed: Vec::new(),
        audit: Vec::new(),
        preserve_source_uids: Vec::new(),
    }
}

fn value_types(root: &TestRoot, world_id: &str) -> BTreeMap<String, String> {
    data::read_state(root.path(), world_id)
        .unwrap()
        .mechanism
        .value_types
}

/// 第二次套用把數字欄換成字串欄（型別表少了那一項）；undo 寫回第一次的整份型別表，骨架才會照舊讀成數字。
#[test]
fn undo_restores_value_types_after_number_to_string_replacement() {
    let root = TestRoot::new("statusbar-types-undo");
    let world_id = data::create_world(root.path(), "驛站").unwrap();
    let selection = RefactorSelection {
        apply_interface: true,
        ..no_player_selection(Vec::new())
    };
    apply_recorded(
        root.path(),
        &world_id,
        &typed_outcome(serde_json::json!(320)),
        &selection,
    );
    let first = BTreeMap::from([("状态栏.粮草".to_owned(), "number".to_owned())]);
    assert_eq!(value_types(&root, &world_id), first);

    apply_recorded(
        root.path(),
        &world_id,
        &typed_outcome(serde_json::json!("三百石")),
        &selection,
    );
    assert!(value_types(&root, &world_id).is_empty());

    receipts::undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert_eq!(value_types(&root, &world_id), first);
}

/// 首次套用寫入型別表，undo 退回原本的空表。
#[test]
fn undo_clears_value_types_written_by_first_apply() {
    let root = TestRoot::new("statusbar-types-first");
    let world_id = data::create_world(root.path(), "驛站").unwrap();
    assert!(value_types(&root, &world_id).is_empty());
    let selection = RefactorSelection {
        apply_interface: true,
        ..no_player_selection(Vec::new())
    };
    apply_recorded(
        root.path(),
        &world_id,
        &typed_outcome(serde_json::json!(320)),
        &selection,
    );
    assert!(!value_types(&root, &world_id).is_empty());
    receipts::undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert!(value_types(&root, &world_id).is_empty());
}
