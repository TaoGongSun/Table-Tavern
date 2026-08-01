use crate::data::{self, CharacterCard, DataResult, Tier, TranscriptEvent, TranscriptKind};
use crate::transport::ChatMessage;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outline {
    pub title: String,
    pub world: String,
    pub characters: Vec<OutlineCharacter>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineCharacter {
    pub name: String,
    pub tagline: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expanded {
    pub title: String,
    pub world: String,
    pub characters: Vec<ExpandedCharacter>,
    pub opening: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedCharacter {
    pub name: String,
    pub emoji: String,
    pub public_md: String,
    pub private_md: String,
}

#[derive(Debug)]
struct RawCharacter {
    name: String,
    lines: Vec<String>,
}

#[derive(Debug, Default)]
struct Sections {
    title: Option<String>,
    world: Vec<String>,
    characters: Vec<RawCharacter>,
    opening: Vec<String>,
}

enum CurrentSection {
    None,
    World,
    Character(usize),
    Opening,
}

pub fn outline_messages(input: &str, genres: &[String], lang: &str) -> Vec<ChatMessage> {
    let genres = genre_line(genres);
    vec![ChatMessage {
        role: "user".to_owned(),
        content: format!(
            "You are a tabletop RPG game master designing a new campaign from a player's brief idea.\n\nPlayer's idea: {input}\n{genres}\nWrite a campaign outline. All content must be written in the language with BCP-47 code \"{lang}\". Output EXACTLY this structure, using these exact English section markers at line start:\n\n## WORLD: <campaign title, one line>\n<2-4 short paragraphs: setting, tone, current situation, and what the player will be doing>\n\n## CHARACTER: <name>\n<one line: who they are and their relation to the player>\n\nRules:\n- Decide the number of characters yourself from the player's idea. A story focused on a single person needs only that person; an ensemble story may need more. Never pad the cast.\n- Characters are NPCs the player will interact with. Do not create a character representing the player.\n- No text outside the sections above. Markers must be exactly \"## WORLD:\" and \"## CHARACTER:\"."
        ),
    }]
}

pub fn expand_messages(
    input: &str,
    genres: &[String],
    outline_raw: &str,
    lang: &str,
) -> Vec<ChatMessage> {
    let genres = genre_line(genres);
    vec![ChatMessage {
        role: "user".to_owned(),
        content: format!(
            "You are a tabletop RPG game master. The player approved this campaign outline. Expand it into full campaign materials.\n\nPlayer's idea: {input}\n{genres}Approved outline:\n{outline_raw}\n\nAll content must be written in the language with BCP-47 code \"{lang}\". Output EXACTLY this structure, using these exact English markers at line start:\n\n## WORLD: <campaign title, one line>\n<full world setting in markdown: the setting, key places or factions, tone, current situation. Enough substance for a GM to run scenes from.>\n\n## CHARACTER: <name>\nEMOJI: <one single emoji fitting this character>\nPUBLIC:\n<what everyone can see: appearance, role, public personality>\nPRIVATE:\n<secrets, hidden motives, inner voice — visible only to this character's actor>\n\n## OPENING\n<the opening narration the GM speaks to start the first scene, addressed to the player, ending at a moment that invites the player to act>\n\nRules:\n- Keep the same characters and names as the outline.\n- Markers exactly as shown. No text outside the sections."
        ),
    }]
}

pub fn parse_outline(raw: &str) -> Option<Outline> {
    let sections = parse_sections(raw);
    let title = sections.title?.trim().to_owned();
    let world = join_lines(&sections.world);
    if title.is_empty() || world.is_empty() {
        return None;
    }
    let characters = sections
        .characters
        .into_iter()
        .filter_map(|character| {
            let name = character.name.trim().to_owned();
            (!name.is_empty()).then(|| OutlineCharacter {
                tagline: character
                    .lines
                    .iter()
                    .find(|line| !line.trim().is_empty())
                    .map(|line| line.trim().to_owned())
                    .unwrap_or_default(),
                name,
            })
        })
        .collect();
    Some(Outline {
        title,
        world,
        characters,
    })
}

pub fn parse_expand(raw: &str) -> Option<Expanded> {
    let sections = parse_sections(raw);
    let title = sections.title?.trim().to_owned();
    let world = join_lines(&sections.world);
    if title.is_empty() || world.is_empty() {
        return None;
    }
    let characters = sections
        .characters
        .into_iter()
        .filter_map(parse_expanded_character)
        .collect();
    Some(Expanded {
        title,
        world,
        characters,
        opening: join_lines(&sections.opening),
    })
}

pub fn materialize(root: &Path, expanded: &Expanded) -> DataResult<String> {
    let worlds = data::list_worlds(root)?;
    let mut name = expanded.title.clone();
    let mut suffix = 2;
    while worlds.iter().any(|world| world.name == name) {
        name = format!("{} {suffix}", expanded.title);
        suffix += 1;
    }

    let world_id = data::create_world(root, &name)?;
    data::write_world_md(root, &world_id, &expanded.world)?;
    let colors = [
        "#e07a5f", "#3d84a8", "#f2a541", "#7b9e89", "#8e7cc3", "#c76b8e",
    ];
    for (index, character) in expanded.characters.iter().enumerate() {
        data::write_character(
            root,
            &world_id,
            &CharacterCard {
                id: data::new_id(),
                name: character.name.clone(),
                color: colors[index % colors.len()].to_owned(),
                avatar: character.emoji.clone(),
                tier: Tier::Balanced,
                show_image: true,
                archived: false,
                gen_prompt: String::new(),
                public_md: character.public_md.clone(),
                private_md: character.private_md.clone(),
            },
        )?;
    }
    if !expanded.opening.trim().is_empty() {
        data::append_transcript(
            root,
            &world_id,
            0,
            &TranscriptEvent {
                ts: "2026-07-20T00:00:00+08:00".to_owned(),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: TranscriptKind::Narration,
                text: expanded.opening.clone(),
            },
        )?;
    }
    Ok(world_id)
}

fn genre_line(genres: &[String]) -> String {
    if genres.is_empty() {
        String::new()
    } else {
        format!("Genre hints: {}\n", genres.join(", "))
    }
}

fn parse_sections(raw: &str) -> Sections {
    let mut sections = Sections::default();
    let mut current = CurrentSection::None;
    for line in raw.lines() {
        if let Some((marker, value)) = section_marker(line) {
            match marker {
                "WORLD" => {
                    sections.title = Some(value.to_owned());
                    sections.world.clear();
                    current = CurrentSection::World;
                }
                "CHARACTER" => {
                    sections.characters.push(RawCharacter {
                        name: value.to_owned(),
                        lines: Vec::new(),
                    });
                    current = CurrentSection::Character(sections.characters.len() - 1);
                }
                "OPENING" => current = CurrentSection::Opening,
                _ => unreachable!(),
            }
            continue;
        }
        match current {
            CurrentSection::None => {}
            CurrentSection::World => sections.world.push(line.to_owned()),
            CurrentSection::Character(index) => {
                sections.characters[index].lines.push(line.to_owned())
            }
            CurrentSection::Opening => sections.opening.push(line.to_owned()),
        }
    }
    sections
}

fn section_marker(line: &str) -> Option<(&'static str, &str)> {
    let rest = trim_heading_prefix(line);
    for marker in ["WORLD", "CHARACTER"] {
        if let Some(value) = split_marker_value(rest, marker) {
            return Some((marker, value));
        }
    }
    if rest.eq_ignore_ascii_case("OPENING")
        || rest.eq_ignore_ascii_case("OPENING:")
        || rest == "OPENING："
    {
        return Some(("OPENING", ""));
    }
    None
}

fn split_marker_value<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    let tail = line.get(..marker.len())?;
    if !tail.eq_ignore_ascii_case(marker) {
        return None;
    }
    let value = line.get(marker.len()..)?.trim_start();
    value
        .strip_prefix(':')
        .or_else(|| value.strip_prefix('：'))
        .map(str::trim)
}

fn trim_heading_prefix(line: &str) -> &str {
    let mut hashes = 0;
    for (index, character) in line.char_indices() {
        if character == '#' {
            hashes += 1;
            if hashes > 6 {
                return "";
            }
        } else if !character.is_whitespace() {
            return &line[index..];
        }
    }
    ""
}

fn parse_expanded_character(character: RawCharacter) -> Option<ExpandedCharacter> {
    let name = character.name.trim().to_owned();
    if name.is_empty() {
        return None;
    }
    let mut emoji = None;
    let mut public = Vec::new();
    let mut private = Vec::new();
    let mut preamble = Vec::new();
    let mut current = None;
    let mut has_subsection = false;
    for line in character.lines {
        if let Some(value) = split_marker_value(trim_heading_prefix(&line), "EMOJI") {
            if !value.is_empty() {
                emoji = Some(value.to_owned());
            }
            continue;
        }
        if split_marker_value(trim_heading_prefix(&line), "PUBLIC").is_some() {
            has_subsection = true;
            current = Some(true);
            continue;
        }
        if split_marker_value(trim_heading_prefix(&line), "PRIVATE").is_some() {
            has_subsection = true;
            current = Some(false);
            continue;
        }
        match current {
            Some(true) => public.push(line),
            Some(false) => private.push(line),
            None => preamble.push(line),
        }
    }
    if has_subsection {
        if !preamble.is_empty() {
            preamble.append(&mut public);
            public = preamble;
        }
    } else {
        public = preamble;
    }
    Some(ExpandedCharacter {
        name,
        emoji: emoji.unwrap_or_else(|| "🎭".to_owned()),
        public_md: join_lines(&public),
        private_md: join_lines(&private),
    })
}

fn join_lines(lines: &[String]) -> String {
    lines.join("\n").trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::{materialize, parse_expand, parse_outline, Expanded, ExpandedCharacter};
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
}
