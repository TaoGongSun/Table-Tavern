use crate::ai_transport::{
    ai_call_failure, chat_transport, lane_provider, prepare_lane_call, stream_turn_via_transport,
};
use crate::commands::character::load_active_cards;
use crate::{config_root, data, data_root, import, lanes, mechanism, transport, usage_log};
use serde::Serialize;

/// 上下文組裝→單發呼叫→串流回傳（KICKOFF §4）。
/// 上下文完全由本機正典（角色卡＋可見世界書＋公開 transcript）經 assemble_messages 組裝，
/// 再依 preferences.transport 分流到 API 或 CLI；增量文字經 on_delta channel 回前端。
#[tauri::command]
pub(crate) async fn chat_with_character(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    on_delta: tauri::ipc::Channel<String>,
) -> Result<String, String> {
    let root = data_root(&app)?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let card =
        data::read_character(&root, &world_id, &character_id).map_err(|error| error.to_string())?;
    let state = data::read_state(&root, &world_id).map_err(|error| error.to_string())?;
    let events = data::read_transcript(&root, &world_id, state.current_scene)
        .map_err(|error| error.to_string())?;
    let worldbook = data::read_worldbook(&root, &world_id).map_err(|error| error.to_string())?;

    let player = data::read_player_card(&root, &world_id).map_err(|error| error.to_string())?;
    // 角色自己那支的狀態（面板指認優先，其次同名比對），只有這條分支會塞進提示詞。
    let branch = transport::resolve_branch(
        &state.state.tree,
        &state.branch_bindings,
        &character_id,
        &card.name,
    );
    let emit = |delta: &str| {
        let _ = on_delta.send(delta.to_owned());
    };
    // claude／grok 訂閱走 resume 續聊線。claude 全角色共用一條 session，私設回合注入、
    // 回合後從 session 檔抹掉（案 C）；grok 沒有抹寫路徑，改成一角一線＋私設提進該角色
    // 自己的凍結 system（grok-cache-miss），兩邊都不讓別的角色讀到不該讀的東西。
    if let Some(provider) = lane_provider(&config) {
        let hoist = provider == lanes::LaneProvider::Grok;
        let lang = transport::ui_language(&config);
        let cards = load_active_cards(&root, &world_id)?;
        let mut frozen = transport::chars_lane_system(&cards, player.as_ref(), &worldbook, &lang);
        let turn = transport::chars_lane_turn(
            &card,
            player.as_ref(),
            &events,
            &worldbook,
            &state.state,
            &state.mechanism,
            branch.as_deref(),
            &lang,
            hoist,
        );
        if let Some(private) = &turn.hoisted_private {
            frozen.push('\n');
            frozen.push_str(private);
        }
        let call = prepare_lane_call(&app, &config, card.tier, provider).await?;
        return lanes::run_turn(
            &call,
            &root,
            &world_id,
            lanes::TurnInput {
                lane: lanes::Lane::Chars,
                scene: state.current_scene,
                events: &events,
                frozen_system: frozen,
                tail: turn.tail,
                confidential: (!hoist).then_some(turn.confidential).flatten(),
                prefix: (!hoist).then(|| format!("{}：", card.name)),
                echo: lanes::ReplyEcho::Dialogue {
                    speaker_id: card.id.clone(),
                },
                scope: hoist.then(|| card.id.clone()),
            },
            emit,
        )
        .await
        .map_err(ai_call_failure);
    }
    // api／codex／agy／grok 走共線組裝（api-shared-lane 包 B）：全角色共用一份與「這輪是誰」
    // 無關的前綴，本輪指定在尾端那則 user。attendant_label 與 closing 傳空字串，因為這份
    // messages 已經自足——台詞自帶名字前綴、指示已在尾端，補了會重複（見 cli::flatten_messages）。
    let cards = load_active_cards(&root, &world_id)?;
    let messages = transport::assemble_shared_messages(
        &card,
        &cards,
        player.as_ref(),
        &events,
        &worldbook,
        &state.state,
        &state.mechanism,
        branch.as_deref(),
        &transport::ui_language(&config),
    );
    // roster 記的是套用策略前的有效角色數，不是實際帶進組裝器的張數——沒有這個數字，
    // 日後零命中退回（no-cache-model-optout）產生的 solo 就跟天然單角色桌長得一樣
    stream_turn_via_transport(
        &app,
        &config,
        None,
        false,
        card.tier,
        Some(&world_id),
        "",
        "",
        &messages,
        usage_log::PromptShape::Turn {
            roster: cards.len(),
            solo: cards.len() <= 1,
        },
        false,
        emit,
    )
    .await
}

/// 這一桌自動隱藏、且未手動封存的角色卡（回合登場檢測用，AI 卡重構包 4b）；
/// 與 load_active_cards 互補，present 名單命中就是「回歸」。
fn load_hidden_cards(
    root: &std::path::Path,
    world_id: &str,
) -> Result<Vec<data::CharacterCard>, String> {
    data::list_characters(root, world_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|meta| meta.auto_hidden && !meta.archived)
        .map(|meta| {
            data::read_character(root, world_id, &meta.id).map_err(|error| error.to_string())
        })
        .collect()
}

/// GM 上下文素材＝world.md＋世界書＋全部角色卡（含私有）＋公開 transcript（NewPlan §7.0）。
/// 單發組裝與 claude lane 續聊共用同一份素材。
struct GmMaterials {
    world_md: String,
    worldbook: Vec<data::WorldbookEntry>,
    state: data::WorldState,
    events: Vec<data::TranscriptEvent>,
    cards: Vec<data::CharacterCard>,
    player: Option<data::CharacterCard>,
}

fn gm_materials(root: &std::path::Path, world_id: &str) -> Result<GmMaterials, String> {
    let state = data::read_state(root, world_id).map_err(|error| error.to_string())?;
    let events = data::read_transcript(root, world_id, state.current_scene)
        .map_err(|error| error.to_string())?;
    Ok(GmMaterials {
        world_md: data::read_world_md(root, world_id).map_err(|error| error.to_string())?,
        worldbook: data::read_worldbook(root, world_id).map_err(|error| error.to_string())?,
        state,
        events,
        cards: load_active_cards(root, world_id)?,
        player: data::read_player_card(root, world_id).map_err(|error| error.to_string())?,
    })
}

/// GM lane 的一輪：凍結 system（GM 指示＋world.md＋全 constant＋全卡）＋回合尾段
/// （keyword 條目＋狀態＋導演指示）。narrate 與 suggest 共用，差別只在指示與 echo。
#[allow(clippy::too_many_arguments)]
async fn gm_lane_reply(
    app: &tauri::AppHandle,
    config: &data::AppConfig,
    root: &std::path::Path,
    world_id: &str,
    materials: &GmMaterials,
    scope: &transport::StateScope,
    instruction: &str,
    echo: lanes::ReplyEcho,
    lang: &str,
    provider: lanes::LaneProvider,
    emit: impl FnMut(&str),
) -> Result<String, String> {
    let frozen = transport::gm_lane_system(
        &materials.world_md,
        &materials.cards,
        materials.player.as_ref(),
        &materials.worldbook,
        &materials.state.mechanism,
        lang,
    );
    let turn = transport::gm_lane_turn(
        &materials.events,
        &materials.worldbook,
        materials.player.as_ref(),
        &materials.state.state,
        &materials.state.mechanism,
        scope,
        instruction,
        lang,
    );
    let call = prepare_lane_call(app, config, transport::gm_tier(config), provider).await?;
    lanes::run_turn(
        &call,
        root,
        world_id,
        lanes::TurnInput {
            lane: lanes::Lane::Gm,
            scene: materials.state.current_scene,
            events: &materials.events,
            frozen_system: frozen,
            tail: turn.tail,
            confidential: None,
            prefix: None,
            echo,
            scope: None, // GM 只有一條線，不細分
        },
        emit,
    )
    .await
    .map_err(ai_call_failure)
}

/// gm_narrate 回傳：剝乾淨的旁白顯示文字＋下一位發言者（角色 id 或玩家哨兵）。
/// GM 沒點名或名字對不上名單＝None，前端就地停下、不當錯誤。
#[derive(Serialize)]
pub(crate) struct GmNarration {
    text: String,
    /// 剝殼前的模型原文；與 text 相同就是 None，前端照樣存進事件裡
    raw: Option<String>,
    next: Option<String>,
    /// 長文字欄這一輪的新值；前端接到就補一則 system 事件進 transcript。空的就不補。
    state_updates: Vec<StateUpdate>,
    /// 這輪剛回歸（登場）的角色卡 id（AI 卡重構包 4b）；前端拿它把卡從隱藏區搬回主區。
    #[serde(default)]
    arrived_characters: Vec<String>,
    /// 這輪剛登場的世界書人物 title（4a 就有登場事件，4b 才在回傳裡帶出來）。
    #[serde(default)]
    arrived_persons: Vec<String>,
}

#[derive(Serialize)]
struct StateUpdate {
    path: String,
    value: String,
}

/// 簡易導演：GM 旁白＋點名一次呼叫完成（NewPlan §6.1＋快取包 5）。
/// 旁白串流回前端後由前端落 transcript；點名行與狀態欄在這裡剝掉，不進顯示文字。
#[tauri::command]
pub(crate) async fn gm_narrate(
    app: tauri::AppHandle,
    world_id: String,
    on_delta: tauri::ipc::Channel<String>,
) -> Result<GmNarration, String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let root = data_root(&app)?;
    let materials = gm_materials(&root, &world_id)?;
    let roster: Vec<String> = materials
        .cards
        .iter()
        .map(|card| card.name.clone())
        .collect();
    let player_name = materials.player.as_ref().map(|card| card.name.as_str());
    // 換幕後第一輪 GM 回合送整棵樹對齊；之後每輪只送在場分支＋變動標記（狀態欄二期包 5）。
    let align = materials.state.mechanism.incremental
        && materials.state.aligned_scene != Some(materials.state.current_scene);
    let scope = transport::state_scope(
        &materials.state.state,
        &materials.state.mechanism,
        &materials.cards,
        materials.player.as_ref(),
        &materials.state.branch_bindings,
        align,
    );
    // 卡片自帶介面時，卡片自己規定了輸出格式，導演指示要讓路，否則模型會照我們的旁白規矩寫，介面永遠對不上。
    let card_scripts: Vec<import::InterfaceScript> = import::read_card_interfaces(&root, &world_id)
        .unwrap_or_default()
        .into_iter()
        .filter(|interface| interface.unsupported.is_none())
        .flat_map(|interface| interface.scripts)
        .collect();
    let (instruction_message, closing) = if card_scripts.is_empty() {
        let instruction_message = transport::narrate_instruction(&lang, &roster, player_name);
        let closing = if roster.is_empty() {
            "現在請以 GM 身分執行上述導演指示，只輸出旁白本文與要求的狀態欄，不要加名字前綴。"
        } else {
            "現在請以 GM 身分執行上述導演指示，只輸出旁白本文、要求的狀態欄與「下一位」行，不要加名字前綴。"
        };
        (instruction_message, closing)
    } else {
        let entry_title = import::card_format_entry(&card_scripts, &materials.worldbook);
        let instruction_message = transport::card_format_instruction(&lang, entry_title.as_deref());
        let closing =
            "現在請以 GM 身分，完全依照上述輸出格式產生本回合的回覆，不要加名字前綴，也不要輸出格式以外的任何內容。";
        (instruction_message, closing)
    };
    let emit = |delta: &str| {
        let _ = on_delta.send(delta.to_owned());
    };
    let reply = if let Some(provider) = lane_provider(&config) {
        let instruction = format!("{}\n{closing}", instruction_message.content);
        gm_lane_reply(
            &app,
            &config,
            &root,
            &world_id,
            &materials,
            &scope,
            &instruction,
            lanes::ReplyEcho::Narration,
            &lang,
            provider,
            emit,
        )
        .await?
    } else {
        let mut messages = transport::assemble_gm_messages(
            &materials.world_md,
            &materials.cards,
            materials.player.as_ref(),
            &materials.events,
            &materials.worldbook,
            &materials.state.state,
            &materials.state.mechanism,
            &scope,
            &lang,
        );
        messages.push(instruction_message);
        // GM 上下文一律全卡，與「這輪誰說話」無關，形狀恆為共線
        stream_turn_via_transport(
            &app,
            &config,
            None,
            false,
            transport::gm_tier(&config),
            Some(&world_id),
            "GM",
            closing,
            &messages,
            usage_log::PromptShape::Turn {
                roster: materials.cards.len(),
                solo: false,
            },
            false,
            emit,
        )
        .await?
    };
    let block = transport::extract_state_block(&reply);
    let (next_raw, display) = transport::extract_next_speaker(&block.display);
    // 剝掉 state／next 控制欄後沒有正文＝這一輪沒東西寫進故事。必須擋在下面的
    // apply_block 之前：狀態套用只要 incremental 為真就必跑，失敗回合會白白重擲一輪骰
    // （stream-failure-visible）。CLI 那條路沒有 stream_chat 的收工判定，這裡是唯一防線。
    if display.trim().is_empty() {
        return Err(format!(
            "AI_EMPTY_RESPONSE: 剝除控制欄後沒有正文 raw_len={}",
            reply.chars().count()
        ));
    }
    let mut state_updates = Vec::new();
    let mut arrived_persons = Vec::new();
    let mut arrived_characters = Vec::new();
    // 狀態更新一律盡力而為：模型格式壞掉或存檔寫不進去，都不該害玩家丟掉整段旁白。
    // 骰值要每回合重擲，就算這一輪模型完全沒吐更新也要跑一次。
    if !block.fields.is_empty()
        || !block.updates.is_empty()
        || materials.state.mechanism.incremental
    {
        let user_name = player_name.unwrap_or_else(|| transport::player_fallback_name(&lang));
        if let Ok(mut state) = data::read_state(&root, &world_id) {
            let scene = state.current_scene;
            let outcome = mechanism::apply_block(&mut state, &block, user_name);
            if align {
                state.aligned_scene = Some(scene);
            }
            if data::write_state(&root, &world_id, &state).is_ok() {
                mechanism::append_log(&root, &world_id, scene, &outcome.records);
                state_updates =
                    transport::snapshot_updates(&state.state, &state.mechanism, user_name)
                        .into_iter()
                        .map(|(path, value)| StateUpdate { path, value })
                        .collect();
                let present = state.state.table.get("present").map(String::as_str);
                // 人物在場登場（AI 卡重構包 4a）：present 套用後檢查新面孔，
                // 命中就把世界書全文記進歷史，system 那邊只留一行名冊。
                arrived_persons = record_person_arrivals(
                    &root,
                    &world_id,
                    scene,
                    &materials.worldbook,
                    &materials.events,
                    present,
                    &display,
                    user_name,
                );
                // 角色卡自動回歸（AI 卡重構包 4b）：鏡射人物登場，鍵換成卡名；
                // auto_hidden 欄位本身不在這裡動，只在換幕結算（data::begin_next_scene）。
                if let Ok(hidden_cards) = load_hidden_cards(&root, &world_id) {
                    arrived_characters = record_card_arrivals(
                        &root,
                        &world_id,
                        scene,
                        &hidden_cards,
                        &materials.events,
                        present,
                        &display,
                        user_name,
                    );
                }
            }
        }
    }
    // LLM 只認名字，點名後對回角色 id（同名取第一個）；玩家哨兵原樣回傳
    let next = next_raw
        .and_then(|raw| transport::pick_speaker(&raw, &roster, player_name))
        .and_then(|picked| {
            if picked == transport::PLAYER_SENTINEL {
                return Some(picked);
            }
            materials
                .cards
                .iter()
                .find(|card| card.name == picked)
                .map(|card| card.id.clone())
        });
    Ok(GmNarration {
        raw: (reply != display).then_some(reply),
        text: display,
        next,
        state_updates,
        arrived_characters,
        arrived_persons,
    })
}

/// 世界書人物條目首次在場（AI 卡重構包 4a）：present 名單（缺席就退回本文比對）比對得上、
/// 本幕還沒登場過的 is_person 條目，逐一把全文 append 成一則系統事件；同一人本幕只記一次，
/// 離場不拔。寫檔失敗一律吞掉，登場記錄不該反過來中斷旁白。回傳這輪實際記上的標題清單
/// （成功寫檔才算），供 gm_narrate 回傳給前端本地移區。
/// visibility 非 Public 的條目帶 gm_only=true（包 4b）：這種條目原本就限定 GM 或特定角色
/// 看得到，全文登場事件不能透過 chars 續聊線的共用歷史洩漏給所有角色。
#[allow(clippy::too_many_arguments)]
fn record_person_arrivals(
    root: &std::path::Path,
    world_id: &str,
    scene: u64,
    worldbook: &[data::WorldbookEntry],
    events: &[data::TranscriptEvent],
    present: Option<&str>,
    reply_body: &str,
    user_name: &str,
) -> Vec<String> {
    let already = transport::appeared_person_titles(events);
    let arrivals = transport::detect_new_arrivals(worldbook, present, reply_body, &already);
    if arrivals.is_empty() {
        return Vec::new();
    }
    let ts = data::local_timestamp().unwrap_or_default();
    let mut titles = Vec::new();
    for entry in arrivals {
        let event = data::TranscriptEvent {
            ts: ts.clone(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: data::TranscriptKind::System,
            text: transport::person_arrival_text(entry, user_name),
            raw: None,
            state: None,
            gm_only: !matches!(entry.visibility, data::Visibility::Public),
        };
        if data::append_transcript(root, world_id, scene, &event).is_ok() {
            titles.push(entry.title.clone());
        }
    }
    titles
}

/// 角色卡自動回歸（AI 卡重構包 4b）：present 名單（缺席就退回本文比對）比對得上、
/// 本幕還沒回歸過的 auto_hidden 卡，逐一把完整設定 append 成一則系統事件；同一張卡
/// 本幕只記一次。鏡射 record_person_arrivals，鍵從世界書 title 換成卡片 name；
/// **不改 auto_hidden 欄位本身**（鐵律：持久欄位只在換幕結算，見 data::begin_next_scene）。
/// chars 快照本來就含全卡，回歸事件不算新洩漏，一律 gm_only=false。
/// 回傳這輪實際記上的卡 id 清單（成功寫檔才算）。
#[allow(clippy::too_many_arguments)]
fn record_card_arrivals(
    root: &std::path::Path,
    world_id: &str,
    scene: u64,
    hidden_cards: &[data::CharacterCard],
    events: &[data::TranscriptEvent],
    present: Option<&str>,
    reply_body: &str,
    user_name: &str,
) -> Vec<String> {
    let already = transport::appeared_card_names(events);
    let arrivals = transport::detect_new_card_arrivals(hidden_cards, present, reply_body, &already);
    if arrivals.is_empty() {
        return Vec::new();
    }
    let ts = data::local_timestamp().unwrap_or_default();
    let mut ids = Vec::new();
    for card in arrivals {
        let event = data::TranscriptEvent {
            ts: ts.clone(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: data::TranscriptKind::System,
            text: transport::card_arrival_text(card, user_name),
            raw: None,
            state: None,
            gm_only: false,
        };
        if data::append_transcript(root, world_id, scene, &event).is_ok() {
            ids.push(card.id.clone());
        }
    }
    ids
}

/// 保溫 ping（包 7）：玩家還在、快取快到期時由前端呼叫，替這桌每條活著的線刷新五分鐘壽命。
/// 回傳實際保溫的線數——claude 以外的傳輸、或這桌還沒開過線時回 0，前端據此不再重試。
#[tauri::command]
pub(crate) async fn keepalive_lanes(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<usize, String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    if chat_transport(&config) != "claude" {
        return Ok(0);
    }
    let root = data_root(&app)?;
    let call = prepare_lane_call(
        &app,
        &config,
        transport::gm_tier(&config),
        lanes::LaneProvider::Claude,
    )
    .await?;
    lanes::keepalive(&call, &root, &world_id).await
}

#[cfg(test)]
mod tests {
    use super::{record_card_arrivals, record_person_arrivals};
    use crate::commands::{character_card, NEXT_TEMP_ID};
    use crate::data;
    use std::sync::atomic::Ordering;

    /// AI 卡重構包 4a 規格 (c)(d)(e)：present 有新面孔就把世界書全文 append 成一則系統事件；
    /// 同一幕重複比對不重複 append；換幕（新場景號、空 events）同名要重新 append 一次。
    #[test]
    fn record_person_arrivals_appends_once_per_scene_and_resets_on_new_scene() {
        let root = std::env::temp_dir().join(format!(
            "table-tavern-person-arrivals-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let world_id = data::create_world(&root, "測試桌").unwrap();

        let alice = data::WorldbookEntry {
            uid: 1,
            title: "愛麗絲".to_owned(),
            keys: Vec::new(),
            content: "愛麗絲是旅店老闆娘。".to_owned(),
            constant: true,
            order: 0,
            disabled: false,
            visibility: data::Visibility::Public,
            is_person: true,
            locked: false,
        };
        let worldbook = [alice];

        // 第一輪：present 有愛麗絲 → append 一則登場事件
        record_person_arrivals(&root, &world_id, 0, &worldbook, &[], Some("愛麗絲"), "", "阿濤");
        let scene0 = data::read_transcript(&root, &world_id, 0).unwrap();
        assert_eq!(scene0.len(), 1);
        assert_eq!(scene0[0].kind, data::TranscriptKind::System);
        assert_eq!(scene0[0].speaker_id, "");
        assert_eq!(scene0[0].speaker_name, "GM");
        assert!(scene0[0].text.starts_with("（人物登場）〈愛麗絲〉\n"));
        assert!(scene0[0].text.contains("愛麗絲是旅店老闆娘。"));

        // 第二輪：present 還是愛麗絲，本幕 events 已含前一則登場事件 → 不重複
        record_person_arrivals(
            &root, &world_id, 0, &worldbook, &scene0, Some("愛麗絲"), "", "阿濤",
        );
        assert_eq!(data::read_transcript(&root, &world_id, 0).unwrap().len(), 1);

        // 換幕：scene 1 是新 jsonl、events 是空的 → 同名重新 append
        record_person_arrivals(&root, &world_id, 1, &worldbook, &[], Some("愛麗絲"), "", "阿濤");
        let scene1 = data::read_transcript(&root, &world_id, 1).unwrap();
        assert_eq!(scene1.len(), 1);
        assert!(scene1[0].text.starts_with("（人物登場）〈愛麗絲〉\n"));

        std::fs::remove_dir_all(&root).unwrap();
    }

    /// AI 卡重構包 4b，鏡射 4a：present 有隱藏卡的名字就把完整設定 append 成一則回歸事件；
    /// 同一幕重複比對不重複 append；不改 auto_hidden 欄位本身（鐵律，換幕才結算）。
    #[test]
    fn record_card_arrivals_appends_once_per_scene_and_does_not_touch_auto_hidden() {
        let root = std::env::temp_dir().join(format!(
            "table-tavern-card-arrivals-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let world_id = data::create_world(&root, "測試桌").unwrap();

        let mut fox = character_card(&data::new_id(), "狐狸");
        fox.public_md = "尾巴很大。".to_owned();
        data::write_character(&root, &world_id, &fox).unwrap();
        data::set_character_auto_hidden(&root, &world_id, &fox.id, true).unwrap();
        let hidden_cards = vec![fox.clone()];

        // 第一輪：present 有狐狸 → append 一則回歸事件
        record_card_arrivals(&root, &world_id, 0, &hidden_cards, &[], Some("狐狸"), "", "阿濤");
        let scene0 = data::read_transcript(&root, &world_id, 0).unwrap();
        assert_eq!(scene0.len(), 1);
        assert_eq!(scene0[0].kind, data::TranscriptKind::System);
        assert!(!scene0[0].gm_only);
        assert!(scene0[0].text.starts_with("（角色回歸）〈狐狸〉\n"));
        assert!(scene0[0].text.contains("尾巴很大。"));

        // 第二輪：present 還是狐狸，本幕 events 已含前一則回歸事件 → 不重複
        record_card_arrivals(
            &root, &world_id, 0, &hidden_cards, &scene0, Some("狐狸"), "", "阿濤",
        );
        assert_eq!(data::read_transcript(&root, &world_id, 0).unwrap().len(), 1);

        // 不碰 auto_hidden 欄位本身：磁碟上仍是 true，要等換幕結算才會變 false
        let meta = data::list_characters(&root, &world_id)
            .unwrap()
            .into_iter()
            .find(|meta| meta.id == fox.id)
            .unwrap();
        assert!(meta.auto_hidden);

        std::fs::remove_dir_all(&root).unwrap();
    }

    /// AI 卡重構包 4b：visibility 非 Public 的世界書人物條目登場時，事件帶 gm_only=true
    /// （chars 續聊線只看得到前綴那一行，不洩漏全文）；這是 4a 遺留的洩漏修正。
    #[test]
    fn record_person_arrivals_marks_gm_only_for_non_public_visibility() {
        let root = std::env::temp_dir().join(format!(
            "table-tavern-person-gm-only-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let world_id = data::create_world(&root, "測試桌").unwrap();

        let spy = data::WorldbookEntry {
            uid: 1,
            title: "密探".to_owned(),
            keys: Vec::new(),
            content: "其實是反派的眼線。".to_owned(),
            constant: true,
            order: 0,
            disabled: false,
            visibility: data::Visibility::Gm,
            is_person: true,
            locked: false,
        };
        let worldbook = [spy];

        record_person_arrivals(&root, &world_id, 0, &worldbook, &[], Some("密探"), "", "阿濤");
        let scene0 = data::read_transcript(&root, &world_id, 0).unwrap();
        assert_eq!(scene0.len(), 1);
        assert!(scene0[0].gm_only);

        std::fs::remove_dir_all(&root).unwrap();
    }
}
