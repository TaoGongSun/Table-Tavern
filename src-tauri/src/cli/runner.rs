use super::types::{CliLine, UsageLog};
use crate::data::DataResult;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

/// `run_cli` spawn 後掛的反登記保險：不論提早 return、`?` 冒出的錯誤，還是外層 future
/// 被 select 取消（中止在途呼叫）整個被 drop，這個 guard 的 Drop 都會觸發，
/// 確保 inflight 的子程序 PID 表不留殘影。
struct ChildPidGuard(Option<u32>);

impl Drop for ChildPidGuard {
    fn drop(&mut self) {
        if let Some(pid) = self.0 {
            crate::inflight::unregister_child(pid);
        }
    }
}

/// headless 單發：prompt 走 stdin，逐行讀 stdout 解析、增量回呼，回傳完整文字。
/// stderr 行的 API 錯誤偵測：None＝不是錯誤行；Some(false)＝暫時性（讓 CLI 自己重試，
/// 但進度要立刻看得到）；Some(true)＝設定類（模型不存在／認證／權限），重試不會好，立即中止。
fn api_error_kind(line: &str) -> Option<bool> {
    let lower = line.to_lowercase();
    if !lower.contains("api error") {
        return None;
    }
    const FATAL: [&str; 8] = [
        "unknown provider",
        "not_found",
        "not found",
        "invalid",
        "authentication",
        "unauthorized",
        "permission",
        "billing",
    ];
    Some(
        FATAL.iter().any(|kw| lower.contains(kw))
            || ["401", "403", "404"].iter().any(|code| lower.contains(code)),
    )
}

pub async fn run_cli(
    program: &Path,
    working_dir: &Path,
    args: &[String],
    stdin_data: &str,
    envs: &[(String, String)],
    parse: fn(&str) -> CliLine,
    // 思考增量要不要餵給 on_delta：只有「進度字尾」型顯示（卡重構）開 true；
    // 聊天／旁白的 on_delta 是劇情正文串流，思考混進去會出戲。
    thinking_to_delta: bool,
    usage_log: Option<UsageLog<'_>>,
    mut on_delta: impl FnMut(&str),
) -> DataResult<String> {
    let mut command = Command::new(program);
    // 先掛系統代理再掛使用者 envs，同名時使用者設定蓋過代理
    crate::proxy::apply_system_proxy(&mut command);
    // CLI 子程序一律不繼承 ANTHROPIC_*：啟動 app 的 shell 若殘留閘道變數（例如指向
    // 本機代理的 ANTHROPIC_BASE_URL＋AUTH_TOKEN），整批呼叫會被劫走。要接第三方閘道
    // 一律走 app 設定欄，claude_cli_envs 會在下面的 envs 顯式補回。
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("ANTHROPIC_") {
            command.env_remove(&key);
        }
    }
    command
        .current_dir(working_dir)
        .args(args)
        .envs(
            envs.iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    command.creation_flags(0x08000000);
    // 呼叫端 future 被 drop（中止在途呼叫時 select 輸掉的分支）時，tokio 順手殺子程序。
    command.kill_on_drop(true);
    let mut child = command.spawn()?;
    if let Some(pid) = child.id() {
        crate::inflight::register_child(pid);
    }
    let _pid_guard = ChildPidGuard(child.id());

    let mut stdin = child.stdin.take().expect("stdin piped");
    // 死法③：CLI 起來但不收 stdin（掛在啟動）＝write_all 永卡，60 秒收不完就中止
    tokio::time::timeout(
        std::time::Duration::from_secs(60),
        stdin.write_all(stdin_data.as_bytes()),
    )
    .await
    .map_err(|_| "CLI 60 秒收不進提示詞，已中止")??;
    drop(stdin); // 關閉讓 CLI 知道輸入結束

    // stderr 逐行即時讀（同時兼排空防死鎖）：CLI 的「API Error…重試中」通知走 stderr，
    // 整包等結束才讀會讓玩家對著靜止的進度框發呆到 CLI 重試放棄為止。
    let stderr = child.stderr.take().expect("stderr piped");
    let mut stderr_lines = BufReader::new(stderr).lines();
    let mut stderr_text = String::new();

    let stdout = child.stdout.take().expect("stdout piped");
    let mut lines = BufReader::new(stdout).lines();
    let mut full_text = String::new();
    let mut done: Option<(String, bool)> = None;
    let mut stdout_open = true;
    let mut stderr_open = true;
    // 子程序死法收網（2026-08-12，跨平台 tokio API）：
    // ①程序退出但管線不 EOF（孫程序繼承 fd）：退出後 800ms 沒新行＝強制收尾；
    // ②程序活著但斷流（網路死、CLI 內部卡死）：120 秒無任何 stdout/stderr 行＝殺程序回錯；
    // ③stdin 餵不進（上方 60 秒逾時）；④crash 無收尾事件（迴圈後 exit status 檢查）。
    let mut exited = false;
    let mut stall: Option<String> = None;
    while stdout_open || stderr_open {
        let line = tokio::select! {
            biased;
            line = lines.next_line(), if stdout_open => match line? {
                Some(line) => line,
                None => {
                    stdout_open = false;
                    continue;
                }
            },
            line = stderr_lines.next_line(), if stderr_open => {
                match line? {
                    Some(line) => {
                        if let Some(fatal) = api_error_kind(&line) {
                            // 進度字尾型顯示（卡重構）立刻看得到錯誤；正文串流不混入
                            if thinking_to_delta {
                                on_delta(&format!("\n⚠ {line}\n"));
                            }
                            if fatal {
                                // 設定類錯誤重試不會好，立即中止（kill_on_drop 收掉子程序）
                                return Err(format!("CLI 回覆錯誤：{line}").into());
                            }
                        }
                        stderr_text.push_str(&line);
                        stderr_text.push('\n');
                    }
                    None => stderr_open = false,
                }
                continue;
            },
            status = child.wait(), if !exited => {
                let _ = status?;
                exited = true;
                continue;
            },
            _ = tokio::time::sleep(std::time::Duration::from_millis(800)), if exited => {
                // 程序已亡、管線遲不 EOF＝孫程序繼承了 fd，放棄排空強制收尾
                stdout_open = false;
                stderr_open = false;
                continue;
            },
            _ = tokio::time::sleep(std::time::Duration::from_secs(120)), if !exited => {
                stall = Some("CLI 120 秒無任何輸出（網路或程序卡死），已中止".to_owned());
                break;
            },
        };
        if let Some(log) = &usage_log {
            if let Some(usage) = (log.parse)(&line) {
                eprintln!(
                    "[prompt-cache] transport={} model={} lane={} prompt_tokens={} cached_tokens={} created_tokens={} hit_rate={}",
                    log.transport,
                    log.model,
                    log.lane.as_ref().map_or("-", |lane| lane.lane.as_str()),
                    usage.prompt_tokens,
                    crate::transport::describe(usage.cached_tokens),
                    crate::transport::describe(usage.created_tokens),
                    usage
                        .hit_rate()
                        .map_or_else(|| "—".to_owned(), |rate| format!("{rate:.0}%")),
                );
                crate::usage_log::append_call(
                    log.path,
                    log.world,
                    log.transport,
                    log.model,
                    log.lane.as_ref(),
                    log.shape,
                    usage,
                );
                if let Some(slot) = log.prompt_tokens_out {
                    slot.store(usage.prompt_tokens, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }
        match parse(&line) {
            CliLine::Delta(text) => {
                on_delta(&text);
                full_text.push_str(&text);
            }
            CliLine::Thinking(text) => {
                if thinking_to_delta {
                    on_delta(&text);
                }
            }
            CliLine::Done { text, is_error } => done = Some((text, is_error)),
            CliLine::Other => {}
        }
    }

    if let Some(msg) = stall {
        let _ = child.start_kill();
        if thinking_to_delta {
            on_delta(&format!("\n⚠ {msg}\n"));
        }
        return Err(format!("CLI 回覆錯誤：{msg}").into());
    }
    let status = child.wait().await?;
    if let Some((text, true)) = &done {
        return Err(format!("CLI 回覆錯誤：{text}").into());
    }
    if done.is_none() && !status.success() {
        // 死法④：CLI crash／被系統殺（無收尾事件＋exit 非零）——殘缺正文不能往下走
        let tail: String = stderr_text
            .lines()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .join("\n");
        let msg = format!("CLI 異常結束（{status}）：{tail}");
        if thinking_to_delta {
            on_delta(&format!("\n⚠ {msg}\n"));
        }
        return Err(format!("CLI 回覆錯誤：{msg}").into());
    }
    if full_text.is_empty() {
        // 串流沒抓到增量時退回收尾文字（例如未來旗標行為變動）
        if let Some((text, false)) = &done {
            if !text.is_empty() {
                on_delta(text);
                full_text = text.clone();
            }
        }
    }
    if full_text.is_empty() {
        let tail: String = stderr_text
            .lines()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .join("\n");
        return Err(format!("CLI 沒有產出回覆（exit {status}）：{tail}").into());
    }
    Ok(full_text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::stream::{parse_claude_line, parse_claude_usage};

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
        assert!(error.contains("CLI 異常結束"), "要報 crash 而非靜默：{error}");
        assert!(error.contains("proxy connection reset"), "要帶 stderr 尾巴：{error}");
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
        assert_eq!(api_error_kind("API Error: 529 overloaded, retrying"), Some(false));
        // 設定類：模型不存在／認證，立即中止
        assert_eq!(
            api_error_kind("API Error: 502 unknown provider for model x"),
            Some(true)
        );
        assert_eq!(api_error_kind("API Error: 401 authentication_error"), Some(true));
        // 非錯誤行不動作
        assert_eq!(api_error_kind("thinking hard..."), None);
    }

}
