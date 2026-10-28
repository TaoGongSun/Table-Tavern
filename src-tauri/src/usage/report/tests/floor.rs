//! 自帶底線套進報表（vendor-prefix-floor、vendor-floor-b2）：判定、省額、選桌一致。

use super::super::*;
use serde_json::json;

/// 一筆 codex 共線呼叫；`version` 為 None＝舊行（版本未知組）。
fn codex(world: &str, version: Option<&str>, prompt: u64, cached: u64) -> String {
    let mut line = json!({
        "ts": "2026-10-28 00:35:00", "transport": "codex", "world": world, "model": "gpt-5.6-terra",
        "mode": "shared", "cache": if cached > 0 { "hit" } else { "zero" }, "roster_size": 3,
        "prompt_tokens": prompt, "cache_reporting": "reported", "cached_tokens": cached,
        "created_tokens": 0, "output_tokens": 150, "cost_usd": 0.01,
    });
    if let Some(version) = version {
        line["cli_version"] = json!(version);
    }
    line.to_string()
}

fn api(endpoint: &str, prompt: u64, cached: u64) -> String {
    json!({
        "ts": "2026-10-28 01:00:00", "transport": "api", "world": "w1", "model": "v/m",
        "mode": "shared", "cache": if cached > 0 { "hit" } else { "zero" }, "prompt_tokens": prompt,
        "cache_reporting": "reported", "cached_tokens": cached, "output_tokens": 20,
        "endpoint": endpoint,
    })
    .to_string()
}

fn report(lines: &[String], scope: Option<&str>) -> UsageReport {
    let names = [
        ("w1".to_owned(), "第一桌".to_owned()),
        ("w2".to_owned(), "第二桌".to_owned()),
    ];
    summarize(&lines.join("\n"), scope, &names, &[])
}

fn chips(report: &UsageReport) -> Vec<(String, u64)> {
    let mut chips: Vec<_> = report
        .caches
        .iter()
        .map(|count| (count.cache.clone(), count.rounds))
        .collect();
    chips.sort();
    chips
}

/// 實測佇列第 14 項的 codex 帳本（2026-10-28，codex 0.164.0）：9 通我方 0 命中原本全標 hit。
/// 底線應認出 11,008，換角色暖那通 6,912 低於底線同樣是只中自帶；頭條沒有任何一輪算已省。
#[test]
fn verification_ledger_codex_rounds_are_vendor_only() {
    let v = Some("codex-cli 0.164.0");
    let lines: Vec<String> = [
        (18_478, 11_008),
        (18_621, 11_008),
        (14_578, 11_008),
        (18_920, 6_912),
        (15_187, 11_008),
        (15_336, 11_008),
        (15_512, 11_008),
        (22_656, 11_008),
    ]
    .iter()
    .map(|&(prompt, cached)| codex("w1", v, prompt, cached))
    .collect();
    let report = report(&lines, Some("w1"));
    assert_eq!(chips(&report), vec![("vendor-only".to_owned(), 8)]);
    let total = &report.total;
    assert_eq!(total.vendor_only_rounds, 8);
    assert_eq!(total.own_hit_rounds, 0);
    assert_eq!(total.saved_tokens, 0.0);
    assert_eq!(total.saved_usd, Some(0.0));
    // 分母保留：輸入量照算進頭條分母，只是省下量記 0
    assert_eq!(total.priced_tokens, 139_288);
    assert_eq!(total.vendor_cached_tokens, 11_008 * 7 + 6_912);
    // 明細原始值照列
    assert_eq!(total.cached_tokens, 11_008 * 7 + 6_912);
    assert_eq!(
        report.latest.as_ref().unwrap().cache.as_deref(),
        Some("vendor-only")
    );
}

/// 成功量出現得比自帶量多：底線仍是自帶段，超過底線的輪次照舊算命中，省額先扣掉自帶段
#[test]
fn frequent_success_stays_a_hit_and_saves_only_our_part() {
    let v = Some("1.0");
    let mut lines: Vec<String> = (0..3).map(|i| codex("w1", v, 20_000 + i, 9_984)).collect();
    for i in 0..5 {
        lines.push(codex("w1", v, 21_000 + i, 19_200));
    }
    let report = report(&lines, Some("w1"));
    assert_eq!(
        chips(&report),
        vec![("hit".to_owned(), 5), ("vendor-only".to_owned(), 3)]
    );
    assert_eq!(report.total.own_hit_rounds, 5);
    assert_eq!(report.total.vendor_cached_tokens, 9_984 * 8);

    // 超過底線那輪：省額比沒有底線時少「底線×(1−讀係數)」，單價照原實付反推
    let hit = codex("w1", v, 21_000, 19_200);
    let alone = summarize(&hit, Some("w1"), &[], &[]).total;
    let with_floor = summarize(
        &[lines[..3].join("\n"), hit.clone()].join("\n"),
        Some("w1"),
        &[],
        &[],
    )
    .total;
    let floor_only = summarize(&lines[..3].join("\n"), Some("w1"), &[], &[]).total;
    assert_eq!(floor_only.saved_tokens, 0.0);
    let ours = with_floor.saved_tokens - floor_only.saved_tokens;
    assert!((ours - (alone.saved_tokens - 9_984.0 * 0.9)).abs() < 1e-6);
    let usd = with_floor.saved_usd.unwrap() - floor_only.saved_usd.unwrap();
    assert!((usd - alone.saved_usd.unwrap() * ours / alone.saved_tokens).abs() < 1e-12);
}

/// 斷崖消失：底線 11,008、讀係數 0.1、沒有寫入時，讀到 11,008／11,009／12,000 依序省 0／0.9／892.8
#[test]
fn savings_grow_smoothly_past_the_floor() {
    let v = Some("0.164.0");
    let floor: Vec<String> = (0..3).map(|i| codex("w1", v, 18_000 + i, 11_008)).collect();
    let base = summarize(&floor.join("\n"), Some("w1"), &[], &[]).total;
    assert_eq!(base.saved_tokens, 0.0);
    for (cached, expected) in [(11_008, 0.0), (11_009, 0.9), (12_000, 892.8)] {
        let line = codex("w1", v, 20_000, cached);
        let total = summarize(&[floor.join("\n"), line].join("\n"), Some("w1"), &[], &[]).total;
        let saved = total.saved_tokens - base.saved_tokens;
        assert!((saved - expected).abs() < 1e-6, "讀到 {cached}：省 {saved}");
        // 美元：該輪金額 0.01 ÷ 原實付等值 × 我方省額
        let paid = (20_000 - cached) as f64 + 0.1 * cached as f64 + 5.0 * 150.0;
        let usd = total.saved_usd.unwrap() - base.saved_usd.unwrap();
        assert!((usd - 0.01 / paid * expected).abs() < 1e-12);
    }
}

/// 算得出理論可中量、讀到量又低於底線的輪次：不標只中自帶，但我方讀到 0、省額 0
#[test]
fn hits_below_the_floor_save_nothing() {
    let mut lines: Vec<String> = (0..3)
        .map(|i| {
            json!({"ts": "t", "transport": "grok", "world": "w1", "model": "grok-4.6", "mode": "resume",
                   "cache": "hit", "prompt_tokens": 1_000 + i, "cache_reporting": "reported",
                   "cached_tokens": 128, "output_tokens": 10, "lane": "chars:grok", "age_secs": 5,
                   "reason": "first-turn", "cost_usd": 0.001})
            .to_string()
        })
        .collect();
    lines.push(
        json!({"ts": "t", "transport": "grok", "world": "w1", "model": "grok-4.6", "mode": "resume",
               "cache": "hit", "prompt_tokens": 1_500, "cache_reporting": "reported",
               "cached_tokens": 100, "output_tokens": 10, "lane": "chars:grok", "age_secs": 5,
               "expected_cached": 100, "cost_usd": 0.001})
        .to_string(),
    );
    let report = report(&lines, Some("w1"));
    assert_eq!(
        chips(&report),
        vec![("hit".to_owned(), 1), ("vendor-only".to_owned(), 3)]
    );
    assert_eq!(report.total.saved_tokens, 0.0);
    assert_eq!(report.total.vendor_cached_tokens, 128 * 3 + 100);
}

/// 底線未辨識：省額照現行（整段讀到量都算）
#[test]
fn unknown_floor_keeps_full_savings() {
    let v = Some("1.0");
    let lines = [codex("w1", v, 20_000, 9_984), codex("w1", v, 21_000, 9_984)];
    let total = report(&lines, Some("w1")).total;
    assert_eq!(total.vendor_cached_tokens, 0);
    assert!((total.saved_tokens - 2.0 * 9_984.0 * 0.9).abs() < 1e-6);
}

/// claude 通道不認底線：重複值再多也不標只中自帶、不拆自帶段、不扣省額，最近一輪燈號照 hit
#[test]
fn claude_rows_never_get_a_floor() {
    let claude = |prompt: u64, cached: u64, created: u64| {
        json!({"ts": "t", "transport": "claude", "world": "w1", "model": "claude-opus-4-8",
               "mode": "single", "cache": "hit", "prompt_tokens": prompt, "cache_reporting": "reported",
               "cached_tokens": cached, "created_tokens": created, "created_1h_tokens": 0,
               "output_tokens": 0, "cost_usd": 0.01})
        .to_string()
    };
    let lines: Vec<String> = (0..4).map(|i| claude(5_000 + i, 3_223, 0)).collect();
    let report = report(&lines, Some("w1"));
    assert_eq!(chips(&report), vec![("hit".to_owned(), 4)]);
    assert_eq!(report.total.vendor_only_rounds, 0);
    assert_eq!(report.total.vendor_cached_tokens, 0);
    assert_eq!(report.total.own_hit_rounds, 4);
    assert!((report.total.saved_tokens - 4.0 * 3_223.0 * 0.9).abs() < 1e-6);
    assert_eq!(report.latest.unwrap().cache.as_deref(), Some("hit"));

    // 重建輪寫入加價照算：讀到少、寫入多時省額為負，不歸 0
    let rebuild = super::priced(&claude(5_000, 100, 4_000));
    assert!((rebuild.saved_tokens - (100.0 * 0.9 - 4_000.0 * 0.25)).abs() < 1e-6);
    assert!(rebuild.saved_tokens < 0.0);
}

/// 同一段輸入重送中同樣的量＝我方內容真的中了，不是底線；0（真 0）與舊 api 假 0 都不當底線
#[test]
fn repeats_and_zeros_never_become_a_floor() {
    let v = Some("1.0");
    let same: Vec<String> = (0..4).map(|_| codex("w1", v, 9_000, 4_096)).collect();
    assert_eq!(
        chips(&report(&same, Some("w1"))),
        vec![("hit".to_owned(), 4)]
    );

    let mut zeros: Vec<String> = (0..5).map(|i| codex("w1", v, 9_000 + i, 0)).collect();
    // 舊 api 行沒有 cache_reporting：0 是讀錯欄位的假數字，量不到，不進樣本
    for i in 0..5 {
        zeros.push(
            json!({"ts": "2026-08-21 13:00:00", "transport": "api", "world": "w1", "model": "v/m",
                   "diag": "single", "prompt_tokens": 4_000 + i, "cached_tokens": 0, "output_tokens": 5})
            .to_string(),
        );
    }
    zeros.push(codex("w1", v, 9_999, 2_048));
    let report = report(&zeros, Some("w1"));
    assert_eq!(report.total.vendor_only_rounds, 0);
    assert_eq!(report.total.vendor_cached_tokens, 0);
}

/// 版本未知（舊行）與有版本的各算各的。已知限制：舊行混了新版 11,008 時，
/// 未知組的底線是 9,984，那輪顯示成我方中了 1,024
#[test]
fn unknown_version_rows_form_their_own_group() {
    let mut lines: Vec<String> = (0..3)
        .map(|i| codex("w1", None, 20_000 + i, 9_984))
        .collect();
    lines.extend((0..3).map(|i| codex("w1", Some("0.164.0"), 18_000 + i, 11_008)));
    lines.push(codex("w1", None, 19_000, 11_008));
    let report = report(&lines, Some("w1"));
    assert_eq!(
        chips(&report),
        vec![("hit".to_owned(), 1), ("vendor-only".to_owned(), 6)]
    );
    assert_eq!(report.total.vendor_cached_tokens, 9_984 * 4 + 11_008 * 3);
    // 未知組底線 9,984：那輪我方讀到 1,024
    assert!((report.total.saved_tokens - 1_024.0 * 0.9).abs() < 1e-6);
}

/// 同一個模型打不同端點：底線各算各的，另一個端點樣本不足就不判
#[test]
fn api_endpoints_are_separate_groups_with_their_own_label() {
    let mut lines: Vec<String> = (0..3)
        .map(|i| api("openrouter.ai:443", 5_000 + i, 64))
        .collect();
    lines.push(api("opencode.ai:443", 5_100, 64));
    let report = report(&lines, Some("w1"));
    assert_eq!(
        chips(&report),
        vec![("hit".to_owned(), 1), ("vendor-only-api".to_owned(), 3)]
    );
    assert_eq!(report.total.vendor_only_api_rounds, 3);
    assert_eq!(report.total.own_hit_rounds, 1);
    // API 計價不明：省額照舊不估
    assert_eq!(report.total.saved_tokens, 0.0);
    assert_eq!(report.total.priced_tokens, 0);
    assert!(report.total.saved_partial);
}

/// 底線用全檔算：別桌的樣本也算數，選一桌與看總計對同一輪的判定一致
#[test]
fn verdict_is_the_same_with_or_without_table_scope() {
    let v = Some("1.0");
    let lines = vec![
        codex("w2", v, 10_000, 9_984),
        codex("w2", v, 11_000, 9_984),
        codex("w2", v, 12_000, 9_984),
        codex("w1", v, 13_000, 9_984),
        codex("w1", v, 14_000, 15_000),
    ];
    let one = report(&lines, Some("w1"));
    assert_eq!(
        chips(&one),
        vec![("hit".to_owned(), 1), ("vendor-only".to_owned(), 1)]
    );
    // 別桌樣本認出的底線一樣扣：w1 只有 15,000 那輪省下超出底線的部分
    let ours = 14_000.0 - 0.1 * 15_000.0 - 0.9 * 9_984.0;
    assert!((one.total.saved_tokens - ours).abs() < 1e-6);
    let all = report(&lines, None);
    assert_eq!(
        chips(&all),
        vec![("hit".to_owned(), 1), ("vendor-only".to_owned(), 4)]
    );
    assert_eq!(all.total.saved_tokens, one.total.saved_tokens);
}

/// 算得出理論可中量的續聊線不走這條；重開那輪（期望為 0）才走
#[test]
fn lanes_with_expectations_keep_their_own_judgement() {
    let lane = |prompt: u64, cached: u64, reason: Option<&str>, expected: u64| {
        let mut line = json!({
            "ts": "2026-10-28 02:00:00", "transport": "grok", "world": "w1", "model": "grok-4.6",
            "mode": "resume", "cache": "hit", "prompt_tokens": prompt, "cache_reporting": "reported",
            "cached_tokens": cached, "output_tokens": 10, "lane": "chars:grok", "age_secs": 5,
            "expected_cached": expected,
        });
        if let Some(reason) = reason {
            line["reason"] = json!(reason);
        }
        line.to_string()
    };
    let lines = vec![
        lane(1_000, 128, Some("first-turn"), 0),
        lane(1_100, 128, Some("first-turn"), 0),
        lane(1_200, 128, Some("first-turn"), 0),
        // 有理論值且記成 hit（≥ 九成）：照舊
        lane(1_300, 128, None, 130),
    ];
    let report = report(&lines, Some("w1"));
    assert_eq!(
        chips(&report),
        vec![("hit".to_owned(), 1), ("vendor-only".to_owned(), 3)]
    );
}

/// 同一主機不同連接埠（例：本機兩個不同的相容服務）是兩組，底線不互相套用
#[test]
fn same_host_different_ports_are_separate_groups() {
    let mut lines: Vec<String> = (0..3)
        .map(|i| api("localhost:1234", 5_000 + i, 64))
        .collect();
    lines.push(api("localhost:8080", 5_100, 64));
    let report = report(&lines, Some("w1"));
    assert_eq!(
        chips(&report),
        vec![("hit".to_owned(), 1), ("vendor-only-api".to_owned(), 3)]
    );
}
