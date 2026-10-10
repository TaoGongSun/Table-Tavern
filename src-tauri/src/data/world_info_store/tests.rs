//! 計時存放與落地（方案四之 3）：讀寫與壞檔、則數、落地與 pending、GM／角色正文清 pending、`finish_turn`、
//! 結算（寫入中四種變數情形、已送出成敗）、待回報檔、原子寫、換幕／分岔／退幕、刪條目清計時、跨視角。
use super::landing::ConflictReason;
use super::*;
use crate::data::card_vars::{self, Layer, LayerWrite};
use crate::data::message_vars::{self, PendingMain, TurnKey, PART_MAIN};
use crate::data::state_commit::with_commit;
use crate::data::test_support::{worldbook_entry, write_worldbook_fixture, TestRoot};
use crate::data::{self, RenameFailGuard, WriteFailGuard};
use crate::world_info::timed::{TimedEffect, WiTimed};
use serde_json::json;

const GM: Perspective = Perspective::Gm;

fn ev(kind: TranscriptKind, text: &str) -> TranscriptEvent {
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
        action_id: None,
    }
}

fn effect(start: f64, end: f64, protected: bool) -> TimedEffect {
    TimedEffect {
        start,
        end,
        protected,
        confidential: false,
    }
}

/// sticky 表：uid → (start, end, protected)
fn sticky(items: &[(&str, f64, f64, bool)]) -> WiTimed {
    WiTimed {
        sticky: items
            .iter()
            .map(|(uid, start, end, protected)| {
                ((*uid).to_owned(), effect(*start, *end, *protected))
            })
            .collect(),
        cooldown: Default::default(),
    }
}

/// 書上放 uid 0..count 的條目（讀計時時才不會被當成已刪條目清掉）。
fn world_with_book(root: &TestRoot, count: u64) -> String {
    let world_id = data::create_world(root.path(), "計時桌").unwrap();
    let entries: serde_json::Map<String, serde_json::Value> = (0..count)
        .map(|uid| {
            (
                uid.to_string(),
                json!({"uid": uid, "key": ["霧"], "content": format!("條目{uid}")}),
            )
        })
        .collect();
    write_worldbook_fixture(root, &world_id, serde_json::Value::Object(entries));
    world_id
}

fn append(root: &TestRoot, world_id: &str, scene: u64, kind: TranscriptKind, text: &str) {
    data::append_event(root.path(), world_id, scene, &ev(kind, text), None).unwrap();
}

fn scene_file(root: &TestRoot, world_id: &str, scene: u64) -> std::path::PathBuf {
    root.path()
        .join(format!("worlds/{world_id}/world-info/{scene}.json"))
}

fn pending_of(root: &TestRoot, world_id: &str, scene: u64) -> Option<Pending> {
    read_scene(root.path(), world_id, scene).unwrap().pending
}

fn table(root: &TestRoot, world_id: &str, scene: u64, perspective: &Perspective) -> WiTimed {
    read_timed(root.path(), world_id, scene, perspective).unwrap()
}

/// 落地到已送出：表換成 `landed`。
fn land_sent(root: &TestRoot, world_id: &str, scene: u64, turn: &str, landed: &WiTimed) {
    begin_landing(root.path(), world_id, scene, turn, &GM, landed).unwrap();
    mark_sent(root.path(), world_id, scene, turn).unwrap();
}

#[test]
fn missing_file_is_empty_and_corrupt_file_errors_without_overwrite() {
    let root = TestRoot::new("wi-store-corrupt");
    let world_id = world_with_book(&root, 2);
    assert_eq!(table(&root, &world_id, 0, &GM), WiTimed::default());

    let path = scene_file(&root, &world_id, 0);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"{not json").unwrap();
    assert!(read_timed(root.path(), &world_id, 0, &GM).is_err());
    assert!(begin_landing(root.path(), &world_id, 0, "t", &GM, &sticky(&[])).is_err());
    assert!(settle_pending(root.path(), &world_id, 0).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"{not json");

    std::fs::write(&path, br#"{"version": 2, "perspectives": {}}"#).unwrap();
    assert!(read_timed(root.path(), &world_id, 0, &GM).is_err());
}

#[test]
fn scan_tables_round_trip_and_fractional_end_rounds_up() {
    let mut timed = sticky(&[("1", 3.0, 5.5, false)]);
    timed.sticky.get_mut("1").unwrap().confidential = true;
    timed
        .cooldown
        .insert("2".to_owned(), effect(4.0, 1e300, true));
    let stored = StoredTimed::from_scan(&timed);
    assert_eq!(stored.sticky["1"].end, 6);
    assert!(stored.sticky["1"].confidential);
    assert_eq!(stored.cooldown["2"].end, i64::MAX);
    let back = stored.to_scan();
    assert_eq!(back.sticky["1"].start, 3.0);
    assert_eq!(back.sticky["1"].end, 6.0);
    assert!(back.sticky["1"].confidential);
    // 則數是整數：>= 5.5 與 >= 6 等價
    for length in 0..10 {
        assert_eq!(length as f64 >= 5.5, length as f64 >= back.sticky["1"].end);
    }
}

#[test]
fn chat_length_skips_system_events_and_scene_summaries() {
    let mut summary = ev(TranscriptKind::Narration, "摘要");
    summary.marker = Some(EventMarker::SceneSummary);
    let events = vec![
        summary,
        ev(TranscriptKind::Player, "嗨"),
        ev(TranscriptKind::System, "登場"),
        ev(TranscriptKind::Narration, "旁白"),
        ev(TranscriptKind::Dialogue, "台詞"),
    ];
    assert_eq!(chat_length(&events), 3);
}

#[test]
fn perspectives_are_stored_separately() {
    let root = TestRoot::new("wi-store-perspectives");
    let world_id = world_with_book(&root, 3);
    let character = Perspective::Character("角色甲".to_owned());
    land_sent(
        &root,
        &world_id,
        0,
        "t1",
        &sticky(&[("1", 1.0, 4.0, false)]),
    );
    reply_landed(root.path(), &world_id, 0, "t1").unwrap();
    begin_landing(
        root.path(),
        &world_id,
        0,
        "t2",
        &character,
        &sticky(&[("2", 1.0, 3.0, false)]),
    )
    .unwrap();
    mark_sent(root.path(), &world_id, 0, "t2").unwrap();
    reply_landed(root.path(), &world_id, 0, "t2").unwrap();

    assert_eq!(
        table(&root, &world_id, 0, &GM)
            .sticky
            .keys()
            .collect::<Vec<_>>(),
        vec!["1"]
    );
    assert_eq!(
        table(&root, &world_id, 0, &character)
            .sticky
            .keys()
            .collect::<Vec<_>>(),
        vec!["2"]
    );
    let file = read_scene(root.path(), &world_id, 0).unwrap();
    assert_eq!(
        file.perspectives.keys().cloned().collect::<Vec<_>>(),
        vec!["char:角色甲".to_owned(), "gm".to_owned()]
    );
}

#[test]
fn landing_requires_settled_pending_and_matching_turn() {
    let root = TestRoot::new("wi-store-landing-guard");
    let world_id = world_with_book(&root, 2);
    begin_landing(root.path(), &world_id, 0, "t1", &GM, &sticky(&[])).unwrap();
    assert!(begin_landing(root.path(), &world_id, 0, "t2", &GM, &sticky(&[])).is_err());
    assert!(mark_sent(root.path(), &world_id, 0, "別的").is_err());
    // 寫入中收到正文（不會發生）不清：日誌留著給結算
    reply_landed(root.path(), &world_id, 0, "t1").unwrap();
    assert_eq!(
        pending_of(&root, &world_id, 0).unwrap().stage,
        Stage::Writing
    );
    mark_sent(root.path(), &world_id, 0, "t1").unwrap();
    assert!(mark_sent(root.path(), &world_id, 0, "t1").is_err());
    assert!(push_var_intent(
        root.path(),
        &world_id,
        0,
        "t1",
        intent(Layer::Chat, None, "{}")
    )
    .is_err());
}

#[test]
fn measurement_read_uses_pre_landing_table_and_writes_nothing() {
    let root = TestRoot::new("wi-store-measure");
    let world_id = world_with_book(&root, 3);
    land_sent(
        &root,
        &world_id,
        0,
        "t1",
        &sticky(&[("1", 1.0, 4.0, false)]),
    );
    reply_landed(root.path(), &world_id, 0, "t1").unwrap();
    land_sent(
        &root,
        &world_id,
        0,
        "t2",
        &sticky(&[("2", 2.0, 5.0, false)]),
    );
    let path = scene_file(&root, &world_id, 0);
    let bytes = std::fs::read(&path).unwrap();
    // 未結的 pending：以落地前的表計算
    assert_eq!(
        table(&root, &world_id, 0, &GM)
            .sticky
            .keys()
            .collect::<Vec<_>>(),
        vec!["1"]
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

fn begin_gm(root: &TestRoot, world_id: &str, turn: &str) {
    with_commit(root.path(), world_id, |tx| {
        message_vars::begin_turn(tx, turn, None)
    })
    .unwrap();
}

fn commit_gm(root: &TestRoot, world_id: &str, turn: &str) {
    with_commit(root.path(), world_id, |tx| {
        message_vars::apply_gm_block(
            tx,
            turn,
            PendingMain {
                text: format!("GM {turn}"),
                raw: None,
                truncated: false,
            },
            |_, _| ((), None),
            |_, _| Vec::new(),
        )
    })
    .unwrap()
    .unwrap();
}

fn finish(root: &TestRoot, world_id: &str, turn: &str, half: Option<&str>) {
    with_commit(root.path(), world_id, |tx| {
        message_vars::finish_turn(
            tx,
            turn,
            half.map(|text| PendingMain {
                text: text.to_owned(),
                raw: None,
                truncated: true,
            }),
        )
    })
    .unwrap();
}

fn append_main(root: &TestRoot, world_id: &str, turn: &str) {
    let key = TurnKey {
        turn_id: turn.to_owned(),
        part: PART_MAIN.to_owned(),
    };
    data::append_event(
        root.path(),
        world_id,
        0,
        &ev(TranscriptKind::Narration, &format!("GM {turn}")),
        Some(&key),
    )
    .unwrap();
}

#[test]
fn gm_main_append_clears_pending_and_drop_after_commit_keeps_it() {
    let root = TestRoot::new("wi-store-gm-success");
    let world_id = world_with_book(&root, 2);
    append(&root, &world_id, 0, TranscriptKind::Player, "嗨");
    begin_gm(&root, &world_id, "g1");
    land_sent(
        &root,
        &world_id,
        0,
        "g1",
        &sticky(&[("1", 1.0, 3.0, false)]),
    );
    commit_gm(&root, &world_id, "g1");
    // 成功 commit 後的 Drop 照樣呼叫 finish_turn(None)：等落檔中，不能還原
    finish(&root, &world_id, "g1", None);
    assert!(pending_of(&root, &world_id, 0).is_some());
    append_main(&root, &world_id, "g1");
    assert!(pending_of(&root, &world_id, 0).is_none());
    assert!(table(&root, &world_id, 0, &GM).sticky.contains_key("1"));
}

#[test]
fn gm_abort_without_text_restores_now_and_half_text_counts_as_success() {
    let root = TestRoot::new("wi-store-gm-abort");
    let world_id = world_with_book(&root, 3);
    append(&root, &world_id, 0, TranscriptKind::Player, "嗨");
    // 確定失敗：生成中→中止、沒有正文
    begin_gm(&root, &world_id, "g1");
    land_sent(
        &root,
        &world_id,
        0,
        "g1",
        &sticky(&[("1", 1.0, 3.0, false)]),
    );
    finish(&root, &world_id, "g1", None);
    assert!(pending_of(&root, &world_id, 0).is_none());
    assert!(table(&root, &world_id, 0, &GM).sticky.is_empty());

    // 中止留下半截：保留 pending，下一筆新事件前後端代落正文＝成功
    begin_gm(&root, &world_id, "g2");
    land_sent(
        &root,
        &world_id,
        0,
        "g2",
        &sticky(&[("2", 1.0, 3.0, false)]),
    );
    finish(&root, &world_id, "g2", Some("半截"));
    finish(&root, &world_id, "g2", None);
    assert!(pending_of(&root, &world_id, 0).is_some());
    append(&root, &world_id, 0, TranscriptKind::Player, "下一句");
    assert!(pending_of(&root, &world_id, 0).is_none());
    assert!(table(&root, &world_id, 0, &GM).sticky.contains_key("2"));
}

#[test]
fn character_reply_path_lands_gm_first_and_clears_pending() {
    let root = TestRoot::new("wi-store-character-path");
    let world_id = world_with_book(&root, 3);
    let character = Perspective::Character("甲".to_owned());
    append(&root, &world_id, 0, TranscriptKind::Player, "嗨");
    // 上一個 GM 回合提交了卻沒落正文
    begin_gm(&root, &world_id, "g1");
    commit_gm(&root, &world_id, "g1");
    begin_landing(
        root.path(),
        &world_id,
        0,
        "c1",
        &character,
        &sticky(&[("1", 2.0, 4.0, false)]),
    )
    .unwrap();
    mark_sent(root.path(), &world_id, 0, "c1").unwrap();

    let mut reply = ev(TranscriptKind::Dialogue, "角色回覆");
    reply.speaker_id = "甲".to_owned();
    let landed = data::append_character_reply(root.path(), &world_id, 0, &reply, "c1").unwrap();
    assert_eq!(
        landed.turn_key,
        Some(TurnKey {
            turn_id: "c1".to_owned(),
            part: message_vars::PART_CHARACTER.to_owned()
        })
    );
    let texts: Vec<String> = data::read_transcript(root.path(), &world_id, 0)
        .unwrap()
        .into_iter()
        .map(|event| event.text)
        .collect();
    assert_eq!(texts, vec!["嗨", "GM g1", "角色回覆"]);
    assert!(pending_of(&root, &world_id, 0).is_none());
    assert!(table(&root, &world_id, 0, &character)
        .sticky
        .contains_key("1"));
    // 同一回合的 GM 冪等路徑不受角色回合鍵影響
    append_main(&root, &world_id, "g1");
    assert_eq!(
        data::read_transcript(root.path(), &world_id, 0)
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn sent_pending_settles_by_turn_key_in_transcript() {
    let root = TestRoot::new("wi-store-settle-sent");
    let world_id = world_with_book(&root, 3);
    append(&root, &world_id, 0, TranscriptKind::Player, "嗨");
    land_sent(
        &root,
        &world_id,
        0,
        "t1",
        &sticky(&[("1", 1.0, 3.0, false)]),
    );
    // 正文落檔後、清 pending 前崩潰：事件帶著回合鍵，pending 還在
    let mut reply = ev(TranscriptKind::Dialogue, "回覆");
    reply.turn_key = Some(TurnKey {
        turn_id: "t1".to_owned(),
        part: message_vars::PART_CHARACTER.to_owned(),
    });
    data::append_event(root.path(), &world_id, 0, &reply, None).unwrap();
    assert!(pending_of(&root, &world_id, 0).is_some());
    settle_pending(root.path(), &world_id, 0).unwrap();
    assert!(pending_of(&root, &world_id, 0).is_none());
    assert!(table(&root, &world_id, 0, &GM).sticky.contains_key("1"));

    // 前端沒落檔：失敗，還原
    land_sent(
        &root,
        &world_id,
        0,
        "t2",
        &sticky(&[("1", 1.0, 3.0, false), ("2", 3.0, 5.0, false)]),
    );
    settle_pending(root.path(), &world_id, 0).unwrap();
    assert!(pending_of(&root, &world_id, 0).is_none());
    assert_eq!(
        table(&root, &world_id, 0, &GM)
            .sticky
            .keys()
            .collect::<Vec<_>>(),
        vec!["1"]
    );
    // pending 已還原之後才到的落檔：回合鍵對不到，不動計時
    data::append_character_reply(
        root.path(),
        &world_id,
        0,
        &ev(TranscriptKind::Dialogue, "晚到"),
        "t2",
    )
    .unwrap();
    assert_eq!(
        table(&root, &world_id, 0, &GM)
            .sticky
            .keys()
            .collect::<Vec<_>>(),
        vec!["1"]
    );
}

#[test]
fn failed_landing_is_undone_even_after_other_perspectives_appended() {
    let root = TestRoot::new("wi-store-cross-perspective");
    let world_id = world_with_book(&root, 3);
    append(&root, &world_id, 0, TranscriptKind::Player, "嗨");
    // GM 落地（start=1）後失敗；角色先追加了事件，則數已大於 start，靠「則數 ≤ start」撤不到
    land_sent(
        &root,
        &world_id,
        0,
        "g1",
        &sticky(&[("1", 1.0, 9.0, false)]),
    );
    append(&root, &world_id, 0, TranscriptKind::Dialogue, "角色先說");
    append(&root, &world_id, 0, TranscriptKind::Player, "再一句");
    settle_pending(root.path(), &world_id, 0).unwrap();
    assert!(table(&root, &world_id, 0, &GM).sticky.is_empty());
}

#[test]
fn fail_turn_only_touches_its_own_turn() {
    let root = TestRoot::new("wi-store-fail-turn");
    let world_id = world_with_book(&root, 2);
    land_sent(
        &root,
        &world_id,
        0,
        "t1",
        &sticky(&[("1", 0.0, 2.0, false)]),
    );
    fail_turn(root.path(), &world_id, 0, "別的", Report::Inline).unwrap();
    assert!(pending_of(&root, &world_id, 0).is_some());
    fail_turn(root.path(), &world_id, 0, "t1", Report::Inline).unwrap();
    assert!(pending_of(&root, &world_id, 0).is_none());
    assert!(table(&root, &world_id, 0, &GM).sticky.is_empty());
}

#[test]
fn retry_after_pop_drops_unprotected_effects_but_keeps_protected_cooldown() {
    use crate::world_info::entry::WiEntry;
    use crate::world_info::timed::{TimedEffects, TimedType};
    let root = TestRoot::new("wi-store-pop");
    let world_id = world_with_book(&root, 3);
    append(&root, &world_id, 0, TranscriptKind::Player, "一");
    append(&root, &world_id, 0, TranscriptKind::Narration, "二");
    let mut landed = sticky(&[("1", 2.0, 6.0, false)]);
    landed
        .cooldown
        .insert("2".to_owned(), effect(2.0, 6.0, true));
    land_sent(&root, &world_id, 0, "t1", &landed);
    append_main_like(&root, &world_id, "t1");
    assert!(data::pop_transcript(root.path(), &world_id, 0).unwrap());
    assert!(data::pop_transcript(root.path(), &world_id, 0).unwrap());
    let events = data::read_transcript(root.path(), &world_id, 0).unwrap();
    let entries: Vec<WiEntry> = ["1", "2"]
        .iter()
        .map(|id| {
            let raw = json!({"key": ["霧"], "content": "x", "sticky": 4, "cooldown": 4});
            WiEntry {
                id: (*id).to_owned(),
                ..crate::world_info::entry::from_world_file(raw.as_object().unwrap())
            }
        })
        .collect();
    let mut effects = TimedEffects::new(
        chat_length(&events),
        &entries,
        table(&root, &world_id, 0, &GM),
    );
    effects.check();
    assert!(!effects.is_active(TimedType::Sticky, "1"));
    assert!(effects.is_active(TimedType::Cooldown, "2"));
}

/// 角色回覆路落檔（帶回合鍵、清 pending）。
fn append_main_like(root: &TestRoot, world_id: &str, turn: &str) {
    data::append_character_reply(
        root.path(),
        world_id,
        0,
        &ev(TranscriptKind::Dialogue, "回覆"),
        turn,
    )
    .unwrap();
}

fn intent(layer: Layer, expected_rev: Option<String>, before: &str) -> VarIntent {
    VarIntent {
        layer,
        id: None,
        expected_rev,
        before: before.to_owned(),
        ops: json!([]),
        after_rev: None,
        restore_rev: None,
    }
}

fn write_layer(
    root: &TestRoot,
    world_id: &str,
    layer: Layer,
    rev: Option<&str>,
    vars: &str,
) -> String {
    let generation = with_commit(root.path(), world_id, message_vars::generation);
    match card_vars::write_layer(root.path(), world_id, layer, None, generation, rev, vars).unwrap()
    {
        LayerWrite::LayerOk { rev } => rev,
        other => panic!("{other:?}"),
    }
}

fn layer_vars(root: &TestRoot, world_id: &str, layer: Layer) -> (Option<String>, String) {
    let doc = card_vars::read_layer(root.path(), world_id, layer, None).unwrap();
    (doc.rev, doc.vars)
}

/// 寫入中崩潰：四種變數情形各一筆，分兩輪落在 chat、global 兩層。
#[test]
fn writing_stage_crash_recovers_var_intents_in_four_cases() {
    let root = TestRoot::new("wi-store-four-cases");
    let world_id = world_with_book(&root, 2);
    land_writing(&root, &world_id, "t1");

    // 1：有結果、目前 rev 等於寫入後 rev → 還原成寫入前內容（寫入前沒有檔＝刪檔）
    let index = push_var_intent(
        root.path(),
        &world_id,
        0,
        "t1",
        intent(Layer::Chat, None, "{}"),
    )
    .unwrap();
    let after = write_layer(&root, &world_id, Layer::Chat, None, r#"{"好感":1}"#);
    update_var_intent(root.path(), &world_id, 0, "t1", index, |stored| {
        stored.after_rev = Some(after)
    })
    .unwrap();
    // 2：有結果、之後被別人寫過 → 保留、回報
    let base = write_layer(&root, &world_id, Layer::Global, None, r#"{"g":0}"#);
    let index = push_var_intent(
        root.path(),
        &world_id,
        0,
        "t1",
        intent(Layer::Global, Some(base.clone()), r#"{"g":0}"#),
    )
    .unwrap();
    let mine = write_layer(&root, &world_id, Layer::Global, Some(&base), r#"{"g":1}"#);
    update_var_intent(root.path(), &world_id, 0, "t1", index, |stored| {
        stored.after_rev = Some(mine.clone())
    })
    .unwrap();
    write_layer(&root, &world_id, Layer::Global, Some(&mine), r#"{"g":2}"#);
    settle_pending(root.path(), &world_id, 0).unwrap();
    assert_eq!(
        layer_vars(&root, &world_id, Layer::Chat),
        (None, "{}".to_owned())
    );
    assert_eq!(layer_vars(&root, &world_id, Layer::Global).1, r#"{"g":2}"#);
    let notices = read_notices(root.path(), &world_id).unwrap();
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].id, "t1:1");
    assert_eq!(notices[0].conflict.reason, ConflictReason::Overwritten);
    assert!(pending_of(&root, &world_id, 0).is_none());

    // 3：沒有結果、目前 rev 等於預期 → 沒寫成，不動；4：沒有結果、rev 不同 → 分不出，保留、回報
    land_writing(&root, &world_id, "t2");
    let chat = write_layer(&root, &world_id, Layer::Chat, None, r#"{"c":0}"#);
    push_var_intent(
        root.path(),
        &world_id,
        0,
        "t2",
        intent(Layer::Chat, Some(chat.clone()), r#"{"c":0}"#),
    )
    .unwrap();
    let global = layer_vars(&root, &world_id, Layer::Global).0;
    push_var_intent(
        root.path(),
        &world_id,
        0,
        "t2",
        intent(Layer::Global, global.clone(), r#"{"g":2}"#),
    )
    .unwrap();
    write_layer(
        &root,
        &world_id,
        Layer::Global,
        global.as_deref(),
        r#"{"g":3}"#,
    );
    settle_pending(root.path(), &world_id, 0).unwrap();
    assert_eq!(
        layer_vars(&root, &world_id, Layer::Chat),
        (Some(chat), r#"{"c":0}"#.to_owned())
    );
    assert_eq!(layer_vars(&root, &world_id, Layer::Global).1, r#"{"g":3}"#);
    let notices = read_notices(root.path(), &world_id).unwrap();
    assert_eq!(notices.len(), 2);
    assert_eq!(notices[1].id, "t2:1");
    assert_eq!(notices[1].conflict.reason, ConflictReason::Unknown);
    // 計時也還原了；下一輪不會重複撤回
    assert!(table(&root, &world_id, 0, &GM).sticky.is_empty());
    settle_pending(root.path(), &world_id, 0).unwrap();
    assert_eq!(read_notices(root.path(), &world_id).unwrap().len(), 2);

    // 玩家看過：逐則確認，最後一則刪完連檔刪掉
    ack_notice(root.path(), &world_id, "t1:1").unwrap();
    ack_notice(root.path(), &world_id, "t2:1").unwrap();
    assert!(read_notices(root.path(), &world_id).unwrap().is_empty());
    assert!(!root
        .path()
        .join(format!("worlds/{world_id}/world-info/notices.json"))
        .exists());
}

fn land_writing(root: &TestRoot, world_id: &str, turn: &str) {
    begin_landing(
        root.path(),
        world_id,
        0,
        turn,
        &GM,
        &sticky(&[("1", 0.0, 3.0, false)]),
    )
    .unwrap();
}

#[test]
fn rerun_after_crash_mid_restore_does_not_report_own_restore() {
    let root = TestRoot::new("wi-store-rerun");
    let world_id = world_with_book(&root, 2);
    let base = write_layer(&root, &world_id, Layer::Chat, None, r#"{"a":0}"#);
    land_writing(&root, &world_id, "t1");
    let index = push_var_intent(
        root.path(),
        &world_id,
        0,
        "t1",
        intent(Layer::Chat, Some(base.clone()), r#"{"a":0}"#),
    )
    .unwrap();
    let after = write_layer(&root, &world_id, Layer::Chat, Some(&base), r#"{"a":1}"#);
    update_var_intent(root.path(), &world_id, 0, "t1", index, |stored| {
        stored.after_rev = Some(after.clone());
        stored.restore_rev = Some("撤回用".to_owned());
    })
    .unwrap();
    // 上一次結算已撤回成功（層是撤回用的 rev），清 pending 前崩潰
    assert_eq!(
        card_vars::restore_if_rev(
            root.path(),
            &world_id,
            Layer::Chat,
            None,
            &after,
            Some(r#"{"a":0}"#),
            "撤回用"
        )
        .unwrap(),
        card_vars::CasRestore::Restored
    );
    settle_pending(root.path(), &world_id, 0).unwrap();
    assert!(read_notices(root.path(), &world_id).unwrap().is_empty());
    assert_eq!(
        layer_vars(&root, &world_id, Layer::Chat),
        (Some("撤回用".to_owned()), r#"{"a":0}"#.to_owned())
    );
}

#[test]
fn cas_retry_restores_to_latest_before_image_keeping_interface_write() {
    let root = TestRoot::new("wi-store-cas-retry");
    let world_id = world_with_book(&root, 2);
    let first = write_layer(&root, &world_id, Layer::Chat, None, r#"{"a":0}"#);
    land_writing(&root, &world_id, "t1");
    let index = push_var_intent(
        root.path(),
        &world_id,
        0,
        "t1",
        intent(Layer::Chat, Some(first.clone()), r#"{"a":0}"#),
    )
    .unwrap();
    // 第一次嘗試前，介面寫了同一層 → Stale；重試前先把預期 rev 與前像更新成這次讀到的值並落檔
    let interface = write_layer(
        &root,
        &world_id,
        Layer::Chat,
        Some(&first),
        r#"{"a":0,"介面":1}"#,
    );
    update_var_intent(root.path(), &world_id, 0, "t1", index, |stored| {
        stored.expected_rev = Some(interface.clone());
        stored.before = r#"{"a":0,"介面":1}"#.to_owned();
    })
    .unwrap();
    let after = write_layer(
        &root,
        &world_id,
        Layer::Chat,
        Some(&interface),
        r#"{"a":5,"介面":1}"#,
    );
    update_var_intent(root.path(), &world_id, 0, "t1", index, |stored| {
        stored.after_rev = Some(after)
    })
    .unwrap();
    // 之後落地失敗：撤回保住介面寫進去的值
    fail_turn(root.path(), &world_id, 0, "t1", Report::Inline).unwrap();
    assert_eq!(
        layer_vars(&root, &world_id, Layer::Chat).1,
        r#"{"a":0,"介面":1}"#
    );
}

#[test]
fn inline_report_returns_conflicts_without_notices_file() {
    let root = TestRoot::new("wi-store-inline");
    let world_id = world_with_book(&root, 2);
    let base = write_layer(&root, &world_id, Layer::Chat, None, r#"{"a":0}"#);
    land_writing(&root, &world_id, "t1");
    push_var_intent(
        root.path(),
        &world_id,
        0,
        "t1",
        intent(Layer::Chat, Some(base.clone()), r#"{"a":0}"#),
    )
    .unwrap();
    write_layer(&root, &world_id, Layer::Chat, Some(&base), r#"{"a":9}"#);
    let conflicts = fail_turn(root.path(), &world_id, 0, "t1", Report::Inline).unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].reason, ConflictReason::Unknown);
    assert!(read_notices(root.path(), &world_id).unwrap().is_empty());
    assert!(pending_of(&root, &world_id, 0).is_none());
}

#[test]
fn notice_write_failure_keeps_pending() {
    let root = TestRoot::new("wi-store-notice-fail");
    let world_id = world_with_book(&root, 2);
    let base = write_layer(&root, &world_id, Layer::Chat, None, r#"{"a":0}"#);
    land_writing(&root, &world_id, "t1");
    push_var_intent(
        root.path(),
        &world_id,
        0,
        "t1",
        intent(Layer::Chat, Some(base.clone()), r#"{"a":0}"#),
    )
    .unwrap();
    write_layer(&root, &world_id, Layer::Chat, Some(&base), r#"{"a":9}"#);
    {
        let _guard = WriteFailGuard::partial_ending("notices.json.tmp", 1);
        assert!(settle_pending(root.path(), &world_id, 0).is_err());
    }
    assert!(pending_of(&root, &world_id, 0).is_some());
    settle_pending(root.path(), &world_id, 0).unwrap();
    assert_eq!(read_notices(root.path(), &world_id).unwrap().len(), 1);
    assert!(pending_of(&root, &world_id, 0).is_none());
}

#[test]
fn interrupted_atomic_write_leaves_previous_file_intact() {
    let root = TestRoot::new("wi-store-atomic");
    let world_id = world_with_book(&root, 2);
    land_writing(&root, &world_id, "t1");
    let path = scene_file(&root, &world_id, 0);
    let before = std::fs::read(&path).unwrap();
    {
        let _guard = WriteFailGuard::partial_ending("0.json.tmp", 1);
        assert!(mark_sent(root.path(), &world_id, 0, "t1").is_err());
    }
    assert_eq!(std::fs::read(&path).unwrap(), before);
    {
        let _guard = RenameFailGuard::fail_ending("0.json", 1);
        assert!(mark_sent(root.path(), &world_id, 0, "t1").is_err());
    }
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!path.with_file_name("0.json.tmp").exists());
    assert_eq!(
        pending_of(&root, &world_id, 0).unwrap().stage,
        Stage::Writing
    );
}

#[test]
fn next_scene_settles_then_shifts_from_after_summary() {
    let root = TestRoot::new("wi-store-next-scene");
    let world_id = world_with_book(&root, 3);
    append(&root, &world_id, 0, TranscriptKind::Player, "一");
    append(&root, &world_id, 0, TranscriptKind::System, "系統不計");
    let character = Perspective::Character("甲".to_owned());
    begin_landing(
        root.path(),
        &world_id,
        0,
        "c1",
        &character,
        &sticky(&[("2", 1.0, 3.0, false)]),
    )
    .unwrap();
    mark_sent(root.path(), &world_id, 0, "c1").unwrap();
    reply_landed(root.path(), &world_id, 0, "c1").unwrap();
    // GM 回合送出後中止留下半截：換幕的交接先代落，結算看得到回合鍵＝成功
    begin_gm(&root, &world_id, "g1");
    land_sent(
        &root,
        &world_id,
        0,
        "g1",
        &sticky(&[("1", 1.0, 5.0, false)]),
    );
    finish(&root, &world_id, "g1", Some("半截"));
    // 殘留的新幕檔要被覆寫
    std::fs::write(scene_file(&root, &world_id, 1), b"garbage").unwrap();

    assert_eq!(
        data::begin_next_scene(root.path(), &world_id, "摘要", None).unwrap(),
        1
    );
    // 舊幕：一、半截 GM＝2 則（系統不計）
    let gm = table(&root, &world_id, 1, &GM);
    assert_eq!((gm.sticky["1"].start, gm.sticky["1"].end), (-1.0, 3.0));
    let mine = table(&root, &world_id, 1, &character);
    assert_eq!((mine.sticky["2"].start, mine.sticky["2"].end), (-1.0, 1.0));
    assert!(pending_of(&root, &world_id, 0).is_none());
    assert!(pending_of(&root, &world_id, 1).is_none());
    // 摘要不計：新幕則數 0
    assert_eq!(
        chat_length(&data::read_transcript(root.path(), &world_id, 1).unwrap()),
        0
    );
}

#[test]
fn next_scene_restores_failed_pending_and_writes_empty_file_without_timers() {
    let root = TestRoot::new("wi-store-next-scene-fail");
    let world_id = world_with_book(&root, 2);
    append(&root, &world_id, 0, TranscriptKind::Player, "一");
    land_sent(
        &root,
        &world_id,
        0,
        "t1",
        &sticky(&[("1", 1.0, 5.0, false)]),
    );
    data::begin_next_scene(root.path(), &world_id, "摘要", None).unwrap();
    assert!(table(&root, &world_id, 0, &GM).sticky.is_empty());
    assert!(table(&root, &world_id, 1, &GM).sticky.is_empty());
    assert!(scene_file(&root, &world_id, 1).exists());
}

#[test]
fn next_scene_fails_and_keeps_pending_when_notices_cannot_be_written() {
    let root = TestRoot::new("wi-store-next-scene-notice");
    let world_id = world_with_book(&root, 2);
    append(&root, &world_id, 0, TranscriptKind::Player, "一");
    let base = write_layer(&root, &world_id, Layer::Chat, None, r#"{"a":0}"#);
    land_writing(&root, &world_id, "t1");
    push_var_intent(
        root.path(),
        &world_id,
        0,
        "t1",
        intent(Layer::Chat, Some(base.clone()), r#"{"a":0}"#),
    )
    .unwrap();
    write_layer(&root, &world_id, Layer::Chat, Some(&base), r#"{"a":9}"#);
    {
        let _guard = WriteFailGuard::partial_ending("notices.json.tmp", 1);
        assert!(data::begin_next_scene(root.path(), &world_id, "摘要", None).is_err());
    }
    assert_eq!(
        data::read_state(root.path(), &world_id)
            .unwrap()
            .current_scene,
        0
    );
    assert!(pending_of(&root, &world_id, 0).is_some());
    data::begin_next_scene(root.path(), &world_id, "摘要", None).unwrap();
    assert_eq!(read_notices(root.path(), &world_id).unwrap().len(), 1);
}

#[test]
fn fork_copies_settled_file_and_revert_drops_child_file() {
    let root = TestRoot::new("wi-store-fork");
    let world_id = world_with_book(&root, 3);
    append(&root, &world_id, 0, TranscriptKind::Player, "一");
    land_sent(
        &root,
        &world_id,
        0,
        "t1",
        &sticky(&[("1", 1.0, 5.0, false)]),
    );
    reply_landed(root.path(), &world_id, 0, "t1").unwrap();
    data::begin_next_scene(root.path(), &world_id, "摘要", None).unwrap();
    // 目前幕（幕 1，分岔後被離開）與來源幕各留一筆已送出、逐字稿裡沒有回合鍵的 pending（前端沒落檔）
    let stale = |turn: &str, before: StoredTimed| Pending {
        turn_key: turn.to_owned(),
        perspective: "gm".to_owned(),
        stage: Stage::Sent,
        before,
        vars: Vec::new(),
    };
    let mut current = read_scene(root.path(), &world_id, 1).unwrap();
    current.pending = Some(stale("目前幕回合", StoredTimed::default()));
    write_scene(root.path(), &world_id, 1, &current).unwrap();
    let mut source = read_scene(root.path(), &world_id, 0).unwrap();
    source.pending = Some(stale("舊回合", StoredTimed::default()));
    write_scene(root.path(), &world_id, 0, &source).unwrap();
    std::fs::write(scene_file(&root, &world_id, 2), b"garbage").unwrap();

    let forked = data::fork_scene(root.path(), &world_id, 0).unwrap();
    assert_eq!(forked, 2);
    // 兩幕都結算成失敗、還原成前像（空）；新幕照來源幕結算後複製、不帶 pending
    assert!(pending_of(&root, &world_id, 1).is_none());
    assert!(table(&root, &world_id, 1, &GM).sticky.is_empty());
    assert!(pending_of(&root, &world_id, 0).is_none());
    let copied = read_scene(root.path(), &world_id, 2).unwrap();
    assert!(copied.pending.is_none());
    assert_eq!(
        copied.perspectives,
        read_scene(root.path(), &world_id, 0).unwrap().perspectives
    );

    assert_eq!(data::revert_scene(root.path(), &world_id).unwrap(), 1);
    // 分岔幕只有複製來的一則，退得回去；子幕計時檔一起刪、父幕的不動
    assert!(!scene_file(&root, &world_id, 2).exists());
    assert!(scene_file(&root, &world_id, 1).exists());
}

#[test]
fn revert_settles_child_pending_first_and_refuses_when_it_cannot() {
    let root = TestRoot::new("wi-store-revert");
    let world_id = world_with_book(&root, 2);
    append(&root, &world_id, 0, TranscriptKind::Player, "一");
    data::begin_next_scene(root.path(), &world_id, "摘要", None).unwrap();
    // 子幕寫入中崩潰：chat 層已寫成，退幕前要撤回（chat 層不分幕，檔刪了就撤不回）
    let base = write_layer(&root, &world_id, Layer::Chat, None, r#"{"a":0}"#);
    begin_landing(root.path(), &world_id, 1, "t1", &GM, &sticky(&[])).unwrap();
    let index = push_var_intent(
        root.path(),
        &world_id,
        1,
        "t1",
        intent(Layer::Chat, Some(base.clone()), r#"{"a":0}"#),
    )
    .unwrap();
    let after = write_layer(&root, &world_id, Layer::Chat, Some(&base), r#"{"a":1}"#);
    update_var_intent(root.path(), &world_id, 1, "t1", index, |stored| {
        stored.after_rev = Some(after)
    })
    .unwrap();
    // 子幕計時檔壞掉：結算回錯，整個不退
    let path = scene_file(&root, &world_id, 1);
    let good = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"{broken").unwrap();
    assert!(data::revert_scene(root.path(), &world_id).is_err());
    assert_eq!(
        data::read_state(root.path(), &world_id)
            .unwrap()
            .current_scene,
        1
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"{broken");

    std::fs::write(&path, good).unwrap();
    assert_eq!(data::revert_scene(root.path(), &world_id).unwrap(), 0);
    assert_eq!(layer_vars(&root, &world_id, Layer::Chat).1, r#"{"a":0}"#);
    assert!(!path.exists());
}

fn upsert(root: &TestRoot, world_id: &str, title: &str) -> u64 {
    data::upsert_worldbook_entry(root.path(), world_id, worldbook_entry(u64::MAX, title)).unwrap()
}

/// 每一幕、每個視角與 pending 前像都放同一組 uid 的計時。
fn seed_timers(root: &TestRoot, world_id: &str, uids: &[u64]) {
    let table = StoredTimed {
        sticky: uids
            .iter()
            .map(|uid| {
                (
                    uid.to_string(),
                    StoredEffect {
                        start: 0,
                        end: 9,
                        protected: false,
                        confidential: false,
                    },
                )
            })
            .collect(),
        cooldown: BTreeMap::new(),
    };
    for scene in [0, 1] {
        write_scene(
            root.path(),
            world_id,
            scene,
            &SceneTimed {
                perspectives: BTreeMap::from([
                    ("gm".to_owned(), table.clone()),
                    ("char:甲".to_owned(), table.clone()),
                ]),
                pending: Some(Pending {
                    turn_key: "t".to_owned(),
                    perspective: "gm".to_owned(),
                    stage: Stage::Sent,
                    before: table.clone(),
                    vars: Vec::new(),
                }),
                ..SceneTimed::default()
            },
        )
        .unwrap();
    }
}

fn stored_uids(root: &TestRoot, world_id: &str) -> Vec<Vec<String>> {
    [0, 1]
        .iter()
        .flat_map(|scene| {
            let file = read_scene(root.path(), world_id, *scene).unwrap();
            let mut tables: Vec<StoredTimed> = file.perspectives.into_values().collect();
            tables.push(file.pending.unwrap().before);
            tables
        })
        .map(|table| table.sticky.into_keys().collect())
        .collect()
}

#[test]
fn deleting_entries_clears_their_timers_everywhere() {
    let root = TestRoot::new("wi-store-prune");
    let world_id = data::create_world(root.path(), "刪條目桌").unwrap();
    let a = upsert(&root, &world_id, "甲");
    let b = upsert(&root, &world_id, "乙");
    seed_timers(&root, &world_id, &[a, b]);
    data::delete_worldbook_entry(root.path(), &world_id, a).unwrap();
    assert!(stored_uids(&root, &world_id)
        .iter()
        .all(|uids| uids == &vec![b.to_string()]));

    // 清重複：兩條同內容，留一條
    let dup = data::upsert_worldbook_entry(root.path(), &world_id, worldbook_entry(u64::MAX, "乙"))
        .unwrap();
    seed_timers(&root, &world_id, &[b, dup]);
    assert_eq!(data::dedupe_worldbook(root.path(), &world_id).unwrap(), 1);
    let left = data::read_worldbook(root.path(), &world_id).unwrap()[0]
        .uid
        .to_string();
    assert!(stored_uids(&root, &world_id)
        .iter()
        .all(|uids| uids == &vec![left.clone()]));
}

#[test]
fn undoing_an_import_clears_timers_of_removed_entries() {
    let root = TestRoot::new("wi-store-prune-undo");
    let world_id = data::create_world(root.path(), "撤銷匯入桌").unwrap();
    let held = data::test_exclusive(&world_id);
    let book = json!({"entries": {"0": {"uid": 0, "key": ["霧"], "content": "匯入的條目"}}});
    crate::import::import_worldbook_file(
        root.path(),
        &world_id,
        book.to_string().as_bytes(),
        "書",
        &held,
    )
    .unwrap();
    let uid = data::read_worldbook(root.path(), &world_id).unwrap()[0].uid;
    seed_timers(&root, &world_id, &[uid]);
    crate::receipts::undo_last_import(root.path(), &world_id, &held).unwrap();
    assert!(stored_uids(&root, &world_id).iter().all(Vec::is_empty));
}

#[test]
fn reused_uid_after_crash_does_not_inherit_old_timers() {
    let root = TestRoot::new("wi-store-uid-reuse");
    let world_id = data::create_world(root.path(), "撞號桌").unwrap();
    let a = upsert(&root, &world_id, "甲");
    let top = upsert(&root, &world_id, "乙");
    seed_timers(&root, &world_id, &[a, top]);
    // 刪掉最大 uid：書寫進去了、清計時前崩潰（直接改書檔模擬）
    let kept = data::read_worldbook_raw(root.path(), &world_id).unwrap()[&a].clone();
    write_worldbook_fixture(&root, &world_id, json!({ a.to_string(): kept }));
    // 讀的時候在記憶體裡照現有 uid 清掉，不寫回
    let path = scene_file(&root, &world_id, 0);
    let bytes = std::fs::read(&path).unwrap();
    assert!(!table(&root, &world_id, 0, &GM)
        .sticky
        .contains_key(&top.to_string()));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    // 重啟後先新增一條拿到同一個 uid：發布前就先清掉舊計時
    let reused = upsert(&root, &world_id, "丙");
    assert_eq!(reused, top);
    assert!(stored_uids(&root, &world_id)
        .iter()
        .all(|uids| uids == &vec![a.to_string()]));
}

#[test]
fn web_import_merges_two_stable_ids_on_one_uid() {
    let root = TestRoot::new("wi-store-web-merge");
    let world_id = world_with_book(&root, 3);
    let character = Perspective::Character("甲".to_owned());
    let item = |cooldown, uid, end, protected| WebTimed {
        cooldown,
        uid,
        start: 1,
        end,
        protected,
    };
    import_web_timed(
        root.path(),
        &world_id,
        0,
        &character,
        &[
            item(false, 1, 4, false),
            item(false, 1, 6, false),
            item(true, 2, 5, false),
            item(true, 2, 5, true),
            item(true, 2, 3, true),
        ],
    )
    .unwrap();
    let timed = table(&root, &world_id, 0, &character);
    assert_eq!(timed.sticky["1"].end, 6.0);
    assert_eq!(timed.cooldown["2"].end, 5.0);
    assert!(timed.cooldown["2"].protected);
    assert!(table(&root, &world_id, 0, &GM).sticky.is_empty());
}

#[test]
fn character_reply_retry_returns_the_landed_event() {
    let root = TestRoot::new("wi-store-character-retry");
    let world_id = world_with_book(&root, 2);
    append(&root, &world_id, 0, TranscriptKind::Player, "嗨");
    let reply = ev(TranscriptKind::Dialogue, "角色回覆");
    let first = data::append_character_reply(root.path(), &world_id, 0, &reply, "c1").unwrap();
    // 寫成了但回傳失敗，前端重送同一則
    let again = data::append_character_reply(root.path(), &world_id, 0, &reply, "c1").unwrap();
    assert_eq!(again.id, first.id);
    assert_eq!(
        data::read_transcript(root.path(), &world_id, 0)
            .unwrap()
            .len(),
        2
    );
    // 不同回合照常追加
    data::append_character_reply(root.path(), &world_id, 0, &reply, "c2").unwrap();
    assert_eq!(
        data::read_transcript(root.path(), &world_id, 0)
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn only_reply_events_count_as_success_not_state_update() {
    let root = TestRoot::new("wi-store-state-update-only");
    let world_id = world_with_book(&root, 2);
    append(&root, &world_id, 0, TranscriptKind::Player, "嗨");
    land_sent(
        &root,
        &world_id,
        0,
        "g1",
        &sticky(&[("1", 1.0, 3.0, false)]),
    );
    // 只有附屬的變動紀錄帶著回合鍵落檔，正文沒有
    let mut side = ev(TranscriptKind::System, "變動紀錄");
    side.turn_key = Some(TurnKey {
        turn_id: "g1".to_owned(),
        part: "state_update".to_owned(),
    });
    data::append_event(root.path(), &world_id, 0, &side, None).unwrap();
    settle_pending(root.path(), &world_id, 0).unwrap();
    assert!(pending_of(&root, &world_id, 0).is_none());
    assert!(table(&root, &world_id, 0, &GM).sticky.is_empty());
}

/// 結算讀出落地前表、寫回之前（撤回變數層時），另一條執行緒刪掉其中的條目：刪除要等結算放開短提交鎖
/// 才進得來，刪完再清計時——舊 uid 不會被結算寫回、之後新條目重用這個 uid 也不會繼承。
#[test]
fn deleting_an_entry_waits_for_settlement_and_its_timer_stays_cleared() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::Duration;
    let _serial = data::write_hook::TESTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let root = TestRoot::new("wi-store-race");
    let world_id = data::create_world(root.path(), "競態桌").unwrap();
    let keep = upsert(&root, &world_id, "甲");
    let doomed = upsert(&root, &world_id, "乙");
    append(&root, &world_id, 0, TranscriptKind::Player, "嗨");
    // 落地前表含兩條；落地後失敗（前端沒落檔）→ 結算要把前表寫回
    let before = sticky(&[
        (&keep.to_string(), 0.0, 9.0, false),
        (&doomed.to_string(), 0.0, 9.0, false),
    ]);
    land_sent(&root, &world_id, 0, "t0", &before);
    reply_landed(root.path(), &world_id, 0, "t0").unwrap();
    // 寫入中崩潰、chat 層已寫成：結算讀出前表之後、寫回計時之前，會先撤回 chat 層（掛點打在那裡）
    let base = write_layer(&root, &world_id, Layer::Chat, None, r#"{"a":0}"#);
    begin_landing(root.path(), &world_id, 0, "t1", &GM, &sticky(&[])).unwrap();
    let index = push_var_intent(
        root.path(),
        &world_id,
        0,
        "t1",
        intent(Layer::Chat, Some(base.clone()), r#"{"a":0}"#),
    )
    .unwrap();
    let after = write_layer(&root, &world_id, Layer::Chat, Some(&base), r#"{"a":1}"#);
    update_var_intent(root.path(), &world_id, 0, "t1", index, |stored| {
        stored.after_rev = Some(after)
    })
    .unwrap();
    let chat_path = root
        .path()
        .join(format!("worlds/{world_id}/card-vars/chat.json"));

    let fired = Arc::new(AtomicBool::new(false));
    let worker: Arc<Mutex<Option<std::thread::JoinHandle<()>>>> = Arc::default();
    let (done_tx, done_rx) = mpsc::channel();
    let done_rx = Arc::new(Mutex::new(done_rx));
    let finished_early = Arc::new(AtomicBool::new(false));
    let _hook = {
        let (fired, worker, finished_early) =
            (fired.clone(), worker.clone(), finished_early.clone());
        let (path, world) = (root.path().to_path_buf(), world_id.clone());
        data::write_hook::install(chat_path, move || {
            if fired.swap(true, Ordering::SeqCst) {
                return;
            }
            let (path, world, done_tx) = (path.clone(), world.clone(), done_tx.clone());
            *worker.lock().unwrap() = Some(std::thread::spawn(move || {
                data::delete_worldbook_entry(&path, &world, doomed).unwrap();
                let _ = done_tx.send(());
            }));
            // 結算還拿著短提交鎖：刪除應該進不來
            if done_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_millis(300))
                .is_ok()
            {
                finished_early.store(true, Ordering::SeqCst);
            }
        })
    };
    settle_pending(root.path(), &world_id, 0).unwrap();
    let handle = worker.lock().unwrap().take().unwrap();
    handle.join().unwrap();
    assert!(!finished_early.load(Ordering::SeqCst), "刪除插進了結算中間");
    let stored = read_scene(root.path(), &world_id, 0).unwrap();
    assert_eq!(
        stored.perspectives["gm"]
            .sticky
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec![keep.to_string()]
    );
}
