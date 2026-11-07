//! table-status-panel 包 1：預設狀態欄在 GM 動態塊的顯示名與清單印法、角色線拿到自己那格、隱藏卡的格被裁掉。
use super::{character_state_block, gm_dynamic_block, resolve_branch, state_scope, StateScope};
use crate::data::default_panel::{sync, PanelCard};
use crate::data::{self, CharacterCard, WorldState};

fn panel(names: &[(&str, &str)]) -> WorldState {
    let mut state: WorldState = serde_json::from_str(r#"{"id":"w","name":"桌"}"#).unwrap();
    let cards: Vec<PanelCard> = names
        .iter()
        .map(|(id, name)| PanelCard {
            id: (*id).into(),
            name: (*name).into(),
            fresh: true,
        })
        .collect();
    sync(&mut state, &cards);
    state
}

fn set(state: &mut WorldState, path: &str, value: &str) {
    let segments: Vec<String> = path.split('.').map(str::to_owned).collect();
    data::set_tree_value(&mut state.state.tree, &segments, value);
}

fn card(id: &str, name: &str) -> CharacterCard {
    CharacterCard {
        id: id.into(),
        name: name.into(),
        color: String::new(),
        avatar: String::new(),
        tier: data::Tier::Balanced,
        show_image: false,
        archived: false,
        gen_prompt: String::new(),
        public_md: String::new(),
        private_md: String::new(),
        stage: false,
    }
}

#[test]
fn gm_block_shows_localized_names_and_lists() {
    let mut state = panel(&[("a", "格羅姆")]);
    set(&mut state, "characters.格羅姆.status", "盯著門口");
    set(&mut state, "tracking.clues.0", "蹄印");
    let block = |lang| {
        gm_dynamic_block(
            "",
            &state.state,
            "阿濤",
            &state.mechanism,
            &StateScope::default(),
            lang,
        )
    };
    let zh = block("zh-TW");
    for line in [
        "角色（characters）：",
        "  格羅姆：",
        "    狀態（status）：盯著門口",
        "    持有（items）：[]",
        "追蹤中（tracking）：",
        "  線索（clues）：\n    - 蹄印",
        "  目標（goals）：[]",
        "  承諾（promises）：[]",
    ] {
        assert!(zh.contains(line), "{line}\n{zh}");
    }
    let en = block("en");
    for line in [
        "Characters (characters):",
        "    Status (status): 盯著門口",
        "    Items (items): []",
        "Tracking (tracking):",
        "  Clues (clues):",
        "  Goals (goals): []",
    ] {
        assert!(en.contains(line), "{line}\n{en}");
    }
}

#[test]
fn character_lane_gets_its_own_cell_even_with_a_suffix() {
    let mut state = panel(&[("a", "衛兵"), ("b", "衛兵")]);
    set(&mut state, "characters.衛兵（2）.status", "站崗");
    let branch = resolve_branch(&state.state.tree, &state.branch_bindings, "b", "衛兵").unwrap();
    assert_eq!(branch, ["characters", "衛兵（2）"]);
    let block = character_state_block(
        &state.state,
        &state.mechanism,
        &branch,
        "衛兵",
        "阿濤",
        "zh-TW",
    )
    .unwrap();
    assert!(block.contains("狀態（status）：站崗"), "{block}");
}

/// 隱藏卡（不在送進提示的卡清單裡）的格是 characters 容器的手足，不在場就裁掉
#[test]
fn hidden_card_cell_is_cut_when_not_present() {
    let mut state = panel(&[("a", "格羅姆"), ("b", "索爾")]);
    state.state.table.insert("present".into(), "格羅姆".into());
    let scope = state_scope(
        &state.state,
        &state.mechanism,
        &[card("a", "格羅姆")],
        None,
        &state.branch_bindings,
        false,
        &["格羅姆".into(), "索爾".into()],
    );
    assert_eq!(
        scope.hidden,
        [vec!["characters".to_owned(), "索爾".to_owned()]]
    );
    let block = gm_dynamic_block("", &state.state, "阿濤", &state.mechanism, &scope, "zh-TW");
    assert!(
        block.contains("格羅姆") && !block.contains("索爾"),
        "{block}"
    );
}
