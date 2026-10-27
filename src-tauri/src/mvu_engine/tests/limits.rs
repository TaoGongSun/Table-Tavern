//! 上限：記憶體、期限、堆疊、輸入大小（規格 2.2 節）。觸限與逾時測試先用同桌同 schema 的無害值暖機，
//! 受測的那一次才給 3 秒（`measured`），建 runtime 不跟受測動作搶期限；觸限會拆掉 runtime，下一次受測前再暖機。

use super::*;

/// 炸彈桌：`n` 是模組層計數器（每批加一，runtime 重建才會從 1 重新數），`a` 收到「炸」就跑 `bomb`
fn bomb_schema(bomb: &str) -> Arc<SchemaScript> {
    script(&format!(
        "let runs = 0;\nregisterMvuSchema(z.object({{ n: z.any().transform(() => ++runs), a: z.any().transform(v => {{ if (v === '炸') {{ {bomb} }} return v; }}), b: z.any() }}));"
    ))
}

fn bomb_counter(engine: &MvuEngine, name: &str, schema: &Arc<SchemaScript>) -> Value {
    batch(
        engine,
        name,
        schema,
        "_.set('n', 0);",
        json!({ "n": 0, "a": 0, "b": 0 }),
    )
    .unwrap()
    .detailed
    .table["stat_data"]["n"]
        .clone()
}

/// 同桌同 schema（雜湊不變，重建只會是觸限拆掉的）：觸限前計數 1→2，`b` 先設再炸、整次回 Err，
/// 觸限後計數重新 1→2。回失敗給呼叫端斷言 kind／detail
fn bomb_and_rebuild(engine: &MvuEngine, name: &str, bomb: &str) -> EngineFailure {
    let schema = bomb_schema(bomb);
    assert_eq!(bomb_counter(engine, name, &schema), json!(1), "{bomb}");
    assert_eq!(bomb_counter(engine, name, &schema), json!(2), "{bomb}");
    let failure = batch_until(
        engine,
        name,
        &schema,
        "_.set('b', 1);_.set('a', '炸');",
        json!({ "n": 0, "a": 0, "b": 0 }),
        measured(),
    )
    .unwrap_err();
    assert_eq!(
        bomb_counter(engine, name, &schema),
        json!(1),
        "{bomb}: 沒重建"
    );
    assert_eq!(bomb_counter(engine, name, &schema), json!(2), "{bomb}");
    failure
}

/// 記憶體觸限：kind 一定是 Memory；最後一塊可能連錯誤物件都建不出來，兩條分類路徑都算
fn assert_memory(failure: &EngineFailure, what: &str) {
    assert_eq!(failure.kind, FailureKind::Memory, "{what}: {failure}");
    assert!(
        failure.detail.contains("out of memory") || failure.detail.contains("丟出 null"),
        "{what}: {failure}"
    );
}

#[test]
fn memory_bombs_fail_the_whole_batch_and_the_runtime_is_rebuilt() {
    let engine = bomb_engine();
    // 字串炸彈拋 InternalError（out of memory），小物件炸彈耗盡到連錯誤物件都建不出來、丟 null：兩條分類路徑
    for (bomb, detail) in [
        ("a.push('x'.repeat(1 << 20) + i)", "out of memory"),
        ("a.push({ i: i, s: [i, i, i] })", "丟出 null"),
    ] {
        let failure = bomb_and_rebuild(
            &engine,
            "bomb",
            &format!("const a = []; for (let i = 0; ; i++) {bomb};"),
        );
        assert_eq!(failure.kind, FailureKind::Memory, "{bomb}: {failure}");
        assert!(failure.detail.contains(detail), "{bomb}: {failure}");
    }
}

/// 多次小配置累加（每塊 16 MB，參照全留到最後）：正式上限 64 MB 擋得住；迴圈有上界（64 塊＝1 GB），
/// 上限漏了就正常跑完回傳、斷言變紅，不會無限配置
#[test]
fn accumulated_buffer_and_typed_array_bombs_hit_the_default_limit() {
    let engine = MvuEngine::new(roomy());
    for alloc in [
        "new ArrayBuffer(16 << 20)",
        "new Uint8Array(16 << 20)",
        "new Float64Array(2 << 20)",
    ] {
        let failure = bomb_and_rebuild(
            &engine,
            "accumulate",
            &format!(
                "const keep = []; for (let i = 0; i < 64; i++) keep.push({alloc}); return keep.length;"
            ),
        );
        assert_memory(&failure, alloc);
    }
}

/// 不可成長的 SharedArrayBuffer 累加（沒裝 SAB 配置掛鉤，走一般配置、受上限管）；
/// 可變長度 ArrayBuffer 每次長 16 MB（realloc 路徑），先確認可變長度與小幅增長成立
#[test]
fn shared_and_slowly_resized_buffers_hit_the_limit() {
    let engine = bomb_engine();
    let failure = bomb_and_rebuild(
        &engine,
        "shared",
        "const keep = []; for (let i = 0; i < 64; i++) keep.push(new SharedArrayBuffer(16 << 20)); return keep.length;",
    );
    assert_memory(&failure, "SharedArrayBuffer 累加");
    let failure = bomb_and_rebuild(
        &engine,
        "grow",
        "const ab = new ArrayBuffer(16, { maxByteLength: 1 << 30 }); \
         if (!ab.resizable || ab.byteLength !== 16) throw new Error('前置：不可變長'); \
         ab.resize(4096); if (ab.byteLength !== 4096) throw new Error('前置：小幅增長'); \
         for (let i = 1; i <= 64; i++) ab.resize(i * (16 << 20)); return ab.byteLength;",
    );
    assert_memory(&failure, "resize 慢長");
}

/// 單次大配置：TypedArray、不可成長的 SharedArrayBuffer、resize、transfer 擴容（前置步驟先確認成立，
/// 前置失敗會是 Script 不是 Memory）。每項一桌、同桌同 schema：觸限前後各數 1→2，證明是觸限拆掉重建
#[test]
fn single_large_typed_array_shared_and_realloc_bombs_hit_the_limit() {
    let engine = bomb_engine();
    let schema = script("registerMvuSchema(z.object({}));");
    let eval = |table: &str, code: &str, deadline: Instant| {
        engine.run_op(table, &schema, Op::Eval(code.into()), deadline)
    };
    let count = |table: &str| {
        eval(
            table,
            "String(globalThis.k = (globalThis.k || 0) + 1)",
            engine.deadline(),
        )
        .unwrap()
    };
    for (i, code) in [
        "String(new Uint8Array(512 << 20).length)",
        "String(new Int32Array(128 << 20).length)",
        "String(new SharedArrayBuffer(512 << 20).byteLength)",
        "const ab = new ArrayBuffer(16, { maxByteLength: 1 << 30 }); \
         if (!ab.resizable || ab.byteLength !== 16) throw new Error('前置：不可變長'); \
         ab.resize(4096); if (ab.byteLength !== 4096) throw new Error('前置：小幅增長'); \
         ab.resize(512 << 20); String(ab.byteLength)",
        "const ab = new ArrayBuffer(16); if (ab.byteLength !== 16) throw new Error('前置：建構'); \
         const t = ab.transfer(4096); if (t.byteLength !== 4096 || !ab.detached) throw new Error('前置：小幅 transfer'); \
         String(t.transfer(512 << 20).byteLength)",
    ]
    .into_iter()
    .enumerate()
    {
        let table = format!("single-{i}");
        assert_eq!((count(&table), count(&table)), ("1".into(), "2".into()), "{code}");
        let failure = eval(&table, code, measured()).map(|_| ()).unwrap_err();
        assert_memory(&failure, code);
        assert_eq!(
            (count(&table), count(&table)),
            ("1".into(), "2".into()),
            "{code}: 沒重建"
        );
    }
}

/// 對照：配置 64 次、每次用完就丟（同時存活的 backing store 約 4 MiB），總量 256 MiB 也照常跑完回傳——
/// 上限算的是同時存活的量，不是累計配置量
#[test]
fn allocating_and_dropping_buffers_stays_under_the_limit() {
    for (engine, label) in [(MvuEngine::new(roomy()), "64 MB"), (bomb_engine(), "32 MB")] {
        let schema = bomb_schema(
            "let total = 0; for (let i = 0; i < 64; i++) { const ab = new ArrayBuffer(4 << 20); total += ab.byteLength; } return total;",
        );
        let output = batch(
            &engine,
            "churn",
            &schema,
            "_.set('a', '炸');",
            json!({ "n": 0, "a": 0, "b": 0 }),
        )
        .unwrap_or_else(|failure| panic!("{label}: {failure}"));
        assert_eq!(
            output.detailed.table["stat_data"]["a"],
            json!(64 * 4 * 1024 * 1024),
            "{label}"
        );
    }
}

#[test]
fn transform_infinite_loop_is_a_whole_failure_not_a_rejection() {
    let engine = MvuEngine::new(roomy());
    let schema = script("registerMvuSchema(z.object({ a: z.any().transform(v => { if (v === '轉') { while (true) {} } return v; }), b: z.any() }));");
    batch(&engine, "loop", &schema, "", json!({ "a": 0, "b": 0 })).unwrap();
    let started = Instant::now();
    let failure = batch_until(
        &engine,
        "loop",
        &schema,
        "_.set('b', 1);_.set('a', '轉');_.set('b', 2);",
        json!({ "a": 0, "b": 0 }),
        measured(),
    )
    .unwrap_err();
    assert_eq!(failure.kind, FailureKind::Deadline, "{failure}");
    assert!(
        started.elapsed() < MEASURED + slack(),
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
    let engine = MvuEngine::new(roomy());
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
    let engine = bomb_engine();
    let bomb = script("registerMvuSchema(z.object({ a: z.any().transform(v => { if (v === '炸') { const a = []; for (let i = 0; ; i++) a.push('x'.repeat(1 << 20) + i); } return v; }) }));");
    // 暖機：不觸發就照常回傳
    assert_eq!(
        engine
            .initialize("init", &bomb, &json!({ "a": 1 }), engine.deadline())
            .unwrap()
            .stat_data,
        json!({ "a": 1 })
    );
    let failure = engine
        .initialize("init", &bomb, &json!({ "a": "炸" }), measured())
        .unwrap_err();
    assert_eq!(failure.kind, FailureKind::Memory, "{failure}");
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
    let engine = MvuEngine::new(roomy());
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
    let engine = MvuEngine::new(roomy());
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

#[test]
fn spike_limits_on_raw_runtime_code() {
    let engine = MvuEngine::new(roomy());
    let schema = script("registerMvuSchema(z.object({}));");
    let eval = |code: &str, deadline: Instant| {
        engine.run_op("raw", &schema, Op::Eval(code.into()), deadline)
    };
    // 每個受測片段都會拆掉 runtime：先暖機，片段本身只給 3 秒
    let kind = |code: &str| {
        assert_eq!(eval("String(1 + 1)", engine.deadline()).unwrap(), "2");
        eval(code, measured()).map(|_| ()).unwrap_err().kind
    };
    assert_eq!(
        kind("String(new ArrayBuffer(512 * 1024 * 1024).byteLength)"),
        FailureKind::Memory
    );
    assert_eq!(
        kind("let s = 'a'; for (let i = 0; i < 40; i++) s = s + s; String(s.length)"),
        FailureKind::Memory
    );
    assert_eq!(eval("String(1 + 1)", engine.deadline()).unwrap(), "2");
    let started = Instant::now();
    assert_eq!(
        eval(
            "(async function () { for (;;) { await null; } })()",
            measured()
        )
        .map(|_| ())
        .unwrap_err()
        .kind,
        FailureKind::Deadline
    );
    assert!(started.elapsed() < MEASURED + slack());
    assert_eq!(
        kind("new Promise(function () {})"),
        FailureKind::Script {
            code: Some("never-settled".into())
        }
    );
    // 觸限、中斷之後照常可用
    assert_eq!(eval("String(1 + 1)", engine.deadline()).unwrap(), "2");
}
