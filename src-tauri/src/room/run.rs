//! 派工與工作（協定 §6.3–§6.5、§7.1–§7.3）：派工 → 落行動批次 → GM 鏈接力（照前端 `useChatController` 的
//! 規則：旁白 → 點到 NPC 就接話 → 直到點到真人玩家、沒點名或達 `max_round_speakers`）→ 每段 `gen.part.*`
//! → `gen.done`；換幕摘要工作；遠端控制。桌上落檔照前端原本的落法（旁白帶回合鍵、點名列、角色回覆）。
use super::{seed, Delta, DispatchRun, JobRun, Room, RunState, Step};
use crate::chat_turn::{self, BatchOutcome, DepartureMode, PartyAction, PartyBatch};
use crate::data::{self, EventMarker, TranscriptEvent, TranscriptKind};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_owned()
}

fn scene_of(root: &Path, world: &str) -> Result<u64, String> {
    data::read_state(root, world)
        .map(|state| state.current_scene)
        .map_err(|error| error.to_string())
}

fn settings_of(settings: &Value) -> Option<(String, String)> {
    Some((
        settings["route"].as_str()?.to_owned(),
        settings["model"].as_str()?.to_owned(),
    ))
}

pub(super) fn status_frame(room: &Room) -> Value {
    let busy = {
        let work = room.work();
        work.dispatch.is_some() || work.job.is_some()
    };
    let route = super::cards::gen_route(room);
    let image_gen = route && !room.saved().image_gen_off;
    json!({ "t": "status", "p": { "keepAwake": room.status().keep_awake, "busy": busy, "imageGen": image_gen, "imageGenRoute": route } })
}

/// 狀態上送（額度協定允許不報：撞額度由 `gen.failed` 的 quota 類錯誤帶）；面板的目前模型跟著 capabilities
fn report(room: &Room) {
    let caps = room.engine.capabilities();
    room.status().model_label = caps["current"]["modelLabel"].as_str().map(str::to_owned);
    let _ = room.out.send(status_frame(room));
}

// ── 錯誤分類（協定 §7.1）──

/// 錯誤字串的代碼：`TTMSG:` 的 code 或冒號前的大寫前綴；認不出就空字串
fn code_of(error: &str) -> String {
    match error.strip_prefix(crate::ui_msg::MARK) {
        Some(json) => {
            serde_json::from_str::<Value>(json).map_or(String::new(), |v| text(&v["code"]))
        }
        None => error
            .split(':')
            .next()
            .filter(|head| {
                head.chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
            })
            .unwrap_or_default()
            .to_owned(),
    }
}

type Kind = (&'static str, &'static [&'static str], &'static str);

fn kind_of(code: &str) -> Kind {
    let has = |needles: &[&str]| needles.iter().any(|needle| code.starts_with(needle));
    if has(&["AI_CONTENT_FILTERED"]) {
        (
            "refusal",
            &["switch_route", "switch_model", "retry", "continue"],
            "The provider refused this GM turn.",
        )
    } else if has(&[
        "AI_HTTP_STATUS_429",
        "AI_HTTP_STATUS_402",
        "AI_FREE_MODEL_BUSY",
    ]) {
        (
            "quota",
            &["switch_route", "switch_model", "retry"],
            "The GM quota or rate limit was hit.",
        )
    } else if has(&["scene_capacity_full", "AI_CONTEXT_TOO_LONG"]) {
        (
            "scene_full",
            &["scene_advance"],
            "This scene is full; advance to the next scene.",
        )
    } else if has(&["AI_HTTP_STATUS_404", "cli_model_unavailable"]) {
        // 模型下架／叫不到：重試沒用，等出額度者換模型（重生或切換）
        (
            "model_unavailable",
            &["switch_model", "switch_route", "retry"],
            "The GM model is unavailable; pick another model.",
        )
    } else if has(&[
        "cli_not_found",
        "cli_risk_not_accepted",
        "openrouter_api_key_missing",
        "codex_auth_missing",
        // 已知限制：403 也可能是地區或內容被擋，這時「金鑰或登入失效」會誤導
        "AI_HTTP_STATUS_401",
        "AI_HTTP_STATUS_403",
    ]) {
        (
            "not_ready",
            &["retry"],
            "The GM host needs setup on the computer.",
        )
    } else if has(&[
        "cli_too_old",
        "cli_output_unrecognized",
        "cli_reply_error",
        "cli_no_reply",
    ]) {
        (
            "tool_format",
            &["retry", "switch_route"],
            "The command-line tool or its output format needs attention.",
        )
    } else if has(&[
        // 回應半截、失控、空白、CLI 正在更新：重生或重試就會好，不叫人去設定
        "AI_INCOMPLETE_RESPONSE",
        "AI_OUTPUT_RUNAWAY",
        "AI_EMPTY_RESPONSE",
        "cli_updating",
        "AI_CALL_FAILED",
        "AI_STREAM_STALLED",
        "AI_HTTP_STATUS_",
        "cli_stalled",
        "cli_crashed",
        "cli_turn_failed",
    ]) {
        (
            "provider_unreachable",
            &["retry", "switch_route"],
            "The GM provider could not be reached.",
        )
    } else {
        ("other", &["retry", "continue"], "The GM host hit an error.")
    }
}

/// 桌面版錯誤字串 → `ExecutorError`；說明不帶回應本文與路徑。回合錯誤被 `ai_call_failure` 包成
/// `AI_CALL_FAILED: TTMSG:…` 時照內層的碼分（比照前端 `ai-error.ts`），內層認不出才用外層
pub(super) fn classify(error: &str) -> Value {
    let mut code = code_of(error);
    let mut kind = kind_of(&code);
    if code == "AI_CALL_FAILED" {
        let inner = error
            .split_once(':')
            .map_or("", |(_, rest)| rest.trim_start());
        if inner.starts_with(crate::ui_msg::MARK) {
            let inner_code = code_of(inner);
            let inner_kind = kind_of(&inner_code);
            if inner_kind.0 != "other" {
                (code, kind) = (inner_code, inner_kind);
            }
        }
    }
    if code.is_empty() {
        code = "OTHER".to_owned();
    }
    let (kind, actions, message) = kind;
    json!({
        "kind": kind, "code": code.chars().take(64).collect::<String>(),
        "message": message, "actions": actions, "resetsAt": null,
    })
}

// ── 派工 ──

pub(super) fn on_dispatch(room: &Arc<Room>, dispatch: &Value) {
    let id = text(&dispatch["dispatchId"]);
    if id.is_empty() {
        return;
    }
    // 同一筆、已做完或失敗：只靠重送佇列（協定 §6.5），不回 ack——重連時 ack 排在重送的 gen.done 之後，
    // 伺服器已開下一回合，ack 只會換來 E_DISPATCH_UNKNOWN
    let finished = matches!(room.saved().dispatch, Some(run) if run.id == id && run.state != RunState::Running);
    if finished {
        return;
    }
    // 先回 ack 再動手（伺服器以此把 queued 轉 generating）
    room.send("dispatch.ack", json!({ "dispatchId": id }));
    if room.work().dispatch.as_deref() == Some(id.as_str()) {
        return;
    }
    match room.saved().dispatch {
        // 同一筆、本機還在 running（程序重啟）：從 lastEndedPart+1 續跑
        Some(run) if run.id == id => {}
        _ => room.update(|saved| {
            saved.settings = settings_of(&dispatch["settings"]).or(saved.settings.take());
            saved.dispatch = Some(DispatchRun {
                id: id.clone(),
                state: RunState::Running,
                last_ended: None,
                used: 0,
                step: Step::Land,
                arrived: Vec::new(),
                called: None,
                dispatch: dispatch.clone(),
            });
        }),
    }
    spawn_chain(room);
}

fn spawn_chain(room: &Arc<Room>) {
    let room = room.clone();
    tokio::spawn(async move { chain(&room).await });
}

async fn chain(room: &Arc<Room>) {
    let Some(run) = room.saved().dispatch else {
        return;
    };
    {
        let mut work = room.work();
        if work.dispatch.is_some() {
            return;
        }
        work.dispatch = Some(run.id.clone());
        work.abort = false;
    }
    room.use_settings();
    report(room);
    let outcome = steps(room, &run.id).await;
    let world = room.world();
    match outcome {
        Ok(aborted) => {
            let (arrived, called) = room
                .saved()
                .dispatch
                .map(|run| (run.arrived, run.called))
                .unwrap_or_default();
            let mut p = json!({ "dispatchId": run.id, "aborted": aborted });
            if !arrived.is_empty() {
                p["cardsArrived"] = json!(arrived);
            }
            if let Some(card) = called {
                p["called"] = json!(card);
            }
            room.durable("gen.done", p, |saved| {
                set_state(saved, &run.id, RunState::Done)
            });
        }
        Err((at_part, message)) => {
            let error = classify(&message);
            room.note_gm_error(
                text(&error["kind"]),
                &format!("{}：{message}", text(&error["code"])),
            );
            let p = json!({ "dispatchId": run.id, "atPart": at_part, "error": error });
            room.durable("gen.failed", p, |saved| {
                set_state(saved, &run.id, RunState::Failed)
            });
        }
    }
    {
        let mut work = room.work();
        work.dispatch = None;
        work.turn = None;
    }
    report(room);
    room.engine.table_changed(&world);
    // 收尾期間又來了新的派工（它看到這支還在跑就沒開工）：接著跑
    if room
        .saved()
        .dispatch
        .is_some_and(|next| next.id != run.id && next.state == RunState::Running)
    {
        spawn_chain(room);
    }
}

fn set_state(saved: &mut super::Saved, id: &str, state: RunState) {
    if let Some(run) = saved.dispatch.as_mut().filter(|run| run.id == id) {
        run.state = state;
    }
}

fn change_run(room: &Room, change: impl FnOnce(&mut DispatchRun)) {
    room.update(|saved| {
        if let Some(run) = saved.dispatch.as_mut() {
            change(run);
        }
    });
}

/// 跑到鏈尾：Ok(是否中止)；Err((失敗那段的段號, 錯誤))。房主 GM 鈕（`dispatch.host.mode`）照單人版三顆鈕：
/// narrate＝旁白一次、不寫點名列（點到的 NPC 記進 `called`）；call＝直接請那張卡接一句（`call` 先寫點名列）；advance＝一般鏈
async fn steps(room: &Arc<Room>, id: &str) -> Result<bool, (Option<u32>, String)> {
    let world = room.world();
    let (config_root, root) = room.engine.roots();
    let max = room.engine.capabilities()["maxRoundSpeakers"]
        .as_u64()
        .unwrap_or(3)
        .max(1) as u32;
    loop {
        let Some(run) = room.saved().dispatch.filter(|run| run.id == id) else {
            return Ok(false);
        };
        let host = &run.dispatch["host"];
        let mode = host["mode"].as_str().unwrap_or_default();
        let part = run.last_ended.map_or(0, |n| n + 1);
        if room.work().abort && run.step != Step::Land {
            return Ok(true);
        }
        match run.step {
            Step::Land => {
                // 同一派工重送：Duplicate 就不重落玩家事件（room.json 遺失時 GM 鏈會重跑，列已知限制）
                let outcome = land_batch(&config_root, &root, &world, &run.dispatch)
                    .map_err(|error| (None, error))?;
                let next = match mode {
                    "call" => {
                        let card = text(&host["cardId"]);
                        if host["call"] == true && !matches!(outcome, BatchOutcome::Duplicate) {
                            let name = data::read_character(&root, &world, &card)
                                .map(|card| card.name)
                                .map_err(|error| (None, error.to_string()))?;
                            append_call(&root, &world, id, name).map_err(|error| (None, error))?;
                        }
                        Step::Reply(card)
                    }
                    _ => Step::Narrate,
                };
                change_run(room, |run| run.step = next);
                room.engine.table_changed(&world);
            }
            Step::Narrate if run.used >= max => change_run(room, |run| run.step = Step::Done),
            Step::Narrate => {
                let turn = begin_turn(room);
                let io = Part::new(room, id, part, json!({ "kind": "gm" }));
                let n = room
                    .engine
                    .narrate(world.clone(), turn.clone(), id.to_owned(), io.delta())
                    .await
                    .map_err(|error| (Some(part), error))?;
                let only = mode == "narrate";
                let next = land_narration(&root, &world, id, &turn, &n, !only)
                    .map_err(|error| (Some(part), error))?;
                let (next, called) = match next {
                    Step::Reply(card) if only => (Step::Done, Some(card)),
                    next => (next, None),
                };
                let arrived = n.arrived.clone();
                let status = n.status.filter(|_| !n.aborted);
                io.end(&n.text, n.aborted, status.as_ref(), move |run| {
                    run.used += 1;
                    run.step = next;
                    run.arrived.extend(arrived);
                    run.called = called.or(run.called.take());
                });
                room.engine.table_changed(&world);
                if n.aborted {
                    return Ok(true);
                }
            }
            Step::Reply(card) => {
                let name = data::read_character(&root, &world, &card)
                    .map(|card| card.name)
                    .map_err(|error| (Some(part), error.to_string()))?;
                ensure_in_roster(room, &card);
                let speaker = json!({ "kind": "npc", "cardId": card, "name": name.chars().take(64).collect::<String>() });
                let turn = begin_turn(room);
                let io = Part::new(room, id, part, speaker);
                let spoken = room
                    .engine
                    .reply(
                        world.clone(),
                        card.clone(),
                        turn.clone(),
                        id.to_owned(),
                        io.delta(),
                    )
                    .await
                    .map_err(|error| (Some(part), error))?;
                land_reply(&root, &world, id, &turn, &card, &name, &spoken)
                    .map_err(|error| (Some(part), error))?;
                let next = if spoken.aborted || run.used >= max || mode == "call" {
                    Step::Done
                } else {
                    Step::Narrate
                };
                io.end(&spoken.text, spoken.aborted, None, move |run| {
                    run.step = next
                });
                room.engine.table_changed(&world);
                if spoken.aborted {
                    return Ok(true);
                }
            }
            Step::Done => return Ok(false),
        }
    }
}

fn begin_turn(room: &Room) -> String {
    let turn = data::new_id();
    room.work().turn = Some(turn.clone());
    turn
}

/// NPC 段的說話者必須在伺服器名冊裡（否則整條連線被 4400 關掉、重連重送又被關）：不在就先送一次新名冊
fn ensure_in_roster(room: &Room, card: &str) {
    if room.work().roster.iter().any(|id| id == card) {
        return;
    }
    let (_, root) = room.engine.roots();
    match seed::roster(&root, &room.world()) {
        Ok(cards) => {
            room.work().roster = seed::card_ids(&cards);
            room.send("roster.update", json!({ "cards": cards }));
        }
        Err(error) => room.note_error(&format!("roster.update：{error}")),
    }
}

/// 一段（`part`）的上送：第一則增量才送 `begin`（中止時一個字都沒有就不開段），增量合併成每 ≥80 ms 或
/// ≥200 字一則，`end` 帶落檔正文。`begin`／`end` 走重送佇列，`end` 與本機鏈進度同一次落地。
struct Part {
    room: Arc<Room>,
    dispatch: String,
    n: u32,
    speaker: Value,
    state: Mutex<(bool, String, Instant)>,
}

impl Part {
    fn new(room: &Arc<Room>, dispatch: &str, n: u32, speaker: Value) -> Arc<Part> {
        Arc::new(Part {
            room: room.clone(),
            dispatch: dispatch.to_owned(),
            n,
            speaker,
            state: Mutex::new((false, String::new(), Instant::now())),
        })
    }

    fn begin(&self, begun: &mut bool) {
        if !*begun {
            *begun = true;
            let p = json!({ "dispatchId": self.dispatch, "part": self.n, "speaker": self.speaker });
            self.room.durable("gen.part.begin", p, |_| {});
        }
    }

    fn delta(self: &Arc<Self>) -> Delta {
        let part = self.clone();
        Arc::new(move |delta: &str| {
            let mut state = part
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            let (begun, buffer, last) = &mut *state;
            part.begin(begun);
            buffer.push_str(delta);
            if buffer.chars().count() >= 200 || last.elapsed() >= Duration::from_millis(80) {
                let p = json!({ "dispatchId": part.dispatch, "part": part.n, "text": std::mem::take(buffer) });
                part.room.send("gen.delta", p);
                *last = Instant::now();
            }
        })
    }

    fn end(
        &self,
        text: &str,
        interrupted: bool,
        status: Option<&Value>,
        change: impl FnOnce(&mut DispatchRun),
    ) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if !state.0 && text.trim().is_empty() {
            drop(state);
            return change_run(&self.room, change);
        }
        self.begin(&mut state.0);
        let n = self.n;
        let mut p = json!({ "dispatchId": self.dispatch, "part": n, "text": text, "interrupted": interrupted });
        if let Some(status) = status {
            p["state"] = status.clone();
        }
        self.room.durable("gen.part.end", p, |saved| {
            if let Some(run) = saved.dispatch.as_mut() {
                run.last_ended = Some(n);
                change(run);
            }
        });
    }
}

// ── 桌上落檔（前端原本做的那幾步）──

fn event(kind: TranscriptKind, speaker: (&str, &str), body: &str, action: &str) -> TranscriptEvent {
    TranscriptEvent {
        ts: data::local_timestamp().unwrap_or_default(),
        speaker_id: speaker.0.to_owned(),
        speaker_name: speaker.1.to_owned(),
        kind,
        text: body.to_owned(),
        raw: None,
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: Some(action.to_owned()),
    }
}

fn land_batch(
    config_root: &Path,
    root: &Path,
    world: &str,
    d: &Value,
) -> Result<BatchOutcome, String> {
    let list = |key: &str| d[key].as_array().cloned().unwrap_or_default();
    let batch = PartyBatch {
        dispatch_id: text(&d["dispatchId"]),
        actions: list("actions")
            .iter()
            .map(|a| match a["kind"] == "act" {
                true => PartyAction::Act {
                    card_id: text(&a["cardId"]),
                    text: text(&a["text"]),
                },
                false => PartyAction::Watch {
                    card_id: text(&a["cardId"]),
                },
            })
            .collect(),
        players: list("players").iter().map(|p| text(&p["cardId"])).collect(),
        arrivals: list("arrivals").iter().map(text).collect(),
        returns: list("returns").iter().map(text).collect(),
        departures: list("departures")
            .iter()
            .map(|x| {
                let mode = match x["mode"].as_str() {
                    Some("npc") => DepartureMode::Npc,
                    Some("delegate") => DepartureMode::Delegate,
                    _ => DepartureMode::Absent,
                };
                (text(&x["cardId"]), mode)
            })
            .collect(),
    };
    let _permit = data::world_write_permit(world)?;
    chat_turn::append_party_batch(config_root, root, world, &batch)
}

/// 旁白正文（回合鍵 main）→ 變動紀錄（state_update）→ 點名列（`call_row`）；回下一步。點到真人玩家只寫點名列就停
fn land_narration(
    root: &Path,
    world: &str,
    action: &str,
    turn: &str,
    n: &super::Narration,
    call_row: bool,
) -> Result<Step, String> {
    let scene = scene_of(root, world)?;
    let key = |part: &str| data::message_vars::TurnKey {
        turn_id: turn.to_owned(),
        part: part.to_owned(),
    };
    let append = |event: &TranscriptEvent, part: Option<&str>| {
        let _permit = data::world_write_permit(world)?;
        data::append_event(root, world, scene, event, part.map(key).as_ref())
            .map(|_| ())
            .map_err(|error| error.to_string())
    };
    if !n.text.trim().is_empty() {
        let mut main = event(TranscriptKind::Narration, ("", "GM"), &n.text, action);
        main.raw = n.raw.clone();
        main.truncated = n.aborted;
        append(&main, Some(data::message_vars::PART_MAIN))?;
    }
    if n.aborted {
        return Ok(Step::Done);
    }
    if !n.state_updates.is_empty() {
        let mut update = event(TranscriptKind::System, ("", "GM"), &n.state_updates, action);
        update.marker = Some(EventMarker::StateUpdate);
        append(&update, Some("state_update"))?;
    }
    let Some(next) = n.next.as_deref() else {
        return Ok(Step::Done);
    };
    let player = next == crate::transport::PLAYER_SENTINEL;
    let card = match player {
        true => n.next_player.as_deref(),
        false => Some(next),
    };
    let card = card.and_then(|id| data::read_character(root, world, id).ok());
    // 點名後被切成舞台卡：不寫點名列、不接話（同前端）
    if !player && card.as_ref().is_none_or(|card| card.stage) {
        return Ok(Step::Done);
    }
    if call_row {
        append_call(
            root,
            world,
            action,
            card.map(|card| card.name).unwrap_or_default(),
        )?;
    }
    Ok(if player {
        Step::Done
    } else {
        Step::Reply(next.to_owned())
    })
}

/// 「GM 請 X 發言」點名列（角色模型靠它知道自己被點）
fn append_call(root: &Path, world: &str, action: &str, name: String) -> Result<(), String> {
    let mut call = event(TranscriptKind::System, ("", "GM"), "", action);
    call.marker = Some(EventMarker::GmCall { name });
    let _permit = data::world_write_permit(world)?;
    let scene = scene_of(root, world)?;
    data::append_event(root, world, scene, &call, None)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn land_reply(
    root: &Path,
    world: &str,
    action: &str,
    turn: &str,
    card: &str,
    name: &str,
    spoken: &super::Spoken,
) -> Result<(), String> {
    if spoken.text.trim().is_empty() {
        return Ok(());
    }
    let mut line = event(TranscriptKind::Dialogue, (card, name), &spoken.text, action);
    line.raw = spoken.raw.clone();
    line.truncated = spoken.aborted;
    let _permit = data::world_write_permit(world)?;
    let scene = scene_of(root, world)?;
    data::append_character_reply(root, world, scene, &line, turn)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

// ── 中止與遠端控制 ──

/// `abort`／`ctl abort`／退出房間模式：打進行中那一次呼叫，鏈在這段收尾後停
pub(super) fn abort(room: &Room) {
    let turn = {
        let mut work = room.work();
        work.abort = true;
        work.turn.clone()
    };
    if let Some(turn) = turn {
        room.engine.abort(&room.world(), &turn);
    }
}

/// 遠端控制（協定 §7.3）：一律先回 `ctl.result` 再動手（伺服器 30 秒內沒收到就退回 held）
pub(super) fn on_ctl(room: &Arc<Room>, p: &Value) {
    let ctl_id = text(&p["ctlId"]);
    let cmd = &p["cmd"];
    let reply = |ok: bool| {
        let mut result = json!({ "ctlId": ctl_id, "ok": ok });
        if !ok {
            result["error"] = classify("E_STATE_CONFLICT: nothing to resume");
        }
        room.send("ctl.result", result);
    };
    match cmd["kind"].as_str().unwrap_or_default() {
        kind @ ("retry" | "switch_model" | "switch_route") => {
            let resumable = room.work().dispatch.is_none()
                && room
                    .saved()
                    .dispatch
                    .is_some_and(|run| run.state != RunState::Done);
            if !resumable {
                return reply(false);
            }
            room.update(|saved| {
                if kind != "retry" {
                    let current = saved
                        .settings
                        .clone()
                        .map(|(route, _)| route)
                        .unwrap_or_default();
                    let route = cmd["route"].as_str().map_or(current, str::to_owned);
                    saved.settings = Some((route, text(&cmd["model"])));
                }
                if let Some(run) = saved.dispatch.as_mut() {
                    run.state = RunState::Running;
                }
            });
            reply(true);
            spawn_chain(room);
        }
        "abort" => {
            reply(true);
            abort(room);
        }
        // 下一幕設定由伺服器存著、隨換幕後的派工帶來；額度不報，這兩樣只照實回
        "set_next" | "refresh_quota" => reply(true),
        // 手機房主遠端暫停（room-seat-delegate §3.3）：先回結果，再走跟桌面「關房（暫停）」同一條流程。
        // exit 要停掉這條連線迴圈，不能在迴圈裡等它，另開一個 task
        "pause" => {
            reply(true);
            let world = room.world();
            super::note_stop(&world, super::PAUSED_REMOTELY);
            tokio::spawn(async move { super::exit_world(&world).await });
        }
        _ => reply(false),
    }
}

// ── 工作：換幕摘要、自建卡 ──

pub(super) fn on_job(room: &Arc<Room>, job: &Value) {
    let id = text(&job["jobId"]);
    if id.is_empty() {
        return;
    }
    room.send("job.ack", json!({ "jobId": id }));
    if matches!(
        job["kind"].as_str(),
        Some("card_create" | "card_import" | "card_image")
    ) {
        return super::cards::on_job(room, id, job.clone());
    }
    if job["kind"] == "undo" {
        return undo(room, &id, job);
    }
    if room.work().job.as_deref() == Some(id.as_str()) {
        return;
    }
    let (_, root) = room.engine.roots();
    let world = room.world();
    match room.saved().job {
        Some(run) if run.id == id && run.state != RunState::Running => return,
        // 程序重啟後同一筆還在 running：桌上幕號已推進＝摘要已提交，直接回結果；否則重做
        Some(run)
            if run.id == id
                && scene_of(&root, &world).is_ok_and(|scene| scene > run.scene_before) =>
        {
            return settle_job(room, &id, None);
        }
        Some(run) if run.id == id => {}
        _ => {
            let Ok(scene_before) = scene_of(&root, &world) else {
                return settle_job(room, &id, Some("SCENE_READ_FAILED".to_owned()));
            };
            room.update(|saved| {
                saved.job = Some(JobRun {
                    id: id.clone(),
                    state: RunState::Running,
                    scene_before,
                })
            });
        }
    }
    let room = room.clone();
    tokio::spawn(async move { scene_job(&room, id).await });
}

async fn scene_job(room: &Arc<Room>, id: String) {
    let turn = data::new_id();
    {
        let mut work = room.work();
        work.job = Some(id.clone());
        work.job_turn = Some(turn.clone());
    }
    report(room);
    // 摘要可能跑很久：至少每 5 分鐘一則進度（協定 §3.5，不套 30 秒逾時）
    let ticker = {
        let room = room.clone();
        let id = id.clone();
        tokio::spawn(async move {
            let every = room.timing.unit * 240;
            loop {
                tokio::time::sleep(every).await;
                room.send("job.progress", json!({ "jobId": id }));
            }
        })
    };
    let result = room.engine.advance(room.world(), turn).await;
    ticker.abort();
    {
        let mut work = room.work();
        work.job = None;
        work.job_turn = None;
    }
    settle_job(room, &id, result.err());
    report(room);
    room.engine.table_changed(&room.world());
}

/// 照實回換幕結果：桌上幕號已推進就是 ok（帶已落的摘要），否則失敗。中止、程序重啟、
/// `job.cancel` 時本機沒在跑，都走這裡，**一定回**（協定 §3.5）
fn settle_job(room: &Room, id: &str, error: Option<String>) {
    let (_, root) = room.engine.roots();
    let world = room.world();
    let run = room.saved().job.filter(|run| run.id == id);
    let scene = scene_of(&root, &world).ok();
    let advanced = matches!((&run, scene), (Some(run), Some(scene)) if scene > run.scene_before);
    let p = match (advanced, scene) {
        (true, Some(scene)) => {
            json!({ "jobId": id, "ok": true, "result": { "summary": summary_of(&root, &world, scene) } })
        }
        _ => {
            let message = error.unwrap_or_else(|| "SCENE_NOT_ADVANCED".to_owned());
            json!({ "jobId": id, "ok": false, "error": classify(&message) })
        }
    };
    room.durable("job.result", p, |saved| {
        if let Some(run) = saved.job.as_mut().filter(|run| run.id == id) {
            run.state = RunState::Done;
        }
        if let (true, Some(scene)) = (advanced, scene) {
            saved.synced_scene = Some(scene); // 房間跟著換到這一幕
        }
    });
}

fn summary_of(root: &Path, world: &str, scene: u64) -> String {
    data::read_transcript(root, world, scene)
        .unwrap_or_default()
        .into_iter()
        .find(|event| event.marker == Some(EventMarker::SceneSummary))
        .map(|event| event.text.chars().take(64000).collect())
        .unwrap_or_default()
}

/// `job.cancel`：本機正在跑就中止那次摘要（跑完的那支回結果）；沒在跑就照桌上幕號回
pub(super) fn on_cancel(room: &Arc<Room>, p: &Value) {
    let id = text(&p["jobId"]);
    let turn = {
        let work = room.work();
        work.job
            .as_deref()
            .filter(|job| *job == id)
            .and(work.job_turn.clone())
    };
    match turn {
        Some(turn) => room.engine.abort(&room.world(), &turn),
        None => settle_job(room, &id, Some("SCENE_ADVANCE_CANCELLED".to_owned())),
    }
}

/// `job.undo`（room-host-actions §3.2）：把那個派工落下的事件從桌尾整批收掉（狀態樹一起回捲），照實回結果。
/// 已收過也回 ok（冪等）；不在桌尾、或本機還在跑它，回失敗、什麼都不改
fn undo(room: &Arc<Room>, id: &str, job: &Value) {
    let dispatch = text(&job["dispatchId"]);
    let (_, root) = room.engine.roots();
    let world = room.world();
    let popped = match room.work().dispatch.as_deref() == Some(dispatch.as_str()) {
        true => Ok(false),
        false => data::world_write_permit(&world)
            .map_err(String::from)
            .and_then(|_permit| {
                let scene = scene_of(&root, &world)?;
                data::pop_action_floors(&root, &world, scene, &dispatch).map_err(|e| e.to_string())
            }),
    };
    let p = match popped {
        Ok(true) => json!({ "jobId": id, "ok": true }),
        Ok(false) => json!({ "jobId": id, "ok": false, "error": classify("UNDO_NOT_TAIL") }),
        Err(error) => json!({ "jobId": id, "ok": false, "error": classify(&error) }),
    };
    room.durable("job.result", p, |saved| {
        if saved
            .dispatch
            .as_ref()
            .is_some_and(|run| run.id == dispatch)
        {
            saved.dispatch = None; // 收掉的派工不再能 retry
        }
    });
    room.engine.table_changed(&world);
}
