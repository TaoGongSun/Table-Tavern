//! takeover-fence-scene-keys：先貼開場白（狀態區塊帶 时间／地点／在场）再介面重構，寫殼時清掉平欄
//! time／place／present。接管回覆不收圍欄，留著就凍在開場值，在場判定（登場、狀態裁切、換幕結算）與
//! GM「目前狀態」（逐行列平欄）會一直讀舊值。沒寫殼的套用（介面沒勾、圍欄還在用的增量桌）照舊保留。
use super::super::test_support::*;
use super::super::*;
use super::takeover_flat_keys::{commit_reply, interface_outcome, interface_selection, rule};
use crate::data::{self, FieldKind, StateNode};
use std::collections::BTreeSet;
use std::path::Path;

const OPENING: &str = "驛站的雪下了一夜。\n<StatusData>\n时间: 第1天 清晨\n地点: 北境驿站 前院\n在场: 阿古拉\n粮草: 320\n</StatusData>";
const SCENE_KEYS: [&str; 3] = ["place", "present", "time"];

fn card(name: &str) -> data::CharacterCard {
    data::CharacterCard {
        id: data::new_id(),
        name: name.to_owned(),
        color: "#888".to_owned(),
        avatar: "🙂".to_owned(),
        tier: data::Tier::Balanced,
        show_image: true,
        archived: false,
        gen_prompt: String::new(),
        public_md: String::new(),
        private_md: String::new(),
    }
}

fn scene_keys(state: &data::WorldState) -> Vec<&str> {
    SCENE_KEYS
        .into_iter()
        .filter(|key| state.state.table.contains_key(*key))
        .collect()
}

/// 開場白照非接管桌（Table scope）套用；回傳 (world_id, 主角色卡 id)
fn opened_world(root: &Path) -> (String, String) {
    let world_id = data::create_world(root, "北境驿站").unwrap();
    let host = card("北境驿站");
    data::write_character(root, &world_id, &host).unwrap();
    let mut state = data::read_state(root, &world_id).unwrap();
    let block = crate::transport::extract_state_block(OPENING);
    crate::mechanism::apply_block(&mut state, &block, "玩家");
    data::write_state(root, &world_id, &state).unwrap();
    let state = data::read_state(root, &world_id).unwrap();
    assert_eq!(
        scene_keys(&state),
        SCENE_KEYS,
        "开场三鍵落平欄：{:?}",
        state.state.table
    );
    (world_id, host.id)
}

fn outcome(shell: bool) -> RefactorOutcome {
    let mut outcome = interface_outcome(
        serde_json::json!({ "时间": "第1天 清晨", "地点": "北境驿站 前院", "粮草": "320" }),
        &[
            ("时间", rule(FieldKind::Text)),
            ("地点", rule(FieldKind::Text)),
        ],
    );
    if !shell {
        outcome.interface.as_mut().unwrap().shell = None;
    }
    outcome
}

/// 寫殼後三鍵消失：GM 更新照常進樹，平欄不再有舊值（「目前狀態」逐行列平欄，就沒有矛盾行）；
/// present 缺席＝接管桌設計上的未知：登場退回正文比對、狀態不裁。
#[test]
fn writing_the_shell_clears_scene_keys_and_presence_falls_back_to_unknown() {
    let root = TestRoot::new("fence-scene-clear");
    let (world_id, _) = opened_world(root.path());
    apply(
        root.path(),
        &world_id,
        &outcome(true),
        &interface_selection(),
    )
    .unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(scene_keys(&state).is_empty(), "{:?}", state.state.table);

    let (state, problems) = commit_reply(
        root.path(),
        &world_id,
        r#"[{"op":"replace","path":"/地点","value":"驿丞房"}]"#,
    );
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(
        state.state.tree.get("地点"),
        Some(&StateNode::Leaf("驿丞房".to_owned()))
    );
    assert!(state.state.table.is_empty(), "{:?}", state.state.table);

    let present = state.state.table.get("present").map(String::as_str);
    let person = data::WorldbookEntry {
        is_person: true,
        ..serde_json::from_value(serde_json::json!({"uid": 1, "title": "沈鬱", "keys": [], "content": "沈鬱的設定", "constant": false, "order": 0, "disabled": false, "visibility": serde_json::to_value(data::Visibility::Public).unwrap()}))
            .unwrap()
    };
    let arrived = crate::transport::detect_new_arrivals(
        std::slice::from_ref(&person),
        &[],
        present,
        "沈鬱推門而入，抖落一身雪。",
        &BTreeSet::new(),
    );
    assert_eq!(arrived.len(), 1, "present 缺席：正文出場就登場");

    let mut scoped = state.state.clone();
    let branch = |name: &str| {
        (
            name.to_owned(),
            StateNode::Branch(std::collections::BTreeMap::from([(
                "好感".to_owned(),
                StateNode::Leaf("10".to_owned()),
            )])),
        )
    };
    scoped.tree.insert(
        "人物".to_owned(),
        StateNode::Branch(std::collections::BTreeMap::from([
            branch("阿古拉"),
            branch("沈鬱"),
        ])),
    );
    let scope = crate::transport::state_scope(
        &scoped,
        &state.mechanism,
        &[card("阿古拉"), card("沈鬱")],
        None,
        &Default::default(),
        false,
        &["阿古拉".to_owned(), "沈鬱".to_owned()],
    );
    assert!(
        scope.hidden.is_empty(),
        "present 缺席不裁：{:?}",
        scope.hidden
    );
}

/// 接管桌 present 缺席不結算：主角色卡換幕後留在主區（舊值還在時會被當成沒出現收進隱藏區）
#[test]
fn host_card_stays_visible_at_scene_change() {
    let root = TestRoot::new("fence-scene-settle");
    let (world_id, host_id) = opened_world(root.path());
    apply(
        root.path(),
        &world_id,
        &outcome(true),
        &interface_selection(),
    )
    .unwrap();
    assert!(!data::requires_present_report(
        root.path(),
        &world_id,
        Some("interface")
    ));
    data::begin_next_scene(root.path(), &world_id, "摘要", None).unwrap();
    let hidden = data::list_characters(root.path(), &world_id)
        .unwrap()
        .into_iter()
        .find(|meta| meta.id == host_id)
        .unwrap()
        .auto_hidden;
    assert!(!hidden);
}

/// 玩家套用前改過地點：清掉的是改過的值，撤銷重構還原的也是改過的值
#[test]
fn edited_place_is_cleared_and_undo_restores_it() {
    let root = TestRoot::new("fence-scene-edited");
    let (world_id, _) = opened_world(root.path());
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    state
        .state
        .table
        .insert("place".to_owned(), "驿站后院".to_owned());
    data::write_state(root.path(), &world_id, &state).unwrap();
    apply_recorded(
        root.path(),
        &world_id,
        &outcome(true),
        &interface_selection(),
    );
    assert!(scene_keys(&data::read_state(root.path(), &world_id).unwrap()).is_empty());

    crate::receipts::undo_last_import(root.path(), &world_id, &data::test_exclusive(&world_id))
        .unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(scene_keys(&state), SCENE_KEYS);
    assert_eq!(
        state.state.table.get("place").map(String::as_str),
        Some("驿站后院")
    );
}

/// 沒寫殼的增量桌圍欄還在用，三鍵照舊留平欄
#[test]
fn incremental_table_without_shell_keeps_scene_keys() {
    let root = TestRoot::new("fence-scene-no-shell");
    let (world_id, _) = opened_world(root.path());
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    state.mechanism.incremental = true;
    data::write_state(root.path(), &world_id, &state).unwrap();
    apply(
        root.path(),
        &world_id,
        &outcome(false),
        &interface_selection(),
    )
    .unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(!data::is_interface_takeover(
        root.path(),
        &world_id,
        state.refactor_mode.as_deref()
    ));
    assert!(state.state.tree.contains_key("地点"), "介面樹有重建");
    assert_eq!(scene_keys(&state), SCENE_KEYS);
}

/// 介面沒勾：不寫殼也不清
#[test]
fn unselected_interface_keeps_scene_keys() {
    let root = TestRoot::new("fence-scene-unselected");
    let (world_id, _) = opened_world(root.path());
    let selection = RefactorSelection {
        apply_interface: false,
        ..interface_selection()
    };
    apply(root.path(), &world_id, &outcome(true), &selection).unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(!data::is_interface_takeover(
        root.path(),
        &world_id,
        state.refactor_mode.as_deref()
    ));
    assert_eq!(scene_keys(&state), SCENE_KEYS);
}
