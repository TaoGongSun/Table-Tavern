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

mod character_delete;
mod record;
mod sources;
mod undo;
pub(crate) mod undo_changes;
pub use character_delete::{
    character_to_worldbook_entry_and_clean, delete_character_and_clean, upsert_entry_dropping_gone,
    CharacterDeleteOutcome,
};
pub use record::{
    pop_refactor_barrier, push_refactor_barrier, record_character_import,
    record_last_import_rename, record_posted_opening, record_refactor_apply,
    record_worldbook_import, RefactorRecord,
};
pub use sources::{
    begin_pending, discard_import_source, finish_pending, import_replays, store_import_source,
    ImportReplay, ImportRoute, ImportSource,
};
#[cfg(test)]
pub use undo::undo_last_import;
pub use undo::{rollback_last_import, undo_expected_import};
use undo_changes::EntryChanges;

/// 匯入記帳的結果：Failed＝收據讀壞被重寫或寫不進去，呼叫端的「未完成」標記就不能解除。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recorded {
    Yes,
    /// 什麼都沒新增，沒留收據（原檔已丟掉）
    NothingNew,
    Failed,
}

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
    /// AI 卡重構的介面骨架檔（interface-shell.html）快照時是否已存在——比照 world_card 的
    /// 存在性 diff 手法：undo 只該刪這次操作新建的殼，不動套用前就有的。
    interface_shell_existed: bool,
    /// 介面骨架檔快照時的內容：只有 snapshot_refactor 會讀（讀不到就整個快照失敗），其餘匯入
    /// 不碰殼檔、維持 None。收據靠它判斷這次套用有沒有刪掉或覆寫舊殼，undo 才寫得回去。
    interface_shell_before: Option<String>,
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
        interface_shell_before: None,
        mechanism_log_before: data::mechanism_log_path(root, world_id)
            .ok()
            .and_then(|path| fs::read_to_string(path).ok())
            .unwrap_or_default(),
    }
}

/// AI 卡重構套用前的快照：狀態與殼內容都必須讀得到——讀失敗回 Err、不套用。當成「沒有殼」
/// 放行的話，套用照樣刪掉或覆寫舊殼，收據卻沒有可寫回的內容，undo 就救不回來。
pub fn snapshot_refactor(root: &Path, world_id: &str) -> DataResult<Snapshot> {
    let mut snapshot = snapshot(root, world_id);
    snapshot.state = Some(data::read_state(root, world_id)?);
    snapshot.interface_shell_before = data::read_interface_shell(root, world_id)?;
    snapshot.interface_shell_existed = snapshot.interface_shell_before.is_some();
    Ok(snapshot)
}

/// 貼到檯面上的開場白：靠場景號＋事件時間戳定位，undo 時只刪這一則。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PostedOpening {
    scene: u64,
    ts: String,
    /// 玩家挑的是開場白清單（card_openings 順序）裡第幾則；重新重構時從匯入原檔取同一則
    #[serde(default, skip_serializing_if = "Option::is_none")]
    index: Option<usize>,
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
    /// 這次操作把玩家卡指定到某張新卡（AI 卡重構的人物合併升格）；refactor::apply 的一桌一張
    /// 限制已經保證這種情況發生時桌上原本沒有玩家卡，undo 一律退回 None，不需要另外記「原本
    /// 是誰」——用 Option<Option<String>> 記還會撞上 serde 把巢狀 None 序列化成 JSON null、
    /// 反序列化時分不出「沒變」還是「變成 None」的坑，乾脆用布林更直接。
    #[serde(default, skip_serializing_if = "data::is_false")]
    player_card_assigned: bool,
    /// 這次操作改了卡專屬回報指引（介面接管）：原值，undo 寫回；None＝沒改。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    guide_before: Option<String>,
    /// 這次操作改了骨架填值用的欄位型別表（mechanism.value_types）：整份原表（含原本的空表），undo 寫回；
    /// None＝沒改。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    value_types_before: Option<BTreeMap<String, String>>,
    /// 這次操作改了數字欄顯示模板表（mechanism.display_formats）：整份原表，undo 寫回；None＝沒改。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    display_formats_before: Option<BTreeMap<String, String>>,
    /// 這次操作改了顯示前綴表（mechanism.display_prefixes，statusbar-only-fields）：整份原表，undo 寫回；None＝沒改。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    display_prefixes_before: Option<BTreeMap<String, String>>,
    /// 這次操作改了開場補齊資料（mechanism.opening，statusbar-only-fields）：整份原值，undo 寫回；None＝沒改。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    opening_before: Option<crate::data::OpeningMeta>,
    /// 這次操作改了集合描述（mechanism.collections）：整份原表（含原本的空表），undo 寫回；None＝沒改。
    /// 不寫回的話撤銷後樹與規則回到舊樣子、描述卻還是新集合，GM 的寫入會被形狀檢查拒收。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    collections_before: Option<BTreeMap<String, data::CollectionSpec>>,
    /// 這次操作改了鏡像欄表（mechanism.mirrors）：整份原表，undo 寫回；None＝沒改。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mirrors_before: Option<BTreeMap<String, String>>,
    /// 這次操作改了卡片狀態欄退場標記（WorldState::statusbar_retired）：原值，undo 寫回；None＝沒改。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    statusbar_retired_before: Option<bool>,
    /// 這次操作從開場狀態平欄搬走的鍵與原值（重構收編規則時搬進樹），undo 插回平欄。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    removed_table: BTreeMap<String, String>,
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
            && !self.player_card_assigned
            && self.guide_before.is_none()
            && self.value_types_before.is_none()
            && self.display_formats_before.is_none()
            && self.display_prefixes_before.is_none()
            && self.opening_before.is_none()
            && self.collections_before.is_none()
            && self.mirrors_before.is_none()
            && self.statusbar_retired_before.is_none()
            && self.removed_table.is_empty()
    }
}

/// 玩法標記（refactor_mode）的倒退資訊：外層 Option 有值＝這次有改到標記，`before` 是套用前
/// 的值（None＝原本沒有標記）。不用 Option<Option<String>>：serde 往返會把「沒變」與「原本是
/// None」都變成 null，分不出來。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModeUndo {
    #[serde(default)]
    before: Option<String>,
}

/// AI 重構套用在收據清單放的屏障（不帶撤銷內容）：AI 套用無還原〔作者裁決 2026-08-11〕，復原撤到這裡就停，
/// 不跨過它去撤重構前的匯入〔作者裁決 2026-10-29〕。
pub const REFACTOR_AI_BARRIER_KIND: &str = "refactor_ai";

fn is_barrier(receipt: &ImportReceipt) -> bool {
    receipt.kind == REFACTOR_AI_BARRIER_KIND
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ImportReceipt {
    kind: String, // "character" | "worldbook" | "refactor" | "refactor_ai"（屏障）
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
    /// 這次操作新建了 AI 卡重構的介面骨架檔（interface-shell.html）；套用前就有的不記，
    /// undo 不動它。跟 world_card_created 是兩回事：那個是「卡片自帶殼」的原始檔，這個是
    /// AI 照搬卡每回合輸出格式產的骨架。
    #[serde(default, skip_serializing_if = "data::is_false")]
    interface_shell_created: bool,
    /// 這次操作刪掉或覆寫了套用前就有的介面骨架檔：原內容，undo 寫回。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    interface_shell_restore: Option<String>,
    /// 這次操作改了玩法標記：undo 退回套用前的值。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refactor_mode: Option<ModeUndo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    renamed_from: Option<String>,
    /// 這次操作改寫或停用了既有世界書條目（AI 卡重構的來源條目改寫、介面／機制的
    /// source_uid 停用）：整條原文快照，undo 時整條覆寫回去。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    rewritten_entries: Vec<WorldbookEntry>,
    /// 這次操作整條刪除的世界書條目（人物合併升格：專屬條目、收尾判定可刪的共用合集條目）：
    /// 整條原文快照，undo 時不論 uid 現在還在不在，一律無條件插回——這些是我們自己刪的，
    /// 不是玩家刪的，跟 rewritten_entries 的「uid 還在才復原」語意不同。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    deleted_entries: Vec<WorldbookEntry>,
    /// deleted_entries 各條的原始 JSON（同序）：撤銷照原始值插回，次要鍵、位置、來源卡等不會掉。
    /// 舊收據沒有這欄（或長度對不上）就照精簡條目插回。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    deleted_entries_raw: Vec<serde_json::Value>,
    /// 匯入時被去重合併改寫可見度／來源卡的既有條目：撤銷只還原這兩個欄位。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    visibility_restores: Vec<data::VisibilityRestore>,
    /// AI 卡重構把既有角色卡標成舞台卡：撤銷時兩個旗標都還等於 after 才一起退回 before，玩家改過就不動。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    stage_restores: Vec<data::StageChange>,
    /// 這次操作往「機制帳本」（mechanism-log.jsonl）追加的原文；undo 時整段挖掉，其餘
    /// （含期間新產生的遊玩紀錄）不動。目前只有 AI 卡重構套用機制那條路會寫非空值。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    added_ledger_lines: String,
    /// 這次匯入的原檔（角色卡／世界書路徑）：重新重構時照它清空重匯；undo 時連原檔一起刪。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    import_source: Option<ImportSource>,
}

/// 前端側欄按鈕與未來路由框用的摘要：不帶復原用的內部細節（指紋、機制差異）。
#[derive(Debug, Clone, Serialize)]
pub struct ImportReceiptSummary {
    pub kind: String,
    pub label: String,
    pub timestamp: String,
    pub character_id: Option<String>,
}

/// 撤銷確認框那筆收據的身分（收據清單長度＋摘要欄位）：時間戳只到分鐘，單靠它分不出同一分鐘的兩筆。
#[derive(Debug, Clone, Deserialize)]
pub struct ExpectedReceipt {
    pub count: usize,
    pub kind: String,
    pub label: String,
    pub timestamp: String,
    #[serde(default)]
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
    /// 卡檔確定已不在的角色 id：前端據此關編輯器、修剪草稿名單。
    pub removed_character_ids: Vec<String>,
    /// 世界書實際被改的條目與欄組；撤銷前後讀不到世界書就是 None。
    pub entry_changes: Option<EntryChanges>,
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

/// 收據檔一律整檔原子寫：寫到一半或改名失敗時整份舊收據原樣留著。
fn write_receipts(root: &Path, world_id: &str, receipts: &[ImportReceipt]) -> DataResult<()> {
    pop_receipts(root, world_id, receipts)
}

/// 原子寫整份收據（撤銷彈出、重構追加）：寫到一半失敗時整份舊收據原樣留著（不是半截 JSON）。
fn pop_receipts(root: &Path, world_id: &str, receipts: &[ImportReceipt]) -> DataResult<()> {
    let path = data::import_receipts_path(root, world_id)?;
    data::commit_world_write_atomic(&path, serde_json::to_string_pretty(receipts)?.as_bytes())?;
    Ok(())
}

/// 記帳失敗不得影響匯入是否成功；讀壞就當成沒有歷史直接重寫（等於放棄復原更早的匯入，但不會讓這次
/// 匯入跟著失敗）。讀壞或寫不進去都回 false：呼叫端據此不解除「未完成」標記，來源就判不完整。
fn append_receipt(root: &Path, world_id: &str, receipt: ImportReceipt) -> bool {
    let (mut receipts, intact) = match read_receipts_checked(root, world_id) {
        Some(receipts) => (receipts, true),
        None => (Vec::new(), false),
    };
    receipts.push(receipt);
    write_receipts(root, world_id, &receipts).is_ok() && intact
}

/// 重構記收據用原子寫：整份收據是覆寫重寫的，寫到一半失敗時先前的收據逐位元組留著，
/// 不會變成壞 JSON 而連上一次匯入的撤銷紀錄都丟掉。讀壞時不重寫（寫了會蓋掉先前的收據）。
fn append_receipt_atomic(root: &Path, world_id: &str, receipt: ImportReceipt) -> bool {
    let Some(mut receipts) = read_receipts_checked(root, world_id) else {
        return false;
    };
    receipts.push(receipt);
    pop_receipts(root, world_id, &receipts).is_ok()
}

/// 讀收據：缺檔＝沒有歷史（空清單）；檔案在但讀不了或壞掉回 None。
fn read_receipts_checked(root: &Path, world_id: &str) -> Option<Vec<ImportReceipt>> {
    let path = data::import_receipts_path(root, world_id).ok()?;
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).ok(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(Vec::new()),
        Err(_) => None,
    }
}

/// 條目的復原用指紋：標題／內文／關鍵字／恆定／順序／停用／可見度，字段等值就當「沒改過」。
/// FNV-1a（跨執行穩定，做法比照 lanes 的 events_fingerprint）。
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

#[cfg(test)]
mod tests;
