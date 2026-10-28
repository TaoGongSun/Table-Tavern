//! grok 在途程序閘（plans/agy-home-cleanup.md）。`sessions/session_search.sqlite` 在兩次 grok 啟動之間
//! 留著抹寫前的內容，要刪；但不能在有 grok 開著它時刪（SQLite 官方 How To Corrupt）。
//! 以 GROK_HOME 為鍵記 app 行程內的在途程序數：每個 grok spawn 前拿一張票，程序真的收屍後才交回。
//! 交回時先刪 prompt 歷史（單純追加的檔，隨時可刪）；在途數歸零就持著閘刪整組索引，
//! 刪除期間新的 spawn 要等閘，所以不會有 grok 在刪的當下啟動。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Default)]
struct Gate {
    running: usize,
    /// 刪除失敗時留著，下次歸零再刪；成功才清
    pending: bool,
    /// 等在途歸零才刪的 session（單發被中止時，程序可能還沒收屍）
    deferred: Vec<String>,
}

fn gate_for(home: &Path) -> Arc<Mutex<Gate>> {
    static GATES: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<Gate>>>>> = OnceLock::new();
    let mut table = GATES
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    table.entry(home.to_path_buf()).or_default().clone()
}

fn lock(gate: &Mutex<Gate>) -> std::sync::MutexGuard<'_, Gate> {
    gate.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 一個在途 grok 程序的票。drop＝程序已收屍（或根本沒 spawn 成功）。
pub(crate) struct Ticket {
    home: PathBuf,
    gate: Arc<Mutex<Gate>>,
}

impl Ticket {
    /// 確認不了程序已退出（沒有 runtime、收屍失敗、收屍任務被中止）：票不交回、在途數不歸零，
    /// 這個行程不再清索引與延後的 session，交給下次 app 啟動。寧可晚刪，不可誤刪。
    pub(crate) fn leak(self) {
        log::warn!("grok 程序無法確認已退出，本行程不再清搜尋索引");
        std::mem::forget(self);
    }
}

/// 單發 session 的善後：沒有 grok 在跑就立刻刪；還有（例如被中止的程序還沒收屍）就等在途歸零。
pub(crate) fn remove_session_when_idle(home: &Path, session_id: &str) {
    let gate = gate_for(home);
    let mut gate = lock(&gate);
    if gate.running == 0 {
        remove_deferred(home, session_id);
    } else {
        gate.deferred.push(session_id.to_owned());
    }
}

fn remove_deferred(home: &Path, session_id: &str) {
    if let Err(error) = crate::cli::home_cleanup::remove_session(home, session_id) {
        log::warn!("grok 單發 session {session_id} 刪除失敗：{error}");
    }
}

/// spawn 前呼叫；刪除進行中會等它做完。
pub(crate) fn enter(home: &Path) -> Ticket {
    let gate = gate_for(home);
    lock(&gate).running += 1;
    Ticket {
        home: home.to_path_buf(),
        gate,
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        if let Err(error) = crate::cli::home_cleanup::remove_prompt_history(&self.home) {
            log::warn!("grok prompt 歷史刪除失敗：{error}");
        }
        let mut gate = lock(&self.gate);
        gate.running = gate.running.saturating_sub(1);
        gate.pending = true;
        if gate.running == 0 {
            for session_id in std::mem::take(&mut gate.deferred) {
                remove_deferred(&self.home, &session_id);
            }
            if gate.pending {
                clear_locked(&self.home, &mut gate);
            }
        }
    }
}

fn clear_locked(home: &Path, gate: &mut Gate) {
    match crate::cli::home_cleanup::remove_search_index(home) {
        Ok(()) => gate.pending = false,
        Err(error) => log::warn!("grok 搜尋索引刪除失敗，下次再試：{error}"),
    }
}

/// app 啟動回收用：沒有 app spawn 的 grok 在跑時刪整組索引（補崩潰或刪除失敗的殘留）。
pub(crate) fn clear_if_idle(home: &Path) {
    let gate = gate_for(home);
    let mut gate = lock(&gate);
    if gate.running == 0 {
        clear_locked(home, &mut gate);
    }
}

#[cfg(test)]
pub(crate) fn running(home: &Path) -> usize {
    lock(&gate_for(home)).running
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home(tag: &str) -> PathBuf {
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce).unwrap();
        let dir = std::env::temp_dir().join(format!(
            "tt-grok-gate-{tag}-{}",
            nonce.iter().map(|b| format!("{b:02x}")).collect::<String>()
        ));
        let group = dir.join("sessions").join("%2Fws");
        std::fs::create_dir_all(&group).unwrap();
        dir
    }

    fn seed(home: &Path) {
        let sessions = home.join("sessions");
        for name in [
            "session_search.sqlite",
            "session_search.sqlite-wal",
            "session_search.sqlite-shm",
            "session_search.sqlite-journal",
            "session_search.sqlite.corrupt.123",
            "session_search.sqlite-wal.corrupt.123",
            "session_search.sqlite.lock",
        ] {
            std::fs::write(sessions.join(name), "x").unwrap();
        }
        std::fs::write(sessions.join("%2Fws/prompt_history.jsonl"), "secret").unwrap();
    }

    fn index_left(home: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(home.join("sessions"))
            .unwrap()
            .flatten()
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| name.starts_with("session_search"))
            .collect();
        names.sort();
        names
    }

    #[test]
    fn last_exit_clears_index_and_keeps_lock_file() {
        let home = home("last");
        seed(&home);
        let a = enter(&home);
        let b = enter(&home);
        drop(a);
        // A 結束、B 還開著：索引不動，prompt 歷史照刪
        assert_eq!(index_left(&home).len(), 7);
        assert!(!home.join("sessions/%2Fws/prompt_history.jsonl").exists());
        drop(b);
        assert_eq!(index_left(&home), vec!["session_search.sqlite.lock"]);
        assert_eq!(running(&home), 0);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn enter_waits_while_clearing() {
        let home = home("wait");
        seed(&home);
        let gate = gate_for(&home);
        let held = lock(&gate); // 模擬刪除進行中
        let home2 = home.clone();
        let started = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = started.clone();
        let handle = std::thread::spawn(move || {
            let ticket = enter(&home2);
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
            ticket
        });
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(!started.load(std::sync::atomic::Ordering::SeqCst));
        drop(held);
        let ticket = handle.join().unwrap();
        assert!(started.load(std::sync::atomic::Ordering::SeqCst));
        drop(ticket);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn failed_clear_stays_pending_until_next_idle() {
        let home = home("retry");
        seed(&home);
        // 刪不掉：把 sessions 目錄收成唯讀模擬（unix）
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let sessions = home.join("sessions");
            std::fs::set_permissions(&sessions, std::fs::Permissions::from_mode(0o500)).unwrap();
            drop(enter(&home));
            assert!(lock(&gate_for(&home)).pending);
            std::fs::set_permissions(&sessions, std::fs::Permissions::from_mode(0o700)).unwrap();
            drop(enter(&home));
            assert!(!lock(&gate_for(&home)).pending);
            assert_eq!(index_left(&home), vec!["session_search.sqlite.lock"]);
        }
        std::fs::remove_dir_all(&home).unwrap();
    }

    /// 被中止的單發：程序還在途時 session 不刪，最後一個交回票時才刪；沒人在跑就立刻刪。
    #[test]
    fn deferred_session_waits_for_idle() {
        let home = home("deferred");
        let a = "aaaaaaaa-0000-4000-8000-000000000001";
        let b = "bbbbbbbb-0000-4000-8000-000000000002";
        for id in [a, b] {
            std::fs::create_dir_all(home.join("sessions/%2Fws").join(id)).unwrap();
        }
        let ticket = enter(&home);
        remove_session_when_idle(&home, a);
        assert!(home.join("sessions/%2Fws").join(a).exists());
        drop(ticket);
        assert!(!home.join("sessions/%2Fws").join(a).exists());
        remove_session_when_idle(&home, b);
        assert!(!home.join("sessions/%2Fws").join(b).exists());
        std::fs::remove_dir_all(&home).unwrap();
    }

    /// 確認不了退出的票：在途數不歸零、索引不刪。
    #[test]
    fn leaked_ticket_keeps_gate_busy() {
        let home = home("leak");
        seed(&home);
        enter(&home).leak();
        drop(enter(&home));
        assert_eq!(running(&home), 1);
        assert_eq!(index_left(&home).len(), 7);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn idle_sweep_skips_while_running() {
        let home = home("idle");
        seed(&home);
        let ticket = enter(&home);
        clear_if_idle(&home);
        assert_eq!(index_left(&home).len(), 7);
        drop(ticket);
        seed(&home);
        clear_if_idle(&home);
        assert_eq!(index_left(&home), vec!["session_search.sqlite.lock"]);
        std::fs::remove_dir_all(&home).unwrap();
    }
}
