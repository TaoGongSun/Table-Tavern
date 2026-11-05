use super::{
    character_messages, expand_messages, materialize, outline_messages, parse_character,
    parse_expand, parse_outline, Expanded, ExpandedCharacter, VnSetup,
};
use crate::data;
use crate::data::vn::{Branching, Length, Mood, Preferences, SetupStage};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TestRoot(std::path::PathBuf);

impl TestRoot {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "table-tavern-genesis-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn outline_parses_two_characters() {
    let outline = parse_outline(
        "## WORLD: Harbor of Glass\nA storm-bound city.\n\n## CHARACTER: Ilya\nA smuggler who owes the player a favor.\n## CHARACTER: Moss\nA watch captain hunting Ilya.",
    )
    .unwrap();
    assert_eq!(outline.title, "Harbor of Glass");
    assert_eq!(outline.characters.len(), 2);
    assert_eq!(
        outline.characters[1].tagline,
        "A watch captain hunting Ilya."
    );
}

#[test]
fn outline_without_world_is_none() {
    assert!(parse_outline("## CHARACTER: Ilya\nA smuggler.").is_none());
}

#[test]
fn outline_accepts_mixed_case_fullwidth_markers_and_no_characters() {
    let outline = parse_outline(" ### wOrLd： 夜港\n迷霧籠罩碼頭。").unwrap();
    assert_eq!(outline.title, "夜港");
    assert_eq!(outline.characters.len(), 0);
}

#[test]
fn character_messages_omits_empty_genres_and_uses_default_hint() {
    let messages = character_messages("海港冒險", &[], "## WORLD: 夜港", "  ", "zh-Hant");
    assert_eq!(messages.len(), 1);
    assert!(messages[0]
        .content
        .contains("Player's idea: 海港冒險\nCurrent outline:"));
    assert!(!messages[0].content.contains("Genre hints:"));
    assert!(messages[0].content.contains(
        "Character request: (none — invent someone who fits the world and fills a gap in the cast)"
    ));
}

#[test]
fn character_messages_includes_genres_and_trimmed_hint() {
    let messages = character_messages(
        "海港冒險",
        &["mystery".to_owned(), "fantasy".to_owned()],
        "## WORLD: 夜港",
        "  尋找一位可靠的嚮導  ",
        "zh-Hant",
    );
    assert!(messages[0]
        .content
        .contains("Genre hints: mystery, fantasy\nCurrent outline:"));
    assert!(messages[0]
        .content
        .contains("Character request: 尋找一位可靠的嚮導\n\nAll content"));
}

#[test]
fn character_parses_heading_prefix_and_fullwidth_colon() {
    let character = parse_character("### cHaRaCtEr： 伊利亞\n\n欠玩家人情的走私客。").unwrap();
    assert_eq!(character.name, "伊利亞");
    assert_eq!(character.tagline, "欠玩家人情的走私客。");
}

#[test]
fn character_uses_first_character_section() {
    let character =
        parse_character("## CHARACTER: Ilya\nA smuggler.\n## CHARACTER: Moss\nA watch captain.")
            .unwrap();
    assert_eq!(character.name, "Ilya");
    assert_eq!(character.tagline, "A smuggler.");
}

#[test]
fn character_is_none_without_character_section() {
    assert!(parse_character("夜港籠罩在迷霧中。玩家正尋找失蹤的船長。").is_none());
}

#[test]
fn character_skips_empty_name_section() {
    let character =
        parse_character("## CHARACTER:   \n無名者。\n## CHARACTER: Moss\nA watch captain.")
            .unwrap();
    assert_eq!(character.name, "Moss");
    assert_eq!(character.tagline, "A watch captain.");
}

#[test]
fn expand_defaults_missing_parts_and_accepts_fullwidth_markers() {
    let expanded =
        parse_expand("# WoRlD：夜港\n迷霧籠罩碼頭。\n## cHaRaCtEr：伊利亞\nPUBLIC：\n走私者。")
            .unwrap();
    assert_eq!(expanded.characters[0].emoji, "🎭");
    assert_eq!(expanded.characters[0].private_md, "");
    assert_eq!(expanded.opening, "");
}

#[test]
fn bad_expand_is_none() {
    assert!(parse_expand("## OPENING\n你好").is_none());
}

#[test]
fn materialize_writes_world_characters_opening_and_unique_name() {
    let root = TestRoot::new();
    data::create_world(&root.0, "夜港").unwrap();
    let expanded = Expanded {
        world: "迷霧籠罩碼頭。".to_owned(),
        characters: vec![
            ExpandedCharacter {
                name: "伊利亞".to_owned(),
                emoji: "🦊".to_owned(),
                public_md: "走私者。".to_owned(),
                private_md: "欠了債。".to_owned(),
            },
            ExpandedCharacter {
                name: "莫斯".to_owned(),
                emoji: "🛡️".to_owned(),
                public_md: "守衛隊長。".to_owned(),
                private_md: String::new(),
            },
        ],
        opening: "雨落在碼頭上。你要怎麼做？".to_owned(),
    };
    let world_id = materialize(&root.0, "夜港", &expanded, data::TableKind::Vn, None).unwrap();
    let state = data::read_state(&root.0, &world_id).unwrap();
    assert_eq!(state.name, "夜港 2");
    assert_eq!(state.table_kind, data::TableKind::Vn);
    assert_eq!(
        data::read_world_md(&root.0, &world_id).unwrap(),
        expanded.world
    );
    assert_eq!(data::list_characters(&root.0, &world_id).unwrap().len(), 2);
    let events = data::read_transcript(&root.0, &world_id, 0).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].speaker_name, "GM");
    assert_eq!(events[0].text, expanded.opening);
}

#[test]
fn materialize_names_table_after_draft_title_even_if_model_renamed_it() {
    let root = TestRoot::new();
    data::create_world(&root.0, "夜港").unwrap();
    let expanded = parse_expand(
        "## WORLD: 模型自己取的新名字\n迷霧籠罩碼頭。\n## CHARACTER: 伊利亞\nPUBLIC:\n走私者。",
    )
    .unwrap();

    let first = materialize(&root.0, "  夜港  ", &expanded, data::TableKind::Chat, None).unwrap();
    let second = materialize(&root.0, "夜港", &expanded, data::TableKind::Chat, None).unwrap();

    assert_eq!(data::read_state(&root.0, &first).unwrap().name, "夜港 2");
    assert_eq!(data::read_state(&root.0, &second).unwrap().name, "夜港 3");
    let names: Vec<String> = data::list_worlds(&root.0)
        .unwrap()
        .into_iter()
        .map(|world| world.name)
        .collect();
    assert!(!names.iter().any(|name| name.contains("模型自己取的新名字")));
}

#[test]
fn materialize_refuses_blank_title_without_writing() {
    let root = TestRoot::new();
    let expanded = parse_expand("## WORLD: X\n迷霧籠罩碼頭。").unwrap();
    assert!(materialize(&root.0, " \t ", &expanded, data::TableKind::Chat, None).is_err());
    assert!(data::list_worlds(&root.0).unwrap().is_empty());
}

#[test]
fn expand_accepts_empty_world_title_but_not_blank_world() {
    let expanded = parse_expand("## WORLD:\n迷霧籠罩碼頭。").unwrap();
    assert_eq!(expanded.world, "迷霧籠罩碼頭。");
    assert!(parse_expand("## WORLD: 夜港\n   \n## CHARACTER: 伊利亞\n走私者。").is_none());
}

#[test]
fn outline_still_requires_a_title() {
    assert!(parse_outline("## WORLD:\n迷霧籠罩碼頭。").is_none());
}

#[test]
fn expand_messages_prefill_the_draft_title() {
    let messages = expand_messages("idea", &[], "夜港", "## WORLD: 夜港\n霧。", "zh-TW", None);
    assert!(messages[0]
        .content
        .contains("\n## WORLD: 夜港\n<full world setting"));
}

fn prefs(name: &str) -> Preferences {
    Preferences {
        genres: vec!["戀愛".to_owned(), "懸疑推理".to_owned()],
        mood: Some(Mood::Warm),
        length: Some(Length::Short),
        branching: Some(Branching::None),
        name: name.to_owned(),
    }
}

/// VN 大綱：空白點子交給 AI 自選；偏好（長度區間、選擇、主角名）、主角段與「簡介不劇透」都進提示詞
#[test]
fn vn_outline_messages_carry_preferences_and_accept_blank_idea() {
    let genres = vec!["戀愛".to_owned()];
    let text = &outline_messages("", &genres, "zh-TW", Some(&prefs("阿哲")))[0].content;
    assert!(text.contains("Genre hints: 戀愛"), "{text}");
    assert!(text.contains("3-5 chapters"), "{text}");
    assert!(text.contains("Choices: none. A linear story"), "{text}");
    assert!(
        text.contains("The player character is named 阿哲."),
        "{text}"
    );
    assert!(
        text.contains("## PLAYER:") && text.contains("back-cover copy"),
        "{text}"
    );
    let blank = &outline_messages("", &[], "zh-TW", Some(&Preferences::default()))[0].content;
    assert!(blank.contains("choose an appealing premise"), "{blank}");
    assert!(blank.contains("addresses them as \"you\""), "{blank}");
    let chat = &outline_messages("霧港", &[], "zh-TW", None)[0].content;
    assert!(
        !chat.contains("## PLAYER:") && !chat.contains("visual novel"),
        "{chat}"
    );
}

#[test]
fn vn_expand_messages_carry_preferences() {
    let text = &expand_messages(
        "",
        &[],
        "夜港",
        "## WORLD: 夜港\n霧。",
        "zh-TW",
        Some(&prefs("阿哲")),
    )[0]
    .content;
    assert!(
        text.contains("3-5 chapters") && text.contains("named 阿哲"),
        "{text}"
    );
    assert!(
        text.contains("PLAYER section is the player character"),
        "{text}"
    );
}

/// 主角段只認標題列；內文一句 Player: 不被吃掉
#[test]
fn outline_parses_player_heading_only() {
    let outline =
        parse_outline("## WORLD: 夜港\nPlayer: 不是段落\n## PLAYER: 阿哲\n新來的燈塔看守\n## CHARACTER: 莫斯\n隊長").unwrap();
    assert!(outline.world.contains("Player: 不是段落"));
    let player = outline.player.unwrap();
    assert_eq!(
        (player.name.as_str(), player.tagline.as_str()),
        ("阿哲", "新來的燈塔看守")
    );
    assert_eq!(outline.characters.len(), 1);
}

/// 精靈建 VN 桌：偏好、劇透鎖與設定階段寫進 vn.json；有名字建只有名字的玩家卡，跟它同名的 NPC 不另建
#[test]
fn materialize_vn_writes_preferences_setup_and_player_card() {
    let root = TestRoot::new();
    let expanded = parse_expand(
        "## WORLD: 夜港\n霧。\n## CHARACTER: 莫斯\nPUBLIC:\n隊長。\n## CHARACTER: 阿哲\nPUBLIC:\n主角。",
    )
    .unwrap();
    let setup = |name: &str| VnSetup {
        preferences: prefs(name),
        spoiler_lock: false,
    };
    let named = materialize(
        &root.0,
        "夜港",
        &expanded,
        data::TableKind::Vn,
        Some(setup("阿哲")),
    )
    .unwrap();
    let player = data::read_player_card(&root.0, &named).unwrap().unwrap();
    assert_eq!(player.name, "阿哲");
    assert!(player.public_md.is_empty() && player.private_md.is_empty());
    let cast = data::list_characters(&root.0, &named).unwrap();
    assert_eq!(
        cast.iter()
            .map(|card| card.name.as_str())
            .collect::<Vec<_>>(),
        ["莫斯"]
    );
    let file = data::vn::read_vn(&root.0, &named).unwrap();
    assert_eq!(file.preferences, Some(prefs("阿哲")));
    assert_eq!(file.setup, Some(SetupStage::Story));
    assert!(!file.spoiler_lock);
    let listed = data::list_worlds(&root.0).unwrap();
    assert_eq!(listed[0].setup, Some(SetupStage::Story));

    let unnamed = materialize(
        &root.0,
        "夜港",
        &expanded,
        data::TableKind::Vn,
        Some(setup(" ")),
    )
    .unwrap();
    assert!(data::read_player_card(&root.0, &unnamed).unwrap().is_none());
    assert_eq!(data::list_characters(&root.0, &unnamed).unwrap().len(), 2);
}
