//! 刪角色後清掉世界書裡的角色 id：可見度名單（`visibility.characters`）與來源卡（`source_cards`）。
//! 名單清空改給 GM〔作者裁決 2026-10-11〕。同一套規則也套在收據的前後值與條目快照上（receipts）。
use crate::data::state_commit::with_commit;

use super::super::DataResult;
use super::book_import::{VisibilityRestore, SOURCE_CARDS};
use super::{
    entries_object_mut, entry_uid, entry_view, read_worldbook_value, table_tavern_field,
    table_tavern_mut, update_entry_fields, write_worldbook_value_atomic, Visibility,
    WorldbookEntry,
};
use std::collections::BTreeMap;
use std::path::Path;

/// 可見度欄位原值套清理：只動讀得成角色名單（物件形、陣列全字串）且含要清的 id 的；
/// 清完空了寫成 `"gm"`。沒變回 None。
pub fn scrub_visibility_field(
    field: &serde_json::Value,
    ids: &[String],
) -> Option<serde_json::Value> {
    let list = field.get("characters")?.as_array()?;
    if !list.iter().all(serde_json::Value::is_string) {
        return None;
    }
    let hit =
        |item: &serde_json::Value| item.as_str().is_some_and(|id| ids.iter().any(|x| x == id));
    if !list.iter().any(hit) {
        return None;
    }
    let kept: Vec<serde_json::Value> = list.iter().filter(|item| !hit(item)).cloned().collect();
    Some(if kept.is_empty() {
        serde_json::Value::String("gm".to_owned())
    } else {
        let mut object = field.as_object()?.clone();
        object.insert("characters".to_owned(), serde_json::Value::Array(kept));
        serde_json::Value::Object(object)
    })
}

/// 來源卡欄位原值套清理：陣列裡拿掉要清的 id，其餘元素照留；空了回 `Some(None)`＝移除欄位。
/// 沒變回 None。
pub fn scrub_cards_field(
    field: &serde_json::Value,
    ids: &[String],
) -> Option<Option<serde_json::Value>> {
    let list = field.as_array()?;
    let hit =
        |item: &serde_json::Value| item.as_str().is_some_and(|id| ids.iter().any(|x| x == id));
    if !list.iter().any(hit) {
        return None;
    }
    let kept: Vec<serde_json::Value> = list.iter().filter(|item| !hit(item)).cloned().collect();
    Some((!kept.is_empty()).then_some(serde_json::Value::Array(kept)))
}

/// 一條原始條目套清理，回傳有沒有變。
pub fn scrub_entry_value(value: &mut serde_json::Value, ids: &[String]) -> bool {
    let visibility = table_tavern_field(value, "visibility")
        .and_then(|field| scrub_visibility_field(field, ids));
    let cards =
        table_tavern_field(value, SOURCE_CARDS).and_then(|field| scrub_cards_field(field, ids));
    if visibility.is_none() && cards.is_none() {
        return false;
    }
    let Some(table_tavern) = table_tavern_mut(value) else {
        return false;
    };
    if let Some(visibility) = visibility {
        table_tavern.insert("visibility".to_owned(), visibility);
    }
    match cards {
        Some(Some(cards)) => {
            table_tavern.insert(SOURCE_CARDS.to_owned(), cards);
        }
        Some(None) => {
            table_tavern.remove(SOURCE_CARDS);
        }
        None => {}
    }
    true
}

/// 精簡條目（收據快照）套清理：名單拿掉要清的 id，空了改 `Gm`。回傳有沒有變。
pub fn scrub_entry(entry: &mut WorldbookEntry, ids: &[String]) -> bool {
    let Visibility::Characters(list) = &entry.visibility else {
        return false;
    };
    if !list.iter().any(|id| ids.contains(id)) {
        return false;
    }
    let kept: Vec<String> = list
        .iter()
        .filter(|id| !ids.contains(id))
        .cloned()
        .collect();
    entry.visibility = if kept.is_empty() {
        Visibility::Gm
    } else {
        Visibility::Characters(kept)
    };
    true
}

/// 原始條目的精簡檢視（收據指紋比對用）。
pub fn entry_view_of(value: &serde_json::Value) -> WorldbookEntry {
    entry_view(value, None)
}

/// 精簡條目的欄位蓋到原始條目上（同 `upsert_worldbook_entry` 對既有條目的效果）。
pub fn apply_entry_fields(value: &mut serde_json::Value, entry: &WorldbookEntry) {
    update_entry_fields(value, entry);
}

/// 收據記的 after 等於這條目前的 `visibility` 與 `source_cards`（原始值相等）：撤銷時才會還原。
pub fn restore_after_matches(value: &serde_json::Value, restore: &VisibilityRestore) -> bool {
    table_tavern_field(value, "visibility") == restore.after_visibility.as_ref()
        && table_tavern_field(value, SOURCE_CARDS) == restore.after_cards.as_ref()
}

/// 把收據記的 before 寫回條目；before 為 None＝移除該欄位（可見度因此讀成 `Gm`）。
pub fn apply_restore_before(value: &mut serde_json::Value, restore: &VisibilityRestore) {
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
}

/// 收據的可見度還原前後值套清理，回傳有沒有變。
pub fn scrub_restore(restore: &mut VisibilityRestore, ids: &[String]) -> bool {
    let mut changed = false;
    for field in [
        &mut restore.before_visibility,
        &mut restore.after_visibility,
    ] {
        if let Some(scrubbed) = field
            .as_ref()
            .and_then(|value| scrub_visibility_field(value, ids))
        {
            *field = Some(scrubbed);
            changed = true;
        }
    }
    for field in [&mut restore.before_cards, &mut restore.after_cards] {
        if let Some(scrubbed) = field
            .as_ref()
            .and_then(|value| scrub_cards_field(value, ids))
        {
            *field = scrubbed;
            changed = true;
        }
    }
    changed
}

/// 清理的結果：清理前整本書的原始條目（uid → 原始值），以及有變動的 uid。
#[derive(Debug, Default)]
pub struct CharacterScrub {
    pub before: BTreeMap<u64, serde_json::Value>,
    pub changed: Vec<u64>,
}

/// 從整本世界書拿掉這些角色 id；有變動才整檔原子寫。在這桌的短提交鎖內讀改寫。
pub fn scrub_character_ids(
    root: &Path,
    world_id: &str,
    ids: &[String],
) -> DataResult<CharacterScrub> {
    with_commit(root, world_id, |_| {
        let mut worldbook = read_worldbook_value(root, world_id)?;
        let entries = entries_object_mut(&mut worldbook)?;
        let mut scrub = CharacterScrub::default();
        for (key, value) in entries.iter_mut() {
            let Some(uid) = entry_uid(key, value) else {
                continue;
            };
            scrub.before.insert(uid, value.clone());
            if scrub_entry_value(value, ids) {
                scrub.changed.push(uid);
            }
        }
        if !scrub.changed.is_empty() {
            write_worldbook_value_atomic(root, world_id, &worldbook)?;
        }
        Ok(scrub)
    })
}
