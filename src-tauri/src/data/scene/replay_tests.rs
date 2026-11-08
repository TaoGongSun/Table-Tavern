use super::*;
use crate::data::test_support::*;
use crate::data::*;

fn line(root: &Path, world_id: &str, scene: u64, text: &str) {
    let event = TranscriptEvent {
        dice_pool: None,
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
        raw: None,
        ts: "2026-11-02T00:00:00Z".to_owned(),
        speaker_id: String::new(),
        speaker_name: "GM".to_owned(),
        kind: TranscriptKind::Narration,
        text: text.to_owned(),
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
    };
    append_transcript(root, world_id, scene, &event).unwrap();
}

fn chapter_now(root: &Path, world_id: &str) -> u32 {
    vn::chapter_in(&read_state(root, world_id).unwrap().state.tree)
}

fn source_of(root: &Path, world_id: &str, scene: u64) -> Option<u64> {
    scene_label(&read_state(root, world_id).unwrap(), scene).source
}

#[test]
fn replay_walks_the_story_chain_to_the_earliest_head_of_the_chapter() {
    let root = TestRoot::new("vn-replay");
    let (root, world_id) = (
        root.path(),
        create_world_of_kind(root.path(), "重玩桌", TableKind::Vn).unwrap(),
    );
    line(root, &world_id, 0, "開場。");
    begin_next_scene_as(root, &world_id, "第一章摘要", None, true).unwrap(); // 1：第 2 章
    line(root, &world_id, 1, "第二章。");
    begin_next_scene_as(root, &world_id, "容量換幕", None, false).unwrap(); // 2：仍第 2 章
    line(root, &world_id, 2, "第二章後半。");

    // 同章兩幕取前者；只留幕首那則
    assert_eq!(replay_chapter(root, &world_id, 2).unwrap(), 3);
    assert_eq!(source_of(root, &world_id, 3), Some(1));
    let head = read_transcript(root, &world_id, 3).unwrap();
    assert_eq!(head.len(), 1);
    assert_eq!(head[0].text, "第一章摘要");
    assert_eq!(chapter_now(root, &world_id), 2);

    // 重玩第 2 章後再重玩第 1 章：回到第 0 幕開場
    assert_eq!(replay_chapter(root, &world_id, 1).unwrap(), 4);
    assert_eq!(source_of(root, &world_id, 4), Some(0));
    assert_eq!(
        read_transcript(root, &world_id, 4).unwrap()[0].text,
        "開場。"
    );
    assert_eq!(chapter_now(root, &world_id), 1);

    // 新分支走到第 2 章後選章：只走這條分支的祖先（第 5 幕），不跳回已離開的第 1 幕；可重玩目前這章
    begin_next_scene_as(root, &world_id, "新線摘要", None, true).unwrap(); // 5
    assert_eq!(replay_chapter(root, &world_id, 2).unwrap(), 6);
    assert_eq!(source_of(root, &world_id, 6), Some(5));
    assert!(replay_chapter(root, &world_id, 3).is_err());
}

#[test]
fn replay_publish_failure_keeps_the_scene_then_retries_once() {
    let root = TestRoot::new("vn-replay-fail");
    let (root, world_id) = (
        root.path(),
        create_world_of_kind(root.path(), "重玩失敗桌", TableKind::Vn).unwrap(),
    );
    line(root, &world_id, 0, "開場。");
    begin_next_scene_as(root, &world_id, "摘要", None, true).unwrap();
    fail_publish::arm();
    assert!(replay_chapter(root, &world_id, 1).is_err());
    let state = read_state(root, &world_id).unwrap();
    assert_eq!(
        (state.current_scene, vn::chapter_in(&state.state.tree)),
        (1, 2)
    );
    assert_eq!(replay_chapter(root, &world_id, 1).unwrap(), 2);
    assert_eq!(chapter_now(root, &world_id), 1);
}
