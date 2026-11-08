//! 單人桌的接力 host：一顆按鈕＝一條接力。模型呼叫與落檔都在 `relay::drive`，
//! 這裡經 Channel 回報段落開始（含 turn id）、增量、每則落檔、登場卡，最後回整條的收據。
//! 停止走 `chat_abort`（帶接力 id）；換桌換幕由前端送 `release`，在途那段照常落完就不再接。
use super::relay::{
    self, CallList, Halt, HostFut, Landed, Line, Nominee, PartClose, Piece, RelayHost, RelayMode,
    RelayProgress, RelayRequest, RelayStep,
};
use crate::{config_root, data, data_root, inflight, transport};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;
use tauri::ipc::{Channel, InvokeResponseBody};

const EMPTY_REPLY: &str = "AI_EMPTY_RESPONSE: 空白回合不寫進故事";

#[derive(Default)]
struct Signal {
    interrupt: bool,
    release: bool,
    /// 目前在途的模型呼叫；停止時連它一起中止。
    turn: Option<String>,
}

type Key = (String, String);

fn signals() -> MutexGuard<'static, HashMap<Key, Signal>> {
    static SIGNALS: OnceLock<Mutex<HashMap<Key, Signal>>> = OnceLock::new();
    SIGNALS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

pub(crate) fn active(world_id: &str) -> bool {
    signals().keys().any(|(world, _)| world == world_id)
}

/// 接力在跑的期間登記著；結束（含失敗、被取消）就摘掉。
struct Registration(Key);

impl Drop for Registration {
    fn drop(&mut self) {
        signals().remove(&self.0);
    }
}

/// 玩家按停止：整條接力不再發新呼叫，在途那段由 `inflight` 中止。對不上的 id 什麼都不做。
pub(crate) fn interrupt(world_id: &str, relay_id: &str) {
    let turn = match signals().get_mut(&(world_id.to_owned(), relay_id.to_owned())) {
        Some(signal) => {
            signal.interrupt = true;
            signal.turn.clone()
        }
        None => return,
    };
    if let Some(turn) = turn {
        inflight::abort_turn(world_id, &turn);
    }
}

/// 換桌換幕：在途那段照常落完，之後不再接。
pub(crate) fn release(world_id: &str, relay_id: &str) {
    if let Some(signal) = signals().get_mut(&(world_id.to_owned(), relay_id.to_owned())) {
        signal.release = true;
    }
}

/// 整條接力的結果。`error` 有值＝失敗，收據照樣帶（前端據此判斷玩家句算不算已回）。
/// `resync`：開場時有上一回合沒落成的部分，這條接力的第一步已由後端代落，畫面要重讀。
/// `owed`：收尾時仍有沒落成的部分，下一筆追加（玩家句、復原）會先代落，前端到時重讀。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RelayReport {
    aborted: bool,
    receipts: Vec<data::TranscriptEvent>,
    state_errors: Vec<String>,
    error: Option<String>,
    resync: bool,
    owed: bool,
}

struct SoloHost {
    app: tauri::AppHandle,
    root: PathBuf,
    world: String,
    key: Key,
    action: String,
    player: String,
    out: Channel<Value>,
}

impl SoloHost {
    fn send(&self, message: Value) {
        let _ = self.out.send(message);
    }

    /// 既有回合收 `Channel<String>`：每則增量轉成一則 `delta`。
    fn deltas(&self) -> Channel<String> {
        let out = self.out.clone();
        Channel::new(move |body| {
            if let InvokeResponseBody::Json(json) = body {
                if let Ok(text) = serde_json::from_str::<String>(&json) {
                    let _ = out.send(json!({ "type": "delta", "text": text }));
                }
            }
            Ok(())
        })
    }

    fn start(&self, turn: &str, speaker: &str, kind: &str) {
        self.send(json!({ "type": "start", "turnId": turn, "speaker": speaker, "kind": kind }));
    }
}

impl RelayHost for SoloHost {
    fn halt(&self) -> Halt {
        halt_of(&self.key)
    }

    fn begin_turn(&mut self) -> String {
        let turn = data::new_id();
        if let Some(signal) = signals().get_mut(&self.key) {
            signal.turn = Some(turn.clone());
        }
        turn
    }

    fn narrate(&mut self, turn_id: String, _part: u32) -> HostFut<'_, Result<Piece, String>> {
        self.start(&turn_id, "", "narration");
        let deltas = self.deltas();
        Box::pin(async move {
            let n = super::gm_narrate(
                self.app.clone(),
                self.world.clone(),
                turn_id,
                deltas,
                Some(self.action.clone()),
                false,
            )
            .await?;
            if !n.aborted && n.text.trim().is_empty() {
                return Err(EMPTY_REPLY.to_owned());
            }
            if !n.arrived_characters.is_empty() {
                self.send(json!({ "type": "arrived", "ids": n.arrived_characters }));
            }
            Ok(Piece {
                state_updates: n.state_update_text(),
                text: n.text,
                raw: n.raw,
                next: n.next,
                next_player: n.next_player,
                arrived: n.arrived_characters,
                aborted: n.aborted,
                truncated: n.truncated,
                state_error: n.state_error,
                status: None,
                panel: None,
            })
        })
    }

    fn reply(
        &mut self,
        card_id: String,
        turn_id: String,
        _part: u32,
    ) -> HostFut<'_, Result<Line, String>> {
        self.start(&turn_id, &card_id, "dialogue");
        let deltas = self.deltas();
        Box::pin(async move {
            let reply = super::chat_with_character(
                self.app.clone(),
                self.world.clone(),
                card_id,
                turn_id,
                deltas,
                Some(self.action.clone()),
            )
            .await?;
            if !reply.aborted && reply.text.trim().is_empty() {
                return Err(EMPTY_REPLY.to_owned());
            }
            Ok(Line {
                text: reply.text,
                raw: reply.raw,
                aborted: reply.aborted,
                truncated: reply.truncated,
            })
        })
    }

    fn classify(&self, next: &str, _next_player: Option<&str>) -> Nominee {
        classify(&self.root, &self.world, &self.player, next)
    }

    fn landed(&mut self, landed: &Landed) {
        self.send(json!({ "type": "landed", "event": landed.event, "next": landed.next }));
    }

    fn note_state_error(&mut self, _message: &str) {}

    fn checkpoint(&mut self, _progress: &RelayProgress, close: Option<PartClose>) -> bool {
        close.is_some()
    }

    fn roots(&self) -> (&Path, &str) {
        (&self.root, &self.world)
    }
}

/// 停止優先於換桌：兩個都送了就當停止。
fn halt_of(key: &Key) -> Halt {
    match signals().get(key) {
        Some(signal) if signal.interrupt => Halt::Interrupt,
        Some(signal) if signal.release => Halt::StopRelay,
        _ => Halt::None,
    }
}

/// 點到玩家用 `player`（可空字串：顯示與送 AI 時再照當下語系補稱呼）；讀不到卡或是舞台卡就停。
fn classify(root: &Path, world_id: &str, player: &str, next: &str) -> Nominee {
    if next == transport::PLAYER_SENTINEL {
        return Nominee::Player {
            name: player.to_owned(),
        };
    }
    match data::read_character(root, world_id, next) {
        Ok(card) if !card.stage => Nominee::Npc {
            id: next.to_owned(),
            name: card.name,
        },
        _ => Nominee::Stop,
    }
}

/// 上一回合有提交了卻沒落檔的部分（任何新事件之前後端會先代落）。
fn owed(root: &Path, world_id: &str) -> bool {
    data::state_commit::with_commit(root, world_id, data::message_vars::has_unlanded_reply)
}

fn max_speakers(config: &data::AppConfig) -> u32 {
    config
        .preferences
        .get("max_round_speakers")
        .and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()))
        .filter(|max| *max > 0)
        .map_or(3, |max| max.min(u64::from(u32::MAX)) as u32)
}

/// 入口對驅動模式（方案 §1 入口與點名契約表）。
fn relay_mode(
    mode: &str,
    card_id: Option<String>,
    narration_id: Option<String>,
) -> Result<(RelayMode, RelayStep), String> {
    Ok(match (mode, card_id) {
        ("narrate", _) => (RelayMode::Narrate, RelayStep::Narrate),
        ("narrateThenOne", _) => (RelayMode::NarrateThenOne, RelayStep::Narrate),
        ("advance", _) => (RelayMode::Advance, RelayStep::Narrate),
        // 請 X 發言：不寫點名列
        ("request", Some(card)) => (
            RelayMode::Call {
                card_id: card.clone(),
                list: CallList::Omit,
            },
            RelayStep::Call(card),
        ),
        // 讓 X 接話：寫點名列，那則旁白之後已有同名的就不疊
        ("nominated", Some(card)) => (
            RelayMode::Call {
                card_id: card.clone(),
                list: CallList::UnlessSince {
                    narration_id: narration_id.ok_or("讓 X 接話要帶旁白 id")?,
                },
            },
            RelayStep::Call(card),
        ),
        _ => return Err(format!("不認得的接力入口：{mode}")),
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn chat_relay(
    app: tauri::AppHandle,
    world_id: String,
    relay_id: String,
    mode: String,
    card_id: Option<String>,
    narration_id: Option<String>,
    action_id: String,
    player_name: String,
    on_event: Channel<Value>,
) -> Result<RelayReport, String> {
    // 先登記：讀設定、查代落期間按的停止也要收得到
    let key = (world_id.clone(), relay_id);
    signals().insert(key.clone(), Signal::default());
    let _registration = Registration(key.clone());
    let (mode, step) = relay_mode(&mode, card_id, narration_id)?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let root = data_root(&app)?;
    let resync = owed(&root, &world_id);
    let mut host = SoloHost {
        app,
        root: root.clone(),
        world: world_id.clone(),
        key,
        action: action_id.clone(),
        player: player_name,
        out: on_event,
    };
    let request = RelayRequest {
        mode,
        max_speakers: max_speakers(&config),
        action_id,
        progress: RelayProgress {
            used: 0,
            step,
            last_part: None,
            arrived: Vec::new(),
            called: None,
        },
        retry_delay: Duration::from_millis(150),
    };
    let (aborted, receipts, state_errors, error) = match relay::drive(&mut host, request).await {
        Ok(done) => (done.aborted, done.receipts, done.state_errors, None),
        Err(fail) => (false, fail.receipts, Vec::new(), Some(fail.message)),
    };
    Ok(RelayReport {
        aborted,
        receipts,
        state_errors,
        error,
        resync,
        owed: owed(&root, &world_id),
    })
}

#[cfg(test)]
#[path = "solo_relay_tests.rs"]
mod tests;
