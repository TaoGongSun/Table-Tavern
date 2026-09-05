use crate::mechanism::{self, Outcome};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use super::super::{DataResult, invalid_data};
use super::super::paths::world_dir;
use super::super::state::{TableState, WorldState, read_state, write_state};



#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptKind {
    Dialogue,
    Narration,
    Player,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranscriptEvent {
    pub ts: String,
    /// 角色事件存角色 id；GM 旁白／系統訊息／玩家發言存空字串（kind 已足以區分）
    pub speaker_id: String,
    /// 發言當下的顯示名快照——改名後舊事件不動，這是既有拍板行為
    pub speaker_name: String,
    pub kind: TranscriptKind,
    pub text: String,
    /// 剝殼前的模型原文：狀態區塊與點名行都還在，供卡片自帶的面板重畫歷史訊息用。
    /// 與 text 相同（沒剝到東西）時不存，舊檔沒有這欄也照樣讀得起來。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<TableState>,
    /// 這則系統事件的全文只給 GM 看；chars 續聊線遇到只留第一行（AI 卡重構包 4b，
    /// 補 4a 遺留的 visibility 洩漏——非 Public 世界書人物的登場全文不該流進扮演引擎）。
    #[serde(default)]
    pub gm_only: bool,
}

pub(super) fn transcript_path(root: &Path, world_id: &str, scene: u64) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?
        .join("transcript")
        .join(format!("{scene}.jsonl")))
}

pub fn append_transcript(
    root: &Path,
    world_id: &str,
    scene: u64,
    event: &TranscriptEvent,
) -> DataResult<()> {
    let mut event = event.clone();
    if event.state.is_none() {
        // 復原舊句子會帶回當時快照，只有新事件才借用目前檯面。
        event.state = read_state(root, world_id).ok().map(|state| state.state);
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(transcript_path(root, world_id, scene)?)?;
    serde_json::to_writer(&mut file, &event)?;
    file.write_all(b"\n")?;
    // 目前值恆等於最後一則事件的快照，復原舊句時狀態才會跟著回到那一刻。
    // 快取寫失敗不該把「事件已經寫進去了」這件事變成錯誤，權威在 transcript。
    if let Some(snapshot) = event.state {
        if let Ok(mut world) = read_state(root, world_id) {
            if world.state != snapshot {
                world.state = snapshot;
                let _ = write_state(root, world_id, &world);
            }
        }
    }
    Ok(())
}

/// 開場白也要存成快照，收回時檯面才能回到貼上前的最後一句；狀態區塊走與 GM 回覆同一條
/// 本地權威（mechanism::apply_block），增量桌的數值一開場就是本機在算。
pub fn append_opening(
    root: &Path,
    world_id: &str,
    scene: u64,
    ts: &str,
    raw: &str,
    block: &crate::transport::StateBlock,
    user_name: &str,
) -> DataResult<(TranscriptEvent, Outcome)> {
    let mut world = read_state(root, world_id)?;
    let outcome = mechanism::apply_block(&mut world, block, user_name);
    let event = TranscriptEvent {
        ts: ts.to_owned(),
        speaker_id: String::new(),
        speaker_name: "GM".to_owned(),
        kind: TranscriptKind::Narration,
        text: block.display.clone(),
        raw: (raw != block.display).then(|| raw.to_owned()),
        state: Some(world.state),
        gm_only: false,
    };
    append_transcript(root, world_id, scene, &event)?;
    Ok((event, outcome))
}

/// 整檔重寫這一幕，並把檯面退回剩下事件的最後一份快照（這一幕沒了就往前一幕找）。
/// 刪事件的兩條路（收回上一句、復原匯入收掉開場白）共用。
fn rewrite_scene(
    root: &Path,
    world_id: &str,
    scene: u64,
    events: &[TranscriptEvent],
) -> DataResult<()> {
    let mut buffer = String::new();
    for event in events {
        buffer.push_str(&serde_json::to_string(event)?);
        buffer.push('\n');
    }
    fs::write(transcript_path(root, world_id, scene)?, buffer)?;
    let mut state = read_state(root, world_id)?;
    state.state = events
        .iter()
        .rev()
        .find_map(|entry| entry.state.clone())
        .or_else(|| {
            scene.checked_sub(1).and_then(|previous_scene| {
                read_transcript(root, world_id, previous_scene)
                    .ok()
                    .and_then(|previous_events| {
                        previous_events
                            .iter()
                            .rev()
                            .find_map(|entry| entry.state.clone())
                    })
            })
        })
        .unwrap_or_default();
    write_state(root, world_id, &state)?;
    Ok(())
}

/// 狀態樹被逐字稿以外的路徑換掉（重構套用重建欄位）之後，把新樹補進這一幕每一則事件的快照。
/// 收回上一句與換幕都拿事件快照當回捲基準，不補的話玩家一收回，介面就被打回重構前的舊欄位。
/// 補整幕而不是只補最後一則：連按收回會一路往前吃，任何一則留著舊欄位都會在那一下現形。
/// 只換 tree／jumps——劇情面的欄位（table、changes、notes）照舊跟著各自那一刻走。
pub fn sync_scene_state_tree(root: &Path, world_id: &str, state: &WorldState) -> DataResult<()> {
    let scene = state.current_scene;
    let mut events = read_transcript(root, world_id, scene)?;
    let mut touched = false;
    for event in events.iter_mut() {
        let Some(snapshot) = event.state.as_mut() else {
            continue;
        };
        if snapshot.tree != state.state.tree || snapshot.jumps != state.state.jumps {
            snapshot.tree = state.state.tree.clone();
            snapshot.jumps = state.state.jumps.clone();
            touched = true;
        }
    }
    if touched {
        rewrite_scene(root, world_id, scene, &events)?;
    }
    Ok(())
}

/// 收回上一句（可連按）：砍掉這一幕最後一筆事件後整檔重寫。
/// 回傳是否真的刪了——這一幕已經空了就是 false，收不會倒退咬到上一幕。
pub fn pop_transcript(root: &Path, world_id: &str, scene: u64) -> DataResult<bool> {
    let mut events = read_transcript(root, world_id, scene)?;
    if events.pop().is_none() {
        return Ok(false);
    }
    rewrite_scene(root, world_id, scene, &events)?;
    Ok(true)
}

/// 復原匯入用：從這一幕刪掉時間戳相符的那一則（貼出的開場白），其餘事件原位不動。
/// 回傳是否真的刪到——玩家自己先收回過就是 false。
pub fn remove_transcript_event(
    root: &Path,
    world_id: &str,
    scene: u64,
    ts: &str,
) -> DataResult<bool> {
    let mut events = read_transcript(root, world_id, scene)?;
    let before = events.len();
    events.retain(|event| event.ts != ts);
    if events.len() == before {
        return Ok(false);
    }
    rewrite_scene(root, world_id, scene, &events)?;
    Ok(true)
}

pub fn set_last_transcript_state(
    root: &Path,
    world_id: &str,
    scene: u64,
    state: &TableState,
) -> DataResult<bool> {
    let mut events = read_transcript(root, world_id, scene)?;
    let Some(entry) = events.last_mut() else {
        return Ok(false);
    };
    entry.state = Some(state.clone());
    let mut buffer = String::new();
    for entry in &events {
        buffer.push_str(&serde_json::to_string(entry)?);
        buffer.push('\n');
    }
    fs::write(transcript_path(root, world_id, scene)?, buffer)?;
    Ok(true)
}

pub fn read_transcript(
    root: &Path,
    world_id: &str,
    scene: u64,
) -> DataResult<Vec<TranscriptEvent>> {
    let path = transcript_path(root, world_id, scene)?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = fs::File::open(path)?;
    let mut events = Vec::new();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line_number = index + 1;
        let line = line?;
        let event = serde_json::from_str(&line).map_err(|error| {
            invalid_data(format!("invalid transcript line {line_number}: {error}"))
        })?;
        events.push(event);
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::*;
    use crate::data::test_support::*;
    use std::collections::BTreeMap;

    #[test]
    fn transcript_round_trip_is_ordered_jsonl_and_rejects_invalid_kind() {
        let root = TestRoot::new("transcript");
        let world_id = create_world(root.path(), "劇場").unwrap();
        let events = vec![
            TranscriptEvent {
                raw: None,
                ts: "2026-07-19T10:00:00+08:00".to_owned(),
                speaker_id: String::new(),
                speaker_name: "旁白".to_owned(),
                kind: TranscriptKind::Narration,
                text: "序幕".to_owned(),
                state: None,
                gm_only: false,
            },
            TranscriptEvent {
                raw: None,
                ts: "2026-07-19T10:00:01+08:00".to_owned(),
                speaker_id: String::new(),
                speaker_name: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "第一行\n仍是同一事件".to_owned(),
                state: None,
                gm_only: false,
            },
            TranscriptEvent {
                raw: None,
                ts: "2026-07-19T10:00:02+08:00".to_owned(),
                speaker_id: "角色代碼".to_owned(),
                speaker_name: "角色".to_owned(),
                kind: TranscriptKind::Dialogue,
                text: "你好".to_owned(),
                state: None,
                gm_only: false,
            },
        ];
        for event in &events {
            append_transcript(root.path(), &world_id, 7, event).unwrap();
        }
        let expected: Vec<_> = events
            .iter()
            .cloned()
            .map(|mut event| {
                event.state = Some(TableState::default());
                event
            })
            .collect();
        assert_eq!(
            read_transcript(root.path(), &world_id, 7).unwrap(),
            expected
        );

        let path = root
            .path()
            .join(format!("worlds/{world_id}/transcript/7.jsonl"));
        let raw = fs::read_to_string(&path).unwrap();
        let lines: Vec<_> = raw.lines().collect();
        assert_eq!(lines.len(), 3);
        for line in lines {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(value.is_object());
            assert!(["dialogue", "narration", "player", "system"]
                .contains(&value["kind"].as_str().unwrap()));
        }

        OpenOptions::new()
            .append(true)
            .open(path)
            .unwrap()
            .write_all(b"{\"ts\":\"now\",\"speaker_id\":\"\",\"speaker_name\":\"x\",\"kind\":\"bad\",\"text\":\"x\"}\n")
            .unwrap();
        let error = read_transcript(root.path(), &world_id, 7)
            .unwrap_err()
            .to_string();
        assert!(error.contains("line 4"), "{error}");
    }

    #[test]
    fn pop_transcript_removes_last_event_until_scene_is_empty() {
        let root = TestRoot::new("transcript-pop");
        let world_id = create_world(root.path(), "收回桌").unwrap();
        let events: Vec<TranscriptEvent> = ["序幕", "我推開門", "誰在那裡？"]
            .iter()
            .enumerate()
            .map(|(index, text)| TranscriptEvent {
                raw: None,
                ts: format!("2026-08-01T10:00:0{index}+08:00"),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: TranscriptKind::Narration,
                text: (*text).to_owned(),
                state: None,
                gm_only: false,
            })
            .collect();
        for event in &events {
            append_transcript(root.path(), &world_id, 0, event).unwrap();
        }

        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        let expected: Vec<_> = events[..2]
            .iter()
            .cloned()
            .map(|mut event| {
                event.state = Some(TableState::default());
                event
            })
            .collect();
        assert_eq!(
            read_transcript(root.path(), &world_id, 0).unwrap(),
            expected
        );
        // 重寫後仍是合法 JSONL：行數對齊事件數，沒有殘留的半行
        let path = root
            .path()
            .join(format!("worlds/{world_id}/transcript/0.jsonl"));
        assert_eq!(fs::read_to_string(&path).unwrap().lines().count(), 2);

        // 連按到底：收乾淨後再按回 false，不會倒退咬到別的幕
        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert!(!pop_transcript(root.path(), &world_id, 0).unwrap());
        assert!(read_transcript(root.path(), &world_id, 0)
            .unwrap()
            .is_empty());

        // 沒開始過的幕：不建檔也不報錯
        assert!(!pop_transcript(root.path(), &world_id, 9).unwrap());
        assert!(!root
            .path()
            .join(format!("worlds/{world_id}/transcript/9.jsonl"))
            .exists());
    }

    #[test]
    fn append_transcript_uses_current_snapshot_without_overwriting_supplied_state() {
        let root = TestRoot::new("transcript-state-snapshot");
        let world_id = create_world(root.path(), "狀態桌").unwrap();
        let mut state = read_state(root.path(), &world_id).unwrap();
        state
            .state
            .table
            .insert("time".to_owned(), "清晨".to_owned());
        write_state(root.path(), &world_id, &state).unwrap();
        let event = TranscriptEvent {
            raw: None,
            ts: "now".to_owned(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: TranscriptKind::Narration,
            text: "第一句".to_owned(),
            state: None,
            gm_only: false,
        };
        append_transcript(root.path(), &world_id, 0, &event).unwrap();
        assert_eq!(
            read_transcript(root.path(), &world_id, 0).unwrap()[0]
                .state
                .as_ref()
                .unwrap()
                .table
                .get("time"),
            Some(&"清晨".to_owned())
        );

        let supplied = TableState {
            table: BTreeMap::from([("time".to_owned(), "午夜".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                raw: None,
                ts: "later".to_owned(),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: TranscriptKind::Narration,
                text: "第二句".to_owned(),
                state: Some(supplied.clone()),
                gm_only: false,
            },
        )
        .unwrap();
        assert_eq!(
            read_transcript(root.path(), &world_id, 0).unwrap()[1].state,
            Some(supplied)
        );
    }

    #[test]
    fn append_opening_skips_raw_when_nothing_was_stripped() {
        let root = TestRoot::new("opening-raw");
        let world_id = create_world(root.path(), "純正文桌").unwrap();
        let raw = "只有旁白，沒有狀態欄。";
        let (event, _) = append_opening(
            root.path(),
            &world_id,
            0,
            "opening",
            raw,
            &crate::transport::extract_state_block(raw),
            "阿濤",
        )
        .unwrap();
        assert_eq!(event.text, raw);
        assert_eq!(event.raw, None);
        // 舊檔沒有 raw 欄位也讀得起來，序列化時同樣不憑空多一欄
        let line = serde_json::to_string(&event).unwrap();
        assert!(!line.contains("\"raw\""));
    }

    #[test]
    fn append_opening_merges_state_and_pop_restores_previous_snapshot() {
        let root = TestRoot::new("opening-state");
        let world_id = create_world(root.path(), "開場狀態桌").unwrap();
        let previous = TableState {
            table: BTreeMap::from([("place".to_owned(), "酒館".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                raw: None,
                ts: "before".to_owned(),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: TranscriptKind::Narration,
                text: "前一則".to_owned(),
                state: Some(previous.clone()),
                gm_only: false,
            },
        )
        .unwrap();

        let raw = "開場旁白<status>place: 碼頭\ntime: 午夜</status>";
        let (event, outcome) = append_opening(
            root.path(),
            &world_id,
            0,
            "opening",
            raw,
            &crate::transport::extract_state_block(raw),
            "阿濤",
        )
        .unwrap();
        assert!(outcome.records.is_empty());
        // 畫面只留正文，模型原文整段另存一份（面板要靠它重畫歷史訊息）
        assert_eq!(event.text, "開場旁白");
        assert_eq!(event.raw.as_deref(), Some(raw));
        let expected = TableState {
            table: BTreeMap::from([
                ("place".to_owned(), "碼頭".to_owned()),
                ("time".to_owned(), "午夜".to_owned()),
            ]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        assert_eq!(event.state, Some(expected.clone()));
        assert_eq!(read_state(root.path(), &world_id).unwrap().state, expected);
        assert_eq!(
            read_transcript(root.path(), &world_id, 0).unwrap()[1],
            event
        );

        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert_eq!(read_state(root.path(), &world_id).unwrap().state, previous);
    }

    #[test]
    fn pop_transcript_restores_the_previous_event_snapshot() {
        let root = TestRoot::new("transcript-state-pop");
        let world_id = create_world(root.path(), "回收狀態桌").unwrap();
        let first = TableState {
            table: BTreeMap::from([("place".to_owned(), "酒館".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        let second = TableState {
            table: BTreeMap::from([("place".to_owned(), "碼頭".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        for (text, snapshot) in [("第一句", first.clone()), ("第二句", second.clone())] {
            append_transcript(
                root.path(),
                &world_id,
                0,
                &TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: TranscriptKind::Narration,
                    text: text.to_owned(),
                    state: Some(snapshot),
                    gm_only: false,
                },
            )
            .unwrap();
        }
        let mut state = read_state(root.path(), &world_id).unwrap();
        state.state = second;
        write_state(root.path(), &world_id, &state).unwrap();

        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert_eq!(read_state(root.path(), &world_id).unwrap().state, first);
    }

    /// 復原＝把帶著自身快照的舊事件原樣寫回，目前值要跟著回到那一刻
    /// （否則狀態欄會停在收回後的舊值，跟桌上最後一句對不起來）
    #[test]
    fn restoring_an_undone_event_puts_its_snapshot_back() {
        let root = TestRoot::new("transcript-state-restore");
        let world_id = create_world(root.path(), "復原狀態桌").unwrap();
        let snapshots = ["清晨", "午夜"].map(|time| TableState {
            table: BTreeMap::from([("time".to_owned(), time.to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        });
        let event = |text: &str, snapshot: &TableState| TranscriptEvent {
            raw: None,
            ts: "now".to_owned(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: TranscriptKind::Narration,
            text: text.to_owned(),
            state: Some(snapshot.clone()),
            gm_only: false,
        };
        for (text, snapshot) in [("第一句", &snapshots[0]), ("第二句", &snapshots[1])] {
            append_transcript(root.path(), &world_id, 0, &event(text, snapshot)).unwrap();
        }
        assert_eq!(
            read_state(root.path(), &world_id).unwrap().state,
            snapshots[1]
        );

        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert_eq!(
            read_state(root.path(), &world_id).unwrap().state,
            snapshots[0]
        );

        append_transcript(root.path(), &world_id, 0, &event("第二句", &snapshots[1])).unwrap();
        assert_eq!(
            read_state(root.path(), &world_id).unwrap().state,
            snapshots[1]
        );
    }

    #[test]
    fn pop_transcript_restores_entire_nested_tree_snapshot() {
        let root = TestRoot::new("nested-state-pop");
        let world_id = create_world(root.path(), "巢狀桌").unwrap();
        let first = TableState {
            table: BTreeMap::new(),
            tree: BTreeMap::from([(
                "World".to_owned(),
                StateNode::Branch(BTreeMap::from([(
                    "城市".to_owned(),
                    StateNode::Branch(BTreeMap::from([(
                        "聲望".to_owned(),
                        StateNode::Leaf("10".to_owned()),
                    )])),
                )])),
            )]),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        let second = TableState {
            table: BTreeMap::new(),
            tree: BTreeMap::from([(
                "World".to_owned(),
                StateNode::Branch(BTreeMap::from([(
                    "城市".to_owned(),
                    StateNode::Branch(BTreeMap::from([(
                        "聲望".to_owned(),
                        StateNode::Leaf("20".to_owned()),
                    )])),
                )])),
            )]),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        for snapshot in [first.clone(), second.clone()] {
            append_transcript(
                root.path(),
                &world_id,
                0,
                &TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: TranscriptKind::Narration,
                    text: "旁白".to_owned(),
                    state: Some(snapshot),
                    gm_only: false,
                },
            )
            .unwrap();
        }
        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert_eq!(read_state(root.path(), &world_id).unwrap().state, first);
    }

}
