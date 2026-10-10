//! 世界書路匯入別桌匯出的書：角色名單與來源卡只留本桌 id（worldbook-path-foreign-ids）。
use super::test_support::TestRoot;
use super::{import_character, import_worldbook_file};
use crate::data::{self, Visibility};
use crate::{chat_assembly, receipts};
use serde_json::{json, Value};
use std::path::Path;

const LANG: &str = "zh-TW";
const FOREIGN: &str = "01FOREIGNTABLECHARACTER0";

fn plain_card(name: &str) -> String {
    json!({"data": {"name": name, "description": "路人"}}).to_string()
}

/// 一條常駐條目的卡；`table_tavern` 為 None 時不寫擴充欄位。
fn entry_card(name: &str, content: &str, table_tavern: Option<Value>) -> String {
    let mut entry = json!({"comment": "設定", "keys": [], "content": content, "constant": true,
        "enabled": true, "position": "before_char"});
    if let Some(table_tavern) = table_tavern {
        entry["extensions"] = json!({ "table_tavern": table_tavern });
    }
    json!({"data": {"name": name, "character_book": {"entries": [entry]}}}).to_string()
}

fn new_table(label: &str) -> (TestRoot, String) {
    let root = TestRoot::new(&format!("foreign-ids-{label}"));
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    (root, world_id)
}

fn local_character(root: &Path, world_id: &str, name: &str) -> String {
    import_character(root, world_id, plain_card(name).as_bytes(), "#3366ff", LANG)
        .unwrap()
        .id
}

fn via_worldbook(root: &Path, world_id: &str, card: &str) {
    import_worldbook_file(
        root,
        world_id,
        card.as_bytes(),
        "書",
        &data::test_exclusive(world_id),
    )
    .unwrap();
}

fn entry_value(root: &Path, world_id: &str, content: &str) -> (u64, Value) {
    data::read_worldbook_raw(root, world_id)
        .unwrap()
        .into_iter()
        .find(|(_, value)| value["content"] == content)
        .unwrap()
}

fn table_tavern(value: &Value) -> &Value {
    &value["extensions"]["table_tavern"]
}

/// 名單：全別桌、空名單、混了非字串都寫成 gm；混雜的只留本桌 id；gm／public 照留。
#[test]
fn worldbook_route_filters_the_character_list() {
    let (root, world_id) = new_table("visibility");
    let local = local_character(root.path(), &world_id, "本桌");
    let cases = [
        ("全別桌", json!({"characters": [FOREIGN]}), json!("gm")),
        ("空名單", json!({"characters": []}), json!("gm")),
        ("混非字串", json!({"characters": [local, 42]}), json!("gm")),
        (
            "混雜",
            json!({"characters": [FOREIGN, local]}),
            json!({"characters": [local]}),
        ),
        ("公開", json!("public"), json!("public")),
        ("明寫gm", json!("gm"), json!("gm")),
    ];
    for (content, visibility, _) in &cases {
        let card = entry_card("書", content, Some(json!({ "visibility": visibility })));
        via_worldbook(root.path(), &world_id, &card);
    }
    for (content, _, expected) in &cases {
        let (_, value) = entry_value(root.path(), &world_id, content);
        assert_eq!(&table_tavern(&value)["visibility"], expected, "{content}");
    }
}

/// 來源卡：名單型全別桌移除欄位、混雜只留本桌；明寫 gm 的原樣保留；名單濾成 gm 的照樣濾。
#[test]
fn worldbook_route_filters_source_cards_except_settled_gm() {
    let (root, world_id) = new_table("source-cards");
    let local = local_character(root.path(), &world_id, "本桌");
    let cases = [
        (
            "名單全別桌",
            json!({"visibility": {"characters": [local]}, "source_cards": [FOREIGN]}),
            None,
        ),
        (
            "名單混雜",
            json!({"visibility": {"characters": [local]}, "source_cards": [FOREIGN, local]}),
            Some(json!([local])),
        ),
        (
            "明寫gm",
            json!({"visibility": "gm", "source_cards": [FOREIGN]}),
            Some(json!([FOREIGN])),
        ),
        (
            "濾成gm",
            json!({"visibility": {"characters": [FOREIGN]}, "source_cards": [FOREIGN]}),
            None,
        ),
    ];
    for (content, table_tavern_value, _) in &cases {
        let card = entry_card("書", content, Some(table_tavern_value.clone()));
        via_worldbook(root.path(), &world_id, &card);
    }
    for (content, _, expected) in &cases {
        let (_, value) = entry_value(root.path(), &world_id, content);
        assert_eq!(
            table_tavern(&value).get("source_cards"),
            expected.as_ref(),
            "{content}"
        );
    }
}

/// 別桌落定的 GM：再匯同內容的角色卡仍是 GM；沒有來源卡的 gm 則會被收成該卡的名單。
#[test]
fn settled_foreign_gm_stays_gm_but_plain_gm_is_claimed() {
    for (label, settled) in [("settled", true), ("plain", false)] {
        let (root, world_id) = new_table(&format!("settle-{label}"));
        let table_tavern_value = match settled {
            true => json!({"visibility": "gm", "source_cards": [FOREIGN]}),
            false => json!({"visibility": "gm"}),
        };
        via_worldbook(
            root.path(),
            &world_id,
            &entry_card("書", "祕密", Some(table_tavern_value)),
        );
        let card = import_character(
            root.path(),
            &world_id,
            entry_card("莉亞", "祕密", None).as_bytes(),
            "#3366ff",
            LANG,
        )
        .unwrap();
        let entries = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(entries.len(), 1, "{label}");
        let expected = match settled {
            true => Visibility::Gm,
            false => Visibility::Characters(vec![card.id.clone()]),
        };
        assert_eq!(entries[0].visibility, expected, "{label}");
    }
}

/// 合併：本桌名單不吸進別桌 id；沒來源卡的 GM 遇到全別桌名單仍是 GM、之後仍收得成名單；
/// 遇到「明寫 gm＋別桌來源卡」則被落定，之後的角色卡收不成名單。
#[test]
fn merging_keeps_foreign_ids_out_of_table_entries() {
    let (root, world_id) = new_table("merge-list");
    let owner = import_character(
        root.path(),
        &world_id,
        entry_card("甲", "名單", None).as_bytes(),
        "#3366ff",
        LANG,
    )
    .unwrap();
    via_worldbook(
        root.path(),
        &world_id,
        &entry_card(
            "書",
            "名單",
            Some(json!({"visibility": {"characters": [FOREIGN]}, "source_cards": [FOREIGN]})),
        ),
    );
    let (_, value) = entry_value(root.path(), &world_id, "名單");
    assert_eq!(
        table_tavern(&value)["visibility"],
        json!({"characters": [owner.id]})
    );
    assert_eq!(table_tavern(&value)["source_cards"], json!([owner.id]));

    for (label, incoming, claimed) in [
        (
            "foreign-list",
            json!({"visibility": {"characters": [FOREIGN]}}),
            true,
        ),
        (
            "settled-gm",
            json!({"visibility": "gm", "source_cards": [FOREIGN]}),
            false,
        ),
    ] {
        let (root, world_id) = new_table(&format!("merge-{label}"));
        via_worldbook(root.path(), &world_id, &entry_card("書", "共用", None));
        via_worldbook(
            root.path(),
            &world_id,
            &entry_card("別桌書", "共用", Some(incoming)),
        );
        let (_, value) = entry_value(root.path(), &world_id, "共用");
        assert_eq!(table_tavern(&value)["visibility"], "gm", "{label}");
        let card = import_character(
            root.path(),
            &world_id,
            entry_card("莉亞", "共用", None).as_bytes(),
            "#3366ff",
            LANG,
        )
        .unwrap();
        let entries = data::read_worldbook(root.path(), &world_id).unwrap();
        let expected = match claimed {
            true => Visibility::Characters(vec![card.id.clone()]),
            false => Visibility::Gm,
        };
        assert_eq!(entries[0].visibility, expected, "{label}");
    }
}

/// 準備一桌：角色卡路條目（本桌名單＋來源卡）與世界書路混雜名單條目，整本匯出成文字。
fn exported_table(label: &str) -> (TestRoot, String, String, Vec<(u64, Value)>) {
    let (root, world_id) = new_table(label);
    let local = local_character(root.path(), &world_id, "本桌");
    import_character(
        root.path(),
        &world_id,
        entry_card("甲", "隨身", None).as_bytes(),
        "#3366ff",
        LANG,
    )
    .unwrap();
    via_worldbook(
        root.path(),
        &world_id,
        &entry_card(
            "書",
            "混雜",
            Some(json!({"visibility": {"characters": [local, FOREIGN]}})),
        ),
    );
    let path = root.path().join("exported-worldbook.json");
    data::export_worldbook(root.path(), &world_id, &path).unwrap();
    let text = std::fs::read_to_string(path).unwrap();
    let before = data::read_worldbook_raw(root.path(), &world_id)
        .unwrap()
        .into_iter()
        .collect();
    (root, world_id, text, before)
}

fn visibility_and_cards(entries: &[(u64, Value)]) -> Vec<(Value, Value)> {
    let mut fields: Vec<(Value, Value)> = entries
        .iter()
        .map(|(_, value)| {
            (
                value["content"].clone(),
                json!([
                    table_tavern(value)["visibility"],
                    table_tavern(value).get("source_cards")
                ]),
            )
        })
        .collect();
    fields.sort_by_key(|(content, _)| content.to_string());
    fields
}

/// 同桌往返（去重）：整本匯出再匯回同一桌，全數略過，名單與來源卡不變，收據沒有還原項。
#[test]
fn same_table_round_trip_dedupes_without_changes() {
    let (root, world_id, text, before) = exported_table("round-trip-dedupe");
    let book =
        data::import_worldbook_as(root.path(), &world_id, &text, &data::BookOwner::Gm).unwrap();
    assert_eq!(book.summary.imported, 0);
    assert_eq!(book.summary.skipped, before.len());
    assert!(book.restores.is_empty());
    let after: Vec<(u64, Value)> = data::read_worldbook_raw(root.path(), &world_id)
        .unwrap()
        .into_iter()
        .collect();
    assert_eq!(visibility_and_cards(&after), visibility_and_cards(&before));
}

/// 同桌往返（新插入）：匯出、清空世界書、匯回，條目全數新插入，本桌名單與來源卡原樣保留。
#[test]
fn same_table_round_trip_into_empty_book_keeps_table_ids() {
    let (root, world_id, text, before) = exported_table("round-trip-empty");
    for (uid, _) in &before {
        data::delete_worldbook_entry(root.path(), &world_id, *uid).unwrap();
    }
    let book =
        data::import_worldbook_as(root.path(), &world_id, &text, &data::BookOwner::Gm).unwrap();
    assert_eq!(book.summary.imported, before.len());
    let after: Vec<(u64, Value)> = data::read_worldbook_raw(root.path(), &world_id)
        .unwrap()
        .into_iter()
        .collect();
    assert_eq!(visibility_and_cards(&after), visibility_and_cards(&before));
    let (_, mixed) = entry_value(root.path(), &world_id, "混雜");
    assert_eq!(
        table_tavern(&mixed)["visibility"]["characters"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

/// 去重收據：混雜名單只併本桌 id，撤銷還原原名單；玩家改過的可見度撤銷時保留。
#[test]
fn merge_receipt_restores_or_keeps_player_change() {
    for (label, player_changes) in [("restore", false), ("player", true)] {
        let (root, world_id) = new_table(&format!("receipt-{label}"));
        let held = data::test_exclusive(&world_id);
        let other = local_character(root.path(), &world_id, "乙");
        let owner = import_character(
            root.path(),
            &world_id,
            entry_card("甲", "收據", None).as_bytes(),
            "#3366ff",
            LANG,
        )
        .unwrap();
        let (uid, before) = entry_value(root.path(), &world_id, "收據");
        import_worldbook_file(
            root.path(),
            &world_id,
            entry_card(
                "書",
                "收據",
                Some(json!({"visibility": {"characters": [other, FOREIGN]}})),
            )
            .as_bytes(),
            "書",
            &held,
        )
        .unwrap();
        let (_, merged) = entry_value(root.path(), &world_id, "收據");
        assert_eq!(
            table_tavern(&merged)["visibility"],
            json!({"characters": [owner.id, other]}),
            "{label}"
        );
        if player_changes {
            let mut entry = data::read_worldbook(root.path(), &world_id)
                .unwrap()
                .into_iter()
                .find(|entry| entry.uid == uid)
                .unwrap();
            entry.visibility = Visibility::Public;
            data::upsert_worldbook_entry(root.path(), &world_id, entry).unwrap();
        }
        receipts::undo_last_import(root.path(), &world_id, &held).unwrap();
        let (_, after) = entry_value(root.path(), &world_id, "收據");
        let expected = match player_changes {
            true => json!("public"),
            false => table_tavern(&before)["visibility"].clone(),
        };
        assert_eq!(table_tavern(&after)["visibility"], expected, "{label}");
    }
}

/// GM 掃描：全別桌名單改成 gm 之後不再是機密條目。
#[test]
fn foreign_list_is_not_confidential_for_the_gm() {
    let (root, world_id) = new_table("gm-scan");
    via_worldbook(
        root.path(),
        &world_id,
        &entry_card(
            "書",
            "別桌限定",
            Some(json!({"visibility": {"characters": [FOREIGN]}})),
        ),
    );
    let (uid, _) = entry_value(root.path(), &world_id, "別桌限定");
    let materials = chat_assembly::gm_materials(root.path(), &world_id).unwrap();
    let scan = chat_assembly::test_gm_scan(root.path(), &world_id, &materials, LANG);
    let placed = scan
        .placed
        .iter()
        .find(|entry| entry.uid == uid)
        .expect("常駐條目進 GM 掃描");
    assert!(!placed.confidential());
}
