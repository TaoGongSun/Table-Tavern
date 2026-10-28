//! CLI 斷流與「交完收尾不退出」（agy-long-survey-fail）：斷流不重開重試；收尾後卡住的程序
//! 寬限後殺掉、照常交回答案，下一輪照常續聊。

use super::*;

fn calls(fake: &FakeCli) -> Vec<serde_json::Value> {
    std::fs::read_to_string(fake.dir.join("calls.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn resumed(call: &serde_json::Value) -> bool {
    call["args"]
        .as_array()
        .unwrap()
        .iter()
        .any(|arg| arg == "--conversation")
}

fn agy_input(events: &[TranscriptEvent]) -> TurnInput<'_> {
    let mut input = turn_input(events, 0);
    input.prefix = None;
    input
}

#[cfg(unix)]
#[tokio::test]
async fn agy_resume_stall_is_sent_once_and_next_turn_reopens() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut fake = fake_agy("stall");
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "老闆晚安")];
    let reply = run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        agy_input(&events),
        None,
        |_| {},
    )
    .await
    .unwrap()
    .text;
    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", &reply));
    events.push(event(TranscriptKind::Player, "", "阿濤", "來一杯麥酒"));

    fake.call
        .envs
        .push(("FAKE_STALL_RESUME".to_owned(), "1".to_owned()));
    let error = run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        agy_input(&events),
        None,
        |_| {},
    )
    .await
    .unwrap_err();
    assert!(cli::is_stall_error(&error), "{error}");
    assert_eq!(calls(&fake).len(), 2, "斷流不得重開重試");
    let store = read_store(&data::lanes_path(&fake.root, &fake.world_id).unwrap());
    let state = store.values().next().expect("agy 留線");
    assert!(
        state.pending_rewrite.is_some(),
        "比照中止留 pending_rewrite"
    );

    fake.call.envs.retain(|(key, _)| key != "FAKE_STALL_RESUME");
    run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        agy_input(&events),
        None,
        |_| {},
    )
    .await
    .unwrap();
    let all = calls(&fake);
    assert!(!resumed(all.last().unwrap()), "下一輪應重開新線");
    std::fs::remove_dir_all(&fake.dir).unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn agy_hang_after_result_is_killed_and_next_turn_resumes() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut fake = fake_agy("hang");
    fake.call
        .envs
        .push(("FAKE_HANG_AFTER_RESULT".to_owned(), "1".to_owned()));
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "老闆晚安")];
    let started = std::time::Instant::now();
    let reply = run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        agy_input(&events),
        None,
        |_| {},
    )
    .await
    .unwrap()
    .text;
    assert_eq!(reply, "回覆1");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "寬限後就收尾，不等斷流期限：{:?}",
        started.elapsed()
    );
    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", &reply));
    events.push(event(TranscriptKind::Player, "", "阿濤", "來一杯麥酒"));
    let reply = run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        agy_input(&events),
        None,
        |_| {},
    )
    .await
    .unwrap()
    .text;
    assert_eq!(reply, "回覆2");
    assert!(
        resumed(calls(&fake).last().unwrap()),
        "強殺後下一輪照常續聊"
    );
    std::fs::remove_dir_all(&fake.dir).unwrap();
}
