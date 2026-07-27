mod cli;
mod data;
mod import;
#[allow(dead_code)]
mod install;
mod transport;

use data::{AppConfig, CharacterCard, CharacterMeta, TranscriptEvent, WorldState};
use serde::Deserialize;
use std::path::PathBuf;
#[cfg(not(target_os = "windows"))]
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

#[tauri::command]
fn install_cli(
    app: tauri::AppHandle,
    provider: String,
    messages: InstallMessages,
) -> Result<(), String> {
    let directory = data_root(&app)?;
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let _ = &messages;
    #[cfg(target_os = "windows")]
    {
        use std::time::Duration;
        use tauri::Emitter;

        let spec = install::windows_specs()?
            .into_iter()
            .find(|spec| spec.id == provider)
            .ok_or_else(|| format!("unsupported CLI provider: {provider}"))?;
        let token = match install::try_begin(&provider, Duration::from_secs(60)) {
            install::BeginOutcome::Started(token) => token,
            install::BeginOutcome::AlreadyRunning => {
                install::raise_login_window(&spec.window_title);
                return Ok(());
            }
            install::BeginOutcome::Cooldown(seconds) => {
                return Err(format!("login-cooldown:{seconds}"))
            }
        };
        let task_app = app.clone();
        tauri::async_runtime::spawn(async move {
            let _token = token;
            let emit_app = task_app.clone();
            let _ = install::run_install(spec, &directory, cli::find_binary, move |progress| {
                let _ = emit_app.emit("cli-install-progress", progress);
            })
            .await;
        });
    }
    #[cfg(not(target_os = "windows"))]
    {
        use std::time::Duration;

        if install::mac_cooldown(&provider, Duration::from_secs(60)).is_some() {
            Command::new("open")
                .args(["-a", "Terminal"])
                .spawn()
                .map_err(|error| error.to_string())?;
            return Ok(());
        }
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
fn save_character_image(
    app: tauri::AppHandle,
    world: String,
    name: String,
    data: Vec<u8>,
) -> Result<(), String> {
    import::save_character_image(&data_root(&app)?, &world, &name, &data)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_character_image(
    app: tauri::AppHandle,
    world: String,
    name: String,
) -> Result<(), String> {
    import::delete_character_image(&data_root(&app)?, &world, &name)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn read_character_avatar(
    app: tauri::AppHandle,
    world: String,
    name: String,
) -> Result<Option<String>, String> {
    import::character_avatar(&data_root(&app)?, &world, &name).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_character_avatar(
    app: tauri::AppHandle,
    world: String,
    name: String,
    data: Vec<u8>,
) -> Result<(), String> {
    import::save_character_avatar(&data_root(&app)?, &world, &name, &data)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_character_avatar(
    app: tauri::AppHandle,
    world: String,
    name: String,
) -> Result<(), String> {
    import::delete_character_avatar(&data_root(&app)?, &world, &name)
        .map_err(|error| error.to_string())
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
    transport_override: Option<&str>,
    allow_cli_tools: bool,
    tier: data::Tier,
    assistant_label: &str,
    cli_closing: &str,
    messages: &[transport::ChatMessage],
    emit: impl FnMut(&str),
) -> Result<String, String> {
    // transport_override：生圖等功能可指定與聊天不同的連線（None＝跟隨 preferences.transport）。
    // allow_cli_tools：只有生圖呼叫為 true——CLI 生圖工具要寫檔／跑指令，聊天一律鎖死工具。
    let transport_kind = transport_override
        .map(str::to_owned)
        .unwrap_or_else(|| {
            config
                .preferences
                .get("transport")
                .and_then(|value| value.as_str())
                .unwrap_or("api")
                .to_owned()
        });
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
            let base_url = config
                .preferences
                .get("claude_base_url")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .unwrap_or("");
            let envs = if base_url.is_empty() {
                Vec::new()
            } else {
                let mut envs = vec![("ANTHROPIC_BASE_URL".to_owned(), base_url.to_owned())];
                if let Some(api_key) = config
                    .api_keys
                    .get("claude_compat")
                    .map(String::as_str)
                    .map(str::trim)
                    .filter(|key| !key.is_empty())
                {
                    envs.push(("ANTHROPIC_AUTH_TOKEN".to_owned(), api_key.to_owned()));
                }
                envs
            };
            cli::run_cli(
                &program,
                &args,
                &prompt,
                &envs,
                cli::parse_claude_line,
                emit,
            )
            .await
        }
        "codex" => {
            // codex 沒有 system prompt 旗標，併進 prompt 開頭；未覆寫時模型用 CLI 預設
            let model = cli::tier_override(&config.tier_models, "codex", tier);
            let args = cli::codex_args(model, cli::codex_effort_for(tier), allow_cli_tools);
            let combined = format!("{system}\n\n{prompt}");
            cli::run_cli(&program, &args, &combined, &[], cli::parse_codex_line, emit).await
        }
        "agy" => {
            // agy 沒有 system prompt 旗標，併進 prompt 開頭；未覆寫時使用 CLI 預設模型
            let model = cli::tier_override(&config.tier_models, "agy", tier);
            let combined = format!("{system}\n\n{prompt}");
            let args = cli::agy_args(model, &combined, allow_cli_tools);
            cli::run_cli(&program, &args, "", &[], cli::parse_agy_line, emit).await
        }
        "grok" => {
            // grok 沒有 system prompt 旗標，併進 prompt 開頭；未覆寫時使用 CLI 預設模型
            let model = cli::tier_override(&config.tier_models, "grok", tier);
            let combined = format!("{system}\n\n{prompt}");
            let args = cli::grok_args(model, &combined, allow_cli_tools);
            cli::run_cli(&program, &args, "", &[], cli::parse_grok_line, emit).await
        }
        other => Err(format!("未知傳輸層：{other}").into()),
    }
    .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageRef {
    DataUrl(String),
    Path(PathBuf),
}

pub fn extract_image_from_text(text: &str) -> Option<ImageRef> {
    if let Some(start) = text.find("data:image/") {
        let data_url = text[start..]
            .split(|character: char| {
                character.is_whitespace() || matches!(character, '\'' | '"' | '`')
            })
            .next()
            .unwrap_or("");
        if !data_url.is_empty() {
            return Some(ImageRef::DataUrl(data_url.to_owned()));
        }
    }
    text.split_whitespace().find_map(|token| {
        let token = token.trim_matches(|character: char| {
            matches!(
                character,
                '\'' | '"' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | ';'
            )
        });
        let path = PathBuf::from(token);
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "webp")
            .then_some(ImageRef::Path(path))
    })
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        output.push(TABLE[((value >> 18) & 0x3f) as usize] as char);
        output.push(TABLE[((value >> 12) & 0x3f) as usize] as char);
        output.push(if chunk.len() > 1 {
            TABLE[((value >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(value & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    output
}

fn decode_base64(value: &str) -> Result<Vec<u8>, String> {
    fn sextet(byte: u8) -> Option<u8> {
        match byte {
            b'A'..=b'Z' => Some(byte - b'A'),
            b'a'..=b'z' => Some(byte - b'a' + 26),
            b'0'..=b'9' => Some(byte - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }

    let bytes = value.as_bytes();
    if bytes.len() % 4 != 0 {
        return Err("非法 base64 資料".to_owned());
    }
    let mut output = Vec::with_capacity(bytes.len() / 4 * 3);
    for (index, chunk) in bytes.chunks_exact(4).enumerate() {
        let padding = chunk.iter().rev().take_while(|&&byte| byte == b'=').count();
        if padding > 2 || (padding > 0 && index + 1 != bytes.len() / 4) {
            return Err("非法 base64 資料".to_owned());
        }
        let a = sextet(chunk[0]).ok_or_else(|| "非法 base64 資料".to_owned())?;
        let b = sextet(chunk[1]).ok_or_else(|| "非法 base64 資料".to_owned())?;
        let c = if padding >= 2 { 0 } else { sextet(chunk[2]).ok_or_else(|| "非法 base64 資料".to_owned())? };
        let d = if padding >= 1 { 0 } else { sextet(chunk[3]).ok_or_else(|| "非法 base64 資料".to_owned())? };
        if (padding >= 1 && chunk[3] != b'=') || (padding >= 2 && chunk[2] != b'=') {
            return Err("非法 base64 資料".to_owned());
        }
        let decoded = (u32::from(a) << 18) | (u32::from(b) << 12) | (u32::from(c) << 6) | u32::from(d);
        output.push((decoded >> 16) as u8);
        if padding < 2 { output.push((decoded >> 8) as u8); }
        if padding == 0 { output.push(decoded as u8); }
    }
    Ok(output)
}

fn validate_gallery_component(value: &str, require_png: bool) -> Result<(), String> {
    if value.is_empty()
        || value.contains("..")
        || value.contains('/')
        || value.contains('\\')
        || (require_png && !value.ends_with(".png"))
    {
        return Err("非法檔名".to_owned());
    }
    Ok(())
}

fn gallery_directory(root: &std::path::Path, world: &str, name: &str) -> Result<PathBuf, String> {
    validate_gallery_component(name, false)?;
    Ok(root.join(world).join("gen-gallery").join(name))
}

fn list_gallery_image_files(root: &std::path::Path, world: &str, name: &str) -> Result<Vec<String>, String> {
    let directory = gallery_directory(root, world, name)?;
    let mut files = match std::fs::read_dir(directory) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|file| file.ends_with(".png"))
            .collect::<Vec<_>>(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.to_string()),
    };
    files.sort_unstable_by(|left, right| right.cmp(left));
    Ok(files)
}

fn save_generated_gallery_image(root: &std::path::Path, world: &str, name: &str, data_url: &str) -> Result<(), String> {
    let Some((header, encoded)) = data_url.split_once(',') else { return Ok(()); };
    if !header.starts_with("data:") || !header.ends_with(";base64") { return Ok(()); }
    let directory = gallery_directory(root, world, name)?;
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|error| error.to_string())?.as_millis();
    std::fs::write(directory.join(format!("{timestamp}.png")), decode_base64(encoded)?).map_err(|error| error.to_string())
}

fn image_file_data_url(path: &std::path::Path) -> Result<String, String> {
    let mime = match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => return Err("不支援的圖片格式".to_owned()),
    };
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    Ok(format!("data:{mime};base64,{}", encode_base64(&bytes)))
}

#[tauri::command]
async fn generate_character_image(
    app: tauri::AppHandle,
    world: String,
    name: String,
    extra_prompt: String,
    source: Option<String>,
) -> Result<String, String> {
    let root = data_root(&app)?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let mut card = data::read_character(&root, &world, &name).map_err(|error| error.to_string())?;
    card.gen_prompt = extra_prompt.clone();
    data::write_character(&root, &world, &card).map_err(|error| error.to_string())?;
    let mut prompt = format!(
        "Generate a single full-body character illustration, portrait orientation 2:3. No text, no watermark, plain background.\nCharacter name: {name}\nCharacter description:\n{}",
        card.public_md
    );
    if !extra_prompt.trim().is_empty() {
        prompt.push_str(&format!("\nAdditional art direction: {extra_prompt}"));
    }
    // 生圖來源可與聊天連線分開選（source 覆寫；空值＝跟隨 preferences.transport）
    let transport_kind = source
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            config
                .preferences
                .get("transport")
                .and_then(|value| value.as_str())
                .unwrap_or("api")
                .to_owned()
        });
    if transport_kind == "api" {
        let image = transport::generate_image(&config, &prompt).await?;
        save_generated_gallery_image(&root, &world, &name, &image)?;
        return Ok(image);
    }
    // CLI 一律照送：能生圖的家（codex $imagegen／agy／grok）會存檔回路徑，其餘掃不到圖就失敗
    prompt.push_str(
        "\nIf you are able to generate images, generate it now, save it as a PNG file, and reply with the absolute file path of the saved image. If you cannot generate images, reply exactly: NO_IMAGE",
    );
    if transport_kind == "codex" {
        prompt = format!("$imagegen {prompt}");
    }
    let messages = [transport::ChatMessage {
        role: "user".to_owned(),
        content: prompt,
    }];
    let reply = stream_via_transport(
        &config,
        Some(&transport_kind),
        true,
        transport::gm_tier(&config),
        "",
        "",
        &messages,
        |_| {},
    )
    .await?;
    let image = match extract_image_from_text(&reply) {
        Some(ImageRef::DataUrl(data_url)) => Ok(data_url),
        Some(ImageRef::Path(path)) if std::fs::metadata(&path).is_ok() => {
            image_file_data_url(&path)
        }
        _ => Err("回覆中沒有圖片".to_owned()),
    }?;
    save_generated_gallery_image(&root, &world, &name, &image)?;
    Ok(image)
}

#[tauri::command]
fn list_gallery_images(app: tauri::AppHandle, world: String, name: String) -> Result<Vec<String>, String> {
    list_gallery_image_files(&data_root(&app)?, &world, &name)
}

#[tauri::command]
fn read_gallery_image(app: tauri::AppHandle, world: String, name: String, file: String) -> Result<String, String> {
    validate_gallery_component(&file, true)?;
    let directory = gallery_directory(&data_root(&app)?, &world, &name)?;
    image_file_data_url(&directory.join(file))
}

#[tauri::command]
fn delete_gallery_image(app: tauri::AppHandle, world: String, name: String, file: String) -> Result<(), String> {
    validate_gallery_component(&file, true)?;
    let directory = gallery_directory(&data_root(&app)?, &world, &name)?;
    std::fs::remove_file(directory.join(file)).map_err(|error| error.to_string())
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
    stream_via_transport(&config, None, false, card.tier, &card.name, &closing, &messages, emit).await
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
        None,
        false,
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
        None,
        false,
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
        None,
        false,
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
            save_character_image,
            delete_character_image,
            read_character_avatar,
            save_character_avatar,
            delete_character_avatar,
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
            generate_character_image,
            list_gallery_images,
            read_gallery_image,
            delete_gallery_image,
            gm_narrate,
            gm_suggest_speaker,
            advance_scene
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{
        cli_install_script, decode_base64, encode_base64, extract_image_from_text,
        list_gallery_image_files, validate_gallery_component, ImageRef, InstallMessages,
    };
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    fn messages() -> InstallMessages {
        InstallMessages {
            start: "start".to_owned(),
            login_hint: "login hint".to_owned(),
            success: "success".to_owned(),
            fail: "fail".to_owned(),
        }
    }

    #[test]
    fn extract_image_from_text_returns_data_url() {
        assert_eq!(
            extract_image_from_text("圖片：`data:image/png;base64,cG5n`"),
            Some(ImageRef::DataUrl("data:image/png;base64,cG5n".to_owned()))
        );
    }

    #[test]
    fn extract_image_from_text_returns_existing_temp_file_path() {
        let path = std::env::temp_dir().join(format!(
            "table-tavern-image-{}-{}.png",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, b"png").unwrap();
        assert_eq!(
            extract_image_from_text(&format!("已生成 {}", path.display())),
            Some(ImageRef::Path(path.clone()))
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn extract_image_from_text_returns_none_without_image() {
        assert_eq!(extract_image_from_text("沒有附圖。"), None);
        assert_eq!(encode_base64(b"png"), "cG5n");
    }

    #[test]
    fn decode_base64_roundtrip_restores_bytes() {
        let bytes = [0, 1, 2, 127, 128, 255];
        assert_eq!(decode_base64(&encode_base64(&bytes)).unwrap(), bytes);
    }

    #[test]
    fn decode_base64_rejects_invalid_input() {
        assert!(decode_base64("not base64!").is_err());
    }

    #[test]
    fn gallery_component_validation_allows_plain_png_name() {
        assert!(validate_gallery_component("1720000000000.png", true).is_ok());
    }

    #[test]
    fn gallery_component_validation_rejects_parent_path() {
        assert!(validate_gallery_component("../secret.png", true).is_err());
    }

    #[test]
    fn gallery_component_validation_rejects_path_separator() {
        assert!(validate_gallery_component("folder/image.png", true).is_err());
    }

    #[test]
    fn list_gallery_image_files_sorts_newest_first() {
        let root = std::env::temp_dir().join(format!(
            "table-tavern-gallery-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let directory = root.join("world").join("gen-gallery").join("character");
        std::fs::create_dir_all(&directory).unwrap();
        for file in ["1720000000000.png", "1730000000000.png", "1710000000000.png"] {
            std::fs::write(directory.join(file), b"png").unwrap();
        }
        assert_eq!(
            list_gallery_image_files(&root, "world", "character").unwrap(),
            ["1730000000000.png", "1720000000000.png", "1710000000000.png"]
        );
        std::fs::remove_dir_all(root).unwrap();
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
}
