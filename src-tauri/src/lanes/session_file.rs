//! Claude CLI session JSONL 的讀寫：回合後抹寫與保溫截尾直接改這個檔。
//!
//! 鏈驗證：user／assistant 之外，CLI 會插入帶 uuid 的 `attachment` 行並接在 parentUuid 鏈上
//! （2.1.227 起偶發、2.1.287 每輪都有）。鏈節點＝user、assistant、attachment；每個節點的
//! parentUuid 必須等於前一個節點（第一個為 null），uuid 全檔唯一。帶 uuid 卻不在名單內的型別
//! 一律拒絕——不知道它會不會改變 resume 的語意，寧可丟線也不硬改。
//! 改寫：只動 user／assistant 行；沒改過的行（含 attachment、雜項行、空白行）以讀入原文寫回，
//! 逐位元組不變。

use serde_json::Value;
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// 帶 uuid、接在 parentUuid 鏈上的非對話行型別（實測只有這一種）。
const CHAIN_ONLY_TYPES: &[&str] = &["attachment"];

#[derive(Debug)]
struct Line {
    /// None＝空白行
    value: Option<Value>,
    /// 讀入原文（不含行尾）
    text: String,
    /// 行尾：`\n`、`\r\n`，或末行沒換行時為空字串
    ending: &'static str,
    /// 被改寫過：寫回時重新序列化，其餘照抄原文
    dirty: bool,
}

/// Claude CLI session JSONL 的完整保留表示。
#[derive(Debug)]
pub(crate) struct SessionFile {
    lines: Vec<Line>,
}

impl SessionFile {
    fn values(&self) -> impl DoubleEndedIterator<Item = &Value> {
        self.lines.iter().filter_map(|line| line.value.as_ref())
    }
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

/// 切行並保留各行的行尾。
fn split_lines(text: &str) -> impl Iterator<Item = (&str, &'static str)> {
    text.split_inclusive('\n').map(|piece| {
        if let Some(body) = piece.strip_suffix("\r\n") {
            (body, "\r\n")
        } else if let Some(body) = piece.strip_suffix('\n') {
            (body, "\n")
        } else {
            (piece, "")
        }
    })
}

/// 鏈驗證的進度：鏈尾與已出現過的節點 uuid。
#[derive(Default)]
struct Chain {
    tip: Option<String>,
    seen: HashSet<String>,
}

pub(crate) fn parse(text: &str) -> Result<SessionFile, String> {
    let mut lines = Vec::new();
    let mut chain = Chain::default();
    for (index, (body, ending)) in split_lines(text).enumerate() {
        let line_number = index + 1;
        let value = if body.trim().is_empty() {
            None
        } else {
            let value: Value = serde_json::from_str(body)
                .map_err(|error| format!("第 {line_number} 行不是有效 JSON：{error}"))?;
            validate_line(&value, line_number, &mut chain)?;
            Some(value)
        };
        lines.push(Line {
            value,
            text: body.to_owned(),
            ending,
            dirty: false,
        });
    }
    Ok(SessionFile { lines })
}

pub(crate) fn serialize(session_file: &SessionFile) -> String {
    let mut out = String::new();
    for line in &session_file.lines {
        match (&line.value, line.dirty) {
            (Some(value), true) => out
                .push_str(&serde_json::to_string(value).expect("serde_json::Value must serialize")),
            _ => out.push_str(&line.text),
        }
        out.push_str(line.ending);
    }
    out
}

pub(crate) fn erase_user_segment(
    session_file: &mut SessionFile,
    uuid: &str,
    segment: &str,
) -> Result<(), String> {
    let line = find_conversation_line_mut(session_file, uuid)?;
    let value = line.value.as_mut().expect("conversation lines have values");
    if conversation_type(value) != Some("user") {
        return Err(format!("uuid {uuid} 不是 user 對話行"));
    }

    let updated = {
        let content = value
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
    *value
        .pointer_mut("/message/content")
        .expect("validated user message content exists") = Value::String(updated);
    line.dirty = true;
    Ok(())
}

/// 找出內容含指定片段的 user 對話行。恰好一行才回其 uuid：
/// 0 行＝注入段不在檔內（CLI 行為變了），≥2 行＝片段不夠獨特，都不能安全抹寫。
pub(crate) fn find_user_line_with_segment(
    session_file: &SessionFile,
    segment: &str,
) -> Result<String, String> {
    let mut matches = session_file.values().filter(|line| {
        conversation_type(line) == Some("user")
            && line
                .pointer("/message/content")
                .and_then(Value::as_str)
                .is_some_and(|content| content.contains(segment))
    });
    let hit = matches
        .next()
        .ok_or_else(|| "找不到含指定片段的 user 對話行".to_owned())?;
    if matches.next().is_some() {
        return Err("含指定片段的 user 對話行超過一行，無法安全抹寫".to_owned());
    }
    hit.get("uuid")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "含指定片段的 user 對話行缺少 uuid".to_owned())
}

pub(crate) fn prefix_last_assistant(
    session_file: &mut SessionFile,
    prefix: &str,
) -> Result<(), String> {
    let line = session_file
        .lines
        .iter_mut()
        .rev()
        .find(|line| line.value.as_ref().and_then(conversation_type) == Some("assistant"))
        .ok_or_else(|| "找不到 assistant 對話行".to_owned())?;
    let value = line.value.as_mut().expect("conversation lines have values");
    let content = value
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
        line.dirty = true;
    }
    Ok(())
}

/// 保溫後的檔必須是「保溫前原文＋CLI 追加的一段」，回傳追加段。
/// 前段被改動＝不能安全還原。
pub(crate) fn appended_since<'a>(before: &str, after: &'a str) -> Result<&'a str, String> {
    after
        .strip_prefix(before)
        .ok_or_else(|| "保溫期間 session 檔前段被改動，無法還原".to_owned())
}

/// 追加段裡含 `marker` 的 user 行數。追加段出現不含 `marker` 的 user 行＝混進了別的對話，拒絕。
pub(crate) fn marker_user_lines(appended: &str, marker: &str) -> Result<usize, String> {
    let mut count = 0;
    for (index, (body, _)) in split_lines(appended).enumerate() {
        if body.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(body)
            .map_err(|error| format!("追加段第 {} 行不是有效 JSON：{error}", index + 1))?;
        if conversation_type(&value) != Some("user") {
            continue;
        }
        let content = value.pointer("/message/content").and_then(Value::as_str);
        match content.is_some_and(|content| content.contains(marker)) {
            true => count += 1,
            false => return Err(format!("追加段第 {} 行是保溫以外的 user 行", index + 1)),
        }
    }
    Ok(count)
}

pub(crate) fn write_atomic(path: &Path, session_file: &SessionFile) -> Result<(), String> {
    write_text_atomic(path, &serialize(session_file))
}

/// 原子寫入（暫存檔＋rename）後逐位元組回讀比對。
pub(crate) fn write_text_atomic(path: &Path, text: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("session 檔路徑沒有父目錄：{}", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("session 檔路徑沒有有效檔名：{}", path.display()))?;
    let mut temporary_path = None;
    for attempt in 0..100 {
        let candidate = parent.join(format!("{file_name}.tmp-{}-{attempt}", std::process::id()));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                file.write_all(text.as_bytes()).map_err(|error| {
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

    if read_text(path)? != text {
        return Err(format!("session 檔回讀驗證不一致：{}", path.display()));
    }
    Ok(())
}

pub(crate) fn read_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map_err(|error| format!("無法讀取 session 檔 {}：{error}", path.display()))
}

pub(crate) fn load(path: &Path) -> Result<SessionFile, String> {
    read_text(path).and_then(|text| parse(&text))
}

fn validate_line(value: &Value, line_number: usize, chain: &mut Chain) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("第 {line_number} 行必須是 JSON 物件"))?;
    let line_type = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("第 {line_number} 行缺少字串 type"))?;
    let conversation = matches!(line_type, "user" | "assistant");
    let uuid = object.get("uuid").and_then(Value::as_str);
    if !conversation && !CHAIN_ONLY_TYPES.contains(&line_type) {
        if uuid.is_some() {
            return Err(format!(
                "第 {line_number} 行是不認得的帶 uuid 行型 {line_type}，無法確定能安全改寫"
            ));
        }
        return Ok(());
    }

    let uuid = uuid.ok_or_else(|| format!("第 {line_number} 行缺少字串 uuid"))?;
    let parent_uuid = object
        .get("parentUuid")
        .ok_or_else(|| format!("第 {line_number} 行缺少 parentUuid（應為字串或 null）"))?;
    match chain.tip.as_deref() {
        None if !parent_uuid.is_null() => {
            return Err(format!(
                "第 {line_number} 行是鏈上第一個節點，parentUuid 必須是 null"
            ));
        }
        Some(tip) if parent_uuid.as_str() != Some(tip) => {
            return Err(format!(
                "第 {line_number} 行 parentUuid 未連到前一個鏈上節點 uuid {tip}"
            ));
        }
        _ => {}
    }
    if !chain.seen.insert(uuid.to_owned()) {
        return Err(format!("第 {line_number} 行 uuid {uuid} 與前面的節點重複"));
    }
    chain.tip = Some(uuid.to_owned());
    if !conversation {
        return Ok(()); // attachment 只驗鏈，內容不管
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

/// 抹寫／補前綴用：只認 user／assistant（attachment 永遠不是改寫對象）。
fn conversation_type(line: &Value) -> Option<&str> {
    line.get("type")?
        .as_str()
        .filter(|line_type| matches!(*line_type, "user" | "assistant"))
}

fn find_conversation_line_mut<'a>(
    session_file: &'a mut SessionFile,
    uuid: &str,
) -> Result<&'a mut Line, String> {
    session_file
        .lines
        .iter_mut()
        .find(|line| {
            line.value.as_ref().is_some_and(|value| {
                conversation_type(value).is_some()
                    && value.get("uuid").and_then(Value::as_str) == Some(uuid)
            })
        })
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

    /// claude CLI 2.1.287 的實測結構（脫敏：只留型別、uuid 鏈與欄位形狀）。
    /// 刻意混入不同鍵順序、多餘空白、CRLF、空白行，末行沒有換行——未改動行要逐位元組寫回。
    const CLI_2_1_287: &str = concat!(
        "{\"type\":\"queue-operation\",\"operation\":\"enqueue\"}\n",
        "{\"type\":\"queue-operation\",\"operation\":\"dequeue\"}\n",
        "{\"parentUuid\":null,\"isSidechain\":false,\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"開場 [機密] 玩家說話\"},\"uuid\":\"u1\",\"version\":\"2.1.287\"}\n",
        "{\"parentUuid\":\"u1\",\"attachment\":{\"type\":\"environment\",\"content\":\"…\"},\"type\":\"attachment\",\"uuid\":\"t1\"}\n",
        "{ \"parentUuid\" : \"t1\", \"attachment\": {\"type\":\"model\"}, \"type\": \"attachment\", \"uuid\": \"t2\" }\r\n",
        "{\"type\":\"file-history-snapshot\",\"messageId\":\"m\"}\n",
        "{\"type\":\"atis-latch\"}\n",
        "\n",
        "{\"parentUuid\":\"t2\",\"attachment\":{\"type\":\"date\"},\"type\":\"attachment\",\"uuid\":\"t3\"}\n",
        "{\"parentUuid\":\"t3\",\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"id\":\"msg1\",\"content\":[{\"type\":\"thinking\",\"thinking\":\"…\"}]},\"uuid\":\"a1t\"}\n",
        "{\"parentUuid\":\"a1t\",\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"id\":\"msg1\",\"content\":[{\"type\":\"text\",\"text\":\"第一句回覆\"}]},\"uuid\":\"a1\"}\n",
        "{\"parentUuid\":\"a1\",\"attachment\":{\"type\":\"prompt_snapshot\"},\"type\":\"attachment\",\"uuid\":\"t4\"}\n",
        "{\"type\":\"last-prompt\",\"lastPrompt\":\"…\"}\n",
        "{\"type\":\"cost-state\"}\n",
        "{\"type\":\"queue-operation\",\"operation\":\"enqueue\"}\n",
        "{\"parentUuid\":\"t4\",\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"第二輪 [機密2]\"},\"uuid\":\"u2\"}\n",
        "{\"parentUuid\":\"u2\",\"attachment\":{\"type\":\"session_context\"},\"type\":\"attachment\",\"uuid\":\"t5\"}\n",
        "{\"parentUuid\":\"t5\",\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"第二句回覆\"}]},\"uuid\":\"a2\"}",
    );

    /// 2.1.227 的形狀：只偶發一行 total_tokens_reminder。
    const CLI_2_1_227: &str = concat!(
        "{\"type\":\"user\",\"uuid\":\"u1\",\"parentUuid\":null,\"message\":{\"role\":\"user\",\"content\":\"x [機密]\"}}\n",
        "{\"type\":\"attachment\",\"uuid\":\"t1\",\"parentUuid\":\"u1\",\"attachment\":{\"type\":\"total_tokens_reminder\"}}\n",
        "{\"type\":\"ai-title\",\"title\":\"t\"}\n",
        "{\"type\":\"assistant\",\"uuid\":\"a1t\",\"parentUuid\":\"t1\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"thinking\",\"thinking\":\"…\"}]}}\n",
        "{\"type\":\"assistant\",\"uuid\":\"a1\",\"parentUuid\":\"a1t\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"回覆\"}]}}\n",
    );

    fn values(session_file: &SessionFile) -> Vec<&Value> {
        session_file.values().collect()
    }

    fn line(text: &str) -> String {
        format!("{text}\n")
    }

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
        let values = values(&session_file);
        assert_eq!(values.len(), 7);
        assert_eq!(values[1]["type"], "queue-operation");
        assert_eq!(values[3]["title"], "unchanged");
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
    fn accepts_attachment_lines_on_the_chain_for_both_cli_shapes() {
        for text in [CLI_2_1_287, CLI_2_1_227] {
            let session_file = parse(text).unwrap();
            // 沒改任何東西：寫回逐位元組相同（CRLF、空白行、鍵順序、末行無換行都保住）
            assert_eq!(serialize(&session_file), text);
        }
    }

    #[test]
    fn attachment_must_link_to_the_chain_tip() {
        let text = line(
            r#"{"type":"user","uuid":"u1","parentUuid":null,"message":{"role":"user","content":"x"}}"#,
        ) + &line(r#"{"type":"attachment","uuid":"t1","parentUuid":"u1"}"#)
            + &line(
                r#"{"type":"assistant","uuid":"a1","parentUuid":"u1","message":{"role":"assistant","content":[]}}"#,
            );
        let error = parse(&text).unwrap_err();
        assert!(error.contains("第 3 行") && error.contains("t1"), "{error}");

        let dangling = line(
            r#"{"type":"user","uuid":"u1","parentUuid":null,"message":{"role":"user","content":"x"}}"#,
        ) + &line(r#"{"type":"attachment","uuid":"t1","parentUuid":"elsewhere"}"#);
        assert!(parse(&dangling).unwrap_err().contains("第 2 行"));
    }

    #[test]
    fn rejects_unknown_types_that_carry_a_uuid() {
        let text = line(
            r#"{"type":"user","uuid":"u1","parentUuid":null,"message":{"role":"user","content":"x"}}"#,
        ) + &line(
            r#"{"type":"system","uuid":"s1","parentUuid":"u1","subtype":"compact_boundary"}"#,
        );
        let error = parse(&text).unwrap_err();
        assert!(error.contains("system"), "{error}");
    }

    #[test]
    fn rejects_duplicate_node_uuids() {
        // 對話行之間重複：u1→a1→u1
        let conversations = line(
            r#"{"type":"user","uuid":"u1","parentUuid":null,"message":{"role":"user","content":"x"}}"#,
        ) + &line(
            r#"{"type":"assistant","uuid":"a1","parentUuid":"u1","message":{"role":"assistant","content":[]}}"#,
        ) + &line(
            r#"{"type":"user","uuid":"u1","parentUuid":"a1","message":{"role":"user","content":"y"}}"#,
        );
        assert!(parse(&conversations).unwrap_err().contains("重複"));
        // 對話行與 attachment 交叉重複：u1→x→u1、u1→x→x
        let crossed = line(
            r#"{"type":"user","uuid":"u1","parentUuid":null,"message":{"role":"user","content":"x"}}"#,
        ) + &line(r#"{"type":"attachment","uuid":"x","parentUuid":"u1"}"#)
            + &line(
                r#"{"type":"user","uuid":"u1","parentUuid":"x","message":{"role":"user","content":"y"}}"#,
            );
        assert!(parse(&crossed).unwrap_err().contains("重複"));
        let attachment_twice = line(
            r#"{"type":"user","uuid":"u1","parentUuid":null,"message":{"role":"user","content":"x"}}"#,
        ) + &line(r#"{"type":"attachment","uuid":"x","parentUuid":"u1"}"#)
            + &line(r#"{"type":"attachment","uuid":"x","parentUuid":"x"}"#);
        assert!(parse(&attachment_twice).unwrap_err().contains("重複"));
    }

    #[test]
    fn rewrite_touches_only_the_two_target_lines() {
        let mut session_file = parse(CLI_2_1_287).unwrap();
        let uuid = find_user_line_with_segment(&session_file, "[機密2]").unwrap();
        assert_eq!(uuid, "u2");
        erase_user_segment(&mut session_file, &uuid, "[機密2]").unwrap();
        prefix_last_assistant(&mut session_file, "內藤：").unwrap();
        let written = serialize(&session_file);
        let before: Vec<(&str, &str)> = split_lines(CLI_2_1_287).collect();
        let after: Vec<(&str, &str)> = split_lines(&written).collect();
        assert_eq!(before.len(), after.len());
        let changed: Vec<usize> = (0..before.len())
            .filter(|&i| before[i] != after[i])
            .collect();
        assert_eq!(
            changed,
            vec![15, 17],
            "只有第二輪 user 與最後 assistant 會變"
        );
        assert!(after[15].0.contains("第二輪 ") && !after[15].0.contains("[機密2]"));
        assert!(after[17].0.contains("內藤：第二句回覆"));
        assert_eq!(after[17].1, "", "末行沒換行就維持沒換行");
        parse(&written).unwrap();
    }

    #[test]
    fn round_trip_keeps_all_values() {
        let original = parse(SAMPLE).unwrap();
        let reparsed = parse(&serialize(&original)).unwrap();
        assert_eq!(values(&original), values(&reparsed));
        assert_eq!(serialize(&original), SAMPLE);
    }

    #[test]
    fn erases_exactly_one_user_segment() {
        let mut session_file = parse(SAMPLE).unwrap();
        erase_user_segment(&mut session_file, "u1", "[private]").unwrap();
        assert_eq!(
            values(&session_file)[0]["message"]["content"],
            "before  after"
        );
        assert!(erase_user_segment(&mut session_file, "u1", "missing").is_err());

        let mut repeated = parse(r#"{"type":"user","uuid":"u1","parentUuid":null,"message":{"role":"user","content":"x secret x secret"}}
"#).unwrap();
        assert!(erase_user_segment(&mut repeated, "u1", "secret").is_err());
        // attachment 的 uuid 不是改寫對象
        let mut with_attachment = parse(CLI_2_1_287).unwrap();
        assert!(erase_user_segment(&mut with_attachment, "t1", "…").is_err());
    }

    #[test]
    fn finds_the_single_user_line_holding_a_segment() {
        let session_file = parse(SAMPLE).unwrap();
        assert_eq!(
            find_user_line_with_segment(&session_file, "[private]").unwrap(),
            "u1"
        );
        assert!(find_user_line_with_segment(&session_file, "missing").is_err());
        // 兩行都含片段＝不夠獨特，拒絕
        assert!(find_user_line_with_segment(&session_file, "e").is_err());
    }

    #[test]
    fn prefixes_only_the_last_assistant_idempotently() {
        let mut session_file = parse(SAMPLE).unwrap();
        prefix_last_assistant(&mut session_file, "Ralph: ").unwrap();
        prefix_last_assistant(&mut session_file, "Ralph: ").unwrap();
        let values = values(&session_file);
        assert_eq!(values[2]["message"]["content"][0]["text"], "first answer");
        assert_eq!(
            values[5]["message"]["content"][0]["text"],
            "Ralph: last answer"
        );

        let mut no_assistant = parse(
            r#"{"type":"user","uuid":"u1","parentUuid":null,"message":{"role":"user","content":"x"}}
"#,
        )
        .unwrap();
        assert!(prefix_last_assistant(&mut no_assistant, "Ralph: ").is_err());

        // 最後一則 assistant 不是 text（只有 thinking）＝不能補
        let mut thinking_last = parse(
            r#"{"type":"user","uuid":"u1","parentUuid":null,"message":{"role":"user","content":"x"}}
{"type":"assistant","uuid":"a1","parentUuid":"u1","message":{"role":"assistant","content":[{"type":"thinking","thinking":"…"}]}}
"#,
        )
        .unwrap();
        assert!(prefix_last_assistant(&mut thinking_last, "Ralph: ").is_err());
    }

    #[test]
    fn ping_restore_accepts_only_appends_and_counts_ping_lines() {
        let marker = "（保溫）";
        let before = CLI_2_1_287.to_owned() + "\n";
        // CLI 先追加 queue-operation 與 attachment，才寫 ping user 與回覆
        let appended = line(r#"{"type":"queue-operation","operation":"enqueue"}"#)
            + &line(
                r#"{"parentUuid":"a2","type":"user","message":{"role":"user","content":"（保溫）"},"uuid":"p1"}"#,
            )
            + &line(r#"{"parentUuid":"p1","type":"attachment","uuid":"p2"}"#)
            + &line(
                r#"{"parentUuid":"p2","type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"ok"}]},"uuid":"p3"}"#,
            );
        let after = before.clone() + &appended;
        let tail = appended_since(&before, &after).unwrap();
        assert_eq!(tail, appended);
        assert_eq!(marker_user_lines(tail, marker).unwrap(), 1);
        // 什麼都沒追加（ping 送出前就失敗）
        assert_eq!(
            marker_user_lines(appended_since(&before, &before).unwrap(), marker).unwrap(),
            0
        );
        // 前段被改動
        let tampered = before.replacen("第一句回覆", "改過", 1) + &appended;
        assert!(appended_since(&before, &tampered).is_err());
        // 追加段混進別的 user 行
        let foreign = appended.clone()
            + &line(
                r#"{"parentUuid":"p3","type":"user","message":{"role":"user","content":"別的"},"uuid":"q1"}"#,
            );
        assert!(marker_user_lines(&foreign, marker).is_err());
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
        let session_file = parse(CLI_2_1_287).unwrap();

        write_atomic(&path, &session_file).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), CLI_2_1_287);
        assert_eq!(values(&load(&path).unwrap()), values(&session_file));

        fs::remove_file(&path).unwrap();
        fs::remove_dir(&directory).unwrap();
    }
}
