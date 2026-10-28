//! cli-version-gate 包 1 的收尾行為，四家各跑一遍（plans/cli-version-gate.md 包 1 第 6 點）：
//! 有正文缺收尾、未知事件正常完成、未知事件加缺收尾、成功收尾但正文空加未知事件、明確失敗、取消、非 0 退出。
#![cfg(unix)]
#![allow(clippy::await_holding_lock)]

use super::super::stream::{AGY_PROTOCOL, CLAUDE_PROTOCOL, CODEX_PROTOCOL, GROK_PROTOCOL};
use super::*;
use std::path::PathBuf;

struct Fake {
    dir: PathBuf,
    script: PathBuf,
}

impl Drop for Fake {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fake(lines: &[&str], tail: &str) -> Fake {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!(
        "tt-fake-gate-{}-{}",
        std::process::id(),
        ulid::Ulid::generate()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let mut body = String::from("#!/bin/sh\ncat > /dev/null\n");
    for line in lines {
        body.push_str(&format!("printf '%s\\n' '{line}'\n"));
    }
    body.push_str(tail);
    let script = dir.join("fake.sh");
    std::fs::write(&script, body).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    Fake { dir, script }
}

/// 一家 CLI 的最小事件組。
struct Shapes {
    protocol: &'static CliProtocol,
    text: &'static str,
    done: &'static str,
    /// 成功收尾、收尾文字也空
    empty_done: &'static str,
    failed: &'static str,
    failed_says: &'static str,
    unknown: &'static str,
}

const SHAPES: [Shapes; 4] = [
    Shapes {
        protocol: &CLAUDE_PROTOCOL,
        text: r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"hi"}}}"#,
        done: r#"{"type":"result","subtype":"success","is_error":false,"result":"hi"}"#,
        empty_done: r#"{"type":"result","subtype":"success","is_error":false,"result":""}"#,
        failed: r#"{"type":"result","subtype":"success","is_error":true,"result":"boom"}"#,
        failed_says: "boom",
        unknown: r#"{"type":"assistant_v2","text":"hi"}"#,
    },
    Shapes {
        protocol: &CODEX_PROTOCOL,
        text: r#"{"type":"item.completed","item":{"id":"i","type":"agent_message","text":"hi"}}"#,
        done: r#"{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":1}}"#,
        empty_done: r#"{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":1}}"#,
        failed: r#"{"type":"turn.failed","error":{"message":"boom"}}"#,
        failed_says: "boom",
        unknown: r#"{"type":"item.completed","item":{"id":"i","type":"assistant_message","text":"hi"}}"#,
    },
    Shapes {
        protocol: &AGY_PROTOCOL,
        text: r#"{"event":"step_update","step_update":{"step_type":"agent_response","text_delta":"hi"}}"#,
        done: r#"{"event":"result","result":{"status":"SUCCESS","response":"hi"}}"#,
        empty_done: r#"{"event":"result","result":{"status":"SUCCESS","response":""}}"#,
        failed: r#"{"event":"result","result":{"status":"ERROR","response":""}}"#,
        failed_says: "ERROR",
        unknown: r#"{"event":"step_update","step_update":{"step_type":"agent_reply","text_delta":"hi"}}"#,
    },
    Shapes {
        protocol: &GROK_PROTOCOL,
        text: r#"{"type":"text","data":"hi"}"#,
        done: r#"{"type":"end","stopReason":"end_turn"}"#,
        empty_done: r#"{"type":"end","stopReason":"end_turn"}"#,
        failed: r#"{"type":"error","message":"boom"}"#,
        failed_says: "boom",
        unknown: r#"{"type":"text_chunk","data":"hi"}"#,
    },
];

async fn run(fx: &Fake, protocol: &CliProtocol) -> DataResult<CliFinish> {
    run_cli_cancellable(
        &fx.script,
        &fx.dir,
        &[],
        "x",
        &[],
        protocol,
        false,
        RunawayPolicy::Off,
        None,
        |_: &str| {},
        None,
        SessionWatch::default(),
    )
    .await
}

fn error_of(result: DataResult<CliFinish>, label: &str) -> String {
    match result {
        Err(error) => error.to_string(),
        Ok(_) => panic!("{label}：應該失敗"),
    }
}

#[tokio::test]
async fn text_without_finish_event_fails_for_every_cli() {
    let _serial = crate::inflight::lock_real_process_tests();
    for shape in &SHAPES {
        let fx = fake(&[shape.text], "exit 0\n");
        let error = error_of(run(&fx, shape.protocol).await, shape.protocol.id);
        assert!(
            error.contains("cli_turn_failed_status"),
            "{}: {error}",
            shape.protocol.id
        );
        assert!(
            error.contains(shape.protocol.name),
            "{}: {error}",
            shape.protocol.id
        );
    }
}

#[tokio::test]
async fn unknown_event_does_not_break_a_finished_turn() {
    let _serial = crate::inflight::lock_real_process_tests();
    for shape in &SHAPES {
        let fx = fake(&[shape.unknown, shape.text, shape.done], "exit 0\n");
        match run(&fx, shape.protocol).await {
            Ok(CliFinish::Completed(text)) => {
                assert!(text.contains("hi"), "{}: {text}", shape.protocol.id)
            }
            other => panic!("{}: 應該完成：{:?}", shape.protocol.id, other.err()),
        }
    }
}

#[tokio::test]
async fn unknown_event_without_finish_is_reported_as_unrecognized() {
    let _serial = crate::inflight::lock_real_process_tests();
    for shape in &SHAPES {
        let fx = fake(&[shape.unknown], "echo 'stderr hint' >&2\nexit 0\n");
        let error = error_of(run(&fx, shape.protocol).await, shape.protocol.id);
        // 測試裡不查版本：落在「查不到版本」那一則，附 stderr 末行
        assert!(
            error.contains("cli_output_unrecognized_unknown_version"),
            "{}: {error}",
            shape.protocol.id
        );
        assert!(
            error.contains("stderr hint"),
            "{}: {error}",
            shape.protocol.id
        );
    }
}

#[tokio::test]
async fn empty_finish_with_unknown_event_is_reported_as_unrecognized() {
    let _serial = crate::inflight::lock_real_process_tests();
    for shape in &SHAPES {
        let fx = fake(&[shape.unknown, shape.empty_done], "exit 0\n");
        let error = error_of(run(&fx, shape.protocol).await, shape.protocol.id);
        assert!(
            error.contains("cli_output_unrecognized_unknown_version"),
            "{}: {error}",
            shape.protocol.id
        );
        // 沒有認不得的行就維持「沒有產出回覆」
        let fx = fake(&[shape.empty_done], "exit 0\n");
        let error = error_of(run(&fx, shape.protocol).await, shape.protocol.id);
        assert!(
            error.contains("cli_no_reply"),
            "{}: {error}",
            shape.protocol.id
        );
    }
}

#[tokio::test]
async fn explicit_failure_keeps_the_cli_reason_even_with_unknown_lines() {
    let _serial = crate::inflight::lock_real_process_tests();
    for shape in &SHAPES {
        let fx = fake(&[shape.unknown, shape.text, shape.failed], "exit 0\n");
        let error = error_of(run(&fx, shape.protocol).await, shape.protocol.id);
        assert!(
            error.contains(shape.failed_says),
            "{}: {error}",
            shape.protocol.id
        );
        assert!(
            !error.contains("unrecognized"),
            "{}: {error}",
            shape.protocol.id
        );
    }
}

#[tokio::test]
async fn nonzero_exit_without_finish_is_a_crash_not_unrecognized() {
    let _serial = crate::inflight::lock_real_process_tests();
    for shape in &SHAPES {
        let fx = fake(
            &[shape.unknown, shape.text],
            "echo 'flag rejected' >&2\nexit 3\n",
        );
        let error = error_of(run(&fx, shape.protocol).await, shape.protocol.id);
        assert!(
            error.contains("cli_crashed"),
            "{}: {error}",
            shape.protocol.id
        );
        assert!(
            error.contains("flag rejected"),
            "{}: {error}",
            shape.protocol.id
        );
    }
}

#[tokio::test]
async fn cancel_still_returns_the_partial_text() {
    let _serial = crate::inflight::lock_real_process_tests();
    for shape in &SHAPES {
        let fx = fake(&[shape.text], "exec sleep 30\n");
        // 收到正文就按停：確定取消發生在有半截文字之後
        let (sender, receiver) = tokio::sync::watch::channel(false);
        let finish = run_cli_cancellable(
            &fx.script,
            &fx.dir,
            &[],
            "x",
            &[],
            shape.protocol,
            false,
            RunawayPolicy::Off,
            None,
            |_: &str| {
                let _ = sender.send(true);
            },
            Some(receiver),
            SessionWatch::default(),
        )
        .await;
        match finish {
            Ok(CliFinish::Aborted(text)) => assert!(text.contains("hi"), "{}", shape.protocol.id),
            other => panic!("{}: 應該中止：{:?}", shape.protocol.id, other.err()),
        }
    }
}

#[tokio::test]
async fn empty_finish_with_unknown_event_and_nonzero_exit_is_not_unrecognized() {
    let _serial = crate::inflight::lock_real_process_tests();
    for shape in &SHAPES {
        let fx = fake(&[shape.unknown, shape.empty_done], "exit 3\n");
        let error = error_of(run(&fx, shape.protocol).await, shape.protocol.id);
        assert!(
            error.contains("cli_no_reply"),
            "{}: {error}",
            shape.protocol.id
        );
        assert!(
            !error.contains("unrecognized"),
            "{}: {error}",
            shape.protocol.id
        );
    }
}

/// codex：沒收尾、頂層 error、非 0 退出——先報 CLI 自己的原因（SoftError），不是 crash
#[tokio::test]
async fn soft_error_without_finish_and_exit_one_reports_the_cli_reason() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake(
        &[
            r#"{"type":"thread.started","thread_id":"t"}"#,
            r#"{"type":"error","message":"stream disconnected before completion"}"#,
        ],
        "echo 'tail line' >&2\nexit 1\n",
    );
    let error = error_of(run(&fx, &CODEX_PROTOCOL).await, "codex");
    assert!(error.contains("stream disconnected"), "{error}");
    assert!(!error.contains("cli_crashed"), "{error}");
}
