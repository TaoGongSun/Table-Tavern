//! 啟動復原標記 `cli-update-pending-<cli>.json`（plans/cli-version-gate.md 包 2 第 3 點「啟動復原標記」）。
//! 開始更新前寫入，成功或還原成功後才刪；還原失敗就保留並記 `restore_failed`。
//! 依 CLI 分檔：兩家同時更新各自的標記不互蓋。

use super::recipe::Target;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Marker {
    pub cli: String,
    /// 更新前 `find_binary` 找到的那支
    pub path: PathBuf,
    /// 更新前的 `--version` 第一行原文
    pub version: String,
    /// agy 更新前的執行檔副本；其他家沒有
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<PathBuf>,
    pub target: Target,
    #[serde(default)]
    pub restore_failed: bool,
    /// 最近一步（更新或還原）的程序群組；啟動復原先確認它停了才判定
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<super::tree::Group>,
}

pub(crate) fn path(data_root: &Path, cli: &str) -> PathBuf {
    data_root.join(format!("cli-update-pending-{cli}.json"))
}

/// agy 備份副本放這裡（還原時複製回去；agy 自己留的 `agy.<ns>.old` 會被清掉，不能依賴）。
pub(crate) fn backup_path(data_root: &Path, cli: &str) -> PathBuf {
    data_root.join("cli-update-backup").join(cli)
}

pub(crate) fn read(data_root: &Path, cli: &str) -> Option<Marker> {
    let text = std::fs::read_to_string(path(data_root, cli)).ok()?;
    match serde_json::from_str(&text) {
        Ok(marker) => Some(marker),
        Err(error) => {
            log::warn!("CLI 更新標記讀不懂（{cli}）：{error}");
            None
        }
    }
}

/// 寫臨時檔再 rename，寫一半當機也不會留下半截標記。
pub(crate) fn write(data_root: &Path, marker: &Marker) -> std::io::Result<()> {
    let target = path(data_root, &marker.cli);
    let temp = target.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(marker).map_err(std::io::Error::other)?;
    // world-write-exempt: CLI 更新復原標記，在資料根目錄，不是桌目錄
    std::fs::write(&temp, text)?;
    std::fs::rename(&temp, &target)
}

/// 刪標記與 agy 備份；刪不掉只記 log（下次啟動會依實際版本判斷，不會誤還原）。
pub(crate) fn clear(data_root: &Path, cli: &str) {
    // world-write-exempt: 刪 CLI 更新復原標記，在資料根目錄，不是桌目錄
    if let Err(error) = std::fs::remove_file(path(data_root, cli)) {
        if error.kind() != std::io::ErrorKind::NotFound {
            log::warn!("刪 CLI 更新標記失敗（{cli}）：{error}");
        }
    }
    // world-write-exempt: 刪 CLI 更新前的備份副本，在資料根目錄，不是桌目錄
    let _ = std::fs::remove_file(backup_path(data_root, cli));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_are_per_cli_and_round_trip() {
        let root = std::env::temp_dir().join(format!("tt-marker-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let claude = Marker {
            cli: "claude".to_owned(),
            path: PathBuf::from("/x/claude"),
            version: "2.1.300 (Claude Code)".to_owned(),
            backup: None,
            target: Target::Version("2.1.311".to_owned()),
            restore_failed: false,
            group: Some(super::super::tree::Group {
                pgid: 4242,
                started: Some(1_700_000_000),
            }),
        };
        let grok = Marker {
            cli: "grok".to_owned(),
            target: Target::Latest,
            ..claude.clone()
        };
        write(&root, &claude).unwrap();
        write(&root, &grok).unwrap();
        assert_eq!(read(&root, "claude"), Some(claude));
        assert_eq!(read(&root, "grok"), Some(grok));
        clear(&root, "claude");
        assert_eq!(read(&root, "claude"), None);
        assert!(read(&root, "grok").is_some());
        let _ = std::fs::remove_dir_all(&root);
    }
}
