//! 世界書匯入：條目正規化、可見度預設、去重與可見度合併。角色卡路與世界書路共用這一份——
//! 兩條路只差「這本書由誰演」（[`BookOwner`]）。

use super::super::character::list_characters;
use super::super::state::read_state;
use super::super::{invalid_data, DataResult};
use super::{
    entries_object_mut, entry_uid, next_uid, read_worldbook_value, set_visibility,
    sorted_entry_keys, table_tavern_field, table_tavern_mut, visibility_from_value,
    write_worldbook_value, Visibility, FORCED_DISABLE,
};
use crate::mechanism::{Record, RecordKind};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

/// 這本書由誰演：條目沒指定可見度時就給誰看。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookOwner {
    /// 世界書路（世界書卡、獨立世界書）：GM 演。
    Gm,
    /// 角色卡路：這張卡自己的角色（角色 id）。條目另記來源卡 `source_cards`。
    Character(String),
}

/// 匯入結果：`imported`＝真的寫進去的條數，`skipped`＝內容重複被略過的條數，
/// `invalid`＝欄位壞掉（`enabled` 不是布林）被略過的條數。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorldbookImport {
    pub imported: usize,
    pub skipped: usize,
    #[serde(default)]
    pub invalid: usize,
}

/// 匯入時被合併改寫的既有條目：撤銷只還原這兩個欄位（`extensions.table_tavern` 的
/// `visibility` 與 `source_cards` 原值，None＝原本沒有這個欄位）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisibilityRestore {
    pub uid: u64,
    #[serde(default)]
    pub before_visibility: Option<serde_json::Value>,
    #[serde(default)]
    pub before_cards: Option<serde_json::Value>,
    #[serde(default)]
    pub after_visibility: Option<serde_json::Value>,
    #[serde(default)]
    pub after_cards: Option<serde_json::Value>,
}

/// 一次世界書匯入的完整結果：收編數字、每條來源條目（卡片契約的 key）落到哪個 UID（內容重複被略過的映到
/// 桌上保留的那一條；那條讀不出 UID、或條目壞掉被略過才是 None），與被合併改寫的既有條目。
#[derive(Debug, Clone, PartialEq)]
pub struct BookImport {
    pub summary: WorldbookImport,
    pub placed: Vec<(String, Option<u64>)>,
    pub restores: Vec<VisibilityRestore>,
}

pub(super) const SOURCE_CARDS: &str = "source_cards";
pub(super) const SOURCE_DISABLE: &str = "source_disable";

#[cfg(test)]
pub fn import_worldbook(
    root: &Path,
    world_id: &str,
    json_text: &str,
) -> DataResult<WorldbookImport> {
    import_worldbook_as(root, world_id, json_text, &BookOwner::Gm).map(|book| book.summary)
}

/// 匯入一本世界書（獨立書 JSON 的物件形 entries，或 character_book 的陣列形 entries）。
pub fn import_worldbook_as(
    root: &Path,
    world_id: &str,
    json_text: &str,
    owner: &BookOwner,
) -> DataResult<BookImport> {
    let imported: serde_json::Value = serde_json::from_str(json_text)
        .map_err(|error| invalid_data(format!("invalid worldbook JSON: {error}")))?;
    let source = imported
        .get("entries")
        .ok_or_else(|| invalid_data("imported worldbook is missing entries"))?;
    let character_book = match source {
        serde_json::Value::Object(_) => false,
        serde_json::Value::Array(_) => true,
        _ => {
            return Err(invalid_data(
                "imported worldbook entries must be an object or array",
            ));
        }
    };
    // 條目照卡片契約展開：物件形照 uid 鍵的數字順序（新 UID 依此配發）、非物件的值略過不算條目
    let source_entries: Vec<(String, serde_json::Value)> =
        crate::import::book_entries_keyed(source)
            .into_iter()
            .map(|(key, value)| (key, value.clone()))
            .collect();
    let table_ids: HashSet<String> = match owner {
        BookOwner::Gm => HashSet::new(),
        BookOwner::Character(_) => list_characters(root, world_id)?
            .into_iter()
            .map(|meta| meta.id)
            .collect(),
    };

    let mut worldbook = read_worldbook_value(root, world_id)?;
    let entries = entries_object_mut(&mut worldbook)?;
    let preexisting: HashSet<String> = entries.keys().cloned().collect();
    // 指紋 → 被保留那條的鍵：重複的來源條目映到它（契約：重複條目指向桌上留下的那一條）
    let mut seen: HashMap<String, String> = sorted_entry_keys(entries)
        .into_iter()
        .rev()
        .map(|key| (dedupe_fingerprint(&entries[&key]), key))
        .collect();
    let mut uid = next_uid(entries)?;
    let mut summary = WorldbookImport::default();
    let mut absorbed = Vec::new();
    let mut placed = Vec::with_capacity(source_entries.len());
    let mut restores: BTreeMap<String, VisibilityRestore> = BTreeMap::new();
    for (key, source_entry) in source_entries {
        let Some(entry) =
            normalize_imported_entry(source_entry, character_book, uid, owner, &table_ids)?
        else {
            summary.invalid += 1;
            placed.push((key, None));
            continue;
        };
        // 已經有一模一樣的條目就跳過，重複匯入同一份書不會塞出兩套內容；可見度與來源卡併進留下那條
        let fingerprint = dedupe_fingerprint(&entry);
        if let Some(kept_key) = seen.get(&fingerprint) {
            let kept = entries
                .get_mut(kept_key)
                .ok_or_else(|| invalid_data("worldbook entry disappeared"))?;
            let kept_uid = entry_uid(kept_key, kept);
            // 只記匯入前就在桌上的條目、同一條只記第一次的值；本次新建的整條屬於這次匯入
            if preexisting.contains(kept_key) && !restores.contains_key(kept_key) {
                if let Some(kept_uid) = kept_uid {
                    restores.insert(
                        kept_key.clone(),
                        VisibilityRestore {
                            uid: kept_uid,
                            before_visibility: table_tavern_field(kept, "visibility").cloned(),
                            before_cards: table_tavern_field(kept, SOURCE_CARDS).cloned(),
                            after_visibility: None,
                            after_cards: None,
                        },
                    );
                }
            }
            merge_entry_into(kept, &entry);
            summary.skipped += 1;
            placed.push((key, kept_uid));
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
                detail: crate::ui_msg::UiMsg::LedgerScaffoldAbsorbed.to_string(),
            });
        }
        let new_key = uid.to_string();
        entries.insert(new_key.clone(), entry);
        seen.insert(fingerprint, new_key);
        placed.push((key, Some(uid)));
        uid = uid
            .checked_add(1)
            .ok_or_else(|| invalid_data("worldbook uid overflow"))?;
        summary.imported += 1;
    }
    // after 以這次匯入結束時的實際值為準；沒有實際變動的不進收據
    let restores: Vec<VisibilityRestore> = restores
        .into_iter()
        .filter_map(|(key, mut restore)| {
            let value = entries.get(&key)?;
            restore.after_visibility = table_tavern_field(value, "visibility").cloned();
            restore.after_cards = table_tavern_field(value, SOURCE_CARDS).cloned();
            (restore.after_visibility != restore.before_visibility
                || restore.after_cards != restore.before_cards)
                .then_some(restore)
        })
        .collect();
    write_worldbook_value(root, world_id, &worldbook)?;
    if !absorbed.is_empty() {
        let scene = read_state(root, world_id)
            .map(|state| state.current_scene)
            .unwrap_or(0);
        crate::mechanism::append_log(root, world_id, scene, &absorbed);
    }
    Ok(BookImport {
        summary,
        placed,
        restores,
    })
}

/// 清掉內容重複的條目：同一份指紋只留顯示順序最前的那條（被刪那條的可見度與來源卡併進去），
/// 回傳刪掉幾條。給去重上線前就已經重複匯入的桌收拾用。
pub fn dedupe_worldbook(root: &Path, world_id: &str) -> DataResult<usize> {
    let mut worldbook = read_worldbook_value(root, world_id)?;
    let entries = entries_object_mut(&mut worldbook)?;
    let mut kept_by_fingerprint: HashMap<String, String> = HashMap::new();
    let mut duplicates: Vec<(String, String)> = Vec::new();
    for key in sorted_entry_keys(entries) {
        let fingerprint = dedupe_fingerprint(&entries[&key]);
        match kept_by_fingerprint.get(&fingerprint) {
            Some(kept) => duplicates.push((key, kept.clone())),
            None => {
                kept_by_fingerprint.insert(fingerprint, key);
            }
        }
    }
    for (key, kept) in &duplicates {
        if let Some(removed) = entries.remove(key) {
            if let Some(kept_value) = entries.get_mut(kept) {
                merge_entry_into(kept_value, &removed);
            }
        }
    }
    if !duplicates.is_empty() {
        write_worldbook_value(root, world_id, &worldbook)?;
    }
    Ok(duplicates.len())
}

/// 正規化一條來源條目。`enabled` 不是布林＝這條壞掉，回 None（呼叫端略過並計數）。
fn normalize_imported_entry(
    mut value: serde_json::Value,
    character_book: bool,
    uid: u64,
    owner: &BookOwner,
    table_ids: &HashSet<String>,
) -> DataResult<Option<serde_json::Value>> {
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
            let Some(enabled) = enabled.as_bool() else {
                return Ok(None);
            };
            object.insert("disable".to_owned(), serde_json::Value::Bool(!enabled));
        }
    }
    object.insert("uid".to_owned(), serde_json::json!(uid));
    match owner {
        // 世界書路：明示的可見度原樣保留（讀取端讀不懂退回 GM），沒寫的給 GM
        BookOwner::Gm => {
            if table_tavern_field(&value, "visibility").is_none() {
                set_visibility(&mut value, &Visibility::Gm);
            }
        }
        // 角色卡路：明示的 gm／public、名單裡有本桌角色的 characters 才算數；讀不懂或名單全是別桌
        // 的 id（舊版匯出卡）一律當沒寫，給這張卡自己的角色
        BookOwner::Character(id) => {
            let visibility = explicit_card_visibility(&value, table_ids)
                .unwrap_or_else(|| Visibility::Characters(vec![id.clone()]));
            set_visibility(&mut value, &visibility);
            if let Some(table_tavern) = table_tavern_mut(&mut value) {
                table_tavern.insert(SOURCE_CARDS.to_owned(), serde_json::json!([id]));
            }
        }
    }
    if is_mechanism_scaffold(&value) {
        // 已經帶來源停用值（這桌匯出的世界書再匯回來）就沿用：目前的 disable 是當初強制停用的結果，
        // 不是原卡的值
        let source_disable = table_tavern_field(&value, SOURCE_DISABLE)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or_else(|| {
                value
                    .get("disable")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false)
            });
        if let Some(object) = value.as_object_mut() {
            object.insert("disable".to_owned(), serde_json::Value::Bool(true));
        }
        if let Some(table_tavern) = table_tavern_mut(&mut value) {
            table_tavern.insert(
                SOURCE_DISABLE.to_owned(),
                serde_json::Value::Bool(source_disable),
            );
            table_tavern.insert(FORCED_DISABLE.to_owned(), serde_json::Value::Bool(true));
        }
    }
    Ok(Some(value))
}

/// 角色卡路的明示可見度：`"gm"`、`"public"`，或名單裡至少一個本桌角色 id 的 `characters`
/// （只留本桌的 id）。其餘一律 None。
fn explicit_card_visibility(
    value: &serde_json::Value,
    table_ids: &HashSet<String>,
) -> Option<Visibility> {
    match table_tavern_field(value, "visibility")? {
        serde_json::Value::String(text) if text == "gm" => Some(Visibility::Gm),
        serde_json::Value::String(text) if text == "public" => Some(Visibility::Public),
        serde_json::Value::Object(object) => {
            let ids: Vec<String> = object
                .get("characters")?
                .as_array()?
                .iter()
                .filter_map(serde_json::Value::as_str)
                .filter(|id| table_ids.contains(*id))
                .map(str::to_owned)
                .collect();
            (!ids.is_empty()).then_some(Visibility::Characters(ids))
        }
        _ => None,
    }
}

/// 條目記的來源卡（角色卡路匯入時寫入）。
pub(super) fn source_cards(value: &serde_json::Value) -> Vec<String> {
    table_tavern_field(value, SOURCE_CARDS)
        .and_then(serde_json::Value::as_array)
        .map(|ids| {
            ids.iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// 把重複的那條併進留下的那條：可見度 `Public` 優先 → `Characters` 取聯集 → `Gm` 最低；
/// 帶來源卡的 `Gm`（角色卡路明寫 gm、或玩家把角色卡條目改成 GM）是落定的，不被 `Characters` 收成名單。
/// 來源卡取聯集。
fn merge_entry_into(kept: &mut serde_json::Value, incoming: &serde_json::Value) {
    let kept_visibility = visibility_from_value(kept);
    let kept_cards = source_cards(kept);
    let merged = match (&kept_visibility, visibility_from_value(incoming)) {
        (Visibility::Public, _) | (_, Visibility::Public) => Visibility::Public,
        (Visibility::Characters(ids), Visibility::Characters(more)) => {
            let mut ids = ids.clone();
            for id in more {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            Visibility::Characters(ids)
        }
        (Visibility::Gm, Visibility::Characters(ids)) if kept_cards.is_empty() => {
            Visibility::Characters(ids)
        }
        (kept, _) => kept.clone(),
    };
    if merged != kept_visibility {
        set_visibility(kept, &merged);
    }
    let mut cards = kept_cards.clone();
    for id in source_cards(incoming) {
        if !cards.contains(&id) {
            cards.push(id);
        }
    }
    if cards != kept_cards {
        if let Some(table_tavern) = table_tavern_mut(kept) {
            table_tavern.insert(SOURCE_CARDS.to_owned(), serde_json::json!(cards));
        }
    }
}

/// 機制鷹架條目：`[initvar]`／`[mvu_update]` 規則表、原生 EJS 腳本，或 ST 把整棵變數樹塞回提示詞的巨集。
/// 本地已接管或原本就不會交給模型的內容，不該再送進模型上下文燒字數。
/// `[mvu_update]` 只認抽得出欄位規則的規則表；同前綴的輸出格式說明 app 沒接手，照原卡啟停
/// （沒重構的卡照酒館做）。
fn is_mechanism_scaffold(entry: &serde_json::Value) -> bool {
    let marker = entry
        .get("comment")
        .and_then(serde_json::Value::as_str)
        .or_else(|| entry.get("title").and_then(serde_json::Value::as_str))
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let content = entry
        .get("content")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if marker.starts_with("[initvar]") {
        return true;
    }
    if marker.starts_with("[mvu_update]") && crate::import::is_field_rule_table(content) {
        return true;
    }
    content.contains("{{format_message_variable::") || content.contains("<%")
}

/// 條目的身分欄位：標題、內文、主鍵、次要鍵、常駐（缺欄＝false）。
pub(super) fn identity_text(entry: &serde_json::Value) -> String {
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
    let constant = entry
        .get("constant")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    format!(
        "{}\u{1e}{}\u{1e}{}\u{1e}{}\u{1e}{constant}",
        text("comment"),
        text("content"),
        keys("key"),
        keys("keysecondary"),
    )
}

/// 去重指紋：同一份世界書重複匯入時用它認出「一模一樣的條目」。身分欄位＋鷹架條目的來源停用值；
/// 不含停用、順序、可見度等玩家會改或隨匯入產生的欄位——玩家用帳本開關或編輯器改過這些之後，
/// 同一張卡再匯入也不會多出一條。鷹架條目匯入後一律停用，靠來源停用值把原本一啟一停的兩條分開。
fn dedupe_fingerprint(entry: &serde_json::Value) -> String {
    let source_disable =
        match table_tavern_field(entry, SOURCE_DISABLE).and_then(serde_json::Value::as_bool) {
            Some(true) => "1",
            Some(false) => "0",
            None => "-",
        };
    format!("{}\u{1e}{source_disable}", identity_text(entry))
}
