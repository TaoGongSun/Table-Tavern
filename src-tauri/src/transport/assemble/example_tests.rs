//! 本桌更新範例（takeover-mvu-path-mismatch 改法 4）：無 guide 的 DeltaOnly 增量桌也附範例，
//! 挑選跳過清單欄與萬用路徑；未重構 MVU 桌（Upstream）提示詞逐字不變。

use super::super::state_view::StateScope;
use crate::data::{FieldKind, FieldRule, Mechanism, NumericUpdate, TableState, UpdateMode};
use crate::transport::test_support::legacy::assemble_gm_messages;
use std::collections::BTreeMap;

const HEADING: &str = "本桌的更新區塊長這樣";

fn rule(kind: FieldKind) -> FieldRule {
    FieldRule::for_kind(kind)
}

fn system(mechanism: &Mechanism) -> String {
    assemble_gm_messages(
        "",
        &[],
        None,
        &[],
        &[],
        &TableState::default(),
        mechanism,
        &StateScope::default(),
        "zh-TW",
    )[0]
    .content
    .clone()
}

fn mechanism(rules: &[(&str, FieldRule)], numeric: NumericUpdate, guide: &str) -> Mechanism {
    Mechanism {
        incremental: true,
        numeric_update: numeric,
        guide: guide.to_owned(),
        rules: rules
            .iter()
            .map(|(path, rule)| ((*path).to_owned(), rule.clone()))
            .collect::<BTreeMap<_, _>>(),
        ..Mechanism::default()
    }
}

#[test]
fn delta_only_table_without_guide_gets_real_path_example() {
    let content = system(&mechanism(
        &[
            ("冒险者.*.恐惧值", rule(FieldKind::Number)),
            ("基础信息.已覆灭小队数", rule(FieldKind::Counter)),
            ("基础信息.时间", rule(FieldKind::Text)),
            ("魔物", rule(FieldKind::List)),
        ],
        NumericUpdate::DeltaOnly,
        "",
    ));
    assert!(content.contains(HEADING));
    assert!(content.contains(r#"{"op":"delta","path":"/基础信息/已覆灭小队数","value":-1}"#));
    assert!(content.contains(r#"{"op":"replace","path":"/基础信息/时间","value":"（新的值）"}"#));
    assert!(!content.contains("/魔物"));
    assert!(!content.contains("/*/"));
    assert!(!content.contains("## 介面由誰畫"));
}

#[test]
fn example_lines_are_skipped_when_no_usable_rule() {
    // 只有文字欄：沒有 delta 行；只有數字欄：沒有 replace 行（不拿文字 placeholder 示範數字 replace）
    let text_only = system(&mechanism(
        &[("基础信息.时间", rule(FieldKind::Text))],
        NumericUpdate::DeltaOnly,
        "",
    ));
    let example = &text_only[text_only.find(HEADING).unwrap()..];
    assert!(!example.contains(r#""op":"delta""#));
    let number_only = system(&mechanism(
        &[(
            "甲.HP",
            FieldRule {
                update: UpdateMode::Replace,
                ..rule(FieldKind::Number)
            },
        )],
        NumericUpdate::DeltaOnly,
        "",
    ));
    assert!(!number_only.contains(HEADING));
    // 只有清單欄與萬用路徑：整段不附
    let unusable = system(&mechanism(
        &[
            ("魔物", rule(FieldKind::List)),
            ("*.HP", rule(FieldKind::Number)),
        ],
        NumericUpdate::DeltaOnly,
        "",
    ));
    assert!(!unusable.contains(HEADING));
}

#[test]
fn upstream_table_with_imported_rules_is_unchanged() {
    let with_rules = system(&mechanism(
        &[("甲.HP", rule(FieldKind::Number))],
        NumericUpdate::Upstream,
        "",
    ));
    let without = system(&mechanism(&[], NumericUpdate::Upstream, ""));
    assert_eq!(with_rules, without);
}

#[test]
fn non_incremental_table_gets_no_example() {
    let mut table = mechanism(
        &[("甲.HP", rule(FieldKind::Number))],
        NumericUpdate::DeltaOnly,
        "",
    );
    table.incremental = false;
    assert!(!system(&table).contains(HEADING));
}

#[test]
fn guide_table_example_skips_wildcard_paths_and_keeps_order() {
    let content = system(&mechanism(
        &[
            ("Heroes.*.Affection", rule(FieldKind::Number)),
            ("World.Day", rule(FieldKind::Counter)),
        ],
        NumericUpdate::DeltaOnly,
        "World.Day 每回合必報。",
    ));
    let guide_at = content.find("World.Day 每回合必報").unwrap();
    let example_at = content.find(HEADING).unwrap();
    assert!(guide_at < example_at && example_at < content.find("## 介面由誰畫").unwrap());
    assert!(content.contains(r#"{"op":"delta","path":"/World/Day","value":-1}"#));
    assert!(!content.contains("/Heroes/*/"));
}
