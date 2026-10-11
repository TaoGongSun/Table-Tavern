//! 送出到回應頭、非 2xx 錯誤本文、生圖整段的逾時（api-request-header-timeout）。
//! 卡住的伺服器撐得遠比受測時限久，時限失效時由 watchdog 讓測試紅。

use super::super::test_support::{hang_server, stall_server, stall_server_after, watchdog};
use super::*;

fn config(base: &str) -> AppConfig {
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

fn user() -> [ChatMessage; 1] {
    [ChatMessage {
        role: "user".to_owned(),
        content: "嗨".to_owned(),
    }]
}

const DELTA: &str = "data: {\"choices\":[{\"delta\":{\"content\":\"字\"}}]}\n\n";

/// 卡住的伺服器撐這麼久才關；遠長於受測時限，舊碼只能等 watchdog。
const HANG_MS: u64 = 30_000;
const PARTIAL_500: &str =
    "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{\"err";
const PARTIAL_429: &str = "HTTP/1.1 429 Too Many Requests\r\nx-ratelimit-remaining: 0\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{\"err";
const PARTIAL_200_JSON: &str = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{\"data";

fn window(first_ms: u64, after_ms: u64, error_body_ms: u64) -> StallWindow {
    StallWindow {
        first: std::time::Duration::from_millis(first_ms),
        after: std::time::Duration::from_millis(after_ms),
        error_body: std::time::Duration::from_millis(error_body_ms),
    }
}

fn short_window() -> StallWindow {
    window(500, 500, 500)
}

async fn chat(base: &str, window: StallWindow) -> DataResult<StreamChatResult> {
    let messages = user();
    stream_chat_windowed(
        &config(base),
        "test/model",
        &messages,
        None,
        None,
        crate::usage::log::PromptShape::Oneshot,
        crate::transport::RunawayPolicy::Off,
        window,
        |_| {},
    )
    .await
}

async fn run_smart(base: &str, window: StallWindow) -> Result<SmartChatResult, ApiFailure> {
    let messages = user();
    stream_chat_models_windowed(
        &config(base),
        "test/model",
        &messages,
        None,
        None,
        crate::usage::log::PromptShape::Oneshot,
        crate::transport::RunawayPolicy::Off,
        window,
        |_| {},
    )
    .await
}

/// 連線接受了但回應頭一直不來：與首字共用同一期限，回停滯碼
#[tokio::test]
async fn headers_never_arrive_reports_stalled() {
    let base = hang_server("", HANG_MS);
    let result = watchdog(chat(&base, short_window())).await;
    let error = result.unwrap_err().to_string();
    assert!(
        error.starts_with("AI_STREAM_STALLED: idle_secs="),
        "{error}"
    );
}

/// 非 2xx 後錯誤本文卡住：放棄本文、狀態碼照報
#[tokio::test]
async fn error_body_stall_keeps_status() {
    let base = hang_server(PARTIAL_500, HANG_MS);
    let result = watchdog(chat(&base, short_window())).await;
    let error = result.unwrap_err().to_string();
    assert!(error.starts_with("AI_HTTP_STATUS_500:"), "{error}");
}

/// 回應頭已到、之後持續有進展：總時間超過首字窗也不誤殺（期限在首個進展後改用進展窗）
#[tokio::test]
async fn progressing_stream_outlives_first_window() {
    let mut script: Vec<(u64, String)> = (0..8).map(|_| (100, DELTA.to_owned())).collect();
    script.push((100, "data: [DONE]\n\n".to_owned()));
    let base = stall_server(script, 0);
    let result = chat(&base, window(300, 400, 500)).await;
    assert_eq!(result.unwrap().text, "字".repeat(8));
}

#[tokio::test]
async fn smart_headers_never_arrive_is_timeout() {
    let base = hang_server("", HANG_MS);
    let failure = watchdog(run_smart(&base, short_window()))
        .await
        .unwrap_err();
    assert_eq!(failure.stage, super::super::FailureStage::Timeout);
    assert!(!failure.emitted_text);
    assert!(failure.display.starts_with("AI_STREAM_STALLED:"));
}

/// 智慧免費：錯誤本文卡住仍保留狀態與 rate-limit 標頭，本文空
#[tokio::test]
async fn smart_error_body_stall_keeps_status_and_headers() {
    let base = hang_server(PARTIAL_429, HANG_MS);
    let failure = watchdog(run_smart(&base, short_window()))
        .await
        .unwrap_err();
    assert_eq!(failure.stage, super::super::FailureStage::Http);
    assert_eq!(failure.status, Some(429));
    assert_eq!(
        failure.rate_limit.and_then(|limit| limit.remaining),
        Some("0".to_owned())
    );
    assert!(failure.display.ends_with("body="), "{}", failure.display);
}

#[tokio::test]
async fn generate_image_headers_never_arrive_times_out() {
    let base = hang_server("", HANG_MS);
    let error = watchdog(generate_image(&config(&base), "畫"))
        .await
        .unwrap_err();
    assert!(error.starts_with("image request timed out"), "{error}");
}

/// 200 已到但成功本文卡住：整段時限照樣收
#[tokio::test]
async fn generate_image_success_body_stall_times_out() {
    let base = hang_server(PARTIAL_200_JSON, HANG_MS);
    let error = watchdog(generate_image(&config(&base), "畫"))
        .await
        .unwrap_err();
    assert!(error.starts_with("image request timed out"), "{error}");
}

/// 回應頭晚到、首個進展也晚到：兩段各自小於首字窗、合計超過，必須逾時
/// （防「收到回應頭後重設期限」的回歸：重設的話首個進展趕得上，串流會正常收完）
#[tokio::test]
async fn late_headers_plus_late_progress_share_one_deadline() {
    let script = vec![(400, DELTA.to_owned()), (0, "data: [DONE]\n\n".to_owned())];
    let base = stall_server_after(400, script, 0);
    let result = watchdog(chat(&base, window(600, 5000, 500))).await;
    let error = result.unwrap_err().to_string();
    assert!(error.starts_with("AI_STREAM_STALLED:"), "{error}");
}
