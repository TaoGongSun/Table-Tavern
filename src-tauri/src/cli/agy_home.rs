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

/// app 行程的真家目錄：先認 `HOME`，沒有（Windows 常見）再認 `USERPROFILE`，與 codex 自己的解析一致。
/// 空值或相對路徑就回錯，不猜。錯誤是原始說明（`HOME=…`），呼叫端各自包成自己 CLI 的 profile 失敗碼。
pub(crate) fn real_home() -> Result<PathBuf, String> {
    home_from(std::env::var_os("HOME"), std::env::var_os("USERPROFILE"))
}

fn home_from(
    home: Option<std::ffi::OsString>,
    userprofile: Option<std::ffi::OsString>,
) -> Result<PathBuf, String> {
    let home = home
        .filter(|value| !value.is_empty())
        .or(userprofile)
        .map(PathBuf::from)
        .unwrap_or_default();
    if home.as_os_str().is_empty() || !home.is_absolute() {
        return Err(format!("HOME={}", home.display()));
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

/// 最後防線（只防漏網）：要 spawn 的是隔離過資料目錄的 CLI 時，envs 必須顯式帶非空、絕對的
/// 隔離變數——agy 的 HOME（否則對話寫進使用者的 `~/.gemini`）、codex 的 CODEX_HOME
/// （否則 session 寫進使用者的 `~/.codex`）。
pub(crate) fn check_spawn_envs(program: &Path, envs: &[(String, String)]) -> Result<(), String> {
    let name = program
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase());
    let (key, report): (&str, fn(String) -> String) = match name.as_deref() {
        Some("agy" | "agy.exe") => ("HOME", failed),
        Some("codex" | "codex.exe") => ("CODEX_HOME", super::codex_home::failed),
        _ => return Ok(()),
    };
    let value = envs
        .iter()
        .rev()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str());
    match value {
        Some(value) if !value.is_empty() && Path::new(value).is_absolute() => Ok(()),
        _ => Err(report(format!("{}: {key}", program.display()))),
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
    let temp = temp_beside(&library, KEYCHAIN_LINK[1])?;
    let result = make_symlink(&target, &temp).and_then(|()| std::fs::rename(&temp, &link));
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(failed(format!("{}：{error}", link.display())));
    }
    Ok(())
}

/// app 專用代理的名字（`--agent` 帶這個）。檔案由 `ensure_text_agent` 維護。
pub const TEXT_AGENT: &str = "table-tavern-text";

/// 精簡工具集代理（plans/agy-long-survey-fail.md）：`tools: []` 加 `excludeDefaultComponents`
/// 拿掉 agy 預設的代理系統提示與約 60 個工具（1.4.8 實測只剩內建的 manage_task）。
/// 模型因此不會為了「看工作區」去叫 run_command 被無頭模式擋掉，思考量也降到約 1/3。
/// 本文只是一句中性說明，劇情指示照舊由 app 放在 stdin 正文。
const TEXT_AGENT_FILE: &str = "---
name: table-tavern-text
description: Table Tavern text replies (maintained by the app; do not edit)
tools: []
excludeDefaultComponents: true
mainAgent: true
subagent: false
---
Respond to the user's message with plain text.
";

/// 確保 `agy_home/.gemini/config/agents/table-tavern-text.md` 是上面那份內容：相同就不動，
/// 不同或不存在就同目錄臨時檔＋rename 寫入。原位被資料夾或連結佔住、或任一步失敗都回錯——
/// agy 找不到 `--agent` 指名的代理時可能靜默退回預設代理，那就又回到會叫工具的老樣子。
pub(crate) fn ensure_text_agent(agy_home: &Path) -> Result<(), String> {
    let dir = agy_home.join(".gemini").join("config").join("agents");
    std::fs::create_dir_all(&dir).map_err(|error| failed(format!("{}：{error}", dir.display())))?;
    let path = dir.join(format!("{TEXT_AGENT}.md"));
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_file() => {
            let current = std::fs::read(&path)
                .map_err(|error| failed(format!("{}：{error}", path.display())))?;
            if current == TEXT_AGENT_FILE.as_bytes() {
                return Ok(());
            }
        }
        Ok(_) => return Err(failed(format!("{} 不是一般檔案", path.display()))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(failed(format!("{}：{error}", path.display()))),
    }
    let temp = temp_beside(&dir, TEXT_AGENT)?;
    let result =
        std::fs::write(&temp, TEXT_AGENT_FILE).and_then(|()| std::fs::rename(&temp, &path));
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(failed(format!("{}：{error}", path.display())));
    }
    Ok(())
}

/// 同目錄、每次唯一的臨時名（並行呼叫各寫各的，rename 時才換上）。
fn temp_beside(dir: &Path, stem: &str) -> Result<PathBuf, String> {
    let mut nonce = [0u8; 8];
    getrandom::fill(&mut nonce).map_err(|error| failed(error.to_string()))?;
    Ok(dir.join(format!(
        ".{stem}.{}.{}",
        std::process::id(),
        nonce.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )))
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
        assert!(home_from(None, None).is_err());
        assert!(home_from(Some("".into()), None).is_err());
        assert!(home_from(Some("relative/home".into()), None).is_err());
        assert_eq!(
            home_from(Some("/Users/x".into()), None).unwrap(),
            PathBuf::from("/Users/x")
        );
    }

    #[test]
    fn real_home_falls_back_to_userprofile() {
        // 只有 USERPROFILE（Windows 常見）
        assert_eq!(
            home_from(None, Some("/Users/y".into())).unwrap(),
            PathBuf::from("/Users/y")
        );
        // HOME 是空字串也退回
        assert_eq!(
            home_from(Some("".into()), Some("/Users/y".into())).unwrap(),
            PathBuf::from("/Users/y")
        );
        // HOME 優先
        assert_eq!(
            home_from(Some("/Users/x".into()), Some("/Users/y".into())).unwrap(),
            PathBuf::from("/Users/x")
        );
        assert!(home_from(None, Some("rel".into())).is_err());
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
        // codex 認 CODEX_HOME，不認 HOME
        let codex = Path::new("/bin/codex");
        assert!(check_spawn_envs(codex, &env("/cfg")).is_err());
        let codex_env = |v: &str| vec![("CODEX_HOME".to_owned(), v.to_owned())];
        assert!(check_spawn_envs(codex, &codex_env("rel")).is_err());
        assert!(check_spawn_envs(codex, &codex_env("/cfg/codex-home")).is_ok());
        let error = check_spawn_envs(Path::new("C:/x/codex.exe"), &[]).unwrap_err();
        assert!(error.contains("codex_profile_failed"), "{error}");
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
    fn text_agent_is_written_once_and_rewritten_when_changed() {
        let (_dir, _, agy, _) = setup();
        ensure_text_agent(&agy).unwrap();
        let file = agy.join(".gemini/config/agents/table-tavern-text.md");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), TEXT_AGENT_FILE);
        let written = std::fs::metadata(&file).unwrap().modified().unwrap();
        ensure_text_agent(&agy).unwrap();
        assert_eq!(
            std::fs::metadata(&file).unwrap().modified().unwrap(),
            written
        );
        std::fs::write(&file, "tools: [run_command]").unwrap();
        ensure_text_agent(&agy).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), TEXT_AGENT_FILE);
        // 沒留下臨時檔
        assert_eq!(
            std::fs::read_dir(file.parent().unwrap()).unwrap().count(),
            1
        );
    }

    #[test]
    fn text_agent_slot_taken_by_non_file_is_an_error() {
        let (dir, _, agy, _) = setup();
        let file = agy.join(".gemini/config/agents/table-tavern-text.md");
        std::fs::create_dir_all(&file).unwrap();
        let error = ensure_text_agent(&agy).unwrap_err();
        assert!(error.contains("agy_profile_failed"), "{error}");
        std::fs::remove_dir(&file).unwrap();
        std::os::unix::fs::symlink(dir.path(), &file).unwrap();
        assert!(ensure_text_agent(&agy).is_err());
        // 建不出資料夾（.gemini 是檔案）也回錯
        let (_dir2, _, agy2, _) = setup();
        std::fs::create_dir_all(&agy2).unwrap();
        std::fs::write(agy2.join(".gemini"), b"x").unwrap();
        assert!(ensure_text_agent(&agy2)
            .unwrap_err()
            .contains("agy_profile_failed"));
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
