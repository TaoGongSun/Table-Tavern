//! table-status-panel 包 1：預設狀態欄的守門、硬預算（補丁與圍欄兩個入口）、清單項目注入繼承；卡格式桌結果不變。
use super::rules::rule_for;
use super::test_support::world_with;
use super::{apply_block, write_refusal, Outcome, RecordKind, Write};
use crate::data::default_panel::{sync, PanelCard};
use crate::data::{self, FieldKind, InjectLevel, Mechanism, StateNode, WorldState};
use crate::transport::extract_state_block;

fn panel_world() -> WorldState {
    let mut world = world_with(&[], Mechanism::default());
    sync(
        &mut world,
        &[PanelCard {
            id: "a".into(),
            name: "艾倫".into(),
            fresh: true,
        }],
    );
    world
}

fn segments(path: &str) -> Vec<String> {
    path.split('.').map(str::to_owned).collect()
}

fn node<'a>(world: &'a WorldState, path: &str) -> Option<&'a StateNode> {
    data::node_at(&world.state.tree, &segments(path))
}

fn list(world: &WorldState, path: &str) -> Vec<String> {
    let Some(StateNode::Branch(children)) = node(world, path) else {
        panic!("{path} 不是清單");
    };
    (0..children.len())
        .map(|index| match &children[&index.to_string()] {
            StateNode::Leaf(value) => value.clone(),
            other => panic!("{other:?}"),
        })
        .collect()
}

fn patch(world: &mut WorldState, ops: &str) -> Outcome {
    let reply =
        format!("旁白。\n<UpdateVariable>\n<JSONPatch>\n[{ops}]\n</JSONPatch>\n</UpdateVariable>");
    apply_block(world, &extract_state_block(&reply), "阿濤")
}

fn fence(world: &mut WorldState, yaml: &str) -> Outcome {
    let reply = format!("旁白。\n```state\n{yaml}```");
    apply_block(world, &extract_state_block(&reply), "阿濤")
}

fn kinds(outcome: &Outcome) -> Vec<(RecordKind, String)> {
    outcome
        .records
        .iter()
        .map(|record| (record.kind, record.path.clone()))
        .collect()
}

#[test]
fn patch_over_budget_is_trimmed_silently() {
    let mut world = panel_world();
    let long = "長".repeat(70);
    let item = "物".repeat(30);
    let items: Vec<String> = (0..10).map(|n| format!("\"道具{n}\"")).collect();
    let outcome = patch(
        &mut world,
        &format!(
            r#"{{"op":"replace","path":"/characters/艾倫/status","value":"{long}"}},
               {{"op":"replace","path":"/characters/艾倫/items","value":["{item}"]}},
               {{"op":"replace","path":"/tracking/clues","value":[{}]}}"#,
            items.join(",")
        ),
    );
    assert_eq!(
        node(&world, "characters.艾倫.status"),
        Some(&StateNode::Leaf("長".repeat(60)))
    );
    assert_eq!(list(&world, "characters.艾倫.items"), ["物".repeat(24)]);
    assert_eq!(list(&world, "tracking.clues").len(), 6);
    assert_eq!(list(&world, "tracking.clues")[5], "道具5");
    assert!(outcome
        .records
        .iter()
        .all(|r| r.kind == RecordKind::Trimmed));
    assert_eq!(outcome.records.len(), 3);
    assert!(outcome.notes.is_empty(), "截斷不回饋");
    assert!(world.state.notes.is_empty());
}

#[test]
fn per_item_writes_and_wrong_shapes_are_refused_with_feedback() {
    let mut world = panel_world();
    patch(
        &mut world,
        r#"{"op":"replace","path":"/tracking/clues","value":["腳印"]}"#,
    );
    let outcome = patch(
        &mut world,
        r#"{"op":"replace","path":"/tracking/clues/0","value":"血跡"},
           {"op":"insert","path":"/tracking/clues/-","value":"信"},
           {"op":"remove","path":"/tracking/clues"},
           {"op":"delta","path":"/tracking/goals","value":1},
           {"op":"replace","path":"/tracking/goals","value":["找人",{"誰":"兇手"}]},
           {"op":"replace","path":"/tracking/promises","value":"一句話"},
           {"op":"replace","path":"/characters/艾倫/status","value":{"心情":"差"}}"#,
    );
    assert_eq!(
        list(&world, "tracking.clues"),
        ["腳印"],
        "逐項與刪除都沒套用"
    );
    assert!(list(&world, "tracking.goals").is_empty());
    assert!(list(&world, "tracking.promises").is_empty());
    assert_eq!(
        node(&world, "characters.艾倫.status"),
        Some(&StateNode::Leaf(String::new()))
    );
    assert_eq!(outcome.records.len(), 7);
    assert!(outcome
        .records
        .iter()
        .all(|r| r.kind == RecordKind::Rejected));
    assert!(outcome
        .notes
        .iter()
        .any(|note| note.contains("整串字串陣列")));
}

#[test]
fn table_external_keys_cells_and_whole_branches_are_refused() {
    let mut world = panel_world();
    let before = world.state.tree.clone();
    let outcome = patch(
        &mut world,
        r#"{"op":"replace","path":"/characters/路人/status","value":"新格"},
           {"op":"insert","path":"/characters/艾倫/心情","value":"差"},
           {"op":"insert","path":"/tracking/伏筆","value":["x"]},
           {"op":"replace","path":"/characters","value":{}},
           {"op":"remove","path":"/characters/艾倫"},
           {"op":"move","from":"/characters/艾倫","path":"/characters/亞倫"},
           {"op":"replace","path":"/tracking","value":{"goals":[]}}"#,
    );
    assert_eq!(world.state.tree, before);
    assert_eq!(
        kinds(&outcome)
            .iter()
            .filter(|(kind, _)| *kind == RecordKind::Rejected)
            .count(),
        7
    );
    assert!(outcome
        .notes
        .iter()
        .any(|note| note.contains("角色格由系統建立")));
}

#[test]
fn fence_nested_fields_are_guarded_and_trimmed() {
    let mut world = panel_world();
    let long = "長".repeat(70);
    let items: String = (0..10).map(|n| format!("      - 物{n}\n")).collect();
    let outcome = fence(
        &mut world,
        &format!(
            "time: 黃昏\ncharacters:\n  艾倫:\n    status: {long}\n    items:\n{items}  路人:\n    status: 新格\ntracking:\n  clues:\n    - 腳印\n  goals:\n    - 名字: 兇手\n"
        ),
    );
    assert_eq!(world.state.table["time"], "黃昏");
    assert_eq!(
        node(&world, "characters.艾倫.status"),
        Some(&StateNode::Leaf("長".repeat(60)))
    );
    assert_eq!(list(&world, "characters.艾倫.items").len(), 8);
    assert_eq!(list(&world, "tracking.clues"), ["腳印"]);
    assert!(list(&world, "tracking.goals").is_empty(), "物件項整支丟");
    assert!(node(&world, "characters.路人").is_none(), "模型開不了格");
    let records = kinds(&outcome);
    assert!(records.contains(&(RecordKind::Trimmed, "characters.艾倫.status".into())));
    assert!(records.contains(&(RecordKind::Trimmed, "characters.艾倫.items".into())));
    assert!(records.contains(&(RecordKind::Rejected, "tracking.goals".into())));
    assert!(records
        .iter()
        .any(|(kind, path)| *kind == RecordKind::Rejected && path.starts_with("characters.路人")));
}

/// 新增項目後，非對齊回合的 GM 動態塊照樣看得到（清單項目繼承父規則的 inject turn）
#[test]
fn list_items_inherit_turn_injection_only_on_panel_tables() {
    let world = panel_world();
    let item = segments("tracking.clues.0");
    let inherited = rule_for(&world.mechanism, &item, Some("腳印"));
    assert_eq!(
        (inherited.kind, inherited.inject),
        (FieldKind::List, InjectLevel::Turn)
    );
    assert!(super::leaf_shown(&world.mechanism, &item, "腳印", false));
    assert!(!super::has_rule(&world.mechanism, &item), "寫入資格不繼承");
    assert!(!super::has_rule(
        &world.mechanism,
        &segments("tracking.clues.0.x")
    ));

    // 卡格式桌：同樣形狀的規則，項目照舊推定 Text／Snapshot
    let mut card = world.mechanism.clone();
    card.default_panel = false;
    card.panel_cells.clear();
    let plain = rule_for(&card, &item, Some("腳印"));
    assert_eq!(
        (plain.kind, plain.inject),
        (FieldKind::Text, InjectLevel::Snapshot)
    );
    assert!(!super::leaf_shown(&card, &item, "腳印", false));
}

/// 卡格式桌：rule_for／write_refusal 結果一字不變（同一組規則換到非預設桌比對）
#[test]
fn card_format_tables_keep_rule_for_and_write_refusal() {
    let panel = panel_world().mechanism;
    let mut card = panel.clone();
    card.default_panel = false;
    card.panel_cells.clear();
    let paths = [
        "characters.艾倫.status",
        "characters.路人.status",
        "characters.艾倫.items",
        "characters.艾倫.items.3",
        "tracking.clues.0",
        "tracking",
        "World.HP",
    ];
    for path in paths {
        let path = segments(path);
        for write in [Write::Replace("x"), Write::Delta, Write::Other] {
            assert_eq!(write_refusal(&card, &path, write), None, "{path:?}");
            assert_eq!(
                write_refusal(&card, &path, write),
                write_refusal(&panel, &path, write),
                "面板手改的守門兩種桌一致：{path:?}"
            );
        }
    }
    let mut card_world = world_with(&[("World.HP", "10")], card);
    let outcome = patch(
        &mut card_world,
        r#"{"op":"insert","path":"/characters/路人/status","value":"新格"},
           {"op":"replace","path":"/tracking/clues/0","value":"x"}"#,
    );
    assert!(outcome
        .records
        .iter()
        .all(|r| r.kind != RecordKind::Trimmed));
    assert_eq!(
        node(&card_world, "characters.路人.status"),
        Some(&StateNode::Leaf("新格".into())),
        "卡格式桌照舊收表外鍵"
    );
}

/// 面板整串寫清單：同一道守門與預算；非清單欄、表外路徑、卡格式桌拒收
#[test]
fn panel_list_write_is_guarded_and_trimmed() {
    let mut world = panel_world();
    let items: Vec<String> = (0..10)
        .map(|n| format!("線索{n}{}", "長".repeat(50)))
        .collect();
    let kept = super::panel_list(&world.mechanism, &segments("tracking.clues"), items).unwrap();
    assert_eq!(kept.len(), 6);
    assert!(kept.iter().all(|item| item.chars().count() == 48));
    assert!(data::default_panel::write_list(
        &mut world,
        &segments("tracking.clues"),
        &kept
    ));
    assert_eq!(list(&world, "tracking.clues"), kept);
    assert!(data::default_panel::write_list(
        &mut world,
        &segments("tracking.clues"),
        &[]
    ));
    assert!(
        list(&world, "tracking.clues").is_empty(),
        "刪到空清單留空分支"
    );
    for bad in [
        "characters.艾倫.status",
        "characters.路人.items",
        "tracking",
        "World.x",
    ] {
        assert!(
            super::panel_list(&world.mechanism, &segments(bad), vec![]).is_err(),
            "{bad}"
        );
    }
    let mut card = world.mechanism.clone();
    card.default_panel = false;
    assert!(super::panel_list(&card, &segments("tracking.clues"), vec![]).is_err());
}

/// 圍欄清單混了物件、巢狀或 `- {}`：整欄拒收、保留舊值，不收成部分清單
#[test]
fn malformed_fence_list_keeps_the_old_value() {
    let mut world = panel_world();
    patch(
        &mut world,
        r#"{"op":"replace","path":"/tracking/clues","value":["舊線索"]}"#,
    );
    for yaml in [
        "tracking:\n  clues:\n    - 腳印\n    - 名字: 兇手\n",
        "tracking:\n  clues:\n    - 腳印\n    - {}\n",
        "tracking:\n  clues:\n    - 腳印\n    - - 巢狀\n",
    ] {
        let outcome = fence(&mut world, yaml);
        assert_eq!(list(&world, "tracking.clues"), ["舊線索"], "{yaml}");
        assert!(
            kinds(&outcome).contains(&(RecordKind::Rejected, "tracking.clues".into())),
            "{yaml}"
        );
    }
}

/// 面板手改文字也照硬預算截；清單欄本身不收單一字串
#[test]
fn hand_edits_follow_the_same_budget() {
    let world = panel_world();
    let mechanism = &world.mechanism;
    let long = "長".repeat(70);
    assert_eq!(
        super::panel_hand_value(mechanism, &segments("characters.艾倫.status"), &long).unwrap(),
        "長".repeat(60)
    );
    assert_eq!(
        super::panel_hand_value(mechanism, &segments("tracking.clues.0"), &long).unwrap(),
        "長".repeat(48)
    );
    assert!(super::panel_hand_value(mechanism, &segments("tracking.clues"), "x").is_err());
    let mut card = mechanism.clone();
    card.default_panel = false;
    assert_eq!(
        super::panel_hand_value(&card, &segments("characters.艾倫.status"), &long).unwrap(),
        long
    );
}

/// 模型用簡體寫格名（艾伦）：清單路徑對齊到「艾倫」，略過項的標記也要跟著對齊，整欄照樣拒收
#[test]
fn malformed_fence_list_under_a_glyph_variant_cell_keeps_the_old_value() {
    let mut world = panel_world();
    patch(
        &mut world,
        r#"{"op":"replace","path":"/characters/艾倫/items","value":["舊劍"]}"#,
    );
    let outcome = fence(
        &mut world,
        "characters:\n  艾伦:\n    items:\n      - 新盾\n      - {}\n",
    );
    assert_eq!(list(&world, "characters.艾倫.items"), ["舊劍"]);
    assert!(kinds(&outcome).contains(&(RecordKind::Rejected, "characters.艾倫.items".into())));
}
