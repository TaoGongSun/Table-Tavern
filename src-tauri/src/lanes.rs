//! claude lane resume 續聊（prompt-cache-optimization 包 2）。
//! 每桌按「線種:實際模型」分線（2026-08-03 拍板）：chars:<model>（解析到同一個模型的角色
//! 共用一條，快取按模型分池、跨模型本來就不共用）＋gm:<model>（GM 獨立——GM 的凍結 system
//! 多了 world.md／私設／GM 條目，依可見性憲法不能和角色同線）。
//! 凍結 system 每輪逐字重帶、只送新事件與回合尾段，
//! 快取命中率的天花板因此變成「只有最後一句沒中」（實驗 E6：99.7%）。
//! 正典 transcript 與 session 歷史靠水位＋指紋＋回覆對點對齊；任何對不上、任何改寫或呼叫失敗，
//! 一律丟線重開全量重建（降級鏈永遠可用，聊天不中斷）。
//! chars 線的私設隔離靠「回合注入機密段→回合後從 session 檔抹掉」維持（案 C，2026-08-03 拍板）。

use crate::cli;
use crate::data::{self, TranscriptEvent, TranscriptKind};
use crate::session_file;
use crate::snapshot_patch;
use crate::transport;
use crate::usage_log;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lane {
    Chars,
    Gm,
}

/// claude CLI 呼叫素材（風險告知、偵測、模型、env 都在 lib.rs 準備好）。
pub(crate) struct ClaudeCall {
    pub program: PathBuf,
    pub working_dir: PathBuf,
    pub envs: Vec<(String, String)>,
    pub model: String,
    pub usage_log: Option<PathBuf>,
    /// session 檔所在的 claude 設定目錄（~/.claude 或 $CLAUDE_CONFIG_DIR）
    pub claude_home: PathBuf,
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
    /// 本輪重組的最新素材全文；與已傳達版本（applied）不同時，快取存活走補丁、過期走追平
    pub frozen_system: String,
    /// 回合尾段（transport::chars_lane_turn／gm_lane_turn 的 tail）
    pub tail: String,
    /// tail 內回合後要抹掉的機密子段（chars 線私設＋限定條目）
    pub confidential: Option<String>,
    /// 回合後補在最後一則 assistant 前的名字前綴（chars 線「X：」）
    pub prefix: Option<String>,
    pub echo: ReplyEcho,
}

/// 線名（key）→ 線狀態。key＝「線種:實際模型」，模型看解析後真正傳給 CLI 的字串，
/// 不看檔位：高中檔都覆寫成 sonnet 就同一條 chars:sonnet。
type LaneStore = std::collections::BTreeMap<String, LaneState>;

fn lane_key(lane: Lane, model: &str) -> String {
    let kind = match lane {
        Lane::Chars => "chars",
        Lane::Gm => "gm",
    };
    format!("{kind}:{model}")
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
    std::fs::write(path, text)
        .map_err(|error| format!("無法寫入 lane 狀態檔 {}：{error}", path.display()))
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
        for field in [kind, &event.speaker_id, &event.speaker_name, &event.text] {
            eat(field.as_bytes());
            eat(&[0x1f]);
        }
        eat(&[0x1e]);
    }
    format!("{hash:016x}")
}

/// 產生 claude CLI 接受的 UUID v4 字串。亂數取自兩顆 ulid（時間戳頭 6 bytes 換成
/// 第二顆的隨機尾），不為此多引一個 uuid/rand 依賴。
fn new_session_id() -> String {
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
        /// 本輪實際傳給 CLI 的 --system-prompt。
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
        }
    }
}

pub(crate) const CACHE_TTL_SECS: u64 = 300;

/// 決定這一輪續聊還是重開。所有「對不上」都走 Reopen：重開永遠正確，只是少省一次快取。
fn plan_turn(state: Option<&LaneState>, input: &TurnInput<'_>, now_epoch: u64) -> TurnPlan {
    let Some(state) = state else {
        return TurnPlan::Reopen {
            reason: ReopenReason::FirstTurn,
        };
    };
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
                    && event.text == expected.text =>
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
        patch: snapshot_patch::render_patch(&state.applied, &input.frozen_system),
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
/// `lane`：chars 線的 gm_only System 事件降一行，GM 線一律全文（AI 卡重構包 4b）。
fn build_prompt(events: &[TranscriptEvent], base: usize, tail: &str, opening: bool, lane: Lane) -> String {
    let redact_gm_only = lane == Lane::Chars;
    let lines: Vec<String> = events[base..]
        .iter()
        .map(|event| transport::lane_event_line(event, redact_gm_only))
        .collect();
    if lines.is_empty() {
        return tail.to_owned();
    }
    let header = if opening {
        "以下是到目前為止的對話紀錄：\n\n"
    } else {
        ""
    };
    format!("{header}{}\n\n——\n{tail}", lines.join("\n\n"))
}

/// 回合後抹寫：機密段從注入的 user 行抹掉、最後一則 assistant 補名字前綴，
/// 原子寫＋回讀驗證（session_file::write_atomic）。
fn apply_rewrite(
    call: &ClaudeCall,
    session_id: &str,
    confidential: Option<&str>,
    prefix: Option<&str>,
) -> Result<(), String> {
    if confidential.is_none() && prefix.is_none() {
        return Ok(());
    }
    let path = session_file::session_file_path(&call.claude_home, &call.working_dir, session_id);
    let mut file = session_file::load(&path)?;
    if let Some(segment) = confidential {
        let uuid = session_file::find_user_line_with_segment(&file, segment)?;
        session_file::erase_user_segment(&mut file, &uuid, segment)?;
    }
    if let Some(prefix) = prefix {
        session_file::prefix_last_assistant(&mut file, prefix)?;
    }
    session_file::write_atomic(&path, &file)
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
pub(crate) async fn run_turn(
    call: &ClaudeCall,
    root: &Path,
    world_id: &str,
    input: TurnInput<'_>,
    mut emit: impl FnMut(&str),
) -> Result<String, String> {
    let store_path = data::lanes_path(root, world_id).map_err(|error| error.to_string())?;
    let key = lane_key(input.lane, &call.model);
    let mut store = read_store(&store_path);
    let call_epoch = now_epoch();
    let prior = store.get(&key);
    // 診斷用（包 4）：距上輪幾秒、上輪送了多少（＝這輪的理論可中量）
    let age_secs = prior.map_or(0, |state| call_epoch.saturating_sub(state.last_call_epoch));
    let expected_cached = prior.map_or(0, |state| state.last_prompt_tokens);
    let mut plan = plan_turn(prior, &input, call_epoch);
    let prompt_tokens = std::sync::atomic::AtomicU64::new(0);

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
        let prompt = build_prompt(input.events, base, &tail, opening, input.lane);
        let session = if opening {
            cli::ClaudeSession::Open(&session_id)
        } else {
            cli::ClaudeSession::Resume(&session_id)
        };
        let args = cli::claude_session_args(&call.model, &system, &session);

        store.insert(
            key.clone(),
            LaneState {
                session_id: session_id.clone(),
                scene: input.scene,
                sent_events: input.events.len(),
                sent_hash: events_fingerprint(input.events),
                snapshot: system,
                applied: input.frozen_system.clone(),
                pending_rewrite: Some(PendingRewrite {
                    confidential: input.confidential.clone(),
                    prefix: input.prefix.clone(),
                }),
                expected_reply: None,
                last_call_epoch: call_epoch,
                last_prompt_tokens: 0,
            },
        );
        write_store(&store_path, &store)?;

        let result = cli::run_cli(
            &call.program,
            &call.working_dir,
            &args,
            &prompt,
            &call.envs,
            cli::parse_claude_line,
            false, // 聊天正文串流，思考不進畫面
            call.usage_log.as_deref().map(|path| cli::UsageLog {
                path,
                world: Some(world_id),
                transport: "claude",
                model: &call.model,
                parse: cli::parse_claude_usage,
                lane: Some(lane_log),
                prompt_tokens_out: Some(&prompt_tokens),
            }),
            &mut emit,
        )
        .await;

        match result {
            Ok(reply) => {
                let rewrite = apply_rewrite(
                    call,
                    &session_id,
                    input.confidential.as_deref(),
                    input.prefix.as_deref(),
                );
                match rewrite {
                    Ok(()) => {
                        if let Some(state) = store.get_mut(&key) {
                            state.pending_rewrite = None;
                            state.expected_reply = Some(expected_reply_for(&input.echo, &reply));
                            state.last_prompt_tokens =
                                prompt_tokens.load(std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                    // 抹寫失敗＝session 內容不可信，丟線；下一輪自動重開全量，本輪回覆照常送回
                    Err(_) => {
                        if let Some(path) = call.usage_log.as_deref() {
                            usage_log::append_event(
                                path,
                                Some(world_id),
                                &key,
                                usage_log::Diag::DropLane,
                                "rewrite-failed",
                            );
                        }
                        store.remove(&key);
                    }
                }
                write_store(&store_path, &store)?;
                return Ok(reply);
            }
            // 續聊失敗（session 檔認不得、CLI 拒絕 resume 等）＝丟線重開全量再試一次
            Err(_) if !opening => {
                plan = TurnPlan::Reopen {
                    reason: ReopenReason::ResumeFailed,
                };
            }
            Err(error) => {
                store.remove(&key);
                write_store(&store_path, &store)?;
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
/// 五分鐘壽命重新計時，代價約為讓快取死掉重建的十二分之一。ping 的問答隨即從
/// session 檔截掉——快取時鐘已被那次讀取刷新，截尾不改變已快取的前綴內容，
/// 下一輪照樣命中，劇情與正典 transcript 也完全不受影響。
/// 回傳實際保溫成功的線數。ping 失敗不當錯誤（保溫是省錢手段，不該中斷聊天）；
/// 截尾失敗則丟線，避免垃圾問答在 session 裡越積越多。
pub(crate) async fn keepalive(
    call: &ClaudeCall,
    root: &Path,
    world_id: &str,
) -> Result<usize, String> {
    let store_path = data::lanes_path(root, world_id).map_err(|error| error.to_string())?;
    let mut store = read_store(&store_path);
    let now = now_epoch();
    // 先挑出該保溫的線再逐條呼叫：迴圈中要改 store，不能同時借著它疊代
    let targets: Vec<(String, LaneState)> = store
        .iter()
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
        // 線名＝「線種:實際模型」，保溫要用該線自己的模型才會匹配到它的快取
        let Some((_, model)) = key.split_once(':') else {
            continue;
        };
        let args = cli::claude_session_args(
            model,
            &state.snapshot,
            &cli::ClaudeSession::Resume(&state.session_id),
        );
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
                model,
                parse: cli::parse_claude_usage,
                lane: Some(lane_log),
                prompt_tokens_out: None,
            }),
            &mut |_: &str| {},
        )
        .await;
        if result.is_err() {
            continue;
        }
        match truncate_ping(call, &state.session_id) {
            Ok(()) => {
                if let Some(state) = store.get_mut(&key) {
                    state.last_call_epoch = now_epoch();
                }
                pinged += 1;
            }
            Err(_) => {
                if let Some(path) = call.usage_log.as_deref() {
                    usage_log::append_event(
                        path,
                        Some(world_id),
                        &key,
                        usage_log::Diag::DropLane,
                        "ping-truncate-failed",
                    );
                }
                store.remove(&key);
            }
        }
    }
    write_store(&store_path, &store)?;
    Ok(pinged)
}

/// 把保溫問答從 session 檔截掉：定位那則 ping user 行，截掉它與其後所有行
/// （模型的 ok 回覆一併消失），檔案回到 ping 前的形狀。
fn truncate_ping(call: &ClaudeCall, session_id: &str) -> Result<(), String> {
    let path = session_file::session_file_path(&call.claude_home, &call.working_dir, session_id);
    let mut file = session_file::load(&path)?;
    let uuid = session_file::find_user_line_with_segment(&file, PING_PROMPT)?;
    session_file::truncate_from(&mut file, &uuid)?;
    session_file::write_atomic(&path, &file)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeCli {
        dir: PathBuf,
        call: ClaudeCall,
        root: PathBuf,
        world_id: String,
        session_dir: PathBuf,
        claude_home: PathBuf,
        working_dir: PathBuf,
    }

    /// 假 claude CLI：照真檔格式寫 session JSONL、逐次把拿到的旗標與 prompt 記進 calls.jsonl，
    /// resume 找不到 session 檔就以非零碼結束（降級鏈用）。
    #[cfg(unix)]
    fn fake_claude(tag: &str) -> FakeCli {
        let dir = std::env::temp_dir().join(format!("tt-lanes-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let working_dir = dir.join("ws");
        let claude_home = dir.join("claude-home");
        let root = dir.join("root");
        let world_id = ulid::Ulid::generate().to_string();
        std::fs::create_dir_all(&working_dir).unwrap();
        std::fs::create_dir_all(root.join("worlds").join(&world_id)).unwrap();
        // 假 CLI 寫 session 檔的位置＝真實 munged 路徑，lanes 的抹寫才找得到
        let session_dir = session_file::session_file_path(&claude_home, &working_dir, "probe")
            .parent()
            .unwrap()
            .to_path_buf();
        std::fs::create_dir_all(&session_dir).unwrap();
        let script = dir.join("fake-claude.py");
        std::fs::write(
            &script,
            r#"#!/usr/bin/env python3
import json, sys, os, uuid
args = sys.argv[1:]
def flag(name):
    return args[args.index(name) + 1] if name in args else None
sid, rid = flag('--session-id'), flag('--resume')
prompt = sys.stdin.read()
d = os.environ['FAKE_SESSION_DIR']
with open(os.path.join(d, 'calls.jsonl'), 'a') as f:
    f.write(json.dumps({'args': args, 'prompt': prompt}) + '\n')
path = os.path.join(d, (sid or rid) + '.jsonl')
lines, last = [], None
if rid:
    if not os.path.exists(path):
        sys.exit(3)
    for l in open(path):
        o = json.loads(l)
        lines.append(o)
        if o.get('type') in ('user', 'assistant'):
            last = o['uuid']
u, a = str(uuid.uuid4()), str(uuid.uuid4())
lines.append({'type': 'user', 'uuid': u, 'parentUuid': last,
              'message': {'role': 'user', 'content': prompt}})
reply = '回覆' + str(sum(1 for o in lines if o.get('type') == 'user'))
lines.append({'type': 'assistant', 'uuid': a, 'parentUuid': u,
              'message': {'role': 'assistant', 'content': [{'type': 'text', 'text': reply}]}})
with open(path, 'w') as f:
    for o in lines:
        f.write(json.dumps(o, ensure_ascii=False) + '\n')
print(json.dumps({'type': 'result', 'is_error': False, 'result': reply}))
"#,
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let call = ClaudeCall {
            program: script,
            working_dir: working_dir.clone(),
            envs: vec![(
                "FAKE_SESSION_DIR".to_owned(),
                session_dir.to_string_lossy().into_owned(),
            )],
            model: "sonnet".to_owned(),
            usage_log: None,
            claude_home: claude_home.clone(),
        };
        FakeCli {
            dir,
            call,
            root,
            world_id,
            session_dir,
            claude_home,
            working_dir,
        }
    }

    fn event(kind: TranscriptKind, speaker_id: &str, name: &str, text: &str) -> TranscriptEvent {
        TranscriptEvent {
            raw: None,
            ts: "2026-08-03T21:00:00+08:00".to_owned(),
            speaker_id: speaker_id.to_owned(),
            speaker_name: name.to_owned(),
            kind,
            text: text.to_owned(),
            state: None,
            gm_only: false,
        }
    }

    fn turn_input<'a>(events: &'a [TranscriptEvent], scene: u64) -> TurnInput<'a> {
        TurnInput {
            lane: Lane::Chars,
            scene,
            events,
            frozen_system: "凍結A".to_owned(),
            tail: "現在你是「狐狸」。".to_owned(),
            confidential: None,
            prefix: Some("狐狸：".to_owned()),
            echo: ReplyEcho::Dialogue {
                speaker_id: "fox-id".to_owned(),
            },
        }
    }

    fn lane_state(events: &[TranscriptEvent], scene: u64) -> LaneState {
        LaneState {
            session_id: "sid-1".to_owned(),
            scene,
            sent_events: events.len(),
            sent_hash: events_fingerprint(events),
            snapshot: "凍結A".to_owned(),
            applied: "凍結A".to_owned(),
            pending_rewrite: None,
            expected_reply: None,
            last_call_epoch: 1_000,
            last_prompt_tokens: 0,
        }
    }

    #[test]
    fn fingerprint_changes_with_any_event_field() {
        let base = [event(TranscriptKind::Player, "", "阿濤", "你好")];
        let renamed = [event(TranscriptKind::Player, "", "阿桃", "你好")];
        let retyped = [event(TranscriptKind::Dialogue, "", "阿濤", "你好")];
        let edited = [event(TranscriptKind::Player, "", "阿濤", "你好嗎")];
        let original = events_fingerprint(&base);
        assert_ne!(original, events_fingerprint(&renamed));
        assert_ne!(original, events_fingerprint(&retyped));
        assert_ne!(original, events_fingerprint(&edited));
        assert_eq!(original, events_fingerprint(&base));
    }

    #[test]
    fn session_ids_are_distinct_valid_uuid_v4() {
        let first = new_session_id();
        let second = new_session_id();
        assert_ne!(first, second);
        for id in [&first, &second] {
            assert_eq!(id.len(), 36);
            let parts: Vec<&str> = id.split('-').collect();
            assert_eq!(
                parts.iter().map(|part| part.len()).collect::<Vec<_>>(),
                [8, 4, 4, 4, 12]
            );
            assert!(id.chars().all(|c| c == '-' || c.is_ascii_hexdigit()));
            assert!(parts[2].starts_with('4'));
            assert!("89ab".contains(&parts[3][..1]));
        }
    }

    /// 續聊的前提一項不合就重開：這是降級鏈的決策核心。
    #[test]
    fn plan_resumes_only_when_everything_lines_up() {
        let events = [
            event(TranscriptKind::Player, "", "阿濤", "你好"),
            event(TranscriptKind::Dialogue, "fox-id", "狐狸", "晚安"),
        ];
        let input = turn_input(&events, 0);

        assert!(matches!(
            plan_turn(None, &input, 1_010),
            TurnPlan::Reopen {
                reason: ReopenReason::FirstTurn
            }
        ));

        let good = lane_state(&events, 0);
        match plan_turn(Some(&good), &input, 1_010) {
            TurnPlan::Resume {
                session_id, base, ..
            } => {
                assert_eq!(session_id, "sid-1");
                assert_eq!(base, 2);
            }
            TurnPlan::Reopen { .. } => panic!("狀態齊備必須續聊"),
        }

        let mut pending = good.clone();
        pending.pending_rewrite = Some(PendingRewrite {
            confidential: None,
            prefix: None,
        });
        assert!(matches!(
            plan_turn(Some(&pending), &input, 1_010),
            TurnPlan::Reopen {
                reason: ReopenReason::PendingRewrite
            }
        ));

        let mut scene_changed = good.clone();
        scene_changed.scene = 1;
        assert!(matches!(
            plan_turn(Some(&scene_changed), &input, 1_010),
            TurnPlan::Reopen {
                reason: ReopenReason::SceneChanged
            }
        ));

        let mut changed_input = turn_input(&events, 0);
        changed_input.frozen_system = "凍結B".to_owned();
        match plan_turn(Some(&good), &changed_input, 1_010) {
            TurnPlan::Resume { system, patch, .. } => {
                assert_eq!(system, "凍結A");
                assert!(patch.is_some());
            }
            TurnPlan::Reopen { .. } => panic!("快取存活時素材變動必須走補丁"),
        }

        let mut ahead = good.clone();
        ahead.sent_events = 3; // 正典被收回到水位前
        assert!(matches!(
            plan_turn(Some(&ahead), &input, 1_010),
            TurnPlan::Reopen {
                reason: ReopenReason::HistoryRewound
            }
        ));

        // 已送段被改動（收回重寫第一句）
        let edited = [
            event(TranscriptKind::Player, "", "阿濤", "改過的第一句"),
            events[1].clone(),
        ];
        let edited_input = turn_input(&edited, 0);
        assert!(matches!(
            plan_turn(Some(&good), &edited_input, 1_010),
            TurnPlan::Reopen {
                reason: ReopenReason::HistoryEdited
            }
        ));
    }

    /// 上輪回覆（expected_reply）落檔後從水位跳過；沒落檔或被改動＝重開。
    #[test]
    fn plan_skips_own_reply_at_watermark_or_reopens() {
        let before = [event(TranscriptKind::Player, "", "阿濤", "你好")];
        let reply = event(TranscriptKind::Dialogue, "fox-id", "狐狸", "晚安");
        let mut state = lane_state(&before, 0);
        state.expected_reply = Some(ExpectedReply {
            speaker_id: "fox-id".to_owned(),
            kind: TranscriptKind::Dialogue,
            text: "晚安".to_owned(),
        });

        let with_reply = [before[0].clone(), reply.clone()];
        let input = turn_input(&with_reply, 0);
        match plan_turn(Some(&state), &input, 1_010) {
            TurnPlan::Resume { base, .. } => assert_eq!(base, 2),
            TurnPlan::Reopen { .. } => panic!("回覆已落檔必須續聊"),
        }

        // 回覆事件被玩家改字＝session 與正典分岔
        let tampered = [
            before[0].clone(),
            event(TranscriptKind::Dialogue, "fox-id", "狐狸", "被改過的晚安"),
        ];
        let tampered_input = turn_input(&tampered, 0);
        assert!(matches!(
            plan_turn(Some(&state), &tampered_input, 1_010),
            TurnPlan::Reopen {
                reason: ReopenReason::ReplyDiverged
            }
        ));

        // 回覆還沒落檔（前端沒寫進 transcript）
        let missing_input = turn_input(&before, 0);
        assert!(matches!(
            plan_turn(Some(&state), &missing_input, 1_010),
            TurnPlan::Reopen {
                reason: ReopenReason::ReplyDiverged
            }
        ));
    }

    #[test]
    fn plan_rebases_changed_material_after_cache_expires() {
        let events = [event(TranscriptKind::Player, "", "阿濤", "你好")];
        let mut input = turn_input(&events, 0);
        input.frozen_system = "凍結B".to_owned();
        let state = lane_state(&events, 0);

        match plan_turn(Some(&state), &input, 1_301) {
            TurnPlan::Resume {
                system,
                patch,
                rebased,
                ..
            } => {
                assert_eq!(system, "凍結B");
                assert!(patch.is_none());
                assert!(rebased);
            }
            TurnPlan::Reopen { .. } => panic!("快取過期時素材變動必須追平"),
        }
    }

    #[test]
    fn narration_echo_expects_display_text_without_state_fence() {
        let reply = "夜更深了。\n```state\ntime: 午夜\n```\n下一位：狐狸";
        let expected = expected_reply_for(&ReplyEcho::Narration, reply);
        assert_eq!(expected.kind, TranscriptKind::Narration);
        assert_eq!(expected.text, "夜更深了。");
    }

    #[test]
    fn prompt_carries_header_only_on_reopen_and_tail_alone_without_events() {
        let events = [
            event(TranscriptKind::Player, "", "阿濤", "你好"),
            event(TranscriptKind::Narration, "", "GM", "夜深了"),
        ];
        let full = build_prompt(&events, 0, "尾段", true, Lane::Chars);
        assert!(full.starts_with("以下是到目前為止的對話紀錄：\n\n阿濤：你好\n\n（旁白）夜深了"));
        assert!(full.ends_with("——\n尾段"));
        let increment = build_prompt(&events, 1, "尾段", false, Lane::Chars);
        assert_eq!(increment, "（旁白）夜深了\n\n——\n尾段");
        assert_eq!(build_prompt(&events, 2, "尾段", false, Lane::Chars), "尾段");
    }

    /// 端到端（假 CLI）：開線→抹寫→續聊只送增量→正典被改→自動重開；
    /// 續聊呼叫失敗（session 檔消失）→ 同一輪內降級重開。
    #[cfg(unix)]
    #[tokio::test]
    async fn lane_turns_open_rewrite_resume_and_degrade() {
        // run_cli 會把子程序 pid 登記進 inflight 的全域 children 表；kill_all_children 的
        // 測試（inflight.rs）不分青紅皂白殺表上全部 pid，故用同一把鎖互斥執行。
        let _serial = crate::inflight::lock_real_process_tests();
        let FakeCli {
            dir,
            call,
            root,
            world_id,
            session_dir,
            claude_home,
            working_dir,
        } = fake_claude("e2e");
        let calls = |index: usize| -> (Vec<String>, String) {
            let text = std::fs::read_to_string(session_dir.join("calls.jsonl")).unwrap();
            let line: serde_json::Value =
                serde_json::from_str(text.lines().nth(index).unwrap()).unwrap();
            (
                line["args"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_owned())
                    .collect(),
                line["prompt"].as_str().unwrap().to_owned(),
            )
        };

        // 第一輪：沒有 lane 狀態 → 開線全量，機密段注入
        let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "老闆晚安")];
        let confidential = "## 「狐狸」的私有設定\n其實是通緝犯\n".to_owned();
        let mut input = turn_input(&events, 0);
        input.tail = format!("{confidential}\n現在你是「狐狸」。");
        input.confidential = Some(confidential.clone());
        let reply1 = run_turn(&call, &root, &world_id, input, |_| {})
            .await
            .unwrap();
        assert_eq!(reply1, "回覆1");
        let (args1, prompt1) = calls(0);
        let open_flag = args1.iter().position(|a| a == "--session-id").unwrap();
        let first_session = args1[open_flag + 1].clone();
        assert!(prompt1.starts_with("以下是到目前為止的對話紀錄："));
        assert!(prompt1.contains("阿濤：老闆晚安"));
        assert!(prompt1.contains("通緝犯"));
        // 回合後抹寫：機密段消失、assistant 補了名字前綴
        let session_path =
            session_file::session_file_path(&claude_home, &working_dir, &first_session);
        let rewritten = std::fs::read_to_string(&session_path).unwrap();
        assert!(!rewritten.contains("通緝犯"));
        assert!(rewritten.contains("狐狸：回覆1"));

        // 第二輪：回覆已落正典＋玩家新句 → 續聊，只送新句
        events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", &reply1));
        events.push(event(TranscriptKind::Player, "", "阿濤", "來一杯麥酒"));
        let reply2 = run_turn(&call, &root, &world_id, turn_input(&events, 0), |_| {})
            .await
            .unwrap();
        assert_eq!(reply2, "回覆2");
        let (args2, prompt2) = calls(1);
        assert!(args2
            .windows(2)
            .any(|w| w == ["--resume", first_session.as_str()]));
        assert!(prompt2.contains("阿濤：來一杯麥酒"));
        assert!(!prompt2.contains("老闆晚安")); // 舊事件不重送
        assert!(!prompt2.contains("回覆1")); // 自家上輪回覆不重送

        // 第三輪：舊事件被改字 → 指紋不合，自動重開新線全量
        events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", &reply2));
        events[0].text = "被改過的第一句".to_owned();
        let reply3 = run_turn(&call, &root, &world_id, turn_input(&events, 0), |_| {})
            .await
            .unwrap();
        assert_eq!(reply3, "回覆1"); // 新 session 檔重新計數＝證明真的重開
        let (args3, prompt3) = calls(2);
        let reopen_flag = args3.iter().position(|a| a == "--session-id").unwrap();
        let second_session = args3[reopen_flag + 1].clone();
        assert_ne!(second_session, first_session);
        assert!(prompt3.contains("被改過的第一句"));

        // 第四輪：session 檔被外力刪掉 → 續聊失敗，同一輪內降級重開成功
        events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", &reply3));
        std::fs::remove_file(session_file::session_file_path(
            &claude_home,
            &working_dir,
            &second_session,
        ))
        .unwrap();
        let reply4 = run_turn(&call, &root, &world_id, turn_input(&events, 0), |_| {})
            .await
            .unwrap();
        assert_eq!(reply4, "回覆1");
        let (args4, _) = calls(3);
        assert!(args4.contains(&"--resume".to_owned())); // 先試續聊
        let (args5, prompt5) = calls(4);
        assert!(args5.contains(&"--session-id".to_owned())); // 降級重開
        assert!(prompt5.starts_with("以下是到目前為止的對話紀錄："));

        // 第五輪換模型（同桌 haiku 角色）：另開自己的線，sonnet 線不受影響（按模型分池）
        events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", &reply4));
        let haiku_call = ClaudeCall {
            model: "haiku".to_owned(),
            program: call.program.clone(),
            working_dir: call.working_dir.clone(),
            envs: call.envs.clone(),
            usage_log: None,
            claude_home: call.claude_home.clone(),
        };
        run_turn(
            &haiku_call,
            &root,
            &world_id,
            turn_input(&events, 0),
            |_| {},
        )
        .await
        .unwrap();
        let (args6, _) = calls(5);
        assert!(args6.contains(&"--session-id".to_owned())); // haiku 沒有既有線，全量開新
        let lanes_json =
            std::fs::read_to_string(data::lanes_path(&root, &world_id).unwrap()).unwrap();
        assert!(lanes_json.contains("chars:sonnet"));
        assert!(lanes_json.contains("chars:haiku"));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn set_lane_epoch(store_path: &Path, epoch: u64) {
        let mut store = read_store(store_path);
        for state in store.values_mut() {
            state.last_call_epoch = epoch;
        }
        write_store(store_path, &store).unwrap();
    }

    /// 端到端（假 CLI）：保溫 ping 讀一次既有快取後把問答截掉，session 檔逐字回到 ping 前，
    /// 下一輪照樣續聊只送增量（回覆編號沒被 ping 墊高＝真的截乾淨）；
    /// 剛呼叫完、快取已過期、上輪沒收尾的線都不浪費這筆錢。
    #[cfg(unix)]
    #[tokio::test]
    async fn keepalive_pings_live_lanes_and_leaves_no_trace() {
        // run_cli 會把子程序 pid 登記進 inflight 的全域 children 表；kill_all_children 的
        // 測試（inflight.rs）不分青紅皂白殺表上全部 pid，故用同一把鎖互斥執行。
        let _serial = crate::inflight::lock_real_process_tests();
        let FakeCli {
            dir,
            call,
            root,
            world_id,
            session_dir,
            claude_home,
            working_dir,
        } = fake_claude("ping");
        let calls = |index: usize| -> (Vec<String>, String) {
            let text = std::fs::read_to_string(session_dir.join("calls.jsonl")).unwrap();
            let line: serde_json::Value =
                serde_json::from_str(text.lines().nth(index).unwrap()).unwrap();
            (
                line["args"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_owned())
                    .collect(),
                line["prompt"].as_str().unwrap().to_owned(),
            )
        };

        let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "老闆晚安")];
        let reply1 = run_turn(&call, &root, &world_id, turn_input(&events, 0), |_| {})
            .await
            .unwrap();
        assert_eq!(reply1, "回覆1");
        let store_path = data::lanes_path(&root, &world_id).unwrap();
        let session_id = read_store(&store_path)
            .values()
            .next()
            .unwrap()
            .session_id
            .clone();
        let session_path = session_file::session_file_path(&claude_home, &working_dir, &session_id);
        let before = std::fs::read_to_string(&session_path).unwrap();

        // 剛呼叫完：快取還很新，不必花這筆
        assert_eq!(keepalive(&call, &root, &world_id).await.unwrap(), 0);

        // 距上輪 200 秒＝快取還活著，正是該保溫的時候
        set_lane_epoch(&store_path, now_epoch() - 200);
        assert_eq!(keepalive(&call, &root, &world_id).await.unwrap(), 1);
        let (ping_args, ping_prompt) = calls(1);
        assert!(ping_args
            .windows(2)
            .any(|w| w == ["--resume", session_id.as_str()]));
        assert_eq!(ping_prompt, PING_PROMPT);
        // 問答已截掉：檔案逐字回到 ping 前，正典 transcript 也沒被碰過
        assert_eq!(std::fs::read_to_string(&session_path).unwrap(), before);
        // 保溫成功＝壽命重新計時
        assert!(now_epoch() - read_store(&store_path)[&"chars:sonnet".to_owned()].last_call_epoch < 5);

        // 快取已過期：保了也只是全額重建，不如留給下一輪自己重開
        set_lane_epoch(&store_path, now_epoch() - 3600);
        assert_eq!(keepalive(&call, &root, &world_id).await.unwrap(), 0);

        // 上輪沒收尾（pending 未清）的線不碰：下一輪本來就要重開
        set_lane_epoch(&store_path, now_epoch() - 200);
        let mut store = read_store(&store_path);
        store.values_mut().next().unwrap().pending_rewrite = Some(PendingRewrite {
            confidential: None,
            prefix: None,
        });
        write_store(&store_path, &store).unwrap();
        assert_eq!(keepalive(&call, &root, &world_id).await.unwrap(), 0);
        store.values_mut().next().unwrap().pending_rewrite = None;
        write_store(&store_path, &store).unwrap();

        // ping 過的線照樣續聊：回覆編號是 2 而不是 3＝session 裡真的沒留下保溫問答
        events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", &reply1));
        events.push(event(TranscriptKind::Player, "", "阿濤", "來一杯麥酒"));
        let reply2 = run_turn(&call, &root, &world_id, turn_input(&events, 0), |_| {})
            .await
            .unwrap();
        assert_eq!(reply2, "回覆2");
        let (args, prompt) = calls(2);
        assert!(args
            .windows(2)
            .any(|w| w == ["--resume", session_id.as_str()]));
        assert!(prompt.contains("阿濤：來一杯麥酒"));
        assert!(!prompt.contains("老闆晚安"));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// 素材變動先用補丁保住快取；超過五分鐘才把新素材追平進凍結快照，並留下可供額度頁讀取的原因紀錄。
    #[cfg(unix)]
    #[tokio::test]
    async fn lane_patches_material_then_rebases_after_cache_expiry() {
        // run_cli 會把子程序 pid 登記進 inflight 的全域 children 表；kill_all_children 的
        // 測試（inflight.rs）不分青紅皂白殺表上全部 pid，故用同一把鎖互斥執行。
        let _serial = crate::inflight::lock_real_process_tests();
        let dir = std::env::temp_dir().join(format!("tt-lanes-patch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let working_dir = dir.join("ws");
        let claude_home = dir.join("claude-home");
        let root = dir.join("root");
        let usage_log = dir.join("usage.log");
        let world_id = ulid::Ulid::generate().to_string();
        std::fs::create_dir_all(&working_dir).unwrap();
        std::fs::create_dir_all(root.join("worlds").join(&world_id)).unwrap();
        let session_dir = session_file::session_file_path(&claude_home, &working_dir, "probe")
            .parent()
            .unwrap()
            .to_path_buf();
        std::fs::create_dir_all(&session_dir).unwrap();

        let script = dir.join("fake-claude.py");
        std::fs::write(
            &script,
            r#"#!/usr/bin/env python3
import json, sys, os, uuid
args = sys.argv[1:]
def flag(name):
    return args[args.index(name) + 1] if name in args else None
sid, rid = flag('--session-id'), flag('--resume')
prompt = sys.stdin.read()
d = os.environ['FAKE_SESSION_DIR']
with open(os.path.join(d, 'calls.jsonl'), 'a') as f:
    f.write(json.dumps({'args': args, 'prompt': prompt}) + '\n')
path = os.path.join(d, (sid or rid) + '.jsonl')
if rid and not os.path.exists(path):
    sys.exit(3)
u, a = str(uuid.uuid4()), str(uuid.uuid4())
with open(path, 'a') as f:
    f.write(json.dumps({'type': 'user', 'uuid': u,
                        'message': {'role': 'user', 'content': prompt}}) + '\n')
    f.write(json.dumps({'type': 'assistant', 'uuid': a,
                        'message': {'role': 'assistant', 'content': [{'type': 'text', 'text': '回覆'}]}}) + '\n')
print(json.dumps({'type': 'result', 'is_error': False, 'result': '回覆',
                  'total_cost_usd': 0.002,
                  'usage': {'input_tokens': 10, 'cache_creation_input_tokens': 0,
                            'cache_read_input_tokens': 90, 'output_tokens': 5}}))
"#,
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let call = ClaudeCall {
            program: script,
            working_dir: working_dir.clone(),
            envs: vec![(
                "FAKE_SESSION_DIR".to_owned(),
                session_dir.to_string_lossy().into_owned(),
            )],
            model: "sonnet".to_owned(),
            usage_log: Some(usage_log.clone()),
            claude_home,
        };
        let calls = |index: usize| -> (Vec<String>, String) {
            let text = std::fs::read_to_string(session_dir.join("calls.jsonl")).unwrap();
            let line: serde_json::Value =
                serde_json::from_str(text.lines().nth(index).unwrap()).unwrap();
            (
                line["args"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_str().unwrap().to_owned())
                    .collect(),
                line["prompt"].as_str().unwrap().to_owned(),
            )
        };
        let old_system = "## 角色卡\n舊設定\n";
        let new_system = "## 角色卡\n新設定\n";
        let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第一句")];
        let mut first = turn_input(&events, 0);
        first.frozen_system = old_system.to_owned();
        first.prefix = None;
        run_turn(&call, &root, &world_id, first, |_| {})
            .await
            .unwrap();

        events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", "回覆"));
        events.push(event(TranscriptKind::Player, "", "阿濤", "第二句"));
        let mut patched = turn_input(&events, 0);
        patched.frozen_system = new_system.to_owned();
        patched.prefix = None;
        run_turn(&call, &root, &world_id, patched, |_| {})
            .await
            .unwrap();
        let (patch_args, patch_prompt) = calls(1);
        assert!(patch_args
            .windows(2)
            .any(|window| window == ["--system-prompt", old_system]));
        assert!(patch_prompt.contains("## 設定更新"));
        assert!(patch_prompt.contains("## 角色卡\n新設定\n"));

        let lanes_path = data::lanes_path(&root, &world_id).unwrap();
        let mut lanes: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&lanes_path).unwrap()).unwrap();
        let epoch = lanes["chars:sonnet"]["last_call_epoch"].as_u64().unwrap();
        lanes["chars:sonnet"]["last_call_epoch"] = serde_json::Value::from(epoch - 3_600);
        std::fs::write(&lanes_path, serde_json::to_string_pretty(&lanes).unwrap()).unwrap();

        events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", "回覆"));
        events.push(event(TranscriptKind::Player, "", "阿濤", "第三句"));
        let mut rebased = turn_input(&events, 0);
        rebased.frozen_system = new_system.to_owned();
        rebased.prefix = None;
        run_turn(&call, &root, &world_id, rebased, |_| {})
            .await
            .unwrap();
        let (rebase_args, rebase_prompt) = calls(2);
        assert!(rebase_args
            .windows(2)
            .any(|window| window == ["--system-prompt", new_system]));
        assert!(!rebase_prompt.contains("## 設定更新"));

        // log（包 4）：一次呼叫一行 JSONL，線的動作與該次用量寫在同一筆
        let log = std::fs::read_to_string(&usage_log).unwrap();
        let records: Vec<serde_json::Value> = log
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(records.len(), 3);
        for record in &records {
            assert_eq!(record["lane"], "chars:sonnet");
            assert_eq!(record["prompt_tokens"], 100); // 10＋0＋90
            assert_eq!(record["cached_tokens"], 90);
            assert_eq!(record["cost_usd"], 0.002);
            assert!(record["system_tokens"].as_u64().unwrap() > 0);
        }
        // 第一輪開線＝暖機，理論可中量 0
        assert_eq!(records[0]["diag"], "warmup");
        assert_eq!(records[0]["reason"], "first-turn");
        assert_eq!(records[0]["expected_cached"], 0);
        // 第二輪走補丁：上輪送了 100，這輪中 90＝正常
        assert_eq!(records[1]["diag"], "ok");
        assert_eq!(records[1]["patched"], true);
        assert_eq!(records[1]["expected_cached"], 100);
        // 第三輪快取已過期（手改 epoch 減 3600）＝追平，換上新素材
        assert_eq!(records[2]["diag"], "expired");
        assert_eq!(records[2]["rebased"], true);
        assert!(records[2]["age_secs"].as_u64().unwrap() >= 3_600);
        assert_ne!(records[2]["system_hash"], records[1]["system_hash"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
