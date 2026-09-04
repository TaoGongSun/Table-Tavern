use super::card::PUBLIC_SECTIONS;
use super::card_io::{base64_encode, png_chunk, PNG_MAGIC};
use super::mechanism::table_tavern_extension;
use crate::data::{self, CharacterCard, DataResult};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

/// 匯出成 SillyTavern chara_card_v2：內容一律由現在的卡重建（匯入後改過的字才會跟著出去）。
/// 副檔名 .json 直接寫 JSON，其餘寫 PNG——把 JSON 塞進 tEXt chara chunk，底圖用這張卡的圖。
pub fn export_character(
    root: &Path,
    world_id: &str,
    character_id: &str,
    path: &Path,
) -> DataResult<()> {
    let card = data::read_character(root, world_id, character_id)?;
    let json = serde_json::to_vec_pretty(&character_card_v2(root, world_id, &card))?;
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        fs::write(path, json)?;
    } else {
        fs::write(
            path,
            embed_chara_chunk(&export_base_png(root, world_id, character_id)?, &json)?,
        )?;
    }
    Ok(())
}

fn character_card_v2(root_dir: &Path, world_id: &str, card: &CharacterCard) -> Value {
    let sections = split_public_markdown(&card.public_md);
    let mut data = serde_json::Map::new();
    for ((_, field), content) in PUBLIC_SECTIONS.into_iter().zip(sections) {
        data.insert(field.to_owned(), Value::String(content));
    }
    let mut root = data.clone();
    data.insert("name".to_owned(), Value::String(card.name.clone()));
    for (field, value) in [
        ("creator_notes", json!("")),
        ("system_prompt", json!("")),
        ("post_history_instructions", json!("")),
        ("alternate_greetings", json!([])),
        ("tags", json!([])),
        ("creator", json!("")),
        ("character_version", json!("")),
        (
            "extensions",
            table_tavern_extension(root_dir, world_id, &card.name),
        ),
    ] {
        data.insert(field.to_owned(), value);
    }
    if let Some(book) = character_book(&card.private_md, &card.name) {
        data.insert("character_book".to_owned(), book);
    }
    // 頂層同時放 V1 欄位：只吃舊格式的工具也讀得到（SillyTavern 自己匯出時也這樣寫）
    root.insert("name".to_owned(), Value::String(card.name.clone()));
    root.insert("spec".to_owned(), json!("chara_card_v2"));
    root.insert("spec_version".to_owned(), json!("2.0"));
    root.insert("data".to_owned(), Value::Object(data));
    Value::Object(root)
}

/// public_markdown 的反向：認得的 `### 標題` 各自回原欄位，其餘（App 內手寫的卡）全歸簡介
fn split_public_markdown(markdown: &str) -> [String; PUBLIC_SECTIONS.len()] {
    let mut sections: [String; PUBLIC_SECTIONS.len()] = Default::default();
    let mut current = 0;
    for line in markdown.lines() {
        if let Some(index) = PUBLIC_SECTIONS
            .iter()
            .position(|(heading, _)| line.trim_end() == format!("### {heading}"))
        {
            current = index;
            continue;
        }
        if !sections[current].is_empty() {
            sections[current].push('\n');
        }
        sections[current].push_str(line);
    }
    sections.map(|section| section.trim().to_owned())
}

/// private_markdown 的反向：`- **關鍵字**：內容` 回成有關鍵字的條目，
/// 其餘私有筆記併成一條沒有關鍵字的常駐條目（ST 那邊 constant 才會固定注入）
fn character_book(private_md: &str, name: &str) -> Option<Value> {
    if private_md.trim().is_empty() {
        return None;
    }
    let mut entries: Vec<(Vec<&str>, String)> = Vec::new();
    let mut loose: Vec<&str> = Vec::new();
    for line in private_md.lines() {
        match line
            .strip_prefix("- **")
            .and_then(|rest| rest.split_once("**："))
        {
            Some((keys, content)) if !content.trim().is_empty() => entries.push((
                keys.split('、')
                    .map(str::trim)
                    .filter(|key| !key.is_empty())
                    .collect(),
                content.trim().to_owned(),
            )),
            _ => loose.push(line),
        }
    }
    let loose = loose.join("\n").trim().to_owned();
    if !loose.is_empty() {
        entries.push((Vec::new(), loose));
    }
    let entries = entries
        .into_iter()
        .enumerate()
        .map(|(index, (keys, content))| {
            json!({
                "id": index,
                "keys": keys,
                "secondary_keys": [],
                "comment": "",
                "content": content,
                "constant": keys.is_empty(),
                "selective": !keys.is_empty(),
                "insertion_order": index,
                "enabled": true,
                "position": "before_char",
                "case_sensitive": false,
                "extensions": {},
            })
        })
        .collect::<Vec<_>>();
    Some(json!({ "name": name, "entries": entries, "extensions": {} }))
}

/// 匯出底圖：優先用卡片圖，其次頭像，都沒有就給一張 1×1 透明 PNG（ST 讀的是 tEXt，圖只是外觀）
fn export_base_png(root: &Path, world_id: &str, character_id: &str) -> DataResult<Vec<u8>> {
    let path = data::character_path(root, world_id, character_id)?;
    for extension in ["png", "avatar.png"] {
        let candidate = path.with_extension(extension);
        if candidate.is_file() {
            let bytes = fs::read(candidate)?;
            if bytes.starts_with(PNG_MAGIC) {
                return Ok(bytes);
            }
        }
    }
    Ok(blank_png())
}

fn blank_png() -> Vec<u8> {
    let mut png = PNG_MAGIC.to_vec();
    let mut header = 1u32.to_be_bytes().to_vec();
    header.extend_from_slice(&1u32.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
    png.extend_from_slice(&png_chunk(b"IHDR", &header));
    // zlib（stored deflate）：一列 filter 0 + 一個全透明像素
    const PIXEL: &[u8] = &[
        0x78, 0x01, 0x01, 0x05, 0x00, 0xfa, 0xff, 0, 0, 0, 0, 0, 0x00, 0x05, 0x00, 0x01,
    ];
    png.extend_from_slice(&png_chunk(b"IDAT", PIXEL));
    png.extend_from_slice(&png_chunk(b"IEND", &[]));
    png
}

/// 把角色卡 JSON 寫成 tEXt chara chunk 放進 IEND 前；底圖原有的卡片 chunk（含 V3 的 ccv3）
/// 先清掉，不然 ST 會讀到匯入當下那份舊資料
fn embed_chara_chunk(base: &[u8], json: &[u8]) -> DataResult<Vec<u8>> {
    if !base.starts_with(PNG_MAGIC) {
        return Err(data::invalid_data("匯出底圖必須是 PNG"));
    }
    let mut output = PNG_MAGIC.to_vec();
    let mut offset = PNG_MAGIC.len();
    while offset < base.len() {
        if base.len() - offset < 12 {
            return Err(data::invalid_data("PNG chunk 格式不完整"));
        }
        let length = u32::from_be_bytes(base[offset..offset + 4].try_into().unwrap()) as usize;
        let chunk_end = offset
            .checked_add(12)
            .and_then(|end| end.checked_add(length))
            .ok_or_else(|| data::invalid_data("PNG chunk 長度無效"))?;
        if chunk_end > base.len() {
            return Err(data::invalid_data("PNG chunk 長度超出檔案範圍"));
        }
        let kind = &base[offset + 4..offset + 8];
        let chunk_data = &base[offset + 8..offset + 8 + length];
        if kind == b"IEND" {
            let mut text = b"chara\0".to_vec();
            text.extend_from_slice(base64_encode(json).as_bytes());
            output.extend_from_slice(&png_chunk(b"tEXt", &text));
            output.extend_from_slice(&base[offset..chunk_end]);
            return Ok(output);
        }
        let is_card_text = matches!(kind, b"tEXt" | b"zTXt" | b"iTXt")
            && chunk_data
                .split(|byte| *byte == 0)
                .next()
                .is_some_and(|keyword| keyword == b"chara" || keyword == b"ccv3");
        if !is_card_text {
            output.extend_from_slice(&base[offset..chunk_end]);
        }
        offset = chunk_end;
    }
    Err(data::invalid_data("PNG 缺少 IEND chunk"))
}

#[cfg(test)]
mod tests {
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
        let value: Value =
            serde_json::from_slice(&decode_png_character(&exported).unwrap()).unwrap();
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
        let value: Value =
            serde_json::from_slice(&decode_png_character(&exported).unwrap()).unwrap();
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
}
