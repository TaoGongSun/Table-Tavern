//! 匯入的未確認記錄（`worlds/<id>/web-save-pending.json`）：後端建好整桌之後，前端還要把卡片 storage 寫進
//! 新桌才算匯入完成。記錄在建桌後第一個寫、每補一層跨桌層之前先記（[`card_vars::fill_missing`]），
//! 玩家確認進桌才刪；放棄匯入照它撤回跨桌層再刪桌。
use super::cleanup_error;
use crate::data::card_vars::{self, Filled};
use crate::data::{self, DataResult};
use crate::ui_msg::UiMsg;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize)]
struct Pending {
    /// 依補的先後；撤回時倒著退
    shared_fill: Vec<Filled>,
}

pub(super) fn write(root: &Path, world_id: &str, shared_fill: &[Filled]) -> DataResult<()> {
    let bytes = serde_json::to_vec_pretty(&Pending {
        shared_fill: shared_fill.to_vec(),
    })?;
    data::commit_world_write_atomic(&data::web_save_pending_path(root, world_id)?, &bytes)
}

/// 沒有記錄（已確認或不是網頁存檔建的桌）回 None；壞掉回錯。
fn read(root: &Path, world_id: &str) -> DataResult<Option<Vec<Filled>>> {
    match std::fs::read(data::web_save_pending_path(root, world_id)?) {
        Ok(bytes) => Ok(Some(serde_json::from_slice::<Pending>(&bytes)?.shared_fill)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// 倒著退記錄裡的每一層（rev compare-and-set），回傳沒撤回的位置。
pub(super) fn retract_all(root: &Path, world_id: &str, shared_fill: &[Filled]) -> Vec<String> {
    let mut leftovers = Vec::new();
    for filled in shared_fill.iter().rev() {
        match card_vars::retract_filled(root, world_id, filled) {
            Ok(kept) => leftovers.extend(kept),
            Err(_) => leftovers.extend(filled.labels()),
        }
    }
    leftovers
}

/// 玩家確認進桌：刪掉記錄，這次匯入補的跨桌鍵從此算玩家的。沒有記錄也算完成。
pub fn confirm_web_save_import(root: &Path, world_id: &str) -> DataResult<()> {
    data::commit_world_remove(&data::web_save_pending_path(root, world_id)?)
}

/// 放棄還沒確認的匯入：照記錄撤回跨桌層（rev 變了的層整層不動），再刪新桌。有沒撤回的鍵或刪不掉的桌，
/// 回 `WebSaveCleanupIncomplete`（error＝`reason`，放棄的原因）。記錄讀不到就不刪桌（撤回依據還在桌裡），
/// 同樣回報殘桌。
pub fn discard_web_save_import(root: &Path, world_id: &str, reason: &str) -> DataResult<()> {
    let Some(held) = data::try_world_exclusive(world_id) else {
        return Err(UiMsg::WorldBusy.into_error());
    };
    let shared_fill = match read(root, world_id) {
        Ok(Some(shared_fill)) => shared_fill,
        Ok(None) => {
            return Err(cleanup_error(
                data::invalid_data("web-save: 這張桌沒有未確認的匯入記錄"),
                vec![format!("world:{world_id}")],
            ))
        }
        Err(error) => return Err(cleanup_error(error, vec![format!("world:{world_id}")])),
    };
    let mut leftovers = retract_all(root, world_id, &shared_fill);
    if data::discard_new_world(root, world_id, &held).is_err() {
        leftovers.push(format!("world:{world_id}"));
    }
    if leftovers.is_empty() {
        return Ok(());
    }
    Err(cleanup_error(reason.into(), leftovers))
}
