use crate::data::{FieldRule, Trigger, WorldbookEntry};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 認人後的一位角色候選：資料可能併自好幾條世界書條目（人物合併，person-promote）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorCharacter {
    pub name: String,
    pub emoji: String,
    pub public_md: String,
    pub private_md: String,
    /// 這位角色的資料來源條目 uid 清單；只有單一專屬來源時長度為 1。
    pub source_uids: Vec<String>,
    /// 此人不升格為角色卡時，自己獨立世界書條目的全文。
    pub solo_entry_md: String,
    /// 盤點階段 AI 標記的疑似玩家本人；整份 RefactorOutcome.characters 至多一筆為 true。
    #[serde(default)]
    pub suspected_player: bool,
}

/// 散文介面指令抽成的狀態樹候選。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorInterface {
    pub state_fields: serde_json::Value,
    pub source_uids: Vec<String>,
    /// 解析失敗退原文的雙軌保底。
    pub raw: String,
    /// AI 順便產的完整 HTML 渲染殼（自包含單檔，佔位符待前端替換）；None＝沒產出或抽不出來，
    /// 不影響 state_fields——渲染殼是錦上添花，不是介面套用成不成立的條件。
    #[serde(default)]
    pub shell: Option<String>,
    /// 這張卡自己的欄位規則（點分路徑→規則）：數值欄要 delta、清單欄整份 replace 都靠它。
    /// 只有接管（interface_shell）變體會產，空的就照現值形狀推定。
    #[serde(default)]
    pub rules: BTreeMap<String, FieldRule>,
    /// 這張卡自己的回報指引：每回合必報哪些欄位、哪些變動才報，照卡原文的規定寫。
    /// 卡與卡的規矩差很多（有的每回合全量重印道具，有的只在變動時提），不能用一套通則蓋過去。
    #[serde(default)]
    pub guide: String,
}

/// 欄位規則＋觸發表候選；rules／triggers 直接複用 data.rs 既有機制型別，不新造平行型別。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorMechanism {
    pub source_uid: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rules: BTreeMap<String, FieldRule>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggers: Vec<Trigger>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorOutcome {
    #[serde(default)]
    pub characters: Vec<RefactorCharacter>,
    #[serde(default)]
    pub interface: Option<RefactorInterface>,
    #[serde(default)]
    pub mechanisms: Vec<RefactorMechanism>,
    #[serde(default)]
    pub entries: Vec<crate::refactor_ai::RefactorNewEntry>,
    /// 收尾階段判定「刪了只剩殘渣」的共用合集條目 uid；套用時還要所有共用這條的人都被勾選
    /// 才會真的刪（要點 7：基準是優先保留而非刪除）。
    #[serde(default)]
    pub deletable_shared_uids: Vec<String>,
    /// 本地零呼叫組裝（refactor_assemble::assemble_local）淘汰的整條／半條內容：預設不套用，
    /// 純粹隨產物保留供玩家展開查看、一鍵放回。apply() 不處理這三欄——落檔與否是前端 UI 的事。
    #[serde(default)]
    pub dropped: Vec<crate::refactor_assemble::RefactorDroppedEntry>,
    /// app 尚無執行機構、原文已照搬進 GM 規則條目的機制清單（資訊性，內容不會遺失）。
    #[serde(default)]
    pub unabsorbed: Vec<crate::refactor_assemble::RefactorUnabsorbedItem>,
    /// 機械稽核紅字：涵蓋漏網／機制守恆／拆組守恆／淘汰稽核，四類之一。
    #[serde(default)]
    pub audit: Vec<crate::refactor_assemble::RefactorAuditItem>,
    /// 產出時玩家選定的玩法："interface"｜"characters"；None＝舊產物，照 interface 行為。
    /// 套用時寫進 WorldState.refactor_mode；characters 並停用卡片介面 fallback。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorSelection {
    #[serde(default)]
    pub character_indices: Vec<usize>,
    #[serde(default)]
    pub apply_interface: bool,
    #[serde(default)]
    pub mechanism_indices: Vec<usize>,
    #[serde(default)]
    pub entry_indices: Vec<usize>,
    /// characters 裡要設成玩家卡的那一位；None＝不指定。不在 character_indices 裡的索引視同
    /// None（沒同時勾選成卡就不可能是玩家卡）。
    #[serde(default)]
    pub player_index: Option<usize>,
}

/// 套用摘要，前端顯示用。
#[derive(Debug, Clone, Default, Serialize)]
pub struct RefactorApplySummary {
    pub new_characters: usize,
    pub new_entries: usize,
    /// 合併升格後整條刪除的來源世界書條目數（專屬條目＋收尾判定可刪的共用合集條目）。
    pub deleted_entries: usize,
    pub rewritten_entries: usize,
    pub interface_applied: bool,
    pub mechanisms_applied: usize,
    pub player_assigned: bool,
}

/// apply() 的完整結果：summary 給前端，其餘給呼叫端組收據（receipts::record_refactor_apply）。
#[derive(Debug)]
pub struct RefactorApplyResult {
    pub summary: RefactorApplySummary,
    pub character_ids: Vec<String>,
    pub rewritten_entries: Vec<WorldbookEntry>,
    /// 整條刪除的來源條目原文快照；undo 時不論 uid 現在還在不在，一律無條件插回。
    pub deleted_entries: Vec<WorldbookEntry>,
}

/// 讀取端玩法標記正規化：舊版可能已落地 "Characters"／帶空白的值——合法值就地修正大小寫
/// 與空白，真未知值回 Err 讓 controller 維持未知（fail-closed 不 fallback），不回 None
/// 冒充「確定沒標記」。
pub fn normalize_stored_mode(stored: Option<String>) -> Result<Option<String>, String> {
    let Some(raw) = stored else {
        return Ok(None);
    };
    let mode = raw.trim().to_ascii_lowercase();
    if mode == "interface" || mode == "characters" {
        Ok(Some(mode))
    } else {
        Err("refactor-mode-invalid".to_owned())
    }
}
