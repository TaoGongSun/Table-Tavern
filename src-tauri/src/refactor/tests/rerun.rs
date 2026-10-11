//! 重新重構（refactor-statusbar-skeleton 待問 1）：已遊玩擋、來源不完整擋、未遊玩照匯入原檔清回剛匯入的
//! 狀態；清回在臨時資料根完整重建、成功才換掉原桌，任何失敗原桌原封不動。
use super::super::test_support::*;
use super::super::*;
use crate::data::{self, FieldKind, FieldRule, InjectLevel, UpdateMode};
use crate::{import, receipts};
use std::collections::BTreeMap;
use std::path::Path;

const CARD: &str = r#"{"spec":"chara_card_v3","data":{"name":"驛站","first_mes":"十月初七，北風。","alternate_greetings":["另一則開場"],"character_book":{"name":"驛站","entries":[{"keys":[],"constant":true,"content":"每回合輸出 <Status_block> YAML","comment":"美化状态栏","enabled":true},{"keys":["驛站"],"content":"北境驛站設定","comment":"驛站","enabled":true}]}}}"#;
const NPC: &str = r#"{"spec":"chara_card_v3","data":{"name":"周掌櫃","description":"軍需官","first_mes":"林大人辛苦了。"}}"#;

fn imported_world(root: &TestRoot) -> String {
    let world_id = data::create_world(root.path(), "北境驛站").unwrap();
    let source = import::import_worldbook_file(
        root.path(),
        &world_id,
        CARD.as_bytes(),
        "驛站",
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap()
    .source;
    import::post_opening_text(
        root.path(),
        &world_id,
        0,
        "2026-10-03T10:00:00",
        "另一則開場",
        "zh-TW",
        Some(1),
        source.as_deref(),
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    world_id
}

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

fn interface_outcome(source_uid: u64) -> RefactorOutcome {
    RefactorOutcome {
        mode: Some("interface".to_owned()),
        characters: Vec::new(),
        interface: Some(RefactorInterface {
            state_fields: serde_json::json!({ "糧草": 320, "地點": "前院" }),
            source_uids: vec![source_uid.to_string()],
            raw: String::new(),
            shell: Some(
                "<Status_block>\n粮草: {{糧草}}\n地点: {{地點}}\n</Status_block>".to_owned(),
            ),
            rules: BTreeMap::from([
                (
                    "糧草".to_owned(),
                    rule(FieldKind::Number, UpdateMode::Delta),
                ),
                (
                    "地點".to_owned(),
                    rule(FieldKind::Text, UpdateMode::Replace),
                ),
            ]),
            guide: "糧草變動才報".to_owned(),
        }),
        mechanisms: Vec::new(),
        entries: Vec::new(),
        deletable_shared_uids: Vec::new(),
        dropped: Vec::new(),
        unabsorbed: Vec::new(),
        audit: Vec::new(),
        preserve_source_uids: Vec::new(),
        source_fingerprints: Default::default(),
    }
}

/// AI 重構套用（不留收據，與正式 AI 路徑一致），外加一張重構拆出的角色卡
fn refactor(root: &TestRoot, world_id: &str) {
    let source_uid = data::read_worldbook(root.path(), world_id)
        .unwrap()
        .into_iter()
        .find(|entry| entry.title == "美化状态栏")
        .unwrap()
        .uid;
    let selection = RefactorSelection {
        apply_interface: true,
        ..no_player_selection(Vec::new())
    };
    apply(
        root.path(),
        world_id,
        &interface_outcome(source_uid),
        &selection,
    )
    .unwrap();
    data::write_character(
        root.path(),
        world_id,
        &data::CharacterCard {
            id: data::new_id(),
            name: "拆出來的".to_owned(),
            color: "#888".to_owned(),
            avatar: "🙂".to_owned(),
            tier: data::Tier::Balanced,
            show_image: true,
            archived: false,
            gen_prompt: String::new(),
            public_md: "拆出來的".to_owned(),
            private_md: String::new(),
        },
    )
    .unwrap();
}

/// 整桌目錄的內容指紋（檔名＋內容），用來確認失敗時原桌一個位元都沒動
fn fingerprint(root: &Path, world_id: &str) -> Vec<(String, Vec<u8>)> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
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

fn assert_untouched_after_failure(root: &TestRoot, world_id: &str, before: &[(String, Vec<u8>)]) {
    assert!(reset_to_import_source(root.path(), world_id, "zh-TW").is_err());
    assert_eq!(
        fingerprint(root.path(), world_id),
        before,
        "失敗時原桌不得改動"
    );
    let worlds = root.path().join("worlds");
    for name in [
        format!(".tt-reset-{world_id}"),
        format!(".tt-staging-{world_id}"),
        format!(".tt-trash-{world_id}"),
        format!(".tt-op-{world_id}.json"),
    ] {
        assert!(!worlds.join(&name).exists(), "殘留 {name}");
    }
    assert_eq!(
        rerun_status(root.path(), world_id).unwrap(),
        RerunStatus::Ready
    );
}

fn source_files(root: &TestRoot, world_id: &str) -> Vec<std::path::PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(root.path().join("worlds").join(world_id))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("import-source-")
        })
        .collect();
    files.sort();
    files
}

/// 做一次匯入，回傳這次新存的原檔路徑
fn import_and_get_source(
    root: &TestRoot,
    world_id: &str,
    run: impl FnOnce(),
) -> std::path::PathBuf {
    let before = source_files(root, world_id);
    run();
    source_files(root, world_id)
        .into_iter()
        .find(|path| !before.contains(path))
        .unwrap()
}

#[test]
fn rerun_status_covers_fresh_ready_played_and_no_source() {
    let root = TestRoot::new("rerun-status");
    let world_id = imported_world(&root);
    assert_eq!(
        rerun_status(root.path(), &world_id).unwrap(),
        RerunStatus::Fresh
    );
    refactor(&root, &world_id);
    assert_eq!(
        rerun_status(root.path(), &world_id).unwrap(),
        RerunStatus::Ready
    );

    // 開場白以外多一則訊息＝已遊玩，擋下時不動任何檔案
    let played = data::TranscriptEvent {
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
        ts: "2026-10-03T10:01:00".to_owned(),
        speaker_id: String::new(),
        speaker_name: "玩家".to_owned(),
        kind: data::TranscriptKind::Player,
        text: "我進門".to_owned(),
        raw: None,
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
    };
    data::append_transcript(root.path(), &world_id, 0, &played).unwrap();
    assert_eq!(
        rerun_status(root.path(), &world_id).unwrap(),
        RerunStatus::Played
    );
    let before = fingerprint(root.path(), &world_id);
    assert!(reset_to_import_source(root.path(), &world_id, "zh-TW").is_err());
    assert_eq!(fingerprint(root.path(), &world_id), before);

    // 舊桌：重構過、匯入時沒留原檔（直接寫資料、沒有收據）
    let old = data::create_world(root.path(), "舊桌").unwrap();
    data::import_worldbook(
        root.path(),
        &old,
        &import::worldbook_json(CARD.as_bytes()).unwrap(),
    )
    .unwrap();
    refactor(&root, &old);
    assert_eq!(
        rerun_status(root.path(), &old).unwrap(),
        RerunStatus::NoSource
    );
    assert!(reset_to_import_source(root.path(), &old, "zh-TW").is_err());
}

#[test]
fn reset_restores_the_freshly_imported_table() {
    let root = TestRoot::new("rerun-reset");
    let world_id = imported_world(&root);
    let fresh_state = data::read_state(root.path(), &world_id).unwrap();
    let fresh_book = data::read_worldbook(root.path(), &world_id).unwrap();
    let fresh_events = data::read_transcript(root.path(), &world_id, 0).unwrap();

    refactor(&root, &world_id);
    assert!(data::read_interface_shell(root.path(), &world_id)
        .unwrap()
        .is_some());

    reset_to_import_source(root.path(), &world_id, "zh-TW").unwrap();

    let state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(state.name, "北境驛站");
    assert_eq!(state.refactor_mode, None);
    assert_eq!(state.mechanism, fresh_state.mechanism);
    assert_eq!(state.state, fresh_state.state);
    assert!(data::read_interface_shell(root.path(), &world_id)
        .unwrap()
        .is_none());
    assert!(data::read_refactor_outcome(root.path(), &world_id)
        .unwrap()
        .is_none());
    let book = data::read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(
        book.iter()
            .map(|entry| (&entry.title, &entry.content))
            .collect::<Vec<_>>(),
        fresh_book
            .iter()
            .map(|entry| (&entry.title, &entry.content))
            .collect::<Vec<_>>(),
    );
    assert!(data::list_characters(root.path(), &world_id)
        .unwrap()
        .is_empty());
    let events = data::read_transcript(root.path(), &world_id, 0).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].text, fresh_events[0].text);
    assert_eq!(events[0].ts, fresh_events[0].ts);
    // 開場白序號跟著原本那筆匯入，臨時根已清掉，可以再清回一次
    assert_eq!(
        import::chosen_opening(root.path(), &world_id).as_deref(),
        Some("另一則開場")
    );
    assert!(!root
        .path()
        .join("worlds")
        .join(format!(".tt-reset-{world_id}"))
        .exists());
    assert_eq!(
        rerun_status(root.path(), &world_id).unwrap(),
        RerunStatus::Fresh
    );
    refactor(&root, &world_id);
    reset_to_import_source(root.path(), &world_id, "zh-TW").unwrap();
    assert_eq!(
        data::read_state(root.path(), &world_id).unwrap().mechanism,
        fresh_state.mechanism
    );
}

#[test]
fn reset_replays_character_route_imports() {
    let root = TestRoot::new("rerun-character");
    let world_id = imported_world(&root);
    import::import_character_file(
        root.path(),
        &world_id,
        NPC.as_bytes(),
        "#abc",
        "zh-TW",
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    refactor(&root, &world_id);
    assert_eq!(
        data::list_characters(root.path(), &world_id).unwrap().len(),
        2
    );
    reset_to_import_source(root.path(), &world_id, "zh-TW").unwrap();
    let characters = data::list_characters(root.path(), &world_id).unwrap();
    assert_eq!(characters.len(), 1);
    assert_eq!(characters[0].name, "周掌櫃");
    assert_eq!(characters[0].color, "#abc");
}

/// 角色原檔壞掉：重建在臨時根失敗，原桌原封不動、臨時根清乾淨
#[test]
fn broken_character_source_leaves_the_table_untouched() {
    let root = TestRoot::new("rerun-bad-character");
    let world_id = imported_world(&root);
    let npc = import_and_get_source(&root, &world_id, || {
        import::import_character_file(
            root.path(),
            &world_id,
            NPC.as_bytes(),
            "#abc",
            "zh-TW",
            &crate::data::test_exclusive(&world_id),
        )
        .unwrap();
    });
    refactor(&root, &world_id);
    std::fs::write(npc, b"{ not a card").unwrap();
    let before = fingerprint(root.path(), &world_id);
    assert_untouched_after_failure(&root, &world_id, &before);
}

/// 第二筆匯入失敗（第一筆已在臨時根重匯成功）：原桌原封不動
#[test]
fn second_import_failure_leaves_the_table_untouched() {
    let root = TestRoot::new("rerun-second-fails");
    let world_id = imported_world(&root);
    let second = import_and_get_source(&root, &world_id, || {
        let book = CARD
            .replace("北境驛站設定", "第二本設定")
            .replace("美化状态栏", "第二條");
        import::import_worldbook_file(
            root.path(),
            &world_id,
            book.as_bytes(),
            "第二本",
            &crate::data::test_exclusive(&world_id),
        )
        .unwrap();
    });
    refactor(&root, &world_id);
    // 解得開 JSON、但沒有任何世界書條目：重匯這筆會失敗
    std::fs::write(second, br#"{"data":{"name":"x"}}"#).unwrap();
    let before = fingerprint(root.path(), &world_id);
    assert_untouched_after_failure(&root, &world_id, &before);
}

/// 交換時寫入失敗（重建桌搬不上位）：原桌原封不動、臨時根與 staging 清乾淨
#[test]
fn swap_write_failure_leaves_the_table_untouched() {
    let root = TestRoot::new("rerun-swap-fails");
    let world_id = imported_world(&root);
    refactor(&root, &world_id);
    let before = fingerprint(root.path(), &world_id);
    let _guard = data::RenameFailGuard::fail(1);
    assert!(reset_to_import_source(root.path(), &world_id, "zh-TW").is_err());
    drop(_guard);
    assert_eq!(fingerprint(root.path(), &world_id), before);
    assert!(!root
        .path()
        .join("worlds")
        .join(format!(".tt-reset-{world_id}"))
        .exists());
    assert!(!root
        .path()
        .join("worlds")
        .join(format!(".tt-staging-{world_id}"))
        .exists());
    reset_to_import_source(root.path(), &world_id, "zh-TW").unwrap();
}

/// 收據寫壞（第二筆的原檔記不進去）或開場序號記不下來：標記來源不完整，重設擋下
#[test]
fn partial_source_records_block_the_reset() {
    let root = TestRoot::new("rerun-partial");
    let world_id = imported_world(&root);
    let receipts_path = root
        .path()
        .join("worlds")
        .join(&world_id)
        .join("import-receipts.json");
    std::fs::write(&receipts_path, b"[ broken").unwrap();
    import::import_character_file(
        root.path(),
        &world_id,
        NPC.as_bytes(),
        "#abc",
        "zh-TW",
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    refactor(&root, &world_id);
    assert_eq!(
        rerun_status(root.path(), &world_id).unwrap(),
        RerunStatus::NoSource
    );
    assert!(reset_to_import_source(root.path(), &world_id, "zh-TW").is_err());

    // 開場白序號記不下來（收據讀不了）
    let other = TestRoot::new("rerun-partial-opening");
    let world_id = data::create_world(other.path(), "驛站").unwrap();
    let source = import::import_worldbook_file(
        other.path(),
        &world_id,
        CARD.as_bytes(),
        "驛站",
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap()
    .source;
    let receipts_path = other
        .path()
        .join("worlds")
        .join(&world_id)
        .join("import-receipts.json");
    std::fs::write(&receipts_path, b"[ broken").unwrap();
    import::post_opening_text(
        other.path(),
        &world_id,
        0,
        "t",
        "另一則開場",
        "zh-TW",
        Some(1),
        source.as_deref(),
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    std::fs::write(&receipts_path, b"[]").unwrap();
    refactor(&other, &world_id);
    assert_eq!(
        rerun_status(other.path(), &world_id).unwrap(),
        RerunStatus::NoSource
    );
}

/// 匯入 A、B，撤銷 B 再重設：B 不會復活，開場白選擇也不會跑到 B
#[test]
fn undone_import_does_not_come_back_on_reset() {
    let root = TestRoot::new("rerun-undo");
    let world_id = imported_world(&root);
    let npc = import::import_character_file(
        root.path(),
        &world_id,
        NPC.as_bytes(),
        "#abc",
        "zh-TW",
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap()
    .source;
    import::post_opening_text(
        root.path(),
        &world_id,
        0,
        "2026-10-03T10:05:00",
        "林大人辛苦了。",
        "zh-TW",
        Some(0),
        npc.as_deref(),
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert_eq!(
        import::chosen_opening(root.path(), &world_id).as_deref(),
        Some("林大人辛苦了。")
    );
    receipts::undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert_eq!(
        import::chosen_opening(root.path(), &world_id).as_deref(),
        Some("另一則開場")
    );
    // B 的原檔跟著收據刪掉，只剩 A 的
    assert_eq!(source_files(&root, &world_id).len(), 1);
    refactor(&root, &world_id);
    reset_to_import_source(root.path(), &world_id, "zh-TW").unwrap();
    assert!(data::list_characters(root.path(), &world_id)
        .unwrap()
        .is_empty());
    assert_eq!(
        data::read_transcript(root.path(), &world_id, 0)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        import::chosen_opening(root.path(), &world_id).as_deref(),
        Some("另一則開場")
    );
}

/// 重設整段持獨占鎖：有在途寫入（共用許可）或另一次重設（獨占）時回「這桌忙」，原桌不動；
/// 重設持鎖期間，新的寫入許可要排隊等它放開。
#[test]
fn reset_holds_the_world_exclusively() {
    let root = TestRoot::new("rerun-lock");
    let world_id = imported_world(&root);
    refactor(&root, &world_id);
    let before = fingerprint(root.path(), &world_id);
    {
        let _writer = data::world_write_permit(&world_id).unwrap();
        assert!(reset_to_import_source(root.path(), &world_id, "zh-TW").is_err());
    }
    {
        let _other_reset = data::try_world_exclusive(&world_id).unwrap();
        assert!(reset_to_import_source(root.path(), &world_id, "zh-TW").is_err());
    }
    assert_eq!(fingerprint(root.path(), &world_id), before);

    let held = data::try_world_exclusive(&world_id).unwrap();
    let id = world_id.clone();
    let waiter = std::thread::spawn(move || {
        let started = std::time::Instant::now();
        let _permit = data::world_write_permit(&id).unwrap();
        started.elapsed()
    });
    std::thread::sleep(std::time::Duration::from_millis(120));
    drop(held);
    assert!(waiter.join().unwrap() >= std::time::Duration::from_millis(100));
    assert_eq!(
        reset_to_import_source(root.path(), &world_id, "zh-TW").unwrap(),
        ResetOutcome::Committed
    );
}

/// 角色匯入寫到一半失敗（角色資料夾寫不進去）：未完成標記留著，來源判不完整
#[cfg(unix)]
#[test]
fn half_written_character_import_marks_sources_incomplete() {
    use std::os::unix::fs::PermissionsExt;
    let root = TestRoot::new("rerun-half-import");
    let world_id = imported_world(&root);
    let characters = root
        .path()
        .join("worlds")
        .join(&world_id)
        .join("characters");
    std::fs::set_permissions(&characters, std::fs::Permissions::from_mode(0o555)).unwrap();
    let failed = import::import_character_file(
        root.path(),
        &world_id,
        NPC.as_bytes(),
        "#abc",
        "zh-TW",
        &crate::data::test_exclusive(&world_id),
    );
    std::fs::set_permissions(&characters, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(failed.is_err());
    refactor(&root, &world_id);
    assert_eq!(
        rerun_status(root.path(), &world_id).unwrap(),
        RerunStatus::NoSource
    );

    // 對照：壞卡檔在動資料前就被擋，不留標記
    let other = TestRoot::new("rerun-bad-card-file");
    let world_id = imported_world(&other);
    assert!(import::import_character_file(
        other.path(),
        &world_id,
        b"{ broken",
        "#abc",
        "zh-TW",
        &crate::data::test_exclusive(&world_id)
    )
    .is_err());
    refactor(&other, &world_id);
    assert_eq!(
        rerun_status(other.path(), &world_id).unwrap(),
        RerunStatus::Ready
    );
}

/// 重建出來的來源或開場與預期不一致：禁止交換，原桌不動
#[test]
fn rebuilt_table_that_differs_is_not_swapped_in() {
    let root = TestRoot::new("rerun-mismatch");
    let world_id = imported_world(&root);
    refactor(&root, &world_id);
    // 收據記著開場白，但逐字稿裡那則不見了：重建時貼不回去，重放清單對不上
    let transcript = root
        .path()
        .join("worlds")
        .join(&world_id)
        .join("transcript")
        .join("0.jsonl");
    std::fs::write(&transcript, b"").unwrap();
    let before = fingerprint(root.path(), &world_id);
    assert!(reset_to_import_source(root.path(), &world_id, "zh-TW").is_err());
    assert_eq!(fingerprint(root.path(), &world_id), before);
    assert!(!root
        .path()
        .join("worlds")
        .join(format!(".tt-reset-{world_id}"))
        .exists());
}

/// A、B 世界書條目相同、開場白不同：B 什麼都沒新增、不留收據，B 的開場白不能掛到 A——歸屬不了就判
/// 來源不完整，A 的開場紀錄不被改寫
#[test]
fn duplicate_worldbook_opening_is_not_attributed_to_the_previous_import() {
    let root = TestRoot::new("rerun-duplicate-opening");
    let world_id = data::create_world(root.path(), "驛站").unwrap();
    let a = import::import_worldbook_file(
        root.path(),
        &world_id,
        CARD.as_bytes(),
        "A",
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap()
    .source;
    assert!(a.is_some());
    let b_card = CARD.replace("另一則開場", "B 卡的開場");
    let b = import::import_worldbook_file(
        root.path(),
        &world_id,
        b_card.as_bytes(),
        "B",
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap()
    .source;
    assert_eq!(b, None, "B 什麼都沒新增，沒有收據可綁");
    import::post_opening_text(
        root.path(),
        &world_id,
        0,
        "t",
        "B 卡的開場",
        "zh-TW",
        Some(1),
        b.as_deref(),
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert_eq!(import::chosen_opening(root.path(), &world_id), None);
    refactor(&root, &world_id);
    assert_eq!(
        rerun_status(root.path(), &world_id).unwrap(),
        RerunStatus::NoSource
    );
}

/// 交換的指定階段故障：原桌搬走後 staging 上位失敗、退回成功→原桌不動；上位與退回都失敗→回錯，下次
/// 開桌照恢復表換上已驗過的重建桌
#[test]
fn swap_failures_after_moving_the_original_away() {
    let root = TestRoot::new("rerun-swap-stage");
    let world_id = imported_world(&root);
    refactor(&root, &world_id);
    let before = fingerprint(root.path(), &world_id);
    {
        // 原桌搬去 trash 那一步失敗：staging 與日誌收掉，原桌不動。照目標路徑打，不數改名次數
        // （重建桌內的收據等原子寫也會改名）
        let _guard = data::RenameFailGuard::fail_ending(&format!(".tt-trash-{world_id}"), 1);
        assert!(reset_to_import_source(root.path(), &world_id, "zh-TW").is_err());
    }
    assert_eq!(fingerprint(root.path(), &world_id), before);
    {
        // 改名順序：重建桌→staging、日誌暫存→正式日誌、原桌→trash、staging→原位（失敗）、
        // trash→原位（退回）；後兩步的目標都是原位
        let _guard = data::RenameFailGuard::fail_ending(
            &format!("{}{world_id}", std::path::MAIN_SEPARATOR),
            1,
        );
        assert!(reset_to_import_source(root.path(), &world_id, "zh-TW").is_err());
    }
    assert_eq!(fingerprint(root.path(), &world_id), before);
    let worlds = root.path().join("worlds");
    for name in [
        format!(".tt-staging-{world_id}"),
        format!(".tt-trash-{world_id}"),
        format!(".tt-op-{world_id}.json"),
    ] {
        assert!(!worlds.join(&name).exists(), "殘留 {name}");
    }
    {
        let _guard = data::RenameFailGuard::fail_ending(
            &format!("{}{world_id}", std::path::MAIN_SEPARATOR),
            2,
        );
        assert!(reset_to_import_source(root.path(), &world_id, "zh-TW").is_err());
    }
    assert!(!worlds.join(&world_id).exists());
    assert_eq!(
        data::open_world(root.path(), &world_id).unwrap(),
        data::OpenWorld::Ready
    );
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(state.refactor_mode, None, "換上的是清回原卡的重建桌");
    for name in [
        format!(".tt-staging-{world_id}"),
        format!(".tt-trash-{world_id}"),
        format!(".tt-op-{world_id}.json"),
    ] {
        assert!(!worlds.join(&name).exists(), "殘留 {name}");
    }
}

/// 交換已提交、臨時根清不掉：回 committed_cleanup_pending，桌已換成清回原卡的重建桌、桌清單看不到殘留；
/// 下次開桌（持獨占）把殘留清掉
#[test]
fn committed_reset_with_stuck_build_root_is_cleaned_on_next_open() {
    let root = TestRoot::new("rerun-cleanup-pending");
    let world_id = imported_world(&root);
    refactor(&root, &world_id);
    let build_root = root
        .path()
        .join("worlds")
        .join(format!(".tt-reset-{world_id}"));
    {
        let _guard = data::RemoveFailGuard::fail_ending(&format!(".tt-reset-{world_id}"), 1);
        assert_eq!(
            reset_to_import_source(root.path(), &world_id, "zh-TW").unwrap(),
            ResetOutcome::CommittedCleanupPending
        );
    }
    assert!(build_root.exists(), "臨時根留著");
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(state.refactor_mode, None, "已換成清回原卡的重建桌");
    let listed: Vec<String> = data::list_worlds(root.path())
        .unwrap()
        .into_iter()
        .map(|world| world.id)
        .collect();
    assert_eq!(listed, vec![world_id.clone()]);

    assert_eq!(
        data::open_world(root.path(), &world_id).unwrap(),
        data::OpenWorld::Ready
    );
    assert!(!build_root.exists(), "開桌時清掉殘留");
    assert_eq!(
        rerun_status(root.path(), &world_id).unwrap(),
        RerunStatus::Fresh
    );
}
