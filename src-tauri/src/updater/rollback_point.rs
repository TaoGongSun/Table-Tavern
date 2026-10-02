//! 回退點：把「目前這一版」補進版本庫。失敗只記 log，不擋剛完成的下載。
//! 網址由 conf `endpoints` 的 repo 與包 1 的固定檔名推出。測試注入 `fetch`，不連 GitHub。

use std::future::Future;
use std::path::Path;

use super::catalog;
use super::store::{self, artifact_name, Platform};
use super::verify::verify_artifact;

pub(crate) fn rollback_file_name(version: &str, platform: Platform) -> Result<String, String> {
    let name = match platform {
        Platform::Windows => format!("TableTavern_{version}_x64-setup.exe"),
        Platform::Mac => format!("TableTavern_{version}_aarch64.app.tar.gz"),
    };
    artifact_name(&format!("https://example.test/{name}"))?;
    if !platform.suffix_ok(&name) {
        return Err("安裝檔副檔名不符合這個平台".to_owned());
    }
    Ok(name)
}

/// `https://github.com/<owner>/<repo>/releases/...` → 該版安裝檔與 `.sig`。
pub(crate) fn rollback_asset_urls(
    endpoint: &str,
    version: &str,
    file_name: &str,
) -> Result<(String, String), String> {
    let marker = "/releases/";
    let index = endpoint
        .find(marker)
        .ok_or_else(|| "更新位址無法推出 repo".to_owned())?;
    let base = &endpoint[..index];
    if !base.starts_with("https://") || base.ends_with('/') {
        return Err("更新位址無法推出 repo".to_owned());
    }
    let asset = format!("{base}/releases/download/v{version}/{file_name}");
    Ok((asset.clone(), format!("{asset}.sig")))
}

pub(crate) fn bundled_endpoints() -> &'static [String] {
    use std::sync::OnceLock;
    static ENDPOINTS: OnceLock<Vec<String>> = OnceLock::new();
    ENDPOINTS
        .get_or_init(|| {
            let conf: serde_json::Value =
                serde_json::from_str(include_str!("../../tauri.conf.json"))
                    .expect("tauri.conf.json 必須能解析");
            conf["plugins"]["updater"]["endpoints"]
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|value| value.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default()
        })
        .as_slice()
}

pub(crate) fn rollback_point_endpoint() -> String {
    endpoint_for_rollback(bundled_endpoints())
        .unwrap_or("")
        .to_owned()
}

pub(crate) fn endpoint_for_rollback(endpoints: &[String]) -> Result<&str, String> {
    endpoints
        .iter()
        .find(|endpoint| endpoint.contains("/releases/"))
        .map(String::as_str)
        .ok_or_else(|| "更新位址無法推出 repo".to_owned())
}

/// 下載前在鎖內看能不能沿用。不能沿用就把該版的半成品清掉，網路階段再放開鎖。
pub(crate) async fn prepare_download_reuse(
    versions: &Path,
    version: &str,
    file: &str,
    platform: &str,
    pubkey: &str,
) -> Result<bool, String> {
    let guard = super::store_lock::store_activity().lock().await;
    let reused = store::reuse_if_valid(versions, version, file, platform, pubkey)?;
    if !reused {
        store::clear_version_residue(versions, version)?;
    }
    drop(guard);
    Ok(reused)
}

pub(crate) struct NewRelease<'a> {
    pub file: &'a str,
    pub bytes: &'a [u8],
    pub signature: &'a str,
    pub format_version: Option<u64>,
}

/// 剛下載的那一版在鎖內提交並修剪。`new_release` 為 None 表示這次是沿用，不再寫一次。
/// 兩種情況最後都補回退點。回退點失敗變成 `Ok(false)`，提交失敗才是 `Err`。
pub(crate) async fn store_downloaded_release<F, Fut>(
    versions: &Path,
    downloaded: &str,
    platform: Platform,
    new_release: Option<NewRelease<'_>>,
    running: &str,
    pubkey: &str,
    current_format: u64,
    endpoint: &str,
    fetch: F,
) -> Result<bool, String>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, String>>,
{
    if let Some(release) = &new_release {
        let _guard = super::store_lock::store_activity().lock().await;
        store::commit_download(
            versions,
            downloaded,
            release.file,
            release.bytes,
            release.signature,
            platform.as_str(),
            release.format_version,
        )?;
        prune_quietly(versions, running, &[downloaded.to_owned()]);
    }
    let keep = [downloaded.to_owned()];
    Ok(rollback_ready_of(
        ensure_rollback_point(
            versions,
            running,
            platform,
            pubkey,
            current_format,
            endpoint,
            &keep,
            fetch,
        )
        .await,
    ))
}

/// 已在且重驗通過就沿用。否則下載、驗簽、照包 3 寫進版本庫。
/// 重驗與提交拿版本庫鎖；下載那段放開。`format_version` 用呼叫端傳入的本版 `CURRENT_FORMAT`。
pub(crate) async fn ensure_rollback_point<F, Fut>(
    versions: &Path,
    running: &str,
    platform: Platform,
    pubkey: &str,
    format_version: u64,
    endpoint: &str,
    extra_keep: &[String],
    fetch: F,
) -> Result<(), String>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, String>>,
{
    let file = rollback_file_name(running, platform)?;
    {
        let _guard = super::store_lock::store_activity().lock().await;
        if store::reuse_if_valid(versions, running, &file, platform.as_str(), pubkey)? {
            return Ok(());
        }
    }
    let (asset, sig_url) = rollback_asset_urls(endpoint, running, &file)?;
    let bytes = fetch(asset).await?;
    let signature = fetch(sig_url).await?;
    let signature = String::from_utf8(signature).map_err(|_| "驗簽失敗".to_owned())?;
    let signature = signature.trim().to_owned();
    let release = super::verify::ReleaseFile {
        version: running.to_owned(),
        platform: platform.as_str().to_owned(),
        file: file.clone(),
        format_version: Some(format_version),
        size: bytes.len() as u64,
        downloaded_at: String::new(),
    };
    verify_artifact(
        &bytes,
        &signature,
        pubkey,
        &release,
        running,
        platform.as_str(),
        &file,
    )?;
    let _guard = super::store_lock::store_activity().lock().await;
    if store::reuse_if_valid(versions, running, &file, platform.as_str(), pubkey)? {
        return Ok(());
    }
    store::commit_download(
        versions,
        running,
        &file,
        &bytes,
        &signature,
        platform.as_str(),
        Some(format_version),
    )?;
    prune_quietly(versions, running, extra_keep);
    Ok(())
}

/// 下載本身已經成功。回退點失敗不往外傳。
pub(crate) fn rollback_ready_of(result: Result<(), String>) -> bool {
    if let Err(error) = &result {
        log::warn!("回退點沒有補上：{error}");
    }
    result.is_ok()
}

/// 版本庫新增一版之後呼叫。失敗只記 log。`extra_keep` 含待安裝或正在回退的版本。
pub(crate) fn prune_quietly(versions: &Path, running: &str, extra_keep: &[String]) {
    if let Err(error) = catalog::prune_versions(versions, running, extra_keep) {
        log::warn!("版本庫修剪失敗：{error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updater::verify::sign_fixture;
    use base64::Engine;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new(label: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("tt-point-{label}-{}", ulid::Ulid::generate()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn endpoint() -> &'static str {
        "https://github.com/TaoGongSun/Table-Tavern/releases/latest/download/latest.json"
    }

    #[test]
    fn urls_follow_the_endpoint_repo_and_package_filenames() {
        let (asset, sig) = rollback_asset_urls(
            endpoint(),
            "0.2.0",
            &rollback_file_name("0.2.0", Platform::Mac).unwrap(),
        )
        .unwrap();
        assert_eq!(
            asset,
            "https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.2.0/TableTavern_0.2.0_aarch64.app.tar.gz"
        );
        assert_eq!(sig, format!("{asset}.sig"));
        let exe = rollback_file_name("0.2.0", Platform::Windows).unwrap();
        assert!(exe.ends_with("_x64-setup.exe"));
        let (from_conf, _) = rollback_asset_urls(
            endpoint_for_rollback(bundled_endpoints()).unwrap(),
            "0.2.0",
            &rollback_file_name("0.2.0", Platform::Mac).unwrap(),
        )
        .unwrap();
        assert_eq!(from_conf, asset);
    }

    #[tokio::test]
    async fn fresh_fill_reuse_failure_and_a_later_retry() {
        let root = TempDir::new("fill");
        let bytes = b"current-installer".to_vec();
        let (public_key, signature) = sign_fixture(&bytes);
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_fetch = Arc::clone(&calls);
        let signature_fetch = signature.clone();
        let fetch = move |url: String| {
            let calls_fetch = Arc::clone(&calls_fetch);
            let signature_fetch = signature_fetch.clone();
            let bytes = bytes.clone();
            async move {
                calls_fetch.fetch_add(1, Ordering::SeqCst);
                if url.ends_with(".sig") {
                    Ok(signature_fetch.into_bytes())
                } else {
                    Ok(bytes)
                }
            }
        };
        ensure_rollback_point(
            &root.0,
            "0.2.0",
            Platform::Mac,
            &public_key,
            1,
            endpoint(),
            &[],
            fetch,
        )
        .await
        .unwrap();
        assert!(root.0.join("0.2.0").is_dir());
        assert!(calls.load(Ordering::SeqCst) >= 2);

        let calls = Arc::new(AtomicUsize::new(0));
        let calls_fetch = Arc::clone(&calls);
        let reuse = ensure_rollback_point(
            &root.0,
            "0.2.0",
            Platform::Mac,
            &public_key,
            1,
            endpoint(),
            &[],
            move |_url: String| {
                let calls_fetch = Arc::clone(&calls_fetch);
                async move {
                    calls_fetch.fetch_add(1, Ordering::SeqCst);
                    Err("不該再下載".to_owned())
                }
            },
        )
        .await;
        assert!(reuse.is_ok(), "沿用重驗通過就不再下載");
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        let empty = TempDir::new("miss");
        let missed = ensure_rollback_point(
            &empty.0,
            "0.2.0",
            Platform::Mac,
            &public_key,
            1,
            endpoint(),
            &[],
            |_url: String| async { Err("連不上".to_owned()) },
        )
        .await;
        assert!(missed.is_err());
        assert!(!rollback_ready_of(missed));
        assert!(!empty.0.join("0.2.0").exists());

        let bytes = b"current-installer".to_vec();
        let signature = signature.clone();
        let retried = ensure_rollback_point(
            &empty.0,
            "0.2.0",
            Platform::Mac,
            &public_key,
            1,
            endpoint(),
            &[],
            move |url: String| {
                let signature = signature.clone();
                let bytes = bytes.clone();
                async move {
                    if url.ends_with(".sig") {
                        Ok(signature.into_bytes())
                    } else {
                        Ok(bytes)
                    }
                }
            },
        )
        .await;
        assert!(rollback_ready_of(retried));
        assert!(empty.0.join("0.2.0").is_dir());
    }

    fn sign_with(pair: &minisign::KeyPair, bytes: &[u8]) -> (String, String) {
        let signature = minisign::sign(
            Some(&pair.pk),
            &pair.sk,
            std::io::Cursor::new(bytes),
            None,
            None,
        )
        .expect("sign");
        let public_key = pair.pk.to_box().expect("public box").into_string();
        let engine = base64::engine::general_purpose::STANDARD;
        (
            engine.encode(public_key),
            engine.encode(signature.into_string()),
        )
    }

    /// 剛下載與沿用都補回退點。拿不到不擋，下一輪再補，剛放進去的新版還在。
    #[tokio::test]
    async fn fresh_download_and_reuse_both_fill_the_rollback_point() {
        let _serial = super::super::store_lock::test_serial();
        let pair = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
        let root = TempDir::new("both");
        let new_bytes = b"installer-0.3.0".to_vec();
        let old_bytes = b"installer-0.2.0".to_vec();
        let (public_key, new_sig) = sign_with(&pair, &new_bytes);
        let (_, old_sig) = sign_with(&pair, &old_bytes);
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_fetch = Arc::clone(&calls);
        let old_bytes_fetch = old_bytes.clone();
        let old_sig_fetch = old_sig.clone();
        let ready = store_downloaded_release(
            &root.0,
            "0.3.0",
            Platform::Mac,
            Some(NewRelease {
                file: "TableTavern_0.3.0_aarch64.app.tar.gz",
                bytes: &new_bytes,
                signature: &new_sig,
                format_version: Some(1),
            }),
            "0.2.0",
            &public_key,
            1,
            endpoint(),
            move |url: String| {
                let calls_fetch = Arc::clone(&calls_fetch);
                let old_bytes_fetch = old_bytes_fetch.clone();
                let old_sig_fetch = old_sig_fetch.clone();
                async move {
                    calls_fetch.fetch_add(1, Ordering::SeqCst);
                    if url.ends_with(".sig") {
                        Ok(old_sig_fetch.into_bytes())
                    } else {
                        Ok(old_bytes_fetch)
                    }
                }
            },
        )
        .await
        .unwrap();
        assert!(ready);
        assert!(calls.load(Ordering::SeqCst) >= 2);
        assert!(root.0.join("0.3.0").is_dir());
        assert!(root.0.join("0.2.0").is_dir());

        let calls = Arc::new(AtomicUsize::new(0));
        let calls_fetch = Arc::clone(&calls);
        let again = store_downloaded_release(
            &root.0,
            "0.3.0",
            Platform::Mac,
            None,
            "0.2.0",
            &public_key,
            1,
            endpoint(),
            move |_url: String| {
                let calls_fetch = Arc::clone(&calls_fetch);
                async move {
                    calls_fetch.fetch_add(1, Ordering::SeqCst);
                    Err("沿用不該再下載".to_owned())
                }
            },
        )
        .await
        .unwrap();
        assert!(again, "沿用新版時仍要確認回退點");
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        let missed = TempDir::new("keep");
        let kept = b"installer-0.4.0".to_vec();
        let (_, kept_sig) = sign_with(&pair, &kept);
        let blocked = store_downloaded_release(
            &missed.0,
            "0.4.0",
            Platform::Mac,
            Some(NewRelease {
                file: "TableTavern_0.4.0_aarch64.app.tar.gz",
                bytes: &kept,
                signature: &kept_sig,
                format_version: Some(1),
            }),
            "0.2.0",
            &public_key,
            1,
            endpoint(),
            |_url: String| async { Err("連不上".to_owned()) },
        )
        .await
        .unwrap();
        assert!(!blocked);
        assert!(
            missed.0.join("0.4.0").is_dir(),
            "回退點失敗不刪剛下載的版本"
        );
        assert!(!missed.0.join("0.2.0").exists());

        let ready = store_downloaded_release(
            &missed.0,
            "0.4.0",
            Platform::Mac,
            None,
            "0.2.0",
            &public_key,
            1,
            endpoint(),
            {
                let old_bytes = old_bytes.clone();
                let old_sig = old_sig.clone();
                move |url: String| {
                    let old_bytes = old_bytes.clone();
                    let old_sig = old_sig.clone();
                    async move {
                        if url.ends_with(".sig") {
                            Ok(old_sig.into_bytes())
                        } else {
                            Ok(old_bytes)
                        }
                    }
                }
            },
        )
        .await
        .unwrap();
        assert!(ready);
        assert!(missed.0.join("0.2.0").is_dir());
        assert!(missed.0.join("0.4.0").is_dir());
    }
}
