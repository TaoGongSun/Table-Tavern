use crate::{cli, config_root, data, data_root, lanes, transport, usage_log};
use std::path::PathBuf;

/// CLI 的工作目錄。Finder 啟動的 macOS app 工作目錄可能是根目錄；CLI 若繼承後做專案探索，
/// 會掃到桌面、下載項目等受 TCC 保護的位置。固定在專用空目錄，避免無關權限彈窗。
pub(crate) fn cli_workspace(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let workspace = config_root(app)?.join("cli-workspace");
    std::fs::create_dir_all(&workspace)
        .map_err(|error| format!("無法準備 CLI 工作目錄：{error}"))?;
    Ok(workspace)
}

/// grok 專用 profile：回傳 (假 HOME, GROK_HOME)。grok 會自動吃 `$HOME/.claude` 下的
/// hooks／skills／CLAUDE.md（官方無 opt-out），玩家的 coding hook 因此擋停過旁白。
/// 這兩個目錄讓 grok 只看得到 app 自己這套，登入態也存在這裡，與使用者終端機的
/// `~/.grok` 互不干擾（2026-08-21 拍板：不共用，避免 CLI 與 app 狀態混淆）。
fn grok_profile(app: &tauri::AppHandle) -> Result<(PathBuf, PathBuf), String> {
    let root = config_root(app)?;
    let home = root.join("cli-home");
    let grok_home = root.join("grok-home");
    for path in [&home, &grok_home] {
        std::fs::create_dir_all(path)
            .map_err(|error| format!("無法準備 grok 設定目錄：{error}"))?;
        // 憑證存在這裡，收成 0700 不讓同機其他使用者讀
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700));
        }
    }
    Ok((home, grok_home))
}

/// 四處呼叫 grok 的地方共用這組環境；其餘 CLI 不需要隔離，回空陣列。
pub(crate) fn cli_envs(
    app: &tauri::AppHandle,
    provider: &str,
) -> Result<Vec<(String, String)>, String> {
    if provider != "grok" {
        return Ok(Vec::new());
    }
    let (home, grok_home) = grok_profile(app)?;
    Ok(cli::grok_envs(&home, &grok_home))
}

/// 使用者選定的聊天傳輸層（preferences.transport，預設 api）。
pub(crate) fn chat_transport(config: &data::AppConfig) -> String {
    config
        .preferences
        .get("transport")
        .and_then(|value| value.as_str())
        .unwrap_or("api")
        .to_owned()
}

/// claude CLI 的環境變數：沙盒告知＋（設了相容端點時）BASE_URL 與 token。
/// 單發（stream_via_transport）與 lane 續聊共用。
fn claude_cli_envs(config: &data::AppConfig) -> Vec<(String, String)> {
    // Claude Code 開場會自建 macOS 沙盒，系統因此以「Table Tavern」的名義向玩家要
    // 桌面／音樂資料夾權限（tccd 日誌實證：accessing=claude-code、responsible=本 app）。
    // 這個變數告訴它「你已經在沙盒裡」，想省掉那組彈窗；實測仍會被要求媒體資料庫權限，
    // 效果未定但無害（我們給的是 --tools ""，它本來就不需要那些資料夾）。
    // 彈窗文案改由 Info.plist 的 NSAppleMusicUsageDescription 等鍵說明。
    let mut envs = vec![("CLAUDE_CODE_SANDBOXED".to_owned(), "1".to_owned())];
    let base_url = config
        .preferences
        .get("claude_base_url")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .unwrap_or("");
    if !base_url.is_empty() {
        envs.push(("ANTHROPIC_BASE_URL".to_owned(), base_url.to_owned()));
        if let Some(api_key) = config
            .api_keys
            .get("claude_compat")
            .map(String::as_str)
            .map(str::trim)
            .filter(|key| !key.is_empty())
        {
            envs.push(("ANTHROPIC_AUTH_TOKEN".to_owned(), api_key.to_owned()));
        }
    }
    envs
}

/// claude CLI 的 session 檔目錄（resume 續聊的抹寫要直接讀寫它）。
fn claude_home_dir() -> PathBuf {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".claude")))
        .or_else(|| std::env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join(".claude")))
        .unwrap_or_else(|| PathBuf::from(".claude"))
}

/// 這個傳輸走不走 lane 續聊。api／codex／agy 沒有可續聊的 CLI session，回 None 照走單發。
pub(crate) fn lane_provider(config: &data::AppConfig) -> Option<lanes::LaneProvider> {
    match chat_transport(config).as_str() {
        "claude" => Some(lanes::LaneProvider::Claude),
        "grok" => Some(lanes::LaneProvider::Grok),
        _ => None,
    }
}

/// 準備 lane 呼叫素材：風險告知檢查＋CLI 偵測＋模型解析＋env。
/// lane 續聊（角色聊天／GM 旁白）專用；其餘呼叫照走 stream_via_transport 單發。
/// claude 的模型解析有檔位預設值，grok 沒有——未覆寫就回 None、由 CLI 用自己的預設。
pub(crate) async fn prepare_lane_call(
    app: &tauri::AppHandle,
    config: &data::AppConfig,
    tier: data::Tier,
    provider: lanes::LaneProvider,
) -> Result<lanes::LaneCall, String> {
    if config.preferences.get("cli_risk_accepted") != Some(&serde_json::Value::Bool(true)) {
        return Err("尚未確認 CLI 訂閱模式的風險告知，請到設定完成確認".to_owned());
    }
    let id = match provider {
        lanes::LaneProvider::Claude => "claude",
        lanes::LaneProvider::Grok => "grok",
    };
    let info = cli::detect_clis()
        .await
        .into_iter()
        .find(|info| info.id == id)
        .ok_or_else(|| format!("找不到 {id} CLI，請確認已安裝並登入"))?;
    let override_model = cli::tier_override(&config.tier_models, id, tier);
    let model = match provider {
        lanes::LaneProvider::Claude => Some(
            override_model
                .unwrap_or_else(|| cli::claude_model_for(tier))
                .to_owned(),
        ),
        lanes::LaneProvider::Grok => override_model.map(str::to_owned),
    };
    Ok(lanes::LaneCall {
        provider,
        program: PathBuf::from(info.path),
        working_dir: cli_workspace(app)?,
        envs: match provider {
            lanes::LaneProvider::Claude => claude_cli_envs(config),
            lanes::LaneProvider::Grok => cli_envs(app, "grok")?,
        },
        model,
        usage_log: data_root(app)
            .ok()
            .map(|root| root.join("prompt-cache.jsonl")),
        claude_home: claude_home_dir(),
    })
}

/// 「真的把話送出去給模型、而它沒能回話」才掛這個碼——前端據此給一句保底人話。
/// 沒掛碼的失敗（讀設定、找不到 CLI、風險告知沒確認、寫檔）本身就是精確可行動的訊息，
/// 套上「AI 沒回應」只會蓋掉重點、害玩家跑去換模型瞎試。
///
/// 已有更精確的碼就原樣放行；只認這份白名單，不用 `AI_` 開頭一概放行——
/// 錯誤字串可能整包來自供應商，讓它自帶前綴就能繞過分流。
pub(crate) fn ai_call_failure(error: String) -> String {
    const CODED: [&str; 4] = [
        "AI_HTTP_STATUS_",
        "AI_EMPTY_RESPONSE:",
        "AI_INCOMPLETE_RESPONSE:",
        "AI_CONTENT_FILTERED:",
    ];
    if CODED.iter().any(|code| error.starts_with(code)) {
        error
    } else {
        format!("AI_CALL_FAILED: {error}")
    }
}

/// 依 preferences.transport 把組裝好的訊息分流到 API 或 CLI，增量經 emit 回呼。
/// assistant_label／cli_closing 供 CLI 攤平使用：角色對話與 GM 導演共用同一條路。
#[allow(clippy::too_many_arguments)]
/// 不建立續輪期待的一次性呼叫（開桌生成、卡重構、換幕摘要、翻譯）走這條；
/// 劇情輪要讓帳本看得出 prompt 形狀，改走 `stream_turn_via_transport`。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn stream_via_transport(
    app: &tauri::AppHandle,
    config: &data::AppConfig,
    transport_override: Option<&str>,
    allow_cli_tools: bool,
    tier: data::Tier,
    world: Option<&str>,
    assistant_label: &str,
    cli_closing: &str,
    messages: &[transport::ChatMessage],
    thinking_to_delta: bool,
    emit: impl FnMut(&str),
) -> Result<String, String> {
    stream_turn_via_transport(
        app,
        config,
        transport_override,
        allow_cli_tools,
        tier,
        world,
        assistant_label,
        cli_closing,
        messages,
        usage_log::PromptShape::Oneshot,
        thinking_to_delta,
        emit,
    )
    .await
}

/// `shape` 是交給帳本的唯讀情報（這通送出去的是什麼形狀），不影響組裝與傳輸。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn stream_turn_via_transport(
    app: &tauri::AppHandle,
    config: &data::AppConfig,
    transport_override: Option<&str>,
    allow_cli_tools: bool,
    tier: data::Tier,
    world: Option<&str>,
    assistant_label: &str,
    cli_closing: &str,
    messages: &[transport::ChatMessage],
    shape: usage_log::PromptShape,
    // 思考增量進不進 emit：只有卡重構的進度字尾開 true（emit＝正文串流的呼叫端一律 false）。
    thinking_to_delta: bool,
    emit: impl FnMut(&str),
) -> Result<String, String> {
    // transport_override：生圖等功能可指定與聊天不同的連線（None＝跟隨 preferences.transport）。
    // allow_cli_tools：只有生圖呼叫為 true——CLI 生圖工具要寫檔／跑指令，聊天一律鎖死工具。
    let transport_kind = transport_override
        .map(str::to_owned)
        .unwrap_or_else(|| chat_transport(config));
    // 每次呼叫的用量落成一行 JSONL（資料目錄的 prompt-cache.jsonl），供額度分頁讀。
    // API 與 CLI 兩條路共用同一份檔案，靠行內的 transport 欄位分辨。
    let usage_log = data_root(app)
        .ok()
        .map(|root| root.join("prompt-cache.jsonl"));
    if transport_kind == "api" {
        let model = transport::resolve_model(tier, config)?;
        return transport::stream_chat(
            config,
            &model,
            messages,
            usage_log.as_deref(),
            world,
            shape,
            emit,
        )
        .await
        .map_err(|error| ai_call_failure(error.to_string()));
    }

    // CLI 訂閱模式：風險告知未確認前後端直接擋（NewPlan §4.2）
    if config.preferences.get("cli_risk_accepted") != Some(&serde_json::Value::Bool(true)) {
        return Err("尚未確認 CLI 訂閱模式的風險告知，請到設定完成確認".to_owned());
    }
    let info = cli::detect_clis()
        .await
        .into_iter()
        .find(|info| info.id == transport_kind)
        .ok_or_else(|| format!("找不到 {transport_kind} CLI，請確認已安裝並登入"))?;
    if transport_kind == "agy" && !cli::agy_supports_stream_json(&info.version) {
        return Err(format!(
            "Gemini CLI {} 太舊：本 app 需要 1.1.8 以上（要用 --output-format stream-json 才拿得到用量）。請執行 `agy update` 後重新驗證。",
            info.version
        ));
    }
    let cli_working_dir = cli_workspace(app)?;

    let (system, prompt) = cli::flatten_messages(assistant_label, cli_closing, messages);
    let program = std::path::PathBuf::from(&info.path);
    match transport_kind.as_str() {
        "claude" => {
            let model = cli::tier_override(&config.tier_models, "claude", tier)
                .unwrap_or_else(|| cli::claude_model_for(tier));
            let args = cli::claude_args(model, &system);
            let envs = claude_cli_envs(config);
            cli::run_cli(
                &program,
                &cli_working_dir,
                &args,
                &prompt,
                &envs,
                cli::parse_claude_line,
                thinking_to_delta,
                usage_log.as_deref().map(|path| cli::UsageLog {
                    path,
                    world,
                    transport: "claude",
                    model,
                    parse: cli::parse_claude_usage,
                    lane: None,
                    shape,
                    prompt_tokens_out: None,
                }),
                emit,
            )
            .await
        }
        "codex" => {
            // codex 沒有 system prompt 旗標，併進 prompt 開頭；未覆寫時模型用 CLI 預設
            let model = cli::tier_override(&config.tier_models, "codex", tier);
            let args = cli::codex_args(model, cli::codex_effort_for(tier), allow_cli_tools);
            let combined = format!("{system}\n\n{prompt}");
            cli::run_cli(
                &program,
                &cli_working_dir,
                &args,
                &combined,
                &[],
                cli::parse_codex_line,
                false, // 只有 claude 解析器會產思考增量
                usage_log.as_deref().map(|path| cli::UsageLog {
                    path,
                    world,
                    transport: "codex",
                    model: model.unwrap_or("(CLI 預設)"),
                    parse: cli::parse_codex_usage,
                    lane: None,
                    shape,
                    prompt_tokens_out: None,
                }),
                emit,
            )
            .await
        }
        "agy" => {
            // agy 沒有 system prompt 旗標，併進 prompt 開頭；未覆寫時使用 CLI 預設模型。
            // 走 stream-json（agy_args 帶旗標）才拿得到含 cache_read_tokens 的用量。
            let model = cli::tier_override(&config.tier_models, "agy", tier);
            let combined = format!("{system}\n\n{prompt}");
            let args = cli::agy_args(model, &combined, allow_cli_tools);
            cli::run_cli(
                &program,
                &cli_working_dir,
                &args,
                "",
                &[],
                cli::parse_agy_line,
                false,
                usage_log.as_deref().map(|path| cli::UsageLog {
                    path,
                    world,
                    transport: "agy",
                    model: model.unwrap_or("(CLI 預設)"),
                    parse: cli::parse_agy_usage,
                    lane: None,
                    shape,
                    prompt_tokens_out: None,
                }),
                emit,
            )
            .await
        }
        "grok" => {
            // system 走 --system-prompt-override 頂掉 grok 內建那份（生圖那條例外，見
            // grok_args）；未覆寫時使用 CLI 預設模型
            let model = cli::tier_override(&config.tier_models, "grok", tier);
            let args = cli::grok_args(model, &system, &prompt, allow_cli_tools);
            let envs = cli_envs(app, "grok")?;
            cli::run_cli(
                &program,
                &cli_working_dir,
                &args,
                "",
                &envs,
                cli::parse_grok_line,
                false,
                usage_log.as_deref().map(|path| cli::UsageLog {
                    path,
                    world,
                    transport: "grok",
                    model: model.unwrap_or("(CLI 預設)"),
                    parse: cli::parse_grok_usage,
                    lane: None,
                    shape,
                    prompt_tokens_out: None,
                }),
                emit,
            )
            .await
        }
        other => Err(format!("未知傳輸層：{other}").into()),
    }
    .map_err(|error| ai_call_failure(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::ai_call_failure;

    /// 只有真的送出去給模型、而它沒回話的失敗才掛碼：讀卡、寫逐字稿、找不到 CLI
    /// 都不經過這裡，前端才不會把「檔案寫不進去」講成「AI 沒回應」害玩家換模型瞎試
    #[test]
    fn ai_call_failure_marks_only_uncoded_errors() {
        // 沒碼的一律掛上（連不上、串流中途斷掉）
        assert_eq!(
            ai_call_failure("error sending request".to_owned()),
            "AI_CALL_FAILED: error sending request"
        );
        // 已經分得更細的碼原樣放行，不可被籠統的一句蓋掉
        for coded in [
            "AI_HTTP_STATUS_503: API 回應 503",
            "AI_EMPTY_RESPONSE: 空白回合",
            "AI_INCOMPLETE_RESPONSE: 被截斷",
            "AI_CONTENT_FILTERED: 被擋",
        ] {
            assert_eq!(ai_call_failure(coded.to_owned()), coded);
        }
        // 白名單以外的 AI_ 開頭不算碼：錯誤字串可能整包來自供應商，
        // 讓它自帶前綴就能繞過分流
        assert_eq!(
            ai_call_failure("AI_SOMETHING_ELSE: 供應商自己寫的".to_owned()),
            "AI_CALL_FAILED: AI_SOMETHING_ELSE: 供應商自己寫的"
        );
    }
}
