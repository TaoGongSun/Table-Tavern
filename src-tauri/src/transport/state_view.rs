use crate::data::{
    self, CharacterCard, InjectLevel, Mechanism, StateNode, TableState, WorldbookEntry,
};

use crate::mechanism;

use std::collections::BTreeMap;

use super::messages::replace_st_macros;

/// GM 的回合動態塊：keyword 條目＋「目前狀態」。
/// assemble_gm_messages（尾端獨立訊息）與 gm_lane_turn（resume 續聊回合尾段）共用。
/// 增量桌（mechanism.incremental）依 scope 裁切分支＋過濾葉子＋加變動標記；
/// 全量桌逐字維持現狀（不裁、不濾、不標）。
pub(super) fn gm_dynamic_block(
    keyword_entries: &[&WorldbookEntry],
    state: &TableState,
    user_name: &str,
    mechanism: &Mechanism,
    scope: &StateScope,
    lang: &str,
) -> String {
    let mut dynamic = String::new();
    if !keyword_entries.is_empty() {
        dynamic.push_str("## 世界書（只進你的上下文）\n");
        for entry in keyword_entries {
            dynamic.push_str(&format!(
                "### {}\n{}\n",
                replace_st_macros(&entry.title, user_name, None),
                replace_st_macros(&entry.content, user_name, None)
            ));
        }
    }
    let mut tree_text = String::new();
    render_state_tree(
        &mut tree_text,
        &state.tree,
        &TreeRender {
            mechanism,
            changes: &state.changes,
            hidden: &scope.hidden,
            align: scope.align,
            user_name,
            base: 0,
        },
        &mut Vec::new(),
    );
    if !state.table.is_empty() || !tree_text.is_empty() {
        if !dynamic.is_empty() {
            dynamic.push('\n');
        }
        let header = if mechanism.incremental && scope.align {
            "## 目前狀態（完整對齊，以下是系統帳上的真值，請以此為準）\n"
        } else {
            "## 目前狀態（這桌的檯面，接續它往下演）\n"
        };
        dynamic.push_str(header);
        for (key, value) in &state.table {
            let display_name = match (lang, key.as_str()) {
                ("en", "time") => "Time",
                ("en", "place") => "Place",
                ("en", "present") => "Present",
                (_, "time") => "時間",
                (_, "place") => "地點",
                (_, "present") => "在場人物",
                _ => key,
            };
            dynamic.push_str(&format!("{display_name}：{value}\n"));
        }
        dynamic.push_str(&tree_text);
    }
    if mechanism.incremental && !state.triggers.is_empty() {
        let lines: Vec<&str> = mechanism
            .triggers
            .iter()
            .filter(|trigger| scope.align || !trigger_scope_hidden(&trigger.scope, &scope.hidden))
            .filter_map(|trigger| state.triggers.get(&trigger.id))
            .map(String::as_str)
            .collect();
        if !lines.is_empty() {
            if !dynamic.is_empty() {
                dynamic.push('\n');
            }
            dynamic.push_str("## 當前情境（系統依狀態表判定的隱藏背景，不要在回覆裡複述本段）\n");
            for (index, text) in lines.iter().enumerate() {
                if index > 0 {
                    dynamic.push('\n');
                }
                dynamic.push_str(text);
                dynamic.push('\n');
            }
        }
    }
    if !state.notes.is_empty() {
        if !dynamic.is_empty() {
            dynamic.push('\n');
        }
        dynamic.push_str("## 上一輪被系統擋下的更新（請照這些現值修正）\n");
        for note in &state.notes {
            dynamic.push_str(&format!("{note}\n"));
        }
    }
    dynamic.trim_end().to_owned()
}

/// 一次注入要用的渲染參數；路徑與輸出隨遞迴走，其餘整趟固定。
struct TreeRender<'a> {
    mechanism: &'a Mechanism,
    changes: &'a BTreeMap<String, String>,
    hidden: &'a [Vec<String>],
    align: bool,
    user_name: &'a str,
    /// 縮排基準：從第幾層開始算第一級（角色線只印自己那支，要從行首印起）
    base: usize,
}

/// 樹沿用模型最容易產生的 YAML 形狀，讓本期全量注入不因資料升級漏掉任何狀態。
/// 全量桌（!mechanism.incremental）：逐字維持現狀，不裁不濾不標。
/// 增量桌：`hidden` 之外的分支才印（align 時忽略 hidden，整棵樹都印）；
/// 葉子依 inject 過濾——Turn 一律印，Snapshot 只有 align 時才印，Rare 一律不印；
/// `changes` 有值的葉子在值後面加全形括號標記。過濾後變空的分支不留空標題。
fn render_state_tree(
    output: &mut String,
    tree: &BTreeMap<String, StateNode>,
    render: &TreeRender,
    path: &mut Vec<String>,
) {
    let incremental = render.mechanism.incremental;
    for (key, node) in tree {
        path.push(key.clone());
        let indent = "  ".repeat(path.len() - 1 - render.base);
        let branch_hidden = incremental && !render.align && render.hidden.iter().any(|h| h == path);
        if !branch_hidden {
            match node {
                StateNode::Leaf(value) => {
                    let show = if incremental {
                        let rule = mechanism::rule_for_path(render.mechanism, path, Some(value));
                        match rule.inject {
                            InjectLevel::Rare => false,
                            InjectLevel::Snapshot => render.align,
                            InjectLevel::Turn => true,
                        }
                    } else {
                        true
                    };
                    if show {
                        let mark = if incremental {
                            render
                                .changes
                                .get(&path.join("."))
                                .map(|mark| format!("（{mark}）"))
                                .unwrap_or_default()
                        } else {
                            String::new()
                        };
                        output.push_str(&format!(
                            "{indent}{key}：{}{mark}\n",
                            replace_st_macros(value, render.user_name, None),
                        ));
                    }
                }
                StateNode::Branch(children) => {
                    let header = format!("{indent}{key}：\n");
                    let before = output.len();
                    output.push_str(&header);
                    render_state_tree(output, children, render, path);
                    if incremental && output.len() == before + header.len() {
                        output.truncate(before);
                    }
                }
            }
        }
        path.pop();
    }
}

/// 觸發表裁切：`trigger_scope` 正好是 `hidden` 某一條、或是其後代路徑，就該裁掉
/// （不在場角色的關係階段文本不該送）。空 `trigger_scope`＝桌級，永遠不裁。
fn trigger_scope_hidden(trigger_scope: &[String], hidden: &[Vec<String>]) -> bool {
    !trigger_scope.is_empty()
        && hidden.iter().any(|branch| {
            trigger_scope.len() >= branch.len() && trigger_scope[..branch.len()] == branch[..]
        })
}

/// 這一輪要裁掉哪些分支、要不要送全樹對齊——回合尾注入策略（包 5）。
#[derive(Debug, Clone, Default)]
pub struct StateScope {
    /// 本輪不送的分支路徑（不在場角色的那一支）。空＝不裁切。
    pub hidden: Vec<Vec<String>>,
    /// 這一輪送全樹對齊（換幕後第一輪 GM 回合）。
    pub align: bool,
}

/// 算這一輪的狀態視角：全量桌完全不裁；增量桌依在場名單裁掉不在場角色的分支
/// （玩家那支永遠送）；在場欄空著就寧可全送，不要因為模型沒報 present 就裁瞎了。
///
/// 認得出來的角色分支有兩種：綁到角色卡的，以及**與它同一個容器的手足**——
/// 一張 MVU 卡的 15 個英雄只會有幾張角色卡，剩下的手足照樣是人、照樣該裁。
/// 手足規則只在容器不是樹根時生效：頂層放的是 World／Player 這類桌級分支，掃進去會把整桌裁掉。
pub fn state_scope(
    state: &TableState,
    mechanism: &Mechanism,
    cards: &[CharacterCard],
    player: Option<&CharacterCard>,
    bindings: &BTreeMap<String, Vec<String>>,
    align: bool,
) -> StateScope {
    if !mechanism.incremental {
        return StateScope::default();
    }
    let present: Vec<String> = state
        .table
        .get("present")
        .map(String::as_str)
        .unwrap_or("")
        .split(['、', '，', ',', '／', '/', '；', ';'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect();
    if present.is_empty() {
        return StateScope {
            hidden: Vec::new(),
            align,
        };
    }
    let player_id = player.map(|player| player.id.as_str());
    let is_present = |name: &str| {
        present
            .iter()
            .any(|item| item == name || item.contains(name))
    };
    let mut hidden: Vec<Vec<String>> = Vec::new();
    let mut keep: Vec<Vec<String>> = Vec::new(); // 玩家那支永遠送，手足規則也不准碰
    let mut containers: Vec<Vec<String>> = Vec::new();
    for card in cards {
        let Some(branch) = resolve_branch(&state.tree, bindings, &card.id, &card.name) else {
            continue; // 沒有分支就沒東西可裁
        };
        if branch.len() > 1 {
            let container = branch[..branch.len() - 1].to_vec();
            if !containers.contains(&container) {
                containers.push(container);
            }
        }
        if Some(card.id.as_str()) == player_id {
            keep.push(branch);
        } else if !is_present(&card.name) {
            hidden.push(branch);
        }
    }
    // 手足：同容器裡沒有角色卡的分支，名字沒出現在在場名單就一起裁
    for container in &containers {
        let Some(StateNode::Branch(children)) = data::node_at(&state.tree, container) else {
            continue;
        };
        for (name, node) in children {
            if !matches!(node, StateNode::Branch(_)) {
                continue;
            }
            let mut path = container.clone();
            path.push(name.clone());
            if keep.contains(&path) || hidden.contains(&path) || is_present(name) {
                continue;
            }
            hidden.push(path);
        }
    }
    StateScope { hidden, align }
}

/// 角色卡對應的狀態樹分支：面板指認優先，其次全樹同名比對。
/// 指認的路徑若在樹裡不存在或不是分支，視為失效、退回自動比對。
pub fn resolve_branch(
    tree: &BTreeMap<String, StateNode>,
    bindings: &BTreeMap<String, Vec<String>>,
    card_id: &str,
    card_name: &str,
) -> Option<Vec<String>> {
    if let Some(path) = bindings.get(card_id) {
        if !path.is_empty() && matches!(data::node_at(tree, path), Some(StateNode::Branch(_))) {
            return Some(path.clone());
        }
    }
    auto_match_branch(tree, card_name)
}

/// 廣度優先找 key 完全等於卡名的分支節點（葉子不算），深度上限 3，取最淺的一筆。
fn auto_match_branch(tree: &BTreeMap<String, StateNode>, card_name: &str) -> Option<Vec<String>> {
    let mut level: Vec<(Vec<String>, &BTreeMap<String, StateNode>)> = vec![(Vec::new(), tree)];
    for _ in 0..3 {
        let mut next = Vec::new();
        for (path, branch) in &level {
            for (key, node) in *branch {
                let StateNode::Branch(children) = node else {
                    continue;
                };
                let mut candidate = path.clone();
                candidate.push(key.clone());
                if key == card_name {
                    return Some(candidate);
                }
                next.push((candidate, children));
            }
        }
        level = next;
    }
    None
}

/// 角色自己那支的狀態（唯讀，給扮演參考）。沒綁到分支或該支空的就是 None。
/// 排除 `inject == Rare` 的葉子，帶變動標記，`{{user}}` 照舊代換。
pub fn character_state_block(
    state: &TableState,
    mechanism: &Mechanism,
    branch: &[String],
    card_name: &str,
    user_name: &str,
) -> Option<String> {
    if branch.is_empty() {
        return None;
    }
    let StateNode::Branch(children) = data::node_at(&state.tree, branch)? else {
        return None; // 分支路徑指到葉子，視同沒有分支
    };
    if children.is_empty() {
        return None;
    }
    let mut body = String::new();
    let mut path = branch.to_vec();
    // align=true：除了 Rare，全部印出（角色要看自己完整的檯面，不是只看這輪變動）。
    render_state_tree(
        &mut body,
        children,
        &TreeRender {
            mechanism,
            changes: &state.changes,
            hidden: &[],
            align: true,
            user_name,
            base: branch.len(),
        },
        &mut path,
    );
    if body.is_empty() {
        return None;
    }
    Some(format!(
        "## 「{card_name}」目前的狀態（系統帳，唯讀；可以拿來演，但不要輸出任何狀態欄或更新區塊）\n{}",
        body.trim_end()
    ))
}

/// 長文字欄（inject == Snapshot）這一輪的新值：(點分路徑, 值)。
/// 只回 `state.changes` 裡有的那些；`{{user}}` 在這裡就代換掉——這批要落成 transcript
/// 系統事件（不再進回合尾的動態塊），事件文字之後不會再過巨集代換，留字面會直接漏進提示詞。
/// 全量桌（!mechanism.incremental）一律回空。
pub fn snapshot_updates(
    state: &TableState,
    mechanism: &Mechanism,
    user_name: &str,
) -> Vec<(String, String)> {
    if !mechanism.incremental || state.changes.is_empty() {
        return Vec::new();
    }
    let mut updates = Vec::new();
    collect_snapshot_updates(
        &state.tree,
        mechanism,
        &state.changes,
        user_name,
        &mut Vec::new(),
        &mut updates,
    );
    updates
}

fn collect_snapshot_updates(
    tree: &BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    changes: &BTreeMap<String, String>,
    user_name: &str,
    path: &mut Vec<String>,
    updates: &mut Vec<(String, String)>,
) {
    for (key, node) in tree {
        path.push(key.clone());
        match node {
            StateNode::Leaf(value) => {
                // 路徑就地 join 去比對 changes，不用 split('.') 反推——欄位名本身可能含 '.'。
                let path_key = path.join(".");
                if changes.contains_key(&path_key) {
                    let rule = mechanism::rule_for_path(mechanism, path, Some(value));
                    if rule.inject == InjectLevel::Snapshot {
                        updates.push((path_key, replace_st_macros(value, user_name, None)));
                    }
                }
            }
            StateNode::Branch(children) => {
                collect_snapshot_updates(children, mechanism, changes, user_name, path, updates);
            }
        }
        path.pop();
    }
}

#[cfg(test)]
mod tests;
