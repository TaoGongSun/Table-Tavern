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
    // 輸入只要含文字；輸出必須只有文字——會同時吐音訊／圖片的模型（例如音樂模型）不當聊天模型〔作者裁決 2026-10-04〕
    if !has_text_modality(entry, "input_modalities") || !outputs_text_only(entry) {
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

fn outputs_text_only(entry: &serde_json::Value) -> bool {
    entry
        .pointer("/architecture/output_modalities")
        .and_then(|value| value.as_array())
        .is_some_and(|modalities| {
            !modalities.is_empty()
                && modalities
                    .iter()
                    .all(|modality| modality.as_str() == Some("text"))
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

/// 穩定免費的排名來源：角色扮演排行優先，抓不到／無合格交集才退回七日熱門榜，再退其他合格穩定。
/// 回傳來源讓 UI 的推薦理由能如實標示，退回 weekly 時不會誤稱「角色扮演熱門」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StableSource {
    Roleplay,
    Weekly,
    Available,
}

/// 穩定名單（lineup）最多幾支：第 1 名是自動模式的首選，其餘依序是擁擠時的換模順序〔作者裁決 2026-10-04〕。
pub const LINEUP_SIZE: usize = 4;

/// 全部合格的穩定候選，依 RP 排行（canonical_slug）→ 七日榜（id）→ 其他合格穩定（可重現順序）排名、不重複。
/// 限時／stealth／上架未滿 7 天的模型不在其中。
pub fn stable_candidates<'a>(
    catalog: &'a [FreeModel],
    roleplay_slugs: &[String],
    weekly_ids: &[String],
    required_context: u64,
    now: u64,
) -> Vec<(&'a FreeModel, StableSource)> {
    let stable_match =
        |model: &&FreeModel| model.context_length >= required_context && is_stable(model, now);
    let mut ranked: Vec<(&FreeModel, StableSource)> = Vec::new();
    let mut push = |model: &'a FreeModel, source: StableSource| {
        if !ranked.iter().any(|(existing, _)| existing.id == model.id) {
            ranked.push((model, source));
        }
    };
    for slug in roleplay_slugs {
        if let Some(model) = catalog
            .iter()
            .find(|model| &model.canonical_slug == slug && stable_match(model))
        {
            push(model, StableSource::Roleplay);
        }
    }
    for id in weekly_ids {
        if let Some(model) = catalog
            .iter()
            .find(|model| &model.id == id && stable_match(model))
        {
            push(model, StableSource::Weekly);
        }
    }
    for model in ordered(catalog.iter().filter(stable_match)) {
        push(model, StableSource::Available);
    }
    ranked
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lineup<'a> {
    pub entries: Vec<(&'a FreeModel, StableSource)>,
    /// false＝名單含上游未知的模型，或因「同上游不相鄰」而少於能列的支數；介面要揭露降級。
    pub diversified: bool,
}

impl Lineup<'_> {
    pub fn ids(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|(model, _)| model.id.clone())
            .collect()
    }
}

fn same_upstream(
    upstreams: &std::collections::BTreeMap<String, Vec<String>>,
    left: &FreeModel,
    right: &FreeModel,
) -> bool {
    // 未知上游視為與任何模型都不同上游：可以入列、也不擋別人，但名單會標成未保證分散。
    match (upstreams.get(&left.id), upstreams.get(&right.id)) {
        (Some(left), Some(right)) => left.iter().any(|provider| right.contains(provider)),
        _ => false,
    }
}

/// 名次為主的貪婪重排：第 1 名固定是原名次第 1；之後每個位置取剩下候選中原名次最高、且與前一支
/// 不同上游的那支（A1,A2,B1,B2 → A1,B1,A2,B2）。找不到就停、少列，不為湊滿讓同上游相鄰〔作者裁決 2026-10-04〕。
pub fn build_lineup<'a>(
    ranked: &[(&'a FreeModel, StableSource)],
    upstreams: &std::collections::BTreeMap<String, Vec<String>>,
) -> Lineup<'a> {
    let mut remaining: Vec<(&FreeModel, StableSource)> = ranked.to_vec();
    let mut entries: Vec<(&FreeModel, StableSource)> = Vec::new();
    if !remaining.is_empty() {
        entries.push(remaining.remove(0));
    }
    while entries.len() < LINEUP_SIZE {
        let previous = entries.last().map(|(model, _)| *model);
        let Some(index) = remaining.iter().position(|(candidate, _)| {
            previous.is_none_or(|previous| !same_upstream(upstreams, previous, candidate))
        }) else {
            break;
        };
        entries.push(remaining.remove(index));
    }
    // 少於 4 支一律標降級（候選本來就不夠也算）：介面不宣稱已分散到 4 家上游
    let constrained = entries.len() < LINEUP_SIZE;
    let unknown = entries
        .iter()
        .any(|(model, _)| upstreams.get(&model.id).is_none_or(Vec::is_empty));
    Lineup {
        diversified: !constrained && !unknown,
        entries,
    }
}

/// 名單世代：名單 id 依序的雜湊；名單任何變動（換人、換順序）都會變。
pub fn lineup_gen(ids: &[String]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for id in ids {
        for byte in id.as_bytes().iter().chain(std::iter::once(&0u8)) {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("{hash:016x}")
}

/// 限時／實驗性推薦：stealth 或有到期日、尚未到期的免費模型。只供玩家明確選擇，不進自動路徑。
pub fn limited_models(catalog: &[FreeModel], required_context: u64, now: u64) -> Vec<&FreeModel> {
    ordered(catalog.iter().filter(|model| {
        model.context_length >= required_context
            && model.expiration_at.is_none_or(|expires| expires > now)
            && high_upside(model)
    }))
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
             "architecture":{"input_modalities":["text","image"],"output_modalities":["text"]}},
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

    /// OpenRouter 官方 `/models` 2026-10-04 的 lyria 欄位原樣：輸出含音訊的音樂模型不得進任何候選。
    #[test]
    fn music_models_that_also_output_audio_are_excluded() {
        let body = serde_json::json!({"data": [
            {"id":"google/lyria-3-pro-preview","canonical_slug":"google/lyria-3-pro-preview",
             "name":"Google: Lyria 3 Pro Preview","created":1774907286,"context_length":1048576,
             "pricing":{"prompt":"0","completion":"0"},
             "architecture":{"modality":"text+image->text+audio","input_modalities":["text","image"],
                             "output_modalities":["text","audio"],"tokenizer":"Other","instruct_type":null},
             "expiration_date":null},
            {"id":"google/lyria-3-clip-preview","canonical_slug":"google/lyria-3-clip-preview",
             "name":"Google: Lyria 3 Clip Preview","created":1774907255,"context_length":1048576,
             "pricing":{"prompt":"0","completion":"0"},
             "architecture":{"modality":"text+image->text+audio","input_modalities":["text","image"],
                             "output_modalities":["text","audio"],"tokenizer":"Other","instruct_type":null},
             "expiration_date":null},
            {"id":"x/empty-output:free","created":100,"context_length":100000,
             "pricing":{"prompt":"0","completion":"0"},
             "architecture":{"input_modalities":["text"],"output_modalities":[]}},
            {"id":"x/chat:free","created":100,"context_length":100000,
             "pricing":{"prompt":"0","completion":"0"},
             "architecture":{"input_modalities":["text","image"],"output_modalities":["text"]}}
        ]});
        let parsed = parse_catalog(&body).unwrap();
        assert_eq!(
            parsed
                .iter()
                .map(|model| model.id.as_str())
                .collect::<Vec<_>>(),
            ["x/chat:free"]
        );
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

    fn ids(entries: &[(&FreeModel, StableSource)]) -> Vec<String> {
        entries.iter().map(|(model, _)| model.id.clone()).collect()
    }

    #[test]
    fn stable_candidates_rank_roleplay_then_weekly_then_available_and_skip_limited() {
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
            model("stable/new:free", now - 86_400, 64_000, None),
            model("stable/weekly:free", old - 1, 64_000, None),
            model("stable/third:free", old - 2, 64_000, None),
            roleplay,
        ];
        let ranked = stable_candidates(
            &models,
            &["stable/rp-base".to_owned()],
            &["stable/weekly:free".to_owned()],
            4096,
            now,
        );
        assert_eq!(
            ranked
                .iter()
                .map(|(model, source)| (model.id.as_str(), *source))
                .collect::<Vec<_>>(),
            [
                ("stable/rp:free", StableSource::Roleplay),
                ("stable/weekly:free", StableSource::Weekly),
                ("stable/third:free", StableSource::Available),
            ]
        );
        // 放不下最小長度的穩定模型不入列
        assert!(stable_candidates(&models, &[], &[], 100_000, now).is_empty());
    }

    fn upstreams(pairs: &[(&str, &str)]) -> std::collections::BTreeMap<String, Vec<String>> {
        pairs
            .iter()
            .map(|(id, provider)| ((*id).to_owned(), vec![(*provider).to_owned()]))
            .collect()
    }

    #[test]
    fn lineup_reorders_so_same_upstream_is_never_adjacent() {
        let models: Vec<FreeModel> = ["a1", "a2", "b1", "b2"]
            .iter()
            .map(|id| model(id, 1, 64_000, None))
            .collect();
        let ranked: Vec<_> = models.iter().map(|m| (m, StableSource::Roleplay)).collect();
        let map = upstreams(&[("a1", "A"), ("a2", "A"), ("b1", "B"), ("b2", "B")]);
        let lineup = build_lineup(&ranked, &map);
        assert_eq!(ids(&lineup.entries), ["a1", "b1", "a2", "b2"]);
        assert!(lineup.diversified);
    }

    #[test]
    fn lineup_lists_fewer_instead_of_placing_same_upstream_adjacent() {
        let models: Vec<FreeModel> = ["a1", "b1", "b2", "b3"]
            .iter()
            .map(|id| model(id, 1, 64_000, None))
            .collect();
        let ranked: Vec<_> = models.iter().map(|m| (m, StableSource::Roleplay)).collect();
        let map = upstreams(&[("a1", "A"), ("b1", "B"), ("b2", "B"), ("b3", "B")]);
        let lineup = build_lineup(&ranked, &map);
        assert_eq!(ids(&lineup.entries), ["a1", "b1"]);
        assert!(!lineup.diversified);
    }

    #[test]
    fn unknown_upstream_never_blocks_but_marks_lineup_not_diversified() {
        let models: Vec<FreeModel> = ["a1", "u1", "a2", "u2"]
            .iter()
            .map(|id| model(id, 1, 64_000, None))
            .collect();
        let ranked: Vec<_> = models.iter().map(|m| (m, StableSource::Weekly)).collect();
        let map = upstreams(&[("a1", "A"), ("a2", "A")]);
        let lineup = build_lineup(&ranked, &map);
        assert_eq!(ids(&lineup.entries), ["a1", "u1", "a2", "u2"]);
        assert!(!lineup.diversified);
        // 兩支都有多個上游時，交集即同上游
        let mut multi = upstreams(&[("a1", "A")]);
        multi.insert("u1".to_owned(), vec!["C".to_owned(), "A".to_owned()]);
        multi.insert("a2".to_owned(), vec!["D".to_owned()]);
        multi.insert("u2".to_owned(), vec!["E".to_owned()]);
        assert_eq!(
            ids(&build_lineup(&ranked, &multi).entries),
            ["a1", "a2", "u1", "u2"]
        );
    }

    #[test]
    fn fewer_than_four_entries_is_always_a_degradation() {
        let models = [model("a1", 1, 64_000, None), model("b1", 1, 64_000, None)];
        let ranked: Vec<_> = models
            .iter()
            .map(|m| (m, StableSource::Available))
            .collect();
        let lineup = build_lineup(&ranked, &upstreams(&[("a1", "A"), ("b1", "B")]));
        assert_eq!(ids(&lineup.entries), ["a1", "b1"]);
        assert!(!lineup.diversified);
        let empty = build_lineup(&[], &Default::default());
        assert!(empty.entries.is_empty());
        assert!(!empty.diversified);
    }

    #[test]
    fn lineup_gen_changes_with_membership_and_order() {
        let ab = lineup_gen(&["a".to_owned(), "b".to_owned()]);
        assert_eq!(ab, lineup_gen(&["a".to_owned(), "b".to_owned()]));
        assert_ne!(ab, lineup_gen(&["b".to_owned(), "a".to_owned()]));
        assert_ne!(ab, lineup_gen(&["ab".to_owned()]));
    }

    #[test]
    fn limited_models_keep_live_high_upside_even_inside_auto_margin() {
        let now = 2_000_000_000;
        let models = vec![
            model("stealth/new", now - 10, 256_000, None),
            model("preview/one:free", now - 20, 128_000, Some(now + 3600)),
            model("preview/expired:free", now - 30, 128_000, Some(now - 1)),
            model("stable/top:free", now - 8 * 86_400, 64_000, None),
        ];
        assert_eq!(
            limited_models(&models, 4096, now)
                .iter()
                .map(|model| model.id.as_str())
                .collect::<Vec<_>>(),
            ["stealth/new", "preview/one:free"]
        );
        // 自動選模的 24 小時邊界不影響推薦清單
        assert!(eligible_models(&models[1..2], 4096, now, &[]).is_empty());
    }

    #[test]
    fn iso_dates_parse_as_utc() {
        let date = parse_iso_utc("1970-01-02").unwrap();
        let timestamp = parse_iso_utc("1970-01-02T01:02:03Z").unwrap();
        assert_eq!(date, 86_400);
        assert_eq!(timestamp, 86_400 + 3_600 + 120 + 3);
    }
}
