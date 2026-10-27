//! QuickJS 宿主：Node 基準對照、上限（記憶體、期限、堆疊）、schema 載入失敗、模組白名單、隔離、
//! 執行緒作廢與重開上限、LRU、重建時機（規格 2.1、2.2 節；短驗證 scratchpad/mvu-spike 的測項）。

use super::host::Op;
use super::runtime::is_mvu_zod_url;
use super::*;
use serde_json::{json, Map, Value};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const MVU_ZOD: &str =
    "https://testingcf.jsdelivr.net/gh/StageDog/tavern_resource/dist/util/mvu_zod.js";

fn script(body: &str) -> Arc<SchemaScript> {
    Arc::new(SchemaScript::new(format!(
        "import {{ registerMvuSchema }} from '{MVU_ZOD}';\n{body}"
    )))
}

fn macros() -> Macros {
    Macros {
        user: "玩家".into(),
        char: None,
    }
}

fn table(stat: Value) -> Map<String, Value> {
    json!({ "stat_data": stat }).as_object().cloned().unwrap()
}

/// 測試用的期限：cargo test 全套並行時機器很忙，debug 版建 runtime 可能超過 1 秒，留足餘裕。
/// 量時間的測試先把 runtime 建好（`counter`），期限只花在要測的那一步
fn quick() -> Limits {
    Limits {
        batch: Duration::from_millis(3000),
        outer_grace: Duration::from_millis(1000),
        ..Limits::default()
    }
}

/// 期限加上忙碌機器的排程誤差
fn slack() -> Duration {
    Duration::from_millis(1500)
}

fn batch(
    engine: &MvuEngine,
    name: &str,
    schema: &Arc<SchemaScript>,
    message: &str,
    stat: Value,
) -> Result<BatchOutput, EngineFailure> {
    engine.run_batch(
        name,
        schema,
        message,
        &table(stat),
        &macros(),
        engine.deadline(),
    )
}

/// 等作廢的執行緒自己結束（卡住的測試操作做完）
fn wait_unstuck(engine: &MvuEngine) {
    let until = Instant::now() + Duration::from_secs(10);
    while engine.stuck_threads() > 0 && Instant::now() < until {
        thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(engine.stuck_threads(), 0);
}

fn statuses(output: &BatchOutput) -> Vec<CommandStatus> {
    output
        .detailed
        .results
        .iter()
        .map(|result| result.status)
        .collect()
}

/// 每批都加一的 transform：看得出 runtime 有沒有重建（重建後從 1 重新數）
const COUNTER: &str =
    "let runs = 0;\nregisterMvuSchema(z.object({ n: z.any().transform(() => ++runs) }));";

fn counter(engine: &MvuEngine, name: &str, schema: &Arc<SchemaScript>) -> Value {
    batch(engine, name, schema, "_.set('n', 0);", json!({ "n": 0 }))
        .unwrap()
        .detailed
        .table["stat_data"]["n"]
        .clone()
}

// ---- Node 基準對照 ----

#[test]
fn corpus_matches_node_golden() {
    let golden: Value = serde_json::from_str(include_str!(
        "../../../src/features/mvu-engine/fixtures/zod-corpus-golden.json"
    ))
    .unwrap();
    let schema = Arc::new(SchemaScript::new(
        include_str!("../../../src/features/mvu-engine/fixtures/clean-zod-schema.txt").into(),
    ));
    let engine = MvuEngine::new(Limits::default());
    let init = engine
        .initialize("corpus", &schema, &golden["initvar"], engine.deadline())
        .unwrap();
    assert_eq!(init.invalid, None);
    assert_eq!(init.stat_data, golden["init"]["stat_data"]);
    let start = table(init.stat_data);
    let cases = golden["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 41);
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let output = engine.run_batch(
            "corpus",
            &schema,
            case["message"].as_str().unwrap(),
            &start,
            &macros(),
            engine.deadline(),
        );
        let expected: DetailedResult = serde_json::from_value(case.clone()).unwrap();
        let got = output
            .unwrap_or_else(|failure| panic!("{name}: {failure}"))
            .detailed;
        assert_eq!(got, expected, "{name}");
    }
}

/// 三張真卡（TestCards 本機限定，CI 略過）：基準由 quickjs-entry.test.ts 在 Node 跑出來寫進 TestCards/
#[test]
fn real_cards_match_node_golden() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../TestCards/zod-cards-golden.json"
    );
    let Ok(text) = std::fs::read_to_string(path) else {
        eprintln!("略過：沒有 {path}（先在有 TestCards 的 checkout 跑 vitest）");
        return;
    };
    let golden: Value = serde_json::from_str(&text).unwrap();
    let engine = MvuEngine::new(Limits::default());
    for card in golden.as_array().unwrap() {
        let name = card["name"].as_str().unwrap();
        let schema = Arc::new(SchemaScript::new(card["schema"].as_str().unwrap().into()));
        let init = engine
            .initialize(name, &schema, &card["initvar"], engine.deadline())
            .unwrap();
        assert_eq!(init.invalid, None, "{name}");
        assert_eq!(init.stat_data, card["init"]["stat_data"], "{name}");
        let output = engine.run_batch(
            name,
            &schema,
            card["message"].as_str().unwrap(),
            &table(init.stat_data),
            &macros(),
            engine.deadline(),
        );
        let expected: DetailedResult = serde_json::from_value(card["detailed"].clone()).unwrap();
        assert_eq!(output.unwrap().detailed, expected, "{name}");
    }
}

// ---- 上限 ----

#[test]
fn memory_bombs_fail_the_whole_batch_and_the_runtime_is_rebuilt() {
    let engine = MvuEngine::new(quick());
    for bomb in [
        "a.push('x'.repeat(1 << 20) + i)",
        "a.push({ i: i, s: [i, i, i] })",
    ] {
        let schema = script(&format!(
            "registerMvuSchema(z.object({{ a: z.any().transform(v => {{ if (v === '炸') {{ const a = []; for (let i = 0; ; i++) {bomb}; }} return v; }}), b: z.any() }}));"
        ));
        let failure = batch(
            &engine,
            "bomb",
            &schema,
            "_.set('b', 1);_.set('a', '炸');",
            json!({ "a": 0, "b": 0 }),
        )
        .unwrap_err();
        assert_eq!(failure.kind, FailureKind::Memory, "{bomb}: {failure}");
        let after = batch(
            &engine,
            "bomb",
            &schema,
            "_.set('a', 2);",
            json!({ "a": 0, "b": 0 }),
        )
        .unwrap();
        assert_eq!(after.detailed.table["stat_data"]["a"], json!(2));
    }
}

#[test]
fn transform_infinite_loop_is_a_whole_failure_not_a_rejection() {
    let engine = MvuEngine::new(quick());
    let schema = script("registerMvuSchema(z.object({ a: z.any().transform(v => { if (v === '轉') { while (true) {} } return v; }), b: z.any() }));");
    batch(&engine, "loop", &schema, "", json!({ "a": 0, "b": 0 })).unwrap();
    let started = Instant::now();
    let failure = batch(
        &engine,
        "loop",
        &schema,
        "_.set('b', 1);_.set('a', '轉');_.set('b', 2);",
        json!({ "a": 0, "b": 0 }),
    )
    .unwrap_err();
    assert_eq!(failure.kind, FailureKind::Deadline, "{failure}");
    assert!(
        started.elapsed() < quick().batch + slack(),
        "{:?}",
        started.elapsed()
    );
    // 中斷後重建，照常可用
    assert!(batch(
        &engine,
        "loop",
        &schema,
        "_.set('a', 1);",
        json!({ "a": 0, "b": 0 })
    )
    .is_ok());
}

#[test]
fn unbounded_recursion_hits_the_stack_limit() {
    let engine = MvuEngine::new(quick());
    let schema = script("function f(n) { return f(n + 1) + 1; }\nregisterMvuSchema(z.object({ a: z.any().transform(v => v === '遞迴' ? f(0) : v) }));");
    let failure = batch(
        &engine,
        "stack",
        &schema,
        "_.set('a', '遞迴');",
        json!({ "a": 0 }),
    )
    .unwrap_err();
    assert_eq!(failure.kind, FailureKind::Stack, "{failure}");
}

#[test]
fn initialize_fails_on_resource_errors_but_reports_validation_failures() {
    let engine = MvuEngine::new(quick());
    let bomb = script("registerMvuSchema(z.object({ a: z.any().transform(v => { const a = []; for (let i = 0; ; i++) a.push('x'.repeat(1 << 20) + i); }) }));");
    assert_eq!(
        engine
            .initialize("init", &bomb, &json!({ "a": 1 }), engine.deadline())
            .unwrap_err()
            .kind,
        FailureKind::Memory
    );
    let strict = script("registerMvuSchema(z.object({ a: z.number() }));");
    let invalid = engine
        .initialize(
            "init",
            &strict,
            &json!({ "a": "x", "$meta": { "extensible": true } }),
            engine.deadline(),
        )
        .unwrap();
    let (issues, count) = invalid.invalid.unwrap();
    assert_eq!((issues.len(), count), (1, 1));
    // 驗不過：照原值（元資料已清）
    assert_eq!(invalid.stat_data, json!({ "a": "x" }));
}

#[test]
fn one_deadline_spans_every_request_of_a_run() {
    let engine = MvuEngine::new(quick());
    let schema = script("registerMvuSchema(z.object({ a: z.any().transform(v => { if (v === '轉') { while (true) {} } return v; }) }));");
    batch(&engine, "shared", &schema, "", json!({ "a": 0 })).unwrap();
    // 前一步用掉大半期限，後一步不重新起算
    let deadline = Instant::now() + Duration::from_millis(1500);
    engine
        .run_op(
            "shared",
            &schema,
            Op::Stall(Duration::from_millis(600)),
            deadline,
        )
        .unwrap();
    let started = Instant::now();
    let failure = engine
        .run_batch(
            "shared",
            &schema,
            "_.set('a', '轉');",
            &table(json!({ "a": 0 })),
            &macros(),
            deadline,
        )
        .unwrap_err();
    assert_eq!(failure.kind, FailureKind::Deadline);
    assert!(
        started.elapsed() < Duration::from_millis(900) + slack() / 2,
        "{:?}",
        started.elapsed()
    );
    // 期限已過：不進引擎
    let started = Instant::now();
    let late = engine
        .run_batch(
            "shared",
            &schema,
            "_.set('a', 1);",
            &table(json!({ "a": 0 })),
            &macros(),
            deadline,
        )
        .unwrap_err();
    assert_eq!(late.kind, FailureKind::Deadline);
    assert!(started.elapsed() < Duration::from_millis(50));
}

#[test]
fn inputs_over_parse_limits_never_reach_the_engine() {
    let engine = MvuEngine::new(quick());
    let schema = script("registerMvuSchema(z.object({}));");
    let message = "a".repeat(256 * 1024 + 1);
    assert_eq!(
        batch(&engine, "big", &schema, &message, json!({}))
            .unwrap_err()
            .kind,
        FailureKind::InputTooLarge
    );
    let huge = json!({ "s": "字".repeat(700 * 1024) });
    assert_eq!(
        batch(&engine, "big", &schema, "", huge).unwrap_err().kind,
        FailureKind::InputTooLarge
    );
    assert!(engine.tables().is_empty());
}

// ---- schema 載入 ----

#[test]
fn schema_load_failures_stop_before_the_core_runs() {
    let engine = MvuEngine::new(quick());
    let cases = [
        ("拋錯", "throw new Error('壞掉');"),
        ("沒註冊", "export const Schema = z.object({});"),
        (
            "展開拋錯",
            "registerMvuSchema(() => { throw new Error('展開失敗'); });",
        ),
        ("無窮迴圈", "while (true) {}"),
        (
            "記憶體",
            "const a = []; for (let i = 0; ; i++) a.push('x'.repeat(1 << 20) + i);",
        ),
        (
            "動態 import",
            "registerMvuSchema(z.object({}));\nawait import('https://example.com/x.js');",
        ),
        (
            "非同步拋錯",
            "registerMvuSchema(z.object({}));\nawait Promise.reject(new Error('晚一點壞'));",
        ),
    ];
    for (name, body) in cases {
        let failure = batch(
            &engine,
            "load",
            &script(body),
            "_.set('a', 1);",
            json!({ "a": 0 }),
        )
        .unwrap_err();
        // 載入時逾時歸期限（可重試），其餘歸載入失敗；兩者都不呼叫核心
        let expected = if name == "無窮迴圈" {
            FailureKind::Deadline
        } else {
            FailureKind::SchemaLoad
        };
        assert_eq!(failure.kind, expected, "{name}: {failure}");
    }
}

#[test]
fn imports_outside_the_whitelist_fail_to_load() {
    let engine = MvuEngine::new(quick());
    for specifier in [
        "fs",
        "os",
        "std",
        "qjs:os",
        "https://example.com/x.js",
        "./local.js",
        "/etc/passwd",
        "https://cdn.jsdelivr.net/npm/zod",
    ] {
        let source = format!(
            "import x from '{specifier}';\nregisterMvuSchema(z.object({{}}));\nglobalThis.y = x;"
        );
        let failure = batch(&engine, "import", &script(&source), "", json!({})).unwrap_err();
        assert_eq!(failure.kind, FailureKind::SchemaLoad, "{specifier}");
    }
}

#[test]
fn mvu_zod_url_whitelist() {
    for url in [
        MVU_ZOD,
        "https://cdn.jsdelivr.net/gh/StageDog/tavern_resource/dist/util/mvu_zod.js",
        "https://fastly.jsdelivr.net/gh/StageDog/tavern_resource@main/dist/util/mvu_zod.js",
        "https://testingcf.jsdelivr.net/gh/StageDog/tavern_resource@1.2.3/dist/util/mvu_zod.js",
    ] {
        assert!(is_mvu_zod_url(url), "{url}");
    }
    for url in [
        "http://testingcf.jsdelivr.net/gh/StageDog/tavern_resource/dist/util/mvu_zod.js",
        "https://evil.example/gh/StageDog/tavern_resource/dist/util/mvu_zod.js",
        "https://cdn.jsdelivr.net/gh/StageDog/tavern_resource/dist/util/mvu.js",
        "https://cdn.jsdelivr.net/gh/StageDog/tavern_resource@/dist/util/mvu_zod.js",
        "https://cdn.jsdelivr.net/gh/StageDog/tavern_resource@x/../dist/util/mvu_zod.js",
        "https://cdn.jsdelivr.net/gh/StageDog/tavern_resourcex/dist/util/mvu_zod.js",
        "https://cdn.jsdelivr.net/gh/StageDog/tavern_resource/dist/util/mvu_zod.js?x=1",
        "mvu_zod",
    ] {
        assert!(!is_mvu_zod_url(url), "{url}");
    }
}

#[test]
fn sandbox_has_no_host_apis_and_no_engine_handles() {
    let engine = MvuEngine::new(quick());
    let schema = script("registerMvuSchema(z.object({}));");
    let probe = "JSON.stringify({ \
        present: ['fetch','XMLHttpRequest','WebSocket','require','process','Deno','std','os','importScripts','navigator','document','window','Worker','WebAssembly','TTMvu','__ttRegisterMvuSchema'].filter(k => typeof globalThis[k] !== 'undefined'), \
        card: [typeof z, typeof _, typeof $] })";
    let report: Value = serde_json::from_str(
        &engine
            .run_op(
                "sandbox",
                &schema,
                Op::Eval(probe.into()),
                engine.deadline(),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        report,
        json!({ "present": [], "card": ["object", "function", "function"] })
    );
    let imports = "Promise.all(['fs','qjs:os','https://example.com/x.js','./x.js'].map(s => import(s).then(() => 'loaded', () => 'blocked'))).then(r => JSON.stringify(r))";
    let blocked: Value = serde_json::from_str(
        &engine
            .run_op(
                "sandbox",
                &schema,
                Op::Eval(imports.into()),
                engine.deadline(),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(blocked, json!(["blocked", "blocked", "blocked", "blocked"]));
}

#[test]
fn duplicate_registrations_are_counted() {
    let engine = MvuEngine::new(quick());
    let schema =
        script("registerMvuSchema(z.object({ a: z.number() }));\nregisterMvuSchema(z.object({}));");
    let init = engine
        .initialize("dup", &schema, &json!({ "a": 1 }), engine.deadline())
        .unwrap();
    assert_eq!(init.duplicates, 1);
    let output = batch(
        &engine,
        "dup",
        &schema,
        "_.set('a', 'x');",
        json!({ "a": 1 }),
    )
    .unwrap();
    // 只認第一份：a 必須是數字
    assert_eq!(statuses(&output), vec![CommandStatus::Rejected]);
}

// ---- 執行緒、作廢與重建 ----

#[test]
fn abandoned_threads_are_capped_and_counted_until_they_exit() {
    // 期限要夠 debug 版建 runtime（約 0.3 秒），卡住的時間要超過期限＋寬限
    let limits = Limits {
        batch: Duration::from_millis(1000),
        outer_grace: Duration::from_millis(200),
        max_busy_threads: 2,
        ..Limits::default()
    };
    let engine = MvuEngine::new(limits);
    let schema = script(COUNTER);
    let stall = |name: &str| {
        engine.run_op(
            name,
            &schema,
            Op::Stall(Duration::from_millis(4000)),
            engine.deadline(),
        )
    };
    counter(&engine, "a", &schema);
    let started = Instant::now();
    assert_eq!(stall("a").unwrap_err().kind, FailureKind::Deadline);
    assert!(
        started.elapsed() < Duration::from_millis(1200) + slack(),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(engine.stuck_threads(), 1);
    counter(&engine, "b", &schema);
    assert_eq!(stall("b").unwrap_err().kind, FailureKind::Deadline);
    assert_eq!(engine.stuck_threads(), 2);
    // 名額被兩條卡死的執行緒佔滿：新請求一律拒收（已有 runtime 的桌也一樣）
    let generous = Limits::default().batch;
    let deadline = || Instant::now() + generous;
    let busy = engine
        .run_batch(
            "c",
            &schema,
            "_.set('n', 0);",
            &table(json!({ "n": 0 })),
            &macros(),
            deadline(),
        )
        .unwrap_err();
    assert_eq!(busy.kind, FailureKind::Busy);
    // 卡住的執行緒結束後計數歸零，又能開新的
    wait_unstuck(&engine);
    assert!(engine
        .run_batch(
            "c",
            &schema,
            "_.set('n', 0);",
            &table(json!({ "n": 0 })),
            &macros(),
            deadline()
        )
        .is_ok());
}

/// Sol 第 2 輪重現：三桌相隔 150 ms 一起卡住，池只留兩桌、先開的被淘汰，作廢數仍不得超過名額
#[test]
fn overlapping_stalls_on_many_tables_never_exceed_the_global_cap() {
    let engine = Arc::new(MvuEngine::new(Limits {
        batch: Duration::from_millis(1500),
        outer_grace: Duration::from_millis(200),
        ..Limits::default()
    }));
    let schema = script("registerMvuSchema(z.object({}));");
    let mut jobs = vec![];
    for name in ["a", "b", "c"] {
        let (engine, schema) = (Arc::clone(&engine), Arc::clone(&schema));
        jobs.push(thread::spawn(move || {
            engine
                .run_op(
                    name,
                    &schema,
                    Op::Stall(Duration::from_secs(5)),
                    engine.deadline(),
                )
                .unwrap_err()
                .kind
        }));
        thread::sleep(Duration::from_millis(150));
    }
    let kinds: Vec<FailureKind> = jobs.into_iter().map(|job| job.join().unwrap()).collect();
    assert_eq!(
        kinds,
        vec![
            FailureKind::Deadline,
            FailureKind::Deadline,
            FailureKind::Busy
        ]
    );
    assert!(engine.stuck_threads() <= 2, "{}", engine.stuck_threads());
    wait_unstuck(&engine);
    assert_eq!(counter(&engine, "c", &schema_counter()), json!(1));
}

fn schema_counter() -> Arc<SchemaScript> {
    script(COUNTER)
}

/// 一般使用：多桌輪流、兩桌同時，都不會被名額誤擋
#[test]
fn rotating_and_two_concurrent_tables_are_not_blocked() {
    let engine = Arc::new(MvuEngine::new(quick()));
    let schema = schema_counter();
    for name in ["a", "b", "c", "d", "a", "b", "c", "d"] {
        assert!(counter(&engine, name, &schema).is_number(), "{name}");
    }
    let pair: Vec<_> = ["x", "y"]
        .into_iter()
        .map(|name| {
            let (engine, schema) = (Arc::clone(&engine), Arc::clone(&schema));
            thread::spawn(move || counter(&engine, name, &schema))
        })
        .collect();
    for job in pair {
        assert_eq!(job.join().unwrap(), json!(1));
    }
}

#[test]
fn respawns_after_abandoning_are_capped_per_table_and_window() {
    let limits = Limits {
        batch: Duration::from_millis(1000),
        outer_grace: Duration::from_millis(100),
        max_busy_threads: 10,
        max_respawns: 2,
        ..Limits::default()
    };
    let engine = MvuEngine::new(limits);
    let schema = script(COUNTER);
    for _ in 0..2 {
        // 先建好（作廢後重開也算在這裡），卡住的只有那一步
        counter(&engine, "a", &schema);
        let failure = engine
            .run_op(
                "a",
                &schema,
                Op::Stall(Duration::from_millis(3000)),
                engine.deadline(),
            )
            .unwrap_err();
        assert_eq!(failure.kind, FailureKind::Deadline);
    }
    wait_unstuck(&engine);
    // 連續逾時的那桌在窗口內開不了新的；別桌不受影響
    let failure = engine
        .run_op("a", &schema, Op::Stall(Duration::ZERO), engine.deadline())
        .unwrap_err();
    assert_eq!(failure.kind, FailureKind::Busy);
    assert_eq!(counter(&engine, "b", &schema), json!(1));
}

#[test]
fn an_evicted_worker_that_times_out_is_still_abandoned_and_counted() {
    let limits = Limits {
        batch: Duration::from_millis(1500),
        outer_grace: Duration::from_millis(200),
        max_tables: 1,
        ..Limits::default()
    };
    let engine = Arc::new(MvuEngine::new(limits));
    let schema = script(COUNTER);
    counter(&engine, "a", &schema);
    let stalled = {
        let (engine, schema) = (Arc::clone(&engine), Arc::clone(&schema));
        thread::spawn(move || {
            engine.run_op(
                "a",
                &schema,
                Op::Stall(Duration::from_millis(5000)),
                engine.deadline(),
            )
        })
    };
    // a 還在跑時切到 b：池只留一桌，a 被淘汰
    thread::sleep(Duration::from_millis(100));
    assert_eq!(counter(&engine, "b", &schema), json!(1));
    assert_eq!(engine.tables(), vec!["b"]);
    assert_eq!(
        stalled.join().unwrap().unwrap_err().kind,
        FailureKind::Deadline
    );
    assert_eq!(engine.stuck_threads(), 1);
    wait_unstuck(&engine);
}

#[test]
fn a_reply_after_the_deadline_is_not_a_success() {
    let engine = MvuEngine::new(quick());
    let schema = script(COUNTER);
    // 先把 runtime 建好，期限只花在操作本身
    counter(&engine, "late", &schema);
    let deadline = Instant::now() + Duration::from_millis(50);
    let failure = engine
        .run_op(
            "late",
            &schema,
            Op::Stall(Duration::from_millis(200)),
            deadline,
        )
        .unwrap_err();
    assert_eq!(failure.kind, FailureKind::Deadline);
    // runtime 沒壞：照常可用、沒重建
    assert_eq!(counter(&engine, "late", &schema), json!(2));
}

/// 中斷穿過各種 catch／finally：catch 與 finally 一行都不跑、不回部分結果
#[test]
fn interrupts_are_not_caught_by_any_js_construct() {
    use super::runtime::{EngineRuntime, Guard};
    let cases = [
        ("同步 catch", "(function () { try { while (true) {} } catch (e) { caught++; return 'swallowed'; } })()"),
        ("Promise 執行器", "(async function () { try { return await new Promise(function (res, rej) { try { while (true) {} } catch (e) { caught++; rej(e); } }); } catch (e) { caught++; return 'swallowed'; } })()"),
        ("待跑中的 Promise 工作", "(async function () { await null; try { while (true) {} } catch (e) { caught++; } return 'swallowed'; })().catch(function () { caught++; return 'caught'; })"),
        ("async 互相 catch", "(async function () { async function inner() { await null; while (true) {} } try { await inner(); } catch (e) { caught++; return 'swallowed'; } return 'x'; })()"),
        ("then 鏈 catch", "Promise.resolve().then(function () { while (true) {} }).catch(function () { caught++; return 'swallowed'; })"),
        ("finally", "(async function () { var done = []; try { done.push('第一條'); await null; while (true) {} } finally { caught++; return JSON.stringify(done); } })()"),
    ];
    let schema = script("registerMvuSchema(z.object({}));");
    let guard = Guard::default();
    let far = || Instant::now() + Duration::from_secs(5);
    guard.arm(far());
    let runtime = EngineRuntime::build(&schema, &Limits::default(), &guard).unwrap();
    for (name, code) in cases {
        guard.arm(far());
        runtime
            .eval_for_test("globalThis.caught = 0; 'ready'", &guard)
            .unwrap();
        guard.arm(Instant::now() + Duration::from_millis(200));
        let failure = runtime.eval_for_test(code, &guard).unwrap_err();
        assert_eq!(failure.kind, FailureKind::Deadline, "{name}");
        guard.arm(far());
        assert_eq!(
            runtime.eval_for_test("String(caught)", &guard).unwrap(),
            "0",
            "{name}"
        );
    }
}

#[test]
fn key_insertion_order_survives_store_and_read_round_trips() {
    // 背包只留最後 3 件（照插入順序）；鍵名排序與插入順序相反
    let engine = MvuEngine::new(quick());
    let schema = script("registerMvuSchema(z.object({ 背包: z.record(z.string(), z.number()).transform(bag => Object.fromEntries(Object.entries(bag).slice(-3))), 備註: z.any() }));");
    let names: Vec<String> = (0..12).rev().map(|i| format!("物品{i:02}")).collect();
    let bag: String = names
        .iter()
        .enumerate()
        .map(|(at, name)| format!("\"{name}\":{at}"))
        .collect::<Vec<_>>()
        .join(",");
    // 存檔：文字 → Map（讀檔）→ 引擎 → 文字（寫檔）→ Map（再讀）
    let stored = format!("{{\"stat_data\":{{\"背包\":{{{bag}}},\"備註\":0}}}}");
    let read: Map<String, Value> = serde_json::from_str(&stored).unwrap();
    let output = engine
        .run_batch(
            "order",
            &schema,
            "_.set('備註', 1);",
            &read,
            &macros(),
            engine.deadline(),
        )
        .unwrap();
    let written = serde_json::to_string(&output.detailed.table).unwrap();
    let reread: Map<String, Value> = serde_json::from_str(&written).unwrap();
    let kept: Vec<&String> = reread["stat_data"]["背包"]
        .as_object()
        .unwrap()
        .keys()
        .collect();
    assert_eq!(kept, names[9..].iter().collect::<Vec<_>>());
    assert_eq!(kept, vec!["物品02", "物品01", "物品00"]);
    // 內容相同的比較不看順序
    assert_eq!(json!({ "a": 1, "b": 2 }), json!({ "b": 2, "a": 1 }));
}

#[test]
fn a_panic_fails_the_batch_and_the_table_keeps_working() {
    let engine = MvuEngine::new(quick());
    let schema = script(COUNTER);
    assert_eq!(
        engine
            .run_op("p", &schema, Op::Panic, engine.deadline())
            .unwrap_err()
            .kind,
        FailureKind::Crashed
    );
    assert_eq!(counter(&engine, "p", &schema), json!(1));
}

#[test]
fn tables_are_evicted_least_recently_used() {
    let engine = MvuEngine::new(quick());
    let schema = script(COUNTER);
    for name in ["a", "b", "c"] {
        counter(&engine, name, &schema);
    }
    assert_eq!(engine.tables(), vec!["b", "c"]);
    counter(&engine, "b", &schema);
    counter(&engine, "a", &schema);
    assert_eq!(engine.tables(), vec!["b", "a"]);
}

#[test]
fn runtime_is_kept_across_batches_and_rebuilt_on_failure_schema_change_and_idle() {
    let limits = Limits {
        idle: Duration::from_millis(300),
        ..quick()
    };
    let engine = MvuEngine::new(limits);
    let schema = script(COUNTER);
    assert_eq!(counter(&engine, "t", &schema), json!(1));
    assert_eq!(counter(&engine, "t", &schema), json!(2));
    // 失敗後重建
    engine
        .run_op("t", &schema, Op::Eval("null.x".into()), engine.deadline())
        .unwrap_err();
    assert_eq!(counter(&engine, "t", &schema), json!(1));
    // schema 換了就重建
    let other = script(&format!("{COUNTER}\n// 改版"));
    assert_eq!(counter(&engine, "t", &other), json!(1));
    assert_eq!(counter(&engine, "t", &other), json!(2));
    // 閒置過久丟掉 runtime
    thread::sleep(Duration::from_millis(700));
    assert_eq!(counter(&engine, "t", &other), json!(1));
}

#[test]
fn core_failures_carry_the_parse_failure_code() {
    let engine = MvuEngine::new(quick());
    let schema = script("registerMvuSchema(z.object({}));");
    let message = "_.set('a', 1);".repeat(1001);
    let failure = batch(&engine, "code", &schema, &message, json!({})).unwrap_err();
    assert_eq!(
        failure.kind,
        FailureKind::Script {
            code: Some("too-many-commands".into())
        },
        "{failure}"
    );
}

#[test]
fn spike_limits_on_raw_runtime_code() {
    let engine = MvuEngine::new(quick());
    let schema = script("registerMvuSchema(z.object({}));");
    let eval = |code: &str| engine.run_op("raw", &schema, Op::Eval(code.into()), engine.deadline());
    let kind = |code: &str| eval(code).map(|_| ()).unwrap_err().kind;
    assert_eq!(
        kind("String(new ArrayBuffer(512 * 1024 * 1024).byteLength)"),
        FailureKind::Memory
    );
    assert_eq!(
        kind("let s = 'a'; for (let i = 0; i < 40; i++) s = s + s; String(s.length)"),
        FailureKind::Memory
    );
    let started = Instant::now();
    assert_eq!(
        kind("(async function () { for (;;) { await null; } })()"),
        FailureKind::Deadline
    );
    assert!(started.elapsed() < quick().batch + slack());
    assert_eq!(
        kind("new Promise(function () {})"),
        FailureKind::Script {
            code: Some("never-settled".into())
        }
    );
    // 觸限、中斷之後照常可用
    assert_eq!(eval("String(1 + 1)").unwrap(), "2");
}

/// P4 會放進 Tauri state、在 spawn_blocking 裡用
#[allow(dead_code)]
fn engine_is_send_and_sync() {
    fn assert<T: Send + Sync>() {}
    assert::<MvuEngine>();
}
