//! takeover-opening-statusbar-branch：介面接管桌的狀態只走 `<UpdateVariable>`。原卡格式的狀態區塊
//! （`<Status_block>` YAML、狀態圍欄、狀態 `<details>`）在開場白是原卡的渲染輸出，在 GM 回覆是違約。
use super::test_support::{mechanism_with, rule, world_with};
use super::{apply_block_typed, BlockScope, RecordKind};
use crate::data::{self, FieldKind, StateNode};

const STATUS_BLOCK: &str = "<Status_block>\n状态栏:\n  地点: \"📍 北境驿站 前院\"\n</Status_block>";
const FENCE: &str = "```state\n糧草: 999\n```";
const DETAILS: &str = "<details><summary>狀態</summary>\n天气: 小雪\n</details>";
const PATCH: &str = "<UpdateVariable><JSONPatch>[{\"op\":\"replace\",\"path\":\"/地點\",\"value\":\"驛丞房\"}]</JSONPatch></UpdateVariable>";

fn takeover_world() -> data::WorldState {
    let mut mechanism = mechanism_with(&[("地點", rule(FieldKind::Text, None, None))]);
    mechanism.incremental = true;
    let mut world = world_with(&[("地點", "前院"), ("糧草", "320")], mechanism);
    world.refactor_mode = Some("interface".to_owned());
    world
}

fn reply() -> String {
    format!("林遠接過熱茶。\n{STATUS_BLOCK}\n{DETAILS}\n{FENCE}\n{PATCH}")
}

fn top_keys(world: &data::WorldState) -> Vec<String> {
    world.state.tree.keys().cloned().collect()
}

#[test]
fn takeover_opening_ignores_status_blocks_and_patches_and_only_logs() {
    let mut world = takeover_world();
    let before = world.state.tree.clone();
    let block = crate::transport::extract_state_block(&reply());
    assert_eq!(block.fields.len(), 3, "三種狀態區塊都要被認出來");
    let outcome = apply_block_typed(
        &mut world,
        &block,
        "阿濤",
        None,
        BlockScope::TakeoverOpening,
    );
    assert_eq!(
        world.state.tree, before,
        "開場白不寫樹（含 UpdateVariable）"
    );
    assert!(world.state.table.is_empty());
    assert_eq!(outcome.records.len(), 1);
    assert_eq!(outcome.records[0].kind, RecordKind::Ignored);
    assert!(
        outcome.notes.is_empty() && world.state.notes.is_empty(),
        "不回饋 GM"
    );
}

#[test]
fn takeover_reply_drops_status_blocks_applies_patches_and_feeds_back_once() {
    let mut world = takeover_world();
    let block = crate::transport::extract_state_block(&reply());
    let outcome = apply_block_typed(&mut world, &block, "阿濤", None, BlockScope::TakeoverReply);
    assert_eq!(top_keys(&world), vec!["地點".to_owned(), "糧草".to_owned()]);
    assert_eq!(
        world.state.tree.get("地點"),
        Some(&StateNode::Leaf("驛丞房".to_owned()))
    );
    assert_eq!(
        world.state.tree.get("糧草"),
        Some(&StateNode::Leaf("320".to_owned())),
        "狀態圍欄寫骨架同名欄位也不收"
    );
    assert!(world.state.table.is_empty(), "單段狀態欄位也不收");
    let rejected: Vec<_> = outcome
        .records
        .iter()
        .filter(|record| record.kind == RecordKind::Rejected)
        .collect();
    assert_eq!(rejected.len(), 1, "三種區塊合併成一則回饋");
    assert_eq!(world.state.notes, vec![rejected[0].detail.clone()]);
    assert!(world.state.notes[0].contains("UpdateVariable"));
}

#[test]
fn takeover_reply_without_status_block_has_no_feedback() {
    let mut world = takeover_world();
    let block = crate::transport::extract_state_block(&format!("旁白。\n{PATCH}"));
    let outcome = apply_block_typed(&mut world, &block, "阿濤", None, BlockScope::TakeoverReply);
    assert!(outcome.records.is_empty());
    assert!(world.state.notes.is_empty());
}

#[test]
fn table_scope_still_collects_status_blocks() {
    let mut world = takeover_world();
    world.refactor_mode = None;
    let block = crate::transport::extract_state_block(&reply());
    apply_block_typed(&mut world, &block, "阿濤", None, BlockScope::Table);
    assert!(world.state.tree.contains_key("状态栏"));
    // 單段路徑落平欄
    assert_eq!(
        world.state.table.get("天气").map(String::as_str),
        Some("小雪")
    );
    assert_eq!(
        world.state.table.get("糧草").map(String::as_str),
        Some("999")
    );
}
