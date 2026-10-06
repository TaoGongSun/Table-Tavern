#[cfg(test)]
use crate::chat_assembly::gm_turn_instruction;
use crate::chat_assembly::{self, gm_materials, GmMaterials};
use crate::commands::character::load_active_cards;
use crate::transport::dispatch::{
    ai_call_failure, lane_provider, prepare_lane_call, stream_turn_reporting_truncation,
    stream_turn_via_transport,
};
use crate::usage::log as usage_log;
use crate::{config_root, data, data_root, inflight, lanes, mechanism, transport};
use serde::Serialize;
use tauri::Emitter;

/// 角色對話的回傳。`aborted` 為真時 `text` 是停止當下已經吐出的半截，可能是空的。
#[derive(Serialize)]
pub(crate) struct ChatReply {
    text: String,
    aborted: bool,
}

struct Spoken {
    text: String,
    aborted: bool,
}

/// 增量先寫進這支緩衝再送前端。中止贏的那個當下，緩衝裡就是玩家已經看到的字。
fn push_delta(
    buffer: &std::sync::Mutex<String>,
    on_delta: &tauri::ipc::Channel<String>,
    delta: &str,
) {
    buffer.lock().expect("delta buffer").push_str(delta);
    let _ = on_delta.send(delta.to_owned());
}

fn take_delta(buffer: &std::sync::Mutex<String>) -> String {
    std::mem::take(&mut *buffer.lock().expect("delta buffer"))
}

/// API 路徑在呼叫處跟取消賽跑。完成的分支排前面：兩邊同時就緒時交回完整結果。
async fn take_abort_or_finish(
    cancel: &mut inflight::CancelSignal,
    buffer: &std::sync::Mutex<String>,
    call: impl std::future::Future<Output = Result<String, String>>,
) -> Result<Spoken, String> {
    tokio::pin!(call);
    let finished = tokio::select! {
        biased;
        result = call.as_mut() => Some(result),
        _ = cancel.cancelled() => None,
    };
    match finished {
        Some(result) => Ok(Spoken {
            text: result?,
            aborted: false,
        }),
        None => {
            drop(call);
            Ok(Spoken {
                text: take_delta(buffer),
                aborted: true,
            })
        }
    }
}

/// 上下文組裝→單發呼叫→串流回傳（KICKOFF §4）。
/// 上下文完全由本機正典（角色卡＋可見世界書＋公開 transcript）經 assemble_messages 組裝，
/// 再依 preferences.transport 分流到 API 或 CLI；增量文字經 on_delta channel 回前端。
#[tauri::command]
pub(crate) async fn chat_with_character(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    turn_id: String,
    on_delta: tauri::ipc::Channel<String>,
    action_id: Option<String>,
) -> Result<ChatReply, String> {
    let _permit = data::world_write_permit_async(&world_id).await?;
    let (_guard, mut cancel) = inflight::register_turn(&world_id, &turn_id);
    let buffer = std::sync::Mutex::new(String::new());
    let root = data_root(&app)?;
    // 換幕容量鎖：排在代落上一回合待落正文之前（待落的已算進本幕），擋下就什麼都不寫。
    // 同一動作已在落玩家句時通過就不重扣（scene_budget::gate）
    crate::scene_budget::check_capacity(
        &config_root(&app)?,
        &root,
        &world_id,
        action_id.as_deref(),
        "",
    )?;
    // 回合交接（計畫 8.4）：上一個 GM 回合提交了正文卻沒落檔，先由後端代落；GM 還在生成就擋
    data::state_commit::with_commit(&root, &world_id, data::message_vars::settle_previous_turn)
        .map_err(|error| error.to_string())?;
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
    let emit = |delta: &str| push_delta(&buffer, &on_delta, delta);
    // CLI 訂閱走 resume 續聊線。claude／grok 全角色共用一條 session，私設回合注入、
    // 回合後從 session 檔抹掉（案 C）；Agy 無抹寫路徑，改成一角一線＋
    // 私設提進該角色自己的凍結 system，不讓別的角色讀到不該讀的東西。
    if let Some(provider) = lane_provider(&config) {
        let hoist = provider == lanes::LaneProvider::Agy;
        let lang = transport::ui_language(&config);
        let cards = load_active_cards(&root, &world_id)?;
        let (frozen, turn) = chat_assembly::character_lane_parts(
            &card,
            &cards,
            player.as_ref(),
            &events,
            &worldbook,
            &state,
            branch.as_deref(),
            &lang,
            hoist,
        );
        let call = prepare_play_lane_call(&app, &config, card.tier, provider).await?;
        let outcome = lanes::run_turn(
            &call,
            &root,
            &world_id,
            lanes::TurnInput {
                lane: lanes::Lane::Chars,
                scene: state.current_scene,
                events: &events,
                lang: &lang,
                frozen_system: frozen,
                tail: turn.tail,
                confidential: (!hoist).then_some(turn.confidential).flatten(),
                prefix: (!hoist).then(|| transport::speaker_prefix(&card.name, &lang)),
                echo: lanes::ReplyEcho::Dialogue {
                    speaker_id: card.id.clone(),
                },
                scope: hoist.then(|| card.id.clone()),
            },
            Some(&mut cancel),
            emit,
        )
        .await
        .map_err(ai_call_failure)?;
        // 半截用這一輪 CLI 自己累的字。外層緩衝會跨降級重開留著前一次嘗試的增量。
        return Ok(ChatReply {
            text: outcome.text,
            aborted: outcome.aborted,
        });
    }
    // api／codex 走共線組裝（api-shared-lane 包 B）：全角色共用一份與「這輪是誰」
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
    let spoken = take_abort_or_finish(
        &mut cancel,
        &buffer,
        stream_turn_via_transport(
            &app,
            &config,
            None,
            false,
            card.tier,
            Some(&world_id),
            Some(&turn_id),
            "",
            "",
            &messages,
            usage_log::PromptShape::Turn {
                roster: cards.len(),
                solo: cards.len() <= 1,
            },
            false,
            emit,
        ),
    )
    .await?;
    Ok(ChatReply {
        text: spoken.text,
        aborted: spoken.aborted,
    })
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
    cancel: Option<&mut inflight::CancelSignal>,
    emit: impl FnMut(&str),
) -> Result<lanes::TurnOutcome, String> {
    let (frozen, tail) = chat_assembly::gm_lane_parts(materials, scope, instruction, lang);
    let call = prepare_play_lane_call(app, config, transport::gm_tier(config), provider).await?;
    lanes::run_turn(
        &call,
        root,
        world_id,
        lanes::TurnInput {
            lane: lanes::Lane::Gm,
            scene: materials.state.current_scene,
            events: &materials.events,
            lang,
            frozen_system: frozen,
            tail,
            confidential: None,
            prefix: None,
            echo,
            scope: None, // GM 只有一條線，不細分
        },
        cancel,
        emit,
    )
    .await
    .map_err(ai_call_failure)
}

/// 中止的旁白：半截原文照正常剝法留成 text，其餘寫入一律空著。
fn aborted_narration(reply: &str) -> GmNarration {
    let block = transport::extract_state_block(reply);
    let (_next, display) = transport::extract_next_speaker(&block.display);
    GmNarration {
        text: display,
        raw: None,
        next: None,
        state_updates: Vec::new(),
        arrived_characters: Vec::new(),
        arrived_persons: Vec::new(),
        aborted: true,
        state_error: None,
    }
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
    /// 玩家按下停止、而且取消贏過完成。true 時上面的寫入欄位都是空的。
    aborted: bool,
    /// 狀態更新沒寫成（磁碟錯誤等）：正文照常落檔，前端提示這一輪的狀態可能沒更新
    state_error: Option<String>,
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
    turn_id: String,
    on_delta: tauri::ipc::Channel<String>,
    action_id: Option<String>,
) -> Result<GmNarration, String> {
    let _permit = data::world_write_permit_async(&world_id).await?;
    let (_guard, mut cancel) = inflight::register_turn(&world_id, &turn_id);
    let buffer = std::sync::Mutex::new(String::new());
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let root = data_root(&app)?;
    // 換幕容量鎖：開回合紀錄之前擋，擋下就什麼都不寫
    crate::scene_budget::check_capacity(
        &config_root(&app)?,
        &root,
        &world_id,
        action_id.as_deref(),
        "",
    )?;
    // 回合開始（鎖內）：記回合紀錄與固定輸入；中止、出錯時 TurnGuard 把它改成已中止（計畫 8.3）
    let turn = TurnGuard::begin(&root, &world_id, &turn_id, &lang)?;
    let materials = gm_materials(&root, &world_id)?;
    let roster: Vec<String> = materials
        .cards
        .iter()
        .map(|card| card.name.clone())
        .collect();
    let player_name = materials.player.as_ref().map(|card| card.name.as_str());
    // 換幕後第一輪 GM 回合送整棵樹對齊；之後每輪只送在場分支＋變動標記（狀態欄二期包 5）。
    let (scope, align) = chat_assembly::gm_scope(&materials);
    let (instruction_message, closing) =
        chat_assembly::gm_instruction(&root, &world_id, &materials, &lang);
    let emit = |delta: &str| push_delta(&buffer, &on_delta, delta);
    // 這一次 GM 呼叫的回覆有沒有被供應商截斷（只認這次呼叫自己回報的，同桌別的呼叫碰不到）
    let cut = std::sync::atomic::AtomicBool::new(false);
    let reply = if let Some(provider) = lane_provider(&config) {
        let instruction = format!("{}\n{closing}", instruction_message.content);
        let outcome = gm_lane_reply(
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
            Some(&mut cancel),
            emit,
        )
        .await?;
        if outcome.aborted {
            // 同角色線：用這一輪的半截，再照正常剝法去掉狀態欄與點名行。
            return Ok(turn.abort(aborted_narration(&outcome.text)));
        }
        outcome.text
    } else {
        let messages = chat_assembly::gm_messages(&materials, &scope, instruction_message, &lang);
        // GM 上下文一律全卡，與「這輪誰說話」無關，形狀恆為共線
        let spoken = take_abort_or_finish(&mut cancel, &buffer, async {
            let (text, truncated) = stream_turn_reporting_truncation(
                &app,
                &config,
                None,
                false,
                transport::gm_tier(&config),
                Some(&world_id),
                Some(&turn_id),
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
            .await?;
            cut.store(truncated, std::sync::atomic::Ordering::Relaxed);
            Ok(text)
        })
        .await?;
        if spoken.aborted {
            return Ok(turn.abort(aborted_narration(&spoken.text)));
        }
        spoken.text
    };
    let block = transport::extract_state_block(&reply);
    let (next_raw, display) = transport::extract_next_speaker(&block.display);
    // 剝掉 state／next 控制欄後沒有正文＝這一輪沒東西寫進故事。必須擋在下面的
    // apply_block 之前：狀態套用只要 incremental 為真就必跑，失敗回合會白白重擲一輪骰
    // （stream-failure-visible）。CLI 那條路沒有 stream_chat 的收工判定，這裡是唯一防線。
    if display.trim().is_empty() {
        return Err(format!(
            "AI_EMPTY_RESPONSE: no_text_after_control_lines raw_len={}",
            reply.chars().count()
        ));
    }
    let mut state_updates: Vec<StateUpdate> = Vec::new();
    let mut arrived_persons = Vec::new();
    let mut arrived_characters = Vec::new();
    // 狀態更新一律盡力而為：模型格式壞掉或存檔寫不進去，都不該害玩家丟掉整段旁白。
    // 骰值要每回合重擲，就算這一輪模型完全沒吐更新也要跑一次。
    let apply = !block.fields.is_empty()
        || !block.updates.is_empty()
        || materials.state.mechanism.incremental;
    let user_name = player_name.unwrap_or_else(|| transport::player_fallback_name(&lang));
    // 提交（鎖內、核對 turn_id、幕與世代）：套在回合開始時固定的輸入上；變數模式時新表與正文留在回合紀錄
    // 等前端落檔。狀態沒寫成要明確回報（state_error），不吞掉
    let main = data::message_vars::PendingMain {
        text: display.clone(),
        raw: (reply != display).then(|| reply.clone()),
        truncated: cut.load(std::sync::atomic::Ordering::Relaxed),
    };
    let committed = turn.commit(
        main,
        |state, stat| {
            let outcome = apply.then(|| {
                let scene = state.current_scene;
                state.mechanism.numeric_update = materials.state.mechanism.numeric_update;
                let outcome = mechanism::apply_block_typed(state, &block, user_name, stat);
                if align {
                    state.aligned_scene = Some(scene);
                }
                outcome
            });
            let typed = outcome.as_ref().and_then(|outcome| outcome.typed.clone());
            (outcome, typed)
        },
        // 變動紀錄與正文在同一次提交登記：前端沒落成時跟正文一起代落（長欄位變動靠它進歷史）
        |state, outcome| {
            if outcome.is_none() {
                return Vec::new();
            }
            state_updates = transport::snapshot_updates(&state.state, &state.mechanism, user_name)
                .into_iter()
                .map(|(path, value)| StateUpdate { path, value })
                .collect();
            state_update_side(&state_updates).into_iter().collect()
        },
    );
    let mut state_error = None;
    let committed = match committed {
        Ok(Some(commit)) => {
            state_error = commit.error;
            Some((commit.state, commit.result))
        }
        Ok(None) => None,
        Err(error) => {
            state_error = Some(error);
            None
        }
    };
    if let Some((state, Some(outcome))) = committed {
        let scene = state.current_scene;
        mechanism::append_log(&root, &world_id, scene, &outcome.records);
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
            Some(&turn.ticket),
            action_id.as_deref(),
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
                Some(&turn.ticket),
                action_id.as_deref(),
            );
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
        aborted: false,
        state_error,
    })
}

/// 一輪 GM 回合的回合紀錄（計畫 8.3）。開始時記下、提交時核對 turn_id；沒走到提交就離開（中止、出錯、
/// 空回覆）時，drop 把還在生成中的紀錄改成已中止，卡寫與面板手改隨即放行。
struct TurnGuard {
    root: std::path::PathBuf,
    world_id: String,
    turn_id: String,
    /// 同回合內部追加（登場紀錄）用的憑證
    ticket: data::message_vars::TurnTicket,
}

/// 變動紀錄（與前端落的那則同一個寫法）；沒有變動就沒有這一部分。
fn state_update_side(updates: &[StateUpdate]) -> Option<data::message_vars::TurnSide> {
    if updates.is_empty() {
        return None;
    }
    Some(data::message_vars::TurnSide {
        part: "state_update".to_owned(),
        kind: data::TranscriptKind::System,
        text: updates
            .iter()
            .map(|update| format!("{}：{}", update.path, update.value))
            .collect::<Vec<_>>()
            .join("\n"),
        marker: Some(data::EventMarker::StateUpdate),
    })
}

impl TurnGuard {
    fn begin(
        root: &std::path::Path,
        world_id: &str,
        turn_id: &str,
        lang: &str,
    ) -> Result<Self, String> {
        let user = data::read_player_card(root, world_id)
            .ok()
            .flatten()
            .map(|card| card.name)
            .unwrap_or_else(|| transport::player_fallback_name(lang).to_owned());
        let macros = data::message_vars::Macros { user, char: None };
        let ticket = data::state_commit::with_commit(root, world_id, |tx| {
            data::message_vars::begin_turn(tx, turn_id, Some(&macros))
        })
        .map_err(|error| error.to_string())?;
        Ok(Self {
            root: root.to_path_buf(),
            world_id: world_id.to_owned(),
            turn_id: turn_id.to_owned(),
            ticket,
        })
    }

    /// turn_id／幕／世代已不符回 Ok(None)；讀寫錯誤回 Err（呼叫端回報，不害玩家丟掉整段旁白）。
    fn commit<R>(
        &self,
        main: data::message_vars::PendingMain,
        apply: impl FnOnce(
            &mut data::WorldState,
            Option<&data::message_vars::Json>,
        ) -> (R, Option<data::message_vars::TypedBatch>),
        sides: impl FnOnce(&data::WorldState, &R) -> Vec<data::message_vars::TurnSide>,
    ) -> Result<Option<data::message_vars::GmCommit<R>>, String> {
        data::state_commit::with_commit(&self.root, &self.world_id, |tx| {
            data::message_vars::apply_gm_block(tx, &self.turn_id, main, apply, sides)
        })
        .map_err(|error| error.to_string())
    }

    /// 中止：半截正文記進回合紀錄（前端沒落成時由下一筆新事件前的交接代落），回原樣的中止結果。
    fn abort(&self, narration: GmNarration) -> GmNarration {
        let half = data::message_vars::PendingMain {
            text: narration.text.clone(),
            raw: None,
            truncated: true,
        };
        let _ = data::state_commit::with_commit(&self.root, &self.world_id, |tx| {
            data::message_vars::finish_turn(tx, &self.turn_id, Some(half))
        });
        narration
    }
}

impl Drop for TurnGuard {
    fn drop(&mut self) {
        let _ = data::state_commit::with_commit(&self.root, &self.world_id, |tx| {
            data::message_vars::finish_turn(tx, &self.turn_id, None)
        });
    }
}

/// 中止這一輪對話。只打 `turn_id` 對得上的那一筆；晚到的舊 id 不會波及下一輪，也不會打到重構。
#[tauri::command]
pub(crate) fn chat_abort(world_id: String, turn_id: String) {
    inflight::abort_turn(&world_id, &turn_id);
}

/// 登場紀錄：GM 回合提交後的附屬追加帶回合憑證（不交接，排在正文前）；沒有憑證就是一般新事件。
/// `action`：觸發這回合的玩家動作，蓋在事件上供換幕容量預測切段。
fn append_arrival(
    root: &std::path::Path,
    world_id: &str,
    scene: u64,
    event: &data::TranscriptEvent,
    turn: Option<&data::message_vars::TurnTicket>,
    action: Option<&str>,
) -> data::DataResult<u64> {
    let event = data::TranscriptEvent {
        action_id: action.map(str::to_owned),
        ..event.clone()
    };
    match turn {
        Some(ticket) => data::append_within_turn(root, world_id, scene, &event, ticket),
        None => data::append_transcript(root, world_id, scene, &event),
    }
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
    turn: Option<&data::message_vars::TurnTicket>,
    action: Option<&str>,
) -> Vec<String> {
    let already = data::appeared_person_titles(events);
    let arrivals = transport::detect_new_arrivals(worldbook, present, reply_body, &already);
    if arrivals.is_empty() {
        return Vec::new();
    }
    let ts = data::local_timestamp().unwrap_or_default();
    let mut titles = Vec::new();
    for entry in arrivals {
        let (marker, text) = transport::person_arrival(entry, user_name);
        let event = data::TranscriptEvent {
            id: None,
            message_vars: None,
            vars_rev: None,
            vars_epoch: None,
            turn_key: None,
            action_id: None,
            ts: ts.clone(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: data::TranscriptKind::System,
            text,
            raw: None,
            state: None,
            truncated: false,
            gm_only: !matches!(entry.visibility, data::Visibility::Public),
            marker: Some(marker),
            opening: false,
        };
        if append_arrival(root, world_id, scene, &event, turn, action).is_ok() {
            titles.push(entry.title.clone());
        }
    }
    titles
}

/// 角色卡自動回歸（AI 卡重構包 4b）：present 名單（缺席就退回本文比對）比對得上、
/// 本幕還沒回歸過的 auto_hidden 卡，逐一 append 公開回歸事件；有私設時另 append
/// 一則 gm_only 私設事件（只有 GM 看得到，card-arrival-private-leak）。同一張卡本幕只記一次。鏡射 record_person_arrivals，鍵從世界書 title 換成卡片 name；
/// **不改 auto_hidden 欄位本身**（鐵律：持久欄位只在換幕結算，見 data::begin_next_scene）。
/// 私設先寫：私設寫失敗就整張跳過、下一輪重試；公開事件寫失敗頂多多一則重複私設，
/// 不會出現「已回歸卻沒有私設」被去重擋住補不回來。
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
    turn: Option<&data::message_vars::TurnTicket>,
    action: Option<&str>,
) -> Vec<String> {
    let already = data::appeared_card_names(events);
    let arrivals = transport::detect_new_card_arrivals(hidden_cards, present, reply_body, &already);
    if arrivals.is_empty() {
        return Vec::new();
    }
    let ts = data::local_timestamp().unwrap_or_default();
    let mut ids = Vec::new();
    let system_event =
        |(marker, text): (data::EventMarker, String), gm_only: bool| data::TranscriptEvent {
            id: None,
            message_vars: None,
            vars_rev: None,
            vars_epoch: None,
            turn_key: None,
            action_id: None,
            ts: ts.clone(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: data::TranscriptKind::System,
            text,
            raw: None,
            state: None,
            truncated: false,
            gm_only,
            marker: Some(marker),
            opening: false,
        };
    for card in arrivals {
        if let Some(private) = transport::card_private(card, user_name) {
            if append_arrival(
                root,
                world_id,
                scene,
                &system_event(private, true),
                turn,
                action,
            )
            .is_err()
            {
                continue;
            }
        }
        let event = system_event(transport::card_arrival(card, user_name), false);
        if append_arrival(root, world_id, scene, &event, turn, action).is_ok() {
            ids.push(card.id.clone());
        }
    }
    ids
}

/// 劇情續聊線（角色線、GM 線）的 CLI 呼叫素材：claude 快取釘 1 小時，
/// 有嘗試落在訂閱超額就通知前端一次（`claude-overage`，app 層只提示一次）。
async fn prepare_play_lane_call(
    app: &tauri::AppHandle,
    config: &data::AppConfig,
    tier: data::Tier,
    provider: lanes::LaneProvider,
) -> Result<lanes::LaneCall, String> {
    let mut call = prepare_lane_call(app, config, tier, provider).await?;
    lanes::pin_lane_cache_ttl(&mut call);
    let app = app.clone();
    call.on_overage = Some(std::sync::Arc::new(
        move |observed: lanes::CacheWriteObserved| {
            let _ = app.emit(
                "claude-overage",
                serde_json::json!({
                    "eventId": ulid::Ulid::generate().to_string(),
                    "observed": observed.as_str(),
                }),
            );
        },
    ));
    Ok(call)
}

#[cfg(test)]
mod tests {
    use super::{record_card_arrivals, record_person_arrivals};
    use crate::commands::{character_card, NEXT_TEMP_ID};
    use crate::data;
    use std::sync::atomic::Ordering;

    /// 零額度讀出 GM lane 實際送出的提示詞（凍結 system＋回合尾段），不打 AI。手動執行：
    /// `TT_PROMPT_ROOT=<資料根目錄> TT_PROMPT_WORLD=<桌 id> cargo test --lib dump_gm_lane_prompt -- --ignored --nocapture`
    /// 會把 system 與回合尾段印到 stdout，並把兩段寫進 TT_PROMPT_OUT（有設才寫）。
    #[test]
    #[ignore]
    fn dump_gm_lane_prompt() {
        let (Ok(root), Ok(world_id)) = (
            std::env::var("TT_PROMPT_ROOT"),
            std::env::var("TT_PROMPT_WORLD"),
        ) else {
            eprintln!("未設 TT_PROMPT_ROOT／TT_PROMPT_WORLD，略過");
            return;
        };
        let root = std::path::PathBuf::from(root);
        let materials = super::gm_materials(&root, &world_id).unwrap();
        let roster: Vec<String> = materials
            .cards
            .iter()
            .map(|card| card.name.clone())
            .collect();
        let player_name = materials.player.as_ref().map(|card| card.name.as_str());
        let lang = "zh-TW";
        let (instruction_message, closing) =
            super::gm_turn_instruction(&root, &world_id, &materials, &roster, player_name, lang);
        let scope = crate::transport::state_scope(
            &materials.state.state,
            &materials.state.mechanism,
            &materials.cards,
            materials.player.as_ref(),
            &materials.state.branch_bindings,
            false,
        );
        let system = crate::transport::gm_lane_system(
            &materials.world_md,
            &materials.cards,
            materials.player.as_ref(),
            &materials.worldbook,
            &materials.state.mechanism,
            lang,
        );
        let instruction = format!("{}\n{closing}", instruction_message.content);
        let turn = crate::transport::gm_lane_turn(
            &materials.events,
            &materials.worldbook,
            materials.player.as_ref(),
            &materials.state.state,
            &materials.state.mechanism,
            &scope,
            &instruction,
            lang,
        );
        let dump = format!(
            "===== SYSTEM =====\n{system}\n\n===== TURN TAIL =====\n{}\n",
            turn.tail
        );
        println!("{dump}");
        if let Ok(out) = std::env::var("TT_PROMPT_OUT") {
            std::fs::write(out, &dump).unwrap();
        }
    }

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
        record_person_arrivals(
            &root,
            &world_id,
            0,
            &worldbook,
            &[],
            Some("愛麗絲"),
            "",
            "阿濤",
            None,
            Some("act-1"),
        );
        let scene0 = data::read_transcript(&root, &world_id, 0).unwrap();
        assert_eq!(scene0.len(), 1);
        // 登場紀錄蓋上觸發它的動作 id（換幕容量預測切段用）
        assert_eq!(scene0[0].action_id.as_deref(), Some("act-1"));
        assert_eq!(scene0[0].kind, data::TranscriptKind::System);
        assert_eq!(scene0[0].speaker_id, "");
        assert_eq!(scene0[0].speaker_name, "GM");
        assert_eq!(
            scene0[0].marker,
            Some(data::EventMarker::PersonArrival {
                title: "愛麗絲".to_owned()
            })
        );
        assert!(scene0[0].text.contains("愛麗絲是旅店老闆娘。"));

        // 第二輪：present 還是愛麗絲，本幕 events 已含前一則登場事件 → 不重複
        record_person_arrivals(
            &root,
            &world_id,
            0,
            &worldbook,
            &scene0,
            Some("愛麗絲"),
            "",
            "阿濤",
            None,
            None,
        );
        assert_eq!(data::read_transcript(&root, &world_id, 0).unwrap().len(), 1);

        // 換幕：scene 1 是新 jsonl、events 是空的 → 同名重新 append
        record_person_arrivals(
            &root,
            &world_id,
            1,
            &worldbook,
            &[],
            Some("愛麗絲"),
            "",
            "阿濤",
            None,
            None,
        );
        let scene1 = data::read_transcript(&root, &world_id, 1).unwrap();
        assert_eq!(scene1.len(), 1);
        assert!(matches!(
            &scene1[0].marker,
            Some(data::EventMarker::PersonArrival { title }) if title == "愛麗絲"
        ));

        std::fs::remove_dir_all(&root).unwrap();
    }

    /// AI 卡重構包 4b，鏡射 4a：present 有隱藏卡的名字就 append 回歸事件——私設先成一則
    /// gm_only 事件、公開設定再成一則（card-arrival-private-leak）；同一幕重複比對不重複
    /// append；不改 auto_hidden 欄位本身（鐵律，換幕才結算）。
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
        fox.private_md = "其實是{{user}}的仇人。".to_owned();
        data::write_character(&root, &world_id, &fox).unwrap();
        data::set_character_auto_hidden(&root, &world_id, &fox.id, true).unwrap();
        let mut owl = character_card(&data::new_id(), "貓頭鷹");
        owl.public_md = "夜裡才醒。".to_owned();
        data::write_character(&root, &world_id, &owl).unwrap();
        data::set_character_auto_hidden(&root, &world_id, &owl.id, true).unwrap();
        let hidden_cards = vec![fox.clone(), owl.clone()];

        // 第一輪：present 有狐狸與貓頭鷹 → 狐狸私設＋公開兩則，貓頭鷹沒私設只有公開一則
        let ids = record_card_arrivals(
            &root,
            &world_id,
            0,
            &hidden_cards,
            &[],
            Some("狐狸、貓頭鷹"),
            "",
            "阿濤",
            None,
            Some("act-2"),
        );
        assert_eq!(ids, vec![fox.id.clone(), owl.id.clone()]);
        let scene0 = data::read_transcript(&root, &world_id, 0).unwrap();
        assert_eq!(scene0.len(), 3);
        // 卡片登場（含私設）全部蓋上觸發它的動作 id
        assert!(scene0
            .iter()
            .all(|event| event.action_id.as_deref() == Some("act-2")));
        assert!(scene0
            .iter()
            .all(|event| event.kind == data::TranscriptKind::System));
        assert!(scene0[0].gm_only);
        let full = |event: &data::TranscriptEvent| data::event_full_text(event, "zh-TW");
        assert_eq!(
            full(&scene0[0]),
            "（角色私設）〈狐狸〉\n私有設定：\n其實是阿濤的仇人。"
        );
        assert!(!scene0[1].gm_only);
        assert_eq!(
            full(&scene0[1]),
            "（角色回歸）〈狐狸〉\n公開設定：\n尾巴很大。"
        );
        assert!(!scene0[2].gm_only);
        assert_eq!(
            full(&scene0[2]),
            "（角色回歸）〈貓頭鷹〉\n公開設定：\n夜裡才醒。"
        );

        // 第二輪：present 還是狐狸，本幕 events 已含前一則回歸事件 → 不重複
        record_card_arrivals(
            &root,
            &world_id,
            0,
            &hidden_cards,
            &scene0,
            Some("狐狸、貓頭鷹"),
            "",
            "阿濤",
            None,
            None,
        );
        assert_eq!(data::read_transcript(&root, &world_id, 0).unwrap().len(), 3);

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

        record_person_arrivals(
            &root,
            &world_id,
            0,
            &worldbook,
            &[],
            Some("密探"),
            "",
            "阿濤",
            None,
            None,
        );
        let scene0 = data::read_transcript(&root, &world_id, 0).unwrap();
        assert_eq!(scene0.len(), 1);
        assert!(scene0[0].gm_only);

        std::fs::remove_dir_all(&root).unwrap();
    }

    /// mvu-replace-numeric：GM 回合在 gm_materials 算一次策略，提示詞與提交（照 turn.commit 的做法把同一份
    /// 帶進鎖內讀的狀態）都用它——沒重構的 MVU 桌兩邊都放行 replace，重構後兩邊都回原規則。
    #[test]
    fn gm_turn_uses_one_numeric_policy_for_prompt_and_commit() {
        let root = std::env::temp_dir().join(format!(
            "table-tavern-gm-numeric-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let world_id = data::create_world(&root, "測試桌").unwrap();
        let raw = serde_json::json!({"data": {"name": "莉亞", "character_book": {"entries": [
            {"comment": "[initvar]初始", "enabled": false, "content": "World:\n  Gold: 100000"}
        ]}}})
        .to_string();
        crate::import::import_character(&root, &world_id, raw.as_bytes(), "#3366ff", "zh-TW")
            .unwrap();
        let reply = "旁白<UpdateVariable><JSONPatch>[{\"op\":\"replace\",\"path\":\"/World/Gold\",\"value\":99000}]</JSONPatch></UpdateVariable>";
        let block = crate::transport::extract_state_block(reply);
        let run = |expected: data::NumericUpdate, expected_gold: &str, prompt: &str| {
            let materials = super::gm_materials(&root, &world_id).unwrap();
            assert_eq!(materials.state.mechanism.numeric_update, expected);
            let system = crate::transport::gm_lane_system(
                &materials.world_md,
                &materials.cards,
                materials.player.as_ref(),
                &materials.worldbook,
                &materials.state.mechanism,
                "zh-TW",
            );
            assert!(system.contains(prompt), "{system}");
            let mut state = data::read_state(&root, &world_id).unwrap();
            state.mechanism.numeric_update = materials.state.mechanism.numeric_update;
            crate::mechanism::apply_block(&mut state, &block, "阿濤");
            let gold =
                data::node_at(&state.state.tree, &["World".to_owned(), "Gold".to_owned()]).cloned();
            assert_eq!(gold, Some(data::StateNode::Leaf(expected_gold.to_owned())));
        };
        run(
            data::NumericUpdate::Upstream,
            "99000",
            "也可以用 replace 直接寫新值",
        );
        let mut state = data::read_state(&root, &world_id).unwrap();
        state.refactor_mode = Some("interface".to_owned());
        data::write_state(&root, &world_id, &state).unwrap();
        run(
            data::NumericUpdate::DeltaOnly,
            "100000",
            "給絕對值會被系統擋下",
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// mvu-replace-numeric × gm-format-directive-missing-target：沒重構的 MVU 卡、原卡格式條目照原卡啟用時，
    /// 完整 GM 提示（凍結 system＋回合尾段的導演指示與收尾句）裡，上游版協定、卡的格式條目、導演指示三者
    /// 不能互相矛盾：不能再出現「數字欄只收 delta／絕對值會被擋／上下限由系統把關」，指示也不能禁掉更新區塊。
    #[test]
    fn full_gm_prompt_with_card_format_and_upstream_protocol_is_consistent() {
        for (label, format_tag) in [("named", "<StatusBlock>"), ("neutral", "【狀態】")] {
            let root = std::env::temp_dir().join(format!(
                "table-tavern-gm-format-upstream-{label}-{}-{}",
                std::process::id(),
                NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&root).unwrap();
            let world_id = data::create_world(&root, "測試桌").unwrap();
            let format = format!(
                "每回合正文後輸出 {format_tag} 狀態欄，最後用 <UpdateVariable><JSONPatch> 回報變數，\
                 數值可直接 replace，例 {{\"op\":\"replace\",\"path\":\"/World/Gold\",\"value\":90}}。"
            );
            let raw = serde_json::json!({"data": {
                "name": "莉亞",
                "extensions": {"regex_scripts": [{
                    "scriptName": "狀態欄", "findRegex": "/<StatusBlock>([\\s\\S]*?)<\\/StatusBlock>/s",
                    "replaceString": "<div>$1</div>", "placement": [2], "disabled": false
                }]},
                "character_book": {"entries": [
                    {"comment": "[initvar]初始", "enabled": false, "content": "World:\n  Gold: 100"},
                    {"comment": "[mvu_update]变量更新规则", "enabled": true,
                     "content": "规则:\n  World:\n    Gold:\n      type: number\n      range: 0-1000\n"},
                    {"comment": "[mvu_update]变量输出格式", "enabled": true, "constant": true, "content": format}
                ]}
            }})
            .to_string();
            crate::import::import_character(&root, &world_id, raw.as_bytes(), "#3366ff", "zh-TW")
                .unwrap();
            let materials = super::gm_materials(&root, &world_id).unwrap();
            assert_eq!(
                materials.state.mechanism.numeric_update,
                data::NumericUpdate::Upstream,
                "{label}"
            );
            let lang = "zh-TW";
            let (instruction, closing) =
                super::gm_turn_instruction(&root, &world_id, &materials, &[], None, lang);
            let system = crate::transport::gm_lane_system(
                &materials.world_md,
                &materials.cards,
                materials.player.as_ref(),
                &materials.worldbook,
                &materials.state.mechanism,
                lang,
            );
            let prompt = format!("{system}\n{}\n{closing}", instruction.content);
            // 卡的格式條目照原卡啟用、全文進提示；上游版協定在
            assert!(prompt.contains("數值可直接 replace"), "{label}: {prompt}");
            assert!(prompt.contains("也可以用 replace 直接寫新值"), "{label}");
            assert!(prompt.contains("系統不會幫你夾在上下限內"), "{label}");
            for contradiction in [
                "數字欄一律用 delta",
                "給絕對值會被系統擋下",
                "只有上限改變（升級）才用 replace",
                "上下限與拒收由系統把關",
                "你只要說「這一幕變動了多少」",
            ] {
                assert!(!prompt.contains(contradiction), "{label}: {contradiction}");
            }
            match label {
                "named" => assert!(
                    instruction.content.contains("[mvu_update]变量输出格式"),
                    "{label}: {}",
                    instruction.content
                ),
                _ => assert!(
                    instruction.content.contains("狀態更新協定"),
                    "{label}: {}",
                    instruction.content
                ),
            }
            // 導演指示與收尾句不禁更新區塊（中性版不寫「只輸出正文」）
            for banned in ["只輸出正文", "不要輸出更新區塊", "不要輸出狀態更新"]
            {
                assert!(!instruction.content.contains(banned), "{label}: {banned}");
                assert!(!closing.contains(banned), "{label}: {banned}");
            }
            let _ = std::fs::remove_dir_all(&root);
        }
    }
}
