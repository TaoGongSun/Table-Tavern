//! card.rs 的單元測試：V1／V2／PNG 卡解析、隨身世界書、探測與世界書路轉換。
use super::*;
use crate::import::test_support::{card_png, minimal_png, TestRoot};
use std::fs;

#[test]
fn imports_v2_json_and_preserves_original() {
    let root = TestRoot::new("v2");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let raw = r#"{"spec":"chara_card_v2","spec_version":"2.0","data":{"name":"莉亞","description":"精靈遊俠","personality":"冷靜","scenario":"雨夜","first_mes":"妳來了。","mes_example":"<START>","character_book":{"entries":[{"keys":["森林","月亮"],"content":"古老盟約","enabled":true},{"keys":["略過"],"content":""}]}}}"#.as_bytes();

    let meta = import_character(root.path(), &world_id, raw, "#3366ff", "zh-TW").unwrap();
    assert_eq!(meta.name, "莉亞");
    let markdown = fs::read_to_string(
        root.path()
            .join(format!("worlds/{world_id}/characters/{}.md", meta.id)),
    )
    .unwrap();
    assert!(markdown.contains(&format!(
        "---\nid: {}\nname: 莉亞\ncolor: #3366ff\navatar: 🎭\ntier: balanced\nshow_image: true\narchived: false\nauto_hidden: false\ndisplay_index: 0\ngen_prompt: \n---",
        meta.id
    )));
    for section in [
        "### 簡介\n精靈遊俠",
        "### 人格與語氣\n冷靜",
        "### 場景\n雨夜",
        "### 開場白\n妳來了。",
        "### 語氣範例\n<START>",
    ] {
        assert!(markdown.contains(section), "missing {section}");
    }
    assert!(!markdown.contains("古老盟約"), "條目不傾印進私設");
    assert_eq!(
        fs::read(root.path().join(format!(
            "worlds/{world_id}/characters/{}.import.json",
            meta.id
        )))
        .unwrap(),
        raw
    );
}

#[test]
fn imports_png_text_chunk_and_preserves_original() {
    let root = TestRoot::new("png");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let png = card_png(r#"{"data":{"name":"凱恩","description":"騎士"}}"#);

    let meta = import_character(root.path(), &world_id, &png, "#111111", "zh-TW").unwrap();
    assert!(root
        .path()
        .join(format!("worlds/{world_id}/characters/{}.md", meta.id))
        .is_file());
    let file = |extension: &str| {
        root.path().join(format!(
            "worlds/{world_id}/characters/{}.{extension}",
            meta.id
        ))
    };
    assert_eq!(fs::read(file("card.png")).unwrap(), png);
    assert_eq!(
        fs::read(file("png")).unwrap(),
        png_clean::strip_card_text(&png).unwrap()
    );
    assert_eq!(
        data::list_characters(root.path(), &world_id).unwrap().len(),
        1
    );
}

#[test]
fn imports_v1_top_level_fields() {
    let root = TestRoot::new("v1");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let meta = import_character(
        root.path(),
        &world_id,
        r#"{"name":"舊卡","personality":"直率"}"#.as_bytes(),
        "#222222",
        "zh-TW",
    )
    .unwrap();
    let card = data::read_character(root.path(), &world_id, &meta.id).unwrap();
    assert_eq!(card.public_md, "### 人格與語氣\n直率");
}

#[test]
fn probe_ignores_invalid_bytes() {
    assert_eq!(probe_import(b"not a card"), ImportProbe::default());
}

#[test]
fn probe_identifies_lorebook_heavy_cards() {
    let entries = json!([{"content":"一"}, {"content":"二"}, {"content":"三"}]);
    let sparse = probe_import(
        json!({"data":{"character_book":{"entries":entries},"description":" ","personality":"","scenario":""}})
            .to_string()
            .as_bytes(),
    );
    assert!(sparse.lorebook_heavy);

    // 西幻真卡的比例：人設 988 字、世界書 21,678 字。人設不算短，重心仍壓倒性在世界書
    let simulator = probe_import(
        json!({"data":{
            "character_book":{"entries":[
                {"content":"世".repeat(7000)},
                {"content":"界".repeat(7000)},
                {"content":"書".repeat(7678)},
            ]},
            "description":"長".repeat(988),
        }})
        .to_string()
        .as_bytes(),
    );
    assert!(simulator.lorebook_heavy);

    // 一般角色卡：帶著自己的隨身設定，但重心還在人設上
    let character = probe_import(
        json!({"data":{
            "character_book":{"entries":[
                {"content":"故鄉".repeat(200)},
                {"content":"家人".repeat(200)},
                {"content":"秘密".repeat(200)},
            ]},
            "description":"人".repeat(2000),
            "mes_example":"例".repeat(1000),
        }})
        .to_string()
        .as_bytes(),
    );
    assert!(!character.lorebook_heavy);

    let detailed = probe_import(
        json!({"data":{"character_book":{"entries":[{}, {}, {}]},"description":"長".repeat(300)}})
            .to_string()
            .as_bytes(),
    );
    assert!(!detailed.lorebook_heavy);
}

/// 前端匯入分流靠 parsed／name／book_entries 三個欄位判斷純世界書、純角色卡、
/// 兩種身分都有料、格式錯誤這四種情況，四種都要顧到。
#[test]
fn probe_reports_parsed_state_and_book_entry_count() {
    // 純世界書 JSON：頂層就是書本體（entries 在最外層），沒有 name 欄位
    let worldbook = probe_import(
        json!({"entries": [{"keys": ["森林"], "content": "古老盟約"}]})
            .to_string()
            .as_bytes(),
    );
    assert!(worldbook.parsed);
    assert_eq!(worldbook.name, None);
    assert!(worldbook.book_shaped);

    // V2 獨立世界書 JSON：頂層是書本體，自帶 name（書名）＋entries——有 name 也是世界書
    let named_book = probe_import(
        json!({"name": "北境設定集", "entries": [{"keys": ["漁村"], "content": "北境的漁村"}]})
            .to_string()
            .as_bytes(),
    );
    assert!(named_book.parsed);
    assert_eq!(named_book.name.as_deref(), Some("北境設定集"));
    assert!(named_book.book_shaped);

    // 損毀 JSON：解析不了，parsed 維持 false（沿用原本走角色路徑報格式錯誤那條）
    let broken = probe_import(b"{not json");
    assert!(!broken.parsed);

    // 有 name 但沒有 character_book：純角色卡
    let character_only = probe_import(
        json!({"data": {"name": "莉亞", "description": "精靈遊俠"}})
            .to_string()
            .as_bytes(),
    );
    assert!(character_only.parsed);
    assert_eq!(character_only.name.as_deref(), Some("莉亞"));
    assert_eq!(character_only.book_entries, 0);
    assert!(!character_only.book_shaped);
    assert_eq!(character_only.alternate_greetings, 0);

    // 情境卡：零條目、人設全塞在 description，靠備用開場白數認出來（實例 furry-male-scenarios）
    let scenarios = probe_import(
        json!({"data": {
            "name": "Furry male Scenarios",
            "description": "{{char}} is not a person but a scenario",
            "first_mes": "開場",
            "alternate_greetings": ["公車站", "海灘", "廚房"],
        }})
        .to_string()
        .as_bytes(),
    );
    assert_eq!(scenarios.book_entries, 0);
    assert!(!scenarios.lorebook_heavy);
    assert_eq!(scenarios.alternate_greetings, 3);

    // 有 name 也帶 character_book：角色與世界書兩種身分都有料
    let both = probe_import(
        json!({"data": {
            "name": "薇拉",
            "character_book": {"entries": [{"content": "北境的漁村"}, {"content": "雙親早逝"}]},
        }})
        .to_string()
        .as_bytes(),
    );
    assert!(both.parsed);
    assert_eq!(both.book_entries, 2);
}

#[test]
fn character_card_brings_its_own_lorebook_entries_to_the_table() {
    let card = r#"{"data":{"name":"薇拉","description":"北境來的斥候","character_book":{"entries":[{"keys":["故鄉"],"content":"北境的漁村","comment":"故鄉"},{"keys":["家人"],"content":"雙親早逝","comment":"家人"}]}}}"#;
    let root = TestRoot::new("character-lorebook");
    let world_id = data::create_world(root.path(), "酒館").unwrap();

    import_character(root.path(), &world_id, card.as_bytes(), "#ffffff", "zh-TW").unwrap();

    let entries = data::read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().any(|entry| entry.content == "北境的漁村"));

    // 同一張卡再匯一次：條目由去重擋下，不會長出第二份
    import_character(root.path(), &world_id, card.as_bytes(), "#ffffff", "zh-TW").unwrap();
    assert_eq!(
        data::read_worldbook(root.path(), &world_id).unwrap().len(),
        2
    );
}

#[test]
fn imports_alternate_greetings_into_private_markdown() {
    let root = TestRoot::new("alternate-greetings");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let raw = r#"{"data":{"name":"莉亞","character_book":{"entries":[{"keys":["森林"],"content":"古老盟約"}]},"alternate_greetings":["第二次見面。","雨天再訪。"]}}"#;

    let meta =
        import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff", "zh-TW").unwrap();
    let private_md = data::read_character(root.path(), &world_id, &meta.id)
        .unwrap()
        .private_md;
    for expected in [
        "### 備用開場白 1",
        "第二次見面。",
        "### 備用開場白 2",
        "雨天再訪。",
    ] {
        assert!(private_md.contains(expected), "missing {expected}");
    }
    // 卡內世界書條目不再傾印進私設：進這桌世界書、給這張卡的角色看
    assert!(!private_md.contains("古老盟約"), "{private_md}");
    let entries = data::read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].visibility,
        data::Visibility::Characters(vec![meta.id.clone()])
    );
}

/// 測試清單 #12：匯入 ST 角色卡產生新 id、name 照原值（含原本會被擋的字元）；
/// 同名再匯入一次也會成功，各自拿到不同 id 且互不影響
#[test]
fn importing_the_same_name_twice_mints_distinct_ids_and_keeps_first_card_intact() {
    let root = TestRoot::new("duplicate-name");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let odd_name = r#"{"name":"a/b/../重名","description":"第一張"}"#;
    let first = import_character(
        root.path(),
        &world_id,
        odd_name.as_bytes(),
        "#000000",
        "zh-TW",
    )
    .unwrap();
    assert_eq!(first.name, "a/b/../重名");

    let second = import_character(
        root.path(),
        &world_id,
        r#"{"name":"a/b/../重名","description":"第二張"}"#.as_bytes(),
        "#ffffff",
        "zh-TW",
    )
    .unwrap();

    assert_ne!(first.id, second.id);
    assert_eq!(
        data::read_character(root.path(), &world_id, &first.id)
            .unwrap()
            .public_md,
        "### 簡介\n第一張"
    );
    assert_eq!(
        data::read_character(root.path(), &world_id, &second.id)
            .unwrap()
            .public_md,
        "### 簡介\n第二張"
    );
    assert_eq!(
        data::list_characters(root.path(), &world_id).unwrap().len(),
        2
    );
}

/// 世界書卡不會建成角色，開場白仍須保留給前端選擇。
#[test]
fn card_openings_reads_all_greetings_from_import_bytes() {
    let card = r#"{"spec":"chara_card_v3","data":{"name":"兽人的洞穴","first_mes":" 夜色落下。 ","alternate_greetings":["另一個開場。","  ","最後一個開場。"],"character_book":{"entries":[]}}}"#;
    assert_eq!(
        card_openings(card.as_bytes()),
        Some((
            "兽人的洞穴".to_owned(),
            vec![
                "夜色落下。".to_owned(),
                "另一個開場。".to_owned(),
                "最後一個開場。".to_owned(),
            ],
        ))
    );
    // PNG 卡與 JSON 卡必須走同一條解析路徑。
    assert_eq!(
        card_openings(&minimal_png(card)),
        Some((
            "兽人的洞穴".to_owned(),
            vec![
                "夜色落下。".to_owned(),
                "另一個開場。".to_owned(),
                "最後一個開場。".to_owned(),
            ],
        ))
    );
    // 有些卡只把真正開場寫在替代開場白。
    assert_eq!(
        card_openings(
            r#"{"data":{"name":"莉亞","first_mes":"  ","alternate_greetings":["在這裡。"]}}"#
                .as_bytes(),
        ),
        Some(("莉亞".to_owned(), vec!["在這裡。".to_owned()]))
    );
    // 完全沒有可顯示的開場白時，前端不應開啟選擇面板。
    assert_eq!(
        card_openings(r#"{"data":{"name":"莉亞"}}"#.as_bytes()),
        None
    );
    assert_eq!(card_openings(b"not a card"), None);
}

/// 世界書卡（PNG 或 JSON 的假角色卡）要能整包匯進世界書；keys 為 null 的常駐條目不能炸
#[test]
fn worldbook_json_unwraps_lorebook_cards() {
    let card = r#"{"spec":"chara_card_v3","spec_version":"3.0","data":{"name":"根源重塑","character_book":{"name":"根源重塑","entries":[{"keys":["森林"],"content":"古老盟約","comment":"盟約","enabled":true,"insertion_order":3},{"keys":null,"constant":true,"content":"世界觀常駐","comment":"世界觀","enabled":true}]}}}"#;

    let root = TestRoot::new("worldbook-card");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    // PNG 卡與純 JSON 卡走同一條路，各匯一次；同一份書第二次全被當重複略過
    let png = minimal_png(card);
    let mut results = Vec::new();
    for bytes in [png.as_slice(), card.as_bytes()] {
        let json = worldbook_json(bytes).unwrap();
        results.push(data::import_worldbook(root.path(), &world_id, &json).unwrap());
    }
    assert_eq!(results[0].imported, 2);
    assert_eq!(
        results[1],
        data::WorldbookImport {
            imported: 0,
            skipped: 2
        }
    );
    let entries = data::read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].title, "盟約");
    assert_eq!(entries[0].keys, ["森林"]);
    assert_eq!(entries[0].order, 3);
    assert!(entries[1].constant);
    assert!(entries[1].keys.is_empty());
    assert_eq!(entries[1].content, "世界觀常駐");

    // 不是卡的一般世界書 JSON 原樣通過
    let plain = r#"{"entries":{"0":{"uid":0,"key":["龍"],"content":"沉睡"}}}"#;
    let round_trip: Value =
        serde_json::from_str(&worldbook_json(plain.as_bytes()).unwrap()).unwrap();
    assert_eq!(round_trip, serde_json::from_str::<Value>(plain).unwrap());
}

/// 物件形 entries（ST 獨立世界書格式塞進 character_book）一律算有條目、照 uid 鍵展開，
/// 不能掉進人設欄轉換（卡片契約）
#[test]
fn object_shaped_character_book_counts_as_entries() {
    let card = json!({"data": {
        "name": "北境驛站",
        "description": "短介紹",
        "character_book": {"name": "北境", "entries": {
            "10": {"uid": 10, "key": ["驛站"], "content": "驛".repeat(40)},
            "2": {"uid": 2, "key": ["王府"], "content": "府".repeat(40)},
            "0": {"uid": 0, "key": [], "content": "常駐", "constant": true},
        }},
    }})
    .to_string();

    let probe = probe_import(card.as_bytes());
    assert_eq!(probe.book_entries, 3);
    assert!(probe.lorebook_heavy);

    let book: Value = serde_json::from_str(&worldbook_json(card.as_bytes()).unwrap()).unwrap();
    assert_eq!(book["name"], "北境");
    let uids: Vec<_> = book_entry_values(&book["entries"])
        .iter()
        .map(|entry| entry["uid"].as_u64().unwrap())
        .collect();
    assert_eq!(uids, [0, 2, 10]);

    let root = TestRoot::new("object-book");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let json = worldbook_json(card.as_bytes()).unwrap();
    assert_eq!(
        data::import_worldbook(root.path(), &world_id, &json)
            .unwrap()
            .imported,
        3
    );
    // 落地順序與新配的 UID 照原鍵 0、2、10，不是字串序 0、10、2
    let landed: Vec<_> = data::read_worldbook(root.path(), &world_id)
        .unwrap()
        .into_iter()
        .map(|entry| (entry.uid, entry.content.chars().next().unwrap()))
        .collect();
    assert_eq!(landed, [(0, '常'), (1, '府'), (2, '驛')]);
}

/// 不是物件的條目值（字串、null）不算條目、匯入時略過，整本不會半路失敗（卡片契約）
#[test]
fn non_object_entry_values_are_skipped_not_fatal() {
    let card = json!({"data": {
        "name": "壞值卡",
        "character_book": {"entries": {"0": {"key": ["甲"], "content": "有效"}, "1": "字串", "2": null}},
    }})
    .to_string();
    assert_eq!(probe_import(card.as_bytes()).book_entries, 1);
    let root = TestRoot::new("non-object-entries");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let json = worldbook_json(card.as_bytes()).unwrap();
    let result = data::import_worldbook(root.path(), &world_id, &json).unwrap();
    assert_eq!(result.imported, 1);
    let array_book =
        json!({"entries": [{"keys": ["乙"], "content": "陣列有效"}, "字串", null]}).to_string();
    assert_eq!(
        data::import_worldbook(root.path(), &world_id, &array_book)
            .unwrap()
            .imported,
        1
    );
    assert_eq!(
        data::read_worldbook(root.path(), &world_id).unwrap().len(),
        2
    );
}

/// 世界書內容寫在人設欄、character_book 空著的卡：轉成一條沒關鍵字的常駐條目才進得來
#[test]
fn worldbook_json_converts_persona_fields_when_card_has_no_entries() {
    let card = json!({"spec": "chara_card_v2", "spec_version": "2.0", "data": {
        "name": "Furry male Scenarios",
        "description": "{{char}} is not a person but a scenario",
        "personality": "",
        "scenario": "毛毛大陸",
        "first_mes": "開場白不該進條目",
        "alternate_greetings": ["公車站", "海灘"],
    }})
    .to_string();

    let root = TestRoot::new("persona-as-book");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let json = worldbook_json(card.as_bytes()).unwrap();
    assert_eq!(
        data::import_worldbook(root.path(), &world_id, &json).unwrap(),
        data::WorldbookImport {
            imported: 1,
            skipped: 0
        }
    );
    let entries = data::read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].title, "Furry male Scenarios");
    assert!(entries[0].constant);
    assert!(entries[0].keys.is_empty());
    // 非空人設欄照 PERSONA_FIELDS 順序併起來，開場白留給 card_openings
    assert_eq!(
        entries[0].content,
        "{{char}} is not a person but a scenario\n\n毛毛大陸"
    );
    assert!(!entries[0].content.contains("開場白不該進條目"));

    // character_book 在但條目是空陣列：一樣走轉換，不會匯進一本空書
    let empty_book = json!({"data": {
        "name": "空書卡", "description": "設定都在這裡", "character_book": {"entries": []},
    }})
    .to_string();
    let converted: Value =
        serde_json::from_str(&worldbook_json(empty_book.as_bytes()).unwrap()).unwrap();
    assert_eq!(converted["entries"][0]["content"], "設定都在這裡");

    // 人設欄全空＝真的沒東西可匯，明講而不是靜默塞一本空書
    let hollow = json!({"data": {"name": "空殼", "first_mes": "只有開場白"}}).to_string();
    assert!(worldbook_json(hollow.as_bytes()).is_err());
}
