//! statusbar-yaml-list-flatten：變數模式（帶 stat_data）套用狀態區塊清單。只有容器「不存在」或「現為索引分支」
//! 才整支替換；stat_data 陣列（樹上是 JSON 文字葉子）與具名物件不動、這支的欄位也不寫。
use super::convert::{merge_tree_change_typed, stat_to_tree, NewValues};
use super::json::parse;
use crate::data::NumericUpdate;
use std::collections::BTreeMap;

/// 照 GM 回合提交的路：stat_data 投影成樹 → apply_block_typed（帶 stat_data）→ 照帶型別合回 stat_data。
fn commit(stat: &str, reply: &str) -> String {
    commit_with_records(stat, reply).0
}

fn commit_with_records(stat: &str, reply: &str) -> (String, Vec<crate::mechanism::Record>) {
    let stat = parse(stat).unwrap();
    let mut state: crate::data::WorldState = serde_json::from_value(serde_json::json!({
        "id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "name": "清單桌",
        "current_scene": 0
    }))
    .unwrap();
    state.state.tree = stat_to_tree(Some(&stat));
    state.mechanism.numeric_update = NumericUpdate::Upstream;
    let before = state.state.tree.clone();
    let outcome = crate::mechanism::apply_block_typed(
        &mut state,
        &crate::transport::extract_state_block(reply),
        "阿濤",
        Some(&stat),
        crate::mechanism::BlockScope::Table,
    );
    let types = BTreeMap::new();
    let rules = NewValues {
        types: &types,
        keep_strings: false,
        clean: None,
    };
    let merged = merge_tree_change_typed(
        &stat,
        &before,
        &state.state.tree,
        &rules,
        outcome.typed.as_ref(),
    )
    .to_text();
    (merged, outcome.records)
}

fn status(body: &str) -> String {
    format!("旁白。\n<Status_block>\n{body}</Status_block>")
}

fn same(left: &str, right: &str) {
    assert_eq!(
        parse(left).unwrap().to_text(),
        parse(right).unwrap().to_text()
    );
}

#[test]
fn stat_data_array_is_not_turned_into_an_object() {
    let stat = r#"{"人物列表":[{"名字":"林远"},{"名字":"贺铮"}],"状态":{"地点":"前院"}}"#;
    let merged = commit(
        stat,
        &status("人物列表:\n  - 名字: 周掌柜\n    身份: 军需官\n状态:\n  地点: 马厩\n"),
    );
    same(
        &merged,
        r#"{"人物列表":[{"名字":"林远"},{"名字":"贺铮"}],"状态":{"地点":"马厩"}}"#,
    );
}

#[test]
fn named_object_in_stat_data_stays_untouched() {
    let stat = r#"{"资源":{"粮草":400,"马匹":37}}"#;
    let merged = commit(stat, &status("资源:\n  - 粮草\n  - 马匹\n"));
    same(&merged, stat);
}

/// 索引分支（stat_data 裡的 `{"0":…}` 物件，非陣列葉子）照整支替換，移除反映到 stat_data
#[test]
fn index_branch_in_stat_data_is_replaced() {
    let stat = r#"{"名單":{"0":"林远","1":"贺铮","2":"周掌柜"}}"#;
    let merged = commit(stat, &status("名單:\n  - 林远\n"));
    same(&merged, r#"{"名單":{"0":"林远"}}"#);
}

/// 已知限制：原本不存在的清單寫進 stat_data 是 `{"0":…}` 物件、不是陣列
#[test]
fn new_list_lands_as_an_index_object() {
    let merged = commit(r#"{"地点":"前院"}"#, &status("名單:\n  - 林远\n  - 贺铮\n"));
    same(&merged, r#"{"地点":"前院","名單":{"0":"林远","1":"贺铮"}}"#);
}

/// 父清單被略過（具名分支）時，底下的巢狀清單與欄位一律不動，只記一筆 Ignored、不回饋 GM
#[test]
fn skipped_parent_keeps_nested_lists_untouched() {
    let stat = r#"{"R":{"0":{"gear":{"0":"sword"}},"note":"keep"}}"#;
    let (merged, records) = commit_with_records(stat, &status("R:\n  - gear:\n      - shield\n"));
    same(&merged, stat);
    let ignored: Vec<&str> = records
        .iter()
        .filter(|record| record.kind == crate::mechanism::RecordKind::Ignored)
        .map(|record| record.path.as_str())
        .collect();
    assert_eq!(ignored, vec!["R"]);
    assert!(records
        .iter()
        .all(|record| record.kind != crate::mechanism::RecordKind::Rejected));
}

/// 非空 → 空 → 非空：空分支也算可重寫
#[test]
fn emptied_list_can_be_filled_again() {
    let emptied = commit(r#"{"名單":{"0":"林远","1":"贺铮"}}"#, &status("名單: []\n"));
    same(&emptied, r#"{"名單":{}}"#);
    let refilled = commit(&emptied, &status("名單:\n  - 周掌柜\n"));
    same(&refilled, r#"{"名單":{"0":"周掌柜"}}"#);
}

/// 略過但這回合沒有欄位被丟（空清單）：不記 Ignored，免得每回合重複記同一筆
#[test]
fn skipped_empty_list_records_nothing() {
    let stat = r#"{"人物列表":[{"名字":"林远"}]}"#;
    let (merged, records) = commit_with_records(stat, &status("人物列表: []\n"));
    same(&merged, stat);
    assert!(records
        .iter()
        .all(|record| record.kind != crate::mechanism::RecordKind::Ignored));
}
