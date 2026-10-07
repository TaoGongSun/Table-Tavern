//! 智慧免費派送失敗的結構化資訊：HTTP 狀態、平台限流 headers、完整解析的錯誤物件、
//! 失敗前有沒有吐出正文。分類（smart_free::failover）只讀這份結構，不回頭解析截斷過的顯示字串。

use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FailureStage {
    /// 非 2xx 回應。
    #[default]
    Http,
    /// 200 之後串流裡的錯誤塊或收尾判定（空回應、不完整、內容過濾）。
    Stream,
    /// 連不上或串流中途斷線。
    Network,
    Timeout,
}

/// `X-RateLimit-*` 原值；平台限流的證據。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RateLimit {
    pub limit: Option<String>,
    pub remaining: Option<String>,
    pub reset: Option<String>,
}

impl RateLimit {
    pub fn remaining_is_zero(&self) -> bool {
        self.remaining
            .as_deref()
            .and_then(|value| value.trim().parse::<f64>().ok())
            .is_some_and(|remaining| remaining <= 0.0)
    }
}

/// 供應商錯誤物件（HTTP body 或 SSE 頂層 `error`）解析出來的欄位。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ErrorDetail {
    pub code: Option<i64>,
    pub message: String,
    pub error_type: Option<String>,
    /// 只當線索，不當上游確證（官方沒保證只出現在上游錯誤）。
    pub provider_name: Option<String>,
    pub has_raw: bool,
}

impl ErrorDetail {
    pub fn from_error_object(error: &serde_json::Value) -> Self {
        let metadata = error.get("metadata");
        let text = |value: Option<&serde_json::Value>| {
            value
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        };
        Self {
            code: error.get("code").and_then(|code| {
                code.as_i64()
                    .or_else(|| code.as_str()?.trim().parse::<i64>().ok())
            }),
            message: text(error.get("message")).unwrap_or_default(),
            error_type: text(metadata.and_then(|metadata| metadata.get("error_type"))),
            provider_name: text(metadata.and_then(|metadata| metadata.get("provider_name"))),
            has_raw: metadata
                .and_then(|metadata| metadata.get("raw"))
                .is_some_and(|raw| !raw.is_null()),
        }
    }

    /// 完整 body（不截斷）裡的頂層 `error`；不是 JSON 或沒有 error 就回空。
    fn from_body(body: &str) -> Self {
        serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|value| value.get("error").map(Self::from_error_object))
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApiFailure {
    pub stage: FailureStage,
    /// HTTP 狀態；串流錯誤時為 None（狀態已是 200），改看 `detail.code`。
    pub status: Option<u16>,
    pub rate_limit: Option<RateLimit>,
    /// `Retry-After` 換算成距今秒數；只記錄，不影響重送與試打節流。
    pub retry_after_secs: Option<u64>,
    pub detail: ErrorDetail,
    /// 失敗前是否已經吐出任何正文。
    pub emitted_text: bool,
    /// 給前端的字串，沿用現行 `AI_HTTP_STATUS_*`／`AI_*` 格式（會截斷）。
    pub display: String,
}

impl std::fmt::Display for ApiFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.display)
    }
}

impl ApiFailure {
    pub fn from_http(
        status: reqwest::StatusCode,
        headers: &reqwest::header::HeaderMap,
        body: &str,
    ) -> Self {
        let header = |name: &str| {
            headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
        };
        let rate_limit = RateLimit {
            limit: header("x-ratelimit-limit"),
            remaining: header("x-ratelimit-remaining"),
            reset: header("x-ratelimit-reset"),
        };
        let has_rate_limit = rate_limit.limit.is_some()
            || rate_limit.remaining.is_some()
            || rate_limit.reset.is_some();
        Self {
            stage: FailureStage::Http,
            status: Some(status.as_u16()),
            rate_limit: has_rate_limit.then_some(rate_limit),
            retry_after_secs: header("retry-after")
                .and_then(|value| parse_retry_after(&value, now_secs())),
            detail: ErrorDetail::from_body(body),
            emitted_text: false,
            display: super::client::http_error(status, body),
        }
    }

    pub fn network(error: &reqwest::Error, emitted_text: bool) -> Self {
        Self {
            stage: if error.is_timeout() {
                FailureStage::Timeout
            } else {
                FailureStage::Network
            },
            emitted_text,
            display: error.to_string(),
            ..Self::default()
        }
    }

    /// 串流停滯逾時（見 `stall.rs`）：分類走 Timeout，顯示字串帶 `AI_STREAM_STALLED:` 碼。
    pub fn stalled(secs: u64, emitted_text: bool) -> Self {
        Self {
            stage: FailureStage::Timeout,
            emitted_text,
            display: super::stall::stalled_message(secs),
            ..Self::default()
        }
    }

    /// 輸出失控（`runaway.rs`）：失控必定已吐出內容，`emitted_text` 恆為真＝智慧免費不重送；
    /// 階段記 Stream，分類落在 Other，不計入換模。
    pub fn runaway(display: String) -> Self {
        Self {
            stage: FailureStage::Stream,
            emitted_text: true,
            display,
            ..Self::default()
        }
    }

    /// 串流收尾判定失敗：`display` 是 `StreamOutcome::failure` 的字串，detail 來自 SSE 錯誤塊（若有）。
    pub fn stream(display: String, detail: Option<ErrorDetail>, emitted_text: bool) -> Self {
        Self {
            stage: FailureStage::Stream,
            detail: detail.unwrap_or_default(),
            emitted_text,
            display,
            ..Self::default()
        }
    }

    /// 不是供應商回的錯（例如缺金鑰）：分類一律落到「其他」。
    pub fn local(display: String) -> Self {
        Self {
            stage: FailureStage::Stream,
            display,
            ..Self::default()
        }
    }

    /// HTTP 狀態或 SSE 錯誤碼，先看 HTTP。
    pub fn code(&self) -> Option<i64> {
        self.status.map(i64::from).or(self.detail.code)
    }
}

/// `Retry-After`：delta-seconds 或 HTTP-date（IMF-fixdate，例如 `Sun, 06 Nov 1994 08:49:37 GMT`）。
pub fn parse_retry_after(raw: &str, now: u64) -> Option<u64> {
    let raw = raw.trim();
    if let Ok(seconds) = raw.parse::<u64>() {
        return Some(seconds);
    }
    let at = parse_http_date(raw)?;
    Some(at.saturating_sub(now))
}

fn parse_http_date(raw: &str) -> Option<u64> {
    // "Sun, 06 Nov 1994 08:49:37 GMT"
    let (_, rest) = raw.split_once(", ")?;
    let mut parts = rest.split_whitespace();
    let day = parts.next()?.parse::<i64>().ok()?;
    let month = match parts.next()? {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return None,
    };
    let year = parts.next()?.parse::<i64>().ok()?;
    let mut clock = parts.next()?.split(':');
    let hour = clock.next()?.parse::<i64>().ok()?;
    let minute = clock.next()?.parse::<i64>().ok()?;
    let second = clock.next()?.parse::<i64>().ok()?;
    if parts.next()? != "GMT" || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60
    {
        return None;
    }
    let days = days_from_civil(year, month, day);
    u64::try_from(days * 86_400 + hour * 3_600 + minute * 60 + second).ok()
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_after_accepts_seconds_and_http_date() {
        assert_eq!(parse_retry_after("30", 0), Some(30));
        // 784111777 = 1994-11-06T08:49:37Z
        assert_eq!(
            parse_retry_after("Sun, 06 Nov 1994 08:49:37 GMT", 784_111_777 - 10),
            Some(10)
        );
        assert_eq!(
            parse_retry_after("Sun, 06 Nov 1994 08:49:37 GMT", 784_111_777 + 5),
            Some(0)
        );
        assert_eq!(parse_retry_after("soon", 0), None);
    }

    #[test]
    fn http_failure_parses_full_body_beyond_display_truncation() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("x-ratelimit-remaining", "0".parse().unwrap());
        headers.insert("retry-after", "12".parse().unwrap());
        let padding = "x".repeat(3000);
        let body = serde_json::json!({
            "padding": padding,
            "error": {
                "code": 429,
                "message": "Rate limit exceeded",
                "metadata": {"error_type": "rate_limit_exceeded", "provider_name": "Google AI Studio", "raw": "upstream"}
            }
        })
        .to_string();
        let failure =
            ApiFailure::from_http(reqwest::StatusCode::TOO_MANY_REQUESTS, &headers, &body);
        assert!(failure.display.ends_with("…[truncated]"));
        assert_eq!(failure.status, Some(429));
        assert_eq!(
            failure.detail.error_type.as_deref(),
            Some("rate_limit_exceeded")
        );
        assert_eq!(
            failure.detail.provider_name.as_deref(),
            Some("Google AI Studio")
        );
        assert!(failure.detail.has_raw);
        assert!(failure.rate_limit.as_ref().unwrap().remaining_is_zero());
        assert_eq!(failure.retry_after_secs, Some(12));
    }

    #[test]
    fn missing_rate_limit_headers_leave_none() {
        let failure = ApiFailure::from_http(
            reqwest::StatusCode::TOO_MANY_REQUESTS,
            &reqwest::header::HeaderMap::new(),
            "not json",
        );
        assert!(failure.rate_limit.is_none());
        assert_eq!(failure.detail, ErrorDetail::default());
    }
}
