//! API 串流的停滯偵測：只有「合格進展」（正文或推理 delta）才延後 deadline。
//! 期限從送出請求就起算，回應頭與首字共用同一個首字窗（見 `StallGuard::send`）。
//! 保活註解、空行、半行、空 `data:`、壞 JSON、role-only 塊都不續命——OpenRouter 會在上游卡住時
//! 持續送 `: OPENROUTER PROCESSING`，若按位元組計時就永遠不會逾時。
//! CLI 通道另有自己的 120 秒（`cli/runner.rs`，數輸出行）；兩邊數的東西不同，常數不共用。

use std::time::Duration;

use futures_util::{Stream, StreamExt};
use tokio::time::Instant;

/// 200 之後到第一個合格進展的等待（推理模型首字可能很慢）。
const FIRST_PROGRESS_SECS: u64 = 300;
/// 首個進展之後，兩個合格進展之間的等待（與 `cli/runner.rs` 的 120 秒同值）。
const PROGRESS_SECS: u64 = 120;
/// 非 2xx 時讀錯誤本文的等待；逾時當空本文，狀態碼照報。
const ERROR_BODY_SECS: u64 = 30;

#[derive(Clone, Copy, Debug)]
pub(crate) struct StallWindow {
    /// 送出到第一個合格進展（含等回應頭）的總預算。
    pub first: Duration,
    pub after: Duration,
    pub error_body: Duration,
}

impl Default for StallWindow {
    fn default() -> Self {
        Self {
            first: Duration::from_secs(FIRST_PROGRESS_SECS),
            after: Duration::from_secs(PROGRESS_SECS),
            error_body: Duration::from_secs(ERROR_BODY_SECS),
        }
    }
}

pub(crate) enum Sent {
    Response(reqwest::Response),
    Failed(reqwest::Error),
    Stalled,
}

pub(crate) enum Next<B> {
    Chunk(Result<B, reqwest::Error>),
    End,
    Stalled,
}

/// deadline 跨 chunk 保存：每次 `next` 只等到剩餘時間，不重給滿額。
pub(crate) struct StallGuard {
    window: StallWindow,
    deadline: Instant,
    progressed: bool,
}

impl StallGuard {
    pub fn new(window: StallWindow) -> Self {
        Self {
            window,
            deadline: Instant::now() + window.first,
            progressed: false,
        }
    }

    pub async fn next<B>(
        &self,
        stream: &mut (impl Stream<Item = Result<B, reqwest::Error>> + Unpin),
    ) -> Next<B> {
        match tokio::time::timeout_at(self.deadline, stream.next()).await {
            Ok(Some(chunk)) => Next::Chunk(chunk),
            Ok(None) => Next::End,
            Err(_) => Next::Stalled,
        }
    }

    /// 送出請求：回應頭與首字共用同一期限（本地伺服器常等首字才回應頭），所以總預算就是 `first`。
    /// 逾時丟掉 future＝取消本地等待並斷線，不保證上游停止或不計費。
    pub async fn send(&self, request: reqwest::RequestBuilder) -> Sent {
        match tokio::time::timeout_at(self.deadline, request.send()).await {
            Ok(Ok(response)) => Sent::Response(response),
            Ok(Err(error)) => Sent::Failed(error),
            Err(_) => Sent::Stalled,
        }
    }

    /// 非 2xx 的錯誤本文：讀失敗或逾時都回空字串。
    pub async fn error_body(&self, response: reqwest::Response) -> String {
        tokio::time::timeout(self.window.error_body, response.text())
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or_default()
    }

    /// 收到合格進展：之後的窗口改用 `after`。
    pub fn progress(&mut self) {
        self.progressed = true;
        self.deadline = Instant::now() + self.window.after;
    }

    /// 這次逾時等了多久（秒），給錯誤訊息。
    pub fn window_secs(&self) -> u64 {
        if self.progressed {
            self.window.after
        } else {
            self.window.first
        }
        .as_secs()
    }
}

/// 後端穩定碼（前端 `errStreamStalled`、`dispatch::ai_call_failure` 白名單認開頭）。
pub(crate) const STALLED_CODE: &str = "AI_STREAM_STALLED:";

pub(crate) fn stalled_message(secs: u64) -> String {
    format!("{STALLED_CODE} idle_secs={secs}")
}

fn non_empty_str<'a>(value: Option<&'a serde_json::Value>) -> bool {
    value
        .and_then(|value| value.as_str())
        .is_some_and(|text| !text.is_empty())
}

/// Chat Completions：`delta.content`、`delta.reasoning`、`delta.reasoning_content` 任一非空。
pub(crate) fn chat_progress(payload: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return false;
    };
    let Some(delta) = value
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("delta"))
    else {
        return false;
    };
    ["content", "reasoning", "reasoning_content"]
        .iter()
        .any(|key| non_empty_str(delta.get(key)))
}

/// Responses：`response.output_text.delta` 或任何 `response.reasoning*.delta` 的非空 `delta`。
pub(crate) fn responses_progress(payload: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return false;
    };
    let Some(kind) = value.get("type").and_then(|kind| kind.as_str()) else {
        return false;
    };
    let wanted = kind == "response.output_text.delta"
        || (kind.starts_with("response.reasoning") && kind.ends_with(".delta"));
    wanted && non_empty_str(value.get("delta"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_progress_counts_only_non_empty_text_or_reasoning() {
        assert!(chat_progress(r#"{"choices":[{"delta":{"content":"你"}}]}"#));
        assert!(chat_progress(
            r#"{"choices":[{"delta":{"reasoning":"想"}}]}"#
        ));
        assert!(chat_progress(
            r#"{"choices":[{"delta":{"reasoning_content":"想"}}]}"#
        ));
        assert!(!chat_progress(
            r#"{"choices":[{"delta":{"role":"assistant","content":""}}]}"#
        ));
        assert!(!chat_progress(r#"{"choices":[{"delta":{}}]}"#));
        assert!(!chat_progress("not json"));
        assert!(!chat_progress(""));
    }

    #[test]
    fn responses_progress_counts_text_and_reasoning_deltas_only() {
        assert!(responses_progress(
            r#"{"type":"response.output_text.delta","delta":"好"}"#
        ));
        assert!(responses_progress(
            r#"{"type":"response.reasoning_summary_text.delta","delta":"想"}"#
        ));
        assert!(responses_progress(
            r#"{"type":"response.reasoning_text.delta","delta":"想"}"#
        ));
        assert!(!responses_progress(
            r#"{"type":"response.created","response":{}}"#
        ));
        assert!(!responses_progress(
            r#"{"type":"response.output_text.delta","delta":""}"#
        ));
        assert!(!responses_progress("{"));
    }
}
