use super::capacity::{Calibration, ModelCapacity};
use super::limits::Limit;
use super::*;
use crate::data::TranscriptEvent;

fn limit(unit: Unit, total: u64, reserve: u64, reliable: bool, ratio: Option<f64>) -> Limit {
    let capacity = ratio.map(|ratio| {
        let mut entry = ModelCapacity::default();
        entry.calibration.insert(
            "summary".into(),
            Calibration {
                estimate: 1000,
                actual: (1000.0 * ratio) as u64,
                model_id: None,
            },
        );
        entry
    });
    Limit {
        unit,
        total,
        reserve,
        reliable,
        model: "m".into(),
        transport: "t".into(),
        capacity,
    }
}

#[test]
fn hint_threshold_includes_equality_and_fixed_part_is_subtracted_once() {
    // L=1000、F=200、R=100 → cap=700；0.8×700＝560
    let l = limit(Unit::Bytes, 1000, 100, true, None);
    let at = path_budget(
        &l,
        "summary",
        Raw {
            full: 760,
            fixed: 200,
        },
        None,
    );
    assert_eq!((at.used, at.cap, at.hint), (560, 700, true));
    let below = path_budget(
        &l,
        "summary",
        Raw {
            full: 759,
            fixed: 200,
        },
        None,
    );
    assert!(!below.hint);
    // F 很大（大世界書的 system）：只從 L 扣一次，H 不含 F
    let big_fixed = path_budget(
        &l,
        "summary",
        Raw {
            full: 900,
            fixed: 850,
        },
        None,
    );
    assert_eq!((big_fixed.used, big_fixed.cap), (50, 50));
    assert!(big_fixed.hint);
    // F＋R 超過 L：cap 0，必提醒
    let none = path_budget(
        &l,
        "summary",
        Raw {
            full: 950,
            fixed: 950,
        },
        None,
    );
    assert_eq!((none.cap, none.hint), (0, true));
}

#[test]
fn reported_total_only_raises_the_estimate() {
    let l = limit(Unit::Tokens, 10_000, 1_000, true, Some(1.0));
    let raw = Raw {
        full: 3_000,
        fixed: 1_000,
    };
    assert_eq!(path_budget(&l, "summary", raw, Some(6_000)).used, 5_000);
    assert_eq!(path_budget(&l, "summary", raw, Some(100)).used, 2_000);
}

#[test]
fn calibration_scales_both_full_and_fixed() {
    let l = limit(Unit::Tokens, 10_000, 0, true, Some(1.5));
    let budget = path_budget(
        &l,
        "summary",
        Raw {
            full: 2_000,
            fixed: 1_000,
        },
        None,
    );
    assert_eq!((budget.used, budget.cap), (1_500, 8_500));
    // 別的路徑種類沒有校正：不套用摘要的倍率
    assert_eq!(
        path_budget(
            &l,
            "gm",
            Raw {
                full: 2_000,
                fixed: 1_000
            },
            None
        )
        .ratio,
        None
    );
}

fn summary(used: u64, cap: u64, g_reply: u64, lockable: bool) -> SummaryBudget {
    SummaryBudget {
        path: PathBudget {
            unit: Unit::Bytes,
            used,
            cap,
            hint: true,
            ratio: Some(1.0),
            reliable: true,
        },
        lockable,
        g_reply,
        over: used > cap,
    }
}

#[test]
fn lock_is_strictly_greater_and_needs_lockable() {
    let s = summary(900, 1_000, 50, true);
    assert!(!s.would_overflow(50)); // 剛好塞滿：不鎖
    assert!(s.would_overflow(51)); // 差一：鎖
                                   // 無玩家句的動作：本句 0
    assert!(!summary(900, 1_000, 100, true).would_overflow(0));
    assert!(summary(901, 1_000, 100, true).would_overflow(0));
    // 來源不可靠／沒校正：永不鎖
    assert!(!summary(5_000, 1_000, 100, false).would_overflow(1_000));
}

#[test]
fn draft_size_counts_text_plus_fixed_wrapping() {
    assert_eq!(draft_size("", Unit::Bytes, 1.0), 0);
    assert_eq!(
        draft_size("測a", Unit::Bytes, 1.0),
        4 + DRAFT_OVERHEAD_BYTES
    );
    // 前端 scene-budget.test.ts 用同一組數字對拍
    assert_eq!(budget_tokens("雷恩說：Let's go! {\"hp\":3}"), 15);
    assert_eq!(
        draft_size("雷恩說：Let's go! {\"hp\":3}", Unit::Tokens, 1.5),
        23 + DRAFT_OVERHEAD_TOKENS
    );
}

/// (是不是玩家句, action_id, 量)
fn sized<'a>(events: &[(bool, Option<&'a str>, u64)]) -> Vec<ReplySize<'a>> {
    events
        .iter()
        .map(|&(player, action, size)| ReplySize {
            player,
            action,
            size,
        })
        .collect()
}

#[test]
fn reply_prediction_sums_every_reply_of_an_action_and_excludes_the_player_line() {
    // 舊事件（沒有 action_id）：玩家 → 三位角色接力（共 3000）→ 玩家 → 一則 1000
    let sizes = sized(&[
        (true, None, 50_000),
        (false, None, 1_000),
        (false, None, 1_000),
        (false, None, 1_000),
        (true, None, 50_000),
        (false, None, 1_000),
    ]);
    assert_eq!(predict_reply(&sizes, Unit::Bytes), 4_500.max(3_600));
    let big = sized(&[
        (true, None, 10),
        (false, None, 10_000),
        (false, None, 5_000),
        (true, None, 10),
        (false, None, 100),
    ]);
    assert_eq!(predict_reply(&big, Unit::Bytes), 18_000);
    // 新事件：玩家句與它的回覆同一個 id，玩家句照樣不算
    let tagged = sized(&[
        (true, Some("a"), 50_000),
        (false, Some("a"), 10_000),
        (false, Some("a"), 5_000),
    ]);
    assert_eq!(predict_reply(&tagged, Unit::Bytes), 18_000);
    // 沒有歷史：下限
    assert_eq!(predict_reply(&[], Unit::Tokens), 1_500);
    assert_eq!(
        predict_reply(&sized(&[(true, None, 99)]), Unit::Bytes),
        4_500
    );
    // 開頭沒有玩家句的回覆（開場白、無玩家句的動作）也算一段
    assert_eq!(
        predict_reply(&sized(&[(false, None, 5_000)]), Unit::Bytes),
        6_000
    );
}

/// 反例〔作者裁決 2026-10-06〕：連續多次推進／旁白／點名沒有玩家句時，每個動作各自一段，
/// 預測是單一動作的最大量，不是全部加總（舊算法會把 5 次推進併成 50,000 ×1.2）。
#[test]
fn consecutive_actions_without_player_lines_are_separate() {
    let mut events = vec![(true, Some("p"), 100), (false, Some("p"), 3_000)];
    for id in ["n1", "n2", "n3", "n4", "n5"] {
        // 一次推進：旁白＋狀態更新＋點名＋角色接話，同一個 id
        events.push((false, Some(id), 6_000));
        events.push((false, Some(id), 500));
        events.push((false, Some(id), 0));
        events.push((false, Some(id), 3_500));
    }
    // 點名一位角色發言、GM 旁白各一次
    events.push((false, Some("call"), 2_000));
    events.push((false, Some("narrate"), 4_000));
    assert_eq!(predict_reply(&sized(&events), Unit::Bytes), 12_000);
    // 動作中途沒帶 id 的附屬事件（後端代落等）併進目前那個動作
    let mixed = sized(&[
        (false, Some("n1"), 6_000),
        (false, None, 4_000),
        (false, Some("n2"), 1_000),
    ]);
    assert_eq!(predict_reply(&mixed, Unit::Bytes), 12_000);
    // 舊段（沒 id）之後接新動作：新動作自成一段，不併進舊段
    let upgraded = sized(&[
        (true, None, 10),
        (false, None, 10_000),
        (false, Some("n1"), 1_000),
        (false, Some("n2"), 1_000),
    ]);
    assert_eq!(predict_reply(&upgraded, Unit::Bytes), 12_000);
}

#[test]
fn only_the_last_ten_actions_count() {
    let mut sizes = vec![(true, None, 1), (false, None, 100_000)];
    for _ in 0..10 {
        sizes.push((true, None, 1));
        sizes.push((false, None, 2_000));
    }
    assert_eq!(predict_reply(&sized(&sizes), Unit::Bytes), 4_500);
    // 新事件同理：最近 10 個動作（無玩家句也各算一個）之外的大回覆不算
    let ids: Vec<String> = (0..10).map(|n| format!("n{n}")).collect();
    let mut tagged = vec![(false, Some("old"), 100_000)];
    tagged.extend(ids.iter().map(|id| (false, Some(id.as_str()), 2_000)));
    assert_eq!(predict_reply(&sized(&tagged), Unit::Bytes), 4_500);
}

fn event(kind: TranscriptKind, name: &str, text: &str) -> TranscriptEvent {
    TranscriptEvent {
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
        raw: None,
        ts: "2026-10-06T08:00:00+08:00".to_owned(),
        speaker_id: String::new(),
        speaker_name: name.to_owned(),
        kind,
        text: text.to_owned(),
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
    }
}

fn world_with(events: &[TranscriptEvent], tag: &str) -> (std::path::PathBuf, String) {
    let root = std::env::temp_dir().join(format!("tt-scene-budget-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let world = data::create_world(&root, "容量測試").unwrap();
    for event in events {
        data::append_transcript(&root, &world, 0, event).unwrap();
    }
    (root, world)
}

fn agy_config() -> AppConfig {
    let mut config = AppConfig::default();
    config
        .preferences
        .insert("transport".into(), serde_json::json!("agy"));
    config
}

/// agy 換幕量的是最終 stdin body：H＝(有事件的 body − 沒事件的 body) 逐 byte 一致
#[test]
fn agy_summary_measures_the_exact_stdin_body() {
    let events = vec![
        event(TranscriptKind::Player, "玩家", "我推開門。"),
        event(TranscriptKind::Narration, "GM", &"雨聲。".repeat(500)),
    ];
    let (root, world) = world_with(&events, "agy");
    let config = agy_config();
    let lang = crate::transport::ui_language(&config);
    let materials = measure::load(&root, &world).unwrap();
    let store = capacity::Store::new();
    let catalog = Default::default();
    let sources = limits::Sources {
        capacity: &store,
        catalog: &catalog,
        codex_cache: None,
        codex_config: None,
        grok_windows: None,
        smart_free_context: None,
    };
    let budget = compute_with(&config, &lang, &root, &world, &materials, &sources, true);
    let summary = budget.summary.unwrap();
    let body = |events: &[TranscriptEvent]| {
        let messages = crate::transport::summary_messages(events, &lang, false);
        let (system, prompt) = crate::cli::flatten_messages(
            "GM",
            crate::transport::summary_closing(&lang),
            &messages,
            &lang,
        );
        crate::cli::agy_body(&system, &prompt).len() as u64
    };
    let stored = data::read_transcript(&root, &world, 0).unwrap();
    assert_eq!(summary.path.used, body(&stored) - body(&[]));
    assert_eq!(summary.path.cap, limits::AGY_BODY_BYTES - body(&[]));
    assert!(summary.lockable);
    assert!(!summary.path.hint);
    std::fs::remove_dir_all(&root).unwrap();
}

/// 世界書很大時，聊天路徑（system 含世界書）比換幕先到提醒門檻
#[test]
fn big_system_makes_chat_hint_before_summary_hint() {
    let events = vec![
        event(TranscriptKind::Player, "玩家", "我推開門。"),
        event(TranscriptKind::Narration, "GM", &"雨聲。".repeat(3000)),
    ];
    let (root, world) = world_with(&events, "chat");
    data::write_world_md(&root, &world, &"世界觀。".repeat(36_000)).unwrap();
    let config = agy_config();
    let lang = crate::transport::ui_language(&config);
    let materials = measure::load(&root, &world).unwrap();
    let store = capacity::Store::new();
    let catalog = Default::default();
    let sources = limits::Sources {
        capacity: &store,
        catalog: &catalog,
        codex_cache: None,
        codex_config: None,
        grok_windows: None,
        smart_free_context: None,
    };
    let budget = compute_with(&config, &lang, &root, &world, &materials, &sources, true);
    assert!(budget.chat_hint);
    assert!(!budget.summary.unwrap().path.hint);
    std::fs::remove_dir_all(&root).unwrap();
}

fn snapshot(scene: u64, gen: &str, used: u64) -> gate::Snapshot {
    gate::Snapshot {
        epoch: 0,
        scene,
        config_gen: gen.to_owned(),
        summary: Some(summary(used, 10_000, 5_000, true)),
    }
}

/// 同一動作只算一次：玩家句落檔後 H 變大，同 action 的回覆不被錯擋；別的動作、換幕、改設定都要重算
#[test]
fn action_receipt_prevents_double_counting_but_expires_on_change() {
    let world = "receipt-world";
    gate::clear_for_test(world);
    // 落玩家句：4000＋本句（100＋64）＋5000 ≤ 10000，通過並發收據
    assert!(gate::admit(
        world,
        Some("a1"),
        &"x".repeat(100),
        &snapshot(0, "g", 4_000)
    ));
    // 玩家句已落檔（H 變 5000＋），同一動作的回覆：再加完整 G 會超過，但有收據不重扣
    assert!(gate::admit(world, Some("a1"), "", &snapshot(0, "g", 5_500)));
    // 多角色接力的第二、三則回覆同樣放行
    assert!(gate::admit(world, Some("a1"), "", &snapshot(0, "g", 7_000)));
    // 新動作：重算，5500＋5000 > 10000，擋
    assert!(!gate::admit(
        world,
        Some("a2"),
        "",
        &snapshot(0, "g", 5_500)
    ));
    // 換了幕或設定世代：舊收據作廢
    assert!(!gate::admit(
        world,
        Some("a1"),
        "",
        &snapshot(1, "g", 5_500)
    ));
    assert!(!gate::admit(
        world,
        Some("a1"),
        "",
        &snapshot(0, "g2", 5_500)
    ));
    // 擋下不發收據：同一動作再來一次照樣擋
    assert!(!gate::admit(
        world,
        Some("a1"),
        "",
        &snapshot(0, "g2", 5_500)
    ));
    // 沒帶 action id（舊呼叫端）：每次都重算
    assert!(gate::admit(world, None, "", &snapshot(0, "g", 4_000)));
    assert!(!gate::admit(world, None, "", &snapshot(0, "g", 5_500)));
    gate::clear_for_test(world);
}

#[test]
fn unlockable_or_unknown_capacity_never_blocks() {
    let world = "receipt-unknown";
    gate::clear_for_test(world);
    let mut unknown = snapshot(0, "g", 99_999);
    unknown.summary = None;
    assert!(gate::admit(
        world,
        Some("a"),
        &"x".repeat(1_000_000),
        &unknown
    ));
    let mut unreliable = snapshot(0, "g", 99_999);
    unreliable.summary = Some(summary(99_999, 10_000, 5_000, false));
    assert!(gate::admit(world, Some("b"), "x", &unreliable));
    gate::clear_for_test(world);
}

/// 後端關卡讀真檔：agy 長幕被擋、逐字稿一個字都沒多
#[test]
fn check_capacity_blocks_a_full_agy_scene_without_writing() {
    let events = vec![
        event(TranscriptKind::Player, "玩家", "開始。"),
        event(TranscriptKind::Narration, "GM", &"雨".repeat(60_000)),
    ];
    let (root, world) = world_with(&events, "gate");
    std::fs::write(
        root.join("config.json"),
        serde_json::to_string(&agy_config()).unwrap(),
    )
    .unwrap();
    let before = data::read_transcript(&root, &world, 0).unwrap().len();
    // 本幕約 180KB：再加回覆預測（≥ 216KB×…）一定超過 190KB
    let blocked = check_capacity(&root, &root, &world, Some("act"), "我再說一句。");
    assert!(blocked.unwrap_err().contains("scene_capacity_full"));
    assert_eq!(
        data::read_transcript(&root, &world, 0).unwrap().len(),
        before
    );
    std::fs::remove_dir_all(&root).unwrap();
}

/// Sol 探針重現的兩條：退回再進同一幕（幕號、設定都一樣）與沒帶 id 的新動作之後，舊收據不能復活
#[test]
fn receipts_die_on_epoch_change_and_on_any_later_action() {
    let world = "receipt-revive";
    gate::clear_for_test(world);
    let at = |epoch: u64, used: u64| gate::Snapshot {
        epoch,
        ..snapshot(0, "g", used)
    };
    assert!(gate::admit(world, Some("a1"), "", &at(7, 4_000)));
    // 退回再換回同一幕：幕號與設定指紋相同，但容量世代推進過
    assert!(!gate::admit(world, Some("a1"), "", &at(8, 5_500)));
    // 設定 A→B→A 同理（世代推進兩格，指紋回到 g）
    assert!(gate::admit(world, Some("a2"), "", &at(9, 4_000)));
    assert!(!gate::admit(world, Some("a2"), "", &at(11, 5_500)));
    // 沒帶 id 的新動作通過後，舊動作的收據被撤掉
    assert!(gate::admit(world, Some("a3"), "", &at(12, 4_000)));
    assert!(gate::admit(world, None, "", &at(12, 4_000)));
    assert!(!gate::admit(world, Some("a3"), "", &at(12, 5_500)));
    gate::clear_for_test(world);
}

/// 上一回合待落的 GM 正文算進本幕：容量關卡排在代落之前也不漏量，擋下時什麼都沒寫（待落的仍待落）
#[test]
fn pending_gm_text_is_counted_and_a_refusal_writes_nothing() {
    let events = vec![
        event(TranscriptKind::Player, "玩家", "開始。"),
        event(TranscriptKind::Narration, "GM", &"雨".repeat(30_000)),
    ];
    let (root, world) = world_with(&events, "pending");
    std::fs::write(
        root.join("config.json"),
        serde_json::to_string(&agy_config()).unwrap(),
    )
    .unwrap();
    let used = |root: &std::path::Path| {
        compute(root, root, &world, &agy_config(), false)
            .unwrap()
            .summary
            .unwrap()
            .path
            .used
    };
    let without = used(&root);
    // 上一個 GM 回合中止留下約 60KB 半截，前端沒落成
    data::state_commit::with_commit(&root, &world, |tx| {
        data::message_vars::begin_turn(tx, "t1", None).unwrap();
        data::message_vars::finish_turn(
            tx,
            "t1",
            Some(data::message_vars::PendingMain {
                text: "風".repeat(20_000),
                raw: None,
                truncated: true,
            }),
        )
        .unwrap();
    });
    let with = used(&root);
    assert!(with >= without + 60_000, "{without} -> {with}");
    let before = data::read_transcript(&root, &world, 0).unwrap().len();
    let refused = check_capacity(&root, &root, &world, Some("next"), "再一句。");
    assert!(refused.unwrap_err().contains("scene_capacity_full"));
    assert_eq!(
        data::read_transcript(&root, &world, 0).unwrap().len(),
        before
    );
    assert!(data::state_commit::with_commit(
        &root,
        &world,
        data::message_vars::has_unlanded_reply
    ));
    std::fs::remove_dir_all(&root).unwrap();
}

fn config_for(transport: &str) -> AppConfig {
    let mut config = AppConfig::default();
    config
        .preferences
        .insert("transport".into(), serde_json::json!(transport));
    config
}

fn gm_path(root: &std::path::Path, world: &str, transport: &str) -> measure::Request {
    let config = config_for(transport);
    let lang = crate::transport::ui_language(&config);
    let materials = measure::load(root, world).unwrap();
    let provider = crate::transport::dispatch::lane_provider(&config);
    measure::chat_paths(
        root,
        world,
        &materials,
        crate::transport::gm_tier(&config),
        provider,
        &lang,
        transport,
    )
    .remove(0)
    .request_full
}

fn text_of(request: &measure::Request) -> String {
    match request {
        measure::Request::Cli { system, prompt } => format!("{system}\n\n{prompt}"),
        measure::Request::Api(messages) => messages
            .iter()
            .map(|m| m.content.clone())
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// 聊天量測與實送同一組裝：GM 指示四態跟著桌況走；codex 量的是 dispatch 攤平後的那份
#[test]
fn chat_measure_uses_the_real_gm_assembly_for_every_format_and_codex_flatten() {
    let events = vec![
        event(TranscriptKind::Player, "玩家", "開始。"),
        event(TranscriptKind::Narration, "GM", "雨。"),
    ];
    let (root, world) = world_with(&events, "assembly");
    let config = config_for("codex");
    let lang = crate::transport::ui_language(&config);
    let materials = crate::chat_assembly::gm_materials(&root, &world).unwrap();
    let (scope, _) = crate::chat_assembly::gm_scope(&materials);
    let (instruction, closing) =
        crate::chat_assembly::gm_instruction(&root, &world, &materials, &lang);
    let narration = instruction.content.clone();
    // codex：與 dispatch 同一個 flatten（label GM＋本輪收尾句）
    let (system, prompt) = crate::cli::flatten_messages(
        "GM",
        closing,
        &crate::chat_assembly::gm_messages(&materials, &scope, instruction, &lang),
        &lang,
    );
    match gm_path(&root, &world, "codex") {
        measure::Request::Cli {
            system: measured_system,
            prompt: measured_prompt,
        } => {
            assert_eq!(measured_system, system);
            assert_eq!(measured_prompt, prompt);
        }
        measure::Request::Api(_) => panic!("codex 要量攤平後的輸入"),
    }
    // API：訊息本身
    assert!(matches!(
        gm_path(&root, &world, "api"),
        measure::Request::Api(_)
    ));
    assert!(text_of(&gm_path(&root, &world, "claude")).contains(&narration));
    // 介面接管桌：指示換成接管版，量測跟著換
    data::write_interface_shell(&root, &world, "<div>shell</div>").unwrap();
    let materials = crate::chat_assembly::gm_materials(&root, &world).unwrap();
    let (takeover, _) = crate::chat_assembly::gm_instruction(&root, &world, &materials, &lang);
    assert_ne!(takeover.content, narration);
    for transport in ["claude", "agy", "codex", "api"] {
        let measured = text_of(&gm_path(&root, &world, transport));
        assert!(measured.contains(&takeover.content), "{transport}");
        assert!(!measured.contains(&narration), "{transport}");
    }
    std::fs::remove_dir_all(&root).unwrap();
}

/// 角色線量測與實送同一組開關（lanes::chars_lane_shape）：claude 單角色私設進 system、
/// 多角色與 grok 私設在 tail 且只算一次；agy 線名分角色。
#[test]
fn character_measure_follows_lane_shape() {
    let events = vec![event(TranscriptKind::Player, "玩家", "開始。")];
    let (root, world) = world_with(&events, "lane-shape");
    let card = |id: &str, name: &str, private: &str| data::CharacterCard {
        id: id.to_owned(),
        name: name.to_owned(),
        color: "#336699".to_owned(),
        avatar: String::new(),
        tier: data::Tier::Balanced,
        show_image: true,
        archived: false,
        gen_prompt: String::new(),
        public_md: format!("{name}的公開設定。"),
        private_md: private.to_owned(),
    };
    let fox_id = ulid::Ulid::generate().to_string();
    data::write_character(&root, &world, &card(&fox_id, "狐狸", "狐狸的秘密甲")).unwrap();
    let chars_path = |transport: &str| {
        let config = config_for(transport);
        let lang = crate::transport::ui_language(&config);
        let materials = measure::load(&root, &world).unwrap();
        let provider = crate::transport::dispatch::lane_provider(&config);
        measure::chat_paths(
            &root,
            &world,
            &materials,
            crate::transport::gm_tier(&config),
            provider,
            &lang,
            transport,
        )
        .into_iter()
        .find(|path| {
            path.kind == "chars"
                && matches!(&path.request_full, measure::Request::Cli { system, .. }
                    if system.contains("狐狸") && !system.contains("「騎士」的私"))
                && match &path.request_full {
                    measure::Request::Cli { prompt, .. } => prompt.contains("現在你是「狐狸」"),
                    measure::Request::Api(_) => false,
                }
        })
        .unwrap()
    };
    let split = |path: &measure::ChatPath| match &path.request_full {
        measure::Request::Cli { system, prompt } => (system.clone(), prompt.clone()),
        measure::Request::Api(_) => panic!("lane 後端是 CLI 形狀"),
    };
    let secret = "狐狸的秘密甲";
    // claude 單角色：私設進 system、不在 prompt
    let (system, prompt) = split(&chars_path("claude"));
    assert!(system.contains(secret));
    assert_eq!(prompt.matches(secret).count(), 0);
    // agy：私設進 system、線名分角色
    let agy = chars_path("agy");
    assert!(split(&agy).0.contains(secret));
    assert_eq!(
        agy.lane,
        Some((crate::lanes::Lane::Chars, Some(fox_id.clone())))
    );
    // grok：私設在 tail 只算一次、線名不分角色
    let grok = chars_path("grok");
    let (system, prompt) = split(&grok);
    assert!(!system.contains(secret));
    assert_eq!(prompt.matches(secret).count(), 1);
    assert_eq!(grok.lane, Some((crate::lanes::Lane::Chars, None)));
    // 多一張在場卡：claude 改成多角色，私設回到 tail、只算一次
    let knight_id = ulid::Ulid::generate().to_string();
    data::write_character(&root, &world, &card(&knight_id, "騎士", "")).unwrap();
    let (system, prompt) = split(&chars_path("claude"));
    assert!(!system.contains(secret));
    assert_eq!(prompt.matches(secret).count(), 1);
    std::fs::remove_dir_all(&root).unwrap();
}

/// 卡片自帶介面的兩態：格式條目全文不在提示裡＝中性版（CardFormatAbsent），
/// 世界書加上提到同款標籤的常駐條目＝點名版（CardFormat）；各後端量測都跟著換
#[test]
fn chat_measure_follows_card_format_and_card_format_absent() {
    let events = vec![
        event(TranscriptKind::Player, "玩家", "開始。"),
        event(TranscriptKind::Narration, "GM", "雨。"),
    ];
    let (root, world) = world_with(&events, "card-format");
    let card = serde_json::json!({"data": {"name": "莉亞", "extensions": {"regex_scripts": [{
        "scriptName": "顯示介面",
        "findRegex": "/<TavernUI>[\\s\\S]*?<\\/TavernUI>/s",
        "replaceString": "<div>$1</div>",
        "placement": [2]
    }]}}})
    .to_string();
    crate::import::import_character(&root, &world, card.as_bytes(), "#3366ff", "zh-TW").unwrap();
    let lang = crate::transport::ui_language(&config_for("claude"));
    let instruction = |root: &std::path::Path| {
        let materials = crate::chat_assembly::gm_materials(root, &world).unwrap();
        crate::chat_assembly::gm_instruction(root, &world, &materials, &lang)
            .0
            .content
    };
    let assert_measured = |expected: &str, absent: &str| {
        for transport in ["claude", "agy", "codex", "api"] {
            let measured = text_of(&gm_path(&root, &world, transport));
            assert!(measured.contains(expected), "{transport}");
            assert!(!measured.contains(absent), "{transport}");
        }
    };
    let neutral = crate::transport::card_format_instruction(&lang, None).content;
    let named = crate::transport::card_format_instruction(&lang, Some("輸出格式")).content;
    assert_eq!(instruction(&root), neutral);
    assert_measured(&neutral, &named);

    data::upsert_worldbook_entry(
        &root,
        &world,
        data::WorldbookEntry {
            uid: 0,
            title: "輸出格式".to_owned(),
            keys: Vec::new(),
            content: "每次回覆都包在 <TavernUI>…</TavernUI> 裡。".to_owned(),
            constant: true,
            order: 0,
            disabled: false,
            visibility: data::Visibility::Gm,
            is_person: false,
            locked: false,
        },
    )
    .unwrap();
    assert_eq!(instruction(&root), named);
    assert_measured(&named, &neutral);
    std::fs::remove_dir_all(&root).unwrap();
}

/// 前端樂觀更新成 C、磁碟還是 A：量測用的是 A，不能被當成 C 的結果
#[test]
fn snapshot_must_equal_the_frontend_config() {
    let disk = agy_config();
    let same = serde_json::to_value(&disk).unwrap();
    assert!(snapshot_matches(&disk, &same));
    let mut optimistic = disk.clone();
    optimistic
        .preferences
        .insert("transport".into(), serde_json::json!("claude"));
    assert!(!snapshot_matches(
        &disk,
        &serde_json::to_value(&optimistic).unwrap()
    ));
    assert_ne!(config_generation(&disk), config_generation(&optimistic));
}
