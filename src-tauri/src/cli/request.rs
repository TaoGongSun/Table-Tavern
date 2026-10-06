use super::types::CliSession;
use crate::data::Tier;
use crate::transport::{history_header, speaker_prefix, ChatMessage};
use std::path::Path;

/// 把共用組裝結果攤平成 CLI 單發需要的 (system, prompt)。
/// assistant 訊息即本發言者（角色或 GM）過往內容，攤平時補回名字前綴；
/// closing 為收尾指示，由呼叫端依發言者身分決定。
/// 兩個參數傳空字串＝「這份 messages 已經自足」：共線組裝（`assemble_shared_messages`）
/// 的台詞自帶「名字：」前綴、本輪指定已在尾端那則 user，再補 label 會變成
/// 「加爾：雷恩：……」、再補 closing 會讓指示出現兩次。
pub fn flatten_messages(
    assistant_label: &str,
    closing: &str,
    messages: &[ChatMessage],
    lang: &str,
) -> (String, String) {
    let system = messages
        .first()
        .map(|message| message.content.clone())
        .unwrap_or_default();
    let history: Vec<String> = messages
        .iter()
        .skip(1)
        .map(|message| {
            if message.role == "assistant" && !assistant_label.is_empty() {
                format!(
                    "{}{}",
                    speaker_prefix(assistant_label, lang),
                    message.content
                )
            } else {
                message.content.clone()
            }
        })
        .collect();
    let history = history.join("\n\n");
    let header = history_header(lang);
    let prompt = match closing.is_empty() {
        true => format!("{header}{history}"),
        false => format!("{header}{history}\n\n——\n{closing}"),
    };
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
/// system 走 `--system-prompt-file`（`--help` 沒列、`--bare` 說明有提），正文走 stdin：
/// 兩者都不佔命令列長度（Windows 32,767 UTF-16 單位、macOS argv＋env 1MB）。
pub fn claude_args(model: &str, system_file: &Path) -> Vec<String> {
    let system_file = system_file.to_string_lossy();
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
        "--system-prompt-file",
        &system_file,
        "--model",
        model,
    ]
    .map(str::to_owned)
    .to_vec()
}

/// claude lane 續聊參數：與 claude_args 同組旗標，但保留 session 落檔
/// （resume 架構的快取命中靠 CLI 自身 session，非 §8.1 無狀態單發）。
pub fn claude_session_args(
    model: &str,
    system_file: &Path,
    session: &CliSession<'_>,
) -> Vec<String> {
    let system_file = system_file.to_string_lossy();
    let mut args: Vec<String> = [
        "-p",
        "--verbose", // --print 的 stream-json 硬性要求
        "--safe-mode",
        "--output-format",
        "stream-json",
        "--include-partial-messages",
        "--tools",
        "",
        "--system-prompt-file",
        &system_file,
        "--model",
        model,
    ]
    .map(str::to_owned)
    .to_vec();
    match session {
        CliSession::Open(id) => {
            args.push("--session-id".to_owned());
            args.push((*id).to_owned());
        }
        CliSession::Resume(id) => {
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
        if allow_tools {
            "workspace-write"
        } else {
            "read-only"
        },
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

/// agy 的 `--output-format` 與 usage 回報是 1.1.8 才有的；更舊的版本會因為不認得旗標
/// 當場失敗。偵測到的版本字串解析不出來時放行——寧可讓呼叫失敗時帶著 CLI 自己的錯誤訊息，
/// 也不要因為版本字串換了格式就把整條路擋死。
pub fn agy_supports_stream_json(version: &str) -> bool {
    let numbers: Vec<u64> = version
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .take(3)
        .filter_map(|part| part.parse().ok())
        .collect();
    match numbers.as_slice() {
        [major, minor, patch] => (*major, *minor, *patch) >= (1, 1, 8),
        _ => true,
    }
}

/// agy 沒有 system prompt 旗標，呼叫端把 system 併進正文。正文走 stdin（不帶 `-p`
/// 就是單發模式），不佔命令列長度；聊天維持安全預設不開工具。
/// allow_tools：agy 的生圖工具在無頭模式需要 command 權限、提示彈不出來會被自動拒絕
/// （2026-07-27 實測），生圖呼叫必須帶 --dangerously-skip-permissions 才會出圖。
/// 注意：agy 對單則訊息約 195KB 以上會靜默只留開頭（2026-10-06 實測，`-p` 與 stdin 相同），
/// 換傳遞方式解不掉，見 .ai/plans/long-prompt-scene-hint.md 發現 D。
pub fn agy_args(model: Option<&str>, allow_tools: bool) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(model) = model {
        args.push("--model".to_owned());
        args.push(model.to_owned());
    }
    if allow_tools {
        args.push("--dangerously-skip-permissions".to_owned());
    }
    // stream-json 才拿得到 usage（agy 1.1.8 起含 cache_read_tokens）；純 json 會把整包
    // 壓到最後一次吐出，串流就沒了。
    args.push("--output-format".to_owned());
    args.push("stream-json".to_owned());
    args
}

/// Agy 對話 lane：首輪帶穩定素材，後續用精確 conversation ID 只送新回合。
/// 不用 `--continue`：它是「這個 workspace 最近一條」，可能誤接生圖／重構對話。
pub fn agy_session_args(model: Option<&str>, conversation_id: Option<&str>) -> Vec<String> {
    let mut args = agy_args(model, false);
    if let Some(id) = conversation_id {
        args.push("--conversation".to_owned());
        args.push(id.to_owned());
    }
    args
}

/// Agy 對話 lane 的 stdin 正文：開線＝system＋本輪，續聊只送本輪（system 已在對話裡）。
pub fn agy_session_body(system: &str, prompt: &str, conversation_id: Option<&str>) -> String {
    match conversation_id {
        Some(_) => prompt.to_owned(),
        None => format!("{system}\n\n{prompt}"),
    }
}

/// grok 通道的環境隔離。grok 有「Claude Code 相容」設計：預設會載入 `$HOME/.claude` 下的
/// hooks、skills、agents、MCP、rules 與 CLAUDE.md。1.0.46 起有 `[compat.claude]` 的
/// agents／hooks／mcps／rules／skills 開關（另有 `GROK_CLAUDE_*_ENABLED`），但遠端 settings
/// 還有沒對應鍵的相容項（如 claude_sessions_enabled），開關不保證涵蓋全部。
/// 所以照舊把 HOME 指向 app 的空目錄（grok 就找不到 ~/.claude），GROK_HOME 指向 app 專用
/// 設定目錄——登入態與 session 也因此跟使用者終端機的 `~/.grok` 分開。
/// 四處呼叫 grok 的地方必須共用這組，否則會出現「UI 顯示已登入、實跑未登入」。
pub fn grok_envs(home: &Path, grok_home: &Path) -> Vec<(String, String)> {
    let home = home.to_string_lossy().into_owned();
    [
        ("HOME", home.clone()),
        // Windows 認 USERPROFILE，HOME 在那邊不作數
        ("USERPROFILE", home),
        ("GROK_HOME", grok_home.to_string_lossy().into_owned()),
        ("GROK_CONFIG", GROK_SAMPLING_OVERLAY.to_owned()),
        // 遠端會下發 campaign（例：grok-4.7-launch 把預設模型改成 4.7）。app 的 grok-home 沒人
        // dismiss 過，campaign 生效時只要 `-m` 不是它的預設模型，`--system-prompt-override`
        // 就被丟掉、換回 coding agent 提示（1.0.46 實測）。官方文件保證這個變數連
        // requirements 都壓得過（user-guide/26-config-reference.md `features.campaigns`）。
        ("GROK_CAMPAIGNS", "0".to_owned()),
    ]
    .map(|(key, value)| (key.to_owned(), value))
    .to_vec()
}

/// 取樣參數：grok 1.0.5 沒有 temperature／top_p 旗標，只吃設定檔的 `[models]` 全域預設。
/// `GROK_CONFIG` 是官方的 JSON 疊加層（deep merge 在 config.toml 之上，白名單含 `models`），
/// 走環境變數就不必動 GROK_HOME 裡的 config.toml，也碰不到登入態。
/// 1.1／1.0 是為了鬆開 grok 在小說／角色扮演時壓縮場景的傾向。
pub const GROK_SAMPLING_OVERLAY: &str = r#"{"models":{"temperature":1.1,"top_p":1.0}}"#;

/// 聊天單發要移除的內建工具全集（grok 1.0.5；1.0.46 新增 send_feedback）。`--deny *` 只擋執行，工具 schema 照樣佔
/// context——實測同一段開場 12604 → 3602 input tokens，CLI 內部 log 的 `tool_count` 由 24 歸 0。
///
/// 名稱有兩套：串流事件 `available_commands` 報的是顯示名（`run_terminal_command`），
/// 但過濾旗標認的是 README 的工具 ID（`run_terminal_cmd`）——只寫顯示名會靜默無效（實測
/// 那次 shell 沒被移除、tool_count 停在 1），所以 shell 兩個名字都列。
/// `--tools` 那條 allowlist 走不通：空字串等同沒設，只列一個工具也只會把清單換成
/// `search_tool`／`use_tool`（tool_count 2），並沒有如文件所說停用預設注入。
///
/// CLI 升版新增工具時這裡要補：漏網的工具會重新出現在 toolset，模型可能挑它去用，
/// 被 `--deny *` 擋下後那一輪就沒了（`--max-turns 1`），玩家看到的是一次空回應。
const GROK_CHAT_DISALLOWED_TOOLS: &str = "run_terminal_cmd,run_terminal_command,read_file,\
search_replace,list_dir,grep,kill_command_or_subagent,todo_write,\
get_command_or_subagent_output,spawn_subagent,scheduler_create,scheduler_delete,scheduler_list,\
monitor,search_tool,use_tool,workflow,enter_plan_mode,exit_plan_mode,ask_user_question,image_gen,\
image_edit,image_to_video,reference_to_video,write,Agent,send_feedback";

/// grok 單發的檔案內容：(agent profile, 正文)。
/// 文字通道：system 由 `promptMode: full` 的 agent profile 整包換掉 grok 自己那份 coding agent
/// system prompt（grok 內建那份偏簡潔精煉，留著會壓縮小說場景），本桌設定才真的坐在 system 層。
/// 生圖不換：那條要靠原生 agent prompt 把 image_gen 叫起來、收工具結果再回路徑，
/// 拔掉整份 system 有機會斷掉工具調度，維持「system 併進正文」的走法。
pub fn grok_payload(system: &str, prompt: &str, allow_tools: bool) -> (Option<String>, String) {
    if allow_tools || system.is_empty() {
        let body = match system.is_empty() {
            true => prompt.to_owned(),
            false => format!("{system}\n\n{prompt}"),
        };
        return (None, body);
    }
    (Some(grok_agent_profile(system)), prompt.to_owned())
}

/// system 改走 agent profile 檔（`--agent <路徑>`）：grok 的 `--system-prompt-override` 沒有
/// 檔案版，參數會撞命令列上限；而且全新 GROK_HOME（沒有 bundled/）下 override 會被忽略
/// （2026-10-06 實測），profile 兩種狀態都正確。
///
/// profile 的 body 會被 grok 的模板引擎渲染（`${{ … }}`、`${% … %}`）、頭尾空白會被修掉，
/// 所以整段包進 `${% raw %}…${% endraw %}`，內文的 `${%` 換成輸出同樣字樣的運算式——
/// raw 區塊裡唯一會被認的就是 `${% endraw %}`。`str::replace` 是單趟替換，插入的模板
/// 不會被再替換。逐 byte 結果以真 grok CLI 驗證（見 tests 的 #[ignore] 測試）。
pub fn grok_agent_profile(system: &str) -> String {
    let escaped = system.replace("${%", r#"${% endraw %}${{ "${%" }}${% raw %}"#);
    format!(
        "---\nname: table-tavern\ndescription: Table Tavern\npromptMode: full\n---\n${{% raw %}}{escaped}${{% endraw %}}"
    )
}

/// 聊天單發一律關閉工具、網路搜尋、計畫與子代理，避免 CLI 執行本機命令。
/// allow_tools：生圖呼叫要用 grok 原生 image_gen 工具，--deny * 換成 --always-approve。
/// 正文走 `--prompt-file`。文字通道加 `--verbatim`：不然正文超過約 100KB 時 grok 會把全文
/// 搬去 session 目錄、訊息只留節錄叫模型用 read_file 讀，而聊天把工具全拆了，模型只看得到
/// 節錄（2026-10-06 實測）。生圖那條正文短、又要原生 agent 行為，不加。
pub fn grok_args(
    model: Option<&str>,
    profile: Option<&Path>,
    prompt_file: &Path,
    allow_tools: bool,
) -> Vec<String> {
    let mut args = grok_common_args(model, allow_tools);
    if let Some(profile) = profile {
        args.push("--agent".to_owned());
        args.push(profile.to_string_lossy().into_owned());
    }
    push_grok_prompt(&mut args, prompt_file, !allow_tools);
    args
}

/// grok lane 續聊參數（grok-cache-miss）：與聊天單發同一組旗標，差別只在讓 session 落檔。
/// 開線 `-s <id>` 自帶 system profile；續聊 `-r <id>` **不重帶 system**——grok 把 system
/// 凍在 session 建立那一刻（session 目錄下的 system_prompt.txt），重帶無效且會打散前綴，
/// 素材漂移一律改走 prompt 內的補丁（見 lanes::plan_turn）。
/// `-s` 對已存在的 id 會直接報「Session ID is already in use」，所以開／續兩條旗標不能互換。
pub fn grok_session_args(
    model: Option<&str>,
    profile: Option<&Path>,
    prompt_file: &Path,
    session: &CliSession<'_>,
) -> Vec<String> {
    let mut args = grok_common_args(model, false);
    match session {
        CliSession::Open(id) => {
            args.push("-s".to_owned());
            args.push((*id).to_owned());
            if let Some(profile) = profile {
                args.push("--agent".to_owned());
                args.push(profile.to_string_lossy().into_owned());
            }
        }
        CliSession::Resume(id) => {
            args.push("-r".to_owned());
            args.push((*id).to_owned());
        }
    }
    push_grok_prompt(&mut args, prompt_file, true);
    args
}

fn push_grok_prompt(args: &mut Vec<String>, prompt_file: &Path, verbatim: bool) {
    if verbatim {
        args.push("--verbatim".to_owned());
    }
    args.push("--prompt-file".to_owned());
    args.push(prompt_file.to_string_lossy().into_owned());
}

/// grok 聊天／續聊共用的旗標段（不含 system、session 與正文）。
fn grok_common_args(model: Option<&str>, allow_tools: bool) -> Vec<String> {
    let mut args: Vec<String> = ["--output-format", "streaming-json"]
        .map(str::to_owned)
        .to_vec();
    if allow_tools {
        args.push("--always-approve".to_owned());
    } else {
        args.push("--deny".to_owned());
        args.push("*".to_owned());
    }
    args.extend(["--disable-web-search", "--no-plan", "--no-subagents"].map(str::to_owned));
    if !allow_tools {
        // 工具定義整包拆掉（--deny 只擋執行不擋注入）。生圖那條當然不能設：它要用 image_gen。
        // 實測這條讓 CLI 的 tool_count 歸 0、input 從 12604 掉到 3602。
        args.push("--disallowed-tools".to_owned());
        args.push(GROK_CHAT_DISALLOWED_TOOLS.to_owned());
        // 旁白是無工具單發，封死模型自己多跑幾輪的出血口（hook 擋停那次就是這樣燒額度）。
        // 生圖不能設：那條要「呼叫 image_gen → 工具回傳 → 再回一句路徑」，砍到一輪會斷在中間。
        args.push("--max-turns".to_owned());
        args.push("1".to_owned());
        // 少推理、多寫正文。grok-4.6 的 effort 選單只有 xhigh/high/medium/low，
        // 傳 none 會被 CLI 當未知等級直接中止（實測 1.0.5），所以最低就到 low；
        // 模型若整個不支援 effort，CLI 只會忽略不會失敗。
        // 生圖那條不設：它要靠推理把 image_gen 叫起來再回報路徑，不動既有行為。
        args.push("--reasoning-effort".to_owned());
        args.push("low".to_owned());
    }
    if let Some(model) = model {
        args.push("-m".to_owned());
        args.push(model.to_owned());
    }
    args
}

#[cfg(test)]
mod tests;
