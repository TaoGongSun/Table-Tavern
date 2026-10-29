//! takeover-flat-nonfixed-keys：先貼開場白再介面重構，開場狀態的平欄鍵不能跟介面欄位兩份並存、不能凍住。
//! 套用流程走 `apply`，GM 回覆走正式提交入口 `gm_commit_apply`（接管桌只收 `<UpdateVariable>`）；
//! 比對分層與採值的細節直接測 `migrate_flat_table`。
use super::super::flat_migration::migrate_flat_table;
use super::super::test_support::*;
use super::super::*;
use crate::data::{self, FieldKind, FieldRule, InjectLevel, StateNode, UpdateMode};
use std::collections::BTreeMap;
use std::path::Path;

pub(super) fn rule(kind: FieldKind) -> FieldRule {
    FieldRule {
        kind,
        min: None,
        max: None,
        update: UpdateMode::Replace,
        inject: InjectLevel::Turn,
        branch: None,
        formula: None,
        same_as: None,
    }
}

fn ranged(kind: FieldKind, min: f64, max: f64) -> FieldRule {
    FieldRule {
        min: Some(min),
        max: Some(max),
        update: UpdateMode::Delta,
        ..rule(kind)
    }
}

fn leaf(value: &str) -> StateNode {
    StateNode::Leaf(value.to_owned())
}

fn key(text: &str) -> Vec<String> {
    text.split('.').map(str::to_owned).collect()
}

pub(super) fn interface_outcome(
    state_fields: serde_json::Value,
    rules: &[(&str, FieldRule)],
) -> RefactorOutcome {
    let shell = state_fields
        .as_object()
        .map(|object| {
            object
                .keys()
                .map(|key| format!("{key}:{{{{{key}}}}}\n"))
                .collect::<String>()
        })
        .unwrap_or_else(|| "<b>殼</b>".to_owned());
    RefactorOutcome {
        mode: Some("interface".to_owned()),
        characters: Vec::new(),
        interface: Some(RefactorInterface {
            state_fields,
            source_uids: Vec::new(),
            raw: String::new(),
            shell: Some(format!("<StatusData>\n{shell}</StatusData>")),
            rules: rules
                .iter()
                .map(|(path, rule)| ((*path).to_owned(), rule.clone()))
                .collect(),
            guide: "每回合必報体力".to_owned(),
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
        stage_cards: Vec::new(),
        fields: Vec::new(),
        untracked: Vec::new(),
    }
}

pub(super) fn interface_selection() -> RefactorSelection {
    RefactorSelection {
        apply_interface: true,
        ..no_player_selection(Vec::new())
    }
}

/// 非 MVU 桌、平欄帶開場狀態（貼開場白後重構的形狀）
fn flat_world(root: &Path, table: &[(&str, &str)]) -> String {
    let world_id = data::create_world(root, "地下城").unwrap();
    let mut state = data::read_state(root, &world_id).unwrap();
    for (key, value) in table {
        state
            .state
            .table
            .insert((*key).to_owned(), (*value).to_owned());
    }
    data::write_state(root, &world_id, &state).unwrap();
    world_id
}

fn table_keys(state: &data::WorldState) -> Vec<&str> {
    state.state.table.keys().map(String::as_str).collect()
}

/// GM 回覆走正式提交入口；回傳提交後的狀態與這輪的拒收／錯誤
pub(super) fn commit_reply(
    root: &Path,
    world_id: &str,
    patch: &str,
) -> (data::WorldState, Vec<String>) {
    let reply =
        format!("陸辰往前走。\n<UpdateVariable><JSONPatch>{patch}</JSONPatch></UpdateVariable>");
    let block = crate::transport::extract_state_block(&reply);
    let state = data::read_state(root, world_id).unwrap();
    let takeover = data::is_interface_takeover(root, world_id, state.refactor_mode.as_deref());
    assert!(takeover, "要是接管桌");
    let mut problems = Vec::new();
    let committed = data::state_commit::with_commit(root, world_id, |tx| {
        data::message_vars::begin_turn(tx, "t1", None).unwrap();
        data::message_vars::apply_gm_block(
            tx,
            "t1",
            data::message_vars::PendingMain {
                text: block.display.clone(),
                raw: Some(reply.clone()),
                truncated: false,
            },
            |world, stat| {
                let result = crate::commands::chat::gm_commit_apply(
                    world,
                    stat,
                    &block,
                    "玩家",
                    &state.mechanism,
                    takeover,
                    false,
                );
                if let Some(outcome) = &result.0 {
                    problems.extend(
                        outcome
                            .records
                            .iter()
                            .filter(|record| {
                                matches!(
                                    record.kind,
                                    crate::mechanism::RecordKind::Rejected
                                        | crate::mechanism::RecordKind::Error
                                )
                            })
                            .map(|record| format!("{} {}", record.path, record.detail)),
                    );
                }
                result
            },
            |_, _| Vec::new(),
        )
    })
    .unwrap()
    .expect("提交成功")
    .state;
    data::settle_pending_turn(root, world_id).unwrap();
    (committed, problems)
}

/// 真流程形狀（StatusData 鍵、產物同名）：套用後平欄只剩固定鍵、樹一份，採進來的開場數字照樣能 delta；
/// 兩輪更新都寫進樹。
/// 介面沒收的鍵搬進樹且有規則，GM replace 寫得進、insert 不長第二份。
#[test]
fn same_named_flat_fields_leave_one_copy_that_updates() {
    let root = TestRoot::new("takeover-flat-same");
    let world_id = flat_world(
        root.path(),
        &[
            ("体力", "60"),
            ("区域", "废弃门卫室"),
            ("驻守天数", "第 1 天"),
            ("place", "门卫室"),
        ],
    );
    let outcome = interface_outcome(
        serde_json::json!({ "体力": "50", "区域": "废弃门卫室" }),
        &[
            ("体力", ranged(FieldKind::Number, 0.0, 100.0)),
            ("区域", rule(FieldKind::Text)),
        ],
    );
    apply(root.path(), &world_id, &outcome, &interface_selection()).unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(table_keys(&state).is_empty(), "寫殼時平欄 place 一併清掉");
    assert_eq!(
        state.state.tree.get("体力"),
        Some(&leaf("60")),
        "採開場數字"
    );
    assert_eq!(state.state.tree.len(), 3, "{:?}", state.state.tree);
    assert_eq!(state.state.tree.get("驻守天数"), Some(&leaf("第 1 天")));
    assert!(state.mechanism.rules.contains_key("驻守天数"));

    let (state, problems) = commit_reply(
        root.path(),
        &world_id,
        r#"[{"op":"delta","path":"/体力","value":-10},{"op":"replace","path":"/区域","value":"旧角斗场"},{"op":"replace","path":"/驻守天数","value":"第 2 天"}]"#,
    );
    assert!(problems.is_empty(), "{problems:?}");
    let (state2, problems) = commit_reply(
        root.path(),
        &world_id,
        r#"[{"op":"delta","path":"/体力","value":-10},{"op":"insert","path":"/驻守天数","value":"第 3 天"}]"#,
    );
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(state.state.tree.get("区域"), Some(&leaf("旧角斗场")));
    assert_eq!(state2.state.tree.get("体力"), Some(&leaf("40")));
    assert_eq!(state2.state.tree.get("驻守天数"), Some(&leaf("第 3 天")));
    assert_eq!(state2.state.tree.len(), 3, "{:?}", state2.state.tree);
    assert!(table_keys(&state2).is_empty());
}

/// 非物件產物不重建樹，平欄也不搬不刪。
#[test]
fn non_object_state_fields_leave_the_flat_table_alone() {
    let root = TestRoot::new("takeover-flat-non-object");
    let world_id = flat_world(root.path(), &[("体力", "60"), ("区域", "门卫室")]);
    let outcome = interface_outcome(serde_json::json!("壞產物"), &[]);
    apply(root.path(), &world_id, &outcome, &interface_selection()).unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(table_keys(&state), vec!["体力", "区域"]);
    assert!(state.state.tree.is_empty());
}

pub(super) fn narration(root: &Path, world_id: &str, text: &str) {
    data::append_transcript(
        root,
        world_id,
        0,
        &data::TranscriptEvent {
            id: None,
            message_vars: None,
            vars_rev: None,
            vars_epoch: None,
            turn_key: None,
            action_id: None,
            ts: "2026-10-27T10:00:00.000Z".to_owned(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: data::TranscriptKind::Narration,
            text: text.to_owned(),
            raw: None,
            state: None,
            truncated: false,
            gm_only: false,
            marker: None,
            opening: false,
        },
    )
    .unwrap();
}

/// 連續收回兩次：整幕事件快照的平欄（含寫殼清掉的 place）已同步拿掉，舊平欄不復活；收據復原回到套用前（平欄回來、殼與樹退掉）。
#[test]
fn retracting_and_undo_do_not_revive_the_flat_copy() {
    let root = TestRoot::new("takeover-flat-retract");
    let table = [("体力", "60"), ("驻守天数", "第 1 天"), ("place", "门卫室")];
    let world_id = flat_world(root.path(), &table);
    narration(root.path(), &world_id, "開場");
    narration(root.path(), &world_id, "第二則");
    narration(root.path(), &world_id, "第三則");
    let events = data::read_transcript(root.path(), &world_id, 0).unwrap();
    assert!(
        events[0]
            .state
            .as_ref()
            .is_some_and(|snapshot| snapshot.table.contains_key("体力")),
        "開場快照帶平欄才測得到復活"
    );
    let outcome = interface_outcome(
        serde_json::json!({ "体力": "60" }),
        &[("体力", ranged(FieldKind::Number, 0.0, 100.0))],
    );
    apply_recorded(root.path(), &world_id, &outcome, &interface_selection());
    let expected = data::read_state(root.path(), &world_id).unwrap().state.tree;
    assert!(expected.contains_key("驻守天数"));

    for _ in 0..2 {
        assert!(data::pop_transcript(root.path(), &world_id, 0).unwrap());
        let state = data::read_state(root.path(), &world_id).unwrap();
        assert!(
            table_keys(&state).is_empty(),
            "快照的 place 也同步拿掉，收回不復活"
        );
        assert_eq!(state.state.tree, expected);
    }

    crate::receipts::undo_last_import(root.path(), &world_id, &data::test_exclusive(&world_id))
        .unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(table_keys(&state), vec!["place", "体力", "驻守天数"]);
    assert!(state.state.tree.is_empty(), "{:?}", state.state.tree);
    assert!(!state.mechanism.incremental);
}

// ---- 比對分層與採值：直接測遷移 ----

fn migrated(
    tree: serde_json::Value,
    table: &[(&str, &str)],
    rules: &[(&str, FieldRule)],
    tree_authoritative: bool,
) -> (data::WorldState, Vec<String>) {
    migrated_with(tree, table, rules, tree_authoritative, false)
}

fn migrated_with(
    tree: serde_json::Value,
    table: &[(&str, &str)],
    rules: &[(&str, FieldRule)],
    tree_authoritative: bool,
    fence_live: bool,
) -> (data::WorldState, Vec<String>) {
    let mut state: data::WorldState = serde_json::from_value(serde_json::json!({
        "id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "name": "遷移桌",
        "current_scene": 0
    }))
    .unwrap();
    let serde_json::Value::Object(object) = tree else {
        panic!("tree 要是物件");
    };
    fn node(value: &serde_json::Value) -> StateNode {
        match value {
            serde_json::Value::Object(object) => StateNode::Branch(
                object
                    .iter()
                    .map(|(key, value)| (key.clone(), node(value)))
                    .collect(),
            ),
            serde_json::Value::String(text) => StateNode::Leaf(text.clone()),
            other => StateNode::Leaf(other.to_string()),
        }
    }
    state.state.tree = object
        .iter()
        .map(|(key, value)| (key.clone(), node(value)))
        .collect::<BTreeMap<_, _>>();
    for (key, value) in table {
        state
            .state
            .table
            .insert((*key).to_owned(), (*value).to_owned());
    }
    for (path, rule) in rules {
        state
            .mechanism
            .rules
            .insert((*path).to_owned(), rule.clone());
    }
    state.mechanism.incremental = true;
    let moved = migrate_flat_table(&mut state, tree_authoritative, fence_live);
    (state, moved)
}

fn at(state: &data::WorldState, path: &str) -> Option<StateNode> {
    data::node_at(&state.state.tree, &key(path)).cloned()
}

#[test]
fn compatible_numbers_take_the_opening_value() {
    let (state, moved) = migrated(
        serde_json::json!({ "体力": "50", "驻守天数": "1", "生命": "5/5" }),
        &[("体力", "60"), ("驻守天数", "120"), ("生命", "3/5")],
        &[
            ("体力", ranged(FieldKind::Number, 0.0, 100.0)),
            ("驻守天数", rule(FieldKind::Counter)),
            ("生命", rule(FieldKind::Pair)),
        ],
        false,
    );
    assert_eq!(moved.len(), 3);
    assert!(state.state.table.is_empty());
    assert_eq!(at(&state, "体力"), Some(leaf("60")));
    assert_eq!(at(&state, "驻守天数"), Some(leaf("120")));
    assert_eq!(at(&state, "生命"), Some(leaf("3/5")));
}

#[test]
fn incompatible_numbers_and_strings_keep_the_product_value() {
    let (state, _) = migrated(
        serde_json::json!({
            "体力": "50", "金币": "3", "驻守天数": "1", "层数": "2", "地点": "前院", "士气": "10",
            "日数": "第 1 天"
        }),
        &[
            ("体力", "150"),         // 超出上限
            ("金币", "3.5"),         // Counter 要整數
            ("驻守天数", "第 9 天"), // 帶模板
            ("层数", "第二层"),      // 不是明確數字
            ("地点", "📍 前院"),     // 字串一律產物值
            ("士气", "20"),          // 文字欄
            ("日数", "第 120 天"), // 帶模板：產物把模板與數字寫在同一字串，換數字也 delta 不了，不採
        ],
        &[
            ("体力", ranged(FieldKind::Number, 0.0, 100.0)),
            ("金币", rule(FieldKind::Counter)),
            ("驻守天数", rule(FieldKind::Number)),
            ("士气", rule(FieldKind::Text)),
            ("日数", rule(FieldKind::Counter)),
        ],
        false,
    );
    assert!(state.state.table.is_empty());
    for (path, value) in [
        ("日数", "第 1 天"),
        ("体力", "50"),
        ("金币", "3"),
        ("驻守天数", "1"),
        ("层数", "2"),
        ("地点", "前院"),
        ("士气", "10"),
    ] {
        assert_eq!(at(&state, path), Some(leaf(value)), "{path}");
    }
}

/// MVU 桌（樹是權威）只刪平欄、不採值。
#[test]
fn authoritative_tree_only_drops_the_flat_copy() {
    let (state, moved) = migrated(
        serde_json::json!({ "体力": "50" }),
        &[("体力", "60"), ("驻守天数", "第 1 天")],
        &[("体力", ranged(FieldKind::Number, 0.0, 100.0))],
        true,
    );
    assert_eq!(moved, vec!["体力", "驻守天数"]);
    assert_eq!(at(&state, "体力"), Some(leaf("50")));
    assert_eq!(at(&state, "驻守天数"), Some(leaf("第 1 天")), "未收編照搬");
}

/// 對到分支＝衝突：刪平欄、不寫值（已知的資料損失：鑰匙、繩索會消失）。
#[test]
fn a_flat_key_landing_on_a_branch_is_dropped_without_writing() {
    let (state, moved) = migrated(
        serde_json::json!({ "背包": { "金幣": "3" } }),
        &[("背包", "鑰匙、繩索")],
        &[],
        false,
    );
    assert_eq!(moved, vec!["背包"]);
    assert!(state.state.table.is_empty());
    assert_eq!(
        at(&state, "背包"),
        Some(StateNode::Branch(BTreeMap::from([(
            "金幣".to_owned(),
            leaf("3")
        )])))
    );
}

#[test]
fn glyph_matches_take_the_unique_variant_and_drop_ambiguous_ones() {
    let (state, _) = migrated(
        serde_json::json!({ "體力": "50" }),
        &[("体力", "60")],
        &[("體力", ranged(FieldKind::Number, 0.0, 100.0))],
        false,
    );
    assert_eq!(at(&state, "體力"), Some(leaf("60")), "唯一字形採值");
    assert!(!state.state.tree.contains_key("体力"));

    let (state, moved) = migrated(
        serde_json::json!({ "沈郁": "甲", "沈鬱": "乙" }),
        &[("瀋鬱", "丙")],
        &[],
        false,
    );
    assert_eq!(moved, vec!["瀋鬱"]);
    assert_eq!(
        state.state.tree.len(),
        2,
        "歧義不搬進樹：{:?}",
        state.state.tree
    );
    assert_eq!(at(&state, "沈郁"), Some(leaf("甲")));
    assert_eq!(at(&state, "沈鬱"), Some(leaf("乙")));
}

/// 「/」前段兩個方向都算；對到分支走衝突。
#[test]
fn slash_heads_match_in_both_directions() {
    let (state, _) = migrated(
        serde_json::json!({ "剧情阶段": "第一幕", "驻留/交涉角色": "托伦" }),
        &[("剧情阶段/要塞事件", "初始"), ("驻留", "布拉姆")],
        &[],
        false,
    );
    assert!(state.state.table.is_empty());
    assert_eq!(state.state.tree.len(), 2, "{:?}", state.state.tree);
    assert_eq!(at(&state, "剧情阶段"), Some(leaf("第一幕")));
    assert_eq!(
        data::node_at(&state.state.tree, &["驻留/交涉角色".to_owned()]),
        Some(&leaf("托伦"))
    );

    let (state, _) = migrated(
        serde_json::json!({ "剧情阶段": { "主线": "一" } }),
        &[("剧情阶段/要塞事件", "初始")],
        &[],
        false,
    );
    assert_eq!(state.state.tree.len(), 1);
    assert!(state.state.table.is_empty());
}

/// 末段唯一只算已取代、不採值；多個同名末段＝衝突，都不寫。
#[test]
fn leaf_tail_matches_only_drop() {
    let (state, moved) = migrated(
        serde_json::json!({ "状态栏": { "体力": "50" } }),
        &[("体力", "60")],
        &[("状态栏.体力", ranged(FieldKind::Number, 0.0, 100.0))],
        false,
    );
    assert_eq!(moved, vec!["体力"]);
    assert_eq!(at(&state, "状态栏.体力"), Some(leaf("50")));
    assert_eq!(state.state.tree.len(), 1);

    let (state, _) = migrated(
        serde_json::json!({ "甲": { "金币": "10" }, "乙": { "金币": "20" } }),
        &[("金币", "30")],
        &[],
        false,
    );
    assert!(state.state.table.is_empty());
    assert_eq!(state.state.tree.len(), 2);
    assert_eq!(at(&state, "甲.金币"), Some(leaf("10")));
    assert_eq!(at(&state, "乙.金币"), Some(leaf("20")));
}

/// 多個平欄鍵收到同一片葉子：整組衝突，都刪、都不寫。
#[test]
fn several_flat_keys_on_one_leaf_all_become_conflicts() {
    let (state, moved) = migrated(
        serde_json::json!({ "體力": "50" }),
        &[("体力", "60"), ("體力", "80")],
        &[("體力", ranged(FieldKind::Number, 0.0, 100.0))],
        false,
    );
    assert_eq!(moved.len(), 2);
    assert!(state.state.table.is_empty());
    assert_eq!(at(&state, "體力"), Some(leaf("50")));
}

/// 沒有接管殼的增量桌（圍欄還在用）：圍欄單段鍵只轉寫到同名或只差字形的頂層葉子。只遷移那兩層與未收編，
/// 下一輪圍欄寫進樹、不回平欄；末段與「/」前段照舊留平欄。對照：全部遷移時，末段那鍵會被圍欄寫回平欄。
#[test]
fn fence_tables_only_migrate_what_the_fence_can_reach() {
    let tree =
        serde_json::json!({ "状态栏": { "体力": "50" }, "心態": "平静", "剧情阶段": "初始" });
    let table = [
        ("体力", "60"),
        ("心态", "警惕"),
        ("剧情阶段/要塞事件", "初始"),
        ("驻守天数", "第 1 天"),
    ];
    let (mut state, moved) = migrated_with(tree.clone(), &table, &[], true, true);
    assert_eq!(moved, vec!["心态", "驻守天数"]);
    assert_eq!(table_keys(&state), vec!["体力", "剧情阶段/要塞事件"]);
    let fence = crate::transport::StateBlock {
        fields: vec![
            (key("心态"), "放松".to_owned()),
            (key("驻守天数"), "第 2 天".to_owned()),
            (key("体力"), "55".to_owned()),
        ],
        updates: Vec::new(),
        display: String::new(),
        lists: Vec::new(),
        maps: Vec::new(),
        fenced: None,
    };
    crate::mechanism::apply_block(&mut state, &fence, "玩家");
    assert_eq!(at(&state, "心態"), Some(leaf("放松")));
    assert_eq!(at(&state, "驻守天数"), Some(leaf("第 2 天")));
    assert_eq!(table_keys(&state), vec!["体力", "剧情阶段/要塞事件"]);
    assert_eq!(
        state.state.table.get("体力").map(String::as_str),
        Some("55")
    );

    let (mut all, _) = migrated_with(tree, &table, &[], true, false);
    assert!(all.state.table.is_empty());
    crate::mechanism::apply_block(&mut all, &fence, "玩家");
    assert_eq!(
        table_keys(&all),
        vec!["体力"],
        "全部遷移時圍欄把末段那鍵寫回平欄"
    );
}

/// 圍欄還在用的桌：多個平欄鍵收到同一片葉子＝衝突，兩鍵都留平欄、不列進搬走的鍵，樹不動。
#[test]
fn fence_tables_keep_multi_source_conflicts_in_the_flat_table() {
    let (state, moved) = migrated_with(
        serde_json::json!({ "體力": "50" }),
        &[("体力", "60"), ("體力", "80")],
        &[],
        false,
        true,
    );
    assert!(moved.is_empty(), "{moved:?}");
    assert_eq!(table_keys(&state), vec!["体力", "體力"]);
    assert_eq!(at(&state, "體力"), Some(leaf("50")));
}
