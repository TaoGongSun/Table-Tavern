use crate::data::{self, CharacterCard, CharacterMeta, DataResult, Tier};
use serde_json::Value;
use std::fs;
use std::path::Path;

const PNG_MAGIC: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

pub fn import_character(
    root: &Path,
    world: &str,
    bytes: &[u8],
    color: &str,
) -> DataResult<CharacterMeta> {
    let (json_bytes, raw_extension) = if bytes.starts_with(PNG_MAGIC) {
        (decode_png_character(bytes)?, "png")
    } else {
        (bytes.to_vec(), "import.json")
    };
    let value: Value = serde_json::from_slice(&json_bytes)
        .map_err(|error| data::invalid_data(format!("角色卡 JSON 無法解析：{error}")))?;
    let card_data = value
        .get("data")
        .filter(|data| data.is_object())
        .unwrap_or(&value);
    let name = string_field(card_data, "name")
        .ok_or_else(|| data::invalid_data("角色卡缺少 name"))?
        .trim()
        .to_owned();
    data::validate_name(&name)?;

    let md_path = data::character_path(root, world, &name)?;
    if md_path.exists() {
        return Err(data::invalid_data(format!("角色 {name} 已存在")));
    }

    let card = CharacterCard {
        name: name.clone(),
        color: color.to_owned(),
        avatar: "🎭".to_owned(),
        tier: Tier::parse("default")?,
        public_md: public_markdown(card_data),
        private_md: private_markdown(card_data),
    };
    data::write_character(root, world, &card)?;
    fs::write(md_path.with_extension(raw_extension), bytes)?;

    Ok(CharacterMeta {
        name,
        color: color.to_owned(),
        avatar: "🎭".to_owned(),
        tier: Tier::parse("default")?,
    })
}

fn string_field<'a>(data: &'a Value, field: &str) -> Option<&'a str> {
    data.get(field).and_then(Value::as_str)
}

fn public_markdown(data: &Value) -> String {
    [
        ("簡介", "description"),
        ("人格與語氣", "personality"),
        ("場景", "scenario"),
        ("開場白", "first_mes"),
        ("語氣範例", "mes_example"),
    ]
    .into_iter()
    .filter_map(|(heading, field)| {
        let content = string_field(data, field)?;
        (!content.trim().is_empty()).then(|| format!("### {heading}\n{content}"))
    })
    .collect::<Vec<_>>()
    .join("\n\n")
}

fn private_markdown(data: &Value) -> String {
    data.get("character_book")
        .and_then(|book| book.get("entries"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let content = string_field(entry, "content")?;
            if content.trim().is_empty() {
                return None;
            }
            let keys = entry
                .get("keys")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("、");
            Some(format!("- **{keys}**：{content}"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn decode_png_character(bytes: &[u8]) -> DataResult<Vec<u8>> {
    let mut offset = PNG_MAGIC.len();
    while offset < bytes.len() {
        if bytes.len() - offset < 12 {
            return Err(data::invalid_data("PNG chunk 格式不完整"));
        }
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let chunk_end = offset
            .checked_add(12)
            .and_then(|end| end.checked_add(length))
            .ok_or_else(|| data::invalid_data("PNG chunk 長度無效"))?;
        if chunk_end > bytes.len() {
            return Err(data::invalid_data("PNG chunk 長度超出檔案範圍"));
        }
        let kind = &bytes[offset + 4..offset + 8];
        let chunk_data = &bytes[offset + 8..offset + 8 + length];
        if kind == b"tEXt" {
            if let Some(separator) = chunk_data.iter().position(|byte| *byte == 0) {
                if &chunk_data[..separator] == b"chara" {
                    return decode_base64(&chunk_data[separator + 1..]);
                }
            }
        }
        offset = chunk_end;
    }
    Err(data::invalid_data("PNG 找不到 chara tEXt chunk"))
}

fn decode_base64(input: &[u8]) -> DataResult<Vec<u8>> {
    if input.len() % 4 != 0 {
        return Err(data::invalid_data("chara base64 長度無效"));
    }
    let mut output = Vec::with_capacity(input.len() / 4 * 3);
    for group in input.chunks_exact(4) {
        let padding = group.iter().filter(|byte| **byte == b'=').count();
        if padding > 2
            || (padding > 0
                && (group[..4 - padding].iter().any(|byte| *byte == b'=')
                    || group[4 - padding..4].iter().any(|byte| *byte != b'=')))
        {
            return Err(data::invalid_data("chara base64 padding 無效"));
        }
        if padding > 0 && group != &input[input.len() - 4..] {
            return Err(data::invalid_data("chara base64 padding 位置無效"));
        }
        let mut values = [0u8; 4];
        for (index, byte) in group.iter().enumerate() {
            values[index] = if *byte == b'=' {
                0
            } else {
                base64_value(*byte).ok_or_else(|| data::invalid_data("chara base64 含非法字元"))?
            };
        }
        output.push((values[0] << 2) | (values[1] >> 4));
        if padding < 2 {
            output.push((values[1] << 4) | (values[2] >> 2));
        }
        if padding == 0 {
            output.push((values[2] << 6) | values[3]);
        }
    }
    Ok(output)
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(label: &str) -> Self {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "table-tavern-import-{label}-{}-{id}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn minimal_png(chara_json: &str) -> Vec<u8> {
        let mut png = PNG_MAGIC.to_vec();
        let text = format!("chara\0{}", base64_encode(chara_json.as_bytes()));
        png.extend_from_slice(&(text.len() as u32).to_be_bytes());
        png.extend_from_slice(b"tEXt");
        png.extend_from_slice(text.as_bytes());
        png.extend_from_slice(&[0; 4]);
        png
    }

    fn base64_encode(bytes: &[u8]) -> String {
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut output = String::new();
        for group in bytes.chunks(3) {
            let a = group[0];
            let b = *group.get(1).unwrap_or(&0);
            let c = *group.get(2).unwrap_or(&0);
            output.push(ALPHABET[(a >> 2) as usize] as char);
            output.push(ALPHABET[(((a & 0x03) << 4) | (b >> 4)) as usize] as char);
            output.push(if group.len() > 1 {
                ALPHABET[(((b & 0x0f) << 2) | (c >> 6)) as usize] as char
            } else {
                '='
            });
            output.push(if group.len() > 2 {
                ALPHABET[(c & 0x3f) as usize] as char
            } else {
                '='
            });
        }
        output
    }

    #[test]
    fn imports_v2_json_and_preserves_original() {
        let root = TestRoot::new("v2");
        data::create_world(root.path(), "酒館").unwrap();
        let raw = r#"{"spec":"chara_card_v2","spec_version":"2.0","data":{"name":"莉亞","description":"精靈遊俠","personality":"冷靜","scenario":"雨夜","first_mes":"妳來了。","mes_example":"<START>","character_book":{"entries":[{"keys":["森林","月亮"],"content":"古老盟約","enabled":true},{"keys":["略過"],"content":""}]}}}"#.as_bytes();

        let meta = import_character(root.path(), "酒館", raw, "#3366ff").unwrap();
        assert_eq!(meta.name, "莉亞");
        let markdown =
            fs::read_to_string(root.path().join("worlds/酒館/characters/莉亞.md")).unwrap();
        assert!(
            markdown.contains("---\nname: 莉亞\ncolor: #3366ff\navatar: 🎭\ntier: default\n---")
        );
        for section in [
            "### 簡介\n精靈遊俠",
            "### 人格與語氣\n冷靜",
            "### 場景\n雨夜",
            "### 開場白\n妳來了。",
            "### 語氣範例\n<START>",
            "## 私有\n- **森林、月亮**：古老盟約",
        ] {
            assert!(markdown.contains(section), "missing {section}");
        }
        assert_eq!(
            fs::read(root.path().join("worlds/酒館/characters/莉亞.import.json")).unwrap(),
            raw
        );
    }

    #[test]
    fn imports_png_text_chunk_and_preserves_original() {
        let root = TestRoot::new("png");
        data::create_world(root.path(), "酒館").unwrap();
        let png = minimal_png(r#"{"data":{"name":"凱恩","description":"騎士"}}"#);

        import_character(root.path(), "酒館", &png, "#111111").unwrap();
        assert!(root.path().join("worlds/酒館/characters/凱恩.md").is_file());
        assert_eq!(
            fs::read(root.path().join("worlds/酒館/characters/凱恩.png")).unwrap(),
            png
        );
        assert_eq!(data::list_characters(root.path(), "酒館").unwrap().len(), 1);
    }

    #[test]
    fn imports_v1_top_level_fields() {
        let root = TestRoot::new("v1");
        data::create_world(root.path(), "酒館").unwrap();
        import_character(
            root.path(),
            "酒館",
            r#"{"name":"舊卡","personality":"直率"}"#.as_bytes(),
            "#222222",
        )
        .unwrap();
        let card = data::read_character(root.path(), "酒館", "舊卡").unwrap();
        assert_eq!(card.public_md, "### 人格與語氣\n直率");
    }

    #[test]
    fn duplicate_name_does_not_modify_existing_card() {
        let root = TestRoot::new("duplicate");
        data::create_world(root.path(), "酒館").unwrap();
        let original = CharacterCard {
            name: "重名".to_owned(),
            color: "#000000".to_owned(),
            avatar: "🎭".to_owned(),
            tier: Tier::Default,
            public_md: "既有內容".to_owned(),
            private_md: String::new(),
        };
        data::write_character(root.path(), "酒館", &original).unwrap();

        let error = import_character(
            root.path(),
            "酒館",
            r#"{"name":"重名","description":"新內容"}"#.as_bytes(),
            "#ffffff",
        )
        .unwrap_err();
        assert_eq!(error.to_string(), "角色 重名 已存在");
        assert_eq!(
            data::read_character(root.path(), "酒館", "重名").unwrap(),
            original
        );
        assert!(!root
            .path()
            .join("worlds/酒館/characters/重名.import.json")
            .exists());
    }

    #[test]
    fn decodes_base64_and_rejects_invalid_input() {
        assert_eq!(decode_base64(b"SGVsbG8=").unwrap(), b"Hello");
        assert!(decode_base64(b"SGVsbG8!").is_err());
        assert!(decode_base64(b"AA=A").is_err());
    }
}
