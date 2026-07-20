//! CLI 傳輸層（訂閱模式，NewPlan §3.2／§4.2）。
//! 原則：只偵測不代辦；CLI 是無狀態傳輸——上下文一律由 transport::assemble_messages
//! 組裝、headless 單發、system prompt 覆寫，不依賴 CLI 自身 session（§8.1）。
//! 旗標依 2026-07-19 當場 --help 查證：claude 2.1.210、codex-cli 0.145.0。

use crate::data::{DataResult, Tier};
use crate::transport::ChatMessage;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

#[derive(Debug, Clone, Serialize)]
pub struct CliInfo {
    pub id: String,
    pub path: String,
    pub version: String,
}

fn candidate_dirs() -> Vec<PathBuf> {
    // PATH 之外補常見安裝位置：GUI App 由 Finder 啟動時 PATH 往往不含它們
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).collect())
        .unwrap_or_default();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        dirs.push(home.join(".local/bin"));
        dirs.push(home.join(".claude/local"));
    }
    dirs.push(PathBuf::from("/opt/homebrew/bin"));
    dirs.push(PathBuf::from("/usr/local/bin"));
    dirs
}

fn is_executable(path: &Path) -> bool {
    // 執行位元檢查僅限 unix；Windows（日後支援）改查副檔名慣例即可，先保住編譯路
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

fn find_binary(name: &str) -> Option<PathBuf> {
    candidate_dirs()
        .into_iter()
        .map(|directory| directory.join(name))
        .find(|path| is_executable(path))
}

pub async fn detect_clis() -> Vec<CliInfo> {
    let mut found = Vec::new();
    for id in ["claude", "codex"] {
        let Some(path) = find_binary(id) else { continue };
        let version = Command::new(&path)
            .arg("--version")
            .output()
            .await
            .ok()
            .filter(|output| output.status.success())
            .map(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_owned()
            })
            .unwrap_or_default();
        found.push(CliInfo {
            id: id.to_owned(),
            path: path.to_string_lossy().into_owned(),
            version,
        });
    }
    found
}

/// 把共用組裝結果攤平成 CLI 單發需要的 (system, prompt)。
/// assistant 訊息即本發言者（角色或 GM）過往內容，攤平時補回名字前綴；
/// closing 為收尾指示，由呼叫端依發言者身分決定。
pub fn flatten_messages(
    assistant_label: &str,
    closing: &str,
    messages: &[ChatMessage],
) -> (String, String) {
    let system = messages
        .first()
        .map(|message| message.content.clone())
        .unwrap_or_default();
    let history: Vec<String> = messages
        .iter()
        .skip(1)
        .map(|message| {
            if message.role == "assistant" {
                format!("{assistant_label}：{}", message.content)
            } else {
                message.content.clone()
            }
        })
        .collect();
    let prompt = format!(
        "以下是到目前為止的對話紀錄：\n\n{}\n\n——\n{closing}",
        history.join("\n\n")
    );
    (system, prompt)
}

/// CLI 檔位覆寫：使用者可在 tier_models 以「{cli}:{tier}」為鍵（如 claude:best）
/// 指定該檔位的模型（別名或完整 id 皆可，CLI 端自行驗證）；空白視同未設。
pub fn tier_override<'a>(
    tier_models: &'a std::collections::BTreeMap<String, String>,
    cli: &str,
    tier: Tier,
) -> Option<&'a str> {
    tier_models
        .get(&format!("{cli}:{}", tier.as_str()))
        .map(String::as_str)
        .map(str::trim)
        .filter(|model| !model.is_empty())
}

/// claude 的檔位預設對應（未覆寫時）：CLI 模型別名是穩定介面，不佔用 OpenRouter 的 tier_models
pub fn claude_model_for(tier: Tier) -> Option<&'static str> {
    match tier {
        Tier::Best => Some("opus"),
        Tier::Balanced => Some("sonnet"),
        Tier::Fast => Some("haiku"),
        Tier::Default => None,
    }
}

/// codex 的檔位對應：模型用 CLI 預設，檔位映射到 reasoning effort
pub fn codex_effort_for(tier: Tier) -> Option<&'static str> {
    match tier {
        Tier::Best => Some("high"),
        Tier::Balanced => Some("medium"),
        Tier::Fast => Some("low"),
        Tier::Default => None,
    }
}

/// --safe-mode：停用使用者的 CLAUDE.md／plugins／hooks，避免 coding 客製污染角色扮演；
/// --tools ""：純文字生成不需要工具；--no-session-persistence：不落 session（§8.1）。
pub fn claude_args(model: Option<&str>, system: &str) -> Vec<String> {
    let mut args: Vec<String> = [
        "-p",
        "--verbose", // --print 的 stream-json 硬性要求
        "--safe-mode",
        "--no-session-persistence",
        "--output-format",
        "stream-json",
        "--include-partial-messages",
        "--tools",
        "",
        "--system-prompt",
        system,
    ]
    .map(str::to_owned)
    .to_vec();
    if let Some(model) = model {
        args.push("--model".to_owned());
        args.push(model.to_owned());
    }
    args
}

/// codex 沒有 system prompt 旗標，呼叫端把 system 併進 prompt。
/// --ignore-user-config：跳過使用者 config.toml（hooks／MCP），auth 不受影響（--help 查證）。
pub fn codex_args(model: Option<&str>, effort: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> = [
        "exec",
        "--json",
        "--ephemeral",
        "--skip-git-repo-check",
        "--ignore-user-config",
        "-s",
        "read-only",
    ]
    .map(str::to_owned)
    .to_vec();
    if let Some(model) = model {
        args.push("-m".to_owned());
        args.push(model.to_owned());
    }
    if let Some(effort) = effort {
        args.push("-c".to_owned());
        args.push(format!("model_reasoning_effort=\"{effort}\""));
    }
    args.push("-".to_owned()); // prompt 走 stdin，避開參數長度上限
    args
}

#[derive(Debug, PartialEq)]
pub enum CliLine {
    Delta(String),
    Done { text: String, is_error: bool },
    Other,
}

/// claude --output-format stream-json 逐行解析：
/// 只取 text_delta（thinking／signature 不進對話），result 事件收尾。
pub fn parse_claude_line(line: &str) -> CliLine {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return CliLine::Other;
    };
    match value.get("type").and_then(|t| t.as_str()) {
        Some("stream_event") => {
            let delta = value.pointer("/event/delta");
            let kind = delta.and_then(|d| d.get("type")).and_then(|t| t.as_str());
            match (kind, delta.and_then(|d| d.get("text")).and_then(|t| t.as_str())) {
                (Some("text_delta"), Some(text)) => CliLine::Delta(text.to_owned()),
                _ => CliLine::Other,
            }
        }
        Some("result") => CliLine::Done {
            text: value
                .get("result")
                .and_then(|r| r.as_str())
                .unwrap_or_default()
                .to_owned(),
            is_error: value
                .get("is_error")
                .and_then(|b| b.as_bool())
                .unwrap_or(false),
        },
        _ => CliLine::Other,
    }
}

/// codex exec --json 逐行解析：agent_message 為增量（通常一則），turn.completed 收尾。
/// item.type=="error" 可能只是非致命警告（例如 hooks 提示），不當失敗。
pub fn parse_codex_line(line: &str) -> CliLine {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return CliLine::Other;
    };
    match value.get("type").and_then(|t| t.as_str()) {
        Some("item.completed") => {
            let item = value.get("item");
            let kind = item.and_then(|i| i.get("type")).and_then(|t| t.as_str());
            match (kind, item.and_then(|i| i.get("text")).and_then(|t| t.as_str())) {
                (Some("agent_message"), Some(text)) => CliLine::Delta(text.to_owned()),
                _ => CliLine::Other,
            }
        }
        Some("turn.completed") => CliLine::Done {
            text: String::new(),
            is_error: false,
        },
        Some("turn.failed") | Some("error") => CliLine::Done {
            text: value
                .pointer("/error/message")
                .or_else(|| value.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("CLI 回合失敗")
                .to_owned(),
            is_error: true,
        },
        _ => CliLine::Other,
    }
}

/// headless 單發：prompt 走 stdin，逐行讀 stdout 解析、增量回呼，回傳完整文字。
pub async fn run_cli(
    program: &Path,
    args: &[String],
    stdin_data: &str,
    parse: fn(&str) -> CliLine,
    mut on_delta: impl FnMut(&str),
) -> DataResult<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().expect("stdin piped");
    stdin.write_all(stdin_data.as_bytes()).await?;
    drop(stdin); // 關閉讓 CLI 知道輸入結束

    // stderr 另開 task 排空，避免管線塞滿造成死鎖
    let mut stderr = child.stderr.take().expect("stderr piped");
    let stderr_task = tokio::spawn(async move {
        let mut buffer = String::new();
        let _ = stderr.read_to_string(&mut buffer).await;
        buffer
    });

    let stdout = child.stdout.take().expect("stdout piped");
    let mut lines = BufReader::new(stdout).lines();
    let mut full_text = String::new();
    let mut done: Option<(String, bool)> = None;
    while let Some(line) = lines.next_line().await? {
        match parse(&line) {
            CliLine::Delta(text) => {
                on_delta(&text);
                full_text.push_str(&text);
            }
            CliLine::Done { text, is_error } => done = Some((text, is_error)),
            CliLine::Other => {}
        }
    }

    let status = child.wait().await?;
    let stderr_text = stderr_task.await.unwrap_or_default();
    if let Some((text, true)) = &done {
        return Err(format!("CLI 回覆錯誤：{text}").into());
    }
    if full_text.is_empty() {
        // 串流沒抓到增量時退回收尾文字（例如未來旗標行為變動）
        if let Some((text, false)) = &done {
            if !text.is_empty() {
                on_delta(text);
                full_text = text.clone();
            }
        }
    }
    if full_text.is_empty() {
        let tail: String = stderr_text.lines().rev().take(5).collect::<Vec<_>>().join("\n");
        return Err(format!("CLI 沒有產出回覆（exit {status}）：{tail}").into());
    }
    Ok(full_text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::ChatMessage;

    fn msg(role: &str, content: &str) -> ChatMessage {
        ChatMessage {
            role: role.to_owned(),
            content: content.to_owned(),
        }
    }

    #[test]
    fn flatten_restores_speaker_prefix_and_appends_turn_instruction() {
        let messages = [
            msg("system", "你在扮演狐狸"),
            msg("user", "玩家：晚安\n（旁白）打烊前"),
            msg("assistant", "晚安，要來一杯嗎？"),
            msg("user", "玩家：好啊"),
        ];
        let (system, prompt) = flatten_messages("狐狸", "現在輪到「狐狸」回應。", &messages);
        assert_eq!(system, "你在扮演狐狸");
        assert!(prompt.contains("玩家：晚安\n（旁白）打烊前"));
        assert!(prompt.contains("狐狸：晚安，要來一杯嗎？"));
        assert!(prompt.ends_with("現在輪到「狐狸」回應。"));
    }

    // 樣本取自 2026-07-19 真實 CLI 冒煙輸出（scratchpad claude-smoke.jsonl／codex-smoke.jsonl）
    #[test]
    fn parses_real_claude_stream_json_lines() {
        assert_eq!(
            parse_claude_line(
                r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"測"}}}"#
            ),
            CliLine::Delta("測".to_owned())
        );
        assert_eq!(
            parse_claude_line(
                r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"内心"}}}"#
            ),
            CliLine::Other
        );
        assert_eq!(
            parse_claude_line(r#"{"type":"result","subtype":"success","is_error":false,"result":"測試"}"#),
            CliLine::Done {
                text: "測試".to_owned(),
                is_error: false
            }
        );
        assert_eq!(
            parse_claude_line(r#"{"type":"result","is_error":true,"result":"Failed to authenticate. API Error: 401 Invalid bearer token"}"#),
            CliLine::Done {
                text: "Failed to authenticate. API Error: 401 Invalid bearer token".to_owned(),
                is_error: true
            }
        );
        assert_eq!(parse_claude_line("not json"), CliLine::Other);
    }

    #[test]
    fn parses_real_codex_json_lines_and_ignores_warning_items() {
        assert_eq!(
            parse_codex_line(
                r#"{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"測試"}}"#
            ),
            CliLine::Delta("測試".to_owned())
        );
        // 非致命警告（真實輸出：hooks 提示）不可視為失敗
        assert_eq!(
            parse_codex_line(
                r#"{"type":"item.completed","item":{"id":"item_0","type":"error","message":"skipping async hook"}}"#
            ),
            CliLine::Other
        );
        assert_eq!(
            parse_codex_line(r#"{"type":"turn.completed","usage":{"input_tokens":15208}}"#),
            CliLine::Done {
                text: String::new(),
                is_error: false
            }
        );
        assert_eq!(
            parse_codex_line(r#"{"type":"turn.failed","error":{"message":"quota exceeded"}}"#),
            CliLine::Done {
                text: "quota exceeded".to_owned(),
                is_error: true
            }
        );
    }

    #[test]
    fn tier_mappings_cover_all_tiers() {
        assert_eq!(claude_model_for(Tier::Best), Some("opus"));
        assert_eq!(claude_model_for(Tier::Fast), Some("haiku"));
        assert_eq!(claude_model_for(Tier::Default), None);
        assert_eq!(codex_effort_for(Tier::Balanced), Some("medium"));
        assert_eq!(codex_effort_for(Tier::Default), None);
        let args = codex_args(None, codex_effort_for(Tier::Best));
        assert!(args.contains(&"model_reasoning_effort=\"high\"".to_owned()));
        assert!(!args.contains(&"-m".to_owned()));
        assert_eq!(args.last().unwrap(), "-");
        let args = codex_args(Some("gpt-5.6-terra"), None);
        assert!(args.windows(2).any(|w| w == ["-m", "gpt-5.6-terra"]));
        let args = claude_args(claude_model_for(Tier::Fast), "系統");
        assert!(args.windows(2).any(|w| w == ["--model", "haiku"]));
        assert!(args.windows(2).any(|w| w == ["--system-prompt", "系統"]));
    }

    #[test]
    fn tier_override_reads_prefixed_keys_and_ignores_blank() {
        let mut map = std::collections::BTreeMap::new();
        map.insert("claude:best".to_owned(), "claude-fable-5".to_owned());
        map.insert("claude:fast".to_owned(), "  ".to_owned());
        map.insert("best".to_owned(), "vendor/api-model".to_owned()); // API 檔位不受影響
        assert_eq!(tier_override(&map, "claude", Tier::Best), Some("claude-fable-5"));
        assert_eq!(tier_override(&map, "claude", Tier::Fast), None); // 空白＝未設
        assert_eq!(tier_override(&map, "codex", Tier::Best), None);
    }

    /// 以假 CLI 腳本走完 spawn→stdin→逐行解析→增量→收尾整條路（sh 腳本，僅 unix）
    #[cfg(unix)]
    #[tokio::test]
    async fn run_cli_streams_deltas_from_fake_cli_and_reads_stdin() {
        let dir = std::env::temp_dir().join(format!("tt-fake-cli-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fake-claude.sh");
        std::fs::write(
            &script,
            concat!(
                "#!/bin/sh\n",
                "input=$(cat)\n", // 必須把 stdin 讀完，證明 prompt 有送達
                "echo '{\"type\":\"system\",\"subtype\":\"init\"}'\n",
                "echo '{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"你\"}}}'\n",
                "echo '{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"好\"}}}'\n",
                "echo \"{\\\"type\\\":\\\"result\\\",\\\"is_error\\\":false,\\\"result\\\":\\\"你好\\\"}\"\n",
                "test \"$input\" = \"提示詞\" || exit 9\n",
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let mut deltas = Vec::new();
        let full = run_cli(&script, &[], "提示詞", parse_claude_line, |delta| {
            deltas.push(delta.to_owned());
        })
        .await
        .unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(full, "你好");
        assert_eq!(deltas, ["你", "好"]);
    }
}
