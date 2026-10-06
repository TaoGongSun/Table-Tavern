//! 「這一幕太長、模型一次讀不完」的認錯（long-prompt-scene-hint 範圍 4）。
//! 只看結構化的失敗訊息：CLI 收尾事件標了 is_error 的那句、API 4xx body 的 error 物件。
//! 不在泛用錯誤全文裡掃字樣——玩家或模型的正文、router 包在 5xx 裡的上游殘骸都可能抄到這些字。

/// 前端分流用的穩定前綴（見 `src/shared/ui/ai-error.ts`）。
pub const CODE: &str = "AI_CONTEXT_TOO_LONG:";

/// 供應商錯誤訊息（單一一則）是不是在說輸入超過模型容量。
/// Gemini 的兩段字樣必須在同一則訊息裡同時出現。
pub fn message_says_too_long(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("prompt is too long")
        || lower.contains("context_length_exceeded")
        || lower.contains("maximum context length")
        || lower.contains("exceeds the context window")
        || lower.contains("maximum prompt length")
        || (lower.contains("input token count") && lower.contains("exceeds the maximum"))
}

/// API 非 2xx：只在 400／413 時看 body 的 error 物件（`code` 字串或 `message`）。
/// body 不是 JSON、沒有 error 物件就不認；陣列包一層（Gemini 相容端點）取第一個。
pub fn api_body_says_too_long(status: u16, body: &str) -> bool {
    if !matches!(status, 400 | 413) {
        return false;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return false;
    };
    let root = match &value {
        serde_json::Value::Array(items) => match items.first() {
            Some(first) => first,
            None => return false,
        },
        other => other,
    };
    let Some(error) = root.get("error") else {
        return false;
    };
    let text = |key: &str| {
        error
            .get(key)
            .and_then(|value| value.as_str())
            .unwrap_or("")
    };
    text("code") == "context_length_exceeded" || message_says_too_long(text("message"))
}

/// CLI 收尾事件報錯時的錯誤字串：認得出是太長就掛碼，否則回 None 走原本的包裝。
pub fn cli_failure(text: &str) -> Option<String> {
    message_says_too_long(text).then(|| format!("{CODE} {text}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_each_provider_wording() {
        for message in [
            "Prompt is too long · the request is ~902529 tokens (limit 200000)",
            "prompt is too long: 213000 tokens > 200000 maximum",
            "This model's maximum context length is 128000 tokens. However, your messages resulted in 130000 tokens.",
            "Your input exceeds the context window of this model.",
            "This model's maximum prompt length is 131072 but the request contains 200000 tokens.",
            "The input token count (1200000) exceeds the maximum number of tokens allowed (1048576).",
        ] {
            assert!(message_says_too_long(message), "{message}");
        }
    }

    #[test]
    fn negations_and_partial_gemini_wording_are_not_matched() {
        for message in [
            "prompt is not too long",
            "The input token count is 5000",
            "exceeds the maximum number of retries",
            "rate limit exceeded",
        ] {
            assert!(!message_says_too_long(message), "{message}");
        }
    }

    #[test]
    fn api_only_looks_at_4xx_error_objects() {
        let openai = r#"{"error":{"message":"too big","type":"invalid_request_error","code":"context_length_exceeded"}}"#;
        assert!(api_body_says_too_long(400, openai));
        assert!(api_body_says_too_long(413, openai));
        let anthropic = r#"{"type":"error","error":{"type":"invalid_request_error","message":"prompt is too long: 213000 tokens > 200000 maximum"}}"#;
        assert!(api_body_says_too_long(400, anthropic));
        let gemini = r#"[{"error":{"code":400,"message":"The input token count (1200000) exceeds the maximum number of tokens allowed (1048576)."}}]"#;
        assert!(api_body_says_too_long(400, gemini));
        // router 把上游 400 包在 5xx 裡：不認
        assert!(!api_body_says_too_long(503, openai));
        assert!(!api_body_says_too_long(200, openai));
        // 字樣不在 error 物件裡（例如回聲了玩家的正文）：不認
        let echoed = r#"{"error":{"message":"bad request"},"echo":"prompt is too long"}"#;
        assert!(!api_body_says_too_long(400, echoed));
        assert!(!api_body_says_too_long(400, "prompt is too long"));
        // Gemini 兩段字樣分在不同欄位：不認
        let split =
            r#"{"error":{"message":"The input token count (5)","status":"exceeds the maximum"}}"#;
        assert!(!api_body_says_too_long(400, split));
    }

    #[test]
    fn cli_failure_prefixes_only_matching_text() {
        assert_eq!(
            cli_failure("Prompt is too long").as_deref(),
            Some("AI_CONTEXT_TOO_LONG: Prompt is too long")
        );
        assert_eq!(cli_failure("Not logged in"), None);
    }
}
