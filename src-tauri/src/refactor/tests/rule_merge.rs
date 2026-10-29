//! refactor-rule-conflict-same-field：同一欄有多份候選規則時，相容就合併、真矛盾只裁決這一欄，
//! 不讓整次套用失敗（clean-orc 第一次真重構：兩條機制條目對 驻守天数 只差 min 0／1 就整次被擋）。

use super::super::rule_merge::{merge_compatible, reconcile, RuleCandidate};
use super::super::test_support::*;
use super::super::*;
use super::rule_reporting::{
    characters_outcome, mechanism_entry, once_at_day, select_entries, set_table,
};
use crate::data::{self, FieldKind, FieldRule, InjectLevel, UpdateMode};
use crate::receipts;
use crate::transport::StateBlock;

const DAYS: &str = "驻守天数";

fn rule(kind: FieldKind, min: Option<f64>, max: Option<f64>) -> FieldRule {
    FieldRule {
        min,
        max,
        ..FieldRule::for_kind(kind)
    }
}

fn counter_from(min: f64) -> FieldRule {
    rule(FieldKind::Counter, Some(min), None)
}

fn day_key() -> Vec<String> {
    vec![DAYS.to_owned()]
}

fn delta_days(state: &mut data::WorldState, delta: i64) -> crate::mechanism::Outcome {
    let block = StateBlock {
        fields: Vec::new(),
        updates: vec![format!(
            r#"<JSONPatch>[{{"op":"delta","path":"/{DAYS}","value":{delta}}}]</JSONPatch>"#
        )],
        display: String::new(),
        lists: Vec::new(),
        maps: Vec::new(),
        fenced: None,
    };
    crate::mechanism::apply_block(state, &block, "玩家")
}

/// 一桌兩條來源（依序 seed，uid 遞增）、開場平欄寫好天數，回傳 (root, world_id, [uid…])。
fn fortress(name: &str, opening: &str, sources: usize) -> (TestRoot, String, Vec<u64>) {
    let root = TestRoot::new(name);
    let world_id = data::create_world(root.path(), "要塞").unwrap();
    let uids = (0..sources)
        .map(|n| seed_entry(root.path(), &world_id, &format!("來源{n}"), "原文"))
        .collect();
    set_table(root.path(), &world_id, &[(DAYS, opening)]);
    (root, world_id, uids)
}

/// clean-orc 重現：只差 min 的兩份 counter 合併成 min=1 一條，整次套得上；撤銷全部退回。
#[test]
fn clean_orc_rules_differing_only_in_min_merge_and_undo_restores() {
    let (root, world_id, uids) = fortress("rule-merge-orc", "第 1 天", 2);
    let outcome = characters_outcome(vec![
        mechanism_entry(
            "Bram & Gil Progression",
            uids[1],
            &[(DAYS, counter_from(0.0))],
            vec![once_at_day(14.0, DAYS, "要塞事件旗標.兄弟")],
        ),
        mechanism_entry(
            "駐守天數與要塞事件推進",
            uids[0],
            &[(DAYS, counter_from(1.0))],
            vec![once_at_day(7.0, DAYS, "要塞事件旗標.斥候兄弟登場")],
        ),
    ]);

    apply_recorded(root.path(), &world_id, &outcome, &select_entries(2));

    let state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(state.mechanism.rules.get(DAYS), Some(&counter_from(1.0)));
    assert_eq!(state.mechanism.triggers.len(), 2);
    assert_eq!(
        data::node_at(&state.state.tree, &day_key()),
        Some(&data::StateNode::Leaf("1".to_owned()))
    );

    receipts::undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(!state.mechanism.rules.contains_key(DAYS));
    assert!(state.mechanism.triggers.is_empty());
    assert!(state.state.tree.is_empty());
    assert_eq!(
        state.state.table.get(DAYS).map(String::as_str),
        Some("第 1 天")
    );
}

/// 開場值低於合併後的 min：套用當下不夾（照現狀），之後的 delta 才夾到 min（已知限制）。
#[test]
fn opening_value_below_merged_min_is_kept_until_the_next_delta() {
    let (root, world_id, uids) = fortress("rule-merge-below-min", "第 0 天", 2);
    let outcome = characters_outcome(vec![
        mechanism_entry("甲", uids[0], &[(DAYS, counter_from(0.0))], Vec::new()),
        mechanism_entry("乙", uids[1], &[(DAYS, counter_from(1.0))], Vec::new()),
    ]);
    apply(root.path(), &world_id, &outcome, &select_entries(2)).unwrap();

    let mut state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(state.mechanism.rules.get(DAYS), Some(&counter_from(1.0)));
    assert_eq!(
        data::node_at(&state.state.tree, &day_key()),
        Some(&data::StateNode::Leaf("0".to_owned()))
    );
    delta_days(&mut state, 0);
    assert_eq!(
        data::node_at(&state.state.tree, &day_key()),
        Some(&data::StateNode::Leaf("1".to_owned()))
    );
}

/// 三條來源真矛盾（text 對 counter）：整欄一次裁決，原樣取來源 uid 最小那份，敗方的 min 不殘留；
/// 產物順序與選取順序反轉，結果相同。
#[test]
fn three_way_conflict_takes_the_smallest_source_uid_regardless_of_order() {
    let text_rule = FieldRule::for_kind(FieldKind::Text);
    let rules_after = |reverse: bool| {
        let (root, world_id, uids) = fortress("rule-merge-three-way", "第 1 天", 3);
        let mut entries = vec![
            mechanism_entry("丙", uids[2], &[(DAYS, counter_from(5.0))], Vec::new()),
            mechanism_entry("乙", uids[1], &[(DAYS, counter_from(0.0))], Vec::new()),
            mechanism_entry("甲", uids[0], &[(DAYS, text_rule.clone())], Vec::new()),
        ];
        let mut selection = select_entries(3);
        if reverse {
            entries.reverse();
            selection.entry_indices.reverse();
        }
        apply(
            root.path(),
            &world_id,
            &characters_outcome(entries),
            &selection,
        )
        .unwrap();
        data::read_state(root.path(), &world_id)
            .unwrap()
            .mechanism
            .rules
    };
    let forward = rules_after(false);
    assert_eq!(forward.get(DAYS), Some(&text_rule));
    assert_eq!(forward, rules_after(true));
}

/// 真矛盾的欄位進摘要；相容合併的不列。
#[test]
fn only_true_conflicts_are_reported_in_the_summary() {
    let conflicts = |second: FieldRule| {
        let (root, world_id, uids) = fortress("rule-merge-reported", "第 1 天", 2);
        let outcome = characters_outcome(vec![
            mechanism_entry("甲", uids[0], &[(DAYS, counter_from(0.0))], Vec::new()),
            mechanism_entry("乙", uids[1], &[(DAYS, second)], Vec::new()),
        ]);
        apply(root.path(), &world_id, &outcome, &select_entries(2))
            .unwrap()
            .summary
            .skipped
            .conflicts
    };
    assert_eq!(
        conflicts(FieldRule::for_kind(FieldKind::Text)),
        vec![DAYS.to_owned()]
    );
    assert!(conflicts(counter_from(1.0)).is_empty());
}

/// 相容合併也不受選取順序影響。
#[test]
fn compatible_merge_ignores_selection_order() {
    let rules_after = |order: Vec<usize>| {
        let (root, world_id, uids) = fortress("rule-merge-order", "第 1 天", 2);
        let outcome = characters_outcome(vec![
            mechanism_entry(
                "甲",
                uids[0],
                &[(DAYS, rule(FieldKind::Counter, Some(0.0), Some(30.0)))],
                Vec::new(),
            ),
            mechanism_entry(
                "乙",
                uids[1],
                &[(DAYS, rule(FieldKind::Counter, Some(1.0), Some(60.0)))],
                Vec::new(),
            ),
        ]);
        let mut selection = select_entries(2);
        selection.entry_indices = order;
        apply(root.path(), &world_id, &outcome, &selection).unwrap();
        data::read_state(root.path(), &world_id)
            .unwrap()
            .mechanism
            .rules
    };
    let forward = rules_after(vec![0, 1]);
    assert_eq!(
        forward.get(DAYS),
        Some(&rule(FieldKind::Counter, Some(1.0), Some(30.0)))
    );
    assert_eq!(forward, rules_after(vec![1, 0]));
}

/// 同一條目兩種字形對到同一路徑：相容就合併；矛盾時依原路徑字典序先出現的那份（駐 U+99D0 先於 驻 U+9A7B）。
#[test]
fn glyph_variants_within_one_entry_merge_or_take_the_first_path() {
    let applied = |rules: &[(&str, FieldRule)]| {
        let (root, world_id, uids) = fortress("rule-merge-glyph", "第 1 天", 1);
        let outcome = characters_outcome(vec![mechanism_entry("甲", uids[0], rules, Vec::new())]);
        apply(root.path(), &world_id, &outcome, &select_entries(1)).unwrap();
        data::read_state(root.path(), &world_id)
            .unwrap()
            .mechanism
            .rules
    };
    let merged = applied(&[("駐守天數", counter_from(0.0)), (DAYS, counter_from(1.0))]);
    assert_eq!(merged.get(DAYS), Some(&counter_from(1.0)));
    assert!(!merged.contains_key("駐守天數"));
    let decided = applied(&[
        ("駐守天數", counter_from(0.0)),
        (DAYS, FieldRule::for_kind(FieldKind::Text)),
    ]);
    assert_eq!(decided.get(DAYS), Some(&counter_from(0.0)));
}

/// 同條目字形變體與跨條目一起整欄裁決一次（不先在條目內合併）：uid 小的條目兩種字形都是 counter、
/// min 0／1，另一條目寫 text——矛盾時原樣取 uid 最小條目裡原路徑字典序先出現那份（駐… 的 min 0），
/// 不是條目內先合成的 min 1；text 條目 uid 較小時取 text。
#[test]
fn glyph_variants_and_other_entries_are_decided_together_once() {
    let decided = |text_first: bool| {
        let (root, world_id, uids) = fortress("rule-merge-mixed", "第 1 天", 2);
        let (counter_uid, text_uid) = if text_first {
            (uids[1], uids[0])
        } else {
            (uids[0], uids[1])
        };
        let outcome = characters_outcome(vec![
            mechanism_entry(
                "甲",
                counter_uid,
                &[("駐守天數", counter_from(0.0)), (DAYS, counter_from(1.0))],
                Vec::new(),
            ),
            mechanism_entry(
                "乙",
                text_uid,
                &[(DAYS, FieldRule::for_kind(FieldKind::Text))],
                Vec::new(),
            ),
        ]);
        apply(root.path(), &world_id, &outcome, &select_entries(2)).unwrap();
        data::read_state(root.path(), &world_id)
            .unwrap()
            .mechanism
            .rules
            .get(DAYS)
            .cloned()
    };
    assert_eq!(decided(false), Some(counter_from(0.0)));
    assert_eq!(decided(true), Some(FieldRule::for_kind(FieldKind::Text)));
}

#[test]
fn inject_takes_the_most_frequent_for_every_pair() {
    let levels = [InjectLevel::Turn, InjectLevel::Snapshot, InjectLevel::Rare];
    let rank = |level: InjectLevel| levels.iter().position(|l| *l == level).unwrap();
    for a in levels {
        for b in levels {
            let with = |inject| FieldRule {
                inject,
                ..FieldRule::for_kind(FieldKind::Text)
            };
            let merged = merge_compatible(&[&with(a), &with(b)]).unwrap();
            let expected = if rank(a) <= rank(b) { a } else { b };
            assert_eq!(merged.inject, expected, "{a:?} + {b:?}");
        }
    }
}

#[test]
fn kind_update_formula_and_branch_must_agree() {
    let number = FieldRule::for_kind(FieldKind::Number);
    let counter = FieldRule::for_kind(FieldKind::Counter);
    assert!(merge_compatible(&[&number, &counter]).is_none());
    let replace = FieldRule {
        update: UpdateMode::Replace,
        ..counter.clone()
    };
    assert!(merge_compatible(&[&counter, &replace]).is_none());
    let derived = |formula: &str| FieldRule {
        formula: Some(formula.to_owned()),
        ..FieldRule::for_kind(FieldKind::Derived)
    };
    assert!(merge_compatible(&[&derived("a+b"), &derived("a-b")]).is_none());
    assert_eq!(
        merge_compatible(&[&derived("a+b"), &derived("a+b")]),
        Some(derived("a+b"))
    );
    let branch = |name: Option<&str>| FieldRule {
        branch: name.map(str::to_owned),
        ..counter.clone()
    };
    assert_eq!(
        merge_compatible(&[&branch(None), &branch(Some("艾拉"))]),
        Some(branch(Some("艾拉")))
    );
    assert_eq!(
        merge_compatible(&[&branch(Some("艾拉")), &branch(None)]),
        Some(branch(Some("艾拉")))
    );
    assert!(merge_compatible(&[&branch(Some("艾拉")), &branch(Some("布蘭"))]).is_none());
    let mirrored = FieldRule {
        same_as: Some("place".to_owned()),
        ..FieldRule::for_kind(FieldKind::Text)
    };
    assert_eq!(
        merge_compatible(&[&mirrored, &FieldRule::for_kind(FieldKind::Text)]),
        Some(FieldRule::for_kind(FieldKind::Text))
    );
}

/// 一般型缺省＝無界（不當 0）；相等合法，交集為空不相容。
#[test]
fn plain_bounds_intersect_with_missing_as_unbounded() {
    let counter = |min, max| rule(FieldKind::Counter, min, max);
    let merged = |a: FieldRule, b: FieldRule| merge_compatible(&[&a, &b]).map(|r| (r.min, r.max));
    assert_eq!(
        merged(counter(None, None), counter(None, None)),
        Some((None, None))
    );
    assert_eq!(
        merged(counter(None, None), counter(Some(-3.0), None)),
        Some((Some(-3.0), None))
    );
    assert_eq!(
        merged(counter(Some(0.0), Some(10.0)), counter(Some(2.0), None)),
        Some((Some(2.0), Some(10.0)))
    );
    assert_eq!(
        merged(counter(Some(5.0), None), counter(None, Some(5.0))),
        Some((Some(5.0), Some(5.0)))
    );
    assert_eq!(
        merged(counter(Some(6.0), None), counter(None, Some(5.0))),
        None
    );
}

/// pair 缺省下限是 0、上限來自現值/上限（無界）；roll 缺省是 1–100 重擲範圍。
#[test]
fn pair_and_roll_bounds_use_their_runtime_defaults() {
    let merged = |kind, a: (Option<f64>, Option<f64>), b: (Option<f64>, Option<f64>)| {
        merge_compatible(&[&rule(kind, a.0, a.1), &rule(kind, b.0, b.1)]).map(|r| (r.min, r.max))
    };
    use FieldKind::{Pair, Roll};
    assert_eq!(merged(Pair, (None, None), (None, None)), Some((None, None)));
    assert_eq!(
        merged(Pair, (None, None), (Some(-5.0), None)),
        Some((Some(0.0), None))
    );
    assert_eq!(
        merged(Pair, (None, None), (None, Some(10.0))),
        Some((None, Some(10.0)))
    );
    assert_eq!(merged(Pair, (None, None), (None, Some(-1.0))), None);
    assert_eq!(merged(Roll, (None, None), (None, None)), Some((None, None)));
    assert_eq!(
        merged(Roll, (None, None), (Some(5.0), None)),
        Some((Some(5.0), None))
    );
    assert_eq!(
        merged(Roll, (None, None), (None, Some(500.0))),
        Some((None, Some(100.0)))
    );
    assert_eq!(merged(Roll, (None, None), (Some(150.0), None)), None);
}

/// 裁決：同 uid 依候選先後；非數字 uid 排在最後。
#[test]
fn reconcile_breaks_ties_by_order_and_puts_non_numeric_uids_last() {
    let candidate = |rule: FieldRule, uid| RuleCandidate {
        rule,
        uid,
        origin: String::new(),
    };
    let text = FieldRule::for_kind(FieldKind::Text);
    let counter = FieldRule::for_kind(FieldKind::Counter);
    assert_eq!(
        reconcile(
            "x",
            &[
                candidate(text.clone(), Some(3)),
                candidate(counter.clone(), Some(3))
            ]
        )
        .0,
        text
    );
    assert_eq!(
        reconcile(
            "x",
            &[
                candidate(text.clone(), None),
                candidate(counter.clone(), Some(9))
            ]
        )
        .0,
        counter
    );
}
