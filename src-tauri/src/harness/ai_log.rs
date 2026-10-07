//! AI log：在實際派送點記錄到 `<root>/harness-ai.log`（JSON Lines），供主線核對「這段操作有沒有打 AI、打了哪個模型」。
//! 每次派送一行 `dispatch`（帶 id），之後同 id 的 `spawned`／`spawn-failed`（CLI）與 `responder`（實際回應模型）另起一行。
//! 寫入失敗不影響呼叫本身，但會計數；計數非零時空 log 不能當成「零派送」的證據。
//! 派送點分散在多個執行緒：每行先序列化成含換行的完整字串，在同一把鎖內一次 write_all；
//! 讀取也拿同一把鎖，不會讀到寫一半的行。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde_json::{json, Value};

static WRITE_FAILURES: AtomicU64 = AtomicU64::new(0);
static LOG_LOCK: Mutex<()> = Mutex::new(());

/// 單元測試用：沒有 boot 時改寫這個路徑（正式執行一律用 `<root>/harness-ai.log`）。
#[cfg(test)]
static TEST_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

fn log_path() -> Option<PathBuf> {
    #[cfg(test)]
    if let Some(path) = TEST_PATH.lock().unwrap_or_else(|p| p.into_inner()).clone() {
        return Some(path);
    }
    super::ai_log_path()
}

pub(super) fn write_failures() -> u64 {
    WRITE_FAILURES.load(Ordering::Relaxed)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 序列化成一整行後，在鎖內開檔並一次寫完。
fn append_at(path: &Path, line: &Value) -> std::io::Result<()> {
    let mut text = line.to_string();
    text.push('\n');
    let _guard = LOG_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(text.as_bytes())
}

fn append(mut line: Value) {
    // 尚未 boot（單元測試）不寫也不算失敗；測試包執行時一定已 boot
    let Some(path) = log_path() else {
        return;
    };
    line["ts"] = Value::from(now_ms());
    if append_at(&path, &line).is_err() {
        WRITE_FAILURES.fetch_add(1, Ordering::Relaxed);
    }
}

/// 一次派送。`context` 帶得到的脈絡（world、shape、lane）。回傳 id 供後續事件關聯。
pub(crate) fn ai_dispatch(provider: &str, model: &str, context: Value) -> String {
    let id = super::random_hex(6);
    append(
        json!({ "id": id, "event": "dispatch", "provider": provider, "model": model, "context": context }),
    );
    id
}

/// 同一次派送的後續事件：`spawned`、`spawn-failed`、`responder` 等。
pub(crate) fn ai_event(id: &str, event: &str, detail: Value) {
    append(json!({ "id": id, "event": event, "detail": detail }));
}

/// 不屬於任何一次派送的紀錄（例如留證失敗）。
pub(super) fn ai_note(event: &str, detail: Value) {
    append(json!({ "event": event, "detail": detail }));
}

/// CLI 參數裡的模型旗標值（--model x、--model=x、-m x）；沒有就是 CLI 預設。
pub(crate) fn cli_model(args: &[String]) -> String {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--model" || arg == "-m" {
            if let Some(value) = iter.next() {
                return value.clone();
            }
        }
        if let Some(value) = arg.strip_prefix("--model=") {
            return value.to_owned();
        }
    }
    "（CLI 預設）".to_owned()
}

/// `-p` 是 claude／agy 的單次提問旗標：帶它的探測會真的跑一次推理。
pub(crate) fn is_ai_probe(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "-p")
}

/// 讀整份 log（與寫入同一把鎖）。檔案不存在＝這個 root 還沒派送過；其他讀取錯誤或任何一行
/// 解析失敗都回錯，不轉成空陣列——空陣列只在「確實讀到、確實沒有」時才成立。
fn read_at(path: &Path) -> Result<Vec<Value>, String> {
    let text = {
        let _guard = LOG_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(format!("讀 {} 失敗：{error}", path.display())),
        }
    };
    let mut entries = Vec::new();
    for (index, line) in text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
    {
        let entry: Value = serde_json::from_str(line)
            .map_err(|e| format!("{} 第 {} 行解析失敗：{e}", path.display(), index + 1))?;
        entries.push(entry);
    }
    Ok(entries)
}

pub(super) fn read() -> Result<Value, String> {
    let path = log_path().ok_or_else(|| "harness 尚未啟動".to_owned())?;
    let entries = read_at(&path)?;
    Ok(json!({ "entries": entries, "writeFailures": write_failures() }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tt-harness-ailog-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn cli_model_flag_is_extracted() {
        let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(cli_model(&args(&["-p", "--model", "opus"])), "opus");
        assert_eq!(cli_model(&args(&["--model=gpt-x"])), "gpt-x");
        assert_eq!(cli_model(&args(&["-m", "grok-4"])), "grok-4");
        assert_eq!(cli_model(&args(&["-p"])), "（CLI 預設）");
        assert!(is_ai_probe(&args(&["-p", "ok"])));
        assert!(!is_ai_probe(&args(&["login", "status"])));
    }

    #[test]
    fn concurrent_writes_keep_every_line_whole_and_ids_linked() {
        let path = scratch("concurrent").join("harness-ai.log");
        const THREADS: usize = 8;
        const PER_THREAD: usize = 100;
        let handles: Vec<_> = (0..THREADS)
            .map(|t| {
                let path = path.clone();
                std::thread::spawn(move || {
                    for i in 0..PER_THREAD {
                        // 長欄位讓單行遠超過一次 write 的常見緩衝，放大交錯機會
                        let id = format!("t{t}-{i}");
                        let filler = "長".repeat(2000);
                        append_at(
                            &path,
                            &json!({ "id": id, "event": "dispatch", "model": filler }),
                        )
                        .unwrap();
                        append_at(&path, &json!({ "id": id, "event": "spawned" })).unwrap();
                    }
                })
            })
            .collect();
        // 寫入途中同時讀：每次讀到的都必須整行可解析
        for _ in 0..20 {
            read_at(&path).unwrap();
        }
        for handle in handles {
            handle.join().unwrap();
        }
        let entries = read_at(&path).unwrap();
        assert_eq!(entries.len(), THREADS * PER_THREAD * 2);
        for t in 0..THREADS {
            for i in 0..PER_THREAD {
                let id = format!("t{t}-{i}");
                let events: Vec<&str> = entries
                    .iter()
                    .filter(|e| e["id"] == id.as_str())
                    .map(|e| e["event"].as_str().unwrap())
                    .collect();
                assert_eq!(events, ["dispatch", "spawned"], "{id}");
            }
        }
    }

    /// 真的 spawn 子程序（假 CLI 腳本與不存在的路徑），不花額度：
    /// 確認 run_cli 會記 dispatch，並依結果記 spawned／spawn-failed。
    #[cfg(unix)]
    #[tokio::test]
    async fn run_cli_logs_spawned_and_spawn_failed() {
        #![allow(clippy::await_holding_lock)]
        let _serial = crate::inflight::lock_real_process_tests();
        let dir = scratch("cli");
        let log = dir.join("harness-ai.log");
        *TEST_PATH.lock().unwrap() = Some(log.clone());
        let script = dir.join("fake-cli.sh");
        std::fs::write(&script, "#!/bin/sh\ncat >/dev/null\necho '{\"type\":\"result\",\"is_error\":false,\"result\":\"ok\"}'\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let args = vec!["--model".to_owned(), "fake-model".to_owned()];
        let ran = crate::cli::run_cli(
            &script,
            &dir,
            &args,
            "提示詞",
            &[],
            crate::cli::parse_claude_line,
            false,
            crate::transport::RunawayPolicy::Off,
            None,
            |_: &str| {},
        )
        .await;
        let missing = crate::cli::run_cli(
            &dir.join("no-such-cli"),
            &dir,
            &[],
            "",
            &[],
            crate::cli::parse_claude_line,
            false,
            crate::transport::RunawayPolicy::Off,
            None,
            |_: &str| {},
        )
        .await;
        *TEST_PATH.lock().unwrap() = None;
        assert!(ran.is_ok(), "{ran:?}");
        assert!(missing.is_err());
        let entries = read_at(&log).unwrap();
        let mine: Vec<_> = entries
            .iter()
            .filter(|e| e["event"] == "dispatch")
            .map(|e| {
                let id = e["id"].as_str().unwrap();
                let follow: Vec<&str> = entries
                    .iter()
                    .filter(|f| f["id"] == id && f["event"] != "dispatch")
                    .map(|f| f["event"].as_str().unwrap())
                    .collect();
                (
                    e["provider"].as_str().unwrap().to_owned(),
                    e["model"].as_str().unwrap().to_owned(),
                    follow,
                )
            })
            .collect();
        assert!(
            mine.contains(&(
                "cli:fake-cli.sh".to_owned(),
                "fake-model".to_owned(),
                vec!["spawned"]
            )),
            "{mine:?}"
        );
        assert!(
            mine.contains(&(
                "cli:no-such-cli".to_owned(),
                "（CLI 預設）".to_owned(),
                vec!["spawn-failed"]
            )),
            "{mine:?}"
        );
    }
}
