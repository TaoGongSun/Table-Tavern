use super::card::PUBLIC_SECTIONS;
use super::card_io::{base64_encode, blank_png, png_chunk, png_invalid, PNG_MAGIC};
use super::mechanism::table_tavern_extension;
use crate::data::{self, CharacterCard, DataResult};
use crate::ui_msg::UiMsg;
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
    crate::data::refuse_if_updating()?;
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
    for ((field, _), content) in PUBLIC_SECTIONS.into_iter().zip(sections) {
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

/// public_markdown 的反向：圍欄外、整行（去掉行尾空白）等於 `### <任一語系的欄位段標>` 的行切欄，
/// 每個段標各自辨識十語系，混語系也拆得回去；其餘（App 內手寫的卡）全歸簡介。
/// 已知歧義：內文自然出現、與段標同字的行（例如英文描述裡的 `### Scenario`）也會被當成段標切欄，
/// 只承諾 App 自己寫的段標拆得回去，不承諾無損還原。
fn split_public_markdown(markdown: &str) -> [String; PUBLIC_SECTIONS.len()] {
    let mut sections: [String; PUBLIC_SECTIONS.len()] = Default::default();
    let mut current = 0;
    let mut fence: Option<Fence> = None;
    for line in markdown.lines() {
        match &fence {
            Some(open) => {
                if open.closed_by(line) {
                    fence = None;
                }
            }
            None => {
                if let Some(opened) = Fence::open(line) {
                    fence = Some(opened);
                } else if let Some(index) = section_index(line) {
                    current = index;
                    continue;
                }
            }
        }
        if !sections[current].is_empty() {
            sections[current].push('\n');
        }
        sections[current].push_str(line);
    }
    sections.map(|section| section.trim().to_owned())
}

fn section_index(line: &str) -> Option<usize> {
    let label = line.trim_end().strip_prefix("### ")?;
    PUBLIC_SECTIONS
        .iter()
        .position(|(_, labels)| labels.contains(&label))
}

/// CommonMark 圍欄：開頭行縮排 ≤3 空白、≥3 個同一種符號（反引號圍欄的 info 不得含反引號）；
/// 關閉行縮排 ≤3 空白、同一種符號、長度 ≥ 開啟長度、後面只有空白；未閉合就延伸到結尾。
struct Fence {
    symbol: char,
    length: usize,
}

impl Fence {
    fn run(line: &str) -> Option<(char, usize, &str)> {
        let indent = line.len() - line.trim_start_matches(' ').len();
        if indent > 3 {
            return None;
        }
        let rest = &line[indent..];
        let symbol = rest.chars().next().filter(|c| *c == '`' || *c == '~')?;
        let length = rest.len() - rest.trim_start_matches(symbol).len();
        (length >= 3).then(|| (symbol, length, &rest[length..]))
    }

    fn open(line: &str) -> Option<Fence> {
        let (symbol, length, info) = Self::run(line)?;
        (symbol == '~' || !info.contains('`')).then_some(Fence { symbol, length })
    }

    fn closed_by(&self, line: &str) -> bool {
        matches!(Self::run(line), Some((symbol, length, rest))
            if symbol == self.symbol && length >= self.length && rest.trim().is_empty())
    }
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

/// 把角色卡 JSON 寫成 tEXt chara chunk 放進 IEND 前；底圖原有的卡片 chunk（含 V3 的 ccv3）
/// 先清掉，不然 ST 會讀到匯入當下那份舊資料
fn embed_chara_chunk(base: &[u8], json: &[u8]) -> DataResult<Vec<u8>> {
    if !base.starts_with(PNG_MAGIC) {
        return Err(UiMsg::ImageNotPng.into_error());
    }
    let mut output = PNG_MAGIC.to_vec();
    let mut offset = PNG_MAGIC.len();
    while offset < base.len() {
        if base.len() - offset < 12 {
            return Err(png_invalid("chunk header truncated"));
        }
        let length = u32::from_be_bytes(base[offset..offset + 4].try_into().unwrap()) as usize;
        let chunk_end = offset
            .checked_add(12)
            .and_then(|end| end.checked_add(length))
            .ok_or_else(|| png_invalid("chunk length overflow"))?;
        if chunk_end > base.len() {
            return Err(png_invalid("chunk runs past end of file"));
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
    Err(png_invalid("missing IEND chunk"))
}

#[cfg(test)]
mod tests;
