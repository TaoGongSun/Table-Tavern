use super::*;
use crate::data::test_support::*;
use crate::data::*;
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;

#[test]
fn transcript_round_trip_is_ordered_jsonl_and_rejects_invalid_kind() {
    let root = TestRoot::new("transcript");
    let world_id = create_world(root.path(), "劇場").unwrap();
    let events = vec![
        TranscriptEvent {
            id: None,
            message_vars: None,
            vars_rev: None,
            vars_epoch: None,
            turn_key: None,
            action_id: None,
            raw: None,
            ts: "2026-07-19T10:00:00+08:00".to_owned(),
            speaker_id: String::new(),
            speaker_name: "旁白".to_owned(),
            kind: TranscriptKind::Narration,
            text: "序幕".to_owned(),
            state: None,
            truncated: false,
            gm_only: false,
            marker: None,
            opening: false,
        },
        TranscriptEvent {
            id: None,
            message_vars: None,
            vars_rev: None,
            vars_epoch: None,
            turn_key: None,
            action_id: None,
            raw: None,
            ts: "2026-07-19T10:00:01+08:00".to_owned(),
            speaker_id: String::new(),
            speaker_name: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "第一行\n仍是同一事件".to_owned(),
            state: None,
            truncated: false,
            gm_only: false,
            marker: None,
            opening: false,
        },
        TranscriptEvent {
            id: None,
            message_vars: None,
            vars_rev: None,
            vars_epoch: None,
            turn_key: None,
            action_id: None,
            raw: None,
            ts: "2026-07-19T10:00:02+08:00".to_owned(),
            speaker_id: "角色代碼".to_owned(),
            speaker_name: "角色".to_owned(),
            kind: TranscriptKind::Dialogue,
            text: "你好".to_owned(),
            state: None,
            truncated: false,
            gm_only: false,
            marker: None,
            opening: false,
        },
    ];
    for event in &events {
        append_transcript(root.path(), &world_id, 7, event).unwrap();
    }
    let read = read_transcript(root.path(), &world_id, 7).unwrap();
    // 每則落檔時都配發了各自的穩定 ID
    let ids: std::collections::BTreeSet<_> = read.iter().map(|event| event.id.clone()).collect();
    assert_eq!(ids.len(), 3);
    assert!(read
        .iter()
        .all(|event| event.id.as_ref().is_some_and(|id| id.len() == 26)));
    let expected: Vec<_> = events
        .iter()
        .cloned()
        .zip(&read)
        .map(|(mut event, stored)| {
            event.state = Some(TableState::default());
            event.id = stored.id.clone();
            event
        })
        .collect();
    assert_eq!(read, expected);

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
            id: None,
            message_vars: None,
            vars_rev: None,
            vars_epoch: None,
            turn_key: None,
            action_id: None,
            raw: None,
            ts: format!("2026-08-01T10:00:0{index}+08:00"),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: TranscriptKind::Narration,
            text: (*text).to_owned(),
            state: None,
            truncated: false,
            gm_only: false,
            marker: None,
            opening: false,
        })
        .collect();
    for event in &events {
        append_transcript(root.path(), &world_id, 0, event).unwrap();
    }

    assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
    let read = read_transcript(root.path(), &world_id, 0).unwrap();
    let expected: Vec<_> = events[..2]
        .iter()
        .cloned()
        .zip(&read)
        .map(|(mut event, stored)| {
            event.state = Some(TableState::default());
            event.id = stored.id.clone();
            event
        })
        .collect();
    assert_eq!(read, expected);
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
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
        raw: None,
        ts: "now".to_owned(),
        speaker_id: String::new(),
        speaker_name: "GM".to_owned(),
        kind: TranscriptKind::Narration,
        text: "第一句".to_owned(),
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
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
            id: None,
            message_vars: None,
            vars_rev: None,
            vars_epoch: None,
            turn_key: None,
            action_id: None,
            raw: None,
            ts: "later".to_owned(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: TranscriptKind::Narration,
            text: "第二句".to_owned(),
            state: Some(supplied.clone()),
            truncated: false,
            gm_only: false,
            marker: None,
            opening: false,
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
    // 開場標記落檔（MVU 墊片靠它分辨開場）；一般事件與舊檔不帶這欄
    assert!(event.opening);
    assert!(line.contains("\"opening\":true"));
    let old: TranscriptEvent = serde_json::from_str(
        r#"{"ts":"t","speaker_id":"","speaker_name":"GM","kind":"narration","text":"x"}"#,
    )
    .unwrap();
    assert!(!old.opening);
    assert!(!serde_json::to_string(&old).unwrap().contains("opening"));
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
            id: None,
            message_vars: None,
            vars_rev: None,
            vars_epoch: None,
            turn_key: None,
            action_id: None,
            raw: None,
            ts: "before".to_owned(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: TranscriptKind::Narration,
            text: "前一則".to_owned(),
            state: Some(previous.clone()),
            truncated: false,
            gm_only: false,
            marker: None,
            opening: false,
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

/// 非接管桌的開場白也照常套用 `<UpdateVariable>`：寫錯的路徑在第 1 回合就回饋給 GM。
#[test]
fn opening_patch_errors_on_a_plain_table_feed_back_on_turn_one() {
    let root = TestRoot::new("transcript-opening-feedback");
    let world_id = create_world(root.path(), "開場回饋桌").unwrap();
    let raw = "開場旁白<UpdateVariable><JSONPatch>[{\"op\":\"replace\",\"path\":\"/角色/生命\",\"value\":1}]</JSONPatch></UpdateVariable>";
    let (_, outcome) = append_opening(
        root.path(),
        &world_id,
        0,
        "opening",
        raw,
        &crate::transport::extract_state_block(raw),
        "阿濤",
    )
    .unwrap();
    let expected = vec!["路徑不存在：/角色。".to_owned()];
    assert_eq!(outcome.notes, expected);
    assert_eq!(
        read_state(root.path(), &world_id).unwrap().state.notes,
        expected
    );
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
                id: None,
                message_vars: None,
                vars_rev: None,
                vars_epoch: None,
                turn_key: None,
                action_id: None,
                raw: None,
                ts: "now".to_owned(),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: TranscriptKind::Narration,
                text: text.to_owned(),
                state: Some(snapshot),
                truncated: false,
                gm_only: false,
                marker: None,
                opening: false,
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
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
        raw: None,
        ts: "now".to_owned(),
        speaker_id: String::new(),
        speaker_name: "GM".to_owned(),
        kind: TranscriptKind::Narration,
        text: text.to_owned(),
        state: Some(snapshot.clone()),
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
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
                id: None,
                message_vars: None,
                vars_rev: None,
                vars_epoch: None,
                turn_key: None,
                action_id: None,
                raw: None,
                ts: "now".to_owned(),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: TranscriptKind::Narration,
                text: "旁白".to_owned(),
                state: Some(snapshot),
                truncated: false,
                gm_only: false,
                marker: None,
                opening: false,
            },
        )
        .unwrap();
    }
    assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
    assert_eq!(read_state(root.path(), &world_id).unwrap().state, first);
}
