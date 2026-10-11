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
    let source =
        import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff", "zh-TW").unwrap();
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

    let round_trip =
        import_character(root.path(), &world_id, &exported, "#000000", "zh-TW").unwrap();
    assert_eq!(
        data::read_character(root.path(), &world_id, &round_trip.id)
            .unwrap()
            .public_md,
        data::read_character(root.path(), &world_id, &source.id)
            .unwrap()
            .public_md
    );
    // 條目回到世界書（同一桌去重合併，兩張卡都看得到），私設不再傾印條目
    assert_eq!(
        data::read_character(root.path(), &world_id, &round_trip.id)
            .unwrap()
            .private_md,
        ""
    );
    let entries = data::read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].visibility,
        data::Visibility::Characters(vec![source.id.clone(), round_trip.id.clone()])
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
        "zh-TW",
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
    import_character(root.path(), &target_world, &exported, "#000000", "zh-TW").unwrap();
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
    let meta = import_character(root.path(), &world_id, &base, "#111111", "zh-TW").unwrap();
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
    assert_eq!(
        save_gm_image(root.path(), &world_id, &png).unwrap(),
        crate::import::GmImage::Saved
    );
    let stored = fs::read(data::gm_image_path(root.path(), &world_id).unwrap()).unwrap();
    assert_eq!(stored, png);
    assert_eq!(
        gm_image(root.path(), &world_id).unwrap(),
        Some(base64_encode(&png))
    );

    assert_eq!(
        save_gm_image(
            root.path(),
            &world_id,
            r#"{"entries":{"0":{"uid":0,"key":["龍"],"content":"沉睡"}}}"#.as_bytes()
        )
        .unwrap(),
        crate::import::GmImage::NotPng
    );
    assert_eq!(
        gm_image(root.path(), &world_id).unwrap(),
        Some(base64_encode(&png))
    );
}

const FIELDS: [&str; 5] = [
    "description",
    "personality",
    "scenario",
    "first_mes",
    "mes_example",
];

/// 十語系各自匯入→匯出，五欄都拆回原欄位；備用開場白段標照介面語系。
#[test]
fn every_language_imports_and_exports_back_into_the_same_fields() {
    let root = TestRoot::new("export-langs");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let raw = r#"{"data":{"name":"莉亞","description":"甲","personality":"乙","scenario":"丙","first_mes":"丁","mes_example":"戊","alternate_greetings":["己"]}}"#;
    for lang in [
        "zh-TW", "zh-CN", "en", "ja", "ko", "es", "pt-BR", "de", "fr", "ru",
    ] {
        let meta =
            import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff", lang).unwrap();
        let card = data::read_character(root.path(), &world_id, &meta.id).unwrap();
        let value = character_card_v2(root.path(), &world_id, &card).unwrap();
        for (field, expected) in FIELDS.into_iter().zip(["甲", "乙", "丙", "丁", "戊"]) {
            assert_eq!(value["data"][field], expected, "{lang} {field}");
        }
        assert!(!card.private_md.contains("{n}"), "{lang}");
        assert!(
            card.private_md.contains(" 1\n己"),
            "{lang}: {}",
            card.private_md
        );
    }
    let meta = import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff", "de").unwrap();
    let card = data::read_character(root.path(), &world_id, &meta.id).unwrap();
    assert!(card.public_md.starts_with("### Beschreibung\n甲"));
    assert!(card.private_md.contains("### Alternative Begrüßung 1\n己"));
}

/// 混語系段標（含玩家手動編輯）逐段辨識；圍欄內的段標不切；行尾空白容忍。
#[test]
fn mixed_language_headings_split_and_fences_are_respected() {
    let markdown = "前言\n### Personality\n乙\n### 場景  \n丙\n```md\n### 開場白\n```\n丙尾\n### Erste Nachricht\n丁\n~~~~\n### Примеры диалога\n~~~\n仍在圍欄\n~~~~\n### Примеры диалога\n戊";
    let sections = split_public_markdown(markdown);
    assert_eq!(sections[0], "前言");
    assert_eq!(sections[1], "乙");
    assert_eq!(sections[2], "丙\n```md\n### 開場白\n```\n丙尾");
    // 圍欄（~~~~ 開、~~~ 太短不算關、~~~~ 才關）整段屬於前一欄的內文
    assert_eq!(
        sections[3],
        "丁\n~~~~\n### Примеры диалога\n~~~\n仍在圍欄\n~~~~"
    );
    assert_eq!(sections[4], "戊");
}

/// 圍欄規則：4 個空白縮排不算圍欄、反引號 info 含反引號不算開頭、關閉要同符號且不短於開啟、
/// 未閉合延伸到結尾；圍欄外內文自然出現的同名段標照規則切欄（已知歧義）。
#[test]
fn fence_rules_and_known_ambiguity() {
    let indented = split_public_markdown("    ```\n### Scenario\n丙");
    assert_eq!(indented[0], "```");
    assert_eq!(indented[2], "丙");

    let bad_info = split_public_markdown("``` a`b\n### Scenario\n丙");
    assert_eq!(bad_info[2], "丙");

    let short_close = split_public_markdown("````\n```\n### Scenario\n````\n### Scenario\n丙");
    assert_eq!(short_close[0], "````\n```\n### Scenario\n````");
    assert_eq!(short_close[2], "丙");

    let unclosed = split_public_markdown("```\n### Scenario\n丙");
    assert_eq!(unclosed[0], "```\n### Scenario\n丙");
    assert_eq!(unclosed[2], "");

    let natural = split_public_markdown("He loves maps.\n### Scenario\nnot a heading by intent");
    assert_eq!(natural[0], "He loves maps.");
    assert_eq!(natural[2], "not a heading by intent");
}

/// 段標表裡同一個字不會對到兩個欄位（十語系擴大命中面時不互相搶）。
#[test]
fn section_labels_never_map_to_two_fields() {
    for (index, (_, labels)) in PUBLIC_SECTIONS.iter().enumerate() {
        for label in labels {
            for (other, (_, other_labels)) in PUBLIC_SECTIONS.iter().enumerate() {
                if other != index {
                    assert!(!other_labels.contains(label), "{label}");
                }
            }
        }
    }
}

/// 舊格式卡檔（段落切不開）：直接 id 匯出與轉世界書都回錯，不寫出檔案、不動世界書與卡檔。
#[test]
fn legacy_card_file_export_and_conversion_fail_without_writing() {
    let root = TestRoot::new("export-legacy");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let id = data::new_id();
    let path = root
        .path()
        .join(format!("worlds/{world_id}/characters/{id}.md"));
    let legacy = format!(
        "---\nid: {id}\nname: 舊卡\ncolor: #000000\navatar: 🎭\ntier: default\n---\n## 公開\n公開\n## 私有\n私密"
    );
    fs::write(&path, &legacy).unwrap();
    let target = root.path().join("舊卡.json");
    assert!(export_character(root.path(), &world_id, &id, &target).is_err());
    assert!(!target.exists());
    assert!(data::character_to_worldbook_entry_held(
        root.path(),
        &world_id,
        &id,
        "zh-TW",
        &data::test_exclusive(&world_id)
    )
    .is_err());
    assert!(data::read_worldbook(root.path(), &world_id)
        .unwrap()
        .is_empty());
    assert_eq!(fs::read_to_string(&path).unwrap(), legacy);
}

/// 備用開場白反解：開場白內文自帶的標題留在該段；圍欄裡長得像段頭的行不切段；第一個段頭之前是私有筆記；
/// 最後一段之後的內容分不出來，併進最後一段（已知限制）。
#[test]
fn private_markdown_splits_back_into_notes_and_greetings() {
    let (notes, greetings) = split_private_markdown(
        "其實是逃兵\n\n### 備用開場白 1\n雨夜。\n### 第一章\n門開了。\n\n### Alternate greeting 2\n晴天。",
    );
    assert_eq!(notes, "其實是逃兵");
    assert_eq!(greetings, vec!["雨夜。\n### 第一章\n門開了。", "晴天。"]);

    let (notes, greetings) =
        split_private_markdown("### 備用開場白 1\n```\n### 備用開場白 2\n```\n結尾");
    assert_eq!(notes, "");
    assert_eq!(greetings, vec!["```\n### 備用開場白 2\n```\n結尾"]);

    let (notes, greetings) = split_private_markdown(
        "### 備用開場白 1\n第一則。\n\n### 備用開場白 2\n第二則。\n\n後來補的筆記",
    );
    assert_eq!(notes, "");
    assert_eq!(greetings, vec!["第一則。", "第二則。\n\n後來補的筆記"]);

    // 不是正整數的不算段頭
    let (notes, greetings) = split_private_markdown("### 備用開場白 0\n不是開場白");
    assert_eq!(notes, "### 備用開場白 0\n不是開場白");
    assert!(greetings.is_empty());
}
