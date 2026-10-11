//! takeover-opening-statusbar-branch：介面接管桌貼開場白、GM 回覆時，原卡格式的狀態區塊不寫進樹；
//! 非接管桌照舊收。開場白走正式的 `post_opening_text`，GM 回覆走正式的提交入口 `apply_gm_block`。
use super::super::test_support::*;
use super::super::*;
use crate::data::{self, FieldKind, FieldRule, InjectLevel, StateNode, UpdateMode};
use crate::import;
use std::collections::BTreeMap;
use std::path::Path;

const CARD: &str = r#"{"spec":"chara_card_v3","data":{"name":"驛站","first_mes":"十月初七。","character_book":{"name":"驛站","entries":[{"keys":[],"constant":true,"content":"每回合輸出 <Status_block> YAML","comment":"美化状态栏","enabled":true}]}}}"#;

/// 原卡開場白：正文＋三種狀態區塊＋一段照原卡結構寫的 UpdateVariable
const OPENING: &str = "<maintext>\n林遠走進驛站。\n</maintext>\n<Status_block>\n状态栏:\n  地点: \"📍 北境驿站 前院\"\n  人物列表:\n    - 人物:\n        名字: \"👤 林远\"\n</Status_block>\n<details><summary>狀態</summary>\n天气: 小雪\n</details>\n```state\n糧草: 999\n```\n<UpdateVariable><JSONPatch>[{\"op\":\"insert\",\"path\":\"/驿站/马匹\",\"value\":37}]</JSONPatch></UpdateVariable>";

fn rule(kind: FieldKind, update: UpdateMode) -> FieldRule {
    FieldRule {
        kind,
        min: None,
        max: None,
        update,
        inject: InjectLevel::Turn,
        branch: None,
        formula: None,
    }
}

fn outcome_with(
    mode: &str,
    state_fields: serde_json::Value,
    shell: &str,
    rules: &[(&str, FieldRule)],
) -> RefactorOutcome {
    RefactorOutcome {
        mode: Some(mode.to_owned()),
        characters: Vec::new(),
        interface: Some(RefactorInterface {
            state_fields,
            source_uids: Vec::new(),
            raw: String::new(),
            shell: Some(shell.to_owned()),
            rules: rules
                .iter()
                .map(|(path, rule)| ((*path).to_owned(), rule.clone()))
                .collect(),
            guide: "地點每回合必報".to_owned(),
        }),
        mechanisms: Vec::new(),
        entries: Vec::new(),
        deletable_shared_uids: Vec::new(),
        dropped: Vec::new(),
        unabsorbed: Vec::new(),
        audit: Vec::new(),
        preserve_source_uids: Vec::new(),
        source_fingerprints: Default::default(),
    }
}

/// 頂層骨架（NorthHall v2 那型）
fn top_level_outcome() -> RefactorOutcome {
    outcome_with(
        "interface",
        serde_json::json!({ "地點": "前院", "糧草": "320" }),
        "<maintext>\n{{本回合.正文}}\n</maintext>\n<Status_block>\n地点: \"📍 {{地點}}\"\n粮草: \"🌾 {{糧草}}\"\n</Status_block>",
        &[
            ("地點", rule(FieldKind::Text, UpdateMode::Replace)),
            ("糧草", rule(FieldKind::Text, UpdateMode::Replace)),
        ],
    )
}

/// 骨架掛在 `状态栏.*` 底下（跟原卡 YAML 同路徑，值不帶裝飾）
fn nested_outcome() -> RefactorOutcome {
    outcome_with(
        "interface",
        serde_json::json!({ "状态栏": { "地点": "北境驿站 前院" } }),
        "<Status_block>\n状态栏:\n  地点: \"📍 {{状态栏.地点}}\"\n</Status_block>",
        &[("状态栏.地点", rule(FieldKind::Text, UpdateMode::Replace))],
    )
}

/// 匯入卡（留收據），回傳 (桌, 匯入來源)
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

fn apply_interface(root: &Path, world_id: &str, outcome: &RefactorOutcome) {
    let selection = RefactorSelection {
        apply_interface: true,
        ..no_player_selection(Vec::new())
    };
    apply(root, world_id, outcome, &selection).unwrap();
}

fn post(
    root: &Path,
    world_id: &str,
    source: Option<&str>,
) -> data::DataResult<data::TranscriptEvent> {
    import::post_opening_text(
        root,
        world_id,
        0,
        "2026-10-11T19:34:52+08:00",
        OPENING,
        "zh-TW",
        Some(0),
        source,
        &data::test_exclusive(world_id),
    )
}

fn tree(root: &Path, world_id: &str) -> BTreeMap<String, StateNode> {
    data::read_state(root, world_id).unwrap().state.tree
}

fn leaf(value: &str) -> StateNode {
    StateNode::Leaf(value.to_owned())
}

fn log_kinds(root: &Path, world_id: &str) -> Vec<String> {
    let path = data::mechanism_log_path(root, world_id).unwrap();
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter_map(|line| line["kind"].as_str().map(str::to_owned))
        .collect()
}

fn pending_leftovers(root: &Path, world_id: &str) -> Vec<String> {
    std::fs::read_dir(root.join("worlds").join(world_id))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("import-pending"))
        .collect()
}

#[test]
fn takeover_opening_keeps_the_skeleton_tree() {
    let root = TestRoot::new("takeover-opening");
    let (world_id, source) = imported(&root);
    apply_interface(root.path(), &world_id, &top_level_outcome());
    let before = tree(root.path(), &world_id);

    let event = post(root.path(), &world_id, source.as_deref()).unwrap();

    assert_eq!(
        tree(root.path(), &world_id),
        before,
        "不長 状态栏、不收圍欄與 UpdateVariable"
    );
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(state.state.table.is_empty());
    assert!(state.state.notes.is_empty(), "開場白不回饋 GM");
    assert_eq!(
        event.state.as_ref().unwrap().tree,
        before,
        "開場事件快照同樣乾淨"
    );
    assert!(!event.text.contains("Status_block") && !event.text.contains("UpdateVariable"));
    assert!(event.text.contains("林遠走進驛站"));
    assert_eq!(event.raw.as_deref(), Some(OPENING));
    assert!(log_kinds(root.path(), &world_id).contains(&"ignored".to_owned()));
    assert!(pending_leftovers(root.path(), &world_id).is_empty());
    let ledger = crate::mechanism::read_ledger(root.path(), &world_id);
    assert_eq!((ledger.rejected, ledger.errors), (0, 0), "面板不計");
}

#[test]
fn takeover_opening_does_not_overwrite_a_nested_skeleton() {
    let root = TestRoot::new("takeover-opening-nested");
    let (world_id, source) = imported(&root);
    apply_interface(root.path(), &world_id, &nested_outcome());
    post(root.path(), &world_id, source.as_deref()).unwrap();
    let tree = tree(root.path(), &world_id);
    assert_eq!(
        tree.get("状态栏"),
        Some(&StateNode::Branch(BTreeMap::from([(
            "地点".to_owned(),
            leaf("北境驿站 前院")
        )]))),
        "不被帶 emoji 的原值蓋掉、不長 人物列表"
    );
}

#[test]
fn non_takeover_tables_still_collect_the_status_block() {
    let root = TestRoot::new("takeover-opening-plain");
    // 未重構
    let (plain, source) = imported(&root);
    post(root.path(), &plain, source.as_deref()).unwrap();
    assert!(tree(root.path(), &plain).contains_key("状态栏"));

    // characters 模式
    let (characters, source) = imported(&root);
    let mut outcome = top_level_outcome();
    outcome.mode = Some("characters".to_owned());
    apply_interface(root.path(), &characters, &outcome);
    post(root.path(), &characters, source.as_deref()).unwrap();
    assert!(tree(root.path(), &characters).contains_key("状态栏"));

    // 清殼後
    let (cleared, source) = imported(&root);
    apply_interface(root.path(), &cleared, &top_level_outcome());
    data::write_interface_shell(root.path(), &cleared, "  ").unwrap();
    post(root.path(), &cleared, source.as_deref()).unwrap();
    assert!(tree(root.path(), &cleared).contains_key("状态栏"));
}

#[test]
fn unreadable_shell_refuses_the_opening() {
    let root = TestRoot::new("takeover-opening-unreadable");
    let (world_id, source) = imported(&root);
    apply_interface(root.path(), &world_id, &top_level_outcome());
    let shell = data::interface_shell_path(root.path(), &world_id).unwrap();
    // 不是 UTF-8：讀殼失敗
    std::fs::write(&shell, [0xff, 0xfe, 0x00]).unwrap();
    let before = tree(root.path(), &world_id);
    assert!(post(root.path(), &world_id, source.as_deref()).is_err());
    assert!(data::read_transcript(root.path(), &world_id, 0)
        .unwrap()
        .is_empty());
    assert_eq!(tree(root.path(), &world_id), before);
}

/// GM 回覆走正式提交入口：`apply_gm_block` 裡用 gm_narrate 同一個 `gm_commit_apply`
fn commit_reply(root: &Path, world_id: &str, reply: &str) -> data::WorldState {
    let block = crate::transport::extract_state_block(reply);
    let state = data::read_state(root, world_id).unwrap();
    let takeover = data::is_interface_takeover(root, world_id, state.refactor_mode.as_deref());
    data::state_commit::with_commit(root, world_id, |tx| {
        data::message_vars::begin_turn(tx, "t1", None).unwrap();
        data::message_vars::apply_gm_block(
            tx,
            "t1",
            data::message_vars::PendingMain {
                text: block.display.clone(),
                raw: Some(reply.to_owned()),
                truncated: false,
            },
            |world, stat| {
                crate::commands::chat::gm_commit_apply(
                    world,
                    stat,
                    &block,
                    "阿濤",
                    &state.mechanism,
                    takeover,
                    false,
                )
            },
            |_, _| Vec::new(),
        )
    })
    .unwrap()
    .expect("提交成功")
    .state
}

#[test]
fn takeover_reply_only_applies_the_patch() {
    let root = TestRoot::new("takeover-reply");
    let (world_id, _) = imported(&root);
    apply_interface(root.path(), &world_id, &top_level_outcome());
    let reply = "林遠走向馬廄。\n<Status_block>\n状态栏:\n  地点: \"📍 马厩\"\n</Status_block>\n```state\n糧草: 1\n```\n<UpdateVariable><JSONPatch>[{\"op\":\"replace\",\"path\":\"/地點\",\"value\":\"馬廄\"}]</JSONPatch></UpdateVariable>";
    let committed = commit_reply(root.path(), &world_id, reply);
    let keys: Vec<&String> = committed.state.tree.keys().collect();
    assert_eq!(keys, vec!["地點", "糧草"]);
    assert_eq!(committed.state.tree.get("地點"), Some(&leaf("馬廄")));
    assert_eq!(committed.state.tree.get("糧草"), Some(&leaf("320")));
    assert!(committed.state.table.is_empty());
    assert_eq!(committed.state.notes.len(), 1);
    assert!(committed.state.notes[0].contains("UpdateVariable"));
    assert_eq!(tree(root.path(), &world_id), committed.state.tree);

    // 待落的正文由後端代落（前端沒落成時的交接）：事件快照乾淨、raw 留原文、text 已剝區塊
    data::settle_pending_turn(root.path(), &world_id).unwrap();
    let events = data::read_transcript(root.path(), &world_id, 0).unwrap();
    let landed = events.last().unwrap();
    assert_eq!(landed.state.as_ref().unwrap().tree, committed.state.tree);
    assert_eq!(landed.raw.as_deref(), Some(reply));
    assert!(landed.text.contains("林遠走向馬廄"));
    assert!(!landed.text.contains("Status_block") && !landed.text.contains("UpdateVariable"));
}

#[test]
fn plain_reply_still_collects_the_status_block() {
    let root = TestRoot::new("takeover-reply-plain");
    let (world_id, _) = imported(&root);
    let reply = "林遠走向馬廄。\n<Status_block>\n状态栏:\n  地点: \"📍 马厩\"\n</Status_block>";
    let committed = commit_reply(root.path(), &world_id, reply);
    assert!(committed.state.tree.contains_key("状态栏"));
    assert!(committed.state.notes.is_empty());
}
