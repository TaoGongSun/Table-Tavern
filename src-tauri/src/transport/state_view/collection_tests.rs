//! takeover-list-structured：接管桌集合印給 GM 的「目前狀態」——項目照 `_序`／索引數值排、`_序` 不印、
//! 清空的集合照印並註明沒有項目。
use super::{render_state_tree, TreeRender};
use crate::data::{FieldKind, FieldRule, InjectLevel, Mechanism, StateNode};
use std::collections::BTreeMap;

fn tree(value: serde_json::Value) -> BTreeMap<String, StateNode> {
    serde_json::from_value(value).unwrap()
}

/// 北境驿站接管桌：重構開的增量桌，只印規則蓋到的欄位。
fn mechanism() -> Mechanism {
    let mut mechanism = Mechanism {
        incremental: true,
        incremental_by_refactor: true,
        ..Mechanism::default()
    };
    let turn = |kind| FieldRule {
        inject: InjectLevel::Turn,
        ..FieldRule::for_kind(kind)
    };
    for (path, kind) in [
        ("在場人物", FieldKind::List),
        ("在場人物.*.身份", FieldKind::Text),
        ("行動選項", FieldKind::List),
        ("行動選項.*", FieldKind::Text),
        ("地點", FieldKind::Text),
    ] {
        mechanism.rules.insert(path.to_owned(), turn(kind));
    }
    mechanism.collections = serde_json::from_value(serde_json::json!({
        "在場人物": { "keyed": true, "item": "object", "fields": { "身份": "text" } },
        "行動選項": { "keyed": false, "item": "scalar", "type": "text" },
    }))
    .unwrap();
    mechanism
}

fn render(tree: &BTreeMap<String, StateNode>, mechanism: &Mechanism) -> String {
    let changes = BTreeMap::new();
    let mut output = String::new();
    render_state_tree(
        &mut output,
        tree,
        &TreeRender {
            mechanism,
            changes: &changes,
            hidden: &[],
            align: false,
            user_name: "阿濤",
            base: 0,
            en: false,
        },
        &mut Vec::new(),
    );
    output
}

#[test]
fn items_follow_order_leaf_and_index_number_and_hide_the_order_leaf() {
    let options: serde_json::Map<String, serde_json::Value> = (0..12)
        .map(|index| (index.to_string(), serde_json::json!(format!("選項{index}"))))
        .collect();
    let state = tree(serde_json::json!({
        "在場人物": {
            "周掌柜": { "_序": "2", "身份": "军需官" },
            "林远": { "_序": "0", "身份": "新任驿丞" },
            "贺铮": { "_序": "10", "身份": "老驿长" },
        },
        "行動選項": options,
    }));
    let output = render(&state, &mechanism());
    let people = "在場人物：\n  林远：\n    身份：新任驿丞\n  周掌柜：\n    身份：军需官\n  贺铮：\n    身份：老驿长\n";
    assert!(output.starts_with(people), "{output}");
    assert!(!output.contains("_序"));
    let expected: Vec<String> = (0..12)
        .map(|index| format!("{index}：選項{index}"))
        .collect();
    let shown: Vec<String> = output
        .lines()
        .filter(|line| line.contains("：選項"))
        .map(|line| line.trim().to_owned())
        .collect();
    assert_eq!(shown, expected);
}

#[test]
fn emptied_collections_stay_visible_even_when_pruned_from_the_tree() {
    let state = tree(serde_json::json!({ "在場人物": {}, "地點": "前院" }));
    let output = render(&state, &mechanism());
    assert!(output.contains("在場人物：（目前沒有項目）\n"), "{output}");
    // 行動選項整支不在樹上（清空時連分支剪掉）也照印
    assert!(output.contains("行動選項：（目前沒有項目）\n"), "{output}");
    assert!(output.contains("地點：前院\n"));
}

#[test]
fn ordinary_branches_without_visible_leaves_still_disappear() {
    let mut mechanism = mechanism();
    mechanism.collections.clear();
    let state = tree(serde_json::json!({ "在場人物": {}, "地點": "前院" }));
    assert_eq!(render(&state, &mechanism), "地點：前院\n");
}

#[test]
fn nested_collections_stay_visible_when_their_branch_or_ancestors_are_pruned() {
    let mut mechanism = mechanism();
    mechanism.collections = serde_json::from_value(serde_json::json!({
        "營地.人物": { "keyed": true, "item": "object", "fields": { "身份": "text" } },
        "遠征.隊伍.成員": { "keyed": true, "item": "object", "fields": { "身份": "text" } },
    }))
    .unwrap();
    let kept = tree(serde_json::json!({ "營地": {}, "地點": "前院" }));
    assert_eq!(
        render(&kept, &mechanism),
        "地點：前院\n營地：\n  人物：（目前沒有項目）\n遠征：\n  隊伍：\n    成員：（目前沒有項目）\n"
    );
    let empty_present = tree(serde_json::json!({ "營地": { "人物": {} } }));
    assert!(render(&empty_present, &mechanism).starts_with("營地：\n  人物：（目前沒有項目）\n"));
}
