//! 續聊中輸出失控（runaway-output-cap）：只派送一次、不走「續聊失敗重開」那條重試，
//! 收尾比照玩家按停止，回錯誤碼而不是半截。

use super::*;

fn call_count(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .count()
}

fn only_state(root: &Path, world_id: &str) -> Option<LaneState> {
    let store = read_store(&data::lanes_path(root, world_id).unwrap());
    assert!(store.len() <= 1, "這張測試桌只該有一條線");
    store.into_values().next()
}

/// Agy 沒有抹寫路徑，輸入不帶名字前綴（照既有 agy 測試）。
fn input(events: &[TranscriptEvent], agy: bool) -> TurnInput<'_> {
    let mut input = turn_input(events, 0);
    if agy {
        input.prefix = None;
    }
    input
}

/// 開一條線、讓第二輪走續聊；回傳第二輪的事件。
async fn open_then_extend(fake: &FakeCli, agy: bool) -> Vec<TranscriptEvent> {
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "老闆晚安")];
    let reply = run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        input(&events, agy),
        None,
        |_| {},
    )
    .await
    .unwrap()
    .text;
    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", &reply));
    events.push(event(TranscriptKind::Player, "", "阿濤", "來一杯麥酒"));
    events
}

#[cfg(unix)]
#[tokio::test]
async fn claude_resume_runaway_is_sent_once_and_next_turn_reopens() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut fake = fake_claude("runaway");
    let calls = fake.session_dir.join("calls.jsonl");
    let events = open_then_extend(&fake, false).await;
    assert_eq!(call_count(&calls), 1);

    fake.call
        .envs
        .push(("FAKE_RUNAWAY_RESUME".to_owned(), "1".to_owned()));
    let mut shown = String::new();
    let error = run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        turn_input(&events, 0),
        None,
        |delta| shown.push_str(delta),
    )
    .await
    .unwrap_err();
    assert!(error.starts_with("AI_OUTPUT_RUNAWAY:"), "{error}");
    assert_eq!(call_count(&calls), 2, "失控不得重開重試");
    let state = only_state(&fake.root, &fake.world_id).expect("claude 抹寫成功就留線");
    assert!(
        state.pending_rewrite.is_some(),
        "比照中止留 pending_rewrite"
    );
    assert!(state.expected_reply.is_none());

    // 下一輪因 pending_rewrite 重開全量
    fake.call
        .envs
        .retain(|(key, _)| key != "FAKE_RUNAWAY_RESUME");
    run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        turn_input(&events, 0),
        None,
        |_| {},
    )
    .await
    .unwrap();
    let last = std::fs::read_to_string(&calls).unwrap();
    let last: serde_json::Value = serde_json::from_str(last.lines().last().unwrap()).unwrap();
    assert!(
        last["args"]
            .as_array()
            .unwrap()
            .iter()
            .any(|arg| arg == "--session-id"),
        "下一輪應重開新線"
    );
    std::fs::remove_dir_all(&fake.dir).unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn claude_runaway_cleanup_failure_still_dispatches_once() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut fake = fake_claude("runaway-block");
    let calls = fake.session_dir.join("calls.jsonl");
    let events = open_then_extend(&fake, false).await;
    fake.call.envs.extend([
        ("FAKE_RUNAWAY_RESUME".to_owned(), "1".to_owned()),
        ("FAKE_RUNAWAY_BLOCK".to_owned(), "1".to_owned()),
    ]);
    let error = run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        turn_input(&events, 0),
        None,
        |_| {},
    )
    .await
    .unwrap_err();
    // 收尾失敗就回收尾的錯，同樣不重派送
    assert!(error.contains("session_abandon_failed"), "{error}");
    assert_eq!(call_count(&calls), 2);
    assert!(only_state(&fake.root, &fake.world_id).is_none());
    std::fs::remove_dir_all(&fake.dir).unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn agy_resume_runaway_keeps_pending_rewrite_like_stop() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut fake = fake_agy("runaway");
    let calls = fake.dir.join("calls.jsonl");
    let events = open_then_extend(&fake, true).await;
    fake.call
        .envs
        .push(("FAKE_RUNAWAY_RESUME".to_owned(), "1".to_owned()));
    let error = run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        input(&events, true),
        None,
        |_| {},
    )
    .await
    .unwrap_err();
    assert!(error.starts_with("AI_OUTPUT_RUNAWAY:"), "{error}");
    assert_eq!(call_count(&calls), 2, "失控不得重開重試");
    let state = only_state(&fake.root, &fake.world_id).expect("agy 不抹不刪");
    assert!(state.pending_rewrite.is_some());
    assert!(state.expected_reply.is_none());
    std::fs::remove_dir_all(&fake.dir).unwrap();
}
