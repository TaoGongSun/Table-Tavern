use crate::data::vn::{Preferences, SetupStage};
use crate::transport::dispatch::stream_via_transport;
use crate::usage::log as usage_log;
use crate::{config_root, data, data_root, genesis, transport};
use serde::Serialize;

/// 聊天桌一句話與題材至少要有一樣；VN 精靈（帶偏好）全空白也收，由 AI 自選題材。
fn refuse_empty(input: &str, genres: &[String], vn: bool) -> Result<(), String> {
    match !vn && input.is_empty() && genres.is_empty() {
        true => Err("EMPTY_INPUT".to_owned()),
        false => Ok(()),
    }
}

/// VN 精靈換步驟時記下停在哪（None＝按開始遊戲成功，設定完成）。匯入來的桌還沒有 vn.json：
/// 先照精靈的劇透鎖建檔（跟聖經生成的 `create_if_missing` 同一條），鎖才不會被預設蓋掉。
#[tauri::command]
pub(crate) fn vn_setup_set(
    app: tauri::AppHandle,
    world_id: String,
    stage: Option<SetupStage>,
    spoiler_lock: Option<bool>,
) -> Result<(), String> {
    let _permit = data::world_write_permit(&world_id)?;
    let root = data_root(&app)?;
    data::state_commit::with_commit(&root, &world_id, |tx| {
        data::vn::create_if_missing(tx, spoiler_lock)?;
        data::vn::edit_vn(tx, |file| file.setup = stage)
    })
    .map_err(|error| error.to_string())
}

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
    preferences: Option<Preferences>,
) -> Result<OutlineOutcome, String> {
    crate::data::refuse_if_updating()?;
    let input = input.trim();
    refuse_empty(input, &genres, preferences.is_some())?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let messages = genesis::outline_messages(input, &genres, &lang, preferences.as_ref());
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
    crate::data::refuse_if_updating()?;
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
    title: String,
    outline_raw: String,
    table_kind: data::TableKind,
    preferences: Option<Preferences>,
    spoiler_lock: Option<bool>,
) -> Result<ExpandOutcome, String> {
    crate::data::refuse_if_updating()?;
    let input = input.trim();
    // 偏好只屬於 VN 桌：聊天桌帶了也不收
    let preferences = preferences.filter(|_| table_kind == data::TableKind::Vn);
    refuse_empty(input, &genres, preferences.is_some())?;
    // 桌名只認玩家草稿的標題：空白就在呼叫模型、寫檔之前擋下
    let title = genesis::draft_title(&title).ok_or_else(|| "EMPTY_TITLE".to_owned())?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let messages = genesis::expand_messages(
        input,
        &genres,
        title,
        &outline_raw,
        &lang,
        preferences.as_ref(),
    );
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
    let setup = preferences.map(|preferences| genesis::VnSetup {
        preferences,
        spoiler_lock: spoiler_lock.unwrap_or(true),
    });
    let world_id = genesis::parse_expand(&raw)
        .map(|expanded| {
            genesis::materialize(&data_root(&app)?, title, &expanded, table_kind, setup)
        })
        .transpose()
        .map_err(|error| error.to_string())?;
    // 開桌這幾次呼叫的額度就是為這桌花的：桌一建好就把剛才那些行認領回來（見 usage_log）
    if let (Some(id), Ok(root)) = (&world_id, data_root(&app)) {
        usage_log::assign_pending_world(&root.join("prompt-cache.jsonl"), id);
    }
    Ok(ExpandOutcome { world_id, raw })
}

#[cfg(test)]
mod tests {
    /// 聊天桌全空白仍拒絕；VN 精靈（帶偏好）全空白放行
    #[test]
    fn empty_input_refused_only_for_chat_tables() {
        assert_eq!(
            super::refuse_empty("", &[], false),
            Err("EMPTY_INPUT".to_owned())
        );
        assert!(super::refuse_empty("", &[], true).is_ok());
        assert!(super::refuse_empty("霧港", &[], false).is_ok());
    }
}
