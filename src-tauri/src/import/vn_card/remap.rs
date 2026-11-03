//! 角色 id 重對應（plans/vn-mode-pkg9.md §2.3 第 2 步、§2.4）：匯入卡內 id → 新桌內 id、匯出桌內 id → `c1…cN`，
//! 兩個方向共用同一套改寫：秘密持有者、世界書條目的可見角色。對不到的一律剔除並記 notice。
use super::schema::{notice, Notice, NoticeKind};
use crate::data::vn::Bible;
use serde_json::{json, Value};
use std::collections::HashMap;

pub(crate) type IdMap = HashMap<String, String>;

/// 秘密的 holder 換成對應的 id；對不到設 null（世界層秘密）。
pub(crate) fn remap_holders(bible: &mut Bible, map: &IdMap, notices: &mut Vec<Notice>) {
    for secret in &mut bible.secrets {
        let Some(holder) = secret.holder.take() else {
            continue;
        };
        secret.holder = map.get(&holder).cloned();
        if secret.holder.is_none() {
            notices.push(notice(NoticeKind::HolderDropped, &secret.id));
        }
    }
}

/// 世界書條目（陣列形或物件形）`extensions.table_tavern.visibility.characters` 換成對應的 id；對不到的剔除，
/// 剔空了改明寫 GM 可見。
pub(crate) fn remap_book(book: &mut Value, map: &IdMap, notices: &mut Vec<Notice>) {
    let entries: Vec<&mut Value> = match book.get_mut("entries") {
        Some(Value::Array(items)) => items.iter_mut().collect(),
        Some(Value::Object(items)) => items.values_mut().collect(),
        _ => return,
    };
    for (index, entry) in entries.into_iter().enumerate() {
        let label = entry
            .get("comment")
            .and_then(Value::as_str)
            .filter(|comment| !comment.is_empty())
            .map_or_else(|| format!("#{index}"), str::to_owned);
        let Some(visibility) = entry
            .pointer_mut("/extensions/table_tavern/visibility")
            .filter(|visibility| visibility.get("characters").is_some_and(Value::is_array))
        else {
            continue;
        };
        let ids: Vec<&str> = visibility["characters"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let mapped: Vec<String> = ids.iter().filter_map(|id| map.get(*id).cloned()).collect();
        if mapped.len() < ids.len() {
            notices.push(notice(NoticeKind::VisibilityDropped, label));
        }
        *visibility = match mapped.is_empty() {
            true => json!("gm"),
            false => json!({ "characters": mapped }),
        };
    }
}
