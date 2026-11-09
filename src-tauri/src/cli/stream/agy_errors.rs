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
                        d.get("retryDelay")?
                            .as_str()?
                            .trim_end_matches('s')
                            .parse::<f64>()
                            .ok()
                            .filter(|s| s.is_finite() && *s > 0.0)
                            .map(|s| {
                                chrono::Utc::now()
                                    .timestamp_millis()
                                    .saturating_add((s * 1000.0) as i64)
                            })
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
