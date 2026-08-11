//! 在途 AI 呼叫中止基礎設施（AI 卡重構取消可中止在途呼叫＋app 退出孤兒子程序清理，包 2）。
//! 兩張全域表：
//! - 呼叫註冊表：依 world 分組的取消訊號，`refactor_abort` 對某桌 abort 時整組一次喚醒。
//! - 子程序 PID 表：`run_cli` 每次 spawn 都登記，app 退出（RunEvent::Exit）時整批 kill，
//!   避免 CLI 子程序變孤兒繼續跑、繼續燒錢。
//! 用 `OnceLock<Mutex<…>>` 而非掛在 tauri State：RunEvent::Exit callback 拿不到 command
//! 的 State 注入，兩處都要能存取就只能是自由 static。

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use tokio::sync::watch;

type WorldSenders = HashMap<u64, watch::Sender<bool>>;
type Registry = HashMap<String, WorldSenders>;

fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn children() -> &'static Mutex<HashSet<u32>> {
    static CHILDREN: OnceLock<Mutex<HashSet<u32>>> = OnceLock::new();
    CHILDREN.get_or_init(|| Mutex::new(HashSet::new()))
}

fn next_id() -> u64 {
    static NEXT: OnceLock<AtomicU64> = OnceLock::new();
    NEXT.get_or_init(|| AtomicU64::new(1))
        .fetch_add(1, Ordering::Relaxed)
}

/// 一次在途呼叫的註冊憑證。留在呼叫端 scope 內；drop 時（正常回傳、錯誤、或 select 輸掉
/// 分支被取消）一律從註冊表移除自己這筆，不會殘留。
pub struct CallGuard {
    world_id: String,
    id: u64,
}

impl Drop for CallGuard {
    fn drop(&mut self) {
        if let Ok(mut map) = registry().lock() {
            if let Some(senders) = map.get_mut(&self.world_id) {
                senders.remove(&self.id);
                if senders.is_empty() {
                    map.remove(&self.world_id);
                }
            }
        }
    }
}

/// select! 裡等的中止訊號；`abort_world` 送出後 `cancelled()` 立即返回。
pub struct CancelSignal {
    rx: watch::Receiver<bool>,
}

impl CancelSignal {
    pub async fn cancelled(&mut self) {
        let _ = self.rx.wait_for(|v| *v).await;
    }
}

/// 登記一次在途呼叫：回傳的 guard 留在呼叫端 scope（負責反登記），cancel 交給
/// `tokio::select!` 與實際呼叫賽跑。
pub fn register(world_id: &str) -> (CallGuard, CancelSignal) {
    let (tx, rx) = watch::channel(false);
    let id = next_id();
    registry()
        .lock()
        .unwrap()
        .entry(world_id.to_owned())
        .or_default()
        .insert(id, tx);
    (
        CallGuard {
            world_id: world_id.to_owned(),
            id,
        },
        CancelSignal { rx },
    )
}

/// 中止某桌全部在途呼叫：該 world 名下每個 sender 都 send_replace(true)。
/// sender 本身留著不清——CallGuard drop 時才移除，這裡只負責喚醒。
pub fn abort_world(world_id: &str) {
    if let Ok(map) = registry().lock() {
        if let Some(senders) = map.get(world_id) {
            for sender in senders.values() {
                sender.send_replace(true);
            }
        }
    }
}

/// 子程序 PID 登記：`run_cli` spawn 成功後呼叫，app 退出時 `kill_all_children` 靠這張表收屍。
pub fn register_child(pid: u32) {
    children().lock().unwrap().insert(pid);
}

pub fn unregister_child(pid: u32) {
    children().lock().unwrap().remove(&pid);
}

/// app 退出（RunEvent::Exit）呼叫：表上全部子程序逐一送 kill，避免孤兒繼續跑。殺完清空表。
pub fn kill_all_children() {
    let mut set = children().lock().unwrap();
    for pid in set.iter() {
        #[cfg(unix)]
        {
            let _ = std::process::Command::new("kill")
                .args(["-9", &pid.to_string()])
                .output();
        }
        #[cfg(windows)]
        {
            let _ = std::process::Command::new("taskkill")
                .args(["/F", "/PID", &pid.to_string()])
                .output();
        }
    }
    set.clear();
}

/// 測試專用：children 表是全域共用的 static，任何測試只要透過 `run_cli` spawn 真實子程序
/// 就會登記進同一張表，而 `kill_all_children` 不分青紅皂白殺表上全部 pid。凡是會這麼做的
/// 測試（本檔的 T1／T3、cli.rs 與 lanes.rs 既有的假 CLI 測試）都要靠這把鎖互斥執行，
/// 不然平行跑時彼此的子程序可能被對方誤殺。只序列化這幾個測試，不影響其他測試的平行度。
#[cfg(test)]
pub(crate) fn lock_real_process_tests() -> std::sync::MutexGuard<'static, ()> {
    use std::sync::Mutex;
    static REAL_PROCESS_TESTS: Mutex<()> = Mutex::new(());
    REAL_PROCESS_TESTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// 輪詢直到 `pid` 對 `kill -0` 不再有回應（程序真的死透，含被系統收屍），逾時回 false。
    #[cfg(unix)]
    async fn process_dead_within(pid: u32, budget: Duration) -> bool {
        let step = Duration::from_millis(20);
        let mut waited = Duration::ZERO;
        loop {
            let alive = std::process::Command::new("kill")
                .args(["-0", &pid.to_string()])
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false);
            if !alive {
                return true;
            }
            if waited >= budget {
                return false;
            }
            tokio::time::sleep(step).await;
            waited += step;
        }
    }

    /// T2 world 隔離：abort 一個 world 不該喚醒另一個 world 的 CancelSignal。
    #[tokio::test]
    async fn abort_world_only_signals_the_targeted_world() {
        let (_guard_a, mut cancel_a) = register("inflight-test-world-a");
        let (_guard_b, mut cancel_b) = register("inflight-test-world-b");

        abort_world("inflight-test-world-a");

        tokio::time::timeout(Duration::from_millis(500), cancel_a.cancelled())
            .await
            .expect("同一 world 的 CancelSignal 應該被 abort 喚醒");
        assert!(
            tokio::time::timeout(Duration::from_millis(200), cancel_b.cancelled())
                .await
                .is_err(),
            "另一 world 的 CancelSignal 不該被觸發"
        );
    }

    /// T4 guard 清理：CallGuard drop 後，該 world 的 sender map 要從註冊表整組消失。
    #[tokio::test]
    async fn dropping_call_guard_clears_its_world_from_registry() {
        let world_id = "inflight-test-world-guard";
        let (guard, _cancel) = register(world_id);
        assert!(registry().lock().unwrap().contains_key(world_id));

        drop(guard);

        assert!(!registry().lock().unwrap().contains_key(world_id));
    }

    /// T3 kill_all_children：手動登記一個真實子程序 pid，殺完程序真的死透、表清空。
    #[cfg(unix)]
    #[tokio::test]
    async fn kill_all_children_kills_process_and_clears_table() {
        let _serial = lock_real_process_tests();
        let mut child = std::process::Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .expect("spawn /bin/sleep");
        let pid = child.id();
        register_child(pid);

        kill_all_children();
        let _ = child.wait(); // 收屍，避免殭屍程序讓 kill -0 誤判仍存活

        assert!(
            process_dead_within(pid, Duration::from_secs(2)).await,
            "kill_all_children 後程序應在 2 秒內死透"
        );
        assert!(children().lock().unwrap().is_empty());
    }

    /// T1 中止殺程序：假 CLI（sleep 30）包進 select＋CancelSignal，abort_world 後
    /// select 要走中止分支，且子程序在 2 秒內真的死透、children 表不再留著這個 pid。
    #[cfg(unix)]
    #[tokio::test]
    async fn abort_world_kills_inflight_cli_child_via_select() {
        let _serial = lock_real_process_tests();
        let world_id = "inflight-test-world-abort-child";
        // children 表是全域共用的，其他測試（lanes.rs／cli.rs 的假 CLI 測試）也會有短暫在途
        // 子程序；先拍照，之後只認「快照裡沒有的新 pid」，不然可能撿到別人的 pid，
        // 提早對一個還沒 register() 的 world 呼叫 abort_world（訊號送不到，白等 30 秒）。
        let before: HashSet<u32> = children().lock().unwrap().clone();

        let handle = tokio::spawn(async move {
            let (_guard, mut cancel) = register(world_id);
            let program = std::path::PathBuf::from("/bin/sleep");
            let working_dir = std::env::temp_dir();
            let args = ["30".to_owned()];
            tokio::select! {
                biased;
                _ = cancel.cancelled() => Err("aborted".to_owned()),
                result = crate::cli::run_cli(
                    &program,
                    &working_dir,
                    &args,
                    "",
                    &[],
                    crate::cli::parse_claude_line,
                    false,
                    None,
                    |_delta: &str| {},
                ) => result.map_err(|error| error.to_string()),
            }
        });

        // 等 run_cli spawn 完成並登記 pid（不早於此就 abort，否則測到的是「還沒開始」）。
        let step = Duration::from_millis(10);
        let mut waited = Duration::ZERO;
        let pid = loop {
            if let Some(pid) = children()
                .lock()
                .unwrap()
                .iter()
                .find(|pid| !before.contains(pid))
                .copied()
            {
                break pid;
            }
            assert!(waited < Duration::from_secs(2), "等子程序 pid 登記逾時");
            tokio::time::sleep(step).await;
            waited += step;
        };

        abort_world(world_id);

        let outcome = handle.await.expect("背景 task 不該 panic");
        assert_eq!(outcome, Err("aborted".to_owned()));
        assert!(
            process_dead_within(pid, Duration::from_secs(2)).await,
            "abort 後子程序應在 2 秒內死透"
        );
        assert!(!children().lock().unwrap().contains(&pid));
    }
}
