//! MVU 引擎的輸入輸出型別與上限。回傳契約對應 src/features/mvu-engine/host-contract.ts（規格 1.4 節）。

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::fmt;
use std::time::Duration;

/// 共用核心的 PARSE_LIMITS（source-limits.ts）：輸入在進引擎前先擋，輸出回來再驗一次（卡片腳本與核心同一個 realm）。
pub const MESSAGE_BYTES: usize = 256 * 1024;
pub const TABLE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_RESULTS: usize = 1000;
pub const MAX_CHANGES: usize = 2000;
/// 卡片 schema 腳本原文上限：真卡 3–7 KB〔模型判斷·未裁決〕
pub const SCHEMA_SOURCE_BYTES: usize = 256 * 1024;

/// 資源上限與宿主時限（規格 2.2 節）。測試用小數字跑，正式值見 `Default`。
#[derive(Debug, Clone)]
pub struct Limits {
    /// QuickJS 記憶體上限
    pub memory_bytes: usize,
    /// QuickJS 堆疊上限（跑在 `thread_stack_bytes` 的專用執行緒上）
    pub stack_bytes: usize,
    pub thread_stack_bytes: usize,
    /// 整次處理期限：呼叫端用 `MvuEngine::deadline()` 起算一次，同一次處理的每個請求共用
    pub batch: Duration,
    /// 外層逾時＝期限＋這段寬限；到了還沒回覆就作廢執行緒
    pub outer_grace: Duration,
    /// 閒置多久丟掉 runtime
    pub idle: Duration,
    /// 同時保有 runtime 的桌數（LRU 淘汰）
    pub max_tables: usize,
    /// 執行中（含排隊）＋作廢未結束的請求名額（全域）：滿了新請求一律 Busy，作廢卡死的執行緒因此不會超過這個數
    pub max_busy_threads: usize,
    /// `respawn_window` 內作廢重開的次數上限
    pub max_respawns: usize,
    pub respawn_window: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            memory_bytes: 64 * 1024 * 1024,
            stack_bytes: 1024 * 1024,
            thread_stack_bytes: 8 * 1024 * 1024,
            batch: Duration::from_secs(5),
            outer_grace: Duration::from_secs(1),
            idle: Duration::from_secs(10 * 60),
            max_tables: 2,
            // 名額全域、重開次數每桌〔模型判斷·三方共識〕
            max_busy_threads: 2,
            max_respawns: 3,
            respawn_window: Duration::from_secs(10 * 60),
        }
    }
}

/// 卡片的變數 schema 腳本原文（ES module，import 卡片的 mvu_zod）與雜湊；雜湊或 `plain` 變了就重建 runtime。
#[derive(Debug)]
pub struct SchemaScript {
    pub source: String,
    pub hash: [u8; 32],
    /// 無 schema（非 zod 桌只借共用核心算開場白的 `<initvar>`）：不載卡片模組、不要求 registerMvuSchema
    pub plain: bool,
}

impl SchemaScript {
    pub fn new(source: String) -> Self {
        let hash = Sha256::digest(source.as_bytes()).into();
        Self {
            source,
            hash,
            plain: false,
        }
    }

    pub fn plain() -> Self {
        Self {
            plain: true,
            ..Self::new(String::new())
        }
    }
}

/// 巨集代換值（`{{user}}`、`{{char}}`）
#[derive(Debug, Clone, Serialize)]
pub struct Macros {
    pub user: String,
    pub char: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommandStatus {
    Applied,
    NoChange,
    Rejected,
    Ignored,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandResult {
    pub index: usize,
    pub op: String,
    pub path: Vec<String>,
    pub to: Option<Vec<String>>,
    pub status: CommandStatus,
    pub reason: Option<String>,
    pub issues: Option<Vec<String>>,
    pub issue_count: Option<usize>,
}

/// stat_data 逐葉變動。`before`／`after` 沒有＝該處原本／之後不存在；值是 null 時為 `Some(Value::Null)`。
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Change {
    pub path: Vec<String>,
    #[serde(default, deserialize_with = "present")]
    pub before: Option<Value>,
    #[serde(default, deserialize_with = "present")]
    pub after: Option<Value>,
    #[serde(default)]
    pub truncated: bool,
}

fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}

/// parseMessageDetailed 的回傳（規格 1.4 節）
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailedResult {
    pub table: Map<String, Value>,
    pub zod: bool,
    pub results: Vec<CommandResult>,
    pub changes: Vec<Change>,
    pub changes_truncated: bool,
}

/// 引擎一批的回傳：契約本體，加上與 results 一一對應的 GM 回饋句（共用核心 zod-feedback.ts 出的句子，None＝不回饋）
#[derive(Debug, Clone, Deserialize)]
pub struct BatchOutput {
    #[serde(flatten)]
    pub detailed: DetailedResult,
    pub feedback: Vec<Option<String>>,
}

/// 開局（initvar 先 cleanUpMetadata 再過 schema）。驗不過時 `stat_data` 是清掉元資料後的原值。
#[derive(Debug, Clone)]
pub struct InitOutput {
    pub stat_data: Value,
    /// 驗不過時的錯誤文字（每則已限字數）與總數；`None`＝通過
    pub invalid: Option<(Vec<String>, usize)>,
    /// 卡片腳本裡重複呼叫 registerMvuSchema 的次數（只認第一次；建種子時告知玩家一次）
    pub duplicates: u32,
    /// 種子：開場白的 `<initvar>` 有沒有套上（`initialize` 一律 false）
    pub applied: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureKind {
    /// 輸入超過 PARSE_LIMITS，沒進引擎
    InputTooLarge,
    /// 卡片 schema 腳本載入失敗（拋錯、觸限、import 白名單外的模組、沒呼叫 registerMvuSchema、展開 schema 拋錯）：
    /// 不呼叫核心。載入途中逾時歸 `Deadline`
    SchemaLoad,
    /// 整次期限到了（中斷處理器觸發，或外層逾時作廢執行緒）
    Deadline,
    Memory,
    Stack,
    /// 核心或卡片腳本拋錯；`code` 是核心 parseFailure 的代碼（too-many-commands、eval-failed、too-large…）
    Script {
        code: Option<String>,
    },
    /// 作廢執行緒或重開次數到上限，暫不受理
    Busy,
    /// 執行緒 panic 或意外結束
    Crashed,
    /// 引擎回傳不符契約或超出上限
    BadOutput,
}

/// 整次失敗：狀態整批不提交（規格 1.4 節）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineFailure {
    pub kind: FailureKind,
    pub detail: String,
}

impl EngineFailure {
    pub fn new(kind: FailureKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for EngineFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

/// 種子的底：卡上的 `[initvar]` 原文，或（卡上沒有 initvar 時）狀態樹物化的 stat_data
#[derive(Clone, Copy)]
pub enum SeedBase<'a> {
    Initvars(&'a [String]),
    Tree(&'a Value),
}
