//! 版本庫：`app_local_data_dir()/versions/<版本>/`。下載先寫 `.partial-<版本>`，
//! 檔案 fsync 後再把舊目錄改成 `.trash-<版本>`，partial 改成正式目錄，最後 fsync 目錄。

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::verify::{verify_artifact, ReleaseFile};
use crate::ui_msg::UiMsg;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Platform {
    Windows,
    Mac,
}

impl Platform {
    pub(crate) fn current() -> Option<Self> {
        match (std::env::consts::OS, std::env::consts::ARCH) {
            ("windows", "x86_64") => Some(Self::Windows),
            ("macos", "aarch64") => Some(Self::Mac),
            _ => None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Windows => "windows-x86_64",
            Self::Mac => "darwin-aarch64",
        }
    }

    pub(crate) fn suffix_ok(self, name: &str) -> bool {
        match self {
            Self::Windows => name.ends_with("-setup.exe"),
            Self::Mac => name.ends_with(".app.tar.gz"),
        }
    }
}

/// 版本目錄名只能是單一路徑元件。開頭的點留給 `.partial-`／`.trash-`，避免啟動清理把正式目錄掃掉。
pub(crate) fn version_dir_name(version: &str) -> Result<&str, String> {
    if version.is_empty()
        || version == "."
        || version == ".."
        || version.starts_with('.')
        || version.contains('/')
        || version.contains('\\')
        || version.contains('\0')
    {
        return Err(UiMsg::VersionNameInvalid.into());
    }
    Ok(version)
}

pub(crate) fn artifact_name(url: &str) -> Result<String, String> {
    let without_fragment = url.split_once('#').map(|(head, _)| head).unwrap_or(url);
    let without_query = without_fragment
        .split_once('?')
        .map(|(head, _)| head)
        .unwrap_or(without_fragment);
    let segment = without_query.rsplit('/').next().unwrap_or("");
    let decoded = percent_decode(segment)?;
    validate_artifact_name(&decoded)?;
    Ok(decoded)
}

fn validate_artifact_name(name: &str) -> Result<(), String> {
    if !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        return Ok(());
    }
    Err(UiMsg::InstallerNameInvalid.into())
}

fn percent_decode(input: &str) -> Result<String, String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(UiMsg::InstallerNameInvalid.into());
            }
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3])
                .map_err(|_| UiMsg::InstallerNameInvalid.to_string())?;
            let value =
                u8::from_str_radix(hex, 16).map_err(|_| UiMsg::InstallerNameInvalid.to_string())?;
            out.push(value);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).map_err(|_| UiMsg::InstallerNameInvalid.to_string())
}

pub(crate) fn unix_secs_to_rfc3339(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let sod = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = sod / 3600;
    let minute = (sod % 3600) / 60;
    let second = sod % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Howard Hinnant `civil_from_days`。`days` 是從 1970-01-01 起算。
fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let month_part = (5 * doy + 2) / 153;
    let day = doy - (153 * month_part + 2) / 5 + 1;
    let month = if month_part < 10 {
        month_part + 3
    } else {
        month_part - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    (year as i32, month as u32, day as u32)
}

fn now_rfc3339() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    unix_secs_to_rfc3339(seconds)
}

pub(crate) fn clear_version_residue(versions: &Path, version: &str) -> Result<(), String> {
    remove_path(&versions.join(format!(".partial-{version}")))?;
    remove_path(&versions.join(format!(".trash-{version}")))?;
    Ok(())
}

pub(crate) fn clear_all_residue(versions: &Path) -> Result<(), String> {
    if !versions.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(versions).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(".partial-") || name.starts_with(".trash-") {
            remove_path(&entry.path())?;
        }
    }
    Ok(())
}

fn remove_path(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        fs::remove_dir_all(path).map_err(|error| error.to_string())
    } else {
        fs::remove_file(path).map_err(|error| error.to_string())
    }
}

pub(crate) fn reuse_if_valid(
    versions: &Path,
    version: &str,
    file: &str,
    platform: &str,
    pubkey_b64: &str,
) -> Result<bool, String> {
    let dir = versions.join(version);
    if !dir.is_dir() {
        return Ok(false);
    }
    Ok(load_verified(&dir, version, file, platform, pubkey_b64).is_ok())
}

pub(crate) fn reverify_for_install(
    versions: &Path,
    version: &str,
    file: &str,
    platform: &str,
    pubkey_b64: &str,
) -> Result<Vec<u8>, String> {
    let dir = versions.join(version);
    if !dir.is_dir() {
        return Err(UiMsg::UpdateNotDownloaded.into());
    }
    load_verified(&dir, version, file, platform, pubkey_b64)
}

fn load_verified(
    dir: &Path,
    version: &str,
    file: &str,
    platform: &str,
    pubkey_b64: &str,
) -> Result<Vec<u8>, String> {
    let release_path = dir.join("release.json");
    let release_text =
        fs::read_to_string(&release_path).map_err(|_| UiMsg::SignatureInvalid.to_string())?;
    let release: ReleaseFile =
        serde_json::from_str(&release_text).map_err(|_| UiMsg::SignatureInvalid.to_string())?;
    let bytes = fs::read(dir.join(file)).map_err(|_| UiMsg::SignatureInvalid.to_string())?;
    let signature = fs::read_to_string(dir.join(format!("{file}.sig")))
        .map_err(|_| UiMsg::SignatureInvalid.to_string())?;
    verify_artifact(
        &bytes, &signature, pubkey_b64, &release, version, platform, file,
    )?;
    Ok(bytes)
}

pub(crate) fn commit_download(
    versions: &Path,
    version: &str,
    file: &str,
    bytes: &[u8],
    signature: &str,
    platform: &str,
    format_version: Option<u64>,
) -> Result<(), String> {
    let version = version_dir_name(version)?;
    fs::create_dir_all(versions).map_err(|error| error.to_string())?;
    clear_version_residue(versions, version)?;
    let partial = versions.join(format!(".partial-{version}"));
    let trash = versions.join(format!(".trash-{version}"));
    let final_dir = versions.join(version);
    fs::create_dir(&partial).map_err(|error| error.to_string())?;
    let release = ReleaseFile {
        version: version.to_owned(),
        platform: platform.to_owned(),
        file: file.to_owned(),
        format_version,
        size: bytes.len() as u64,
        downloaded_at: now_rfc3339(),
    };
    let body = serde_json::to_vec_pretty(&release).map_err(|error| error.to_string())?;
    if let Err(error) = (|| -> Result<(), String> {
        write_synced(&partial.join(file), bytes)?;
        write_synced(&partial.join(format!("{file}.sig")), signature.as_bytes())?;
        write_synced(&partial.join("release.json"), &body)?;
        if final_dir.exists() {
            fs::rename(&final_dir, &trash).map_err(|error| error.to_string())?;
        }
        if let Err(error) = fs::rename(&partial, &final_dir) {
            if trash.exists() && !final_dir.exists() {
                let _ = fs::rename(&trash, &final_dir);
            }
            return Err(error.to_string());
        }
        sync_dir(versions)?;
        Ok(())
    })() {
        let _ = remove_path(&partial);
        return Err(error);
    }
    if trash.exists() {
        remove_path(&trash)?;
    }
    Ok(())
}

pub(crate) fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = File::create(path).map_err(|error| error.to_string())?;
    file.write_all(bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    Ok(())
}

/// 改寫已存在的紀錄：先寫同目錄暫存、fsync，再原子改名蓋過正式檔，最後 fsync 上層目錄。
/// 改名之前斷電，正式檔維持舊的完整內容。暫存檔名是正式檔名加 `.tmp`，讀取端不看它。
pub(crate) fn replace_synced(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = temp_sibling(path)?;
    write_synced(&tmp, bytes)?;
    #[cfg(test)]
    if FAIL_BEFORE_RENAME.with(|flag| flag.get()) {
        return Err("改寫中斷".to_owned());
    }
    rename_over(&tmp, path)?;
    if let Some(parent) = path.parent() {
        if parent.as_os_str().is_empty() {
            return Ok(());
        }
        sync_dir(parent)?;
    }
    Ok(())
}

fn temp_sibling(path: &Path) -> Result<PathBuf, String> {
    let name = path
        .file_name()
        .ok_or_else(|| "沒有檔名".to_owned())?
        .to_os_string();
    let mut tmp_name = name;
    tmp_name.push(".tmp");
    let parent = path.parent().ok_or_else(|| "沒有上層目錄".to_owned())?;
    Ok(parent.join(tmp_name))
}

/// Windows 的 fs::rename 是 MoveFileExW＋MOVEFILE_REPLACE_EXISTING，兩平台都能直接蓋過。
fn rename_over(from: &Path, to: &Path) -> Result<(), String> {
    fs::rename(from, to).map_err(|error| error.to_string())
}

#[cfg(test)]
thread_local! {
    static FAIL_BEFORE_RENAME: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// 測試用：暫存已 fsync、正式檔還沒被蓋過時失敗。離開作用域就恢復。
#[cfg(test)]
pub(crate) struct FailBeforeRename;

#[cfg(test)]
impl FailBeforeRename {
    pub(crate) fn arm() -> Self {
        FAIL_BEFORE_RENAME.with(|flag| flag.set(true));
        Self
    }
}

#[cfg(test)]
impl Drop for FailBeforeRename {
    fn drop(&mut self) {
        FAIL_BEFORE_RENAME.with(|flag| flag.set(false));
    }
}

/// 目錄 fsync。Windows 開目錄常常失敗；檔案已經各自 sync_all，那裡只記 log、不讓下載因此失敗。
pub(crate) fn sync_dir(path: &Path) -> Result<(), String> {
    match File::open(path).and_then(|file| file.sync_all()) {
        Ok(()) => Ok(()),
        #[cfg(windows)]
        Err(error) => {
            log::warn!("versions 目錄 fsync 失敗（檔案已各自 sync）：{error}");
            Ok(())
        }
        #[cfg(not(windows))]
        Err(error) => Err(UiMsg::VersionsSyncFailed {
            error: error.to_string(),
        }
        .into()),
    }
}

pub(crate) fn versions_root(local_data: &Path) -> PathBuf {
    local_data.join("versions")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updater::verify::sign_fixture;

    fn temp_versions(label: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tt-versions-{label}-{}", ulid::Ulid::generate()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    struct TempDir(PathBuf);
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn artifact_names_follow_the_url_tail_and_the_charset() {
        assert_eq!(
            artifact_name(
                "https://example.test/releases/download/v1/TableTavern_1.2.3_aarch64.app.tar.gz?raw=1#top"
            )
            .unwrap(),
            "TableTavern_1.2.3_aarch64.app.tar.gz"
        );
        assert_eq!(
            artifact_name("https://example.test/a/%2E%2E-setup.exe").unwrap(),
            "..-setup.exe"
        );
        assert!(artifact_name("https://example.test/a/has space-setup.exe").is_err());
        assert!(artifact_name("https://example.test/a/bad%20name-setup.exe").is_err());
        assert!(Platform::Mac.suffix_ok("TableTavern_1.2.3_aarch64.app.tar.gz"));
        assert!(!Platform::Mac.suffix_ok("TableTavern_1.2.3_x64-setup.exe"));
        assert!(Platform::Windows.suffix_ok("..-setup.exe"));
        assert!(version_dir_name("1.2.3").is_ok());
        assert!(version_dir_name("..").is_err());
        assert!(version_dir_name(".partial-1.2.3").is_err());
        assert!(version_dir_name("1.2.3/../x").is_err());
    }

    #[test]
    fn epoch_formats_as_rfc3339_utc() {
        assert_eq!(unix_secs_to_rfc3339(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn reuse_keeps_a_verified_dir_and_replace_drops_the_old_one() {
        let root = TempDir(temp_versions("reuse"));
        let versions = &root.0;
        let bytes = b"good-installer";
        let (public_key, signature) = sign_fixture(bytes);
        let file = "App_1.2.3_aarch64.app.tar.gz";
        commit_download(
            versions,
            "1.2.3",
            file,
            bytes,
            &signature,
            "darwin-aarch64",
            Some(1),
        )
        .unwrap();
        assert!(reuse_if_valid(versions, "1.2.3", file, "darwin-aarch64", &public_key).unwrap());
        assert!(!versions.join(".partial-1.2.3").exists());
        assert!(!versions.join(".trash-1.2.3").exists());

        let replacement = b"replacement-installer";
        let (public_key, signature) = sign_fixture(replacement);
        commit_download(
            versions,
            "1.2.3",
            file,
            replacement,
            &signature,
            "darwin-aarch64",
            None,
        )
        .unwrap();
        assert_eq!(
            fs::read(versions.join("1.2.3").join(file)).unwrap(),
            replacement
        );
        assert!(reuse_if_valid(versions, "1.2.3", file, "darwin-aarch64", &public_key).unwrap());
        let tampered = versions.join("1.2.3").join(file);
        fs::write(&tampered, b"tampered").unwrap();
        assert!(!reuse_if_valid(versions, "1.2.3", file, "darwin-aarch64", &public_key).unwrap());
    }

    #[test]
    fn startup_clears_partial_and_trash_but_keeps_version_dirs() {
        let root = TempDir(temp_versions("residue"));
        let versions = &root.0;
        fs::create_dir_all(versions.join(".partial-1.0.0")).unwrap();
        fs::create_dir_all(versions.join(".trash-1.0.0")).unwrap();
        fs::create_dir_all(versions.join("1.0.0")).unwrap();
        fs::write(versions.join(".partial-1.0.0").join("x"), b"x").unwrap();
        clear_all_residue(versions).unwrap();
        assert!(!versions.join(".partial-1.0.0").exists());
        assert!(!versions.join(".trash-1.0.0").exists());
        assert!(versions.join("1.0.0").is_dir());

        fs::create_dir_all(versions.join(".partial-1.0.0")).unwrap();
        fs::create_dir_all(versions.join(".trash-9.9.9")).unwrap();
        clear_version_residue(versions, "1.0.0").unwrap();
        assert!(!versions.join(".partial-1.0.0").exists());
        assert!(versions.join(".trash-9.9.9").is_dir());
    }
}
