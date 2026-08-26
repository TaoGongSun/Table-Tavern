use crate::ai_transport::stream_via_transport;
use crate::{config_root, data, data_root, genesis, transport, usage_log};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OutlineOutcome {
    parsed: Option<genesis::Outline>,
    raw: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CharacterOutcome {
    parsed: Option<genesis::OutlineCharacter>,
    raw: String,
}

#[tauri::command]
pub(crate) async fn generate_table_outline(
    app: tauri::AppHandle,
    input: String,
    genres: Vec<String>,
) -> Result<OutlineOutcome, String> {
    let input = input.trim();
    if input.is_empty() && genres.is_empty() {
        return Err("EMPTY_INPUT".to_owned());
    }
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let messages = genesis::outline_messages(input, &genres, &lang);
    let raw = stream_via_transport(
        &app,
        &config,
        None,
        false,
        transport::gm_tier(&config),
        None,
        "GM",
        "Generate the campaign outline exactly in the requested structure.",
        &messages,
        false,
        |_| {},
    )
    .await?;
    Ok(OutlineOutcome {
        parsed: genesis::parse_outline(&raw),
        raw,
    })
}

#[tauri::command]
pub(crate) async fn generate_table_character(
    app: tauri::AppHandle,
    input: String,
    genres: Vec<String>,
    outline_raw: String,
    hint: String,
) -> Result<CharacterOutcome, String> {
    let input = input.trim();
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let messages = genesis::character_messages(input, &genres, &outline_raw, &hint, &lang);
    let raw = stream_via_transport(
        &app,
        &config,
        None,
        false,
        transport::gm_tier(&config),
        None,
        "GM",
        "Generate exactly one character in the requested structure.",
        &messages,
        false,
        |_| {},
    )
    .await?;
    Ok(CharacterOutcome {
        parsed: genesis::parse_character(&raw),
        raw,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExpandOutcome {
    world_id: Option<String>,
    raw: String,
}

#[tauri::command]
pub(crate) async fn generate_table_expand(
    app: tauri::AppHandle,
    input: String,
    genres: Vec<String>,
    outline_raw: String,
) -> Result<ExpandOutcome, String> {
    let input = input.trim();
    if input.is_empty() && genres.is_empty() {
        return Err("EMPTY_INPUT".to_owned());
    }
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let messages = genesis::expand_messages(input, &genres, &outline_raw, &lang);
    let raw = stream_via_transport(
        &app,
        &config,
        None,
        false,
        transport::gm_tier(&config),
        None,
        "GM",
        "Generate the full campaign materials exactly in the requested structure.",
        &messages,
        false,
        |_| {},
    )
    .await?;
    let world_id = genesis::parse_expand(&raw)
        .map(|expanded| genesis::materialize(&data_root(&app)?, &expanded))
        .transpose()
        .map_err(|error| error.to_string())?;
    // 開桌這幾次呼叫的額度就是為這桌花的：桌一建好就把剛才那些行認領回來（見 usage_log）
    if let (Some(id), Ok(root)) = (&world_id, data_root(&app)) {
        usage_log::assign_pending_world(&root.join("prompt-cache.jsonl"), id);
    }
    Ok(ExpandOutcome { world_id, raw })
}
