//! 同一路徑有多份候選規則（不同機制條目各寫一份、或同條目兩種字形對齊到同一路徑）時的合併與裁決。
//! 吸收是並行呼叫、彼此看不到別條寫的規則，選填的 min／max／inject 常寫得不一樣；相容就合併，
//! 真矛盾只裁決這一欄，不讓整次套用失敗。

use crate::data::{FieldKind, FieldRule, InjectLevel};

/// 一份候選：`uid` 是來源 uid 的數值（條目有多個取最小、非數字是 None），裁決時越小越優先，
/// 同 uid 依候選在清單裡的先後；`origin` 只用來記 log。
pub(super) struct RuleCandidate {
    pub(super) rule: FieldRule,
    pub(super) uid: Option<u64>,
    pub(super) origin: String,
}

/// 來源 uid 字串裡數值最小的那個。
pub(super) fn min_uid<'a>(uids: impl IntoIterator<Item = &'a String>) -> Option<u64> {
    uids.into_iter().filter_map(|uid| uid.parse().ok()).min()
}

/// 整欄的候選收齊後一次裁決：全部相容就合併；任一對不相容＝真矛盾，原樣取來源 uid 最小那份
/// （不逐對合併，敗方的限制不會殘留），記 warn。
pub(super) fn reconcile(path: &str, candidates: &[RuleCandidate]) -> FieldRule {
    if let [only] = candidates {
        return only.rule.clone();
    }
    let rules: Vec<&FieldRule> = candidates.iter().map(|candidate| &candidate.rule).collect();
    if let Some(merged) = merge_compatible(&rules) {
        return merged;
    }
    let winner = candidates
        .iter()
        .enumerate()
        .min_by_key(|(order, candidate)| (candidate.uid.unwrap_or(u64::MAX), *order))
        .map(|(_, candidate)| candidate)
        .expect("reconcile 至少要有一份候選");
    let origins: Vec<&str> = candidates
        .iter()
        .map(|candidate| candidate.origin.as_str())
        .collect();
    log::warn!(
        "重構套用：{path} 的規則互相矛盾（{}），採用「{}」那份",
        origins.join("／"),
        winner.origin
    );
    winner.rule.clone()
}

/// 相容＝kind、update、formula 相同，branch 相同或一邊缺省；min／max 依該型的缺省語意取交集，
/// 交集為空不相容；inject 取最頻繁。`same_as` 不比（套用時由盤點重寫）。不相容回 None。
pub(super) fn merge_compatible(rules: &[&FieldRule]) -> Option<FieldRule> {
    let (first, rest) = rules.split_first()?;
    let mut merged = (*first).clone();
    merged.same_as = None;
    for rule in rest {
        if rule.kind != merged.kind
            || rule.update != merged.update
            || rule.formula != merged.formula
        {
            return None;
        }
        merged.branch = match (merged.branch.take(), &rule.branch) {
            (None, other) => other.clone(),
            (Some(mine), None) => Some(mine),
            (Some(mine), Some(other)) if mine == *other => Some(mine),
            _ => return None,
        };
        if inject_rank(rule.inject) > inject_rank(merged.inject) {
            merged.inject = rule.inject;
        }
    }
    let (default_min, default_max) = bound_defaults(merged.kind);
    merged.min = intersect(rules.iter().map(|rule| rule.min), default_min, f64::max);
    merged.max = intersect(rules.iter().map(|rule| rule.max), default_max, f64::min);
    if let (Some(min), Some(max)) = (merged.min.or(default_min), merged.max.or(default_max)) {
        if min > max {
            return None;
        }
    }
    Some(merged)
}

/// 每回合送＞快照＞罕見（不靠 enum 宣告順序）。
fn inject_rank(level: InjectLevel) -> u8 {
    match level {
        InjectLevel::Turn => 2,
        InjectLevel::Snapshot => 1,
        InjectLevel::Rare => 0,
    }
}

/// 規則沒寫 min／max 時執行端實際用的值：pair 的下限是 0（上限來自現值/上限）、roll 是 1–100 的
/// 重擲範圍，其餘是無界（None）。
fn bound_defaults(kind: FieldKind) -> (Option<f64>, Option<f64>) {
    match kind {
        FieldKind::Pair => (Some(0.0), None),
        FieldKind::Roll => (Some(1.0), Some(100.0)),
        _ => (None, None),
    }
}

/// 全部缺省就維持缺省；否則缺省的換成該型的實際值（無界的不參與）再用 `pick` 收斂。
fn intersect(
    values: impl Iterator<Item = Option<f64>>,
    default: Option<f64>,
    pick: fn(f64, f64) -> f64,
) -> Option<f64> {
    let values: Vec<Option<f64>> = values.collect();
    if values.iter().all(Option::is_none) {
        return None;
    }
    values
        .into_iter()
        .filter_map(|value| value.or(default))
        .reduce(pick)
}
