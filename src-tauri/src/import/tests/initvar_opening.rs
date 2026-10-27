//! initvar-nonzod-overwrite：非 zod 卡片變數模式的開場白 `<initvar>`（上游 initCheck）——有套上就整份取代
//! `[initvar]` 物化的初始值，解析走共用核心（YAML 優先、JSON5、jsonrepair；保序）；沒套上、引擎失敗照舊物化。
use crate::data::{self, StateNode};
use crate::import::import_character;
use crate::import::test_support::TestRoot;
use crate::mechanism::{Outcome, RecordKind};
use serde_json::{json, Value};

const INITVAR: &str = "角色:\n  金幣: 10\n  地點: 村莊\n";

fn card(initvar: &str, mvu_script: bool) -> String {
    let extensions = if mvu_script {
        json!({"tavern_helper": {"scripts": [{
            "name": "MVU", "enabled": true, "type": "script",
            "content": "import 'https://x/MagVarUpdate/bundle.js';"
        }]}})
    } else {
        json!({})
    };
    json!({"data": {
        "name": "艾琳",
        "first_mes": "開場",
        "extensions": extensions,
        "character_book": {"entries": [
            {"comment": "[initvar]变量初始化勿开", "enabled": false, "content": initvar}
        ]}
    }})
    .to_string()
}

fn table(label: &str, initvar: &str, mvu_script: bool) -> (TestRoot, String) {
    let root = TestRoot::new(label);
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    import_character(
        root.path(),
        &world_id,
        card(initvar, mvu_script).as_bytes(),
        "#3366ff",
        "zh-TW",
    )
    .unwrap();
    (root, world_id)
}

fn opening(root: &TestRoot, world_id: &str, raw: &str) -> (data::TranscriptEvent, Outcome) {
    let _serial = crate::mvu_engine::TEST_SERIAL
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    data::append_opening(
        root.path(),
        world_id,
        0,
        "opening",
        raw,
        &crate::transport::extract_state_block(raw),
        "阿濤",
    )
    .unwrap()
}

fn stat(event: &data::TranscriptEvent) -> Value {
    let table = event.message_vars.as_ref().unwrap().parse().unwrap();
    serde_json::from_str(&table.get("stat_data").unwrap().to_text()).unwrap()
}

/// 開場白帶 `<initvar>` 的 stat_data
fn overridden(label: &str, raw: &str) -> Value {
    let (root, world_id) = table(label, INITVAR, true);
    stat(&opening(&root, &world_id, raw).0)
}

fn keys(value: &Value) -> Vec<&str> {
    value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect()
}

#[test]
fn fenced_yaml_block_replaces_whole_initvar_keeping_order_and_tree() {
    let (root, world_id) = table("initvar-open-yaml", INITVAR, true);
    let raw = "開場旁白\n<initvar>\n```yaml\n角色:\n  金幣: 999\n  職業: 騎士\n```\n</initvar>";
    let (event, _) = opening(&root, &world_id, raw);
    let stat = stat(&event);
    assert_eq!(stat, json!({"角色": {"金幣": 999, "職業": "騎士"}}));
    assert_eq!(keys(&stat["角色"]), ["金幣", "職業"]);
    let state = data::read_state(root.path(), &world_id).unwrap();
    let Some(StateNode::Branch(role)) = state.state.tree.get("角色") else {
        panic!("角色不是分支");
    };
    assert_eq!(role.keys().collect::<Vec<_>>(), ["職業", "金幣"]);
    assert_eq!(event.state.unwrap().tree, state.state.tree);
}

#[test]
fn json5_block_and_macros_use_effective_seed_names() {
    let stat = overridden(
        "initvar-open-json5",
        "<initvar>{角色: {金幣: 5, /* 註解 */ 名字: '{{user}}', 對象: \"{{char}}\",},}</initvar>",
    );
    assert_eq!(
        stat,
        json!({"角色": {"金幣": 5, "名字": "阿濤", "對象": "艾琳"}})
    );
}

#[test]
fn blocks_merge_and_failed_blocks_are_skipped() {
    let stat = overridden(
        "initvar-open-mixed",
        "<initvar>\n甲: 1\n清單: [1, 2]\n</initvar>中段<initvar>*&:|*[</initvar><INITVAR>\n乙: 2\n清單: [3]\n</INITVAR>",
    );
    assert_eq!(stat, json!({"甲": 1, "清單": [3], "乙": 2}));
}

#[test]
fn all_failed_blocks_keep_materialized_seed_with_metadata_cleaned() {
    let initvar = "$meta:\n  strictSet: true\n角色:\n  金幣: 10\n";
    let (plain_root, plain_world) = table("initvar-open-meta-plain", initvar, true);
    let plain = stat(&opening(&plain_root, &plain_world, "開場").0);
    let (root, world_id) = table("initvar-open-meta-failed", initvar, true);
    let (event, outcome) = opening(&root, &world_id, "開場<initvar>*&:|*[</initvar>");
    assert_eq!(stat(&event), plain);
    assert_eq!(plain, json!({"角色": {"金幣": 10}}));
    assert!(outcome.records.iter().all(|record| record.path != "開局"));
}

#[test]
fn applied_block_with_same_values_still_replaces_key_order() {
    let stat = overridden(
        "initvar-open-same",
        "<initvar>\n角色:\n  金幣: 10\n  地點: 村莊\n  $meta: {strictSet: true}\n</initvar>",
    );
    assert_eq!(stat, json!({"角色": {"金幣": 10, "地點": "村莊"}}));
    assert_eq!(keys(&stat["角色"]), ["金幣", "地點"]);
}

#[test]
fn empty_object_block_empties_the_seed() {
    assert_eq!(
        overridden("initvar-open-empty", "<initvar>{}</initvar>"),
        json!({})
    );
}

#[test]
fn yaml_comment_after_number_parses_as_number() {
    let stat = overridden(
        "initvar-open-comment",
        "<initvar>\n金幣: 999 # 註解\n</initvar>",
    );
    assert_eq!(stat, json!({"金幣": 999}));
}

#[test]
fn opening_commands_run_on_overridden_base() {
    let stat = overridden(
        "initvar-open-command",
        "<initvar>\n角色:\n  金幣: 999\n</initvar>\n<UpdateVariable><JSONPatch>[\
        {\"op\":\"delta\",\"path\":\"/角色/金幣\",\"value\":1}\
        ]</JSONPatch></UpdateVariable>",
    );
    assert_eq!(stat, json!({"角色": {"金幣": 1000}}));
}

#[test]
fn second_opening_does_not_reapply() {
    let (root, world_id) = table("initvar-open-twice", INITVAR, true);
    let first = stat(&opening(&root, &world_id, "<initvar>\n甲: 1\n</initvar>").0);
    let second = stat(&opening(&root, &world_id, "<initvar>\n乙: 2\n</initvar>").0);
    assert_eq!(first, json!({"甲": 1}));
    assert_eq!(second, first);
}

#[test]
fn tree_mode_table_without_mvu_script_is_untouched() {
    let (root, world_id) = table("initvar-open-tree", INITVAR, false);
    let (event, _) = opening(&root, &world_id, "<initvar>\n甲: 1\n</initvar>");
    assert!(event.message_vars.is_none());
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(state.state.tree.contains_key("角色"));
    assert!(!state.state.tree.contains_key("甲"));
}

#[test]
fn engine_failure_falls_back_to_materialized_seed_and_records() {
    let (root, world_id) = table("initvar-open-too-large", INITVAR, true);
    let raw = format!("<initvar>\n甲: 1\n</initvar>{}", "長".repeat(1024 * 1024));
    let (event, outcome) = opening(&root, &world_id, &raw);
    assert_eq!(stat(&event), json!({"角色": {"地點": "村莊", "金幣": 10}}));
    let record = outcome
        .records
        .iter()
        .find(|record| record.path == "開局")
        .expect("記一筆開局帳");
    assert_eq!(record.kind, RecordKind::Error);
    assert!(record.detail.contains("<initvar>"), "{}", record.detail);
}

// ---- 引擎忙與判定失敗（鎖外 prepare；用自己的引擎，名額只開 1 個） ----

use crate::mvu_engine::plain_opening::{prepare_with, PlainOpening};
use crate::mvu_engine::{Limits, Macros as EngineMacros, MvuEngine, SchemaScript};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const BUSY_RAW: &str = "<initvar>\n甲: 1\n</initvar>";

fn one_slot_engine(batch: Duration) -> Arc<MvuEngine> {
    Arc::new(MvuEngine::new(Limits {
        batch,
        max_busy_threads: 1,
        ..Limits::default()
    }))
}

/// 另一桌占住唯一的名額約 `hold`（zod schema 的 transform 忙等；自己的期限放寬到 10 秒）
fn occupy(engine: &Arc<MvuEngine>, hold: Duration) -> thread::JoinHandle<()> {
    let (signal, ready) = std::sync::mpsc::channel();
    let started = {
        let engine = Arc::clone(engine);
        thread::spawn(move || {
            let schema = Arc::new(SchemaScript::new(format!(
            "import {{ registerMvuSchema }} from 'https://testingcf.jsdelivr.net/gh/StageDog/tavern_resource/dist/util/mvu_zod.js';\n\
             registerMvuSchema(z.object({{ a: z.any().transform(v => {{ if (v === '等') {{ const end = Date.now() + {}; while (Date.now() < end) {{}} }} return v; }}) }}));",
            hold.as_millis()
        )));
            let old = json!({ "stat_data": { "a": 0 } })
                .as_object()
                .cloned()
                .unwrap();
            let macros = EngineMacros {
                user: "玩家".into(),
                char: None,
            };
            let deadline = Instant::now() + Duration::from_secs(10);
            signal.send(()).unwrap();
            let _ = engine.run_batch(
                "佔位",
                &schema,
                "_.set('a', '等');",
                &old,
                &macros,
                deadline,
            );
        })
    };
    // 占位執行緒開跑後（名額在進引擎第一步、建 runtime 之前就拿走）再留一段餘裕
    ready.recv().unwrap();
    thread::sleep(Duration::from_millis(200));
    started
}

#[test]
fn busy_engine_is_retried_within_the_opening_deadline() {
    let (root, world_id) = table("initvar-open-busy-retry", INITVAR, true);
    let engine = one_slot_engine(Duration::from_secs(8));
    let occupant = occupy(&engine, Duration::from_millis(600));
    let started = Instant::now();
    let prepared = prepare_with(&engine, root.path(), &world_id, BUSY_RAW, "阿濤").unwrap();
    // 有等到占位的那一個做完（第一次撞到 Busy、重試才成功）
    assert!(started.elapsed() >= Duration::from_millis(300));
    occupant.join().unwrap();
    let PlainOpening::Override(stat) = prepared else {
        panic!("忙完應該重試成功");
    };
    assert_eq!(stat.to_text(), json!({ "甲": 1 }).to_string());
}

#[test]
fn engine_busy_past_the_deadline_falls_back_with_a_record() {
    let (root, world_id) = table("initvar-open-busy-fail", INITVAR, true);
    let engine = one_slot_engine(Duration::from_millis(500));
    let occupant = occupy(&engine, Duration::from_millis(2000));
    let prepared = prepare_with(&engine, root.path(), &world_id, BUSY_RAW, "阿濤").unwrap();
    occupant.join().unwrap();
    let PlainOpening::Failed(detail) = prepared else {
        panic!("期限內一直忙應該退回");
    };
    // 最後一次是 Busy，或重試睡過頭剛好到期（期限檢查）；兩者都是期限內一直沒拿到名額
    assert!(
        detail.contains("上限") || detail.contains("期限"),
        "{detail}"
    );
}

#[test]
fn unreadable_table_detection_falls_back_with_a_record() {
    let (root, world_id) = table("initvar-open-detect-fail", INITVAR, true);
    // 重構產物讀不到（非 UTF-8）：zod 判定失敗，但這桌照樣符合卡片變數模式
    let outcome = data::refactor_outcome_path(root.path(), &world_id).unwrap();
    std::fs::write(outcome, [0xff, 0xfe, 0xfd]).unwrap();
    let (event, outcome) = opening(&root, &world_id, BUSY_RAW);
    assert_eq!(stat(&event), json!({"角色": {"地點": "村莊", "金幣": 10}}));
    let record = outcome
        .records
        .iter()
        .find(|record| record.path == "開局")
        .expect("記一筆開局帳");
    assert!(record.detail.contains("判定"), "{}", record.detail);
}
