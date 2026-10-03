//! 逐字稿寫入安全：同檔鎖讓追加、截回、讀改寫、讀者不互相踩到；追加失敗截回；
//! 收回沒有回覆的玩家句只在收據與內容都對上時才截檔。
use super::*;
use crate::data::test_support::*;
use crate::data::world_file::with_file_lock;
use crate::data::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

fn event(kind: TranscriptKind, ts: &str, text: &str) -> TranscriptEvent {
    TranscriptEvent {
        raw: None,
        ts: ts.to_owned(),
        speaker_id: String::new(),
        speaker_name: if kind == TranscriptKind::Player {
            "阿濤"
        } else {
            "GM"
        }
        .to_owned(),
        kind,
        text: text.to_owned(),
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
    }
}

fn texts(root: &Path, world_id: &str) -> Vec<String> {
    read_transcript(root, world_id, 0)
        .unwrap()
        .into_iter()
        .map(|event| event.text)
        .collect()
}

fn path_of(root: &Path, world_id: &str) -> PathBuf {
    transcript_path(root, world_id, 0).unwrap()
}

/// 一方在鎖內停住時，另一方不得完成；放開後兩邊各自完成。
fn while_locked<B>(
    path: PathBuf,
    inside: impl FnOnce(&super::super::super::world_file::LockedFile<'_>) + Send + 'static,
    other: B,
) where
    B: FnOnce() + Send + 'static,
{
    let (paused_tx, paused_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let holder = std::thread::spawn(move || {
        with_file_lock(&path, |file| {
            paused_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            inside(file);
        });
    });
    paused_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let done = Arc::new(AtomicBool::new(false));
    let waiter = {
        let done = done.clone();
        std::thread::spawn(move || {
            other();
            done.store(true, Ordering::SeqCst);
        })
    };
    std::thread::sleep(Duration::from_millis(150));
    assert!(
        !done.load(Ordering::SeqCst),
        "持鎖的一方還沒放開，另一方不該完成"
    );
    release_tx.send(()).unwrap();
    holder.join().unwrap();
    waiter.join().unwrap();
}

#[test]
fn partial_append_is_cut_back_to_the_original_length() {
    let root = TestRoot::new("transcript-partial-cut");
    let world_id = create_world(root.path(), "截回桌").unwrap();
    append_transcript(
        root.path(),
        &world_id,
        0,
        &event(TranscriptKind::Narration, "t0", "序幕"),
    )
    .unwrap();
    let before = fs::read(path_of(root.path(), &world_id)).unwrap();
    {
        let _partial = AppendFailGuard::partial(1);
        assert!(append_transcript(
            root.path(),
            &world_id,
            0,
            &event(TranscriptKind::Player, "t1", "我推開門")
        )
        .is_err());
    }
    assert_eq!(fs::read(path_of(root.path(), &world_id)).unwrap(), before);
    assert_eq!(texts(root.path(), &world_id), ["序幕"]);
}

#[test]
fn residue_left_when_cut_back_fails_is_skipped_by_readers() {
    let root = TestRoot::new("transcript-partial-stuck");
    let world_id = create_world(root.path(), "殘段桌").unwrap();
    append_transcript(
        root.path(),
        &world_id,
        0,
        &event(TranscriptKind::Narration, "t0", "序幕"),
    )
    .unwrap();
    {
        let _partial = AppendFailGuard::partial(1);
        let _truncate = TruncateFailGuard::fail(1);
        assert!(append_transcript(
            root.path(),
            &world_id,
            0,
            &event(TranscriptKind::Player, "t1", "我推開門")
        )
        .is_err());
    }
    let bytes = fs::read(path_of(root.path(), &world_id)).unwrap();
    assert!(!bytes.ends_with(b"\n"), "截不回時殘段留著");
    assert_eq!(texts(root.path(), &world_id), ["序幕"]);
}

#[test]
fn complete_last_line_without_newline_counts_and_next_append_splits_it() {
    let root = TestRoot::new("transcript-no-newline");
    let world_id = create_world(root.path(), "缺換行桌").unwrap();
    let path = path_of(root.path(), &world_id);
    let line = serde_json::to_string(&event(TranscriptKind::Narration, "t0", "序幕")).unwrap();
    commit_world_write(&path, line.as_bytes()).unwrap();
    assert_eq!(texts(root.path(), &world_id), ["序幕"]);

    let offset = append_transcript(
        root.path(),
        &world_id,
        0,
        &event(TranscriptKind::Player, "t1", "我推開門"),
    )
    .unwrap();
    assert_eq!(offset, line.len() as u64 + 1, "收據落在補上的換行之後");
    assert_eq!(texts(root.path(), &world_id), ["序幕", "我推開門"]);
}

#[test]
fn bad_line_in_the_middle_still_fails() {
    let root = TestRoot::new("transcript-bad-middle");
    let world_id = create_world(root.path(), "壞行桌").unwrap();
    let path = path_of(root.path(), &world_id);
    let good = serde_json::to_string(&event(TranscriptKind::Narration, "t0", "序幕")).unwrap();
    commit_world_write(&path, format!("{{壞掉\n{good}\n").as_bytes()).unwrap();
    assert!(read_transcript(root.path(), &world_id, 0).is_err());
}

#[test]
fn failed_append_cut_back_does_not_eat_a_concurrent_append() {
    let root = TestRoot::new("transcript-interleave");
    let world_id = create_world(root.path(), "交錯桌").unwrap();
    append_transcript(
        root.path(),
        &world_id,
        0,
        &event(TranscriptKind::Narration, "t0", "序幕"),
    )
    .unwrap();
    let line = {
        let mut bytes =
            serde_json::to_vec(&event(TranscriptKind::Player, "t-a", "甲半途失敗")).unwrap();
        bytes.push(b'\n');
        bytes
    };
    let (path, root_b, world_b) = (
        path_of(root.path(), &world_id),
        root.path().to_path_buf(),
        world_id.clone(),
    );
    while_locked(
        path,
        move |file| {
            let _partial = AppendFailGuard::partial(1);
            assert!(file.append(&line).is_err());
        },
        move || {
            append_transcript(
                &root_b,
                &world_b,
                0,
                &event(TranscriptKind::Player, "t-b", "乙完整"),
            )
            .unwrap();
        },
    );
    assert_eq!(texts(root.path(), &world_id), ["序幕", "乙完整"]);
}

#[test]
fn failed_ledger_append_cut_back_does_not_eat_a_concurrent_append() {
    let root = TestRoot::new("ledger-interleave");
    let world_id = create_world(root.path(), "帳本桌").unwrap();
    let path = mechanism_log_path(root.path(), &world_id).unwrap();
    commit_world_append(&path, b"{\"n\":0}\n").unwrap();
    let other = path.clone();
    while_locked(
        path.clone(),
        |file| {
            let _partial = AppendFailGuard::partial(1);
            assert!(file.append(b"{\"n\":1}\n{\"n\":2}\n").is_err());
        },
        move || {
            commit_world_append(&other, b"{\"n\":3}\n").unwrap();
        },
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "{\"n\":0}\n{\"n\":3}\n");
}

#[test]
fn pop_waits_for_an_in_flight_append_and_removes_the_real_last_event() {
    let root = TestRoot::new("transcript-pop-interleave");
    let world_id = create_world(root.path(), "讀改寫桌").unwrap();
    for (ts, text) in [("t0", "序幕"), ("t1", "我推開門")] {
        append_transcript(
            root.path(),
            &world_id,
            0,
            &event(TranscriptKind::Narration, ts, text),
        )
        .unwrap();
    }
    let line = {
        let mut bytes =
            serde_json::to_vec(&event(TranscriptKind::Dialogue, "t2", "誰在那裡？")).unwrap();
        bytes.push(b'\n');
        bytes
    };
    let (root_b, world_b) = (root.path().to_path_buf(), world_id.clone());
    while_locked(
        path_of(root.path(), &world_id),
        move |file| {
            file.append(&line).unwrap();
        },
        move || {
            assert!(pop_transcript(&root_b, &world_b, 0).unwrap());
        },
    );
    assert_eq!(texts(root.path(), &world_id), ["序幕", "我推開門"]);
}

#[test]
fn reader_waits_out_a_half_written_window() {
    let root = TestRoot::new("transcript-reader-window");
    let world_id = create_world(root.path(), "讀者桌").unwrap();
    append_transcript(
        root.path(),
        &world_id,
        0,
        &event(TranscriptKind::Narration, "t0", "序幕"),
    )
    .unwrap();
    let path = path_of(root.path(), &world_id);
    let original = fs::read(&path).unwrap();
    let (root_b, world_b) = (root.path().to_path_buf(), world_id.clone());
    let (seen_tx, seen_rx) = mpsc::channel();
    while_locked(
        path,
        move |file| {
            let mut half = original.clone();
            half.extend_from_slice(b"{\"ts\":\"t1\",\"spea");
            file.write(&half).unwrap();
            file.write(&original).unwrap();
        },
        move || {
            seen_tx
                .send(read_transcript(&root_b, &world_b, 0).unwrap().len())
                .unwrap();
        },
    );
    assert_eq!(seen_rx.recv().unwrap(), 1);
}

fn discard_setup(label: &str) -> (TestRoot, String, u64) {
    let root = TestRoot::new(label);
    let world_id = create_world(root.path(), "收回玩家句桌").unwrap();
    append_transcript(
        root.path(),
        &world_id,
        0,
        &event(TranscriptKind::Narration, "t0", "序幕"),
    )
    .unwrap();
    let offset = append_transcript(
        root.path(),
        &world_id,
        0,
        &event(TranscriptKind::Player, "t1", "我推開門"),
    )
    .unwrap();
    (root, world_id, offset)
}

#[test]
fn discard_cuts_the_matching_player_line_and_rewinds_state() {
    let (root, world_id, offset) = discard_setup("discard-ok");
    let path = path_of(root.path(), &world_id);
    let before = fs::read(&path).unwrap();
    // 玩家句落檔後檯面被改（例如 GM 在後端套了狀態區塊），收回要一起退回
    let mut state = read_state(root.path(), &world_id).unwrap();
    state.state.table.insert("hp".to_owned(), "3".to_owned());
    write_state(root.path(), &world_id, &state).unwrap();

    assert!(
        discard_unanswered_player(root.path(), &world_id, 0, offset, "t1", "我推開門").unwrap()
    );
    assert_eq!(fs::read(&path).unwrap(), before[..offset as usize]);
    assert_eq!(texts(root.path(), &world_id), ["序幕"]);
    assert_eq!(
        read_state(root.path(), &world_id).unwrap().state,
        TableState::default()
    );
}

#[test]
fn discard_leaves_a_line_the_backend_appended_after_the_player() {
    let (root, world_id, offset) = discard_setup("discard-arrival");
    let mut arrival = event(TranscriptKind::System, "t2", "");
    arrival.marker = Some(EventMarker::PersonArrival {
        title: "老闆".to_owned(),
    });
    append_transcript(root.path(), &world_id, 0, &arrival).unwrap();
    let before = fs::read(path_of(root.path(), &world_id)).unwrap();
    assert!(
        !discard_unanswered_player(root.path(), &world_id, 0, offset, "t1", "我推開門").unwrap()
    );
    assert_eq!(fs::read(path_of(root.path(), &world_id)).unwrap(), before);
}

#[test]
fn discard_only_takes_the_receipted_line_among_duplicates() {
    let root = TestRoot::new("discard-dup");
    let world_id = create_world(root.path(), "重複句桌").unwrap();
    append_transcript(
        root.path(),
        &world_id,
        0,
        &event(TranscriptKind::Player, "t1", "再說一次"),
    )
    .unwrap();
    let offset = append_transcript(
        root.path(),
        &world_id,
        0,
        &event(TranscriptKind::Player, "t1", "再說一次"),
    )
    .unwrap();
    assert!(
        discard_unanswered_player(root.path(), &world_id, 0, offset, "t1", "再說一次").unwrap()
    );
    assert_eq!(texts(root.path(), &world_id), ["再說一次"]);
}

#[test]
fn discard_refuses_any_mismatch() {
    let (root, world_id, offset) = discard_setup("discard-mismatch");
    let before = fs::read(path_of(root.path(), &world_id)).unwrap();
    for (at, ts, text) in [
        (offset + 1, "t1", "我推開門"), // 不是行首
        (0, "t0", "序幕"),              // 不是最後一行
        (offset, "t9", "我推開門"),     // ts 不符
        (offset, "t1", "我關上門"),     // 內容不符
        (offset + 9_999, "t1", "我推開門"),
    ] {
        assert!(!discard_unanswered_player(root.path(), &world_id, 0, at, ts, text).unwrap());
    }
    assert_eq!(fs::read(path_of(root.path(), &world_id)).unwrap(), before);

    // 尾筆不是玩家句
    let narration = append_transcript(
        root.path(),
        &world_id,
        0,
        &event(TranscriptKind::Narration, "t2", "門開了"),
    )
    .unwrap();
    assert!(
        !discard_unanswered_player(root.path(), &world_id, 0, narration, "t2", "門開了").unwrap()
    );
    // 這一幕沒有檔
    assert!(!discard_unanswered_player(root.path(), &world_id, 5, 0, "t1", "我推開門").unwrap());
}

#[test]
fn discard_whose_truncate_errors_reports_false_and_leaves_the_file() {
    let (root, world_id, offset) = discard_setup("discard-truncate-fail");
    let before = fs::read(path_of(root.path(), &world_id)).unwrap();
    let _truncate = TruncateFailGuard::fail(1);
    assert!(
        !discard_unanswered_player(root.path(), &world_id, 0, offset, "t1", "我推開門").unwrap()
    );
    assert_eq!(fs::read(path_of(root.path(), &world_id)).unwrap(), before);
}

#[test]
fn discard_counts_a_truncate_that_errors_after_actually_cutting() {
    let (root, world_id, offset) = discard_setup("discard-cut-then-fail");
    let _truncate = TruncateFailGuard::cut_then_fail(1);
    assert!(
        discard_unanswered_player(root.path(), &world_id, 0, offset, "t1", "我推開門").unwrap()
    );
    assert_eq!(texts(root.path(), &world_id), ["序幕"]);
}

#[test]
fn discard_is_false_when_neither_truncate_nor_measuring_succeeds() {
    let (root, world_id, offset) = discard_setup("discard-len-fail");
    let _truncate = TruncateFailGuard::cut_then_fail(1).len_fails(1);
    assert!(
        !discard_unanswered_player(root.path(), &world_id, 0, offset, "t1", "我推開門").unwrap()
    );
}

#[test]
fn discard_still_true_when_the_state_file_cannot_be_read_for_rewind() {
    let (root, world_id, offset) = discard_setup("discard-state-fail");
    let state_path = root
        .path()
        .join("worlds")
        .join(&world_id)
        .join("state.json");
    // state.json 壞掉：檯面回捲讀不到，收回本身照樣算數
    commit_world_write(&state_path, b"not json").unwrap();
    assert!(
        discard_unanswered_player(root.path(), &world_id, 0, offset, "t1", "我推開門").unwrap()
    );
    assert_eq!(texts(root.path(), &world_id), ["序幕"]);
}
