//! 世界書匯入：條目正規化、可見度預設、去重與可見度合併。角色卡路與世界書路共用這一份——
//! 兩條路只差「這本書由誰演」（[`BookOwner`]）。
use crate::data::state_commit::with_commit;

use super::super::character::list_characters;
use super::super::state::read_state;
use super::super::{invalid_data, DataResult};
use super::{
    entries_object_mut, entry_uid, next_uid, read_worldbook_value, set_source_cards,
    set_visibility, sorted_entry_keys, table_tavern_field, table_tavern_mut, visibility_from_value,
    write_worldbook_value, Visibility, FORCED_DISABLE,
};
use crate::mechanism::{Record, RecordKind};
use crate::world_info::book_order::{st_order, SourceEntries, StOrdered};
use crate::world_info::entry::{character_book_to_world_object, from_world_file, RecursionDelay};
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

/// 匯入結果：`imported`＝真的寫進去的條數，`skipped`＝內容重複被略過的條數。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorldbookImport {
    pub imported: usize,
    pub skipped: usize,
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
    // 純解析與排序只看 json_text，放在鎖外；鎖內只讀桌上的書與角色、配 uid、合併、寫書、清計時
    let members: HashMap<String, &serde_json::value::RawValue> = serde_json::from_str(json_text)
        .map_err(|error| invalid_data(format!("invalid worldbook JSON: {error}")))?;
    let entries_text = members
        .get("entries")
        .ok_or_else(|| invalid_data("imported worldbook is missing entries"))?
        .get();
    // 從原文解析才保得住物件形鍵的出現順序；新 UID 依 ST 載入順序配發（陣列形照 `id`，重複的後蓋前）
    let source = SourceEntries::parse(entries_text)
        .map_err(|error| invalid_data(format!("invalid worldbook JSON: {error}")))?
        .ok_or_else(|| invalid_data("imported worldbook entries must be an object or array"))?;
    let character_book = source.is_array();
    let StOrdered {
        entries: source_entries,
        dropped,
    } = st_order(&source);
    with_commit(root, world_id, |_| {
        let table_ids: HashSet<String> = list_characters(root, world_id)?
            .into_iter()
            .map(|meta| meta.id)
            .collect();

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
            let entry = normalize_imported_entry(
                serde_json::Value::Object(source_entry.clone()),
                character_book,
                uid,
                owner,
                &table_ids,
            )?;
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
        // `id` 重複被蓋掉的條目指向留下的那一條
        for (gone, kept) in dropped {
            let uid = placed
                .iter()
                .find(|(key, _)| *key == kept)
                .and_then(|(_, uid)| *uid);
            placed.push((gone, uid));
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
    })
}

/// 清掉內容重複的條目：同一份指紋只留顯示順序最前的那條（被刪那條的可見度與來源卡併進去），
/// 回傳刪掉幾條。給去重上線前就已經重複匯入的桌收拾用。
pub fn dedupe_worldbook(root: &Path, world_id: &str) -> DataResult<usize> {
    with_commit(root, world_id, |_| {
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
    })
}

/// 正規化一條來源條目（V2 陣列形轉成 ST 物件形）。
fn normalize_imported_entry(
    mut value: serde_json::Value,
    character_book: bool,
    uid: u64,
    owner: &BookOwner,
    table_ids: &HashSet<String>,
) -> DataResult<serde_json::Value> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| invalid_data("worldbook entry must be an object"))?;
    if character_book {
        let converted = character_book_to_world_object(object);
        for field in ["keys", "secondary_keys", "insertion_order", "enabled"] {
            object.remove(field);
        }
        object.extend(converted);
    }
    object.insert("uid".to_owned(), serde_json::json!(uid));
    match owner {
        // 世界書路：明示的 gm／public 照留，名單只留本桌 id；濾空、讀不懂、沒寫一律給 GM。
        // 來源卡只留本桌 id，但明寫 gm 的條目原樣保留：那是「落定」標記，別桌作者只給 GM 的決定要跟著走
        BookOwner::Gm => {
            let settled_gm = matches!(
                table_tavern_field(&value, "visibility"),
                Some(serde_json::Value::String(text)) if text == "gm"
            ) && !source_cards(&value).is_empty();
            let visibility = explicit_book_visibility(&value, table_ids).unwrap_or(Visibility::Gm);
            set_visibility(&mut value, &visibility);
            if !settled_gm && table_tavern_field(&value, SOURCE_CARDS).is_some() {
                let cards: Vec<String> = source_cards(&value)
                    .into_iter()
                    .filter(|id| table_ids.contains(id))
                    .collect();
                set_source_cards(&mut value, &cards);
            }
        }
        // 角色卡路：明示的 gm／public、名單裡有本桌角色的 characters 才算數；讀不懂或名單全是別桌
        // 的 id（舊版匯出卡）一律當沒寫，給這張卡自己的角色（世界書路則給 GM）
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
    Ok(value)
}

/// 世界書路的明示可見度：`"gm"`、`"public"`，或照讀取端規則（整個陣列都是字串）讀得懂、
/// 至少一個本桌角色 id 的 `characters`（只留本桌的 id）。其餘一律 None。
fn explicit_book_visibility(
    value: &serde_json::Value,
    table_ids: &HashSet<String>,
) -> Option<Visibility> {
    if let Some(serde_json::Value::Object(object)) = table_tavern_field(value, "visibility") {
        let ids = object.get("characters")?.as_array()?;
        if !ids.iter().all(serde_json::Value::is_string) {
            return None;
        }
    }
    explicit_card_visibility(value, table_ids)
}

/// 角色卡路的明示可見度：`"gm"`、`"public"`，或名單裡至少一個本桌角色 id 的 `characters`
/// （只留本桌的 id，非字串略過）。其餘一律 None。
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

/// 去重指紋：同一份世界書重複匯入時用它認出「一模一樣的條目」。以讀取端（[`from_world_file`]）讀出的
/// `WiEntry` 計算，納入所有影響觸發的欄位（鍵與其順序、次要鍵、邏輯、位置、機率、群組、計時、遞迴、
/// 掃描範圍、`triggers`、`match*`、裝飾…）加標題、內文與鷹架條目的來源停用值；不含停用、順序、可見度
/// 等玩家會改或隨匯入產生的欄位——玩家用帳本開關或編輯器改過這些之後，同一張卡再匯入也不會多出一條。
/// 先正規化再算：`delayUntilRecursion` 的 `false`／`0` 都當 0、`true` 當 1，計時的 `null`／`0` 都當 0，
/// 缺 `key` 當空陣列；標題、內文、鍵一律用原文（內文原樣進提示，只差空白也不是同一行為）；沒有次要鍵時 `selective` 與 `selectiveLogic` 不算。鷹架條目匯入後一律停用，靠來源停用值把原本一啟一停的兩條分開。
fn dedupe_fingerprint(entry: &serde_json::Value) -> String {
    let empty = serde_json::Map::new();
    let mut wi = from_world_file(entry.as_object().unwrap_or(&empty));
    wi.order = 0.0;
    wi.disable = false;
    // 缺 `key`／非陣列與空陣列行為相同
    wi.key.get_or_insert_with(Vec::new);
    wi.delay_until_recursion = RecursionDelay::Level(wi.delay_until_recursion.level());
    // 沒有次要鍵時 `selective`／`selectiveLogic` 不影響觸發（V2 缺欄是 false、物件形缺欄是 true）
    if wi.keysecondary.is_empty() {
        wi.selective = false;
        wi.selective_logic = 0.0;
    }
    for timer in [&mut wi.sticky, &mut wi.cooldown, &mut wi.delay] {
        *timer = Some(timer.unwrap_or(0.0));
    }
    let behavior = serde_json::to_string(&wi).unwrap_or_default();
    let source_disable =
        match table_tavern_field(entry, SOURCE_DISABLE).and_then(serde_json::Value::as_bool) {
            Some(true) => "1",
            Some(false) => "0",
            None => "-",
        };
    format!("{behavior}\u{1e}{source_disable}")
}
