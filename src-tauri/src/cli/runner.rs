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
            || ["401", "403", "404"]
                .iter()
                .any(|code| lower.contains(code)),
    )
}

#[allow(clippy::too_many_arguments)]
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
mod tests;
