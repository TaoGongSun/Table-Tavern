use super::*;

/// 空目錄沒有 format.json，寫入閘門會當成唯讀。測試桌補上本版標記才寫得了 lanes.json。
fn mark_playable(dir: &std::path::Path) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        dir.join("format.json"),
        r#"{"format_version":1,"app_version":"test"}"#,
    )
    .unwrap();
}

struct FakeCli {
    dir: PathBuf,
    call: LaneCall,
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
    mark_playable(&root.join("worlds").join(&world_id));
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

    let call = LaneCall {
        provider: LaneProvider::Claude,
        program: script,
        working_dir: working_dir.clone(),
        envs: vec![(
            "FAKE_SESSION_DIR".to_owned(),
            session_dir.to_string_lossy().into_owned(),
        )],
        model: Some("sonnet".to_owned()),
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

/// 假 Agy：開線由 CLI 回傳固定 conversation ID；續聊只接受同一 ID，並回報 usage。
#[cfg(unix)]
fn fake_agy(tag: &str) -> FakeCli {
    let dir = std::env::temp_dir().join(format!("tt-lanes-agy-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let working_dir = dir.join("ws");
    let root = dir.join("root");
    let world_id = ulid::Ulid::generate().to_string();
    let usage_log = dir.join("usage.jsonl");
    std::fs::create_dir_all(&working_dir).unwrap();
    mark_playable(&root.join("worlds").join(&world_id));
    let script = dir.join("fake-agy.py");
    std::fs::write(
        &script,
        r#"#!/usr/bin/env python3
import json, os, sys
args = sys.argv[1:]
def flag(name):
    return args[args.index(name) + 1] if name in args else None
cid = flag('--conversation')
prompt = flag('-p') or ''
d = os.environ['FAKE_AGY_DIR']
with open(os.path.join(d, 'calls.jsonl'), 'a') as f:
    f.write(json.dumps({'args': args, 'prompt': prompt}, ensure_ascii=False) + '\n')
if cid and cid != 'agy-conversation-1':
    sys.exit(3)
turn = 2 if cid else 1
input_tokens = 1200 if cid else 1000
output_tokens = 250 if cid else 100
cached = 900 if cid else 0
print(json.dumps({'event': 'init', 'conversation_id': 'agy-conversation-1'}))
print(json.dumps({'event': 'result', 'result': {
    'status': 'SUCCESS', 'response': '回覆' + str(turn), 'num_turns': turn,
    'usage': {'input_tokens': input_tokens, 'output_tokens': output_tokens,
              'thinking_tokens': 20, 'cache_read_tokens': cached,
              'total_tokens': input_tokens + output_tokens}}}, ensure_ascii=False))
"#,
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let call = LaneCall {
        provider: LaneProvider::Agy,
        program: script,
        working_dir: working_dir.clone(),
        envs: vec![(
            "FAKE_AGY_DIR".to_owned(),
            dir.to_string_lossy().into_owned(),
        )],
        model: Some("gemini-test".to_owned()),
        usage_log: Some(usage_log),
        claude_home: dir.join("unused-claude-home"),
    };
    FakeCli {
        dir,
        call,
        root,
        world_id,
        session_dir: PathBuf::new(),
        claude_home: PathBuf::new(),
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
        truncated: false,
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
        scope: None,
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
        provider: "claude".to_owned(),
        model: "sonnet".to_owned(),
        last_call_epoch: 1_000,
        last_prompt_tokens: 0,
        agy_usage: None,
        redaction: REDACTION_VERSION,
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
        plan_turn(None, &input, 1_010, LaneProvider::Claude),
        TurnPlan::Reopen {
            reason: ReopenReason::FirstTurn
        }
    ));

    let good = lane_state(&events, 0);
    match plan_turn(Some(&good), &input, 1_010, LaneProvider::Claude) {
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
        plan_turn(Some(&pending), &input, 1_010, LaneProvider::Claude),
        TurnPlan::Reopen {
            reason: ReopenReason::PendingRewrite
        }
    ));

    let mut scene_changed = good.clone();
    scene_changed.scene = 1;
    assert!(matches!(
        plan_turn(Some(&scene_changed), &input, 1_010, LaneProvider::Claude),
        TurnPlan::Reopen {
            reason: ReopenReason::SceneChanged
        }
    ));

    let mut changed_input = turn_input(&events, 0);
    changed_input.frozen_system = "凍結B".to_owned();
    match plan_turn(Some(&good), &changed_input, 1_010, LaneProvider::Claude) {
        TurnPlan::Resume { system, patch, .. } => {
            assert_eq!(system, "凍結A");
            assert!(patch.is_some());
        }
        TurnPlan::Reopen { .. } => panic!("快取存活時素材變動必須走補丁"),
    }

    let mut ahead = good.clone();
    ahead.sent_events = 3; // 正典被收回到水位前
    assert!(matches!(
        plan_turn(Some(&ahead), &input, 1_010, LaneProvider::Claude),
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
        plan_turn(Some(&good), &edited_input, 1_010, LaneProvider::Claude),
        TurnPlan::Reopen {
            reason: ReopenReason::HistoryEdited
        }
    ));
}

/// 上輪回覆（expected_reply）落檔後從水位跳過；沒落檔或被改動＝重開。
/// 線名細分：claude 不帶 scope（全角色共用一條），grok 的 chars 線一角一條。
#[test]
fn lane_key_splits_by_scope_only_when_given() {
    assert_eq!(lane_key(Lane::Chars, "sonnet", None), "chars:sonnet");
    assert_eq!(
        lane_key(Lane::Chars, "grok-4.6", Some("fox-id")),
        "chars:grok-4.6:fox-id"
    );
    assert_eq!(lane_key(Lane::Gm, "grok-4.6", None), "gm:grok-4.6");
}

/// 線名撞到別家 CLI 開的線（同桌換傳輸、模型字樣剛好一樣）：session id 是對方的，
/// 直接重開比讓 resume 撞牆再降級便宜。舊檔沒有 provider 欄位一律當 claude。
#[test]
fn switching_cli_reopens_instead_of_resuming_someone_elses_session() {
    let events = [event(TranscriptKind::Player, "", "阿濤", "你好")];
    let mut input = turn_input(&events, 0);
    input.scope = Some("fox-id".to_owned());
    let mut state = lane_state(&events, 0);
    state.expected_reply = None;
    assert!(matches!(
        plan_turn(Some(&state), &input, 1_010, LaneProvider::Grok),
        TurnPlan::Reopen {
            reason: ReopenReason::ProviderChanged
        }
    ));
    let legacy: LaneState = serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
    assert_eq!(legacy.provider, "claude");
    let without_provider = serde_json::to_string(&state)
        .unwrap()
        .replace("\"provider\":\"claude\",", "");
    let old: LaneState = serde_json::from_str(&without_provider).unwrap();
    assert_eq!(old.provider, "claude"); // 舊檔照樣續聊，不白重建一次快取
}

/// 素材漂移的兩種處置：claude 快取過期時整份追平、還活著時走 prompt 補丁；
/// grok 的 system 凍在 session 建立那刻換不掉，補丁壓不過舊 system，所以一漂移就整線重開。
#[test]
fn drifted_system_rebases_on_claude_and_reopens_on_grok() {
    let events = [event(TranscriptKind::Player, "", "阿濤", "你好")];
    let mut input = turn_input(&events, 0);
    input.frozen_system = "凍結B".to_owned(); // 素材漂移
    let mut state = lane_state(&events, 0);
    state.expected_reply = None;
    let expired = state.last_call_epoch + CACHE_TTL_SECS + 1;

    match plan_turn(Some(&state), &input, expired, LaneProvider::Claude) {
        TurnPlan::Resume {
            system,
            patch,
            rebased,
            ..
        } => {
            assert_eq!(system, "凍結B"); // 整份換新
            assert!(patch.is_none());
            assert!(rebased);
        }
        TurnPlan::Reopen { .. } => panic!("claude 過期只追平不重開"),
    }
    match plan_turn(
        Some(&state),
        &input,
        state.last_call_epoch + 1,
        LaneProvider::Claude,
    ) {
        TurnPlan::Resume { system, patch, .. } => {
            assert_eq!(system, "凍結A"); // 快取還活著就別動它
            assert!(patch.is_some()); // 漂移走 prompt 內的補丁
        }
        TurnPlan::Reopen { .. } => panic!("claude 漂移走補丁不重開"),
    }

    state.provider = LaneProvider::Grok.as_str().to_owned();
    assert!(matches!(
        plan_turn(Some(&state), &input, expired, LaneProvider::Grok),
        TurnPlan::Reopen {
            reason: ReopenReason::SystemChanged
        }
    ));
    let same = turn_input(&events, 0); // frozen_system 維持「凍結A」
    match plan_turn(Some(&state), &same, expired, LaneProvider::Grok) {
        TurnPlan::Resume { system, patch, .. } => {
            assert_eq!(system, "凍結A");
            assert!(patch.is_none()); // grok 從頭到尾只靠開線那份 system
        }
        TurnPlan::Reopen { .. } => panic!("素材沒動就該續聊"),
    }
}

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
    match plan_turn(Some(&state), &input, 1_010, LaneProvider::Claude) {
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
        plan_turn(Some(&state), &tampered_input, 1_010, LaneProvider::Claude),
        TurnPlan::Reopen {
            reason: ReopenReason::ReplyDiverged
        }
    ));

    // 回覆還沒落檔（前端沒寫進 transcript）
    let missing_input = turn_input(&before, 0);
    assert!(matches!(
        plan_turn(Some(&state), &missing_input, 1_010, LaneProvider::Claude),
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

    match plan_turn(Some(&state), &input, 1_301, LaneProvider::Claude) {
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

/// grok 沒有回合後抹寫，機密段一旦送進去就永遠留在該線歷史裡：run_turn 出聲擋下，
/// 不讓呼叫端誤用共線＋回合注入那套（會把 A 的私設漏給 B）。
#[cfg(unix)]
#[tokio::test]
async fn grok_lane_refuses_confidential_injection() {
    let FakeCli {
        dir: _dir,
        mut call,
        root,
        world_id,
        ..
    } = fake_claude("grok-guard");
    call.provider = LaneProvider::Grok;
    let events = [event(TranscriptKind::Player, "", "阿濤", "你好")];
    let mut input = turn_input(&events, 0);
    input.confidential = Some("狐狸的私設".to_owned());
    let error = run_turn(&call, &root, &world_id, input, None, |_: &str| {})
        .await
        .expect_err("帶機密段的 grok 線必須被擋下");
    assert!(error.starts_with(r#"TTMSG:{"code":"lane_rewrite_unsupported""#));
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

#[cfg(unix)]
#[tokio::test]
async fn agy_lane_persists_exact_conversation_and_resumes_with_delta_only() {
    let _serial = crate::inflight::lock_real_process_tests();
    let FakeCli {
        dir,
        call,
        root,
        world_id,
        ..
    } = fake_agy("resume");
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第一句")];
    let mut first = turn_input(&events, 0);
    first.prefix = None;
    first.scope = Some("fox-id".to_owned());
    assert_eq!(
        run_turn(&call, &root, &world_id, first, None, |_| {})
            .await
            .unwrap()
            .text,
        "回覆1"
    );
    let store_path = data::lanes_path(&root, &world_id).unwrap();
    let state = read_store(&store_path).values().next().unwrap().clone();
    assert_eq!(state.session_id, "agy-conversation-1");
    assert_eq!(state.redaction, REDACTION_VERSION); // 新寫入的線一律標上目前的角色側渲染版本

    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", "回覆1"));
    events.push(event(TranscriptKind::Player, "", "阿濤", "只有這句是新的"));
    let mut second = turn_input(&events, 0);
    second.prefix = None;
    second.scope = Some("fox-id".to_owned());
    assert_eq!(
        run_turn(&call, &root, &world_id, second, None, |_| {})
            .await
            .unwrap()
            .text,
        "回覆2"
    );

    let calls = std::fs::read_to_string(dir.join("calls.jsonl")).unwrap();
    let calls: Vec<serde_json::Value> = calls
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(calls.len(), 2);
    assert!(!calls[0]["args"]
        .as_array()
        .unwrap()
        .iter()
        .any(|arg| arg == "--conversation"));
    assert!(calls[0]["prompt"].as_str().unwrap().contains("凍結A"));
    let second_args = calls[1]["args"].as_array().unwrap();
    let resume = second_args
        .iter()
        .position(|arg| arg == "--conversation")
        .unwrap();
    assert_eq!(second_args[resume + 1], "agy-conversation-1");
    let delta = calls[1]["prompt"].as_str().unwrap();
    assert!(delta.contains("只有這句是新的"));
    assert!(!delta.contains("第一句"));
    assert!(!delta.contains("回覆1"));
    assert!(!delta.contains("凍結A"));

    let usage = std::fs::read_to_string(dir.join("usage.jsonl")).unwrap();
    let usage: Vec<serde_json::Value> = usage
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(usage.len(), 2);
    assert_eq!(usage[0]["agy_conversation_id"], "agy-conversation-1");
    assert_eq!(usage[1]["agy_conversation_id"], "agy-conversation-1");
    assert_eq!(usage[0]["agy_cache_read_tokens"], 0);
    assert_eq!(usage[1]["agy_cache_read_tokens"], 900);
    assert_eq!(usage[1]["agy_num_turns"], 2);
    assert_eq!(usage[1]["prompt_tokens"], 1100);
    assert_eq!(usage[1]["cached_tokens"], 900);
    assert_eq!(usage[1]["output_tokens"], 150);
    assert_eq!(usage[1]["hit_rate"], 81.8);

    std::fs::remove_dir_all(&dir).unwrap();
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
    let reply1 = run_turn(&call, &root, &world_id, input, None, |_| {})
        .await
        .unwrap()
        .text;
    assert_eq!(reply1, "回覆1");
    let (args1, prompt1) = calls(0);
    let open_flag = args1.iter().position(|a| a == "--session-id").unwrap();
    let first_session = args1[open_flag + 1].clone();
    assert!(prompt1.starts_with("以下是到目前為止的對話紀錄："));
    assert!(prompt1.contains("阿濤：老闆晚安"));
    assert!(prompt1.contains("通緝犯"));
    // 回合後抹寫：機密段消失、assistant 補了名字前綴
    let session_path = session_file::session_file_path(&claude_home, &working_dir, &first_session);
    let rewritten = std::fs::read_to_string(&session_path).unwrap();
    assert!(!rewritten.contains("通緝犯"));
    assert!(rewritten.contains("狐狸：回覆1"));

    // 第二輪：回覆已落正典＋玩家新句 → 續聊，只送新句
    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", &reply1));
    events.push(event(TranscriptKind::Player, "", "阿濤", "來一杯麥酒"));
    let reply2 = run_turn(
        &call,
        &root,
        &world_id,
        turn_input(&events, 0),
        None,
        |_| {},
    )
    .await
    .unwrap()
    .text;
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
    let reply3 = run_turn(
        &call,
        &root,
        &world_id,
        turn_input(&events, 0),
        None,
        |_| {},
    )
    .await
    .unwrap()
    .text;
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
    let reply4 = run_turn(
        &call,
        &root,
        &world_id,
        turn_input(&events, 0),
        None,
        |_| {},
    )
    .await
    .unwrap()
    .text;
    assert_eq!(reply4, "回覆1");
    let (args4, _) = calls(3);
    assert!(args4.contains(&"--resume".to_owned())); // 先試續聊
    let (args5, prompt5) = calls(4);
    assert!(args5.contains(&"--session-id".to_owned())); // 降級重開
    assert!(prompt5.starts_with("以下是到目前為止的對話紀錄："));

    // 第五輪換模型（同桌 haiku 角色）：另開自己的線，sonnet 線不受影響（按模型分池）
    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", &reply4));
    let haiku_call = LaneCall {
        provider: LaneProvider::Claude,
        model: Some("haiku".to_owned()),
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
        None,
        |_| {},
    )
    .await
    .unwrap();
    let (args6, _) = calls(5);
    assert!(args6.contains(&"--session-id".to_owned())); // haiku 沒有既有線，全量開新
    let lanes_json = std::fs::read_to_string(data::lanes_path(&root, &world_id).unwrap()).unwrap();
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
    let reply1 = run_turn(
        &call,
        &root,
        &world_id,
        turn_input(&events, 0),
        None,
        |_| {},
    )
    .await
    .unwrap()
    .text;
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
    let reply2 = run_turn(
        &call,
        &root,
        &world_id,
        turn_input(&events, 0),
        None,
        |_| {},
    )
    .await
    .unwrap()
    .text;
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
    mark_playable(&root.join("worlds").join(&world_id));
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

    let call = LaneCall {
        provider: LaneProvider::Claude,
        program: script,
        working_dir: working_dir.clone(),
        envs: vec![(
            "FAKE_SESSION_DIR".to_owned(),
            session_dir.to_string_lossy().into_owned(),
        )],
        model: Some("sonnet".to_owned()),
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
    run_turn(&call, &root, &world_id, first, None, |_| {})
        .await
        .unwrap();

    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", "回覆"));
    events.push(event(TranscriptKind::Player, "", "阿濤", "第二句"));
    let mut patched = turn_input(&events, 0);
    patched.frozen_system = new_system.to_owned();
    patched.prefix = None;
    run_turn(&call, &root, &world_id, patched, None, |_| {})
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
    run_turn(&call, &root, &world_id, rebased, None, |_| {})
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
    // 第一輪開線：理論可中量 0，但假 CLI 回報中了 90——數字優先，快取軸說 hit，
    // 「這條線剛開」由 reason 那欄負責講，不去蓋掉觀測值
    assert_eq!(records[0]["mode"], "resume");
    assert_eq!(records[0]["cache"], "hit");
    assert_eq!(records[0]["reason"], "first-turn");
    assert_eq!(records[0]["expected_cached"], 0);
    // 第二輪走補丁：上輪送了 100，這輪中 90＝正常
    assert_eq!(records[1]["cache"], "hit");
    assert_eq!(records[1]["patched"], true);
    assert_eq!(records[1]["expected_cached"], 100);
    // 第三輪隔了一小時（手改 epoch 減 3600）＝追平，換上新素材。假 CLI 照樣回報中了 90，
    // 快取軸就照數字說 hit——「過期」是數字掉下來時用來解釋為什麼，不是時間到就蓋章
    assert_eq!(records[2]["cache"], "hit");
    assert_eq!(records[2]["rebased"], true);
    assert!(records[2]["age_secs"].as_u64().unwrap() >= 3_600);
    assert_ne!(records[2]["system_hash"], records[1]["system_hash"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// 中止後 session 檔刪不掉：線照清、usage 記下原因，settle_abort 回 Err。
/// run_turn 用 `?` 接這個結果，所以不會再回報中止成功。NotFound 仍當已刪。
#[test]
fn abort_delete_failure_clears_lane_and_returns_error() {
    let dir = std::env::temp_dir().join(format!(
        "tt-lanes-abandon-{}-{}",
        std::process::id(),
        ulid::Ulid::generate()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let working_dir = dir.join("ws");
    let claude_home = dir.join("claude-home");
    std::fs::create_dir_all(&working_dir).unwrap();
    let session_id = "sid-blocked";
    // 路徑是目錄，remove_file 會失敗，而且不是 NotFound。
    let session_path = session_file::session_file_path(&claude_home, &working_dir, session_id);
    std::fs::create_dir_all(&session_path).unwrap();
    let usage_log = dir.join("usage.jsonl");
    let world_dir = dir
        .join("root")
        .join("worlds")
        .join(ulid::Ulid::generate().to_string());
    mark_playable(&world_dir);
    let store_path = world_dir.join("lanes.json");
    let key = "chars:sonnet";
    let mut store = LaneStore::new();
    store.insert(key.to_owned(), lane_state(&[], 0));
    let call = LaneCall {
        provider: LaneProvider::Claude,
        program: dir.join("unused"),
        working_dir: working_dir.clone(),
        envs: Vec::new(),
        model: Some("sonnet".to_owned()),
        usage_log: Some(usage_log.clone()),
        claude_home: claude_home.clone(),
    };
    let error = settle_abort(
        &call,
        "world-1",
        key,
        session_id,
        Some("機密"),
        None,
        &mut store,
        &store_path,
    )
    .expect_err("刪不掉 session 不能當成中止成功");
    assert!(
        error.starts_with(r#"TTMSG:{"code":"session_abandon_failed""#),
        "{error}"
    );
    assert!(store.get(key).is_none());
    assert!(read_store(&store_path).get(key).is_none());
    assert!(session_path.exists(), "刪失敗時檔還在");
    let log = std::fs::read_to_string(&usage_log).unwrap();
    assert!(log.contains("rewrite-failed"), "{log}");
    assert!(log.contains("session_abandon_failed"), "{log}");

    store.insert(key.to_owned(), lane_state(&[], 0));
    settle_abort(
        &call,
        "world-1",
        key,
        "sid-missing",
        Some("機密"),
        None,
        &mut store,
        &store_path,
    )
    .expect("檔案本來就不在，棄用算成功");
    assert!(store.get(key).is_none());
    assert!(read_store(&store_path).get(key).is_none());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
async fn process_dead_within(pid: u32, budget: std::time::Duration) -> bool {
    let step = std::time::Duration::from_millis(20);
    let mut waited = std::time::Duration::ZERO;
    loop {
        let alive = std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);
        if !alive {
            return true;
        }
        if waited >= budget {
            return false;
        }
        tokio::time::sleep(step).await;
        waited += step;
    }
}

/// 假 CLI 把 session 寫完就睡。取消要等它真的退出，再抹掉私設；expected_reply 維持 None。
#[cfg(unix)]
#[tokio::test]
async fn lane_cancel_waits_for_exit_and_does_not_record_expected_reply() {
    let _serial = crate::inflight::lock_real_process_tests();
    let dir = std::env::temp_dir().join(format!(
        "tt-lanes-abort-{}-{}",
        std::process::id(),
        ulid::Ulid::generate()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let working_dir = dir.join("ws");
    let claude_home = dir.join("claude-home");
    let root = dir.join("root");
    let world_id = ulid::Ulid::generate().to_string();
    std::fs::create_dir_all(&working_dir).unwrap();
    mark_playable(&root.join("worlds").join(&world_id));
    let session_dir = session_file::session_file_path(&claude_home, &working_dir, "probe")
        .parent()
        .unwrap()
        .to_path_buf();
    std::fs::create_dir_all(&session_dir).unwrap();
    let script = dir.join("fake-claude.py");
    std::fs::write(
        &script,
        r#"#!/usr/bin/env python3
import json, sys, os, uuid, time
args = sys.argv[1:]
def flag(name):
    return args[args.index(name) + 1] if name in args else None
sid = flag('--session-id')
prompt = sys.stdin.read()
d = os.environ['FAKE_SESSION_DIR']
path = os.path.join(d, sid + '.jsonl')
u, a = str(uuid.uuid4()), str(uuid.uuid4())
lines = [
    {'type': 'user', 'uuid': u, 'parentUuid': None,
     'message': {'role': 'user', 'content': prompt}},
    {'type': 'assistant', 'uuid': a, 'parentUuid': u,
     'message': {'role': 'assistant', 'content': [{'type': 'text', 'text': '半截'}]}},
]
with open(path, 'w') as f:
    for o in lines:
        f.write(json.dumps(o, ensure_ascii=False) + '\n')
    f.flush()
    os.fsync(f.fileno())
open(os.path.join(d, 'ready'), 'w').close()
time.sleep(60)
"#,
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let call = LaneCall {
        provider: LaneProvider::Claude,
        program: script,
        working_dir: working_dir.clone(),
        envs: vec![(
            "FAKE_SESSION_DIR".to_owned(),
            session_dir.to_string_lossy().into_owned(),
        )],
        model: Some("sonnet".to_owned()),
        usage_log: None,
        claude_home: claude_home.clone(),
    };
    let world_for_abort = world_id.clone();
    let (guard, mut cancel) = crate::inflight::register_turn(&world_for_abort, "turn-1");
    let before = crate::inflight::child_pids();
    let handle = tokio::spawn(async move {
        let _guard = guard;
        let confidential = "## 「狐狸」的私有設定\n其實是通緝犯\n".to_owned();
        let events = vec![event(TranscriptKind::Player, "", "阿濤", "老闆晚安")];
        let mut input = turn_input(&events, 0);
        input.tail = format!("{confidential}\n現在你是「狐狸」。");
        input.confidential = Some(confidential);
        run_turn(&call, &root, &world_id, input, Some(&mut cancel), |_| {}).await
    });

    let step = std::time::Duration::from_millis(20);
    let mut waited = std::time::Duration::ZERO;
    let (pid, session_path) = loop {
        let ready = session_dir.join("ready").exists();
        let pid = crate::inflight::child_pids()
            .into_iter()
            .find(|pid| !before.contains(pid));
        let session_path = std::fs::read_dir(&session_dir).ok().and_then(|entries| {
            entries.filter_map(|entry| entry.ok()).find_map(|entry| {
                let path = entry.path();
                (path.extension().and_then(|ext| ext.to_str()) == Some("jsonl")).then_some(path)
            })
        });
        if ready {
            if let (Some(pid), Some(session_path)) = (pid, session_path) {
                break (pid, session_path);
            }
        }
        assert!(
            waited < std::time::Duration::from_secs(5),
            "等假 CLI 寫完 session 逾時"
        );
        tokio::time::sleep(step).await;
        waited += step;
    };

    crate::inflight::abort_turn(&world_for_abort, "turn-1");
    let outcome = handle
        .await
        .expect("背景 task 不該 panic")
        .expect("中止不是錯誤");
    assert!(outcome.aborted);
    assert!(
        process_dead_within(pid, std::time::Duration::from_secs(2)).await,
        "取消後子程序應在 2 秒內退出"
    );

    let store_path = data::lanes_path(&dir.join("root"), &world_for_abort).unwrap();
    match read_store(&store_path).get("chars:sonnet") {
        Some(state) => {
            assert!(state.expected_reply.is_none(), "中止不更新 expected_reply");
            assert!(state.pending_rewrite.is_some(), "中止留下 pending_rewrite");
            let rewritten = std::fs::read_to_string(&session_path).unwrap();
            assert!(!rewritten.contains("通緝犯"), "私設應已從 session 抹掉");
        }
        None => {
            assert!(
                !session_path.exists(),
                "抹寫失敗時應刪掉 session 檔並清掉 lane 記錄"
            );
        }
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

fn legacy_arrival() -> TranscriptEvent {
    event(
        TranscriptKind::System,
        "",
        "GM",
        "（角色回歸）〈騎士〉\n公開設定：\n王國騎士\n私有設定：\n奉密令而來",
    )
}

fn card_private_event() -> TranscriptEvent {
    let mut private = event(
        TranscriptKind::System,
        "",
        "GM",
        "（角色私設）〈騎士〉\n私有設定：\n奉密令而來",
    );
    private.gm_only = true;
    private
}

/// card-arrival-private-leak 遷移：舊版合併回歸事件已送進角色線 session 才重開一次，三家都一樣；
/// 新版本寫入後不再重開；只在未送段的舊事件增量遮掉即可；GM 線不因此重開。
#[test]
fn chars_lane_reopens_once_when_legacy_arrival_was_already_sent() {
    let events = [
        event(TranscriptKind::Player, "", "阿濤", "你好"),
        legacy_arrival(),
        event(TranscriptKind::Dialogue, "fox-id", "狐狸", "晚安"),
    ];
    let input = turn_input(&events, 0);
    let mut old = lane_state(&events, 0);
    old.redaction = 0;
    for provider in [LaneProvider::Claude, LaneProvider::Grok, LaneProvider::Agy] {
        let mut state = old.clone();
        state.provider = provider.as_str().to_owned();
        assert!(matches!(
            plan_turn(Some(&state), &input, 1_010, provider),
            TurnPlan::Reopen {
                reason: ReopenReason::HistoryRedacted
            }
        ));
        state.redaction = REDACTION_VERSION;
        assert!(matches!(
            plan_turn(Some(&state), &input, 1_010, provider),
            TurnPlan::Resume { base: 3, .. }
        ));
    }

    let mut gm_input = turn_input(&events, 0);
    gm_input.lane = Lane::Gm;
    assert!(matches!(
        plan_turn(Some(&old), &gm_input, 1_010, LaneProvider::Claude),
        TurnPlan::Resume { base: 3, .. }
    ));

    let mut unsent = lane_state(&events[..1], 0);
    unsent.redaction = 0;
    assert!(matches!(
        plan_turn(Some(&unsent), &input, 1_010, LaneProvider::Claude),
        TurnPlan::Resume { base: 1, .. }
    ));
}

/// 角色線渲染略過私設事件但不動索引：增量夾著、位在尾端、整段都被略過都不留空行；
/// 回覆對點照原事件序列走。GM 線照送全文。
#[test]
fn chars_lane_skips_card_private_event_without_shifting_watermark() {
    let events = [
        event(TranscriptKind::Player, "", "阿濤", "你好"),
        card_private_event(),
        event(TranscriptKind::Narration, "", "GM", "騎士推門進來"),
        card_private_event(),
    ];
    let full = build_prompt(&events, 0, "尾段", true, Lane::Chars);
    assert_eq!(
        full,
        "以下是到目前為止的對話紀錄：\n\n阿濤：你好\n\n（旁白）騎士推門進來\n\n——\n尾段"
    );
    assert_eq!(build_prompt(&events, 3, "尾段", false, Lane::Chars), "尾段");
    assert!(build_prompt(&events, 0, "尾段", true, Lane::Gm).contains("奉密令而來"));

    let reply = event(TranscriptKind::Dialogue, "fox-id", "狐狸", "晚安");
    let events = [
        event(TranscriptKind::Player, "", "阿濤", "你好"),
        reply.clone(),
        card_private_event(),
    ];
    let mut state = lane_state(&events[..1], 0);
    state.expected_reply = Some(ExpectedReply {
        speaker_id: reply.speaker_id.clone(),
        kind: reply.kind.clone(),
        text: reply.text.clone(),
    });
    let input = turn_input(&events, 0);
    match plan_turn(Some(&state), &input, 1_010, LaneProvider::Claude) {
        TurnPlan::Resume { base, .. } => {
            assert_eq!(base, 2);
            assert_eq!(
                build_prompt(&events, base, "尾段", false, Lane::Chars),
                "尾段"
            );
        }
        TurnPlan::Reopen { .. } => panic!("回覆對得上就續聊"),
    }
}
