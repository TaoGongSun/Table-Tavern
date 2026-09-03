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
}

/// 一筆記帳，供面板列「哪些更新被擋下」用。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub kind: RecordKind,
    pub path: String,
    pub detail: String,
}

impl Record {
    pub(super) fn new(kind: RecordKind, path: String, detail: String) -> Self {
        Self { kind, path, detail }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Outcome {
    pub records: Vec<Record>,
    /// 自癒回饋句：只從 Rejected 記錄產生，給下一輪模型看該怎麼改。
    pub notes: Vec<String>,
    /// 這一輪真的改到樹的變動：路徑（點分）→ 顯示標記。被拒收／硬錯誤不進來，
    /// 骰值本地重擲也不算（狀態欄二期包 5：回合尾注入策略要靠這個標「哪裡變了」）。
    pub changes: BTreeMap<String, String>,
}
