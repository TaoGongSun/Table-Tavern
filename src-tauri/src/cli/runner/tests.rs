#![allow(clippy::await_holding_lock)]

use super::super::stream::{parse_claude_line, parse_claude_usage};
use super::*;

/// 以假 CLI 腳本走完 spawn→stdin→逐行解析→增量→收尾整條路（sh 腳本，僅 unix）
#[cfg(unix)]
#[tokio::test]
async fn run_cli_streams_deltas_from_fake_cli_and_reads_stdin() {
    // run_cli 現在會把子程序 pid 登記進 inflight 的全域 children 表；kill_all_children
    // 的測試（inflight.rs）會不分青紅皂白殺表上全部 pid，故用同一把鎖互斥執行。
    let _serial = crate::inflight::lock_real_process_tests();
    let dir = std::env::temp_dir().join(format!("tt-fake-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let working_dir = dir.join("workspace");
    std::fs::create_dir_all(&working_dir).unwrap();
    std::fs::write(working_dir.join("cwd-marker"), "").unwrap();
    let script = dir.join("fake-claude.sh");
    std::fs::write(
        &script,
        concat!(
            "#!/bin/sh\n",
            "input=$(cat)\n", // 必須把 stdin 讀完，證明 prompt 有送達
            "test -f ./cwd-marker || exit 8\n",
            "echo '{\"type\":\"system\",\"subtype\":\"init\"}'\n",
            "echo '{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"想\"}}}'\n",
            "echo '{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"你\"}}}'\n",
            "echo '{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"好\"}}}'\n",
            "echo \"{\\\"type\\\":\\\"result\\\",\\\"is_error\\\":false,\\\"result\\\":\\\"你好\\\",\\\"total_cost_usd\\\":0.0015,\\\"usage\\\":{\\\"input_tokens\\\":1,\\\"cache_creation_input_tokens\\\":0,\\\"cache_read_input_tokens\\\":99,\\\"output_tokens\\\":2}}\"\n",
            "test \"$input\" = \"提示詞\" || exit 9\n",
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let log_path = dir.join("prompt-cache.jsonl");
    let seen = std::sync::atomic::AtomicU64::new(0);
    let mut deltas = Vec::new();
    let full = run_cli(
        &script,
        &working_dir,
        &[],
        "提示詞",
        &[],
        parse_claude_line,
        true,
        Some(UsageLog {
            path: &log_path,
            world: Some("w1"),
            transport: "claude",
            model: "sonnet",
            parse: parse_claude_usage,
            lane: None,
            shape: crate::usage_log::PromptShape::Oneshot,
            prompt_tokens_out: Some(&seen),
            conversation_id_out: None,
            expected_conversation_id: None,
            agy_usage_base: None,
            agy_usage_out: None,
        }),
        |delta: &str| {
            deltas.push(delta.to_owned());
        },
    )
    .await
    .unwrap();
    // 同一份輸出、關掉思考轉發：聊天正文串流不得混進思考
    let mut quiet_deltas = Vec::new();
    let quiet = run_cli(
        &script,
        &working_dir,
        &[],
        "提示詞",
        &[],
        parse_claude_line,
        false,
        None,
        |delta: &str| {
            quiet_deltas.push(delta.to_owned());
        },
    )
    .await
    .unwrap();
    let logged = std::fs::read_to_string(&log_path).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(quiet, "你好");
    assert_eq!(quiet_deltas, ["你", "好"]);
    // 思考增量進顯示流、不進正文
    assert_eq!(full, "你好");
    assert_eq!(deltas, ["想", "你", "好"]);
    // 收尾事件落一行 JSONL：總輸入 100（1＋0＋99）、讀快取 99 → 99%
    assert_eq!(logged.lines().count(), 1);
    let record: serde_json::Value = serde_json::from_str(logged.trim()).unwrap();
    assert_eq!(record["transport"], "claude");
    assert_eq!(record["world"], "w1");
    assert_eq!(record["model"], "sonnet");
    assert_eq!(record["prompt_tokens"], 100);
    assert_eq!(record["cached_tokens"], 99);
    assert_eq!(record["created_tokens"], 0);
    assert_eq!(record["output_tokens"], 2);
    assert_eq!(record["hit_rate"], 99.0);
    assert_eq!(record["cost_usd"], 0.0015);
    // 無狀態路徑照樣判快取結果（本案修的就是這裡以前短路成「單發」）；
    // 時間戳到秒（分鐘精度分不出是否踩到 5 分鐘過期線）
    assert_eq!(record["mode"], "oneshot");
    assert_eq!(record["cache"], "hit");
    assert_eq!(record["ts"].as_str().unwrap().len(), 19);
    // 總輸入回填給呼叫端，續聊線用它當下輪的理論可中量
    assert_eq!(seen.load(std::sync::atomic::Ordering::Relaxed), 100);
}

#[cfg(unix)]
#[tokio::test]
async fn run_cli_aborts_instantly_on_fatal_stderr_api_error_and_shows_it_in_tail() {
    let _serial = crate::inflight::lock_real_process_tests();
    let dir = std::env::temp_dir().join(format!("tt-fake-cli-fatal-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("fake-claude-fatal.sh");
    // stderr 吐設定類 API 錯誤後長睡（模擬 CLI 自己退避重試）；沒有立即中止就會撞測試逾時
    std::fs::write(
        &script,
        concat!(
            "#!/bin/sh\n",
            "cat > /dev/null\n",
            "echo 'API Error: 502 unknown provider for model claude-opus-4-7' >&2\n",
            "sleep 30\n",
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut deltas = Vec::new();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        run_cli(
            &script,
            &dir,
            &[],
            "提示詞",
            &[],
            parse_claude_line,
            true,
            None,
            |delta: &str| deltas.push(delta.to_owned()),
        ),
    )
    .await
    .expect("設定類錯誤必須立即中止，不得等 CLI 睡完");
    let error = result.unwrap_err().to_string();
    assert!(error.contains("unknown provider"), "錯誤要帶原文：{error}");
    // 進度字尾也要同步看到錯誤行
    assert!(deltas.iter().any(|d| d.contains("API Error")));
}

#[cfg(unix)]
#[tokio::test]
async fn run_cli_reports_crash_without_result_event_instead_of_returning_partial_text() {
    let _serial = crate::inflight::lock_real_process_tests();
    let dir = std::env::temp_dir().join(format!("tt-fake-cli-crash-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("fake-claude-crash.sh");
    // 吐一筆正文增量後 crash（無 result 收尾事件）：殘缺正文不得當成功返回
    std::fs::write(
        &script,
        concat!(
            "#!/bin/sh\n",
            "cat > /dev/null\n",
            "echo '{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"殘\"}}}'\n",
            "echo 'proxy connection reset' >&2\n",
            "exit 3\n",
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut deltas = Vec::new();
    let error = run_cli(
        &script,
        &dir,
        &[],
        "提示詞",
        &[],
        parse_claude_line,
        true,
        None,
        |delta: &str| deltas.push(delta.to_owned()),
    )
    .await
    .unwrap_err()
    .to_string();
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(
        error.contains("CLI 異常結束"),
        "要報 crash 而非靜默：{error}"
    );
    assert!(
        error.contains("proxy connection reset"),
        "要帶 stderr 尾巴：{error}"
    );
    // 進度字尾同步看到 ⚠，玩家不用等到收尾才知道
    assert!(deltas.iter().any(|d| d.contains('⚠')));
}

#[cfg(unix)]
#[tokio::test]
async fn run_cli_strips_inherited_anthropic_env_but_keeps_explicit_envs() {
    let _serial = crate::inflight::lock_real_process_tests();
    let dir = std::env::temp_dir().join(format!("tt-fake-cli-env-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("fake-claude-env.sh");
    // 繼承的 ANTHROPIC_BASE_URL 必須被拔掉；顯式 envs 傳入的 MARKER 必須到位
    std::fs::write(
        &script,
        concat!(
            "#!/bin/sh\n",
            "cat > /dev/null\n",
            "test -z \"$ANTHROPIC_BASE_URL\" || exit 7\n",
            "test \"$ANTHROPIC_MARKER\" = \"explicit\" || exit 9\n",
            "echo '{\"type\":\"result\",\"is_error\":false,\"result\":\"ok\"}'\n",
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    std::env::set_var("ANTHROPIC_BASE_URL", "http://127.0.0.1:9999");
    let result = run_cli(
        &script,
        &dir,
        &[],
        "提示詞",
        &[("ANTHROPIC_MARKER".to_owned(), "explicit".to_owned())],
        parse_claude_line,
        false,
        None,
        |_: &str| {},
    )
    .await;
    std::env::remove_var("ANTHROPIC_BASE_URL");
    assert_eq!(result.unwrap(), "ok");
}

#[test]
fn api_error_kind_classifies_fatal_vs_transient() {
    // 暫時性：讓 CLI 重試，但 Some(false) 表示要餵進度
    assert_eq!(
        api_error_kind("API Error: 529 overloaded, retrying"),
        Some(false)
    );
    // 設定類：模型不存在／認證，立即中止
    assert_eq!(
        api_error_kind("API Error: 502 unknown provider for model x"),
        Some(true)
    );
    assert_eq!(
        api_error_kind("API Error: 401 authentication_error"),
        Some(true)
    );
    // 非錯誤行不動作
    assert_eq!(api_error_kind("thinking hard..."), None);
}
