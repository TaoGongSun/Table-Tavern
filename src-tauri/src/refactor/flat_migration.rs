//! 介面接管後的平欄遷移：先貼開場白再重構時，開場狀態區塊的單段鍵落在平欄，介面重建只換樹，平欄那份
//! 會凍住、跟介面欄位兩份並存（接管回覆只收 `<UpdateVariable>`，寫不回平欄）。重建樹之後在本地把
//! time／place／present 以外的平欄鍵逐一對到樹上：
//! - 比對分層、只取唯一、前層命中就停：精確 → 簡繁字形 → 「/」前段（兩個方向）→ 葉子末段。
//! - 對到唯一一片葉子＝已取代：刪平欄那份。前三層命中、本次樹非權威、兩邊都是不帶模板的明確數字（純數字或
//!   `現值/上限`）、平欄值合該欄數字規則時，採平欄的開場數字；其餘一律留產物值。帶模板的（`第 N 天`）不採：
//!   介面產物把模板跟數字寫在同一個字串裡，換數字也還是 delta 不了，殼又直接讀樹值，不能改存數字＋模板。
//! - 衝突（對到分支、同層多個候選、多個平欄鍵收到同一片葉子）：刪平欄、不寫值，只記 log。
//! - 未收編（哪層都對不到）：搬成樹的頂層葉子並補每輪送的文字規則。
//! - 圍欄還在用的桌（沒有接管殼）：圍欄的單段鍵只會轉寫到同名或只差字形的頂層葉子，其餘寫回平欄。所以只遷移
//!   精確／字形命中與未收編；「/」前段、末段與衝突照舊留平欄，免得刪掉後下一輪圍欄又寫回來。
//!
//! 先建整張清單再一次套用，唯一性看整張清單。回傳離開平欄的鍵，呼叫端同步整幕事件快照。

use super::apply_mechanism::{explicit_number, slash_head_match, turn_rule, BASE_TABLE_KEYS};
use crate::data::{self, FieldKind, FieldRule, StateNode, WorldState};
use crate::mechanism;
use std::collections::BTreeMap;

/// 命中的比對層：前三層可採值；圍欄轉寫（`fence_tree_target`）只認得前兩層。
#[derive(Debug, Clone, Copy, PartialEq)]
enum Layer {
    Exact,
    Glyph,
    Slash,
    Tail,
}

#[derive(Debug, Clone, PartialEq)]
enum Fate {
    /// 對到唯一葉子；`layer`＝哪一層命中
    Replaced {
        path: Vec<String>,
        layer: Layer,
    },
    Conflict {
        targets: Vec<String>,
    },
    Unabsorbed,
}

/// 遷移平欄：`tree_authoritative`＝本次重建保留了既有樹的現值（MVU 桌），此時不採平欄數字；
/// `fence_live`＝這桌沒有接管殼、GM 圍欄還會寫平欄。
pub(super) fn migrate_flat_table(
    state: &mut WorldState,
    tree_authoritative: bool,
    fence_live: bool,
) -> Vec<String> {
    let tree = &state.state.tree;
    let mut plan: BTreeMap<String, Fate> = state
        .state
        .table
        .keys()
        .filter(|key| !BASE_TABLE_KEYS.contains(&key.as_str()))
        .map(|key| (key.clone(), classify(tree, key)))
        .collect();

    // 多個平欄鍵收到同一片葉子：分不出哪個才是那一欄，整組改判衝突
    let mut claims: BTreeMap<Vec<String>, usize> = BTreeMap::new();
    for fate in plan.values() {
        if let Fate::Replaced { path, .. } = fate {
            *claims.entry(path.clone()).or_default() += 1;
        }
    }
    for fate in plan.values_mut() {
        if let Fate::Replaced { path, .. } = fate {
            if claims[path] > 1 {
                *fate = Fate::Conflict {
                    targets: vec![path.join(".")],
                };
            }
        }
    }

    // 衝突判定完整張清單之後才過濾：圍欄還在用的桌，衝突（含多來源同一葉）照舊留平欄
    if fence_live {
        plan.retain(|_, fate| match fate {
            Fate::Replaced { layer, .. } => matches!(layer, Layer::Exact | Layer::Glyph),
            Fate::Conflict { .. } => false,
            Fate::Unabsorbed => true,
        });
    }

    let mut moved = Vec::new();
    for (key, fate) in plan {
        let Some(value) = state.state.table.remove(&key) else {
            continue;
        };
        match fate {
            Fate::Replaced { path, layer } => {
                if layer == Layer::Tail {
                    log::warn!(
                        "重構套用：平欄 {key} 只對到同名末段 {}，平欄值「{value}」不寫進樹",
                        path.join(".")
                    );
                } else if !tree_authoritative {
                    adopt_number(state, &path, &value);
                }
            }
            Fate::Conflict { targets } => {
                log::warn!(
                    "重構套用：平欄 {key} 對到 {}，分不出是哪一欄，平欄值「{value}」不寫進樹",
                    targets.join("、")
                );
            }
            Fate::Unabsorbed => {
                let segments = vec![key.clone()];
                data::set_tree_value(&mut state.state.tree, &segments, &value);
                if !mechanism::has_rule(&state.mechanism, &segments) {
                    state
                        .mechanism
                        .rules
                        .insert(key.clone(), turn_rule(FieldKind::Text));
                }
            }
        }
        moved.push(key);
    }
    moved
}

fn classify(tree: &BTreeMap<String, StateNode>, key: &str) -> Fate {
    // 1. 精確
    if let Some(node) = tree.get(key) {
        return top_level(key, node, Layer::Exact);
    }
    // 2. 簡繁字形（canonical_path 對不到會原樣退回，要確認節點真的在）
    match mechanism::canonical_path(tree, &[key.to_owned()], true) {
        Ok(aligned) => {
            if let [found] = aligned.as_slice() {
                if let Some(node) = tree.get(found) {
                    return top_level(found, node, Layer::Glyph);
                }
            }
        }
        Err(targets) => return Fate::Conflict { targets },
    }
    // 3. 「/」前段：平欄鍵含「/」對樹頂層它的前段，或樹頂層含「/」的鍵前段對平欄鍵
    let mut heads: Vec<String> = match slash_head_match(tree, key) {
        Ok(found) => found.into_iter().collect(),
        Err(many) => many,
    };
    if let Some((head, _)) = key.split_once('/') {
        heads.extend(
            tree.keys()
                .filter(|name| *name == head || data::glyph_equal(name, head))
                .cloned(),
        );
    }
    heads.sort();
    heads.dedup();
    match heads.as_slice() {
        [] => {}
        [found] => return top_level(found, &tree[found], Layer::Slash),
        many => {
            return Fate::Conflict {
                targets: many.to_vec(),
            }
        }
    }
    // 4. 葉子末段（只算已取代、不採值：末段同名不代表是同一欄的值）
    let mut leaves = Vec::new();
    collect_leaf_tails(tree, &mut Vec::new(), key, &mut leaves);
    match leaves.len() {
        0 => Fate::Unabsorbed,
        1 => Fate::Replaced {
            path: leaves.remove(0),
            layer: Layer::Tail,
        },
        _ => Fate::Conflict {
            targets: leaves.iter().map(|path| path.join(".")).collect(),
        },
    }
}

fn top_level(key: &str, node: &StateNode, layer: Layer) -> Fate {
    match node {
        StateNode::Leaf(_) => Fate::Replaced {
            path: vec![key.to_owned()],
            layer,
        },
        StateNode::Branch(_) => Fate::Conflict {
            targets: vec![key.to_owned()],
        },
    }
}

/// 深度 ≥2 的葉子裡末段跟平欄鍵同名或只差字形的（頂層葉子前幾層已經比過）。
fn collect_leaf_tails(
    children: &BTreeMap<String, StateNode>,
    prefix: &mut Vec<String>,
    key: &str,
    out: &mut Vec<Vec<String>>,
) {
    for (name, node) in children {
        prefix.push(name.clone());
        match node {
            StateNode::Leaf(_) => {
                if prefix.len() >= 2 && (name == key || data::glyph_equal(name, key)) {
                    out.push(prefix.clone());
                }
            }
            StateNode::Branch(next) => collect_leaf_tails(next, prefix, key, out),
        }
        prefix.pop();
    }
}

/// 採平欄的開場數字：兩邊都是不帶模板的明確數字、平欄值合該欄的數字規則才採。
fn adopt_number(state: &mut WorldState, path: &[String], flat: &str) {
    let Some(StateNode::Leaf(product)) = data::node_at(&state.state.tree, path) else {
        return;
    };
    let (Some((flat_number, flat_format)), Some((_, product_format))) =
        (explicit_number(flat), explicit_number(product))
    else {
        return;
    };
    if flat_format.is_some() || product_format.is_some() {
        return;
    }
    let rule = mechanism::rule_for_path(&state.mechanism, path, Some(product));
    if !number_fits(&rule, &flat_number) {
        return;
    }
    data::set_tree_value(&mut state.state.tree, path, &flat_number);
}

fn number_fits(rule: &FieldRule, value: &str) -> bool {
    let in_range = |number: f64| {
        rule.min.is_none_or(|min| number >= min) && rule.max.is_none_or(|max| number <= max)
    };
    match rule.kind {
        FieldKind::Number => value
            .parse::<f64>()
            .is_ok_and(|n| n.is_finite() && in_range(n)),
        FieldKind::Counter => value
            .parse::<f64>()
            .is_ok_and(|n| n.is_finite() && n.fract() == 0.0 && in_range(n)),
        FieldKind::Pair => value.split_once('/').is_some_and(|(current, max)| {
            match (current.trim().parse::<f64>(), max.trim().parse::<f64>()) {
                (Ok(current), Ok(max)) => {
                    current.is_finite()
                        && max.is_finite()
                        && current >= rule.min.unwrap_or(0.0)
                        && current <= max
                }
                _ => false,
            }
        }),
        _ => false,
    }
}
