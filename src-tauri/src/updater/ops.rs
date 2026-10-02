//! 版本庫鎖包住清單與刪版。修剪在下載提交裡拿同一把鎖。呼叫端不要在已經拿著這把鎖時再進來。

use std::path::Path;

use super::catalog::{self, VersionList};
use super::store::Platform;
use super::store_lock::store_activity;

pub(crate) async fn list_versions_locked(
    versions: &Path,
    platform: Platform,
    running: &str,
    pubkey: &str,
) -> Result<VersionList, String> {
    let _guard = store_activity().lock().await;
    catalog::list_versions(versions, platform, running, pubkey)
}

pub(crate) async fn delete_version_locked(
    versions: &Path,
    version: &str,
    running: &str,
) -> Result<(), String> {
    let activity = store_activity().lock().await;
    catalog::delete_version_dir(versions, version, running, &activity)
}

#[cfg(test)]
mod tests {
    use super::super::rollback_point::{store_downloaded_release, NewRelease};
    use super::*;
    use crate::updater::store_lock::{self, test_serial, with_installing};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("tt-ops-{}", ulid::Ulid::generate()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test]
    async fn delete_prune_and_rollback_wait_for_the_same_lock() {
        let _serial = test_serial();
        let root = TempDir::new();
        std::fs::create_dir_all(root.0.join("0.1.0")).unwrap();
        let finished = Arc::new(AtomicUsize::new(0));
        let hold = store_activity().lock().await;

        let spawn = |fut: std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>| {
            let finished = Arc::clone(&finished);
            tokio::spawn(async move {
                fut.await;
                finished.fetch_add(1, Ordering::SeqCst);
            })
        };
        let versions = root.0.clone();
        spawn(Box::pin(async move {
            delete_version_locked(&versions, "0.1.0", "9.9.9")
                .await
                .unwrap();
        }));
        let versions = root.0.clone();
        spawn(Box::pin(async move {
            let ready = store_downloaded_release(
                &versions,
                "0.2.0",
                Platform::Mac,
                Some(NewRelease {
                    file: "TableTavern_0.2.0_aarch64.app.tar.gz",
                    bytes: b"exe",
                    signature: "sig",
                    format_version: Some(1),
                }),
                "0.1.0",
                "pubkey",
                1,
                "",
                |_| async { Err("不上網".to_owned()) },
            )
            .await
            .unwrap();
            assert!(!ready);
        }));
        let versions = root.0.clone();
        spawn(Box::pin(async move {
            list_versions_locked(&versions, Platform::Mac, "0.1.0", "")
                .await
                .unwrap();
        }));
        spawn(Box::pin(async move {
            let error = with_installing("0.1.0", async { Err::<(), String>("停".to_owned()) })
                .await
                .unwrap_err();
            assert_eq!(error, "停");
        }));

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(
            finished.load(Ordering::SeqCst),
            0,
            "鎖還在手上，四件事都不該做完"
        );
        assert!(root.0.join("0.1.0").is_dir());
        drop(hold);
        for _ in 0..50 {
            if finished.load(Ordering::SeqCst) == 4 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(finished.load(Ordering::SeqCst), 4);
        assert!(
            store_lock::store_activity()
                .lock()
                .await
                .installing
                .is_none(),
            "失敗的回退要把安裝旗標清掉"
        );
    }

    #[tokio::test]
    async fn delete_rejects_the_version_being_downloaded() {
        let _serial = test_serial();
        let root = TempDir::new();
        std::fs::create_dir_all(root.0.join("0.1.0")).unwrap();
        store_lock::mark_downloading("0.1.0").await.unwrap();
        let error = delete_version_locked(&root.0, "0.1.0", "9.9.9")
            .await
            .unwrap_err();
        store_lock::clear_downloading().await;
        assert_eq!(error, "這個版本正在下載或安裝");
        assert!(root.0.join("0.1.0").is_dir());
    }
}
