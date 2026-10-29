//! 盤點欄位帶進套用（refactor-outcome-fields）：只作用在退場桌（characters、卡片狀態欄已淘汰、非 MVU）。
//! - UNTRACKED：地圖或導航清單、記憶或摘要〔作者裁決 2026-10-28〕，整個節點連同規則、前綴、鏡像移除，改由正文描述。
//! - FIELDS：樹上沒有的欄位放空值葉子，交給開場補齊補每回合規則；開場白沒附狀態欄的卡才有格子可追蹤。

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

/// `key` 是 `path` 本身或其下層（比較前先折疊字形）。
fn under(key: &str, path: &str) -> bool {
    let key = data::fold_glyphs(key);
    let path = data::fold_glyphs(path);
    key == path || key.starts_with(&format!("{path}."))
}

fn remove_node(tree: &mut BTreeMap<String, StateNode>, path: &[String]) {
    match path {
        [] => {}
        [key] => {
            tree.remove(key);
        }
        [key, rest @ ..] => {
            if let Some(StateNode::Branch(children)) = tree.get_mut(key) {
                remove_node(children, rest);
            }
        }
    }
}

/// 移除不收的格子。回傳從平欄拿掉的鍵（呼叫端同步把事件快照的平欄也拿掉）。
pub(super) fn drop_untracked(
    state: &mut WorldState,
    untracked: &[String],
    mirrors: &mut BTreeMap<String, String>,
) -> Vec<String> {
    let space = namespace(&state.state.tree, &state.state.table);
    let mut moved = Vec::new();
    for name in untracked {
        // 樹與平欄上沒有這格（開場白沒附狀態欄）也要清掉本輪機制條目抽出的規則：照明確路徑清
        let Some(path) = locate(name, &space).or_else(|| align_lenient(name, &space)) else {
            continue;
        };
        log::warn!("重構套用：{path} 列為不收格子，移除、改由正文描述");
        if state.state.table.remove(&path).is_some() {
            moved.push(path.clone());
        }
        remove_node(&mut state.state.tree, &segments(&path));
        let mechanism = &mut state.mechanism;
        mechanism.rules.retain(|key, _| !under(key, &path));
        mechanism
            .display_formats
            .retain(|key, _| !under(key, &path));
        mechanism
            .display_prefixes
            .retain(|key, _| !under(key, &path));
        mechanism.collections.retain(|key, _| !under(key, &path));
        mirrors.retain(|key, _| !under(key, &path));
    }
    moved
}

/// FIELDS 裡樹上還沒有的欄位放空值葉子（開場補齊會替它補每回合文字規則）。跳過：固定鍵、鏡像、
/// 不收名單、樹上已有（含末段同名）、已有數字類規則（空字串報不了增減）、不能當鍵的名字。
pub(super) fn seed_fields(
    state: &mut WorldState,
    fields: &[String],
    untracked: &[String],
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
        let last = path_segments.last().map(String::as_str).unwrap_or_default();
        if bad
            || numeric
            || BASE_TABLE_KEYS.contains(&path.as_str())
            || mirrors.contains_key(&path)
            || untracked
                .iter()
                .any(|item| data::glyph_equal(item.rsplit('.').next().unwrap_or(item), last))
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
