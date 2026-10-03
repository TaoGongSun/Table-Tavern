//! 狀態樹（葉子一律字串）與 MVU 變數表（帶型別的 JSON）之間的轉換。規則照包 1 前端
//! `card-mvu-shim.ts` 的 `restoreLeaf`／`cleanValue`（計畫第 2 節），兩邊要一致。
use super::json::Json;
use crate::data::StateNode;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// `{{user}}`／`{{char}}` 的代換值；`char` 沒有就保留字面。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Macros {
    pub user: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub char: Option<String>,
}

pub type Tree = BTreeMap<String, StateNode>;

/// MVU 標記可擴充陣列的元素（schema.ts 的 EXTENSIBLE_MARKER）
const EXTENSIBLE_MARKER: &str = "$__META_EXTENSIBLE__$";

fn is_number_text(text: &str) -> bool {
    // ^-?(0|[1-9]\d*)(\.\d+)?([eE][+-]?\d+)?$
    let bytes = text.as_bytes();
    let mut index = 0;
    if bytes.first() == Some(&b'-') {
        index += 1;
    }
    let digits = |from: usize| {
        bytes[from..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count()
    };
    let integer = digits(index);
    if integer == 0 || (integer > 1 && bytes[index] == b'0') {
        return false;
    }
    index += integer;
    if bytes.get(index) == Some(&b'.') {
        let fraction = digits(index + 1);
        if fraction == 0 {
            return false;
        }
        index += 1 + fraction;
    }
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        index += 1;
        if matches!(bytes.get(index), Some(b'+' | b'-')) {
            index += 1;
        }
        let exponent = digits(index);
        if exponent == 0 {
            return false;
        }
        index += exponent;
    }
    index == bytes.len()
}

fn parse_number(text: &str) -> Option<Json> {
    if !is_number_text(text) {
        return None;
    }
    let value: f64 = text.parse().ok()?;
    if !value.is_finite() {
        return None;
    }
    // 整數值照整數存（JS 的 JSON.stringify 也寫成整數），其餘照 f64
    if value.fract() == 0.0 && value.abs() < 9_007_199_254_740_992.0 {
        return Some(Json::Number((value as i64).into()));
    }
    serde_json::Number::from_f64(value).map(Json::Number)
}

fn finite_deep(value: &Json) -> bool {
    match value {
        Json::Number(number) => number.as_f64().is_some_and(f64::is_finite),
        Json::Array(items) => items.iter().all(finite_deep),
        Json::Object(entries) => entries.iter().all(|(_, child)| finite_deep(child)),
        _ => true,
    }
}

fn parse_json(text: &str) -> Option<Json> {
    if !text.starts_with('[') && !text.starts_with('{') {
        return None;
    }
    let value = super::json::parse(text).ok()?;
    finite_deep(&value).then_some(value)
}

/// 狀態樹的葉子 → 帶型別的值。`hint`＝重構記下的原卡欄位型別（number／bool／list）。
pub fn restore_leaf(text: &str, hint: Option<&str>) -> Json {
    match hint {
        Some("number") => return parse_number(text).unwrap_or_else(|| Json::String(text.into())),
        Some("bool") => {
            return match text {
                "true" => Json::Bool(true),
                "false" => Json::Bool(false),
                _ => Json::String(text.into()),
            }
        }
        Some("list") => {
            return match parse_json(text) {
                Some(value @ Json::Array(_)) => value,
                _ => Json::String(text.into()),
            }
        }
        _ => {}
    }
    if let Some(number) = parse_number(text) {
        return number;
    }
    match text {
        "true" => Json::Bool(true),
        "false" => Json::Bool(false),
        "null" => Json::Null,
        _ => parse_json(text).unwrap_or_else(|| Json::String(text.into())),
    }
}

/// 帶型別的值 → 狀態樹葉子（字串照原樣，其餘寫成 JSON 文字）。
pub fn leaf_text(value: &Json) -> String {
    match value {
        Json::String(text) => text.clone(),
        other => other.to_text(),
    }
}

fn substitute(text: &str, macros: &Macros) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let replaced = after.find("}}").and_then(|end| {
            let name = &after[..end];
            let value = if name.eq_ignore_ascii_case("user") {
                Some(macros.user.as_str())
            } else if name.eq_ignore_ascii_case("char") {
                macros.char.as_deref()
            } else {
                None
            };
            value.map(|value| (value, end))
        });
        match replaced {
            Some((value, end)) => {
                out.push_str(value);
                rest = &after[end + 2..];
            }
            None => {
                out.push_str("{{");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn is_array_meta_carrier(value: &Json) -> bool {
    matches!(value.get("$arrayMeta"), Some(Json::Bool(true))) && value.get("$meta").is_some()
}

/// 整份只清一次：移除物件的 `$meta`、陣列裡的 EXTENSIBLE_MARKER 與 metadata 載體元素；字串值與鍵代換巨集，
/// 鍵代換後撞到同層別的鍵就保留原字面。
pub fn clean(value: &Json, macros: Option<&Macros>) -> Json {
    match value {
        Json::String(text) => Json::String(match macros {
            Some(macros) => substitute(text, macros),
            None => text.clone(),
        }),
        Json::Array(items) => Json::Array(
            items
                .iter()
                .filter(|item| {
                    !matches!(item, Json::String(text) if text == EXTENSIBLE_MARKER)
                        && !is_array_meta_carrier(item)
                })
                .map(|item| clean(item, macros))
                .collect(),
        ),
        Json::Object(entries) => {
            let literal: BTreeSet<&str> = entries.iter().map(|(key, _)| key.as_str()).collect();
            let mut result = Json::empty_object();
            for (key, child) in entries {
                if key == "$meta" {
                    continue;
                }
                let renamed = match macros {
                    Some(macros) => substitute(key, macros),
                    None => key.clone(),
                };
                let target = if renamed != *key
                    && (literal.contains(renamed.as_str()) || result.get(&renamed).is_some())
                {
                    key.clone()
                } else {
                    renamed
                };
                result.insert(&target, clean(child, macros));
            }
            result
        }
        other => other.clone(),
    }
}

fn restore_tree(tree: &Tree, path: &mut Vec<String>, types: &BTreeMap<String, String>) -> Json {
    let mut object = Json::empty_object();
    for (key, node) in tree {
        path.push(key.clone());
        let value = match node {
            StateNode::Leaf(text) => {
                restore_leaf(text, types.get(&path.join(".")).map(String::as_str))
            }
            StateNode::Branch(children) => restore_tree(children, path, types),
        };
        path.pop();
        object.insert(key, value);
    }
    object
}

/// 一棵狀態樹 → stat_data（物化）：型別還原、清 metadata、代換巨集。
pub fn tree_to_stat(
    tree: &Tree,
    types: &BTreeMap<String, String>,
    macros: Option<&Macros>,
) -> Json {
    clean(&restore_tree(tree, &mut Vec::new(), types), macros)
}

/// 物化時沿用上一份的原值：同路徑、投影出來的字串相同就留原 JSON 值（含 `[值, 說明]`、字串 "123"）。
pub fn keep_previous(fresh: Json, previous: Option<&Json>) -> Json {
    match (fresh, previous) {
        (Json::Object(entries), Some(previous @ Json::Object(_))) => Json::Object(
            entries
                .into_iter()
                .map(|(key, value)| {
                    let kept = keep_previous(value, previous.get(&key));
                    (key, kept)
                })
                .collect(),
        ),
        (fresh, Some(previous))
            if !matches!(previous, Json::Object(_)) && leaf_text(previous) == leaf_text(&fresh) =>
        {
            previous.clone()
        }
        (fresh, _) => fresh,
    }
}

/// stat_data → 狀態樹投影：物件成分支，其餘成字串葉子。不是物件（沒有 stat_data）就是空樹。
pub fn stat_to_tree(stat: Option<&Json>) -> Tree {
    let mut tree = Tree::new();
    let Some(Json::Object(entries)) = stat else {
        return tree;
    };
    for (key, value) in entries {
        let node = match value {
            Json::Object(_) => StateNode::Branch(stat_to_tree(Some(value))),
            other => StateNode::Leaf(leaf_text(other)),
        };
        tree.insert(key.clone(), node);
    }
    tree
}

/// 新寫進表裡的值怎麼還原型別。
pub struct NewValues<'a> {
    pub types: &'a BTreeMap<String, String>,
    /// 舊值是字串時新值照存字串（面板手改）；false＝一律照包 1 規則還原（GM、匯入）
    pub keep_strings: bool,
    /// 匯入帶進來的值要清 metadata、代換巨集；GM 與手改寫的是字面值
    pub clean: Option<&'a Macros>,
}

fn convert_node(node: &StateNode, path: &mut Vec<String>, rules: &NewValues) -> Json {
    let value = match node {
        StateNode::Leaf(text) => {
            restore_leaf(text, rules.types.get(&path.join(".")).map(String::as_str))
        }
        StateNode::Branch(children) => restore_tree(children, path, rules.types),
    };
    match rules.clean {
        Some(macros) => clean(&value, Some(macros)),
        None => value,
    }
}

/// 狀態樹上的一次改動（before → after）套回 stat_data：沒動到的路徑保留原 JSON 值與鍵順序，改到的換新值，
/// 刪掉的移除。舊值是 `[值, 說明]`（任何長度二的陣列）而新值不是陣列時只改第 0 項（照 MVU updateVariable）。
pub fn merge_tree_change(stat: &Json, before: &Tree, after: &Tree, rules: &NewValues) -> Json {
    let mut result = match stat {
        Json::Object(_) => stat.clone(),
        _ => Json::empty_object(),
    };
    merge_level(&mut result, before, after, &mut Vec::new(), rules);
    result
}

fn merge_level(
    target: &mut Json,
    before: &Tree,
    after: &Tree,
    path: &mut Vec<String>,
    rules: &NewValues,
) {
    for key in before.keys() {
        if !after.contains_key(key) {
            target.remove(key);
        }
    }
    for (key, node) in after {
        let previous = before.get(key);
        if previous == Some(node) {
            continue;
        }
        path.push(key.clone());
        match (previous, node, target.get_mut(key)) {
            (
                Some(StateNode::Branch(old)),
                StateNode::Branch(new),
                Some(child @ Json::Object(_)),
            ) => {
                merge_level(child, old, new, path, rules);
            }
            (_, StateNode::Leaf(text), Some(old)) => {
                let value = if rules.keep_strings && matches!(old, Json::String(_)) {
                    Json::String(text.clone())
                } else {
                    convert_node(node, path, rules)
                };
                match old {
                    Json::Array(pair) if pair.len() == 2 && !matches!(value, Json::Array(_)) => {
                        pair[0] = value;
                    }
                    _ => *old = value,
                }
            }
            _ => {
                let value = convert_node(node, path, rules);
                target.insert(key, value);
            }
        }
        path.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::message_vars::json::parse;

    fn leaf(text: &str) -> String {
        restore_leaf(text, None).to_text()
    }

    #[test]
    fn restore_leaf_matches_package_one_rules() {
        assert_eq!(leaf("12"), "12");
        assert_eq!(leaf("-2.5E-2"), "-0.025");
        assert_eq!(leaf("1e3"), "1000");
        assert_eq!(leaf("-0"), "0");
        for kept in [
            "007",
            ".5",
            "1.",
            "+1",
            "1e999",
            "[1, 说明]",
            "#烦躁 #废了",
            "abc",
        ] {
            assert_eq!(
                restore_leaf(kept, None),
                Json::String(kept.into()),
                "{kept}"
            );
        }
        assert_eq!(leaf("true"), "true");
        assert_eq!(leaf("null"), "null");
        assert_eq!(leaf(r#"[1, "說明"]"#), r#"[1,"說明"]"#);
        assert_eq!(
            restore_leaf("[1, [2, 1e999]]", None),
            Json::String("[1, [2, 1e999]]".into())
        );
        assert_eq!(restore_leaf("12", Some("bool")), Json::String("12".into()));
        assert_eq!(restore_leaf("x", Some("number")), Json::String("x".into()));
        assert_eq!(
            restore_leaf("{\"a\":1}", Some("list")),
            Json::String("{\"a\":1}".into())
        );
    }

    #[test]
    fn clean_drops_meta_and_substitutes_macros_once() {
        let value = parse(
            r#"{"$meta":{"extensible":true},"{{user}}":{"名":"{{USER}} 與 {{char}}","l":["$__META_EXTENSIBLE__$",{"$arrayMeta":true,"$meta":{}},"{{user}}"]},"阿濤":1}"#,
        )
        .unwrap();
        let macros = Macros {
            user: "阿濤$&".into(),
            char: Some("貓".into()),
        };
        let cleaned = clean(&value, Some(&macros));
        // 鍵 {{user}} 代換後撞到既有的「阿濤$&」？沒撞（名字帶 $&）→ 換掉
        assert_eq!(
            cleaned.to_text(),
            r#"{"阿濤$&":{"名":"阿濤$& 與 貓","l":["阿濤$&"]},"阿濤":1}"#
        );
        let collide = parse(r#"{"{{user}}":1,"阿濤":2}"#).unwrap();
        let macros = Macros {
            user: "阿濤".into(),
            char: None,
        };
        assert_eq!(
            clean(&collide, Some(&macros)).to_text(),
            r#"{"{{user}}":1,"阿濤":2}"#
        );
    }

    fn tree(json: &str) -> Tree {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn projection_and_merge_keep_untouched_json_values() {
        let stat = parse(r#"{"z":{"錢":"123","hp":[5,"血量"],"l":[1,2]},"a":true}"#).unwrap();
        let before = stat_to_tree(Some(&stat));
        assert_eq!(
            serde_json::to_string(&before).unwrap(),
            r#"{"a":"true","z":{"hp":"[5,\"血量\"]","l":"[1,2]","錢":"123"}}"#
        );
        let mut after = before.clone();
        crate::data::set_tree_value(&mut after, &["z".into(), "錢".into()], "150");
        crate::data::set_tree_value(&mut after, &["z".into(), "hp".into()], "4");
        crate::data::set_tree_value(&mut after, &["新".into(), "x".into()], "1");
        crate::data::set_tree_value(&mut after, &["a".into()], "");
        let types = BTreeMap::new();
        let gm = NewValues {
            types: &types,
            keep_strings: false,
            clean: None,
        };
        assert_eq!(
            merge_tree_change(&stat, &before, &after, &gm).to_text(),
            r#"{"z":{"錢":150,"hp":[4,"血量"],"l":[1,2]},"新":{"x":1}}"#
        );
        let manual = NewValues {
            keep_strings: true,
            ..gm
        };
        assert_eq!(
            merge_tree_change(&stat, &before, &after, &manual).to_text(),
            r#"{"z":{"錢":"150","hp":[4,"血量"],"l":[1,2]},"新":{"x":1}}"#
        );
    }

    #[test]
    fn materialize_keeps_previous_values_with_same_projection() {
        let previous = parse(r#"{"錢":"123","hp":[5,"血"],"x":1}"#).unwrap();
        let fresh = tree_to_stat(
            &tree(r#"{"錢":"123","hp":"[5,\"血\"]","x":"2"}"#),
            &BTreeMap::new(),
            None,
        );
        assert_eq!(
            keep_previous(fresh, Some(&previous)).to_text(),
            r#"{"hp":[5,"血"],"x":2,"錢":"123"}"#
        );
    }
}
