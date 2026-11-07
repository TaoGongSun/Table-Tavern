use super::*;
use crate::data::test_support::{character_card, TestRoot};
use crate::data::{create_world, write_character, write_state, write_world_md};

fn card(id: &str, name: &str) -> PanelCard {
    PanelCard {
        id: id.to_owned(),
        name: name.to_owned(),
        fresh: true,
    }
}

fn blank() -> WorldState {
    serde_json::from_str(r#"{"id":"w","name":"桌"}"#).unwrap()
}

fn path(text: &str) -> Vec<String> {
    text.split('.').map(str::to_owned).collect()
}

fn leaf<'a>(state: &'a WorldState, at: &str) -> Option<&'a StateNode> {
    crate::data::node_at(&state.state.tree, &path(at))
}

fn set(state: &mut WorldState, at: &str, value: &str) {
    crate::data::set_tree_value(&mut state.state.tree, &path(at), value);
}

#[test]
fn seed_plants_rules_flag_cells_and_is_idempotent() {
    let mut state = blank();
    assert!(sync(&mut state, &[card("a", "艾倫")]));
    assert!(state.mechanism.default_panel && state.mechanism.incremental);
    assert_eq!(state.mechanism.rules, seed_rules());
    assert_eq!(
        leaf(&state, "characters.艾倫.status"),
        Some(&StateNode::Leaf(String::new()))
    );
    for list in [
        "characters.艾倫.items",
        "tracking.goals",
        "tracking.promises",
        "tracking.clues",
    ] {
        assert_eq!(
            leaf(&state, list),
            Some(&StateNode::Branch(BTreeMap::new())),
            "{list}"
        );
    }
    assert_eq!(state.mechanism.panel_cells["a"], "艾倫");
    assert_eq!(state.branch_bindings["a"], path("characters.艾倫"));
    let before = state.clone();
    assert!(
        !sync(&mut state, &[card("a", "艾倫")]),
        "沒變動回 false（不寫檔）"
    );
    assert_eq!(state, before);
}

#[test]
fn rename_moves_the_cell_with_its_data_and_custom_fields() {
    let mut state = blank();
    sync(&mut state, &[card("a", "艾倫")]);
    set(&mut state, "characters.艾倫.status", "左臂受傷");
    set(&mut state, "characters.艾倫.items.0", "短劍");
    assert!(add_field(&mut state, &path("characters.艾倫"), "心情"));
    set(&mut state, "characters.艾倫.心情", "煩躁");
    assert!(sync(&mut state, &[card("a", "艾倫·改")]));
    assert!(leaf(&state, "characters.艾倫").is_none());
    assert_eq!(
        leaf(&state, "characters.艾倫·改.status"),
        Some(&StateNode::Leaf("左臂受傷".into()))
    );
    assert_eq!(
        leaf(&state, "characters.艾倫·改.items.0"),
        Some(&StateNode::Leaf("短劍".into()))
    );
    assert_eq!(
        leaf(&state, "characters.艾倫·改.心情"),
        Some(&StateNode::Leaf("煩躁".into()))
    );
    assert_eq!(state.mechanism.panel_cells["a"], "艾倫·改");
    assert!(
        state
            .mechanism
            .rules
            .contains_key("characters.艾倫·改.心情"),
        "自訂欄規則跟著搬"
    );
    assert!(!state.mechanism.rules.contains_key("characters.艾倫.心情"));
    assert!(remove_field(
        &mut state,
        &path("characters.艾倫·改"),
        "心情"
    ));
    assert!(add_field(&mut state, &path("characters.艾倫·改"), "心情"));
    sync(&mut state, &[]);
    assert!(
        !state
            .mechanism
            .rules
            .keys()
            .any(|key| key.starts_with("characters.艾倫")),
        "刪格一併清掉自訂欄規則"
    );
}

#[test]
fn adds_and_removes_cells_and_refills_missing_leaves() {
    let mut state = blank();
    sync(&mut state, &[card("a", "艾倫")]);
    assert!(sync(&mut state, &[card("a", "艾倫"), card("b", "布魯")]));
    assert!(leaf(&state, "characters.布魯.status").is_some());
    // 玩家把 status 清空（set_state_path 空值＝刪葉子）：下一輪補回來
    set(&mut state, "characters.艾倫.status", "");
    state
        .state
        .tree
        .insert("tracking".into(), StateNode::Leaf("x".into()));
    assert!(sync(&mut state, &[card("a", "艾倫"), card("b", "布魯")]));
    assert!(leaf(&state, "characters.艾倫.status").is_some());
    assert!(leaf(&state, "tracking.clues").is_some());
    // 卡刪掉才刪格；封存（fresh=false）留著舊格
    let mut archived = card("b", "布魯");
    archived.fresh = false;
    assert!(!sync(&mut state, &[card("a", "艾倫"), archived]));
    assert!(sync(&mut state, &[card("a", "艾倫")]));
    assert!(leaf(&state, "characters.布魯").is_none());
    assert!(!state.mechanism.panel_cells.contains_key("b"));
    assert!(!state.branch_bindings.contains_key("b"));
}

#[test]
fn same_name_gets_suffix_and_cells_cap_at_twelve() {
    let mut state = blank();
    let mut cards = vec![card("a", "衛兵"), card("b", "衛兵")];
    let mut stage = card("s", "城堡");
    stage.fresh = false;
    cards.push(stage);
    sync(&mut state, &cards);
    assert_eq!(state.mechanism.panel_cells["a"], "衛兵");
    assert_eq!(state.mechanism.panel_cells["b"], "衛兵（2）");
    assert!(
        !state.mechanism.panel_cells.contains_key("s"),
        "舞台卡不開格"
    );
    assert!(!sync(&mut state, &cards), "後綴格名字沒變不搬");
    let many: Vec<PanelCard> = (0..15)
        .map(|n| card(&format!("c{n}"), &format!("人{n}")))
        .collect();
    let mut state = blank();
    sync(&mut state, &many);
    assert_eq!(state.mechanism.panel_cells.len(), MAX_CELLS);
    assert!(!state.mechanism.panel_cells.contains_key("c12"));
}

/// 收回到改名前的快照：樹上是舊鍵，對照是新鍵——唯一一支沒歸屬的格接回來，資料不丟
#[test]
fn undo_past_a_rename_adopts_the_single_orphan_cell() {
    let mut state = blank();
    sync(&mut state, &[card("a", "艾倫")]);
    set(&mut state, "characters.艾倫.status", "警戒");
    let snapshot = state.state.tree.clone();
    sync(&mut state, &[card("a", "亞倫")]);
    state.state.tree = snapshot;
    sync(&mut state, &[card("a", "亞倫")]);
    assert_eq!(
        leaf(&state, "characters.亞倫.status"),
        Some(&StateNode::Leaf("警戒".into()))
    );
    assert!(leaf(&state, "characters.艾倫").is_none());
}

#[test]
fn add_and_remove_field_validate_names() {
    let mut state = blank();
    let cell = path("characters.艾倫");
    assert!(!add_field(&mut state, &cell, "心情"), "非預設桌不收");
    sync(&mut state, &[card("a", "艾倫")]);
    for bad in [
        "",
        "  ",
        "status",
        "items",
        "clues",
        "_私",
        "a.b",
        "*",
        &"長".repeat(25),
    ] {
        assert!(!add_field(&mut state, &cell, bad), "{bad}");
    }
    assert!(!add_field(&mut state, &path("characters.不存在"), "心情"));
    assert!(!add_field(&mut state, &path("World"), "心情"));
    assert!(add_field(&mut state, &cell, "心情"));
    assert!(!add_field(&mut state, &cell, "心情"), "重名");
    let rule = &state.mechanism.rules["characters.艾倫.心情"];
    assert_eq!(
        (rule.kind, rule.update, rule.inject),
        (FieldKind::Text, UpdateMode::Replace, InjectLevel::Turn)
    );
    assert_eq!(rule.max_chars, Some(CUSTOM_CHARS));
    assert!(add_field(&mut state, &path("tracking"), "伏筆"));
    assert!(!remove_field(&mut state, &cell, "status"), "系統欄不給刪");
    assert!(remove_field(&mut state, &cell, "心情"));
    assert!(leaf(&state, "characters.艾倫.心情").is_none());
    assert!(!state.mechanism.rules.contains_key("characters.艾倫.心情"));
    assert!(!sync(&mut state, &[card("a", "艾倫")]), "ensure 不動自訂欄");
    assert!(leaf(&state, "tracking.伏筆").is_some());
}

#[test]
fn ensure_seeds_only_plain_chat_tables_and_skips_unchanged_writes() {
    let root = TestRoot::new("default-panel-ensure");
    let world = create_world(root.path(), "空桌").unwrap();
    write_character(
        root.path(),
        &world,
        &character_card(&crate::data::new_id(), "艾倫"),
    )
    .unwrap();
    assert!(ensure(root.path(), &world).unwrap());
    let state = read_state(root.path(), &world).unwrap();
    assert!(state.mechanism.default_panel);
    assert!(crate::data::node_at(&state.state.tree, &path("characters.艾倫.status")).is_some());
    let file = root.path().join(format!("worlds/{world}/state.json"));
    let bytes = std::fs::read(&file).unwrap();
    assert!(!ensure(root.path(), &world).unwrap());
    assert_eq!(std::fs::read(&file).unwrap(), bytes, "沒變動不寫檔");

    // 宣告了狀態欄格式的桌、有機制的桌、VN 桌、重播桌都不種
    let declared = create_world(root.path(), "有格式").unwrap();
    write_world_md(root.path(), &declared, "每回合附上狀態欄。").unwrap();
    assert!(!ensure(root.path(), &declared).unwrap());
    for edit in [
        (|state: &mut WorldState| state.mechanism.incremental = true) as fn(&mut WorldState),
        |state| state.table_kind = TableKind::Vn,
        |state| state.replay = true,
        |state| {
            crate::data::set_tree_value(&mut state.state.tree, &path("World.HP"), "10");
        },
    ] {
        let other = create_world(root.path(), "別桌").unwrap();
        let mut state = read_state(root.path(), &other).unwrap();
        edit(&mut state);
        write_state(root.path(), &other, &state).unwrap();
        assert!(!ensure(root.path(), &other).unwrap());
        assert!(
            !read_state(root.path(), &other)
                .unwrap()
                .mechanism
                .default_panel
        );
    }
}

/// 面板把有規則的欄位清空：留空值不刪葉子（不變幽靈欄）；沒規則的路徑照舊刪
#[test]
fn clearing_a_ruled_field_keeps_an_empty_value() {
    let mut state = blank();
    sync(&mut state, &[card("a", "艾倫")]);
    assert!(add_field(&mut state, &path("characters.艾倫"), "心情"));
    set(&mut state, "characters.艾倫.status", "受傷");
    set(&mut state, "characters.艾倫.心情", "煩躁");
    set(&mut state, "characters.艾倫.items.0", "劍");
    for at in ["characters.艾倫.status", "characters.艾倫.心情"] {
        assert_eq!(clear_field(&mut state, &path(at), ""), Some(true), "{at}");
        assert_eq!(
            leaf(&state, at),
            Some(&StateNode::Leaf(String::new())),
            "{at}"
        );
    }
    assert_eq!(
        clear_field(&mut state, &path("characters.艾倫.items"), ""),
        Some(true)
    );
    assert_eq!(
        leaf(&state, "characters.艾倫.items"),
        Some(&StateNode::Branch(BTreeMap::new()))
    );
    assert_eq!(
        clear_field(&mut state, &path("characters.艾倫.status"), ""),
        Some(false)
    );
    assert!(
        !add_field(&mut state, &path("characters.艾倫"), "心情"),
        "欄位還在，不是幽靈"
    );
    assert!(remove_field(&mut state, &path("characters.艾倫"), "心情"));
    assert_eq!(clear_field(&mut state, &path("World.x"), ""), None);
    assert_eq!(
        clear_field(&mut state, &path("characters.艾倫.status"), "x"),
        None
    );
}
