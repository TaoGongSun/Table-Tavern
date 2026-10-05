//! refactor-noshell-panel：套用後沒有新殼就清掉桌上的殼、收據記原殼與玩法標記、undo 寫回。
use super::super::test_support::*;
use super::super::*;
use crate::data::{self, FieldKind, FieldRule, InjectLevel, UpdateMode};
use crate::receipts;
use crate::ui_msg::UiMsg;
use std::collections::BTreeMap;
use std::path::Path;

fn interface(fields: serde_json::Value, shell: Option<&str>) -> RefactorInterface {
    RefactorInterface {
        state_fields: fields,
        source_uids: Vec::new(),
        raw: "狀態欄散文".to_owned(),
        shell: shell.map(str::to_owned),
        rules: BTreeMap::new(),
        guide: String::new(),
    }
}

fn outcome(mode: Option<&str>, interface: Option<RefactorInterface>) -> RefactorOutcome {
    RefactorOutcome {
        mode: mode.map(str::to_owned),
        characters: Vec::new(),
        interface,
        mechanisms: Vec::new(),
        entries: Vec::new(),
        deletable_shared_uids: Vec::new(),
        dropped: Vec::new(),
        unabsorbed: Vec::new(),
        audit: Vec::new(),
        preserve_source_uids: Vec::new(),
    }
}

fn selection(apply_interface: bool) -> RefactorSelection {
    RefactorSelection {
        apply_interface,
        ..no_player_selection(Vec::new())
    }
}

fn shell(root: &Path, world_id: &str) -> Option<String> {
    data::read_interface_shell(root, world_id).unwrap()
}

fn mode(root: &Path, world_id: &str) -> Option<String> {
    data::read_state(root, world_id).unwrap().refactor_mode
}

fn receipt_count(root: &Path, world_id: &str) -> usize {
    receipts::list_import_receipts(root, world_id).len()
}

/// 第一輪接管產殼，作為後續各案的起點。
fn takeover(root: &Path, world_id: &str, shell_text: &str) {
    apply_recorded(
        root,
        world_id,
        &outcome(
            Some("interface"),
            Some(interface(
                serde_json::json!({ "世界": { "時間": "清晨" } }),
                Some(shell_text),
            )),
        ),
        &selection(true),
    );
    assert_eq!(shell(root, world_id).as_deref(), Some(shell_text));
}

#[test]
fn rerun_without_shell_removes_previous_shell_and_undo_restores_it() {
    let root = TestRoot::new("noshell-rerun");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    takeover(root.path(), &world_id, "<UI>{{世界.時間}}</UI>");

    apply_recorded(
        root.path(),
        &world_id,
        &outcome(
            Some("interface"),
            Some(interface(
                serde_json::json!({ "驛站": { "糧草": "320" } }),
                None,
            )),
        ),
        &selection(true),
    );
    assert_eq!(shell(root.path(), &world_id), None);
    assert_eq!(mode(root.path(), &world_id).as_deref(), Some("interface"));

    receipts::undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert_eq!(
        shell(root.path(), &world_id).as_deref(),
        Some("<UI>{{世界.時間}}</UI>")
    );
    assert_eq!(mode(root.path(), &world_id).as_deref(), Some("interface"));
}

#[test]
fn unchecked_interface_still_clears_existing_shell() {
    let root = TestRoot::new("noshell-unchecked");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    takeover(root.path(), &world_id, "<UI>{{世界.時間}}</UI>");

    // 產物有殼、玩家沒勾介面：這次套用後桌上沒有新殼，舊殼一樣清
    apply_recorded(
        root.path(),
        &world_id,
        &outcome(
            Some("interface"),
            Some(interface(
                serde_json::json!({ "世界": { "時間": "黃昏" } }),
                Some("<UI>新</UI>"),
            )),
        ),
        &selection(false),
    );
    assert_eq!(shell(root.path(), &world_id), None);
}

#[test]
fn whitespace_shell_counts_as_no_shell() {
    let root = TestRoot::new("noshell-blank");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    takeover(root.path(), &world_id, "<UI>{{世界.時間}}</UI>");

    apply_recorded(
        root.path(),
        &world_id,
        &outcome(
            Some("interface"),
            Some(interface(
                serde_json::json!({ "世界": { "時間": "黃昏" } }),
                Some(" \n\t "),
            )),
        ),
        &selection(true),
    );
    assert!(!data::interface_shell_path(root.path(), &world_id)
        .unwrap()
        .exists());
}

#[test]
fn overwritten_shell_is_restored_by_undo() {
    let root = TestRoot::new("noshell-overwrite");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    takeover(root.path(), &world_id, "<UI>舊 {{世界.時間}}</UI>");

    apply_recorded(
        root.path(),
        &world_id,
        &outcome(
            Some("interface"),
            Some(interface(
                serde_json::json!({ "世界": { "時間": "黃昏" } }),
                Some("<UI>新 {{世界.時間}}</UI>"),
            )),
        ),
        &selection(true),
    );
    assert_eq!(
        shell(root.path(), &world_id).as_deref(),
        Some("<UI>新 {{世界.時間}}</UI>")
    );
    receipts::undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert_eq!(
        shell(root.path(), &world_id).as_deref(),
        Some("<UI>舊 {{世界.時間}}</UI>")
    );
}

#[test]
fn cleanup_only_apply_still_leaves_a_receipt() {
    let root = TestRoot::new("noshell-cleanup-only");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    takeover(root.path(), &world_id, "<UI>{{世界.時間}}</UI>");
    let receipts_before = receipt_count(root.path(), &world_id);

    // mode 不變、沒有任何產物：唯一的變動是清殼
    apply_recorded(
        root.path(),
        &world_id,
        &outcome(Some("interface"), None),
        &selection(false),
    );
    assert_eq!(shell(root.path(), &world_id), None);
    assert_eq!(receipt_count(root.path(), &world_id), receipts_before + 1);

    receipts::undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert_eq!(
        shell(root.path(), &world_id).as_deref(),
        Some("<UI>{{世界.時間}}</UI>")
    );
}

/// 只有玩法標記變動也產收據；undo 從磁碟讀回收據（serde 往返）後退回原值，null 也分得出來。
#[test]
fn mode_only_change_is_receipted_and_undone_after_round_trip() {
    let cases: [(Option<&str>, &str); 4] = [
        (None, "interface"),
        (None, "characters"),
        (Some("interface"), "characters"),
        (Some("characters"), "interface"),
    ];
    for (index, (before, after)) in cases.into_iter().enumerate() {
        let root = TestRoot::new(&format!("noshell-mode-{index}"));
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let mut state = data::read_state(root.path(), &world_id).unwrap();
        state.refactor_mode = before.map(str::to_owned);
        data::write_state(root.path(), &world_id, &state).unwrap();

        apply_recorded(
            root.path(),
            &world_id,
            &outcome(Some(after), None),
            &selection(false),
        );
        assert_eq!(mode(root.path(), &world_id).as_deref(), Some(after));
        assert_eq!(
            receipt_count(root.path(), &world_id),
            1,
            "{before:?}→{after}"
        );

        receipts::undo_last_import(
            root.path(),
            &world_id,
            &crate::data::test_exclusive(&world_id),
        )
        .unwrap();
        assert_eq!(
            mode(root.path(), &world_id).as_deref(),
            before,
            "{before:?}→{after}"
        );
    }
}

#[test]
fn first_takeover_undo_returns_table_to_unrefactored() {
    let root = TestRoot::new("noshell-first-undo");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    takeover(root.path(), &world_id, "<UI>{{世界.時間}}</UI>");

    receipts::undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert_eq!(shell(root.path(), &world_id), None);
    assert_eq!(mode(root.path(), &world_id), None);
    assert!(data::read_state(root.path(), &world_id)
        .unwrap()
        .state
        .tree
        .is_empty());
}

#[test]
fn preflight_rejection_keeps_shell_and_table_untouched() {
    let root = TestRoot::new("noshell-preflight");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    takeover(root.path(), &world_id, "<UI>{{世界.時間}}</UI>");
    let state_before = data::read_state(root.path(), &world_id).unwrap();

    // 介面路徑衝突（鏡像兩套值不同）
    let conflicting = outcome(
        Some("interface"),
        Some(interface(
            serde_json::json!({
                "地點": "王府",
                "日期時間": "清晨",
                "状态栏": { "地點": "霍府", "日期時間": "清晨" }
            }),
            Some("<UI>{{地點}}</UI>"),
        )),
    );
    assert!(apply(root.path(), &world_id, &conflicting, &selection(true)).is_err());

    // 桌上已有玩家卡又指定玩家
    let mut state = state_before.clone();
    state.player_card_id = Some("someone".to_owned());
    data::write_state(root.path(), &world_id, &state).unwrap();
    let mut with_player = outcome(Some("characters"), None);
    with_player.characters = vec![character("新來的人", &[])];
    let player_selection = RefactorSelection {
        player_index: Some(0),
        ..no_player_selection(vec![0])
    };
    let error = apply(root.path(), &world_id, &with_player, &player_selection).unwrap_err();
    assert_eq!(error.to_string(), UiMsg::PlayerCardExists.to_string());

    assert_eq!(
        shell(root.path(), &world_id).as_deref(),
        Some("<UI>{{世界.時間}}</UI>")
    );
    let after = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(after.refactor_mode, state_before.refactor_mode);
    assert_eq!(after.state.tree, state_before.state.tree);
    assert!(data::list_characters(root.path(), &world_id)
        .unwrap()
        .is_empty());
}

#[test]
fn shell_removal_failure_fails_apply_with_nothing_written() {
    let root = TestRoot::new("noshell-remove-fail");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    takeover(root.path(), &world_id, "<UI>{{世界.時間}}</UI>");
    let state_before = data::read_state(root.path(), &world_id).unwrap();
    let worldbook_before = data::read_worldbook(root.path(), &world_id).unwrap();

    let mut rerun = outcome(
        Some("interface"),
        Some(interface(
            serde_json::json!({ "驛站": { "糧草": "320" } }),
            None,
        )),
    );
    rerun.characters = vec![character("亞瑟", &[])];
    let result = {
        let _guard = data::RemoveFailGuard::fail(1);
        apply(
            root.path(),
            &world_id,
            &rerun,
            &RefactorSelection {
                apply_interface: true,
                ..no_player_selection(vec![0])
            },
        )
    };
    assert!(result.is_err());
    assert_eq!(
        shell(root.path(), &world_id).as_deref(),
        Some("<UI>{{世界.時間}}</UI>")
    );
    let after = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(after.state.tree, state_before.state.tree);
    assert_eq!(after.refactor_mode, state_before.refactor_mode);
    assert_eq!(
        data::read_worldbook(root.path(), &world_id).unwrap().len(),
        worldbook_before.len()
    );
    assert!(data::list_characters(root.path(), &world_id)
        .unwrap()
        .is_empty());
}

#[test]
fn unrelated_mechanism_values_survive_shell_cleanup() {
    let root = TestRoot::new("noshell-keep-mechanism");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    state.mechanism.incremental = true;
    state.mechanism.guide = "每回合報時間".to_owned();
    state.mechanism.rules.insert(
        "別處.金幣".to_owned(),
        FieldRule {
            kind: FieldKind::Number,
            min: Some(0.0),
            max: None,
            update: UpdateMode::Delta,
            inject: InjectLevel::Turn,
            branch: None,
            formula: None,
        },
    );
    data::write_state(root.path(), &world_id, &state).unwrap();
    data::write_interface_shell(root.path(), &world_id, "<UI>舊</UI>").unwrap();

    apply_recorded(
        root.path(),
        &world_id,
        &outcome(
            Some("interface"),
            Some(interface(
                serde_json::json!({ "驛站": { "糧草": "320" } }),
                None,
            )),
        ),
        &selection(true),
    );
    let after = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(shell(root.path(), &world_id), None);
    assert!(after.mechanism.incremental);
    assert_eq!(after.mechanism.guide, "每回合報時間");
    assert!(after.mechanism.rules.contains_key("別處.金幣"));
}

/// 接受的限制（不清沒有來源歸屬的機制值）：重跑後新樹已經沒有的欄位，舊 rules 還留著；
/// 模型照舊 guide 用 insert 回報時，該欄位會重新長回狀態樹。釘住現行行為，改動時要有意識。
#[test]
fn stale_rules_survive_rerun_and_insert_regrows_retired_field() {
    let root = TestRoot::new("noshell-stale-rules");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let mut first = interface(
        serde_json::json!({ "舊區": { "HP": "50" } }),
        Some("<UI>{{舊區.HP}}</UI>"),
    );
    first.rules.insert(
        "舊區.HP".to_owned(),
        FieldRule {
            kind: FieldKind::Number,
            min: Some(0.0),
            max: Some(100.0),
            update: UpdateMode::Delta,
            inject: InjectLevel::Turn,
            branch: None,
            formula: None,
        },
    );
    first.guide = "每回合報舊區.HP".to_owned();
    apply_recorded(
        root.path(),
        &world_id,
        &outcome(Some("interface"), Some(first)),
        &selection(true),
    );

    apply_recorded(
        root.path(),
        &world_id,
        &outcome(
            Some("interface"),
            Some(interface(
                serde_json::json!({ "驛站": { "糧草": "320" } }),
                None,
            )),
        ),
        &selection(true),
    );
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    assert!(!state.state.tree.contains_key("舊區"));
    assert!(state.mechanism.rules.contains_key("舊區.HP"));
    assert_eq!(state.mechanism.guide, "每回合報舊區.HP");
    assert!(state.mechanism.incremental);

    let block = crate::transport::extract_state_block(
        "劇情。\n<UpdateVariable>\n<JSONPatch>\n[{ \"op\": \"insert\", \"path\": \"/舊區/HP\", \"value\": 70 }]\n</JSONPatch>\n</UpdateVariable>",
    );
    crate::mechanism::apply_block(&mut state, &block, "玩家");
    assert!(state.state.tree.contains_key("舊區"));
}

#[test]
fn snapshot_refuses_unreadable_shell() {
    let root = TestRoot::new("noshell-snapshot");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let path = data::interface_shell_path(root.path(), &world_id).unwrap();
    std::fs::write(&path, [0xff, 0xfe, 0x00, 0xc3]).unwrap();
    assert!(receipts::snapshot_refactor(root.path(), &world_id).is_err());
}

#[test]
fn undo_shell_failures_keep_receipt_and_other_domains() {
    let root = TestRoot::new("noshell-undo-fail");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    takeover(root.path(), &world_id, "<UI>{{世界.時間}}</UI>");

    // 寫回舊殼失敗：Err、收據還在、玩法標記與狀態樹不動
    apply_recorded(
        root.path(),
        &world_id,
        &outcome(
            Some("characters"),
            Some(interface(
                serde_json::json!({ "驛站": { "糧草": "320" } }),
                None,
            )),
        ),
        &selection(false),
    );
    let state_mid = data::read_state(root.path(), &world_id).unwrap();
    // 殼路徑被目錄佔住：寫回必失敗
    let shell_path = data::interface_shell_path(root.path(), &world_id).unwrap();
    std::fs::create_dir(&shell_path).unwrap();
    assert!(receipts::undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id)
    )
    .is_err());
    assert_eq!(receipt_count(root.path(), &world_id), 2);
    assert_eq!(mode(root.path(), &world_id), state_mid.refactor_mode);
    std::fs::remove_dir(&shell_path).unwrap();
    receipts::undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert_eq!(
        shell(root.path(), &world_id).as_deref(),
        Some("<UI>{{世界.時間}}</UI>")
    );
    assert_eq!(mode(root.path(), &world_id).as_deref(), Some("interface"));

    // 刪除新建的殼失敗：Err、收據還在、狀態樹不動
    let tree_before = data::read_state(root.path(), &world_id).unwrap().state.tree;
    {
        let _guard = data::RemoveFailGuard::fail(1);
        assert!(receipts::undo_last_import(
            root.path(),
            &world_id,
            &crate::data::test_exclusive(&world_id)
        )
        .is_err());
    }
    assert_eq!(receipt_count(root.path(), &world_id), 1);
    assert!(shell(root.path(), &world_id).is_some());
    assert_eq!(
        data::read_state(root.path(), &world_id).unwrap().state.tree,
        tree_before
    );
    receipts::undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert_eq!(shell(root.path(), &world_id), None);
    assert_eq!(mode(root.path(), &world_id), None);
}

/// 匯出存檔讀的是原產物，不受清理與 undo 影響；同一份產物在新桌重匯，結果相同。
#[test]
fn saved_outcome_reimports_to_the_same_result() {
    let root = TestRoot::new("noshell-reimport");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    takeover(root.path(), &world_id, "<UI>{{世界.時間}}</UI>");
    let noshell = outcome(
        Some("interface"),
        Some(interface(
            serde_json::json!({ "驛站": { "糧草": "320" } }),
            None,
        )),
    );
    apply_recorded(root.path(), &world_id, &noshell, &selection(true));
    let saved = data::read_refactor_outcome(root.path(), &world_id)
        .unwrap()
        .unwrap();
    assert_eq!(parse_card(&saved).unwrap().outcome, noshell);
    receipts::undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert_eq!(
        data::read_refactor_outcome(root.path(), &world_id)
            .unwrap()
            .unwrap(),
        saved
    );

    let reimported = parse_card(&saved).unwrap().outcome;
    let fresh = data::create_world(root.path(), "新桌").unwrap();
    apply_recorded(root.path(), &fresh, &reimported, &selection(true));
    assert_eq!(shell(root.path(), &fresh), None);
    assert_eq!(mode(root.path(), &fresh).as_deref(), Some("interface"));
    assert!(data::read_state(root.path(), &fresh)
        .unwrap()
        .state
        .tree
        .contains_key("驛站"));
}
