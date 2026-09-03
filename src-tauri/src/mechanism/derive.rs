use crate::data::{self, FieldKind, Mechanism, StateNode};
use std::collections::BTreeMap;

use super::rules::rule_for;
use super::tree::{format_num, leaf_at, numeric_value};
use super::types::{Record, RecordKind};

// ---------------------------------------------------------------------
// 骰值欄每回合本地重擲
// ---------------------------------------------------------------------

/// 骰值欄（kind == Roll）每回合本地重擲一次真隨機，寫回樹裡。
pub fn reroll(tree: &mut BTreeMap<String, StateNode>, mechanism: &Mechanism) {
    let mut path = Vec::new();
    reroll_branch(tree, mechanism, &mut path);
}

fn reroll_branch(
    branch: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    path: &mut Vec<String>,
) {
    let keys: Vec<String> = branch.keys().cloned().collect();
    for key in keys {
        path.push(key.clone());
        let is_leaf = matches!(branch.get(&key), Some(StateNode::Leaf(_)));
        if is_leaf {
            let rule = rule_for(mechanism, path, None);
            if rule.kind == FieldKind::Roll {
                let min = rule.min.unwrap_or(1.0);
                let max = rule.max.unwrap_or(100.0);
                branch.insert(
                    key.clone(),
                    StateNode::Leaf(random_int_in_range(min, max).to_string()),
                );
            }
        } else if let Some(StateNode::Branch(children)) = branch.get_mut(&key) {
            reroll_branch(children, mechanism, path);
        }
        path.pop();
    }
}

fn random_int_in_range(min: f64, max: f64) -> i64 {
    let low = min.round() as i64;
    let high = max.round() as i64;
    if high <= low {
        return low;
    }
    let span = (high - low + 1) as u128;
    let random_bits = ulid::Ulid::generate().random();
    low + (random_bits % span) as i64
}

// ---------------------------------------------------------------------
// 衍生值欄（kind == Derived）每回合本地重算：公式以整棵樹當取值來源，
// 算出來的數字寫回它自己那個葉子。只重算樹上已經有這個葉子的欄位——
// 跟 Roll 一樣，路徑要先存在（初始樹或手動建立），這裡不會平白生出新分支。
// ---------------------------------------------------------------------

/// 先只讀一輪收集「路徑＋公式」，算完再一次寫回去——同一棵樹不能又借給
/// 取值用的 closure、又借去改，兩階段分開才過得了借用檢查。
pub fn recompute_derived(tree: &mut BTreeMap<String, StateNode>, mechanism: &Mechanism) -> Vec<Record> {
    let mut targets = Vec::new();
    collect_derived_targets(tree, mechanism, &mut Vec::new(), &mut targets);
    if targets.is_empty() {
        return Vec::new();
    }
    let mut records = Vec::new();
    let mut writes = Vec::new();
    {
        let lookup = tree_lookup(tree);
        for (path, formula) in &targets {
            let path_str = path.join(".");
            match crate::evaluator::eval(formula, &lookup) {
                Ok(value) => writes.push((path.clone(), format_num(value))),
                Err(error) => records.push(Record::new(
                    RecordKind::Error,
                    path_str.clone(),
                    format!("{path_str} 的衍生公式算不出來：{error}"),
                )),
            }
        }
    }
    for (path, value) in writes {
        data::set_tree_value(tree, &path, &value);
    }
    records
}

fn collect_derived_targets(
    branch: &BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    path: &mut Vec<String>,
    targets: &mut Vec<(Vec<String>, String)>,
) {
    for (key, node) in branch {
        path.push(key.clone());
        match node {
            StateNode::Leaf(_) => {
                let rule = rule_for(mechanism, path, None);
                if rule.kind == FieldKind::Derived {
                    if let Some(formula) = rule.formula {
                        targets.push((path.clone(), formula));
                    }
                }
            }
            StateNode::Branch(children) => {
                collect_derived_targets(children, mechanism, path, targets)
            }
        }
        path.pop();
    }
}

/// 公式取值來源：點分路徑在樹上查葉子，數字抽取沿用 `numeric_value`——
/// 現值/上限對取現值、帶前後綴的文字抽第一段數字，跟全量桌跳動比對同一套規則。
fn tree_lookup(tree: &BTreeMap<String, StateNode>) -> impl Fn(&str) -> Option<f64> + '_ {
    move |path: &str| leaf_at(tree, path).and_then(numeric_value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{FieldRule, InjectLevel};
    use crate::mechanism::apply_block;
    use crate::mechanism::test_support::{derived_rule, mechanism_with, tree_from, world_with};

    // ---- reroll ----

    #[test]
    fn reroll_always_lands_in_range_and_changes_the_value() {
        let mut tree = tree_from(&[("World.Roll100", "1")]);
        let mechanism = mechanism_with(&[(
            "World.Roll100",
            FieldRule {
                kind: FieldKind::Roll,
                min: Some(1.0),
                max: Some(6.0),
                update: crate::data::UpdateMode::Local,
                inject: InjectLevel::Turn,
                branch: None,
                formula: None,
            },
        )]);
        let mut saw_change = false;
        for _ in 0..30 {
            reroll(&mut tree, &mechanism);
            let StateNode::Branch(world) = tree.get("World").unwrap() else {
                panic!("World 應該是分支");
            };
            let StateNode::Leaf(value) = world.get("Roll100").unwrap() else {
                panic!("Roll100 應該是葉子");
            };
            let parsed: i64 = value.parse().expect("骰值應該是整數字串");
            assert!((1..=6).contains(&parsed));
            if value != "1" {
                saw_change = true;
            }
        }
        assert!(saw_change, "30 次重擲全部落在同一個值，機率上不合理");
    }

    // ---- 衍生值（derived）----

    #[test]
    fn recompute_derived_evaluates_the_formula_against_the_tree() {
        let mut tree = tree_from(&[("HP", "10"), ("Half", "0")]);
        let mechanism = mechanism_with(&[("Half", derived_rule("floor(HP/2)+1"))]);
        let records = recompute_derived(&mut tree, &mechanism);
        assert!(records.is_empty());
        assert_eq!(tree.get("Half"), Some(&StateNode::Leaf("6".to_owned())));
    }

    #[test]
    fn recompute_derived_leaves_a_missing_leaf_alone() {
        // 跟 Roll 一樣的規矩：路徑不存在就不平白生出新分支。
        let mut tree = tree_from(&[("HP", "10")]);
        let mechanism = mechanism_with(&[("Half", derived_rule("HP*2"))]);
        let records = recompute_derived(&mut tree, &mechanism);
        assert!(records.is_empty());
        assert!(tree.get("Half").is_none());
    }

    #[test]
    fn recompute_derived_records_a_formula_error_and_keeps_the_old_value() {
        let mut tree = tree_from(&[("Half", "0")]); // 沒有 HP，公式取值會失敗
        let mechanism = mechanism_with(&[("Half", derived_rule("HP/2"))]);
        let records = recompute_derived(&mut tree, &mechanism);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].kind, RecordKind::Error);
        assert_eq!(tree.get("Half"), Some(&StateNode::Leaf("0".to_owned())));
    }

    #[test]
    fn apply_block_recomputes_derived_fields_end_to_end() {
        let mechanism = mechanism_with(&[("Half", derived_rule("floor(HP/2)+1"))]);
        let mut world = world_with(&[("HP", "10"), ("Half", "0")], mechanism);
        let block = crate::transport::StateBlock {
            fields: Vec::new(),
            updates: Vec::new(),
            display: String::new(),
        };

        apply_block(&mut world, &block, "阿濤");

        assert_eq!(
            world.state.tree.get("Half"),
            Some(&StateNode::Leaf("6".to_owned()))
        );
    }
}
