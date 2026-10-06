use super::*;
use crate::import;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

mod race;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TestRoot(PathBuf);

impl TestRoot {
    fn new(label: &str) -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "table-tavern-receipts-{label}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn import_character_recorded(root: &Path, world_id: &str, raw: &[u8]) -> data::CharacterMeta {
    let before = snapshot(root, world_id);
    let meta = import::import_character(root, world_id, raw, "#3366ff", "zh-TW").unwrap();
    record_character_import(root, world_id, &meta.id, &meta.name, before, None);
    meta
}

fn import_worldbook_recorded(root: &Path, world_id: &str, label: &str, json_text: &str) {
    let before = snapshot(root, world_id);
    data::import_worldbook(root, world_id, json_text).unwrap();
    record_worldbook_import(root, world_id, label, before, None);
}

fn transcript_event(ts: &str, text: &str) -> data::TranscriptEvent {
    data::TranscriptEvent {
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
        ts: ts.to_owned(),
        speaker_id: String::new(),
        speaker_name: "GM".to_owned(),
        kind: data::TranscriptKind::Narration,
        text: text.to_owned(),
        raw: None,
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
    }
}

fn character_book_card(name: &str, entries: serde_json::Value) -> Vec<u8> {
    serde_json::json!({
        "data": { "name": name, "character_book": { "entries": entries } }
    })
    .to_string()
    .into_bytes()
}

/// 角色卡匯入→undo：角色 md／原始檔消失、帶入的世界書條目消失、匯入前既有同名條目仍在。
#[test]
fn undo_character_import_removes_card_and_its_entries_but_keeps_preexisting() {
    let root = TestRoot::new("character-basic");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let existing = data::upsert_worldbook_entry(
        root.path(),
        &world_id,
        data::WorldbookEntry {
            uid: 0,
            title: "既有設定".to_owned(),
            keys: vec!["鎮".to_owned()],
            content: "霧口鎮的既有設定".to_owned(),
            constant: true,
            order: 1,
            disabled: false,
            visibility: data::Visibility::Gm,
            is_person: false,
            locked: false,
        },
    )
    .unwrap();

    let raw = character_book_card(
        "莉亞",
        serde_json::json!([{"keys": ["森林"], "content": "古老盟約", "enabled": true}]),
    );
    let meta = import_character_recorded(root.path(), &world_id, &raw);
    let md_path = data::character_path(root.path(), &world_id, &meta.id).unwrap();
    assert!(md_path.exists());
    assert!(md_path.with_extension("import.json").exists());
    let entries_after_import = data::read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(entries_after_import.len(), 2);

    let report = undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert_eq!(report.removed_character, Some("莉亞".to_owned()));
    assert_eq!(report.removed_entries, 1);
    assert_eq!(report.kept_entries, 0);
    assert!(!report.renamed_back);

    assert!(data::read_character(root.path(), &world_id, &meta.id).is_err());
    assert!(!md_path.exists());
    assert!(!md_path.with_extension("import.json").exists());
    let remaining = data::read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].uid, existing);
    assert_eq!(remaining[0].title, "既有設定");
}

/// undo 時被玩家改過內容的條目要保留，且 kept_entries 正確計數。
#[test]
fn undo_keeps_entries_the_player_edited_since_import() {
    let root = TestRoot::new("character-kept");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let raw = character_book_card(
        "莉亞",
        serde_json::json!([
            {"keys": ["森林"], "content": "古老盟約", "enabled": true},
            {"keys": ["月亮"], "content": "月神信仰", "enabled": true}
        ]),
    );
    import_character_recorded(root.path(), &world_id, &raw);
    let entries = data::read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(entries.len(), 2);

    // 玩家改掉其中一條的內文
    let mut edited = entries[0].clone();
    edited.content = "玩家改過的內容".to_owned();
    data::upsert_worldbook_entry(root.path(), &world_id, edited).unwrap();

    let report = undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert_eq!(report.removed_entries, 1);
    assert_eq!(report.kept_entries, 1);
    let remaining = data::read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].content, "玩家改過的內容");
}

/// 世界書匯入（非角色卡路徑）→undo→條目消失。
#[test]
fn undo_worldbook_import_removes_its_entries() {
    let root = TestRoot::new("worldbook-basic");
    let world_id = data::create_world(root.path(), "世界").unwrap();
    let book = serde_json::json!({
        "entries": {
            "0": {"uid": 0, "key": ["城門"], "comment": "城門", "content": "城門已關。", "constant": false, "order": 1, "disable": false}
        }
    });
    import_worldbook_recorded(root.path(), &world_id, "worldbook.json", &book.to_string());
    assert_eq!(
        data::read_worldbook(root.path(), &world_id).unwrap().len(),
        1
    );

    let report = undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert_eq!(report.removed_entries, 1);
    assert!(report.removed_character.is_none());
    assert!(data::read_worldbook(root.path(), &world_id)
        .unwrap()
        .is_empty());
}

/// PNG 世界書匯入→undo：這次新建的 GM 卡圖跟著收掉（回到內建書本圖）；
/// 第二張 PNG 只是覆寫既有的圖，undo 不刪——那張圖不是這次匯入生出來的。
#[test]
fn undo_worldbook_import_removes_only_gm_image_created_this_time() {
    let root = TestRoot::new("worldbook-gm-image");
    let world_id = data::create_world(root.path(), "世界").unwrap();
    let image_path = data::gm_image_path(root.path(), &world_id).unwrap();
    let book = |content: &str| {
        serde_json::json!({
                "entries": {
                    "0": {"uid": 0, "key": ["城門"], "comment": "城門", "content": content, "constant": false, "order": 1, "disable": false}
                }
            })
            .to_string()
    };
    let import_png = |label: &str, json_text: &str, png: &[u8]| {
        let before = snapshot(root.path(), &world_id);
        data::import_worldbook(root.path(), &world_id, json_text).unwrap();
        assert_eq!(
            import::save_gm_image(root.path(), &world_id, png).unwrap(),
            import::GmImage::Saved
        );
        record_worldbook_import(root.path(), &world_id, label, before, None);
    };

    let first = crate::import::png_image::test_png::real_png(2, 2);
    let second = crate::import::png_image::test_png::real_png(3, 3);
    import_png("第一張.png", &book("城門已關。"), &first);
    assert!(image_path.exists());
    undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert!(!image_path.exists());

    import_png("第一張.png", &book("城門已關。"), &first);
    import_png("第二張.png", &book("城門又開了。"), &second);
    undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert_eq!(fs::read(&image_path).unwrap(), second);
}

/// 匯完貼上檯面的開場白→undo：那則跟著收掉，玩家自己後來加的話原封不動。
/// 重匯同一張多開場白的卡想改挑一則時，舊的那則不該還壓在開局上。
#[test]
fn undo_import_removes_the_posted_opening_but_keeps_later_events() {
    let root = TestRoot::new("posted-opening");
    let world_id = data::create_world(root.path(), "世界").unwrap();
    let book = serde_json::json!({
        "entries": {
            "0": {"uid": 0, "key": ["城門"], "comment": "城門", "content": "城門已關。", "constant": false, "order": 1, "disable": false}
        }
    });
    import_worldbook_recorded(root.path(), &world_id, "卡.png", &book.to_string());

    let opening = transcript_event("2026-08-07T10:00:00.000Z", "開場白配圖那一段");
    let mine = transcript_event("2026-08-07T10:05:00.000Z", "玩家後來自己加的一句");
    data::append_transcript(root.path(), &world_id, 0, &opening).unwrap();
    data::append_transcript(root.path(), &world_id, 0, &mine).unwrap();
    record_posted_opening(
        root.path(),
        &world_id,
        0,
        &opening.ts,
        None,
        None,
        &crate::data::test_exclusive(&world_id),
    );

    let report = undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert!(report.removed_opening);
    let left = data::read_transcript(root.path(), &world_id, 0).unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].ts, mine.ts);
}

/// 玩家自己先把開場白收回去了：undo 找不到那則，回報沒收掉，不誤刪別的事件。
#[test]
fn undo_import_reports_no_opening_when_player_already_took_it_back() {
    let root = TestRoot::new("posted-opening-gone");
    let world_id = data::create_world(root.path(), "世界").unwrap();
    let book = serde_json::json!({
        "entries": {
            "0": {"uid": 0, "key": ["城門"], "comment": "城門", "content": "城門已關。", "constant": false, "order": 1, "disable": false}
        }
    });
    import_worldbook_recorded(root.path(), &world_id, "卡.png", &book.to_string());

    let opening = transcript_event("2026-08-07T10:00:00.000Z", "開場白");
    data::append_transcript(root.path(), &world_id, 0, &opening).unwrap();
    record_posted_opening(
        root.path(),
        &world_id,
        0,
        &opening.ts,
        None,
        None,
        &crate::data::test_exclusive(&world_id),
    );
    assert!(data::pop_transcript(root.path(), &world_id, 0).unwrap());

    let report = undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert!(!report.removed_opening);
}

/// 什麼都沒新增的匯入沒留收據，這時貼開場白不該掛到更早那筆收據上，
/// 否則復原上一筆匯入會連帶刪掉不相干的開場白。
#[test]
fn record_posted_opening_without_any_receipt_is_a_no_op() {
    let root = TestRoot::new("posted-opening-no-receipt");
    let world_id = data::create_world(root.path(), "世界").unwrap();

    record_posted_opening(
        root.path(),
        &world_id,
        0,
        "2026-08-07T10:00:00.000Z",
        None,
        None,
        &crate::data::test_exclusive(&world_id),
    );

    assert!(list_import_receipts(root.path(), &world_id).is_empty());
}

/// 全部重複、機制／介面殼都沒變化的世界書匯入不留收據——不該亮出可以「復原」的按鈕。
#[test]
fn worldbook_import_with_nothing_new_leaves_no_receipt() {
    let root = TestRoot::new("worldbook-noop");
    let world_id = data::create_world(root.path(), "世界").unwrap();
    let book = serde_json::json!({
        "entries": {
            "0": {"uid": 0, "key": ["城門"], "comment": "城門", "content": "城門已關。", "constant": false, "order": 1, "disable": false}
        }
    });
    import_worldbook_recorded(root.path(), &world_id, "first.json", &book.to_string());
    assert_eq!(list_import_receipts(root.path(), &world_id).len(), 1);

    // 同一份書再匯一次：內容全部重複，不該多一筆收據
    import_worldbook_recorded(root.path(), &world_id, "second.json", &book.to_string());
    assert_eq!(list_import_receipts(root.path(), &world_id).len(), 1);
}

/// 兩筆收據連按兩次 undo：逐筆倒退，順序正確（後進先出）。
#[test]
fn two_receipts_undo_in_reverse_order() {
    let root = TestRoot::new("sequential");
    let world_id = data::create_world(root.path(), "世界").unwrap();
    let first = import_character_recorded(
        root.path(),
        &world_id,
        &character_book_card("甲", serde_json::json!([])),
    );
    let second = import_character_recorded(
        root.path(),
        &world_id,
        &character_book_card("乙", serde_json::json!([])),
    );
    assert_eq!(
        data::list_characters(root.path(), &world_id).unwrap().len(),
        2
    );

    let report1 = undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert_eq!(report1.removed_character, Some("乙".to_owned()));
    assert!(data::read_character(root.path(), &world_id, &first.id).is_ok());
    assert!(data::read_character(root.path(), &world_id, &second.id).is_err());

    let report2 = undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert_eq!(report2.removed_character, Some("甲".to_owned()));
    assert!(data::read_character(root.path(), &world_id, &first.id).is_err());
    assert!(list_import_receipts(root.path(), &world_id).is_empty());

    // 收據已經清空，再按一次要回錯而不是 panic
    assert!(undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id)
    )
    .is_err());
}

/// 收據檔內容損毀：undo 回傳「無可復原」錯誤，不 panic；缺檔（從沒匯入過）同樣回錯。
#[test]
fn undo_reports_error_without_panicking_on_missing_or_corrupt_receipts() {
    let root = TestRoot::new("corrupt");
    let world_id = data::create_world(root.path(), "世界").unwrap();
    assert!(undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id)
    )
    .is_err());

    fs::write(
        data::import_receipts_path(root.path(), &world_id).unwrap(),
        "{ 不是合法 JSON 陣列",
    )
    .unwrap();
    assert!(undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id)
    )
    .is_err());
}

/// renamed_from 存在時，undo 後桌名要退回去。
#[test]
fn undo_restores_table_name_when_renamed_from_is_recorded() {
    let root = TestRoot::new("renamed");
    let world_id = data::create_world(root.path(), "新的一桌").unwrap();
    import_character_recorded(
        root.path(),
        &world_id,
        &character_book_card("莉亞", serde_json::json!([])),
    );
    record_last_import_rename(
        root.path(),
        &world_id,
        "新的一桌",
        &crate::data::test_exclusive(&world_id),
    );
    data::rename_world(root.path(), &world_id, "莉亞").unwrap();

    let report = undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert!(report.renamed_back);
    assert_eq!(
        data::read_state(root.path(), &world_id).unwrap().name,
        "新的一桌"
    );
}

/// 機制／狀態樹的倒退：undo 只退這一筆匯入自己加的規則與初始值，
/// 較早那筆（尚未復原）帶進來的規則要留著——等同「undo 後等同沒匯入過」只作用在這一筆。
#[test]
fn undo_reverts_only_this_imports_mechanism_writes() {
    let root = TestRoot::new("mechanism");
    let world_id = data::create_world(root.path(), "世界").unwrap();

    let first_raw = character_book_card(
        "甲",
        serde_json::json!([{
            "comment": "[initvar] 初始值",
            "enabled": false,
            "content": "World:\n  Time: 清晨"
        }]),
    );
    import_character_recorded(root.path(), &world_id, &first_raw);
    assert_eq!(
        data::read_state(root.path(), &world_id)
            .unwrap()
            .state
            .tree
            .get("World"),
        Some(&data::StateNode::Branch(BTreeMap::from([(
            "Time".to_owned(),
            data::StateNode::Leaf("清晨".to_owned()),
        )])))
    );

    let second_raw = character_book_card(
        "乙",
        serde_json::json!([{
            "comment": "[mvu_update]规则",
            "enabled": true,
            "content": "变量更新规则:\n  Player:\n    HP:\n      type: number\n      range: 0-100"
        }]),
    );
    import_character_recorded(root.path(), &world_id, &second_raw);
    let mid_state = data::read_state(root.path(), &world_id).unwrap();
    assert!(mid_state.mechanism.incremental);
    assert!(mid_state.mechanism.rules.contains_key("Player.HP"));

    // 復原第二筆（乙）：HP 規則消失，但 incremental 仍是 true——甲的 [initvar] 自己就會
    // 把這桌標成增量桌（import_mechanism：`initial_tree.is_some() || mvu_seen`），甲還沒
    // 復原，這面旗子不該被乙的 undo 連帶關掉；甲帶進來的初始樹同樣要保留。
    let report = undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert!(report.removed_character.is_some());
    let after_first_undo = data::read_state(root.path(), &world_id).unwrap();
    assert!(!after_first_undo.mechanism.rules.contains_key("Player.HP"));
    assert!(after_first_undo.mechanism.incremental);
    assert_eq!(
        after_first_undo.state.tree.get("World"),
        Some(&data::StateNode::Branch(BTreeMap::from([(
            "Time".to_owned(),
            data::StateNode::Leaf("清晨".to_owned()),
        )])))
    );

    // 再復原第一筆（甲）：這下 incremental 才真的退回 false，初始樹也一併消失。
    undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    let after_second_undo = data::read_state(root.path(), &world_id).unwrap();
    assert!(!after_second_undo.mechanism.incremental);
    assert!(!after_second_undo.state.tree.contains_key("World"));
}

/// list_import_receipts 的摘要即時反映 append／undo。
#[test]
fn list_import_receipts_reflects_append_and_undo() {
    let root = TestRoot::new("list");
    let world_id = data::create_world(root.path(), "世界").unwrap();
    assert!(list_import_receipts(root.path(), &world_id).is_empty());

    let meta = import_character_recorded(
        root.path(),
        &world_id,
        &character_book_card("莉亞", serde_json::json!([])),
    );
    let list = list_import_receipts(root.path(), &world_id);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].kind, "character");
    assert_eq!(list[0].label, "莉亞");
    assert_eq!(list[0].character_id.as_deref(), Some(meta.id.as_str()));

    undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert!(list_import_receipts(root.path(), &world_id).is_empty());
}

/// 舊格式收據 JSON（沒有 character_ids／rewritten_entries 這兩個新欄位）照樣能解析，
/// undo 照常運作——新欄位一律 #[serde(default)]，向後相容。
#[test]
fn undo_reads_old_format_receipt_without_new_fields() {
    let root = TestRoot::new("legacy-format");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let meta = import::import_character(
        root.path(),
        &world_id,
        &character_book_card("莉亞", serde_json::json!([])),
        "#3366ff",
        "zh-TW",
    )
    .unwrap();

    // 手寫舊格式收據：只有新欄位加入前就存在的那些鍵。
    let legacy_json = serde_json::json!([{
        "kind": "character",
        "label": "莉亞",
        "timestamp": "2026-01-01 00:00",
        "character_id": meta.id,
    }]);
    fs::write(
        data::import_receipts_path(root.path(), &world_id).unwrap(),
        serde_json::to_string_pretty(&legacy_json).unwrap(),
    )
    .unwrap();

    let report = undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert_eq!(report.removed_character, Some("莉亞".to_owned()));
    assert!(report.removed_characters.is_empty());
    assert!(data::read_character(root.path(), &world_id, &meta.id).is_err());
}
