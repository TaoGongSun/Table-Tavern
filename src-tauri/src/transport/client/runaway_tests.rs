//! API 串流的輸出失控（runaway-output-cap）：腳本伺服器持續送空白，確認會停、回碼、帳本照實。

use super::super::test_support::stall_server;
use super::*;
use crate::transport::RunawayPolicy;

fn config(base: &str) -> AppConfig {
    let mut config = AppConfig::default();
    config.preferences.insert(
        "base_url".to_owned(),
        serde_json::Value::String(base.to_owned()),
    );
    config
}

fn sse_content(text: &str) -> String {
    format!(
        "data: {}\n\n",
        serde_json::json!({ "choices": [{ "delta": { "content": text } }] })
    )
}

fn sse_reasoning(text: &str) -> String {
    format!(
        "data: {}\n\n",
        serde_json::json!({ "choices": [{ "delta": { "reasoning": text } }] })
    )
}

const USAGE: &str =
    "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2}}\n\n";
const DONE: &str = "data: [DONE]\n\n";

fn user() -> [ChatMessage; 1] {
    [ChatMessage {
        role: "user".to_owned(),
        content: "嗨".to_owned(),
    }]
}

fn window() -> StallWindow {
    StallWindow {
        first: std::time::Duration::from_secs(10),
        after: std::time::Duration::from_secs(10),
    }
}

async fn chat(
    script: Vec<String>,
    policy: RunawayPolicy,
    log: Option<&std::path::Path>,
) -> DataResult<StreamChatResult> {
    // 連線撐 8 秒不關：失控偵測若沒生效，會等到連線結束才回別種錯誤
    let base = stall_server(script.into_iter().map(|text| (0, text)).collect(), 8000);
    let messages = user();
    stream_chat_windowed(
        &config(&base),
        "test/model",
        &messages,
        log,
        Some("w1"),
        crate::usage::log::PromptShape::Oneshot,
        policy,
        window(),
        |_| {},
    )
    .await
}

fn log_path(tag: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("tt-runaway-{tag}-{}.jsonl", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

#[tokio::test]
async fn blank_content_stops_with_runaway_code_not_incomplete() {
    let script = vec![sse_content("開頭"), sse_content(&" ".repeat(2500))];
    let error = chat(script, RunawayPolicy::Full, None)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        error.starts_with("AI_OUTPUT_RUNAWAY: reason=whitespace_run"),
        "{error}"
    );
}

#[tokio::test]
async fn length_cap_only_under_full_policy() {
    let long: Vec<String> = (0..40).map(|_| sse_content(&"字".repeat(1000))).collect();
    let error = chat(long.clone(), RunawayPolicy::Full, None)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        error.starts_with("AI_OUTPUT_RUNAWAY: reason=length"),
        "{error}"
    );
    let mut finished = long;
    finished.push(DONE.to_owned());
    let ok = chat(finished, RunawayPolicy::DegenerateOnly, None)
        .await
        .unwrap();
    assert_eq!(ok.text.chars().count(), 40_000);
}

#[tokio::test]
async fn blank_reasoning_trips_unless_policy_is_off() {
    let script = vec![
        sse_reasoning(&" ".repeat(2500)),
        sse_content("好"),
        DONE.to_owned(),
    ];
    let error = chat(script.clone(), RunawayPolicy::DegenerateOnly, None)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.starts_with("AI_OUTPUT_RUNAWAY:"), "{error}");
    let ok = chat(script, RunawayPolicy::Off, None).await.unwrap();
    assert_eq!(ok.text, "好");
}

fn sse_both_reasoning(text: &str) -> String {
    format!(
        "data: {}\n\n",
        serde_json::json!({ "choices": [{ "delta": { "reasoning": text, "reasoning_content": text } }] })
    )
}

/// 供應商兩欄都給同一段思考：只算一次，1,000 空白不會被算成 2,000 而誤殺
#[tokio::test]
async fn duplicated_reasoning_fields_are_counted_once() {
    let script = vec![
        sse_both_reasoning(&" ".repeat(1000)),
        sse_content("好"),
        DONE.to_owned(),
    ];
    let ok = chat(script, RunawayPolicy::Full, None).await.unwrap();
    assert_eq!(ok.text, "好");
}

/// 思考那支觸發時，碼裡的 chars 是思考的計數，不是正文的 0
#[tokio::test]
async fn thinking_trip_reports_thinking_count() {
    let script = vec![sse_reasoning(&" ".repeat(2500))];
    let error = chat(script, RunawayPolicy::Full, None)
        .await
        .unwrap_err()
        .to_string();
    assert_eq!(error, "AI_OUTPUT_RUNAWAY: reason=whitespace_run chars=2000");
}

#[tokio::test]
async fn usage_received_before_runaway_is_recorded_as_is() {
    let path = log_path("usage-before");
    let script = vec![
        sse_content("字"),
        USAGE.to_owned(),
        sse_content(&" ".repeat(2500)),
    ];
    let error = chat(script, RunawayPolicy::Full, Some(&path))
        .await
        .unwrap_err()
        .to_string();
    assert!(error.starts_with("AI_OUTPUT_RUNAWAY:"), "{error}");
    let logged = std::fs::read_to_string(&path).unwrap();
    assert_eq!(logged.lines().count(), 1, "{logged}");
    assert!(logged.contains("\"prompt_tokens\":10"), "{logged}");
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn no_usage_means_nothing_is_recorded() {
    let path = log_path("usage-none");
    let script = vec![sse_content(&" ".repeat(2500)), USAGE.to_owned()];
    let error = chat(script, RunawayPolicy::Full, Some(&path))
        .await
        .unwrap_err()
        .to_string();
    assert!(error.starts_with("AI_OUTPUT_RUNAWAY:"), "{error}");
    assert!(!path.exists(), "沒拿到 usage 就不估、不記");
}

#[tokio::test]
async fn smart_free_stream_reports_runaway_as_emitted_stream_failure() {
    // 第一塊就觸發：emitted_text 仍為真，智慧免費不會重送
    let base = stall_server(vec![(0, sse_content(&" ".repeat(2500)))], 8000);
    let messages = user();
    let failure = stream_chat_models_windowed(
        &config(&base),
        "test/model",
        &messages,
        None,
        None,
        crate::usage::log::PromptShape::Turn {
            roster: 1,
            solo: true,
        },
        RunawayPolicy::Full,
        window(),
        |_| {},
    )
    .await
    .unwrap_err();
    assert!(failure.emitted_text);
    assert_eq!(failure.stage, crate::transport::FailureStage::Stream);
    assert!(
        failure.display.starts_with("AI_OUTPUT_RUNAWAY:"),
        "{}",
        failure.display
    );
}
