//! 假 host 覆蓋四種模式、三種點名列、截斷不接力、兩種停止訊號、落檔收據。
use super::*;
use crate::data::{EventMarker, TranscriptKind};
use crate::transport::PLAYER_SENTINEL;
use std::cell::Cell;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn piece(text: &str, next: Option<&str>) -> Piece {
    Piece {
        text: text.to_owned(),
        raw: None,
        next: next.map(str::to_owned),
        next_player: None,
        state_updates: String::new(),
        arrived: Vec::new(),
        aborted: false,
        truncated: false,
        state_error: None,
        status: None,
        panel: None,
    }
}

fn line(text: &str) -> Line {
    Line {
        text: text.to_owned(),
        raw: None,
        aborted: false,
        truncated: false,
    }
}

struct Fake {
    root: PathBuf,
    world: String,
    narrations: VecDeque<Piece>,
    lines: VecDeque<Line>,
    halt: Halt,
    /// narrate 回傳前改成這個訊號（模擬落檔前換桌或停止）。
    halt_on_narrate: Option<Halt>,
    /// 第 n 次 checkpoint 之後改 halt（1＝第一次收段之後）。
    halt_after_checkpoint: Option<(usize, Halt)>,
    /// 旁白回合已開、尚未落檔時真的換到下一幕。
    switch_scene_on_narrate: bool,
    /// 開場快照之後、第一段之前換幕（`halt` 是第一個還看得到 host 的點）。
    switch_scene_on_halt: Cell<bool>,
    /// 回覆呼叫進行中把這張卡改成舞台卡。
    stage_during_reply: bool,
    checkpoints: usize,
    open_turns: bool,
    names: HashMap<String, Nominee>,
    narrate_calls: usize,
    reply_calls: usize,
    state_notes: Vec<String>,
    landed_next: Vec<Option<String>>,
}

impl Fake {
    fn new(label: &str) -> Self {
        let root = crate::smart_free::test_root(&format!("relay-{label}"));
        let world = data::create_world(&root, "接力").unwrap();
        Self {
            root,
            world,
            narrations: VecDeque::new(),
            lines: VecDeque::new(),
            halt: Halt::None,
            halt_on_narrate: None,
            halt_after_checkpoint: None,
            switch_scene_on_narrate: false,
            switch_scene_on_halt: Cell::new(false),
            stage_during_reply: false,
            checkpoints: 0,
            open_turns: true,
            names: HashMap::new(),
            narrate_calls: 0,
            reply_calls: 0,
            state_notes: Vec::new(),
            landed_next: Vec::new(),
        }
    }

    fn npc(&mut self, id: &str, name: &str) {
        self.names.insert(
            id.to_owned(),
            Nominee::Npc {
                id: id.to_owned(),
                name: name.to_owned(),
            },
        );
    }

    fn open_turn(&self, turn_id: &str) -> Result<(), String> {
        if !self.open_turns {
            return Ok(());
        }
        data::state_commit::with_commit(&self.root, &self.world, |tx| {
            data::message_vars::begin_turn(tx, turn_id, None)?;
            data::message_vars::finish_turn(tx, turn_id, None)
        })
        .map_err(|error| error.to_string())
    }
}

impl RelayHost for Fake {
    fn halt(&self) -> Halt {
        if self.switch_scene_on_halt.replace(false) {
            data::begin_next_scene(&self.root, &self.world, "換幕摘要", None).expect("換幕");
        }
        self.halt
    }

    fn begin_turn(&mut self) -> String {
        data::new_id()
    }

    fn narrate(&mut self, turn_id: String, _part: u32) -> HostFut<'_, Result<Piece, String>> {
        Box::pin(async move {
            self.open_turn(&turn_id)?;
            self.narrate_calls += 1;
            if self.switch_scene_on_narrate {
                data::begin_next_scene(&self.root, &self.world, "換幕摘要", None)
                    .map_err(|error| error.to_string())?;
            }
            if let Some(halt) = self.halt_on_narrate {
                self.halt = halt;
            }
            Ok(self.narrations.pop_front().expect("不該再叫旁白"))
        })
    }

    fn reply(
        &mut self,
        card_id: String,
        _turn_id: String,
        _part: u32,
    ) -> HostFut<'_, Result<Line, String>> {
        Box::pin(async move {
            self.reply_calls += 1;
            if self.stage_during_reply {
                self.names.insert(card_id, Nominee::Stop);
            }
            Ok(self.lines.pop_front().expect("不該再叫接話"))
        })
    }

    fn classify(&self, next: &str, _next_player: Option<&str>) -> Nominee {
        self.names.get(next).cloned().unwrap_or(Nominee::Stop)
    }

    fn landed(&mut self, landed: &Landed) {
        if landed.event.kind == TranscriptKind::Narration {
            let events = transcript(&self.root, &self.world);
            assert!(
                events
                    .iter()
                    .all(|event| event.marker != Some(EventMarker::StateUpdate)),
                "正文回報要早於變動紀錄"
            );
        }
        self.landed_next.push(landed.next.clone());
    }

    fn note_state_error(&mut self, message: &str) {
        self.state_notes.push(message.to_owned());
    }

    fn checkpoint(&mut self, _progress: &RelayProgress, close: Option<PartClose>) -> bool {
        self.checkpoints += 1;
        if let Some((n, halt)) = self.halt_after_checkpoint {
            if self.checkpoints == n {
                self.halt = halt;
            }
        }
        close.is_some_and(|close| !close.text.trim().is_empty())
    }

    fn roots(&self) -> (&Path, &str) {
        (&self.root, &self.world)
    }
}

fn transcript(root: &Path, world: &str) -> Vec<TranscriptEvent> {
    let scene = data::read_state(root, world).unwrap().current_scene;
    data::read_transcript(root, world, scene).unwrap()
}

fn request(mode: RelayMode, max: u32, step: RelayStep) -> RelayRequest {
    RelayRequest {
        mode,
        max_speakers: max,
        action_id: "act".to_owned(),
        retry_delay: Duration::ZERO,
        progress: RelayProgress {
            used: 0,
            step,
            last_part: None,
            arrived: Vec::new(),
            called: None,
        },
    }
}

async fn go(host: &mut Fake, mode: RelayMode, max: u32) -> RelayDone {
    let step = match &mode {
        RelayMode::Call { card_id, .. } => RelayStep::Call(card_id.clone()),
        _ => RelayStep::Narrate,
    };
    drive(host, request(mode, max, step))
        .await
        .expect("接力不該失敗")
}

fn markers(events: &[TranscriptEvent]) -> Vec<(TranscriptKind, Option<EventMarker>, String, bool)> {
    events
        .iter()
        .map(|event| {
            (
                event.kind.clone(),
                event.marker.clone(),
                event.text.clone(),
                event.truncated,
            )
        })
        .collect()
}

#[tokio::test]
async fn advance_stops_at_the_player_after_one_npc_reply() {
    let mut host = Fake::new("advance");
    host.npc("boss", "老闆");
    host.names.insert(
        PLAYER_SENTINEL.to_owned(),
        Nominee::Player {
            name: "阿濤".to_owned(),
        },
    );
    host.narrations = VecDeque::from([
        piece("店裡很安靜。", Some("boss")),
        piece("輪到你了。", Some(PLAYER_SENTINEL)),
    ]);
    host.lines = VecDeque::from([line("歡迎。")]);
    let done = go(&mut host, RelayMode::Advance, 3).await;
    assert!(!done.aborted);
    assert_eq!(host.narrate_calls, 2);
    assert_eq!(host.reply_calls, 1);
    assert_eq!(
        markers(&transcript(&host.root, &host.world)),
        vec![
            (
                TranscriptKind::Narration,
                None,
                "店裡很安靜。".to_owned(),
                false
            ),
            (
                TranscriptKind::System,
                Some(EventMarker::GmCall {
                    name: "老闆".to_owned()
                }),
                String::new(),
                false
            ),
            (TranscriptKind::Dialogue, None, "歡迎。".to_owned(), false),
            (
                TranscriptKind::Narration,
                None,
                "輪到你了。".to_owned(),
                false
            ),
            (
                TranscriptKind::System,
                Some(EventMarker::GmCall {
                    name: "阿濤".to_owned()
                }),
                String::new(),
                false
            ),
        ]
    );
    assert!(done
        .receipts
        .iter()
        .all(|event| event.action_id.as_deref() == Some("act")));
    assert_eq!(
        done.receipts
            .iter()
            .filter(|event| event.kind == TranscriptKind::Narration)
            .count(),
        2
    );
}

#[tokio::test]
async fn narrate_once_reports_the_npc_and_does_not_reply() {
    let mut host = Fake::new("once");
    host.npc("boss", "老闆");
    host.narrations = VecDeque::from([piece("老闆抬起頭。", Some("boss"))]);
    let done = go(&mut host, RelayMode::Narrate, 3).await;
    assert_eq!(done.progress.called.as_deref(), Some("boss"));
    assert_eq!(host.reply_calls, 0);
    assert_eq!(host.landed_next, vec![Some("boss".to_owned())]);
    assert_eq!(transcript(&host.root, &host.world).len(), 1);
}

#[tokio::test]
async fn narrate_then_one_stops_after_the_reply() {
    let mut host = Fake::new("one");
    host.npc("boss", "老闆");
    host.narrations = VecDeque::from([piece("點名。", Some("boss")), piece("不該再旁白。", None)]);
    host.lines = VecDeque::from([line("一句。"), line("第二句不該出現。")]);
    let done = go(&mut host, RelayMode::NarrateThenOne, 3).await;
    assert!(!done.aborted);
    assert_eq!(host.narrate_calls, 1);
    assert_eq!(host.reply_calls, 1);
    assert_eq!(transcript(&host.root, &host.world).len(), 3);
}

#[tokio::test]
async fn direct_call_follows_the_three_list_rules() {
    let mut host = Fake::new("call");
    host.npc("boss", "老闆");
    host.lines = VecDeque::from([line("甲"), line("乙"), line("丙")]);
    let omit = go(
        &mut host,
        RelayMode::Call {
            card_id: "boss".to_owned(),
            list: CallList::Omit,
        },
        3,
    )
    .await;
    assert!(omit.receipts.iter().all(|event| event.marker.is_none()));

    let written = go(
        &mut host,
        RelayMode::Call {
            card_id: "boss".to_owned(),
            list: CallList::AsHost(true),
        },
        3,
    )
    .await;
    assert!(written.receipts.iter().any(|event| {
        event.marker
            == Some(EventMarker::GmCall {
                name: "老闆".to_owned(),
            })
    }));

    let skipped = go(
        &mut host,
        RelayMode::Call {
            card_id: "boss".to_owned(),
            list: CallList::AsHost(false),
        },
        3,
    )
    .await;
    assert_eq!(skipped.receipts.len(), 1);
    assert_eq!(host.reply_calls, 3);
}

#[tokio::test]
async fn call_unless_since_does_not_stack_the_same_name() {
    let mut host = Fake::new("since");
    host.npc("boss", "老闆");
    host.narrations = VecDeque::from([piece("旁白。", Some("boss"))]);
    host.lines = VecDeque::from([line("先。"), line("後。")]);
    let first = go(&mut host, RelayMode::Advance, 1).await;
    let narration_id = first.receipts[0].id.clone().unwrap();
    let again = go(
        &mut host,
        RelayMode::Call {
            card_id: "boss".to_owned(),
            list: CallList::UnlessSince { narration_id },
        },
        3,
    )
    .await;
    let calls = transcript(&host.root, &host.world)
        .into_iter()
        .filter(|event| matches!(event.marker, Some(EventMarker::GmCall { .. })))
        .count();
    assert_eq!(calls, 1);
    assert_eq!(again.receipts.len(), 1);
}

#[tokio::test]
async fn truncated_narration_lands_state_and_does_not_relay() {
    let mut host = Fake::new("cut");
    host.npc("boss", "老闆");
    let mut cut = piece("說到一半", Some("boss"));
    cut.truncated = true;
    cut.state_updates = "hp：1".to_owned();
    cut.state_error = Some("狀態沒寫成".to_owned());
    host.narrations = VecDeque::from([cut]);
    host.lines = VecDeque::from([line("不該接。")]);
    let done = go(&mut host, RelayMode::Advance, 3).await;
    assert!(!done.aborted);
    assert_eq!(host.reply_calls, 0);
    assert_eq!(host.landed_next, vec![None]);
    assert_eq!(done.state_errors, vec!["狀態沒寫成".to_owned()]);
    assert_eq!(host.state_notes, vec!["狀態沒寫成".to_owned()]);
    assert_eq!(
        markers(&transcript(&host.root, &host.world)),
        vec![
            (TranscriptKind::Narration, None, "說到一半".to_owned(), true),
            (
                TranscriptKind::System,
                Some(EventMarker::StateUpdate),
                "hp：1".to_owned(),
                false
            ),
        ]
    );
}

#[tokio::test]
async fn aborted_narration_skips_state_and_stops() {
    let mut host = Fake::new("abort");
    let mut stopped = piece("半句", Some("boss"));
    stopped.aborted = true;
    stopped.state_updates = "不該落".to_owned();
    host.narrations = VecDeque::from([stopped]);
    let done = go(&mut host, RelayMode::Advance, 3).await;
    assert!(done.aborted);
    assert_eq!(host.reply_calls, 0);
    let events = transcript(&host.root, &host.world);
    assert_eq!(events.len(), 1);
    assert!(events[0].truncated);
    assert_eq!(
        events[0].turn_key.as_ref().map(|key| key.part.as_str()),
        Some("main")
    );
}

#[tokio::test]
async fn stop_relay_finishes_the_segment_without_another_call() {
    let mut host = Fake::new("swap");
    host.npc("boss", "老闆");
    host.halt_on_narrate = Some(Halt::StopRelay);
    let mut said = piece("落在原桌。", Some("boss"));
    said.state_updates = "地點：酒館".to_owned();
    host.narrations = VecDeque::from([said]);
    host.lines = VecDeque::from([line("不該接。")]);
    let done = go(&mut host, RelayMode::Advance, 3).await;
    assert!(!done.aborted);
    assert_eq!(host.reply_calls, 0);
    assert_eq!(transcript(&host.root, &host.world).len(), 2);
    assert!(transcript(&host.root, &host.world)
        .iter()
        .all(|event| event.marker
            != Some(EventMarker::GmCall {
                name: "老闆".to_owned()
            })));
}

#[tokio::test]
async fn interrupt_between_segments_skips_the_reply() {
    let mut host = Fake::new("between");
    host.npc("boss", "老闆");
    host.halt_after_checkpoint = Some((1, Halt::Interrupt));
    host.narrations = VecDeque::from([piece("點了。", Some("boss"))]);
    host.lines = VecDeque::from([line("不該接。")]);
    let done = go(&mut host, RelayMode::Advance, 3).await;
    assert!(done.aborted);
    assert_eq!(host.reply_calls, 0);
    assert_eq!(transcript(&host.root, &host.world).len(), 2);
}

#[tokio::test]
async fn stage_card_does_not_relay() {
    let mut host = Fake::new("stage");
    host.names.insert("stage".to_owned(), Nominee::Stop);
    host.narrations = VecDeque::from([piece("看向舞台。", Some("stage"))]);
    host.lines = VecDeque::from([line("不該接。")]);
    let done = go(&mut host, RelayMode::Advance, 3).await;
    assert!(!done.aborted);
    assert_eq!(host.reply_calls, 0);
    assert_eq!(transcript(&host.root, &host.world).len(), 1);
}

#[tokio::test]
async fn truncated_reply_does_not_stop_the_advance() {
    let mut host = Fake::new("line-cut");
    host.npc("boss", "老闆");
    host.narrations = VecDeque::from([piece("第一段。", Some("boss")), piece("第二段。", None)]);
    let mut spoken = line("沒說完");
    spoken.truncated = true;
    host.lines = VecDeque::from([spoken]);
    let done = go(&mut host, RelayMode::Advance, 3).await;
    assert_eq!(host.narrate_calls, 2);
    assert!(!done.aborted);
    assert!(transcript(&host.root, &host.world)
        .iter()
        .any(|event| event.kind == TranscriptKind::Dialogue && event.truncated));
}

#[tokio::test]
async fn direct_call_on_the_player_writes_the_name_and_does_not_reply() {
    let mut host = Fake::new("player-call");
    host.names.insert(
        PLAYER_SENTINEL.to_owned(),
        Nominee::Player {
            name: String::new(),
        },
    );
    host.lines = VecDeque::from([line("不該接。")]);
    let done = go(
        &mut host,
        RelayMode::Call {
            card_id: PLAYER_SENTINEL.to_owned(),
            list: CallList::AsHost(true),
        },
        3,
    )
    .await;
    assert_eq!(host.reply_calls, 0);
    assert!(!done.aborted);
    assert_eq!(
        transcript(&host.root, &host.world)[0].marker,
        Some(EventMarker::GmCall {
            name: String::new()
        })
    );
}

#[tokio::test]
async fn cap_already_spent_does_not_call_the_model() {
    let mut host = Fake::new("cap");
    host.narrations = VecDeque::from([piece("不該旁白。", None)]);
    let mut req = request(RelayMode::Advance, 1, RelayStep::Narrate);
    req.progress.used = 1;
    let done = drive(&mut host, req).await.unwrap();
    assert_eq!(host.narrate_calls, 0);
    assert_eq!(done.progress.step, RelayStep::Done);
}

#[tokio::test]
async fn blank_narration_does_not_land() {
    let mut host = Fake::new("blank");
    host.narrations = VecDeque::from([piece("   ", None)]);
    let done = go(&mut host, RelayMode::Narrate, 3).await;
    assert!(transcript(&host.root, &host.world).is_empty());
    assert_eq!(done.progress.last_part, None);
    assert_eq!(done.progress.used, 1);
}

#[tokio::test]
async fn scene_switched_during_narration_lands_on_the_opening_scene() {
    let mut host = Fake::new("scene");
    host.npc("boss", "老闆");
    host.switch_scene_on_narrate = true;
    let mut said = piece("還在原幕。", Some("boss"));
    said.state_updates = "地點：原幕".to_owned();
    host.narrations = VecDeque::from([said, piece("不該再旁白。", None)]);
    host.lines = VecDeque::from([line("也在原幕。")]);
    let done = go(&mut host, RelayMode::Advance, 1).await;
    assert!(!done.aborted);
    assert_eq!(host.narrate_calls, 1);
    assert_eq!(host.reply_calls, 1);
    assert_eq!(
        data::read_state(&host.root, &host.world)
            .unwrap()
            .current_scene,
        1
    );
    let original = data::read_transcript(&host.root, &host.world, 0).unwrap();
    assert_eq!(
        markers(&original),
        vec![
            (
                TranscriptKind::Narration,
                None,
                "還在原幕。".to_owned(),
                false
            ),
            (
                TranscriptKind::System,
                Some(EventMarker::StateUpdate),
                "地點：原幕".to_owned(),
                false
            ),
            (
                TranscriptKind::System,
                Some(EventMarker::GmCall {
                    name: "老闆".to_owned()
                }),
                String::new(),
                false
            ),
            (
                TranscriptKind::Dialogue,
                None,
                "也在原幕。".to_owned(),
                false
            ),
        ]
    );
    assert_eq!(original.last().unwrap().speaker_name, "老闆");
    let switched = data::read_transcript(&host.root, &host.world, 1).unwrap();
    assert!(switched.iter().all(|event| event.text == "換幕摘要"));
}

#[tokio::test]
async fn called_since_reads_the_opening_scene() {
    let mut host = Fake::new("since");
    host.npc("boss", "老闆");
    let mut narration = story_event(TranscriptKind::Narration, ("", "GM"), "先前。", "old");
    narration.id = Some("narr-1".to_owned());
    data::append_event(&host.root, &host.world, 0, &narration, None).unwrap();
    let mut call = story_event(TranscriptKind::System, ("", "GM"), "", "old");
    call.marker = Some(EventMarker::GmCall {
        name: "老闆".to_owned(),
    });
    data::append_event(&host.root, &host.world, 0, &call, None).unwrap();
    host.switch_scene_on_halt.set(true);
    host.lines = VecDeque::from([line("接在原幕。")]);
    go(
        &mut host,
        RelayMode::Call {
            card_id: "boss".to_owned(),
            list: CallList::UnlessSince {
                narration_id: "narr-1".to_owned(),
            },
        },
        3,
    )
    .await;
    let original = data::read_transcript(&host.root, &host.world, 0).unwrap();
    assert_eq!(
        original
            .iter()
            .filter(|event| matches!(event.marker, Some(EventMarker::GmCall { .. })))
            .count(),
        1
    );
    assert!(original
        .iter()
        .any(|event| event.kind == TranscriptKind::Dialogue && event.text == "接在原幕。"));
    let switched = data::read_transcript(&host.root, &host.world, 1).unwrap();
    assert!(switched.iter().all(
        |event| !matches!(event.marker, Some(EventMarker::GmCall { .. }))
            && event.kind != TranscriptKind::Dialogue
    ));
}

#[tokio::test]
async fn reply_keeps_the_name_saved_before_a_stage_switch() {
    let mut host = Fake::new("stage-mid");
    host.npc("boss", "老闆");
    host.stage_during_reply = true;
    host.narrations = VecDeque::from([piece("點了。", Some("boss")), piece("不該再旁白。", None)]);
    host.lines = VecDeque::from([line("我還是老闆。")]);
    let done = go(&mut host, RelayMode::Advance, 3).await;
    assert!(!done.aborted);
    assert_eq!(host.narrate_calls, 1);
    assert_eq!(host.reply_calls, 1);
    let spoken = transcript(&host.root, &host.world)
        .into_iter()
        .find(|event| event.kind == TranscriptKind::Dialogue)
        .expect("有回覆");
    assert_eq!(spoken.speaker_id, "boss");
    assert_eq!(spoken.speaker_name, "老闆");
    assert_eq!(spoken.text, "我還是老闆。");
}

#[tokio::test]
async fn main_land_failure_reports_the_part() {
    let mut host = Fake::new("fail");
    host.open_turns = false;
    host.narrations = VecDeque::from([piece("落不了。", None)]);
    let error = drive(
        &mut host,
        request(RelayMode::Narrate, 3, RelayStep::Narrate),
    )
    .await
    .expect_err("沒有回合紀錄就該失敗");
    assert_eq!(error.part, Some(0));
    assert!(error.receipts.is_empty());
    assert!(error.message.contains("turn-mismatch"));
}
