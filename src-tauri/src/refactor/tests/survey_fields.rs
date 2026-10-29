//! refactor-outcome-fields：退場桌套用盤點 FIELDS（沒有開場值的欄位補空格子追蹤）與 UNTRACKED（地圖、記憶類移除）。
use super::super::test_support::*;
use super::super::*;
use super::opening_fields::{
    first_uid, gm_update, leaf, north_world, outcome, path, post_opening, selection,
};
use crate::data::{self, FieldKind, FieldRule, InjectLevel};
use crate::{import, receipts};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::Path;

fn names(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

/// 西幻這類卡：世界書定義狀態欄、開場白沒附狀態區塊。
fn plain_world(root: &Path) -> String {
    let world_id = data::create_world(root, "西幻").unwrap();
    let card = json!({"spec":"chara_card_v3","data":{
        "name":"西幻","first_mes":"请选择你的身份","extensions":{},
        "character_book":{"name":"西幻","entries":[{"keys":[],"constant":true,"content":"<Time>…</Time>","comment":"回复规则","enabled":true}]}
    }})
    .to_string();
    let source = import::import_worldbook_file(
        root,
        &world_id,
        card.as_bytes(),
        "西幻",
        &data::test_exclusive(&world_id),
    )
    .unwrap()
    .source;
    post_opening(root, &world_id, "请选择你的身份", source.as_deref());
    world_id
}

fn apply_fields(
    root: &Path,
    world_id: &str,
    fields: &[&str],
    untracked: &[&str],
) -> data::WorldState {
    let mut product = outcome(first_uid(root, world_id), BTreeMap::new(), Vec::new(), &[]);
    product.fields = names(fields);
    product.untracked = names(untracked);
    apply_recorded(root, world_id, &product, &selection(false));
    data::read_state(root, world_id).unwrap()
}

#[test]
fn fields_without_opening_values_become_tracked_cells() {
    let root = TestRoot::new("survey-fields-plain");
    let world_id = plain_world(root.path());
    let before = data::read_state(root.path(), &world_id).unwrap();
    let mut state = apply_fields(
        root.path(),
        &world_id,
        &["当前物品", "推荐行动", "世界地图", "time"],
        &["世界地图"],
    );
    assert!(state.mechanism.incremental);
    assert_eq!(leaf(&state, "当前物品"), Some(""));
    assert_eq!(leaf(&state, "推荐行动"), Some(""));
    assert!(data::node_at(&state.state.tree, &path("世界地图")).is_none());
    assert!(data::node_at(&state.state.tree, &path("time")).is_none());
    let rule = &state.mechanism.rules["当前物品"];
    assert_eq!(rule.kind, FieldKind::Text);
    assert_eq!(rule.inject, InjectLevel::Turn);
    assert!(crate::mechanism::leaf_shown(
        &state.mechanism,
        &path("当前物品"),
        "",
        false
    ));
    gm_update(
        &mut state,
        r#"[{"op":"replace","path":"/当前物品","value":"短剑"}]"#,
    );
    assert_eq!(leaf(&state, "当前物品"), Some("短剑"));

    receipts::undo_last_import(root.path(), &world_id, &data::test_exclusive(&world_id)).unwrap();
    let undone = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(undone.state.tree, before.state.tree);
    assert!(undone.mechanism.rules.is_empty());
    assert!(!undone.mechanism.incremental);
}

#[test]
fn untracked_cells_are_removed_with_their_rules_and_prefixes() {
    let root = TestRoot::new("survey-fields-untracked");
    let world_id = north_world(root.path());
    let state = apply_fields(
        root.path(),
        &world_id,
        &["天气", "伤势"],
        &["驿站物资", "状态栏.天气"],
    );
    assert!(data::node_at(&state.state.tree, &path("状态栏.驿站物资")).is_none());
    assert!(data::node_at(&state.state.tree, &path("状态栏.天气")).is_none());
    let tracked = |key: &str| key.contains("驿站物资") || key.contains("天气");
    assert!(!state.mechanism.rules.keys().any(|key| tracked(key)));
    assert!(!state
        .mechanism
        .display_prefixes
        .keys()
        .any(|key| tracked(key)));
    // 天气 在不收名單裡，不在頂層補第二份；伤势 樹上沒有，補空格子
    assert!(data::node_at(&state.state.tree, &path("天气")).is_none());
    assert_eq!(leaf(&state, "伤势"), Some(""));
    // 其餘開場格子照常追蹤
    assert!(state.mechanism.rules.contains_key("状态栏.日期和时间"));
}

#[test]
fn fields_already_on_the_tree_under_another_branch_are_not_duplicated() {
    let root = TestRoot::new("survey-fields-nested");
    let world_id = north_world(root.path());
    let state = apply_fields(root.path(), &world_id, &["天气", "地点"], &[]);
    assert!(data::node_at(&state.state.tree, &path("天气")).is_none());
    assert!(data::node_at(&state.state.tree, &path("地点")).is_none());
    assert!(leaf(&state, "状态栏.天气").is_some());
}

#[test]
fn interface_products_ignore_fields_and_untracked() {
    let root = TestRoot::new("survey-fields-interface");
    let world_id = plain_world(root.path());
    let mut product = outcome(
        first_uid(root.path(), &world_id),
        BTreeMap::new(),
        Vec::new(),
        &[],
    );
    product.mode = Some("interface".to_owned());
    product.fields = names(&["当前物品"]);
    apply_recorded(root.path(), &world_id, &product, &selection(false));
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(data::node_at(&state.state.tree, &path("当前物品")).is_none());
}

#[test]
fn survey_untracked_block_is_parsed_and_kept_only_for_characters() {
    let raw = "## MODE: characters\n## FIELDS\n- 当前物品\n## MIRRORS\n## STAGE\n## UNTRACKED\n- 世界地图\n- 记忆\n";
    let mut survey = crate::refactor_ai::parse_survey(raw);
    assert_eq!(survey.untracked, names(&["世界地图", "记忆"]));
    crate::refactor_ai::normalize_survey_for_mode(&mut survey);
    assert_eq!(survey.untracked.len(), 2);
    survey.mode = "interface".to_owned();
    crate::refactor_ai::normalize_survey_for_mode(&mut survey);
    assert!(survey.untracked.is_empty());
}

#[test]
fn untracked_rules_without_any_value_are_dropped() {
    let root = TestRoot::new("survey-fields-rule-only");
    let world_id = plain_world(root.path());
    let rules: BTreeMap<String, FieldRule> = ["核心记忆", "当前物品"]
        .into_iter()
        .map(|key| (key.to_owned(), FieldRule::for_kind(FieldKind::Text)))
        .collect();
    let mut product = outcome(first_uid(root.path(), &world_id), rules, Vec::new(), &[]);
    product.untracked = names(&["核心記憶"]);
    apply_recorded(root.path(), &world_id, &product, &selection(true));
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(!state.mechanism.rules.contains_key("核心记忆"));
    assert!(state.mechanism.rules.contains_key("当前物品"));
}

#[test]
fn undo_brings_back_rules_removed_as_untracked() {
    let root = TestRoot::new("survey-fields-undo-rule");
    let world_id = north_world(root.path());
    let mut seeded = data::read_state(root.path(), &world_id).unwrap();
    let rule = FieldRule::for_kind(FieldKind::Text);
    seeded
        .mechanism
        .rules
        .insert("状态栏.天气".to_owned(), rule.clone());
    data::write_state(root.path(), &world_id, &seeded).unwrap();
    let state = apply_fields(root.path(), &world_id, &[], &["状态栏.天气"]);
    assert!(!state.mechanism.rules.contains_key("状态栏.天气"));
    receipts::undo_last_import(root.path(), &world_id, &data::test_exclusive(&world_id)).unwrap();
    let undone = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(undone.mechanism.rules.get("状态栏.天气"), Some(&rule));
    assert_eq!(undone.state.tree, seeded.state.tree);
}
