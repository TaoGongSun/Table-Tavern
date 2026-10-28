use super::*;

fn usage(prompt: u64, cached: u64, created: u64) -> PromptCacheUsage {
    PromptCacheUsage {
        created_1h_tokens: None,
        prompt_tokens: prompt,
        cached_tokens: Some(cached),
        created_tokens: Some(created),
        output_tokens: 20,
        cost_usd: Some(0.0031),
    }
}

fn lane(age_secs: u64, expected_cached: u64) -> LaneContext {
    LaneContext {
        lane: "chars:sonnet".to_owned(),
        reopen: None,
        patched: false,
        rebased: false,
        age_secs,
        expected_cached,
        system_tokens: 4_000,
        system_hash: text_hash("凍結素材"),
        ttl_secs: Some(300),
        stale: None,
    }
}

fn turn(roster: usize) -> PromptShape {
    PromptShape::Turn {
        roster,
        solo: roster <= 1,
    }
}

/// 快取軸是有限清單，判定只看 app 自己的決策＋數字，不猜。
#[test]
fn cache_axis_covers_each_label_by_rule() {
    // 續聊、幾乎全中
    assert_eq!(
        classify_cache(Some(&lane(30, 9_000)), &usage(9_300, 9_000, 300), "claude"),
        (Cache::Hit, None)
    );
    // 上輪送 9000，這輪只中 200＝中了但遠低於理論值
    assert_eq!(
        classify_cache(Some(&lane(30, 9_000)), &usage(9_300, 200, 9_100), "claude"),
        (Cache::Partial, Some(CacheReason::BelowExpected))
    );
    // 超過保守窗口且一個字都沒中＝歸給時間，不算故障
    assert_eq!(
        classify_cache(Some(&lane(600, 9_000)), &usage(9_300, 0, 9_300), "claude"),
        (Cache::Zero, Some(CacheReason::Expired))
    );
    // 讀寫皆 0 但上輪送過內容＝CLI 這句沒帶標記，優先於過期判定
    assert_eq!(
        classify_cache(Some(&lane(600, 9_000)), &usage(9_300, 0, 0), "claude"),
        (Cache::Zero, Some(CacheReason::Skipped))
    );
    // 上輪沒有可中量＝這輪本來就不該中
    assert_eq!(
        classify_cache(Some(&lane(30, 0)), &usage(9_300, 0, 0), "claude"),
        (Cache::NotExpected, None)
    );
    // 重開線＝我方內容不該中；一個字都沒中就是「本來就沒得中」
    let mut reopened = lane(30, 0);
    reopened.reopen = Some("scene-changed");
    assert_eq!(
        classify_cache(Some(&reopened), &usage(9_300, 0, 9_300), "claude"),
        (Cache::NotExpected, None)
    );
    // 但重開線照樣可能中到供應商自己的固定前綴——實測帳本有 11 筆 warmup 中了六到九成。
    // 數字說中了就說中了，不能因為「這輪不該中」把 95.7% 講成「本來就沒得中」
    assert_eq!(
        classify_cache(Some(&reopened), &usage(19_300, 18_499, 800), "claude"),
        (Cache::Hit, None)
    );
    // 隔了一小時但幾乎全中＝快取其實還活著，過期只是解釋沒中的理由，不是蓋章
    assert_eq!(
        classify_cache(
            Some(&lane(3_600, 30_184)),
            &usage(37_000, 30_178, 6_800),
            "claude"
        ),
        (Cache::Hit, None)
    );
    // 沒回報＝量不到，與「量到了、是 0」不同
    let mut blind = usage(9_300, 0, 0);
    blind.cached_tokens = None;
    blind.created_tokens = None;
    assert_eq!(
        classify_cache(None, &blind, "claude"),
        (Cache::Unknown, None)
    );
}

/// 本案的病灶：無狀態路徑以前在碰到快取數字之前就被判成「單發」，
/// 於是命中率過半的輪次照樣對玩家說「整包重新送出」。
#[test]
fn stateless_paths_report_real_cache_result_not_call_mode() {
    // 共線劇情輪中了八成——舊版標 single，現在快取軸誠實說 hit
    assert_eq!(
        classify_cache(None, &usage(9_300, 7_500, 1_800), "api"),
        (Cache::Hit, None)
    );
    // 有回報、值就是 0：說 zero，但不宣稱原因（無狀態算不出理論值）
    assert_eq!(
        classify_cache(None, &usage(4_411, 0, 4_411), "api"),
        (Cache::Zero, None)
    );
}

/// 「這次沒有快取」不推測原因；`skipped` 等原因只留給 claude 續聊線（顯示拍板第 2 項）。
#[test]
fn zero_reasons_belong_to_claude_lanes_only() {
    let mut agy_zero = usage(9_300, 0, 0);
    agy_zero.created_tokens = None; // agy／grok 不回報寫入量
    for transport in ["agy", "grok"] {
        // 續聊讀 0：近、遠、帶寫入量都一樣只說 zero，不帶原因
        for (age, sample) in [
            (30, &agy_zero),
            (600, &agy_zero),
            (30, &usage(9_300, 0, 9_300)),
        ] {
            assert_eq!(
                classify_cache(Some(&lane(age, 9_000)), sample, transport),
                (Cache::Zero, None),
                "{transport} age {age}"
            );
        }
        // 首輪與重開讀 0＝新桌／新線
        assert_eq!(
            classify_cache(Some(&lane(30, 0)), &agy_zero, transport),
            (Cache::NotExpected, None)
        );
        let mut reopened = lane(30, 9_000);
        reopened.reopen = Some("scene-changed");
        assert_eq!(
            classify_cache(Some(&reopened), &agy_zero, transport),
            (Cache::NotExpected, None)
        );
        // 缺快取欄仍是量不到
        let mut blind = agy_zero;
        blind.cached_tokens = None;
        assert_eq!(
            classify_cache(Some(&lane(30, 9_000)), &blind, transport),
            (Cache::Unknown, None)
        );
        // 中了但不足九成是數字診斷，不屬「值為 0」，照舊帶原因
        assert_eq!(
            classify_cache(Some(&lane(30, 9_000)), &usage(9_300, 200, 0), transport),
            (Cache::Partial, Some(CacheReason::BelowExpected))
        );
    }
    // claude 續聊讀 0 照舊：skipped／expired／below-expected
    assert_eq!(
        classify_cache(Some(&lane(30, 9_000)), &usage(9_300, 0, 0), "claude"),
        (Cache::Zero, Some(CacheReason::Skipped))
    );
    assert_eq!(
        classify_cache(Some(&lane(600, 9_000)), &usage(9_300, 0, 9_300), "claude"),
        (Cache::Zero, Some(CacheReason::Expired))
    );
    assert_eq!(
        classify_cache(Some(&lane(30, 9_000)), &usage(9_300, 0, 9_300), "claude"),
        (Cache::Zero, Some(CacheReason::BelowExpected))
    );
    // 無 lane（API、codex、claude 一次性呼叫）讀 0：zero 不帶原因，不推測新桌
    for transport in ["api", "codex", "claude"] {
        assert_eq!(
            classify_cache(None, &usage(4_411, 0, 4_411), transport),
            (Cache::Zero, None)
        );
    }
    // 落帳行：agy 零命中沒有 cache_reason 欄
    let line = call_fields(
        Some("w1"),
        "agy",
        "gemini",
        Some(&lane(30, 9_000)),
        PromptShape::Oneshot,
        agy_zero,
    );
    assert_eq!(line["cache"], json!("zero"));
    assert!(line.get("cache_reason").is_none());
}

/// 「隔太久」看呼叫前採用的壽命：1 小時線閒置 6 分鐘沒中滿，不能歸給過期；
/// 舊列（沒記壽命）照 5 分鐘解釋。
#[test]
fn expiry_reason_follows_the_ttl_adopted_before_the_call() {
    let mut hour = lane(400, 9_000);
    hour.ttl_secs = Some(3600);
    assert_eq!(short_reason(&hour), CacheReason::BelowExpected);
    hour.age_secs = 3601;
    assert_eq!(short_reason(&hour), CacheReason::Expired);
    let mut legacy = lane(301, 9_000);
    legacy.ttl_secs = None;
    assert_eq!(short_reason(&legacy), CacheReason::Expired);
    legacy.age_secs = 300;
    assert_eq!(short_reason(&legacy), CacheReason::BelowExpected);
}

/// 1h 拆分有回報才落帳；沒有就不寫欄位（時效未知）。
#[test]
fn one_hour_split_is_logged_only_when_reported() {
    let mut usage = usage(10_000, 0, 9_000);
    usage.created_1h_tokens = Some(9_000);
    let line = call_fields(
        None,
        "claude",
        "sonnet",
        Some(&lane(0, 0)),
        PromptShape::Oneshot,
        usage,
    );
    assert_eq!(line["created_1h_tokens"], json!(9_000));
    assert_eq!(line["ttl_secs"], json!(300));
    usage.created_1h_tokens = None;
    let line = call_fields(
        None,
        "claude",
        "sonnet",
        Some(&lane(0, 0)),
        PromptShape::Oneshot,
        usage,
    );
    assert!(line.get("created_1h_tokens").is_none());
}

/// mode 只描述形狀，與快取結果互不干涉。
#[test]
fn mode_separates_call_shape_from_cache_result() {
    assert_eq!(classify_mode(Some(&lane(30, 9_000)), turn(4)), Mode::Resume);
    assert_eq!(classify_mode(None, turn(4)), Mode::Shared);
    // 天然單角色桌與（日後）策略退回同樣是 solo，靠 roster_size 分辨
    assert_eq!(classify_mode(None, turn(1)), Mode::Solo);
    assert_eq!(
        classify_mode(
            None,
            PromptShape::Turn {
                roster: 4,
                solo: true
            }
        ),
        Mode::Solo
    );
    assert_eq!(classify_mode(None, PromptShape::Oneshot), Mode::Oneshot);
}

/// 量不到的那一輪：cache_reporting 標 absent，快取數字與命中率**一個欄位都不寫**。
/// 寫成 0 會被報表讀成「量到了、沒中」——API 路的假 0.0% 就是這樣來的。
#[test]
fn unreported_cache_writes_no_number_fields() {
    let dir = std::env::temp_dir().join(format!("tt-usage-absent-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("prompt-cache.jsonl");

    append_call(
        &path,
        None,
        "api",
        "vendor/model",
        None,
        turn(3),
        PromptCacheUsage {
            created_1h_tokens: None,
            prompt_tokens: 4_690,
            cached_tokens: None,
            created_tokens: None,
            output_tokens: 551,
            cost_usd: None,
        },
        &Map::new(),
    );
    // 對照組：同一條路回報了、值是 0＝量到了沒中，欄位照寫
    append_call(
        &path,
        None,
        "api",
        "vendor/model",
        None,
        turn(3),
        PromptCacheUsage {
            created_1h_tokens: None,
            prompt_tokens: 2_495,
            cached_tokens: Some(0),
            created_tokens: None,
            output_tokens: 16,
            cost_usd: None,
        },
        &Map::new(),
    );
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    let lines: Vec<&str> = text.lines().collect();

    let absent: Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(absent["cache_reporting"], json!("absent"));
    assert!(absent.get("cached_tokens").is_none());
    assert!(absent.get("created_tokens").is_none());
    assert!(absent.get("hit_rate").is_none());
    assert_eq!(absent["prompt_tokens"], json!(4_690)); // 花費照記，只是快取看不見

    let reported: Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(reported["cache_reporting"], json!("reported"));
    assert_eq!(reported["cached_tokens"], json!(0));
    assert_eq!(reported["hit_rate"], json!(0.0));
    assert!(reported.get("created_tokens").is_none()); // 寫入數沒回報就不寫
}

#[test]
fn call_line_is_one_json_object_with_lane_and_cost() {
    let dir = std::env::temp_dir().join(format!("tt-usage-log-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("prompt-cache.jsonl");

    let mut patched = lane(42, 9_000);
    patched.patched = true;
    append_call(
        &path,
        Some("w1"),
        "claude",
        "sonnet",
        Some(&patched),
        turn(3),
        usage(9_300, 9_000, 300),
        &Map::new(),
    );
    append_event(
        &path,
        "claude",
        Some("w1"),
        "chars:sonnet",
        Event::DropLane,
        "rewrite-failed",
    );
    append_drop(
        &path,
        "grok",
        Some("w1"),
        "chars:grok-4.6",
        "rewrite-failed",
        "find-segment",
        "找不到含指定片段的 user 對話行",
    );

    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3);

    let call: Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(call["ts"].as_str().unwrap().len(), 19); // 秒級：分不出五分鐘過期線就沒用
    assert_eq!(call["transport"], json!("claude"));
    assert_eq!(call["world"], json!("w1"));
    assert_eq!(call["lane"], json!("chars:sonnet"));
    assert_eq!(call["mode"], json!("resume"));
    assert_eq!(call["cache"], json!("hit"));
    assert!(call.get("cache_reason").is_none()); // 中了就不必解釋為什麼沒中
    assert!(call.get("roster_size").is_none()); // 續聊線沒交代過角色數，就不寫
    assert_eq!(call["prompt_tokens"], json!(9_300));
    assert_eq!(call["cached_tokens"], json!(9_000));
    assert_eq!(call["output_tokens"], json!(20));
    assert_eq!(call["hit_rate"], json!(96.8));
    assert_eq!(call["cost_usd"], json!(0.0031));
    assert_eq!(call["expected_cached"], json!(9_000));
    assert_eq!(call["age_secs"], json!(42));
    assert_eq!(call["patched"], json!(true));
    assert!(call.get("rebased").is_none()); // 沒發生的旗標不佔位
    assert!(call.get("reason").is_none());

    let event: Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(event["event"], json!("drop-lane"));
    // 事件行不偽造 mode／cache，否則快取統計會憑空多一筆
    assert!(event.get("mode").is_none() && event.get("cache").is_none());
    assert_eq!(event["reason"], json!("rewrite-failed"));
    assert_eq!(event["transport"], json!("claude"));
    assert!(event.get("prompt_tokens").is_none());
    assert!(event.get("stage").is_none()); // 沒有原因的事件不偽造 stage

    let drop: Value = serde_json::from_str(lines[2]).unwrap();
    assert_eq!(drop["event"], json!("drop-lane"));
    assert_eq!(drop["reason"], json!("rewrite-failed")); // reason 不變，舊讀法照用
    assert_eq!(drop["transport"], json!("grok")); // 記實際 provider，不寫死 claude
    assert_eq!(drop["stage"], json!("find-segment"));
    assert_eq!(drop["detail"], json!("找不到含指定片段的 user 對話行"));
    assert!(drop.get("mode").is_none() && drop.get("cache").is_none());
}

#[test]
fn image_calls_are_marked_without_losing_usage() {
    let dir = std::env::temp_dir().join(format!("tt-usage-image-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("prompt-cache.jsonl");
    append_call(
        &path,
        Some("w1"),
        "agy",
        "gemini",
        None,
        PromptShape::Image,
        usage(8_000, 7_000, 0),
        &Map::new(),
    );
    let line: Value = serde_json::from_str(std::fs::read_to_string(&path).unwrap().trim()).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(line["purpose"], json!("image"));
    assert_eq!(line["prompt_tokens"], json!(8_000));
    assert_eq!(line["cached_tokens"], json!(7_000));
    assert_eq!(line["cost_usd"], json!(0.0031));
}

/// 開桌生成落檔時桌還沒建出來；桌一建好，那幾行要認到這桌名下，已經有桌的不受影響。
#[test]
fn pending_lines_are_claimed_by_the_table_they_created() {
    let dir = std::env::temp_dir().join(format!("tt-usage-claim-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("prompt-cache.jsonl");

    append_call(
        &path,
        None,
        "claude",
        "opus",
        None,
        PromptShape::Oneshot,
        usage(500, 0, 0),
        &Map::new(),
    ); // 開桌大綱
    append_call(
        &path,
        None,
        "claude",
        "opus",
        None,
        PromptShape::Oneshot,
        usage(900, 0, 0),
        &Map::new(),
    ); // 開桌展開
    append_call(
        &path,
        Some("w9"),
        "claude",
        "sonnet",
        None,
        turn(2),
        usage(100, 0, 0),
        &Map::new(),
    ); // 別桌，不該被動到
    assign_pending_world(&path, "w1");

    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    let worlds: Vec<String> = text
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap()["world"].to_string())
        .collect();
    assert_eq!(worlds, ["\"w1\"", "\"w1\"", "\"w9\""]);
    // 其餘欄位原樣保留
    let first: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert_eq!(first["prompt_tokens"], json!(500));
    assert_eq!(first["mode"], json!("oneshot"));
}

/// 中日韓一字約一 token、英數約四字元一 token；只求同一個數量級。
#[test]
fn token_estimate_separates_wide_and_ascii() {
    assert_eq!(estimate_tokens("你好世界"), 4);
    assert_eq!(estimate_tokens("abcdefgh"), 2);
    assert_eq!(estimate_tokens(""), 0);
    assert_ne!(text_hash("凍結A"), text_hash("凍結B"));
    assert_eq!(text_hash("凍結A"), text_hash("凍結A"));
}

/// 來源欄：取不到就不寫（歸未知組）；端點記主機、連接埠與路徑雜湊，路徑本身、query、帳密都不進帳本
#[test]
fn origin_fields_keep_only_version_and_endpoint_id() {
    let id = |url: &str| endpoint_id(url).unwrap();
    let keyed = id("https://user:secret@OpenRouter.ai:8443/api/v1?key=abc");
    assert!(keyed.starts_with("openrouter.ai:8443#"), "{keyed}");
    assert!(!keyed.contains("api") && !keyed.contains("secret") && !keyed.contains("abc"));
    // 預設連接埠補上，寫不寫 :443 是同一個端點
    assert_eq!(
        id("https://openrouter.ai/api/v1"),
        id("https://openrouter.ai:443/api/v1")
    );
    assert!(id("http://localhost:1234/v1").starts_with("localhost:1234#"));
    // 同主機同埠、不同路徑（閘道代理不同供應商）分兩組
    assert_ne!(
        id("https://gateway.example/a/v1"),
        id("https://gateway.example/b/v1")
    );
    // 同路徑帶不同 query、fragment、帳密是同一組
    assert_eq!(
        id("https://gateway.example/a/v1?key=1#x"),
        id("https://u:p@gateway.example/a/v1?key=2")
    );
    assert_eq!(endpoint_id("not a url"), None);
    let fields = origin_fields(Some(" codex-cli 0.164.0 "), Some("openrouter.ai"));
    assert_eq!(fields["cli_version"], json!("codex-cli 0.164.0"));
    assert_eq!(fields["endpoint"], json!("openrouter.ai"));
    assert!(origin_fields(Some(""), None).is_empty());

    let dir = std::env::temp_dir().join(format!("tt-usage-origin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("prompt-cache.jsonl");
    append_call(
        &path,
        Some("w1"),
        "codex",
        "gpt",
        None,
        turn(3),
        usage(1_000, 500, 0),
        &origin_fields(Some("codex-cli 0.164.0"), None),
    );
    let line: Value = serde_json::from_str(std::fs::read_to_string(&path).unwrap().trim()).unwrap();
    assert_eq!(line["cli_version"], json!("codex-cli 0.164.0"));
    assert!(line.get("endpoint").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
