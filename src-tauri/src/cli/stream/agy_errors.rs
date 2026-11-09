//! agy 工具錯誤只讀結構化欄位，絕不從 prompt 或故事猜額度。
use super::super::types::CliLine;
use serde_json::Value;

pub(crate) const QUOTA: &str = "AI_IMAGE_QUOTA:";

pub(super) fn tool_error(step: &Value) -> Option<CliLine> {
    if step.get("step_type")?.as_str()? != "tool"
        || step.get("tool_name")?.as_str()? != "generate_image"
        || step.get("state")?.as_str()? != "ERROR"
    {
        return None;
    }
    let message = step.pointer("/tool_info/error/message")?.as_str()?;
    // 不記 parameters；常見路徑與帳號在進任何診斷出口前遮蔽。
    let mut safe = message.to_owned();
    for key in ["HOME", "USER", "USERNAME", "USERPROFILE"] {
        if let Ok(value) = std::env::var(key) {
            if !value.is_empty() {
                safe = safe.replace(&value, "<local>");
            }
        }
    }
    static ACCOUNT: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let safe = ACCOUNT
        .get_or_init(|| {
            regex::Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}")
                .expect("account pattern")
        })
        .replace_all(&safe, "<account>")
        .into_owned();
    log::warn!("agy generate_image: {safe}");
    let body = message
        .split_once("body:")
        .and_then(|(_, body)| serde_json::from_str::<Value>(body.trim()).ok());
    if let Some(detail) = body
        .as_ref()
        .and_then(|body| body.pointer("/error/details")?.as_array())
    {
        if detail
            .iter()
            .any(|d| d.get("reason").and_then(Value::as_str) == Some("QUOTA_EXHAUSTED"))
        {
            let reset = detail
                .iter()
                .find_map(|d| {
                    d.pointer("/metadata/quotaResetTimeStamp")?
                        .as_str()
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.timestamp_millis())
                })
                .or_else(|| {
                    detail.iter().find_map(|d| {
                        let delay = d
                            .get("retryDelay")
                            .or_else(|| d.pointer("/metadata/quotaResetDelay"))?
                            .as_str()?;
                        chrono::Utc::now()
                            .timestamp_millis()
                            .checked_add(delay_ms(delay)?)
                    })
                });
            return Some(CliLine::Done {
                text: format!(
                    "{QUOTA}{}",
                    reset.map(|ms| ms.to_string()).unwrap_or_default()
                ),
                is_error: true,
            });
        }
    }
    Some(CliLine::SoftError(safe))
}

fn delay_ms(text: &str) -> Option<i64> {
    static PARTS: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let parts = PARTS
        .get_or_init(|| regex::Regex::new(r"(\d+(?:\.\d+)?)(h|m|s)").expect("duration pattern"));
    let mut end = 0;
    let mut seconds = 0.0;
    for part in parts.captures_iter(text) {
        let matched = part.get(0)?;
        if matched.start() != end {
            return None;
        }
        end = matched.end();
        let scale = match &part[2] {
            "h" => 3600.0,
            "m" => 60.0,
            _ => 1.0,
        };
        seconds += part[1].parse::<f64>().ok()? * scale;
    }
    (end == text.len()
        && seconds > 0.0
        && seconds.is_finite()
        && seconds < i64::MAX as f64 / 1000.0)
        .then_some((seconds * 1000.0).round() as i64)
}

pub(crate) const REFUSAL: &str = "AI_CONTENT_FILTERED: agy";
pub(crate) fn is_refusal(text: &str) -> bool {
    let text = text.trim();
    text.len() <= 1024
        && text.starts_with("The prompt could not be submitted.")
        && text.contains("Prohibited Use policy")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn error(reason: &str) -> Value {
        json!({"step_type":"tool","tool_name":"generate_image","state":"ERROR","tool_info":{"error":{"type":"TOOL_ERROR","message":format!("failed: 429 body: {}", json!({"error":{"code":429,"details":[{"reason":reason,"metadata":{"quotaResetTimeStamp":"2030-01-01T00:00:00Z"}}]}}))}}})
    }
    #[test]
    fn quota_is_structural_and_survives_a_successful_finish() {
        assert_eq!(
            tool_error(&error("QUOTA_EXHAUSTED")),
            Some(CliLine::Done {
                text: "AI_IMAGE_QUOTA:1893456000000".into(),
                is_error: true
            })
        );
        assert!(matches!(
            tool_error(&error("OTHER")),
            Some(CliLine::SoftError(_))
        ));
        let mut other = error("QUOTA_EXHAUSTED");
        other["tool_name"] = json!("view_file");
        assert_eq!(tool_error(&other), None);
        assert!(crate::cli::AGY_PROTOCOL.sticky_failed_done);
    }
    #[test]
    fn unknown_reset_is_not_invented() {
        let step = json!({"step_type":"tool","tool_name":"generate_image","state":"ERROR","tool_info":{"error":{"message":"body: {\"error\":{\"details\":[{\"reason\":\"QUOTA_EXHAUSTED\"}]}}"}}});
        assert_eq!(
            tool_error(&step),
            Some(CliLine::Done {
                text: QUOTA.into(),
                is_error: true
            })
        );
    }
}

#[cfg(test)]
mod refusal_tests {
    use super::*;
    const SAMPLE: &str = "The prompt could not be submitted. The prompt contains sensitive words that violate Google's [Generative AI Prohibited Use policy](https://policies.google.com/terms/generative-ai/use-policy). Try rephrasing the prompt. If you think this was an error, [send feedback](https://aistudio.google.com/docs/troubleshooting).";
    #[test]
    fn matches_only_the_known_short_prefix_and_policy_combination() {
        assert!(is_refusal(&format!(" \n{SAMPLE}\r\n")));
        for text in [
            format!("The character said: {SAMPLE}"),
            SAMPLE.replace("Prohibited Use policy", "other policy"),
            format!("{SAMPLE}{}", "x".repeat(1024)),
            "I cannot help you".to_owned(),
        ] {
            assert!(!is_refusal(&text));
        }
        let line =
            serde_json::json!({"event":"result","result":{"status":"SUCCESS","response":SAMPLE}})
                .to_string();
        assert_eq!(
            super::super::parse_agy_line(&line),
            CliLine::Done {
                text: REFUSAL.to_owned(),
                is_error: true
            }
        );
    }
}

#[cfg(test)]
mod duration_tests {
    use super::*;
    #[test]
    fn accepts_provider_delays_without_inventing_unknown_units() {
        assert_eq!(delay_ms("4h39m1.35s"), Some(16_741_350));
        assert_eq!(delay_ms("12.5s"), Some(12_500));
        assert_eq!(delay_ms("oops4h"), None);
        assert_eq!(delay_ms("4days"), None);
        assert_eq!(delay_ms(""), None);
    }
}
