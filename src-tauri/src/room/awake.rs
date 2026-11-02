//! 防休眠（計畫 §2、協定 §7.4）：房間模式開著時擋系統睡眠，`Guard` 丟掉就解除。
//! Windows 用 `SetThreadExecutionState`（專用執行緒持有）；macOS 起 `caffeinate -i -w <本程序>`、
//! Linux 起 `systemd-inhibit … tail --pid=<本程序>`，本程序意外結束時它們跟著退。拿不到回報 false。

#[derive(Default)]
pub(super) struct Guard {
    #[cfg(unix)]
    _child: Option<crate::cli::Spawned>,
    #[cfg(windows)]
    stop: Option<std::sync::mpsc::Sender<()>>,
}

#[cfg(windows)]
impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

/// 回 (Guard, 是否生效)；平台不支援回 None
#[cfg(unix)]
pub(super) async fn hold() -> (Guard, Option<bool>) {
    let pid = std::process::id().to_string();
    let (program, args) = if cfg!(target_os = "macos") {
        ("caffeinate", vec!["-i".to_owned(), "-w".to_owned(), pid])
    } else {
        let args = [
            "--what=sleep:idle",
            "--who=Table Tavern",
            "--why=Multiplayer room mode",
            "--mode=block",
            "tail",
            &format!("--pid={pid}"),
            "-f",
            "/dev/null",
        ];
        ("systemd-inhibit", args.map(str::to_owned).to_vec())
    };
    let Ok(mut child) = crate::cli::spawn_helper(program, &args) else {
        return (Guard::default(), Some(false));
    };
    // 起得來但拿不到（沒有 logind、權限不足）會馬上退出：等一下再看它還在不在
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    match child.child().try_wait() {
        Ok(None) => (
            Guard {
                _child: Some(child),
            },
            Some(true),
        ),
        _ => (Guard::default(), Some(false)),
    }
}

#[cfg(windows)]
pub(super) async fn hold() -> (Guard, Option<bool>) {
    use windows_sys::Win32::System::Power::{
        SetThreadExecutionState, ES_CONTINUOUS, ES_SYSTEM_REQUIRED,
    };
    let (stop, wait) = std::sync::mpsc::channel::<()>();
    let (held, report) = std::sync::mpsc::channel::<bool>();
    // 執行狀態跟著呼叫的執行緒：開一條專用執行緒持有到 Guard 丟掉
    std::thread::spawn(move || {
        // SAFETY: 只傳旗標、沒有指標；回 0 表示失敗
        let ok = unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) } != 0;
        let _ = held.send(ok);
        let _ = wait.recv();
        // SAFETY: 同上，清掉這條執行緒的持有
        unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
    });
    let ok = report.recv().unwrap_or(false);
    (Guard { stop: Some(stop) }, Some(ok))
}

#[cfg(not(any(unix, windows)))]
pub(super) async fn hold() -> (Guard, Option<bool>) {
    (Guard::default(), None)
}
