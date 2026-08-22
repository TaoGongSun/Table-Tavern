//! 額度分頁（prompt-cache-optimization 包 6）：把 `prompt-cache.jsonl` 彙總成
//! 「一桌一份、桌內按模型分行」的報表給設定頁顯示。
//!
//! 兩條規矩：
//! - **token 是主軸**：四家 CLI 的 token 語意已在 cli.rs 換算成同一把尺，可以直接相加比較。
//! - **第一眼只講省下多少**：收合處出「已省幾成、省了多少錢」（總花費看了只會焦慮），
//!   花費留在細項——那裡有保溫 ping 這種本來就沒有「省下」可言的列。
//! - **金額只轉述**：app 不自算牌價、不建價目表，`cost_usd` 照舊是各 CLI 自己回報值的加總；
//!   省下的錢再拿它反推該輪的輸入單價乘回省下的 token，估不出的輪次標 `saved_partial`。
//!
//! 診斷標籤與原因代碼原樣送到前端配 i18n（字典見 usage_log.rs 模組頂註解）。

use serde::Serialize;
use serde_json::Value;

/// 下拉選單的一項：app 現有的每一桌都在（沒紀錄就 0 輪）。
/// 另有兩種沒有名字的項目——`id` 空字串＝開桌生成（那時桌還沒建出來），
/// `id` 有值但 `name` 空＝這桌已經被刪掉，紀錄還在。
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WorldOption {
    pub id: String,
    pub name: String,
    pub rounds: u64,
}

/// 一列小計。總計列的 `source`／`model` 為空字串。
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct UsageRow {
    pub source: String,
    pub model: String,
    pub rounds: u64,
    pub prompt_tokens: u64,
    pub cached_tokens: u64,
    pub output_tokens: u64,
    /// 命中率；這一列沒有任何可判讀輪次＝None（前端顯示「—」，不是 0%）
    pub hit_rate: Option<f64>,
    /// 供應商有回報快取欄位的輪數。少於 rounds 時前端加註「可判讀 N/M 輪」
    pub observed_rounds: u64,
    /// 上面那些輪次的輸入量＝命中率的分母。量不到的輪次不進來，才不會稀釋命中率
    pub observed_prompt_tokens: u64,
    /// 各 CLI 官方回報值的加總；一筆都沒有＝None（前端顯示「—」）
    pub cost_usd: Option<f64>,
    /// 有輪次沒回報金額，加總只是部分
    pub cost_partial: bool,
    /// 快取省下的輸入等值 token：沒快取要付的（全額）減掉實際付的
    pub saved_tokens: f64,
    /// 上面那筆的分母：算得出省下多少的輸入 token（計價不明的來源不進來）
    pub priced_tokens: u64,
    /// 省下的錢；一筆都估不出＝None（前端顯示「—」）
    pub saved_usd: Option<f64>,
    /// 有輪次估不出（沒回報金額或計價不明），加總只是部分
    pub saved_partial: bool,
    /// 完全不回報用量的輪數（agy）
    pub unreported: u64,
    /// 這個模型是目前設定在用的
    pub in_use: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CacheCount {
    pub cache: String,
    pub rounds: u64,
}

/// 統計用的分類：把「快取結果」與「沒中的原因」壓成一個玩家看得懂的格子。
/// `missed` 是唯一該亮紅燈的——它代表**證明得了照理該中**（算得出理論可中量、且沒中滿），
/// 其餘一律不是故障：`zero` 只是這輪沒省到、`expired` 只代表超過 app 的保守窗口。
fn chip_state(cache: &str, cache_reason: Option<&str>) -> String {
    let faulty = matches!(cache_reason, Some("below-expected") | Some("skipped"));
    match (cache, faulty) {
        ("partial" | "zero", true) => "missed".to_owned(),
        _ => cache.to_owned(),
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LatestCall {
    pub ts: String,
    /// 這通送出去的形狀；舊行推不出來就是 None（前端不顯示，不猜）
    pub mode: Option<String>,
    /// 快取結果；線事件沒有這個東西，改由 `event` 說明
    pub cache: Option<String>,
    pub cache_reason: Option<String>,
    /// 線事件種類（目前只有 drop-lane）；一般呼叫是 None
    pub event: Option<String>,
    /// 線為什麼重開／丟線，與 cache_reason 不同軸
    pub reason: Option<String>,
    /// 這一輪的快取數字看不看得見；false＝前端要在診斷句旁講明「沒回報快取資料」
    pub reported: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UsageReport {
    /// 全檔有紀錄的桌（不受選桌影響，供下拉用）
    pub worlds: Vec<WorldOption>,
    pub rows: Vec<UsageRow>,
    pub total: UsageRow,
    /// 保溫 ping 小計：不推進劇情，與劇情輪分開算
    pub ping: UsageRow,
    /// 快取結果統計。事件行（丟線重來）沒有快取結果，不進這個分母
    pub caches: Vec<CacheCount>,
    /// 線事件的次數（目前只有丟線重來）
    pub events: u64,
    /// 最近一筆非保溫紀錄，供燈號與原因句
    pub latest: Option<LatestCall>,
}

/// 一行帳本 →（形狀, 快取結果, 沒中的原因）。新行直接讀 `mode`／`cache`；
/// 舊行只有一欄兩用的 `diag`，但**續聊線的舊行帶著 `expected_cached`／`age_secs`**，
/// 拿它們套同一條規則就好，不必查 diag 對照表——那張表當年正是把數字蓋掉的元凶
/// （帳本實測：11 筆 `warmup` 中了六到九成、10 筆 `expired` 中了八成以上）。
/// 形狀回推不出來就交白卷（None），不拿「單發」冒充。
fn classify(line: &Value) -> (Option<String>, String, Option<String>) {
    if let Some(cache) = line.get("cache").and_then(Value::as_str) {
        return (
            line.get("mode").and_then(Value::as_str).map(str::to_owned),
            cache.to_owned(),
            line.get("cache_reason")
                .and_then(Value::as_str)
                .map(str::to_owned),
        );
    }
    // 舊行只有 ping 的形狀是自明的，其餘一律不知道
    let mode = match text(line, "diag").as_str() {
        "ping" => Some("ping".to_owned()),
        _ => None,
    };
    // 先問看不看得見再問中沒中：加 `cache_reporting` 欄之前的 api 行寫過壓平的假 0
    // （見 .ai/plans/api-cache-visibility.md），把它當真 0 會與同頁「—」的命中率打架
    let cached = line.get("cached_tokens").and_then(Value::as_u64);
    let (Some(cached), true) = (cached, reported(line)) else {
        return (mode, "unknown".to_owned(), None);
    };
    // 沒有 lane 欄＝無狀態呼叫，本來就算不出理論可中量，只憑數字說話
    if line.get("lane").is_none() {
        let cache = match cached > 0 {
            true => "hit",
            false => "zero",
        };
        return (mode, cache.to_owned(), None);
    }
    let created = line.get("created_tokens").and_then(Value::as_u64);
    // 舊的續聊線行：reason 在＝那輪重開了線，我方內容本來就不該中
    let expected = match line.get("reason").is_some() {
        true => 0,
        false => number(line, "expected_cached"),
    };
    let short = match number(line, "age_secs") > crate::lanes::CACHE_TTL_SECS {
        true => "expired",
        false => "below-expected",
    };
    if cached > 0 {
        if expected == 0 || cached * 10 >= expected * 9 {
            return (mode, "hit".to_owned(), None);
        }
        return (mode, "partial".to_owned(), Some(short.to_owned()));
    }
    if expected == 0 {
        return (mode, "not-expected".to_owned(), None);
    }
    let reason = match created {
        Some(0) => "skipped",
        _ => short,
    };
    (mode, "zero".to_owned(), Some(reason.to_owned()))
}

fn number(line: &Value, key: &str) -> u64 {
    line.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn text(line: &Value, key: &str) -> String {
    line.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn hit_rate(row: &UsageRow) -> Option<f64> {
    if row.observed_rounds == 0 {
        return None; // 一輪都量不到＝命中率不存在，不是 0
    }
    if row.observed_prompt_tokens == 0 {
        return Some(0.0);
    }
    let rate = row.cached_tokens as f64 * 100.0 / row.observed_prompt_tokens as f64;
    Some((rate * 10.0).round() / 10.0)
}

/// 這一輪的快取數字看不看得見。舊行沒有 `cache_reporting` 欄位，靠兩條規則還原：
/// - api：當年只讀 OpenRouter 的 `prompt_tokens_details.cached_tokens`，其他 schema
///   一律被 `unwrap_or(0)` 捏成 0。**捏得出來的只有 0**——所以 >0 必是讀對欄位的真值
///   （那時真的連著 OpenRouter），採信；記 0 的分不出真假，保守當量不到。
/// - CLI 三路：欄位一直是對的，照舊採信。
fn reported(line: &Value) -> bool {
    if line.get("unreported").and_then(Value::as_bool) == Some(true) {
        return false; // 連 token 都不回報的來源（agy），快取自然更無從得知
    }
    match line.get("cache_reporting").and_then(Value::as_str) {
        Some(value) => value == "reported",
        None => text(line, "transport") != "api" || number(line, "cached_tokens") > 0,
    }
}

/// 快取計價係數（相對一般輸入價）：`(讀快取, 寫快取)`。讀便宜、寫（Anthropic）反而貴，
/// 兩邊都算進去才不會把「省下多少」灌水。沒列到的來源不估——agy 完全不回報用量，
/// api 走哪個模型不定。
fn cache_price(transport: &str) -> Option<(f64, f64)> {
    match transport {
        "claude" => Some((0.1, 1.25)), // Anthropic：讀一折、寫加價兩成半
        "codex" => Some((0.1, 1.0)),   // OpenAI：讀一折、寫不加價
        "grok" => Some((0.5, 1.0)),    // xAI 折扣落在五到七五折，取最保守的那頭
        _ => None,
    }
}

/// 輸出價是輸入價的幾倍（Anthropic 與 xAI 全系列都是 5 倍）。
/// 用途：把 CLI 回報的整輪金額拆回「一個輸入 token 值多少錢」，才不必自建價目表。
const OUTPUT_MULTIPLE: f64 = 5.0;

fn accumulate(row: &mut UsageRow, line: &Value) {
    row.rounds += 1;
    if line.get("unreported").and_then(Value::as_bool) == Some(true) {
        row.unreported += 1;
        row.cost_partial = true;
        row.saved_partial = true;
        return;
    }
    let prompt = number(line, "prompt_tokens");
    let cached = number(line, "cached_tokens");
    let created = number(line, "created_tokens");
    let output = number(line, "output_tokens");
    row.prompt_tokens += prompt;
    row.output_tokens += output;
    // 量不到的輪次照計輸入輸出與花費，但不進命中率的分子分母
    if reported(line) {
        row.observed_rounds += 1;
        row.observed_prompt_tokens += prompt;
        row.cached_tokens += cached;
    }
    let cost = line.get("cost_usd").and_then(Value::as_f64);
    match cost {
        Some(value) => row.cost_usd = Some(row.cost_usd.unwrap_or(0.0) + value),
        None => row.cost_partial = true,
    }

    // 省下多少一輪一算再累加：每輪的單價與命中結構都不同，先加總會算錯
    let Some((read_mult, write_mult)) = cache_price(&text(line, "transport")) else {
        row.saved_partial = true;
        return;
    };
    row.priced_tokens += prompt;
    let fresh = prompt.saturating_sub(cached + created) as f64;
    let paid = fresh + read_mult * cached as f64 + write_mult * created as f64;
    let saved = prompt as f64 - paid;
    row.saved_tokens += saved;
    let charged = paid + OUTPUT_MULTIPLE * output as f64;
    match cost {
        Some(value) if charged > 0.0 => {
            row.saved_usd = Some(row.saved_usd.unwrap_or(0.0) + value / charged * saved);
        }
        _ => row.saved_partial = true,
    }
}

/// `scope` 為 None＝所有桌總計；`Some(id)` 只算該桌（空字串＝未標桌）。
/// `names` 是桌 id→顯示名（找不到就用 id）；`in_use` 是目前設定在用的（來源, 模型）。
pub fn summarize(
    log: &str,
    scope: Option<&str>,
    names: &[(String, String)],
    in_use: &[(String, String)],
) -> UsageReport {
    // app 現有的桌先全部列進去（沒紀錄就顯示 0 輪），順序照桌列表
    let mut worlds: Vec<WorldOption> = names
        .iter()
        .map(|(id, name)| WorldOption {
            id: id.clone(),
            name: name.clone(),
            rounds: 0,
        })
        .collect();
    let mut rows: Vec<UsageRow> = Vec::new();
    let mut total = UsageRow::default();
    let mut ping = UsageRow::default();
    let mut caches: Vec<CacheCount> = Vec::new();
    let mut events = 0u64;
    let mut latest = None;

    for line in log.lines() {
        let Ok(line) = serde_json::from_str::<Value>(line) else {
            continue; // 壞行跳過：診斷設施本來就是盡力而為
        };
        let world = text(&line, "world");
        // 對不上現有桌＝開桌生成或已刪掉的桌，接在桌列表後面
        match worlds.iter_mut().find(|option| option.id == world) {
            Some(option) => option.rounds += 1,
            None => worlds.push(WorldOption {
                id: world.clone(),
                name: String::new(),
                rounds: 1,
            }),
        }
        if scope.is_some_and(|scope| scope != world) {
            continue;
        }

        // 事件行（丟線重來）沒有快取結果，不進快取統計的分母；但它是最近發生的事，
        // 該讓玩家在收合狀態就看到，所以照樣占用「最近一輪」那句話
        if line.get("model").is_none() {
            events += 1;
            latest = Some(LatestCall {
                ts: text(&line, "ts"),
                mode: None,
                cache: None,
                cache_reason: None,
                // 加 event 欄之前的舊行把它寫在 diag
                event: Some(match line.get("event").and_then(Value::as_str) {
                    Some(event) => event.to_owned(),
                    None => text(&line, "diag"),
                }),
                reason: line
                    .get("reason")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                reported: true,
            });
            continue;
        }
        let (mode, cache, cache_reason) = classify(&line);
        let is_ping = mode.as_deref() == Some("ping");
        let state = chip_state(&cache, cache_reason.as_deref());
        match caches.iter_mut().find(|count| count.cache == state) {
            Some(count) => count.rounds += 1,
            None => caches.push(CacheCount {
                cache: state,
                rounds: 1,
            }),
        }
        if !is_ping {
            latest = Some(LatestCall {
                ts: text(&line, "ts"),
                mode,
                cache: Some(cache.clone()),
                cache_reason,
                event: None,
                reason: line
                    .get("reason")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                reported: reported(&line),
            });
        }
        if is_ping {
            accumulate(&mut ping, &line);
            continue;
        }
        accumulate(&mut total, &line);
        let source = text(&line, "transport");
        let model = text(&line, "model");
        let row = match rows
            .iter_mut()
            .position(|row| row.source == source && row.model == model)
        {
            Some(index) => &mut rows[index],
            None => {
                rows.push(UsageRow {
                    in_use: in_use
                        .iter()
                        .any(|(used_source, used_model)| {
                            *used_source == source && *used_model == model
                        }),
                    source,
                    model,
                    ..UsageRow::default()
                });
                rows.last_mut().expect("just pushed")
            }
        };
        accumulate(row, &line);
    }

    for row in rows.iter_mut().chain([&mut total, &mut ping]) {
        row.hit_rate = hit_rate(row);
    }
    // 花得最多的排前面（桌下拉不排序，照桌列表原順序）
    rows.sort_by_key(|row| std::cmp::Reverse(row.prompt_tokens));
    caches.sort_by_key(|count| std::cmp::Reverse(count.rounds));

    UsageReport {
        worlds,
        rows,
        total,
        ping,
        caches,
        events,
        latest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = concat!(
        r#"{"ts":"2026-08-04 01:00:00","transport":"claude","world":"w1","model":"sonnet","diag":"warmup","reason":"first-turn","prompt_tokens":1000,"cached_tokens":0,"created_tokens":1000,"output_tokens":100,"hit_rate":0.0,"cost_usd":0.02}"#, "\n",
        r#"{"ts":"2026-08-04 01:01:00","transport":"claude","world":"w1","model":"sonnet","diag":"ok","prompt_tokens":1200,"cached_tokens":1000,"created_tokens":200,"output_tokens":120,"hit_rate":83.3,"cost_usd":0.01}"#, "\n",
        r#"{"ts":"2026-08-04 01:02:00","transport":"claude","world":"w1","model":"opus","diag":"ok","prompt_tokens":4000,"cached_tokens":3600,"created_tokens":400,"output_tokens":200,"hit_rate":90.0,"cost_usd":0.05}"#, "\n",
        r#"{"ts":"2026-08-04 01:03:00","transport":"claude","world":"w1","model":"sonnet","diag":"ping","prompt_tokens":1200,"cached_tokens":1200,"created_tokens":0,"output_tokens":5,"hit_rate":100.0,"cost_usd":0.001}"#, "\n",
        r#"{"ts":"2026-08-04 01:04:00","transport":"agy","world":"w1","model":"(CLI 預設)","diag":"single","unreported":true}"#, "\n",
        r#"{"ts":"2026-08-04 01:05:00","transport":"claude","world":"w2","model":"sonnet","diag":"ok","prompt_tokens":800,"cached_tokens":400,"created_tokens":0,"output_tokens":60,"hit_rate":50.0,"cost_usd":0.008}"#, "\n",
        r#"{"ts":"2026-08-04 01:06:00","transport":"api","model":"x/y","diag":"single","prompt_tokens":500,"cached_tokens":0,"created_tokens":0,"output_tokens":50,"hit_rate":0.0}"#, "\n",
        "壞掉的一行\n",
    );

    /// 命中率的分母只收「量得到」的輪次，並與舊行相容：
    /// 舊 api 行沒有 cache_reporting 欄位，記下的 0 是讀錯欄位名的假數字，一律當量不到。
    #[test]
    fn hit_rate_counts_only_observed_rounds() {
        const MIXED: &str = concat!(
            // 舊行（無 cache_reporting）：api＝假 0 不可採信、claude＝欄位一直是對的
            r#"{"ts":"2026-08-21 13:47:35","transport":"api","world":"w1","model":"v/m","diag":"single","prompt_tokens":4690,"cached_tokens":0,"created_tokens":0,"output_tokens":551,"hit_rate":0.0}"#, "\n",
            // 新行：量到了、這輪沒中
            r#"{"ts":"2026-08-21 14:00:00","transport":"api","world":"w1","model":"v/m","diag":"single","cache_reporting":"reported","prompt_tokens":1000,"cached_tokens":400,"output_tokens":20}"#, "\n",
            // 新行：這條路不回報
            r#"{"ts":"2026-08-21 14:01:00","transport":"api","world":"w1","model":"v/m","diag":"single","cache_reporting":"absent","prompt_tokens":2000,"output_tokens":30}"#, "\n",
        );
        let report = summarize(MIXED, Some("w1"), &[("w1".to_owned(), "桌".to_owned())], &[]);
        let row = &report.rows[0];
        assert_eq!(row.rounds, 3);
        assert_eq!(row.observed_rounds, 1); // 三輪只有一輪的數字可信
        assert_eq!(row.prompt_tokens, 7_690); // 花費照算全部
        assert_eq!(row.observed_prompt_tokens, 1_000); // 命中率的分母只有那一輪
        assert_eq!(row.cached_tokens, 400);
        assert_eq!(row.hit_rate, Some(40.0)); // 不是 400/7690＝5.2%：量不到的輪次不稀釋

        // 舊 api 行的正命中是真值：當年 unwrap_or(0) 捏得出來的只有 0，>0 必是讀對了欄位
        const OLD_HIT: &str = r#"{"ts":"2026-08-20 10:00:00","transport":"api","world":"w1","model":"v/m","diag":"single","prompt_tokens":1000,"cached_tokens":600,"created_tokens":0,"output_tokens":20,"hit_rate":60.0}"#;
        let old = summarize(OLD_HIT, Some("w1"), &[("w1".to_owned(), "桌".to_owned())], &[]);
        assert_eq!(old.rows[0].observed_rounds, 1);
        assert_eq!(old.rows[0].hit_rate, Some(60.0));

        // 整條路都量不到＝命中率不存在，不可顯示 0%
        const ALL_ABSENT: &str = r#"{"ts":"2026-08-21 14:02:00","transport":"api","world":"w1","model":"v/m","diag":"single","cache_reporting":"absent","prompt_tokens":500,"output_tokens":10}"#;
        let blind = summarize(ALL_ABSENT, Some("w1"), &[("w1".to_owned(), "桌".to_owned())], &[]);
        assert_eq!(blind.rows[0].hit_rate, None);
        assert_eq!(blind.rows[0].observed_rounds, 0);
        assert_eq!(blind.total.hit_rate, None);
    }

    fn names() -> Vec<(String, String)> {
        vec![
            ("w1".to_owned(), "第一桌".to_owned()),
            ("w2".to_owned(), "第二桌".to_owned()),
            ("w3".to_owned(), "還沒玩過的桌".to_owned()),
        ]
    }

    /// 一桌一份：ping 不混進劇情輪、不回報用量的來源只算輪數、桌名對得回來。
    #[test]
    fn one_table_splits_models_and_keeps_ping_apart() {
        let report = summarize(
            LOG,
            Some("w1"),
            &names(),
            &[("claude".to_owned(), "sonnet".to_owned())],
        );

        // 下拉列出 app 現有的每一桌（沒紀錄的照列 0 輪）、不受選桌影響；
        // 對不上桌的紀錄（開桌生成）接在最後，名字留空由前端配字
        assert_eq!(
            report.worlds,
            vec![
                WorldOption { id: "w1".to_owned(), name: "第一桌".to_owned(), rounds: 5 },
                WorldOption { id: "w2".to_owned(), name: "第二桌".to_owned(), rounds: 1 },
                WorldOption { id: "w3".to_owned(), name: "還沒玩過的桌".to_owned(), rounds: 0 },
                WorldOption { id: String::new(), name: String::new(), rounds: 1 },
            ]
        );

        // 桌內按模型分行，用量大的排前面；使用中的模型帶標記
        let models: Vec<(&str, &str, u64, bool)> = report
            .rows
            .iter()
            .map(|row| (row.source.as_str(), row.model.as_str(), row.rounds, row.in_use))
            .collect();
        assert_eq!(
            models,
            vec![
                ("claude", "opus", 1, false),
                ("claude", "sonnet", 2, true),
                ("agy", "(CLI 預設)", 1, false),
            ]
        );

        let sonnet = &report.rows[1];
        assert_eq!(sonnet.prompt_tokens, 2_200); // ping 的 1200 不算進去
        assert_eq!(sonnet.cached_tokens, 1_000);
        assert!((sonnet.hit_rate.expect("claude 有回報") - 45.5).abs() < 0.05);
        assert_eq!(sonnet.cost_usd, Some(0.03));
        // 省下多少一輪一算：建快取那輪反而多付 250（寫入加價兩成半），
        // 讀到快取那輪省下 850，兩輪淨省 600 個輸入 token 的錢
        assert_eq!(sonnet.saved_tokens, 600.0);
        assert_eq!(sonnet.priced_tokens, 2_200);
        assert!((sonnet.saved_usd.expect("claude 有回報金額") - 0.006_090).abs() < 1e-5);
        assert!(!sonnet.saved_partial);

        // 不回報用量的來源：只知道跑過一輪，省下多少無從估起
        let agy = &report.rows[2];
        assert_eq!(agy.unreported, 1);
        assert_eq!(agy.prompt_tokens, 0);
        assert_eq!(agy.priced_tokens, 0);
        assert_eq!(agy.saved_usd, None);
        assert!(agy.cost_partial && agy.saved_partial);

        // 總計含整體命中率，保溫另計
        assert_eq!(report.total.rounds, 4);
        assert_eq!(report.total.prompt_tokens, 6_200);
        assert_eq!(report.total.cached_tokens, 4_600);
        assert!((report.total.hit_rate.expect("有可判讀輪次") - 74.2).abs() < 0.05);
        assert_eq!(report.total.saved_tokens, 3_740.0); // sonnet 600 ＋ opus 3140
        assert!(report.total.cost_partial && report.total.saved_partial); // agy 那輪兩樣都缺
        assert_eq!(report.ping.rounds, 1);
        assert_eq!(report.ping.cost_usd, Some(0.001));
        assert!((report.ping.saved_usd.expect("保溫也有金額") - 0.007_448).abs() < 1e-5);

        // 燈號看最近一筆非保溫紀錄
        assert_eq!(
            report.latest,
            Some(LatestCall {
                ts: "2026-08-04 01:04:00".to_owned(),
                mode: None, // 舊行只寫過 single，推不出當時的 prompt 形狀
                cache: Some("unknown".to_owned()),
                cache_reason: None,
                event: None,
                reason: None,
                reported: false, // agy 連 token 都不回報
            })
        );
    }

    /// 取消選桌＝所有桌總計；線事件（沒有 model）不算一輪呼叫、不進快取統計的分母。
    #[test]
    fn no_scope_totals_every_table_and_counts_lane_events() {
        let log = format!(
            "{LOG}{}\n",
            r#"{"ts":"2026-08-04 01:07:00","transport":"claude","world":"w1","lane":"chars:sonnet","diag":"drop-lane","reason":"rewrite-failed"}"#
        );
        let report = summarize(&log, None, &names(), &[]);

        assert_eq!(report.total.rounds, 6); // 兩桌劇情輪＋未標桌那筆，丟線事件不算一輪
        assert_eq!(report.total.prompt_tokens, 7_500);
        // api 那筆的快取怎麼計價不明，500 個輸入 token 不進「省了幾成」的分母
        assert_eq!(report.total.priced_tokens, 7_000);
        assert_eq!(report.total.saved_tokens, 4_100.0);
        assert_eq!(
            report
                .caches
                .iter()
                .find(|count| count.cache == "hit")
                .map(|count| count.rounds),
            // 三筆舊 ok ＋ 保溫那筆（它也中了）；丟線事件不在分母裡
            Some(4)
        );
        assert_eq!(
            report.latest,
            Some(LatestCall {
                ts: "2026-08-04 01:07:00".to_owned(),
                mode: None,
                cache: None,
                cache_reason: None,
                event: Some("drop-lane".to_owned()),
                reason: Some("rewrite-failed".to_owned()),
                reported: true, // 線事件沒有 model，不涉及快取量測
            })
        );

        // 空檔＝空報表，不 panic
        let empty = summarize("", None, &[], &[]);
        assert!(empty.rows.is_empty() && empty.latest.is_none() && empty.total.rounds == 0);
    }

    /// 拆兩軸之後：新行直接讀 mode／cache，舊行照 diag 回推且**不冒充形狀**。
    /// 本案的病灶就在這裡——舊帳本裡命中率過半的 single，回推後要說 hit 而不是「單發」。
    #[test]
    fn new_rows_read_two_axes_and_legacy_rows_never_fake_the_shape() {
        let mixed = [
            // 舊行：標 single，但供應商回報中了 81%
            r#"{"ts":"2026-08-21 10:00:00","transport":"api","world":"w1","model":"x/y","diag":"single","prompt_tokens":8000,"cached_tokens":6480,"created_tokens":1520,"output_tokens":100,"hit_rate":81.0}"#,
            // 舊行：加 cache_reporting 欄之前的 api 行，這個 0 分不出真假 → 量不到
            r#"{"ts":"2026-08-21 10:01:00","transport":"api","world":"w1","model":"x/y","diag":"single","prompt_tokens":4411,"cached_tokens":0,"created_tokens":4411,"output_tokens":90,"hit_rate":0.0}"#,
            // 新行：有回報、值就是 0＝真的沒中
            r#"{"ts":"2026-08-22 10:02:30","transport":"api","world":"w1","model":"x/y","mode":"shared","cache":"zero","roster_size":4,"cache_reporting":"reported","prompt_tokens":4411,"cached_tokens":0,"created_tokens":4411,"output_tokens":90,"hit_rate":0.0}"#,
            // 舊行：整條路不回報
            r#"{"ts":"2026-08-21 10:02:00","transport":"agy","world":"w1","model":"g","diag":"single","prompt_tokens":0,"output_tokens":0}"#,
            // 新行：共線劇情輪，四個角色
            r#"{"ts":"2026-08-22 10:03:00","transport":"api","world":"w1","model":"x/y","mode":"shared","cache":"hit","roster_size":4,"cache_reporting":"reported","prompt_tokens":9000,"cached_tokens":7000,"created_tokens":2000,"output_tokens":110,"hit_rate":77.8}"#,
            // 新行：續聊線該中沒中
            r#"{"ts":"2026-08-22 10:04:00","transport":"claude","world":"w1","model":"opus","lane":"chars:opus","mode":"resume","cache":"partial","cache_reason":"below-expected","cache_reporting":"reported","prompt_tokens":9300,"cached_tokens":200,"created_tokens":9100,"output_tokens":50,"hit_rate":2.2}"#,
        ]
        .join("\n")
            + "\n";
        let report = summarize(&mixed, Some("w1"), &names(), &[]);

        let count = |cache: &str| {
            report
                .caches
                .iter()
                .find(|entry| entry.cache == cache)
                .map_or(0, |entry| entry.rounds)
        };
        assert_eq!(count("hit"), 2); // 舊那筆 81% ＋ 新那筆共線
        assert_eq!(count("zero"), 1); // 只有標明有回報的那筆算真的沒中
        // 不回報的 agy ＋ 加欄之前那筆分不出真假的 api 0
        assert_eq!(count("unknown"), 2);

        // 最近一輪：新行兩軸照讀，形狀與快取結果各說各的
        let latest = report.latest.expect("有紀錄");
        assert_eq!(latest.mode.as_deref(), Some("resume"));
        assert_eq!(latest.cache.as_deref(), Some("partial"));
        assert_eq!(latest.cache_reason.as_deref(), Some("below-expected"));
        assert_eq!(latest.event, None);
        // partial 帶 below-expected＝證明得了照理該中，統計要落進紅色的 missed 那格，
        // 不能跟「這輪沒省到」混在一起
        assert_eq!(count("missed"), 1);
        assert_eq!(count("partial"), 0);
    }

    /// 統計的格子只有 missed 該亮紅燈：expired 只代表超過保守窗口，沒有證明該中。
    #[test]
    fn only_proven_misses_land_in_the_red_chip() {
        assert_eq!(chip_state("zero", Some("below-expected")), "missed");
        assert_eq!(chip_state("zero", Some("skipped")), "missed");
        assert_eq!(chip_state("partial", Some("below-expected")), "missed");
        assert_eq!(chip_state("zero", Some("expired")), "zero");
        assert_eq!(chip_state("partial", Some("expired")), "partial");
        assert_eq!(chip_state("zero", None), "zero"); // 無狀態路徑，算不出理論可中量
        assert_eq!(chip_state("not-expected", None), "not-expected");
        assert_eq!(chip_state("hit", None), "hit");
    }
}