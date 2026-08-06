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
    pub hit_rate: f64,
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
pub struct DiagCount {
    pub diag: String,
    pub rounds: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LatestCall {
    pub ts: String,
    pub diag: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UsageReport {
    /// 全檔有紀錄的桌（不受選桌影響，供下拉用）
    pub worlds: Vec<WorldOption>,
    pub rows: Vec<UsageRow>,
    pub total: UsageRow,
    /// 保溫 ping 小計：不推進劇情，與劇情輪分開算
    pub ping: UsageRow,
    pub diags: Vec<DiagCount>,
    /// 最近一筆非保溫紀錄，供燈號與原因句
    pub latest: Option<LatestCall>,
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

fn hit_rate(prompt_tokens: u64, cached_tokens: u64) -> f64 {
    if prompt_tokens == 0 {
        return 0.0;
    }
    let rate = cached_tokens as f64 * 100.0 / prompt_tokens as f64;
    (rate * 10.0).round() / 10.0
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
    row.cached_tokens += cached;
    row.output_tokens += output;
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
    let mut diags: Vec<DiagCount> = Vec::new();
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

        let diag = text(&line, "diag");
        match diags.iter_mut().find(|count| count.diag == diag) {
            Some(count) => count.rounds += 1,
            None => diags.push(DiagCount {
                diag: diag.clone(),
                rounds: 1,
            }),
        }
        if diag != "ping" {
            latest = Some(LatestCall {
                ts: text(&line, "ts"),
                diag: diag.clone(),
                reason: line
                    .get("reason")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            });
        }
        // 沒有 model＝丟線之類的線事件，只進診斷統計，不算一輪呼叫
        if line.get("model").is_none() {
            continue;
        }
        if diag == "ping" {
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
        row.hit_rate = hit_rate(row.prompt_tokens, row.cached_tokens);
    }
    // 花得最多的排前面（桌下拉不排序，照桌列表原順序）
    rows.sort_by_key(|row| std::cmp::Reverse(row.prompt_tokens));
    diags.sort_by_key(|count| std::cmp::Reverse(count.rounds));

    UsageReport {
        worlds,
        rows,
        total,
        ping,
        diags,
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
        assert!((sonnet.hit_rate - 45.5).abs() < 0.05);
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
        assert!((report.total.hit_rate - 74.2).abs() < 0.05);
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
                diag: "single".to_owned(),
                reason: None,
            })
        );
    }

    /// 取消選桌＝所有桌總計；線事件（沒有 model）只進診斷統計。
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
            report.diags.iter().find(|count| count.diag == "ok").map(|count| count.rounds),
            Some(3)
        );
        assert_eq!(
            report.latest,
            Some(LatestCall {
                ts: "2026-08-04 01:07:00".to_owned(),
                diag: "drop-lane".to_owned(),
                reason: Some("rewrite-failed".to_owned()),
            })
        );

        // 空檔＝空報表，不 panic
        let empty = summarize("", None, &[], &[]);
        assert!(empty.rows.is_empty() && empty.latest.is_none() && empty.total.rounds == 0);
    }
}
