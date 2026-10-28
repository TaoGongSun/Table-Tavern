//! card-field-overlaps-fixed-keys：卡欄位與固定鍵 place／present 同義（clean-orc 的「当前环境」「驻留/交涉角色」）。
//! 重構 AI 標 `same_as` 後，收圍欄的桌由 App 把固定鍵抄進卡欄位：GM 只報固定鍵，卡欄位不再凍在開場值，
//! 觸發條件與佔位照舊讀樹上的卡欄位。
use super::super::test_support::*;
use super::super::*;
use crate::data::{self, FieldKind, FieldRule, StateNode};
use std::path::Path;

const PLACE: &str = "当前环境";
const ROSTER: &str = "驻留/交涉角色";

fn key(text: &str) -> Vec<String> {
    vec![text.to_owned()]
}

fn marked(same_as: Option<&str>) -> FieldRule {
    FieldRule {
        same_as: same_as.map(str::to_owned),
        ..FieldRule::for_kind(FieldKind::Text)
    }
}

fn leaf<'a>(state: &'a data::WorldState, name: &str) -> Option<&'a str> {
    match data::node_at(&state.state.tree, &key(name)) {
        Some(StateNode::Leaf(value)) => Some(value),
        _ => None,
    }
}

/// clean-orc 形狀的桌：開場狀態只在平欄（卡欄位四個，`extra` 另加平欄鍵，例如已報過的 place）。
fn orc_world(root: &Path, extra: &[(&str, &str)]) -> (String, u64) {
    let world_id = data::create_world(root, "要塞").unwrap();
    let source_uid = seed_entry(root, &world_id, "要塞規則", "原文");
    let mut state = data::read_state(root, &world_id).unwrap();
    for (name, value) in [
        ("驻守天数", "第 1 天"),
        (PLACE, "灰岩要塞城门内侧"),
        (ROSTER, "托伦 (Toren)"),
        ("剧情阶段/要塞事件", "初始阶段。"),
    ]
    .iter()
    .chain(extra)
    {
        state
            .state
            .table
            .insert((*name).to_owned(), (*value).to_owned());
    }
    data::write_state(root, &world_id, &state).unwrap();
    (world_id, source_uid)
}

/// characters 重構產物（真流程形狀：兩欄沒有 AI 規則，由開回報管道補文字規則）；`mirrors` 是盤點的 MIRRORS。
fn orc_outcome_with(
    source_uid: u64,
    mode: &str,
    mirrors: &[(&str, &str)],
) -> (RefactorOutcome, RefactorSelection) {
    let outcome = RefactorOutcome {
        mode: Some(mode.to_owned()),
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
                    "驻守天数".to_owned(),
                    FieldRule::for_kind(FieldKind::Counter),
                ),
            ]
            .into_iter()
            .collect(),
            triggers: vec![serde_json::from_value(serde_json::json!({
                "id": "north",
                "title": "北坡",
                "mode": "range",
                "cases": [{ "when": [{ "kind": "contains", "path": PLACE, "any": ["北坡"] }], "text": "风雪更大了（{{state:当前环境}}）" }]
            }))
            .unwrap()],
            meta: None,
        }],
        deletable_shared_uids: Vec::new(),
        dropped: Vec::new(),
        unabsorbed: Vec::new(),
        audit: Vec::new(),
        preserve_source_uids: Vec::new(),
        source_fingerprints: Default::default(),
        mirrors: mirrors
            .iter()
            .map(|(name, key)| ((*name).to_owned(), (*key).to_owned()))
            .collect(),
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

/// `mark`：盤點把兩欄標成 place／present；false＝未標記桌。
fn orc_outcome(source_uid: u64, mode: &str, mark: bool) -> (RefactorOutcome, RefactorSelection) {
    let mirrors: &[(&str, &str)] = if mark {
        &[(PLACE, "place"), (ROSTER, "present")]
    } else {
        &[]
    };
    orc_outcome_with(source_uid, mode, mirrors)
}

/// 走真的回合提交入口（非接管桌）：回覆帶狀態圍欄與更新區塊，落一則帶快照的 GM 事件。
fn commit(root: &Path, world_id: &str, turn: &str, fence: &str, patch: &str) -> data::WorldState {
    let reply = format!(
        "风雪拍打城墙。\n```state\n{fence}\n```\n<UpdateVariable><JSONPatch>{patch}</JSONPatch></UpdateVariable>"
    );
    let block = crate::transport::extract_state_block(&reply);
    let state = data::read_state(root, world_id).unwrap();
    let takeover = data::is_interface_takeover(root, world_id, state.refactor_mode.as_deref());
    assert!(!takeover, "要是收圍欄的桌");
    let committed = data::state_commit::with_commit(root, world_id, |tx| {
        data::message_vars::begin_turn(tx, turn, None).unwrap();
        data::message_vars::apply_gm_block(
            tx,
            turn,
            data::message_vars::PendingMain {
                text: block.display.clone(),
                raw: Some(reply.clone()),
                truncated: false,
            },
            |world, stat| {
                crate::commands::chat::gm_commit_apply(
                    world,
                    stat,
                    &block,
                    "玩家",
                    &state.mechanism,
                    takeover,
                    false,
                )
            },
            |_, _| Vec::new(),
        )
    })
    .unwrap()
    .expect("提交成功")
    .state;
    data::settle_pending_turn(root, world_id).unwrap();
    committed
}

/// 套用當下就同步：平欄已有 place→卡欄位跟上；present 沒報過→卡欄位留開場值。
/// 兩欄收進 mirrors、拿掉規則、不列給 GM（不會被要求雙報）；其他卡欄位照舊有規則、列給 GM。
#[test]
fn apply_adopts_marks_and_syncs_before_any_turn() {
    let root = TestRoot::new("mirror-adopt");
    let (world_id, source_uid) = orc_world(root.path(), &[("place", "城门")]);
    let (outcome, selection) = orc_outcome(source_uid, "characters", true);
    apply(root.path(), &world_id, &outcome, &selection).unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(
        state.mechanism.mirrors,
        [
            (PLACE.to_owned(), "place".to_owned()),
            (ROSTER.to_owned(), "present".to_owned())
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(leaf(&state, PLACE), Some("城门"));
    assert_eq!(leaf(&state, ROSTER), Some("托伦 (Toren)"));
    for mirrored in [PLACE, ROSTER] {
        assert!(!state.mechanism.rules.contains_key(mirrored), "{mirrored}");
        assert!(!crate::mechanism::leaf_shown(
            &state.mechanism,
            &key(mirrored),
            "x",
            true
        ));
    }
    assert!(state.mechanism.rules.contains_key("驻守天数"));
    assert!(crate::mechanism::leaf_shown(
        &state.mechanism,
        &key("剧情阶段/要塞事件"),
        "x",
        false
    ));
    assert!(state
        .mechanism
        .rules
        .values()
        .all(|rule| rule.same_as.is_none()));
}

/// 同輪兩邊都寫：固定鍵勝；觸發條件與命中文本的 `{{state:当前环境}}` 讀到鏡像後的值。present 明確寫空：卡欄位跟著清空。
/// 收回上一句：卡欄位退回前一則快照的值。
#[test]
fn turn_mirrors_fixed_keys_and_retract_rewinds() {
    let root = TestRoot::new("mirror-turn");
    let (world_id, source_uid) = orc_world(root.path(), &[]);
    let (outcome, selection) = orc_outcome(source_uid, "characters", true);
    apply(root.path(), &world_id, &outcome, &selection).unwrap();

    let state = commit(
        root.path(),
        &world_id,
        "t1",
        "time: 清晨\nplace: 北坡\npresent: 托伦、布拉姆",
        r#"[{"op":"replace","path":"/当前环境","value":"食堂"}]"#,
    );
    assert_eq!(leaf(&state, PLACE), Some("北坡"));
    assert_eq!(leaf(&state, ROSTER), Some("托伦、布拉姆"));
    assert_eq!(
        state.state.triggers.get("north").map(String::as_str),
        Some("风雪更大了（北坡）"),
        "觸發條件與 {{state:}} 佔位都讀鏡像後的卡欄位"
    );

    let state = commit(
        root.path(),
        &world_id,
        "t2",
        "time: 午后\nplace: 指挥塔\npresent:",
        "[]",
    );
    assert_eq!(leaf(&state, PLACE), Some("指挥塔"));
    assert_eq!(leaf(&state, ROSTER), Some(""));

    assert!(data::pop_transcript(root.path(), &world_id, 0).unwrap());
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(leaf(&state, PLACE), Some("北坡"));
    assert_eq!(leaf(&state, ROSTER), Some("托伦、布拉姆"));
}

/// 玩家手改平欄（不經回合套用）：改地點帶過去；清空在場（平欄的鍵被刪掉）卡欄位也清空。
#[test]
fn manual_fixed_key_edit_reaches_the_mirror() {
    let root = TestRoot::new("mirror-manual");
    let (world_id, source_uid) = orc_world(root.path(), &[("present", "托伦")]);
    let (outcome, selection) = orc_outcome(source_uid, "characters", true);
    apply(root.path(), &world_id, &outcome, &selection).unwrap();
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(leaf(&state, ROSTER), Some("托伦"));
    let fields: std::collections::BTreeMap<String, String> = [
        ("place".to_owned(), "食堂".to_owned()),
        ("present".to_owned(), String::new()),
    ]
    .into_iter()
    .collect();
    for (name, value) in &fields {
        if value.is_empty() {
            state.state.table.remove(name);
        } else {
            state.state.table.insert(name.clone(), value.clone());
        }
    }
    crate::mechanism::sync_mirrors_from_edit(&mut state, &fields);
    assert_eq!(leaf(&state, PLACE), Some("食堂"));
    assert_eq!(leaf(&state, ROSTER), Some(""));
}

/// 撤銷重構：mirrors 退回空表、平欄原值回來、樹上搬進來的退掉。
#[test]
fn undo_refactor_restores_the_mirror_table() {
    let root = TestRoot::new("mirror-undo");
    let (world_id, source_uid) = orc_world(root.path(), &[("place", "城门")]);
    let (outcome, selection) = orc_outcome(source_uid, "characters", true);
    apply_recorded(root.path(), &world_id, &outcome, &selection);
    assert!(!data::read_state(root.path(), &world_id)
        .unwrap()
        .mechanism
        .mirrors
        .is_empty());
    crate::receipts::undo_last_import(root.path(), &world_id, &data::test_exclusive(&world_id))
        .unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(state.mechanism.mirrors.is_empty());
    assert!(state.mechanism.rules.is_empty() && !state.mechanism.incremental);
    assert!(state.state.tree.is_empty(), "{:?}", state.state.tree);
    assert_eq!(
        state.state.table.get(PLACE).map(String::as_str),
        Some("灰岩要塞城门内侧")
    );
}

/// 未標記：照舊是有規則、列給 GM 的一般卡欄位，固定鍵不會抄過去。
#[test]
fn unmarked_fields_are_untouched() {
    let root = TestRoot::new("mirror-unmarked");
    let (world_id, source_uid) = orc_world(root.path(), &[]);
    let (outcome, selection) = orc_outcome(source_uid, "characters", false);
    apply(root.path(), &world_id, &outcome, &selection).unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(state.mechanism.mirrors.is_empty());
    assert!(state.mechanism.rules.contains_key(PLACE));
    let state = commit(root.path(), &world_id, "t1", "place: 北坡", "[]");
    assert_eq!(leaf(&state, PLACE), Some("灰岩要塞城门内侧"));
}

/// 標記只收合格的：不是 place／present、不在樹上、萬用路徑、集合裡的欄位都丟掉標記、規則照舊。
#[test]
fn ineligible_marks_are_dropped_and_rules_kept() {
    let root = TestRoot::new("mirror-ineligible");
    let world_id = data::create_world(root.path(), "要塞").unwrap();
    let blank = data::read_state(root.path(), &world_id).unwrap();
    let mut state = blank.clone();
    state
        .state
        .tree
        .insert(PLACE.to_owned(), StateNode::Leaf("城门".to_owned()));
    state
        .mechanism
        .rules
        .insert(PLACE.to_owned(), marked(Some("time")));
    state
        .mechanism
        .rules
        .insert("不存在".to_owned(), marked(Some("place")));
    state
        .mechanism
        .rules
        .insert("人物.*.位置".to_owned(), marked(Some("place")));
    // 集合底下的欄位（具名集合的項目欄）
    state.state.tree.insert(
        "人物".to_owned(),
        StateNode::Branch(
            [(
                "托伦".to_owned(),
                StateNode::Branch(
                    [("位置".to_owned(), StateNode::Leaf("城门".to_owned()))]
                        .into_iter()
                        .collect(),
                ),
            )]
            .into_iter()
            .collect(),
        ),
    );
    state.mechanism.collections.insert(
        "人物".to_owned(),
        data::CollectionSpec {
            keyed: true,
            item: data::CollectionItem::Object,
            fields: Default::default(),
            value_type: None,
        },
    );
    state
        .mechanism
        .rules
        .insert("人物.托伦.位置".to_owned(), marked(Some("present")));
    assert!(crate::mechanism::adopt_mirror_marks(&mut state, true));
    assert!(state.mechanism.mirrors.is_empty());
    assert_eq!(state.mechanism.rules.len(), 4);
    assert!(state
        .mechanism
        .rules
        .values()
        .all(|rule| rule.same_as.is_none()));

    // 不收圍欄的桌（接管）：合格的標記一樣丟掉
    let mut takeover = blank;
    takeover
        .state
        .tree
        .insert(PLACE.to_owned(), StateNode::Leaf("城门".to_owned()));
    takeover
        .mechanism
        .rules
        .insert(PLACE.to_owned(), marked(Some("place")));
    crate::mechanism::adopt_mirror_marks(&mut takeover, false);
    assert!(takeover.mechanism.mirrors.is_empty());
    assert!(takeover.mechanism.rules[PLACE].same_as.is_none());
}

/// 盤點是唯一標記來源：AI 規則上自帶的 same_as 不算；MIRRORS 名字只認樹上頂層葉子整段精確相等，
/// 字形不同、不存在的行丟掉，規則照舊（開回報管道補的文字規則留著）。
#[test]
fn survey_mirrors_match_exact_leaf_names_only() {
    let root = TestRoot::new("mirror-exact");
    let (world_id, source_uid) = orc_world(root.path(), &[]);
    let (mut outcome, selection) = orc_outcome_with(
        source_uid,
        "characters",
        &[
            ("當前環境", "place"),
            ("不存在", "present"),
            (ROSTER, "present"),
        ],
    );
    outcome.entries[0]
        .rules
        .insert(PLACE.to_owned(), marked(Some("place")));
    apply(root.path(), &world_id, &outcome, &selection).unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(
        state.mechanism.mirrors,
        [(ROSTER.to_owned(), "present".to_owned())]
            .into_iter()
            .collect()
    );
    assert!(state.mechanism.rules.contains_key(PLACE));
    assert!(state
        .mechanism
        .rules
        .values()
        .all(|rule| rule.same_as.is_none()));
    assert!(!state.mechanism.rules.contains_key("不存在"));
    assert!(data::node_at(&state.state.tree, &key("不存在")).is_none());
}

/// 沒有規則與觸發（開回報管道提前返回、欄位留在平欄）：對不上樹葉就整份丟掉，不建空葉、不改平欄。
#[test]
fn survey_mirrors_without_tree_leaves_are_dropped() {
    let root = TestRoot::new("mirror-noleaf");
    let (world_id, source_uid) = orc_world(root.path(), &[]);
    let (mut outcome, selection) = orc_outcome(source_uid, "characters", true);
    outcome.entries[0].rules.clear();
    outcome.entries[0].triggers.clear();
    apply(root.path(), &world_id, &outcome, &selection).unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(state.mechanism.mirrors.is_empty());
    assert!(state.mechanism.rules.is_empty());
    assert!(state.state.tree.is_empty(), "{:?}", state.state.tree);
    assert_eq!(
        state.state.table.get(PLACE).map(String::as_str),
        Some("灰岩要塞城门内侧")
    );
}
