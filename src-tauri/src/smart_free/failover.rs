//! 穩定免費的目前模型、失敗分類與換模（計畫 .ai/plans/stable-free-failover.md §2.2、§4）。
//!
//! 狀態以 root 為鍵放在程序內，記憶體與 `smart_free_current.json` 同一份：提交時先把新 revision
//! 寫進待寫版本、落盤成功才整份替換。鎖內不 await；跨 await 的派送與試打靠「票」核對世代。

use super::select::lineup_gen;
use super::store::{self, CurrentState, ProbeOutcome, ProbeRecord};
use crate::transport::{ApiFailure, FailureStage};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// 連續幾次可計數失敗就換模〔作者裁決 2026-10-04〕。
pub const FAILURES_TO_SWITCH: u32 = 2;
/// exhausted 冷卻：自最近一次有模型被加進 exhausted 起算〔作者裁決 2026-10-04〕。
pub const EXHAUSTED_COOLDOWN_SECS: u64 = 30 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureClass {
    /// 上游擁擠／不可用、連線失敗、逾時。
    Model,
    /// 404：模型下架，直接視為已達換模次數。
    Gone,
    /// 沒有平台證據的 429：走有限 failover，保留原錯誤，不宣稱是上游限流〔作者裁決 2026-10-04〕。
    UnknownRateLimit,
    /// 401／402／403、平台限流、每日次數用完：換模救不了。
    Account,
    /// 內容過濾、拒答、空回應、不完整、其他 400。
    Other,
}

impl FailureClass {
    pub fn counts(self) -> bool {
        matches!(self, Self::Model | Self::Gone | Self::UnknownRateLimit)
    }
}

/// 無平台 headers 的 429 才查 `/key`；查詢結果只用來排除「每日用完」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCheck {
    NotQueried,
    Failed,
    Remaining(i64),
}

fn is_rate_limited(failure: &ApiFailure) -> bool {
    failure.code() == Some(429)
}

fn platform_evidence(failure: &ApiFailure) -> bool {
    if failure
        .rate_limit
        .as_ref()
        .is_some_and(|limit| limit.remaining_is_zero())
    {
        return true;
    }
    [&failure.detail.message, &failure.display]
        .iter()
        .any(|text| {
            let text = text.to_ascii_lowercase();
            text.contains("free-models-per-day") || text.contains("free-models-per-min")
        })
}

pub fn needs_key_check(failure: &ApiFailure) -> bool {
    is_rate_limited(failure) && !platform_evidence(failure)
}

pub fn classify(failure: &ApiFailure, key: KeyCheck) -> FailureClass {
    let code = failure.code();
    if matches!(code, Some(401..=403)) {
        return FailureClass::Account;
    }
    if is_rate_limited(failure) {
        let daily_gone = matches!(key, KeyCheck::Remaining(remaining) if remaining <= 0);
        return if platform_evidence(failure) || daily_gone {
            FailureClass::Account
        } else {
            FailureClass::UnknownRateLimit
        };
    }
    // HTTP 狀態或串流錯誤塊的 code 都算（SSE 中途的 404 也是下架）
    if code == Some(404) {
        return FailureClass::Gone;
    }
    if matches!(failure.stage, FailureStage::Network | FailureStage::Timeout)
        || matches!(code, Some(408 | 502 | 503 | 504))
        || matches!(
            failure.detail.error_type.as_deref(),
            Some("provider_overloaded" | "provider_unavailable" | "timeout")
        )
    {
        return FailureClass::Model;
    }
    FailureClass::Other
}

/// 本句要用哪支（§4.5）：目前模型放得下就用它；否則依名單、再依名單外穩定候選找放得下的當本句替代。
/// `exclude` 用於重送：排除第 1 發的模型與 exhausted。回傳 (模型, 是否本句替代)。
pub fn pick_for_sentence(
    current: &str,
    lineup: &[String],
    others: &[String],
    fits: impl Fn(&str) -> bool,
    exclude: &[String],
) -> Option<(String, bool)> {
    let allowed =
        |model: &str| !model.is_empty() && !exclude.iter().any(|ex| ex == model) && fits(model);
    if allowed(current) {
        return Some((current.to_owned(), false));
    }
    lineup
        .iter()
        .chain(others)
        .find(|model| model.as_str() != current && allowed(model))
        .map(|model| (model.clone(), true))
}

/// 派送或試打開始前在鎖內取的票；回來時核對，晚到的結果不得污染新狀態。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ticket {
    pub account: String,
    pub epoch: u64,
    pub revision: u64,
    pub lineup_gen: String,
    pub current_model: String,
    pub selected_model: String,
    pub substitute: bool,
    /// false＝這張票的結果一律不計：重建落盤失敗、或送出用的設定已不是磁碟上的最新設定。
    pub countable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatRecord {
    /// 票對不上、替代模型、或不可計數的失敗：不動計數。
    Ignored,
    Success,
    Counted,
    Switched {
        from: String,
        to: String,
    },
    /// 名單全在 exhausted：exhausted 已落盤，不換、不重送。
    AllBusy,
    /// 落盤失敗：記憶體不變、不提示、不重送。
    CommitFailed,
}

pub type Persist<'a> = &'a mut dyn FnMut(&CurrentState) -> std::io::Result<()>;

#[derive(Debug, Default)]
pub struct Runtime {
    pub saved: CurrentState,
    loaded: bool,
    /// 選模世代：程序內單調遞增、不重用。目前模型、模式、金鑰、base_url、名單任何一項變動都 +1。
    pub epoch: u64,
    /// 目前模型的連續可計數失敗次數；綁 epoch，世代一變就歸零。
    pub count: u32,
    config_key: Option<String>,
}

impl Runtime {
    /// 推進選模世代；計數綁世代，一律同步歸零。
    fn advance_epoch(&mut self) {
        self.epoch += 1;
        self.count = 0;
    }

    /// 作廢所有在途的票（例如名單刷新成空）：只推進世代，不改磁碟狀態。
    pub fn invalidate(&mut self) {
        self.advance_epoch();
    }

    /// 寫進待寫版本 → 落盤 → 成功才整份替換。磁碟與記憶體的 revision 一律相同。
    pub fn commit(&mut self, mut pending: CurrentState, persist: Persist) -> bool {
        pending.revision = self.saved.revision + 1;
        if persist(&pending).is_err() {
            return false;
        }
        let new_generation = pending.model != self.saved.model
            || pending.account != self.saved.account
            || pending.lineup_gen != self.saved.lineup_gen;
        self.saved = pending;
        if new_generation {
            self.advance_epoch();
        }
        true
    }

    /// 模式／金鑰指紋／base_url 組成的鍵；變了就推進 epoch（stable→recommended→stable 會推進兩次），
    /// 計數同步歸零。
    pub fn observe_config(&mut self, key: &str) {
        if self.config_key.as_deref() != Some(key) {
            if self.config_key.is_some() {
                self.advance_epoch();
            }
            self.config_key = Some(key.to_owned());
        }
    }

    /// 取票前對齊帳號與名單、處理冷卻；回傳本句的目前模型。重建落盤失敗回 None：記憶體不變但世代推進
    /// （先前發出的票全部作廢），呼叫端只能發不可計數的票。
    pub fn reconcile(
        &mut self,
        account: &str,
        lineup: &[String],
        now: u64,
        persist: Persist,
    ) -> Option<String> {
        let gen = lineup_gen(lineup);
        if self.saved.account != account
            || self.saved.lineup_gen != gen
            || !lineup.contains(&self.saved.model)
        {
            let mut pending = if self.saved.account == account {
                CurrentState {
                    probe: self.saved.probe.clone(),
                    ..CurrentState::default()
                }
            } else {
                CurrentState::default()
            };
            pending.account = account.to_owned();
            pending.lineup_gen = gen;
            pending.model = lineup.first().cloned().unwrap_or_default();
            let model = pending.model.clone();
            if self.commit(pending, persist) {
                // 帳號或名單沒變、只是目前模型掉出名單時 commit 也會推進世代；這裡再保險歸零
                self.count = 0;
                return Some(model);
            }
            self.advance_epoch();
            return None;
        }
        if self
            .saved
            .exhausted_at
            .is_some_and(|at| now >= at.saturating_add(EXHAUSTED_COOLDOWN_SECS))
        {
            let mut pending = self.saved.clone();
            pending.exhausted.clear();
            pending.exhausted_at = None;
            if self.commit(pending, persist) {
                self.count = 0;
            }
        }
        Some(self.saved.model.clone())
    }

    pub fn ticket(&self, current: &str, selected: &str, substitute: bool) -> Ticket {
        Ticket {
            account: self.saved.account.clone(),
            epoch: self.epoch,
            revision: self.saved.revision,
            lineup_gen: self.saved.lineup_gen.clone(),
            current_model: current.to_owned(),
            selected_model: selected.to_owned(),
            substitute,
            countable: true,
        }
    }

    /// 結果一律不計的票（重建落盤失敗、送出設定不是最新）。
    pub fn uncountable_ticket(&self, selected: &str) -> Ticket {
        Ticket {
            countable: false,
            ..self.ticket(selected, selected, true)
        }
    }

    fn ticket_matches(&self, ticket: &Ticket) -> bool {
        ticket.account == self.saved.account
            && ticket.epoch == self.epoch
            && ticket.lineup_gen == self.saved.lineup_gen
            && ticket.current_model == self.saved.model
    }

    /// 聊天結果（`None`＝成功）。只有票對得上、非替代的結果才計入；`allow_switch` 為 false 時（第 2 發）
    /// 只記次數不換模。
    pub fn record_chat(
        &mut self,
        ticket: &Ticket,
        failure: Option<FailureClass>,
        allow_switch: bool,
        lineup: &[String],
        now: u64,
        persist: Persist,
    ) -> ChatRecord {
        if !ticket.countable || !self.ticket_matches(ticket) || ticket.substitute {
            return ChatRecord::Ignored;
        }
        let class = match failure {
            None => {
                self.count = 0;
                if !self.saved.exhausted.is_empty() {
                    let mut pending = self.saved.clone();
                    pending.exhausted.clear();
                    pending.exhausted_at = None;
                    self.commit(pending, persist);
                }
                return ChatRecord::Success;
            }
            Some(class) if !class.counts() => return ChatRecord::Ignored,
            Some(class) => class,
        };
        self.count = if class == FailureClass::Gone {
            self.count.max(FAILURES_TO_SWITCH)
        } else {
            self.count + 1
        };
        if !allow_switch || self.count < FAILURES_TO_SWITCH {
            return ChatRecord::Counted;
        }
        self.switch(lineup, now, persist)
    }

    fn switch(&mut self, lineup: &[String], now: u64, persist: Persist) -> ChatRecord {
        let from = self.saved.model.clone();
        let mut pending = self.saved.clone();
        if !pending.exhausted.contains(&from) {
            pending.exhausted.push(from.clone());
        }
        pending.exhausted_at = Some(now);
        let start = lineup.iter().position(|model| model == &from).unwrap_or(0);
        let target = (1..=lineup.len())
            .map(|offset| &lineup[(start + offset) % lineup.len()])
            .find(|model| !pending.exhausted.contains(model))
            .cloned();
        let record = match &target {
            Some(to) => {
                pending.model = to.clone();
                ChatRecord::Switched {
                    from,
                    to: to.clone(),
                }
            }
            None => ChatRecord::AllBusy,
        };
        if !self.commit(pending, persist) {
            return ChatRecord::CommitFailed;
        }
        self.count = 0;
        record
    }

    /// 試打一輪的提交（§5）：只在同帳號時，從鎖內最新狀態合併 `probe` 兩欄；帳號已換就整筆丟棄。
    /// 選模欄位只在 confirmed 且 epoch、revision 都沒變時才寫。
    pub fn commit_probe(
        &mut self,
        ticket: &ProbeTicket,
        outcome: &ProbeOutcome,
        gen: &str,
        now: u64,
        persist: Persist,
    ) -> bool {
        if self.saved.account != ticket.account {
            return false;
        }
        let mut pending = self.saved.clone();
        pending.probe = ProbeRecord {
            last_run_at: now,
            lineup_gen: gen.to_owned(),
            outcome: outcome.clone(),
        };
        let selects = self.epoch == ticket.epoch && self.saved.revision == ticket.revision;
        let confirmed = match outcome {
            ProbeOutcome::Confirmed { model } if selects => Some(model),
            _ => None,
        };
        if let Some(model) = confirmed {
            pending.model = model.clone();
            pending.confirmed_at = Some(now);
            pending.exhausted.clear();
            pending.exhausted_at = None;
        }
        let committed = self.commit(pending, persist);
        if committed && confirmed.is_some() {
            self.count = 0;
        }
        committed
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeTicket {
    pub account: String,
    pub epoch: u64,
    pub revision: u64,
}

static RUNTIMES: Mutex<BTreeMap<PathBuf, Runtime>> = Mutex::new(BTreeMap::new());

/// 鎖內操作同一 root 的狀態；第一次用到時從磁碟載入。
pub fn with_runtime<R>(root: &Path, operate: impl FnOnce(&mut Runtime) -> R) -> R {
    let mut runtimes = RUNTIMES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime = runtimes.entry(root.to_owned()).or_default();
    if !runtime.loaded {
        runtime.saved = store::read_current(root);
        runtime.loaded = true;
    }
    operate(runtime)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{ErrorDetail, RateLimit};

    fn http(status: u16) -> ApiFailure {
        ApiFailure {
            status: Some(status),
            display: format!("AI_HTTP_STATUS_{status}: status={status} body="),
            ..ApiFailure::default()
        }
    }

    fn ok_persist() -> impl FnMut(&CurrentState) -> std::io::Result<()> {
        |_: &CurrentState| Ok(())
    }

    fn lineup(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| (*id).to_owned()).collect()
    }

    #[test]
    fn classification_table() {
        use FailureClass::*;
        let mut platform = http(429);
        platform.rate_limit = Some(RateLimit {
            remaining: Some("0".to_owned()),
            ..RateLimit::default()
        });
        assert_eq!(classify(&platform, KeyCheck::NotQueried), Account);
        assert!(!needs_key_check(&platform));

        let mut per_day = http(429);
        per_day.detail.message = "Rate limit exceeded: free-models-per-day".to_owned();
        assert_eq!(classify(&per_day, KeyCheck::NotQueried), Account);

        // provider_name 只是線索：沒有平台證據仍歸 UnknownRateLimit
        let mut hinted = http(429);
        hinted.detail.provider_name = Some("Google AI Studio".to_owned());
        hinted.detail.has_raw = true;
        assert!(needs_key_check(&hinted));
        assert_eq!(classify(&hinted, KeyCheck::Remaining(30)), UnknownRateLimit);
        assert_eq!(classify(&hinted, KeyCheck::Remaining(0)), Account);
        assert_eq!(classify(&hinted, KeyCheck::Failed), UnknownRateLimit);
        assert_eq!(classify(&http(429), KeyCheck::Failed), UnknownRateLimit);

        // 串流中途的 429：沒有 headers，看 SSE code
        let sse_429 = ApiFailure::stream(
            "Rate limit exceeded".to_owned(),
            Some(ErrorDetail {
                code: Some(429),
                ..ErrorDetail::default()
            }),
            false,
        );
        assert_eq!(classify(&sse_429, KeyCheck::Remaining(5)), UnknownRateLimit);
        let overloaded = ApiFailure::stream(
            "overloaded".to_owned(),
            Some(ErrorDetail {
                error_type: Some("provider_overloaded".to_owned()),
                ..ErrorDetail::default()
            }),
            true,
        );
        assert_eq!(classify(&overloaded, KeyCheck::NotQueried), Model);

        for status in [401, 402, 403] {
            assert_eq!(classify(&http(status), KeyCheck::NotQueried), Account);
        }
        for status in [408, 502, 503, 504] {
            assert_eq!(classify(&http(status), KeyCheck::NotQueried), Model);
        }
        assert_eq!(classify(&http(404), KeyCheck::NotQueried), Gone);
        // 串流中途的 404（SSE error.code）也是下架
        let sse_404 = ApiFailure::stream(
            "model not found".to_owned(),
            Some(ErrorDetail {
                code: Some(404),
                ..ErrorDetail::default()
            }),
            false,
        );
        assert_eq!(classify(&sse_404, KeyCheck::NotQueried), Gone);
        assert_eq!(classify(&http(400), KeyCheck::NotQueried), Other);
        let timeout = ApiFailure {
            stage: FailureStage::Timeout,
            ..ApiFailure::default()
        };
        assert_eq!(classify(&timeout, KeyCheck::NotQueried), Model);
        let network = ApiFailure {
            stage: FailureStage::Network,
            ..ApiFailure::default()
        };
        assert_eq!(classify(&network, KeyCheck::NotQueried), Model);
        for display in [
            "AI_CONTENT_FILTERED: model=x",
            "AI_EMPTY_RESPONSE: model=x",
            "AI_INCOMPLETE_RESPONSE: model=x",
        ] {
            let failure = ApiFailure::stream(display.to_owned(), None, false);
            assert_eq!(classify(&failure, KeyCheck::NotQueried), Other);
        }
    }

    #[test]
    fn sentence_pick_prefers_current_then_lineup_then_outside_long_context() {
        let names = lineup(&["a", "b", "c"]);
        let others = lineup(&["long"]);
        assert_eq!(
            pick_for_sentence("a", &names, &others, |_| true, &[]),
            Some(("a".to_owned(), false))
        );
        assert_eq!(
            pick_for_sentence("a", &names, &others, |m| m == "c", &[]),
            Some(("c".to_owned(), true))
        );
        assert_eq!(
            pick_for_sentence("a", &names, &others, |m| m == "long", &[]),
            Some(("long".to_owned(), true))
        );
        assert_eq!(
            pick_for_sentence("a", &names, &others, |_| false, &[]),
            None
        );
        // 重送：A 連敗換到 B，B 放不下 → 不挑回 A、也不挑 exhausted
        assert_eq!(
            pick_for_sentence("b", &names, &others, |m| m != "b", &lineup(&["a"])),
            Some(("c".to_owned(), true))
        );
        assert_eq!(
            pick_for_sentence("b", &names, &[], |m| m == "a", &lineup(&["a"])),
            None
        );
    }

    fn fresh(names: &[String]) -> Runtime {
        let mut runtime = Runtime::default();
        runtime.observe_config("stable|acct|official");
        runtime.reconcile("acct", names, 1_000, &mut ok_persist());
        runtime
    }

    #[test]
    fn two_counted_failures_switch_and_other_results_in_between_are_ignored() {
        let names = lineup(&["a", "b", "c", "d"]);
        let mut runtime = fresh(&names);
        assert_eq!(runtime.saved.model, "a");
        let ticket = runtime.ticket("a", "a", false);
        let mut persist = ok_persist();
        assert_eq!(
            runtime.record_chat(
                &ticket,
                Some(FailureClass::Model),
                true,
                &names,
                1_000,
                &mut persist
            ),
            ChatRecord::Counted
        );
        // Other／Account 不歸零也不加：Model → Other → Account → UnknownRateLimit 仍算連續 2 次
        for class in [FailureClass::Other, FailureClass::Account] {
            assert_eq!(
                runtime.record_chat(&ticket, Some(class), true, &names, 1_000, &mut persist),
                ChatRecord::Ignored
            );
        }
        assert_eq!(
            runtime.record_chat(
                &ticket,
                Some(FailureClass::UnknownRateLimit),
                true,
                &names,
                1_000,
                &mut persist
            ),
            ChatRecord::Switched {
                from: "a".to_owned(),
                to: "b".to_owned()
            }
        );
        assert_eq!(runtime.saved.model, "b");
        assert_eq!(runtime.saved.exhausted, ["a"]);
        assert_eq!(runtime.saved.exhausted_at, Some(1_000));
        assert_eq!(runtime.count, 0);
        // 舊票（epoch 已推進）晚回：不計
        assert_eq!(
            runtime.record_chat(&ticket, None, true, &names, 1_000, &mut persist),
            ChatRecord::Ignored
        );
    }

    #[test]
    fn success_resets_and_clears_exhausted_but_substitute_success_does_not() {
        let names = lineup(&["a", "b"]);
        let mut runtime = fresh(&names);
        let mut persist = ok_persist();
        let ticket = runtime.ticket("a", "a", false);
        runtime.record_chat(
            &ticket,
            Some(FailureClass::Model),
            true,
            &names,
            1,
            &mut persist,
        );
        runtime.record_chat(
            &ticket,
            Some(FailureClass::Model),
            true,
            &names,
            1,
            &mut persist,
        );
        assert_eq!(runtime.saved.model, "b");
        let on_b = runtime.ticket("b", "b", false);
        runtime.record_chat(
            &on_b,
            Some(FailureClass::Model),
            true,
            &names,
            2,
            &mut persist,
        );
        assert_eq!(runtime.count, 1);
        // 本句替代成功：不歸零目前模型的計數、不清 exhausted
        let substitute = runtime.ticket("b", "long", true);
        assert_eq!(
            runtime.record_chat(&substitute, None, true, &names, 2, &mut persist),
            ChatRecord::Ignored
        );
        assert_eq!(
            runtime.record_chat(
                &substitute,
                Some(FailureClass::Model),
                true,
                &names,
                2,
                &mut persist
            ),
            ChatRecord::Ignored
        );
        assert_eq!(runtime.count, 1);
        assert_eq!(
            runtime.record_chat(&on_b, None, true, &names, 3, &mut persist),
            ChatRecord::Success
        );
        assert_eq!(runtime.count, 0);
        assert!(runtime.saved.exhausted.is_empty());
        assert_eq!(runtime.saved.exhausted_at, None);
    }

    #[test]
    fn gone_switches_immediately_and_second_dispatch_never_switches() {
        let names = lineup(&["a", "b", "c"]);
        let mut runtime = fresh(&names);
        let mut persist = ok_persist();
        let ticket = runtime.ticket("a", "a", false);
        assert!(matches!(
            runtime.record_chat(
                &ticket,
                Some(FailureClass::Gone),
                true,
                &names,
                1,
                &mut persist
            ),
            ChatRecord::Switched { .. }
        ));
        let retry = runtime.ticket("b", "b", false);
        assert_eq!(
            runtime.record_chat(
                &retry,
                Some(FailureClass::Gone),
                false,
                &names,
                1,
                &mut persist
            ),
            ChatRecord::Counted
        );
        assert_eq!(runtime.saved.model, "b");
    }

    #[test]
    fn no_target_persists_exhausted_resets_count_and_reports_all_busy() {
        let names = lineup(&["a", "b"]);
        let mut runtime = fresh(&names);
        let writes = std::cell::RefCell::new(Vec::new());
        let mut persist = |state: &CurrentState| {
            writes.borrow_mut().push(state.clone());
            Ok(())
        };
        for model in ["a", "b"] {
            let ticket = runtime.ticket(model, model, false);
            runtime.record_chat(
                &ticket,
                Some(FailureClass::Model),
                true,
                &names,
                10,
                &mut persist,
            );
            runtime.record_chat(
                &ticket,
                Some(FailureClass::Model),
                true,
                &names,
                20,
                &mut persist,
            );
        }
        // a → b 換過一次；b 再連敗兩次時名單全在 exhausted
        let last = writes.borrow().last().unwrap().clone();
        assert_eq!(runtime.saved.model, "b");
        assert_eq!(last.exhausted, ["a", "b"]);
        assert_eq!(last.exhausted_at, Some(20));
        assert_eq!(runtime.saved, last);
        assert_eq!(runtime.count, 0);
        let ticket = runtime.ticket("b", "b", false);
        runtime.record_chat(
            &ticket,
            Some(FailureClass::Model),
            true,
            &names,
            30,
            &mut persist,
        );
        assert_eq!(
            runtime.record_chat(
                &ticket,
                Some(FailureClass::Model),
                true,
                &names,
                30,
                &mut persist
            ),
            ChatRecord::AllBusy
        );
    }

    #[test]
    fn cooldown_counts_from_latest_exhaustion_and_resets_count() {
        let names = lineup(&["a", "b"]);
        let mut runtime = fresh(&names);
        let mut persist = ok_persist();
        let ticket = runtime.ticket("a", "a", false);
        runtime.record_chat(
            &ticket,
            Some(FailureClass::Model),
            true,
            &names,
            100,
            &mut persist,
        );
        runtime.record_chat(
            &ticket,
            Some(FailureClass::Model),
            true,
            &names,
            100,
            &mut persist,
        );
        let on_b = runtime.ticket("b", "b", false);
        runtime.record_chat(
            &on_b,
            Some(FailureClass::Model),
            true,
            &names,
            500,
            &mut persist,
        );
        assert_eq!(runtime.count, 1);
        // 起算點是加進 exhausted 的時間（100），之後的計數不延後冷卻；未滿 30 分鐘不清
        assert_eq!(runtime.saved.exhausted_at, Some(100));
        runtime.reconcile(
            "acct",
            &names,
            100 + EXHAUSTED_COOLDOWN_SECS - 1,
            &mut persist,
        );
        assert_eq!(runtime.saved.exhausted, ["a"]);
        assert_eq!(runtime.count, 1);
        runtime.reconcile("acct", &names, 100 + EXHAUSTED_COOLDOWN_SECS, &mut persist);
        assert!(runtime.saved.exhausted.is_empty());
        assert_eq!(runtime.saved.exhausted_at, None);
        assert_eq!(runtime.count, 0);
        // 冷卻不改目前模型
        assert_eq!(runtime.saved.model, "b");
    }

    #[test]
    fn commit_failure_leaves_memory_untouched_and_disk_revision_matches_memory() {
        let names = lineup(&["a", "b"]);
        let mut runtime = fresh(&names);
        let before = runtime.saved.clone();
        let epoch = runtime.epoch;
        let ticket = runtime.ticket("a", "a", false);
        let mut failing = |_: &CurrentState| Err(std::io::Error::other("disk full"));
        runtime.record_chat(
            &ticket,
            Some(FailureClass::Model),
            true,
            &names,
            1,
            &mut failing,
        );
        assert_eq!(
            runtime.record_chat(
                &ticket,
                Some(FailureClass::Model),
                true,
                &names,
                1,
                &mut failing
            ),
            ChatRecord::CommitFailed
        );
        assert_eq!(runtime.saved, before);
        assert_eq!(runtime.epoch, epoch);
        // 計數留在記憶體：下一次落盤成功就換
        let mut written = None;
        let mut persist = |state: &CurrentState| {
            written = Some(state.clone());
            Ok(())
        };
        assert!(matches!(
            runtime.record_chat(
                &ticket,
                Some(FailureClass::Model),
                true,
                &names,
                1,
                &mut persist
            ),
            ChatRecord::Switched { .. }
        ));
        let written = written.unwrap();
        assert_eq!(written.revision, before.revision + 1);
        assert_eq!(runtime.saved, written);
    }

    #[test]
    fn aba_model_round_trip_rejects_the_stale_ticket() {
        let names = lineup(&["a", "b"]);
        let mut runtime = fresh(&names);
        let mut persist = ok_persist();
        let stale = runtime.ticket("a", "a", false);
        // a → b → a（兩次換模），舊 a 票的晚回結果不得計入新的 a
        for model in ["a", "b"] {
            let ticket = runtime.ticket(model, model, false);
            runtime.record_chat(
                &ticket,
                Some(FailureClass::Model),
                true,
                &names,
                1,
                &mut persist,
            );
            runtime.record_chat(
                &ticket,
                Some(FailureClass::Model),
                true,
                &names,
                1,
                &mut persist,
            );
        }
        // 第二次換模時 a 已在 exhausted → AllBusy；手動把它清掉模擬冷卻後回到 a
        let mut pending = runtime.saved.clone();
        pending.exhausted.clear();
        pending.model = "a".to_owned();
        assert!(runtime.commit(pending, &mut persist));
        assert_eq!(runtime.saved.model, "a");
        assert_eq!(
            runtime.record_chat(
                &stale,
                Some(FailureClass::Model),
                true,
                &names,
                2,
                &mut persist
            ),
            ChatRecord::Ignored
        );
        assert_eq!(runtime.count, 0);
    }

    #[test]
    fn config_round_trip_advances_epoch_twice() {
        let names = lineup(&["a"]);
        let mut runtime = fresh(&names);
        let stale = runtime.ticket("a", "a", false);
        let epoch = runtime.epoch;
        runtime.observe_config("recommended|acct|official");
        runtime.observe_config("stable|acct|official");
        assert_eq!(runtime.epoch, epoch + 2);
        runtime.observe_config("stable|acct|official");
        assert_eq!(runtime.epoch, epoch + 2);
        assert_eq!(
            runtime.record_chat(
                &stale,
                Some(FailureClass::Model),
                true,
                &names,
                1,
                &mut ok_persist()
            ),
            ChatRecord::Ignored
        );
    }

    #[test]
    fn account_or_lineup_change_rebuilds_from_the_top() {
        let names = lineup(&["a", "b"]);
        let mut runtime = fresh(&names);
        let mut persist = ok_persist();
        let ticket = runtime.ticket("a", "a", false);
        runtime.record_chat(
            &ticket,
            Some(FailureClass::Model),
            true,
            &names,
            1,
            &mut persist,
        );
        runtime.record_chat(
            &ticket,
            Some(FailureClass::Model),
            true,
            &names,
            1,
            &mut persist,
        );
        assert_eq!(runtime.saved.model, "b");
        let reordered = lineup(&["c", "b"]);
        assert_eq!(
            runtime
                .reconcile("acct", &reordered, 2, &mut persist)
                .as_deref(),
            Some("c")
        );
        assert!(runtime.saved.exhausted.is_empty());
        assert_eq!(
            runtime
                .reconcile("other", &reordered, 2, &mut persist)
                .as_deref(),
            Some("c")
        );
        assert_eq!(runtime.saved.account, "other");
        // 重建寫檔失敗：回 None、記憶體不變，但世代推進讓先前的票作廢
        let before = runtime.ticket("c", "c", false);
        let mut failing = |_: &CurrentState| Err(std::io::Error::other("no"));
        assert_eq!(runtime.reconcile("third", &names, 3, &mut failing), None);
        assert_eq!(runtime.saved.account, "other");
        assert_eq!(
            runtime.record_chat(
                &before,
                Some(FailureClass::Model),
                true,
                &reordered,
                3,
                &mut persist
            ),
            ChatRecord::Ignored
        );
    }

    #[test]
    fn failed_rebuild_issues_only_uncountable_tickets_even_when_models_coincide() {
        // 舊帳號與新帳號的第 1 名同為 a：重建落盤失敗後發出的票不得把新帳號的失敗記到舊帳號
        let names = lineup(&["a", "b"]);
        let mut runtime = fresh(&names);
        let mut failing = |_: &CurrentState| Err(std::io::Error::other("no"));
        assert_eq!(runtime.reconcile("new", &names, 1, &mut failing), None);
        let ticket = runtime.uncountable_ticket("a");
        let mut persist = ok_persist();
        for _ in 0..3 {
            assert_eq!(
                runtime.record_chat(
                    &ticket,
                    Some(FailureClass::Model),
                    true,
                    &names,
                    1,
                    &mut persist
                ),
                ChatRecord::Ignored
            );
        }
        assert_eq!(runtime.count, 0);
        assert_eq!(runtime.saved.account, "acct");
        assert_eq!(runtime.saved.model, "a");
    }

    #[test]
    fn mode_round_trip_resets_the_failure_count() {
        // A 失敗一次 → 切 recommended → 切回 stable → 新世代第一次失敗不能直接換模
        let names = lineup(&["a", "b"]);
        let mut runtime = fresh(&names);
        let mut persist = ok_persist();
        let ticket = runtime.ticket("a", "a", false);
        runtime.record_chat(
            &ticket,
            Some(FailureClass::Model),
            true,
            &names,
            1,
            &mut persist,
        );
        assert_eq!(runtime.count, 1);
        runtime.observe_config("recommended|acct|official");
        runtime.observe_config("stable|acct|official");
        assert_eq!(runtime.count, 0);
        let fresh_ticket = runtime.ticket("a", "a", false);
        assert_eq!(
            runtime.record_chat(
                &fresh_ticket,
                Some(FailureClass::Model),
                true,
                &names,
                2,
                &mut persist
            ),
            ChatRecord::Counted
        );
        assert_eq!(runtime.saved.model, "a");
    }
}
