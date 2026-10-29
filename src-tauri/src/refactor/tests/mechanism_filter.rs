//! 套用時篩掉對不到既有狀態樹的新規則／新觸發（takeover-mvu-path-mismatch 改法 3）。

use super::super::test_support::*;
use super::super::types::{RefactorInterface, RefactorMechanism};
use super::super::*;
use crate::data::{
    self, Condition, FieldKind, FieldRule, StateNode, Trigger, TriggerCase, TriggerMode,
};
use crate::mechanism;
use std::collections::BTreeMap;
use std::path::Path;

fn leaf(value: &str) -> StateNode {
    StateNode::Leaf(value.to_owned())
}

fn branch(children: &[(&str, StateNode)]) -> StateNode {
    StateNode::Branch(
        children
            .iter()
            .map(|(key, node)| ((*key).to_owned(), node.clone()))
            .collect(),
    )
}

/// 套用前的樹：MVU 卡匯入後那種已經有變數的桌（世界書 MVU 鷹架會開 incremental，即 MVU 來源）。
fn seed_tree(root: &Path, world_id: &str) {
    let mut state = data::read_state(root, world_id).unwrap();
    state.mechanism.incremental = true;
    state.state.tree = BTreeMap::from([
        ("基础信息".to_owned(), branch(&[("时间", leaf("未知"))])),
        ("冒险者".to_owned(), branch(&[])),
        ("甲".to_owned(), branch(&[("HP", leaf("5"))])),
    ]);
    data::write_state(root, world_id, &state).unwrap();
}

fn range(path: &str, default: Option<f64>) -> Condition {
    Condition::Range {
        path: path.to_owned(),
        min: Some(1.0),
        max: None,
        min_exclusive: false,
        max_exclusive: false,
        default,
    }
}

fn trigger(id: &str, when: Vec<Condition>, flag: Option<&str>) -> Trigger {
    Trigger {
        id: id.to_owned(),
        title: id.to_owned(),
        mode: if flag.is_some() {
            TriggerMode::Once
        } else {
            TriggerMode::Range
        },
        cases: vec![TriggerCase {
            when,
            text: format!("{id} 命中"),
        }],
        preamble: String::new(),
        scope: Vec::new(),
        flag: flag.map(str::to_owned),
    }
}

fn rule(kind: FieldKind) -> FieldRule {
    FieldRule::for_kind(kind)
}

fn mechanism_outcome(
    rules: BTreeMap<String, FieldRule>,
    triggers: Vec<Trigger>,
    source_uid: u64,
    interface: Option<RefactorInterface>,
    mode: Option<&str>,
) -> RefactorOutcome {
    RefactorOutcome {
        mode: mode.map(str::to_owned),
        characters: Vec::new(),
        interface,
        mechanisms: vec![RefactorMechanism {
            source_uid: source_uid.to_string(),
            rules,
            triggers,
        }],
        entries: Vec::new(),
        deletable_shared_uids: Vec::new(),
        dropped: Vec::new(),
        unabsorbed: Vec::new(),
        audit: Vec::new(),
        preserve_source_uids: Vec::new(),
        source_fingerprints: Default::default(),
        mirrors: Default::default(),
        stage_cards: Vec::new(),
        fields: Vec::new(),
    }
}

fn mechanism_selection(apply_interface: bool) -> RefactorSelection {
    RefactorSelection {
        character_indices: Vec::new(),
        apply_interface,
        mechanism_indices: vec![0],
        entry_indices: Vec::new(),
        player_index: None,
        stage_indices: Vec::new(),
    }
}

fn applied_mechanism(root: &Path, world_id: &str) -> data::Mechanism {
    data::read_state(root, world_id).unwrap().mechanism
}

#[test]
fn empty_pre_apply_tree_keeps_every_candidate() {
    let root = TestRoot::new("filter-empty-tree");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let source = seed_entry(root.path(), &world_id, "機制", "撤退進度規則");
    let outcome = mechanism_outcome(
        BTreeMap::from([("撤退進度".to_owned(), rule(FieldKind::Number))]),
        vec![trigger("撤退", vec![range("撤退進度", None)], None)],
        source,
        None,
        None,
    );
    apply(
        root.path(),
        &world_id,
        &outcome,
        &mechanism_selection(false),
    )
    .unwrap();
    let applied = applied_mechanism(root.path(), &world_id);
    assert!(applied.rules.contains_key("撤退進度"));
    assert_eq!(applied.triggers.len(), 1);
}

#[test]
fn rules_and_triggers_off_the_tree_are_dropped() {
    let root = TestRoot::new("filter-off-tree");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    seed_tree(root.path(), &world_id);
    let source = seed_entry(root.path(), &world_id, "機制", "變數更新規則");
    let rules = BTreeMap::from([
        ("基础信息.时间".to_owned(), rule(FieldKind::Text)),
        ("迷宮之主位置".to_owned(), rule(FieldKind::Text)),
        ("*.HP".to_owned(), rule(FieldKind::Counter)),
        ("冒险者.*.恐惧值".to_owned(), rule(FieldKind::Number)),
    ]);
    let triggers = vec![
        trigger("翻譯名", vec![range("撤退進度", None)], None),
        trigger("帶預設", vec![range("未知分支.值", Some(0.0))], None),
        trigger(
            "萬用帶預設",
            vec![range("冒险者.*.恐惧值", Some(0.0))],
            None,
        ),
        trigger(
            "萬用包含",
            vec![Condition::Contains {
                path: "*.HP".to_owned(),
                any: vec!["5".to_owned()],
            }],
            None,
        ),
        trigger(
            "包含未知",
            vec![Condition::Contains {
                path: "未知.x".to_owned(),
                any: vec!["是".to_owned()],
            }],
            None,
        ),
        trigger(
            "一次性",
            vec![
                Condition::Flag {
                    path: "旗標.事件甲".to_owned(),
                    expect: false,
                },
                range("甲.HP", None),
            ],
            Some("旗標.事件甲"),
        ),
    ];
    let outcome = mechanism_outcome(rules, triggers, source, None, None);
    apply(
        root.path(),
        &world_id,
        &outcome,
        &mechanism_selection(false),
    )
    .unwrap();
    let applied = applied_mechanism(root.path(), &world_id);

    assert_eq!(
        applied.rules.keys().cloned().collect::<Vec<_>>(),
        vec![
            "*.HP".to_owned(),
            "冒险者.*.恐惧值".to_owned(),
            "基础信息.时间".to_owned(),
            // 留下的一次性觸發替旗標補唯讀規則（refactor-trigger-overlap）
            "旗標.事件甲".to_owned()
        ]
    );
    // 首段萬用的規則命中實際葉子
    let hp = mechanism::rule_for_path(&applied, &["甲".to_owned(), "HP".to_owned()], Some("5"));
    assert_eq!(hp.kind, FieldKind::Counter);
    let ids: Vec<&str> = applied.triggers.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, vec!["帶預設", "一次性"]);
}

#[test]
fn existing_rules_are_not_filtered_and_only_valid_candidates_override() {
    let root = TestRoot::new("filter-existing");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    seed_tree(root.path(), &world_id);
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    state.mechanism.rules = BTreeMap::from([
        ("甲.HP".to_owned(), rule(FieldKind::ReadOnly)),
        ("未知.旗".to_owned(), rule(FieldKind::ReadOnly)),
    ]);
    data::write_state(root.path(), &world_id, &state).unwrap();
    let source = seed_entry(root.path(), &world_id, "機制", "規則");
    let outcome = mechanism_outcome(
        BTreeMap::from([
            ("甲.HP".to_owned(), rule(FieldKind::Number)),
            ("未知.旗".to_owned(), rule(FieldKind::Number)),
        ]),
        Vec::new(),
        source,
        None,
        None,
    );
    apply(
        root.path(),
        &world_id,
        &outcome,
        &mechanism_selection(false),
    )
    .unwrap();
    let applied = applied_mechanism(root.path(), &world_id);
    assert_eq!(applied.rules["甲.HP"].kind, FieldKind::Number);
    assert_eq!(applied.rules["未知.旗"].kind, FieldKind::ReadOnly);
}

fn interface(
    state_fields: serde_json::Value,
    shell: Option<&str>,
    source: u64,
) -> RefactorInterface {
    RefactorInterface {
        state_fields,
        source_uids: vec![source.to_string()],
        raw: String::new(),
        shell: shell.map(str::to_owned),
        rules: BTreeMap::new(),
        guide: String::new(),
    }
}

/// 介面頂層只在這次真的套用介面時才算數：沒勾介面、characters 模式都不算。
#[test]
fn interface_top_level_counts_only_when_interface_is_applied() {
    for (apply_interface, mode, kept) in [
        (true, None, true),
        (false, None, false),
        (true, Some("characters"), false),
    ] {
        let root = TestRoot::new("filter-interface");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        seed_tree(root.path(), &world_id);
        let source = seed_entry(root.path(), &world_id, "機制", "規則");
        let ui_source = seed_entry(root.path(), &world_id, "介面", "介面格式");
        let outcome = mechanism_outcome(
            BTreeMap::from([("新枝.值".to_owned(), rule(FieldKind::Number))]),
            Vec::new(),
            source,
            Some(interface(
                serde_json::json!({ "新枝": { "值": "1" } }),
                None,
                ui_source,
            )),
            mode,
        );
        apply(
            root.path(),
            &world_id,
            &outcome,
            &mechanism_selection(apply_interface),
        )
        .unwrap();
        assert_eq!(
            applied_mechanism(root.path(), &world_id)
                .rules
                .contains_key("新枝.值"),
            kept,
            "apply_interface={apply_interface} mode={mode:?}"
        );
    }
}

/// 介面頂層取正規化（鏡像折疊）之後的 state_fields：被折掉的容器名不算既有頂層。
#[test]
fn interface_top_level_uses_mirror_folded_fields() {
    let root = TestRoot::new("filter-interface-mirror");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    seed_tree(root.path(), &world_id);
    let source = seed_entry(root.path(), &world_id, "機制", "規則");
    let ui_source = seed_entry(root.path(), &world_id, "介面", "介面格式");
    let outcome = mechanism_outcome(
        BTreeMap::from([
            ("地點".to_owned(), rule(FieldKind::Text)),
            ("状态栏.地點".to_owned(), rule(FieldKind::Text)),
        ]),
        Vec::new(),
        source,
        Some(interface(
            serde_json::json!({
                "地點": "",
                "日期": "",
                "状态栏": { "地點": "📍 霍府", "日期": "大梁年間" }
            }),
            Some("<div>{{地點}}{{日期}}{{本回合.正文}}</div>"),
            ui_source,
        )),
        None,
    );
    apply(root.path(), &world_id, &outcome, &mechanism_selection(true)).unwrap();
    let rules = applied_mechanism(root.path(), &world_id).rules;
    assert!(rules.contains_key("地點"));
    assert!(!rules.contains_key("状态栏.地點"));
}

/// 對照組：同一棵樹但不是 MVU 來源（樹只是開場白狀態區塊解析出來的）又套用介面，套用前的樹不算數，
/// 照「樹空整批不篩」走。
#[test]
fn non_mvu_interface_apply_does_not_filter_by_the_parsed_tree() {
    let root = TestRoot::new("filter-non-mvu");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    seed_tree(root.path(), &world_id);
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    state.mechanism.incremental = false;
    data::write_state(root.path(), &world_id, &state).unwrap();
    let source = seed_entry(root.path(), &world_id, "機制", "規則");
    let ui_source = seed_entry(root.path(), &world_id, "介面", "介面格式");
    let outcome = mechanism_outcome(
        BTreeMap::from([
            ("地點".to_owned(), rule(FieldKind::Text)),
            ("别處.值".to_owned(), rule(FieldKind::Number)),
        ]),
        Vec::new(),
        source,
        Some(interface(
            serde_json::json!({ "地點": "" }),
            Some("<div>{{地點}}{{本回合.正文}}</div>"),
            ui_source,
        )),
        None,
    );
    apply(root.path(), &world_id, &outcome, &mechanism_selection(true)).unwrap();
    let rules = applied_mechanism(root.path(), &world_id).rules;
    assert!(rules.contains_key("地點"));
    assert!(rules.contains_key("别處.值"));

    // 同樣的產物套到 MVU 桌：對不到樹與介面頂層的規則照篩
    let root = TestRoot::new("filter-mvu-control");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    seed_tree(root.path(), &world_id);
    let source = seed_entry(root.path(), &world_id, "機制", "規則");
    let ui_source = seed_entry(root.path(), &world_id, "介面", "介面格式");
    let outcome = mechanism_outcome(
        BTreeMap::from([
            ("地點".to_owned(), rule(FieldKind::Text)),
            ("别處.值".to_owned(), rule(FieldKind::Number)),
        ]),
        Vec::new(),
        source,
        Some(interface(
            serde_json::json!({ "地點": "" }),
            Some("<div>{{地點}}{{本回合.正文}}</div>"),
            ui_source,
        )),
        None,
    );
    apply(root.path(), &world_id, &outcome, &mechanism_selection(true)).unwrap();
    let rules = applied_mechanism(root.path(), &world_id).rules;
    assert!(rules.contains_key("地點"));
    assert!(!rules.contains_key("别處.值"));
}

/// 旗標路徑含 `*`（trigger.flag 或 flag 條件）整組丟：once 命中會長出字面 `*` 分支。
#[test]
fn wildcard_flags_are_dropped() {
    let root = TestRoot::new("filter-wildcard-flag");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    seed_tree(root.path(), &world_id);
    let source = seed_entry(root.path(), &world_id, "機制", "規則");
    let triggers = vec![
        trigger("萬用旗標", vec![range("甲.HP", None)], Some("旗標.*.事件")),
        trigger(
            "萬用旗標條件",
            vec![Condition::Flag {
                path: "旗標.*.事件".to_owned(),
                expect: false,
            }],
            None,
        ),
        trigger("具體旗標", vec![range("甲.HP", None)], Some("旗標.事件甲")),
    ];
    let outcome = mechanism_outcome(BTreeMap::new(), triggers, source, None, None);
    apply(
        root.path(),
        &world_id,
        &outcome,
        &mechanism_selection(false),
    )
    .unwrap();
    let ids: Vec<String> = applied_mechanism(root.path(), &world_id)
        .triggers
        .into_iter()
        .map(|trigger| trigger.id)
        .collect();
    assert_eq!(ids, vec!["具體旗標".to_owned()]);
}

/// `*` 觸發是例外：套用前樹空（不做首段比對）也照樣丟。
#[test]
fn wildcard_triggers_are_dropped_even_when_tree_is_empty() {
    let root = TestRoot::new("filter-wildcard-empty-tree");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let source = seed_entry(root.path(), &world_id, "機制", "規則");
    let triggers = vec![
        trigger("萬用", vec![range("冒险者.*.恐惧值", Some(0.0))], None),
        trigger("具體", vec![range("撤退進度", None)], None),
    ];
    let outcome = mechanism_outcome(BTreeMap::new(), triggers, source, None, None);
    apply(
        root.path(),
        &world_id,
        &outcome,
        &mechanism_selection(false),
    )
    .unwrap();
    let ids: Vec<String> = applied_mechanism(root.path(), &world_id)
        .triggers
        .into_iter()
        .map(|trigger| trigger.id)
        .collect();
    assert_eq!(ids, vec!["具體".to_owned()]);
}
