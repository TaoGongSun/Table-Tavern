#[allow(unused_imports)]
use super::super::arrivals::*;
#[allow(unused_imports)]
use super::super::assemble::*;
#[allow(unused_imports)]
use super::super::context::*;
#[allow(unused_imports)]
use super::super::messages::*;
#[allow(unused_imports)]
use super::super::response::*;
#[allow(unused_imports)]
use super::super::state_view::*;
#[allow(unused_imports)]
use super::super::test_support::{card, event, stall_server, worldbook_entry};
#[allow(unused_imports)]
use super::super::turns::*;
use super::*;
#[allow(unused_imports)]
use crate::data::{
    self, AppConfig, CharacterCard, DataResult, FieldKind, FieldRule, InjectLevel, Mechanism,
    StateNode, TableState, Tier, TranscriptEvent, TranscriptKind, Visibility, WorldbookEntry,
};
#[allow(unused_imports)]
use crate::mechanism;
#[allow(unused_imports)]
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn gm_tier_defaults_to_best_and_reads_preference() {
    let mut config = AppConfig::default();
    assert_eq!(gm_tier(&config), Tier::Best);
    config.preferences.insert(
        "gm_tier".to_owned(),
        serde_json::Value::String("fast".to_owned()),
    );
    assert_eq!(gm_tier(&config), Tier::Fast);
    // 亂值退回預設 best
    config.preferences.insert(
        "gm_tier".to_owned(),
        serde_json::Value::String("impossible".to_owned()),
    );
    assert_eq!(gm_tier(&config), Tier::Best);
}

#[test]
fn refactor_expand_tier_falls_back_only_on_api_without_balanced_model() {
    let mut config = AppConfig::default();
    // API 模式未設 balanced 模型 → 退 GM 檔（預設 best）
    assert_eq!(refactor_expand_tier(&config, "api"), Tier::Best);
    // CLI 模式一律 balanced（CLI 有內建檔位對應，不用退）
    assert_eq!(refactor_expand_tier(&config, "claude"), Tier::Balanced);
    // API 模式設了 balanced 模型 → balanced
    config
        .tier_models
        .insert("balanced".to_owned(), "vendor/mid-model".to_owned());
    assert_eq!(refactor_expand_tier(&config, "api"), Tier::Balanced);
}

#[test]
fn tier_model_matches_what_actually_gets_sent() {
    let mut config = AppConfig::default();
    // claude 未覆寫：內建別名，永遠有值
    let fast = tier_model(&config, "claude", Tier::Fast);
    assert_eq!(fast.model.as_deref(), Some("haiku"));
    assert_eq!(fast.effective_tier, "fast");
    assert!(fast.effort.is_none());
    // claude 有覆寫：顯示覆寫後的實際 id（同樣是「低」檔，兩台機器送的不一樣）
    config
        .tier_models
        .insert("claude:fast".to_owned(), "claude-haiku-4-5".to_owned());
    assert_eq!(
        tier_model(&config, "claude", Tier::Fast).model.as_deref(),
        Some("claude-haiku-4-5")
    );
    // codex 未覆寫：走 CLI 預設模型（model=None），檔位落在 reasoning effort
    let codex = tier_model(&config, "codex", Tier::Best);
    assert_eq!(codex.model, None);
    assert_eq!(codex.effort.as_deref(), Some("high"));
    // API 模式該檔沒設模型 → 照實反映會退到 GM 檔
    config
        .tier_models
        .insert("best".to_owned(), "vendor/big-model".to_owned());
    let api_fast = tier_model(&config, "api", Tier::Fast);
    assert_eq!(api_fast.effective_tier, "best");
    assert_eq!(api_fast.model.as_deref(), Some("vendor/big-model"));
}

#[test]
fn resolve_model_reads_config() {
    let mut config = AppConfig::default();
    assert!(resolve_model(Tier::Best, &config).is_err());

    config
        .tier_models
        .insert("best".to_owned(), "vendor/big-model".to_owned());
    config
        .tier_models
        .insert("balanced".to_owned(), "vendor/mid-model".to_owned());
    config
        .tier_models
        .insert("fast".to_owned(), "vendor/small-model".to_owned());
    assert_eq!(
        resolve_model(Tier::Best, &config).unwrap(),
        "vendor/big-model"
    );
    assert_eq!(
        resolve_model(Tier::Balanced, &config).unwrap(),
        "vendor/mid-model"
    );
    assert_eq!(
        resolve_model(Tier::Fast, &config).unwrap(),
        "vendor/small-model"
    );
}

#[test]
fn base_url_defaults_and_trims_trailing_slash() {
    let mut config = AppConfig::default();
    assert_eq!(base_url(&config), DEFAULT_BASE_URL);
    config.preferences.insert(
        "base_url".to_owned(),
        serde_json::Value::String("http://localhost:11434/v1/".to_owned()),
    );
    assert_eq!(base_url(&config), "http://localhost:11434/v1");
}

#[test]
fn sse_parser_handles_split_chunks_comments_and_multibyte_boundaries() {
    let mut parser = SseParser::default();
    assert!(parser.push(b": OPENROUTER PROCESSING\n\n").is_empty());

    // 一則 payload 被切成兩塊，且切點落在多位元組字元中間
    let payload = r#"data: {"choices":[{"delta":{"content":"你好"}}]}"#;
    let bytes = payload.as_bytes();
    let split = payload.find("你").unwrap() + 1; // 「你」的第 2 個位元組處
    let mut collected = parser.push(&bytes[..split]);
    assert!(collected.is_empty());
    collected.extend(parser.push(&bytes[split..]));
    collected.extend(parser.push(b"\ndata: [DONE]\n"));
    assert_eq!(collected.len(), 2);
    assert_eq!(extract_delta(&collected[0]).unwrap(), "你好");
    assert_eq!(collected[1], "[DONE]");
}

#[tokio::test]
async fn stream_chat_streams_deltas_from_mock_server_and_requires_key_for_openrouter() {
    use std::io::{Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = [0u8; 4096];
        let _ = socket.read(&mut request);
        let body = concat!(
            ": OPENROUTER PROCESSING\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"你\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"好\"}}]}\n\n",
            "data: [DONE]\n\n",
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(response.as_bytes()).unwrap();
    });

    // 預設 OpenRouter endpoint 且沒 key：呼叫前就擋下
    let mut config = AppConfig::default();
    let messages = [message("user", "嗨".to_owned())];
    let error = stream_chat(
        &config,
        "test/model",
        &messages,
        None,
        None,
        crate::usage::log::PromptShape::Oneshot,
        crate::transport::RunawayPolicy::Off,
        |_| {},
    )
    .await
    .unwrap_err()
    .to_string();
    assert_eq!(
        error,
        crate::ui_msg::UiMsg::OpenrouterApiKeyMissing.to_string()
    );

    // 自訂 base URL（無 key）：走 mock server，增量與全文一致
    config.preferences.insert(
        "base_url".to_owned(),
        serde_json::Value::String(format!("http://{address}")),
    );
    let mut deltas = Vec::new();
    let full = stream_chat(
        &config,
        "test/model",
        &messages,
        None,
        None,
        crate::usage::log::PromptShape::Oneshot,
        crate::transport::RunawayPolicy::Off,
        |delta| {
            deltas.push(delta.to_owned());
        },
    )
    .await
    .unwrap();
    assert_eq!(full.text, "你好");
    assert_eq!(full.truncated, None);
    assert_eq!(deltas, ["你", "好"]);
}

#[test]
fn smart_free_body_sends_one_model_without_provider_specific_cache_fields() {
    let messages = [
        message("system", "stable prefix".to_owned()),
        message("user", "hello".to_owned()),
    ];
    let body = smart_request_body("anthropic/first:free", &messages);
    assert_eq!(body["model"], "anthropic/first:free");
    assert!(body.get("models").is_none());
    assert_eq!(body["messages"][0]["content"], "stable prefix");
    assert!(!body.to_string().contains("cache_control"));
}

#[test]
fn smart_free_extracts_the_model_that_actually_answered() {
    assert_eq!(
        extract_response_model(
            r#"{"model":"beta/fallback:free","choices":[{"delta":{"content":"好"}}]}"#,
        )
        .as_deref(),
        Some("beta/fallback:free")
    );
    assert_eq!(extract_response_model(r#"{"choices":[]}"#), None);
}

/// 收工判定的優先序（stream-failure-visible）：實測 2026-08-21 免費 DeepSeek
/// 「思考完但零內容」時串流是正常走完 [DONE] 的，靠 content 判不出失敗。
#[test]
fn stream_outcome_ranks_failures_by_priority() {
    // 供應商中途 error：原話原樣拋，不加碼——交給 ai-error.ts 既有的額度正則分流
    let mut outcome = StreamOutcome::default();
    outcome.absorb(
        r#"{"error":{"code":429,"message":"Rate limit exceeded"},"choices":[{"delta":{"content":""},"finish_reason":"error"}]}"#,
    );
    assert_eq!(
        outcome.failure("", "test/model").unwrap(),
        "Rate limit exceeded"
    );

    // error.message 不是字串：整包序列化，不靜默吞掉
    let mut outcome = StreamOutcome::default();
    outcome.absorb(r#"{"error":{"code":500}}"#);
    assert!(outcome.failure("", "test/model").unwrap().contains("500"));

    // error.message 是空字串：等同缺失，一樣回退整包——Err("") 在前端等於什麼都沒顯示
    let mut outcome = StreamOutcome::default();
    outcome.absorb(r#"{"error":{"code":500,"message":""}}"#);
    let failure = outcome.failure("", "test/model").unwrap();
    assert!(
        !failure.trim().is_empty() && failure.contains("500"),
        "{failure}"
    );

    // 有正文＋[DONE]＋供應商沒給 finish_reason＝成功：共用 OpenAI-compatible 路徑
    // 不強迫所有供應商都回收尾原因，[DONE] 本身就是完成訊號
    let mut outcome = StreamOutcome::default();
    outcome.absorb(r#"{"choices":[{"delta":{"content":"旁白"}}]}"#);
    outcome.saw_done = true;
    assert_eq!(outcome.failure("旁白", "test/model"), None);

    // content_filter 有自己的碼（玩家的下一步是換說法，不是重試）
    let mut outcome = StreamOutcome::default();
    outcome.absorb(r#"{"choices":[{"finish_reason":"content_filter"}]}"#);
    outcome.saw_done = true;
    assert!(outcome
        .failure("", "test/model")
        .unwrap()
        .starts_with("AI_CONTENT_FILTERED:"));
    assert_eq!(outcome.failure("半截旁白", "test/model"), None);
    assert_eq!(
        outcome.truncation("半截旁白"),
        Some("content_filter".to_owned())
    );

    // length 又零正文：歸 INCOMPLETE 不歸 EMPTY——原因是被截斷，不是模型沒話說
    let mut outcome = StreamOutcome::default();
    outcome.absorb(r#"{"choices":[{"finish_reason":"length"}]}"#);
    outcome.absorb(r#"{"usage":{"completion_tokens_details":{"reasoning_tokens":4437}}}"#);
    outcome.saw_done = true;
    let failure = outcome.failure("", "test/model").unwrap();
    assert!(failure.starts_with("AI_INCOMPLETE_RESPONSE:"), "{failure}");
    assert!(failure.contains("reasoning_tokens=4437"), "{failure}");

    // length 但正文非空：保留正文，另帶截斷原因給上層標記
    let mut outcome = StreamOutcome::default();
    outcome.absorb(r#"{"choices":[{"finish_reason":"length"}]}"#);
    outcome.saw_done = true;
    assert_eq!(outcome.failure("半截旁白", "test/model"), None);
    assert_eq!(outcome.truncation("半截旁白"), Some("length".to_owned()));

    // 沒收尾原因又沒見到 [DONE]＝串流被截斷
    let outcome = StreamOutcome::default();
    assert!(outcome
        .failure("有字", "test/model")
        .unwrap()
        .starts_with("AI_INCOMPLETE_RESPONSE:"));

    // 正常收尾但正文只有空白：這就是實測那兩次的形狀
    let mut outcome = StreamOutcome::default();
    outcome.absorb(r#"{"choices":[{"finish_reason":"stop"}]}"#);
    outcome.saw_done = true;
    assert!(outcome
        .failure(" \n ", "test/model")
        .unwrap()
        .starts_with("AI_EMPTY_RESPONSE:"));

    // 正常收尾且有正文＝成功
    let mut outcome = StreamOutcome::default();
    outcome.absorb(r#"{"choices":[{"finish_reason":"stop"}]}"#);
    outcome.saw_done = true;
    assert_eq!(outcome.failure("旁白", "test/model"), None);
}

/// 非 2xx 一律掛開頭碼給前端分流：碼取自真正的 HTTP 狀態，
/// 不受 body 裡那些上游轉包的數字影響（今天實測的 503 body 就長這樣）
#[test]
fn http_error_prefixes_real_status_not_body_digits() {
    let real =
        r#"{"error":{"message":"openai_error","type":"bad_response_status_code"},"id":157975}"#;
    let text = http_error(reqwest::StatusCode::SERVICE_UNAVAILABLE, real);
    // 前綴留在起首，後面是不帶語言的 status=… body=…，玩家換語系也不會冒中文
    assert_eq!(
        text,
        format!("AI_HTTP_STATUS_503: status=503 Service Unavailable body={real}")
    );
    assert!(text.contains("bad_response_status_code"), "{text}");

    // body 自稱 429，狀態是 503：碼必須跟著狀態走
    let lying = r#"{"error":{"message":"upstream said 429 rate limit"}}"#;
    let text = http_error(reqwest::StatusCode::SERVICE_UNAVAILABLE, lying);
    assert!(text.starts_with("AI_HTTP_STATUS_503: "), "{text}");

    // 沒超過上限就不留截斷字樣（玩家複製到的是完整原文）
    let short = "毒".repeat(2000);
    let text = http_error(reqwest::StatusCode::BAD_GATEWAY, &short);
    assert_eq!(text.matches('毒').count(), 2000);
    assert!(!text.contains("[truncated]"), "{text}");

    // 超長才截，且一定標記出來：看似完整其實殘缺的 JSON 比明說截斷更難查
    let long = "毒".repeat(2500);
    let text = http_error(reqwest::StatusCode::BAD_GATEWAY, &long);
    assert!(text.starts_with("AI_HTTP_STATUS_502: "), "{text}");
    assert_eq!(text.matches('毒').count(), 2000);
    assert!(text.ends_with("…[truncated]"), "{text}");
}

/// 增量塊的 finish_reason 是 null，真正的收尾原因在最後一塊：取最後一則有值的
#[test]
fn stream_outcome_absorbs_last_finish_reason_and_ignores_nulls() {
    let mut outcome = StreamOutcome::default();
    outcome.absorb(r#"{"choices":[{"delta":{"content":"嗨"},"finish_reason":null}]}"#);
    assert_eq!(outcome.finish_reason, None);
    outcome.absorb(r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#);
    assert_eq!(outcome.finish_reason.as_deref(), Some("stop"));
    // 壞掉的 JSON 不該讓整條串流爆掉
    outcome.absorb("{不是 JSON");
    assert_eq!(outcome.finish_reason.as_deref(), Some("stop"));
}

/// 端到端：串流正常走完 [DONE] 但一個字都沒有，現在回 Err 而不是 Ok("")
#[tokio::test]
async fn stream_chat_fails_when_stream_completes_with_no_content() {
    use std::io::{Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = [0u8; 4096];
        let _ = socket.read(&mut request);
        let body = concat!(
            ": OPENROUTER PROCESSING\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"\"},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n",
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(response.as_bytes()).unwrap();
    });

    let mut config = AppConfig::default();
    config.preferences.insert(
        "base_url".to_owned(),
        serde_json::Value::String(format!("http://{address}")),
    );
    let messages = [message("user", "嗨".to_owned())];
    let error = stream_chat(
        &config,
        "test/model",
        &messages,
        None,
        None,
        crate::usage::log::PromptShape::Oneshot,
        crate::transport::RunawayPolicy::Off,
        |_| {},
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(error.starts_with("AI_EMPTY_RESPONSE:"), "{error}");
}

#[tokio::test]
async fn generate_image_returns_data_url_from_b64_json() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = [0u8; 4096];
        let _ = socket.read(&mut request);
        let body = r#"{"data":[{"b64_json":"cG5n"}]}"#;
        let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        socket.write_all(response.as_bytes()).unwrap();
    });
    let mut config = AppConfig::default();
    config
        .api_keys
        .insert("openrouter".to_owned(), "key".to_owned());
    config.preferences.insert(
        "base_url".to_owned(),
        serde_json::Value::String(format!("http://{address}")),
    );
    assert_eq!(
        generate_image(&config, "畫一位角色").await.unwrap(),
        "data:image/png;base64,cG5n"
    );
}

#[tokio::test]
async fn generate_image_rejects_empty_data() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = [0u8; 4096];
        let _ = socket.read(&mut request);
        let body = r#"{"data":[]}"#;
        let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        socket.write_all(response.as_bytes()).unwrap();
    });
    let mut config = AppConfig::default();
    config
        .api_keys
        .insert("openrouter".to_owned(), "key".to_owned());
    config.preferences.insert(
        "base_url".to_owned(),
        serde_json::Value::String(format!("http://{address}")),
    );
    assert_eq!(
        generate_image(&config, "畫一位角色").await.unwrap_err(),
        crate::ui_msg::UiMsg::ImageMissingInReply.to_string()
    );
}

/// 請求本體維持素樸：usage accounting 參數已被 OpenRouter 官方廢止（帶了無效，
/// 嚴格端點還會拒絕），一個多餘的鍵都不能有。
#[test]
fn chat_request_body_stays_bytewise_identical_for_plain_models() {
    let messages = [message("user", "嗨".to_owned())];
    let plain = chat_request_body("test/model", &messages);
    assert_eq!(
        plain,
        serde_json::json!({
            "model": "test/model",
            "messages": [{"role": "user", "content": "嗨"}],
            "stream": true,
        })
    );
    assert!(plain.get("usage").is_none());
}

/// Claude 顯式斷點（prompt-cache-optimization B）：anthropic/ 系模型 content 轉 multipart，
/// 斷點恰好兩個——system 與最後一則 assistant；其他模型維持純字串 content。
#[test]
fn anthropic_models_get_multipart_content_with_two_breakpoints() {
    let messages = [
        message("system", "設定".to_owned()),
        message("assistant", "旁白一".to_owned()),
        message("user", "玩家：嗨".to_owned()),
        message("assistant", "旁白二".to_owned()),
        message("user", "動態塊".to_owned()),
    ];
    let body = chat_request_body("anthropic/claude-sonnet-4.5", &messages);
    let out = body["messages"].as_array().unwrap();
    assert_eq!(out.len(), 5);
    // multipart：每則 content 是單一 text 分段，role 與文字照舊
    assert_eq!(out[2]["role"], "user");
    assert_eq!(out[2]["content"][0]["type"], "text");
    assert_eq!(out[2]["content"][0]["text"], "玩家：嗨");
    // 斷點恰好兩個：system（index 0）與最後一則 assistant（index 3）
    let marked: Vec<usize> = out
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry["content"][0].get("cache_control").is_some())
        .map(|(index, _)| index)
        .collect();
    assert_eq!(marked, [0, 3]);
    assert_eq!(
        out[0]["content"][0]["cache_control"],
        serde_json::json!({ "type": "ephemeral" })
    );

    // 非 anthropic 模型：content 維持純字串（形狀逐位元不變由上一條測試保證）
    let plain = chat_request_body("test/model", &messages);
    assert!(plain["messages"][0]["content"].is_string());

    // 開桌第一輪沒有 assistant：只標 system，不出錯
    let fresh = [
        message("system", "設定".to_owned()),
        message("user", "嗨".to_owned()),
    ];
    let fresh_body = chat_request_body("anthropic/claude-haiku", &fresh);
    let fresh_out = fresh_body["messages"].as_array().unwrap();
    assert!(fresh_out[0]["content"][0].get("cache_control").is_some());
    assert!(fresh_out[1]["content"][0].get("cache_control").is_none());
}

#[test]
fn extract_usage_reads_final_chunk_and_ignores_delta_chunks() {
    // OpenRouter 尾塊：prompt_tokens_details.cached_tokens 是快取命中數
    let usage = extract_usage(
        r#"{"choices":[],"usage":{"prompt_tokens":194,"prompt_tokens_details":{"cached_tokens":150,"audio_tokens":0},"completion_tokens":2,"total_tokens":196}}"#,
    )
    .unwrap();
    assert_eq!(
        usage,
        PromptCacheUsage {
            created_1h_tokens: None,
            prompt_tokens: 194,
            cached_tokens: Some(150),
            created_tokens: None, // 這則沒有 cache_write_tokens：沒回報，不是 0
            output_tokens: 2,
            cost_usd: None, // 金額只有 claude CLI 直接回報
        }
    );

    // OpenRouter 也回寫入數時照收
    let with_write = extract_usage(
        r#"{"usage":{"prompt_tokens":300,"prompt_tokens_details":{"cached_tokens":100,"cache_write_tokens":200},"completion_tokens":5}}"#,
    )
    .unwrap();
    assert_eq!(
        (with_write.cached_tokens, with_write.created_tokens),
        (Some(100), Some(200))
    );

    // 混合 schema（相容層改版或雙格式轉送都可能同時吐出 normalized 與 upstream-native
    // 欄位）：讀、寫各自挑第一個有值的來源，寫入數不可遮蔽掉另一組的讀取數
    let mixed = extract_usage(
        r#"{"usage":{"prompt_tokens":100,"prompt_tokens_details":{"cache_write_tokens":20},"prompt_cache_hit_tokens":80,"completion_tokens":1}}"#,
    )
    .unwrap();
    assert_eq!(
        (mixed.cached_tokens, mixed.created_tokens),
        (Some(80), Some(20))
    );

    // 兩組讀取欄位同時存在＝第一順位（OpenRouter）勝出，不做衝突偵測
    let both_reads = extract_usage(
        r#"{"usage":{"prompt_tokens":100,"prompt_tokens_details":{"cached_tokens":70},"prompt_cache_hit_tokens":50,"completion_tokens":1}}"#,
    )
    .unwrap();
    assert_eq!(both_reads.cached_tokens, Some(70));

    // DeepSeek 原生欄位（中轉站照抄這組、不回 prompt_tokens_details）：
    // 讀錯這裡正是額度分頁對 API 路顯示假 0.0% 的根因，2026-08-21 對 tokenrouter 實測取證
    let deepseek = extract_usage(
        r#"{"usage":{"prompt_tokens":2495,"completion_tokens":16,"prompt_cache_hit_tokens":0,"prompt_cache_miss_tokens":2495}}"#,
    )
    .unwrap();
    assert_eq!(deepseek.cached_tokens, Some(0)); // 量到了、這輪沒中
    assert!(deepseek.reported());
    assert_eq!(deepseek.hit_rate(), Some(0.0));

    // Anthropic 原生欄位直通
    let anthropic = extract_usage(
        r#"{"usage":{"prompt_tokens":900,"cache_read_input_tokens":800,"cache_creation_input_tokens":100,"completion_tokens":3}}"#,
    )
    .unwrap();
    assert_eq!(
        (anthropic.cached_tokens, anthropic.created_tokens),
        (Some(800), Some(100))
    );

    // 一組欄位都沒有＝這條路不回報：cached 為 None、命中率不存在，**不可退成 0**
    let without_details =
        extract_usage(r#"{"usage":{"prompt_tokens":10,"completion_tokens":1}}"#).unwrap();
    assert_eq!(without_details.cached_tokens, None);
    assert!(!without_details.reported());
    assert_eq!(without_details.hit_rate(), None);

    // 增量塊：usage 為 null 或不存在，一律回 None
    assert_eq!(
        extract_usage(r#"{"choices":[{"delta":{"content":"嗨"}}],"usage":null}"#),
        None
    );
    assert_eq!(
        extract_usage(r#"{"choices":[{"delta":{"content":"嗨"}}]}"#),
        None
    );
    assert_eq!(extract_usage("not json"), None);
}

/// 尾端 usage 塊混在串流裡：增量文字照常回傳，usage 塊不產生任何 delta
#[tokio::test]
async fn stream_chat_passes_usage_chunk_through_without_breaking_deltas() {
    use std::io::{Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = [0u8; 4096];
        let _ = socket.read(&mut request);
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"你\"}}],\"usage\":null}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"好\"}}],\"usage\":null}\n\n",
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":20,\"prompt_tokens_details\":{\"cached_tokens\":12}}}\n\n",
            "data: [DONE]\n\n",
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(response.as_bytes()).unwrap();
    });

    let mut config = AppConfig::default();
    config.preferences.insert(
        "base_url".to_owned(),
        serde_json::Value::String(format!("http://{address}")),
    );
    let messages = [message("user", "嗨".to_owned())];
    let log_path =
        std::env::temp_dir().join(format!("tt-prompt-cache-test-{}.jsonl", std::process::id()));
    let _ = std::fs::remove_file(&log_path);
    let mut deltas = Vec::new();
    let full = stream_chat(
        &config,
        "test/model",
        &messages,
        Some(&log_path),
        Some("w1"),
        crate::usage::log::PromptShape::Turn {
            roster: 3,
            solo: false,
        },
        crate::transport::RunawayPolicy::Off,
        |delta| {
            deltas.push(delta.to_owned());
        },
    )
    .await
    .unwrap();
    assert_eq!(full.text, "你好");
    assert_eq!(full.truncated, None);
    assert_eq!(deltas, ["你", "好"]);

    // usage 落檔：一行 JSONL 含時間戳、模型、token 數與命中率（12/20 = 60%）
    let logged = std::fs::read_to_string(&log_path).unwrap();
    assert_eq!(logged.lines().count(), 1);
    let record: serde_json::Value = serde_json::from_str(logged.trim()).unwrap();
    assert_eq!(record["transport"], "api");
    assert_eq!(record["model"], "test/model");
    assert_eq!(record["prompt_tokens"], 20);
    assert_eq!(record["cached_tokens"], 12);
    assert_eq!(record["hit_rate"], 60.0);
    // 本案的核心：無狀態的 api 路徑也要說出真正的快取結果，不再一律「單發」
    assert_eq!(record["mode"], "shared");
    assert_eq!(record["cache"], "hit");
    assert_eq!(record["roster_size"], 3);
    let _ = std::fs::remove_file(&log_path);
}

#[test]
fn extract_delta_ignores_non_delta_payloads() {
    assert_eq!(extract_delta(r#"{"choices":[]}"#), None);
    assert_eq!(extract_delta(r#"{"usage":{"total_tokens":9}}"#), None);
    assert_eq!(
        extract_delta(r#"{"choices":[{"delta":{"content":""}}]}"#),
        None
    );
    assert_eq!(
        extract_delta(r#"{"choices":[{"delta":{"content":"嗨"}}]}"#).unwrap(),
        "嗨"
    );
}

/// 範圍 4：400 的 error 物件說容量爆掉才在最前面加 AI_CONTEXT_TOO_LONG；5xx 包著同一段不加
#[test]
fn http_error_marks_context_overflow_only_on_4xx_error_object() {
    let body = r#"{"error":{"message":"This endpoint's maximum context length is 131072 tokens.","code":400}}"#;
    let text = http_error(reqwest::StatusCode::BAD_REQUEST, body);
    assert!(
        text.starts_with("AI_CONTEXT_TOO_LONG: AI_HTTP_STATUS_400: "),
        "{text}"
    );
    let wrapped = http_error(reqwest::StatusCode::BAD_GATEWAY, body);
    assert!(wrapped.starts_with("AI_HTTP_STATUS_502: "), "{wrapped}");
    let other = http_error(
        reqwest::StatusCode::BAD_REQUEST,
        r#"{"error":{"message":"bad model"}}"#,
    );
    assert!(other.starts_with("AI_HTTP_STATUS_400: "), "{other}");
}

/// `/images` 與 `/key` 的本機假伺服器：依請求路徑回不同內容；`/key` 可延遲模擬逾時。
/// 回傳 base URL 與「`/key` 被打了幾次」的計數。
fn serve_image_and_key(
    image_status: &'static str,
    key_status: &'static str,
    key_body: &'static str,
    key_delay: std::time::Duration,
) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use std::io::{Read, Write};
    use std::sync::atomic::Ordering;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let key_hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let hits = key_hits.clone();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else { break };
            let hits = hits.clone();
            std::thread::spawn(move || {
                let mut request = [0u8; 4096];
                let read = socket.read(&mut request).unwrap_or(0);
                let request = String::from_utf8_lossy(&request[..read]).to_string();
                let (status, body) = if request.starts_with("GET /key") {
                    hits.fetch_add(1, Ordering::SeqCst);
                    std::thread::sleep(key_delay);
                    (key_status, key_body)
                } else {
                    (
                        image_status,
                        r#"{"error":{"code":402,"message":"Insufficient credits"}}"#,
                    )
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes());
            });
        }
    });
    (format!("http://{address}"), key_hits)
}

fn image_config(base: &str) -> AppConfig {
    let mut config = AppConfig::default();
    config
        .api_keys
        .insert("openrouter".to_owned(), "key".to_owned());
    config.preferences.insert(
        "base_url".to_owned(),
        serde_json::Value::String(base.to_owned()),
    );
    config
}

const FREE_KEY: &str = r#"{"data":{"is_free_tier":true,"limit":null}}"#;

#[tokio::test]
async fn generate_image_marks_free_key_on_402_and_403() {
    for status in ["402 Payment Required", "403 Forbidden"] {
        let (base, _) = serve_image_and_key(status, "200 OK", FREE_KEY, std::time::Duration::ZERO);
        let error = generate_image(&image_config(&base), "畫")
            .await
            .unwrap_err();
        let code = &status[..3];
        assert!(
            error.starts_with(&format!("AI_IMAGE_FREE_KEY: AI_HTTP_STATUS_{code}: ")),
            "{error}"
        );
    }
}

/// 認不準就保留原錯誤：任何一種「不確定」都不能被說成免費 key
#[tokio::test]
async fn generate_image_keeps_original_error_when_key_lookup_is_inconclusive() {
    let cases: [(&str, &str, std::time::Duration); 6] = [
        (
            "200 OK",
            r#"{"data":{"is_free_tier":false}}"#,
            std::time::Duration::ZERO,
        ),
        (
            "200 OK",
            r#"{"data":{"is_free_tier":"true"}}"#,
            std::time::Duration::ZERO,
        ),
        (
            "200 OK",
            r#"{"data":{"limit":null}}"#,
            std::time::Duration::ZERO,
        ),
        (
            "200 OK",
            r#"{"data":{"is_free_tier":tru"#,
            std::time::Duration::ZERO,
        ),
        (
            "500 Internal Server Error",
            FREE_KEY,
            std::time::Duration::ZERO,
        ),
        ("200 OK", FREE_KEY, std::time::Duration::from_millis(1500)),
    ];
    for (key_status, key_body, delay) in cases {
        let (base, hits) = serve_image_and_key("402 Payment Required", key_status, key_body, delay);
        let error = generate_image(&image_config(&base), "畫")
            .await
            .unwrap_err();
        assert!(
            error.starts_with("AI_HTTP_STATUS_402: "),
            "{key_body}: {error}"
        );
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn generate_image_skips_key_lookup_for_other_statuses() {
    let (base, hits) = serve_image_and_key(
        "500 Internal Server Error",
        "200 OK",
        FREE_KEY,
        std::time::Duration::ZERO,
    );
    let error = generate_image(&image_config(&base), "畫")
        .await
        .unwrap_err();
    assert!(error.starts_with("AI_HTTP_STATUS_500: "), "{error}");
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 0);
}

/// `/key` 標頭送出後 body 停住：短逾時要涵蓋讀 body，保留原錯誤、不卡到伺服器斷線
#[tokio::test]
async fn generate_image_key_lookup_times_out_when_body_stalls() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else { break };
            std::thread::spawn(move || {
                let mut request = [0u8; 4096];
                let read = socket.read(&mut request).unwrap_or(0);
                if String::from_utf8_lossy(&request[..read]).starts_with("GET /key") {
                    let _ = socket.write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 100\r\n\r\n{\"data\":",
                    );
                    let _ = socket.flush();
                    std::thread::sleep(std::time::Duration::from_secs(10));
                } else {
                    let body = r#"{"error":{"code":402}}"#;
                    let _ = socket.write_all(format!("HTTP/1.1 402 Payment Required\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
                }
            });
        }
    });
    let started = std::time::Instant::now();
    let error = generate_image(&image_config(&format!("http://{address}")), "畫")
        .await
        .unwrap_err();
    assert!(error.starts_with("AI_HTTP_STATUS_402: "), "{error}");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn key_tier_for_reads_free_and_paid() {
    for (body, expected) in [
        (FREE_KEY, Some(true)),
        (r#"{"data":{"is_free_tier":false}}"#, Some(false)),
    ] {
        let (base, hits) = serve_image_and_key("200 OK", "200 OK", body, std::time::Duration::ZERO);
        let (queried, tier) = key_tier_for(&image_config(&base), " sk-draft ").await;
        assert_eq!(
            (queried.as_str(), tier),
            (base.as_str(), expected),
            "{body}"
        );
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn key_tier_for_is_unknown_when_inconclusive() {
    let cases: [(&str, &str, std::time::Duration); 7] = [
        (
            "200 OK",
            r#"{"data":{"limit":null}}"#,
            std::time::Duration::ZERO,
        ),
        (
            "200 OK",
            r#"{"data":{"is_free_tier":null}}"#,
            std::time::Duration::ZERO,
        ),
        (
            "200 OK",
            r#"{"data":{"is_free_tier":"true"}}"#,
            std::time::Duration::ZERO,
        ),
        ("200 OK", r#"{"data":null}"#, std::time::Duration::ZERO),
        (
            "200 OK",
            r#"{"data":{"is_free_tier":tru"#,
            std::time::Duration::ZERO,
        ),
        ("404 Not Found", FREE_KEY, std::time::Duration::ZERO),
        ("200 OK", FREE_KEY, std::time::Duration::from_millis(1500)),
    ];
    for (status, body, delay) in cases {
        let (base, _) = serve_image_and_key("200 OK", status, body, delay);
        let (_, tier) = key_tier_for(&image_config(&base), "sk-draft").await;
        assert_eq!(tier, None, "{status} {body}");
    }
}

#[tokio::test]
async fn key_tier_for_is_unknown_when_connection_fails() {
    // 綁定後立刻放掉，這個 port 沒人聽
    let address = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let (_, tier) = key_tier_for(&image_config(&format!("http://{address}")), "sk-draft").await;
    assert_eq!(tier, None);
}

/// 標頭已回、body 停住：整段逾時要涵蓋讀 body
#[tokio::test]
async fn key_tier_for_times_out_when_body_stalls() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else { break };
            std::thread::spawn(move || {
                let mut request = [0u8; 4096];
                let _ = socket.read(&mut request);
                let _ = socket.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 100\r\n\r\n{\"data\":",
                );
                let _ = socket.flush();
                std::thread::sleep(std::time::Duration::from_secs(10));
            });
        }
    });
    let started = std::time::Instant::now();
    let (_, tier) = key_tier_for(&image_config(&format!("http://{address}")), "sk-draft").await;
    assert_eq!(tier, None);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn key_tier_for_skips_request_for_blank_key_and_normalizes_saved_base() {
    let (base, hits) = serve_image_and_key("200 OK", "200 OK", FREE_KEY, std::time::Duration::ZERO);
    let (queried, tier) = key_tier_for(&image_config(&format!("{base}/")), "   ").await;
    assert_eq!((queried.as_str(), tier), (base.as_str(), None));
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 0);
    let (queried, _) = key_tier_for(&AppConfig::default(), "").await;
    assert_eq!(queried, DEFAULT_BASE_URL);
}

fn stall_config(base: &str) -> AppConfig {
    let mut config = AppConfig::default();
    config.preferences.insert(
        "base_url".to_owned(),
        serde_json::Value::String(base.to_owned()),
    );
    config
}

fn window(first_ms: u64, after_ms: u64) -> StallWindow {
    StallWindow {
        first: std::time::Duration::from_millis(first_ms),
        after: std::time::Duration::from_millis(after_ms),
    }
}

const KEEPALIVE: &str = ": OPENROUTER PROCESSING\n\n";
const ROLE_ONLY: &str =
    "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"\"}}]}\n\n";
const DELTA: &str = "data: {\"choices\":[{\"delta\":{\"content\":\"字\"}}]}\n\n";
const REASONING: &str = "data: {\"choices\":[{\"delta\":{\"reasoning\":\"想\"}}]}\n\n";

async fn run_chat(base: &str, window: StallWindow) -> (DataResult<StreamChatResult>, u128) {
    let started = std::time::Instant::now();
    let messages = [message("user", "嗨".to_owned())];
    let result = stream_chat_windowed(
        &stall_config(base),
        "test/model",
        &messages,
        None,
        None,
        crate::usage::log::PromptShape::Oneshot,
        crate::transport::RunawayPolicy::Off,
        window,
        |_| {},
    )
    .await;
    (result, started.elapsed().as_millis())
}

/// 保活註解一直來也要逾時：deadline 不因註解續命。判定靠錯誤碼：若續命，串流會先自然結束而回別種錯誤。
/// （不量時間：reqwest::Client::new() 載憑證就可能花數秒。）
#[tokio::test]
async fn stall_keepalive_comments_do_not_extend_deadline() {
    let script = (0..40).map(|_| (50, KEEPALIVE.to_owned())).collect();
    let base = stall_server(script, 100);
    let (result, _) = run_chat(&base, window(400, 400)).await;
    let error = result.unwrap_err().to_string();
    assert!(error.starts_with("AI_STREAM_STALLED:"), "{error}");
}

/// role-only 塊不算進展，也不把首字窗切成進展窗
#[tokio::test]
async fn stall_role_only_chunk_is_not_progress() {
    let base = stall_server(vec![(0, ROLE_ONLY.to_owned())], 1500);
    let (result, _) = run_chat(&base, window(300, 5000)).await;
    assert!(result
        .unwrap_err()
        .to_string()
        .starts_with("AI_STREAM_STALLED:"));
}

/// 首字窗→進展窗：第一個進展之後改用較短的窗口
#[tokio::test]
async fn stall_window_switches_after_first_progress() {
    let base = stall_server(vec![(100, DELTA.to_owned())], 1500);
    let (result, _) = run_chat(&base, window(3000, 300)).await;
    assert!(result
        .unwrap_err()
        .to_string()
        .starts_with("AI_STREAM_STALLED:"));
}

/// 慢但持續有進展（含只有推理 delta）的串流不誤殺
#[tokio::test]
async fn stall_slow_progressing_stream_survives() {
    let mut script = vec![(100, REASONING.to_owned())];
    script.extend((0..5).map(|_| (150, DELTA.to_owned())));
    script.push((150, "data: [DONE]\n\n".to_owned()));
    let base = stall_server(script, 0);
    let (result, _) = run_chat(&base, window(1000, 400)).await;
    assert_eq!(result.unwrap().text, "字字字字字");
}

/// 已吐字、已收 usage 之後停滯：先記帳再回停滯錯誤，emitted_text 為真、分類 Timeout
#[tokio::test]
async fn stall_after_usage_still_records_and_reports_timeout() {
    let usage =
        "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2}}\n\n";
    let base = stall_server(vec![(0, DELTA.to_owned()), (0, usage.to_owned())], 4000);
    let log_path =
        std::env::temp_dir().join(format!("tt-stall-usage-{}.jsonl", std::process::id()));
    let _ = std::fs::remove_file(&log_path);
    let messages = [message("user", "嗨".to_owned())];
    let failure = stream_chat_models_windowed(
        &stall_config(&base),
        "test/model",
        &messages,
        Some(&log_path),
        Some("w1"),
        crate::usage::log::PromptShape::Oneshot,
        crate::transport::RunawayPolicy::Off,
        window(1000, 300),
        |_| {},
    )
    .await
    .unwrap_err();
    assert_eq!(failure.stage, super::super::FailureStage::Timeout);
    assert!(failure.emitted_text);
    assert!(failure.display.starts_with("AI_STREAM_STALLED:"));
    let logged = std::fs::read_to_string(&log_path).unwrap();
    assert!(
        logged.contains("\"prompt_tokens\":10") || logged.contains("10"),
        "{logged}"
    );
    let _ = std::fs::remove_file(&log_path);
}
