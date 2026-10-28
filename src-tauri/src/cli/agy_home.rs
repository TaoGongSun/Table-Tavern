//! agy 的假 HOME（plans/agy-conversation-isolation.md）。agy 沒有資料目錄旗標或變數，對話、標題庫
//! 一律寫進 `$HOME/.gemini`；app 的劇情因此出現在使用者終端機 agy 的 /resume。換 HOME 才隔得開。
//! 登入共用〔作者裁決 2026-10-28〕：macOS 的 keychain 路徑跟著 HOME 走，假 HOME 裡只放一條
//! `Library/Keychains` → 使用者真 keychain 的 symlink，其他一律不共用。
// symlink 只在 macOS 建（Windows 的 Credential Manager 不跟路徑走）；其他平台只剩測試與測試通道在用
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use crate::ui_msg::UiMsg;
use std::path::{Path, PathBuf};

/// 假 HOME 裡共用登入的那條 symlink 的相對位置（測試通道的 symlink 檢查也認這條）。
pub(crate) const KEYCHAIN_LINK: [&str; 2] = ["Library", "Keychains"];

/// app 行程的真 HOME。只認 `HOME`：空值或相對路徑就回錯，不猜。
pub(crate) fn real_home() -> Result<PathBuf, String> {
    home_from(std::env::var_os("HOME"))
}

fn home_from(value: Option<std::ffi::OsString>) -> Result<PathBuf, String> {
    let home = value.map(PathBuf::from).unwrap_or_default();
    if home.as_os_str().is_empty() || !home.is_absolute() {
        return Err(failed(format!("HOME={}", home.display())));
    }
    Ok(home)
}

/// 給 agy 子程序的環境：HOME 指向 app 自己的 agy-home。
pub fn agy_envs(home: &Path) -> Vec<(String, String)> {
    let home = home.to_string_lossy().into_owned();
    [
        ("HOME", home.clone()),
        // Windows 認 USERPROFILE，HOME 在那邊不作數
        ("USERPROFILE", home),
        // HOME 被換掉時自動更新的落點未定（可能裝進假 HOME、真 binary 永遠不更新）；
        // 更新交給使用者終端機與安裝腳本。只認 "true"：1.4.8 實測 "1" 照樣起背景更新程序
        ("AGY_CLI_DISABLE_AUTO_UPDATE", "true".to_owned()),
    ]
    .map(|(key, value)| (key.to_owned(), value))
    .to_vec()
}

/// 建假 HOME 並收成 0700（裡面有對話與 keychain 連結，不讓同機其他使用者讀）；任一步失敗附路徑回錯。
pub(crate) fn prepare_home(home: &Path) -> Result<(), String> {
    std::fs::create_dir_all(home)
        .map_err(|error| failed(format!("{}：{error}", home.display())))?;
    restrict(home)
}

#[cfg(unix)]
fn restrict(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .map_err(|error| failed(format!("{}：{error}", path.display())))
}

#[cfg(not(unix))]
fn restrict(_path: &Path) -> Result<(), String> {
    Ok(())
}

/// 最後防線（只防漏網）：要 spawn 的是 agy 時，envs 必須顯式帶非空、絕對的 HOME，
/// 否則它會繼承 app 的 HOME、把對話寫進使用者自己的 `~/.gemini`。
pub(crate) fn check_spawn_envs(program: &Path, envs: &[(String, String)]) -> Result<(), String> {
    let name = program
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase());
    if !matches!(name.as_deref(), Some("agy" | "agy.exe")) {
        return Ok(());
    }
    let home = envs
        .iter()
        .rev()
        .find(|(key, _)| key == "HOME")
        .map(|(_, value)| value.as_str());
    match home {
        Some(home) if !home.is_empty() && Path::new(home).is_absolute() => Ok(()),
        _ => Err(failed(format!("{}: HOME", program.display()))),
    }
}

/// 確保 `agy_home/Library/Keychains` 是指向 `real_home/Library/Keychains` 的 symlink。
/// 冪等、可並行（臨時名＋rename）；原位是實體檔或目錄就回錯，不刪不搬。
pub(crate) fn ensure_keychain_link(agy_home: &Path, real_home: &Path) -> Result<(), String> {
    let target: PathBuf = KEYCHAIN_LINK
        .iter()
        .fold(real_home.to_path_buf(), |p, s| p.join(s));
    if !std::fs::metadata(&target).is_ok_and(|meta| meta.is_dir()) {
        return Err(failed(format!("{} 不存在或不是資料夾", target.display())));
    }
    let library = agy_home.join(KEYCHAIN_LINK[0]);
    std::fs::create_dir_all(&library)
        .map_err(|error| failed(format!("{}：{error}", library.display())))?;
    let link = library.join(KEYCHAIN_LINK[1]);
    match std::fs::symlink_metadata(&link) {
        Ok(meta) if meta.file_type().is_symlink() => {
            if std::fs::read_link(&link).is_ok_and(|current| current == target) {
                return Ok(());
            }
        }
        Ok(_) => {
            return Err(UiMsg::AgyKeychainSlotOccupied {
                path: link.display().to_string(),
            }
            .into())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(failed(format!("{}：{error}", link.display()))),
    }
    // 不存在、斷鏈或指向別處：同目錄唯一臨時名建好再 rename 蓋上，並行呼叫不會看到半成品
    let mut nonce = [0u8; 8];
    getrandom::fill(&mut nonce).map_err(|error| failed(error.to_string()))?;
    let temp = library.join(format!(
        ".Keychains.{}.{}",
        std::process::id(),
        nonce.iter().map(|b| format!("{b:02x}")).collect::<String>()
    ));
    let result = make_symlink(&target, &temp).and_then(|()| std::fs::rename(&temp, &link));
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(failed(format!("{}：{error}", link.display())));
    }
    Ok(())
}

#[cfg(unix)]
fn make_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(not(unix))]
fn make_symlink(_target: &Path, _link: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other("symlink unsupported"))
}

fn failed(error: String) -> String {
    UiMsg::AgyProfileFailed { error }.into()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    struct Scratch(PathBuf);
    impl Scratch {
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn setup() -> (Scratch, PathBuf, PathBuf, PathBuf) {
        let dir = Scratch(std::env::temp_dir().join(format!(
            "tt-agy-home-{}-{}",
            std::process::id(),
            ulid::Ulid::generate()
        )));
        let real = dir.path().join("real");
        let keychains = real.join("Library/Keychains");
        std::fs::create_dir_all(&keychains).unwrap();
        let agy = dir.path().join("agy-home");
        (dir, real, agy, keychains)
    }

    #[test]
    fn real_home_rejects_empty_missing_and_relative() {
        assert!(home_from(None).is_err());
        assert!(home_from(Some("".into())).is_err());
        assert!(home_from(Some("relative/home".into())).is_err());
        assert_eq!(
            home_from(Some("/Users/x".into())).unwrap(),
            PathBuf::from("/Users/x")
        );
    }

    #[test]
    fn agy_envs_point_home_at_the_app_profile() {
        let envs = agy_envs(Path::new("/cfg/agy-home"));
        let get = |key: &str| envs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
        assert_eq!(get("HOME"), Some("/cfg/agy-home"));
        assert_eq!(get("USERPROFILE"), Some("/cfg/agy-home"));
        assert_eq!(get("AGY_CLI_DISABLE_AUTO_UPDATE"), Some("true"));
    }

    #[test]
    fn prepare_home_creates_private_dir_and_reports_failures() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, _, agy, _) = setup();
        prepare_home(&agy).unwrap();
        let mode = std::fs::metadata(&agy).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
        // 建不出來（父層是檔案）：回錯附路徑
        let blocker = dir.path().join("file");
        std::fs::write(&blocker, b"x").unwrap();
        let error = prepare_home(&blocker.join("agy-home")).unwrap_err();
        assert!(error.contains("agy_profile_failed"), "{error}");
        assert!(error.contains("agy-home"), "{error}");
        // 收權限失敗（路徑不存在時 set_permissions 必失敗）：回錯附路徑、不吞掉
        let missing = dir.path().join("gone");
        let error = restrict(&missing).unwrap_err();
        assert!(
            error.contains("agy_profile_failed") && error.contains("gone"),
            "{error}"
        );
    }

    #[test]
    fn agy_spawn_requires_absolute_home() {
        let agy = Path::new("/bin/agy");
        assert!(check_spawn_envs(agy, &[]).is_err());
        let env = |v: &str| vec![("HOME".to_owned(), v.to_owned())];
        assert!(check_spawn_envs(agy, &env("")).is_err());
        assert!(check_spawn_envs(agy, &env("rel")).is_err());
        assert!(check_spawn_envs(agy, &env("/cfg/agy-home")).is_ok());
        assert!(check_spawn_envs(Path::new("C:/x/AGY.EXE"), &[]).is_err());
        assert!(check_spawn_envs(Path::new("/bin/claude"), &[]).is_ok());
    }

    #[test]
    fn creates_link_and_is_idempotent() {
        let (_dir, real, agy, keychains) = setup();
        ensure_keychain_link(&agy, &real).unwrap();
        let link = agy.join("Library/Keychains");
        assert_eq!(std::fs::read_link(&link).unwrap(), keychains);
        ensure_keychain_link(&agy, &real).unwrap();
        assert_eq!(std::fs::read_link(&link).unwrap(), keychains);
        // 沒留下臨時檔
        assert_eq!(std::fs::read_dir(agy.join("Library")).unwrap().count(), 1);
    }

    #[test]
    fn repoints_wrong_and_dangling_links() {
        let (dir, real, agy, keychains) = setup();
        std::fs::create_dir_all(agy.join("Library")).unwrap();
        let link = agy.join("Library/Keychains");
        std::os::unix::fs::symlink(dir.path().join("gone"), &link).unwrap();
        ensure_keychain_link(&agy, &real).unwrap();
        assert_eq!(std::fs::read_link(&link).unwrap(), keychains);
        std::fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(dir.path(), &link).unwrap();
        ensure_keychain_link(&agy, &real).unwrap();
        assert_eq!(std::fs::read_link(&link).unwrap(), keychains);
    }

    #[test]
    fn missing_target_is_an_error() {
        let (dir, _real, agy, _) = setup();
        let error = ensure_keychain_link(&agy, &dir.path().join("nobody")).unwrap_err();
        assert!(error.contains("agy_profile_failed"), "{error}");
        assert!(!agy.join("Library/Keychains").exists());
    }

    #[test]
    fn real_directory_in_place_is_kept_and_reported() {
        let (_dir, real, agy, _) = setup();
        let slot = agy.join("Library/Keychains");
        std::fs::create_dir_all(&slot).unwrap();
        std::fs::write(slot.join("login.keychain-db"), b"keep").unwrap();
        let error = ensure_keychain_link(&agy, &real).unwrap_err();
        assert!(error.contains("agy_keychain_slot_occupied"), "{error}");
        assert!(error.contains(&slot.display().to_string()), "{error}");
        assert_eq!(
            std::fs::read(slot.join("login.keychain-db")).unwrap(),
            b"keep"
        );
    }
}
