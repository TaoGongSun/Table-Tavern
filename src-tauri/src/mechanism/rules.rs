use crate::data::{FieldKind, FieldRule, Mechanism};

use super::tree::split_pair;

/// `rule_for` 的公開包裝，給 `transport.rs` 依路徑取欄位規則（渲染狀態樹要知道 inject 層級）。
pub fn rule_for_path(mechanism: &Mechanism, path: &[String], current: Option<&str>) -> FieldRule {
    rule_for(mechanism, path, current)
}

/// 找欄位規則：先精確比對 path 的點分路徑，沒有就找同段數、每段相同或為 `*`
/// 的萬用規則（多筆命中取萬用段最少的那筆），都沒有就依現值形狀推定 kind。
pub(super) fn rule_for(
    mechanism: &Mechanism,
    path: &[String],
    current: Option<&str>,
) -> FieldRule {
    let key = path.join(".");
    if let Some(rule) = mechanism.rules.get(&key) {
        return rule.clone();
    }
    if let Some(rule) = wildcard_rule(mechanism, path) {
        return rule.clone();
    }
    let kind = match current {
        Some(value) if split_pair(value).is_some() => FieldKind::Pair,
        Some(value) if value.trim().parse::<f64>().is_ok() => FieldKind::Number,
        _ => FieldKind::Text,
    };
    FieldRule::for_kind(kind)
}

fn wildcard_rule<'a>(mechanism: &'a Mechanism, path: &[String]) -> Option<&'a FieldRule> {
    let mut best: Option<(usize, &FieldRule)> = None;
    for (rule_path, rule) in &mechanism.rules {
        let segments: Vec<&str> = rule_path.split('.').collect();
        if segments.len() != path.len() {
            continue;
        }
        let mut wildcard_count = 0usize;
        let matched = segments.iter().zip(path.iter()).all(|(segment, actual)| {
            if *segment == "*" {
                wildcard_count += 1;
                true
            } else {
                segment == actual
            }
        });
        if matched && best.is_none_or(|(count, _)| wildcard_count < count) {
            best = Some((wildcard_count, rule));
        }
    }
    best.map(|(_, rule)| rule)
}
