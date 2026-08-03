//! claude lane resume 續聊（prompt-cache-optimization 包 2）。
//! 每桌兩條 session：chars（全角色共用）＋gm。凍結 system 每輪逐字重帶、只送新事件與回合尾段，
//! 快取命中率的天花板因此變成「只有最後一句沒中」（實驗 E6：99.7%）。
//! 正典 transcript 與 session 歷史靠水位＋指紋＋回覆對點對齊；任何對不上、任何改寫或呼叫失敗，
//! 一律丟線重開全量重建（降級鏈永遠可用，聊天不中斷）。
//! chars 線的私設隔離靠「回合注入機密段→回合後從 session 檔抹掉」維持（案 C，2026-08-03 拍板）。

use crate::cli;
use crate::data::{self, TranscriptEvent, TranscriptKind};
use crate::session_file;
use crate::transport;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
    /// 不落 transcript（GM 點名）
    None,
    /// 角色台詞：事件原文＝回覆原文
    Dialogue { speaker_id: String },
    /// GM 旁白：事件原文＝剝掉狀態欄後的顯示文字
    Narration,
}

pub(crate) struct TurnInput<'a> {
    pub lane: Lane,
    pub scene: u64,
    pub events: &'a [TranscriptEvent],
    /// 本輪重組的凍結 system；與存檔快照逐字不同＝素材變動＝重開線（包 3 改為補丁）
    pub frozen_system: String,
    /// 回合尾段（transport::chars_lane_turn／gm_lane_turn 的 tail）
    pub tail: String,
    /// tail 內回合後要抹掉的機密子段（chars 線私設＋限定條目）
    pub confidential: Option<String>,
    /// 回合後補在最後一則 assistant 前的名字前綴（chars 線「X：」）
    pub prefix: Option<String>,
    pub echo: ReplyEcho,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct LaneStore {
    #[serde(default)]
    chars: Option<LaneState>,
    #[serde(default)]
    gm: Option<LaneState>,
}

impl LaneStore {
    fn lane_mut(&mut self, lane: Lane) -> &mut Option<LaneState> {
        match lane {
            Lane::Chars => &mut self.chars,
            Lane::Gm => &mut self.gm,
        }
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
    /// 凍結 system 快照全文（resume 每輪重帶同一份，E7：動一字整條快取全滅）
    snapshot: String,
    /// 呼叫前先寫、抹寫完成後清空——中途崩潰時下一輪看到未清的 pending 就整線重開，
    /// 機密段不會留在 session 歷史裡被下一個角色看到
    pending_rewrite: Option<PendingRewrite>,
    /// 上輪回覆應以此形狀出現在水位位置（前端呼叫返回後才落 transcript）
    expected_reply: Option<ExpectedReply>,
    /// 追平判斷用（包 3：距上輪 >5 分鐘＝快取已死，改寫快照零成本）
    last_call_at: String,
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
    Resume { session_id: String, base: usize },
    Reopen,
}

/// 決定這一輪續聊還是重開。所有「對不上」都走 Reopen：重開永遠正確，只是少省一次快取。
fn plan_turn(state: Option<&LaneState>, input: &TurnInput<'_>) -> TurnPlan {
    let Some(state) = state else {
        return TurnPlan::Reopen;
    };
    if state.pending_rewrite.is_some() {
        return TurnPlan::Reopen; // 上一輪中途斷掉，session 內容不可信（可能殘留機密段）
    }
    if state.scene != input.scene {
        return TurnPlan::Reopen; // 換場＝重開（拍板行為）
    }
    if state.snapshot != input.frozen_system {
        return TurnPlan::Reopen; // 凍結素材變動（包 3 改為補丁＋追平）
    }
    let mut base = state.sent_events;
    if base > input.events.len() {
        return TurnPlan::Reopen; // 正典被收回到水位之前
    }
    if events_fingerprint(&input.events[..base]) != state.sent_hash {
        return TurnPlan::Reopen; // 已送段被改動
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
            _ => return TurnPlan::Reopen, // 回覆沒落檔或被改＝session 與正典分岔
        }
    }
    TurnPlan::Resume {
        session_id: state.session_id.clone(),
        base,
    }
}

/// 組本輪 prompt：水位之後的新事件＋回合尾段。開線（全量重建）帶對話紀錄標頭，
/// 形狀比照單發 flatten；續聊只送增量，與 session 內既有歷史逐字銜接。
fn build_prompt(events: &[TranscriptEvent], base: usize, tail: &str, opening: bool) -> String {
    let lines: Vec<String> = events[base..].iter().map(transport::lane_event_line).collect();
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

fn expected_reply_for(echo: &ReplyEcho, reply: &str) -> Option<ExpectedReply> {
    match echo {
        ReplyEcho::None => None,
        ReplyEcho::Dialogue { speaker_id } => Some(ExpectedReply {
            speaker_id: speaker_id.clone(),
            kind: TranscriptKind::Dialogue,
            text: reply.to_owned(),
        }),
        // 前端落 transcript 的是剝掉狀態欄的顯示文字（gm_narrate 的既有行為）
        ReplyEcho::Narration => Some(ExpectedReply {
            speaker_id: String::new(),
            kind: TranscriptKind::Narration,
            text: transport::extract_state_block(reply).1,
        }),
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
    let mut store = read_store(&store_path);
    let mut plan = plan_turn(store.lane_mut(input.lane).as_ref(), &input);

    loop {
        let (session_id, base, opening) = match &plan {
            TurnPlan::Resume { session_id, base } => (session_id.clone(), *base, false),
            TurnPlan::Reopen => (new_session_id(), 0, true),
        };
        let prompt = build_prompt(input.events, base, &input.tail, opening);
        let session = if opening {
            cli::ClaudeSession::Open(&session_id)
        } else {
            cli::ClaudeSession::Resume(&session_id)
        };
        let args = cli::claude_session_args(&call.model, &input.frozen_system, &session);

        *store.lane_mut(input.lane) = Some(LaneState {
            session_id: session_id.clone(),
            scene: input.scene,
            sent_events: input.events.len(),
            sent_hash: events_fingerprint(input.events),
            snapshot: input.frozen_system.clone(),
            pending_rewrite: Some(PendingRewrite {
                confidential: input.confidential.clone(),
                prefix: input.prefix.clone(),
            }),
            expected_reply: None,
            last_call_at: data::local_timestamp_seconds().unwrap_or_default(),
        });
        write_store(&store_path, &store)?;

        let result = cli::run_cli(
            &call.program,
            &call.working_dir,
            &args,
            &prompt,
            &call.envs,
            cli::parse_claude_line,
            call.usage_log.as_deref().map(|path| cli::UsageLog {
                path,
                transport: "claude",
                model: &call.model,
                parse: cli::parse_claude_usage,
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
                let slot = store.lane_mut(input.lane);
                match rewrite {
                    Ok(()) => {
                        if let Some(state) = slot.as_mut() {
                            state.pending_rewrite = None;
                            state.expected_reply = expected_reply_for(&input.echo, &reply);
                        }
                    }
                    // 抹寫失敗＝session 內容不可信，丟線；下一輪自動重開全量，本輪回覆照常送回
                    Err(_) => *slot = None,
                }
                write_store(&store_path, &store)?;
                return Ok(reply);
            }
            // 續聊失敗（session 檔認不得、CLI 拒絕 resume 等）＝丟線重開全量再試一次
            Err(_) if !opening => {
                plan = TurnPlan::Reopen;
            }
            Err(error) => {
                *store.lane_mut(input.lane) = None;
                write_store(&store_path, &store)?;
                return Err(error.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(kind: TranscriptKind, speaker_id: &str, name: &str, text: &str) -> TranscriptEvent {
        TranscriptEvent {
            ts: "2026-08-03T21:00:00+08:00".to_owned(),
            speaker_id: speaker_id.to_owned(),
            speaker_name: name.to_owned(),
            kind,
            text: text.to_owned(),
            state: None,
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
            pending_rewrite: None,
            expected_reply: None,
            last_call_at: String::new(),
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

        assert!(matches!(plan_turn(None, &input), TurnPlan::Reopen));

        let good = lane_state(&events, 0);
        match plan_turn(Some(&good), &input) {
            TurnPlan::Resume { session_id, base } => {
                assert_eq!(session_id, "sid-1");
                assert_eq!(base, 2);
            }
            TurnPlan::Reopen => panic!("狀態齊備必須續聊"),
        }

        let mut pending = good.clone();
        pending.pending_rewrite = Some(PendingRewrite {
            confidential: None,
            prefix: None,
        });
        assert!(matches!(plan_turn(Some(&pending), &input), TurnPlan::Reopen));

        let mut scene_changed = good.clone();
        scene_changed.scene = 1;
        assert!(matches!(
            plan_turn(Some(&scene_changed), &input),
            TurnPlan::Reopen
        ));

        let mut snapshot_changed = good.clone();
        snapshot_changed.snapshot = "凍結B".to_owned();
        assert!(matches!(
            plan_turn(Some(&snapshot_changed), &input),
            TurnPlan::Reopen
        ));

        let mut ahead = good.clone();
        ahead.sent_events = 3; // 正典被收回到水位前
        assert!(matches!(plan_turn(Some(&ahead), &input), TurnPlan::Reopen));

        // 已送段被改動（收回重寫第一句）
        let edited = [
            event(TranscriptKind::Player, "", "阿濤", "改過的第一句"),
            events[1].clone(),
        ];
        let edited_input = turn_input(&edited, 0);
        assert!(matches!(
            plan_turn(Some(&good), &edited_input),
            TurnPlan::Reopen
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
        match plan_turn(Some(&state), &input) {
            TurnPlan::Resume { base, .. } => assert_eq!(base, 2),
            TurnPlan::Reopen => panic!("回覆已落檔必須續聊"),
        }

        // 回覆事件被玩家改字＝session 與正典分岔
        let tampered = [
            before[0].clone(),
            event(TranscriptKind::Dialogue, "fox-id", "狐狸", "被改過的晚安"),
        ];
        let tampered_input = turn_input(&tampered, 0);
        assert!(matches!(
            plan_turn(Some(&state), &tampered_input),
            TurnPlan::Reopen
        ));

        // 回覆還沒落檔（前端沒寫進 transcript）
        let missing_input = turn_input(&before, 0);
        assert!(matches!(
            plan_turn(Some(&state), &missing_input),
            TurnPlan::Reopen
        ));
    }

    #[test]
    fn narration_echo_expects_display_text_without_state_fence() {
        let reply = "夜更深了。\n```state\ntime: 午夜\n```";
        let expected = expected_reply_for(&ReplyEcho::Narration, reply).unwrap();
        assert_eq!(expected.kind, TranscriptKind::Narration);
        assert_eq!(expected.text, "夜更深了。");
        assert!(expected_reply_for(&ReplyEcho::None, reply).is_none());
    }

    #[test]
    fn prompt_carries_header_only_on_reopen_and_tail_alone_without_events() {
        let events = [
            event(TranscriptKind::Player, "", "阿濤", "你好"),
            event(TranscriptKind::Narration, "", "GM", "夜深了"),
        ];
        let full = build_prompt(&events, 0, "尾段", true);
        assert!(full.starts_with("以下是到目前為止的對話紀錄：\n\n阿濤：你好\n\n（旁白）夜深了"));
        assert!(full.ends_with("——\n尾段"));
        let increment = build_prompt(&events, 1, "尾段", false);
        assert_eq!(increment, "（旁白）夜深了\n\n——\n尾段");
        assert_eq!(build_prompt(&events, 2, "尾段", false), "尾段");
    }

    /// 端到端（假 CLI）：開線→抹寫→續聊只送增量→正典被改→自動重開；
    /// 續聊呼叫失敗（session 檔消失）→ 同一輪內降級重開。
    #[cfg(unix)]
    #[tokio::test]
    async fn lane_turns_open_rewrite_resume_and_degrade() {
        let dir = std::env::temp_dir().join(format!("tt-lanes-e2e-{}", std::process::id()));
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

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
