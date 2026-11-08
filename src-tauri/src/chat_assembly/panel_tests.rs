//! table-status-panel 包 1：預設狀態欄桌從 `gm_materials` 載入一路到實送 GM 提示——策略是 DeltaOnly、不是 MVU 來源，
//! system 帶指引與固定範例、不帶多行字串清單句與介面歸屬聲明；新增項目後下一輪非對齊回合動態塊看得到。
use super::*;

fn card(name: &str) -> data::CharacterCard {
    data::CharacterCard {
        id: data::new_id(),
        name: name.to_owned(),
        color: "#336699".to_owned(),
        avatar: "🐗".to_owned(),
        tier: data::Tier::Balanced,
        show_image: true,
        archived: false,
        gen_prompt: String::new(),
        public_md: "獸人戰士。".to_owned(),
        private_md: String::new(),
        stage: false,
    }
}

fn panel_table(label: &str) -> (std::path::PathBuf, String) {
    let root = crate::smart_free::test_root(label);
    let world_id = data::create_world(&root, "一句話開桌").unwrap();
    for name in ["格羅姆", "索爾"] {
        data::write_character(&root, &world_id, &card(name)).unwrap();
    }
    assert!(data::default_panel::ensure(&root, &world_id).unwrap());
    (root, world_id)
}

fn messages(root: &std::path::Path, world_id: &str, lang: &str) -> Vec<transport::ChatMessage> {
    let materials = gm_materials(root, world_id).unwrap();
    let scan = test_gm_scan(root, world_id, &materials, lang);
    let (instruction, _) = gm_instruction(root, world_id, &materials, &scan, lang);
    let (scope, _) = gm_scope(&materials);
    gm_messages(&materials, &scan, &scope, instruction, lang)
}

#[test]
fn panel_table_prompt_carries_guide_and_fixed_example() {
    let (root, world_id) = panel_table("panel-prompt");
    let state = data::read_state(&root, &world_id).unwrap();
    assert_eq!(
        mechanism::resolve_numeric_update(&root, &world_id, &state),
        data::NumericUpdate::DeltaOnly
    );
    assert!(!mechanism::mvu_source(&root, &world_id, &state).unwrap());
    let materials = gm_materials(&root, &world_id).unwrap();
    assert_eq!(
        materials.state.mechanism.numeric_update,
        data::NumericUpdate::DeltaOnly
    );
    let system = messages(&root, &world_id, "zh-TW")[0].content.clone();
    assert!(system.contains("## 狀態更新協定 v1"), "{system}");
    assert!(system.contains("## 本桌狀態欄（由系統保管）"));
    assert!(system
        .contains(r#"{"op":"replace","path":"/characters/格羅姆/status","value":"（新的值）"}"#));
    assert!(system.contains(
        r#"{"op":"replace","path":"/tracking/clues","value":["（線索一）","（線索二）"]}"#
    ));
    assert!(
        !system.contains("值是多行字串的清單欄"),
        "不附 list_protocol 的字串句"
    );
    assert!(!system.contains("## 介面由誰畫"));
    let english = messages(&root, &world_id, "en")[0].content.clone();
    assert!(english.contains("## This table's status panel (kept by the system)"));
    assert!(!english.contains("List fields (values that are multi-line strings)"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn new_list_item_is_visible_on_the_next_non_aligned_turn() {
    let (root, world_id) = panel_table("panel-next-turn");
    let mut state = data::read_state(&root, &world_id).unwrap();
    let reply = "旁白。\n<UpdateVariable>\n<JSONPatch>\n[{\"op\":\"replace\",\"path\":\"/tracking/clues\",\"value\":[\"泥地上的蹄印\"]}]\n</JSONPatch>\n</UpdateVariable>";
    mechanism::apply_block(&mut state, &transport::extract_state_block(reply), "阿濤");
    state.aligned_scene = Some(state.current_scene);
    data::write_state(&root, &world_id, &state).unwrap();
    let materials = gm_materials(&root, &world_id).unwrap();
    assert!(!gm_scope(&materials).1, "非對齊回合");
    let all: String = messages(&root, &world_id, "zh-TW")
        .iter()
        .map(|message| message.content.as_str())
        .collect();
    assert!(all.contains("線索（clues）（更新）："), "{all}");
    assert!(all.contains("- 泥地上的蹄印"), "{all}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn combat_table_prompt_carries_combat_guide_and_dice_example() {
    let (root, world_id) = panel_table("panel-combat-prompt");
    data::update_state(&root, &world_id, |state| {
        state.dice_enabled = true;
        Ok(Some(state.clone()))
    })
    .unwrap();
    assert!(data::default_panel::ensure(&root, &world_id).unwrap());
    let system = messages(&root, &world_id, "zh-TW")[0].content.clone();
    assert!(system.contains("## 戰鬥欄（數字由系統算）"), "{system}");
    assert!(system.contains("同種敵人要取不同名字"));
    assert!(system
        .contains(r#"{"op":"delta","path":"/characters/格羅姆/hp","value":-7,"dice":"d8#1+3"}"#));
    let english = messages(&root, &world_id, "en")[0].content.clone();
    assert!(english.contains("## Combat fields (arithmetic done by the system)"));
    let _ = std::fs::remove_dir_all(&root);
}
