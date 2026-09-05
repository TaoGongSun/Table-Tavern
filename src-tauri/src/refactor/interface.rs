use crate::data::{FieldRule, StateNode};
use std::collections::{BTreeMap, BTreeSet};

/// 介面產物的路徑正規化：模型會把同一份狀態欄輸出成兩套重複結構（頂層一套＋「状态栏」
/// 之類的頂層分支再鏡像一套），殼佔位符只綁其中一側，GM 執行期挑另一側寫，殼綁那側
/// 永遠空字串、面板死在初始文字。只認精確別名 `W.p ↔ p`，不採相似度：
/// - 有殼：佔位符引用哪側，哪側是正典；兩側都被引用＝產物自相矛盾，Err 拒套不猜。
/// - 無殼（或殼沒引用任何別名對）：分支下每葉在根層都有精確對應（完整鏡像）才折疊，
///   正典取根層短路徑；不是完整鏡像就整份不動。
/// 至少兩對別名才算鏡像分支（單葉同名視為巧合）。值合併：別名側空＝取正典；正典空＝搬
/// 別名值；兩側非空且不同＝Err。別名分支多出的葉改掛正典側路徑；rules 跟著 remap（同
/// key 規則不同＝Err），最後剔除不在正規化後葉集合的懸空規則。呼叫端必須在 apply 寫入
/// 任何檔之前跑（preflight），Err 才能保證零落檔。
pub(super) fn normalize_interface_paths(
    state_fields: &serde_json::Value,
    shell: Option<&str>,
    rules: &BTreeMap<String, FieldRule>,
) -> Result<(serde_json::Value, BTreeMap<String, FieldRule>), String> {
    let Some(root_object) = state_fields.as_object() else {
        return Ok((state_fields.clone(), rules.clone()));
    };
    let mut leaves = BTreeMap::new();
    flatten_leaves("", state_fields, &mut leaves);
    let placeholders = shell.map(shell_placeholders).unwrap_or_default();

    let mut merged = leaves.clone();
    // 別名葉路徑 → 正典葉路徑；rules remap 靠它。
    let mut alias_of: BTreeMap<String, String> = BTreeMap::new();

    for (branch, value) in root_object {
        if !value.is_object() {
            continue;
        }
        let prefix = format!("{branch}.");
        let branch_leaves: Vec<&String> =
            leaves.keys().filter(|path| path.starts_with(&prefix)).collect();
        // 別名對：剝掉分支前綴後，樹上其他位置有一模一樣的葉路徑。
        let pairs: Vec<(String, String)> = branch_leaves
            .iter()
            .filter_map(|nested| {
                let stripped = &nested[prefix.len()..];
                (!stripped.starts_with(&prefix) && leaves.contains_key(stripped))
                    .then(|| ((*nested).clone(), stripped.to_owned()))
            })
            .collect();
        if pairs.len() < 2 {
            continue;
        }
        let nested_referenced = pairs.iter().any(|(nested, _)| placeholders.contains(nested));
        let root_referenced = pairs.iter().any(|(_, root)| placeholders.contains(root));
        let canon_is_root = match (nested_referenced, root_referenced) {
            (true, true) => {
                return Err(format!(
                    "介面產物自相矛盾：渲染殼同時綁定「{branch}.…」與頂層兩套路徑，請重新執行重構"
                ));
            }
            (true, false) => false,
            (false, true) => true,
            // 殼沒表態：完整鏡像（分支每葉都有對應）才折疊，正典取根層短路徑。
            (false, false) => {
                if pairs.len() != branch_leaves.len() {
                    continue;
                }
                true
            }
        };
        for (nested, root_path) in &pairs {
            let (canon, alias) = if canon_is_root { (root_path, nested) } else { (nested, root_path) };
            let canon_value = merged.get(canon).cloned().unwrap_or(serde_json::Value::Null);
            let alias_value = merged.get(alias).cloned().unwrap_or(serde_json::Value::Null);
            if !is_empty_value(&alias_value) {
                if is_empty_value(&canon_value) {
                    merged.insert(canon.clone(), alias_value);
                } else if canon_value != alias_value {
                    return Err(format!(
                        "介面產物同一欄位兩套初始值不一致：「{alias}」＝{alias_value}、「{canon}」＝{canon_value}，請重新執行重構"
                    ));
                }
            }
            merged.remove(alias);
            alias_of.insert(alias.clone(), canon.clone());
        }
        // 正典在根層時，鏡像分支多出的葉（根層沒有對應的）改掛根層路徑，值不丟。
        if canon_is_root {
            for nested in &branch_leaves {
                let stripped = nested[prefix.len()..].to_owned();
                if let Some(value) = merged.remove(*nested) {
                    if let Some(existing) = merged.get(&stripped) {
                        if !is_empty_value(existing) && !is_empty_value(&value) && *existing != value {
                            return Err(format!(
                                "介面產物同一欄位兩套初始值不一致：「{nested}」＝{value}、「{stripped}」＝{existing}，請重新執行重構"
                            ));
                        }
                        if is_empty_value(existing) && !is_empty_value(&value) {
                            merged.insert(stripped.clone(), value);
                        }
                    } else {
                        merged.insert(stripped.clone(), value);
                    }
                    alias_of.insert((*nested).clone(), stripped);
                }
            }
        }
    }

    let mut normalized_rules: BTreeMap<String, FieldRule> = BTreeMap::new();
    for (path, rule) in rules {
        let target = alias_of.get(path).cloned().unwrap_or_else(|| path.clone());
        if let Some(existing) = normalized_rules.get(&target) {
            if existing != rule {
                return Err(format!(
                    "介面產物同一欄位兩套規則不一致：「{path}」與「{target}」，請重新執行重構"
                ));
            }
            continue;
        }
        normalized_rules.insert(target, rule.clone());
    }
    normalized_rules.retain(|path, _| merged.contains_key(path));

    Ok((unflatten(&merged)?, normalized_rules))
}

/// 深度優先攤平：葉＝任何非物件值（字串／數字／陣列都原樣保留），路徑點分。空物件沒有葉，
/// 攤平後自然消失——空分支對狀態樹沒有意義。
fn flatten_leaves(
    prefix: &str,
    value: &serde_json::Value,
    out: &mut BTreeMap<String, serde_json::Value>,
) {
    match value.as_object() {
        Some(object) => {
            for (key, child) in object {
                let path =
                    if prefix.is_empty() { key.clone() } else { format!("{prefix}.{key}") };
                flatten_leaves(&path, child, out);
            }
        }
        None => {
            out.insert(prefix.to_owned(), value.clone());
        }
    }
}

/// 葉路徑集合還原成巢狀物件。路徑互為前綴（「地點」既是葉又是「地點.x」的分支）＝結構
/// 矛盾，Err——正常攤平不會產生，只有折疊搬移撞到才會。
fn unflatten(leaves: &BTreeMap<String, serde_json::Value>) -> Result<serde_json::Value, String> {
    let mut root = serde_json::Map::new();
    for (path, value) in leaves {
        let mut node = &mut root;
        let mut segments = path.split('.').peekable();
        while let Some(segment) = segments.next() {
            if segments.peek().is_none() {
                if node.get(segment).is_some_and(serde_json::Value::is_object) {
                    return Err(format!("介面產物欄位路徑互相衝突：「{path}」，請重新執行重構"));
                }
                node.insert(segment.to_owned(), value.clone());
            } else {
                let child = node
                    .entry(segment.to_owned())
                    .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
                match child.as_object_mut() {
                    Some(_) => {
                        node = child.as_object_mut().unwrap();
                    }
                    None => {
                        return Err(format!(
                            "介面產物欄位路徑互相衝突：「{path}」，請重新執行重構"
                        ));
                    }
                }
            }
        }
    }
    Ok(serde_json::Value::Object(root))
}

fn is_empty_value(value: &serde_json::Value) -> bool {
    value.is_null() || value.as_str().is_some_and(|text| text.trim().is_empty())
}

/// 殼裡所有 `{{路徑}}` 佔位符（trim 過）。
fn shell_placeholders(shell: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = shell;
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else { break };
        out.insert(after[..end].trim().to_owned());
        rest = &after[end + 2..];
    }
    out
}

/// state_fields 是物件時整份重建狀態樹：同名頂層鍵保留目前值，其餘舊鍵一律捨棄；非物件產物
/// 則不動任何狀態，避免壞產物清空整桌。新欄位的 JSON 轉換集中在 json_to_state_node。
pub(super) fn rebuild_state_fields(tree: &mut BTreeMap<String, StateNode>, jumps: &mut BTreeMap<String, String>, state_fields: &serde_json::Value) {
    let Some(object) = state_fields.as_object() else {
        return;
    };
    let rebuilt = object.iter().map(|(key, value)| (
        key.clone(),
        tree.get(key).cloned().unwrap_or_else(|| json_to_state_node(value)),
    )).collect();
    *tree = rebuilt;
    jumps.retain(|path, _| tree.contains_key(path.split('.').next().unwrap_or_default()));
}

fn json_to_state_node(value: &serde_json::Value) -> StateNode {
    match value {
        serde_json::Value::Object(object) => StateNode::Branch(
            object
                .iter()
                .map(|(key, value)| (key.clone(), json_to_state_node(value)))
                .collect(),
        ),
        serde_json::Value::Array(items) => StateNode::Branch(
            items
                .iter()
                .enumerate()
                .map(|(index, item)| (index.to_string(), json_to_state_node(item)))
                .collect(),
        ),
        serde_json::Value::String(text) => StateNode::Leaf(text.clone()),
        serde_json::Value::Number(number) => StateNode::Leaf(number.to_string()),
        serde_json::Value::Bool(flag) => StateNode::Leaf(flag.to_string()),
        serde_json::Value::Null => StateNode::Leaf(String::new()),
    }
}
