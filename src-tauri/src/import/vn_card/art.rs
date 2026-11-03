//! VN 卡美術欄位的驗證正規化（plans/vn-mode-pkg9b.md §2.1）：文字剝控制字元、照上限夾斷；鍵不合格、撞名、
//! 重複或超量的整筆丟掉。夾斷記 Clipped、丟掉記 ArtDropped（target＝欄位路徑）。
use super::schema::{notice, Notice, NoticeKind};
use crate::data::vn_art::{valid_art_key, Appearance, ArtEntry, CardArt, CardLocation, CastArt};
use crate::transport::VN_EXPRESSIONS;
use serde_json::Value;
use std::collections::HashSet;

const STYLE_CAP: usize = 600;
const BACKGROUND_RULES_CAP: usize = 800;
const NEGATIVE_CAP: usize = 300;
const SPEC_CAP: usize = 600;
const NOTES_CAP: usize = 300;
const LOCATION_PROMPT_CAP: usize = 500;
const SCALE_CAP: usize = 200;
const LABEL_CAP: usize = 32;
const PROMPT_CAP: usize = 300;
const LOCATIONS_MAX: usize = 20;
const EXPRESSIONS_MAX: usize = 6;
const VARIANTS_MAX: usize = 4;
const HEIGHT_RANGE: (f64, f64) = (50.0, 2000.0);
/// 劇本括號裡的演出指令：表情鍵不得撞名
const DIRECTIVES: [&str; 2] = ["closeup", "shake"];

/// 一個欄位的文字：控制字元換空白、去頭尾空白，超過上限截短並記 Clipped
fn field(value: &Value, key: &str, cap: usize, target: &str, notices: &mut Vec<Notice>) -> String {
    let text: String = value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let text = text.trim();
    if text.chars().count() <= cap {
        return text.to_owned();
    }
    notices.push(notice(NoticeKind::Clipped, target));
    text.chars().take(cap).collect()
}

fn list<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = (usize, &'a Value)> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
}

fn dropped(notices: &mut Vec<Notice>, target: String) {
    notices.push(notice(NoticeKind::ArtDropped, target));
}

/// 卡層級：`art` 與 `locations`
pub(super) fn card_art(
    vn: &Value,
    notices: &mut Vec<Notice>,
) -> (Option<CardArt>, Vec<CardLocation>) {
    let art = vn
        .get("art")
        .filter(|art| art.is_object())
        .map(|art| CardArt {
            style: field(art, "style", STYLE_CAP, "art.style", notices),
            background_rules: field(
                art,
                "background_rules",
                BACKGROUND_RULES_CAP,
                "art.background_rules",
                notices,
            ),
            negative: field(art, "negative", NEGATIVE_CAP, "art.negative", notices),
            cover_as_style_ref: art.get("cover_as_style_ref").and_then(Value::as_bool)
                == Some(true),
        });
    let mut seen = HashSet::new();
    let mut locations = Vec::new();
    for (index, item) in list(vn, "locations") {
        let target = format!("locations[{index}]");
        let key = item
            .get("slug")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        let name = field(item, "name", LABEL_CAP, &format!("{target}.name"), notices);
        if !valid_art_key(key) || name.is_empty() || !seen.insert(key.to_owned()) {
            dropped(notices, target);
            continue;
        }
        if locations.len() == LOCATIONS_MAX {
            dropped(notices, target);
            continue;
        }
        let mut times: Vec<String> = Vec::new();
        for (slot, time) in list(item, "times") {
            match time.as_str().map(str::trim) {
                Some(time) if valid_art_key(time) && !times.iter().any(|seen| seen == time) => {
                    times.push(time.to_owned())
                }
                _ => dropped(notices, format!("{target}.times[{slot}]")),
            }
        }
        locations.push(CardLocation {
            prompt: field(
                item,
                "prompt",
                LOCATION_PROMPT_CAP,
                &format!("{target}.prompt"),
                notices,
            ),
            scale: field(
                item,
                "scale",
                SCALE_CAP,
                &format!("{target}.scale"),
                notices,
            ),
            key: key.to_owned(),
            name,
            times,
        });
    }
    (art, locations)
}

/// 逐角色：`appearance`、`expressions_extra`、`variants`、`height_cm`（`id`＝卡內角色 id，提示用）
pub(super) fn cast_art(member: &Value, id: &str, notices: &mut Vec<Notice>) -> CastArt {
    let appearance = member
        .get("appearance")
        .filter(|value| value.is_object())
        .map(|value| Appearance {
            spec: field(
                value,
                "spec",
                SPEC_CAP,
                &format!("{id}.appearance.spec"),
                notices,
            ),
            notes: field(
                value,
                "notes",
                NOTES_CAP,
                &format!("{id}.appearance.notes"),
                notices,
            ),
        })
        .filter(|appearance| !appearance.spec.is_empty() || !appearance.notes.is_empty());
    let fixed = |key: &str| VN_EXPRESSIONS.contains(&key) || DIRECTIVES.contains(&key);
    let expressions = entries(
        member,
        "expressions_extra",
        EXPRESSIONS_MAX,
        id,
        notices,
        |entry| !fixed(&entry.key),
    );
    let variants = entries(member, "variants", VARIANTS_MAX, id, notices, |entry| {
        entry.key != "base" && crate::vn_sprites::valid_label(&entry.label)
    });
    let height_cm = match member.get("height_cm") {
        None | Some(Value::Null) => None,
        Some(value) => {
            let height = value
                .as_f64()
                .filter(|cm| (HEIGHT_RANGE.0..=HEIGHT_RANGE.1).contains(cm));
            if height.is_none() {
                dropped(notices, format!("{id}.height_cm"));
            }
            height.map(|cm| cm.round() as u16)
        }
    };
    CastArt {
        appearance,
        expressions,
        variants,
        height_cm,
    }
}

/// `[{key, label, prompt}]`：鍵合格、`allowed` 通過、同清單不重複才收（留第一筆），最多 `max` 筆
fn entries(
    member: &Value,
    key: &str,
    max: usize,
    id: &str,
    notices: &mut Vec<Notice>,
    allowed: impl Fn(&ArtEntry) -> bool,
) -> Vec<ArtEntry> {
    let mut kept: Vec<ArtEntry> = Vec::new();
    for (index, item) in list(member, key) {
        let target = format!("{id}.{key}[{index}]");
        let entry_key = item
            .get("key")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        let label = field(
            item,
            "label",
            LABEL_CAP,
            &format!("{target}.label"),
            notices,
        );
        let entry = ArtEntry {
            label: match label.is_empty() {
                true => entry_key.to_owned(),
                false => label,
            },
            prompt: field(
                item,
                "prompt",
                PROMPT_CAP,
                &format!("{target}.prompt"),
                notices,
            ),
            key: entry_key.to_owned(),
        };
        let duplicate = kept.iter().any(|seen| seen.key == entry.key);
        if !valid_art_key(&entry.key) || duplicate || !allowed(&entry) || kept.len() == max {
            dropped(notices, target);
            continue;
        }
        kept.push(entry);
    }
    kept
}
