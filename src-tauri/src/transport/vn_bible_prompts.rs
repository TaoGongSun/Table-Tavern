//! VN 劇本聖經（plans/vn-mode-pkg4.md §2.2／§2.4）：生成與角色秘密重骰的提示詞、回覆解析，
//! 以及注入 GM 線的常駐段與當章包。生成提示詞一份英文（欄位名英文、內容用內容語言）；注入段 zh／en 照
//! `scaffold_en` 分流。
use super::messages::{scaffold_en, ChatMessage};
use crate::data::vn::{
    Bible, Branching, CardOrigin, CastRole, Chapter, FlagKind, Preferences, Reveal, Secret,
};
use crate::data::CharacterCard;
use crate::data::Condition;

/// 生成與重骰共用的素材：world.md、角色（id、名字、公開、私設）、GM-only 世界書條目（標題、內文）。
pub struct BibleSource<'a> {
    pub world_md: &'a str,
    pub cast: Vec<CastEntry<'a>>,
    pub gm_entries: Vec<(&'a str, &'a str)>,
    /// 精靈 ① 的偏好（vn.json `preferences`）；匯入建的桌沒有
    pub preferences: Option<&'a Preferences>,
    /// 照指示重寫時帶上目前聖經 JSON；整份重骰不帶
    pub current: Option<&'a str>,
    /// 玩家的修改要求（已 trim、非空）；沒有就是整份重骰
    pub instruction: Option<&'a str>,
}

pub struct CastEntry<'a> {
    pub name: &'a str,
    pub public_md: &'a str,
    pub private_md: &'a str,
}

const SCHEMA: &str = r#"{
  "logline": "the main plot in one sentence",
  "style": "prose style and tone",
  "branching": "full",
  "chapter_max": 8,
  "chapter_reason": "one sentence: why this many chapters",
  "chapters": [{"title": "", "goal": "", "beats": "key beats of the chapter", "end_condition": "when the chapter ends", "plant": ["foreshadowing to plant"], "payoff": ["foreshadowing to pay off"], "anchors": ["a line or image that must appear"]}],
  "secrets": [{"id": "a_secret", "holder": "character name or null", "title": "short title", "text": "the truth", "reveal": {"flag": "flag_id"}, "hint": "how to hint before the reveal"}],
  "flags": [
    {"id": "met_rival", "label": "short label", "type": "bool", "initial": false, "trigger": "when it becomes true"},
    {"id": "trust", "label": "", "type": "counter", "initial": 0, "min": 0, "max": 10, "trigger": "what raises or lowers it"},
    {"id": "route", "label": "", "type": "route", "values": ["a", "b"], "initial": "a", "trigger": "what decides it"}
  ],
  "endings": [{"id": "good", "title": "", "when": [{"kind": "range", "path": "flags.trust", "min": 7}]}, {"id": "default", "title": "", "when": []}]
}"#;

const RULES: &str = "Rules:\n\
- Ids are lowercase ASCII letters, digits and underscores, at most 32 characters, unique within each list.\n\
- \"branching\" is \"none\" (linear: choices change tone and affection, not the chapter goal), \"light\" or \"full\" (choices may lead to different endings). Prefer \"full\" unless the material is clearly linear.\n\
- \"chapter_max\" is your recommended number of chapters for this story, between 3 and 30.\n\
- Secrets: \"holder\" is the name of the character who keeps it, or null for a truth of the world. \"reveal\" is {\"flag\": \"<flag id>\"}, {\"chapter\": <number>} or \"never\" (only hinted, never stated).\n\
- Flags: \"type\" is bool, counter (number with optional min and max) or route (2 to 8 \"values\", \"initial\" must be one of them). \"trigger\" describes in words what story event changes it.\n\
- Endings: \"when\" lists conditions that must all hold, using only paths \"flags.<flag id>\" or \"chapter\". Condition kinds: {\"kind\":\"range\",\"path\":...,\"min\":n,\"max\":n}, {\"kind\":\"flag\",\"path\":...,\"expect\":true}, {\"kind\":\"contains\",\"path\":...,\"any\":[\"value\"]}. Always include one ending with id \"default\" and an empty \"when\".\n\
- Build on the existing characters and their secrets; do not invent new main characters.";

fn material(source: &BibleSource<'_>) -> String {
    let mut text = format!("World:\n{}\n", source.world_md.trim());
    for card in &source.cast {
        text.push_str(&format!(
            "\nCharacter: {}\nPublic:\n{}\nPrivate:\n{}\n",
            card.name,
            card.public_md.trim(),
            card.private_md.trim()
        ));
    }
    if !source.gm_entries.is_empty() {
        text.push_str("\nHidden world truths (GM only):\n");
        for (title, content) in &source.gm_entries {
            text.push_str(&format!("- {title}: {}\n", content.trim()));
        }
    }
    if let Some(preferences) = source.preferences {
        text.push_str(&preference_rules(preferences));
    }
    if let Some(instruction) = source.instruction {
        text.push_str(&format!(
            "\nCurrent bible JSON (keep everything the request does not change):\n{}\n\nPlayer's revision request (apply only this; leave everything else as it is):\n{instruction}\nThe output is still the complete bible: a line `## BIBLE` followed by one ```json fenced block.\n",
            source.current.unwrap_or("(none)")
        ));
    }
    text
}

/// 玩家在精靈 ① 選的偏好：模型照做，章數區間與分支蓋過 RULES 的預設建議。
fn preference_rules(preferences: &Preferences) -> String {
    let mut text = "\nPlayer preferences (follow them):\n".to_owned();
    if !preferences.genres.is_empty() {
        text.push_str(&format!("- Genres: {}\n", preferences.genres.join(", ")));
    }
    if let Some(length) = preferences.length {
        let (min, max) = length.chapters();
        text.push_str(&format!(
            "- \"chapter_max\" must be between {min} and {max}.\n"
        ));
    }
    if let Some(branching) = preferences.branching {
        let code = match branching {
            Branching::None => "none",
            Branching::Light => "light",
            Branching::Full => "full",
        };
        text.push_str(&format!(
            "- \"branching\" must be \"{code}\". {}\n",
            crate::genesis::branching_sentence(branching)
        ));
    }
    for line in crate::genesis::preference_lines(&Preferences {
        length: None,
        branching: None,
        ..preferences.clone()
    })
    .lines()
    {
        text.push_str(&format!("- {line}\n"));
    }
    text
}

/// 整份聖經生成（精靈 ②、匯入建桌、整份重骰共用）。
pub fn bible_messages(source: &BibleSource<'_>, lang: &str) -> Vec<ChatMessage> {
    vec![ChatMessage {
        role: "user".to_owned(),
        content: format!(
            "You are the head writer of a visual novel. From the materials below, write the story bible the game master will follow.\n\n{}\nAll content must be written in the language with BCP-47 code \"{lang}\"; keep the JSON keys in English. Output exactly a line `## BIBLE` followed by one ```json fenced block with this shape:\n\n{SCHEMA}\n\n{RULES}\n- No text outside the block.",
            material(source)
        ),
    }]
}

/// 單一角色的私設與他持有的秘密重骰：其他角色與聖經其餘部分不動。
pub fn secret_messages(
    source: &BibleSource<'_>,
    name: &str,
    bible: &Bible,
    lang: &str,
) -> Vec<ChatMessage> {
    let flags: Vec<&str> = bible.flags.iter().map(|flag| flag.id.as_str()).collect();
    vec![ChatMessage {
        role: "user".to_owned(),
        content: format!(
            "You are the head writer of a visual novel. Rewrite the hidden side of the character {name}: a new private profile (secrets, hidden motives, inner voice) and the story secrets this character keeps. Keep the public profile as it is.\n\n{}\nMain plot: {}\nExisting flag ids: {}\n\nAll content must be written in the language with BCP-47 code \"{lang}\". Output exactly:\n\n## PRIVATE\n<the new private profile, at most 2000 characters>\n\n## SECRETS\n```json\n[{{\"id\": \"a_secret\", \"title\": \"\", \"text\": \"\", \"reveal\": {{\"flag\": \"<existing flag id>\"}}, \"hint\": \"\"}}]\n```\n\nIds are lowercase ASCII letters, digits and underscores. \"reveal\" is {{\"flag\": id}}, {{\"chapter\": n}} or \"never\". No other text.",
            material(source),
            bible.logline,
            if flags.is_empty() { "(none)".to_owned() } else { flags.join(", ") },
        ),
    }]
}

/// 取 `marker` 之後第一個 JSON 圍欄（沒有圍欄就取第一個 `{`／`[` 起到最後一個對應括號）。
fn fenced_json(raw: &str, marker: &str) -> Option<serde_json::Value> {
    let after = &raw[raw.find(marker)? + marker.len()..];
    let body = match after.find("```") {
        Some(start) => {
            let rest = &after[start + 3..];
            let rest = rest.trim_start_matches("json");
            &rest[..rest.find("```").unwrap_or(rest.len())]
        }
        None => {
            let start = after.find(['{', '['])?;
            let end = after.rfind(['}', ']'])?;
            after.get(start..=end)?
        }
    };
    serde_json::from_str(body.trim()).ok()
}

/// `## BIBLE` 段；解析不出物件回 None（呼叫端重試一次）。
pub fn parse_bible_reply(raw: &str) -> Option<Bible> {
    crate::data::vn::parse_bible(&fenced_json(raw, "## BIBLE")?)
}

/// `## PRIVATE` 段文字與 `## SECRETS` 陣列；私設空或陣列解析失敗回 None。
pub fn parse_secret_reply(raw: &str) -> Option<(String, Vec<Secret>)> {
    let start = raw.find("## PRIVATE")? + "## PRIVATE".len();
    let end = raw.find("## SECRETS")?;
    let private = raw.get(start..end)?.trim().to_owned();
    let list = fenced_json(raw, "## SECRETS")?;
    if private.is_empty() || !list.is_array() {
        return None;
    }
    let wrapped = serde_json::json!({ "secrets": list });
    Some((private, crate::data::vn::items(&wrapped, "secrets")))
}

/// 常駐段（GM system 穩定前綴）：主線、文風、分支語意、章目錄、秘密表、旗標表、結局表與揭露寫法。
/// `names`＝角色 id → 名字（秘密持有者）。
pub fn bible_resident(lang: &str, bible: &Bible, names: &dyn Fn(&str) -> String) -> String {
    let en = scaffold_en(lang);
    let pick = |zh: &'static str, english: &'static str| if en { english } else { zh };
    let mut text = format!(
        "\n{}\n{}{}\n{}{}\n{}\n{}\n",
        pick(
            "## 劇本聖經（只有 GM 看得到，不得對玩家複述）",
            "## Story bible (GM only; never recite it to the player)"
        ),
        pick("主線：", "Main plot: "),
        bible.logline,
        pick("文風：", "Style: "),
        bible.style,
        branching_line(en, bible.branching),
        match en {
            true => format!("Chapters ({} in total):", bible.chapter_max),
            false => format!("章節（共 {} 章）：", bible.chapter_max),
        },
    );
    for (index, chapter) in bible.chapters.iter().enumerate() {
        text.push_str(&format!(
            "- {} {}: {}\n",
            index + 1,
            chapter.title,
            chapter.goal
        ));
    }
    if !bible.secrets.is_empty() {
        text.push_str(pick("秘密：\n", "Secrets:\n"));
        for secret in &bible.secrets {
            let holder = secret
                .holder
                .as_deref()
                .map(|id| format!(" ({})", names(id)))
                .unwrap_or_default();
            let reveal = match &secret.reveal {
                Reveal::Flag { flag } => {
                    format!("{}flags.{flag}", pick("揭露：旗標成立時 ", "reveal when "))
                }
                Reveal::Chapter { chapter } => match en {
                    true => format!("reveal in chapter {chapter}"),
                    false => format!("揭露：第 {chapter} 章"),
                },
                Reveal::Never(_) => {
                    pick("永不明說，只能暗示", "never state it, only hint").to_owned()
                }
            };
            text.push_str(&format!(
                "- revealed.{}{holder} {}: {} [{reveal}] {}{}\n",
                secret.id,
                secret.title,
                secret.text,
                pick("暗示：", "Hint: "),
                secret.hint
            ));
        }
    }
    if !bible.flags.is_empty() {
        text.push_str(pick("旗標：\n", "Flags:\n"));
        for flag in &bible.flags {
            let values = match flag.kind {
                FlagKind::Bool => "true / false".to_owned(),
                FlagKind::Route => flag.values.join(" / "),
                FlagKind::Counter => format!(
                    "{} {}–{}",
                    pick("計數，用 delta", "counter, use delta"),
                    flag.min.map_or("".to_owned(), |min| min.to_string()),
                    flag.max.map_or("".to_owned(), |max| max.to_string()),
                ),
            };
            text.push_str(&format!(
                "- flags.{} ({}) [{values}] {}{}\n",
                flag.id,
                flag.label,
                pick("觸發：", "Trigger: "),
                flag.trigger
            ));
        }
    }
    text.push_str(pick("結局：\n", "Endings:\n"));
    for ending in &bible.endings {
        let when: Vec<String> = ending.when.iter().map(condition_text).collect();
        let when = match (
            ending.id == crate::data::vn::DEFAULT_ENDING,
            when.is_empty(),
        ) {
            (true, _) => pick(
                "保底結局（只在導演指示要求時用）",
                "fallback ending (only when the director instruction asks for it)",
            )
            .to_owned(),
            (false, true) => pick("無條件", "always").to_owned(),
            (false, false) => when.join(pick("，且 ", " and ")),
        };
        text.push_str(&format!("- {} {}: {when}\n", ending.id, ending.title));
    }
    text.push_str(pick(
        "劇情揭露某個秘密時，在 <UpdateVariable> 把 revealed.<秘密id> 設為 \"true\"；旗標只在觸發情境發生時更新，值只能用方括號裡的可用值。\n",
        "When the story reveals a secret, set revealed.<secret id> to \"true\" in <UpdateVariable>. Update a flag only when its trigger happens, using only the values in brackets.\n",
    ));
    text
}

fn branching_line(en: bool, branching: Branching) -> &'static str {
    match (en, branching) {
        (false, Branching::None) => "分支：線性作品。選項只改語氣、節奏、好感，不改本章目標。",
        (false, Branching::Light) => "分支：輕度分支。選項影響好感與細節，少數關鍵選擇可改變結局。",
        (false, Branching::Full) => "分支：選項可以分岔到不同結局。",
        (true, Branching::None) => "Branching: linear. Choices change tone, pacing and affection, never the chapter goal.",
        (true, Branching::Light) => "Branching: light. Choices shape affection and details; a few key choices may change the ending.",
        (true, Branching::Full) => "Branching: choices may lead to different endings.",
    }
}

pub(crate) fn condition_text(condition: &Condition) -> String {
    match condition {
        Condition::Range { path, min, max, .. } => match (min, max) {
            (Some(min), Some(max)) => format!("{min} <= {path} <= {max}"),
            (Some(min), None) => format!("{path} >= {min}"),
            (None, Some(max)) => format!("{path} <= {max}"),
            (None, None) => path.clone(),
        },
        Condition::Flag { path, expect } => format!("{path} = {expect}"),
        Condition::Contains { path, any } => format!("{path} ∈ {}", any.join(" / ")),
    }
}

/// 當章包：本章節拍、結束條件、要埋與要收的伏筆、錨句。章號超出表外回空字串。
pub fn bible_chapter(lang: &str, bible: &Bible, chapter: u32) -> String {
    let Some(pack) = (chapter as usize)
        .checked_sub(1)
        .and_then(|index| bible.chapters.get(index))
    else {
        return String::new();
    };
    let en = scaffold_en(lang);
    let Chapter {
        title,
        beats,
        end_condition,
        plant,
        payoff,
        anchors,
        ..
    } = pack;
    let list = |items: &Vec<String>| items.join(if en { "; " } else { "；" });
    match en {
        true => format!(
            "\n## Current chapter ({chapter}: {title})\nBeats: {beats}\nEnds when: {end_condition}\nPlant: {}\nPay off: {}\nAnchor lines: {}\n",
            list(plant),
            list(payoff),
            list(anchors)
        ),
        false => format!(
            "\n## 本章（第 {chapter} 章：{title}）\n節拍：{beats}\n結束條件：{end_condition}\n要埋的伏筆：{}\n要收的伏筆：{}\n錨句：{}\n",
            list(plant),
            list(payoff),
            list(anchors)
        ),
    }
}

#[cfg(test)]
#[path = "vn_bible_prompts_tests.rs"]
mod tests;

/// 登場者出場限制（VN 卡的 `role: memory／deity` 與 `appears`，pkg9 §2.6）：GM 常駐段一行一人，只列有填的；
/// 都沒填回空字串。`cards`＝桌上角色（限制以桌內 id 對到名字，角色刪掉就不列）。
pub fn cast_limits(lang: &str, card: &CardOrigin, cards: &[CharacterCard]) -> String {
    let en = scaffold_en(lang);
    let lines: Vec<String> = cards
        .iter()
        .filter_map(|character| {
            let origin = card.cast.get(&character.id)?;
            let role = match (en, origin.role) {
                (true, Some(CastRole::Memory)) => "appears only in memories",
                (true, Some(CastRole::Deity)) => "a deity who manifests only on special occasions",
                (false, Some(CastRole::Memory)) => "只在回憶中出場",
                (false, Some(CastRole::Deity)) => "神祇，只在特定場合顯現",
                _ => "",
            };
            let parts: Vec<&str> = [role, origin.appears.trim()]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect();
            (!parts.is_empty()).then(|| {
                format!(
                    "- {}: {}",
                    character.name,
                    parts.join(match en {
                        true => "; ",
                        false => "；",
                    })
                )
            })
        })
        .collect();
    if lines.is_empty() {
        return String::new();
    }
    let heading = match en {
        true => "## Appearance limits (keep these characters within them)",
        false => "## 登場者出場限制（照這些限制安排出場）",
    };
    format!("\n{heading}\n{}\n", lines.join("\n"))
}
