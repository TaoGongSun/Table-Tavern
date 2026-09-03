use crate::data::{self, StateNode};
use std::collections::BTreeMap;

use super::types::{Record, RecordKind};

pub(super) fn build_notes(records: &[Record]) -> Vec<String> {
    let mut notes = Vec::new();
    for record in records {
        if record.kind != RecordKind::Rejected {
            continue;
        }
        if notes.contains(&record.detail) {
            continue;
        }
        notes.push(record.detail.clone());
        if notes.len() >= 5 {
            break;
        }
    }
    notes
}

// ---------------------------------------------------------------------
// 樹操作小工具（本檔自用，data::set_tree_value 只覆蓋葉子字串的情形）
// ---------------------------------------------------------------------

pub(super) fn leaf_value(node: &StateNode) -> Option<&str> {
    match node {
        StateNode::Leaf(value) => Some(value),
        StateNode::Branch(_) => None,
    }
}

/// 在 path 寫入任意節點（含整棵子樹），缺的中間層自動補成 Branch；
/// 撞到既有葉子擋路就放棄，回傳 Err 不動樹。路徑最後一段是 `-` 時，
/// 在該 Branch 用「目前沒被用掉的最小非負整數」當 key 附加。
pub(super) fn insert_node(
    branch: &mut BTreeMap<String, StateNode>,
    path: &[String],
    node: StateNode,
) -> Result<(), ()> {
    let (first, rest) = path.split_first().ok_or(())?;
    if rest.is_empty() {
        let key = if first == "-" {
            next_free_key(branch)
        } else {
            first.clone()
        };
        branch.insert(key, node);
        return Ok(());
    }
    let entry = branch
        .entry(first.clone())
        .or_insert_with(|| StateNode::Branch(BTreeMap::new()));
    match entry {
        StateNode::Branch(children) => insert_node(children, rest, node),
        StateNode::Leaf(_) => Err(()),
    }
}

fn next_free_key(branch: &BTreeMap<String, StateNode>) -> String {
    let mut candidate = 0u64;
    loop {
        let key = candidate.to_string();
        if !branch.contains_key(&key) {
            return key;
        }
        candidate += 1;
    }
}

/// 取出並刪除 path 上的節點，因此變空的父分支一併剪掉。
pub(super) fn take_node(
    branch: &mut BTreeMap<String, StateNode>,
    path: &[String],
) -> Option<StateNode> {
    let (first, rest) = path.split_first()?;
    if rest.is_empty() {
        return branch.remove(first);
    }
    let StateNode::Branch(children) = branch.get_mut(first)? else {
        return None;
    };
    let removed = take_node(children, rest)?;
    if let Some(StateNode::Branch(children)) = branch.get(first) {
        if children.is_empty() {
            branch.remove(first);
        }
    }
    Some(removed)
}

/// JSON 值轉狀態樹節點：物件→Branch（遞迴）、陣列→以 0/1/… 為 key 的 Branch、
/// 純量→Leaf 字串（整數不印成 5.0）。
pub(super) fn json_to_node(value: &serde_json::Value) -> StateNode {
    match value {
        serde_json::Value::Object(map) => StateNode::Branch(
            map.iter()
                .map(|(k, v)| (k.clone(), json_to_node(v)))
                .collect(),
        ),
        serde_json::Value::Array(items) => StateNode::Branch(
            items
                .iter()
                .enumerate()
                .map(|(index, item)| (index.to_string(), json_to_node(item)))
                .collect(),
        ),
        serde_json::Value::String(text) => StateNode::Leaf(text.clone()),
        serde_json::Value::Number(number) => StateNode::Leaf(format_json_number(number)),
        serde_json::Value::Bool(flag) => StateNode::Leaf(flag.to_string()),
        serde_json::Value::Null => StateNode::Leaf(String::new()),
    }
}

fn format_json_number(number: &serde_json::Number) -> String {
    if let Some(value) = number.as_i64() {
        return value.to_string();
    }
    if let Some(value) = number.as_u64() {
        return value.to_string();
    }
    number.to_string()
}

pub(super) fn format_num(value: f64) -> String {
    if value.is_finite() && value == value.trunc() && value.abs() < 1e15 {
        (value as i64).to_string()
    } else {
        format!("{value}")
    }
}

pub(super) fn split_pair(value: &str) -> Option<(f64, f64)> {
    let mut parts = value.splitn(2, '/');
    let current = parts.next()?.trim().parse::<f64>().ok()?;
    let max = parts.next()?.trim().parse::<f64>().ok()?;
    Some((current, max))
}

pub(super) fn value_as_f64(value: Option<&serde_json::Value>) -> Option<f64> {
    match value? {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

/// 一段點分路徑在樹上求出來的形狀：有葉子、完全沒這條路徑、或撞到分支
/// （分支要跟「沒這條路徑」分開算——`Flag{expect:false}` 兩者語意不同）。
pub(super) enum PathValue<'a> {
    Leaf(&'a str),
    Missing,
    Branch,
}

pub(super) fn resolve_path<'a>(
    tree: &'a BTreeMap<String, StateNode>,
    path: &str,
) -> PathValue<'a> {
    let segments: Vec<String> = path.split('.').map(str::to_owned).collect();
    match data::node_at(tree, &segments) {
        Some(StateNode::Leaf(value)) => PathValue::Leaf(value.as_str()),
        Some(StateNode::Branch(_)) => PathValue::Branch,
        None => PathValue::Missing,
    }
}

/// 點分路徑取葉子字串（供佔位換值用）；路徑不存在或撞到分支都回 None，換成空字串。
pub(super) fn leaf_at<'a>(tree: &'a BTreeMap<String, StateNode>, path: &str) -> Option<&'a str> {
    match resolve_path(tree, path) {
        PathValue::Leaf(value) => Some(value),
        PathValue::Missing | PathValue::Branch => None,
    }
}

/// 從一段可能帶前後綴的字串抽出第一段數字：先試 `split_pair`（"500/500" 取現值），
/// 抽不出來就逐字掃出第一段允許負號與小數的數字子字串（吃得下「❤️ 60」「体力60」「第 3 天」）。
/// 整段都沒有數字（純文字欄）就回 None，呼叫端跳過不比。
pub(super) fn numeric_value(raw: &str) -> Option<f64> {
    if let Some((current, _max)) = split_pair(raw) {
        return Some(current);
    }
    let chars: Vec<char> = raw.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        let starts_number = chars[index].is_ascii_digit()
            || (chars[index] == '-' && chars.get(index + 1).is_some_and(char::is_ascii_digit));
        if !starts_number {
            index += 1;
            continue;
        }
        let start = index;
        if chars[index] == '-' {
            index += 1;
        }
        while index < chars.len() && chars[index].is_ascii_digit() {
            index += 1;
        }
        if chars.get(index) == Some(&'.') && chars.get(index + 1).is_some_and(char::is_ascii_digit)
        {
            index += 1;
            while index < chars.len() && chars[index].is_ascii_digit() {
                index += 1;
            }
        }
        let text: String = chars[start..index].iter().collect();
        if let Ok(value) = text.parse::<f64>() {
            return Some(value);
        }
    }
    None
}
