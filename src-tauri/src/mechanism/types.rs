use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchOp {
    Replace,
    Delta,
    Insert,
    Remove,
    Move,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Patch {
    pub op: PatchOp,
    pub path: Vec<String>,
    /// 只有 Move 用得到；其餘 op 固定空。
    pub from: Vec<String>,
    pub value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordKind {
    Rejected,
    Clamped,
    Error,
    /// 機制鷹架條目（[initvar]／[mvu_update]／整棵樹重送巨集）已被系統接管，不再送模型。
    Absorbed,
    /// 卡片腳本認不出來（隨機事件庫、要跑迴圈統計的判定等），沒轉成觸發表，預設也不送模型。
    Skipped,
    /// 全量桌跳動警示：這一欄一回合內變動幅度超過保守門檻，疑似模型算錯。只給玩家看，
    /// `build_notes` 不理它——不是拒收，沒有東西要模型改。
    Jump,
    /// 只落記帳檔、不回饋模型、面板不計：依桌型規定不收的內容（介面接管桌開場白自帶的原卡狀態區塊），
    /// 以及清單欄鍵名改回原卡寫法的校正（`list_keys`）。
    Ignored,
}

/// 一筆記帳，供面板列「哪些更新被擋下」用。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub kind: RecordKind,
    pub path: String,
    pub detail: String,
    /// 「路徑不存在」的回饋分組：同一批撞同一個不存在前綴的合成一條回饋。只在記憶體，帳本照舊一筆一筆記。
    #[serde(skip)]
    pub(crate) group: Option<Box<NoteGroup>>,
}

impl Record {
    pub(super) fn new(kind: RecordKind, path: String, detail: String) -> Self {
        Self {
            kind,
            path,
            detail,
            group: None,
        }
    }
}

/// 一條可合併的回饋句：`head` ＋ `intro` ＋ 列鍵 ＋ `tail`，合併時再補「本輪 N 筆未套用：…」。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NoteGroup {
    pub(super) key: String,
    pub(super) head: String,
    pub(super) intro: String,
    /// 合併兩筆以上時改用的開頭與列鍵引言（同一層多個新鍵時不點名單一路徑）
    pub(super) merged_head: String,
    pub(super) merged_intro: String,
    pub(super) items: Vec<String>,
    pub(super) max_items: usize,
    pub(super) tail: String,
    /// 這一筆原本要寫的路徑末段（合併尾綴列的鍵名）
    pub(super) leaf: String,
}

/// 解析 `<UpdateVariable>` 的一個結果：可套用的指令，或解析時就拒收的回饋（照原位置排）。
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Parsed {
    Patch(Patch),
    Note(Record),
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Outcome {
    pub records: Vec<Record>,
    /// 自癒回饋句：只從 Rejected 記錄產生，給下一輪模型看該怎麼改。
    pub notes: Vec<String>,
    /// 這一輪真的改到樹的變動：路徑（點分）→ 顯示標記。被拒收／硬錯誤不進來，
    /// 骰值本地重擲也不算（狀態欄二期包 5：回合尾注入策略要靠這個標「哪裡變了」）。
    pub changes: BTreeMap<String, String>,
    /// 上游 set 這一批做完的完整帶型別 stat_data（只在沒重構的 MVU 卡、變數模式出現）：合回新表的底。不送前端。
    #[serde(skip)]
    pub typed: Option<crate::data::message_vars::TypedBatch>,
}
