//! statusbar-yaml-list-flatten：清單出身的索引分支印給 GM 的形狀，與狀態區塊解析器往返一致。
use super::{render_state_tree, TreeRender};
use crate::data::{Mechanism, StateNode};
use std::collections::BTreeMap;

fn leaf(value: &str) -> StateNode {
    StateNode::Leaf(value.to_owned())
}

fn branch(children: Vec<(&str, StateNode)>) -> StateNode {
    StateNode::Branch(
        children
            .into_iter()
            .map(|(key, node)| (key.to_owned(), node))
            .collect(),
    )
}

fn list(items: Vec<StateNode>) -> StateNode {
    StateNode::Branch(
        items
            .into_iter()
            .enumerate()
            .map(|(index, node)| (index.to_string(), node))
            .collect(),
    )
}

fn render(tree: &BTreeMap<String, StateNode>, incremental: bool) -> String {
    let mechanism = Mechanism {
        incremental,
        ..Mechanism::default()
    };
    let changes = BTreeMap::new();
    let mut output = String::new();
    render_state_tree(
        &mut output,
        tree,
        &TreeRender {
            mechanism: &mechanism,
            changes: &changes,
            hidden: &[],
            align: true,
            user_name: "阿濤",
            base: 0,
            en: false,
        },
        &mut Vec::new(),
    );
    output
}

fn sample() -> BTreeMap<String, StateNode> {
    let people = (0..12)
        .map(|index| {
            branch(vec![(
                "人物",
                branch(vec![
                    ("名字", leaf(&format!("人{index}"))),
                    ("身份", leaf("驿卒：夜班")),
                ]),
            )])
        })
        .collect();
    BTreeMap::from([(
        "状态栏".to_owned(),
        branch(vec![
            ("人物列表", list(people)),
            (
                "选项",
                list(vec![
                    leaf("1. 核对马匹：先看马厩"),
                    leaf("2. 去门外"),
                    leaf("- 破折号开头"),
                ]),
            ),
            ("空", list(Vec::new())),
            (
                "格子",
                list(vec![
                    list(vec![leaf("a"), leaf("b")]),
                    list(vec![leaf("c")]),
                    list(Vec::new()),
                ]),
            ),
            ("地点", leaf("驿站 前院")),
            ("备注", leaf("第一行\n第二行：带冒号")),
            (
                "日志",
                list(vec![
                    leaf("清点马匹\n少了三匹"),
                    leaf("单行"),
                    leaf("他说：走吧\n- 条列\n结尾: 冒号"),
                ]),
            ),
            ("笔记", leaf("开头\n- 条列一\n键: 值")),
        ]),
    )])
}

#[test]
fn full_table_prints_lists_as_yaml_in_numeric_order() {
    let text = render(&sample(), false);
    assert!(text.contains("  空：[]\n"), "{text}");
    assert!(
        text.contains("  备注：|\n    第一行\n    第二行：带冒号\n"),
        "{text}"
    );
    assert!(
        text.contains("  日志：\n    - |\n      清点马匹\n      少了三匹\n    - 单行\n"),
        "{text}"
    );
    assert!(
        text.contains(
            "  选项：\n    - \"1. 核对马匹：先看马厩\"\n    - 2. 去门外\n    - \"- 破折号开头\"\n"
        ),
        "{text}"
    );
    assert!(
        text.contains("  格子：\n    - - a\n      - b\n    - - c\n    - []\n"),
        "{text}"
    );
    assert!(
        text.contains("    - 人物：\n        名字：人1\n        身份：驿卒：夜班\n    - 人物：\n        名字：人2\n"),
        "{text}"
    );
    let ten = text.find("名字：人10").unwrap();
    assert!(text.find("名字：人9").unwrap() < ten && ten < text.find("名字：人11").unwrap());
}

#[test]
fn incremental_table_keeps_index_keys_in_numeric_order() {
    let tree = BTreeMap::from([(
        "名單".to_owned(),
        list((0..12).map(|index| leaf(&format!("人{index}"))).collect()),
    )]);
    let text = render(&tree, true);
    assert!(
        text.starts_with("名單：\n  0：人0\n  1：人1\n  2：人2\n"),
        "{text}"
    );
    assert!(
        text.contains("  9：人9\n  10：人10\n  11：人11\n"),
        "{text}"
    );
}

/// 全量桌印出來的樹丟回狀態區塊解析器、套到空桌，要得回同一棵樹。
#[test]
fn rendered_tree_round_trips_through_the_status_block_parser() {
    let tree = sample();
    let text = render(&tree, false);
    let mut world: crate::data::WorldState = serde_json::from_value(serde_json::json!({
        "id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "name": "往返桌",
        "current_scene": 0
    }))
    .unwrap();
    let block = crate::transport::extract_state_block(&format!(
        "旁白。\n<Status_block>\n{text}</Status_block>"
    ));
    crate::mechanism::apply_block_typed(
        &mut world,
        &block,
        "阿濤",
        None,
        crate::mechanism::BlockScope::Table,
    );
    assert_eq!(world.state.tree, tree, "{text}");
}
