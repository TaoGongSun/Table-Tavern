//! 沒重構 MVU 卡（Upstream）的陣列索引路徑：陣列在狀態樹是一片 JSON 文字葉子，索引路徑（`/HP/0`、
//! `insert /清單/-`、`remove /清單/1`）改在帶型別視圖的 JSON 上照上游 MVU 操作，做完把整個陣列寫回那片葉子。
//! 只有被操作的元素經 `Val` 轉換，其餘元素保留原始 JSON（大整數不經 f64）；非有限數標記隨 splice 位移。
//! 不做 schema 檢查（zod 版 MVU 上游 schema 是 any，本來就不查；舊式 `$meta` 卡另立 mvu-meta-schema）。
use crate::data::message_vars::{clean, leaf_text, parse_json, Json};
use crate::data::{self, Mechanism, StateNode};
use std::collections::BTreeMap;

use super::super::note::{plain, shown_pointer};
use super::super::rules::rule_for;
use super::super::tree::insert_node;
use super::super::types::{Patch, PatchOp, Record, RecordKind};
use super::super::upstream_set::{
    array_index, collect_non_finite, is_vwd, set_non_finite, upstream_set, TypedView, Val,
};
use super::{finite_delta, hint, signed_delta_mark, to_precision_12, vwd_delta_value};

/// 路徑在樹上第一個葉子出現在中途、且視圖在那裡是陣列：回傳陣列欄的段數。
pub(super) fn array_field(
    tree: &BTreeMap<String, StateNode>,
    view: &TypedView,
    path: &[String],
) -> Option<usize> {
    if !view.is_active() {
        return None;
    }
    for end in 1..path.len() {
        match data::node_at(tree, &path[..end]) {
            Some(StateNode::Branch(_)) => continue,
            Some(StateNode::Leaf(_)) => {
                return matches!(view.json_at(&path[..end]), Some(Json::Array(_))).then_some(end);
            }
            None => return None,
        }
    }
    None
}

/// move 的來源或目的進到陣列：上游整筆不套用，這裡拒收並回饋（偏離上游：回饋優於靜默）。
pub(super) fn move_refusal(
    tree: &BTreeMap<String, StateNode>,
    view: &TypedView,
    patch: &Patch,
) -> Option<Record> {
    [&patch.from, &patch.path].into_iter().find_map(|side| {
        let base = array_field(tree, view, side)?;
        Some(Record::new(
            RecordKind::Rejected,
            side.join("."),
            plain(format!(
                "{} 是陣列項目，不能 move；請 remove 後 insert，或 replace {} 寫出整個陣列。",
                shown_pointer(side),
                shown_pointer(&side[..base])
            )),
        ))
    })
}

/// GM 給的值照上游 insert：解析成 JSON 後剝掉 `$meta`、可擴充標記與 `$arrayMeta` 元素（不代換巨集）。
pub(super) fn gm_value(value: Option<&serde_json::Value>) -> Json {
    let json = value
        .and_then(|value| parse_json(&value.to_string()).ok())
        .unwrap_or(Json::Null);
    clean(&json, None)
}

/// 照 stat_data 投影規則轉成樹節點：物件成分支，其餘（含陣列）成一片葉子。
pub(super) fn json_node(value: &Json) -> StateNode {
    match value {
        Json::Object(entries) => StateNode::Branch(
            entries
                .iter()
                .map(|(key, child)| (key.clone(), json_node(child)))
                .collect(),
        ),
        other => StateNode::Leaf(leaf_text(other)),
    }
}

/// 走不到的位置（相對陣列欄的段號 `at`）。
enum Miss {
    /// 上一層是陣列，沒有這個索引
    Index { at: usize, len: usize },
    /// 上一層是物件，沒有這個鍵
    Key { at: usize, keys: Vec<String> },
    /// `rest[..at]` 是單一值，往下沒有子欄位
    Scalar { at: usize },
}

enum Fail {
    Miss(Miss),
    Say(String),
}

/// 這一筆做完的結果：變動標記、額外記錄（夾值）。
type Done = Result<(String, Option<Record>), Fail>;

fn walk<'a>(node: &'a Json, segments: &[String]) -> Result<&'a Json, Miss> {
    let mut node = node;
    for (at, segment) in segments.iter().enumerate() {
        node = match node {
            Json::Array(items) => match array_index(segment).and_then(|index| items.get(index)) {
                Some(child) => child,
                None => {
                    return Err(Miss::Index {
                        at,
                        len: items.len(),
                    })
                }
            },
            Json::Object(entries) => match entries.iter().find(|(key, _)| key == segment) {
                Some((_, child)) => child,
                None => {
                    return Err(Miss::Key {
                        at,
                        keys: entries
                            .iter()
                            .map(|(key, _)| key.clone())
                            .filter(|key| !key.starts_with('_'))
                            .collect(),
                    })
                }
            },
            _ => return Err(Miss::Scalar { at }),
        };
    }
    Ok(node)
}

/// 已確認存在的位置（`walk` 走得到）。
fn node_mut<'a>(node: &'a mut Json, segments: &[String]) -> &'a mut Json {
    let mut node = node;
    for segment in segments {
        node = match node {
            Json::Array(items) => {
                let index = array_index(segment).expect("walk 確認過的索引");
                &mut items[index]
            }
            other => other.get_mut(segment).expect("walk 確認過的鍵"),
        };
    }
    node
}

/// 末段是不是上游 insert／remove 認的數字段（`/^\d+$/`，`01` 也算）。
fn loose_index(segment: &str) -> bool {
    !segment.is_empty() && segment.bytes().all(|byte| byte.is_ascii_digit())
}

/// insert 到物件時的鍵：數字段照上游先解析成數字再轉字串（`01`→`1`）；超過 15 位只去前導 0（罕見，輕防）。
fn object_key(segment: &str) -> String {
    if !loose_index(segment) {
        return segment.to_owned();
    }
    let trimmed = segment.trim_start_matches('0');
    match trimmed {
        "" => "0".to_owned(),
        digits => digits.to_owned(),
    }
}

/// 陣列欄的工作副本：JSON 與其底下的非有限數標記（相對陣列欄）。
struct Work {
    doc: Json,
    marks: Vec<Vec<String>>,
}

impl Work {
    /// 元素的帶型別值：照標記還原批內產生的 NaN／Infinity。
    fn value(&self, json: &Json, at: &[String]) -> Val {
        let mut value = Val::from_json(json);
        for mark in &self.marks {
            if let Some(rest) = mark.strip_prefix(at) {
                set_non_finite(&mut value, rest);
            }
        }
        value
    }

    /// 只改寫一個元素：撤銷它底下的舊標記、記上新值的非有限數。
    fn write(&mut self, at: &[String], value: &Val) {
        *node_mut(&mut self.doc, at) = value.to_json();
        self.marks.retain(|mark| !mark.starts_with(at));
        collect_non_finite(value, &mut at.to_vec(), &mut self.marks);
    }

    /// splice 只移動位置、不改寫元素值：只位移被 splice 那一層容器的直接索引段，被刪元素的標記丟掉。
    fn shift(&mut self, container: &[String], index: usize, inserted: bool) {
        let depth = container.len();
        self.marks.retain_mut(|mark| {
            if mark.len() <= depth || !mark.starts_with(container) {
                return true;
            }
            let Some(at) = array_index(&mark[depth]) else {
                return true;
            };
            match (inserted, at.cmp(&index)) {
                (_, std::cmp::Ordering::Less) => true,
                (true, _) => {
                    mark[depth] = (at + 1).to_string();
                    true
                }
                (false, std::cmp::Ordering::Equal) => false,
                (false, std::cmp::Ordering::Greater) => {
                    mark[depth] = (at - 1).to_string();
                    true
                }
            }
        });
    }
}

/// 照上游套用一筆進到陣列裡的 replace／delta／insert／remove（`base`＝陣列欄段數）。
pub(super) fn apply(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patch: &Patch,
    base: usize,
    view: &mut TypedView,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) {
    let path = &patch.path;
    let field = &path[..base];
    let rest = &path[base..];
    let path_str = path.join(".");
    let rejected =
        |detail: String| Record::new(RecordKind::Rejected, path_str.clone(), plain(detail));
    let Some(StateNode::Leaf(leaf)) = data::node_at(tree, field) else {
        return;
    };
    let field_rule = rule_for(mechanism, field, Some(leaf));
    for (rule, shown) in [
        (field_rule.clone(), shown_pointer(field)),
        (rule_for(mechanism, path, None), shown_pointer(path)),
    ] {
        if let Some(detail) = hint::guard_refusal(&rule, &shown) {
            records.push(rejected(detail));
            return;
        }
    }
    let Some(doc) = view.json_at(field).cloned() else {
        return;
    };
    let mut work = Work {
        doc,
        marks: view.marks_under(field),
    };
    let done = match patch.op {
        PatchOp::Delta => delta(&mut work, mechanism, rest, patch.value.as_ref(), path),
        PatchOp::Replace => replace(&mut work, rest, patch.value.as_ref()),
        PatchOp::Insert => insert(&mut work, rest, patch.value.as_ref(), path, base),
        PatchOp::Remove => remove(&mut work, rest),
        PatchOp::Move => return,
    };
    match done {
        Ok((mark, extra)) => {
            if insert_node(tree, field, StateNode::Leaf(leaf_text(&work.doc))).is_err() {
                return;
            }
            view.replace_json(field, work.doc, work.marks);
            changes.insert(field.join("."), mark);
            records.extend(extra);
        }
        Err(Fail::Say(detail)) => records.push(rejected(detail)),
        Err(Fail::Miss(miss)) => records.push(miss_record(path, base, miss, patch.op)),
    }
}

fn replace(work: &mut Work, rest: &[String], value: Option<&serde_json::Value>) -> Done {
    let old = walk(&work.doc, rest).map_err(Fail::Miss)?;
    let old = work.value(old, rest);
    let new = upstream_set(
        &old,
        Val::from_serde(value.unwrap_or(&serde_json::Value::Null)),
    );
    work.write(rest, &new);
    Ok(("更新".to_owned(), None))
}

fn delta(
    work: &mut Work,
    mechanism: &Mechanism,
    rest: &[String],
    value: Option<&serde_json::Value>,
    path: &[String],
) -> Done {
    let current = walk(&work.doc, rest).map_err(Fail::Miss)?;
    // `delta …/X/0`、X 是數字 [值, 說明]（X 可以是陣列欄本身或巢在裡面）：與 `delta /X` 同一套——照 X 的規則
    // 守門、夾範圍並記夾值（記在 X），寫回 X
    if let Some((last, parent_at)) = rest.split_last() {
        if last == "0" {
            let parent = walk(&work.doc, parent_at).map_err(Fail::Miss)?;
            if let Val::Arr(items) = work.value(parent, parent_at) {
                if is_vwd(&items) && matches!(items[0], Val::Num(_)) {
                    let x = &path[..path.len() - 1];
                    let rule = rule_for(mechanism, x, None);
                    let result = vwd_delta_value(&rule, &shown_pointer(path), &items, value)
                        .map_err(Fail::Say)?;
                    work.write(parent_at, &result.value);
                    // 帶號增量只在 X 就是陣列欄本身時寫（標記記在陣列欄，巢狀的寫「更新」）
                    let mark = if parent_at.is_empty() {
                        signed_delta_mark(result.delta)
                    } else {
                        "更新".to_owned()
                    };
                    return Ok((mark, result.clamped(&x.join("."))));
                }
            }
        }
    }
    let current = work.value(current, rest);
    let shown = shown_pointer(path);
    let not_number = || {
        Fail::Say(format!(
            "{shown} 的值不是數字，不能 delta；請 replace {shown} 寫新值。"
        ))
    };
    let next = match current {
        Val::Num(number) => {
            let delta = finite_delta(value).ok_or_else(|| bad_delta(&shown))?;
            Val::Num(to_precision_12(number + delta))
        }
        // 元素本身是數字 [值, 說明]：照上游加第 0 項、不夾範圍（元素沒有規則）
        Val::Arr(mut items) if is_vwd(&items) && matches!(items[0], Val::Num(_)) => {
            let delta = finite_delta(value).ok_or_else(|| bad_delta(&shown))?;
            if let Val::Num(first) = items[0] {
                items[0] = Val::Num(to_precision_12(first + delta));
            }
            Val::Arr(items)
        }
        _ => return Err(not_number()),
    };
    work.write(rest, &next);
    Ok(("更新".to_owned(), None))
}

fn bad_delta(shown: &str) -> Fail {
    Fail::Say(format!(
        "{shown} 是數字，delta 的值要是純數字增減量（例 5、-10）。"
    ))
}

fn insert(
    work: &mut Work,
    rest: &[String],
    value: Option<&serde_json::Value>,
    path: &[String],
    base: usize,
) -> Done {
    let (last, container_at) = rest.split_last().expect("陣列路徑至少一段");
    let value = gm_value(value);
    let container = walk(&work.doc, container_at).map_err(Fail::Miss)?;
    match container {
        Json::Array(items) => {
            let len = items.len();
            let index = if last == "-" {
                len
            } else if loose_index(last) {
                last.parse::<usize>().map_or(len, |index| index.min(len))
            } else {
                // 上游在陣列上設非索引屬性、序列化後消失（靜默不套用）；這裡拒收並回饋（偏離上游：回饋優於靜默）
                let container = shown_pointer(&path[..base + container_at.len()]);
                return Err(Fail::Say(format!(
                    "{container} 是陣列，insert 的最後一段要寫索引或 -（例 {container}/-）。"
                )));
            };
            work.shift(container_at, index, true);
            if let Json::Array(items) = node_mut(&mut work.doc, container_at) {
                items.insert(index, value);
            }
        }
        Json::Object(_) => {
            let key = object_key(last);
            let mut target = container_at.to_vec();
            target.push(key.clone());
            work.marks.retain(|mark| !mark.starts_with(&target));
            node_mut(&mut work.doc, container_at).insert(&key, value);
        }
        // 上游：容器是 null（上一層已確認是集合）就建空物件再設鍵；批內產生的 NaN 也存成 null，但它是數字、照上游擋下
        Json::Null if !work.marks.iter().any(|mark| mark == container_at) => {
            *node_mut(&mut work.doc, container_at) = Json::Object(vec![(object_key(last), value)]);
        }
        _ => {
            return Err(Fail::Miss(Miss::Scalar {
                at: container_at.len(),
            }))
        }
    }
    Ok(("更新".to_owned(), None))
}

fn remove(work: &mut Work, rest: &[String]) -> Done {
    let (last, container_at) = rest.split_last().expect("陣列路徑至少一段");
    // 上游快速路徑：末段是數字、容器是陣列而且有這一項
    if loose_index(last) {
        if let Ok(Json::Array(items)) = walk(&work.doc, container_at) {
            let len = items.len();
            if let Some(index) = last.parse::<usize>().ok().filter(|index| *index < len) {
                work.shift(container_at, index, false);
                if let Json::Array(items) = node_mut(&mut work.doc, container_at) {
                    items.remove(index);
                }
                return Ok(("更新".to_owned(), None));
            }
        }
    }
    // 其餘照原路徑：有才刪（陣列的情形上面已處理，走得到就一定是物件的鍵，`01` 照原字面刪）
    walk(&work.doc, rest).map_err(Fail::Miss)?;
    node_mut(&mut work.doc, container_at).remove(last);
    work.marks.retain(|mark| !mark.starts_with(rest));
    Ok(("更新".to_owned(), None))
}

/// 走不到時給 GM 的修正提示：不讓它來回打轉，增減項目一律能退到「replace 整個陣列」（上游 set 不查可擴充）。
fn miss_record(path: &[String], base: usize, miss: Miss, op: PatchOp) -> Record {
    let path_str = path.join(".");
    let rejected =
        |detail: String| Record::new(RecordKind::Rejected, path_str.clone(), plain(detail));
    match miss {
        Miss::Index { at, len } => {
            let end = base + at;
            let missing = shown_pointer(&path[..=end]);
            let container = shown_pointer(&path[..end]);
            let count = match len {
                0 => "目前是空的".to_owned(),
                1 => "目前 1 項（索引 0）".to_owned(),
                len => format!("目前 {len} 項（索引 0–{}）", len - 1),
            };
            let appending =
                op == PatchOp::Replace && end + 1 == path.len() && path[end] == len.to_string();
            let advice = if appending {
                format!("新增請用 insert {container}/-，或 replace {container} 寫出整個陣列。")
            } else {
                format!("改項目請 replace {container} 寫出整個陣列。")
            };
            rejected(format!("{missing} 不存在：{container} {count}；{advice}"))
        }
        Miss::Key { at, keys } => hint::missing_group(path, base + at + 1, keys, op),
        Miss::Scalar { at } => {
            let scalar = shown_pointer(&path[..base + at]);
            rejected(format!(
                "{scalar} 是單一值，沒有子欄位；請 replace {scalar}。"
            ))
        }
    }
}
