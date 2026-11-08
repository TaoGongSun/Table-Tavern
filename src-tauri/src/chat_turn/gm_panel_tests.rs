//! table-status-panel：房間廣播的預設狀態欄快照只在預設桌有，白名單取欄位、索引分支轉字串陣列。
use super::scene_panel;
use crate::data::default_panel::{add_field, sync, PanelCard};
use crate::data::{self, WorldState};
use serde_json::json;

fn set(state: &mut WorldState, path: &str, value: &str) {
    let segments: Vec<String> = path.split('.').map(str::to_owned).collect();
    data::set_tree_value(&mut state.state.tree, &segments, value);
}

#[test]
fn panel_snapshot_only_on_default_tables_with_lists_as_arrays() {
    let mut state: WorldState = serde_json::from_str(r#"{"id":"w","name":"桌"}"#).unwrap();
    set(&mut state, "World.HP", "10");
    assert_eq!(scene_panel(&state), None, "非預設桌沒有 panel");
    state.state.tree.clear();
    let card = PanelCard {
        id: "a".into(),
        name: "格羅姆".into(),
        fresh: true,
    };
    sync(&mut state, &[card]);
    add_field(&mut state, &["characters".into(), "格羅姆".into()], "心情");
    set(&mut state, "characters.格羅姆.status", "盯著門口");
    for (index, item) in [
        "斧", "繩", "火把", "麵包", "水袋", "地圖", "鑰匙", "哨子", "藥", "盾", "弓",
    ]
    .iter()
    .enumerate()
    {
        set(
            &mut state,
            &format!("characters.格羅姆.items.{index}"),
            item,
        );
    }
    set(&mut state, "characters.格羅姆.心情", "煩躁");
    set(&mut state, "tracking.clues.0", "蹄印");
    let panel = scene_panel(&state).unwrap();
    assert_eq!(
        panel["characters"]["格羅姆"]["items"],
        json!([
            "斧", "繩", "火把", "麵包", "水袋", "地圖", "鑰匙", "哨子", "藥", "盾", "弓"
        ]),
        "照數字索引排（10 不排在 1 後面）"
    );
    assert_eq!(panel["characters"]["格羅姆"]["status"], "盯著門口");
    assert_eq!(panel["characters"]["格羅姆"]["心情"], "煩躁");
    assert_eq!(
        panel["tracking"],
        json!({ "goals": [], "promises": [], "clues": ["蹄印"] })
    );
    assert!(panel.get("World").is_none());
}

#[test]
fn panel_snapshot_carries_combat_but_not_refused_marks() {
    let mut state: WorldState = serde_json::from_str(r#"{"id":"w","name":"桌"}"#).unwrap();
    state.dice_enabled = true;
    let card = PanelCard {
        id: "a".into(),
        name: "沃恩".into(),
        fresh: true,
    };
    sync(&mut state, &[card]);
    let panel = scene_panel(&state).unwrap();
    assert!(panel.get("combat").is_none(), "沒開打不附");
    assert_eq!(panel["characters"]["沃恩"]["hp"], json!(""));
    assert_eq!(panel["characters"]["沃恩"]["conditions"], json!([]));
    set(&mut state, "characters.沃恩.hp", "18/24");
    set(&mut state, "characters.沃恩.conditions.0", "中毒");
    set(&mut state, "combat.order.0", "沃恩");
    set(&mut state, "combat.order.1", "哥布林A");
    set(&mut state, "combat.turn", "哥布林A");
    set(&mut state, "combat.foes.哥布林A.hp", "3/7");
    state
        .state
        .refused
        .insert("characters.沃恩.hp".into(), Default::default());
    let panel = scene_panel(&state).unwrap();
    assert_eq!(panel["characters"]["沃恩"]["conditions"], json!(["中毒"]));
    assert_eq!(
        panel["combat"],
        json!({
            "order": ["沃恩", "哥布林A"],
            "turn": "哥布林A",
            "foes": { "哥布林A": { "hp": "3/7", "conditions": [] } }
        })
    );
    assert!(!panel.to_string().contains("refused"));
}
