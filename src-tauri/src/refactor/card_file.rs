//! 重構卡封套：`{ format, version, outcome, applied? }`。桌內落檔（refactor-outcome.json）、
//! .json 匯出、PNG manifest 共用同一形狀；applied 是套用時 outcome_index → 角色 id 的顯式映射，
//! 角色改名後匯出角色圖仍配得回去。讀取端同時接受舊版裸 RefactorOutcome（applied 為 None）。

use super::types::RefactorOutcome;
use crate::data::DataResult;
use crate::ui_msg::UiMsg;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

pub const CARD_FORMAT: &str = "table-tavern-refactor-card";
pub const CARD_VERSION: u64 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorAppliedCharacter {
    pub outcome_index: usize,
    /// None＝這一位沒建卡（走 person 條目）。
    pub character_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorApplied {
    pub characters: Vec<RefactorAppliedCharacter>,
    pub player_index: Option<usize>,
    /// 來源桌本地資訊：只留在桌內落檔，對外匯出前去掉；跨桌重現玩家選擇用 player_index。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_card_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorCardFile {
    pub format: String,
    pub version: u64,
    pub outcome: RefactorOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied: Option<RefactorApplied>,
}

impl RefactorCardFile {
    pub fn new(outcome: RefactorOutcome, applied: Option<RefactorApplied>) -> Self {
        Self {
            format: CARD_FORMAT.to_owned(),
            version: CARD_VERSION,
            outcome,
            applied,
        }
    }

    /// 對外版本（.json 匯出、PNG manifest）：去掉來源桌本地的 player_card_id。
    pub fn for_export(&self) -> Self {
        let mut card = self.clone();
        if let Some(applied) = card.applied.as_mut() {
            applied.player_card_id = None;
        }
        card
    }
}

fn invalid(detail: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
    UiMsg::RefactorCardInvalid {
        detail: detail.into(),
    }
    .into_error()
}

/// 解析封套或舊版裸 RefactorOutcome。封套：format 必須相符、version 只收 1（更大的回
/// RefactorCardNewer）、applied 過完整性檢查。玩家選的檔與桌內落檔都走這裡。
pub fn parse_card(text: &str) -> DataResult<RefactorCardFile> {
    let value: Value = serde_json::from_str(text).map_err(|error| invalid(error.to_string()))?;
    parse_card_value(value)
}

pub fn parse_card_value(value: Value) -> DataResult<RefactorCardFile> {
    let Some(object) = value.as_object() else {
        return Err(invalid("not a JSON object"));
    };
    if !object.contains_key("format") {
        let outcome: RefactorOutcome =
            serde_json::from_value(value).map_err(|error| invalid(error.to_string()))?;
        return Ok(RefactorCardFile::new(outcome, None));
    }
    if object.get("format").and_then(Value::as_str) != Some(CARD_FORMAT) {
        return Err(invalid("unknown format"));
    }
    match object.get("version").and_then(Value::as_u64) {
        Some(CARD_VERSION) => {}
        Some(version) if version > CARD_VERSION => {
            return Err(UiMsg::RefactorCardNewer.into_error())
        }
        _ => return Err(invalid("unsupported version")),
    }
    let card: RefactorCardFile =
        serde_json::from_value(value).map_err(|error| invalid(error.to_string()))?;
    if let Some(applied) = &card.applied {
        validate_applied(applied, card.outcome.characters.len()).map_err(invalid)?;
    }
    Ok(card)
}

/// applied 完整性（前端 refactor-review.ts 同一套規則）：恰好覆蓋 0..角色數 每個 index 各一次；
/// character_id 為 None 或非空字串，非 None 的互不重複；player_index 為 None 或指向有卡的 index。
pub fn validate_applied(applied: &RefactorApplied, character_count: usize) -> Result<(), String> {
    if applied.characters.len() != character_count {
        return Err("applied does not cover every character".to_owned());
    }
    let mut seen_indices = BTreeSet::new();
    let mut seen_ids = BTreeSet::new();
    for item in &applied.characters {
        if item.outcome_index >= character_count || !seen_indices.insert(item.outcome_index) {
            return Err("applied index out of range or duplicated".to_owned());
        }
        if let Some(id) = &item.character_id {
            if id.trim().is_empty() || !seen_ids.insert(id.as_str()) {
                return Err("applied character id empty or duplicated".to_owned());
            }
        }
    }
    if let Some(player) = applied.player_index {
        let carded = applied
            .characters
            .iter()
            .any(|item| item.outcome_index == player && item.character_id.is_some());
        if !carded {
            return Err("applied player index has no card".to_owned());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
