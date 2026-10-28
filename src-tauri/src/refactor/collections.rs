//! 套用介面產物時的集合（takeover-list-structured）：骨架區段 `{{#集合}}` 標出哪些欄位是集合，這裡從驗證過的
//! STATE＋骨架＋規則產生 `Mechanism.collections`，把集合項目的規則一律設成每回合注入，並依 STATE 的 JSON 順序
//! 補具名項目的 `_序`。

use crate::data::{
    CollectionItem, CollectionSpec, CollectionValueType, FieldRule, InjectLevel, StateNode,
    COLLECTION_ORDER_KEY,
};
use crate::refactor_ai::shell_sections::{split_sections, value_at};
use std::collections::{BTreeMap, BTreeSet};

/// 骨架裡區段標出的集合路徑；骨架拆不開（驗證時就會擋下）當沒有。
pub(super) fn section_paths(shell: Option<&str>) -> BTreeSet<String> {
    shell
        .and_then(|shell| split_sections(shell).ok())
        .map(|split| {
            split
                .sections
                .into_iter()
                .map(|section| section.path)
                .collect()
        })
        .unwrap_or_default()
}

/// 路徑是集合本身或在集合底下（項目、項目欄位、萬用規則）。
pub(super) fn within(path: &str, collections: &BTreeSet<String>) -> bool {
    collections.iter().any(|base| {
        path == base
            || path
                .strip_prefix(base.as_str())
                .is_some_and(|rest| rest.starts_with('.'))
    })
}

fn value_type(value: &serde_json::Value) -> CollectionValueType {
    match value {
        serde_json::Value::Number(_) => CollectionValueType::Number,
        serde_json::Value::Bool(_) => CollectionValueType::Bool,
        _ => CollectionValueType::Text,
    }
}

/// 一組值的型別：全是數字才算數字、全是布林才算布林，其餘（含沒有值）是文字。
fn common_type<'a>(mut values: impl Iterator<Item = &'a serde_json::Value>) -> CollectionValueType {
    let Some(first) = values.next().map(value_type) else {
        return CollectionValueType::Text;
    };
    if values.all(|value| value_type(value) == first) {
        first
    } else {
        CollectionValueType::Text
    }
}

/// 產生集合描述：具名＝STATE 是物件、序列＝陣列；物件項目的欄位取 `集合.*.欄位` 規則，型別看各項目的
/// JSON 值；純量項目看 `集合.*` 規則（序列空的時候看有沒有它判斷項目種類）。
pub(super) fn build(
    state_fields: &serde_json::Value,
    shell: Option<&str>,
    rules: &BTreeMap<String, FieldRule>,
) -> BTreeMap<String, CollectionSpec> {
    let mut out = BTreeMap::new();
    for path in section_paths(shell) {
        let Some(value) = value_at(state_fields, &path) else {
            continue;
        };
        let items: Vec<&serde_json::Value> = match value {
            serde_json::Value::Object(map) => map.values().collect(),
            serde_json::Value::Array(list) => list.iter().collect(),
            _ => continue,
        };
        let keyed = value.is_object();
        let scalar_rule = rules.contains_key(&format!("{path}.*"));
        let scalar_items = if items.is_empty() {
            !keyed && scalar_rule
        } else {
            items
                .iter()
                .all(|item| !item.is_object() && !item.is_array())
        };
        let spec = if scalar_items {
            CollectionSpec {
                keyed,
                item: CollectionItem::Scalar,
                fields: BTreeMap::new(),
                value_type: Some(common_type(items.into_iter())),
                present_scoped: false,
            }
        } else {
            let prefix = format!("{path}.*.");
            let fields = rules
                .keys()
                .filter_map(|rule| rule.strip_prefix(&prefix))
                .filter(|field| !field.contains('.'))
                .map(|field| {
                    let values = items.iter().filter_map(|item| item.get(field));
                    (field.to_owned(), common_type(values))
                })
                .collect();
            CollectionSpec {
                keyed,
                item: CollectionItem::Object,
                fields,
                value_type: None,
                present_scoped: false,
            }
        };
        out.insert(path, spec);
    }
    out
}

/// 集合本身與項目的規則一律每回合注入：接管桌每回合都要讓 GM 看到在場的人與他們的欄位，
/// snapshot／rare 會讓清單在非對齊回合從「目前狀態」消失。
pub(super) fn inject_every_turn(
    rules: &mut BTreeMap<String, FieldRule>,
    collections: &BTreeSet<String>,
) {
    for (path, rule) in rules.iter_mut() {
        if within(path, collections) {
            rule.inject = InjectLevel::Turn;
        }
    }
}

/// 具名項目補 `_序`：依 STATE 的 JSON 順序；樹上已有名次（沿用目前值時）的不動，產物沒有的接在最後。
pub(super) fn stamp_order(
    tree: &mut BTreeMap<String, StateNode>,
    state_fields: &serde_json::Value,
    collections: &BTreeMap<String, CollectionSpec>,
) {
    for path in collections
        .iter()
        .filter(|(_, spec)| spec.keyed)
        .map(|(path, _)| path)
    {
        let order: Vec<&String> = match value_at(state_fields, path) {
            Some(serde_json::Value::Object(map)) => map.keys().collect(),
            _ => Vec::new(),
        };
        let Some(items) = branch_mut(tree, path) else {
            continue;
        };
        let mut next = items.len();
        for (key, item) in items.iter_mut() {
            let StateNode::Branch(fields) = item else {
                continue;
            };
            if fields.contains_key(COLLECTION_ORDER_KEY) {
                continue;
            }
            let rank = order
                .iter()
                .position(|name| *name == key)
                .unwrap_or_else(|| {
                    next += 1;
                    next
                });
            fields.insert(
                COLLECTION_ORDER_KEY.to_owned(),
                StateNode::Leaf(rank.to_string()),
            );
        }
    }
}

fn branch_mut<'a>(
    tree: &'a mut BTreeMap<String, StateNode>,
    path: &str,
) -> Option<&'a mut BTreeMap<String, StateNode>> {
    path.split('.')
        .try_fold(tree, |branch, key| match branch.get_mut(key) {
            Some(StateNode::Branch(children)) => Some(children),
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::FieldKind;
    use serde_json::json;

    const SHELL: &str = "{{#在場人物}}- {{@key}}: {{身份}} {{体力}}\n{{/在場人物}}{{#行動選項}}- {{.}}\n{{/行動選項}}{{#空序列}}{{.}}{{/空序列}}{{地點}}";

    fn rules() -> BTreeMap<String, FieldRule> {
        [
            ("在場人物", FieldKind::List),
            ("在場人物.*.身份", FieldKind::Text),
            ("在場人物.*.体力", FieldKind::Number),
            ("行動選項", FieldKind::List),
            ("行動選項.*", FieldKind::Text),
            ("空序列", FieldKind::List),
            ("空序列.*", FieldKind::Text),
            ("地點", FieldKind::Text),
        ]
        .into_iter()
        .map(|(path, kind)| (path.to_owned(), FieldRule::for_kind(kind)))
        .collect()
    }

    fn state() -> serde_json::Value {
        json!({
            "在場人物": { "贺铮": { "身份": "驿长", "体力": 70 }, "林远": { "身份": "驿丞", "体力": 55 } },
            "行動選項": ["稳妥", "调查"],
            "空序列": [],
            "地點": "前院",
        })
    }

    #[test]
    fn descriptors_come_from_state_shape_rules_and_item_types() {
        let built = build(&state(), Some(SHELL), &rules());
        assert_eq!(
            serde_json::to_value(&built).unwrap(),
            json!({
                "在場人物": { "keyed": true, "item": "object", "fields": { "体力": "number", "身份": "text" } },
                "行動選項": { "keyed": false, "item": "scalar", "type": "text" },
                "空序列": { "keyed": false, "item": "scalar", "type": "text" },
            })
        );
    }

    #[test]
    fn collection_rules_are_injected_every_turn() {
        let mut rules = rules();
        for rule in rules.values_mut() {
            rule.inject = InjectLevel::Snapshot;
        }
        inject_every_turn(&mut rules, &section_paths(Some(SHELL)));
        assert_eq!(rules["在場人物.*.身份"].inject, InjectLevel::Turn);
        assert_eq!(rules["行動選項"].inject, InjectLevel::Turn);
        assert_eq!(rules["地點"].inject, InjectLevel::Snapshot);
    }

    #[test]
    fn order_follows_the_json_order_and_keeps_existing_ranks() {
        let state = state();
        let mut tree: BTreeMap<String, StateNode> = serde_json::from_value(json!({
            "在場人物": {
                "林远": { "身份": "驿丞" },
                "贺铮": { "身份": "驿长" },
                "周掌柜": { "身份": "军需官", "_序": "0" },
            },
        }))
        .unwrap();
        let built = build(&state, Some(SHELL), &rules());
        stamp_order(&mut tree, &state, &built);
        let rank =
            |name: &str| serde_json::to_value(&tree["在場人物"]).unwrap()[name]["_序"].clone();
        assert_eq!(rank("贺铮"), json!("0"));
        assert_eq!(rank("林远"), json!("1"));
        assert_eq!(rank("周掌柜"), json!("0"));
    }
}
