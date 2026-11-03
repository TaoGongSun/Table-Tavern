use super::*;
use crate::data::test_support::*;
use crate::data::*;

fn keys(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("{i:016x}")).collect()
}

#[test]
fn reading_is_an_idempotent_union_with_a_position_per_scene() {
    let root = TestRoot::new("vn-progress-union");
    let world_id = create_world(root.path(), "進度桌").unwrap();
    let path = root.path();
    record_reading(path, &world_id, &keys(3), Some((2, "e1#0"))).unwrap();
    record_reading(path, &world_id, &keys(4), Some((2, "e1#3"))).unwrap();
    let progress = read_progress(path, &world_id).unwrap();
    assert_eq!(progress.read, keys(4));
    assert_eq!(
        progress.positions.get("2").map(String::as_str),
        Some("e1#3")
    );
}

#[test]
fn reading_keeps_the_newer_half_past_the_cap_and_rejects_bad_keys() {
    let root = TestRoot::new("vn-progress-cap");
    let world_id = create_world(root.path(), "進度桌").unwrap();
    let path = root.path();
    record_reading(path, &world_id, &keys(READ_CAP + 1), None).unwrap();
    let read = read_progress(path, &world_id).unwrap().read;
    assert_eq!(read.len(), READ_CAP / 2);
    assert_eq!(read.last(), keys(READ_CAP + 1).last());
    assert!(record_reading(path, &world_id, &["x".repeat(KEY_MAX + 1)], None).is_err());
    assert!(record_reading(path, &world_id, &[String::new()], None).is_err());
}

#[test]
fn a_broken_file_reads_as_empty_and_is_rewritten() {
    let root = TestRoot::new("vn-progress-broken");
    let world_id = create_world(root.path(), "進度桌").unwrap();
    let path = root.path();
    let file = progress_path(path, &world_id).unwrap();
    commit_world_write(&file, b"{not json").unwrap();
    assert_eq!(read_progress(path, &world_id).unwrap(), Progress::default());
    record_reading(path, &world_id, &keys(1), None).unwrap();
    assert_eq!(read_progress(path, &world_id).unwrap().read, keys(1));
}

#[test]
fn saves_stop_at_the_cap_and_delete_by_id() {
    let root = TestRoot::new("vn-progress-saves");
    let world_id = create_world(root.path(), "進度桌").unwrap();
    let path = root.path();
    let save = |id: &str| Save {
        id: id.to_owned(),
        name: id.to_owned(),
        scene: 0,
        event_id: "e".to_owned(),
        cell: 0,
        chapter: 1,
        ts: String::new(),
        preview: String::new(),
    };
    for n in 0..SAVES_CAP {
        add_save(path, &world_id, save(&n.to_string())).unwrap();
    }
    let error = add_save(path, &world_id, save("over"))
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        UiMsg::VnSavesFull {
            cap: SAVES_CAP as u32
        }
        .to_string()
    );
    delete_save(path, &world_id, "0").unwrap();
    assert_eq!(
        read_progress(path, &world_id).unwrap().saves.len(),
        SAVES_CAP - 1
    );
}
