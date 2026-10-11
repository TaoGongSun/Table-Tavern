//! 路徑寫錯時給 GM 的修正提示：路徑不存在、打到分支、在單一欄位底下寫子路徑。
//! 每一類都先過欄位規則守門（骰值、唯讀照拒收句），守門沒擋才告訴 GM 該怎麼寫。
use crate::data::{self, FieldKind, FieldRule, Mechanism, StateNode, UpdateMode};
use std::collections::BTreeMap;

use super::super::note::{compose, plain, pointer, render_group, shown_pointer, snippet};
use super::super::rules::{leaf_shown, rule_for};
use super::super::types::{NoteGroup, PatchOp, Record, RecordKind};
use super::super::upstream_set::{is_vwd, TypedView, Val};

/// `[值, 說明]` 第 0 項的顯示文字：字串照原文，其餘寫成 JSON，截到 30 字。
pub(super) fn first_item_text(item: &Val) -> String {
    match item {
        Val::Str(text) => snippet(text, 30),
        other => snippet(&other.to_json().to_text(), 30),
    }
}

/// 「路徑不存在」最多列幾個現有鍵。
const MISSING_KEYS: usize = 20;
/// 分支提示最多列幾個子欄位。
const BRANCH_KEYS: usize = 8;

/// 骰值、唯讀（含衍生值）一律先擋：回傳拒收句。`shown` 是句中要印的路徑。
pub(super) fn guard_refusal(rule: &FieldRule, shown: &str) -> Option<String> {
    if rule.update == UpdateMode::Local || rule.kind == FieldKind::Roll {
        return Some(format!("{shown} 由系統本地擲骰，請勿更新。"));
    }
    if rule.update == UpdateMode::Reject
        || matches!(rule.kind, FieldKind::ReadOnly | FieldKind::Derived)
    {
        return Some(format!("{shown} 是唯讀欄位，不接受更新。"));
    }
    None
}

fn rejected(path: &[String], detail: String) -> Record {
    Record::new(RecordKind::Rejected, path.join("."), detail)
}

/// 路徑在樹上走不到（或 insert／move 寫不進去）時的拒收記錄。
pub(super) fn missing(
    tree: &BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    view: &TypedView,
    path: &[String],
    op: PatchOp,
) -> Record {
    let full_rule = rule_for(mechanism, path, None);
    if let Some(detail) = guard_refusal(&full_rule, &shown_pointer(path)) {
        return rejected(path, plain(detail));
    }
    for end in 1..=path.len() {
        let prefix = &path[..end];
        match data::node_at(tree, prefix) {
            Some(StateNode::Branch(_)) => continue,
            Some(StateNode::Leaf(value)) => {
                return leaf_in_the_way(mechanism, view, prefix, value, op);
            }
            None => return missing_prefix(tree, mechanism, path, end, op),
        }
    }
    // 整條路徑都在（呼叫端不該走到這裡）：當成最後一段不存在
    missing_prefix(tree, mechanism, path, path.len(), op)
}

/// 路徑中途撞到葉子：陣列（上游 MVU）＞清單欄＞一般單一欄位。
fn leaf_in_the_way(
    mechanism: &Mechanism,
    view: &TypedView,
    prefix: &[String],
    value: &str,
    op: PatchOp,
) -> Record {
    let shown = shown_pointer(prefix);
    let rule = rule_for(mechanism, prefix, Some(value));
    if let Some(detail) = guard_refusal(&rule, &shown) {
        return rejected(prefix, plain(detail));
    }
    if view.is_active() {
        if let Some(Val::Arr(items)) = view.get(prefix) {
            let detail = if is_vwd(&items) && matches!(items[0], Val::Num(_)) {
                format!(
                    "{shown} 是 [值, 說明] 欄，不能用索引路徑；replace 它只會改第 0 項（值只寫新數字），增減用 delta 它。"
                )
            } else if is_vwd(&items) {
                // 第 0 項不是數字也照上游當 [值, 說明]（例如恰好兩個字串的清單）：照實講 replace 只動第 0 項
                format!(
                    "{shown} 依上游規則視為 [值, 說明]：replace 它只會改第 0 項（目前「{}」）；增刪項目目前不支援。",
                    first_item_text(&items[0])
                )
            } else {
                format!(
                    "{shown} 是陣列，目前不支援逐項路徑；請 replace 它，值寫成整個 JSON 陣列（不要加引號）。"
                )
            };
            return rejected(prefix, plain(detail));
        }
    }
    let detail = match rule.kind {
        FieldKind::List if op == PatchOp::Insert => format!(
            "{shown} 是清單欄，不能逐行插入；改任何一行都用 replace 寫出整個欄位的完整字串（沒變的行也照抄）。"
        ),
        FieldKind::List => format!(
            "{shown} 是清單欄，不能逐項改；改任何一行都用 replace 寫出整個欄位的完整字串（沒變的行也照抄）。"
        ),
        FieldKind::Number | FieldKind::Counter | FieldKind::Pair => {
            format!("{shown} 是單一欄位，沒有子欄位；增減請 delta 它給增減量。")
        }
        _ => format!("{shown} 是單一欄位，沒有子欄位；請 replace 它寫新值。"),
    };
    rejected(prefix, plain(detail))
}

/// `path[..end]` 是第一個不存在的前綴：印前綴、列上一層現有的鍵；同一前綴的多筆在回饋裡合成一條。
fn missing_prefix(
    tree: &BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    path: &[String],
    end: usize,
    op: PatchOp,
) -> Record {
    let prefix = &path[..end];
    let parent = &prefix[..end - 1];
    let siblings = if parent.is_empty() {
        Some(tree)
    } else {
        match data::node_at(tree, parent) {
            Some(StateNode::Branch(children)) => Some(children),
            _ => None,
        }
    };
    let items = siblings
        .map(|children| listable_keys(mechanism, children, parent))
        .unwrap_or_default();
    let last_only = end == path.len();
    let insert_hint = last_only && matches!(op, PatchOp::Replace | PatchOp::Delta);
    let head = format!("路徑不存在：{}", shown_pointer(prefix));
    let intro = if parent.is_empty() {
        "。頂層現有："
    } else {
        "。上一層現有："
    };
    // 只缺最後一段：同一層的多筆新鍵（同類 op）合成一條；中間層缺：同一個不存在的前綴合成一條
    let (key, merged_head, merged_intro) = if last_only {
        let place = if parent.is_empty() {
            "頂層".to_owned()
        } else {
            shown_pointer(parent)
        };
        (
            format!("L\u{0}{}\u{0}{insert_hint}", pointer(parent)),
            format!("路徑不存在：{place} 底下沒有這些欄位"),
            "。現有：",
        )
    } else {
        (format!("M\u{0}{}", pointer(prefix)), head.clone(), intro)
    };
    let group = NoteGroup {
        key,
        head,
        intro: intro.to_owned(),
        merged_head,
        merged_intro: merged_intro.to_owned(),
        items,
        max_items: MISSING_KEYS,
        tail: if insert_hint {
            "。若要新增請用 insert。".to_owned()
        } else {
            "。".to_owned()
        },
        leaf: path.last().cloned().unwrap_or_default(),
    };
    let mut record = rejected(
        path,
        render_group(&group, std::slice::from_ref(&group.leaf)),
    );
    record.group = Some(Box::new(group));
    record
}

/// 對分支下了只對單一欄位有效的操作（增減、替換成單一值）。
pub(super) fn branch(
    mechanism: &Mechanism,
    path: &[String],
    children: &BTreeMap<String, StateNode>,
    verb: &str,
) -> Record {
    let shown = shown_pointer(path);
    let rule = rule_for(mechanism, path, None);
    if let Some(detail) = guard_refusal(&rule, &shown) {
        return rejected(path, plain(detail));
    }
    let items = listable_keys(mechanism, children, path);
    let detail = compose(
        &format!("{shown} 是分支，不能{verb}"),
        "；請改底下的欄位：",
        &items,
        BRANCH_KEYS,
        "。",
    );
    rejected(path, detail)
}

/// 列給 GM 看的子鍵：略過 `_` 開頭；增量桌照渲染規則略過 Rare 葉子與整支不顯示的分支。
fn listable_keys(
    mechanism: &Mechanism,
    children: &BTreeMap<String, StateNode>,
    parent: &[String],
) -> Vec<String> {
    let mut path = parent.to_vec();
    children
        .iter()
        .filter(|(key, _)| !key.starts_with('_'))
        .filter(|(key, node)| {
            path.push((*key).clone());
            let shown = node_shown(mechanism, node, &mut path);
            path.pop();
            shown
        })
        .map(|(key, _)| key.clone())
        .collect()
}

fn node_shown(mechanism: &Mechanism, node: &StateNode, path: &mut Vec<String>) -> bool {
    if !mechanism.incremental {
        return true;
    }
    match node {
        StateNode::Leaf(value) => leaf_shown(mechanism, path, value, true),
        StateNode::Branch(children) => children.iter().any(|(key, child)| {
            path.push(key.clone());
            let shown = node_shown(mechanism, child, path);
            path.pop();
            shown
        }),
    }
}
