//! 包 2b 非 message 層的測試：六層落檔與固定身分、身分檔名雜湊與原 ID 核對、根目錄原子寫與更新閘門、
//! 跨桌 rev 衝突、桌世代、上限、故障注入。
use super::*;
use crate::data::test_support::TestRoot;
use crate::data::{self, RenameFailGuard, WriteFailGuard};

fn world(root: &TestRoot) -> String {
    data::create_world(root.path(), "桌").unwrap()
}

fn generation(root: &TestRoot, world_id: &str) -> u64 {
    with_commit(root.path(), world_id, message_vars::generation)
}

/// 寫一層，期望成功，回新 rev
fn put(
    root: &TestRoot,
    world_id: &str,
    layer: Layer,
    id: Option<&str>,
    expected: Option<&str>,
    json: &str,
) -> String {
    let generation = generation(root, world_id);
    match write_layer(root.path(), world_id, layer, id, generation, expected, json).unwrap() {
        LayerWrite::LayerOk { rev } => rev,
        other => panic!("寫入沒成功：{other:?}"),
    }
}

fn try_put(
    root: &TestRoot,
    world_id: &str,
    layer: Layer,
    id: Option<&str>,
    expected: Option<&str>,
    json: &str,
) -> LayerWrite {
    let generation = generation(root, world_id);
    write_layer(root.path(), world_id, layer, id, generation, expected, json).unwrap()
}

fn read(root: &TestRoot, world_id: &str, layer: Layer, id: Option<&str>) -> LayerDoc {
    read_layer(root.path(), world_id, layer, id).unwrap()
}

#[test]
fn six_layers_round_trip_in_their_fixed_places() {
    let root = TestRoot::new("layers-place");
    let w = world(&root);
    let r = root.path();
    put(&root, &w, Layer::Chat, None, None, r#"{"z":1,"a":2}"#);
    put(
        &root,
        &w,
        Layer::Character,
        Some("card-1"),
        None,
        r#"{"c":1}"#,
    );
    put(&root, &w, Layer::Global, None, None, r#"{"g":1}"#);
    put(&root, &w, Layer::Preset, None, None, r#"{"p":1}"#);
    put(&root, &w, Layer::Script, Some("腳本/1"), None, r#"{"s":1}"#);
    put(
        &root,
        &w,
        Layer::Extension,
        Some("ext.a"),
        None,
        r#"{"e":1}"#,
    );

    let world_dir = r.join("worlds").join(&w).join("card-vars");
    assert!(world_dir.join("chat.json").is_file());
    assert!(world_dir
        .join("character")
        .join(format!("{}.json", identity_name("card-1")))
        .is_file());
    assert!(world_dir
        .join("script")
        .join(format!("{}.json", identity_name("腳本/1")))
        .is_file());
    assert!(r.join("card-vars/global.json").is_file());
    assert!(r.join("card-vars/preset.json").is_file());
    assert!(r
        .join("card-vars/extension")
        .join(format!("{}.json", identity_name("ext.a")))
        .is_file());

    // 往返保留鍵順序
    assert_eq!(read(&root, &w, Layer::Chat, None).vars, r#"{"z":1,"a":2}"#);
    assert_eq!(read(&root, &w, Layer::Global, None).vars, r#"{"g":1}"#);
    assert_eq!(read(&root, &w, Layer::Preset, None).vars, r#"{"p":1}"#);
    assert_eq!(
        read(&root, &w, Layer::Script, Some("腳本/1")).vars,
        r#"{"s":1}"#
    );
    assert_eq!(
        read(&root, &w, Layer::Extension, Some("ext.a")).vars,
        r#"{"e":1}"#
    );
    // 沒寫過的層：空表、版本「無」
    let empty = read(&root, &w, Layer::Script, Some("沒有"));
    assert_eq!((empty.rev, empty.vars.as_str()), (None, "{}"));
}

#[test]
fn identity_names_are_hashed_and_file_keeps_the_original_id() {
    let root = TestRoot::new("layers-identity");
    let w = world(&root);
    let id = "../../escape";
    put(&root, &w, Layer::Script, Some(id), None, r#"{"s":1}"#);
    assert_eq!(identity_name(id).len(), 32);
    assert!(identity_name(id).chars().all(|c| c.is_ascii_hexdigit()));
    assert!(!root.path().join("escape.json").exists());
    let path = root
        .path()
        .join("worlds")
        .join(&w)
        .join("card-vars/script")
        .join(format!("{}.json", identity_name(id)));
    let file: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(file["id"], id);
    // 檔內 ID 對不上（雜湊撞名或被改）當不存在
    let mut tampered = file.clone();
    tampered["id"] = "別的".into();
    std::fs::write(&path, tampered.to_string()).unwrap();
    // 身分衝突不是不存在：讀回報錯，寫入（含 null rev）拒絕且原檔位元組不變
    assert!(read_layer(root.path(), &w, Layer::Script, Some(id)).is_err());
    let before = std::fs::read(&path).unwrap();
    assert!(matches!(
        try_put(&root, &w, Layer::Script, Some(id), None, "{}"),
        LayerWrite::Rejected { ref code, .. } if code == "id-mismatch"
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    // 長度與空 ID
    assert!(read_layer(root.path(), &w, Layer::Script, Some("")).is_err());
    let long = "字".repeat(257);
    assert!(read_layer(root.path(), &w, Layer::Extension, Some(&long)).is_err());
    assert!(read_layer(root.path(), &w, Layer::Script, Some(&"字".repeat(256))).is_ok());
    assert!(read_layer(root.path(), &w, Layer::Script, None).is_err());
}

#[test]
fn rev_conflict_returns_stale_with_authority_across_worlds() {
    let root = TestRoot::new("layers-rev");
    let a = world(&root);
    let b = data::create_world(root.path(), "另一桌").unwrap();
    let first = put(&root, &a, Layer::Global, None, None, r#"{"n":1}"#);
    // 另一桌改了 global
    let second = put(&root, &b, Layer::Global, None, Some(&first), r#"{"n":2}"#);
    assert_ne!(first, second);
    // 這桌還拿舊 rev 寫：stale 附新值
    match try_put(&root, &a, Layer::Global, None, Some(&first), r#"{"n":3}"#) {
        LayerWrite::Stale { found, rev, table } => {
            assert!(found);
            assert_eq!(rev.as_deref(), Some(second.as_str()));
            assert_eq!(table.as_deref(), Some(r#"{"n":2}"#));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(read(&root, &a, Layer::Global, None).vars, r#"{"n":2}"#);
    // 檔案不存在卻帶了 rev、檔案存在卻說無：都 stale
    assert!(matches!(
        try_put(&root, &a, Layer::Preset, None, Some("X"), "{}"),
        LayerWrite::Stale { rev: None, .. }
    ));
    assert!(matches!(
        try_put(&root, &a, Layer::Global, None, None, "{}"),
        LayerWrite::Stale { .. }
    ));
}

#[test]
fn world_generation_change_makes_world_layers_stale() {
    let root = TestRoot::new("layers-generation");
    let w = world(&root);
    let rev = put(&root, &w, Layer::Chat, None, None, r#"{"a":1}"#);
    let old = generation(&root, &w);
    message_vars::world_swapped(root.path(), &w);
    let result = write_layer(
        root.path(),
        &w,
        Layer::Chat,
        None,
        old,
        Some(&rev),
        r#"{"a":2}"#,
    )
    .unwrap();
    assert!(matches!(result, LayerWrite::Stale { .. }));
    assert_eq!(read(&root, &w, Layer::Chat, None).vars, r#"{"a":1}"#);
}

#[test]
fn limits_reject_whole_write_and_leave_file_untouched() {
    let root = TestRoot::new("layers-limits");
    let w = world(&root);
    let rev = put(&root, &w, Layer::Chat, None, None, r#"{"a":1}"#);
    for (bad, code) in [
        ("[1]", "not-object"),
        (r#"{"x":1e999}"#, "invalid-json"),
        (r#"{"":1}"#, "empty-key"),
    ] {
        match try_put(&root, &w, Layer::Chat, None, Some(&rev), bad) {
            LayerWrite::Rejected {
                code: got,
                rev: now,
                table,
                ..
            } => {
                assert_eq!(got, code);
                assert_eq!(now.as_deref(), Some(rev.as_str()));
                assert_eq!(table.as_deref(), Some(r#"{"a":1}"#));
            }
            other => panic!("{other:?}"),
        }
    }
    let deep = format!("{}1{}", "{\"a\":".repeat(40), "}".repeat(40));
    assert!(matches!(
        try_put(&root, &w, Layer::Chat, None, Some(&rev), &deep),
        LayerWrite::Rejected { .. }
    ));
    // 危險鍵存成自有屬性、照原樣往返
    let text = r#"{"__proto__":{"x":1},"constructor":2,"prototype":3}"#;
    let next = put(&root, &w, Layer::Chat, None, Some(&rev), text);
    assert_eq!(read(&root, &w, Layer::Chat, None).vars, text);
    assert_ne!(next, rev);
}

#[test]
fn atomic_write_failures_leave_the_old_file() {
    let root = TestRoot::new("layers-atomic");
    let w = world(&root);
    let rev = put(&root, &w, Layer::Global, None, None, r#"{"v":1}"#);
    let generation = generation(&root, &w);
    {
        let _fail = RenameFailGuard::fail(1);
        assert!(write_layer(
            root.path(),
            &w,
            Layer::Global,
            None,
            generation,
            Some(&rev),
            r#"{"v":2}"#
        )
        .is_err());
    }
    {
        let _fail = WriteFailGuard::partial_ending("global.json.tmp", 1);
        assert!(write_layer(
            root.path(),
            &w,
            Layer::Global,
            None,
            generation,
            Some(&rev),
            r#"{"v":2}"#
        )
        .is_err());
    }
    let doc = read(&root, &w, Layer::Global, None);
    assert_eq!(
        (doc.rev.as_deref(), doc.vars.as_str()),
        (Some(rev.as_str()), r#"{"v":1}"#)
    );
    assert!(!root.path().join("card-vars/global.json.tmp").exists());
    // 世界內的層同樣
    let chat = put(&root, &w, Layer::Chat, None, None, r#"{"v":1}"#);
    {
        let _fail = RenameFailGuard::fail(1);
        assert!(write_layer(
            root.path(),
            &w,
            Layer::Chat,
            None,
            generation,
            Some(&chat),
            r#"{"v":2}"#
        )
        .is_err());
    }
    assert_eq!(read(&root, &w, Layer::Chat, None).vars, r#"{"v":1}"#);
    // 失敗後同 rev 還能重試成功
    put(&root, &w, Layer::Chat, None, Some(&chat), r#"{"v":3}"#);
}

#[test]
fn update_gate_blocks_root_writes_without_touching_disk() {
    let root = TestRoot::new("layers-gate");
    let w = world(&root);
    let rev = put(&root, &w, Layer::Global, None, None, r#"{"v":1}"#);
    let generation = generation(&root, &w);
    let _gate = crate::data::world_lock::GateOverride::raised();
    let error = write_layer(
        root.path(),
        &w,
        Layer::Global,
        None,
        generation,
        Some(&rev),
        r#"{"v":2}"#,
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        crate::ui_msg::UiMsg::UpdateGateClosed.to_string()
    );
    assert_eq!(read(&root, &w, Layer::Global, None).vars, r#"{"v":1}"#);
    assert!(write_layer(
        root.path(),
        &w,
        Layer::Extension,
        Some("x"),
        generation,
        None,
        "{}"
    )
    .is_err());
    assert!(!root.path().join("card-vars/extension").exists());
}

#[test]
fn load_lists_script_and_extension() {
    let root = TestRoot::new("layers-load");
    let w = world(&root);
    put(&root, &w, Layer::Script, Some("s1"), None, r#"{"a":1}"#);
    put(&root, &w, Layer::Script, Some("s2"), None, r#"{"a":2}"#);
    put(&root, &w, Layer::Extension, Some("e1"), None, r#"{"b":1}"#);
    put(&root, &w, Layer::Character, Some("c1"), None, r#"{"c":1}"#);
    let entries = load_layers(root.path(), &w, Some("c1")).unwrap();
    assert!(entries.iter().all(|entry| entry.error.is_none()));
    let keys: Vec<&str> = entries.iter().map(|entry| entry.key.as_str()).collect();
    // script 依檔名（雜湊）排序，順序不保證：固定層在前，script 與 extension 只比成員
    assert_eq!(&keys[..4], ["chat", "character:c1", "global", "preset"]);
    let mut rest: Vec<&str> = keys[4..].to_vec();
    rest.sort_unstable();
    assert_eq!(rest, ["extension:e1", "script:s1", "script:s2"]);
    let chat = &entries[0];
    assert_eq!((chat.doc.rev.clone(), chat.doc.vars.as_str()), (None, "{}"));
    let none = load_layers(root.path(), &w, None).unwrap();
    assert!(none
        .iter()
        .all(|entry| !entry.key.starts_with("character:")));
}

#[test]
fn corrupt_or_foreign_files_are_errors_not_empty_and_writes_keep_the_original_bytes() {
    let root = TestRoot::new("layers-corrupt");
    let w = world(&root);
    let rev = put(&root, &w, Layer::Chat, None, None, r#"{"a":1}"#);
    let world_dir = root.path().join("worlds").join(&w).join("card-vars");
    for (layer, id, path) in [
        (Layer::Chat, None, world_dir.join("chat.json")),
        (
            Layer::Global,
            None,
            root.path().join("card-vars/global.json"),
        ),
        (
            Layer::Preset,
            None,
            root.path().join("card-vars/preset.json"),
        ),
    ] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{壞掉").unwrap();
        assert!(read_layer(root.path(), &w, layer, id).is_err());
        for expected in [None, Some(rev.as_str())] {
            assert!(matches!(
                try_put(&root, &w, layer, id, expected, "{}"),
                LayerWrite::Rejected { ref code, found: false, .. } if code == "corrupt-file"
            ));
        }
        assert_eq!(std::fs::read(&path).unwrap(), "{壞掉".as_bytes());
    }
    // 載入時帶 error（未知不等於不存在）；沒壞的層照常
    let entries = load_layers(root.path(), &w, None).unwrap();
    let by = |key: &str| entries.iter().find(|entry| entry.key == key).unwrap();
    assert!(
        by("chat").error.is_some() && by("global").error.is_some() && by("preset").error.is_some()
    );
    // 擴充與腳本資料夾裡的壞檔、撞名檔：整個類別標錯
    std::fs::create_dir_all(root.path().join("card-vars/extension")).unwrap();
    std::fs::write(
        root.path()
            .join("card-vars/extension/ffffffffffffffffffffffffffffffff.json"),
        "{}",
    )
    .unwrap();
    let entries = load_layers(root.path(), &w, None).unwrap();
    assert!(entries
        .iter()
        .any(|entry| entry.key == "extension:" && entry.error.is_some()));
    // 讀不了資料夾（路徑是檔案）同樣標錯，不當成沒有
    std::fs::remove_dir_all(root.path().join("card-vars/extension")).unwrap();
    std::fs::write(root.path().join("card-vars/extension"), "x").unwrap();
    let entries = load_layers(root.path(), &w, None).unwrap();
    assert!(entries
        .iter()
        .any(|entry| entry.key == "extension:" && entry.error.is_some()));
    // 修好之後重讀恢復
    std::fs::remove_file(root.path().join("card-vars/extension")).unwrap();
    std::fs::remove_file(root.path().join("card-vars/global.json")).unwrap();
    let entries = load_layers(root.path(), &w, None).unwrap();
    assert!(entries
        .iter()
        .all(|entry| entry.key == "chat" || entry.key == "preset" || entry.error.is_none()));
    assert_eq!(read(&root, &w, Layer::Global, None).rev, None);
    put(&root, &w, Layer::Global, None, None, r#"{"ok":1}"#);
}

#[test]
fn root_layers_check_world_generation_too() {
    let root = TestRoot::new("layers-root-generation");
    let w = world(&root);
    let globals = [
        (Layer::Global, None),
        (Layer::Preset, None),
        (Layer::Extension, Some("e1")),
    ];
    let revs: Vec<String> = globals
        .iter()
        .map(|(layer, id)| put(&root, &w, *layer, *id, None, r#"{"v":1}"#))
        .collect();
    let old = generation(&root, &w);
    message_vars::world_swapped(root.path(), &w);
    for ((layer, id), rev) in globals.iter().zip(&revs) {
        let result =
            write_layer(root.path(), &w, *layer, *id, old, Some(rev), r#"{"v":2}"#).unwrap();
        assert!(
            matches!(result, LayerWrite::Stale { found: true, .. }),
            "{layer:?}"
        );
        let doc = read(&root, &w, *layer, *id);
        assert_eq!(
            (doc.rev.as_deref(), doc.vars.as_str()),
            (Some(rev.as_str()), r#"{"v":1}"#)
        );
    }
}

#[test]
fn concurrent_writes_with_the_same_rev_exactly_one_wins() {
    let root = TestRoot::new("layers-race");
    let a = world(&root);
    let b = data::create_world(root.path(), "另一桌").unwrap();
    let rev = put(&root, &a, Layer::Global, None, None, r#"{"n":0}"#);
    let path = root.path().to_path_buf();
    let handles: Vec<_> = [(a.clone(), 1), (b.clone(), 2)]
        .into_iter()
        .map(|(world_id, n)| {
            let path = path.clone();
            let rev = rev.clone();
            std::thread::spawn(move || {
                let generation = with_commit(&path, &world_id, message_vars::generation);
                write_layer(
                    &path,
                    &world_id,
                    Layer::Global,
                    None,
                    generation,
                    Some(&rev),
                    &format!(r#"{{"n":{n}}}"#),
                )
                .unwrap()
            })
        })
        .collect();
    let results: Vec<LayerWrite> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let winners = results
        .iter()
        .filter(|r| matches!(r, LayerWrite::LayerOk { .. }))
        .count();
    assert_eq!(winners, 1);
    let current = read(&root, &a, Layer::Global, None);
    for result in &results {
        if let LayerWrite::Stale {
            rev: new, table, ..
        } = result
        {
            assert_eq!(new, &current.rev);
            assert_eq!(table.as_deref(), Some(current.vars.as_str()));
        }
    }
}
