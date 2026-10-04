//! 包 2a 寫入核心的整合測試（計畫「測試清單／包 2」）：資料模型、初始化來源、模式交接、幕操作、匯入、
//! 版本與世代、卡寫、GM 回合、手改、收回／復原。故障注入用既有的改名／追加失敗守門與唯讀檔。
use super::*;
use crate::data::state_commit::with_commit;
use crate::data::test_support::TestRoot;
use crate::data::{self, RenameFailGuard, TranscriptEvent, TranscriptKind};
use serde_json::json;
use std::path::Path;

const USER: &str = "阿濤";

fn macros() -> Macros {
    Macros {
        user: USER.to_owned(),
        char: None,
    }
}

/// 一張載入 MVU 的卡，帶 `[initvar]`：角色.好感 10、角色.名字 {{user}}、角色.標籤 [1,"說明"]、金錢 "123"。
pub(super) fn mvu_world(root: &TestRoot) -> String {
    let world_id = data::create_world(root.path(), "MVU桌").unwrap();
    import_card(root.path(), &world_id, "貓娘", true);
    world_id
}

fn import_card(root: &Path, world_id: &str, name: &str, mvu: bool) {
    let card = json!({"data": {
        "name": name,
        "first_mes": "開場",
        "extensions": {"tavern_helper": {"scripts": [{
            "name": "MVU", "enabled": true, "type": "script",
            "content": if mvu { "import 'https://x/MagVarUpdate/bundle.js';" } else { "console.log(1)" }
        }]}},
        "character_book": {"entries": [{
            "comment": "[initvar]",
            "enabled": false,
            "content": "角色:\n  好感: 10\n  名字: \"{{user}}\"\n  標籤: [1, \"說明\"]\n金錢: \"123\""
        }]}
    }});
    crate::import::import_character(
        root,
        world_id,
        card.to_string().as_bytes(),
        "#336699",
        "zh-TW",
    )
    .unwrap();
}

fn event(kind: TranscriptKind, text: &str) -> TranscriptEvent {
    TranscriptEvent {
        ts: format!("ts-{text}"),
        speaker_id: String::new(),
        speaker_name: "GM".to_owned(),
        kind,
        text: text.to_owned(),
        raw: None,
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
    }
}

fn scene_of(root: &Path, world_id: &str) -> u64 {
    data::read_state(root, world_id).unwrap().current_scene
}

fn player(root: &Path, world_id: &str, text: &str) -> TranscriptEvent {
    let scene = scene_of(root, world_id);
    data::append_event(
        root,
        world_id,
        scene,
        &event(TranscriptKind::Player, text),
        None,
    )
    .unwrap()
    .0
}

pub(super) fn opening(root: &Path, world_id: &str) -> TranscriptEvent {
    let raw = "開場白";
    data::append_opening(
        root,
        world_id,
        0,
        "opening",
        raw,
        &crate::transport::extract_state_block(raw),
        USER,
    )
    .unwrap()
    .0
}

fn key(turn_id: &str, part: &str) -> TurnKey {
    TurnKey {
        turn_id: turn_id.to_owned(),
        part: part.to_owned(),
    }
}

pub(super) fn begin(root: &Path, world_id: &str, turn_id: &str) {
    with_commit(root, world_id, |tx| {
        begin_turn(tx, turn_id, Some(&macros()))
    })
    .unwrap();
}

fn commit(root: &Path, world_id: &str, turn_id: &str, edits: &[(&str, &str)]) -> bool {
    commit_parts(root, world_id, turn_id, edits, None)
}

/// 提交，`update` 有值就同一次登記一則變動紀錄（與 gm_narrate 一樣）。
fn commit_parts(
    root: &Path,
    world_id: &str,
    turn_id: &str,
    edits: &[(&str, &str)],
    update: Option<&str>,
) -> bool {
    with_commit(root, world_id, |tx| {
        let main = PendingMain {
            text: format!("GM {turn_id}"),
            raw: None,
            truncated: false,
        };
        apply_gm_block(
            tx,
            turn_id,
            main,
            |state, _| {
                for (path, value) in edits {
                    let path: Vec<String> = path.split('.').map(str::to_owned).collect();
                    data::set_tree_value(&mut state.state.tree, &path, value);
                }
                ((), None)
            },
            |_, _| {
                update
                    .map(|text| TurnSide {
                        part: "state_update".to_owned(),
                        kind: TranscriptKind::System,
                        text: text.to_owned(),
                        marker: Some(data::EventMarker::StateUpdate),
                    })
                    .into_iter()
                    .collect()
            },
        )
    })
    .unwrap()
    .is_some()
}

pub(super) fn append_part(
    root: &Path,
    world_id: &str,
    turn_id: &str,
    part: &str,
    text: &str,
) -> data::DataResult<TranscriptEvent> {
    let scene = scene_of(root, world_id);
    let kind = if part == PART_MAIN {
        TranscriptKind::Narration
    } else {
        TranscriptKind::System
    };
    data::append_event(
        root,
        world_id,
        scene,
        &event(kind, text),
        Some(&key(turn_id, part)),
    )
    .map(|(event, _)| event)
}

/// 一整輪 GM：開始、提交（改這些路徑）、落正文。
fn gm(root: &Path, world_id: &str, turn_id: &str, edits: &[(&str, &str)]) -> TranscriptEvent {
    begin(root, world_id, turn_id);
    assert!(commit(root, world_id, turn_id, edits));
    append_part(root, world_id, turn_id, PART_MAIN, &format!("GM {turn_id}")).unwrap()
}

fn table_of(event: &TranscriptEvent) -> Json {
    event.message_vars.as_ref().unwrap().parse().unwrap()
}

pub(super) fn stat_text(event: &TranscriptEvent) -> String {
    table_of(event).get("stat_data").unwrap().to_text()
}

fn effective(root: &Path, world_id: &str) -> Source {
    current_source(root, world_id, scene_of(root, world_id))
        .unwrap()
        .expect("變數模式")
}

fn effective_stat(root: &Path, world_id: &str) -> String {
    effective(root, world_id)
        .table()
        .get("stat_data")
        .map(Json::to_text)
        .unwrap_or_default()
}

fn mode(root: &Path, world_id: &str) -> Mode {
    read_control(root, world_id).unwrap().mode
}

fn target(event: &TranscriptEvent) -> CardWriteTarget {
    CardWriteTarget {
        id: event.id.clone(),
        index: None,
        legacy: None,
    }
}

fn write(
    root: &Path,
    world_id: &str,
    target: &CardWriteTarget,
    expected: Option<&str>,
    stat: &str,
) -> CardWrite {
    let table = parse_table(&format!(r#"{{"stat_data":{stat},"schema":{{"k":1}}}}"#)).unwrap();
    with_commit(root, world_id, |tx| {
        let generation = generation(tx);
        let scene = data::read_state(root, world_id).unwrap().current_scene;
        card_write(
            tx,
            Some(&macros()),
            generation,
            scene,
            target,
            expected,
            &table,
        )
    })
    .unwrap()
}

fn written(result: CardWrite) -> TranscriptEvent {
    match result {
        CardWrite::Ok { event } => event,
        other => panic!("應該寫成：{other:?}"),
    }
}

fn transcript(root: &Path, world_id: &str) -> Vec<TranscriptEvent> {
    data::read_transcript(root, world_id, scene_of(root, world_id)).unwrap()
}

fn tree_leaf(root: &Path, world_id: &str, path: &[&str]) -> Option<String> {
    let path: Vec<String> = path.iter().map(|segment| (*segment).to_owned()).collect();
    match data::node_at(&data::read_state(root, world_id).unwrap().state.tree, &path) {
        Some(data::StateNode::Leaf(text)) => Some(text.clone()),
        _ => None,
    }
}

// ── 資料模型與啟用

#[test]
fn opening_activates_variable_mode_and_materializes_initvar_with_types_and_macros() {
    let root = TestRoot::new("vars-opening");
    let world_id = mvu_world(&root);
    assert_eq!(mode(root.path(), &world_id), Mode::Tree);
    let opened = opening(root.path(), &world_id);
    assert_eq!(mode(root.path(), &world_id), Mode::Events);
    assert!(opened.id.is_some() && opened.vars_rev.is_some() && opened.vars_epoch.is_some());
    assert_eq!(
        stat_text(&opened),
        r#"{"角色":{"名字":"阿濤","好感":10,"標籤":[1,"說明"]},"金錢":123}"#
    );
    let table = table_of(&opened);
    assert_eq!(table.get("display_data"), table.get("stat_data"));
    assert_eq!(table.get("delta_data").unwrap().to_text(), "{}");
    // 開場就是初始化來源；投影入口把它轉回字串葉子給既有機制
    assert!(matches!(
        effective(root.path(), &world_id),
        Source::Event { .. }
    ));
    assert_eq!(
        tree_leaf(root.path(), &world_id, &["角色", "名字"]).as_deref(),
        Some(USER)
    );
    assert_eq!(
        tree_leaf(root.path(), &world_id, &["角色", "標籤"]).as_deref(),
        Some(r#"[1,"說明"]"#)
    );
}

#[test]
fn table_without_mvu_card_stays_in_tree_mode_and_read_state_is_unchanged() {
    let root = TestRoot::new("vars-tree-mode");
    let world_id = data::create_world(root.path(), "一般桌").unwrap();
    import_card(root.path(), &world_id, "路人", false);
    let before = data::read_state(root.path(), &world_id).unwrap();
    let opened = opening(root.path(), &world_id);
    assert!(opened.message_vars.is_none());
    assert_eq!(mode(root.path(), &world_id), Mode::Tree);
    let after = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(after.state.tree, before.state.tree);
    let read = gm(root.path(), &world_id, "t1", &[("角色.好感", "11")]);
    assert!(read.message_vars.is_none());
    assert_eq!(
        tree_leaf(root.path(), &world_id, &["角色", "好感"]).as_deref(),
        Some("11")
    );
}

#[test]
fn message_vars_round_trip_keeps_types_key_order_and_explicit_tables() {
    let root = TestRoot::new("vars-round-trip");
    let world_id = mvu_world(&root);
    let opened = opening(root.path(), &world_id);
    let reply = gm(root.path(), &world_id, "t1", &[]);
    // 卡寫：保留鍵順序、型別、自訂鍵；明確表可以沒有 stat_data
    let custom = r#"{"z":"123","a":[1,"說明"],"n":null,"b":false}"#;
    let event = written(write(
        root.path(),
        &world_id,
        &target(&reply),
        reply.vars_rev.as_deref(),
        custom,
    ));
    assert_eq!(stat_text(&event), custom);
    assert_eq!(
        table_of(&event).get("schema").unwrap().to_text(),
        r#"{"k":1}"#
    );
    let stored = transcript(root.path(), &world_id);
    assert_eq!(stat_text(stored.last().unwrap()), custom);
    let bare = parse_table(r#"{"display_data":{}}"#).unwrap();
    let explicit = with_commit(root.path(), &world_id, |tx| {
        let generation = generation(tx);
        card_write(
            tx,
            None,
            generation,
            0,
            &target(&opened),
            opened.vars_rev.as_deref(),
            &bare,
        )
    })
    .unwrap();
    let explicit = written(explicit);
    assert_eq!(table_of(&explicit).to_text(), r#"{"display_data":{}}"#);
    // 「沒有表」與「明確沒有 stat_data 的表」不同
    let player = player(root.path(), &world_id, "玩家");
    assert!(player.message_vars.is_none());
    assert!(explicit.message_vars.is_some());
}

#[test]
fn legacy_event_without_id_is_located_by_position_and_content_and_gets_an_id() {
    let root = TestRoot::new("vars-legacy");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    // 舊事件：直接寫進逐字稿、沒有 id；兩則同 ts 不同內文
    let path = data::scene::transcript_path(root.path(), &world_id, 0).unwrap();
    let mut legacy_a = event(TranscriptKind::Narration, "舊A");
    legacy_a.ts = "same".to_owned();
    let mut legacy_b = event(TranscriptKind::Narration, "舊B");
    legacy_b.ts = "same".to_owned();
    for legacy in [&legacy_a, &legacy_b] {
        let mut line = serde_json::to_vec(legacy).unwrap();
        line.push(b'\n');
        crate::data::commit_world_append(&path, &line).unwrap();
    }
    let events = transcript(root.path(), &world_id);
    assert!(events[1].id.is_none() && events[2].id.is_none());
    // 位置對、內容不對（拿 A 的內容指 B 的位置）＝ stale
    let wrong = CardWriteTarget {
        id: None,
        index: Some(2),
        legacy: Some(events[1].clone()),
    };
    assert!(matches!(
        write(root.path(), &world_id, &wrong, None, r#"{"x":1}"#),
        CardWrite::Stale { found: false, .. }
    ));
    let right = CardWriteTarget {
        id: None,
        index: Some(2),
        legacy: Some(events[2].clone()),
    };
    let event = written(write(root.path(), &world_id, &right, None, r#"{"x":1}"#));
    assert!(event.id.is_some());
    let events = transcript(root.path(), &world_id);
    assert_eq!(events[2].id, event.id);
    assert!(events[1].id.is_none() && events[1].message_vars.is_none());
}

// ── 初始化來源

#[test]
fn init_source_is_latest_table_by_position_not_by_write_time() {
    let root = TestRoot::new("vars-source-position");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    let first = gm(root.path(), &world_id, "t1", &[("角色.好感", "20")]);
    let second = gm(root.path(), &world_id, "t2", &[("角色.好感", "30")]);
    assert_eq!(effective_stat(root.path(), &world_id), stat_text(&second));
    // 改寫較早的樓不會讓它變成來源
    written(write(
        root.path(),
        &world_id,
        &target(&first),
        first.vars_rev.as_deref(),
        r#"{"改":1}"#,
    ));
    assert_eq!(effective_stat(root.path(), &world_id), stat_text(&second));
    // 寫入沒有表的樓 B（在 second 之後）：只動 B；B 成為來源只因為位置最新
    let b = player(root.path(), &world_id, "玩家 B");
    let b = written(write(
        root.path(),
        &world_id,
        &target(&b),
        None,
        r#"{"B":1}"#,
    ));
    assert_eq!(stat_text(&b), r#"{"B":1}"#);
    let events = transcript(root.path(), &world_id);
    let second_now = events.iter().find(|event| event.id == second.id).unwrap();
    assert_eq!(second_now.vars_rev, second.vars_rev);
    assert_eq!(effective_stat(root.path(), &world_id), r#"{"B":1}"#);
}

#[test]
fn popping_every_table_event_falls_back_to_scene_seed_not_cache() {
    let root = TestRoot::new("vars-pop-seed");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    gm(root.path(), &world_id, "t1", &[("角色.好感", "20")]);
    let seed = read_control(root.path(), &world_id).unwrap().scenes["0"]
        .seed
        .clone();
    // 快取故意寫壞：退回時不能讀它
    let mut cache = data::state::read_state_cache(root.path(), &world_id).unwrap();
    data::set_tree_value(&mut cache.state.tree, &["金錢".into()], "999999");
    data::write_state(root.path(), &world_id, &cache).unwrap();
    assert!(data::pop_transcript(root.path(), &world_id, 0).unwrap());
    assert!(data::pop_transcript(root.path(), &world_id, 0).unwrap());
    assert!(transcript(root.path(), &world_id).is_empty());
    assert!(matches!(
        effective(root.path(), &world_id),
        Source::Seed { .. }
    ));
    assert_eq!(
        effective(root.path(), &world_id).table().to_text(),
        seed.text()
    );
    assert_eq!(
        tree_leaf(root.path(), &world_id, &["金錢"]).as_deref(),
        Some("123")
    );
    // 快取已依投影重建
    let cache = data::state::read_state_cache(root.path(), &world_id).unwrap();
    assert_eq!(
        cache.state.tree,
        data::read_state(root.path(), &world_id).unwrap().state.tree
    );
}

#[test]
fn cache_write_failure_does_not_fail_the_commit_and_is_rebuilt_later() {
    let root = TestRoot::new("vars-cache-fail");
    let world_id = mvu_world(&root);
    let opened = opening(root.path(), &world_id);
    let event = {
        let _fail = state_write_breaks();
        written(write(
            root.path(),
            &world_id,
            &target(&opened),
            opened.vars_rev.as_deref(),
            r#"{"金錢":5}"#,
        ))
    };
    assert_state_intact(root.path(), &world_id);
    assert_eq!(stat_text(&event), r#"{"金錢":5}"#);
    // 快取沒跟上，但投影入口讀到的是有效值
    let cache = data::state::read_state_cache(root.path(), &world_id).unwrap();
    assert_ne!(
        cache.state.tree,
        data::read_state(root.path(), &world_id).unwrap().state.tree
    );
    assert_eq!(
        tree_leaf(root.path(), &world_id, &["金錢"]).as_deref(),
        Some("5")
    );
    // 下一次提交重建
    player(root.path(), &world_id, "下一句");
    let cache = data::state::read_state_cache(root.path(), &world_id).unwrap();
    assert_eq!(
        cache.state.tree,
        data::read_state(root.path(), &world_id).unwrap().state.tree
    );
}

/// state.json 下一次整檔寫入停在半截（打在原子替換的暫存檔上，不是寫前就拒絕）。
fn state_write_breaks() -> data::WriteFailGuard {
    data::WriteFailGuard::partial_ending("state.json.tmp", 1)
}

/// 半截寫入之後 state.json 仍是完整可讀的 JSON，暫存檔也清掉了。
fn assert_state_intact(root: &Path, world_id: &str) {
    data::state::read_state_cache(root, world_id).expect("state.json 仍可讀");
    assert!(!root
        .join(format!("worlds/{world_id}/state.json.tmp"))
        .exists());
}

// ── 模式交接

#[test]
fn old_table_first_activation_materializes_current_values_and_tail_card_write_counts() {
    let root = TestRoot::new("vars-old-table");
    let world_id = mvu_world(&root);
    // 舊桌：沒啟用前就有對話，狀態樹被改過（例如舊版 GM 回合）
    player(root.path(), &world_id, "很久以前");
    let mut cache = data::read_state(root.path(), &world_id).unwrap();
    data::set_tree_value(&mut cache.state.tree, &["角色".into(), "好感".into()], "55");
    data::write_state(root.path(), &world_id, &cache).unwrap();
    let tail = player(root.path(), &world_id, "尾樓");
    assert_eq!(mode(root.path(), &world_id), Mode::Tree);
    // 觸發啟用的那筆卡寫寫在尾樓
    let event = written(write(
        root.path(),
        &world_id,
        &target(&tail),
        None,
        r#"{"x":1}"#,
    ));
    assert_eq!(mode(root.path(), &world_id), Mode::Events);
    let seed = read_control(root.path(), &world_id).unwrap().scenes["0"]
        .seed
        .parse()
        .unwrap();
    assert_eq!(
        seed.get("stat_data")
            .unwrap()
            .get("角色")
            .unwrap()
            .get("好感")
            .unwrap()
            .to_text(),
        "55"
    );
    assert_eq!(
        event.vars_epoch.as_deref(),
        Some(
            read_control(root.path(), &world_id).unwrap().scenes["0"]
                .epoch
                .as_str()
        )
    );
    assert_eq!(effective_stat(root.path(), &world_id), r#"{"x":1}"#);
}

#[test]
fn control_write_failure_leaves_mode_unchanged() {
    let root = TestRoot::new("vars-control-fail");
    let world_id = mvu_world(&root);
    let tail = player(root.path(), &world_id, "尾樓");
    {
        let _fail = RenameFailGuard::fail(1);
        assert!(with_commit(root.path(), &world_id, |tx| {
            let generation = generation(tx);
            card_write(
                tx,
                None,
                generation,
                0,
                &target(&tail),
                None,
                &parse_table("{}").unwrap(),
            )
        })
        .is_err());
    }
    assert_eq!(mode(root.path(), &world_id), Mode::Tree);
    assert!(transcript(root.path(), &world_id)[0].message_vars.is_none());
    // 重做成功
    written(write(
        root.path(),
        &world_id,
        &target(&tail),
        None,
        r#"{"x":1}"#,
    ));
    assert_eq!(mode(root.path(), &world_id), Mode::Events);
}

#[test]
fn handover_to_tree_crash_between_tree_and_control_is_redone_correctly() {
    let root = TestRoot::new("vars-handover-crash");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    gm(root.path(), &world_id, "t1", &[("角色.好感", "77")]);
    {
        let _fail = RenameFailGuard::fail(1);
        assert!(with_commit(root.path(), &world_id, handover_to_tree).is_err());
    }
    // 樹已寫、控制檔沒寫：仍是 events，有效狀態照舊從事件來
    assert_eq!(mode(root.path(), &world_id), Mode::Events);
    assert_eq!(
        tree_leaf(root.path(), &world_id, &["角色", "好感"]).as_deref(),
        Some("77")
    );
    with_commit(root.path(), &world_id, handover_to_tree).unwrap();
    assert_eq!(mode(root.path(), &world_id), Mode::Tree);
    let cache = data::state::read_state_cache(root.path(), &world_id).unwrap();
    assert_eq!(
        data::node_at(&cache.state.tree, &["角色".into(), "好感".into()]),
        Some(&data::StateNode::Leaf("77".into()))
    );
}

#[test]
fn refactor_round_trip_keeps_values_edited_while_refactored_and_schema() {
    let root = TestRoot::new("vars-refactor-trip");
    let world_id = mvu_world(&root);
    let opened = opening(root.path(), &world_id);
    // schema 等自訂鍵在表上
    let event = written(write(
        root.path(),
        &world_id,
        &target(&opened),
        opened.vars_rev.as_deref(),
        r#"{"角色":{"好感":10},"金錢":"123"}"#,
    ));
    let old_epoch = event.vars_epoch.clone();
    // 重構套用：events → tree；重構期間改樹
    with_commit(root.path(), &world_id, handover_to_tree).unwrap();
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    data::set_tree_value(&mut state.state.tree, &["角色".into(), "好感".into()], "99");
    data::write_state(root.path(), &world_id, &state).unwrap();
    // 停用腳本／讀不到卡片介面不會切回（這裡模擬：樹模式下讀不到資格就不動）
    // 重構復原：tree → events，以當下樹物化新種子、新 epoch
    assert!(with_commit(root.path(), &world_id, |tx| ensure_active(tx, None)).unwrap());
    let control = read_control(root.path(), &world_id).unwrap();
    assert_ne!(Some(control.scenes["0"].epoch.clone()), old_epoch);
    let seed = control.scenes["0"].seed.parse().unwrap();
    assert_eq!(seed.get("schema").unwrap().to_text(), r#"{"k":1}"#);
    // 重構期間改的值保留；字串 "123" 沒被動過，沿用原 JSON 字串
    assert_eq!(
        seed.get("stat_data").unwrap().to_text(),
        r#"{"角色":{"好感":99},"金錢":"123"}"#
    );
    // 舊 epoch 的表仍在事件上，但不是初始化來源
    assert!(matches!(
        effective(root.path(), &world_id),
        Source::Seed { .. }
    ));
    assert!(transcript(root.path(), &world_id)[0].message_vars.is_some());
}

#[test]
fn events_mode_does_not_switch_back_when_card_script_is_gone() {
    let root = TestRoot::new("vars-no-switch-back");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    // 卡片介面讀不到（刪掉原始卡檔）
    for meta in data::list_characters(root.path(), &world_id).unwrap() {
        let md = data::character_path(root.path(), &world_id, &meta.id).unwrap();
        let _ = std::fs::remove_file(md.with_extension("import.json"));
        let _ = std::fs::remove_file(md.with_extension("png"));
    }
    assert!(eligible_char(root.path(), &world_id, None).is_none());
    let reply = gm(root.path(), &world_id, "t1", &[("角色.好感", "12")]);
    assert_eq!(mode(root.path(), &world_id), Mode::Events);
    assert!(reply.message_vars.is_some());
}

// ── 幕

#[test]
fn next_scene_seeds_from_old_scene_and_revert_drops_child_seed() {
    let root = TestRoot::new("vars-next-scene");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    let last = gm(root.path(), &world_id, "t1", &[("角色.好感", "40")]);
    let next = data::begin_next_scene(root.path(), &world_id, "摘要", None).unwrap();
    assert_eq!(next, 1);
    let control = read_control(root.path(), &world_id).unwrap();
    assert_ne!(control.scenes["1"].epoch, control.scenes["0"].epoch);
    assert_eq!(control.scenes["1"].seed.parse().unwrap(), table_of(&last));
    // 新幕開頭的摘要帶一份種子表（第一個 GM 回覆前卡片讀得到值）；不讀上一幕
    let summary = &data::read_transcript(root.path(), &world_id, 1).unwrap()[0];
    assert_eq!(
        summary.message_vars.as_ref(),
        Some(&control.scenes["1"].seed)
    );
    assert_eq!(
        summary.vars_epoch.as_deref(),
        Some(control.scenes["1"].epoch.as_str())
    );
    assert!(matches!(
        effective(root.path(), &world_id),
        Source::Event { index: 0, .. }
    ));
    let child = gm(root.path(), &world_id, "t2", &[("角色.好感", "41")]);
    assert_eq!(
        child.vars_epoch.as_deref(),
        Some(control.scenes["1"].epoch.as_str())
    );
    // 子幕玩過就不能退；收回到只剩摘要再退
    assert!(data::revert_scene(root.path(), &world_id).is_err());
    assert!(data::pop_transcript(root.path(), &world_id, 1).unwrap());
    assert_eq!(data::revert_scene(root.path(), &world_id).unwrap(), 0);
    let control = read_control(root.path(), &world_id).unwrap();
    assert!(!control.scenes.contains_key("1"));
    // 父幕原封不動
    assert_eq!(effective_stat(root.path(), &world_id), stat_text(&last));
}

#[test]
fn next_scene_failure_at_each_step_never_leaves_current_scene_without_seed() {
    let root = TestRoot::new("vars-next-fault");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    let last = gm(root.path(), &world_id, "t1", &[("角色.好感", "40")]);
    let parent_epoch = read_control(root.path(), &world_id).unwrap().scenes["0"]
        .epoch
        .clone();
    // ① 控制檔寫失敗
    {
        let _fail = RenameFailGuard::fail(1);
        assert!(data::begin_next_scene(root.path(), &world_id, "摘要", None).is_err());
    }
    assert_eq!(scene_of(root.path(), &world_id), 0);
    // ② 摘要追加失敗
    {
        let _fail = data::AppendFailGuard::partial(1);
        assert!(data::begin_next_scene(root.path(), &world_id, "摘要", None).is_err());
    }
    assert_eq!(scene_of(root.path(), &world_id), 0);
    // ③ current_scene 寫失敗
    {
        let _fail = state_write_breaks();
        assert!(data::begin_next_scene(root.path(), &world_id, "摘要", None).is_err());
    }
    assert_state_intact(root.path(), &world_id);
    assert_eq!(scene_of(root.path(), &world_id), 0);
    // 目前幕（0）的種子與 epoch 從沒被覆寫，有效狀態照舊
    assert_eq!(
        read_control(root.path(), &world_id).unwrap().scenes["0"].epoch,
        parent_epoch
    );
    assert_eq!(effective_stat(root.path(), &world_id), stat_text(&last));
    // 重試成功；新幕只有一則摘要（殘留被覆寫）
    assert_eq!(
        data::begin_next_scene(root.path(), &world_id, "摘要", None).unwrap(),
        1
    );
    assert_eq!(
        data::read_transcript(root.path(), &world_id, 1)
            .unwrap()
            .len(),
        1
    );
    assert!(read_control(root.path(), &world_id)
        .unwrap()
        .active(1)
        .is_some());
}

#[test]
fn fork_reassigns_ids_and_epoch_and_failures_leave_published_scenes_alone() {
    let root = TestRoot::new("vars-fork");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    let scene0 = gm(root.path(), &world_id, "t1", &[("角色.好感", "40")]);
    data::begin_next_scene(root.path(), &world_id, "摘要", None).unwrap();
    gm(root.path(), &world_id, "t2", &[("角色.好感", "50")]);
    let before = read_control(root.path(), &world_id).unwrap();
    // ② 控制檔寫失敗：current_scene 沒動，已發布的幕原封不動
    {
        let _fail = RenameFailGuard::fail(1);
        assert!(data::fork_scene(root.path(), &world_id, 0).is_err());
    }
    assert_eq!(scene_of(root.path(), &world_id), 1);
    assert_eq!(read_control(root.path(), &world_id).unwrap(), before);
    // ③ current_scene 寫失敗
    {
        let _fail = state_write_breaks();
        assert!(data::fork_scene(root.path(), &world_id, 0).is_err());
    }
    assert_state_intact(root.path(), &world_id);
    assert_eq!(scene_of(root.path(), &world_id), 1);
    assert_eq!(
        read_control(root.path(), &world_id).unwrap().scenes["1"],
        before.scenes["1"]
    );
    // 重試成功
    assert_eq!(data::fork_scene(root.path(), &world_id, 0).unwrap(), 2);
    let control = read_control(root.path(), &world_id).unwrap();
    let copied = data::read_transcript(root.path(), &world_id, 2).unwrap();
    let source = data::read_transcript(root.path(), &world_id, 0).unwrap();
    for (copy, original) in copied.iter().zip(&source) {
        assert_ne!(copy.id, original.id);
        if original.message_vars.is_some() {
            assert_ne!(copy.vars_rev, original.vars_rev);
            assert_eq!(
                copy.vars_epoch.as_deref(),
                Some(control.scenes["2"].epoch.as_str())
            );
        }
    }
    assert_eq!(control.scenes["2"].seed, control.scenes["0"].seed);
    // 新幕的來源是複製來的最新表；原幕不受影響
    assert_eq!(effective_stat(root.path(), &world_id), stat_text(&scene0));
    assert_eq!(control.scenes["0"], before.scenes["0"]);
}

#[test]
fn child_scene_refactor_round_trip_then_revert_keeps_parent_table_valid() {
    let root = TestRoot::new("vars-child-refactor");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    let parent = gm(root.path(), &world_id, "t1", &[("角色.好感", "40")]);
    data::begin_next_scene(root.path(), &world_id, "摘要", None).unwrap();
    // 換幕結算會把沒登場的卡自動隱藏（隱藏的卡不出卡片介面）；這裡讓 MVU 卡留在桌上
    for meta in data::list_characters(root.path(), &world_id).unwrap() {
        data::set_character_auto_hidden(root.path(), &world_id, &meta.id, false).unwrap();
    }
    let parent_vars = read_control(root.path(), &world_id).unwrap().scenes["0"].clone();
    // 子幕重構往返：events → tree → events（只換目前這一幕的 epoch 與種子）
    with_commit(root.path(), &world_id, handover_to_tree).unwrap();
    assert!(with_commit(root.path(), &world_id, |tx| ensure_active(tx, None)).unwrap());
    let control = read_control(root.path(), &world_id).unwrap();
    assert_eq!(control.scenes["0"], parent_vars);
    // 退回父幕：父幕的表仍是有效來源
    assert_eq!(data::revert_scene(root.path(), &world_id).unwrap(), 0);
    assert_eq!(effective_stat(root.path(), &world_id), stat_text(&parent));
    assert!(matches!(
        effective(root.path(), &world_id),
        Source::Event { .. }
    ));
}

#[test]
fn revert_switch_failure_leaves_child_seed_untouched() {
    let root = TestRoot::new("vars-revert-fault");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    data::begin_next_scene(root.path(), &world_id, "摘要", None).unwrap();
    let child = read_control(root.path(), &world_id).unwrap().scenes["1"].clone();
    {
        let _fail = state_write_breaks();
        assert!(data::revert_scene(root.path(), &world_id).is_err());
    }
    assert_state_intact(root.path(), &world_id);
    assert_eq!(
        read_control(root.path(), &world_id).unwrap().scenes["1"],
        child
    );
    // 切回沒發布成：子幕逐字稿還在、桌仍在子幕，沒有指向已刪幕的狀態
    assert_eq!(scene_of(root.path(), &world_id), 1);
    assert_eq!(
        data::read_transcript(root.path(), &world_id, 1)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(data::revert_scene(root.path(), &world_id).unwrap(), 0);
    assert!(data::read_transcript(root.path(), &world_id, 1)
        .unwrap()
        .is_empty());
}

// ── 匯入

#[test]
fn import_fill_and_undo_only_touch_the_init_source() {
    let root = TestRoot::new("vars-import");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    let reply = gm(root.path(), &world_id, "t1", &[("角色.好感", "40")]);
    let seed_before = read_control(root.path(), &world_id).unwrap().scenes["0"]
        .seed
        .clone();
    let opening_before = transcript(root.path(), &world_id)[0].clone();
    // 第二張卡的 initvar：只補缺的鍵，不覆蓋（好感 40 不倒回 10），新鍵清 metadata、代換巨集
    let card = json!({"data": {"name": "第二張", "character_book": {"entries": [{
        "comment": "[initvar]", "enabled": false,
        "content": "角色:\n  好感: 10\n新角色:\n  $meta:\n    extensible: true\n  稱呼: \"{{user}}\""
    }]}}});
    let held = data::test_exclusive(&world_id);
    crate::import::import_character_file(
        root.path(),
        &world_id,
        card.to_string().as_bytes(),
        "#000000",
        "zh-TW",
        &held,
    )
    .unwrap();
    let events = transcript(root.path(), &world_id);
    let source = events.iter().find(|event| event.id == reply.id).unwrap();
    assert_ne!(source.vars_rev, reply.vars_rev);
    let stat = table_of(source).get("stat_data").unwrap().clone();
    assert_eq!(
        stat.get("角色").unwrap().get("好感").unwrap().to_text(),
        "40"
    );
    assert_eq!(stat.get("新角色").unwrap().to_text(), r#"{"稱呼":"阿濤"}"#);
    // 其他樓與種子不動
    assert_eq!(events[0], opening_before);
    assert_eq!(
        read_control(root.path(), &world_id).unwrap().scenes["0"].seed,
        seed_before
    );
    // 復原匯入：從復原當下的初始化來源移除加進的分支
    crate::receipts::undo_last_import(root.path(), &world_id, &held).unwrap();
    drop(held);
    let stat = effective(root.path(), &world_id)
        .table()
        .get("stat_data")
        .unwrap()
        .clone();
    assert!(stat.get("新角色").is_none());
    assert_eq!(
        stat.get("角色").unwrap().get("好感").unwrap().to_text(),
        "40"
    );
}

// ── 版本、世代與收回／復原

#[test]
fn pop_then_restore_issues_new_token_and_old_in_flight_write_is_rejected() {
    let root = TestRoot::new("vars-aba");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    let reply = gm(root.path(), &world_id, "t1", &[("角色.好感", "40")]);
    // 收回最高版本那則再復原：同一則、同 id，版本 token 換新、不撞號
    assert!(data::pop_transcript(root.path(), &world_id, 0).unwrap());
    let restored = data::append_event(root.path(), &world_id, 0, &reply, None)
        .unwrap()
        .0;
    assert_eq!(restored.id, reply.id);
    assert_ne!(restored.vars_rev, reply.vars_rev);
    assert_eq!(restored.message_vars, reply.message_vars);
    assert_eq!(effective_stat(root.path(), &world_id), stat_text(&reply));
    // 收回前發出的在途寫入（預期舊版本）被拒，附上權威值
    match write(
        root.path(),
        &world_id,
        &target(&reply),
        reply.vars_rev.as_deref(),
        r#"{"x":1}"#,
    ) {
        CardWrite::Stale { found, rev, table } => {
            assert!(found);
            assert_eq!(rev, restored.vars_rev);
            assert_eq!(
                table.as_deref(),
                restored.message_vars.as_ref().map(VarsTable::text)
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn whole_table_restore_bumps_generation_and_rejects_old_requests() {
    let root = TestRoot::new("vars-generation");
    let world_id = mvu_world(&root);
    let opened = opening(root.path(), &world_id);
    let old_generation = with_commit(root.path(), &world_id, generation);
    begin(root.path(), &world_id, "t1");
    // 整桌還原（帶回相同事件 id 與 token）：世代加一、回合紀錄清掉
    let checkpoint = data::opening_checkpoint(root.path(), &world_id, 0).unwrap();
    assert!(checkpoint.restore());
    assert!(!with_commit(root.path(), &world_id, busy));
    let table = parse_table(r#"{"stat_data":{"x":1}}"#).unwrap();
    let result = with_commit(root.path(), &world_id, |tx| {
        card_write(
            tx,
            None,
            old_generation,
            0,
            &target(&opened),
            opened.vars_rev.as_deref(),
            &table,
        )
    })
    .unwrap();
    assert!(matches!(result, CardWrite::Stale { .. }));
    for swap in [
        world_swapped as fn(&Path, &str),
        |root: &Path, world: &str| {
            let _ = data::open_world(root, world);
        },
    ] {
        let before = with_commit(root.path(), &world_id, generation);
        swap(root.path(), &world_id);
        assert_eq!(with_commit(root.path(), &world_id, generation), before + 1);
    }
    // 現世代照常寫得進去
    written(write(
        root.path(),
        &world_id,
        &target(&opened),
        opened.vars_rev.as_deref(),
        r#"{"x":2}"#,
    ));
}

#[test]
fn writing_a_historical_floor_only_changes_that_floor() {
    let root = TestRoot::new("vars-history");
    let world_id = mvu_world(&root);
    let opened = opening(root.path(), &world_id);
    let latest = gm(root.path(), &world_id, "t1", &[("角色.好感", "40")]);
    let event = written(write(
        root.path(),
        &world_id,
        &target(&opened),
        opened.vars_rev.as_deref(),
        r#"{"舊":1}"#,
    ));
    let events = transcript(root.path(), &world_id);
    assert_eq!(events[0].message_vars, event.message_vars);
    assert_eq!(events[1].message_vars, latest.message_vars);
    assert_eq!(events[1].vars_rev, latest.vars_rev);
    assert_eq!(effective_stat(root.path(), &world_id), stat_text(&latest));
    // 版本不符：stale 附權威值，不寫
    match write(
        root.path(),
        &world_id,
        &target(&opened),
        opened.vars_rev.as_deref(),
        r#"{"再":1}"#,
    ) {
        CardWrite::Stale {
            found: true, rev, ..
        } => assert_eq!(rev, event.vars_rev),
        other => panic!("{other:?}"),
    }
}

#[test]
fn writes_outside_the_current_scene_or_on_non_variable_tables_are_refused() {
    let root = TestRoot::new("vars-refuse");
    let world_id = data::create_world(root.path(), "一般桌").unwrap();
    import_card(root.path(), &world_id, "路人", false);
    let tail = player(root.path(), &world_id, "尾樓");
    assert!(matches!(
        write(root.path(), &world_id, &target(&tail), None, "{}"),
        CardWrite::Rejected { .. }
    ));
    assert_eq!(mode(root.path(), &world_id), Mode::Tree);
    let world_id = mvu_world(&root);
    let opened = opening(root.path(), &world_id);
    let table = parse_table("{}").unwrap();
    let result = with_commit(root.path(), &world_id, |tx| {
        let generation = generation(tx);
        card_write(
            tx,
            None,
            generation,
            5,
            &target(&opened),
            opened.vars_rev.as_deref(),
            &table,
        )
    })
    .unwrap();
    assert!(matches!(result, CardWrite::Stale { .. }));
}

// ── GM 回合

#[test]
fn gm_turn_blocks_writes_until_main_lands_and_keeps_untouched_types() {
    let root = TestRoot::new("vars-gm-turn");
    let world_id = mvu_world(&root);
    let opened = opening(root.path(), &world_id);
    begin(root.path(), &world_id, "t1");
    // 生成中：卡寫 busy（附權威值）、手改 busy
    assert!(matches!(
        write(
            root.path(),
            &world_id,
            &target(&opened),
            opened.vars_rev.as_deref(),
            "{}"
        ),
        CardWrite::Busy { found: true, .. }
    ));
    assert!(with_commit(root.path(), &world_id, busy));
    // turn_id 不符的提交被拒、不改狀態
    assert!(!commit(
        root.path(),
        &world_id,
        "別的",
        &[("角色.好感", "1")]
    ));
    assert!(commit(
        root.path(),
        &world_id,
        "t1",
        &[("角色.好感", "20"), ("角色.新", "x")]
    ));
    // 提交後到前端落檔前也擋；有效狀態還沒變
    assert!(with_commit(root.path(), &world_id, busy));
    assert_eq!(effective_stat(root.path(), &world_id), stat_text(&opened));
    // 不符的 turn_id 落檔被拒
    assert!(append_part(root.path(), &world_id, "別的", PART_MAIN, "x").is_err());
    // 變動紀錄先落：永遠不掛表，busy 照舊
    let update = append_part(root.path(), &world_id, "t1", "state_update", "好感：20").unwrap();
    assert!(update.message_vars.is_none());
    assert!(with_commit(root.path(), &world_id, busy));
    let main = append_part(root.path(), &world_id, "t1", PART_MAIN, "GM 回覆").unwrap();
    assert!(!with_commit(root.path(), &world_id, busy));
    // GM 改到的換新值（還原型別），沒改到的保留原 JSON 值（[1,"說明"]、名字字串）
    assert_eq!(
        stat_text(&main),
        r#"{"角色":{"名字":"阿濤","好感":20,"標籤":[1,"說明"],"新":"x"},"金錢":123}"#
    );
    assert_eq!(main.turn_key, Some(key("t1", PART_MAIN)));
    assert_eq!(
        main.state.as_ref().unwrap().tree,
        data::read_state(root.path(), &world_id).unwrap().state.tree
    );
    assert_eq!(effective_stat(root.path(), &world_id), stat_text(&main));
    // 同鍵重試：回原事件、不再追加；main 與 state_update 各自冪等
    let count = transcript(root.path(), &world_id).len();
    assert_eq!(
        append_part(root.path(), &world_id, "t1", PART_MAIN, "重試").unwrap(),
        main
    );
    assert_eq!(
        append_part(root.path(), &world_id, "t1", "state_update", "重試").unwrap(),
        update
    );
    assert_eq!(transcript(root.path(), &world_id).len(), count);
}

#[test]
fn gm_main_append_failure_can_be_retried_and_attaches_the_table_once() {
    let root = TestRoot::new("vars-gm-retry");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    begin(root.path(), &world_id, "t1");
    assert!(commit(root.path(), &world_id, "t1", &[("角色.好感", "21")]));
    {
        let _fail = data::AppendFailGuard::partial(1);
        assert!(append_part(root.path(), &world_id, "t1", PART_MAIN, "GM").is_err());
    }
    assert!(with_commit(root.path(), &world_id, busy));
    let main = append_part(root.path(), &world_id, "t1", PART_MAIN, "GM").unwrap();
    assert!(main.message_vars.is_some());
    assert_eq!(transcript(root.path(), &world_id).len(), 2);
}

#[test]
fn gm_abort_releases_busy_and_half_text_lands_without_table() {
    let root = TestRoot::new("vars-gm-abort");
    let world_id = mvu_world(&root);
    let opened = opening(root.path(), &world_id);
    begin(root.path(), &world_id, "t1");
    // 別的 turn_id 的中止不算數
    with_commit(root.path(), &world_id, |tx| {
        finish_turn(tx, "別的", None).unwrap()
    });
    assert!(with_commit(root.path(), &world_id, busy));
    with_commit(root.path(), &world_id, |tx| {
        finish_turn(tx, "t1", None).unwrap()
    });
    assert!(!with_commit(root.path(), &world_id, busy));
    assert_eq!(
        with_commit(root.path(), &world_id, phases)["t1"],
        Phase::Aborted
    );
    // 中止後的提交被拒
    assert!(!commit(root.path(), &world_id, "t1", &[("角色.好感", "1")]));
    let half = append_part(root.path(), &world_id, "t1", PART_MAIN, "半截").unwrap();
    assert!(half.message_vars.is_none());
    assert_eq!(effective_stat(root.path(), &world_id), stat_text(&opened));
    assert_eq!(
        append_part(root.path(), &world_id, "t1", PART_MAIN, "半截").unwrap(),
        half
    );
}

#[test]
fn world_swap_clears_turn_record() {
    let root = TestRoot::new("vars-gm-swap");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    begin(root.path(), &world_id, "t1");
    world_swapped(root.path(), &world_id);
    assert!(!with_commit(root.path(), &world_id, busy));
    assert!(!commit(root.path(), &world_id, "t1", &[("角色.好感", "1")]));
    assert!(append_part(root.path(), &world_id, "t1", PART_MAIN, "x").is_err());
}

#[test]
fn partial_overwrite_failure_leaves_the_scene_intact() {
    let root = TestRoot::new("vars-partial-write");
    let world_id = mvu_world(&root);
    let opened = opening(root.path(), &world_id);
    let reply = gm(root.path(), &world_id, "t1", &[("角色.好感", "20")]);
    let path = data::scene::transcript_path(root.path(), &world_id, 0).unwrap();
    let before = std::fs::read(&path).unwrap();
    {
        let _fail = data::WriteFailGuard::partial(1);
        let table = parse_table(r#"{"stat_data":{"x":1}}"#).unwrap();
        assert!(with_commit(root.path(), &world_id, |tx| {
            let generation = generation(tx);
            card_write(
                tx,
                None,
                generation,
                0,
                &target(&opened),
                opened.vars_rev.as_deref(),
                &table,
            )
        })
        .is_err());
    }
    {
        let _fail = data::WriteFailGuard::partial(1);
        assert!(data::pop_transcript(root.path(), &world_id, 0).is_err());
    }
    {
        let _fail = data::WriteFailGuard::partial(1);
        assert!(with_commit(root.path(), &world_id, |tx| {
            edit_effective_tree(tx, true, false, |tree| {
                data::set_tree_value(tree, &["金錢".to_owned()], "1")
            })
        })
        .is_err());
    }
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(transcript(root.path(), &world_id).len(), 2);
    assert_eq!(effective_stat(root.path(), &world_id), stat_text(&reply));
    assert!(!path.with_file_name("0.jsonl.tmp").exists());
}

#[test]
fn concurrent_state_updates_and_card_writes_do_not_lose_each_other() {
    let root = TestRoot::new("vars-race");
    let world_id = mvu_world(&root);
    let opened = opening(root.path(), &world_id);
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (path, world_id) = (root.path().to_path_buf(), world_id.clone());
        handles.push(std::thread::spawn(move || {
            for _ in 0..25 {
                data::update_state(&path, &world_id, |state| {
                    let count: u32 = state
                        .state
                        .table
                        .get("次數")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(0);
                    state
                        .state
                        .table
                        .insert("次數".to_owned(), (count + 1).to_string());
                    Ok(Some(()))
                })
                .unwrap();
            }
        }));
    }
    // 同時卡寫：每一筆都以拿到的最新版本寫成，快取重建不蓋掉並行的欄位更新
    let mut rev = opened.vars_rev.clone();
    for i in 0..20 {
        let stat = format!(r#"{{"i":{i}}}"#);
        let event = written(write(
            root.path(),
            &world_id,
            &target(&opened),
            rev.as_deref(),
            &stat,
        ));
        rev = event.vars_rev;
    }
    for handle in handles {
        handle.join().unwrap();
    }
    let state = data::state::read_state_cache(root.path(), &world_id).unwrap();
    assert_eq!(
        state.state.table.get("次數").map(String::as_str),
        Some("50")
    );
    assert_eq!(effective_stat(root.path(), &world_id), r#"{"i":19}"#);
}

#[test]
fn gm_turn_activates_only_on_commit_and_is_bound_to_scene() {
    let root = TestRoot::new("vars-gm-activate");
    let world_id = mvu_world(&root);
    // 樹模式、符合條件的桌：開始時不啟用，失敗的回合不改逐樓語意
    begin(root.path(), &world_id, "t0");
    assert_eq!(mode(root.path(), &world_id), Mode::Tree);
    with_commit(root.path(), &world_id, |tx| {
        finish_turn(tx, "t0", None).unwrap()
    });
    assert_eq!(mode(root.path(), &world_id), Mode::Tree);
    // 提交時才啟用，正文掛新表
    let reply = gm(root.path(), &world_id, "t1", &[("角色.好感", "12")]);
    assert_eq!(mode(root.path(), &world_id), Mode::Events);
    assert!(stat_text(&reply).contains(r#""好感":12"#));
    // 回合綁幕：生成中擋換幕、分岔；換過世代的回合提交被拒
    begin(root.path(), &world_id, "t2");
    assert!(data::begin_next_scene(root.path(), &world_id, "摘要", None).is_err());
    assert!(data::fork_scene(root.path(), &world_id, 0).is_err());
    world_swapped(root.path(), &world_id);
    assert!(!commit(root.path(), &world_id, "t2", &[("角色.好感", "1")]));
}

#[test]
fn generating_turn_cannot_be_replaced_by_a_new_one() {
    let root = TestRoot::new("vars-gm-overwrite");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    begin(root.path(), &world_id, "t1");
    assert!(with_commit(root.path(), &world_id, |tx| begin_turn(tx, "t2", None)).is_err());
    assert_eq!(
        with_commit(root.path(), &world_id, phases)["t1"],
        Phase::Generating
    );
    assert!(with_commit(root.path(), &world_id, settle_previous_turn).is_err());
}

#[test]
fn unlanded_main_is_landed_by_the_next_turn_and_late_retries_do_not_duplicate() {
    let root = TestRoot::new("vars-gm-handover");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    begin(root.path(), &world_id, "t1");
    assert!(commit(root.path(), &world_id, "t1", &[("角色.好感", "33")]));
    // 前端一直沒落正文：下一個 GM 回合開始時後端用留下的正文代落、表掛上
    begin(root.path(), &world_id, "t2");
    let events = transcript(root.path(), &world_id);
    let landed = events.last().unwrap().clone();
    assert_eq!(landed.text, "GM t1");
    assert_eq!(landed.turn_key, Some(key("t1", PART_MAIN)));
    assert!(stat_text(&landed).contains(r#""好感":33"#));
    // 前端晚到的同鍵重試：回代落的那則，不再追加
    let count = events.len();
    let retried = append_part(root.path(), &world_id, "t1", PART_MAIN, "GM t1").unwrap();
    assert_eq!(retried, landed);
    assert_eq!(transcript(root.path(), &world_id).len(), count);
    // 角色回合開始也做同樣的交接
    assert!(commit(root.path(), &world_id, "t2", &[("角色.好感", "34")]));
    with_commit(root.path(), &world_id, settle_previous_turn).unwrap();
    let last = transcript(root.path(), &world_id).last().unwrap().clone();
    assert!(stat_text(&last).contains(r#""好感":34"#));
    assert!(!with_commit(root.path(), &world_id, busy));
}

fn texts(root: &Path, world_id: &str) -> Vec<String> {
    transcript(root, world_id)
        .into_iter()
        .map(|event| event.text)
        .collect()
}

#[test]
fn next_player_send_lands_the_lost_turn_first_in_order_with_its_state_update() {
    let root = TestRoot::new("vars-gm-send-order");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    let (asked, offset) = data::append_event(
        root.path(),
        &world_id,
        0,
        &event(TranscriptKind::Player, "我推門"),
        None,
    )
    .unwrap();
    begin(root.path(), &world_id, "t1");
    // 正文與變動紀錄在同一次提交登記：放鎖前待落清單就完整
    assert!(commit_parts(
        root.path(),
        &world_id,
        "t1",
        &[("角色.好感", "33")],
        Some("角色.好感：33")
    ));
    // 前端照實際送出流程：正文同鍵重試三次都失敗，這輪算失敗
    {
        let _fail = data::AppendFailGuard::partial(3);
        for _ in 0..3 {
            assert!(append_part(root.path(), &world_id, "t1", PART_MAIN, "GM t1").is_err());
        }
    }
    // 回覆已產生只是沒落檔：觸發這輪的玩家句不能當「沒回成」收掉
    assert!(!data::discard_unanswered_player(
        root.path(),
        &world_id,
        0,
        offset.unwrap(),
        &asked.ts,
        &asked.text
    )
    .unwrap());
    // 玩家接著送下一句：玩家句追加前，上一輪的正文與變動紀錄先依序代落
    let sent = player(root.path(), &world_id, "下一句");
    assert_eq!(
        texts(root.path(), &world_id),
        ["開場白", "我推門", "GM t1", "角色.好感：33", "下一句"]
    );
    let events = transcript(root.path(), &world_id);
    assert!(stat_text(&events[2]).contains(r#""好感":33"#));
    assert_eq!(events[3].marker, Some(data::EventMarker::StateUpdate));
    assert_eq!(events[3].turn_key, Some(key("t1", "state_update")));
    assert!(events[3].message_vars.is_none());
    assert_eq!(events[4], sent);
    assert!(!with_commit(root.path(), &world_id, busy));
    // 前端晚到的同鍵重試回代落的那幾則，不再追加；下一個回合照常開始
    assert_eq!(
        append_part(root.path(), &world_id, "t1", PART_MAIN, "GM t1").unwrap(),
        events[2]
    );
    assert_eq!(
        append_part(root.path(), &world_id, "t1", "state_update", "x").unwrap(),
        events[3]
    );
    begin(root.path(), &world_id, "t2");
    assert_eq!(transcript(root.path(), &world_id).len(), 5);
}

#[test]
fn state_update_left_behind_after_main_lands_is_landed_before_the_next_event() {
    let root = TestRoot::new("vars-gm-update-left");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    begin(root.path(), &world_id, "t1");
    // 正文與變動紀錄在同一次提交登記：放鎖前待落清單就完整
    assert!(commit_parts(
        root.path(),
        &world_id,
        "t1",
        &[("角色.好感", "34")],
        Some("角色.好感：34")
    ));
    append_part(root.path(), &world_id, "t1", PART_MAIN, "GM t1").unwrap();
    {
        let _fail = data::AppendFailGuard::partial(1);
        assert!(append_part(
            root.path(),
            &world_id,
            "t1",
            "state_update",
            "角色.好感：34"
        )
        .is_err());
    }
    player(root.path(), &world_id, "下一句");
    assert_eq!(
        texts(root.path(), &world_id),
        ["開場白", "GM t1", "角色.好感：34", "下一句"]
    );
}

#[test]
fn aborted_half_text_is_kept_and_recovered_before_the_next_event() {
    let root = TestRoot::new("vars-gm-abort-recover");
    let world_id = mvu_world(&root);
    let opened = opening(root.path(), &world_id);
    begin(root.path(), &world_id, "t1");
    let half = PendingMain {
        text: "半截".to_owned(),
        raw: None,
        truncated: true,
    };
    with_commit(root.path(), &world_id, |tx| {
        finish_turn(tx, "t1", Some(half))
    })
    .unwrap();
    assert!(!with_commit(root.path(), &world_id, busy));
    // 前端三次追加都失敗：半截仍留在回合紀錄，下一筆新事件前代落（不帶表、標記截斷）
    {
        let _fail = data::AppendFailGuard::partial(3);
        for _ in 0..3 {
            assert!(append_part(root.path(), &world_id, "t1", PART_MAIN, "半截").is_err());
        }
    }
    player(root.path(), &world_id, "下一句");
    assert_eq!(texts(root.path(), &world_id), ["開場白", "半截", "下一句"]);
    let landed = transcript(root.path(), &world_id)[1].clone();
    assert!(landed.truncated);
    assert!(landed.message_vars.is_none());
    assert_eq!(landed.turn_key, Some(key("t1", PART_MAIN)));
    assert_eq!(effective_stat(root.path(), &world_id), stat_text(&opened));
    assert_eq!(
        append_part(root.path(), &world_id, "t1", PART_MAIN, "半截").unwrap(),
        landed
    );
    // 下一個回合開始時代落也一樣；空白半截不落
    begin(root.path(), &world_id, "t2");
    let half = PendingMain {
        text: "又一截".to_owned(),
        raw: None,
        truncated: true,
    };
    with_commit(root.path(), &world_id, |tx| {
        finish_turn(tx, "t2", Some(half))
    })
    .unwrap();
    begin(root.path(), &world_id, "t3");
    let blank = PendingMain {
        text: "  ".to_owned(),
        raw: None,
        truncated: true,
    };
    with_commit(root.path(), &world_id, |tx| {
        finish_turn(tx, "t3", Some(blank))
    })
    .unwrap();
    player(root.path(), &world_id, "再一句");
    assert_eq!(
        texts(root.path(), &world_id),
        ["開場白", "半截", "下一句", "又一截", "再一句"]
    );
}

#[test]
fn only_same_turn_appends_skip_the_handover_and_every_other_entry_lands_the_turn_first() {
    let root = TestRoot::new("vars-gm-entries");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    let ticket = with_commit(root.path(), &world_id, |tx| {
        begin_turn(tx, "t1", Some(&macros()))
    })
    .unwrap();
    assert!(commit(root.path(), &world_id, "t1", &[("角色.好感", "35")]));
    // 同回合的登場紀錄帶憑證：不交接，照原本順序排在正文前
    data::append_within_turn(
        root.path(),
        &world_id,
        0,
        &event(TranscriptKind::System, "登場"),
        &ticket,
    )
    .unwrap();
    assert_eq!(texts(root.path(), &world_id), ["開場白", "登場"]);
    assert!(with_commit(root.path(), &world_id, busy));
    // 公開的 append_transcript 是其他新事件：先代落正文
    data::append_transcript(
        root.path(),
        &world_id,
        0,
        &event(TranscriptKind::System, "別的"),
    )
    .unwrap();
    assert_eq!(
        texts(root.path(), &world_id),
        ["開場白", "登場", "GM t1", "別的"]
    );
    // 回合已落檔後，舊憑證就只是一般新事件
    data::append_within_turn(
        root.path(),
        &world_id,
        0,
        &event(TranscriptKind::System, "晚到"),
        &ticket,
    )
    .unwrap();
    assert_eq!(transcript(root.path(), &world_id).len(), 5);
}

#[test]
fn direct_append_opening_lands_the_pending_turn_first_and_keeps_its_new_values() {
    let root = TestRoot::new("vars-gm-opening-direct");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    begin(root.path(), &world_id, "t1");
    assert!(commit(root.path(), &world_id, "t1", &[("角色.好感", "37")]));
    // 直接走 append_opening（不經 post_opening_text）：開場表要以代落的 GM 新值為底
    let posted = opening(root.path(), &world_id);
    assert_eq!(texts(root.path(), &world_id), ["開場白", "GM t1", "開場白"]);
    assert!(stat_text(&posted).contains(r#""好感":37"#));
    assert_eq!(effective_stat(root.path(), &world_id), stat_text(&posted));
}

#[test]
fn posting_an_opening_or_changing_scene_lands_the_pending_turn_first() {
    let root = TestRoot::new("vars-gm-opening-handover");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    begin(root.path(), &world_id, "t1");
    assert!(commit(root.path(), &world_id, "t1", &[("角色.好感", "36")]));
    // 前端正文沒落成就貼新開場：舊回覆先代落，不會之後才插到開場後面、讓舊表重新變成有效狀態
    let held = data::test_exclusive(&world_id);
    crate::import::post_opening_text(
        root.path(),
        &world_id,
        0,
        "ts-新開場",
        "新開場",
        "zh-TW",
        None,
        None,
        &held,
    )
    .unwrap();
    drop(held);
    assert_eq!(texts(root.path(), &world_id), ["開場白", "GM t1", "新開場"]);
    let latest = transcript(root.path(), &world_id).last().unwrap().clone();
    assert_eq!(effective_stat(root.path(), &world_id), stat_text(&latest));
    assert!(!with_commit(root.path(), &world_id, has_unlanded_reply));
    // 中止留下的半截沒落成就換幕：先落進原來那一幕
    begin(root.path(), &world_id, "t2");
    let half = PendingMain {
        text: "半截".to_owned(),
        raw: None,
        truncated: true,
    };
    with_commit(root.path(), &world_id, |tx| {
        finish_turn(tx, "t2", Some(half))
    })
    .unwrap();
    data::begin_next_scene(root.path(), &world_id, "摘要", None).unwrap();
    let scene0: Vec<String> = data::read_transcript(root.path(), &world_id, 0)
        .unwrap()
        .into_iter()
        .map(|event| event.text)
        .collect();
    assert_eq!(scene0.last().map(String::as_str), Some("半截"));
}

#[test]
fn landed_main_that_was_popped_is_not_resurrected_by_a_late_retry() {
    let root = TestRoot::new("vars-gm-resurrect");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    gm(root.path(), &world_id, "t1", &[("角色.好感", "33")]);
    assert!(data::pop_transcript(root.path(), &world_id, 0).unwrap());
    assert!(append_part(root.path(), &world_id, "t1", PART_MAIN, "GM t1").is_err());
    assert_eq!(transcript(root.path(), &world_id).len(), 1);
}

#[test]
fn gm_commit_cache_failure_is_reported_and_the_table_still_lands() {
    let root = TestRoot::new("vars-gm-cache-fail");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    begin(root.path(), &world_id, "t1");
    // state.json 寫到一半失敗（不是寫前就拒絕）：原檔不動、回報錯誤，正文照樣帶表落檔
    let committed = {
        let _fail = state_write_breaks();
        with_commit(root.path(), &world_id, |tx| {
            let main = PendingMain {
                text: "GM".to_owned(),
                raw: None,
                truncated: false,
            };
            apply_gm_block(
                tx,
                "t1",
                main,
                |state, _| {
                    data::set_tree_value(
                        &mut state.state.tree,
                        &["角色".into(), "好感".into()],
                        "44",
                    );
                    ((), None)
                },
                |_, _| Vec::new(),
            )
        })
    };
    assert_state_intact(root.path(), &world_id);
    assert!(committed.unwrap().unwrap().error.is_some());
    let main = append_part(root.path(), &world_id, "t1", PART_MAIN, "GM").unwrap();
    assert!(stat_text(&main).contains(r#""好感":44"#));
    // 樹模式（state.json 是權威）同樣：寫到一半失敗不毀檔，之後的代落讀得到它
    let world_id = data::create_world(root.path(), "一般桌").unwrap();
    import_card(root.path(), &world_id, "路人", false);
    begin(root.path(), &world_id, "t2");
    let committed = {
        let _fail = state_write_breaks();
        with_commit(root.path(), &world_id, |tx| {
            let main = PendingMain {
                text: "GM t2".to_owned(),
                raw: None,
                truncated: false,
            };
            apply_gm_block(tx, "t2", main, |_, _| ((), None), |_, _| Vec::new())
        })
    };
    assert!(committed.unwrap().unwrap().error.is_some());
    assert_state_intact(root.path(), &world_id);
    player(root.path(), &world_id, "下一句");
    let texts: Vec<String> = transcript(root.path(), &world_id)
        .into_iter()
        .map(|event| event.text)
        .collect();
    assert_eq!(texts, ["GM t2", "下一句"]);
}

#[test]
fn restored_table_sent_as_text_keeps_key_order() {
    let value = json!({
        "ts": "t", "speaker_id": "", "speaker_name": "GM", "kind": "narration", "text": "x",
        "message_vars": r#"{"z":1,"a":{"y":2,"b":3}}"#
    });
    let event: TranscriptEvent = serde_json::from_value(value).unwrap();
    assert_eq!(
        event.message_vars.unwrap().text(),
        r#"{"z":1,"a":{"y":2,"b":3}}"#
    );
    let bad = json!({"ts": "t", "speaker_id": "", "speaker_name": "GM", "kind": "narration",
        "text": "x", "message_vars": "[1]"});
    assert!(serde_json::from_value::<TranscriptEvent>(bad).is_err());
}

// ── 面板手改

#[test]
fn manual_edit_changes_the_source_event_and_keeps_string_types() {
    let root = TestRoot::new("vars-manual");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    let reply = written(write(
        root.path(),
        &world_id,
        &target(&player(root.path(), &world_id, "尾")),
        None,
        r#"{"錢":"100","hp":5,"pair":[3,"說明"]}"#,
    ));
    let edit = |path: &str, value: &str| {
        with_commit(root.path(), &world_id, |tx| {
            edit_effective_tree(tx, true, false, |tree| {
                data::set_tree_value(tree, &[path.to_owned()], value)
            })
        })
        .unwrap()
    };
    assert_eq!(edit("錢", "150"), Some(true));
    assert_eq!(edit("hp", "7"), Some(true));
    assert_eq!(edit("pair", "4"), Some(true));
    let events = transcript(root.path(), &world_id);
    let source = events.last().unwrap();
    assert_eq!(source.id, reply.id);
    assert_ne!(source.vars_rev, reply.vars_rev);
    assert_eq!(
        stat_text(source),
        r#"{"錢":"150","hp":7,"pair":[4,"說明"]}"#
    );
    // 沒有帶表事件時改的是這一幕的種子
    let world_id = mvu_world(&root);
    let tail = player(root.path(), &world_id, "尾");
    with_commit(root.path(), &world_id, |tx| {
        ensure_active(tx, Some(&macros()))
    })
    .unwrap();
    assert!(matches!(
        effective(root.path(), &world_id),
        Source::Seed { .. }
    ));
    with_commit(root.path(), &world_id, |tx| {
        edit_effective_tree(tx, true, false, |tree| {
            data::set_tree_value(tree, &["金錢".to_owned()], "7")
        })
    })
    .unwrap();
    let seed = read_control(root.path(), &world_id).unwrap().scenes["0"]
        .seed
        .parse()
        .unwrap();
    assert_eq!(
        seed.get("stat_data")
            .unwrap()
            .get("金錢")
            .unwrap()
            .to_text(),
        "7"
    );
    assert!(transcript(root.path(), &world_id)[0].message_vars.is_none());
    assert_eq!(tail.message_vars, None);
}

#[test]
fn append_opening_uses_the_locked_append_and_set_player_card_touches_one_field() {
    let root = TestRoot::new("vars-misc");
    let world_id = mvu_world(&root);
    // append_opening 在鎖內呼叫 tx 版追加：會自鎖的話這裡會 panic
    opening(root.path(), &world_id);
    let before = data::state::read_state_cache(root.path(), &world_id).unwrap();
    data::set_player_card(root.path(), &world_id, Some("卡".to_owned())).unwrap();
    let after = data::state::read_state_cache(root.path(), &world_id).unwrap();
    assert_eq!(after.player_card_id.as_deref(), Some("卡"));
    assert_eq!(
        data::WorldState {
            player_card_id: None,
            ..after
        },
        before
    );
    data::set_player_card(root.path(), &world_id, None).unwrap();
    assert!(data::state::read_state_cache(root.path(), &world_id)
        .unwrap()
        .player_card_id
        .is_none());
}

/// 效能閘（計畫 8.1）：500 則、每則帶約 25 KB 表（含同大小的狀態快照）的逐字稿，讀取與整檔改寫。
/// 預設略過（debug 版沒有參考價值）；`cargo test --release perf_gate -- --ignored --nocapture` 實跑。
#[test]
#[ignore]
fn perf_gate_500_events_with_25kb_tables() {
    use std::time::Instant;
    let root = TestRoot::new("vars-perf");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    let mut stat = String::from("{");
    for a in 0..6 {
        if a > 0 {
            stat.push(',');
        }
        stat.push_str(&format!("\"角色{a}\":{{"));
        for b in 0..6 {
            if b > 0 {
                stat.push(',');
            }
            stat.push_str(&format!("\"分類{b}\":{{"));
            for c in 0..10 {
                if c > 0 {
                    stat.push(',');
                }
                let value = match c % 3 {
                    0 => format!("{}", a * 100 + c),
                    1 => format!("[\"說明{}\",\"備註\"]", "描述".repeat(3)),
                    _ => format!("\"值{}\"", "內容".repeat(4)),
                };
                stat.push_str(&format!("\"欄位{a}{b}{c}\":{value}"));
            }
            stat.push('}');
        }
        stat.push('}');
    }
    stat.push('}');
    let table_text = format!(r#"{{"stat_data":{stat},"display_data":{stat},"delta_data":{{}}}}"#);
    let table = parse_table(&table_text).unwrap();
    println!("table bytes {}", table_text.len());
    let path = data::scene::transcript_path(root.path(), &world_id, 0).unwrap();
    let tree = convert::stat_to_tree(table.get("stat_data"));
    let mut buffer = Vec::new();
    for index in 0..500 {
        let mut item = event(TranscriptKind::Narration, &"正文".repeat(300));
        item.ts = format!("ts{index}");
        item.id = Some(new_token());
        item.state = Some(data::TableState {
            tree: tree.clone(),
            ..Default::default()
        });
        item.message_vars = Some(VarsTable::from_json(&table));
        item.vars_rev = Some(new_token());
        item.vars_epoch = Some("old".to_owned());
        buffer.extend(serde_json::to_vec(&item).unwrap());
        buffer.push(b'\n');
    }
    data::commit_world_write(&path, &buffer).unwrap();
    println!("file MB {:.2}", buffer.len() as f64 / 1e6);
    let events = transcript(root.path(), &world_id);
    let target = target(&events[250]);
    for round in 0..3 {
        let started = Instant::now();
        let read = transcript(root.path(), &world_id);
        let read_time = started.elapsed();
        assert_eq!(read.len(), 500);
        let rev = read[250].vars_rev.clone();
        let started = Instant::now();
        written(write(
            root.path(),
            &world_id,
            &target,
            rev.as_deref(),
            &stat,
        ));
        let write_time = started.elapsed();
        let started = Instant::now();
        let _ = data::read_state(root.path(), &world_id).unwrap();
        let project_time = started.elapsed();
        println!("round {round}: read {read_time:?} card write {write_time:?} read_state {project_time:?}");
    }
}
