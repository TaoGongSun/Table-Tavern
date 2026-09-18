use super::select;
use crate::transport::DEFAULT_BASE_URL;
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
        .get(format!("{DEFAULT_BASE_URL}/models/user"))
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
        .get(format!("{DEFAULT_BASE_URL}/models?sort=top-weekly"))
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
        .get(format!("{DEFAULT_BASE_URL}/models?category=roleplay"))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    select::parse_ranked_slugs(&response.json().await.ok()?)
}

/// 帳號免費模型的每日請求額度（所有 `:free` 模型共用一個池）。
#[derive(Debug, Clone, Copy, serde::Serialize, PartialEq, Eq)]
pub struct FreeDaily {
    pub limit: i64,
    pub remaining: i64,
}

pub async fn fetch_free_daily(client: &reqwest::Client, api_key: &str) -> Option<FreeDaily> {
    let response = client
        .get(format!("{DEFAULT_BASE_URL}/key"))
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
        .get(format!("{DEFAULT_BASE_URL}/key"))
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
