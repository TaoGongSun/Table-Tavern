//! 測試用：把目錄設成 0o000 測「查不到在不在」的錯誤路徑。

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// 離開範圍（含 panic）就把目錄權限還原成 0o755。
struct Restore(PathBuf);

impl Drop for Restore {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o755));
    }
}

/// `dir` 設成 0o000 後跑 `work`，結束（含 panic）一律還原權限。0o000 擋不住對 `probe` 的查詢
/// （以 root 執行）就 eprintln 原因後略過，回 None。
pub(crate) fn with_unsearchable<T>(
    dir: &Path,
    probe: &Path,
    work: impl FnOnce() -> T,
) -> Option<T> {
    let _restore = Restore(dir.to_path_buf());
    fs::set_permissions(dir, fs::Permissions::from_mode(0o000)).unwrap();
    match probe.try_exists() {
        Err(_) => Some(work()),
        Ok(true) => {
            eprintln!("略過：0o000 擋不住查詢（以 root 執行），測不出查詢錯誤");
            None
        }
        Ok(false) => panic!("檔案在，查詢卻回不在"),
    }
}
