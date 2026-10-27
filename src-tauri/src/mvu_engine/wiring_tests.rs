//! 桌面接線（規格 3 節 P4 驗收）：保序種子、GM 回合三段提交、過期結果不提交、整次失敗正文照落、開場白同一條路、
//! 收回讀已存快照不重跑 transform、後處理退回連帶撤回觸發與記錄。用乾淨 schema 夾具（src/features/mvu-engine/fixtures）。
use super::wiring::{self, commit_turn, failure_message, gm_commit, prepare_turn, zod_table};
use super::{EngineFailure, FailureKind};
use crate::data::message_vars::{self, Json, Macros, PendingMain, TurnKey};
use crate::data::state_commit::with_commit;
use crate::data::{self, StateNode, TranscriptEvent, TranscriptKind};
use crate::mechanism::{Outcome, RecordKind};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const SCHEMA: &str = include_str!("../../../src/features/mvu-engine/fixtures/clean-zod-schema.txt");
const INITVAR: &str =
    include_str!("../../../src/features/mvu-engine/fixtures/clean-zod-initvar.yaml");
const USER: &str = "玩家";

/// 引擎是全 app 一份、全機名額 2：這組測試互相排隊，免得並行時互搶名額
static SERIAL: Mutex<()> = Mutex::new(());

struct TestRoot(PathBuf);

impl TestRoot {
    fn new(label: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("table-tavern-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 狀態樹的葉（點分路徑）
fn leaf(tree: &std::collections::BTreeMap<String, StateNode>, path: &str) -> Option<String> {
    let segments: Vec<String> = path.split('.').map(str::to_owned).collect();
    match data::node_at(tree, &segments) {
        Some(StateNode::Leaf(value)) => Some(value.clone()),
        _ => None,
    }
}

/// 匯入一張 zod 卡（`initvar` None＝卡上沒有 `[initvar]`），回角色 id
fn import_card(
    root: &Path,
    world_id: &str,
    name: &str,
    schema: &str,
    initvar: Option<&str>,
) -> String {
    let entries: Vec<serde_json::Value> = initvar
        .map(|content| json!({ "comment": "[initvar]初始", "enabled": false, "content": content }))
        .into_iter()
        .collect();
    let card = json!({"data": {
        "name": name,
        "first_mes": "開場",
        "extensions": {"tavern_helper": {"scripts": [
            { "name": "MVU", "enabled": true, "type": "script", "content": "import 'https://x/MagVarUpdate/bundle.js';" },
            { "name": "变量结构", "enabled": true, "type": "script", "content": schema },
        ]}},
        "character_book": {"entries": entries}
    }});
    crate::import::import_character(
        root,
        world_id,
        card.to_string().as_bytes(),
        "#336699",
        "zh-TW",
    )
    .unwrap()
    .id
}

fn zod_world(root: &TestRoot, schema: &str) -> String {
    let world_id = data::create_world(root.path(), "zod桌").unwrap();
    import_card(root.path(), &world_id, "冒險隊", schema, Some(INITVAR));
    world_id
}

/// 小 schema：金 夾到 100、名 有預設
const SMALL: &str = "import { registerMvuSchema } from 'https://testingcf.jsdelivr.net/gh/StageDog/tavern_resource/dist/util/mvu_zod.js';\nregisterMvuSchema(z.object({ 金: z.coerce.number().transform(v => Math.min(v, 100)).prefault(5), 名: z.string().prefault('無名') }));";

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
        action_id: None,
    }
}

fn macros() -> Macros {
    Macros {
        user: USER.to_owned(),
        char: None,
    }
}

fn opening(root: &Path, world_id: &str, raw: &str) -> (TranscriptEvent, Outcome) {
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
}

fn begin(root: &Path, world_id: &str, turn_id: &str) {
    with_commit(root, world_id, |tx| {
        message_vars::begin_turn(tx, turn_id, Some(&macros()))
    })
    .unwrap();
}

fn main_text(turn_id: &str) -> PendingMain {
    PendingMain {
        text: format!("GM {turn_id}"),
        raw: None,
        truncated: false,
    }
}

/// 跑一輪 GM：開始 → zod 提交 → 前端落正文。回提交結果與落檔的那則
fn gm_turn(
    root: &Path,
    world_id: &str,
    turn_id: &str,
    reply: &str,
) -> (
    Option<message_vars::GmCommit<Option<Outcome>>>,
    TranscriptEvent,
) {
    begin(root, world_id, turn_id);
    let zod = zod_table(root, world_id).expect("zod 桌");
    let block = crate::transport::extract_state_block(reply);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let committed = runtime
        .block_on(gm_commit(
            root,
            world_id,
            turn_id,
            zod,
            main_text(turn_id),
            reply,
            &block,
            USER,
            macros(),
            |_, _| Vec::new(),
        ))
        .unwrap();
    let key = TurnKey {
        turn_id: turn_id.to_owned(),
        part: message_vars::PART_MAIN.to_owned(),
    };
    let landed = data::append_event(
        root,
        world_id,
        0,
        &event(TranscriptKind::Narration, &format!("GM {turn_id}")),
        Some(&key),
    )
    .unwrap()
    .0;
    (committed, landed)
}

fn stat(event: &TranscriptEvent) -> Json {
    event
        .message_vars
        .as_ref()
        .expect("帶表")
        .parse()
        .unwrap()
        .get("stat_data")
        .unwrap()
        .clone()
}

fn keys(json: &Json) -> Vec<String> {
    json.as_object()
        .unwrap()
        .iter()
        .map(|(key, _)| key.clone())
        .collect()
}

#[test]
fn opening_seeds_in_initvar_order_and_runs_its_commands_through_the_engine() {
    let _serial = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
    let root = TestRoot::new("zod-opening");
    let world_id = zod_world(&root, SCHEMA);
    let (landed, outcome) = opening(
        root.path(),
        &world_id,
        "旅程開始。_.set('世界.天氣', '雨');_.set('世界.天氣', 7);",
    );
    let stat = stat(&landed);
    // 種子照 initvar 原文的鍵順序（樹會依鍵名排成 世界、標籤、檔案、稱號、隊員）
    assert_eq!(keys(&stat), vec!["世界", "隊員", "檔案", "標籤", "稱號"]);
    // 開局過 schema（夾值、自訂 transform），再套開場白指令：第一條寫入、第二條被拒收
    assert_eq!(
        stat.get("世界").unwrap().get("天氣"),
        Some(&Json::String("雨".into()))
    );
    assert_eq!(
        stat.get("檔案").unwrap().get("等級").unwrap().to_text(),
        "5"
    );
    let 乙 = stat.get("隊員").unwrap().get("乙").unwrap();
    assert_eq!(乙.get("體力"), Some(&Json::String("50/50".into())));
    assert!(outcome
        .records
        .iter()
        .any(|record| record.kind == RecordKind::Rejected && record.path == "世界.天氣"));
    let table = landed.message_vars.as_ref().unwrap().parse().unwrap();
    assert_eq!(
        table.get("schema"),
        Some(&Json::String("没有用别管这个".into()))
    );
}

#[test]
fn gm_turn_commits_engine_table_and_feeds_back_rejections() {
    let _serial = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
    let root = TestRoot::new("zod-gm");
    let world_id = zod_world(&root, SCHEMA);
    opening(root.path(), &world_id, "開場");
    let (committed, landed) = gm_turn(
        root.path(),
        &world_id,
        "t1",
        "戰鬥。<UpdateVariable>_.add('世界.威脅', 500);_.set('稱號', 3);</UpdateVariable>",
    );
    let commit = committed.expect("提交");
    assert_eq!(commit.error, None);
    let stat = stat(&landed);
    assert_eq!(
        stat.get("世界").unwrap().get("威脅").unwrap().to_text(),
        "100"
    );
    assert_eq!(stat.get("稱號"), Some(&Json::String("見習".into())));
    assert_eq!(keys(&stat), vec!["世界", "隊員", "檔案", "標籤", "稱號"]);
    let outcome = commit.result.unwrap();
    assert_eq!(outcome.notes.len(), 1, "{:?}", outcome.notes);
    assert!(outcome.notes[0].contains("稱號"));
    assert_eq!(
        outcome.changes.get("世界.威脅").map(String::as_str),
        Some("+70")
    );
    // 狀態快取（樹）跟著新表
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(leaf(&state.state.tree, "世界.威脅").as_deref(), Some("100"));
}

#[test]
fn stale_results_are_not_committed() {
    let _serial = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
    let root = TestRoot::new("zod-stale");
    let world_id = zod_world(&root, SCHEMA);
    opening(root.path(), &world_id, "開場");
    begin(root.path(), &world_id, "t1");
    let input = with_commit(root.path(), &world_id, |tx| {
        message_vars::engine_turn_input(tx, "t1", None)
    })
    .unwrap()
    .expect("固定輸入");
    // 輸入換了（別份表）：不提交
    let mut other = input.clone();
    other.insert("額外", Json::Null);
    let stale = with_commit(root.path(), &world_id, |tx| {
        message_vars::apply_engine_turn(
            tx,
            "t1",
            Some(&other),
            main_text("t1"),
            |_| ((), Some(Json::empty_object())),
            |_, _| Vec::new(),
        )
    })
    .unwrap();
    assert!(stale.is_none());
    // 桌世代換了（回合被清掉）：不提交
    with_commit(root.path(), &world_id, |tx| {
        message_vars::bump_generation(tx)
    });
    let stale = with_commit(root.path(), &world_id, |tx| {
        message_vars::apply_engine_turn(
            tx,
            "t1",
            Some(&input),
            main_text("t1"),
            |_| ((), Some(Json::empty_object())),
            |_, _| Vec::new(),
        )
    })
    .unwrap();
    assert!(stale.is_none());
}

#[test]
fn a_whole_failure_lands_the_text_without_variables_or_state() {
    let _serial = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
    let root = TestRoot::new("zod-fail");
    let world_id = zod_world(&root, SCHEMA);
    let (opened, _) = opening(root.path(), &world_id, "開場");
    let before = data::read_state(root.path(), &world_id).unwrap().state.tree;
    let flood = "_.set('世界.威脅', 1);".repeat(1001);
    let (committed, landed) = gm_turn(root.path(), &world_id, "t1", &flood);
    let commit = committed.expect("正文照常登記");
    let error = commit.error.expect("回饋玩家");
    assert!(
        error.contains("mvu_update_failed_too_many_commands"),
        "{error}"
    );
    assert!(landed.message_vars.is_none());
    assert_eq!(landed.text, "GM t1");
    assert_eq!(
        data::read_state(root.path(), &world_id).unwrap().state.tree,
        before
    );
    assert!(commit
        .result
        .unwrap()
        .records
        .iter()
        .any(|record| record.kind == RecordKind::Error));
    // 下一輪照常以上一份有效表（開場）為底
    let (next, landed) = gm_turn(root.path(), &world_id, "t2", "_.set('世界.威脅', 40);");
    assert_eq!(next.unwrap().error, None);
    assert_eq!(
        stat(&landed)
            .get("世界")
            .unwrap()
            .get("威脅")
            .unwrap()
            .to_text(),
        "40"
    );
    assert_eq!(
        stat(&opened)
            .get("世界")
            .unwrap()
            .get("威脅")
            .unwrap()
            .to_text(),
        "30"
    );
}

#[test]
fn busy_and_failure_messages_name_the_limit_and_reason() {
    let busy = failure_message(&EngineFailure::new(FailureKind::Busy, "滿"));
    assert!(busy.contains("mvu_engine_busy"), "{busy}");
    let memory = failure_message(&EngineFailure::new(FailureKind::Memory, "炸"));
    assert!(memory.contains("mvu_update_failed_memory"), "{memory}");
    let stack = failure_message(&EngineFailure::new(FailureKind::Stack, "深"));
    assert!(stack.contains("mvu_update_failed_stack"), "{stack}");
}

#[test]
fn retracting_reads_the_stored_snapshot_without_rerunning_transforms() {
    let _serial = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
    let root = TestRoot::new("zod-retract");
    // 每次過 schema 都加一：重跑 transform 就看得出來
    let counter = "import { registerMvuSchema } from 'https://testingcf.jsdelivr.net/gh/StageDog/tavern_resource/dist/util/mvu_zod.js';\nlet runs = 0;\nregisterMvuSchema(z.object({ 次數: z.any().transform(() => ++runs) }));";
    let world_id = zod_world(&root, counter);
    opening(root.path(), &world_id, "開場");
    let (_, first) = gm_turn(root.path(), &world_id, "t1", "_.set('次數', 0);");
    let (_, second) = gm_turn(root.path(), &world_id, "t2", "_.set('次數', 0);");
    let first_count = stat(&first).get("次數").unwrap().to_text();
    assert_ne!(stat(&second).get("次數").unwrap().to_text(), first_count);
    assert!(data::pop_transcript(root.path(), &world_id, 0).unwrap());
    let source = with_commit(root.path(), &world_id, |tx| {
        message_vars::activate_for_opening(tx, &macros(), None)
    })
    .unwrap()
    .unwrap();
    assert_eq!(
        source
            .get("stat_data")
            .unwrap()
            .get("次數")
            .unwrap()
            .to_text(),
        first_count
    );
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(leaf(&state.state.tree, "次數"), Some(first_count));
}

fn add_once_trigger(root: &Path, world_id: &str, flag: &str) {
    let mut state = data::read_state(root, world_id).unwrap();
    assert!(state.mechanism.incremental, "initvar 匯入後是增量桌");
    state.mechanism.triggers.push(data::Trigger {
        id: "事件".into(),
        title: "事件".into(),
        mode: data::TriggerMode::Once,
        cases: vec![data::TriggerCase {
            when: Vec::new(),
            text: "發生了".into(),
        }],
        preamble: String::new(),
        scope: Vec::new(),
        flag: Some(flag.into()),
    });
    data::write_state(root, world_id, &state).unwrap();
}

#[test]
fn post_processing_that_passes_the_schema_is_kept() {
    let _serial = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
    let root = TestRoot::new("zod-post-ok");
    let world_id = zod_world(&root, SCHEMA);
    opening(root.path(), &world_id, "開場");
    add_once_trigger(root.path(), &world_id, "劇情.已觸發");
    let (committed, landed) = gm_turn(root.path(), &world_id, "t1", "_.set('世界.天氣', '晴');");
    let commit = committed.unwrap();
    assert_eq!(
        commit.state.state.triggers.get("事件").map(String::as_str),
        Some("發生了")
    );
    let outcome = commit.result.unwrap();
    assert!(outcome
        .records
        .iter()
        .any(|record| record.kind == RecordKind::Absorbed));
    let stat = stat(&landed);
    assert_eq!(
        stat.get("劇情").unwrap().get("已觸發"),
        Some(&Json::Bool(true))
    );
    assert_eq!(
        stat.get("世界").unwrap().get("天氣"),
        Some(&Json::String("晴".into()))
    );
}

#[test]
fn post_processing_rejected_by_the_schema_is_rolled_back_with_its_triggers_and_records() {
    let _serial = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
    let root = TestRoot::new("zod-post-revert");
    let world_id = zod_world(&root, SCHEMA);
    opening(root.path(), &world_id, "開場");
    // 旗標寫成 true，但 威脅 的 schema 會把它轉成數字 1：transform 改了值 → 整組退回
    add_once_trigger(root.path(), &world_id, "世界.威脅");
    let (committed, landed) = gm_turn(root.path(), &world_id, "t1", "_.set('世界.天氣', '晴');");
    let commit = committed.unwrap();
    assert!(
        commit.state.state.triggers.is_empty(),
        "{:?}",
        commit.state.state.triggers
    );
    let outcome = commit.result.unwrap();
    assert!(!outcome
        .records
        .iter()
        .any(|record| record.kind == RecordKind::Absorbed));
    assert!(outcome
        .records
        .iter()
        .any(|record| record.kind == RecordKind::Error && record.path == "後處理"));
    let stat = stat(&landed);
    assert_eq!(
        stat.get("世界").unwrap().get("威脅").unwrap().to_text(),
        "30"
    );
    assert_eq!(
        stat.get("世界").unwrap().get("天氣"),
        Some(&Json::String("晴".into()))
    );
}

#[test]
fn zod_detection_skips_refactored_tables_and_tells_about_extra_schema_cards() {
    let _serial = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
    let root = TestRoot::new("zod-detect");
    let world_id = zod_world(&root, SCHEMA);
    assert!(zod_table(root.path(), &world_id).is_some());
    let plain = data::create_world(root.path(), "一般桌").unwrap();
    assert!(zod_table(root.path(), &plain).is_none());
    data::write_refactor_outcome(root.path(), &world_id, "{}").unwrap();
    assert!(zod_table(root.path(), &world_id).is_none());
    let _ = wiring::shared();
}

#[test]
fn a_seed_that_cannot_be_built_fails_the_whole_update_without_materializing_the_tree() {
    let _serial = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
    let root = TestRoot::new("zod-seed-fail");
    // schema 載入就拋錯：建種子失敗，不得改從樹物化續跑
    let broken = "throw new Error('壞'); registerMvuSchema(z.object({}));";
    let world_id = zod_world(&root, broken);
    let (landed, outcome) = opening(
        root.path(),
        &world_id,
        "開場<initvar>\n世界: {}\n</initvar>",
    );
    assert!(landed.message_vars.is_none());
    assert!(outcome
        .records
        .iter()
        .any(|record| record.kind == RecordKind::Error && record.path == "開局"));
    assert!(
        with_commit(root.path(), &world_id, message_vars::needs_seed).unwrap(),
        "沒有啟用變數模式"
    );
    let (committed, landed) = gm_turn(root.path(), &world_id, "t1", "_.set('世界.威脅', 1);");
    let commit = committed.expect("正文照常登記");
    let error = commit.error.expect("回饋玩家");
    assert!(error.contains("mvu_update_failed_schema_load"), "{error}");
    assert!(landed.message_vars.is_none());
    assert!(with_commit(root.path(), &world_id, message_vars::needs_seed).unwrap());
}

#[test]
fn without_initvar_the_tree_seed_still_goes_through_the_schema_and_the_opening_override() {
    let _serial = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
    let root = TestRoot::new("zod-tree-seed");
    let world_id = data::create_world(root.path(), "zod桌").unwrap();
    import_card(root.path(), &world_id, "冒險隊", SMALL, None);
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    state
        .state
        .tree
        .insert("金".into(), StateNode::Leaf("500".into()));
    data::write_state(root.path(), &world_id, &state).unwrap();
    let (landed, _) = opening(root.path(), &world_id, "開場");
    let seeded = stat(&landed);
    assert_eq!(
        seeded.get("金").unwrap().to_text(),
        "100",
        "樹的值過 schema 夾值"
    );
    assert_eq!(
        seeded.get("名"),
        Some(&Json::String("無名".into())),
        "schema 預設值補上"
    );

    let root = TestRoot::new("zod-tree-override");
    let world_id = data::create_world(root.path(), "zod桌").unwrap();
    import_card(root.path(), &world_id, "冒險隊", SMALL, None);
    let (landed, _) = opening(root.path(), &world_id, "<initvar>\n金: 9\n</initvar>開場");
    let seeded = stat(&landed);
    assert_eq!(seeded.get("金").unwrap().to_text(), "9");
    assert_eq!(seeded.get("名"), Some(&Json::String("無名".into())));
}

#[test]
fn the_first_imported_schema_stays_in_effect_after_reordering() {
    let _serial = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
    let root = TestRoot::new("zod-order");
    let world_id = data::create_world(root.path(), "zod桌").unwrap();
    let first = import_card(root.path(), &world_id, "先來", SCHEMA, Some(INITVAR));
    std::thread::sleep(std::time::Duration::from_millis(3));
    let second = import_card(root.path(), &world_id, "後到", SMALL, None);
    let effective = zod_table(root.path(), &world_id).unwrap();
    assert_eq!(effective.char_name, "先來");
    assert_eq!(effective.skipped, vec!["後到".to_owned()]);
    data::reorder_characters(root.path(), &world_id, &[second.clone(), first.clone()]).unwrap();
    let names: Vec<String> = data::list_characters(root.path(), &world_id)
        .unwrap()
        .into_iter()
        .map(|meta| meta.name)
        .collect();
    assert_eq!(
        names.first().map(String::as_str),
        Some("後到"),
        "側欄順序真的換了"
    );
    let reordered = zod_table(root.path(), &world_id).unwrap();
    assert_eq!(reordered.char_name, "先來");
    assert_eq!(reordered.schema.hash, effective.schema.hash);
}

#[test]
fn results_computed_under_a_replaced_schema_or_generation_are_not_used() {
    let _serial = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
    let root = TestRoot::new("zod-credential");
    let world_id = data::create_world(root.path(), "zod桌").unwrap();
    let first = import_card(root.path(), &world_id, "先來", SCHEMA, Some(INITVAR));
    std::thread::sleep(std::time::Duration::from_millis(3));
    import_card(root.path(), &world_id, "後到", SMALL, None);
    opening(root.path(), &world_id, "開場");
    let block = crate::transport::extract_state_block("");
    let run_turn = |turn_id: &str| {
        begin(root.path(), &world_id, turn_id);
        let zod = zod_table(root.path(), &world_id).unwrap();
        let (input, run) = prepare_turn(
            root.path(),
            &world_id,
            turn_id,
            &zod,
            "_.set('世界.天氣', '晴');",
            &block,
            USER,
            &macros(),
            super::wiring::shared().deadline(),
        )
        .unwrap()
        .unwrap();
        (zod.schema.hash, input, run)
    };
    // 對照：憑證沒變就提交
    let (schema, input, run) = run_turn("t1");
    let kept = commit_turn(
        root.path(),
        &world_id,
        "t1",
        schema,
        input,
        run,
        main_text("t1"),
        |_, _| Vec::new(),
    )
    .unwrap();
    assert!(kept.is_some());
    data::append_event(
        root.path(),
        &world_id,
        0,
        &event(TranscriptKind::Narration, "GM t1"),
        Some(&TurnKey {
            turn_id: "t1".into(),
            part: message_vars::PART_MAIN.into(),
        }),
    )
    .unwrap();
    // 段 2 跑完、段 3 之前生效的 schema 換了（先匯入的那張封存）：輸入相同也不提交
    let (schema, input, run) = run_turn("t2");
    data::set_character_archived(root.path(), &world_id, &first, true).unwrap();
    assert_ne!(
        zod_table(root.path(), &world_id).unwrap().schema.hash,
        schema
    );
    let stale = commit_turn(
        root.path(),
        &world_id,
        "t2",
        schema,
        input,
        run,
        main_text("t2"),
        |_, _| Vec::new(),
    )
    .unwrap();
    assert!(stale.is_none());

    // 開場白：鎖外算完後 schema 或桌世代換了 → 整次失敗，正文照落、不帶表
    for change in ["schema", "generation"] {
        let root = TestRoot::new(&format!("zod-opening-credential-{change}"));
        let world_id = data::create_world(root.path(), "zod桌").unwrap();
        let first = import_card(root.path(), &world_id, "先來", SCHEMA, Some(INITVAR));
        std::thread::sleep(std::time::Duration::from_millis(3));
        import_card(root.path(), &world_id, "後到", SMALL, None);
        let prepared = wiring::prepare_opening(root.path(), &world_id, "開場", &block, USER)
            .unwrap()
            .unwrap();
        match change {
            "schema" => data::set_character_archived(root.path(), &world_id, &first, true).unwrap(),
            _ => with_commit(root.path(), &world_id, |tx| {
                message_vars::bump_generation(tx)
            }),
        }
        let mut world = data::read_state(root.path(), &world_id).unwrap();
        let (table, outcome) = with_commit(root.path(), &world_id, |tx| {
            wiring::opening_result(tx, &prepared, &mut world, USER)
        })
        .unwrap();
        assert!(table.is_none(), "{change}");
        assert!(
            outcome
                .records
                .iter()
                .any(|record| record.kind == RecordKind::Error),
            "{change}"
        );
    }
}
