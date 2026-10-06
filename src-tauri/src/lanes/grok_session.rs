//! grok 角色共線的回合後抹寫（grok-shared-lane）。grok 1.0.46 的 session 目錄
//! `$GROK_HOME/sessions/<群組>/<id>/` 裡，送給模型的歷史在 `chat_history.jsonl`，
//! 續聊用 `updates.jsonl` 重建／比對，所以兩檔都要改：抹機密段、拿掉 reasoning
//! （摘要會明文寫出私設，encrypted_content 續聊時回送 API）、回覆補名字前綴。
//! 形狀只要有一處不在預期內就回錯，由呼叫端丟線重開——寧可少一次快取也不留私設。

use serde_json::Value;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// 丟線原因（寫進用量帳本的 detail）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DropReason {
    RewriteFailed,
    Compacted,
    LockTimeout,
}

impl DropReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::RewriteFailed => "rewrite-failed",
            Self::Compacted => "compacted",
            Self::LockTimeout => "lock-timeout",
        }
    }
}

#[derive(Debug)]
pub(crate) struct RewriteError {
    pub reason: DropReason,
    pub detail: String,
}

fn failed(detail: impl Into<String>) -> RewriteError {
    RewriteError {
        reason: DropReason::RewriteFailed,
        detail: detail.into(),
    }
}

fn compacted(detail: impl Into<String>) -> RewriteError {
    RewriteError {
        reason: DropReason::Compacted,
        detail: detail.into(),
    }
}

const LOCK_BUDGET: Duration = Duration::from_secs(2);
const TMP_DIR: &str = "tt-rewrite-tmp";

/// `$GROK_HOME/sessions/*/<id>` 的所有目錄。群組名可能是 URL 編碼的 cwd，也可能是
/// 過長時的 slug＋hash，所以不自己重算，只往下一層找。sessions 不存在＝沒有。
pub(crate) fn find_session_dirs(
    grok_home: &Path,
    session_id: &str,
) -> Result<Vec<PathBuf>, String> {
    let sessions = grok_home.join("sessions");
    let entries = match fs::read_dir(&sessions) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("無法列出 {}：{error}", sessions.display())),
    };
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("無法列出 {}：{error}", sessions.display()))?;
        let candidate = entry.path().join(session_id);
        if candidate.is_dir() {
            found.push(candidate);
        }
    }
    Ok(found)
}

/// 刪掉這個 id 的 session 目錄。0 個＝已刪；多個一個都不刪（不知道哪個才是），回錯。
/// 只刪 `<群組>/<id>` 這一層，不碰上一層。
pub(crate) fn remove_session(grok_home: &Path, session_id: &str) -> Result<(), String> {
    if session_id.is_empty() || session_id.contains(['/', '\\']) || session_id.starts_with('.') {
        return Err(format!("session id 不合法：{session_id:?}"));
    }
    let dirs = find_session_dirs(grok_home, session_id)?;
    match dirs.as_slice() {
        [] => Ok(()),
        [dir] => {
            fs::remove_dir_all(dir).map_err(|error| format!("無法刪除 {}：{error}", dir.display()))
        }
        _ => Err(format!(
            "session {session_id} 有 {} 個目錄，一個都不刪",
            dirs.len()
        )),
    }
}

/// 回合後抹寫。`confidential` 是本輪送出的機密段（None／空字串＝這輪沒送）；
/// `prefix` 補在回覆前；`reply` 是本輪串流收到的回覆原文，用來核對兩檔。
pub(crate) fn rewrite(
    grok_home: &Path,
    session_id: &str,
    confidential: Option<&str>,
    prefix: &str,
    reply: &str,
) -> Result<(), RewriteError> {
    let confidential = confidential.filter(|segment| !segment.is_empty());
    let dirs = find_session_dirs(grok_home, session_id).map_err(failed)?;
    let dir = match dirs.as_slice() {
        [dir] => dir.clone(),
        _ => {
            return Err(failed(format!(
                "session {session_id} 目錄數 {}",
                dirs.len()
            )))
        }
    };
    let _locks = [
        lock_file(&dir.join("chat_history.jsonl.lock"))?,
        lock_file(&dir.join("updates.jsonl.lock"))?,
    ];
    check_not_compacted(&dir)?;

    let chat_path = dir.join("chat_history.jsonl");
    let updates_path = dir.join("updates.jsonl");
    let mut chat = Lines::load(&chat_path)?;
    let mut updates = Lines::load(&updates_path)?;
    let chat_user = rewrite_chat_history(&mut chat, confidential, prefix, reply)?;
    let updates_user = rewrite_updates(&mut updates, confidential, prefix, reply)?;
    if chat_user != updates_user {
        return Err(failed("兩檔的本輪 user 內容不一致"));
    }

    let tmp_dir = grok_home.join(TMP_DIR);
    fs::create_dir_all(&tmp_dir).map_err(|error| failed(format!("無法建立暫存目錄：{error}")))?;
    chat.write_atomic(&chat_path, &tmp_dir, session_id)?;
    updates.write_atomic(&updates_path, &tmp_dir, session_id)?;
    if let Some(segment) = confidential {
        scan_residue(&dir, segment)?;
    }
    Ok(())
}

/// 取 `.lock` 的獨占鎖（grok 自己也用這幾把 flock，卡住時會一直等）。限時輪詢，拿不到就丟線。
fn lock_file(path: &Path) -> Result<File, RewriteError> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|error| failed(format!("無法開啟 {}：{error}", path.display())))?;
    let deadline = Instant::now() + LOCK_BUDGET;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(fs::TryLockError::WouldBlock) => {
                return Err(RewriteError {
                    reason: DropReason::LockTimeout,
                    detail: path.display().to_string(),
                })
            }
            Err(fs::TryLockError::Error(error)) => {
                return Err(failed(format!("無法鎖定 {}：{error}", path.display())))
            }
        }
    }
}

/// 壓縮過的 session：摘要與 checkpoint 可能已經寫進當時還沒抹的機密段，整條不能再用。
fn check_not_compacted(dir: &Path) -> Result<(), RewriteError> {
    for name in [
        "compaction",
        "compaction_checkpoints",
        "compaction_requests",
    ] {
        if dir.join(name).exists() {
            return Err(compacted(name));
        }
    }
    let signals = dir.join("signals.json");
    match fs::read_to_string(&signals) {
        Ok(text) => {
            let value: Value = serde_json::from_str(&text)
                .map_err(|error| failed(format!("signals.json 不是 JSON：{error}")))?;
            if value
                .get("compactionCount")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                > 0
            {
                return Err(compacted("compactionCount"));
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(failed(format!("無法讀取 signals.json：{error}"))),
    }
}

/// JSONL 的保留表示：沒動的行保留原字串，改過的行才重新序列化。
struct Lines {
    rows: Vec<Row>,
}

struct Row {
    raw: String,
    value: Value,
    dirty: bool,
}

impl Lines {
    fn load(path: &Path) -> Result<Self, RewriteError> {
        let text = fs::read_to_string(path)
            .map_err(|error| failed(format!("無法讀取 {}：{error}", path.display())))?;
        Self::parse(&text)
    }

    fn parse(text: &str) -> Result<Self, RewriteError> {
        let mut rows = Vec::new();
        for (index, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let value = serde_json::from_str(line)
                .map_err(|error| failed(format!("第 {} 行不是 JSON：{error}", index + 1)))?;
            rows.push(Row {
                raw: line.to_owned(),
                value,
                dirty: false,
            });
        }
        Ok(Self { rows })
    }

    fn serialize(&self) -> String {
        let mut out = String::new();
        for row in &self.rows {
            if row.dirty {
                out.push_str(&serde_json::to_string(&row.value).expect("Value 一定序列化得了"));
            } else {
                out.push_str(&row.raw);
            }
            out.push('\n');
        }
        out
    }

    /// 暫存檔在 session 目錄外（同一個 GROK_HOME、同一顆磁碟），檔名帶 session id：
    /// 不同桌各持一把 lane 鎖，會同時寫這個暫存目錄。
    fn write_atomic(
        &self,
        path: &Path,
        tmp_dir: &Path,
        session_id: &str,
    ) -> Result<(), RewriteError> {
        let text = self.serialize();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| failed("檔名無效"))?;
        let tmp = tmp_dir.join(format!("{session_id}-{name}-{}", std::process::id()));
        let result = (|| {
            let mut file = File::create(&tmp).map_err(|error| format!("建立暫存檔：{error}"))?;
            file.write_all(text.as_bytes())
                .map_err(|error| format!("寫入暫存檔：{error}"))?;
            file.sync_all()
                .map_err(|error| format!("同步暫存檔：{error}"))?;
            drop(file);
            // Windows 的 rename 是 MoveFileExW＋REPLACE_EXISTING，可直接蓋過既有檔
            fs::rename(&tmp, path).map_err(|error| format!("替換 {}：{error}", path.display()))
        })();
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result.map_err(failed)?;
        let back = fs::read_to_string(path)
            .map_err(|error| failed(format!("回讀 {}：{error}", path.display())))?;
        if back != text {
            return Err(failed(format!("{} 回讀不一致", path.display())));
        }
        Ok(())
    }
}

fn row_type(value: &Value) -> Option<&str> {
    value.get("type").and_then(Value::as_str)
}

fn update_kind(value: &Value) -> Option<&str> {
    value
        .pointer("/params/update/sessionUpdate")
        .and_then(Value::as_str)
}

/// 抹機密段：只在這輪送了才找，必須恰好一次。
fn erase_once(text: &str, confidential: Option<&str>) -> Result<String, RewriteError> {
    match confidential {
        None => Ok(text.to_owned()),
        Some(segment) => match text.matches(segment).count() {
            1 => Ok(text.replacen(segment, "", 1)),
            count => Err(failed(format!("機密段出現 {count} 次"))),
        },
    }
}

/// 補上前綴後的回覆。模型會模仿歷史裡的「名字：」自己先寫一次，這時不重補（同 claude 的
/// prefix_last_assistant）。
fn with_prefix(text: &str, prefix: &str) -> String {
    match text.starts_with(prefix) {
        true => text.to_owned(),
        false => format!("{prefix}{text}"),
    }
}

/// 檔案裡的回覆必須就是本輪串流收到的那段，前綴在開頭且整段只出現這一次
/// （回覆內文自己又含「名字：」時無法分辨，丟線只損快取）。
fn check_prefixed(text: &str, prefix: &str, reply: &str) -> Result<(), RewriteError> {
    if text != with_prefix(reply, prefix)
        || (!prefix.is_empty() && text.matches(prefix).count() != 1)
    {
        return Err(failed("回覆與前綴對不上"));
    }
    Ok(())
}

/// chat_history：最後一則真 user（帶 prompt_index）之後只能有 reasoning 與恰好一則
/// 字串內容、無工具呼叫的 assistant。回傳抹好的 user 本文（剝掉 `<user_query>` 包裝）供跨檔比對。
fn rewrite_chat_history(
    lines: &mut Lines,
    confidential: Option<&str>,
    prefix: &str,
    reply: &str,
) -> Result<String, RewriteError> {
    if lines.rows.iter().any(|row| {
        row.value.get("synthetic_reason").and_then(Value::as_str) == Some("compaction_meta")
    }) {
        return Err(compacted("compaction_meta"));
    }
    let user_index = lines
        .rows
        .iter()
        .rposition(|row| {
            row_type(&row.value) == Some("user") && row.value.get("prompt_index").is_some()
        })
        .ok_or_else(|| failed("chat_history 找不到本輪 user"))?;
    let mut assistant_index = None;
    for (index, row) in lines.rows.iter().enumerate().skip(user_index + 1) {
        match row_type(&row.value) {
            Some("reasoning") => {}
            Some("assistant") if assistant_index.is_none() => {
                let tool_calls = row
                    .value
                    .get("tool_calls")
                    .is_some_and(|calls| !calls.as_array().is_some_and(Vec::is_empty));
                if tool_calls || !row.value.get("content").is_some_and(Value::is_string) {
                    return Err(failed("assistant 帶工具呼叫或內容不是字串"));
                }
                assistant_index = Some(index);
            }
            other => return Err(failed(format!("chat_history 本輪有預期外的項目 {other:?}"))),
        }
    }
    let assistant_index =
        assistant_index.ok_or_else(|| failed("chat_history 本輪沒有 assistant"))?;

    let user = &mut lines.rows[user_index];
    let parts = user
        .value
        .get_mut("content")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| failed("user content 不是陣列"))?;
    let mut body = String::new();
    let mut erased = false;
    for part in parts.iter_mut() {
        let Some(text) = part.get("text").and_then(Value::as_str) else {
            return Err(failed("user content 有非文字分段"));
        };
        let has_segment = confidential.is_some_and(|segment| text.contains(segment));
        let updated = if has_segment {
            if erased {
                return Err(failed("機密段出現在多個分段"));
            }
            erased = true;
            erase_once(text, confidential)?
        } else {
            text.to_owned()
        };
        body.push_str(&updated);
        part["text"] = Value::String(updated);
    }
    if confidential.is_some() && !erased {
        return Err(failed("chat_history 找不到機密段"));
    }
    user.dirty |= erased;
    let body = body
        .strip_prefix("<user_query>\n")
        .and_then(|rest| rest.strip_suffix("\n</user_query>"))
        .ok_or_else(|| failed("user 本文不是 <user_query> 包裝"))?
        .to_owned();

    let assistant = &mut lines.rows[assistant_index];
    let content = assistant.value["content"].as_str().unwrap_or_default();
    let prefixed = with_prefix(content, prefix);
    check_prefixed(&prefixed, prefix, reply)?;
    assistant.value["content"] = Value::String(prefixed);
    assistant.dirty = true;

    // 每一輪的 reasoning 都拿掉（摘要為空時 encrypted_content 仍帶著推理）
    lines
        .rows
        .retain(|row| row_type(&row.value) != Some("reasoning"));
    Ok(body)
}

/// updates：最後一個 user_message_chunk 之後只能有 thought／message chunk、turn_completed、
/// background_tasks。全檔出現壓縮事件就丟線。回傳抹好的 user 本文。
fn rewrite_updates(
    lines: &mut Lines,
    confidential: Option<&str>,
    prefix: &str,
    reply: &str,
) -> Result<String, RewriteError> {
    if lines.rows.iter().any(|row| {
        update_kind(&row.value)
            .is_some_and(|kind| kind == "compaction_checkpoint" || kind.starts_with("auto_compact"))
    }) {
        return Err(compacted("updates 壓縮事件"));
    }
    let user_index = lines
        .rows
        .iter()
        .rposition(|row| update_kind(&row.value) == Some("user_message_chunk"))
        .ok_or_else(|| failed("updates 找不到本輪 user"))?;
    let mut message_indices = Vec::new();
    for (index, row) in lines.rows.iter().enumerate().skip(user_index + 1) {
        match update_kind(&row.value) {
            Some("agent_thought_chunk" | "turn_completed" | "background_tasks") => {}
            Some("agent_message_chunk") => message_indices.push(index),
            other => return Err(failed(format!("updates 本輪有預期外的項目 {other:?}"))),
        }
    }
    let first_message = *message_indices
        .first()
        .ok_or_else(|| failed("updates 本輪沒有回覆"))?;

    let user = &mut lines.rows[user_index];
    let text = user
        .value
        .pointer("/params/update/content/text")
        .and_then(Value::as_str)
        .ok_or_else(|| failed("user chunk 沒有文字"))?;
    let body = erase_once(text, confidential)?;
    if confidential.is_some() {
        user.value["params"]["update"]["content"]["text"] = Value::String(body.clone());
        user.dirty = true;
    }

    let mut joined = String::new();
    for &index in &message_indices {
        let text = lines.rows[index]
            .value
            .pointer("/params/update/content/text")
            .and_then(Value::as_str)
            .ok_or_else(|| failed("回覆 chunk 沒有文字"))?;
        joined.push_str(text);
    }
    check_prefixed(&with_prefix(&joined, prefix), prefix, reply)?;
    let already = joined.starts_with(prefix);
    let first = &mut lines.rows[first_message];
    let text = first.value["params"]["update"]["content"]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    if !already {
        first.value["params"]["update"]["content"]["text"] =
            Value::String(format!("{prefix}{text}"));
        first.dirty = true;
    }

    lines
        .rows
        .retain(|row| update_kind(&row.value) != Some("agent_thought_chunk"));
    Ok(body)
}

/// 走遍 session 目錄：JSON／JSONL 先解碼再看所有字串（含鍵），其他檔當文字。
/// 任何讀檔、解析、遍歷失敗都算掃描失敗，不能跳過判乾淨。
fn scan_residue(dir: &Path, segment: &str) -> Result<(), RewriteError> {
    let entries = fs::read_dir(dir)
        .map_err(|error| failed(format!("無法列出 {}：{error}", dir.display())))?;
    for entry in entries {
        let path = entry
            .map_err(|error| failed(format!("無法列出 {}：{error}", dir.display())))?
            .path();
        let meta = fs::symlink_metadata(&path)
            .map_err(|error| failed(format!("無法讀取 {}：{error}", path.display())))?;
        if meta.is_dir() {
            scan_residue(&path, segment)?;
            continue;
        }
        let bytes = fs::read(&path)
            .map_err(|error| failed(format!("無法讀取 {}：{error}", path.display())))?;
        let text = String::from_utf8_lossy(&bytes);
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let hit = if name.ends_with(".jsonl") {
            let mut hit = false;
            for line in text.lines().filter(|line| !line.trim().is_empty()) {
                let value: Value = serde_json::from_str(line)
                    .map_err(|error| failed(format!("{} 有壞行：{error}", path.display())))?;
                hit |= value_contains(&value, segment);
            }
            hit
        } else if name.ends_with(".json") {
            let value: Value = serde_json::from_str(&text)
                .map_err(|error| failed(format!("{} 不是 JSON：{error}", path.display())))?;
            value_contains(&value, segment)
        } else {
            false
        };
        if hit || text.contains(segment) {
            return Err(failed(format!("{} 仍含機密段", path.display())));
        }
    }
    Ok(())
}

fn value_contains(value: &Value, segment: &str) -> bool {
    match value {
        Value::String(text) => text.contains(segment),
        Value::Array(items) => items.iter().any(|item| value_contains(item, segment)),
        Value::Object(map) => map
            .iter()
            .any(|(key, item)| key.contains(segment) || value_contains(item, segment)),
        _ => false,
    }
}

#[cfg(test)]
mod tests;
