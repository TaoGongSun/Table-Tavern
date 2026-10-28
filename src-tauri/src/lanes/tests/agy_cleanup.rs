//! agy 線的即時刪（agy-home-cleanup）：重開時舊對話落檔成功才刪、續聊失敗（含 mismatch 另開的新對話）
//! 也刪；家目錄只由 config_root 推出，本輪就是 agy 時 envs 的 HOME 要對得上。

use super::*;

const A: &str = "aaaaaaaa-0000-4000-8000-000000000001";
const B: &str = "bbbbbbbb-0000-4000-8000-000000000002";
const C: &str = "cccccccc-0000-4000-8000-000000000003";
const D: &str = "dddddddd-0000-4000-8000-000000000004";

struct FakeAgy {
    dir: PathBuf,
    call: LaneCall,
    root: PathBuf,
    world_id: String,
    home: PathBuf,
}

impl Drop for FakeAgy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// 假 agy：每次呼叫從 ids.txt 取下一個 conversation id 回報（續聊與否都照單）。
#[cfg(unix)]
fn fake_agy(tag: &str, ids: &[&str]) -> FakeAgy {
    let dir = std::env::temp_dir().join(format!(
        "tt-lanes-agy-clean-{tag}-{}",
        ulid::Ulid::generate()
    ));
    let working_dir = dir.join("ws");
    let root = dir.join("root");
    let config_root = dir.join("config");
    let home = config_root.join("agy-home");
    let world_id = ulid::Ulid::generate().to_string();
    std::fs::create_dir_all(&working_dir).unwrap();
    mark_playable(&root.join("worlds").join(&world_id));
    std::fs::write(dir.join("ids.txt"), ids.join("\n")).unwrap();
    let script = dir.join("fake-agy.py");
    std::fs::write(
        &script,
        r#"#!/usr/bin/env python3
import json, os, sys
d = os.environ['FAKE_AGY_DIR']
sys.stdin.read()
path = os.path.join(d, 'ids.txt')
ids = open(path).read().split('\n')
cid = ids[0]
open(path, 'w').write('\n'.join(ids[1:]))
print(json.dumps({'event': 'init', 'conversation_id': cid}))
print(json.dumps({'event': 'result', 'result': {
    'status': 'SUCCESS', 'response': '回覆', 'num_turns': 1,
    'usage': {'input_tokens': 10, 'output_tokens': 5, 'thinking_tokens': 0,
              'cache_read_tokens': 0, 'total_tokens': 15}}}, ensure_ascii=False))
"#,
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let call = LaneCall {
        provider: LaneProvider::Agy,
        program: script,
        working_dir: working_dir.clone(),
        prompt_dir: dir.join("prompts"),
        envs: vec![
            (
                "FAKE_AGY_DIR".to_owned(),
                dir.to_string_lossy().into_owned(),
            ),
            ("HOME".to_owned(), home.to_string_lossy().into_owned()),
        ],
        model: Some("gemini-test".to_owned()),
        // conversation id 經帳本那格回報：沒有帳本就拿不到 id
        usage_log: Some(dir.join("usage.jsonl")),
        claude_home: dir.join("unused-claude-home"),
        effort: None,
        config_root: Some(config_root),
        on_overage: None,
    };
    FakeAgy {
        dir,
        call,
        root,
        world_id,
        home,
    }
}

/// 在假家目錄放一組對話檔（agy 1.4.8 的形狀）。
fn plant(home: &Path, id: &str) {
    let base = home.join(".gemini/antigravity-cli");
    for (sub, name) in [
        ("conversations", format!("{id}.db")),
        ("conversations", format!("{id}.db-wal")),
        ("annotations", format!("{id}.pbtxt")),
    ] {
        std::fs::create_dir_all(base.join(sub)).unwrap();
        std::fs::write(base.join(sub).join(name), "x").unwrap();
    }
    std::fs::create_dir_all(base.join("brain").join(id).join("scratch")).unwrap();
}

fn planted(home: &Path, id: &str) -> bool {
    let base = home.join(".gemini/antigravity-cli");
    base.join("conversations").join(format!("{id}.db")).exists()
        || base.join("brain").join(id).exists()
}

fn input<'a>(events: &'a [TranscriptEvent], scene: u64) -> TurnInput<'a> {
    let mut input = turn_input(events, scene);
    input.prefix = None;
    input.scope = Some("fox-id".to_owned());
    input
}

fn stored(fake: &FakeAgy) -> String {
    let path = data::lanes_path(&fake.root, &fake.world_id).unwrap();
    read_store(&path)
        .values()
        .next()
        .unwrap()
        .session_id
        .clone()
}

#[cfg(unix)]
#[tokio::test]
async fn reopen_retires_old_conversation() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_agy("reopen", &[A, B]);
    plant(&fake.home, A);
    let events = vec![event(TranscriptKind::Player, "", "阿濤", "第一句")];
    run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        input(&events, 0),
        None,
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(stored(&fake), A);
    // 換幕重開：A 被取代
    plant(&fake.home, B);
    let events = vec![event(TranscriptKind::Player, "", "阿濤", "新的一幕")];
    run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        input(&events, 1),
        None,
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(stored(&fake), B);
    assert!(!planted(&fake.home, A), "被取代的對話要刪");
    assert!(planted(&fake.home, B), "現行對話不能刪");
}

/// 續聊時 agy 回了別的 id（mismatch）：另開的 C 與舊的 A 都刪，降級重開後的 D 留著。
#[cfg(unix)]
#[tokio::test]
async fn mismatch_discards_new_and_retires_old() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_agy("mismatch", &[A, C, D]);
    for id in [A, C, D] {
        plant(&fake.home, id);
    }
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第一句")];
    run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        input(&events, 0),
        None,
        |_| {},
    )
    .await
    .unwrap();
    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", "回覆"));
    events.push(event(TranscriptKind::Player, "", "阿濤", "第二句"));
    run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        input(&events, 0),
        None,
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(stored(&fake), D);
    assert!(!planted(&fake.home, A));
    assert!(!planted(&fake.home, C));
    assert!(planted(&fake.home, D));
}

/// 落檔失敗：retire 回錯、舊對話留著（磁碟上的 lanes.json 還指著它）。
#[cfg(unix)]
#[tokio::test]
async fn store_write_failure_keeps_old_conversation() {
    use std::os::unix::fs::PermissionsExt;
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_agy("readonly", &[A, B]);
    plant(&fake.home, A);
    let events = vec![event(TranscriptKind::Player, "", "阿濤", "第一句")];
    run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        input(&events, 0),
        None,
        |_| {},
    )
    .await
    .unwrap();
    let world = data::lanes_path(&fake.root, &fake.world_id).unwrap();
    std::fs::set_permissions(&world, std::fs::Permissions::from_mode(0o444)).unwrap();
    let events = vec![event(TranscriptKind::Player, "", "阿濤", "新的一幕")];
    let result = run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        input(&events, 1),
        None,
        |_| {},
    )
    .await;
    std::fs::set_permissions(&world, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(result.is_err());
    assert!(planted(&fake.home, A), "落檔失敗不能先刪");
}

/// 本輪就是 agy、envs 的 HOME 卻指向別處：不刪。
#[cfg(unix)]
#[tokio::test]
async fn foreign_home_env_blocks_deletion() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut fake = fake_agy("foreign", &[A, B]);
    plant(&fake.home, A);
    for (key, value) in fake.call.envs.iter_mut() {
        if key == "HOME" {
            *value = fake.dir.join("elsewhere").to_string_lossy().into_owned();
        }
    }
    let events = vec![event(TranscriptKind::Player, "", "阿濤", "第一句")];
    run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        input(&events, 0),
        None,
        |_| {},
    )
    .await
    .unwrap();
    let events = vec![event(TranscriptKind::Player, "", "阿濤", "新的一幕")];
    run_turn(
        &fake.call,
        &fake.root,
        &fake.world_id,
        input(&events, 1),
        None,
        |_| {},
    )
    .await
    .unwrap();
    assert!(planted(&fake.home, A));
}

/// 家目錄判定：撤別家的舊線不看本輪 envs；同一家時 envs 要對得上；沒有設定根就不刪。
#[test]
fn cleanup_home_by_provider() {
    let root = PathBuf::from("/cfg");
    let call = |provider, envs: Vec<(&str, &str)>, config_root: Option<PathBuf>| LaneCall {
        provider,
        program: PathBuf::new(),
        working_dir: PathBuf::new(),
        prompt_dir: PathBuf::new(),
        envs: envs
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect(),
        model: None,
        usage_log: None,
        claude_home: PathBuf::new(),
        effort: None,
        config_root,
        on_overage: None,
    };
    // agy→grok：本輪是 grok（沒有 HOME 指向 agy-home），撤舊 agy 線照刪
    let grok = call(
        LaneProvider::Grok,
        vec![("HOME", "/cfg/cli-home"), ("GROK_HOME", "/cfg/grok-home")],
        Some(root.clone()),
    );
    assert_eq!(
        cleanup::cleanup_home(&grok, LaneProvider::Agy).unwrap(),
        PathBuf::from("/cfg/agy-home")
    );
    assert_eq!(
        cleanup::cleanup_home(&grok, LaneProvider::Grok).unwrap(),
        PathBuf::from("/cfg/grok-home")
    );
    // grok→agy：本輪 agy，撤舊 grok 線不看 GROK_HOME（envs 沒有）
    let agy = call(
        LaneProvider::Agy,
        vec![("HOME", "/cfg/agy-home")],
        Some(root.clone()),
    );
    assert!(cleanup::cleanup_home(&agy, LaneProvider::Grok).is_ok());
    assert!(cleanup::cleanup_home(&agy, LaneProvider::Agy).is_ok());
    // 同一家但 envs 指向別處或缺少：不刪
    let stray = call(
        LaneProvider::Agy,
        vec![("HOME", "/elsewhere")],
        Some(root.clone()),
    );
    assert!(cleanup::cleanup_home(&stray, LaneProvider::Agy).is_err());
    let bare = call(LaneProvider::Grok, vec![], Some(root));
    assert!(cleanup::cleanup_home(&bare, LaneProvider::Grok).is_err());
    let rootless = call(
        LaneProvider::Grok,
        vec![("GROK_HOME", "/cfg/grok-home")],
        None,
    );
    assert!(cleanup::cleanup_home(&rootless, LaneProvider::Agy).is_err());
}
