//! 格式標記、恢復表、寫入閘門。每列恢復與每種判讀都有斷言。
use std::fs;
use std::path::{Path, PathBuf};

use super::super::test_support::{character_card, worldbook_entry, TestRoot};
use super::super::world_file::{self, RemoveFailGuard, RenameFailGuard};
use super::super::{
    append_transcript, create_world, delete_world, list_worlds, read_state, reclaim_world_if_empty,
    upsert_worldbook_entry, write_character, write_state, write_world_md, TranscriptEvent,
    TranscriptKind,
};
use super::marker::{read_format, CurrentOverride, FormatVersion};
use super::{open_world, read_world_readonly, restore_world_backup, OpenWorld, StepOverride};
use crate::import::save_character_image;
use crate::mechanism::{append_log, Record, RecordKind};

fn live(root: &Path, id: &str) -> PathBuf {
    root.join("worlds").join(id)
}

fn side(root: &Path, id: &str, kind: &str) -> PathBuf {
    let name = match kind {
        "I" => id.to_owned(),
        "S" => format!(".tt-staging-{id}"),
        "P" => format!(".tt-pre-{id}"),
        "T" => format!(".tt-trash-{id}"),
        "N" => format!(".tt-newer-{id}"),
        "L" => format!(".tt-op-{id}.json"),
        other => panic!("unknown sidecar {other}"),
    };
    root.join("worlds").join(name)
}

fn fresh(label: &str) -> (TestRoot, String) {
    let root = TestRoot::new(label);
    let id = create_world(root.path(), "霧港").unwrap();
    fs::write(live(root.path(), &id).join("who.txt"), "I").unwrap();
    (root, id)
}

fn clone_as(root: &Path, id: &str, kind: &str, label: &str) {
    let dest = side(root, id, kind);
    world_file::copy_dir(&side(root, id, "I"), &dest).unwrap();
    fs::write(dest.join("who.txt"), label).unwrap();
}

fn set_ver(dir: &Path, version: impl AsRef<str>) {
    fs::write(
        dir.join("format.json"),
        format!(
            "{{\"format_version\":{},\"app_version\":\"9.9.9\"}}",
            version.as_ref()
        ),
    )
    .unwrap();
}

fn who(dir: &Path) -> String {
    fs::read_to_string(dir.join("who.txt")).unwrap_or_default()
}

fn put_log(root: &Path, id: &str, body: serde_json::Value) {
    fs::write(
        side(root, id, "L"),
        serde_json::to_vec_pretty(&body).unwrap(),
    )
    .unwrap();
}

fn migrate_log(stage: &str, had_pre: bool) -> serde_json::Value {
    serde_json::json!({
        "op": "migrate",
        "stage": stage,
        "from": 1,
        "to": 1,
        "had_pre": had_pre,
    })
}

fn restore_log(stage: &str, had_newer: bool) -> serde_json::Value {
    serde_json::json!({"op": "restore", "stage": stage, "had_newer": had_newer})
}

/// `dirs`：五個標籤，None 表示該目錄不該在。
fn expect_open(root: &Path, id: &str, dirs: [Option<&str>; 5], log: bool, repair: Option<&str>) {
    let opened = open_world(root, id).unwrap();
    match (repair, opened) {
        (None, OpenWorld::Ready) => {}
        (Some(needle), OpenWorld::NeedsRepair { message, directory }) => {
            assert!(message.contains(needle), "{message}");
            assert!(!directory.is_empty());
        }
        (_, other) => panic!("unexpected open result {other:?}"),
    }
    for (kind, label) in ["I", "S", "P", "T", "N"].into_iter().zip(dirs) {
        let path = side(root, id, kind);
        match label {
            None => assert!(!path.exists(), "{kind} should be absent"),
            Some(label) => {
                assert!(path.is_dir(), "{kind} missing");
                assert_eq!(who(&path), label, "{kind} moved wrong");
            }
        }
    }
    assert_eq!(side(root, id, "L").is_file(), log, "log presence");
}

fn event(text: &str) -> TranscriptEvent {
    TranscriptEvent {
        raw: None,
        ts: "2026-07-20T00:00:00+08:00".to_owned(),
        speaker_id: String::new(),
        speaker_name: "甲".to_owned(),
        kind: TranscriptKind::Dialogue,
        text: text.to_owned(),
        state: None,
        truncated: false,
        gm_only: false,
    }
}

#[test]
fn marker_reads_every_specified_case() {
    let root = TestRoot::new("marker");
    let id = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    let dir = root.path().join("worlds").join(id);
    fs::create_dir_all(&dir).unwrap();

    let missing = read_format(&dir);
    assert_eq!(missing.version, FormatVersion::Unknown);
    assert_eq!(missing.app_version, None);

    fs::write(dir.join("state.json"), r#"{"id":"x","name":"桌"}"#).unwrap();
    assert_eq!(read_format(&dir).version, FormatVersion::Known(1));

    fs::write(dir.join("state.json"), "{").unwrap();
    assert_eq!(read_format(&dir).version, FormatVersion::Unknown);
    fs::remove_file(dir.join("state.json")).unwrap();

    set_ver(&dir, "3");
    assert_eq!(read_format(&dir).version, FormatVersion::Known(3));
    assert_eq!(read_format(&dir).app_version.as_deref(), Some("9.9.9"));

    set_ver(&dir, "0");
    let zero = read_format(&dir);
    assert_eq!(zero.version, FormatVersion::Unknown);
    assert_eq!(zero.app_version.as_deref(), Some("9.9.9"));

    set_ver(&dir, "-1");
    assert_eq!(read_format(&dir).version, FormatVersion::Unknown);
    assert_eq!(read_format(&dir).app_version.as_deref(), Some("9.9.9"));

    set_ver(&dir, "1.5");
    assert_eq!(read_format(&dir).version, FormatVersion::Unknown);

    // 2.0 不是整數：這版 serde_json 的 as_u64 不收帶小數點的數字。〔模型判斷·未裁決〕
    set_ver(&dir, "2.0");
    assert_eq!(read_format(&dir).version, FormatVersion::Unknown);
    assert_eq!(read_format(&dir).app_version.as_deref(), Some("9.9.9"));

    fs::write(dir.join("format.json"), "nope").unwrap();
    assert_eq!(read_format(&dir).version, FormatVersion::Unknown);

    fs::write(
        dir.join("format.json"),
        r#"{"format_version":"1","app_version":"0.1.0"}"#,
    )
    .unwrap();
    let text = read_format(&dir);
    assert_eq!(text.version, FormatVersion::Unknown);
    assert_eq!(text.app_version.as_deref(), Some("0.1.0"));

    fs::remove_file(dir.join("format.json")).unwrap();
    fs::write(dir.join("state.json"), r#"{"id":"x","name":"舊"}"#).unwrap();
    assert_eq!(read_format(&dir).version, FormatVersion::Known(1));
    write_world_md(root.path(), id, "仍可寫").unwrap();
    assert_eq!(fs::read_to_string(dir.join("world.md")).unwrap(), "仍可寫");
}

#[test]
fn create_world_writes_marker_and_plain_save_does_not_rewrite_it() {
    let (root, id) = fresh("marker-save");
    let path = live(root.path(), &id).join("format.json");
    let before = fs::read(&path).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&before).unwrap();
    assert_eq!(value["format_version"], 1);
    assert_eq!(value["app_version"], env!("CARGO_PKG_VERSION"));
    let state = read_state(root.path(), &id).unwrap();
    write_state(root.path(), &id, &state).unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn migrate_build_drops_optional_staging_and_keeps_newer() {
    let (root, id) = fresh("m-build");
    clone_as(root.path(), &id, "N", "N");
    put_log(root.path(), &id, migrate_log("build", false));
    expect_open(
        root.path(),
        &id,
        [Some("I"), None, None, None, Some("N")],
        false,
        None,
    );

    let (root, id) = fresh("m-build-s");
    clone_as(root.path(), &id, "S", "S");
    clone_as(root.path(), &id, "P", "P");
    put_log(root.path(), &id, migrate_log("build", true));
    expect_open(
        root.path(),
        &id,
        [Some("I"), None, Some("P"), None, None],
        false,
        None,
    );
}

#[test]
fn migrate_swap_before_r1_drops_staging() {
    let (root, id) = fresh("m-r0");
    clone_as(root.path(), &id, "S", "S");
    put_log(root.path(), &id, migrate_log("swap", false));
    expect_open(
        root.path(),
        &id,
        [Some("I"), None, None, None, None],
        false,
        None,
    );
}

#[test]
fn migrate_swap_after_rollback_deleted_staging_drops_log() {
    let (root, id) = fresh("m-drop-log");
    clone_as(root.path(), &id, "P", "P");
    put_log(root.path(), &id, migrate_log("swap", true));
    expect_open(
        root.path(),
        &id,
        [Some("I"), None, Some("P"), None, None],
        false,
        None,
    );
}

#[test]
fn migrate_swap_after_r1_rolls_trash_back_to_pre() {
    let (root, id) = fresh("m-r1");
    clone_as(root.path(), &id, "S", "S");
    clone_as(root.path(), &id, "T", "T");
    put_log(root.path(), &id, migrate_log("swap", true));
    expect_open(
        root.path(),
        &id,
        [Some("I"), None, Some("T"), None, None],
        false,
        None,
    );
}

#[test]
fn migrate_swap_after_r2_continues_when_staging_verifies() {
    let (root, id) = fresh("m-r2-ok");
    fs::remove_dir_all(live(root.path(), &id)).unwrap();
    // 先有一份完整桌才能複製；刪掉 I 之後用暫存的那份當 S。
    let (donor_root, donor) = fresh("m-r2-donor");
    world_file::copy_dir(
        &live(donor_root.path(), &donor),
        &side(root.path(), &id, "S"),
    )
    .unwrap();
    fs::write(side(root.path(), &id, "S").join("who.txt"), "S").unwrap();
    world_file::copy_dir(
        &live(donor_root.path(), &donor),
        &side(root.path(), &id, "P"),
    )
    .unwrap();
    fs::write(side(root.path(), &id, "P").join("who.txt"), "P").unwrap();
    put_log(root.path(), &id, migrate_log("swap", true));
    // T 也在（h 真）。接續後 cleanup 會刪掉它。
    world_file::copy_dir(
        &live(donor_root.path(), &donor),
        &side(root.path(), &id, "T"),
    )
    .unwrap();
    fs::write(side(root.path(), &id, "T").join("who.txt"), "T").unwrap();
    expect_open(
        root.path(),
        &id,
        [Some("S"), None, Some("P"), None, None],
        false,
        None,
    );
}

#[test]
fn migrate_swap_after_r2_rolls_back_when_staging_fails_verify() {
    let (root, id) = fresh("m-r2-bad");
    clone_as(root.path(), &id, "S", "S");
    clone_as(root.path(), &id, "P", "P");
    clone_as(root.path(), &id, "T", "T");
    set_ver(&side(root.path(), &id, "S"), "99");
    fs::remove_dir_all(live(root.path(), &id)).unwrap();
    put_log(root.path(), &id, migrate_log("swap", true));
    expect_open(
        root.path(),
        &id,
        [Some("P"), None, Some("T"), None, None],
        false,
        None,
    );

    let (root, id) = fresh("m-r2-bad-h0");
    clone_as(root.path(), &id, "S", "S");
    clone_as(root.path(), &id, "P", "P");
    set_ver(&side(root.path(), &id, "S"), "99");
    fs::remove_dir_all(live(root.path(), &id)).unwrap();
    put_log(root.path(), &id, migrate_log("swap", false));
    expect_open(
        root.path(),
        &id,
        [Some("P"), None, None, None, None],
        false,
        None,
    );
}

#[test]
fn migrate_swap_after_r3_enters_cleanup() {
    let (root, id) = fresh("m-r3");
    clone_as(root.path(), &id, "P", "P");
    clone_as(root.path(), &id, "T", "T");
    put_log(root.path(), &id, migrate_log("swap", true));
    expect_open(
        root.path(),
        &id,
        [Some("I"), None, Some("P"), None, None],
        false,
        None,
    );
}

#[test]
fn migrate_cleanup_drops_optional_trash() {
    let (root, id) = fresh("m-clean");
    clone_as(root.path(), &id, "P", "P");
    clone_as(root.path(), &id, "T", "T");
    put_log(root.path(), &id, migrate_log("cleanup", true));
    expect_open(
        root.path(),
        &id,
        [Some("I"), None, Some("P"), None, None],
        false,
        None,
    );

    let (root, id) = fresh("m-clean-empty");
    clone_as(root.path(), &id, "P", "P");
    put_log(root.path(), &id, migrate_log("cleanup", true));
    expect_open(
        root.path(),
        &id,
        [Some("I"), None, Some("P"), None, None],
        false,
        None,
    );
}

#[test]
fn restore_swap_before_r1_drops_log_only() {
    let (root, id) = fresh("r-r0");
    clone_as(root.path(), &id, "P", "P");
    clone_as(root.path(), &id, "N", "N");
    put_log(root.path(), &id, restore_log("swap", true));
    expect_open(
        root.path(),
        &id,
        [Some("I"), None, Some("P"), None, Some("N")],
        false,
        None,
    );
}

#[test]
fn restore_swap_after_r1_rolls_trash_back_to_newer() {
    let (root, id) = fresh("r-r1");
    clone_as(root.path(), &id, "P", "P");
    clone_as(root.path(), &id, "T", "T");
    put_log(root.path(), &id, restore_log("swap", true));
    expect_open(
        root.path(),
        &id,
        [Some("I"), None, Some("P"), None, Some("T")],
        false,
        None,
    );
}

#[test]
fn restore_swap_after_r2_continues_when_pre_is_playable() {
    let (donor, donor_id) = fresh("r-r2-donor");
    let root = TestRoot::new("r-r2");
    let id = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
    fs::create_dir_all(root.path().join("worlds")).unwrap();
    for (kind, label) in [("P", "P"), ("N", "N"), ("T", "T")] {
        world_file::copy_dir(&live(donor.path(), &donor_id), &side(root.path(), id, kind)).unwrap();
        fs::write(side(root.path(), id, kind).join("who.txt"), label).unwrap();
    }
    put_log(root.path(), id, restore_log("swap", true));
    expect_open(
        root.path(),
        id,
        [Some("P"), None, None, None, Some("N")],
        false,
        None,
    );
}

#[test]
fn restore_swap_after_r2_is_outside_when_pre_is_newer_than_us() {
    let (root, id) = fresh("r-r2-new");
    clone_as(root.path(), &id, "P", "P");
    clone_as(root.path(), &id, "N", "N");
    set_ver(&side(root.path(), &id, "P"), "99");
    fs::remove_dir_all(live(root.path(), &id)).unwrap();
    put_log(root.path(), &id, restore_log("swap", false));
    expect_open(
        root.path(),
        &id,
        [None, None, Some("P"), None, Some("N")],
        true,
        Some("對不上"),
    );
}

#[test]
fn restore_swap_after_r3_and_cleanup_finish() {
    let (root, id) = fresh("r-r3");
    clone_as(root.path(), &id, "N", "N");
    clone_as(root.path(), &id, "T", "T");
    put_log(root.path(), &id, restore_log("swap", true));
    expect_open(
        root.path(),
        &id,
        [Some("I"), None, None, None, Some("N")],
        false,
        None,
    );

    let (root, id) = fresh("r-clean");
    clone_as(root.path(), &id, "N", "N");
    clone_as(root.path(), &id, "T", "T");
    put_log(root.path(), &id, restore_log("cleanup", true));
    expect_open(
        root.path(),
        &id,
        [Some("I"), None, None, None, Some("N")],
        false,
        None,
    );

    let (root, id) = fresh("r-clean-no-t");
    clone_as(root.path(), &id, "N", "N");
    put_log(root.path(), &id, restore_log("cleanup", false));
    expect_open(
        root.path(),
        &id,
        [Some("I"), None, None, None, Some("N")],
        false,
        None,
    );
}

#[test]
fn outside_table_does_not_touch_directories() {
    let (root, id) = fresh("out-t");
    clone_as(root.path(), &id, "T", "T");
    expect_open(
        root.path(),
        &id,
        [Some("I"), None, None, Some("T"), None],
        false,
        Some("對不上"),
    );

    let (root, id) = fresh("out-s");
    clone_as(root.path(), &id, "S", "S");
    fs::remove_dir_all(live(root.path(), &id)).unwrap();
    expect_open(
        root.path(),
        &id,
        [None, Some("S"), None, None, None],
        false,
        Some("對不上"),
    );

    let (root, id) = fresh("out-log");
    fs::write(side(root.path(), &id, "L"), b"not json").unwrap();
    expect_open(
        root.path(),
        &id,
        [Some("I"), None, None, None, None],
        true,
        Some("對不上"),
    );

    let (root, id) = fresh("out-op");
    put_log(
        root.path(),
        &id,
        serde_json::json!({"op": "shrink", "stage": "build"}),
    );
    expect_open(
        root.path(),
        &id,
        [Some("I"), None, None, None, None],
        true,
        Some("對不上"),
    );

    let (root, id) = fresh("out-h");
    clone_as(root.path(), &id, "P", "P");
    put_log(root.path(), &id, migrate_log("build", false));
    expect_open(
        root.path(),
        &id,
        [Some("I"), None, Some("P"), None, None],
        true,
        Some("對不上"),
    );
}

#[test]
fn no_log_deletes_staging_only_when_live_or_pre_remains() {
    let (root, id) = fresh("s-with-i");
    clone_as(root.path(), &id, "S", "S");
    expect_open(
        root.path(),
        &id,
        [Some("I"), None, None, None, None],
        false,
        None,
    );

    let (donor, donor_id) = fresh("s-only-p-donor");
    let root = TestRoot::new("s-only-p");
    let id = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
    fs::create_dir_all(root.path().join("worlds")).unwrap();
    world_file::copy_dir(&live(donor.path(), &donor_id), &side(root.path(), id, "P")).unwrap();
    fs::write(side(root.path(), id, "P").join("who.txt"), "P").unwrap();
    world_file::copy_dir(&live(donor.path(), &donor_id), &side(root.path(), id, "S")).unwrap();
    fs::write(side(root.path(), id, "S").join("who.txt"), "S").unwrap();
    let opened = open_world(root.path(), id).unwrap();
    match opened {
        OpenWorld::NeedsRepair { message, .. } => {
            assert!(message.contains("主資料夾"), "{message}")
        }
        other => panic!("{other:?}"),
    }
    assert!(!side(root.path(), id, "S").exists());
    assert_eq!(who(&side(root.path(), id, "P")), "P");
}

#[test]
fn missing_live_with_only_backup_is_needs_repair() {
    let (root, id) = fresh("missing-p");
    clone_as(root.path(), &id, "P", "P");
    fs::remove_dir_all(live(root.path(), &id)).unwrap();
    expect_open(
        root.path(),
        &id,
        [None, None, Some("P"), None, None],
        false,
        Some("主資料夾"),
    );
}

#[test]
fn rename_failure_leaves_directories_and_retries_next_open() {
    let (root, id) = fresh("rename");
    clone_as(root.path(), &id, "S", "S");
    clone_as(root.path(), &id, "T", "T");
    put_log(root.path(), &id, migrate_log("swap", true));
    {
        let _guard = RenameFailGuard::fail(1);
        expect_open(
            root.path(),
            &id,
            [Some("I"), Some("S"), None, Some("T"), None],
            true,
            Some("改名失敗"),
        );
    }
    expect_open(
        root.path(),
        &id,
        [Some("I"), None, Some("T"), None, None],
        false,
        None,
    );
}

#[test]
fn onedrive_recreating_live_during_swap_is_outside() {
    let (root, id) = fresh("cloud-h0");
    clone_as(root.path(), &id, "S", "S");
    clone_as(root.path(), &id, "P", "P");
    put_log(root.path(), &id, migrate_log("swap", false));
    expect_open(
        root.path(),
        &id,
        [Some("I"), Some("S"), Some("P"), None, None],
        true,
        Some("對不上"),
    );

    let (root, id) = fresh("cloud-h1");
    clone_as(root.path(), &id, "S", "S");
    clone_as(root.path(), &id, "P", "P");
    clone_as(root.path(), &id, "T", "T");
    put_log(root.path(), &id, migrate_log("swap", true));
    expect_open(
        root.path(),
        &id,
        [Some("I"), Some("S"), Some("P"), Some("T"), None],
        true,
        Some("對不上"),
    );
}

#[test]
fn fake_migration_chain_keeps_only_the_latest_pre() {
    let (root, id) = fresh("chain");
    let _steps = StepOverride::set(vec![(1, 2), (2, 3)]);
    {
        let _current = CurrentOverride::set(2);
        assert_eq!(
            open_world(root.path(), &id).unwrap(),
            OpenWorld::Migrated { from: 1, to: 2 }
        );
        assert_eq!(
            read_format(&live(root.path(), &id)).version,
            FormatVersion::Known(2)
        );
        assert_eq!(
            read_format(&side(root.path(), &id, "P")).version,
            FormatVersion::Known(1)
        );
        let note = fs::read_to_string(live(root.path(), &id).join("migration-log.txt")).unwrap();
        assert_eq!(note, "1->2\n");
    }
    {
        let _current = CurrentOverride::set(3);
        assert_eq!(
            open_world(root.path(), &id).unwrap(),
            OpenWorld::Migrated { from: 2, to: 3 }
        );
        assert_eq!(
            read_format(&side(root.path(), &id, "P")).version,
            FormatVersion::Known(2)
        );
        assert!(!side(root.path(), &id, "T").exists());
        assert!(!side(root.path(), &id, "L").exists());
    }
}

#[test]
fn migration_build_failure_leaves_the_original_and_retries() {
    let (root, id) = fresh("convert-fail");
    let _current = CurrentOverride::set(2);
    match open_world(root.path(), &id).unwrap() {
        OpenWorld::NeedsRepair { message, .. } => {
            assert!(message.contains("格式轉換沒有完成"), "{message}")
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        read_format(&live(root.path(), &id)).version,
        FormatVersion::Known(1)
    );
    assert!(!side(root.path(), &id, "S").exists());
    assert!(!side(root.path(), &id, "L").exists());

    let _steps = StepOverride::set(vec![(1, 2)]);
    assert_eq!(
        open_world(root.path(), &id).unwrap(),
        OpenWorld::Migrated { from: 1, to: 2 }
    );
}

#[test]
fn restore_backup_then_open_and_rejects_bad_preconditions() {
    let (root, id) = fresh("restore");
    let _steps = StepOverride::set(vec![(1, 2)]);
    {
        let _current = CurrentOverride::set(2);
        open_world(root.path(), &id).unwrap();
    }
    let _current = CurrentOverride::set(1);
    match open_world(root.path(), &id).unwrap() {
        OpenWorld::ReadOnly {
            format_version,
            backup_available,
            ..
        } => {
            assert_eq!(format_version, Some(2));
            assert!(backup_available);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        restore_world_backup(root.path(), &id).unwrap(),
        OpenWorld::Ready
    );
    assert_eq!(
        read_format(&live(root.path(), &id)).version,
        FormatVersion::Known(1)
    );
    assert!(side(root.path(), &id, "N").is_dir());
    assert_eq!(
        read_format(&side(root.path(), &id, "N")).version,
        FormatVersion::Known(2)
    );

    let err = restore_world_backup(root.path(), &id)
        .unwrap_err()
        .to_string();
    assert!(err.contains("不是唯讀"), "{err}");

    let (root, id) = fresh("restore-none");
    set_ver(&live(root.path(), &id), "99");
    let err = restore_world_backup(root.path(), &id)
        .unwrap_err()
        .to_string();
    assert!(err.contains("沒有可用的轉換前備份"), "{err}");
}

#[test]
fn open_world_is_busy_while_a_write_permit_is_held() {
    let (root, id) = fresh("busy");
    let permit = super::super::world_lock::world_write_permit(&id).unwrap();
    assert_eq!(open_world(root.path(), &id).unwrap(), OpenWorld::Busy);
    drop(permit);
    assert_eq!(open_world(root.path(), &id).unwrap(), OpenWorld::Ready);
}

#[test]
fn unknown_id_is_not_found_and_list_skips_illegal_directories() {
    let root = TestRoot::new("unknown");
    let err = open_world(root.path(), "01ARZ3NDEKTSV4RRFFQ69G5FAV")
        .unwrap_err()
        .to_string();
    assert!(err.contains("找不到這張桌"), "{err}");

    let (root, id) = fresh("list");
    fs::create_dir_all(root.path().join("worlds/not-an-id")).unwrap();
    fs::create_dir_all(root.path().join("worlds/.tt-noise")).unwrap();
    let listed = list_worlds(root.path()).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, id);
    assert_eq!(listed[0].name, "霧港");
    assert!(!listed[0].read_only);
    assert!(!listed[0].needs_repair);

    set_ver(&live(root.path(), &id), "99");
    let listed = list_worlds(root.path()).unwrap();
    assert!(listed[0].read_only);
    assert!(!listed[0].needs_repair);

    let _current = CurrentOverride::set(4);
    set_ver(&live(root.path(), &id), "2");
    let listed = list_worlds(root.path()).unwrap();
    assert!(
        !listed[0].read_only,
        "older than current is still playable once migrated"
    );
    assert!(!listed[0].needs_repair);
}

#[test]
fn list_names_a_repair_world_from_pre_and_skips_recover_when_busy() {
    let (root, id) = fresh("list-pre");
    clone_as(root.path(), &id, "P", "P");
    fs::remove_dir_all(live(root.path(), &id)).unwrap();
    let listed = list_worlds(root.path()).unwrap();
    assert_eq!(listed[0].id, id);
    assert_eq!(listed[0].name, "霧港");
    assert!(listed[0].needs_repair);

    let (root, id) = fresh("list-busy");
    clone_as(root.path(), &id, "S", "S");
    let permit = super::super::world_lock::world_write_permit(&id).unwrap();
    list_worlds(root.path()).unwrap();
    assert!(side(root.path(), &id, "S").exists());
    drop(permit);
    list_worlds(root.path()).unwrap();
    assert!(!side(root.path(), &id, "S").exists());
}

#[test]
fn read_only_world_blocks_every_write_category_but_delete_still_works() {
    let (root, id) = fresh("gate");
    let mut card = character_card("01ARZ3NDEKTSV4RRFFQ69G5FB0", "旅人");
    card.id = "01ARZ3NDEKTSV4RRFFQ69G5FB0".to_owned();
    write_character(root.path(), &id, &card).unwrap();
    let png = b"\x89PNG\r\n\x1a\nextra";
    save_character_image(root.path(), &id, &card.id, png).unwrap();
    set_ver(&live(root.path(), &id), "99");

    let mut state = read_state(root.path(), &id).unwrap();
    state.name = "不該寫上".to_owned();
    let err = write_state(root.path(), &id, &state)
        .unwrap_err()
        .to_string();
    assert!(err.contains("唯讀"), "{err}");
    assert_eq!(read_state(root.path(), &id).unwrap().name, "霧港");

    let err = append_transcript(root.path(), &id, 0, &event("不該出現"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("唯讀"), "{err}");
    assert!(!live(root.path(), &id).join("transcript/0.jsonl").exists());

    card.name = "改名".to_owned();
    let err = write_character(root.path(), &id, &card)
        .unwrap_err()
        .to_string();
    assert!(err.contains("唯讀"), "{err}");

    let err = upsert_worldbook_entry(root.path(), &id, worldbook_entry(1, "霧"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("唯讀"), "{err}");

    let err = save_character_image(root.path(), &id, &card.id, png)
        .unwrap_err()
        .to_string();
    assert!(err.contains("唯讀"), "{err}");

    let err = write_world_md(root.path(), &id, "改設定")
        .unwrap_err()
        .to_string();
    assert!(err.contains("唯讀"), "{err}");

    append_log(
        root.path(),
        &id,
        0,
        &[Record {
            kind: RecordKind::Error,
            path: "hp".to_owned(),
            detail: "x".to_owned(),
        }],
    );
    assert!(!live(root.path(), &id).join("mechanism-log.jsonl").exists());

    let err = world_file::commit_world_write(&live(root.path(), &id).join("lanes.json"), b"{}")
        .unwrap_err()
        .to_string();
    assert!(err.contains("唯讀"), "{err}");

    let (root, id) = fresh("gate-dirty");
    put_log(root.path(), &id, migrate_log("build", false));
    let err = write_world_md(root.path(), &id, "x")
        .unwrap_err()
        .to_string();
    assert!(err.contains("轉換或需要修復"), "{err}");

    let (root, id) = fresh("gate-reclaim");
    set_ver(&live(root.path(), &id), "99");
    let err = reclaim_world_if_empty(root.path(), &id)
        .unwrap_err()
        .to_string();
    assert!(err.contains("唯讀"), "{err}");
    assert!(live(root.path(), &id).is_dir());

    let (root, id) = fresh("gate-delete");
    clone_as(root.path(), &id, "P", "P");
    set_ver(&live(root.path(), &id), "99");
    delete_world(root.path(), &id).unwrap();
    assert!(!live(root.path(), &id).exists());
    assert!(!side(root.path(), &id, "P").exists());
}

#[test]
fn append_transcript_errors_when_state_cannot_be_read() {
    let (root, id) = fresh("no-state");
    fs::remove_file(live(root.path(), &id).join("state.json")).unwrap();
    let err = append_transcript(root.path(), &id, 0, &event("嗨"))
        .unwrap_err()
        .to_string();
    assert!(!err.is_empty());
    assert!(!live(root.path(), &id).join("transcript/0.jsonl").exists());
}

#[test]
fn read_world_readonly_skips_unreadable_lines_and_falls_back_to_max_scene() {
    let (root, id) = fresh("ro-read");
    let transcript = live(root.path(), &id).join("transcript");
    fs::write(
        transcript.join("0.jsonl"),
        concat!(
            "{\"speaker_name\":\"甲\",\"text\":\"嗨\",\"kind\":\"dialogue\"}\n",
            "not-json\n",
            "{\"speaker_name\":\"缺\"}\n",
            "\n",
            "{\"speaker_name\":\"乙\",\"text\":\"喔\",\"kind\":\"narration\"}\n",
        ),
    )
    .unwrap();
    let world = read_world_readonly(root.path(), &id).unwrap();
    assert_eq!(world.scene, 0);
    assert_eq!(world.skipped, 2);
    assert_eq!(world.events.len(), 2);
    assert_eq!(world.events[0].text, "嗨");
    assert_eq!(world.events[1].kind, "narration");

    let mut state = read_state(root.path(), &id).unwrap();
    state.current_scene = 9;
    write_state(root.path(), &id, &state).unwrap();
    fs::write(
        transcript.join("3.jsonl"),
        "{\"speaker_name\":\"丙\",\"text\":\"三\",\"kind\":\"system\"}\n",
    )
    .unwrap();
    let world = read_world_readonly(root.path(), &id).unwrap();
    assert_eq!(world.scene, 3);
    assert_eq!(world.events[0].text, "三");
}

#[test]
fn leftover_log_tmp_does_not_change_recovery_and_is_removed() {
    let (root, id) = fresh("log-tmp");
    let tmp = root
        .path()
        .join("worlds")
        .join(format!(".tt-op-{id}.json.tmp"));
    // 若把暫存當成正式日誌，had_pre 為真但沒有 P，會落到表外。
    fs::write(
        &tmp,
        br#"{"op":"migrate","stage":"swap","from":1,"to":9,"had_pre":true}"#,
    )
    .unwrap();
    assert_eq!(open_world(root.path(), &id).unwrap(), OpenWorld::Ready);
    assert!(!tmp.exists());
    assert_eq!(who(&live(root.path(), &id)), "I");
    assert!(!side(root.path(), &id, "L").exists());
}

#[test]
fn recovery_io_error_repairs_one_world_and_lists_the_other() {
    let (root, bad) = fresh("recover-io");
    let good = create_world(root.path(), "好桌").unwrap();
    clone_as(root.path(), &bad, "S", "S");
    clone_as(root.path(), &bad, "P", "P");

    {
        let _guard = RemoveFailGuard::fail(1);
        let listed = list_worlds(root.path()).unwrap();
        assert_eq!(listed.len(), 2);
        let bad_row = listed.iter().find(|row| row.id == bad).unwrap();
        assert!(bad_row.needs_repair);
        let good_row = listed.iter().find(|row| row.id == good).unwrap();
        assert!(!good_row.needs_repair);
        assert!(!good_row.read_only);
    }
    assert_eq!(who(&live(root.path(), &bad)), "I");
    assert_eq!(who(&side(root.path(), &bad, "S")), "S");
    assert_eq!(who(&side(root.path(), &bad, "P")), "P");

    {
        let _guard = RemoveFailGuard::fail(1);
        match open_world(root.path(), &bad).unwrap() {
            OpenWorld::NeedsRepair { message, .. } => {
                assert!(message.contains("刪除失敗"), "{message}");
            }
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(who(&live(root.path(), &bad)), "I");
    assert_eq!(who(&side(root.path(), &bad, "S")), "S");
    assert_eq!(who(&side(root.path(), &bad, "P")), "P");
}
