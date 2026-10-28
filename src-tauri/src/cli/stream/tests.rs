use super::*;

// 2026-10-07 grok-4.5 實測：超過伺服器上限 500000 tokens 的 error 行（被擋、不計費，exit 1）
#[test]
fn grok_prompt_too_long_error_is_structured_failure() {
    let line = r#"{"type":"error","message":"Internal error: {\n  \"message\": \"API error (status 400 Bad Request): invalid-argument: Failed to start sampling: [input_too_large] The prompt is too long for this model's context window (544617 tokens > 500000 tokens)\",\n  \"http_status\": 400\n}"}"#;
    let CliLine::Done { text, is_error } = parse_grok_line(line) else {
        panic!("not done");
    };
    assert!(is_error);
    assert!(crate::transport::context_overflow::cli_failure(&text).is_some());
    // 字面改版：錯誤碼 input_too_large 仍認得
    let CliLine::Done { text, .. } =
        parse_grok_line(r#"{"type":"error","message":"[input_too_large] request rejected"}"#)
    else {
        panic!("not done");
    };
    assert!(crate::transport::context_overflow::cli_failure(&text).is_some());
    // 正文增量寫了同一句不是失敗
    assert_eq!(
        parse_grok_line(r#"{"type":"text","data":"The prompt is too long"}"#),
        CliLine::Delta("The prompt is too long".to_owned())
    );
}

// 2026-10-06 haiku 實測：超長請求的收尾行（被擋、不計費）
#[test]
fn claude_prompt_too_long_result_is_structured_failure() {
    let line = r#"{"type":"result","subtype":"success","is_error":true,"api_error_status":400,"terminal_reason":"prompt_too_long","stop_reason":"stop_sequence","result":"Prompt is too long · the request is ~902529 tokens (limit 200000)"}"#;
    let CliLine::Done { text, is_error } = parse_claude_line(line) else {
        panic!("not done");
    };
    assert!(is_error);
    assert!(crate::transport::context_overflow::cli_failure(&text).is_some());
    // 字面改版：結構化欄位仍認得
    let reworded = r#"{"type":"result","is_error":true,"terminal_reason":"prompt_too_long","result":"Request exceeds limits"}"#;
    let CliLine::Done { text, .. } = parse_claude_line(reworded) else {
        panic!("not done");
    };
    assert!(crate::transport::context_overflow::cli_failure(&text).is_some());
    // is_error 缺省或 false：結構化欄位說太長仍一律失敗
    for line in [
        r#"{"type":"result","terminal_reason":"prompt_too_long","result":"Prompt is too long"}"#,
        r#"{"type":"result","is_error":false,"terminal_reason":"prompt_too_long","result":"x"}"#,
    ] {
        let CliLine::Done { text, is_error } = parse_claude_line(line) else {
            panic!("not done");
        };
        assert!(is_error, "{line}");
        assert!(crate::transport::context_overflow::cli_failure(&text).is_some());
    }
    // 成功回覆的正文就算寫了同一句也不是失敗
    assert_eq!(
        parse_claude_line(
            r#"{"type":"result","is_error":false,"result":"他說：Prompt is too long"}"#
        ),
        CliLine::Done {
            text: "他說：Prompt is too long".to_owned(),
            is_error: false
        }
    );
}

#[test]
fn claude_context_window_stop_is_incomplete_failure() {
    let line = r#"{"type":"result","is_error":false,"stop_reason":"model_context_window_exceeded","result":"半截"}"#;
    let CliLine::Done { text, is_error } = parse_claude_line(line) else {
        panic!("not done");
    };
    assert!(is_error);
    assert!(text.starts_with("AI_INCOMPLETE_RESPONSE:"));
}

// 樣本取自 2026-07-19 真實 CLI 冒煙輸出（scratchpad claude-smoke.jsonl／codex-smoke.jsonl）
#[test]
fn parses_real_claude_stream_json_lines() {
    assert_eq!(
        parse_claude_line(
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"測"}}}"#
        ),
        CliLine::Delta("測".to_owned())
    );
    assert_eq!(
        parse_claude_line(
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"内心"}}}"#
        ),
        CliLine::Thinking("内心".to_owned())
    );
    // 2026-08-12 實測樣本：4.7 世代 CLI 思考本文隱去，空增量轉心跳點
    assert_eq!(
        parse_claude_line(
            r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"","estimated_tokens":50}}}"#
        ),
        CliLine::Thinking("⋯".to_owned())
    );
    assert_eq!(
        parse_claude_line(
            r#"{"type":"result","subtype":"success","is_error":false,"result":"測試"}"#
        ),
        CliLine::Done {
            text: "測試".to_owned(),
            is_error: false
        }
    );
    assert_eq!(
        parse_claude_line(
            r#"{"type":"result","is_error":true,"result":"Failed to authenticate. API Error: 401 Invalid bearer token"}"#
        ),
        CliLine::Done {
            text: "Failed to authenticate. API Error: 401 Invalid bearer token".to_owned(),
            is_error: true
        }
    );
    assert_eq!(parse_claude_line("not json"), CliLine::Other);
}

/// 1h 拆分：只有 cache_creation 給了數字才算已知；缺欄、空物件、null、非數字都是未知；
/// 大於總寫入的壞值壓回總寫入。
#[test]
fn parses_claude_one_hour_split_only_when_numeric() {
    let line = |split: &str| {
        format!(
            r#"{{"type":"result","usage":{{"input_tokens":1,"cache_creation_input_tokens":1000,"cache_read_input_tokens":0,"output_tokens":3{split}}}}}"#
        )
    };
    let one_hour = |split: &str| parse_claude_usage(&line(split)).unwrap().created_1h_tokens;
    assert_eq!(
        one_hour(
            r#","cache_creation":{"ephemeral_1h_input_tokens":1000,"ephemeral_5m_input_tokens":0}"#
        ),
        Some(1000)
    );
    assert_eq!(
        one_hour(
            r#","cache_creation":{"ephemeral_1h_input_tokens":0,"ephemeral_5m_input_tokens":1000}"#
        ),
        Some(0)
    );
    assert_eq!(
        one_hour(r#","cache_creation":{"ephemeral_1h_input_tokens":5000}"#),
        Some(1000)
    );
    for unknown in [
        "",
        r#","cache_creation":{}"#,
        r#","cache_creation":{"ephemeral_1h_input_tokens":null}"#,
        r#","cache_creation":{"ephemeral_1h_input_tokens":"x"}"#,
    ] {
        assert_eq!(one_hour(unknown), None, "{unknown}");
    }
}

/// 超額：只有在用超額且沒被拒才算；不是 rate_limit_event 的行一律 None。
#[test]
fn claude_overage_reads_rate_limit_event() {
    let event = |info: &str| {
        format!(
            r#"{{"type":"rate_limit_event","rate_limit_info":{info},"uuid":"u","session_id":"s"}}"#
        )
    };
    assert_eq!(
        claude_overage(&event(
            r#"{"status":"allowed","isUsingOverage":true,"overageStatus":"allowed"}"#
        )),
        Some(true)
    );
    assert_eq!(
        claude_overage(&event(
            r#"{"status":"allowed","isUsingOverage":true,"overageStatus":"allowed_warning"}"#
        )),
        Some(true)
    );
    assert_eq!(
        claude_overage(&event(r#"{"status":"allowed","isUsingOverage":true}"#)),
        Some(true)
    );
    assert_eq!(
        claude_overage(&event(
            r#"{"status":"rejected","isUsingOverage":true,"overageStatus":"rejected"}"#
        )),
        Some(false)
    );
    // 實跑樣本（2026-10-07，本帳號超額未開通）
    assert_eq!(
        claude_overage(&event(
            r#"{"status":"allowed","resetsAt":1791327600,"rateLimitType":"five_hour","overageStatus":"rejected","overageDisabledReason":"org_level_disabled","isUsingOverage":false}"#
        )),
        Some(false)
    );
    assert_eq!(
        claude_overage(r#"{"type":"result","result":"rate_limit_event"}"#),
        None
    );
    assert_eq!(claude_overage("not json"), None);
}

/// 樣本取自 2026-08-03 真實冒煙輸出（同一段 system prompt 連跑兩次：第一次建快取、
/// 第二次全命中）。各家 input_tokens 語意不同，這裡鎖住換算後的「總輸入／讀快取」。
#[test]
fn parses_claude_usage_adding_cache_tokens_to_input() {
    // 第一次：建快取 4771，讀 0 → 總輸入 4772、命中 0%；金額直接取 CLI 回報值
    assert_eq!(
        parse_claude_usage(
            r#"{"type":"result","total_cost_usd":0.0179,"usage":{"input_tokens":1,"cache_creation_input_tokens":4771,"cache_read_input_tokens":0,"output_tokens":3}}"#
        ),
        Some(PromptCacheUsage {
            created_1h_tokens: None,
            prompt_tokens: 4772,
            cached_tokens: Some(0),
            created_tokens: Some(4771),
            output_tokens: 3,
            cost_usd: Some(0.0179),
        })
    );
    // 第二次：讀滿 4771 → 總輸入 4772、命中 100%
    let hit = parse_claude_usage(
        r#"{"type":"result","usage":{"input_tokens":1,"cache_creation_input_tokens":0,"cache_read_input_tokens":4771,"output_tokens":3}}"#
    )
    .expect("result 事件有 usage");
    assert_eq!(hit.prompt_tokens, 4772);
    assert_eq!(hit.cached_tokens, Some(4771));
    assert!((hit.hit_rate().expect("claude 有回報") - 99.98).abs() < 0.01);
    // 增量行與非收尾事件不出數字
    assert_eq!(
        parse_claude_usage(
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"好"}}}"#
        ),
        None
    );
    assert_eq!(parse_claude_usage("not json"), None);
}

/// codex 的 input_tokens 已含 cached_input_tokens，不可再加總（會虛報分母、低估命中率）。
#[test]
fn parses_codex_usage_without_double_counting_cached_input() {
    let usage = parse_codex_usage(
        r#"{"type":"turn.completed","usage":{"input_tokens":23144,"cached_input_tokens":11008,"cache_write_input_tokens":0,"output_tokens":5}}"#
    )
    .expect("turn.completed 有 usage");
    assert_eq!(usage.prompt_tokens, 23144);
    assert_eq!(usage.cached_tokens, Some(11008));
    assert!((usage.hit_rate().expect("codex 有回報") - 47.56).abs() < 0.01);
    assert_eq!(parse_codex_usage(r#"{"type":"turn.started"}"#), None);
}

/// grok 的 cache_read 可遠大於 input_tokens，證明兩者不重疊，總輸入要加總。
/// 金額在頂層 total_cost_usd（包 6 額度分頁要顯示），缺欄時仍能解析、只是沒金額。
#[test]
fn parses_grok_usage_adding_cache_read_to_input() {
    let usage = parse_grok_usage(
        r#"{"type":"end","stopReason":"EndTurn","total_cost_usd":0.0421,"usage":{"input_tokens":31509,"cache_read_input_tokens":146304,"output_tokens":1416,"total_tokens":179229}}"#
    )
    .expect("end 事件有 usage");
    assert_eq!(usage.prompt_tokens, 177813);
    assert_eq!(usage.cached_tokens, Some(146304));
    assert_eq!(usage.created_tokens, None); // grok 不回報寫入數：None 不是 0
    assert_eq!(usage.cost_usd, Some(0.0421));
    assert!((usage.hit_rate().expect("grok 有回報") - 82.28).abs() < 0.01);
    assert_eq!(
        parse_grok_usage(r#"{"type":"end","usage":{"input_tokens":10}}"#)
            .expect("缺金額欄仍解析")
            .cost_usd,
        None
    );
    assert_eq!(parse_grok_usage(r#"{"type":"text","data":"好"}"#), None);
}

/// 缺欄位當 0，不可 panic 也不可整筆丟掉（CLI 版本變動時仍留下可讀的一行）。
#[test]
fn missing_usage_fields_count_as_zero() {
    assert_eq!(
        parse_claude_usage(r#"{"type":"result","usage":{"input_tokens":12}}"#),
        Some(PromptCacheUsage {
            created_1h_tokens: None,
            prompt_tokens: 12,
            cached_tokens: Some(0),
            created_tokens: Some(0),
            output_tokens: 0,
            cost_usd: None, // 沒回報金額就不記，額度分頁靠有值的那些行加總
        })
    );
    assert_eq!(
        PromptCacheUsage {
            created_1h_tokens: None,
            prompt_tokens: 0,
            cached_tokens: Some(0),
            created_tokens: Some(0),
            output_tokens: 0,
            cost_usd: None,
        }
        .hit_rate(),
        Some(0.0)
    );
}

/// 頂層 error（重試中的 Reconnecting 也是）不結束回合，交給收尾規則；turn.failed 才是失敗。
#[test]
fn codex_top_level_error_is_soft() {
    assert_eq!(
        parse_codex_line(
            r#"{"type":"error","message":"Reconnecting... 2/5 (stream disconnected)"}"#
        ),
        CliLine::SoftError("Reconnecting... 2/5 (stream disconnected)".to_owned())
    );
    assert!(matches!(
        parse_codex_line(r#"{"type":"error"}"#),
        CliLine::SoftError(text) if !text.is_empty()
    ));
}

#[test]
fn codex_rollout_save_failure_warning_is_recognized() {
    assert!(codex_rollout_save_failed(
        r#"{"type":"item.completed","item":{"id":"w","type":"error","message":"Failed to save the conversation transcript; Codex will continue retrying. Error: x"}}"#
    ));
    // 正文寫了同一句不算
    assert!(!codex_rollout_save_failed(
        r#"{"type":"item.completed","item":{"id":"m","type":"agent_message","text":"Failed to save the conversation transcript"}}"#
    ));
    assert!(!codex_rollout_save_failed(r#"{"type":"turn.completed"}"#));
}

#[test]
fn codex_thread_id_and_cumulative_usage() {
    let started = r#"{"type":"thread.started","thread_id":"01a1812e-3b6c-7a62-8346-bb5f6c99dcbc"}"#;
    assert_eq!(
        codex_thread_id(started).as_deref(),
        Some("01a1812e-3b6c-7a62-8346-bb5f6c99dcbc")
    );
    assert_eq!(codex_thread_id(r#"{"type":"turn.started"}"#), None);
    assert_eq!(
        codex_thread_id(r#"{"type":"thread.started","thread_id":""}"#),
        None
    );
    // 真實累計值（2026-10-28 實測第 1、2 輪）
    let first = codex_usage_counters(
        r#"{"type":"turn.completed","usage":{"input_tokens":20948,"cached_input_tokens":11136,"cache_write_input_tokens":0,"output_tokens":93,"reasoning_output_tokens":0}}"#,
    )
    .unwrap();
    let second = codex_usage_counters(
        r#"{"type":"turn.completed","usage":{"input_tokens":42034,"cached_input_tokens":32000,"cache_write_input_tokens":0,"output_tokens":171,"reasoning_output_tokens":0}}"#,
    )
    .unwrap();
    let delta = codex_usage_delta(second, first).unwrap();
    assert_eq!(delta.prompt_tokens, 21086);
    assert_eq!(delta.cached_tokens, Some(20864));
    assert_eq!(delta.output_tokens, 78);
    // 倒退＝基準對不上，不猜
    assert_eq!(codex_usage_delta(first, second), None);
}

#[test]
fn parses_real_codex_json_lines_and_ignores_warning_items() {
    assert_eq!(
        parse_codex_line(
            r#"{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"測試"}}"#
        ),
        CliLine::Delta("測試\n".to_owned())
    );
    // 非致命警告（真實輸出：hooks 提示）不可視為失敗
    assert_eq!(
        parse_codex_line(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"error","message":"skipping async hook"}}"#
        ),
        CliLine::Other
    );
    assert_eq!(
        parse_codex_line(r#"{"type":"turn.completed","usage":{"input_tokens":15208}}"#),
        CliLine::Done {
            text: String::new(),
            is_error: false
        }
    );
    assert_eq!(
        parse_codex_line(r#"{"type":"turn.failed","error":{"message":"quota exceeded"}}"#),
        CliLine::Done {
            text: "quota exceeded".to_owned(),
            is_error: true
        }
    );
}

#[test]
/// 樣本取自 agy 1.1.17 實跑（`-p ... --output-format stream-json`）。
fn parses_agy_stream_json_events() {
    // 正文增量：ACTIVE 與 DONE 兩種狀態都可能帶 text_delta，都要進正文
    assert_eq!(
        parse_agy_line(
            r#"{"event":"step_update","step_update":{"step_index":2,"state":"ACTIVE","step_type":"agent_response","text_delta":"好的"}}"#
        ),
        CliLine::Delta("好的".to_owned())
    );
    assert_eq!(
        parse_agy_line(
            r#"{"event":"step_update","step_update":{"step_index":2,"state":"DONE","step_type":"agent_response","text_delta":"\n"}}"#
        ),
        CliLine::Delta("\n".to_owned())
    );
    // 非 agent_response 的步驟不進正文
    assert_eq!(
        parse_agy_line(
            r#"{"event":"step_update","step_update":{"step_index":1,"state":"DONE","step_type":"checkpoint"}}"#
        ),
        CliLine::Other
    );
    assert_eq!(
        parse_agy_line(r#"{"event":"init","init":{"cwd":"/x"}}"#),
        CliLine::Other
    );
    // result 收尾：response 是全文重述，不可當增量，否則正文出現兩次
    // response 進 Done.text 當零增量 fallback：run_cli 只在完全沒收到增量時才用它
    assert_eq!(
        parse_agy_line(r#"{"event":"result","result":{"status":"SUCCESS","response":"好的\n"}}"#),
        CliLine::Done {
            text: "好的\n".to_owned(),
            is_error: false,
        }
    );
    // 非 SUCCESS 仍要失敗，不能靜默當成功
    assert!(matches!(
        parse_agy_line(r#"{"event":"result","result":{"status":"ERROR"}}"#),
        CliLine::Done { is_error: true, .. }
    ));
    assert!(matches!(
        parse_agy_line(r#"{"event":"result","result":{}}"#),
        CliLine::Done { is_error: true, .. }
    ));
    assert_eq!(parse_agy_line("不是 JSON"), CliLine::Other);

    // 整段實跑逐行餵進去：增量串起來要跟 result.response 一字不差
    // （少一段＝正文被吞，多一段＝result.response 被當增量重播）
    let captured = [
        r#"{"event":"init","conversation_id":"c1","init":{"cwd":"/x"}}"#,
        r#"{"event":"step_update","step_update":{"step_index":0,"state":"DONE","step_type":"user_input"}}"#,
        r#"{"event":"step_update","step_update":{"step_index":1,"state":"DONE","step_type":"checkpoint"}}"#,
        r#"{"event":"step_update","step_update":{"step_index":2,"state":"ACTIVE","step_type":"agent_response","text_delta":"好的"}}"#,
        r#"{"event":"step_update","step_update":{"step_index":2,"state":"DONE","step_type":"agent_response","text_delta":"\n","usage":{"input_tokens":14827}}}"#,
        r#"{"event":"result","result":{"status":"SUCCESS","response":"好的\n","usage":{"input_tokens":14827,"output_tokens":282,"cache_read_tokens":0}}}"#,
    ];
    let body: String = captured
        .iter()
        .filter_map(|line| match parse_agy_line(line) {
            CliLine::Delta(text) => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(body, "好的\n");
}

#[test]
/// agy 1.1.8 起才有 usage；欄位在 result.usage，判別鍵是 event 不是 type。
fn parses_agy_usage_from_result_event() {
    let usage = parse_agy_usage(
        r#"{"event":"result","result":{"status":"SUCCESS","usage":{"input_tokens":14827,"output_tokens":282,"thinking_tokens":281,"cache_read_tokens":1024,"total_tokens":15109}}}"#,
    )
    .expect("result 事件要解得出用量");
    assert_eq!(usage.prompt_tokens, 14_827); // input 已含 cache_read，不加總
    assert_eq!(usage.cached_tokens, Some(1_024));
    assert_eq!(usage.created_tokens, None); // agy 不回報寫入數，沒回報不是 0
    assert_eq!(usage.output_tokens, 282);
    assert_eq!(usage.cost_usd, None); // agy 不回報金額
                                      // step_update 也帶 usage，但只有 result 是全回合總計
    assert!(parse_agy_usage(
        r#"{"event":"step_update","step_update":{"step_type":"agent_response","usage":{"input_tokens":1}}}"#
    )
    .is_none());
    assert!(parse_agy_usage(r#"{"event":"result","result":{"status":"SUCCESS"}}"#).is_none());
}

/// 官方契約的 input 已含 cache read；舊版另一種加總形狀仍相容。
/// total 單獨漂移時不得抹掉明確回報的 cache_read_tokens。
#[test]
fn agy_usage_picks_contract_by_total_and_bails_out_when_neither_fits() {
    let build = |input: u64, cached: u64, output: u64, total: u64| {
        parse_agy_usage(&format!(
            r#"{{"event":"result","result":{{"usage":{{"input_tokens":{input},"cache_read_tokens":{cached},"output_tokens":{output},"total_tokens":{total}}}}}}}"#
        ))
        .expect("有 usage 就要解得出來")
    };
    // total = input + output → input 已含 cache_read
    let included = build(1_000, 400, 100, 1_100);
    assert_eq!(included.prompt_tokens, 1_000);
    assert_eq!(included.cached_tokens, Some(400));
    // total = input + cache_read + output → input 未含，要加回
    let excluded = build(1_000, 400, 100, 1_500);
    assert_eq!(excluded.prompt_tokens, 1_400);
    assert_eq!(excluded.cached_tokens, Some(400));
    // total 後端聚合值若漂移，input/cache 本身仍是明確計數
    let drifted_total = build(1_000, 400, 100, 9_999);
    assert_eq!(drifted_total.prompt_tokens, 1_000);
    assert_eq!(drifted_total.cached_tokens, Some(400));
    // 讀取數大於輸入數又不符第二式：一樣不猜
    assert_eq!(build(100, 900, 50, 150).cached_tokens, None);
}

#[test]
fn agy_usage_evidence_keeps_counts_and_conversation_without_content() {
    let line = r#"{"event":"result","result":{"conversation_id":"c-result","status":"SUCCESS","response":"private","duration_seconds":6.5,"num_turns":2,"usage":{"input_tokens":1000,"output_tokens":100,"thinking_tokens":80,"cache_read_tokens":400,"total_tokens":1100}}}"#;
    let evidence = agy_usage_evidence(line, Some("c-init")).expect("result usage");
    assert_eq!(
        evidence["agy_conversation_id"],
        serde_json::json!("c-result")
    );
    assert_eq!(evidence["agy_cache_read_tokens"], serde_json::json!(400));
    assert_eq!(evidence["agy_thinking_tokens"], serde_json::json!(80));
    assert_eq!(evidence["agy_num_turns"], serde_json::json!(2));
    assert!(!evidence.values().any(|value| value == "private"));

    let init = r#"{"event":"init","conversation_id":"c-init","init":{"cwd":"/x"}}"#;
    assert_eq!(agy_conversation_id(init).as_deref(), Some("c-init"));
}

#[test]
fn agy_resume_usage_subtracts_conversation_counters() {
    let base = super::agy_usage_counters(
        r#"{"event":"result","result":{"usage":{"input_tokens":20103,"output_tokens":1493,"cache_read_tokens":0}}}"#,
    )
    .unwrap();
    let current = super::agy_usage_counters(
        r#"{"event":"result","result":{"usage":{"input_tokens":25726,"output_tokens":3102,"cache_read_tokens":16343}}}"#,
    )
    .unwrap();
    let usage = super::parse_agy_usage_delta(current, base).unwrap();
    assert_eq!(usage.prompt_tokens, 21_966); // 5623 新輸入 + 16343 cache read
    assert_eq!(usage.cached_tokens, Some(16_343));
    assert_eq!(usage.output_tokens, 1_609);
    assert!((usage.hit_rate().unwrap() - 74.4).abs() < 0.1);
}

#[test]
fn parses_grok_streaming_json_lines() {
    assert_eq!(
        parse_grok_line(r#"{"type":"text","data":"測試"}"#),
        CliLine::Delta("測試".to_owned())
    );
    assert_eq!(
        parse_grok_line(r#"{"type":"thought","data":"推理"}"#),
        CliLine::Thinking("推理".to_owned())
    );
    assert_eq!(parse_grok_line(r#"{"type":"unknown"}"#), CliLine::Other);
    assert_eq!(parse_grok_line("not json"), CliLine::Other);
    assert_eq!(
        parse_grok_line(r#"{"type":"end","stopReason":"EndTurn"}"#),
        CliLine::Done {
            text: String::new(),
            is_error: false
        }
    );
    // 文件列的每個值都明確分類；大小寫與底線不拘
    for (reason, finish) in [
        ("MaxTokens", "max_tokens"),
        ("max_tokens", "max_tokens"),
        ("MaxTurnRequests", "max_turn_requests"),
        ("max_turn_requests", "max_turn_requests"),
    ] {
        let line = format!(r#"{{"type":"end","stopReason":"{reason}"}}"#);
        assert_eq!(
            parse_grok_line(&line),
            CliLine::Done {
                text: format!("AI_INCOMPLETE_RESPONSE: finish_reason={finish}"),
                is_error: true
            }
        );
    }
    for reason in ["Cancelled", "cancelled", "refusal", "Refusal", "pause_turn"] {
        let line = format!(r#"{{"type":"end","stopReason":"{reason}"}}"#);
        assert!(matches!(
            parse_grok_line(&line),
            CliLine::Done { is_error: true, text } if text.contains(reason)
        ));
    }
    // 缺欄不當完成
    assert!(matches!(
        parse_grok_line(r#"{"type":"end"}"#),
        CliLine::Done { is_error: true, .. }
    ));
    assert_eq!(
        parse_grok_line(r#"{"type":"end","stopReason":"end_turn"}"#),
        CliLine::Done {
            text: String::new(),
            is_error: false
        }
    );
    assert_eq!(
        parse_grok_line(r#"{"type":"error","data":"quota exceeded"}"#),
        CliLine::Done {
            text: "quota exceeded".to_owned(),
            is_error: true
        }
    );
    assert_eq!(
        parse_grok_line(r#"{"type":"error"}"#),
        CliLine::Done {
            text: turn_failed("Grok"),
            is_error: true
        }
    );
}
