//! grok 角色共線端到端（假 grok CLI 照 1.0.46 的 session 目錄落檔）。
//! 假 CLI 的 reasoning 摘要會複述整段 prompt——真 grok 的摘要就會明文寫出私設。

use super::*;
use serde_json::Value;

struct FakeGrok {
    dir: PathBuf,
    call: LaneCall,
    root: PathBuf,
    world_id: String,
    grok_home: PathBuf,
}

impl Drop for FakeGrok {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[cfg(unix)]
fn fake_grok(tag: &str, extra_env: &[(&str, &str)]) -> FakeGrok {
    let dir = std::env::temp_dir().join(format!("tt-lanes-grok-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let working_dir = dir.join("ws");
    let grok_home = dir.join("grok-home");
    let root = dir.join("root");
    let world_id = ulid::Ulid::generate().to_string();
    std::fs::create_dir_all(&working_dir).unwrap();
    mark_playable(&root.join("worlds").join(&world_id));
    let script = dir.join("fake-grok.py");
    std::fs::write(
        &script,
        r#"#!/usr/bin/env python3
import json, os, sys, time
args = sys.argv[1:]
def flag(name):
    return args[args.index(name) + 1] if name in args else None
def read(path):
    return open(path, encoding='utf-8').read() if path else ''
sid, rid, prompt = flag('-s'), flag('-r'), read(flag('--prompt-file'))
home = os.environ['GROK_HOME']
log = os.path.join(os.environ['FAKE_DIR'], 'calls.jsonl')
start = time.time()
time.sleep(float(os.environ.get('FAKE_SLEEP', '0')))
d = os.path.join(home, 'sessions', '%2Fws', sid or rid)
def record():
    with open(log, 'a') as f:
        f.write(json.dumps({'args': args, 'prompt': prompt, 'profile': read(flag('--agent')) if flag('--agent') else None,
                            'start': start, 'end': time.time()}, ensure_ascii=False) + '\n')
if rid and not os.path.isdir(d):
    record()
    sys.exit(3)
if sid and os.path.isdir(d):
    sys.exit(4)
os.makedirs(d, exist_ok=True)
ch, up = os.path.join(d, 'chat_history.jsonl'), os.path.join(d, 'updates.jsonl')
def append(path, rows):
    with open(path, 'a') as f:
        for r in rows:
            f.write(json.dumps(r, ensure_ascii=False, separators=(',', ':')) + '\n')
def upd(kind, text=None, **extra):
    u = {'sessionUpdate': kind, **extra}
    if text is not None:
        u['content'] = {'type': 'text', 'text': text}
    return {'timestamp': 1, 'method': 'session/update', 'params': {'sessionId': sid or rid, 'update': u}}
if sid:
    append(ch, [{'type': 'system', 'content': read(flag('--agent'))},
                {'type': 'user', 'content': [{'type': 'text', 'text': '<user_info>\nOS\n</user_info>'}]}])
    for name in ['chat_history.jsonl.lock', 'updates.jsonl.lock', 'summary.json.lock']:
        open(os.path.join(d, name), 'a').close()
    with open(os.path.join(d, 'signals.json'), 'w') as f:
        f.write('{"compactionCount":0}')
else:
    append(up, [upd('background_tasks', tasks=[])])
n = sum(1 for l in open(ch) if '"prompt_index"' in l)
reply = os.environ.get('FAKE_REPLY_PREFIX', '') + '回覆' + str(n + 1)
summary = '正在想：' + prompt
tool = os.environ.get('FAKE_TOOL') == '1'
# 真 CLI：帶 --verbatim 原文照收，不帶才包 <user_query>（FAKE_WRAP 強制包，模擬舊行為）
wrap = '--verbatim' not in args or os.environ.get('FAKE_WRAP') == '1'
body = '<user_query>\n' + prompt + '\n</user_query>' if wrap else prompt
append(ch, [{'type': 'user', 'content': [{'type': 'text', 'text': body}], 'prompt_index': n},
            {'type': 'reasoning', 'id': 'rs', 'summary': [{'type': 'summary_text', 'text': summary}], 'encrypted_content': 'x'}]
           + ([{'type': 'backend_tool_call', 'kind': {'tool_type': 'x_search'}}] if tool else [])
           + [{'type': 'assistant', 'content': reply}])
append(up, [upd('user_message_chunk', prompt), upd('agent_thought_chunk', summary),
            upd('agent_message_chunk', reply), upd('turn_completed', stop_reason='end_turn')])
lock = os.environ.get('FAKE_LOCK_STORE')
if lock:
    os.chmod(os.path.join(lock, 'lanes.json'), 0o444)
    os.chmod(lock, 0o555)
record()
if rid and os.environ.get('FAKE_RUNAWAY_RESUME') == '1':
    while True:
        print(json.dumps({'type': 'text', 'data': ' '}), flush=True)
print(json.dumps({'type': 'text', 'data': reply}, ensure_ascii=False))
print(json.dumps({'type': 'end', 'usage': {'input_tokens': 100, 'cache_read_input_tokens': 128, 'output_tokens': 5}}))
"#,
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut envs = vec![
        (
            "GROK_HOME".to_owned(),
            grok_home.to_string_lossy().into_owned(),
        ),
        ("FAKE_DIR".to_owned(), dir.to_string_lossy().into_owned()),
    ];
    envs.extend(
        extra_env
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned())),
    );
    let call = LaneCall {
        provider: LaneProvider::Grok,
        program: script,
        working_dir,
        envs,
        model: Some("grok-4.6".to_owned()),
        usage_log: Some(dir.join("usage.jsonl")),
        claude_home: PathBuf::new(),
        prompt_dir: dir.join("prompts"),
        on_overage: None,
    };
    FakeGrok {
        dir,
        call,
        root,
        world_id,
        grok_home,
    }
}

struct Call {
    args: Vec<String>,
    prompt: String,
    /// `--agent` 指向的 profile 檔內容；沒帶就是 None
    profile: Option<String>,
    start: f64,
    end: f64,
}

fn calls(dir: &Path) -> Vec<Call> {
    std::fs::read_to_string(dir.join("calls.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| {
            let value: Value = serde_json::from_str(line).unwrap();
            Call {
                args: value["args"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|arg| arg.as_str().unwrap().to_owned())
                    .collect(),
                prompt: value["prompt"].as_str().unwrap().to_owned(),
                profile: value["profile"].as_str().map(str::to_owned),
                start: value["start"].as_f64().unwrap(),
                end: value["end"].as_f64().unwrap(),
            }
        })
        .collect()
}

fn flag(call: &Call, name: &str) -> Option<String> {
    call.args
        .iter()
        .position(|arg| arg == name)
        .map(|index| call.args[index + 1].clone())
}

/// grok-home 底下所有檔案的文字（含 .lock、暫存目錄）；機密段不含跳脫字元，直接比字串。
fn all_text(home: &Path) -> String {
    let mut out = String::new();
    let mut stack = vec![home.to_path_buf()];
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            for entry in std::fs::read_dir(&path).unwrap() {
                stack.push(entry.unwrap().path());
            }
        } else {
            out.push_str(&String::from_utf8_lossy(&std::fs::read(&path).unwrap()));
        }
    }
    out
}

fn char_turn<'a>(
    events: &'a [TranscriptEvent],
    name: &str,
    id: &str,
    secret: Option<&str>,
) -> TurnInput<'a> {
    let mut input = turn_input(events, 0);
    let confidential = secret.map(|secret| format!("## 「{name}」的私有設定\n{secret}\n"));
    input.tail = format!(
        "{}現在你是「{name}」。",
        confidential.clone().unwrap_or_default()
    );
    input.confidential = confidential;
    input.prefix = Some(format!("{name}："));
    input.echo = ReplyEcho::Dialogue {
        speaker_id: id.to_owned(),
        prefix: format!("{name}："),
    };
    input
}

fn session_dirs(home: &Path) -> Vec<PathBuf> {
    let group = home.join("sessions").join("%2Fws");
    std::fs::read_dir(group)
        .map(|entries| entries.map(|entry| entry.unwrap().path()).collect())
        .unwrap_or_default()
}

/// 兩個角色輪流共用一條線：私設每輪送、回合後兩檔都抹掉（含 reasoning 複述），
/// 下一個角色的 prompt 與 session 都看不到；只改私設不重開；cached 128 不算失敗。
#[cfg(unix)]
#[tokio::test]
async fn two_characters_share_one_grok_lane_without_residue() {
    shared_lane_without_residue("shared", &[]).await;
}

/// 同上，但 user 本文帶 `<user_query>` 包裝（不帶 --verbatim 的 CLI 行為）：剝掉包裝再比對。
#[cfg(unix)]
#[tokio::test]
async fn shared_lane_rewrite_also_accepts_user_query_wrapping() {
    shared_lane_without_residue("shared-wrap", &[("FAKE_WRAP", "1")]).await;
}

#[cfg(unix)]
async fn shared_lane_without_residue(tag: &str, extra_env: &[(&str, &str)]) {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_grok(tag, extra_env);
    let FakeGrok {
        call,
        root,
        world_id,
        grok_home,
        dir,
        ..
    } = &fake;
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "老闆晚安")];

    let fox1 = run_turn(
        call,
        root,
        world_id,
        char_turn(&events, "狐狸", "fox-id", Some("狐狸是通緝犯")),
        None,
        |_| {},
    )
    .await
    .unwrap()
    .text;
    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", &fox1));
    let knight = run_turn(
        call,
        root,
        world_id,
        char_turn(&events, "騎士", "knight-id", Some("騎士欠賭債")),
        None,
        |_| {},
    )
    .await
    .unwrap()
    .text;
    events.push(event(
        TranscriptKind::Dialogue,
        "knight-id",
        "騎士",
        &knight,
    ));
    // 只改了狐狸的私設：凍結 system 沒變，照樣續聊
    run_turn(
        call,
        root,
        world_id,
        char_turn(&events, "狐狸", "fox-id", Some("狐狸其實是公主")),
        None,
        |_| {},
    )
    .await
    .unwrap();

    let calls = calls(dir);
    assert_eq!(calls.len(), 3);
    let session = flag(&calls[0], "-s").expect("第一輪開線");
    assert_eq!(flag(&calls[1], "-r").as_deref(), Some(session.as_str()));
    assert_eq!(flag(&calls[2], "-r").as_deref(), Some(session.as_str()));
    assert!(!calls[1].prompt.contains("通緝犯"));
    let store = read_store(&data::lanes_path(root, world_id).unwrap());
    assert_eq!(store.keys().collect::<Vec<_>>(), ["chars:grok-4.6"]);
    assert!(store["chars:grok-4.6"].pending_rewrite.is_none());

    let text = all_text(grok_home);
    for secret in ["通緝犯", "賭債", "公主"] {
        assert!(!text.contains(secret), "{secret} 留在 grok-home");
    }
    assert!(!text.contains("agent_thought_chunk") && !text.contains("\"reasoning\""));
    assert!(text.contains("狐狸：回覆1") && text.contains("騎士：回覆2"));
}

/// GM 線一律原文：不抹、不拿 reasoning、不補前綴。
#[cfg(unix)]
#[tokio::test]
async fn grok_gm_lane_files_are_left_untouched() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_grok("gm", &[]);
    let events = vec![event(TranscriptKind::Player, "", "阿濤", "老闆晚安")];
    let mut input = turn_input(&events, 0);
    input.lane = Lane::Gm;
    input.prefix = None;
    input.tail = "## 狐狸私設\n狐狸是通緝犯\n旁白".to_owned();
    input.echo = ReplyEcho::Narration;
    run_turn(&fake.call, &fake.root, &fake.world_id, input, None, |_| {})
        .await
        .unwrap();
    let text = all_text(&fake.grok_home);
    assert!(text.contains("通緝犯") && text.contains("agent_thought_chunk"));
    assert!(!text.contains("狐狸：回覆1"));
    assert!(
        read_store(&data::lanes_path(&fake.root, &fake.world_id).unwrap())
            .contains_key("gm:grok-4.6")
    );
}

/// 抹寫遇到工具呼叫：本輪回覆照送，線撤銷（store 拿掉、目錄刪掉），下一輪開新線。
#[cfg(unix)]
#[tokio::test]
async fn unexpected_turn_shape_revokes_lane_and_next_turn_reopens() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut fake = fake_grok("tool", &[("FAKE_TOOL", "1")]);
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "老闆晚安")];
    let reply = run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        char_turn(&events, "狐狸", "fox-id", Some("狐狸是通緝犯")),
        None,
        |_| {},
    )
    .await
    .unwrap()
    .text;
    assert_eq!(reply, "回覆1");
    let store_path = data::lanes_path(&fake.root, &fake.world_id).unwrap();
    assert!(read_store(&store_path).is_empty());
    assert!(
        session_dirs(&fake.grok_home).is_empty(),
        "撤銷的線目錄要刪掉"
    );
    let log = std::fs::read_to_string(fake.dir.join("usage.jsonl")).unwrap();
    let drops: Vec<serde_json::Value> = log
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .filter(|row: &serde_json::Value| row["event"] == "drop-lane")
        .collect();
    assert_eq!(drops.len(), 1, "{log}");
    // 丟線落帳記實際 provider、固定的 reason，壞在哪步與原因另欄，路徑遮掉
    assert_eq!(drops[0]["transport"], "grok");
    assert_eq!(drops[0]["reason"], "rewrite-failed");
    assert_eq!(drops[0]["stage"], "rewrite");
    let detail = drops[0]["detail"].as_str().unwrap();
    assert!(!detail.is_empty());
    assert!(
        !detail.contains(&*fake.grok_home.to_string_lossy()),
        "{detail}"
    );

    fake.call.envs.retain(|(key, _)| key != "FAKE_TOOL");
    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", &reply));
    run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        char_turn(&events, "狐狸", "fox-id", Some("狐狸是通緝犯")),
        None,
        |_| {},
    )
    .await
    .unwrap();
    let calls = calls(&fake.dir);
    assert!(flag(&calls[1], "-s").is_some(), "下一輪重開");
    assert_ne!(flag(&calls[0], "-s"), flag(&calls[1], "-s"));
}

/// 回合完成後 store 落檔失敗：呼叫前寫下的 pending 留在磁碟，下一輪只能重開，舊目錄撤銷。
#[cfg(unix)]
#[tokio::test]
async fn store_write_failure_leaves_pending_and_next_turn_revokes_old_session() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut fake = fake_grok("store-fail", &[]);
    let world_dir = fake.root.join("worlds").join(&fake.world_id);
    fake.call.envs.push((
        "FAKE_LOCK_STORE".to_owned(),
        world_dir.to_string_lossy().into_owned(),
    ));
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "老闆晚安")];
    let result = run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        char_turn(&events, "狐狸", "fox-id", Some("狐狸是通緝犯")),
        None,
        |_| {},
    )
    .await;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&world_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::set_permissions(
        world_dir.join("lanes.json"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(result.is_err(), "最終落檔失敗要回錯");
    let store_path = data::lanes_path(&fake.root, &fake.world_id).unwrap();
    assert!(read_store(&store_path)["chars:grok-4.6"]
        .pending_rewrite
        .is_some());
    let old = flag(&calls(&fake.dir)[0], "-s").unwrap();

    fake.call.envs.retain(|(key, _)| key != "FAKE_LOCK_STORE");
    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", "回覆1"));
    run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        char_turn(&events, "狐狸", "fox-id", None),
        None,
        |_| {},
    )
    .await
    .unwrap();
    let calls = calls(&fake.dir);
    let new = flag(&calls[1], "-s").expect("pending 只能重開");
    assert_ne!(new, old);
    let dirs = session_dirs(&fake.grok_home);
    assert_eq!(dirs.len(), 1);
    assert!(dirs[0].ends_with(&new), "舊目錄已撤銷");
    // 預定重開不是出事：不記丟線，原因在呼叫行的 reopen
    let log = std::fs::read_to_string(fake.dir.join("usage.jsonl")).unwrap_or_default();
    assert!(!log.contains("drop-lane"), "{log}");
}

/// 續聊失敗（session 目錄不見）：先撤銷舊線再降級重開，本輪照樣有回覆。
#[cfg(unix)]
#[tokio::test]
async fn resume_failure_revokes_then_reopens() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_grok("resume-fail", &[]);
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "老闆晚安")];
    let reply = run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        char_turn(&events, "狐狸", "fox-id", None),
        None,
        |_| {},
    )
    .await
    .unwrap()
    .text;
    for dir in session_dirs(&fake.grok_home) {
        std::fs::remove_dir_all(dir).unwrap();
    }
    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", &reply));
    run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        char_turn(&events, "騎士", "knight-id", None),
        None,
        |_| {},
    )
    .await
    .unwrap();
    let calls = calls(&fake.dir);
    assert!(flag(&calls[1], "-r").is_some());
    assert!(flag(&calls[2], "-s").is_some());
    let log = std::fs::read_to_string(fake.dir.join("usage.jsonl")).unwrap();
    assert!(log.contains("resume-failed"));
}

/// 續聊中輸出失控：撤線、只派送一次、回錯誤碼；下一輪開新線。
#[cfg(unix)]
#[tokio::test]
async fn resume_runaway_revokes_lane_without_retry() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_grok("runaway", &[("FAKE_RUNAWAY_RESUME", "1")]);
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "老闆晚安")];
    let reply = run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        char_turn(&events, "狐狸", "fox-id", None),
        None,
        |_| {},
    )
    .await
    .unwrap()
    .text;
    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", &reply));
    events.push(event(TranscriptKind::Player, "", "阿濤", "來一杯麥酒"));
    let error = run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        char_turn(&events, "狐狸", "fox-id", Some("其實是通緝犯")),
        None,
        |_| {},
    )
    .await
    .unwrap_err();
    assert!(error.starts_with("AI_OUTPUT_RUNAWAY:"), "{error}");
    let sent = calls(&fake.dir);
    assert_eq!(sent.len(), 2, "失控不得重開重試");
    assert!(flag(&sent[1], "-r").is_some());
    let store = read_store(&data::lanes_path(&fake.root, &fake.world_id).unwrap());
    assert!(store.is_empty(), "grok 失控比照中止整條撤線");
    assert!(session_dirs(&fake.grok_home).is_empty());
    assert!(!all_text(&fake.grok_home).contains("通緝犯"));
}

/// 同桌並發：GM＋角色、角色＋角色都被每桌鎖串行——CLI 呼叫不重疊、store 不互蓋。
#[cfg(unix)]
#[tokio::test]
async fn concurrent_turns_on_one_table_run_one_at_a_time() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_grok("race", &[("FAKE_SLEEP", "0.3")]);
    let events = vec![event(TranscriptKind::Player, "", "阿濤", "老闆晚安")];
    let mut gm = turn_input(&events, 0);
    gm.lane = Lane::Gm;
    gm.prefix = None;
    gm.echo = ReplyEcho::Narration;
    let (gm_result, fox_result) = tokio::join!(
        run_turn(&fake.call, &fake.root, &fake.world_id, gm, None, |_| {}),
        run_turn(
            &fake.call,
            &fake.root,
            &fake.world_id,
            char_turn(&events, "狐狸", "fox-id", Some("狐狸是通緝犯")),
            None,
            |_| {}
        ),
    );
    gm_result.unwrap();
    fox_result.unwrap();
    let store_path = data::lanes_path(&fake.root, &fake.world_id).unwrap();
    let store = read_store(&store_path);
    assert!(store.contains_key("gm:grok-4.6") && store.contains_key("chars:grok-4.6"));

    // 兩個角色同時被點：後到的那個等前一輪落檔，續同一條線
    let mut events = events.clone();
    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", "回覆1"));
    let (fox, knight) = tokio::join!(
        run_turn(
            &fake.call,
            &fake.root,
            &fake.world_id,
            char_turn(&events, "狐狸", "fox-id", Some("狐狸是通緝犯")),
            None,
            |_| {}
        ),
        run_turn(
            &fake.call,
            &fake.root,
            &fake.world_id,
            char_turn(&events, "騎士", "knight-id", Some("騎士欠賭債")),
            None,
            |_| {}
        ),
    );
    fox.unwrap();
    knight.unwrap();

    let calls = calls(&fake.dir);
    assert_eq!(calls.len(), 4);
    let mut spans: Vec<(f64, f64)> = calls.iter().map(|call| (call.start, call.end)).collect();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    for pair in spans.windows(2) {
        assert!(pair[0].1 <= pair[1].0, "CLI 呼叫重疊：{pair:?}");
    }
    let store = read_store(&store_path);
    assert!(store.values().all(|state| state.pending_rewrite.is_none()));
    assert_eq!(store.len(), 2);
    let text = all_text(&fake.grok_home);
    assert!(!text.contains("通緝犯") && !text.contains("賭債"));
}

/// 開線把 system 寫成 agent profile（內容就是 grok_payload 的 profile）、續聊不帶；
/// 正文都從 --prompt-file 讀到、不走 -p，文字通道帶 --verbatim。暫存檔呼叫完就刪。
#[cfg(unix)]
#[tokio::test]
async fn grok_lane_opens_with_profile_file_and_resumes_with_prompt_file_only() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_grok("files", &[]);
    let FakeGrok {
        call,
        root,
        world_id,
        dir,
        ..
    } = &fake;
    let frozen = "凍結A ${% raw %} 保持原樣";
    fn gm_turn<'a>(events: &'a [TranscriptEvent], frozen: &str) -> TurnInput<'a> {
        let mut input = turn_input(events, 0);
        input.lane = Lane::Gm;
        input.prefix = None;
        input.echo = ReplyEcho::Narration;
        input.frozen_system = frozen.to_owned();
        input
    }
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第一句")];
    let reply = run_turn(call, root, world_id, gm_turn(&events, frozen), None, |_| {})
        .await
        .unwrap()
        .text;
    events.push(event(TranscriptKind::Narration, "", "", &reply));
    events.push(event(TranscriptKind::Player, "", "阿濤", "只有這句是新的"));
    run_turn(call, root, world_id, gm_turn(&events, frozen), None, |_| {})
        .await
        .unwrap();

    let calls = calls(dir);
    assert_eq!(calls.len(), 2);
    let open = &calls[0];
    assert!(flag(open, "-s").is_some());
    assert!(open.args.contains(&"--verbatim".to_owned()));
    assert!(!open
        .args
        .iter()
        .any(|arg| arg.contains("凍結A") || arg == "-p"));
    assert_eq!(
        open.profile.as_deref(),
        crate::cli::grok_payload(frozen, "", false).0.as_deref()
    );
    assert!(open.prompt.contains("第一句"));
    assert!(!open.prompt.contains("凍結A")); // system 只在 profile，不混進正文

    let resume = &calls[1];
    assert_eq!(flag(resume, "-r"), flag(open, "-s"));
    assert!(!resume.args.contains(&"--agent".to_owned()));
    assert!(resume.profile.is_none());
    assert!(resume.prompt.contains("只有這句是新的"));
    assert!(!resume.prompt.contains("第一句"));
    assert!(!resume.prompt.contains("凍結A"));

    let left: Vec<_> = std::fs::read_dir(&call.prompt_dir)
        .map(|entries| entries.collect())
        .unwrap_or_default();
    assert!(left.is_empty(), "暫存檔沒刪：{left:?}");
}

/// 真 grok CLI（手動跑，會花一點額度）：照 app 環境與共線旗標（帶 --verbatim），同一角色
/// 帶私設講兩輪——第一輪 -s 開線、第二輪 -r 續聊；兩輪都抹寫成功、線沒被拆、真 session 檔
/// 裡沒有機密段與 reasoning。
/// `TT_GROK_CONFIG_ROOT=<設定根（含 cli-home、grok-home、cli-workspace）> cargo test --lib real_grok_shared_lane -- --ignored`
#[cfg(unix)]
#[tokio::test]
#[ignore]
async fn real_grok_shared_lane_rewrites_verbatim_sessions() {
    let _serial = crate::inflight::lock_real_process_tests();
    let config_root = PathBuf::from(std::env::var("TT_GROK_CONFIG_ROOT").unwrap());
    let grok_home = config_root.join("grok-home");
    let dir = std::env::temp_dir().join(format!("tt-real-grok-{}", ulid::Ulid::generate()));
    let root = dir.join("root");
    let world_id = ulid::Ulid::generate().to_string();
    mark_playable(&root.join("worlds").join(&world_id));
    let call = LaneCall {
        provider: LaneProvider::Grok,
        program: PathBuf::from(std::env::var("HOME").unwrap()).join(".grok/bin/grok"),
        working_dir: config_root.join("cli-workspace"),
        envs: crate::cli::grok_envs(&config_root.join("cli-home"), &grok_home),
        model: Some("grok-4.5".to_owned()),
        usage_log: Some(dir.join("usage.jsonl")),
        claude_home: PathBuf::new(),
        prompt_dir: dir.join("prompts"),
        on_overage: None,
    };
    let secret = "狐狸其實是失蹤的公主ZQX17";
    let mut events = vec![event(
        TranscriptKind::Player,
        "",
        "阿濤",
        "老闆晚安，今晚有什麼推薦？",
    )];
    let first = run_turn(
        &call,
        &root,
        &world_id,
        char_turn(&events, "狐狸", "fox-id", Some(secret)),
        None,
        |_| {},
    )
    .await
    .unwrap()
    .text;
    let key = "chars:grok-4.5";
    let opened = read_store(&data::lanes_path(&root, &world_id).unwrap())[key]
        .session_id
        .clone();
    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", &first));
    events.push(event(TranscriptKind::Player, "", "阿濤", "那就來一杯吧。"));
    let second = run_turn(
        &call,
        &root,
        &world_id,
        char_turn(&events, "狐狸", "fox-id", Some(secret)),
        None,
        |_| {},
    )
    .await
    .unwrap()
    .text;
    assert!(!second.trim().is_empty());
    let store = read_store(&data::lanes_path(&root, &world_id).unwrap());
    assert_eq!(store[key].session_id, opened, "續聊後線被換掉");
    assert!(store[key].pending_rewrite.is_none());
    let log = std::fs::read_to_string(dir.join("usage.jsonl")).unwrap_or_default();
    assert!(!log.contains("drop"), "{log}");
    let dirs = crate::lanes::grok_session::find_session_dirs(&grok_home, &opened).unwrap();
    assert_eq!(dirs.len(), 1);
    let text = all_text(&dirs[0]);
    assert!(!text.contains("ZQX17"), "機密段留在 session");
    assert!(!text.contains("\"reasoning\"") && !text.contains("agent_thought_chunk"));
    assert!(!text.contains("<user_query>"), "verbatim 不該有包裝");
    assert!(text.contains("那就來一杯吧"));
    eprintln!("session {opened}\n1: {first}\n2: {second}");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// char-line-prefix：模型自加正典「狐狸：」時抹寫照過（不撤線、不雙前綴），
/// 逐字稿存剝餘的下一輪續聊；私設照抹。
#[cfg(unix)]
#[tokio::test]
async fn self_prefixed_reply_keeps_grok_lane_and_resumes() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_grok("own-prefix", &[("FAKE_REPLY_PREFIX", "狐狸：")]);
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "老闆晚安")];
    let first = run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        char_turn(&events, "狐狸", "fox-id", Some("狐狸是通緝犯")),
        None,
        |_| {},
    )
    .await
    .unwrap()
    .text;
    assert_eq!(first, "狐狸：回覆1");
    let text = all_text(&fake.grok_home);
    assert!(text.contains("狐狸：回覆1"));
    assert!(!text.contains("狐狸：狐狸："));
    assert!(!text.contains("通緝犯"));
    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", "回覆1"));
    events.push(event(TranscriptKind::Player, "", "阿濤", "來杯麥酒"));
    run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        char_turn(&events, "狐狸", "fox-id", Some("狐狸是通緝犯")),
        None,
        |_| {},
    )
    .await
    .unwrap();
    let calls = calls(&fake.dir);
    assert_eq!(calls.len(), 2);
    assert!(flag(&calls[1], "-r").is_some());
    assert!(!all_text(&fake.grok_home).contains("通緝犯"));
}
