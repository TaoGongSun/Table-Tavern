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
        assert_eq!(begin(&root, &world, &turn).unwrap(), Some(pool.clone()));
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
