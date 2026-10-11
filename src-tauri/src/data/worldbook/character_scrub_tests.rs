//! 刪角色後清世界書名單與來源卡的規則（character-delete-visibility-cleanup）。

use super::*;
use crate::data::test_support::*;
use crate::data::*;
use serde_json::{json, Value};

const A: &str = "01CHARACTERAAAAAAAAAAAAAAA";
const B: &str = "01CHARACTERBBBBBBBBBBBBBBB";
const X: &str = "01CHARACTERXXXXXXXXXXXXXXX";

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|id| (*id).to_owned()).collect()
}

fn entry_with(table_tavern: Value) -> Value {
    json!({"uid": 0, "comment": "設定", "key": [], "content": "內容", "constant": true,
        "extensions": {"table_tavern": table_tavern}})
}

fn scrubbed(table_tavern: Value, remove: &[&str]) -> (bool, Value) {
    let mut value = entry_with(table_tavern);
    let changed = scrub_entry_value(&mut value, &ids(remove));
    (changed, value["extensions"]["table_tavern"].clone())
}

/// 名單：拿掉要清的 id，重複的一併拿掉；清空寫成 gm；沒命中不動。
#[test]
fn character_list_drops_ids_and_empties_to_gm() {
    let cases = [
        (json!({"characters": [A]}), vec![A], json!("gm"), true),
        (
            json!({"characters": [A, B]}),
            vec![A],
            json!({"characters": [B]}),
            true,
        ),
        (json!({"characters": [A, A]}), vec![A], json!("gm"), true),
        (
            json!({"characters": [B]}),
            vec![A],
            json!({"characters": [B]}),
            false,
        ),
        (json!({"characters": [A, B]}), vec![A, B], json!("gm"), true),
    ];
    for (visibility, remove, expected, changed) in cases {
        let (did, table_tavern) = scrubbed(json!({ "visibility": visibility.clone() }), &remove);
        assert_eq!(table_tavern["visibility"], expected, "{visibility}");
        assert_eq!(did, changed, "{visibility}");
    }
}

/// gm／public、讀不懂的名單、空名單都不動；public 帶來源卡只移除欄位。
#[test]
fn other_visibility_values_are_untouched() {
    for visibility in [
        json!("gm"),
        json!("public"),
        json!({"characters": [A, 42]}),
        json!({"characters": []}),
    ] {
        let (did, table_tavern) = scrubbed(json!({ "visibility": visibility.clone() }), &[A]);
        assert!(!did, "{visibility}");
        assert_eq!(table_tavern["visibility"], visibility);
    }
    let (did, table_tavern) = scrubbed(json!({"visibility": "public", "source_cards": [A]}), &[A]);
    assert!(did);
    assert_eq!(table_tavern, json!({"visibility": "public"}));
}

/// 來源卡：只剩要清的 id 時移除欄位；其餘照留；名單清成 gm 時留下別的來源卡（落定的 Gm）。
#[test]
fn source_cards_drop_ids_and_vanish_when_empty() {
    let (_, table_tavern) = scrubbed(json!({"visibility": "gm", "source_cards": [A]}), &[A]);
    assert_eq!(table_tavern, json!({"visibility": "gm"}));
    let (_, table_tavern) = scrubbed(json!({"visibility": "gm", "source_cards": [A, X]}), &[A]);
    assert_eq!(table_tavern["source_cards"], json!([X]));
    let (_, table_tavern) = scrubbed(
        json!({"visibility": {"characters": [A]}, "source_cards": [A, X]}),
        &[A],
    );
    assert_eq!(
        table_tavern,
        json!({"visibility": "gm", "source_cards": [X]})
    );
}

/// 精簡條目（收據快照）同一套規則。
#[test]
fn compact_entry_follows_the_same_rule() {
    let mut entry = worldbook_entry(1, "快照");
    entry.visibility = Visibility::Characters(ids(&[A, B]));
    assert!(scrub_entry(&mut entry, &ids(&[A])));
    assert_eq!(entry.visibility, Visibility::Characters(ids(&[B])));
    assert!(scrub_entry(&mut entry, &ids(&[B])));
    assert_eq!(entry.visibility, Visibility::Gm);
    assert!(!scrub_entry(&mut entry, &ids(&[B])));
}

fn book_with(root: &TestRoot, world_id: &str, table_tavern: Value) -> std::path::PathBuf {
    let path = worldbook_path(root.path(), world_id).unwrap();
    commit_world_write(
        &path,
        json!({"entries": {"0": entry_with(table_tavern)}})
            .to_string()
            .as_bytes(),
    )
    .unwrap();
    path
}

/// 整本書：第一次清有寫；再跑一次沒有變動、逐位元組不變，注入寫入失敗也不報錯（證明沒寫）。
#[test]
fn scrubbing_the_book_is_idempotent() {
    let root = TestRoot::new("scrub-idempotent");
    let world_id = create_world(root.path(), "桌").unwrap();
    let path = book_with(&root, &world_id, json!({"visibility": {"characters": [A]}}));
    let first = scrub_character_ids(root.path(), &world_id, &ids(&[A])).unwrap();
    assert_eq!(first.changed, vec![0]);
    assert_eq!(
        first.before[&0]["extensions"]["table_tavern"]["visibility"],
        json!({"characters": [A]})
    );
    assert_eq!(
        read_worldbook(root.path(), &world_id).unwrap()[0].visibility,
        Visibility::Gm
    );
    let bytes = std::fs::read(&path).unwrap();
    let _write = WriteFailGuard::partial(5);
    let _rename = RenameFailGuard::fail(5);
    let second = scrub_character_ids(root.path(), &world_id, &ids(&[A])).unwrap();
    assert!(second.changed.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

/// 原子寫：暫存檔寫壞或改名失敗都回錯，世界書逐位元組不變、沒有殘留暫存檔。
#[test]
fn scrub_write_failure_leaves_the_book_untouched() {
    for label in ["partial", "rename"] {
        let root = TestRoot::new(&format!("scrub-atomic-{label}"));
        let world_id = create_world(root.path(), "桌").unwrap();
        let path = book_with(&root, &world_id, json!({"visibility": {"characters": [A]}}));
        let bytes = std::fs::read(&path).unwrap();
        let result = if label == "partial" {
            let _guard = WriteFailGuard::partial_ending("worldbook.json.tmp", 1);
            scrub_character_ids(root.path(), &world_id, &ids(&[A]))
        } else {
            let _guard = RenameFailGuard::fail_ending("worldbook.json", 1);
            scrub_character_ids(root.path(), &world_id, &ids(&[A]))
        };
        assert!(result.is_err(), "{label}");
        assert_eq!(std::fs::read(&path).unwrap(), bytes, "{label}");
        assert!(
            !path.with_file_name("worldbook.json.tmp").exists(),
            "{label}"
        );
    }
}

/// 可見度還原的 before 為 None＝移除該欄位：可見度讀成 GM、來源卡消失。
#[test]
fn restore_before_none_removes_the_fields() {
    let mut value = entry_with(json!({"visibility": {"characters": [A]}, "source_cards": [A]}));
    let restore: VisibilityRestore = serde_json::from_value(json!({
        "uid": 0,
        "after_visibility": {"characters": [A]},
        "after_cards": [A]
    }))
    .unwrap();
    assert!(restore_after_matches(&value, &restore));
    apply_restore_before(&mut value, &restore);
    assert_eq!(value["extensions"]["table_tavern"], json!({}));
    assert_eq!(entry_view_of(&value).visibility, Visibility::Gm);
}
