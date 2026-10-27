//! refactor-trigger-overlap 重現：clean-orc 真重構產物（orc-outcome2）兩條機制條目對 驻守天数 各自的觸發，
//! 套用後逐回合推進天數，印出 GM 動態區「當前情境」段實際會收到的文字。

use super::super::test_support::*;
use super::super::*;
use super::rule_reporting::{characters_outcome, select_entries, set_table};
use crate::data::{self, FieldKind, FieldRule};
use crate::refactor_ai::RefactorNewEntry;
use crate::transport::StateBlock;

const DAYS: &str = "驻守天数";

fn add_days(state: &mut data::WorldState, delta: i64) -> crate::mechanism::Outcome {
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

/// 照 transport::state_view::gm_dynamic_block 的「當前情境」段組法（依 mechanism.triggers 順序、空行分隔）。
fn situation(state: &data::WorldState) -> Vec<(String, String)> {
    state
        .mechanism
        .triggers
        .iter()
        .filter_map(|t| {
            state
                .state
                .triggers
                .get(&t.id)
                .map(|text| (t.id.clone(), text.clone()))
        })
        .collect()
}

/// once 觸發只在跨過門檻那一回合命中；兩條目並存的支線階段（range）照常每回合印。
#[test]
fn clean_orc_two_entries_triggers_per_turn() {
    let root = TestRoot::new("trigger-overlap-orc");
    let world_id = data::create_world(root.path(), "要塞").unwrap();
    let fixture: Vec<RefactorNewEntry> =
        serde_json::from_str(include_str!("clean_orc_triggers.json")).unwrap();
    let mut entries = Vec::new();
    for entry in fixture {
        let uid = seed_entry(root.path(), &world_id, &entry.title, "原文");
        entries.push(RefactorNewEntry {
            source_uids: vec![uid.to_string()],
            content: format!("{}說明", entry.title),
            ..entry
        });
    }
    set_table(root.path(), &world_id, &[(DAYS, "第 1 天")]);
    let outcome = characters_outcome(entries);
    apply(root.path(), &world_id, &outcome, &select_entries(2)).unwrap();

    let mut state = data::read_state(root.path(), &world_id).unwrap();
    println!("rules[{DAYS}] = {:?}", state.mechanism.rules.get(DAYS));
    println!(
        "triggers = {:?}",
        state
            .mechanism
            .triggers
            .iter()
            .map(|t| (&t.id, t.mode, &t.flag))
            .collect::<Vec<_>>()
    );
    let mut current = 1;
    for day in [1, 6, 7, 8, 10, 12, 14, 15, 21, 22, 30, 31] {
        let out = add_days(&mut state, day - current);
        current = day;
        let leaf = data::node_at(&state.state.tree, &[DAYS.to_owned()]);
        println!("tree {DAYS} = {leaf:?}; notes = {:?}", out.notes);
        let lines = situation(&state);
        println!("\n===== 第 {day} 天：當前情境 {} 段 =====", lines.len());
        for (id, text) in &lines {
            println!("[{id}] {text}");
        }
        let ids: Vec<&str> = lines.iter().map(|(id, _)| id.as_str()).collect();
        let once_now: Vec<&str> = match day {
            7 => vec!["斥候兄弟解鎖"],
            14 => vec!["元帥視察"],
            21 => vec!["野獸出沒"],
            30 => vec!["商隊交易"],
            _ => Vec::new(),
        };
        for once in ["斥候兄弟解鎖", "元帥視察", "野獸出沒", "商隊交易"] {
            assert_eq!(
                ids.contains(&once),
                once_now.contains(&once),
                "第 {day} 天 {once}"
            );
        }
        let pinned = out
            .records
            .iter()
            .filter(|r| r.detail.starts_with("一次性事件已觸發"))
            .count();
        assert_eq!(pinned, once_now.len(), "第 {day} 天一次性記錄筆數");
    }
}

const FLAG: &str = "要塞事件进程.已觸發.斥候兄弟解鎖";

fn clean_orc_world(name: &str) -> (TestRoot, String, RefactorOutcome) {
    let root = TestRoot::new(name);
    let world_id = data::create_world(root.path(), "要塞").unwrap();
    let fixture: Vec<RefactorNewEntry> =
        serde_json::from_str(include_str!("clean_orc_triggers.json")).unwrap();
    let mut entries = Vec::new();
    for entry in fixture {
        let uid = seed_entry(root.path(), &world_id, &entry.title, "原文");
        entries.push(RefactorNewEntry {
            source_uids: vec![uid.to_string()],
            ..entry
        });
    }
    set_table(root.path(), &world_id, &[(DAYS, "第 1 天")]);
    (root, world_id, characters_outcome(entries))
}

/// 重構套用比照 EJS 匯入，替每個 once 旗標補唯讀規則（分支＝旗標首段）。
#[test]
fn applying_once_triggers_pins_read_only_rules_on_their_flags() {
    let (root, world_id, outcome) = clean_orc_world("trigger-overlap-readonly");
    apply(root.path(), &world_id, &outcome, &select_entries(2)).unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    let mut expected = FieldRule::for_kind(FieldKind::ReadOnly);
    expected.branch = Some("要塞事件进程".to_owned());
    for flag in ["斥候兄弟解鎖", "元帥視察", "野獸出沒", "商隊交易"] {
        let path = format!("要塞事件进程.已觸發.{flag}");
        assert_eq!(state.mechanism.rules.get(&path), Some(&expected), "{path}");
    }
}

/// 旗標路徑已有別的規則：不覆蓋（只記 warn）。
#[test]
fn existing_rule_on_a_once_flag_is_not_overwritten() {
    let (root, world_id, outcome) = clean_orc_world("trigger-overlap-keep-rule");
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    let kept = FieldRule::for_kind(FieldKind::Text);
    state.mechanism.rules.insert(FLAG.to_owned(), kept.clone());
    data::write_state(root.path(), &world_id, &state).unwrap();

    apply(root.path(), &world_id, &outcome, &select_entries(2)).unwrap();

    let state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(state.mechanism.rules.get(FLAG), Some(&kept));
}
