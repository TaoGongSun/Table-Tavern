//! 卡內世界書的條目形狀、順序與去重（worldbook-st-trigger-parity 包 2）：V2 條目匯入時轉成 ST 物件形、
//! 新 UID 依 ST 載入順序配發（三個入口都從原文保序）、去重指紋納入所有影響觸發的欄位、匯出收回 `extensions`。

use super::test_support::TestRoot;
use super::web_save::import_web_save;
use super::{export_character, import_character, import_character_file, import_worldbook_file};
use crate::data;
use crate::world_info::entry::{from_character_book, from_world_file, RecursionDelay, WiEntry};
use serde_json::{json, Map, Value};
use std::path::Path;

const LANG: &str = "zh-TW";

fn card_text(entries: &str) -> String {
    format!(r#"{{"data":{{"name":"莉亞","character_book":{{"entries":{entries}}}}}}}"#)
}

fn raw_by_content(root: &Path, world_id: &str) -> Vec<(u64, Value)> {
    data::read_worldbook_raw(root, world_id)
        .unwrap()
        .into_iter()
        .collect()
}

fn uid_of(root: &Path, world_id: &str, content: &str) -> u64 {
    raw_by_content(root, world_id)
        .into_iter()
        .find(|(_, value)| value["content"] == content)
        .unwrap_or_else(|| panic!("沒有條目 {content}"))
        .0
}

/// 內容依 uid 遞增排出來的清單。
fn contents_in_uid_order(root: &Path, world_id: &str) -> Vec<String> {
    raw_by_content(root, world_id)
        .into_iter()
        .map(|(_, value)| value["content"].as_str().unwrap().to_owned())
        .collect()
}

fn read_back(raw: &Value) -> WiEntry {
    from_world_file(raw.as_object().unwrap())
}

#[test]
fn v2_entries_are_stored_in_st_object_form() {
    let root = TestRoot::new("book-shape-convert");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let v2 = json!({
        "keys": ["月"], "secondary_keys": ["夜"], "comment": "月", "content": "@@activate\n月夜傳說",
        "constant": false, "enabled": true, "insertion_order": 7, "position": "after_char",
        "extensions": {
            "depth": 2, "probability": 40, "useProbability": true, "sticky": 3, "group": "g",
            "scan_depth": 5, "delay_until_recursion": true, "triggers": ["swipe"],
            "table_tavern": {"visibility": "public"}, "unknown_ext": {"keep": 1}
        }
    });
    let card = json!({"data": {"name": "莉亞", "character_book": {"entries": [v2.clone()]}}});
    import_character(
        root.path(),
        &world_id,
        card.to_string().as_bytes(),
        "#3366ff",
        LANG,
    )
    .unwrap();
    let (_, raw) = raw_by_content(root.path(), &world_id).remove(0);
    for gone in ["keys", "secondary_keys", "insertion_order", "enabled"] {
        assert!(raw.get(gone).is_none(), "{gone} 要改成物件形的欄名");
    }
    assert_eq!(raw["key"], json!(["月"]));
    assert_eq!(raw["keysecondary"], json!(["夜"]));
    assert_eq!(raw["order"], 7);
    assert_eq!(raw["disable"], false);
    assert_eq!(
        raw["selective"], false,
        "V2 缺 selective 明寫 false，不被物件形預設翻成 true"
    );
    assert_eq!(raw["position"], 1);
    assert_eq!(raw["depth"], 2);
    assert_eq!(raw["probability"], 40);
    assert_eq!(raw["sticky"], 3);
    assert_eq!(raw["scanDepth"], 5);
    assert_eq!(raw["delayUntilRecursion"], true);
    assert_eq!(
        raw["content"], "@@activate\n月夜傳說",
        "裝飾留在內文，由讀取端拆"
    );
    assert_eq!(
        raw["extensions"]["unknown_ext"],
        json!({"keep": 1}),
        "extensions 其餘鍵原樣"
    );
    assert_eq!(raw["extensions"]["depth"], 2);
    // 讀回的條目與網頁版直接讀 V2 的結果相同（可見度是桌面版寫進去的，不影響）
    let mut expected = from_character_book(v2.as_object().unwrap());
    let mut actual = read_back(&raw);
    expected.id.clear();
    actual.id.clear();
    assert_eq!(actual, expected);
    assert_eq!(actual.decorators, ["@@activate"]);
}

#[test]
fn missing_v2_fields_follow_the_author_rulings() {
    let root = TestRoot::new("book-shape-missing");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    // 缺 enabled＝啟用；缺 insertion_order 補 100（作者裁決 2026-10-10）
    let card = card_text(r#"[{"keys":["a"],"content":"缺欄位"}]"#);
    import_character(root.path(), &world_id, card.as_bytes(), "#3366ff", LANG).unwrap();
    let (_, raw) = raw_by_content(root.path(), &world_id).remove(0);
    assert_eq!(raw["disable"], false);
    assert_eq!(raw["order"], 100);
    assert_eq!(raw["selective"], false);
    let entry = read_back(&raw);
    assert_eq!(entry.order, 100.0);
    assert!(!entry.disable);
    // 再經一次 JSON 文字來回（真的落過檔）
    let text = std::fs::read_to_string(
        root.path()
            .join("worlds")
            .join(&world_id)
            .join("worldbook.json"),
    )
    .unwrap();
    let book: Value = serde_json::from_str(&text).unwrap();
    let stored = book["entries"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap();
    assert_eq!(read_back(stored).order, 100.0);
    // 物件形世界書檔缺 order 仍是 100
    let plain: Map<String, Value> = json!({"content": "x"}).as_object().unwrap().clone();
    assert_eq!(from_world_file(&plain).order, 100.0);
    // 明寫 null／字串的 insertion_order 不是數字，補 100
    let world_b = data::create_world(root.path(), "乙").unwrap();
    let card = card_text(
        r#"[{"keys":[],"content":"空值","enabled":true,"insertion_order":null},{"keys":[],"content":"字串","enabled":true,"insertion_order":"7"}]"#,
    );
    import_character(root.path(), &world_b, card.as_bytes(), "#3366ff", LANG).unwrap();
    let (_, null_entry) = raw_by_content(root.path(), &world_b).remove(0);
    assert_eq!(read_back(&null_entry).order, 100.0);
    let (_, string_entry) = raw_by_content(root.path(), &world_b).remove(1);
    assert_eq!(read_back(&string_entry).order, 100.0);
}

#[test]
fn array_form_uids_follow_st_load_order_and_duplicate_ids_keep_the_last() {
    let root = TestRoot::new("book-shape-array-order");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let card = card_text(
        r#"[
        {"id":30,"keys":[],"content":"id30","enabled":true},
        {"id":10,"keys":[],"content":"id10","enabled":true},
        {"id":20,"keys":[],"content":"id20舊","enabled":true},
        {"keys":[],"content":"無id在索引3","enabled":true},
        {"id":20,"keys":[],"content":"id20新","enabled":true},
        {"id":"x","keys":[],"content":"字串id","enabled":true}
    ]"#,
    );
    import_character(root.path(), &world_id, card.as_bytes(), "#3366ff", LANG).unwrap();
    // 整數鍵由小到大（3、10、20、30），其餘照出現順序；id 20 重複＝後一條蓋前一條、位置在前一條
    assert_eq!(
        contents_in_uid_order(root.path(), &world_id),
        ["無id在索引3", "id10", "id20新", "id30", "字串id"]
    );
    // 契約的 key（陣列索引）→ uid：被蓋掉的那條指向留下的
    let imported = super::card::import_character_placing(
        root.path(),
        &data::create_world(root.path(), "乙").unwrap(),
        card.as_bytes(),
        "#3366ff",
        LANG,
        super::card::BookWrite::Strict,
    )
    .unwrap();
    let placed = imported.book.unwrap().placed;
    let uid_for = |key: &str| placed.iter().find(|(k, _)| k == key).unwrap().1;
    assert_eq!(
        uid_for("2"),
        uid_for("4"),
        "重複 id 的舊條目映到留下的那一條"
    );
    assert_eq!(uid_for("3"), Some(0));
}

/// 物件形 `entries`（鍵 `zeta`、`alpha` 與整數鍵）：整數鍵由小到大在前，其餘照原檔出現順序。
const OBJECT_ENTRIES: &str = r#"{
    "zeta": {"uid": 90, "key": [], "content": "zeta", "comment": "z"},
    "10": {"uid": 91, "key": [], "content": "ten", "comment": "t"},
    "alpha": {"uid": 92, "key": [], "content": "alpha", "comment": "a"},
    "2": {"uid": 93, "key": [], "content": "two", "comment": "w"}
}"#;
const OBJECT_ORDER: [&str; 4] = ["two", "ten", "zeta", "alpha"];

#[test]
fn object_form_order_survives_every_import_entry() {
    // 角色卡路
    let root = TestRoot::new("book-shape-object-card");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    import_character(
        root.path(),
        &world_id,
        card_text(OBJECT_ENTRIES).as_bytes(),
        "#3366ff",
        LANG,
    )
    .unwrap();
    assert_eq!(contents_in_uid_order(root.path(), &world_id), OBJECT_ORDER);

    // 世界書路：卡檔（character_book）與獨立書檔（頂層 entries）
    let held = data::test_exclusive(&world_id);
    let world_b = data::create_world(root.path(), "乙").unwrap();
    let held_b = data::test_exclusive(&world_b);
    import_worldbook_file(
        root.path(),
        &world_b,
        card_text(OBJECT_ENTRIES).as_bytes(),
        "書",
        &held_b,
    )
    .unwrap();
    assert_eq!(contents_in_uid_order(root.path(), &world_b), OBJECT_ORDER);
    let world_c = data::create_world(root.path(), "丙").unwrap();
    let held_c = data::test_exclusive(&world_c);
    import_worldbook_file(
        root.path(),
        &world_c,
        format!(r#"{{"name":"書","entries":{OBJECT_ENTRIES}}}"#).as_bytes(),
        "書",
        &held_c,
    )
    .unwrap();
    assert_eq!(contents_in_uid_order(root.path(), &world_c), OBJECT_ORDER);
    drop((held, held_b, held_c));
}

#[test]
fn web_save_import_keeps_object_form_order() {
    let fixture: Value = serde_json::from_slice(
        &std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../src/shared/contracts/web-save/short.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let mut save = fixture;
    save["card"]["data"]["character_book"]["entries"] = json!("__ENTRIES__");
    save["world_info"]["entries"] = json!([]);
    save["world_info"]["timed"] = json!({});
    // 存檔文字照原樣帶條目鍵的順序（Value 會排序，所以用文字拼進去）
    let text = serde_json::to_string(&save)
        .unwrap()
        .replace("\"__ENTRIES__\"", OBJECT_ENTRIES);
    let root = TestRoot::new("book-shape-web-save");
    let imported = import_web_save(root.path(), text.as_bytes(), LANG).unwrap();
    assert_eq!(
        contents_in_uid_order(root.path(), &imported.world_id),
        OBJECT_ORDER
    );
}

#[test]
fn dedupe_counts_every_trigger_field() {
    let root = TestRoot::new("book-shape-dedupe");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let held = data::test_exclusive(&world_id);
    let entry = |extensions: Value| {
        json!({"keys": ["月"], "content": "月夜傳說", "enabled": true, "insertion_order": 5,
            "position": "before_char", "extensions": extensions})
    };
    let import = |entry: Value| {
        let card = json!({"data": {"name": "莉亞", "character_book": {"entries": [entry]}}});
        import_character_file(
            root.path(),
            &world_id,
            card.to_string().as_bytes(),
            "#3366ff",
            LANG,
            &held,
        )
        .unwrap()
        .book
        .unwrap()
    };
    assert_eq!(import(entry(json!({}))).imported, 1);
    // 同一條換個寫法：預設值明寫、false／0 與缺欄、數字 order 不同、停用狀態不同＝同一條
    let mut same = entry(
        json!({"probability": 100, "sticky": 0, "delay_until_recursion": false,
        "useProbability": true, "group": ""}),
    );
    same["insertion_order"] = json!(99);
    same["enabled"] = json!(false);
    same["selective"] = json!(false);
    assert_eq!(import(same).skipped, 1);
    // 影響觸發的欄位不同＝另一條
    for extensions in [
        json!({"probability": 50}),
        json!({"sticky": 2}),
        json!({"cooldown": 2}),
        json!({"delay": 2}),
        json!({"group": "g"}),
        json!({"depth": 1}),
        json!({"scan_depth": 3}),
        json!({"role": 1}),
        json!({"delay_until_recursion": true}),
        json!({"exclude_recursion": true}),
        json!({"prevent_recursion": true}),
        json!({"triggers": ["swipe"]}),
        json!({"ignore_budget": true}),
        json!({"outlet_name": "o"}),
        json!({"match_scenario": true}),
        json!({"case_sensitive": true}),
        json!({"match_whole_words": true}),
        json!({"use_group_scoring": true}),
        json!({"position": 4}),
    ] {
        assert_eq!(
            import(entry(extensions.clone())).imported,
            1,
            "{extensions}"
        );
    }
    let mut keys_reordered = entry(json!({}));
    keys_reordered["keys"] = json!(["月", "夜"]);
    assert_eq!(import(keys_reordered.clone()).imported, 1);
    keys_reordered["keys"] = json!(["夜", "月"]);
    assert_eq!(import(keys_reordered).imported, 1, "鍵的順序算指紋");
    let mut secondary = entry(json!({}));
    secondary["secondary_keys"] = json!(["夜"]);
    secondary["selective"] = json!(true);
    assert_eq!(import(secondary.clone()).imported, 1);
    secondary["selective"] = json!(false);
    assert_eq!(
        import(secondary.clone()).imported,
        1,
        "有次要鍵時 selective 算指紋"
    );
    secondary["selective"] = json!(true);
    secondary["extensions"] = json!({"selectiveLogic": 2});
    assert_eq!(
        import(secondary).imported,
        1,
        "有次要鍵時 selectiveLogic 算指紋"
    );
    let decorated = {
        let mut decorated = entry(json!({}));
        decorated["content"] = json!("@@activate\n月夜傳說");
        decorated
    };
    assert_eq!(import(decorated).imported, 1, "裝飾算指紋");
}

#[test]
fn v2_and_object_form_of_the_same_entry_dedupe_across_routes() {
    let root = TestRoot::new("book-shape-cross-route");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let held = data::test_exclusive(&world_id);
    let card = json!({"data": {"name": "莉亞", "character_book": {"entries": [{
        "keys": ["月"], "content": "月夜傳說", "enabled": true, "position": "before_char",
        "extensions": {"probability": 100, "sticky": null, "delay_until_recursion": false}
    }]}}});
    import_character(
        root.path(),
        &world_id,
        card.to_string().as_bytes(),
        "#3366ff",
        LANG,
    )
    .unwrap();
    let book = json!({"entries": {"0": {"uid": 0, "key": ["月"], "content": "月夜傳說",
        "disable": false, "delayUntilRecursion": 0, "sticky": 0}}});
    let imported = import_worldbook_file(
        root.path(),
        &world_id,
        book.to_string().as_bytes(),
        "書",
        &held,
    )
    .unwrap();
    assert_eq!(imported.value.imported, 0);
    assert_eq!(imported.value.skipped, 1);
}

fn exported_entries(root: &Path, world_id: &str, character_id: &str) -> Vec<Value> {
    let path = root.join(format!("{character_id}.json"));
    export_character(root, world_id, character_id, &path).unwrap();
    let value: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    value["data"]["character_book"]["entries"]
        .as_array()
        .cloned()
        .unwrap()
}

#[test]
fn export_folds_trigger_fields_back_and_carries_the_uid_as_id() {
    let root = TestRoot::new("book-shape-export");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let v2 = json!({
        "keys": ["月"], "secondary_keys": ["夜"], "comment": "月", "content": "月夜傳說",
        "constant": false, "selective": true, "enabled": true, "insertion_order": 7,
        "position": "after_char",
        "extensions": {"probability": 40, "sticky": 3, "group": "g", "scan_depth": 5,
            "delay_until_recursion": true, "triggers": ["swipe"], "case_sensitive": true,
            "match_whole_words": true}
    });
    let no_order = json!({"keys": [], "content": "沒有順序", "constant": true});
    let card =
        json!({"data": {"name": "莉亞", "character_book": {"entries": [v2.clone(), no_order]}}});
    let meta = import_character(
        root.path(),
        &world_id,
        card.to_string().as_bytes(),
        "#3366ff",
        LANG,
    )
    .unwrap();
    let exported = exported_entries(root.path(), &world_id, &meta.id);
    let moon = exported
        .iter()
        .find(|e| e["content"] == "月夜傳說")
        .unwrap();
    for stray in [
        "probability",
        "sticky",
        "group",
        "scanDepth",
        "delayUntilRecursion",
        "triggers",
        "caseSensitive",
    ] {
        assert!(moon.get(stray).is_none(), "{stray} 不留在頂層");
    }
    let ext = &moon["extensions"];
    assert_eq!(ext["probability"], 40);
    assert_eq!(ext["sticky"], 3);
    assert_eq!(ext["group"], "g");
    assert_eq!(ext["scan_depth"], 5);
    assert_eq!(ext["delay_until_recursion"], true);
    assert_eq!(ext["triggers"], json!(["swipe"]));
    assert_eq!(ext["case_sensitive"], true);
    assert_eq!(ext["match_whole_words"], true);
    assert_eq!(moon["insertion_order"], 7);
    assert_eq!(moon["position"], "after_char");
    // id＝uid，唯一
    let ids: Vec<u64> = exported.iter().map(|e| e["id"].as_u64().unwrap()).collect();
    let moon_uid = uid_of(root.path(), &world_id, "月夜傳說");
    assert_eq!(moon["id"], moon_uid);
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), ids.len());
    // 缺 insertion_order 的條目匯出時寫成補好的 100
    let bare = exported
        .iter()
        .find(|e| e["content"] == "沒有順序")
        .unwrap();
    assert_eq!(bare["insertion_order"], 100);
    assert!(bare["extensions"].get("table_tavern").is_none());

    // 匯出的卡再匯入新桌：每條讀出的 WiEntry 與原卡直接讀 V2 的結果相同（缺的 order 兩邊都是 100）
    let world_b = data::create_world(root.path(), "乙").unwrap();
    let path = root.path().join(format!("{}.json", meta.id));
    import_character(
        root.path(),
        &world_b,
        &std::fs::read(path).unwrap(),
        "#3366ff",
        LANG,
    )
    .unwrap();
    for source in [
        v2,
        json!({"keys": [], "content": "沒有順序", "constant": true}),
    ] {
        let content = source["content"].as_str().unwrap();
        let (_, raw) = raw_by_content(root.path(), &world_b)
            .into_iter()
            .find(|(_, value)| value["content"] == content)
            .unwrap();
        let mut expected = from_character_book(source.as_object().unwrap());
        let mut actual = read_back(&raw);
        expected.id.clear();
        actual.id.clear();
        assert_eq!(actual, expected, "{content}");
    }
}

#[test]
fn object_form_entries_round_trip_through_export() {
    let root = TestRoot::new("book-shape-object-export");
    let world_a = data::create_world(root.path(), "甲").unwrap();
    let meta = import_character(
        root.path(),
        &world_a,
        json!({"data": {"name": "莉亞"}}).to_string().as_bytes(),
        "#3366ff",
        LANG,
    )
    .unwrap();
    // 物件形世界書檔：缺 selective、缺 order、字串 order、小數 order
    let book = json!({"entries": {
        "0": {"uid": 0, "key": ["月"], "keysecondary": ["夜"], "comment": "缺欄", "content": "缺欄內容"},
        "1": {"uid": 1, "key": ["月"], "comment": "字串", "content": "字串內容", "order": "7"},
        "2": {"uid": 2, "key": ["月"], "comment": "小數", "content": "小數內容", "order": 7.5, "selective": false},
        "3": {"uid": 3, "key": ["月"], "keysecondary": ["夜"], "comment": "空值", "content": "空值內容", "selective": null, "constant": null}
    }});
    data::import_worldbook_as(
        root.path(),
        &world_a,
        &book.to_string(),
        &data::BookOwner::Character(meta.id.clone()),
    )
    .unwrap();
    let exported = exported_entries(root.path(), &world_a, &meta.id);
    let by = |content: &str| exported.iter().find(|e| e["content"] == content).unwrap();
    assert_eq!(by("缺欄內容")["selective"], true);
    assert_eq!(by("缺欄內容")["insertion_order"], 100);
    assert_eq!(by("字串內容")["insertion_order"], 100);
    assert_eq!(by("小數內容")["insertion_order"], 7.5);

    let world_b = data::create_world(root.path(), "乙").unwrap();
    let path = root.path().join(format!("{}.json", meta.id));
    import_character(
        root.path(),
        &world_b,
        &std::fs::read(path).unwrap(),
        "#3366ff",
        LANG,
    )
    .unwrap();
    assert_eq!(by("空值內容")["selective"], true);
    assert_eq!(by("空值內容")["constant"], false);
    for content in ["缺欄內容", "字串內容", "小數內容", "空值內容"] {
        let find = |world: &str| {
            raw_by_content(root.path(), world)
                .into_iter()
                .find(|(_, value)| value["content"] == content)
                .map(|(_, value)| read_back(&value))
                .unwrap()
        };
        let (mut before, mut after) = (find(&world_a), find(&world_b));
        before.id.clear();
        after.id.clear();
        // `false` 與 0 是同一個行為
        for entry in [&mut before, &mut after] {
            entry.delay_until_recursion =
                RecursionDelay::Level(entry.delay_until_recursion.level());
        }
        assert_eq!(before, after, "{content}：觸發欄位與排序值往返不變");
    }
}

#[test]
fn web_save_png_with_a_different_entry_order_is_not_used() {
    use base64::Engine;
    let mut save: Value = serde_json::from_slice(
        &std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../src/shared/contracts/web-save/short.json"),
        )
        .unwrap(),
    )
    .unwrap();
    save["card"]["data"]["character_book"]["entries"] = json!("__ENTRIES__");
    save["world_info"]["entries"] = json!([]);
    save["world_info"]["timed"] = json!({});
    let ordered = |first: &str, second: &str| {
        format!(
            r#"{{"{first}":{{"key":[],"content":"{first}"}},"{second}":{{"key":[],"content":"{second}"}}}}"#
        )
    };
    // PNG 內嵌的卡鍵順序是 a、z；存檔的 card 是 z、a。Value 相等，但 ST 載入順序不同
    let png_card = serde_json::to_string(&save["card"])
        .unwrap()
        .replace("\"__ENTRIES__\"", &ordered("a", "z"));
    let png = crate::import::test_support::card_png(&png_card);
    save["card_png"] = json!(base64::engine::general_purpose::STANDARD.encode(&png));
    let text = serde_json::to_string(&save)
        .unwrap()
        .replace("\"__ENTRIES__\"", &ordered("z", "a"));
    let root = TestRoot::new("book-shape-web-save-png");
    let imported = import_web_save(root.path(), text.as_bytes(), LANG).unwrap();
    assert_eq!(
        contents_in_uid_order(root.path(), &imported.world_id),
        ["z", "a"]
    );
}

#[test]
fn editor_view_of_order_follows_the_read_side_rule() {
    let root = TestRoot::new("book-shape-editor-order");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let book = json!({"entries": {
        "0": {"uid": 0, "key": [], "content": "缺", "constant": true},
        "1": {"uid": 1, "key": [], "content": "字串", "constant": true, "order": "7"},
        "2": {"uid": 2, "key": [], "content": "小數", "constant": true, "order": 7.5}
    }});
    data::import_worldbook_as(
        root.path(),
        &world_id,
        &book.to_string(),
        &data::BookOwner::Gm,
    )
    .unwrap();
    let view = || {
        data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .into_iter()
            .map(|entry| (entry.content.clone(), entry.order))
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    assert_eq!(view()["缺"], 100);
    assert_eq!(view()["字串"], 100);
    assert_eq!(view()["小數"], 8);
    // 編輯器原樣存回（沒改 order）不動原值；改了才寫新值
    let mut entries = data::read_worldbook(root.path(), &world_id).unwrap();
    for entry in &mut entries {
        data::upsert_worldbook_entry(root.path(), &world_id, entry.clone()).unwrap();
    }
    let raw = data::read_worldbook_raw(root.path(), &world_id).unwrap();
    assert_eq!(raw[&2]["order"], 7.5);
    assert_eq!(raw[&1]["order"], "7");
    assert!(raw[&0].get("order").is_none());
    entries[2].order = 9;
    data::upsert_worldbook_entry(root.path(), &world_id, entries[2].clone()).unwrap();
    assert_eq!(
        data::read_worldbook_raw(root.path(), &world_id).unwrap()[&2]["order"],
        9
    );
}

#[test]
fn book_order_check_covers_every_book_shape() {
    use super::card::book_object_key_order;
    let order = |text: &str| book_object_key_order(text.as_bytes());
    let entries = |first: &str, second: &str| {
        format!(r#"{{"{first}":{{"key":[],"content":"1"}},"{second}":{{"key":[],"content":"2"}}}}"#)
    };
    let shapes: [fn(&str) -> String; 3] = [
        |e| format!(r#"{{"data":{{"name":"甲","character_book":{{"entries":{e}}}}}}}"#),
        |e| format!(r#"{{"data":{{"name":"甲","entries":{e}}}}}"#),
        |e| format!(r#"{{"name":"甲","entries":{e}}}"#),
    ];
    for shape in shapes {
        let az = order(&shape(&entries("a", "z")));
        let za = order(&shape(&entries("z", "a")));
        assert_eq!(az, Some(vec!["a".to_owned(), "z".to_owned()]));
        assert_eq!(za, Some(vec!["z".to_owned(), "a".to_owned()]));
    }
}
