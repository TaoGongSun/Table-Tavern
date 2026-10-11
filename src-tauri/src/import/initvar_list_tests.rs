//! initvar-yaml-list 整合：`[initvar]` 的 `- ` 清單匯入後是 JSON 葉子；樹模式 Upstream 桌的索引路徑照陣列
//! 語意套用，卡片變數模式物化與補初始化來源都是真陣列。
use crate::data::{self, StateNode};
use crate::import::import_character;
use crate::import::test_support::TestRoot;
use serde_json::{json, Value};

const INITVAR: &str = "角色:\n  名字: 艾琳\n  物品:\n    - 長劍\n    - 治療藥水\n  技能:\n    - 名稱: 火球\n      消耗: 5\n    - 名稱: 冰牆\n      消耗: 8\n任務:\n- 找到鑰匙\n";

fn card(name: &str, initvar: &str, mvu_script: bool) -> String {
    let extensions = if mvu_script {
        json!({"tavern_helper": {"scripts": [{
            "name": "MVU", "enabled": true, "type": "script",
            "content": "import 'https://x/MagVarUpdate/bundle.js';"
        }]}})
    } else {
        json!({})
    };
    json!({"data": {
        "name": name,
        "first_mes": "開場",
        "extensions": extensions,
        "character_book": {"entries": [
            {"comment": "[initvar]变量初始化勿开", "enabled": false, "content": initvar}
        ]}
    }})
    .to_string()
}

fn table(label: &str, raw: &str) -> (TestRoot, String) {
    let root = TestRoot::new(label);
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff", "zh-TW").unwrap();
    (root, world_id)
}

fn leaf(root: &TestRoot, world_id: &str, path: &[&str]) -> Value {
    let state = data::read_state(root.path(), world_id).unwrap();
    let path: Vec<String> = path.iter().map(|segment| (*segment).to_owned()).collect();
    match data::node_at(&state.state.tree, &path) {
        Some(StateNode::Leaf(text)) => serde_json::from_str(text).unwrap(),
        other => panic!("{path:?} 不是葉子：{other:?}"),
    }
}

fn opening(root: &TestRoot, world_id: &str, raw: &str) -> data::TranscriptEvent {
    data::append_opening(
        root.path(),
        world_id,
        0,
        "opening",
        raw,
        &crate::transport::extract_state_block(raw),
        "阿濤",
    )
    .unwrap()
    .0
}

fn stat_of(event: &data::TranscriptEvent) -> Value {
    let table = event.message_vars.as_ref().unwrap().parse().unwrap();
    serde_json::from_str(&table.get("stat_data").unwrap().to_text()).unwrap()
}

#[test]
fn tree_mode_lists_import_as_json_leaves_and_take_index_paths() {
    let (root, world_id) = table("initvar-list-tree", &card("艾琳", INITVAR, false));
    assert_eq!(
        leaf(&root, &world_id, &["角色", "物品"]),
        json!(["長劍", "治療藥水"])
    );
    assert_eq!(
        leaf(&root, &world_id, &["角色", "技能"]),
        json!([{"名稱": "火球", "消耗": 5}, {"名稱": "冰牆", "消耗": 8}])
    );
    assert_eq!(leaf(&root, &world_id, &["任務"]), json!(["找到鑰匙"]));
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(
        crate::mechanism::resolve_numeric_update(root.path(), &world_id, &state),
        data::NumericUpdate::Upstream
    );
    let raw = "開場旁白\n<UpdateVariable><JSONPatch>[\
        {\"op\":\"insert\",\"path\":\"/角色/物品/-\",\"value\":\"盾\"},\
        {\"op\":\"remove\",\"path\":\"/角色/物品/0\"},\
        {\"op\":\"replace\",\"path\":\"/角色/技能/1/消耗\",\"value\":9},\
        {\"op\":\"insert\",\"path\":\"/任務/0\",\"value\":\"出發\"}\
        ]</JSONPatch></UpdateVariable>";
    opening(&root, &world_id, raw);
    assert_eq!(
        leaf(&root, &world_id, &["角色", "物品"]),
        json!(["治療藥水", "盾"])
    );
    assert_eq!(
        leaf(&root, &world_id, &["角色", "技能"]),
        json!([{"名稱": "火球", "消耗": 5}, {"名稱": "冰牆", "消耗": 9}])
    );
    assert_eq!(
        leaf(&root, &world_id, &["任務"]),
        json!(["出發", "找到鑰匙"])
    );
}

#[test]
fn variable_mode_materializes_and_fills_init_source_with_real_arrays() {
    let (root, world_id) = table("initvar-list-vars", &card("艾琳", INITVAR, true));
    // 樹模式匯入 → 開場切變數模式物化
    let opened = opening(&root, &world_id, "開場白");
    let stat = stat_of(&opened);
    assert_eq!(stat["角色"]["物品"], json!(["長劍", "治療藥水"]));
    assert_eq!(
        stat["角色"]["技能"],
        json!([{"名稱": "火球", "消耗": 5}, {"名稱": "冰牆", "消耗": 8}])
    );
    assert_eq!(stat["任務"], json!(["找到鑰匙"]));
    // 已在變數模式：第二張卡的 initvar 走 edit_tree_if_events 補進初始化來源
    let second = card(
        "第二張",
        "隊友:\n  - 名字: 甲\n    等級: 2\n  - 乙\n空槽:\n  - \"\"\n  -\n",
        false,
    );
    let held = data::test_exclusive(&world_id);
    crate::import::import_character_file(
        root.path(),
        &world_id,
        second.as_bytes(),
        "#000000",
        "zh-TW",
        &held,
    )
    .unwrap();
    drop(held);
    let events = data::read_transcript(root.path(), &world_id, 0).unwrap();
    let source = events.iter().find(|event| event.id == opened.id).unwrap();
    let stat = stat_of(source);
    assert_eq!(stat["隊友"], json!([{"名字": "甲", "等級": 2}, "乙"]));
    assert_eq!(stat["空槽"], json!(["", null]));
    assert_eq!(stat["角色"]["物品"], json!(["長劍", "治療藥水"]));
}
