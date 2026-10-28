//! takeover-opening-statusbar-branch：非 MVU 來源的桌，既有樹只是開場白狀態區塊解析出來的原卡渲染輸出
//! （有損）。介面重構時不把它附給 AI、套用時以產物的 state_fields 為準；MVU 桌與 characters 模式照
//! takeover-mvu-path-mismatch 的做法。
use super::super::test_support::*;
use super::super::*;
use crate::data::{self, StateNode};
use crate::{import, receipts};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::Path;

/// NorthHall 型開場白：三個人物、裝飾 emoji、一欄拼兩值
const OPENING: &str = "<maintext>\n林遠走進驛站。\n</maintext>\n<Status_block>\n状态栏:\n  地点: \"📍 北境驿站 前院\"\n  人物列表:\n    - 人物:\n        名字: \"👤 林远\"\n    - 人物:\n        名字: \"👤 贺铮\"\n    - 人物:\n        名字: \"👤 周掌柜\"\n</Status_block>";

const PEOPLE: &str = "    - 人物:\n        名字: \"👤 林远\"\n    - 人物:\n        名字: \"👤 贺铮\"\n    - 人物:\n        名字: \"👤 周掌柜\"";

fn card(extensions: serde_json::Value, book: serde_json::Value) -> String {
    json!({"spec":"chara_card_v3","data":{
        "name":"驛站","first_mes":"十月初七。","extensions": extensions,
        "character_book":{"name":"驛站","entries": book}
    }})
    .to_string()
}

fn plain_book() -> serde_json::Value {
    json!([{"keys":[],"constant":true,"content":"每回合輸出 <Status_block> YAML","comment":"美化状态栏","enabled":true}])
}

/// 世界書卡（非 MVU）：匯入、貼開場白，樹上長出 状态栏.*
fn plain_table(root: &TestRoot) -> String {
    let world_id = data::create_world(root.path(), "北境驛站").unwrap();
    let source = import::import_worldbook_file(
        root.path(),
        &world_id,
        card(json!({}), plain_book()).as_bytes(),
        "驛站",
        &data::test_exclusive(&world_id),
    )
    .unwrap()
    .source;
    import::post_opening_text(
        root.path(),
        &world_id,
        0,
        "2026-10-11T19:00:00",
        OPENING,
        "zh-TW",
        Some(0),
        source.as_deref(),
        &data::test_exclusive(&world_id),
    )
    .unwrap();
    world_id
}

fn mark_mvu(root: &Path, world_id: &str) {
    let mut state = data::read_state(root, world_id).unwrap();
    state.mechanism.incremental = true;
    data::write_state(root, world_id, &state).unwrap();
}

fn nested_fields() -> serde_json::Value {
    json!({ "状态栏": { "地点": "北境驿站 前院", "人物列表": PEOPLE, "新葉": "x" } })
}

fn outcome(mode: &str, state_fields: serde_json::Value) -> RefactorOutcome {
    RefactorOutcome {
        mode: Some(mode.to_owned()),
        characters: Vec::new(),
        interface: Some(RefactorInterface {
            state_fields,
            source_uids: Vec::new(),
            raw: String::new(),
            shell: Some(
                "<Status_block>\n状态栏:\n  地点: \"📍 {{状态栏.地点}}\"\n  人物列表:\n{{状态栏.人物列表}}\n  新: {{状态栏.新葉}}\n</Status_block>"
                    .to_owned(),
            ),
            rules: BTreeMap::new(),
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

fn tree(root: &Path, world_id: &str) -> BTreeMap<String, StateNode> {
    data::read_state(root, world_id).unwrap().state.tree
}

fn leaf_at(tree: &BTreeMap<String, StateNode>, path: &[&str]) -> Option<StateNode> {
    let segments: Vec<String> = path.iter().map(|segment| (*segment).to_owned()).collect();
    data::node_at(tree, &segments).cloned()
}

fn leaf(value: &str) -> Option<StateNode> {
    Some(StateNode::Leaf(value.to_owned()))
}

#[test]
fn non_mvu_interface_apply_takes_the_product_values_and_undo_restores() {
    let root = TestRoot::new("tree-authority-plain");
    let world_id = plain_table(&root);
    let parsed = tree(root.path(), &world_id);
    assert_eq!(
        leaf_at(&parsed, &["状态栏", "地点"]),
        leaf("📍 北境驿站 前院"),
        "前提：開場白的 YAML 解析進樹"
    );

    apply_recorded(
        root.path(),
        &world_id,
        &outcome("interface", nested_fields()),
        &selection(),
    );
    let applied = tree(root.path(), &world_id);
    assert_eq!(
        leaf_at(&applied, &["状态栏", "地点"]),
        leaf("北境驿站 前院")
    );
    assert_eq!(leaf_at(&applied, &["状态栏", "人物列表"]), leaf(PEOPLE));
    assert_eq!(leaf_at(&applied, &["状态栏", "新葉"]), leaf("x"));

    receipts::undo_last_import(root.path(), &world_id, &data::test_exclusive(&world_id)).unwrap();
    assert_eq!(tree(root.path(), &world_id), parsed);
}

#[test]
fn mvu_table_keeps_the_existing_top_level_branch() {
    let root = TestRoot::new("tree-authority-mvu");
    let world_id = plain_table(&root);
    mark_mvu(root.path(), &world_id);
    let parsed = tree(root.path(), &world_id);
    apply_recorded(
        root.path(),
        &world_id,
        &outcome("interface", nested_fields()),
        &selection(),
    );
    assert_eq!(
        tree(root.path(), &world_id).get("状态栏"),
        parsed.get("状态栏"),
        "MVU 桌照 44db586：同名頂層整棵保留"
    );
}

/// 非 MVU 接管桌清回匯入原卡後再套同頂層的產物：新葉與乾淨值進樹
#[test]
fn reapply_after_reset_takes_the_product_values() {
    let root = TestRoot::new("tree-authority-rerun");
    let world_id = plain_table(&root);
    apply(
        root.path(),
        &world_id,
        &outcome("interface", nested_fields()),
        &selection(),
    )
    .unwrap();
    assert_eq!(
        rerun_status(root.path(), &world_id).unwrap(),
        RerunStatus::Ready
    );
    reset_to_import_source(root.path(), &world_id, "zh-TW").unwrap();
    assert_eq!(
        leaf_at(&tree(root.path(), &world_id), &["状态栏", "地点"]),
        leaf("📍 北境驿站 前院"),
        "清回後開場白重貼、樹又是解析出來的那棵"
    );
    let mut fields = nested_fields();
    fields["状态栏"]["再一葉"] = json!("y");
    apply(
        root.path(),
        &world_id,
        &outcome("interface", fields),
        &selection(),
    )
    .unwrap();
    let applied = tree(root.path(), &world_id);
    assert_eq!(
        leaf_at(&applied, &["状态栏", "地点"]),
        leaf("北境驿站 前院")
    );
    assert_eq!(leaf_at(&applied, &["状态栏", "新葉"]), leaf("x"));
    assert_eq!(leaf_at(&applied, &["状态栏", "再一葉"]), leaf("y"));
}

#[test]
fn tree_section_is_withheld_only_for_non_mvu_interface_runs() {
    let root = TestRoot::new("tree-authority-section");
    let world_id = plain_table(&root);
    let section = |mode: &str| {
        crate::refactor_ai::table_tree_section(root.path(), &world_id, Some(mode)).unwrap()
    };
    assert_eq!(section("interface"), "");
    assert!(section("characters").contains("状态栏.地点"));
    mark_mvu(root.path(), &world_id);
    assert!(section("interface").contains("状态栏.地点"));
}

const MVU_SCRIPT: &str =
    "import'https://testingcf.jsdelivr.net/gh/MagicalAstrogy/MagVarUpdate/artifact/bundle.js';";

fn script_card(remote_loader: bool) -> String {
    let regex = if remote_loader {
        json!([{"scriptName":"載入","findRegex":"/[\\s\\S]*/","replaceString":"<div></div><script>$('body').load('https://example.com/ui');</script>","placement":[2]}])
    } else {
        json!([])
    };
    card(
        json!({
            "tavern_helper": {"scripts": [{"name":"MVU","enabled":true,"type":"script","content": MVU_SCRIPT}]},
            "regex_scripts": regex
        }),
        plain_book(),
    )
}

fn character_table(root: &TestRoot, raw: &str) -> String {
    let world_id = data::create_world(root.path(), "腳本卡").unwrap();
    import::import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff", "zh-TW").unwrap();
    world_id
}

#[test]
fn mvu_source_counts_supported_script_cards_and_reports_read_errors() {
    let root = TestRoot::new("tree-authority-source");
    let supported = character_table(&root, &script_card(false));
    let state = data::read_state(root.path(), &supported).unwrap();
    assert!(!state.mechanism.incremental);
    assert!(crate::mechanism::mvu_source(root.path(), &supported, &state).unwrap());

    // 被判不支援的卡（遠端載入器）不算，與卡片變數模式的啟用條件一致
    let remote = character_table(&root, &script_card(true));
    let cards = import::read_card_interfaces(root.path(), &remote).unwrap();
    assert!(cards
        .iter()
        .any(|card| card.mvu && card.unsupported.is_some()));
    let state = data::read_state(root.path(), &remote).unwrap();
    assert!(!crate::mechanism::mvu_source(root.path(), &remote, &state).unwrap());

    // 讀不到角色目錄：重構用途回錯，不預設成非 MVU
    let broken = plain_table(&root);
    let characters = root.path().join("worlds").join(&broken).join("characters");
    if characters.exists() {
        std::fs::remove_dir_all(&characters).unwrap();
    }
    std::fs::write(&characters, "not a dir").unwrap();
    let state = data::read_state(root.path(), &broken).unwrap();
    assert!(crate::mechanism::mvu_source(root.path(), &broken, &state).is_err());
    assert!(
        crate::refactor_ai::table_tree_section(root.path(), &broken, Some("interface")).is_err()
    );
    assert!(apply(
        root.path(),
        &broken,
        &outcome("interface", nested_fields()),
        &selection()
    )
    .is_err());
}
