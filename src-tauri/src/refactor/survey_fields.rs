//! 盤點 FIELDS 帶進套用（refactor-outcome-fields）：只作用在退場桌（characters、卡片狀態欄已淘汰、非 MVU）。
//! 樹上沒有的欄位放空值葉子，交給開場補齊補每回合規則；開場白沒附狀態欄的卡才有格子可追蹤。

use super::apply_mechanism::{align_lenient, namespace, BASE_TABLE_KEYS};
use crate::data::{self, FieldKind, StateNode, WorldState};
use std::collections::BTreeMap;

/// 盤點寫的名字對到樹上的路徑：先照點分路徑對齊；對不到就找末段同名（只差字形）的唯一節點
/// （盤點寫 `世界地图`、樹上是 `终端.世界地图`）。多個同名就不猜。
fn locate(name: &str, space: &BTreeMap<String, StateNode>) -> Option<String> {
    let aligned = align_lenient(name, space)?;
    if data::node_at(space, &segments(&aligned)).is_some() {
        return Some(aligned);
    }
    let last = name.rsplit('.').next().unwrap_or(name);
    let mut found = Vec::new();
    walk(space, "", &mut |path, key| {
        if data::glyph_equal(key, last) {
            found.push(path.to_owned());
        }
    });
    (found.len() == 1).then(|| found.remove(0))
}

fn walk(tree: &BTreeMap<String, StateNode>, prefix: &str, visit: &mut dyn FnMut(&str, &str)) {
    for (key, node) in tree {
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        visit(&path, key);
        if let StateNode::Branch(children) = node {
            walk(children, &path, visit);
        }
    }
}

fn segments(path: &str) -> Vec<String> {
    path.split('.').map(str::to_owned).collect()
}

/// FIELDS 裡樹上還沒有的欄位放空值葉子（開場補齊會替它補每回合文字規則）。跳過：固定鍵、鏡像、
/// 樹上已有（含末段同名）、已有數字類規則（空字串報不了增減）、不能當鍵的名字。
pub(super) fn seed_fields(
    state: &mut WorldState,
    fields: &[String],
    mirrors: &BTreeMap<String, String>,
) {
    let space = namespace(&state.state.tree, &state.state.table);
    for name in fields {
        let Some(path) = align_lenient(name, &space) else {
            continue;
        };
        let path_segments = segments(&path);
        let bad = path_segments.iter().any(|segment| {
            segment.is_empty() || segment == "*" || segment.contains(['{', '}', '\n'])
        });
        let numeric = state.mechanism.rules.get(&path).is_some_and(|rule| {
            matches!(
                rule.kind,
                FieldKind::Number | FieldKind::Counter | FieldKind::Pair
            )
        });
        if bad
            || numeric
            || BASE_TABLE_KEYS.contains(&path.as_str())
            || mirrors.contains_key(&path)
            || locate(name, &space).is_some()
        {
            continue;
        }
        insert_empty(&mut state.state.tree, &path_segments);
    }
}

/// `set_tree_value` 把空字串當刪除，空值葉子另外插。中途撞到葉子就放棄（不吃掉既有值）。
fn insert_empty(tree: &mut BTreeMap<String, StateNode>, path: &[String]) {
    let Some((key, rest)) = path.split_first() else {
        return;
    };
    if rest.is_empty() {
        tree.entry(key.clone())
            .or_insert_with(|| StateNode::Leaf(String::new()));
        return;
    }
    let node = tree
        .entry(key.clone())
        .or_insert_with(|| StateNode::Branch(BTreeMap::new()));
    if let StateNode::Branch(children) = node {
        insert_empty(children, rest);
    }
}
