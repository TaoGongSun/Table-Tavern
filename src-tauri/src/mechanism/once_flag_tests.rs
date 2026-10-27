//! refactor-trigger-overlap：once 觸發求值時先讀旗標——旗標已真（或路徑寫不進樹）整條跳過，
//! 不靠 `when` 裡有沒有 `Flag{expect:false}`（重構 AI 產的 once 多半沒帶）。

use super::test_support::{else_case, mechanism_with, once_mechanism, rule, tree_from, world_with};
use super::{apply_block, evaluate_triggers};
use crate::data::{
    self, Condition, FieldKind, Mechanism, StateNode, Trigger, TriggerCase, TriggerMode,
};
use crate::transport::StateBlock;

const DAYS: &str = "天數";

fn once_at(day: f64, id: &str, flag: &str) -> Trigger {
    Trigger {
        id: id.to_owned(),
        title: id.to_owned(),
        mode: TriggerMode::Once,
        cases: vec![TriggerCase {
            when: vec![Condition::Range {
                path: DAYS.to_owned(),
                min: Some(day),
                max: None,
                min_exclusive: false,
                max_exclusive: false,
                default: None,
            }],
            text: format!("{id}登場"),
        }],
        preamble: String::new(),
        scope: Vec::new(),
        flag: Some(flag.to_owned()),
    }
}

fn stage() -> Trigger {
    Trigger {
        id: "階段".to_owned(),
        title: "階段".to_owned(),
        mode: TriggerMode::Range,
        cases: vec![
            TriggerCase {
                when: vec![Condition::Range {
                    path: DAYS.to_owned(),
                    min: Some(7.0),
                    max: None,
                    min_exclusive: false,
                    max_exclusive: false,
                    default: None,
                }],
                text: "後期".to_owned(),
            },
            else_case("初期"),
        ],
        preamble: String::new(),
        scope: Vec::new(),
        flag: None,
    }
}

fn fortress(day: &str, extra: &[(&str, &str)], triggers: Vec<Trigger>) -> data::WorldState {
    let mut mechanism: Mechanism =
        mechanism_with(&[(DAYS, rule(FieldKind::Counter, Some(1.0), None))]);
    mechanism.incremental = true;
    mechanism.triggers = triggers;
    let mut pairs = vec![(DAYS, day)];
    pairs.extend_from_slice(extra);
    world_with(&pairs, mechanism)
}

/// 推進天數一回合，回傳 (這回合命中的 trigger id, 「一次性事件已觸發」記錄的旗標路徑)。
fn turn(world: &mut data::WorldState, delta: i64) -> (Vec<String>, Vec<String>) {
    let block = StateBlock {
        updates: vec![format!(
            r#"<JSONPatch>[{{"op":"delta","path":"/{DAYS}","value":{delta}}}]</JSONPatch>"#
        )],
        ..Default::default()
    };
    let outcome = apply_block(world, &block, "玩家");
    let pinned = outcome
        .records
        .iter()
        .filter(|record| record.detail.starts_with("一次性事件已觸發"))
        .map(|record| record.path.clone())
        .collect();
    (world.state.triggers.keys().cloned().collect(), pinned)
}

fn leaf(world: &data::WorldState, path: &str) -> Option<StateNode> {
    let segments: Vec<String> = path.split('.').map(str::to_owned).collect();
    data::node_at(&world.state.tree, &segments).cloned()
}

/// 沒帶 Flag 條件的 once：跨門檻那回合命中並釘旗標，之後不再命中、不再記；range 階段照常每回合印。
#[test]
fn once_without_flag_condition_hits_only_on_the_crossing_turn() {
    let mut world = fortress("6", &[], vec![stage(), once_at(7.0, "登場", "事件.登場")]);
    assert_eq!(turn(&mut world, 0), (vec!["階段".to_owned()], vec![]));
    assert_eq!(
        turn(&mut world, 1),
        (
            vec!["登場".to_owned(), "階段".to_owned()],
            vec!["事件.登場".to_owned()]
        )
    );
    assert_eq!(
        leaf(&world, "事件.登場"),
        Some(StateNode::Leaf("true".to_owned()))
    );
    for _ in 0..3 {
        assert_eq!(turn(&mut world, 1), (vec!["階段".to_owned()], vec![]));
    }
    assert_eq!(
        world.state.triggers.get("階段").map(String::as_str),
        Some("後期")
    );
}

/// 旗標真假沿用 Flag 條件那套：去空白、不分大小寫，true／1 為真；其他值與缺路徑＝還沒演。
#[test]
fn once_flag_truthiness_matches_the_flag_condition() {
    let mut mechanism = mechanism_with(&[]);
    mechanism.triggers = vec![once_at(7.0, "登場", "事件.登場")];
    for (value, played) in [
        (" TRUE ", true),
        ("True", true),
        ("1", true),
        ("false", false),
        ("0", false),
        ("否", false),
    ] {
        let tree = tree_from(&[(DAYS, "8"), ("事件.登場", value)]);
        let hits = evaluate_triggers(&tree, &mechanism, "玩家").hits;
        assert_eq!(hits.contains_key("登場"), !played, "{value:?}");
    }
    let missing = tree_from(&[(DAYS, "8")]);
    assert!(evaluate_triggers(&missing, &mechanism, "玩家")
        .hits
        .contains_key("登場"));
}

/// 旗標路徑指到分支、或父層是葉子：釘不進去，當作已演跳過——不蓋掉分支、不改葉子、不記已觸發，
/// 也不會每回合重演。
#[test]
fn once_flag_path_that_cannot_be_pinned_is_skipped_without_touching_the_tree() {
    for (extra, flag, guarded) in [
        (("事件.子項", "保留"), "事件", "事件.子項"),
        (("事件", "一段文字"), "事件.登場", "事件"),
    ] {
        let mut world = fortress("6", &[extra], vec![once_at(7.0, "登場", flag)]);
        let before = leaf(&world, guarded);
        for _ in 0..3 {
            assert_eq!(turn(&mut world, 1), (vec![], vec![]), "{flag}");
        }
        assert_eq!(leaf(&world, guarded), before, "{flag}");
    }
}

/// 旗標路徑有空段（`事件..登場`、結尾點）：同樣釘不進去，跳過且不在樹上留空鍵。
#[test]
fn once_flag_path_with_an_empty_segment_is_skipped() {
    for flag in ["事件..登場", "事件.", ""] {
        let mut world = fortress("6", &[], vec![once_at(7.0, "登場", flag)]);
        let before = world.state.tree.clone();
        for _ in 0..2 {
            assert_eq!(turn(&mut world, 1), (vec![], vec![]), "{flag:?}");
        }
        let mut expected = before;
        expected.insert(DAYS.to_owned(), StateNode::Leaf("8".to_owned()));
        assert_eq!(world.state.tree, expected, "{flag:?}");
    }
}

/// 收回（樹退回演出前；以 clone 模擬，未走真收回 API）→ 下一回合條件仍成立就重演一次並重新釘旗標 → 之後又只剩 range。
#[test]
fn retracting_the_pin_replays_once_then_settles_again() {
    let mut world = fortress("6", &[], vec![stage(), once_at(7.0, "登場", "事件.登場")]);
    let before = world.clone();
    assert_eq!(turn(&mut world, 1).1, vec!["事件.登場".to_owned()]);
    world = before;
    assert_eq!(leaf(&world, "事件.登場"), None);
    assert_eq!(
        turn(&mut world, 1),
        (
            vec!["登場".to_owned(), "階段".to_owned()],
            vec!["事件.登場".to_owned()]
        )
    );
    assert_eq!(turn(&mut world, 1), (vec!["階段".to_owned()], vec![]));
}

/// 一回合跳過好幾個門檻（第 6 跳到 15）：符合的里程碑同回合都補觸發一次，下一回合全停。
#[test]
fn skipping_days_fires_every_crossed_milestone_once() {
    let mut world = fortress(
        "6",
        &[],
        vec![
            once_at(7.0, "兄弟", "事件.兄弟"),
            once_at(14.0, "元帥", "事件.元帥"),
            once_at(21.0, "野獸", "事件.野獸"),
        ],
    );
    assert_eq!(
        turn(&mut world, 9),
        (
            vec!["元帥".to_owned(), "兄弟".to_owned()],
            vec!["事件.兄弟".to_owned(), "事件.元帥".to_owned()]
        )
    );
    assert_eq!(turn(&mut world, 1), (vec![], vec![]));
    assert_eq!(
        turn(&mut world, 6),
        (vec!["野獸".to_owned()], vec!["事件.野獸".to_owned()])
    );
}

/// EJS 匯入形狀（when 自帶 Flag{expect:false}）照舊：命中一次、之後不再命中。
#[test]
fn ejs_once_with_flag_condition_still_fires_once() {
    let mut world = world_with(&[("World.Invasion", "90")], once_mechanism());
    world.mechanism.incremental = true;
    world.mechanism.rules =
        mechanism_with(&[("World.Invasion", rule(FieldKind::Number, None, None))]).rules;
    let block = StateBlock::default();
    apply_block(&mut world, &block, "玩家");
    assert!(world.state.triggers.contains_key("國變"));
    apply_block(&mut world, &block, "玩家");
    assert!(world.state.triggers.is_empty());
}
