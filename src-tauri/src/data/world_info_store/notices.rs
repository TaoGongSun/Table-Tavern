//! 待回報檔 `worlds/<id>/world-info/notices.json`（三之 8）：結算撤回時沒還原的變數寫入。換幕、分岔沒有
//! 聊天回傳可附，所以一律先落這個檔才清 pending；聊天呼叫、換幕、分岔的回傳與開桌時讀它，前端提示玩家一次後
//! 呼叫確認刪掉該則。
use super::landing::VarConflict;
use crate::data::paths::world_info_dir;
use crate::data::world_file::with_file_lock;
use crate::data::{invalid_data, DataResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notice {
    /// `<回合鍵>:<意圖序號>`：同一筆重跑結算不會重複記
    pub id: String,
    pub turn_key: String,
    #[serde(flatten)]
    pub conflict: VarConflict,
}

#[derive(Serialize, Deserialize)]
struct NoticesFile {
    version: u32,
    #[serde(default)]
    notices: Vec<Notice>,
}

fn notices_path(root: &Path, world_id: &str) -> DataResult<PathBuf> {
    Ok(world_info_dir(root, world_id)?.join("notices.json"))
}

fn parse(bytes: Option<Vec<u8>>, path: &Path) -> DataResult<Vec<Notice>> {
    let Some(bytes) = bytes else {
        return Ok(Vec::new());
    };
    let file: NoticesFile = serde_json::from_slice(&bytes)
        .map_err(|error| invalid_data(format!("world-info: {}：{error}", path.display())))?;
    if file.version != super::FORMAT_VERSION {
        return Err(invalid_data(format!(
            "world-info: {} 版本 {} 不認得",
            path.display(),
            file.version
        )));
    }
    Ok(file.notices)
}

fn modify(
    root: &Path,
    world_id: &str,
    work: impl FnOnce(&mut Vec<Notice>) -> bool,
) -> DataResult<()> {
    let path = notices_path(root, world_id)?;
    with_file_lock(&path, |file| {
        let mut notices = parse(file.read()?, &path)?;
        if !work(&mut notices) {
            return Ok(());
        }
        if notices.is_empty() {
            return file.remove();
        }
        file.write_atomic(&serde_json::to_vec_pretty(&NoticesFile {
            version: super::FORMAT_VERSION,
            notices,
        })?)
    })
}

/// 還沒確認的回報，依記下的先後。
/// 讀得懂嗎（重設用：讀不懂的才移去備份）。
pub(super) fn check(root: &Path, world_id: &str) -> DataResult<()> {
    read_notices(root, world_id).map(|_| ())
}

pub fn read_notices(root: &Path, world_id: &str) -> DataResult<Vec<Notice>> {
    let path = notices_path(root, world_id)?;
    with_file_lock(&path, |file| parse(file.read()?, &path))
}

/// 玩家看過一則：刪掉它（最後一則刪完連檔一起刪）。不存在的 id 什麼都不做。
pub fn ack_notice(root: &Path, world_id: &str, id: &str) -> DataResult<()> {
    modify(root, world_id, |notices| {
        let before = notices.len();
        notices.retain(|notice| notice.id != id);
        notices.len() != before
    })
}

/// 追加（同 id 已在就略過）。
pub(super) fn append_notices(root: &Path, world_id: &str, new: &[Notice]) -> DataResult<()> {
    modify(root, world_id, |notices| {
        let before = notices.len();
        for notice in new {
            if !notices.iter().any(|existing| existing.id == notice.id) {
                notices.push(notice.clone());
            }
        }
        notices.len() != before
    })
}
