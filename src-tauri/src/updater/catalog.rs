//! 版本清單、可回退條件、自動保留 3 版、逐版刪除。
//! 大小是目錄裡檔案位元組加總（含 `.sig` 與 `release.json`），不是 `release.json` 的 `size` 欄。

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::previous::{self, PreviousState};
use super::semver_util::{is_older, version_ord, versions_equal};
use super::store::{self, sync_dir, version_dir_name, Platform};
use super::store_lock::StoreActivity;
use super::verify::ReleaseFile;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct VersionList {
    pub versions: Vec<VersionRow>,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct VersionRow {
    pub version: String,
    pub size: u64,
    pub format_version: Option<u64>,
    pub usable: bool,
    /// 可用、而且回退的資格判斷（`require_eligible`）也過。
    pub eligible: bool,
    pub current: bool,
    pub previous: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct EligibleRelease {
    pub file: String,
    pub format_version: u64,
}

pub(crate) fn list_versions(
    versions: &Path,
    platform: Platform,
    running: &str,
    pubkey: &str,
) -> Result<VersionList, String> {
    let state = previous::read_previous(versions);
    let mut rows = Vec::new();
    for (name, dir) in version_dirs(versions)? {
        let release = read_release(&dir);
        let usable = release.as_ref().is_some_and(|release| {
            release_usable(release, &name, platform)
                && store::reverify_for_install(
                    versions,
                    &name,
                    &release.file,
                    platform.as_str(),
                    pubkey,
                )
                .is_ok()
        });
        let eligible = usable && require_eligible(versions, &name, platform, running).is_ok();
        rows.push(VersionRow {
            eligible,
            current: versions_equal(&name, running),
            previous: usable && confirmed_previous(&state, &name),
            format_version: release.as_ref().and_then(|release| release.format_version),
            size: dir_bytes(&dir),
            usable,
            version: name,
        });
    }
    rows.sort_by(|left, right| cmp_version_desc(&left.version, &right.version));
    let total_bytes = rows.iter().map(|row| row.size).sum();
    Ok(VersionList {
        versions: rows,
        total_bytes,
    })
}

fn confirmed_previous(state: &PreviousState, version: &str) -> bool {
    !state.unreadable
        && state
            .previous
            .as_deref()
            .is_some_and(|previous| versions_equal(previous, version))
}

pub(crate) fn require_eligible(
    versions: &Path,
    version: &str,
    platform: Platform,
    running: &str,
) -> Result<EligibleRelease, String> {
    let name = version_dir_name(version)?;
    let eligible = load_usable(versions, name, platform)?;
    if !is_older(name, running) {
        return Err("不能回退到同版或較新的版本".to_owned());
    }
    Ok(eligible)
}

fn load_usable(versions: &Path, name: &str, platform: Platform) -> Result<EligibleRelease, String> {
    let dir = versions.join(name);
    let release = read_release(&dir).ok_or_else(|| "這個版本不能回退".to_owned())?;
    if release.platform != platform.as_str() {
        return Err("平台不符".to_owned());
    }
    let Some(format_version) = release.format_version else {
        return Err("沒有格式版本".to_owned());
    };
    if !versions_equal(&release.version, name) || !dir.join(&release.file).is_file() {
        return Err("這個版本不能回退".to_owned());
    }
    Ok(EligibleRelease {
        file: release.file,
        format_version,
    })
}

/// 救援副本能不能刪：該版在版本庫、平台相符、有格式版本、檔案在，而且重驗過。
/// 不要求比執行中舊——副本本身可能就是目前這版。
pub(crate) fn has_verified_rollback_point(
    versions: &Path,
    version: &str,
    platform: Platform,
    pubkey: &str,
) -> bool {
    let Ok(name) = version_dir_name(version) else {
        return false;
    };
    let Ok(eligible) = load_usable(versions, name, platform) else {
        return false;
    };
    store::reverify_for_install(versions, name, &eligible.file, platform.as_str(), pubkey).is_ok()
}

pub(crate) fn prune_versions(
    versions: &Path,
    running: &str,
    extra_keep: &[String],
) -> Result<(), String> {
    let state = previous::read_previous(versions);
    let mut keep = Vec::new();
    keep.push(running.to_owned());
    if !state.unreadable {
        if let Some(previous) = state.previous.clone() {
            keep.push(previous);
        }
        if let Some(from) = state.pending_from.clone() {
            keep.push(from);
        }
        if let Some(to) = state.pending_to.clone() {
            keep.push(to);
        }
    }
    keep.extend(extra_keep.iter().cloned());

    let dirs = version_dirs(versions)?;
    let mut protected = Vec::new();
    let mut rest = Vec::new();
    for (name, dir) in dirs {
        if keep.iter().any(|item| versions_equal(item, &name)) {
            protected.push((name, dir));
        } else {
            rest.push((name, dir));
        }
    }
    rest.sort_by(|left, right| cmp_version_desc(&left.0, &right.0));
    let room = 3usize.saturating_sub(protected.len());
    for (name, dir) in rest.into_iter().skip(room) {
        remove_version_dir(versions, &dir, &name)?;
    }
    Ok(())
}

pub(crate) fn delete_version_dir(
    versions: &Path,
    version: &str,
    running: &str,
    activity: &StoreActivity,
) -> Result<(), String> {
    let name = version_dir_name(version)?.to_owned();
    if versions_equal(&name, running) {
        return Err("不能刪除目前版本".to_owned());
    }
    if activity.blocks(&name) {
        return Err("這個版本正在下載或安裝".to_owned());
    }
    let dir = versions.join(&name);
    if !dir.is_dir() {
        return Err("沒有這個版本".to_owned());
    }
    remove_version_dir(versions, &dir, &name)?;
    previous::clear_previous_if(versions, &name)?;
    Ok(())
}

fn remove_version_dir(versions: &Path, dir: &Path, name: &str) -> Result<(), String> {
    let expected = versions.join(name);
    if dir != expected.as_path() {
        return Err("沒有這個版本".to_owned());
    }
    fs::remove_dir_all(dir).map_err(|error| error.to_string())?;
    sync_dir(versions)?;
    Ok(())
}

fn release_usable(release: &ReleaseFile, dir_name: &str, platform: Platform) -> bool {
    versions_equal(&release.version, dir_name)
        && release.platform == platform.as_str()
        && release.format_version.is_some()
}

fn read_release(dir: &Path) -> Option<ReleaseFile> {
    let text = fs::read_to_string(dir.join("release.json")).ok()?;
    serde_json::from_str(&text).ok()
}

fn version_dirs(versions: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    if !versions.exists() {
        return Ok(Vec::new());
    }
    let mut found = Vec::new();
    for entry in fs::read_dir(versions).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if version_dir_name(name).is_err() {
            continue;
        }
        found.push((name.to_owned(), entry.path()));
    }
    Ok(found)
}

fn dir_bytes(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    let mut total = 0u64;
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            total = total.saturating_add(dir_bytes(&path));
        } else {
            total = total.saturating_add(meta.len());
        }
    }
    total
}

/// 解析不了的排最後（修剪時先被刪）。其餘 SemVer 由新到舊。
fn cmp_version_desc(left: &str, right: &str) -> std::cmp::Ordering {
    match (
        version_ord(left, right),
        semver::Version::parse(left),
        semver::Version::parse(right),
    ) {
        (Some(order), _, _) => order.reverse(),
        (_, Ok(_), Err(_)) => std::cmp::Ordering::Less,
        (_, Err(_), Ok(_)) => std::cmp::Ordering::Greater,
        _ => right.cmp(left),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    struct Signer {
        public: String,
        pair: minisign::KeyPair,
    }

    impl Signer {
        fn generate() -> Self {
            let pair = minisign::KeyPair::generate_unencrypted_keypair().expect("keypair");
            let public_key = pair.pk.to_box().expect("public box").into_string();
            Self {
                public: base64::engine::general_purpose::STANDARD.encode(public_key),
                pair,
            }
        }

        fn sign(&self, bytes: &[u8]) -> String {
            let signature = minisign::sign(
                Some(&self.pair.pk),
                &self.pair.sk,
                std::io::Cursor::new(bytes),
                None,
                None,
            )
            .expect("sign");
            base64::engine::general_purpose::STANDARD.encode(signature.into_string())
        }
    }

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(label: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("tt-cat-{label}-{}", ulid::Ulid::generate()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn put(
        versions: &Path,
        version: &str,
        platform: &str,
        format_version: Option<u64>,
        pubkey_out: &mut String,
    ) {
        let key = Signer::generate();
        *pubkey_out = key.public.clone();
        put_signed(versions, version, platform, format_version, &key);
    }

    fn put_signed(
        versions: &Path,
        version: &str,
        platform: &str,
        format_version: Option<u64>,
        key: &Signer,
    ) {
        let bytes = format!("installer-{version}").into_bytes();
        let signature = key.sign(&bytes);
        let file = match platform {
            "windows-x86_64" => format!("TableTavern_{version}_x64-setup.exe"),
            _ => format!("TableTavern_{version}_aarch64.app.tar.gz"),
        };
        store::commit_download(
            versions,
            version,
            &file,
            &bytes,
            &signature,
            platform,
            format_version,
        )
        .unwrap();
    }

    #[test]
    fn eligibility_rejects_missing_format_wrong_platform_and_not_older() {
        let root = TempDir::new("elig");
        let mut key = String::new();
        put(&root.0, "0.1.0", "darwin-aarch64", None, &mut key);
        put(&root.0, "0.2.0", "windows-x86_64", Some(1), &mut key);
        put(&root.0, "0.3.0", "darwin-aarch64", Some(1), &mut key);
        put(&root.0, "0.4.0", "darwin-aarch64", Some(1), &mut key);
        assert_eq!(
            require_eligible(&root.0, "0.1.0", Platform::Mac, "0.4.0").unwrap_err(),
            "沒有格式版本"
        );
        assert_eq!(
            require_eligible(&root.0, "0.2.0", Platform::Mac, "0.4.0").unwrap_err(),
            "平台不符"
        );
        assert_eq!(
            require_eligible(&root.0, "0.4.0", Platform::Mac, "0.4.0").unwrap_err(),
            "不能回退到同版或較新的版本"
        );
        assert_eq!(
            require_eligible(&root.0, "0.4.0", Platform::Mac, "0.3.0").unwrap_err(),
            "不能回退到同版或較新的版本"
        );
        let ok = require_eligible(&root.0, "0.3.0", Platform::Mac, "0.4.0").unwrap();
        assert_eq!(ok.format_version, 1);
    }

    #[test]
    fn list_marks_only_the_confirmed_usable_previous() {
        let root = TempDir::new("list");
        let key = Signer::generate();
        put_signed(&root.0, "0.8.0", "darwin-aarch64", Some(1), &key);
        put_signed(&root.0, "0.9.0", "darwin-aarch64", Some(1), &key);
        put_signed(&root.0, "1.0.0", "darwin-aarch64", Some(1), &key);
        fs::create_dir_all(root.0.join("junk")).unwrap();
        fs::write(root.0.join("junk").join("a"), b"abcd").unwrap();
        previous::write_pending(&root.0, "1.0.0", "1.1.0").unwrap();
        // 待確認不是上一版。已確認的 0.8 不是 SemVer 上緊鄰的 0.9。
        let file = root.0.join("previous.json");
        let mut state: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        state["previous"] = serde_json::json!("0.8.0");
        fs::write(&file, serde_json::to_vec_pretty(&state).unwrap()).unwrap();

        let list = list_versions(&root.0, Platform::Mac, "1.0.0", &key.public).unwrap();
        let previous: Vec<_> = list
            .versions
            .iter()
            .filter(|row| row.previous)
            .map(|row| row.version.as_str())
            .collect();
        assert_eq!(previous, ["0.8.0"]);
        let junk = list
            .versions
            .iter()
            .find(|row| row.version == "junk")
            .unwrap();
        assert!(!junk.usable);
        assert_eq!(junk.size, 4);
        assert!(list
            .versions
            .iter()
            .any(|row| row.version == "1.0.0" && row.current));
        assert!(list.total_bytes >= 4);
    }

    #[test]
    fn missing_signature_or_a_changed_installer_is_not_usable() {
        let root = TempDir::new("bad-sig");
        let key = Signer::generate();
        put_signed(&root.0, "0.8.0", "darwin-aarch64", Some(1), &key);
        put_signed(&root.0, "0.9.0", "darwin-aarch64", Some(1), &key);
        put_signed(&root.0, "1.0.0", "darwin-aarch64", Some(1), &key);
        let path = root.0.join("previous.json");
        previous::write_pending(&root.0, "1.0.0", "1.1.0").unwrap();
        let mut state: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        state["previous"] = serde_json::json!("0.8.0");
        fs::write(&path, serde_json::to_vec_pretty(&state).unwrap()).unwrap();

        fs::remove_file(
            root.0
                .join("0.8.0")
                .join("TableTavern_0.8.0_aarch64.app.tar.gz.sig"),
        )
        .unwrap();
        fs::write(
            root.0
                .join("0.9.0")
                .join("TableTavern_0.9.0_aarch64.app.tar.gz"),
            b"tampered",
        )
        .unwrap();

        let list = list_versions(&root.0, Platform::Mac, "1.0.0", &key.public).unwrap();
        let row = |version: &str| {
            list.versions
                .iter()
                .find(|row| row.version == version)
                .unwrap()
                .clone()
        };
        let missing_sig = row("0.8.0");
        assert!(!missing_sig.usable);
        assert!(!missing_sig.previous);
        let tampered = row("0.9.0");
        assert!(!tampered.usable);
        assert!(!tampered.previous);
        assert!(row("1.0.0").usable);
    }

    #[test]
    fn eligible_means_usable_and_older_than_running() {
        let root = TempDir::new("eligible");
        let key = Signer::generate();
        put_signed(&root.0, "0.8.0", "darwin-aarch64", Some(1), &key);
        put_signed(&root.0, "0.9.0", "darwin-aarch64", Some(1), &key);
        put_signed(&root.0, "0.7.0", "darwin-aarch64", None, &key);
        put_signed(&root.0, "1.0.0", "darwin-aarch64", Some(1), &key);
        put_signed(&root.0, "1.1.0", "darwin-aarch64", Some(1), &key);
        fs::write(
            root.0
                .join("0.9.0")
                .join("TableTavern_0.9.0_aarch64.app.tar.gz"),
            b"tampered",
        )
        .unwrap();
        fs::create_dir_all(root.0.join("junk")).unwrap();

        let list = list_versions(&root.0, Platform::Mac, "1.0.0", &key.public).unwrap();
        let eligible: Vec<_> = list
            .versions
            .iter()
            .filter(|row| row.eligible)
            .map(|row| row.version.as_str())
            .collect();
        assert_eq!(eligible, ["0.8.0"]);
        let newer = list
            .versions
            .iter()
            .find(|row| row.version == "1.1.0")
            .unwrap();
        assert!(newer.usable && !newer.eligible, "較新的版本可用但不能回退");
        let current = list.versions.iter().find(|row| row.current).unwrap();
        assert!(current.usable && !current.eligible);
    }

    #[test]
    fn prune_keeps_protected_versions_then_fills_to_three() {
        let root = TempDir::new("prune");
        let mut key = String::new();
        for version in [
            "0.1.0", "0.2.0", "0.3.0", "0.4.0", "0.9.0", "0.10.0", "1.0.0",
        ] {
            put(&root.0, version, "darwin-aarch64", Some(1), &mut key);
        }
        previous::write_pending(&root.0, "1.0.0", "1.2.0").unwrap();
        let file = root.0.join("previous.json");
        let mut state: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        state["previous"] = serde_json::json!("0.2.0");
        fs::write(&file, serde_json::to_vec_pretty(&state).unwrap()).unwrap();
        // 目前 1.0.0、上一版 0.2.0、待安裝 0.4.0 已滿 3，其餘刪。0.10 比 0.9 新也不留。
        prune_versions(&root.0, "1.0.0", &["0.4.0".to_owned()]).unwrap();
        let names = names_of(&root.0);
        assert_eq!(names, ["0.2.0", "0.4.0", "1.0.0"]);

        for version in ["0.5.0", "0.6.0", "0.7.0"] {
            put(&root.0, version, "darwin-aarch64", Some(1), &mut key);
        }
        fs::remove_file(&file).unwrap();
        prune_versions(&root.0, "1.0.0", &[]).unwrap();
        let names = names_of(&root.0);
        assert_eq!(names, ["0.6.0", "0.7.0", "1.0.0"]);
        assert!(!names.contains(&"0.9.0".to_owned()));
    }

    #[test]
    fn prune_keeps_0_10_ahead_of_0_9() {
        let root = TempDir::new("semver");
        let mut key = String::new();
        for version in ["0.1.0", "0.9.0", "0.10.0", "1.0.0"] {
            put(&root.0, version, "darwin-aarch64", Some(1), &mut key);
        }
        previous::write_pending(&root.0, "0.1.0", "1.1.0").unwrap();
        prune_versions(&root.0, "1.0.0", &[]).unwrap();
        let names = names_of(&root.0);
        assert!(names.contains(&"0.10.0".to_owned()));
        assert!(names.contains(&"0.1.0".to_owned()));
        assert!(names.contains(&"1.0.0".to_owned()));
        assert!(!names.contains(&"0.9.0".to_owned()));
    }

    #[test]
    fn prune_keeps_more_than_three_when_protection_requires_it() {
        let root = TempDir::new("protect");
        let mut key = String::new();
        for version in ["0.1.0", "0.2.0", "0.3.0", "0.4.0", "1.0.0"] {
            put(&root.0, version, "darwin-aarch64", Some(1), &mut key);
        }
        fs::create_dir_all(root.0.join("not-semver")).unwrap();
        previous::write_pending(&root.0, "0.3.0", "0.4.0").unwrap();
        let file = root.0.join("previous.json");
        let mut state: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        state["previous"] = serde_json::json!("0.2.0");
        fs::write(&file, serde_json::to_vec_pretty(&state).unwrap()).unwrap();
        prune_versions(&root.0, "1.0.0", &["0.4.0".to_owned()]).unwrap();
        let names = names_of(&root.0);
        assert!(names.contains(&"0.2.0".to_owned()));
        assert!(names.contains(&"0.3.0".to_owned()));
        assert!(names.contains(&"0.4.0".to_owned()));
        assert!(names.contains(&"1.0.0".to_owned()));
        assert!(!names.contains(&"0.1.0".to_owned()));
        assert!(!names.contains(&"not-semver".to_owned()));
        assert!(names.len() > 3);
    }

    #[test]
    fn delete_refuses_current_and_busy_and_clears_a_matching_previous() {
        let root = TempDir::new("del");
        let mut key = String::new();
        put(&root.0, "0.2.0", "darwin-aarch64", Some(1), &mut key);
        put(&root.0, "1.0.0", "darwin-aarch64", Some(1), &mut key);
        let file = previous::read_previous(&root.0);
        let _ = file;
        crate::updater::previous::write_pending(&root.0, "1.0.0", "1.1.0").unwrap();
        let path = root.0.join("previous.json");
        let mut state: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        state["previous"] = serde_json::json!("0.2.0");
        fs::write(&path, serde_json::to_vec_pretty(&state).unwrap()).unwrap();

        let idle = StoreActivity::default();
        assert_eq!(
            delete_version_dir(&root.0, "1.0.0", "1.0.0", &idle).unwrap_err(),
            "不能刪除目前版本"
        );
        let busy = StoreActivity {
            downloading: Some("0.2.0".to_owned()),
            installing: None,
        };
        assert_eq!(
            delete_version_dir(&root.0, "0.2.0", "1.0.0", &busy).unwrap_err(),
            "這個版本正在下載或安裝"
        );
        delete_version_dir(&root.0, "0.2.0", "1.0.0", &idle).unwrap();
        assert!(!root.0.join("0.2.0").exists());
        assert!(previous::read_previous(&root.0).previous.is_none());
        assert_eq!(
            previous::read_previous(&root.0).pending_to.as_deref(),
            Some("1.1.0")
        );
    }

    #[test]
    fn verified_rollback_point_needs_a_real_signature() {
        let root = TempDir::new("point");
        let mut key = String::new();
        put(&root.0, "0.2.0", "darwin-aarch64", Some(1), &mut key);
        assert!(has_verified_rollback_point(
            &root.0,
            "0.2.0",
            Platform::Mac,
            &key
        ));
        fs::write(
            root.0
                .join("0.2.0")
                .join("TableTavern_0.2.0_aarch64.app.tar.gz"),
            b"tampered",
        )
        .unwrap();
        assert!(!has_verified_rollback_point(
            &root.0,
            "0.2.0",
            Platform::Mac,
            &key
        ));
    }

    fn names_of(versions: &Path) -> Vec<String> {
        let mut names = version_dirs(versions).unwrap();
        names.sort_by(|left, right| left.0.cmp(&right.0));
        names.into_iter().map(|(name, _)| name).collect()
    }
}
