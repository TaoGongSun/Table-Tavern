use super::*;
use crate::data::{StateNode, Tier};
use crate::import::card_io::{crc32, decode_png_character};
use crate::import::images::{gm_image, save_gm_image};
use crate::import::import_character;
use crate::import::mechanism::import_card_extension;
use crate::import::test_support::TestRoot;
use std::collections::BTreeMap;

/// 匯出→再匯入要拿回一模一樣的內容：公開五段照原欄位歸位，私有條目回到 character_book
#[test]
fn exported_png_reimports_with_identical_content() {
    let root = TestRoot::new("export-png");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let raw = r#"{"data":{"name":"莉亞","description":"精靈遊俠","personality":"冷靜","scenario":"雨夜","first_mes":"妳來了。","mes_example":"<START>","character_book":{"entries":[{"keys":["森林","月亮"],"content":"古老盟約"}]}}}"#;
    let source = import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff").unwrap();
    let target = root.path().join("莉亞.png");

    export_character(root.path(), &world_id, &source.id, &target).unwrap();

    let exported = fs::read(&target).unwrap();
    let value: Value = serde_json::from_slice(&decode_png_character(&exported).unwrap()).unwrap();
    assert_eq!(value["spec"], "chara_card_v2");
    assert_eq!(value["name"], "莉亞");
    assert_eq!(value["data"]["personality"], "冷靜");
    assert_eq!(
        value["data"]["character_book"]["entries"][0]["keys"][1],
        "月亮"
    );
    assert_eq!(
        value["data"]["character_book"]["entries"][0]["content"],
        "古老盟約"
    );

    let round_trip = import_character(root.path(), &world_id, &exported, "#000000").unwrap();
    assert_eq!(
        data::read_character(root.path(), &world_id, &round_trip.id)
            .unwrap()
            .public_md,
        data::read_character(root.path(), &world_id, &source.id)
            .unwrap()
            .public_md
    );
    assert_eq!(
        data::read_character(root.path(), &world_id, &round_trip.id)
            .unwrap()
            .private_md,
        "- **森林、月亮**：古老盟約"
    );
}

#[test]
fn character_export_import_round_trips_rules_and_initial_tree() {
    let root = TestRoot::new("mechanism-round-trip");
    let source_world = data::create_world(root.path(), "來源桌").unwrap();
    let source = import_character(
        root.path(),
        &source_world,
        r#"{"data":{"name":"亞瑟","description":"騎士"}}"#.as_bytes(),
        "#3366ff",
    )
    .unwrap();
    let mut state = data::read_state(root.path(), &source_world).unwrap();
    let mut rule = data::FieldRule::for_kind(data::FieldKind::Pair);
    rule.branch = Some("亞瑟".to_owned());
    state
        .mechanism
        .rules
        .insert("亞瑟.能力.HP".to_owned(), rule);
    let initial = StateNode::Branch(BTreeMap::from([(
        "能力".to_owned(),
        StateNode::Branch(BTreeMap::from([(
            "HP".to_owned(),
            StateNode::Leaf("50/50".to_owned()),
        )])),
    )]));
    state.state.tree.insert("亞瑟".to_owned(), initial.clone());
    data::write_state(root.path(), &source_world, &state).unwrap();

    let export_path = root.path().join("亞瑟.json");
    export_character(root.path(), &source_world, &source.id, &export_path).unwrap();
    let exported = fs::read(&export_path).unwrap();
    let target_world = data::create_world(root.path(), "目標桌").unwrap();
    import_character(root.path(), &target_world, &exported, "#000000").unwrap();
    let target = data::read_state(root.path(), &target_world).unwrap();
    assert_eq!(
        target.mechanism.rules["亞瑟.能力.HP"].branch.as_deref(),
        Some("亞瑟")
    );
    assert_eq!(target.state.tree["亞瑟"], initial);

    // 同一份匯出檔改用世界書身分匯入，機制照樣要收進來（兩條路徑對等）
    let book_world = data::create_world(root.path(), "世界書桌").unwrap();
    import_card_extension(root.path(), &book_world, "亞瑟", &exported);
    let book_state = data::read_state(root.path(), &book_world).unwrap();
    assert_eq!(
        book_state.mechanism.rules["亞瑟.能力.HP"].branch.as_deref(),
        Some("亞瑟")
    );
    assert_eq!(book_state.state.tree["亞瑟"], initial);
}

/// App 內手寫的卡沒有那五個標題，全部併進 description；私有筆記進常駐條目
#[test]
fn exports_freeform_card_as_json() {
    let root = TestRoot::new("export-json");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let id = data::new_id();
    data::write_character(
        root.path(),
        &world_id,
        &CharacterCard {
            id: id.clone(),
            name: "凱恩".to_owned(),
            color: "#111111".to_owned(),
            avatar: "🎭".to_owned(),
            tier: Tier::Balanced,
            show_image: true,
            archived: false,
            gen_prompt: String::new(),
            public_md: "騎士，話少。\n### 開場白\n有事？".to_owned(),
            private_md: "其實是逃兵".to_owned(),
        },
    )
    .unwrap();
    let target = root.path().join("凱恩.json");

    export_character(root.path(), &world_id, &id, &target).unwrap();

    let value: Value = serde_json::from_slice(&fs::read(&target).unwrap()).unwrap();
    assert_eq!(value["data"]["description"], "騎士，話少。");
    assert_eq!(value["data"]["first_mes"], "有事？");
    let entry = &value["data"]["character_book"]["entries"][0];
    assert_eq!(entry["content"], "其實是逃兵");
    assert_eq!(entry["constant"], true);
}

/// 沒有圖的卡也匯得出 PNG：底圖是自己組的 1×1，chunk 長度與 CRC 都要對
#[test]
fn blank_png_is_a_valid_png_container() {
    let png = blank_png();
    assert!(png.starts_with(PNG_MAGIC));
    let mut offset = PNG_MAGIC.len();
    let mut kinds = Vec::new();
    while offset < png.len() {
        let length = u32::from_be_bytes(png[offset..offset + 4].try_into().unwrap()) as usize;
        let crc_at = offset + 8 + length;
        assert_eq!(
            u32::from_be_bytes(png[crc_at..crc_at + 4].try_into().unwrap()),
            crc32(&png[offset + 4..crc_at])
        );
        kinds.push(String::from_utf8_lossy(&png[offset + 4..offset + 8]).into_owned());
        offset = crc_at + 4;
    }
    assert_eq!(kinds, ["IHDR", "IDAT", "IEND"]);
    assert_eq!(offset, png.len());
}

/// 底圖若還留著匯入當下那份 chara，匯出後必須被換掉而不是疊上去
#[test]
fn export_replaces_stale_chara_chunk_in_the_image() {
    let root = TestRoot::new("export-stale");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let mut base = blank_png();
    base.truncate(base.len() - 12); // 拆掉 IEND，插入舊 chara 後再補回
    let stale = format!(
        "chara\0{}",
        base64_encode(r#"{"data":{"name":"舊名"}}"#.as_bytes())
    );
    base.extend_from_slice(&png_chunk(b"tEXt", stale.as_bytes()));
    base.extend_from_slice(&png_chunk(b"IEND", &[]));
    let meta = import_character(root.path(), &world_id, &base, "#111111").unwrap();
    let mut card = data::read_character(root.path(), &world_id, &meta.id).unwrap();
    card.name = "新名".to_owned();
    data::write_character(root.path(), &world_id, &card).unwrap();
    let target = root.path().join("新名.png");

    export_character(root.path(), &world_id, &meta.id, &target).unwrap();

    let exported = fs::read(&target).unwrap();
    let value: Value = serde_json::from_slice(&decode_png_character(&exported).unwrap()).unwrap();
    assert_eq!(value["name"], "新名");
    assert_eq!(
        exported
            .windows(6)
            .filter(|window| *window == b"chara\0")
            .count(),
        1
    );
}

/// GM 卡的圖：匯的是 PNG 卡就整張存起來，純 JSON 世界書不存也不刪舊圖
/// （換書不該讓 GM 卡突然變回內建書本圖）。
#[test]
fn save_gm_image_stores_png_and_keeps_it_for_plain_json() {
    let root = TestRoot::new("gm-image");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    // 還沒匯過 PNG：沒有圖，前端據此回退內建書本圖
    assert_eq!(gm_image(root.path(), &world_id).unwrap(), None);

    let png = embed_chara_chunk(&blank_png(), r#"{"name":"莉亞"}"#.as_bytes()).unwrap();
    assert!(save_gm_image(root.path(), &world_id, &png));
    let stored = fs::read(data::gm_image_path(root.path(), &world_id).unwrap()).unwrap();
    assert_eq!(stored, png);
    assert_eq!(
        gm_image(root.path(), &world_id).unwrap(),
        Some(base64_encode(&png))
    );

    assert!(!save_gm_image(
        root.path(),
        &world_id,
        r#"{"entries":{"0":{"uid":0,"key":["龍"],"content":"沉睡"}}}"#.as_bytes()
    ));
    assert_eq!(
        gm_image(root.path(), &world_id).unwrap(),
        Some(base64_encode(&png))
    );
}
