use super::*;

mod floor;

const LOG: &str = concat!(
    r#"{"ts":"2026-08-04 01:00:00","transport":"claude","world":"w1","model":"sonnet","diag":"warmup","reason":"first-turn","prompt_tokens":1000,"cached_tokens":0,"created_tokens":1000,"output_tokens":100,"hit_rate":0.0,"cost_usd":0.02}"#,
    "\n",
    r#"{"ts":"2026-08-04 01:01:00","transport":"claude","world":"w1","model":"sonnet","diag":"ok","prompt_tokens":1200,"cached_tokens":1000,"created_tokens":200,"output_tokens":120,"hit_rate":83.3,"cost_usd":0.01}"#,
    "\n",
    r#"{"ts":"2026-08-04 01:02:00","transport":"claude","world":"w1","model":"opus","diag":"ok","prompt_tokens":4000,"cached_tokens":3600,"created_tokens":400,"output_tokens":200,"hit_rate":90.0,"cost_usd":0.05}"#,
    "\n",
    r#"{"ts":"2026-08-04 01:03:00","transport":"claude","world":"w1","model":"sonnet","diag":"ping","prompt_tokens":1200,"cached_tokens":1200,"created_tokens":0,"output_tokens":5,"hit_rate":100.0,"cost_usd":0.001}"#,
    "\n",
    r#"{"ts":"2026-08-04 01:04:00","transport":"agy","world":"w1","model":"(CLI 預設)","diag":"single","unreported":true}"#,
    "\n",
    r#"{"ts":"2026-08-04 01:05:00","transport":"claude","world":"w2","model":"sonnet","diag":"ok","prompt_tokens":800,"cached_tokens":400,"created_tokens":0,"output_tokens":60,"hit_rate":50.0,"cost_usd":0.008}"#,
    "\n",
    r#"{"ts":"2026-08-04 01:06:00","transport":"api","model":"x/y","diag":"single","prompt_tokens":500,"cached_tokens":0,"created_tokens":0,"output_tokens":50,"hit_rate":0.0}"#,
    "\n",
    "壞掉的一行\n",
);

/// 命中率的分母只收「量得到」的輪次，並與舊行相容：
/// 舊 api 行沒有 cache_reporting 欄位，記下的 0 是讀錯欄位名的假數字，一律當量不到。
#[test]
fn hit_rate_counts_only_observed_rounds() {
    const MIXED: &str = concat!(
        // 舊行（無 cache_reporting）：api＝假 0 不可採信、claude＝欄位一直是對的
        r#"{"ts":"2026-08-21 13:47:35","transport":"api","world":"w1","model":"v/m","diag":"single","prompt_tokens":4690,"cached_tokens":0,"created_tokens":0,"output_tokens":551,"hit_rate":0.0}"#,
        "\n",
        // 新行：量到了、這輪沒中
        r#"{"ts":"2026-08-21 14:00:00","transport":"api","world":"w1","model":"v/m","diag":"single","cache_reporting":"reported","prompt_tokens":1000,"cached_tokens":400,"output_tokens":20}"#,
        "\n",
        // 新行：這條路不回報
        r#"{"ts":"2026-08-21 14:01:00","transport":"api","world":"w1","model":"v/m","diag":"single","cache_reporting":"absent","prompt_tokens":2000,"output_tokens":30}"#,
        "\n",
    );
    let report = summarize(
        MIXED,
        Some("w1"),
        &[("w1".to_owned(), "桌".to_owned())],
        &[],
    );
    let row = &report.rows[0];
    assert_eq!(row.rounds, 3);
    assert_eq!(row.observed_rounds, 1); // 三輪只有一輪的數字可信
    assert_eq!(row.prompt_tokens, 7_690); // 花費照算全部
    assert_eq!(row.observed_prompt_tokens, 1_000); // 命中率的分母只有那一輪
    assert_eq!(row.cached_tokens, 400);
    assert_eq!(row.hit_rate, Some(40.0)); // 不是 400/7690＝5.2%：量不到的輪次不稀釋

    // 舊 api 行的正命中是真值：當年 unwrap_or(0) 捏得出來的只有 0，>0 必是讀對了欄位
    const OLD_HIT: &str = r#"{"ts":"2026-08-20 10:00:00","transport":"api","world":"w1","model":"v/m","diag":"single","prompt_tokens":1000,"cached_tokens":600,"created_tokens":0,"output_tokens":20,"hit_rate":60.0}"#;
    let old = summarize(
        OLD_HIT,
        Some("w1"),
        &[("w1".to_owned(), "桌".to_owned())],
        &[],
    );
    assert_eq!(old.rows[0].observed_rounds, 1);
    assert_eq!(old.rows[0].hit_rate, Some(60.0));

    // 整條路都量不到＝命中率不存在，不可顯示 0%
    const ALL_ABSENT: &str = r#"{"ts":"2026-08-21 14:02:00","transport":"api","world":"w1","model":"v/m","diag":"single","cache_reporting":"absent","prompt_tokens":500,"output_tokens":10}"#;
    let blind = summarize(
        ALL_ABSENT,
        Some("w1"),
        &[("w1".to_owned(), "桌".to_owned())],
        &[],
    );
    assert_eq!(blind.rows[0].hit_rate, None);
    assert_eq!(blind.rows[0].observed_rounds, 0);
    assert_eq!(blind.total.hit_rate, None);
}

#[test]
fn image_usage_keeps_tokens_and_cost_but_not_dialogue_cache_metrics() {
    const WITH_IMAGE: &str = concat!(
        r#"{"ts":"2026-09-09 13:51:00","transport":"agy","world":"w1","model":"gemini","purpose":"image","mode":"oneshot","cache":"hit","cache_reporting":"reported","prompt_tokens":9000,"cached_tokens":8000,"output_tokens":100}"#,
        "\n",
        r#"{"ts":"2026-09-09 14:12:00","transport":"agy","world":"w1","model":"gemini","mode":"resume","cache":"zero","cache_reporting":"reported","prompt_tokens":1000,"cached_tokens":0,"output_tokens":50}"#,
        "\n",
    );
    let report = summarize(
        WITH_IMAGE,
        Some("w1"),
        &[("w1".to_owned(), "桌".to_owned())],
        &[],
    );
    let row = &report.rows[0];
    assert_eq!(row.rounds, 2); // token／花費明細仍保留生圖呼叫
    assert_eq!(row.prompt_tokens, 10_000);
    assert_eq!(row.cache_rounds, 1);
    assert_eq!(row.observed_rounds, 1);
    assert_eq!(row.cached_tokens, 0); // 生圖的 8000 不混進對話命中率
    assert_eq!(row.hit_rate, Some(0.0));
    assert_eq!(report.caches.len(), 1);
    assert_eq!(report.caches[0].cache, "zero");
    assert_eq!(report.caches[0].rounds, 1);
    assert_eq!(report.latest.as_ref().unwrap().ts, "2026-09-09 14:12:00");

    let image_only = summarize(
        WITH_IMAGE.lines().next().unwrap(),
        Some("w1"),
        &[("w1".to_owned(), "桌".to_owned())],
        &[],
    );
    assert_eq!(image_only.total.rounds, 1);
    assert_eq!(image_only.total.prompt_tokens, 9_000);
    assert_eq!(image_only.total.cache_rounds, 0);
    assert_eq!(image_only.total.hit_rate, None);
    assert!(image_only.caches.is_empty());
    assert!(image_only.latest.is_none());
}

fn names() -> Vec<(String, String)> {
    vec![
        ("w1".to_owned(), "第一桌".to_owned()),
        ("w2".to_owned(), "第二桌".to_owned()),
        ("w3".to_owned(), "還沒玩過的桌".to_owned()),
    ]
}

/// 一桌一份：舊保溫列只算錢、不回報用量的來源只算輪數、桌名對得回來。
#[test]
fn one_table_splits_models_and_counts_legacy_ping_cost_only() {
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
            WorldOption {
                id: "w1".to_owned(),
                name: "第一桌".to_owned(),
                rounds: 5
            },
            WorldOption {
                id: "w2".to_owned(),
                name: "第二桌".to_owned(),
                rounds: 1
            },
            WorldOption {
                id: "w3".to_owned(),
                name: "還沒玩過的桌".to_owned(),
                rounds: 0
            },
            WorldOption {
                id: String::new(),
                name: String::new(),
                rounds: 1
            },
        ]
    );

    // 桌內按模型分行，用量大的排前面；使用中的模型帶標記
    let models: Vec<(&str, &str, u64, bool)> = report
        .rows
        .iter()
        .map(|row| {
            (
                row.source.as_str(),
                row.model.as_str(),
                row.rounds,
                row.in_use,
            )
        })
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
    assert_eq!(sonnet.observed_rounds, 2);
    assert!((sonnet.hit_rate.expect("claude 有回報") - 45.5).abs() < 0.05);
    // ping 的 0.001 進這列的花費
    assert!((sonnet.cost_usd.expect("claude 有回報") - 0.031).abs() < 1e-9);
    // 省下多少一輪一算：建快取那輪反而多付 250（寫入加價兩成半），
    // 讀到快取那輪省下 850，兩輪淨省 600 個輸入 token 的錢
    assert_eq!(sonnet.saved_tokens, 600.0);
    assert_eq!(sonnet.priced_tokens, 2_200);
    assert!((sonnet.saved_usd.expect("claude 有回報金額") - 0.006_090).abs() < 1e-5);
    assert!(!sonnet.saved_partial);
    assert_eq!(sonnet.estimated_rounds, 2); // 舊列沒有 1h 拆分，寫入按 1.25 估
                                            // 別名 opus 按現行 Opus 5.5 讀價 0.05 估：360 讀＋500 寫＝付 680，省 3320
    let opus = &report.rows[0];
    assert_eq!(opus.saved_tokens, 3_320.0);
    assert_eq!(opus.estimated_rounds, 1);

    // 不回報用量的來源：只知道跑過一輪，省下多少無從估起
    let agy = &report.rows[2];
    assert_eq!(agy.unreported, 1);
    assert_eq!(agy.prompt_tokens, 0);
    assert_eq!(agy.priced_tokens, 0);
    assert_eq!(agy.saved_usd, None);
    assert!(agy.cost_partial && agy.saved_partial);

    // 總計含整體命中率；舊保溫列只貢獻花費
    assert_eq!(report.total.rounds, 4);
    assert_eq!(report.total.prompt_tokens, 6_200);
    assert_eq!(report.total.cached_tokens, 4_600);
    assert_eq!(report.total.observed_prompt_tokens, 6_200);
    assert!((report.total.hit_rate.expect("有可判讀輪次") - 74.2).abs() < 0.05);
    assert_eq!(report.total.saved_tokens, 3_920.0); // sonnet 600 ＋ opus 3320
    assert_eq!(report.total.estimated_rounds, 3);
    assert!((report.total.cost_usd.expect("有金額") - 0.081).abs() < 1e-9);
    assert!(report.total.cost_partial && report.total.saved_partial); // agy 那輪兩樣都缺
    assert_eq!(
        report.caches.iter().map(|count| count.rounds).sum::<u64>(),
        4
    );

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
    assert_eq!(report.total.saved_tokens, 4_280.0);
    assert_eq!(
        report
            .caches
            .iter()
            .find(|count| count.cache == "hit")
            .map(|count| count.rounds),
        // 三筆舊 ok；舊保溫列與丟線事件都不在分母裡
        Some(3)
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

/// 讀到 0 的原因只留給 claude 續聊線：舊帳本裡 agy／grok 的 skipped（以及任何零命中原因）
/// 讀的時候濾掉，兩種帳本格式都一樣；不足九成的數字診斷不動。
#[test]
fn zero_reasons_from_non_claude_lanes_are_dropped_on_read() {
    let row = |transport: &str, body: &str| {
        format!(
            r#"{{"ts":"2026-10-06 01:00:00","transport":"{transport}","world":"w1","model":"m","lane":"chars:m",{body}}}"#
        )
    };
    let new_zero = |reason: &str| {
        format!(
            r#""mode":"resume","cache":"zero","cache_reason":"{reason}","cache_reporting":"reported","prompt_tokens":9000,"cached_tokens":0,"output_tokens":10"#
        )
    };
    // 加 cache 欄之前的舊續聊線行：靠 expected_cached／age_secs／created_tokens 推導
    let legacy_skipped = r#""prompt_tokens":9000,"cached_tokens":0,"created_tokens":0,"expected_cached":8000,"age_secs":30,"output_tokens":10"#;
    let legacy_expired = r#""prompt_tokens":9000,"cached_tokens":0,"created_tokens":9000,"expected_cached":8000,"age_secs":900,"output_tokens":10"#;
    let legacy_below = r#""prompt_tokens":9000,"cached_tokens":0,"created_tokens":9000,"expected_cached":8000,"age_secs":30,"output_tokens":10"#;
    let samples = [
        (new_zero("skipped"), "skipped"),
        (new_zero("expired"), "expired"),
        (new_zero("below-expected"), "below-expected"),
        (legacy_skipped.to_owned(), "skipped"),
        (legacy_expired.to_owned(), "expired"),
        (legacy_below.to_owned(), "below-expected"),
    ];
    for (body, claude_reason) in &samples {
        for transport in ["agy", "grok"] {
            let line: Value = serde_json::from_str(&row(transport, body)).unwrap();
            let (_, cache, reason) = classify(&line);
            assert_eq!(
                (cache.as_str(), reason),
                ("zero", None),
                "{transport} {body}"
            );
            let report = summarize(&row(transport, body), Some("w1"), &names(), &[]);
            assert_eq!(report.caches[0].cache, "zero"); // 不再落進紅色的 missed
            assert_eq!(report.latest.unwrap().cache_reason, None);
        }
        // claude 對照：原因保留
        let line: Value = serde_json::from_str(&row("claude", body)).unwrap();
        let (_, cache, reason) = classify(&line);
        assert_eq!(
            (cache.as_str(), reason.as_deref()),
            ("zero", Some(*claude_reason))
        );
    }
    // 非 claude 不足九成：數字診斷照留
    let partial = row(
        "agy",
        r#""mode":"resume","cache":"partial","cache_reason":"below-expected","cache_reporting":"reported","prompt_tokens":9000,"cached_tokens":200,"output_tokens":10"#,
    );
    let line: Value = serde_json::from_str(&partial).unwrap();
    assert_eq!(
        classify(&line),
        (
            Some("resume".to_owned()),
            "partial".to_owned(),
            Some("below-expected".to_owned())
        )
    );
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

/// 一列 claude 新帳本：1000 輸入、讀 cached、寫 created（其中 1h 部分 one_hour）。
fn claude_line(model: &str, cached: u64, created: u64, one_hour: Option<u64>) -> String {
    let split = one_hour.map_or(String::new(), |value| {
        format!(r#","created_1h_tokens":{value}"#)
    });
    format!(
        r#"{{"ts":"t","transport":"claude","world":"w1","model":"{model}","mode":"resume","cache":"hit","cache_reporting":"reported","prompt_tokens":1000,"cached_tokens":{cached},"created_tokens":{created}{split},"output_tokens":0,"cost_usd":0.01}}"#
    )
}

fn priced(line: &str) -> UsageRow {
    summarize(line, Some("w1"), &[("w1".to_owned(), "桌".to_owned())], &[]).total
}

/// 讀快取係數按完整版本比到邊界；別名看不出世代＝估計。
#[test]
fn claude_read_price_matches_exact_versions() {
    assert_eq!(claude_read_price("claude-opus-5-5"), (0.05, false));
    assert_eq!(claude_read_price("claude-opus-5-5-20261001"), (0.05, false));
    assert_eq!(claude_read_price("claude-fable-5-1"), (0.025, false));
    for older in [
        "claude-opus-4-6",
        "claude-opus-4-7",
        "claude-opus-5",
        "claude-fable-5",
        "claude-opus-5-50",
        "claude-sonnet-5-5",
        "sonnet",
        "haiku",
    ] {
        assert_eq!(claude_read_price(older), (0.1, false), "{older}");
    }
    assert_eq!(claude_read_price("opus"), (0.05, true));
    assert_eq!(claude_read_price("fable"), (0.025, true));

    // 讀 1000：0.05 → 付 50 省 950；0.1 → 付 100 省 900
    let exact = priced(&claude_line("claude-opus-5-5", 1000, 0, Some(0)));
    assert_eq!(exact.saved_tokens, 950.0);
    assert_eq!(exact.estimated_rounds, 0);
    assert_eq!(
        priced(&claude_line("claude-opus-4-7", 1000, 0, Some(0))).saved_tokens,
        900.0
    );
    let alias = priced(&claude_line("opus", 1000, 0, Some(0)));
    assert_eq!(alias.saved_tokens, 950.0);
    assert_eq!(alias.estimated_rounds, 1);
    assert_eq!(
        priced(&claude_line("sonnet", 1000, 0, Some(0))).estimated_rounds,
        0
    );
}

/// 寫快取：1h 部分 2 倍、5m 部分 1.25 倍；沒有拆分按 1.25 估；拆分大於總寫入時壓回總量。
#[test]
fn claude_write_cost_follows_one_hour_split() {
    // 寫 1000 全 1h：付 2000，省 -1000
    let hour = priced(&claude_line("sonnet", 0, 1000, Some(1000)));
    assert_eq!(hour.saved_tokens, -1000.0);
    assert_eq!(hour.estimated_rounds, 0);
    // 純 5m：付 1250
    let five = priced(&claude_line("sonnet", 0, 1000, Some(0)));
    assert_eq!(five.saved_tokens, -250.0);
    assert_eq!(five.estimated_rounds, 0);
    // 混合 400＋600：800＋750＝1550
    assert_eq!(
        priced(&claude_line("sonnet", 0, 1000, Some(400))).saved_tokens,
        -550.0
    );
    // 沒拆分：按 1.25 估並標估計
    let unknown = priced(&claude_line("sonnet", 0, 1000, None));
    assert_eq!(unknown.saved_tokens, -250.0);
    assert_eq!(unknown.estimated_rounds, 1);
    // 拆分壞資料大於總寫入：壓回 1000，不下溢
    assert_eq!(
        priced(&claude_line("sonnet", 0, 1000, Some(5000))).saved_tokens,
        -1000.0
    );
    // 全為新列時沒有估計標記
    let log = format!(
        "{}\n{}",
        claude_line("sonnet", 0, 1000, Some(1000)),
        claude_line("claude-opus-5-5", 900, 100, Some(100))
    );
    assert_eq!(priced(&log).estimated_rounds, 0);
}
