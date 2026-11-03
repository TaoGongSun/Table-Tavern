//! VN 玩家個人進度 `vn/vn-progress.json`（plans/vn-mode-pkg6.md §2.6、§2.7）：已讀、各幕閱讀位置、存檔書籤。
//! 不進事件流（收回、分岔、退幕都不回滾它）、不進新周目、不打包。檔壞了當空檔，不擋遊玩。
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::paths::world_dir;
use super::world_file::with_file_lock;
use super::DataResult;
use crate::ui_msg::UiMsg;

/// 已讀鍵上限：超過丟最舊的一半
const READ_CAP: usize = 20_000;
/// 已讀鍵是前端算的 64 位元雜湊（16 位十六進位）
const KEY_MAX: usize = 32;
const POSITION_MAX: usize = 200;
pub const SAVES_CAP: usize = 100;
pub const NAME_MAX: usize = 60;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    #[serde(default)]
    pub version: u32,
    /// 已讀格的內容雜湊（依加入順序，修剪時丟前面的）
    #[serde(default)]
    pub read: Vec<String>,
    /// 幕號字串 → 最後讀到的格鍵（`<事件 id>#<格序>`）
    #[serde(default)]
    pub positions: BTreeMap<String, String>,
    #[serde(default)]
    pub saves: Vec<Save>,
}

/// 存檔書籤：幕＋事件＋格；讀檔時才切換或分岔（§2.7）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Save {
    pub id: String,
    pub name: String,
    pub scene: u64,
    pub event_id: String,
    pub cell: u32,
    pub chapter: u32,
    pub ts: String,
    pub preview: String,
}

fn progress_path(root: &Path, world_id: &str) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?
        .join("vn")
        .join("vn-progress.json"))
}

fn parse(bytes: Option<Vec<u8>>) -> Progress {
    bytes
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn read_progress(root: &Path, world_id: &str) -> DataResult<Progress> {
    let path = progress_path(root, world_id)?;
    with_file_lock(&path, |file| Ok(parse(file.read()?)))
}

/// 檔案鎖內讀改寫、原子寫回。
pub fn update_progress<T>(
    root: &Path,
    world_id: &str,
    change: impl FnOnce(&mut Progress) -> DataResult<T>,
) -> DataResult<T> {
    let path = progress_path(root, world_id)?;
    with_file_lock(&path, |file| {
        let mut progress = parse(file.read()?);
        let result = change(&mut progress)?;
        progress.version = 1;
        file.write_atomic(&serde_json::to_vec(&progress)?)?;
        Ok(result)
    })
}

/// 記已讀（集合聯集，冪等）與這幕的閱讀位置。鍵長不合的整批拒收（前端算錯，不寫半批）。
pub fn record_reading(
    root: &Path,
    world_id: &str,
    read_add: &[String],
    position: Option<(u64, &str)>,
) -> DataResult<()> {
    let bad = |key: &str| key.is_empty() || key.len() > KEY_MAX;
    if read_add.iter().any(|key| bad(key))
        || position.is_some_and(|(_, key)| key.is_empty() || key.len() > POSITION_MAX)
    {
        return Err(crate::data::invalid_data(
            "vn-progress：已讀鍵或位置格式不對",
        ));
    }
    update_progress(root, world_id, |progress| {
        let mut known: HashSet<String> = progress.read.iter().cloned().collect();
        for key in read_add {
            if known.insert(key.clone()) {
                progress.read.push(key.clone());
            }
        }
        if progress.read.len() > READ_CAP {
            let drop = progress.read.len() - READ_CAP / 2;
            progress.read.drain(..drop);
        }
        if let Some((scene, key)) = position {
            progress.positions.insert(scene.to_string(), key.to_owned());
        }
        Ok(())
    })
}

pub fn add_save(root: &Path, world_id: &str, save: Save) -> DataResult<()> {
    update_progress(root, world_id, |progress| {
        if progress.saves.len() >= SAVES_CAP {
            return Err(UiMsg::VnSavesFull {
                cap: SAVES_CAP as u32,
            }
            .into_error());
        }
        progress.saves.push(save);
        Ok(())
    })
}

pub fn delete_save(root: &Path, world_id: &str, id: &str) -> DataResult<()> {
    update_progress(root, world_id, |progress| {
        progress.saves.retain(|save| save.id != id);
        Ok(())
    })
}

#[cfg(test)]
#[path = "vn_progress_tests.rs"]
mod tests;
