//! 前端存條目的防線（world-editor-delete-character-guard）：卡檔確定已不在的角色 id 寫不回可見名單，
//! 以及它與刪角色整段（刪卡＋清理）在短提交鎖上的排序。
use super::*;
use crate::data::state_commit::contention_hook;
use crate::data::write_hook;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};
use std::time::Duration;

fn characters(ids: &[&str]) -> Visibility {
    Visibility::Characters(ids.iter().map(|id| (*id).to_owned()).collect())
}

/// 書裡放一條 GM 條目，回傳 uid。
fn gm_entry(root: &Path, world_id: &str, content: &str) -> u64 {
    import_book(root, world_id, content);
    entry(root, world_id, content).0
}

fn save_with(root: &Path, world_id: &str, uid: u64, visibility: Visibility) -> Visibility {
    let mut current = compact(root, world_id, uid);
    current.visibility = visibility;
    upsert_entry_dropping_gone(root, world_id, current, false).unwrap();
    compact(root, world_id, uid).visibility
}

#[test]
fn saving_drops_only_ids_whose_card_file_is_gone() {
    let (root, world_id) = new_table("guard-drop");
    let a = import_card(root.path(), &world_id, &plain_card("甲"));
    let b = import_card(root.path(), &world_id, &plain_card("乙"));
    // 玩家卡：卡檔在，但不在 list_characters 上
    let player = import_card(root.path(), &world_id, &plain_card("我"));
    data::set_player_card(root.path(), &world_id, Some(player.clone())).unwrap();
    // 解析不了但檔還在的卡
    let broken = data::new_id();
    fs::write(
        root.path()
            .join(format!("worlds/{world_id}/characters/{broken}.md")),
        "壞掉的卡",
    )
    .unwrap();
    let listed: Vec<String> = data::list_characters(root.path(), &world_id)
        .unwrap()
        .into_iter()
        .map(|meta| meta.id)
        .collect();
    assert!(!listed.contains(&player) && !listed.contains(&broken));
    delete(root.path(), &world_id, &b);
    // 格式合法、本來就沒卡的殘留 id 也一併清（刻意比刪除清理寬）
    let stale = data::new_id();
    let uid = gm_entry(root.path(), &world_id, "名單");

    let saved = save_with(
        root.path(),
        &world_id,
        uid,
        characters(&[&a, &b, &b, &player, &broken, &stale]),
    );
    assert_eq!(saved, characters(&[&a, &player, &broken]));
}

#[test]
fn saving_only_gone_ids_hands_the_entry_to_the_gm() {
    let (root, world_id) = new_table("guard-gm");
    let b = import_card(root.path(), &world_id, &plain_card("乙"));
    delete(root.path(), &world_id, &b);
    let uid = gm_entry(root.path(), &world_id, "只剩乙");
    assert_eq!(
        save_with(root.path(), &world_id, uid, characters(&[&b, &b])),
        Visibility::Gm
    );
}

#[test]
fn saving_without_hits_keeps_the_entry_as_is() {
    let (root, world_id) = new_table("guard-keep");
    let a = import_card(root.path(), &world_id, &plain_card("甲"));
    let uid = gm_entry(root.path(), &world_id, "沒命中");
    assert_eq!(
        save_with(root.path(), &world_id, uid, characters(&[&a])),
        characters(&[&a])
    );
    // 本來就空的名單不改 GM
    assert_eq!(
        save_with(root.path(), &world_id, uid, characters(&[])),
        characters(&[])
    );
}

/// 查詢錯誤（id 不合法）保留、存檔照樣成功。
#[test]
fn invalid_ids_are_kept_and_the_save_succeeds() {
    let (root, world_id) = new_table("guard-invalid");
    let uid = gm_entry(root.path(), &world_id, "不合法");
    assert_eq!(
        save_with(root.path(), &world_id, uid, characters(&["../壞 id"])),
        characters(&["../壞 id"])
    );
}

/// 實際 I/O 查詢錯誤（角色目錄不可進入）也保留、存檔成功。root 下權限擋不住就略過。
#[cfg(unix)]
#[test]
fn io_errors_while_checking_keep_the_id() {
    let (root, world_id) = new_table("guard-io");
    let a = import_card(root.path(), &world_id, &plain_card("甲"));
    let uid = gm_entry(root.path(), &world_id, "查詢錯誤");
    let directory = root.path().join(format!("worlds/{world_id}/characters"));
    let mut current = compact(root.path(), &world_id, uid);
    current.visibility = characters(&[&a]);
    let Some(saved) =
        crate::test_perm::with_unsearchable(&directory, &directory.join(format!("{a}.md")), || {
            upsert_entry_dropping_gone(root.path(), &world_id, current, false)
        })
    else {
        return;
    };
    saved.unwrap();
    assert_eq!(
        compact(root.path(), &world_id, uid).visibility,
        characters(&[&a])
    );
}

/// 新條目分支也套用。
#[test]
fn new_entries_are_guarded_too() {
    let (root, world_id) = new_table("guard-new");
    let b = import_card(root.path(), &world_id, &plain_card("乙"));
    delete(root.path(), &world_id, &b);
    let uid = upsert_entry_dropping_gone(
        root.path(),
        &world_id,
        WorldbookEntry {
            uid: u64::MAX,
            title: "新".to_owned(),
            keys: Vec::new(),
            content: "新條目".to_owned(),
            constant: false,
            order: 100,
            disabled: false,
            visibility: characters(&[&b]),
            is_person: false,
            locked: false,
        },
        true,
    )
    .unwrap();
    assert_eq!(
        compact(root.path(), &world_id, uid).visibility,
        Visibility::Gm
    );
}

/// 存條目先拿到鎖：檢查時乙還在、照寫；刪角色整段排在後面，清理把它拿掉。
#[test]
fn delete_waits_for_a_save_that_already_checked() {
    let _serial = write_hook::TESTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (root, world_id) = new_table("guard-save-first");
    let b = import_card(root.path(), &world_id, &plain_card("乙"));
    let uid = gm_entry(root.path(), &world_id, "先存");
    let book = root
        .path()
        .join(format!("worlds/{world_id}/worldbook.json"));
    let fired = Arc::new(AtomicBool::new(false));
    let (done_sender, done) = mpsc::channel();
    let blocked = Arc::new(AtomicBool::new(false));
    // 握手：刪角色那條執行緒搶這桌提交鎖、發現被占時回報
    let (contended_sender, contended) = mpsc::channel::<()>();
    let contended = Arc::new(std::sync::Mutex::new(contended));
    let _contention = contention_hook::install(&world_id, move || {
        let _ = contended_sender.send(());
    });
    let _hook = {
        let contended = contended.clone();
        let (fired, blocked) = (fired.clone(), blocked.clone());
        let (root, world_id, b) = (root.path().to_path_buf(), world_id.clone(), b.clone());
        write_hook::install(book, move || {
            if fired.swap(true, Ordering::SeqCst) {
                return;
            }
            let (thread_root, thread_world, thread_b, sender) = (
                root.clone(),
                world_id.clone(),
                b.clone(),
                done_sender.clone(),
            );
            std::thread::spawn(move || {
                let outcome = delete_character_and_clean(&thread_root, &thread_world, &thread_b);
                let _ = sender.send(outcome.map(|outcome| outcome.worldbook_cleanup_failed));
            });
            let waiting = contended
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(10))
                .is_ok();
            let card_still_there = !data::character_card_gone(&root, &world_id, &b).unwrap();
            blocked.store(waiting && card_still_there, Ordering::SeqCst);
        })
    };
    let mut current = compact(root.path(), &world_id, uid);
    current.visibility = characters(&[&b]);
    upsert_entry_dropping_gone(root.path(), &world_id, current, false).unwrap();
    let cleanup_failed = done.recv_timeout(Duration::from_secs(10)).unwrap().unwrap();
    drop(_hook);
    drop(_contention);
    assert!(blocked.load(Ordering::SeqCst), "刪卡沒有等存條目放鎖");
    assert!(!cleanup_failed);
    assert!(data::character_card_gone(root.path(), &world_id, &b).unwrap());
    assert_eq!(
        compact(root.path(), &world_id, uid).visibility,
        Visibility::Gm
    );
}

/// 刪角色先拿到鎖、清理失敗：之後才輪到的存條目看到卡檔已不在，照樣不寫回乙。
#[test]
fn a_save_after_a_failed_cleanup_still_drops_the_deleted_id() {
    let _serial = write_hook::TESTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (root, world_id) = new_table("guard-delete-first");
    let b = import_card(root.path(), &world_id, &plain_card("乙"));
    let uid = gm_entry(root.path(), &world_id, "後存");
    edit(root.path(), &world_id, uid, |entry| {
        entry.visibility = characters(&[&b]);
    });
    let stale = compact(root.path(), &world_id, uid);
    let book = root
        .path()
        .join(format!("worlds/{world_id}/worldbook.json"));
    let fired = Arc::new(AtomicBool::new(false));
    let save_done = Arc::new(AtomicBool::new(false));
    let blocked = Arc::new(AtomicBool::new(false));
    let (saved_sender, saved) = mpsc::channel();
    // 握手：存條目那條執行緒搶這桌提交鎖、發現被占時回報
    let (contended_sender, contended) = mpsc::channel::<()>();
    let contended = Arc::new(std::sync::Mutex::new(contended));
    let _contention = contention_hook::install(&world_id, move || {
        let _ = contended_sender.send(());
    });
    let _hook = {
        let contended = contended.clone();
        let (fired, save_done, blocked) = (fired.clone(), save_done.clone(), blocked.clone());
        let (root, world_id) = (root.path().to_path_buf(), world_id.clone());
        write_hook::install(book, move || {
            if fired.swap(true, Ordering::SeqCst) {
                return;
            }
            // 刪角色的清理正要寫世界書：卡檔已刪、鎖在它手上
            let (root, world_id, stale, sender, done) = (
                root.clone(),
                world_id.clone(),
                stale.clone(),
                saved_sender.clone(),
                save_done.clone(),
            );
            std::thread::spawn(move || {
                let saved = upsert_entry_dropping_gone(&root, &world_id, stale, false).is_ok();
                done.store(true, Ordering::SeqCst);
                let _ = sender.send(saved);
            });
            let waiting = contended
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(10))
                .is_ok();
            blocked.store(
                waiting && !save_done.load(Ordering::SeqCst),
                Ordering::SeqCst,
            );
        })
    };
    let outcome = {
        let _guard = WriteFailGuard::partial_ending("worldbook.json.tmp", 1);
        delete(root.path(), &world_id, &b)
    };
    assert!(outcome.worldbook_cleanup_failed);
    assert!(saved.recv_timeout(Duration::from_secs(10)).unwrap());
    drop(_hook);
    drop(_contention);
    assert!(blocked.load(Ordering::SeqCst), "存條目沒有等刪角色放鎖");
    assert_eq!(
        compact(root.path(), &world_id, uid).visibility,
        Visibility::Gm
    );
}

/// 既有條目（create＝false）已不在：回 `worldbook_entry_gone`、世界書不變；新條目照常新增。
#[test]
fn saving_a_vanished_existing_entry_is_refused() {
    let (root, world_id) = new_table("guard-gone");
    let uid = data::upsert_worldbook_entry(
        root.path(),
        &world_id,
        WorldbookEntry {
            uid: u64::MAX,
            title: "舊".to_owned(),
            keys: Vec::new(),
            content: "會被刪".to_owned(),
            constant: false,
            order: 100,
            disabled: false,
            visibility: Visibility::Gm,
            is_person: false,
            locked: false,
        },
    )
    .unwrap();
    let stale = compact(root.path(), &world_id, uid);
    data::delete_worldbook_entry(root.path(), &world_id, uid).unwrap();
    let before = data::read_worldbook_raw(root.path(), &world_id).unwrap();

    let error = upsert_entry_dropping_gone(root.path(), &world_id, stale.clone(), false)
        .unwrap_err()
        .to_string();
    assert_eq!(error, crate::ui_msg::UiMsg::WorldbookEntryGone.to_string());
    assert_eq!(
        data::read_worldbook_raw(root.path(), &world_id).unwrap(),
        before
    );

    let created = upsert_entry_dropping_gone(root.path(), &world_id, stale, true).unwrap();
    assert_eq!(compact(root.path(), &world_id, created).content, "會被刪");
}
