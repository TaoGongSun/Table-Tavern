//! 輸出失控（runaway-output-cap）在 CLI 讀迴圈上的整條路：假 CLI 腳本吐不停，確認會停、程序被殺。
#![cfg(unix)]
#![allow(clippy::await_holding_lock)]

use super::super::stream::{parse_claude_line, parse_grok_line};
use super::*;
use std::path::PathBuf;
use std::time::Duration;

struct Fake {
    dir: PathBuf,
    script: PathBuf,
}

impl Drop for Fake {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// python3 假 CLI：腳本可用 `PID_FILE` 環境變數寫下自己的 pid。
fn fake(tag: &str, body: &str) -> Fake {
    let dir = std::env::temp_dir().join(format!("tt-runaway-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("fake.py");
    std::fs::write(
        &script,
        format!(
            "#!/usr/bin/env python3\nimport os, sys, time\nopen(os.environ['PID_FILE'], 'w').write(str(os.getpid()))\n{body}\n"
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    Fake { dir, script }
}

impl Fake {
    fn envs(&self) -> Vec<(String, String)> {
        vec![(
            "PID_FILE".to_owned(),
            self.dir.join("pid").to_string_lossy().into_owned(),
        )]
    }

    fn alive(&self) -> bool {
        let pid = std::fs::read_to_string(self.dir.join("pid")).unwrap();
        std::process::Command::new("kill")
            .args(["-0", pid.trim()])
            .status()
            .unwrap()
            .success()
    }

    async fn run(
        &self,
        parse: fn(&str) -> CliLine,
        policy: RunawayPolicy,
    ) -> DataResult<CliFinish> {
        let envs = self.envs();
        tokio::time::timeout(
            Duration::from_secs(20),
            run_cli_cancellable(
                &self.script,
                &self.dir,
                &[],
                "",
                &envs,
                parse,
                false,
                policy,
                None,
                |_: &str| {},
                None,
            ),
        )
        .await
        .expect("失控應在時限內被攔下")
    }
}

fn reason_of(finish: DataResult<CliFinish>) -> Option<RunawayReason> {
    match finish.unwrap() {
        CliFinish::Runaway { reason, .. } => Some(reason),
        _ => None,
    }
}

const GROK_BLANK_FOREVER: &str = r#"
while True:
    print('{"type":"text","data":" "}', flush=True)
"#;

#[tokio::test]
async fn endless_blank_text_is_stopped_and_child_killed() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake("blank", GROK_BLANK_FOREVER);
    let finish = fx.run(parse_grok_line, RunawayPolicy::Full).await;
    assert_eq!(reason_of(finish), Some(RunawayReason::WhitespaceRun));
    assert!(!fx.alive(), "失控後子程序要被殺掉");
}

#[tokio::test]
async fn run_cli_surfaces_runaway_as_stable_code_not_ttmsg() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake("code", GROK_BLANK_FOREVER);
    let envs = fx.envs();
    let error = run_cli(
        &fx.script,
        &fx.dir,
        &[],
        "",
        &envs,
        parse_grok_line,
        false,
        RunawayPolicy::DegenerateOnly,
        None,
        |_: &str| {},
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(
        error.starts_with("AI_OUTPUT_RUNAWAY: reason=whitespace_run"),
        "{error}"
    );
    // 原碼要穿過 ai_call_failure 白名單，不被包成 AI_CALL_FAILED
    assert_eq!(
        crate::transport::dispatch::ai_call_failure(error.clone()),
        error
    );
}

#[tokio::test]
async fn length_cap_applies_only_to_full_policy() {
    let _serial = crate::inflight::lock_real_process_tests();
    let body = r#"
for _ in range(400):
    print('{"type":"text","data":"' + '正常的長篇敘述。' * 12 + '"}', flush=True)
print('{"type":"end"}', flush=True)
"#;
    let fx = fake("length", body);
    assert_eq!(
        reason_of(fx.run(parse_grok_line, RunawayPolicy::Full).await),
        Some(RunawayReason::Length)
    );
    let fx = fake("length-oneshot", body);
    match fx
        .run(parse_grok_line, RunawayPolicy::DegenerateOnly)
        .await
        .unwrap()
    {
        CliFinish::Completed(text) => assert_eq!(text.chars().count(), 400 * 8 * 12),
        _ => panic!("單發不設字數上限"),
    }
}

#[tokio::test]
async fn runaway_done_fallback_is_caught_even_if_cli_never_exits() {
    let _serial = crate::inflight::lock_real_process_tests();
    // 零增量、收尾全文失控，之後持續吐 stderr 不退出：收到 Done 當下就要攔，不等程序結束
    let fx = fake(
        "done-hang",
        r#"
import json
print(json.dumps({'type': 'result', 'is_error': False, 'result': '開頭' + ' ' * 2500}), flush=True)
while True:
    print('still alive', file=sys.stderr, flush=True)
    time.sleep(0.05)
"#,
    );
    let started = std::time::Instant::now();
    let finish = fx.run(parse_claude_line, RunawayPolicy::Full).await;
    assert_eq!(reason_of(finish), Some(RunawayReason::WhitespaceRun));
    assert!(started.elapsed() < Duration::from_secs(10));
    assert!(!fx.alive());
}

#[tokio::test]
async fn normal_done_fallback_is_counted_once() {
    let _serial = crate::inflight::lock_real_process_tests();
    // 20,000 字只計一次就在 30,000 之內；重複計數會誤判成 Length
    let fx = fake(
        "done-once",
        r#"
import json
print(json.dumps({'type': 'result', 'is_error': False, 'result': '字' * 20000}), flush=True)
"#,
    );
    match fx
        .run(parse_claude_line, RunawayPolicy::Full)
        .await
        .unwrap()
    {
        CliFinish::Completed(text) => assert_eq!(text.chars().count(), 20_000),
        _ => panic!("正常的收尾全文要照常採用"),
    }
}

const GROK_BLANK_THOUGHT: &str = r#"
for _ in range(3000):
    print('{"type":"thought","data":" "}', flush=True)
print('{"type":"text","data":"好"}', flush=True)
print('{"type":"end"}', flush=True)
"#;

#[tokio::test]
async fn blank_thinking_trips_unless_policy_is_off() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake("thought", GROK_BLANK_THOUGHT);
    match fx
        .run(parse_grok_line, RunawayPolicy::DegenerateOnly)
        .await
        .unwrap()
    {
        // chars 是思考那支的計數（正文還是 0）
        CliFinish::Runaway { reason, chars } => {
            assert_eq!(reason, RunawayReason::WhitespaceRun);
            assert_eq!(chars, 2000);
        }
        _ => panic!("空白思考應觸發"),
    }
    let fx = fake("thought-off", GROK_BLANK_THOUGHT);
    match fx.run(parse_grok_line, RunawayPolicy::Off).await.unwrap() {
        CliFinish::Completed(text) => assert_eq!(text, "好"),
        _ => panic!("Off 不查思考"),
    }
}

#[tokio::test]
async fn overlong_stdout_line_is_runaway_even_with_policy_off() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake(
        "long-stdout",
        r#"
sys.stdout.write('x' * (1024 * 1024 + 10))
sys.stdout.flush()
time.sleep(30)
"#,
    );
    assert_eq!(
        reason_of(fx.run(parse_grok_line, RunawayPolicy::Off).await),
        Some(RunawayReason::LineTooLong)
    );
    assert!(!fx.alive());
}

#[tokio::test]
async fn overlong_stderr_line_is_runaway_too() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake(
        "long-stderr",
        r#"
sys.stderr.write('e' * (1024 * 1024 + 10))
sys.stderr.flush()
time.sleep(30)
"#,
    );
    assert_eq!(
        reason_of(fx.run(parse_grok_line, RunawayPolicy::Off).await),
        Some(RunawayReason::LineTooLong)
    );
}

#[tokio::test]
async fn stderr_between_halves_of_a_stdout_line_loses_nothing() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake(
        "interleave",
        r#"
sys.stdout.write('{"type":"text","da')
sys.stdout.flush()
time.sleep(0.3)
for i in range(20):
    print('noise', i, file=sys.stderr, flush=True)
time.sleep(0.3)
sys.stdout.write('ta":"完整"}\r\n{"type":"end"}')
sys.stdout.flush()
"#,
    );
    match fx.run(parse_grok_line, RunawayPolicy::Full).await.unwrap() {
        CliFinish::Completed(text) => assert_eq!(text, "完整"),
        _ => panic!("半條 stdout 要完整拼回"),
    }
}
