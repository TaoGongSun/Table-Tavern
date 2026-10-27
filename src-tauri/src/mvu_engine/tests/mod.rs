//! QuickJS 宿主：Node 基準對照、上限（記憶體、期限、堆疊）、schema 載入失敗、模組白名單、隔離、
//! 執行緒作廢與重開上限、LRU、重建時機（規格 2.1、2.2 節；短驗證 scratchpad/mvu-spike 的測項）。

use super::host::Op;
use super::runtime::is_mvu_zod_url;
use super::*;
use serde_json::{json, Map, Value};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

/// 上限（記憶體、期限、堆疊）與輸入大小：tests/limits.rs
mod limits;

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

/// 測試引擎一律用這組上限：期限 30 秒只當安全網，給預期不是 Deadline 的呼叫（暖機、收尾、重建、載入失敗；
/// 負載衝高時 debug 版建 runtime 會超過 5 秒）。預期 Deadline 或要量時間的那一次另用 `measured()` 或自訂期限，
/// 要量時間的先用同桌同 schema 的無害值暖機（建 runtime 不算在裡面）
fn roomy() -> Limits {
    Limits {
        batch: Duration::from_secs(30),
        ..Limits::default()
    }
}

/// 會炸記憶體的測試引擎：上限縮成 32 MB（觸頂快一倍；正式值見 `Limits::default`）
fn bomb_engine() -> MvuEngine {
    MvuEngine::new(Limits {
        memory_bytes: 32 * 1024 * 1024,
        ..roomy()
    })
}

/// 受測那一次的期限長度
const MEASURED: Duration = Duration::from_millis(3000);

/// 受測那一次的期限（runtime 已暖機）
fn measured() -> Instant {
    Instant::now() + MEASURED
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
    batch_until(engine, name, schema, message, stat, engine.deadline())
}

fn batch_until(
    engine: &MvuEngine,
    name: &str,
    schema: &Arc<SchemaScript>,
    message: &str,
    stat: Value,
    deadline: Instant,
) -> Result<BatchOutput, EngineFailure> {
    engine.run_batch(name, schema, message, &table(stat), &macros(), deadline)
}

/// 等作廢的執行緒自己結束（卡住的測試操作做完）
fn wait_unstuck(engine: &MvuEngine) {
    let until = Instant::now() + Duration::from_secs(10);
    while engine.stuck_threads() > 0 && Instant::now() < until {
        thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(engine.stuck_threads(), 0);
}

/// 卡住到測試放行的操作：回（操作、已開始訊號、放行把手）。把手留在測試作用域，丟掉＝放行（panic 時也會丟）
fn hold() -> (Op, mpsc::Receiver<()>, mpsc::Sender<()>) {
    let (started, started_rx) = mpsc::sync_channel(1);
    let (release_tx, release) = mpsc::channel();
    (Op::Hold { started, release }, started_rx, release_tx)
}

/// 受測的卡住操作要真的開始過（作廢的是卡在裡面的執行緒，不是入口就因期限退回的請求）
fn assert_hold_started(started: &mpsc::Receiver<()>) {
    started
        .recv_timeout(Duration::from_secs(2))
        .expect("Hold 未開始");
}

/// 受測操作的期限：準備步驟另用寬鬆的 `Limits::batch`，不跟它搶
fn tight() -> Instant {
    Instant::now() + Duration::from_millis(1000)
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
        "../../../../src/features/mvu-engine/fixtures/zod-corpus-golden.json"
    ))
    .unwrap();
    let schema = Arc::new(SchemaScript::new(
        include_str!("../../../../src/features/mvu-engine/fixtures/clean-zod-schema.txt").into(),
    ));
    let engine = MvuEngine::new(roomy());
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
        let got = output.unwrap_or_else(|failure| panic!("{name}: {failure}"));
        assert_eq!(got.detailed, expected, "{name}");
        let feedback: Vec<Option<String>> =
            serde_json::from_value(case["feedback"].clone()).unwrap();
        assert_eq!(got.feedback, feedback, "{name}");
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
    let engine = MvuEngine::new(roomy());
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

// ---- schema 載入 ----

#[test]
fn schema_load_failures_stop_before_the_core_runs() {
    let engine = MvuEngine::new(roomy());
    let bombs = bomb_engine();
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
        // 預期逾時的那條只給 3 秒；記憶體那條用 32 MB 引擎
        let (engine, deadline) = match name {
            "無窮迴圈" => (&engine, measured()),
            "記憶體" => (&bombs, bombs.deadline()),
            _ => (&engine, engine.deadline()),
        };
        let failure = batch_until(
            engine,
            "load",
            &script(body),
            "_.set('a', 1);",
            json!({ "a": 0 }),
            deadline,
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
    let engine = MvuEngine::new(roomy());
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
    let engine = MvuEngine::new(roomy());
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
    let engine = MvuEngine::new(roomy());
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
    // 準備與收尾用寬鬆期限（debug 版建 runtime 在負載下要好幾秒）；受測的卡住操作另給 1 秒期限，卡到測試放行為止
    let limits = Limits {
        outer_grace: Duration::from_millis(200),
        max_busy_threads: 2,
        ..roomy()
    };
    let engine = MvuEngine::new(limits);
    let schema = script(COUNTER);
    counter(&engine, "a", &schema);
    let (op, a_started, a_release) = hold();
    let started = Instant::now();
    assert_eq!(
        engine.run_op("a", &schema, op, tight()).unwrap_err().kind,
        FailureKind::Deadline
    );
    assert!(
        started.elapsed() < Duration::from_millis(1200) + slack(),
        "{:?}",
        started.elapsed()
    );
    assert_hold_started(&a_started);
    assert_eq!(engine.stuck_threads(), 1);
    counter(&engine, "b", &schema);
    let (op, b_started, b_release) = hold();
    assert_eq!(
        engine.run_op("b", &schema, op, tight()).unwrap_err().kind,
        FailureKind::Deadline
    );
    assert_hold_started(&b_started);
    assert_eq!(engine.stuck_threads(), 2);
    // 名額被兩條卡死的執行緒佔滿：新請求一律拒收（已有 runtime 的桌也一樣）
    let busy = engine
        .run_batch(
            "c",
            &schema,
            "_.set('n', 0);",
            &table(json!({ "n": 0 })),
            &macros(),
            engine.deadline(),
        )
        .unwrap_err();
    assert_eq!(busy.kind, FailureKind::Busy);
    // 卡住的執行緒結束後計數歸零，又能開新的
    drop((a_release, b_release));
    wait_unstuck(&engine);
    assert!(engine
        .run_batch(
            "c",
            &schema,
            "_.set('n', 0);",
            &table(json!({ "n": 0 })),
            &macros(),
            engine.deadline()
        )
        .is_ok());
}

/// Sol 第 2 輪重現：三桌相隔 150 ms 一起卡住，池只留兩桌、先開的被淘汰，作廢數仍不得超過名額
#[test]
fn overlapping_stalls_on_many_tables_never_exceed_the_global_cap() {
    // 收尾用寬鬆期限；受測的三個卡住操作各給 1.5 秒期限
    let engine = Arc::new(MvuEngine::new(Limits {
        outer_grace: Duration::from_millis(200),
        ..roomy()
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
                    Instant::now() + Duration::from_millis(1500),
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
    let engine = Arc::new(MvuEngine::new(roomy()));
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
    // 準備用寬鬆期限；受測的卡住操作另給 1 秒期限，卡到測試放行為止
    let limits = Limits {
        outer_grace: Duration::from_millis(100),
        max_busy_threads: 10,
        max_respawns: 2,
        ..roomy()
    };
    let engine = MvuEngine::new(limits);
    let schema = script(COUNTER);
    for _ in 0..2 {
        // 先建好（作廢後重開也算在這裡），卡住的只有那一步
        counter(&engine, "a", &schema);
        let (op, started, release) = hold();
        let failure = engine.run_op("a", &schema, op, tight()).unwrap_err();
        assert_eq!(failure.kind, FailureKind::Deadline);
        assert_hold_started(&started);
        drop(release);
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
    // 準備用寬鬆期限；受測的卡住操作另給 1.5 秒期限，卡到淘汰與作廢都斷言完才放行
    let limits = Limits {
        outer_grace: Duration::from_millis(200),
        max_tables: 1,
        ..roomy()
    };
    let engine = Arc::new(MvuEngine::new(limits));
    let schema = script(COUNTER);
    counter(&engine, "a", &schema);
    let (op, started, release) = hold();
    let stalled = {
        let (engine, schema) = (Arc::clone(&engine), Arc::clone(&schema));
        thread::spawn(move || {
            engine.run_op(
                "a",
                &schema,
                op,
                Instant::now() + Duration::from_millis(1500),
            )
        })
    };
    // a 卡住後切到 b：池只留一桌，a 被淘汰
    assert_hold_started(&started);
    assert_eq!(counter(&engine, "b", &schema), json!(1));
    assert_eq!(engine.tables(), vec!["b"]);
    assert_eq!(
        stalled.join().unwrap().unwrap_err().kind,
        FailureKind::Deadline
    );
    assert_eq!(engine.stuck_threads(), 1);
    drop(release);
    wait_unstuck(&engine);
}

/// 名額剛好在邊上：一條卡住佔一個，另一桌收到回覆就立刻送下一個，不得撞到還沒歸還的名額
#[test]
fn back_to_back_requests_beside_a_held_slot_are_never_busy() {
    let engine = Arc::new(MvuEngine::new(Limits {
        max_busy_threads: 2,
        ..roomy()
    }));
    let schema = script(COUNTER);
    counter(&engine, "a", &schema);
    counter(&engine, "b", &schema);
    let (op, started, release) = hold();
    let held = {
        let (engine, schema) = (Arc::clone(&engine), Arc::clone(&schema));
        // 佔名額的那一個不受測，期限給到測試一定跑得完：結束時要是正常回覆，不是逾時作廢
        let deadline = Instant::now() + Duration::from_secs(60);
        thread::spawn(move || engine.run_op("a", &schema, op, deadline))
    };
    assert_hold_started(&started);
    for round in 0..200 {
        let reply = batch(&engine, "b", &schema, "_.set('n', 0);", json!({ "n": 0 }));
        assert!(reply.is_ok(), "第 {round} 次：{:?}", reply.err());
    }
    // 確定性版本：上一個請求回覆後執行緒先別結束這一輪，下一個請求照樣拿得到名額
    let (gate_tx, gate) = mpsc::channel();
    engine
        .run_op("b", &schema, Op::Linger(gate), engine.deadline())
        .unwrap();
    let next = engine.run_op("c", &schema, Op::Stall(Duration::ZERO), engine.deadline());
    drop(gate_tx);
    assert!(next.is_ok(), "{:?}", next.err());
    drop(release);
    assert!(held.join().unwrap().is_ok());
}

#[test]
fn a_reply_after_the_deadline_is_not_a_success() {
    let engine = MvuEngine::new(roomy());
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
    let far = || Instant::now() + roomy().batch;
    guard.arm(far());
    let runtime = EngineRuntime::build(&schema, &roomy(), &guard).unwrap();
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
    let engine = MvuEngine::new(roomy());
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
    let engine = MvuEngine::new(roomy());
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
    let engine = MvuEngine::new(roomy());
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
        ..roomy()
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
    let engine = MvuEngine::new(roomy());
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

/// P4 會放進 Tauri state、在 spawn_blocking 裡用
#[allow(dead_code)]
fn engine_is_send_and_sync() {
    fn assert<T: Send + Sync>() {}
    assert::<MvuEngine>();
}

// ---- 無 schema runtime（非 zod 桌開場白的 `<initvar>`，initvar-nonzod-overwrite） ----

fn plain_seed(
    engine: &MvuEngine,
    name: &str,
    opening: &str,
) -> Result<super::contract::InitOutput, EngineFailure> {
    engine.seed(
        name,
        &Arc::new(SchemaScript::plain()),
        SeedBase::Tree(&json!({ "金幣": 10 })),
        opening,
        &macros(),
        engine.deadline(),
    )
}

fn empty_zod_script() -> Arc<SchemaScript> {
    Arc::new(SchemaScript::new(String::new()))
}

#[test]
fn plain_runtime_seeds_without_a_schema_and_reports_applied() {
    let engine = MvuEngine::new(roomy());
    let applied = plain_seed(
        &engine,
        "plain",
        "<initvar>\n職業: 騎士\n名字: '{{user}}'\n</initvar>",
    )
    .unwrap();
    assert!(applied.applied);
    assert_eq!(applied.stat_data, json!({ "職業": "騎士", "名字": "玩家" }));
    assert!(applied.invalid.is_none());
    let untouched = plain_seed(&engine, "plain", "開場").unwrap();
    assert!(!untouched.applied);
    assert_eq!(untouched.stat_data, json!({ "金幣": 10 }));
    // 同一桌接著送空字串的 zod 腳本：快取身分不同，照舊報沒有註冊
    let failure = batch(
        &engine,
        "plain",
        &empty_zod_script(),
        "_.set('a', 1);",
        json!({ "a": 0 }),
    )
    .unwrap_err();
    assert_eq!(failure.kind, FailureKind::SchemaLoad, "{failure}");
}

#[test]
fn switching_between_plain_and_zod_on_one_table_rebuilds_the_runtime() {
    let engine = MvuEngine::new(roomy());
    let schema = script(COUNTER);
    assert_eq!(counter(&engine, "t", &schema), json!(1));
    assert!(
        plain_seed(&engine, "t", "<initvar>\n甲: 1\n</initvar>")
            .unwrap()
            .applied
    );
    // 換回 zod：runtime 重建（計數從頭），schema 照常生效
    assert_eq!(counter(&engine, "t", &schema), json!(1));
    assert_eq!(counter(&engine, "t", &schema), json!(2));
    assert!(
        plain_seed(&engine, "t", "<initvar>\n甲: 1\n</initvar>")
            .unwrap()
            .applied
    );
}

#[test]
fn zod_runtime_rebuilds_after_a_plain_table_evicts_it() {
    let engine = MvuEngine::new(Limits {
        max_tables: 1,
        ..roomy()
    });
    let schema = script(COUNTER);
    assert_eq!(counter(&engine, "zod", &schema), json!(1));
    assert!(
        plain_seed(&engine, "plain", "<initvar>\n甲: 1\n</initvar>")
            .unwrap()
            .applied
    );
    assert_eq!(engine.tables(), vec!["plain"]);
    assert_eq!(counter(&engine, "zod", &schema), json!(1));
    assert_eq!(engine.tables(), vec!["zod"]);
}
