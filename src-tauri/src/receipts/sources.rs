//! 匯入原檔（refactor-statusbar-skeleton 待問 1〔作者裁決 2026-10-03〕；存法〔模型判斷·未裁決〕）：
//! 每次匯入（角色卡、世界書路徑）先把原始 bytes 另存成 `import-source-<id>.<png|json>`，識別掛在那筆
//! 匯入收據上，開場白序號掛在開場白紀錄上——撤銷匯入時收據彈出、原檔一起刪，來源清單永遠跟收據同步。
//! 每次匯入與貼開場白在動資料前先寫一個專屬的「未完成」標記（`import-pending-<id>`，寫不進去就整次不做），
//! 原檔與收據（或開場序號）都記好才刪掉自己那個標記；收據讀壞、寫不進去、序號歸屬不了時標記留著，
//! 桌上只要還有未完成標記，來源就判不完整、重新重構一律擋下。
use super::{read_receipts_checked, ImportReceipt};
use crate::data::{self, DataResult};
use serde::{Deserialize, Serialize};
use std::path::Path;

const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportRoute {
    Worldbook,
    Character,
}

/// 一筆匯入的原檔與重匯參數。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportSource {
    pub route: ImportRoute,
    /// 世界書路徑的匯入名；角色卡路徑留空
    #[serde(default)]
    pub label: String,
    /// 角色卡路徑的角色色；世界書路徑留空
    #[serde(default)]
    pub color: String,
    pub file: String,
}

/// 重匯一筆：原檔、原檔 bytes，以及匯完貼出的那則開場白（場景、時間戳、序號）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportReplay {
    pub source: ImportSource,
    pub bytes: Vec<u8>,
    pub opening: Option<(u64, String, Option<usize>)>,
}

/// 先存原檔再匯入：存不進去整次匯入就不做（什麼都還沒動），不會出現「有匯入、沒原檔」。
pub fn store_import_source(root: &Path, world_id: &str, bytes: &[u8]) -> DataResult<String> {
    let extension = if bytes.starts_with(PNG_MAGIC) {
        "png"
    } else {
        "json"
    };
    let file = format!("import-source-{}.{extension}", data::new_id());
    data::commit_world_write(
        &data::import_source_file_path(root, world_id, &file)?,
        bytes,
    )?;
    Ok(file)
}

/// 原檔不再被任何收據引用（匯入失敗、沒新增東西、撤銷）：盡力刪掉，刪不掉也不影響——沒被引用的
/// 原檔不會被重匯。
pub fn discard_import_source(root: &Path, world_id: &str, file: &str) {
    if let Ok(path) = data::import_source_file_path(root, world_id, file) {
        let _ = data::commit_world_remove(&path);
    }
}

/// 動資料前先落「未完成」標記；寫不進去回 Err，呼叫端整次不做。回傳標記檔名，完成後交給 finish_pending。
pub fn begin_pending(root: &Path, world_id: &str) -> DataResult<String> {
    let name = format!("import-pending-{}", data::new_id());
    data::commit_world_write(
        &data::import_pending_path(root, world_id, &name)?,
        b"import or opening record in progress\n",
    )?;
    Ok(name)
}

/// 原檔與收據都記好了：刪掉自己的未完成標記。刪不掉就留著（保守：來源判不完整）。
pub fn finish_pending(root: &Path, world_id: &str, name: &str) {
    if let Ok(path) = data::import_pending_path(root, world_id, name) {
        let _ = data::commit_world_remove(&path);
    }
}

fn has_pending(root: &Path, world_id: &str) -> DataResult<bool> {
    let Some(dir) = data::import_receipts_path(root, world_id)?
        .parent()
        .map(Path::to_path_buf)
    else {
        return Ok(false);
    };
    Ok(std::fs::read_dir(dir)?.any(|entry| {
        entry.is_ok_and(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("import-pending-")
        })
    }))
}

fn is_import(receipt: &ImportReceipt) -> bool {
    matches!(receipt.kind.as_str(), "character" | "worldbook")
}

/// 這桌依序要重匯的原檔。來源不完整就回 None：有未完成標記、收據讀不了、有匯入收據沒掛原檔、
/// 或原檔讀不到——拿缺了幾筆的來源清回會永久丟資料。
pub fn import_replays(root: &Path, world_id: &str) -> DataResult<Option<Vec<ImportReplay>>> {
    if has_pending(root, world_id)? {
        return Ok(None);
    }
    let Some(receipts) = read_receipts_checked(root, world_id) else {
        return Ok(None);
    };
    let mut replays = Vec::new();
    for receipt in receipts.iter().filter(|receipt| is_import(receipt)) {
        let Some(source) = &receipt.import_source else {
            return Ok(None);
        };
        let Ok(bytes) = std::fs::read(data::import_source_file_path(root, world_id, &source.file)?)
        else {
            return Ok(None);
        };
        replays.push(ImportReplay {
            source: source.clone(),
            bytes,
            opening: receipt
                .opening
                .as_ref()
                .map(|opening| (opening.scene, opening.ts.clone(), opening.index)),
        });
    }
    Ok(Some(replays))
}
