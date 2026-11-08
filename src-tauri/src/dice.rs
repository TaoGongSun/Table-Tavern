//! GM 預餵骰池：骰面由系統亂數決定，池隨旁白正文保存。
use crate::data::{self, message_vars, state_commit::with_commit};
use std::path::Path;

pub(crate) const PROFILE: [(u32, usize); 7] =
    [(4, 2), (6, 12), (8, 2), (10, 2), (12, 2), (20, 8), (100, 4)];
const OPEN: &str = "[dice_pool]\n";
const CLOSE: &str = "\n[/dice_pool]";

fn face(sides: u32) -> Result<u32, String> {
    let limit = u32::MAX - u32::MAX % sides;
    loop {
        let mut bytes = [0; 4];
        getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
        let value = u32::from_le_bytes(bytes);
        if value < limit {
            return Ok(value % sides + 1);
        }
    }
}

fn format_pool(mut roll: impl FnMut(u32) -> Result<u32, String>) -> Result<String, String> {
    PROFILE
        .iter()
        .map(|&(sides, count)| {
            let faces = (0..count)
                .map(|_| roll(sides).map(|n| n.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("d{sides}:[{}]", faces.join(",")))
        })
        .collect::<Result<Vec<_>, String>>()
        .map(|rows| rows.join("\n"))
}

pub(crate) fn enabled(state: &data::WorldState) -> bool {
    state.dice_enabled && state.table_kind == data::TableKind::Chat && !state.replay
}

pub(crate) fn begin(root: &Path, world: &str, turn: &str) -> Result<Option<String>, String> {
    with_commit(root, world, |tx| {
        let state = data::read_state(root, world).map_err(|e| e.to_string())?;
        let pool = enabled(&state).then(|| format_pool(face)).transpose()?;
        message_vars::set_dice_pool(tx, turn, pool.clone()).map_err(|e| e.to_string())?;
        Ok(pool)
    })
}

/// 唯讀容量預估用最大位數，不消耗亂數也不寫回合。
pub(crate) fn measured_pool() -> String {
    format_pool(Ok).expect("固定骰型必可格式化")
}

pub(crate) fn instruction(pool: &str, room: bool, lang: &str) -> String {
    let (common, solo, party) = if lang.starts_with("zh") {
        ("本輪骰池：各骰型依序取，每顆一次；以(d20#2=19)標原面值，多顆逐一標號，加值／合計寫括號外。用罄或缺型就明說待補，禁自報、挑號、重用或用舊池。",
         "直接採用玩家公開擲果，只引數字，不加骰號括號；骰號僅用於本輪池。請玩家擲骰須寫完整式，如1d20+3。",
         "玩家不能擲骰，所有骰由你從池取用。")
    } else {
        ("Current dice pool: consume each die type in order, once per die. Cite (d20#2=19); full-width parentheses and whitespace are allowed. Cite each die separately; put modifiers and totals outside parentheses. If exhausted or missing, ask for replenishment. Never invent faces, skip, reuse, or use an older pool.",
         "Use public player rolls as given. Quote their numbers without dice-reference parentheses: dice references are only for this pool. Ask players with a complete formula such as 1d20+3.",
         "Players cannot roll dice here. Take every roll from this pool yourself.")
    };
    format!(
        "{OPEN}{common}\n{}\n{pool}{CLOSE}",
        if room { party } else { solo }
    )
}

/// 只供 TailStale 計帳，不把這段再注入一次。
pub(crate) fn retained(instruction: &str) -> Option<&str> {
    let start = instruction.find(OPEN)?;
    let end = instruction[start..].find(CLOSE)? + start + CLOSE.len();
    Some(&instruction[start..end])
}

pub(crate) fn set_enabled(
    root: &Path,
    world: &str,
    on: bool,
    lang: &str,
    room: bool,
) -> Result<(), String> {
    with_commit(root, world, |tx| {
        message_vars::refuse_during_turn(tx).map_err(|e| e.to_string())?;
        let mut state = data::read_state(root, world).map_err(|e| e.to_string())?;
        data::vn::refuse_replay(&state).map_err(|e| e.to_string())?;
        if state.table_kind != data::TableKind::Chat {
            return Err("dice-chat-only".into());
        }
        if state.dice_enabled == on {
            return Ok(());
        }
        if on {
            let copy: serde_json::Value =
                serde_json::from_str(include_str!("../../src/features/dice/dice-copy.json"))
                    .expect("骰子字典為有效 JSON");
            let text = copy.get(lang).unwrap_or(&copy["en"])
                [if room { "enabledRoom" } else { "enabledSolo" }]
            .as_str()
            .expect("骰子通知字串");
            let event = serde_json::from_value(serde_json::json!({
                "ts": data::local_timestamp().unwrap_or_default(), "speaker_id": "", "speaker_name": "",
                "kind": "system", "text": text
            })).map_err(|e| e.to_string())?;
            data::append_transcript(root, world, state.current_scene, &event)
                .map_err(|e| e.to_string())?;
        }
        state.dice_enabled = on;
        data::write_state(root, world, &state).map_err(|e| e.to_string())
    })
}

#[cfg(test)]
#[path = "dice_tests.rs"]
mod tests;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Formula {
    count: u32,
    sides: u32,
    modifier: i32,
}
impl Formula {
    fn canonical(&self) -> String {
        format!(
            "{}d{}{}",
            self.count,
            self.sides,
            if self.modifier == 0 {
                String::new()
            } else {
                format!("{:+}", self.modifier)
            }
        )
    }
}
fn formula_regex() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)([0-9]*)d([0-9]+)(?:\s*([+-])\s*([0-9]+))?").unwrap())
}
fn captured(c: &regex::Captures<'_>) -> Option<Formula> {
    let count = if c[1].is_empty() {
        1
    } else {
        c[1].parse().ok()?
    };
    let sides = c[2].parse().ok()?;
    let magnitude: i32 = c.get(4).map_or(Some(0), |m| m.as_str().parse().ok())?;
    let modifier = if c.get(3).is_some_and(|m| m.as_str() == "-") {
        -magnitude
    } else {
        magnitude
    };
    ((1..=100).contains(&count)
        && (2..=1000).contains(&sides)
        && (-9999..=9999).contains(&modifier))
    .then_some(Formula {
        count,
        sides,
        modifier,
    })
}
fn parse_formula(text: &str) -> Option<Formula> {
    if text.len() > 128 {
        return None;
    }
    let text = text.trim();
    let c = formula_regex().captures(text)?;
    (c.get(0)?.as_str() == text).then(|| captured(&c)).flatten()
}
pub(crate) fn suggestions(text: &str) -> Vec<String> {
    let text: String = text.chars().take(32768).collect();
    let mut result = Vec::new();
    for c in formula_regex().captures_iter(&text) {
        let m = c.get(0).unwrap();
        let left = &text[..m.start()];
        let right = &text[m.end()..];
        let word = |ch: char| ch.is_ascii_alphanumeric() || ch == '_';
        let operator = |ch: char| "+-*/!".contains(ch);
        if left.chars().next_back().is_some_and(word)
            || right.chars().next().is_some_and(word)
            || left.trim_end().chars().next_back().is_some_and(operator)
            || right.trim_start().chars().next().is_some_and(operator)
        {
            continue;
        }
        if let Some(formula) = captured(&c) {
            let value = formula.canonical();
            if !result.contains(&value) {
                result.push(value);
            }
            if result.len() == 8 {
                break;
            }
        }
    }
    result
}

pub(crate) fn roll(
    root: &Path,
    world: &str,
    scene: u64,
    formula: &str,
) -> Result<data::TranscriptEvent, String> {
    with_commit(root, world, |tx| {
        message_vars::refuse_during_turn(tx).map_err(|e| e.to_string())?;
        let state = data::read_state(root, world).map_err(|e| e.to_string())?;
        if state.current_scene != scene {
            return Err("dice-scene-changed".into());
        }
        if crate::chat_turn::solo_relay_active(world) {
            return Err("dice-relay-active".into());
        }
        crate::room::refuse_if_active(world)?;
        data::vn::refuse_replay(&state).map_err(|e| e.to_string())?;
        if !enabled(&state) {
            return Err("dice-disabled".into());
        }
        let player = data::read_player_card(root, world).map_err(|e| e.to_string())?;
        let formula = parse_formula(formula).ok_or("dice-invalid-formula")?;
        let faces = (0..formula.count)
            .map(|_| face(formula.sides))
            .collect::<Result<Vec<_>, _>>()?;
        let total = faces.iter().sum::<u32>() as i32 + formula.modifier;
        let name = player
            .map(|p| p.name)
            .filter(|s| !s.trim().is_empty())
            .map(|name| format!("{name} "))
            .unwrap_or_default();
        let modifier = if formula.modifier == 0 {
            String::new()
        } else {
            format!(" {:+}", formula.modifier)
        };
        let values = faces
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let event = serde_json::from_value(serde_json::json!({
            "ts": data::local_timestamp().unwrap_or_default(), "speaker_id": "", "speaker_name": "",
            "kind": "system", "marker": {"type": "dice_roll"},
            "text": format!("{name}{}: [{values}]{modifier} = {total}", formula.canonical())
        }))
        .map_err(|e| e.to_string())?;
        data::append_event(root, world, scene, &event, None)
            .map(|(e, _)| e)
            .map_err(|e| e.to_string())
    })
}
