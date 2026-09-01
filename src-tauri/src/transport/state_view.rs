use crate::data::{self, CharacterCard, InjectLevel, Mechanism, StateNode, TableState, WorldbookEntry};

use crate::mechanism;

use std::collections::BTreeMap;

use super::messages::{replace_st_macros};



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
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::data::{self, AppConfig, CharacterCard, DataResult, FieldKind, FieldRule, InjectLevel, Mechanism, StateNode, TableState, Tier, TranscriptEvent, TranscriptKind, Visibility, WorldbookEntry};
    #[allow(unused_imports)]
    use crate::mechanism;
    #[allow(unused_imports)]
    use std::collections::{BTreeMap, BTreeSet};
    #[allow(unused_imports)]
    use super::super::test_support::{card, event, worldbook_entry};
    #[allow(unused_imports)]
    use super::super::messages::*;
    #[allow(unused_imports)]
    use super::super::context::*;
    #[allow(unused_imports)]
    use super::super::assemble::*;
    #[allow(unused_imports)]
    use super::super::arrivals::*;
    #[allow(unused_imports)]
    use super::super::turns::*;
    #[allow(unused_imports)]
    use super::super::response::*;
    #[allow(unused_imports)]
    use super::super::client::*;

    #[test]
    fn gm_dynamic_block_renders_nested_tree() {
        let state = TableState {
            table: std::collections::BTreeMap::new(),
            tree: std::collections::BTreeMap::from([(
                "World".to_owned(),
                StateNode::Branch(std::collections::BTreeMap::from([(
                    "城市".to_owned(),
                    StateNode::Leaf("晨港".to_owned()),
                )])),
            )]),
            notes: Vec::new(),
            changes: std::collections::BTreeMap::new(),
            triggers: std::collections::BTreeMap::new(),
            jumps: std::collections::BTreeMap::new(),
        };
        let dynamic = gm_dynamic_block(
            &[],
            &state,
            "阿濤",
            &Mechanism::default(),
            &StateScope::default(),
            "zh-TW",
        );
        assert!(dynamic.contains("World：\n  城市：晨港"));
    }

    /// 初始樹保留的玩家巨集只在送進這桌模型上下文前才換成實名。
    #[test]
    fn gm_dynamic_block_replaces_user_macro_in_tree_leaves() {
        let state = TableState {
            table: std::collections::BTreeMap::new(),
            tree: std::collections::BTreeMap::from([(
                "Player".to_owned(),
                StateNode::Branch(std::collections::BTreeMap::from([(
                    "Name".to_owned(),
                    StateNode::Leaf("{{user}}".to_owned()),
                )])),
            )]),
            notes: Vec::new(),
            changes: std::collections::BTreeMap::new(),
            triggers: std::collections::BTreeMap::new(),
            jumps: std::collections::BTreeMap::new(),
        };

        let dynamic = gm_dynamic_block(
            &[],
            &state,
            "阿濤",
            &Mechanism::default(),
            &StateScope::default(),
            "zh-TW",
        );
        assert!(dynamic.contains("Name：阿濤"));
        assert!(!dynamic.contains("{{user}}"));
    }

    #[test]
    fn gm_dynamic_block_prints_notes_after_current_state_and_hides_when_empty() {
        let with_notes = TableState {
            table: std::collections::BTreeMap::new(),
            tree: std::collections::BTreeMap::new(),
            notes: vec!["World.HP 已夾在範圍內，目前值 100。".to_owned()],
            changes: std::collections::BTreeMap::new(),
            triggers: std::collections::BTreeMap::new(),
            jumps: std::collections::BTreeMap::new(),
        };
        let dynamic = gm_dynamic_block(
            &[],
            &with_notes,
            "阿濤",
            &Mechanism::default(),
            &StateScope::default(),
            "zh-TW",
        );
        assert!(dynamic.contains("## 上一輪被系統擋下的更新（請照這些現值修正）"));
        assert!(dynamic.contains("World.HP 已夾在範圍內，目前值 100。"));

        let without_notes = TableState::default();
        let dynamic = gm_dynamic_block(
            &[],
            &without_notes,
            "阿濤",
            &Mechanism::default(),
            &StateScope::default(),
            "zh-TW",
        );
        assert!(!dynamic.contains("上一輪被系統擋下的更新"));
    }

    // ---- gm_dynamic_block：觸發表命中文本的「當前情境」段 ----

    fn trigger_with_scope(id: &str, scope: &[&str]) -> data::Trigger {
        data::Trigger {
            id: id.to_owned(),
            title: id.to_owned(),
            mode: data::TriggerMode::Range,
            cases: Vec::new(),
            preamble: String::new(),
            scope: scope.iter().map(|segment| (*segment).to_owned()).collect(),
            flag: None,
        }
    }

    #[test]
    fn gm_dynamic_block_prints_trigger_hits_after_current_state() {
        let mechanism = Mechanism {
            version: 1,
            rules: BTreeMap::new(),
            triggers: vec![trigger_with_scope("侵略", &[])],
            incremental: true,
            guide: String::new(),
        };
        let state = TableState {
            table: BTreeMap::from([("time".to_owned(), "黃昏".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::from([("侵略".to_owned(), "戰雲密布".to_owned())]),
            jumps: BTreeMap::new(),
        };
        let dynamic = gm_dynamic_block(
            &[],
            &state,
            "阿濤",
            &mechanism,
            &StateScope::default(),
            "zh-TW",
        );
        let state_pos = dynamic.find("## 目前狀態").expect("目前狀態應該有印");
        let trigger_pos = dynamic.find("## 當前情境").expect("當前情境應該有印");
        assert!(trigger_pos > state_pos);
        assert!(dynamic.contains("戰雲密布"));
    }

    #[test]
    fn gm_dynamic_block_hides_trigger_section_when_state_triggers_is_empty() {
        let mechanism = Mechanism {
            version: 1,
            rules: BTreeMap::new(),
            triggers: vec![trigger_with_scope("侵略", &[])],
            incremental: true,
            guide: String::new(),
        };
        let state = TableState {
            table: BTreeMap::from([("time".to_owned(), "黃昏".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        let dynamic = gm_dynamic_block(
            &[],
            &state,
            "阿濤",
            &mechanism,
            &StateScope::default(),
            "zh-TW",
        );
        assert!(!dynamic.contains("當前情境"));
    }

    /// 不在場角色那支被裁掉時，牽到那支的觸發文本不該送；`align`（換幕全樹對齊）
    /// 忽略裁切，照樣全印。
    #[test]
    fn gm_dynamic_block_hides_trigger_scoped_to_a_hidden_branch_but_prints_when_aligned() {
        let mechanism = Mechanism {
            version: 1,
            rules: BTreeMap::new(),
            triggers: vec![
                trigger_with_scope("亞瑟關係", &["Heroes", "亞瑟"]),
                trigger_with_scope("世界氛圍", &[]),
            ],
            incremental: true,
            guide: String::new(),
        };
        let state = TableState {
            table: BTreeMap::from([("time".to_owned(), "黃昏".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::from([
                ("亞瑟關係".to_owned(), "亞瑟關係文本".to_owned()),
                ("世界氛圍".to_owned(), "世界氛圍文本".to_owned()),
            ]),
            jumps: BTreeMap::new(),
        };
        let hidden = vec![vec!["Heroes".to_owned(), "亞瑟".to_owned()]];
        let scope = StateScope {
            hidden: hidden.clone(),
            align: false,
        };
        let dynamic = gm_dynamic_block(&[], &state, "阿濤", &mechanism, &scope, "zh-TW");
        assert!(!dynamic.contains("亞瑟關係文本"));
        assert!(dynamic.contains("世界氛圍文本"));

        let aligned = StateScope {
            hidden,
            align: true,
        };
        let dynamic = gm_dynamic_block(&[], &state, "阿濤", &mechanism, &aligned, "zh-TW");
        assert!(dynamic.contains("亞瑟關係文本"));
    }

    /// scope 比隱藏分支更深（後代路徑）也要跟著裁——亞瑟底下的好感細節同樣屬於他那支。
    #[test]
    fn gm_dynamic_block_hides_trigger_scoped_to_a_descendant_of_a_hidden_branch() {
        let mechanism = Mechanism {
            version: 1,
            rules: BTreeMap::new(),
            triggers: vec![trigger_with_scope(
                "亞瑟細節",
                &["Heroes", "亞瑟", "Affection"],
            )],
            incremental: true,
            guide: String::new(),
        };
        let state = TableState {
            table: BTreeMap::from([("time".to_owned(), "黃昏".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::from([("亞瑟細節".to_owned(), "亞瑟細節文本".to_owned())]),
            jumps: BTreeMap::new(),
        };
        let scope = StateScope {
            hidden: vec![vec!["Heroes".to_owned(), "亞瑟".to_owned()]],
            align: false,
        };
        let dynamic = gm_dynamic_block(&[], &state, "阿濤", &mechanism, &scope, "zh-TW");
        assert!(!dynamic.contains("亞瑟細節文本"));
    }

    #[test]
    fn gm_dynamic_block_orders_triggers_by_mechanism_list_not_by_map_key() {
        let mechanism = Mechanism {
            version: 1,
            rules: BTreeMap::new(),
            // 刻意讓清單順序跟字典序相反，確認印出順序跟著 Vec 走。
            triggers: vec![trigger_with_scope("乙", &[]), trigger_with_scope("甲", &[])],
            incremental: true,
            guide: String::new(),
        };
        let state = TableState {
            table: BTreeMap::from([("time".to_owned(), "黃昏".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::from([
                ("甲".to_owned(), "甲文本".to_owned()),
                ("乙".to_owned(), "乙文本".to_owned()),
            ]),
            jumps: BTreeMap::new(),
        };
        let dynamic = gm_dynamic_block(
            &[],
            &state,
            "阿濤",
            &mechanism,
            &StateScope::default(),
            "zh-TW",
        );
        let pos_b = dynamic.find("乙文本").unwrap();
        let pos_a = dynamic.find("甲文本").unwrap();
        assert!(pos_b < pos_a);
    }

    /// 全量桌（`!mechanism.incremental`）逐字維持現狀：就算 `state.triggers` 有值也不印。
    #[test]
    fn gm_dynamic_block_never_prints_trigger_section_for_a_full_snapshot_table() {
        let state = TableState {
            table: BTreeMap::from([("time".to_owned(), "黃昏".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::from([("侵略".to_owned(), "戰雲密布".to_owned())]),
            jumps: BTreeMap::new(),
        };
        let dynamic = gm_dynamic_block(
            &[],
            &state,
            "阿濤",
            &Mechanism::default(),
            &StateScope::default(),
            "zh-TW",
        );
        assert!(!dynamic.contains("當前情境"));
        assert!(!dynamic.contains("戰雲密布"));
    }

    // ---- 狀態欄二期包 5：注入策略＋分支切割 ----

    #[test]
    fn state_scope_hides_absent_characters_keeps_player_and_present_and_is_disabled_by_empty_present(
    ) {
        let arthur = card("arthur-id", "亞瑟", "", "");
        let crow = card("crow-id", "鴉", "", "");
        let player = card("player-id", "阿濤", "", "");
        let tree = BTreeMap::from([
            (
                "亞瑟".to_owned(),
                StateNode::Branch(BTreeMap::from([(
                    "HP".to_owned(),
                    StateNode::Leaf("100".to_owned()),
                )])),
            ),
            (
                "鴉".to_owned(),
                StateNode::Branch(BTreeMap::from([(
                    "HP".to_owned(),
                    StateNode::Leaf("50".to_owned()),
                )])),
            ),
        ]);
        let mechanism = Mechanism {
            incremental: true,
            ..Mechanism::default()
        };
        let bindings = BTreeMap::new();

        let mut present_state = TableState {
            tree: tree.clone(),
            ..TableState::default()
        };
        present_state
            .table
            .insert("present".to_owned(), "亞瑟".to_owned());
        let scope = state_scope(
            &present_state,
            &mechanism,
            &[arthur.clone(), crow.clone()],
            Some(&player),
            &bindings,
            false,
        );
        assert_eq!(scope.hidden, vec![vec!["鴉".to_owned()]]);
        assert!(!scope.align);

        // present 欄空著＝寧可全送，不要因為模型沒報 present 就裁瞎了
        let empty_present_state = TableState {
            tree,
            ..TableState::default()
        };
        let scope = state_scope(
            &empty_present_state,
            &mechanism,
            &[arthur, crow],
            Some(&player),
            &bindings,
            true,
        );
        assert!(scope.hidden.is_empty());
        assert!(scope.align);

        // 全量桌完全不裁、不對齊
        let scope = state_scope(
            &present_state,
            &Mechanism::default(),
            &[],
            None,
            &bindings,
            true,
        );
        assert!(scope.hidden.is_empty());
        assert!(!scope.align);
    }

    /// 手足規則：容器裡有一支綁到角色卡，同容器其餘分支就一律當人看——
    /// MVU 卡 15 個英雄只會有幾張角色卡，剩下的沒卡也該裁。頂層不套這條（會把 World 裁掉）。
    #[test]
    fn state_scope_hides_uncarded_siblings_in_the_same_container_but_never_at_the_top_level() {
        let hero = |hp: &str| {
            StateNode::Branch(BTreeMap::from([(
                "HP".to_owned(),
                StateNode::Leaf(hp.to_owned()),
            )]))
        };
        let mut state = TableState {
            tree: BTreeMap::from([
                (
                    "World".to_owned(),
                    StateNode::Branch(BTreeMap::from([(
                        "Invasion".to_owned(),
                        StateNode::Leaf("35".to_owned()),
                    )])),
                ),
                (
                    "Heroes".to_owned(),
                    StateNode::Branch(BTreeMap::from([
                        ("亞瑟".to_owned(), hero("100")),
                        ("鴉".to_owned(), hero("50")),
                        ("諾亞".to_owned(), hero("70")),
                    ])),
                ),
            ]),
            ..TableState::default()
        };
        state.table.insert("present".to_owned(), "諾亞".to_owned());
        let mechanism = Mechanism {
            incremental: true,
            ..Mechanism::default()
        };
        // 只有亞瑟有角色卡：他不在場所以被裁，沒卡的鴉跟著被裁，在場的諾亞留著
        let scope = state_scope(
            &state,
            &mechanism,
            &[card("arthur-id", "亞瑟", "", "")],
            None,
            &BTreeMap::new(),
            false,
        );
        assert!(scope
            .hidden
            .contains(&vec!["Heroes".to_owned(), "亞瑟".to_owned()]));
        assert!(scope
            .hidden
            .contains(&vec!["Heroes".to_owned(), "鴉".to_owned()]));
        assert!(!scope
            .hidden
            .contains(&vec!["Heroes".to_owned(), "諾亞".to_owned()]));
        // 桌級分支不受手足規則波及
        assert!(!scope.hidden.contains(&vec!["World".to_owned()]));
        assert!(!scope.hidden.contains(&vec!["Heroes".to_owned()]));
    }

    #[test]
    fn incremental_round_tail_hides_absent_branch_snapshot_and_rare_but_shows_turn_with_marks() {
        let mechanism = Mechanism {
            incremental: true,
            guide: String::new(),
            rules: BTreeMap::from([(
                "World.Secret".to_owned(),
                FieldRule::for_kind(FieldKind::ReadOnly),
            )]),
            ..Mechanism::default()
        };
        let mut state = TableState::default();
        state.table.insert("present".to_owned(), "亞瑟".to_owned());
        state.tree = BTreeMap::from([
            (
                "World".to_owned(),
                StateNode::Branch(BTreeMap::from([
                    ("HP".to_owned(), StateNode::Leaf("100".to_owned())),
                    ("Desc".to_owned(), StateNode::Leaf("晨港".to_owned())),
                    ("Secret".to_owned(), StateNode::Leaf("藏寶圖".to_owned())),
                ])),
            ),
            (
                "亞瑟".to_owned(),
                StateNode::Branch(BTreeMap::from([(
                    "HP".to_owned(),
                    StateNode::Leaf("80".to_owned()),
                )])),
            ),
            (
                "鴉".to_owned(),
                StateNode::Branch(BTreeMap::from([(
                    "HP".to_owned(),
                    StateNode::Leaf("50".to_owned()),
                )])),
            ),
        ]);
        state.changes.insert("World.HP".to_owned(), "+5".to_owned());

        // 平常輪：不在場的「鴉」整支不印；Snapshot（Desc）不印；Rare（Secret）不印；
        // Turn（HP）印，且帶變動標記。
        let scope = StateScope {
            hidden: vec![vec!["鴉".to_owned()]],
            align: false,
        };
        let dynamic = gm_dynamic_block(&[], &state, "阿濤", &mechanism, &scope, "zh-TW");
        assert!(dynamic.contains("## 目前狀態（這桌的檯面，接續它往下演）"));
        assert!(dynamic.contains("HP：100（+5）"));
        assert!(!dynamic.contains("Desc"));
        assert!(!dynamic.contains("Secret"));
        assert!(!dynamic.contains("鴉"));
        assert!(dynamic.contains("亞瑟"));

        // 對齊輪：忽略 hidden、Snapshot 也印，只有 Rare 還是不印；標題換成對齊版。
        let align_scope = StateScope {
            hidden: vec![vec!["鴉".to_owned()]],
            align: true,
        };
        let aligned = gm_dynamic_block(&[], &state, "阿濤", &mechanism, &align_scope, "zh-TW");
        assert!(aligned.contains("## 目前狀態（完整對齊，以下是系統帳上的真值，請以此為準）"));
        assert!(aligned.contains("Desc：晨港"));
        assert!(!aligned.contains("Secret"));
        assert!(aligned.contains("鴉"));
    }

    /// 全量桌（!mechanism.incremental）逐字維持現狀：不管 mechanism.rules、changes、
    /// scope 塞了什麼，輸出都跟本包以前的行為一樣——不裁、不濾、不標。
    #[test]
    fn full_scale_table_renders_everything_verbatim_ignoring_rules_scope_and_changes() {
        let mechanism = Mechanism {
            incremental: false,
            guide: String::new(),
            rules: BTreeMap::from([(
                "World.Secret".to_owned(),
                FieldRule::for_kind(FieldKind::ReadOnly),
            )]),
            ..Mechanism::default()
        };
        let mut state = TableState {
            tree: BTreeMap::from([(
                "World".to_owned(),
                StateNode::Branch(BTreeMap::from([
                    ("HP".to_owned(), StateNode::Leaf("100".to_owned())),
                    ("Desc".to_owned(), StateNode::Leaf("晨港".to_owned())),
                    ("Secret".to_owned(), StateNode::Leaf("藏寶圖".to_owned())),
                ])),
            )]),
            ..TableState::default()
        };
        state.changes.insert("World.HP".to_owned(), "+5".to_owned());
        let scope = StateScope {
            hidden: vec![vec!["World".to_owned()]],
            align: false,
        };

        let dynamic = gm_dynamic_block(&[], &state, "阿濤", &mechanism, &scope, "zh-TW");
        assert_eq!(
            dynamic,
            "## 目前狀態（這桌的檯面，接續它往下演）\n\
             World：\n  Desc：晨港\n  HP：100\n  Secret：藏寶圖"
        );
    }

    #[test]
    fn character_state_block_shows_only_own_branch_with_marks_and_excludes_rare() {
        let mechanism = Mechanism {
            incremental: true,
            guide: String::new(),
            rules: BTreeMap::from([(
                "Heroes.亞瑟.Hidden".to_owned(),
                FieldRule::for_kind(FieldKind::ReadOnly),
            )]),
            ..Mechanism::default()
        };
        let mut state = TableState {
            tree: BTreeMap::from([(
                "Heroes".to_owned(),
                StateNode::Branch(BTreeMap::from([
                    (
                        "亞瑟".to_owned(),
                        StateNode::Branch(BTreeMap::from([
                            ("HP".to_owned(), StateNode::Leaf("80".to_owned())),
                            ("Hidden".to_owned(), StateNode::Leaf("秘密".to_owned())),
                        ])),
                    ),
                    (
                        "鴉".to_owned(),
                        StateNode::Branch(BTreeMap::from([(
                            "HP".to_owned(),
                            StateNode::Leaf("50".to_owned()),
                        )])),
                    ),
                ])),
            )]),
            ..TableState::default()
        };
        state
            .changes
            .insert("Heroes.亞瑟.HP".to_owned(), "-10".to_owned());

        let branch = vec!["Heroes".to_owned(), "亞瑟".to_owned()];
        let block = character_state_block(&state, &mechanism, &branch, "亞瑟", "阿濤").unwrap();
        assert!(block.starts_with(
            "## 「亞瑟」目前的狀態（系統帳，唯讀；可以拿來演，但不要輸出任何狀態欄或更新區塊）"
        ));
        assert!(block.contains("HP：80（-10）"));
        assert!(!block.contains("Hidden"));
        assert!(!block.contains("鴉"));

        // 沒綁到分支、分支其實是葉子、分支不存在——都是 None
        assert!(character_state_block(&state, &mechanism, &[], "亞瑟", "阿濤").is_none());
        assert!(character_state_block(
            &state,
            &mechanism,
            &["Heroes".to_owned(), "亞瑟".to_owned(), "HP".to_owned()],
            "亞瑟",
            "阿濤"
        )
        .is_none());
        assert!(
            character_state_block(&state, &mechanism, &["不存在".to_owned()], "亞瑟", "阿濤")
                .is_none()
        );
    }

    #[test]
    fn resolve_branch_prefers_binding_falls_back_when_invalid_and_finds_nested_names() {
        let tree = BTreeMap::from([
            (
                "Heroes".to_owned(),
                StateNode::Branch(BTreeMap::from([(
                    "亞瑟".to_owned(),
                    StateNode::Branch(BTreeMap::from([(
                        "HP".to_owned(),
                        StateNode::Leaf("80".to_owned()),
                    )])),
                )])),
            ),
            ("鴉".to_owned(), StateNode::Branch(BTreeMap::new())),
        ]);

        // 指認優先：卡名明明是「鴉」，指認路徑照樣贏
        let bindings = BTreeMap::from([(
            "card-crow".to_owned(),
            vec!["Heroes".to_owned(), "亞瑟".to_owned()],
        )]);
        assert_eq!(
            resolve_branch(&tree, &bindings, "card-crow", "鴉"),
            Some(vec!["Heroes".to_owned(), "亞瑟".to_owned()])
        );

        // 指認路徑不存在＝失效，退回同名比對
        let invalid = BTreeMap::from([(
            "card-crow".to_owned(),
            vec!["Not".to_owned(), "Exist".to_owned()],
        )]);
        assert_eq!(
            resolve_branch(&tree, &invalid, "card-crow", "鴉"),
            Some(vec!["鴉".to_owned()])
        );

        // 指認路徑指到葉子（不是分支）也視為失效
        let points_at_leaf = BTreeMap::from([(
            "card-arthur".to_owned(),
            vec!["Heroes".to_owned(), "亞瑟".to_owned(), "HP".to_owned()],
        )]);
        assert_eq!(
            resolve_branch(&tree, &points_at_leaf, "card-arthur", "亞瑟"),
            Some(vec!["Heroes".to_owned(), "亞瑟".to_owned()])
        );

        // 沒有指認：巢狀（Heroes/亞瑟）也找得到
        assert_eq!(
            resolve_branch(&tree, &BTreeMap::new(), "card-arthur", "亞瑟"),
            Some(vec!["Heroes".to_owned(), "亞瑟".to_owned()])
        );

        // 完全找不到
        assert_eq!(
            resolve_branch(&tree, &BTreeMap::new(), "card-x", "不存在的人"),
            None
        );
    }

    #[test]
    fn snapshot_updates_returns_only_snapshot_level_changes_with_user_macro_replaced() {
        let mechanism = Mechanism {
            incremental: true,
            ..Mechanism::default()
        };
        let state = TableState {
            tree: BTreeMap::from([(
                "World".to_owned(),
                StateNode::Branch(BTreeMap::from([
                    ("HP".to_owned(), StateNode::Leaf("100".to_owned())),
                    (
                        "Desc".to_owned(),
                        StateNode::Leaf("{{user}} 的家鄉".to_owned()),
                    ),
                ])),
            )]),
            changes: BTreeMap::from([
                ("World.HP".to_owned(), "+5".to_owned()),
                ("World.Desc".to_owned(), "更新".to_owned()),
            ]),
            ..TableState::default()
        };

        let updates = snapshot_updates(&state, &mechanism, "阿濤");
        assert_eq!(
            updates,
            vec![("World.Desc".to_owned(), "阿濤 的家鄉".to_owned())]
        );

        // 全量桌一律回空
        assert!(snapshot_updates(&state, &Mechanism::default(), "阿濤").is_empty());
    }

}
