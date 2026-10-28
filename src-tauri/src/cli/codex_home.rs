//! codex 的隔離資料目錄（plans/codex-lane-resume.md）。codex 的 session、thread 資料庫、技能與
//! 全域 AGENTS.md 都跟著 `CODEX_HOME` 走；app 改用自己的 codex-home，劇情不再混進使用者的
//! `~/.codex`，使用者的技能清單與 AGENTS.md 也不再帶進跑團請求。
//! 登入共用〔作者裁決 2026-10-28〕：codex-home 裡的 `auth.json` 連回使用者的 `~/.codex/auth.json`
//! ——macOS／Linux 用 symlink，Windows 用硬連結（一般玩家建不了 symlink）〔作者裁決 2026-10-28〕。
//! codex 寫 auth.json 是原地覆寫，兩種連結都保得住；終端機登出會連 app 一起登出。

use crate::ui_msg::UiMsg;
use std::path::{Path, PathBuf};

/// codex-home 裡共用登入的那個檔名（測試通道的 symlink 檢查也認這個位置）。
pub(crate) const AUTH_FILE: &str = "auth.json";
/// Windows 硬連結的來歷標記：記著我們建連結時那個檔的身分。
const HARDLINK_MARKER: &str = ".auth-hardlink";

/// 給 codex 子程序的環境：CODEX_HOME 指向 app 自己的 codex-home。HOME 不動。
pub fn codex_envs(home: &Path) -> Vec<(String, String)> {
    vec![("CODEX_HOME".to_owned(), home.to_string_lossy().into_owned())]
}

/// 共用登入的目標：使用者的 `~/.codex/auth.json`。使用者自己在 shell 設的 CODEX_HOME 不追。
pub(crate) fn auth_target(real_home: &Path) -> PathBuf {
    real_home.join(".codex").join(AUTH_FILE)
}

/// 建 codex-home 並收成 0700（裡面有劇情 session 與登入連結）；任一步失敗附路徑回錯。
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

/// 確保 codex-home 的 auth.json 連到使用者的登入檔。只管連結：還沒登入不算錯（設定頁的模型清單、
/// 換幕容量都會走到這裡），登入與否由真正要跑 codex 的地方用 `require_auth` 擋。
pub(crate) fn ensure_auth_link(home: &Path, real_home: &Path) -> Result<(), String> {
    let target = auth_target(real_home);
    #[cfg(windows)]
    return ensure_auth_hardlink(home, &target);
    #[cfg(not(windows))]
    ensure_auth_symlink(home, &target)
}

/// 真的要跑 codex 前確認使用者的登入檔在（不在＝沒登入，或登入存在系統鑰匙圈、app 共用不到）。
pub(crate) fn require_auth(real_home: &Path) -> Result<(), String> {
    let target = auth_target(real_home);
    if target.is_file() {
        return Ok(());
    }
    Err(UiMsg::CodexAuthMissing {
        path: target.display().to_string(),
    }
    .into())
}

/// symlink 版：不存在、指向別處 → 臨時名建好再 rename 蓋上；已指向目標（含斷鏈）→ 不動；
/// 原位是實體檔或目錄 → 回錯，不刪不搬。斷鏈照留：使用者之後登入，codex 會穿過去建出目標。
#[cfg_attr(windows, allow(dead_code))]
fn ensure_auth_symlink(home: &Path, target: &Path) -> Result<(), String> {
    let slot = home.join(AUTH_FILE);
    match std::fs::symlink_metadata(&slot) {
        Ok(meta) if meta.file_type().is_symlink() => {
            if std::fs::read_link(&slot).is_ok_and(|current| current == target) {
                return Ok(());
            }
        }
        Ok(_) => return Err(occupied(&slot)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(failed(format!("{}：{error}", slot.display()))),
    }
    replace_with(home, &slot, |temp| make_symlink(target, temp))
}

/// 硬連結版（Windows 用；判斷全部跨平台，mac 上也測得到）。硬連結看不出是誰建的，所以建好後在
/// 標記檔記下那個檔的身分；之後只有「槽位現在那個檔＝標記記的身分」才算我們的，可以換掉或刪掉。
/// 使用者重新登入時 codex 會先刪舊檔再建新檔，舊連結就此分離，靠這裡在下一次呼叫接回。
#[cfg_attr(not(windows), allow(dead_code))]
fn ensure_auth_hardlink(home: &Path, target: &Path) -> Result<(), String> {
    let slot = home.join(AUTH_FILE);
    let marker = home.join(HARDLINK_MARKER);
    let recorded = std::fs::read_to_string(&marker).ok();
    let slot_id = match std::fs::symlink_metadata(&slot) {
        Ok(meta) if meta.is_file() => Some(
            file_identity(&slot).map_err(|error| failed(format!("{}：{error}", slot.display())))?,
        ),
        Ok(_) => return Err(occupied(&slot)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(failed(format!("{}：{error}", slot.display()))),
    };
    let ours = slot_id.is_some() && slot_id == recorded;
    let target_id = match file_identity(target) {
        Ok(id) => id,
        // 目標不在＝使用者沒登入：我們留下的舊連結清掉，交給 ensure_auth_link 回 CodexAuthMissing
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if slot_id.is_some() {
                if !ours {
                    return Err(occupied(&slot));
                }
                std::fs::remove_file(&slot)
                    .map_err(|error| failed(format!("{}：{error}", slot.display())))?;
            }
            let _ = std::fs::remove_file(&marker);
            return Ok(());
        }
        Err(error) => return Err(failed(format!("{}：{error}", target.display()))),
    };
    match slot_id {
        // 已經是同一個檔：缺標記或標記過時就補寫
        Some(id) if id == target_id => {
            if recorded.as_deref() != Some(id.as_str()) {
                write_marker(&marker, &id)?;
            }
            return Ok(());
        }
        Some(_) if !ours => return Err(occupied(&slot)),
        _ => {}
    }
    // 跨磁碟等建不了硬連結的情況：回錯附槽位路徑
    replace_with(home, &slot, |temp| std::fs::hard_link(target, temp))?;
    write_marker(&marker, &target_id)
}

fn write_marker(marker: &Path, id: &str) -> Result<(), String> {
    std::fs::write(marker, id).map_err(|error| failed(format!("{}：{error}", marker.display())))
}

/// 同目錄唯一臨時名建好再 rename 蓋上，並行呼叫不會看到半成品；失敗清掉臨時檔。
fn replace_with(
    home: &Path,
    slot: &Path,
    make: impl FnOnce(&Path) -> std::io::Result<()>,
) -> Result<(), String> {
    let mut nonce = [0u8; 8];
    getrandom::fill(&mut nonce).map_err(|error| failed(error.to_string()))?;
    let temp = home.join(format!(
        ".{AUTH_FILE}.{}.{}",
        std::process::id(),
        nonce.iter().map(|b| format!("{b:02x}")).collect::<String>()
    ));
    let result = make(&temp).and_then(|()| std::fs::rename(&temp, slot));
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(failed(format!("{}：{error}", slot.display())));
    }
    Ok(())
}

/// 檔案身分（同一個檔的所有硬連結都相同）：Windows 是磁碟序號＋file index，unix 是 dev＋inode。
#[cfg(windows)]
fn file_identity(path: &Path) -> std::io::Result<String> {
    let handle = winapi_util::Handle::from_path_any(path)?;
    let info = winapi_util::file::information(&handle)?;
    Ok(format!(
        "{:x}:{:x}",
        info.volume_serial_number(),
        info.file_index()
    ))
}

#[cfg(unix)]
fn file_identity(path: &Path) -> std::io::Result<String> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(path)?;
    Ok(format!("{:x}:{:x}", meta.dev(), meta.ino()))
}

#[cfg(unix)]
fn make_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(not(unix))]
fn make_symlink(_target: &Path, _link: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other("symlink unsupported"))
}

fn occupied(slot: &Path) -> String {
    UiMsg::CodexAuthSlotOccupied {
        path: slot.display().to_string(),
    }
    .into()
}

pub(super) fn failed(error: String) -> String {
    UiMsg::CodexProfileFailed { error }.into()
}

#[cfg(all(test, unix))]
mod tests;
