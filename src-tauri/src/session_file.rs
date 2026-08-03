#![allow(dead_code)] // 後續 resume 流程接線後移除。

use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Claude CLI session JSONL 的完整保留表示。
#[derive(Debug)]
pub(crate) struct SessionFile {
    lines: Vec<Value>,
}

pub(crate) fn session_file_path(claude_home: &Path, cwd: &Path, session_id: &str) -> PathBuf {
    let munged_cwd: String = cwd
        .to_string_lossy()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect();
    claude_home
        .join("projects")
        .join(munged_cwd)
        .join(format!("{session_id}.jsonl"))
}

pub(crate) fn parse(text: &str) -> Result<SessionFile, String> {
    let mut lines = Vec::new();
    let mut previous_uuid: Option<String> = None;

    for (index, source_line) in text.lines().enumerate() {
        if source_line.trim().is_empty() {
            continue;
        }
        let line_number = index + 1;
        let value: Value = serde_json::from_str(source_line)
            .map_err(|error| format!("第 {line_number} 行不是有效 JSON：{error}"))?;
        validate_line(&value, line_number, previous_uuid.as_deref())?;
        if let Some(uuid) = conversation_uuid(&value) {
            previous_uuid = Some(uuid.to_owned());
        }
        lines.push(value);
    }

    Ok(SessionFile { lines })
}

pub(crate) fn serialize(session_file: &SessionFile) -> String {
    session_file
        .lines
        .iter()
        .map(|line| serde_json::to_string(line).expect("serde_json::Value must serialize"))
        .collect::<Vec<_>>()
        .join("\n")
        + if session_file.lines.is_empty() {
            ""
        } else {
            "\n"
        }
}

pub(crate) fn erase_user_segment(
    session_file: &mut SessionFile,
    uuid: &str,
    segment: &str,
) -> Result<(), String> {
    let line = find_conversation_line_mut(session_file, uuid)?;
    if conversation_type(line) != Some("user") {
        return Err(format!("uuid {uuid} 不是 user 對話行"));
    }

    let updated = {
        let content = line
            .pointer("/message/content")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("uuid {uuid} 的 user content 不是字串"))?;
        let occurrences = content.match_indices(segment).count();
        if occurrences != 1 {
            return Err(format!(
                "uuid {uuid} 的指定片段出現 {occurrences} 次，必須恰好一次"
            ));
        }
        content.replacen(segment, "", 1)
    };
    *line
        .pointer_mut("/message/content")
        .expect("validated user message content exists") = Value::String(updated);
    Ok(())
}

pub(crate) fn prefix_last_assistant(
    session_file: &mut SessionFile,
    prefix: &str,
) -> Result<(), String> {
    let line = session_file
        .lines
        .iter_mut()
        .rev()
        .find(|line| conversation_type(line) == Some("assistant"))
        .ok_or_else(|| "找不到 assistant 對話行".to_owned())?;
    let content = line
        .pointer_mut("/message/content")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "assistant content 不是陣列".to_owned())?;
    let first_segment = content
        .first_mut()
        .and_then(Value::as_object_mut)
        .filter(|segment| segment.get("type").and_then(Value::as_str) == Some("text"))
        .ok_or_else(|| "最後一條 assistant 的第一個分段不是 text 型".to_owned())?;
    let text = first_segment
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(|| "最後一條 assistant 的 text 分段沒有字串 text".to_owned())?;
    if !text.starts_with(prefix) {
        let updated = format!("{prefix}{text}");
        *first_segment
            .get_mut("text")
            .expect("text was checked as a string") = Value::String(updated);
    }
    Ok(())
}

pub(crate) fn truncate_from(session_file: &mut SessionFile, uuid: &str) -> Result<(), String> {
    let index = session_file
        .lines
        .iter()
        .position(|line| conversation_uuid(line) == Some(uuid))
        .ok_or_else(|| format!("找不到對話 uuid {uuid}"))?;
    session_file.lines.truncate(index);
    Ok(())
}

pub(crate) fn write_atomic(path: &Path, session_file: &SessionFile) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("session 檔路徑沒有父目錄：{}", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("session 檔路徑沒有有效檔名：{}", path.display()))?;
    let serialized = serialize(session_file);
    let mut temporary_path = None;
    for attempt in 0..100 {
        let candidate = parent.join(format!("{file_name}.tmp-{}-{attempt}", std::process::id()));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                file.write_all(serialized.as_bytes()).map_err(|error| {
                    format!("無法寫入暫存 session 檔 {}：{error}", candidate.display())
                })?;
                file.sync_all().map_err(|error| {
                    format!("無法同步暫存 session 檔 {}：{error}", candidate.display())
                })?;
                temporary_path = Some(candidate);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "無法建立暫存 session 檔 {}：{error}",
                    candidate.display()
                ));
            }
        }
    }
    let temporary_path =
        temporary_path.ok_or_else(|| "無法取得未衝突的暫存 session 檔名".to_owned())?;
    fs::rename(&temporary_path, path)
        .map_err(|error| format!("無法以暫存 session 檔替換 {}：{error}", path.display()))?;

    let reloaded = load(path)?;
    if reloaded.lines != session_file.lines {
        return Err(format!("session 檔回讀驗證不一致：{}", path.display()));
    }
    Ok(())
}

pub(crate) fn load(path: &Path) -> Result<SessionFile, String> {
    fs::read_to_string(path)
        .map_err(|error| format!("無法讀取 session 檔 {}：{error}", path.display()))
        .and_then(|text| parse(&text))
}

fn validate_line(
    value: &Value,
    line_number: usize,
    previous_uuid: Option<&str>,
) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("第 {line_number} 行必須是 JSON 物件"))?;
    let line_type = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("第 {line_number} 行缺少字串 type"))?;
    if !matches!(line_type, "user" | "assistant") {
        return Ok(());
    }

    object
        .get("uuid")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("第 {line_number} 行缺少字串 uuid"))?;
    let parent_uuid = object
        .get("parentUuid")
        .ok_or_else(|| format!("第 {line_number} 行缺少 parentUuid（應為字串或 null）"))?;
    match previous_uuid {
        None if !parent_uuid.is_null() => {
            return Err(format!(
                "第 {line_number} 行第一條對話的 parentUuid 必須是 null"
            ));
        }
        Some(previous) if parent_uuid.as_str() != Some(previous) => {
            return Err(format!(
                "第 {line_number} 行 parentUuid 未連到前一條對話 uuid {previous}"
            ));
        }
        _ => {}
    }
    let message = object
        .get("message")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("第 {line_number} 行缺少 message 物件"))?;
    if message.get("role").and_then(Value::as_str) != Some(line_type) {
        return Err(format!("第 {line_number} 行 message.role 必須與 type 相同"));
    }
    let content = message
        .get("content")
        .ok_or_else(|| format!("第 {line_number} 行缺少 message.content"))?;
    match line_type {
        "user" if !content.is_string() => Err(format!(
            "第 {line_number} 行 user message.content 必須是字串"
        )),
        "assistant" if !content.is_array() => Err(format!(
            "第 {line_number} 行 assistant message.content 必須是陣列"
        )),
        _ => Ok(()),
    }
}

fn conversation_type(line: &Value) -> Option<&str> {
    line.get("type")?
        .as_str()
        .filter(|line_type| matches!(*line_type, "user" | "assistant"))
}

fn conversation_uuid(line: &Value) -> Option<&str> {
    conversation_type(line)?;
    line.get("uuid")?.as_str()
}

fn find_conversation_line_mut<'a>(
    session_file: &'a mut SessionFile,
    uuid: &str,
) -> Result<&'a mut Value, String> {
    session_file
        .lines
        .iter_mut()
        .find(|line| conversation_uuid(line) == Some(uuid))
        .ok_or_else(|| format!("找不到對話 uuid {uuid}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"type":"user","uuid":"u1","parentUuid":null,"message":{"role":"user","content":"before [private] after"},"cwd":"/tmp"}
{"type":"queue-operation","operation":"noop"}
{"type":"assistant","uuid":"a1","parentUuid":"u1","message":{"role":"assistant","content":[{"type":"text","text":"first answer"}]},"usage":{"x":1}}
{"type":"ai-title","title":"unchanged"}
{"type":"user","uuid":"u2","parentUuid":"a1","message":{"role":"user","content":"second user"}}
{"type":"assistant","uuid":"a2","parentUuid":"u2","message":{"role":"assistant","content":[{"type":"text","text":"last answer"}]}}
{"type":"last-prompt","prompt":"tail metadata"}
"#;

    #[test]
    fn munges_every_non_ascii_alphanumeric_cwd_character() {
        let path = session_file_path(
            Path::new("/claude"),
            Path::new("/private/tmp/x/a_b.c"),
            "session",
        );
        assert_eq!(
            path,
            Path::new("/claude/projects/-private-tmp-x-a-b-c/session.jsonl")
        );
    }

    #[test]
    fn parses_conversations_and_preserves_metadata_lines() {
        let session_file = parse(SAMPLE).unwrap();
        assert_eq!(session_file.lines.len(), 7);
        assert_eq!(session_file.lines[1]["type"], "queue-operation");
        assert_eq!(session_file.lines[3]["title"], "unchanged");
    }

    #[test]
    fn rejects_bad_json_with_line_number() {
        let error = parse("{bad json}\n").unwrap_err();
        assert!(error.contains("第 1 行"));
        assert!(error.contains("有效 JSON"));
    }

    #[test]
    fn rejects_broken_uuid_chain() {
        let error = parse(
            r#"{"type":"user","uuid":"u1","parentUuid":null,"message":{"role":"user","content":"x"}}
{"type":"assistant","uuid":"a1","parentUuid":"wrong","message":{"role":"assistant","content":[]}}
"#,
        )
        .unwrap_err();
        assert!(error.contains("第 2 行"));
        assert!(error.contains("parentUuid"));
    }

    #[test]
    fn rejects_non_string_user_content() {
        let error = parse(
            r#"{"type":"user","uuid":"u1","parentUuid":null,"message":{"role":"user","content":[]}}
"#,
        )
        .unwrap_err();
        assert!(error.contains("第 1 行"));
        assert!(error.contains("content 必須是字串"));
    }

    #[test]
    fn round_trip_keeps_all_values() {
        let original = parse(SAMPLE).unwrap();
        let reparsed = parse(&serialize(&original)).unwrap();
        assert_eq!(original.lines, reparsed.lines);
    }

    #[test]
    fn erases_exactly_one_user_segment() {
        let mut session_file = parse(SAMPLE).unwrap();
        erase_user_segment(&mut session_file, "u1", "[private]").unwrap();
        assert_eq!(session_file.lines[0]["message"]["content"], "before  after");
        assert!(erase_user_segment(&mut session_file, "u1", "missing").is_err());

        let mut repeated = parse(r#"{"type":"user","uuid":"u1","parentUuid":null,"message":{"role":"user","content":"x secret x secret"}}
"#).unwrap();
        assert!(erase_user_segment(&mut repeated, "u1", "secret").is_err());
    }

    #[test]
    fn prefixes_only_the_last_assistant_idempotently() {
        let mut session_file = parse(SAMPLE).unwrap();
        prefix_last_assistant(&mut session_file, "Ralph: ").unwrap();
        prefix_last_assistant(&mut session_file, "Ralph: ").unwrap();
        assert_eq!(
            session_file.lines[2]["message"]["content"][0]["text"],
            "first answer"
        );
        assert_eq!(
            session_file.lines[5]["message"]["content"][0]["text"],
            "Ralph: last answer"
        );

        let mut no_assistant = parse(
            r#"{"type":"user","uuid":"u1","parentUuid":null,"message":{"role":"user","content":"x"}}
"#,
        )
        .unwrap();
        assert!(prefix_last_assistant(&mut no_assistant, "Ralph: ").is_err());
    }

    #[test]
    fn truncates_target_conversation_and_all_later_lines() {
        let mut session_file = parse(SAMPLE).unwrap();
        truncate_from(&mut session_file, "u2").unwrap();
        assert_eq!(session_file.lines.len(), 4);
        assert_eq!(session_file.lines[3]["type"], "ai-title");
        parse(&serialize(&session_file)).unwrap();
    }

    #[test]
    fn atomically_writes_and_overwrites_existing_file() {
        let directory = std::env::temp_dir().join(format!(
            "table-tavern-session-file-test-{}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("session.jsonl");
        fs::write(&path, "old contents").unwrap();
        let session_file = parse(SAMPLE).unwrap();

        write_atomic(&path, &session_file).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), serialize(&session_file));
        assert_eq!(load(&path).unwrap().lines, session_file.lines);

        fs::remove_file(&path).unwrap();
        fs::remove_dir(&directory).unwrap();
    }
}
