//! macOS 專用：把子行程的「責任歸屬」還給它自己。
//!
//! macOS 的權限彈窗是照 responsible process 歸戶，子行程預設繼承父行程。Claude Code CLI 一啟動
//! 就自建沙盒、向系統索取桌面／音樂等標準資料夾權限，於是彈窗掛的是「Table Tavern」的名字
//! （tccd 日誌實證 2026-07-28）。posix_spawn 的 disclaim 屬性能讓子行程自己當責任人，彈窗就會
//! 落回 CLI 自己頭上。標準庫的 Command 沒有這個開關，所以這裡自己接管線與 posix_spawn。

use std::ffi::CString;
use std::io;
use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use tokio::net::unix::pipe;

extern "C" {
    /// 私有 SPI（Chromium／Electron 同款用法）：子行程脫離父行程的責任歸屬
    fn responsibility_spawnattrs_setdisclaim(
        attr: *mut libc::posix_spawnattr_t,
        disclaim: libc::c_int,
    ) -> libc::c_int;
}

pub struct DisclaimedChild {
    pub pid: libc::pid_t,
    pub stdin: pipe::Sender,
    pub stdout: pipe::Receiver,
    pub stderr: pipe::Receiver,
}

/// 三條管線各建一組，子行程端 dup2 到 0／1／2，父行程端交給 tokio 非同步收送。
pub fn spawn(
    program: &Path,
    args: &[String],
    envs: &[(String, String)],
) -> io::Result<DisclaimedChild> {
    let path = CString::new(program.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "程式路徑含 NUL"))?;
    let argv_owned = std::iter::once(program.as_os_str().to_string_lossy().into_owned())
        .chain(args.iter().cloned())
        .map(CString::new)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "參數含 NUL"))?;
    // 父行程環境為底、呼叫端指定的覆寫在上
    let mut env: Vec<(String, String)> = std::env::vars().collect();
    for (key, value) in envs {
        env.retain(|(existing, _)| existing != key);
        env.push((key.clone(), value.clone()));
    }
    let envp_owned = env
        .into_iter()
        .map(|(key, value)| CString::new(format!("{key}={value}")))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "環境變數含 NUL"))?;
    let mut argv: Vec<*mut libc::c_char> =
        argv_owned.iter().map(|arg| arg.as_ptr() as *mut _).collect();
    argv.push(std::ptr::null_mut());
    let mut envp: Vec<*mut libc::c_char> =
        envp_owned.iter().map(|item| item.as_ptr() as *mut _).collect();
    envp.push(std::ptr::null_mut());

    let (stdin_read, stdin_write) = new_pipe()?;
    let (stdout_read, stdout_write) = new_pipe()?;
    let (stderr_read, stderr_write) = new_pipe()?;

    let pid = unsafe {
        let mut actions: libc::posix_spawn_file_actions_t = std::mem::zeroed();
        check(libc::posix_spawn_file_actions_init(&mut actions))?;
        let mut attr: libc::posix_spawnattr_t = std::mem::zeroed();
        check(libc::posix_spawnattr_init(&mut attr))?;
        let result: io::Result<libc::pid_t> = (|| {
            check(responsibility_spawnattrs_setdisclaim(&mut attr, 1))?;
            for (from, to) in [
                (&stdin_read, libc::STDIN_FILENO),
                (&stdout_write, libc::STDOUT_FILENO),
                (&stderr_write, libc::STDERR_FILENO),
            ] {
                check(libc::posix_spawn_file_actions_adddup2(
                    &mut actions,
                    as_raw(from),
                    to,
                ))?;
            }
            let mut pid: libc::pid_t = 0;
            check(libc::posix_spawn(
                &mut pid,
                path.as_ptr(),
                &actions,
                &attr,
                argv.as_ptr(),
                envp.as_ptr(),
            ))?;
            Ok(pid)
        })();
        libc::posix_spawn_file_actions_destroy(&mut actions);
        libc::posix_spawnattr_destroy(&mut attr);
        result?
    };
    // 子行程端留在子行程裡，父行程這邊關掉，否則讀端永遠等不到 EOF
    drop((stdin_read, stdout_write, stderr_write));

    // 父行程這端交給 tokio 事件迴圈，必須非阻塞（子行程那端不受影響，是另一組描述）
    for fd in [&stdin_write, &stdout_read, &stderr_read] {
        set_nonblocking(fd)?;
    }
    Ok(DisclaimedChild {
        pid,
        stdin: pipe::Sender::from_owned_fd(stdin_write)?,
        stdout: pipe::Receiver::from_owned_fd(stdout_read)?,
        stderr: pipe::Receiver::from_owned_fd(stderr_read)?,
    })
}

/// 收屍並回傳給人看的結束狀態字串（阻塞的 waitpid 丟到 blocking 執行緒）。
pub async fn wait(pid: libc::pid_t) -> io::Result<String> {
    tokio::task::spawn_blocking(move || {
        let mut status: libc::c_int = 0;
        if unsafe { libc::waitpid(pid, &mut status, 0) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(if libc::WIFEXITED(status) {
            format!("exit status: {}", libc::WEXITSTATUS(status))
        } else if libc::WIFSIGNALED(status) {
            format!("signal: {}", libc::WTERMSIG(status))
        } else {
            format!("status: {status}")
        })
    })
    .await
    .map_err(|error| io::Error::other(error.to_string()))?
}

/// pipe(2) 與 posix_spawn 系列的錯誤慣例不同：失敗回 -1 並設 errno。
/// 兩端一律設 close-on-exec：否則子行程會連我們手上的另外兩組管線一起繼承，
/// 它的 stdin 因而永遠等不到 EOF（dup2 到 0／1／2 的那三個會自動去掉這個旗標）。
fn new_pipe() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [0 as libc::c_int; 2];
    if unsafe { libc::pipe(fds.as_mut_ptr()) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let ends = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
    for fd in [&ends.0, &ends.1] {
        if unsafe { libc::fcntl(as_raw(fd), libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(ends)
}

fn set_nonblocking(fd: &OwnedFd) -> io::Result<()> {
    let raw = as_raw(fd);
    let flags = unsafe { libc::fcntl(raw, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(raw, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn as_raw(fd: &OwnedFd) -> libc::c_int {
    use std::os::fd::AsRawFd;
    fd.as_raw_fd()
}

/// posix_spawn 系列回傳 errno 本身（不是設 errno 回 -1），非 0 一律當錯誤
fn check(code: libc::c_int) -> io::Result<()> {
    if code == 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    extern "C" {
        fn responsibility_get_pid_responsible_for_pid(pid: libc::pid_t) -> libc::pid_t;
    }

    /// 這個模組存在的唯一理由：子行程的責任人必須是它自己，不是我們
    #[tokio::test]
    async fn spawned_child_is_responsible_for_itself() {
        let child = spawn(Path::new("/bin/echo"), &["ok".to_owned()], &[]).unwrap();
        let responsible = unsafe { responsibility_get_pid_responsible_for_pid(child.pid) };
        assert_eq!(responsible, child.pid, "子行程仍掛在父行程的責任歸屬下");
        let pid = child.pid;
        drop(child);
        wait(pid).await.unwrap();
    }
}
