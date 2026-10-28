//! statusbar-only-fields：在場名單集合裡不在場、但已有資料的項目，GM 動態區塊帶集合路徑列出。
use super::{gm_dynamic_block, StateScope};
use crate::data::{Mechanism, StateNode, TableState};
use std::collections::BTreeMap;

fn north_absent_state(present: &str) -> (TableState, Mechanism) {
    let person = |order: &str| {
        StateNode::Branch(BTreeMap::from([
            ("_序".to_owned(), StateNode::Leaf(order.to_owned())),
            ("体力".to_owned(), StateNode::Leaf("80".to_owned())),
        ]))
    };
    let state = TableState {
        table: BTreeMap::from([("present".to_owned(), present.to_owned())]),
        tree: BTreeMap::from([(
            "状态栏".to_owned(),
            StateNode::Branch(BTreeMap::from([(
                "人物列表".to_owned(),
                StateNode::Branch(BTreeMap::from([
                    ("林远".to_owned(), person("0")),
                    ("贺铮".to_owned(), person("1")),
                    ("周掌柜".to_owned(), person("2")),
                ])),
            )])),
        )]),
        ..Default::default()
    };
    let mut mechanism = Mechanism {
        incremental: true,
        ..Mechanism::default()
    };
    mechanism.collections = serde_json::from_value(serde_json::json!({
        "状态栏.人物列表": { "keyed": true, "item": "object", "fields": { "体力": "text" }, "present_scoped": true },
    }))
    .unwrap();
    (state, mechanism)
}

/// 在場名單集合有不在場的人：動態區塊帶集合路徑列出，提醒回場不要重新 insert；present 空不附；
/// `{{user}}` 換玩家名、字形折疊比得到的算在場。
#[test]
fn gm_dynamic_block_lists_absent_items_of_present_scoped_collections() {
    let render = |present: &str| {
        let (state, mechanism) = north_absent_state(present);
        gm_dynamic_block(
            "",
            &state,
            "林远",
            &mechanism,
            &StateScope::default(),
            "zh-TW",
        )
    };
    let dynamic = render("{{user}}、賀錚");
    assert!(
        dynamic.contains("`/状态栏/人物列表` 不在場但已有資料：周掌柜；回場時只報有變動的格子，不要重新 insert。"),
        "{dynamic}"
    );
    assert!(!render("").contains("不在場但已有資料"));
    assert!(!render("林远、贺铮、周掌柜").contains("不在場但已有資料"));
}
