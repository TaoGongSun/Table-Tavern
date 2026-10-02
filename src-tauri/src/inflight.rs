//! 在途 AI 呼叫中止基礎設施（AI 卡重構取消可中止在途呼叫＋app 退出孤兒子程序清理，包 2；
//! 對話停止鍵把鑰匙拆成「種類＋桌」）。
//! 兩張全域表：
//! - 呼叫註冊表：鑰匙是 `(Kind, world_id)`。重構仍整組喚醒；對話再以 `turn_id` 只打那一輪。
//! - 子程序 PID 表：`run_cli` 每次 spawn 都登記，app 退出（RunEvent::Exit）時整批 kill，
//!   避免 CLI 子程序變孤兒繼續跑、繼續燒錢。
//!
//! 用 `OnceLock<Mutex<…>>` 而非掛在 tauri State：RunEvent::Exit callback 拿不到 command
//! 的 State 注入，兩處都要能存取就只能是自由 static。

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use tokio::sync::watch;

/// 對話與重構各用自己的鑰匙，同桌互不波及。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Chat,
    Refactor,
}

struct Registration {
    sender: watch::Sender<bool>,
    /// 對話輪才有；重構整組中止，不看這個。
    turn_id: Option<String>,
}

type SlotSenders = HashMap<u64, Registration>;
type RegistryKey = (Kind, String);
type Registry = HashMap<RegistryKey, SlotSenders>;

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
    kind: Kind,
    world_id: String,
    id: u64,
}

impl Drop for CallGuard {
    fn drop(&mut self) {
        if let Ok(mut map) = registry().lock() {
            let key = (self.kind, self.world_id.clone());
            if let Some(senders) = map.get_mut(&key) {
                senders.remove(&self.id);
                if senders.is_empty() {
                    map.remove(&key);
                }
            }
        }
    }
}

/// select! 裡等的中止訊號；`abort_kind`／`abort_turn` 送出後 `cancelled()` 立即返回。
pub struct CancelSignal {
    rx: watch::Receiver<bool>,
}

impl CancelSignal {
    pub async fn cancelled(&mut self) {
        let _ = self.rx.wait_for(|v| *v).await;
    }

    /// 複製目前的接收端。watch 保留最新值，複製之後才送出的中止一樣看得到。
    pub fn receiver(&self) -> watch::Receiver<bool> {
        self.rx.clone()
    }
}

fn insert(kind: Kind, world_id: &str, turn_id: Option<String>) -> (CallGuard, CancelSignal) {
    let (tx, rx) = watch::channel(false);
    let id = next_id();
    registry()
        .lock()
        .unwrap()
        .entry((kind, world_id.to_owned()))
        .or_default()
        .insert(
            id,
            Registration {
                sender: tx,
                turn_id,
            },
        );
    (
        CallGuard {
            kind,
            world_id: world_id.to_owned(),
            id,
        },
        CancelSignal { rx },
    )
}

/// 登記一次在途呼叫：回傳的 guard 留在呼叫端 scope（負責反登記）。
/// 重構走這支，不帶 turn；`abort_kind` 會把該種類該桌整組喚醒。
pub fn register(kind: Kind, world_id: &str) -> (CallGuard, CancelSignal) {
    insert(kind, world_id, None)
}

/// 登記一輪對話。`abort_turn` 只喚醒 turn_id 相同的那一筆，晚到的舊 id 打不中下一輪。
pub fn register_turn(world_id: &str, turn_id: &str) -> (CallGuard, CancelSignal) {
    insert(Kind::Chat, world_id, Some(turn_id.to_owned()))
}

/// 中止某種類、某桌的全部在途呼叫。sender 留著不清——CallGuard drop 時才移除。
pub fn abort_kind(kind: Kind, world_id: &str) {
    if let Ok(map) = registry().lock() {
        if let Some(senders) = map.get(&(kind, world_id.to_owned())) {
            for registration in senders.values() {
                registration.sender.send_replace(true);
            }
        }
    }
}

/// 只中止該輪對話。對不上的 turn_id（含已經結束、註冊已摘掉的）什麼都不做。
pub fn abort_turn(world_id: &str, turn_id: &str) {
    if let Ok(map) = registry().lock() {
        if let Some(senders) = map.get(&(Kind::Chat, world_id.to_owned())) {
            for registration in senders.values() {
                if registration.turn_id.as_deref() == Some(turn_id) {
                    registration.sender.send_replace(true);
                }
            }
        }
    }
}

/// 測試看子程序表。正式路徑不要靠這張快照做決策。
#[cfg(test)]
pub(crate) fn child_pids() -> HashSet<u32> {
    children().lock().unwrap().clone()
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
#[allow(clippy::await_holding_lock)]
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
        let (_guard_a, mut cancel_a) = register(Kind::Refactor, "inflight-test-world-a");
        let (_guard_b, mut cancel_b) = register(Kind::Refactor, "inflight-test-world-b");

        abort_kind(Kind::Refactor, "inflight-test-world-a");

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
        let (guard, _cancel) = register(Kind::Refactor, world_id);
        let key = (Kind::Refactor, world_id.to_owned());
        assert!(registry().lock().unwrap().contains_key(&key));

        drop(guard);

        assert!(!registry().lock().unwrap().contains_key(&key));
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
            let (_guard, mut cancel) = register(Kind::Refactor, world_id);
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

        abort_kind(Kind::Refactor, world_id);

        let outcome = handle.await.expect("背景 task 不該 panic");
        assert_eq!(outcome, Err("aborted".to_owned()));
        assert!(
            process_dead_within(pid, Duration::from_secs(2)).await,
            "abort 後子程序應在 2 秒內死透"
        );
        assert!(!children().lock().unwrap().contains(&pid));
    }

    /// 同桌的對話與重構鑰匙分開：中止一邊不會喚醒另一邊。
    #[tokio::test]
    async fn chat_and_refactor_on_the_same_world_do_not_cross() {
        let world_id = "inflight-test-kind-split";
        let (_chat_guard, mut chat) = register_turn(world_id, "turn-a");
        let (_refactor_guard, mut refactor) = register(Kind::Refactor, world_id);

        abort_kind(Kind::Refactor, world_id);
        tokio::time::timeout(Duration::from_millis(500), refactor.cancelled())
            .await
            .expect("重構中止要喚醒重構呼叫");
        assert!(
            tokio::time::timeout(Duration::from_millis(200), chat.cancelled())
                .await
                .is_err(),
            "重構中止不該打到同桌的對話"
        );

        abort_turn(world_id, "turn-a");
        tokio::time::timeout(Duration::from_millis(500), chat.cancelled())
            .await
            .expect("對話中止要喚醒對上的那一輪");
    }

    /// 舊 turn_id 的中止打不中已經換上的新呼叫；兩輪同時在冊時也只打指定的那筆。
    #[tokio::test]
    async fn aborting_an_old_turn_id_leaves_the_new_call_alone() {
        let world_id = "inflight-test-old-turn";
        let (old_guard, mut old_cancel) = register_turn(world_id, "old");
        let (_new_guard, mut new_cancel) = register_turn(world_id, "new");

        abort_turn(world_id, "old");
        tokio::time::timeout(Duration::from_millis(500), old_cancel.cancelled())
            .await
            .expect("舊 turn 應該被自己的 id 喚醒");
        assert!(
            tokio::time::timeout(Duration::from_millis(200), new_cancel.cancelled())
                .await
                .is_err(),
            "舊 turn_id 不該中止新呼叫"
        );

        drop(old_guard);
        abort_turn(world_id, "old");
        assert!(
            tokio::time::timeout(Duration::from_millis(200), new_cancel.cancelled())
                .await
                .is_err(),
            "舊呼叫已經摘掉之後，晚到的中止仍不該打中新呼叫"
        );
    }
}
