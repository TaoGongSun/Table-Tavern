//! 給畫面看的後端訊息：回代碼與參數，翻譯交給前端（src/shared/ui/backend-text.ts）。
//! 字串格式是 `TTMSG:` 接 serde JSON，仍走既有的 `Result<_, String>`／`invalid_data`。
//! code 一旦落檔就是持久契約，不得改名；新增變體要在 src/i18n/features/backend-msg.ts
//! 補十語系 `be_<code>` 與參數表，`npm run check:i18n` 會從這支檔抽 code 與欄位核對。
//! 包裹別人的錯誤一律用名為 `error` 的欄位：前端只對這個欄位做巢狀翻譯，其餘參數原文代入。
// 呼叫端由後續分包逐批接上，先放行未使用警告。
#![allow(dead_code)]

use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::data::invalid_data;

pub const MARK: &str = "TTMSG:";
/// 巢狀翻譯最多幾層；超過就原文保留，跟前端一致。
const MAX_DEPTH: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum UiMsg {
    /// 讀寫檔案失敗；error 是系統錯誤原文或另一則 TTMSG。
    IoFailed { error: String },
}

impl fmt::Display for UiMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_string(self).map_err(|_| fmt::Error)?;
        write!(f, "{MARK}{json}")
    }
}

impl From<UiMsg> for String {
    fn from(msg: UiMsg) -> Self {
        msg.to_string()
    }
}

impl UiMsg {
    /// 給 `DataResult`／`?` 用，等同 `invalid_data(msg.to_string())`。
    pub fn into_error(self) -> Box<dyn Error + Send + Sync> {
        invalid_data(self.to_string())
    }

    /// 送 AI 的英文短句。
    fn ai_text(&self, depth: usize) -> String {
        match self {
            UiMsg::IoFailed { error } => {
                format!("File read/write failed: {}", nested(error, depth))
            }
        }
    }

    /// 把字串裡的 TTMSG 換成英文短句，給提示詞用。舊中文、未知或壞碼原樣保留。
    pub fn ai_text_from_str(text: &str) -> String {
        render_ai(text, 0)
    }
}

fn nested(error: &str, depth: usize) -> String {
    if depth + 1 >= MAX_DEPTH {
        error.to_owned()
    } else {
        render_ai(error, depth + 1)
    }
}

fn render_ai(text: &str, depth: usize) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find(MARK) {
        out.push_str(&rest[..at]);
        let body = &rest[at + MARK.len()..];
        // 括號不成對或 JSON 語法壞掉：只跳過標記本身，裡面可能還夾著合法標記。
        let Some(end) = json_object_end(body)
            .filter(|&end| serde_json::from_str::<serde_json::Value>(&body[..end]).is_ok())
        else {
            out.push_str(MARK);
            rest = body;
            continue;
        };
        // 合法 JSON 但不是認得的訊息：整段原文保留。
        match serde_json::from_str::<UiMsg>(&body[..end]) {
            Ok(msg) => out.push_str(&msg.ai_text(depth)),
            Err(_) => out.push_str(&rest[at..at + MARK.len() + end]),
        }
        rest = &body[end..];
    }
    out.push_str(rest);
    out
}

/// `text` 以 `{` 開頭時，回傳對應右括號之後的位元組位置；跨過字串內的括號與跳脫。
fn json_object_end(text: &str) -> Option<usize> {
    if !text.starts_with('{') {
        return None;
    }
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (index, ch) in text.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn io(error: impl Into<String>) -> UiMsg {
        UiMsg::IoFailed {
            error: error.into(),
        }
    }

    #[test]
    fn display_is_mark_plus_tagged_json() {
        assert_eq!(
            io("磁碟已滿").to_string(),
            r#"TTMSG:{"code":"io_failed","error":"磁碟已滿"}"#
        );
        let as_string: String = io("x").into();
        assert!(as_string.starts_with(MARK));
        assert_eq!(io("x").into_error().to_string(), io("x").to_string());
    }

    #[test]
    fn ai_text_translates_known_codes_and_keeps_the_rest() {
        let text = format!("前文 {} 後文", io("disk full"));
        assert_eq!(
            UiMsg::ai_text_from_str(&text),
            "前文 File read/write failed: disk full 後文"
        );
        assert_eq!(UiMsg::ai_text_from_str("舊的中文說明"), "舊的中文說明");
    }

    #[test]
    fn ai_text_keeps_unknown_or_broken_marks_and_reads_later_good_ones() {
        let unknown = r#"TTMSG:{"code":"nope","error":"x"}"#;
        assert_eq!(UiMsg::ai_text_from_str(unknown), unknown);
        let wrong_type = r#"TTMSG:{"code":"io_failed","error":3}"#;
        assert_eq!(UiMsg::ai_text_from_str(wrong_type), wrong_type);
        let text = format!("TTMSG:{{broken {}", io("a"));
        assert_eq!(
            UiMsg::ai_text_from_str(&text),
            "TTMSG:{broken File read/write failed: a"
        );
    }

    #[test]
    fn ai_text_reads_good_marks_inside_a_broken_balanced_one() {
        let text = format!("TTMSG:{{bad {}}}", io("ok"));
        assert_eq!(
            UiMsg::ai_text_from_str(&text),
            "TTMSG:{bad File read/write failed: ok}"
        );
    }

    #[test]
    fn ai_text_nests_error_up_to_the_depth_limit() {
        let deep = io(io(io(io("root").to_string()).to_string()).to_string()).to_string();
        let rendered = UiMsg::ai_text_from_str(&deep);
        assert!(rendered.starts_with(
            "File read/write failed: File read/write failed: File read/write failed: "
        ));
        assert!(rendered.ends_with(&io("root").to_string()), "{rendered}");
    }

    #[test]
    fn json_scanner_skips_braces_inside_strings() {
        let text = format!("{} tail", io(r#"a } b \" { c"#));
        assert_eq!(
            UiMsg::ai_text_from_str(&text),
            r#"File read/write failed: a } b \" { c tail"#
        );
    }
}
