//! 背景清理（agy-home-cleanup）用的唯讀掃描：不取鎖、不復原（不同於 `recover_for_list`）。

use std::fs;
use std::path::{Path, PathBuf};

use super::super::paths::{validate_id, worlds_dir};
use super::commit::{
    assess, discover_ids, live_dir, log_path, newer_dir, pre_dir, reset_build_root, staging_dir,
    trash_dir,
};

/// 讀 lanes.json 前後各拍一次的快照：每桌的目錄組合與交易紀錄。
/// 有桌待修復或有進行中的交易就回錯，呼叫端整輪停。
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LaneScan {
    worlds: Vec<(String, [bool; 6])>,
    /// 可能放著 lanes.json 的目錄：本體、四種副本、`.tt-reset-<id>/worlds/<id>`
    pub dirs: Vec<PathBuf>,
}

pub(crate) fn readonly_lane_scan(root: &Path) -> Result<LaneScan, String> {
    let mut ids: std::collections::BTreeSet<String> = discover_ids(root)
        .map_err(|error| error.to_string())?
        .into_iter()
        .collect();
    let directory = worlds_dir(root);
    match fs::read_dir(&directory) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.map_err(|error| error.to_string())?;
                let name = entry.file_name();
                if let Some(id) = name
                    .to_str()
                    .and_then(|name| name.strip_prefix(".tt-reset-"))
                    .filter(|id| validate_id(id).is_ok())
                {
                    ids.insert(id.to_owned());
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("{}：{error}", directory.display())),
    }
    let mut scan = LaneScan {
        worlds: Vec::new(),
        dirs: Vec::new(),
    };
    for id in ids {
        // 只有 NotFound 算不存在：權限或 I/O 錯誤整輪停（寧可晚刪，不可誤刪）
        let exists = |path: &Path| {
            path.try_exists()
                .map_err(|error| format!("{}：{error}", path.display()))
        };
        if exists(&log_path(root, &id))? || assess(root, &id).needs_repair {
            return Err(format!("桌 {id} 待修復或有進行中的交易"));
        }
        let reset = reset_build_root(root, &id).join("worlds").join(&id);
        let candidates = [
            live_dir(root, &id),
            staging_dir(root, &id),
            pre_dir(root, &id),
            newer_dir(root, &id),
            trash_dir(root, &id),
            reset,
        ];
        let mut here = [false; 6];
        for (slot, dir) in here.iter_mut().zip(&candidates) {
            *slot = exists(dir)?;
        }
        scan.worlds.push((id, here));
        scan.dirs.extend(
            candidates
                .into_iter()
                .zip(here)
                .filter_map(|(dir, exists)| exists.then_some(dir)),
        );
    }
    Ok(scan)
}
