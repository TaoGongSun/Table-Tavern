use super::character::{
    delete_character, read_character, write_character, CharacterCard, CharacterMeta,
};
use super::paths::{validate_single_line, world_dir};
use super::state::read_state;
use super::{invalid_data, new_id, DataResult, Tier};
use crate::ui_msg::UiMsg;
use serde::{Deserialize, Serialize};

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "characters", rename_all = "lowercase")]
pub enum Visibility {
    Gm,
    Public,
    /// 存的是角色 id
    Characters(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldbookEntry {
    pub uid: u64,
    pub title: String,
    pub keys: Vec<String>,
    pub content: String,
    pub constant: bool,
    pub order: i64,
    pub disabled: bool,
    pub visibility: Visibility,
    /// AI 卡重構切出來、玩家選擇「不升格為角色卡」的人物條目標記；一般條目一律 false。
    #[serde(default)]
    pub is_person: bool,
    /// 被 app 接管的機制條目唯讀標記；資料層只負責原樣保存。
    #[serde(default)]
    pub locked: bool,
}

fn worldbook_path(root: &Path, world_id: &str) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?.join("worldbook.json"))
}

fn empty_worldbook() -> serde_json::Value {
    serde_json::json!({ "entries": {} })
}

pub(super) fn read_worldbook_value(root: &Path, world_id: &str) -> DataResult<serde_json::Value> {
    let path = worldbook_path(root, world_id)?;
    if !path.exists() {
        return Ok(empty_worldbook());
    }
    let text = fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| invalid_data(format!("invalid worldbook JSON: {error}")))?;
    if !value
        .get("entries")
        .is_some_and(serde_json::Value::is_object)
    {
        return Err(invalid_data("worldbook entries must be an object"));
    }
    Ok(value)
}

fn write_worldbook_value(root: &Path, world_id: &str, value: &serde_json::Value) -> DataResult<()> {
    super::world_file::commit_world_write(
        &worldbook_path(root, world_id)?,
        serde_json::to_string_pretty(value)?.as_bytes(),
    )?;
    Ok(())
}

fn visibility_from_value(value: &serde_json::Value) -> Visibility {
    match value
        .get("extensions")
        .and_then(|value| value.get("table_tavern"))
        .and_then(|value| value.get("visibility"))
    {
        Some(serde_json::Value::String(value)) if value == "public" => Visibility::Public,
        Some(serde_json::Value::Object(value)) => value
            .get("characters")
            .and_then(serde_json::Value::as_array)
            .filter(|ids| ids.iter().all(serde_json::Value::is_string))
            .map(|ids| {
                Visibility::Characters(
                    ids.iter()
                        .filter_map(serde_json::Value::as_str)
                        .map(str::to_owned)
                        .collect(),
                )
            })
            .unwrap_or(Visibility::Gm),
        _ => Visibility::Gm,
    }
}

fn visibility_value(visibility: &Visibility) -> serde_json::Value {
    match visibility {
        Visibility::Gm => serde_json::Value::String("gm".to_owned()),
        Visibility::Public => serde_json::Value::String("public".to_owned()),
        Visibility::Characters(ids) => serde_json::json!({ "characters": ids }),
    }
}

fn set_visibility(value: &mut serde_json::Value, visibility: &Visibility) {
    let Some(entry) = value.as_object_mut() else {
        return;
    };
    let extensions = entry
        .entry("extensions")
        .or_insert_with(|| serde_json::json!({}));
    if !extensions.is_object() {
        *extensions = serde_json::json!({});
    }
    let extensions = extensions.as_object_mut().expect("object set above");
    let table_tavern = extensions
        .entry("table_tavern")
        .or_insert_with(|| serde_json::json!({}));
    if !table_tavern.is_object() {
        *table_tavern = serde_json::json!({});
    }
    table_tavern
        .as_object_mut()
        .expect("object set above")
        .insert("visibility".to_owned(), visibility_value(visibility));
}

/// 匯入時被判成機制鷹架而強制停用：玩家改過停用狀態就移除（見 update_entry_fields）。
const FORCED_DISABLE: &str = "forced_disable";

/// 條目的 `extensions.table_tavern` 物件（沒有就建）；條目本身不是物件時回 None。
fn table_tavern_mut(
    value: &mut serde_json::Value,
) -> Option<&mut serde_json::Map<String, serde_json::Value>> {
    let entry = value.as_object_mut()?;
    let extensions = entry
        .entry("extensions")
        .or_insert_with(|| serde_json::json!({}));
    if !extensions.is_object() {
        *extensions = serde_json::json!({});
    }
    let table_tavern = extensions
        .as_object_mut()?
        .entry("table_tavern")
        .or_insert_with(|| serde_json::json!({}));
    if !table_tavern.is_object() {
        *table_tavern = serde_json::json!({});
    }
    table_tavern.as_object_mut()
}

/// 條目 `extensions.table_tavern.<key>` 的原值。
fn table_tavern_field<'a>(
    value: &'a serde_json::Value,
    key: &str,
) -> Option<&'a serde_json::Value> {
    value
        .get("extensions")
        .and_then(|value| value.get("table_tavern"))
        .and_then(|value| value.get(key))
}

fn is_person_from_value(value: &serde_json::Value) -> bool {
    value
        .get("extensions")
        .and_then(|value| value.get("table_tavern"))
        .and_then(|value| value.get("is_person"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn set_is_person(value: &mut serde_json::Value, is_person: bool) {
    let Some(entry) = value.as_object_mut() else {
        return;
    };
    let extensions = entry
        .entry("extensions")
        .or_insert_with(|| serde_json::json!({}));
    if !extensions.is_object() {
        *extensions = serde_json::json!({});
    }
    let extensions = extensions.as_object_mut().expect("object set above");
    let table_tavern = extensions
        .entry("table_tavern")
        .or_insert_with(|| serde_json::json!({}));
    if !table_tavern.is_object() {
        *table_tavern = serde_json::json!({});
    }
    table_tavern
        .as_object_mut()
        .expect("object set above")
        .insert("is_person".to_owned(), serde_json::Value::Bool(is_person));
}

fn locked_from_value(value: &serde_json::Value) -> bool {
    value
        .get("extensions")
        .and_then(|value| value.get("table_tavern"))
        .and_then(|value| value.get("locked"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn set_locked(value: &mut serde_json::Value, locked: bool) {
    let Some(entry) = value.as_object_mut() else {
        return;
    };
    let extensions = entry
        .entry("extensions")
        .or_insert_with(|| serde_json::json!({}));
    if !extensions.is_object() {
        *extensions = serde_json::json!({});
    }
    let extensions = extensions.as_object_mut().expect("object set above");
    let table_tavern = extensions
        .entry("table_tavern")
        .or_insert_with(|| serde_json::json!({}));
    if !table_tavern.is_object() {
        *table_tavern = serde_json::json!({});
    }
    table_tavern
        .as_object_mut()
        .expect("object set above")
        .insert("locked".to_owned(), serde_json::Value::Bool(locked));
}

/// 條目的 order 給編輯器看的整數：與掃描讀取端同一條規則（有限數字照用，缺或不是數字補 100），
/// 小數四捨五入。
fn view_order(value: &serde_json::Value) -> i64 {
    value
        .get("order")
        .and_then(serde_json::Value::as_f64)
        .filter(|order| order.is_finite())
        .unwrap_or(100.0)
        .round() as i64
}

fn entry_view(value: &serde_json::Value, fallback_uid: Option<u64>) -> WorldbookEntry {
    WorldbookEntry {
        uid: value
            .get("uid")
            .and_then(serde_json::Value::as_u64)
            .or(fallback_uid)
            .unwrap_or(0),
        title: value
            .get("comment")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        keys: value
            .get("key")
            .and_then(serde_json::Value::as_array)
            .map(|keys| {
                keys.iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        content: value
            .get("content")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        constant: value
            .get("constant")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        order: view_order(value),
        disabled: value
            .get("disable")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        visibility: visibility_from_value(value),
        is_person: is_person_from_value(value),
        locked: locked_from_value(value),
    }
}

fn entries_object(
    value: &serde_json::Value,
) -> DataResult<&serde_json::Map<String, serde_json::Value>> {
    value
        .get("entries")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| invalid_data("worldbook entries must be an object"))
}

fn entries_object_mut(
    value: &mut serde_json::Value,
) -> DataResult<&mut serde_json::Map<String, serde_json::Value>> {
    value
        .get_mut("entries")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or_else(|| invalid_data("worldbook entries must be an object"))
}

fn entry_uid(key: &str, value: &serde_json::Value) -> Option<u64> {
    value
        .get("uid")
        .and_then(serde_json::Value::as_u64)
        .or_else(|| key.parse().ok())
}

fn max_uid(entries: &serde_json::Map<String, serde_json::Value>) -> Option<u64> {
    entries
        .iter()
        .filter_map(|(key, value)| entry_uid(key, value))
        .max()
}

fn next_uid(entries: &serde_json::Map<String, serde_json::Value>) -> DataResult<u64> {
    max_uid(entries)
        .map(|uid| {
            uid.checked_add(1)
                .ok_or_else(|| invalid_data("worldbook uid overflow"))
        })
        .unwrap_or(Ok(0))
}

fn sorted_entry_keys(entries: &serde_json::Map<String, serde_json::Value>) -> Vec<String> {
    let mut keys: Vec<_> = entries.keys().cloned().collect();
    keys.sort_by_key(|key| {
        let value = &entries[key];
        (
            value
                .get("displayIndex")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(u64::MAX),
            entry_uid(key, value).unwrap_or(0),
        )
    });
    keys
}

fn set_display_index(value: &mut serde_json::Value, display_index: u64) -> DataResult<()> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| invalid_data("worldbook entry must be an object"))?;
    object.insert("displayIndex".to_owned(), serde_json::json!(display_index));
    Ok(())
}

fn normalize_display_indices(
    entries: &mut serde_json::Map<String, serde_json::Value>,
    keys: &[String],
) -> DataResult<()> {
    for (index, key) in keys.iter().enumerate() {
        let display_index =
            u64::try_from(index).map_err(|_| invalid_data("worldbook displayIndex overflow"))?;
        let value = entries
            .get_mut(key)
            .ok_or_else(|| invalid_data("worldbook entry disappeared"))?;
        set_display_index(value, display_index)?;
    }
    Ok(())
}

fn update_entry_fields(value: &mut serde_json::Value, entry: &WorldbookEntry) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    object.insert("key".to_owned(), serde_json::json!(entry.keys));
    object.insert(
        "comment".to_owned(),
        serde_json::Value::String(entry.title.clone()),
    );
    object.insert(
        "content".to_owned(),
        serde_json::Value::String(entry.content.clone()),
    );
    object.insert(
        "constant".to_owned(),
        serde_json::Value::Bool(entry.constant),
    );
    // 編輯器沒改 order 就不碰原值（7.5、缺值、字串經整數視圖寫回會被截成別的數字）
    if view_order(&serde_json::Value::Object(object.clone())) != entry.order {
        object.insert("order".to_owned(), serde_json::json!(entry.order));
    }
    let disable_changed = object
        .get("disable")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
        != entry.disabled;
    object.insert(
        "disable".to_owned(),
        serde_json::Value::Bool(entry.disabled),
    );
    // 玩家（帳本開關、編輯器）改過停用狀態：匯入時強制停用的標記作廢，匯出照現值
    if disable_changed {
        if let Some(table_tavern) = table_tavern_mut(value) {
            table_tavern.remove(FORCED_DISABLE);
        }
    }
    set_visibility(value, &entry.visibility);
    set_is_person(value, entry.is_person);
    set_locked(value, entry.locked);
}

fn new_entry_value(entry: &WorldbookEntry, uid: u64, display_index: u64) -> serde_json::Value {
    let mut value = serde_json::json!({
        "uid": uid,
        "key": entry.keys,
        "keysecondary": [],
        "comment": entry.title,
        "content": entry.content,
        "constant": entry.constant,
        "vectorized": false,
        "selective": true,
        "selectiveLogic": 0,
        "addMemo": true,
        "order": entry.order,
        "position": 0,
        "disable": entry.disabled,
        "excludeRecursion": false,
        "preventRecursion": false,
        "delayUntilRecursion": false,
        "probability": 100,
        "useProbability": true,
        "depth": 4,
        "group": "",
        "groupOverride": false,
        "groupWeight": 100,
        "scanDepth": null,
        "caseSensitive": null,
        "matchWholeWords": null,
        "useGroupScoring": null,
        "automationId": "",
        "role": null,
        "sticky": 0,
        "cooldown": 0,
        "delay": 0,
        "displayIndex": display_index
    });
    set_visibility(&mut value, &entry.visibility);
    set_is_person(&mut value, entry.is_person);
    set_locked(&mut value, entry.locked);
    value
}

pub fn read_worldbook(root: &Path, world_id: &str) -> DataResult<Vec<WorldbookEntry>> {
    let value = read_worldbook_value(root, world_id)?;
    let mut entries: Vec<_> = entries_object(&value)?
        .iter()
        .map(|(key, value)| {
            let entry = entry_view(value, key.parse().ok());
            (
                value
                    .get("displayIndex")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(u64::MAX),
                entry.uid,
                entry,
            )
        })
        .collect();
    entries.sort_by_key(|(display_index, uid, _)| (*display_index, *uid));
    Ok(entries.into_iter().map(|(_, _, entry)| entry).collect())
}

pub fn upsert_worldbook_entry(
    root: &Path,
    world_id: &str,
    entry: WorldbookEntry,
) -> DataResult<u64> {
    let mut worldbook = read_worldbook_value(root, world_id)?;
    let entries = entries_object_mut(&mut worldbook)?;
    let existing_key = entries
        .iter()
        .find(|(key, value)| entry_uid(key, value) == Some(entry.uid))
        .map(|(key, _)| key.clone());
    let actual_uid = if let Some(key) = existing_key {
        let value = entries
            .get_mut(&key)
            .ok_or_else(|| invalid_data("worldbook entry disappeared"))?;
        if !value.is_object() {
            return Err(invalid_data("worldbook entry must be an object"));
        }
        update_entry_fields(value, &entry);
        entry.uid
    } else {
        let uid = next_uid(entries)?;
        insert_new_entry(entries, &entry, uid)?;
        uid
    };
    write_worldbook_value(root, world_id, &worldbook)?;
    Ok(actual_uid)
}

/// 撤銷用：把整條刪掉的條目照原 uid 插回（顯示在最前面，同 upsert 新增）。這個 uid 已經有條目
/// （先前的撤銷已插回過、或被別的條目佔走）就不動、回 false——重做撤銷不會插出第二份。
pub fn restore_worldbook_entry(
    root: &Path,
    world_id: &str,
    entry: WorldbookEntry,
) -> DataResult<bool> {
    let mut worldbook = read_worldbook_value(root, world_id)?;
    let entries = entries_object_mut(&mut worldbook)?;
    let taken = entries.contains_key(&entry.uid.to_string())
        || entries
            .iter()
            .any(|(key, value)| entry_uid(key, value) == Some(entry.uid));
    if taken {
        return Ok(false);
    }
    insert_new_entry(entries, &entry, entry.uid)?;
    write_worldbook_value(root, world_id, &worldbook)?;
    Ok(true)
}

/// 新條目插在最前面：其餘條目顯示序號往後挪一格。
fn insert_new_entry(
    entries: &mut serde_json::Map<String, serde_json::Value>,
    entry: &WorldbookEntry,
    uid: u64,
) -> DataResult<()> {
    insert_entry_value(entries, new_entry_value(entry, uid, 0), uid)
}

/// 原始條目值插在最前面（uid 照給的、displayIndex 0）：其餘條目顯示序號往後挪一格。
fn insert_entry_value(
    entries: &mut serde_json::Map<String, serde_json::Value>,
    mut value: serde_json::Value,
    uid: u64,
) -> DataResult<()> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| invalid_data("worldbook entry must be an object"))?;
    object.insert("uid".to_owned(), serde_json::json!(uid));
    object.insert("displayIndex".to_owned(), serde_json::json!(0));
    {
        let keys = sorted_entry_keys(entries);
        let has_missing_display_index = entries.values().any(|value| {
            value
                .get("displayIndex")
                .and_then(serde_json::Value::as_u64)
                .is_none()
        });
        if has_missing_display_index {
            normalize_display_indices(entries, &keys)?;
        }
        for key in keys {
            let value = entries
                .get_mut(&key)
                .ok_or_else(|| invalid_data("worldbook entry disappeared"))?;
            let display_index = value
                .get("displayIndex")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| invalid_data("worldbook displayIndex missing"))?
                .checked_add(1)
                .ok_or_else(|| invalid_data("worldbook displayIndex overflow"))?;
            set_display_index(value, display_index)?;
        }
        entries.insert(uid.to_string(), value);
    }
    Ok(())
}

/// 拖曳排序：uids 就是新的顯示順序，沒送到的條目依原順序接在後面
pub fn reorder_worldbook_entries(root: &Path, world_id: &str, uids: &[u64]) -> DataResult<()> {
    let mut worldbook = read_worldbook_value(root, world_id)?;
    let entries = entries_object_mut(&mut worldbook)?;
    let keys = sorted_entry_keys(entries);

    let mut ordered: Vec<String> = Vec::with_capacity(keys.len());
    for uid in uids {
        let Some(key) = keys
            .iter()
            .find(|key| entry_uid(key, &entries[*key]) == Some(*uid))
        else {
            continue;
        };
        if !ordered.contains(key) {
            ordered.push(key.clone());
        }
    }
    for key in &keys {
        if !ordered.contains(key) {
            ordered.push(key.clone());
        }
    }

    normalize_display_indices(entries, &ordered)?;
    write_worldbook_value(root, world_id, &worldbook)
}

pub fn delete_worldbook_entry(root: &Path, world_id: &str, uid: u64) -> DataResult<()> {
    let path = worldbook_path(root, world_id)?;
    if !path.exists() {
        return Ok(());
    }
    let mut worldbook = read_worldbook_value(root, world_id)?;
    let entries = entries_object_mut(&mut worldbook)?;
    let key = entries
        .iter()
        .find(|(key, value)| entry_uid(key, value) == Some(uid))
        .map(|(key, _)| key.clone());
    if let Some(key) = key {
        entries.remove(&key);
        write_worldbook_value(root, world_id, &worldbook)?;
    }
    Ok(())
}

/// 把世界書條目搬成可上桌的角色卡。
pub fn worldbook_entry_to_character(
    root: &Path,
    world_id: &str,
    uid: u64,
    color: String,
    as_player: bool,
) -> DataResult<CharacterMeta> {
    let worldbook = read_worldbook_value(root, world_id)?;
    let entries = entries_object(&worldbook)?;
    let entry = entries
        .iter()
        .find(|(key, value)| entry_uid(key, value) == Some(uid))
        .map(|(key, value)| entry_view(value, key.parse().ok()))
        .ok_or_else(|| {
            UiMsg::WorldbookEntryNotFound {
                uid: uid.to_string(),
            }
            .into_error()
        })?;
    if entry.title.trim().is_empty() {
        return Err(UiMsg::EntryUntitled.into_error());
    }
    validate_single_line("name", &entry.title)?;

    let state = if as_player {
        let state = read_state(root, world_id)?;
        if state.player_card_id.is_some() {
            return Err(UiMsg::PlayerCardExists.into_error());
        }
        Some(state)
    } else {
        None
    };
    let card = CharacterCard {
        id: new_id(),
        name: entry.title,
        color,
        avatar: "🎭".to_owned(),
        tier: Tier::Balanced,
        show_image: true,
        archived: false,
        gen_prompt: String::new(),
        public_md: entry.content.trim().to_owned(),
        private_md: String::new(),
    };

    write_character(root, world_id, &card)?;
    if state.is_some() {
        super::state::set_player_card(root, world_id, Some(card.id.clone()))?;
    }
    delete_worldbook_entry(root, world_id, uid)?;

    Ok(CharacterMeta {
        id: card.id,
        name: card.name,
        color: card.color,
        avatar: card.avatar,
        tier: card.tier,
        show_image: card.show_image,
        archived: card.archived,
        auto_hidden: false,
        display_index: None,
    })
}

/// 卡轉世界書時公開與私有內容之間的段標；寫入當下照介面語系，之後就是條目內文。
fn private_heading(lang: &str) -> &'static str {
    match super::lang_key(lang) {
        "zh-TW" => "## 私有",
        "zh-CN" => "## 私有",
        "ja" => "## 非公開",
        "ko" => "## 비공개",
        "es" => "## Privado",
        "pt-BR" => "## Privado",
        "de" => "## Privat",
        "fr" => "## Privé",
        "ru" => "## Приватное",
        _ => "## Private",
    }
}

/// 把角色卡搬回世界書，桌上與隱藏區的卡都可以。`lang`：介面語系，決定私有段標的語言。
/// 取獨占：回合或換幕持共用許可期間會讀卡、寫卡，交錯會點名已刪的卡或把卡檔寫回來。
pub fn character_to_worldbook_entry(
    root: &Path,
    world_id: &str,
    character_id: &str,
    lang: &str,
) -> DataResult<()> {
    let Some(_lock) = super::world_lock::try_world_exclusive(world_id) else {
        return Err(UiMsg::WorldBusy.into_error());
    };
    let card = read_character(root, world_id, character_id)?;
    let state = read_state(root, world_id)?;
    if state.player_card_id.as_deref() == Some(character_id) {
        return Err(UiMsg::PlayerCardNotConvertible.into_error());
    }

    let content = match (card.public_md.is_empty(), card.private_md.is_empty()) {
        (false, false) => format!(
            "{}\n\n{}\n{}",
            card.public_md,
            private_heading(lang),
            card.private_md
        ),
        (false, true) => card.public_md,
        (true, false) => card.private_md,
        (true, true) => String::new(),
    };
    let entry = WorldbookEntry {
        uid: 0,
        title: card.name,
        keys: Vec::new(),
        content,
        constant: true,
        order: 100,
        disabled: false,
        visibility: Visibility::Gm,
        is_person: false,
        locked: false,
    };
    let mut worldbook = read_worldbook_value(root, world_id)?;
    let entries = entries_object_mut(&mut worldbook)?;
    let uid = next_uid(entries)?;
    insert_new_entry(entries, &entry, uid)?;
    write_worldbook_value(root, world_id, &worldbook)?;
    delete_character(root, world_id, character_id)
}

pub fn export_worldbook(root: &Path, world_id: &str, path: &Path) -> DataResult<()> {
    crate::data::refuse_if_updating()?;
    let source = worldbook_path(root, world_id)?;
    if source.exists() {
        // world-write-exempt: 寫到玩家選定的匯出路徑，不是桌目錄
        fs::copy(source, path)?;
    } else {
        // world-write-exempt: 寫到玩家選定的匯出路徑，不是桌目錄
        fs::write(path, serde_json::to_string_pretty(&empty_worldbook())?)?;
    }
    Ok(())
}

mod book_import;
mod raw_entries;

#[cfg(test)]
pub use book_import::import_worldbook;
pub use book_import::{
    dedupe_worldbook, import_worldbook_as, BookImport, BookOwner, VisibilityRestore,
    WorldbookImport,
};
pub use raw_entries::{
    apply_visibility_restore, character_book_raw_entries, identity_fingerprint,
    identity_fingerprints, insert_worldbook_entry_raw, read_worldbook_raw,
    restore_deleted_entry_raw, set_entry_flags, set_source_cards, source_cards_of,
    worldbook_entry_value, RestoreOutcome,
};

#[cfg(test)]
mod book_import_tests;
#[cfg(test)]
mod tests;
