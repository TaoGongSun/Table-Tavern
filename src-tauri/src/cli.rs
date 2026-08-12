//! CLI 傳輸層（訂閱模式，NewPlan §3.2／§4.2）。
//! 原則：只偵測不代辦；CLI 是無狀態傳輸——上下文一律由 transport::assemble_messages
//! 組裝、headless 單發、system prompt 覆寫，不依賴 CLI 自身 session（§8.1）。
//! 旗標依當場原始碼／--help 查證：claude 2.1.210、codex-cli 0.145.0、agy 1.1.3、grok 0.2.111。

use crate::data::{DataResult, Tier};
use crate::transport::{ChatMessage, PromptCacheUsage};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

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
    #[cfg(windows)]
    {
        if let Some(profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
            dirs.push(profile.join(".local").join("bin"));
            dirs.push(profile.join(".grok").join("bin"));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            dirs.push(
                local
                    .join("Programs")
                    .join("OpenAI")
                    .join("Codex")
                    .join("bin"),
            );
            dirs.push(local.join("agy").join("bin"));
        }
    }
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

pub(crate) fn find_binary(name: &str) -> Option<PathBuf> {
    // Windows 執行檔帶 .exe 副檔名（四家官方安裝器皆落 .exe）
    let filename = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    candidate_dirs()
        .into_iter()
        .map(|directory| directory.join(&filename))
        .find(|path| is_executable(path))
}

/// 同步跑 `<program> <arg>` 並取 stdout，Windows 下隱藏主控台視窗。
fn hidden_output(program: std::path::PathBuf, arg: &str) -> Option<std::process::Output> {
    let mut command = std::process::Command::new(program);
    command.arg(arg);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .output()
        .ok()
        .filter(|output| output.status.success())
}

/// 單支 CLI 探測。5 秒上限＋kill_on_drop：某支卡住只損失自己，不拖垮其餘三支。
async fn probe_cli(id: &str) -> Option<CliInfo> {
    let path = find_binary(id)?;
    let mut command = Command::new(&path);
    command.arg("--version").kill_on_drop(true);
    // GUI app 下 console 子程序會閃出黑視窗，一律 CREATE_NO_WINDOW
    #[cfg(target_os = "windows")]
    command.creation_flags(0x08000000);
    let version = timeout(Duration::from_secs(5), command.output())
        .await
        .ok()
        .and_then(Result::ok)
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
    Some(CliInfo {
        id: id.to_owned(),
        path: path.to_string_lossy().into_owned(),
        version,
    })
}

pub async fn detect_clis() -> Vec<CliInfo> {
    // 四支並行：總耗時取決於最慢一支而非累加（序列版遇冷啟動／防毒即時掃描要等數十秒）
    let (claude, codex, agy, grok) = tokio::join!(
        probe_cli("claude"),
        probe_cli("codex"),
        probe_cli("agy"),
        probe_cli("grok"),
    );
    [claude, codex, agy, grok].into_iter().flatten().collect()
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

/// 設定 UI 下拉用的模型選項。清單讀自各 CLI 自身（codex：~/.codex/models_cache.json；
/// claude：執行檔內建的模型註冊表），非本程式寫死的正典；
/// 實際用哪個模型仍由 config 的覆寫決定。
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ModelOption {
    pub id: String,
    pub label: String,
}

/// codex 快取解析：跳過內部項與 hidden，依 priority 排序，label 用 display_name
pub fn parse_codex_catalog(json: &str) -> Vec<ModelOption> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let Some(models) = value.get("models").and_then(|m| m.as_array()) else {
        return Vec::new();
    };
    let mut ranked: Vec<(i64, ModelOption)> = models
        .iter()
        .filter_map(|item| {
            let slug = item.get("slug").and_then(|s| s.as_str())?.trim();
            if slug.is_empty() || slug == "codex-auto-review" {
                return None;
            }
            if matches!(
                item.get("visibility").and_then(|v| v.as_str()),
                Some("hidden") | Some("none")
            ) {
                return None;
            }
            let label = item
                .get("display_name")
                .and_then(|s| s.as_str())
                .unwrap_or(slug);
            let priority = item.get("priority").and_then(|p| p.as_i64()).unwrap_or(0);
            Some((
                priority,
                ModelOption {
                    id: slug.to_owned(),
                    label: label.to_owned(),
                },
            ))
        })
        .collect();
    ranked.sort_by_key(|(priority, _)| *priority);
    ranked.into_iter().map(|(_, option)| option).collect()
}

/// 掃 Claude CLI 執行檔內建的模型註冊表（`{id:"…",family:"…",display_name:"…"`）。
/// 官方每次改版都換一份表，因此新模型上線後 `claude update` 即自動出現，不必改本程式。
/// 逐塊掃描：執行檔數百 MB，不整支讀進記憶體；跨塊邊界靠重疊窗＋去重涵蓋。
pub fn parse_claude_registry(source: impl std::io::Read) -> Vec<ModelOption> {
    let Ok(pattern) = regex::bytes::Regex::new(
        r#"\{id:"(claude-[^"]{1,64})",family:"[^"]{1,32}",display_name:"([^"]{1,64})""#,
    ) else {
        return Vec::new();
    };
    const CHUNK: usize = 4 << 20;
    const OVERLAP: usize = 512; // 單筆前綴最長約 200 bytes
    let mut reader = std::io::BufReader::new(source);
    let mut buffer = vec![0u8; CHUNK];
    let mut window: Vec<u8> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut options = Vec::new();
    while let Ok(read) = std::io::Read::read(&mut reader, &mut buffer) {
        if read == 0 {
            break;
        }
        window.extend_from_slice(&buffer[..read]);
        for capture in pattern.captures_iter(&window) {
            let id = String::from_utf8_lossy(&capture[1]).into_owned();
            // 3.x 世代已退場，不佔用下拉版面
            if id.starts_with("claude-3-") || !seen.insert(id.clone()) {
                continue;
            }
            options.push(ModelOption {
                id,
                label: String::from_utf8_lossy(&capture[2]).into_owned(),
            });
        }
        let keep = window.len().min(OVERLAP);
        window.drain(..window.len() - keep);
    }
    options.reverse(); // 註冊表按世代遞增排列，UI 要最新的在最上面
    options
}

/// grok models 只認縮排列；保留原列為 label，去掉預設標記後作為可傳入的 id。
pub fn parse_grok_catalog(output: &str) -> Vec<ModelOption> {
    output
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let label = trimmed.strip_prefix('*')?.trim();
            let id = label.strip_suffix(" (default)").unwrap_or(label).trim();
            (!id.is_empty()).then(|| ModelOption {
                id: id.to_owned(),
                label: label.to_owned(),
            })
        })
        .collect()
}

/// 組下拉目錄：claude 固定前置官方別名（CLI 穩定介面）再接快取；快取讀不到就只剩別名。
/// codex 純靠快取；agy／grok 即時讀 CLI 輸出，讀不到回空（UI 都保留「自訂」手填逃生口）。
pub fn cli_model_catalog(cli: &str) -> Vec<ModelOption> {
    let read = |rel: &[&str]| -> Option<String> {
        let mut path = PathBuf::from(std::env::var_os("HOME")?);
        for part in rel {
            path.push(part);
        }
        std::fs::read_to_string(path).ok()
    };
    match cli {
        "claude" => {
            let mut options: Vec<ModelOption> = ["fable", "opus", "sonnet", "haiku"]
                .iter()
                .map(|alias| ModelOption {
                    id: (*alias).to_owned(),
                    label: format!("{alias}（官方別名）"),
                })
                .collect();
            options.extend(
                find_binary("claude")
                    .and_then(|path| std::fs::File::open(path).ok())
                    .map(parse_claude_registry)
                    .unwrap_or_default(),
            );
            options
        }
        "codex" => read(&[".codex", "models_cache.json"])
            .map(|json| parse_codex_catalog(&json))
            .unwrap_or_default(),
        "agy" => find_binary("agy")
            .and_then(|program| hidden_output(program, "models"))
            .map(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| ModelOption {
                        id: line.to_owned(),
                        label: line.to_owned(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        "grok" => find_binary("grok")
            .and_then(|program| hidden_output(program, "models"))
            .map(|output| parse_grok_catalog(&String::from_utf8_lossy(&output.stdout)))
            .unwrap_or_default(),
        _ => Vec::new(),
    }
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
pub fn claude_model_for(tier: Tier) -> &'static str {
    match tier {
        Tier::Best => "opus",
        Tier::Balanced => "sonnet",
        Tier::Fast => "haiku",
    }
}

/// codex 的檔位對應：模型用 CLI 預設，檔位映射到 reasoning effort
pub fn codex_effort_for(tier: Tier) -> &'static str {
    match tier {
        Tier::Best => "high",
        Tier::Balanced => "medium",
        Tier::Fast => "low",
    }
}

/// --safe-mode：停用使用者的 CLAUDE.md／plugins／hooks，避免 coding 客製污染角色扮演；
/// --tools ""：純文字生成不需要工具；--no-session-persistence：不落 session（§8.1）。
pub fn claude_args(model: &str, system: &str) -> Vec<String> {
    [
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
        "--model",
        model,
    ]
    .map(str::to_owned)
    .to_vec()
}

/// claude lane 續聊（prompt-cache-optimization 包 2）的 session 指定方式。
pub enum ClaudeSession<'a> {
    /// 開新線：session id 由本程式產生（UUID），之後靠它 resume 與定位 session 檔。
    Open(&'a str),
    /// 續聊既有線：實測 resume 沿用同一 id、續寫同一檔（不分叉）。
    Resume(&'a str),
}

/// claude lane 續聊參數：與 claude_args 同組旗標，但保留 session 落檔
/// （resume 架構的快取命中靠 CLI 自身 session，非 §8.1 無狀態單發）。
pub fn claude_session_args(model: &str, system: &str, session: &ClaudeSession<'_>) -> Vec<String> {
    let mut args: Vec<String> = [
        "-p",
        "--verbose", // --print 的 stream-json 硬性要求
        "--safe-mode",
        "--output-format",
        "stream-json",
        "--include-partial-messages",
        "--tools",
        "",
        "--system-prompt",
        system,
        "--model",
        model,
    ]
    .map(str::to_owned)
    .to_vec();
    match session {
        ClaudeSession::Open(id) => {
            args.push("--session-id".to_owned());
            args.push((*id).to_owned());
        }
        ClaudeSession::Resume(id) => {
            args.push("--resume".to_owned());
            args.push((*id).to_owned());
        }
    }
    args
}

/// codex 沒有 system prompt 旗標，呼叫端把 system 併進 prompt。
/// --ignore-user-config：跳過使用者 config.toml（hooks／MCP），auth 不受影響（--help 查證）。
/// allow_tools：生圖呼叫需要 $imagegen 寫檔，沙盒放寬到 workspace-write；聊天一律唯讀。
pub fn codex_args(model: Option<&str>, effort: &str, allow_tools: bool) -> Vec<String> {
    let mut args: Vec<String> = [
        "exec",
        "--json",
        "--ephemeral",
        "--skip-git-repo-check",
        "--ignore-user-config",
        "-s",
        if allow_tools { "workspace-write" } else { "read-only" },
    ]
    .map(str::to_owned)
    .to_vec();
    if let Some(model) = model {
        args.push("-m".to_owned());
        args.push(model.to_owned());
    }
    args.push("-c".to_owned());
    args.push(format!("model_reasoning_effort=\"{effort}\""));
    args.push("-".to_owned()); // prompt 走 stdin，避開參數長度上限
    args
}

/// agy 沒有 system prompt 旗標，呼叫端把 system 併進 prompt。
/// -p 必須直接帶整包 prompt；聊天維持安全預設不開工具。
/// allow_tools：agy 的生圖工具在無頭模式需要 command 權限、提示彈不出來會被自動拒絕
/// （2026-07-27 實測），生圖呼叫必須帶 --dangerously-skip-permissions 才會出圖。
pub fn agy_args(model: Option<&str>, prompt: &str, allow_tools: bool) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(model) = model {
        args.push("--model".to_owned());
        args.push(model.to_owned());
    }
    if allow_tools {
        args.push("--dangerously-skip-permissions".to_owned());
    }
    args.push("-p".to_owned());
    args.push(prompt.to_owned());
    args
}

/// grok 沒有 system prompt 旗標，呼叫端把 system 併進 prompt。
/// 聊天單發一律關閉工具、網路搜尋、計畫與子代理，避免 CLI 執行本機命令。
/// allow_tools：生圖呼叫要用 grok 原生 image_gen 工具，--deny * 換成 --always-approve。
pub fn grok_args(model: Option<&str>, prompt: &str, allow_tools: bool) -> Vec<String> {
    let mut args: Vec<String> = ["--output-format", "streaming-json"]
        .map(str::to_owned)
        .to_vec();
    if allow_tools {
        args.push("--always-approve".to_owned());
    } else {
        args.push("--deny".to_owned());
        args.push("*".to_owned());
    }
    args.extend(
        ["--disable-web-search", "--no-plan", "--no-subagents"].map(str::to_owned),
    );
    if let Some(model) = model {
        args.push("-m".to_owned());
        args.push(model.to_owned());
    }
    args.push("-p".to_owned());
    args.push(prompt.to_owned());
    args
}

#[derive(Debug, PartialEq)]
pub enum CliLine {
    Delta(String),
    /// 思考增量：只餵進度顯示（on_delta），不進正文——長思考段（如卡重構盤點）若無它，
    /// 進度小框會整段空白，玩家分不出「在想」與「掛了」。
    Thinking(String),
    Done { text: String, is_error: bool },
    Other,
}

/// claude --output-format stream-json 逐行解析：
/// text_delta 進正文；thinking_delta 只餵進度顯示（signature 仍略過）；result 事件收尾。
pub fn parse_claude_line(line: &str) -> CliLine {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return CliLine::Other;
    };
    match value.get("type").and_then(|t| t.as_str()) {
        Some("stream_event") => {
            let delta = value.pointer("/event/delta");
            let kind = delta.and_then(|d| d.get("type")).and_then(|t| t.as_str());
            match (
                kind,
                delta.and_then(|d| d.get("text")).and_then(|t| t.as_str()),
                delta.and_then(|d| d.get("thinking")).and_then(|t| t.as_str()),
            ) {
                (Some("text_delta"), Some(text), _) => CliLine::Delta(text.to_owned()),
                // opus 4.7 世代 CLI 隱去思考本文（thinking 恆空、只剩 estimated_tokens），
                // 空增量轉一顆心跳點（約每 50 tok 一筆），字尾才看得出「在想」不是「掛了」。
                (Some("thinking_delta"), _, Some(text)) => CliLine::Thinking(if text.is_empty() {
                    "⋯".to_owned()
                } else {
                    text.to_owned()
                }),
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

/// 各家收尾事件裡的 token 用量。usage 只出現在收尾那一行，增量行不含 "usage" 字串——
/// 先做字串預檢，串流上千行也只有收尾那行真的解析 JSON。
fn usage_event(line: &str, kind: &str) -> Option<serde_json::Value> {
    if !line.contains("\"usage\"") {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.get("type").and_then(|t| t.as_str()) != Some(kind) {
        return None;
    }
    Some(value)
}

fn token_count(usage: &serde_json::Value, field: &str) -> u64 {
    usage.get(field).and_then(|v| v.as_u64()).unwrap_or(0)
}

/// claude result 事件的用量。input_tokens **不含**快取部分（實測：讀滿快取時 input_tokens=1、
/// cache_read=4771），總輸入要把建快取與讀快取加回來。
/// 四支 CLI 只有 claude 直接回報金額（total_cost_usd），額度分頁的花費以它為準。
pub fn parse_claude_usage(line: &str) -> Option<PromptCacheUsage> {
    let value = usage_event(line, "result")?;
    let usage = value.get("usage")?;
    let cached = token_count(usage, "cache_read_input_tokens");
    let created = token_count(usage, "cache_creation_input_tokens");
    Some(PromptCacheUsage {
        prompt_tokens: token_count(usage, "input_tokens") + created + cached,
        cached_tokens: cached,
        created_tokens: created,
        output_tokens: token_count(usage, "output_tokens"),
        cost_usd: value.get("total_cost_usd").and_then(|cost| cost.as_f64()),
    })
}

/// codex turn.completed 事件的用量。input_tokens **已含** cached_input_tokens
/// （codex 自己的顯示邏輯是「非快取輸入＝input − cached」），不再加總。
pub fn parse_codex_usage(line: &str) -> Option<PromptCacheUsage> {
    let value = usage_event(line, "turn.completed")?;
    let usage = value.get("usage")?;
    Some(PromptCacheUsage {
        prompt_tokens: token_count(usage, "input_tokens"),
        cached_tokens: token_count(usage, "cached_input_tokens"),
        created_tokens: token_count(usage, "cache_write_input_tokens"),
        output_tokens: token_count(usage, "output_tokens"),
        cost_usd: None,
    })
}

/// grok end 事件的用量。input_tokens **不含** cache_read_input_tokens
/// （實測讀取數可遠大於輸入數：input=31509、cache_read=146304），總輸入要加總。
/// grok 不回報寫入快取的 token 數（實測 usage 只有 read），created 恆為 0。
/// 金額在 end 事件頂層的 total_cost_usd（grok 0.2.111 實測），缺欄照慣例當 None。
pub fn parse_grok_usage(line: &str) -> Option<PromptCacheUsage> {
    let value = usage_event(line, "end")?;
    let usage = value.get("usage")?;
    let cached = token_count(usage, "cache_read_input_tokens");
    Some(PromptCacheUsage {
        prompt_tokens: token_count(usage, "input_tokens") + cached,
        cached_tokens: cached,
        created_tokens: 0,
        output_tokens: token_count(usage, "output_tokens"),
        cost_usd: value.get("total_cost_usd").and_then(|cost| cost.as_f64()),
    })
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
            match (
                kind,
                item.and_then(|i| i.get("text")).and_then(|t| t.as_str()),
            ) {
                // 一回合可能有多則 agent_message（前導說明＋結論），補換行才不會黏成一句
                (Some("agent_message"), Some(text)) => CliLine::Delta(format!("{text}\n")),
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

/// agy 輸出純文字；逐行補回換行，包含空行，以 EOF 作為回合結束。
pub fn parse_agy_line(line: &str) -> CliLine {
    CliLine::Delta(format!("{line}\n"))
}

/// grok --output-format streaming-json 逐行解析：thought 不進對話，text 為增量，end 正常收尾。
pub fn parse_grok_line(line: &str) -> CliLine {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return CliLine::Other;
    };
    match value.get("type").and_then(|t| t.as_str()) {
        Some("text") => value
            .get("data")
            .and_then(|data| data.as_str())
            .map(|text| CliLine::Delta(text.to_owned()))
            .unwrap_or(CliLine::Other),
        Some("end") => CliLine::Done {
            text: String::new(),
            is_error: false,
        },
        Some("error") => CliLine::Done {
            text: value
                .get("data")
                .or_else(|| value.get("message"))
                .and_then(|message| message.as_str())
                .unwrap_or("Grok CLI 回合失敗")
                .to_owned(),
            is_error: true,
        },
        _ => CliLine::Other,
    }
}

/// CLI 路徑的用量落檔設定（prompt-cache-optimization：CLI 量測）。
/// 帶著就在收尾事件把這次呼叫追加成一行 JSONL，與 API 那條共用同一份檔案與格式。
pub struct UsageLog<'a> {
    pub path: &'a Path,
    /// 這次呼叫屬於哪一桌；None＝開桌生成等不屬於任何一桌的呼叫。
    pub world: Option<&'a str>,
    /// log 的 transport 欄位，例如 "claude"。
    pub transport: &'a str,
    pub model: &'a str,
    pub parse: fn(&str) -> Option<PromptCacheUsage>,
    /// 續聊線的脈絡；None＝單發路徑，只記數字不做診斷。
    pub lane: Option<crate::usage_log::LaneContext>,
    /// 回填本輪總輸入，供 lane 記成下輪的理論可中量（跨 await 需 Sync，故用 atomic）。
    pub prompt_tokens_out: Option<&'a std::sync::atomic::AtomicU64>,
}

/// `run_cli` spawn 後掛的反登記保險：不論提早 return、`?` 冒出的錯誤，還是外層 future
/// 被 select 取消（中止在途呼叫）整個被 drop，這個 guard 的 Drop 都會觸發，
/// 確保 inflight 的子程序 PID 表不留殘影。
struct ChildPidGuard(Option<u32>);

impl Drop for ChildPidGuard {
    fn drop(&mut self) {
        if let Some(pid) = self.0 {
            crate::inflight::unregister_child(pid);
        }
    }
}

/// headless 單發：prompt 走 stdin，逐行讀 stdout 解析、增量回呼，回傳完整文字。
/// stderr 行的 API 錯誤偵測：None＝不是錯誤行；Some(false)＝暫時性（讓 CLI 自己重試，
/// 但進度要立刻看得到）；Some(true)＝設定類（模型不存在／認證／權限），重試不會好，立即中止。
fn api_error_kind(line: &str) -> Option<bool> {
    let lower = line.to_lowercase();
    if !lower.contains("api error") {
        return None;
    }
    const FATAL: [&str; 8] = [
        "unknown provider",
        "not_found",
        "not found",
        "invalid",
        "authentication",
        "unauthorized",
        "permission",
        "billing",
    ];
    Some(
        FATAL.iter().any(|kw| lower.contains(kw))
            || ["401", "403", "404"].iter().any(|code| lower.contains(code)),
    )
}

pub async fn run_cli(
    program: &Path,
    working_dir: &Path,
    args: &[String],
    stdin_data: &str,
    envs: &[(String, String)],
    parse: fn(&str) -> CliLine,
    // 思考增量要不要餵給 on_delta：只有「進度字尾」型顯示（卡重構）開 true；
    // 聊天／旁白的 on_delta 是劇情正文串流，思考混進去會出戲。
    thinking_to_delta: bool,
    usage_log: Option<UsageLog<'_>>,
    mut on_delta: impl FnMut(&str),
) -> DataResult<String> {
    let mut command = Command::new(program);
    // 先掛系統代理再掛使用者 envs，同名時使用者設定蓋過代理
    crate::proxy::apply_system_proxy(&mut command);
    // CLI 子程序一律不繼承 ANTHROPIC_*：啟動 app 的 shell 若殘留閘道變數（例如指向
    // 本機代理的 ANTHROPIC_BASE_URL＋AUTH_TOKEN），整批呼叫會被劫走。要接第三方閘道
    // 一律走 app 設定欄，claude_cli_envs 會在下面的 envs 顯式補回。
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("ANTHROPIC_") {
            command.env_remove(&key);
        }
    }
    command
        .current_dir(working_dir)
        .args(args)
        .envs(
            envs.iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    command.creation_flags(0x08000000);
    // 呼叫端 future 被 drop（中止在途呼叫時 select 輸掉的分支）時，tokio 順手殺子程序。
    command.kill_on_drop(true);
    let mut child = command.spawn()?;
    if let Some(pid) = child.id() {
        crate::inflight::register_child(pid);
    }
    let _pid_guard = ChildPidGuard(child.id());

    let mut stdin = child.stdin.take().expect("stdin piped");
    // 死法③：CLI 起來但不收 stdin（掛在啟動）＝write_all 永卡，60 秒收不完就中止
    tokio::time::timeout(
        std::time::Duration::from_secs(60),
        stdin.write_all(stdin_data.as_bytes()),
    )
    .await
    .map_err(|_| "CLI 60 秒收不進提示詞，已中止")??;
    drop(stdin); // 關閉讓 CLI 知道輸入結束

    // stderr 逐行即時讀（同時兼排空防死鎖）：CLI 的「API Error…重試中」通知走 stderr，
    // 整包等結束才讀會讓玩家對著靜止的進度框發呆到 CLI 重試放棄為止。
    let stderr = child.stderr.take().expect("stderr piped");
    let mut stderr_lines = BufReader::new(stderr).lines();
    let mut stderr_text = String::new();

    let stdout = child.stdout.take().expect("stdout piped");
    let mut lines = BufReader::new(stdout).lines();
    let mut full_text = String::new();
    let mut done: Option<(String, bool)> = None;
    let mut stdout_open = true;
    let mut stderr_open = true;
    // 子程序死法收網（2026-08-12，跨平台 tokio API）：
    // ①程序退出但管線不 EOF（孫程序繼承 fd）：退出後 800ms 沒新行＝強制收尾；
    // ②程序活著但斷流（網路死、CLI 內部卡死）：120 秒無任何 stdout/stderr 行＝殺程序回錯；
    // ③stdin 餵不進（上方 60 秒逾時）；④crash 無收尾事件（迴圈後 exit status 檢查）。
    let mut exited = false;
    let mut stall: Option<String> = None;
    while stdout_open || stderr_open {
        let line = tokio::select! {
            biased;
            line = lines.next_line(), if stdout_open => match line? {
                Some(line) => line,
                None => {
                    stdout_open = false;
                    continue;
                }
            },
            line = stderr_lines.next_line(), if stderr_open => {
                match line? {
                    Some(line) => {
                        if let Some(fatal) = api_error_kind(&line) {
                            // 進度字尾型顯示（卡重構）立刻看得到錯誤；正文串流不混入
                            if thinking_to_delta {
                                on_delta(&format!("\n⚠ {line}\n"));
                            }
                            if fatal {
                                // 設定類錯誤重試不會好，立即中止（kill_on_drop 收掉子程序）
                                return Err(format!("CLI 回覆錯誤：{line}").into());
                            }
                        }
                        stderr_text.push_str(&line);
                        stderr_text.push('\n');
                    }
                    None => stderr_open = false,
                }
                continue;
            },
            status = child.wait(), if !exited => {
                let _ = status?;
                exited = true;
                continue;
            },
            _ = tokio::time::sleep(std::time::Duration::from_millis(800)), if exited => {
                // 程序已亡、管線遲不 EOF＝孫程序繼承了 fd，放棄排空強制收尾
                stdout_open = false;
                stderr_open = false;
                continue;
            },
            _ = tokio::time::sleep(std::time::Duration::from_secs(120)), if !exited => {
                stall = Some("CLI 120 秒無任何輸出（網路或程序卡死），已中止".to_owned());
                break;
            },
        };
        if let Some(log) = &usage_log {
            if let Some(usage) = (log.parse)(&line) {
                eprintln!(
                    "[prompt-cache] transport={} model={} lane={} prompt_tokens={} cached_tokens={} created_tokens={} hit_rate={:.0}%",
                    log.transport,
                    log.model,
                    log.lane.as_ref().map_or("-", |lane| lane.lane.as_str()),
                    usage.prompt_tokens,
                    usage.cached_tokens,
                    usage.created_tokens,
                    usage.hit_rate(),
                );
                crate::usage_log::append_call(
                    log.path,
                    log.world,
                    log.transport,
                    log.model,
                    log.lane.as_ref(),
                    usage,
                );
                if let Some(slot) = log.prompt_tokens_out {
                    slot.store(usage.prompt_tokens, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }
        match parse(&line) {
            CliLine::Delta(text) => {
                on_delta(&text);
                full_text.push_str(&text);
            }
            CliLine::Thinking(text) => {
                if thinking_to_delta {
                    on_delta(&text);
                }
            }
            CliLine::Done { text, is_error } => done = Some((text, is_error)),
            CliLine::Other => {}
        }
    }

    if let Some(msg) = stall {
        let _ = child.start_kill();
        if thinking_to_delta {
            on_delta(&format!("\n⚠ {msg}\n"));
        }
        return Err(format!("CLI 回覆錯誤：{msg}").into());
    }
    let status = child.wait().await?;
    if let Some((text, true)) = &done {
        return Err(format!("CLI 回覆錯誤：{text}").into());
    }
    if done.is_none() && !status.success() {
        // 死法④：CLI crash／被系統殺（無收尾事件＋exit 非零）——殘缺正文不能往下走
        let tail: String = stderr_text
            .lines()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .join("\n");
        let msg = format!("CLI 異常結束（{status}）：{tail}");
        if thinking_to_delta {
            on_delta(&format!("\n⚠ {msg}\n"));
        }
        return Err(format!("CLI 回覆錯誤：{msg}").into());
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
        let tail: String = stderr_text
            .lines()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .join("\n");
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
            CliLine::Thinking("内心".to_owned())
        );
        // 2026-08-12 實測樣本：4.7 世代 CLI 思考本文隱去，空增量轉心跳點
        assert_eq!(
            parse_claude_line(
                r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"","estimated_tokens":50}}}"#
            ),
            CliLine::Thinking("⋯".to_owned())
        );
        assert_eq!(
            parse_claude_line(
                r#"{"type":"result","subtype":"success","is_error":false,"result":"測試"}"#
            ),
            CliLine::Done {
                text: "測試".to_owned(),
                is_error: false
            }
        );
        assert_eq!(
            parse_claude_line(
                r#"{"type":"result","is_error":true,"result":"Failed to authenticate. API Error: 401 Invalid bearer token"}"#
            ),
            CliLine::Done {
                text: "Failed to authenticate. API Error: 401 Invalid bearer token".to_owned(),
                is_error: true
            }
        );
        assert_eq!(parse_claude_line("not json"), CliLine::Other);
    }

    /// 樣本取自 2026-08-03 真實冒煙輸出（同一段 system prompt 連跑兩次：第一次建快取、
    /// 第二次全命中）。各家 input_tokens 語意不同，這裡鎖住換算後的「總輸入／讀快取」。
    #[test]
    fn parses_claude_usage_adding_cache_tokens_to_input() {
        // 第一次：建快取 4771，讀 0 → 總輸入 4772、命中 0%；金額直接取 CLI 回報值
        assert_eq!(
            parse_claude_usage(
                r#"{"type":"result","total_cost_usd":0.0179,"usage":{"input_tokens":1,"cache_creation_input_tokens":4771,"cache_read_input_tokens":0,"output_tokens":3}}"#
            ),
            Some(PromptCacheUsage {
                prompt_tokens: 4772,
                cached_tokens: 0,
                created_tokens: 4771,
                output_tokens: 3,
                cost_usd: Some(0.0179),
            })
        );
        // 第二次：讀滿 4771 → 總輸入 4772、命中 100%
        let hit = parse_claude_usage(
            r#"{"type":"result","usage":{"input_tokens":1,"cache_creation_input_tokens":0,"cache_read_input_tokens":4771,"output_tokens":3}}"#
        )
        .expect("result 事件有 usage");
        assert_eq!(hit.prompt_tokens, 4772);
        assert_eq!(hit.cached_tokens, 4771);
        assert!((hit.hit_rate() - 99.98).abs() < 0.01);
        // 增量行與非收尾事件不出數字
        assert_eq!(
            parse_claude_usage(
                r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"好"}}}"#
            ),
            None
        );
        assert_eq!(parse_claude_usage("not json"), None);
    }

    /// codex 的 input_tokens 已含 cached_input_tokens，不可再加總（會虛報分母、低估命中率）。
    #[test]
    fn parses_codex_usage_without_double_counting_cached_input() {
        let usage = parse_codex_usage(
            r#"{"type":"turn.completed","usage":{"input_tokens":23144,"cached_input_tokens":11008,"cache_write_input_tokens":0,"output_tokens":5}}"#
        )
        .expect("turn.completed 有 usage");
        assert_eq!(usage.prompt_tokens, 23144);
        assert_eq!(usage.cached_tokens, 11008);
        assert!((usage.hit_rate() - 47.56).abs() < 0.01);
        assert_eq!(parse_codex_usage(r#"{"type":"turn.started"}"#), None);
    }

    /// grok 的 cache_read 可遠大於 input_tokens，證明兩者不重疊，總輸入要加總。
    /// 金額在頂層 total_cost_usd（包 6 額度分頁要顯示），缺欄時仍能解析、只是沒金額。
    #[test]
    fn parses_grok_usage_adding_cache_read_to_input() {
        let usage = parse_grok_usage(
            r#"{"type":"end","stopReason":"EndTurn","total_cost_usd":0.0421,"usage":{"input_tokens":31509,"cache_read_input_tokens":146304,"output_tokens":1416,"total_tokens":179229}}"#
        )
        .expect("end 事件有 usage");
        assert_eq!(usage.prompt_tokens, 177813);
        assert_eq!(usage.cached_tokens, 146304);
        assert_eq!(usage.cost_usd, Some(0.0421));
        assert!((usage.hit_rate() - 82.28).abs() < 0.01);
        assert_eq!(
            parse_grok_usage(r#"{"type":"end","usage":{"input_tokens":10}}"#)
                .expect("缺金額欄仍解析")
                .cost_usd,
            None
        );
        assert_eq!(parse_grok_usage(r#"{"type":"text","data":"好"}"#), None);
    }

    /// 缺欄位當 0，不可 panic 也不可整筆丟掉（CLI 版本變動時仍留下可讀的一行）。
    #[test]
    fn missing_usage_fields_count_as_zero() {
        assert_eq!(
            parse_claude_usage(r#"{"type":"result","usage":{"input_tokens":12}}"#),
            Some(PromptCacheUsage {
                prompt_tokens: 12,
                cached_tokens: 0,
                created_tokens: 0,
                output_tokens: 0,
                cost_usd: None, // 沒回報金額就不記，額度分頁靠有值的那些行加總
            })
        );
        assert_eq!(
            PromptCacheUsage {
                prompt_tokens: 0,
                cached_tokens: 0,
                created_tokens: 0,
                output_tokens: 0,
                cost_usd: None,
            }
            .hit_rate(),
            0.0
        );
    }

    #[test]
    fn parses_real_codex_json_lines_and_ignores_warning_items() {
        assert_eq!(
            parse_codex_line(
                r#"{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"測試"}}"#
            ),
            CliLine::Delta("測試\n".to_owned())
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
    fn parses_agy_plain_text_lines_and_preserves_paragraphs() {
        assert_eq!(
            parse_agy_line("一般文字"),
            CliLine::Delta("一般文字\n".to_owned())
        );
        assert_eq!(parse_agy_line(""), CliLine::Delta("\n".to_owned()));
        assert!(matches!(
            parse_agy_line(r#"{"type":"result"}"#),
            CliLine::Delta(_)
        ));
    }

    #[test]
    fn agy_args_put_prompt_in_final_p_value_with_optional_model() {
        let prompt = "system\n\n整包 prompt（含空格）";
        assert_eq!(
            agy_args(Some("Claude Sonnet 4.6 (Thinking)"), prompt, false),
            ["--model", "Claude Sonnet 4.6 (Thinking)", "-p", prompt]
        );
        assert_eq!(agy_args(None, prompt, false), ["-p", prompt]);
    }

    #[test]
    fn grok_args_disable_every_tool_and_put_prompt_last() {
        let prompt = "system\n\n整包 prompt（含空格）";
        let args = grok_args(Some("grok-4.5"), prompt, false);
        assert!(args
            .windows(2)
            .any(|pair| pair == ["--output-format", "streaming-json"]));
        assert!(args.windows(2).any(|pair| pair == ["--deny", "*"]));
        assert!(args.contains(&"--disable-web-search".to_owned()));
        assert!(args.contains(&"--no-plan".to_owned()));
        assert!(args.contains(&"--no-subagents".to_owned()));
        assert!(args.windows(2).any(|pair| pair == ["-m", "grok-4.5"]));
        assert_eq!(args[args.len() - 2..], ["-p", prompt]);
        let default_args = grok_args(None, prompt, false);
        assert_eq!(default_args[default_args.len() - 2..], ["-p", prompt]);
    }

    #[test]
    fn parses_grok_streaming_json_lines() {
        assert_eq!(
            parse_grok_line(r#"{"type":"text","data":"測試"}"#),
            CliLine::Delta("測試".to_owned())
        );
        assert_eq!(
            parse_grok_line(r#"{"type":"thought","data":"推理"}"#),
            CliLine::Other
        );
        assert_eq!(parse_grok_line(r#"{"type":"unknown"}"#), CliLine::Other);
        assert_eq!(parse_grok_line("not json"), CliLine::Other);
        assert_eq!(
            parse_grok_line(r#"{"type":"end","stopReason":"EndTurn"}"#),
            CliLine::Done {
                text: String::new(),
                is_error: false
            }
        );
        assert_eq!(
            parse_grok_line(r#"{"type":"error","data":"quota exceeded"}"#),
            CliLine::Done {
                text: "quota exceeded".to_owned(),
                is_error: true
            }
        );
        assert_eq!(
            parse_grok_line(r#"{"type":"error"}"#),
            CliLine::Done {
                text: "Grok CLI 回合失敗".to_owned(),
                is_error: true
            }
        );
    }

    #[test]
    fn tier_mappings_cover_all_tiers() {
        assert_eq!(claude_model_for(Tier::Best), "opus");
        assert_eq!(claude_model_for(Tier::Fast), "haiku");
        assert_eq!(codex_effort_for(Tier::Balanced), "medium");
        let args = codex_args(None, codex_effort_for(Tier::Best), false);
        assert!(args.contains(&"model_reasoning_effort=\"high\"".to_owned()));
        assert!(!args.contains(&"-m".to_owned()));
        assert_eq!(args.last().unwrap(), "-");
        let args = codex_args(Some("gpt-5.6-terra"), codex_effort_for(Tier::Fast), false);
        assert!(args.windows(2).any(|w| w == ["-m", "gpt-5.6-terra"]));
        let args = claude_args(claude_model_for(Tier::Fast), "系統");
        assert!(args.windows(2).any(|w| w == ["--model", "haiku"]));
        assert!(args.windows(2).any(|w| w == ["--system-prompt", "系統"]));
    }

    /// lane 續聊參數必須保留 session 落檔（無 --no-session-persistence），
    /// 開線帶 --session-id、續聊帶 --resume，其餘旗標與單發相同。
    #[test]
    fn claude_session_args_keep_persistence_and_pick_session_flag() {
        let opened = claude_session_args("sonnet", "系統", &ClaudeSession::Open("uuid-1"));
        assert!(!opened.contains(&"--no-session-persistence".to_owned()));
        assert!(opened.windows(2).any(|w| w == ["--session-id", "uuid-1"]));
        assert!(opened.windows(2).any(|w| w == ["--system-prompt", "系統"]));
        assert!(opened.windows(2).any(|w| w == ["--model", "sonnet"]));
        let resumed = claude_session_args("sonnet", "系統", &ClaudeSession::Resume("uuid-1"));
        assert!(resumed.windows(2).any(|w| w == ["--resume", "uuid-1"]));
        assert!(!resumed.contains(&"--session-id".to_owned()));
    }

    #[test]
    fn codex_catalog_skips_internal_and_hidden_and_sorts_by_priority() {
        let json = r#"{"models":[
            {"slug":"codex-auto-review","display_name":"內部"},
            {"slug":"gpt-5.4","display_name":"GPT-5.4","priority":2},
            {"slug":"gpt-5.6-sol","display_name":"GPT-5.6-Sol","priority":1},
            {"slug":"secret-model","visibility":"hidden"}
        ]}"#;
        let ids: Vec<_> = parse_codex_catalog(json)
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(ids, ["gpt-5.6-sol", "gpt-5.4"]);
        assert!(parse_codex_catalog("not json").is_empty());
    }

    /// 一筆註冊表記錄（與 CLI 執行檔內的形態一致，後面還接著其他欄位）
    fn registry_entry(id: &str, label: &str) -> String {
        format!(r#"{{id:"{id}",family:"opus",display_name:"{label}",knowledge_cutoff:"May 2026"}},"#)
    }

    #[test]
    fn claude_registry_lists_newest_first_and_drops_legacy() {
        let binary = format!(
            "somejsnoise{}{}{}{}",
            registry_entry("claude-3-5-haiku", "Haiku 3.5"),
            registry_entry("claude-opus-4-6", "Opus 4.6"),
            registry_entry("claude-opus-5", "Opus 5"),
            registry_entry("claude-opus-4-6", "Opus 4.6"), // 表內重複定義只留一筆
        );
        let options = parse_claude_registry(binary.as_bytes());
        assert_eq!(
            options,
            vec![
                ModelOption {
                    id: "claude-opus-5".to_owned(),
                    label: "Opus 5".to_owned(),
                },
                ModelOption {
                    id: "claude-opus-4-6".to_owned(),
                    label: "Opus 4.6".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn claude_registry_catches_entry_across_chunk_boundary() {
        // 讓記錄剛好橫跨 4 MiB 讀取塊的邊界，驗證重疊窗有接住
        let entry = registry_entry("claude-opus-5", "Opus 5");
        let pad = (4 << 20) - entry.len() / 2;
        let binary = format!("{}{entry}", "x".repeat(pad));
        let options = parse_claude_registry(binary.as_bytes());
        assert_eq!(options.len(), 1);
        assert_eq!(options[0].id, "claude-opus-5");
    }

    #[test]
    fn grok_catalog_ignores_noise_and_strips_default_marker() {
        let output = "You are not authenticated.\nDefault model: grok-4.5\nAvailable models:\n  * grok-4.5 (default)\n  * grok-4.1-fast\n";
        assert_eq!(
            parse_grok_catalog(output),
            vec![
                ModelOption {
                    id: "grok-4.5".to_owned(),
                    label: "grok-4.5 (default)".to_owned()
                },
                ModelOption {
                    id: "grok-4.1-fast".to_owned(),
                    label: "grok-4.1-fast".to_owned()
                },
            ]
        );
    }

    #[test]
    fn tier_override_reads_prefixed_keys_and_ignores_blank() {
        let mut map = std::collections::BTreeMap::new();
        map.insert("claude:best".to_owned(), "claude-fable-5".to_owned());
        map.insert("claude:fast".to_owned(), "  ".to_owned());
        map.insert("best".to_owned(), "vendor/api-model".to_owned()); // API 檔位不受影響
        assert_eq!(
            tier_override(&map, "claude", Tier::Best),
            Some("claude-fable-5")
        );
        assert_eq!(tier_override(&map, "claude", Tier::Fast), None); // 空白＝未設
        assert_eq!(tier_override(&map, "codex", Tier::Best), None);
    }

    /// 以假 CLI 腳本走完 spawn→stdin→逐行解析→增量→收尾整條路（sh 腳本，僅 unix）
    #[cfg(unix)]
    #[tokio::test]
    async fn run_cli_streams_deltas_from_fake_cli_and_reads_stdin() {
        // run_cli 現在會把子程序 pid 登記進 inflight 的全域 children 表；kill_all_children
        // 的測試（inflight.rs）會不分青紅皂白殺表上全部 pid，故用同一把鎖互斥執行。
        let _serial = crate::inflight::lock_real_process_tests();
        let dir = std::env::temp_dir().join(format!("tt-fake-cli-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let working_dir = dir.join("workspace");
        std::fs::create_dir_all(&working_dir).unwrap();
        std::fs::write(working_dir.join("cwd-marker"), "").unwrap();
        let script = dir.join("fake-claude.sh");
        std::fs::write(
            &script,
            concat!(
                "#!/bin/sh\n",
                "input=$(cat)\n", // 必須把 stdin 讀完，證明 prompt 有送達
                "test -f ./cwd-marker || exit 8\n",
                "echo '{\"type\":\"system\",\"subtype\":\"init\"}'\n",
                "echo '{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"想\"}}}'\n",
                "echo '{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"你\"}}}'\n",
                "echo '{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"好\"}}}'\n",
                "echo \"{\\\"type\\\":\\\"result\\\",\\\"is_error\\\":false,\\\"result\\\":\\\"你好\\\",\\\"total_cost_usd\\\":0.0015,\\\"usage\\\":{\\\"input_tokens\\\":1,\\\"cache_creation_input_tokens\\\":0,\\\"cache_read_input_tokens\\\":99,\\\"output_tokens\\\":2}}\"\n",
                "test \"$input\" = \"提示詞\" || exit 9\n",
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let log_path = dir.join("prompt-cache.jsonl");
        let seen = std::sync::atomic::AtomicU64::new(0);
        let mut deltas = Vec::new();
        let full = run_cli(
            &script,
            &working_dir,
            &[],
            "提示詞",
            &[],
            parse_claude_line,
            true,
            Some(UsageLog {
                path: &log_path,
                world: Some("w1"),
                transport: "claude",
                model: "sonnet",
                parse: parse_claude_usage,
                lane: None,
                prompt_tokens_out: Some(&seen),
            }),
            |delta: &str| {
                deltas.push(delta.to_owned());
            },
        )
        .await
        .unwrap();
        // 同一份輸出、關掉思考轉發：聊天正文串流不得混進思考
        let mut quiet_deltas = Vec::new();
        let quiet = run_cli(
            &script,
            &working_dir,
            &[],
            "提示詞",
            &[],
            parse_claude_line,
            false,
            None,
            |delta: &str| {
                quiet_deltas.push(delta.to_owned());
            },
        )
        .await
        .unwrap();
        let logged = std::fs::read_to_string(&log_path).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(quiet, "你好");
        assert_eq!(quiet_deltas, ["你", "好"]);
        // 思考增量進顯示流、不進正文
        assert_eq!(full, "你好");
        assert_eq!(deltas, ["想", "你", "好"]);
        // 收尾事件落一行 JSONL：總輸入 100（1＋0＋99）、讀快取 99 → 99%
        assert_eq!(logged.lines().count(), 1);
        let record: serde_json::Value = serde_json::from_str(logged.trim()).unwrap();
        assert_eq!(record["transport"], "claude");
        assert_eq!(record["world"], "w1");
        assert_eq!(record["model"], "sonnet");
        assert_eq!(record["prompt_tokens"], 100);
        assert_eq!(record["cached_tokens"], 99);
        assert_eq!(record["created_tokens"], 0);
        assert_eq!(record["output_tokens"], 2);
        assert_eq!(record["hit_rate"], 99.0);
        assert_eq!(record["cost_usd"], 0.0015);
        // 非續聊路徑不做診斷；時間戳到秒（分鐘精度分不出是否踩到 5 分鐘過期線）
        assert_eq!(record["diag"], "single");
        assert_eq!(record["ts"].as_str().unwrap().len(), 19);
        // 總輸入回填給呼叫端，續聊線用它當下輪的理論可中量
        assert_eq!(seen.load(std::sync::atomic::Ordering::Relaxed), 100);
    }

    #[tokio::test]
    async fn run_cli_aborts_instantly_on_fatal_stderr_api_error_and_shows_it_in_tail() {
        let _serial = crate::inflight::lock_real_process_tests();
        let dir = std::env::temp_dir().join(format!("tt-fake-cli-fatal-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fake-claude-fatal.sh");
        // stderr 吐設定類 API 錯誤後長睡（模擬 CLI 自己退避重試）；沒有立即中止就會撞測試逾時
        std::fs::write(
            &script,
            concat!(
                "#!/bin/sh\n",
                "cat > /dev/null\n",
                "echo 'API Error: 502 unknown provider for model claude-opus-4-7' >&2\n",
                "sleep 30\n",
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let mut deltas = Vec::new();
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            run_cli(
                &script,
                &dir,
                &[],
                "提示詞",
                &[],
                parse_claude_line,
                true,
                None,
                |delta: &str| deltas.push(delta.to_owned()),
            ),
        )
        .await
        .expect("設定類錯誤必須立即中止，不得等 CLI 睡完");
        let error = result.unwrap_err().to_string();
        assert!(error.contains("unknown provider"), "錯誤要帶原文：{error}");
        // 進度字尾也要同步看到錯誤行
        assert!(deltas.iter().any(|d| d.contains("API Error")));
    }

    #[tokio::test]
    async fn run_cli_reports_crash_without_result_event_instead_of_returning_partial_text() {
        let _serial = crate::inflight::lock_real_process_tests();
        let dir = std::env::temp_dir().join(format!("tt-fake-cli-crash-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fake-claude-crash.sh");
        // 吐一筆正文增量後 crash（無 result 收尾事件）：殘缺正文不得當成功返回
        std::fs::write(
            &script,
            concat!(
                "#!/bin/sh\n",
                "cat > /dev/null\n",
                "echo '{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"殘\"}}}'\n",
                "echo 'proxy connection reset' >&2\n",
                "exit 3\n",
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let mut deltas = Vec::new();
        let error = run_cli(
            &script,
            &dir,
            &[],
            "提示詞",
            &[],
            parse_claude_line,
            true,
            None,
            |delta: &str| deltas.push(delta.to_owned()),
        )
        .await
        .unwrap_err()
        .to_string();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(error.contains("CLI 異常結束"), "要報 crash 而非靜默：{error}");
        assert!(error.contains("proxy connection reset"), "要帶 stderr 尾巴：{error}");
        // 進度字尾同步看到 ⚠，玩家不用等到收尾才知道
        assert!(deltas.iter().any(|d| d.contains('⚠')));
    }

    #[tokio::test]
    async fn run_cli_strips_inherited_anthropic_env_but_keeps_explicit_envs() {
        let _serial = crate::inflight::lock_real_process_tests();
        let dir = std::env::temp_dir().join(format!("tt-fake-cli-env-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fake-claude-env.sh");
        // 繼承的 ANTHROPIC_BASE_URL 必須被拔掉；顯式 envs 傳入的 MARKER 必須到位
        std::fs::write(
            &script,
            concat!(
                "#!/bin/sh\n",
                "cat > /dev/null\n",
                "test -z \"$ANTHROPIC_BASE_URL\" || exit 7\n",
                "test \"$ANTHROPIC_MARKER\" = \"explicit\" || exit 9\n",
                "echo '{\"type\":\"result\",\"is_error\":false,\"result\":\"ok\"}'\n",
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        std::env::set_var("ANTHROPIC_BASE_URL", "http://127.0.0.1:9999");
        let result = run_cli(
            &script,
            &dir,
            &[],
            "提示詞",
            &[("ANTHROPIC_MARKER".to_owned(), "explicit".to_owned())],
            parse_claude_line,
            false,
            None,
            |_: &str| {},
        )
        .await;
        std::env::remove_var("ANTHROPIC_BASE_URL");
        assert_eq!(result.unwrap(), "ok");
    }

    #[test]
    fn api_error_kind_classifies_fatal_vs_transient() {
        // 暫時性：讓 CLI 重試，但 Some(false) 表示要餵進度
        assert_eq!(api_error_kind("API Error: 529 overloaded, retrying"), Some(false));
        // 設定類：模型不存在／認證，立即中止
        assert_eq!(
            api_error_kind("API Error: 502 unknown provider for model x"),
            Some(true)
        );
        assert_eq!(api_error_kind("API Error: 401 authentication_error"), Some(true));
        // 非錯誤行不動作
        assert_eq!(api_error_kind("thinking hard..."), None);
    }
}

