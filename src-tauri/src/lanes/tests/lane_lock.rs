//! 每桌 lane 鎖：同桌兩次 run_turn（不同線）不得交錯讀寫 lanes.json。

use super::*;

/// 在假 CLI 外包一道閘：第 n 次呼叫一啟動就寫 `started-n`，等測試放出 `release-n` 才往下跑；
/// 有 `fail-n` 就直接以非零碼結束。每次進出都記進 order.log，用來證明兩通沒有交錯。
#[cfg(unix)]
fn gate(call: &mut LaneCall, gate_dir: &Path) {
    let wrapper = gate_dir.join("gate.py");
    std::fs::write(
        &wrapper,
        format!(
            r#"#!/usr/bin/env python3
import os, subprocess, sys, time
d = {gate:?}
data = sys.stdin.read()
log = os.path.join(d, 'order.log')
with open(log, 'a') as f:
    f.write('start\n')
n = sum(1 for line in open(log) if line.startswith('start'))
open(os.path.join(d, 'started-%d' % n), 'w').close()
while not os.path.exists(os.path.join(d, 'release-%d' % n)):
    time.sleep(0.01)
if os.path.exists(os.path.join(d, 'fail-%d' % n)):
    code = 7
else:
    code = subprocess.run([{inner:?}] + sys.argv[1:], input=data, text=True).returncode
with open(log, 'a') as f:
    f.write('end\n')
sys.exit(code)
"#,
            gate = gate_dir.to_string_lossy(),
            inner = call.program.to_string_lossy()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    call.program = wrapper;
}

async fn wait_for(path: &Path) {
    let step = std::time::Duration::from_millis(10);
    let mut waited = std::time::Duration::ZERO;
    while !path.exists() {
        assert!(
            waited < std::time::Duration::from_secs(20),
            "等不到 {path:?}"
        );
        tokio::time::sleep(step).await;
        waited += step;
    }
}

fn touch(path: PathBuf) {
    std::fs::write(path, "").unwrap();
}

fn gm_input(events: &[TranscriptEvent]) -> TurnInput<'_> {
    TurnInput {
        lane: Lane::Gm,
        prefix: None,
        echo: ReplyEcho::Narration,
        ..turn_input(events, 0)
    }
}

#[cfg(unix)]
#[tokio::test]
async fn same_table_turns_serialize_and_lock_survives_failure_and_abort() {
    let _serial = crate::inflight::lock_real_process_tests();
    let FakeCli {
        dir,
        mut call,
        root,
        world_id,
        session_dir,
        ..
    } = fake_claude("lane-lock");
    let gate_dir = dir.join("gate");
    std::fs::create_dir_all(&gate_dir).unwrap();
    gate(&mut call, &gate_dir);
    let store_path = data::lanes_path(&root, &world_id).unwrap();
    let events = vec![event(TranscriptKind::Player, "", "阿濤", "老闆晚安")];

    // A（角色線）卡在閘裡時 B（GM 線）也出發：B 必須等 A 整輪落檔才拿得到鎖
    let a = run_turn(
        &call,
        &root,
        &world_id,
        turn_input(&events, 0),
        None,
        |_| {},
    );
    let b = async {
        wait_for(&gate_dir.join("started-1")).await;
        run_turn(&call, &root, &world_id, gm_input(&events), None, |_| {}).await
    };
    let control = async {
        wait_for(&gate_dir.join("started-1")).await;
        for _ in 0..20 {
            tokio::task::yield_now().await;
        }
        assert!(
            lane_lock(&store_path).try_lock().is_err(),
            "A 跑 CLI 時要握著鎖"
        );
        assert!(
            !gate_dir.join("started-2").exists(),
            "B 不得在 A 收尾前啟動 CLI"
        );
        touch(gate_dir.join("release-1"));
        wait_for(&gate_dir.join("started-2")).await;
        touch(gate_dir.join("release-2"));
    };
    let (a, b, ()) = tokio::join!(a, b, control);
    a.unwrap();
    b.unwrap();
    assert_eq!(
        std::fs::read_to_string(gate_dir.join("order.log")).unwrap(),
        "start\nend\nstart\nend\n"
    );

    // 兩條線都在，各自的 session id 與水位指紋都對得上自己那通
    let opened: Vec<String> = std::fs::read_to_string(session_dir.join("calls.jsonl"))
        .unwrap()
        .lines()
        .map(|line| {
            let call: serde_json::Value = serde_json::from_str(line).unwrap();
            let args: Vec<String> = serde_json::from_value(call["args"].clone()).unwrap();
            let at = args.iter().position(|arg| arg == "--session-id").unwrap();
            args[at + 1].clone()
        })
        .collect();
    let store = read_store(&store_path);
    assert_eq!(store["chars:sonnet"].session_id, opened[0]);
    assert_eq!(store["gm:sonnet"].session_id, opened[1]);
    for key in ["chars:sonnet", "gm:sonnet"] {
        assert_eq!(store[key].sent_hash, events_fingerprint(&events));
        assert!(store[key].pending_rewrite.is_none());
    }

    // 第 3 通（上輪回覆不在水位上＝直接重開）失敗：整輪回錯，鎖要放得掉
    touch(gate_dir.join("fail-3"));
    touch(gate_dir.join("release-3"));
    let more = vec![
        events[0].clone(),
        event(TranscriptKind::Player, "", "阿濤", "再來一杯"),
    ];
    assert!(
        run_turn(&call, &root, &world_id, turn_input(&more, 0), None, |_| {})
            .await
            .is_err()
    );
    touch(gate_dir.join("release-4"));
    run_turn(&call, &root, &world_id, gm_input(&more), None, |_| {})
        .await
        .expect("失敗那輪之後同桌仍拿得到鎖");

    // 第 5 通卡在閘裡被中止：中止收場後鎖同樣放得掉
    let (guard, mut cancel) = crate::inflight::register_turn(&world_id, "lock-abort");
    let aborted = async {
        let _guard = guard;
        run_turn(
            &call,
            &root,
            &world_id,
            gm_input(&more),
            Some(&mut cancel),
            |_| {},
        )
        .await
    };
    let abort = async {
        wait_for(&gate_dir.join("started-5")).await;
        crate::inflight::abort_turn(&world_id, "lock-abort");
    };
    let (aborted, ()) = tokio::join!(aborted, abort);
    assert!(aborted.expect("中止不是錯誤").aborted);
    touch(gate_dir.join("release-6"));
    run_turn(&call, &root, &world_id, turn_input(&more, 0), None, |_| {})
        .await
        .expect("中止那輪之後同桌仍拿得到鎖");

    std::fs::remove_dir_all(&dir).unwrap();
}
