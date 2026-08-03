//! 額度分頁（prompt-cache-optimization 包 6）：把 `prompt-cache.jsonl` 彙總成
//! 「一桌一份、桌內按模型分行」的報表給設定頁顯示。
//!
//! 兩條規矩：
//! - **token 是主軸**：四家 CLI 的 token 語意已在 cli.rs 換算成同一把尺，可以直接相加比較。
//! - **金額只轉述**：app 不自算牌價、不建價目表——分不出玩家是訂閱制還是 API 計費，
//!   只把各 CLI 自己回報的 `cost_usd` 加起來，並標記「有些輪次沒回報」。
//!
//! 診斷標籤與原因代碼原樣送到前端配 i18n（字典見 usage_log.rs 模組頂註解）。

use serde::Serialize;
use serde_json::Value;

/// 有紀錄的桌；`id` 空字串＝未標桌（加桌欄位之前的舊行、開桌生成的呼叫）。
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

fn accumulate(row: &mut UsageRow, line: &Value) {
    row.rounds += 1;
    if line.get("unreported").and_then(Value::as_bool) == Some(true) {
        row.unreported += 1;
        row.cost_partial = true;
        return;
    }
    row.prompt_tokens += number(line, "prompt_tokens");
    row.cached_tokens += number(line, "cached_tokens");
    row.output_tokens += number(line, "output_tokens");
    match line.get("cost_usd").and_then(Value::as_f64) {
        Some(cost) => row.cost_usd = Some(row.cost_usd.unwrap_or(0.0) + cost),
        None => row.cost_partial = true,
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
    let mut worlds: Vec<WorldOption> = Vec::new();
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
        match worlds.iter_mut().find(|option| option.id == world) {
            Some(option) => option.rounds += 1,
            None => worlds.push(WorldOption {
                id: world.clone(),
                name: names
                    .iter()
                    .find(|(id, _)| *id == world)
                    .map_or_else(|| world.clone(), |(_, name)| name.clone()),
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
    // 花得最多的排前面；桌下拉照輪數，未標桌自然墊底
    rows.sort_by_key(|row| std::cmp::Reverse(row.prompt_tokens));
    worlds.sort_by_key(|world| std::cmp::Reverse(world.rounds));
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

        // 下拉不受選桌影響：w1 五筆、w2 一筆、未標桌一筆（api 那行沒有 world）
        assert_eq!(
            report.worlds,
            vec![
                WorldOption { id: "w1".to_owned(), name: "第一桌".to_owned(), rounds: 5 },
                WorldOption { id: "w2".to_owned(), name: "第二桌".to_owned(), rounds: 1 },
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

        // 不回報用量的來源：只知道跑過一輪
        let agy = &report.rows[2];
        assert_eq!(agy.unreported, 1);
        assert_eq!(agy.prompt_tokens, 0);
        assert!(agy.cost_partial);

        // 總計含整體命中率，保溫另計
        assert_eq!(report.total.rounds, 4);
        assert_eq!(report.total.prompt_tokens, 6_200);
        assert_eq!(report.total.cached_tokens, 4_600);
        assert!((report.total.hit_rate - 74.2).abs() < 0.05);
        assert!(report.total.cost_partial); // agy 那輪沒金額
        assert_eq!(report.ping.rounds, 1);
        assert_eq!(report.ping.cost_usd, Some(0.001));

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
