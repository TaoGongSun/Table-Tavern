//! 每桌 lane 鎖：保溫與回合不得交錯讀寫 lanes.json。

use super::*;

/// 保溫已讀 store、ping 還在跑 → 角色輪因正典改動要重開 → 保溫收尾。沒有鎖時保溫會把
/// 讀到的舊狀態整份寫回，已撤銷的 session id 復活；有鎖時回合等保溫結束才開始。
#[cfg(unix)]
#[tokio::test]
async fn keepalive_cannot_resurrect_a_lane_replaced_mid_ping() {
    let _serial = crate::inflight::lock_real_process_tests();
    let FakeCli {
        dir,
        mut call,
        root,
        world_id,
        session_dir,
        ..
    } = fake_claude("lock-ping");
    // 包一層：保溫訊息先睡一下，讓回合有機會插進來
    let wrapper = dir.join("slow-ping.py");
    std::fs::write(
        &wrapper,
        format!(
            r#"#!/usr/bin/env python3
import subprocess, sys, time
data = sys.stdin.read()
if '系統保溫訊息' in data:
    time.sleep(0.6)
sys.exit(subprocess.run([{inner:?}] + sys.argv[1:], input=data, text=True).returncode)
"#,
            inner = call.program.to_string_lossy()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    call.program = wrapper;

    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "老闆晚安")];
    run_turn(
        &call,
        &root,
        &world_id,
        turn_input(&events, 0),
        None,
        |_| {},
    )
    .await
    .unwrap();
    let store_path = data::lanes_path(&root, &world_id).unwrap();
    let old = read_store(&store_path)["chars:sonnet"].session_id.clone();
    set_lane_epoch(&store_path, now_epoch() - 200);

    // 已送段被改字 → 這輪必定重開成新 id
    events[0].text = "老闆早安".to_owned();
    let turn = async {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        run_turn(
            &call,
            &root,
            &world_id,
            turn_input(&events, 0),
            None,
            |_| {},
        )
        .await
    };
    let (pinged, turned) = tokio::join!(keepalive(&call, &root, &world_id), turn);
    assert_eq!(pinged.unwrap(), 1);
    turned.unwrap();

    let store = read_store(&store_path);
    let current = &store["chars:sonnet"].session_id;
    assert_ne!(current, &old, "舊 session id 不得被保溫寫回");
    let last_call = std::fs::read_to_string(session_dir.join("calls.jsonl")).unwrap();
    let last: serde_json::Value = serde_json::from_str(last_call.lines().last().unwrap()).unwrap();
    let args: Vec<&str> = last["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|arg| arg.as_str().unwrap())
        .collect();
    let open = args
        .iter()
        .position(|arg| *arg == "--session-id")
        .expect("最後一輪是重開");
    assert_eq!(args[open + 1], current);
    let _ = std::fs::remove_dir_all(&dir);
}
