//! `versions/previous.json`。已確認的 `previous` 與待確認的 `pending` 分開。
//! 回退不寫這個檔。讀不懂時不覆寫，避免把還在的紀錄清成空的。

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::semver_util::versions_equal;
use super::store::{replace_synced, sync_dir};

const FILE_NAME: &str = "previous.json";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PreviousState {
    pub previous: Option<String>,
    pub pending_from: Option<String>,
    pub pending_to: Option<String>,
    pub unreadable: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct PreviousFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pending: Option<Pending>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Pending {
    from: String,
    to: String,
}

fn file_path(versions: &Path) -> std::path::PathBuf {
    versions.join(FILE_NAME)
}

pub(crate) fn read_previous(versions: &Path) -> PreviousState {
    let path = file_path(versions);
    if !path.is_file() {
        return PreviousState::default();
    }
    let Ok(text) = fs::read_to_string(&path) else {
        return PreviousState {
            unreadable: true,
            ..PreviousState::default()
        };
    };
    let Ok(file) = serde_json::from_str::<PreviousFile>(&text) else {
        return PreviousState {
            unreadable: true,
            ..PreviousState::default()
        };
    };
    let (pending_from, pending_to) = match file.pending {
        Some(pending) if !pending.from.is_empty() && !pending.to.is_empty() => {
            (Some(pending.from), Some(pending.to))
        }
        _ => (None, None),
    };
    PreviousState {
        previous: file.previous.filter(|version| !version.is_empty()),
        pending_from,
        pending_to,
        unreadable: false,
    }
}

/// 正向更新重驗通過、開閘之前。已確認的 `previous` 留著。
pub(crate) fn write_pending(versions: &Path, from: &str, to: &str) -> Result<(), String> {
    let current = read_previous(versions);
    if current.unreadable {
        log::warn!("previous.json 讀不懂，這次只寫入待確認");
    }
    let file = PreviousFile {
        previous: if current.unreadable {
            None
        } else {
            current.previous
        },
        pending: Some(Pending {
            from: from.to_owned(),
            to: to.to_owned(),
        }),
    };
    write_file(versions, &file)
}

/// 啟動後。執行中＝`pending.to` 才把 `previous` 改成 `pending.from`。
/// 執行中＝`pending.from`，或兩者都不是，只清 `pending`。
/// 已確認的 `previous` 等於執行中版本時清掉。
pub(crate) fn confirm_on_launch(versions: &Path, running: &str) -> Result<(), String> {
    let current = read_previous(versions);
    if current.unreadable {
        log::warn!("previous.json 讀不懂，這次不改它");
        return Ok(());
    }
    if current.previous.is_none() && current.pending_from.is_none() {
        return Ok(());
    }
    let mut previous = current.previous;
    let mut pending = current.pending_from.zip(current.pending_to);
    if let Some((from, to)) = pending.clone() {
        if versions_equal(running, &to) {
            previous = Some(from);
        }
        pending = None;
    }
    if previous
        .as_deref()
        .is_some_and(|version| versions_equal(version, running))
    {
        previous = None;
    }
    write_state(versions, previous, pending)
}

/// 刪掉的那一版若是已確認的上一版，把欄位清掉。`pending` 不動。
pub(crate) fn clear_previous_if(versions: &Path, version: &str) -> Result<(), String> {
    let current = read_previous(versions);
    if current.unreadable {
        return Ok(());
    }
    let Some(previous) = current.previous else {
        return Ok(());
    };
    if !versions_equal(&previous, version) {
        return Ok(());
    }
    write_state(versions, None, current.pending_from.zip(current.pending_to))
}

fn write_state(
    versions: &Path,
    previous: Option<String>,
    pending: Option<(String, String)>,
) -> Result<(), String> {
    if previous.is_none() && pending.is_none() {
        let path = file_path(versions);
        if path.exists() {
            fs::remove_file(&path).map_err(|error| error.to_string())?;
            sync_dir(versions)?;
        }
        return Ok(());
    }
    let file = PreviousFile {
        previous,
        pending: pending.map(|(from, to)| Pending { from, to }),
    };
    write_file(versions, &file)
}

fn write_file(versions: &Path, file: &PreviousFile) -> Result<(), String> {
    fs::create_dir_all(versions).map_err(|error| error.to_string())?;
    let body = serde_json::to_vec_pretty(file).map_err(|error| error.to_string())?;
    replace_synced(&file_path(versions), &body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new(label: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("tt-prev-{label}-{}", ulid::Ulid::generate()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn seed(versions: &Path, previous: Option<&str>, pending: Option<(&str, &str)>) {
        write_state(
            versions,
            previous.map(str::to_owned),
            pending.map(|(from, to)| (from.to_owned(), to.to_owned())),
        )
        .unwrap();
    }

    #[test]
    fn new_version_confirms_pending_and_keeps_a_failed_install() {
        let root = TempDir::new("confirm");
        seed(&root.0, Some("0.1.0"), Some(("0.2.0", "0.3.0")));
        confirm_on_launch(&root.0, "0.3.0").unwrap();
        let state = read_previous(&root.0);
        assert_eq!(state.previous.as_deref(), Some("0.2.0"));
        assert!(state.pending_from.is_none());

        seed(&root.0, Some("0.1.0"), Some(("0.2.0", "0.3.0")));
        confirm_on_launch(&root.0, "0.2.0").unwrap();
        let state = read_previous(&root.0);
        assert_eq!(
            state.previous.as_deref(),
            Some("0.1.0"),
            "安裝沒成功要留原紀錄"
        );
        assert!(state.pending_from.is_none());
    }

    #[test]
    fn mismatched_pending_is_cleared_and_running_previous_is_cleared() {
        let root = TempDir::new("mismatch");
        seed(&root.0, Some("0.1.0"), Some(("0.2.0", "0.3.0")));
        confirm_on_launch(&root.0, "0.4.0").unwrap();
        let state = read_previous(&root.0);
        assert_eq!(state.previous.as_deref(), Some("0.1.0"));
        assert!(state.pending_to.is_none());

        seed(&root.0, Some("0.3.0"), None);
        confirm_on_launch(&root.0, "0.3.0").unwrap();
        assert!(read_previous(&root.0).previous.is_none());
        assert!(!root.0.join(FILE_NAME).exists());
    }

    #[test]
    fn write_pending_keeps_the_confirmed_previous() {
        let root = TempDir::new("keep");
        seed(&root.0, Some("0.1.0"), None);
        write_pending(&root.0, "0.2.0", "0.3.0").unwrap();
        let state = read_previous(&root.0);
        assert_eq!(state.previous.as_deref(), Some("0.1.0"));
        assert_eq!(state.pending_from.as_deref(), Some("0.2.0"));
        assert_eq!(state.pending_to.as_deref(), Some("0.3.0"));
    }

    #[test]
    fn interrupted_rewrite_keeps_the_old_previous_json() {
        let root = TempDir::new("atomic");
        write_pending(&root.0, "0.1.0", "0.2.0").unwrap();
        let official = fs::read(root.0.join(FILE_NAME)).unwrap();
        let tmp = root.0.join(format!("{FILE_NAME}.tmp"));
        {
            let _hold = super::super::store::FailBeforeRename::arm();
            let error = write_pending(&root.0, "0.3.0", "0.4.0").unwrap_err();
            assert_eq!(error, "改寫中斷");
        }
        assert_eq!(fs::read(root.0.join(FILE_NAME)).unwrap(), official);
        let state = read_previous(&root.0);
        assert_eq!(state.pending_from.as_deref(), Some("0.1.0"));
        assert_eq!(state.pending_to.as_deref(), Some("0.2.0"));
        fs::write(&tmp, b"{").unwrap();
        let state = read_previous(&root.0);
        assert_eq!(state.pending_from.as_deref(), Some("0.1.0"));
        assert!(!state.unreadable, "殘留暫存不參與判讀");
    }

    #[test]
    fn unreadable_file_is_left_in_place() {
        let root = TempDir::new("bad");
        fs::write(root.0.join(FILE_NAME), b"{").unwrap();
        confirm_on_launch(&root.0, "0.2.0").unwrap();
        assert_eq!(fs::read(root.0.join(FILE_NAME)).unwrap(), b"{");
    }
}
