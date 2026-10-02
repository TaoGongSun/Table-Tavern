//! 啟動後：確認 previous.json、清掉上次回退留下的 `.launch/`、Mac 再整理救援副本。
//! 任一步失敗都記 log 並繼續，最後把第一個錯誤交回去。前端本來就吞掉這個結果。

use std::path::Path;

use super::launch;
use super::previous;
use super::residue::{self, CleanupMode};

pub(crate) async fn settle_launch_locked(
    versions: &Path,
    running: &str,
    mac_bundle: Option<&Path>,
    rollback_ready: impl Fn(&str) -> bool,
    swap: impl Fn(&Path, &Path) -> Result<(), String>,
) -> Result<(), String> {
    let _guard = super::store_lock::store_activity().lock().await;
    settle_launch(versions, running, mac_bundle, rollback_ready, swap)
}

pub(crate) fn settle_launch(
    versions: &Path,
    running: &str,
    mac_bundle: Option<&Path>,
    rollback_ready: impl Fn(&str) -> bool,
    swap: impl Fn(&Path, &Path) -> Result<(), String>,
) -> Result<(), String> {
    let mut first: Option<String> = None;
    if let Err(error) = previous::confirm_on_launch(versions, running) {
        log::warn!("確認上一版失敗：{error}");
        first = Some(error);
    }
    if let Err(error) = launch::clear_launch_dir(versions) {
        log::warn!("清 .launch 失敗：{error}");
        if first.is_none() {
            first = Some(error);
        }
    }
    if let Some(bundle) = mac_bundle {
        if let Err(error) =
            residue::cleanup_residue(bundle, CleanupMode::AfterLaunch, rollback_ready, swap)
        {
            log::warn!("啟動後整理失敗：{error}");
            if first.is_none() {
                first = Some(error);
            }
        }
    }
    match first {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updater::launch::{stage_launch_installer, LAUNCH_DIR};
    use crate::updater::previous::{self, read_previous};

    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("tt-launch2-{}", ulid::Ulid::generate()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn new_version_confirms_previous_and_clears_the_launch_dir() {
        let root = TempDir::new();
        previous::write_pending(&root.0, "0.1.0", "0.2.0").unwrap();
        stage_launch_installer(&root.0, "TableTavern_0.1.0_x64-setup.exe", b"exe").unwrap();
        settle_launch(&root.0, "0.2.0", None, |_| false, |_, _| Ok(())).unwrap();
        let state = read_previous(&root.0);
        assert_eq!(state.previous.as_deref(), Some("0.1.0"));
        assert!(state.pending_from.is_none());
        assert!(!root.0.join(LAUNCH_DIR).exists());
    }

    #[test]
    fn a_failed_install_clears_pending_and_keeps_the_old_previous() {
        let root = TempDir::new();
        previous::write_pending(&root.0, "0.2.0", "0.3.0").unwrap();
        let path = root.0.join("previous.json");
        let mut value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        value["previous"] = serde_json::json!("0.1.0");
        std::fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        settle_launch(&root.0, "0.2.0", None, |_| false, |_, _| Ok(())).unwrap();
        let state = read_previous(&root.0);
        assert_eq!(state.previous.as_deref(), Some("0.1.0"));
        assert!(state.pending_from.is_none());
    }

    #[test]
    fn running_the_confirmed_previous_clears_it() {
        let root = TempDir::new();
        previous::write_pending(&root.0, "0.9.0", "1.0.0").unwrap();
        settle_launch(&root.0, "1.0.0", None, |_| false, |_, _| Ok(())).unwrap();
        settle_launch(&root.0, "0.9.0", None, |_| false, |_, _| Ok(())).unwrap();
        let state = read_previous(&root.0);
        assert!(state.previous.is_none());
        assert!(state.pending_from.is_none());
    }
}
