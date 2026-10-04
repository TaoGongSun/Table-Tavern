use super::select;
use crate::transport::{openrouter_api_base, ApiFailure};
use std::time::Duration;

pub const FETCH_TIMEOUT: Duration = Duration::from_secs(15);

pub fn client() -> Option<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(FETCH_TIMEOUT)
        .build()
        .ok()
}

pub async fn fetch_user_catalog(
    client: &reqwest::Client,
    api_key: &str,
) -> Option<Vec<select::FreeModel>> {
    let response = client
        .get(format!("{}/models/user", openrouter_api_base()))
        .bearer_auth(api_key)
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    select::parse_catalog(&response.json().await.ok()?)
}

pub async fn fetch_weekly_ids(client: &reqwest::Client) -> Option<Vec<String>> {
    let response = client
        .get(format!("{}/models?sort=top-weekly", openrouter_api_base()))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    select::parse_ranked_ids(&response.json().await.ok()?)
}

/// 角色扮演排行：回傳順序＝名次，走 canonical_slug 才能對上帳號的免費版模型。
/// 公開排行、不佔免費模型每日次數，也不需要金鑰。
pub async fn fetch_roleplay_slugs(client: &reqwest::Client) -> Option<Vec<String>> {
    let response = client
        .get(format!(
            "{}/models?category=roleplay",
            openrouter_api_base()
        ))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    select::parse_ranked_slugs(&response.json().await.ok()?)
}

/// 單支模型的免費端點上游（endpoints API 的 provider_name）。路徑帶 `:free` 只回免費端點。
/// 抓不到或格式不對回 None，呼叫端保留舊值。不是模型呼叫，不佔每日次數。
pub async fn fetch_upstreams(
    client: &reqwest::Client,
    api_key: &str,
    model_id: &str,
) -> Option<Vec<String>> {
    let response = client
        .get(format!(
            "{}/models/{model_id}/endpoints",
            openrouter_api_base()
        ))
        .bearer_auth(api_key)
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    parse_upstreams(&response.json().await.ok()?)
}

fn parse_upstreams(body: &serde_json::Value) -> Option<Vec<String>> {
    let endpoints = body.pointer("/data/endpoints")?.as_array()?;
    let mut providers: Vec<String> = endpoints
        .iter()
        .filter_map(|endpoint| endpoint.get("provider_name")?.as_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect();
    providers.sort();
    providers.dedup();
    Some(providers)
}

pub const PROBE_TIMEOUT: Duration = Duration::from_secs(20);

/// 試打：非串流最小請求（`max_tokens: 1`）。只看上游有沒有接單——2xx 且 body 無 `error` 即算可用，
/// 不看內容（小 max_tokens 可能被推理吃光而回空）。文件沒寫會不會扣每日次數，一律當作會扣。
pub async fn probe(api_key: &str, model: &str) -> Result<(), ApiFailure> {
    let client = reqwest::Client::builder()
        .timeout(PROBE_TIMEOUT)
        .build()
        .map_err(|error| ApiFailure::network(&error, false))?;
    let response = client
        .post(format!("{}/chat/completions", openrouter_api_base()))
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 1,
            "stream": false,
        }))
        .send()
        .await
        .map_err(|error| ApiFailure::network(&error, false))?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response
        .text()
        .await
        .map_err(|error| ApiFailure::network(&error, false))?;
    let has_error = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .is_some_and(|value| value.get("error").is_some_and(|error| !error.is_null()));
    if status.is_success() && !has_error {
        return Ok(());
    }
    if status.is_success() {
        // 200 卻帶 error：當成串流錯誤塊處理，分類只看錯誤物件
        let value: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
        let detail = value
            .get("error")
            .map(crate::transport::ErrorDetail::from_error_object);
        return Err(ApiFailure::stream(
            crate::transport::http_error(status, &body),
            detail,
            false,
        ));
    }
    Err(ApiFailure::from_http(status, &headers, &body))
}

/// 帳號免費模型的每日請求額度（所有 `:free` 模型共用一個池）。
#[derive(Debug, Clone, Copy, serde::Serialize, PartialEq, Eq)]
pub struct FreeDaily {
    pub limit: i64,
    pub remaining: i64,
}

pub async fn fetch_free_daily(client: &reqwest::Client, api_key: &str) -> Option<FreeDaily> {
    let response = client
        .get(format!("{}/key", openrouter_api_base()))
        .bearer_auth(api_key)
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    free_daily_from_body(&response.json().await.ok()?)
}

fn free_daily_from_body(body: &serde_json::Value) -> Option<FreeDaily> {
    let data = body.get("data").unwrap_or(body);
    let daily = data.get("free_model_daily_requests")?;
    let limit = daily.get("limit").and_then(integer)?;
    let remaining = daily.get("remaining").and_then(integer).or_else(|| {
        let used = daily
            .get("used")
            .or_else(|| daily.get("usage"))
            .and_then(integer)?;
        Some(limit.saturating_sub(used))
    })?;
    Some(FreeDaily { limit, remaining })
}

pub async fn free_daily_remaining(client: &reqwest::Client, api_key: &str) -> Option<i64> {
    let response = client
        .get(format!("{}/key", openrouter_api_base()))
        .bearer_auth(api_key)
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    quota_remaining_from_body(&response.json().await.ok()?)
}

fn integer(value: &serde_json::Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|number| i64::try_from(number).ok()))
        .or_else(|| value.as_str()?.parse::<i64>().ok())
}

fn quota_remaining_from_body(body: &serde_json::Value) -> Option<i64> {
    let data = body.get("data").unwrap_or(body);
    let daily = data.get("free_model_daily_requests")?;
    if let Some(remaining) = integer(daily) {
        return Some(remaining);
    }
    if let Some(remaining) = daily.get("remaining").and_then(integer) {
        return Some(remaining);
    }
    if let Some(remaining) = daily.get("remaining_requests").and_then(integer) {
        return Some(remaining);
    }
    let limit = daily.get("limit").and_then(integer)?;
    let used = daily
        .get("usage")
        .or_else(|| daily.get("used"))
        .and_then(integer)?;
    Some(limit.saturating_sub(used))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstreams_read_provider_names_of_free_endpoints() {
        assert_eq!(
            parse_upstreams(&serde_json::json!({"data": {"endpoints": [
                {"provider_name": "Google AI Studio", "tag": "google-ai-studio"},
                {"provider_name": "Chutes"},
                {"provider_name": "Google AI Studio"},
                {"provider_name": ""}
            ]}})),
            Some(vec!["Chutes".to_owned(), "Google AI Studio".to_owned()])
        );
        assert_eq!(parse_upstreams(&serde_json::json!({"error": "down"})), None);
    }

    #[test]
    fn free_daily_reads_remaining_or_derives_from_limit_and_usage() {
        assert_eq!(
            free_daily_from_body(&serde_json::json!({
                "data": {"free_model_daily_requests": {"limit": 50, "remaining": 47}}
            })),
            Some(FreeDaily {
                limit: 50,
                remaining: 47
            })
        );
        assert_eq!(
            free_daily_from_body(&serde_json::json!({
                "data": {"free_model_daily_requests": {"limit": 50, "used": 12}}
            })),
            Some(FreeDaily {
                limit: 50,
                remaining: 38
            })
        );
        // 沒有 free_model_daily_requests（有付費額度的帳號）＝無每日上限，UI 顯示「無限」。
        assert_eq!(free_daily_from_body(&serde_json::json!({"data": {}})), None);
    }

    #[test]
    fn quota_field_is_optional_and_missing_never_means_zero() {
        assert_eq!(
            quota_remaining_from_body(&serde_json::json!({"data": {}})),
            None
        );
        assert_eq!(
            quota_remaining_from_body(&serde_json::json!({
                "data": {"free_model_daily_requests": {"remaining": 0}}
            })),
            Some(0)
        );
        assert_eq!(
            quota_remaining_from_body(&serde_json::json!({
                "data": {"free_model_daily_requests": {"limit": 50, "usage": 12}}
            })),
            Some(38)
        );
        assert_eq!(
            quota_remaining_from_body(&serde_json::json!({
                "data": {"free_model_daily_requests": "17"}
            })),
            Some(17)
        );
    }
}
