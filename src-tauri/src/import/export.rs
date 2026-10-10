use super::card::{ALTERNATE_GREETING, PUBLIC_SECTIONS};
use super::card_io::{base64_encode, blank_png, png_chunk, png_invalid, PNG_MAGIC};
use super::mechanism::table_tavern_extension;
use crate::data::{self, CharacterCard, DataResult};
use crate::ui_msg::UiMsg;
use crate::world_info::entry::EXTENSION_FIELDS;
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
    let json = serde_json::to_vec_pretty(&character_card_v2(root, world_id, &card)?)?;
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

fn character_card_v2(root_dir: &Path, world_id: &str, card: &CharacterCard) -> DataResult<Value> {
    let sections = split_public_markdown(&card.public_md);
    let (notes, greetings) = split_private_markdown(&card.private_md);
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
        ("alternate_greetings", json!(greetings)),
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
    // 卡內世界書：這桌世界書裡跟著這張卡的條目＋私有筆記。世界書壞掉就整次匯出失敗——吞掉的話匯出
    // 照樣成功、整本角色設定卻不見了；沒有世界書檔＝沒有條目，不算錯
    let entries = data::character_book_raw_entries(root_dir, world_id, &card.id)?;
    if let Some(book) = character_book(&entries, &notes, &card.name) {
        data.insert("character_book".to_owned(), book);
    }
    // 頂層同時放 V1 欄位：只吃舊格式的工具也讀得到（SillyTavern 自己匯出時也這樣寫）
    root.insert("name".to_owned(), Value::String(card.name.clone()));
    root.insert("spec".to_owned(), json!("chara_card_v2"));
    root.insert("spec_version".to_owned(), json!("2.0"));
    root.insert("data".to_owned(), Value::Object(data));
    Ok(Value::Object(root))
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

/// private_markdown 的反向：拆成私有筆記與備用開場白。
/// 段頭＝圍欄外、整行（去掉行尾空白）等於十語系 `### 備用開場白 {n}`（n 為正整數）的行；圍欄裡長得像段頭的
/// 行不算。一段開場白從段頭下一行到下一個段頭或結尾為止，內文自帶的其他標題留在該段。第一個段頭之前是
/// 私有筆記；最後一段之後的內容分不出來，一律算進最後一段（已知限制，匯入時開場白段寫在最後）。
fn split_private_markdown(private_md: &str) -> (String, Vec<String>) {
    let mut notes: Vec<&str> = Vec::new();
    let mut greetings: Vec<Vec<&str>> = Vec::new();
    let mut fence: Option<Fence> = None;
    for line in private_md.lines() {
        let heading = match &fence {
            Some(open) => {
                if open.closed_by(line) {
                    fence = None;
                }
                false
            }
            None => {
                fence = Fence::open(line);
                fence.is_none() && is_greeting_heading(line)
            }
        };
        if heading {
            greetings.push(Vec::new());
            continue;
        }
        match greetings.last_mut() {
            Some(greeting) => greeting.push(line),
            None => notes.push(line),
        }
    }
    let greetings = greetings
        .into_iter()
        .map(|lines| lines.join("\n").trim_end_matches('\n').to_owned())
        .collect();
    (notes.join("\n").trim().to_owned(), greetings)
}

fn is_greeting_heading(line: &str) -> bool {
    let Some(label) = line.trim_end().strip_prefix("### ") else {
        return false;
    };
    ALTERNATE_GREETING.iter().any(|template| {
        let Some((prefix, suffix)) = template.split_once("{n}") else {
            return false;
        };
        label
            .strip_prefix(prefix)
            .and_then(|rest| rest.strip_suffix(suffix))
            .is_some_and(|number| {
                !number.is_empty()
                    && number.bytes().all(|byte| byte.is_ascii_digit())
                    && number.parse::<u64>().is_ok_and(|number| number > 0)
            })
    })
}

/// 匯出的 character_book：跟著這張卡的世界書條目（原始值轉回 V2 欄位）＋私有筆記併成的一條常駐條目
/// （ST 那邊 constant 才會固定注入）。
fn character_book(entries: &[Value], notes: &str, name: &str) -> Option<Value> {
    let mut book: Vec<Value> = entries.iter().map(v2_entry).collect();
    // 條目的 id＝uid（ST 以 id 當載入順序的鍵）；私有筆記條目取比所有 id 都大的下一個
    let next_id = book
        .iter()
        .filter_map(|entry| entry.get("id").and_then(Value::as_u64))
        .max()
        .map_or(0, |max| max + 1);
    if !notes.is_empty() {
        book.push(json!({
            "id": next_id,
            "keys": [],
            "secondary_keys": [],
            "comment": "",
            "content": notes,
            "constant": true,
            "selective": false,
            "insertion_order": book.len(),
            "enabled": true,
            "position": "before_char",
            "case_sensitive": false,
            "extensions": {},
        }));
    }
    if book.is_empty() {
        return None;
    }
    Some(json!({ "name": name, "entries": book, "extensions": {} }))
}

/// 這桌世界書的原始條目轉回 V2 character_book 條目（照 ST convertWorldInfoToCharacterBook 的對照）：
/// 欄位改名、位置數字轉字串（0→before_char、其他→after_char，原數字留在 extensions.position）、
/// 大小寫優先用原卡的 snake_case 值；匯入時強制停用的鷹架條目還原原卡啟停；table_tavern 只留
/// 明寫的 gm／public 可見度（角色名單、來源卡、停用標記都拿掉，下一桌重新套預設）。
fn v2_entry(raw: &Value) -> Value {
    let mut entry = raw.as_object().cloned().unwrap_or_default();
    if let Some(uid) = entry.remove("uid") {
        entry.insert("id".to_owned(), uid);
    }
    entry.remove("displayIndex");
    let keys = entry.remove("key").unwrap_or_else(|| json!([]));
    entry.insert("keys".to_owned(), keys);
    let secondary = entry.remove("keysecondary").unwrap_or_else(|| json!([]));
    entry.insert("secondary_keys".to_owned(), secondary);
    // 缺或不是有限數字＝讀取端的 100（作者裁決 2026-10-10）
    let order = entry
        .remove("order")
        .filter(|order| order.as_f64().is_some_and(f64::is_finite))
        .unwrap_or_else(|| json!(100));
    entry.insert("insertion_order".to_owned(), order);
    let table_tavern = raw
        .get("extensions")
        .and_then(|extensions| extensions.get("table_tavern"));
    let disable = entry
        .remove("disable")
        .and_then(|disable| disable.as_bool())
        .unwrap_or(false);
    let forced = table_tavern
        .and_then(|table_tavern| table_tavern.get("forced_disable"))
        .is_some();
    let disable = match table_tavern
        .and_then(|table_tavern| table_tavern.get("source_disable"))
        .and_then(Value::as_bool)
    {
        Some(source) if forced && disable => source,
        _ => disable,
    };
    entry.insert("enabled".to_owned(), json!(!disable));
    let case_sensitive = entry.remove("caseSensitive");
    if !entry.contains_key("case_sensitive") {
        let value = case_sensitive
            .as_ref()
            .and_then(Value::as_bool)
            .unwrap_or(false);
        entry.insert("case_sensitive".to_owned(), json!(value));
    }
    // 布林欄位照讀取端的解讀補：非布林（含 null）時 constant 是 false、selective 是 true
    // （物件形預設）；不然 V2 讀回來變 false，有次要鍵時觸發會放寬
    for (field, fallback) in [("constant", false), ("selective", true)] {
        if !entry.get(field).is_some_and(Value::is_boolean) {
            entry.insert(field.to_owned(), json!(fallback));
        }
    }
    let mut extensions = entry
        .remove("extensions")
        .and_then(|extensions| match extensions {
            Value::Object(map) => Some(map),
            _ => None,
        })
        .unwrap_or_default();
    // 物件形的觸發欄位收回 extensions 的 snake_case（ST 匯入只讀 extensions，留在頂層會丟）
    if let Some(case_sensitive) = case_sensitive {
        extensions.insert("case_sensitive".to_owned(), case_sensitive);
    }
    for (field, snake) in EXTENSION_FIELDS {
        if let Some(value) = entry.remove(field) {
            extensions.insert(snake.to_owned(), value);
        }
    }
    match entry.remove("position") {
        Some(Value::String(position)) => {
            entry.insert("position".to_owned(), Value::String(position));
        }
        Some(Value::Number(number)) => {
            let label = if number.as_i64() == Some(0) {
                "before_char"
            } else {
                "after_char"
            };
            entry.insert("position".to_owned(), json!(label));
            extensions.insert("position".to_owned(), Value::Number(number));
        }
        _ => {
            entry.insert("position".to_owned(), json!("before_char"));
        }
    }
    let visibility = table_tavern
        .and_then(|table_tavern| table_tavern.get("visibility"))
        .and_then(Value::as_str)
        .filter(|visibility| matches!(*visibility, "gm" | "public"));
    extensions.remove("table_tavern");
    if let Some(visibility) = visibility {
        extensions.insert(
            "table_tavern".to_owned(),
            json!({ "visibility": visibility }),
        );
    }
    entry.insert("extensions".to_owned(), Value::Object(extensions));
    Value::Object(entry)
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
