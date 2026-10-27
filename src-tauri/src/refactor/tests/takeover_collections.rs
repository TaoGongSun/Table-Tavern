//! takeover-list-structured：介面產物帶集合區段——套用寫入集合描述、項目規則每回合注入、具名項目補 `_序`、
//! 空集合保留；GM 照路徑報變動；撤銷時集合描述跟樹與規則一起回到套用前；平欄同名鍵不會毀掉集合。
use super::super::test_support::*;
use super::super::*;
use super::takeover_flat_keys::commit_reply;
use crate::data::{self, FieldKind, FieldRule, InjectLevel, UpdateMode};
use crate::receipts;
use serde_json::json;
use std::path::Path;

const SHELL: &str = "<Status_block>\n状态栏:\n  地点: \"📍 {{地點}}\"\n  人物列表:\n{{#在場人物}}\n    - 人物:\n        名字: \"👤 {{@key}}\"\n        身份: \"🪪 {{身份}}\"\n        体力: \"💪 {{体力}}\"\n{{/在場人物}}\n  选项:\n{{#行動選項}}\n    - \"{{@n}}. {{.}}\"\n{{/行動選項}}\n  隊伍:\n{{#隊伍}}\n    - {{@key}}\n{{/隊伍}}\n</Status_block>";

fn rule(kind: FieldKind, inject: InjectLevel) -> FieldRule {
    FieldRule {
        inject,
        update: UpdateMode::Replace,
        ..FieldRule::for_kind(kind)
    }
}

fn outcome() -> RefactorOutcome {
    RefactorOutcome {
        mode: Some("interface".to_owned()),
        characters: Vec::new(),
        interface: Some(RefactorInterface {
            state_fields: json!({
                "地點": "前院",
                "在場人物": {
                    "贺铮": { "身份": "老驿长", "体力": 70 },
                    "林远": { "身份": "新任驿丞", "体力": 55 },
                },
                "行動選項": ["稳妥", "调查"],
                "隊伍": {},
            }),
            source_uids: Vec::new(),
            raw: String::new(),
            shell: Some(SHELL.to_owned()),
            rules: [
                ("地點", rule(FieldKind::Text, InjectLevel::Turn)),
                ("在場人物", rule(FieldKind::List, InjectLevel::Turn)),
                (
                    "在場人物.*.身份",
                    rule(FieldKind::Text, InjectLevel::Snapshot),
                ),
                ("在場人物.*.体力", rule(FieldKind::Text, InjectLevel::Turn)),
                ("行動選項", rule(FieldKind::List, InjectLevel::Turn)),
                ("行動選項.*", rule(FieldKind::Text, InjectLevel::Turn)),
                ("隊伍", rule(FieldKind::List, InjectLevel::Turn)),
                ("隊伍.*.職位", rule(FieldKind::Text, InjectLevel::Turn)),
            ]
            .into_iter()
            .map(|(path, rule)| (path.to_owned(), rule))
            .collect(),
            guide: "每回合必報地點".to_owned(),
        }),
        mechanisms: Vec::new(),
        entries: Vec::new(),
        deletable_shared_uids: Vec::new(),
        dropped: Vec::new(),
        unabsorbed: Vec::new(),
        audit: Vec::new(),
        preserve_source_uids: Vec::new(),
        source_fingerprints: Default::default(),
        mirrors: Default::default(),
    }
}

fn selection() -> RefactorSelection {
    RefactorSelection {
        apply_interface: true,
        ..no_player_selection(Vec::new())
    }
}

fn tree_json(root: &Path, world_id: &str) -> serde_json::Value {
    serde_json::to_value(data::read_state(root, world_id).unwrap().state.tree).unwrap()
}

#[test]
fn applying_writes_descriptors_turn_rules_order_and_keeps_empty_collections() {
    let root = TestRoot::new("collections-apply");
    let world_id = data::create_world(root.path(), "驛站").unwrap();
    apply(root.path(), &world_id, &outcome(), &selection()).unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(
        serde_json::to_value(&state.mechanism.collections).unwrap(),
        json!({
            "在場人物": { "keyed": true, "item": "object", "fields": { "体力": "number", "身份": "text" } },
            "行動選項": { "keyed": false, "item": "scalar", "type": "text" },
            "隊伍": { "keyed": true, "item": "object", "fields": { "職位": "text" } },
        })
    );
    // 集合本身與項目萬用規則沒被當成懸空規則剔掉，且一律每回合注入
    assert_eq!(state.mechanism.rules["在場人物"].kind, FieldKind::List);
    assert_eq!(
        state.mechanism.rules["在場人物.*.身份"].inject,
        InjectLevel::Turn
    );
    assert!(state.mechanism.rules.contains_key("隊伍.*.職位"));
    // 型別看集合描述，型別表不記集合底下的路徑
    assert!(state
        .mechanism
        .value_types
        .keys()
        .all(|path| !path.starts_with("在場人物")));
    let tree = tree_json(root.path(), &world_id);
    assert_eq!(
        tree["在場人物"]["贺铮"],
        json!({ "_序": "0", "身份": "老驿长", "体力": "70" })
    );
    assert_eq!(tree["在場人物"]["林远"]["_序"], json!("1"));
    assert_eq!(tree["行動選項"], json!({ "0": "稳妥", "1": "调查" }));
    assert_eq!(tree["隊伍"], json!({}));
}

#[test]
fn gm_reports_item_paths_through_the_real_commit_entry() {
    let root = TestRoot::new("collections-commit");
    let world_id = data::create_world(root.path(), "驛站").unwrap();
    apply(root.path(), &world_id, &outcome(), &selection()).unwrap();
    let (state, problems) = commit_reply(
        root.path(),
        &world_id,
        r#"[{"op":"replace","path":"/在場人物/林遠/身份","value":"驿丞"},
            {"op":"insert","path":"/在場人物/周掌柜","value":{"身份":"军需官","体力":80}},
            {"op":"remove","path":"/在場人物/贺铮"},
            {"op":"replace","path":"/行動選項","value":["甲","乙","丙"]}]"#,
    );
    assert!(problems.is_empty(), "{problems:?}");
    let tree = serde_json::to_value(&state.state.tree).unwrap();
    assert_eq!(tree["在場人物"]["林远"]["身份"], json!("驿丞"));
    assert_eq!(tree["在場人物"]["周掌柜"]["_序"], json!("2"));
    assert!(tree["在場人物"].get("贺铮").is_none());
    assert_eq!(tree["行動選項"]["2"], json!("丙"));
}

#[test]
fn undo_restores_collection_descriptors_with_the_tree_and_rules() {
    let root = TestRoot::new("collections-undo");
    let world_id = data::create_world(root.path(), "驛站").unwrap();
    let before = data::read_state(root.path(), &world_id).unwrap();
    apply_recorded(root.path(), &world_id, &outcome(), &selection());
    assert!(!data::read_state(root.path(), &world_id)
        .unwrap()
        .mechanism
        .collections
        .is_empty());
    receipts::undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    let after = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(after.mechanism.collections, before.mechanism.collections);
    assert_eq!(after.mechanism.rules, before.mechanism.rules);
    assert_eq!(after.state.tree, before.state.tree);
}

#[test]
fn a_flat_key_named_like_a_collection_does_not_damage_it() {
    let root = TestRoot::new("collections-flat");
    let world_id = data::create_world(root.path(), "驛站").unwrap();
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    state
        .state
        .table
        .insert("在場人物".to_owned(), "林远、贺铮".to_owned());
    state
        .state
        .table
        .insert("天氣".to_owned(), "小雪".to_owned());
    data::write_state(root.path(), &world_id, &state).unwrap();
    apply(root.path(), &world_id, &outcome(), &selection()).unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(!state.state.table.contains_key("在場人物"));
    let tree = serde_json::to_value(&state.state.tree).unwrap();
    assert_eq!(tree["在場人物"]["林远"]["身份"], json!("新任驿丞"));
    assert_eq!(tree["在場人物"].as_object().unwrap().len(), 2);
}

/// 鏡像分支（状态栏 底下再一份）的同名集合跟殼引用的集合用同樣粒度比對：值不同拒套、相同就折疊成一份。
#[test]
fn mirrored_collections_are_compared_whole_and_conflicts_refuse() {
    use super::super::interface::normalize_interface_paths;
    let shell = "{{#人物}}- {{@key}}: {{身份}}\n{{/人物}}{{地點}} {{時間}}";
    let rules: std::collections::BTreeMap<String, FieldRule> = [
        ("人物", FieldKind::List),
        ("人物.*.身份", FieldKind::Text),
        ("地點", FieldKind::Text),
        ("時間", FieldKind::Text),
    ]
    .into_iter()
    .map(|(path, kind)| (path.to_owned(), rule(kind, InjectLevel::Turn)))
    .collect();
    let state = |mirror: &str| {
        json!({
            "人物": { "甲": { "身份": "正典" } },
            "地點": "前院",
            "時間": "酉时",
            "状态栏": { "人物": { "甲": { "身份": mirror } }, "地點": "前院", "時間": "酉时" },
        })
    };
    assert!(normalize_interface_paths(&state("別名不同值"), Some(shell), &rules).is_err());
    let (fields, kept) = normalize_interface_paths(&state("正典"), Some(shell), &rules).unwrap();
    assert_eq!(
        fields,
        json!({ "人物": { "甲": { "身份": "正典" } }, "地點": "前院", "時間": "酉时" })
    );
    assert!(kept.contains_key("人物.*.身份"));
}
