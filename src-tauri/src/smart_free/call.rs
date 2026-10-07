//! 穩定免費的一次邏輯呼叫（計畫 §2.3、§6）：最多派送兩次；第 1 發的失敗讓換模成功提交、
//! 且還沒吐出正文時，才用新選的模型重送一次。取消＝呼叫端 drop 這個 future，
//! 正在等的派送、`/key` 查詢、重送都跟著停，沒回到鎖內的結果一律不計。

use super::failover::{self, ChatRecord, FailureClass, KeyCheck, Ticket};
use super::store::CurrentState;
use crate::transport::{ApiFailure, SmartChatResult};
use crate::ui_msg::UiMsg;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub type PersistSend<'a> = &'a mut (dyn FnMut(&CurrentState) -> std::io::Result<()> + Send);

/// 這一句的選模素材：名單、名單外穩定候選、放得下本句的模型、顯示名。
#[derive(Debug, Clone, Default)]
pub struct CallPlan {
    pub account: String,
    pub config_key: String,
    pub lineup: Vec<String>,
    pub others: Vec<String>,
    pub fits: BTreeSet<String>,
    pub names: BTreeMap<String, String>,
    /// 送出用的設定（呼叫開始時讀的）與磁碟上的最新設定是否同一份；不同就只發不可計數的票。
    pub matches_sender: bool,
}

impl CallPlan {
    /// 顯示名：catalog 對得到就用名稱，對不到退回 model id。
    pub fn name(&self, model: &str) -> String {
        self.names
            .get(model)
            .cloned()
            .unwrap_or_else(|| model.to_owned())
    }
}

pub(crate) trait CallEnv {
    /// 取票當下的權威選模素材：重讀磁碟上的設定與快取（不連網、可在鎖內呼叫）。
    fn plan(&mut self) -> Result<CallPlan, String>;
    async fn send(&mut self, model: &str) -> Result<SmartChatResult, ApiFailure>;
    async fn daily_remaining(&mut self) -> Option<i64>;
    /// 換模已提交：發 `smart-free-failover`（retried＝這一句會用新模型重送）。
    fn failover(&mut self, from: &str, to: &str, retried: bool);
    /// 記下這桌實際回話的模型；回 true＝這桌換手了。
    fn record_responder(&mut self, model: &str) -> bool;
    /// 發 `smart-free-model-switched`（顯示名）。
    fn switched(&mut self, name: &str);
}

#[derive(Debug)]
pub struct CallOutcome {
    pub result: SmartChatResult,
}

async fn classify_with_key(failure: &ApiFailure, env: &mut impl CallEnv) -> FailureClass {
    let key = if failover::needs_key_check(failure) {
        match env.daily_remaining().await {
            Some(remaining) => KeyCheck::Remaining(remaining),
            None => KeyCheck::Failed,
        }
    } else {
        KeyCheck::NotQueried
    };
    failover::classify(failure, key)
}

/// 回給前端的錯誤：模型層級掛 `AI_FREE_MODEL_BUSY:`；UnknownRateLimit 保留原錯誤，不宣稱上游擁擠。
fn display(failure: &ApiFailure, class: FailureClass) -> String {
    // 停滯逾時＝我們自己等不到輸出；輸出失控＝我們自己喊停。兩者都不宣稱上游擁擠，換模判斷仍照 class
    let own_code = [
        crate::transport::STALLED_CODE,
        crate::transport::RUNAWAY_CODE,
    ]
    .iter()
    .any(|code| failure.display.starts_with(code));
    let raw = match class {
        _ if own_code => failure.display.clone(),
        FailureClass::Model | FailureClass::Gone => {
            format!("AI_FREE_MODEL_BUSY: {}", failure.display)
        }
        _ => failure.display.clone(),
    };
    crate::transport::dispatch::ai_call_failure(raw)
}

fn record(
    root: &Path,
    plan: &CallPlan,
    ticket: &Ticket,
    failure: Option<FailureClass>,
    allow_switch: bool,
    now: u64,
    persist: PersistSend,
) -> ChatRecord {
    failover::with_runtime(root, |runtime| {
        runtime.record_chat(ticket, failure, allow_switch, &plan.lineup, now, persist)
    })
}

/// 成功收尾：記回話者；同一呼叫已發過換模提示就不再發換手提示（§2.5）。
fn finish(
    plan: &CallPlan,
    env: &mut impl CallEnv,
    result: SmartChatResult,
    failover_notified: bool,
) -> CallOutcome {
    if let Some(model) = result.model.as_deref() {
        if env.record_responder(model) && !failover_notified {
            env.switched(&plan.name(model));
        }
    }
    CallOutcome { result }
}

pub(crate) async fn run_call(
    root: &Path,
    env: &mut impl CallEnv,
    persist: PersistSend<'_>,
    now: u64,
) -> Result<CallOutcome, String> {
    // 取票時才讀權威設定與名單（不用 await 之前備好的舊素材），舊呼叫不能把狀態重建回舊帳號。
    let (plan, first) = failover::with_runtime(root, |runtime| {
        let plan = env.plan()?;
        runtime.observe_config(&plan.config_key);
        let current = runtime.reconcile(&plan.account, &plan.lineup, now, persist);
        let fits = |model: &str| plan.fits.contains(model);
        let ticket = match current {
            Some(current) if plan.matches_sender => {
                failover::pick_for_sentence(&current, &plan.lineup, &plan.others, fits, &[])
                    .map(|(selected, substitute)| runtime.ticket(&current, &selected, substitute))
            }
            // 重建落盤失敗或送出設定不是最新：照樣送，但結果一律不計、不換模
            current => {
                let base = current
                    .or_else(|| plan.lineup.first().cloned())
                    .unwrap_or_default();
                failover::pick_for_sentence(&base, &plan.lineup, &plan.others, fits, &[])
                    .map(|(selected, _)| runtime.uncountable_ticket(&selected))
            }
        };
        let ticket = ticket.ok_or_else(|| UiMsg::NoFreeModels.to_string())?;
        Ok::<_, String>((plan, ticket))
    })?;

    let failure = match env.send(&first.selected_model).await {
        Ok(result) => {
            record(root, &plan, &first, None, true, now, persist);
            return Ok(finish(&plan, env, result, false));
        }
        Err(failure) => failure,
    };
    let class = classify_with_key(&failure, env).await;
    // 記錄第一發、換模、重讀權威素材、取重送票在同一次鎖內完成：中間沒有別的執行緒插得進來。
    let (record, retry) = failover::with_runtime(root, |runtime| {
        let record = runtime.record_chat(&first, Some(class), true, &plan.lineup, now, persist);
        // 已吐出正文就不重送〔作者裁決 2026-10-04〕：兩支模型的半截會接在一起。
        let retry = match &record {
            ChatRecord::Switched { .. } if !failure.emitted_text => {
                retry_ticket(runtime, env, &first)
            }
            _ => None,
        };
        (record, retry)
    });
    match record {
        ChatRecord::Switched { from, to } => {
            env.failover(&plan.name(&from), &plan.name(&to), retry.is_some());
            let Some((retry_plan, second)) = retry else {
                return Err(display(&failure, class));
            };
            match env.send(&second.selected_model).await {
                Ok(result) => {
                    record_result(root, &retry_plan, &second, None, now, persist);
                    Ok(finish(&retry_plan, env, result, true))
                }
                Err(retry_failure) => {
                    let retry_class = classify_with_key(&retry_failure, env).await;
                    record_result(root, &retry_plan, &second, Some(retry_class), now, persist);
                    Err(display(&retry_failure, retry_class))
                }
            }
        }
        ChatRecord::AllBusy => Err(UiMsg::SmartFreeAllBusy.to_string()),
        _ => Err(display(&failure, class)),
    }
}

/// 重送的票（鎖內）：和第一發一樣重讀權威設定與名單；送出設定已不是最新、帳號或名單世代已變，就不重送。
/// 候選排除第一發的模型與 exhausted。
fn retry_ticket(
    runtime: &mut failover::Runtime,
    env: &mut impl CallEnv,
    first: &Ticket,
) -> Option<(CallPlan, Ticket)> {
    let fresh = env.plan().ok()?;
    let gen = super::select::lineup_gen(&fresh.lineup);
    if !fresh.matches_sender
        || fresh.account != first.account
        || fresh.account != runtime.saved.account
        || gen != runtime.saved.lineup_gen
    {
        return None;
    }
    runtime.observe_config(&fresh.config_key);
    let current = runtime.saved.model.clone();
    let mut exclude = vec![first.selected_model.clone()];
    exclude.extend(runtime.saved.exhausted.iter().cloned());
    let fits = |model: &str| fresh.fits.contains(model);
    let ticket =
        failover::pick_for_sentence(&current, &fresh.lineup, &fresh.others, fits, &exclude)
            .map(|(selected, substitute)| runtime.ticket(&current, &selected, substitute))?;
    Some((fresh, ticket))
}

/// 第二發的結果：照票核對計入，但本呼叫內不再換模。
fn record_result(
    root: &Path,
    plan: &CallPlan,
    ticket: &Ticket,
    failure: Option<FailureClass>,
    now: u64,
    persist: PersistSend,
) -> ChatRecord {
    record(root, plan, ticket, failure, false, now, persist)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::time::Duration;

    enum Reply {
        Ok,
        Fail(ApiFailure),
        /// 永遠不回：模擬卡住的派送，用來測取消（drop）。
        Hang,
    }

    #[derive(Default)]
    struct FakeEnv {
        replies: VecDeque<Reply>,
        sent: Vec<String>,
        notices: Vec<(String, String, bool)>,
        daily: Option<i64>,
        hang_daily: bool,
        daily_queries: usize,
        plan: CallPlan,
        /// record_responder 的回傳（模擬這桌換手）
        responder_switch: bool,
        switched: Vec<String>,
        plan_calls: usize,
        /// 第 n 次 plan()（從 1 起算）改回這份素材：模擬取票當下磁碟設定已變
        plan_override: Option<(usize, CallPlan)>,
    }

    impl CallEnv for FakeEnv {
        async fn send(&mut self, model: &str) -> Result<SmartChatResult, ApiFailure> {
            self.sent.push(model.to_owned());
            match self.replies.pop_front().unwrap_or(Reply::Ok) {
                Reply::Ok => Ok(SmartChatResult {
                    text: "好".to_owned(),
                    model: Some(model.to_owned()),
                    truncated: None,
                }),
                Reply::Fail(failure) => Err(failure),
                Reply::Hang => std::future::pending().await,
            }
        }
        async fn daily_remaining(&mut self) -> Option<i64> {
            self.daily_queries += 1;
            if self.hang_daily {
                std::future::pending::<()>().await;
            }
            self.daily
        }
        fn failover(&mut self, from: &str, to: &str, retried: bool) {
            self.notices.push((from.to_owned(), to.to_owned(), retried));
        }
        fn plan(&mut self) -> Result<CallPlan, String> {
            self.plan_calls += 1;
            match &self.plan_override {
                Some((at, plan)) if *at == self.plan_calls => Ok(plan.clone()),
                _ => Ok(self.plan.clone()),
            }
        }
        fn record_responder(&mut self, _model: &str) -> bool {
            self.responder_switch
        }
        fn switched(&mut self, name: &str) {
            self.switched.push(name.to_owned());
        }
    }

    fn http(status: u16) -> Reply {
        Reply::Fail(ApiFailure {
            status: Some(status),
            display: format!("AI_HTTP_STATUS_{status}: status={status} body="),
            ..ApiFailure::default()
        })
    }

    fn plan(fits: &[&str]) -> CallPlan {
        CallPlan {
            account: "acct".to_owned(),
            config_key: "stable|acct|official".to_owned(),
            lineup: ["a", "b", "c"].iter().map(|id| (*id).to_owned()).collect(),
            others: vec!["long".to_owned()],
            fits: fits.iter().map(|id| (*id).to_owned()).collect(),
            names: BTreeMap::from([
                ("a".to_owned(), "Model A".to_owned()),
                ("b".to_owned(), "Model B".to_owned()),
            ]),
            matches_sender: true,
        }
    }

    fn env(plan: CallPlan, replies: Vec<Reply>) -> FakeEnv {
        FakeEnv {
            replies: VecDeque::from(replies),
            plan,
            ..FakeEnv::default()
        }
    }

    fn ok_persist() -> impl FnMut(&CurrentState) -> std::io::Result<()> + Send {
        |_: &CurrentState| Ok(())
    }

    fn state(root: &Path) -> (String, u32, Vec<String>) {
        failover::with_runtime(root, |runtime| {
            (
                runtime.saved.model.clone(),
                runtime.count,
                runtime.saved.exhausted.clone(),
            )
        })
    }

    #[tokio::test]
    async fn second_consecutive_failure_switches_and_retries_the_same_sentence_once() {
        let root = super::super::test_root("call-retry");
        let plan = plan(&["a", "b", "c"]);
        let mut persist = ok_persist();
        let mut env = FakeEnv {
            plan: plan.clone(),
            replies: VecDeque::from([http(503), http(503), Reply::Ok]),
            ..FakeEnv::default()
        };
        let first = run_call(&root, &mut env, &mut persist, 1).await;
        assert!(first
            .unwrap_err()
            .starts_with("AI_FREE_MODEL_BUSY: AI_HTTP_STATUS_503"));
        let second = run_call(&root, &mut env, &mut persist, 2).await.unwrap();
        assert!(env.switched.is_empty());
        assert_eq!(second.result.model.as_deref(), Some("b"));
        assert_eq!(env.sent, ["a", "a", "b"]);
        assert_eq!(
            env.notices,
            [("Model A".to_owned(), "Model B".to_owned(), true)]
        );
        // 重送用新的目前模型（非替代）成功＝確認可用，exhausted 跟著清空（§4.4）
        assert_eq!(state(&root), ("b".to_owned(), 0, Vec::new()));
        let _ = std::fs::remove_dir_all(root);
    }

    /// 停滯逾時分類仍是 Model（連兩次照樣換模），但回給前端的碼是 AI_STREAM_STALLED，
    /// 不被包成「免費模型擁擠」。
    #[tokio::test]
    async fn stalled_stream_keeps_its_own_code_but_still_counts_toward_switching() {
        let root = super::super::test_root("call-stalled");
        let plan = plan(&["a", "b", "c"]);
        let mut persist = ok_persist();
        let stalled = ApiFailure::stalled(120, false);
        let mut env = FakeEnv {
            plan: plan.clone(),
            replies: VecDeque::from([
                Reply::Fail(stalled.clone()),
                Reply::Fail(stalled),
                Reply::Ok,
            ]),
            ..FakeEnv::default()
        };
        let first = run_call(&root, &mut env, &mut persist, 1)
            .await
            .unwrap_err();
        assert!(first.starts_with("AI_STREAM_STALLED:"), "{first}");
        let second = run_call(&root, &mut env, &mut persist, 2).await.unwrap();
        assert_eq!(second.result.model.as_deref(), Some("b"));
        assert_eq!(env.sent, ["a", "a", "b"]);
        let _ = std::fs::remove_dir_all(root);
    }

    /// 輸出失控：只派送一次（首塊就觸發也不重送），不換模，碼原樣交給前端。
    #[tokio::test]
    async fn runaway_is_sent_once_never_switches_and_keeps_its_code() {
        let root = super::super::test_root("call-runaway");
        let plan = plan(&["a", "b", "c"]);
        let mut persist = ok_persist();
        let runaway = ApiFailure::runaway(crate::transport::runaway_message(
            crate::transport::RunawayReason::WhitespaceRun,
            2000,
        ));
        let mut env = FakeEnv {
            plan: plan.clone(),
            replies: VecDeque::from([
                Reply::Fail(runaway.clone()),
                Reply::Fail(runaway),
                Reply::Ok,
            ]),
            ..FakeEnv::default()
        };
        for now in [1, 2] {
            let error = run_call(&root, &mut env, &mut persist, now)
                .await
                .unwrap_err();
            assert!(error.starts_with("AI_OUTPUT_RUNAWAY:"), "{error}");
        }
        assert_eq!(env.sent, ["a", "a"]);
        assert!(env.notices.is_empty());
        assert_eq!(state(&root).0, "a");
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn emitted_text_blocks_retry_but_switch_still_commits() {
        let root = super::super::test_root("call-emitted");
        let plan = plan(&["a", "b", "c"]);
        let mut persist = ok_persist();
        let half = ApiFailure {
            stage: crate::transport::FailureStage::Network,
            emitted_text: true,
            display: "stream cut".to_owned(),
            ..ApiFailure::default()
        };
        let mut env = FakeEnv {
            plan: plan.clone(),
            replies: VecDeque::from([http(502), Reply::Fail(half.clone())]),
            ..FakeEnv::default()
        };
        let _ = run_call(&root, &mut env, &mut persist, 1).await;
        let error = run_call(&root, &mut env, &mut persist, 1)
            .await
            .unwrap_err();
        assert!(error.starts_with("AI_FREE_MODEL_BUSY:"));
        assert_eq!(env.sent, ["a", "a"]);
        assert_eq!(
            env.notices,
            [("Model A".to_owned(), "Model B".to_owned(), false)]
        );
        assert_eq!(state(&root).0, "b");
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn retry_failure_counts_once_and_never_chains_a_second_switch() {
        let root = super::super::test_root("call-retry-fail");
        let plan = plan(&["a", "b", "c"]);
        let mut persist = ok_persist();
        let mut env = FakeEnv {
            plan: plan.clone(),
            replies: VecDeque::from([http(503), http(503), http(404)]),
            ..FakeEnv::default()
        };
        let _ = run_call(&root, &mut env, &mut persist, 1).await;
        let error = run_call(&root, &mut env, &mut persist, 1)
            .await
            .unwrap_err();
        assert!(error.contains("AI_HTTP_STATUS_404"));
        assert_eq!(env.sent, ["a", "a", "b"]);
        // 404 在第 2 發只記到 2、不換模；下一次可計數失敗才換
        assert_eq!(state(&root).0, "b");
        assert_eq!(state(&root).1, 2);
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn retry_skips_the_failed_model_and_exhausted_and_may_give_up() {
        // A 放得下本句、連敗兩次換到 B；B 放不下，替代搜尋不挑回 A → 找名單外長 context
        let root = super::super::test_root("call-retry-pick");
        let plan_long = plan(&["a", "long"]);
        let mut persist = ok_persist();
        let mut env = FakeEnv {
            plan: plan_long.clone(),
            replies: VecDeque::from([http(503), http(503), Reply::Ok]),
            ..FakeEnv::default()
        };
        let _ = run_call(&root, &mut env, &mut persist, 1).await;
        let outcome = run_call(&root, &mut env, &mut persist, 1).await.unwrap();
        assert_eq!(env.sent, ["a", "a", "long"]);
        assert_eq!(outcome.result.model.as_deref(), Some("long"));
        // 替代成功不歸零全域目前模型（b）的計數，也不清 exhausted
        assert_eq!(state(&root), ("b".to_owned(), 0, vec!["a".to_owned()]));
        let _ = std::fs::remove_dir_all(root);

        // 都沒有合格候選：不派第 2 發，換模保留，回第 1 次失敗
        let root = super::super::test_root("call-retry-none");
        let only_a = plan(&["a"]);
        let mut env = FakeEnv {
            plan: only_a.clone(),
            replies: VecDeque::from([http(503), http(503)]),
            ..FakeEnv::default()
        };
        let _ = run_call(&root, &mut env, &mut persist, 1).await;
        let error = run_call(&root, &mut env, &mut persist, 1)
            .await
            .unwrap_err();
        assert!(error.contains("503"));
        assert_eq!(env.sent, ["a", "a"]);
        assert_eq!(env.notices[0].2, false);
        assert_eq!(state(&root).0, "b");
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn account_errors_never_switch_and_unknown_429_uses_limited_failover() {
        let root = super::super::test_root("call-account");
        let plan = plan(&["a", "b", "c"]);
        let mut persist = ok_persist();
        let mut env = FakeEnv {
            plan: plan.clone(),
            replies: VecDeque::from([http(402), http(402), http(402)]),
            ..FakeEnv::default()
        };
        for _ in 0..3 {
            let _ = run_call(&root, &mut env, &mut persist, 1).await;
        }
        assert_eq!(state(&root), ("a".to_owned(), 0, Vec::new()));
        // 無證據的 429＋/key 查不到：UnknownRateLimit，連 2 次換模；原錯誤保留不掛 BUSY
        let mut env = FakeEnv {
            plan: plan.clone(),
            replies: VecDeque::from([http(429), http(429), Reply::Ok]),
            ..FakeEnv::default()
        };
        let error = run_call(&root, &mut env, &mut persist, 1)
            .await
            .unwrap_err();
        assert!(error.starts_with("AI_HTTP_STATUS_429"));
        assert!(run_call(&root, &mut env, &mut persist, 1).await.is_ok());
        assert_eq!(env.daily_queries, 2);
        assert_eq!(state(&root).0, "b");
        // 每日用完（/key 剩 0）：帳號層級
        let mut env = FakeEnv {
            plan: plan.clone(),
            replies: VecDeque::from([http(429), http(429)]),
            daily: Some(0),
            ..FakeEnv::default()
        };
        for _ in 0..2 {
            let _ = run_call(&root, &mut env, &mut persist, 1).await;
        }
        assert_eq!(state(&root).0, "b");
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn all_busy_returns_backend_message() {
        let root = super::super::test_root("call-all-busy");
        let mut plan = plan(&["a", "b"]);
        plan.lineup.truncate(2);
        let mut persist = ok_persist();
        let mut env = FakeEnv {
            plan: plan.clone(),
            replies: VecDeque::from([http(503), http(503), http(503), http(503)]),
            ..FakeEnv::default()
        };
        // a 連敗 → 換 b（重送 b 也敗，計 1）；b 再敗 → 名單全 exhausted
        let _ = run_call(&root, &mut env, &mut persist, 1).await;
        let _ = run_call(&root, &mut env, &mut persist, 1).await;
        let error = run_call(&root, &mut env, &mut persist, 1)
            .await
            .unwrap_err();
        assert_eq!(error, UiMsg::SmartFreeAllBusy.to_string());
        assert_eq!(state(&root).2, ["a", "b"]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn cancelling_during_key_check_or_retry_records_nothing_more() {
        // 取消＝呼叫端 drop future（chat.rs 的 select!）；這裡用 timeout 觸發同樣的 drop。
        // 取消在 /key 查詢中：第 1 發的失敗沒回到鎖內，不計
        let root = super::super::test_root("call-cancel-key");
        let plan = plan(&["a", "b", "c"]);
        let mut persist = ok_persist();
        let mut env = FakeEnv {
            plan: plan.clone(),
            replies: VecDeque::from([http(429)]),
            hang_daily: true,
            ..FakeEnv::default()
        };
        let cancelled = tokio::time::timeout(
            Duration::from_millis(50),
            run_call(&root, &mut env, &mut persist, 1),
        )
        .await;
        assert!(cancelled.is_err());
        assert_eq!(env.daily_queries, 1);
        assert_eq!(state(&root), ("a".to_owned(), 0, Vec::new()));
        let _ = std::fs::remove_dir_all(root);

        // 取消在重送中：換模已提交，第 2 發沒回來，不計
        let root = super::super::test_root("call-cancel-retry");
        let mut env = FakeEnv {
            plan: plan.clone(),
            replies: VecDeque::from([http(503), http(503), Reply::Hang]),
            ..FakeEnv::default()
        };
        let _ = run_call(&root, &mut env, &mut persist, 1).await;
        let cancelled = tokio::time::timeout(
            Duration::from_millis(50),
            run_call(&root, &mut env, &mut persist, 1),
        )
        .await;
        assert!(cancelled.is_err());
        assert_eq!(env.sent, ["a", "a", "b"]);
        assert_eq!(env.notices.len(), 1);
        assert_eq!(state(&root), ("b".to_owned(), 0, vec!["a".to_owned()]));
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn sentence_too_long_for_every_candidate_is_a_length_error_not_busy() {
        let root = super::super::test_root("call-too-long");
        let mut env = env(plan(&[]), Vec::new());
        let error = run_call(&root, &mut env, &mut ok_persist(), 1)
            .await
            .unwrap_err();
        assert_eq!(error, UiMsg::NoFreeModels.to_string());
        assert!(env.sent.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn stale_sender_config_never_counts_or_rebuilds_the_old_account() {
        let root = super::super::test_root("call-stale-sender");
        let mut persist = ok_persist();
        // 新帳號先建立狀態
        let mut fresh_plan = plan(&["a", "b", "c"]);
        fresh_plan.account = "new".to_owned();
        fresh_plan.config_key = "stable|new|official".to_owned();
        let mut first = env(fresh_plan.clone(), vec![Reply::Ok]);
        run_call(&root, &mut first, &mut persist, 1).await.unwrap();
        // 舊呼叫（送出用的是舊金鑰）：取票時讀到的權威素材是新帳號，但 matches_sender=false
        let mut stale_plan = fresh_plan.clone();
        stale_plan.matches_sender = false;
        let mut stale = env(stale_plan, vec![http(503), http(503), http(503)]);
        for _ in 0..3 {
            let _ = run_call(&root, &mut stale, &mut persist, 2).await;
        }
        assert_eq!(stale.sent, ["a", "a", "a"]);
        assert!(stale.notices.is_empty());
        let saved = failover::with_runtime(&root, |runtime| {
            (runtime.saved.account.clone(), runtime.count)
        });
        assert_eq!(saved, ("new".to_owned(), 0));
        assert_eq!(state(&root).0, "a");
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn switched_notice_uses_display_name_and_is_suppressed_after_failover() {
        let root = super::super::test_root("call-switched");
        let mut persist = ok_persist();
        let mut normal = env(plan(&["a", "b", "c"]), vec![Reply::Ok]);
        normal.responder_switch = true;
        run_call(&root, &mut normal, &mut persist, 1).await.unwrap();
        assert_eq!(normal.switched, ["Model A"]);
        // 顯示名查不到時退回 id
        let mut unnamed = plan(&["a", "b", "c"]);
        unnamed.names.clear();
        let mut bare = env(unnamed, vec![Reply::Ok]);
        bare.responder_switch = true;
        run_call(&root, &mut bare, &mut persist, 1).await.unwrap();
        assert_eq!(bare.switched, ["a"]);
        // 換模後重送成功：只有 failover 提示，不再發換手提示
        let mut failing = env(
            plan(&["a", "b", "c"]),
            vec![http(503), http(503), Reply::Ok],
        );
        failing.responder_switch = true;
        let _ = run_call(&root, &mut failing, &mut persist, 1).await;
        let _outcome = run_call(&root, &mut failing, &mut persist, 1)
            .await
            .unwrap();
        assert_eq!(failing.notices.len(), 1);
        assert!(failing.switched.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn retry_rereads_authority_and_skips_when_settings_changed() {
        // 第 2 句：第一發失敗提交換模；取重送票時重讀到的設定已換帳號（送出設定過期）→ 不重送、
        // 換模保留、回第一發的失敗。plan() 呼叫順序：第 1 句取票、第 2 句取票、第 2 句重送取票。
        let root = super::super::test_root("call-retry-authority");
        let mut persist = ok_persist();
        let mut moved = plan(&["a", "b", "c"]);
        moved.account = "new".to_owned();
        moved.config_key = "stable|new|official".to_owned();
        moved.matches_sender = false;
        let mut env = env(
            plan(&["a", "b", "c"]),
            vec![http(503), http(503), Reply::Ok],
        );
        env.plan_override = Some((3, moved));
        let _ = run_call(&root, &mut env, &mut persist, 1).await;
        let error = run_call(&root, &mut env, &mut persist, 1)
            .await
            .unwrap_err();
        assert!(error.contains("503"));
        assert_eq!(env.sent, ["a", "a"]);
        assert_eq!(env.plan_calls, 3);
        assert_eq!(
            env.notices,
            [("Model A".to_owned(), "Model B".to_owned(), false)]
        );
        assert_eq!(state(&root).0, "b");

        // 同帳號但名單世代已變（重讀的名單和 runtime 不一致）也不重送
        let root = super::super::test_root("call-retry-lineup");
        let mut reordered = plan(&["a", "b", "c"]);
        reordered.lineup = ["c", "a", "b"].iter().map(|id| (*id).to_owned()).collect();
        let mut env = env_with_override(reordered);
        let _ = run_call(&root, &mut env, &mut persist, 1).await;
        let _ = run_call(&root, &mut env, &mut persist, 1).await;
        assert_eq!(env.sent, ["a", "a"]);
        let _ = std::fs::remove_dir_all(root);
    }

    fn env_with_override(at_retry: CallPlan) -> FakeEnv {
        let mut env = env(
            plan(&["a", "b", "c"]),
            vec![http(503), http(503), Reply::Ok],
        );
        env.plan_override = Some((3, at_retry));
        env
    }

    /// 真 transport（`stream_chat_models`）＋本機假伺服器：斷言實際送出的請求、換模事件、用量帳本。
    mod live_transport {
        use super::*;
        use crate::data::AppConfig;
        use crate::transport::ChatMessage;
        use std::io::{Read, Write};
        use std::sync::{Arc, Mutex};

        fn serve(responses: Vec<String>) -> (String, Arc<Mutex<Vec<String>>>) {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let models = Arc::new(Mutex::new(Vec::new()));
            let seen = models.clone();
            std::thread::spawn(move || {
                for body in responses {
                    let (mut socket, _) = listener.accept().unwrap();
                    let mut raw = Vec::new();
                    let mut chunk = [0u8; 4096];
                    loop {
                        let read = socket.read(&mut chunk).unwrap();
                        raw.extend_from_slice(&chunk[..read]);
                        let text = String::from_utf8_lossy(&raw).to_string();
                        if let Some(split) = text.find("\r\n\r\n") {
                            let length = text[..split]
                                .lines()
                                .find_map(|line| {
                                    line.to_ascii_lowercase()
                                        .strip_prefix("content-length:")
                                        .map(|value| value.trim().parse::<usize>().unwrap())
                                })
                                .unwrap_or(0);
                            if raw.len() >= split + 4 + length {
                                let request: serde_json::Value =
                                    serde_json::from_slice(&raw[split + 4..split + 4 + length])
                                        .unwrap();
                                seen.lock()
                                    .unwrap()
                                    .push(request["model"].as_str().unwrap().to_owned());
                                break;
                            }
                        }
                        if read == 0 {
                            break;
                        }
                    }
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    socket.write_all(response.as_bytes()).unwrap();
                }
            });
            (format!("http://{address}"), models)
        }

        /// 串流先吐用量、再吐上游擁擠錯誤（沒有正文）：帳本記一筆，分類為模型層級。
        fn overloaded_with_usage() -> String {
            concat!(
                "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":11}}\n\n",
                "data: {\"error\":{\"code\":503,\"message\":\"overloaded\",\"metadata\":{\"error_type\":\"provider_overloaded\"}},\"choices\":[{\"finish_reason\":\"error\"}]}\n\n",
                "data: [DONE]\n\n",
            )
            .to_owned()
        }

        fn success(model: &str) -> String {
            format!(
                "data: {{\"model\":\"{model}\",\"choices\":[{{\"delta\":{{\"content\":\"好\"}}}}]}}\n\ndata: {{\"model\":\"{model}\",\"choices\":[{{\"delta\":{{}},\"finish_reason\":\"stop\"}}],\"usage\":{{\"prompt_tokens\":22}}}}\n\ndata: [DONE]\n\n"
            )
        }

        struct LiveEnv {
            config: AppConfig,
            plan: CallPlan,
            ledger: std::path::PathBuf,
            events: Vec<serde_json::Value>,
            deltas: String,
        }

        impl CallEnv for LiveEnv {
            fn plan(&mut self) -> Result<CallPlan, String> {
                Ok(self.plan.clone())
            }
            async fn send(&mut self, model: &str) -> Result<SmartChatResult, ApiFailure> {
                let messages = [ChatMessage {
                    role: "user".to_owned(),
                    content: "嗨".to_owned(),
                }];
                let deltas = &mut self.deltas;
                crate::transport::stream_chat_models(
                    &self.config,
                    model,
                    &messages,
                    Some(&self.ledger),
                    Some("w1"),
                    crate::usage::log::PromptShape::Oneshot,
                    crate::transport::RunawayPolicy::Off,
                    |delta| deltas.push_str(delta),
                )
                .await
            }
            async fn daily_remaining(&mut self) -> Option<i64> {
                None
            }
            fn failover(&mut self, from: &str, to: &str, retried: bool) {
                self.events.push(super::super::super::failover_payload(
                    Some("w1"),
                    Some("turn-1"),
                    from,
                    to,
                    retried,
                ));
            }
            fn record_responder(&mut self, _model: &str) -> bool {
                true
            }
            fn switched(&mut self, name: &str) {
                self.events.push(super::super::super::switched_payload(
                    Some("w1"),
                    Some("turn-1"),
                    name,
                ));
            }
        }

        #[tokio::test]
        async fn two_failures_switch_retry_and_every_dispatch_writes_its_own_ledger_line() {
            let root = super::super::super::test_root("call-live");
            let (base, models) = serve(vec![
                overloaded_with_usage(),
                overloaded_with_usage(),
                success("b"),
            ]);
            let mut config = AppConfig::default();
            config
                .preferences
                .insert("base_url".to_owned(), serde_json::Value::String(base));
            let ledger = root.join("prompt-cache.jsonl");
            let mut live = LiveEnv {
                config,
                plan: plan(&["a", "b", "c"]),
                ledger: ledger.clone(),
                events: Vec::new(),
                deltas: String::new(),
            };
            let mut persist = ok_persist();
            let first = run_call(&root, &mut live, &mut persist, 1)
                .await
                .unwrap_err();
            assert!(first.starts_with("AI_FREE_MODEL_BUSY:"));
            let outcome = run_call(&root, &mut live, &mut persist, 1).await.unwrap();
            assert_eq!(outcome.result.text, "好");
            assert_eq!(live.deltas, "好");
            // 請求：a 兩次、換到 b 重送一次
            assert_eq!(*models.lock().unwrap(), ["a", "a", "b"]);
            // 事件：只有一則換模提示（顯示名、retried），重送成功不再發換手提示
            assert_eq!(live.events.len(), 1);
            let event = &live.events[0];
            assert_eq!(event["from"], "Model A");
            assert_eq!(event["to"], "Model B");
            assert_eq!(event["retried"], true);
            assert_eq!(event["world"], "w1");
            assert_eq!(event["turnId"], "turn-1");
            // 帳本：三次派送各一行，各帶自己的 model
            let lines: Vec<serde_json::Value> = std::fs::read_to_string(&ledger)
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
            assert_eq!(
                lines
                    .iter()
                    .map(|line| line["model"].as_str().unwrap())
                    .collect::<Vec<_>>(),
                ["a", "a", "b"]
            );
            assert_eq!(state(&root), ("b".to_owned(), 0, Vec::new()));
            let _ = std::fs::remove_dir_all(root);
        }
    }
}
