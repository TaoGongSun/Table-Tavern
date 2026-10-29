//! 退場桌狀態欄照酒館追蹤（statusbar-only-fields，規格見 .ai/plans/statusbar-only-fields.md）：characters 玩法、
//! 卡片狀態欄已淘汰的非 MVU 桌，開場白狀態欄解析出來的每一格都要能被 GM 回報、狀態列跟著劇情變。
//! - 平欄其餘鍵搬進樹；沒有規則的開場格子一律補每回合注入的規則（明確數字給數字欄，其餘文字欄）。
//! - 開場值開頭的「表情符號＋空白」剝進 `mechanism.display_prefixes`。
//! - 物件組成的序列（YAML `- 人物: {名字: …}`）轉成以名字為鍵的具名集合；純量序列登記成序列集合。
//!   選不出唯一的名字欄、名字不能當鍵、重名、或既有引用保全不了：這個序列不轉成集合，但每一格（含巢狀）補萬用
//!   文字規則照常追蹤（playdata-loss-fixes D2）。

use super::apply_mechanism::{explicit_number, turn_rule, BASE_TABLE_KEYS};
use super::collections;
use crate::data::{
    self, CollectionItem, CollectionSpec, CollectionValueType, FieldKind, StateNode, WorldState,
    COLLECTION_ORDER_KEY,
};
use crate::mechanism;
use crate::mechanism::prefix::split_prefix;
use std::collections::{BTreeMap, BTreeSet};

/// 套用端給的參考資料。
pub(super) struct OpeningContext<'a> {
    /// 本桌角色卡名、本次套用的人物名、玩家名（名字欄辨認第 1 順位用）。
    pub names: &'a [String],
    /// 世界書條目內文（套用後）：有 `{{state:}}` 指進要轉的序列就不轉，條目內文不改寫。
    pub entry_texts: &'a [String],
    /// 這次淘汰的卡片狀態欄原文（dropped 裡 `statusbar` 的條目或段落）：辨認「只顯示在場人物」的寫法。
    pub statusbar_texts: &'a [String],
}

/// 常見名字欄的鍵名（名字欄辨認第 0 順位）。
const NAME_KEYS: [&str; 7] = ["名字", "姓名", "名稱", "角色", "角色名", "name", "Name"];

/// 原卡狀態欄寫明只顯示在場人物（「只显示在场人物」「在对话现场的人物才显示」「不在场的不显示」這類）。
/// 先折成簡體字形再比，繁簡寫法都認。
fn declares_present_only(texts: &[String]) -> bool {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(
            r"只(显示|列出?)[^。\n]{0,8}(在场|现场)|(在场|现场)[^。\n]{0,12}?(才|再)(显示|列出?)|不在场的?[^。\n]{0,4}不(显示|列出?)",
        )
        .expect("present-only regex")
    });
    texts
        .iter()
        .any(|text| re.is_match(&data::fold_glyphs(text)))
}

/// 桌上有沒有開場格子：平欄 time／place／present 以外的鍵，或樹上有任何葉子。
pub(super) fn has_opening_cells(state: &WorldState) -> bool {
    fn any_leaf(tree: &BTreeMap<String, StateNode>) -> bool {
        tree.values().any(|node| match node {
            StateNode::Leaf(_) => true,
            StateNode::Branch(children) => any_leaf(children),
        })
    }
    state
        .state
        .table
        .keys()
        .any(|key| !BASE_TABLE_KEYS.contains(&key.as_str()))
        || any_leaf(&state.state.tree)
}

/// 開場格子補齊。`mirrors` 是對齊後的盤點 MIRRORS（樹上路徑 → 固定鍵）：序列轉成集合時一起改寫。
/// 回傳從平欄搬走的鍵（呼叫端同步把事件快照裡的平欄也拿掉）。
pub(super) fn adopt(
    state: &mut WorldState,
    context: &OpeningContext,
    mirrors: &mut BTreeMap<String, String>,
) -> Vec<String> {
    let moved = move_flat_keys(state);
    for path in index_branches(&state.state.tree, &state.mechanism.collections) {
        convert_sequence(state, &path, context, mirrors);
    }
    // 轉成集合的已登記；剩下的是轉不了的序列
    for path in index_branches(&state.state.tree, &state.mechanism.collections) {
        track_unconverted(state, &path);
    }
    let leaves = adopt_leaves(state);
    state.mechanism.opening.tracked = true;
    state.mechanism.opening.time_cells = super::opening_notes::time_cells(&leaves);
    state.mechanism.opening.field_notes =
        super::opening_notes::field_notes(state, context.statusbar_texts);
    let paths: BTreeSet<String> = state.mechanism.collections.keys().cloned().collect();
    collections::inject_every_turn(&mut state.mechanism.rules, &paths);
    moved
}

/// 轉不了的序列：每一項底下的葉子（含巢狀）相對路徑，序列索引段換成 `*`，補每回合文字規則；已有規則的不補。
/// 樹形不動，GM 逐格 replace、增減項目走 insert／remove。
fn track_unconverted(state: &mut WorldState, base: &str) {
    fn walk(node: &StateNode, path: &mut Vec<String>, out: &mut BTreeSet<Vec<String>>) {
        match node {
            StateNode::Leaf(_) => {
                out.insert(path.clone());
            }
            StateNode::Branch(children) => {
                let index = data::is_index_branch(children);
                for (key, child) in children {
                    path.push(if index { "*".to_owned() } else { key.clone() });
                    walk(child, path, out);
                    path.pop();
                }
            }
        }
    }
    let mut path = segments(base);
    let Some(node) = data::node_at(&state.state.tree, &path).cloned() else {
        return;
    };
    let mut paths = BTreeSet::new();
    walk(&node, &mut path, &mut paths);
    for path in paths {
        if !mechanism::has_rule(&state.mechanism, &path) {
            state
                .mechanism
                .rules
                .insert(path.join("."), turn_rule(FieldKind::Text));
        }
    }
}

/// 平欄其餘鍵搬進樹（樹上已有同名的以樹為準，平欄那份刪掉）。
fn move_flat_keys(state: &mut WorldState) -> Vec<String> {
    let keys: Vec<String> = state
        .state
        .table
        .keys()
        .filter(|key| !BASE_TABLE_KEYS.contains(&key.as_str()))
        .cloned()
        .collect();
    for key in &keys {
        let Some(value) = state.state.table.remove(key) else {
            continue;
        };
        let segments = vec![key.clone()];
        if data::node_at(&state.state.tree, &segments).is_none() {
            data::set_tree_value(&mut state.state.tree, &segments, &value);
        }
    }
    keys
}

/// 樹上的編號分支（鍵恰為 0..n−1）路徑，不含已登記集合底下的。由深到淺排好不必：只收最外層。
fn index_branches(
    tree: &BTreeMap<String, StateNode>,
    collections: &BTreeMap<String, CollectionSpec>,
) -> Vec<String> {
    fn walk(
        tree: &BTreeMap<String, StateNode>,
        prefix: &str,
        collections: &BTreeMap<String, CollectionSpec>,
        out: &mut Vec<String>,
    ) {
        for (key, node) in tree {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            let StateNode::Branch(children) = node else {
                continue;
            };
            if collections.contains_key(&path) {
                continue;
            }
            if data::is_index_branch(children) {
                out.push(path);
            } else {
                walk(children, &path, collections, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(tree, "", collections, &mut out);
    out
}

fn segments(path: &str) -> Vec<String> {
    path.split('.').map(str::to_owned).collect()
}

/// 剝掉前綴後的值（沒有前綴原樣）。
fn bare(value: &str) -> &str {
    split_prefix(value).map_or(value, |(_, rest)| rest)
}

fn prefix_of(value: &str) -> Option<String> {
    split_prefix(value).map(|(prefix, _)| prefix.to_owned())
}

/// 不能當集合項目鍵（同 `mechanism::collection` 的 bad_key）。
fn bad_key(key: &str) -> bool {
    key.is_empty() || key == "-" || key.starts_with('_') || key.contains(['.', '{', '}', '\n'])
}

fn scalar_shaped(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.parse::<f64>().is_ok() || matches!(trimmed, "true" | "false")
}

fn plain_token(value: &str) -> bool {
    value.chars().count() <= 16
        && !value.chars().any(|ch| {
            ch.is_whitespace()
                || ch.is_ascii_punctuation()
                || "，。、；：！？（）《》「」『』【】…—·～".contains(ch)
        })
}

/// 一個序列項目拆包後的欄位（葉子）；項目不是物件、欄位不是葉子都回 None。
fn item_fields<'a>(
    item: &'a StateNode,
    wrapper: Option<&str>,
) -> Option<&'a BTreeMap<String, StateNode>> {
    let StateNode::Branch(children) = item else {
        return None;
    };
    let fields = match wrapper {
        Some(key) => match children.get(key) {
            Some(StateNode::Branch(inner)) => inner,
            _ => return None,
        },
        None => children,
    };
    fields
        .values()
        .all(|node| matches!(node, StateNode::Leaf(_)))
        .then_some(fields)
}

/// 名字欄：共用資格＝所有項目都有、剝前綴後字形折疊兩兩不同、能當鍵、不是數字或布林形狀；
/// 第 0 順位常見鍵名（有這種鍵卻不合格也停，不改挑別欄）、第 1 順位值都對到已知人名、第 2 順位短而沒有
/// 空白標點。每順位只收唯一候選，兩欄以上＝歧義，停止（回 Err）；全部沒有回 Ok(None)。
fn name_field(
    items: &[&BTreeMap<String, StateNode>],
    names: &[String],
) -> Result<Option<(String, bool)>, String> {
    let Some(first) = items.first() else {
        return Ok(None);
    };
    let values = |field: &str| -> Option<Vec<String>> {
        items
            .iter()
            .map(|fields| match fields.get(field) {
                Some(StateNode::Leaf(value)) => Some(bare(value).trim().to_owned()),
                _ => None,
            })
            .collect()
    };
    let qualified: Vec<(String, Vec<String>)> = first
        .keys()
        .filter_map(|field| values(field).map(|vals| (field.clone(), vals)))
        .filter(|(_, vals)| {
            let folded: BTreeSet<String> = vals
                .iter()
                .map(|v| data::fold_glyphs(v).into_owned())
                .collect();
            folded.len() == vals.len() && vals.iter().all(|v| !bad_key(v) && !scalar_shaped(v))
        })
        .collect();
    // 第 0 順位看鍵名：每項都有的常見名字鍵只能有一個，而且要合格（重名、不能當鍵就停，不往下找別欄）
    let named: Vec<&String> = first
        .keys()
        .filter(|field| NAME_KEYS.contains(&field.as_str()) && values(field).is_some())
        .collect();
    match named.as_slice() {
        [] => {}
        [field] => {
            return if qualified.iter().any(|(name, _)| name == *field) {
                Ok(Some(((*field).clone(), true)))
            } else {
                Err(format!("名字欄 {field} 有重名、不能當鍵或是數字"))
            };
        }
        _ => {
            return Err(format!(
                "名字欄有歧義：{}",
                named
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join("、")
            ))
        }
    }
    let ranks: [&dyn Fn(&str, &[String]) -> bool; 2] = [
        &|_, vals| {
            vals.iter()
                .all(|v| names.iter().any(|name| data::glyph_equal(name, v)))
        },
        &|_, vals| vals.iter().all(|v| plain_token(v)),
    ];
    for (rank_index, rank) in ranks.into_iter().enumerate() {
        let hits: Vec<&String> = qualified
            .iter()
            .filter(|(field, vals)| rank(field, vals))
            .map(|(field, _)| field)
            .collect();
        match hits.len() {
            0 => continue,
            // 第 1 順位（值都是已知人名）算人物集合；第 2 順位只是短字串
            1 => return Ok(Some((hits[0].clone(), rank_index == 0))),
            _ => {
                return Err(format!(
                    "名字欄有歧義：{}",
                    hits.iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join("、")
                ))
            }
        }
    }
    Ok(None)
}

/// 序列轉集合的路徑映射：舊路徑（含萬用）→ 新路徑；不在序列底下回 Ok(None)，保全不了回 Err。
struct PathMap<'a> {
    base: &'a str,
    wrapper: Option<&'a str>,
    name_field: &'a str,
    /// 編號 → 名字
    keys: &'a [String],
}

impl PathMap<'_> {
    fn map(&self, path: &str) -> Result<Option<String>, String> {
        let Some(rest) = path.strip_prefix(self.base) else {
            return Ok(None);
        };
        if rest.is_empty() {
            return Ok(None);
        }
        let Some(rest) = rest.strip_prefix('.') else {
            return Ok(None);
        };
        let mut parts = rest.split('.');
        let index = parts.next().unwrap_or_default();
        let item = if index == "*" {
            "*".to_owned()
        } else {
            let position: usize = index.parse().map_err(|_| format!("{path} 不是編號路徑"))?;
            self.keys
                .get(position)
                .cloned()
                .ok_or_else(|| format!("{path} 的編號超出序列"))?
        };
        if let Some(wrapper) = self.wrapper {
            match parts.next() {
                None => return Ok(Some(format!("{}.{item}", self.base))),
                Some(segment) if segment == wrapper => {}
                Some(_) => return Err(format!("{path} 的萬用段或鍵落在包裝層")),
            }
        }
        let Some(field) = parts.next() else {
            return Ok(Some(format!("{}.{item}", self.base)));
        };
        if parts.next().is_some() {
            return Err(format!("{path} 比集合欄位更深"));
        }
        if field == self.name_field {
            return Err(format!("{path} 指到名字欄"));
        }
        Ok(Some(format!("{}.{item}.{field}", self.base)))
    }

    fn map_or_keep(&self, path: &str) -> Result<String, String> {
        Ok(self.map(path)?.unwrap_or_else(|| path.to_owned()))
    }
}

fn convert_sequence(
    state: &mut WorldState,
    base: &str,
    context: &OpeningContext,
    mirrors: &mut BTreeMap<String, String>,
) {
    let base_segments = segments(base);
    let Some(StateNode::Branch(children)) = data::node_at(&state.state.tree, &base_segments) else {
        return;
    };
    let mut ordered: Vec<(usize, StateNode)> = children
        .iter()
        .filter_map(|(key, node)| key.parse::<usize>().ok().map(|index| (index, node.clone())))
        .collect();
    ordered.sort_by_key(|(index, _)| *index);
    let owned: Vec<StateNode> = ordered.into_iter().map(|(_, node)| node).collect();
    let items: Vec<&StateNode> = owned.iter().collect();

    if items.iter().all(|item| matches!(item, StateNode::Leaf(_))) {
        register_scalar_sequence(state, base, &items);
        return;
    }
    // 只有一個鍵、而且每項同名的包裝物件（`- 人物: {…}`）先拆掉
    let wrapper = match items.first() {
        Some(StateNode::Branch(first)) if first.len() == 1 => {
            let key = first.keys().next().cloned().unwrap_or_default();
            items
                .iter()
                .all(|item| matches!(item, StateNode::Branch(c) if c.len() == 1 && matches!(c.get(&key), Some(StateNode::Branch(_)))))
                .then_some(key)
        }
        _ => None,
    };
    let Some(fields): Option<Vec<&BTreeMap<String, StateNode>>> = items
        .iter()
        .map(|item| item_fields(item, wrapper.as_deref()))
        .collect()
    else {
        log::warn!("開場序列 {base} 的項目不是平的物件，不收");
        return;
    };
    let (name_field, person) = match name_field(&fields, context.names) {
        Ok(Some(found)) => found,
        Ok(None) => {
            log::warn!("開場序列 {base} 找不到名字欄，不收");
            return;
        }
        Err(reason) => {
            log::warn!("開場序列 {base} {reason}，不收");
            return;
        }
    };
    let keys: Vec<String> = fields
        .iter()
        .map(|item| match item.get(&name_field) {
            Some(StateNode::Leaf(value)) => bare(value).trim().to_owned(),
            _ => String::new(),
        })
        .collect();
    let path_map = PathMap {
        base,
        wrapper: wrapper.as_deref(),
        name_field: &name_field,
        keys: &keys,
    };
    // 既有引用保全不了就不轉：條目內文的 `{{state:}}` 不改寫，指進序列就整個不轉
    let marker = format!("{{{{state:{base}.");
    if context
        .entry_texts
        .iter()
        .any(|text| text.contains(&marker))
    {
        log::warn!("開場序列 {base} 被世界書條目的 {{{{state:}}}} 引用，不收");
        return;
    }
    let mapped = match remap(state, mirrors, &path_map) {
        Ok(mapped) => mapped,
        Err(reason) => {
            log::warn!("開場序列 {base} 既有引用保全不了（{reason}），不收");
            return;
        }
    };

    // 新的具名集合：名字為鍵、名字欄不存、前綴另記、依原序補 `_序`
    // 欄位取所有項目的聯集（第二人多一欄「傷勢」也要留著）；項目沒有的欄位不補空值
    let field_names: Vec<String> = fields
        .iter()
        .flat_map(|item| item.keys())
        .filter(|field| **field != name_field)
        .cloned()
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect();
    let mut prefixes: BTreeMap<String, String> = BTreeMap::new();
    let name_values: Vec<(usize, &str)> = fields
        .iter()
        .enumerate()
        .filter_map(|(index, item)| match item.get(&name_field) {
            Some(StateNode::Leaf(value)) => Some((index, value.as_str())),
            _ => None,
        })
        .collect();
    store_prefixes(
        &mut prefixes,
        &format!("{base}.*"),
        |i| format!("{base}.{}", keys[i]),
        &name_values,
    );
    let mut converted = BTreeMap::new();
    for (index, item) in fields.iter().enumerate() {
        let mut node = BTreeMap::new();
        for field in &field_names {
            if let Some(StateNode::Leaf(value)) = item.get(field) {
                node.insert(field.clone(), StateNode::Leaf(bare(value).to_owned()));
            }
        }
        node.insert(
            COLLECTION_ORDER_KEY.to_owned(),
            StateNode::Leaf(index.to_string()),
        );
        converted.insert(keys[index].clone(), StateNode::Branch(node));
    }
    let mut spec_fields = BTreeMap::new();
    for field in &field_names {
        let values: Vec<(usize, &str)> = fields
            .iter()
            .enumerate()
            .filter_map(|(index, item)| match item.get(field) {
                Some(StateNode::Leaf(value)) => Some((index, value.as_str())),
                _ => None,
            })
            .collect();
        store_prefixes(
            &mut prefixes,
            &format!("{base}.*.{field}"),
            |i| format!("{base}.{}.{field}", keys[i]),
            &values,
        );
        let numeric = values
            .iter()
            .all(|(_, v)| bare(v).trim().parse::<f64>().is_ok());
        spec_fields.insert(
            field.clone(),
            if numeric {
                CollectionValueType::Number
            } else {
                CollectionValueType::Text
            },
        );
        let rule_path = format!("{base}.*.{field}");
        if !mapped.contains(&rule_path) && !state.mechanism.rules.contains_key(&rule_path) {
            state.mechanism.rules.insert(
                rule_path,
                turn_rule(if numeric {
                    FieldKind::Number
                } else {
                    FieldKind::Text
                }),
            );
        }
    }
    replace_node(
        &mut state.state.tree,
        &base_segments,
        StateNode::Branch(converted),
    );
    state.mechanism.display_prefixes.extend(prefixes);
    state.mechanism.collections.insert(
        base.to_owned(),
        CollectionSpec {
            keyed: true,
            item: CollectionItem::Object,
            fields: spec_fields,
            value_type: None,
            present_scoped: person && declares_present_only(context.statusbar_texts),
        },
    );
}

/// 把既有路徑上的節點整個換掉（路徑一定存在：剛從樹上讀到）。
fn replace_node(tree: &mut BTreeMap<String, StateNode>, path: &[String], node: StateNode) {
    let Some((last, parents)) = path.split_last() else {
        return;
    };
    let mut branch = tree;
    for segment in parents {
        match branch.get_mut(segment) {
            Some(StateNode::Branch(children)) => branch = children,
            _ => return,
        }
    }
    branch.insert(last.clone(), node);
}

/// 純量序列（行動選項這類）：登記成序列集合、補 `集合.*` 文字規則；GM 整批換用 replace 整個陣列。
/// 前綴只在每項都一樣時剝（序列插入後編號會位移，精確路徑的前綴留不住）。
fn register_scalar_sequence(state: &mut WorldState, base: &str, items: &[&StateNode]) {
    let values: Vec<&str> = items
        .iter()
        .map(|item| match item {
            StateNode::Leaf(value) => value.as_str(),
            StateNode::Branch(_) => "",
        })
        .collect();
    let common = values
        .first()
        .and_then(|first| prefix_of(first))
        .filter(|prefix| {
            values
                .iter()
                .all(|v| prefix_of(v).as_deref() == Some(prefix.as_str()))
        });
    if let Some(prefix) = common {
        let base_segments = segments(base);
        for (index, value) in values.iter().enumerate() {
            let mut path = base_segments.clone();
            path.push(index.to_string());
            data::set_tree_value(&mut state.state.tree, &path, bare(value));
        }
        state
            .mechanism
            .display_prefixes
            .insert(format!("{base}.*"), prefix);
    }
    let rule_path = format!("{base}.*");
    state
        .mechanism
        .rules
        .entry(rule_path)
        .or_insert_with(|| turn_rule(FieldKind::Text));
    state.mechanism.collections.insert(
        base.to_owned(),
        CollectionSpec {
            keyed: false,
            item: CollectionItem::Scalar,
            fields: BTreeMap::new(),
            value_type: Some(CollectionValueType::Text),
            present_scoped: false,
        },
    );
}

/// 一欄的前綴：所有項目都一樣就存萬用路徑，否則有前綴的各存精確路徑。
fn store_prefixes(
    prefixes: &mut BTreeMap<String, String>,
    wildcard: &str,
    exact: impl Fn(usize) -> String,
    values: &[(usize, &str)],
) {
    let found: Vec<(usize, Option<String>)> = values
        .iter()
        .map(|(index, value)| (*index, prefix_of(value)))
        .collect();
    if let Some((_, Some(first))) = found.first() {
        if found
            .iter()
            .all(|(_, prefix)| prefix.as_deref() == Some(first.as_str()))
        {
            prefixes.insert(wildcard.to_owned(), first.clone());
            return;
        }
    }
    for (index, prefix) in found {
        if let Some(prefix) = prefix {
            prefixes.insert(exact(index), prefix);
        }
    }
}

/// 改寫指向序列的規則鍵、觸發（條件、scope、flag、`{{state:}}`）與鏡像；任何一處保全不了整批不動。
/// 回傳改寫後落在序列底下的規則鍵。
fn remap(
    state: &mut WorldState,
    mirrors: &mut BTreeMap<String, String>,
    path_map: &PathMap,
) -> Result<BTreeSet<String>, String> {
    let mut rules = BTreeMap::new();
    let mut mapped = BTreeSet::new();
    for (path, rule) in &state.mechanism.rules {
        match path_map.map(path)? {
            Some(new_path) => {
                mapped.insert(new_path.clone());
                rules.insert(new_path, rule.clone());
            }
            None => {
                rules.insert(path.clone(), rule.clone());
            }
        }
    }
    let mut triggers = state.mechanism.triggers.clone();
    for trigger in &mut triggers {
        let text = |text: &str| {
            mechanism::map_state_placeholders(text, |path| {
                path_map
                    .map_or_keep(path)
                    .map(|mapped| format!("{{{{state:{mapped}}}}}"))
            })
        };
        for case in &mut trigger.cases {
            for condition in &mut case.when {
                match condition {
                    data::Condition::Range { path, .. }
                    | data::Condition::Contains { path, .. }
                    | data::Condition::Flag { path, .. } => {
                        *path = path_map.map_or_keep(path)?;
                    }
                }
            }
            case.text = text(&case.text)?;
        }
        trigger.preamble = text(&trigger.preamble)?;
        if let Some(flag) = &trigger.flag {
            trigger.flag = Some(path_map.map_or_keep(flag)?);
        }
        if !trigger.scope.is_empty() {
            trigger.scope = segments(&path_map.map_or_keep(&trigger.scope.join("."))?);
        }
    }
    let mut new_mirrors = BTreeMap::new();
    for (path, key) in mirrors.iter() {
        new_mirrors.insert(path_map.map_or_keep(path)?, key.clone());
    }
    state.mechanism.rules = rules;
    state.mechanism.triggers = triggers;
    *mirrors = new_mirrors;
    Ok(mapped)
}

/// 集合以外沒有規則的葉子補每回合規則；開頭的前綴剝進顯示前綴表（精確路徑），明確數字給數字欄。
/// 回傳集合以外的葉子（剝過前綴的值）。
fn adopt_leaves(state: &mut WorldState) -> Vec<(String, String)> {
    let mut leaves = Vec::new();
    fn walk(
        tree: &BTreeMap<String, StateNode>,
        prefix: &str,
        collections: &BTreeMap<String, CollectionSpec>,
        out: &mut Vec<(String, String)>,
    ) {
        for (key, node) in tree {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            if collections.contains_key(&path) {
                continue;
            }
            match node {
                StateNode::Leaf(value) => out.push((path, value.clone())),
                StateNode::Branch(children) if data::is_index_branch(children) => {}
                StateNode::Branch(children) => walk(children, &path, collections, out),
            }
        }
    }
    walk(
        &state.state.tree,
        "",
        &state.mechanism.collections,
        &mut leaves,
    );
    let mut adopted = Vec::new();
    for (path, value) in leaves {
        let segments = segments(&path);
        let mut value = value;
        if let Some((prefix, rest)) = split_prefix(&value) {
            state
                .mechanism
                .display_prefixes
                .insert(path.clone(), prefix.to_owned());
            let rest = rest.to_owned();
            data::set_tree_value(&mut state.state.tree, &segments, &rest);
            value = rest;
        }
        adopted.push((path.clone(), value.clone()));
        if mechanism::has_rule(&state.mechanism, &segments) {
            continue;
        }
        let kind = match explicit_number(&value) {
            Some((number, format)) => {
                data::set_tree_value(&mut state.state.tree, &segments, &number);
                if let Some(format) = format {
                    state.mechanism.display_formats.insert(path.clone(), format);
                }
                if number.contains('/') {
                    FieldKind::Pair
                } else {
                    FieldKind::Number
                }
            }
            None => FieldKind::Text,
        };
        state.mechanism.rules.insert(path, turn_rule(kind));
    }
    adopted
}
