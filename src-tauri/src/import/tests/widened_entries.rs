//! 匯入回報 `widened_entries`（import-refresh-world-editor）：被合併擴大了可見度的既有條目 uid，
//! 開著的世界設定據此定基草稿。角色卡路與世界書路對等；只動來源卡、全新條目不報；同一條只報一次。

use crate::import::test_support::TestRoot;
use crate::import::{import_character_file, import_worldbook_file};
use crate::{data, mechanism};
use serde_json::{json, Value};
use std::path::Path;

const LANG: &str = "zh-TW";

fn card(name: &str, entries: Vec<Value>) -> Vec<u8> {
    json!({"data": {"name": name, "character_book": {"entries": entries}}})
        .to_string()
        .into_bytes()
}

fn card_entry(content: &str, extensions: Value) -> Value {
    json!({"comment": "條目", "keys": [], "content": content, "constant": true, "enabled": true,
           "position": "before_char", "extensions": extensions})
}

/// 物件形獨立書；visibility＝None 不寫可見度（世界書路給 GM）
fn book(content: &str, visibility: Option<Value>) -> Vec<u8> {
    let mut entry = json!({"uid": 0, "comment": "條目", "key": [], "content": content,
                           "constant": true});
    if let Some(visibility) = visibility {
        entry["extensions"] = json!({"table_tavern": {"visibility": visibility}});
    }
    json!({"entries": {"0": entry}}).to_string().into_bytes()
}

fn uid_of(root: &Path, world_id: &str, content: &str) -> u64 {
    data::read_worldbook(root, world_id)
        .unwrap()
        .into_iter()
        .find(|entry| entry.content == content)
        .unwrap()
        .uid
}

fn import_card(
    root: &Path,
    world_id: &str,
    bytes: &[u8],
) -> crate::import::files::Imported<data::CharacterMeta> {
    import_character_file(
        root,
        world_id,
        bytes,
        "#3366ff",
        LANG,
        &data::test_exclusive(world_id),
    )
    .unwrap()
}

fn import_book(
    root: &Path,
    world_id: &str,
    bytes: &[u8],
) -> crate::import::files::Imported<data::WorldbookImport> {
    import_worldbook_file(root, world_id, bytes, "書", &data::test_exclusive(world_id)).unwrap()
}

/// 角色卡路：桌上 GM 條目被明寫 public 的同內容條目合併 → 擴大，回報該 uid
#[test]
fn card_route_reports_widened_entry() {
    let root = TestRoot::new("widened-card");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    import_book(root.path(), &world_id, &book("共用設定", None));
    let uid = uid_of(root.path(), &world_id, "共用設定");
    let imported = import_card(
        root.path(),
        &world_id,
        &card(
            "莉亞",
            vec![card_entry(
                "共用設定",
                json!({"table_tavern": {"visibility": "public"}}),
            )],
        ),
    );
    assert_eq!(imported.widened_entries, vec![uid]);
}

/// 世界書路：桌上角色名單條目被明寫含本桌 id 名單（加上另一位）的同內容條目合併 → 擴大，回報該 uid
#[test]
fn book_route_reports_widened_entry() {
    let root = TestRoot::new("widened-book");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let first = import_card(
        root.path(),
        &world_id,
        &card("甲", vec![card_entry("共用設定", json!({}))]),
    );
    let plain = json!({"data": {"name": "乙", "description": "路人"}}).to_string();
    let second = import_card(root.path(), &world_id, plain.as_bytes());
    let uid = uid_of(root.path(), &world_id, "共用設定");
    let imported = import_book(
        root.path(),
        &world_id,
        &book(
            "共用設定",
            Some(json!({"characters": [first.value.id.clone(), second.value.id.clone()]})),
        ),
    );
    assert_eq!(imported.widened_entries, vec![uid]);
    // 世界書路明寫 public 也算擴大
    let root = TestRoot::new("widened-book-public");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    import_card(
        root.path(),
        &world_id,
        &card("甲", vec![card_entry("共用設定", json!({}))]),
    );
    let uid = uid_of(root.path(), &world_id, "共用設定");
    let imported = import_book(
        root.path(),
        &world_id,
        &book("共用設定", Some(json!("public"))),
    );
    assert_eq!(imported.widened_entries, vec![uid]);
}

/// 只多了來源卡（可見度沒變）不報；世界書路同內容但可見度沒變（GM 併進名單條目）也不報
#[test]
fn unchanged_visibility_is_not_reported() {
    let root = TestRoot::new("widened-cards-only");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let entry = |extensions: Value| card_entry("人人都知道", extensions);
    import_card(
        root.path(),
        &world_id,
        &card(
            "甲",
            vec![entry(json!({"table_tavern": {"visibility": "public"}}))],
        ),
    );
    let imported = import_card(root.path(), &world_id, &card("乙", vec![entry(json!({}))]));
    let (_, value) = data::read_worldbook_raw(root.path(), &world_id)
        .unwrap()
        .into_iter()
        .find(|(_, value)| value["content"] == "人人都知道")
        .unwrap();
    assert_eq!(data::source_cards_of(&value).len(), 2, "來源卡確實合併了");
    assert!(imported.widened_entries.is_empty());

    let root = TestRoot::new("widened-book-same");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    import_card(
        root.path(),
        &world_id,
        &card("甲", vec![card_entry("共用設定", json!({}))]),
    );
    let imported = import_book(root.path(), &world_id, &book("共用設定", None));
    assert_eq!(imported.value.skipped, 1);
    assert!(imported.widened_entries.is_empty());
}

/// 全新條目兩條路都不報
#[test]
fn new_entries_are_not_reported() {
    let root = TestRoot::new("widened-new");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let imported = import_book(root.path(), &world_id, &book("書的條目", None));
    assert_eq!(imported.value.imported, 1);
    assert!(imported.widened_entries.is_empty());
    let imported = import_card(
        root.path(),
        &world_id,
        &card("甲", vec![card_entry("卡的條目", json!({}))]),
    );
    assert!(imported.widened_entries.is_empty());
}

/// 同一條既有條目被同批多個來源條目合併：只報一次
#[test]
fn repeated_hits_report_once() {
    let root = TestRoot::new("widened-repeated");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    import_book(root.path(), &world_id, &book("既有條目", None));
    let uid = uid_of(root.path(), &world_id, "既有條目");
    let public = json!({"table_tavern": {"visibility": "public"}});
    let imported = import_card(
        root.path(),
        &world_id,
        &card(
            "莉亞",
            vec![
                card_entry("既有條目", json!({})),
                card_entry("既有條目", public),
            ],
        ),
    );
    assert_eq!(imported.widened_entries, vec![uid]);
}

/// 匯入收編機制鷹架：帳本多一筆收編紀錄（世界設定的帳本靠匯入通知重讀）
#[test]
fn import_appends_absorbed_record_to_ledger() {
    let root = TestRoot::new("widened-ledger");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let before = mechanism::read_ledger(root.path(), &world_id).entries.len();
    let entry = json!({"comment": "[initvar]初始變數", "keys": [], "content": "hp: 10",
                       "constant": true, "enabled": true, "position": "before_char"});
    import_card(root.path(), &world_id, &card("莉亞", vec![entry]));
    let ledger = mechanism::read_ledger(root.path(), &world_id);
    assert!(ledger.entries.len() > before);
    assert!(ledger
        .entries
        .iter()
        .any(|entry| entry.title.starts_with("[initvar]")
            && entry.kind == mechanism::RecordKind::Absorbed));
}
