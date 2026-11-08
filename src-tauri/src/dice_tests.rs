use super::*;
use crate::data::{message_vars as vars, TranscriptEvent};
use serde_json::json;

fn event(text: &str) -> TranscriptEvent {
    serde_json::from_value(
        json!({"ts":"now", "kind":"narration", "speaker_id":"", "speaker_name":"", "text":text}),
    )
    .unwrap()
}

#[test]
fn generated_faces_match_profile_and_bounds() {
    for _ in 0..20 {
        let pool = format_pool(face).unwrap();
        assert!(pool.len() <= 512);
        let rows: Vec<_> = pool.lines().collect();
        assert_eq!(rows.len(), PROFILE.len());
        for ((sides, count), row) in PROFILE.iter().zip(rows) {
            let (head, values) = row.split_once(':').unwrap();
            assert_eq!(head, format!("d{sides}"));
            let values: Vec<u32> = serde_json::from_str(values).unwrap();
            assert_eq!(values.len(), *count);
            assert!(values.iter().all(|n| (1..=*sides).contains(n)));
        }
    }
    assert!(format_pool(|_| Err("unavailable".into())).is_err());
}

#[test]
fn instructions_split_player_and_room_and_track_the_exact_block() {
    let pool = measured_pool();
    for lang in ["zh-TW", "en"] {
        let solo = instruction(&pool, false, lang);
        let room = instruction(&pool, true, lang);
        assert!(solo.contains("1d20+3"));
        assert!(!room.contains("1d20+3"));
        assert!(solo.contains("(d20#2=19)"));
        assert_ne!(solo, room);
        assert_eq!(
            retained(&format!("before\n{solo}\nafter")),
            Some(solo.as_str())
        );
    }
    assert_eq!(retained("plain instruction"), None);
}

#[test]
fn pool_follows_normal_half_pending_and_idempotent_landing() {
    let root = crate::smart_free::test_root("dice-land");
    let world = data::create_world(&root, "沃恩的骰桌").unwrap();
    assert!(!data::read_state(&root, &world).unwrap().dice_enabled);
    set_enabled(&root, &world, true, "zh-TW", false).unwrap();
    set_enabled(&root, &world, true, "zh-TW", false).unwrap();
    assert_eq!(data::read_transcript(&root, &world, 0).unwrap().len(), 1);
    for (index, aborted) in [false, true].into_iter().enumerate() {
        let turn = format!("turn-{index}");
        with_commit(&root, &world, |tx| vars::begin_turn(tx, &turn, None)).unwrap();
        let pool = begin(&root, &world, &turn).unwrap().unwrap();
        assert!(set_enabled(&root, &world, false, "en", false).is_err());
        let main = vars::PendingMain {
            text: format!("沃恩 (d20#1=1) {index}"),
            raw: None,
            truncated: aborted,
        };
        let text = main.text.clone();
        with_commit(&root, &world, |tx| {
            if aborted {
                vars::finish_turn(tx, &turn, Some(main))
            } else {
                vars::apply_gm_block(tx, &turn, main, |_, _| ((), None), |_, _| vec![]).map(|_| ())
            }
        })
        .unwrap();
        // 下一筆事件代落，與正常正文追加共用 prepare_turn_append。
        data::append_transcript(&root, &world, 0, &event("雷克等待")).unwrap();
        let key = vars::TurnKey {
            turn_id: turn,
            part: vars::PART_MAIN.into(),
        };
        let (landed, receipt) =
            data::append_event(&root, &world, 0, &event(&text), Some(&key)).unwrap();
        assert_eq!(receipt, None);
        assert_eq!(landed.dice_pool.as_deref(), Some(pool.as_str()));
        let (_, receipt) =
            data::append_event(&root, &world, 0, &event("late"), Some(&key)).unwrap();
        assert_eq!(receipt, None);
    }
    set_enabled(&root, &world, false, "en", false).unwrap();
    with_commit(&root, &world, |tx| vars::begin_turn(tx, "off", None)).unwrap();
    assert!(begin(&root, &world, "off").unwrap().is_none());
    with_commit(&root, &world, |tx| vars::finish_turn(tx, "off", None)).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn gm_lane_tail_retains_one_pool_and_api_history_does_not_add_it() {
    let root = crate::smart_free::test_root("dice-tail");
    let world = data::create_world(&root, "雷克的骰桌").unwrap();
    let m = crate::chat_assembly::gm_materials(&root, &world).unwrap();
    let scan = crate::chat_assembly::test_gm_scan(&root, &world, &m, "en");
    let (scope, _) = crate::chat_assembly::gm_scope(&m);
    let dice = instruction(&measured_pool(), false, "en");
    let (_, turn) = crate::chat_assembly::gm_lane_parts(&m, &scan, &scope, &dice, "en");
    assert_eq!(turn.tail.matches(OPEN).count(), 1);
    assert_eq!(
        turn.tail_worldbook.as_ref().unwrap().matches(OPEN).count(),
        1
    );
    let mut e = event("沃恩 (d20#1=19)");
    e.dice_pool = Some(measured_pool());
    let line = crate::transport::lane_event_line(&e, "en", crate::transport::Side::Gm).unwrap();
    assert!(!line.contains("d4:["));
    assert!(line.contains("(d20#1=19)"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn editing_undo_and_restore_keep_the_saved_pool() {
    let root = crate::smart_free::test_root("dice-restore");
    let world = data::create_world(&root, "沃恩的骰桌").unwrap();
    let mut e = event("沃恩 (d20#1=19)");
    e.dice_pool = Some(measured_pool());
    let (saved, _) = data::append_event(&root, &world, 0, &e, None).unwrap();
    assert!(data::replace_transcript_text(
        &root,
        &world,
        0,
        saved.id.as_deref().unwrap(),
        &e.text,
        "雷克 (d20#1=19)",
        None
    )
    .unwrap());
    let popped = data::pop_transcript_floor(&root, &world, 0).unwrap();
    assert_eq!(popped[0].dice_pool, e.dice_pool);
    data::restore_transcript_events(&root, &world, 0, &popped).unwrap();
    assert_eq!(
        data::read_transcript(&root, &world, 0).unwrap()[0].dice_pool,
        e.dice_pool
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn player_formulas_obey_bounds_and_never_salvage_partial_expressions() {
    assert_eq!(
        suggestions("沃恩請擲d20、2D6 + 3，或1d20-2；再擲2d6+3。"),
        ["1d20", "2d6+3", "1d20-2"]
    );
    for bad in [
        "0d6",
        "101d6",
        "1d1",
        "1d1001",
        "1d6+10000",
        "1d6-10000",
        "2d20kh1",
        "1d6+1d4",
        "1d6 + 1d4",
        "abc1d20",
        "1d6!",
        "999999999999999d6",
    ] {
        assert!(parse_formula(bad).is_none(), "{bad}");
        assert!(suggestions(bad).is_empty(), "{bad}");
    }
    for good in ["100d1000+9999", "1d2-9999", " D6 "] {
        assert!(parse_formula(good).is_some());
    }
    assert_eq!(suggestions("請擲 1d20＋3，或 1d6－1"), ["1d20+3", "1d6-1"]);
    assert!(suggestions("(d4#1=2)（d20 # 2=19）＋2。").is_empty());
    assert_eq!(
        suggestions("**命中骰：1d20+7**，**2d6**"),
        ["1d20+7", "2d6"]
    );
    assert!(suggestions("1d6**2 與 2**1d6").is_empty());
    assert_eq!(suggestions("1d2 1d3 1d4 1d5 1d6 1d7 1d8 1d9 1d10").len(), 8);
    assert!(suggestions(&format!("{}1d20", "沃".repeat(32768))).is_empty());
}

#[test]
fn player_roll_checks_scene_and_turn_then_lands_one_restorable_public_floor() {
    let root = crate::smart_free::test_root("dice-player");
    let world = data::create_world(&root, "沃恩的骰桌").unwrap();
    assert!(roll(&root, &world, 0, "1d6").is_err());
    set_enabled(&root, &world, true, "zh-TW", false).unwrap();
    assert_eq!(
        roll(&root, &world, 1, "1d6").unwrap_err(),
        "dice-scene-changed"
    );
    with_commit(&root, &world, |tx| vars::begin_turn(tx, "busy", None)).unwrap();
    assert!(roll(&root, &world, 0, "1d6").is_err());
    with_commit(&root, &world, |tx| vars::finish_turn(tx, "busy", None)).unwrap();
    assert!(roll(&root, &world, 0, "101d6").is_err());
    data::append_transcript(&root, &world, 0, &event("雷克等待擲果")).unwrap();
    let result = roll(&root, &world, 0, "2d6+3").unwrap();
    assert_eq!(result.marker, Some(data::EventMarker::DiceRoll));
    assert!(!result.gm_only);
    assert!(result.text.starts_with("2d6+3: ["));
    let values = result
        .text
        .split_once('[')
        .unwrap()
        .1
        .split_once(']')
        .unwrap()
        .0;
    let sum: u32 = values
        .split(", ")
        .map(|v| v.parse::<u32>().unwrap())
        .inspect(|v| assert!((1..=6).contains(v)))
        .sum();
    assert!(result.text.ends_with(&format!(" +3 = {}", sum + 3)));
    let line =
        crate::transport::lane_event_line(&result, "en", crate::transport::Side::Gm).unwrap();
    assert!(line.contains(&result.text));
    let materials = crate::chat_assembly::gm_materials(&root, &world).unwrap();
    let scan = crate::chat_assembly::test_gm_scan(&root, &world, &materials, "en");
    let api = crate::transport::assemble_gm_messages_with(
        &[],
        None,
        std::slice::from_ref(&result),
        &scan,
        &Default::default(),
        &Default::default(),
        &Default::default(),
        "en",
        crate::transport::GmVoice::Shared,
        &Default::default(),
        &Default::default(),
        false,
    );
    assert!(api
        .iter()
        .any(|message| message.role == "user" && message.content.contains(&result.text)));

    let popped = data::pop_transcript_floor(&root, &world, 0).unwrap();
    assert_eq!(popped.len(), 1);
    assert_eq!(popped[0].text, result.text);
    data::restore_transcript_events(&root, &world, 0, &popped).unwrap();
    assert_eq!(
        data::read_transcript(&root, &world, 0)
            .unwrap()
            .last()
            .unwrap()
            .text,
        result.text
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn import_detects_only_enabled_new_entries_for_both_book_formats() {
    let root = crate::smart_free::test_root("dice-import");
    let world = data::create_world(&root, "雷克的骰桌").unwrap();
    for (book, detected) in [
        (
            json!({"entries":{"0":{"content":"2d6+3", "disable":true}}}),
            false,
        ),
        (json!({"entries":{"0":{"content":"沃恩擲2d20kh1"}}}), false),
        (json!({"entries":{"0":{"content":"雷克擲1d20+3"}}}), true),
        (
            json!({"entries":[{"content":"沃恩請擲2d6", "enabled":true}]}),
            true,
        ),
    ] {
        let first = data::import_worldbook(&root, &world, &book.to_string()).unwrap();
        assert_eq!(first.dice_detected, detected, "{book}");
        assert!(
            !data::import_worldbook(&root, &world, &book.to_string())
                .unwrap()
                .dice_detected
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn resolve_reads_faces_from_pool() {
    let pool = "d4:[1,3]\nd6:[4,2]\nd20:[19,3]";
    assert_eq!(
        resolve(pool, "d6#1 + d6#2 + 3"),
        Ok((9, vec![(6, 1), (6, 2)]))
    );
    assert_eq!(resolve(pool, "D20#1-2"), Ok((17, vec![(20, 1)])));
    assert_eq!(resolve(pool, "d4#1-3").map(|(total, _)| total), Ok(-2));
    for bad in [
        "d6#3",
        "d8#1",
        "d6#0",
        "d6#1+d6#1",
        "2d6",
        "d6#1+3+4",
        "",
        "d6",
    ] {
        assert!(resolve(pool, bad).is_err(), "{bad}");
    }
    let eleven = ["d6#1"; 11].join("+");
    assert!(resolve(pool, &eleven).is_err(), "最多 10 顆");
}
