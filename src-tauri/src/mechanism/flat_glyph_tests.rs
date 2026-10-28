//! flat-field-glyph-dup：狀態圍欄寫平欄時，模型換一種字形（簡繁）寫既有鍵不該長出第二份。

use super::ledger::apply_block;
use super::test_support::{fence, mechanism_with, rule, world_with};
use super::types::{Outcome, RecordKind};
use crate::data::{self, FieldKind, StateNode};
use crate::transport::StateBlock;

fn world_with_table(pairs: &[(&str, &str)], incremental: bool) -> data::WorldState {
    let mut mechanism = mechanism_with(&[]);
    mechanism.incremental = incremental;
    let mut world = world_with(&[], mechanism);
    for (key, value) in pairs {
        world
            .state
            .table
            .insert((*key).to_owned(), (*value).to_owned());
    }
    world
}

fn table(world: &data::WorldState) -> Vec<(&str, &str)> {
    world
        .state
        .table
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect()
}

fn rejected(outcome: &Outcome) -> usize {
    outcome
        .records
        .iter()
        .filter(|record| record.kind == RecordKind::Rejected)
        .count()
}

/// 非增量桌：開場白用簡體鍵、GM 用繁體寫同一欄，寫回原鍵，不長第二份。
#[test]
fn non_incremental_fence_folds_onto_the_existing_flat_key() {
    let mut world = world_with_table(&[("剧情阶段", "序章"), ("place", "城门")], false);
    let outcome = apply_block(
        &mut world,
        &fence(&[("劇情階段", "第一章"), ("place", "食堂")]),
        "玩家",
    );
    assert_eq!(rejected(&outcome), 0, "{:?}", outcome.records);
    assert_eq!(
        table(&world),
        vec![("place", "食堂"), ("剧情阶段", "第一章")]
    );
}

/// 增量桌但鍵不在樹上（照舊走平欄）：同樣折回平欄原鍵。
#[test]
fn incremental_fence_off_tree_folds_onto_the_existing_flat_key() {
    let mut world = world_with_table(&[("剧情阶段", "序章")], true);
    let outcome = apply_block(&mut world, &fence(&[("劇情階段", "第一章")]), "玩家");
    assert_eq!(rejected(&outcome), 0, "{:?}", outcome.records);
    assert_eq!(table(&world), vec![("剧情阶段", "第一章")]);
}

/// 精確優先：平欄已有兩個只差字形的鍵，照原字寫哪個就改哪個，不併、不拒。
#[test]
fn exact_flat_key_wins_over_glyph_variants() {
    let mut world = world_with_table(&[("沈郁", "甲"), ("沈鬱", "乙")], false);
    let outcome = apply_block(&mut world, &fence(&[("沈鬱", "丙")]), "玩家");
    assert_eq!(rejected(&outcome), 0, "{:?}", outcome.records);
    assert_eq!(table(&world), vec![("沈郁", "甲"), ("沈鬱", "丙")]);
}

/// 歧義：第三種字形同時對到兩個既有鍵＝拒收並回饋，平欄不動。
#[test]
fn ambiguous_glyph_fold_on_flat_keys_is_rejected() {
    let mut world = world_with_table(&[("沈郁", "甲"), ("沈鬱", "乙")], false);
    let outcome = apply_block(&mut world, &fence(&[("瀋鬱", "丙")]), "玩家");
    assert_eq!(rejected(&outcome), 1, "{:?}", outcome.records);
    assert_eq!(table(&world), vec![("沈郁", "甲"), ("沈鬱", "乙")]);
}

/// 對不到任何既有鍵的新欄位照原字新增。
#[test]
fn unmatched_flat_key_is_inserted_verbatim() {
    let mut world = world_with_table(&[("剧情阶段", "序章")], false);
    let outcome = apply_block(&mut world, &fence(&[("天氣", "小雪")]), "玩家");
    assert_eq!(rejected(&outcome), 0, "{:?}", outcome.records);
    assert_eq!(table(&world), vec![("剧情阶段", "序章"), ("天氣", "小雪")]);
}

/// 數字欄換字形：跳動比對拿表上原鍵的舊值，面板記號也落在原鍵。
#[test]
fn glyph_folded_numeric_field_still_reports_jumps_on_the_existing_key() {
    let mut world = world_with_table(&[("体力", "10")], false);
    let outcome = apply_block(&mut world, &fence(&[("體力", "100")]), "玩家");
    let jumps = outcome
        .records
        .iter()
        .filter(|record| record.kind == RecordKind::Jump)
        .count();
    assert_eq!(jumps, 1, "{:?}", outcome.records);
    assert_eq!(table(&world), vec![("体力", "100")]);
    assert!(
        world.state.jumps.contains_key("体力"),
        "{:?}",
        world.state.jumps
    );
}

// ---------------------------------------------------------------------
// 非增量桌的多段路徑（`角色.当前环境`）：樹上同樣不該因換字形長出第二枝
// ---------------------------------------------------------------------

/// 狀態區塊的多段欄位，路徑以「.」分段。
fn nested(fields: &[(&str, &str)]) -> StateBlock {
    let mut block = fence(&[]);
    block.fields = fields
        .iter()
        .map(|(path, value)| {
            (
                path.split('.').map(str::to_owned).collect(),
                (*value).to_owned(),
            )
        })
        .collect();
    block
}

fn tree_leaves(world: &data::WorldState) -> Vec<(String, String)> {
    fn walk(
        branch: &std::collections::BTreeMap<String, StateNode>,
        prefix: &str,
        out: &mut Vec<(String, String)>,
    ) {
        for (key, node) in branch {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            match node {
                StateNode::Leaf(value) => out.push((path, value.clone())),
                StateNode::Branch(child) => walk(child, &path, out),
            }
        }
    }
    let mut out = Vec::new();
    walk(&world.state.tree, "", &mut out);
    out
}

fn owned(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

/// 枝名與葉名都換字形：寫回既有路徑，不另長「狀態」枝。
#[test]
fn non_incremental_nested_path_folds_onto_the_existing_branch_and_leaf() {
    let mut world = world_with(&[("状态.当前环境", "城门")], mechanism_with(&[]));
    let outcome = apply_block(&mut world, &nested(&[("狀態.當前環境", "食堂")]), "玩家");
    assert_eq!(rejected(&outcome), 0, "{:?}", outcome.records);
    assert_eq!(tree_leaves(&world), owned(&[("状态.当前环境", "食堂")]));
}

/// 同層兩個只差字形的葉子、模型寫第三種字形＝拒收，樹不動。
#[test]
fn ambiguous_nested_glyph_fold_is_rejected() {
    let mut world = world_with(
        &[("角色.沈郁", "甲"), ("角色.沈鬱", "乙")],
        mechanism_with(&[]),
    );
    let outcome = apply_block(&mut world, &nested(&[("角色.瀋鬱", "丙")]), "玩家");
    assert_eq!(rejected(&outcome), 1, "{:?}", outcome.records);
    assert_eq!(
        tree_leaves(&world),
        owned(&[("角色.沈郁", "甲"), ("角色.沈鬱", "乙")])
    );
}

/// 守門：精確路徑優先，同層有字形變體也只改照原字寫的那個。
#[test]
fn exact_nested_path_wins_over_glyph_variants() {
    let mut world = world_with(
        &[("角色.沈郁", "甲"), ("角色.沈鬱", "乙")],
        mechanism_with(&[]),
    );
    let outcome = apply_block(&mut world, &nested(&[("角色.沈鬱", "丙")]), "玩家");
    assert_eq!(rejected(&outcome), 0, "{:?}", outcome.records);
    assert_eq!(
        tree_leaves(&world),
        owned(&[("角色.沈郁", "甲"), ("角色.沈鬱", "丙")])
    );
}

/// 守門：對不到既有葉子的新欄位照原字長在既有枝下。
#[test]
fn unmatched_nested_leaf_is_inserted_verbatim() {
    let mut world = world_with(&[("角色.沈郁", "甲")], mechanism_with(&[]));
    let outcome = apply_block(&mut world, &nested(&[("角色.天氣", "小雪")]), "玩家");
    assert_eq!(rejected(&outcome), 0, "{:?}", outcome.records);
    assert_eq!(
        tree_leaves(&world),
        owned(&[("角色.天氣", "小雪"), ("角色.沈郁", "甲")])
    );
}

/// 數字葉子換字形：跳動比對拿既有路徑的舊值，記號落在既有路徑。
#[test]
fn glyph_folded_nested_numeric_leaf_still_reports_jumps_on_the_existing_path() {
    let mut world = world_with(&[("状态.体力", "10")], mechanism_with(&[]));
    let outcome = apply_block(&mut world, &nested(&[("狀態.體力", "100")]), "玩家");
    let jumps = outcome
        .records
        .iter()
        .filter(|record| record.kind == RecordKind::Jump)
        .count();
    assert_eq!(jumps, 1, "{:?}", outcome.records);
    assert_eq!(tree_leaves(&world), owned(&[("状态.体力", "100")]));
    assert!(
        world.state.jumps.contains_key("状态.体力"),
        "{:?}",
        world.state.jumps
    );
}

fn segments(path: &str) -> Vec<String> {
    path.split('.').map(str::to_owned).collect()
}

fn jump_count(outcome: &Outcome) -> usize {
    outcome
        .records
        .iter()
        .filter(|record| record.kind == RecordKind::Jump)
        .count()
}

/// 計數器欄換字形：對回原鍵後照計數器規則，不報跳動。
#[test]
fn glyph_folded_counter_field_does_not_report_jumps() {
    let mut world = world_with_table(&[("驻守天数", "1")], false);
    world.mechanism = mechanism_with(&[("驻守天数", rule(FieldKind::Counter, Some(0.0), None))]);
    let outcome = apply_block(&mut world, &fence(&[("駐守天數", "100")]), "玩家");
    assert_eq!(jump_count(&outcome), 0, "{:?}", outcome.records);
    assert_eq!(table(&world), vec![("驻守天数", "100")]);
    assert!(world.state.jumps.is_empty(), "{:?}", world.state.jumps);
}

/// 增量桌的多段路徑同樣對回既有枝葉。
#[test]
fn incremental_nested_path_folds_onto_the_existing_branch_and_leaf() {
    let mut mechanism = mechanism_with(&[]);
    mechanism.incremental = true;
    let mut world = world_with(&[("状态.当前环境", "城门")], mechanism);
    let outcome = apply_block(&mut world, &nested(&[("狀態.當前環境", "食堂")]), "玩家");
    assert_eq!(rejected(&outcome), 0, "{:?}", outcome.records);
    assert_eq!(tree_leaves(&world), owned(&[("状态.当前环境", "食堂")]));
}

/// 只有祖先換字形、末段是新鍵：長在既有祖先底下，新鍵照原字。
#[test]
fn new_suffix_lands_under_the_glyph_folded_ancestor() {
    let mut world = world_with(&[("状态.当前环境", "城门")], mechanism_with(&[]));
    let outcome = apply_block(&mut world, &nested(&[("狀態.天氣", "小雪")]), "玩家");
    assert_eq!(rejected(&outcome), 0, "{:?}", outcome.records);
    assert_eq!(
        tree_leaves(&world),
        owned(&[("状态.天氣", "小雪"), ("状态.当前环境", "城门")])
    );
}

/// 祖先歧義：整條拒收，樹不動。
#[test]
fn ambiguous_ancestor_is_rejected_without_writing() {
    let mut world = world_with(
        &[("沈郁.心情", "甲"), ("沈鬱.心情", "乙")],
        mechanism_with(&[]),
    );
    let before = world.state.tree.clone();
    let outcome = apply_block(&mut world, &nested(&[("瀋鬱.心情", "丙")]), "玩家");
    assert_eq!(rejected(&outcome), 1, "{:?}", outcome.records);
    assert_eq!(world.state.tree, before);
}

/// 清單容器換字形：替換既有那支，不另長「狀態.物品」。
#[test]
fn glyph_folded_list_replaces_the_existing_list() {
    let mut world = world_with(
        &[("状态.物品.0", "剑"), ("状态.物品.1", "盾")],
        mechanism_with(&[]),
    );
    let mut block = nested(&[("狀態.物品.0", "弓")]);
    block.lists = vec![segments("狀態.物品")];
    let outcome = apply_block(&mut world, &block, "玩家");
    assert_eq!(rejected(&outcome), 0, "{:?}", outcome.records);
    assert_eq!(tree_leaves(&world), owned(&[("状态.物品.0", "弓")]));
}

/// 空清單換字形：清空既有那支，留空分支。
#[test]
fn glyph_folded_empty_list_clears_the_existing_list() {
    let mut world = world_with(&[("状态.物品.0", "剑")], mechanism_with(&[]));
    let mut block = nested(&[]);
    block.lists = vec![segments("狀態.物品")];
    let outcome = apply_block(&mut world, &block, "玩家");
    assert_eq!(rejected(&outcome), 0, "{:?}", outcome.records);
    assert!(tree_leaves(&world).is_empty(), "{:?}", world.state.tree);
    assert!(
        matches!(
            data::node_at(&world.state.tree, &segments("状态.物品")),
            Some(StateNode::Branch(children)) if children.is_empty()
        ),
        "{:?}",
        world.state.tree
    );
    assert!(!world.state.tree.contains_key("狀態"));
}

/// 歧義容器：容器與底下欄位整支拒收，不先刪舊清單。
#[test]
fn ambiguous_list_container_is_rejected_without_writing() {
    let mut world = world_with(
        &[("沈郁.物品.0", "剑"), ("沈鬱.物品.0", "盾")],
        mechanism_with(&[]),
    );
    let before = world.state.tree.clone();
    let mut block = nested(&[("瀋鬱.物品.0", "弓")]);
    block.lists = vec![segments("瀋鬱.物品")];
    let outcome = apply_block(&mut world, &block, "玩家");
    assert_eq!(rejected(&outcome), 1, "{:?}", outcome.records);
    assert_eq!(world.state.tree, before);
}

/// 守門：巢狀寫法「剧情阶段 → 要塞事件」接回既有的單鍵「剧情阶段/要塞事件」，不另長枝。
#[test]
fn nested_write_rejoins_the_existing_slash_key() {
    let mut world = world_with(&[], mechanism_with(&[]));
    world.state.tree.insert(
        "剧情阶段/要塞事件".to_owned(),
        StateNode::Leaf("初".to_owned()),
    );
    let block = StateBlock {
        fields: vec![(
            vec!["剧情阶段".to_owned(), "要塞事件".to_owned()],
            "二".to_owned(),
        )],
        ..fence(&[])
    };
    let outcome = apply_block(&mut world, &block, "玩家");
    assert_eq!(rejected(&outcome), 0, "{:?}", outcome.records);
    assert_eq!(tree_leaves(&world), owned(&[("剧情阶段/要塞事件", "二")]));
}

/// 清單項目是物件：新項目整支照原字寫，不拿舊項目做字形比對（舊項目同時有 沈郁／沈鬱 也不算歧義）。
#[test]
fn object_items_inside_a_rewritten_list_keep_their_new_keys() {
    let mut world = world_with(
        &[("物品.0.沈郁", "甲"), ("物品.0.沈鬱", "乙")],
        mechanism_with(&[]),
    );
    let mut block = nested(&[("物品.0.瀋鬱", "新值")]);
    block.lists = vec![segments("物品")];
    block.maps = vec![segments("物品.0")];
    let outcome = apply_block(&mut world, &block, "玩家");
    assert_eq!(rejected(&outcome), 0, "{:?}", outcome.records);
    assert_eq!(tree_leaves(&world), owned(&[("物品.0.瀋鬱", "新值")]));
}

/// 兩段寫法接回單鍵「剧情阶段/要塞事件」的數字欄：舊值照樹上實際的鍵讀，跳動記號落在那個鍵。
#[test]
fn rejoined_slash_key_numeric_field_reports_jumps_from_the_tree() {
    let mut world = world_with(&[], mechanism_with(&[]));
    world.state.tree.insert(
        "剧情阶段/要塞事件".to_owned(),
        StateNode::Leaf("10".to_owned()),
    );
    let block = StateBlock {
        fields: vec![(
            vec!["剧情阶段".to_owned(), "要塞事件".to_owned()],
            "100".to_owned(),
        )],
        ..fence(&[])
    };
    let outcome = apply_block(&mut world, &block, "玩家");
    assert_eq!(jump_count(&outcome), 1, "{:?}", outcome.records);
    assert_eq!(tree_leaves(&world), owned(&[("剧情阶段/要塞事件", "100")]));
    assert!(
        world.state.jumps.contains_key("剧情阶段/要塞事件"),
        "{:?}",
        world.state.jumps
    );
}
