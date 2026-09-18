use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

pub const RESERVED_OUTPUT_TOKENS: u64 = 4_096;
const EXPIRY_MARGIN_SECS: u64 = 24 * 60 * 60;
const STABLE_AGE_SECS: u64 = 7 * 24 * 60 * 60;

/// `/models/user` 中，智慧免費選模真正需要的欄位。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreeModel {
    pub id: String,
    /// 免費版與付費版共用的模型代號，用來對上角色扮演排行（排行走 base id）。
    #[serde(default)]
    pub canonical_slug: String,
    pub name: String,
    pub created: u64,
    pub context_length: u64,
    #[serde(default)]
    pub expiration_at: Option<u64>,
    #[serde(default)]
    pub supported_parameters: Vec<String>,
}

/// 壞回應回 None，讓呼叫端保留上一份好快取；空 data 是合法的「這個帳號目前沒有模型」。
pub fn parse_catalog(body: &serde_json::Value) -> Option<Vec<FreeModel>> {
    let data = body.get("data")?.as_array()?;
    Some(data.iter().filter_map(parse_model).collect())
}

/// `?sort=top-weekly` 已由 OpenRouter 排好順序；只存 id 就足夠做交集。
pub fn parse_ranked_ids(body: &serde_json::Value) -> Option<Vec<String>> {
    let data = body.get("data")?.as_array()?;
    Some(
        data.iter()
            .filter_map(|entry| entry.get("id")?.as_str().map(str::to_owned))
            .collect(),
    )
}

/// `?category=roleplay` 依角色扮演使用度排好順序；排行走 base id，用 canonical_slug 才能對上免費版。
pub fn parse_ranked_slugs(body: &serde_json::Value) -> Option<Vec<String>> {
    let data = body.get("data")?.as_array()?;
    Some(
        data.iter()
            .filter_map(|entry| {
                let slug = entry
                    .get("canonical_slug")
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|slug| !slug.is_empty());
                slug.or_else(|| entry.get("id").and_then(|value| value.as_str()))
                    .map(str::to_owned)
            })
            .collect(),
    )
}

fn parse_model(entry: &serde_json::Value) -> Option<FreeModel> {
    let id = entry.get("id")?.as_str()?.trim();
    if id.is_empty() {
        return None;
    }
    let pricing = entry.get("pricing")?;
    // OpenRouter 只列出實際計費的維度：缺鍵＝該維度不計費（等同 0）。
    // 免費模型普遍沒有 request 鍵，硬要三鍵齊全會把所有模型都篩掉。
    if !["prompt", "completion", "request"]
        .iter()
        .all(|key| pricing.get(*key).map_or(true, is_zero_price))
    {
        return None;
    }
    if !has_text_modality(entry, "input_modalities")
        || !has_text_modality(entry, "output_modalities")
    {
        return None;
    }

    let expiration_at = match entry.get("expiration_date") {
        None | Some(serde_json::Value::Null) => None,
        Some(value) => Some(parse_timestamp(value)?),
    };
    let supported_parameters = entry
        .get("supported_parameters")
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();

    Some(FreeModel {
        id: id.to_owned(),
        // canonical_slug 缺失／null／空字串都退回 id，否則空字串會被誤判成 schema 未升級而反覆重抓。
        canonical_slug: entry
            .get("canonical_slug")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|slug| !slug.is_empty())
            .unwrap_or(id)
            .to_owned(),
        name: entry
            .get("name")
            .and_then(|value| value.as_str())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(id)
            .to_owned(),
        created: entry.get("created").and_then(parse_timestamp).unwrap_or(0),
        context_length: entry
            .get("context_length")
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
        expiration_at,
        supported_parameters,
    })
}

fn is_zero_price(value: &serde_json::Value) -> bool {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse::<f64>().ok())
        .is_some_and(|price| price == 0.0)
}

fn has_text_modality(entry: &serde_json::Value, field: &str) -> bool {
    entry
        .pointer(&format!("/architecture/{field}"))
        .and_then(|value| value.as_array())
        .is_some_and(|modalities| {
            modalities
                .iter()
                .any(|modality| modality.as_str() == Some("text"))
        })
}

fn parse_timestamp(value: &serde_json::Value) -> Option<u64> {
    if let Some(timestamp) = value.as_u64() {
        return Some(timestamp);
    }
    let raw = value.as_str()?.trim();
    if let Ok(timestamp) = raw.parse::<u64>() {
        return Some(timestamp);
    }
    parse_iso_utc(raw)
}

/// OpenRouter 的日期欄位目前是 UTC ISO 字串；只需支援日期與常見的 Z 時間戳。
fn parse_iso_utc(raw: &str) -> Option<u64> {
    let date = raw.get(..10)?;
    let mut parts = date.split('-');
    let year = parts.next()?.parse::<i64>().ok()?;
    let month = parts.next()?.parse::<i64>().ok()?;
    let day = parts.next()?.parse::<i64>().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let mut seconds = days_from_civil(year, month, day).checked_mul(86_400)?;
    if raw.len() > 10 {
        let time = raw.get(11..19)?;
        let mut time_parts = time.split(':');
        let hour = time_parts.next()?.parse::<i64>().ok()?;
        let minute = time_parts.next()?.parse::<i64>().ok()?;
        let second = time_parts.next()?.parse::<i64>().ok()?;
        if hour > 23 || minute > 59 || second > 60 {
            return None;
        }
        seconds = seconds.checked_add(hour * 3_600 + minute * 60 + second)?;
    }
    u64::try_from(seconds).ok()
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

pub fn eligible_models(
    catalog: &[FreeModel],
    required_context: u64,
    now: u64,
    required_parameters: &[&str],
) -> Vec<FreeModel> {
    catalog
        .iter()
        .filter(|model| model.context_length >= required_context)
        .filter(|model| {
            model
                .expiration_at
                .is_none_or(|expires| expires > now.saturating_add(EXPIRY_MARGIN_SECS))
        })
        .filter(|model| {
            required_parameters.iter().all(|required| {
                model
                    .supported_parameters
                    .iter()
                    .any(|parameter| parameter == required)
            })
        })
        .cloned()
        .collect()
}

fn high_upside(model: &FreeModel) -> bool {
    model.id.starts_with("stealth/") || model.expiration_at.is_some()
}

pub fn model_namespace(model: &FreeModel) -> &str {
    model.id.split_once('/').map_or("", |(owner, _)| owner)
}

fn preference_cmp(left: &FreeModel, right: &FreeModel) -> Ordering {
    let left_day = left.created / 86_400;
    let right_day = right.created / 86_400;
    right_day
        .cmp(&left_day)
        .then_with(|| right.context_length.cmp(&left.context_length))
        .then_with(|| left.id.cmp(&right.id))
}

fn ordered<'a>(models: impl Iterator<Item = &'a FreeModel>) -> Vec<&'a FreeModel> {
    let mut models: Vec<&FreeModel> = models.collect();
    models.sort_by(|left, right| preference_cmp(left, right));
    models
}

fn is_stable(model: &FreeModel, now: u64) -> bool {
    !model.id.starts_with("stealth/")
        && model.expiration_at.is_none()
        && model.created > 0
        && model.created <= now.saturating_sub(STABLE_AGE_SECS)
}

fn stable_ranked<'a>(
    models: &'a [FreeModel],
    weekly_ids: &[String],
    now: u64,
) -> Vec<&'a FreeModel> {
    weekly_ids
        .iter()
        .filter_map(|id| {
            models
                .iter()
                .find(|model| &model.id == id && is_stable(model, now))
        })
        .collect()
}

/// 穩定免費保底的排名來源：角色扮演排行優先，抓不到／無合格交集才退回七日熱門榜。
/// 回傳來源讓 UI 的推薦理由能如實標示，退回 weekly 時不會誤稱「角色扮演熱門」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StableSource {
    Roleplay,
    Weekly,
    Available,
}

/// 推薦清單與自動選模的到期策略不同：自動冷啟動不綁 24 小時內要到期的模型，
/// 但玩家明確看到並手動選擇的限時模型，只要尚未到期就仍值得列出。
///
/// 穩定免費保底照角色扮演排行（`?category=roleplay`，canonical_slug 對上免費版）挑最高一支；
/// 排行抓不到、或排行裡沒有任何合格免費常駐模型時，退回七日熱門榜（weekly，走 id）。
/// 穩定免費畫面顯示的支數：只送第一名，第二名供玩家在第一名當天失效時手動改用
/// （兩支同時掛掉的機率遠低於一支，基本避免「當天不能玩」）〔作者裁決 2026-09-18〕。
pub const STABLE_RECOMMENDATION_COUNT: usize = 2;

pub fn recommendation_models<'a>(
    catalog: &'a [FreeModel],
    roleplay_slugs: &[String],
    weekly_ids: &[String],
    required_context: u64,
    now: u64,
) -> (Vec<&'a FreeModel>, Vec<(&'a FreeModel, StableSource)>) {
    let usable = |model: &&FreeModel| {
        model.context_length >= required_context
            && model.expiration_at.is_none_or(|expires| expires > now)
    };
    let limited = ordered(
        catalog
            .iter()
            .filter(usable)
            .filter(|model| high_upside(model)),
    );
    let stable_match = |model: &&FreeModel| {
        model.context_length >= required_context
            && model.expiration_at.is_none_or(|expires| expires > now)
            && is_stable(model, now)
    };
    // 前兩名照角色扮演排行 → 七日榜 → 其他合格穩定的順序取，不重複。
    let mut stable: Vec<(&FreeModel, StableSource)> = Vec::new();
    for slug in roleplay_slugs {
        if let Some(model) = catalog
            .iter()
            .find(|model| &model.canonical_slug == slug && stable_match(model))
        {
            push_stable(&mut stable, model, StableSource::Roleplay);
        }
        if stable.len() == STABLE_RECOMMENDATION_COUNT {
            break;
        }
    }
    if stable.len() < STABLE_RECOMMENDATION_COUNT {
        for id in weekly_ids {
            if let Some(model) = catalog
                .iter()
                .find(|model| &model.id == id && stable_match(model))
            {
                push_stable(&mut stable, model, StableSource::Weekly);
            }
            if stable.len() == STABLE_RECOMMENDATION_COUNT {
                break;
            }
        }
    }
    if stable.len() < STABLE_RECOMMENDATION_COUNT {
        for model in ordered(catalog.iter().filter(stable_match)) {
            push_stable(&mut stable, model, StableSource::Available);
            if stable.len() == STABLE_RECOMMENDATION_COUNT {
                break;
            }
        }
    }
    (limited, stable)
}

fn push_stable<'a>(
    out: &mut Vec<(&'a FreeModel, StableSource)>,
    model: &'a FreeModel,
    source: StableSource,
) {
    if out.len() < STABLE_RECOMMENDATION_COUNT
        && !out.iter().any(|(existing, _)| existing.id == model.id)
    {
        out.push((model, source));
    }
}

fn push_unique(result: &mut Vec<String>, model: &FreeModel) {
    if result.len() < 3 && !result.iter().any(|id| id == &model.id) {
        result.push(model.id.clone());
    }
}

/// 動態穩定免費每次送出都依最新排名重建：RP 穩定榜優先，其次七日榜，
/// 再補其他合格穩定免費。限時／stealth 模型不會被自動塞進備援；它們只在玩家
/// 明確點選後才固定使用。最多三支、不重複。
pub fn stable_fallback_models(
    models: &[FreeModel],
    roleplay_slugs: &[String],
    weekly_ids: &[String],
    now: u64,
) -> Vec<String> {
    let mut result = Vec::new();

    for slug in roleplay_slugs {
        if let Some(model) = models
            .iter()
            .find(|model| &model.canonical_slug == slug && is_stable(model, now))
        {
            push_unique(&mut result, model);
        }
        if result.len() == 3 {
            return result;
        }
    }

    for model in stable_ranked(models, weekly_ids, now) {
        push_unique(&mut result, model);
        if result.len() == 3 {
            return result;
        }
    }

    for model in ordered(models.iter().filter(|model| is_stable(model, now))) {
        push_unique(&mut result, model);
        if result.len() == 3 {
            return result;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(id: &str, created: u64, context: u64, expiration_at: Option<u64>) -> FreeModel {
        FreeModel {
            id: id.to_owned(),
            canonical_slug: id.to_owned(),
            name: id.to_owned(),
            created,
            context_length: context,
            expiration_at,
            supported_parameters: Vec::new(),
        }
    }

    #[test]
    fn catalog_accepts_zero_or_absent_prices_and_requires_text_io() {
        let body = serde_json::json!({"data": [
            {"id":"stealth/new","name":"Stealth","created":100,"context_length":100000,
             "pricing":{"prompt":"0","completion":"0","request":"0"},
             "architecture":{"input_modalities":["text"],"output_modalities":["text","audio"]}},
            // 真實情況：OpenRouter 免費模型只列 prompt/completion，沒有 request 鍵。
            {"id":"x/no-request:free","created":100,"context_length":100000,
             "pricing":{"prompt":"0","completion":"0"},
             "architecture":{"input_modalities":["text"],"output_modalities":["text"]}},
            {"id":"x/request-paid:free","created":100,"context_length":100000,
             "pricing":{"prompt":"0","completion":"0","request":"0.1"},
             "architecture":{"input_modalities":["text"],"output_modalities":["text"]}},
            {"id":"x/no-text:free","created":100,"context_length":100000,
             "pricing":{"prompt":"0","completion":"0","request":"0"},
             "architecture":{"input_modalities":["image"],"output_modalities":["text"]}}
        ]});
        let parsed = parse_catalog(&body).unwrap();
        let ids: Vec<_> = parsed.iter().map(|model| model.id.as_str()).collect();
        assert_eq!(ids, ["stealth/new", "x/no-request:free"]);
        assert!(parse_catalog(&serde_json::json!({"error":"down"})).is_none());
    }

    #[test]
    fn eligibility_checks_context_expiry_margin_and_required_parameters() {
        let now = 2_000_000_000;
        let mut okay = model("x/okay:free", now - 100, 20_000, None);
        okay.supported_parameters = vec!["temperature".to_owned()];
        let mut expiring = model(
            "x/soon:free",
            now - 100,
            20_000,
            Some(now + EXPIRY_MARGIN_SECS),
        );
        expiring.supported_parameters = vec!["temperature".to_owned()];
        let short = model("x/short:free", now - 100, 5_000, None);
        assert_eq!(
            eligible_models(&[okay, expiring, short], 10_000, now, &["temperature"])
                .into_iter()
                .map(|model| model.id)
                .collect::<Vec<_>>(),
            ["x/okay:free"]
        );
    }

    #[test]
    fn stable_fallback_prefers_roleplay_then_weekly_and_never_uses_limited_models() {
        let now = 2_000_000_000;
        let old = now - 8 * 86_400;
        let mut roleplay = model("stable/rp:free", old, 64_000, None);
        roleplay.canonical_slug = "stable/rp-base".to_owned();
        let models = vec![
            model("stealth/limited", now - 10, 256_000, None),
            model(
                "preview/limited:free",
                now - 20,
                256_000,
                Some(now + 200_000),
            ),
            model("stable/weekly:free", old - 1, 64_000, None),
            model("stable/third:free", old - 2, 64_000, None),
            roleplay,
        ];
        let weekly = vec![
            "stable/weekly:free".to_owned(),
            "stable/third:free".to_owned(),
        ];
        assert_eq!(
            stable_fallback_models(&models, &["stable/rp-base".to_owned()], &weekly, now),
            ["stable/rp:free", "stable/weekly:free", "stable/third:free"]
        );
    }

    #[test]
    fn stable_fallback_shrinks_instead_of_auto_using_limited_models() {
        let now = 2_000_000_000;
        let old = now - 8 * 86_400;
        let models = vec![
            model("stable/one:free", old, 64_000, None),
            model("stealth/limited", now - 10, 256_000, None),
            model(
                "preview/limited:free",
                now - 20,
                256_000,
                Some(now + 200_000),
            ),
        ];
        assert_eq!(
            stable_fallback_models(&models, &[], &["stable/one:free".to_owned()], now),
            ["stable/one:free"]
        );
    }

    #[test]
    fn recommendations_list_top_two_stable_plus_all_live_high_upside_weekly_fallback() {
        let now = 2_000_000_000;
        let old = now - 8 * 86_400;
        let models = vec![
            model("stealth/new", now - 10, 256_000, None),
            model("preview/one:free", now - 20, 128_000, Some(now + 3600)),
            model("preview/expired:free", now - 30, 128_000, Some(now - 1)),
            model("stable/top:free", old, 64_000, None),
            model("stable/second:free", old - 1, 64_000, None),
        ];
        let weekly = vec![
            "stable/top:free".to_owned(),
            "stable/second:free".to_owned(),
        ];
        // 角色扮演排行抓不到時退回七日榜，來源標 Weekly；穩定區列前兩名。
        let (limited, stable) = recommendation_models(&models, &[], &weekly, 4096, now);
        assert_eq!(
            limited
                .iter()
                .map(|model| model.id.as_str())
                .collect::<Vec<_>>(),
            ["stealth/new", "preview/one:free"]
        );
        assert_eq!(
            stable
                .iter()
                .map(|(model, source)| (model.id.as_str(), *source))
                .collect::<Vec<_>>(),
            [
                ("stable/top:free", StableSource::Weekly),
                ("stable/second:free", StableSource::Weekly)
            ]
        );
    }

    #[test]
    fn stable_recommendation_prefers_roleplay_rank_over_weekly() {
        let now = 2_000_000_000;
        let old = now - 8 * 86_400;
        let mut rp_pick = model("vendor/rp-star:free", old, 64_000, None);
        rp_pick.canonical_slug = "vendor/rp-star-20260101".to_owned();
        let mut weekly_pick = model("vendor/weekly-star:free", old, 64_000, None);
        weekly_pick.canonical_slug = "vendor/weekly-star-20260101".to_owned();
        let models = vec![weekly_pick, rp_pick];
        // 角色扮演排行走 canonical_slug；即使七日榜把 weekly-star 排前面，也以 RP 首選為準。
        let roleplay = vec!["vendor/rp-star-20260101".to_owned()];
        let weekly = vec![
            "vendor/weekly-star:free".to_owned(),
            "vendor/rp-star:free".to_owned(),
        ];
        let (_, stable) = recommendation_models(&models, &roleplay, &weekly, 4096, now);
        assert_eq!(
            stable
                .first()
                .map(|(model, source)| (model.id.as_str(), *source)),
            Some(("vendor/rp-star:free", StableSource::Roleplay))
        );
    }

    #[test]
    fn recommendation_keeps_models_inside_the_auto_binding_24h_margin() {
        let now = 2_000_000_000;
        let soon = model("preview/soon:free", now - 10, 128_000, Some(now + 3600));
        assert!(eligible_models(&[soon.clone()], 4096, now, &[]).is_empty());
        let models = [soon];
        let (limited, _) = recommendation_models(&models, &[], &[], 4096, now);
        assert_eq!(limited.len(), 1);
    }

    #[test]
    fn iso_dates_parse_as_utc() {
        let date = parse_iso_utc("1970-01-02").unwrap();
        let timestamp = parse_iso_utc("1970-01-02T01:02:03Z").unwrap();
        assert_eq!(date, 86_400);
        assert_eq!(timestamp, 86_400 + 3_600 + 120 + 3);
    }
}
