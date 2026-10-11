//! 刪角色後清世界書名單、收據一致轉換、撤銷時的清理（character-delete-visibility-cleanup）。
use super::super::{
    append_receipt_atomic, rollback_last_import, undo_last_import, write_receipts, ImportReceipt,
    UndoReport,
};
use super::*;
use crate::data::{RemoveFailGuard, RenameFailGuard, Visibility, WorldbookEntry, WriteFailGuard};
use crate::{chat_assembly, import};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const LANG: &str = "zh-TW";
/// 別桌的角色 id：撤銷插回時原樣保留的來源卡。
const OTHER_TABLE_CARD: &str = "01FOREIGNTABLECHARACTER0AA";
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TestRoot(PathBuf);

impl TestRoot {
    fn new(label: &str) -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "table-tavern-character-delete-{label}-{}-{id}",
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

fn new_table(label: &str) -> (TestRoot, String) {
    let root = TestRoot::new(label);
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    (root, world_id)
}

/// 一條常駐條目的角色卡；`table_tavern` 為 None 時不寫擴充欄位。
fn card(name: &str, content: &str, table_tavern: Option<Value>) -> Vec<u8> {
    let mut entry = json!({"comment": "設定", "keys": [], "content": content, "constant": true,
        "enabled": true, "position": "before_char"});
    if let Some(table_tavern) = table_tavern {
        entry["extensions"] = json!({ "table_tavern": table_tavern });
    }
    json!({"data": {"name": name, "character_book": {"entries": [entry]}}})
        .to_string()
        .into_bytes()
}

fn plain_card(name: &str) -> Vec<u8> {
    json!({"data": {"name": name, "description": "路人"}})
        .to_string()
        .into_bytes()
}

/// 角色卡路匯入並記收據，回傳角色 id。
fn import_card(root: &Path, world_id: &str, bytes: &[u8]) -> String {
    import::import_character_file(
        root,
        world_id,
        bytes,
        "#3366ff",
        LANG,
        &data::test_exclusive(world_id),
    )
    .unwrap()
    .value
    .id
}

fn import_book(root: &Path, world_id: &str, content: &str) {
    let book = json!({"entries": {"0": {"uid": 0, "comment": "設定", "key": [], "content": content,
        "constant": true}}});
    import::import_worldbook_file(
        root,
        world_id,
        book.to_string().as_bytes(),
        "書",
        &data::test_exclusive(world_id),
    )
    .unwrap();
}

fn entries_with(root: &Path, world_id: &str, content: &str) -> Vec<(u64, Value)> {
    data::read_worldbook_raw(root, world_id)
        .unwrap()
        .into_iter()
        .filter(|(_, value)| value["content"] == content)
        .collect()
}

fn entry(root: &Path, world_id: &str, content: &str) -> (u64, Value) {
    let mut found = entries_with(root, world_id, content);
    assert_eq!(found.len(), 1, "{content}");
    found.remove(0)
}

fn compact(root: &Path, world_id: &str, uid: u64) -> WorldbookEntry {
    data::read_worldbook(root, world_id)
        .unwrap()
        .into_iter()
        .find(|entry| entry.uid == uid)
        .unwrap()
}

fn table_tavern(value: &Value) -> &Value {
    &value["extensions"]["table_tavern"]
}

fn edit(root: &Path, world_id: &str, uid: u64, change: impl FnOnce(&mut WorldbookEntry)) {
    let mut current = compact(root, world_id, uid);
    change(&mut current);
    data::upsert_worldbook_entry(root, world_id, current).unwrap();
}

fn without_display_index(value: &Value) -> Value {
    let mut value = value.clone();
    value.as_object_mut().unwrap().remove("displayIndex");
    value
}

fn undo(root: &Path, world_id: &str) -> UndoReport {
    undo_last_import(root, world_id, &data::test_exclusive(world_id)).unwrap()
}

fn delete(root: &Path, world_id: &str, id: &str) -> CharacterDeleteOutcome {
    delete_character_and_clean(root, world_id, id).unwrap()
}

fn receipts_path(root: &Path, world_id: &str) -> PathBuf {
    data::import_receipts_path(root, world_id).unwrap()
}

/// 模擬 AI 重構的收據：整條刪掉或改寫既有條目（快照照撤銷要的格式記），新建的角色記在 character_ids。
fn refactor_receipt(fields: Value) -> ImportReceipt {
    let mut receipt = json!({"kind": "refactor", "label": "重構", "timestamp": ""});
    for (key, value) in fields.as_object().unwrap() {
        receipt[key] = value.clone();
    }
    serde_json::from_value(receipt).unwrap()
}

/// 重構整條刪掉 uid 那條；`with_raw` 為 false 時模擬舊收據（沒有原始 JSON）。
fn refactor_delete(root: &Path, world_id: &str, uid: u64, with_raw: bool) {
    let raw = data::read_worldbook_raw(root, world_id).unwrap()[&uid].clone();
    let snapshot = compact(root, world_id, uid);
    data::delete_worldbook_entry(root, world_id, uid).unwrap();
    let raws = if with_raw { vec![raw] } else { Vec::new() };
    assert!(append_receipt_atomic(
        root,
        world_id,
        refactor_receipt(json!({"deleted_entries": [snapshot], "deleted_entries_raw": raws})),
    ));
}

/// 重構改寫 uid 那條的內文（可見度不動）。
fn refactor_rewrite(root: &Path, world_id: &str, uid: u64, content: &str) {
    let snapshot = compact(root, world_id, uid);
    edit(root, world_id, uid, |entry| {
        entry.content = content.to_owned()
    });
    assert!(append_receipt_atomic(
        root,
        world_id,
        refactor_receipt(json!({ "rewritten_entries": [snapshot] })),
    ));
}

/// 玩家刪角色：只給這個角色看的條目改給 GM，GM 掃描不再當機密條目。
#[test]
fn deleting_a_character_hands_its_entries_to_the_gm() {
    let (root, world_id) = new_table("gm-scan");
    let a = import_card(root.path(), &world_id, &card("甲", "祕密", None));
    let (uid, value) = entry(root.path(), &world_id, "祕密");
    assert_eq!(
        table_tavern(&value)["visibility"],
        json!({ "characters": [a] })
    );

    let outcome = delete(root.path(), &world_id, &a);
    assert!(!outcome.worldbook_cleanup_failed);
    let (_, value) = entry(root.path(), &world_id, "祕密");
    assert_eq!(table_tavern(&value)["visibility"], json!("gm"));
    assert!(table_tavern(&value).get("source_cards").is_none());
    let materials = chat_assembly::gm_materials(root.path(), &world_id).unwrap();
    let scan = chat_assembly::test_gm_scan(root.path(), &world_id, &materials, LANG);
    let placed = scan.placed.iter().find(|entry| entry.uid == uid).unwrap();
    assert!(!placed.confidential());
}

/// 卡檔已刪、圖庫刪除失敗：回原錯但名單照清；卡檔刪不掉：名單不動。
#[test]
fn cleanup_follows_whether_the_card_file_is_gone() {
    for (label, gone) in [("gallery", true), ("md", false)] {
        let (root, world_id) = new_table(&format!("judge-{label}"));
        let a = import_card(root.path(), &world_id, &card("甲", "祕密", None));
        let gallery = root
            .path()
            .join(format!("worlds/{world_id}/gen-gallery/{a}"));
        fs::create_dir_all(&gallery).unwrap();
        let suffix = match gone {
            true => a.clone(),
            false => format!("{a}.md"),
        };
        let result = {
            let _guard = RemoveFailGuard::fail_ending(&suffix, 1);
            delete_character_and_clean(root.path(), &world_id, &a)
        };
        assert!(result.is_err(), "{label}");
        let (_, value) = entry(root.path(), &world_id, "祕密");
        let expected = match gone {
            true => json!("gm"),
            false => json!({ "characters": [a] }),
        };
        assert_eq!(table_tavern(&value)["visibility"], expected, "{label}");
    }
}

/// 轉條目路：刪卡那步圖庫刪除失敗（卡檔已刪）照樣清。
#[test]
fn conversion_cleans_even_when_the_card_delete_half_fails() {
    let (root, world_id) = new_table("convert-half");
    let a = import_card(root.path(), &world_id, &card("甲", "祕密", None));
    fs::create_dir_all(
        root.path()
            .join(format!("worlds/{world_id}/gen-gallery/{a}")),
    )
    .unwrap();
    let result = {
        let _guard = RemoveFailGuard::fail_ending(&a, 1);
        character_to_worldbook_entry_and_clean(root.path(), &world_id, &a, LANG)
    };
    assert!(result.is_err());
    let (_, value) = entry(root.path(), &world_id, "祕密");
    assert_eq!(table_tavern(&value)["visibility"], json!("gm"));
}

/// 撤銷（非嚴格）：刪卡那步圖庫刪除失敗，卡檔已刪的 id 照樣從名單拿掉。
#[test]
fn undo_cleans_when_the_card_delete_half_fails() {
    let (root, world_id) = new_table("undo-half");
    import_book(root.path(), &world_id, "既有");
    let a = import_card(root.path(), &world_id, &plain_card("甲"));
    let (uid, _) = entry(root.path(), &world_id, "既有");
    edit(root.path(), &world_id, uid, |entry| {
        entry.visibility = Visibility::Characters(vec![a.clone()]);
    });
    fs::create_dir_all(
        root.path()
            .join(format!("worlds/{world_id}/gen-gallery/{a}")),
    )
    .unwrap();
    {
        let _guard = RemoveFailGuard::fail_ending(&a, 1);
        undo(root.path(), &world_id);
    }
    assert_eq!(
        compact(root.path(), &world_id, uid).visibility,
        Visibility::Gm
    );
}

/// 卡檔存在與否查不到（目錄沒有搜尋權限）：不當成已刪、名單不動；刪卡本身成功時回旗標，不回錯。
#[cfg(unix)]
#[test]
fn unreadable_card_directory_is_not_treated_as_deleted() {
    use std::os::unix::fs::PermissionsExt;

    let (root, world_id) = new_table("try-exists");
    let a = import_card(root.path(), &world_id, &card("甲", "祕密", None));
    let book = root
        .path()
        .join(format!("worlds/{world_id}/worldbook.json"));
    let before = fs::read(&book).unwrap();
    let characters = root.path().join(format!("worlds/{world_id}/characters"));
    fs::set_permissions(&characters, fs::Permissions::from_mode(0o000)).unwrap();
    let checked = data::character_card_gone(root.path(), &world_id, &a);
    // 刪卡在沒有搜尋權限的目錄裡看不到檔案、什麼都沒刪而回成功；之後的存在檢查回錯
    let outcome = delete_character_and_clean(root.path(), &world_id, &a);
    fs::set_permissions(&characters, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(checked.is_err());
    assert!(outcome.unwrap().worldbook_cleanup_failed);
    assert_eq!(fs::read(&book).unwrap(), before);
}

/// 清理失敗：世界書寫不進去＝兩檔都不動；收據寫不進去＝世界書已清、收據不動。都回旗標。
#[test]
fn cleanup_failures_raise_the_flag() {
    for (label, target) in [
        ("book", "worldbook.json.tmp"),
        ("receipts", "import-receipts.json.tmp"),
    ] {
        let (root, world_id) = new_table(&format!("cleanup-fail-{label}"));
        let a = import_card(root.path(), &world_id, &card("甲", "祕密", None));
        let book = root
            .path()
            .join(format!("worlds/{world_id}/worldbook.json"));
        let book_before = fs::read(&book).unwrap();
        let receipts_before = fs::read(receipts_path(root.path(), &world_id)).unwrap();
        let outcome = {
            let _guard = WriteFailGuard::partial_ending(target, 1);
            delete(root.path(), &world_id, &a)
        };
        assert!(outcome.worldbook_cleanup_failed, "{label}");
        assert!(data::read_character(root.path(), &world_id, &a).is_err());
        assert_eq!(
            fs::read(receipts_path(root.path(), &world_id)).unwrap(),
            receipts_before,
            "{label}"
        );
        assert_eq!(fs::read(&book).unwrap() == book_before, label == "book");
    }
}

/// 角色卡轉條目：被轉的角色從其他條目拿掉，新條目照舊是 GM。
#[test]
fn converting_a_card_cleans_other_entries() {
    let (root, world_id) = new_table("convert");
    let a = import_card(root.path(), &world_id, &card("甲", "祕密", None));
    let outcome = character_to_worldbook_entry_and_clean(root.path(), &world_id, &a, LANG).unwrap();
    assert!(!outcome.worldbook_cleanup_failed);
    let entries = data::read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries
        .iter()
        .all(|entry| entry.visibility == Visibility::Gm));
}

/// 刪後撤銷：A 建的條目照常刪掉；刪 A 前玩家改過內文的那條保留、讀成 GM。
#[test]
fn undo_after_delete_removes_unmodified_entries() {
    for (label, player_edits) in [("plain", false), ("edited", true)] {
        let (root, world_id) = new_table(&format!("delete-undo-{label}"));
        let a = import_card(root.path(), &world_id, &card("甲", "祕密", None));
        let (uid, _) = entry(root.path(), &world_id, "祕密");
        if player_edits {
            edit(root.path(), &world_id, uid, |entry| {
                entry.content = "玩家改過的內文".to_owned()
            });
        }
        delete(root.path(), &world_id, &a);
        let report = undo(root.path(), &world_id);
        let left = data::read_worldbook(root.path(), &world_id).unwrap();
        match player_edits {
            false => {
                assert_eq!((report.removed_entries, report.kept_entries), (1, 0));
                assert!(left.is_empty());
            }
            true => {
                assert_eq!((report.removed_entries, report.kept_entries), (0, 1));
                assert_eq!(left[0].visibility, Visibility::Gm);
            }
        }
    }
}

/// 撤銷本身：玩家把既有條目加給這次匯入的角色，撤銷後那個 id 拿掉；本次新建的條目照舊刪掉。
#[test]
fn undo_cleans_the_characters_it_deletes() {
    let (root, world_id) = new_table("undo-clean");
    import_book(root.path(), &world_id, "既有");
    let (uid, _) = entry(root.path(), &world_id, "既有");
    let a = import_card(root.path(), &world_id, &card("甲", "祕密", None));
    edit(root.path(), &world_id, uid, |entry| {
        entry.visibility = Visibility::Characters(vec![a.clone()]);
    });
    let report = undo(root.path(), &world_id);
    assert_eq!(report.removed_entries, 1);
    assert!(entries_with(root.path(), &world_id, "祕密").is_empty());
    assert_eq!(
        compact(root.path(), &world_id, uid).visibility,
        Visibility::Gm
    );
}

/// 公開升級：B 的卡明寫 public 把 A 的條目升成公開；刪 B 後撤銷 R2，回到只給 A。
#[test]
fn restore_after_is_converted_when_reachable() {
    let (root, world_id) = new_table("sol");
    let a = import_card(root.path(), &world_id, &card("甲", "共", None));
    let b = import_card(
        root.path(),
        &world_id,
        &card("乙", "共", Some(json!({"visibility": "public"}))),
    );
    let (_, value) = entry(root.path(), &world_id, "共");
    assert_eq!(table_tavern(&value)["visibility"], json!("public"));
    delete(root.path(), &world_id, &b);
    undo(root.path(), &world_id);
    let (_, value) = entry(root.path(), &world_id, "共");
    assert_eq!(
        table_tavern(&value)["visibility"],
        json!({ "characters": [a] })
    );
    assert_eq!(table_tavern(&value)["source_cards"], json!([a]));
    assert_eq!(undo(root.path(), &world_id).removed_entries, 1);
    assert!(entries_with(root.path(), &world_id, "共").is_empty());
}

/// 鏈式：R1 建 [A]、R2 合併 B；刪 A 後撤 R2 回 gm、撤 R1 刪掉，不留孤兒。
/// 玩家先把條目改成 GM（不可抵達）：兩次撤銷都保留。
#[test]
fn chained_merge_reaches_the_older_fingerprint() {
    for (label, player_gm) in [("chain", false), ("player", true)] {
        let (root, world_id) = new_table(&format!("chain-{label}"));
        let a = import_card(root.path(), &world_id, &card("甲", "共", None));
        import_card(root.path(), &world_id, &card("乙", "共", None));
        let (uid, _) = entry(root.path(), &world_id, "共");
        if player_gm {
            edit(root.path(), &world_id, uid, |entry| {
                entry.visibility = Visibility::Gm
            });
        }
        delete(root.path(), &world_id, &a);
        undo(root.path(), &world_id);
        assert_eq!(
            compact(root.path(), &world_id, uid).visibility,
            Visibility::Gm
        );
        let report = undo(root.path(), &world_id);
        let left = entries_with(root.path(), &world_id, "共");
        match player_gm {
            false => assert!(left.is_empty() && report.removed_entries == 1, "{label}"),
            true => assert_eq!(left.len(), 1, "{label}"),
        }
    }
}

/// 撤銷這筆時自己的角色清理也算進模擬：玩家把 B 加進 A 的條目、刪 A，撤 R2、撤 R1 不留孤兒。
#[test]
fn own_character_cleanup_is_simulated() {
    let (root, world_id) = new_table("own-clean");
    let a = import_card(root.path(), &world_id, &card("甲", "共", None));
    let b = import_card(root.path(), &world_id, &plain_card("乙"));
    let (uid, _) = entry(root.path(), &world_id, "共");
    edit(root.path(), &world_id, uid, |entry| {
        entry.visibility = Visibility::Characters(vec![a.clone(), b.clone()]);
    });
    delete(root.path(), &world_id, &a);
    undo(root.path(), &world_id);
    assert_eq!(
        compact(root.path(), &world_id, uid).visibility,
        Visibility::Gm
    );
    undo(root.path(), &world_id);
    assert!(entries_with(root.path(), &world_id, "共").is_empty());
}

/// 重構刪除／改寫（含沒有原始 JSON 的舊收據）：刪 A 後撤重構插回或覆寫回 gm，再撤匯入不留孤兒。
#[test]
fn refactor_snapshots_are_scrubbed_and_reachable() {
    for label in ["delete", "delete-legacy", "rewrite"] {
        let (root, world_id) = new_table(&format!("refactor-{label}"));
        let a = import_card(root.path(), &world_id, &card("甲", "共", None));
        let (uid, _) = entry(root.path(), &world_id, "共");
        match label {
            "rewrite" => refactor_rewrite(root.path(), &world_id, uid, "改寫後"),
            _ => refactor_delete(root.path(), &world_id, uid, label == "delete"),
        }
        delete(root.path(), &world_id, &a);
        undo(root.path(), &world_id);
        let restored = compact(root.path(), &world_id, uid);
        assert_eq!(restored.content, "共", "{label}");
        assert_eq!(restored.visibility, Visibility::Gm, "{label}");
        let report = undo(root.path(), &world_id);
        assert_eq!(report.removed_entries, 1, "{label}");
        assert!(data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .is_empty());
    }
}

/// 世界書沒有含 A 的條目、只有重構快照帶 A：刪 A 後快照照樣清、收據有寫。
#[test]
fn snapshots_are_scrubbed_without_book_hits() {
    let (root, world_id) = new_table("snapshot-only");
    let a = import_card(root.path(), &world_id, &card("甲", "共", None));
    let (uid, _) = entry(root.path(), &world_id, "共");
    refactor_delete(root.path(), &world_id, uid, true);
    delete(root.path(), &world_id, &a);
    let text = fs::read_to_string(receipts_path(root.path(), &world_id)).unwrap();
    let receipts: Vec<ImportReceipt> = serde_json::from_str(&text).unwrap();
    let refactor = receipts.last().unwrap();
    assert_eq!(refactor.deleted_entries[0].visibility, Visibility::Gm);
    assert_eq!(
        refactor.deleted_entries_raw[0]["extensions"]["table_tavern"]["visibility"],
        json!("gm")
    );
}

/// 原 uid 被佔：E′ 精簡欄位與 E 相同但原始欄位不同 → E 插到新 uid、E′ 保留、撤 R1 不刪 E′；
/// E′ 與清理後快照全同 → 撤 R2 判已插回略過、撤 R1 不刪 E′。
#[test]
fn occupied_uid_is_not_reached_by_the_snapshot() {
    for (label, identical) in [("differs", false), ("identical", true)] {
        let (root, world_id) = new_table(&format!("collision-{label}"));
        let a = import_card(root.path(), &world_id, &card("甲", "共", None));
        let (uid, raw) = entry(root.path(), &world_id, "共");
        refactor_delete(root.path(), &world_id, uid, true);
        let mut occupant = raw.clone();
        data::scrub_entry_value(&mut occupant, std::slice::from_ref(&a));
        if !identical {
            occupant["selectiveLogic"] = json!(3);
        }
        let book = root
            .path()
            .join(format!("worlds/{world_id}/worldbook.json"));
        let mut value: Value = serde_json::from_str(&fs::read_to_string(&book).unwrap()).unwrap();
        value["entries"][uid.to_string()] = occupant.clone();
        fs::write(&book, value.to_string()).unwrap();

        delete(root.path(), &world_id, &a);
        undo(root.path(), &world_id);
        let same_content = entries_with(root.path(), &world_id, "共");
        assert_eq!(same_content.len(), if identical { 1 } else { 2 }, "{label}");
        assert!(same_content.iter().any(|(at, _)| *at == uid), "{label}");
        undo(root.path(), &world_id);
        let left = data::read_worldbook_raw(root.path(), &world_id).unwrap();
        assert_eq!(
            left.get(&uid).map(without_display_index),
            Some(without_display_index(&occupant)),
            "{label}"
        );
    }
}

/// 三筆交錯：R1 匯入 A → R2 重構改寫或刪除 → R3 匯入合併 → 刪 A → 逐筆撤到底不留孤兒。
#[test]
fn three_receipts_unwind_cleanly() {
    for label in ["rewrite", "delete"] {
        let (root, world_id) = new_table(&format!("three-{label}"));
        let a = import_card(root.path(), &world_id, &card("甲", "共", None));
        let (uid, _) = entry(root.path(), &world_id, "共");
        let merged_content = match label {
            "rewrite" => {
                refactor_rewrite(root.path(), &world_id, uid, "改寫後");
                "改寫後"
            }
            _ => {
                refactor_delete(root.path(), &world_id, uid, true);
                "共"
            }
        };
        import_card(root.path(), &world_id, &card("丙", merged_content, None));
        delete(root.path(), &world_id, &a);
        for _ in 0..3 {
            undo(root.path(), &world_id);
        }
        assert!(
            data::read_worldbook(root.path(), &world_id)
                .unwrap()
                .is_empty(),
            "{label}"
        );
    }
}

/// 三筆交錯、刪 A 前玩家改過內文：刪除分支那條（R3 建的）一路保留；改寫分支撤 R2 會無條件覆寫回
/// 改寫快照（既有行為），之後照原收據撤銷，撤 R1 時指紋相符就刪掉。
#[test]
fn three_receipts_with_player_edit() {
    for label in ["rewrite", "delete"] {
        let (root, world_id) = new_table(&format!("three-edit-{label}"));
        let a = import_card(root.path(), &world_id, &card("甲", "共", None));
        let (uid, _) = entry(root.path(), &world_id, "共");
        match label {
            "rewrite" => refactor_rewrite(root.path(), &world_id, uid, "改寫後"),
            _ => refactor_delete(root.path(), &world_id, uid, true),
        }
        let merged_content = if label == "rewrite" {
            "改寫後"
        } else {
            "共"
        };
        import_card(root.path(), &world_id, &card("丙", merged_content, None));
        let (target, _) = entry(root.path(), &world_id, merged_content);
        edit(root.path(), &world_id, target, |entry| {
            entry.content = "玩家改過的內文".to_owned();
        });
        delete(root.path(), &world_id, &a);
        for _ in 0..3 {
            undo(root.path(), &world_id);
        }
        let left = data::read_worldbook(root.path(), &world_id).unwrap();
        match label {
            "rewrite" => assert!(left.is_empty(), "{label}"),
            _ => {
                assert!(
                    left.iter().any(|entry| entry.content == "玩家改過的內文"),
                    "{label}"
                );
                assert!(left.iter().all(|entry| entry.visibility == Visibility::Gm));
            }
        }
    }
}

/// 撤銷重試：嚴格模式清理成功、後段（彈出收據）失敗；再撤兩次，插回的條目只有一條、原始欄位不掉。
#[test]
fn rollback_retry_does_not_duplicate_restored_entries() {
    let (root, world_id) = new_table("retry");
    import_book(root.path(), &world_id, "共");
    let (uid, _) = entry(root.path(), &world_id, "共");
    edit(root.path(), &world_id, uid, |entry| {
        entry.keys = vec!["鑰".to_owned()]
    });
    let book = root
        .path()
        .join(format!("worlds/{world_id}/worldbook.json"));
    let mut value: Value = serde_json::from_str(&fs::read_to_string(&book).unwrap()).unwrap();
    value["entries"][uid.to_string()]["keysecondary"] = json!(["次"]);
    value["entries"][uid.to_string()]["position"] = json!(4);
    value["entries"][uid.to_string()]["extensions"]["table_tavern"]["source_cards"] =
        json!([OTHER_TABLE_CARD]);
    fs::write(&book, value.to_string()).unwrap();
    let original = data::read_worldbook_raw(root.path(), &world_id).unwrap()[&uid].clone();
    let raw = original.clone();
    let snapshot = compact(root.path(), &world_id, uid);
    data::delete_worldbook_entry(root.path(), &world_id, uid).unwrap();
    let z = import_card(root.path(), &world_id, &card("新", "新卡", None));
    let receipts: Vec<ImportReceipt> =
        serde_json::from_str(&fs::read_to_string(receipts_path(root.path(), &world_id)).unwrap())
            .unwrap();
    let mut merged = receipts.last().unwrap().clone();
    merged.kind = "refactor".to_owned();
    merged.character_ids = vec![merged.character_id.take().unwrap()];
    merged.deleted_entries = vec![snapshot];
    merged.deleted_entries_raw = vec![raw];
    let mut all = receipts;
    *all.last_mut().unwrap() = merged;
    write_receipts(root.path(), &world_id, &all).unwrap();
    let held = data::test_exclusive(&world_id);

    for _ in 0..2 {
        let _guard = WriteFailGuard::partial_ending("import-receipts.json.tmp", 1);
        assert!(rollback_last_import(root.path(), &world_id, &held).is_err());
    }
    rollback_last_import(root.path(), &world_id, &held).unwrap();
    assert!(data::read_character(root.path(), &world_id, &z).is_err());
    let restored = entries_with(root.path(), &world_id, "共");
    assert_eq!(restored.len(), 1);
    assert_eq!(
        without_display_index(&restored[0].1),
        without_display_index(&original)
    );
    assert_eq!(
        restored[0].1["extensions"]["table_tavern"]["source_cards"],
        json!([OTHER_TABLE_CARD])
    );
}

/// 兩條執行緒各刪一張卡：世界書與收據的指紋更新都在，逐筆撤銷都刪得掉。
#[test]
fn concurrent_deletes_keep_both_fingerprint_updates() {
    let (root, world_id) = new_table("concurrent");
    let a = import_card(root.path(), &world_id, &card("甲", "甲的", None));
    let b = import_card(root.path(), &world_id, &card("乙", "乙的", None));
    std::thread::scope(|scope| {
        for id in [&a, &b] {
            let (root, world_id) = (root.path(), world_id.as_str());
            scope.spawn(move || delete(root, world_id, id));
        }
    });
    assert_eq!(undo(root.path(), &world_id).removed_entries, 1);
    assert_eq!(undo(root.path(), &world_id).removed_entries, 1);
    assert!(data::read_worldbook(root.path(), &world_id)
        .unwrap()
        .is_empty());
}

/// 收據一律原子寫：暫存檔寫壞或改名失敗，收據檔逐位元組不變。
#[test]
fn receipt_writes_are_atomic() {
    for label in ["partial", "rename"] {
        let (root, world_id) = new_table(&format!("receipts-atomic-{label}"));
        import_card(root.path(), &world_id, &plain_card("甲"));
        let path = receipts_path(root.path(), &world_id);
        let before = fs::read(&path).unwrap();
        let receipts: Vec<ImportReceipt> = serde_json::from_slice(&before).unwrap();
        let doubled = [receipts.clone(), receipts].concat();
        let result = if label == "partial" {
            let _guard = WriteFailGuard::partial_ending("import-receipts.json.tmp", 1);
            write_receipts(root.path(), &world_id, &doubled)
        } else {
            let _guard = RenameFailGuard::fail_ending("import-receipts.json", 1);
            write_receipts(root.path(), &world_id, &doubled)
        };
        assert!(result.is_err(), "{label}");
        assert_eq!(fs::read(&path).unwrap(), before, "{label}");
    }
}

/// 落定失效（接受）：落定的 GM 只剩被刪的卡時移除來源卡，之後被同內容的卡收成名單；
/// 沒有來源卡的 GM 遇到明寫 gm 的同內容卡仍是 GM 並重新落定。
#[test]
fn settled_gm_loses_its_settlement_with_the_card() {
    let (root, world_id) = new_table("settled");
    let a = import_card(
        root.path(),
        &world_id,
        &card("甲", "共", Some(json!({"visibility": "gm"}))),
    );
    delete(root.path(), &world_id, &a);
    let (_, value) = entry(root.path(), &world_id, "共");
    assert_eq!(table_tavern(&value), &json!({"visibility": "gm"}));
    let c = import_card(root.path(), &world_id, &card("丙", "共", None));
    let (_, value) = entry(root.path(), &world_id, "共");
    assert_eq!(
        table_tavern(&value)["visibility"],
        json!({ "characters": [c] })
    );

    let (root, world_id) = new_table("resettle");
    import_book(root.path(), &world_id, "共");
    let d = import_card(
        root.path(),
        &world_id,
        &card("丁", "共", Some(json!({"visibility": "gm"}))),
    );
    let (_, value) = entry(root.path(), &world_id, "共");
    assert_eq!(table_tavern(&value)["visibility"], json!("gm"));
    assert_eq!(table_tavern(&value)["source_cards"], json!([d]));
    import_card(root.path(), &world_id, &card("戊", "共", None));
    let (_, value) = entry(root.path(), &world_id, "共");
    assert_eq!(table_tavern(&value)["visibility"], json!("gm"));
}

/// 收據沒有可轉換的內容就不寫收據檔：注入收據寫入失敗也不回旗標，收據逐位元組不變、世界書照清。
#[test]
fn unchanged_receipts_are_not_rewritten() {
    let (root, world_id) = new_table("receipts-untouched");
    import_book(root.path(), &world_id, "既有");
    let a = import_card(root.path(), &world_id, &plain_card("甲"));
    let (uid, _) = entry(root.path(), &world_id, "既有");
    edit(root.path(), &world_id, uid, |entry| {
        entry.visibility = Visibility::Characters(vec![a.clone()]);
    });
    let before = fs::read(receipts_path(root.path(), &world_id)).unwrap();
    let outcome = {
        let _guard = WriteFailGuard::partial_ending("import-receipts.json.tmp", 1);
        delete(root.path(), &world_id, &a)
    };
    assert!(!outcome.worldbook_cleanup_failed);
    assert_eq!(
        fs::read(receipts_path(root.path(), &world_id)).unwrap(),
        before
    );
    assert_eq!(
        compact(root.path(), &world_id, uid).visibility,
        Visibility::Gm
    );
}

/// 清理階段全程持獨占：轉條目寫世界書（含清理）的當下，別的執行緒拿不到獨占也拿不到共用許可。
#[test]
fn conversion_holds_the_exclusive_through_cleanup() {
    use crate::data::write_hook;
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::Duration;

    let _serial = write_hook::TESTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (root, world_id) = new_table("convert-exclusive");
    let a = import_card(root.path(), &world_id, &card("甲", "祕密", None));
    let book = root
        .path()
        .join(format!("worlds/{world_id}/worldbook.json"));
    let seen: Arc<Mutex<Vec<(bool, bool)>>> = Arc::default();
    let _hook = {
        let (seen, world_id) = (seen.clone(), world_id.clone());
        write_hook::install(book, move || {
            let world = world_id.clone();
            let exclusive_free =
                std::thread::spawn(move || data::try_world_exclusive(&world).is_some())
                    .join()
                    .unwrap();
            let (sender, receiver) = mpsc::channel();
            let world = world_id.clone();
            std::thread::spawn(move || {
                let permit = data::world_write_permit(&world);
                let _ = sender.send(permit.is_ok());
            });
            let permit_granted = receiver.recv_timeout(Duration::from_millis(150)).is_ok();
            seen.lock().unwrap().push((exclusive_free, permit_granted));
        })
    };
    let outcome = character_to_worldbook_entry_and_clean(root.path(), &world_id, &a, LANG).unwrap();
    drop(_hook);
    assert!(!outcome.worldbook_cleanup_failed);
    let seen = seen.lock().unwrap();
    // 轉換新增條目一次、清理改名單一次
    assert!(seen.len() >= 2, "{seen:?}");
    assert!(
        seen.iter()
            .all(|&(exclusive, permit)| !exclusive && !permit),
        "{seen:?}"
    );
    let (_, value) = entry(root.path(), &world_id, "祕密");
    assert_eq!(table_tavern(&value)["visibility"], json!("gm"));
}
