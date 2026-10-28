use crate::data::{self, CharacterCard, InjectLevel, Mechanism, StateNode, TableState};

use crate::mechanism;

use std::collections::BTreeMap;

use super::context::worldbook_heading;
use super::messages::{replace_st_macros, scaffold_en};

/// GM 的回合動態塊：世界書（尾段那份，已渲染好的本文）＋「目前狀態」。
/// assemble_gm_messages（尾端獨立訊息）與 gm_lane_turn（resume 續聊回合尾段）共用。
/// 增量桌（mechanism.incremental）依 scope 裁切分支＋過濾葉子＋加變動標記；
/// 全量桌逐字維持現狀（不裁、不濾、不標）。
pub(super) fn gm_dynamic_block(
    worldbook: &str,
    state: &TableState,
    user_name: &str,
    mechanism: &Mechanism,
    scope: &StateScope,
    lang: &str,
) -> String {
    let en = scaffold_en(lang);
    let mut dynamic = String::new();
    if !worldbook.is_empty() {
        dynamic.push_str(worldbook_heading(lang).trim_start_matches('\n'));
        dynamic.push_str(worldbook);
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
            en,
        },
        &mut Vec::new(),
    );
    if !state.table.is_empty() || !tree_text.is_empty() {
        if !dynamic.is_empty() {
            dynamic.push('\n');
        }
        let header = match (en, mechanism.incremental && scope.align) {
            (true, true) => "## Current state (full alignment: these are the true values in the system ledger; treat them as authoritative)\n",
            (true, false) => "## Current state (the table as it stands; continue from here)\n",
            (false, true) => "## 目前狀態（完整對齊，以下是系統帳上的真值，請以此為準）\n",
            (false, false) => "## 目前狀態（這桌的檯面，接續它往下演）\n",
        };
        dynamic.push_str(header);
        for (key, value) in &state.table {
            let display_name = match (en, key.as_str()) {
                (true, "time") => "Time",
                (true, "place") => "Place",
                (true, "present") => "Present",
                (false, "time") => "時間",
                (false, "place") => "地點",
                (false, "present") => "在場人物",
                _ => key,
            };
            dynamic.push_str(&field_line("", display_name, value, en));
        }
        dynamic.push_str(&tree_text);
        for line in absent_lines(state, mechanism, user_name, en) {
            dynamic.push_str(&line);
            dynamic.push('\n');
        }
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
            dynamic.push_str(match en {
                true => "## Current situation (hidden background the system derived from the state table; do not restate this section in your reply)\n",
                false => "## 當前情境（系統依狀態表判定的隱藏背景，不要在回覆裡複述本段）\n",
            });
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
        dynamic.push_str(match en {
            true => "## Updates the system rejected last turn (correct them using these current values)\n",
            false => "## 上一輪被系統擋下的更新（請照這些現值修正）\n",
        });
        for note in &state.notes {
            dynamic.push_str(&format!("{note}\n"));
        }
    }
    dynamic.trim_end().to_owned()
}

/// 在場名單集合（statusbar-only-fields）裡不在場、但已有資料的項目：每個集合一行，提醒 GM 回場時只報變動、
/// 不要整筆重新 insert（不在場的分支被 `state_scope` 裁掉時 GM 看不到存著的值）。present 沒有值就不附；
/// 誰在場跟 `state_scope` 同一套比對（在場項目含名字、原字優先、字形折疊兜底，歧義算在場）。
fn absent_lines(
    state: &TableState,
    mechanism: &Mechanism,
    user_name: &str,
    en: bool,
) -> Vec<String> {
    if !mechanism.incremental {
        return Vec::new();
    }
    let raw = state.table.get("present").map(String::as_str).unwrap_or("");
    let present = data::split_present_names(&raw.replace("{{user}}", user_name));
    if present.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    for (path, spec) in &mechanism.collections {
        if !spec.present_scoped {
            continue;
        }
        let segments: Vec<String> = path.split('.').map(str::to_owned).collect();
        let Some(StateNode::Branch(items)) = data::node_at(&state.tree, &segments) else {
            continue;
        };
        let names: Vec<&str> = items
            .iter()
            .filter(|(_, node)| matches!(node, StateNode::Branch(_)))
            .map(|(name, _)| name.as_str())
            .collect();
        let found = data::match_present_with(&present, &names, |item, name| item.contains(name));
        let absent: Vec<&str> = names
            .iter()
            .enumerate()
            .filter(|(index, _)| !found.matched.contains(index) && !found.ambiguous.contains(index))
            .map(|(_, name)| *name)
            .collect();
        if absent.is_empty() {
            continue;
        }
        let pointer: String = segments
            .iter()
            .map(|segment| format!("/{}", segment.replace('~', "~0").replace('/', "~1")))
            .collect();
        lines.push(if en {
            format!(
                "`{pointer}` absent but already recorded: {}. When they come back, report only the fields that changed; do not insert them again.",
                absent.join(", ")
            )
        } else {
            format!(
                "`{pointer}` 不在場但已有資料：{}；回場時只報有變動的格子，不要重新 insert。",
                absent.join("、")
            )
        });
    }
    lines
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
    /// 英文骨架：冒號與變動標記括號用半形
    en: bool,
}

/// 狀態欄一行「鍵：值」；英文骨架用半形冒號。
fn field_line(indent: &str, key: &str, value: &str, en: bool) -> String {
    match en {
        true => format!("{indent}{key}: {value}\n"),
        false => format!("{indent}{key}：{value}\n"),
    }
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
    let collection = is_collection(render.mechanism, path);
    let in_item = path
        .split_last()
        .is_some_and(|(_, parent)| is_collection(render.mechanism, parent));
    for (key, node) in ordered(tree, collection) {
        // 具名集合項目的順序葉子是 app 的帳，不給 GM 看
        if in_item && key == data::COLLECTION_ORDER_KEY {
            continue;
        }
        path.push(key.clone());
        let indent = "  ".repeat(path.len() - 1 - render.base);
        let branch_hidden = incremental && !render.align && render.hidden.iter().any(|h| h == path);
        if !branch_hidden {
            match node {
                StateNode::Leaf(value) => {
                    let show = mechanism::leaf_shown(render.mechanism, path, value, render.align);
                    if show {
                        let mark = if incremental {
                            render
                                .changes
                                .get(&path.join("."))
                                .map(|mark| match render.en {
                                    true => format!(" ({mark})"),
                                    false => format!("（{mark}）"),
                                })
                                .unwrap_or_default()
                        } else {
                            String::new()
                        };
                        let value = replace_st_macros(value, render.user_name, None);
                        if !incremental && value.contains('\n') {
                            output.push_str(&field_line(&indent, key, "|", render.en));
                            push_block_lines(output, &format!("{indent}  "), &value);
                        } else {
                            output.push_str(&field_line(
                                &indent,
                                key,
                                &format!("{value}{mark}"),
                                render.en,
                            ));
                        }
                    }
                }
                StateNode::Branch(children)
                    if !incremental && (children.is_empty() || data::is_index_branch(children)) =>
                {
                    render_list(output, &indent, key, children, render, path);
                }
                StateNode::Branch(children) => {
                    let header = match render.en {
                        true => format!("{indent}{key}:\n"),
                        false => format!("{indent}{key}：\n"),
                    };
                    let before = output.len();
                    output.push_str(&header);
                    render_state_tree(output, children, render, path);
                    if incremental && output.len() == before + header.len() {
                        output.truncate(before);
                        // 集合清空了也照印：標頭消失 GM 就不知道這欄還在、可以再新增
                        if is_collection(render.mechanism, path) {
                            output.push_str(&empty_collection_line(&indent, key, render.en));
                        }
                    }
                }
            }
        }
        path.pop();
    }
    if incremental {
        render_missing_collections(output, tree, render, path);
    }
}

/// 登記了、卻不在樹上的集合（清空時連分支、甚至祖先分支一起剪掉）照空集合印：本層缺的鍵是集合就印「沒有項目」，
/// 是集合的祖先就印標頭、往下補。增量桌才有集合。
fn render_missing_collections(
    output: &mut String,
    tree: &BTreeMap<String, StateNode>,
    render: &TreeRender,
    path: &mut Vec<String>,
) {
    let mut missing: Vec<String> = Vec::new();
    for name in render.mechanism.collections.keys() {
        let segments: Vec<&str> = name.split('.').collect();
        if segments.len() <= path.len() || !segments.iter().zip(path.iter()).all(|(a, b)| a == b) {
            continue;
        }
        let next = segments[path.len()];
        if !tree.contains_key(next) && !missing.iter().any(|seen| seen == next) {
            missing.push(next.to_owned());
        }
    }
    for key in missing {
        path.push(key.clone());
        let hidden = !render.align && render.hidden.iter().any(|h| h == path);
        if !hidden {
            let indent = "  ".repeat(path.len() - 1 - render.base);
            if is_collection(render.mechanism, path) {
                output.push_str(&empty_collection_line(&indent, &key, render.en));
            } else {
                output.push_str(&match render.en {
                    true => format!("{indent}{key}:\n"),
                    false => format!("{indent}{key}：\n"),
                });
                render_missing_collections(output, &BTreeMap::new(), render, path);
            }
        }
        path.pop();
    }
}

fn is_collection(mechanism: &Mechanism, path: &[String]) -> bool {
    !path.is_empty() && mechanism.collections.contains_key(&path.join("."))
}

fn empty_collection_line(indent: &str, key: &str, en: bool) -> String {
    field_line(
        indent,
        key,
        if en {
            "(no items yet)"
        } else {
            "（目前沒有項目）"
        },
        en,
    )
}

/// 分支的子節點照印出順序：清單出身的索引分支依數字排（字串排會變成 0,1,10,2…）；集合（`collection`）
/// 具名項目照 `_序`、序列照索引數字，排不出名次的接最後依鍵排——殼也照這個順序畫，GM 照狀態整份回寫才保序。
fn ordered(tree: &BTreeMap<String, StateNode>, collection: bool) -> Vec<(&String, &StateNode)> {
    let mut entries: Vec<_> = tree.iter().collect();
    if collection {
        entries.sort_by_key(|(key, node)| (collection_rank(key, node), (*key).clone()));
    } else if data::is_index_branch(tree) {
        entries.sort_by_key(|(key, _)| key.parse::<usize>().unwrap_or(usize::MAX));
    }
    entries
}

fn collection_rank(key: &str, node: &StateNode) -> i64 {
    let order = match node {
        StateNode::Branch(children) => match children.get(data::COLLECTION_ORDER_KEY) {
            Some(StateNode::Leaf(value)) => value.trim().parse::<i64>().ok(),
            _ => None,
        },
        StateNode::Leaf(_) => None,
    };
    order
        .or_else(|| key.parse::<i64>().ok())
        .unwrap_or(i64::MAX)
}

/// 全量桌把清單出身的分支印回 YAML 清單（與狀態區塊解析器往返一致）：空的印 `鍵：[]`；
/// 純量項 `- 值`（含冒號就加引號）；物件項第一個子鍵接在 `- ` 後、其餘對齊它；巢狀清單 `- - 值`。
fn render_list(
    output: &mut String,
    indent: &str,
    key: &str,
    children: &BTreeMap<String, StateNode>,
    render: &TreeRender,
    path: &mut Vec<String>,
) {
    if children.is_empty() {
        output.push_str(&field_line(indent, key, "[]", render.en));
        return;
    }
    output.push_str(&match render.en {
        true => format!("{indent}{key}:\n"),
        false => format!("{indent}{key}：\n"),
    });
    render_items(output, &format!("{indent}  "), children, render, path);
}

fn render_items(
    output: &mut String,
    item_indent: &str,
    children: &BTreeMap<String, StateNode>,
    render: &TreeRender,
    path: &mut Vec<String>,
) {
    for (index, node) in ordered(children, false) {
        path.push(index.clone());
        match node {
            StateNode::Leaf(value) => {
                let value = replace_st_macros(value, render.user_name, None);
                if value.contains('\n') {
                    output.push_str(&format!("{item_indent}- |\n"));
                    push_block_lines(output, &format!("{item_indent}  "), &value);
                } else {
                    output.push_str(&format!("{item_indent}- {}\n", list_scalar(&value)));
                }
            }
            StateNode::Branch(grand) if grand.is_empty() => {
                output.push_str(&format!("{item_indent}- []\n"));
            }
            StateNode::Branch(grand) => {
                let inner_indent = format!("{item_indent}  ");
                let mut inner = String::new();
                if data::is_index_branch(grand) {
                    render_items(&mut inner, &inner_indent, grand, render, path);
                } else {
                    render_state_tree(&mut inner, grand, render, path);
                }
                // 第一行的縮排換成 `- `，其餘行原樣（子鍵已在 `- ` 後內容那一欄）
                let first = inner.strip_prefix(inner_indent.as_str()).unwrap_or(&inner);
                output.push_str(&format!("{item_indent}- {first}"));
            }
        }
        path.pop();
    }
}

/// 多行葉子印成區塊純量的續行（`鍵：|` 底下多縮排一層）；空白行與行首縮排不保留。
fn push_block_lines(output: &mut String, indent: &str, value: &str) {
    for line in value.lines().map(str::trim).filter(|line| !line.is_empty()) {
        output.push_str(&format!("{indent}{line}\n"));
    }
}

/// 清單純量項：含冒號、引號開頭、空字串或長得像空容器時加引號，免得讀回時被當成鍵或容器。
fn list_scalar(value: &str) -> String {
    let needs_quote = value.is_empty()
        || value.contains([':', '：'])
        || value.starts_with(['"', '\'', '-', '#', '*', '+', '>', '[', '{']);
    if !needs_quote {
        return value.to_owned();
    }
    if !value.contains('"') {
        format!("\"{value}\"")
    } else if !value.contains('\'') {
        format!("'{value}'")
    } else {
        value.to_owned()
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
/// `table_names`：整桌人物名字（全部卡名＋人物條目標題），只拿來判折疊歧義。
pub fn state_scope(
    state: &TableState,
    mechanism: &Mechanism,
    cards: &[CharacterCard],
    player: Option<&CharacterCard>,
    bindings: &BTreeMap<String, Vec<String>>,
    align: bool,
    table_names: &[String],
) -> StateScope {
    if !mechanism.incremental {
        return StateScope::default();
    }
    let present =
        data::split_present_names(state.table.get("present").map(String::as_str).unwrap_or(""));
    if present.is_empty() {
        return StateScope {
            hidden: Vec::new(),
            align,
        };
    }
    let player_id = player.map(|player| player.id.as_str());
    // 先收齊候選（有卡的分支＝卡名、同容器手足＝分支鍵），再整批比對在場名單：單向包含（在場項目含名字），
    // 原文命中優先、簡繁折疊兜底；折疊後多個候選分不出是誰（沈郁／沈鬱）就都留，寧可多送不裁瞎。
    let mut carded: Vec<(Vec<String>, &str, bool)> = Vec::new(); // (分支, 名字, 永遠送)
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
        // 玩家那支與舞台卡那支（場景狀態；GM 不會把舞台寫進 present）永遠送
        carded.push((
            branch,
            card.name.as_str(),
            Some(card.id.as_str()) == player_id || card.stage,
        ));
    }
    let mut siblings: Vec<(Vec<String>, &str)> = Vec::new();
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
            if carded.iter().any(|(branch, _, _)| *branch == path) {
                continue;
            }
            siblings.push((path, name.as_str()));
        }
    }
    // 整桌人物名字（隱藏與封存的卡、人物條目）也進候選池但不對應分支：present 精確寫的是人物條目「沈郁」時，
    // 只剩「沈鬱」一支的樹不會因為折疊唯一命中就把那支當在場送出（與登場、換幕結算同一個名字池）
    let names: Vec<&str> = carded
        .iter()
        .map(|(_, name, _)| *name)
        .chain(siblings.iter().map(|(_, name)| *name))
        .chain(table_names.iter().map(String::as_str))
        .collect();
    let found = data::match_present_with(&present, &names, |item, name| item.contains(name));
    let shown = |index: usize| found.matched.contains(&index) || found.ambiguous.contains(&index);
    let mut hidden: Vec<Vec<String>> = Vec::new();
    for (index, (branch, _, always)) in carded.iter().enumerate() {
        if !always && !shown(index) {
            hidden.push(branch.clone());
        }
    }
    for (offset, (path, _)) in siblings.iter().enumerate() {
        if !shown(carded.len() + offset) && !hidden.contains(path) {
            hidden.push(path.clone());
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
    lang: &str,
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
            en: scaffold_en(lang),
        },
        &mut path,
    );
    if body.is_empty() {
        return None;
    }
    let heading = match scaffold_en(lang) {
        true => format!(
            "## {card_name}'s current state (system ledger, read-only; this block is authoritative and replaces every earlier state block of the same name in the conversation; use it in your portrayal, but do not output any state fence or update block)"
        ),
        false => format!(
            "## 「{card_name}」目前的狀態（系統帳，唯讀；以本區為準，取代先前對話中所有同名狀態區塊；可以拿來演，但不要輸出任何狀態欄或更新區塊）"
        ),
    };
    Some(format!("{heading}\n{}", body.trim_end()))
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
mod absent_tests;
#[cfg(test)]
mod collection_tests;
#[cfg(test)]
mod list_render_tests;
#[cfg(test)]
mod tests;
