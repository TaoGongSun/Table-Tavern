//! 一鍵復原：逆向最後一筆收據。玩家手動撤銷（盡力而為）與重構中途失敗的自動回滾（嚴格）共用
//! `undo_last`；收據檔壞掉時才回錯。

use super::undo_changes;
use super::{
    discard_import_source, pop_receipts, worldbook_entry_fingerprint, ExpectedReceipt,
    ImportReceipt, UndoReport,
};
use crate::data::{self, DataResult, Trigger, WorldbookEntry};
use crate::ui_msg::UiMsg;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

fn apply_map_undo<K: Ord + Clone, V: Clone>(
    map: &mut BTreeMap<K, V>,
    added_keys: &[K],
    restored: &BTreeMap<K, V>,
) {
    for key in added_keys {
        map.remove(key);
    }
    for (key, value) in restored {
        map.insert(key.clone(), value.clone());
    }
}

/// triggers 是 Vec（要保留原順序，不能像 map 一樣重排）：新增的整條移除，被覆寫的原地換回舊值。
fn apply_trigger_undo(
    triggers: &mut Vec<Trigger>,
    added_ids: &[String],
    restored: &BTreeMap<String, Trigger>,
) {
    triggers.retain(|trigger| !added_ids.contains(&trigger.id));
    for trigger in triggers.iter_mut() {
        if let Some(old) = restored.get(&trigger.id) {
            *trigger = old.clone();
        }
    }
}

/// 逆向最後一筆收據。收據檔存在但解析失敗要回錯（不能悄悄當空，不然玩家以為復原了
/// 其實什麼都沒發生）；缺檔／空陣列＝沒有可復原的紀錄，同樣回錯。
/// 要出示整桌獨占：讀出收據、各域逆向到寫回彈出後的清單之間，別的匯入、開場紀錄或聊天寫入不能插進來。
/// 不比對收據的版本只剩測試在用；玩家撤銷走 `undo_expected_import`。
#[cfg(test)]
pub fn undo_last_import(
    root: &Path,
    world_id: &str,
    held: &data::WorldExclusive,
) -> DataResult<UndoReport> {
    undo_last(root, world_id, held, false, None)
}

/// 玩家從陣容欄撤銷：最後一筆收據不是確認框顯示的那筆（含收據已空）就回 `ImportReceiptChanged`、什麼都不動。
pub fn undo_expected_import(
    root: &Path,
    world_id: &str,
    expected: &ExpectedReceipt,
    held: &data::WorldExclusive,
) -> DataResult<UndoReport> {
    undo_last(root, world_id, held, false, Some(expected))
}

/// 重構套用中途失敗的自動回滾：同玩家撤銷但不比對收據，但任一域還原失敗就回 Err、收據不彈出
/// （留著給玩家手動撤銷），不像玩家手動撤銷那樣盡力而為、吞掉個別失敗。
pub fn rollback_last_import(
    root: &Path,
    world_id: &str,
    held: &data::WorldExclusive,
) -> DataResult<UndoReport> {
    undo_last(root, world_id, held, true, None)
}

/// 撤銷刪角色沒刪乾淨：包一層「撤銷沒完成，再按一次會重試刪除」。附檔殘留時改報件數與位置，
/// 不帶刪角色那則「請手動刪除」的指引（撤銷可以重試）。
fn undo_incomplete(
    error: Box<dyn std::error::Error + Send + Sync>,
) -> Box<dyn std::error::Error + Send + Sync> {
    match UiMsg::parse(&error.to_string()) {
        Some(UiMsg::CharacterFilesLeft { count, files }) => {
            UiMsg::UndoFilesLeft { count, files }.into_error()
        }
        _ => UiMsg::UndoIncomplete {
            error: error.to_string(),
        }
        .into_error(),
    }
}

/// strict＝false 時個別域的失敗吞掉照常彈出收據（玩家手動撤銷的既有行為）；true 時一律傳回。
fn kept<T>(strict: bool, result: DataResult<T>) -> DataResult<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error) if strict => Err(error),
        Err(_) => Ok(None),
    }
}

fn undo_last(
    root: &Path,
    world_id: &str,
    _held: &data::WorldExclusive,
    strict: bool,
    expected: Option<&ExpectedReceipt>,
) -> DataResult<UndoReport> {
    let path = data::import_receipts_path(root, world_id)?;
    let mut receipts: Vec<ImportReceipt> = if path.exists() {
        let text = fs::read_to_string(&path)?;
        serde_json::from_str(&text).map_err(|error| {
            UiMsg::ImportReceiptCorrupt {
                error: error.to_string(),
            }
            .into_error()
        })?
    } else {
        Vec::new()
    };
    // 帶 expected 時收據已空也算「換過了」：前端看到的那筆已被別處撤掉，要它重讀
    let receipt = receipts.pop().ok_or_else(|| {
        if expected.is_some() {
            UiMsg::ImportReceiptChanged.into_error()
        } else {
            UiMsg::NoImportToUndo.into_error()
        }
    })?;
    if let Some(expected) = expected {
        let same = expected.count == receipts.len() + 1
            && expected.kind == receipt.kind
            && expected.label == receipt.label
            && expected.timestamp == receipt.timestamp
            && expected.character_id == receipt.character_id;
        if !same {
            return Err(UiMsg::ImportReceiptChanged.into_error());
        }
    }

    let mut report = UndoReport::default();

    // 0. 介面骨架檔與玩法標記先退：兩者決定前端給不給面板，退一半會讓面板與狀態樹對不上。
    // 失敗就回 Err，磁碟上的收據不彈出、後面各域還沒動，玩家可以再按一次（兩步都可重做）。
    if receipt.interface_shell_created {
        data::commit_world_remove(&data::interface_shell_path(root, world_id)?)?;
    } else if let Some(original) = &receipt.interface_shell_restore {
        data::write_interface_shell(root, world_id, original)?;
    }
    if let Some(mode) = &receipt.refactor_mode {
        let mut state = data::read_state(root, world_id)?;
        state.refactor_mode = mode.before.clone();
        data::write_state(root, world_id, &state)?;
    }

    // 前快照：給前端草稿定基的改動比對用，讀不到只少了報告，不中斷撤銷
    let book_before = data::read_worldbook(root, world_id).ok();
    let mut step_deleted = HashSet::new();

    // 1. 角色卡：md／原卡／圖片／圖庫一併刪除——按鈕語意是撤銷這次匯入，玩家後續編輯一併退場。
    // 刪不乾淨一律回 Err（手動撤銷也不吞）：收據不彈出、後面各域還沒動，玩家再按一次會重刪（可重做），
    // 不會留下收據已彈出、再也清不掉的原卡與圖。卡檔確定已不在的 id 記下來，3a 之後從世界書名單清掉。
    let mut gone_characters = Vec::new();
    if let Some(character_id) = &receipt.character_id {
        data::delete_character(root, world_id, character_id).map_err(undo_incomplete)?;
        report.removed_character = Some(receipt.label.clone());
        if kept(
            strict,
            data::character_card_gone(root, world_id, character_id),
        )? == Some(true)
        {
            gone_characters.push(character_id.clone());
        }
    }
    // AI 卡重構等一次套用多張角色卡的路徑：character_id／character_ids 兩欄位互斥，
    // 沒有共用的「單一 label＝角色名」可用，逐張讀當下名字再刪。名字先全部讀齊：刪到一半失敗、
    // 玩家重按時，已刪的那幾張讀不到名字了（因此重按的報告只列這次才刪掉的）。
    let names: Vec<Option<String>> = receipt
        .character_ids
        .iter()
        .map(|character_id| {
            data::read_character(root, world_id, character_id)
                .ok()
                .map(|card| card.name)
        })
        .collect();
    for (character_id, name) in receipt.character_ids.iter().zip(names) {
        data::delete_character(root, world_id, character_id).map_err(undo_incomplete)?;
        if let Some(name) = name {
            report.removed_characters.push(name);
        }
        if kept(
            strict,
            data::character_card_gone(root, world_id, character_id),
        )? == Some(true)
        {
            gone_characters.push(character_id.clone());
        }
    }

    // 1a. 被合併改寫的既有條目：只還原可見度與來源卡；玩家之後改過（目前值不是收據記的 after）就不動。
    // 先做這步：前一筆收據新建、被這筆擴大的條目還原後，前一筆撤銷時的指紋才對得上。
    for restore in &receipt.visibility_restores {
        if let Some(data::RestoreOutcome::Kept) = kept(
            strict,
            data::apply_visibility_restore(root, world_id, restore),
        )? {
            report.kept_entries += 1;
        }
    }

    // 1b. 被標成舞台卡的既有角色卡：兩個旗標都還是套用後的值才退回，玩家改過就保留。
    for change in &receipt.stage_restores {
        kept(
            strict,
            data::restore_stage_flags(
                root,
                world_id,
                &change.character_id,
                change.after,
                change.before,
            ),
        )?;
    }

    // 2. 世界書條目：uid 還在且指紋沒變才刪；指紋變了＝玩家改過，保留並計數。
    let current = kept(strict, data::read_worldbook(root, world_id))?.unwrap_or_default();
    for recorded in &receipt.worldbook_entries {
        let Some(entry) = current.iter().find(|entry| entry.uid == recorded.uid) else {
            continue; // 已經不在了（例如玩家自己刪過），不重複計數
        };
        if worldbook_entry_fingerprint(entry) == recorded.fingerprint {
            if kept(
                strict,
                data::delete_worldbook_entry(root, world_id, recorded.uid),
            )?
            .is_some()
            {
                step_deleted.insert(recorded.uid);
            }
            report.removed_entries += 1;
        } else {
            report.kept_entries += 1;
        }
    }

    // 3. 被改寫或停用的既有條目（AI 卡重構的來源條目改寫、介面／機制的 source_uid 停用）：
    // 整條覆寫回原文快照。uid 已經不在了（玩家自己刪過）就略過，不當新條目生回來。
    let current_uids: HashSet<u64> = kept(strict, data::read_worldbook(root, world_id))?
        .unwrap_or_default()
        .into_iter()
        .map(|entry| entry.uid)
        .collect();
    for original in &receipt.rewritten_entries {
        if current_uids.contains(&original.uid) {
            kept(
                strict,
                data::upsert_worldbook_entry(root, world_id, original.clone()),
            )?;
        }
    }

    // 3a. 這次操作整條刪除的條目（人物合併升格的專屬條目或收尾判定可刪的共用合集條目）：
    // 這些是這次操作自己刪的，不是玩家刪的，一律插回，而且可安全重做（撤銷做到一半失敗、
    // 收據保留後再撤一次，不得插出第二份）：原 uid 空著就照原 uid 插回；原 uid 上已是除 uid
    // 外全部欄位都相同的條目＝先前已插回，跳過；原 uid 被別的條目佔走才換新 uid 插，且全欄位
    // 相同的條目已在就跳過。
    let same = |a: &WorldbookEntry, b: &WorldbookEntry| {
        WorldbookEntry {
            uid: 0,
            ..a.clone()
        } == WorldbookEntry {
            uid: 0,
            ..b.clone()
        }
    };
    let raw_complete = receipt.deleted_entries_raw.len() == receipt.deleted_entries.len();
    for (index, original) in receipt.deleted_entries.iter().enumerate() {
        if raw_complete {
            kept(
                strict,
                data::restore_deleted_entry_raw(
                    root,
                    world_id,
                    &receipt.deleted_entries_raw[index],
                ),
            )?;
            continue;
        }
        let current = kept(strict, data::read_worldbook(root, world_id))?.unwrap_or_default();
        match current.iter().find(|entry| entry.uid == original.uid) {
            Some(entry) if same(entry, original) => {}
            Some(_) => {
                if !current.iter().any(|entry| same(entry, original)) {
                    let fresh = WorldbookEntry {
                        uid: u64::MAX,
                        ..original.clone()
                    };
                    kept(strict, data::upsert_worldbook_entry(root, world_id, fresh))?;
                }
            }
            None => {
                kept(
                    strict,
                    data::restore_worldbook_entry(root, world_id, original.clone()),
                )?;
            }
        }
    }

    // 3b. 這次刪掉的角色從世界書名單與來源卡拿掉（名單清空改 GM）。放在 2、3a 之後：先清會讓本次新建的
    // 條目指紋對不上而被保留。插回的快照早於這次匯入、不含這些 id，清理不會改到它們，重試時「已插回」照樣認得。
    if !gone_characters.is_empty() {
        kept(
            strict,
            data::scrub_character_ids(root, world_id, &gone_characters),
        )?;
    }
    report.entry_changes = match (book_before, data::read_worldbook(root, world_id)) {
        (Some(before), Ok(after)) => Some(undo_changes::diff(
            &before,
            &after,
            &step_deleted,
            &gone_characters,
        )),
        _ => None,
    };
    report.removed_character_ids = gone_characters;

    // 4. 機制／狀態樹：只退回這次匯入自己造成的鍵，其餘（別筆匯入或期間的正常遊玩）不動。
    if let Some(mechanism) = &receipt.mechanism {
        // 卡片變數模式：匯入加進的分支從復原當下的初始化來源那一份移除（計畫 8.4）；較早事件與其他幕
        // 種子裡的不追溯
        let tree_in_events = kept(
            strict,
            data::message_vars::edit_tree_if_events(root, world_id, |tree| {
                let before = tree.clone();
                apply_map_undo(tree, &mechanism.added_state_keys, &mechanism.restored_state);
                *tree != before
            }),
        )?
        .unwrap_or(false);
        if let Some(mut state) = kept(strict, data::read_state(root, world_id))? {
            apply_map_undo(
                &mut state.mechanism.rules,
                &mechanism.added_rule_keys,
                &mechanism.restored_rules,
            );
            apply_trigger_undo(
                &mut state.mechanism.triggers,
                &mechanism.added_trigger_ids,
                &mechanism.restored_triggers,
            );
            if let Some(before) = mechanism.incremental_before {
                state.mechanism.incremental = before;
                if !before {
                    state.mechanism.incremental_by_refactor = false;
                }
            }
            if !tree_in_events {
                apply_map_undo(
                    &mut state.state.tree,
                    &mechanism.added_state_keys,
                    &mechanism.restored_state,
                );
            }
            // AI 卡重構可能把某張新卡指定為玩家卡；退回 None，不然桌上已經沒有那張卡了，
            // player_card_id 卻還指著它，之後永遠建不了新的玩家卡。
            if mechanism.player_card_assigned {
                state.player_card_id = None;
            }
            if let Some(guide) = &mechanism.guide_before {
                state.mechanism.guide = guide.clone();
            }
            if let Some(value_types) = &mechanism.value_types_before {
                state.mechanism.value_types = value_types.clone();
            }
            if let Some(display_formats) = &mechanism.display_formats_before {
                state.mechanism.display_formats = display_formats.clone();
            }
            if let Some(display_prefixes) = &mechanism.display_prefixes_before {
                state.mechanism.display_prefixes = display_prefixes.clone();
            }
            if let Some(opening) = &mechanism.opening_before {
                state.mechanism.opening = opening.clone();
            }
            if let Some(collections) = &mechanism.collections_before {
                state.mechanism.collections = collections.clone();
            }
            if let Some(mirrors) = &mechanism.mirrors_before {
                state.mechanism.mirrors = mirrors.clone();
            }
            if let Some(retired) = mechanism.statusbar_retired_before {
                state.statusbar_retired = retired;
            }
            for (key, value) in &mechanism.removed_table {
                state
                    .state
                    .table
                    .entry(key.clone())
                    .or_insert_with(|| value.clone());
            }
            kept(strict, data::write_state(root, world_id, &state))?;
        }
    }

    // 5. 機制帳本：這次操作自己追加的那段原文整段挖掉，其餘（含期間新產生的遊玩紀錄）不動。
    if !receipt.added_ledger_lines.is_empty() {
        if let Ok(path) = data::mechanism_log_path(root, world_id) {
            if let Some(current) = kept(strict, fs::read_to_string(&path).map_err(Into::into))? {
                if let Some(index) = current.rfind(receipt.added_ledger_lines.as_str()) {
                    let mut restored = current[..index].to_owned();
                    restored.push_str(&current[index + receipt.added_ledger_lines.len()..]);
                    kept(strict, data::commit_world_write(&path, restored.as_bytes()))?;
                }
            }
        }
    }

    // 6. 這次匯入新建的卡片介面殼：匯入前就有的不動。
    if let Some(extension) = &receipt.world_card_created {
        if let Ok(card_path) = data::world_card_path(root, world_id, extension) {
            kept(strict, data::commit_world_remove(&card_path))?;
        }
    }

    // 6a. 匯完貼上檯面的開場白：一併收掉，重匯同一張卡才能乾淨地改挑一則。
    //     玩家自己先收回過就刪不到，回 false，前端不必多提一句。
    if let Some(opening) = &receipt.opening {
        report.removed_opening =
            data::remove_transcript_event(root, world_id, opening.scene, &opening.ts)
                .unwrap_or(false);
    }

    // 6b. 這次匯入新建的 GM 卡圖：匯入前就有的不動。
    if receipt.gm_image_created {
        if let Ok(image_path) = data::gm_image_path(root, world_id) {
            kept(strict, data::commit_world_remove(&image_path))?;
        }
    }

    // 8. 桌名：這次匯入有改過名字才退回去。
    if let Some(old_name) = &receipt.renamed_from {
        if kept(strict, data::rename_world(root, world_id, old_name))?.is_some() {
            report.renamed_back = true;
        }
    }

    pop_receipts(root, world_id, &receipts)?;
    // 整桌還原（計畫 8.4）：清回合紀錄、桌世代加一。重構的復原讓這桌回到照原卡玩時，卡片變數模式
    // 以當下狀態樹物化新種子、換新 epoch（重構期間的修改因此保留）。
    data::message_vars::world_swapped(root, world_id);
    if receipt.kind == "refactor" {
        let _ = data::state_commit::with_commit(root, world_id, |tx| {
            data::message_vars::ensure_active(tx, None)
        });
    }
    // 9. 這次匯入的原檔：收據已經彈出、不再被引用，刪掉後重新重構不會把撤銷掉的卡重匯回來
    if let Some(source) = &receipt.import_source {
        discard_import_source(root, world_id, &source.file);
    }
    Ok(report)
}
