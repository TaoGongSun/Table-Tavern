//! 鏡像欄（card-field-overlaps-fixed-keys）：卡自帶、與固定鍵 place／present 嚴格同義的欄位（例「当前环境」
//! 「驻留/交涉角色」）。收圍欄的桌由 GM 每回合在圍欄報固定鍵，卡欄位改由 App 從平欄抄過去，模型不必雙報；
//! 觸發條件與 `{{state:…}}` 佔位照舊讀樹上的這一欄。

use std::collections::BTreeMap;

use crate::data::{self, StateNode, WorldState};

/// 可以當鏡像來源的固定鍵。time 不收：卡上的天數／日期欄多半帶計數語義，不是同一件事。
const MIRROR_KEYS: [&str; 2] = ["place", "present"];

fn segments(path: &str) -> Vec<String> {
    path.split('.').map(str::to_owned).collect()
}

/// 只寫既有的葉子：欄位被刪掉或變成分支就不再鏡像，不替它重建。空值照存成空葉子（`set_tree_value`
/// 寫空會刪鍵，下一輪就鏡像不回來）。
fn write_leaf(tree: &mut BTreeMap<String, StateNode>, path: &str, value: &str) {
    let segments = segments(path);
    let Some((last, parents)) = segments.split_last() else {
        return;
    };
    let mut branch = tree;
    for segment in parents {
        match branch.get_mut(segment) {
            Some(StateNode::Branch(child)) => branch = child,
            _ => return,
        }
    }
    if let Some(StateNode::Leaf(current)) = branch.get_mut(last) {
        if current != value {
            *current = value.to_owned();
        }
    }
}

/// 本輪模型更新都套完後（本地規則之前）：平欄有這個固定鍵就抄進鏡像欄——同輪兩邊都寫時固定鍵勝；
/// present 明確寫空時鏡像欄跟著清空。平欄沒這個鍵＝還沒人報過，鏡像欄留原值。
pub(crate) fn sync_mirrors(world: &mut WorldState) {
    for (path, key) in &world.mechanism.mirrors {
        if let Some(value) = world.state.table.get(key) {
            write_leaf(&mut world.state.tree, path, value);
        }
    }
}

/// 玩家手改平欄（`set_table_state`，不經回合套用）：這次改到的固定鍵抄進鏡像欄，清空也照清——
/// 手改清空會把平欄的鍵整個刪掉，`sync_mirrors` 看不到。
pub fn sync_mirrors_from_edit(world: &mut WorldState, fields: &BTreeMap<String, String>) {
    for (path, key) in &world.mechanism.mirrors {
        if let Some(value) = fields.get(key) {
            write_leaf(&mut world.state.tree, path, value);
        }
    }
}

/// 盤點的 MIRRORS 對照蓋到規則上（盤點是唯一標記來源：先清掉規則上既有的 `same_as`）。只在收圍欄的桌、
/// 名字與樹上頂層葉子整段精確相等時標（斜線鍵是一整段）；對不上、命中分支就丟掉該行，不建空葉、不捏初值。
/// 標上的欄位沒有規則就補一條文字規則承載標記（`adopt_mirror_marks` 收件時會拿掉）。回傳丟掉的行。
pub fn mark_survey_mirrors(
    world: &mut WorldState,
    mirrors: &BTreeMap<String, String>,
    fence_table: bool,
) -> Vec<String> {
    for rule in world.mechanism.rules.values_mut() {
        rule.same_as = None;
    }
    if !fence_table {
        return Vec::new();
    }
    let mut dropped = Vec::new();
    for (name, key) in mirrors {
        if matches!(world.state.tree.get(name), Some(StateNode::Leaf(_))) {
            world
                .mechanism
                .rules
                .entry(name.clone())
                .or_insert_with(|| data::FieldRule::for_kind(data::FieldKind::Text))
                .same_as = Some(key.clone());
        } else {
            dropped.push(format!("{name} = {key}"));
        }
    }
    dropped
}

/// 套用重構收尾：規則上的 `same_as` 標記收進 `mirrors` 並拿掉該欄的規則（不再要模型回報），
/// 條件是這桌收圍欄（`fence_table`）、標的是 place／present、路徑是樹上既有的純文字葉子、不在集合裡；
/// 其餘標記丟掉、規則照舊。收進後立刻同步一次（固定鍵有值則勝，缺席時鏡像欄留卡的初值）。
/// 回傳有沒有改到桌上狀態。
pub fn adopt_mirror_marks(world: &mut WorldState, fence_table: bool) -> bool {
    let marked: Vec<(String, String)> = world
        .mechanism
        .rules
        .iter()
        .filter_map(|(path, rule)| rule.same_as.clone().map(|key| (path.clone(), key)))
        .collect();
    if marked.is_empty() {
        return false;
    }
    for (path, key) in marked {
        let in_collection =
            world.mechanism.collections.keys().any(|collection| {
                path == *collection || path.starts_with(&format!("{collection}."))
            });
        let eligible = fence_table
            && MIRROR_KEYS.contains(&key.as_str())
            && !path.contains('*')
            && !in_collection
            && matches!(
                data::node_at(&world.state.tree, &segments(&path)),
                Some(StateNode::Leaf(_))
            );
        if eligible {
            world.mechanism.rules.remove(&path);
            world.mechanism.mirrors.insert(path, key);
        } else if let Some(rule) = world.mechanism.rules.get_mut(&path) {
            rule.same_as = None;
        }
    }
    sync_mirrors(world);
    true
}
