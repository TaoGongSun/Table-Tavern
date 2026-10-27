//! 物件值的巢狀鍵對回樹上的寫法（GM 用 replace／insert 寫整個物件、鍵寫成另一種字形時）：在寫進樹與帶型別視圖
//! 之前改寫值本身，兩邊吃同一份。不改樹；同批每筆依當下的樹解析，前一筆寫成功後一筆看得到。
//!
//! 每一層沿配對到的舊節點往下：鍵精確相符照原字；否則該節點同層只差字形且唯一就改成它；該節點對不上時，拿同層
//! 兄弟物件的鍵聯集當候補，一樣精確優先、唯一才改；都對不上是合法新鍵，照收。底線開頭的鍵不參與。
//! 寫入物件裡折完撞成同一鍵、或新引入只差字形的雙鍵，整筆拒收；舊節點本來就有的雙鍵照原字精確回寫放行。
//! 寫入鍵對不上任何原鍵、又同時像兩個以上，整筆拒收（照收會長出第三種寫法、之後永遠折不回去）。
//! 陣列不往下走，路徑進到陣列裡（中途是葉子）也不插手（沒重構 MVU 的陣列在樹上是一片葉子）。
use super::note::shown_pointer;
use crate::data::{self, StateNode};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

type Children = BTreeMap<String, StateNode>;

/// 一層的參考：配對到的舊節點的子鍵，與同層兄弟物件的子鍵（候補）。
struct Level<'a> {
    own: Option<&'a Children>,
    siblings: Vec<&'a Children>,
}

/// `path` 是對好的寫入路徑、`value` 是物件值。回傳鍵改好的值；拒收回傳給模型的回饋句。
pub(super) fn fold_object_keys(
    tree: &Children,
    path: &[String],
    value: &Map<String, Value>,
) -> Result<Map<String, Value>, String> {
    let (last, parent_path) = path.split_last().ok_or_else(String::new)?;
    // 路徑中途是葉子（沒重構 MVU 的陣列 `/背包/0`）：陣列另案，照原行為不插手
    if (1..path.len())
        .any(|end| matches!(data::node_at(tree, &path[..end]), Some(StateNode::Leaf(_))))
    {
        return Ok(value.clone());
    }
    let parent = if parent_path.is_empty() {
        Some(tree)
    } else {
        branch(data::node_at(tree, parent_path))
    };
    let level = Level {
        own: branch(data::node_at(tree, path)),
        siblings: parent
            .map(|children| other_branches(children, last))
            .unwrap_or_default(),
    };
    fold_level(&level, value, &mut path.to_vec())
}

fn branch(node: Option<&StateNode>) -> Option<&Children> {
    match node {
        Some(StateNode::Branch(children)) => Some(children),
        _ => None,
    }
}

fn other_branches<'a>(children: &'a Children, except: &str) -> Vec<&'a Children> {
    children
        .iter()
        .filter(|(key, _)| key.as_str() != except && !key.starts_with('_'))
        .filter_map(|(_, node)| branch(Some(node)))
        .collect()
}

fn fold_level(
    level: &Level,
    map: &Map<String, Value>,
    at: &mut Vec<String>,
) -> Result<Map<String, Value>, String> {
    let mut targets = Vec::with_capacity(map.len());
    for key in map.keys() {
        let target = if key.starts_with('_') {
            key.clone()
        } else {
            resolve(level, key).map_err(|keys| {
                format!(
                    "{} 的 {key} 對到好幾個只差字形的欄位（{}），請照狀態裡的原字寫。",
                    shown_pointer(at),
                    keys.join("、")
                )
            })?
        };
        targets.push(target);
    }
    check_twins(level, map, &targets, at)?;
    let mut out = Map::with_capacity(map.len());
    for ((key, value), target) in map.iter().zip(targets) {
        let value = match value {
            Value::Object(inner) if !key.starts_with('_') => {
                at.push(target.clone());
                let folded = fold_level(&child_level(level, &target), inner, at);
                at.pop();
                Value::Object(folded?)
            }
            other => other.clone(),
        };
        out.insert(target, value);
    }
    Ok(out)
}

/// 下一層的參考：配對後的子節點存在就沿它走，候補是舊節點底下其他分支。子節點不存在（新項目、既有項目新增的
/// 分支）時候補是舊節點底下其他分支（新人物對其他人物），加上這一層兄弟物件底下同名的分支（贺铮新增「關係」
/// 對铁三的「关系」）。
fn child_level<'a>(level: &Level<'a>, target: &str) -> Level<'a> {
    let own_child = level.own.and_then(|own| branch(own.get(target)));
    let peers = level
        .own
        .map(|own| other_branches(own, target))
        .unwrap_or_default();
    if own_child.is_some() {
        return Level {
            own: own_child,
            siblings: peers,
        };
    }
    let parallel = level
        .siblings
        .iter()
        .filter_map(|sibling| branch(sibling.get(target)));
    Level {
        own: None,
        siblings: peers.into_iter().chain(parallel).collect(),
    }
}

/// 精確優先、唯一折疊；歧義回 Err（候選鍵）。都對不上照原字（新鍵）。
fn resolve(level: &Level, key: &str) -> Result<String, Vec<String>> {
    if let Some(own) = level.own {
        if own.contains_key(key) {
            return Ok(key.to_owned());
        }
        if let Some(found) = unique_twin(own.keys(), key)? {
            return Ok(found);
        }
    }
    let union: BTreeSet<&String> = level
        .siblings
        .iter()
        .flat_map(|children| children.keys())
        .collect();
    if union.contains(&key.to_owned()) {
        return Ok(key.to_owned());
    }
    Ok(unique_twin(union.into_iter(), key)?.unwrap_or_else(|| key.to_owned()))
}

fn unique_twin<'a>(
    keys: impl Iterator<Item = &'a String>,
    key: &str,
) -> Result<Option<String>, Vec<String>> {
    let twins: BTreeSet<&String> = keys
        .filter(|candidate| !candidate.starts_with('_') && data::glyph_equal(candidate, key))
        .collect();
    match twins.len() {
        0 => Ok(None),
        1 => Ok(twins.into_iter().next().cloned()),
        _ => Err(twins.into_iter().cloned().collect()),
    }
}

/// 寫入物件裡兩個鍵落到同一鍵、或落到只差字形的兩個鍵（除非兩個都是舊節點本來就有的鍵）＝同一欄寫了兩次。
fn check_twins(
    level: &Level,
    map: &Map<String, Value>,
    targets: &[String],
    at: &[String],
) -> Result<(), String> {
    let written: Vec<&String> = map.keys().collect();
    for i in 0..targets.len() {
        for j in i + 1..targets.len() {
            if written[i].starts_with('_') || written[j].starts_with('_') {
                continue;
            }
            let (a, b) = (&targets[i], &targets[j]);
            let preexisting = a != b
                && level
                    .own
                    .is_some_and(|own| own.contains_key(a) && own.contains_key(b));
            if (a == b || data::glyph_equal(a, b)) && !preexisting {
                return Err(format!(
                    "{} 的 {} 和 {} 只差字形，是同一個欄位寫了兩次；同一個欄位只寫一次。",
                    shown_pointer(at),
                    written[i],
                    written[j]
                ));
            }
        }
    }
    Ok(())
}
