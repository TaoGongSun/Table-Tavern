//! 更新擋板與依 CLI 的在途表（plans/cli-version-gate.md 包 2 第 3 點「互斥與在途」）。
//! 一般呼叫與更新共用同一把鎖：一般呼叫在鎖內先看擋板、再寫一筆佔位（pid 還沒有），出鎖後 spawn、
//! 成功就回填 pid；佔位由 `Slot` 持有，Drop 時移除，future 被取消或 panic 都不留永久佔位。
//! 更新端在同一把鎖內清掉已死的 pid，還有佔位或活的 pid 就回 Busy，沒有才立起擋板。
//! 不走世界鎖：換的是全 app 共用的執行檔。

use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::path::Path;
use std::sync::Mutex;

/// 這支程式是不是四家 CLI 之一（看檔名，Windows 去掉 `.exe`）；安裝腳本的 bash／sh／powershell 不算。
pub(crate) fn cli_of(program: &OsStr) -> Option<&'static str> {
    let stem = Path::new(program).file_stem()?.to_str()?;
    ["claude", "codex", "agy", "grok"]
        .into_iter()
        .find(|cli| stem.eq_ignore_ascii_case(cli))
}

#[derive(Default)]
struct State {
    barriers: HashSet<&'static str>,
    /// 程序停不下來而留著的擋板（重新啟動 app 前不撤）
    stuck: HashSet<&'static str>,
    /// 佔位 id → (CLI, pid)；pid 是 None＝已取得資格、child 還沒登記完成
    inflight: HashMap<u64, (&'static str, Option<u32>)>,
    next: u64,
}

type Table = &'static Mutex<State>;

#[cfg(not(test))]
fn table() -> Table {
    static STATE: std::sync::OnceLock<Mutex<State>> = std::sync::OnceLock::new();
    STATE.get_or_init(Default::default)
}

/// 測試：每條測試執行緒一張表（tokio 測試預設單執行緒），會立擋板的測試不會擋到同時在跑、
/// spawn 同名假 CLI 的其他測試。擋板相關測試必須用 current_thread（`#[tokio::test]` 預設）：
/// 多執行緒 runtime 下立擋板與 spawn 可能落在不同執行緒、看到不同張表，測試會假綠或假紅。
#[cfg(test)]
fn table() -> Table {
    thread_local! {
        static STATE: Table = Box::leak(Box::default());
    }
    STATE.with(|table| *table)
}

fn lock_table(table: Table) -> std::sync::MutexGuard<'static, State> {
    // 鎖中毒（某次持鎖時 panic）也照樣用：表的內容仍一致，只是那次操作沒做完
    table
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn lock() -> std::sync::MutexGuard<'static, State> {
    lock_table(table())
}

/// 一般呼叫的在途佔位；Drop 時移除。
pub(crate) struct Slot {
    id: u64,
    table: Table,
}

impl Slot {
    pub(crate) fn set_pid(&self, pid: Option<u32>) {
        if let Some(entry) = lock_table(self.table).inflight.get_mut(&self.id) {
            entry.1 = pid;
        }
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        lock_table(self.table).inflight.remove(&self.id);
    }
}

/// 擋板立著回 Err（呼叫端轉成 `CliUpdating`，不排隊）；否則寫一筆佔位。
pub(crate) fn enter(cli: &'static str) -> Result<Slot, ()> {
    let mut state = lock();
    if state.barriers.contains(cli) {
        return Err(());
    }
    state.next += 1;
    let id = state.next;
    state.inflight.insert(id, (cli, None));
    Ok(Slot { id, table: table() })
}

/// 立著的擋板；Drop 時撤掉。
pub(crate) struct Barrier {
    cli: &'static str,
    table: Table,
}

impl Drop for Barrier {
    fn drop(&mut self) {
        lock_table(self.table).barriers.remove(self.cli);
    }
}

pub(crate) enum Raise {
    Raised(Barrier),
    /// 還有活的在途呼叫（或剛取得資格、還沒登記 pid 的）：等目前這輪結束
    Busy,
    /// 已經有一個更新或啟動復原在跑
    AlreadyUpdating,
}

/// 按下更新：同一把鎖內清掉死掉的 pid → 還有活的或佔位就 Busy → 沒有才立擋板。
pub(crate) fn try_raise(cli: &'static str, alive: impl Fn(u32) -> bool) -> Raise {
    let mut state = lock();
    if state.barriers.contains(cli) {
        return Raise::AlreadyUpdating;
    }
    state
        .inflight
        .retain(|_, (owner, pid)| *owner != cli || pid.is_none_or(&alive));
    if state.inflight.values().any(|(owner, _)| *owner == cli) {
        return Raise::Busy;
    }
    state.barriers.insert(cli);
    Raise::Raised(Barrier {
        cli,
        table: table(),
    })
}

/// 啟動復原用：不看在途（app 剛啟動、還沒有呼叫），直接立起；已經立著回 None。
pub(crate) fn raise_now(cli: &'static str) -> Option<Barrier> {
    lock().barriers.insert(cli).then_some(Barrier {
        cli,
        table: table(),
    })
}

pub(crate) fn is_raised(cli: &str) -> bool {
    lock().barriers.contains(cli)
}

/// 更新或還原的程序確認不了已停：擋板留著不撤（重新啟動 app 才會再處理），設定頁顯示「停不下來」。
pub(crate) fn hold(barrier: Barrier) {
    lock_table(barrier.table).stuck.insert(barrier.cli);
    std::mem::forget(barrier);
}

pub(crate) fn is_stuck(cli: &str) -> bool {
    lock().stuck.contains(cli)
}

/// pid 還活著嗎（殭屍也算活著：還沒收屍的程序由持有它的 `Spawned` 負責）。
#[cfg(unix)]
pub(crate) fn pid_alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    // SAFETY: signal 0 只做存在與權限檢查，不送訊號
    let result = unsafe { libc::kill(pid, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(windows)]
pub(crate) fn pid_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // SAFETY: 只查詢，拿到的 handle 一定關掉
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(handle, &mut code) != 0;
        CloseHandle(handle);
        ok && code == STILL_ACTIVE as u32
    }
}

/// 測試收尾：撤掉 `hold` 留著的擋板（正式流程只有重新啟動 app 會撤）。
#[cfg(test)]
pub(crate) fn release_held(cli: &str) {
    let mut state = lock();
    state.stuck.remove(cli);
    state.barriers.remove(cli);
}

#[cfg(test)]
pub(crate) fn inflight_count(cli: &str) -> usize {
    lock()
        .inflight
        .values()
        .filter(|(owner, _)| *owner == cli)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    // 全域表：各測試用不同的假 CLI 名字互不干擾（cli_of 只認四家，這裡直接用 &'static str）

    #[test]
    fn recognizes_cli_programs_only() {
        assert_eq!(
            cli_of(OsStr::new("/Users/x/.local/bin/claude")),
            Some("claude")
        );
        assert_eq!(cli_of(OsStr::new("codex.exe")), Some("codex"));
        assert_eq!(cli_of(OsStr::new("grok")), Some("grok"));
        assert_eq!(cli_of(OsStr::new("/bin/bash")), None);
        assert_eq!(cli_of(OsStr::new("powershell")), None);
    }

    #[test]
    fn placeholder_blocks_raise_until_dropped() {
        let slot = enter("t-placeholder").unwrap();
        assert!(matches!(try_raise("t-placeholder", |_| false), Raise::Busy));
        drop(slot);
        assert!(matches!(
            try_raise("t-placeholder", |_| false),
            Raise::Raised(_)
        ));
    }

    #[test]
    fn live_pid_blocks_and_dead_pid_is_cleared() {
        let slot = enter("t-pid").unwrap();
        slot.set_pid(Some(42));
        assert!(matches!(try_raise("t-pid", |_| true), Raise::Busy));
        // 死掉的 pid 被清掉後放行（佔位 Slot 還在也一樣：那筆已從表裡移除）
        let Raise::Raised(barrier) = try_raise("t-pid", |_| false) else {
            panic!("死掉的 pid 應放行");
        };
        assert_eq!(inflight_count("t-pid"), 0);
        drop(barrier);
        drop(slot);
    }

    #[test]
    fn barrier_rejects_new_calls_until_lowered() {
        let Raise::Raised(barrier) = try_raise("t-barrier", |_| false) else {
            panic!();
        };
        assert!(enter("t-barrier").is_err());
        assert!(is_raised("t-barrier"));
        assert!(matches!(
            try_raise("t-barrier", |_| false),
            Raise::AlreadyUpdating
        ));
        // 別家不受影響
        assert!(enter("t-barrier-other").is_ok());
        drop(barrier);
        assert!(enter("t-barrier").is_ok());
        assert!(!is_raised("t-barrier"));
    }

    /// 佔位期間 future 被丟棄：Slot 跟著 drop，之後按更新照樣放行。
    #[tokio::test]
    async fn dropped_future_releases_its_placeholder() {
        let pending = async {
            let _slot = enter("t-future").unwrap();
            std::future::pending::<()>().await;
        };
        let timed_out = tokio::time::timeout(std::time::Duration::from_millis(20), pending).await;
        assert!(timed_out.is_err());
        assert!(matches!(try_raise("t-future", |_| true), Raise::Raised(_)));
    }

    #[cfg(unix)]
    #[test]
    fn own_pid_is_alive_and_bogus_pid_is_not() {
        assert!(pid_alive(std::process::id()));
        assert!(!pid_alive(999_999_999));
    }
}
