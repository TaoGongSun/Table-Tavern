use super::types::{CliLine, UsageLog};
use crate::data::DataResult;
use crate::transport::{
    context_overflow, runaway_message, RunawayGuard, RunawayPolicy, RunawayReason, LINE_CAP_BYTES,
};
use crate::ui_msg::UiMsg;
use lines::{CappedLines, LineRead};
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::watch;

mod lines;

/// 餵 stdin 的上限：CLI 起來但不收 stdin（掛在啟動）時，寫入會永卡。測試縮短以免反例測試等一分鐘。
#[cfg(not(test))]
const STDIN_WRITE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
#[cfg(test)]
const STDIN_WRITE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// `run_cli_cancellable` 的收場。中止不是錯誤：呼叫端要拿半截文字去做抹寫，不能走失敗重試。
pub enum CliFinish {
    Completed(String),
    Aborted(String),
    /// 輸出失控（runaway-output-cap）：程序已殺。不是錯誤值，免得 lane 把它當續聊失敗重開重試；
    /// 呼叫端照中止收尾後再回 `AI_OUTPUT_RUNAWAY:` 碼。半截不交給玩家（A4），所以不帶。
    Runaway {
        reason: RunawayReason,
        chars: usize,
    },
}

async fn wait_cancel(cancel: &mut Option<watch::Receiver<bool>>) {
    match cancel.as_mut() {
        Some(receiver) => {
            let _ = receiver.wait_for(|flag| *flag).await;
        }
        None => std::future::pending::<()>().await,
    }
}

/// 先送殺，再等它真的退出。已經死掉時 `start_kill` 會失敗，`wait` 仍負責收屍。
async fn kill_child_and_wait(child: &mut Child) {
    let _ = child.start_kill();
    let _ = child.wait().await;
}

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
    thinking_to_delta: bool,
    policy: RunawayPolicy,
    usage_log: Option<UsageLog<'_>>,
    on_delta: impl FnMut(&str),
) -> DataResult<String> {
    match run_cli_cancellable(
        program,
        working_dir,
        args,
        stdin_data,
        envs,
        parse,
        thinking_to_delta,
        policy,
        usage_log,
        on_delta,
        None,
    )
    .await?
    {
        CliFinish::Completed(text) => Ok(text),
        CliFinish::Aborted(_) => Err(UiMsg::CliUnexpectedAbort.into_error()),
        // 不包成 UiMsg：TTMSG 會被 ai_call_failure 再包成 AI_CALL_FAILED，前端認不出碼
        CliFinish::Runaway { reason, chars } => {
            Err(crate::data::invalid_data(runaway_message(reason, chars)))
        }
    }
}

/// 與 `run_cli` 同一條讀迴圈。`cancel` 有值時，中止在迴圈內收掉：殺掉程序並等它退出後，
/// 帶著已經吐出的半截文字返回，不把 future 丟給外層 select。
#[allow(clippy::too_many_arguments)]
pub async fn run_cli_cancellable(
    program: &Path,
    working_dir: &Path,
    args: &[String],
    stdin_data: &str,
    envs: &[(String, String)],
    parse: fn(&str) -> CliLine,
    // 思考增量要不要餵給 on_delta：只有「進度字尾」型顯示（卡重構）開 true；
    // 聊天／旁白的 on_delta 是劇情正文串流，思考混進去會出戲。
    thinking_to_delta: bool,
    // 失控檢查的範圍：呼叫端明確給，不從 usage_log 推（lane 的 shape 固定寫 Oneshot）
    policy: RunawayPolicy,
    usage_log: Option<UsageLog<'_>>,
    mut on_delta: impl FnMut(&str),
    cancel: Option<watch::Receiver<bool>>,
) -> DataResult<CliFinish> {
    #[cfg(feature = "test-harness")]
    let harness_dispatch = crate::harness::ai_dispatch(
        &format!(
            "cli:{}",
            program.file_name().unwrap_or_default().to_string_lossy()
        ),
        &crate::harness::cli_model(args),
        serde_json::json!({
            "world": usage_log.as_ref().and_then(|log| log.world),
            "shape": usage_log.as_ref().map(|log| format!("{:?}", log.shape)),
            "lane": usage_log.as_ref().and_then(|log| log.lane.as_ref().map(|lane| lane.lane.clone())),
        }),
    );
    let mut command = Command::new(program);
    // 先掛系統代理再掛使用者 envs，同名時使用者設定蓋過代理
    crate::cli::proxy::apply_system_proxy(&mut command);
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
    let spawned = command.spawn();
    #[cfg(feature = "test-harness")]
    match &spawned {
        Ok(_) => crate::harness::ai_event(&harness_dispatch, "spawned", serde_json::Value::Null),
        Err(error) => crate::harness::ai_event(
            &harness_dispatch,
            "spawn-failed",
            serde_json::json!({ "error": error.to_string() }),
        ),
    }
    let mut child = spawned?;
    if let Some(pid) = child.id() {
        crate::inflight::register_child(pid);
    }
    let _pid_guard = ChildPidGuard(child.id());
    let result = drive_child(
        &mut child,
        stdin_data,
        parse,
        thinking_to_delta,
        policy,
        usage_log,
        &mut on_delta,
        cancel,
        #[cfg(feature = "test-harness")]
        &harness_dispatch,
    )
    .await;
    // 不論怎麼收場（完成、錯誤、斷流、取消）都先收屍再返回：呼叫端接著會刪提示詞暫存檔，
    // Windows 上程序還開著檔就刪不掉。已經 wait 過的程序再收一次是空操作。
    kill_child_and_wait(&mut child).await;
    result
}

/// spawn 之後的整段：餵 stdin、逐行讀 stdout／stderr、判收尾。任何提早 return 都由呼叫端
/// `run_cli_cancellable` 接著收屍。
#[cfg_attr(feature = "test-harness", allow(clippy::too_many_arguments))]
async fn drive_child(
    child: &mut Child,
    stdin_data: &str,
    parse: fn(&str) -> CliLine,
    thinking_to_delta: bool,
    policy: RunawayPolicy,
    usage_log: Option<UsageLog<'_>>,
    on_delta: &mut impl FnMut(&str),
    mut cancel: Option<watch::Receiver<bool>>,
    #[cfg(feature = "test-harness")] harness_dispatch: &str,
) -> DataResult<CliFinish> {
    // stdin 跟讀迴圈並行：正文改走 stdin 後可能好幾 MB，先寫完才讀的話，CLI 若先大量輸出
    // 再讀 stdin，兩邊各自卡在滿掉的管線上互等。寫完就關 stdin 讓 CLI 知道輸入結束。
    // 死法③：CLI 起來但不收 stdin（掛在啟動）＝寫入永卡，60 秒寫不完就中止。
    let stdin = child.stdin.take().expect("stdin piped");
    let write_stdin = tokio::time::timeout(STDIN_WRITE_TIMEOUT, async move {
        let mut stdin = stdin;
        stdin.write_all(stdin_data.as_bytes()).await
    });
    tokio::pin!(write_stdin);
    let mut stdin_open = true;

    // stderr 逐行即時讀（同時兼排空防死鎖）：CLI 的「API Error…重試中」通知走 stderr，
    // 整包等結束才讀會讓玩家對著靜止的進度框發呆到 CLI 重試放棄為止。
    let stderr = child.stderr.take().expect("stderr piped");
    // 單行上限是記憶體上限，不看政策（A8）
    let mut stderr_lines = CappedLines::new(BufReader::new(stderr), LINE_CAP_BYTES);
    let mut stderr_text = String::new();

    let stdout = child.stdout.take().expect("stdout piped");
    let mut lines = CappedLines::new(BufReader::new(stdout), LINE_CAP_BYTES);
    let mut full_text = String::new();
    let mut done: Option<(String, bool)> = None;
    let mut stdout_open = true;
    let mut stderr_open = true;
    let mut agy_conversation_id: Option<String> = None;
    // 子程序死法收網（2026-08-12，跨平台 tokio API）：
    // ①程序退出但管線不 EOF（孫程序繼承 fd）：退出後 800ms 沒新行＝強制收尾；
    // ②程序活著但斷流（網路死、CLI 內部卡死）：120 秒無任何 stdout/stderr 行＝殺程序回錯；
    //   （API 通道的對應偵測在 transport/stall.rs，數的是模型進展而非輸出行，常數不共用。）
    // ③stdin 餵不進（上方 60 秒逾時）；④crash 無收尾事件（迴圈後 exit status 檢查）。
    let mut exited = false;
    let mut stall: Option<String> = None;
    let mut aborted = false;
    // 輸出失控：正文與思考各一支，分開計（思考只做退化偵測，Off 時兩支都不查）
    let mut text_guard = RunawayGuard::text(policy);
    let mut thinking_guard = RunawayGuard::thinking(policy);
    // 觸發理由＋觸發那一支的計數（思考觸發時正文可能是 0；單行超限記正文計數）
    let mut runaway: Option<(RunawayReason, usize)> = None;
    // 收場要四件都到：兩條輸出 EOF、stdin 寫完（或放棄）、程序退出。只看輸出的話，CLI 先關掉
    // 輸出再讀 stdin（或乾脆不讀）時，迴圈會在還沒寫完時就離開，逾時、取消、斷流偵測全失效。
    while stdout_open || stderr_open || stdin_open || !exited {
        // 收尾還沒到時，取消排在讀管線之前：輸出一直有字時，biased 不會把停止排到後面。
        // 已經收到收尾行就關掉這支：完成與停止同時就緒時，完成贏，交回全文。
        let arm_cancel = cancel.is_some() && done.is_none();
        let line = tokio::select! {
            biased;
            _ = wait_cancel(&mut cancel), if arm_cancel => {
                aborted = true;
                break;
            }
            line = lines.next_line(), if stdout_open => match line? {
                LineRead::Line(line) => line,
                LineRead::Eof => {
                    stdout_open = false;
                    continue;
                }
                LineRead::TooLong => {
                    runaway = Some((RunawayReason::LineTooLong, text_guard.chars()));
                    break;
                }
            },
            line = stderr_lines.next_line(), if stderr_open => {
                match line? {
                    LineRead::TooLong => {
                        runaway = Some((RunawayReason::LineTooLong, text_guard.chars()));
                        break;
                    }
                    LineRead::Line(line) => {
                        if let Some(fatal) = api_error_kind(&line) {
                            // 進度字尾型顯示（卡重構）立刻看得到錯誤；正文串流不混入
                            if thinking_to_delta {
                                on_delta(&format!("\n⚠ {line}\n"));
                            }
                            if fatal {
                                // 設定類錯誤重試不會好，立即中止（kill_on_drop 收掉子程序）
                                if let Some(coded) = context_overflow::cli_failure(&line) {
                                    return Err(crate::data::invalid_data(coded));
                                }
                                return Err(UiMsg::CliReplyError { error: line }.into_error());
                            }
                        }
                        stderr_text.push_str(&line);
                        stderr_text.push('\n');
                    }
                    LineRead::Eof => stderr_open = false,
                }
                continue;
            },
            written = write_stdin.as_mut(), if stdin_open => {
                stdin_open = false;
                match written {
                    Err(_) => return Err(UiMsg::CliStdinTimeout.into_error()),
                    // CLI 沒讀完就退出：交給後面的退出碼／收尾判斷帶出 CLI 自己的錯誤
                    Ok(Err(error)) if error.kind() == std::io::ErrorKind::BrokenPipe => {}
                    Ok(Err(error)) => return Err(error.into()),
                    Ok(Ok(())) => {}
                }
                continue;
            },
            status = child.wait(), if !exited => {
                let _ = status?;
                exited = true;
                continue;
            },
            _ = tokio::time::sleep(std::time::Duration::from_millis(800)), if exited => {
                // 程序已亡、管線遲不 EOF＝孫程序繼承了 fd，放棄排空強制收尾（stdin 同理不再寫）
                stdout_open = false;
                stderr_open = false;
                stdin_open = false;
                continue;
            },
            _ = tokio::time::sleep(std::time::Duration::from_secs(120)), if !exited => {
                // 網路或程序卡死
                stall = Some(UiMsg::CliStalled.to_string());
                break;
            },
        };
        if let Some(log) = usage_log.as_ref().filter(|log| log.transport == "agy") {
            if let Some(id) = super::stream::agy_conversation_id(&line) {
                if log
                    .expected_conversation_id
                    .is_some_and(|expected| expected != id)
                {
                    return Err(UiMsg::AgyConversationMismatch {
                        expected: log.expected_conversation_id.unwrap_or_default().to_owned(),
                        actual: id,
                    }
                    .into_error());
                }
                if let Some(slot) = log.conversation_id_out {
                    *slot
                        .lock()
                        .map_err(|_| UiMsg::AgyLockPoisoned.into_error())? = Some(id.clone());
                }
                agy_conversation_id = Some(id);
            }
        }
        if let Some(slot) = usage_log.as_ref().and_then(|log| log.overage_out) {
            // 超額事件可能在 result 前後任何位置；一次嘗試內見過 true 就不撤銷（那段已按超額計費）
            if super::stream::claude_overage(&line) == Some(true) {
                slot.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
        if let Some(log) = &usage_log {
            if let Some(mut usage) = (log.parse)(&line) {
                // claude 回報實際模型身分與容量：換幕容量判定的上限來源（計畫 §3.4）
                // 辨識不了（多個或都對不上）：這組容量與校正不再可信，整筆作廢（只提醒不鎖）
                if log.transport == "claude" {
                    let capacity_file = log.path.with_file_name("model-capacity.json");
                    match crate::scene_budget::claude_model_usage(&line, log.model) {
                        Some((model_id, context, max_output)) => {
                            crate::scene_budget::record_model(
                                &capacity_file,
                                log.transport,
                                log.model,
                                &model_id,
                                context,
                                max_output,
                            );
                            if let Some(slot) = log.identity_out {
                                if let Ok(mut identity) = slot.lock() {
                                    *identity = Some(model_id);
                                }
                            }
                        }
                        None => crate::scene_budget::forget_model(
                            &capacity_file,
                            log.transport,
                            log.model,
                        ),
                    }
                }
                #[cfg(feature = "test-harness")]
                let mut harness_agy = serde_json::Value::Null;
                if log.transport == "agy" {
                    if let Some(current) = super::stream::agy_usage_counters(&line) {
                        #[cfg(feature = "test-harness")]
                        {
                            harness_agy = serde_json::json!({
                                "current": current,
                                "base": log.agy_usage_base,
                            });
                        }
                        if let Some(base) = log.agy_usage_base {
                            if let Some(delta) = super::stream::parse_agy_usage_delta(current, base)
                            {
                                usage = delta;
                            }
                        }
                        if let Some(slot) = log.agy_usage_out {
                            *slot
                                .lock()
                                .map_err(|_| UiMsg::AgyLockPoisoned.into_error())? = Some(current);
                        }
                    }
                }
                // 原始收尾 usage 與解析後要落帳的數字並排（agy 另附累積值與上輪基準）
                #[cfg(feature = "test-harness")]
                crate::harness::ai_event(
                    harness_dispatch,
                    "usage-raw",
                    serde_json::json!({
                        "raw": crate::harness::raw_usage(&line),
                        "parsed": crate::harness::parsed_usage(&usage),
                        "agy": harness_agy,
                    }),
                );
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
                match super::stream::agy_usage_evidence(&line, agy_conversation_id.as_deref()) {
                    Some(evidence) => crate::usage::log::append_call_with_evidence(
                        log.path,
                        log.world,
                        log.transport,
                        log.model,
                        log.lane.as_ref(),
                        log.shape,
                        usage,
                        &evidence,
                    ),
                    None => crate::usage::log::append_call(
                        log.path,
                        log.world,
                        log.transport,
                        log.model,
                        log.lane.as_ref(),
                        log.shape,
                        usage,
                    ),
                }
                if let Some(slot) = log.prompt_tokens_out {
                    slot.store(usage.prompt_tokens, std::sync::atomic::Ordering::Relaxed);
                }
                if let Some(slot) = log.usage_out {
                    if let Ok(mut held) = slot.lock() {
                        *held = Some(usage);
                    }
                }
            }
        }
        match parse(&line) {
            CliLine::Delta(text) => {
                if let Some(reason) = text_guard.push(&text) {
                    runaway = Some((reason, text_guard.chars()));
                    break;
                }
                on_delta(&text);
                full_text.push_str(&text);
            }
            CliLine::Thinking(text) => {
                if let Some(reason) = thinking_guard.push(&text) {
                    runaway = Some((reason, thinking_guard.chars()));
                    break;
                }
                if thinking_to_delta {
                    on_delta(&text);
                }
            }
            CliLine::Done { text, is_error } => {
                // 零增量時收尾全文會被當成正文採用：收到當下就檢查，不等程序退出
                // （CLI 送完失控的 Done 後若持續吐 stderr 不退，迴圈後的 fallback 永遠輪不到）。
                // 只在這裡計一次，迴圈後採用 fallback 時不再餵 guard。
                if !is_error && full_text.is_empty() {
                    if let Some(reason) = text_guard.push(&text) {
                        runaway = Some((reason, text_guard.chars()));
                        break;
                    }
                }
                done = Some((text, is_error));
            }
            CliLine::Other => {}
        }
    }

    if aborted {
        kill_child_and_wait(child).await;
        return Ok(CliFinish::Aborted(full_text));
    }
    if let Some((reason, chars)) = runaway {
        kill_child_and_wait(child).await;
        if thinking_to_delta {
            on_delta(&format!("\n⚠ {}\n", runaway_message(reason, chars)));
        }
        return Ok(CliFinish::Runaway { reason, chars });
    }
    if let Some(msg) = stall {
        let _ = child.start_kill();
        if thinking_to_delta {
            on_delta(&format!("\n⚠ {msg}\n"));
        }
        return Err(UiMsg::CliReplyError { error: msg }.into_error());
    }
    let status = child.wait().await?;
    if let Some((text, true)) = &done {
        // 容量爆掉／生成撞到容量上限：掛穩定碼原樣上拋，前端認得出、換幕流程據此縮塊
        if let Some(coded) = context_overflow::cli_failure(text) {
            return Err(crate::data::invalid_data(coded));
        }
        if text.starts_with("AI_INCOMPLETE_RESPONSE:") {
            return Err(crate::data::invalid_data(text.clone()));
        }
        return Err(UiMsg::CliReplyError {
            error: text.clone(),
        }
        .into_error());
    }
    if done.is_none() && !status.success() {
        // 死法④：CLI crash／被系統殺（無收尾事件＋exit 非零）——殘缺正文不能往下走
        let tail: String = stderr_text
            .lines()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .join("\n");
        let msg = UiMsg::CliCrashed {
            status: status.to_string(),
            tail,
        }
        .to_string();
        if thinking_to_delta {
            on_delta(&format!("\n⚠ {msg}\n"));
        }
        return Err(UiMsg::CliReplyError { error: msg }.into_error());
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
        return Err(UiMsg::CliNoReply {
            status: status.to_string(),
            tail,
        }
        .into_error());
    }
    Ok(CliFinish::Completed(full_text))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod runaway_tests;
