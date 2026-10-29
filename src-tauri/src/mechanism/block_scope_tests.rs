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
    // 增量桌：樹上已有同名頂層葉子的圍欄欄位改走 replace（數字欄照 replace 守門），不在平欄長第二份
    assert!(!world.state.table.contains_key("糧草"));
    assert_eq!(
        world.state.tree.get("糧草"),
        Some(&StateNode::Leaf("320".to_owned()))
    );
}

/// mechanism-error-feedback：接管桌 GM 回覆的 `<UpdateVariable>` 寫錯路徑也回饋。
#[test]
fn takeover_reply_patch_with_a_wrong_path_feeds_back() {
    let mut world = takeover_world();
    let reply = "<UpdateVariable><JSONPatch>[{\"op\":\"replace\",\"path\":\"/地點名\",\"value\":\"驛丞房\"}]</JSONPatch></UpdateVariable>";
    let block = crate::transport::extract_state_block(reply);
    apply_block_typed(&mut world, &block, "阿濤", None, BlockScope::TakeoverReply);
    assert_eq!(
        world.state.notes,
        vec!["路徑不存在：/地點名。頂層現有：地點、糧草。若要新增請用 insert。".to_owned()]
    );
}

/// gm-reprints-retired-statusbar：卡片狀態欄退場桌只拒收卡片自帶的狀態區塊，圍欄（含固定鍵）與更新區塊照收、回饋一次
#[test]
fn retired_card_blocks_keep_fence_and_patches_and_feed_back() {
    let mut mechanism = mechanism_with(&[("地點", rule(FieldKind::Text, None, None))]);
    mechanism.incremental = true;
    let mut world = world_with(&[("地點", "前院"), ("糧草", "320")], mechanism);
    world.refactor_mode = Some("characters".to_owned());
    world.statusbar_retired = true;
    let reply = format!(
        "林遠接過熱茶。\n{STATUS_BLOCK}\n{DETAILS}\n```state\ntime: 夜\nplace: 驛站\npresent: 林遠\n天色: 黑\n```\n{PATCH}"
    );
    let block = crate::transport::extract_state_block(&reply);
    let outcome = apply_block_typed(
        &mut world,
        &block,
        "阿濤",
        None,
        BlockScope::RetiredCardBlocks,
    );
    assert_eq!(
        world.state.table.get("place").map(String::as_str),
        Some("驛站")
    );
    assert_eq!(
        world.state.table.get("present").map(String::as_str),
        Some("林遠")
    );
    assert_eq!(
        world.state.table.get("天色").map(String::as_str),
        Some("黑")
    );
    assert_eq!(
        world.state.tree.get("地點"),
        Some(&StateNode::Leaf("驛丞房".to_owned()))
    );
    assert!(!world.state.table.contains_key("天气"), "卡片狀態欄不收");
    assert!(!top_keys(&world).contains(&"状态栏".to_owned()));
    let rejected: Vec<_> = outcome
        .records
        .iter()
        .filter(|record| record.kind == RecordKind::Rejected)
        .collect();
    assert_eq!(rejected.len(), 1);
    assert_eq!(world.state.notes.len(), 1, "下一輪回饋 GM");

    // 沒有卡片狀態區塊的回覆：照常、不回饋
    let mut clean = world.clone();
    clean.state.notes.clear();
    let block = crate::transport::extract_state_block("夜深了。\n```state\nplace: 前院\n```");
    assert!(block.fenced.is_none());
    let outcome = apply_block_typed(
        &mut clean,
        &block,
        "阿濤",
        None,
        BlockScope::RetiredCardBlocks,
    );
    assert!(outcome
        .records
        .iter()
        .all(|record| record.kind != RecordKind::Rejected));
    assert_eq!(
        clean.state.table.get("place").map(String::as_str),
        Some("前院")
    );
}

/// 卡片區塊排在合法圍欄之後、寫同一個鍵，或帶清單／容器：被拒的來源不蓋合法值、不長枝
#[test]
fn retired_card_blocks_after_fence_do_not_override_or_grow_branches() {
    let mut world = world_with(&[("地點", "前院")], mechanism_with(&[]));
    world.refactor_mode = Some("characters".to_owned());
    world.statusbar_retired = true;
    let reply = "夜深了。\n```state\nplace: 驛站\n天色: 黑\n名單: 甲\n```\n<details><summary>状态栏</summary>\nplace: 錯地方\n天色: 白\n名單:\n  - 乙\n  - 丙\n隊伍:\n  林遠: 疲憊\n</details>";
    let block = crate::transport::extract_state_block(reply);
    assert!(
        block
            .fields
            .iter()
            .any(|(path, value)| path == &vec!["天色".to_owned()] && value == "白"),
        "不分來源時後塊勝出"
    );
    let outcome = apply_block_typed(
        &mut world,
        &block,
        "阿濤",
        None,
        BlockScope::RetiredCardBlocks,
    );
    assert_eq!(
        world.state.table.get("place").map(String::as_str),
        Some("驛站")
    );
    assert_eq!(
        world.state.table.get("天色").map(String::as_str),
        Some("黑")
    );
    assert_eq!(
        world.state.table.get("名單").map(String::as_str),
        Some("甲")
    );
    assert!(
        world.state.tree.get("名單").is_none(),
        "卡片區塊的清單不長枝"
    );
    assert!(
        world.state.tree.get("隊伍").is_none(),
        "卡片區塊的容器不長枝"
    );
    assert_eq!(
        outcome
            .records
            .iter()
            .filter(|record| record.kind == RecordKind::Rejected)
            .count(),
        1
    );
}

/// playdata-loss-fixes D1：退場桌照舊印的卡片狀態欄，對得上樹上既有葉子的格子照收；對不上的（含超過回饋句
/// 240 字上限的長值、卡片清單底下的）整值列進 `unapplied`，由呼叫端寫進狀態更新事件。回饋句下一輪就覆寫，保不住值。
#[test]
fn retired_card_cells_on_existing_leaves_are_kept_and_the_rest_are_listed_whole() {
    let mut mechanism = mechanism_with(&[
        ("天气", rule(FieldKind::Text, None, None)),
        ("体力", rule(FieldKind::Number, Some(0.0), Some(100.0))),
    ]);
    mechanism.incremental = true;
    let mut world = world_with(
        &[("天气", "晴"), ("体力", "5"), ("人物.林遠.心情", "平靜")],
        mechanism,
    );
    world.refactor_mode = Some("characters".to_owned());
    world.statusbar_retired = true;
    let long = "军需".repeat(150);
    let reply = format!(
        "雪越下越大。\n<details><summary>状态栏</summary>\n天气: 大雪\n体力: 15\n人物:\n  林遠:\n    心情: 愉快\n军需: {long}\n名單:\n  - 乙\n</details>"
    );
    let block = crate::transport::extract_state_block(&reply);
    let cells = super::card_block_cells(&block);
    let outcome = apply_block_typed(
        &mut world,
        &block,
        "阿濤",
        None,
        BlockScope::RetiredCardBlocks,
    );
    assert_eq!(
        world.state.tree.get("天气"),
        Some(&StateNode::Leaf("大雪".to_owned()))
    );
    assert_eq!(
        data::node_at(
            &world.state.tree,
            &["人物".to_owned(), "林遠".to_owned(), "心情".to_owned()]
        ),
        Some(&StateNode::Leaf("愉快".to_owned()))
    );
    assert_eq!(
        world.state.tree.get("体力"),
        Some(&StateNode::Leaf("15".to_owned())),
        "數字格走 delta 收進樹"
    );
    assert!(world.state.tree.get("军需").is_none(), "對不上的不長新格");
    assert!(world.state.tree.get("名單").is_none(), "卡片清單不長枝");
    let left = super::unapplied(&world, &cells);
    let paths: Vec<&str> = left.iter().map(|(path, _)| path.as_str()).collect();
    assert!(paths.contains(&"军需"), "{left:?}");
    assert!(
        paths.iter().any(|path| path.starts_with("名單")),
        "{left:?}"
    );
    let kept = left.iter().find(|(path, _)| path == "军需").unwrap();
    assert_eq!(kept.1, long, "長值整段留存");
    assert!(outcome.notes.iter().all(|note| !note.contains(&long)));
    // 下一輪 GM 沒補報：回饋句被覆寫，值只剩狀態更新事件那份
    let next = crate::transport::extract_state_block("夜深了。");
    apply_block_typed(
        &mut world,
        &next,
        "阿濤",
        None,
        BlockScope::RetiredCardBlocks,
    );
    assert!(world.state.notes.iter().all(|note| !note.contains("军需")));
}
