//! flat-field-reporting：clean-orc 形狀的桌（開場狀態只在平欄、非 MVU、characters 重構收編規則）
//! 重構後每個平欄欄位都要能隨回合更新，而且不能長出兩份。

use super::super::test_support::*;
use super::super::*;
use crate::data::{self, FieldKind, FieldRule};
use crate::transport::StateBlock;

fn gm_update(state: &mut data::WorldState, patch_json: &str) -> crate::mechanism::Outcome {
    let block = StateBlock {
        fields: Vec::new(),
        updates: vec![format!("<JSONPatch>{patch_json}</JSONPatch>")],
        display: String::new(),
        lists: Vec::new(),
        maps: Vec::new(),
        fenced: None,
    };
    crate::mechanism::apply_block(state, &block, "玩家")
}

fn orc_outcome(source_uid: u64) -> (RefactorOutcome, RefactorSelection) {
    let outcome = RefactorOutcome {
        mode: Some("characters".to_owned()),
        characters: Vec::new(),
        interface: None,
        mechanisms: Vec::new(),
        entries: vec![crate::refactor_ai::RefactorNewEntry {
            title: "要塞規則".to_owned(),
            kind: "mechanism".to_owned(),
            content: "說明".to_owned(),
            source_uids: vec![source_uid.to_string()],
            rules: [
                (
                    "駐守天數".to_owned(),
                    FieldRule::for_kind(FieldKind::Counter),
                ),
                ("劇情階段".to_owned(), FieldRule::for_kind(FieldKind::Text)),
            ]
            .into_iter()
            .collect(),
            triggers: Vec::new(),
            meta: None,
        }],
        deletable_shared_uids: Vec::new(),
        dropped: Vec::new(),
        unabsorbed: Vec::new(),
        audit: Vec::new(),
        preserve_source_uids: Vec::new(),
        source_fingerprints: Default::default(),
        mirrors: Default::default(),
        stage_cards: Vec::new(),
    };
    let selection = RefactorSelection {
        character_indices: Vec::new(),
        apply_interface: false,
        mechanism_indices: Vec::new(),
        entry_indices: vec![0],
        player_index: None,
        stage_indices: Vec::new(),
    };
    (outcome, selection)
}

fn key(text: &str) -> Vec<String> {
    vec![text.to_owned()]
}

/// clean-orc 開場平欄四欄，AI 規則寫 駐守天數（數字）與 劇情階段（文字，平欄鍵是「剧情阶段/要塞事件」）。
/// 套用後平欄只剩 time／place／present，其餘全搬進樹（鍵名原樣，含「/」）、各有規則看得見；
/// GM 照「目前狀態」原字寫的路徑（未跳脫的「/」）與 RFC 6901 `~1` 都寫得進去，insert 不長第二份，
/// 狀態圍欄寫到同名鍵也進樹、不回平欄。
#[test]
fn every_flat_field_moves_into_the_tree_and_updates_without_duplicates() {
    let root = TestRoot::new("flat-field-repro");
    let world_id = data::create_world(root.path(), "要塞").unwrap();
    let source_uid = seed_entry(root.path(), &world_id, "要塞規則", "原文");
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    for (key, value) in [
        ("驻守天数", "第 1 天"),
        ("当前环境", "灰岩要塞城门内侧"),
        ("驻留/交涉角色", "托伦 (Toren)"),
        ("剧情阶段/要塞事件", "初始阶段。"),
        ("place", "城门"),
    ] {
        state.state.table.insert(key.to_owned(), value.to_owned());
    }
    data::write_state(root.path(), &world_id, &state).unwrap();
    let (outcome, selection) = orc_outcome(source_uid);
    apply(root.path(), &world_id, &outcome, &selection).unwrap();
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(state.state.table.keys().collect::<Vec<_>>(), vec!["place"]);
    for moved in ["当前环境", "驻留/交涉角色", "剧情阶段/要塞事件", "驻守天数"]
    {
        assert!(
            data::node_at(&state.state.tree, &key(moved)).is_some(),
            "{moved} 不在樹上"
        );
        assert!(
            state.mechanism.rules.contains_key(moved),
            "{moved} 沒有規則"
        );
        let value = match data::node_at(&state.state.tree, &key(moved)) {
            Some(data::StateNode::Leaf(value)) => value.clone(),
            _ => String::new(),
        };
        // 沒有 AI 規則的鍵補的是每輪送的文字規則；AI 規則照它自己的 inject（對齊輪一定送）
        let ai_rule = ["剧情阶段/要塞事件", "驻守天数"].contains(&moved);
        assert!(
            crate::mechanism::leaf_shown(&state.mechanism, &key(moved), &value, ai_rule),
            "{moved} 不在給 GM 的目前狀態裡"
        );
    }
    assert!(!state.mechanism.rules.contains_key("劇情階段"));
    assert_eq!(state.state.tree.len(), 4);

    let raw = gm_update(
        &mut state,
        r#"[{"op":"replace","path":"/剧情阶段/要塞事件","value":"第二阶段"},{"op":"replace","path":"/当前环境","value":"指挥塔"}]"#,
    );
    assert!(
        raw.records
            .iter()
            .all(|r| r.kind != crate::mechanism::RecordKind::Rejected),
        "{:?}",
        raw.records
    );
    assert_eq!(
        data::node_at(&state.state.tree, &key("剧情阶段/要塞事件")),
        Some(&data::StateNode::Leaf("第二阶段".to_owned()))
    );
    let escaped = gm_update(
        &mut state,
        r#"[{"op":"replace","path":"/驻留~1交涉角色","value":"托伦、布拉姆"}]"#,
    );
    assert!(escaped.records.is_empty(), "{:?}", escaped.records);
    assert_eq!(
        data::node_at(&state.state.tree, &key("驻留/交涉角色")),
        Some(&data::StateNode::Leaf("托伦、布拉姆".to_owned()))
    );
    gm_update(
        &mut state,
        r#"[{"op":"insert","path":"/剧情阶段/要塞事件","value":"第三阶段"},{"op":"insert","path":"/当前环境","value":"食堂"}]"#,
    );
    assert_eq!(state.state.tree.len(), 4, "{:?}", state.state.tree);
    assert_eq!(
        data::node_at(&state.state.tree, &key("剧情阶段/要塞事件")),
        Some(&data::StateNode::Leaf("第三阶段".to_owned()))
    );
    assert_eq!(
        data::node_at(&state.state.tree, &key("当前环境")),
        Some(&data::StateNode::Leaf("食堂".to_owned()))
    );

    let fence = StateBlock {
        fields: vec![
            (key("当前环境"), "北坡".to_owned()),
            (key("place"), "北坡".to_owned()),
        ],
        updates: Vec::new(),
        display: String::new(),
        lists: Vec::new(),
        maps: Vec::new(),
        fenced: None,
    };
    crate::mechanism::apply_block(&mut state, &fence, "玩家");
    assert!(!state.state.table.contains_key("当前环境"));
    assert_eq!(
        data::node_at(&state.state.tree, &key("当前环境")),
        Some(&data::StateNode::Leaf("北坡".to_owned()))
    );
}

fn orc_world(root: &std::path::Path, table: &[(&str, &str)]) -> (String, u64) {
    let world_id = data::create_world(root, "要塞").unwrap();
    let source_uid = seed_entry(root, &world_id, "要塞規則", "原文");
    let mut state = data::read_state(root, &world_id).unwrap();
    for (key, value) in table {
        state
            .state
            .table
            .insert((*key).to_owned(), (*value).to_owned());
    }
    data::write_state(root, &world_id, &state).unwrap();
    (world_id, source_uid)
}

/// 規則 `劇情階段` 對平欄鍵首段：兩個鍵共用首段＝歧義，整次拒套、桌上不動。
#[test]
fn slash_head_alignment_ambiguity_rejects_the_whole_apply() {
    let root = TestRoot::new("flat-field-ambiguous");
    let (world_id, source_uid) = orc_world(
        root.path(),
        &[("剧情阶段/要塞事件", "初始"), ("剧情阶段/支线", "无")],
    );
    let (outcome, selection) = orc_outcome(source_uid);
    let error = apply(root.path(), &world_id, &outcome, &selection).unwrap_err();
    assert!(error.to_string().contains("ambiguous"), "{error}");
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(state.state.tree.is_empty() && !state.mechanism.incremental);
    assert_eq!(state.state.table.len(), 2);
}

/// 搬移後收據復原：樹上搬進來的全退、平欄原值與規則還原。
#[test]
fn undo_puts_every_moved_flat_field_back() {
    let root = TestRoot::new("flat-field-undo");
    let (world_id, source_uid) = orc_world(
        root.path(),
        &[
            ("当前环境", "城门"),
            ("剧情阶段/要塞事件", "初始"),
            ("place", "城门"),
        ],
    );
    let (outcome, selection) = orc_outcome(source_uid);
    apply_recorded(root.path(), &world_id, &outcome, &selection);
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(state.state.tree.contains_key("剧情阶段/要塞事件"));
    crate::receipts::undo_last_import(root.path(), &world_id, &data::test_exclusive(&world_id))
        .unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(state.state.tree.is_empty(), "{:?}", state.state.tree);
    assert!(state.mechanism.rules.is_empty() && !state.mechanism.incremental);
    let table: Vec<&str> = state.state.table.keys().map(String::as_str).collect();
    assert_eq!(table, vec!["place", "剧情阶段/要塞事件", "当前环境"]);
}

fn narration(root: &std::path::Path, world_id: &str, text: &str) {
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

/// 搬移後「收回上一句」退回前一則的事件快照：平欄搬走的鍵不復活、樹照舊。
#[test]
fn retracting_after_the_move_does_not_revive_flat_fields() {
    let root = TestRoot::new("flat-field-retract");
    let (world_id, source_uid) = orc_world(
        root.path(),
        &[
            ("当前环境", "城门"),
            ("剧情阶段/要塞事件", "初始"),
            ("place", "城门"),
        ],
    );
    narration(root.path(), &world_id, "開場");
    narration(root.path(), &world_id, "第二則");
    let events = data::read_transcript(root.path(), &world_id, 0).unwrap();
    assert!(
        events[0]
            .state
            .as_ref()
            .is_some_and(|snapshot| snapshot.table.contains_key("当前环境")),
        "開場快照帶平欄才測得到復活"
    );
    let (outcome, selection) = orc_outcome(source_uid);
    apply_recorded(root.path(), &world_id, &outcome, &selection);
    let expected = data::read_state(root.path(), &world_id).unwrap().state.tree;
    assert!(expected.contains_key("当前环境"));

    assert!(data::pop_transcript(root.path(), &world_id, 0).unwrap());
    let state = data::read_state(root.path(), &world_id).unwrap();
    let table: Vec<&str> = state.state.table.keys().map(String::as_str).collect();
    assert_eq!(table, vec!["place"]);
    assert_eq!(state.state.tree, expected);
}
