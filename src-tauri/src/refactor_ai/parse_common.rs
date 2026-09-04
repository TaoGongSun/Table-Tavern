/// 標記式輸出裡的一個區塊：`## MARKER` 或 `## MARKER: value` 開頭，直到下一個認得的標記或結尾。
pub(super) struct Block {
    pub(super) marker: &'static str,
    pub(super) value: String,
    pub(super) lines: Vec<String>,
}

/// 通用標記掃描：只認 `markers` 清單裡的名字（大小寫不拘、`#` 數量不拘、標記後可接半形或全形冒號接值），
/// 標記之外（含最前面模型的寒暄「好的，以下是……」）一律略過，不會 panic。同一個標記可以出現多次，各自
/// 成一塊。
pub(super) fn parse_blocks(raw: &str, markers: &[&'static str]) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut current: Option<usize> = None;
    for line in raw.lines() {
        let rest = trim_heading_prefix(line);
        if let Some((marker, value)) = match_marker(rest, markers) {
            blocks.push(Block {
                marker,
                value: value.to_owned(),
                lines: Vec::new(),
            });
            current = Some(blocks.len() - 1);
            continue;
        }
        if let Some(index) = current {
            blocks[index].lines.push(line.to_owned());
        }
    }
    blocks
}

/// 吃掉開頭的 `#` 與空白；超過 6 個井字號視為非標題（比照 genesis.rs 的同款寫法）。
pub(super) fn trim_heading_prefix(line: &str) -> &str {
    let mut hashes = 0;
    for (index, character) in line.char_indices() {
        if character == '#' {
            hashes += 1;
            if hashes > 6 {
                return "";
            }
        } else if !character.is_whitespace() {
            return &line[index..];
        }
    }
    ""
}

pub(super) fn match_marker<'a>(
    line: &'a str,
    markers: &[&'static str],
) -> Option<(&'static str, &'a str)> {
    for &marker in markers {
        let Some(head) = line.get(..marker.len()) else {
            continue;
        };
        if !head.eq_ignore_ascii_case(marker) {
            continue;
        }
        let after = line[marker.len()..].trim_start();
        let value = after
            .strip_prefix(':')
            .or_else(|| after.strip_prefix('：'))
            .map(str::trim)
            .unwrap_or(after);
        return Some((marker, value));
    }
    None
}

pub(super) fn join_trim(lines: &[String]) -> String {
    lines.join("\n").trim().to_owned()
}

/// 剝掉 AI 常見的 ```json ... ``` 圍欄；沒有圍欄的內容原樣放行。
pub(super) fn strip_json_fence(text: &str) -> &str {
    let trimmed = text.trim();
    let trimmed = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```JSON"))
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed);
    trimmed.strip_suffix("```").unwrap_or(trimmed).trim()
}

/// 剝掉 AI 常見的 ```html ... ``` 圍欄；沒有圍欄的內容原樣放行。截斷輸出（沒有結尾 ``` ）
/// 一樣安全：strip_suffix 找不到就原樣放行，不 panic。
pub(super) fn strip_html_fence(text: &str) -> &str {
    let trimmed = text.trim();
    let Some(rest) = trimmed.strip_prefix("```") else {
        return trimmed.strip_suffix("```").unwrap_or(trimmed).trim();
    };
    // 開頭那行剩下的語言標記（```xml 的 xml、```html 的 html）連著換行一起剝掉——
    // 只剝反引號會把標記留在骨架第一行，跟著寫進 interface-shell.html。
    let body = match rest.split_once('\n') {
        Some((tag, body)) if tag.trim().chars().all(|char| char.is_ascii_alphanumeric()) => body,
        _ => rest,
    };
    body.strip_suffix("```").unwrap_or(body).trim()
}

/// 缺席的區塊視為「這份沒有內容」給空集合（不是失敗）；有出現但解析不出來才是真的失敗。
pub(super) fn parse_json_block<T: Default + serde::de::DeserializeOwned>(
    block: Option<&Block>,
) -> Option<T> {
    let Some(block) = block else {
        return Some(T::default());
    };
    let text = join_trim(&block.lines);
    if text.is_empty() {
        Some(T::default())
    } else {
        serde_json::from_str(strip_json_fence(&text)).ok()
    }
}
