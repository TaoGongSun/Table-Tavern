//! MVU 機制格式：解析模型吐出的 `<UpdateVariable>` JSON Patch，依欄位規則本地決定收不收。
//! 模型只說「這一幕變動多少」，加減／夾邊界／擲骰全在這裡做——模型算數字會幻覺，
//! 本地帳才是真相。容錯是紅線：格式壞的那一筆丟掉沿用舊值，絕不 panic、絕不中斷整批更新。

use crate::data::{
    self, Condition, FieldKind, FieldRule, Mechanism, StateNode, TriggerMode, UpdateMode,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// 全量桌跳動標記的門檻：兩個條件都要達到才算「跳」，寧可少標也不要一直誤報
/// （模型每回合都會有些許措辭差異造成的小數字漂移，不該被當成幻覺）。
/// 絕對幅度：小數值欄位（如個位數好感度）漲跌幾點很正常，不到 30 不算異常。
const JUMP_ABS_THRESHOLD: f64 = 30.0;
/// 相對幅度：大數值欄位（如上千的聲望）漲跌 30 只是零頭，要佔舊值／新值中較大者的四成才算異常。
const JUMP_RATIO_THRESHOLD: f64 = 0.4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchOp {
    Replace,
    Delta,
    Insert,
    Remove,
    Move,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Patch {
    pub op: PatchOp,
    pub path: Vec<String>,
    /// 只有 Move 用得到；其餘 op 固定空。
    pub from: Vec<String>,
    pub value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordKind {
    Rejected,
    Clamped,
    Error,
    /// 機制鷹架條目（[initvar]／[mvu_update]／整棵樹重送巨集）已被系統接管，不再送模型。
    Absorbed,
    /// 卡片腳本認不出來（隨機事件庫、要跑迴圈統計的判定等），沒轉成觸發表，預設也不送模型。
    Skipped,
    /// 全量桌跳動警示：這一欄一回合內變動幅度超過保守門檻，疑似模型算錯。只給玩家看，
    /// `build_notes` 不理它——不是拒收，沒有東西要模型改。
    Jump,
}

/// 一筆記帳，供面板列「哪些更新被擋下」用。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub kind: RecordKind,
    pub path: String,
    pub detail: String,
}

impl Record {
    fn new(kind: RecordKind, path: String, detail: String) -> Self {
        Self { kind, path, detail }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Outcome {
    pub records: Vec<Record>,
    /// 自癒回饋句：只從 Rejected 記錄產生，給下一輪模型看該怎麼改。
    pub notes: Vec<String>,
    /// 這一輪真的改到樹的變動：路徑（點分）→ 顯示標記。被拒收／硬錯誤不進來，
    /// 骰值本地重擲也不算（狀態欄二期包 5：回合尾注入策略要靠這個標「哪裡變了」）。
    pub changes: BTreeMap<String, String>,
}

// ---------------------------------------------------------------------
// 解析：<UpdateVariable> 原文 → Vec<Patch>
// ---------------------------------------------------------------------

/// 從 `<UpdateVariable>` 原始內容取出更新指令；解析不出來就回空 Vec，絕不 panic。
pub fn parse_updates(block: &str) -> Vec<Patch> {
    let without_analysis = strip_analysis(block);
    let section = extract_json_patch_section(&without_analysis);
    let candidate = strip_code_fences(&section);
    let candidate = candidate.trim();

    let objects: Vec<serde_json::Value> =
        match serde_json::from_str::<Vec<serde_json::Value>>(candidate) {
            Ok(values) => values,
            // 模型漏逗號等格式壞掉：退回逐個掃出最外層平衡的 {…} 物件，好的照收壞的跳過。
            Err(_) => scan_balanced_objects(candidate)
                .into_iter()
                .filter_map(|text| serde_json::from_str::<serde_json::Value>(text).ok())
                .collect(),
        };

    objects.iter().filter_map(value_to_patch).collect()
}

fn strip_analysis(block: &str) -> String {
    let lower = block.to_ascii_lowercase();
    let (Some(start), Some(open_end)) = (
        lower.find("<analysis"),
        lower
            .find("<analysis")
            .and_then(|start| lower[start..].find('>').map(|offset| start + offset)),
    ) else {
        return block.to_owned();
    };
    let Some(close_start) = lower[open_end + 1..]
        .find("</analysis>")
        .map(|offset| open_end + 1 + offset)
    else {
        return block.to_owned();
    };
    let close_end = close_start + "</analysis>".len();
    format!("{}{}", &block[..start], &block[close_end..])
}

fn extract_json_patch_section(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    if let Some(start) = lower.find("<jsonpatch") {
        if let Some(open_end) = lower[start..].find('>').map(|offset| start + offset) {
            if let Some(close_start) = lower[open_end + 1..]
                .find("</jsonpatch>")
                .map(|offset| open_end + 1 + offset)
            {
                return text[open_end + 1..close_start].to_owned();
            }
        }
    }
    text.to_owned()
}

/// 剝掉包住整段候選文字的 ``` 圍欄（含 ```json 這種帶語言標記的開頭）。
fn strip_code_fences(text: &str) -> String {
    let trimmed = text.trim();
    let Some(rest) = trimmed.strip_prefix("```") else {
        return trimmed.to_owned();
    };
    let rest = match rest.find('\n') {
        Some(index) => &rest[index + 1..],
        None => rest,
    };
    rest.strip_suffix("```").unwrap_or(rest).trim().to_owned()
}

/// 逐個掃出最外層平衡的 `{…}` 物件，正確跳過字串內的引號與跳脫字元。
fn scan_balanced_objects(text: &str) -> Vec<&str> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut result = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index].1 != '{' {
            index += 1;
            continue;
        }
        let start = chars[index].0;
        let mut depth = 0i32;
        let mut in_string = false;
        let mut escape = false;
        let mut end = None;
        let mut cursor = index;
        while cursor < chars.len() {
            let (byte_pos, character) = chars[cursor];
            if in_string {
                if escape {
                    escape = false;
                } else if character == '\\' {
                    escape = true;
                } else if character == '"' {
                    in_string = false;
                }
            } else {
                match character {
                    '"' => in_string = true,
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(byte_pos + character.len_utf8());
                            break;
                        }
                    }
                    _ => {}
                }
            }
            cursor += 1;
        }
        match end {
            Some(end) => {
                result.push(&text[start..end]);
                index = cursor + 1;
            }
            None => break, // 沒收尾：後面已經壞掉，寧可少收也不要瞎猜
        }
    }
    result
}

fn value_to_patch(value: &serde_json::Value) -> Option<Patch> {
    let obj = value.as_object()?;
    let op = match obj.get("op")?.as_str()? {
        "replace" => PatchOp::Replace,
        "delta" => PatchOp::Delta,
        "insert" => PatchOp::Insert,
        "remove" => PatchOp::Remove,
        "move" => PatchOp::Move,
        _ => return None,
    };
    if op == PatchOp::Move {
        let from = obj
            .get("from")
            .and_then(|v| v.as_str())
            .map(parse_pointer)?;
        let path = obj.get("to").and_then(|v| v.as_str()).map(parse_pointer)?;
        if from.is_empty() || path.is_empty() {
            return None;
        }
        return Some(Patch {
            op,
            path,
            from,
            value: None,
        });
    }
    let path = obj
        .get("path")
        .and_then(|v| v.as_str())
        .map(parse_pointer)?;
    if path.is_empty() {
        return None;
    }
    Some(Patch {
        op,
        path,
        from: Vec::new(),
        value: obj.get("value").cloned(),
    })
}

/// JSON Pointer 拆段：`~1`→`/`、`~0`→`~`，開頭的 `/` 忽略；
/// 容錯：整串沒有 `/` 但有 `.` 時改用 `.` 拆（模型有時給點分路徑）。
fn parse_pointer(raw: &str) -> Vec<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Vec::new();
    }
    if !raw.contains('/') && raw.contains('.') {
        return raw
            .split('.')
            .map(str::to_owned)
            .filter(|segment| !segment.is_empty())
            .collect();
    }
    let raw = raw.strip_prefix('/').unwrap_or(raw);
    if raw.is_empty() {
        return Vec::new();
    }
    raw.split('/')
        .map(|segment| segment.replace("~1", "/").replace("~0", "~"))
        .collect()
}

// ---------------------------------------------------------------------
// 套用：依欄位規則把 Patch 套進狀態樹
// ---------------------------------------------------------------------

/// 依欄位規則把更新套進狀態樹，回傳記帳、下一輪要給模型的自癒回饋句、以及這一輪真的改到樹的變動。
pub fn apply_updates(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patches: &[Patch],
) -> Outcome {
    let mut records = Vec::new();
    let mut changes = BTreeMap::new();
    for patch in patches {
        apply_one(tree, mechanism, patch, &mut records, &mut changes);
    }
    let notes = build_notes(&records);
    Outcome {
        records,
        notes,
        changes,
    }
}

fn apply_one(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patch: &Patch,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) {
    if let Some(offending) = readonly_violation(patch) {
        records.push(Record::new(
            RecordKind::Rejected,
            offending.clone(),
            format!("{offending} 是唯讀欄位（底線開頭），不接受更新。"),
        ));
        return;
    }
    match patch.op {
        PatchOp::Delta => apply_delta(tree, mechanism, patch, records, changes),
        PatchOp::Replace => apply_replace(tree, mechanism, patch, records, changes),
        PatchOp::Insert => apply_insert(tree, mechanism, patch, records, changes),
        PatchOp::Remove => apply_remove(tree, patch, records, changes),
        PatchOp::Move => apply_move(tree, patch, records, changes),
    }
}

/// delta 的變動標記：帶號數字，正數補 `+`（負數 `format_num` 本身就帶 `-`）。
/// 標記照原始 delta 寫——被夾邊界也算改到，記的是模型要求的量，不是夾完的量。
fn signed_delta_mark(delta: f64) -> String {
    if delta >= 0.0 {
        format!("+{}", format_num(delta))
    } else {
        format_num(delta)
    }
}

fn readonly_violation(patch: &Patch) -> Option<String> {
    if patch.path.iter().any(|segment| segment.starts_with('_')) {
        return Some(patch.path.join("."));
    }
    if patch.op == PatchOp::Move && patch.from.iter().any(|segment| segment.starts_with('_')) {
        return Some(patch.from.join("."));
    }
    None
}

fn apply_delta(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patch: &Patch,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) {
    let path_str = patch.path.join(".");
    let Some(node) = data::node_at(tree, &patch.path) else {
        records.push(Record::new(
            RecordKind::Error,
            path_str.clone(),
            format!("路徑不存在：{path_str}"),
        ));
        return;
    };
    let Some(current) = leaf_value(node).map(str::to_owned) else {
        records.push(Record::new(
            RecordKind::Error,
            path_str.clone(),
            format!("{path_str} 是分支，不能做增減。"),
        ));
        return;
    };
    let Some(delta) = value_as_f64(patch.value.as_ref()) else {
        records.push(Record::new(
            RecordKind::Error,
            path_str.clone(),
            format!("{path_str} 的更新值不是數字。"),
        ));
        return;
    };
    let rule = rule_for(mechanism, &patch.path, Some(&current));
    match rule.update {
        UpdateMode::Local => {
            records.push(Record::new(
                RecordKind::Rejected,
                path_str.clone(),
                format!("{path_str} 由系統本地擲骰，請勿更新。"),
            ));
            return;
        }
        UpdateMode::Reject => {
            records.push(Record::new(
                RecordKind::Rejected,
                path_str.clone(),
                format!("{path_str} 是唯讀欄位，不接受更新。"),
            ));
            return;
        }
        UpdateMode::Delta | UpdateMode::Replace => {}
    }
    match rule.kind {
        FieldKind::Pair => {
            let Some((current_value, max)) = split_pair(&current) else {
                records.push(Record::new(
                    RecordKind::Error,
                    path_str.clone(),
                    format!("{path_str} 現值格式不是「現值/上限」。"),
                ));
                return;
            };
            let min = rule.min.unwrap_or(0.0);
            let raw_next = current_value + delta;
            let next = raw_next.clamp(min, max);
            data::set_tree_value(
                tree,
                &patch.path,
                &format!("{}/{}", format_num(next), format_num(max)),
            );
            changes.insert(path_str.clone(), signed_delta_mark(delta));
            if next != raw_next {
                records.push(Record::new(
                    RecordKind::Clamped,
                    path_str.clone(),
                    format!(
                        "{path_str} 已夾在範圍內，目前值 {}/{}。",
                        format_num(next),
                        format_num(max)
                    ),
                ));
            }
        }
        FieldKind::Number | FieldKind::Counter => {
            let Ok(current_value) = current.trim().parse::<f64>() else {
                records.push(Record::new(
                    RecordKind::Error,
                    path_str.clone(),
                    format!("{path_str} 現值不是數字，無法增減。"),
                ));
                return;
            };
            let mut next = current_value + delta;
            let mut clamped = false;
            if let Some(min) = rule.min {
                if next < min {
                    next = min;
                    clamped = true;
                }
            }
            if let Some(max) = rule.max {
                if next > max {
                    next = max;
                    clamped = true;
                }
            }
            data::set_tree_value(tree, &patch.path, &format_num(next));
            changes.insert(path_str.clone(), signed_delta_mark(delta));
            if clamped {
                records.push(Record::new(
                    RecordKind::Clamped,
                    path_str.clone(),
                    format!("{path_str} 已夾在範圍內，目前值 {}。", format_num(next)),
                ));
            }
        }
        FieldKind::Text
        | FieldKind::List
        | FieldKind::Roll
        | FieldKind::ReadOnly
        | FieldKind::Derived => {
            records.push(Record::new(
                RecordKind::Error,
                path_str.clone(),
                format!("{path_str} 是文字欄位，不能做增減。"),
            ));
        }
    }
}

fn apply_replace(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patch: &Patch,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) {
    let path_str = patch.path.join(".");
    let Some(node) = data::node_at(tree, &patch.path) else {
        records.push(Record::new(
            RecordKind::Error,
            path_str.clone(),
            format!("路徑不存在：{path_str}"),
        ));
        return;
    };
    let current_leaf = leaf_value(node).map(str::to_owned);
    replace_existing(
        tree,
        mechanism,
        &patch.path,
        path_str,
        current_leaf,
        patch.value.as_ref(),
        records,
        changes,
    );
}

/// Replace 與「Insert 目標已存在」共用的規則：一律照 Replace 的語意走。
#[allow(clippy::too_many_arguments)]
fn replace_existing(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    path: &[String],
    path_str: String,
    current_leaf: Option<String>,
    value: Option<&serde_json::Value>,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) {
    let rule = rule_for(mechanism, path, current_leaf.as_deref());
    match rule.update {
        UpdateMode::Replace => {
            let node = json_to_node(value.unwrap_or(&serde_json::Value::Null));
            if insert_node(tree, path, node).is_ok() {
                changes.insert(path_str.clone(), "更新".to_owned());
            }
        }
        UpdateMode::Local => {
            records.push(Record::new(
                RecordKind::Rejected,
                path_str.clone(),
                format!("{path_str} 由系統本地擲骰，請勿更新。"),
            ));
        }
        UpdateMode::Reject => {
            records.push(Record::new(
                RecordKind::Rejected,
                path_str.clone(),
                format!("{path_str} 是唯讀欄位，不接受更新。"),
            ));
        }
        UpdateMode::Delta if rule.kind == FieldKind::Pair => {
            let Some(current) = current_leaf else {
                records.push(Record::new(
                    RecordKind::Error,
                    path_str.clone(),
                    format!("{path_str} 是分支，不能替換。"),
                ));
                return;
            };
            let Some((current_value, max)) = split_pair(&current) else {
                records.push(Record::new(
                    RecordKind::Error,
                    path_str.clone(),
                    format!("{path_str} 現值格式不是「現值/上限」。"),
                ));
                return;
            };
            let Some((new_value, new_max)) = value.and_then(|v| v.as_str()).and_then(split_pair)
            else {
                records.push(Record::new(
                    RecordKind::Rejected,
                    path_str.clone(),
                    format!("{path_str} 新值不是「現值/上限」格式，已忽略。"),
                ));
                return;
            };
            if new_value != current_value {
                records.push(Record::new(
                    RecordKind::Rejected,
                    path_str.clone(),
                    format!(
                        "{path_str} 現值 {}/{}，請用增減量（delta）而不是絕對值。",
                        format_num(current_value),
                        format_num(max)
                    ),
                ));
            }
            if new_max != max {
                data::set_tree_value(
                    tree,
                    path,
                    &format!("{}/{}", format_num(current_value), format_num(new_max)),
                );
                changes.insert(path_str.clone(), "更新".to_owned());
            }
        }
        UpdateMode::Delta => {
            let current_display = current_leaf.unwrap_or_default();
            records.push(Record::new(
                RecordKind::Rejected,
                path_str.clone(),
                format!("{path_str} 現值 {current_display}，請用增減量（delta）而不是絕對值。"),
            ));
        }
    }
}

fn apply_insert(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patch: &Patch,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) {
    let path_str = patch.path.join(".");
    if let Some(node) = data::node_at(tree, &patch.path) {
        let current_leaf = leaf_value(node).map(str::to_owned);
        replace_existing(
            tree,
            mechanism,
            &patch.path,
            path_str,
            current_leaf,
            patch.value.as_ref(),
            records,
            changes,
        );
        return;
    }
    let node = json_to_node(patch.value.as_ref().unwrap_or(&serde_json::Value::Null));
    if insert_node(tree, &patch.path, node).is_ok() {
        changes.insert(path_str, "更新".to_owned());
    } else {
        records.push(Record::new(
            RecordKind::Error,
            path_str.clone(),
            format!("{path_str} 中間層已被其他欄位占用，無法建立。"),
        ));
    }
}

fn apply_remove(
    tree: &mut BTreeMap<String, StateNode>,
    patch: &Patch,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) {
    let path_str = patch.path.join(".");
    if data::node_at(tree, &patch.path).is_none() {
        records.push(Record::new(
            RecordKind::Error,
            path_str.clone(),
            format!("路徑不存在，無法刪除：{path_str}"),
        ));
        return;
    }
    // 空字串＝刪除，並沿用 set_tree_value 既有的「因此變空的父分支一併剪掉」行為。
    data::set_tree_value(tree, &patch.path, "");
    changes.insert(path_str, "移除".to_owned());
}

fn apply_move(
    tree: &mut BTreeMap<String, StateNode>,
    patch: &Patch,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) {
    let from_str = patch.from.join(".");
    let Some(node) = take_node(tree, &patch.from) else {
        records.push(Record::new(
            RecordKind::Error,
            from_str.clone(),
            format!("路徑不存在：{from_str}"),
        ));
        return;
    };
    let path_str = patch.path.join(".");
    if insert_node(tree, &patch.path, node.clone()).is_ok() {
        changes.insert(path_str, "搬移".to_owned());
    } else {
        let _ = insert_node(tree, &patch.from, node); // 寫不進去就放回原位，不憑空丟資料
        records.push(Record::new(
            RecordKind::Error,
            path_str.clone(),
            format!("{path_str} 中間層已被其他欄位占用，搬移失敗。"),
        ));
    }
}

fn build_notes(records: &[Record]) -> Vec<String> {
    let mut notes = Vec::new();
    for record in records {
        if record.kind != RecordKind::Rejected {
            continue;
        }
        if notes.contains(&record.detail) {
            continue;
        }
        notes.push(record.detail.clone());
        if notes.len() >= 5 {
            break;
        }
    }
    notes
}

// ---------------------------------------------------------------------
// 樹操作小工具（本檔自用，data::set_tree_value 只覆蓋葉子字串的情形）
// ---------------------------------------------------------------------

fn leaf_value(node: &StateNode) -> Option<&str> {
    match node {
        StateNode::Leaf(value) => Some(value),
        StateNode::Branch(_) => None,
    }
}

/// 在 path 寫入任意節點（含整棵子樹），缺的中間層自動補成 Branch；
/// 撞到既有葉子擋路就放棄，回傳 Err 不動樹。路徑最後一段是 `-` 時，
/// 在該 Branch 用「目前沒被用掉的最小非負整數」當 key 附加。
fn insert_node(
    branch: &mut BTreeMap<String, StateNode>,
    path: &[String],
    node: StateNode,
) -> Result<(), ()> {
    let (first, rest) = path.split_first().ok_or(())?;
    if rest.is_empty() {
        let key = if first == "-" {
            next_free_key(branch)
        } else {
            first.clone()
        };
        branch.insert(key, node);
        return Ok(());
    }
    let entry = branch
        .entry(first.clone())
        .or_insert_with(|| StateNode::Branch(BTreeMap::new()));
    match entry {
        StateNode::Branch(children) => insert_node(children, rest, node),
        StateNode::Leaf(_) => Err(()),
    }
}

fn next_free_key(branch: &BTreeMap<String, StateNode>) -> String {
    let mut candidate = 0u64;
    loop {
        let key = candidate.to_string();
        if !branch.contains_key(&key) {
            return key;
        }
        candidate += 1;
    }
}

/// 取出並刪除 path 上的節點，因此變空的父分支一併剪掉。
fn take_node(branch: &mut BTreeMap<String, StateNode>, path: &[String]) -> Option<StateNode> {
    let (first, rest) = path.split_first()?;
    if rest.is_empty() {
        return branch.remove(first);
    }
    let StateNode::Branch(children) = branch.get_mut(first)? else {
        return None;
    };
    let removed = take_node(children, rest)?;
    if let Some(StateNode::Branch(children)) = branch.get(first) {
        if children.is_empty() {
            branch.remove(first);
        }
    }
    Some(removed)
}

/// JSON 值轉狀態樹節點：物件→Branch（遞迴）、陣列→以 0/1/… 為 key 的 Branch、
/// 純量→Leaf 字串（整數不印成 5.0）。
fn json_to_node(value: &serde_json::Value) -> StateNode {
    match value {
        serde_json::Value::Object(map) => StateNode::Branch(
            map.iter()
                .map(|(k, v)| (k.clone(), json_to_node(v)))
                .collect(),
        ),
        serde_json::Value::Array(items) => StateNode::Branch(
            items
                .iter()
                .enumerate()
                .map(|(index, item)| (index.to_string(), json_to_node(item)))
                .collect(),
        ),
        serde_json::Value::String(text) => StateNode::Leaf(text.clone()),
        serde_json::Value::Number(number) => StateNode::Leaf(format_json_number(number)),
        serde_json::Value::Bool(flag) => StateNode::Leaf(flag.to_string()),
        serde_json::Value::Null => StateNode::Leaf(String::new()),
    }
}

fn format_json_number(number: &serde_json::Number) -> String {
    if let Some(value) = number.as_i64() {
        return value.to_string();
    }
    if let Some(value) = number.as_u64() {
        return value.to_string();
    }
    number.to_string()
}

fn format_num(value: f64) -> String {
    if value.is_finite() && value == value.trunc() && value.abs() < 1e15 {
        (value as i64).to_string()
    } else {
        format!("{value}")
    }
}

fn split_pair(value: &str) -> Option<(f64, f64)> {
    let mut parts = value.splitn(2, '/');
    let current = parts.next()?.trim().parse::<f64>().ok()?;
    let max = parts.next()?.trim().parse::<f64>().ok()?;
    Some((current, max))
}

fn value_as_f64(value: Option<&serde_json::Value>) -> Option<f64> {
    match value? {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

/// `rule_for` 的公開包裝，給 `transport.rs` 依路徑取欄位規則（渲染狀態樹要知道 inject 層級）。
pub fn rule_for_path(mechanism: &Mechanism, path: &[String], current: Option<&str>) -> FieldRule {
    rule_for(mechanism, path, current)
}

/// 找欄位規則：先精確比對 path 的點分路徑，沒有就找同段數、每段相同或為 `*`
/// 的萬用規則（多筆命中取萬用段最少的那筆），都沒有就依現值形狀推定 kind。
fn rule_for(mechanism: &Mechanism, path: &[String], current: Option<&str>) -> FieldRule {
    let key = path.join(".");
    if let Some(rule) = mechanism.rules.get(&key) {
        return rule.clone();
    }
    if let Some(rule) = wildcard_rule(mechanism, path) {
        return rule.clone();
    }
    let kind = match current {
        Some(value) if split_pair(value).is_some() => FieldKind::Pair,
        Some(value) if value.trim().parse::<f64>().is_ok() => FieldKind::Number,
        _ => FieldKind::Text,
    };
    FieldRule::for_kind(kind)
}

fn wildcard_rule<'a>(mechanism: &'a Mechanism, path: &[String]) -> Option<&'a FieldRule> {
    let mut best: Option<(usize, &FieldRule)> = None;
    for (rule_path, rule) in &mechanism.rules {
        let segments: Vec<&str> = rule_path.split('.').collect();
        if segments.len() != path.len() {
            continue;
        }
        let mut wildcard_count = 0usize;
        let matched = segments.iter().zip(path.iter()).all(|(segment, actual)| {
            if *segment == "*" {
                wildcard_count += 1;
                true
            } else {
                segment == actual
            }
        });
        if matched && best.is_none_or(|(count, _)| wildcard_count < count) {
            best = Some((wildcard_count, rule));
        }
    }
    best.map(|(_, rule)| rule)
}

// ---------------------------------------------------------------------
// 骰值欄每回合本地重擲
// ---------------------------------------------------------------------

/// 骰值欄（kind == Roll）每回合本地重擲一次真隨機，寫回樹裡。
pub fn reroll(tree: &mut BTreeMap<String, StateNode>, mechanism: &Mechanism) {
    let mut path = Vec::new();
    reroll_branch(tree, mechanism, &mut path);
}

fn reroll_branch(
    branch: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    path: &mut Vec<String>,
) {
    let keys: Vec<String> = branch.keys().cloned().collect();
    for key in keys {
        path.push(key.clone());
        let is_leaf = matches!(branch.get(&key), Some(StateNode::Leaf(_)));
        if is_leaf {
            let rule = rule_for(mechanism, path, None);
            if rule.kind == FieldKind::Roll {
                let min = rule.min.unwrap_or(1.0);
                let max = rule.max.unwrap_or(100.0);
                branch.insert(
                    key.clone(),
                    StateNode::Leaf(random_int_in_range(min, max).to_string()),
                );
            }
        } else if let Some(StateNode::Branch(children)) = branch.get_mut(&key) {
            reroll_branch(children, mechanism, path);
        }
        path.pop();
    }
}

fn random_int_in_range(min: f64, max: f64) -> i64 {
    let low = min.round() as i64;
    let high = max.round() as i64;
    if high <= low {
        return low;
    }
    let span = (high - low + 1) as u128;
    let random_bits = ulid::Ulid::generate().random();
    low + (random_bits % span) as i64
}

// ---------------------------------------------------------------------
// 衍生值欄（kind == Derived）每回合本地重算：公式以整棵樹當取值來源，
// 算出來的數字寫回它自己那個葉子。只重算樹上已經有這個葉子的欄位——
// 跟 Roll 一樣，路徑要先存在（初始樹或手動建立），這裡不會平白生出新分支。
// ---------------------------------------------------------------------

/// 先只讀一輪收集「路徑＋公式」，算完再一次寫回去——同一棵樹不能又借給
/// 取值用的 closure、又借去改，兩階段分開才過得了借用檢查。
pub fn recompute_derived(tree: &mut BTreeMap<String, StateNode>, mechanism: &Mechanism) -> Vec<Record> {
    let mut targets = Vec::new();
    collect_derived_targets(tree, mechanism, &mut Vec::new(), &mut targets);
    if targets.is_empty() {
        return Vec::new();
    }
    let mut records = Vec::new();
    let mut writes = Vec::new();
    {
        let lookup = tree_lookup(tree);
        for (path, formula) in &targets {
            let path_str = path.join(".");
            match crate::evaluator::eval(formula, &lookup) {
                Ok(value) => writes.push((path.clone(), format_num(value))),
                Err(error) => records.push(Record::new(
                    RecordKind::Error,
                    path_str.clone(),
                    format!("{path_str} 的衍生公式算不出來：{error}"),
                )),
            }
        }
    }
    for (path, value) in writes {
        data::set_tree_value(tree, &path, &value);
    }
    records
}

fn collect_derived_targets(
    branch: &BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    path: &mut Vec<String>,
    targets: &mut Vec<(Vec<String>, String)>,
) {
    for (key, node) in branch {
        path.push(key.clone());
        match node {
            StateNode::Leaf(_) => {
                let rule = rule_for(mechanism, path, None);
                if rule.kind == FieldKind::Derived {
                    if let Some(formula) = rule.formula {
                        targets.push((path.clone(), formula));
                    }
                }
            }
            StateNode::Branch(children) => {
                collect_derived_targets(children, mechanism, path, targets)
            }
        }
        path.pop();
    }
}

/// 公式取值來源：點分路徑在樹上查葉子，數字抽取沿用 `numeric_value`——
/// 現值/上限對取現值、帶前後綴的文字抽第一段數字，跟全量桌跳動比對同一套規則。
fn tree_lookup(tree: &BTreeMap<String, StateNode>) -> impl Fn(&str) -> Option<f64> + '_ {
    move |path: &str| leaf_at(tree, path).and_then(numeric_value)
}

// ---------------------------------------------------------------------
// 觸發表：每回合本地求值（卡片原本用 EJS 腳本做的關係階段／環境氛圍／
// 一次性國家事件，改成資料化條件比對，命中的那段文本才送模型，劇透原文留在本機）
// ---------------------------------------------------------------------

/// 觸發表求值輸出：這輪命中的文本（trigger id → 文本）與要釘死的一次性旗標路徑
/// （呼叫端負責把 flags 寫進樹並記帳，這裡只管求值，不碰樹）。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TriggerOutcome {
    pub hits: BTreeMap<String, String>,
    pub flags: Vec<String>,
}

/// 逐個 Trigger 依序掃 cases，第一個所有 `when` 都成立的就停（空 `when` 一定成立，
/// 當 else 兜底）；沒有任何 case 命中＝這個 trigger 這輪沒有文本。命中文本＝
/// `preamble`（非空時）＋空行＋case 文本，換完 `{{state:路徑}}` 佔位再過一次
/// `{{user}}` 代換。`Once` 命中則把它的 flag 路徑收進輸出，由呼叫端釘進樹。
pub fn evaluate_triggers(
    tree: &BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    user_name: &str,
) -> TriggerOutcome {
    let mut outcome = TriggerOutcome::default();
    for trigger in &mechanism.triggers {
        let Some(case) = trigger.cases.iter().find(|case| {
            case.when
                .iter()
                .all(|condition| condition_holds(tree, condition))
        }) else {
            continue;
        };
        let mut text = case.text.clone();
        if !trigger.preamble.is_empty() {
            text = format!("{}\n\n{text}", trigger.preamble);
        }
        let text = resolve_state_placeholders(&text, tree);
        let text = crate::transport::replace_st_macros(&text, user_name, None);
        outcome.hits.insert(trigger.id.clone(), text);
        if trigger.mode == TriggerMode::Once {
            if let Some(flag) = &trigger.flag {
                outcome.flags.push(flag.clone());
            }
        }
    }
    outcome
}

/// 一段點分路徑在樹上求出來的形狀：有葉子、完全沒這條路徑、或撞到分支
/// （分支要跟「沒這條路徑」分開算——`Flag{expect:false}` 兩者語意不同）。
enum PathValue<'a> {
    Leaf(&'a str),
    Missing,
    Branch,
}

fn resolve_path<'a>(tree: &'a BTreeMap<String, StateNode>, path: &str) -> PathValue<'a> {
    let segments: Vec<String> = path.split('.').map(str::to_owned).collect();
    match data::node_at(tree, &segments) {
        Some(StateNode::Leaf(value)) => PathValue::Leaf(value.as_str()),
        Some(StateNode::Branch(_)) => PathValue::Branch,
        None => PathValue::Missing,
    }
}

/// 條件求值：值一律從樹上讀葉子字串；路徑指到分支（不是葉子）一律不成立，
/// 不論條件型別、不論 `default`／`expect` 怎麼設。
fn condition_holds(tree: &BTreeMap<String, StateNode>, condition: &Condition) -> bool {
    match condition {
        Condition::Range {
            path,
            min,
            max,
            min_exclusive,
            max_exclusive,
            default,
        } => {
            let value = match resolve_path(tree, path) {
                PathValue::Leaf(text) => match current_number(text) {
                    Some(value) => value,
                    None => return false,
                },
                PathValue::Missing => match default {
                    Some(default) => *default,
                    None => return false,
                },
                PathValue::Branch => return false,
            };
            if let Some(min) = min {
                if (*min_exclusive && value <= *min) || (!*min_exclusive && value < *min) {
                    return false;
                }
            }
            if let Some(max) = max {
                if (*max_exclusive && value >= *max) || (!*max_exclusive && value > *max) {
                    return false;
                }
            }
            true
        }
        Condition::Contains { path, any } => {
            let text = match resolve_path(tree, path) {
                PathValue::Leaf(text) => text,
                PathValue::Missing => "",
                PathValue::Branch => return false,
            };
            any.iter().any(|needle| text.contains(needle.as_str()))
        }
        Condition::Flag { path, expect } => {
            let actual = match resolve_path(tree, path) {
                PathValue::Leaf(text) => {
                    let text = text.trim().to_ascii_lowercase();
                    text == "true" || text == "1"
                }
                PathValue::Missing => false,
                PathValue::Branch => return false,
            };
            actual == *expect
        }
    }
}

/// 點分路徑取葉子字串（供佔位換值用）；路徑不存在或撞到分支都回 None，換成空字串。
fn leaf_at<'a>(tree: &'a BTreeMap<String, StateNode>, path: &str) -> Option<&'a str> {
    match resolve_path(tree, path) {
        PathValue::Leaf(value) => Some(value),
        PathValue::Missing | PathValue::Branch => None,
    }
}

/// `"480/500"` 這種現值/上限對取現值，純數字照原樣 parse。
fn current_number(value: &str) -> Option<f64> {
    split_pair(value)
        .map(|(current, _max)| current)
        .or_else(|| value.trim().parse::<f64>().ok())
}

/// 把命中文本裡的 `{{state:<點分路徑>}}` 換成樹上的現值，路徑不存在就換成空字串。
fn resolve_state_placeholders(text: &str, tree: &BTreeMap<String, StateNode>) -> String {
    const MARK: &str = "{{state:";
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(MARK) {
        result.push_str(&rest[..start]);
        let after = &rest[start + MARK.len()..];
        let Some(end) = after.find("}}") else {
            // 沒有收尾的殘缺標記：原樣保留，不吃掉後面的文字
            result.push_str(&rest[start..]);
            rest = "";
            break;
        };
        result.push_str(leaf_at(tree, &after[..end]).unwrap_or(""));
        rest = &after[end + 2..];
    }
    result.push_str(rest);
    result
}

// ---------------------------------------------------------------------
// 回合套用：平欄／樹欄照舊套用＋增量走本地權威＋骰值每回合重擲＋觸發表求值
// ---------------------------------------------------------------------

/// 全量桌跳動比對用：這個路徑目前的舊值（平欄查 table，樹查現有 leaf_at）。
fn old_field_value(world: &data::WorldState, path: &[String]) -> Option<String> {
    if path.len() == 1 {
        world.state.table.get(&path[0]).cloned()
    } else {
        leaf_at(&world.state.tree, &path.join(".")).map(str::to_owned)
    }
}

/// 從一段可能帶前後綴的字串抽出第一段數字：先試 `split_pair`（"500/500" 取現值），
/// 抽不出來就逐字掃出第一段允許負號與小數的數字子字串（吃得下「❤️ 60」「体力60」「第 3 天」）。
/// 整段都沒有數字（純文字欄）就回 None，呼叫端跳過不比。
fn numeric_value(raw: &str) -> Option<f64> {
    if let Some((current, _max)) = split_pair(raw) {
        return Some(current);
    }
    let chars: Vec<char> = raw.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        let starts_number = chars[index].is_ascii_digit()
            || (chars[index] == '-' && chars.get(index + 1).is_some_and(char::is_ascii_digit));
        if !starts_number {
            index += 1;
            continue;
        }
        let start = index;
        if chars[index] == '-' {
            index += 1;
        }
        while index < chars.len() && chars[index].is_ascii_digit() {
            index += 1;
        }
        if chars.get(index) == Some(&'.') && chars.get(index + 1).is_some_and(char::is_ascii_digit)
        {
            index += 1;
            while index < chars.len() && chars[index].is_ascii_digit() {
                index += 1;
            }
        }
        let text: String = chars[start..index].iter().collect();
        if let Ok(value) = text.parse::<f64>() {
            return Some(value);
        }
    }
    None
}

/// 全量桌跳動比對：這一輪有報的每個路徑，新舊值都抽得出數字、不是計數器欄、
/// 幅度同時過絕對與相對門檻才算「跳」——命中就標上面板記號、記一筆 Jump 給玩家看。
fn detect_jumps(
    world: &mut data::WorldState,
    fields: &[(Vec<String>, String)],
    old_values: &[Option<String>],
    records: &mut Vec<Record>,
) {
    world.state.jumps.clear();
    for ((path, new_value), old_value) in fields.iter().zip(old_values) {
        let Some(old_value) = old_value else { continue };
        let (Some(old_num), Some(new_num)) = (numeric_value(old_value), numeric_value(new_value))
        else {
            continue;
        };
        let rule = rule_for_path(&world.mechanism, path, Some(old_value.as_str()));
        if rule.kind == FieldKind::Counter {
            continue;
        }
        let delta = new_num - old_num;
        if delta.abs() < JUMP_ABS_THRESHOLD
            || delta.abs() < JUMP_RATIO_THRESHOLD * old_num.abs().max(new_num.abs())
        {
            continue;
        }
        let path_str = path.join(".");
        let mark = signed_delta_mark(delta);
        world.state.jumps.insert(path_str.clone(), mark.clone());
        records.push(Record::new(
            RecordKind::Jump,
            path_str.clone(),
            format!(
                "{path_str} 一回合內從 {} 跳到 {}（{mark}），疑似模型算錯；\
                 若這欄本來就該大幅變動（例如天數計數器），可在面板點記號標成計數器，之後不再提醒。",
                format_num(old_num),
                format_num(new_num)
            ),
        ));
    }
}

/// 把一則回覆的狀態區塊套進這桌：平欄照舊、增量走本地權威、骰值每回合重擲、
/// 觸發表求值（模型套用到樹之後才查表）、全量桌跳動比對（只給玩家看，不進提示詞）。
/// `user_name` 供觸發文本的 `{{user}}` 代換。
pub fn apply_block(
    world: &mut data::WorldState,
    block: &crate::transport::StateBlock,
    user_name: &str,
) -> Outcome {
    let jump_check = !world.mechanism.incremental;
    let old_values: Vec<Option<String>> = if jump_check {
        block
            .fields
            .iter()
            .map(|(path, _)| old_field_value(world, path))
            .collect()
    } else {
        Vec::new()
    };

    for (path, value) in &block.fields {
        if path.len() == 1 {
            world.state.table.insert(path[0].clone(), value.clone());
        } else {
            data::set_tree_value(&mut world.state.tree, path, value);
        }
    }
    let patches: Vec<Patch> = block
        .updates
        .iter()
        .flat_map(|update| parse_updates(update))
        .collect();
    let mut outcome = apply_updates(&mut world.state.tree, &world.mechanism, &patches);
    if jump_check {
        detect_jumps(world, &block.fields, &old_values, &mut outcome.records);
    }
    if world.mechanism.incremental {
        reroll(&mut world.state.tree, &world.mechanism);
        let triggered = evaluate_triggers(&world.state.tree, &world.mechanism, user_name);
        world.state.triggers = triggered.hits;
        for flag in &triggered.flags {
            let segments: Vec<String> = flag.split('.').map(str::to_owned).collect();
            data::set_tree_value(&mut world.state.tree, &segments, "true");
            outcome.records.push(Record::new(
                RecordKind::Absorbed,
                flag.clone(),
                format!("一次性事件已觸發，旗標 {flag} 釘死為 true，不再重演。"),
            ));
        }
    }
    // 衍生值不分全量／增量桌都要重算：模型看不到 rare 欄位、也算不出它的值，
    // 全靠本地用這一輪剛套用完的樹重新求一次。
    outcome
        .records
        .extend(recompute_derived(&mut world.state.tree, &world.mechanism));
    world.state.notes = outcome.notes.clone();
    world.state.changes = outcome.changes.clone();
    outcome
}

// ---------------------------------------------------------------------
// 記帳落檔：worlds/<world_id>/mechanism-log.jsonl
// ---------------------------------------------------------------------

/// 每筆記錄落一行 JSON；寫檔失敗一律吞掉，記帳設施不該反過來中斷遊戲。
pub fn append_log(root: &Path, world_id: &str, scene: u64, records: &[Record]) {
    if records.is_empty() {
        return;
    }
    let Ok(path) = data::mechanism_log_path(root, world_id) else {
        return;
    };
    let ts = data::local_timestamp_seconds().unwrap_or_else(|_| "unknown-time".to_owned());
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    else {
        return;
    };
    use std::io::Write;
    for record in records {
        let line = serde_json::json!({
            "ts": ts,
            "scene": scene,
            "kind": record.kind,
            "path": record.path,
            "detail": record.detail,
        });
        if let Ok(text) = serde_json::to_string(&line) {
            let _ = writeln!(file, "{text}");
        }
    }
}

// ---------------------------------------------------------------------
// 帳本讀取：世界書分頁「機制帳本」面板用，彙總 mechanism-log.jsonl
// ---------------------------------------------------------------------

/// 帳本一列：一條可切換開關的機制條目（接管或跳過）。`uid` 供面板呼叫既有
/// `upsert_worldbook_entry` 切換 `disabled` 用；`sent` 是目前是否照原文送模型。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub uid: u64,
    pub title: String,
    pub kind: RecordKind,
    pub detail: String,
    pub sent: bool,
}

/// 世界書分頁「機制帳本」面板用：對得上目前世界書條目的接管／跳過清單，
/// 加上另外四類記帳（拒收／夾邊界／格式錯誤／跳動）的次數。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Ledger {
    pub entries: Vec<LedgerEntry>,
    pub rejected: usize,
    pub clamped: usize,
    pub errors: usize,
    pub jumps: usize,
}

fn ledger_rank(kind: RecordKind) -> u8 {
    match kind {
        RecordKind::Absorbed => 0,
        RecordKind::Skipped => 1,
        _ => 2,
    }
}

/// 讀 `mechanism-log.jsonl` 彙總成面板用帳本。容錯是紅線：檔案不存在、讀不到、
/// 壞行一律跳過，絕不 panic。`Absorbed`／`Skipped` 以 `path`（條目標題，trim 後）為 key
/// 去重，同一條目重複記帳只留最新那筆；再拿目前世界書比對標題，對不上的（例如一次性
/// 事件旗標這類不是條目的記錄）沒有開關可切，不列進 `entries`。其餘四種只累計次數。
pub fn read_ledger(root: &Path, world_id: &str) -> Ledger {
    let mut ledger = Ledger::default();
    let Ok(path) = data::mechanism_log_path(root, world_id) else {
        return ledger;
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return ledger;
    };

    let mut absorbed_or_skipped: BTreeMap<String, Record> = BTreeMap::new();
    for line in text.lines() {
        let Ok(record) = serde_json::from_str::<Record>(line) else {
            continue;
        };
        match record.kind {
            RecordKind::Absorbed | RecordKind::Skipped => {
                absorbed_or_skipped.insert(record.path.trim().to_owned(), record);
            }
            RecordKind::Rejected => ledger.rejected += 1,
            RecordKind::Clamped => ledger.clamped += 1,
            RecordKind::Error => ledger.errors += 1,
            RecordKind::Jump => ledger.jumps += 1,
        }
    }
    if absorbed_or_skipped.is_empty() {
        return ledger;
    }

    let worldbook = data::read_worldbook(root, world_id).unwrap_or_default();
    let mut entries: Vec<LedgerEntry> = worldbook
        .iter()
        .filter_map(|entry| {
            let record = absorbed_or_skipped.get(entry.title.trim())?;
            Some(LedgerEntry {
                uid: entry.uid,
                title: entry.title.clone(),
                kind: record.kind,
                detail: record.detail.clone(),
                sent: !entry.disabled,
            })
        })
        .collect();
    entries.sort_by(|a, b| {
        ledger_rank(a.kind)
            .cmp(&ledger_rank(b.kind))
            .then_with(|| a.title.cmp(&b.title))
    });
    ledger.entries = entries;
    ledger
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::InjectLevel;
    use std::path::PathBuf;

    fn tree_from(pairs: &[(&str, &str)]) -> BTreeMap<String, StateNode> {
        let mut tree = BTreeMap::new();
        for (path, value) in pairs {
            let segments: Vec<String> = path.split('.').map(str::to_owned).collect();
            data::set_tree_value(&mut tree, &segments, value);
        }
        tree
    }

    fn rule(kind: FieldKind, min: Option<f64>, max: Option<f64>) -> FieldRule {
        let mut rule = FieldRule::for_kind(kind);
        rule.min = min;
        rule.max = max;
        rule
    }

    fn mechanism_with(rules: &[(&str, FieldRule)]) -> Mechanism {
        Mechanism {
            version: 1,
            rules: rules
                .iter()
                .map(|(path, rule)| ((*path).to_owned(), rule.clone()))
                .collect(),
            triggers: Vec::new(),
            incremental: false,
            guide: String::new(),
        }
    }

    // ---- parse_updates ----

    #[test]
    fn parse_updates_reads_all_five_ops_from_the_documented_shape() {
        let block = r#"<Analysis>english analysis, discarded</Analysis>
<JSONPatch>
[
  { "op": "replace", "path": "/World/Location", "value": "晨港" },
  { "op": "delta",   "path": "/Heroes/亚瑟·晨光/Affection", "value": 5 },
  { "op": "insert",  "path": "/Player/Inventory/藥水", "value": { "描述": "紅色的", "數量": 2 } },
  { "op": "remove",  "path": "/Player/Inventory/舊劍" },
  { "op": "move",    "from": "/NPCs/A", "to": "/Heroes/鴉" }
]
</JSONPatch>"#;
        let patches = parse_updates(block);
        assert_eq!(patches.len(), 5);
        assert_eq!(patches[0].op, PatchOp::Replace);
        assert_eq!(patches[0].path, vec!["World", "Location"]);
        assert_eq!(patches[1].op, PatchOp::Delta);
        assert_eq!(patches[1].path, vec!["Heroes", "亚瑟·晨光", "Affection"]);
        assert_eq!(patches[2].op, PatchOp::Insert);
        assert_eq!(patches[3].op, PatchOp::Remove);
        assert_eq!(patches[3].path, vec!["Player", "Inventory", "舊劍"]);
        assert_eq!(patches[4].op, PatchOp::Move);
        assert_eq!(patches[4].from, vec!["NPCs", "A"]);
        assert_eq!(patches[4].path, vec!["Heroes", "鴉"]);
    }

    #[test]
    fn parse_updates_skips_the_broken_object_but_keeps_the_rest() {
        // 陣列漏逗號：整段 parse 會失敗，退回逐個掃物件，壞的跳過、好的照收。
        let block = r#"<JSONPatch>[{"op":"delta","path":"/a","value":1} {"op":"delta","path":"/b","value":2}]</JSONPatch>"#;
        let patches = parse_updates(block);
        assert_eq!(patches.len(), 2);
        assert_eq!(patches[0].path, vec!["a"]);
        assert_eq!(patches[1].path, vec!["b"]);
    }

    #[test]
    fn parse_updates_accepts_dot_path_and_code_fence_without_jsonpatch_tag() {
        let block = "```json\n[{\"op\":\"delta\",\"path\":\"World.HP\",\"value\":-3}]\n```";
        let patches = parse_updates(block);
        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0].path, vec!["World", "HP"]);
    }

    #[test]
    fn parse_updates_returns_empty_on_garbage() {
        assert!(parse_updates("這回覆完全沒有 JSON。").is_empty());
    }

    // ---- apply_updates：五種 op 各一條 ----

    #[test]
    fn delta_adds_to_current_value_within_bounds() {
        let mut tree = tree_from(&[("World.HP", "10")]);
        let mechanism =
            mechanism_with(&[("World.HP", rule(FieldKind::Number, Some(0.0), Some(100.0)))]);
        let patches = vec![Patch {
            op: PatchOp::Delta,
            path: vec!["World".to_owned(), "HP".to_owned()],
            from: Vec::new(),
            value: Some(serde_json::json!(5)),
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert!(outcome.records.is_empty());
        assert_eq!(tree.get("World"), {
            let mut inner = BTreeMap::new();
            inner.insert("HP".to_owned(), StateNode::Leaf("15".to_owned()));
            Some(&StateNode::Branch(inner))
        });
    }

    #[test]
    fn replace_overwrites_text_field() {
        let mut tree = tree_from(&[("World.Location", "舊城")]);
        let mechanism = Mechanism::default();
        let patches = vec![Patch {
            op: PatchOp::Replace,
            path: vec!["World".to_owned(), "Location".to_owned()],
            from: Vec::new(),
            value: Some(serde_json::json!("晨港")),
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert!(outcome.records.is_empty());
        assert_eq!(
            tree.get("World"),
            Some(&StateNode::Branch(BTreeMap::from([(
                "Location".to_owned(),
                StateNode::Leaf("晨港".to_owned())
            )])))
        );
    }

    #[test]
    fn insert_creates_missing_branches_and_leaf() {
        let mut tree: BTreeMap<String, StateNode> = BTreeMap::new();
        let mechanism = Mechanism::default();
        let patches = vec![Patch {
            op: PatchOp::Insert,
            path: vec![
                "Player".to_owned(),
                "Inventory".to_owned(),
                "藥水".to_owned(),
            ],
            from: Vec::new(),
            value: Some(serde_json::json!({ "數量": 2 })),
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert!(outcome.records.is_empty());
        let StateNode::Branch(player) = tree.get("Player").unwrap() else {
            panic!("Player 應該是分支");
        };
        let StateNode::Branch(inventory) = player.get("Inventory").unwrap() else {
            panic!("Inventory 應該是分支");
        };
        assert!(inventory.contains_key("藥水"));
    }

    #[test]
    fn remove_deletes_leaf_and_prunes_empty_parent() {
        let mut tree = tree_from(&[("Player.Inventory.舊劍", "鏽")]);
        let mechanism = Mechanism::default();
        let patches = vec![Patch {
            op: PatchOp::Remove,
            path: vec![
                "Player".to_owned(),
                "Inventory".to_owned(),
                "舊劍".to_owned(),
            ],
            from: Vec::new(),
            value: None,
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert!(outcome.records.is_empty());
        // Inventory 底下只有這一項，刪掉後 Inventory、Player 都該一併被剪掉。
        assert!(!tree.contains_key("Player"));
    }

    #[test]
    fn move_relocates_subtree_without_recording_anything() {
        let mut tree = tree_from(&[("NPCs.A.HP", "10")]);
        let mechanism = Mechanism::default();
        let patches = vec![Patch {
            op: PatchOp::Move,
            path: vec!["Heroes".to_owned(), "鴉".to_owned()],
            from: vec!["NPCs".to_owned(), "A".to_owned()],
            value: None,
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert!(outcome.records.is_empty());
        assert!(!tree.contains_key("NPCs"));
        let StateNode::Branch(heroes) = tree.get("Heroes").unwrap() else {
            panic!("Heroes 應該是分支");
        };
        assert!(heroes.contains_key("鴉"));
    }

    // ---- 拒收與夾邊界 ----

    #[test]
    fn replace_absolute_value_on_number_field_is_rejected_and_local_state_unchanged() {
        let mut tree = tree_from(&[("World.HP", "80")]);
        let mechanism =
            mechanism_with(&[("World.HP", rule(FieldKind::Number, Some(0.0), Some(100.0)))]);
        let patches = vec![Patch {
            op: PatchOp::Replace,
            path: vec!["World".to_owned(), "HP".to_owned()],
            from: Vec::new(),
            value: Some(serde_json::json!(999)),
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert_eq!(outcome.records.len(), 1);
        assert_eq!(outcome.records[0].kind, RecordKind::Rejected);
        let StateNode::Branch(world) = tree.get("World").unwrap() else {
            panic!("World 應該是分支");
        };
        assert_eq!(world.get("HP"), Some(&StateNode::Leaf("80".to_owned())));
    }

    #[test]
    fn delta_clamps_to_max_and_records_clamped() {
        let mut tree = tree_from(&[("World.HP", "95")]);
        let mechanism =
            mechanism_with(&[("World.HP", rule(FieldKind::Number, Some(0.0), Some(100.0)))]);
        let patches = vec![Patch {
            op: PatchOp::Delta,
            path: vec!["World".to_owned(), "HP".to_owned()],
            from: Vec::new(),
            value: Some(serde_json::json!(50)),
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert_eq!(outcome.records.len(), 1);
        assert_eq!(outcome.records[0].kind, RecordKind::Clamped);
        let StateNode::Branch(world) = tree.get("World").unwrap() else {
            panic!("World 應該是分支");
        };
        assert_eq!(world.get("HP"), Some(&StateNode::Leaf("100".to_owned())));
    }

    #[test]
    fn pair_delta_moves_current_value_and_replace_only_changes_cap() {
        let mut tree = tree_from(&[("亚瑟·晨光.HP", "480/500")]);
        let mechanism = mechanism_with(&[(
            "亚瑟·晨光.HP",
            rule(FieldKind::Pair, Some(0.0), Some(500.0)),
        )]);
        let delta_patch = Patch {
            op: PatchOp::Delta,
            path: vec!["亚瑟·晨光".to_owned(), "HP".to_owned()],
            from: Vec::new(),
            value: Some(serde_json::json!(-30)),
        };
        let outcome = apply_updates(&mut tree, &mechanism, std::slice::from_ref(&delta_patch));
        assert!(outcome.records.is_empty());
        let StateNode::Branch(character) = tree.get("亚瑟·晨光").unwrap() else {
            panic!("角色應該是分支");
        };
        assert_eq!(
            character.get("HP"),
            Some(&StateNode::Leaf("450/500".to_owned()))
        );

        // replace 只認上限：升級成 450/600（現值不變才收，現值變了要記一筆）。
        let upgrade_patch = Patch {
            op: PatchOp::Replace,
            path: vec!["亚瑟·晨光".to_owned(), "HP".to_owned()],
            from: Vec::new(),
            value: Some(serde_json::json!("450/600")),
        };
        let outcome = apply_updates(&mut tree, &mechanism, std::slice::from_ref(&upgrade_patch));
        assert!(outcome.records.is_empty());
        let StateNode::Branch(character) = tree.get("亚瑟·晨光").unwrap() else {
            panic!("角色應該是分支");
        };
        assert_eq!(
            character.get("HP"),
            Some(&StateNode::Leaf("450/600".to_owned()))
        );

        // replace 想連現值一起改：上限照改，但現值變動被拒收，本地帳沿用舊現值。
        let bad_patch = Patch {
            op: PatchOp::Replace,
            path: vec!["亚瑟·晨光".to_owned(), "HP".to_owned()],
            from: Vec::new(),
            value: Some(serde_json::json!("999/700")),
        };
        let outcome = apply_updates(&mut tree, &mechanism, std::slice::from_ref(&bad_patch));
        assert_eq!(outcome.records.len(), 1);
        assert_eq!(outcome.records[0].kind, RecordKind::Rejected);
        assert_eq!(
            outcome.records[0].detail,
            "亚瑟·晨光.HP 現值 450/600，請用增減量（delta）而不是絕對值。"
        );
        let StateNode::Branch(character) = tree.get("亚瑟·晨光").unwrap() else {
            panic!("角色應該是分支");
        };
        assert_eq!(
            character.get("HP"),
            Some(&StateNode::Leaf("450/700".to_owned()))
        );
    }

    #[test]
    fn delta_on_missing_path_is_a_hard_error() {
        let mut tree: BTreeMap<String, StateNode> = BTreeMap::new();
        let mechanism = Mechanism::default();
        let patches = vec![Patch {
            op: PatchOp::Delta,
            path: vec!["World".to_owned(), "HP".to_owned()],
            from: Vec::new(),
            value: Some(serde_json::json!(5)),
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert_eq!(outcome.records.len(), 1);
        assert_eq!(outcome.records[0].kind, RecordKind::Error);
    }

    #[test]
    fn delta_on_text_field_is_a_hard_error() {
        let mut tree = tree_from(&[("World.Location", "晨港")]);
        let mechanism = Mechanism::default();
        let patches = vec![Patch {
            op: PatchOp::Delta,
            path: vec!["World".to_owned(), "Location".to_owned()],
            from: Vec::new(),
            value: Some(serde_json::json!(5)),
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert_eq!(outcome.records.len(), 1);
        assert_eq!(outcome.records[0].kind, RecordKind::Error);
        let StateNode::Branch(world) = tree.get("World").unwrap() else {
            panic!("World 應該是分支");
        };
        assert_eq!(
            world.get("Location"),
            Some(&StateNode::Leaf("晨港".to_owned()))
        );
    }

    #[test]
    fn remove_on_missing_path_is_a_hard_error() {
        let mut tree: BTreeMap<String, StateNode> = BTreeMap::new();
        let mechanism = Mechanism::default();
        let patches = vec![Patch {
            op: PatchOp::Remove,
            path: vec![
                "Player".to_owned(),
                "Inventory".to_owned(),
                "舊劍".to_owned(),
            ],
            from: Vec::new(),
            value: None,
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert_eq!(outcome.records.len(), 1);
        assert_eq!(outcome.records[0].kind, RecordKind::Error);
    }

    #[test]
    fn underscore_prefixed_field_is_rejected_and_untouched() {
        let mut tree = tree_from(&[("World._Secret", "隱藏")]);
        let mechanism = Mechanism::default();
        let patches = vec![Patch {
            op: PatchOp::Replace,
            path: vec!["World".to_owned(), "_Secret".to_owned()],
            from: Vec::new(),
            value: Some(serde_json::json!("洩漏")),
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert_eq!(outcome.records.len(), 1);
        assert_eq!(outcome.records[0].kind, RecordKind::Rejected);
        let StateNode::Branch(world) = tree.get("World").unwrap() else {
            panic!("World 應該是分支");
        };
        assert_eq!(
            world.get("_Secret"),
            Some(&StateNode::Leaf("隱藏".to_owned()))
        );
    }

    #[test]
    fn wildcard_rule_clamps_any_hero_affection_between_0_and_200() {
        let mut tree = tree_from(&[("Heroes.亚瑟·晨光.Affection", "190")]);
        let mechanism = mechanism_with(&[(
            "Heroes.*.Affection",
            rule(FieldKind::Number, Some(0.0), Some(200.0)),
        )]);
        let patches = vec![Patch {
            op: PatchOp::Delta,
            path: vec![
                "Heroes".to_owned(),
                "亚瑟·晨光".to_owned(),
                "Affection".to_owned(),
            ],
            from: Vec::new(),
            value: Some(serde_json::json!(50)),
        }];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert_eq!(outcome.records.len(), 1);
        assert_eq!(outcome.records[0].kind, RecordKind::Clamped);
        let StateNode::Branch(heroes) = tree.get("Heroes").unwrap() else {
            panic!("Heroes 應該是分支");
        };
        let StateNode::Branch(hero) = heroes.get("亚瑟·晨光").unwrap() else {
            panic!("角色應該是分支");
        };
        assert_eq!(
            hero.get("Affection"),
            Some(&StateNode::Leaf("200".to_owned()))
        );
    }

    // ---- notes：只從 Rejected 產生 ----

    #[test]
    fn notes_are_deduped_and_capped_at_five() {
        let mut tree = tree_from(&[("World.Roll100", "42")]);
        let mechanism = mechanism_with(&[(
            "World.Roll100",
            FieldRule {
                kind: FieldKind::Roll,
                min: Some(1.0),
                max: Some(100.0),
                update: crate::data::UpdateMode::Local,
                inject: InjectLevel::Turn,
                branch: None,
                formula: None,
            },
        )]);
        let patch = Patch {
            op: PatchOp::Delta,
            path: vec!["World".to_owned(), "Roll100".to_owned()],
            from: Vec::new(),
            value: Some(serde_json::json!(1)),
        };
        let patches = vec![patch.clone(), patch.clone(), patch];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert_eq!(outcome.records.len(), 3);
        assert_eq!(
            outcome.notes,
            vec!["World.Roll100 由系統本地擲骰，請勿更新。".to_owned()]
        );
    }

    // ---- reroll ----

    #[test]
    fn reroll_always_lands_in_range_and_changes_the_value() {
        let mut tree = tree_from(&[("World.Roll100", "1")]);
        let mechanism = mechanism_with(&[(
            "World.Roll100",
            FieldRule {
                kind: FieldKind::Roll,
                min: Some(1.0),
                max: Some(6.0),
                update: crate::data::UpdateMode::Local,
                inject: InjectLevel::Turn,
                branch: None,
                formula: None,
            },
        )]);
        let mut saw_change = false;
        for _ in 0..30 {
            reroll(&mut tree, &mechanism);
            let StateNode::Branch(world) = tree.get("World").unwrap() else {
                panic!("World 應該是分支");
            };
            let StateNode::Leaf(value) = world.get("Roll100").unwrap() else {
                panic!("Roll100 應該是葉子");
            };
            let parsed: i64 = value.parse().expect("骰值應該是整數字串");
            assert!((1..=6).contains(&parsed));
            if value != "1" {
                saw_change = true;
            }
        }
        assert!(saw_change, "30 次重擲全部落在同一個值，機率上不合理");
    }

    // ---- 衍生值（derived）----

    fn derived_rule(formula: &str) -> FieldRule {
        let mut rule = FieldRule::for_kind(FieldKind::Derived);
        rule.formula = Some(formula.to_owned());
        rule
    }

    #[test]
    fn recompute_derived_evaluates_the_formula_against_the_tree() {
        let mut tree = tree_from(&[("HP", "10"), ("Half", "0")]);
        let mechanism = mechanism_with(&[("Half", derived_rule("floor(HP/2)+1"))]);
        let records = recompute_derived(&mut tree, &mechanism);
        assert!(records.is_empty());
        assert_eq!(tree.get("Half"), Some(&StateNode::Leaf("6".to_owned())));
    }

    #[test]
    fn recompute_derived_leaves_a_missing_leaf_alone() {
        // 跟 Roll 一樣的規矩：路徑不存在就不平白生出新分支。
        let mut tree = tree_from(&[("HP", "10")]);
        let mechanism = mechanism_with(&[("Half", derived_rule("HP*2"))]);
        let records = recompute_derived(&mut tree, &mechanism);
        assert!(records.is_empty());
        assert!(tree.get("Half").is_none());
    }

    #[test]
    fn recompute_derived_records_a_formula_error_and_keeps_the_old_value() {
        let mut tree = tree_from(&[("Half", "0")]); // 沒有 HP，公式取值會失敗
        let mechanism = mechanism_with(&[("Half", derived_rule("HP/2"))]);
        let records = recompute_derived(&mut tree, &mechanism);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].kind, RecordKind::Error);
        assert_eq!(tree.get("Half"), Some(&StateNode::Leaf("0".to_owned())));
    }

    #[test]
    fn apply_block_recomputes_derived_fields_end_to_end() {
        let mechanism = mechanism_with(&[("Half", derived_rule("floor(HP/2)+1"))]);
        let mut world = world_with(&[("HP", "10"), ("Half", "0")], mechanism);
        let block = crate::transport::StateBlock {
            fields: Vec::new(),
            updates: Vec::new(),
            display: String::new(),
        };

        apply_block(&mut world, &block, "阿濤");

        assert_eq!(
            world.state.tree.get("Half"),
            Some(&StateNode::Leaf("6".to_owned()))
        );
    }

    // ---- 整合測試：從一段完整的 <UpdateVariable> 原文一路跑到樹被更新 ----

    #[test]
    fn full_update_variable_block_parses_and_applies_to_the_tree() {
        let mut tree = tree_from(&[
            ("World.Location", "舊港"),
            ("Heroes.亚瑟·晨光.Affection", "10"),
            ("Player.Inventory.舊劍", "鏽蝕"),
        ]);
        let mechanism = mechanism_with(&[(
            "Heroes.*.Affection",
            rule(FieldKind::Number, Some(0.0), Some(200.0)),
        )]);
        let block = r#"<Analysis>Some english reasoning about the scene.</Analysis>
<JSONPatch>
[
  { "op": "replace", "path": "/World/Location", "value": "晨港" },
  { "op": "delta",   "path": "/Heroes/亚瑟·晨光/Affection", "value": 5 },
  { "op": "insert",  "path": "/Player/Inventory/藥水", "value": { "描述": "紅色的", "數量": 2 } },
  { "op": "remove",  "path": "/Player/Inventory/舊劍" }
]
</JSONPatch>"#;
        let patches = parse_updates(block);
        assert_eq!(patches.len(), 4);
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert!(outcome.records.is_empty());

        let StateNode::Branch(world) = tree.get("World").unwrap() else {
            panic!("World 應該是分支");
        };
        assert_eq!(
            world.get("Location"),
            Some(&StateNode::Leaf("晨港".to_owned()))
        );

        let StateNode::Branch(heroes) = tree.get("Heroes").unwrap() else {
            panic!("Heroes 應該是分支");
        };
        let StateNode::Branch(hero) = heroes.get("亚瑟·晨光").unwrap() else {
            panic!("角色應該是分支");
        };
        assert_eq!(
            hero.get("Affection"),
            Some(&StateNode::Leaf("15".to_owned()))
        );

        let StateNode::Branch(player) = tree.get("Player").unwrap() else {
            panic!("Player 應該是分支");
        };
        let StateNode::Branch(inventory) = player.get("Inventory").unwrap() else {
            panic!("Inventory 應該是分支");
        };
        assert!(inventory.contains_key("藥水"));
        assert!(!inventory.contains_key("舊劍"));
    }

    // ---- apply_block：平欄／樹欄套用＋增量本地權威一次跑完 ----

    fn world_with(pairs: &[(&str, &str)], mechanism: Mechanism) -> data::WorldState {
        data::WorldState {
            id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned(),
            name: "測試桌".to_owned(),
            model_bindings: BTreeMap::new(),
            player_card_id: None,
            current_scene: 0,
            catchup_summaries: BTreeMap::new(),
            scene_titles: BTreeMap::new(),
            scene_labels: BTreeMap::new(),
            state: data::TableState {
                table: BTreeMap::new(),
                tree: tree_from(pairs),
                notes: Vec::new(),
                changes: BTreeMap::new(),
                triggers: BTreeMap::new(),
                jumps: BTreeMap::new(),
            },
            mechanism,
            aligned_scene: None,
            branch_bindings: BTreeMap::new(),
            refactor_mode: None,
        }
    }

    #[test]
    fn apply_block_merges_fields_applies_updates_and_records_notes_onto_state() {
        let mechanism =
            mechanism_with(&[("World.HP", rule(FieldKind::Number, Some(0.0), Some(100.0)))]);
        let mut world = world_with(&[("World.HP", "80")], mechanism);
        let block = crate::transport::StateBlock {
            fields: vec![(
                vec!["World".to_owned(), "Location".to_owned()],
                "晨港".to_owned(),
            )],
            updates: vec![
                r#"<JSONPatch>[{"op":"replace","path":"/World/HP","value":999}]</JSONPatch>"#
                    .to_owned(),
            ],
            display: String::new(),
        };

        let outcome = apply_block(&mut world, &block, "阿濤");

        assert_eq!(outcome.records.len(), 1);
        assert_eq!(outcome.records[0].kind, RecordKind::Rejected);
        assert!(!world.state.notes.is_empty());
        assert_eq!(world.state.notes, outcome.notes);

        let StateNode::Branch(world_branch) = world.state.tree.get("World").unwrap() else {
            panic!("World 應該是分支");
        };
        // 絕對值被拒收，本地帳沿用舊值；平欄套用的 Location 正常寫入。
        assert_eq!(
            world_branch.get("HP"),
            Some(&StateNode::Leaf("80".to_owned()))
        );
        assert_eq!(
            world_branch.get("Location"),
            Some(&StateNode::Leaf("晨港".to_owned()))
        );
    }

    // ---- outcome.changes：只有真的改到樹的更新才記帳（狀態欄二期包 5）----

    #[test]
    fn accepted_delta_records_signed_mark_but_rejection_and_error_do_not() {
        let mut tree = tree_from(&[("World.HP", "80"), ("World.Location", "舊城")]);
        let mechanism =
            mechanism_with(&[("World.HP", rule(FieldKind::Number, Some(0.0), Some(100.0)))]);
        let patches = vec![
            // 收下的 delta：標記照原始量寫，即使之後會被夾邊界。
            Patch {
                op: PatchOp::Delta,
                path: vec!["World".to_owned(), "HP".to_owned()],
                from: Vec::new(),
                value: Some(serde_json::json!(50)),
            },
            // 絕對值頂替 delta 欄＝拒收，不進 changes。
            Patch {
                op: PatchOp::Replace,
                path: vec!["World".to_owned(), "HP".to_owned()],
                from: Vec::new(),
                value: Some(serde_json::json!(999)),
            },
            // 路徑不存在＝硬錯誤，不進 changes。
            Patch {
                op: PatchOp::Delta,
                path: vec!["World".to_owned(), "沒有這欄".to_owned()],
                from: Vec::new(),
                value: Some(serde_json::json!(1)),
            },
        ];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert_eq!(
            outcome.changes.get("World.HP"),
            Some(&"+50".to_owned()),
            "標記要帶原始 delta（+50），不是夾完的 +20"
        );
        assert_eq!(outcome.changes.len(), 1, "拒收與硬錯誤都不該進 changes");
    }

    #[test]
    fn negative_delta_and_replace_insert_remove_move_each_record_the_documented_mark() {
        let mut tree = tree_from(&[
            ("World.HP", "480/500"),
            ("World.Location", "舊城"),
            ("Player.Inventory.舊劍", "鏽"),
            ("NPCs.A.HP", "10"),
        ]);
        let mechanism =
            mechanism_with(&[("World.HP", rule(FieldKind::Pair, Some(0.0), Some(500.0)))]);
        let patches = vec![
            // pair delta：負數本身帶「-」，標記不再重複加號。
            Patch {
                op: PatchOp::Delta,
                path: vec!["World".to_owned(), "HP".to_owned()],
                from: Vec::new(),
                value: Some(serde_json::json!(-80)),
            },
            // replace：文字欄收下＝「更新」。
            Patch {
                op: PatchOp::Replace,
                path: vec!["World".to_owned(), "Location".to_owned()],
                from: Vec::new(),
                value: Some(serde_json::json!("晨港")),
            },
            // insert：新建路徑＝「更新」。
            Patch {
                op: PatchOp::Insert,
                path: vec![
                    "Player".to_owned(),
                    "Inventory".to_owned(),
                    "藥水".to_owned(),
                ],
                from: Vec::new(),
                value: Some(serde_json::json!({ "數量": 2 })),
            },
            // remove：刪除既有路徑＝「移除」。
            Patch {
                op: PatchOp::Remove,
                path: vec![
                    "Player".to_owned(),
                    "Inventory".to_owned(),
                    "舊劍".to_owned(),
                ],
                from: Vec::new(),
                value: None,
            },
            // move：搬移成功＝「搬移」，記在目的地路徑。
            Patch {
                op: PatchOp::Move,
                path: vec!["Heroes".to_owned(), "鴉".to_owned()],
                from: vec!["NPCs".to_owned(), "A".to_owned()],
                value: None,
            },
        ];
        let outcome = apply_updates(&mut tree, &mechanism, &patches);
        assert_eq!(outcome.changes.get("World.HP"), Some(&"-80".to_owned()));
        assert_eq!(
            outcome.changes.get("World.Location"),
            Some(&"更新".to_owned())
        );
        assert_eq!(
            outcome.changes.get("Player.Inventory.藥水"),
            Some(&"更新".to_owned())
        );
        assert_eq!(
            outcome.changes.get("Player.Inventory.舊劍"),
            Some(&"移除".to_owned())
        );
        assert_eq!(outcome.changes.get("Heroes.鴉"), Some(&"搬移".to_owned()));
    }

    // ---- append_log：記帳落檔 ----

    #[test]
    fn append_log_writes_one_json_line_per_record_with_all_fields() {
        let root = std::env::temp_dir().join(format!(
            "table-tavern-mechanism-log-{}",
            ulid::Ulid::generate()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let world_id = data::create_world(&root, "測試桌").unwrap();

        let records = vec![
            Record::new(
                RecordKind::Rejected,
                "World.HP".to_owned(),
                "World.HP 現值 80，請用增減量（delta）而不是絕對值。".to_owned(),
            ),
            Record::new(
                RecordKind::Absorbed,
                "[mvu_update] 規則".to_owned(),
                "機制鷹架條目，已由本地機制接管，不再送入提示詞。".to_owned(),
            ),
        ];
        append_log(&root, &world_id, 3, &records);

        let text =
            std::fs::read_to_string(data::mechanism_log_path(&root, &world_id).unwrap()).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        for (line, record) in lines.iter().zip(&records) {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(value
                .get("ts")
                .and_then(serde_json::Value::as_str)
                .is_some());
            assert_eq!(value["scene"].as_u64(), Some(3));
            assert_eq!(value["path"].as_str(), Some(record.path.as_str()));
            assert_eq!(value["detail"].as_str(), Some(record.detail.as_str()));
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    // ---- read_ledger：帳本讀取彙總 ----

    fn ledger_test_world(name: &str) -> (PathBuf, String) {
        let root = std::env::temp_dir().join(format!(
            "table-tavern-mechanism-ledger-{name}-{}",
            ulid::Ulid::generate()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let world_id = data::create_world(&root, "測試桌").unwrap();
        (root, world_id)
    }

    #[test]
    fn read_ledger_matches_titles_counts_rejected_and_skips_broken_lines() {
        let (root, world_id) = ledger_test_world("basic");
        data::upsert_worldbook_entry(
            &root,
            &world_id,
            data::WorldbookEntry {
                uid: u64::MAX,
                title: "宝物栏初始化".to_owned(),
                keys: Vec::new(),
                content: String::new(),
                constant: false,
                order: 0,
                disabled: true,
                visibility: data::Visibility::Gm,
                is_person: false,
                locked: false,
            },
        )
        .unwrap();
        data::upsert_worldbook_entry(
            &root,
            &world_id,
            data::WorldbookEntry {
                uid: u64::MAX,
                title: "随机事件表".to_owned(),
                keys: Vec::new(),
                content: String::new(),
                constant: false,
                order: 0,
                disabled: false,
                visibility: data::Visibility::Gm,
                is_person: false,
                locked: false,
            },
        )
        .unwrap();

        append_log(
            &root,
            &world_id,
            1,
            &[
                Record::new(
                    RecordKind::Absorbed,
                    "宝物栏初始化".to_owned(),
                    "機制鷹架條目，已由本地機制接管。".to_owned(),
                ),
                Record::new(
                    RecordKind::Skipped,
                    "随机事件表".to_owned(),
                    "卡片腳本認不出來，預設不送模型。".to_owned(),
                ),
                Record::new(
                    RecordKind::Rejected,
                    "World.HP".to_owned(),
                    "拒收".to_owned(),
                ),
            ],
        );
        // 壞行：非 JSON，讀檔時要跳過而不影響其餘行。
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(data::mechanism_log_path(&root, &world_id).unwrap())
            .unwrap();
        writeln!(file, "這不是 JSON").unwrap();

        let ledger = read_ledger(&root, &world_id);
        assert_eq!(ledger.entries.len(), 2);
        assert_eq!(ledger.entries[0].title, "宝物栏初始化");
        assert_eq!(ledger.entries[0].kind, RecordKind::Absorbed);
        assert!(!ledger.entries[0].sent); // disabled=true → 不送模型
        assert_eq!(ledger.entries[1].title, "随机事件表");
        assert_eq!(ledger.entries[1].kind, RecordKind::Skipped);
        assert!(ledger.entries[1].sent); // disabled=false → 照原文送
        assert_eq!(ledger.rejected, 1);
        assert_eq!(ledger.clamped, 0);
        assert_eq!(ledger.errors, 0);
        assert_eq!(ledger.jumps, 0);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn read_ledger_keeps_only_the_latest_record_for_a_repeated_entry() {
        let (root, world_id) = ledger_test_world("dedupe");
        data::upsert_worldbook_entry(
            &root,
            &world_id,
            data::WorldbookEntry {
                uid: u64::MAX,
                title: "机制条目A".to_owned(),
                keys: Vec::new(),
                content: String::new(),
                constant: false,
                order: 0,
                disabled: true,
                visibility: data::Visibility::Gm,
                is_person: false,
                locked: false,
            },
        )
        .unwrap();

        // 同一條目重複匯入會記兩筆帳，後面那筆才是最新狀態。
        append_log(
            &root,
            &world_id,
            1,
            &[Record::new(
                RecordKind::Absorbed,
                "机制条目A".to_owned(),
                "第一次匯入".to_owned(),
            )],
        );
        append_log(
            &root,
            &world_id,
            2,
            &[Record::new(
                RecordKind::Absorbed,
                "机制条目A".to_owned(),
                "第二次匯入".to_owned(),
            )],
        );

        let ledger = read_ledger(&root, &world_id);
        assert_eq!(ledger.entries.len(), 1);
        assert_eq!(ledger.entries[0].detail, "第二次匯入");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn read_ledger_returns_empty_when_log_file_is_missing() {
        // 不建世界目錄：mechanism_log_path 組得出路徑，但檔案不存在。
        let root = std::env::temp_dir().join(format!(
            "table-tavern-mechanism-ledger-missing-{}",
            ulid::Ulid::generate()
        ));
        let world_id = data::new_id();
        let ledger = read_ledger(&root, &world_id);
        assert_eq!(ledger, Ledger::default());
    }

    // ---- evaluate_triggers：四種條件各自成立／不成立 ----

    #[test]
    fn range_condition_checks_inclusive_exclusive_and_default() {
        let tree = tree_from(&[("World.HP", "50")]);
        let inclusive_bounds = Condition::Range {
            path: "World.HP".to_owned(),
            min: Some(50.0),
            max: Some(50.0),
            min_exclusive: false,
            max_exclusive: false,
            default: None,
        };
        assert!(condition_holds(&tree, &inclusive_bounds));

        let exclusive_min = Condition::Range {
            path: "World.HP".to_owned(),
            min: Some(50.0),
            max: None,
            min_exclusive: true,
            max_exclusive: false,
            default: None,
        };
        assert!(!condition_holds(&tree, &exclusive_min));

        let missing_with_default = Condition::Range {
            path: "World.Missing".to_owned(),
            min: Some(0.0),
            max: None,
            min_exclusive: false,
            max_exclusive: false,
            default: Some(10.0),
        };
        assert!(condition_holds(&tree, &missing_with_default));

        let missing_without_default = Condition::Range {
            path: "World.Missing".to_owned(),
            min: Some(0.0),
            max: None,
            min_exclusive: false,
            max_exclusive: false,
            default: None,
        };
        assert!(!condition_holds(&tree, &missing_without_default));
    }

    /// 計數器門檻＝只給 min 的 Range，跟一般數值區間邏輯逐字相同，不另立型別。
    #[test]
    fn range_condition_as_counter_threshold_only_checks_min() {
        let tree = tree_from(&[("World.Kills", "3")]);
        let at_threshold = Condition::Range {
            path: "World.Kills".to_owned(),
            min: Some(3.0),
            max: None,
            min_exclusive: false,
            max_exclusive: false,
            default: None,
        };
        assert!(condition_holds(&tree, &at_threshold));

        let below_threshold = Condition::Range {
            path: "World.Kills".to_owned(),
            min: Some(4.0),
            max: None,
            min_exclusive: false,
            max_exclusive: false,
            default: None,
        };
        assert!(!condition_holds(&tree, &below_threshold));
    }

    #[test]
    fn range_condition_reads_current_value_out_of_a_pair_field() {
        let tree = tree_from(&[("World.HP", "480/500")]);
        let holds = Condition::Range {
            path: "World.HP".to_owned(),
            min: Some(400.0),
            max: None,
            min_exclusive: false,
            max_exclusive: false,
            default: None,
        };
        assert!(condition_holds(&tree, &holds));
        let fails = Condition::Range {
            path: "World.HP".to_owned(),
            min: Some(490.0),
            max: None,
            min_exclusive: false,
            max_exclusive: false,
            default: None,
        };
        assert!(!condition_holds(&tree, &fails));
    }

    #[test]
    fn contains_condition_matches_any_needle_and_missing_leaf_is_empty_string() {
        let tree = tree_from(&[("World.Location", "北方雪原")]);
        let hits = Condition::Contains {
            path: "World.Location".to_owned(),
            any: vec!["南方".to_owned(), "雪原".to_owned()],
        };
        assert!(condition_holds(&tree, &hits));

        let missing = Condition::Contains {
            path: "World.Missing".to_owned(),
            any: vec!["雪原".to_owned()],
        };
        assert!(!condition_holds(&tree, &missing));
    }

    #[test]
    fn flag_condition_reads_true_variants_and_missing_leaf_counts_as_false() {
        let tree = tree_from(&[("Events.已發生", "TRUE"), ("Events.也算真", "1")]);
        assert!(condition_holds(
            &tree,
            &Condition::Flag {
                path: "Events.已發生".to_owned(),
                expect: true,
            }
        ));
        assert!(condition_holds(
            &tree,
            &Condition::Flag {
                path: "Events.也算真".to_owned(),
                expect: true,
            }
        ));
        // 沒發生過（葉子不存在）視為 false，expect: false 才會成立——一次性事件的初始狀態。
        assert!(condition_holds(
            &tree,
            &Condition::Flag {
                path: "Events.還沒發生".to_owned(),
                expect: false,
            }
        ));
        assert!(!condition_holds(
            &tree,
            &Condition::Flag {
                path: "Events.還沒發生".to_owned(),
                expect: true,
            }
        ));
    }

    /// 路徑指到分支＝一律不成立，跟「路徑不存在」的預設語意分開算：
    /// `Flag{expect:false}` 對著一個真的存在的分支不該被當成「沒發生過」。
    #[test]
    fn condition_pointing_at_a_branch_never_holds() {
        let tree = tree_from(&[("World.City.Name", "晨港")]);
        assert!(!condition_holds(
            &tree,
            &Condition::Flag {
                path: "World.City".to_owned(),
                expect: false,
            }
        ));
        assert!(!condition_holds(
            &tree,
            &Condition::Contains {
                path: "World.City".to_owned(),
                any: vec!["晨".to_owned()],
            }
        ));
        assert!(!condition_holds(
            &tree,
            &Condition::Range {
                path: "World.City".to_owned(),
                min: None,
                max: None,
                min_exclusive: false,
                max_exclusive: false,
                default: Some(0.0),
            }
        ));
    }

    // ---- evaluate_triggers：if/else 鏈語意、佔位換值、一次性事件收乾淨 ----

    fn range_case(min: f64, text: &str) -> data::TriggerCase {
        data::TriggerCase {
            when: vec![Condition::Range {
                path: "World.Invasion".to_owned(),
                min: Some(min),
                max: None,
                min_exclusive: false,
                max_exclusive: false,
                default: Some(0.0),
            }],
            text: text.to_owned(),
        }
    }

    fn else_case(text: &str) -> data::TriggerCase {
        data::TriggerCase {
            when: Vec::new(),
            text: text.to_owned(),
        }
    }

    /// if/else-if 鏈：前面命中的 case 贏，後面即使也成立也不會被拿到；全不中時才輪到空
    /// `when` 的兜底 case；命中文本會先套 preamble 再過一次佔位換值。
    #[test]
    fn evaluate_triggers_stops_at_the_first_matching_case_and_falls_back_to_else() {
        let mechanism = Mechanism {
            version: 1,
            rules: BTreeMap::new(),
            triggers: vec![data::Trigger {
                id: "侵略".to_owned(),
                title: "環境氛圍".to_owned(),
                mode: TriggerMode::Range,
                cases: vec![
                    range_case(80.0, "淪陷邊緣：現值 {{state:World.Invasion}}"),
                    range_case(50.0, "戰雲密布"),
                    else_case("風平浪靜"),
                ],
                preamble: "隱藏背景".to_owned(),
                scope: Vec::new(),
                flag: None,
            }],
            incremental: true,
            guide: String::new(),
        };

        let high = tree_from(&[("World.Invasion", "90")]);
        let outcome = evaluate_triggers(&high, &mechanism, "阿濤");
        assert_eq!(
            outcome.hits.get("侵略"),
            Some(&"隱藏背景\n\n淪陷邊緣：現值 90".to_owned())
        );

        let mid = tree_from(&[("World.Invasion", "60")]);
        let outcome = evaluate_triggers(&mid, &mechanism, "阿濤");
        assert_eq!(
            outcome.hits.get("侵略"),
            Some(&"隱藏背景\n\n戰雲密布".to_owned())
        );

        let low = tree_from(&[("World.Invasion", "10")]);
        let outcome = evaluate_triggers(&low, &mechanism, "阿濤");
        assert_eq!(
            outcome.hits.get("侵略"),
            Some(&"隱藏背景\n\n風平浪靜".to_owned())
        );
    }

    /// 沒有任何 case 命中（沒有兜底）＝這個 trigger 這輪沒有文本。
    #[test]
    fn evaluate_triggers_produces_no_text_when_no_case_matches_and_there_is_no_fallback() {
        let mechanism = Mechanism {
            version: 1,
            rules: BTreeMap::new(),
            triggers: vec![data::Trigger {
                id: "侵略".to_owned(),
                title: "環境氛圍".to_owned(),
                mode: TriggerMode::Range,
                cases: vec![range_case(80.0, "淪陷邊緣")],
                preamble: String::new(),
                scope: Vec::new(),
                flag: None,
            }],
            incremental: true,
            guide: String::new(),
        };
        let tree = tree_from(&[("World.Invasion", "10")]);
        let outcome = evaluate_triggers(&tree, &mechanism, "阿濤");
        assert!(outcome.hits.is_empty());
    }

    /// {{user}} 巨集跟 {{state:路徑}} 佔位都要在命中文本裡換好。
    #[test]
    fn evaluate_triggers_replaces_state_placeholder_and_user_macro() {
        let mechanism = Mechanism {
            version: 1,
            rules: BTreeMap::new(),
            triggers: vec![data::Trigger {
                id: "招呼".to_owned(),
                title: "招呼".to_owned(),
                mode: TriggerMode::Range,
                cases: vec![data::TriggerCase {
                    when: Vec::new(),
                    text: "{{user}} 現在在 {{state:World.Location}}".to_owned(),
                }],
                preamble: String::new(),
                scope: Vec::new(),
                flag: None,
            }],
            incremental: true,
            guide: String::new(),
        };
        let tree = tree_from(&[("World.Location", "晨港")]);
        let outcome = evaluate_triggers(&tree, &mechanism, "阿濤");
        assert_eq!(
            outcome.hits.get("招呼"),
            Some(&"阿濤 現在在 晨港".to_owned())
        );

        // 路徑不存在就換成空字串，不留下沒收尾的佔位標記。
        let empty_tree = BTreeMap::new();
        let outcome = evaluate_triggers(&empty_tree, &mechanism, "阿濤");
        assert_eq!(outcome.hits.get("招呼"), Some(&"阿濤 現在在 ".to_owned()));
    }

    fn once_mechanism() -> Mechanism {
        Mechanism {
            version: 1,
            rules: BTreeMap::new(),
            triggers: vec![data::Trigger {
                id: "國變".to_owned(),
                title: "國變".to_owned(),
                mode: TriggerMode::Once,
                cases: vec![data::TriggerCase {
                    when: vec![
                        Condition::Range {
                            path: "World.Invasion".to_owned(),
                            min: Some(50.0),
                            max: None,
                            min_exclusive: false,
                            max_exclusive: false,
                            default: Some(0.0),
                        },
                        Condition::Flag {
                            path: "Events.國變".to_owned(),
                            expect: false,
                        },
                    ],
                    text: "國都淪陷。".to_owned(),
                }],
                preamble: String::new(),
                scope: Vec::new(),
                flag: Some("Events.國變".to_owned()),
            }],
            incremental: true,
            guide: String::new(),
        }
    }

    /// Once 命中：文本有了、旗標收進 flags（由 apply_block 負責釘進樹）。
    #[test]
    fn evaluate_triggers_once_hit_reports_the_flag_to_pin() {
        let mechanism = once_mechanism();
        let tree = tree_from(&[("World.Invasion", "90")]);
        let outcome = evaluate_triggers(&tree, &mechanism, "阿濤");
        assert_eq!(outcome.hits.get("國變"), Some(&"國都淪陷。".to_owned()));
        assert_eq!(outcome.flags, vec!["Events.國變".to_owned()]);
    }

    /// 一次性事件全流程：第一次求值命中→文本有了、旗標被釘成 true、記一筆 Absorbed；
    /// 同一棵樹（旗標已釘）再求值一次→不再命中，模型翻不了案。
    #[test]
    fn once_event_pins_the_flag_and_never_fires_again_on_the_same_tree() {
        let mechanism = once_mechanism();
        let mut world = world_with(&[("World.Invasion", "90")], mechanism);
        let block = crate::transport::StateBlock {
            fields: Vec::new(),
            updates: Vec::new(),
            display: String::new(),
        };

        let outcome = apply_block(&mut world, &block, "阿濤");
        assert_eq!(
            world.state.triggers.get("國變"),
            Some(&"國都淪陷。".to_owned())
        );
        assert_eq!(leaf_at(&world.state.tree, "Events.國變"), Some("true"));
        assert!(outcome
            .records
            .iter()
            .any(|record| record.kind == RecordKind::Absorbed && record.path == "Events.國變"));

        let second = apply_block(&mut world, &block, "阿濤");
        assert!(!world.state.triggers.contains_key("國變"));
        assert!(!second
            .records
            .iter()
            .any(|record| record.kind == RecordKind::Absorbed));
    }

    /// 全量桌（`!mechanism.incremental`）逐字維持現狀：不做觸發表求值。
    #[test]
    fn apply_block_skips_trigger_evaluation_for_a_full_snapshot_table() {
        let mut mechanism = once_mechanism();
        mechanism.incremental = false;
        let mut world = world_with(&[("World.Invasion", "90")], mechanism);
        let block = crate::transport::StateBlock {
            fields: Vec::new(),
            updates: Vec::new(),
            display: String::new(),
        };
        apply_block(&mut world, &block, "阿濤");
        assert!(world.state.triggers.is_empty());
        assert!(leaf_at(&world.state.tree, "Events.國變").is_none());
    }

    // ---- 全量桌跳動標記（狀態欄二期包 6）----

    #[test]
    fn full_snapshot_jump_over_threshold_is_marked_and_recorded() {
        let mechanism = mechanism_with(&[]);
        let mut world = world_with(&[("World.HP", "60")], mechanism);
        let block = crate::transport::StateBlock {
            fields: vec![(vec!["World".to_owned(), "HP".to_owned()], "100".to_owned())],
            updates: Vec::new(),
            display: String::new(),
        };
        let outcome = apply_block(&mut world, &block, "阿濤");
        assert_eq!(world.state.jumps.get("World.HP"), Some(&"+40".to_owned()));
        assert!(outcome
            .records
            .iter()
            .any(|record| record.kind == RecordKind::Jump && record.path == "World.HP"));
    }

    /// 幅度沒過絕對門檻（3→10 只差 7）：不標，不管相對幅度多誇張。
    #[test]
    fn full_snapshot_small_change_under_absolute_threshold_is_not_marked() {
        let mechanism = mechanism_with(&[]);
        let mut world = world_with(&[("World.HP", "3")], mechanism);
        let block = crate::transport::StateBlock {
            fields: vec![(vec!["World".to_owned(), "HP".to_owned()], "10".to_owned())],
            updates: Vec::new(),
            display: String::new(),
        };
        apply_block(&mut world, &block, "阿濤");
        assert!(world.state.jumps.is_empty());
    }

    /// 已標成計數器的欄位（例如卡片自己的「第 N 天」）就算幅度誇張也不標，
    /// 這是玩家點記號之後的效果，時間跳躍是那張卡的明文功能。
    #[test]
    fn full_snapshot_counter_field_is_never_marked_even_with_a_huge_jump() {
        let mechanism = mechanism_with(&[("World.Day", rule(FieldKind::Counter, None, None))]);
        let mut world = world_with(&[("World.Day", "3")], mechanism);
        let block = crate::transport::StateBlock {
            fields: vec![(vec!["World".to_owned(), "Day".to_owned()], "100".to_owned())],
            updates: Vec::new(),
            display: String::new(),
        };
        apply_block(&mut world, &block, "阿濤");
        assert!(world.state.jumps.is_empty());
    }

    /// 增量桌（本地算術權威，模型只回報變動量）一律不做跳動比對，`jumps` 維持空。
    #[test]
    fn incremental_table_never_populates_jumps() {
        let mechanism = Mechanism {
            incremental: true,
            ..Mechanism::default()
        };
        let mut world = world_with(&[("World.HP", "60")], mechanism);
        let block = crate::transport::StateBlock {
            fields: vec![(vec!["World".to_owned(), "HP".to_owned()], "100".to_owned())],
            updates: Vec::new(),
            display: String::new(),
        };
        apply_block(&mut world, &block, "阿濤");
        assert!(world.state.jumps.is_empty());
    }

    /// 真卡格式端到端：donass 的 `<StatusData>` 與 orc-cave 的 `<details>` 摺疊狀態欄，
    /// 從剝殼一路走到套用——比對吃的是模型真的會吐的字串（全形冒號、「第 N 天」這種前後綴、
    /// 中文數字的純文字欄），不是理想化的純數字。
    #[test]
    fn full_snapshot_jump_reads_real_card_state_blocks() {
        let mut world = world_with(&[], mechanism_with(&[]));
        let opening = "陆辰咬牙。\n<StatusData>\n体力:60\n好感:20\n层数:第一层\n</StatusData>";
        apply_block(
            &mut world,
            &crate::transport::extract_state_block(opening),
            "阿濤",
        );
        assert!(world.state.jumps.is_empty());

        let next = "他愣了一下。\n<StatusData>\n体力:55\n好感:70\n层数:第一层\n</StatusData>";
        apply_block(
            &mut world,
            &crate::transport::extract_state_block(next),
            "阿濤",
        );
        // 好感一輪跳 50 標出來；体力只掉 5、层数是中文數字抽不出數，兩個都不標
        assert_eq!(world.state.jumps.get("好感"), Some(&"+50".to_owned()));
        assert_eq!(world.state.jumps.len(), 1);

        let day = |n: u32| {
            format!(
                "……\n<details>\n<summary>状态栏</summary>\n<hr>\n\n- 沦陷天数：第 {n} 天\n- 当前环境：洞穴深处\n\n</details>"
            )
        };
        let mut cave = world_with(&[], mechanism_with(&[]));
        apply_block(
            &mut cave,
            &crate::transport::extract_state_block(&day(1)),
            "阿濤",
        );
        apply_block(
            &mut cave,
            &crate::transport::extract_state_block(&day(10)),
            "阿濤",
        );
        assert!(cave.state.jumps.is_empty(), "尋常推進幾天不該示警");

        apply_block(
            &mut cave,
            &crate::transport::extract_state_block(&day(60)),
            "阿濤",
        );
        assert_eq!(
            cave.state.jumps.get("沦陷天数"),
            Some(&"+50".to_owned()),
            "一口氣跳 50 天先示警，玩家點記號標成計數器之後才不再提醒"
        );
    }
}
