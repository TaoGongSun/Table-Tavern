//! 世界書條目 → 掃描用的 `WiEntry`：照網頁版 `world-info-book.ts`（ST 06bde939）。
//! 物件形（ST 世界書檔）照 `fromWorldFile`、缺欄照 `newWorldInfoEntryDefinition` 預設；陣列形（V2
//! `character_book`）照 `fromCharacterBook`（`convertCharacterBook`）；內文開頭的 `@@` 裝飾照 `parseDecorators` 拆出。

use serde::Serialize;
use serde_json::{Map, Value};

use super::js_semantics::truthy;

/// ST `world_info_position`。
pub mod position {
    pub const BEFORE: f64 = 0.0;
    pub const AFTER: f64 = 1.0;
    pub const AN_TOP: f64 = 2.0;
    pub const AN_BOTTOM: f64 = 3.0;
    pub const AT_DEPTH: f64 = 4.0;
    pub const EM_TOP: f64 = 5.0;
    pub const EM_BOTTOM: f64 = 6.0;
    pub const OUTLET: f64 = 7.0;
}

/// ST `world_info_logic`。
pub mod logic {
    pub const AND_ANY: f64 = 0.0;
    pub const NOT_ALL: f64 = 1.0;
    pub const NOT_ANY: f64 = 2.0;
    pub const AND_ALL: f64 = 3.0;
}

pub const DEFAULT_DEPTH: f64 = 4.0;
pub const DEFAULT_WEIGHT: f64 = 100.0;
const KNOWN_DECORATORS: [&str; 2] = ["@@activate", "@@dont_activate"];

/// `delayUntilRecursion`：布林或層數（JS 兩種都收）。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum RecursionDelay {
    Flag(bool),
    Level(f64),
}

impl RecursionDelay {
    pub fn truthy(&self) -> bool {
        match self {
            RecursionDelay::Flag(flag) => *flag,
            RecursionDelay::Level(level) => *level != 0.0 && !level.is_nan(),
        }
    }

    /// `delayUntilRecursion === true ? 1 : Number(delayUntilRecursion)`
    pub fn level(&self) -> f64 {
        match self {
            RecursionDelay::Flag(flag) => f64::from(u8::from(*flag)),
            RecursionDelay::Level(level) => *level,
        }
    }
}

/// 掃描用條目；欄位與網頁版 `WiEntry` 一對一（序列化成同名 JSON 給對拍）。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WiEntry {
    /// 穩定 ID（桌面版是 uid 字串）
    #[serde(skip)]
    pub id: String,
    pub key: Option<Vec<String>>,
    pub keysecondary: Vec<String>,
    pub comment: String,
    pub content: String,
    pub constant: bool,
    pub selective: bool,
    /// 原值（可能不是數字；排序照 JS 相減）；`None`＝undefined
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<Value>,
    pub position: f64,
    pub exclude_recursion: bool,
    pub prevent_recursion: bool,
    pub delay_until_recursion: RecursionDelay,
    pub disable: bool,
    pub probability: f64,
    pub use_probability: bool,
    pub depth: f64,
    pub selective_logic: f64,
    pub outlet_name: String,
    pub group: String,
    pub group_override: bool,
    pub group_weight: f64,
    pub scan_depth: Option<f64>,
    pub case_sensitive: Option<bool>,
    pub match_whole_words: Option<bool>,
    pub use_group_scoring: Option<bool>,
    pub role: f64,
    pub sticky: Option<f64>,
    pub cooldown: Option<f64>,
    pub delay: Option<f64>,
    pub match_persona_description: bool,
    pub match_character_description: bool,
    pub match_character_personality: bool,
    pub match_character_depth_prompt: bool,
    pub match_scenario: bool,
    pub match_creator_notes: bool,
    pub triggers: Vec<String>,
    pub ignore_budget: bool,
    pub decorators: Vec<String>,
    /// 桌面版：條目本身是限定可見（內文算私密片段，見 scan 的觸發來源）；網頁版沒有
    #[serde(skip)]
    pub limited: bool,
}

fn strings(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

fn key_list(value: Option<&Value>) -> Option<Vec<String>> {
    matches!(value, Some(Value::Array(_))).then(|| strings(value))
}

fn bool_or(value: Option<&Value>, fallback: bool) -> bool {
    value.and_then(Value::as_bool).unwrap_or(fallback)
}

fn num_or(value: Option<&Value>, fallback: f64) -> f64 {
    nullable_num(value).unwrap_or(fallback)
}

fn nullable_num(value: Option<&Value>) -> Option<f64> {
    value
        .and_then(Value::as_f64)
        .filter(|number| number.is_finite())
}

fn nullable_bool(value: Option<&Value>) -> Option<bool> {
    value.and_then(Value::as_bool)
}

fn str_or(value: Option<&Value>, fallback: &str) -> String {
    value.and_then(Value::as_str).unwrap_or(fallback).to_owned()
}

fn recursion_delay(value: Option<&Value>, fallback: RecursionDelay) -> RecursionDelay {
    match value {
        Some(Value::Bool(flag)) => RecursionDelay::Flag(*flag),
        Some(Value::Number(number)) => number
            .as_f64()
            .map(RecursionDelay::Level)
            .unwrap_or(fallback),
        _ => fallback,
    }
}

/// `parseDecorators`：開頭連續的 `@@` 行裡認得的裝飾拿出來，其餘內容照原樣。
pub fn parse_decorators(content: &str) -> (Vec<String>, String) {
    let known = |line: &str| {
        let data = line
            .strip_prefix('@')
            .filter(|_| line.starts_with("@@@"))
            .unwrap_or(line);
        KNOWN_DECORATORS
            .iter()
            .any(|decorator| data.starts_with(decorator))
    };
    if !content.starts_with("@@") {
        return (Vec::new(), content.to_owned());
    }
    let lines: Vec<&str> = content.split('\n').collect();
    let mut new_content = content.to_owned();
    let mut decorators = Vec::new();
    let mut fallbacked = false;
    for (index, line) in lines.iter().enumerate() {
        if line.starts_with("@@") {
            if line.starts_with("@@@") && !fallbacked {
                continue;
            }
            if known(line) {
                decorators.push(match line.starts_with("@@@") {
                    true => line[1..].to_owned(),
                    false => (*line).to_owned(),
                });
                fallbacked = false;
            } else {
                fallbacked = true;
            }
        } else {
            new_content = lines[index..].join("\n");
            break;
        }
    }
    (decorators, new_content)
}

fn finish(mut entry: WiEntry) -> WiEntry {
    let (decorators, content) = parse_decorators(&entry.content);
    entry.decorators = decorators;
    entry.content = content;
    entry
}

/// 物件形（ST 世界書檔）的一條；`fromWorldFile`。
pub fn from_world_file(entry: &Map<String, Value>) -> WiEntry {
    let get = |field: &str| entry.get(field);
    finish(WiEntry {
        id: String::new(),
        key: match entry.contains_key("key") {
            true => key_list(get("key")),
            false => Some(Vec::new()),
        },
        keysecondary: strings(get("keysecondary")),
        comment: str_or(get("comment"), ""),
        content: str_or(get("content"), ""),
        constant: bool_or(get("constant"), false),
        selective: bool_or(get("selective"), true),
        order: Some(get("order").cloned().unwrap_or(Value::from(100))),
        position: num_or(get("position"), position::BEFORE),
        exclude_recursion: bool_or(get("excludeRecursion"), false),
        prevent_recursion: bool_or(get("preventRecursion"), false),
        delay_until_recursion: recursion_delay(
            get("delayUntilRecursion"),
            RecursionDelay::Level(0.0),
        ),
        disable: bool_or(get("disable"), false),
        probability: num_or(get("probability"), 100.0),
        use_probability: bool_or(get("useProbability"), true),
        depth: num_or(get("depth"), DEFAULT_DEPTH),
        selective_logic: num_or(get("selectiveLogic"), logic::AND_ANY),
        outlet_name: str_or(get("outletName"), ""),
        group: str_or(get("group"), ""),
        group_override: bool_or(get("groupOverride"), false),
        group_weight: num_or(get("groupWeight"), DEFAULT_WEIGHT),
        scan_depth: nullable_num(get("scanDepth")),
        case_sensitive: nullable_bool(get("caseSensitive")),
        match_whole_words: nullable_bool(get("matchWholeWords")),
        use_group_scoring: nullable_bool(get("useGroupScoring")),
        role: num_or(get("role"), 0.0),
        sticky: nullable_num(get("sticky")),
        cooldown: nullable_num(get("cooldown")),
        delay: nullable_num(get("delay")),
        match_persona_description: bool_or(get("matchPersonaDescription"), false),
        match_character_description: bool_or(get("matchCharacterDescription"), false),
        match_character_personality: bool_or(get("matchCharacterPersonality"), false),
        match_character_depth_prompt: bool_or(get("matchCharacterDepthPrompt"), false),
        match_scenario: bool_or(get("matchScenario"), false),
        match_creator_notes: bool_or(get("matchCreatorNotes"), false),
        triggers: strings(get("triggers")),
        ignore_budget: bool_or(get("ignoreBudget"), false),
        decorators: Vec::new(),
        limited: false,
    })
}

/// 陣列形（V2 `character_book`）的一條；`fromCharacterBook`（`convertCharacterBook`）。
pub fn from_character_book(entry: &Map<String, Value>) -> WiEntry {
    let empty = Map::new();
    let ext = entry
        .get("extensions")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let get = |field: &str| entry.get(field);
    let x = |field: &str| ext.get(field);
    let fallback_position = match get("position").and_then(Value::as_str) {
        Some("before_char") => position::BEFORE,
        _ => position::AFTER,
    };
    finish(WiEntry {
        id: String::new(),
        key: key_list(get("keys")),
        keysecondary: strings(get("secondary_keys")),
        comment: str_or(get("comment"), ""),
        content: str_or(get("content"), ""),
        constant: bool_or(get("constant"), false),
        selective: bool_or(get("selective"), false),
        order: get("insertion_order").cloned(),
        position: num_or(x("position"), fallback_position),
        exclude_recursion: bool_or(x("exclude_recursion"), false),
        prevent_recursion: bool_or(x("prevent_recursion"), false),
        delay_until_recursion: recursion_delay(
            x("delay_until_recursion"),
            RecursionDelay::Flag(false),
        ),
        disable: !truthy(get("enabled")),
        probability: num_or(x("probability"), 100.0),
        use_probability: bool_or(x("useProbability"), true),
        depth: num_or(x("depth"), DEFAULT_DEPTH),
        selective_logic: num_or(x("selectiveLogic"), logic::AND_ANY),
        outlet_name: str_or(x("outlet_name"), ""),
        group: str_or(x("group"), ""),
        group_override: bool_or(x("group_override"), false),
        group_weight: num_or(x("group_weight"), DEFAULT_WEIGHT),
        scan_depth: nullable_num(x("scan_depth")),
        case_sensitive: nullable_bool(x("case_sensitive")),
        match_whole_words: nullable_bool(x("match_whole_words")),
        use_group_scoring: nullable_bool(x("use_group_scoring")),
        role: num_or(x("role"), 0.0),
        sticky: nullable_num(x("sticky")),
        cooldown: nullable_num(x("cooldown")),
        delay: nullable_num(x("delay")),
        match_persona_description: bool_or(x("match_persona_description"), false),
        match_character_description: bool_or(x("match_character_description"), false),
        match_character_personality: bool_or(x("match_character_personality"), false),
        match_character_depth_prompt: bool_or(x("match_character_depth_prompt"), false),
        match_scenario: bool_or(x("match_scenario"), false),
        match_creator_notes: bool_or(x("match_creator_notes"), false),
        triggers: strings(x("triggers")),
        ignore_budget: bool_or(x("ignore_budget"), false),
        decorators: Vec::new(),
        limited: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decorators_follow_st() {
        assert_eq!(
            parse_decorators("@@activate\n@@unknown\n@@@dont_activate\nbody"),
            (
                vec!["@@activate".to_owned(), "@@dont_activate".to_owned()],
                "body".to_owned()
            )
        );
        // 全是裝飾行：內文照原樣
        assert_eq!(parse_decorators("@@activate").1, "@@activate".to_owned());
        assert_eq!(parse_decorators("body"), (Vec::new(), "body".to_owned()));
    }
}
