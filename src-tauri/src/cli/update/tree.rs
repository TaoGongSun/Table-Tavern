//! 更新與還原程序的程序樹（plans/cli-version-gate.md 包 2 第 3 點「程序樹與逾時」）：
//! unix 放進獨立程序群組、Windows 以 CREATE_SUSPENDED 建立 → 綁進 Job Object → 才放行；
//! 不論正常結束或逾時，都要確認整組都停了才往下走（還原、撤擋板）。
//! spawn 本身在 `cli/spawn.rs`（子程序唯一入口），這裡只管群組的建立前設定、綁定與整組殺。

use std::io;
use std::process::Output;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};

/// 整組都停了的確認上限：送出殺訊號後還有成員就一直等到這裡為止。
const CONFIRM_LIMIT: Duration = Duration::from_secs(10);

/// 更新程序的身分（寫進啟動復原標記）：程序群組 id＋組長的啟動時間，防 pid 重用時誤殺別人。
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Group {
    pub pgid: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started: Option<u64>,
}

pub(crate) struct TreeChild {
    child: Child,
    /// 程序群組 id（＝子程序 pid）；主程序收屍後 `child.id()` 就拿不到了，所以先記下
    #[cfg(unix)]
    group: Option<libc::pid_t>,
    #[cfg(windows)]
    job: Job,
}

/// spawn 前的設定：unix 自成一個程序群組（群組 id＝子程序 pid）；
/// Windows 先暫停建立，綁進 Job 之後才放行，免得它在綁定前就叫起不受管的子孫。
pub(crate) fn prepare(command: &mut Command) {
    #[cfg(unix)]
    command.process_group(0);
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Threading::{CREATE_NO_WINDOW, CREATE_SUSPENDED};
        command.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);
    }
}

/// spawn 後綁定。Windows：綁進 Job 再恢復執行；任一步失敗就殺掉（還沒跑過一行）回錯。
pub(crate) fn adopt(mut child: Child) -> io::Result<TreeChild> {
    #[cfg(windows)]
    {
        let bound = Job::new().and_then(|job| {
            job.assign(&child)?;
            resume(&child)?;
            Ok(job)
        });
        match bound {
            Ok(job) => Ok(TreeChild { child, job }),
            Err(error) => {
                let _ = child.start_kill();
                Err(error)
            }
        }
    }
    #[cfg(unix)]
    {
        let _ = &mut child;
        let group = child.id().and_then(|pid| libc::pid_t::try_from(pid).ok());
        Ok(TreeChild { child, group })
    }
}

/// 一步的結果：`stopped`＝確認整組都停了（false 時呼叫端不得往下走）。
pub(crate) struct Finished {
    /// None＝逾時或等待失敗
    pub output: Option<Output>,
    pub stopped: bool,
}

impl TreeChild {
    /// 寫進標記用的身分（Windows 沒有：app 一結束 Job 就整組收掉）。
    pub(crate) fn group(&self) -> Option<Group> {
        #[cfg(unix)]
        return self.group.map(|pgid| Group {
            pgid,
            started: start_time(pgid),
        });
        #[cfg(windows)]
        return None;
    }

    /// 等它跑完（收 stdout／stderr）；逾時就殺整組。兩種收場都要確認整組都停了。
    pub(crate) async fn finish(mut self, limit: Duration) -> Finished {
        let stdout = self.child.stdout.take();
        let stderr = self.child.stderr.take();
        let run = async {
            let (status, stdout, stderr) =
                tokio::try_join!(self.child.wait(), read_all(stdout), read_all(stderr))?;
            io::Result::Ok(Output {
                status,
                stdout,
                stderr,
            })
        };
        let output = match tokio::time::timeout(limit, run).await {
            Ok(output) => output.ok(),
            Err(_) => None,
        };
        // 正常結束也一樣：群組裡可能還留著安裝腳本叫起來的背景程序
        let stopped = {
            #[cfg(unix)]
            let Self { child, group } = &mut self;
            #[cfg(unix)]
            let group = *group;
            #[cfg(windows)]
            let Self { child, job } = &mut self;
            #[cfg(windows)]
            let job = &*job;
            confirm_stopped(
                child.wait(),
                || {
                    #[cfg(unix)]
                    if let Some(group) = group {
                        kill_group(group);
                    }
                    #[cfg(windows)]
                    job.terminate();
                },
                || {
                    #[cfg(unix)]
                    return group.is_none_or(group_gone);
                    #[cfg(windows)]
                    return job.active_processes() == Some(0);
                },
                CONFIRM_LIMIT,
            )
            .await
        };
        Finished { output, stopped }
    }
}

/// 送出殺訊號後確認整組都停了：主程序收屍與「群組裡沒人了」共用同一個期限，
/// 收屍遲遲不回來（核心卡住等）也算確認不了，回 false。
async fn confirm_stopped<R: std::future::Future>(
    reap: R,
    mut kill: impl FnMut(),
    mut gone: impl FnMut() -> bool,
    limit: Duration,
) -> bool {
    let deadline = tokio::time::Instant::now() + limit;
    kill();
    if tokio::time::timeout_at(deadline, reap).await.is_err() {
        return false;
    }
    loop {
        if gone() {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        kill();
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[cfg(unix)]
fn kill_group(pgid: libc::pid_t) {
    // SAFETY: 送 SIGKILL 給自己建的群組
    unsafe {
        libc::killpg(pgid, libc::SIGKILL);
    }
}

#[cfg(unix)]
fn group_gone(pgid: libc::pid_t) -> bool {
    // SAFETY: signal 0 只檢查群組是否還有成員
    // EPERM＝這個群組 id 現在屬於別的使用者：不可能是我們建的那組
    let result = unsafe { libc::killpg(pgid, 0) };
    result != 0
        && matches!(
            io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH) | Some(libc::EPERM)
        )
}

/// 測試用：程序的啟動時間。
#[cfg(all(test, unix))]
pub(crate) fn start_time_of(pid: libc::pid_t) -> Option<u64> {
    start_time(pid)
}

/// 程序的啟動時間（秒，跟 pid 合起來當身分）；查不到回 None。
#[cfg(target_os = "macos")]
fn start_time(pid: libc::pid_t) -> Option<u64> {
    // SAFETY: proc_pidinfo 寫進大小正確的 proc_bsdinfo
    unsafe {
        let mut info: libc::proc_bsdinfo = std::mem::zeroed();
        let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
        let written = libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            &mut info as *mut _ as *mut libc::c_void,
            size,
        );
        (written == size).then_some(info.pbi_start_tvsec)
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn start_time(pid: libc::pid_t) -> Option<u64> {
    // /proc/<pid>/stat 第 22 欄（開機後的 clock ticks）；行程名可能含空白，從最後一個 ')' 之後切
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rest = &stat[stat.rfind(')')? + 1..];
    rest.split_whitespace().nth(19)?.parse().ok()
}

/// 啟動復原：上一輪記下的更新程序群組是否已經停了；還活著（而且確定是我們那組）就殺整組並確認。
/// 組長還在但啟動時間對不上＝pid 被別人重用，我們那組早就沒了；查不到組長的啟動時間就不動它，回 false。
#[cfg(unix)]
pub(crate) async fn ensure_gone(group: &Group) -> bool {
    let pgid = group.pgid;
    if group_gone(pgid) {
        return true;
    }
    // SAFETY: signal 0 只檢查組長在不在
    let leader_alive = unsafe { libc::kill(pgid, 0) } == 0;
    if leader_alive {
        match (start_time(pgid), group.started) {
            (Some(now), Some(then)) if now != then => return true,
            (Some(_), Some(_)) => {}
            _ => return false,
        }
    }
    let deadline = tokio::time::Instant::now() + CONFIRM_LIMIT;
    loop {
        kill_group(pgid);
        if group_gone(pgid) {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Windows：app 結束時 Job 的 handle 關掉、整組已被系統收掉，標記裡不會有群組。
#[cfg(windows)]
pub(crate) async fn ensure_gone(_group: &Group) -> bool {
    true
}

async fn read_all(pipe: Option<impl AsyncReadExt + Unpin>) -> io::Result<Vec<u8>> {
    let mut buffer = Vec::new();
    if let Some(mut pipe) = pipe {
        pipe.read_to_end(&mut buffer).await?;
    }
    Ok(buffer)
}

#[cfg(windows)]
struct Job(windows_sys::Win32::Foundation::HANDLE);

// SAFETY: Job handle 是核心物件的參照，可跨執行緒使用
#[cfg(windows)]
unsafe impl Send for Job {}
#[cfg(windows)]
unsafe impl Sync for Job {}

#[cfg(windows)]
impl Job {
    fn new() -> io::Result<Self> {
        use windows_sys::Win32::System::JobObjects::{
            CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        // SAFETY: 建立匿名 Job，設定「handle 關掉就殺整組」，失敗時關掉 handle
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            let job = Job(handle);
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(job)
        }
    }

    fn assign(&self, child: &Child) -> io::Result<()> {
        use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
        let handle = child
            .raw_handle()
            .ok_or_else(|| io::Error::other("child already exited"))?;
        // SAFETY: handle 來自還活著的 Child
        match unsafe { AssignProcessToJobObject(self.0, handle as _) } {
            0 => Err(io::Error::last_os_error()),
            _ => Ok(()),
        }
    }

    fn terminate(&self) {
        // SAFETY: 殺掉 Job 裡全部程序
        unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.0, 1);
        }
    }

    /// 查不到回 None（當成還沒停）。
    fn active_processes(&self) -> Option<u32> {
        use windows_sys::Win32::System::JobObjects::{
            JobObjectBasicAccountingInformation, QueryInformationJobObject,
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
        };
        // SAFETY: 查詢 Job 的計數
        unsafe {
            let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = std::mem::zeroed();
            let ok = QueryInformationJobObject(
                self.0,
                JobObjectBasicAccountingInformation,
                &mut info as *mut _ as *mut std::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                std::ptr::null_mut(),
            );
            match ok {
                0 => None,
                _ => Some(info.ActiveProcesses),
            }
        }
    }
}

/// CREATE_SUSPENDED 建立的程序只有一條主執行緒：找出來放行。
#[cfg(windows)]
fn resume(child: &Child) -> io::Result<()> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    use windows_sys::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};
    let pid = child
        .id()
        .ok_or_else(|| io::Error::other("child already exited"))?;
    // SAFETY: 快照與執行緒 handle 用完都關掉
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let mut entry: THREADENTRY32 = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
        let mut resumed = 0;
        let mut more = Thread32First(snapshot, &mut entry) != 0;
        while more {
            if entry.th32OwnerProcessID == pid {
                let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID);
                if !thread.is_null() {
                    if ResumeThread(thread) != u32::MAX {
                        resumed += 1;
                    }
                    CloseHandle(thread);
                }
            }
            more = Thread32Next(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
        match resumed {
            0 => Err(io::Error::other("could not resume the suspended process")),
            _ => Ok(()),
        }
    }
}

#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        // SAFETY: KILL_ON_JOB_CLOSE：關 handle 時殘留成員一併結束
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 主程序收屍遲遲不完成：期限到了就判確認不了，不會一直等下去。
    #[tokio::test]
    async fn stuck_reap_is_not_confirmed() {
        let started = std::time::Instant::now();
        let stopped = confirm_stopped(
            std::future::pending::<()>(),
            || {},
            || true,
            Duration::from_millis(200),
        )
        .await;
        assert!(!stopped);
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[tokio::test]
    async fn reaped_and_gone_is_confirmed_and_lingering_group_is_not() {
        let mut kills = 0;
        assert!(confirm_stopped(async {}, || kills += 1, || true, Duration::from_secs(1)).await);
        assert_eq!(kills, 1);
        assert!(!confirm_stopped(async {}, || {}, || false, Duration::from_millis(200)).await);
    }
}
