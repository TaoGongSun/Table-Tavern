//! statusbar-yaml-list-flatten：狀態區塊的清單整支替換（全量桌、增量桌、非接管重構桌、接管桌篩掉）。
use super::test_support::{mechanism_with, world_with};
use super::{apply_block_typed, BlockScope, RecordKind};
use crate::data::{self, StateNode};
use crate::transport::extract_state_block;
use std::collections::BTreeMap;

fn apply(world: &mut data::WorldState, reply: &str) -> super::Outcome {
    apply_block_typed(
        world,
        &extract_state_block(reply),
        "阿濤",
        None,
        BlockScope::Table,
    )
}

fn status(body: &str) -> String {
    format!("旁白。\n<Status_block>\n{body}</Status_block>")
}

/// 樹上一支的所有葉子（點分路徑→值），看整支形狀用。
fn leaves(world: &data::WorldState, root: &str) -> Vec<(String, String)> {
    fn walk(node: &StateNode, path: String, out: &mut Vec<(String, String)>) {
        match node {
            StateNode::Leaf(value) => out.push((path, value.clone())),
            StateNode::Branch(children) => {
                for (key, child) in children {
                    walk(child, format!("{path}.{key}"), out);
                }
            }
        }
    }
    let mut out = Vec::new();
    if let Some(node) = world.state.tree.get(root) {
        walk(node, root.to_owned(), &mut out);
    }
    out
}

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(path, value)| (path.to_string(), value.to_string()))
        .collect()
}

const THREE: &str = "人物列表:\n  - 人物:\n      名字: 林远\n  - 人物:\n      名字: 贺铮\n  - 人物:\n      名字: 周掌柜\n";
const TWO: &str = "人物列表:\n  - 人物:\n      名字: 林远\n  - 人物:\n      名字: 周掌柜\n";

#[test]
fn shorter_list_next_turn_leaves_no_stale_items() {
    let mut world = world_with(&[], mechanism_with(&[]));
    apply(&mut world, &status(THREE));
    assert_eq!(leaves(&world, "人物列表").len(), 3);
    apply(&mut world, &status(TWO));
    assert_eq!(
        leaves(&world, "人物列表"),
        pairs(&[
            ("人物列表.0.人物.名字", "林远"),
            ("人物列表.1.人物.名字", "周掌柜")
        ])
    );
}

/// 舊桌攤平留下的 `人物列表/人物/*` 經一次套用整支換成索引形
#[test]
fn flattened_old_tree_is_replaced_in_one_turn() {
    let mut world = world_with(
        &[
            ("人物列表.人物.名字", "周掌柜"),
            ("人物列表.人物.身份", "军需官"),
        ],
        mechanism_with(&[]),
    );
    apply(&mut world, &status(TWO));
    assert_eq!(
        leaves(&world, "人物列表"),
        pairs(&[
            ("人物列表.0.人物.名字", "林远"),
            ("人物列表.1.人物.名字", "周掌柜")
        ])
    );
}

/// 清單身分黏著：上回合兩項同名單鍵是清單，這回合只剩一項（解析成具名鍵）也先整支移除
#[test]
fn list_identity_sticks_when_the_list_shrinks_to_a_dict_shape() {
    let mut world = world_with(&[], mechanism_with(&[]));
    apply(
        &mut world,
        &status("名單:\n  - 名字: 林远\n  - 名字: 贺铮\n"),
    );
    assert_eq!(
        leaves(&world, "名單"),
        pairs(&[("名單.0.名字", "林远"), ("名單.1.名字", "贺铮")])
    );
    apply(&mut world, &status("名單:\n  - 名字: 林远\n"));
    assert_eq!(leaves(&world, "名單"), pairs(&[("名單.名字", "林远")]));
}

/// 重複單鍵物件兩項 → 下回合一項
#[test]
fn repeated_single_key_objects_shrink_to_one() {
    let mut world = world_with(&[], mechanism_with(&[]));
    apply(
        &mut world,
        &status("名單:\n  - 人物:\n      名字: 林远\n  - 人物:\n      名字: 贺铮\n"),
    );
    apply(&mut world, &status("名單:\n  - 人物:\n      名字: 贺铮\n"));
    assert_eq!(
        leaves(&world, "名單"),
        pairs(&[("名單.0.人物.名字", "贺铮")])
    );
}

/// 年份那種數字鍵不是清單：沒報到的年份照舊保留
#[test]
fn year_keys_are_not_a_list() {
    let mut world = world_with(
        &[("年度.2025", "平"), ("年度.2026", "丰")],
        mechanism_with(&[]),
    );
    apply(&mut world, &status("年度:\n  2026: 歉\n"));
    assert_eq!(
        leaves(&world, "年度"),
        pairs(&[("年度.2025", "平"), ("年度.2026", "歉")])
    );
}

#[test]
fn empty_list_leaves_an_empty_branch() {
    let mut world = world_with(&[], mechanism_with(&[]));
    apply(&mut world, &status(THREE));
    apply(&mut world, &status("人物列表: []\n"));
    assert_eq!(
        world.state.tree.get("人物列表"),
        Some(&StateNode::Branch(BTreeMap::new()))
    );
}

#[test]
fn root_list_drops_the_same_named_table_field() {
    let mut world = world_with(&[], mechanism_with(&[]));
    world
        .state
        .table
        .insert("人物列表".to_owned(), "林远、贺铮".to_owned());
    apply(&mut world, &status(TWO));
    assert!(!world.state.table.contains_key("人物列表"));
    assert_eq!(leaves(&world, "人物列表").len(), 2);
}

/// 清單底下的舊值不拿來比跳動（換人後同索引的數字不相干）；清單外照舊比
#[test]
fn list_items_do_not_report_jumps() {
    let mut world = world_with(
        &[("名單.0.体力", "10"), ("名單.1.体力", "12")],
        mechanism_with(&[]),
    );
    world.state.table.insert("糧草".to_owned(), "10".to_owned());
    let outcome = apply(
        &mut world,
        &status("名單:\n  - 体力: 100\n  - 体力: 90\n糧草: 100\n"),
    );
    let jumps: Vec<&str> = outcome
        .records
        .iter()
        .filter(|record| record.kind == RecordKind::Jump)
        .map(|record| record.path.as_str())
        .collect();
    assert_eq!(jumps, vec!["糧草"]);
}

#[test]
fn incremental_and_non_takeover_refactor_tables_replace_lists_too() {
    for (incremental, refactor_mode) in [(true, None), (false, Some("characters"))] {
        let mut mechanism = mechanism_with(&[]);
        mechanism.incremental = incremental;
        let mut world = world_with(&[], mechanism);
        world.refactor_mode = refactor_mode.map(str::to_owned);
        apply(&mut world, &status(THREE));
        apply(&mut world, &status(TWO));
        assert_eq!(
            leaves(&world, "人物列表"),
            pairs(&[
                ("人物列表.0.人物.名字", "林远"),
                ("人物列表.1.人物.名字", "周掌柜")
            ]),
            "{incremental} {refactor_mode:?}"
        );
    }
}

/// 同一回覆 YAML 清單＋JSONPatch：狀態區塊先寫，patch 後套（同支照索引改、別支各自生效）
#[test]
fn yaml_list_then_json_patch_final_result() {
    let mut world = world_with(&[("地点", "前院")], mechanism_with(&[]));
    let reply = format!(
        "{}\n<UpdateVariable><JSONPatch>[{{\"op\":\"replace\",\"path\":\"/人物列表/1/人物/名字\",\"value\":\"贺铮\"}},{{\"op\":\"replace\",\"path\":\"/地点\",\"value\":\"马厩\"}}]</JSONPatch></UpdateVariable>",
        status(TWO)
    );
    apply(&mut world, &reply);
    assert_eq!(
        leaves(&world, "人物列表"),
        pairs(&[
            ("人物列表.0.人物.名字", "林远"),
            ("人物列表.1.人物.名字", "贺铮")
        ])
    );
    assert_eq!(leaves(&world, "地点"), pairs(&[("地点", "马厩")]));
}

/// 接管桌：只有清單（`[]`）的狀態區塊也算有狀態區塊——回覆記拒收、開場記略過，樹不動
#[test]
fn takeover_scopes_drop_list_only_blocks() {
    for (scope, kind) in [
        (BlockScope::TakeoverReply, RecordKind::Rejected),
        (BlockScope::TakeoverOpening, RecordKind::Ignored),
    ] {
        let mut mechanism = mechanism_with(&[]);
        mechanism.incremental = true;
        let mut world = world_with(&[("名單.0", "林远")], mechanism);
        let block = extract_state_block(&status("名單: []\n"));
        assert!(block.fields.is_empty() && !block.lists.is_empty());
        let outcome = apply_block_typed(&mut world, &block, "阿濤", None, scope);
        assert!(
            outcome.records.iter().any(|record| record.kind == kind),
            "{scope:?}"
        );
        assert_eq!(
            leaves(&world, "名單"),
            pairs(&[("名單.0", "林远")]),
            "{scope:?}"
        );
    }
}

/// 同一回覆前塊報兩個 hp（清單）、後塊報一個（具名形）：樹上只剩 `L/hp`
#[test]
fn later_block_map_shape_replaces_the_earlier_list() {
    let mut world = world_with(&[], mechanism_with(&[]));
    apply(
        &mut world,
        "旁白<Status_block>L:\n  - hp: 1\n  - hp: 2\n</Status_block>\n```state\nL:\n  - hp: 3\n```",
    );
    assert_eq!(leaves(&world, "L"), pairs(&[("L.hp", "3")]));
}

/// 帶引號的 `"[]"` 是字串：不當清單、不留空分支
#[test]
fn quoted_brackets_are_a_string_leaf() {
    let mut world = world_with(&[("状态.名單.0", "林远")], mechanism_with(&[]));
    apply(&mut world, &status("状态:\n  名單: \"[]\"\n"));
    assert_eq!(leaves(&world, "状态"), pairs(&[("状态.名單", "[]")]));
}

/// 在場人物空清單：平欄寫空值，不進樹、不沿用舊名單
#[test]
fn empty_present_list_clears_the_table_field() {
    let mut world = world_with(&[], mechanism_with(&[]));
    world
        .state
        .table
        .insert("present".to_owned(), "林远、贺铮".to_owned());
    apply(&mut world, &status("在场人物: []\n"));
    assert_eq!(
        world.state.table.get("present").map(String::as_str),
        Some("")
    );
    assert!(world.state.tree.is_empty());
}

/// 鍵值 `|` 底下沒有續行：既有值不被清掉
#[test]
fn empty_block_scalar_keeps_the_existing_value() {
    let mut world = world_with(&[("状态.描述", "旧描述")], mechanism_with(&[]));
    apply(&mut world, &status("状态:\n  描述: |\n  地点: 前院\n"));
    assert_eq!(
        leaves(&world, "状态"),
        pairs(&[("状态.地点", "前院"), ("状态.描述", "旧描述")])
    );
}
