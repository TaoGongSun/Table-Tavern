//! 重構只在未遊玩的桌做〔作者裁決 2026-10-11〕：已遊玩判定（開場白不算、待落的 GM 回覆算、網頁存檔匯入的桌
//! 一律算〔作者裁決 2026-10-12〕）、`rerun_status` 先看已遊玩、`apply_to_table` 擋下時什麼都不寫也不換世代。
use super::super::reset::{REFACTOR_EMPTY_SELECTION, REFACTOR_PLAYED};
use super::super::test_support::*;
use super::super::*;
use crate::data::{self, StateNode};
use crate::import;
use std::path::Path;

const CARD: &str = r#"{"spec":"chara_card_v3","data":{"name":"驛站","first_mes":"十月初七。","alternate_greetings":["另一則"],"character_book":{"name":"驛站","entries":[{"keys":[],"constant":true,"content":"每回合輸出 <Status_block> YAML","comment":"美化状态栏","enabled":true}]}}}"#;

fn imported(root: &TestRoot) -> (String, Option<String>) {
    let world_id = data::create_world(root.path(), "北境驛站").unwrap();
    let source = import::import_worldbook_file(
        root.path(),
        &world_id,
        CARD.as_bytes(),
        "驛站",
        &data::test_exclusive(&world_id),
    )
    .unwrap()
    .source;
    (world_id, source)
}

fn post_opening(root: &Path, world_id: &str, ts: &str, index: usize, source: Option<&str>) {
    import::post_opening_text(
        root,
        world_id,
        0,
        ts,
        "開場白",
        "zh-TW",
        Some(index),
        source,
        &data::test_exclusive(world_id),
    )
    .unwrap();
}

fn event(kind: data::TranscriptKind, opening: bool) -> data::TranscriptEvent {
    data::TranscriptEvent {
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
        ts: "2026-10-11T20:00:00".to_owned(),
        speaker_id: String::new(),
        speaker_name: "玩家".to_owned(),
        kind,
        text: "我進門".to_owned(),
        raw: None,
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening,
    }
}

fn outcome() -> RefactorOutcome {
    RefactorOutcome {
        mode: Some("interface".to_owned()),
        characters: Vec::new(),
        interface: Some(RefactorInterface {
            state_fields: serde_json::json!({ "地點": "前院" }),
            source_uids: Vec::new(),
            raw: String::new(),
            shell: Some("<Status_block>\n地点: {{地點}}\n</Status_block>".to_owned()),
            rules: Default::default(),
            guide: String::new(),
        }),
        mechanisms: Vec::new(),
        entries: Vec::new(),
        deletable_shared_uids: Vec::new(),
        dropped: Vec::new(),
        unabsorbed: Vec::new(),
        audit: Vec::new(),
        preserve_source_uids: Vec::new(),
        source_fingerprints: Default::default(),
        mirrors: Default::default(),
        stage_cards: Vec::new(),
    }
}

fn selection() -> RefactorSelection {
    RefactorSelection {
        apply_interface: true,
        ..no_player_selection(Vec::new())
    }
}

fn apply_here(root: &Path, world_id: &str) -> Result<RefactorApplySummary, String> {
    apply_to_table(
        root,
        world_id,
        "zh-TW",
        &outcome(),
        &selection(),
        &[],
        crate::refactor::RefactorApplyOrigin::Card,
        &data::test_exclusive(world_id),
    )
}

/// 整桌目錄的內容指紋：擋下時一個位元都不能動
fn fingerprint(root: &Path, world_id: &str) -> Vec<(String, Vec<u8>)> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        for path in entries {
            let name = path.strip_prefix(base).unwrap().display().to_string();
            if path.is_dir() {
                out.push((name, Vec::new()));
                walk(&path, base, out);
            } else {
                out.push((name, std::fs::read(&path).unwrap()));
            }
        }
    }
    let dir = root.join("worlds").join(world_id);
    let mut out = Vec::new();
    walk(&dir, &dir, &mut out);
    out
}

fn generation(root: &Path, world_id: &str) -> u64 {
    data::state_commit::with_commit(root, world_id, data::message_vars::generation)
}

fn unlanded(root: &Path, world_id: &str) -> bool {
    data::state_commit::with_commit(root, world_id, data::message_vars::has_unlanded_reply)
}

/// 擋下：回 refactor-played，整桌、世代與回合紀錄都不動；重設一樣被擋
fn assert_refused(root: &Path, world_id: &str) {
    assert_eq!(rerun_status(root, world_id).unwrap(), RerunStatus::Played);
    let before = fingerprint(root, world_id);
    let generation_before = generation(root, world_id);
    let unlanded_before = unlanded(root, world_id);
    assert_eq!(apply_here(root, world_id).unwrap_err(), REFACTOR_PLAYED);
    assert!(reset_to_import_source(root, world_id, "zh-TW").is_err());
    assert_eq!(fingerprint(root, world_id), before);
    assert_eq!(generation(root, world_id), generation_before);
    assert_eq!(unlanded(root, world_id), unlanded_before);
}

#[test]
fn openings_alone_are_not_play() {
    let root = TestRoot::new("played-openings");
    let (world_id, source) = imported(&root);
    assert_eq!(
        rerun_status(root.path(), &world_id).unwrap(),
        RerunStatus::Fresh
    );
    post_opening(
        root.path(),
        &world_id,
        "2026-10-11T19:00:00",
        0,
        source.as_deref(),
    );
    assert_eq!(
        rerun_status(root.path(), &world_id).unwrap(),
        RerunStatus::Fresh
    );
    post_opening(
        root.path(),
        &world_id,
        "2026-10-11T19:01:00",
        1,
        source.as_deref(),
    );
    assert_eq!(
        rerun_status(root.path(), &world_id).unwrap(),
        RerunStatus::Fresh
    );
    // 兩則開場白仍可重構，重構過之後也能清回重跑
    let generation_before = generation(root.path(), &world_id);
    apply_here(root.path(), &world_id).unwrap();
    assert_eq!(generation(root.path(), &world_id), generation_before + 1);
    assert_eq!(
        rerun_status(root.path(), &world_id).unwrap(),
        RerunStatus::Ready
    );
    reset_to_import_source(root.path(), &world_id, "zh-TW").unwrap();
    assert_eq!(
        data::read_transcript(root.path(), &world_id, 0)
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn any_non_opening_event_is_play() {
    let root = TestRoot::new("played-single-message");
    let (world_id, _) = imported(&root);
    data::append_transcript(
        root.path(),
        &world_id,
        0,
        &event(data::TranscriptKind::Player, false),
    )
    .unwrap();
    assert_refused(root.path(), &world_id);

    // 重構過的桌一樣
    let (refactored, source) = imported(&root);
    post_opening(
        root.path(),
        &refactored,
        "2026-10-11T19:00:00",
        0,
        source.as_deref(),
    );
    apply_here(root.path(), &refactored).unwrap();
    data::append_transcript(
        root.path(),
        &refactored,
        0,
        &event(data::TranscriptKind::Narration, false),
    )
    .unwrap();
    assert_refused(root.path(), &refactored);
}

#[test]
fn later_scene_is_play() {
    let root = TestRoot::new("played-scene");
    let (world_id, _) = imported(&root);
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    state.current_scene = 1;
    data::write_state(root.path(), &world_id, &state).unwrap();
    assert_refused(root.path(), &world_id);
}

/// GM 已提交、正文還沒落檔（前端沒落成）：那一輪遲早會落，算已遊玩；擋下時回合紀錄照留
#[test]
fn committed_but_unlanded_reply_is_play() {
    let root = TestRoot::new("played-unlanded");
    let (world_id, source) = imported(&root);
    post_opening(
        root.path(),
        &world_id,
        "2026-10-11T19:00:00",
        0,
        source.as_deref(),
    );
    data::state_commit::with_commit(root.path(), &world_id, |tx| {
        data::message_vars::begin_turn(tx, "t1", None).unwrap();
        data::message_vars::apply_gm_block(
            tx,
            "t1",
            data::message_vars::PendingMain {
                text: "GM 旁白".to_owned(),
                raw: None,
                truncated: false,
            },
            |_, _| ((), None),
            |_, _| Vec::new(),
        )
    })
    .unwrap()
    .expect("提交成功");
    assert!(unlanded(root.path(), &world_id));
    assert_refused(root.path(), &world_id);
    assert!(unlanded(root.path(), &world_id), "被擋不清回合紀錄");
}

#[test]
fn aborted_half_reply_is_play() {
    let root = TestRoot::new("played-aborted");
    let (world_id, source) = imported(&root);
    post_opening(
        root.path(),
        &world_id,
        "2026-10-11T19:00:00",
        0,
        source.as_deref(),
    );
    data::state_commit::with_commit(root.path(), &world_id, |tx| {
        data::message_vars::begin_turn(tx, "t1", None).unwrap();
        data::message_vars::finish_turn(
            tx,
            "t1",
            Some(data::message_vars::PendingMain {
                text: "半截".to_owned(),
                raw: None,
                truncated: true,
            }),
        )
    })
    .unwrap();
    assert_refused(root.path(), &world_id);
}

#[test]
fn generating_turn_is_play() {
    let root = TestRoot::new("played-generating");
    let (world_id, source) = imported(&root);
    post_opening(
        root.path(),
        &world_id,
        "2026-10-11T19:00:00",
        0,
        source.as_deref(),
    );
    data::state_commit::with_commit(root.path(), &world_id, |tx| {
        data::message_vars::begin_turn(tx, "t1", None)
    })
    .unwrap();
    assert_eq!(
        rerun_status(root.path(), &world_id).unwrap(),
        RerunStatus::Played
    );
    assert_eq!(
        apply_here(root.path(), &world_id).unwrap_err(),
        REFACTOR_PLAYED
    );
}

/// 網頁存檔匯入的桌天生有遊玩紀錄：只有開場白也擋〔作者裁決 2026-10-12〕
#[test]
fn web_save_tables_are_play() {
    let root = TestRoot::new("played-web-save");
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/shared/contracts/web-save/short.json");
    let mut save: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    save["messages"] = serde_json::json!([{
        "id": "m-open", "role": "char", "text": "開場", "ts": "2026-10-07T21:00:00+08:00", "opening": true
    }]);
    save["world_info"]["last_message_id"] = serde_json::Value::Null;
    let imported =
        import::import_web_save(root.path(), &serde_json::to_vec(&save).unwrap(), "zh-TW").unwrap();
    import::confirm_web_save_import(root.path(), &imported.world_id).unwrap();
    let events = data::read_transcript(root.path(), &imported.world_id, 0).unwrap();
    assert!(events.iter().all(|event| event.opening), "只有開場白");
    assert_refused(root.path(), &imported.world_id);
}

#[test]
fn unplayed_table_applies_normally() {
    let root = TestRoot::new("played-unplayed-apply");
    let (world_id, source) = imported(&root);
    post_opening(
        root.path(),
        &world_id,
        "2026-10-11T19:00:00",
        0,
        source.as_deref(),
    );
    apply_here(root.path(), &world_id).unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(
        state.state.tree.get("地點"),
        Some(&StateNode::Leaf("前院".to_owned()))
    );
}

/// 一項都沒勾：後端也拒收，整桌、世代都不動〔作者裁決 2026-10-27〕
#[test]
fn empty_selection_is_refused_without_touching_the_table() {
    let root = TestRoot::new("played-empty-selection");
    let (world_id, _) = imported(&root);
    let before = fingerprint(root.path(), &world_id);
    let generation_before = generation(root.path(), &world_id);
    let error = apply_to_table(
        root.path(),
        &world_id,
        "zh-TW",
        &outcome(),
        &no_player_selection(Vec::new()),
        &[],
        crate::refactor::RefactorApplyOrigin::Card,
        &data::test_exclusive(&world_id),
    )
    .unwrap_err();
    assert_eq!(error, REFACTOR_EMPTY_SELECTION);
    assert_eq!(fingerprint(root.path(), &world_id), before);
    assert_eq!(generation(root.path(), &world_id), generation_before);
}
