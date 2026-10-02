//! 回退預覽。目標不合格（不在、驗不過、不比目前舊、平台不符）回錯誤；
//! 掃桌用包 2 的格式判讀，不跑恢復，掃不了就回成功並標 `scan_failed`。沒有主資料夾的桌略過。

use std::path::Path;

use serde::Serialize;

use super::catalog::require_eligible;
use super::store::{reverify_for_install, version_dir_name, Platform};
use crate::data::{self, FormatVersion};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct PreviewWorld {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RollbackPreview {
    pub will_be_readonly: Vec<PreviewWorld>,
    pub maybe_readonly: Vec<PreviewWorld>,
    pub scan_failed: bool,
}

/// 可回退條件在版本庫鎖內讀完就放開，接著才掃桌目錄。
pub(crate) async fn rollback_preview_locked(
    config_root: &Path,
    versions: &Path,
    version: &str,
    platform: Platform,
    running: &str,
    pubkey: &str,
) -> Result<RollbackPreview, String> {
    let format_version = {
        let _guard = super::store_lock::store_activity().lock().await;
        let eligible = require_eligible(versions, version, platform, running)?;
        reverify_for_install(
            versions,
            version_dir_name(version)?,
            &eligible.file,
            platform.as_str(),
            pubkey,
        )?;
        eligible.format_version
    };
    Ok(
        worlds_at(config_root, format_version).unwrap_or_else(|error| {
            log::warn!("回退預覽掃桌失敗：{error}");
            RollbackPreview {
                will_be_readonly: Vec::new(),
                maybe_readonly: Vec::new(),
                scan_failed: true,
            }
        }),
    )
}

fn worlds_at(root: &Path, target_format: u64) -> Result<RollbackPreview, String> {
    let mut will_be_readonly = Vec::new();
    let mut maybe_readonly = Vec::new();
    let ids = data::discover_ids(root).map_err(|error| error.to_string())?;
    for id in ids {
        let live = data::live_dir(root, &id);
        if !live.is_dir() {
            continue;
        }
        let row = PreviewWorld {
            name: data::loose_name(&live, &id),
            id,
        };
        match data::read_format(&live).version {
            FormatVersion::Known(version) if version > target_format => {
                will_be_readonly.push(row);
            }
            FormatVersion::Unknown => maybe_readonly.push(row),
            FormatVersion::Known(_) => {}
        }
    }
    will_be_readonly.sort_by(|left, right| left.id.cmp(&right.id));
    maybe_readonly.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(RollbackPreview {
        will_be_readonly,
        maybe_readonly,
        scan_failed: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_msg::UiMsg;
    use crate::updater::store::{self, Platform};
    use crate::updater::verify::sign_fixture;

    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new(label: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("tt-prev-{label}-{}", ulid::Ulid::generate()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// 回傳簽這一版的公鑰。
    fn put_release(
        versions: &std::path::Path,
        version: &str,
        format_version: Option<u64>,
    ) -> String {
        let bytes = format!("bytes-{version}").into_bytes();
        let (public_key, signature) = sign_fixture(&bytes);
        store::commit_download(
            versions,
            version,
            &format!("TableTavern_{version}_aarch64.app.tar.gz"),
            &bytes,
            &signature,
            "darwin-aarch64",
            format_version,
        )
        .unwrap();
        public_key
    }

    fn world(root: &std::path::Path, name: &str) -> String {
        let id = ulid::Ulid::generate().to_string();
        let dir = root.join("worlds").join(&id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("state.json"),
            format!(r#"{{"id":"{id}","name":"{name}"}}"#),
        )
        .unwrap();
        id
    }

    #[tokio::test]
    async fn preview_splits_newer_formats_and_unknown_and_skips_worlds_without_a_live_dir() {
        let versions = TempDir::new("ver");
        let desks = TempDir::new("desks");
        let key = put_release(&versions.0, "0.1.0", Some(1));
        let newer = world(&desks.0, "新格式");
        std::fs::write(
            desks.0.join("worlds").join(&newer).join("format.json"),
            r#"{"format_version":3}"#,
        )
        .unwrap();
        let same = world(&desks.0, "同格式");
        std::fs::write(
            desks.0.join("worlds").join(&same).join("format.json"),
            r#"{"format_version":1}"#,
        )
        .unwrap();
        let missing_marker = world(&desks.0, "霧港");
        let unknown = world(&desks.0, "讀不懂");
        std::fs::write(
            desks.0.join("worlds").join(&unknown).join("format.json"),
            r#"{"format_version":"nope"}"#,
        )
        .unwrap();
        let broken_state = world(&desks.0, "壞狀態");
        std::fs::remove_file(
            desks
                .0
                .join("worlds")
                .join(&broken_state)
                .join("state.json"),
        )
        .unwrap();
        std::fs::write(
            desks
                .0
                .join("worlds")
                .join(&broken_state)
                .join("state.json"),
            "{",
        )
        .unwrap();
        let sidecar = ulid::Ulid::generate().to_string();
        std::fs::create_dir_all(desks.0.join("worlds").join(format!(".tt-pre-{sidecar}"))).unwrap();

        let preview =
            rollback_preview_locked(&desks.0, &versions.0, "0.1.0", Platform::Mac, "0.2.0", &key)
                .await
                .unwrap();
        assert!(!preview.scan_failed);
        assert_eq!(
            preview
                .will_be_readonly
                .iter()
                .map(|row| row.name.as_str())
                .collect::<Vec<_>>(),
            ["新格式"]
        );
        let maybe: Vec<_> = preview
            .maybe_readonly
            .iter()
            .map(|row| row.id.as_str())
            .collect();
        assert!(preview
            .maybe_readonly
            .iter()
            .any(|row| row.name == "讀不懂"));
        assert!(
            maybe.contains(&broken_state.as_str()),
            "缺標記且 state 解不開算版本不明"
        );
        assert!(
            !maybe.contains(&missing_marker.as_str()),
            "缺標記但 state 解得開是格式 1"
        );
        assert!(!preview
            .will_be_readonly
            .iter()
            .chain(preview.maybe_readonly.iter())
            .any(|row| row.id == same || row.id == sidecar));
        assert_eq!(preview.will_be_readonly[0].id, newer);
    }

    #[tokio::test]
    async fn ineligible_or_unverified_targets_are_errors() {
        let versions = TempDir::new("bad");
        let desks = TempDir::new("empty");
        let key = put_release(&versions.0, "0.1.0", None);
        let other = put_release(&versions.0, "0.3.0", Some(1));
        let preview = |version: &'static str, platform, running: &'static str, pubkey: String| {
            let versions = versions.0.clone();
            let desks = desks.0.clone();
            async move {
                rollback_preview_locked(&desks, &versions, version, platform, running, &pubkey)
                    .await
                    .unwrap_err()
            }
        };
        assert_eq!(
            preview("0.1.0", Platform::Mac, "0.4.0", key.clone()).await,
            UiMsg::RollbackNoFormat.to_string()
        );
        assert_eq!(
            preview("0.3.0", Platform::Windows, "0.4.0", other.clone()).await,
            UiMsg::RollbackPlatformMismatch.to_string()
        );
        assert_eq!(
            preview("0.3.0", Platform::Mac, "0.3.0", other.clone()).await,
            UiMsg::RollbackNotOlder.to_string()
        );
        assert_eq!(
            preview("9.9.9", Platform::Mac, "0.4.0", other.clone()).await,
            UiMsg::RollbackNotEligible.to_string()
        );
        assert_eq!(
            preview("0.3.0", Platform::Mac, "0.4.0", key).await,
            UiMsg::SignatureInvalid.to_string(),
            "別把公鑰對不上的版本當成可回退"
        );
    }

    #[tokio::test]
    async fn a_failed_world_scan_is_success_with_scan_failed() {
        let versions = TempDir::new("scan");
        let desks = TempDir::new("scan-desks");
        let key = put_release(&versions.0, "0.1.0", Some(1));
        // `worlds` 是檔案不是目錄：讀不了桌清單。
        std::fs::write(desks.0.join("worlds"), b"x").unwrap();
        let preview =
            rollback_preview_locked(&desks.0, &versions.0, "0.1.0", Platform::Mac, "0.2.0", &key)
                .await
                .unwrap();
        assert!(preview.scan_failed);
        assert!(preview.will_be_readonly.is_empty());
        assert!(preview.maybe_readonly.is_empty());
    }
}
