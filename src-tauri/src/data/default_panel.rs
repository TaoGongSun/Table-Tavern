//! 預設狀態欄（table-status-panel §2）：沒有卡格式的桌（空桌、一句話開桌、房間一句話開世界）由 app 種
//! 固定的兩組欄位——每個角色一格（`characters.<格鍵>.status`／`items`）與追蹤三欄（`tracking.goals`／
//! `promises`／`clues`）。欄位由 app 定、模型只填值；開格、改名搬格、刪格都是 app 的事，模型開不了格。
//! 鍵固定英文（模型回報的路徑不隨介面語言變），顯示才在地化（`label`）。
use super::{
    is_interface_takeover, list_characters_with, read_state, set_last_transcript_state,
    update_state, world_has_state_bar, DataResult, FieldKind, FieldRule, InjectLevel, StateNode,
    TableKind, UpdateMode, WorldState,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub mod combat;

pub const CHARACTERS: &str = "characters";
pub const TRACKING: &str = "tracking";
pub const STATUS: &str = "status";
pub const ITEMS: &str = "items";
pub const TRACKING_LISTS: [&str; 3] = ["goals", "promises", "clues"];
/// 系統欄名：手動加欄位不得重名
pub const RESERVED: [&str; 5] = [STATUS, ITEMS, "goals", "promises", "clues"];

pub const MAX_CELLS: usize = 12;
pub const STATUS_CHARS: u32 = 60;
pub const ITEMS_MAX: u32 = 8;
pub const ITEM_CHARS: u32 = 24;
pub const TRACKING_MAX: u32 = 6;
pub const TRACKING_CHARS: u32 = 48;
/// 手動加的文字欄字數上限（與 status 同）
pub const CUSTOM_CHARS: u32 = 60;
/// 手動欄名最多幾個字
const FIELD_NAME_CHARS: usize = 24;

fn rule(kind: FieldKind, max_chars: u32, max_items: Option<u32>) -> FieldRule {
    let mut rule = FieldRule::for_kind(kind);
    rule.update = UpdateMode::Replace;
    rule.inject = InjectLevel::Turn;
    rule.max_chars = Some(max_chars);
    rule.max_items = max_items;
    rule
}

/// 種入的規則表（§2.3）。
pub fn seed_rules() -> BTreeMap<String, FieldRule> {
    let mut rules = BTreeMap::from([
        (
            format!("{CHARACTERS}.*.{STATUS}"),
            rule(FieldKind::Text, STATUS_CHARS, None),
        ),
        (
            format!("{CHARACTERS}.*.{ITEMS}"),
            rule(FieldKind::List, ITEM_CHARS, Some(ITEMS_MAX)),
        ),
    ]);
    for key in TRACKING_LISTS {
        rules.insert(
            format!("{TRACKING}.{key}"),
            rule(FieldKind::List, TRACKING_CHARS, Some(TRACKING_MAX)),
        );
    }
    rules
}

/// GM 動態塊與角色線用的顯示名（照 time／place／present 的作法）；不是預設欄位的路徑回 None（照鍵印）。
pub fn label(path: &[String], en: bool) -> Option<&'static str> {
    let parts: Vec<&str> = path.iter().map(String::as_str).collect();
    let (zh, english) = match parts.as_slice() {
        [CHARACTERS] => ("角色", "Characters"),
        [CHARACTERS, _, STATUS] => ("狀態", "Status"),
        [CHARACTERS, _, ITEMS] => ("持有", "Items"),
        [TRACKING] => ("追蹤中", "Tracking"),
        [TRACKING, "goals"] => ("目標", "Goals"),
        [TRACKING, "promises"] => ("承諾", "Promises"),
        [TRACKING, "clues"] => ("線索", "Clues"),
        [CHARACTERS, _, combat::HP] | [combat::COMBAT, combat::FOES, _, combat::HP] => ("HP", "HP"),
        [CHARACTERS, _, combat::CONDITIONS]
        | [combat::COMBAT, combat::FOES, _, combat::CONDITIONS] => ("狀態效果", "Conditions"),
        [combat::COMBAT] => ("戰鬥", "Combat"),
        [combat::COMBAT, combat::ORDER] => ("行動順序", "Turn order"),
        [combat::COMBAT, combat::TURN] => ("輪到", "Turn"),
        [combat::COMBAT, combat::FOES] => ("敵人", "Foes"),
        _ => return None,
    };
    Some(if en { english } else { zh })
}

/// 這桌該不該種預設狀態欄（還沒種過時才問）：聊天桌、非重播、非介面接管、沒宣告狀態欄格式、
/// 機制與狀態樹全空（`[initvar]` 只開 incremental 的桌也算匯入過機制，不種）。
fn eligible(root: &Path, world_id: &str, state: &WorldState) -> DataResult<bool> {
    if state.table_kind != TableKind::Chat
        || state.replay
        || !state.mechanism.is_empty()
        || !state.state.tree.is_empty()
    {
        return Ok(false);
    }
    if is_interface_takeover(root, world_id, state.refactor_mode.as_deref()) {
        return Ok(false);
    }
    Ok(!world_has_state_bar(root, world_id)?)
}

/// 一張卡在角色格對照裡的樣子：`fresh`＝可以開新格（非封存、非舞台）。封存、舞台卡不開新格，
/// 但已有的格留著（封存可還原，刪了資料就回不來）；卡整張刪掉才刪格。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelCard {
    pub id: String,
    pub name: String,
    pub fresh: bool,
}

/// 桌上的卡，玩家卡（房間是在場玩家卡固定順序）排最前，其餘照側欄順序。
fn panel_cards(root: &Path, world_id: &str, state: &WorldState) -> DataResult<Vec<PanelCard>> {
    let metas = list_characters_with(root, world_id, true)?;
    let players: Vec<&String> = match state.player_card_ids.is_empty() {
        true => state.player_card_id.iter().collect(),
        false => state.player_card_ids.iter().collect(),
    };
    let rank = |id: &String| players.iter().position(|player| *player == id);
    let mut cards: Vec<(Option<usize>, usize, PanelCard)> = metas
        .into_iter()
        .enumerate()
        .map(|(index, meta)| {
            let card = PanelCard {
                fresh: !meta.archived && !meta.stage,
                id: meta.id,
                name: meta.name,
            };
            (rank(&card.id), index, card)
        })
        .collect();
    cards.sort_by_key(|(player, index, _)| (player.is_none(), *player, *index));
    Ok(cards.into_iter().map(|(_, _, card)| card).collect())
}

/// GM 回合取材料前呼叫（已持 world write permit）：該種就種，種過就對照角色格。沒有變動不寫檔——規則進
/// 系統提示，寫了快取就滅。回傳有沒有寫。
pub fn ensure(root: &Path, world_id: &str) -> DataResult<bool> {
    let state = read_state(root, world_id)?;
    if !state.mechanism.default_panel && !eligible(root, world_id, &state)? {
        return Ok(false);
    }
    let cards = panel_cards(root, world_id, &state)?;
    super::state_commit::with_commit(root, world_id, |_| {
        let Some(state) = update_state(root, world_id, |state| {
            Ok(sync(state, &cards).then(|| state.clone()))
        })?
        else {
            return Ok(false);
        };
        // 桌上的目前值恆等於最後一則事件的快照（MECHANISM-FORMAT「狀態樹」）
        set_last_transcript_state(root, world_id, state.current_scene, &state.state)?;
        Ok(true)
    })
}

/// 純函式本體：沒種過就種整棵＋規則＋旗標；之後補缺的格與葉子、搬改名的格、刪已刪除的卡的格。
/// 回傳有沒有變動。
pub fn sync(state: &mut WorldState, cards: &[PanelCard]) -> bool {
    let before = (
        state.mechanism.clone(),
        state.state.tree.clone(),
        state.branch_bindings.clone(),
    );
    let mechanism = &mut state.mechanism;
    if !mechanism.default_panel {
        mechanism.default_panel = true;
        mechanism.incremental = true;
        mechanism.rules.extend(seed_rules());
    }
    let tree = &mut state.state.tree;
    let tracking = branch_mut(tree, TRACKING);
    for key in TRACKING_LISTS {
        branch_mut(tracking, key);
    }
    let rules = &mut mechanism.rules;
    let cells = &mut mechanism.panel_cells;
    let bindings = &mut state.branch_bindings;
    let characters = branch_mut(tree, CHARACTERS);
    // 卡整張刪掉才刪格
    let alive: BTreeSet<&str> = cards.iter().map(|card| card.id.as_str()).collect();
    let gone: Vec<(String, String)> = cells
        .iter()
        .filter(|(id, _)| !alive.contains(id.as_str()))
        .map(|(id, key)| (id.clone(), key.clone()))
        .collect();
    for (id, key) in gone {
        cells.remove(&id);
        characters.remove(&key);
        move_cell_rules(rules, &key, None);
        if bindings.get(&id) == Some(&cell_path(&key)) {
            bindings.remove(&id);
        }
    }
    // 收回到改名前的快照時，樹上是舊鍵、對照是新鍵：唯一一支沒歸屬的格就是它，接回來
    let mapped: BTreeSet<String> = cells.values().cloned().collect();
    let missing: Vec<String> = mapped
        .iter()
        .filter(|key| !characters.contains_key(*key))
        .cloned()
        .collect();
    let orphans: Vec<String> = characters
        .iter()
        .filter(|(key, node)| !mapped.contains(*key) && matches!(node, StateNode::Branch(_)))
        .map(|(key, _)| key.clone())
        .collect();
    if let ([key], [orphan]) = (missing.as_slice(), orphans.as_slice()) {
        if let Some(node) = characters.remove(orphan) {
            characters.insert(key.clone(), node);
        }
    }
    for card in cards {
        let current = cells.get(&card.id).cloned();
        let key = match current {
            Some(key) if key_base(&key) == base_name(card) => key,
            Some(old) => {
                // 改名：整格搬到新鍵，資料與自訂欄都留著
                let key = unique_key(base_name(card), cells, characters, Some(&old));
                if let Some(node) = characters.remove(&old) {
                    characters.insert(key.clone(), node);
                }
                move_cell_rules(rules, &old, Some(&key));
                key
            }
            None if card.fresh && cells.len() < MAX_CELLS => {
                unique_key(base_name(card), cells, characters, None)
            }
            None => continue,
        };
        cells.insert(card.id.clone(), key.clone());
        let cell = branch_mut(characters, &key);
        cell.entry(STATUS.to_owned())
            .or_insert_with(|| StateNode::Leaf(String::new()));
        branch_mut(cell, ITEMS);
        // 角色線靠綁定拿到自己那格（同名加後綴的格自動比對對不上）；玩家自己指認到別處的不蓋
        let bound = bindings.get(&card.id);
        if bound.is_none_or(|path| path.first().is_some_and(|root| root == CHARACTERS)) {
            bindings.insert(card.id.clone(), cell_path(&key));
        }
    }
    combat::sync(state);
    before
        != (
            state.mechanism.clone(),
            state.state.tree.clone(),
            state.branch_bindings.clone(),
        )
}

/// 一格的精確規則（面板手動加的欄位 `characters.<格鍵>.<欄名>`）跟著格搬到新鍵；`to` 為 None＝格刪了，一併清掉。
fn move_cell_rules(rules: &mut BTreeMap<String, FieldRule>, from: &str, to: Option<&str>) {
    let prefix = format!("{CHARACTERS}.{from}.");
    let owned: Vec<String> = rules
        .keys()
        .filter(|path| path.starts_with(&prefix))
        .cloned()
        .collect();
    for path in owned {
        let Some(rule) = rules.remove(&path) else {
            continue;
        };
        if let Some(to) = to {
            rules.insert(format!("{CHARACTERS}.{to}.{}", &path[prefix.len()..]), rule);
        }
    }
}

fn cell_path(key: &str) -> Vec<String> {
    vec![CHARACTERS.to_owned(), key.to_owned()]
}

fn base_name(card: &PanelCard) -> &str {
    match card.name.trim() {
        "" => &card.id,
        name => name,
    }
}

/// 格鍵去掉同名後綴「（n）」。
fn key_base(key: &str) -> &str {
    key.strip_suffix('）')
        .and_then(|rest| rest.rsplit_once('（'))
        .filter(|(_, digits)| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
        .map_or(key, |(base, _)| base)
}

/// 同名第二張起加「（2）」「（3）」…；樹上沒歸屬的舊格也算佔用，不蓋掉它的資料。
fn unique_key(
    name: &str,
    cells: &BTreeMap<String, String>,
    characters: &BTreeMap<String, StateNode>,
    own: Option<&String>,
) -> String {
    let taken = |key: &str| {
        own.is_none_or(|own| own != key)
            && (cells.values().any(|used| used == key) || characters.contains_key(key))
    };
    let mut key = name.to_owned();
    let mut n = 2;
    while taken(&key) {
        key = format!("{name}（{n}）");
        n += 1;
    }
    key
}

/// 取分支（不存在、或被寫成葉子都換成空分支）。
fn branch_mut<'a>(
    tree: &'a mut BTreeMap<String, StateNode>,
    key: &str,
) -> &'a mut BTreeMap<String, StateNode> {
    let node = tree
        .entry(key.to_owned())
        .or_insert_with(|| StateNode::Branch(BTreeMap::new()));
    if let StateNode::Leaf(_) = node {
        *node = StateNode::Branch(BTreeMap::new());
    }
    match node {
        StateNode::Branch(children) => children,
        StateNode::Leaf(_) => unreachable!("上面剛換成分支"),
    }
}

/// 手動加欄位的父路徑：某一格（`characters.<格鍵>`）或追蹤（`tracking`）。
fn field_parent(state: &WorldState, parent: &[String]) -> bool {
    match parent {
        [root] => root == TRACKING,
        [root, key] => {
            root == CHARACTERS && state.mechanism.panel_cells.values().any(|cell| cell == key)
        }
        _ => false,
    }
}

/// 面板「＋欄位」（§2.5）：一次登記精確規則（文字、replace、turn、60 字）並種空字串葉子。拒收回 false：
/// 不是預設桌、父路徑不對、欄名空白／太長／含 `.` 或 `*`（會變成萬用規則）／底線開頭／是系統欄名／已有同名欄位。
pub fn add_field(state: &mut WorldState, parent: &[String], name: &str) -> bool {
    let name = name.trim();
    if !state.mechanism.default_panel
        || !field_parent(state, parent)
        || name.is_empty()
        || name.chars().count() > FIELD_NAME_CHARS
        || name.contains(['.', '*'])
        || name.starts_with('_')
        || RESERVED.contains(&name)
        || combat::RESERVED.contains(&name)
    {
        return false;
    }
    let mut path = parent.to_vec();
    path.push(name.to_owned());
    let key = path.join(".");
    let Some(StateNode::Branch(children)) =
        super::state::node_at_mut(&mut state.state.tree, parent)
    else {
        return false;
    };
    if children.contains_key(name) || state.mechanism.rules.contains_key(&key) {
        return false;
    }
    children.insert(name.to_owned(), StateNode::Leaf(String::new()));
    state
        .mechanism
        .rules
        .insert(key, rule(FieldKind::Text, CUSTOM_CHARS, None));
    true
}

/// 把清單欄整支換成這些項目（索引分支 `0..n-1`；空清單留空分支，GM 才看得到這欄在）。項目已由呼叫端夾好。
/// 路徑中途不是分支回 false。
pub fn write_list(state: &mut WorldState, path: &[String], items: &[String]) -> bool {
    let Some((key, parent)) = path.split_last() else {
        return false;
    };
    let Some(StateNode::Branch(children)) =
        super::state::node_at_mut(&mut state.state.tree, parent)
    else {
        return false;
    };
    let next = StateNode::Branch(
        items
            .iter()
            .enumerate()
            .map(|(index, item)| (index.to_string(), StateNode::Leaf(item.clone())))
            .collect(),
    );
    if children.get(key) == Some(&next) {
        return false;
    }
    children.insert(key.clone(), next);
    true
}

/// 面板手改把預設桌有規則的欄位清空：留空值（文字欄空字串、清單欄空清單）不刪葉子——`set_tree_value` 的
/// 空值＝刪除會讓欄位從面板消失、規則還在，同名再加被拒，變成刪不掉的幽靈欄。不歸這裡管回 None。
pub fn clear_field(state: &mut WorldState, path: &[String], value: &str) -> Option<bool> {
    if !value.is_empty()
        || !state.mechanism.default_panel
        || !crate::mechanism::has_rule(&state.mechanism, path)
    {
        return None;
    }
    let list =
        crate::mechanism::rule_for_path(&state.mechanism, path, None).kind == FieldKind::List;
    if list {
        return Some(write_list(state, path, &[]));
    }
    let (key, parent) = path.split_last()?;
    let Some(StateNode::Branch(children)) =
        super::state::node_at_mut(&mut state.state.tree, parent)
    else {
        return Some(false);
    };
    let next = StateNode::Leaf(String::new());
    if children.get(key) == Some(&next) {
        return Some(false);
    }
    children.insert(key.clone(), next);
    Some(true)
}

/// `add_field` 的反向：刪規則與葉子。只刪手動加的欄位（系統欄名與不是自己加的路徑回 false）。
pub fn remove_field(state: &mut WorldState, parent: &[String], name: &str) -> bool {
    if !state.mechanism.default_panel || !field_parent(state, parent) || RESERVED.contains(&name) {
        return false;
    }
    let mut path = parent.to_vec();
    path.push(name.to_owned());
    if state.mechanism.rules.remove(&path.join(".")).is_none() {
        return false;
    }
    if let Some(StateNode::Branch(children)) =
        super::state::node_at_mut(&mut state.state.tree, parent)
    {
        children.remove(name);
    }
    true
}

#[cfg(test)]
mod tests;
