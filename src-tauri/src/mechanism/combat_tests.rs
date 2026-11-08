//! ttrpg-combat-tracker 包 1：戰鬥欄的種入、HP 四入口驗證、綁骰池、敵人整筆、先攻連帶清理、拒收標記、淨值標記。
use super::test_support::{mechanism_with, rule, world_with};
use super::{apply_block, Outcome, RecordKind};
use crate::data::default_panel::combat::{in_order, settle};
use crate::data::default_panel::{add_field, sync, PanelCard};
use crate::data::{self, FieldKind, Mechanism, RefusedMark, StateNode, WorldState};
use crate::transport::extract_state_block;

const POOL: &str = "d4:[1,3]\nd6:[4,2,6,1,5,3,2,2,6,4,1,5]\nd8:[5,2]\nd10:[7,1]\nd12:[9,4]\nd20:[19,3,11,8,2,20,14,6]\nd100:[42,7,88,15]";

fn cards() -> Vec<PanelCard> {
    ["沃恩", "雷克"]
        .iter()
        .map(|name| PanelCard {
            id: format!("id-{name}"),
            name: (*name).into(),
            fresh: true,
        })
        .collect()
}

fn combat_world() -> WorldState {
    let mut world = world_with(&[], Mechanism::default());
    world.dice_enabled = true;
    sync(&mut world, &cards());
    world.mechanism.turn_dice = Some(POOL.to_owned());
    world
}

fn segments(path: &str) -> Vec<String> {
    path.split('.').map(str::to_owned).collect()
}

fn leaf(world: &WorldState, path: &str) -> String {
    match data::node_at(&world.state.tree, &segments(path)) {
        Some(StateNode::Leaf(value)) => value.clone(),
        other => panic!("{path}: {other:?}"),
    }
}

fn set(world: &mut WorldState, path: &str, value: &str) {
    data::set_tree_value(&mut world.state.tree, &segments(path), value);
}

fn patch(world: &mut WorldState, ops: &str) -> Outcome {
    patches(world, &[ops])
}

/// 一則回覆裡的多個更新區塊
fn patches(world: &mut WorldState, blocks: &[&str]) -> Outcome {
    let body: String = blocks
        .iter()
        .map(|ops| {
            format!("\n<UpdateVariable>\n<JSONPatch>\n[{ops}]\n</JSONPatch>\n</UpdateVariable>")
        })
        .collect();
    apply_block(
        world,
        &extract_state_block(&format!("旁白。{body}")),
        "沃恩",
    )
}

fn rejected(outcome: &Outcome) -> Vec<String> {
    outcome
        .records
        .iter()
        .filter(|record| record.kind == RecordKind::Rejected)
        .map(|record| record.path.clone())
        .collect()
}

#[test]
fn combat_fields_seed_only_with_dice_and_stay() {
    let mut world = world_with(&[], Mechanism::default());
    sync(&mut world, &cards());
    assert!(!world.mechanism.combat_panel, "沒開擲骰不種");
    assert!(data::node_at(&world.state.tree, &segments("combat")).is_none());
    world.dice_enabled = true;
    assert!(sync(&mut world, &cards()));
    assert!(world.mechanism.combat_panel);
    assert_eq!(leaf(&world, "characters.沃恩.hp"), "");
    assert_eq!(leaf(&world, "combat.turn"), "");
    assert!(!sync(&mut world, &cards()), "沒變動不寫");
    world.dice_enabled = false;
    assert!(!sync(&mut world, &cards()), "關擲骰不收回");
    assert_eq!(leaf(&world, "characters.雷克.hp"), "");
}

#[test]
fn custom_field_named_hp_keeps_value_and_rule() {
    let mut world = world_with(&[], Mechanism::default());
    sync(&mut world, &cards());
    // 舊桌：保留名生效前玩家就用「＋欄位」加了 hp（精確文字規則）
    let mut custom = world.mechanism.rules["characters.*.status"].clone();
    custom.max_chars = Some(60);
    world
        .mechanism
        .rules
        .insert("characters.沃恩.hp".into(), custom);
    set(&mut world, "characters.沃恩.hp", "精神很好");
    world.dice_enabled = true;
    sync(&mut world, &cards());
    assert_eq!(leaf(&world, "characters.沃恩.hp"), "精神很好");
    assert_eq!(
        world.mechanism.rules["characters.沃恩.hp"].kind,
        FieldKind::Text
    );
    assert!(!sync(&mut world, &cards()), "沒變動不寫");
    let outcome = patch(
        &mut world,
        r#"{"op":"replace","path":"/characters/沃恩/hp","value":"有點累"}"#,
    );
    assert!(rejected(&outcome).is_empty(), "{:?}", outcome.records);
    assert_eq!(leaf(&world, "characters.沃恩.hp"), "有點累", "照文字欄運作");
    assert!(
        !add_field(&mut world, &segments("characters.雷克"), "conditions"),
        "新桌保留名"
    );
}

#[test]
fn hp_initialization_accepts_only_valid_whole_numbers() {
    let mut world = combat_world();
    for bad in [
        "2.5/10", "-1/10", "5/0", "11/10", "1/100000", "NaN/10", "abc",
    ] {
        let outcome = patch(
            &mut world,
            &format!(r#"{{"op":"replace","path":"/characters/沃恩/hp","value":"{bad}"}}"#),
        );
        assert_eq!(rejected(&outcome), ["characters.沃恩.hp"], "{bad}");
        assert_eq!(leaf(&world, "characters.沃恩.hp"), "");
    }
    patch(
        &mut world,
        r#"{"op":"replace","path":"/characters/沃恩/hp","value":"24/24"}"#,
    );
    assert_eq!(leaf(&world, "characters.沃恩.hp"), "24/24");
}

#[test]
fn delta_clamps_and_marks_net_actual_change() {
    let mut world = combat_world();
    set(&mut world, "characters.雷克.hp", "3/10");
    let outcome = patch(
        &mut world,
        r#"{"op":"delta","path":"/characters/雷克/hp","value":-5},
           {"op":"delta","path":"/characters/雷克/hp","value":5}"#,
    );
    assert_eq!(leaf(&world, "characters.雷克.hp"), "5/10");
    assert_eq!(outcome.changes["characters.雷克.hp"], "+2");
    assert!(outcome
        .records
        .iter()
        .any(|record| record.kind == RecordKind::Clamped));
    let outcome = patch(
        &mut world,
        r#"{"op":"delta","path":"/characters/雷克/hp","value":-2},
           {"op":"delta","path":"/characters/雷克/hp","value":-1.5},
           {"op":"delta","path":"/characters/雷克/hp","value":20}"#,
    );
    assert_eq!(leaf(&world, "characters.雷克.hp"), "10/10");
    assert_eq!(
        outcome.changes["characters.雷克.hp"], "+5",
        "拒收的那筆不算"
    );
}

#[test]
fn illegal_current_values_refuse_safely() {
    let mut world = combat_world();
    let outcome = patch(
        &mut world,
        r#"{"op":"delta","path":"/characters/沃恩/hp","value":-3}"#,
    );
    assert_eq!(rejected(&outcome), ["characters.沃恩.hp"], "還沒有 HP");
    for current in ["abc", "5/-3", "12/10", "1.5/3"] {
        set(&mut world, "characters.沃恩.hp", current);
        let outcome = patch(
            &mut world,
            r#"{"op":"delta","path":"/characters/沃恩/hp","value":-3}"#,
        );
        assert_eq!(rejected(&outcome), ["characters.沃恩.hp"], "{current}");
        assert_eq!(
            leaf(&world, "characters.沃恩.hp"),
            current,
            "手改的值原樣留著"
        );
    }
}

#[test]
fn max_change_requires_current_copied() {
    let mut world = combat_world();
    set(&mut world, "characters.沃恩.hp", "20/24");
    let outcome = patch(
        &mut world,
        r#"{"op":"replace","path":"/characters/沃恩/hp","value":"16/30"}"#,
    );
    assert_eq!(rejected(&outcome), ["characters.沃恩.hp"]);
    assert_eq!(
        leaf(&world, "characters.沃恩.hp"),
        "20/24",
        "現值不符整筆不套、上限也不改"
    );
    patch(
        &mut world,
        r#"{"op":"replace","path":"/characters/沃恩/hp","value":"20/30"}"#,
    );
    assert_eq!(leaf(&world, "characters.沃恩.hp"), "20/30");
}

#[test]
fn dice_total_from_pool_is_applied() {
    let mut world = combat_world();
    set(&mut world, "characters.雷克.hp", "20/20");
    // d8#1=5，+3 → 8
    let outcome = patch(
        &mut world,
        r#"{"op":"delta","path":"/characters/雷克/hp","value":-8,"dice":"d8#1+3"}"#,
    );
    assert!(rejected(&outcome).is_empty(), "{:?}", outcome.records);
    assert_eq!(leaf(&world, "characters.雷克.hp"), "12/20");
    // 報 -5 但骰面 d6#1=4 +3 → 7：依骰面套 -7，回饋＋標記
    let outcome = patch(
        &mut world,
        r#"{"op":"delta","path":"/characters/雷克/hp","value":-5,"dice":"d6#1+3"}"#,
    );
    assert_eq!(leaf(&world, "characters.雷克.hp"), "5/20");
    assert!(
        outcome
            .notes
            .iter()
            .any(|note| note.contains("依骰面套 -7")),
        "{:?}",
        outcome.notes
    );
    assert_eq!(
        world.state.refused["characters.雷克.hp"],
        RefusedMark {
            wrote: Some("-5".into()),
            applied: Some("-7".into())
        }
    );
    // 治療取正號
    patch(
        &mut world,
        r#"{"op":"delta","path":"/characters/雷克/hp","value":1,"dice":"d4#2"}"#,
    );
    assert_eq!(leaf(&world, "characters.雷克.hp"), "8/20");
}

#[test]
fn dice_edge_cases_refuse_and_leave_tree() {
    let mut world = combat_world();
    set(&mut world, "characters.雷克.hp", "20/20");
    for (dice, value) in [
        ("d8#3", -5),      // 池裡沒有
        ("d7#1", -5),      // 沒這種骰
        ("d4#1-3", -2),    // 1-3 = -2：負總和
        ("d6#2", 0),       // 方向不明
        ("d6#1+d6#1", -8), // 同一個 dice 裡重複
        ("2d6", -7),       // 格式不對
    ] {
        let outcome = patch(
            &mut world,
            &format!(
                r#"{{"op":"delta","path":"/characters/雷克/hp","value":{value},"dice":"{dice}"}}"#
            ),
        );
        assert_eq!(rejected(&outcome), ["characters.雷克.hp"], "{dice}");
        assert_eq!(leaf(&world, "characters.雷克.hp"), "20/20", "{dice}");
    }
    // 本則跨區塊重用同一顆
    let outcome = patches(
        &mut world,
        &[
            r#"{"op":"delta","path":"/characters/雷克/hp","value":-5,"dice":"d8#1"}"#,
            r#"{"op":"delta","path":"/characters/沃恩/hp","value":-5,"dice":"d8#1"}"#,
        ],
    );
    assert_eq!(leaf(&world, "characters.雷克.hp"), "15/20");
    assert!(rejected(&outcome).contains(&"characters.沃恩.hp".to_owned()));
    // 沒有池
    world.mechanism.turn_dice = None;
    let outcome = patch(
        &mut world,
        r#"{"op":"delta","path":"/characters/雷克/hp","value":-5,"dice":"d8#1"}"#,
    );
    assert_eq!(rejected(&outcome), ["characters.雷克.hp"]);
    // 不帶 dice 照收
    patch(
        &mut world,
        r#"{"op":"delta","path":"/characters/雷克/hp","value":-6}"#,
    );
    assert_eq!(leaf(&world, "characters.雷克.hp"), "9/20");
}

#[test]
fn foe_insert_only_adds_new_names() {
    let mut world = combat_world();
    patch(
        &mut world,
        r#"{"op":"replace","path":"/combat/order","value":["沃恩","哥布林A"]}"#,
    );
    let outcome = patch(
        &mut world,
        r#"{"op":"insert","path":"/combat/foes/哥布林A","value":{"hp":"7/7","conditions":["中毒"],"ac":15}}"#,
    );
    assert!(rejected(&outcome).is_empty(), "{:?}", outcome.records);
    assert_eq!(leaf(&world, "combat.foes.哥布林A.hp"), "7/7");
    assert_eq!(leaf(&world, "combat.foes.哥布林A.conditions.0"), "中毒");
    assert!(data::node_at(&world.state.tree, &segments("combat.foes.哥布林A.ac")).is_none());
    patch(
        &mut world,
        r#"{"op":"delta","path":"/combat/foes/哥布林A/hp","value":-4}"#,
    );
    let outcome = patch(
        &mut world,
        r#"{"op":"insert","path":"/combat/foes/哥布林A","value":{"hp":"7/7"}}"#,
    );
    assert_eq!(rejected(&outcome), ["combat.foes.哥布林A"]);
    assert_eq!(
        leaf(&world, "combat.foes.哥布林A.hp"),
        "3/7",
        "同名 insert 不蓋掉扣過的 HP"
    );
    for bad in [r#"{"hp":"7.5/7"}"#, r#"{"conditions":[]}"#, r#""7/7""#] {
        let outcome = patch(
            &mut world,
            &format!(r#"{{"op":"insert","path":"/combat/foes/哥布林B","value":{bad}}}"#),
        );
        assert_eq!(rejected(&outcome), ["combat.foes.哥布林B"], "{bad}");
    }
    for n in 0..7 {
        patch(
            &mut world,
            &format!(r#"{{"op":"insert","path":"/combat/foes/狼{n}","value":{{"hp":"5/5"}}}}"#),
        );
    }
    let outcome = patch(
        &mut world,
        r#"{"op":"insert","path":"/combat/foes/狼9","value":{"hp":"5/5"}}"#,
    );
    assert_eq!(rejected(&outcome), ["combat.foes.狼9"], "最多 8 筆");
    let outcome = patch(
        &mut world,
        r#"{"op":"replace","path":"/combat/foes/哥布林A","value":{"hp":"7/7"}},
           {"op":"replace","path":"/combat/foes","value":{}}"#,
    );
    assert_eq!(rejected(&outcome).len(), 2);
    patch(
        &mut world,
        r#"{"op":"remove","path":"/combat/foes/哥布林A"}"#,
    );
    assert!(data::node_at(&world.state.tree, &segments("combat.foes.哥布林A")).is_none());
}

#[test]
fn turn_must_be_in_order_and_empty_order_ends_combat() {
    let mut world = combat_world();
    patch(
        &mut world,
        r#"{"op":"replace","path":"/combat/order","value":["沃恩","哥布林A","雷克","沃恩"]},
           {"op":"insert","path":"/combat/foes/哥布林A","value":{"hp":"7/7"}},
           {"op":"replace","path":"/combat/turn","value":"雷克"}"#,
    );
    assert_eq!(leaf(&world, "combat.order.2"), "雷克");
    assert!(
        data::node_at(&world.state.tree, &segments("combat.order.3")).is_none(),
        "去重"
    );
    assert_eq!(leaf(&world, "combat.turn"), "雷克");
    let outcome = patch(
        &mut world,
        r#"{"op":"replace","path":"/combat/turn","value":"巨魔"}"#,
    );
    assert_eq!(rejected(&outcome), ["combat.turn"]);
    assert_eq!(leaf(&world, "combat.turn"), "雷克");
    // 順序換掉、輪到的人不在新順序 → 清空
    patch(
        &mut world,
        r#"{"op":"replace","path":"/combat/order","value":["沃恩","哥布林A"]}"#,
    );
    assert_eq!(leaf(&world, "combat.turn"), "");
    patch(
        &mut world,
        r#"{"op":"replace","path":"/combat/turn","value":"沃恩"}"#,
    );
    patch(
        &mut world,
        r#"{"op":"replace","path":"/combat/order","value":[]}"#,
    );
    assert_eq!(leaf(&world, "combat.turn"), "");
    assert_eq!(
        data::node_at(&world.state.tree, &segments("combat.foes")),
        Some(&StateNode::Branch(Default::default()))
    );
    assert_eq!(leaf(&world, "characters.沃恩.hp"), "", "角色欄不受收場影響");
}

#[test]
fn settle_and_membership_for_hand_edits() {
    let mut world = combat_world();
    set(&mut world, "combat.order.0", "沃恩");
    set(&mut world, "combat.turn", "雷克");
    assert!(!in_order(&world.state.tree, "雷克"));
    assert!(in_order(&world.state.tree, "沃恩"));
    assert!(settle(&mut world.state.tree));
    assert_eq!(leaf(&world, "combat.turn"), "");
    assert!(!settle(&mut world.state.tree), "清完沒有變動");
}

#[test]
fn fence_cannot_write_hp_or_foes() {
    let mut world = combat_world();
    let outcome = apply_block(
        &mut world,
        &extract_state_block(
            "旁白。\n```state\ncharacters:\n  沃恩:\n    hp: 99/99\ncombat:\n  foes:\n    巨魔:\n      hp: 40/40\n  turn: 沃恩\n```",
        ),
        "沃恩",
    );
    let paths = rejected(&outcome);
    assert!(
        paths.iter().any(|path| path == "characters.沃恩.hp"),
        "{paths:?}"
    );
    assert!(
        paths.iter().any(|path| path.starts_with("combat.foes")),
        "{paths:?}"
    );
    assert_eq!(leaf(&world, "characters.沃恩.hp"), "");
    assert!(data::node_at(&world.state.tree, &segments("combat.foes.巨魔")).is_none());
}

#[test]
fn refused_marks_cover_combat_fields_only_and_recompute() {
    let mut world = combat_world();
    patch(
        &mut world,
        r#"{"op":"replace","path":"/characters/沃恩/hp","value":"9.5/10"},
           {"op":"replace","path":"/tracking/clues","value":"一條線索"}"#,
    );
    assert_eq!(
        world.state.refused.keys().collect::<Vec<_>>(),
        ["characters.沃恩.hp"]
    );
    assert_eq!(
        world.state.refused["characters.沃恩.hp"].wrote.as_deref(),
        Some("9.5/10")
    );
    patch(
        &mut world,
        r#"{"op":"replace","path":"/characters/沃恩/hp","value":"10/10"}"#,
    );
    assert!(world.state.refused.is_empty(), "下一則重算");
}

#[test]
fn net_mark_accumulates_on_other_table_kinds() {
    // 卡格式桌的數字欄與「現值/上限」欄：夾到界線後反向增減，標記是實際淨值
    let mut world = world_with(
        &[("金幣", "3"), ("體力", "3/10")],
        mechanism_with(&[
            ("金幣", rule(FieldKind::Number, Some(0.0), Some(10.0))),
            ("體力", rule(FieldKind::Pair, None, None)),
        ]),
    );
    let outcome = patch(
        &mut world,
        r#"{"op":"delta","path":"/金幣","value":-5},{"op":"delta","path":"/金幣","value":5},
           {"op":"delta","path":"/體力","value":-5},{"op":"delta","path":"/體力","value":5}"#,
    );
    assert_eq!(leaf(&world, "金幣"), "5");
    assert_eq!(outcome.changes["金幣"], "+2");
    assert_eq!(leaf(&world, "體力"), "5/10");
    assert_eq!(outcome.changes["體力"], "+2");
}

#[test]
fn foes_need_order_and_whole_insert() {
    let mut world = combat_world();
    let outcome = patch(
        &mut world,
        r#"{"op":"insert","path":"/combat/foes/哥布林A","value":{"hp":"7/7"}}"#,
    );
    assert_eq!(
        rejected(&outcome),
        ["combat.foes.哥布林A"],
        "沒先攻不收敵人"
    );
    assert!(outcome.notes.iter().any(|note| note.contains("先攻順序")));
    patch(
        &mut world,
        r#"{"op":"replace","path":"/combat/order","value":["沃恩","哥布林A"]}"#,
    );
    let outcome = patch(
        &mut world,
        r#"{"op":"insert","path":"/combat/foes/哥布林A/conditions","value":["中毒"]},
           {"op":"replace","path":"/combat/foes/哥布林A/hp","value":"7/7"}"#,
    );
    assert_eq!(
        rejected(&outcome),
        ["combat.foes.哥布林A.conditions", "combat.foes.哥布林A.hp"]
    );
    assert!(data::node_at(&world.state.tree, &segments("combat.foes.哥布林A")).is_none());
    let outcome = patch(
        &mut world,
        r#"{"op":"insert","path":"/combat/foes/哥布林A","value":{"hp":"7/7"}}"#,
    );
    assert!(rejected(&outcome).is_empty(), "{:?}", outcome.records);
    patch(
        &mut world,
        r#"{"op":"replace","path":"/combat/foes/哥布林A/conditions","value":["中毒"]}"#,
    );
    assert_eq!(
        leaf(&world, "combat.foes.哥布林A.conditions.0"),
        "中毒",
        "已存在的敵人子欄照收"
    );
}
