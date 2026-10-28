//! codex 的收尾規則（CODEX_PROTOCOL.require_done）與 SessionWatch：假 CLI 腳本吐指定的 JSON 行與退出碼。
#![cfg(unix)]
#![allow(clippy::await_holding_lock)]

use super::super::stream::{parse_codex_usage, CLAUDE_PROTOCOL, CODEX_PROTOCOL, GROK_PROTOCOL};
use super::super::types::CodexUsageCounters;
use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

struct Fake {
    dir: PathBuf,
    script: PathBuf,
}

impl Drop for Fake {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fake(lines: &[&str], exit: i32) -> Fake {
    fake_with_tail(lines, &format!("exit {exit}\n"))
}

/// tail：印完各行之後的腳本尾段（例如 `exec sleep 60` 模擬印完收尾卻不退出）。
fn fake_with_tail(lines: &[&str], tail: &str) -> Fake {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!(
        "tt-fake-codex-{}-{}",
        std::process::id(),
        ulid::Ulid::generate()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let mut body = String::from("#!/bin/sh\ncat > /dev/null\n");
    for line in lines {
        body.push_str(&format!("printf '%s\\n' '{line}'\n"));
    }
    body.push_str(tail);
    let script = dir.join("fake-codex.sh");
    std::fs::write(&script, body).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    Fake { dir, script }
}

const STARTED: &str = r#"{"type":"thread.started","thread_id":"t-1"}"#;
const MESSAGE: &str =
    r#"{"type":"item.completed","item":{"id":"i","type":"agent_message","text":"台詞"}}"#;
const RETRY: &str = r#"{"type":"error","message":"Reconnecting... 2/5"}"#;
const COMPLETED: &str = r#"{"type":"turn.completed","usage":{"input_tokens":42034,"cached_input_tokens":32000,"cache_write_input_tokens":0,"output_tokens":171}}"#;

async fn run(
    fx: &Fake,
    protocol: &CliProtocol,
    usage_log: Option<UsageLog<'_>>,
    session: SessionWatch<'_>,
) -> DataResult<CliFinish> {
    run_cli_cancellable(
        &fx.script,
        &fx.dir,
        &[],
        "x",
        &[],
        protocol,
        false,
        RunawayPolicy::Off,
        usage_log,
        |_: &str| {},
        None,
        session,
    )
    .await
}

fn text(finish: DataResult<CliFinish>) -> String {
    match finish.unwrap() {
        CliFinish::Completed(text) => text,
        _ => panic!("應該完成"),
    }
}

#[tokio::test]
async fn retry_error_then_completed_is_success() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake(&[STARTED, RETRY, MESSAGE, COMPLETED], 0);
    assert_eq!(
        text(run(&fx, &CODEX_PROTOCOL, None, SessionWatch::default()).await),
        "台詞\n"
    );
}

#[tokio::test]
async fn only_soft_error_with_exit_zero_fails_with_that_message() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake(
        &[
            STARTED,
            r#"{"type":"error","message":"stream closed early"}"#,
        ],
        0,
    );
    let error = run(&fx, &CODEX_PROTOCOL, None, SessionWatch::default())
        .await
        .err()
        .expect("沒收到完成事件不算成功")
        .to_string();
    assert!(error.contains("stream closed early"), "{error}");
}

#[tokio::test]
async fn text_without_completed_and_exit_zero_fails() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake(&[STARTED, MESSAGE], 0);
    let error = run(&fx, &CODEX_PROTOCOL, None, SessionWatch::default())
        .await
        .err()
        .expect("半截正文不能當完成")
        .to_string();
    assert!(error.contains("cli_turn_failed_status"), "{error}");
    // 太長類錯誤照樣掛穩定碼
    let fx = fake(
        &[
            STARTED,
            r#"{"type":"error","message":"prompt is too long: 300000 tokens > 200000 maximum"}"#,
        ],
        0,
    );
    let error = run(&fx, &CODEX_PROTOCOL, None, SessionWatch::default())
        .await
        .err()
        .unwrap()
        .to_string();
    assert!(error.starts_with("AI_CONTEXT_TOO_LONG"), "{error}");
}

#[tokio::test]
async fn turn_failed_fails_with_its_message() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake(
        &[
            STARTED,
            RETRY,
            r#"{"type":"turn.failed","error":{"message":"quota exceeded"}}"#,
        ],
        1,
    );
    let error = run(&fx, &CODEX_PROTOCOL, None, SessionWatch::default())
        .await
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("quota exceeded"), "{error}");
}

#[tokio::test]
async fn completed_with_nonzero_exit_delivers_once_and_flags_unclean() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake(&[STARTED, MESSAGE, COMPLETED], 3);
    let unclean = AtomicBool::new(false);
    let session = SessionWatch {
        unclean_exit_out: Some(&unclean),
        ..SessionWatch::default()
    };
    assert_eq!(
        text(run(&fx, &CODEX_PROTOCOL, None, session).await),
        "台詞\n"
    );
    assert!(unclean.load(Ordering::Relaxed));
    // 正常結束不標
    let fx = fake(&[STARTED, MESSAGE, COMPLETED], 0);
    let clean = AtomicBool::new(false);
    let session = SessionWatch {
        unclean_exit_out: Some(&clean),
        ..SessionWatch::default()
    };
    text(run(&fx, &CODEX_PROTOCOL, None, session).await);
    assert!(!clean.load(Ordering::Relaxed));
}

/// 收到 turn.completed 後不退出、被寬限期殺掉：照常完成，而且不標非正常結束（lane 不丟線）。
#[tokio::test]
async fn completed_then_killed_by_grace_is_not_unclean() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake_with_tail(&[STARTED, MESSAGE, COMPLETED], "exec sleep 60\n");
    let unclean = AtomicBool::new(false);
    let session = SessionWatch {
        unclean_exit_out: Some(&unclean),
        ..SessionWatch::default()
    };
    let started = std::time::Instant::now();
    assert_eq!(
        text(run(&fx, &CODEX_PROTOCOL, None, session).await),
        "台詞\n"
    );
    assert!(started.elapsed() < STALL_WINDOW, "{:?}", started.elapsed());
    assert!(!unclean.load(Ordering::Relaxed));
}

/// 寬限殺掉前 codex 報過 rollout 存檔失敗：不能假設已落檔，照舊標非正常結束（lane 丟線）。
#[tokio::test]
async fn grace_kill_after_rollout_save_failure_is_unclean() {
    let _serial = crate::inflight::lock_real_process_tests();
    let save_failed = r#"{"type":"item.completed","item":{"id":"w","type":"error","message":"Failed to save the conversation transcript; Codex will continue retrying. Error: disk full"}}"#;
    let fx = fake_with_tail(
        &[STARTED, MESSAGE, save_failed, COMPLETED],
        "exec sleep 60\n",
    );
    let unclean = AtomicBool::new(false);
    let session = SessionWatch {
        unclean_exit_out: Some(&unclean),
        ..SessionWatch::default()
    };
    assert_eq!(
        text(run(&fx, &CODEX_PROTOCOL, None, session).await),
        "台詞\n"
    );
    assert!(unclean.load(Ordering::Relaxed));
}

/// grok：頂層 error 之後 end_turn＝成功、交回正文（後到的收尾為準，只有 codex 保留先到的失敗）。
#[tokio::test]
async fn grok_error_then_end_turn_succeeds() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake(
        &[
            r#"{"type":"error","data":"transient upstream hiccup"}"#,
            r#"{"type":"text","data":"接著說"}"#,
            r#"{"type":"end","stopReason":"end_turn"}"#,
        ],
        0,
    );
    assert_eq!(
        text(run(&fx, &GROK_PROTOCOL, None, SessionWatch::default()).await),
        "接著說"
    );
}

/// 其他 CLI 的判定不變：沒有收尾事件、程序以 0 結束、有正文＝照舊當完成。
#[tokio::test]
async fn other_protocols_keep_text_on_clean_exit() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake(
        &[
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"你好"}}}"#,
        ],
        0,
    );
    assert_eq!(
        text(run(&fx, &CLAUDE_PROTOCOL, None, SessionWatch::default()).await),
        "你好"
    );
}

#[tokio::test]
async fn thread_id_is_captured_without_usage_log_and_mismatch_aborts() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake(&[STARTED, MESSAGE, COMPLETED], 0);
    let id = Mutex::new(None);
    let counters = Mutex::new(None);
    let session = SessionWatch {
        id_out: Some(&id),
        codex_usage_out: Some(&counters),
        ..SessionWatch::default()
    };
    text(run(&fx, &CODEX_PROTOCOL, None, session).await);
    assert_eq!(id.lock().unwrap().as_deref(), Some("t-1"));
    assert_eq!(
        counters.lock().unwrap().map(|c| c.input_tokens),
        Some(42034)
    );
    let session = SessionWatch {
        expected_id: Some("t-other"),
        ..SessionWatch::default()
    };
    let error = run(&fx, &CODEX_PROTOCOL, None, session)
        .await
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("cli_session_mismatch"), "{error}");
}

fn usage_log<'a>(
    path: &'a std::path::Path,
    seen: &'a std::sync::atomic::AtomicU64,
) -> UsageLog<'a> {
    UsageLog {
        usage_out: None,
        overage_out: None,
        path,
        world: Some("w1"),
        transport: "codex",
        model: "(CLI 預設)",
        parse: parse_codex_usage,
        lane: None,
        shape: crate::usage::log::PromptShape::Oneshot,
        prompt_tokens_out: Some(seen),
        conversation_id_out: None,
        expected_conversation_id: None,
        agy_usage_base: None,
        agy_usage_out: None,
        identity_out: None,
    }
}

/// 續聊：累計值減上一輪才入帳；基準缺就記未知（0、附診斷），不拿累計值冒充單輪。
#[tokio::test]
async fn resumed_usage_is_delta_or_unknown() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake(&[STARTED, MESSAGE, COMPLETED], 0);
    let path = fx.dir.join("prompt-cache.jsonl");
    let seen = std::sync::atomic::AtomicU64::new(99);
    let base = CodexUsageCounters {
        input_tokens: 20948,
        cached_input_tokens: 11136,
        cache_write_input_tokens: 0,
        output_tokens: 93,
    };
    let session = SessionWatch {
        expected_id: Some("t-1"),
        codex_usage_base: Some(base),
        resumed: true,
        ..SessionWatch::default()
    };
    text(run(&fx, &CODEX_PROTOCOL, Some(usage_log(&path, &seen)), session).await);
    assert_eq!(seen.load(Ordering::Relaxed), 21086);

    let session = SessionWatch {
        expected_id: Some("t-1"),
        codex_usage_base: None,
        resumed: true,
        ..SessionWatch::default()
    };
    text(run(&fx, &CODEX_PROTOCOL, Some(usage_log(&path, &seen)), session).await);
    assert_eq!(seen.load(Ordering::Relaxed), 0);
    let logged = std::fs::read_to_string(&path).unwrap();
    let rows: Vec<serde_json::Value> = logged
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows.len(), 2);
    assert!(rows[0].get("codex_usage_unknown").is_none());
    assert_eq!(
        rows[1]["codex_usage_unknown"]["current"]["input_tokens"],
        42034
    );
}
