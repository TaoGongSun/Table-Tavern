//! 人名裡的 `{{user}}` 落檔前換成玩家名（refactor-player-card-user-macro）：玩家卡名本身是巨集時
//! 注入會拿它代換自己。產物檔留巨集，只換落檔的卡名與恆定人物條目標題。
use super::super::test_support::*;
use super::super::*;
use crate::data::{self, CharacterCard, Tier, WorldbookEntry};
use crate::refactor::apply::apply_with_assets;
use crate::refactor::card_export::export_saved;
use crate::refactor::card_import::open_card;
use crate::refactor::types::RefactorApplyResult;
use crate::transport::replace_st_macros;
use std::path::Path;

fn outcome_of(characters: Vec<RefactorCharacter>) -> RefactorOutcome {
    RefactorOutcome {
        mode: None,
        characters,
        interface: None,
        mechanisms: Vec::new(),
        entries: Vec::new(),
        deletable_shared_uids: Vec::new(),
        dropped: Vec::new(),
        unabsorbed: Vec::new(),
        audit: Vec::new(),
        preserve_source_uids: Vec::new(),
        source_fingerprints: Default::default(),
        mirrors: Default::default(),
        stage_cards: Vec::new(),
    }
}

fn person(root: &Path, world_id: &str, name: &str) -> RefactorCharacter {
    let uid = seed_entry(
        root,
        world_id,
        &format!("{name}設定"),
        &format!("{name}的來源"),
    );
    character(name, &[uid])
}

fn run(
    root: &Path,
    world_id: &str,
    lang: &str,
    outcome: &RefactorOutcome,
    selection: &RefactorSelection,
) -> RefactorApplyResult {
    apply_with_assets(root, world_id, lang, outcome, selection, &[])
        .map_err(|failure| failure.error)
        .unwrap()
}

fn card_name(root: &Path, world_id: &str, id: &str) -> String {
    data::read_character(root, world_id, id).unwrap().name
}

fn person_entries(root: &Path, world_id: &str) -> Vec<WorldbookEntry> {
    data::read_worldbook(root, world_id)
        .unwrap()
        .into_iter()
        .filter(|entry| entry.is_person)
        .collect()
}

/// 卡沒給玩家名（`{{USER}}` 混大小寫）→ 玩家卡名用語系退路稱呼；正文與產物檔都留巨集，
/// 落檔後後端注入把正文的 `{{user}}` 換成玩家卡名。
#[test]
fn player_named_user_macro_gets_fallback_and_body_and_outcome_keep_macro() {
    let root = TestRoot::new("user-macro-player");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let mut player = person(root.path(), &world_id, "{{USER}}");
    player.public_md = "Name: {{user}}，测绘员。".to_owned();
    let outcome = outcome_of(vec![player]);
    let selection = RefactorSelection {
        player_index: Some(0),
        ..no_player_selection(vec![0])
    };

    let result = run(root.path(), &world_id, "zh-TW", &outcome, &selection);

    let card = data::read_player_card(root.path(), &world_id)
        .unwrap()
        .unwrap();
    assert_eq!(card.id, result.character_ids[0]);
    assert_eq!(card.name, "玩家");
    assert_eq!(card.public_md, "Name: {{user}}，测绘员。");
    assert_eq!(
        replace_st_macros(&card.public_md, &card.name, None),
        "Name: 玩家，测绘员。"
    );
    let saved = data::read_refactor_outcome(root.path(), &world_id)
        .unwrap()
        .unwrap();
    let saved: serde_json::Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(saved["outcome"]["characters"][0]["name"], "{{USER}}");
}

/// 退路稱呼照介面語系。
#[test]
fn player_named_user_macro_follows_ui_language() {
    let root = TestRoot::new("user-macro-en");
    let world_id = data::create_world(root.path(), "Tavern").unwrap();
    let outcome = outcome_of(vec![person(root.path(), &world_id, "{{user}}")]);
    let selection = RefactorSelection {
        player_index: Some(0),
        ..no_player_selection(vec![0])
    };
    let result = run(root.path(), &world_id, "en", &outcome, &selection);
    assert_eq!(
        card_name(root.path(), &world_id, &result.character_ids[0]),
        "Player"
    );
}

/// 同批指定具名玩家、NPC 排在玩家前面：NPC 名裡的 `{{user}}` 仍換成玩家名（與順序無關）；
/// 普通名字不變、`{{char}}` 原樣；換完就固定，之後玩家改名不回溯。
#[test]
fn npc_names_use_named_player_regardless_of_order_and_stay_fixed() {
    let root = TestRoot::new("user-macro-npc-order");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let outcome = outcome_of(vec![
        person(root.path(), &world_id, "{{user}}的妹妹"),
        person(root.path(), &world_id, "托伦"),
        person(root.path(), &world_id, "{{char}}的劍"),
        person(root.path(), &world_id, "阿明"),
    ]);
    let selection = RefactorSelection {
        player_index: Some(3),
        ..no_player_selection(vec![0, 1, 2, 3])
    };

    let result = run(root.path(), &world_id, "zh-TW", &outcome, &selection);

    let names: Vec<String> = result
        .character_ids
        .iter()
        .map(|id| card_name(root.path(), &world_id, id))
        .collect();
    assert_eq!(names, ["阿明的妹妹", "托伦", "{{char}}的劍", "阿明"]);

    let mut player = data::read_player_card(root.path(), &world_id)
        .unwrap()
        .unwrap();
    player.name = "小明".to_owned();
    data::write_character(root.path(), &world_id, &player).unwrap();
    assert_eq!(
        card_name(root.path(), &world_id, &result.character_ids[0]),
        "阿明的妹妹"
    );
}

/// 桌上已有玩家卡、這次只新增 NPC：用既有玩家卡的名字。
#[test]
fn npc_names_use_existing_player_when_none_assigned() {
    let root = TestRoot::new("user-macro-existing-player");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let existing = CharacterCard {
        id: data::new_id(),
        name: "阿濤".to_owned(),
        color: "#fff".to_owned(),
        avatar: "🧑".to_owned(),
        tier: Tier::Balanced,
        show_image: true,
        archived: false,
        gen_prompt: String::new(),
        public_md: String::new(),
        private_md: String::new(),
        stage: false,
    };
    data::write_character(root.path(), &world_id, &existing).unwrap();
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    state.player_card_id = Some(existing.id.clone());
    data::write_state(root.path(), &world_id, &state).unwrap();

    let outcome = outcome_of(vec![person(root.path(), &world_id, "{{user}}的妹妹")]);
    let result = run(
        root.path(),
        &world_id,
        "zh-TW",
        &outcome,
        &no_player_selection(vec![0]),
    );
    assert_eq!(
        card_name(root.path(), &world_id, &result.character_ids[0]),
        "阿濤的妹妹"
    );
}

/// 沒勾成卡的人物：恆定人物條目標題同樣換掉（名冊不出現字面巨集）；沒有玩家時用退路稱呼。
#[test]
fn unselected_person_entry_title_is_resolved() {
    let root = TestRoot::new("user-macro-person-entry");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let outcome = outcome_of(vec![
        person(root.path(), &world_id, "{{user}}的妹妹"),
        person(root.path(), &world_id, "阿明"),
    ]);
    let selection = RefactorSelection {
        player_index: Some(1),
        ..no_player_selection(vec![1])
    };
    run(root.path(), &world_id, "zh-TW", &outcome, &selection);
    let titles: Vec<String> = person_entries(root.path(), &world_id)
        .into_iter()
        .map(|entry| entry.title)
        .collect();
    assert_eq!(titles, ["阿明的妹妹"]);

    let root = TestRoot::new("user-macro-person-entry-fallback");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let outcome = outcome_of(vec![person(root.path(), &world_id, "{{User}}")]);
    run(
        root.path(),
        &world_id,
        "zh-TW",
        &outcome,
        &no_player_selection(Vec::new()),
    );
    let titles: Vec<String> = person_entries(root.path(), &world_id)
        .into_iter()
        .map(|entry| entry.title)
        .collect();
    assert_eq!(titles, ["玩家"]);
}

/// 玩家名只有部分是巨集、NPC 排在前面：玩家名只展開一次，NPC 用同一個最終名。
#[test]
fn partial_macro_player_name_expands_once_and_npc_uses_same_name() {
    let root = TestRoot::new("user-macro-partial-player");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let outcome = outcome_of(vec![
        person(root.path(), &world_id, "{{user}}的妹妹"),
        person(root.path(), &world_id, "{{user}}的化身"),
    ]);
    let selection = RefactorSelection {
        player_index: Some(1),
        ..no_player_selection(vec![0, 1])
    };
    let result = run(root.path(), &world_id, "zh-TW", &outcome, &selection);
    let names: Vec<String> = result
        .character_ids
        .iter()
        .map(|id| card_name(root.path(), &world_id, id))
        .collect();
    assert_eq!(names, ["玩家的化身的妹妹", "玩家的化身"]);
}

/// 匯出的重構卡留巨集：在 en 桌走匯入路徑（open_card → 照 applied 重現選擇）套用，玩家卡名是 Player。
#[test]
fn exported_card_keeps_macro_and_resolves_in_other_language_table() {
    let source = TestRoot::new("user-macro-export-source");
    let source_world = data::create_world(source.path(), "酒館").unwrap();
    let outcome = outcome_of(vec![
        person(source.path(), &source_world, "托伦"),
        person(source.path(), &source_world, "{{user}}"),
    ]);
    let selection = RefactorSelection {
        player_index: Some(1),
        ..no_player_selection(vec![0, 1])
    };
    run(source.path(), &source_world, "zh-TW", &outcome, &selection);
    let path = source.path().join("card.json");
    export_saved(source.path(), &source_world, &path, false).unwrap();

    let card = open_card(&std::fs::read(&path).unwrap()).unwrap().card;
    assert_eq!(card.outcome.characters[1].name, "{{user}}");
    let applied = card.applied.unwrap();
    let selection = RefactorSelection {
        player_index: applied.player_index,
        ..no_player_selection(
            applied
                .characters
                .iter()
                .filter(|item| item.character_id.is_some())
                .map(|item| item.outcome_index)
                .collect(),
        )
    };
    let target = TestRoot::new("user-macro-export-target");
    let target_world = data::create_world(target.path(), "Tavern").unwrap();
    run(
        target.path(),
        &target_world,
        "en",
        &card.outcome,
        &selection,
    );
    let player = data::read_player_card(target.path(), &target_world)
        .unwrap()
        .unwrap();
    assert_eq!(player.name, "Player");
}
