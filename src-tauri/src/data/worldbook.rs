use crate::mechanism::{Record, RecordKind};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use super::{DataResult, Tier, invalid_data, new_id};
use super::character::{CharacterCard, CharacterMeta, delete_character, read_character, write_character};
use super::paths::{validate_single_line, world_dir};
use super::state::{read_state, write_state};

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
    fs::write(
        worldbook_path(root, world_id)?,
        serde_json::to_string_pretty(value)?,
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
        order: value
            .get("order")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0),
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
    object.insert("order".to_owned(), serde_json::json!(entry.order));
    object.insert(
        "disable".to_owned(),
        serde_json::Value::Bool(entry.disabled),
    );
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
        entries.insert(uid.to_string(), new_entry_value(&entry, uid, 0));
        uid
    };
    write_worldbook_value(root, world_id, &worldbook)?;
    Ok(actual_uid)
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
        .ok_or_else(|| invalid_data("找不到世界書條目"))?;
    if entry.title.trim().is_empty() {
        return Err(invalid_data("條目沒有標題，先給標題再轉"));
    }
    validate_single_line("name", &entry.title)?;

    let mut state = if as_player {
        let state = read_state(root, world_id)?;
        if state.player_card_id.is_some() {
            return Err(invalid_data("這桌已經有玩家卡"));
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
    if let Some(state) = state.as_mut() {
        state.player_card_id = Some(card.id.clone());
        write_state(root, world_id, state)?;
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

/// 把封存角色卡搬回世界書。
pub fn character_to_worldbook_entry(
    root: &Path,
    world_id: &str,
    character_id: &str,
) -> DataResult<()> {
    let card = read_character(root, world_id, character_id)?;
    if !card.archived {
        return Err(invalid_data("這張卡還在桌上"));
    }
    let state = read_state(root, world_id)?;
    if state.player_card_id.as_deref() == Some(character_id) {
        return Err(invalid_data("玩家卡不能轉"));
    }

    let content = match (card.public_md.is_empty(), card.private_md.is_empty()) {
        (false, false) => format!("{}\n\n## 私有\n{}", card.public_md, card.private_md),
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
    entries.insert(uid.to_string(), new_entry_value(&entry, uid, 0));
    write_worldbook_value(root, world_id, &worldbook)?;
    delete_character(root, world_id, character_id)
}

fn normalize_imported_entry(
    mut value: serde_json::Value,
    character_book: bool,
    uid: u64,
) -> DataResult<serde_json::Value> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| invalid_data("worldbook entry must be an object"))?;
    if character_book {
        if let Some(keys) = object.remove("keys") {
            object.insert("key".to_owned(), keys);
        }
        if let Some(keys) = object.remove("secondary_keys") {
            object.insert("keysecondary".to_owned(), keys);
        }
        if let Some(order) = object.remove("insertion_order") {
            object.insert("order".to_owned(), order);
        }
        if let Some(enabled) = object.remove("enabled") {
            let enabled = enabled
                .as_bool()
                .ok_or_else(|| invalid_data("character_book enabled must be a boolean"))?;
            object.insert("disable".to_owned(), serde_json::Value::Bool(!enabled));
        }
    }
    object.insert("uid".to_owned(), serde_json::json!(uid));
    let has_visibility = value
        .get("extensions")
        .and_then(|value| value.get("table_tavern"))
        .and_then(|value| value.get("visibility"))
        .is_some();
    if !has_visibility {
        set_visibility(&mut value, &Visibility::Gm);
    }
    if is_mechanism_scaffold(&value) {
        if let Some(object) = value.as_object_mut() {
            object.insert("disable".to_owned(), serde_json::Value::Bool(true));
        }
    }
    Ok(value)
}

/// 機制鷹架條目：`[initvar]`／`[mvu_update]` 規則表、原生 EJS 腳本，或 ST 把整棵變數樹塞回提示詞的巨集。
/// 本地已接管或原本就不會交給模型的內容，不該再送進模型上下文燒字數。
fn is_mechanism_scaffold(entry: &serde_json::Value) -> bool {
    let marker = entry
        .get("comment")
        .and_then(serde_json::Value::as_str)
        .or_else(|| entry.get("title").and_then(serde_json::Value::as_str))
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if marker.starts_with("[initvar]") || marker.starts_with("[mvu_update]") {
        return true;
    }
    entry
        .get("content")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|content| {
            content.contains("{{format_message_variable::") || content.contains("<%")
        })
}

/// 條目的實質內容指紋：同一份世界書重複匯入時用它認出「一模一樣的條目」。
/// 只看標題、內文與兩組關鍵字——uid、順序、可見度等隨匯入產生的欄位不算差異。
fn entry_fingerprint(entry: &serde_json::Value) -> String {
    let text = |field: &str| {
        entry
            .get(field)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_owned()
    };
    let keys = |field: &str| {
        let mut items: Vec<String> = entry
            .get(field)
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(|key| key.trim().to_owned())
                    .collect()
            })
            .unwrap_or_default();
        items.sort();
        items.join("\u{1f}")
    };
    format!(
        "{}\u{1e}{}\u{1e}{}\u{1e}{}",
        text("comment"),
        text("content"),
        keys("key"),
        keys("keysecondary"),
    )
}

/// 匯入結果：`imported`＝真的寫進去的條數，`skipped`＝內容重複被略過的條數。
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct WorldbookImport {
    pub imported: usize,
    pub skipped: usize,
}

pub fn import_worldbook(
    root: &Path,
    world_id: &str,
    json_text: &str,
) -> DataResult<WorldbookImport> {
    let imported: serde_json::Value = serde_json::from_str(json_text)
        .map_err(|error| invalid_data(format!("invalid worldbook JSON: {error}")))?;
    let source = imported
        .get("entries")
        .ok_or_else(|| invalid_data("imported worldbook is missing entries"))?;
    let (source_entries, character_book): (Vec<serde_json::Value>, bool) = match source {
        serde_json::Value::Object(entries) => (entries.values().cloned().collect(), false),
        serde_json::Value::Array(entries) => (entries.clone(), true),
        _ => {
            return Err(invalid_data(
                "imported worldbook entries must be an object or array",
            ));
        }
    };

    let mut worldbook = read_worldbook_value(root, world_id)?;
    let entries = entries_object_mut(&mut worldbook)?;
    let total = source_entries.len();
    let mut seen: HashSet<String> = entries.values().map(entry_fingerprint).collect();
    let mut uid = next_uid(entries)?;
    let mut imported = 0;
    let mut absorbed = Vec::new();
    for source_entry in source_entries {
        let entry = normalize_imported_entry(source_entry, character_book, uid)?;
        // 已經有一模一樣的條目就跳過，重複匯入同一份書不會塞出兩套內容
        if !seen.insert(entry_fingerprint(&entry)) {
            continue;
        }
        if is_mechanism_scaffold(&entry) {
            let title = entry
                .get("comment")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned();
            absorbed.push(Record {
                kind: RecordKind::Absorbed,
                path: title,
                detail: "機制鷹架條目，已由本地機制接管，不再送入提示詞。".to_owned(),
            });
        }
        entries.insert(uid.to_string(), entry);
        uid = uid
            .checked_add(1)
            .ok_or_else(|| invalid_data("worldbook uid overflow"))?;
        imported += 1;
    }
    write_worldbook_value(root, world_id, &worldbook)?;
    if !absorbed.is_empty() {
        let scene = read_state(root, world_id)
            .map(|state| state.current_scene)
            .unwrap_or(0);
        crate::mechanism::append_log(root, world_id, scene, &absorbed);
    }
    Ok(WorldbookImport {
        imported,
        skipped: total - imported,
    })
}

/// 清掉內容重複的條目：同一份指紋只留顯示順序最前的那條，回傳刪掉幾條。
/// 給去重上線前就已經重複匯入的桌收拾用。
pub fn dedupe_worldbook(root: &Path, world_id: &str) -> DataResult<usize> {
    let mut worldbook = read_worldbook_value(root, world_id)?;
    let entries = entries_object_mut(&mut worldbook)?;
    let mut seen = HashSet::new();
    let duplicates: Vec<String> = sorted_entry_keys(entries)
        .into_iter()
        .filter(|key| {
            entries
                .get(key)
                .is_some_and(|entry| !seen.insert(entry_fingerprint(entry)))
        })
        .collect();
    for key in &duplicates {
        entries.remove(key);
    }
    if !duplicates.is_empty() {
        write_worldbook_value(root, world_id, &worldbook)?;
    }
    Ok(duplicates.len())
}

pub fn export_worldbook(root: &Path, world_id: &str, path: &Path) -> DataResult<()> {
    let source = worldbook_path(root, world_id)?;
    if source.exists() {
        fs::copy(source, path)?;
    } else {
        fs::write(path, serde_json::to_string_pretty(&empty_worldbook())?)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::*;
    use crate::data::test_support::*;

    #[test]
    fn worldbook_missing_returns_empty_and_invalid_json_errors() {
        let root = TestRoot::new("worldbook-missing");
        let world_id = create_world(root.path(), "舊桌").unwrap();
        assert_eq!(read_worldbook(root.path(), &world_id).unwrap(), Vec::new());
        assert_eq!(
            serde_json::to_value(Visibility::Gm).unwrap(),
            serde_json::json!({"type": "gm"})
        );
        assert_eq!(
            serde_json::to_value(Visibility::Characters(vec!["角色代碼".to_owned()])).unwrap(),
            serde_json::json!({"type": "characters", "characters": ["角色代碼"]})
        );

        fs::write(
            root.path()
                .join(format!("worlds/{world_id}/worldbook.json")),
            "{broken",
        )
        .unwrap();
        assert!(read_worldbook(root.path(), &world_id).is_err());
    }

    #[test]
    fn imports_st_worldbook_losslessly_and_round_trips_export() {
        let root = TestRoot::new("worldbook-st-import");
        let source = create_world(root.path(), "來源").unwrap();
        let imported = serde_json::json!({
            "entries": {
                "7": {
                    "uid": 7,
                    "key": ["dragon", "wyrm"],
                    "comment": "龍",
                    "content": "古龍沉睡於山下。",
                    "constant": false,
                    "order": 20,
                    "disable": false,
                    "sticky": 4,
                    "probability": 37
                },
                "9": {
                    "uid": 9,
                    "key": [],
                    "comment": "王都",
                    "content": "王都戒嚴。",
                    "constant": true,
                    "order": 5,
                    "disable": false,
                    "extensions": {
                        "foreign_app": {"kept": true},
                        "table_tavern": {"visibility": "public"}
                    }
                }
            }
        });
        assert_eq!(
            import_worldbook(root.path(), &source, &imported.to_string())
                .unwrap()
                .imported,
            2
        );

        let entries = read_worldbook(root.path(), &source).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].uid, 0);
        assert_eq!(entries[0].title, "龍");
        assert_eq!(entries[0].keys, ["dragon", "wyrm"]);
        assert_eq!(entries[0].visibility, Visibility::Gm);
        assert_eq!(entries[1].uid, 1);
        assert_eq!(entries[1].visibility, Visibility::Public);

        let raw: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(root.path().join(format!("worlds/{source}/worldbook.json")))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(raw["entries"]["0"]["sticky"], 4);
        assert_eq!(raw["entries"]["0"]["probability"], 37);
        assert_eq!(
            raw["entries"]["0"]["extensions"]["table_tavern"]["visibility"],
            "gm"
        );
        assert_eq!(
            raw["entries"]["1"]["extensions"]["foreign_app"]["kept"],
            true
        );

        let exported = root.path().join("exported-worldbook.json");
        export_worldbook(root.path(), &source, &exported).unwrap();
        let destination = create_world(root.path(), "目的").unwrap();
        let exported_text = fs::read_to_string(exported).unwrap();
        assert_eq!(
            import_worldbook(root.path(), &destination, &exported_text)
                .unwrap()
                .imported,
            entries.len()
        );
        assert_eq!(
            read_worldbook(root.path(), &destination).unwrap().len(),
            entries.len()
        );
    }

    #[test]
    fn import_skips_entries_identical_to_existing_ones() {
        let root = TestRoot::new("worldbook-dedupe");
        let world_id = create_world(root.path(), "世界").unwrap();
        let book = serde_json::json!({
            "entries": {
                "0": {
                    "uid": 0,
                    "key": ["城門", "夜"],
                    "comment": "城門",
                    "content": "城門已關。",
                    "constant": false,
                    "order": 1,
                    "disable": false
                },
                "1": {
                    "uid": 1,
                    "key": ["市集"],
                    "comment": "市集",
                    "content": "市集喧鬧。",
                    "constant": false,
                    "order": 2,
                    "disable": false
                }
            }
        });
        let first = import_worldbook(root.path(), &world_id, &book.to_string()).unwrap();
        assert_eq!(
            first,
            WorldbookImport {
                imported: 2,
                skipped: 0
            }
        );

        // 同一份書再匯一次：內容一模一樣，全部略過
        let again = import_worldbook(root.path(), &world_id, &book.to_string()).unwrap();
        assert_eq!(
            again,
            WorldbookImport {
                imported: 0,
                skipped: 2
            }
        );
        assert_eq!(read_worldbook(root.path(), &world_id).unwrap().len(), 2);

        // 關鍵字順序不同、內文前後有空白＝同一條；改過內文的才算新條目
        let mixed = serde_json::json!({
            "entries": {
                "0": {
                    "uid": 0,
                    "key": ["夜", "城門"],
                    "comment": "城門",
                    "content": "  城門已關。  ",
                    "constant": false,
                    "order": 1,
                    "disable": false
                },
                "1": {
                    "uid": 1,
                    "key": ["市集"],
                    "comment": "市集",
                    "content": "市集已散。",
                    "constant": false,
                    "order": 2,
                    "disable": false
                }
            }
        });
        let third = import_worldbook(root.path(), &world_id, &mixed.to_string()).unwrap();
        assert_eq!(
            third,
            WorldbookImport {
                imported: 1,
                skipped: 1
            }
        );
        assert_eq!(read_worldbook(root.path(), &world_id).unwrap().len(), 3);
    }

    /// 機制鷹架條目（[initvar]／[mvu_update]／整棵樹重送巨集）匯入後要被系統關掉，
    /// 不再送模型；一般條目完全不受影響。
    #[test]
    fn import_worldbook_disables_mechanism_scaffold_entries_and_leaves_others_alone() {
        let root = TestRoot::new("worldbook-absorb");
        let world_id = create_world(root.path(), "世界").unwrap();
        let book = serde_json::json!({
            "entries": [
                {
                    "keys": ["初始"],
                    "comment": "[initvar] 初始值",
                    "content": "World:\n  Time: 清晨",
                    "enabled": false
                },
                {
                    "keys": [],
                    "comment": "[mvu_update] 規則",
                    "content": "规则:\n  World:\n    HP:\n      type: number",
                    "enabled": true
                },
                {
                    "keys": [],
                    "comment": "整棵樹重送",
                    "content": "{{format_message_variable::World}}",
                    "enabled": true
                },
                {
                    "keys": ["城門"],
                    "comment": "城門",
                    "content": "城門已關。",
                    "enabled": true
                }
            ]
        });
        import_worldbook(root.path(), &world_id, &book.to_string()).unwrap();
        let entries = read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(entries.len(), 4);
        for entry in &entries {
            let should_be_disabled = entry.title != "城門";
            assert_eq!(entry.disabled, should_be_disabled, "{}", entry.title);
        }
    }

    #[test]
    fn dedupe_keeps_first_of_each_duplicate_group() {
        let root = TestRoot::new("worldbook-dedupe-command");
        let world_id = create_world(root.path(), "世界").unwrap();
        let entry = |uid: u64, comment: &str, content: &str, order: u64| {
            serde_json::json!({
                "uid": uid,
                "key": ["k"],
                "comment": comment,
                "content": content,
                "constant": false,
                "order": order,
                "disable": false
            })
        };
        let book = serde_json::json!({
            "entries": {
                "0": entry(0, "城門", "城門已關。", 1),
                "1": entry(1, "市集", "市集喧鬧。", 2),
                "2": entry(2, "城門", "城門已關。", 3),
                "3": entry(3, "城門", "城門大開。", 4)
            }
        });
        write_worldbook_value(root.path(), &world_id, &book).unwrap();

        assert_eq!(dedupe_worldbook(root.path(), &world_id).unwrap(), 1);
        let entries = read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(entries.len(), 3);
        // 留下的是排在最前面那條，被留下的內容一條不少
        assert_eq!(entries[0].uid, 0);
        assert_eq!(entries[1].uid, 1);
        assert_eq!(entries[2].content, "城門大開。");
        // 再按一次沒東西可清
        assert_eq!(dedupe_worldbook(root.path(), &world_id).unwrap(), 0);
    }

    #[test]
    fn imports_character_book_mapping_and_appends_unique_uids() {
        let root = TestRoot::new("worldbook-character-book");
        let world_id = create_world(root.path(), "世界").unwrap();
        let first = serde_json::json!({
            "entries": {
                "12": {
                    "uid": 12,
                    "key": ["existing"],
                    "comment": "既有",
                    "content": "內容",
                    "constant": false,
                    "order": 1,
                    "disable": false
                }
            }
        });
        import_worldbook(root.path(), &world_id, &first.to_string()).unwrap();

        let character_book = serde_json::json!({
            "entries": [
                {
                    "keys": ["gate"],
                    "secondary_keys": ["night"],
                    "comment": "城門",
                    "content": "城門已關。",
                    "constant": false,
                    "insertion_order": 42,
                    "enabled": false,
                    "priority": 8
                },
                {
                    "keys": ["market"],
                    "comment": "市集",
                    "content": "市集喧鬧。",
                    "constant": false,
                    "insertion_order": 43,
                    "enabled": true
                }
            ]
        });
        assert_eq!(
            import_worldbook(root.path(), &world_id, &character_book.to_string())
                .unwrap()
                .imported,
            2
        );

        let entries = read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(
            entries.iter().map(|entry| entry.uid).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert_eq!(entries[1].keys, ["gate"]);
        assert_eq!(entries[1].order, 42);
        assert!(entries[1].disabled);
        assert!(!entries[2].disabled);

        let raw: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(
                root.path()
                    .join(format!("worlds/{world_id}/worldbook.json")),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(raw["entries"]["1"]["keysecondary"][0], "night");
        assert_eq!(raw["entries"]["1"]["priority"], 8);
        assert!(raw["entries"]["1"].get("keys").is_none());
        assert!(raw["entries"]["1"].get("enabled").is_none());
    }

    #[test]
    fn upsert_preserves_unknown_fields_allocates_uid_and_deletes() {
        let root = TestRoot::new("worldbook-upsert");
        let world_id = create_world(root.path(), "世界").unwrap();
        let imported = serde_json::json!({
            "entries": {
                "5": {
                    "uid": 5,
                    "key": ["old"],
                    "comment": "舊標題",
                    "content": "舊內容",
                    "constant": false,
                    "order": 1,
                    "disable": false,
                    "sticky": 99
                }
            }
        });
        import_worldbook(root.path(), &world_id, &imported.to_string()).unwrap();

        let mut updated = worldbook_entry(0, "新標題");
        updated.visibility = Visibility::Characters(vec!["角色代碼".to_owned()]);
        assert_eq!(
            upsert_worldbook_entry(root.path(), &world_id, updated.clone()).unwrap(),
            0
        );
        let raw: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(
                root.path()
                    .join(format!("worlds/{world_id}/worldbook.json")),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(raw["entries"]["0"]["sticky"], 99);
        assert_eq!(raw["entries"]["0"]["comment"], "新標題");
        assert_eq!(
            raw["entries"]["0"]["extensions"]["table_tavern"]["visibility"]["characters"][0],
            "角色代碼"
        );

        let allocated =
            upsert_worldbook_entry(root.path(), &world_id, worldbook_entry(u64::MAX, "新增"))
                .unwrap();
        assert_eq!(allocated, 1);
        let raw: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(
                root.path()
                    .join(format!("worlds/{world_id}/worldbook.json")),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(raw["entries"]["1"]["selective"], true);
        assert_eq!(raw["entries"]["1"]["probability"], 100);
        assert_eq!(raw["entries"]["1"]["useProbability"], true);
        assert_eq!(raw["entries"]["1"]["depth"], 4);
        assert_eq!(raw["entries"]["1"]["displayIndex"], 0);

        delete_worldbook_entry(root.path(), &world_id, 0).unwrap();
        assert_eq!(
            read_worldbook(root.path(), &world_id)
                .unwrap()
                .into_iter()
                .map(|entry| entry.uid)
                .collect::<Vec<_>>(),
            [1]
        );
    }

    #[test]
    fn worldbook_entry_to_character_moves_content_and_keeps_other_entries() {
        let root = TestRoot::new("worldbook-entry-to-character");
        let world_id = create_world(root.path(), "世界").unwrap();
        let mut source = worldbook_entry(u64::MAX, "霧港船長");
        source.content = "第一段\n\n第二段".to_owned();
        let source_uid = upsert_worldbook_entry(root.path(), &world_id, source).unwrap();
        let other_uid = upsert_worldbook_entry(
            root.path(),
            &world_id,
            worldbook_entry(u64::MAX, "留下的條目"),
        )
        .unwrap();

        let meta = worldbook_entry_to_character(
            root.path(),
            &world_id,
            source_uid,
            "#123456".to_owned(),
            false,
        )
        .unwrap();

        assert_eq!(meta.name, "霧港船長");
        assert_eq!(
            read_character(root.path(), &world_id, &meta.id)
                .unwrap()
                .public_md,
            "第一段\n\n第二段"
        );
        assert_eq!(
            read_worldbook(root.path(), &world_id)
                .unwrap()
                .iter()
                .map(|entry| entry.uid)
                .collect::<Vec<_>>(),
            [other_uid]
        );
    }

    #[test]
    fn worldbook_entry_to_player_card_sets_state_and_rejects_second_card() {
        let root = TestRoot::new("worldbook-entry-to-player");
        let world_id = create_world(root.path(), "世界").unwrap();
        let first_uid =
            upsert_worldbook_entry(root.path(), &world_id, worldbook_entry(u64::MAX, "玩家"))
                .unwrap();
        let second_uid = upsert_worldbook_entry(
            root.path(),
            &world_id,
            worldbook_entry(u64::MAX, "候補玩家"),
        )
        .unwrap();

        let player = worldbook_entry_to_character(
            root.path(),
            &world_id,
            first_uid,
            "#abcdef".to_owned(),
            true,
        )
        .unwrap();
        assert_eq!(
            read_state(root.path(), &world_id).unwrap().player_card_id,
            Some(player.id)
        );

        assert_eq!(
            worldbook_entry_to_character(
                root.path(),
                &world_id,
                second_uid,
                "#abcdef".to_owned(),
                true,
            )
            .unwrap_err()
            .to_string(),
            "這桌已經有玩家卡"
        );
        assert!(read_worldbook(root.path(), &world_id)
            .unwrap()
            .iter()
            .any(|entry| entry.uid == second_uid));
    }

    #[test]
    fn worldbook_entry_to_character_rejects_empty_title_without_deleting() {
        let root = TestRoot::new("worldbook-entry-empty-title");
        let world_id = create_world(root.path(), "世界").unwrap();
        let uid = upsert_worldbook_entry(root.path(), &world_id, worldbook_entry(u64::MAX, "  "))
            .unwrap();

        assert_eq!(
            worldbook_entry_to_character(root.path(), &world_id, uid, "#abcdef".to_owned(), false,)
                .unwrap_err()
                .to_string(),
            "條目沒有標題，先給標題再轉"
        );
        assert!(read_worldbook(root.path(), &world_id)
            .unwrap()
            .iter()
            .any(|entry| entry.uid == uid));
    }

    #[test]
    fn character_to_worldbook_entry_moves_archived_card_and_private_content() {
        let root = TestRoot::new("character-to-worldbook-entry");
        let world_id = create_world(root.path(), "世界").unwrap();
        let mut card = character_card(&new_id(), "封存船長");
        card.archived = true;
        card.public_md = "公開設定".to_owned();
        card.private_md = "GM 秘密".to_owned();
        write_character(root.path(), &world_id, &card).unwrap();

        character_to_worldbook_entry(root.path(), &world_id, &card.id).unwrap();

        let entries = read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "封存船長");
        assert_eq!(entries[0].content, "公開設定\n\n## 私有\nGM 秘密");
        assert!(entries[0].constant);
        assert_eq!(entries[0].visibility, Visibility::Gm);
        assert_eq!(entries[0].order, 100);
        assert!(read_character(root.path(), &world_id, &card.id).is_err());
    }

    #[test]
    fn character_to_worldbook_entry_rejects_active_and_player_cards() {
        let root = TestRoot::new("character-to-worldbook-rejects");
        let world_id = create_world(root.path(), "世界").unwrap();
        let active = character_card(&new_id(), "還在桌上");
        write_character(root.path(), &world_id, &active).unwrap();
        assert_eq!(
            character_to_worldbook_entry(root.path(), &world_id, &active.id)
                .unwrap_err()
                .to_string(),
            "這張卡還在桌上"
        );

        let mut player = character_card(&new_id(), "玩家");
        player.archived = true;
        write_character(root.path(), &world_id, &player).unwrap();
        let mut state = read_state(root.path(), &world_id).unwrap();
        state.player_card_id = Some(player.id.clone());
        write_state(root.path(), &world_id, &state).unwrap();
        assert_eq!(
            character_to_worldbook_entry(root.path(), &world_id, &player.id)
                .unwrap_err()
                .to_string(),
            "玩家卡不能轉"
        );
        assert!(read_character(root.path(), &world_id, &player.id).is_ok());
    }

    #[test]
    fn new_worldbook_entry_is_first_and_shifts_display_indices() {
        let root = TestRoot::new("worldbook-new-first");
        let world_id = create_world(root.path(), "世界").unwrap();
        write_worldbook_fixture(
            &root,
            &world_id,
            serde_json::json!({
                "10": {
                    "uid": 10, "comment": "甲", "order": 10, "displayIndex": 0
                },
                "20": {
                    "uid": 20, "comment": "乙", "order": 20, "displayIndex": 1
                }
            }),
        );

        let uid = upsert_worldbook_entry(root.path(), &world_id, worldbook_entry(u64::MAX, "新增"))
            .unwrap();
        assert_eq!(read_worldbook(root.path(), &world_id).unwrap()[0].uid, uid);
        let raw = read_worldbook_fixture(&root, &world_id);
        assert_eq!(raw["entries"]["10"]["displayIndex"], 1);
        assert_eq!(raw["entries"]["20"]["displayIndex"], 2);
        assert_eq!(raw["entries"][uid.to_string()]["displayIndex"], 0);
    }

    #[test]
    fn reordering_worldbook_entries_applies_the_given_order() {
        let root = TestRoot::new("worldbook-reorder");
        let world_id = create_world(root.path(), "世界").unwrap();
        write_worldbook_fixture(
            &root,
            &world_id,
            serde_json::json!({
                "0": {"uid": 0, "comment": "甲", "displayIndex": 0},
                "1": {"uid": 1, "comment": "乙", "displayIndex": 1},
                "2": {"uid": 2, "comment": "丙", "displayIndex": 2}
            }),
        );

        // 跨多格拖曳：最後一筆拉到最前
        reorder_worldbook_entries(root.path(), &world_id, &[2, 0, 1]).unwrap();
        assert_eq!(
            read_worldbook(root.path(), &world_id)
                .unwrap()
                .iter()
                .map(|entry| entry.uid)
                .collect::<Vec<_>>(),
            [2, 0, 1]
        );
        reorder_worldbook_entries(root.path(), &world_id, &[0, 1, 2]).unwrap();
        assert_eq!(
            read_worldbook(root.path(), &world_id)
                .unwrap()
                .iter()
                .map(|entry| entry.uid)
                .collect::<Vec<_>>(),
            [0, 1, 2]
        );
    }

    #[test]
    fn reordering_worldbook_keeps_unlisted_entries_after_the_listed_ones() {
        let root = TestRoot::new("worldbook-reorder-partial");
        let world_id = create_world(root.path(), "世界").unwrap();
        write_worldbook_fixture(
            &root,
            &world_id,
            serde_json::json!({
                "0": {"uid": 0, "comment": "甲", "displayIndex": 0},
                "1": {"uid": 1, "comment": "乙", "displayIndex": 1},
                "2": {"uid": 2, "comment": "丙", "displayIndex": 2}
            }),
        );

        // uid 9 不存在應被忽略；沒送到的 0 依原順序接在後面
        reorder_worldbook_entries(root.path(), &world_id, &[2, 9, 1]).unwrap();

        assert_eq!(
            read_worldbook(root.path(), &world_id)
                .unwrap()
                .iter()
                .map(|entry| entry.uid)
                .collect::<Vec<_>>(),
            [2, 1, 0]
        );
    }

    #[test]
    fn reordering_legacy_worldbook_entries_normalizes_display_indices() {
        let root = TestRoot::new("worldbook-reorder-legacy");
        let world_id = create_world(root.path(), "世界").unwrap();
        write_worldbook_fixture(
            &root,
            &world_id,
            serde_json::json!({
                "7": {"uid": 7, "comment": "丙"},
                "3": {"uid": 3, "comment": "甲"},
                "5": {"uid": 5, "comment": "乙"}
            }),
        );

        reorder_worldbook_entries(root.path(), &world_id, &[5, 3, 7]).unwrap();

        assert_eq!(
            read_worldbook(root.path(), &world_id)
                .unwrap()
                .iter()
                .map(|entry| entry.uid)
                .collect::<Vec<_>>(),
            [5, 3, 7]
        );
        let raw = read_worldbook_fixture(&root, &world_id);
        let mut indices = raw["entries"]
            .as_object()
            .unwrap()
            .values()
            .map(|entry| entry["displayIndex"].as_u64().unwrap())
            .collect::<Vec<_>>();
        indices.sort_unstable();
        assert_eq!(indices, [0, 1, 2]);
    }

    #[test]
    fn reordering_worldbook_entries_preserves_order_and_unknown_fields() {
        let root = TestRoot::new("worldbook-reorder-lossless");
        let world_id = create_world(root.path(), "世界").unwrap();
        write_worldbook_fixture(
            &root,
            &world_id,
            serde_json::json!({
                "0": {
                    "uid": 0, "comment": "甲", "order": 91, "displayIndex": 0,
                    "foreign": {"nested": true}
                },
                "1": {
                    "uid": 1, "comment": "乙", "order": 7, "displayIndex": 1,
                    "sticky": 42
                }
            }),
        );

        reorder_worldbook_entries(root.path(), &world_id, &[1, 0]).unwrap();

        let raw = read_worldbook_fixture(&root, &world_id);
        assert_eq!(raw["entries"]["0"]["order"], 91);
        assert_eq!(raw["entries"]["1"]["order"], 7);
        assert_eq!(
            raw["entries"]["0"]["foreign"],
            serde_json::json!({"nested": true})
        );
        assert_eq!(raw["entries"]["1"]["sticky"], 42);
    }

}
