//! 桌面更新的領域邏輯。command 只負責把 Tauri 的參數交進來。
//! 版本庫、驗簽、提醒等級、Mac 替換都在這裡，不在 `commands/`。

mod catalog;
mod install_prep;
mod launch;
mod level;
pub(crate) mod macos;
mod ops;
mod post_launch;
mod preview;
mod previous;
mod residue;
mod rollback_point;
mod semver_util;
mod slot;
mod store;
mod store_lock;
mod verify;

use std::sync::OnceLock;

use serde::Serialize;
use serde_json::Value;

pub(crate) use level::{
    allow_auto_check, is_skipped, reminder_level, remote_format, ReminderLevel,
};
#[cfg(target_os = "macos")]
pub(crate) use macos::swap_directories;
#[cfg(target_os = "macos")]
pub(crate) use macos::{parent_is_writable, replace_installed_app};
// 這句只在非 Mac、非 Windows 的安裝路徑用到。本機是 Mac，不標 allow 會被當成沒人用。
pub(crate) use catalog::{has_verified_rollback_point, require_eligible, VersionList};
pub(crate) use install_prep::{forward_before_raise, rollback_before_raise};
#[allow(unused_imports)]
pub(crate) use launch::stage_launch_installer;
#[cfg(target_os = "windows")]
pub(crate) use launch::start_nsis_installer;
#[allow(unused_imports)]
pub(crate) use macos::CANNOT_REPLACE;
pub(crate) use ops::{delete_version_locked, list_versions_locked};
pub(crate) use post_launch::settle_launch_locked;
pub(crate) use preview::{rollback_preview_locked, RollbackPreview};
pub(crate) use rollback_point::{
    prepare_download_reuse, rollback_point_endpoint, store_downloaded_release, NewRelease,
};
pub(crate) use slot::UpdateSlot;
pub(crate) use store::{
    artifact_name, clear_all_residue, reverify_for_install, version_dir_name, versions_root,
    Platform,
};
pub(crate) use store_lock::{clear_downloading, mark_downloading, with_installing};

/// 最近一次檢查留下的 plugin `Update`，以及下載／安裝進行到哪。
/// 下載或安裝還沒結束時，後來的檢查不得覆寫。
pub struct PendingUpdate {
    pub inner: tokio::sync::Mutex<UpdateSlot<tauri_plugin_updater::Update>>,
}

impl Default for PendingUpdate {
    fn default() -> Self {
        Self {
            inner: tokio::sync::Mutex::new(UpdateSlot::default()),
        }
    }
}

pub(crate) fn bundled_pubkey() -> &'static str {
    static KEY: OnceLock<String> = OnceLock::new();
    KEY.get_or_init(|| {
        let conf: Value = serde_json::from_str(include_str!("../../tauri.conf.json"))
            .expect("tauri.conf.json 必須能解析");
        conf["plugins"]["updater"]["pubkey"]
            .as_str()
            .expect("tauri.conf.json 缺少 updater.pubkey")
            .to_owned()
    })
    .as_str()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DownloadResult {
    pub version: String,
    pub rollback_ready: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UpdateOffer {
    pub version: String,
    pub current_version: String,
    pub notes: Option<String>,
    pub pub_date: Option<String>,
    pub level: ReminderLevel,
    pub skipped: bool,
}
