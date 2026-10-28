//! 各家 CLI 認得的事件形狀（plans/cli-version-gate.md 包 1 第 3 點）。清單＝夾具出現過的型別 ∪
//! 解析器有分派的型別（codex 另含 0.164.0 原始碼 exec_events.rs 列舉的全部型別）。
//! 只看頂層判別鍵與解析器有分派的子型別；子型別改名、判別鍵缺失或不是字串、非 JSON 行都算認不得。
//! 回 Some(種類標籤) 給記錄與錯誤判定用，空白行當沒事。

use serde_json::Value;

const CLAUDE_TYPES: &[&str] = &[
    "system",
    "stream_event",
    "assistant",
    "user",
    "result",
    "rate_limit_event",
];
const CLAUDE_STREAM_EVENTS: &[&str] = &[
    "message_start",
    "content_block_start",
    "content_block_delta",
    "content_block_stop",
    "message_delta",
    "message_stop",
    "ping",
    "error",
];
const CLAUDE_DELTAS: &[&str] = &[
    "text_delta",
    "thinking_delta",
    "signature_delta",
    "input_json_delta",
    "citations_delta",
];

const CODEX_TYPES: &[&str] = &[
    "thread.started",
    "turn.started",
    "turn.completed",
    "turn.failed",
    "item.started",
    "item.updated",
    "item.completed",
    "error",
];
const CODEX_ITEMS: &[&str] = &[
    "agent_message",
    "reasoning",
    "command_execution",
    "file_change",
    "mcp_tool_call",
    "collab_tool_call",
    "web_search",
    "todo_list",
    "error",
];

const AGY_EVENTS: &[&str] = &["init", "step_update", "result"];
const AGY_STEPS: &[&str] = &[
    "user_input",
    "agent_response",
    "tool",
    "subagent",
    "system_message",
];

const GROK_TYPES: &[&str] = &[
    "text",
    "thought",
    "tool_call",
    "tool_call_update",
    "usage",
    "plan",
    "available_commands",
    "max_turns_reached",
    "end",
    "error",
];
/// grok 文件寫的 `auto_compact_*` 一族
const GROK_TYPE_PREFIXES: &[&str] = &["auto_compact_"];

enum Parsed {
    Blank,
    NotJson,
    Json(Value),
}

fn parse(line: &str) -> Parsed {
    if line.trim().is_empty() {
        return Parsed::Blank;
    }
    match serde_json::from_str::<Value>(line) {
        Ok(value) if value.is_object() => Parsed::Json(value),
        _ => Parsed::NotJson,
    }
}

/// 判別鍵：缺失或不是字串回 Err(標籤)。
fn discriminator<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    match value.get(key) {
        Some(Value::String(kind)) => Ok(kind),
        Some(_) => Err(format!("{key}=<not-string>")),
        None => Err(format!("{key}=<missing>")),
    }
}

/// 判別鍵＋清單核對的共同骨架；`sub` 再看解析器有分派的子型別。
fn check(
    line: &str,
    key: &str,
    known: &[&str],
    prefixes: &[&str],
    sub: impl Fn(&str, &Value) -> Option<String>,
) -> Option<String> {
    let value = match parse(line) {
        Parsed::Blank => return None,
        Parsed::NotJson => return Some("<non-json>".to_owned()),
        Parsed::Json(value) => value,
    };
    let kind = match discriminator(&value, key) {
        Ok(kind) => kind,
        Err(label) => return Some(label),
    };
    if !known.contains(&kind) && !prefixes.iter().any(|prefix| kind.starts_with(prefix)) {
        return Some(format!("{key}={kind}"));
    }
    sub(kind, &value)
}

/// 必要的子判別鍵：路徑上必須有字串值且在清單內，缺欄也算認不得。
fn sub_kind(value: &Value, pointer: &str, label: &str, known: &[&str]) -> Option<String> {
    match value.pointer(pointer) {
        None => Some(format!("{label}=<missing>")),
        Some(Value::String(kind)) if known.contains(&kind.as_str()) => None,
        Some(Value::String(kind)) => Some(format!("{label}={kind}")),
        Some(_) => Some(format!("{label}=<not-string>")),
    }
}

pub(super) fn claude(line: &str) -> Option<String> {
    check(line, "type", CLAUDE_TYPES, &[], |kind, value| {
        if kind != "stream_event" {
            return None;
        }
        // event.type 必要；delta.type 只有 content_block_delta 必要（其餘事件沒有 delta 或 delta 不帶型別）
        sub_kind(value, "/event/type", "event.type", CLAUDE_STREAM_EVENTS).or_else(|| {
            (value.pointer("/event/type").and_then(Value::as_str) == Some("content_block_delta"))
                .then(|| sub_kind(value, "/event/delta/type", "delta.type", CLAUDE_DELTAS))
                .flatten()
        })
    })
}

pub(super) fn codex(line: &str) -> Option<String> {
    check(line, "type", CODEX_TYPES, &[], |kind, value| {
        if !kind.starts_with("item.") {
            return None;
        }
        sub_kind(value, "/item/type", "item.type", CODEX_ITEMS)
    })
}

pub(super) fn agy(line: &str) -> Option<String> {
    check(line, "event", AGY_EVENTS, &[], |kind, value| {
        if kind != "step_update" {
            return None;
        }
        sub_kind(value, "/step_update/step_type", "step_type", AGY_STEPS)
    })
}

pub(super) fn grok(line: &str) -> Option<String> {
    check(line, "type", GROK_TYPES, GROK_TYPE_PREFIXES, |_, _| None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_known_shapes_and_flags_the_rest() {
        assert_eq!(claude(""), None);
        assert_eq!(claude("not json"), Some("<non-json>".to_owned()));
        assert_eq!(claude("[1]"), Some("<non-json>".to_owned()));
        assert_eq!(claude(r#"{"x":1}"#), Some("type=<missing>".to_owned()));
        assert_eq!(
            claude(r#"{"type":3}"#),
            Some("type=<not-string>".to_owned())
        );
        assert_eq!(claude(r#"{"type":"result"}"#), None);
        assert_eq!(claude(r#"{"type":"novel"}"#), Some("type=novel".to_owned()));
        assert_eq!(
            claude(
                r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_v2"}}}"#
            ),
            Some("delta.type=text_v2".to_owned())
        );
        assert_eq!(
            codex(r#"{"type":"item.completed","item":{"type":"assistant_message","text":"x"}}"#),
            Some("item.type=assistant_message".to_owned())
        );
        assert_eq!(
            codex(r#"{"type":"item.completed","item":{"type":"agent_message"}}"#),
            None
        );
        assert_eq!(
            agy(r#"{"type":"result"}"#),
            Some("event=<missing>".to_owned())
        );
        assert_eq!(
            agy(r#"{"event":"step_update","step_update":{"step_type":"planner"}}"#),
            Some("step_type=planner".to_owned())
        );
        assert_eq!(
            codex(r#"{"type":"item.completed","item":{"text":"x"}}"#),
            Some("item.type=<missing>".to_owned())
        );
        assert_eq!(
            agy(r#"{"event":"step_update","step_update":{}}"#),
            Some("step_type=<missing>".to_owned())
        );
        assert_eq!(
            claude(r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{}}}"#),
            Some("delta.type=<missing>".to_owned())
        );
        assert_eq!(
            claude(r#"{"type":"stream_event","event":{"delta":{"type":"text_delta"}}}"#),
            Some("event.type=<missing>".to_owned())
        );
        // message_delta 的 delta 沒有 type：放行
        assert_eq!(
            claude(
                r#"{"type":"stream_event","event":{"type":"message_delta","delta":{"stop_reason":"end_turn"}}}"#
            ),
            None
        );
        assert_eq!(grok(r#"{"type":"auto_compact_started"}"#), None);
        assert_eq!(grok(r#"{"type":"done"}"#), Some("type=done".to_owned()));
    }
}
