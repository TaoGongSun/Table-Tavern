//! CLI lane resume 續聊（Claude/Grok 走各自 session，Agy 走精確 `--conversation`）。
//! 每桌按「線種:實際模型」分線（2026-08-03 拍板）：chars:<model>（解析到同一個模型的角色
//! 共用一條，快取按模型分池、跨模型本來就不共用）＋gm:<model>（GM 獨立——GM 的凍結 system
//! 多了 world.md／私設／GM 條目，依可見性憲法不能和角色同線）。
//! Agy 的 chars 線再按角色細分（chars:<model>:<角色 id>）：它的 session 沒有回合後抹寫路徑，
//! 私設改提進該角色自己的凍結 system，一角一線才不會洩漏給別的角色。Grok 與 claude 一樣共線，
//! 回合後抹 session 目錄的兩個檔（grok_session）。
//! 同桌的 lane 呼叫以每桌一把 mutex 串行：lanes.json 整份讀寫、session 檔回合後改寫，
//! 交錯就會互蓋狀態或把抹掉的機密段寫回。
//! 凍結 system 每輪逐字重帶、只送新事件與回合尾段。claude 2.1.287 實測：每個 request 只有最後一則
//! 訊息的斷點留下可重用的快取，回合後抹掉最新 user 行的機密段就作廢它（下一輪只中 system 段）；
//! 開線後第 2 輪另有一次 CLI 端的不中。所以 claude 單角色線不抹（`TurnInput::single_owner`），
//! 見 plans/claude-resume-tail-cache.md。
//! 正典 transcript 與 session 歷史靠水位＋指紋＋回覆對點對齊；任何對不上、任何改寫或呼叫失敗，
//! 一律丟線重開全量重建（降級鏈永遠可用，聊天不中斷）。
//! chars 線的私設隔離靠「回合注入機密段→回合後從 session 檔抹掉」維持（案 C，2026-08-03 拍板）；
//! claude 單角色線改成不抹，隔離靠「含未抹內容的 session 只給同一角色、單角色模式續用」
//! （`LaneState::unerased_owner`，〔作者裁決 2026-10-07〕）。

mod grok_session;
mod rewrite_failure;
mod session_file;
mod snapshot_patch;

use rewrite_failure::at;
pub(crate) use rewrite_failure::RewriteFailure;

use crate::cli;
use crate::data::{self, TranscriptEvent, TranscriptKind};
use crate::transport;
use crate::ui_msg::UiMsg;
use crate::usage::log as usage_log;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lane {
    Chars,
    Gm,
}

/// 走 lane 續聊的 CLI（風險告知、偵測、模型、env 都在 lib.rs 準備好）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LaneProvider {
    Claude,
    Agy,
    Grok,
}

impl LaneProvider {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Agy => "agy",
            Self::Grok => "grok",
        }
    }
}

/// CLI 呼叫素材（風險告知、偵測、模型、env 都在 lib.rs 準備好）。
pub(crate) struct LaneCall {
    pub provider: LaneProvider,
    pub program: PathBuf,
    pub working_dir: PathBuf,
    /// 提示詞暫存檔資料夾（cli::PromptFile）：system／正文不塞命令列參數
    pub prompt_dir: PathBuf,
    pub envs: Vec<(String, String)>,
    /// None＝不覆寫、用 CLI 自己的預設模型（Agy/Grok 可能這樣）
    pub model: Option<String>,
    pub usage_log: Option<PathBuf>,
    /// session 檔所在的 claude 設定目錄（~/.claude 或 $CLAUDE_CONFIG_DIR）；grok 不用
    pub claude_home: PathBuf,
    /// 這一輪有嘗試落在 Claude 訂閱超額時呼叫一次（帶那次嘗試自己的寫入觀測）。
    /// 在那次嘗試結束當下呼叫，整輪後來失敗也照報——錢已經花了。
    pub on_overage: Option<OverageSink>,
}

pub(crate) type OverageSink = std::sync::Arc<dyn Fn(CacheWriteObserved) + Send + Sync>;

/// 一次 claude 嘗試實際寫了哪種快取（超額提示據此決定能不能講倍數）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CacheWriteObserved {
    /// 只寫 1 小時快取
    OneHour,
    /// 純 5 分鐘、混合、缺拆分或沒有寫入
    Other,
    /// 沒拿到用量（中止或失敗在 result 之前）
    Unknown,
}

impl CacheWriteObserved {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::OneHour => "one-hour",
            Self::Other => "other",
            Self::Unknown => "unknown",
        }
    }

    fn of(usage: Option<&transport::PromptCacheUsage>) -> Self {
        let Some(usage) = usage else {
            return Self::Unknown;
        };
        let created = usage.created_tokens.unwrap_or(0);
        match usage.created_1h_tokens {
            Some(one_hour) if created > 0 && one_hour == created => Self::OneHour,
            _ => Self::Other,
        }
    }
}

/// 續聊線把 claude 的 prompt 快取釘成 1 小時〔作者裁決 2026-10-07〕。
/// 只給角色線與 GM 線用；卡重構與單發呼叫維持 CLI 預設。FORCE_PROMPT_CACHING_5M=0
/// 壓掉啟動 app 的環境殘留（實測空字串或 0 都讓它失效），玩家設定檔裡的那份壓不住。
pub(crate) fn pin_lane_cache_ttl(call: &mut LaneCall) {
    if call.provider == LaneProvider::Claude {
        call.envs
            .push(("CLAUDE_CODE_PROMPT_CACHE_TTL".to_owned(), "1h".to_owned()));
        call.envs
            .push(("FORCE_PROMPT_CACHING_5M".to_owned(), "0".to_owned()));
    }
}

impl LaneCall {
    /// 線名與用量 log 用的模型字樣；未覆寫時與單發路徑同字（"(CLI 預設)"）
    fn model_label(&self) -> &str {
        self.model.as_deref().unwrap_or("(CLI 預設)")
    }
}

/// 回覆會以什麼形狀落回正典 transcript（下一輪靠它跳過 session 裡已有的自家回覆）。
pub(crate) enum ReplyEcho {
    /// 角色台詞：事件原文＝剝掉本輪 `speaker_prefix`（`prefix`）後的回覆，見 transport::strip_own_prefix。
    /// 前綴另傳、不從 `TurnInput.prefix` 反推：Agy 那欄是 None，但模型照樣會自加前綴
    Dialogue { speaker_id: String, prefix: String },
    /// GM 旁白：事件原文＝剝掉狀態欄與「下一位」點名行後的顯示文字
    Narration,
}

pub(crate) struct TurnInput<'a> {
    pub lane: Lane,
    pub scene: u64,
    pub events: &'a [TranscriptEvent],
    /// 介面語系：事件標頭照提示詞慣例組字（en 出英文、其餘繁中），玩家空名退回該語系稱呼
    pub lang: &'a str,
    /// 本輪重組的最新素材全文；與已傳達版本（applied）不同時，快取存活走補丁、過期走追平
    pub frozen_system: String,
    /// 回合尾段（transport::chars_lane_turn／gm_lane_turn 的 tail）
    pub tail: String,
    /// tail 內回合後要抹掉的機密子段（chars 線私設＋限定條目）
    pub confidential: Option<String>,
    /// 回合後補在最後一則 assistant 前的名字前綴（chars 線「X：」）
    pub prefix: Option<String>,
    pub echo: ReplyEcho,
    /// 線名後綴：Agy 的 chars 線帶角色 id（一角一線），其餘 None
    pub scope: Option<String>,
    /// claude 角色線、單角色模式時＝開口者 id：本輪不抹，session 記成這個角色的未抹線。
    /// 其他供應商與 GM 線即使填了也會在 run_turn 歸零（只有 claude 角色線靠這欄隔離）。
    pub single_owner: Option<String>,
    /// 本輪 tail 含角色狀態區塊；單角色線上一輪有、這輪沒有就重開（舊值撤不掉）
    pub has_state_block: bool,
    /// 不抹尾段的角色線（Agy 一角一線、claude 單角色）提進凍結 system 的世界書段（`LaneTurn::hoisted_worldbook`）。
    /// 跟上一輪不同就整線重開、不走補丁：補丁回合後不抹，舊世界書會留在 session 歷史裡（方案三之 3）
    pub hoisted_worldbook: Option<String>,
}

/// 角色線的三個開關＋線名是否分角色（plans/claude-resume-tail-cache.md 三-B-2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CharsLaneShape {
    /// 私設與本角色限定可見的 constant 條目搬進凍結 system
    pub hoist_private: bool,
    /// 傳 confidential、回合後從 session 檔抹掉
    pub erase: bool,
    /// 回合後替最後一則 assistant 補名字前綴
    pub prefix: bool,
    /// 線名帶角色 id（一角一線）
    pub scope_by_card: bool,
    /// 這條線記成開口者的未抹線（claude 單角色模式）
    pub single_owner: bool,
}

/// `sole_present`：有效在場集合只有開口者本人（`chat_assembly::sole_present_character`）。
pub(crate) fn chars_lane_shape(provider: LaneProvider, sole_present: bool) -> CharsLaneShape {
    match provider {
        // Agy 沒有回合後抹寫路徑：一角一線＋私設進該角色自己的 system
        LaneProvider::Agy => CharsLaneShape {
            hoist_private: true,
            erase: false,
            prefix: false,
            scope_by_card: true,
            single_owner: false,
        },
        LaneProvider::Claude if sole_present => CharsLaneShape {
            hoist_private: true,
            erase: false,
            prefix: true,
            scope_by_card: false,
            single_owner: true,
        },
        // grok 不套單角色模式：system 凍在開線那刻〔作者裁決 2026-10-07〕
        LaneProvider::Claude | LaneProvider::Grok => CharsLaneShape {
            hoist_private: false,
            erase: true,
            prefix: true,
            scope_by_card: false,
            single_owner: false,
        },
    }
}

/// 線名（key）→ 線狀態。key＝「線種:實際模型」，模型看解析後真正傳給 CLI 的字串，
/// 不看檔位：高中檔都覆寫成 sonnet 就同一條 chars:sonnet。
type LaneStore = std::collections::BTreeMap<String, LaneState>;

/// 換幕容量用：這條線上一輪實報的總輸入（claude 回報的是整段 context）。
/// 線種、模型、CLI、幕都對得上才採用，對不上（換模、換幕、別的 CLI 開的線）一律 None。
pub(crate) fn last_prompt_tokens(
    root: &Path,
    world_id: &str,
    lane: Lane,
    provider: LaneProvider,
    model: &str,
    scope: Option<&str>,
    scene: u64,
) -> Option<u64> {
    let path = data::lanes_path(root, world_id).ok()?;
    let store = read_store(&path);
    let state = store.get(&lane_key(lane, model, scope))?;
    (state.provider == provider.as_str()
        && state.model == model
        && state.scene == scene
        && state.last_prompt_tokens > 0)
        .then_some(state.last_prompt_tokens)
}

/// `scope`：同一線種要再細分時的後綴（agy 的 chars 線帶角色 id）。None＝不細分。
fn lane_key(lane: Lane, model: &str, scope: Option<&str>) -> String {
    let kind = match lane {
        Lane::Chars => "chars",
        Lane::Gm => "gm",
    };
    match scope {
        Some(scope) => format!("{kind}:{model}:{scope}"),
        None => format!("{kind}:{model}"),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LaneState {
    session_id: String,
    scene: u64,
    /// 水位：呼叫當下已反映進 session 的正典事件數（不含 pending 的回覆事件）
    sent_events: usize,
    /// 已反映事件的指紋，偵測外部改動（改字、收回）
    sent_hash: String,
    /// 本輪實際傳給 CLI 的 system 全文；快取存活時維持舊字串，避免一字變動就失效
    snapshot: String,
    /// 最新已傳達的素材全文（快照加上歷來補丁）；下輪補丁只傳尚未傳達的差異
    applied: String,
    /// 呼叫前先寫、抹寫完成後清空——中途崩潰時下一輪看到未清的 pending 就整線重開，
    /// 機密段不會留在 session 歷史裡被下一個角色看到
    pending_rewrite: Option<PendingRewrite>,
    /// 上輪回覆應以此形狀出現在水位位置（前端呼叫返回後才落 transcript）
    expected_reply: Option<ExpectedReply>,
    /// 追平判斷用（距上輪超過 cache_ttl_secs＝快取已死，改寫快照零成本）；呼叫開始的時刻
    last_call_epoch: u64,
    /// 這條線快取的估計壽命（秒），由上次成功呼叫實際寫入的快取時效推得（見 next_cache_ttl）。
    /// 舊檔沒這欄位當 5 分鐘：當時還沒釘 1 小時。
    #[serde(default = "legacy_cache_ttl")]
    cache_ttl_secs: u64,
    /// 上次成功呼叫的總輸入＝下輪的理論可中量（診斷用；舊檔沒這欄位當 0，不觸發重開）
    #[serde(default)]
    last_prompt_tokens: u64,
    /// Agy result 的 conversation 累積計數；續聊時相減才是這一輪實際用量。
    #[serde(default)]
    agy_usage: Option<cli::AgyUsageCounters>,
    /// 開這條線的 CLI（"claude"／"grok"）。換 CLI 就整線重開——session id 認的是自己那套。
    /// grok lane 之前只有 claude 會開線，舊檔缺這欄位一律當 claude，不必白白重建一次快取。
    #[serde(default = "legacy_provider")]
    provider: String,
    /// 開這條線用的模型。線名會被 scope 撐開，模型不能再從線名回推
    #[serde(default)]
    model: String,
    /// 這個 session 含哪個角色未抹的機密（claude 單角色線）。有值時只給同一角色、單角色模式續用。
    /// 舊檔缺欄＝None＝照舊每輪抹的線，不猜成單角色。
    #[serde(default)]
    unerased_owner: Option<String>,
    /// 上一輪 tail 有角色狀態區塊（只在 unerased_owner 有值時有意義）
    #[serde(default)]
    had_state_block: bool,
    /// 上一輪提進凍結 system 的世界書段指紋（`TurnInput::hoisted_worldbook`）；舊檔沒這欄＝沒有
    #[serde(default)]
    hoisted_worldbook: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingRewrite {
    confidential: Option<String>,
    prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExpectedReply {
    speaker_id: String,
    kind: TranscriptKind,
    text: String,
}

fn read_store(path: &Path) -> LaneStore {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write_store(path: &Path, store: &LaneStore) -> Result<(), String> {
    let text = serde_json::to_string_pretty(store).map_err(|error| error.to_string())?;
    crate::data::commit_world_write(path, text.as_bytes()).map_err(|error| {
        UiMsg::LaneStateWriteFailed {
            path: path.display().to_string(),
            error: error.to_string(),
        }
        .to_string()
    })
}

/// 每桌一把 lane 鎖（以 lanes.json 路徑為鍵）。run_turn 從讀 store 到最終落檔全程持有。
fn lane_lock(store_path: &Path) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    static LOCKS: std::sync::OnceLock<
        std::sync::Mutex<
            std::collections::HashMap<PathBuf, std::sync::Arc<tokio::sync::Mutex<()>>>,
        >,
    > = std::sync::OnceLock::new();
    let mut table = LOCKS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    table.entry(store_path.to_path_buf()).or_default().clone()
}

/// 丟線落帳（沒有 stage／detail 的那種）。`transport` 記實際 provider。
fn log_drop(call: &LaneCall, world_id: &str, key: &str, reason: &str) {
    if let Some(path) = call.usage_log.as_deref() {
        usage_log::append_event(
            path,
            call.provider.as_str(),
            Some(world_id),
            key,
            usage_log::Event::DropLane,
            reason,
        );
    }
}

/// 丟線落帳，多記壞在哪一步與原錯誤字串（先遮掉 `paths` 裡的路徑再截斷）。
fn log_drop_failure(
    call: &LaneCall,
    world_id: &str,
    key: &str,
    reason: &str,
    failure: &RewriteFailure,
    paths: &[(String, String)],
) {
    if let Some(log) = call.usage_log.as_deref() {
        let detail = rewrite_failure::mask_detail(&failure.detail, paths);
        usage_log::append_drop(
            log,
            call.provider.as_str(),
            Some(world_id),
            key,
            reason,
            failure.stage,
            &detail,
        );
    }
}

/// grok 的 session 根目錄＝呼叫 CLI 時給的 GROK_HOME（cli::grok_envs），兩邊一定指同一處。
fn grok_home(call: &LaneCall) -> Result<PathBuf, String> {
    call.envs
        .iter()
        .find(|(key, _)| key == "GROK_HOME")
        .map(|(_, value)| PathBuf::from(value))
        .ok_or_else(|| "grok lane 缺 GROK_HOME".to_owned())
}

/// grok 錯誤字串裡可能出現的路徑：session 目錄（群組名是編碼過的 cwd）、GROK_HOME、工作目錄。
/// 要在刪目錄之前算。
fn grok_known_paths(call: &LaneCall, session_id: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    if let Ok(home) = grok_home(call) {
        for dir in grok_session::find_session_dirs(&home, session_id).unwrap_or_default() {
            pairs.push((dir.display().to_string(), "<session>".to_owned()));
        }
        pairs.push((home.display().to_string(), "<dir>".to_owned()));
    }
    pairs.push((call.working_dir.display().to_string(), "<dir>".to_owned()));
    pairs
}

/// grok 角色線回合後抹寫。stage：locate（找不到 GROK_HOME）／rewrite／compacted／lock-timeout。
fn rewrite_grok(
    call: &LaneCall,
    session_id: &str,
    confidential: Option<&str>,
    prefix: &str,
    reply: &str,
) -> Result<(), RewriteFailure> {
    let home = at("locate", grok_home(call))?;
    grok_session::rewrite(&home, session_id, confidential, prefix, reply).map_err(|error| {
        RewriteFailure {
            stage: error.reason.stage(),
            detail: error.detail,
        }
    })
}

/// 撤銷 grok 線的緣由。預定重開（換幕、改卡等）不是出事，不記丟線——呼叫行本來就帶 reopen。
enum GrokRevoke {
    Reopen,
    Drop(&'static str),
    Failed(RewriteFailure),
}

/// 撤銷一條 grok 線：先從 store 拿掉並落檔，再刪 session 目錄。落檔失敗就回錯——磁碟上的
/// 舊狀態要嘛帶著 pending_rewrite、要嘛已被別的路徑重開，都不會再續用這個 id。
/// 刪目錄失敗只記帳：store 已沒有這個 id，不會再續聊。
fn revoke_grok_lane(
    call: &LaneCall,
    world_id: &str,
    key: &str,
    session_id: &str,
    store: &mut LaneStore,
    store_path: &Path,
    why: GrokRevoke,
) -> Result<(), String> {
    let paths = grok_known_paths(call, session_id);
    match &why {
        GrokRevoke::Reopen => {}
        GrokRevoke::Drop(reason) => log_drop(call, world_id, key, reason),
        GrokRevoke::Failed(failure) => {
            log_drop_failure(call, world_id, key, "rewrite-failed", failure, &paths)
        }
    }
    store.remove(key);
    let written = write_store(store_path, store);
    let removed = grok_home(call).and_then(|home| grok_session::remove_session(&home, session_id));
    if let Err(detail) = removed {
        let failure = RewriteFailure {
            stage: "cleanup",
            detail,
        };
        log_drop_failure(call, world_id, key, "cleanup-failed", &failure, &paths);
    }
    written
}

/// FNV-1a 64：跨執行、跨版本皆穩定的事件指紋（std 的雜湊器不保證跨版本一致）。
fn events_fingerprint(events: &[TranscriptEvent]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    };
    for event in events {
        let kind = match event.kind {
            TranscriptKind::Dialogue => "dialogue",
            TranscriptKind::Narration => "narration",
            TranscriptKind::Player => "player",
            TranscriptKind::System => "system",
        };
        let marker = event
            .marker
            .as_ref()
            .and_then(|marker| serde_json::to_string(marker).ok())
            .unwrap_or_default();
        let gm_only = if event.gm_only { "1" } else { "0" };
        for field in [
            kind,
            &event.speaker_id,
            &event.speaker_name,
            &event.text,
            &marker,
            gm_only,
        ] {
            eat(field.as_bytes());
            eat(&[0x1f]);
        }
        eat(&[0x1e]);
    }
    format!("{hash:016x}")
}

/// 產生 claude CLI 接受的 UUID v4 字串。亂數取自兩顆 ulid（時間戳頭 6 bytes 換成
/// 第二顆的隨機尾），不為此多引一個 uuid/rand 依賴。
/// pub(crate)：refactor_session（重構兩段判官）開線也用同一套 id。
pub(crate) fn new_session_id() -> String {
    let mut bytes = u128::from(ulid::Ulid::generate()).to_be_bytes();
    let filler = u128::from(ulid::Ulid::generate()).to_be_bytes();
    bytes[..6].copy_from_slice(&filler[10..]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant 10
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

enum TurnPlan {
    Resume {
        session_id: String,
        base: usize,
        /// 本輪實際傳給 CLI 的 system（claude 的 --system-prompt-file 內容）。
        system: String,
        patch: Option<String>,
        /// 追平只供用量 log 區分；不改變續聊流程。
        rebased: bool,
    },
    Reopen {
        reason: ReopenReason,
    },
}

#[derive(Clone, Copy)]
enum ReopenReason {
    FirstTurn,
    PendingRewrite,
    SceneChanged,
    HistoryRewound,
    HistoryEdited,
    ReplyDiverged,
    ResumeFailed,
    ProviderChanged,
    SystemChanged,
    /// 未抹線換了開口者（含別的角色解析到同一模型而共用同一 key）
    OwnerChanged,
    /// 未抹線遇到多角色模式（在場變多）
    ModeChanged,
    /// 未抹線的狀態區塊整塊消失（歷史裡的舊值撤不掉）
    StateBlockGone,
    /// 不抹尾段的線提進 system 的世界書變了（補丁會留在歷史裡，只能重開）
    WorldbookChanged,
}

impl ReopenReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::FirstTurn => "first-turn",
            Self::PendingRewrite => "pending-rewrite",
            Self::SceneChanged => "scene-changed",
            Self::HistoryRewound => "history-rewound",
            Self::HistoryEdited => "history-edited",
            Self::ReplyDiverged => "reply-diverged",
            Self::ResumeFailed => "resume-failed",
            Self::ProviderChanged => "provider-changed",
            Self::SystemChanged => "system-changed",
            Self::OwnerChanged => "owner-changed",
            Self::ModeChanged => "mode-changed",
            Self::StateBlockGone => "state-block-gone",
            Self::WorldbookChanged => "worldbook-changed",
        }
    }
}

fn legacy_provider() -> String {
    LaneProvider::Claude.as_str().to_owned()
}

/// 5 分鐘快取壽命：舊 lanes.json／舊帳本列沒記時效時的預設，也是 grok／agy 線的固定窗口。
pub(crate) const LEGACY_CACHE_TTL_SECS: u64 = 300;
const ONE_HOUR_CACHE_TTL_SECS: u64 = 3600;

fn legacy_cache_ttl() -> u64 {
    LEGACY_CACHE_TTL_SECS
}

/// 成功一輪後這條 claude 線的估計快取壽命〔模型判斷·未裁決，見 plans/claude-1h-cache.md〕。
/// 只有確定整段都寫成 1 小時才給 3600；混合或時效不明一律保守取 5 分鐘；
/// 純命中沿用上輪（同 session 才算）；讀寫皆 0 或沒拿到用量＝當作已過期，不延壽。
fn next_cache_ttl(
    usage: Option<&transport::PromptCacheUsage>,
    prior_ttl: Option<u64>,
    resumed: bool,
) -> u64 {
    let Some(usage) = usage else {
        return 0;
    };
    let created = usage.created_tokens.unwrap_or(0);
    let cached = usage.cached_tokens.unwrap_or(0);
    if created == 0 {
        return match (cached > 0, resumed, prior_ttl) {
            (false, _, _) => 0,
            (true, true, Some(prior)) => prior,
            (true, _, _) => LEGACY_CACHE_TTL_SECS,
        };
    }
    match usage.created_1h_tokens {
        Some(one_hour) if one_hour == created => ONE_HOUR_CACHE_TTL_SECS,
        _ => LEGACY_CACHE_TTL_SECS,
    }
}

/// 決定這一輪續聊還是重開。所有「對不上」都走 Reopen：重開永遠正確，只是少省一次快取。
/// 素材漂移的處置分兩家：claude 的 system 每輪隨旗標重帶，補丁補得動、快取死了還能整份追平；
/// grok 的 system 凍在 session 建立那刻，補丁只是一則 user 訊息、壓不過權重更高的舊 system，
/// 所以 grok 一有漂移就整線重開（改卡、改世界書都是低頻動作）。
fn plan_turn(
    state: Option<&LaneState>,
    input: &TurnInput<'_>,
    now_epoch: u64,
    provider: LaneProvider,
) -> TurnPlan {
    let Some(state) = state else {
        return TurnPlan::Reopen {
            reason: ReopenReason::FirstTurn,
        };
    };
    if state.provider != provider.as_str() {
        return TurnPlan::Reopen {
            reason: ReopenReason::ProviderChanged,
        }; // 線名撞到別家 CLI 開的線：session id 是對方的，resume 一定失敗
    }
    if state.pending_rewrite.is_some() {
        return TurnPlan::Reopen {
            reason: ReopenReason::PendingRewrite,
        }; // 上一輪中途斷掉，session 內容不可信（可能殘留機密段）
    }
    if state.scene != input.scene {
        return TurnPlan::Reopen {
            reason: ReopenReason::SceneChanged,
        }; // 換場＝重開（拍板行為）
    }
    let mut base = state.sent_events;
    if base > input.events.len() {
        return TurnPlan::Reopen {
            reason: ReopenReason::HistoryRewound,
        }; // 正典被收回到水位之前
    }
    if events_fingerprint(&input.events[..base]) != state.sent_hash {
        return TurnPlan::Reopen {
            reason: ReopenReason::HistoryEdited,
        }; // 已送段被改動
    }
    if let Some(expected) = &state.expected_reply {
        match input.events.get(base) {
            Some(event)
                if event.speaker_id == expected.speaker_id
                    && event.kind == expected.kind
                    && event.text == expected.text
                    && event.marker.is_none()
                    && !event.gm_only =>
            {
                base += 1; // 上輪回覆已在 session 裡（assistant），跳過不重送
            }
            _ => {
                return TurnPlan::Reopen {
                    reason: ReopenReason::ReplyDiverged,
                }
            } // 回覆沒落檔或被改＝session 與正典分岔
        }
    }
    // 未抹線（claude 單角色）：順序固定，StateBlockGone 不能被「同一人續用」短路
    if let Some(owner) = &state.unerased_owner {
        let speaker = match &input.echo {
            ReplyEcho::Dialogue { speaker_id, .. } => Some(speaker_id.as_str()),
            ReplyEcho::Narration => None,
        };
        if speaker != Some(owner.as_str()) {
            return TurnPlan::Reopen {
                reason: ReopenReason::OwnerChanged,
            };
        }
        if input.single_owner.as_deref() != Some(owner.as_str()) {
            return TurnPlan::Reopen {
                reason: ReopenReason::ModeChanged,
            };
        }
        if state.had_state_block && !input.has_state_block {
            return TurnPlan::Reopen {
                reason: ReopenReason::StateBlockGone,
            };
        }
    }
    if state.hoisted_worldbook != worldbook_fingerprint(input.hoisted_worldbook.as_deref()) {
        return TurnPlan::Reopen {
            reason: ReopenReason::WorldbookChanged,
        };
    }
    if provider != LaneProvider::Claude {
        if state.applied != input.frozen_system {
            return TurnPlan::Reopen {
                reason: ReopenReason::SystemChanged,
            };
        }
        return TurnPlan::Resume {
            session_id: state.session_id.clone(),
            base,
            system: state.snapshot.clone(),
            patch: None,
            rebased: false,
        };
    }
    let age = now_epoch.saturating_sub(state.last_call_epoch);
    if age > state.cache_ttl_secs {
        return TurnPlan::Resume {
            session_id: state.session_id.clone(),
            base,
            system: input.frozen_system.clone(),
            patch: None,
            rebased: state.snapshot != input.frozen_system,
        };
    }
    TurnPlan::Resume {
        session_id: state.session_id.clone(),
        base,
        system: state.snapshot.clone(),
        patch: snapshot_patch::render_patch(&state.applied, &input.frozen_system, input.lang),
        rebased: false,
    }
}

/// 提進 system 的世界書段的指紋（存進 lanes.json，不存全文）；沒有或空的算沒有。
fn worldbook_fingerprint(worldbook: Option<&str>) -> Option<String> {
    let worldbook = worldbook.filter(|text| !text.is_empty())?;
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in worldbook.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    Some(format!("{hash:016x}"))
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// 組本輪 prompt：水位之後的新事件＋回合尾段。開線（全量重建）帶對話紀錄標頭，
/// 形狀比照單發 flatten；續聊只送增量，與 session 內既有歷史逐字銜接。
/// `lane`：chars 線走角色側渲染（略過角色私設事件、遮 gm_only），
/// GM 線一律全文。略過只發生在渲染，水位與指紋仍以原事件序列計。
pub(crate) fn build_prompt(
    events: &[TranscriptEvent],
    base: usize,
    tail: &str,
    opening: bool,
    lane: Lane,
    lang: &str,
) -> String {
    let side = match lane {
        Lane::Chars => transport::Side::Character,
        Lane::Gm => transport::Side::Gm,
    };
    let lines: Vec<String> = events[base..]
        .iter()
        .filter_map(|event| transport::lane_event_line(event, lang, side))
        .collect();
    if lines.is_empty() {
        return tail.to_owned();
    }
    let header = if opening {
        transport::history_header(lang)
    } else {
        ""
    };
    format!("{header}{}\n\n——\n{tail}", lines.join("\n\n"))
}

/// 回合後抹寫：機密段從注入的 user 行抹掉、最後一則 assistant 補名字前綴，
/// 原子寫＋回讀驗證（session_file::write_atomic）。
/// 回傳 Some(檔內有沒有 CLI 的 total_tokens 提醒)；沒東西要改、根本沒讀檔時回 None。
fn apply_rewrite(
    call: &LaneCall,
    session_id: &str,
    confidential: Option<&str>,
    prefix: Option<&str>,
) -> Result<Option<bool>, RewriteFailure> {
    if confidential.is_none() && prefix.is_none() {
        return Ok(None);
    }
    let path = session_file::session_file_path(&call.claude_home, &call.working_dir, session_id);
    let mut file = at("load", session_file::load(&path))?;
    let reminder = session_file::has_total_tokens_reminder(&file);
    if let Some(segment) = confidential {
        let uuid = at(
            "find-segment",
            session_file::find_user_line_with_segment(&file, segment),
        )?;
        at(
            "erase-segment",
            session_file::erase_user_segment(&mut file, &uuid, segment),
        )?;
    }
    if let Some(prefix) = prefix {
        at(
            "prefix-assistant",
            session_file::prefix_last_assistant(&mut file, prefix),
        )?;
    }
    at("write", session_file::write_atomic(&path, &file))?;
    Ok(Some(reminder))
}

/// 沒有抹寫的 claude 線（GM）另做只讀掃描；讀不到只回錯、由呼叫端記診斷，不丟線。
fn scan_reminder_readonly(call: &LaneCall, session_id: &str) -> Result<bool, String> {
    let path = session_file::session_file_path(&call.claude_home, &call.working_dir, session_id);
    session_file::read_text(&path).map(|text| session_file::text_has_total_tokens_reminder(&text))
}

/// D 偵測的診斷：只記帳不改線。同一 session、同一種結果在一次 app 執行內只記一次。
fn note_reminder(
    call: &LaneCall,
    world_id: &str,
    key: &str,
    session_id: &str,
    scan: Result<bool, String>,
) {
    let reason = match scan {
        Ok(false) => return,
        Ok(true) => "seen",
        Err(_) => "scan-failed",
    };
    static NOTED: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::OnceLock::new();
    let first = NOTED
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(format!("{session_id}\u{1f}{reason}"));
    if !first {
        return;
    }
    if let Some(path) = call.usage_log.as_deref() {
        usage_log::append_event(
            path,
            call.provider.as_str(),
            Some(world_id),
            key,
            usage_log::Event::CliReminderSeen,
            reason,
        );
    }
}

/// 抹寫／截尾失敗落帳：哪一步壞、原錯誤字串（遮路徑、截斷）。測試包另在丟線與刪檔之前
/// 把 session 檔與要抹的片段留證。只記錄，不改丟線策略。
#[allow(clippy::too_many_arguments)]
fn record_rewrite_failure(
    call: &LaneCall,
    world_id: &str,
    key: &str,
    reason: &str,
    session_id: &str,
    failure: &RewriteFailure,
    confidential: Option<&str>,
    prefix: Option<&str>,
) {
    let path = session_file::session_file_path(&call.claude_home, &call.working_dir, session_id);
    #[cfg(feature = "test-harness")]
    crate::harness::rewrite_evidence(key, reason, failure, &path, confidential, prefix);
    #[cfg(not(feature = "test-harness"))]
    let _ = (confidential, prefix);
    let paths = rewrite_failure::known_paths(&path, &call.claude_home, &call.working_dir);
    log_drop_failure(call, world_id, key, reason, failure, &paths);
}

/// 抹寫失敗：session 內容不可信，刪掉檔。檔案本來就不在（NotFound）當已刪。
/// 其他刪除錯誤交回呼叫端，不能當成這條線已經棄用成功。
fn abandon_session(call: &LaneCall, session_id: &str) -> Result<(), String> {
    if call.provider != LaneProvider::Claude {
        return Ok(());
    }
    let path = session_file::session_file_path(&call.claude_home, &call.working_dir, session_id);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(UiMsg::SessionAbandonFailed {
            path: path.display().to_string(),
            error: error.to_string(),
        }
        .into()),
    }
}

/// 中止回合的收尾。抹寫成功就什麼都不改——呼叫前已經寫下 pending_rewrite、expected_reply 仍是 None。
fn settle_abort(
    call: &LaneCall,
    world_id: &str,
    key: &str,
    session_id: &str,
    confidential: Option<&str>,
    prefix: Option<&str>,
    store: &mut LaneStore,
    store_path: &Path,
) -> Result<(), String> {
    // grok 中止時檔案可能停在半截（有 user、沒回覆），不去抹，整條撤銷
    if call.provider == LaneProvider::Grok {
        return revoke_grok_lane(
            call,
            world_id,
            key,
            session_id,
            store,
            store_path,
            GrokRevoke::Drop("aborted"),
        );
    }
    let rewrite = match call.provider {
        LaneProvider::Claude => apply_rewrite(call, session_id, confidential, prefix).map(|_| ()),
        LaneProvider::Agy | LaneProvider::Grok => Ok(()),
    };
    if let Err(failure) = rewrite {
        record_rewrite_failure(
            call,
            world_id,
            key,
            "rewrite-failed",
            session_id,
            &failure,
            confidential,
            prefix,
        );
        // 刪檔失敗也要先清線再回錯：呼叫端看到 Err 就不會把這一輪報成中止成功。
        let abandon = abandon_session(call, session_id);
        store.remove(key);
        write_store(store_path, store)?;
        if let Err(error) = abandon {
            let path =
                session_file::session_file_path(&call.claude_home, &call.working_dir, session_id);
            let paths = rewrite_failure::known_paths(&path, &call.claude_home, &call.working_dir);
            let failure = RewriteFailure {
                stage: "cleanup",
                detail: error.clone(),
            };
            log_drop_failure(call, world_id, key, "cleanup-failed", &failure, &paths);
            return Err(error);
        }
    }
    Ok(())
}

/// 一輪 lane 的收場。`aborted` 時 `text` 是已經吐出的半截，呼叫端不再把它當完整回覆寫狀態。
#[derive(Debug)]
pub(crate) struct TurnOutcome {
    pub text: String,
    pub aborted: bool,
}

fn expected_reply_for(echo: &ReplyEcho, reply: &str) -> ExpectedReply {
    match echo {
        ReplyEcho::Dialogue { speaker_id, prefix } => ExpectedReply {
            speaker_id: speaker_id.clone(),
            kind: TranscriptKind::Dialogue,
            text: transport::strip_own_prefix(reply, prefix).to_owned(),
        },
        // 前端落 transcript 的是剝掉狀態欄與「下一位」點名行的顯示文字（gm_narrate 的行為）
        ReplyEcho::Narration => ExpectedReply {
            speaker_id: String::new(),
            kind: TranscriptKind::Narration,
            text: transport::extract_next_speaker(&transport::extract_state_block(reply).display).1,
        },
    }
}

/// 跑一輪 lane 呼叫：計畫（續聊或重開）→ 呼叫前落狀態 → CLI → 回合後抹寫 → 落最終狀態。
/// 續聊呼叫失敗自動降級為重開全量再試一次；重開也失敗才把錯誤丟回（與現行單發同表現）。
/// `cancel` 在這層收，不把整個 future 丟給外層 select：中止時先等子程序退出，再照案 C 抹私設。
pub(crate) async fn run_turn(
    call: &LaneCall,
    root: &Path,
    world_id: &str,
    mut input: TurnInput<'_>,
    mut cancel: Option<&mut crate::inflight::CancelSignal>,
    mut emit: impl FnMut(&str),
) -> Result<TurnOutcome, String> {
    // Agy 沒有 session 檔抹寫路徑：機密段送進去就永久留在該線歷史裡。呼叫端必須把私設
    // 提進該角色自己的凍結 system（hoist_private）＋一角一線；這裡出聲擋下，寧可整輪失敗
    // 也不讓別的角色讀到不該讀的東西。
    if call.provider == LaneProvider::Agy
        && (input.confidential.is_some() || input.prefix.is_some())
    {
        return Err(UiMsg::LaneRewriteUnsupported {
            provider: call.provider.as_str().to_owned(),
        }
        .into());
    }
    // 只有 claude 角色線靠 unerased_owner 隔離；agy 靠分角色線名、grok 與 GM 每輪抹或不含機密，
    // 一律歸零，plan_turn 的未抹線判定就套不到它們
    if !(call.provider == LaneProvider::Claude && input.lane == Lane::Chars) {
        input.single_owner = None;
        input.has_state_block = false;
    }
    let store_path = data::lanes_path(root, world_id).map_err(|error| error.to_string())?;
    let key = lane_key(input.lane, call.model_label(), input.scope.as_deref());
    let lock = lane_lock(&store_path);
    let _lane_guard = lock.lock().await;
    let mut store = read_store(&store_path);
    let call_epoch = now_epoch();
    let prior = store.get(&key);
    let prior_agy_usage = prior.and_then(|state| state.agy_usage);
    // 診斷用（包 4）：距上輪幾秒、上輪送了多少（＝這輪的理論可中量）
    let age_secs = prior.map_or(0, |state| call_epoch.saturating_sub(state.last_call_epoch));
    let expected_cached = prior.map_or(0, |state| state.last_prompt_tokens);
    // 呼叫前採用的快取壽命：過期判斷與帳本診斷都用它，不拿這輪新寫入的時效回頭解釋
    let prior_ttl = prior.map(|state| state.cache_ttl_secs);
    let mut plan = plan_turn(prior, &input, call_epoch, call.provider);
    // grok 重開前先撤銷舊線：崩潰留下的 pending 可能還帶著沒抹的機密段，舊 id 一律不再用
    if let (TurnPlan::Reopen { .. }, Some(prior)) = (&plan, prior) {
        if call.provider == LaneProvider::Grok && prior.provider == LaneProvider::Grok.as_str() {
            let old = prior.session_id.clone();
            revoke_grok_lane(
                call,
                world_id,
                &key,
                &old,
                &mut store,
                &store_path,
                GrokRevoke::Reopen,
            )?;
        }
    }
    let prompt_tokens = std::sync::atomic::AtomicU64::new(0);
    let mut overage_reported = false;
    // 每一輪重試共用同一個接收端。watch 留著最新值，停止若在降級重開前就到了，下一輪 CLI 一進迴圈就看得到。
    let cancel_rx = cancel.as_mut().map(|signal| signal.receiver());

    loop {
        let (session_id, base, opening, system, patch, lane_log) = match &plan {
            TurnPlan::Resume {
                session_id,
                base,
                system,
                patch,
                rebased,
            } => {
                let lane_log = usage_log::LaneContext {
                    lane: key.clone(),
                    reopen: None,
                    patched: patch.is_some(),
                    rebased: *rebased,
                    age_secs,
                    expected_cached,
                    system_tokens: usage_log::estimate_tokens(system),
                    system_hash: usage_log::text_hash(system),
                    ttl_secs: prior_ttl,
                };
                (
                    session_id.clone(),
                    *base,
                    false,
                    system.clone(),
                    patch.clone(),
                    lane_log,
                )
            }
            TurnPlan::Reopen { reason } => {
                let system = input.frozen_system.clone();
                let lane_log = usage_log::LaneContext {
                    lane: key.clone(),
                    reopen: Some(reason.as_str()),
                    patched: false,
                    rebased: false,
                    age_secs,
                    expected_cached: 0, // 重開＝從零建快取
                    system_tokens: usage_log::estimate_tokens(&system),
                    system_hash: usage_log::text_hash(&system),
                    ttl_secs: None,
                };
                (new_session_id(), 0, true, system, None, lane_log)
            }
        };
        let tail = patch
            .as_ref()
            .map(|patch| format!("{patch}\n\n{}", input.tail))
            .unwrap_or_else(|| input.tail.clone());
        let prompt = build_prompt(input.events, base, &tail, opening, input.lane, input.lang);
        // 換幕容量的估計校正（計畫 §3.5）：只有重開全量那次「送出的就是整段」，估計與實報才配得上；
        // agy 量的是 bytes，不需要 token 校正
        let calibration_estimate = (opening && call.provider != LaneProvider::Agy).then(|| {
            crate::scene_budget::budget_tokens(&system)
                + crate::scene_budget::budget_tokens(&prompt)
        });
        let session = if opening {
            cli::CliSession::Open(&session_id)
        } else {
            cli::CliSession::Resume(&session_id)
        };
        // system／正文一律不進命令列（Windows 整條命令列只有 32,767 個 UTF-16 單位）：
        // Claude 的 system 寫檔、正文走 stdin；Grok 開線的 system 寫成 agent profile、正文寫檔；
        // Agy 整包走 stdin（開線帶 system、續聊只送本輪）。暫存檔活到 run_cli 收屍返回後才刪。
        let mut prompt_files = Vec::new();
        let (args, stdin) = match call.provider {
            LaneProvider::Claude => {
                let system_file = cli::PromptFile::create(&call.prompt_dir, "system.txt", &system)?;
                let args =
                    cli::claude_session_args(call.model_label(), system_file.path(), &session);
                prompt_files.push(system_file);
                (args, prompt.clone())
            }
            LaneProvider::Grok => {
                let profile = match (&session, system.is_empty()) {
                    (cli::CliSession::Open(_), false) => Some(cli::PromptFile::create(
                        &call.prompt_dir,
                        "grok-system.md",
                        &cli::grok_payload(&system, "", false).0.unwrap_or_default(),
                    )?),
                    _ => None,
                };
                let body = cli::PromptFile::create(&call.prompt_dir, "prompt.txt", &prompt)?;
                let args = cli::grok_session_args(
                    call.model.as_deref(),
                    profile.as_ref().map(cli::PromptFile::path),
                    body.path(),
                    &session,
                );
                prompt_files.extend(profile);
                prompt_files.push(body);
                (args, String::new())
            }
            LaneProvider::Agy => {
                let conversation = (!opening).then_some(session_id.as_str());
                (
                    cli::agy_session_args(call.model.as_deref(), conversation),
                    cli::agy_session_body(&system, &prompt, conversation),
                )
            }
        };

        store.insert(
            key.clone(),
            LaneState {
                session_id: session_id.clone(),
                scene: input.scene,
                sent_events: input.events.len(),
                sent_hash: events_fingerprint(input.events),
                snapshot: system,
                applied: input.frozen_system.clone(),
                provider: call.provider.as_str().to_owned(),
                model: call.model_label().to_owned(),
                pending_rewrite: Some(PendingRewrite {
                    confidential: input.confidential.clone(),
                    prefix: input.prefix.clone(),
                }),
                expected_reply: None,
                last_call_epoch: call_epoch,
                // 這次嘗試成功才知道寫了哪種快取；中途失敗、中止的線下一輪本來就重開
                cache_ttl_secs: 0,
                last_prompt_tokens: 0,
                agy_usage: None,
                // 呼叫前就落檔：中途崩潰時下一輪靠 pending 重開，靠這欄擋別人續用
                unerased_owner: input.single_owner.clone(),
                had_state_block: input.single_owner.is_some() && input.has_state_block,
                hoisted_worldbook: worldbook_fingerprint(input.hoisted_worldbook.as_deref()),
            },
        );
        write_store(&store_path, &store)?;

        let conversation_id = std::sync::Mutex::new(None);
        let agy_usage = std::sync::Mutex::new(None);
        let identity = std::sync::Mutex::new(None::<String>);
        // 每次嘗試各自觀測：降級重開不沿用失敗那次的用量與超額旗標
        let attempt_usage = std::sync::Mutex::new(None);
        let attempt_overage = std::sync::atomic::AtomicBool::new(false);
        // 本輪角色自加的「名字：」不進畫面；每次 attempt 新建，續聊失敗重開不帶上一試的扣留
        let mut stream = transport::OwnPrefixStream::new(match &input.echo {
            ReplyEcho::Dialogue { prefix, .. } => prefix,
            ReplyEcho::Narration => "",
        });
        let mut filtered = |delta: &str| {
            let out = stream.push(delta);
            if !out.is_empty() {
                emit(&out);
            }
        };
        let result = cli::run_cli_cancellable(
            &call.program,
            &call.working_dir,
            &args,
            &stdin,
            &call.envs,
            match call.provider {
                LaneProvider::Claude => cli::parse_claude_line,
                LaneProvider::Agy => cli::parse_agy_line,
                LaneProvider::Grok => cli::parse_grok_line,
            },
            false, // 聊天正文串流，思考不進畫面
            // lane 只跑劇情輪（角色、GM 旁白、GM 建議）：字數上限＋退化偵測
            transport::RunawayPolicy::Full,
            call.usage_log.as_deref().map(|path| cli::UsageLog {
                usage_out: Some(&attempt_usage),
                overage_out: (call.provider == LaneProvider::Claude).then_some(&attempt_overage),
                path,
                world: Some(world_id),
                transport: call.provider.as_str(),
                model: call.model_label(),
                parse: match call.provider {
                    LaneProvider::Claude => cli::parse_claude_usage,
                    LaneProvider::Agy => cli::parse_agy_usage,
                    LaneProvider::Grok => cli::parse_grok_usage,
                },
                lane: Some(lane_log),
                shape: usage_log::PromptShape::Oneshot, // 續聊線的形狀由 LaneContext 說明，這欄不參與判定
                prompt_tokens_out: Some(&prompt_tokens),
                conversation_id_out: (call.provider == LaneProvider::Agy)
                    .then_some(&conversation_id),
                expected_conversation_id: (call.provider == LaneProvider::Agy && !opening)
                    .then_some(session_id.as_str()),
                agy_usage_base: (call.provider == LaneProvider::Agy && !opening)
                    .then_some(prior_agy_usage)
                    .flatten(),
                agy_usage_out: (call.provider == LaneProvider::Agy).then_some(&agy_usage),
                identity_out: Some(&identity),
            }),
            &mut filtered,
            cancel_rx.clone(),
        )
        .await;
        let attempt_usage = attempt_usage.into_inner().ok().flatten();
        if !overage_reported && attempt_overage.load(std::sync::atomic::Ordering::Relaxed) {
            overage_reported = true;
            if let Some(sink) = &call.on_overage {
                sink(CacheWriteObserved::of(attempt_usage.as_ref()));
            }
        }
        if matches!(
            result,
            Ok(cli::CliFinish::Completed(_) | cli::CliFinish::Aborted(_))
        ) {
            let held = stream.finish();
            if !held.is_empty() {
                emit(&held);
            }
        }

        match result {
            // 中止不是續聊失敗：不降級重試。私設照抹；抹不掉就棄用這條 session。
            // expected_reply 維持呼叫前寫下的 None，pending_rewrite 也留著，下一輪改由正典重建。
            Ok(cli::CliFinish::Aborted(partial)) => {
                settle_abort(
                    call,
                    world_id,
                    &key,
                    &session_id,
                    input.confidential.as_deref(),
                    input.prefix.as_deref(),
                    &mut store,
                    &store_path,
                )?;
                return Ok(TurnOutcome {
                    text: partial,
                    aborted: true,
                });
            }
            // 輸出失控：收尾比照玩家按停止（grok 撤線；claude 抹私設、claude／agy 留
            // pending_rewrite 下一輪重開），但回錯而不是半截——不重開重試（A5）、半截不交出（A4）。
            // settle_abort 自己失敗就回那個錯，同樣不重派送。
            Ok(cli::CliFinish::Runaway { reason, chars }) => {
                settle_abort(
                    call,
                    world_id,
                    &key,
                    &session_id,
                    input.confidential.as_deref(),
                    input.prefix.as_deref(),
                    &mut store,
                    &store_path,
                )?;
                return Err(transport::runaway_message(reason, chars));
            }
            Ok(cli::CliFinish::Completed(reply)) => {
                let actual_session_id = match call.provider {
                    LaneProvider::Agy => conversation_id
                        .into_inner()
                        .ok()
                        .flatten()
                        .filter(|id| !id.is_empty()),
                    _ => Some(session_id.clone()),
                };
                // Agy 若沒有回 conversation ID，本輪回覆仍可用，但不能把假 ID 留給下輪。
                if actual_session_id.is_none() {
                    store.remove(&key);
                    write_store(&store_path, &store)?;
                    return Ok(TurnOutcome {
                        text: reply,
                        aborted: false,
                    });
                }
                let actual_session_id = actual_session_id.expect("checked above");
                let actual_agy_usage = agy_usage.into_inner().ok().flatten();
                let mut reminder: Option<bool> = None;
                let rewrite = match call.provider {
                    LaneProvider::Claude => apply_rewrite(
                        call,
                        &actual_session_id,
                        input.confidential.as_deref(),
                        input.prefix.as_deref(),
                    )
                    .map(|loaded| reminder = loaded),
                    // GM 線一律原文，不抹；角色共線每輪都抹（至少要拿 reasoning、補前綴）
                    LaneProvider::Grok if input.lane == Lane::Chars => rewrite_grok(
                        call,
                        &actual_session_id,
                        input.confidential.as_deref(),
                        input.prefix.as_deref().unwrap_or_default(),
                        &reply,
                    ),
                    LaneProvider::Agy | LaneProvider::Grok => Ok(()),
                };
                match rewrite {
                    Ok(()) => {
                        // 偵測只看已載入的內容或另做只讀掃描；抹寫本身的失敗走下面的丟線
                        if call.provider == LaneProvider::Claude {
                            let scan = match reminder {
                                Some(seen) => Ok(seen),
                                None => scan_reminder_readonly(call, &actual_session_id),
                            };
                            note_reminder(call, world_id, &key, &actual_session_id, scan);
                        }
                        if let Some(state) = store.get_mut(&key) {
                            state.session_id = actual_session_id;
                            state.pending_rewrite = None;
                            state.expected_reply = Some(expected_reply_for(&input.echo, &reply));
                            state.last_prompt_tokens =
                                prompt_tokens.load(std::sync::atomic::Ordering::Relaxed);
                            // claude 要這次回報辨識得出實際模型才記校正（身分不明就不記）
                            let confirmed = identity.lock().ok().and_then(|id| id.clone());
                            let identified =
                                call.provider != LaneProvider::Claude || confirmed.is_some();
                            if let (Some(estimate), true) = (calibration_estimate, identified) {
                                crate::scene_budget::record_calibration(
                                    root,
                                    call.provider.as_str(),
                                    call.model_label(),
                                    match input.lane {
                                        Lane::Gm => "gm",
                                        Lane::Chars => "chars",
                                    },
                                    estimate,
                                    state.last_prompt_tokens,
                                    confirmed.as_deref(),
                                );
                            }
                            state.cache_ttl_secs = match call.provider {
                                LaneProvider::Claude => {
                                    next_cache_ttl(attempt_usage.as_ref(), prior_ttl, !opening)
                                }
                                // grok／agy 不回報快取時效，維持原本的五分鐘窗口
                                LaneProvider::Agy | LaneProvider::Grok => LEGACY_CACHE_TTL_SECS,
                            };
                            if call.provider == LaneProvider::Agy {
                                state.agy_usage = actual_agy_usage;
                            }
                        }
                    }
                    // 抹寫失敗＝session 內容不可信，丟線；下一輪自動重開全量，本輪回覆照常送回
                    Err(failure) if call.provider == LaneProvider::Grok => {
                        revoke_grok_lane(
                            call,
                            world_id,
                            &key,
                            &actual_session_id,
                            &mut store,
                            &store_path,
                            GrokRevoke::Failed(failure),
                        )?;
                        return Ok(TurnOutcome {
                            text: reply,
                            aborted: false,
                        });
                    }
                    Err(failure) => {
                        record_rewrite_failure(
                            call,
                            world_id,
                            &key,
                            "rewrite-failed",
                            &actual_session_id,
                            &failure,
                            input.confidential.as_deref(),
                            input.prefix.as_deref(),
                        );
                        store.remove(&key);
                    }
                }
                write_store(&store_path, &store)?;
                return Ok(TurnOutcome {
                    text: reply,
                    aborted: false,
                });
            }
            // 續聊失敗（session 檔認不得、CLI 拒絕 resume 等）＝丟線重開全量再試一次
            Err(_) if !opening => {
                if call.provider == LaneProvider::Grok {
                    revoke_grok_lane(
                        call,
                        world_id,
                        &key,
                        &session_id,
                        &mut store,
                        &store_path,
                        GrokRevoke::Drop("resume-failed"),
                    )?;
                }
                plan = TurnPlan::Reopen {
                    reason: ReopenReason::ResumeFailed,
                };
            }
            Err(error) => {
                if call.provider == LaneProvider::Grok {
                    // 開線失敗前 CLI 可能已把帶機密段的 user 寫進新目錄
                    revoke_grok_lane(
                        call,
                        world_id,
                        &key,
                        &session_id,
                        &mut store,
                        &store_path,
                        GrokRevoke::Drop("open-failed"),
                    )?;
                } else {
                    store.remove(&key);
                    write_store(&store_path, &store)?;
                }
                return Err(error.to_string());
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::await_holding_lock)]
mod tests;

#[cfg(test)]
mod scaffold_tests;
