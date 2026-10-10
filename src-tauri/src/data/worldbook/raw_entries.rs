//! 原始條目 JSON 的讀寫：保留 ST 欄位（次要鍵、位置、大小寫…）與 table_tavern 擴充欄位，
//! 給重構套用、撤銷插回、角色卡匯出這些不能只靠精簡 `WorldbookEntry` 的地方用。
use crate::data::state_commit::with_commit;

use super::super::{invalid_data, DataResult};
use super::book_import::{identity_text, source_cards, VisibilityRestore, SOURCE_CARDS};
use super::{
    entries_object, entries_object_mut, entry_uid, insert_entry_value, new_entry_value, next_uid,
    read_worldbook_value, set_is_person, set_locked, sorted_entry_keys, table_tavern_field,
    table_tavern_mut, visibility_from_value, write_worldbook_value, Visibility, WorldbookEntry,
};
use std::collections::BTreeMap;
use std::path::Path;

/// 整桌原始條目，uid → 原始值。
pub fn read_worldbook_raw(
    root: &Path,
    world_id: &str,
) -> DataResult<BTreeMap<u64, serde_json::Value>> {
    let value = read_worldbook_value(root, world_id)?;
    Ok(entries_object(&value)?
        .iter()
        .filter_map(|(key, value)| Some((entry_uid(key, value)?, value.clone())))
        .collect())
}

/// 精簡條目展開成新條目的原始值（ST 欄位用預設值）；uid／displayIndex 由插入時配發。
pub fn worldbook_entry_value(entry: &WorldbookEntry) -> serde_json::Value {
    new_entry_value(entry, 0, 0)
}

/// 插入一條原始值條目（顯示在最前面），uid 配發 max+1，回傳實際 uid。
pub fn insert_worldbook_entry_raw(
    root: &Path,
    world_id: &str,
    value: serde_json::Value,
) -> DataResult<u64> {
    with_commit(root, world_id, |_| {
        let mut worldbook = read_worldbook_value(root, world_id)?;
        let entries = entries_object_mut(&mut worldbook)?;
        let uid = next_uid(entries)?;
        insert_entry_value(entries, value, uid)?;
        write_worldbook_value(root, world_id, &worldbook)?;
        Ok(uid)
    })
}

/// 兩條原始值除了 uid、displayIndex 以外完全相同（比解析後的值，欄位順序不算差異）。
fn same_raw(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    let strip = |value: &serde_json::Value| {
        let mut value = value.clone();
        if let Some(object) = value.as_object_mut() {
            object.remove("uid");
            object.remove("displayIndex");
        }
        value
    };
    strip(a) == strip(b)
}

/// 撤銷用：把這次操作整條刪掉的條目照原始值插回，可安全重做——原 uid 空著就照原 uid 插回；
/// 原 uid 上已是除 uid／displayIndex 外全部相同的條目＝先前已插回，跳過；原 uid 被別的條目佔走才
/// 換新 uid 插，且全部相同的條目已在就跳過。
pub fn restore_deleted_entry_raw(
    root: &Path,
    world_id: &str,
    raw: &serde_json::Value,
) -> DataResult<()> {
    with_commit(root, world_id, |_| {
        let uid = raw
            .get("uid")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| invalid_data("deleted worldbook entry has no uid"))?;
        let mut worldbook = read_worldbook_value(root, world_id)?;
        let entries = entries_object_mut(&mut worldbook)?;
        let at_uid = entries
            .iter()
            .find(|(key, value)| entry_uid(key, value) == Some(uid))
            .map(|(_, value)| value.clone());
        let target = match at_uid {
            Some(current) if same_raw(&current, raw) => return Ok(()),
            Some(_) => {
                if entries.values().any(|value| same_raw(value, raw)) {
                    return Ok(());
                }
                next_uid(entries)?
            }
            None => uid,
        };
        insert_entry_value(entries, raw.clone(), target)?;
        write_worldbook_value(root, world_id, &worldbook)
    })
}

/// 可見度還原的結果：還原了／目前值已被改過（玩家改的，不動）／條目已不在。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreOutcome {
    Restored,
    Kept,
    Gone,
}

/// 撤銷匯入時把被合併改寫的既有條目的 `visibility`、`source_cards` 還原；目前值不等於收據記的
/// after（玩家改過或按過清重複）就不動。其餘欄位一律不碰。
pub fn apply_visibility_restore(
    root: &Path,
    world_id: &str,
    restore: &VisibilityRestore,
) -> DataResult<RestoreOutcome> {
    with_commit(root, world_id, |_| {
        let mut worldbook = read_worldbook_value(root, world_id)?;
        let entries = entries_object_mut(&mut worldbook)?;
        let Some(value) = entries
            .iter_mut()
            .find(|(key, value)| entry_uid(key, value) == Some(restore.uid))
            .map(|(_, value)| value)
        else {
            return Ok(RestoreOutcome::Gone);
        };
        if table_tavern_field(value, "visibility") != restore.after_visibility.as_ref()
            || table_tavern_field(value, SOURCE_CARDS) != restore.after_cards.as_ref()
        {
            return Ok(RestoreOutcome::Kept);
        }
        if let Some(table_tavern) = table_tavern_mut(value) {
            for (key, before) in [
                ("visibility", &restore.before_visibility),
                (SOURCE_CARDS, &restore.before_cards),
            ] {
                match before {
                    Some(before) => table_tavern.insert(key.to_owned(), before.clone()),
                    None => table_tavern.remove(key),
                };
            }
        }
        write_worldbook_value(root, world_id, &worldbook)?;
        Ok(RestoreOutcome::Restored)
    })
}

/// 條目的身分指紋（標題、內文、主鍵、次要鍵、常駐的雜湊）：重構產物核對來源條目用。
/// 不含停用、順序、可見度、來源卡等玩家會改的欄位。
pub fn identity_fingerprint(value: &serde_json::Value) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in identity_text(value).as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// 整桌條目的身分指紋，uid 字串 → 指紋。
pub fn identity_fingerprints(root: &Path, world_id: &str) -> DataResult<BTreeMap<String, String>> {
    Ok(read_worldbook_raw(root, world_id)?
        .into_iter()
        .map(|(uid, value)| (uid.to_string(), identity_fingerprint(&value)))
        .collect())
}

/// 跟著這張角色卡匯出的條目（原始值，照顯示順序）：可見度名單含這張卡，或來源卡含這張卡。
pub fn character_book_raw_entries(
    root: &Path,
    world_id: &str,
    character_id: &str,
) -> DataResult<Vec<serde_json::Value>> {
    let value = read_worldbook_value(root, world_id)?;
    let entries = entries_object(&value)?;
    Ok(sorted_entry_keys(entries)
        .into_iter()
        .map(|key| &entries[&key])
        .filter(|entry| {
            matches!(visibility_from_value(entry), Visibility::Characters(ids) if ids.iter().any(|id| id == character_id))
                || source_cards(entry).iter().any(|id| id == character_id)
        })
        .cloned()
        .collect())
}

/// 條目記的來源卡。
pub fn source_cards_of(value: &serde_json::Value) -> Vec<String> {
    source_cards(value)
}

/// 寫入來源卡；空清單就移除欄位。
pub fn set_source_cards(value: &mut serde_json::Value, cards: &[String]) {
    if let Some(table_tavern) = table_tavern_mut(value) {
        if cards.is_empty() {
            table_tavern.remove(SOURCE_CARDS);
        } else {
            table_tavern.insert(SOURCE_CARDS.to_owned(), serde_json::json!(cards));
        }
    }
}

/// 寫入套用端判定的旗標（唯讀機制條目、人物條目），蓋掉原始值裡的舊值。
pub fn set_entry_flags(value: &mut serde_json::Value, locked: bool, is_person: bool) {
    set_locked(value, locked);
    set_is_person(value, is_person);
}
