use super::*;
use crate::data::test_support::*;
use crate::data::*;
use std::collections::BTreeMap;

/// 接管桌判定（換幕摘要兩入口與出場收尾共用）：非空骨架＋非 characters 才算
#[test]
fn interface_takeover_needs_nonblank_shell_and_non_character_mode() {
    let root = TestRoot::new("interface-takeover");
    let world_id = create_world(root.path(), "接管判定桌").unwrap();
    assert!(!is_interface_takeover(root.path(), &world_id, None));
    write_interface_shell(root.path(), &world_id, " \n").unwrap();
    assert!(!is_interface_takeover(root.path(), &world_id, None));
    write_interface_shell(root.path(), &world_id, "<UI>{{本回合.正文}}</UI>").unwrap();
    assert!(is_interface_takeover(root.path(), &world_id, None));
    assert!(is_interface_takeover(
        root.path(),
        &world_id,
        Some("interface")
    ));
    assert!(!is_interface_takeover(
        root.path(),
        &world_id,
        Some("characters")
    ));
}

#[test]
fn begin_next_scene_appends_summary_and_advances_scene() {
    let root = TestRoot::new("begin-next-scene");
    let world_id = create_world(root.path(), "換場桌").unwrap();
    let event = TranscriptEvent {
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
        raw: None,
        ts: "2026-07-19T00:00:00Z".to_owned(),
        speaker_id: String::new(),
        speaker_name: "玩家".to_owned(),
        kind: TranscriptKind::Player,
        text: "第一場的對話".to_owned(),
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
    };
    append_transcript(root.path(), &world_id, 0, &event).unwrap();

    let next = begin_next_scene(root.path(), &world_id, "壓縮後的摘要", None).unwrap();
    assert_eq!(next, 1);
    assert_eq!(read_state(root.path(), &world_id).unwrap().current_scene, 1);

    // 摘要落在新場景檔開頭，舊場景不受影響
    assert_eq!(read_transcript(root.path(), &world_id, 0).unwrap().len(), 1);
    let new_scene = read_transcript(root.path(), &world_id, 1).unwrap();
    assert_eq!(new_scene.len(), 1);
    assert_eq!(new_scene[0].speaker_name, "GM");
    assert_eq!(new_scene[0].speaker_id, "");
    assert_eq!(new_scene[0].kind, TranscriptKind::Narration);
    // 只存本文＋代碼，標頭在顯示／匯出／送 AI 時照語系組
    assert_eq!(new_scene[0].text, "壓縮後的摘要");
    assert_eq!(new_scene[0].marker, Some(EventMarker::SceneSummary));
}

#[test]
fn begin_next_scene_stores_title_on_old_scene_when_given() {
    let root = TestRoot::new("begin-next-scene-title");
    let world_id = create_world(root.path(), "取名桌").unwrap();
    let event = TranscriptEvent {
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
        raw: None,
        ts: "2026-07-24T00:00:00Z".to_owned(),
        speaker_id: String::new(),
        speaker_name: "玩家".to_owned(),
        kind: TranscriptKind::Player,
        text: "第一幕的對話".to_owned(),
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
    };
    append_transcript(root.path(), &world_id, 0, &event).unwrap();

    begin_next_scene(root.path(), &world_id, "摘要", Some("酒館夜話")).unwrap();
    let state = read_state(root.path(), &world_id).unwrap();
    assert_eq!(state.current_scene, 1);
    assert_eq!(
        state.scene_titles.get("0").map(String::as_str),
        Some("酒館夜話")
    );
    assert!(!state.scene_titles.contains_key("1"));

    // 空字串／None 都不進表
    begin_next_scene(root.path(), &world_id, "摘要二", Some("   ")).unwrap();
    begin_next_scene(root.path(), &world_id, "摘要三", None).unwrap();
    let state = read_state(root.path(), &world_id).unwrap();
    assert!(!state.scene_titles.contains_key("1"));
    assert!(!state.scene_titles.contains_key("2"));
}

#[test]
fn revert_scene_returns_to_previous_scene_and_drops_title() {
    let root = TestRoot::new("revert-scene");
    let world_id = create_world(root.path(), "退幕桌").unwrap();
    let snapshot = TableState {
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
            ts: "2026-08-06T00:00:00Z".to_owned(),
            speaker_id: String::new(),
            speaker_name: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "第一幕的對話".to_owned(),
            state: Some(snapshot.clone()),
            truncated: false,
            gm_only: false,
            marker: None,
            opening: false,
        },
    )
    .unwrap();
    begin_next_scene(root.path(), &world_id, "摘要", Some("酒館夜話")).unwrap();
    assert_eq!(read_state(root.path(), &world_id).unwrap().current_scene, 1);

    let previous = revert_scene(root.path(), &world_id).unwrap();
    assert_eq!(previous, 0);

    let state = read_state(root.path(), &world_id).unwrap();
    assert_eq!(state.current_scene, 0);
    // 前幕最後一則帶快照事件的 state 要跟著回來，不是砍完就放著預設值
    assert_eq!(state.state, snapshot);
    assert!(!state.scene_titles.contains_key("0"));
    assert!(!root
        .path()
        .join(format!("worlds/{world_id}/transcript/1.jsonl"))
        .exists());
    // 舊幕本身完全沒被動過
    assert_eq!(read_transcript(root.path(), &world_id, 0).unwrap().len(), 1);
}

#[test]
fn revert_scene_rejects_extra_events_without_touching_anything() {
    let root = TestRoot::new("revert-scene-blocked");
    let world_id = create_world(root.path(), "退幕擋桌").unwrap();
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
            ts: "2026-08-06T00:00:00Z".to_owned(),
            speaker_id: String::new(),
            speaker_name: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "序幕".to_owned(),
            state: None,
            truncated: false,
            gm_only: false,
            marker: None,
            opening: false,
        },
    )
    .unwrap();
    begin_next_scene(root.path(), &world_id, "摘要", None).unwrap();
    // 這一幕除了摘要之外，玩家已經多說了一句——不是「剛好一則」了
    append_transcript(
        root.path(),
        &world_id,
        1,
        &TranscriptEvent {
            id: None,
            message_vars: None,
            vars_rev: None,
            vars_epoch: None,
            turn_key: None,
            action_id: None,
            raw: None,
            ts: "2026-08-06T00:01:00Z".to_owned(),
            speaker_id: String::new(),
            speaker_name: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "新的一句".to_owned(),
            state: None,
            truncated: false,
            gm_only: false,
            marker: None,
            opening: false,
        },
    )
    .unwrap();

    let before_state = read_state(root.path(), &world_id).unwrap();
    let before_events = read_transcript(root.path(), &world_id, 1).unwrap();

    let error = revert_scene(root.path(), &world_id)
        .unwrap_err()
        .to_string();
    assert!(error == UiMsg::SceneRewindHasNewContent.to_string());

    // 擋下時檔案與 state 都沒被動過
    assert_eq!(read_state(root.path(), &world_id).unwrap(), before_state);
    assert_eq!(
        read_transcript(root.path(), &world_id, 1).unwrap(),
        before_events
    );
}

#[test]
fn revert_scene_rejects_at_first_scene() {
    let root = TestRoot::new("revert-scene-first");
    let world_id = create_world(root.path(), "第一幕桌").unwrap();
    let error = revert_scene(root.path(), &world_id)
        .unwrap_err()
        .to_string();
    assert!(error == UiMsg::SceneFirstNoPrevious.to_string());
}

#[test]
fn replace_scene_summary_overwrites_text_and_drops_title_when_none() {
    let root = TestRoot::new("replace-scene-summary");
    let world_id = create_world(root.path(), "重寫摘要桌").unwrap();
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
            ts: "2026-08-06T00:00:00Z".to_owned(),
            speaker_id: String::new(),
            speaker_name: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "序幕".to_owned(),
            state: None,
            truncated: false,
            gm_only: false,
            marker: None,
            opening: false,
        },
    )
    .unwrap();
    begin_next_scene(root.path(), &world_id, "舊摘要", Some("舊標題")).unwrap();
    assert_eq!(
        read_state(root.path(), &world_id)
            .unwrap()
            .scene_titles
            .get("0")
            .map(String::as_str),
        Some("舊標題")
    );

    replace_scene_summary(root.path(), &world_id, "新摘要", None).unwrap();

    let events = read_transcript(root.path(), &world_id, 1).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].text, "新摘要");
    assert_eq!(events[0].marker, Some(EventMarker::SceneSummary));
    assert_eq!(events[0].speaker_name, "GM");
    assert_eq!(events[0].kind, TranscriptKind::Narration);

    // title 傳 None：舊幕名被移除，不留上一次的殘留
    assert!(!read_state(root.path(), &world_id)
        .unwrap()
        .scene_titles
        .contains_key("0"));
}

/// 分岔幕開頭那則是複製來的真實對話，不是前情提要。源頭幕剛好只有一則時，
/// 「這幕只有一則」那道守門會放行——沒有 forked 這一格擋著，重寫就把玩家的對話覆寫掉了。
#[test]
fn replace_scene_summary_refuses_a_forked_scene() {
    let root = TestRoot::new("replace-summary-forked");
    let world_id = create_world(root.path(), "分岔守門桌").unwrap();
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
            speaker_name: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "玩家的第一句".to_owned(),
            state: None,
            truncated: false,
            gm_only: false,
            marker: None,
            opening: false,
        },
    )
    .unwrap();
    begin_next_scene(root.path(), &world_id, "第一幕摘要", None).unwrap();

    // 幕 0 只有一則，分岔出來的這一幕同樣只有那一則——正是守門會誤放的形狀
    let forked = fork_scene(root.path(), &world_id, 0).unwrap();
    assert_eq!(
        read_transcript(root.path(), &world_id, forked)
            .unwrap()
            .len(),
        1
    );

    assert!(replace_scene_summary(root.path(), &world_id, "不該蓋掉", None).is_err());
    let events = read_transcript(root.path(), &world_id, forked).unwrap();
    assert_eq!(events[0].text, "玩家的第一句");
}

/// 重寫摘要只換文字：那則的狀態快照要留著。摘要是這一幕唯一一則，
/// 快照掉了的話，之後退回這一幕就只能把狀態欄清成空的。
#[test]
fn replace_scene_summary_keeps_snapshot_for_later_revert() {
    let root = TestRoot::new("replace-scene-summary-snapshot");
    let world_id = create_world(root.path(), "快照保留桌").unwrap();
    let snapshot = TableState {
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
            ts: "now".to_owned(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: TranscriptKind::Narration,
            text: "序幕".to_owned(),
            state: Some(snapshot.clone()),
            truncated: false,
            gm_only: false,
            marker: None,
            opening: false,
        },
    )
    .unwrap();
    let mut state = read_state(root.path(), &world_id).unwrap();
    state.state = snapshot.clone();
    write_state(root.path(), &world_id, &state).unwrap();

    begin_next_scene(root.path(), &world_id, "舊摘要", None).unwrap();
    replace_scene_summary(root.path(), &world_id, "新摘要", None).unwrap();
    // 再換一幕：這時第 1 幕那則摘要成了回推狀態的唯一來源
    begin_next_scene(root.path(), &world_id, "第二幕摘要", None).unwrap();

    assert_eq!(revert_scene(root.path(), &world_id).unwrap(), 1);
    assert_eq!(read_state(root.path(), &world_id).unwrap().state, snapshot);
}

/// 驗收劇本：原線三幕分岔、續玩換幕、退回吃 parent、再分岔看 version 疊加。
#[test]
fn fork_scene_copies_history_and_relabels_through_continue_and_revert() {
    let root = TestRoot::new("fork-scene-scenario");
    let world_id = create_world(root.path(), "分岔桌").unwrap();

    // 原線三幕（內部 0、1、2），人在幕 2
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
            ts: "2026-08-06T00:00:00Z".to_owned(),
            speaker_id: "船長代碼".to_owned(),
            speaker_name: "船長".to_owned(),
            kind: TranscriptKind::Dialogue,
            text: "啟航前的最後一夜。".to_owned(),
            state: None,
            truncated: false,
            gm_only: false,
            marker: None,
            opening: false,
        },
    )
    .unwrap();
    begin_next_scene(root.path(), &world_id, "第一幕摘要", None).unwrap();
    begin_next_scene(root.path(), &world_id, "第二幕摘要", None).unwrap();
    assert_eq!(read_state(root.path(), &world_id).unwrap().current_scene, 2);

    let scene0_before = read_transcript(root.path(), &world_id, 0).unwrap();
    let scene1_before = read_transcript(root.path(), &world_id, 1).unwrap();
    let scene2_before = read_transcript(root.path(), &world_id, 2).unwrap();

    // 從幕 0 分岔
    let forked = fork_scene(root.path(), &world_id, 0).unwrap();
    assert_eq!(forked, 3);
    // 分岔重新配發事件 id（計畫 8.4），其餘逐欄相同
    let copied = read_transcript(root.path(), &world_id, 3).unwrap();
    assert!(copied
        .iter()
        .zip(&scene0_before)
        .all(|(copy, original)| copy.id.is_some() && copy.id != original.id));
    let without_ids = |events: Vec<TranscriptEvent>| {
        events
            .into_iter()
            .map(|mut event| {
                event.id = None;
                event
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(without_ids(copied), without_ids(scene0_before.clone()));
    // 舊幕一個字都沒被動過
    assert_eq!(
        read_transcript(root.path(), &world_id, 0).unwrap(),
        scene0_before
    );
    assert_eq!(
        read_transcript(root.path(), &world_id, 1).unwrap(),
        scene1_before
    );
    assert_eq!(
        read_transcript(root.path(), &world_id, 2).unwrap(),
        scene2_before
    );

    let state = read_state(root.path(), &world_id).unwrap();
    assert_eq!(state.current_scene, 3);
    assert_eq!(
        state.scene_labels.get("3").copied(),
        Some(SceneLabel {
            base: 0,
            version: 2,
            parent: Some(2),
            forked: true
        })
    );

    // 在幕 3 續玩一句，再換幕
    append_transcript(
        root.path(),
        &world_id,
        3,
        &TranscriptEvent {
            id: None,
            message_vars: None,
            vars_rev: None,
            vars_epoch: None,
            turn_key: None,
            action_id: None,
            raw: None,
            ts: "2026-08-06T00:01:00Z".to_owned(),
            speaker_id: "船長代碼".to_owned(),
            speaker_name: "船長".to_owned(),
            kind: TranscriptKind::Dialogue,
            text: "這次我們往南走。".to_owned(),
            state: None,
            truncated: false,
            gm_only: false,
            marker: None,
            opening: false,
        },
    )
    .unwrap();
    let advanced =
        begin_next_scene(root.path(), &world_id, "分岔後摘要", Some("南航夜話")).unwrap();
    assert_eq!(advanced, 4);

    let state = read_state(root.path(), &world_id).unwrap();
    assert_eq!(
        state.scene_labels.get("4").copied(),
        Some(SceneLabel {
            base: 1,
            version: 2,
            parent: Some(3),
            forked: false
        })
    );
    assert_eq!(
        state.scene_titles.get("3").map(String::as_str),
        Some("南航夜話")
    );

    // 退回幕 4：回到 parent（3），不是算術上的 4-1=3 巧合——這裡故意驗證的是「回到分岔前所在幕」
    let reverted = revert_scene(root.path(), &world_id).unwrap();
    assert_eq!(reverted, 3);
    assert!(!root
        .path()
        .join(format!("worlds/{world_id}/transcript/4.jsonl"))
        .exists());
    let state = read_state(root.path(), &world_id).unwrap();
    assert_eq!(state.current_scene, 3);
    assert!(!state.scene_titles.contains_key("3"));
    assert!(!state.scene_labels.contains_key("4"));

    // 再從幕 0 分岔一次：幕 0 與幕 3 都是 base 0，這次該排第 3 個版本
    let forked_again = fork_scene(root.path(), &world_id, 0).unwrap();
    assert_eq!(forked_again, 4);
    let state = read_state(root.path(), &world_id).unwrap();
    assert_eq!(
        state.scene_labels.get("4").copied(),
        Some(SceneLabel {
            base: 0,
            version: 3,
            parent: Some(3),
            forked: true
        })
    );
}

#[test]
fn fork_scene_rejects_current_or_future_scene() {
    let root = TestRoot::new("fork-scene-rejects-current");
    let world_id = create_world(root.path(), "分岔擋桌").unwrap();
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
            ts: "2026-08-06T00:00:00Z".to_owned(),
            speaker_id: String::new(),
            speaker_name: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "序幕".to_owned(),
            state: None,
            truncated: false,
            gm_only: false,
            marker: None,
            opening: false,
        },
    )
    .unwrap();

    // from_scene == current_scene：還沒換幕，不能從自己這幕分岔
    let error = fork_scene(root.path(), &world_id, 0)
        .unwrap_err()
        .to_string();
    assert!(error == UiMsg::SceneForkNotEarlier.to_string());

    // from_scene > current_scene：幕號還沒出現過
    let error = fork_scene(root.path(), &world_id, 5)
        .unwrap_err()
        .to_string();
    assert!(error == UiMsg::SceneForkNotEarlier.to_string());
}

#[test]
fn fork_scene_rejects_a_scene_with_no_events() {
    let root = TestRoot::new("fork-scene-rejects-empty");
    let world_id = create_world(root.path(), "分岔空幕桌").unwrap();
    let mut state = read_state(root.path(), &world_id).unwrap();
    state.current_scene = 1; // 幕 0 從沒寫過任何事件，模擬空幕
    write_state(root.path(), &world_id, &state).unwrap();

    let error = fork_scene(root.path(), &world_id, 0)
        .unwrap_err()
        .to_string();
    assert!(error == UiMsg::SceneNothingToContinue.to_string());
}

#[test]
fn scene_label_falls_back_to_original_line_for_unlabeled_scene() {
    let root = TestRoot::new("scene-label-fallback");
    let world_id = create_world(root.path(), "原線桌").unwrap();
    let state = read_state(root.path(), &world_id).unwrap();

    assert_eq!(
        scene_label(&state, 5),
        SceneLabel {
            base: 5,
            version: 1,
            parent: Some(4),
            forked: false
        }
    );
    // 幕 0 沒有前幕：fallback 的 parent 也要是 None，跟 revert_scene 的邊界檢查對得起來
    assert_eq!(
        scene_label(&state, 0),
        SceneLabel {
            base: 0,
            version: 1,
            parent: None,
            forked: false
        }
    );
}
