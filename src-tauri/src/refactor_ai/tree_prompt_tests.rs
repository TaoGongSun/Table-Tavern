//! 既有狀態樹段（takeover-mvu-path-mismatch 改法 2）：樹非空時盤點／接管／合組／介面展開都附上，
//! 寫明路徑鍵逐字沿用、`*` 只准 RULES 鍵；樹空時訊息不變（schema 那句除外）。

use super::*;
use crate::data::StateNode;
use std::collections::BTreeMap;

fn leaf(value: &str) -> StateNode {
    StateNode::Leaf(value.to_owned())
}

fn mvu_tree() -> BTreeMap<String, StateNode> {
    BTreeMap::from([
        (
            "基础信息".to_owned(),
            StateNode::Branch(BTreeMap::from([("时间".to_owned(), leaf("未知"))])),
        ),
        ("冒险者".to_owned(), StateNode::Branch(BTreeMap::new())),
    ])
}

const PRIORITY: &str = "路徑命名優先序";
const NO_WILDCARD_ELSEWHERE: &str =
    "TRIGGERS 的 when.path／flag、STATE 的鍵、SHELL 佔位符一律寫具體的絕對路徑";

#[test]
fn tree_section_lists_leaves_empty_branches_and_priority() {
    assert_eq!(existing_tree_section(&BTreeMap::new()), "");
    let section = existing_tree_section(&mvu_tree());
    assert!(section.contains("- 基础信息.时间\n"));
    assert!(section.contains("- 冒险者（空分支，子鍵由劇情新增）"));
    assert!(section.contains(PRIORITY));
    assert!(section.contains("萬用 `*` 只准用在 RULES 的鍵"));
    assert!(section.contains(NO_WILDCARD_ELSEWHERE));
    assert!(section.contains("不可指到上面列出的既有葉子"));
    assert!(!section.contains("非完整清單"));
}

#[test]
fn tree_section_truncates_but_keeps_every_top_level_key() {
    let mut tree = BTreeMap::new();
    let big: BTreeMap<String, StateNode> = (0..250)
        .map(|index| (format!("房{index:03}"), leaf("x")))
        .collect();
    tree.insert("甲迷宫房间".to_owned(), StateNode::Branch(big));
    tree.insert("乙基础信息".to_owned(), leaf("1"));
    tree.insert("丙冒险者".to_owned(), leaf("1"));
    let section = existing_tree_section(&tree);
    assert_eq!(section.matches("\n- ").count(), 200);
    assert!(section.contains("非完整清單"));
    assert!(section.contains("全部頂層鍵：丙冒险者、乙基础信息、甲迷宫房间"));
}

#[test]
fn every_refactor_stage_carries_the_tree_and_key_exception() {
    let section = existing_tree_section(&mvu_tree());
    let survey = &survey_messages("ctx", &[], "zh-TW", "interface", &section)[1].content;
    let absorb = &absorb_messages("ctx", "1", "全文", &[], &section, "zh-TW")[1].content;
    let group = &group_messages(
        "ctx",
        "機制",
        GroupKind::Mechanism,
        &[("1#s1".to_owned(), "段".to_owned())],
        &[],
        &section,
        "zh-TW",
    )[1]
    .content;
    let expand = &expand_messages(
        "ctx",
        "1",
        "全文",
        EntryKind::InterfaceStatusbar,
        &[],
        &section,
        None,
    )[1]
    .content;
    for content in [survey, absorb, group, expand] {
        assert!(content.contains("- 基础信息.时间"));
        assert!(content.contains(PRIORITY));
        assert!(content.contains(NO_WILDCARD_ELSEWHERE));
        assert!(content.contains("唯獨路徑鍵照「既有狀態樹」段的優先序逐字寫"));
    }
    // 欄位清單空時不再叫 AI 自己用玩家語言命名樹上已有的欄位
    assert!(absorb.contains("狀態樹已有的欄位照下面「既有狀態樹」段逐字沿用"));
}

#[test]
fn empty_tree_leaves_messages_without_tree_text() {
    let survey = &survey_messages("ctx", &[], "zh-TW", "interface", "")[1].content;
    let absorb = &absorb_messages("ctx", "1", "全文", &[], "", "zh-TW")[1].content;
    let expand =
        &expand_messages("ctx", "1", "全文", EntryKind::InterfaceShell, &[], "", None)[1].content;
    for content in [survey, absorb, expand] {
        assert!(!content.contains("既有狀態樹"));
        assert!(!content.contains(PRIORITY));
    }
    assert!(absorb.contains("任何新欄位都由你命名（玩家語言、機器好處理的值）"));
}

#[test]
fn field_schema_uses_full_root_paths() {
    let absorb = &absorb_messages("ctx", "1", "全文", &[], "", "zh-TW")[1].content;
    assert!(absorb.contains("從狀態樹根起算的完整點分路徑"));
    assert!(!absorb.contains("不含分支前綴"));
}
