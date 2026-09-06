use super::{
    character_messages, materialize, parse_character, parse_expand, parse_outline, Expanded,
    ExpandedCharacter,
};
use crate::data;
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
        title: "夜港".to_owned(),
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
    let world_id = materialize(&root.0, &expanded).unwrap();
    assert_eq!(data::read_state(&root.0, &world_id).unwrap().name, "夜港 2");
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
