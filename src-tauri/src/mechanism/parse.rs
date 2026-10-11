use super::note::{plain, snippet};
use super::types::{Parsed, Patch, PatchOp, Record, RecordKind};

/// 解析拒收、區塊層級回饋的記帳路徑（抓不到指令自己的 path 時用）。
const BLOCK_PATH: &str = "更新區塊";
const OPS: &str = "op 只能是 replace、delta、insert、remove、move";

// ---------------------------------------------------------------------
// 解析：<UpdateVariable> 原文 → Vec<Patch>
// ---------------------------------------------------------------------

/// 只取可套用的指令（測試用；正式路徑走 `parse_block` 連拒收一起帶）。
#[cfg(test)]
pub fn parse_updates(block: &str) -> Vec<Patch> {
    parse_block(block)
        .into_iter()
        .filter_map(|item| match item {
            Parsed::Patch(patch) => Some(patch),
            Parsed::Note(_) => None,
        })
        .collect()
}

/// 從 `<UpdateVariable>` 原始內容取出更新指令，連同解析時就拒收的回饋，照原順序；絕不 panic。
/// - 內容（去掉 Analysis、取 JSONPatch 段、剝圍欄後）是空的：什麼都不記。
/// - 整段是 JSON 陣列（含 `[]`）：逐物件轉指令。
/// - 不是：逐個掃出平衡的 `{…}` 救援，好的照收；壞的合成一則「格式壞掉」回饋，排在第一個壞物件處；
///   結尾沒收尾的段落（字串少收引號會吞掉之後所有物件）另一則，要 GM 重送那筆起的更新；
///   一個 `{` 都沒有（YAML、條列、`_.set(...)`、說明文字）就一則「沒有可解析的 JSONPatch 陣列」。
pub(super) fn parse_block(block: &str) -> Vec<Parsed> {
    let without_analysis = strip_analysis(block);
    let section = extract_json_patch_section(&without_analysis);
    let candidate = strip_code_fences(&section);
    let candidate = candidate.trim();
    if candidate.is_empty() {
        return Vec::new();
    }
    if let Ok(values) = serde_json::from_str::<Vec<serde_json::Value>>(candidate) {
        return values.iter().map(to_parsed).collect();
    }
    let scan = scan_balanced_objects(candidate);
    let mut items = Vec::new();
    let mut broken: Vec<&str> = Vec::new();
    let mut broken_at = None;
    let mut valid = 0usize;
    for text in &scan.objects {
        match serde_json::from_str::<serde_json::Value>(text) {
            Ok(value) => {
                valid += 1;
                items.push(to_parsed(&value));
            }
            Err(_) => {
                broken_at.get_or_insert(items.len());
                broken.push(text);
            }
        }
    }
    if broken.is_empty() && scan.unclosed.is_none() {
        if valid == 0 {
            items.push(block_note(
                "更新區塊裡沒有可解析的 JSONPatch 陣列，整段未套用。".to_owned(),
            ));
        }
        return items;
    }
    // 平衡但壞掉的物件：只丟那幾筆，其他照收
    if let (Some(first), Some(at)) = (broken.first(), broken_at) {
        // 結尾另有沒收尾的段落時「其餘」不全套用了，交給下面那則講
        let rest = if scan.unclosed.is_none() && valid > 0 {
            "；其餘已處理（被擋下的另列回饋），已套用的不要重送"
        } else {
            ""
        };
        items.insert(
            at,
            block_note(format!(
                "本輪更新區塊有 {} 筆格式壞掉未套用（JSON 語法錯誤）：{}{rest}。",
                broken.len(),
                broken_label(first)
            )),
        );
    }
    // 沒收尾的尾段（多半是字串沒收引號）：從它起到結尾的更新全被吞掉，要 GM 重送
    if let Some(tail) = scan.unclosed {
        let before = if valid > 0 {
            "前面的已處理（被擋下的另列回饋）；"
        } else {
            ""
        };
        items.push(block_note(format!(
            "本輪更新區塊從 {} 起格式壞掉（JSON 語法錯誤，多半是字串少了收尾引號），{before}這筆起到結尾的更新都未套用，請重送。",
            broken_label(tail)
        )));
    }
    items
}

fn block_note(detail: String) -> Parsed {
    Parsed::Note(Record::new(
        RecordKind::Rejected,
        BLOCK_PATH.to_owned(),
        plain(detail),
    ))
}

/// 壞物件怎麼指給 GM 看：抓得到 `"path":"…"` 就印路徑，否則引開頭約 30 字。
fn broken_label(text: &str) -> String {
    if let Some(start) = text.find("\"path\"") {
        let rest = text[start + 6..].trim_start();
        if let Some(rest) = rest.strip_prefix(':') {
            if let Some(rest) = rest.trim_start().strip_prefix('"') {
                if let Some(end) = rest.find('"') {
                    return format!("path {}", snippet(&rest[..end], 60));
                }
            }
        }
    }
    format!("「{}」", snippet(text, 30))
}

fn strip_analysis(block: &str) -> String {
    let lower = block.to_ascii_lowercase();
    let (Some(start), Some(open_end)) = (
        lower.find("<analysis"),
        lower
            .find("<analysis")
            .and_then(|start| lower[start..].find('>').map(|offset| start + offset)),
    ) else {
        return block.to_owned();
    };
    let Some(close_start) = lower[open_end + 1..]
        .find("</analysis>")
        .map(|offset| open_end + 1 + offset)
    else {
        return block.to_owned();
    };
    let close_end = close_start + "</analysis>".len();
    format!("{}{}", &block[..start], &block[close_end..])
}

fn extract_json_patch_section(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    if let Some(start) = lower.find("<jsonpatch") {
        if let Some(open_end) = lower[start..].find('>').map(|offset| start + offset) {
            if let Some(close_start) = lower[open_end + 1..]
                .find("</jsonpatch>")
                .map(|offset| open_end + 1 + offset)
            {
                return text[open_end + 1..close_start].to_owned();
            }
        }
    }
    text.to_owned()
}

/// 剝掉包住整段候選文字的 ``` 圍欄（含 ```json 這種帶語言標記的開頭）。
fn strip_code_fences(text: &str) -> String {
    let trimmed = text.trim();
    let Some(rest) = trimmed.strip_prefix("```") else {
        return trimmed.to_owned();
    };
    let rest = match rest.find('\n') {
        Some(index) => &rest[index + 1..],
        None => rest,
    };
    rest.strip_suffix("```").unwrap_or(rest).trim().to_owned()
}

struct Scan<'a> {
    objects: Vec<&'a str>,
    /// 最後一個 `{` 起沒收尾（缺 `}`、引號沒收）的那段
    unclosed: Option<&'a str>,
}

/// 逐個掃出最外層平衡的 `{…}` 物件，正確跳過字串內的引號與跳脫字元。
fn scan_balanced_objects(text: &str) -> Scan<'_> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut result = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index].1 != '{' {
            index += 1;
            continue;
        }
        let start = chars[index].0;
        let mut depth = 0i32;
        let mut in_string = false;
        let mut escape = false;
        let mut end = None;
        let mut cursor = index;
        while cursor < chars.len() {
            let (byte_pos, character) = chars[cursor];
            if in_string {
                if escape {
                    escape = false;
                } else if character == '\\' {
                    escape = true;
                } else if character == '"' {
                    in_string = false;
                }
            } else {
                match character {
                    '"' => in_string = true,
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(byte_pos + character.len_utf8());
                            break;
                        }
                    }
                    _ => {}
                }
            }
            cursor += 1;
        }
        match end {
            Some(end) => {
                result.push(&text[start..end]);
                index = cursor + 1;
            }
            None => {
                // 沒收尾：後面已經壞掉，寧可少收也不要瞎猜
                return Scan {
                    objects: result,
                    unclosed: Some(&text[start..]),
                };
            }
        }
    }
    Scan {
        objects: result,
        unclosed: None,
    }
}

fn to_parsed(value: &serde_json::Value) -> Parsed {
    match value_to_patch(value) {
        Ok(patch) => Parsed::Patch(patch),
        Err(record) => Parsed::Note(record),
    }
}

/// 一個 JSON 值 → 指令；認不出來就是拒收記錄（告訴 GM 缺什麼）。
/// 照上游：`add` 視同 insert；move 的目的地是 `path ?? to`（path 缺或 null 才看 to）。
fn value_to_patch(value: &serde_json::Value) -> Result<Patch, Record> {
    use serde_json::Value;
    let Some(obj) = value.as_object() else {
        return Err(reject(
            BLOCK_PATH,
            format!(
                "更新區塊裡有一筆不是 JSON 物件（{}），未套用。",
                snippet(&value.to_string(), 30)
            ),
        ));
    };
    let label = obj
        .get("path")
        .and_then(Value::as_str)
        .or_else(|| obj.get("to").and_then(Value::as_str))
        .unwrap_or(BLOCK_PATH);
    // 句中點名是哪一筆（同句去重後 GM 才分得出來）：抓得到 path／to／from 就寫進去
    let at = ["path", "to", "from"]
        .iter()
        .find_map(|field| {
            obj.get(*field)
                .and_then(Value::as_str)
                .map(|raw| format!("（{field} {}）", snippet(raw, 60)))
        })
        .unwrap_or_default();
    let Some(name) = obj.get("op").and_then(Value::as_str) else {
        return Err(reject(
            label,
            format!("有一筆更新缺 op 或 op 不是字串{at}，未套用；{OPS}。"),
        ));
    };
    let op = match name {
        "replace" => PatchOp::Replace,
        "delta" => PatchOp::Delta,
        "insert" | "add" => PatchOp::Insert,
        "remove" => PatchOp::Remove,
        "move" => PatchOp::Move,
        other => {
            return Err(reject(
                label,
                format!("op「{}」無法辨識{at}，未套用；{OPS}。", snippet(other, 20)),
            ))
        }
    };
    if op == PatchOp::Move {
        let from = match obj.get("from") {
            Some(Value::String(raw)) => parse_pointer(raw),
            _ => Vec::new(),
        };
        if from.is_empty() {
            return Err(reject(label, format!("move 缺來源路徑 from{at}，未套用。")));
        }
        let target = match obj.get("path") {
            None | Some(Value::Null) => obj.get("to"),
            Some(Value::String(_)) => obj.get("path"),
            Some(_) => {
                return Err(reject(
                    label,
                    format!("move 的 path 不是字串{at}，未套用。"),
                ));
            }
        };
        let path = match target {
            Some(Value::String(raw)) => parse_pointer(raw),
            _ => Vec::new(),
        };
        if path.is_empty() {
            return Err(reject(label, format!("move 缺目的地 path{at}，未套用。")));
        }
        return Ok(Patch {
            op,
            path,
            from,
            value: None,
        });
    }
    let path = match obj.get("path") {
        Some(Value::String(raw)) => parse_pointer(raw),
        _ => {
            return Err(reject(
                label,
                format!("{name} 缺 path 或 path 不是字串{at}，未套用。"),
            ))
        }
    };
    if path.is_empty() {
        return Err(reject(label, format!("{name} 的 path 是空的，未套用。")));
    }
    Ok(Patch {
        op,
        path,
        from: Vec::new(),
        value: obj.get("value").cloned(),
    })
}

fn reject(path: &str, detail: String) -> Record {
    Record::new(RecordKind::Rejected, path.to_owned(), plain(detail))
}

/// JSON Pointer 拆段：`~1`→`/`、`~0`→`~`，開頭的 `/` 忽略；
/// 容錯：整串沒有 `/` 但有 `.` 時改用 `.` 拆（模型有時給點分路徑）。
fn parse_pointer(raw: &str) -> Vec<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Vec::new();
    }
    if !raw.contains('/') && raw.contains('.') {
        return raw
            .split('.')
            .map(str::to_owned)
            .filter(|segment| !segment.is_empty())
            .collect();
    }
    let raw = raw.strip_prefix('/').unwrap_or(raw);
    if raw.is_empty() {
        return Vec::new();
    }
    raw.split('/')
        .map(|segment| segment.replace("~1", "/").replace("~0", "~"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- parse_updates ----

    #[test]
    fn parse_updates_reads_all_five_ops_from_the_documented_shape() {
        let block = r#"<Analysis>english analysis, discarded</Analysis>
<JSONPatch>
[
  { "op": "replace", "path": "/World/Location", "value": "晨港" },
  { "op": "delta",   "path": "/Heroes/亚瑟·晨光/Affection", "value": 5 },
  { "op": "insert",  "path": "/Player/Inventory/藥水", "value": { "描述": "紅色的", "數量": 2 } },
  { "op": "remove",  "path": "/Player/Inventory/舊劍" },
  { "op": "move",    "from": "/NPCs/A", "to": "/Heroes/鴉" }
]
</JSONPatch>"#;
        let patches = parse_updates(block);
        assert_eq!(patches.len(), 5);
        assert_eq!(patches[0].op, PatchOp::Replace);
        assert_eq!(patches[0].path, vec!["World", "Location"]);
        assert_eq!(patches[1].op, PatchOp::Delta);
        assert_eq!(patches[1].path, vec!["Heroes", "亚瑟·晨光", "Affection"]);
        assert_eq!(patches[2].op, PatchOp::Insert);
        assert_eq!(patches[3].op, PatchOp::Remove);
        assert_eq!(patches[3].path, vec!["Player", "Inventory", "舊劍"]);
        assert_eq!(patches[4].op, PatchOp::Move);
        assert_eq!(patches[4].from, vec!["NPCs", "A"]);
        assert_eq!(patches[4].path, vec!["Heroes", "鴉"]);
    }

    #[test]
    fn parse_updates_skips_the_broken_object_but_keeps_the_rest() {
        // 陣列漏逗號：整段 parse 會失敗，退回逐個掃物件，壞的跳過、好的照收。
        let block = r#"<JSONPatch>[{"op":"delta","path":"/a","value":1} {"op":"delta","path":"/b","value":2}]</JSONPatch>"#;
        let patches = parse_updates(block);
        assert_eq!(patches.len(), 2);
        assert_eq!(patches[0].path, vec!["a"]);
        assert_eq!(patches[1].path, vec!["b"]);
    }

    #[test]
    fn parse_updates_accepts_dot_path_and_code_fence_without_jsonpatch_tag() {
        let block = "```json\n[{\"op\":\"delta\",\"path\":\"World.HP\",\"value\":-3}]\n```";
        let patches = parse_updates(block);
        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0].path, vec!["World", "HP"]);
    }

    #[test]
    fn parse_updates_returns_empty_on_garbage() {
        assert!(parse_updates("這回覆完全沒有 JSON。").is_empty());
    }
}
