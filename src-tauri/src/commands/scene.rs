use crate::ai_transport::{chat_transport, stream_via_transport};
use crate::data::TranscriptEvent;
use crate::{config_root, data, data_root, mechanism, receipts, translate, transport};
use serde::Serialize;

#[tauri::command]
pub(crate) fn append_transcript(
    app: tauri::AppHandle,
    world_id: String,
    scene: u64,
    event: TranscriptEvent,
) -> Result<(), String> {
    data::append_transcript(&data_root(&app)?, &world_id, scene, &event)
        .map_err(|error| error.to_string())
}

/// 貼開場白＝GM 旁白，但狀態區塊要走與 GM 回覆同一條解析。剝除、併進檯面、事件帶快照
/// 三件事由這一個指令一次做完——前端各寫一半會破壞「目前值＝最後一則事件快照」的不變式。
#[tauri::command]
pub(crate) fn post_opening(
    app: tauri::AppHandle,
    world_id: String,
    scene: u64,
    ts: String,
    text: String,
) -> Result<TranscriptEvent, String> {
    let block = transport::extract_state_block(&text);
    let root = data_root(&app)?;
    let config = data::read_config(&config_root(&app)?).unwrap_or_default();
    let lang = transport::ui_language(&config);
    let player_name = data::read_player_card(&root, &world_id)
        .ok()
        .flatten()
        .map(|card| card.name);
    let user_name = player_name
        .as_deref()
        .unwrap_or_else(|| transport::player_fallback_name(&lang));
    let (event, outcome) =
        data::append_opening(&root, &world_id, scene, &ts, &text, &block, user_name)
            .map_err(|error| error.to_string())?;
    mechanism::append_log(&root, &world_id, scene, &outcome.records);
    // 掛到剛才那筆匯入收據上：復原匯入時這則開場白要跟著收掉，
    // 不然重匯同一張卡想改挑一則時，舊的那則還壓在開局上
    receipts::record_posted_opening(&root, &world_id, scene, &ts);
    Ok(event)
}

/// 開場白翻譯：選擇視窗按下「翻譯」時呼叫，把單則開場白譯成玩家語言方便挑選、貼出。
/// 一律走 fast 檔（單則翻譯用不到 GM 檔的推理力，要點 3）；API 模式沒設定 fast 模型時
/// 退回 GM 檔，讓按鈕在任何設定下都能用。lang 由前端帶入（玩家介面語言），這裡不再另外查。
#[tauri::command]
pub(crate) async fn translate_opening(
    app: tauri::AppHandle,
    world_id: String,
    text: String,
    lang: String,
    tier: Option<String>,
) -> Result<String, String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let messages = translate::opening_messages(&text, &lang);
    // 檔位由開場白視窗的挑選器帶來（省額度預設低檔，翻不出來的玩家自己調高再重翻）；
    // 沒帶＝維持舊行為的低檔。未知值 fail-closed，不默默降級成別的檔位。
    let requested = match tier.as_deref() {
        None => data::Tier::Fast,
        Some(value) => data::Tier::parse(value).map_err(|error| error.to_string())?,
    };
    let tier = if chat_transport(&config) == "api"
        && transport::resolve_model(requested, &config).is_err()
    {
        transport::gm_tier(&config)
    } else {
        requested
    };
    let raw = stream_via_transport(
        &app,
        &config,
        None,
        false,
        tier,
        Some(&world_id),
        "GM",
        "Output only the translated text itself, nothing else.",
        &messages,
        false,
        |_| {},
    )
    .await?;
    Ok(raw.trim().to_owned())
}

/// 開場白視窗的檔位挑選器選項：低／中／高各自實際會叫的模型，解析與真正送出時同源。
/// 玩家看得到「低檔＝claude-haiku-4-5」，拒譯時才知道要往上調哪一檔（同一家的不同世代
/// 對同樣內容的容忍度不一樣，只顯示「sonnet」分不出 4.6 與 5）。
#[tauri::command]
pub(crate) fn translate_tier_models(
    app: tauri::AppHandle,
) -> Result<Vec<transport::TierModel>, String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let kind = chat_transport(&config);
    Ok([data::Tier::Fast, data::Tier::Balanced, data::Tier::Best]
        .into_iter()
        .map(|tier| transport::tier_model(&config, &kind, tier))
        .collect())
}

#[tauri::command]
pub(crate) fn read_transcript(
    app: tauri::AppHandle,
    world_id: String,
    scene: u64,
) -> Result<Vec<TranscriptEvent>, String> {
    data::read_transcript(&data_root(&app)?, &world_id, scene).map_err(|error| error.to_string())
}

/// 這一幕已經出場的角色卡與世界書人物（AI 卡重構包 4b）：前端載入時用來初始化本地分區，
/// 不必自己重掃 transcript 猜前綴。卡登場記的是名字，這裡拿現有卡清單反查回 id。
#[derive(Serialize)]
pub(crate) struct SceneAppearances {
    character_ids: Vec<String>,
    person_titles: Vec<String>,
}

fn scene_appearances_at(
    root: &std::path::Path,
    world_id: &str,
) -> Result<SceneAppearances, String> {
    let state = data::read_state(root, world_id).map_err(|error| error.to_string())?;
    let events = data::read_transcript(root, world_id, state.current_scene)
        .map_err(|error| error.to_string())?;
    let person_titles = transport::appeared_person_titles(&events).into_iter().collect();
    let card_names = data::appeared_titles(&events, data::CARD_ARRIVAL_PREFIX);
    let character_ids = data::list_characters(root, world_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|meta| card_names.iter().any(|name| data::name_matches(name, &meta.name)))
        .map(|meta| meta.id)
        .collect();
    Ok(SceneAppearances {
        character_ids,
        person_titles,
    })
}

#[tauri::command]
pub(crate) fn scene_appearances(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<SceneAppearances, String> {
    scene_appearances_at(&data_root(&app)?, &world_id)
}

// 收回上一句：只砍當前這一幕的最後一筆，可連按；回傳 false＝這一幕已經收乾淨了
#[tauri::command]
pub(crate) fn pop_transcript(
    app: tauri::AppHandle,
    world_id: String,
    scene: u64,
) -> Result<bool, String> {
    data::pop_transcript(&data_root(&app)?, &world_id, scene).map_err(|error| error.to_string())
}

// 存檔位置由前端的「另存新檔」對話框決定，這裡只負責產內容寫入該路徑
#[tauri::command]
pub(crate) fn export_transcript(
    app: tauri::AppHandle,
    world_id: String,
    path: String,
) -> Result<(), String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let markdown = data::export_transcript_markdown(&data_root(&app)?, &world_id, &lang)
        .map_err(|error| error.to_string())?;
    std::fs::write(&path, markdown).map_err(|error| error.to_string())
}

// 單場匯出：格式與 export_transcript 一致，但只匯一場，供「過去的場」單場檢視使用
#[tauri::command]
pub(crate) fn export_scene(
    app: tauri::AppHandle,
    world_id: String,
    scene: u64,
    path: String,
) -> Result<(), String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let markdown = data::export_scene_markdown(&data_root(&app)?, &world_id, scene, &lang)
        .map_err(|error| error.to_string())?;
    std::fs::write(&path, markdown).map_err(|error| error.to_string())
}

/// 換場：把當前場景公開紀錄壓成一則摘要，寫進新場景開頭，current_scene +1（NewPlan 換場＋場景摘要）。
/// 摘要走既有 stream_via_transport＋GM 檔位，不新開連線路徑、不新增設定項。
#[tauri::command]
pub(crate) async fn advance_scene(app: tauri::AppHandle, world_id: String) -> Result<u64, String> {
    let root = data_root(&app)?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let state = data::read_state(&root, &world_id).map_err(|error| error.to_string())?;
    let events = data::read_transcript(&root, &world_id, state.current_scene)
        .map_err(|error| error.to_string())?;
    if events.is_empty() {
        return Err("這個場景還沒有任何紀錄，沒東西可以換場".to_owned());
    }

    let messages = transport::summary_messages(&events, &lang);
    let reply = stream_via_transport(
        &app,
        &config,
        None,
        false,
        transport::gm_tier(&config),
        Some(&world_id),
        "GM",
        "現在請執行上述導演指示，只輸出摘要本文，不要加名字前綴。",
        &messages,
        false,
        |_| {},
    )
    .await?;

    // 換幕順手取幕名：回覆第一行「標題：…」／「Title: …」解析不到就整段當摘要，不報錯
    let (title, summary) = transport::extract_scene_title(&reply);
    data::begin_next_scene(&root, &world_id, &summary, &lang, title.as_deref())
        .map_err(|error| error.to_string())
}

/// 退回前幕：換幕的精確反向操作，純本地檔案處理不必等模型回覆。
#[tauri::command]
pub(crate) fn revert_scene(app: tauri::AppHandle, world_id: String) -> Result<u64, String> {
    let root = data_root(&app)?;
    data::revert_scene(&root, &world_id).map_err(|error| error.to_string())
}

/// 從前幕分岔：把那一幕的紀錄複製成新的一幕接著玩，純本地檔案處理不必等模型回覆。
#[tauri::command]
pub(crate) fn fork_scene(
    app: tauri::AppHandle,
    world_id: String,
    scene: u64,
) -> Result<u64, String> {
    let root = data_root(&app)?;
    data::fork_scene(&root, &world_id, scene).map_err(|error| error.to_string())
}

/// 重寫前情提要：結構照 advance_scene，差別是摘要對象換成「前一幕」的紀錄，
/// 換出來的文字覆寫目前這幕既有的那則摘要，而不是開一個新場景。
#[tauri::command]
pub(crate) async fn regenerate_scene_summary(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<(), String> {
    let root = data_root(&app)?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let state = data::read_state(&root, &world_id).map_err(|error| error.to_string())?;
    let scene = state.current_scene;
    let label = data::scene_label(&state, scene);
    let Some(previous_scene) = label.parent else {
        return Err("第一幕沒有前情提要可以重寫".to_owned());
    };
    if label.forked {
        return Err("這一幕是從前幕接續來的，開頭不是前情提要".to_owned());
    }
    let current_events =
        data::read_transcript(&root, &world_id, scene).map_err(|error| error.to_string())?;
    if current_events.len() != 1 {
        // 早退：這一幕已經有新內容，不值得先花一次模型呼叫才發現不能用
        return Err("這一幕已經有新內容，不能重寫前情提要".to_owned());
    }

    let previous_events = data::read_transcript(&root, &world_id, previous_scene)
        .map_err(|error| error.to_string())?;
    if previous_events.is_empty() {
        return Err("前一幕還沒有任何紀錄，沒東西可以重新摘要".to_owned());
    }

    let messages = transport::summary_messages(&previous_events, &lang);
    let reply = stream_via_transport(
        &app,
        &config,
        None,
        false,
        transport::gm_tier(&config),
        Some(&world_id),
        "GM",
        "現在請執行上述導演指示，只輸出摘要本文，不要加名字前綴。",
        &messages,
        false,
        |_| {},
    )
    .await?;

    let (title, summary) = transport::extract_scene_title(&reply);
    data::replace_scene_summary(&root, &world_id, &summary, &lang, title.as_deref())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::scene_appearances_at;
    use crate::commands::{character_card, NEXT_TEMP_ID};
    use crate::data;
    use std::sync::atomic::Ordering;

    /// AI 卡重構包 4b：scene_appearances 掃現在這幕的 transcript，角色卡回歸事件反查回 id，
    /// 世界書人物登場事件直接回 title；兩種前綴互不干擾。
    #[test]
    fn scene_appearances_at_scans_both_prefixes() {
        let root = std::env::temp_dir().join(format!(
            "table-tavern-scene-appearances-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let world_id = data::create_world(&root, "測試桌").unwrap();

        let fox = character_card(&data::new_id(), "狐狸");
        data::write_character(&root, &world_id, &fox).unwrap();

        data::append_transcript(
            &root,
            &world_id,
            0,
            &data::TranscriptEvent {
                ts: "now".to_owned(),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: data::TranscriptKind::System,
                text: "（角色回歸）〈狐狸〉\n尾巴很大。".to_owned(),
                raw: None,
                state: None,
                gm_only: false,
            },
        )
        .unwrap();
        data::append_transcript(
            &root,
            &world_id,
            0,
            &data::TranscriptEvent {
                ts: "now".to_owned(),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: data::TranscriptKind::System,
                text: "（人物登場）〈愛麗絲〉\n旅店老闆娘。".to_owned(),
                raw: None,
                state: None,
                gm_only: false,
            },
        )
        .unwrap();

        let result = scene_appearances_at(&root, &world_id).unwrap();
        assert_eq!(result.character_ids, vec![fox.id.clone()]);
        assert_eq!(result.person_titles, vec!["愛麗絲".to_owned()]);

        std::fs::remove_dir_all(&root).unwrap();
    }
}
