//! refactor-outcome-fields：退場桌套用盤點 FIELDS（沒有開場值的欄位補空格子追蹤）；地圖、記憶類照一般格子收〔作者裁決 2026-10-29〕。
use super::super::test_support::*;
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

fn apply_fields(root: &Path, world_id: &str, fields: &[&str]) -> data::WorldState {
    let mut product = outcome(first_uid(root, world_id), BTreeMap::new(), Vec::new(), &[]);
    product.fields = names(fields);
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
    );
    assert!(state.mechanism.incremental);
    assert_eq!(leaf(&state, "当前物品"), Some(""));
    assert_eq!(leaf(&state, "推荐行动"), Some(""));
    assert_eq!(leaf(&state, "世界地图"), Some(""));
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
fn fields_already_on_the_tree_under_another_branch_are_not_duplicated() {
    let root = TestRoot::new("survey-fields-nested");
    let world_id = north_world(root.path());
    let state = apply_fields(root.path(), &world_id, &["天气", "地点", "伤势"]);
    assert!(data::node_at(&state.state.tree, &path("天气")).is_none());
    assert!(data::node_at(&state.state.tree, &path("地点")).is_none());
    assert!(leaf(&state, "状态栏.天气").is_some());
    assert_eq!(leaf(&state, "伤势"), Some(""));
}

#[test]
fn interface_products_ignore_fields() {
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

/// 收據記被刪的規則：套用刪掉既有規則，undo 要插回原值。
#[test]
fn undo_brings_back_rules_removed_by_an_apply() {
    let root = TestRoot::new("survey-fields-undo-rule");
    let world_id = north_world(root.path());
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    let rule = FieldRule::for_kind(FieldKind::Text);
    state
        .mechanism
        .rules
        .insert("状态栏.天气".to_owned(), rule.clone());
    data::write_state(root.path(), &world_id, &state).unwrap();
    let before = receipts::snapshot_refactor(root.path(), &world_id).unwrap();
    state.mechanism.rules.remove("状态栏.天气");
    data::write_state(root.path(), &world_id, &state).unwrap();
    receipts::record_refactor_apply(
        root.path(),
        &world_id,
        "refactor",
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        before,
        &data::test_exclusive(&world_id),
    );
    receipts::undo_last_import(root.path(), &world_id, &data::test_exclusive(&world_id)).unwrap();
    let undone = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(undone.mechanism.rules.get("状态栏.天气"), Some(&rule));
}
