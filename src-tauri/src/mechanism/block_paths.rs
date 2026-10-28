//! 狀態區塊路徑對齊：模型換一種字形（簡繁）寫既有欄位時，寫回原鍵，不長第二份。
//! 每個欄位與容器只解析一次（精確優先 → 字形唯一沿用原鍵 → 多候選拒收 → 都對不到照原字新增），
//! 之後的舊值比對、清單替換、寫入、跳動記號都用解析後的路徑。只管模型狀態區塊這個入口，
//! 玩家手動編輯（`set_table_state`、`set_state_path`）照原字寫。

use crate::data::{self, StateNode};
use crate::transport::StateBlock;
use std::collections::BTreeMap;

use super::tree::canonical_path;
use super::types::{Record, RecordKind};

/// 單一欄位解析後要寫哪裡。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Target {
    /// 單段鍵寫平欄
    Table,
    /// 增量桌的單段鍵對到樹上頂層葉子：轉成一筆 replace，跟更新區塊走同一套規則
    TreeReplace,
    /// 多段路徑直接寫樹
    Tree,
}

/// 對齊後的區塊：欄位、清單、具名容器路徑都已換成實際目標；拒收的欄位與容器（連同底下的欄位）已拿掉。
pub(super) struct Aligned {
    pub block: StateBlock,
    /// 與 `block.fields` 一一對應
    pub targets: Vec<Target>,
    pub rejected: Vec<Record>,
}

/// 同層鍵比對：精確同名 → 看 `accept` 收不收（不收就當沒有，不再折字形）；否則字形唯一命中且 `accept`
/// 收的鍵 → Some；多個 → Err（歧義）；都沒有 → None。
pub(super) fn match_key<V>(
    map: &BTreeMap<String, V>,
    key: &str,
    accept: impl Fn(&V) -> bool,
) -> Result<Option<String>, Vec<String>> {
    if let Some(value) = map.get(key) {
        return Ok(accept(value).then(|| key.to_owned()));
    }
    let hits: Vec<String> = map
        .iter()
        .filter(|(name, value)| accept(value) && data::glyph_equal(name, key))
        .map(|(name, _)| name.clone())
        .collect();
    match hits.len() {
        0 => Ok(None),
        1 => Ok(hits.into_iter().next()),
        _ => Err(hits),
    }
}

/// 狀態圍欄的單段欄位該不該進樹：增量桌裡 time／place／present 以外、樹上有同名頂層葉子 → Some(樹上的鍵)
/// （重構把平欄搬進樹後，圍欄才不會把平欄那份寫回來變成兩份）；字形多候選 → Err；其餘 → None（寫平欄）。
fn fence_tree_target(world: &data::WorldState, key: &str) -> Result<Option<String>, Vec<String>> {
    const FENCE_TABLE_KEYS: [&str; 3] = ["time", "place", "present"];
    if !world.mechanism.incremental || FENCE_TABLE_KEYS.contains(&key) {
        return Ok(None);
    }
    match_key(&world.state.tree, key, |node| {
        matches!(node, StateNode::Leaf(_))
    })
}

/// 多段路徑的實際目標。落在區塊自己的清單底下（不論隔幾層）時，先對齊最外層那支清單，底下全程照原字
/// （整支會重寫，新項目不對舊項目，中間的具名容器也不查舊樹）；只在具名容器底下時，先對齊最近的容器，
/// 剩下的段對容器裡既有的鍵。
fn aligned_path(
    tree: &BTreeMap<String, StateNode>,
    containers: &[(Vec<String>, bool)],
    path: &[String],
) -> Result<Vec<String>, Vec<String>> {
    let ancestors = || {
        containers
            .iter()
            .filter(|(container, _)| container.len() < path.len() && path.starts_with(container))
    };
    if let Some((list, _)) = ancestors()
        .filter(|(_, is_list)| *is_list)
        .min_by_key(|(container, _)| container.len())
    {
        let mut out = aligned_path(tree, containers, list)?;
        out.extend_from_slice(&path[list.len()..]);
        return Ok(out);
    }
    let Some((container, _)) = ancestors().max_by_key(|(container, _)| container.len()) else {
        return canonical_path(tree, path, true);
    };
    let mut out = aligned_path(tree, containers, container)?;
    let rest = &path[container.len()..];
    match data::node_at(tree, &out) {
        Some(StateNode::Branch(children)) => out.extend(canonical_path(children, rest, true)?),
        _ => out.extend_from_slice(rest),
    }
    Ok(out)
}

fn ambiguous(path: &[String], keys: &[String]) -> Record {
    let shown = path.join(".");
    Record::new(
        RecordKind::Rejected,
        shown.clone(),
        format!(
            "{shown} 對到好幾個只差字形的欄位（{}），請照狀態裡的原字寫。",
            keys.join("、")
        ),
    )
}

/// 在任何寫入之前把整個區塊的路徑對齊；歧義的容器連同底下的欄位整支拿掉，不會先刪後拒。
pub(super) fn align(world: &data::WorldState, block: &StateBlock) -> Aligned {
    let tree = &world.state.tree;
    let mut containers: Vec<(Vec<String>, bool)> = block
        .lists
        .iter()
        .map(|path| (path.clone(), true))
        .chain(block.maps.iter().map(|path| (path.clone(), false)))
        .collect();
    // 祖先先判：歧義的容器先記下，底下的子容器與欄位整支跳過
    containers.sort_by_key(|(path, _)| path.len());
    let mut rejected = Vec::new();
    let mut dropped: Vec<&Vec<String>> = Vec::new();
    let mut resolved_containers: Vec<(bool, Vec<String>)> = Vec::new();
    for (path, is_list) in &containers {
        if dropped.iter().any(|bad| path.starts_with(bad)) {
            continue;
        }
        match aligned_path(tree, &containers, path) {
            Ok(target) => resolved_containers.push((*is_list, target)),
            Err(keys) => {
                rejected.push(ambiguous(path, &keys));
                dropped.push(path);
            }
        }
    }
    let mut out = block.clone();
    out.lists.clear();
    out.maps.clear();
    for (is_list, path) in resolved_containers {
        if is_list {
            out.lists.push(path);
        } else {
            out.maps.push(path);
        }
    }
    out.fields.clear();
    let mut targets = Vec::new();
    for (path, value) in &block.fields {
        if dropped.iter().any(|bad| path.starts_with(bad)) {
            continue;
        }
        let resolved = if let [key] = path.as_slice() {
            match fence_tree_target(world, key) {
                Ok(Some(tree_key)) => Ok((vec![tree_key], Target::TreeReplace)),
                Err(keys) => Err(keys),
                Ok(None) => match_key(&world.state.table, key, |_| true)
                    .map(|hit| (vec![hit.unwrap_or_else(|| key.clone())], Target::Table)),
            }
        } else {
            aligned_path(tree, &containers, path).map(|target| (target, Target::Tree))
        };
        match resolved {
            Ok((target_path, target)) => {
                out.fields.push((target_path, value.clone()));
                targets.push(target);
            }
            Err(keys) => rejected.push(ambiguous(path, &keys)),
        }
    }
    Aligned {
        block: out,
        targets,
        rejected,
    }
}
