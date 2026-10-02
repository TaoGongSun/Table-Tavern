//! Windows 回退的安裝檔啟動。參數組成與 `.launch/` 清理在任何平台都能測。
//! 真正呼叫 `ShellExecuteW` 只在 Windows。

use std::path::{Path, PathBuf};

use super::store::{sync_dir, write_synced};

pub(crate) const LAUNCH_DIR: &str = ".launch";

/// plugin passive 的 NSIS 參數。不帶 `/ARGS`（那是把目前程序的參數轉給新版）。
/// 參數只寫在這一份；下面的字串由它組出來。非 Windows 的 cargo check 編譯不到啟動函式。
#[cfg_attr(not(any(test, windows)), allow(dead_code))]
pub(crate) fn nsis_rollback_args() -> &'static [&'static str] {
    &["/P", "/UPDATE", "/R"]
}

#[cfg_attr(not(any(test, windows)), allow(dead_code))]
pub(crate) fn nsis_rollback_parameters() -> String {
    nsis_rollback_args().join(" ")
}

/// 複製到 `versions/.launch/`。不刪，下次 `clear_launch_dir` 才清。
#[cfg_attr(not(any(test, windows)), allow(dead_code))]
pub(crate) fn stage_launch_installer(
    versions: &Path,
    file_name: &str,
    bytes: &[u8],
) -> Result<PathBuf, String> {
    if file_name.contains('/') || file_name.contains('\\') || file_name.contains('\0') {
        return Err("安裝檔名稱不符合規則".to_owned());
    }
    let launch = versions.join(LAUNCH_DIR);
    fs_create(&launch)?;
    let dest = launch.join(file_name);
    if dest.parent() != Some(launch.as_path()) {
        return Err("安裝檔名稱不符合規則".to_owned());
    }
    write_synced(&dest, bytes)?;
    sync_dir(&launch)?;
    Ok(dest)
}

pub(crate) fn clear_launch_dir(versions: &Path) -> Result<(), String> {
    let launch = versions.join(LAUNCH_DIR);
    if !launch.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(&launch).map_err(|error| error.to_string())?;
    if versions.exists() {
        sync_dir(versions)?;
    }
    Ok(())
}

#[cfg_attr(not(any(test, windows)), allow(dead_code))]
fn fs_create(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|error| error.to_string())
}

#[cfg(target_os = "windows")]
pub(crate) fn start_nsis_installer(exe: &Path) -> Result<(), String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOW;

    fn wide(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain(std::iter::once(0)).collect()
    }

    let file = wide(exe.as_os_str());
    let joined = nsis_rollback_parameters();
    let params = wide(OsStr::new(joined.as_str()));
    let operation = wide(OsStr::new("open"));
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            params.as_ptr(),
            std::ptr::null(),
            SW_SHOW,
        )
    };
    if (result as isize) <= 32 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    // 安裝程式已啟動。跟 plugin 一樣直接退出，不放閘。
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("tt-launch-{}", ulid::Ulid::generate()));
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
    fn rollback_args_are_passive_update_restart_without_args() {
        assert_eq!(nsis_rollback_args(), ["/P", "/UPDATE", "/R"]);
        assert_eq!(nsis_rollback_parameters(), "/P /UPDATE /R");
    }

    #[test]
    fn launch_copy_stays_until_the_next_clear() {
        let root = TempDir::new();
        let dest =
            stage_launch_installer(&root.0, "TableTavern_0.1.0_x64-setup.exe", b"exe").unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"exe");
        assert!(dest.starts_with(root.0.join(LAUNCH_DIR)));
        clear_launch_dir(&root.0).unwrap();
        assert!(!root.0.join(LAUNCH_DIR).exists());
        clear_launch_dir(&root.0).unwrap();
    }
}
