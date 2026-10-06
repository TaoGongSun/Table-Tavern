//! CLI lane resume 續聊（Claude/Grok 走各自 session，Agy 走精確 `--conversation`）。
//! 每桌按「線種:實際模型」分線（2026-08-03 拍板）：chars:<model>（解析到同一個模型的角色
//! 共用一條，快取按模型分池、跨模型本來就不共用）＋gm:<model>（GM 獨立——GM 的凍結 system
//! 多了 world.md／私設／GM 條目，依可見性憲法不能和角色同線）。
//! Agy 的 chars 線再按角色細分（chars:<model>:<角色 id>）：它的 session 沒有回合後抹寫路徑，
//! 私設改提進該角色自己的凍結 system，一角一線才不會洩漏給別的角色。Grok 與 claude 一樣共線，
//! 回合後抹 session 目錄的兩個檔（grok_session）。
//! 同桌的 lane 呼叫（含保溫）以每桌一把 mutex 串行：lanes.json 整份讀寫、session 檔回合後改寫，
//! 交錯就會互蓋狀態或把抹掉的機密段寫回。
//! 凍結 system 每輪逐字重帶、只送新事件與回合尾段，
//! 快取命中率的天花板因此變成「只有最後一句沒中」（實驗 E6：99.7%）。
//! 正典 transcript 與 session 歷史靠水位＋指紋＋回覆對點對齊；任何對不上、任何改寫或呼叫失敗，
//! 一律丟線重開全量重建（降級鏈永遠可用，聊天不中斷）。
//! chars 線的私設隔離靠「回合注入機密段→回合後從 session 檔抹掉」維持（案 C，2026-08-03 拍板）。

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
    fn as_str(self) -> &'static str {
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
}

impl LaneCall {
    /// 線名與用量 log 用的模型字樣；未覆寫時與單發路徑同字（"(CLI 預設)"）
    fn model_label(&self) -> &str {
        self.model.as_deref().unwrap_or("(CLI 預設)")
    }
}

/// 回覆會以什麼形狀落回正典 transcript（下一輪靠它跳過 session 裡已有的自家回覆）。
pub(crate) enum ReplyEcho {
    /// 角色台詞：事件原文＝回覆原文
    Dialogue { speaker_id: String },
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
}

/// 線名（key）→ 線狀態。key＝「線種:實際模型」，模型看解析後真正傳給 CLI 的字串，
/// 不看檔位：高中檔都覆寫成 sonnet 就同一條 chars:sonnet。
type LaneStore = std::collections::BTreeMap<String, LaneState>;

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
    /// 追平判斷用（距上輪超過五分鐘＝快取已死，改寫快照零成本）
    last_call_epoch: u64,
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

/// 每桌一把 lane 鎖（以 lanes.json 路徑為鍵）。run_turn 與 keepalive 從讀 store 到最終落檔全程持有。
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
        }
    }
}

fn legacy_provider() -> String {
    LaneProvider::Claude.as_str().to_owned()
}

pub(crate) const CACHE_TTL_SECS: u64 = 300;

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
    if age > CACHE_TTL_SECS {
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
fn apply_rewrite(
    call: &LaneCall,
    session_id: &str,
    confidential: Option<&str>,
    prefix: Option<&str>,
) -> Result<(), RewriteFailure> {
    if confidential.is_none() && prefix.is_none() {
        return Ok(());
    }
    let path = session_file::session_file_path(&call.claude_home, &call.working_dir, session_id);
    let mut file = at("load", session_file::load(&path))?;
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
    at("write", session_file::write_atomic(&path, &file))
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
        LaneProvider::Claude => apply_rewrite(call, session_id, confidential, prefix),
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
        ReplyEcho::Dialogue { speaker_id } => ExpectedReply {
            speaker_id: speaker_id.clone(),
            kind: TranscriptKind::Dialogue,
            text: reply.to_owned(),
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
    input: TurnInput<'_>,
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
                    ping: false,
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
                    ping: false,
                };
                (new_session_id(), 0, true, system, None, lane_log)
            }
        };
        let tail = patch
            .as_ref()
            .map(|patch| format!("{patch}\n\n{}", input.tail))
            .unwrap_or_else(|| input.tail.clone());
        let prompt = build_prompt(input.events, base, &tail, opening, input.lane, input.lang);
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
                last_prompt_tokens: 0,
                agy_usage: None,
            },
        );
        write_store(&store_path, &store)?;

        let conversation_id = std::sync::Mutex::new(None);
        let agy_usage = std::sync::Mutex::new(None);
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
            call.usage_log.as_deref().map(|path| cli::UsageLog {
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
            }),
            &mut emit,
            cancel_rx.clone(),
        )
        .await;

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
                let rewrite = match call.provider {
                    LaneProvider::Claude => apply_rewrite(
                        call,
                        &actual_session_id,
                        input.confidential.as_deref(),
                        input.prefix.as_deref(),
                    ),
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
                        if let Some(state) = store.get_mut(&key) {
                            state.session_id = actual_session_id;
                            state.pending_rewrite = None;
                            state.expected_reply = Some(expected_reply_for(&input.echo, &reply));
                            state.last_prompt_tokens =
                                prompt_tokens.load(std::sync::atomic::Ordering::Relaxed);
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

/// 保溫訊息本文：兼作截尾時的定位片段，所以要夠獨特、不可能出現在劇情裡。
const PING_PROMPT: &str = "（系統保溫訊息，不是劇情，也不要記進故事。請只回覆 ok。）";
/// 剛呼叫完的線不必保溫（前端節奏之外的第二道防呆）。
const PING_MIN_AGE_SECS: u64 = 180;

/// 保溫 ping（包 7）：對每條快取還活著的線送一則極短訊息，讀一次既有快取就能把
/// 五分鐘壽命重新計時，代價約為讓快取死掉重建的十二分之一。ping 前先記下 session 檔原文，
/// 事後整份還原——快取時鐘已被那次讀取刷新，截尾不改變已快取的前綴內容，
/// 下一輪照樣命中，劇情與正典 transcript 也完全不受影響。
/// 回傳實際保溫成功的線數。ping 失敗不當錯誤（保溫是省錢手段，不該中斷聊天），但同樣還原；
/// 還原失敗則丟線，避免垃圾問答在 session 裡越積越多。
pub(crate) async fn keepalive(
    call: &LaneCall,
    root: &Path,
    world_id: &str,
) -> Result<usize, String> {
    let store_path = data::lanes_path(root, world_id).map_err(|error| error.to_string())?;
    // 與 run_turn 同一把：保溫整份讀寫 lanes.json、事後還原 session 檔，交錯會把撤銷的線寫回
    let lock = lane_lock(&store_path);
    let _lane_guard = lock.lock().await;
    let mut store = read_store(&store_path);
    let now = now_epoch();
    // 先挑出該保溫的線再逐條呼叫：迴圈中要改 store，不能同時借著它疊代
    let targets: Vec<(String, LaneState)> = store
        .iter()
        // 保溫後要整份還原 session 檔，只對 claude 的格式做；grok 線一律不 ping
        .filter(|(_, state)| state.provider == LaneProvider::Claude.as_str())
        .filter(|(_, state)| state.pending_rewrite.is_none())
        .filter(|(_, state)| {
            let age = now.saturating_sub(state.last_call_epoch);
            (PING_MIN_AGE_SECS..=CACHE_TTL_SECS).contains(&age)
        })
        .map(|(key, state)| (key.clone(), state.clone()))
        .collect();
    if targets.is_empty() {
        return Ok(0);
    }

    let mut pinged = 0;
    for (key, state) in targets {
        // 保溫要用該線自己的模型才會匹配到它的快取（線名可能帶 scope，模型讀狀態不回推）
        if state.model.is_empty() {
            continue;
        }
        // 保溫也要帶同一份 system 才匹配得到快取前綴；system 寫檔，活到 run_cli 返回後才刪
        let system_file = cli::PromptFile::create(&call.prompt_dir, "system.txt", &state.snapshot)?;
        let args = cli::claude_session_args(
            &state.model,
            system_file.path(),
            &cli::CliSession::Resume(&state.session_id),
        );
        // 保溫前的原文：事後整份還原（CLI 只會追加）。讀不到就不保溫這條線
        let session_path = session_file::session_file_path(
            &call.claude_home,
            &call.working_dir,
            &state.session_id,
        );
        let Ok(before) = session_file::read_text(&session_path) else {
            continue;
        };
        let lane_log = usage_log::LaneContext {
            lane: key.clone(),
            reopen: None,
            patched: false,
            rebased: false,
            age_secs: now.saturating_sub(state.last_call_epoch),
            expected_cached: state.last_prompt_tokens,
            system_tokens: usage_log::estimate_tokens(&state.snapshot),
            system_hash: usage_log::text_hash(&state.snapshot),
            ping: true,
        };
        let result = cli::run_cli(
            &call.program,
            &call.working_dir,
            &args,
            PING_PROMPT,
            &call.envs,
            cli::parse_claude_line,
            false, // 聊天正文串流，思考不進畫面
            call.usage_log.as_deref().map(|path| cli::UsageLog {
                path,
                world: Some(world_id),
                transport: "claude",
                model: &state.model,
                parse: cli::parse_claude_usage,
                lane: Some(lane_log),
                shape: usage_log::PromptShape::Oneshot, // 同上：ping 的 mode 來自 LaneContext.ping
                prompt_tokens_out: None,
                conversation_id_out: None,
                expected_conversation_id: None,
                agy_usage_base: None,
                agy_usage_out: None,
            }),
            &mut |_: &str| {},
        )
        .await;
        // ping 失敗也要還原：CLI 可能已寫下半截問答
        match restore_before_ping(&session_path, &before, result.is_ok()) {
            Ok(()) if result.is_ok() => {
                if let Some(state) = store.get_mut(&key) {
                    state.last_call_epoch = now_epoch();
                }
                pinged += 1;
            }
            Ok(()) => {}
            Err(failure) => {
                record_rewrite_failure(
                    call,
                    world_id,
                    &key,
                    "ping-truncate-failed",
                    &state.session_id,
                    &failure,
                    Some(PING_PROMPT),
                    None,
                );
                store.remove(&key);
            }
        }
    }
    write_store(&store_path, &store)?;
    Ok(pinged)
}

/// 把 session 檔還原成保溫前的原文：保溫後的檔必須是「原文＋CLI 追加段」，追加段的 user 行
/// 只能是保溫訊息（ping 成功時恰好一則）。CLI 在 ping user 之前追加的 queue-operation／attachment
/// 也一併消失，檔案逐位元組回到保溫前。
fn restore_before_ping(path: &Path, before: &str, pinged: bool) -> Result<(), RewriteFailure> {
    let after = at("load", session_file::read_text(path))?;
    let appended = at("truncate", session_file::appended_since(before, &after))?;
    let pings = at(
        "find-segment",
        session_file::marker_user_lines(appended, PING_PROMPT),
    )?;
    if pinged && pings != 1 {
        return Err(RewriteFailure {
            stage: "find-segment",
            detail: format!("保溫後追加段有 {pings} 則保溫訊息，應恰好一則"),
        });
    }
    if appended.is_empty() {
        return Ok(());
    }
    at("write", session_file::write_text_atomic(path, before))
}

#[cfg(test)]
#[allow(clippy::await_holding_lock)]
mod tests;

#[cfg(test)]
mod scaffold_tests;
