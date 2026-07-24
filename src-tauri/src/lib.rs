mod cli;
mod data;
mod import;
mod transport;

use data::{AppConfig, CharacterCard, CharacterMeta, TranscriptEvent, WorldState};
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Command;
use tauri::Manager;

fn data_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .document_dir()
        .map(|path| path.join("TableTavern"))
        .map_err(|error| error.to_string())
}

fn config_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .config_dir()
        .map(|path| path.join("TableTavern"))
        .map_err(|error| error.to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallMessages {
    start: String,
    login_hint: String,
    success: String,
    fail: String,
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(any(target_os = "windows", test))]
fn powershell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn cli_install_script(provider: &str, messages: &InstallMessages) -> Result<String, String> {
    let start = shell_quote(&messages.start);
    let login_hint = shell_quote(&messages.login_hint);
    let success = shell_quote(&messages.success);
    let fail = shell_quote(&messages.fail);
    let (install_command, login_command, probe_command, poll_seconds) = match provider {
        "claude" => (
            "curl -fsSL https://claude.ai/install.sh | bash",
            Some("claude auth login"),
            "claude -p \"ok\"",
            120,
        ),
        "codex" => (
            "curl -fsSL https://chatgpt.com/codex/install.sh | sh",
            Some("codex login"),
            // codex exec 在非 git 目錄會拒跑，probe 改用即時且不耗額度的 login status
            "codex login status",
            120,
        ),
        "agy" => (
            "curl -fsSL https://antigravity.google/cli/install.sh | bash",
            None,
            "agy -p \"ok\"",
            600,
        ),
        "grok" => (
            "curl -fsSL https://x.ai/cli/install.sh | bash",
            Some("grok login"),
            "grok -p \"ok\"",
            120,
        ),
        _ => return Err(format!("unsupported CLI provider: {provider}")),
    };
    let login_flow = login_command
        .map(|command| format!("  {command} || {{ echo {fail}; exit 1; }}\n"))
        .unwrap_or_default();
    Ok(format!(
        r#"#!/bin/bash
echo {start}
export PATH="$HOME/.local/bin:$HOME/.grok/bin:$HOME/.codex/bin:$PATH"
if ! command -v {provider} >/dev/null 2>&1; then
  {install_command} || {{ echo {fail}; exit 1; }}
fi
echo {login_hint}
verified=0
if {probe_command} >/dev/null 2>&1; then
  verified=1
else
{login_flow}  elapsed=0
  while [ "$elapsed" -lt {poll_seconds} ]; do
    sleep 5
    elapsed=$((elapsed + 5))
    if {probe_command} >/dev/null 2>&1; then
      verified=1
      break
    fi
  done
fi
if [ "$verified" -ne 1 ]; then
  echo {fail}
  exit 1
fi
echo ""
echo {success}
"#
    ))
}

#[cfg(any(target_os = "windows", test))]
fn cli_install_script_windows(
    provider: &str,
    messages: &InstallMessages,
) -> Result<String, String> {
    let start = powershell_quote(&messages.start);
    let login_hint = powershell_quote(&messages.login_hint);
    let success = powershell_quote(&messages.success);
    let fail = powershell_quote(&messages.fail);
    // 探針不加引號：要塞進 cmd /c "..." 包裝，內層引號會讓跳脫變地獄
    let (install_command, login_command, probe_command, poll_seconds) = match provider {
        "claude" => (
            "irm https://claude.ai/install.ps1 | iex",
            Some("claude auth login"),
            "claude -p ok",
            120,
        ),
        "codex" => (
            "irm https://chatgpt.com/codex/install.ps1 | iex",
            Some("codex login"),
            "codex login status",
            120,
        ),
        "agy" => (
            "irm https://antigravity.google/cli/install.ps1 | iex",
            None,
            "agy -p ok",
            600,
        ),
        "grok" => (
            "irm https://x.ai/cli/install.ps1 | iex",
            Some("grok login"),
            "grok -p ok",
            120,
        ),
        _ => return Err(format!("unsupported CLI provider: {provider}")),
    };
    let path = r#"$env:Path = "$env:USERPROFILE\.local\bin;$env:LOCALAPPDATA\Programs\OpenAI\Codex\bin;$env:LOCALAPPDATA\agy\bin;$env:USERPROFILE\.grok\bin;$env:Path""#;
    // PS 5.1 會把原生程式被重導的 stderr 包成 NativeCommandError 紅字漏到畫面上，
    // 改讓 cmd 自己吞輸出（exit code 照樣穿透）
    let silent_probe = format!("cmd /c \"{probe_command} >nul 2>&1\"");
    let login_flow = match login_command {
        Some(command) => format!(
            "  {command}\n  if (-not ($LASTEXITCODE -eq 0)) {{ Write-Output {fail}; exit 1 }}\n"
        ),
        // 無獨立登入指令（agy）：可見地跑一次探針，讓 CLI 把登入 URL 印在視窗上
        None => format!("  {probe_command}\n"),
    };
    Ok(format!(
        r#"{path}
Write-Output {start}
if (-not (Get-Command {provider} -ErrorAction SilentlyContinue)) {{
  {install_command}
  if (-not ($LASTEXITCODE -eq 0)) {{ Write-Output {fail}; exit 1 }}
  {path}
}}
Write-Output {login_hint}
$verified = $false
{silent_probe}
if ($LASTEXITCODE -eq 0) {{
  $verified = $true
}} else {{
{login_flow}  $elapsed = 0
  while ($elapsed -lt {poll_seconds}) {{
    Start-Sleep -Seconds 5
    $elapsed += 5
    {silent_probe}
    if ($LASTEXITCODE -eq 0) {{
      $verified = $true
      break
    }}
  }}
}}
if (-not $verified) {{
  Write-Output {fail}
  exit 1
}}
Write-Output ""
Write-Output {success}
"#
    ))
}

#[tauri::command]
fn install_cli(
    app: tauri::AppHandle,
    provider: String,
    messages: InstallMessages,
) -> Result<(), String> {
    let directory = data_root(&app)?;
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    #[cfg(target_os = "windows")]
    {
        let script = cli_install_script_windows(&provider, &messages)?;
        let script_path = directory.join(format!("install-{provider}.ps1"));
        // UTF-8 BOM 必加：Windows PowerShell 5.1 讀無 BOM 腳本走系統 ANSI 編碼頁，
        // 非英語系統會把多位元組訊息解成亂碼並吞掉引號，整份腳本 ParserError
        let mut bytes = Vec::with_capacity(script.len() + 3);
        bytes.extend_from_slice(b"\xEF\xBB\xBF");
        bytes.extend_from_slice(script.as_bytes());
        std::fs::write(&script_path, bytes).map_err(|error| error.to_string())?;
        Command::new("cmd")
            .args([
                "/C",
                "start",
                "powershell",
                "-NoExit",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(&script_path)
            .spawn()
            .map_err(|error| error.to_string())?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        let script = cli_install_script(&provider, &messages)?;
        let script_path = directory.join(format!("install-{provider}.command"));
        std::fs::write(&script_path, script).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))
                .map_err(|error| error.to_string())?;
        }
        Command::new("open")
            .args(["-a", "Terminal"])
            .arg(&script_path)
            .spawn()
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn list_worlds(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    data::list_worlds(&data_root(&app)?).map_err(|error| error.to_string())
}

#[tauri::command]
fn create_world(app: tauri::AppHandle, name: String) -> Result<(), String> {
    data::create_world(&data_root(&app)?, &name).map_err(|error| error.to_string())
}

#[tauri::command]
fn create_sample_world(app: tauri::AppHandle, lang: String) -> Result<String, String> {
    data::create_sample_world(&data_root(&app)?, &lang).map_err(|error| error.to_string())
}

#[tauri::command]
fn reclaim_world_if_empty(app: tauri::AppHandle, world: String) -> Result<bool, String> {
    data::reclaim_world_if_empty(&data_root(&app)?, &world).map_err(|error| error.to_string())
}

#[tauri::command]
fn rename_world(app: tauri::AppHandle, world: String, new_name: String) -> Result<(), String> {
    data::rename_world(&data_root(&app)?, &world, &new_name).map_err(|error| error.to_string())
}

#[tauri::command]
fn read_world_md(app: tauri::AppHandle, world: String) -> Result<String, String> {
    data::read_world_md(&data_root(&app)?, &world).map_err(|error| error.to_string())
}

#[tauri::command]
fn write_world_md(app: tauri::AppHandle, world: String, content: String) -> Result<(), String> {
    data::write_world_md(&data_root(&app)?, &world, &content).map_err(|error| error.to_string())
}

#[tauri::command]
fn read_worldbook(
    app: tauri::AppHandle,
    world: String,
) -> Result<Vec<data::WorldbookEntry>, String> {
    data::read_worldbook(&data_root(&app)?, &world).map_err(|error| error.to_string())
}

#[tauri::command]
fn upsert_worldbook_entry(
    app: tauri::AppHandle,
    world: String,
    entry: data::WorldbookEntry,
) -> Result<u64, String> {
    data::upsert_worldbook_entry(&data_root(&app)?, &world, entry)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn move_worldbook_entry(
    app: tauri::AppHandle,
    world: String,
    uid: u64,
    up: bool,
) -> Result<(), String> {
    data::move_worldbook_entry(&data_root(&app)?, &world, uid, up)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_worldbook_entry(app: tauri::AppHandle, world: String, uid: u64) -> Result<(), String> {
    data::delete_worldbook_entry(&data_root(&app)?, &world, uid).map_err(|error| error.to_string())
}

#[tauri::command]
fn import_worldbook(
    app: tauri::AppHandle,
    world: String,
    json_text: String,
) -> Result<usize, String> {
    data::import_worldbook(&data_root(&app)?, &world, &json_text).map_err(|error| error.to_string())
}

// 存檔位置由前端的「另存新檔」對話框決定
#[tauri::command]
fn export_worldbook(app: tauri::AppHandle, world: String, path: String) -> Result<(), String> {
    data::export_worldbook(&data_root(&app)?, &world, std::path::Path::new(&path))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_characters(app: tauri::AppHandle, world: String) -> Result<Vec<CharacterMeta>, String> {
    data::list_characters(&data_root(&app)?, &world).map_err(|error| error.to_string())
}

#[tauri::command]
fn read_character(
    app: tauri::AppHandle,
    world: String,
    name: String,
) -> Result<CharacterCard, String> {
    data::read_character(&data_root(&app)?, &world, &name).map_err(|error| error.to_string())
}

#[tauri::command]
fn write_character(
    app: tauri::AppHandle,
    world: String,
    card: CharacterCard,
) -> Result<(), String> {
    data::write_character(&data_root(&app)?, &world, &card).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_character_archived(
    app: tauri::AppHandle,
    world: String,
    name: String,
    archived: bool,
) -> Result<(), String> {
    data::set_character_archived(&data_root(&app)?, &world, &name, archived)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_character(app: tauri::AppHandle, world: String, name: String) -> Result<(), String> {
    data::delete_character(&data_root(&app)?, &world, &name).map_err(|error| error.to_string())
}

#[tauri::command]
fn import_character(
    app: tauri::AppHandle,
    world: String,
    data: Vec<u8>,
    color: String,
) -> Result<CharacterMeta, String> {
    import::import_character(&data_root(&app)?, &world, &data, &color)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn read_character_image(
    app: tauri::AppHandle,
    world: String,
    name: String,
) -> Result<Option<String>, String> {
    import::character_image(&data_root(&app)?, &world, &name).map_err(|error| error.to_string())
}

#[tauri::command]
fn append_transcript(
    app: tauri::AppHandle,
    world: String,
    scene: u64,
    event: TranscriptEvent,
) -> Result<(), String> {
    data::append_transcript(&data_root(&app)?, &world, scene, &event)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn read_transcript(
    app: tauri::AppHandle,
    world: String,
    scene: u64,
) -> Result<Vec<TranscriptEvent>, String> {
    data::read_transcript(&data_root(&app)?, &world, scene).map_err(|error| error.to_string())
}

// 存檔位置由前端的「另存新檔」對話框決定，這裡只負責產內容寫入該路徑
#[tauri::command]
fn export_transcript(app: tauri::AppHandle, world: String, path: String) -> Result<(), String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let markdown = data::export_transcript_markdown(&data_root(&app)?, &world, &lang)
        .map_err(|error| error.to_string())?;
    std::fs::write(&path, markdown).map_err(|error| error.to_string())
}

// 單場匯出：格式與 export_transcript 一致，但只匯一場，供「過去的場」單場檢視使用
#[tauri::command]
fn export_scene(
    app: tauri::AppHandle,
    world: String,
    scene: u64,
    path: String,
) -> Result<(), String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let markdown = data::export_scene_markdown(&data_root(&app)?, &world, scene, &lang)
        .map_err(|error| error.to_string())?;
    std::fs::write(&path, markdown).map_err(|error| error.to_string())
}

#[tauri::command]
fn read_state(app: tauri::AppHandle, world: String) -> Result<WorldState, String> {
    data::read_state(&data_root(&app)?, &world).map_err(|error| error.to_string())
}

#[tauri::command]
fn write_state(app: tauri::AppHandle, world: String, state: WorldState) -> Result<(), String> {
    data::write_state(&data_root(&app)?, &world, &state).map_err(|error| error.to_string())
}

#[tauri::command]
fn read_config(app: tauri::AppHandle) -> Result<AppConfig, String> {
    data::read_config(&config_root(&app)?).map_err(|error| error.to_string())
}

#[tauri::command]
fn write_config(app: tauri::AppHandle, config: AppConfig) -> Result<(), String> {
    data::write_config(&config_root(&app)?, &config).map_err(|error| error.to_string())
}

#[tauri::command]
async fn detect_clis() -> Vec<cli::CliInfo> {
    cli::detect_clis().await
}

/// 設定 UI 下拉用：列出 CLI 訂閱模式可選的模型（讀 CLI 本機快取，見 cli::cli_model_catalog）
#[tauri::command]
fn list_cli_models(cli: String) -> Vec<cli::ModelOption> {
    cli::cli_model_catalog(&cli)
}

/// 依 preferences.transport 把組裝好的訊息分流到 API 或 CLI，增量經 emit 回呼。
/// assistant_label／cli_closing 供 CLI 攤平使用：角色對話與 GM 導演共用同一條路。
async fn stream_via_transport(
    config: &data::AppConfig,
    tier: data::Tier,
    assistant_label: &str,
    cli_closing: &str,
    messages: &[transport::ChatMessage],
    emit: impl FnMut(&str),
) -> Result<String, String> {
    let transport_kind = config
        .preferences
        .get("transport")
        .and_then(|value| value.as_str())
        .unwrap_or("api")
        .to_owned();
    if transport_kind == "api" {
        let model = transport::resolve_model(tier, config)?;
        return transport::stream_chat(config, &model, messages, emit)
            .await
            .map_err(|error| error.to_string());
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

    let (system, prompt) = cli::flatten_messages(assistant_label, cli_closing, messages);
    let program = std::path::PathBuf::from(&info.path);
    match transport_kind.as_str() {
        "claude" => {
            let model = cli::tier_override(&config.tier_models, "claude", tier)
                .or_else(|| cli::claude_model_for(tier));
            let args = cli::claude_args(model, &system);
            cli::run_cli(&program, &args, &prompt, cli::parse_claude_line, emit).await
        }
        "codex" => {
            // codex 沒有 system prompt 旗標，併進 prompt 開頭；未覆寫時模型用 CLI 預設
            let model = cli::tier_override(&config.tier_models, "codex", tier);
            let args = cli::codex_args(model, cli::codex_effort_for(tier));
            let combined = format!("{system}\n\n{prompt}");
            cli::run_cli(&program, &args, &combined, cli::parse_codex_line, emit).await
        }
        "agy" => {
            // agy 沒有 system prompt 旗標，併進 prompt 開頭；未覆寫時使用 CLI 預設模型
            let model = cli::tier_override(&config.tier_models, "agy", tier);
            let combined = format!("{system}\n\n{prompt}");
            let args = cli::agy_args(model, &combined);
            cli::run_cli(&program, &args, "", cli::parse_agy_line, emit).await
        }
        "grok" => {
            // grok 沒有 system prompt 旗標，併進 prompt 開頭；未覆寫時使用 CLI 預設模型
            let model = cli::tier_override(&config.tier_models, "grok", tier);
            let combined = format!("{system}\n\n{prompt}");
            let args = cli::grok_args(model, &combined);
            cli::run_cli(&program, &args, "", cli::parse_grok_line, emit).await
        }
        other => Err(format!("未知傳輸層：{other}").into()),
    }
    .map_err(|error| error.to_string())
}

/// 上下文組裝→單發呼叫→串流回傳（KICKOFF §4）。
/// 上下文完全由本機正典（角色卡＋可見世界書＋公開 transcript）經 assemble_messages 組裝，
/// 再依 preferences.transport 分流到 API 或 CLI；增量文字經 on_delta channel 回前端。
#[tauri::command]
async fn chat_with_character(
    app: tauri::AppHandle,
    world: String,
    character: String,
    on_delta: tauri::ipc::Channel<String>,
) -> Result<String, String> {
    let root = data_root(&app)?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let card =
        data::read_character(&root, &world, &character).map_err(|error| error.to_string())?;
    let state = data::read_state(&root, &world).map_err(|error| error.to_string())?;
    let events = data::read_transcript(&root, &world, state.current_scene)
        .map_err(|error| error.to_string())?;
    let worldbook = data::read_worldbook(&root, &world).map_err(|error| error.to_string())?;

    let messages =
        transport::assemble_messages(&card, &events, &worldbook, &transport::ui_language(&config));
    let closing = format!(
        "現在輪到「{}」回應。請直接輸出台詞與動作描寫，不要加名字前綴、不要任何角色之外的說明。",
        card.name
    );
    let emit = |delta: &str| {
        let _ = on_delta.send(delta.to_owned());
    };
    stream_via_transport(&config, card.tier, &card.name, &closing, &messages, emit).await
}

/// GM 上下文＝world.md＋世界書＋全部角色卡（含私有）＋公開 transcript（NewPlan §7.0）。
/// 回傳（角色名單, 組裝好的訊息）。
fn assemble_gm(
    root: &std::path::Path,
    world: &str,
    lang: &str,
) -> Result<(Vec<String>, Vec<transport::ChatMessage>), String> {
    let world_md = data::read_world_md(root, world).map_err(|error| error.to_string())?;
    let worldbook = data::read_worldbook(root, world).map_err(|error| error.to_string())?;
    let state = data::read_state(root, world).map_err(|error| error.to_string())?;
    let events = data::read_transcript(root, world, state.current_scene)
        .map_err(|error| error.to_string())?;
    let cards: Vec<data::CharacterCard> = data::list_characters(root, world)
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|meta| !meta.archived)
        .map(|meta| data::read_character(root, world, &meta.name))
        .collect::<Result<_, _>>()
        .map_err(|error| error.to_string())?;
    let roster = cards.iter().map(|card| card.name.clone()).collect();
    Ok((
        roster,
        transport::assemble_gm_messages(&world_md, &cards, &events, &worldbook, lang),
    ))
}

/// 簡易導演：GM 插入旁白（NewPlan §6.1），串流回前端後由前端落 transcript。
#[tauri::command]
async fn gm_narrate(
    app: tauri::AppHandle,
    world: String,
    on_delta: tauri::ipc::Channel<String>,
) -> Result<String, String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let (_, mut messages) =
        assemble_gm(&data_root(&app)?, &world, &transport::ui_language(&config))?;
    messages.push(transport::narrate_instruction());
    let emit = |delta: &str| {
        let _ = on_delta.send(delta.to_owned());
    };
    stream_via_transport(
        &config,
        transport::gm_tier(&config),
        "GM",
        "現在請以 GM 身分執行上述導演指示，只輸出旁白本文，不要加名字前綴。",
        &messages,
        emit,
    )
    .await
}

/// 簡易導演：GM 建議下一位發言者（NewPlan §6.1）。
/// 回傳名單中的角色名，或「玩家」表示該輪到玩家行動。
#[tauri::command]
async fn gm_suggest_speaker(app: tauri::AppHandle, world: String) -> Result<String, String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let (roster, mut messages) =
        assemble_gm(&data_root(&app)?, &world, &transport::ui_language(&config))?;
    if roster.is_empty() {
        return Err("這一桌還沒有角色，先建立角色再讓 GM 點名".to_owned());
    }
    messages.push(transport::suggest_instruction(&roster));
    let reply = stream_via_transport(
        &config,
        transport::gm_tier(&config),
        "GM",
        "現在請執行上述導演指示，只輸出一個名字。",
        &messages,
        |_| {},
    )
    .await?;
    transport::pick_speaker(&reply, &roster)
        .ok_or_else(|| format!("GM 的點名無法對應任何角色：{reply}"))
}

/// 換場：把當前場景公開紀錄壓成一則摘要，寫進新場景開頭，current_scene +1（NewPlan 換場＋場景摘要）。
/// 摘要走既有 stream_via_transport＋GM 檔位，不新開連線路徑、不新增設定項。
#[tauri::command]
async fn advance_scene(app: tauri::AppHandle, world: String) -> Result<u64, String> {
    let root = data_root(&app)?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let state = data::read_state(&root, &world).map_err(|error| error.to_string())?;
    let events = data::read_transcript(&root, &world, state.current_scene)
        .map_err(|error| error.to_string())?;
    if events.is_empty() {
        return Err("這個場景還沒有任何紀錄，沒東西可以換場".to_owned());
    }

    let messages = transport::summary_messages(&events, &lang);
    let reply = stream_via_transport(
        &config,
        transport::gm_tier(&config),
        "GM",
        "現在請執行上述導演指示，只輸出摘要本文，不要加名字前綴。",
        &messages,
        |_| {},
    )
    .await?;

    // 換幕順手取幕名：回覆第一行「標題：…」／「Title: …」解析不到就整段當摘要，不報錯
    let (title, summary) = transport::extract_scene_title(&reply);
    data::begin_next_scene(&root, &world, &summary, &lang, title.as_deref())
        .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            list_worlds,
            create_world,
            create_sample_world,
            reclaim_world_if_empty,
            rename_world,
            read_world_md,
            write_world_md,
            read_worldbook,
            upsert_worldbook_entry,
            move_worldbook_entry,
            delete_worldbook_entry,
            import_worldbook,
            export_worldbook,
            list_characters,
            read_character,
            write_character,
            set_character_archived,
            delete_character,
            import_character,
            read_character_image,
            append_transcript,
            read_transcript,
            export_transcript,
            export_scene,
            read_state,
            write_state,
            read_config,
            write_config,
            detect_clis,
            install_cli,
            list_cli_models,
            chat_with_character,
            gm_narrate,
            gm_suggest_speaker,
            advance_scene
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{cli_install_script, cli_install_script_windows, InstallMessages};

    fn messages() -> InstallMessages {
        InstallMessages {
            start: "start".to_owned(),
            login_hint: "login hint".to_owned(),
            success: "success".to_owned(),
            fail: "fail".to_owned(),
        }
    }

    fn assert_messages(script: &str) {
        for text in ["start", "login hint", "success", "fail"] {
            assert!(script.contains(text));
        }
    }

    #[test]
    fn claude_install_script_contains_messages_and_flow() {
        let script = cli_install_script("claude", &messages()).unwrap();
        assert_messages(&script);
        assert!(script.contains("curl -fsSL https://claude.ai/install.sh | bash"));
        assert!(script.contains("claude auth login"));
        assert!(script.contains("claude -p \"ok\" >/dev/null 2>&1"));
        assert!(script.contains("while [ \"$elapsed\" -lt 120 ]"));
    }

    #[test]
    fn codex_install_script_contains_messages_and_flow() {
        let script = cli_install_script("codex", &messages()).unwrap();
        assert_messages(&script);
        assert!(script.contains("curl -fsSL https://chatgpt.com/codex/install.sh | sh"));
        assert!(script.contains("codex login"));
        assert!(script.contains("codex login status >/dev/null 2>&1"));
        assert!(script.contains("while [ \"$elapsed\" -lt 120 ]"));
    }

    #[test]
    fn agy_provider_script_contains_messages_and_flow() {
        let script = cli_install_script("agy", &messages()).unwrap();
        assert_messages(&script);
        assert!(script.contains("curl -fsSL https://antigravity.google/cli/install.sh | bash"));
        assert!(!script.contains("claude auth login"));
        assert!(!script.contains("codex login"));
        assert!(!script.contains("grok login"));
        assert!(script.contains("agy -p \"ok\" >/dev/null 2>&1"));
        assert!(script.contains("while [ \"$elapsed\" -lt 600 ]"));
        assert!(script.contains("sleep 5"));
    }

    #[test]
    fn grok_install_script_contains_messages_and_flow() {
        let script = cli_install_script("grok", &messages()).unwrap();
        assert_messages(&script);
        assert!(script.contains("curl -fsSL https://x.ai/cli/install.sh | bash"));
        assert!(script.contains("grok login"));
        assert!(script.contains("grok -p \"ok\" >/dev/null 2>&1"));
        assert!(script.contains("while [ \"$elapsed\" -lt 120 ]"));
    }

    #[test]
    fn cli_install_script_escapes_single_quotes_and_rejects_unknown_provider() {
        let quoted_messages = InstallMessages {
            start: "don't".to_owned(),
            login_hint: "login".to_owned(),
            success: "success".to_owned(),
            fail: "fail".to_owned(),
        };
        assert!(cli_install_script("agy", &quoted_messages)
            .unwrap()
            .contains("'don'\"'\"'t'"));
        assert!(cli_install_script("unknown", &messages()).is_err());
    }

    const WINDOWS_PATH: &str = "$env:Path = \"$env:USERPROFILE\\.local\\bin;$env:LOCALAPPDATA\\Programs\\OpenAI\\Codex\\bin;$env:LOCALAPPDATA\\agy\\bin;$env:USERPROFILE\\.grok\\bin;$env:Path\"";

    #[test]
    fn windows_claude_install_script_contains_install_command_and_path() {
        let script = cli_install_script_windows("claude", &messages()).unwrap();
        assert!(script.contains("irm https://claude.ai/install.ps1 | iex"));
        assert!(script.contains(WINDOWS_PATH));
    }

    #[test]
    fn windows_codex_install_script_contains_install_command_and_path() {
        let script = cli_install_script_windows("codex", &messages()).unwrap();
        assert!(script.contains("irm https://chatgpt.com/codex/install.ps1 | iex"));
        assert!(script.contains(WINDOWS_PATH));
    }

    #[test]
    fn windows_agy_install_script_contains_install_command_and_path() {
        let script = cli_install_script_windows("agy", &messages()).unwrap();
        assert!(script.contains("irm https://antigravity.google/cli/install.ps1 | iex"));
        assert!(script.contains(WINDOWS_PATH));
    }

    #[test]
    fn windows_grok_install_script_contains_install_command_and_path() {
        let script = cli_install_script_windows("grok", &messages()).unwrap();
        assert!(script.contains("irm https://x.ai/cli/install.ps1 | iex"));
        assert!(script.contains(WINDOWS_PATH));
    }

    #[test]
    fn windows_install_script_escapes_single_quotes() {
        let quoted_messages = InstallMessages {
            start: "don't".to_owned(),
            login_hint: "login".to_owned(),
            success: "success".to_owned(),
            fail: "fail".to_owned(),
        };
        assert!(cli_install_script_windows("agy", &quoted_messages)
            .unwrap()
            .contains("'don''t'"));
    }

    #[test]
    fn windows_agy_script_shows_login_url_and_silences_polling() {
        let script = cli_install_script_windows("agy", &messages()).unwrap();
        // 輪詢探針交給 cmd 吞輸出，避免 PS 5.1 NativeCommandError 紅字
        assert!(script.contains("cmd /c \"agy -p ok >nul 2>&1\""));
        // agy 無獨立登入指令：登入階段可見地跑一次探針，讓登入 URL 印得出來
        assert!(script.contains("\n  agy -p ok\n"));
    }

    #[test]
    fn windows_install_script_rejects_unknown_provider() {
        assert!(cli_install_script_windows("unknown", &messages()).is_err());
    }
}
