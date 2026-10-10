//! ST 巨集語法剖析，照網頁版 `macro-parser.ts`（ST 06bde939 MacroLexer／MacroParser）：
//! `{{` 旗標* （變數簡寫 | 名字 參數?） `}}`；參數是 `::` 分隔的清單，或可帶一個 `:` 起頭、可含 `::` 的
//! 單一參數；參數裡可以巢狀巨集，前後空白不算進參數。沒收尾的 `{{` 當純文字。
//! 位置一律是位元組位置（語法字元都是 ASCII，與網頁版的碼元位置一一對應）。

use crate::world_info::js_semantics::is_js_whitespace;

/// 巢狀上限：超過就整段求值放棄、原文照回（JS 在這裡是堆疊溢位被 `evaluate` 接住）。
pub const MAX_DEPTH: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Abort;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub start: usize,
    /// 不含
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VariableExpr {
    pub global: bool,
    pub name: String,
    pub operator: Option<&'static str>,
    pub value: Option<Range>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MacroNode {
    pub start: usize,
    pub end: usize,
    /// `{{` 之後、`}}` 之前
    pub inner_start: usize,
    pub inner_end: usize,
    pub flags: Vec<char>,
    /// 變數簡寫（`.x`／`$x`）時為空字串
    pub name: String,
    /// 每個參數去掉前後空白的範圍；空參數＝`None`
    pub args: Vec<Option<Range>>,
    /// 第一個參數的第一個字（`{{///}}` 判定收尾用）
    pub first_arg_char: Option<char>,
    pub variable: Option<VariableExpr>,
}

const FLAG_CHARS: &[char] = &['!', '?', '~', '#', '/', '>'];
const OPERATORS: &[&str] = &[
    "++", "--", "??=", "??", "||=", "||", "-=", "==", "!=", ">=", ">", "<=", "<", "+=", "=",
];

fn char_at(text: &str, at: usize) -> Option<char> {
    text.get(at..).and_then(|rest| rest.chars().next())
}

fn is_space(text: &str, at: usize) -> bool {
    char_at(text, at).is_some_and(is_js_whitespace)
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// `/[a-zA-Z][\w-]*/y`
fn identifier(text: &str, at: usize) -> Option<&str> {
    let bytes = text.as_bytes();
    if !bytes.get(at).is_some_and(u8::is_ascii_alphabetic) {
        return None;
    }
    let mut end = at + 1;
    while bytes.get(end).is_some_and(|b| is_word(*b) || *b == b'-') {
        end += 1;
    }
    Some(&text[at..end])
}

/// `/[a-zA-Z](?:[\w-]*\w)?/y`：最長、且結尾是字元的那一段。
fn var_identifier(text: &str, at: usize) -> Option<&str> {
    let found = identifier(text, at)?;
    Some(found.trim_end_matches('-'))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    Sep,
    Colon,
    Text,
}

#[derive(Clone, Copy)]
struct Token {
    kind: TokenKind,
    start: usize,
    end: usize,
}

/// 參數區：讀到同層的 `}}` 為止；巢狀巨集整段算一個 token。剖不出來回 `None`。
fn scan_args(text: &str, from: usize, depth: usize) -> Result<Option<(Vec<Token>, usize)>, Abort> {
    let mut tokens = Vec::new();
    let mut at = from;
    while at < text.len() {
        let rest = &text[at..];
        if rest.starts_with("}}") {
            return Ok(Some((tokens, at)));
        }
        if rest.starts_with("{{") {
            let Some(nested) = parse_macro_at_depth(text, at, depth + 1)? else {
                return Ok(None);
            };
            tokens.push(Token {
                kind: TokenKind::Text,
                start: nested.start,
                end: nested.end,
            });
            at = nested.end;
            continue;
        }
        let ch = rest.chars().next().expect("at < len");
        if is_js_whitespace(ch) {
            at += ch.len_utf8();
            continue;
        }
        if rest.starts_with("::") {
            tokens.push(Token {
                kind: TokenKind::Sep,
                start: at,
                end: at + 2,
            });
            at += 2;
            continue;
        }
        let kind = if ch == ':' {
            TokenKind::Colon
        } else {
            TokenKind::Text
        };
        tokens.push(Token {
            kind,
            start: at,
            end: at + ch.len_utf8(),
        });
        at += ch.len_utf8();
    }
    Ok(None)
}

fn span(tokens: &[Token]) -> Option<Range> {
    match (tokens.first(), tokens.last()) {
        (Some(first), Some(last)) => Some(Range {
            start: first.start,
            end: last.end,
        }),
        _ => None,
    }
}

fn split_args(tokens: &[Token]) -> Vec<Option<Range>> {
    let Some(first) = tokens.first() else {
        return Vec::new();
    };
    if first.kind == TokenKind::Sep {
        let mut args = Vec::new();
        let mut current: Vec<Token> = Vec::new();
        for token in &tokens[1..] {
            if token.kind == TokenKind::Sep {
                args.push(span(&current));
                current.clear();
            } else {
                current.push(*token);
            }
        }
        args.push(span(&current));
        return args;
    }
    let rest = if first.kind == TokenKind::Colon {
        &tokens[1..]
    } else {
        tokens
    };
    if rest.is_empty() {
        Vec::new()
    } else {
        vec![span(rest)]
    }
}

fn skip_spaces(text: &str, mut at: usize) -> usize {
    while let Some(ch) = char_at(text, at).filter(|ch| is_js_whitespace(*ch)) {
        at += ch.len_utf8();
    }
    at
}

fn parse_variable(
    text: &str,
    at: usize,
    start: usize,
    flags: Vec<char>,
    depth: usize,
) -> Result<Option<MacroNode>, Abort> {
    let global = text.as_bytes()[at] == b'$';
    let mut cursor = skip_spaces(text, at + 1);
    let Some(name) = var_identifier(text, cursor) else {
        return Ok(None);
    };
    let name = name.to_owned();
    cursor = skip_spaces(text, cursor + name.len());
    let mut operator = None;
    let mut value = None;
    if !text[cursor..].starts_with("}}") {
        let Some(found) = OPERATORS
            .iter()
            .find(|candidate| text[cursor..].starts_with(**candidate))
        else {
            return Ok(None);
        };
        operator = Some(*found);
        cursor += found.len();
        if *found == "++" || *found == "--" {
            cursor = skip_spaces(text, cursor);
            if !text[cursor..].starts_with("}}") {
                return Ok(None);
            }
        } else {
            let Some((tokens, end)) = scan_args(text, cursor, depth)? else {
                return Ok(None);
            };
            value = span(&tokens);
            cursor = end;
        }
    }
    Ok(Some(MacroNode {
        start,
        end: cursor + 2,
        inner_start: start + 2,
        inner_end: cursor,
        flags,
        name: String::new(),
        args: Vec::new(),
        first_arg_char: None,
        variable: Some(VariableExpr {
            global,
            name,
            operator,
            value,
        }),
    }))
}

/// 從 `{{` 起剖一個完整巨集；語法不成立或沒收尾回 `None`（呼叫端把 `{{` 當純文字）。
pub fn parse_macro_at(text: &str, start: usize) -> Result<Option<MacroNode>, Abort> {
    parse_macro_at_depth(text, start, 0)
}

fn parse_macro_at_depth(
    text: &str,
    start: usize,
    depth: usize,
) -> Result<Option<MacroNode>, Abort> {
    if depth > MAX_DEPTH {
        return Err(Abort);
    }
    let mut at = start + 2;
    let mut flags = Vec::new();
    let mut name: Option<String> = None;
    while at < text.len() && name.is_none() {
        let rest = &text[at..];
        if rest.starts_with("}}") {
            return Ok(None);
        }
        if rest.starts_with("//") {
            name = Some("//".to_owned());
            at += 2;
            break;
        }
        let ch = rest.chars().next().expect("at < len");
        if ch == '.' || ch == '$' {
            return parse_variable(text, at, start, flags, depth);
        }
        if FLAG_CHARS.contains(&ch) {
            flags.push(ch);
            at += 1;
        } else if is_js_whitespace(ch) {
            at += ch.len_utf8();
        } else {
            let Some(found) = identifier(text, at) else {
                return Ok(None);
            };
            name = Some(found.to_owned());
            at += found.len();
            let next = char_at(text, at);
            let closes = text[at..].starts_with("}}");
            let allowed = matches!(next, Some(':' | '|' | '}')) || is_space(text, at);
            if !closes && !allowed {
                return Ok(None);
            }
        }
    }
    let Some(name) = name else {
        return Ok(None);
    };
    let Some((tokens, end)) = scan_args(text, at, depth)? else {
        return Ok(None);
    };
    let args = split_args(&tokens);
    let first_arg_char = tokens
        .iter()
        .find(|token| token.kind == TokenKind::Text)
        .and_then(|token| char_at(text, token.start));
    Ok(Some(MacroNode {
        start,
        end: end + 2,
        inner_start: start + 2,
        inner_end: end,
        flags,
        name,
        args,
        first_arg_char,
        variable: None,
    }))
}

/// 文件裡同層的完整巨集（依位置）；巢狀在參數裡的不列（求值時重剖參數文字）。
pub fn parse_document(text: &str) -> Result<Vec<MacroNode>, Abort> {
    let mut nodes = Vec::new();
    let mut at = 0;
    while at < text.len() {
        let Some(open) = text[at..].find("{{").map(|offset| at + offset) else {
            break;
        };
        // 後面已經沒有 `}}`：不可能再有完整巨集
        if !text[open + 2..].contains("}}") {
            break;
        }
        // `{{{x}}`：最前面的 `{` 是純文字
        let mut start = open;
        while text[start..].starts_with("{{{") {
            start += 1;
        }
        match parse_macro_at(text, start)? {
            Some(node) => {
                at = node.end;
                nodes.push(node);
            }
            None => at = start + 2,
        }
    }
    Ok(nodes)
}
