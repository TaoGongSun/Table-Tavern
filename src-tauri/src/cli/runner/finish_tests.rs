//! 收尾與斷流（agy-long-survey-fail）：成功收尾後程序不退要寬限後殺掉、照常交回全文；
//! 第一個正文／思考前有保底等待；錯誤收尾、取消競態、晚到的計費事件。
#![allow(clippy::await_holding_lock)]

use super::super::stream::AGY_PROTOCOL;
#[cfg(unix)]
use super::super::stream::{CLAUDE_PROTOCOL, CODEX_PROTOCOL};
use super::*;
use std::path::PathBuf;
use std::time::{Duration, Instant};

struct Fake {
    dir: PathBuf,
    script: PathBuf,
    extra_envs: Vec<(String, String)>,
}

impl Drop for Fake {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// python3 假 CLI（僅 unix）；`emit(obj)` 印一行 JSON 並 flush，`hang()` 忽略 SIGTERM 後睡到天荒地老。
#[cfg(unix)]
fn fake(tag: &str, body: &str) -> Fake {
    let dir = fake_dir(tag);
    let script = dir.join("fake.py");
    std::fs::write(
        &script,
        format!(
            "#!/usr/bin/env python3\nimport json, os, signal, sys, time\nopen(os.environ['PID_FILE'], 'w').write(str(os.getpid()))\nsys.stdin.read()\ndef emit(obj):\n    print(json.dumps(obj, ensure_ascii=False), flush=True)\ndef hang():\n    signal.signal(signal.SIGTERM, signal.SIG_IGN)\n    time.sleep(120)\n{body}\n"
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    Fake {
        dir,
        script,
        extra_envs: Vec::new(),
    }
}

fn fake_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tt-finish-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 跨平台假 CLI（`examples/fake_cli.rs` 的 script 角色），開頭同 python 版：記 pid、讀完 stdin。
fn fake_cli(tag: &str, lines: &[&str]) -> Fake {
    let lines: Vec<&str> = ["pid", "stdin"].iter().chain(lines).copied().collect();
    Fake {
        dir: fake_dir(tag),
        script: crate::test_fake_cli::program(),
        extra_envs: crate::test_fake_cli::script(&lines),
    }
}

impl Fake {
    fn envs(&self) -> Vec<(String, String)> {
        let pid_file = self.dir.join("pid").to_string_lossy().into_owned();
        let mut envs = vec![("PID_FILE".to_owned(), pid_file)];
        envs.extend(self.extra_envs.iter().cloned());
        envs
    }

    fn alive(&self) -> bool {
        let pid = std::fs::read_to_string(self.dir.join("pid")).unwrap();
        crate::cli::update::gate::pid_alive(pid.trim().parse().unwrap())
    }

    async fn run(
        &self,
        protocol: &CliProtocol,
        usage_log: Option<UsageLog<'_>>,
        cancel: Option<watch::Receiver<bool>>,
    ) -> DataResult<CliFinish> {
        let envs = self.envs();
        tokio::time::timeout(
            Duration::from_secs(30),
            run_cli_cancellable(
                &self.script,
                &self.dir,
                &[],
                "提示詞",
                &envs,
                protocol,
                false,
                RunawayPolicy::Off,
                usage_log,
                |_: &str| {},
                cancel,
                SessionWatch::default(),
            ),
        )
        .await
        .expect("要在時限內收場")
    }
}

#[cfg(unix)]
const AGY_RESULT: &str = r#"emit({'event': 'step_update', 'step_update': {'step_type': 'agent_response', 'text_delta': '答案'}})
emit({'event': 'result', 'result': {'status': 'SUCCESS', 'response': '答案'}})"#;

fn completed(result: DataResult<CliFinish>) -> String {
    match result {
        Ok(CliFinish::Completed(text)) => text,
        Ok(_) => panic!("不是完成"),
        Err(error) => panic!("不是完成：{error}"),
    }
}

#[tokio::test]
async fn success_then_hang_is_killed_after_grace_and_completes() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake_cli(
        "hang",
        &[
            r#"out {"event":"step_update","step_update":{"step_type":"agent_response","text_delta":"答案"}}"#,
            r#"out {"event":"result","result":{"status":"SUCCESS","response":"答案"}}"#,
            "hang",
        ],
    );
    let started = Instant::now();
    assert_eq!(completed(fx.run(&AGY_PROTOCOL, None, None).await), "答案");
    assert!(started.elapsed() < STALL_WINDOW, "{:?}", started.elapsed());
    assert!(!fx.alive(), "程序要被殺掉收屍");
}

#[cfg(unix)]
#[tokio::test]
async fn stderr_spam_after_success_does_not_starve_the_grace_deadline() {
    let _serial = crate::inflight::lock_real_process_tests();
    let body = format!(
        "{AGY_RESULT}\nsignal.signal(signal.SIGTERM, signal.SIG_IGN)\nwhile True:\n    print('noise', file=sys.stderr, flush=True)\n    time.sleep(0.01)"
    );
    let fx = fake("spam", &body);
    let started = Instant::now();
    assert_eq!(completed(fx.run(&AGY_PROTOCOL, None, None).await), "答案");
    assert!(started.elapsed() < STALL_WINDOW, "{:?}", started.elapsed());
    assert!(!fx.alive());
}

/// claude 的 rate_limit_event 可能在 result 之後才到：寬限期間照常解析，超額旗標照記。
#[cfg(unix)]
#[tokio::test]
async fn billing_event_after_result_is_still_parsed_during_grace() {
    let _serial = crate::inflight::lock_real_process_tests();
    let body = r#"emit({'type': 'result', 'is_error': False, 'result': '好'})
time.sleep(0.3)
emit({'type': 'rate_limit_event', 'rate_limit_info': {'isUsingOverage': True, 'overageStatus': 'allowed'}})
hang()"#;
    let fx = fake("late-billing", body);
    let overage = std::sync::atomic::AtomicBool::new(false);
    let log_path = fx.dir.join("usage.jsonl");
    let log = UsageLog {
        usage_out: None,
        overage_out: Some(&overage),
        path: &log_path,
        world: None,
        transport: "claude",
        model: "sonnet",
        parse: super::super::stream::parse_claude_usage,
        lane: None,
        shape: crate::usage::log::PromptShape::Oneshot,
        prompt_tokens_out: None,
        conversation_id_out: None,
        expected_conversation_id: None,
        agy_usage_base: None,
        agy_usage_out: None,
        identity_out: None,
    };
    assert_eq!(
        completed(fx.run(&CLAUDE_PROTOCOL, Some(log), None).await),
        "好"
    );
    assert!(overage.load(std::sync::atomic::Ordering::Relaxed));
}

/// 錯誤收尾不起算寬限、也不當完成；codex 的頂層 error 之後還有 turn.completed 時，寬限照樣起算。
#[cfg(unix)]
#[tokio::test]
async fn error_finish_is_not_completion_and_codex_notice_does_not_block_grace() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake(
        "error-finish",
        "emit({'type': 'result', 'is_error': True, 'result': '額度用完'})",
    );
    let error = fx
        .run(&CLAUDE_PROTOCOL, None, None)
        .await
        .err()
        .expect("要回錯");
    assert!(error.to_string().contains("額度用完"), "{error}");

    let body = r#"emit({'type': 'error', 'message': 'Reconnecting... 1/5'})
emit({'type': 'item.completed', 'item': {'type': 'agent_message', 'text': '接上了'}})
emit({'type': 'turn.completed'})
hang()"#;
    let fx = fake("codex-notice", body);
    let started = Instant::now();
    assert_eq!(
        completed(fx.run(&CODEX_PROTOCOL, None, None).await),
        "接上了\n"
    );
    assert!(started.elapsed() < STALL_WINDOW, "{:?}", started.elapsed());
}

/// 收尾後才按停止：完成贏，交回全文。
#[cfg(unix)]
#[tokio::test]
async fn cancel_after_success_still_completes() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake("cancel-race", &format!("{AGY_RESULT}\nhang()"));
    let (tx, rx) = watch::channel(false);
    let pid_file = fx.dir.join("pid");
    let trigger = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        while !pid_file.exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let _ = tx.send(true);
        tx
    });
    assert_eq!(
        completed(fx.run(&AGY_PROTOCOL, None, Some(rx)).await),
        "答案"
    );
    drop(trigger.await);
}

/// 第一個進展前：只有 init 行時，超過斷流窗但沒超過保底照樣等到答案。
#[cfg(unix)]
#[tokio::test]
async fn silent_thinking_before_first_progress_gets_the_floor_window() {
    let _serial = crate::inflight::lock_real_process_tests();
    let wait = (STALL_WINDOW + FIRST_PROGRESS_WINDOW).as_secs_f64() / 2.0;
    let body = format!("emit({{'event': 'init'}})\ntime.sleep({wait})\n{AGY_RESULT}");
    let fx = fake("floor", &body);
    assert_eq!(completed(fx.run(&AGY_PROTOCOL, None, None).await), "答案");
}

/// 超過保底仍沒有進展＝斷流；有過進展之後回到一般斷流窗。
#[tokio::test]
async fn no_progress_past_the_floor_or_silence_after_progress_stalls() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake_cli("floor-exceeded", &[r#"out {"event":"init"}"#, "hang"]);
    let started = Instant::now();
    let error = fx
        .run(&AGY_PROTOCOL, None, None)
        .await
        .err()
        .expect("要斷流");
    assert!(is_stall_error(&error.to_string()), "{error}");
    assert!(
        !error.to_string().contains("cli_stalled_after_output"),
        "{error}"
    );
    assert!(
        started.elapsed() >= FIRST_PROGRESS_WINDOW,
        "{:?}",
        started.elapsed()
    );
    assert!(!fx.alive());

    let fx = fake_cli(
        "after-progress",
        &[
            r#"out {"event":"step_update","step_update":{"step_type":"agent_response","text_delta":"半"}}"#,
            "hang",
        ],
    );
    let started = Instant::now();
    let error = fx
        .run(&AGY_PROTOCOL, None, None)
        .await
        .err()
        .expect("要斷流");
    assert!(is_stall_error(&error.to_string()), "{error}");
    // 吐過正文就不說「可能仍在思考」
    assert!(
        error.to_string().contains("cli_stalled_after_output"),
        "{error}"
    );
    assert!(
        started.elapsed() < FIRST_PROGRESS_WINDOW,
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn stall_errors_are_recognised_raw_and_nested() {
    let raw = UiMsg::CliStalled.to_string();
    assert!(is_stall_error(&raw));
    let nested = UiMsg::CliReplyError { error: raw }.to_string();
    assert!(is_stall_error(&nested));
    assert!(is_stall_error(&UiMsg::CliStalledAfterOutput.to_string()));
    assert!(!is_stall_error(&UiMsg::CliUnexpectedAbort.to_string()));
}
