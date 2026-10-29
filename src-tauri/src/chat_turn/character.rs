//! 角色接話回合。
use super::stream::{prepare_play_lane_call, push_delta, take_abort_or_finish};
use crate::chat_assembly;
use crate::transport::dispatch::{ai_call_failure, lane_provider, stream_turn_via_transport};
use crate::usage::log as usage_log;
use crate::{config_root, data, data_root, inflight, lanes, transport, world_scan};
use serde::Serialize;

/// 角色對話的回傳。`aborted` 為真時 `text` 是停止當下已經吐出的半截，可能是空的。
#[derive(Serialize)]
pub(crate) struct ChatReply {
    text: String,
    /// 剝殼前的台詞原文（已剝本輪前綴）；與 text 相同就是 None，前端照樣存進事件裡
    raw: Option<String>,
    aborted: bool,
}

/// 角色回覆收尾的接線層：純函式剝前綴與控制區塊，完成回合剝完沒正文就回錯
/// （中止沒正文照舊 Ok＋aborted，由 settle 撤回世界書落地）。
fn character_reply(reply: &str, prefix: &str, aborted: bool) -> Result<ChatReply, String> {
    let finished = transport::finish_character_reply(reply, prefix, aborted);
    if !aborted && finished.text.is_empty() {
        return Err(transport::empty_reply_error(reply));
    }
    Ok(ChatReply {
        text: finished.text,
        raw: finished.raw,
        aborted: finished.aborted,
    })
}

/// 上下文組裝→單發呼叫→串流回傳（KICKOFF §4）。
/// 上下文完全由本機正典（角色卡＋可見世界書＋公開 transcript）經 assemble_messages 組裝，
/// 再依 preferences.transport 分流到 API 或 CLI；增量文字經 on_delta channel 回前端。
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
    // 介面接管桌只走 GM：擋在容量記帳與代落之前，被拒什麼都不寫
    chat_assembly::reject_character_turn_on_takeover(&root, &world_id)?;
    // 舞台卡不發言：同樣擋在任何副作用之前
    chat_assembly::reject_stage_character_turn(&root, &world_id, &character_id)?;
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
    let scene = state.current_scene;
    let events =
        data::read_transcript(&root, &world_id, scene).map_err(|error| error.to_string())?;
    // 開場白政策這一輪只算一次：本角色的卡與桌上全卡共用
    let takeover = data::is_interface_takeover(&root, &world_id, state.refactor_mode.as_deref());
    let strip =
        chat_assembly::greeting::policy(&root, &world_id, &state, &events, takeover).strip_cards;
    let card = chat_assembly::prompt_card(card, strip);
    let player = data::read_player_card(&root, &world_id).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let cards = chat_assembly::active_cards_with(&root, &world_id, strip)?;
    let prompt_regex = chat_assembly::prompt_regex(&root, &world_id);
    // 世界書（方案三之 2）：交接之後先結算上一次落地，再以這個角色的視角掃一次；結果供組裝與落地共用
    let perspective = data::world_info_store::Perspective::Character(card.id.clone());
    let timed = world_scan::landing::before_scan(&root, &world_id, scene, &turn_id, &perspective)?;
    let book = world_scan::TableBook::load(&root, &world_id)?;
    let metas = data::list_characters(&root, &world_id).map_err(|error| error.to_string())?;
    let path = crate::scene_budget::path_limits(&config_root(&app)?, &root, &config, card.tier);
    let scan = chat_assembly::character_scan(
        &book,
        &card,
        &cards,
        chat_assembly::sole_card_name(&metas, &events).as_deref(),
        player.as_ref(),
        &events,
        timed,
        path.budget,
        world_scan::MacroInputs::load(
            &root,
            &world_id,
            &perspective,
            path.model.clone(),
            path.limits,
            true,
        ),
        world_scan::Randomness::Live,
        &lang,
    );
    // 角色自己那支的狀態（面板指認優先，其次同名比對），只有這條分支會塞進提示詞。
    let branch = transport::resolve_branch(
        &state.state.tree,
        &state.branch_bindings,
        &character_id,
        &card.name,
    );
    // 模型照歷史格式自加的本輪「名字：」：畫面串流、落檔與 lane 預期回聲都剝掉（char-line-prefix）
    let own_prefix = transport::speaker_prefix(&card.name, &transport::ui_language(&config));
    // 呼叫回錯、或中止時一個字都沒有＝確定失敗，當場撤回這回合的世界書落地；有正文的等前端落檔清掉
    // （呼叫回錯時沒還原的變數寫入連同原錯回報；中止沒字沒有錯可附，寫進待回報檔）
    let settle = |result: Result<ChatReply, String>| -> Result<ChatReply, String> {
        use data::world_info_store::Report;
        match result {
            Err(error) => {
                let kept =
                    world_scan::landing::fail(&root, &world_id, scene, &turn_id, Report::Inline);
                Err(world_scan::landing::with_kept(error, &kept))
            }
            Ok(reply) => {
                if reply.aborted && reply.text.trim().is_empty() {
                    world_scan::landing::fail(&root, &world_id, scene, &turn_id, Report::Notices);
                }
                Ok(reply)
            }
        }
    };
    // CLI 訂閱走 resume 續聊線。claude／grok 全角色共用一條 session，私設回合注入、
    // 回合後從 session 檔抹掉（案 C）；Agy 無抹寫路徑，改成一角一線＋
    // 私設提進該角色自己的凍結 system，不讓別的角色讀到不該讀的東西。
    // claude 單角色桌不抹：私設進 system，session 記成這個角色的未抹線（claude-resume-tail-cache）。
    if let Some(provider) = lane_provider(&config) {
        let sole = provider == lanes::LaneProvider::Claude
            && chat_assembly::sole_present_character(&root, &world_id, &card.id, &events)?;
        let shape = lanes::chars_lane_shape(provider, sole);
        let (frozen, turn) = chat_assembly::character_lane_parts(
            &card,
            &cards,
            player.as_ref(),
            &scan,
            &state,
            branch.as_deref(),
            &lang,
            shape.hoist_private,
        );
        let call = prepare_play_lane_call(&app, &config, card.tier, provider).await?;
        world_scan::landing::land(&root, &world_id, scene, &turn_id, &perspective, &scan)?;
        // 串流過濾在 lane 內按 attempt 做，這裡收的已是過濾後的字
        let emit = |delta: &str| push_delta(&buffer, &on_delta, delta);
        let outcome = lanes::run_turn(
            &call,
            &root,
            &world_id,
            lanes::TurnInput {
                lane: lanes::Lane::Chars,
                scene,
                events: &events,
                lang: &lang,
                frozen_system: frozen,
                tail: turn.tail,
                confidential: shape.erase.then_some(turn.confidential).flatten(),
                prefix: shape
                    .prefix
                    .then(|| transport::speaker_prefix(&card.name, &lang)),
                echo: lanes::ReplyEcho::Dialogue {
                    speaker_id: card.id.clone(),
                    prefix: own_prefix.clone(),
                },
                scope: shape.scope_by_card.then(|| card.id.clone()),
                single_owner: shape.single_owner.then(|| card.id.clone()),
                has_state_block: turn.has_state_block,
                hoisted_worldbook: turn.hoisted_worldbook,
                tail_worldbook: turn.tail_worldbook,
                // Hoist::All 的線世界書在 system，尾段沒有世界書，不累計（看線的形狀，不看欄位巧合）
                tail_tracked: !shape.hoist_private,
                opening_raw: false,
                capacity: path.budget,
                prompt_regex,
            },
            Some(&mut cancel),
            emit,
        )
        .await
        .map_err(ai_call_failure);
        // 半截用這一輪 CLI 自己累的字。外層緩衝會跨降級重開留著前一次嘗試的增量。
        return settle(
            outcome
                .and_then(|outcome| character_reply(&outcome.text, &own_prefix, outcome.aborted)),
        );
    }
    // api／codex 走共線組裝（api-shared-lane 包 B）：全角色共用一份與「這輪是誰」
    // 無關的前綴，本輪指定在尾端那則 user。attendant_label 與 closing 傳空字串，因為這份
    // messages 已經自足——台詞自帶名字前綴、指示已在尾端，補了會重複（見 cli::flatten_messages）。
    let messages = transport::assemble_shared_messages(
        &card,
        &cards,
        player.as_ref(),
        &events,
        &scan,
        &state.state,
        &state.mechanism,
        branch.as_deref(),
        &lang,
        &prompt_regex,
    );
    // roster 記的是套用策略前的有效角色數，不是實際帶進組裝器的張數——沒有這個數字，
    // 日後零命中退回（no-cache-model-optout）產生的 solo 就跟天然單角色桌長得一樣
    // buffer 收原文（中止半截再從原文剝一次）；畫面只收過濾後的字
    let mut stream = transport::OwnPrefixStream::new(&own_prefix);
    let emit = |delta: &str| {
        buffer.lock().expect("delta buffer").push_str(delta);
        let out = stream.push(delta);
        if !out.is_empty() {
            let _ = on_delta.send(out);
        }
    };
    world_scan::landing::land(&root, &world_id, scene, &turn_id, &perspective, &scan)?;
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
                roster: chat_assembly::speakers(&cards).count(),
                solo: chat_assembly::speakers(&cards).count() <= 1,
            },
            false,
            emit,
        ),
    )
    .await;
    let held = stream.finish();
    if spoken.is_ok() && !held.is_empty() {
        let _ = on_delta.send(held);
    }
    settle(spoken.and_then(|spoken| character_reply(&spoken.text, &own_prefix, spoken.aborted)))
}

#[cfg(test)]
#[path = "character_tests.rs"]
mod tests;
