//! GM 旁白回合（簡易導演：旁白＋點名一次呼叫完成）。
use super::arrivals::{load_hidden_cards, record_card_arrivals, record_person_arrivals};
use super::commit::{
    gm_commit_apply, state_update_lines, state_update_side, StateUpdate, TurnGuard,
};
use super::gm_reply::{aborted_narration, gm_reply_parts, GmReplyParts};
use super::stream::{prepare_play_lane_call, push_delta, take_abort_or_finish};
use super::{vn_ending, vn_script};
#[cfg(test)]
use crate::chat_assembly::gm_turn_instruction;
use crate::chat_assembly::{self, gm_materials, GmMaterials};
use crate::transport::dispatch::{
    ai_call_failure, lane_provider, stream_turn_reporting_truncation,
};
use crate::usage::log as usage_log;
use crate::{config_root, data, data_root, inflight, lanes, mechanism, transport, world_scan};
use serde::Serialize;
use serde_json::{json, Value};

/// GM lane 的一輪：凍結 system（GM 指示＋world.md＋全 constant＋全卡）＋回合尾段
/// （keyword 條目＋狀態＋導演指示）。narrate 與 suggest 共用，差別只在指示與 echo。
#[allow(clippy::too_many_arguments)]
async fn gm_lane_reply(
    app: &tauri::AppHandle,
    config: &data::AppConfig,
    root: &std::path::Path,
    world_id: &str,
    turn_id: &str,
    materials: &GmMaterials,
    scan: &world_scan::WorldScan,
    scope: &transport::StateScope,
    instruction: &str,
    echo: lanes::ReplyEcho,
    lang: &str,
    provider: lanes::LaneProvider,
    capacity: Option<world_scan::Budget>,
    cancel: Option<&mut inflight::CancelSignal>,
    emit: impl FnMut(&str),
) -> Result<lanes::TurnOutcome, String> {
    let (frozen, turn) = chat_assembly::gm_lane_parts(materials, scan, scope, instruction, lang);
    let call = prepare_play_lane_call(app, config, transport::gm_tier(config), provider).await?;
    world_scan::landing::land(
        root,
        world_id,
        materials.state.current_scene,
        turn_id,
        &data::world_info_store::Perspective::Gm,
        scan,
    )?;
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
            tail: turn.tail,
            confidential: None,
            prefix: None,
            echo,
            scope: None, // GM 只有一條線，不細分
            single_owner: None,
            has_state_block: false,
            hoisted_worldbook: None,
            tail_worldbook: turn.tail_worldbook,
            tail_tracked: true,
            opening_raw: materials.opening.raw_history,
            capacity,
            prompt_regex: materials.prompt_regex.clone(),
            vn: chat_assembly::vn_table(&materials.state),
        },
        cancel,
        emit,
    )
    .await
    .map_err(ai_call_failure)
}

/// gm_narrate 回傳：剝乾淨的旁白顯示文字＋下一位發言者（角色 id 或玩家哨兵）。
/// GM 沒點名或名字對不上名單＝None，前端就地停下、不當錯誤。
#[derive(Serialize)]
pub(crate) struct GmNarration {
    pub(crate) text: String,
    /// 剝殼前的模型原文；與 text 相同就是 None，前端照樣存進事件裡
    pub(crate) raw: Option<String>,
    pub(crate) next: Option<String>,
    /// 多人房間（multiplayer-room 包 3）：GM 點名點到某位真人玩家時是那張玩家卡 id（此時 next 是玩家哨兵）；
    /// 一般桌恆 None、不序列化，前端契約不變。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_player: Option<String>,
    /// VN 桌這一拍停在哪種終止標記（此時 `next` 一律 None）；聊天桌恆 None
    pub(super) vn_stop: Option<super::vn_script::VnStop>,
    /// 長文字欄這一輪的新值；前端接到就補一則 system 事件進 transcript。空的就不補。
    pub(super) state_updates: Vec<StateUpdate>,
    /// 這輪剛回歸（登場）的角色卡 id（AI 卡重構包 4b）；前端拿它把卡從隱藏區搬回主區。
    #[serde(default)]
    pub(crate) arrived_characters: Vec<String>,
    /// 這輪剛登場的世界書人物 title（4a 就有登場事件，4b 才在回傳裡帶出來）。
    #[serde(default)]
    pub(super) arrived_persons: Vec<String>,
    /// 玩家按下停止、而且取消贏過完成。true 時上面的寫入欄位都是空的。
    pub(crate) aborted: bool,
    /// 這一次呼叫被輸出上限截斷；接力驅動落檔時據此標 `truncated`。
    #[serde(skip)]
    pub(crate) truncated: bool,
    /// 狀態更新沒寫成（磁碟錯誤等）：正文照常落檔，前端提示這一輪的狀態可能沒更新
    pub(crate) state_error: Option<String>,
    /// 提交成功後的三鍵快照（平欄）。單人契約不帶這欄；房間 `gen.part.end` 才送
    #[serde(skip)]
    pub(crate) status: Option<Value>,
    /// 提交成功後的預設狀態欄快照（`scene_panel`）；只在預設桌有，房間 `gen.part.end` 的 `panel` 才送
    #[serde(skip)]
    pub(crate) panel: Option<Value>,
}

/// 房間廣播的預設狀態欄快照（table-status-panel §4）：只在預設桌回 Some。白名單取欄位、不整棵序列化——
/// 每格帶 `status`、`items`（索引分支轉字串陣列）與手動加的文字欄；追蹤只帶三條清單。底線開頭的鍵不送。
pub(crate) fn scene_panel(state: &data::WorldState) -> Option<Value> {
    use data::default_panel::{CHARACTERS, ITEMS, STATUS, TRACKING, TRACKING_LISTS};
    use data::StateNode;
    if !state.mechanism.default_panel {
        return None;
    }
    fn children(
        node: Option<&StateNode>,
    ) -> Option<&std::collections::BTreeMap<String, StateNode>> {
        match node {
            Some(StateNode::Branch(children)) => Some(children),
            _ => None,
        }
    }
    let list = |node: Option<&StateNode>| -> Value {
        let mut items: Vec<(usize, &str)> = children(node)
            .into_iter()
            .flatten()
            .filter_map(|(key, item)| match (key.parse::<usize>(), item) {
                (Ok(index), StateNode::Leaf(text)) => Some((index, text.as_str())),
                _ => None,
            })
            .collect();
        items.sort_by_key(|(index, _)| *index);
        json!(items.into_iter().map(|(_, text)| text).collect::<Vec<_>>())
    };
    let tree = &state.state.tree;
    let mut characters = serde_json::Map::new();
    for (key, cell) in children(tree.get(CHARACTERS)).into_iter().flatten() {
        let Some(fields) = children(Some(cell)) else {
            continue;
        };
        let mut out = serde_json::Map::new();
        for (name, node) in fields {
            match (name.as_str(), node) {
                (ITEMS, _) => {
                    out.insert(name.clone(), list(Some(node)));
                }
                (_, StateNode::Leaf(text)) if !name.starts_with('_') => {
                    out.insert(name.clone(), json!(text));
                }
                _ => {}
            }
        }
        out.entry(STATUS).or_insert_with(|| json!(""));
        out.entry(ITEMS).or_insert_with(|| json!([]));
        characters.insert(key.clone(), Value::Object(out));
    }
    let tracking = children(tree.get(TRACKING));
    let tracking: serde_json::Map<String, Value> = TRACKING_LISTS
        .iter()
        .map(|key| ((*key).to_owned(), list(tracking.and_then(|t| t.get(*key)))))
        .collect();
    Some(json!({ "characters": characters, "tracking": tracking }))
}

/// 房間狀態列：提交後的平欄三鍵。沒有的鍵送空字串（清空）。
pub(crate) fn scene_status(table: &std::collections::BTreeMap<String, String>) -> Value {
    let value = |key: &str| table.get(key).cloned().unwrap_or_default();
    json!({ "time": value("time"), "place": value("place"), "present": value("present") })
}

impl GmNarration {
    /// 長文字欄變動紀錄的本文（房間執行端代前端落 `state_update` 事件用）；空字串＝這輪沒有
    pub(crate) fn state_update_text(&self) -> String {
        let lines: Vec<String> = self.state_updates.iter().map(StateUpdate::line).collect();
        lines.join("\n")
    }
}

/// 簡易導演：GM 旁白＋點名一次呼叫完成（NewPlan §6.1＋快取包 5）。
/// 旁白串流回前端後由前端落 transcript；點名行與狀態欄在這裡剝掉，不進顯示文字。
pub(crate) async fn gm_narrate(
    app: tauri::AppHandle,
    world_id: String,
    turn_id: String,
    on_delta: tauri::ipc::Channel<String>,
    action_id: Option<String>,
    room: bool,
) -> Result<GmNarration, String> {
    let _permit = data::world_write_permit_async(&world_id).await?;
    let (_guard, mut cancel) = inflight::register_turn(&world_id, &turn_id);
    let buffer = std::sync::Mutex::new(String::new());
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let root = data_root(&app)?;
    // 完結的 VN 桌不再續寫（包 5 §2.6）
    data::vn::refuse_if_ended(&root, &world_id)?;
    // 換幕容量鎖：開回合紀錄之前擋，擋下就什麼都不寫
    crate::scene_budget::check_capacity(
        &config_root(&app)?,
        &root,
        &world_id,
        action_id.as_deref(),
        "",
    )?;
    // 預設狀態欄（table-status-panel §2.1）：沒卡格式的桌種預設欄位、對照角色格；出錯只記，不擋回合
    if let Err(error) = data::default_panel::ensure(&root, &world_id) {
        log::warn!("預設狀態欄種入失敗：{error}");
    }
    // 回合開始（鎖內）：記回合紀錄與固定輸入；中止、出錯時 TurnGuard 把它改成已中止（計畫 8.3）
    let turn = TurnGuard::begin(&root, &world_id, &turn_id, &lang)?;
    let dice_pool = crate::dice::begin(&root, &world_id, &turn_id)?;
    let mut materials = gm_materials(&root, &world_id)?;
    materials
        .party
        .hand_over(&materials.events, action_id.as_deref());
    // 世界書（方案三之 2）：回合交接（begin 裡）之後先結算上一次落地，再以 GM 視角掃一次；
    // 掃描結果供組裝、導演指示點名格式條目、落地三者共用。落地之後的確定失敗由 TurnGuard（finish_turn）撤回
    let scene = materials.state.current_scene;
    let timed = world_scan::landing::before_scan(
        &root,
        &world_id,
        scene,
        &turn_id,
        &data::world_info_store::Perspective::Gm,
    )?;
    let path = crate::scene_budget::path_limits(
        &config_root(&app)?,
        &root,
        &config,
        transport::gm_tier(&config),
    );
    let scan = chat_assembly::gm_scan(
        &materials,
        &materials.events,
        timed,
        path.budget,
        world_scan::MacroInputs::load(
            &root,
            &world_id,
            &data::world_info_store::Perspective::Gm,
            path.model.clone(),
            path.limits,
            true,
        ),
        world_scan::Randomness::Live,
        &lang,
    );
    let player_name = materials.player.as_ref().map(|card| card.name.as_str());
    // 換幕後第一輪 GM 回合送整棵樹對齊；之後每輪只送在場分支＋變動標記（狀態欄二期包 5）。
    let (scope, align) = chat_assembly::gm_scope(&materials);
    let (mut instruction_message, closing) =
        chat_assembly::gm_instruction(&root, &world_id, &materials, &scan, &lang);
    if let Some(pool) = dice_pool {
        instruction_message.content.push_str(&format!(
            "\n{}",
            crate::dice::instruction(&pool, room, &lang)
        ));
    }
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
            &turn_id,
            &materials,
            &scan,
            &scope,
            &instruction,
            lanes::ReplyEcho::Narration,
            &lang,
            provider,
            path.budget,
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
        let messages =
            chat_assembly::gm_messages(&materials, &scan, &scope, instruction_message, &lang);
        world_scan::landing::land(
            &root,
            &world_id,
            scene,
            &turn_id,
            &data::world_info_store::Perspective::Gm,
            &scan,
        )?;
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
                    roster: chat_assembly::speakers(&materials.cards).count(),
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
    let GmReplyParts {
        block,
        next_raw,
        display,
        raw,
        pending,
        echo_patch,
    } = gm_reply_parts(
        &reply,
        &materials.state.state.tree,
        materials.state.statusbar_retired && !materials.takeover,
    );
    // 剝掉 state／next 控制欄後沒有正文＝這一輪沒東西寫進故事。必須擋在下面的
    // apply_block 之前：狀態套用只要 incremental 為真就必跑，失敗回合會白白重擲一輪骰
    // （stream-failure-visible）。CLI 那條路沒有 stream_chat 的收工判定，這裡是唯一防線。
    if display.trim().is_empty() {
        return Err(transport::empty_reply_error(&reply));
    }
    let mut state_updates: Vec<StateUpdate> = Vec::new();
    let mut arrived_persons = Vec::new();
    let mut arrived_characters = Vec::new();
    let user_name = player_name.unwrap_or_else(|| transport::player_fallback_name(&lang));
    // 提交（鎖內、核對 turn_id、幕與世代）：套在回合開始時固定的輸入上；變數模式時新表與正文留在回合紀錄
    // 等前端落檔。狀態沒寫成要明確回報（state_error），不吞掉
    let truncated = cut.load(std::sync::atomic::Ordering::Relaxed);
    // VN 桌：章末／結局標記在提交閉包內依套完的狀態驗（包 5 §2.2），正文照判定改寫
    let vn = chat_assembly::vn_table(&materials.state);
    let endings = vn_endings(materials.vn_bible.as_ref());
    let pending_main = |verdict: Option<&vn_ending::Verdict>| data::message_vars::PendingMain {
        text: verdict.map_or_else(|| display.clone(), |verdict| verdict.rewrite(&display)),
        raw: raw
            .as_deref()
            .map(|raw| verdict.map_or_else(|| raw.to_owned(), |verdict| verdict.rewrite(raw))),
        truncated,
    };
    // 變動紀錄與正文在同一次提交登記：前端沒落成時跟正文一起代落（長欄位變動靠它進歷史）
    let mut sides = |state: &data::WorldState, outcome: &Option<mechanism::Outcome>| {
        if outcome.is_none() {
            return Vec::new();
        }
        state_updates = state_update_lines(state, user_name, &pending);
        state_update_side(&state_updates).into_iter().collect()
    };
    // zod 卡（mvu-zod-semantics）：卡片 schema 與共用 MVU 核心在隔離引擎裡跑，三段提交見 mvu_engine::wiring
    let committed = match crate::mvu_engine::wiring::zod_table(&root, &world_id) {
        Some(zod) => {
            let engine_reply = match &echo_patch {
                Some(patch) => format!("{reply}\n<UpdateVariable>{patch}</UpdateVariable>"),
                None => reply.clone(),
            };
            // zod 引擎桌不驗結局：`[ending]` 一律剝（已知限制）
            let verdict = vn.then(vn_ending::Verdict::unjudged);
            let main = pending_main(verdict.as_ref());
            turn.commit_engine(zod, main, &engine_reply, &block, user_name, sides)
                .await
                .map(|commit| {
                    commit.map(|commit| data::message_vars::GmCommit {
                        state: commit.state,
                        result: (commit.result, verdict),
                        error: commit.error,
                    })
                })
        }
        None => turn.commit(
            |(_, verdict): &(_, Option<vn_ending::Verdict>)| pending_main(verdict.as_ref()),
            |state, stat| {
                if vn {
                    // VN 桌：引擎三鍵的規則（沒聖經的桌第一次補），表內但樹上缺的旗標、揭露、章號補成初值
                    data::vn::ensure_engine_rules(&mut state.mechanism);
                    data::vn::fill_missing(&mut state.state.tree, &materials.vn_seeds);
                }
                let (outcome, typed) = gm_commit_apply(
                    state,
                    stat,
                    &block,
                    user_name,
                    &materials.state.mechanism,
                    materials.takeover,
                    align,
                );
                let verdict = vn.then(|| {
                    let tree = &state.state.tree;
                    let verdict = vn_ending::Judge {
                        endings: &endings,
                        chapter: data::vn::chapter_in(tree),
                        max: vn_chapter_max(materials.vn_bible.as_ref()),
                        finale: data::vn::finale_in(tree),
                        tree,
                        mechanism: &state.mechanism,
                    }
                    .judge(&display);
                    verdict.apply(&mut state.state);
                    verdict
                });
                ((outcome, verdict), typed)
            },
            move |state: &data::WorldState, (outcome, _): &(_, _)| sides(state, outcome),
        ),
    };
    let mut state_error = None;
    let mut verdict = None;
    let mut status = None;
    let mut panel = None;
    let committed = match committed {
        Ok(Some(commit)) => {
            if commit.error.is_none() {
                status = Some(scene_status(&commit.state.state.table));
                panel = scene_panel(&commit.state);
            }
            state_error = commit.error;
            verdict = commit.result.1;
            Some((commit.state, commit.result.0))
        }
        Ok(None) => None,
        Err(error) => {
            state_error = Some(error);
            None
        }
    };
    let reply = verdict
        .as_ref()
        .map_or(reply.clone(), |verdict| verdict.rewrite(&reply));
    let display = verdict
        .as_ref()
        .map_or(display.clone(), |verdict| verdict.rewrite(&display));
    if let Some((state, Some(outcome))) = committed {
        let scene = state.current_scene;
        mechanism::append_log(&root, &world_id, scene, &outcome.records);
        let present = state.state.table.get("present").map(String::as_str);
        // 登場事件的本文以 GM 視角中性代換一次寫死（不執行副作用，方案三之 7）
        // 公開事件用公開環境（只讀公開設定、角色側事件、不帶 outlet），GM 專屬事件才看得到私設與 outlet
        let gm = chat_assembly::gm_neutral_session(&root, &world_id, &materials, &path, &lang);
        let public = chat_assembly::gm_neutral_session(&root, &world_id, &materials, &path, &lang)
            .publicized(&materials.events, &lang);
        let fill = |text: &str, card: Option<&data::CharacterCard>, gm_only: bool| match gm_only {
            true => gm.fill(text, card, true),
            false => public.fill(text, card, false),
        };
        // 人物在場登場（AI 卡重構包 4a）：present 套用後檢查新面孔，
        // 命中就把世界書全文記進歷史，system 那邊只留一行名冊。
        // 精簡條目照編輯器顯示順序（登場事件的先後跟著它）；讀不到就不記登場（本來就盡力而為）
        let worldbook = data::read_worldbook(&root, &world_id).unwrap_or_else(|error| {
            log::warn!("偵測人物登場時讀世界書失敗，這輪不記登場：{error}");
            Vec::new()
        });
        let cards = load_hidden_cards(&root, &world_id, materials.strip_greetings);
        // 整桌人物名字（角色卡＋人物條目）一個候選池判折疊歧義，人物登場與卡回歸互不搶錯人
        let card_names: Vec<String> = match &cards {
            Ok((hidden, others)) => hidden
                .iter()
                .map(|card| card.name.clone())
                .chain(others.iter().cloned())
                .collect(),
            Err(_) => Vec::new(),
        };
        arrived_persons = record_person_arrivals(
            &root,
            &world_id,
            scene,
            &worldbook,
            &card_names,
            &materials.events,
            present,
            &display,
            &fill,
            Some(&turn.ticket),
            action_id.as_deref(),
        );
        // 角色卡自動回歸（AI 卡重構包 4b）：鏡射人物登場，鍵換成卡名；
        // auto_hidden 欄位本身不在這裡動，只在換幕結算（data::begin_next_scene）與退回／分岔復原。
        if let Ok((hidden_cards, mut other_names)) = cards {
            other_names.extend(
                worldbook
                    .iter()
                    .filter(|entry| entry.is_person && !entry.disabled)
                    .map(|entry| entry.title.clone()),
            );
            arrived_characters = record_card_arrivals(
                &root,
                &world_id,
                scene,
                &hidden_cards,
                &other_names,
                &materials.events,
                present,
                &display,
                &fill,
                Some(&turn.ticket),
                action_id.as_deref(),
            );
        }
    }
    let (next, next_player) = chat_assembly::gm_next_speaker(&materials, next_raw);
    // VN 桌：選項／章末／結局任一出現就停接力（不點名、不寫點名列）
    let (next, vn_stop) = match chat_assembly::vn_table(&materials.state) {
        true => {
            let roster = vn_script::VnRoster::new(
                &materials.metas,
                materials.player.as_ref(),
                materials.vn_card.as_ref(),
            );
            let finale = verdict.as_ref().and_then(vn_ending::Verdict::stop);
            vn_script::gm_stop(&display, &roster, next, finale)
        }
        false => (next, None),
    };
    let next_player = next_player.filter(|_| vn_stop.is_none());
    Ok(GmNarration {
        raw: (reply != display).then_some(reply),
        text: display,
        next,
        next_player,
        vn_stop,
        state_updates,
        arrived_characters,
        arrived_persons,
        aborted: false,
        truncated,
        state_error,
        status,
        panel,
    })
}

/// 結局表（凍結聖經的；沒聖經＝只有引擎的 `default`）。
fn vn_endings(frozen: Option<&data::vn::Frozen>) -> Vec<data::vn::Ending> {
    frozen.map_or_else(
        || {
            vec![data::vn::Ending {
                id: data::vn::DEFAULT_ENDING.to_owned(),
                title: String::new(),
                when: Vec::new(),
            }]
        },
        |frozen| frozen.bible.endings.clone(),
    )
}

/// 章數上限 M（凍結聖經的；沒聖經＝保底值）。
pub(super) fn vn_chapter_max(frozen: Option<&data::vn::Frozen>) -> u32 {
    frozen.map_or(data::vn::CHAPTER_FALLBACK, |frozen| {
        frozen.bible.chapter_max
    })
}

#[cfg(test)]
#[path = "gm_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "gm_panel_tests.rs"]
mod gm_panel_tests;
