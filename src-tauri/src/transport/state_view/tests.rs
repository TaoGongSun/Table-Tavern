#[allow(unused_imports)]
use super::super::arrivals::*;
#[allow(unused_imports)]
use super::super::assemble::*;
#[allow(unused_imports)]
use super::super::client::*;
#[allow(unused_imports)]
use super::super::context::*;
#[allow(unused_imports)]
use super::super::messages::*;
#[allow(unused_imports)]
use super::super::response::*;
#[allow(unused_imports)]
use super::super::test_support::{card, event, worldbook_entry};
#[allow(unused_imports)]
use super::super::turns::*;
use super::*;
#[allow(unused_imports)]
use crate::data::{
    self, AppConfig, CharacterCard, DataResult, FieldKind, FieldRule, InjectLevel, Mechanism,
    StateNode, TableState, Tier, TranscriptEvent, TranscriptKind, Visibility, WorldbookEntry,
};
#[allow(unused_imports)]
use crate::mechanism;
#[allow(unused_imports)]
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn gm_dynamic_block_renders_nested_tree() {
    let state = TableState {
        table: std::collections::BTreeMap::new(),
        tree: std::collections::BTreeMap::from([(
            "World".to_owned(),
            StateNode::Branch(std::collections::BTreeMap::from([(
                "城市".to_owned(),
                StateNode::Leaf("晨港".to_owned()),
            )])),
        )]),
        notes: Vec::new(),
        changes: std::collections::BTreeMap::new(),
        triggers: std::collections::BTreeMap::new(),
        jumps: std::collections::BTreeMap::new(),
    };
    let dynamic = gm_dynamic_block(
        &[],
        &state,
        "阿濤",
        &Mechanism::default(),
        &StateScope::default(),
        "zh-TW",
    );
    assert!(dynamic.contains("World：\n  城市：晨港"));
}

/// 初始樹保留的玩家巨集只在送進這桌模型上下文前才換成實名。
#[test]
fn gm_dynamic_block_replaces_user_macro_in_tree_leaves() {
    let state = TableState {
        table: std::collections::BTreeMap::new(),
        tree: std::collections::BTreeMap::from([(
            "Player".to_owned(),
            StateNode::Branch(std::collections::BTreeMap::from([(
                "Name".to_owned(),
                StateNode::Leaf("{{user}}".to_owned()),
            )])),
        )]),
        notes: Vec::new(),
        changes: std::collections::BTreeMap::new(),
        triggers: std::collections::BTreeMap::new(),
        jumps: std::collections::BTreeMap::new(),
    };

    let dynamic = gm_dynamic_block(
        &[],
        &state,
        "阿濤",
        &Mechanism::default(),
        &StateScope::default(),
        "zh-TW",
    );
    assert!(dynamic.contains("Name：阿濤"));
    assert!(!dynamic.contains("{{user}}"));
}

#[test]
fn gm_dynamic_block_prints_notes_after_current_state_and_hides_when_empty() {
    let with_notes = TableState {
        table: std::collections::BTreeMap::new(),
        tree: std::collections::BTreeMap::new(),
        notes: vec!["World.HP 已夾在範圍內，目前值 100。".to_owned()],
        changes: std::collections::BTreeMap::new(),
        triggers: std::collections::BTreeMap::new(),
        jumps: std::collections::BTreeMap::new(),
    };
    let dynamic = gm_dynamic_block(
        &[],
        &with_notes,
        "阿濤",
        &Mechanism::default(),
        &StateScope::default(),
        "zh-TW",
    );
    assert!(dynamic.contains("## 上一輪被系統擋下的更新（請照這些現值修正）"));
    assert!(dynamic.contains("World.HP 已夾在範圍內，目前值 100。"));

    let without_notes = TableState::default();
    let dynamic = gm_dynamic_block(
        &[],
        &without_notes,
        "阿濤",
        &Mechanism::default(),
        &StateScope::default(),
        "zh-TW",
    );
    assert!(!dynamic.contains("上一輪被系統擋下的更新"));
}

// ---- gm_dynamic_block：觸發表命中文本的「當前情境」段 ----

fn trigger_with_scope(id: &str, scope: &[&str]) -> data::Trigger {
    data::Trigger {
        id: id.to_owned(),
        title: id.to_owned(),
        mode: data::TriggerMode::Range,
        cases: Vec::new(),
        preamble: String::new(),
        scope: scope.iter().map(|segment| (*segment).to_owned()).collect(),
        flag: None,
    }
}

#[test]
fn gm_dynamic_block_prints_trigger_hits_after_current_state() {
    let mechanism = Mechanism {
        value_types: Default::default(),
        version: 1,
        rules: BTreeMap::new(),
        triggers: vec![trigger_with_scope("侵略", &[])],
        incremental: true,
        guide: String::new(),
    };
    let state = TableState {
        table: BTreeMap::from([("time".to_owned(), "黃昏".to_owned())]),
        tree: BTreeMap::new(),
        notes: Vec::new(),
        changes: BTreeMap::new(),
        triggers: BTreeMap::from([("侵略".to_owned(), "戰雲密布".to_owned())]),
        jumps: BTreeMap::new(),
    };
    let dynamic = gm_dynamic_block(
        &[],
        &state,
        "阿濤",
        &mechanism,
        &StateScope::default(),
        "zh-TW",
    );
    let state_pos = dynamic.find("## 目前狀態").expect("目前狀態應該有印");
    let trigger_pos = dynamic.find("## 當前情境").expect("當前情境應該有印");
    assert!(trigger_pos > state_pos);
    assert!(dynamic.contains("戰雲密布"));
}

#[test]
fn gm_dynamic_block_hides_trigger_section_when_state_triggers_is_empty() {
    let mechanism = Mechanism {
        value_types: Default::default(),
        version: 1,
        rules: BTreeMap::new(),
        triggers: vec![trigger_with_scope("侵略", &[])],
        incremental: true,
        guide: String::new(),
    };
    let state = TableState {
        table: BTreeMap::from([("time".to_owned(), "黃昏".to_owned())]),
        tree: BTreeMap::new(),
        notes: Vec::new(),
        changes: BTreeMap::new(),
        triggers: BTreeMap::new(),
        jumps: BTreeMap::new(),
    };
    let dynamic = gm_dynamic_block(
        &[],
        &state,
        "阿濤",
        &mechanism,
        &StateScope::default(),
        "zh-TW",
    );
    assert!(!dynamic.contains("當前情境"));
}

/// 不在場角色那支被裁掉時，牽到那支的觸發文本不該送；`align`（換幕全樹對齊）
/// 忽略裁切，照樣全印。
#[test]
fn gm_dynamic_block_hides_trigger_scoped_to_a_hidden_branch_but_prints_when_aligned() {
    let mechanism = Mechanism {
        value_types: Default::default(),
        version: 1,
        rules: BTreeMap::new(),
        triggers: vec![
            trigger_with_scope("亞瑟關係", &["Heroes", "亞瑟"]),
            trigger_with_scope("世界氛圍", &[]),
        ],
        incremental: true,
        guide: String::new(),
    };
    let state = TableState {
        table: BTreeMap::from([("time".to_owned(), "黃昏".to_owned())]),
        tree: BTreeMap::new(),
        notes: Vec::new(),
        changes: BTreeMap::new(),
        triggers: BTreeMap::from([
            ("亞瑟關係".to_owned(), "亞瑟關係文本".to_owned()),
            ("世界氛圍".to_owned(), "世界氛圍文本".to_owned()),
        ]),
        jumps: BTreeMap::new(),
    };
    let hidden = vec![vec!["Heroes".to_owned(), "亞瑟".to_owned()]];
    let scope = StateScope {
        hidden: hidden.clone(),
        align: false,
    };
    let dynamic = gm_dynamic_block(&[], &state, "阿濤", &mechanism, &scope, "zh-TW");
    assert!(!dynamic.contains("亞瑟關係文本"));
    assert!(dynamic.contains("世界氛圍文本"));

    let aligned = StateScope {
        hidden,
        align: true,
    };
    let dynamic = gm_dynamic_block(&[], &state, "阿濤", &mechanism, &aligned, "zh-TW");
    assert!(dynamic.contains("亞瑟關係文本"));
}

/// scope 比隱藏分支更深（後代路徑）也要跟著裁——亞瑟底下的好感細節同樣屬於他那支。
#[test]
fn gm_dynamic_block_hides_trigger_scoped_to_a_descendant_of_a_hidden_branch() {
    let mechanism = Mechanism {
        value_types: Default::default(),
        version: 1,
        rules: BTreeMap::new(),
        triggers: vec![trigger_with_scope(
            "亞瑟細節",
            &["Heroes", "亞瑟", "Affection"],
        )],
        incremental: true,
        guide: String::new(),
    };
    let state = TableState {
        table: BTreeMap::from([("time".to_owned(), "黃昏".to_owned())]),
        tree: BTreeMap::new(),
        notes: Vec::new(),
        changes: BTreeMap::new(),
        triggers: BTreeMap::from([("亞瑟細節".to_owned(), "亞瑟細節文本".to_owned())]),
        jumps: BTreeMap::new(),
    };
    let scope = StateScope {
        hidden: vec![vec!["Heroes".to_owned(), "亞瑟".to_owned()]],
        align: false,
    };
    let dynamic = gm_dynamic_block(&[], &state, "阿濤", &mechanism, &scope, "zh-TW");
    assert!(!dynamic.contains("亞瑟細節文本"));
}

#[test]
fn gm_dynamic_block_orders_triggers_by_mechanism_list_not_by_map_key() {
    let mechanism = Mechanism {
        value_types: Default::default(),
        version: 1,
        rules: BTreeMap::new(),
        // 刻意讓清單順序跟字典序相反，確認印出順序跟著 Vec 走。
        triggers: vec![trigger_with_scope("乙", &[]), trigger_with_scope("甲", &[])],
        incremental: true,
        guide: String::new(),
    };
    let state = TableState {
        table: BTreeMap::from([("time".to_owned(), "黃昏".to_owned())]),
        tree: BTreeMap::new(),
        notes: Vec::new(),
        changes: BTreeMap::new(),
        triggers: BTreeMap::from([
            ("甲".to_owned(), "甲文本".to_owned()),
            ("乙".to_owned(), "乙文本".to_owned()),
        ]),
        jumps: BTreeMap::new(),
    };
    let dynamic = gm_dynamic_block(
        &[],
        &state,
        "阿濤",
        &mechanism,
        &StateScope::default(),
        "zh-TW",
    );
    let pos_b = dynamic.find("乙文本").unwrap();
    let pos_a = dynamic.find("甲文本").unwrap();
    assert!(pos_b < pos_a);
}

/// 全量桌（`!mechanism.incremental`）逐字維持現狀：就算 `state.triggers` 有值也不印。
#[test]
fn gm_dynamic_block_never_prints_trigger_section_for_a_full_snapshot_table() {
    let state = TableState {
        table: BTreeMap::from([("time".to_owned(), "黃昏".to_owned())]),
        tree: BTreeMap::new(),
        notes: Vec::new(),
        changes: BTreeMap::new(),
        triggers: BTreeMap::from([("侵略".to_owned(), "戰雲密布".to_owned())]),
        jumps: BTreeMap::new(),
    };
    let dynamic = gm_dynamic_block(
        &[],
        &state,
        "阿濤",
        &Mechanism::default(),
        &StateScope::default(),
        "zh-TW",
    );
    assert!(!dynamic.contains("當前情境"));
    assert!(!dynamic.contains("戰雲密布"));
}

// ---- 狀態欄二期包 5：注入策略＋分支切割 ----

#[test]
fn state_scope_hides_absent_characters_keeps_player_and_present_and_is_disabled_by_empty_present() {
    let arthur = card("arthur-id", "亞瑟", "", "");
    let crow = card("crow-id", "鴉", "", "");
    let player = card("player-id", "阿濤", "", "");
    let tree = BTreeMap::from([
        (
            "亞瑟".to_owned(),
            StateNode::Branch(BTreeMap::from([(
                "HP".to_owned(),
                StateNode::Leaf("100".to_owned()),
            )])),
        ),
        (
            "鴉".to_owned(),
            StateNode::Branch(BTreeMap::from([(
                "HP".to_owned(),
                StateNode::Leaf("50".to_owned()),
            )])),
        ),
    ]);
    let mechanism = Mechanism {
        incremental: true,
        ..Mechanism::default()
    };
    let bindings = BTreeMap::new();

    let mut present_state = TableState {
        tree: tree.clone(),
        ..TableState::default()
    };
    present_state
        .table
        .insert("present".to_owned(), "亞瑟".to_owned());
    let scope = state_scope(
        &present_state,
        &mechanism,
        &[arthur.clone(), crow.clone()],
        Some(&player),
        &bindings,
        false,
    );
    assert_eq!(scope.hidden, vec![vec!["鴉".to_owned()]]);
    assert!(!scope.align);

    // present 欄空著＝寧可全送，不要因為模型沒報 present 就裁瞎了
    let empty_present_state = TableState {
        tree,
        ..TableState::default()
    };
    let scope = state_scope(
        &empty_present_state,
        &mechanism,
        &[arthur, crow],
        Some(&player),
        &bindings,
        true,
    );
    assert!(scope.hidden.is_empty());
    assert!(scope.align);

    // 全量桌完全不裁、不對齊
    let scope = state_scope(
        &present_state,
        &Mechanism::default(),
        &[],
        None,
        &bindings,
        true,
    );
    assert!(scope.hidden.is_empty());
    assert!(!scope.align);
}

/// 手足規則：容器裡有一支綁到角色卡，同容器其餘分支就一律當人看——
/// MVU 卡 15 個英雄只會有幾張角色卡，剩下的沒卡也該裁。頂層不套這條（會把 World 裁掉）。
#[test]
fn state_scope_hides_uncarded_siblings_in_the_same_container_but_never_at_the_top_level() {
    let hero = |hp: &str| {
        StateNode::Branch(BTreeMap::from([(
            "HP".to_owned(),
            StateNode::Leaf(hp.to_owned()),
        )]))
    };
    let mut state = TableState {
        tree: BTreeMap::from([
            (
                "World".to_owned(),
                StateNode::Branch(BTreeMap::from([(
                    "Invasion".to_owned(),
                    StateNode::Leaf("35".to_owned()),
                )])),
            ),
            (
                "Heroes".to_owned(),
                StateNode::Branch(BTreeMap::from([
                    ("亞瑟".to_owned(), hero("100")),
                    ("鴉".to_owned(), hero("50")),
                    ("諾亞".to_owned(), hero("70")),
                ])),
            ),
        ]),
        ..TableState::default()
    };
    state.table.insert("present".to_owned(), "諾亞".to_owned());
    let mechanism = Mechanism {
        incremental: true,
        ..Mechanism::default()
    };
    // 只有亞瑟有角色卡：他不在場所以被裁，沒卡的鴉跟著被裁，在場的諾亞留著
    let scope = state_scope(
        &state,
        &mechanism,
        &[card("arthur-id", "亞瑟", "", "")],
        None,
        &BTreeMap::new(),
        false,
    );
    assert!(scope
        .hidden
        .contains(&vec!["Heroes".to_owned(), "亞瑟".to_owned()]));
    assert!(scope
        .hidden
        .contains(&vec!["Heroes".to_owned(), "鴉".to_owned()]));
    assert!(!scope
        .hidden
        .contains(&vec!["Heroes".to_owned(), "諾亞".to_owned()]));
    // 桌級分支不受手足規則波及
    assert!(!scope.hidden.contains(&vec!["World".to_owned()]));
    assert!(!scope.hidden.contains(&vec!["Heroes".to_owned()]));
}

#[test]
fn incremental_round_tail_hides_absent_branch_snapshot_and_rare_but_shows_turn_with_marks() {
    let mechanism = Mechanism {
        incremental: true,
        guide: String::new(),
        rules: BTreeMap::from([(
            "World.Secret".to_owned(),
            FieldRule::for_kind(FieldKind::ReadOnly),
        )]),
        ..Mechanism::default()
    };
    let mut state = TableState::default();
    state.table.insert("present".to_owned(), "亞瑟".to_owned());
    state.tree = BTreeMap::from([
        (
            "World".to_owned(),
            StateNode::Branch(BTreeMap::from([
                ("HP".to_owned(), StateNode::Leaf("100".to_owned())),
                ("Desc".to_owned(), StateNode::Leaf("晨港".to_owned())),
                ("Secret".to_owned(), StateNode::Leaf("藏寶圖".to_owned())),
            ])),
        ),
        (
            "亞瑟".to_owned(),
            StateNode::Branch(BTreeMap::from([(
                "HP".to_owned(),
                StateNode::Leaf("80".to_owned()),
            )])),
        ),
        (
            "鴉".to_owned(),
            StateNode::Branch(BTreeMap::from([(
                "HP".to_owned(),
                StateNode::Leaf("50".to_owned()),
            )])),
        ),
    ]);
    state.changes.insert("World.HP".to_owned(), "+5".to_owned());

    // 平常輪：不在場的「鴉」整支不印；Snapshot（Desc）不印；Rare（Secret）不印；
    // Turn（HP）印，且帶變動標記。
    let scope = StateScope {
        hidden: vec![vec!["鴉".to_owned()]],
        align: false,
    };
    let dynamic = gm_dynamic_block(&[], &state, "阿濤", &mechanism, &scope, "zh-TW");
    assert!(dynamic.contains("## 目前狀態（這桌的檯面，接續它往下演）"));
    assert!(dynamic.contains("HP：100（+5）"));
    assert!(!dynamic.contains("Desc"));
    assert!(!dynamic.contains("Secret"));
    assert!(!dynamic.contains("鴉"));
    assert!(dynamic.contains("亞瑟"));

    // 對齊輪：忽略 hidden、Snapshot 也印，只有 Rare 還是不印；標題換成對齊版。
    let align_scope = StateScope {
        hidden: vec![vec!["鴉".to_owned()]],
        align: true,
    };
    let aligned = gm_dynamic_block(&[], &state, "阿濤", &mechanism, &align_scope, "zh-TW");
    assert!(aligned.contains("## 目前狀態（完整對齊，以下是系統帳上的真值，請以此為準）"));
    assert!(aligned.contains("Desc：晨港"));
    assert!(!aligned.contains("Secret"));
    assert!(aligned.contains("鴉"));
}

/// 全量桌（!mechanism.incremental）逐字維持現狀：不管 mechanism.rules、changes、
/// scope 塞了什麼，輸出都跟本包以前的行為一樣——不裁、不濾、不標。
#[test]
fn full_scale_table_renders_everything_verbatim_ignoring_rules_scope_and_changes() {
    let mechanism = Mechanism {
        incremental: false,
        guide: String::new(),
        rules: BTreeMap::from([(
            "World.Secret".to_owned(),
            FieldRule::for_kind(FieldKind::ReadOnly),
        )]),
        ..Mechanism::default()
    };
    let mut state = TableState {
        tree: BTreeMap::from([(
            "World".to_owned(),
            StateNode::Branch(BTreeMap::from([
                ("HP".to_owned(), StateNode::Leaf("100".to_owned())),
                ("Desc".to_owned(), StateNode::Leaf("晨港".to_owned())),
                ("Secret".to_owned(), StateNode::Leaf("藏寶圖".to_owned())),
            ])),
        )]),
        ..TableState::default()
    };
    state.changes.insert("World.HP".to_owned(), "+5".to_owned());
    let scope = StateScope {
        hidden: vec![vec!["World".to_owned()]],
        align: false,
    };

    let dynamic = gm_dynamic_block(&[], &state, "阿濤", &mechanism, &scope, "zh-TW");
    assert_eq!(
        dynamic,
        "## 目前狀態（這桌的檯面，接續它往下演）\n\
             World：\n  Desc：晨港\n  HP：100\n  Secret：藏寶圖"
    );
}

#[test]
fn character_state_block_shows_only_own_branch_with_marks_and_excludes_rare() {
    let mechanism = Mechanism {
        incremental: true,
        guide: String::new(),
        rules: BTreeMap::from([(
            "Heroes.亞瑟.Hidden".to_owned(),
            FieldRule::for_kind(FieldKind::ReadOnly),
        )]),
        ..Mechanism::default()
    };
    let mut state = TableState {
        tree: BTreeMap::from([(
            "Heroes".to_owned(),
            StateNode::Branch(BTreeMap::from([
                (
                    "亞瑟".to_owned(),
                    StateNode::Branch(BTreeMap::from([
                        ("HP".to_owned(), StateNode::Leaf("80".to_owned())),
                        ("Hidden".to_owned(), StateNode::Leaf("秘密".to_owned())),
                    ])),
                ),
                (
                    "鴉".to_owned(),
                    StateNode::Branch(BTreeMap::from([(
                        "HP".to_owned(),
                        StateNode::Leaf("50".to_owned()),
                    )])),
                ),
            ])),
        )]),
        ..TableState::default()
    };
    state
        .changes
        .insert("Heroes.亞瑟.HP".to_owned(), "-10".to_owned());

    let branch = vec!["Heroes".to_owned(), "亞瑟".to_owned()];
    let block = character_state_block(&state, &mechanism, &branch, "亞瑟", "阿濤").unwrap();
    assert!(block.starts_with(
        "## 「亞瑟」目前的狀態（系統帳，唯讀；可以拿來演，但不要輸出任何狀態欄或更新區塊）"
    ));
    assert!(block.contains("HP：80（-10）"));
    assert!(!block.contains("Hidden"));
    assert!(!block.contains("鴉"));

    // 沒綁到分支、分支其實是葉子、分支不存在——都是 None
    assert!(character_state_block(&state, &mechanism, &[], "亞瑟", "阿濤").is_none());
    assert!(character_state_block(
        &state,
        &mechanism,
        &["Heroes".to_owned(), "亞瑟".to_owned(), "HP".to_owned()],
        "亞瑟",
        "阿濤"
    )
    .is_none());
    assert!(
        character_state_block(&state, &mechanism, &["不存在".to_owned()], "亞瑟", "阿濤").is_none()
    );
}

#[test]
fn resolve_branch_prefers_binding_falls_back_when_invalid_and_finds_nested_names() {
    let tree = BTreeMap::from([
        (
            "Heroes".to_owned(),
            StateNode::Branch(BTreeMap::from([(
                "亞瑟".to_owned(),
                StateNode::Branch(BTreeMap::from([(
                    "HP".to_owned(),
                    StateNode::Leaf("80".to_owned()),
                )])),
            )])),
        ),
        ("鴉".to_owned(), StateNode::Branch(BTreeMap::new())),
    ]);

    // 指認優先：卡名明明是「鴉」，指認路徑照樣贏
    let bindings = BTreeMap::from([(
        "card-crow".to_owned(),
        vec!["Heroes".to_owned(), "亞瑟".to_owned()],
    )]);
    assert_eq!(
        resolve_branch(&tree, &bindings, "card-crow", "鴉"),
        Some(vec!["Heroes".to_owned(), "亞瑟".to_owned()])
    );

    // 指認路徑不存在＝失效，退回同名比對
    let invalid = BTreeMap::from([(
        "card-crow".to_owned(),
        vec!["Not".to_owned(), "Exist".to_owned()],
    )]);
    assert_eq!(
        resolve_branch(&tree, &invalid, "card-crow", "鴉"),
        Some(vec!["鴉".to_owned()])
    );

    // 指認路徑指到葉子（不是分支）也視為失效
    let points_at_leaf = BTreeMap::from([(
        "card-arthur".to_owned(),
        vec!["Heroes".to_owned(), "亞瑟".to_owned(), "HP".to_owned()],
    )]);
    assert_eq!(
        resolve_branch(&tree, &points_at_leaf, "card-arthur", "亞瑟"),
        Some(vec!["Heroes".to_owned(), "亞瑟".to_owned()])
    );

    // 沒有指認：巢狀（Heroes/亞瑟）也找得到
    assert_eq!(
        resolve_branch(&tree, &BTreeMap::new(), "card-arthur", "亞瑟"),
        Some(vec!["Heroes".to_owned(), "亞瑟".to_owned()])
    );

    // 完全找不到
    assert_eq!(
        resolve_branch(&tree, &BTreeMap::new(), "card-x", "不存在的人"),
        None
    );
}

#[test]
fn snapshot_updates_returns_only_snapshot_level_changes_with_user_macro_replaced() {
    let mechanism = Mechanism {
        incremental: true,
        ..Mechanism::default()
    };
    let state = TableState {
        tree: BTreeMap::from([(
            "World".to_owned(),
            StateNode::Branch(BTreeMap::from([
                ("HP".to_owned(), StateNode::Leaf("100".to_owned())),
                (
                    "Desc".to_owned(),
                    StateNode::Leaf("{{user}} 的家鄉".to_owned()),
                ),
            ])),
        )]),
        changes: BTreeMap::from([
            ("World.HP".to_owned(), "+5".to_owned()),
            ("World.Desc".to_owned(), "更新".to_owned()),
        ]),
        ..TableState::default()
    };

    let updates = snapshot_updates(&state, &mechanism, "阿濤");
    assert_eq!(
        updates,
        vec![("World.Desc".to_owned(), "阿濤 的家鄉".to_owned())]
    );

    // 全量桌一律回空
    assert!(snapshot_updates(&state, &Mechanism::default(), "阿濤").is_empty());
}
