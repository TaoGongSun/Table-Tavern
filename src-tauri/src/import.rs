use crate::data::{self, CharacterCard, CharacterMeta, DataResult, Tier};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

const PNG_MAGIC: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

/// 公開段落與 SillyTavern 欄位的對照表：匯入時拆成 `### 標題`，匯出時再併回欄位
const PUBLIC_SECTIONS: [(&str, &str); 5] = [
    ("簡介", "description"),
    ("人格與語氣", "personality"),
    ("場景", "scenario"),
    ("開場白", "first_mes"),
    ("語氣範例", "mes_example"),
];

#[derive(serde::Serialize, Default, Debug, PartialEq)]
pub struct ImportProbe {
    pub scripts: Vec<String>,
    pub lorebook_heavy: bool,
    pub alternate_greetings: usize,
}

/// 匯入前只提示可能無法保留的內容；真正的格式錯誤仍交給匯入處理。
pub fn probe_import(bytes: &[u8]) -> ImportProbe {
    let json_bytes = if bytes.starts_with(PNG_MAGIC) {
        match decode_png_character(bytes) {
            Ok(bytes) => bytes,
            Err(_) => return ImportProbe::default(),
        }
    } else {
        bytes.to_vec()
    };
    let value: Value = match serde_json::from_slice(&json_bytes) {
        Ok(value) => value,
        Err(_) => return ImportProbe::default(),
    };
    let card_data = value
        .get("data")
        .filter(|data| data.is_object())
        .unwrap_or(&value);
    let mut probe = ImportProbe::default();
    let benign_extensions = ["talkativeness", "fav", "world", "depth_prompt"];
    if card_data
        .get("extensions")
        .and_then(Value::as_object)
        .is_some_and(|extensions| {
            extensions
                .keys()
                .any(|key| !benign_extensions.contains(&key.as_str()))
        })
    {
        probe.scripts.push("extensions".to_owned());
    }
    let serialized = serde_json::to_string(&value).unwrap_or_default();
    if serialized.contains("<script") {
        probe.scripts.push("script_tag".to_owned());
    }
    if serialized.contains("<%") {
        probe.scripts.push("template".to_owned());
    }
    probe.lorebook_heavy = card_data
        .get("character_book")
        .and_then(|book| book.get("entries"))
        .and_then(Value::as_array)
        .is_some_and(|entries| {
            entries.len() >= 3
                && ["description", "personality", "scenario"]
                    .into_iter()
                    .map(|field| string_field(card_data, field).unwrap_or("").trim().chars().count())
                    .sum::<usize>()
                    < 200
        });
    probe.alternate_greetings = card_data
        .get("alternate_greetings")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    probe
}

/// 匯入永遠是全新一張卡：mint 新 id，name 照卡片原值（不再擋特殊字元，只擋換行）。
pub fn import_character(
    root: &Path,
    world_id: &str,
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
    data::validate_single_line("name", &name)?;

    let id = data::new_id();
    let md_path = data::character_path(root, world_id, &id)?;

    let card = CharacterCard {
        id: id.clone(),
        name: name.clone(),
        color: color.to_owned(),
        avatar: "🎭".to_owned(),
        tier: Tier::Balanced,
        show_image: true,
        archived: false,
        gen_prompt: String::new(),
        public_md: public_markdown(card_data),
        private_md: private_markdown(card_data),
    };
    data::write_character(root, world_id, &card)?;
    fs::write(md_path.with_extension(raw_extension), bytes)?;

    Ok(CharacterMeta {
        id,
        name,
        color: color.to_owned(),
        avatar: "🎭".to_owned(),
        tier: Tier::Balanced,
        show_image: true,
        archived: false,
        display_index: None,
    })
}

/// 匯出成 SillyTavern chara_card_v2：內容一律由現在的卡重建（匯入後改過的字才會跟著出去）。
/// 副檔名 .json 直接寫 JSON，其餘寫 PNG——把 JSON 塞進 tEXt chara chunk，底圖用這張卡的圖。
pub fn export_character(
    root: &Path,
    world_id: &str,
    character_id: &str,
    path: &Path,
) -> DataResult<()> {
    let card = data::read_character(root, world_id, character_id)?;
    let json = serde_json::to_vec_pretty(&character_card_v2(&card))?;
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        fs::write(path, json)?;
    } else {
        fs::write(path, embed_chara_chunk(&export_base_png(root, world_id, character_id)?, &json)?)?;
    }
    Ok(())
}

fn character_card_v2(card: &CharacterCard) -> Value {
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
        ("extensions", json!({})),
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

/// 世界書卡不會先建成角色，仍要直接從匯入檔取得所有可選開場白。
pub fn card_openings(bytes: &[u8]) -> Option<(String, Vec<String>)> {
    let json_bytes = if bytes.starts_with(PNG_MAGIC) {
        decode_png_character(bytes).ok()?
    } else {
        bytes.to_vec()
    };
    let value: Value = serde_json::from_slice(&json_bytes).ok()?;
    let card_data = value
        .get("data")
        .filter(|data| data.is_object())
        .unwrap_or(&value);
    let mut openings = Vec::new();
    if let Some(opening) = string_field(card_data, "first_mes") {
        let opening = opening.trim();
        if !opening.is_empty() {
            openings.push(opening.to_owned());
        }
    }
    if let Some(alternates) = card_data
        .get("alternate_greetings")
        .and_then(Value::as_array)
    {
        openings.extend(
            alternates
                .iter()
                .filter_map(Value::as_str)
                .filter_map(|opening| {
                    let opening = opening.trim();
                    (!opening.is_empty()).then(|| opening.to_owned())
                }),
        );
    }
    if openings.is_empty() {
        return None;
    }
    let name = string_field(card_data, "name")
        .unwrap_or_default()
        .trim()
        .to_owned();
    Some((name, openings))
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
                keys.split('、').map(str::trim).filter(|key| !key.is_empty()).collect(),
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

fn png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut chunk = (data.len() as u32).to_be_bytes().to_vec();
    chunk.extend_from_slice(kind);
    chunk.extend_from_slice(data);
    chunk.extend_from_slice(&crc32(&chunk[4..]).to_be_bytes());
    chunk
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in bytes {
        crc ^= *byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 == 0 {
                crc >> 1
            } else {
                (crc >> 1) ^ 0xedb8_8320
            };
        }
    }
    !crc
}

/// 匯入時存下的原 PNG（characters/<id>.png）；沒有圖回 None，前端拿 base64 組 data URL 顯示
pub fn character_image(
    root: &Path,
    world_id: &str,
    character_id: &str,
) -> DataResult<Option<String>> {
    let path = data::character_path(root, world_id, character_id)?.with_extension("png");
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(base64_encode(&fs::read(path)?)))
}

pub fn save_character_image(
    root: &Path,
    world_id: &str,
    character_id: &str,
    bytes: &[u8],
) -> DataResult<()> {
    save_character_png(root, world_id, character_id, bytes, "png")
}

pub fn delete_character_image(root: &Path, world_id: &str, character_id: &str) -> DataResult<()> {
    delete_character_png(root, world_id, character_id, "png")
}

pub fn character_avatar(
    root: &Path,
    world_id: &str,
    character_id: &str,
) -> DataResult<Option<String>> {
    let path = data::character_path(root, world_id, character_id)?.with_extension("avatar.png");
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(base64_encode(&fs::read(path)?)))
}

pub fn save_character_avatar(
    root: &Path,
    world_id: &str,
    character_id: &str,
    bytes: &[u8],
) -> DataResult<()> {
    save_character_png(root, world_id, character_id, bytes, "avatar.png")
}

pub fn delete_character_avatar(root: &Path, world_id: &str, character_id: &str) -> DataResult<()> {
    delete_character_png(root, world_id, character_id, "avatar.png")
}

fn save_character_png(
    root: &Path,
    world_id: &str,
    character_id: &str,
    bytes: &[u8],
    extension: &str,
) -> DataResult<()> {
    if !bytes.starts_with(PNG_MAGIC) {
        return Err(data::invalid_data("圖片必須是 PNG"));
    }
    let path = data::character_path(root, world_id, character_id)?;
    if !path.exists() {
        return Err(data::invalid_data(format!("角色 {character_id} 不存在")));
    }
    fs::write(path.with_extension(extension), bytes)?;
    Ok(())
}

fn delete_character_png(
    root: &Path,
    world_id: &str,
    character_id: &str,
    extension: &str,
) -> DataResult<()> {
    let path = data::character_path(root, world_id, character_id)?.with_extension(extension);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn string_field<'a>(data: &'a Value, field: &str) -> Option<&'a str> {
    data.get(field).and_then(Value::as_str)
}

fn public_markdown(data: &Value) -> String {
    PUBLIC_SECTIONS
        .into_iter()
        .filter_map(|(heading, field)| {
            let content = string_field(data, field)?;
            (!content.trim().is_empty()).then(|| format!("### {heading}\n{content}"))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn private_markdown(data: &Value) -> String {
    // 條目維持單換行緊湊排列；備用開場白各成一段，段間空行
    let entry_block = data
        .get("character_book")
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
        .join("\n");
    let mut sections = if entry_block.is_empty() {
        Vec::new()
    } else {
        vec![entry_block]
    };
    sections.extend(
        data.get("alternate_greetings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|greeting| !greeting.is_empty())
            .enumerate()
            .map(|(index, greeting)| format!("### 備用開場白 {}\n{greeting}", index + 1)),
    );
    sections.join("\n\n")
}

/// 世界書匯入的前處理：PNG 卡先解出內嵌 JSON；整包若是角色卡（社群發佈的世界書卡），
/// 剝到 character_book 那層再交給 data::import_worldbook
pub fn worldbook_json(bytes: &[u8]) -> DataResult<String> {
    let json_bytes = if bytes.starts_with(PNG_MAGIC) {
        decode_png_character(bytes)?
    } else {
        bytes.to_vec()
    };
    let value: Value = serde_json::from_slice(&json_bytes)
        .map_err(|error| data::invalid_data(format!("世界書 JSON 無法解析：{error}")))?;
    let book = value
        .get("data")
        .and_then(|data| data.get("character_book"))
        .or_else(|| value.get("character_book"))
        .unwrap_or(&value);
    Ok(book.to_string())
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

fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
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

    #[test]
    fn imports_v2_json_and_preserves_original() {
        let root = TestRoot::new("v2");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let raw = r#"{"spec":"chara_card_v2","spec_version":"2.0","data":{"name":"莉亞","description":"精靈遊俠","personality":"冷靜","scenario":"雨夜","first_mes":"妳來了。","mes_example":"<START>","character_book":{"entries":[{"keys":["森林","月亮"],"content":"古老盟約","enabled":true},{"keys":["略過"],"content":""}]}}}"#.as_bytes();

        let meta = import_character(root.path(), &world_id, raw, "#3366ff").unwrap();
        assert_eq!(meta.name, "莉亞");
        let markdown = fs::read_to_string(
            root.path()
                .join(format!("worlds/{world_id}/characters/{}.md", meta.id)),
        )
        .unwrap();
        assert!(markdown.contains(&format!(
            "---\nid: {}\nname: 莉亞\ncolor: #3366ff\navatar: 🎭\ntier: balanced\nshow_image: true\narchived: false\ndisplay_index: 0\ngen_prompt: \n---",
            meta.id
        )));
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
        let png = minimal_png(r#"{"data":{"name":"凱恩","description":"騎士"}}"#);

        let meta = import_character(root.path(), &world_id, &png, "#111111").unwrap();
        assert!(root
            .path()
            .join(format!("worlds/{world_id}/characters/{}.md", meta.id))
            .is_file());
        assert_eq!(
            fs::read(
                root.path()
                    .join(format!("worlds/{world_id}/characters/{}.png", meta.id))
            )
            .unwrap(),
            png
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
        )
        .unwrap();
        let card = data::read_character(root.path(), &world_id, &meta.id).unwrap();
        assert_eq!(card.public_md, "### 人格與語氣\n直率");
    }

    #[test]
    fn probe_ignores_benign_extensions_and_flags_other_extensions() {
        let benign = probe_import(
            r#"{"data":{"extensions":{"talkativeness":0.5,"fav":true,"world":"酒館","depth_prompt":"提示"}}}"#
                .as_bytes(),
        );
        assert!(benign.scripts.is_empty());

        let flagged = probe_import(&minimal_png(
            r#"{"data":{"extensions":{"tavern_helper":{"enabled":true}}}}"#,
        ));
        assert_eq!(flagged.scripts, ["extensions"]);
    }

    #[test]
    fn probe_finds_script_and_template_text_and_ignores_invalid_bytes() {
        let probe = probe_import(
            br#"{"data":{"description":"<script>alert(1)</script>","personality":"<% user.name %>"}}"#,
        );
        assert!(probe.scripts.contains(&"script_tag".to_owned()));
        assert!(probe.scripts.contains(&"template".to_owned()));
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

        let detailed = probe_import(
            json!({"data":{"character_book":{"entries":[{}, {}, {}]},"description":"長".repeat(300)}})
                .to_string()
                .as_bytes(),
        );
        assert!(!detailed.lorebook_heavy);
    }

    #[test]
    fn imports_alternate_greetings_into_private_markdown() {
        let root = TestRoot::new("alternate-greetings");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let raw = r#"{"data":{"name":"莉亞","character_book":{"entries":[{"keys":["森林"],"content":"古老盟約"}]},"alternate_greetings":["第二次見面。","雨天再訪。"]}}"#;

        assert_eq!(probe_import(raw.as_bytes()).alternate_greetings, 2);
        let meta = import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff").unwrap();
        let private_md = data::read_character(root.path(), &world_id, &meta.id)
            .unwrap()
            .private_md;
        for expected in [
            "- **森林**：古老盟約",
            "### 備用開場白 1",
            "第二次見面。",
            "### 備用開場白 2",
            "雨天再訪。",
        ] {
            assert!(private_md.contains(expected), "missing {expected}");
        }
    }

    /// 測試清單 #12：匯入 ST 角色卡產生新 id、name 照原值（含原本會被擋的字元）；
    /// 同名再匯入一次也會成功，各自拿到不同 id 且互不影響
    #[test]
    fn importing_the_same_name_twice_mints_distinct_ids_and_keeps_first_card_intact() {
        let root = TestRoot::new("duplicate-name");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let odd_name = r#"{"name":"a/b/../重名","description":"第一張"}"#;
        let first =
            import_character(root.path(), &world_id, odd_name.as_bytes(), "#000000").unwrap();
        assert_eq!(first.name, "a/b/../重名");

        let second = import_character(
            root.path(),
            &world_id,
            r#"{"name":"a/b/../重名","description":"第二張"}"#.as_bytes(),
            "#ffffff",
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
        assert_eq!(value["data"]["character_book"]["entries"][0]["keys"][1], "月亮");
        assert_eq!(value["data"]["character_book"]["entries"][0]["content"], "古老盟約");

        let round_trip = import_character(root.path(), &world_id, &exported, "#000000").unwrap();
        assert_eq!(
            data::read_character(root.path(), &world_id, &round_trip.id).unwrap().public_md,
            data::read_character(root.path(), &world_id, &source.id).unwrap().public_md
        );
        assert_eq!(
            data::read_character(root.path(), &world_id, &round_trip.id).unwrap().private_md,
            "- **森林、月亮**：古老盟約"
        );
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
        assert_eq!(exported.windows(6).filter(|window| *window == b"chara\0").count(), 1);
    }

    #[test]
    fn decodes_base64_and_rejects_invalid_input() {
        assert_eq!(decode_base64(b"SGVsbG8=").unwrap(), b"Hello");
        assert!(decode_base64(b"SGVsbG8!").is_err());
        assert!(decode_base64(b"AA=A").is_err());
    }

    #[test]
    fn character_image_returns_png_base64_or_none() {
        let root = TestRoot::new("image");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let png = minimal_png(r#"{"data":{"name":"凱恩"}}"#);
        let meta = import_character(root.path(), &world_id, &png, "#111111").unwrap();

        let encoded = character_image(root.path(), &world_id, &meta.id)
            .unwrap()
            .unwrap();
        assert_eq!(decode_base64(encoded.as_bytes()).unwrap(), png);
        assert_eq!(
            character_image(root.path(), &world_id, &data::new_id()).unwrap(),
            None
        );
    }

    #[test]
    fn saves_and_reads_character_image_and_avatar() {
        let root = TestRoot::new("save-images");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let png = minimal_png(r#"{"data":{"name":"凱恩"}}"#);
        let meta = import_character(root.path(), &world_id, &png, "#111111").unwrap();
        let image = PNG_MAGIC.iter().copied().chain([1, 2]).collect::<Vec<_>>();
        let avatar = PNG_MAGIC.iter().copied().chain([3, 4]).collect::<Vec<_>>();

        save_character_image(root.path(), &world_id, &meta.id, &image).unwrap();
        save_character_avatar(root.path(), &world_id, &meta.id, &avatar).unwrap();

        assert_eq!(
            decode_base64(
                character_image(root.path(), &world_id, &meta.id)
                    .unwrap()
                    .unwrap()
                    .as_bytes()
            )
            .unwrap(),
            image
        );
        assert_eq!(
            decode_base64(
                character_avatar(root.path(), &world_id, &meta.id)
                    .unwrap()
                    .unwrap()
                    .as_bytes()
            )
            .unwrap(),
            avatar
        );
    }

    #[test]
    fn save_character_images_reject_invalid_png_and_missing_character() {
        let root = TestRoot::new("reject-images");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let missing_id = data::new_id();

        assert_eq!(
            save_character_image(root.path(), &world_id, &missing_id, b"not png")
                .unwrap_err()
                .to_string(),
            "圖片必須是 PNG"
        );
        assert_eq!(
            save_character_avatar(root.path(), &world_id, &missing_id, PNG_MAGIC)
                .unwrap_err()
                .to_string(),
            format!("角色 {missing_id} 不存在")
        );
    }

    #[test]
    fn delete_character_images_is_idempotent() {
        let root = TestRoot::new("delete-images");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let png = minimal_png(r#"{"data":{"name":"凱恩"}}"#);
        let meta = import_character(root.path(), &world_id, &png, "#111111").unwrap();

        delete_character_image(root.path(), &world_id, &meta.id).unwrap();
        delete_character_image(root.path(), &world_id, &meta.id).unwrap();
        delete_character_avatar(root.path(), &world_id, &meta.id).unwrap();
        delete_character_avatar(root.path(), &world_id, &meta.id).unwrap();
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
        assert_eq!(results[1], data::WorldbookImport { imported: 0, skipped: 2 });
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
}
