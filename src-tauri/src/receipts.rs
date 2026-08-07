//! 匯入收據＋一鍵復原：每次匯入角色卡／世界書時記下「實際新增」的東西，
//! 「復原上次匯入」逆向最後一筆、可連按逐筆退。收據落檔 worlds/<world_id>/import-receipts.json。
//! 容錯是核心：收據寫入失敗不得影響匯入成功與否；收據檔缺檔／壞掉在「記帳」與「查詢」時
//! 一律當作沒有歷史（悄悄放棄），只有「復原」動作本身遇到壞檔才回錯——不然玩家會以為復原了
//! 其實什麼都沒發生。

use crate::data::{self, DataResult, FieldRule, StateNode, Trigger, WorldState, WorldbookEntry};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

/// 匯入呼叫前先拍的快照：worldbook 既有 uid、世界狀態（mechanism／狀態樹）、
/// 這桌等級卡片介面殼（source-card.*）是否已存在。呼叫端在匯入完成後連同這份快照
/// 交回 record_* 函式，兩相比對才知道「這次匯入實際新增了什麼」。
pub struct Snapshot {
    state: Option<WorldState>,
    worldbook_uids: HashSet<u64>,
    world_card: (bool, bool), // (png 已存在, import.json 已存在)
    /// GM 卡的圖（gm.png）快照時是否已存在——比照 world_card：undo 只刪這次匯入新建的那張，
    /// 匯入前就有的圖不動（被這次 PNG 覆寫掉的舊圖不還原，原始卡檔還在使用者手上）。
    gm_image_existed: bool,
    /// AI 卡重構的介面渲染殼檔（interface-shell.html）快照時是否已存在——比照 world_card 的
    /// 存在性 diff 手法：undo 只該刪這次操作新建的殼，不動套用前就有的。
    interface_shell_existed: bool,
    /// 機制帳本（mechanism-log.jsonl）快照時的原始內容；這檔是純 append，記著這份就能在
    /// undo 時精準挖掉「這次操作自己追加的那一段」，不牽連期間新產生的遊玩紀錄。
    mechanism_log_before: String,
}

pub fn snapshot(root: &Path, world_id: &str) -> Snapshot {
    Snapshot {
        state: data::read_state(root, world_id).ok(),
        worldbook_uids: data::read_worldbook(root, world_id)
            .unwrap_or_default()
            .into_iter()
            .map(|entry| entry.uid)
            .collect(),
        world_card: (
            data::world_card_path(root, world_id, "png").is_ok_and(|path| path.exists()),
            data::world_card_path(root, world_id, "import.json").is_ok_and(|path| path.exists()),
        ),
        gm_image_existed: data::gm_image_path(root, world_id).is_ok_and(|path| path.exists()),
        interface_shell_existed: data::interface_shell_path(root, world_id)
            .is_ok_and(|path| path.exists()),
        mechanism_log_before: data::mechanism_log_path(root, world_id)
            .ok()
            .and_then(|path| fs::read_to_string(path).ok())
            .unwrap_or_default(),
    }
}

/// 貼到檯面上的開場白：靠場景號＋事件時間戳定位，undo 時只刪這一則。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PostedOpening {
    scene: u64,
    ts: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReceiptWorldbookEntry {
    uid: u64,
    /// 條目內容指紋：復原時判斷玩家改過沒——指紋不同就保留、計進 kept_entries。
    fingerprint: String,
}

/// 機制／狀態樹的倒退資訊。只記這次匯入實際新增或覆寫的鍵，沒被動到的鍵完全不進收據，
/// 這樣連續多筆匯入逐一復原時，每一筆只退回自己造成的變動，不會牽連別筆或期間的正常遊玩。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct MechanismUndo {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    added_rule_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    restored_rules: BTreeMap<String, FieldRule>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    added_trigger_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    restored_triggers: BTreeMap<String, Trigger>,
    /// 這次匯入把 incremental 從 false 翻成 true 才會有值（undo 就退回 false）；
    /// 匯入前已經是 true（別筆匯入或手動開的）就不記，undo 不該把它關掉。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    incremental_before: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    added_state_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    restored_state: BTreeMap<String, StateNode>,
}

impl MechanismUndo {
    fn is_empty(&self) -> bool {
        self.added_rule_keys.is_empty()
            && self.restored_rules.is_empty()
            && self.added_trigger_ids.is_empty()
            && self.restored_triggers.is_empty()
            && self.incremental_before.is_none()
            && self.added_state_keys.is_empty()
            && self.restored_state.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ImportReceipt {
    kind: String, // "character" | "worldbook" | "refactor"
    label: String,
    timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    character_id: Option<String>,
    /// AI 卡重構一次套用可能新增多張角色卡；`character_id` 留給既有單張路徑，
    /// 兩欄位互斥（各自的 record_* 函式只填自己那個），undo 兩邊都掃一次。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    character_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    worldbook_entries: Vec<ReceiptWorldbookEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mechanism: Option<MechanismUndo>,
    /// 這次匯入新建的這桌等級卡片介面殼副檔名（"png"｜"import.json"）；
    /// 匯入前就有的殼不記，undo 不動它（不是這次匯入造成的，不該被這次 undo 清掉）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    world_card_created: Option<String>,
    /// 這次匯入新建了 GM 卡的圖（gm.png）；匯入前就有的圖不記，undo 不動它。
    #[serde(default, skip_serializing_if = "data::is_false")]
    gm_image_created: bool,
    /// 匯完之後玩家從卡片挑了一則貼上檯面的開場白；undo 要連它一起收掉，
    /// 不然重匯同一張卡想換一則時，舊的那則還壓在開局上。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    opening: Option<PostedOpening>,
    /// 這次操作新建了 AI 卡重構的介面渲染殼檔（interface-shell.html）；套用前就有的殼不記，
    /// undo 不動它。跟 world_card_created 是兩回事：那個是「卡片自帶殼」的原始檔，這個是
    /// AI 依狀態樹規則另外產的靜態渲染殼。
    #[serde(default, skip_serializing_if = "data::is_false")]
    interface_shell_created: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    renamed_from: Option<String>,
    /// 這次操作改寫或停用了既有世界書條目（AI 卡重構的來源條目改寫、介面／機制的
    /// source_uid 停用）：整條原文快照，undo 時整條覆寫回去。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    rewritten_entries: Vec<WorldbookEntry>,
    /// 這次操作往「機制帳本」（mechanism-log.jsonl）追加的原文；undo 時整段挖掉，其餘
    /// （含期間新產生的遊玩紀錄）不動。目前只有 AI 卡重構套用機制那條路會寫非空值。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    added_ledger_lines: String,
}

/// 前端側欄按鈕與未來路由框用的摘要：不帶復原用的內部細節（指紋、機制差異）。
#[derive(Debug, Clone, Serialize)]
pub struct ImportReceiptSummary {
    pub kind: String,
    pub label: String,
    pub timestamp: String,
    pub character_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct UndoReport {
    /// 被刪角色的名字（給前端組訊息用；id 已經沒意義了）——單張角色匯入路徑專用。
    pub removed_character: Option<String>,
    /// AI 卡重構等一次套用多張角色卡的路徑：這次 undo 刪掉的角色名字清單。
    pub removed_characters: Vec<String>,
    pub removed_entries: usize,
    /// 玩家改過內容而保留下來的世界書條目數
    pub kept_entries: usize,
    pub renamed_back: bool,
    /// 這次 undo 把匯完貼上檯面的那則開場白也收掉了；前端據此重載逐字稿。
    pub removed_opening: bool,
}

fn read_receipts(root: &Path, world_id: &str) -> Vec<ImportReceipt> {
    let Ok(path) = data::import_receipts_path(root, world_id) else {
        return Vec::new();
    };
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn write_receipts(root: &Path, world_id: &str, receipts: &[ImportReceipt]) -> DataResult<()> {
    let path = data::import_receipts_path(root, world_id)?;
    fs::write(path, serde_json::to_string_pretty(receipts)?)?;
    Ok(())
}

/// 記帳失敗不得影響匯入是否成功，這裡全程不回傳 Result；讀壞就當成沒有歷史直接重寫
/// （等於放棄復原更早的匯入，但不會讓這次匯入跟著失敗）。
fn append_receipt(root: &Path, world_id: &str, receipt: ImportReceipt) {
    let mut receipts = read_receipts(root, world_id);
    receipts.push(receipt);
    let _ = write_receipts(root, world_id, &receipts);
}

/// post_opening 成功後呼叫：把剛貼上檯面的開場白掛到最後一筆收據，undo 時一併收掉。
/// 沒有收據（整份重複、什麼都沒新增的匯入）就不掛——那種匯入本來就沒東西可復原。
/// 同 append_receipt 的容錯原則：記帳失敗不影響開場白已經貼成功這件事。
pub fn record_posted_opening(root: &Path, world_id: &str, scene: u64, ts: &str) {
    let mut receipts = read_receipts(root, world_id);
    let Some(last) = receipts.last_mut() else {
        return;
    };
    last.opening = Some(PostedOpening {
        scene,
        ts: ts.to_owned(),
    });
    let _ = write_receipts(root, world_id, &receipts);
}

/// 一般化前後 diff：新增的鍵（undo 時整條移除）、被覆寫的鍵（undo 時整條恢復成舊值）。
/// 沒被這次匯入動到的鍵完全不會出現在任一份結果裡。
fn diff_added_and_overwritten<K, V>(
    before: &BTreeMap<K, V>,
    after: &BTreeMap<K, V>,
) -> (Vec<K>, BTreeMap<K, V>)
where
    K: Ord + Clone,
    V: PartialEq + Clone,
{
    let added = after
        .keys()
        .filter(|key| !before.contains_key(*key))
        .cloned()
        .collect();
    let overwritten = before
        .iter()
        .filter(|(key, value)| after.get(*key).is_some_and(|new_value| new_value != *value))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    (added, overwritten)
}

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

/// 匯入呼叫之後比對 mechanism／狀態樹前後差異；`before` 是這桌在匯入前的完整快照。
fn diff_mechanism(before: Option<&WorldState>, root: &Path, world_id: &str) -> Option<MechanismUndo> {
    let before = before?;
    let after = data::read_state(root, world_id).ok()?;

    let (added_rule_keys, restored_rules) =
        diff_added_and_overwritten(&before.mechanism.rules, &after.mechanism.rules);

    let as_map = |triggers: &[Trigger]| -> BTreeMap<String, Trigger> {
        triggers.iter().cloned().map(|t| (t.id.clone(), t)).collect()
    };
    let (added_trigger_ids, restored_triggers) = diff_added_and_overwritten(
        &as_map(&before.mechanism.triggers),
        &as_map(&after.mechanism.triggers),
    );

    let incremental_before = (before.mechanism.incremental != after.mechanism.incremental)
        .then_some(before.mechanism.incremental);

    let (added_state_keys, restored_state) =
        diff_added_and_overwritten(&before.state.tree, &after.state.tree);

    let undo = MechanismUndo {
        added_rule_keys,
        restored_rules,
        added_trigger_ids,
        restored_triggers,
        incremental_before,
        added_state_keys,
        restored_state,
    };
    (!undo.is_empty()).then_some(undo)
}

/// 機制帳本是純 append 檔，這次操作新增的內容＝目前檔案內容扣掉快照時的舊內容那段前綴。
/// 對不上前綴（檔案被外力改過，理論上不會發生）就當沒有新增，undo 那端找不到片段時
/// 本來就會靜默放棄，不會誤刪玩家的合法紀錄。
fn diff_ledger_suffix(before: &str, root: &Path, world_id: &str) -> String {
    let Ok(path) = data::mechanism_log_path(root, world_id) else {
        return String::new();
    };
    let after = fs::read_to_string(path).unwrap_or_default();
    after.strip_prefix(before).unwrap_or_default().to_owned()
}

/// 條目的復原用指紋：標題／內文／關鍵字／恆定／順序／停用／可見度，字段等值就當「沒改過」。
/// FNV-1a（跨執行穩定，做法比照 lanes.rs 的 events_fingerprint）。
fn worldbook_entry_fingerprint(entry: &WorldbookEntry) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        hash ^= 0x1f;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    };
    eat(entry.title.as_bytes());
    eat(entry.content.as_bytes());
    eat(entry.keys.join("\u{1f}").as_bytes());
    eat(&[entry.constant as u8]);
    eat(&entry.order.to_be_bytes());
    eat(&[entry.disabled as u8]);
    eat(format!("{:?}", entry.visibility).as_bytes());
    format!("{hash:016x}")
}

fn new_worldbook_entries(
    root: &Path,
    world_id: &str,
    before_uids: &HashSet<u64>,
) -> Vec<ReceiptWorldbookEntry> {
    data::read_worldbook(root, world_id)
        .unwrap_or_default()
        .into_iter()
        .filter(|entry| !before_uids.contains(&entry.uid))
        .map(|entry| ReceiptWorldbookEntry {
            uid: entry.uid,
            fingerprint: worldbook_entry_fingerprint(&entry),
        })
        .collect()
}

/// 這次匯入是否新建了一份這桌等級的卡片介面殼（source-card.*）；匯入前就有的不算。
fn detect_world_card_created(root: &Path, world_id: &str, before: &Snapshot) -> Option<String> {
    for (extension, existed_before) in [("png", before.world_card.0), ("import.json", before.world_card.1)]
    {
        if existed_before {
            continue;
        }
        if data::world_card_path(root, world_id, extension).is_ok_and(|path| path.exists()) {
            return Some(extension.to_owned());
        }
    }
    None
}

/// 這次匯入是否新建了 GM 卡的圖（gm.png）；匯入前就有的不算（含被覆寫掉的舊圖）。
fn detect_gm_image_created(root: &Path, world_id: &str, before: &Snapshot) -> bool {
    !before.gm_image_existed
        && data::gm_image_path(root, world_id).is_ok_and(|path| path.exists())
}

/// 這次操作是否新建了介面渲染殼檔（interface-shell.html）；套用前就有的殼不算。
fn detect_interface_shell_created(root: &Path, world_id: &str, before: &Snapshot) -> bool {
    !before.interface_shell_existed
        && data::interface_shell_path(root, world_id).is_ok_and(|path| path.exists())
}

/// import_character 指令成功後呼叫：新角色卡本身永遠是這次匯入的具體產物，一律留一筆收據
/// （即使卡片沒帶世界書／機制資料，「刪掉這張新卡」仍然是有意義的復原動作）。
pub fn record_character_import(
    root: &Path,
    world_id: &str,
    character_id: &str,
    label: &str,
    before: Snapshot,
) {
    let worldbook_entries = new_worldbook_entries(root, world_id, &before.worldbook_uids);
    let mechanism = diff_mechanism(before.state.as_ref(), root, world_id);
    append_receipt(
        root,
        world_id,
        ImportReceipt {
            kind: "character".to_owned(),
            label: label.to_owned(),
            timestamp: data::local_timestamp().unwrap_or_default(),
            character_id: Some(character_id.to_owned()),
            character_ids: Vec::new(),
            worldbook_entries,
            mechanism,
            world_card_created: None,
            gm_image_created: false,
            opening: None,
            renamed_from: None,
            rewritten_entries: Vec::new(),
            added_ledger_lines: String::new(),
            interface_shell_created: false,
        },
    );
}

/// import_worldbook 指令成功後呼叫：這條路徑沒有角色卡本體，實際新增為零（entries／機制／
/// 介面殼皆無變化，例如整份都跟既有內容重複）就不留空收據——按鈕不該對「什麼都沒發生」的
/// 匯入也亮出來，連按復原時也不該吃到一筆空操作。
pub fn record_worldbook_import(root: &Path, world_id: &str, label: &str, before: Snapshot) {
    let worldbook_entries = new_worldbook_entries(root, world_id, &before.worldbook_uids);
    let mechanism = diff_mechanism(before.state.as_ref(), root, world_id);
    let world_card_created = detect_world_card_created(root, world_id, &before);
    let gm_image_created = detect_gm_image_created(root, world_id, &before);
    if worldbook_entries.is_empty()
        && mechanism.is_none()
        && world_card_created.is_none()
        && !gm_image_created
    {
        return;
    }
    append_receipt(
        root,
        world_id,
        ImportReceipt {
            kind: "worldbook".to_owned(),
            label: label.to_owned(),
            timestamp: data::local_timestamp().unwrap_or_default(),
            character_id: None,
            character_ids: Vec::new(),
            worldbook_entries,
            mechanism,
            world_card_created,
            gm_image_created,
            opening: None,
            renamed_from: None,
            rewritten_entries: Vec::new(),
            added_ledger_lines: String::new(),
            interface_shell_created: false,
        },
    );
}

/// refactor_apply 指令成功後呼叫：AI 卡重構可能一次新增多張角色卡、多條世界書條目，
/// 並改寫或停用既有條目——收據記「實際套用的那份」，undo 才能逐項退回。
/// 跟 record_worldbook_import 一樣：實際套用為零就不留空收據。
pub fn record_refactor_apply(
    root: &Path,
    world_id: &str,
    label: &str,
    character_ids: Vec<String>,
    rewritten_entries: Vec<WorldbookEntry>,
    before: Snapshot,
) {
    let worldbook_entries = new_worldbook_entries(root, world_id, &before.worldbook_uids);
    let mechanism = diff_mechanism(before.state.as_ref(), root, world_id);
    let added_ledger_lines = diff_ledger_suffix(&before.mechanism_log_before, root, world_id);
    let interface_shell_created = detect_interface_shell_created(root, world_id, &before);
    if character_ids.is_empty()
        && worldbook_entries.is_empty()
        && mechanism.is_none()
        && rewritten_entries.is_empty()
        && added_ledger_lines.is_empty()
        && !interface_shell_created
    {
        return;
    }
    append_receipt(
        root,
        world_id,
        ImportReceipt {
            kind: "refactor".to_owned(),
            label: label.to_owned(),
            timestamp: data::local_timestamp().unwrap_or_default(),
            character_id: None,
            character_ids,
            worldbook_entries,
            mechanism,
            world_card_created: None,
            gm_image_created: false,
            opening: None,
            renamed_from: None,
            rewritten_entries,
            added_ledger_lines,
            interface_shell_created,
        },
    );
}

/// adoptImportName 改名成功後呼叫：把舊桌名補進最後一筆收據，undo 時桌名才退得回去。
/// 這桌還沒有任何收據（例如那次匯入本身沒留收據）就悄悄放棄——改名已經成功了，
/// 不該因為記帳補不上而報錯。
pub fn record_last_import_rename(root: &Path, world_id: &str, old_name: &str) {
    let mut receipts = read_receipts(root, world_id);
    let Some(last) = receipts.last_mut() else {
        return;
    };
    last.renamed_from = Some(old_name.to_owned());
    let _ = write_receipts(root, world_id, &receipts);
}

pub fn list_import_receipts(root: &Path, world_id: &str) -> Vec<ImportReceiptSummary> {
    read_receipts(root, world_id)
        .into_iter()
        .map(|receipt| ImportReceiptSummary {
            kind: receipt.kind,
            label: receipt.label,
            timestamp: receipt.timestamp,
            character_id: receipt.character_id,
        })
        .collect()
}

/// 逆向最後一筆收據。收據檔存在但解析失敗要回錯（不能悄悄當空，不然玩家以為復原了
/// 其實什麼都沒發生）；缺檔／空陣列＝沒有可復原的紀錄，同樣回錯。
pub fn undo_last_import(root: &Path, world_id: &str) -> DataResult<UndoReport> {
    let path = data::import_receipts_path(root, world_id)?;
    let mut receipts: Vec<ImportReceipt> = if path.exists() {
        let text = fs::read_to_string(&path)?;
        serde_json::from_str(&text)
            .map_err(|error| data::invalid_data(format!("匯入收據已損毀，無可復原：{error}")))?
    } else {
        Vec::new()
    };
    let receipt = receipts
        .pop()
        .ok_or_else(|| data::invalid_data("沒有可復原的匯入紀錄"))?;

    let mut report = UndoReport::default();

    // 1. 角色卡：md／原始檔／圖片／圖庫一併刪除——按鈕語意是撤銷這次匯入，玩家後續編輯一併退場。
    if let Some(character_id) = &receipt.character_id {
        if data::delete_character(root, world_id, character_id).is_ok() {
            report.removed_character = Some(receipt.label.clone());
        }
        // delete_character 不清 .import.json（非 PNG 匯入時的原始檔，PNG 匯入時原始檔與
        // 角色圖是同一個 .png，已經被 delete_character 清掉）。
        if let Ok(character_path) = data::character_path(root, world_id, character_id) {
            let _ = fs::remove_file(character_path.with_extension("import.json"));
        }
    }
    // AI 卡重構等一次套用多張角色卡的路徑：character_id／character_ids 兩欄位互斥，
    // 沒有共用的「單一 label＝角色名」可用，逐張讀當下名字再刪。
    for character_id in &receipt.character_ids {
        let name = data::read_character(root, world_id, character_id)
            .ok()
            .map(|card| card.name);
        if data::delete_character(root, world_id, character_id).is_ok() {
            if let Some(name) = name {
                report.removed_characters.push(name);
            }
            if let Ok(character_path) = data::character_path(root, world_id, character_id) {
                let _ = fs::remove_file(character_path.with_extension("import.json"));
            }
        }
    }

    // 2. 世界書條目：uid 還在且指紋沒變才刪；指紋變了＝玩家改過，保留並計數。
    let current = data::read_worldbook(root, world_id).unwrap_or_default();
    for recorded in &receipt.worldbook_entries {
        let Some(entry) = current.iter().find(|entry| entry.uid == recorded.uid) else {
            continue; // 已經不在了（例如玩家自己刪過），不重複計數
        };
        if worldbook_entry_fingerprint(entry) == recorded.fingerprint {
            let _ = data::delete_worldbook_entry(root, world_id, recorded.uid);
            report.removed_entries += 1;
        } else {
            report.kept_entries += 1;
        }
    }

    // 3. 被改寫或停用的既有條目（AI 卡重構的來源條目改寫、介面／機制的 source_uid 停用）：
    // 整條覆寫回原文快照。uid 已經不在了（玩家自己刪過）就略過，不當新條目生回來。
    let current_uids: HashSet<u64> = data::read_worldbook(root, world_id)
        .unwrap_or_default()
        .into_iter()
        .map(|entry| entry.uid)
        .collect();
    for original in &receipt.rewritten_entries {
        if current_uids.contains(&original.uid) {
            let _ = data::upsert_worldbook_entry(root, world_id, original.clone());
        }
    }

    // 4. 機制／狀態樹：只退回這次匯入自己造成的鍵，其餘（別筆匯入或期間的正常遊玩）不動。
    if let Some(mechanism) = &receipt.mechanism {
        if let Ok(mut state) = data::read_state(root, world_id) {
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
            }
            apply_map_undo(
                &mut state.state.tree,
                &mechanism.added_state_keys,
                &mechanism.restored_state,
            );
            let _ = data::write_state(root, world_id, &state);
        }
    }

    // 5. 機制帳本：這次操作自己追加的那段原文整段挖掉，其餘（含期間新產生的遊玩紀錄）不動。
    if !receipt.added_ledger_lines.is_empty() {
        if let Ok(path) = data::mechanism_log_path(root, world_id) {
            if let Ok(current) = fs::read_to_string(&path) {
                if let Some(index) = current.rfind(receipt.added_ledger_lines.as_str()) {
                    let mut restored = current[..index].to_owned();
                    restored.push_str(&current[index + receipt.added_ledger_lines.len()..]);
                    let _ = fs::write(&path, restored);
                }
            }
        }
    }

    // 6. 這次匯入新建的卡片介面殼：匯入前就有的不動。
    if let Some(extension) = &receipt.world_card_created {
        if let Ok(card_path) = data::world_card_path(root, world_id, extension) {
            let _ = fs::remove_file(card_path);
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
            let _ = fs::remove_file(image_path);
        }
    }

    // 7. 這次操作新建的介面渲染殼檔（AI 卡重構產物）：套用前就有的不動。
    if receipt.interface_shell_created {
        if let Ok(shell_path) = data::interface_shell_path(root, world_id) {
            let _ = fs::remove_file(shell_path);
        }
    }

    // 8. 桌名：這次匯入有改過名字才退回去。
    if let Some(old_name) = &receipt.renamed_from {
        if data::rename_world(root, world_id, old_name).is_ok() {
            report.renamed_back = true;
        }
    }

    write_receipts(root, world_id, &receipts)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(label: &str) -> Self {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "table-tavern-receipts-{label}-{}-{id}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn import_character_recorded(root: &Path, world_id: &str, raw: &[u8]) -> data::CharacterMeta {
        let before = snapshot(root, world_id);
        let meta = import::import_character(root, world_id, raw, "#3366ff").unwrap();
        record_character_import(root, world_id, &meta.id, &meta.name, before);
        meta
    }

    fn import_worldbook_recorded(root: &Path, world_id: &str, label: &str, json_text: &str) {
        let before = snapshot(root, world_id);
        data::import_worldbook(root, world_id, json_text).unwrap();
        record_worldbook_import(root, world_id, label, before);
    }

    fn transcript_event(ts: &str, text: &str) -> data::TranscriptEvent {
        data::TranscriptEvent {
            ts: ts.to_owned(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: data::TranscriptKind::Narration,
            text: text.to_owned(),
            raw: None,
            state: None,
            gm_only: false,
        }
    }

    fn character_book_card(name: &str, entries: serde_json::Value) -> Vec<u8> {
        serde_json::json!({
            "data": { "name": name, "character_book": { "entries": entries } }
        })
        .to_string()
        .into_bytes()
    }

    /// 角色卡匯入→undo：角色 md／原始檔消失、帶入的世界書條目消失、匯入前既有同名條目仍在。
    #[test]
    fn undo_character_import_removes_card_and_its_entries_but_keeps_preexisting() {
        let root = TestRoot::new("character-basic");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let existing = data::upsert_worldbook_entry(
            root.path(),
            &world_id,
            data::WorldbookEntry {
                uid: 0,
                title: "既有設定".to_owned(),
                keys: vec!["鎮".to_owned()],
                content: "霧口鎮的既有設定".to_owned(),
                constant: true,
                order: 1,
                disabled: false,
                visibility: data::Visibility::Gm,
                is_person: false,
            },
        )
        .unwrap();

        let raw = character_book_card(
            "莉亞",
            serde_json::json!([{"keys": ["森林"], "content": "古老盟約", "enabled": true}]),
        );
        let meta = import_character_recorded(root.path(), &world_id, &raw);
        let md_path = data::character_path(root.path(), &world_id, &meta.id).unwrap();
        assert!(md_path.exists());
        assert!(md_path.with_extension("import.json").exists());
        let entries_after_import = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(entries_after_import.len(), 2);

        let report = undo_last_import(root.path(), &world_id).unwrap();
        assert_eq!(report.removed_character, Some("莉亞".to_owned()));
        assert_eq!(report.removed_entries, 1);
        assert_eq!(report.kept_entries, 0);
        assert!(!report.renamed_back);

        assert!(data::read_character(root.path(), &world_id, &meta.id).is_err());
        assert!(!md_path.exists());
        assert!(!md_path.with_extension("import.json").exists());
        let remaining = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].uid, existing);
        assert_eq!(remaining[0].title, "既有設定");
    }

    /// undo 時被玩家改過內容的條目要保留，且 kept_entries 正確計數。
    #[test]
    fn undo_keeps_entries_the_player_edited_since_import() {
        let root = TestRoot::new("character-kept");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let raw = character_book_card(
            "莉亞",
            serde_json::json!([
                {"keys": ["森林"], "content": "古老盟約", "enabled": true},
                {"keys": ["月亮"], "content": "月神信仰", "enabled": true}
            ]),
        );
        import_character_recorded(root.path(), &world_id, &raw);
        let entries = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(entries.len(), 2);

        // 玩家改掉其中一條的內文
        let mut edited = entries[0].clone();
        edited.content = "玩家改過的內容".to_owned();
        data::upsert_worldbook_entry(root.path(), &world_id, edited).unwrap();

        let report = undo_last_import(root.path(), &world_id).unwrap();
        assert_eq!(report.removed_entries, 1);
        assert_eq!(report.kept_entries, 1);
        let remaining = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].content, "玩家改過的內容");
    }

    /// 世界書匯入（非角色卡路徑）→undo→條目消失。
    #[test]
    fn undo_worldbook_import_removes_its_entries() {
        let root = TestRoot::new("worldbook-basic");
        let world_id = data::create_world(root.path(), "世界").unwrap();
        let book = serde_json::json!({
            "entries": {
                "0": {"uid": 0, "key": ["城門"], "comment": "城門", "content": "城門已關。", "constant": false, "order": 1, "disable": false}
            }
        });
        import_worldbook_recorded(root.path(), &world_id, "worldbook.json", &book.to_string());
        assert_eq!(data::read_worldbook(root.path(), &world_id).unwrap().len(), 1);

        let report = undo_last_import(root.path(), &world_id).unwrap();
        assert_eq!(report.removed_entries, 1);
        assert!(report.removed_character.is_none());
        assert!(data::read_worldbook(root.path(), &world_id).unwrap().is_empty());
    }

    /// PNG 世界書匯入→undo：這次新建的 GM 卡圖跟著收掉（回到內建書本圖）；
    /// 第二張 PNG 只是覆寫既有的圖，undo 不刪——那張圖不是這次匯入生出來的。
    #[test]
    fn undo_worldbook_import_removes_only_gm_image_created_this_time() {
        let root = TestRoot::new("worldbook-gm-image");
        let world_id = data::create_world(root.path(), "世界").unwrap();
        let image_path = data::gm_image_path(root.path(), &world_id).unwrap();
        let book = |content: &str| {
            serde_json::json!({
                "entries": {
                    "0": {"uid": 0, "key": ["城門"], "comment": "城門", "content": content, "constant": false, "order": 1, "disable": false}
                }
            })
            .to_string()
        };
        let import_png = |label: &str, json_text: &str, png: &[u8]| {
            let before = snapshot(root.path(), &world_id);
            data::import_worldbook(root.path(), &world_id, json_text).unwrap();
            assert!(import::save_gm_image(root.path(), &world_id, png));
            record_worldbook_import(root.path(), &world_id, label, before);
        };

        import_png("第一張.png", &book("城門已關。"), b"\x89PNG\r\n\x1a\nfirst");
        assert!(image_path.exists());
        undo_last_import(root.path(), &world_id).unwrap();
        assert!(!image_path.exists());

        import_png("第一張.png", &book("城門已關。"), b"\x89PNG\r\n\x1a\nfirst");
        import_png("第二張.png", &book("城門又開了。"), b"\x89PNG\r\n\x1a\nsecond");
        undo_last_import(root.path(), &world_id).unwrap();
        assert_eq!(fs::read(&image_path).unwrap(), b"\x89PNG\r\n\x1a\nsecond");
    }

    /// 匯完貼上檯面的開場白→undo：那則跟著收掉，玩家自己後來加的話原封不動。
    /// 重匯同一張多開場白的卡想改挑一則時，舊的那則不該還壓在開局上。
    #[test]
    fn undo_import_removes_the_posted_opening_but_keeps_later_events() {
        let root = TestRoot::new("posted-opening");
        let world_id = data::create_world(root.path(), "世界").unwrap();
        let book = serde_json::json!({
            "entries": {
                "0": {"uid": 0, "key": ["城門"], "comment": "城門", "content": "城門已關。", "constant": false, "order": 1, "disable": false}
            }
        });
        import_worldbook_recorded(root.path(), &world_id, "卡.png", &book.to_string());

        let opening = transcript_event("2026-08-07T10:00:00.000Z", "開場白配圖那一段");
        let mine = transcript_event("2026-08-07T10:05:00.000Z", "玩家後來自己加的一句");
        data::append_transcript(root.path(), &world_id, 0, &opening).unwrap();
        data::append_transcript(root.path(), &world_id, 0, &mine).unwrap();
        record_posted_opening(root.path(), &world_id, 0, &opening.ts);

        let report = undo_last_import(root.path(), &world_id).unwrap();
        assert!(report.removed_opening);
        let left = data::read_transcript(root.path(), &world_id, 0).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].ts, mine.ts);
    }

    /// 玩家自己先把開場白收回去了：undo 找不到那則，回報沒收掉，不誤刪別的事件。
    #[test]
    fn undo_import_reports_no_opening_when_player_already_took_it_back() {
        let root = TestRoot::new("posted-opening-gone");
        let world_id = data::create_world(root.path(), "世界").unwrap();
        let book = serde_json::json!({
            "entries": {
                "0": {"uid": 0, "key": ["城門"], "comment": "城門", "content": "城門已關。", "constant": false, "order": 1, "disable": false}
            }
        });
        import_worldbook_recorded(root.path(), &world_id, "卡.png", &book.to_string());

        let opening = transcript_event("2026-08-07T10:00:00.000Z", "開場白");
        data::append_transcript(root.path(), &world_id, 0, &opening).unwrap();
        record_posted_opening(root.path(), &world_id, 0, &opening.ts);
        assert!(data::pop_transcript(root.path(), &world_id, 0).unwrap());

        let report = undo_last_import(root.path(), &world_id).unwrap();
        assert!(!report.removed_opening);
    }

    /// 什麼都沒新增的匯入沒留收據，這時貼開場白不該掛到更早那筆收據上，
    /// 否則復原上一筆匯入會連帶刪掉不相干的開場白。
    #[test]
    fn record_posted_opening_without_any_receipt_is_a_no_op() {
        let root = TestRoot::new("posted-opening-no-receipt");
        let world_id = data::create_world(root.path(), "世界").unwrap();

        record_posted_opening(root.path(), &world_id, 0, "2026-08-07T10:00:00.000Z");

        assert!(list_import_receipts(root.path(), &world_id).is_empty());
    }

    /// 全部重複、機制／介面殼都沒變化的世界書匯入不留收據——不該亮出可以「復原」的按鈕。
    #[test]
    fn worldbook_import_with_nothing_new_leaves_no_receipt() {
        let root = TestRoot::new("worldbook-noop");
        let world_id = data::create_world(root.path(), "世界").unwrap();
        let book = serde_json::json!({
            "entries": {
                "0": {"uid": 0, "key": ["城門"], "comment": "城門", "content": "城門已關。", "constant": false, "order": 1, "disable": false}
            }
        });
        import_worldbook_recorded(root.path(), &world_id, "first.json", &book.to_string());
        assert_eq!(list_import_receipts(root.path(), &world_id).len(), 1);

        // 同一份書再匯一次：內容全部重複，不該多一筆收據
        import_worldbook_recorded(root.path(), &world_id, "second.json", &book.to_string());
        assert_eq!(list_import_receipts(root.path(), &world_id).len(), 1);
    }

    /// 兩筆收據連按兩次 undo：逐筆倒退，順序正確（後進先出）。
    #[test]
    fn two_receipts_undo_in_reverse_order() {
        let root = TestRoot::new("sequential");
        let world_id = data::create_world(root.path(), "世界").unwrap();
        let first = import_character_recorded(
            root.path(),
            &world_id,
            &character_book_card("甲", serde_json::json!([])),
        );
        let second = import_character_recorded(
            root.path(),
            &world_id,
            &character_book_card("乙", serde_json::json!([])),
        );
        assert_eq!(data::list_characters(root.path(), &world_id).unwrap().len(), 2);

        let report1 = undo_last_import(root.path(), &world_id).unwrap();
        assert_eq!(report1.removed_character, Some("乙".to_owned()));
        assert!(data::read_character(root.path(), &world_id, &first.id).is_ok());
        assert!(data::read_character(root.path(), &world_id, &second.id).is_err());

        let report2 = undo_last_import(root.path(), &world_id).unwrap();
        assert_eq!(report2.removed_character, Some("甲".to_owned()));
        assert!(data::read_character(root.path(), &world_id, &first.id).is_err());
        assert!(list_import_receipts(root.path(), &world_id).is_empty());

        // 收據已經清空，再按一次要回錯而不是 panic
        assert!(undo_last_import(root.path(), &world_id).is_err());
    }

    /// 收據檔內容損毀：undo 回傳「無可復原」錯誤，不 panic；缺檔（從沒匯入過）同樣回錯。
    #[test]
    fn undo_reports_error_without_panicking_on_missing_or_corrupt_receipts() {
        let root = TestRoot::new("corrupt");
        let world_id = data::create_world(root.path(), "世界").unwrap();
        assert!(undo_last_import(root.path(), &world_id).is_err());

        fs::write(
            data::import_receipts_path(root.path(), &world_id).unwrap(),
            "{ 不是合法 JSON 陣列",
        )
        .unwrap();
        assert!(undo_last_import(root.path(), &world_id).is_err());
    }

    /// renamed_from 存在時，undo 後桌名要退回去。
    #[test]
    fn undo_restores_table_name_when_renamed_from_is_recorded() {
        let root = TestRoot::new("renamed");
        let world_id = data::create_world(root.path(), "新的一桌").unwrap();
        import_character_recorded(
            root.path(),
            &world_id,
            &character_book_card("莉亞", serde_json::json!([])),
        );
        record_last_import_rename(root.path(), &world_id, "新的一桌");
        data::rename_world(root.path(), &world_id, "莉亞").unwrap();

        let report = undo_last_import(root.path(), &world_id).unwrap();
        assert!(report.renamed_back);
        assert_eq!(data::read_state(root.path(), &world_id).unwrap().name, "新的一桌");
    }

    /// 機制／狀態樹的倒退：undo 只退這一筆匯入自己加的規則與初始值，
    /// 較早那筆（尚未復原）帶進來的規則要留著——等同「undo 後等同沒匯入過」只作用在這一筆。
    #[test]
    fn undo_reverts_only_this_imports_mechanism_writes() {
        let root = TestRoot::new("mechanism");
        let world_id = data::create_world(root.path(), "世界").unwrap();

        let first_raw = character_book_card(
            "甲",
            serde_json::json!([{
                "comment": "[initvar] 初始值",
                "enabled": false,
                "content": "World:\n  Time: 清晨"
            }]),
        );
        import_character_recorded(root.path(), &world_id, &first_raw);
        assert_eq!(
            data::read_state(root.path(), &world_id)
                .unwrap()
                .state
                .tree
                .get("World"),
            Some(&data::StateNode::Branch(BTreeMap::from([(
                "Time".to_owned(),
                data::StateNode::Leaf("清晨".to_owned()),
            )])))
        );

        let second_raw = character_book_card(
            "乙",
            serde_json::json!([{
                "comment": "[mvu_update]规则",
                "enabled": true,
                "content": "变量更新规则:\n  Player:\n    HP:\n      type: number\n      range: 0-100"
            }]),
        );
        import_character_recorded(root.path(), &world_id, &second_raw);
        let mid_state = data::read_state(root.path(), &world_id).unwrap();
        assert!(mid_state.mechanism.incremental);
        assert!(mid_state.mechanism.rules.contains_key("Player.HP"));

        // 復原第二筆（乙）：HP 規則消失，但 incremental 仍是 true——甲的 [initvar] 自己就會
        // 把這桌標成增量桌（import_mechanism：`initial_tree.is_some() || mvu_seen`），甲還沒
        // 復原，這面旗子不該被乙的 undo 連帶關掉；甲帶進來的初始樹同樣要保留。
        let report = undo_last_import(root.path(), &world_id).unwrap();
        assert!(report.removed_character.is_some());
        let after_first_undo = data::read_state(root.path(), &world_id).unwrap();
        assert!(!after_first_undo.mechanism.rules.contains_key("Player.HP"));
        assert!(after_first_undo.mechanism.incremental);
        assert_eq!(
            after_first_undo.state.tree.get("World"),
            Some(&data::StateNode::Branch(BTreeMap::from([(
                "Time".to_owned(),
                data::StateNode::Leaf("清晨".to_owned()),
            )])))
        );

        // 再復原第一筆（甲）：這下 incremental 才真的退回 false，初始樹也一併消失。
        undo_last_import(root.path(), &world_id).unwrap();
        let after_second_undo = data::read_state(root.path(), &world_id).unwrap();
        assert!(!after_second_undo.mechanism.incremental);
        assert!(after_second_undo.state.tree.get("World").is_none());
    }

    /// list_import_receipts 的摘要即時反映 append／undo。
    #[test]
    fn list_import_receipts_reflects_append_and_undo() {
        let root = TestRoot::new("list");
        let world_id = data::create_world(root.path(), "世界").unwrap();
        assert!(list_import_receipts(root.path(), &world_id).is_empty());

        let meta = import_character_recorded(
            root.path(),
            &world_id,
            &character_book_card("莉亞", serde_json::json!([])),
        );
        let list = list_import_receipts(root.path(), &world_id);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].kind, "character");
        assert_eq!(list[0].label, "莉亞");
        assert_eq!(list[0].character_id.as_deref(), Some(meta.id.as_str()));

        undo_last_import(root.path(), &world_id).unwrap();
        assert!(list_import_receipts(root.path(), &world_id).is_empty());
    }

    /// 舊格式收據 JSON（沒有 character_ids／rewritten_entries 這兩個新欄位）照樣能解析，
    /// undo 照常運作——新欄位一律 #[serde(default)]，向後相容。
    #[test]
    fn undo_reads_old_format_receipt_without_new_fields() {
        let root = TestRoot::new("legacy-format");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let meta = import::import_character(
            root.path(),
            &world_id,
            &character_book_card("莉亞", serde_json::json!([])),
            "#3366ff",
        )
        .unwrap();

        // 手寫舊格式收據：只有新欄位加入前就存在的那些鍵。
        let legacy_json = serde_json::json!([{
            "kind": "character",
            "label": "莉亞",
            "timestamp": "2026-01-01 00:00",
            "character_id": meta.id,
        }]);
        fs::write(
            data::import_receipts_path(root.path(), &world_id).unwrap(),
            serde_json::to_string_pretty(&legacy_json).unwrap(),
        )
        .unwrap();

        let report = undo_last_import(root.path(), &world_id).unwrap();
        assert_eq!(report.removed_character, Some("莉亞".to_owned()));
        assert!(report.removed_characters.is_empty());
        assert!(data::read_character(root.path(), &world_id, &meta.id).is_err());
    }
}
