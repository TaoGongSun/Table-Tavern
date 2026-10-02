//! Mac 替換自身。解壓、對調、`renamex_np` 在這裡。殘留怎麼接續在 `residue`。
//! 對調與「上層可寫」由呼叫端注入，測試不 chmod、也不叫 renamex_np。

use std::fs::{self, File};
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use super::residue::{self, CleanupMode};
use super::semver_util::versions_equal;

pub(crate) const BUNDLE_ID: &str = "com.tabletavern.app";
pub(crate) const APP_NAME: &str = "Table Tavern.app";
pub(crate) const UPDATE_NAME: &str = ".TableTavern-update.app";
pub(crate) const PREVIOUS_NAME: &str = "Table Tavern (previous).app";
pub(crate) const CANNOT_REPLACE: &str = "無法自動替換";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BundleInfo {
    pub version: Option<String>,
    pub bundle_id: Option<String>,
    pub plist_readable: bool,
}

pub(crate) fn read_bundle_info(app: &Path) -> BundleInfo {
    let Ok(text) = fs::read_to_string(app.join("Contents/Info.plist")) else {
        return BundleInfo {
            version: None,
            bundle_id: None,
            plist_readable: false,
        };
    };
    BundleInfo {
        version: plist_string(&text, "CFBundleShortVersionString"),
        bundle_id: plist_string(&text, "CFBundleIdentifier"),
        plist_readable: true,
    }
}

fn plist_string(xml: &str, key: &str) -> Option<String> {
    let needle = format!("<key>{key}</key>");
    let index = xml.find(&needle)?;
    let rest = &xml[index + needle.len()..];
    let start = rest.find("<string>")? + "<string>".len();
    let end = rest[start..].find("</string>")?;
    let value = rest[start..start + end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_owned())
    }
}

pub(crate) fn is_translocated(path: &Path) -> bool {
    path.components()
        .any(|component| component.as_os_str() == "AppTranslocation")
        || path.to_string_lossy().contains("/AppTranslocation/")
}

pub(crate) fn is_official_bundle(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some(APP_NAME) && !is_translocated(path)
}

pub(crate) fn same_bundle(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

/// 上層可寫：在父目錄建一個探針檔再刪掉。測試不走這條，改注入結果。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn parent_is_writable(parent: &Path) -> bool {
    let probe = parent.join(".tt-update-probe");
    match File::create(&probe) {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// `touch` 的等價：打開 `.app` 目錄改 mtime。不呼叫 touch 執行檔。
pub(crate) fn touch_app(app: &Path) -> io::Result<()> {
    let file = File::open(app)?;
    file.set_modified(SystemTime::now())
}

/// 對調成功才回 Ok。在那之前的任何失敗都還沒換上，呼叫端要放閘。
/// 改名成 previous 失敗仍回 Ok：舊版留在 `.TableTavern-update.app`，交給下次整理。
pub(crate) fn replace_installed_app(
    running_app: &Path,
    bytes: &[u8],
    version: &str,
    parent_writable: impl Fn(&Path) -> bool,
    swap: impl Fn(&Path, &Path) -> Result<(), String>,
) -> Result<(), String> {
    if is_translocated(running_app) || !is_official_bundle(running_app) {
        return Err(CANNOT_REPLACE.to_owned());
    }
    let Some(parent) = running_app.parent() else {
        return Err(CANNOT_REPLACE.to_owned());
    };
    if !parent_writable(parent) {
        return Err(CANNOT_REPLACE.to_owned());
    }
    if read_bundle_info(running_app).bundle_id.as_deref() != Some(BUNDLE_ID) {
        return Err(CANNOT_REPLACE.to_owned());
    }
    let report =
        residue::cleanup_residue(running_app, CleanupMode::BeforeExtract, |_| false, &swap)?;
    if report.update_remains {
        return Err(CANNOT_REPLACE.to_owned());
    }
    let Some(from) = read_bundle_info(running_app).version else {
        return Err(CANNOT_REPLACE.to_owned());
    };
    let update_app = parent.join(UPDATE_NAME);
    if let Err(error) = residue::write_record(parent, &from, version, residue::Stage::Extracting) {
        return Err(error);
    }
    if let Err(error) = extract_new_app(&update_app, bytes, version) {
        let _ = fs::remove_dir_all(&update_app);
        let _ = residue::delete_record(parent);
        return Err(error);
    }
    if swap(&update_app, running_app).is_err() {
        let _ = fs::remove_dir_all(&update_app);
        let _ = residue::delete_record(parent);
        return Err(CANNOT_REPLACE.to_owned());
    }
    // 不歸點。下面失敗不再放閘：新版已經在原路徑上。
    // 紀錄仍是 extracting。這裡再跑一次整理：主對調已完成，會先改成 swapped 再放進 previous。
    if let Err(error) =
        residue::cleanup_residue(running_app, CleanupMode::BeforeExtract, |_| false, &swap)
    {
        log::warn!("舊版留在 {UPDATE_NAME}：{error}");
    }
    if let Err(error) = touch_app(running_app) {
        log::warn!("touch 新版失敗，仍會重啟：{error}");
    }
    Ok(())
}

fn extract_new_app(dest: &Path, bytes: &[u8], version: &str) -> Result<(), String> {
    extract_stripped(bytes, dest)?;
    let info = read_bundle_info(dest);
    let Some(found) = info.version else {
        return Err("安裝檔版本不符".to_owned());
    };
    if !versions_equal(&found, version) {
        return Err("安裝檔版本不符".to_owned());
    }
    Ok(())
}

fn extract_stripped(bytes: &[u8], dest: &Path) -> Result<(), String> {
    let decoder = flate2::read::GzDecoder::new(std::io::Cursor::new(bytes));
    let mut archive = tar::Archive::new(decoder);
    fs::create_dir_all(dest).map_err(|error| error.to_string())?;
    let entries = archive.entries().map_err(|error| error.to_string())?;
    for entry in entries {
        let mut entry = entry.map_err(|error| error.to_string())?;
        let kind = entry.header().entry_type();
        let path = entry
            .path()
            .map_err(|error| error.to_string())?
            .into_owned();
        let relative = strip_first(&path)?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let out = dest.join(&relative);
        if !out.starts_with(dest) {
            return Err("壓縮檔路徑不安全".to_owned());
        }
        let mode = entry.header().mode().map_err(|error| error.to_string())?;
        if kind.is_hard_link() {
            return Err("壓縮檔含連結".to_owned());
        }
        if kind.is_symlink() {
            let target = entry
                .link_name()
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "壓縮檔含連結".to_owned())?;
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            // 先確認目標還在 dest 裡，被拒的連結不落盤。
            place_symlink(dest, &out, target.as_ref())?;
            continue;
        }
        if kind.is_dir() {
            fs::create_dir_all(&out).map_err(|error| error.to_string())?;
            set_install_mode(&out, mode)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut file = File::create(&out).map_err(|error| error.to_string())?;
        io::copy(&mut entry, &mut file).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        drop(file);
        set_install_mode(&out, mode)?;
    }
    Ok(())
}

/// GNU tar 常把檔案類型放在高位（例如 `0100755`）。只留 `0o777`：執行位元留著，
/// setuid、setgid、sticky 拿掉。
fn install_mode(mode: u32) -> u32 {
    mode & 0o777
}

fn set_install_mode(path: &Path, mode: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(install_mode(mode));
        fs::set_permissions(path, permissions).map_err(|error| error.to_string())?;
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
    }
    Ok(())
}

fn place_symlink(dest: &Path, link_path: &Path, target: &Path) -> Result<(), String> {
    if !symlink_stays_inside(dest, link_path, target) {
        return Err("壓縮檔含連結".to_owned());
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link_path).map_err(|error| error.to_string())
    }
    #[cfg(not(unix))]
    {
        let _ = target;
        Err("壓縮檔含連結".to_owned())
    }
}

/// 目標照 tar 的相對路徑，從連結所在目錄往上算。算完仍在 dest 裡才准。
/// 不跟著已經存在的連結走，避免一條鏈把目標帶出目錄。
fn symlink_stays_inside(dest: &Path, link_path: &Path, target: &Path) -> bool {
    if target.as_os_str().is_empty() || target.is_absolute() {
        return false;
    }
    let Some(parent) = link_path.parent() else {
        return false;
    };
    let Ok(base) = parent.strip_prefix(dest) else {
        return false;
    };
    let mut depth = 0usize;
    for component in base.components().chain(target.components()) {
        match component {
            Component::CurDir => {}
            Component::Normal(_) => depth += 1,
            Component::ParentDir => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
            }
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

fn strip_first(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Err("壓縮檔路徑不安全".to_owned());
    }
    let mut components = path.components();
    let Some(first) = components.next() else {
        return Err("壓縮檔路徑不安全".to_owned());
    };
    if first.as_os_str() != APP_NAME {
        return Err("壓縮檔第一層不是 Table Tavern.app".to_owned());
    }
    let mut relative = PathBuf::new();
    for component in components {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            _ => return Err("壓縮檔路徑不安全".to_owned()),
        }
    }
    Ok(relative)
}

#[cfg(target_os = "macos")]
pub(crate) fn swap_directories(new_app: &Path, old_app: &Path) -> Result<(), String> {
    use std::ffi::CString;
    use std::os::raw::{c_char, c_int, c_uint};
    use std::os::unix::ffi::OsStrExt;

    extern "C" {
        fn renamex_np(from: *const c_char, to: *const c_char, flags: c_uint) -> c_int;
    }
    const RENAME_SWAP: c_uint = 0x2;
    let from =
        CString::new(new_app.as_os_str().as_bytes()).map_err(|_| CANNOT_REPLACE.to_owned())?;
    let to = CString::new(old_app.as_os_str().as_bytes()).map_err(|_| CANNOT_REPLACE.to_owned())?;
    let rc = unsafe { renamex_np(from.as_ptr(), to.as_ptr(), RENAME_SWAP) };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("tt-mac-{label}-{}", ulid::Ulid::generate()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_app(dir: &Path, version: &str, bundle_id: &str) {
        let plist_dir = dir.join("Contents");
        fs::create_dir_all(&plist_dir).unwrap();
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>{bundle_id}</string>
<key>CFBundleShortVersionString</key><string>{version}</string>
</dict></plist>
"#
        );
        fs::write(plist_dir.join("Info.plist"), plist).unwrap();
    }

    fn app_tar(version: &str) -> Vec<u8> {
        let plist = format!(
            "<plist><dict><key>CFBundleIdentifier</key><string>{BUNDLE_ID}</string>\
<key>CFBundleShortVersionString</key><string>{version}</string></dict></plist>"
        );
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_size(plist.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(
                &mut header,
                "Table Tavern.app/Contents/Info.plist",
                plist.as_bytes(),
            )
            .unwrap();
        let raw = builder.into_inner().unwrap();
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gzip.write_all(&raw).unwrap();
        gzip.finish().unwrap()
    }

    fn fake_swap(new_app: &Path, old_app: &Path) -> Result<(), String> {
        let parent = old_app.parent().unwrap();
        let holding = parent.join(".tt-swap-holding");
        fs::rename(old_app, &holding).map_err(|error| error.to_string())?;
        fs::rename(new_app, old_app).map_err(|error| error.to_string())?;
        fs::rename(&holding, new_app).map_err(|error| error.to_string())?;
        Ok(())
    }

    fn version_of(app: &Path) -> Option<String> {
        read_bundle_info(app).version
    }

    #[test]
    fn extract_strips_the_first_component_and_checks_the_version() {
        let root = TempDir::new("extract");
        let dest = root.0.join(UPDATE_NAME);
        extract_new_app(&dest, &app_tar("1.4.0"), "1.4.0").unwrap();
        assert!(dest.join("Contents/Info.plist").is_file());
        assert!(!dest.join(APP_NAME).exists());
        let mismatch = root.0.join("mismatch.app");
        let error = extract_new_app(&mismatch, &app_tar("1.4.0"), "2.0.0").unwrap_err();
        assert_eq!(error, "安裝檔版本不符");
    }

    #[test]
    fn unsupported_swap_and_unwritable_parent_leave_the_old_app() {
        let root = TempDir::new("bail");
        let running = root.0.join(APP_NAME);
        write_app(&running, "0.2.0", BUNDLE_ID);
        let error =
            replace_installed_app(&running, &app_tar("0.3.0"), "0.3.0", |_| false, fake_swap)
                .unwrap_err();
        assert_eq!(error, CANNOT_REPLACE);
        assert!(!root.0.join(UPDATE_NAME).exists());
        assert_eq!(version_of(&running).as_deref(), Some("0.2.0"));

        let error = replace_installed_app(
            &running,
            &app_tar("0.3.0"),
            "0.3.0",
            |_| true,
            |_new, _old| Err("swap unsupported".to_owned()),
        )
        .unwrap_err();
        assert_eq!(error, CANNOT_REPLACE);
        assert!(!root.0.join(UPDATE_NAME).exists());
        assert_eq!(version_of(&running).as_deref(), Some("0.2.0"));
    }

    #[test]
    fn translocation_and_a_non_app_path_change_nothing() {
        let root = TempDir::new("translocate");
        let nested = root.0.join("AppTranslocation").join("ABC").join(APP_NAME);
        write_app(&nested, "0.2.0", BUNDLE_ID);
        let error = replace_installed_app(&nested, &app_tar("0.3.0"), "0.3.0", |_| true, fake_swap)
            .unwrap_err();
        assert_eq!(error, CANNOT_REPLACE);
        assert_eq!(version_of(&nested).as_deref(), Some("0.2.0"));
        residue::cleanup_residue(&nested, CleanupMode::AfterLaunch, |_| true, fake_swap).unwrap();
        assert!(nested.join("Contents/Info.plist").is_file());

        let dev = root.0.join("table-tavern");
        fs::write(&dev, b"exe").unwrap();
        assert_eq!(
            replace_installed_app(&dev, &app_tar("0.3.0"), "0.3.0", |_| true, fake_swap)
                .unwrap_err(),
            CANNOT_REPLACE
        );
    }

    #[test]
    fn successful_swap_renames_the_old_version_to_previous() {
        let root = TempDir::new("swap");
        let running = root.0.join(APP_NAME);
        write_app(&running, "0.2.0", BUNDLE_ID);
        replace_installed_app(&running, &app_tar("0.3.0"), "0.3.0", |_| true, fake_swap).unwrap();
        assert_eq!(version_of(&running).as_deref(), Some("0.3.0"));
        assert_eq!(
            version_of(&root.0.join(PREVIOUS_NAME)).as_deref(),
            Some("0.2.0")
        );
        assert!(!root.0.join(UPDATE_NAME).exists());
        assert!(!root.0.join(residue::RECORD_NAME).exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn renamex_np_swaps_two_temp_directories() {
        let root = TempDir::new("renamex");
        let left = root.0.join("left.app");
        let right = root.0.join("right.app");
        fs::create_dir_all(left.join("Contents")).unwrap();
        fs::create_dir_all(right.join("Contents")).unwrap();
        fs::write(left.join("Contents/mark"), b"left").unwrap();
        fs::write(right.join("Contents/mark"), b"right").unwrap();
        swap_directories(&left, &right).unwrap();
        assert_eq!(fs::read(left.join("Contents/mark")).unwrap(), b"right");
        assert_eq!(fs::read(right.join("Contents/mark")).unwrap(), b"left");
    }

    fn finish_gz(builder: tar::Builder<Vec<u8>>) -> Vec<u8> {
        let raw = builder.into_inner().unwrap();
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gzip.write_all(&raw).unwrap();
        gzip.finish().unwrap()
    }

    fn append_file(builder: &mut tar::Builder<Vec<u8>>, path: &str, mode: u32, body: &[u8]) {
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(mode);
        header.set_cksum();
        builder.append_data(&mut header, path, body).unwrap();
    }

    fn append_link(builder: &mut tar::Builder<Vec<u8>>, path: &str, target: &str) {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        header.set_mode(0o777);
        builder.append_link(&mut header, path, target).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn extract_restores_mode_and_keeps_only_internal_relative_symlinks() {
        use std::os::unix::fs::PermissionsExt;

        let mut builder = tar::Builder::new(Vec::new());
        append_file(
            &mut builder,
            "Table Tavern.app/Contents/MacOS/table-tavern",
            0o100755,
            b"bin",
        );
        append_file(
            &mut builder,
            "Table Tavern.app/Contents/Info.plist",
            0o100644,
            b"plist",
        );
        append_file(
            &mut builder,
            "Table Tavern.app/Contents/MacOS/setuid-bin",
            0o104755,
            b"s",
        );
        append_link(
            &mut builder,
            "Table Tavern.app/Contents/Resources/helper",
            "../MacOS/table-tavern",
        );
        let bytes = finish_gz(builder);

        let root = TempDir::new("modes");
        let dest = root.0.join("app");
        extract_stripped(&bytes, &dest).unwrap();
        let perms = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o7777;
        let exe = dest.join("Contents/MacOS/table-tavern");
        assert_eq!(perms(&exe), 0o755);
        assert_ne!(perms(&exe) & 0o111, 0);
        assert_eq!(perms(&dest.join("Contents/Info.plist")), 0o644);
        assert_eq!(perms(&dest.join("Contents/MacOS/setuid-bin")), 0o755);
        let link = dest.join("Contents/Resources/helper");
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            fs::read_link(&link).unwrap(),
            Path::new("../MacOS/table-tavern")
        );

        for target in ["../../../../etc/passwd", "/etc/passwd"] {
            let mut builder = tar::Builder::new(Vec::new());
            append_link(
                &mut builder,
                "Table Tavern.app/Contents/MacOS/escape",
                target,
            );
            let dest = root.0.join(format!("bad-{}", target.len()));
            let error = extract_stripped(&finish_gz(builder), &dest).unwrap_err();
            assert_eq!(error, "壓縮檔含連結");
            assert!(!dest.join("Contents/MacOS/escape").exists());
        }
    }

    /// 真的 `.app.tar.gz`。`npm run verify` 不跑 ignore。
    /// `TT_REAL_APP_TARBALL=/path/to/Table\\ Tavern.app.tar.gz cargo test --lib real_app_tarball -- --ignored`
    #[test]
    #[ignore]
    fn real_app_tarball_keeps_the_executable_bit_and_verifies() {
        let path =
            std::env::var("TT_REAL_APP_TARBALL").expect("TT_REAL_APP_TARBALL 要指向 .app.tar.gz");
        let bytes = fs::read(&path).unwrap();
        let root = TempDir::new("real-tarball");
        let dest = root.0.join(APP_NAME);
        extract_stripped(&bytes, &dest).unwrap();
        assert!(
            read_bundle_info(&dest).version.is_some(),
            "Info.plist 版本可讀"
        );
        let binary = dest.join("Contents/MacOS/table-tavern");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&binary).unwrap().permissions().mode();
            assert_ne!(mode & 0o111, 0, "主程式要有執行位元");
        }
        #[cfg(not(unix))]
        {
            let _ = binary;
        }
        #[cfg(target_os = "macos")]
        {
            let output = std::process::Command::new("codesign")
                .args(["--verify", "--strict", "--verbose=2"])
                .arg(&dest)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "codesign --verify --strict 失敗：{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}
