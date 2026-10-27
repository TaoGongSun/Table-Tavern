//! 清單欄字串值裡的 YAML 鍵定位：只認一個明訂的有限子集，認得的才回鍵的位元組範圍，認不得整份回 None
//! （呼叫端就保留原文）。不做 YAML 往返重印——後端沒有 YAML 套件，重印會改引號與縮排，卡片腳本未必吃。
//!
//! 支援的子集（逐行）：
//! - 縮排只用半形空白；空白行、`#` 開頭的註解行略過。
//! - 區塊鍵 `鍵: 值`、`鍵:`（底下接更深一層的子節點），以及序列項目 `- 鍵: 值`、`- "純量"`、`- 純量`。
//! - 鍵是普通純量（不以引號、`{`、`[`、`|`、`>`、`&`、`*`、`!`、`?`、`%`、`@`、反引號開頭，鍵裡不含引號），
//!   鍵尾只認半形 `:`（後面接空白或行尾）；全形「：」不算。
//! - 值可以是空、普通純量、同一行收尾的雙引號字串（跳過引號裡的內容），或 `|`／`>` 區塊純量（底下縮排更深的
//!   行全是值，不當鍵）。
//!
//! 以下一律整份回 None：tab 縮排、單引號、flow（`{` `[`）、跨行雙引號、引號鍵、錨點／別名／標籤、
//! 文件分隔線、行內巢狀序列、`- ` 後面沒內容、縮排對不上（值後面接更深的行、夾在兩層之間）、
//! 映射層級出現不是鍵的行（多行普通純量）。

/// 一個鍵在原字串裡的位置：`start..end` 只涵蓋鍵那幾個字，冒號與值不在內。
#[derive(Debug, Clone, PartialEq)]
pub(super) struct KeySite {
    pub start: usize,
    pub end: usize,
    /// 所屬映射的編號：同一個映射裡的鍵編號相同（檢查改名會不會撞出重複鍵用）。
    pub mapping: usize,
    /// 結構位置：從欄位根到這個鍵所在映射的路徑，父鍵照原字、序列項目記成 `[]`
    /// （`- 人物:` 底下的 `名字` 是 `["[]", "人物"]`）。參考鍵只在同一個位置套用。
    pub parent: Vec<String>,
    /// 第幾行（1 起算），記帳用。
    pub line: usize,
}

/// 序列項目在結構位置裡的記號。
pub(super) const ITEM: &str = "[]";

struct Frame {
    col: usize,
    /// 映射編號；None＝序列（`-` 所在的欄）
    id: Option<usize>,
    /// 這個映射（或序列）的結構位置
    path: Vec<String>,
}

/// 上一行是空值的鍵：它所在的欄與它自己的結構位置（子節點的位置）。
type Open = Option<(usize, Vec<String>)>;

pub(super) fn scan(text: &str) -> Option<Vec<KeySite>> {
    let mut sites = Vec::new();
    let mut frames: Vec<Frame> = Vec::new();
    let mut next_id = 0usize;
    // 上一行是空值的鍵（下一行可以比它深一層）
    let mut open: Open = None;
    // `|`／`>` 區塊純量的擁有者欄位：縮排比它深的行（與空白行）都是值
    let mut block_owner: Option<usize> = None;
    let mut offset = 0usize;
    for (index, raw) in text.split('\n').enumerate() {
        let line_start = offset;
        offset += raw.len() + 1;
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let trimmed = line.trim_start_matches(' ');
        let indent = line.len() - trimmed.len();
        if let Some(owner) = block_owner {
            if trimmed.trim().is_empty() || indent > owner {
                continue;
            }
            block_owner = None;
        }
        if trimmed.trim().is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('\t') || trimmed.starts_with("---") || trimmed.starts_with("...") {
            return None;
        }
        let was_open = open.take();
        let (col, content, item) = match trimmed.strip_prefix('-') {
            Some(after) if after.starts_with(' ') => {
                let content = after.trim_start_matches(' ');
                if content.is_empty() || content.starts_with("- ") || content.starts_with('\t') {
                    return None;
                }
                (line.len() - content.len(), content, true)
            }
            Some("") => return None,
            _ => (indent, trimmed, false),
        };
        if item {
            enter_item(&mut frames, indent, &was_open)?;
        } else {
            while frames
                .last()
                .is_some_and(|frame| frame.col > col || (frame.id.is_none() && frame.col >= col))
            {
                frames.pop();
            }
        }
        let Some(key) = plain_key(content) else {
            // 不是鍵的行：只有序列項目的純量可以，而且要能確定整行收尾
            if !item || !scalar_closes(content)? {
                return None;
            }
            continue;
        };
        let (mapping, parent) = match frames.last() {
            Some(Frame {
                col: top,
                id: Some(id),
                path,
            }) if !item && *top == col => (*id, path.clone()),
            // 比目前層深：只能接在空值的鍵底下
            Some(Frame {
                col: top,
                id: Some(_),
                ..
            }) if !item
                && was_open
                    .as_ref()
                    .is_some_and(|(open_col, _)| open_col == top) =>
            {
                let path = was_open.map(|(_, path)| path).unwrap_or_default();
                push(&mut frames, &mut next_id, col, path)
            }
            None if !item => push(&mut frames, &mut next_id, col, Vec::new()),
            Some(Frame { id: None, path, .. }) if item => {
                let mut path = path.clone();
                path.push(ITEM.to_owned());
                push(&mut frames, &mut next_id, col, path)
            }
            _ => return None,
        };
        let value = content[key.len() + 1..].trim_matches(' ');
        match value_kind(value)? {
            Value::Empty => {
                let mut path = parent.clone();
                path.push(key.to_owned());
                open = Some((col, path));
            }
            Value::Block => block_owner = Some(col),
            Value::Scalar => {}
        }
        let start = line_start + (line.len() - content.len());
        sites.push(KeySite {
            start,
            end: start + key.len(),
            mapping,
            parent,
            line: index + 1,
        });
    }
    Some(sites)
}

/// 序列項目：同一欄已有序列就是兄弟項；否則開新序列，只能接在空值的鍵底下（同欄或更深）或在最外層。
fn enter_item(frames: &mut Vec<Frame>, indent: usize, was_open: &Open) -> Option<()> {
    while frames.last().is_some_and(|frame| frame.col > indent) {
        frames.pop();
    }
    let path = match frames.last() {
        Some(Frame { col, id: None, .. }) if *col == indent => return Some(()),
        Some(Frame {
            col, id: Some(_), ..
        }) if was_open
            .as_ref()
            .is_some_and(|(open_col, _)| open_col == col) =>
        {
            was_open
                .as_ref()
                .map(|(_, path)| path.clone())
                .unwrap_or_default()
        }
        None => Vec::new(),
        _ => return None,
    };
    frames.push(Frame {
        col: indent,
        id: None,
        path,
    });
    Some(())
}

fn push(
    frames: &mut Vec<Frame>,
    next_id: &mut usize,
    col: usize,
    path: Vec<String>,
) -> (usize, Vec<String>) {
    let id = *next_id;
    *next_id += 1;
    frames.push(Frame {
        col,
        id: Some(id),
        path: path.clone(),
    });
    (id, path)
}

/// 行首普通純量鍵（不含尾端冒號）；不是鍵或無法確定都回 None。
fn plain_key(content: &str) -> Option<&str> {
    let first = content.chars().next()?;
    if "\"'{[|>&*!?%@`#".contains(first) {
        return None;
    }
    let bytes = content.as_bytes();
    let mut end = None;
    for (position, ch) in content.char_indices() {
        match ch {
            '"' | '\'' => return None,
            '#' if position > 0 && bytes[position - 1] == b' ' => return None,
            ':' if bytes.get(position + 1).is_none_or(|next| *next == b' ') => {
                end = Some(position);
                break;
            }
            _ => {}
        }
    }
    let end = end?;
    let key = content[..end].trim_end_matches(' ');
    // 鍵與冒號之間有空白就不是這個子集（`鍵 :`），保守不認
    (!key.is_empty() && key.len() == end).then_some(key)
}

enum Value {
    Empty,
    Block,
    Scalar,
}

/// 冒號後面的值；無法確定（單引號、flow、跨行引號、錨點標籤）回 None。
fn value_kind(value: &str) -> Option<Value> {
    if value.is_empty() || value.starts_with('#') {
        return Some(Value::Empty);
    }
    match value.chars().next()? {
        '|' | '>' => {
            let rest = value[1..].trim_start_matches(|ch: char| "+-0123456789".contains(ch));
            let rest = rest.trim_start_matches(' ');
            (rest.is_empty() || rest.starts_with('#')).then_some(Value::Block)
        }
        '\'' | '{' | '[' | '&' | '*' | '!' | '%' | '@' | '`' => None,
        '"' => closes_quote(value).then_some(Value::Scalar),
        _ => Some(Value::Scalar),
    }
}

/// 序列項目的純量能不能確定在這一行收尾：雙引號要同行收尾，普通純量照收；其他開頭不認。
fn scalar_closes(content: &str) -> Option<bool> {
    match content.chars().next()? {
        '"' => Some(closes_quote(content)),
        '\'' | '{' | '[' | '|' | '>' | '&' | '*' | '!' | '%' | '@' | '`' => Some(false),
        _ => Some(true),
    }
}

/// 以雙引號開頭的值在同一行收尾，收尾後只剩空白或註解。
fn closes_quote(value: &str) -> bool {
    let mut escaped = false;
    for (position, ch) in value.char_indices().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => {
                let rest = value[position + 1..].trim_start_matches(' ');
                return rest.is_empty() || rest.starts_with('#');
            }
            _ => {}
        }
    }
    false
}
