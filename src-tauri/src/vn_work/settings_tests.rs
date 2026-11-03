//! 附設定與可續寫（plans/vn-mode-pkg8.md §3 8c）：往返還原快照、設定、世界書與機制規則，匯入後一拍旗標與秘密揭露
//! 被接受；快照檔長度不符拒收不留桌；變數模式桌不能附設定；附設定的包當作品玩時不讀設定。
use super::format::{ImportMode, LIMITS};
use super::materialize::import_with;
use super::tests::{entries, played, staged, Table, SECRET};
use super::*;
use crate::data::{self, StateNode, TableState};
use crate::ui_msg::UiMsg;
use std::io::Write;
use std::path::PathBuf;

/// 有聖經（旗標、角色秘密）、卡原件、世界書（限主角可見）、world.md 的已玩桌；目前幕的快照帶旗標與揭露
fn with_settings(label: &str) -> (Table, data::vn::Bible) {
    let table = played(label);
    let (root, world, hero) = (&table.root, &table.world, &table.hero);
    let bible = data::vn::parse_bible(&serde_json::json!({
        "chapters": [{ "title": "霧" }, { "title": "港" }],
        "flags": [{ "id": "met", "type": "bool", "initial": "false" }],
        "secrets": [{ "id": "s1", "holder": hero, "text": "她是王女", "reveal": { "flag": "met" } }],
        "endings": [{ "id": "default", "title": "天亮" }],
    }))
    .unwrap();
    let bible = data::vn::normalize(bible, &[(hero.clone(), "艾琳".to_owned())]);
    let card: data::vn::CardOrigin = serde_json::from_value(serde_json::json!({
        "lang": "zh-TW",
        "rating": { "level": "teen", "warnings": ["暴力"] },
        "locations": [{ "key": "harbor", "name": "港口", "times": ["day", "night"], "prompt": "霧港" }],
        "cast": { (hero.clone()): { "appears": "只在霧裡", "art": {
            "expressions": [{ "key": "smug", "label": "得意", "prompt": "smirk" }],
            "variants": [{ "key": "coat", "label": "大衣", "prompt": "coat" }],
            "height_cm": 160 } } },
    }))
    .unwrap();
    data::state_commit::with_commit(root, world, |tx| {
        data::vn::edit_vn(tx, |file| {
            file.bible = Some(bible.clone());
            file.card = Some(card.clone());
        })
    })
    .unwrap();
    data::write_world_md(root, world, "霧港的世界設定").unwrap();
    let book = serde_json::json!({ "entries": [{ "keys": ["王女"], "content": "王女的傳說",
        "extensions": { "table_tavern": { "visibility": { "characters": [hero] } } } }] });
    data::import_worldbook_as(root, world, &book.to_string(), &data::BookOwner::Gm).unwrap();
    let mut state = staged(2, None);
    let leaf = |text: &str| StateNode::Leaf(text.to_owned());
    state.tree.insert(
        "flags".to_owned(),
        StateNode::Branch([("met".to_owned(), leaf("false"))].into()),
    );
    state.tree.insert(
        "revealed".to_owned(),
        StateNode::Branch([("s1".to_owned(), leaf("false"))].into()),
    );
    data::set_last_transcript_state(root, world, 2, &state).unwrap();
    (table, bible)
}

fn export_settings(table: &Table) -> PathBuf {
    let path = table.root.join("full.ttvn");
    export_work(&table.root, &table.world, &path, None, "zh-TW", true).unwrap();
    path
}

fn states(root: &std::path::Path, world: &str, scene: u64) -> Vec<Option<TableState>> {
    data::read_transcript(root, world, scene)
        .unwrap()
        .into_iter()
        .map(|event| event.state)
        .collect()
}

#[test]
fn continue_round_trip_restores_settings_and_accepts_the_next_turn() {
    let (table, bible) = with_settings("vn-work-continue");
    let pack = export_settings(&table);
    let (root, hero) = (table.root.as_path(), &table.hero);
    let copy = import_work(root, &pack, ImportMode::Continue, true)
        .unwrap()
        .world_id;
    let original = data::read_state(root, &table.world).unwrap();
    let state = data::read_state(root, &copy).unwrap();
    assert!(!state.replay);
    assert_eq!(state.current_scene, original.current_scene);
    let last = states(root, &table.world, original.current_scene)
        .pop()
        .flatten();
    assert_eq!(Some(&state.state), last.as_ref());
    for scene in 0..=2 {
        assert_eq!(
            states(root, &table.world, scene),
            states(root, &copy, scene),
            "scene {scene}"
        );
    }
    let file = data::vn::read_vn(root, &copy).unwrap();
    let source = data::vn::read_vn(root, &table.world).unwrap();
    assert_eq!(file.bible.as_ref(), Some(&bible));
    assert_eq!(file.card, source.card);
    assert!(file.spoiler_lock);
    assert_eq!(
        data::read_character(root, &copy, hero).unwrap().private_md,
        SECRET
    );
    assert_eq!(data::read_world_md(root, &copy).unwrap(), "霧港的世界設定");
    let book = serde_json::to_string(&data::read_worldbook_raw(root, &copy).unwrap()).unwrap();
    assert!(book.contains("王女的傳說") && book.contains(hero.as_str()));
    // 機制規則照聖經推導，且開增量回報（Sol 方案審提醒：rules() 與 ensure_engine_rules 不會代設）
    let mechanism = &state.mechanism;
    assert!(mechanism.incremental);
    assert_eq!(mechanism.rules, bible.rules());
    for locked in data::vn::LOCKED_ROOTS {
        assert!(mechanism.locked_roots.iter().any(|root| root == locked));
    }
    // 下一拍 GM 回報：旗標成立、秘密揭露都被接受
    let mut world = state.clone();
    let split = |path: &str| path.split('.').map(str::to_owned).collect::<Vec<_>>();
    let block = crate::transport::StateBlock {
        fields: vec![
            (split("flags.met"), "true".to_owned()),
            (split("revealed.s1"), "true".to_owned()),
        ],
        ..Default::default()
    };
    let outcome = crate::mechanism::apply_block(&mut world, &block, "玩家");
    assert!(outcome.notes.is_empty(), "{:?}", outcome.notes);
    for path in ["flags.met", "revealed.s1"] {
        assert_eq!(
            data::node_at(&world.state.tree, &split(path)),
            Some(&StateNode::Leaf("true".to_owned())),
            "{path}"
        );
    }
}

#[test]
fn a_pack_with_settings_still_imports_as_a_bare_replay() {
    let (table, _) = with_settings("vn-work-settings-replay");
    let pack = export_settings(&table);
    let copy = import_work(&table.root, &pack, ImportMode::Replay, true)
        .unwrap()
        .world_id;
    let root = table.root.as_path();
    assert!(data::read_state(root, &copy).unwrap().replay);
    assert!(data::read_character(root, &copy, &table.hero)
        .unwrap()
        .private_md
        .is_empty());
    assert!(data::read_worldbook_raw(root, &copy).unwrap().is_empty());
    // 沒附設定的包不能當可續寫的桌
    let bare = table.root.join("bare.ttvn");
    export_work(root, &table.world, &bare, None, "zh-TW", false).unwrap();
    assert!(import_work(root, &bare, ImportMode::Continue, true).is_err());
}

#[test]
fn a_history_that_does_not_line_up_leaves_no_table() {
    let (table, _) = with_settings("vn-work-history-length");
    let pack = export_settings(&table);
    let bad = table.root.join("bad.ttvn");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&bad).unwrap());
    for (name, mut bytes) in entries(&pack) {
        if name == "settings/history/0.json" {
            let mut states: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
            states.pop();
            bytes = serde_json::to_vec(&states).unwrap();
        }
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&bytes).unwrap();
    }
    zip.finish().unwrap();
    let before = data::list_worlds(&table.root).unwrap().len();
    let mut hook = |_: &str| Ok(());
    let error = import_with(
        &table.root,
        &bad,
        ImportMode::Continue,
        true,
        LIMITS,
        &mut hook,
    )
    .unwrap_err();
    assert!(error.contains("history/0.json"), "{error}");
    assert_eq!(data::list_worlds(&table.root).unwrap().len(), before);
    // 故障注入：世界書與狀態之後失敗也不留半桌
    for step in ["worldbook", "state", "vn"] {
        let mut hook = |at: &str| match at == step {
            true => Err("故障注入".to_owned()),
            false => Ok(()),
        };
        assert!(import_with(
            &table.root,
            &pack,
            ImportMode::Continue,
            true,
            LIMITS,
            &mut hook
        )
        .is_err());
        assert_eq!(
            data::list_worlds(&table.root).unwrap().len(),
            before,
            "{step}"
        );
    }
}

#[test]
fn a_card_vars_table_cannot_export_settings() {
    let table = played("vn-work-vars-settings");
    let control = data::message_vars::control_path(&table.root, &table.world).unwrap();
    let scene = data::read_state(&table.root, &table.world)
        .unwrap()
        .current_scene;
    let json = format!(r#"{{"mode":"events","scenes":{{"{scene}":{{"epoch":"e","seed":{{}}}}}}}}"#);
    data::commit_world_write_atomic(&control, json.as_bytes()).unwrap();
    let path = table.root.join("vars.ttvn");
    let error = export_work(&table.root, &table.world, &path, None, "zh-TW", true).unwrap_err();
    assert_eq!(error, UiMsg::VnWorkVarsMode.to_string());
    assert!(!path.exists());
}
