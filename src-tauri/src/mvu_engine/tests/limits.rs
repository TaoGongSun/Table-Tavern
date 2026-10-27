//! 上限：記憶體、期限、堆疊、輸入大小（規格 2.2 節）。觸限與逾時測試先用同桌同 schema 的無害值暖機，
//! 受測的那一次才給 3 秒（`measured`），建 runtime 不跟受測動作搶期限；觸限會拆掉 runtime，下一次受測前再暖機。

use super::*;

#[test]
fn memory_bombs_fail_the_whole_batch_and_the_runtime_is_rebuilt() {
    let engine = bomb_engine();
    // 字串炸彈拋 InternalError（out of memory），小物件炸彈耗盡到連錯誤物件都建不出來、丟 null：兩條分類路徑
    for (bomb, detail) in [
        ("a.push('x'.repeat(1 << 20) + i)", "out of memory"),
        ("a.push({ i: i, s: [i, i, i] })", "丟出 null"),
    ] {
        let schema = script(&format!(
            "registerMvuSchema(z.object({{ a: z.any().transform(v => {{ if (v === '炸') {{ const a = []; for (let i = 0; ; i++) {bomb}; }} return v; }}), b: z.any() }}));"
        ));
        batch(
            &engine,
            "bomb",
            &schema,
            "_.set('a', 1);",
            json!({ "a": 0, "b": 0 }),
        )
        .unwrap();
        let failure = batch_until(
            &engine,
            "bomb",
            &schema,
            "_.set('b', 1);_.set('a', '炸');",
            json!({ "a": 0, "b": 0 }),
            measured(),
        )
        .unwrap_err();
        assert_eq!(failure.kind, FailureKind::Memory, "{bomb}: {failure}");
        assert!(failure.detail.contains(detail), "{bomb}: {failure}");
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
