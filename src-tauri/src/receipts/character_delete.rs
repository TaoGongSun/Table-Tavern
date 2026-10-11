//! 刪角色後的清理：世界書拿掉角色 id（名單清空改 GM），收據跟著一致轉換，撤銷時的比對才以清理後的
//! 世界為準。方案見 .ai/plans/character-delete-visibility-cleanup.md 二之 3。
use super::{pop_receipts, read_receipts_checked, worldbook_entry_fingerprint, ImportReceipt};
use crate::data::{self, DataResult};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// 刪角色、角色卡轉條目的回傳：角色已刪除，但世界書或收據的清理沒做完。
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct CharacterDeleteOutcome {
    pub worldbook_cleanup_failed: bool,
}

/// 玩家刪角色：先刪卡、再清。刪卡回錯時照樣清（md 已刪就清），清完回原錯；刪卡成功但清理沒做完
/// （含查不到卡檔是否還在）回旗標。呼叫端持共用許可。
pub fn delete_character_and_clean(
    root: &Path,
    world_id: &str,
    character_id: &str,
) -> DataResult<CharacterDeleteOutcome> {
    let deleted = data::delete_character(root, world_id, character_id);
    let cleaned = after_character_delete(root, world_id, &[character_id.to_owned()]);
    deleted?;
    Ok(CharacterDeleteOutcome {
        worldbook_cleanup_failed: !cleaned,
    })
}

/// 角色卡轉世界書條目：持同一把獨占做完轉換、清理、收據轉換，中間沒有空窗。轉換回錯時照樣清
/// （刪卡那步回錯但 md 已刪），清完回原錯。
pub fn character_to_worldbook_entry_and_clean(
    root: &Path,
    world_id: &str,
    character_id: &str,
    lang: &str,
) -> DataResult<CharacterDeleteOutcome> {
    let Some(held) = data::try_world_exclusive(world_id) else {
        return Err(crate::ui_msg::UiMsg::WorldBusy.into_error());
    };
    let converted =
        data::character_to_worldbook_entry_held(root, world_id, character_id, lang, &held);
    let cleaned = after_character_delete(root, world_id, &[character_id.to_owned()]);
    converted?;
    Ok(CharacterDeleteOutcome {
        worldbook_cleanup_failed: !cleaned,
    })
}

/// 刪除動作結束後呼叫（不論刪除本身成敗）：卡檔確定已不在的 id 才清。卡檔查不到（權限、I/O 錯誤）
/// 記 log、什麼都不清，算清理沒做完。回傳清理是否完成；沒有要清的 id 也算完成。
pub fn after_character_delete(root: &Path, world_id: &str, ids: &[String]) -> bool {
    let mut gone = Vec::new();
    for id in ids {
        match data::character_card_gone(root, world_id, id) {
            Ok(true) => gone.push(id.clone()),
            Ok(false) => {}
            Err(error) => {
                log::warn!("刪角色後查不到卡檔是否還在，名單不清：{error}");
                return false;
            }
        }
    }
    if gone.is_empty() {
        return true;
    }
    data::state_commit::with_commit(root, world_id, |_| cleanup(root, world_id, &gone))
}

/// 先清世界書，成功才轉換收據；兩檔各自原子寫。任一步失敗回 false。
fn cleanup(root: &Path, world_id: &str, ids: &[String]) -> bool {
    let scrub = match data::scrub_character_ids(root, world_id, ids) {
        Ok(scrub) => scrub,
        Err(error) => {
            log::warn!("刪角色後清世界書名單失敗：{error}");
            return false;
        }
    };
    let Some(mut receipts) = read_receipts_checked(root, world_id) else {
        log::warn!("刪角色後讀不到匯入收據，收據沒有轉換");
        return false;
    };
    if !transform_receipts(&mut receipts, &scrub.before, ids) {
        return true;
    }
    match pop_receipts(root, world_id, &receipts) {
        Ok(()) => true,
        Err(error) => {
            log::warn!("刪角色後寫回匯入收據失敗：{error}");
            false
        }
    }
}

/// 收據一致轉換：可抵達值模擬（指紋、可見度還原）＋快照一律清理。`current` 是世界書清理前的原始條目。
/// 回傳收據內容有沒有變。
fn transform_receipts(
    receipts: &mut [ImportReceipt],
    current: &BTreeMap<u64, serde_json::Value>,
    ids: &[String],
) -> bool {
    let original = serde_json::to_value(&*receipts).ok();
    let mut uids = BTreeSet::new();
    for receipt in receipts.iter() {
        uids.extend(receipt.worldbook_entries.iter().map(|entry| entry.uid));
        uids.extend(
            receipt
                .visibility_restores
                .iter()
                .map(|restore| restore.uid),
        );
        uids.extend(receipt.rewritten_entries.iter().map(|entry| entry.uid));
        uids.extend(receipt.deleted_entries.iter().map(|entry| entry.uid));
    }
    for uid in uids {
        simulate(receipts, uid, current.get(&uid).cloned(), ids);
    }
    for receipt in receipts.iter_mut() {
        for entry in receipt
            .rewritten_entries
            .iter_mut()
            .chain(receipt.deleted_entries.iter_mut())
        {
            data::scrub_entry(entry, ids);
        }
        for raw in &mut receipt.deleted_entries_raw {
            data::scrub_entry_value(raw, ids);
        }
    }
    serde_json::to_value(&*receipts).ok() != original
}

/// 沿撤銷順序（由新到舊）推這個 uid 上的值（`None`＝沒有條目），值記清理前的完整原始條目；
/// 每筆收據照撤銷步驟 1a → 2 → 3 → 3a → 撤銷自己的角色清理推進。
fn simulate(
    receipts: &mut [ImportReceipt],
    uid: u64,
    start: Option<serde_json::Value>,
    ids: &[String],
) {
    let mut state = start;
    for receipt in receipts.iter_mut().rev() {
        // 1a 可見度還原：命中才轉換前後值，值換成套上 before
        for restore in receipt
            .visibility_restores
            .iter_mut()
            .filter(|restore| restore.uid == uid)
        {
            let Some(value) = state.as_mut() else {
                continue;
            };
            if data::restore_after_matches(value, restore) {
                data::apply_restore_before(value, restore);
                data::scrub_restore(restore, ids);
            }
        }
        // 2 指紋：命中＝撤銷會刪掉這條，換成清理後的指紋
        for recorded in receipt
            .worldbook_entries
            .iter_mut()
            .filter(|recorded| recorded.uid == uid)
        {
            let Some(value) = state.as_ref() else {
                continue;
            };
            if worldbook_entry_fingerprint(&data::entry_view_of(value)) != recorded.fingerprint {
                continue;
            }
            let mut scrubbed = value.clone();
            data::scrub_entry_value(&mut scrubbed, ids);
            recorded.fingerprint = worldbook_entry_fingerprint(&data::entry_view_of(&scrubbed));
            state = None;
        }
        // 3 改寫快照：uid 還在才覆寫回快照（來源卡沿用原值）
        for snapshot in receipt
            .rewritten_entries
            .iter()
            .filter(|snapshot| snapshot.uid == uid)
        {
            if let Some(value) = state.as_mut() {
                data::apply_entry_fields(value, snapshot);
            }
        }
        // 3a 刪除快照：uid 空著才插回原 uid；被佔時（已插回而略過或改插新 uid）原 uid 上的值不變
        let raw_complete = receipt.deleted_entries_raw.len() == receipt.deleted_entries.len();
        for (index, snapshot) in receipt.deleted_entries.iter().enumerate() {
            if snapshot.uid != uid || state.is_some() {
                continue;
            }
            state = Some(if raw_complete {
                receipt.deleted_entries_raw[index].clone()
            } else {
                data::worldbook_entry_value(snapshot)
            });
        }
        // 撤銷這筆時會刪掉並清理它自己建的角色
        if let Some(value) = state.as_mut() {
            let own: Vec<String> = receipt
                .character_id
                .iter()
                .chain(receipt.character_ids.iter())
                .cloned()
                .collect();
            if !own.is_empty() {
                data::scrub_entry_value(value, &own);
            }
        }
    }
}

#[cfg(test)]
#[path = "character_delete_tests.rs"]
mod tests;
