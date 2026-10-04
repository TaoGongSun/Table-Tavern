//! 試打（計畫 §5）：從名單第 1 名往下逐支送最小請求，第一支成功就停。會佔玩家每日免費次數（作者接受），
//! 所以有節流、低額度不打、帳號層級錯誤停整輪、每發前核對帳號與選模世代。

use super::call::PersistSend;
use super::failover::{self, FailureClass, KeyCheck, ProbeTicket};
use super::select::lineup_gen;
use super::store::{CurrentState, ProbeOutcome};
use crate::transport::ApiFailure;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// 剩餘每日次數不超過這個就不試打，次數留給對話。
pub const LOW_QUOTA: i64 = 5;
const STARTUP_INTERVAL_SECS: u64 = 3600;
const CLIMB_INTERVAL_SECS: u64 = 3 * 3600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// App 啟動、OAuth 完成、設定變更後的 warm。
    Startup,
    /// 背景每小時檢查。
    Background,
}

/// 這次要試哪些模型；None＝節流中不打。冷卻清空不屬於觸發條件。
pub fn probe_targets(
    saved: &CurrentState,
    account: &str,
    lineup: &[String],
    trigger: Trigger,
    now: u64,
) -> Option<Vec<String>> {
    if lineup.is_empty() {
        return None;
    }
    let record = (saved.account == account).then_some(&saved.probe);
    let never = record.is_none_or(|record| record.last_run_at == 0);
    let gen_changed = record.is_none_or(|record| record.lineup_gen != lineup_gen(lineup));
    let since = record.map_or(u64::MAX, |record| now.saturating_sub(record.last_run_at));
    match trigger {
        Trigger::Startup => {
            (never || gen_changed || since >= STARTUP_INTERVAL_SECS).then(|| lineup.to_vec())
        }
        Trigger::Background => {
            if never || gen_changed {
                return Some(lineup.to_vec());
            }
            // 目前不是第 1 名：只試它前面那幾支，爭取回到第 1 名
            let position = lineup.iter().position(|model| model == &saved.model)?;
            (position > 0 && since >= CLIMB_INTERVAL_SECS).then(|| lineup[..position].to_vec())
        }
    }
}

pub(crate) trait ProbeEnv {
    async fn daily_remaining(&mut self) -> Option<i64>;
    async fn probe(&mut self, model: &str) -> Result<(), ApiFailure>;
    /// 重讀設定：仍是穩定免費、金鑰仍是這個帳號就回設定鍵，否則 None。
    /// 權威資料（重讀磁碟設定與快取，不連網）：仍是穩定免費且有金鑰才回 (設定鍵, 帳號, 名單)。
    fn authority(&mut self) -> Option<Authority>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authority {
    pub config_key: String,
    pub account: String,
    pub lineup: Vec<String>,
}

/// 同一個 root 同時只跑一輪試打。
static PROBE_LOCKS: std::sync::Mutex<BTreeMap<PathBuf, Arc<tokio::sync::Mutex<()>>>> =
    std::sync::Mutex::new(BTreeMap::new());

fn probe_lock(root: &Path) -> Arc<tokio::sync::Mutex<()>> {
    PROBE_LOCKS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .entry(root.to_owned())
        .or_default()
        .clone()
}

/// 跑一輪試打；回傳本輪結果（節流中、設定已變則 None）。只要打過一發就提交 probe 紀錄。
pub(crate) async fn run_probe(
    root: &Path,
    trigger: Trigger,
    env: &mut impl ProbeEnv,
    persist: PersistSend<'_>,
    now: u64,
) -> Option<ProbeOutcome> {
    let lock = probe_lock(root);
    let _guard = lock.lock().await;
    // 拿到鎖之後才讀權威設定與名單、判節流：排隊期間名單或設定變了也不會拿舊資料重建，
    // 排隊的觸發也看得到前一輪剛寫的紀錄，不會串行重複扣次數。
    let (ticket, targets, lineup) = failover::with_runtime(root, |runtime| {
        let authority = env.authority()?;
        runtime.observe_config(&authority.config_key);
        // 重建落盤失敗就不試打（狀態對不齊，試打結果也無處可記）
        runtime.reconcile(&authority.account, &authority.lineup, now, persist)?;
        let targets = probe_targets(
            &runtime.saved,
            &authority.account,
            &authority.lineup,
            trigger,
            now,
        )?;
        Some((
            ProbeTicket {
                account: authority.account,
                epoch: runtime.epoch,
                revision: runtime.saved.revision,
            },
            targets,
            authority.lineup,
        ))
    })?;
    if env
        .daily_remaining()
        .await
        .is_some_and(|remaining| remaining <= LOW_QUOTA)
    {
        return Some(ProbeOutcome::Stopped {
            reason: "low_quota".to_owned(),
        });
    }

    let mut fired = 0;
    let mut saw_other = false;
    let mut outcome = None;
    for model in targets {
        let still_valid = failover::with_runtime(root, |runtime| {
            env.authority().is_some_and(|authority| {
                runtime.observe_config(&authority.config_key);
                authority.account == ticket.account
                    && runtime.epoch == ticket.epoch
                    && runtime.saved.account == ticket.account
            })
        });
        if !still_valid {
            outcome = Some(ProbeOutcome::Stopped {
                reason: "stale".to_owned(),
            });
            break;
        }
        fired += 1;
        let failure = match env.probe(&model).await {
            Ok(()) => {
                outcome = Some(ProbeOutcome::Confirmed { model });
                break;
            }
            Err(failure) => failure,
        };
        let key_check = if failover::needs_key_check(&failure) {
            match env.daily_remaining().await {
                Some(remaining) => KeyCheck::Remaining(remaining),
                None => KeyCheck::Failed,
            }
        } else {
            KeyCheck::NotQueried
        };
        match failover::classify(&failure, key_check) {
            FailureClass::Account => {
                outcome = Some(ProbeOutcome::Stopped {
                    reason: "account".to_owned(),
                });
                break;
            }
            FailureClass::Other => saw_other = true,
            _ => {}
        }
    }
    let outcome = outcome.unwrap_or(if saw_other {
        ProbeOutcome::Inconclusive
    } else {
        ProbeOutcome::AllFailed
    });
    if fired > 0 {
        let gen = lineup_gen(&lineup);
        failover::with_runtime(root, |runtime| {
            runtime.commit_probe(&ticket, &outcome, &gen, now, persist)
        });
    }
    Some(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    #[derive(Clone, Default)]
    struct Shared {
        /// None＝設定已不是穩定免費／換了帳號
        key: Arc<Mutex<Option<String>>>,
        /// 權威名單與帳號（probe_with 第一次呼叫時填入）
        lineup: Arc<Mutex<Option<Vec<String>>>>,
        account: Arc<Mutex<Option<String>>>,
    }

    impl Shared {
        fn set_lineup(&self, lineup: Vec<String>) {
            *self.lineup.lock().unwrap() = Some(lineup);
        }
    }

    struct FakeProbe {
        replies: BTreeMap<String, VecDeque<Result<(), ApiFailure>>>,
        fired: Vec<String>,
        daily: Option<i64>,
        shared: Shared,
        /// 打完第 n 發後把設定改掉（模擬玩家中途切 recommended）
        switch_mode_after: Option<usize>,
        /// 打第一發時先等這個信號（模擬受控延遲）
        gate: Option<Arc<tokio::sync::Notify>>,
    }

    impl FakeProbe {
        fn new(replies: &[(&str, Result<(), ApiFailure>)]) -> Self {
            let mut map: BTreeMap<String, VecDeque<Result<(), ApiFailure>>> = BTreeMap::new();
            for (model, reply) in replies {
                map.entry((*model).to_owned())
                    .or_default()
                    .push_back(reply.clone());
            }
            let shared = Shared::default();
            *shared.key.lock().unwrap() = Some("stable|acct|official".to_owned());
            Self {
                replies: map,
                fired: Vec::new(),
                daily: Some(40),
                shared,
                switch_mode_after: None,
                gate: None,
            }
        }
    }

    impl ProbeEnv for FakeProbe {
        async fn daily_remaining(&mut self) -> Option<i64> {
            self.daily
        }
        async fn probe(&mut self, model: &str) -> Result<(), ApiFailure> {
            self.fired.push(model.to_owned());
            if let Some(gate) = self.gate.take() {
                gate.notified().await;
            }
            if self.switch_mode_after == Some(self.fired.len()) {
                *self.shared.key.lock().unwrap() = None;
            }
            self.replies
                .get_mut(model)
                .and_then(VecDeque::pop_front)
                .unwrap_or(Ok(()))
        }
        fn authority(&mut self) -> Option<Authority> {
            Some(Authority {
                config_key: self.shared.key.lock().unwrap().clone()?,
                account: self.shared.account.lock().unwrap().clone()?,
                lineup: self.shared.lineup.lock().unwrap().clone()?,
            })
        }
    }

    fn http(status: u16) -> Result<(), ApiFailure> {
        Err(ApiFailure {
            status: Some(status),
            display: format!("AI_HTTP_STATUS_{status}: status={status} body="),
            ..ApiFailure::default()
        })
    }

    /// 測試用：第一次呼叫時把帳號與名單填成權威資料，之後由測試自己改 Shared 模擬變動。
    fn probe_with<'a>(
        root: &'a Path,
        account: &str,
        lineup: &[String],
        trigger: Trigger,
        env: &'a mut FakeProbe,
        persist: PersistSend<'a>,
        now: u64,
    ) -> impl std::future::Future<Output = Option<ProbeOutcome>> + 'a {
        env.shared
            .account
            .lock()
            .unwrap()
            .get_or_insert_with(|| account.to_owned());
        env.shared
            .lineup
            .lock()
            .unwrap()
            .get_or_insert_with(|| lineup.to_vec());
        run_probe(root, trigger, env, persist, now)
    }

    fn names(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| (*id).to_owned()).collect()
    }

    fn ok_persist() -> impl FnMut(&CurrentState) -> std::io::Result<()> + Send {
        |_: &CurrentState| Ok(())
    }

    fn saved(root: &Path) -> CurrentState {
        failover::with_runtime(root, |runtime| runtime.saved.clone())
    }

    #[tokio::test]
    async fn first_success_stops_and_becomes_current() {
        let root = super::super::test_root("probe-first");
        let lineup = names(&["a", "b", "c", "d"]);
        let mut env = FakeProbe::new(&[("a", http(429)), ("b", Ok(()))]);
        let outcome = probe_with(
            &root,
            "acct",
            &lineup,
            Trigger::Startup,
            &mut env,
            &mut ok_persist(),
            100,
        )
        .await;
        assert_eq!(
            outcome,
            Some(ProbeOutcome::Confirmed {
                model: "b".to_owned()
            })
        );
        assert_eq!(env.fired, ["a", "b"]);
        let state = saved(&root);
        assert_eq!(state.model, "b");
        assert_eq!(state.confirmed_at, Some(100));
        assert_eq!(state.probe.last_run_at, 100);
        // 1 小時內再啟動不重打；背景迴圈也不打（名單沒變、還不到 3 小時）
        let mut env = FakeProbe::new(&[]);
        for trigger in [Trigger::Startup, Trigger::Background] {
            assert_eq!(
                probe_with(
                    &root,
                    "acct",
                    &lineup,
                    trigger,
                    &mut env,
                    &mut ok_persist(),
                    3_000
                )
                .await,
                None
            );
        }
        assert!(env.fired.is_empty());
        // 3 小時後背景只試目前模型前面那幾支
        let mut env = FakeProbe::new(&[("a", http(503))]);
        let outcome = probe_with(
            &root,
            "acct",
            &lineup,
            Trigger::Background,
            &mut env,
            &mut ok_persist(),
            100 + 3 * 3600,
        )
        .await;
        assert_eq!(env.fired, ["a"]);
        assert_eq!(outcome, Some(ProbeOutcome::AllFailed));
        assert_eq!(saved(&root).model, "b");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn account_error_stops_the_round_and_other_makes_it_inconclusive() {
        let root = super::super::test_root("probe-stop");
        let lineup = names(&["a", "b", "c"]);
        let mut env = FakeProbe::new(&[("a", http(503)), ("b", http(402))]);
        let outcome = probe_with(
            &root,
            "acct",
            &lineup,
            Trigger::Startup,
            &mut env,
            &mut ok_persist(),
            10,
        )
        .await;
        assert_eq!(
            outcome,
            Some(ProbeOutcome::Stopped {
                reason: "account".to_owned()
            })
        );
        assert_eq!(env.fired, ["a", "b"]);
        assert_eq!(saved(&root).confirmed_at, None);

        let root2 = super::super::test_root("probe-other");
        let mut env = FakeProbe::new(&[("a", http(503)), ("b", http(400)), ("c", http(503))]);
        let outcome = probe_with(
            &root2,
            "acct",
            &lineup,
            Trigger::Startup,
            &mut env,
            &mut ok_persist(),
            10,
        )
        .await;
        assert_eq!(outcome, Some(ProbeOutcome::Inconclusive));
        let state = saved(&root2);
        assert_eq!(state.model, "a");
        assert_eq!(state.confirmed_at, None);
        assert_eq!(state.probe.outcome, ProbeOutcome::Inconclusive);
        // all_failed／inconclusive 也記 last_run_at：背景不每小時重打
        let mut env = FakeProbe::new(&[]);
        assert_eq!(
            probe_with(
                &root2,
                "acct",
                &lineup,
                Trigger::Background,
                &mut env,
                &mut ok_persist(),
                3_610
            )
            .await,
            None
        );
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(root2);
    }

    #[tokio::test]
    async fn low_quota_skips_without_recording() {
        let root = super::super::test_root("probe-low");
        let lineup = names(&["a"]);
        let mut env = FakeProbe::new(&[]);
        env.daily = Some(LOW_QUOTA);
        let outcome = probe_with(
            &root,
            "acct",
            &lineup,
            Trigger::Startup,
            &mut env,
            &mut ok_persist(),
            10,
        )
        .await;
        assert_eq!(
            outcome,
            Some(ProbeOutcome::Stopped {
                reason: "low_quota".to_owned()
            })
        );
        assert!(env.fired.is_empty());
        assert_eq!(saved(&root).probe.last_run_at, 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn mode_switch_mid_round_stops_before_the_next_shot() {
        let root = super::super::test_root("probe-mode");
        let lineup = names(&["a", "b", "c"]);
        let mut env = FakeProbe::new(&[("a", http(503)), ("b", http(503))]);
        env.switch_mode_after = Some(1);
        let outcome = probe_with(
            &root,
            "acct",
            &lineup,
            Trigger::Startup,
            &mut env,
            &mut ok_persist(),
            10,
        )
        .await;
        assert_eq!(
            outcome,
            Some(ProbeOutcome::Stopped {
                reason: "stale".to_owned()
            })
        );
        assert_eq!(env.fired, ["a"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn late_probe_after_chat_failover_keeps_its_record_but_not_its_pick() {
        let root = super::super::test_root("probe-late");
        let lineup = names(&["a", "b", "c"]);
        let gate = Arc::new(tokio::sync::Notify::new());
        let mut env = FakeProbe::new(&[("a", Ok(()))]);
        env.gate = Some(gate.clone());
        let mut persist = ok_persist();
        let probe = probe_with(
            &root,
            "acct",
            &lineup,
            Trigger::Startup,
            &mut env,
            &mut persist,
            10,
        );
        tokio::pin!(probe);
        // 試打卡在第一發時，聊天把目前模型從 a 換到 b
        assert!(futures_util::poll!(probe.as_mut()).is_pending());
        failover::with_runtime(&root, |runtime| {
            let ticket = runtime.ticket("a", "a", false);
            let mut persist = ok_persist();
            for _ in 0..2 {
                runtime.record_chat(
                    &ticket,
                    Some(FailureClass::Model),
                    true,
                    &lineup,
                    11,
                    &mut persist,
                );
            }
        });
        gate.notify_one();
        let outcome = probe.await;
        assert_eq!(
            outcome,
            Some(ProbeOutcome::Confirmed {
                model: "a".to_owned()
            })
        );
        let state = saved(&root);
        assert_eq!(state.model, "b");
        assert_eq!(state.exhausted, ["a"]);
        assert_eq!(state.confirmed_at, None);
        assert_eq!(state.probe.last_run_at, 10);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn old_account_probe_returning_late_never_touches_the_new_account() {
        let root = super::super::test_root("probe-account");
        let lineup = names(&["a", "b"]);
        let gate = Arc::new(tokio::sync::Notify::new());
        let mut old_env = FakeProbe::new(&[("a", http(503)), ("b", Ok(()))]);
        old_env.gate = Some(gate.clone());
        let old_shared = old_env.shared.clone();
        let mut old_persist = ok_persist();
        let old = probe_with(
            &root,
            "old",
            &lineup,
            Trigger::Startup,
            &mut old_env,
            &mut old_persist,
            10,
        );
        tokio::pin!(old);
        assert!(futures_util::poll!(old.as_mut()).is_pending());
        // 換 key：新帳號的對話把狀態重建成新帳號（試打鎖被舊輪佔住，新帳號的試打要等）
        *old_shared.key.lock().unwrap() = Some("stable|new|official".to_owned());
        failover::with_runtime(&root, |runtime| {
            runtime.observe_config("stable|new|official");
            runtime.reconcile("new", &lineup, 20, &mut ok_persist());
        });
        gate.notify_one();
        let outcome = old.await;
        // 舊輪第 2 發前發現世代變了就停；它的紀錄不寫進新帳號
        assert_eq!(
            outcome,
            Some(ProbeOutcome::Stopped {
                reason: "stale".to_owned()
            })
        );
        let state = saved(&root);
        assert_eq!(state.account, "new");
        assert_eq!(state.probe.last_run_at, 0);
        // 新帳號試打完成後，舊帳號的票就算晚回也不能合併
        let mut new_env = FakeProbe::new(&[("a", Ok(()))]);
        *new_env.shared.key.lock().unwrap() = Some("stable|new|official".to_owned());
        probe_with(
            &root,
            "new",
            &lineup,
            Trigger::Startup,
            &mut new_env,
            &mut ok_persist(),
            30,
        )
        .await;
        let after_new = saved(&root);
        assert_eq!(after_new.probe.last_run_at, 30);
        let committed = failover::with_runtime(&root, |runtime| {
            runtime.commit_probe(
                &ProbeTicket {
                    account: "old".to_owned(),
                    epoch: 0,
                    revision: 0,
                },
                &ProbeOutcome::Confirmed {
                    model: "b".to_owned(),
                },
                "gen",
                40,
                &mut ok_persist(),
            )
        });
        assert!(!committed);
        assert_eq!(saved(&root), after_new);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn queued_triggers_recheck_throttle_after_taking_the_lock() {
        let root = super::super::test_root("probe-queue");
        let lineup = names(&["a"]);
        let mut first = FakeProbe::new(&[("a", Ok(()))]);
        let mut second = FakeProbe::new(&[("a", Ok(()))]);
        let (mut persist_one, mut persist_two) = (ok_persist(), ok_persist());
        let (one, two) = tokio::join!(
            probe_with(
                &root,
                "acct",
                &lineup,
                Trigger::Startup,
                &mut first,
                &mut persist_one,
                10
            ),
            probe_with(
                &root,
                "acct",
                &lineup,
                Trigger::Startup,
                &mut second,
                &mut persist_two,
                10
            ),
        );
        assert_eq!(first.fired.len() + second.fired.len(), 1);
        assert!(one.is_none() || two.is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn queued_probe_reads_the_lineup_after_taking_the_lock() {
        let root = super::super::test_root("probe-queue-lineup");
        let old_lineup = names(&["a", "b"]);
        let new_lineup = names(&["c", "a"]);
        let gate = Arc::new(tokio::sync::Notify::new());
        let mut holder = FakeProbe::new(&[("a", Ok(()))]);
        holder.gate = Some(gate.clone());
        let mut queued = FakeProbe::new(&[("c", Ok(()))]);
        let (holder_shared, queued_shared) = (holder.shared.clone(), queued.shared.clone());
        let (mut persist_one, mut persist_two) = (ok_persist(), ok_persist());
        let first = probe_with(
            &root,
            "acct",
            &old_lineup,
            Trigger::Startup,
            &mut holder,
            &mut persist_one,
            10,
        );
        tokio::pin!(first);
        assert!(futures_util::poll!(first.as_mut()).is_pending());
        // 第二輪排隊等鎖（呼叫時帶的是舊名單）
        let second = probe_with(
            &root,
            "acct",
            &old_lineup,
            Trigger::Startup,
            &mut queued,
            &mut persist_two,
            10,
        );
        tokio::pin!(second);
        assert!(futures_util::poll!(second.as_mut()).is_pending());
        // 等待期間同帳號刷新名單
        failover::with_runtime(&root, |runtime| {
            runtime.reconcile("acct", &new_lineup, 11, &mut ok_persist());
        });
        queued_shared.set_lineup(new_lineup.clone());
        holder_shared.set_lineup(new_lineup.clone());
        gate.notify_one();
        let _ = first.await;
        let _ = second.await;
        let state = saved(&root);
        assert_eq!(state.lineup_gen, lineup_gen(&new_lineup));
        assert!(new_lineup.contains(&state.model));
        let _ = std::fs::remove_dir_all(&root);
    }
}
