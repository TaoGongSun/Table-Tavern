//! 硬預算（table-status-panel §2.3）：`FieldRule.max_chars`／`max_items` 是資料，不是提示詞。模型寫樹的兩個入口
//! （`<UpdateVariable>` 補丁、```state 圍欄的巢狀欄位）共用同一套夾法：清單先驗形狀（父路徑只收字串陣列，
//! 物件、巢狀、非字串項目整筆丟並回饋），再只留前 N 項、每項截前 N 字；文字截前 N 字。截斷靜默——只記一筆
//! `Trimmed` 進記帳檔，不回饋、不報錯，遊戲照常。只在預設狀態欄桌跑。
use crate::data::{FieldKind, FieldRule, Mechanism};

use super::block_paths::Aligned;
use super::note::shown_pointer;
use super::types::{Patch, Record, RecordKind};
use super::write_guard::{panel_patch, panel_target, value_text};

/// 截前 `max` 字；沒超過回 None。
fn cut(value: &str, max: Option<u32>) -> Option<String> {
    let max = max? as usize;
    (value.chars().count() > max).then(|| value.chars().take(max).collect())
}

/// 夾一份清單（已驗過是字串）：回夾好的項目與有沒有截到。
fn clamp_items(rule: &FieldRule, items: Vec<String>) -> (Vec<String>, bool) {
    let limit = rule.max_items.map_or(usize::MAX, |max| max as usize);
    let mut trimmed = items.len() > limit;
    let kept = items
        .into_iter()
        .take(limit)
        .map(|item| match cut(&item, rule.max_chars) {
            Some(short) => {
                trimmed = true;
                short
            }
            None => item,
        })
        .collect();
    (kept, trimmed)
}

fn trimmed(path: &[String], rule: &FieldRule) -> Record {
    let mut limits = Vec::new();
    if let Some(max) = rule.max_items {
        limits.push(format!("最多 {max} 項"));
    }
    if let Some(max) = rule.max_chars {
        let each = if rule.max_items.is_some() {
            "每項"
        } else {
            ""
        };
        limits.push(format!("{each}最多 {max} 字"));
    }
    Record::new(
        RecordKind::Trimmed,
        path.join("."),
        format!(
            "{} 超過上限（{}），只收前面的部分。",
            path.join("."),
            limits.join("、")
        ),
    )
}

pub(super) fn list_shape(path: &[String]) -> Record {
    let shown = shown_pointer(path);
    Record::new(
        RecordKind::Rejected,
        path.join("."),
        format!("{shown} 是清單欄，值要是字串陣列（例 [\"…\", \"…\"]），整串用 replace 寫；這次的值沒有收。"),
    )
}

/// 補丁入口（`apply_updates_typed` 對齊路徑之後）：預設桌 `characters`／`tracking` 底下先過守門，再驗形狀、夾值，
/// 把夾好的值寫回 `patch`。Err＝整筆拒收（記錄照放），Ok 帶截斷記錄（沒截到為 None）。不歸預設欄位管的補丁原樣放行。
pub(super) fn patch(mechanism: &Mechanism, patch: &mut Patch) -> Result<Option<Record>, Record> {
    let rule = match panel_patch(mechanism, patch) {
        None => return Ok(None),
        Some(Err(record)) => return Err(record),
        Some(Ok(rule)) => rule,
    };
    let value = patch.value.take().unwrap_or(serde_json::Value::Null);
    if rule.kind == FieldKind::List {
        let items: Option<Vec<String>> = match value {
            serde_json::Value::Array(items) => items
                .into_iter()
                .map(|item| match item {
                    serde_json::Value::String(text) => Some(text),
                    _ => None,
                })
                .collect(),
            _ => None,
        };
        let Some(items) = items else {
            return Err(list_shape(&patch.path));
        };
        let (kept, cut_any) = clamp_items(&rule, items);
        patch.value = Some(serde_json::Value::Array(
            kept.into_iter().map(serde_json::Value::String).collect(),
        ));
        return Ok(cut_any.then(|| trimmed(&patch.path, &rule)));
    }
    let Some(text) = value_text(Some(&value)) else {
        return Err(Record::new(
            RecordKind::Rejected,
            patch.path.join("."),
            format!(
                "{} 是文字欄，請用 replace 寫一句文字；這次的值沒有收。",
                shown_pointer(&patch.path)
            ),
        ));
    };
    let short = cut(&text, rule.max_chars);
    let record = short.is_some().then(|| trimmed(&patch.path, &rule));
    patch.value = Some(serde_json::Value::String(short.unwrap_or(text)));
    Ok(record)
}

/// 圍欄入口（`panel_fence` 之後）：預設桌的清單容器底下只能是 `清單.<索引>` 的字串項，形狀不對整支連同欄位丟掉；
/// 項數與字數照規則夾，文字欄截字。回拒收與截斷記錄。
pub(super) fn fence(mechanism: &Mechanism, aligned: &mut Aligned) -> Vec<Record> {
    let mut records = Vec::new();
    let block = &mut aligned.block;
    let mut lists: Vec<(Vec<String>, FieldRule)> = Vec::new();
    let mut bad: Vec<Vec<String>> = Vec::new();
    for list in &block.lists {
        let Some(Ok(rule)) = panel_target(mechanism, list) else {
            continue;
        };
        let shaped = block
            .fields
            .iter()
            .filter(|(path, _)| path.starts_with(list) && path.len() > list.len())
            .all(|(path, _)| {
                path.len() == list.len() + 1 && path[list.len()].bytes().all(|b| b.is_ascii_digit())
            });
        if shaped {
            lists.push((list.clone(), rule));
        } else {
            records.push(list_shape(list));
            bad.push(list.clone());
        }
    }
    block.lists.retain(|list| !bad.contains(list));
    let mut cut_lists: Vec<usize> = Vec::new();
    let fields = std::mem::take(&mut block.fields);
    let targets = std::mem::take(&mut aligned.targets);
    for ((path, value), target) in fields.into_iter().zip(targets) {
        if bad.iter().any(|list| path.starts_with(list)) {
            continue;
        }
        let owner = lists
            .iter()
            .position(|(list, _)| path.len() == list.len() + 1 && path.starts_with(list));
        let value = match owner {
            Some(index) => {
                let rule = &lists[index].1;
                let position: usize = path[path.len() - 1].parse().unwrap_or(usize::MAX);
                let over = rule.max_items.is_some_and(|max| position >= max as usize);
                let short = cut(&value, rule.max_chars);
                if over || short.is_some() {
                    cut_lists.push(index);
                }
                if over {
                    continue;
                }
                short.unwrap_or(value)
            }
            None => match panel_target(mechanism, &path) {
                Some(Ok(rule)) if rule.kind != FieldKind::List => match cut(&value, rule.max_chars)
                {
                    Some(short) => {
                        records.push(trimmed(&path, &rule));
                        short
                    }
                    None => value,
                },
                _ => value,
            },
        };
        block.fields.push((path, value));
        aligned.targets.push(target);
    }
    cut_lists.sort_unstable();
    cut_lists.dedup();
    records.extend(
        cut_lists
            .into_iter()
            .map(|index| trimmed(&lists[index].0, &lists[index].1)),
    );
    records
}

/// 面板整串寫清單（包 2 刪一項、改一項都重寫整串）：只收預設桌有 list 規則的父路徑，先過 `write_refusal`，
/// 再照規則夾項數與字數。回夾好的項目，或給玩家看的拒收句。
pub fn panel_list(
    mechanism: &Mechanism,
    path: &[String],
    items: Vec<String>,
) -> Result<Vec<String>, String> {
    match panel_target(mechanism, path) {
        Some(Ok(rule)) if rule.kind == FieldKind::List => {
            if let Some(detail) = super::write_refusal(mechanism, path, super::Write::Other) {
                return Err(detail);
            }
            Ok(clamp_items(&rule, items).0)
        }
        Some(Err(detail)) => Err(detail),
        _ => Err(format!("{} 不是預設狀態欄的清單欄。", path.join("."))),
    }
}

/// 面板手改一格的值（`set_state_path`）：預設桌有字數上限的欄位（文字欄、清單項）照規則截字；清單欄本身不收
/// 單一字串（整串改用 `set_panel_list`）。不歸預設欄位管的路徑原值放行。
pub fn panel_hand_value(
    mechanism: &Mechanism,
    path: &[String],
    value: &str,
) -> Result<String, String> {
    if !mechanism.default_panel || value.is_empty() {
        return Ok(value.to_owned());
    }
    let rule = super::rules::rule_for(mechanism, path, Some(value));
    if rule.kind == FieldKind::List && super::rules::has_rule(mechanism, path) {
        return Err(format!("{} 是清單欄，請整串寫入。", path.join(".")));
    }
    Ok(cut(value, rule.max_chars).unwrap_or_else(|| value.to_owned()))
}
