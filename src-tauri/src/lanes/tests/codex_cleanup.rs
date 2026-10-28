//! codex 線的即時刪（agy-home-cleanup 包 2）：重開與續聊失敗時舊 thread 落檔成功才交給 `codex delete`；
//! mismatch 另開的新 thread、非正常結束丟掉的 thread 也刪；本輪是 codex 時 CODEX_HOME 要對得上。

use super::*;

const A: &str = "01a18237-6c98-7783-9d89-ca15d7ebe9b8";
const C: &str = "01a18237-8e13-70b3-82ad-48b0f02ad690";
const D: &str = "01a18237-9f00-7000-8000-000000000004";

struct FakeCodex {
    dir: PathBuf,
    call: LaneCall,
    root: PathBuf,
    world_id: String,
}

impl Drop for FakeCodex {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// 假 codex：開線依序吐 `ids` 裡的 thread id；`resume` 時有 `reject` 檔就回 no rollout found、
/// 有 `swap` 檔就吐下一個 id（mismatch）；有 `hang` 檔就吐完 thread id 後卡住；`delete <id> --force` 記下 id 與當下 lanes.json 是否還指著它。
#[cfg(unix)]
fn fake_codex(tag: &str, ids: &[&str]) -> FakeCodex {
    let dir =
        std::env::temp_dir().join(format!("tt-lanes-codex-clean-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let working_dir = dir.join("ws");
    let root = dir.join("root");
    let config_root = dir.join("cfg");
    let home = config_root.join("codex-home");
    let world_id = ulid::Ulid::generate().to_string();
    std::fs::create_dir_all(&working_dir).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    mark_playable(&root.join("worlds").join(&world_id));
    let lanes = data::lanes_path(&root, &world_id).unwrap();
    std::fs::write(dir.join("ids"), ids.join("\n")).unwrap();
    // delete 子程序只拿到 CODEX_HOME（其餘環境繼承 app），假 codex 從自己所在的資料夾找設定
    std::fs::write(dir.join("lanes-path"), lanes.to_string_lossy().as_bytes()).unwrap();
    let script = dir.join("fake-codex.py");
    std::fs::write(
        &script,
        r#"#!/usr/bin/env python3
import json, os, sys, time
args = sys.argv[1:]
d = os.path.dirname(os.path.abspath(__file__))
if args[0] == 'delete':
    path = open(os.path.join(d, 'lanes-path')).read()
    lanes = open(path).read() if os.path.exists(path) else ''
    with open(os.path.join(d, 'deletes.jsonl'), 'a') as f:
        f.write(json.dumps({'id': args[1], 'stored': args[1] in lanes, 'home': os.environ.get('CODEX_HOME')}) + '\n')
    sys.exit(0)
sys.stdin.read()
def next_id():
    ids = open(os.path.join(d, 'ids')).read().split()
    n = int(open(os.path.join(d, 'n')).read()) if os.path.exists(os.path.join(d, 'n')) else 0
    open(os.path.join(d, 'n'), 'w').write(str(n + 1))
    return ids[n]
tid = args[args.index('resume') + 1] if 'resume' in args else None
if tid and os.path.exists(os.path.join(d, 'reject')):
    print(json.dumps({'type': 'error', 'message': 'thread/resume failed: no rollout found'}))
    sys.exit(1)
if tid is None or os.path.exists(os.path.join(d, 'swap')):
    tid = next_id()
print(json.dumps({'type': 'thread.started', 'thread_id': tid}))
if os.path.exists(os.path.join(d, 'hang')):
    sys.stdout.flush()
    time.sleep(30)
print(json.dumps({'type': 'item.completed', 'item': {'id': 'i', 'type': 'agent_message', 'text': '回覆'}}, ensure_ascii=False))
print(json.dumps({'type': 'turn.completed', 'usage': {'input_tokens': 10, 'cached_input_tokens': 0, 'cache_write_input_tokens': 0, 'output_tokens': 5}}))
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
        envs: vec![("CODEX_HOME".to_owned(), home.to_string_lossy().into_owned())],
        model: None,
        usage_log: None,
        claude_home: dir.join("unused-claude-home"),
        effort: Some("low".to_owned()),
        config_root: Some(config_root),
        on_overage: None,
    };
    FakeCodex {
        dir,
        call,
        root,
        world_id,
    }
}

fn input<'a>(events: &'a [TranscriptEvent], scene: u64) -> TurnInput<'a> {
    let mut input = turn_input(events, scene);
    input.prefix = None;
    input.scope = Some("fox-id".to_owned());
    input
}

/// （刪掉的 id, 刪時 lanes.json 是否還指著它）
fn deletes(fake: &FakeCodex) -> Vec<(String, bool)> {
    std::fs::read_to_string(fake.dir.join("deletes.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            (
                value["id"].as_str().unwrap().to_owned(),
                value["stored"].as_bool().unwrap(),
            )
        })
        .collect()
}

fn stored(fake: &FakeCodex) -> Option<String> {
    let path = data::lanes_path(&fake.root, &fake.world_id).unwrap();
    read_store(&path)
        .values()
        .next()
        .map(|state| state.session_id.clone())
}

async fn turn(fake: &FakeCodex, events: &[TranscriptEvent], scene: u64) -> Result<(), String> {
    run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        input(events, scene),
        None,
        |_| {},
    )
    .await
    .map(|_| ())
}

fn second_turn() -> Vec<TranscriptEvent> {
    vec![
        event(TranscriptKind::Player, "", "阿濤", "第一句"),
        event(TranscriptKind::Dialogue, "fox-id", "狐狸", "回覆"),
        event(TranscriptKind::Player, "", "阿濤", "第二句"),
    ]
}

/// 換幕重開：舊 thread 先從 lanes.json 拿掉、才交給 codex delete；CODEX_HOME 是 app 的 codex-home。
#[cfg(unix)]
#[tokio::test]
async fn reopen_retires_old_thread_after_store_write() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_codex("reopen", &[A, C]);
    let events = vec![event(TranscriptKind::Player, "", "阿濤", "第一句")];
    turn(&fake, &events, 0).await.unwrap();
    assert_eq!(stored(&fake).as_deref(), Some(A));
    let events = vec![event(TranscriptKind::Player, "", "阿濤", "新的一幕")];
    turn(&fake, &events, 1).await.unwrap();
    assert_eq!(stored(&fake).as_deref(), Some(C));
    assert_eq!(deletes(&fake), vec![(A.to_owned(), false)]);
}

/// 續聊失敗（no rollout found）：舊 thread 撤掉並刪，重開的新 thread 留著。
#[cfg(unix)]
#[tokio::test]
async fn resume_failure_retires_old_thread() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_codex("reject", &[A, C]);
    let events = vec![event(TranscriptKind::Player, "", "阿濤", "第一句")];
    turn(&fake, &events, 0).await.unwrap();
    std::fs::write(fake.dir.join("reject"), "").unwrap();
    turn(&fake, &second_turn(), 0).await.unwrap();
    assert_eq!(stored(&fake).as_deref(), Some(C));
    assert_eq!(deletes(&fake), vec![(A.to_owned(), false)]);
}

/// 續聊時 codex 回了別的 thread（mismatch）：另開的 C 與舊的 A 都刪，降級重開後的 D 留著。
#[cfg(unix)]
#[tokio::test]
async fn mismatch_discards_new_and_retires_old() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_codex("mismatch", &[A, C, D]);
    let events = vec![event(TranscriptKind::Player, "", "阿濤", "第一句")];
    turn(&fake, &events, 0).await.unwrap();
    std::fs::write(fake.dir.join("swap"), "").unwrap();
    turn(&fake, &second_turn(), 0).await.unwrap();
    assert_eq!(stored(&fake).as_deref(), Some(D));
    assert_eq!(
        deletes(&fake),
        vec![(C.to_owned(), false), (A.to_owned(), false)]
    );
}

/// 收到完成卻非正常結束：正文照交、線丟掉，這條 thread 也刪。
#[cfg(unix)]
#[tokio::test]
async fn unclean_exit_deletes_dropped_thread() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut fake = fake_codex("unclean", &[A]);
    fake.call
        .envs
        .push(("FAKE_EXIT".to_owned(), "3".to_owned()));
    let events = vec![event(TranscriptKind::Player, "", "阿濤", "第一句")];
    turn(&fake, &events, 0).await.unwrap();
    assert_eq!(stored(&fake), None);
    assert_eq!(deletes(&fake), vec![(A.to_owned(), false)]);
}

/// 落檔失敗：retire 回錯、舊 thread 不刪（磁碟上的 lanes.json 還指著它）。
#[cfg(unix)]
#[tokio::test]
async fn store_write_failure_keeps_old_thread() {
    use std::os::unix::fs::PermissionsExt;
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_codex("readonly", &[A, C]);
    let events = vec![event(TranscriptKind::Player, "", "阿濤", "第一句")];
    turn(&fake, &events, 0).await.unwrap();
    let lanes = data::lanes_path(&fake.root, &fake.world_id).unwrap();
    std::fs::set_permissions(&lanes, std::fs::Permissions::from_mode(0o444)).unwrap();
    let events = vec![event(TranscriptKind::Player, "", "阿濤", "新的一幕")];
    let result = turn(&fake, &events, 1).await;
    std::fs::set_permissions(&lanes, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(result.is_err());
    assert!(deletes(&fake).is_empty(), "落檔失敗不能先刪");
}

/// 本輪就是 codex、envs 的 CODEX_HOME 卻指向別處：不刪。
#[cfg(unix)]
#[tokio::test]
async fn foreign_codex_home_blocks_deletion() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut fake = fake_codex("foreign", &[A, C]);
    let elsewhere = fake.dir.join("elsewhere");
    for (key, value) in fake.call.envs.iter_mut() {
        if key == "CODEX_HOME" {
            *value = elsewhere.to_string_lossy().into_owned();
        }
    }
    let events = vec![event(TranscriptKind::Player, "", "阿濤", "第一句")];
    turn(&fake, &events, 0).await.unwrap();
    let events = vec![event(TranscriptKind::Player, "", "阿濤", "新的一幕")];
    turn(&fake, &events, 1).await.unwrap();
    assert!(deletes(&fake).is_empty());
}

/// 開線途中玩家按停止：codex 已建好的新 thread 從沒進 store，當場刪。
#[cfg(unix)]
#[tokio::test]
async fn abort_while_opening_discards_new_thread() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_codex("abort", &[A]);
    std::fs::write(fake.dir.join("hang"), "").unwrap();
    let (_guard, mut signal) =
        crate::inflight::register(crate::inflight::Kind::Chat, &fake.world_id);
    let world_id = fake.world_id.clone();
    let stopper = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        crate::inflight::abort_kind(crate::inflight::Kind::Chat, &world_id);
    });
    let events = vec![event(TranscriptKind::Player, "", "阿濤", "第一句")];
    let outcome = run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        input(&events, 0),
        Some(&mut signal),
        |_| {},
    )
    .await
    .unwrap();
    stopper.await.unwrap();
    assert!(outcome.aborted);
    assert_eq!(deletes(&fake), vec![(A.to_owned(), false)]);
}
