use super::types::CliSession;
use crate::data::Tier;
use crate::transport::ChatMessage;
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
                format!("{assistant_label}：{}", message.content)
            } else {
                message.content.clone()
            }
        })
        .collect();
    let history = history.join("\n\n");
    let prompt = match closing.is_empty() {
        true => format!("以下是到目前為止的對話紀錄：\n\n{history}"),
        false => format!("以下是到目前為止的對話紀錄：\n\n{history}\n\n——\n{closing}"),
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

/// claude lane 續聊參數：與 claude_args 同組旗標，但保留 session 落檔
/// （resume 架構的快取命中靠 CLI 自身 session，非 §8.1 無狀態單發）。
pub fn claude_session_args(model: &str, system: &str, session: &CliSession<'_>) -> Vec<String> {
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
    // stream-json 才拿得到 usage（agy 1.1.8 起含 cache_read_tokens）；純 json 會把整包
    // 壓到最後一次吐出，串流就沒了。
    args.push("--output-format".to_owned());
    args.push("stream-json".to_owned());
    args.push("-p".to_owned());
    args.push(prompt.to_owned());
    args
}

/// grok 通道的環境隔離。grok 有「Claude Code 相容」設計：會自動載入 `$HOME/.claude` 下的
/// hooks、skills、plugins、CLAUDE.md 與 permissions，官方沒有可關的旗標或設定
/// （`[features] claude_hooks` 是伺服器端 flag、`CLAUDE_CONFIG_DIR` 無效，皆實測過）。
/// 玩家的 coding hook 因此會擋停旁白、CLAUDE.md 會混進 GM 的系統提示。
/// 唯一槓桿是環境變數：HOME 指向 app 的空目錄（grok 就找不到 ~/.claude），
/// GROK_HOME 指向 app 專用設定目錄（登入態與 session 都留在 app 自己這套）。
/// 四處呼叫 grok 的地方必須共用這組，否則會出現「UI 顯示已登入、實跑未登入」。
pub fn grok_envs(home: &Path, grok_home: &Path) -> Vec<(String, String)> {
    let home = home.to_string_lossy().into_owned();
    [
        ("HOME", home.clone()),
        // Windows 認 USERPROFILE，HOME 在那邊不作數
        ("USERPROFILE", home),
        ("GROK_HOME", grok_home.to_string_lossy().into_owned()),
        ("GROK_CONFIG", GROK_SAMPLING_OVERLAY.to_owned()),
    ]
    .map(|(key, value)| (key.to_owned(), value))
    .to_vec()
}

/// 取樣參數：grok 1.0.5 沒有 temperature／top_p 旗標，只吃設定檔的 `[models]` 全域預設。
/// `GROK_CONFIG` 是官方的 JSON 疊加層（deep merge 在 config.toml 之上，白名單含 `models`），
/// 走環境變數就不必動 GROK_HOME 裡的 config.toml，也碰不到登入態。
/// 1.1／1.0 是為了鬆開 grok 在小說／角色扮演時壓縮場景的傾向。
pub const GROK_SAMPLING_OVERLAY: &str = r#"{"models":{"temperature":1.1,"top_p":1.0}}"#;

/// 聊天單發要移除的內建工具全集（grok 1.0.5）。`--deny *` 只擋執行，工具 schema 照樣佔
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
image_edit,image_to_video,reference_to_video,write,Agent";

/// 聊天單發一律關閉工具、網路搜尋、計畫與子代理，避免 CLI 執行本機命令。
/// allow_tools：生圖呼叫要用 grok 原生 image_gen 工具，--deny * 換成 --always-approve。
pub fn grok_args(
    model: Option<&str>,
    system: &str,
    prompt: &str,
    allow_tools: bool,
) -> Vec<String> {
    let mut args = grok_common_args(model, allow_tools);
    // --system-prompt-override（grok 1.0.5）整包換掉 CLI 自己那份 coding agent system
    // prompt，本桌的設定才真的坐在 system 層；grok 內建那份偏簡潔精煉，留著會壓縮小說場景。
    // 生圖不換：那條要靠原生 agent prompt 把 image_gen 叫起來、收工具結果再回路徑，
    // 拔掉整份 system 有機會斷掉工具調度，維持原本「system 併進 prompt」的走法。
    let override_system = !allow_tools && !system.is_empty();
    if override_system {
        args.push("--system-prompt-override".to_owned());
        args.push(system.to_owned());
    }
    args.push("-p".to_owned());
    args.push(match override_system || system.is_empty() {
        true => prompt.to_owned(),
        false => format!("{system}\n\n{prompt}"),
    });
    args
}

/// grok lane 續聊參數（grok-cache-miss）：與聊天單發同一組旗標，差別只在讓 session 落檔。
/// 開線 `-s <id>` 自帶 system override；續聊 `-r <id>` **不重帶 system**——grok 把 system
/// 凍在 session 建立那一刻（session 目錄下的 system_prompt.txt），重帶無效且會打散前綴，
/// 素材漂移一律改走 prompt 內的補丁（見 lanes::plan_turn）。
/// `-s` 對已存在的 id 會直接報「Session ID is already in use」，所以開／續兩條旗標不能互換。
pub fn grok_session_args(
    model: Option<&str>,
    system: &str,
    prompt: &str,
    session: &CliSession<'_>,
) -> Vec<String> {
    let mut args = grok_common_args(model, false);
    match session {
        CliSession::Open(id) => {
            args.push("-s".to_owned());
            args.push((*id).to_owned());
            if !system.is_empty() {
                args.push("--system-prompt-override".to_owned());
                args.push(system.to_owned());
            }
        }
        CliSession::Resume(id) => {
            args.push("-r".to_owned());
            args.push((*id).to_owned());
        }
    }
    args.push("-p".to_owned());
    args.push(prompt.to_owned());
    args
}

/// grok 聊天／續聊共用的旗標段（不含 system、session 與 -p）。
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
    args.extend(
        ["--disable-web-search", "--no-plan", "--no-subagents"].map(str::to_owned),
    );
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
mod tests {
    use super::*;
    use std::path::PathBuf;

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

    /// agy 1.1.8 以下不認 --output-format，要在打過去之前擋下；版本字串認不得就放行，
    /// 讓呼叫失敗時帶著 CLI 自己的錯誤訊息，而不是因為格式換了就把整條路擋死。
    #[test]
    fn agy_stream_json_support_gates_on_1_1_8() {
        assert!(!agy_supports_stream_json("1.1.7"));
        assert!(!agy_supports_stream_json("1.0.99"));
        assert!(!agy_supports_stream_json("0.9.9"));
        assert!(agy_supports_stream_json("1.1.8"));
        assert!(agy_supports_stream_json("1.1.17"));
        assert!(agy_supports_stream_json("2.0.0"));
        assert!(agy_supports_stream_json("agy version 1.1.17 (darwin)"));
        assert!(agy_supports_stream_json("")); // 認不得就放行
        assert!(agy_supports_stream_json("nightly"));
    }

    /// 共線後 messages 已自足：label 傳空就不再補名字前綴（否則「加爾：雷恩：……」），
    /// closing 傳空就不再接收尾指示（否則本輪指定會出現兩次）。
    #[test]
    fn flatten_skips_label_and_closing_when_self_contained() {
        let messages = vec![
            ChatMessage {
                role: "system".to_owned(),
                content: "共用 system".to_owned(),
            },
            ChatMessage {
                role: "assistant".to_owned(),
                content: "加爾：抬起頭。".to_owned(),
            },
            ChatMessage {
                role: "user".to_owned(),
                content: "現在你是「雷恩」。".to_owned(),
            },
        ];
        let (system, prompt) = flatten_messages("", "", &messages);
        assert_eq!(system, "共用 system");
        assert_eq!(
            prompt,
            "以下是到目前為止的對話紀錄：\n\n加爾：抬起頭。\n\n現在你是「雷恩」。"
        );
        assert!(!prompt.contains("——")); // closing 為空就不留分隔線
        // 舊行為不變：有 label 就補前綴、有 closing 就接在後面
        let (_, legacy) = flatten_messages("雷恩", "收尾指示", &messages);
        assert!(legacy.contains("雷恩：加爾：抬起頭。"));
        assert!(legacy.ends_with("——\n收尾指示"));
    }

    #[test]
    fn agy_args_put_prompt_in_final_p_value_with_optional_model() {
        let prompt = "system\n\n整包 prompt（含空格）";
        assert_eq!(
            agy_args(Some("Claude Sonnet 4.6 (Thinking)"), prompt, false),
            [
                "--model",
                "Claude Sonnet 4.6 (Thinking)",
                "--output-format",
                "stream-json",
                "-p",
                prompt
            ]
        );
        assert_eq!(
            agy_args(None, prompt, false),
            ["--output-format", "stream-json", "-p", prompt]
        );
    }

    #[test]
    fn grok_args_disable_every_tool_and_put_prompt_last() {
        let system = "本桌 system（含空格）";
        let prompt = "整包 prompt（含空格）";
        let args = grok_args(Some("grok-4.5"), system, prompt, false);
        assert!(args
            .windows(2)
            .any(|pair| pair == ["--output-format", "streaming-json"]));
        assert!(args.windows(2).any(|pair| pair == ["--deny", "*"]));
        assert!(args.contains(&"--disable-web-search".to_owned()));
        assert!(args.contains(&"--no-plan".to_owned()));
        assert!(args.contains(&"--no-subagents".to_owned()));
        assert!(args.windows(2).any(|pair| pair == ["--max-turns", "1"]));
        // low 是 grok-4.6／4.5 選單的最低檔；none 會被 CLI 判成未知等級、整次生成中止
        assert!(args
            .windows(2)
            .any(|pair| pair == ["--reasoning-effort", "low"]));
        // 工具定義只在聊天那條拆：清單裡含 image_gen，生圖若跟著設就沒圖可生
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--disallowed-tools" && pair[1].contains("image_gen")));
        // 生圖要跑「呼叫工具→拿結果→回一句」，帶 max-turns 1 會斷在工具回傳那步；
        // 推理等級也維持原樣，免得把叫工具那步壓掉
        let image_args = grok_args(Some("grok-4.5"), system, prompt, true);
        assert!(!image_args.contains(&"--disallowed-tools".to_owned()));
        assert!(!image_args.contains(&"--max-turns".to_owned()));
        assert!(!image_args.contains(&"--reasoning-effort".to_owned()));
        // 生圖保留 grok 原生 agent system prompt（工具調度靠它），system 照舊併進 prompt
        assert!(!image_args.contains(&"--system-prompt-override".to_owned()));
        assert_eq!(
            image_args[image_args.len() - 2..],
            ["-p".to_owned(), format!("{system}\n\n{prompt}")]
        );
        assert!(args.windows(2).any(|pair| pair == ["-m", "grok-4.5"]));
        // 文字通道：system 自己坐 system 層，-p 只剩真正的 user prompt
        assert!(args
            .windows(2)
            .any(|pair| pair == ["--system-prompt-override", system]));
        assert_eq!(args[args.len() - 2..], ["-p", prompt]);
        let default_args = grok_args(None, system, prompt, false);
        assert_eq!(default_args[default_args.len() - 2..], ["-p", prompt]);
        // 清單是逗號串，不能混進換行或空白（續行寫壞的話 CLI 會把整包當一個工具名）
        let list = args
            .windows(2)
            .find(|pair| pair[0] == "--disallowed-tools")
            .map(|pair| pair[1].clone())
            .expect("聊天單發要帶 --disallowed-tools");
        assert!(!list.contains(char::is_whitespace));
        assert_eq!(list.split(',').count(), 26);
        // shell 的顯示名與過濾 ID 不同名，只寫顯示名會靜默無效——兩個都要在
        assert!(list.contains("run_terminal_cmd,"));
        assert!(list.contains("run_terminal_command,"));
        assert!(list.ends_with(",Agent"));
        // system 是空的（訊息串空）就不帶旗標，也不要在 prompt 前面留兩個空行
        let empty = grok_args(None, "", prompt, false);
        assert!(!empty.contains(&"--system-prompt-override".to_owned()));
        assert_eq!(empty[empty.len() - 2..], ["-p", prompt]);
    }

    #[test]
    fn grok_session_args_open_carries_system_and_resume_does_not() {
        let system = "你是狐狸";
        let open = grok_session_args(
            Some("grok-4.6"),
            system,
            "第一輪",
            &CliSession::Open("sid-1"),
        );
        // 開線：-s 建 session，system 這時才坐進去（grok 把它凍在 session 建立那刻）
        assert!(open.windows(2).any(|pair| pair == ["-s", "sid-1"]));
        assert!(open
            .windows(2)
            .any(|pair| pair == ["--system-prompt-override", system]));
        assert_eq!(open[open.len() - 2..], ["-p", "第一輪"]);
        // 聊天單發那組硬化旗標一個都不能少（工具全拆、單輪、低推理）
        assert!(open.windows(2).any(|pair| pair == ["--max-turns", "1"]));
        assert!(open
            .windows(2)
            .any(|pair| pair == ["--reasoning-effort", "low"]));
        assert!(open
            .windows(2)
            .any(|pair| pair[0] == "--disallowed-tools" && pair[1].contains("image_gen")));
        assert!(open.windows(2).any(|pair| pair == ["--deny", "*"]));
        assert!(open.windows(2).any(|pair| pair == ["-m", "grok-4.6"]));

        // 續聊：-r 接同一條線，system 不重帶（重帶無效又會打散前綴），增量進 -p
        let resume = grok_session_args(
            Some("grok-4.6"),
            system,
            "增量",
            &CliSession::Resume("sid-1"),
        );
        assert!(resume.windows(2).any(|pair| pair == ["-r", "sid-1"]));
        assert!(!resume.contains(&"--system-prompt-override".to_owned()));
        assert!(!resume.contains(&"-s".to_owned()));
        assert_eq!(resume[resume.len() - 2..], ["-p", "增量"]);

        // 未覆寫模型＝不帶 -m，由 CLI 自己選預設
        let default_model = grok_session_args(None, system, "x", &CliSession::Open("sid-2"));
        assert!(!default_model.contains(&"-m".to_owned()));
    }

    #[test]
    fn grok_envs_point_home_and_grok_home_at_the_app_profile() {
        let envs = grok_envs(
            &PathBuf::from("/app/cli-home"),
            &PathBuf::from("/app/grok-home"),
        );
        // HOME 換掉才擋得住 ~/.claude 的 hooks／CLAUDE.md；Windows 認的是 USERPROFILE
        assert!(envs.contains(&("HOME".to_owned(), "/app/cli-home".to_owned())));
        assert!(envs.contains(&("USERPROFILE".to_owned(), "/app/cli-home".to_owned())));
        // GROK_HOME 另指一處，登入態才不會跟使用者終端機的 ~/.grok 混在一起
        assert!(envs.contains(&("GROK_HOME".to_owned(), "/app/grok-home".to_owned())));
        // 取樣參數走 GROK_CONFIG 疊加層：只在 app 這幾次呼叫生效，不寫進任何 config.toml
        assert!(envs.contains(&(
            "GROK_CONFIG".to_owned(),
            r#"{"models":{"temperature":1.1,"top_p":1.0}}"#.to_owned()
        )));
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
        let opened = claude_session_args("sonnet", "系統", &CliSession::Open("uuid-1"));
        assert!(!opened.contains(&"--no-session-persistence".to_owned()));
        assert!(opened.windows(2).any(|w| w == ["--session-id", "uuid-1"]));
        assert!(opened.windows(2).any(|w| w == ["--system-prompt", "系統"]));
        assert!(opened.windows(2).any(|w| w == ["--model", "sonnet"]));
        let resumed = claude_session_args("sonnet", "系統", &CliSession::Resume("uuid-1"));
        assert!(resumed.windows(2).any(|w| w == ["--resume", "uuid-1"]));
        assert!(!resumed.contains(&"--session-id".to_owned()));
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

}
