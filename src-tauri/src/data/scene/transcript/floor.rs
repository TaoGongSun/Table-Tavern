//! 收回一樓、整組放回（undo-two-clicks-summary-line）：GM 回合在逐字稿裡是好幾行（登場塊、正文、變動摘要、
//! 點名），玩家按一下「收回上一句」要整樓收走，復原也整組放回；兩邊都在一把鎖、一次重寫、檯面只回捲一次。
use super::super::marker::EventMarker;
use super::{edit_scene, rewind_state, TranscriptEvent, TranscriptKind};
use crate::data::message_vars;
use crate::data::state::{read_state, read_state_cache, write_state};
use crate::data::state_commit::with_commit;
use crate::data::DataResult;
use std::path::Path;

/// 附屬行：只跟著它前面那一樓走，自己不成樓。
fn is_accessory(event: &TranscriptEvent) -> bool {
    matches!(
        event.marker,
        Some(EventMarker::StateUpdate) | Some(EventMarker::GmCall { .. })
    )
}

/// 登場塊：GM 提交後、正文前寫下的登場與私設，快照已是回合後，要跟正文同進退。
fn is_arrival_block(event: &TranscriptEvent) -> bool {
    matches!(
        event.marker,
        Some(EventMarker::CardArrival { .. })
            | Some(EventMarker::PersonArrival { .. })
            | Some(EventMarker::CardPrivate { .. })
    )
}

/// 真正的一樓：玩家句、GM 旁白、角色回覆、開場白。附屬行只掛在這種樓上。
fn is_story_floor(event: &TranscriptEvent) -> bool {
    event.opening || event.kind != TranscriptKind::System
}

/// 這一幕最後一樓從哪一則開始（含）；空幕回 None。
/// 尾端連續附屬行先收；前面是真正的一樓就連它一起收，不是（換幕摘要、私設、無標記舊系統行）就只收附屬行，
/// 不穿越邊界。收到的那一樓再往前吃緊貼、`action_id` 相同（兩邊都有值）的登場塊。
pub(super) fn floor_start(events: &[TranscriptEvent]) -> Option<usize> {
    let mut start = events.len();
    if start == 0 {
        return None;
    }
    while start > 0 && is_accessory(&events[start - 1]) {
        start -= 1;
    }
    let floor = match start.checked_sub(1) {
        Some(index) if start == events.len() || is_story_floor(&events[index]) => index,
        _ => return Some(start),
    };
    start = floor;
    let head = &events[floor];
    if is_story_floor(head) || is_arrival_block(head) {
        if let Some(action) = head.action_id.as_deref() {
            while start > 0
                && is_arrival_block(&events[start - 1])
                && events[start - 1].action_id.as_deref() == Some(action)
            {
                start -= 1;
            }
        }
    }
    Some(start)
}

/// 收回上一樓（可連按）：回傳實際刪掉的事件（原序）；這一幕已經空了就是空的，不倒退咬到上一幕。
pub fn pop_transcript_floor(
    root: &Path,
    world_id: &str,
    scene: u64,
) -> DataResult<Vec<TranscriptEvent>> {
    with_commit(root, world_id, |tx| {
        let popped = edit_scene(root, world_id, scene, |mut events| {
            Ok(match floor_start(&events) {
                None => (None, None),
                Some(start) => {
                    let removed = events.split_off(start);
                    (Some((removed, events.clone())), Some(events))
                }
            })
        })?;
        let Some((removed, remaining)) = popped else {
            return Ok(Vec::new());
        };
        rewind_state(tx, scene, &remaining)?;
        Ok(removed)
    })
}

/// 多人房間收回一個派工（room-host-actions §4）：從桌尾逐樓往前，樓頭 `action_id` 是這個派工就收，不是就停；
/// 一次原子重寫、檯面回捲一次，不留復原疊。回 true＝收乾淨了（含本幕早就沒有它的事件＝已收過）；
/// false＝它的事件還在、但不在桌尾，什麼都不改。
pub fn pop_action_floors(
    root: &Path,
    world_id: &str,
    scene: u64,
    action_id: &str,
) -> DataResult<bool> {
    with_commit(root, world_id, |tx| {
        let cut = edit_scene(root, world_id, scene, |mut events| {
            let ours = |event: &TranscriptEvent| event.action_id.as_deref() == Some(action_id);
            let mut end = events.len();
            while let Some(start) = floor_start(&events[..end]) {
                if ours(&events[start]) {
                    end = start;
                    continue;
                }
                // 樓頭是別的派工：尾端還黏著這個派工的附屬行（「讓 X 接話」的點名列跟著上一則旁白成樓）就切到那裡
                end = (start..end)
                    .find(|&i| events[i..end].iter().all(ours))
                    .unwrap_or(end);
                break;
            }
            if events[..end].iter().any(ours) {
                return Ok((None, None));
            }
            if end == events.len() {
                return Ok((Some(None), None));
            }
            events.truncate(end);
            Ok((Some(Some(events.clone())), Some(events)))
        })?;
        match cut {
            None => Ok(false),
            Some(None) => Ok(true),
            Some(Some(remaining)) => rewind_state(tx, scene, &remaining).map(|()| true),
        }
    })
}

/// 復原整組：照原序一次寫回（先交接上一回合沒落成的部分），每則照一般追加的規矩配 ID、換新版本 token、
/// 沒快照才借目前檯面；檯面採最後一則的快照。整檔原子重寫，失敗就一則都沒放回、可整組重試。
pub fn restore_transcript_events(
    root: &Path,
    world_id: &str,
    scene: u64,
    events: &[TranscriptEvent],
) -> DataResult<Vec<TranscriptEvent>> {
    if events.is_empty() {
        return Ok(Vec::new());
    }
    with_commit(root, world_id, |tx| {
        message_vars::settle_before_append(tx, None)?;
        let current = read_state(root, world_id)?.state;
        let restored: Vec<TranscriptEvent> = events
            .iter()
            .map(|event| {
                let mut event = event.clone();
                if event.id.is_none() {
                    event.id = Some(message_vars::new_token());
                }
                if event.message_vars.is_some() {
                    event.vars_rev = Some(message_vars::new_token());
                } else {
                    event.vars_rev = None;
                    event.vars_epoch = None;
                }
                if event.state.is_none() {
                    event.state = Some(current.clone());
                }
                event
            })
            .collect();
        edit_scene(root, world_id, scene, |mut existing| {
            existing.extend(restored.iter().cloned());
            Ok(((), Some(existing)))
        })?;
        // 快取寫失敗不把「已經寫回」變成錯誤，權威在逐字稿
        if let Some(snapshot) = restored.last().and_then(|event| event.state.as_ref()) {
            if let Ok(mut world) = read_state_cache(root, world_id) {
                if world.state != *snapshot {
                    world.state = snapshot.clone();
                    let _ = write_state(root, world_id, &world);
                }
            }
        }
        message_vars::refresh_cache(tx);
        Ok(restored)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(
        kind: TranscriptKind,
        marker: Option<EventMarker>,
        action: Option<&str>,
    ) -> TranscriptEvent {
        TranscriptEvent {
            dice_pool: None,
            ts: String::new(),
            speaker_id: String::new(),
            speaker_name: String::new(),
            kind,
            text: "x".to_owned(),
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
            action_id: action.map(str::to_owned),
        }
    }

    fn player(action: &str) -> TranscriptEvent {
        line(TranscriptKind::Player, None, Some(action))
    }
    fn gm(action: Option<&str>) -> TranscriptEvent {
        line(TranscriptKind::Narration, None, action)
    }
    fn summary(action: Option<&str>) -> TranscriptEvent {
        line(
            TranscriptKind::System,
            Some(EventMarker::StateUpdate),
            action,
        )
    }
    fn call(action: Option<&str>) -> TranscriptEvent {
        line(
            TranscriptKind::System,
            Some(EventMarker::GmCall { name: "甲".into() }),
            action,
        )
    }
    fn arrival(action: Option<&str>) -> TranscriptEvent {
        line(
            TranscriptKind::System,
            Some(EventMarker::CardArrival { name: "甲".into() }),
            action,
        )
    }
    fn private(action: Option<&str>) -> TranscriptEvent {
        line(
            TranscriptKind::System,
            Some(EventMarker::CardPrivate { name: "甲".into() }),
            action,
        )
    }

    #[test]
    fn gm_floor_takes_summary_call_and_same_action_arrival_block_but_not_the_player_line() {
        let events = vec![
            gm(None),
            player("a"),
            line(
                TranscriptKind::System,
                Some(EventMarker::PersonArrival {
                    title: "乙".into()
                }),
                Some("a"),
            ),
            private(Some("a")),
            arrival(Some("a")),
            private(Some("a")),
            arrival(Some("a")),
            gm(Some("a")),
            summary(Some("a")),
            call(Some("a")),
        ];
        assert_eq!(floor_start(&events), Some(2));
        assert_eq!(floor_start(&events[..2]), Some(1));
        assert_eq!(floor_start(&[]), None);
    }

    #[test]
    fn arrival_block_needs_the_same_present_action_id() {
        // 舊登場行沒有 action_id：獨立一樓
        assert_eq!(
            floor_start(&[player("a"), arrival(None), gm(Some("a"))]),
            Some(2)
        );
        assert_eq!(
            floor_start(&[player("a"), arrival(Some("b")), gm(Some("a"))]),
            Some(2)
        );
        // 正文自己沒有 action_id 也不吃（None 對 None 不算同一回合）
        assert_eq!(
            floor_start(&[player("a"), arrival(None), gm(None)]),
            Some(2)
        );
        // 不同動作的私設把塊切斷
        assert_eq!(
            floor_start(&[
                player("a"),
                arrival(Some("a")),
                private(Some("b")),
                arrival(Some("a")),
                gm(Some("a"))
            ]),
            Some(3)
        );
    }

    #[test]
    fn accessories_do_not_cross_independent_lines() {
        let scene_summary = line(
            TranscriptKind::System,
            Some(EventMarker::SceneSummary),
            None,
        );
        let legacy = line(TranscriptKind::System, None, None);
        for boundary in [
            scene_summary,
            legacy,
            private(Some("a")),
            arrival(Some("a")),
        ] {
            let events = vec![boundary.clone(), summary(Some("a")), call(Some("a"))];
            assert_eq!(floor_start(&events), Some(1), "{:?}", boundary.marker);
            // 邊界自己成一樓
            assert_eq!(floor_start(&events[..1]), Some(0));
        }
        // 整幕只剩附屬行：一起收
        assert_eq!(floor_start(&[summary(None), call(None)]), Some(0));
    }

    #[test]
    fn character_reply_is_its_own_floor_and_the_gm_call_stays_with_the_narration() {
        let reply = line(TranscriptKind::Dialogue, None, Some("a"));
        let events = vec![
            player("a"),
            gm(Some("a")),
            summary(Some("a")),
            call(Some("a")),
            reply,
        ];
        assert_eq!(floor_start(&events), Some(4));
        assert_eq!(floor_start(&events[..4]), Some(1));
    }

    #[test]
    fn opening_is_a_floor() {
        let mut opening = line(TranscriptKind::System, None, None);
        opening.opening = true;
        assert_eq!(floor_start(&[opening.clone(), summary(None)]), Some(0));
    }

    /// 收回派工：尾段整批收（含登場塊、點名、離席標記），已收過回 true 不動，不在桌尾回 false 不動；改名失敗一則都不收
    #[test]
    fn pop_action_floors_takes_the_tail_batch_or_nothing() {
        let root = crate::smart_free::test_root("pop-action-floors");
        let world = crate::data::create_world(&root, "收回桌").unwrap();
        let absent = line(
            TranscriptKind::System,
            Some(EventMarker::PlayerAbsent { name: "乙".into() }),
            Some("d2"),
        );
        let reply = line(TranscriptKind::Dialogue, None, Some("d2"));
        let lines = [
            gm(Some("d1")),
            absent,
            player("d2"),
            arrival(Some("d2")),
            gm(Some("d2")),
            summary(Some("d2")),
            call(Some("d2")),
            reply,
        ];
        for event in &lines {
            crate::data::append_event(&root, &world, 0, event, None).unwrap();
        }
        let count = || {
            crate::data::read_transcript(&root, &world, 0)
                .unwrap()
                .len()
        };
        {
            let _fail = crate::data::world_file::RenameFailGuard::fail_ending(".jsonl", 1);
            assert!(pop_action_floors(&root, &world, 0, "d2").is_err());
        }
        assert_eq!(count(), 8, "重寫失敗一則都不收");
        assert!(
            !pop_action_floors(&root, &world, 0, "d1").unwrap(),
            "不在桌尾"
        );
        assert_eq!(count(), 8);
        assert!(pop_action_floors(&root, &world, 0, "d2").unwrap());
        assert_eq!(count(), 1);
        assert!(pop_action_floors(&root, &world, 0, "d2").unwrap(), "已收過");
        assert_eq!(count(), 1);
        // 「讓 X 接話」：點名列（d3）跟著上一則旁白（d1）成樓，照樣收得掉
        for event in [
            call(Some("d3")),
            line(TranscriptKind::Dialogue, None, Some("d3")),
        ] {
            crate::data::append_event(&root, &world, 0, &event, None).unwrap();
        }
        assert!(pop_action_floors(&root, &world, 0, "d3").unwrap());
        assert_eq!(count(), 1);
    }
}
