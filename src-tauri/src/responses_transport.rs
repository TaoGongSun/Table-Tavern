//! OpenAI Responses API transport.
//!
//! Keep this separate from `transport::stream_chat`: the existing Chat Completions path is mature
//! and shared by OpenRouter / OpenAI-compatible providers. Responses uses a different request body,
//! streaming event schema, completion signal, and usage shape, so adapting it here avoids changing
//! the behavior of the legacy path.

use crate::data::{AppConfig, DataResult};
use crate::{transport, usage_log};
use futures_util::StreamExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ApiMode {
    ChatCompletions,
    Responses,
}

/// Resolve the API dialect without hard-coding any model id.
///
/// `auto` is deliberately conservative for backwards compatibility: existing base URLs keep using
/// Chat Completions. A full endpoint ending in `/responses` is the only automatic opt-in. Users can
/// always explicitly select `responses` in settings when their provider exposes both endpoint types.
pub(crate) fn api_mode(config: &AppConfig) -> ApiMode {
    match config
        .preferences
        .get("api_mode")
        .and_then(|value| value.as_str())
        .unwrap_or("auto")
    {
        "responses" => ApiMode::Responses,
        "chat_completions" => ApiMode::ChatCompletions,
        _ => {
            let base = transport::base_url(config);
            if base.trim_end_matches('/').ends_with("/responses") {
                ApiMode::Responses
            } else {
                ApiMode::ChatCompletions
            }
        }
    }
}

fn endpoint(config: &AppConfig) -> String {
    let base = transport::base_url(config);
    let base = base.trim_end_matches('/');
    if base.ends_with("/responses") {
        base.to_owned()
    } else if let Some(root) = base.strip_suffix("/chat/completions") {
        format!("{root}/responses")
    } else {
        format!("{base}/responses")
    }
}

fn request_body(model: &str, messages: &[transport::ChatMessage]) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "input": messages,
        "stream": true,
    })
}

fn extract_delta(payload: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;
    if value.get("type")?.as_str()? != "response.output_text.delta" {
        return None;
    }
    let text = value.get("delta")?.as_str()?;
    if text.is_empty() {
        None
    } else {
        Some(text.to_owned())
    }
}

fn extract_usage(payload: &str) -> Option<transport::PromptCacheUsage> {
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;
    let usage = value.get("response")?.get("usage")?;
    if usage.is_null() {
        return None;
    }
    let input_tokens = usage.get("input_tokens")?.as_u64()?;
    let cached_tokens = usage
        .get("input_tokens_details")
        .and_then(|details| details.get("cached_tokens"))
        .and_then(|tokens| tokens.as_u64());
    Some(transport::PromptCacheUsage {
        prompt_tokens: input_tokens,
        cached_tokens,
        created_tokens: None,
        output_tokens: usage
            .get("output_tokens")
            .and_then(|tokens| tokens.as_u64())
            .unwrap_or(0),
        cost_usd: None,
    })
}

#[derive(Default)]
struct ResponsesOutcome {
    error: Option<String>,
    incomplete_reason: Option<String>,
    reasoning_tokens: Option<u64>,
    terminal: bool,
}

impl ResponsesOutcome {
    fn absorb(&mut self, payload: &str) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
            return;
        };
        let event_type = value.get("type").and_then(|value| value.as_str());

        // The Responses stream has a standalone `error` event in addition to response.failed.
        if event_type == Some("error") {
            self.error = value
                .get("message")
                .and_then(|message| message.as_str())
                .filter(|message| !message.trim().is_empty())
                .map(str::to_owned)
                .or_else(|| Some(value.to_string()));
            self.terminal = true;
            return;
        }

        let response = value.get("response");
        if let Some(tokens) = response
            .and_then(|response| response.get("usage"))
            .and_then(|usage| usage.get("output_tokens_details"))
            .and_then(|details| details.get("reasoning_tokens"))
            .and_then(|tokens| tokens.as_u64())
        {
            self.reasoning_tokens = Some(tokens);
        }

        match event_type {
            Some("response.completed") => {
                self.terminal = true;
            }
            Some("response.failed") => {
                self.terminal = true;
                self.error = response
                    .and_then(|response| response.get("error"))
                    .filter(|error| !error.is_null())
                    .map(|error| {
                        error
                            .get("message")
                            .and_then(|message| message.as_str())
                            .filter(|message| !message.trim().is_empty())
                            .map(str::to_owned)
                            .unwrap_or_else(|| error.to_string())
                    })
                    .or_else(|| Some("Responses API 回傳失敗".to_owned()));
            }
            Some("response.incomplete") => {
                self.terminal = true;
                self.incomplete_reason = response
                    .and_then(|response| response.get("incomplete_details"))
                    .and_then(|details| details.get("reason"))
                    .and_then(|reason| reason.as_str())
                    .map(str::to_owned)
                    .or_else(|| Some("unknown".to_owned()));
            }
            _ => {}
        }
    }

    fn failure(&self, text: &str, model: &str) -> Option<String> {
        if let Some(error) = &self.error {
            return Some(error.clone());
        }
        let reasoning = self
            .reasoning_tokens
            .map(|tokens| format!(" reasoning_tokens={tokens}"))
            .unwrap_or_default();
        if let Some(reason) = &self.incomplete_reason {
            return Some(format!(
                "AI_INCOMPLETE_RESPONSE: model={model} status=incomplete reason={reason}{reasoning}"
            ));
        }
        if !self.terminal {
            return Some(format!(
                "AI_INCOMPLETE_RESPONSE: model={model} status=(無完成事件){reasoning}"
            ));
        }
        if text.trim().is_empty() {
            return Some(format!(
                "AI_EMPTY_RESPONSE: model={model} status=completed{reasoning}"
            ));
        }
        None
    }
}

fn http_error(status: reqwest::StatusCode, body: &str) -> String {
    const LIMIT: usize = 2000;
    let kept: String = body.chars().take(LIMIT).collect();
    let cut = if body.chars().nth(LIMIT).is_some() {
        "…（原始回應已截斷）"
    } else {
        ""
    };
    format!(
        "AI_HTTP_STATUS_{}: API 回應 {status}：{kept}{cut}",
        status.as_u16(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn stream_responses(
    config: &AppConfig,
    model: &str,
    messages: &[transport::ChatMessage],
    usage_log_path: Option<&std::path::Path>,
    world: Option<&str>,
    shape: usage_log::PromptShape,
    mut on_delta: impl FnMut(&str),
) -> DataResult<String> {
    let base = transport::base_url(config);
    let api_key = config
        .api_keys
        .get("openrouter")
        .filter(|key| !key.is_empty());
    if api_key.is_none() && base == transport::DEFAULT_BASE_URL {
        return Err("尚未設定 OpenRouter API key，請先到設定貼上".into());
    }

    let mut request = reqwest::Client::new()
        .post(endpoint(config))
        .json(&request_body(model, messages));
    if let Some(key) = api_key {
        request = request.bearer_auth(key);
    }

    let response = request.send().await?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(http_error(status, &body).into());
    }

    let mut stream = response.bytes_stream();
    let mut parser = transport::SseParser::default();
    let mut full_text = String::new();
    let mut usage = None;
    let mut outcome = ResponsesOutcome::default();

    'outer: while let Some(chunk) = stream.next().await {
        for payload in parser.push(&chunk?) {
            // Some compatible gateways still send the Chat Completions sentinel after a Responses
            // stream. Accept it as a terminal marker, but do not require it.
            if payload == "[DONE]" {
                outcome.terminal = true;
                break 'outer;
            }
            outcome.absorb(&payload);
            if let Some(parsed) = extract_usage(&payload) {
                usage = Some(parsed);
            }
            if let Some(delta) = extract_delta(&payload) {
                on_delta(&delta);
                full_text.push_str(&delta);
            }
            if outcome.terminal {
                break 'outer;
            }
        }
    }

    if let Some(usage) = usage {
        eprintln!(
            "[prompt-cache] transport=api model={model} prompt_tokens={} cached_tokens={} created_tokens={} hit_rate={}",
            usage.prompt_tokens,
            transport::describe(usage.cached_tokens),
            transport::describe(usage.created_tokens),
            usage
                .hit_rate()
                .map_or_else(|| "—（這條路不回報快取）".to_owned(), |rate| format!("{rate:.0}%")),
        );
        if let Some(path) = usage_log_path {
            usage_log::append_call(path, world, "api", model, None, shape, usage);
        }
    }

    if let Some(failure) = outcome.failure(&full_text, model) {
        return Err(failure.into());
    }
    Ok(full_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(role: &str, content: &str) -> transport::ChatMessage {
        transport::ChatMessage {
            role: role.to_owned(),
            content: content.to_owned(),
        }
    }

    #[test]
    fn api_mode_defaults_to_chat_and_allows_explicit_responses() {
        let mut config = AppConfig::default();
        assert_eq!(api_mode(&config), ApiMode::ChatCompletions);

        config.preferences.insert(
            "api_mode".to_owned(),
            serde_json::Value::String("responses".to_owned()),
        );
        assert_eq!(api_mode(&config), ApiMode::Responses);
    }

    #[test]
    fn api_mode_auto_detects_full_responses_endpoint() {
        let mut config = AppConfig::default();
        config.preferences.insert(
            "base_url".to_owned(),
            serde_json::Value::String("https://opencode.ai/zen/v1/responses".to_owned()),
        );
        assert_eq!(api_mode(&config), ApiMode::Responses);
        assert_eq!(endpoint(&config), "https://opencode.ai/zen/v1/responses");
    }

    #[test]
    fn request_body_uses_responses_input_without_model_special_cases() {
        let messages = [message("system", "規則"), message("user", "嗨")];
        assert_eq!(
            request_body("any/responses-model", &messages),
            serde_json::json!({
                "model": "any/responses-model",
                "input": [
                    {"role": "system", "content": "規則"},
                    {"role": "user", "content": "嗨"}
                ],
                "stream": true
            })
        );
    }

    #[test]
    fn responses_delta_and_usage_are_parsed() {
        assert_eq!(
            extract_delta(
                r#"{"type":"response.output_text.delta","delta":"你好","sequence_number":1}"#
            )
            .as_deref(),
            Some("你好")
        );
        let usage = extract_usage(
            r#"{"type":"response.completed","response":{"usage":{"input_tokens":120,"input_tokens_details":{"cached_tokens":80},"output_tokens":30,"output_tokens_details":{"reasoning_tokens":5}}}}"#,
        )
        .unwrap();
        assert_eq!(usage.prompt_tokens, 120);
        assert_eq!(usage.cached_tokens, Some(80));
        assert_eq!(usage.created_tokens, None);
        assert_eq!(usage.output_tokens, 30);
    }

    #[test]
    fn responses_outcome_uses_responses_terminal_events() {
        let mut completed = ResponsesOutcome::default();
        completed.absorb(r#"{"type":"response.completed","response":{"usage":null}}"#);
        assert_eq!(completed.failure("正文", "model"), None);

        let mut incomplete = ResponsesOutcome::default();
        incomplete.absorb(
            r#"{"type":"response.incomplete","response":{"incomplete_details":{"reason":"max_output_tokens"}}}"#,
        );
        assert!(incomplete
            .failure("半截", "model")
            .unwrap()
            .starts_with("AI_INCOMPLETE_RESPONSE:"));

        let mut failed = ResponsesOutcome::default();
        failed.absorb(
            r#"{"type":"response.failed","response":{"error":{"message":"provider failed"}}}"#,
        );
        assert_eq!(failed.failure("", "model").as_deref(), Some("provider failed"));
    }
}
