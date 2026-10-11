//! 撤銷報告的已刪角色 id 與條目改動（undo-import-editor-guard）。
use super::super::{
    append_receipt_atomic, undo_last_import, worldbook_entry_fingerprint, ImportReceipt, UndoReport,
};
use super::{EntryChange, EntryChanges};
use crate::data::{self, Visibility, WorldbookEntry};
use crate::import;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TestRoot(PathBuf);

impl TestRoot {
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
    let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "table-tavern-undo-changes-{label}-{}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(&path).unwrap();
    let world_id = data::create_world(&path, "酒館").unwrap();
    (TestRoot(path), world_id)
}

fn card(name: &str, content: &str) -> Vec<u8> {
    json!({"data": {"name": name, "character_book": {"entries": [
        {"comment": "設定", "keys": [], "content": content, "constant": true, "enabled": true}
    ]}}})
    .to_string()
    .into_bytes()
}

fn import_card(root: &Path, world_id: &str, bytes: &[u8]) -> String {
    import::import_character_file(
        root,
        world_id,
        bytes,
        "#3366ff",
        "zh-TW",
        &data::test_exclusive(world_id),
    )
    .unwrap()
    .value
    .id
}

fn gm_entry(content: &str) -> WorldbookEntry {
    WorldbookEntry {
        uid: u64::MAX,
        title: "條目".to_owned(),
        keys: Vec::new(),
        content: content.to_owned(),
        constant: true,
        order: 100,
        disabled: false,
        visibility: Visibility::Gm,
        is_person: false,
        locked: false,
    }
}

fn uid_of(root: &Path, world_id: &str, content: &str) -> u64 {
    data::read_worldbook(root, world_id)
        .unwrap()
        .into_iter()
        .find(|entry| entry.content == content)
        .unwrap()
        .uid
}

fn compact(root: &Path, world_id: &str, uid: u64) -> WorldbookEntry {
    data::read_worldbook(root, world_id)
        .unwrap()
        .into_iter()
        .find(|entry| entry.uid == uid)
        .unwrap()
}

fn refactor_receipt(fields: Value) -> ImportReceipt {
    let mut receipt = json!({"kind": "refactor", "label": "重構", "timestamp": ""});
    for (key, value) in fields.as_object().unwrap() {
        receipt[key] = value.clone();
    }
    serde_json::from_value(receipt).unwrap()
}

fn undo(root: &Path, world_id: &str) -> UndoReport {
    undo_last_import(root, world_id, &data::test_exclusive(world_id)).unwrap()
}

fn changes(report: &UndoReport) -> &EntryChanges {
    report.entry_changes.as_ref().unwrap()
}

/// 單張匯入：卡 id 報出來、帶入的條目報刪除；既有條目沒動不報。
#[test]
fn character_import_reports_its_card_and_entry() {
    let (root, world_id) = new_table("single");
    data::upsert_worldbook_entry(root.path(), &world_id, gm_entry("既有")).unwrap();
    let id = import_card(root.path(), &world_id, &card("甲", "甲的設定"));
    let uid = uid_of(root.path(), &world_id, "甲的設定");

    let report = undo(root.path(), &world_id);
    assert_eq!(report.removed_character_ids, vec![id]);
    assert_eq!(
        report.entry_changes,
        Some(EntryChanges {
            changed: Vec::new(),
            deleted: vec![uid],
        })
    );
}

/// 多張（重構的 character_ids）全部報出來；卡檔已先不在的也算。
#[test]
fn multi_card_receipts_report_every_gone_card() {
    let (root, world_id) = new_table("multi");
    let a = import_card(root.path(), &world_id, &card("甲", "甲"));
    let b = import_card(root.path(), &world_id, &card("乙", "乙"));
    data::delete_character(root.path(), &world_id, &b).unwrap();
    assert!(append_receipt_atomic(
        root.path(),
        &world_id,
        refactor_receipt(json!({ "character_ids": [a, b] })),
    ));
    let report = undo(root.path(), &world_id);
    assert_eq!(report.removed_character_ids, vec![a, b]);
}

/// 整條覆寫只報實際不同的欄組。
#[test]
fn rewrite_reports_only_the_groups_that_differ() {
    let (root, world_id) = new_table("rewrite");
    let uid = data::upsert_worldbook_entry(root.path(), &world_id, gm_entry("原文")).unwrap();
    let snapshot = compact(root.path(), &world_id, uid);
    let mut rewritten = snapshot.clone();
    rewritten.content = "改寫".to_owned();
    rewritten.order = 5;
    rewritten.visibility = Visibility::Public;
    data::upsert_worldbook_entry(root.path(), &world_id, rewritten).unwrap();
    assert!(append_receipt_atomic(
        root.path(),
        &world_id,
        refactor_receipt(json!({ "rewritten_entries": [snapshot] })),
    ));
    let report = undo(root.path(), &world_id);
    assert_eq!(
        changes(&report).changed,
        vec![EntryChange {
            uid,
            groups: vec!["content", "order", "visibility"],
        }]
    );
    assert!(changes(&report).deleted.is_empty());
}

/// 撤銷刪角色時 3b 純剪名單不算改欄：交給前端照刪角色修剪。
#[test]
fn scrubbing_the_undone_card_is_not_a_change() {
    let (root, world_id) = new_table("scrub");
    let other = import_card(root.path(), &world_id, &card("乙", "乙"));
    let a = import_card(root.path(), &world_id, &card("甲", "甲的設定"));
    let uid = uid_of(root.path(), &world_id, "甲的設定");
    // 玩家改過內文（撤銷保留這條）並把乙加進名單
    let mut kept = compact(root.path(), &world_id, uid);
    kept.content = "玩家改過".to_owned();
    kept.visibility = Visibility::Characters(vec![a.clone(), other.clone()]);
    data::upsert_worldbook_entry(root.path(), &world_id, kept).unwrap();

    let report = undo(root.path(), &world_id);
    assert_eq!(report.removed_character_ids, vec![a]);
    assert_eq!(
        compact(root.path(), &world_id, uid).visibility,
        Visibility::Characters(vec![other])
    );
    assert_eq!(changes(&report), &EntryChanges::default());
}

/// 撤銷刪掉的 uid 被插回的舊條目佔走：一律報刪除，不當欄位差異（raw 與非 raw 兩條還原路徑）。
#[test]
fn a_reused_uid_is_reported_as_deleted() {
    for with_raw in [true, false] {
        let (root, world_id) = new_table("reuse");
        let uid = data::upsert_worldbook_entry(root.path(), &world_id, gm_entry("舊條目")).unwrap();
        let raw = data::read_worldbook_raw(root.path(), &world_id).unwrap()[&uid].clone();
        let old = compact(root.path(), &world_id, uid);
        data::delete_worldbook_entry(root.path(), &world_id, uid).unwrap();
        let mut fresh = gm_entry("新條目");
        fresh.uid = uid;
        assert!(data::restore_worldbook_entry(root.path(), &world_id, fresh).unwrap());
        let created = compact(root.path(), &world_id, uid);
        let raws = if with_raw { vec![raw] } else { Vec::new() };
        assert!(append_receipt_atomic(
            root.path(),
            &world_id,
            refactor_receipt(json!({
                "worldbook_entries": [{"uid": uid, "fingerprint": worldbook_entry_fingerprint(&created)}],
                "deleted_entries": [old],
                "deleted_entries_raw": raws,
            })),
        ));

        let report = undo(root.path(), &world_id);
        assert_eq!(
            compact(root.path(), &world_id, uid).content,
            "舊條目",
            "{with_raw}"
        );
        assert_eq!(
            report.entry_changes,
            Some(EntryChanges {
                changed: Vec::new(),
                deleted: vec![uid],
            }),
            "{with_raw}"
        );
    }
}

/// 世界書讀不了：報告是 None，撤銷照做完（收據彈出）。
#[test]
fn unreadable_book_reports_none_and_still_undoes() {
    let (root, world_id) = new_table("unreadable");
    let id = import_card(
        root.path(),
        &world_id,
        &json!({"data": {"name": "甲"}}).to_string().into_bytes(),
    );
    let book = data::import_receipts_path(root.path(), &world_id)
        .unwrap()
        .with_file_name("worldbook.json");
    fs::write(&book, "{ 壞掉").unwrap();
    let report = undo(root.path(), &world_id);
    assert_eq!(report.entry_changes, None);
    assert_eq!(report.removed_character_ids, vec![id]);
    assert!(super::super::list_import_receipts(root.path(), &world_id).is_empty());
}
