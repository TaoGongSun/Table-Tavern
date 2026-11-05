use crate::data::vn::{Branching, Length, Mood, Preferences};
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
    /// VN 桌的主角（`## PLAYER:`，AI 提出、玩家在精靈 ② 改）；聊天桌沒有
    pub player: Option<OutlineCharacter>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineCharacter {
    pub name: String,
    pub tagline: String,
}

/// 展開結果不帶標題：桌名一律用玩家草稿的標題（見 materialize），模型寫了什麼都不採用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expanded {
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
    player: Option<RawCharacter>,
    opening: Vec<String>,
}

enum CurrentSection {
    None,
    World,
    Character(usize),
    Player,
    Opening,
}

/// 精靈建 VN 桌時一起寫進桌的：① 的偏好（名字有填就建玩家卡）與 ② 的劇透鎖。
pub struct VnSetup {
    pub preferences: Preferences,
    pub spoiler_lock: bool,
}

/// VN 偏好（plans/vn-setup-wizard.md §2 ①）換成提示詞句子：氛圍、長度、選擇、主角名。題材另走 `genre_line`。
pub fn preference_lines(preferences: &Preferences) -> String {
    let mut lines = Vec::new();
    if let Some(mood) = preferences.mood {
        lines.push(
            match mood {
                Mood::Light => "Tone: light-hearted and comedic.",
                Mood::Warm => "Tone: heartwarming and moving, allowed to be bittersweet.",
                Mood::Dark => "Tone: tense and dark.",
            }
            .to_owned(),
        );
    }
    if let Some(length) = preferences.length {
        let (min, max) = length.chapters();
        let label = match length {
            Length::Short => "short (finished in one sitting)",
            Length::Medium => "medium (a few evenings)",
            Length::Long => "long",
        };
        lines.push(format!("Story length: {label}, {min}-{max} chapters."));
    }
    if let Some(branching) = preferences.branching {
        lines.push(branching_sentence(branching).to_owned());
    }
    let name = preferences.name.trim();
    lines.push(match name.is_empty() {
        true => "The player character has no name; the story addresses them as \"you\".".to_owned(),
        false => format!("The player character is named {name}."),
    });
    lines.iter().map(|line| format!("{line}\n")).collect()
}

/// 玩家選的「選擇」程度；聖經提示詞也用同一句。
pub fn branching_sentence(branching: Branching) -> &'static str {
    match branching {
        Branching::None => "Choices: none. A linear story the player reads through.",
        Branching::Light => "Choices: occasional. Choices color the story; only a few key ones may change the ending.",
        Branching::Full => "Choices: they change the ending. Plan several distinct endings.",
    }
}

/// VN 桌加在大綱與展開提示詞的段落（偏好＋主角）；聊天桌回空字串。
fn vn_brief(preferences: Option<&Preferences>) -> String {
    preferences.map_or_else(String::new, |preferences| {
        format!("This is a visual novel.\n{}", preference_lines(preferences))
    })
}

/// `preferences` 有值＝VN 桌：空白點子也收（AI 自選題材）、加主角段、簡介當封底文案不劇透。
pub fn outline_messages(
    input: &str,
    genres: &[String],
    lang: &str,
    preferences: Option<&Preferences>,
) -> Vec<ChatMessage> {
    let vn = preferences.is_some();
    let idea = match (input.is_empty() && genres.is_empty(), vn) {
        (true, true) => "(left blank: choose an appealing premise and genre yourself)",
        _ => input,
    };
    let genres = genre_line(genres);
    let brief = vn_brief(preferences);
    let player = match vn {
        true => "\n\n## PLAYER: <the player character's name, or \"you\" when unnamed>\n<one line: who the player character is in this story>",
        false => "",
    };
    let vn_rules = match vn {
        true => "\n- The WORLD text is back-cover copy: make the reader want to play, and never reveal twists, secrets or endings.\n- The PLAYER section is the player character; when the player character is named above, use exactly that name.",
        false => "",
    };
    vec![ChatMessage {
        role: "user".to_owned(),
        content: format!(
            "You are a tabletop RPG game master designing a new campaign from a player's brief idea.\n\nPlayer's idea: {idea}\n{genres}{brief}\nWrite a campaign outline. All content must be written in the language with BCP-47 code \"{lang}\". Output EXACTLY this structure, using these exact English section markers at line start:\n\n## WORLD: <campaign title, one line>\n<2-4 short paragraphs: setting, tone, current situation, and what the player will be doing>\n\n## CHARACTER: <name>\n<one line: who they are and their relation to the player>{player}\n\nRules:\n- Decide the number of characters yourself from the player's idea. A story focused on a single person needs only that person; an ensemble story may need more. Never pad the cast.\n- Characters are NPCs the player will interact with. Do not create a character representing the player.\n- No text outside the sections above. Markers must be exactly \"## WORLD:\" and \"## CHARACTER:\".{vn_rules}"
        ),
    }]
}

pub fn expand_messages(
    input: &str,
    genres: &[String],
    title: &str,
    outline_raw: &str,
    lang: &str,
    preferences: Option<&Preferences>,
) -> Vec<ChatMessage> {
    let genres = genre_line(genres);
    let brief = vn_brief(preferences);
    // VN 桌：開場第一行就是場景標記，精靈 ③ 才有首幕背景可預生（plans/vn-mode-pkg3.md §2）；主角寫進世界與開場
    let opening_scene = match preferences.is_some() {
        true => "\n- The OPENING's first line must be a scene marker `[bg: location | time]` on its own line: location is a short place name and time is the time of day (morning, daytime, dusk or night), both written in the content language.\n- The outline's PLAYER section is the player character: weave who they are into the WORLD text and the OPENING, and do not write a CHARACTER section for them.",
        false => "",
    };
    vec![ChatMessage {
        role: "user".to_owned(),
        content: format!(
            "You are a tabletop RPG game master. The player approved this campaign outline. Expand it into full campaign materials.\n\nPlayer's idea: {input}\n{genres}{brief}Approved outline:\n{outline_raw}\n\nAll content must be written in the language with BCP-47 code \"{lang}\". Output EXACTLY this structure, using these exact English markers at line start:\n\n## WORLD: {title}\n<full world setting in markdown: the setting, key places or factions, tone, current situation. Enough substance for a GM to run scenes from.>\n\n## CHARACTER: <name>\nEMOJI: <one single emoji fitting this character>\nPUBLIC:\n<what everyone can see: appearance, role, public personality>\nPRIVATE:\n<secrets, hidden motives, inner voice — visible only to this character's actor>\n\n## OPENING\n<the opening narration the GM speaks to start the first scene, addressed to the player, ending at a moment that invites the player to act>\n\nRules:\n- Keep the WORLD line exactly as written above; do not rename the campaign.\n- Keep the same characters and names as the outline.\n- Markers exactly as shown. No text outside the sections.{opening_scene}"
        ),
    }]
}

pub fn character_messages(
    input: &str,
    genres: &[String],
    outline_raw: &str,
    hint: &str,
    lang: &str,
) -> Vec<ChatMessage> {
    let genres = genre_line(genres);
    let hint = hint.trim();
    let character_request = if hint.is_empty() {
        "Character request: (none — invent someone who fits the world and fills a gap in the cast)"
            .to_owned()
    } else {
        format!("Character request: {hint}")
    };
    vec![ChatMessage {
        role: "user".to_owned(),
        content: format!(
            "You are a tabletop RPG game master. The player is drafting a campaign outline and wants one more NPC for it.\n\nPlayer's idea: {input}\n{genres}Current outline:\n{outline_raw}\n\n{character_request}\n\nAll content must be written in the language with BCP-47 code \"{lang}\". Output EXACTLY this structure, using this exact English marker at line start:\n\n## CHARACTER: <name>\n<one line: who they are and their relation to the player>\n\nRules:\n- Exactly one character. Do not duplicate or rename characters already in the outline.\n- No text outside the section."
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
    Some(Outline {
        title,
        world,
        characters: sections
            .characters
            .into_iter()
            .filter_map(outline_character)
            .collect(),
        player: sections.player.and_then(outline_character),
    })
}

/// 名字＋第一行非空文字；沒名字回 None。
fn outline_character(character: RawCharacter) -> Option<OutlineCharacter> {
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
}

pub fn parse_character(raw: &str) -> Option<OutlineCharacter> {
    parse_sections(raw)
        .characters
        .into_iter()
        .find_map(outline_character)
}

pub fn parse_expand(raw: &str) -> Option<Expanded> {
    let sections = parse_sections(raw);
    let world = join_lines(&sections.world);
    if world.is_empty() {
        return None;
    }
    let characters = sections
        .characters
        .into_iter()
        .filter_map(parse_expanded_character)
        .collect();
    Some(Expanded {
        world,
        characters,
        opening: join_lines(&sections.opening),
    })
}

/// 玩家草稿的標題：trim 後非空才算數。開桌前（呼叫模型、寫檔之前）先過這關。
pub fn draft_title(title: &str) -> Option<&str> {
    let title = title.trim();
    (!title.is_empty()).then_some(title)
}

/// `title` 是玩家草稿的標題，桌名與重名補號都以它為準。`vn`＝精靈建的 VN 桌：偏好與劇透鎖寫進 vn.json、
/// 設定階段從 ② 起；有名字就建一張只有名字的玩家卡（GM 用名字稱呼，走一般玩家分工）。
pub fn materialize(
    root: &Path,
    title: &str,
    expanded: &Expanded,
    kind: data::TableKind,
    vn: Option<VnSetup>,
) -> DataResult<String> {
    let title = draft_title(title).ok_or_else(|| data::invalid_data("table title is empty"))?;
    let worlds = data::list_worlds(root)?;
    let mut name = title.to_owned();
    let mut suffix = 2;
    while worlds.iter().any(|world| world.name == name) {
        name = format!("{title} {suffix}");
        suffix += 1;
    }

    let world_id = data::create_world_of_kind(root, &name, kind)?;
    // 建桌的許可在 create_world 裡就放下了。這裡再拿一次，蓋住後面的世界書與開場白，
    // 更新閘門才等得了這次開桌寫完。
    let _permit = data::world_write_permit(&world_id)?;
    data::write_world_md(root, &world_id, &expanded.world)?;
    let colors = [
        "#e07a5f", "#3d84a8", "#f2a541", "#7b9e89", "#8e7cc3", "#c76b8e",
    ];
    let player = vn
        .as_ref()
        .map(|setup| setup.preferences.name.trim())
        .unwrap_or_default();
    // 模型偶爾把主角也寫成一段 CHARACTER：VN 桌主角由玩家卡代表，不另建 NPC
    let cast = expanded
        .characters
        .iter()
        .filter(|character| player.is_empty() || character.name.trim() != player);
    for (index, character) in cast.enumerate() {
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
                stage: false,
            },
        )?;
    }
    if !expanded.opening.trim().is_empty() {
        data::append_transcript(root, &world_id, 0, &opening_event(&expanded.opening))?;
    }
    if let Some(setup) = vn {
        let name = setup.preferences.name.trim();
        if !name.is_empty() {
            let id = data::new_id();
            data::write_character(
                root,
                &world_id,
                &CharacterCard {
                    id: id.clone(),
                    name: name.to_owned(),
                    color: colors[expanded.characters.len() % colors.len()].to_owned(),
                    avatar: "🙂".to_owned(),
                    tier: Tier::Balanced,
                    show_image: true,
                    archived: false,
                    gen_prompt: String::new(),
                    public_md: String::new(),
                    private_md: String::new(),
                    stage: false,
                },
            )?;
            data::set_player_card(root, &world_id, Some(id))?;
        }
        data::state_commit::with_commit(root, &world_id, |tx| {
            data::vn::start_setup(tx, setup.preferences, setup.spoiler_lock)
        })?;
    }
    Ok(world_id)
}

/// 開場旁白事件（`opening: true`）：genesis 建桌與 VN 卡匯入共用。
pub fn opening_event(text: &str) -> TranscriptEvent {
    TranscriptEvent {
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
        raw: None,
        ts: "2026-07-20T00:00:00+08:00".to_owned(),
        speaker_id: String::new(),
        speaker_name: "GM".to_owned(),
        kind: TranscriptKind::Narration,
        text: text.to_owned(),
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening: true,
    }
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
                "PLAYER" => {
                    sections.player = Some(RawCharacter {
                        name: value.to_owned(),
                        lines: Vec::new(),
                    });
                    current = CurrentSection::Player;
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
            CurrentSection::Player => {
                if let Some(player) = sections.player.as_mut() {
                    player.lines.push(line.to_owned());
                }
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
    // 主角段只認標題列：內文裡一句「Player: ……」不能被吃成段落
    if line.trim_start().starts_with('#') {
        if let Some(value) = split_marker_value(rest, "PLAYER") {
            return Some(("PLAYER", value));
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
mod tests;
