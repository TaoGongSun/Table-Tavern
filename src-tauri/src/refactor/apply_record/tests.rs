use super::*;
use crate::refactor::test_support::*;
use serde_json::json;

fn outcome_of(names: &[&str]) -> RefactorOutcome {
    serde_json::from_value(json!({
        "characters": names.iter().map(|name| json!({
            "name": name, "emoji": "🙂", "public_md": "", "private_md": "",
            "source_uids": [], "solo_entry_md": "",
        })).collect::<Vec<_>>(),
    }))
    .unwrap()
}

fn character_files(root: &Path, world_id: &str) -> usize {
    let dir = data::character_path(root, world_id, &data::new_id())
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    std::fs::read_dir(dir)
        .map(|entries| entries.filter_map(Result::ok).count())
        .unwrap_or(0)
}

/// 第二位建卡寫一半失敗：已建的第一位、寫一半的第二位都被撤掉，沒有收據、沒有存檔，
/// 重試只建出一份。
#[test]
fn second_character_failure_rolls_back_to_zero_writes() {
    let root = TestRoot::new("apply-rollback");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let held = data::test_exclusive(&world_id);
    let outcome = outcome_of(&["甲", "乙", "丙"]);
    let selection = no_player_selection(vec![0, 1, 2]);
    let worldbook_before = data::read_worldbook(root.path(), &world_id).unwrap();

    let error = {
        let _guard = data::WriteFailGuard::partial_ending_after(".md", 1, 1);
        apply_and_record(
            root.path(),
            &world_id,
            &outcome,
            &selection,
            &[],
            true,
            &held,
        )
        .unwrap_err()
        .to_string()
    };
    assert!(error.contains("injected partial write"), "{error}");
    assert!(data::list_characters(root.path(), &world_id)
        .unwrap()
        .is_empty());
    assert_eq!(character_files(root.path(), &world_id), 0);
    assert_eq!(
        data::read_worldbook(root.path(), &world_id).unwrap(),
        worldbook_before
    );
    assert!(receipts::list_import_receipts(root.path(), &world_id).is_empty());
    assert!(data::read_refactor_outcome(root.path(), &world_id)
        .unwrap()
        .is_none());

    apply_and_record(
        root.path(),
        &world_id,
        &outcome,
        &selection,
        &[],
        true,
        &held,
    )
    .unwrap();
    assert_eq!(
        data::list_characters(root.path(), &world_id).unwrap().len(),
        3
    );
}

/// 寫入前就被拒（玩家卡已存在）：沒東西可記，不得撤掉上一筆無關的收據。
#[test]
fn rejection_before_writes_keeps_earlier_receipt() {
    let root = TestRoot::new("apply-reject");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let held = data::test_exclusive(&world_id);
    let outcome = outcome_of(&["甲"]);
    let with_player = RefactorSelection {
        player_index: Some(0),
        ..no_player_selection(vec![0])
    };
    apply_and_record(
        root.path(),
        &world_id,
        &outcome,
        &with_player,
        &[],
        true,
        &held,
    )
    .unwrap();
    let error = apply_and_record(
        root.path(),
        &world_id,
        &outcome,
        &with_player,
        &[],
        true,
        &held,
    )
    .unwrap_err()
    .to_string();
    assert_eq!(error, UiMsg::PlayerCardExists.to_string());
    assert_eq!(
        receipts::list_import_receipts(root.path(), &world_id).len(),
        1
    );
    let player = data::read_state(root.path(), &world_id)
        .unwrap()
        .player_card_id
        .unwrap();
    assert!(data::read_character(root.path(), &world_id, &player).is_ok());
}

/// 雙重失敗（Sol 反例）：第二位寫卡半截失敗，自動回滾刪第一位也失敗 → 不吞錯、收據留著、
/// 回「可手動撤銷」；手動撤銷後重試只會有一份。
#[test]
fn rollback_failure_keeps_receipt_and_reports_partial() {
    let root = TestRoot::new("apply-double-fail");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let held = data::test_exclusive(&world_id);
    let outcome = outcome_of(&["甲", "乙", "丙"]);
    let selection = no_player_selection(vec![0, 1, 2]);

    let error = {
        let _write = data::WriteFailGuard::partial_ending_after(".md", 1, 1);
        let _remove = data::RemoveFailGuard::fail_ending(".md", 1);
        apply_and_record(
            root.path(),
            &world_id,
            &outcome,
            &selection,
            &[],
            true,
            &held,
        )
        .unwrap_err()
        .to_string()
    };
    assert!(error.contains("\"refactor_apply_partial\""), "{error}");
    assert_eq!(
        receipts::list_import_receipts(root.path(), &world_id).len(),
        1
    );

    receipts::undo_last_import(root.path(), &world_id, &held).unwrap();
    assert_eq!(character_files(root.path(), &world_id), 0);
    apply_and_record(
        root.path(),
        &world_id,
        &outcome,
        &selection,
        &[],
        true,
        &held,
    )
    .unwrap();
    assert_eq!(
        data::list_characters(root.path(), &world_id).unwrap().len(),
        3
    );
}

/// 這次的收據寫不進去、上一筆匯入的收據還在：回「沒有可撤銷紀錄」，不撤上一筆。
#[test]
fn unrecorded_failure_does_not_touch_the_earlier_receipt() {
    let root = TestRoot::new("apply-no-receipt");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let held = data::test_exclusive(&world_id);
    apply_and_record(
        root.path(),
        &world_id,
        &outcome_of(&["前一次"]),
        &no_player_selection(vec![0]),
        &[],
        true,
        &held,
    )
    .unwrap();
    let receipts_path = data::import_receipts_path(root.path(), &world_id).unwrap();
    let before = std::fs::read(&receipts_path).unwrap();
    // 收據暫存檔路徑被目錄佔住：這次的收據原子寫寫不進去
    let blocker = receipts_path.with_extension("json.tmp");
    std::fs::create_dir(&blocker).unwrap();
    std::fs::write(blocker.join("x"), b"x").unwrap();

    let error = {
        let _write = data::WriteFailGuard::partial_ending_after(".md", 1, 1);
        apply_and_record(
            root.path(),
            &world_id,
            &outcome_of(&["甲", "乙"]),
            &no_player_selection(vec![0, 1]),
            &[],
            true,
            &held,
        )
        .unwrap_err()
        .to_string()
    };
    assert!(
        error.contains("refactor_apply_partial_no_receipt"),
        "{error}"
    );
    assert_eq!(std::fs::read(&receipts_path).unwrap(), before);
    assert!(data::list_characters(root.path(), &world_id)
        .unwrap()
        .iter()
        .any(|card| card.name == "前一次"));
}

/// 套用一位合併兩條專屬來源的角色並指定為玩家：兩條來源整條刪除、狀態有玩家卡（撤銷要寫狀態）。
fn applied_with_deleted_sources(label: &str) -> (TestRoot, String, [u64; 2]) {
    let root = TestRoot::new(label);
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let held = data::test_exclusive(&world_id);
    let bio = seed_entry(root.path(), &world_id, "亞瑟人物设定", "亞瑟：劍術高超。");
    let trait_uid = seed_entry(root.path(), &world_id, "亞瑟性格", "亞瑟：沉默寡言。");
    let mut outcome = outcome_of(&["亞瑟"]);
    outcome.characters[0].source_uids = vec![bio.to_string(), trait_uid.to_string()];
    let selection = RefactorSelection {
        player_index: Some(0),
        ..no_player_selection(vec![0])
    };
    let summary = apply_and_record(
        root.path(),
        &world_id,
        &outcome,
        &selection,
        &[],
        true,
        &held,
    )
    .unwrap();
    assert_eq!(summary.deleted_entries, 2);
    (root, world_id, [bio, trait_uid])
}

fn titles(root: &Path, world_id: &str) -> Vec<(u64, String)> {
    let mut out: Vec<_> = data::read_worldbook(root, world_id)
        .unwrap()
        .into_iter()
        .map(|entry| (entry.uid, entry.title))
        .collect();
    out.sort();
    out
}

/// 嚴格撤銷插回來源之後，下一步（寫狀態）失敗：收據留著；再撤一次，來源各只有一份、uid 照舊。
#[test]
fn restored_sources_are_not_duplicated_when_a_later_step_fails() {
    let (root, world_id, uids) = applied_with_deleted_sources("redo-after-state-fail");
    let held = data::test_exclusive(&world_id);
    {
        let _guard = data::WriteFailGuard::partial_ending("state.json.tmp", 1);
        assert!(receipts::rollback_last_import(root.path(), &world_id, &held).is_err());
    }
    assert_eq!(
        receipts::list_import_receipts(root.path(), &world_id).len(),
        1
    );
    receipts::undo_last_import(root.path(), &world_id, &held).unwrap();
    assert_eq!(
        titles(root.path(), &world_id),
        vec![
            (uids[0], "亞瑟人物设定".to_owned()),
            (uids[1], "亞瑟性格".to_owned())
        ]
    );
    assert!(receipts::list_import_receipts(root.path(), &world_id).is_empty());
}

/// 嚴格撤銷全部做完、最後彈出收據失敗：收據留著；再撤一次不會多插一份來源。
#[test]
fn restored_sources_are_not_duplicated_when_popping_the_receipt_fails() {
    let (root, world_id, uids) = applied_with_deleted_sources("redo-after-pop-fail");
    let held = data::test_exclusive(&world_id);
    {
        let _guard = data::WriteFailGuard::partial_ending("import-receipts.json.tmp", 1);
        assert!(receipts::rollback_last_import(root.path(), &world_id, &held).is_err());
    }
    assert_eq!(
        receipts::list_import_receipts(root.path(), &world_id).len(),
        1
    );
    receipts::undo_last_import(root.path(), &world_id, &held).unwrap();
    assert_eq!(
        titles(root.path(), &world_id),
        vec![
            (uids[0], "亞瑟人物设定".to_owned()),
            (uids[1], "亞瑟性格".to_owned())
        ]
    );
    assert!(data::list_characters(root.path(), &world_id)
        .unwrap()
        .is_empty());
}

/// 原 uid 已被別的條目佔走：換新 uid 插回；重做時同內容已在就跳過。
#[test]
fn restore_falls_back_to_a_new_uid_once_when_the_old_one_is_taken() {
    let (root, world_id, uids) = applied_with_deleted_sources("redo-uid-taken");
    let held = data::test_exclusive(&world_id);
    // 佔走第一條來源的 uid
    data::restore_worldbook_entry(
        root.path(),
        &world_id,
        data::WorldbookEntry {
            uid: uids[0],
            title: "別的條目".to_owned(),
            keys: Vec::new(),
            content: "不相干".to_owned(),
            constant: false,
            order: 100,
            disabled: false,
            visibility: data::Visibility::Gm,
            is_person: false,
            locked: false,
        },
    )
    .unwrap();
    {
        let _guard = data::WriteFailGuard::partial_ending("import-receipts.json.tmp", 1);
        assert!(receipts::rollback_last_import(root.path(), &world_id, &held).is_err());
    }
    receipts::undo_last_import(root.path(), &world_id, &held).unwrap();
    let all = titles(root.path(), &world_id);
    assert_eq!(
        all.iter()
            .filter(|(_, title)| title == "亞瑟人物设定")
            .count(),
        1
    );
    assert_eq!(
        all.iter().filter(|(_, title)| title == "亞瑟性格").count(),
        1
    );
    assert!(all.contains(&(uids[0], "別的條目".to_owned())));
}

/// 重構記收據寫到一半（暫存檔半寫入）或改名失敗：先前的收據逐位元組不變、還列得出來，
/// 回報寫不進去（Failed）。
#[test]
fn refactor_receipt_append_failure_keeps_earlier_receipts_intact() {
    let root = TestRoot::new("receipt-append-atomic");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let held = data::test_exclusive(&world_id);
    apply_and_record(
        root.path(),
        &world_id,
        &outcome_of(&["前一次"]),
        &no_player_selection(vec![0]),
        &[],
        true,
        &held,
    )
    .unwrap();
    let receipts_path = data::import_receipts_path(root.path(), &world_id).unwrap();
    let earlier = std::fs::read(&receipts_path).unwrap();

    let record_new_card = |label: &str| {
        let before = receipts::snapshot_refactor(root.path(), &world_id).unwrap();
        let card = data::CharacterCard {
            id: data::new_id(),
            name: label.to_owned(),
            color: "#000000".to_owned(),
            avatar: String::new(),
            tier: data::Tier::Balanced,
            show_image: true,
            archived: false,
            gen_prompt: String::new(),
            public_md: String::new(),
            private_md: String::new(),
        };
        data::write_character(root.path(), &world_id, &card).unwrap();
        receipts::record_refactor_apply(
            root.path(),
            &world_id,
            "測試",
            vec![card.id],
            Vec::new(),
            Vec::new(),
            before,
            &held,
        )
    };
    {
        let _guard = data::WriteFailGuard::partial_ending("import-receipts.json.tmp", 1);
        assert_eq!(record_new_card("半寫入"), RefactorRecord::Failed);
    }
    assert_eq!(std::fs::read(&receipts_path).unwrap(), earlier);
    {
        let _guard = data::RenameFailGuard::fail(1);
        assert_eq!(record_new_card("改名失敗"), RefactorRecord::Failed);
    }
    assert_eq!(std::fs::read(&receipts_path).unwrap(), earlier);
    assert_eq!(
        receipts::list_import_receipts(root.path(), &world_id).len(),
        1
    );
}
