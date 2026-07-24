mod cli;
mod data;
mod import;
mod transport;

use data::{AppConfig, CharacterCard, CharacterMeta, TranscriptEvent, WorldState};
use std::path::PathBuf;
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
        other => Err(format!("未知傳輸層：{other}").into()),
    }
    .map_err(|error| error.to_string())
}

/// 上下文組裝→單發呼叫→串流回傳（KICKOFF §4）。
/// 上下文完全由本機正典（角色卡＋公開 transcript）經 assemble_messages 組裝，
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
    let card = data::read_character(&root, &world, &character).map_err(|error| error.to_string())?;
    let state = data::read_state(&root, &world).map_err(|error| error.to_string())?;
    let events = data::read_transcript(&root, &world, state.current_scene)
        .map_err(|error| error.to_string())?;

    let messages = transport::assemble_messages(&card, &events, &transport::ui_language(&config));
    let closing = format!(
        "現在輪到「{}」回應。請直接輸出台詞與動作描寫，不要加名字前綴、不要任何角色之外的說明。",
        card.name
    );
    let emit = |delta: &str| {
        let _ = on_delta.send(delta.to_owned());
    };
    stream_via_transport(&config, card.tier, &card.name, &closing, &messages, emit).await
}

/// GM 上下文＝world.md（只進 GM）＋全部角色卡（含私有）＋公開 transcript（NewPlan §7.0）。
/// 回傳（角色名單, 組裝好的訊息）。
fn assemble_gm(root: &std::path::Path, world: &str, lang: &str) -> Result<(Vec<String>, Vec<transport::ChatMessage>), String> {
    let world_md = data::read_world_md(root, world).map_err(|error| error.to_string())?;
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
    Ok((roster, transport::assemble_gm_messages(&world_md, &cards, &events, lang)))
}

/// 簡易導演：GM 插入旁白（NewPlan §6.1），串流回前端後由前端落 transcript。
#[tauri::command]
async fn gm_narrate(
    app: tauri::AppHandle,
    world: String,
    on_delta: tauri::ipc::Channel<String>,
) -> Result<String, String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let (_, mut messages) = assemble_gm(&data_root(&app)?, &world, &transport::ui_language(&config))?;
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
    let (roster, mut messages) = assemble_gm(&data_root(&app)?, &world, &transport::ui_language(&config))?;
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
            list_cli_models,
            chat_with_character,
            gm_narrate,
            gm_suggest_speaker,
            advance_scene
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
