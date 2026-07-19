mod data;
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

/// 上下文組裝→單發呼叫→串流回傳（KICKOFF §4）。
/// 上下文完全由本機正典（角色卡＋公開 transcript）組裝；
/// 增量文字經 on_delta channel 回前端，完整回覆作為回傳值。
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

    let model = transport::resolve_model(card.tier, &config)?;
    let messages = transport::assemble_messages(&card, &events);
    transport::stream_chat(&config, &model, &messages, |delta| {
        let _ = on_delta.send(delta.to_owned());
    })
    .await
    .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            list_worlds,
            create_world,
            read_world_md,
            write_world_md,
            list_characters,
            read_character,
            write_character,
            append_transcript,
            read_transcript,
            read_state,
            write_state,
            read_config,
            write_config,
            chat_with_character
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
