use super::*;
use crate::data::{TableKind, TranscriptEvent, TranscriptKind};

fn line(root: &Path, world_id: &str, scene: u64, kind: TranscriptKind, text: &str) {
    let event = TranscriptEvent {
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
        raw: None,
        ts: "2026-11-02 00:00".to_owned(),
        speaker_id: if kind == TranscriptKind::Dialogue {
            "c1".to_owned()
        } else {
            String::new()
        },
        speaker_name: "GM".to_owned(),
        kind,
        text: text.to_owned(),
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
    };
    data::append_transcript(root, world_id, scene, &event).unwrap();
}

const CHOICE: &str = "門外有聲音。\n[choices]\n- 甲\n- 乙";

/// 第 0 幕：選項 → 角色插話 → 玩家選甲 → 續。回桌與選項事件 id。
fn table(name: &str) -> (std::path::PathBuf, String) {
    let root = crate::smart_free::test_root(name);
    let world_id = data::create_world_of_kind(&root, "選項桌", TableKind::Vn).unwrap();
    let path = root.as_path();
    line(path, &world_id, 0, TranscriptKind::Narration, CHOICE);
    line(path, &world_id, 0, TranscriptKind::Dialogue, "誰？");
    line(path, &world_id, 0, TranscriptKind::Player, "甲");
    line(path, &world_id, 0, TranscriptKind::Narration, "你開了門。");
    (root, world_id)
}

/// 第一個選項點：（事件 id, 玩家選的那句；還沒選＝None）
fn choice_event(path: &Path, world_id: &str, scene: u64) -> (String, Option<String>) {
    let (cells, _) = read_scene(path, world_id, scene).unwrap();
    let event = cells
        .into_iter()
        .find(|event| event.choice.is_some())
        .unwrap();
    (event.event_id.unwrap(), event.choice.unwrap().answered)
}

fn options(branches: &[ChoiceBranch]) -> Vec<(&str, u64)> {
    branches
        .iter()
        .map(|branch| (branch.option.as_str(), branch.tip))
        .collect()
}

/// 選項後夾角色線：分岔保留角色線、截到玩家句前；新分支的選項仍是未選（面板照出、送出失敗可重按）。
#[test]
fn fork_keeps_character_lines_and_leaves_the_choice_open() {
    let (root, world_id) = table("vn-branches-fork");
    let path = root.as_path();
    let (event_id, mark) = choice_event(path, &world_id, 0);
    assert_eq!(mark.as_deref(), Some("甲"));

    assert_eq!(fork_choice(path, &world_id, &event_id).unwrap(), 1);
    let kept: Vec<String> = data::read_transcript(path, &world_id, 1)
        .unwrap()
        .into_iter()
        .map(|event| event.text)
        .collect();
    assert_eq!(kept, vec![CHOICE.to_owned(), "誰？".to_owned()]);
    let (live_id, mark) = choice_event(path, &world_id, 1);
    assert_eq!(mark, None);

    // 還沒選的選項點不分岔
    let error = fork_choice(path, &world_id, &live_id).unwrap_err();
    assert_eq!(error, UiMsg::VnChoiceNotPast.to_string());
}

/// 分支查詢：源頭自己選的、同點分岔後自己選的各一條；分岔了還沒選的、截得更後（複製了選擇）的不列。
#[test]
fn branches_at_a_choice_point() {
    let (root, world_id) = table("vn-branches-list");
    let path = root.as_path();
    let (origin_id, _) = choice_event(path, &world_id, 0);
    fork_choice(path, &world_id, &origin_id).unwrap(); // 1
    line(path, &world_id, 1, TranscriptKind::Player, "乙");
    line(
        path,
        &world_id,
        1,
        TranscriptKind::Narration,
        "你躲了起來。",
    );
    let (here, _) = choice_event(path, &world_id, 1);
    assert_eq!(
        options(&choice_branches(path, &world_id, &here).unwrap()),
        vec![("乙", 1), ("甲", 0)]
    );

    // 從 1 再分岔一次但不選；從 0 整幕分岔（複製了「甲」）：都不多列
    fork_choice(path, &world_id, &here).unwrap(); // 2
    data::fork_scene(path, &world_id, 0).unwrap(); // 3
    let (copy, mark) = choice_event(path, &world_id, 3);
    assert_eq!(mark.as_deref(), Some("甲"));
    assert_eq!(
        options(&choice_branches(path, &world_id, &copy).unwrap()),
        vec![("乙", 1), ("甲", 0)]
    );

    // 從分岔的分岔看回去（2 在 1 截、1 在 0 截）：同一組
    data::switch_scene(path, &world_id, 2).unwrap();
    line(path, &world_id, 2, TranscriptKind::Player, "丙");
    let (deep, _) = choice_event(path, &world_id, 2);
    assert_eq!(
        options(&choice_branches(path, &world_id, &deep).unwrap()),
        vec![("丙", 2), ("乙", 1), ("甲", 0)]
    );
}

fn save_at(path: &Path, world_id: &str, scene: u64, text: &str) -> String {
    let (cells, _) = read_scene(path, world_id, scene).unwrap();
    let event = cells
        .iter()
        .find(|event| matches!(event.cells.first(), Some(crate::chat_turn::VnCell::Narration { text: t }) if t == text))
        .unwrap();
    create_save(
        path,
        world_id,
        "存一下",
        event.event_id.as_deref().unwrap(),
        0,
    )
    .unwrap()
    .id
}

/// 讀檔四種：目前幕只回位置；別條尾幕切換；已換章（非尾幕）整幕分岔、回新 id；事件或格不在＝失效。
/// 另含「目前幕號小於存檔來源」（切回低號分支後讀高號分支的存檔）。
#[test]
fn loading_a_save_stays_switches_or_forks() {
    let (root, world_id) = table("vn-branches-saves");
    let path = root.as_path();
    let here = save_at(path, &world_id, 0, "你開了門。");
    assert_eq!(load_save(path, &world_id, &here).unwrap().scene, 0);

    // 換章：第 0 幕不再是尾幕 → 整幕分岔，往前讀得到原本的內容
    data::begin_next_scene_as(path, &world_id, "摘要", None, false).unwrap(); // 1
    let loaded = load_save(path, &world_id, &here).unwrap();
    assert_eq!(loaded.scene, 2);
    let events = data::read_transcript(path, &world_id, 2).unwrap();
    assert_eq!(events.len(), 4);
    assert_eq!(events[3].id.as_deref(), Some(loaded.event_id.as_str()));
    assert_eq!(texts(path, &world_id, 0), texts(path, &world_id, 2));

    // 在幕 1 存檔，從幕 2（較高號）切過去之後讀它：幕 1 是尾幕 → 切換；再讀高號分支 2 的存檔 → 切回
    data::switch_scene(path, &world_id, 1).unwrap();
    let one = save_at(path, &world_id, 1, "摘要");
    data::switch_scene(path, &world_id, 2).unwrap();
    let two = save_at(path, &world_id, 2, "你開了門。");
    assert_eq!(load_save(path, &world_id, &one).unwrap().scene, 1);
    assert_eq!(load_save(path, &world_id, &two).unwrap().scene, 2);
    assert_eq!(data::read_state(path, &world_id).unwrap().current_scene, 2);

    // 存檔指的事件被收回：失效
    data::switch_scene(path, &world_id, 1).unwrap();
    let error = create_save(path, &world_id, "x", "沒有這則", 0).unwrap_err();
    assert_eq!(error, UiMsg::VnSaveStale.to_string());
    data::switch_scene(path, &world_id, 2).unwrap();
    data::pop_transcript(path, &world_id, 2).unwrap();
    assert_eq!(
        load_save(path, &world_id, &two).unwrap_err(),
        UiMsg::VnSaveStale.to_string()
    );
    // 格索引超出、名字空白
    let (cells, _) = read_scene(path, &world_id, 2).unwrap();
    let id = cells[0].event_id.clone().unwrap();
    assert_eq!(
        create_save(path, &world_id, "x", &id, 99).unwrap_err(),
        UiMsg::VnSaveStale.to_string()
    );
    assert!(create_save(path, &world_id, "  ", &id, 0).is_err());
}

fn texts(path: &Path, world_id: &str, scene: u64) -> Vec<String> {
    data::read_transcript(path, world_id, scene)
        .unwrap()
        .into_iter()
        .map(|event| event.text)
        .collect()
}
