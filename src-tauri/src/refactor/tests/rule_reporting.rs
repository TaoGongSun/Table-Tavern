//! ai-card-refactor 紅 3：非 MVU 卡收編了規則／觸發，卻沒有殼路徑替它開增量協定時，套用要自己開回報管道
//! （incremental＋樹上初值），路徑先對齊這桌既有的寫法（只差簡繁字形），已有的遊玩現值不動。

use super::super::test_support::*;
use super::super::*;
use crate::data::{self, Condition, FieldKind, FieldRule, Trigger, TriggerCase, TriggerMode};
use crate::receipts;

fn mechanism_entry(
    title: &str,
    source_uid: u64,
    rules: &[(&str, FieldRule)],
    triggers: Vec<Trigger>,
) -> crate::refactor_ai::RefactorNewEntry {
    crate::refactor_ai::RefactorNewEntry {
        title: title.to_owned(),
        kind: "mechanism".to_owned(),
        content: format!("{title}說明"),
        source_uids: vec![source_uid.to_string()],
        rules: rules
            .iter()
            .map(|(path, rule)| ((*path).to_owned(), rule.clone()))
            .collect(),
        triggers,
        meta: None,
    }
}

fn once_at_day(day: f64, path: &str, flag: &str) -> Trigger {
    Trigger {
        id: format!("day{day}"),
        title: format!("第{day}天"),
        mode: TriggerMode::Once,
        cases: vec![TriggerCase {
            when: vec![Condition::Range {
                path: path.to_owned(),
                min: Some(day),
                max: None,
                min_exclusive: false,
                max_exclusive: false,
                default: None,
            }],
            text: "事件".to_owned(),
        }],
        preamble: String::new(),
        scope: Vec::new(),
        flag: Some(flag.to_owned()),
    }
}

fn characters_outcome(entries: Vec<crate::refactor_ai::RefactorNewEntry>) -> RefactorOutcome {
    RefactorOutcome {
        mode: Some("characters".to_owned()),
        characters: Vec::new(),
        interface: None,
        mechanisms: Vec::new(),
        entries,
        deletable_shared_uids: Vec::new(),
        dropped: Vec::new(),
        unabsorbed: Vec::new(),
        audit: Vec::new(),
        preserve_source_uids: Vec::new(),
        source_fingerprints: Default::default(),
        mirrors: Default::default(),
    }
}

fn select_entries(n: usize) -> RefactorSelection {
    RefactorSelection {
        character_indices: Vec::new(),
        apply_interface: false,
        mechanism_indices: Vec::new(),
        entry_indices: (0..n).collect(),
        player_index: None,
    }
}

fn set_table(root: &std::path::Path, world_id: &str, pairs: &[(&str, &str)]) {
    let mut state = data::read_state(root, world_id).unwrap();
    for (key, value) in pairs {
        state
            .state
            .table
            .insert((*key).to_owned(), (*value).to_owned());
    }
    data::write_state(root, world_id, &state).unwrap();
}

/// clean-orc 的形狀：開場狀態只在平欄（驻守天数：第 1 天），AI 寫成繁體 駐守天數。
/// 套用後：路徑對齊成 驻守天数、樹上有初值 1、平欄那份拿掉、增量協定打開且標成重構開的；
/// 旗標是新鍵，不折成既有欄位；復原把這些都退回。
#[test]
fn rules_in_another_script_align_to_the_opening_table_and_open_reporting_then_undo() {
    let root = TestRoot::new("rule-reporting-align");
    let world_id = data::create_world(root.path(), "要塞").unwrap();
    let source_uid = seed_entry(root.path(), &world_id, "天數規則", "原文");
    set_table(
        root.path(),
        &world_id,
        &[
            ("驻守天数", "第 1 天"),
            ("当前环境", "城门"),
            ("place", "城门"),
        ],
    );
    let outcome = characters_outcome(vec![mechanism_entry(
        "駐守天數與事件",
        source_uid,
        &[("駐守天數", FieldRule::for_kind(FieldKind::Counter))],
        vec![once_at_day(7.0, "駐守天數", "事件.斥候登場")],
    )]);

    apply_recorded(root.path(), &world_id, &outcome, &select_entries(1));

    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(state.mechanism.incremental && state.mechanism.incremental_by_refactor);
    assert!(state.mechanism.rules.contains_key("驻守天数"));
    assert!(!state.mechanism.rules.contains_key("駐守天數"));
    let trigger = &state.mechanism.triggers[0];
    assert!(
        matches!(&trigger.cases[0].when[0], Condition::Range { path, .. } if path == "驻守天数")
    );
    assert_eq!(trigger.flag.as_deref(), Some("事件.斥候登場"));
    assert_eq!(
        data::node_at(&state.state.tree, &["驻守天数".to_owned()]),
        Some(&data::StateNode::Leaf("1".to_owned()))
    );
    assert!(!state.state.table.contains_key("驻守天数"));
    // 沒有規則讀的平欄鍵也搬進樹、補文字規則（flat-field-reporting）；time／place／present 留平欄
    assert!(!state.state.table.contains_key("当前环境"));
    assert_eq!(
        data::node_at(&state.state.tree, &["当前环境".to_owned()]),
        Some(&data::StateNode::Leaf("城门".to_owned()))
    );
    assert_eq!(
        state.mechanism.rules.get("当前环境").map(|rule| rule.kind),
        Some(FieldKind::Text)
    );
    assert_eq!(
        state.state.table.get("place").map(String::as_str),
        Some("城门")
    );
    assert!(!crate::mechanism::mvu_source(root.path(), &world_id, &state).unwrap());
    // 樹上存數字供觸發比大小，畫面照原寫法顯示〔作者裁決 2026-10-27〕
    assert_eq!(
        state
            .mechanism
            .display_formats
            .get("驻守天数")
            .map(String::as_str),
        Some("第 {} 天")
    );

    receipts::undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(!state.mechanism.incremental && !state.mechanism.incremental_by_refactor);
    assert!(state.state.tree.is_empty());
    assert!(state.mechanism.display_formats.is_empty());
    assert_eq!(
        state.state.table.get("驻守天数").map(String::as_str),
        Some("第 1 天")
    );
    assert_eq!(
        state.state.table.get("当前环境").map(String::as_str),
        Some("城门")
    );
    assert!(!state.mechanism.rules.contains_key("当前环境"));
}

/// 只有觸發、沒有規則的桌：觸發讀的欄位補一條每輪送的規則，回合尾才列得出來；
/// 樹上已有的現值（玩到一半）不會被開場值蓋掉；數字欄只認明確格式。
#[test]
fn trigger_only_tables_get_rules_for_read_paths_and_keep_played_values() {
    let root = TestRoot::new("rule-reporting-trigger-only");
    let world_id = data::create_world(root.path(), "驛站").unwrap();
    let source_uid = seed_entry(root.path(), &world_id, "劇情", "原文");
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    data::set_tree_value(
        &mut state.state.tree,
        &["状态栏".to_owned(), "地点".to_owned()],
        "驿丞房",
    );
    state
        .state
        .table
        .insert("糧草".to_owned(), "账载四百四十石".to_owned());
    data::write_state(root.path(), &world_id, &state).unwrap();
    let contains = Trigger {
        id: "冬營地".to_owned(),
        title: "冬營地".to_owned(),
        mode: TriggerMode::Once,
        cases: vec![TriggerCase {
            when: vec![Condition::Contains {
                path: "狀態欄.地點".to_owned(),
                any: vec!["冬營地".to_owned()],
            }],
            text: "對質".to_owned(),
        }],
        preamble: String::new(),
        scope: Vec::new(),
        flag: Some("劇情.冬營地".to_owned()),
    };
    let outcome = characters_outcome(vec![mechanism_entry(
        "劇情",
        source_uid,
        &[("糧草", FieldRule::for_kind(FieldKind::Number))],
        vec![contains],
    )]);

    apply(root.path(), &world_id, &outcome, &select_entries(1)).unwrap();

    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(state.mechanism.incremental);
    let path = ["状态栏".to_owned(), "地点".to_owned()];
    assert_eq!(
        data::node_at(&state.state.tree, &path),
        Some(&data::StateNode::Leaf("驿丞房".to_owned())),
        "遊玩現值不動"
    );
    let added = state
        .mechanism
        .rules
        .get("状态栏.地点")
        .expect("觸發讀的欄位補規則");
    assert_eq!(added.inject, data::InjectLevel::Turn);
    // 「账载四百四十石」不是明確數字格式：照原文字搬進樹、規則改文字欄（留平欄會永遠凍住）
    assert_eq!(
        state
            .mechanism
            .rules
            .get("糧草")
            .map(|rule| (rule.kind, rule.inject)),
        Some((FieldKind::Text, data::InjectLevel::Turn))
    );
    assert_eq!(
        data::node_at(&state.state.tree, &["糧草".to_owned()]),
        Some(&data::StateNode::Leaf("账载四百四十石".to_owned()))
    );
    assert!(!state.state.table.contains_key("糧草"));
    // 回合尾只列規則蓋到的欄位
    assert!(crate::mechanism::leaf_shown(
        &state.mechanism,
        &path,
        "驿丞房",
        false
    ));
    assert!(!crate::mechanism::leaf_shown(
        &state.mechanism,
        &["状态栏".to_owned(), "天气".to_owned()],
        "小雪",
        false
    ));
}

/// 兩條規則對齊後落在同一路徑、內容不同＝產物自相矛盾：整次拒套，什麼都不寫。
#[test]
fn aligned_rule_collision_rejects_the_whole_apply() {
    let root = TestRoot::new("rule-reporting-collision");
    let world_id = data::create_world(root.path(), "要塞").unwrap();
    let source_uid = seed_entry(root.path(), &world_id, "天數規則", "原文");
    set_table(root.path(), &world_id, &[("驻守天数", "第 1 天")]);
    let outcome = characters_outcome(vec![
        mechanism_entry(
            "甲",
            source_uid,
            &[("駐守天數", FieldRule::for_kind(FieldKind::Counter))],
            Vec::new(),
        ),
        mechanism_entry(
            "乙",
            source_uid,
            &[("驻守天数", FieldRule::for_kind(FieldKind::Text))],
            Vec::new(),
        ),
    ]);
    let before = data::read_worldbook(root.path(), &world_id).unwrap();

    assert!(apply(root.path(), &world_id, &outcome, &select_entries(2)).is_err());

    assert_eq!(
        data::read_worldbook(root.path(), &world_id).unwrap(),
        before
    );
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(!state.mechanism.incremental && state.mechanism.rules.is_empty());
}

/// 樹上同層同時有 人物.沈郁 與 人物.沈鬱，規則寫 人物.瀋鬱：折疊後兩個都像，分不出是誰——
/// 不能留原路徑當新欄位，整次拒套、零寫入。
#[test]
fn glyph_ambiguous_rule_path_rejects_the_whole_apply() {
    let root = TestRoot::new("rule-reporting-ambiguous");
    let world_id = data::create_world(root.path(), "書院").unwrap();
    let source_uid = seed_entry(root.path(), &world_id, "好感規則", "原文");
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    for name in ["沈郁", "沈鬱"] {
        data::set_tree_value(
            &mut state.state.tree,
            &["人物".to_owned(), name.to_owned(), "好感".to_owned()],
            "10",
        );
    }
    data::write_state(root.path(), &world_id, &state).unwrap();
    let outcome = characters_outcome(vec![mechanism_entry(
        "好感",
        source_uid,
        &[("人物.瀋鬱.好感", FieldRule::for_kind(FieldKind::Number))],
        Vec::new(),
    )]);
    let worldbook_before = data::read_worldbook(root.path(), &world_id).unwrap();

    assert!(apply(root.path(), &world_id, &outcome, &select_entries(1)).is_err());

    assert_eq!(
        data::read_worldbook(root.path(), &world_id).unwrap(),
        worldbook_before
    );
    assert_eq!(data::read_state(root.path(), &world_id).unwrap(), state);
}

/// 觸發條件路徑對齊成 驻守天数 時，命中文本裡的 `{{state:駐守天數}}` 也要一起對齊，
/// 否則條件成立、插出來卻是空字串。
#[test]
fn trigger_text_placeholders_align_with_their_conditions() {
    let root = TestRoot::new("rule-reporting-placeholder");
    let world_id = data::create_world(root.path(), "要塞").unwrap();
    let source_uid = seed_entry(root.path(), &world_id, "天數規則", "原文");
    set_table(root.path(), &world_id, &[("驻守天数", "第 1 天")]);
    let mut trigger = once_at_day(1.0, "駐守天數", "事件.第一天");
    trigger.cases[0].text = "駐守第{{state:駐守天數}}天".to_owned();
    trigger.preamble = "（第{{state:駐守天數}}天）".to_owned();
    let outcome = characters_outcome(vec![mechanism_entry(
        "駐守天數與事件",
        source_uid,
        &[],
        vec![trigger],
    )]);

    apply(root.path(), &world_id, &outcome, &select_entries(1)).unwrap();

    let state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(
        state.mechanism.triggers[0].cases[0].text,
        "駐守第{{state:驻守天数}}天"
    );
    let hits =
        crate::mechanism::evaluate_triggers(&state.state.tree, &state.mechanism, "玩家").hits;
    assert_eq!(
        hits.get("day1").map(String::as_str),
        Some("（第1天）\n\n駐守第1天")
    );
}
