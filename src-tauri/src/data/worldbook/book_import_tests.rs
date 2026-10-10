//! 世界書路的可見度讀寫、清重複的合併、撤銷時照原始值插回。

use super::*;
use crate::data::test_support::*;
use crate::data::*;
use serde_json::json;

fn one_entry_book(extensions: serde_json::Value) -> String {
    json!({"entries": {"0": {"uid": 0, "comment": "設定", "key": [], "content": "設定內容",
        "constant": true, "extensions": extensions}}})
    .to_string()
}

/// 世界書路：缺欄位寫成 GM；格式壞掉的原樣保留、讀取端退回 GM——兩者分開。
#[test]
fn worldbook_route_missing_and_broken_visibility_read_as_gm() {
    let root = TestRoot::new("book-visibility-missing");
    let world_id = create_world(root.path(), "桌").unwrap();
    import_worldbook(root.path(), &world_id, &one_entry_book(json!({}))).unwrap();
    let raw = read_worldbook_raw(root.path(), &world_id).unwrap();
    assert_eq!(raw[&0]["extensions"]["table_tavern"]["visibility"], "gm");
    assert_eq!(
        read_worldbook(root.path(), &world_id).unwrap()[0].visibility,
        Visibility::Gm
    );

    let root = TestRoot::new("book-visibility-broken");
    let world_id = create_world(root.path(), "桌").unwrap();
    import_worldbook(
        root.path(),
        &world_id,
        &one_entry_book(json!({"table_tavern": {"visibility": 42}})),
    )
    .unwrap();
    let raw = read_worldbook_raw(root.path(), &world_id).unwrap();
    assert_eq!(raw[&0]["extensions"]["table_tavern"]["visibility"], 42);
    assert_eq!(
        read_worldbook(root.path(), &world_id).unwrap()[0].visibility,
        Visibility::Gm
    );
}

fn entry_with(visibility: Visibility, cards: &[&str]) -> serde_json::Value {
    let mut value = worldbook_entry_value(&WorldbookEntry {
        uid: 0,
        title: "傳說".to_owned(),
        keys: vec!["月".to_owned()],
        content: "月下的傳說".to_owned(),
        constant: false,
        order: 1,
        disabled: false,
        visibility,
        is_person: false,
        locked: false,
    });
    set_source_cards(
        &mut value,
        &cards
            .iter()
            .map(|card| (*card).to_owned())
            .collect::<Vec<_>>(),
    );
    value
}

/// 清重複保留顯示順序最前那條，被刪那條的角色名單與來源卡併進來。
#[test]
fn dedupe_worldbook_merges_visibility_into_the_kept_entry() {
    let root = TestRoot::new("book-dedupe-merge");
    let world_id = create_world(root.path(), "桌").unwrap();
    insert_worldbook_entry_raw(
        root.path(),
        &world_id,
        entry_with(Visibility::Characters(vec!["乙".to_owned()]), &["乙"]),
    )
    .unwrap();
    insert_worldbook_entry_raw(
        root.path(),
        &world_id,
        entry_with(Visibility::Characters(vec!["甲".to_owned()]), &["甲"]),
    )
    .unwrap();
    assert_eq!(dedupe_worldbook(root.path(), &world_id).unwrap(), 1);
    let raw = read_worldbook_raw(root.path(), &world_id).unwrap();
    assert_eq!(raw.len(), 1);
    let kept = raw.values().next().unwrap();
    assert_eq!(
        visibility_from_value(kept),
        Visibility::Characters(vec!["甲".to_owned(), "乙".to_owned()])
    );
    assert_eq!(
        source_cards_of(kept),
        vec!["甲".to_owned(), "乙".to_owned()]
    );
}

/// 撤銷插回：原 uid 空著照原 uid 插回、重做不插第二份；原 uid 被佔走改插新 uid、重做同樣不插第二份；
/// 只差次要鍵的條目不算已插回；欄位順序不同不算差異。
#[test]
fn restore_deleted_entry_raw_handles_conflicts_and_retries() {
    let root = TestRoot::new("book-restore-raw");
    let world_id = create_world(root.path(), "桌").unwrap();
    let mut original = entry_with(Visibility::Characters(vec!["甲".to_owned()]), &["甲"]);
    original["keysecondary"] = json!(["夜"]);
    original["position"] = json!(4);
    let uid = insert_worldbook_entry_raw(root.path(), &world_id, original.clone()).unwrap();
    let stored = read_worldbook_raw(root.path(), &world_id).unwrap()[&uid].clone();
    delete_worldbook_entry(root.path(), &world_id, uid).unwrap();

    restore_deleted_entry_raw(root.path(), &world_id, &stored).unwrap();
    restore_deleted_entry_raw(root.path(), &world_id, &stored).unwrap();
    let raw = read_worldbook_raw(root.path(), &world_id).unwrap();
    assert_eq!(raw.len(), 1);
    assert_eq!(raw[&uid]["keysecondary"], json!(["夜"]));
    assert_eq!(raw[&uid]["position"], 4);
    assert_eq!(source_cards_of(&raw[&uid]), vec!["甲".to_owned()]);

    // 欄位順序不同（重新序列化）＝同一條，已插回
    let reordered: serde_json::Value = serde_json::from_str(&{
        let mut fields: Vec<(String, serde_json::Value)> =
            stored.as_object().unwrap().clone().into_iter().collect();
        fields.reverse();
        let object: serde_json::Map<String, serde_json::Value> = fields.into_iter().collect();
        serde_json::to_string(&object).unwrap()
    })
    .unwrap();
    restore_deleted_entry_raw(root.path(), &world_id, &reordered).unwrap();
    assert_eq!(read_worldbook_raw(root.path(), &world_id).unwrap().len(), 1);

    // 原 uid 被一條只差次要鍵的條目佔走：不算已插回，改插新 uid；重做不插第二份
    delete_worldbook_entry(root.path(), &world_id, uid).unwrap();
    let mut near = stored.clone();
    near["keysecondary"] = json!([]);
    let occupant = insert_worldbook_entry_raw(root.path(), &world_id, near).unwrap();
    assert_eq!(occupant, uid);
    restore_deleted_entry_raw(root.path(), &world_id, &stored).unwrap();
    restore_deleted_entry_raw(root.path(), &world_id, &stored).unwrap();
    let raw = read_worldbook_raw(root.path(), &world_id).unwrap();
    assert_eq!(raw.len(), 2);
    let restored = raw
        .iter()
        .find(|(key, _)| **key != uid)
        .map(|(_, value)| value)
        .unwrap();
    assert_eq!(restored["keysecondary"], json!(["夜"]));
    assert_eq!(source_cards_of(restored), vec!["甲".to_owned()]);
}
