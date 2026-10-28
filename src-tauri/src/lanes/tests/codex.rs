//! codex lane（exec resume）：一角一線、thread id 由 CLI 產生、累計用量相減、effort 換了重開、
//! 收到完成卻自己非正常結束時正文只交付一次且下一輪不續用；完成後被寬限期殺掉的照常續用。

use super::*;

/// 假 codex：開線吐新 thread id，`resume <id>` 只接受 codex-thread-1；turn.completed 回累計用量。
#[cfg(unix)]
fn fake_codex(tag: &str) -> FakeCli {
    let dir = std::env::temp_dir().join(format!("tt-lanes-codex-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let working_dir = dir.join("ws");
    let root = dir.join("root");
    let world_id = ulid::Ulid::generate().to_string();
    std::fs::create_dir_all(&working_dir).unwrap();
    mark_playable(&root.join("worlds").join(&world_id));
    let script = dir.join("fake-codex.py");
    std::fs::write(
        &script,
        r#"#!/usr/bin/env python3
import json, os, sys, time
args = sys.argv[1:]
prompt = sys.stdin.read()
d = os.environ['FAKE_CODEX_DIR']
calls = os.path.join(d, 'calls.jsonl')
with open(calls, 'a') as f:
    f.write(json.dumps({'args': args, 'prompt': prompt}, ensure_ascii=False) + '\n')
n = sum(1 for _ in open(calls))
tid = args[args.index('resume') + 1] if 'resume' in args else None
if tid and tid != 'codex-thread-1':
    print(json.dumps({'type': 'error', 'message': 'thread/resume failed: no rollout found'}))
    sys.exit(1)
turns = int(open(os.path.join(d, 'turns')).read()) if tid and os.path.exists(os.path.join(d, 'turns')) else 0
turns += 1
open(os.path.join(d, 'turns'), 'w').write(str(turns))
print(json.dumps({'type': 'thread.started', 'thread_id': tid or 'codex-thread-1'}))
print(json.dumps({'type': 'item.completed', 'item': {'id': 'i', 'type': 'agent_message', 'text': '回覆' + str(n)}}, ensure_ascii=False))
print(json.dumps({'type': 'turn.completed', 'usage': {'input_tokens': 1000 * turns, 'cached_input_tokens': 600 * turns - 600, 'cache_write_input_tokens': 0, 'output_tokens': 50 * turns}}))
if os.environ.get('FAKE_HANG_AFTER_DONE') == '1':
    sys.stdout.flush()
    time.sleep(60)
sys.exit(int(os.environ.get('FAKE_EXIT', '0')))
"#,
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let call = LaneCall {
        provider: LaneProvider::Codex,
        program: script,
        working_dir: working_dir.clone(),
        prompt_dir: working_dir.with_file_name("prompts"),
        envs: vec![(
            "FAKE_CODEX_DIR".to_owned(),
            dir.to_string_lossy().into_owned(),
        )],
        model: None,
        usage_log: Some(dir.join("usage.jsonl")),
        claude_home: dir.join("unused-claude-home"),
        config_root: None,
        on_overage: None,
        effort: Some("low".to_owned()),
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

fn codex_input(events: &[TranscriptEvent]) -> TurnInput<'_> {
    let mut input = turn_input(events, 0);
    input.prefix = None;
    input.scope = Some("fox-id".to_owned());
    input
}

fn calls(dir: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(dir.join("calls.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn has_resume(call: &serde_json::Value) -> bool {
    call["args"]
        .as_array()
        .unwrap()
        .iter()
        .any(|arg| arg == "resume")
}

fn only_state(root: &Path, world_id: &str) -> Option<LaneState> {
    let store = read_store(&data::lanes_path(root, world_id).unwrap());
    store.into_values().next()
}

#[test]
fn codex_chars_lane_is_one_lane_per_card_without_erase() {
    assert_eq!(
        chars_lane_shape(LaneProvider::Codex, false),
        chars_lane_shape(LaneProvider::Agy, false)
    );
}

#[cfg(unix)]
#[tokio::test]
async fn codex_lane_refuses_confidential_injection() {
    let FakeCli {
        dir: _dir,
        call,
        root,
        world_id,
        ..
    } = fake_codex("guard");
    let events = [event(TranscriptKind::Player, "", "阿濤", "你好")];
    let mut input = codex_input(&events);
    input.confidential = Some("狐狸的私設".to_owned());
    let error = run_turn(&call, &root, &world_id, input, None, |_: &str| {})
        .await
        .expect_err("帶機密段的 codex 線必須被擋下");
    assert!(error.starts_with(r#"TTMSG:{"code":"lane_rewrite_unsupported""#));
}

#[cfg(unix)]
#[tokio::test]
async fn codex_lane_resumes_thread_with_delta_only_and_subtracts_usage() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake_codex("resume");
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第一句")];
    let first = run_turn(
        &fx.call,
        &fx.root,
        &fx.world_id,
        codex_input(&events),
        None,
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(first.text, "回覆1\n");
    let state = only_state(&fx.root, &fx.world_id).unwrap();
    assert_eq!(state.session_id, "codex-thread-1");
    assert_eq!(state.effort.as_deref(), Some("low"));
    assert_eq!(state.codex_usage.map(|u| u.input_tokens), Some(1000));

    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", "回覆1"));
    events.push(event(TranscriptKind::Player, "", "阿濤", "只有這句是新的"));
    run_turn(
        &fx.call,
        &fx.root,
        &fx.world_id,
        codex_input(&events),
        None,
        |_| {},
    )
    .await
    .unwrap();

    let calls = calls(&fx.dir);
    assert_eq!(calls.len(), 2);
    assert!(!has_resume(&calls[0]));
    assert!(calls[0]["prompt"].as_str().unwrap().contains("凍結A"));
    assert!(has_resume(&calls[1]));
    let args = calls[1]["args"].as_array().unwrap();
    let at = |needle: &str| args.iter().position(|arg| arg == needle).unwrap();
    assert_eq!(args[at("resume") + 1], "codex-thread-1");
    assert!(at("-s") < at("resume"));
    let resumed = calls[1]["prompt"].as_str().unwrap();
    assert!(!resumed.contains("凍結A"));
    assert!(!resumed.contains("第一句"));
    assert!(resumed.contains("只有這句是新的"));
    // 帳本：第二輪是累計 2000 減上一輪 1000
    let usage = std::fs::read_to_string(fx.dir.join("usage.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = usage
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows[1]["prompt_tokens"], 1000);
    assert_eq!(rows[1]["cached_tokens"], 600);
    assert_eq!(
        only_state(&fx.root, &fx.world_id)
            .unwrap()
            .codex_usage
            .map(|u| u.input_tokens),
        Some(2000)
    );
}

#[cfg(unix)]
#[tokio::test]
async fn codex_effort_change_reopens_and_same_effort_resumes() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut fx = fake_codex("effort");
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第一句")];
    run_turn(
        &fx.call,
        &fx.root,
        &fx.world_id,
        codex_input(&events),
        None,
        |_| {},
    )
    .await
    .unwrap();
    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", "回覆1"));
    events.push(event(TranscriptKind::Player, "", "阿濤", "第二句"));
    run_turn(
        &fx.call,
        &fx.root,
        &fx.world_id,
        codex_input(&events),
        None,
        |_| {},
    )
    .await
    .unwrap();
    assert!(has_resume(&calls(&fx.dir)[1]), "effort 相同就續用");

    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", "回覆2"));
    events.push(event(TranscriptKind::Player, "", "阿濤", "第三句"));
    fx.call.effort = Some("high".to_owned());
    run_turn(
        &fx.call,
        &fx.root,
        &fx.world_id,
        codex_input(&events),
        None,
        |_| {},
    )
    .await
    .unwrap();
    let calls = calls(&fx.dir);
    assert!(!has_resume(&calls[2]), "effort 不同就重開");
    assert!(calls[2]["prompt"].as_str().unwrap().contains("第一句"));
    let state = only_state(&fx.root, &fx.world_id).unwrap();
    assert_eq!(state.effort.as_deref(), Some("high"));
    // 重開後基準換成新線第一輪的累計值
    assert_eq!(state.codex_usage.map(|u| u.input_tokens), Some(1000));
}

#[cfg(unix)]
#[tokio::test]
async fn codex_unknown_thread_reopens_once() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake_codex("bad-id");
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第一句")];
    run_turn(
        &fx.call,
        &fx.root,
        &fx.world_id,
        codex_input(&events),
        None,
        |_| {},
    )
    .await
    .unwrap();
    // 把線上的 thread id 換成假的：resume 失敗一次，重開後回話正常
    let store_path = data::lanes_path(&fx.root, &fx.world_id).unwrap();
    let mut store = read_store(&store_path);
    for state in store.values_mut() {
        state.session_id = "codex-thread-gone".to_owned();
    }
    write_store(&store_path, &store).unwrap();
    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", "回覆1"));
    events.push(event(TranscriptKind::Player, "", "阿濤", "第二句"));
    let outcome = run_turn(
        &fx.call,
        &fx.root,
        &fx.world_id,
        codex_input(&events),
        None,
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(outcome.text, "回覆3\n");
    let calls = calls(&fx.dir);
    assert_eq!(calls.len(), 3);
    assert!(has_resume(&calls[1]));
    assert!(!has_resume(&calls[2]));
    assert_eq!(
        only_state(&fx.root, &fx.world_id).unwrap().session_id,
        "codex-thread-1"
    );
}

/// 收到 turn.completed 但程序非 0 結束：正文只交付一次（不重派送），這條線不續用。
#[cfg(unix)]
#[tokio::test]
async fn codex_completed_with_unclean_exit_delivers_once_and_drops_lane() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut fx = fake_codex("unclean");
    fx.call.envs.push(("FAKE_EXIT".to_owned(), "3".to_owned()));
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第一句")];
    let mut streamed = String::new();
    let outcome = run_turn(
        &fx.call,
        &fx.root,
        &fx.world_id,
        codex_input(&events),
        None,
        |delta| streamed.push_str(delta),
    )
    .await
    .unwrap();
    assert_eq!(outcome.text, "回覆1\n");
    assert_eq!(streamed, "回覆1\n");
    assert_eq!(calls(&fx.dir).len(), 1, "不重派送");
    assert!(only_state(&fx.root, &fx.world_id).is_none(), "線已丟");

    fx.call.envs.retain(|(key, _)| key != "FAKE_EXIT");
    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", "回覆1"));
    events.push(event(TranscriptKind::Player, "", "阿濤", "第二句"));
    run_turn(
        &fx.call,
        &fx.root,
        &fx.world_id,
        codex_input(&events),
        None,
        |_| {},
    )
    .await
    .unwrap();
    assert!(!has_resume(&calls(&fx.dir)[1]), "下一輪重開、不續用舊線");
}

/// 收到 turn.completed 後卡住、被 app 的寬限期殺掉：不是 CLI 自己沒收好，這條線下一輪照常續用。
#[cfg(unix)]
#[tokio::test]
async fn codex_completed_then_killed_by_grace_keeps_lane() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut fx = fake_codex("grace");
    fx.call
        .envs
        .push(("FAKE_HANG_AFTER_DONE".to_owned(), "1".to_owned()));
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第一句")];
    let outcome = run_turn(
        &fx.call,
        &fx.root,
        &fx.world_id,
        codex_input(&events),
        None,
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(outcome.text, "回覆1\n");
    assert!(only_state(&fx.root, &fx.world_id).is_some(), "線要留著");

    fx.call
        .envs
        .retain(|(key, _)| key != "FAKE_HANG_AFTER_DONE");
    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", "回覆1"));
    events.push(event(TranscriptKind::Player, "", "阿濤", "第二句"));
    run_turn(
        &fx.call,
        &fx.root,
        &fx.world_id,
        codex_input(&events),
        None,
        |_| {},
    )
    .await
    .unwrap();
    let calls = calls(&fx.dir);
    assert_eq!(calls.len(), 2);
    assert!(has_resume(&calls[1]), "下一輪續用同一條線");
}
