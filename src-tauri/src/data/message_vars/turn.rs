//! 行程內每桌的執行期狀態：桌世代與 GM 回合紀錄（計畫 8.1「桌世代」、8.3「GM 回合」）。
//! 都不落檔：app 重開時宿主佇列與回合本來就清空。讀寫一律在短提交鎖內（`&CommitTx` 當憑證）。
use super::json::Json;
use crate::data::state_commit::CommitTx;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// GM 回合落檔的冪等鍵：同一回合的每一部分（正文、變動紀錄……）各自只落一次。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TurnKey {
    pub turn_id: String,
    pub part: String,
}

/// 回合的主事件：唯一會掛表的那一則。
pub const PART_MAIN: &str = "main";

/// 角色回覆的回合鍵 part（不走 GM 冪等路徑，只給世界書落地認回合）。
pub const PART_CHARACTER: &str = "character";

/// 開場白的回合鍵 part（帶變數副作用的開場白才有，只給世界書落地認回合）。
pub const PART_OPENING: &str = "opening";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// 模型生成中（不持鎖）：卡寫與面板手改回 busy
    Generating,
    /// 已套用狀態、等前端落正文：同樣 busy
    AwaitingAppend,
    /// 正文已落檔：放行
    Appended,
    /// 中止或出錯：不套狀態、不產生表
    Aborted,
}

/// 回合的正文：提交（或中止留下半截）時交給回合紀錄，前端一直沒落 `main` 時由後端代落。
#[derive(Debug, Clone)]
pub struct PendingMain {
    pub text: String,
    pub raw: Option<String>,
    /// 中止留下的半截一律 true；正常回覆的截斷標記只有前端知道，代落時不帶〔模型判斷·未裁決〕
    pub truncated: bool,
}

/// 回合裡還沒落檔的一部分（`main` 正文、`state_update` 變動紀錄……），照落檔順序排。
#[derive(Debug, Clone)]
pub struct PendingPart {
    pub part: String,
    pub event: crate::data::TranscriptEvent,
}

#[derive(Debug, Clone)]
pub struct TurnRecord {
    pub turn_id: String,
    pub phase: Phase,
    /// 開始當下的幕與桌世代：提交與落檔都要對得上，舊回合不會套到新幕或換過的桌
    pub scene: u64,
    pub generation: u64,
    /// 開始時固定的輸入：有效表（變數模式才有）
    pub input: Option<Json>,
    /// 樹模式開始的回合：提交時符合條件就在鎖內啟用變數模式，用這組巨集物化種子
    pub macros: Option<super::convert::Macros>,
    /// 提交算好、等 `main` 落檔時掛上的新表
    pub pending: Option<Json>,
    /// 還沒落檔的各部分：前端一直沒落（追加失敗、畫面被關）時，下一筆新事件追加前由後端依序代落
    pub parts: Vec<PendingPart>,
}

impl TurnRecord {
    pub fn blocks_writes(&self) -> bool {
        matches!(self.phase, Phase::Generating | Phase::AwaitingAppend)
    }
}

#[derive(Default)]
struct Runtime {
    generation: u64,
    turn: Option<TurnRecord>,
}

fn registry() -> &'static Mutex<HashMap<String, Runtime>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, Runtime>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn key(tx: &CommitTx<'_>) -> String {
    format!("{}\u{0}{}", tx.root.display(), tx.world_id)
}

fn with_runtime<T>(tx: &CommitTx<'_>, work: impl FnOnce(&mut Runtime) -> T) -> T {
    let mut table = registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    work(table.entry(key(tx)).or_default())
}

pub fn generation(tx: &CommitTx<'_>) -> u64 {
    with_runtime(tx, |runtime| runtime.generation)
}

/// 整桌交換／還原：世代加一、清掉回合紀錄。舊殼帶著舊世代的寫入一律 stale。
pub fn bump_generation(tx: &CommitTx<'_>) {
    with_runtime(tx, |runtime| {
        runtime.generation += 1;
        runtime.turn = None;
    });
}

/// 整桌交換／還原之後（備份還原、格式轉換、重設、匯入復原、重構套用、開場回復）：自己取短提交鎖。
pub fn world_swapped(root: &std::path::Path, world_id: &str) {
    crate::data::state_commit::with_commit(root, world_id, bump_generation);
}

pub fn turn(tx: &CommitTx<'_>) -> Option<TurnRecord> {
    with_runtime(tx, |runtime| runtime.turn.clone())
}

pub fn set_turn(tx: &CommitTx<'_>, record: TurnRecord) {
    with_runtime(tx, |runtime| runtime.turn = Some(record));
}

/// 只改 turn_id 相符的那一筆；不符回 false、什麼都不動。
pub fn update_turn(tx: &CommitTx<'_>, turn_id: &str, work: impl FnOnce(&mut TurnRecord)) -> bool {
    with_runtime(tx, |runtime| match runtime.turn.as_mut() {
        Some(record) if record.turn_id == turn_id => {
            work(record);
            true
        }
        _ => false,
    })
}

/// 這桌有沒有回合擋著卡寫與手改。
pub fn busy(tx: &CommitTx<'_>) -> bool {
    turn(tx).is_some_and(|record| record.blocks_writes())
}

/// 測試用：各階段的快照。
#[cfg(test)]
pub fn phases(tx: &CommitTx<'_>) -> std::collections::BTreeMap<String, Phase> {
    turn(tx)
        .map(|record| std::collections::BTreeMap::from([(record.turn_id, record.phase)]))
        .unwrap_or_default()
}
