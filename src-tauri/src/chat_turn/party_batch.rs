//! 多人房間的行動批次落檔（multiplayer-room 包 3）：派工（協定 §6.4）的 departures／arrivals／actions 依序落成
//! 在場標記與玩家事件，`action_id = dispatch_id` 去重。包 2 的房間模組接到派工先呼叫這裡，之後照舊跑 GM 鏈。
use crate::data::{self, EventMarker, TranscriptEvent, TranscriptKind};
use std::path::Path;

/// 協定 `ActionItem` 的執行端投影：名字一律取本機卡名，不收派工帶來的 `name`。
pub(crate) enum PartyAction {
    /// 行動文字（玩家輸入，信任邊界：這裡再驗一次長度，不只信伺服器）
    Act { card_id: String, text: String },
    /// 這回合沒行動。`to_gm`＝玩家按「交給 GM」（`watchReason: "skipped"`），落成交給 GM 標記；
    /// 逾時、房主結算都是一般觀望
    Watch { card_id: String, to_gm: bool },
}

/// 離席處置（協定 `departures[].mode`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DepartureMode {
    /// 卡回 NPC 池：之後是一般角色卡，GM 可以點名、由角色線接話
    Npc,
    /// 標離席：卡留在桌上但不點名
    Absent,
    /// 暫時交給 AI 代管：跟 Npc 一樣由 GM 演，只差標記寫「暫離」（座位在房間層保留，之後接回）
    Delegate,
}

/// 一次派工要落的東西（協定 `Dispatch` 去掉房間層欄位）。
pub(crate) struct PartyBatch {
    pub dispatch_id: String,
    /// 依序；open／scene_open 回合為空
    pub actions: Vec<PartyAction>,
    /// 在場真人玩家卡 id，固定順序；第一張當主玩家（`player_card_id`）
    pub players: Vec<String>,
    pub arrivals: Vec<String>,
    pub departures: Vec<(String, DepartureMode)>,
    /// 本回合從 AI 代管接回的卡（arrivals 的子集）：寫「玩家回來」，不寫「玩家登場」
    pub returns: Vec<String>,
}

/// `Duplicate`＝本幕已有 `action_id == dispatch_id` 的事件，整批跳過、什麼都沒寫；`Appended` 帶寫了幾則。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum BatchOutcome {
    Appended(usize),
    Duplicate,
}

/// 行動文字上限（協定 §10：送出時 1–2000 字元）。
const ACTION_MAX_CHARS: usize = 2000;

/// 落一批：去重 → 驗卡與文字 → 換幕容量鎖（整批文字當一句草稿，同 `append_player_event`）→ 換上在場玩家 →
/// 依序寫離席、登場、行動（觀望寫標記）。驗證或容量不過就回錯、什麼都不寫。
/// 呼叫端照 commands 慣例先持 `data::world_write_permit`。
pub(crate) fn append_party_batch(
    config_root: &Path,
    root: &Path,
    world_id: &str,
    batch: &PartyBatch,
) -> Result<BatchOutcome, String> {
    if landed(root, world_id, &batch.dispatch_id)? {
        return Ok(BatchOutcome::Duplicate);
    }
    let events = batch_events(root, world_id, batch)?;
    let draft: Vec<&str> = batch
        .actions
        .iter()
        .filter_map(|action| match action {
            PartyAction::Act { text, .. } => Some(text.trim()),
            PartyAction::Watch { .. } => None,
        })
        .collect();
    crate::scene_budget::check_capacity(
        config_root,
        root,
        world_id,
        Some(&batch.dispatch_id),
        &draft.join("\n"),
    )?;
    let ids = |modes: &[DepartureMode]| -> Vec<String> {
        batch
            .departures
            .iter()
            .filter(|(_, departed)| modes.contains(departed))
            .map(|(id, _)| id.clone())
            .collect()
    };
    // 去重複查、換玩家、寫事件同一把提交鎖：兩次同時送進來也只落一次
    data::state_commit::with_commit(root, world_id, |_| {
        if landed(root, world_id, &batch.dispatch_id)? {
            return Ok(BatchOutcome::Duplicate);
        }
        data::set_party(
            root,
            world_id,
            &batch.players,
            &ids(&[DepartureMode::Absent]),
            &ids(&[DepartureMode::Npc, DepartureMode::Delegate]),
        )
        .map_err(|error| error.to_string())?;
        let scene = data::read_state(root, world_id)
            .map_err(|error| error.to_string())?
            .current_scene;
        for event in &events {
            data::append_event(root, world_id, scene, event, None)
                .map_err(|error| error.to_string())?;
        }
        Ok(BatchOutcome::Appended(events.len()))
    })
}

/// 本幕是否已有這個派工落下的事件（玩家句、觀望與在場標記、GM 回覆都蓋同一個 action_id）。
fn landed(root: &Path, world_id: &str, dispatch_id: &str) -> Result<bool, String> {
    let scene = data::read_state(root, world_id)
        .map_err(|error| error.to_string())?
        .current_scene;
    Ok(data::read_transcript(root, world_id, scene)
        .map_err(|error| error.to_string())?
        .iter()
        .any(|event| event.action_id.as_deref() == Some(dispatch_id)))
}

/// 驗證並組出要落的事件（順序＝離席、登場、行動）。卡要在這桌、行動者要在在場名單、文字 1–2000 字元。
fn batch_events(
    root: &Path,
    world_id: &str,
    batch: &PartyBatch,
) -> Result<Vec<TranscriptEvent>, String> {
    let name = |card_id: &str| {
        data::read_character(root, world_id, card_id)
            .map(|card| card.name)
            .map_err(|error| format!("派工引用的卡 {card_id} 讀不到：{error}"))
    };
    for (index, card_id) in batch.players.iter().enumerate() {
        name(card_id)?;
        if batch.players[..index].contains(card_id) {
            return Err(format!("在場玩家 {card_id} 重複"));
        }
    }
    let event = |kind, name: String, text: String, marker| TranscriptEvent {
        dice_pool: None,
        ts: data::local_timestamp().unwrap_or_default(),
        speaker_id: String::new(),
        speaker_name: name,
        kind,
        text,
        raw: None,
        state: None,
        truncated: false,
        gm_only: false,
        marker,
        opening: false,
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: Some(batch.dispatch_id.clone()),
    };
    let system = |marker| {
        event(
            TranscriptKind::System,
            "GM".to_owned(),
            String::new(),
            Some(marker),
        )
    };
    let mut events = Vec::new();
    // 伺服器正常操作會送：離席後又選回同卡（離席＋在場）、中途加入又在結算前離開（登場＋不在場）。
    // 以 players 為準略過那筆標記、其餘照落，不整批回錯（回錯會讓同一派工 retry 永遠失敗）
    let present = |id: &String| batch.players.contains(id);
    for (card_id, mode) in batch.departures.iter().filter(|(id, _)| !present(id)) {
        let name = name(card_id)?;
        events.push(system(match mode {
            DepartureMode::Npc => EventMarker::PlayerToNpc { name },
            DepartureMode::Absent => EventMarker::PlayerAbsent { name },
            DepartureMode::Delegate => EventMarker::PlayerDelegated { name },
        }));
    }
    for card_id in batch.arrivals.iter().filter(|id| present(id)) {
        let name = name(card_id)?;
        events.push(system(match batch.returns.contains(card_id) {
            true => EventMarker::PlayerReturned { name },
            false => EventMarker::PlayerArrival { name },
        }));
    }
    for action in &batch.actions {
        let (PartyAction::Act { card_id, .. } | PartyAction::Watch { card_id, .. }) = action;
        if !batch.players.contains(card_id) {
            return Err(format!("行動者 {card_id} 不在這回合的在場玩家裡"));
        }
        let name = name(card_id)?;
        events.push(match action {
            PartyAction::Watch { to_gm: true, .. } => {
                system(EventMarker::PlayerHandedToGm { name })
            }
            PartyAction::Watch { .. } => system(EventMarker::PlayerWatch { name }),
            PartyAction::Act { text, .. } => {
                // 同前端單人送出：去頭尾空白後才驗、才落檔
                let text = text.trim();
                let count = text.chars().count();
                if text.is_empty() || count > ACTION_MAX_CHARS {
                    return Err(format!("{name} 的行動文字長度不合（{count} 字元）"));
                }
                event(TranscriptKind::Player, name, text.to_owned(), None)
            }
        });
    }
    Ok(events)
}

#[cfg(test)]
#[path = "party_batch_tests.rs"]
mod tests;
