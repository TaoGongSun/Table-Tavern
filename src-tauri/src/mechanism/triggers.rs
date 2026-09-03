use crate::data::{Condition, Mechanism, StateNode, TriggerMode};
use std::collections::BTreeMap;

use super::tree::{leaf_at, resolve_path, split_pair, PathValue};

// ---------------------------------------------------------------------
// 觸發表：每回合本地求值（卡片原本用 EJS 腳本做的關係階段／環境氛圍／
// 一次性國家事件，改成資料化條件比對，命中的那段文本才送模型，劇透原文留在本機）
// ---------------------------------------------------------------------

/// 觸發表求值輸出：這輪命中的文本（trigger id → 文本）與要釘死的一次性旗標路徑
/// （呼叫端負責把 flags 寫進樹並記帳，這裡只管求值，不碰樹）。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TriggerOutcome {
    pub hits: BTreeMap<String, String>,
    pub flags: Vec<String>,
}

/// 逐個 Trigger 依序掃 cases，第一個所有 `when` 都成立的就停（空 `when` 一定成立，
/// 當 else 兜底）；沒有任何 case 命中＝這個 trigger 這輪沒有文本。命中文本＝
/// `preamble`（非空時）＋空行＋case 文本，換完 `{{state:路徑}}` 佔位再過一次
/// `{{user}}` 代換。`Once` 命中則把它的 flag 路徑收進輸出，由呼叫端釘進樹。
pub fn evaluate_triggers(
    tree: &BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    user_name: &str,
) -> TriggerOutcome {
    let mut outcome = TriggerOutcome::default();
    for trigger in &mechanism.triggers {
        let Some(case) = trigger.cases.iter().find(|case| {
            case.when
                .iter()
                .all(|condition| condition_holds(tree, condition))
        }) else {
            continue;
        };
        let mut text = case.text.clone();
        if !trigger.preamble.is_empty() {
            text = format!("{}\n\n{text}", trigger.preamble);
        }
        let text = resolve_state_placeholders(&text, tree);
        let text = crate::transport::replace_st_macros(&text, user_name, None);
        outcome.hits.insert(trigger.id.clone(), text);
        if trigger.mode == TriggerMode::Once {
            if let Some(flag) = &trigger.flag {
                outcome.flags.push(flag.clone());
            }
        }
    }
    outcome
}

/// 條件求值：值一律從樹上讀葉子字串；路徑指到分支（不是葉子）一律不成立，
/// 不論條件型別、不論 `default`／`expect` 怎麼設。
fn condition_holds(tree: &BTreeMap<String, StateNode>, condition: &Condition) -> bool {
    match condition {
        Condition::Range {
            path,
            min,
            max,
            min_exclusive,
            max_exclusive,
            default,
        } => {
            let value = match resolve_path(tree, path) {
                PathValue::Leaf(text) => match current_number(text) {
                    Some(value) => value,
                    None => return false,
                },
                PathValue::Missing => match default {
                    Some(default) => *default,
                    None => return false,
                },
                PathValue::Branch => return false,
            };
            if let Some(min) = min {
                if (*min_exclusive && value <= *min) || (!*min_exclusive && value < *min) {
                    return false;
                }
            }
            if let Some(max) = max {
                if (*max_exclusive && value >= *max) || (!*max_exclusive && value > *max) {
                    return false;
                }
            }
            true
        }
        Condition::Contains { path, any } => {
            let text = match resolve_path(tree, path) {
                PathValue::Leaf(text) => text,
                PathValue::Missing => "",
                PathValue::Branch => return false,
            };
            any.iter().any(|needle| text.contains(needle.as_str()))
        }
        Condition::Flag { path, expect } => {
            let actual = match resolve_path(tree, path) {
                PathValue::Leaf(text) => {
                    let text = text.trim().to_ascii_lowercase();
                    text == "true" || text == "1"
                }
                PathValue::Missing => false,
                PathValue::Branch => return false,
            };
            actual == *expect
        }
    }
}

/// `"480/500"` 這種現值/上限對取現值，純數字照原樣 parse。
fn current_number(value: &str) -> Option<f64> {
    split_pair(value)
        .map(|(current, _max)| current)
        .or_else(|| value.trim().parse::<f64>().ok())
}

/// 把命中文本裡的 `{{state:<點分路徑>}}` 換成樹上的現值，路徑不存在就換成空字串。
fn resolve_state_placeholders(text: &str, tree: &BTreeMap<String, StateNode>) -> String {
    const MARK: &str = "{{state:";
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(MARK) {
        result.push_str(&rest[..start]);
        let after = &rest[start + MARK.len()..];
        let Some(end) = after.find("}}") else {
            // 沒有收尾的殘缺標記：原樣保留，不吃掉後面的文字
            result.push_str(&rest[start..]);
            rest = "";
            break;
        };
        result.push_str(leaf_at(tree, &after[..end]).unwrap_or(""));
        rest = &after[end + 2..];
    }
    result.push_str(rest);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data;
    use crate::mechanism::test_support::{
        else_case, once_mechanism, range_case, tree_from,
    };

    // ---- evaluate_triggers：四種條件各自成立／不成立 ----

    #[test]
    fn range_condition_checks_inclusive_exclusive_and_default() {
        let tree = tree_from(&[("World.HP", "50")]);
        let inclusive_bounds = Condition::Range {
            path: "World.HP".to_owned(),
            min: Some(50.0),
            max: Some(50.0),
            min_exclusive: false,
            max_exclusive: false,
            default: None,
        };
        assert!(condition_holds(&tree, &inclusive_bounds));

        let exclusive_min = Condition::Range {
            path: "World.HP".to_owned(),
            min: Some(50.0),
            max: None,
            min_exclusive: true,
            max_exclusive: false,
            default: None,
        };
        assert!(!condition_holds(&tree, &exclusive_min));

        let missing_with_default = Condition::Range {
            path: "World.Missing".to_owned(),
            min: Some(0.0),
            max: None,
            min_exclusive: false,
            max_exclusive: false,
            default: Some(10.0),
        };
        assert!(condition_holds(&tree, &missing_with_default));

        let missing_without_default = Condition::Range {
            path: "World.Missing".to_owned(),
            min: Some(0.0),
            max: None,
            min_exclusive: false,
            max_exclusive: false,
            default: None,
        };
        assert!(!condition_holds(&tree, &missing_without_default));
    }

    /// 計數器門檻＝只給 min 的 Range，跟一般數值區間邏輯逐字相同，不另立型別。
    #[test]
    fn range_condition_as_counter_threshold_only_checks_min() {
        let tree = tree_from(&[("World.Kills", "3")]);
        let at_threshold = Condition::Range {
            path: "World.Kills".to_owned(),
            min: Some(3.0),
            max: None,
            min_exclusive: false,
            max_exclusive: false,
            default: None,
        };
        assert!(condition_holds(&tree, &at_threshold));

        let below_threshold = Condition::Range {
            path: "World.Kills".to_owned(),
            min: Some(4.0),
            max: None,
            min_exclusive: false,
            max_exclusive: false,
            default: None,
        };
        assert!(!condition_holds(&tree, &below_threshold));
    }

    #[test]
    fn range_condition_reads_current_value_out_of_a_pair_field() {
        let tree = tree_from(&[("World.HP", "480/500")]);
        let holds = Condition::Range {
            path: "World.HP".to_owned(),
            min: Some(400.0),
            max: None,
            min_exclusive: false,
            max_exclusive: false,
            default: None,
        };
        assert!(condition_holds(&tree, &holds));
        let fails = Condition::Range {
            path: "World.HP".to_owned(),
            min: Some(490.0),
            max: None,
            min_exclusive: false,
            max_exclusive: false,
            default: None,
        };
        assert!(!condition_holds(&tree, &fails));
    }

    #[test]
    fn contains_condition_matches_any_needle_and_missing_leaf_is_empty_string() {
        let tree = tree_from(&[("World.Location", "北方雪原")]);
        let hits = Condition::Contains {
            path: "World.Location".to_owned(),
            any: vec!["南方".to_owned(), "雪原".to_owned()],
        };
        assert!(condition_holds(&tree, &hits));

        let missing = Condition::Contains {
            path: "World.Missing".to_owned(),
            any: vec!["雪原".to_owned()],
        };
        assert!(!condition_holds(&tree, &missing));
    }

    #[test]
    fn flag_condition_reads_true_variants_and_missing_leaf_counts_as_false() {
        let tree = tree_from(&[("Events.已發生", "TRUE"), ("Events.也算真", "1")]);
        assert!(condition_holds(
            &tree,
            &Condition::Flag {
                path: "Events.已發生".to_owned(),
                expect: true,
            }
        ));
        assert!(condition_holds(
            &tree,
            &Condition::Flag {
                path: "Events.也算真".to_owned(),
                expect: true,
            }
        ));
        // 沒發生過（葉子不存在）視為 false，expect: false 才會成立——一次性事件的初始狀態。
        assert!(condition_holds(
            &tree,
            &Condition::Flag {
                path: "Events.還沒發生".to_owned(),
                expect: false,
            }
        ));
        assert!(!condition_holds(
            &tree,
            &Condition::Flag {
                path: "Events.還沒發生".to_owned(),
                expect: true,
            }
        ));
    }

    /// 路徑指到分支＝一律不成立，跟「路徑不存在」的預設語意分開算：
    /// `Flag{expect:false}` 對著一個真的存在的分支不該被當成「沒發生過」。
    #[test]
    fn condition_pointing_at_a_branch_never_holds() {
        let tree = tree_from(&[("World.City.Name", "晨港")]);
        assert!(!condition_holds(
            &tree,
            &Condition::Flag {
                path: "World.City".to_owned(),
                expect: false,
            }
        ));
        assert!(!condition_holds(
            &tree,
            &Condition::Contains {
                path: "World.City".to_owned(),
                any: vec!["晨".to_owned()],
            }
        ));
        assert!(!condition_holds(
            &tree,
            &Condition::Range {
                path: "World.City".to_owned(),
                min: None,
                max: None,
                min_exclusive: false,
                max_exclusive: false,
                default: Some(0.0),
            }
        ));
    }

    // ---- evaluate_triggers：if/else 鏈語意、佔位換值、一次性事件收乾淨 ----

    /// if/else-if 鏈：前面命中的 case 贏，後面即使也成立也不會被拿到；全不中時才輪到空
    /// `when` 的兜底 case；命中文本會先套 preamble 再過一次佔位換值。
    #[test]
    fn evaluate_triggers_stops_at_the_first_matching_case_and_falls_back_to_else() {
        let mechanism = Mechanism {
            version: 1,
            rules: BTreeMap::new(),
            triggers: vec![data::Trigger {
                id: "侵略".to_owned(),
                title: "環境氛圍".to_owned(),
                mode: TriggerMode::Range,
                cases: vec![
                    range_case(80.0, "淪陷邊緣：現值 {{state:World.Invasion}}"),
                    range_case(50.0, "戰雲密布"),
                    else_case("風平浪靜"),
                ],
                preamble: "隱藏背景".to_owned(),
                scope: Vec::new(),
                flag: None,
            }],
            incremental: true,
            guide: String::new(),
        };

        let high = tree_from(&[("World.Invasion", "90")]);
        let outcome = evaluate_triggers(&high, &mechanism, "阿濤");
        assert_eq!(
            outcome.hits.get("侵略"),
            Some(&"隱藏背景\n\n淪陷邊緣：現值 90".to_owned())
        );

        let mid = tree_from(&[("World.Invasion", "60")]);
        let outcome = evaluate_triggers(&mid, &mechanism, "阿濤");
        assert_eq!(
            outcome.hits.get("侵略"),
            Some(&"隱藏背景\n\n戰雲密布".to_owned())
        );

        let low = tree_from(&[("World.Invasion", "10")]);
        let outcome = evaluate_triggers(&low, &mechanism, "阿濤");
        assert_eq!(
            outcome.hits.get("侵略"),
            Some(&"隱藏背景\n\n風平浪靜".to_owned())
        );
    }

    /// 沒有任何 case 命中（沒有兜底）＝這個 trigger 這輪沒有文本。
    #[test]
    fn evaluate_triggers_produces_no_text_when_no_case_matches_and_there_is_no_fallback() {
        let mechanism = Mechanism {
            version: 1,
            rules: BTreeMap::new(),
            triggers: vec![data::Trigger {
                id: "侵略".to_owned(),
                title: "環境氛圍".to_owned(),
                mode: TriggerMode::Range,
                cases: vec![range_case(80.0, "淪陷邊緣")],
                preamble: String::new(),
                scope: Vec::new(),
                flag: None,
            }],
            incremental: true,
            guide: String::new(),
        };
        let tree = tree_from(&[("World.Invasion", "10")]);
        let outcome = evaluate_triggers(&tree, &mechanism, "阿濤");
        assert!(outcome.hits.is_empty());
    }

    /// {{user}} 巨集跟 {{state:路徑}} 佔位都要在命中文本裡換好。
    #[test]
    fn evaluate_triggers_replaces_state_placeholder_and_user_macro() {
        let mechanism = Mechanism {
            version: 1,
            rules: BTreeMap::new(),
            triggers: vec![data::Trigger {
                id: "招呼".to_owned(),
                title: "招呼".to_owned(),
                mode: TriggerMode::Range,
                cases: vec![data::TriggerCase {
                    when: Vec::new(),
                    text: "{{user}} 現在在 {{state:World.Location}}".to_owned(),
                }],
                preamble: String::new(),
                scope: Vec::new(),
                flag: None,
            }],
            incremental: true,
            guide: String::new(),
        };
        let tree = tree_from(&[("World.Location", "晨港")]);
        let outcome = evaluate_triggers(&tree, &mechanism, "阿濤");
        assert_eq!(
            outcome.hits.get("招呼"),
            Some(&"阿濤 現在在 晨港".to_owned())
        );

        // 路徑不存在就換成空字串，不留下沒收尾的佔位標記。
        let empty_tree = BTreeMap::new();
        let outcome = evaluate_triggers(&empty_tree, &mechanism, "阿濤");
        assert_eq!(outcome.hits.get("招呼"), Some(&"阿濤 現在在 ".to_owned()));
    }

    /// Once 命中：文本有了、旗標收進 flags（由 apply_block 負責釘進樹）。
    #[test]
    fn evaluate_triggers_once_hit_reports_the_flag_to_pin() {
        let mechanism = once_mechanism();
        let tree = tree_from(&[("World.Invasion", "90")]);
        let outcome = evaluate_triggers(&tree, &mechanism, "阿濤");
        assert_eq!(outcome.hits.get("國變"), Some(&"國都淪陷。".to_owned()));
        assert_eq!(outcome.flags, vec!["Events.國變".to_owned()]);
    }
}
