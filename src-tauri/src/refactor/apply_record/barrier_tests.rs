//! AI 套用的復原屏障（refactor-undo-ai-apply）：AI 套用無還原〔作者裁決 2026-08-11〕，復原撤到屏障就停
//! 〔作者裁決 2026-10-29〕；套用沒留下變動時屏障要拿掉，不無故擋住更早的匯入。
use super::*;
use crate::import;
use crate::refactor::test_support::*;
use crate::refactor::{rerun_status, reset_to_import_source, RerunStatus};
use serde_json::{json, Value};

const LANG: &str = "zh-TW";

fn outcome_of(names: &[&str]) -> RefactorOutcome {
    serde_json::from_value(json!({
        "characters": names.iter().map(|name| json!({
            "name": name, "emoji": "🙂", "public_md": "", "private_md": "",
            "source_uids": [], "solo_entry_md": "",
        })).collect::<Vec<_>>(),
    }))
    .unwrap()
}

fn book(title: &str, content: &str) -> Vec<u8> {
    json!({"entries": {"0": {"uid": 0, "comment": title, "key": [], "content": content,
        "constant": true}}})
    .to_string()
    .into_bytes()
}

fn import_book(root: &Path, world_id: &str, title: &str, content: &str) {
    import::import_worldbook_file(
        root,
        world_id,
        &book(title, content),
        title,
        &data::test_exclusive(world_id),
    )
    .unwrap();
}

fn import_card(root: &Path, world_id: &str, name: &str) -> String {
    import::import_character_file(
        root,
        world_id,
        json!({"data": {"name": name, "description": "路人"}})
            .to_string()
            .as_bytes(),
        "#3366ff",
        LANG,
        &data::test_exclusive(world_id),
    )
    .unwrap()
    .value
    .id
}

/// 世界書匯入過一次的新桌
fn imported_world(label: &str) -> (TestRoot, String) {
    let root = TestRoot::new(label);
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    import_book(root.path(), &world_id, "設定", "北境驛站");
    (root, world_id)
}

fn apply_ai(
    root: &Path,
    world_id: &str,
    outcome: &RefactorOutcome,
    selection: &RefactorSelection,
) -> DataResult<RefactorApplySummary> {
    apply_and_record(
        root,
        world_id,
        LANG,
        outcome,
        selection,
        &[],
        RefactorApplyOrigin::Ai,
        &data::test_exclusive(world_id),
    )
}

/// 收據清單的結構（缺檔＝空清單）：比結構與順序，不比 bytes
fn receipts_value(root: &Path, world_id: &str) -> Value {
    let path = data::import_receipts_path(root, world_id).unwrap();
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap(),
        Err(_) => json!([]),
    }
}

fn kinds(root: &Path, world_id: &str) -> Vec<String> {
    receipts::list_import_receipts(root, world_id)
        .into_iter()
        .map(|receipt| receipt.kind)
        .collect()
}

fn expected_last(root: &Path, world_id: &str) -> receipts::ExpectedReceipt {
    let list = receipts::list_import_receipts(root, world_id);
    let last = list.last().unwrap().clone();
    receipts::ExpectedReceipt {
        count: list.len(),
        kind: last.kind,
        label: last.label,
        timestamp: last.timestamp,
        character_id: last.character_id,
    }
}

fn undo(root: &Path, world_id: &str) -> DataResult<receipts::UndoReport> {
    let expected = expected_last(root, world_id);
    receipts::undo_expected_import(root, world_id, &expected, &data::test_exclusive(world_id))
}

/// 整桌目錄的內容指紋（檔名＋內容）
fn fingerprint(root: &Path, world_id: &str) -> Vec<(String, Vec<u8>)> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        for path in entries {
            let name = path.strip_prefix(base).unwrap().display().to_string();
            if path.is_dir() {
                out.push((name, Vec::new()));
                walk(&path, base, out);
            } else {
                out.push((name, std::fs::read(&path).unwrap()));
            }
        }
    }
    let dir = root.join("worlds").join(world_id);
    let mut out = Vec::new();
    walk(&dir, &dir, &mut out);
    out
}

fn blocked(result: DataResult<receipts::UndoReport>) -> bool {
    matches!(result, Err(error) if error.to_string() == UiMsg::UndoBlockedByRefactor.to_string())
}

/// 回歸（症狀重現）：世界書匯入 → AI 套用 → 復原被拒，整桌逐檔不變。舊版會撤掉世界書匯入、留下半重構的桌。
#[test]
fn undo_stops_at_the_ai_apply_barrier() {
    let (root, world_id) = imported_world("barrier-block");
    apply_ai(
        root.path(),
        &world_id,
        &outcome_of(&["甲"]),
        &no_player_selection(vec![0]),
    )
    .unwrap();
    assert_eq!(
        kinds(root.path(), &world_id),
        vec!["worldbook", receipts::REFACTOR_AI_BARRIER_KIND]
    );
    let before = fingerprint(root.path(), &world_id);
    assert!(blocked(undo(root.path(), &world_id)));
    assert_eq!(fingerprint(root.path(), &world_id), before);
    // 自動回滾那條（嚴格撤銷）也不跨過屏障
    assert!(blocked(receipts::rollback_last_import(
        root.path(),
        &world_id,
        &data::test_exclusive(&world_id),
    )));
    assert_eq!(fingerprint(root.path(), &world_id), before);
}

/// 屏障之後匯入的角色卡照常可撤，撤完停在屏障
#[test]
fn character_import_after_barrier_undoes_down_to_the_barrier() {
    let (root, world_id) = imported_world("barrier-character");
    apply_ai(
        root.path(),
        &world_id,
        &outcome_of(&["甲"]),
        &no_player_selection(vec![0]),
    )
    .unwrap();
    let card = import_card(root.path(), &world_id, "路人乙");
    let report = undo(root.path(), &world_id).unwrap();
    assert_eq!(report.removed_character_ids, vec![card.clone()]);
    assert!(data::read_character(root.path(), &world_id, &card).is_err());
    assert!(blocked(undo(root.path(), &world_id)));
    assert_eq!(
        data::list_characters(root.path(), &world_id)
            .unwrap()
            .iter()
            .map(|card| card.name.as_str())
            .collect::<Vec<_>>(),
        vec!["甲"]
    );
}

/// 屏障之後匯入的世界書（帶初始值與規則）照常可撤：狀態樹、機制、世界書都回到那次匯入前（AI 套用後），屏障仍在
#[test]
fn worldbook_import_after_barrier_undoes_down_to_the_barrier() {
    let (root, world_id) = imported_world("barrier-worldbook");
    apply_ai(
        root.path(),
        &world_id,
        &outcome_of(&["甲"]),
        &no_player_selection(vec![0]),
    )
    .unwrap();
    let state_before = data::read_state(root.path(), &world_id).unwrap();
    let book_before = data::read_worldbook(root.path(), &world_id).unwrap();
    // 帶初始值與規則的世界書：會新增狀態樹與機制，撤回才驗得到這兩域
    let mechanism_book = json!({"entries": {
        "0": {"uid": 0, "comment": "[initvar] 初始值", "key": [], "disable": true,
            "content": "World:\n  Time: 清晨"},
        "1": {"uid": 1, "comment": "[mvu_update]规则", "key": [], "constant": true,
            "content": "变量更新规则:\n  Player:\n    HP:\n      type: number\n      range: 0-100"},
    }});
    import::import_worldbook_file(
        root.path(),
        &world_id,
        mechanism_book.to_string().as_bytes(),
        "後來的書",
        &data::test_exclusive(&world_id),
    )
    .unwrap();
    let state_mid = data::read_state(root.path(), &world_id).unwrap();
    assert!(state_mid.state.tree.contains_key("World"));
    assert!(state_mid.mechanism.rules.contains_key("Player.HP"));
    assert_ne!(
        serde_json::to_value(&state_mid.state).unwrap(),
        serde_json::to_value(&state_before.state).unwrap()
    );
    assert_ne!(
        serde_json::to_value(&state_mid.mechanism).unwrap(),
        serde_json::to_value(&state_before.mechanism).unwrap()
    );
    assert_ne!(
        data::read_worldbook(root.path(), &world_id).unwrap(),
        book_before
    );
    undo(root.path(), &world_id).unwrap();
    let state_after = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(
        serde_json::to_value(&state_after.state).unwrap(),
        serde_json::to_value(&state_before.state).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&state_after.mechanism).unwrap(),
        serde_json::to_value(&state_before.mechanism).unwrap()
    );
    assert_eq!(
        data::read_worldbook(root.path(), &world_id).unwrap(),
        book_before
    );
    assert_eq!(
        kinds(root.path(), &world_id).last().map(String::as_str),
        Some(receipts::REFACTOR_AI_BARRIER_KIND)
    );
    assert!(blocked(undo(root.path(), &world_id)));
}

/// 寫入前就被拒（桌上已有玩家卡）：屏障拿掉，收據結構不變，更早的世界書匯入照樣撤得掉
#[test]
fn rejection_before_writes_removes_the_barrier() {
    let (root, world_id) = imported_world("barrier-reject");
    // 用資料層直接建玩家卡：不跑 Card 套用，最後一筆才會是世界書匯入
    let player = data::CharacterCard {
        id: data::new_id(),
        name: "主角".to_owned(),
        color: "#000000".to_owned(),
        avatar: String::new(),
        tier: data::Tier::Balanced,
        show_image: true,
        archived: false,
        gen_prompt: String::new(),
        public_md: String::new(),
        private_md: String::new(),
        stage: false,
    };
    data::write_character(root.path(), &world_id, &player).unwrap();
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    state.player_card_id = Some(player.id.clone());
    data::write_state(root.path(), &world_id, &state).unwrap();
    let receipts_before = receipts_value(root.path(), &world_id);

    let error = apply_ai(
        root.path(),
        &world_id,
        &outcome_of(&["甲"]),
        &RefactorSelection {
            player_index: Some(0),
            ..no_player_selection(vec![0])
        },
    )
    .unwrap_err();
    assert_eq!(error.to_string(), UiMsg::PlayerCardExists.to_string());
    assert_eq!(receipts_value(root.path(), &world_id), receipts_before);
    undo(root.path(), &world_id).unwrap();
    assert!(receipts::list_import_receipts(root.path(), &world_id).is_empty());
}

/// 中途失敗、回滾成功：收據回到套用前（沒有殘留屏障）
#[test]
fn rolled_back_failure_removes_the_barrier() {
    let (root, world_id) = imported_world("barrier-rollback");
    let receipts_before = receipts_value(root.path(), &world_id);
    let error = {
        let _write = data::WriteFailGuard::partial_ending_after(".md", 1, 1);
        apply_ai(
            root.path(),
            &world_id,
            &outcome_of(&["甲", "乙", "丙"]),
            &no_player_selection(vec![0, 1, 2]),
        )
        .unwrap_err()
        .to_string()
    };
    assert!(error.contains("injected partial write"), "{error}");
    assert_eq!(receipts_value(root.path(), &world_id), receipts_before);
    assert!(data::list_characters(root.path(), &world_id)
        .unwrap()
        .is_empty());
}

/// 中途失敗、回滾也失敗：這次的收據疊在屏障上面，手動撤掉它之後停在屏障
#[test]
fn partial_failure_keeps_progress_receipt_above_the_barrier() {
    let (root, world_id) = imported_world("barrier-partial");
    let error = {
        let _write = data::WriteFailGuard::partial_ending_after(".md", 1, 1);
        let _remove = data::RemoveFailGuard::fail_ending(".md", 1);
        apply_ai(
            root.path(),
            &world_id,
            &outcome_of(&["甲", "乙", "丙"]),
            &no_player_selection(vec![0, 1, 2]),
        )
        .unwrap_err()
        .to_string()
    };
    assert!(error.contains("\"refactor_apply_partial\""), "{error}");
    assert_eq!(
        kinds(root.path(), &world_id),
        vec!["worldbook", receipts::REFACTOR_AI_BARRIER_KIND, "refactor"]
    );
    undo(root.path(), &world_id).unwrap();
    assert!(blocked(undo(root.path(), &world_id)));
}

/// 中途失敗、這次的收據沒寫成：屏障在最上面，擋住撤到更早的匯入
#[test]
fn unrecorded_failure_leaves_the_barrier_on_top() {
    let (root, world_id) = imported_world("barrier-no-receipt");
    let error = {
        let _write = data::WriteFailGuard::partial_ending_after(".md", 1, 1);
        // 屏障那次改名放行，這次收據那次失敗（整檔寫入的注入只能同時打一種結尾，收據改用改名注入）
        let _rename = data::RenameFailGuard::fail_ending_after("import-receipts.json", 1, 1);
        apply_ai(
            root.path(),
            &world_id,
            &outcome_of(&["甲", "乙"]),
            &no_player_selection(vec![0, 1]),
        )
        .unwrap_err()
        .to_string()
    };
    assert!(
        error.contains("refactor_apply_partial_no_receipt"),
        "{error}"
    );
    assert_eq!(
        kinds(root.path(), &world_id),
        vec!["worldbook", receipts::REFACTOR_AI_BARRIER_KIND]
    );
    assert!(blocked(undo(root.path(), &world_id)));
}

/// 屏障寫不進去（收據檔壞掉）：不套用，整桌零寫入
#[test]
fn barrier_write_failure_applies_nothing() {
    let (root, world_id) = imported_world("barrier-corrupt");
    let path = data::import_receipts_path(root.path(), &world_id).unwrap();
    std::fs::write(&path, b"{").unwrap();
    let before = fingerprint(root.path(), &world_id);
    let error = apply_ai(
        root.path(),
        &world_id,
        &outcome_of(&["甲"]),
        &no_player_selection(vec![0]),
    )
    .unwrap_err();
    assert_eq!(error.to_string(), UiMsg::RefactorBarrierFailed.to_string());
    assert_eq!(fingerprint(root.path(), &world_id), before);
}

/// 重新重構要重匯的原檔清單不受屏障影響；清回原卡後的新桌收據沒有屏障，原卡匯入又能復原
#[test]
fn rerun_reset_drops_the_barrier() {
    let (root, world_id) = imported_world("barrier-rerun");
    let replays_before = receipts::import_replays(root.path(), &world_id)
        .unwrap()
        .unwrap();
    apply_ai(
        root.path(),
        &world_id,
        &outcome_of(&["甲"]),
        &no_player_selection(vec![0]),
    )
    .unwrap();
    let replays_after = receipts::import_replays(root.path(), &world_id)
        .unwrap()
        .unwrap();
    assert_eq!(replays_after.len(), replays_before.len());
    for (after, before) in replays_after.iter().zip(&replays_before) {
        assert_eq!(after.source.label, before.source.label);
        assert_eq!(after.bytes, before.bytes);
    }
    assert_eq!(
        rerun_status(root.path(), &world_id).unwrap(),
        RerunStatus::Ready
    );
    reset_to_import_source(root.path(), &world_id, LANG).unwrap();
    assert_eq!(kinds(root.path(), &world_id), vec!["worldbook"]);
    undo(root.path(), &world_id).unwrap();
}

/// 掛到「最後一筆」的寫入遇到屏障在最上面：不掛上去（屏障撤不掉，掛上去的東西就再也收不掉）
#[test]
fn last_receipt_writers_skip_the_barrier() {
    let (root, world_id) = imported_world("barrier-last-writers");
    apply_ai(
        root.path(),
        &world_id,
        &outcome_of(&["甲"]),
        &no_player_selection(vec![0]),
    )
    .unwrap();
    let held = data::test_exclusive(&world_id);
    let before = receipts_value(root.path(), &world_id);
    assert!(receipts::record_posted_opening(
        root.path(),
        &world_id,
        0,
        "2026-10-29T10:00:00",
        None,
        None,
        &held,
    ));
    receipts::record_last_import_rename(root.path(), &world_id, "舊桌名", &held);
    assert_eq!(receipts_value(root.path(), &world_id), before);
}
