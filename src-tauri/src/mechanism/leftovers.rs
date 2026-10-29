//! 正文剝掉、狀態卻沒收進去的值（playdata-loss-fixes D1、11a）：退場桌 GM 照舊印的卡片狀態欄、正文尾端照抄的
//! 狀態變動段都會從正文剝掉。提交前先把這些格子列成候選，提交後跟樹比對，沒收進去的由呼叫端寫進本回合的
//! 「狀態更新」系統事件（進逐字稿、隨歷史送 GM、玩家看得到），不只靠每輪覆寫、有字數上限的回饋句。

use crate::data::{self, StateNode, WorldState};
use crate::transport::StateBlock;
use std::collections::BTreeMap;

use super::parse::parse_block;
use super::prefix::split_prefix;
use super::tree::canonical_path;
use super::types::Parsed;

/// 一個候選格子：路徑分段、回覆裡寫的值。
pub type Cell = (Vec<String>, String);

/// 回覆裡卡片自帶狀態區塊（`<details>` 狀態欄、`<status…>`）的格子：`block.fields` 扣掉 ```state 圍欄那份。
/// 沒有卡片區塊回空。
pub fn card_block_cells(block: &StateBlock) -> Vec<Cell> {
    let Some(fenced) = &block.fenced else {
        return Vec::new();
    };
    block
        .fields
        .iter()
        .filter(|field| !fenced.fields.contains(field))
        .cloned()
        .collect()
}

/// 退場桌回覆裡要在提交後核對的卡片格子：本回合圍欄與更新區塊寫到的格子以正式回報為準，不列（免得把被取代的
/// 舊卡片值當成沒套用再追加一次）。
pub fn retired_card_pending(block: &StateBlock, tree: &BTreeMap<String, StateNode>) -> Vec<Cell> {
    let Some(fenced) = &block.fenced else {
        return Vec::new();
    };
    let reported = StateBlock {
        fields: fenced.fields.clone(),
        updates: block.updates.clone(),
        ..Default::default()
    };
    unreported(&card_block_cells(block), &reported, tree)
}

fn pointer(path: &[String]) -> String {
    path.iter()
        .map(|segment| format!("/{}", segment.replace('~', "~0").replace('/', "~1")))
        .collect()
}

fn leaf_at<'a>(tree: &'a BTreeMap<String, StateNode>, path: &[String]) -> Option<&'a str> {
    match data::node_at(tree, path) {
        Some(StateNode::Leaf(value)) => Some(value),
        _ => None,
    }
}

/// 本回合正式回報寫到的路徑（圍欄欄位、更新區塊的 JSONPatch 路徑與 `_.set('…')` 類指令的路徑），都對齊成樹上的
/// 實際鍵（簡繁別名算同一格）。
fn reported_paths(block: &StateBlock, tree: &BTreeMap<String, StateNode>) -> Vec<Vec<String>> {
    static LEGACY: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let legacy = LEGACY.get_or_init(|| {
        regex::Regex::new(r#"_\.\w+\(\s*['"]([^'"]+)['"]"#).expect("legacy command regex")
    });
    let mut paths: Vec<Vec<String>> = block.fields.iter().map(|(path, _)| path.clone()).collect();
    for update in &block.updates {
        for parsed in parse_block(update) {
            if let Parsed::Patch(patch) = parsed {
                paths.push(patch.path);
                if !patch.from.is_empty() {
                    paths.push(patch.from);
                }
            }
        }
        for found in legacy.captures_iter(update) {
            paths.push(found[1].split('.').map(str::to_owned).collect());
        }
    }
    paths
        .into_iter()
        .map(|path| canonical_path(tree, &path, true).unwrap_or(path))
        .collect()
}

/// 候選裡本回合正式回報沒寫到的（對齊後，寫到父路徑整項也算寫到）。寫到的格子由正式回報為準：不補收、
/// 提交後也不當成「沒套用」再追加舊值。
pub fn unreported(
    cells: &[Cell],
    block: &StateBlock,
    tree: &BTreeMap<String, StateNode>,
) -> Vec<Cell> {
    let reported = reported_paths(block, tree);
    cells
        .iter()
        .filter(|(path, _)| {
            let path = canonical_path(tree, path, true).unwrap_or_else(|_| path.clone());
            !reported.iter().any(|written| path.starts_with(written))
        })
        .cloned()
        .collect()
}

/// 照抄段的新值補收：值跟套用前不同、本回合正式回報沒寫到的行（`unreported`），合成一段 `<JSONPatch>`
/// （數字對數字寫 delta，其餘 replace），交給各桌真正的提交路徑（非 zod 桌併進更新區塊、zod 桌接在送引擎的回覆
/// 後面；接管桌只收更新區塊、zod 不看圍欄，所以不走 `block.fields`）。寫到的不補，免得 delta 算兩次。
/// 回（候選, 合成的區塊）。
pub fn echo_patch(
    lines: &[Cell],
    block: &StateBlock,
    tree: &BTreeMap<String, StateNode>,
) -> (Vec<Cell>, Option<String>) {
    let fresh: Vec<Cell> = unreported(lines, block, tree)
        .into_iter()
        .filter(|(path, value)| leaf_at(tree, path).map(str::trim) != Some(value.as_str()))
        .collect();
    if fresh.is_empty() {
        return (fresh, None);
    }
    // 兩邊都是數字就寫成 delta（數字欄只收增減量），其餘 replace
    let number = |text: &str| text.trim().parse::<f64>().ok().filter(|n| n.is_finite());
    let ops: Vec<serde_json::Value> = fresh
        .iter()
        .map(|(path, value)| {
            let current = leaf_at(tree, path).and_then(number);
            match (current, number(value)) {
                (Some(current), Some(new)) => {
                    let change = new - current;
                    let change = if change.fract() == 0.0 && change.abs() < 1e15 {
                        serde_json::Value::from(change as i64)
                    } else {
                        serde_json::Value::from(change)
                    };
                    serde_json::json!({ "op": "delta", "path": pointer(path), "value": change })
                }
                _ => serde_json::json!({ "op": "replace", "path": pointer(path), "value": value }),
            }
        })
        .collect();
    let patch = format!("<JSONPatch>{}</JSONPatch>", serde_json::Value::Array(ops));
    (fresh, Some(patch))
}

/// 寫的值跟樹上（或平欄）現值算不算同一個：去空白相等，或剝掉顯示前綴後相等。拿不準（例如「第 30 天」對 3）
/// 一律當沒收，完整留進事件。
fn same(written: &str, current: &str) -> bool {
    let (written, current) = (written.trim(), current.trim());
    written == current || split_prefix(written).is_some_and(|(_, rest)| rest.trim() == current)
}

/// 提交後還沒收進狀態的候選（路徑以點分、值照原文），同一路徑只留最後一筆。
pub fn unapplied(state: &WorldState, cells: &[Cell]) -> Vec<(String, String)> {
    let tree = &state.state.tree;
    let mut out: Vec<(String, String)> = Vec::new();
    for (path, value) in cells {
        let target = canonical_path(tree, path, true).unwrap_or_else(|_| path.clone());
        let kept = match leaf_at(tree, &target) {
            Some(current) => same(value, current),
            None => {
                path.len() == 1
                    && state
                        .state
                        .table
                        .get(&path[0])
                        .is_some_and(|current| same(value, current))
            }
        };
        if kept {
            continue;
        }
        let shown = path.join(".");
        out.retain(|(existing, _)| *existing != shown);
        out.push((shown, value.clone()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_equal_values_count_as_kept() {
        assert!(same("💪 15", "15"));
        assert!(!same("15", "5"));
        assert!(!same("第 30 天", "3"), "拿不準就當沒收");
        assert!(!same("大雪", "小雪"));
    }

    /// 本回合更新區塊寫別的鍵（金币）不算寫到「金」：照樣合成；指令或 JSONPatch 寫到同一格才不合成。
    #[test]
    fn only_quoted_paths_or_pointers_count_as_mentioned() {
        let tree = BTreeMap::from([("金".to_owned(), StateNode::Leaf("3".to_owned()))]);
        let lines = vec![(vec!["金".to_owned()], "9".to_owned())];
        for (update, patched) in [
            ("_.set('金币', 1);", true),
            ("_.set('金', 1);", false),
            (
                r#"<JSONPatch>[{"op":"delta","path":"/金","value":1}]</JSONPatch>"#,
                false,
            ),
        ] {
            let block = StateBlock {
                updates: vec![update.to_owned()],
                ..Default::default()
            };
            let (_, patch) = echo_patch(&lines, &block, &tree);
            assert_eq!(patch.is_some(), patched, "{update}");
        }
    }
}
