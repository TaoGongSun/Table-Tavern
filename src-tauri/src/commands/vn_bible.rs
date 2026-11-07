//! VN 劇本聖經（plans/vn-mode-pkg4.md）的 Tauri 邊界：讀、生成／整份重骰、劇本分頁存檔、劇透鎖、角色秘密重骰。
use super::vn::GmOneshot;
use crate::data::vn::{Bible, VnFile};
use crate::vn_bible::{self, Revealed};
use crate::{config_root, data, data_root, transport};
use serde::Serialize;
use std::collections::BTreeMap;

/// 劇本分頁與精靈 ② 要的一切：檔（鎖、聖經）、旗標現值、已揭露清單、能不能重骰（未遊玩）。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BibleView {
    #[serde(flatten)]
    file: VnFile,
    flag_values: BTreeMap<String, String>,
    revealed: Vec<Revealed>,
    played: bool,
    /// 角色 id → 名字（秘密持有者顯示用；含玩家卡＝卡定主角）
    names: BTreeMap<String, String>,
    /// VN 卡匯入時存了封面（匯出成卡時不必再挑）
    cover: bool,
    /// 卡片變數模式的桌：匯出作品包不能附設定
    vars_mode: bool,
}

#[tauri::command]
pub(crate) fn vn_bible_read(app: tauri::AppHandle, world_id: String) -> Result<BibleView, String> {
    let root = data_root(&app)?;
    let state = data::read_state(&root, &world_id).map_err(|error| error.to_string())?;
    let mut file = data::vn::read_vn(&root, &world_id).map_err(|error| error.to_string())?;
    // 凍結副本只給注入用，前端不需要
    file.frozen = None;
    let flag_values = match state.state.tree.get("flags") {
        Some(data::StateNode::Branch(flags)) => flags
            .iter()
            .filter_map(|(id, node)| match node {
                data::StateNode::Leaf(value) => Some((id.clone(), value.clone())),
                data::StateNode::Branch(_) => None,
            })
            .collect(),
        _ => BTreeMap::new(),
    };
    Ok(BibleView {
        file,
        flag_values,
        revealed: vn_bible::revealed(&root, &world_id, &state)?,
        played: crate::refactor::played(&root, &world_id, &state)
            .map_err(|error| error.to_string())?,
        names: data::list_characters(&root, &world_id)
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|meta| (meta.id, meta.name))
            .chain(
                data::read_player_card(&root, &world_id)
                    .map_err(|error| error.to_string())?
                    .map(|card| (card.id, card.name)),
            )
            .collect(),
        cover: data::vn::cover_path(&root, &world_id)
            .map_err(|error| error.to_string())?
            .is_file(),
        vars_mode: data::message_vars::read_control(&root, &world_id)
            .map_err(|error| error.to_string())?
            .active(state.current_scene)
            .is_some(),
    })
}

async fn with_caller<T>(
    app: &tauri::AppHandle,
    world_id: &str,
    what: &'static str,
    work: impl AsyncFnOnce(&std::path::Path, &str, &mut GmOneshot<'_>) -> Result<T, String>,
) -> Result<T, String> {
    crate::data::refuse_if_updating()?;
    let _permit = data::world_write_permit_async(world_id).await?;
    let config = data::read_config(&config_root(app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let root = data_root(app)?;
    data::vn::refuse_if_replay(&root, world_id)?;
    let system = "Write the requested visual novel material exactly in the requested structure.";
    let mut caller = GmOneshot {
        app,
        config: &config,
        world_id,
        system,
        what,
    };
    work(&root, &lang, &mut caller).await
}

/// 整份生成或重骰（精靈 ② 首次、劇本分頁「重生成聖經」、遮罩「不看就重骰」）。`spoiler_lock` 只在建檔時用。
#[tauri::command]
pub(crate) async fn vn_bible_generate(
    app: tauri::AppHandle,
    world_id: String,
    spoiler_lock: Option<bool>,
    instruction: Option<String>,
) -> Result<(), String> {
    with_caller(&app, &world_id, "bible", async |root, lang, caller| {
        vn_bible::generate(root, &world_id, lang, spoiler_lock, instruction, caller).await
    })
    .await
}

/// 角色私設遮罩的「不看就重骰」：只重生這個角色的私設與他持有的秘密。
#[tauri::command]
pub(crate) async fn vn_secret_reroll(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<(), String> {
    with_caller(
        &app,
        &world_id,
        "secret reroll",
        async |root, lang, caller| {
            vn_bible::reroll_secrets(root, &world_id, &character_id, lang, caller).await
        },
    )
    .await
}

#[tauri::command]
pub(crate) fn vn_bible_save(
    app: tauri::AppHandle,
    world_id: String,
    bible: Bible,
) -> Result<(), String> {
    let _permit = data::world_write_permit(&world_id)?;
    vn_bible::save(&data_root(&app)?, &world_id, bible)
}

#[tauri::command]
pub(crate) fn vn_spoiler_set(
    app: tauri::AppHandle,
    world_id: String,
    locked: bool,
) -> Result<(), String> {
    let _permit = data::world_write_permit(&world_id)?;
    vn_bible::set_spoiler_lock(&data_root(&app)?, &world_id, locked)
}
