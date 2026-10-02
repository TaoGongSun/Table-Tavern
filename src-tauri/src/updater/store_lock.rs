//! 版本庫操作鎖。下載提交、修剪、刪版、回退從重驗到安裝啟動，都拿這一把。
//! `downloading`／`installing` 記「哪一版正忙」。下載的網路階段會放開鎖、留下旗標，
//! 刪版才能立刻拒絕那一版；提交與回退則把鎖持有到做完。

use std::sync::OnceLock;

use tokio::sync::Mutex;

use super::semver_util::versions_equal;

#[derive(Debug, Default)]
pub(crate) struct StoreActivity {
    pub downloading: Option<String>,
    pub installing: Option<String>,
}

impl StoreActivity {
    pub(crate) fn is_busy(&self) -> bool {
        self.downloading.is_some() || self.installing.is_some()
    }

    pub(crate) fn blocks(&self, version: &str) -> bool {
        self.downloading
            .as_deref()
            .is_some_and(|current| versions_equal(current, version))
            || self
                .installing
                .as_deref()
                .is_some_and(|current| versions_equal(current, version))
    }
}

pub(crate) fn store_activity() -> &'static Mutex<StoreActivity> {
    static LOCK: OnceLock<Mutex<StoreActivity>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(StoreActivity::default()))
}

pub(crate) async fn mark_downloading(version: &str) -> Result<(), String> {
    let mut activity = store_activity().lock().await;
    if activity.is_busy() {
        return Err("已在安裝或下載".to_owned());
    }
    activity.downloading = Some(version.to_owned());
    Ok(())
}

pub(crate) async fn clear_downloading() {
    let mut activity = store_activity().lock().await;
    activity.downloading = None;
}

/// 版本庫鎖的測試不要跟別的測試同時改旗標。正式程式不走這裡。
#[cfg(test)]
pub(crate) fn test_serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poison| poison.into_inner())
}

/// 持有鎖直到 `body` 結束。失敗才清掉安裝旗標；成功留給程序退出（`exit`／`restart` 不跑 Drop）。
pub(crate) async fn with_installing<T>(
    version: &str,
    body: impl std::future::Future<Output = Result<T, String>>,
) -> Result<T, String> {
    let mut activity = store_activity().lock().await;
    if activity.is_busy() {
        return Err("已在安裝或下載".to_owned());
    }
    activity.installing = Some(version.to_owned());
    let result = body.await;
    if result.is_err() {
        activity.installing = None;
    }
    result
}
