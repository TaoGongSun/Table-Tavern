//! GM 預餵骰池：骰面由系統亂數決定，池隨旁白正文保存。
use crate::data::{self, message_vars, state_commit::with_commit};
use std::path::Path;

/// d20 夠滿編 12 人第一回合的先攻加攻擊，傷害骰夠幾隻同型敵人同回合命中。
pub(crate) const PROFILE: [(u32, usize); 7] = [
    (4, 6),
    (6, 16),
    (8, 6),
    (10, 4),
    (12, 4),
    (20, 24),
    (100, 4),
];
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
        ("本輪骰池：各骰型依序取，每顆一次；以(d20#2=19)標原面值，多顆逐一標號，加值／合計寫括號外。先攻、命中、豁免、傷害，凡從池取的骰都要在旁白標號，更新區塊的 dice 不能代替。用罄或缺型就停在下一個行動開始前，不寫半截結果，明說骰子用完、請玩家說一聲就接著跑；禁自報、挑號、重用或用舊池。",
         "直接採用玩家公開擲果，只引數字，不加骰號括號；骰號僅用於本輪池。請玩家擲骰須寫完整式，如1d20+3。",
         "玩家不能擲骰，所有骰由你從池取用。")
    } else {
        ("Current dice pool: consume each die type in order, once per die. Cite (d20#2=19); full-width parentheses and whitespace are allowed. Cite each die separately; put modifiers and totals outside parentheses. Every die taken from the pool (initiative, attacks, saves, damage) must be cited in the narration; `dice` in the update block does not replace that. If exhausted or missing, stop before the next action, never leave one half-resolved, and tell the player the dice ran out and to say anything to continue. Never invent faces, skip, reuse, or use an older pool.",
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
    // 模型常寫全形加減號
    let text: String = text
        .chars()
        .take(32768)
        .map(|ch| match ch {
            '＋' => '+',
            '－' | '−' => '-',
            other => other,
        })
        .collect();
    let mut result = Vec::new();
    for c in formula_regex().captures_iter(&text) {
        let m = c.get(0).unwrap();
        let word = |ch: char| ch.is_ascii_alphanumeric() || ch == '_';
        let operator = |ch: char| "+-*/!".contains(ch);
        // Markdown 粗體不算運算子；`**` 另一側緊接算式字元（1d6**2）才是乘冪
        let left = &text[..m.start()];
        let left = left
            .strip_suffix("**")
            .filter(|l| !l.ends_with(word))
            .unwrap_or(left);
        let right = &text[m.end()..];
        let right = right
            .strip_prefix("**")
            .filter(|r| !r.starts_with(word))
            .unwrap_or(right);
        if left.chars().next_back().is_some_and(word)
            || right.chars().next().is_some_and(word)
            || left.trim_end().chars().next_back().is_some_and(operator)
            || right.trim_start().chars().next().is_some_and(operator)
            || right.trim_start().starts_with('#')
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

/// 骰號算式引用到的一顆骰：骰型與型內序號（1 起算）。
pub(crate) type DieRef = (u32, usize);

/// 戰鬥 HP 綁骰池（ttrpg-combat-tracker §2.5）：本輪池（`format_pool` 的固定格式）＋骰號算式 → 總和與用到的骰。
/// 算式：`dN#k` 以 `+` 相連，最後可接一個 `±整數`（例 `d8#1+d6#3+3`），最多 10 顆、常數 ±9999；骰面一律取池裡的值。
/// 擋下時回給模型看的原因。
pub(crate) fn resolve(pool: &str, formula: &str) -> Result<(i64, Vec<DieRef>), String> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(
            r"^(d[0-9]{1,4}#[0-9]{1,3}(?:\+d[0-9]{1,4}#[0-9]{1,3}){0,9})(?:([+-])([0-9]{1,4}))?$",
        )
        .unwrap()
    });
    let compact: String = formula
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    let shape = || {
        format!("dice「{formula}」格式不對：骰號用 dN#k 以 + 相連、最後可加一個常數（例 d8#1+3），最多 10 顆。")
    };
    let caps = re.captures(&compact).ok_or_else(shape)?;
    let rows: Vec<(u32, Vec<i64>)> = pool
        .lines()
        .filter_map(|line| {
            let (sides, faces) = line.strip_prefix('d')?.split_once(":[")?;
            let faces = faces.strip_suffix(']')?;
            Some((
                sides.parse().ok()?,
                faces.split(',').filter_map(|f| f.parse().ok()).collect(),
            ))
        })
        .collect();
    let mut total: i64 = 0;
    let mut used: Vec<DieRef> = Vec::new();
    for term in caps[1].split('+') {
        let (sides, index) = term[1..].split_once('#').ok_or_else(shape)?;
        let (sides, index): (u32, usize) = (
            sides.parse().map_err(|_| shape())?,
            index.parse().map_err(|_| shape())?,
        );
        let face = rows
            .iter()
            .find(|(row, _)| *row == sides)
            .and_then(|(_, faces)| faces.get(index.wrapping_sub(1)))
            .ok_or_else(|| format!("本輪骰池沒有 d{sides}#{index}，dice 只能引用本輪池裡的骰。"))?;
        if used.contains(&(sides, index)) {
            return Err(format!("d{sides}#{index} 在同一個 dice 裡用了兩次。"));
        }
        used.push((sides, index));
        total += face;
    }
    if let (Some(sign), Some(digits)) = (caps.get(2), caps.get(3)) {
        let constant: i64 = digits.as_str().parse().map_err(|_| shape())?;
        total += if sign.as_str() == "-" {
            -constant
        } else {
            constant
        };
    }
    Ok((total, used))
}
