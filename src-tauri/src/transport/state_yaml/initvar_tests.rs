//! initvar-yaml-list：initvar 清單模式解析。清單斷言整個陣列（型別、長度、順序）並驗物化結果。
use super::{parse_initvar_tree, parse_state_fields};
use crate::data::message_vars::restore_leaf;
use crate::data::StateNode;
use serde_json::{json, Value};
use std::collections::BTreeMap;

type Tree = BTreeMap<String, StateNode>;

fn node<'a>(tree: &'a Tree, path: &[&str]) -> &'a StateNode {
    let (first, rest) = path.split_first().unwrap();
    let mut current = tree.get(*first).unwrap_or_else(|| panic!("缺 {first}"));
    for segment in rest {
        current = match current {
            StateNode::Branch(children) => children
                .get(*segment)
                .unwrap_or_else(|| panic!("缺 {segment}")),
            StateNode::Leaf(_) => panic!("{segment} 前是葉子"),
        };
    }
    current
}

fn leaf<'a>(tree: &'a Tree, path: &[&str]) -> &'a str {
    match node(tree, path) {
        StateNode::Leaf(text) => text,
        StateNode::Branch(_) => panic!("{path:?} 是分支"),
    }
}

/// 葉子物化（同 `tree_to_stat` 的葉子還原）後的值。
fn typed(tree: &Tree, path: &[&str]) -> Value {
    serde_json::from_str(&restore_leaf(leaf(tree, path), None).to_text()).unwrap()
}

/// 清單葉子：原文就是該陣列的 JSON，物化後也是同一個陣列。
fn assert_list(tree: &Tree, path: &[&str], expected: Value) {
    let text = leaf(tree, path);
    assert_eq!(
        serde_json::from_str::<Value>(text).unwrap(),
        expected,
        "{path:?} 原文 {text}"
    );
    assert_eq!(typed(tree, path), expected, "{path:?} 物化");
}

#[test]
fn lists_become_json_arrays() {
    let tree = parse_initvar_tree(
        "角色:\n  名字: 艾琳\n  物品:\n    - 長劍\n    - 治療藥水\n  技能:\n    - 名稱: 火球\n      消耗: 5\n    - 名稱: 冰牆\n      消耗: 8\n任務:\n- 找到鑰匙\n- 打開門\n",
    );
    assert_eq!(leaf(&tree, &["角色", "名字"]), "艾琳");
    assert_list(&tree, &["角色", "物品"], json!(["長劍", "治療藥水"]));
    assert_list(
        &tree,
        &["角色", "技能"],
        json!([{"名稱": "火球", "消耗": 5}, {"名稱": "冰牆", "消耗": 8}]),
    );
    assert_list(&tree, &["任務"], json!(["找到鑰匙", "打開門"]));
}

#[test]
fn item_keys_need_yaml_colon() {
    let tree = parse_initvar_tree(
        "清單:\n  - 08:00 起床\n  - https://x\n  - 火球：造成10傷害\n  - {\"a\":1}\n  - \"鍵: 值\"\n  - 名稱: 08:00\n",
    );
    assert_list(
        &tree,
        &["清單"],
        json!(["08:00 起床", "https://x", "火球：造成10傷害", {"a": 1}, "鍵: 值", {"名稱": "08:00"}]),
    );
}

#[test]
fn empty_items_keep_their_index() {
    let tree = parse_initvar_tree(
        "技能槽:\n  - \"\"\n  -\n  - {}\n元素:\n  - 空:\n    空字串: \"\"\n    空物件: {}\n空:\n",
    );
    assert_list(&tree, &["技能槽"], json!(["", null, {}]));
    assert_list(
        &tree,
        &["元素"],
        json!([{"空": null, "空字串": "", "空物件": {}}]),
    );
    // 清單外的具名分支照舊：值空就是空分支
    assert!(matches!(node(&tree, &["空"]), StateNode::Branch(children) if children.is_empty()));
}

#[test]
fn quoted_values_are_strings() {
    let tree = parse_initvar_tree(
        "清單:\n  - \"a\\\"b\\\\c\\n\"\n  - 'it''s'\n  - \"5\"\n\"引號鍵\": 1\n",
    );
    assert_list(&tree, &["清單"], json!(["a\"b\\c\\n", "it's", "5"]));
    assert_eq!(leaf(&tree, &["引號鍵"]), "1");
}

#[test]
fn flow_sequences_in_every_scalar_position() {
    let tree = parse_initvar_tree(
        "值: [火, \"冰, 霜\", '', 3, true, ~]\n清單:\n  - [火, 冰]\n  - 標籤: [火, 範圍]\n巢狀: [a, [b]]\n",
    );
    assert_list(&tree, &["值"], json!(["火", "冰, 霜", "", 3, true, null]));
    assert_list(
        &tree,
        &["清單"],
        json!([["火", "冰"], {"標籤": ["火", "範圍"]}]),
    );
    // 不支援的整段回退原文
    assert_eq!(leaf(&tree, &["巢狀"]), "[a, [b]]");
}

#[test]
fn json_values_drop_metadata_and_stay_verbatim_otherwise() {
    let tree = parse_initvar_tree(
        "VWD: [100, \"生命值\"]\n帶標記: [\"$__META_EXTENSIBLE__$\", \"劍\"]\n清單:\n  - $__META_EXTENSIBLE__$\n  - 盾\n",
    );
    assert_eq!(leaf(&tree, &["VWD"]), "[100, \"生命值\"]");
    assert_eq!(leaf(&tree, &["帶標記"]), "[\"劍\"]");
    assert_list(&tree, &["清單"], json!(["盾"]));
}

#[test]
fn block_scalars_are_strings() {
    let tree = parse_initvar_tree("清單:\n  - |-\n    5\n");
    assert_list(&tree, &["清單"], json!(["5"]));
}

#[test]
fn repeated_paths_merge_like_before() {
    let tree = parse_initvar_tree("角色:\n  名字: 甲\n角色:\n  等級: 2\n值: 1\n值:\n  子: 2\n");
    assert_eq!(leaf(&tree, &["角色", "名字"]), "甲");
    assert_eq!(leaf(&tree, &["角色", "等級"]), "2");
    assert_eq!(leaf(&tree, &["值"]), "1");
}

#[test]
fn state_block_heuristics_stay_out_of_initvar_mode() {
    let text = "名單:\n  - 名稱: 火球\n";
    assert_eq!(
        parse_state_fields(text).fields,
        vec![(
            vec!["名單".to_owned(), "名稱".to_owned()],
            "火球".to_owned()
        )]
    );
    assert_list(
        &parse_initvar_tree(text),
        &["名單"],
        json!([{"名稱": "火球"}]),
    );
}
