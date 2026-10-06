//! CLI 提示詞暫存檔：system 與正文不再塞命令列參數（Windows 整條命令列只有 32,767 個
//! UTF-16 單位，中文桌約 3 萬字就 spawn 失敗；macOS 是 argv＋env 合計 1MB）。
//!
//! 生命週期：呼叫端持有 `PromptFile` 跨過整個 `run_cli` await。`run_cli` 返回前必定已
//! kill＋wait 收屍，所以正常路徑上 drop 時沒有程序還開著檔。外層 future 被 drop
//! （中止在途呼叫）時只有 `kill_on_drop`、等不到收屍，Windows 上刪檔可能失敗——
//! 那時改由背景執行緒重試幾秒，仍刪不掉就留給下次建立時的過期清理。

use crate::data::DataResult;
use crate::ui_msg::UiMsg;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// 超過這個年紀的殘檔（當機、刪檔重試也失敗）在下次建立時清掉。
const STALE_AFTER: Duration = Duration::from_secs(24 * 60 * 60);
const RETRY_EVERY: Duration = Duration::from_millis(500);
const RETRY_TIMES: u32 = 20;
/// 檔名＝26 字 ulid＋`-`＋用途；清理只認這個格式，不動資料夾裡的其他東西。
const ULID_LEN: usize = 26;

pub struct PromptFile {
    path: PathBuf,
}

impl PromptFile {
    /// 在 `dir` 底下建立 `<ulid>-<suffix>` 並寫入 `content`。`create_new` 不覆寫既有檔；
    /// unix 上建立當下就是 0600（資料夾 0700），不是寫完才收權限。寫入失敗刪掉半成品再回錯。
    /// 錯誤沿用「CLI 工作目錄準備不了」那句人話。
    pub fn create(dir: &Path, suffix: &str, content: &str) -> Result<Self, String> {
        Self::create_inner(dir, suffix, content).map_err(|error| {
            UiMsg::CliWorkspaceFailed {
                error: error.to_string(),
            }
            .to_string()
        })
    }

    fn create_inner(dir: &Path, suffix: &str, content: &str) -> DataResult<Self> {
        std::fs::create_dir_all(dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
        }
        sweep_stale(dir, SystemTime::now());
        let path = dir.join(format!("{}-{suffix}", ulid::Ulid::generate()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path)?;
        let written = file
            .write_all(content.as_bytes())
            .and_then(|_| file.flush());
        drop(file);
        if let Err(error) = written {
            let _ = std::fs::remove_file(&path);
            return Err(error.into());
        }
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PromptFile {
    fn drop(&mut self) {
        if remove_now(&self.path) {
            return;
        }
        // 多半是 Windows 上程序還開著檔（外層 future 被 drop、沒等到收屍）：背景重試幾秒
        let path = self.path.clone();
        let _ = std::thread::Builder::new()
            .name("prompt-file-cleanup".to_owned())
            .spawn(move || {
                for _ in 0..RETRY_TIMES {
                    std::thread::sleep(RETRY_EVERY);
                    if remove_now(&path) {
                        return;
                    }
                }
                eprintln!("[prompt-file] 刪不掉 {}，留給下次過期清理", path.display());
            });
    }
}

/// 刪掉了或本來就不在都算成功。
fn remove_now(path: &Path) -> bool {
    match std::fs::remove_file(path) {
        Ok(()) => true,
        Err(error) => error.kind() == std::io::ErrorKind::NotFound,
    }
}

fn is_ours(name: &str) -> bool {
    name.len() > ULID_LEN + 1
        && name.as_bytes()[ULID_LEN] == b'-'
        && ulid::Ulid::from_string(&name[..ULID_LEN]).is_ok()
}

/// 清掉本模組留下、超過 24 小時的檔。並發時檔案已被別人刪掉、權限不足、讀不到時間，
/// 一律只記 log，不能讓新請求因清理失敗。
fn sweep_stale(dir: &Path, now: SystemTime) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !is_ours(name) {
            continue;
        }
        let stale = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age > STALE_AFTER);
        if stale && !remove_now(&entry.path()) {
            eprintln!("[prompt-file] 清不掉過期檔 {}", entry.path().display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "tt-prompt-file-{tag}-{}-{}",
            std::process::id(),
            ulid::Ulid::generate()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn create_writes_exact_bytes_and_drop_removes_the_file() {
        let dir = temp_dir("roundtrip");
        let content = "  前導空白\r\n中文「引號」\"雙引號\" $HOME %USERPROFILE%\n\n";
        let file = PromptFile::create(&dir, "system.txt", content).unwrap();
        let path = file.path().to_path_buf();
        assert_eq!(std::fs::read(&path).unwrap(), content.as_bytes());
        assert!(path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .ends_with("-system.txt"));
        assert!(is_ours(path.file_name().unwrap().to_str().unwrap()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
            let dir_mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
            assert_eq!(dir_mode, 0o700);
        }
        drop(file);
        assert!(!path.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn two_files_with_same_suffix_never_collide() {
        let dir = temp_dir("collide");
        let a = PromptFile::create(&dir, "p.txt", "a").unwrap();
        let b = PromptFile::create(&dir, "p.txt", "b").unwrap();
        assert_ne!(a.path(), b.path());
        assert_eq!(std::fs::read_to_string(a.path()).unwrap(), "a");
        assert_eq!(std::fs::read_to_string(b.path()).unwrap(), "b");
        drop((a, b));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn drop_after_someone_else_deleted_the_file_is_fine() {
        let dir = temp_dir("gone");
        let file = PromptFile::create(&dir, "p.txt", "x").unwrap();
        std::fs::remove_file(file.path()).unwrap();
        drop(file); // 不 panic、不開重試執行緒卡住
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn sweep_only_removes_our_stale_files() {
        let dir = temp_dir("sweep");
        std::fs::create_dir_all(&dir).unwrap();
        let ours = dir.join(format!("{}-system.txt", ulid::Ulid::generate()));
        let foreign = dir.join("notes.txt");
        let lookalike = dir.join("NOT-A-ULID-AT-ALL-123456789-x.txt");
        for path in [&ours, &foreign, &lookalike] {
            std::fs::write(path, "x").unwrap();
        }
        // 現在掃：都還新鮮，一個都不動
        sweep_stale(&dir, SystemTime::now());
        assert!(ours.exists() && foreign.exists() && lookalike.exists());
        // 假裝過了 25 小時：只刪本模組格式的檔
        sweep_stale(&dir, SystemTime::now() + Duration::from_secs(25 * 60 * 60));
        assert!(!ours.exists());
        assert!(foreign.exists() && lookalike.exists());
        // 資料夾不存在、檔案並發消失都不出錯
        sweep_stale(&dir.join("missing"), SystemTime::now());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn create_fails_cleanly_when_dir_is_a_file() {
        let dir = temp_dir("blocked");
        std::fs::write(&dir, "我是檔案不是資料夾").unwrap();
        assert!(PromptFile::create(&dir, "p.txt", "x").is_err());
        std::fs::remove_file(&dir).unwrap();
    }
}
