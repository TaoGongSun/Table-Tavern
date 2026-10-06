use crate::data::TranscriptEvent;
use crate::transport::dispatch::{chat_transport, stream_via_transport};
use crate::transport::translate;
use crate::ui_msg::UiMsg;
use crate::{config_root, data, data_root, import, transport};
use serde::Serialize;

/// 前端落一則：事件沒帶快照就由後端補上目前檯面，補完的那份（含配發的 ID）回給前端——前端記憶體裡的
/// 事件從一開始就帶著快照，收回後復原才送得回當時的值。GM 回合的正文與變動紀錄帶 `turn_id`／`turn_part`
/// 冪等鍵（計畫 8.3）：同鍵重試回原事件，`main` 掛上回合算好的變數表。
#[tauri::command]
pub(crate) fn append_transcript(
    app: tauri::AppHandle,
    world_id: String,
    scene: u64,
    event: TranscriptEvent,
    turn_id: Option<String>,
    turn_part: Option<String>,
) -> Result<TranscriptEvent, String> {
    let _permit = data::world_write_permit(&world_id)?;
    let root = data_root(&app)?;
    let turn = match (turn_id, turn_part) {
        (Some(turn_id), Some(part)) => Some(data::message_vars::TurnKey { turn_id, part }),
        (None, None) => None,
        _ => return Err("turn_id 與 turn_part 要一起給".to_owned()),
    };
    data::append_event(&root, &world_id, scene, &event, turn.as_ref())
        .map(|(event, _)| event)
        .map_err(|error| error.to_string())
}

/// 玩家打字送出的那句：同 append_transcript，另回追加收據（這一行在檔裡的起始位元組），
/// AI 沒回成時憑它收回（discard_unanswered_player）。
#[derive(serde::Serialize)]
pub(crate) struct PlayerAppend {
    event: TranscriptEvent,
    offset: u64,
}

#[tauri::command]
pub(crate) fn append_player_event(
    app: tauri::AppHandle,
    world_id: String,
    scene: u64,
    event: TranscriptEvent,
) -> Result<PlayerAppend, String> {
    let _permit = data::world_write_permit(&world_id)?;
    let root = data_root(&app)?;
    let (event, offset) = data::append_event(&root, &world_id, scene, &event, None)
        .map_err(|error| error.to_string())?;
    Ok(PlayerAppend {
        event,
        offset: offset.unwrap_or_default(),
    })
}

/// 收回沒有回覆的玩家句：true＝確定已刪；false＝尾筆不是這句或結果不明，什麼都別假設。
#[tauri::command]
pub(crate) fn discard_unanswered_player(
    app: tauri::AppHandle,
    world_id: String,
    scene: u64,
    offset: u64,
    ts: String,
    text: String,
) -> Result<bool, String> {
    let _permit = data::world_write_permit(&world_id)?;
    data::discard_unanswered_player(&data_root(&app)?, &world_id, scene, offset, &ts, &text)
        .map_err(|error| error.to_string())
}

/// 貼開場白（見 import::post_opening_text）。opening_index 是玩家在開場白面板挑的那則（card_openings 的
/// 順序），import_source 是跳出這個面板的那次匯入的原檔識別（匯入結果回傳）；序號記在那筆匯入的收據上：重新重構時 expand 從原卡取同一則開場白當初始值依據。
/// 整段持整桌獨占：追加失敗時要把逐字稿與狀態寫回貼之前，中間不能有別的寫入，否則會被一起蓋掉。
#[tauri::command]
pub(crate) async fn post_opening(
    app: tauri::AppHandle,
    world_id: String,
    scene: u64,
    ts: String,
    text: String,
    opening_index: Option<usize>,
    import_source: Option<String>,
) -> Result<TranscriptEvent, String> {
    let held = data::world_exclusive_async(&world_id).await?;
    let root = data_root(&app)?;
    let config = data::read_config(&config_root(&app)?).unwrap_or_default();
    let lang = transport::ui_language(&config);
    import::post_opening_text(
        &root,
        &world_id,
        scene,
        &ts,
        &text,
        &lang,
        opening_index,
        import_source.as_deref(),
        &held,
    )
    .map_err(|error| error.to_string())
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
    crate::data::refuse_if_updating()?;
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
    let root = config_root(&app)?;
    let config = data::read_config(&root).map_err(|error| error.to_string())?;
    let kind = chat_transport(&config);
    // 智慧免費三檔送的是同一支，照實顯示，不顯示檔位裡留著的保底值
    let smart = (kind == "api" && crate::smart_free::is_active(&config))
        .then(|| crate::smart_free::status(&root, &config).model)
        .filter(|model| !model.is_empty());
    Ok([data::Tier::Fast, data::Tier::Balanced, data::Tier::Best]
        .into_iter()
        .map(|tier| {
            let mut entry = transport::tier_model(&config, &kind, tier);
            if let Some(model) = &smart {
                entry.effective_tier = entry.tier.clone();
                entry.model = Some(model.clone());
            }
            entry
        })
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
    let person_titles = data::appeared_person_titles(&events).into_iter().collect();
    let card_names = data::appeared_card_names(&events);
    let character_ids = data::list_characters(root, world_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|meta| {
            card_names
                .iter()
                .any(|name| data::name_matches(name, &meta.name))
        })
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
    let _permit = data::world_write_permit(&world_id)?;
    data::pop_transcript(&data_root(&app)?, &world_id, scene).map_err(|error| error.to_string())
}

// 存檔位置由前端的「另存新檔」對話框決定，這裡只負責產內容寫入該路徑
#[tauri::command]
pub(crate) fn export_transcript(
    app: tauri::AppHandle,
    world_id: String,
    path: String,
) -> Result<(), String> {
    crate::data::refuse_if_updating()?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let markdown = data::export_transcript_markdown(&data_root(&app)?, &world_id, &lang)
        .map_err(|error| error.to_string())?;
    // world-write-exempt: 寫到玩家選定的匯出路徑，不是桌目錄
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
    crate::data::refuse_if_updating()?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let markdown = data::export_scene_markdown(&data_root(&app)?, &world_id, scene, &lang)
        .map_err(|error| error.to_string())?;
    // world-write-exempt: 寫到玩家選定的匯出路徑，不是桌目錄
    std::fs::write(&path, markdown).map_err(|error| error.to_string())
}

/// 換場：把當前場景公開紀錄壓成一則摘要，寫進新場景開頭，current_scene +1（NewPlan 換場＋場景摘要）。
/// 摘要走既有 stream_via_transport＋GM 檔位，不新開連線路徑、不新增設定項。
#[tauri::command]
pub(crate) async fn advance_scene(app: tauri::AppHandle, world_id: String) -> Result<u64, String> {
    let _permit = data::world_write_permit_async(&world_id).await?;
    let root = data_root(&app)?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let state = data::read_state(&root, &world_id).map_err(|error| error.to_string())?;
    let events = data::read_transcript(&root, &world_id, state.current_scene)
        .map_err(|error| error.to_string())?;
    if events.is_empty() {
        return Err(UiMsg::SceneEmptyCannotAdvance.into());
    }

    let takeover = data::is_interface_takeover(&root, &world_id, state.refactor_mode.as_deref());
    let messages = transport::summary_messages(&events, &lang, takeover);
    let reply = stream_via_transport(
        &app,
        &config,
        None,
        false,
        transport::gm_tier(&config),
        Some(&world_id),
        "GM",
        transport::summary_closing(&lang),
        &messages,
        false,
        |_| {},
    )
    .await
    .map_err(summary_failure)?;

    // 換幕順手取幕名：回覆第一行「標題：…」／「Title: …」解析不到就整段當摘要，不報錯
    let (title, summary) = transport::extract_scene_title(&reply);
    data::begin_next_scene(&root, &world_id, &summary, title.as_deref())
        .map_err(|error| error.to_string())
}

/// 換幕摘要撞到容量上限：不能把聊天用的「請換幕」錯誤丟回去（按了只會重跑同一條路），
/// 改回摘要失敗的人話；原紀錄沒動過。其他錯誤原樣上拋。
fn summary_failure(error: String) -> String {
    if error.starts_with(transport::context_overflow::CODE) {
        UiMsg::SceneSummaryFailed.into()
    } else {
        error
    }
}

/// 退回前幕：換幕的精確反向操作，純本地檔案處理不必等模型回覆。
#[tauri::command]
pub(crate) fn revert_scene(app: tauri::AppHandle, world_id: String) -> Result<u64, String> {
    let _permit = data::world_write_permit(&world_id)?;
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
    let _permit = data::world_write_permit(&world_id)?;
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
    let _permit = data::world_write_permit_async(&world_id).await?;
    let root = data_root(&app)?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let state = data::read_state(&root, &world_id).map_err(|error| error.to_string())?;
    let scene = state.current_scene;
    let label = data::scene_label(&state, scene);
    let Some(previous_scene) = label.parent else {
        return Err(UiMsg::SummaryFirstScene.into());
    };
    if label.forked {
        return Err(UiMsg::SummaryContinuedScene.into());
    }
    let current_events =
        data::read_transcript(&root, &world_id, scene).map_err(|error| error.to_string())?;
    if current_events.len() != 1 {
        // 早退：這一幕已經有新內容，不值得先花一次模型呼叫才發現不能用
        return Err(UiMsg::SummaryHasNewContent.into());
    }

    let previous_events = data::read_transcript(&root, &world_id, previous_scene)
        .map_err(|error| error.to_string())?;
    if previous_events.is_empty() {
        return Err(UiMsg::PreviousSceneEmpty.into());
    }

    let takeover = data::is_interface_takeover(&root, &world_id, state.refactor_mode.as_deref());
    let messages = transport::summary_messages(&previous_events, &lang, takeover);
    let reply = stream_via_transport(
        &app,
        &config,
        None,
        false,
        transport::gm_tier(&config),
        Some(&world_id),
        "GM",
        transport::summary_closing(&lang),
        &messages,
        false,
        |_| {},
    )
    .await
    .map_err(summary_failure)?;

    let (title, summary) = transport::extract_scene_title(&reply);
    data::replace_scene_summary(&root, &world_id, &summary, title.as_deref())
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
                id: None,
                message_vars: None,
                vars_rev: None,
                vars_epoch: None,
                turn_key: None,
                ts: "now".to_owned(),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: data::TranscriptKind::System,
                text: "尾巴很大。".to_owned(),
                raw: None,
                state: None,
                truncated: false,
                gm_only: false,
                marker: Some(data::EventMarker::CardArrival {
                    name: "狐狸".to_owned(),
                }),
                opening: false,
            },
        )
        .unwrap();
        data::append_transcript(
            &root,
            &world_id,
            0,
            &data::TranscriptEvent {
                id: None,
                message_vars: None,
                vars_rev: None,
                vars_epoch: None,
                turn_key: None,
                ts: "now".to_owned(),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: data::TranscriptKind::System,
                text: "旅店老闆娘。".to_owned(),
                raw: None,
                state: None,
                truncated: false,
                gm_only: false,
                marker: Some(data::EventMarker::PersonArrival {
                    title: "愛麗絲".to_owned(),
                }),
                opening: false,
            },
        )
        .unwrap();

        let result = scene_appearances_at(&root, &world_id).unwrap();
        assert_eq!(result.character_ids, vec![fox.id.clone()]);
        assert_eq!(result.person_titles, vec!["愛麗絲".to_owned()]);

        std::fs::remove_dir_all(&root).unwrap();
    }

    /// 收回後復原要能把狀態欄帶回那一刻：前端畫面上的事件必須從落地那一刻就帶著快照，
    /// 所以 append 這一路要把補完的那份交回去；已經自帶快照的事件不准被目前檯面蓋掉。
    #[test]
    fn stamp_state_fills_in_current_table_but_keeps_a_carried_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "table-tavern-stamp-state-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let world_id = data::create_world(&root, "快照桌").unwrap();

        let mut world = data::read_state(&root, &world_id).unwrap();
        world
            .state
            .table
            .insert("時辰".to_owned(), "清晨".to_owned());
        data::write_state(&root, &world_id, &world).unwrap();

        let bare = data::TranscriptEvent {
            id: None,
            message_vars: None,
            vars_rev: None,
            vars_epoch: None,
            turn_key: None,
            ts: "now".to_owned(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: data::TranscriptKind::Narration,
            text: "天亮了。".to_owned(),
            raw: None,
            state: None,
            truncated: false,
            gm_only: false,
            marker: None,
            opening: false,
        };
        let (stamped, _) = data::append_event(&root, &world_id, 0, &bare, None).unwrap();
        assert_eq!(stamped.state.as_ref().unwrap().table["時辰"], "清晨");
        assert!(stamped.id.is_some());

        let mut carried = world.state.clone();
        carried.table.insert("時辰".to_owned(), "午夜".to_owned());
        let (kept, _) = data::append_event(
            &root,
            &world_id,
            0,
            &data::TranscriptEvent {
                state: Some(carried.clone()),
                ..bare
            },
            None,
        )
        .unwrap();
        assert_eq!(kept.state, Some(carried));

        std::fs::remove_dir_all(&root).unwrap();
    }
}
