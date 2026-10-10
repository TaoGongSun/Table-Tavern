//! 匯入、撤銷、貼開場白整段持整桌獨占：在資料寫入點停住一方，驗另一方的寫入進不來、放開後兩邊的
//! 世界書、逐字稿、狀態都對；以及貼開場白部分追加後失敗的回復。
use super::*;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

fn book(title: &str, opening: &str) -> Vec<u8> {
    serde_json::json!({
        "spec": "chara_card_v3",
        "data": {
            "name": title,
            "first_mes": opening,
            "character_book": {"name": title, "entries": [
                {"keys": [title], "content": format!("{title}設定"), "comment": title, "enabled": true}
            ]}
        }
    })
    .to_string()
    .into_bytes()
}

fn pending_markers(root: &Path, world_id: &str) -> usize {
    fs::read_dir(root.join("worlds").join(world_id))
        .unwrap()
        .filter(|entry| {
            entry
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("import-pending-")
        })
        .count()
}

fn world_file(root: &Path, world_id: &str, name: &str) -> PathBuf {
    root.join("worlds").join(world_id).join(name)
}

/// 正式 command 的取鎖方式：排隊等整桌獨占
fn wait_exclusive(world_id: &str) -> data::WorldExclusive {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(data::world_exclusive_async(world_id))
        .unwrap()
}

/// 先讓 `first` 在 `path` 的第一次寫入前停住，再放 `second` 跑：`first` 放開前 `second` 不得完成；放開後
/// 兩邊各自完成。
fn interleave<A, B>(path: PathBuf, first: A, second: B)
where
    A: FnOnce() + Send + 'static,
    B: FnOnce() + Send + 'static,
{
    let writes = Arc::new(AtomicU32::new(0));
    let second_done = Arc::new(AtomicBool::new(false));
    let (paused_tx, paused_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let paused_tx = Mutex::new(paused_tx);
    let release_rx = Mutex::new(release_rx);
    let _hook = {
        let writes = writes.clone();
        data::write_hook::install(path, move || {
            if writes.fetch_add(1, Ordering::SeqCst) == 0 {
                paused_tx.lock().unwrap().send(()).unwrap();
                release_rx.lock().unwrap().recv().unwrap();
            }
        })
    };
    let first = std::thread::spawn(first);
    paused_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let second = {
        let second_done = second_done.clone();
        std::thread::spawn(move || {
            second();
            second_done.store(true, Ordering::SeqCst);
        })
    };
    std::thread::sleep(Duration::from_millis(200));
    assert!(
        !second_done.load(Ordering::SeqCst),
        "先動手的一方還在寫入中，另一方的寫入不該完成"
    );
    release_tx.send(()).unwrap();
    first.join().unwrap();
    second.join().unwrap();
}

/// 一般聊天寫入的取鎖方式：共用許可＋追加一則
fn chat_write(root: PathBuf, world_id: String, ts: &'static str) -> impl FnOnce() + Send {
    move || {
        let _permit = data::world_write_permit(&world_id).unwrap();
        data::append_transcript(&root, &world_id, 0, &transcript_event(ts, "聊天的一句")).unwrap();
    }
}

fn transcript_ts(root: &Path, world_id: &str) -> Vec<String> {
    data::read_transcript(root, world_id, 0)
        .unwrap()
        .into_iter()
        .map(|event| event.ts)
        .collect()
}

/// 貼 A 的開場白停在逐字稿追加前，B 的聊天寫入被擋到 A 完成；A 成功時逐字稿是 A 再 B、開場紀錄掛在 A
#[test]
fn chat_write_waits_for_the_opening_post() {
    let _serial = data::write_hook::TESTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let root = TestRoot::new("race-opening-chat");
    let world_id = data::create_world(root.path(), "驛站").unwrap();
    let a = import::import_worldbook_file(
        root.path(),
        &world_id,
        &book("甲", "甲的開場"),
        "甲",
        &data::test_exclusive(&world_id),
    )
    .unwrap()
    .source;

    let (path, world) = (root.path().to_path_buf(), world_id.clone());
    interleave(
        world_file(root.path(), &world_id, "transcript/0.jsonl"),
        move || {
            let held = wait_exclusive(&world);
            import::post_opening_text(
                &path,
                &world,
                0,
                "t-a",
                "甲的開場",
                "zh-TW",
                Some(0),
                a.as_deref(),
                &held,
            )
            .unwrap();
        },
        chat_write(root.path().to_path_buf(), world_id.clone(), "t-b"),
    );

    assert_eq!(transcript_ts(root.path(), &world_id), ["t-a", "t-b"]);
    let replays = import_replays(root.path(), &world_id).unwrap().unwrap();
    assert_eq!(replays[0].opening, Some((0, "t-a".to_owned(), Some(0))));
    assert_eq!(pending_markers(root.path(), &world_id), 0);
}

/// Sol 第 11 輪的情境：A 拍完回復點後 B 想寫，A 追加只寫進半行就失敗、整檔寫回貼之前。B 被擋在 A 回復
/// 完成之後才寫，所以 B 的那句與狀態都留著；A 的標記在確認回復後解除
#[test]
fn opening_rollback_does_not_wipe_a_concurrent_chat_write() {
    let _serial = data::write_hook::TESTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let root = TestRoot::new("race-opening-rollback");
    let world_id = data::create_world(root.path(), "驛站").unwrap();
    let a = import::import_worldbook_file(
        root.path(),
        &world_id,
        &book("甲", "甲的開場"),
        "甲",
        &data::test_exclusive(&world_id),
    )
    .unwrap()
    .source;
    data::append_transcript(root.path(), &world_id, 0, &transcript_event("t-0", "前情")).unwrap();

    let (path, world) = (root.path().to_path_buf(), world_id.clone());
    interleave(
        world_file(root.path(), &world_id, "transcript/0.jsonl"),
        move || {
            let held = wait_exclusive(&world);
            let _partial = data::AppendFailGuard::partial(1);
            assert!(import::post_opening_text(
                &path,
                &world,
                0,
                "t-a",
                "甲的開場",
                "zh-TW",
                Some(0),
                a.as_deref(),
                &held,
            )
            .is_err());
        },
        chat_write(root.path().to_path_buf(), world_id.clone(), "t-b"),
    );

    assert_eq!(transcript_ts(root.path(), &world_id), ["t-0", "t-b"]);
    let last = data::read_transcript(root.path(), &world_id, 0)
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(
        Some(data::read_state(root.path(), &world_id).unwrap().state),
        last.state,
        "狀態是 B 寫完那一刻，不是被 A 寫回的舊位元組"
    );
    let replays = import_replays(root.path(), &world_id).unwrap().unwrap();
    assert_eq!(replays[0].opening, None);
    assert_eq!(pending_markers(root.path(), &world_id), 0);
}

/// 撤銷 A 停在世界書寫入前，匯入 B 被擋到撤銷完成：世界書只剩 B 的條目（A 不會被 B 寫回而復活）、收據
/// 只剩 B、A 的原檔已刪
#[test]
fn import_waits_for_undo_so_the_undone_entries_stay_gone() {
    let _serial = data::write_hook::TESTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let root = TestRoot::new("race-undo-import");
    let world_id = data::create_world(root.path(), "驛站").unwrap();
    let a = import::import_worldbook_file(
        root.path(),
        &world_id,
        &book("甲", "甲的開場"),
        "甲",
        &data::test_exclusive(&world_id),
    )
    .unwrap()
    .source
    .unwrap();

    let (path_a, world_a) = (root.path().to_path_buf(), world_id.clone());
    let (path_b, world_b) = (root.path().to_path_buf(), world_id.clone());
    interleave(
        world_file(root.path(), &world_id, "worldbook.json"),
        move || {
            let held = wait_exclusive(&world_a);
            undo_last_import(&path_a, &world_a, &held).unwrap();
        },
        move || {
            let held = wait_exclusive(&world_b);
            import::import_worldbook_file(&path_b, &world_b, &book("乙", "乙的開場"), "乙", &held)
                .unwrap();
        },
    );

    let titles: Vec<String> = data::read_worldbook(root.path(), &world_id)
        .unwrap()
        .into_iter()
        .map(|entry| entry.title)
        .collect();
    assert_eq!(titles, ["乙"]);
    let replays = import_replays(root.path(), &world_id).unwrap().unwrap();
    assert_eq!(replays.len(), 1);
    assert_eq!(replays[0].source.label, "乙");
    assert!(!world_file(root.path(), &world_id, &a).exists());
    assert_eq!(pending_markers(root.path(), &world_id), 0);
}

/// 逐字稿只追加進半行就失敗：寫回貼之前的逐字稿與狀態、確認回到原樣才解除標記，桌上不留半行
#[test]
fn partially_appended_opening_is_rolled_back_before_the_marker_is_cleared() {
    let root = TestRoot::new("opening-partial");
    let world_id = data::create_world(root.path(), "驛站").unwrap();
    let a = import::import_worldbook_file(
        root.path(),
        &world_id,
        &book("甲", "甲的開場"),
        "甲",
        &data::test_exclusive(&world_id),
    )
    .unwrap()
    .source;
    // 這一幕先有一則，回復要寫回原內容而不是刪檔
    data::append_transcript(root.path(), &world_id, 0, &transcript_event("t-0", "前情")).unwrap();
    let transcript = root
        .path()
        .join("worlds")
        .join(&world_id)
        .join("transcript")
        .join("0.jsonl");
    let state = root
        .path()
        .join("worlds")
        .join(&world_id)
        .join("state.json");
    let (transcript_before, state_before) =
        (fs::read(&transcript).unwrap(), fs::read(&state).unwrap());
    {
        let _guard = data::AppendFailGuard::partial(1);
        assert!(import::post_opening_text(
            root.path(),
            &world_id,
            0,
            "t-a",
            "甲的開場",
            "zh-TW",
            Some(0),
            a.as_deref(),
            &data::test_exclusive(&world_id),
        )
        .is_err());
    }
    assert_eq!(fs::read(&transcript).unwrap(), transcript_before);
    assert_eq!(fs::read(&state).unwrap(), state_before);
    assert_eq!(pending_markers(root.path(), &world_id), 0);
    assert!(import_replays(root.path(), &world_id).unwrap().is_some());
}

/// 部分追加後連回復都失敗（原本沒有逐字稿、刪不掉半行）：標記留著，來源判不完整
#[test]
fn opening_that_cannot_be_rolled_back_keeps_the_marker() {
    let root = TestRoot::new("opening-partial-stuck");
    let world_id = data::create_world(root.path(), "驛站").unwrap();
    let a = import::import_worldbook_file(
        root.path(),
        &world_id,
        &book("甲", "甲的開場"),
        "甲",
        &data::test_exclusive(&world_id),
    )
    .unwrap()
    .source;
    let transcript = root
        .path()
        .join("worlds")
        .join(&world_id)
        .join("transcript")
        .join("0.jsonl");
    assert!(!transcript.exists());
    {
        let _append = data::AppendFailGuard::partial(1);
        let _remove = data::RemoveFailGuard::fail_ending("0.jsonl", 1);
        assert!(import::post_opening_text(
            root.path(),
            &world_id,
            0,
            "t-a",
            "甲的開場",
            "zh-TW",
            Some(0),
            a.as_deref(),
            &data::test_exclusive(&world_id),
        )
        .is_err());
    }
    assert_eq!(pending_markers(root.path(), &world_id), 1);
    assert_eq!(import_replays(root.path(), &world_id).unwrap(), None);
}
