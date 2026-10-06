//! claude 續聊線的快取壽命估計與超額觀測（claude-1h-cache）。

use super::*;
use std::sync::{Arc, Mutex};

fn usage(cached: u64, created: u64, one_hour: Option<u64>) -> transport::PromptCacheUsage {
    transport::PromptCacheUsage {
        prompt_tokens: cached + created + 3,
        cached_tokens: Some(cached),
        created_tokens: Some(created),
        created_1h_tokens: one_hour,
        output_tokens: 10,
        cost_usd: Some(0.01),
    }
}

/// 估計表：只有整段寫成 1h 才給一小時；混合、時效不明保守取 5 分鐘；
/// 純命中只在同 session 續聊時沿用；讀寫皆 0 或沒用量＝視為過期。
#[test]
fn next_cache_ttl_follows_observed_writes() {
    assert_eq!(
        next_cache_ttl(Some(&usage(0, 1000, Some(1000))), None, false),
        3600
    );
    assert_eq!(
        next_cache_ttl(Some(&usage(0, 1000, Some(0))), Some(3600), true),
        300
    );
    assert_eq!(
        next_cache_ttl(Some(&usage(0, 1000, Some(400))), Some(3600), true),
        300
    );
    assert_eq!(
        next_cache_ttl(Some(&usage(0, 1000, None)), Some(3600), true),
        300
    );
    assert_eq!(
        next_cache_ttl(Some(&usage(900, 0, Some(0))), Some(3600), true),
        3600
    );
    assert_eq!(
        next_cache_ttl(Some(&usage(900, 0, None)), Some(300), true),
        300
    );
    assert_eq!(
        next_cache_ttl(Some(&usage(900, 0, Some(0))), Some(3600), false),
        300
    );
    assert_eq!(
        next_cache_ttl(Some(&usage(0, 0, Some(0))), Some(3600), true),
        0
    );
    assert_eq!(next_cache_ttl(None, Some(3600), true), 0);
}

/// 過期判斷看這條線自己的壽命：未過期走補丁續聊，過期整份追平（同 session，不重開）。
#[test]
fn plan_expiry_uses_the_lane_ttl() {
    let events = [event(TranscriptKind::Player, "", "阿濤", "你好")];
    let mut input = turn_input(&events, 0);
    input.frozen_system = "凍結B".to_owned(); // 素材漂移，才看得出補丁還是追平
    for (ttl, age, rebased_expected) in [
        (300, 300, false),
        (300, 301, true),
        (3600, 301, false),
        (3600, 3600, false),
        (3600, 3601, true),
    ] {
        let mut state = lane_state(&events, 0);
        state.cache_ttl_secs = ttl;
        match plan_turn(
            Some(&state),
            &input,
            state.last_call_epoch + age,
            LaneProvider::Claude,
        ) {
            TurnPlan::Resume {
                session_id,
                patch,
                rebased,
                ..
            } => {
                assert_eq!(session_id, "sid-1", "ttl {ttl} age {age}");
                assert_eq!(rebased, rebased_expected, "ttl {ttl} age {age}");
                assert_eq!(patch.is_some(), !rebased_expected, "ttl {ttl} age {age}");
            }
            TurnPlan::Reopen { .. } => panic!("ttl {ttl} age {age} 不該重開"),
        }
    }
}

/// 舊 lanes.json 沒有壽命欄＝5 分鐘；寫回再讀保得住新值。
#[test]
fn legacy_lane_file_reads_five_minutes_and_ttl_survives_rewrite() {
    let events = [event(TranscriptKind::Player, "", "阿濤", "你好")];
    let mut value = serde_json::to_value(lane_state(&events, 0)).unwrap();
    value.as_object_mut().unwrap().remove("cache_ttl_secs");
    let legacy: LaneState = serde_json::from_value(value).unwrap();
    assert_eq!(legacy.cache_ttl_secs, LEGACY_CACHE_TTL_SECS);

    let dir = std::env::temp_dir().join(format!("tt-lanes-ttl-file-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let world_id = ulid::Ulid::generate().to_string();
    mark_playable(&dir.join("worlds").join(&world_id));
    let path = data::lanes_path(&dir, &world_id).unwrap();
    let mut state = lane_state(&events, 0);
    state.cache_ttl_secs = 3600;
    let store: LaneStore = [("chars:sonnet".to_owned(), state)].into_iter().collect();
    write_store(&path, &store).unwrap();
    assert_eq!(read_store(&path)["chars:sonnet"].cache_ttl_secs, 3600);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// 只有 claude 續聊線釘 1h；別家不動。
#[test]
fn pin_lane_cache_ttl_only_touches_claude() {
    let mut call = LaneCall {
        provider: LaneProvider::Claude,
        program: PathBuf::new(),
        working_dir: PathBuf::new(),
        envs: Vec::new(),
        model: None,
        usage_log: None,
        claude_home: PathBuf::new(),
        prompt_dir: PathBuf::new(),
        on_overage: None,
    };
    pin_lane_cache_ttl(&mut call);
    assert!(call
        .envs
        .contains(&("CLAUDE_CODE_PROMPT_CACHE_TTL".to_owned(), "1h".to_owned())));
    assert!(call
        .envs
        .contains(&("FORCE_PROMPT_CACHING_5M".to_owned(), "0".to_owned())));
    call.provider = LaneProvider::Grok;
    call.envs.clear();
    pin_lane_cache_ttl(&mut call);
    assert!(call.envs.is_empty());
}

fn set_env(call: &mut LaneCall, key: &str, value: Option<&str>) {
    call.envs.retain(|(name, _)| name != key);
    if let Some(value) = value {
        call.envs.push((key.to_owned(), value.to_owned()));
    }
}

fn clear_fake_env(call: &mut LaneCall) {
    call.envs
        .retain(|(name, _)| !name.starts_with("FAKE_") || name == "FAKE_SESSION_DIR");
}

fn usage_json(cached: u64, created: u64, split: &str) -> String {
    format!(
        r#"{{"input_tokens":3,"cache_read_input_tokens":{cached},"cache_creation_input_tokens":{created},"output_tokens":10{split}}}"#
    )
}

fn split(one_hour: u64, five: u64) -> String {
    format!(
        r#","cache_creation":{{"ephemeral_1h_input_tokens":{one_hour},"ephemeral_5m_input_tokens":{five}}}"#
    )
}

fn last_ledger(call: &LaneCall) -> serde_json::Value {
    let text = std::fs::read_to_string(call.usage_log.as_ref().unwrap()).unwrap();
    serde_json::from_str(text.lines().last().unwrap()).unwrap()
}

fn sink(call: &mut LaneCall) -> Arc<Mutex<Vec<CacheWriteObserved>>> {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let held = seen.clone();
    call.on_overage = Some(Arc::new(move |observed| {
        held.lock().unwrap().push(observed)
    }));
    seen
}

const OVERAGE: &str = r#"{"status":"allowed","isUsingOverage":true,"overageStatus":"allowed"}"#;
const NOT_OVERAGE: &str =
    r#"{"status":"allowed","isUsingOverage":false,"overageStatus":"allowed"}"#;
const REJECTED: &str = r#"{"status":"rejected","isUsingOverage":true,"overageStatus":"rejected"}"#;

/// 端到端（假 CLI）：每輪依實際寫入更新壽命，帳本記呼叫前採用的壽命與 1h 拆分。
#[cfg(unix)]
#[tokio::test]
async fn lane_ttl_tracks_each_successful_turn() {
    let _serial = crate::inflight::lock_real_process_tests();
    let FakeCli {
        dir,
        mut call,
        root,
        world_id,
        ..
    } = fake_claude("ttl-e2e");
    let store_path = data::lanes_path(&root, &world_id).unwrap();
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第0句")];
    let ttl = || read_store(&store_path)["chars:sonnet"].cache_ttl_secs;

    // 開線、純 1h（假 CLI 預設）
    run_turn(
        &call,
        &root,
        &world_id,
        turn_input(&events, 0),
        None,
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(ttl(), 3600);
    let opened = last_ledger(&call);
    assert_eq!(opened["created_1h_tokens"], 1000);
    assert!(opened.get("ttl_secs").is_none(), "開線沒有呼叫前的壽命");

    // （收尾用量, 之後的壽命, 帳本該不該有 1h 拆分）
    let cases: Vec<(String, u64, bool)> = vec![
        (usage_json(900, 0, &split(0, 0)), 3600, true), // 純命中：沿用
        (usage_json(0, 1000, &split(0, 1000)), 300, true), // 純 5m
        (usage_json(0, 1000, &split(1000, 0)), 3600, true), // 純 1h
        (usage_json(0, 1000, &split(400, 600)), 300, true), // 混合
        (usage_json(0, 1000, ""), 300, false),          // 缺 cache_creation
        (usage_json(0, 1000, r#","cache_creation":{}"#), 300, false),
        (
            usage_json(
                0,
                1000,
                r#","cache_creation":{"ephemeral_1h_input_tokens":null}"#,
            ),
            300,
            false,
        ),
        (
            usage_json(
                0,
                1000,
                r#","cache_creation":{"ephemeral_1h_input_tokens":"x"}"#,
            ),
            300,
            false,
        ),
        (usage_json(0, 0, &split(0, 0)), 0, true), // 讀寫皆 0：視為過期
    ];
    let mut previous = 3600;
    for (index, (usage, expected, has_split)) in cases.iter().enumerate() {
        events.push(event(
            TranscriptKind::Dialogue,
            "fox-id",
            "狐狸",
            &format!("回覆{}", index + 1),
        ));
        events.push(event(
            TranscriptKind::Player,
            "",
            "阿濤",
            &format!("第{}句", index + 1),
        ));
        set_env(&mut call, "FAKE_USAGE", Some(usage));
        run_turn(
            &call,
            &root,
            &world_id,
            turn_input(&events, 0),
            None,
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(ttl(), *expected, "case {index}: {usage}");
        let line = last_ledger(&call);
        assert_eq!(
            line["ttl_secs"], previous,
            "case {index} 帳本記呼叫前的壽命"
        );
        assert!(line.get("reason").is_none(), "case {index} 應續聊");
        assert_eq!(
            line.get("created_1h_tokens").is_some(),
            *has_split,
            "case {index}"
        );
        previous = *expected;
    }

    // 重開時的純命中：新 session 撞到舊前綴，時效未知 → 5 分鐘
    events[0].text = "改過的第0句".to_owned();
    set_env(
        &mut call,
        "FAKE_USAGE",
        Some(&usage_json(900, 0, &split(0, 0))),
    );
    run_turn(
        &call,
        &root,
        &world_id,
        turn_input(&events, 0),
        None,
        |_| {},
    )
    .await
    .unwrap();
    assert!(last_ledger(&call).get("reason").is_some(), "改字要重開");
    assert_eq!(ttl(), 300);

    std::fs::remove_dir_all(&dir).unwrap();
}

/// 降級重開不沿用失敗那次的觀測：續聊那次超額且純 1h 但失敗，重開那次非超額、純 5m 成功
/// → 超額提示講的是第一次（1h），線的壽命看第二次（5 分鐘）。
#[cfg(unix)]
#[tokio::test]
async fn retry_keeps_each_attempt_observation_apart() {
    let _serial = crate::inflight::lock_real_process_tests();
    let FakeCli {
        dir,
        mut call,
        root,
        world_id,
        ..
    } = fake_claude("ttl-retry");
    let store_path = data::lanes_path(&root, &world_id).unwrap();
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第0句")];
    run_turn(
        &call,
        &root,
        &world_id,
        turn_input(&events, 0),
        None,
        |_| {},
    )
    .await
    .unwrap();

    let seen = sink(&mut call);
    set_env(&mut call, "FAKE_FAIL_RESUME", Some("1"));
    set_env(&mut call, "FAKE_RATE_BEFORE_RESUME", Some(OVERAGE));
    set_env(
        &mut call,
        "FAKE_USAGE_RESUME",
        Some(&usage_json(0, 1000, &split(1000, 0))),
    );
    set_env(
        &mut call,
        "FAKE_USAGE_OPEN",
        Some(&usage_json(0, 1000, &split(0, 1000))),
    );
    set_env(&mut call, "FAKE_RATE_BEFORE_OPEN", Some(NOT_OVERAGE));
    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", "回覆1"));
    events.push(event(TranscriptKind::Player, "", "阿濤", "第1句"));
    run_turn(
        &call,
        &root,
        &world_id,
        turn_input(&events, 0),
        None,
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(*seen.lock().unwrap(), vec![CacheWriteObserved::OneHour]);
    assert_eq!(read_store(&store_path)["chars:sonnet"].cache_ttl_secs, 300);

    // 兩次都失敗、整輪回錯：超額那次的錢已花，照樣報
    seen.lock().unwrap().clear();
    set_env(&mut call, "FAKE_FAIL_OPEN", Some("1"));
    set_env(&mut call, "FAKE_RATE_BEFORE_OPEN", None);
    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", "回覆1"));
    events.push(event(TranscriptKind::Player, "", "阿濤", "第2句"));
    assert!(run_turn(
        &call,
        &root,
        &world_id,
        turn_input(&events, 0),
        None,
        |_| {}
    )
    .await
    .is_err());
    assert_eq!(*seen.lock().unwrap(), vec![CacheWriteObserved::OneHour]);

    std::fs::remove_dir_all(&dir).unwrap();
}

/// 超額事件的各種位置與狀態，對到每次嘗試自己的寫入觀測。
#[cfg(unix)]
#[tokio::test]
async fn overage_is_reported_once_with_the_attempt_observation() {
    let _serial = crate::inflight::lock_real_process_tests();
    let FakeCli {
        dir,
        mut call,
        root,
        world_id,
        ..
    } = fake_claude("ttl-overage");
    let seen = sink(&mut call);
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第0句")];
    let cases: Vec<(
        &str,
        Option<&str>,
        Option<&str>,
        String,
        Option<CacheWriteObserved>,
    )> = vec![
        (
            "result 前",
            Some(OVERAGE),
            None,
            usage_json(0, 1000, &split(1000, 0)),
            Some(CacheWriteObserved::OneHour),
        ),
        (
            "result 後",
            None,
            Some(OVERAGE),
            usage_json(0, 1000, &split(1000, 0)),
            Some(CacheWriteObserved::OneHour),
        ),
        (
            "true→false",
            Some(OVERAGE),
            Some(NOT_OVERAGE),
            usage_json(0, 1000, &split(1000, 0)),
            Some(CacheWriteObserved::OneHour),
        ),
        (
            "被拒",
            Some(REJECTED),
            None,
            usage_json(0, 1000, &split(1000, 0)),
            None,
        ),
        (
            "沒在超額",
            Some(NOT_OVERAGE),
            None,
            usage_json(0, 1000, &split(1000, 0)),
            None,
        ),
        (
            "純 5m",
            Some(OVERAGE),
            None,
            usage_json(0, 1000, &split(0, 1000)),
            Some(CacheWriteObserved::Other),
        ),
        (
            "混合",
            Some(OVERAGE),
            None,
            usage_json(0, 1000, &split(400, 600)),
            Some(CacheWriteObserved::Other),
        ),
        (
            "缺拆分",
            Some(OVERAGE),
            None,
            usage_json(0, 1000, ""),
            Some(CacheWriteObserved::Other),
        ),
        (
            "純命中",
            Some(OVERAGE),
            None,
            usage_json(900, 0, &split(0, 0)),
            Some(CacheWriteObserved::Other),
        ),
    ];
    for (index, (name, before, after, usage, expected)) in cases.into_iter().enumerate() {
        clear_fake_env(&mut call);
        set_env(&mut call, "FAKE_RATE_BEFORE", before);
        set_env(&mut call, "FAKE_RATE_AFTER", after);
        set_env(&mut call, "FAKE_USAGE", Some(&usage));
        seen.lock().unwrap().clear();
        if index > 0 {
            events.push(event(
                TranscriptKind::Dialogue,
                "fox-id",
                "狐狸",
                &format!("回覆{index}"),
            ));
            events.push(event(
                TranscriptKind::Player,
                "",
                "阿濤",
                &format!("第{index}句"),
            ));
        }
        run_turn(
            &call,
            &root,
            &world_id,
            turn_input(&events, 0),
            None,
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(
            *seen.lock().unwrap(),
            expected.into_iter().collect::<Vec<_>>(),
            "{name}"
        );
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

/// 超額事件已到、result 還沒到就被中止：觀測是「不知道」，照樣報。
#[cfg(unix)]
#[tokio::test]
async fn overage_before_abort_is_reported_as_unknown() {
    let _serial = crate::inflight::lock_real_process_tests();
    let FakeCli {
        dir,
        mut call,
        root,
        world_id,
        session_dir,
        ..
    } = fake_claude("ttl-abort");
    let seen = sink(&mut call);
    set_env(&mut call, "FAKE_RATE_BEFORE", Some(OVERAGE));
    set_env(&mut call, "FAKE_HANG", Some("1"));
    let events = vec![event(TranscriptKind::Player, "", "阿濤", "第0句")];
    let (guard, mut cancel) = crate::inflight::register_turn(&world_id, "ttl-abort");
    let turn = async {
        let _guard = guard;
        run_turn(
            &call,
            &root,
            &world_id,
            turn_input(&events, 0),
            Some(&mut cancel),
            |_| {},
        )
        .await
    };
    let abort = async {
        let marker = session_dir.join("hanging");
        let step = std::time::Duration::from_millis(10);
        let mut waited = std::time::Duration::ZERO;
        while !marker.exists() {
            assert!(waited < std::time::Duration::from_secs(20));
            tokio::time::sleep(step).await;
            waited += step;
        }
        // 超額那行在卡住前已吐出；給 runner 讀進來的機會再停
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        crate::inflight::abort_turn(&world_id, "ttl-abort");
    };
    let (outcome, ()) = tokio::join!(turn, abort);
    assert!(outcome.unwrap().aborted);
    assert_eq!(*seen.lock().unwrap(), vec![CacheWriteObserved::Unknown]);
    std::fs::remove_dir_all(&dir).unwrap();
}
