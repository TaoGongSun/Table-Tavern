use super::types::{Patch, PatchOp};

// ---------------------------------------------------------------------
// 解析：<UpdateVariable> 原文 → Vec<Patch>
// ---------------------------------------------------------------------

/// 從 `<UpdateVariable>` 原始內容取出更新指令；解析不出來就回空 Vec，絕不 panic。
pub fn parse_updates(block: &str) -> Vec<Patch> {
    let without_analysis = strip_analysis(block);
    let section = extract_json_patch_section(&without_analysis);
    let candidate = strip_code_fences(&section);
    let candidate = candidate.trim();

    let objects: Vec<serde_json::Value> =
        match serde_json::from_str::<Vec<serde_json::Value>>(candidate) {
            Ok(values) => values,
            // 模型漏逗號等格式壞掉：退回逐個掃出最外層平衡的 {…} 物件，好的照收壞的跳過。
            Err(_) => scan_balanced_objects(candidate)
                .into_iter()
                .filter_map(|text| serde_json::from_str::<serde_json::Value>(text).ok())
                .collect(),
        };

    objects.iter().filter_map(value_to_patch).collect()
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

/// 逐個掃出最外層平衡的 `{…}` 物件，正確跳過字串內的引號與跳脫字元。
fn scan_balanced_objects(text: &str) -> Vec<&str> {
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
            None => break, // 沒收尾：後面已經壞掉，寧可少收也不要瞎猜
        }
    }
    result
}

fn value_to_patch(value: &serde_json::Value) -> Option<Patch> {
    let obj = value.as_object()?;
    let op = match obj.get("op")?.as_str()? {
        "replace" => PatchOp::Replace,
        "delta" => PatchOp::Delta,
        "insert" => PatchOp::Insert,
        "remove" => PatchOp::Remove,
        "move" => PatchOp::Move,
        _ => return None,
    };
    if op == PatchOp::Move {
        let from = obj
            .get("from")
            .and_then(|v| v.as_str())
            .map(parse_pointer)?;
        let path = obj.get("to").and_then(|v| v.as_str()).map(parse_pointer)?;
        if from.is_empty() || path.is_empty() {
            return None;
        }
        return Some(Patch {
            op,
            path,
            from,
            value: None,
        });
    }
    let path = obj
        .get("path")
        .and_then(|v| v.as_str())
        .map(parse_pointer)?;
    if path.is_empty() {
        return None;
    }
    Some(Patch {
        op,
        path,
        from: Vec::new(),
        value: obj.get("value").cloned(),
    })
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
