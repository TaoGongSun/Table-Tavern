//! 角色卡自動隱藏（`CharacterMeta.auto_hidden`）跟著幕回溯：離開一幕時把每張卡的值拍成快照存進
//! `WorldState.scene_card_visibility`，退回或分岔回到那一幕時照快照寫回卡檔。
//! 規格見 .ai/plans/revert-fork-auto-hidden.md「施工要點」。

use std::path::Path;

use super::super::character::{list_characters, set_character_auto_hidden};
use super::super::state::WorldState;
use super::super::DataResult;

/// 離開目前這一幕前呼叫：把這幕的快照記進 `state`（只改記憶體，呼叫端負責 write_state）。
/// 上次在這一幕的分岔沒做完（`pending_leave` 指著這幕、快照還在）就沿用那份——卡可能已被改了一半，
/// 重拍會把半途的值當成原值。`pending_leave` 指著別幕是殘留，清掉照常重拍。
/// 列卡或讀卡的 I/O 錯誤往上回，不記成空表。
pub(super) fn record_leaving_scene(
    root: &Path,
    world_id: &str,
    state: &mut WorldState,
) -> DataResult<()> {
    let key = state.current_scene.to_string();
    if state.pending_leave == Some(state.current_scene)
        && state.scene_card_visibility.contains_key(&key)
    {
        return Ok(());
    }
    state.pending_leave = None;
    let snapshot = list_characters(root, world_id)?
        .into_iter()
        .map(|meta| (meta.id, meta.auto_hidden))
        .collect();
    state.scene_card_visibility.insert(key, snapshot);
    Ok(())
}

/// 回到 `scene` 時照它的快照寫回卡檔。沒有這幕的快照（舊桌）什麼都不動；快照外的卡（之後才建）、
/// 已刪的卡、現在已封存的卡跳過；值已相同的不寫。任何一張寫入失敗就回錯（寫回冪等，重試可補完）。
pub(super) fn restore_scene_visibility(
    root: &Path,
    world_id: &str,
    state: &WorldState,
    scene: u64,
) -> DataResult<()> {
    let Some(snapshot) = state.scene_card_visibility.get(&scene.to_string()) else {
        return Ok(());
    };
    for meta in list_characters(root, world_id)? {
        if meta.archived {
            continue;
        }
        if let Some(&hidden) = snapshot.get(&meta.id) {
            if hidden != meta.auto_hidden {
                set_character_auto_hidden(root, world_id, &meta.id, hidden)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
