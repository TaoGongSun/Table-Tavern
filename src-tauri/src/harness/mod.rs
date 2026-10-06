//! 測試通道：只在 `test-harness` feature 編譯，正式包完全不含。
//! 讓主線（Claude 透過 `scripts/harness.mjs`）對真 app 送 JS、回答原生對話窗。
//! 規格：.ai/plans/test-harness.md。

mod ai_log;
pub(crate) mod dialog;
mod eval;
pub(crate) mod motion;
mod rewrite_evidence;
mod root;
mod route;
mod server;
mod usage_raw;

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub(crate) use ai_log::{ai_dispatch, ai_event, cli_model, is_ai_probe};
pub(crate) use rewrite_evidence::rewrite_evidence;
pub(crate) use root::HARNESS_IDENTIFIER;
pub(crate) use usage_raw::{parsed_usage, raw_usage};

struct Boot {
    root: PathBuf,
    session: String,
    launch_id: String,
    /// 持有到 process 結束；drop 或死掉時 flock 自動釋放。
    _lock: File,
}

static BOOT: OnceLock<Boot> = OnceLock::new();

fn fail(message: &str) -> ! {
    eprintln!("[test-harness] 拒絕啟動：{message}");
    std::process::exit(78);
}

pub(crate) fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).expect("系統亂數不可用");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| fail("找不到家目錄"))
}

/// 建 Tauri 之前呼叫：核對 identifier、驗 root、取鎖、必要時 fresh 清理與複製設定。任一步失敗就結束 process。
pub(crate) fn boot(identifier: &str) {
    if identifier != HARNESS_IDENTIFIER {
        fail(&format!(
            "test-harness feature 只能配 {HARNESS_IDENTIFIER}（目前 {identifier}）；請用 npm run harness:build"
        ));
    }
    let raw = std::env::var("TT_HARNESS_ROOT").unwrap_or_else(|_| fail("缺 TT_HARNESS_ROOT"));
    let layout = root::Layout::from_home(home_dir());
    let root = root::validate_root(&raw, &layout).unwrap_or_else(|e| fail(&e));
    let session = random_hex(16);
    let launch_id = std::env::var("TT_HARNESS_LAUNCH_ID").unwrap_or_default();
    let holder = format!(
        "pid={} root={} session={}",
        std::process::id(),
        root.display(),
        session
    );
    let lock = root::acquire_lock(&layout.control, &holder).unwrap_or_else(|e| fail(&e));
    if std::env::var("TT_HARNESS_FRESH").as_deref() == Ok("1") {
        root::wipe(&root, &layout.shared).unwrap_or_else(|e| fail(&e));
    }
    let config_from = std::env::var("TT_HARNESS_CONFIG_FROM")
        .ok()
        .filter(|from| !from.is_empty())
        .map(PathBuf::from);
    root::prepare(&root, &layout, config_from.as_deref()).unwrap_or_else(|e| fail(&e));
    let _ = BOOT.set(Boot {
        root,
        session,
        launch_id,
        _lock: lock,
    });
}

fn boot_state() -> &'static Boot {
    BOOT.get().expect("harness::boot 必須先跑")
}

pub(crate) fn data_root() -> PathBuf {
    boot_state().root.join("data")
}

pub(crate) fn config_root() -> PathBuf {
    boot_state().root.join("config")
}

/// 尚未 boot（例如單元測試）時為 None。
fn harness_root() -> Option<PathBuf> {
    BOOT.get().map(|boot| boot.root.clone())
}

/// 尚未 boot（例如單元測試）時為 None，ai_log 就不寫。
fn ai_log_path() -> Option<PathBuf> {
    BOOT.get().map(|boot| boot.root.join("harness-ai.log"))
}

fn discovery_path() -> PathBuf {
    boot_state().root.join("harness.json")
}

/// setup 裡呼叫：開控制埠、掛 eval 回報監聽。
pub(crate) fn start(app: &tauri::AppHandle) {
    eval::listen(app);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = server::serve(app.clone()).await {
            eprintln!("[test-harness] 控制埠啟動失敗：{error}");
            app.exit(78);
        }
    });
}

/// 控制埠就緒後寫 discovery 檔（0600）。
fn write_discovery(port: u16, token: &str) -> Result<(), String> {
    let boot = boot_state();
    let body = serde_json::json!({
        "port": port,
        "token": token,
        "session": boot.session,
        "launchId": boot.launch_id,
        "pid": std::process::id(),
        "root": boot.root,
    });
    let path = discovery_path();
    let tmp = path.with_extension("json.tmp");
    write_private(&tmp, body.to_string().as_bytes())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|e| e.to_string())?;
    file.write_all(bytes).map_err(|e| e.to_string())
}

/// app 退出：清 discovery 檔。鎖隨 process 結束釋放，鎖檔本身不刪。
pub(crate) fn shutdown() {
    if BOOT.get().is_some() {
        let _ = std::fs::remove_file(discovery_path());
    }
}

fn status_json() -> serde_json::Value {
    let boot = boot_state();
    serde_json::json!({
        "session": boot.session,
        "launchId": boot.launch_id,
        "pid": std::process::id(),
        "root": boot.root,
        "pendingDialogs": dialog::pending_count(),
        "pendingEvals": eval::pending_count(),
        "aiLogWriteFailures": ai_log::write_failures(),
    })
}
