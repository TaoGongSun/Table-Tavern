//! char-line-status-strip：角色回覆帶控制區塊時，共用 session 換寫成收尾台詞、續聊照常對得上；
//! 只有控制區塊的完成回合比照失控收尾（不重試、留 pending_rewrite、下一輪重開）。

use super::*;

const TAG: &str = "<UpdateVariable>_.set('錢包', 3);</UpdateVariable>";

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

fn resumed(calls: &Path) -> bool {
    let text = std::fs::read_to_string(calls).unwrap();
    let last: serde_json::Value = serde_json::from_str(text.lines().last().unwrap()).unwrap();
    last["args"]
        .as_array()
        .unwrap()
        .iter()
        .any(|arg| arg == "--resume")
}

#[cfg(unix)]
#[tokio::test]
async fn claude_tagged_reply_is_rewritten_and_next_turn_resumes() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut fake = fake_claude("status-strip");
    fake.call
        .envs
        .push(("FAKE_REPLY_SUFFIX".to_owned(), TAG.to_owned()));
    let calls = fake.session_dir.join("calls.jsonl");
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "老闆晚安")];
    let reply = run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        turn_input(&events, 0),
        None,
        |_| {},
    )
    .await
    .unwrap()
    .text;
    let landed = transport::finish_character_reply(&reply, "狐狸：", false);
    assert_eq!(landed.text, "回覆1");
    let state = only_state(&fake.root, &fake.world_id).unwrap();
    let session =
        std::fs::read_to_string(fake.session_dir.join(format!("{}.jsonl", state.session_id)))
            .unwrap();
    assert!(
        !session.contains("UpdateVariable"),
        "共用 session 不留控制區塊"
    );
    assert!(session.contains("狐狸：回覆1"));

    // 前端落的是收尾台詞：下一輪續聊對得上，不因 ReplyDiverged 重開
    events.push(event(
        TranscriptKind::Dialogue,
        "fox-id",
        "狐狸",
        &landed.text,
    ));
    events.push(event(TranscriptKind::Player, "", "阿濤", "來一杯麥酒"));
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
    assert_eq!(call_count(&calls), 2);
    assert!(resumed(&calls), "收尾後的台詞應讓下一輪續聊");
    std::fs::remove_dir_all(&fake.dir).unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn claude_tags_only_reply_errors_once_erases_secret_and_reopens() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut fake = fake_claude("status-strip-empty");
    let calls = fake.session_dir.join("calls.jsonl");
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "老闆晚安")];
    let reply = run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        turn_input(&events, 0),
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
        .push(("FAKE_REPLY_RESUME".to_owned(), format!("狐狸：{TAG}")));
    let mut input = turn_input(&events, 0);
    input.confidential = Some("## 狐狸的私有設定\n其實是通緝犯\n".to_owned());
    input.tail = format!("{}現在你是「狐狸」。", input.confidential.clone().unwrap());
    let error = run_turn(&fake.call, &fake.root, &fake.world_id, input, None, |_| {})
        .await
        .unwrap_err();
    assert!(error.starts_with("AI_EMPTY_RESPONSE"), "{error}");
    assert_eq!(call_count(&calls), 2, "沒有台詞不得重開重試");
    let state = only_state(&fake.root, &fake.world_id).expect("claude 抹寫成功就留線");
    assert!(
        state.pending_rewrite.is_some(),
        "留 pending_rewrite 讓下一輪重開"
    );
    assert!(state.expected_reply.is_none());
    let session =
        std::fs::read_to_string(fake.session_dir.join(format!("{}.jsonl", state.session_id)))
            .unwrap();
    assert!(!session.contains("通緝犯"), "機密段照樣抹掉");

    fake.call.envs.retain(|(key, _)| key != "FAKE_REPLY_RESUME");
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
    assert!(!resumed(&calls), "下一輪應重開新線");
    std::fs::remove_dir_all(&fake.dir).unwrap();
}

/// 已送段指紋只看 text：raw 不同不算改動
#[test]
fn raw_does_not_change_the_sent_fingerprint() {
    let plain = event(TranscriptKind::Dialogue, "fox-id", "狐狸", "回覆1");
    let mut with_raw = plain.clone();
    with_raw.raw = Some(format!("回覆1{TAG}"));
    assert_eq!(
        events_fingerprint(&[plain]),
        events_fingerprint(&[with_raw])
    );
}

/// 帶控制區塊的回覆：預期回聲＝前端落檔的收尾台詞，plan_turn 接受；換別的角色也續用同一條線
#[test]
fn tagged_echo_resumes_for_the_next_speaker() {
    let echo = ReplyEcho::Dialogue {
        speaker_id: "fox-id".to_owned(),
        prefix: "狐狸：".to_owned(),
    };
    let before = [event(TranscriptKind::Player, "", "阿濤", "你好")];
    let mut state = lane_state(&before, 0);
    state.expected_reply = Some(expected_reply_for(&echo, &format!("{TAG}\n狐狸：晚安")));
    let stored = [
        before[0].clone(),
        event(TranscriptKind::Dialogue, "fox-id", "狐狸", "晚安"),
        event(TranscriptKind::Player, "", "阿濤", "兔子你呢？"),
    ];
    let mut input = turn_input(&stored, 0);
    input.prefix = Some("兔子：".to_owned());
    input.echo = ReplyEcho::Dialogue {
        speaker_id: "rabbit-id".to_owned(),
        prefix: "兔子：".to_owned(),
    };
    assert!(matches!(
        plan_turn(Some(&state), &input, 1_010, LaneProvider::Claude),
        TurnPlan::Resume { base: 2, .. }
    ));
}
