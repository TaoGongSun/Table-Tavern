//! 機制帳本的匯入路徑對等（mechanism-ledger-card-path）：含 `<%` 的條目只由 extract_triggers 判決
//! （認得記接管、認不得記跳過），角色卡路、世界書路、網頁存檔兩路結果相同，且各自對。
//! 只驗兩路相等擋不住「兩路同漏」，所以每個情境都另外斷言分類與條數。

use crate::import::test_support::TestRoot;
use crate::import::{import_character_file, import_web_save, import_worldbook_file};
use crate::mechanism::{self, RecordKind};
use crate::{data, refactor_ai};
use serde_json::{json, Value};
use std::path::Path;

const LANG: &str = "zh-TW";
const KNOWN: &str = "已知腳本";
const UNKNOWN: &str = "未知腳本";
const INITVAR: &str = "[initvar]初始";
const MIXED: &str = "[initvar]混合腳本";

/// 單一 if 鏈、getvar 有來源：parse_triggers 認得
const KNOWN_EJS: &str = "<%_ if (typeof affection === 'undefined') var affection = getvar('stat_data.Heroes.亞瑟.Affection', { defaults: 0 }); _%>\n<%_ if (affection >= 60) { _%>親近<%_ } else { _%>觀察<%_ } _%>";
/// 迴圈：parse_triggers 認不得
const UNKNOWN_EJS: &str = "<% for (let i = 0; i < 3; i++) { %>重複<% } %>";

type LedgerRow = (RecordKind, String, bool);

fn entry(comment: &str, content: &str, enabled: bool) -> Value {
    json!({"comment": comment, "keys": [comment], "content": content, "constant": false,
           "enabled": enabled, "position": "before_char"})
}

fn plain() -> Value {
    entry("酒館", "灰燼旅店，雨夜。", true)
}

/// 認得的 EJS、認不得的 EJS、`[initvar]`、`[initvar]` 標記但內容是 EJS 的混合條目、普通條目
fn mixed_entries() -> Vec<Value> {
    vec![
        entry(KNOWN, KNOWN_EJS, true),
        entry(UNKNOWN, UNKNOWN_EJS, true),
        entry(INITVAR, "Heroes:\n  亞瑟:\n    Affection: 5\n", false),
        entry(MIXED, UNKNOWN_EJS, true),
        plain(),
    ]
}

fn card(entries: Vec<Value>) -> Vec<u8> {
    json!({"spec": "chara_card_v2", "spec_version": "2.0",
           "data": {"name": "莉亞", "first_mes": "妳來了。", "character_book": {"entries": entries}}})
    .to_string()
    .into_bytes()
}

fn new_world(root: &TestRoot) -> String {
    data::create_world(root.path(), "酒館").unwrap()
}

fn via_character(root: &Path, world_id: &str, bytes: &[u8]) {
    let held = data::test_exclusive(world_id);
    import_character_file(root, world_id, bytes, "#3366ff", LANG, &held).unwrap();
}

fn via_worldbook(root: &Path, world_id: &str, bytes: &[u8]) {
    let held = data::test_exclusive(world_id);
    import_worldbook_file(root, world_id, bytes, "莉亞", &held).unwrap();
}

/// 帳本依 (kind, title) 排序，連同 sent
fn ledger(root: &Path, world_id: &str) -> Vec<LedgerRow> {
    let mut rows: Vec<LedgerRow> = mechanism::read_ledger(root, world_id)
        .entries
        .into_iter()
        .map(|entry| (entry.kind, entry.title, entry.sent))
        .collect();
    rows.sort_by(|a, b| format!("{:?}{}", a.0, a.1).cmp(&format!("{:?}{}", b.0, b.1)));
    rows
}

fn expected_mixed() -> Vec<LedgerRow> {
    let mut rows = vec![
        (RecordKind::Absorbed, KNOWN.to_owned(), false),
        (RecordKind::Absorbed, INITVAR.to_owned(), false),
        (RecordKind::Skipped, UNKNOWN.to_owned(), false),
        (RecordKind::Skipped, MIXED.to_owned(), false),
    ];
    rows.sort_by(|a, b| format!("{:?}{}", a.0, a.1).cmp(&format!("{:?}{}", b.0, b.1)));
    rows
}

fn both_routes(bytes: &[u8]) -> (Vec<LedgerRow>, Vec<LedgerRow>) {
    let root = TestRoot::new("ledger-route");
    let character = new_world(&root);
    via_character(root.path(), &character, bytes);
    let worldbook = new_world(&root);
    via_worldbook(root.path(), &worldbook, bytes);
    (
        ledger(root.path(), &character),
        ledger(root.path(), &worldbook),
    )
}

#[test]
fn both_routes_record_recognized_as_absorbed_and_unrecognized_as_skipped() {
    let (character, worldbook) = both_routes(&card(mixed_entries()));
    assert_eq!(character, expected_mixed());
    assert_eq!(worldbook, expected_mixed());
}

/// 只有認不出的 EJS（沒有初始樹、規則、認得的觸發）：狀態不用動，但跳過照記
#[test]
fn unrecognized_only_card_still_records_skipped_on_both_routes() {
    let (character, worldbook) =
        both_routes(&card(vec![entry(UNKNOWN, UNKNOWN_EJS, true), plain()]));
    let expected = vec![(RecordKind::Skipped, UNKNOWN.to_owned(), false)];
    assert_eq!(character, expected);
    assert_eq!(worldbook, expected);
}

#[test]
fn reimporting_the_same_card_keeps_the_same_ledger() {
    let bytes = card(mixed_entries());
    let root = TestRoot::new("ledger-reimport");
    for import in [via_character, via_worldbook] {
        let world_id = new_world(&root);
        import(root.path(), &world_id, &bytes);
        import(root.path(), &world_id, &bytes);
        assert_eq!(ledger(root.path(), &world_id), expected_mixed());
    }
}

/// 修正前角色卡路會替認不得的 EJS 記一筆接管：不遷移，但同一張卡重匯一次（任一路）就改回跳過
#[test]
fn table_with_the_old_wrong_absorbed_record_is_corrected_by_reimport() {
    let bytes = card(mixed_entries());
    let root = TestRoot::new("ledger-legacy");
    for import in [via_character, via_worldbook] {
        let world_id = new_world(&root);
        via_character(root.path(), &world_id, &bytes);
        // 舊版 book_import 在 import_mechanism 之後寫的接管紀錄
        let legacy: Vec<mechanism::Record> = [UNKNOWN, MIXED]
            .into_iter()
            .map(|title| mechanism::Record {
                kind: RecordKind::Absorbed,
                path: title.to_owned(),
                detail: crate::ui_msg::UiMsg::LedgerScaffoldAbsorbed.to_string(),
                group: None,
            })
            .collect();
        mechanism::append_log(root.path(), &world_id, 0, &legacy);
        let wrong = ledger(root.path(), &world_id);
        assert!(wrong
            .iter()
            .all(|(kind, _, _)| *kind == RecordKind::Absorbed));

        import(root.path(), &world_id, &bytes);
        assert_eq!(ledger(root.path(), &world_id), expected_mixed());
    }
}

/// 網頁存檔匯入：角色卡路線走 card.rs、世界書路線走 web_save 自己的那段，兩路都要對
#[test]
fn web_save_routes_record_the_same_ledger() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/shared/contracts/web-save/minimal.json");
    let template: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let root = TestRoot::new("ledger-web-save");
    for route in ["character", "worldbook"] {
        let mut save = template.clone();
        save["import_route"] = json!(route);
        save["card"]["data"]["character_book"] = json!({"entries": mixed_entries()});
        let imported =
            import_web_save(root.path(), &serde_json::to_vec(&save).unwrap(), LANG).unwrap();
        assert_eq!(
            ledger(root.path(), &imported.world_id),
            expected_mixed(),
            "web save route {route}"
        );
    }
}

/// 匯入後組 AI 重構脈絡：認不得的腳本要列成重構目標，認得的列已接管（兩路都是）
#[test]
fn refactor_context_lists_unrecognized_script_as_target_after_import() {
    let bytes = card(mixed_entries());
    let root = TestRoot::new("ledger-context");
    for import in [via_character, via_worldbook] {
        let world_id = new_world(&root);
        import(root.path(), &world_id, &bytes);
        let context = refactor_ai::assemble_card_context(root.path(), &world_id).unwrap();
        for title in [UNKNOWN, MIXED] {
            assert!(
                context.contains(&format!("《{title}》曾嘗試接管失敗")),
                "{title} 應是重構目標：{context}"
            );
        }
        for title in [KNOWN, INITVAR] {
            assert!(
                context.contains(&format!("《{title}》已接管")),
                "{title} 應已接管：{context}"
            );
        }
    }
}
