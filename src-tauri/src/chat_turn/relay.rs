//! GM 接力驅動：旁白 → 點到 NPC 接話 → 點到真人、沒點名、舞台卡、截斷或達上限就停。
//! 落檔（旁白正文帶回合鍵、變動紀錄、點名列、角色回覆、截斷旗標）都在這裡。
//! 房間與單人各做一個 host：模型呼叫、段號廣播、步驟持久化、停止訊號留在 host。
use crate::data::{self, EventMarker, TranscriptEvent, TranscriptKind};
use serde_json::Value;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::Duration;

pub(crate) type HostFut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// 整條接力共用，不會每段重設。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Halt {
    None,
    /// 玩家按停止。在途的模型呼叫由 host 自己中止；驅動不再發新呼叫。
    Interrupt,
    /// 換桌換幕。在途那段照常落完（含狀態提交），之後不再發新呼叫。房間不送。
    StopRelay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RelayMode {
    /// 一次旁白，回報點名，不寫點名列、不接話。
    Narrate,
    /// 旁白加被點到的角色一句就停。寫點名列。
    NarrateThenOne,
    /// 推進到上限。寫點名列。
    Advance,
    /// 直接點名。點名列照 `list`。
    Call { card_id: String, list: CallList },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CallList {
    /// 請 X 發言：不寫。
    Omit,
    /// 讓 X 接話：寫，但那則旁白之後已有同名點名列就不疊。
    UnlessSince { narration_id: String },
    /// 照這個布林寫點名列。測試用；房間的 call 在 Land 已寫過，驅動從 Reply 進來時用 Omit。
    #[cfg_attr(not(test), allow(dead_code))]
    AsHost(bool),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RelayStep {
    Narrate,
    /// 直接點名的起點（先點名列，再接話）。不寫進 room.json；單人用。
    Call(String),
    Reply(String),
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RelayProgress {
    pub(crate) used: u32,
    pub(crate) step: RelayStep,
    pub(crate) last_part: Option<u32>,
    pub(crate) arrived: Vec<String>,
    pub(crate) called: Option<String>,
}

/// 一次 GM 旁白。`truncated` 是輸出上限；`aborted` 是玩家按停止。
pub(crate) struct Piece {
    pub(crate) text: String,
    pub(crate) raw: Option<String>,
    pub(crate) next: Option<String>,
    pub(crate) next_player: Option<String>,
    pub(crate) state_updates: String,
    pub(crate) arrived: Vec<String>,
    pub(crate) aborted: bool,
    pub(crate) truncated: bool,
    pub(crate) state_error: Option<String>,
    pub(crate) status: Option<Value>,
    pub(crate) panel: Option<Value>,
}

pub(crate) struct Line {
    pub(crate) text: String,
    pub(crate) raw: Option<String>,
    pub(crate) aborted: bool,
    pub(crate) truncated: bool,
}

/// host 決定點名怎麼接。玩家名字也由 host 給（單人 `playerName`，房間 `next_player` 卡名）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Nominee {
    /// 舞台卡，或房間讀不到這張卡。
    Stop,
    Player {
        name: String,
    },
    Npc {
        id: String,
        name: String,
    },
}

/// 每則落檔當下的回報（正文、變動紀錄、點名列、角色回覆，依落檔順序）。`next` 只在旁白正文有：
/// 給「讓 X 接話」，停止或截斷是 None，其餘照模型點名（含玩家哨兵）。房間 host 不讀；單人讀。
pub(crate) struct Landed {
    pub(crate) event: TranscriptEvent,
    pub(crate) next: Option<String>,
}

/// 這段要收進廣播的內容。沒有＝這步不開段（碰到上限、只寫點名列）。
pub(crate) struct PartClose {
    pub(crate) text: String,
    pub(crate) interrupted: bool,
    pub(crate) status: Option<Value>,
    pub(crate) panel: Option<Value>,
}

pub(crate) struct RelayRequest {
    pub(crate) mode: RelayMode,
    pub(crate) max_speakers: u32,
    pub(crate) action_id: String,
    pub(crate) progress: RelayProgress,
    /// 正文落檔失敗的重試間隔。零＝立刻重試（測試）。
    pub(crate) retry_delay: Duration,
}

#[derive(Debug)]
pub(crate) struct RelayDone {
    pub(crate) aborted: bool,
    /// 收尾進度。房間已在 checkpoint 寫進 room.json，單人不需要；測試讀。
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) progress: RelayProgress,
    /// 單人用來判斷玩家句算不算已回。房間不讀。
    pub(crate) receipts: Vec<TranscriptEvent>,
    /// 單人用來提示狀態沒寫成。房間不讀。
    pub(crate) state_errors: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct RelayFail {
    pub(crate) part: Option<u32>,
    pub(crate) message: String,
    /// 失敗前已經落檔的收據。單人用來判斷能不能收回玩家句。
    pub(crate) receipts: Vec<TranscriptEvent>,
}

pub(crate) trait RelayHost {
    fn halt(&self) -> Halt;
    fn begin_turn(&mut self) -> String;
    fn narrate(&mut self, turn_id: String, part: u32) -> HostFut<'_, Result<Piece, String>>;
    fn reply(
        &mut self,
        card_id: String,
        turn_id: String,
        part: u32,
    ) -> HostFut<'_, Result<Line, String>>;
    fn classify(&self, next: &str, next_player: Option<&str>) -> Nominee;
    fn landed(&mut self, landed: &Landed);
    fn note_state_error(&mut self, message: &str);
    /// 持久化進度。`close` 有值才收這一段（房間送 `gen.part.end`）。回傳這段有沒有佔掉段號。
    fn checkpoint(&mut self, progress: &RelayProgress, close: Option<PartClose>) -> bool;
    fn roots(&self) -> (&Path, &str);
}

/// 這條接力的落檔目標。開場決定，之後不跟當下幕號走。
struct Desk {
    root: PathBuf,
    world: String,
    scene: u64,
}

struct RelayRun {
    desk: Desk,
    progress: RelayProgress,
    receipts: Vec<TranscriptEvent>,
    state_errors: Vec<String>,
    aborted: bool,
}

pub(crate) async fn drive<H: RelayHost>(
    host: &mut H,
    req: RelayRequest,
) -> Result<RelayDone, RelayFail> {
    let desk = {
        let (root, world) = host.roots();
        match scene_of(root, world) {
            Ok(scene) => Desk {
                root: root.to_path_buf(),
                world: world.to_owned(),
                scene,
            },
            Err(message) => {
                return Err(RelayFail {
                    part: None,
                    message,
                    receipts: Vec::new(),
                });
            }
        }
    };
    let mut run = RelayRun {
        desk,
        progress: req.progress.clone(),
        receipts: Vec::new(),
        state_errors: Vec::new(),
        aborted: false,
    };
    let max = req.max_speakers.max(1);
    loop {
        if matches!(run.progress.step, RelayStep::Done) {
            break;
        }
        match host.halt() {
            Halt::Interrupt => {
                run.aborted = true;
                break;
            }
            Halt::StopRelay => break,
            Halt::None => {}
        }
        let part = run.progress.last_part.map_or(0, |n| n + 1);
        match run.progress.step.clone() {
            RelayStep::Done => break,
            RelayStep::Narrate if run.progress.used >= max => {
                run.progress.step = RelayStep::Done;
                host.checkpoint(&run.progress, None);
            }
            RelayStep::Narrate => {
                if let Err(message) = narrate_segment(host, &req, &mut run, part).await {
                    return Err(RelayFail {
                        part: Some(part),
                        message,
                        receipts: run.receipts,
                    });
                }
                if run.aborted {
                    break;
                }
            }
            RelayStep::Call(card) => {
                if let Err(message) = call_segment(host, &req, &mut run, &card).await {
                    return Err(RelayFail {
                        part: Some(part),
                        message,
                        receipts: run.receipts,
                    });
                }
                if run.aborted {
                    break;
                }
            }
            RelayStep::Reply(card) => {
                if let Err(message) = reply_segment(host, &req, &mut run, part, &card).await {
                    return Err(RelayFail {
                        part: Some(part),
                        message,
                        receipts: run.receipts,
                    });
                }
                if run.aborted {
                    break;
                }
            }
        }
    }
    Ok(RelayDone {
        aborted: run.aborted,
        progress: run.progress,
        receipts: run.receipts,
        state_errors: run.state_errors,
    })
}

async fn narrate_segment<H: RelayHost>(
    host: &mut H,
    req: &RelayRequest,
    run: &mut RelayRun,
    part: u32,
) -> Result<(), String> {
    let turn = host.begin_turn();
    let piece = host.narrate(turn.clone(), part).await?;
    let main = land_main(&run.desk, &req.action_id, &turn, &piece, req.retry_delay).await?;
    if let Some(event) = main {
        let next = (!piece.aborted && !piece.truncated)
            .then(|| piece.next.clone())
            .flatten();
        keep(host, run, event, next);
    }
    if !piece.aborted {
        if let Some(message) = piece.state_error.clone() {
            run.state_errors.push(message.clone());
            host.note_state_error(&message);
        }
        if !piece.state_updates.is_empty() {
            let event = land_state(&run.desk, &req.action_id, &turn, &piece.state_updates)?;
            keep(host, run, event, None);
        }
    }
    run.progress.used += 1;
    run.progress.arrived.extend(piece.arrived.iter().cloned());
    let continue_relay = !piece.aborted && !piece.truncated && host.halt() == Halt::None;
    if continue_relay {
        nominate(host, req, run, &piece)?;
    } else {
        run.progress.step = RelayStep::Done;
        if piece.aborted || host.halt() == Halt::Interrupt {
            run.aborted = true;
        }
    }
    let keep_status = !piece.aborted;
    let text = piece.text.clone();
    let status = piece.status.filter(|_| keep_status);
    let panel = piece.panel.filter(|_| keep_status);
    close_part(
        host,
        &mut run.progress,
        part,
        &text,
        piece.aborted,
        status,
        panel,
    );
    Ok(())
}

/// 點名列與下一步。呼叫前已確定這段沒被截斷、也沒有停止或換桌。
fn nominate<H: RelayHost>(
    host: &mut H,
    req: &RelayRequest,
    run: &mut RelayRun,
    piece: &Piece,
) -> Result<(), String> {
    let Some(next) = piece.next.as_deref() else {
        run.progress.step = RelayStep::Done;
        return Ok(());
    };
    let nominee = host.classify(next, piece.next_player.as_deref());
    let write_row = matches!(req.mode, RelayMode::NarrateThenOne | RelayMode::Advance);
    match nominee {
        Nominee::Stop => run.progress.step = RelayStep::Done,
        Nominee::Player { name } => {
            if write_row {
                let event = land_call(&run.desk, &req.action_id, name)?;
                keep(host, run, event, None);
            }
            run.progress.step = RelayStep::Done;
        }
        Nominee::Npc { id, name } => {
            if write_row {
                let event = land_call(&run.desk, &req.action_id, name)?;
                keep(host, run, event, None);
            }
            run.progress.step = match &req.mode {
                RelayMode::Narrate => {
                    if run.progress.called.is_none() {
                        run.progress.called = Some(id);
                    }
                    RelayStep::Done
                }
                _ => RelayStep::Reply(id),
            };
        }
    }
    Ok(())
}

async fn call_segment<H: RelayHost>(
    host: &mut H,
    req: &RelayRequest,
    run: &mut RelayRun,
    card: &str,
) -> Result<(), String> {
    let RelayMode::Call { list, .. } = &req.mode else {
        run.progress.step = RelayStep::Reply(card.to_owned());
        host.checkpoint(&run.progress, None);
        return Ok(());
    };
    let list = list.clone();
    let nominee = host.classify(card, None);
    let Some(name) = nominee_name(&nominee) else {
        run.progress.step = RelayStep::Done;
        host.checkpoint(&run.progress, None);
        return Ok(());
    };
    if should_write_call(&run.desk, &list, &name)? {
        let event = land_call(&run.desk, &req.action_id, name)?;
        keep(host, run, event, None);
    }
    match host.halt() {
        Halt::Interrupt => {
            run.aborted = true;
            run.progress.step = RelayStep::Done;
            host.checkpoint(&run.progress, None);
            return Ok(());
        }
        Halt::StopRelay => {
            run.progress.step = RelayStep::Done;
            host.checkpoint(&run.progress, None);
            return Ok(());
        }
        Halt::None => {}
    }
    run.progress.step = match nominee {
        Nominee::Npc { id, .. } => RelayStep::Reply(id),
        _ => RelayStep::Done,
    };
    host.checkpoint(&run.progress, None);
    Ok(())
}

fn nominee_name(nominee: &Nominee) -> Option<String> {
    match nominee {
        Nominee::Stop => None,
        Nominee::Player { name } | Nominee::Npc { name, .. } => Some(name.clone()),
    }
}

fn should_write_call(desk: &Desk, list: &CallList, name: &str) -> Result<bool, String> {
    Ok(match list {
        CallList::Omit => false,
        CallList::AsHost(write) => *write,
        CallList::UnlessSince { narration_id } => !called_since(desk, narration_id, name)?,
    })
}

async fn reply_segment<H: RelayHost>(
    host: &mut H,
    req: &RelayRequest,
    run: &mut RelayRun,
    part: u32,
    card: &str,
) -> Result<(), String> {
    let turn = host.begin_turn();
    let card_id = card.to_owned();
    // 名字在呼叫前定案。回來之後再分類：變成停止只決定不再接力，不改這段的身分。
    let name = nominee_name(&host.classify(&card_id, None)).unwrap_or_default();
    let line = host.reply(card_id.clone(), turn.clone(), part).await?;
    let stopped = matches!(host.classify(&card_id, None), Nominee::Stop);
    let landed = land_line(&run.desk, &req.action_id, &turn, &card_id, &name, &line)?;
    if let Some(event) = landed {
        keep(host, run, event, None);
    }
    let one_shot = matches!(req.mode, RelayMode::NarrateThenOne | RelayMode::Call { .. });
    run.progress.step =
        if line.aborted || one_shot || stopped || run.progress.used >= req.max_speakers.max(1) {
            RelayStep::Done
        } else {
            RelayStep::Narrate
        };
    if line.aborted {
        run.aborted = true;
    }
    close_part(
        host,
        &mut run.progress,
        part,
        &line.text,
        line.aborted,
        None,
        None,
    );
    Ok(())
}

/// 收據與即時回報同一處：每則落檔都經這裡。
fn keep<H: RelayHost>(
    host: &mut H,
    run: &mut RelayRun,
    event: TranscriptEvent,
    next: Option<String>,
) {
    run.receipts.push(event.clone());
    host.landed(&Landed { event, next });
}

fn close_part<H: RelayHost>(
    host: &mut H,
    progress: &mut RelayProgress,
    part: u32,
    text: &str,
    interrupted: bool,
    status: Option<Value>,
    panel: Option<Value>,
) {
    let consumed = host.checkpoint(
        progress,
        Some(PartClose {
            text: text.to_owned(),
            interrupted,
            status,
            panel,
        }),
    );
    if consumed {
        progress.last_part = Some(part);
    }
}

fn scene_of(root: &Path, world: &str) -> Result<u64, String> {
    data::read_state(root, world)
        .map(|state| state.current_scene)
        .map_err(|error| error.to_string())
}

fn story_event(
    kind: TranscriptKind,
    speaker: (&str, &str),
    body: &str,
    action: &str,
) -> TranscriptEvent {
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

fn append(
    desk: &Desk,
    event: &TranscriptEvent,
    turn: Option<&data::message_vars::TurnKey>,
) -> Result<TranscriptEvent, String> {
    let _permit = data::world_write_permit(&desk.world).map_err(|error| error.to_string())?;
    data::append_event(&desk.root, &desk.world, desk.scene, event, turn)
        .map(|(event, _)| event)
        .map_err(|error| error.to_string())
}

async fn land_main(
    desk: &Desk,
    action: &str,
    turn: &str,
    piece: &Piece,
    delay: Duration,
) -> Result<Option<TranscriptEvent>, String> {
    if piece.text.trim().is_empty() {
        return Ok(None);
    }
    let mut event = story_event(TranscriptKind::Narration, ("", "GM"), &piece.text, action);
    event.raw = piece.raw.clone();
    event.truncated = piece.aborted || piece.truncated;
    let key = data::message_vars::TurnKey {
        turn_id: turn.to_owned(),
        part: data::message_vars::PART_MAIN.to_owned(),
    };
    let mut attempt = 0u32;
    loop {
        match append(desk, &event, Some(&key)) {
            Ok(landed) => return Ok(Some(landed)),
            Err(error) if attempt >= 2 || error.starts_with("AI_EMPTY_RESPONSE") => {
                return Err(error);
            }
            Err(_) => {
                attempt += 1;
                if !delay.is_zero() {
                    tokio::time::sleep(delay * attempt).await;
                }
            }
        }
    }
}

fn land_state(
    desk: &Desk,
    action: &str,
    turn: &str,
    text: &str,
) -> Result<TranscriptEvent, String> {
    let mut event = story_event(TranscriptKind::System, ("", "GM"), text, action);
    event.marker = Some(EventMarker::StateUpdate);
    let key = data::message_vars::TurnKey {
        turn_id: turn.to_owned(),
        part: "state_update".to_owned(),
    };
    append(desk, &event, Some(&key))
}

fn land_call(desk: &Desk, action: &str, name: String) -> Result<TranscriptEvent, String> {
    let mut event = story_event(TranscriptKind::System, ("", "GM"), "", action);
    event.marker = Some(EventMarker::GmCall { name });
    append(desk, &event, None)
}

fn land_line(
    desk: &Desk,
    action: &str,
    turn: &str,
    card: &str,
    name: &str,
    line: &Line,
) -> Result<Option<TranscriptEvent>, String> {
    if line.text.trim().is_empty() {
        return Ok(None);
    }
    let mut event = story_event(TranscriptKind::Dialogue, (card, name), &line.text, action);
    event.raw = line.raw.clone();
    event.truncated = line.aborted || line.truncated;
    let _permit = data::world_write_permit(&desk.world).map_err(|error| error.to_string())?;
    data::append_character_reply(&desk.root, &desk.world, desk.scene, &event, turn)
        .map_err(|error| error.to_string())
        .map(Some)
}

fn called_since(desk: &Desk, narration_id: &str, name: &str) -> Result<bool, String> {
    let events = data::read_transcript(&desk.root, &desk.world, desk.scene)
        .map_err(|error| error.to_string())?;
    let Some(index) = events
        .iter()
        .position(|event| event.id.as_deref() == Some(narration_id))
    else {
        return Ok(false);
    };
    Ok(events[index + 1..].iter().any(|event| {
        matches!(&event.marker, Some(EventMarker::GmCall { name: called }) if called == name)
    }))
}

#[cfg(test)]
#[path = "relay_tests.rs"]
mod tests;
