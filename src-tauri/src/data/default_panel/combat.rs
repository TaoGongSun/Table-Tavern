//! 戰鬥欄（ttrpg-combat-tracker §2.1／2.2）：開了擲骰的預設桌，每格多 `hp`（「現值/上限」）與 `conditions`
//! （狀態效果），另有 `combat` 分支放先攻順序、輪到誰與無卡敵人。種過就留，關擲骰不收回。
use super::{rule, CHARACTERS};
use crate::data::{FieldKind, FieldRule, InjectLevel, StateNode, UpdateMode, WorldState};
use std::collections::BTreeMap;

pub const COMBAT: &str = "combat";
pub const ORDER: &str = "order";
pub const TURN: &str = "turn";
pub const FOES: &str = "foes";
pub const HP: &str = "hp";
pub const CONDITIONS: &str = "conditions";
/// 「＋欄位」不得用的名字（新桌不再撞戰鬥欄）；`remove_field` 不認它，舊桌自訂的同名欄照樣刪得掉。
pub const RESERVED: [&str; 3] = [HP, CONDITIONS, COMBAT];

pub const CONDITIONS_MAX: u32 = 6;
pub const CONDITION_CHARS: u32 = 16;
pub const ORDER_MAX: u32 = 12;
pub const NAME_CHARS: u32 = 24;
pub const FOES_MAX: usize = 8;
/// HP 上限與單次增減的絕對值上限
pub const HP_MAX: i64 = 99_999;

/// 戰鬥欄的規則（萬用路徑；玩家自訂的同名欄是精確規則，優先於這些）。
pub fn rules() -> BTreeMap<String, FieldRule> {
    let mut hp = FieldRule::for_kind(FieldKind::Pair);
    hp.update = UpdateMode::Delta;
    hp.inject = InjectLevel::Turn;
    hp.min = Some(0.0);
    let mut rules = BTreeMap::new();
    for owner in [format!("{CHARACTERS}.*"), format!("{COMBAT}.{FOES}.*")] {
        rules.insert(format!("{owner}.{HP}"), hp.clone());
        rules.insert(
            format!("{owner}.{CONDITIONS}"),
            rule(FieldKind::List, CONDITION_CHARS, Some(CONDITIONS_MAX)),
        );
    }
    rules.insert(
        format!("{COMBAT}.{ORDER}"),
        rule(FieldKind::List, NAME_CHARS, Some(ORDER_MAX)),
    );
    rules.insert(
        format!("{COMBAT}.{TURN}"),
        rule(FieldKind::Text, NAME_CHARS, None),
    );
    rules
}

/// `default_panel::sync` 收尾時呼叫：擲骰開著就種戰鬥欄（第一次登記規則），之後每回合替每格補缺的 `hp`／
/// `conditions` 與 `combat` 底下的三個鍵。只在鍵不存在時插入——玩家自訂過同名欄就保留原值與原規則，不覆蓋、不轉型。
pub(super) fn sync(state: &mut WorldState) {
    let mechanism = &mut state.mechanism;
    if !mechanism.combat_panel {
        if !state.dice_enabled {
            return;
        }
        mechanism.combat_panel = true;
        for (path, rule) in rules() {
            mechanism.rules.entry(path).or_insert(rule);
        }
    }
    let tree = &mut state.state.tree;
    if let Some(StateNode::Branch(cells)) = tree.get_mut(CHARACTERS) {
        for cell in cells.values_mut() {
            if let StateNode::Branch(fields) = cell {
                fields
                    .entry(HP.to_owned())
                    .or_insert_with(|| StateNode::Leaf(String::new()));
                fields
                    .entry(CONDITIONS.to_owned())
                    .or_insert_with(|| StateNode::Branch(BTreeMap::new()));
            }
        }
    }
    if let StateNode::Branch(combat) = tree
        .entry(COMBAT.to_owned())
        .or_insert_with(|| StateNode::Branch(BTreeMap::new()))
    {
        for key in [ORDER, FOES] {
            combat
                .entry(key.to_owned())
                .or_insert_with(|| StateNode::Branch(BTreeMap::new()));
        }
        combat
            .entry(TURN.to_owned())
            .or_insert_with(|| StateNode::Leaf(String::new()));
    }
}

/// 清單分支（索引鍵 `0..n`）照索引排成字串；不是分支回空。
pub fn list_values(node: Option<&StateNode>) -> Vec<String> {
    let Some(StateNode::Branch(children)) = node else {
        return Vec::new();
    };
    let mut items: Vec<(usize, &str)> = children
        .iter()
        .filter_map(|(key, item)| match (key.parse::<usize>(), item) {
            (Ok(index), StateNode::Leaf(text)) => Some((index, text.as_str())),
            _ => None,
        })
        .collect();
    items.sort_by_key(|(index, _)| *index);
    items.into_iter().map(|(_, text)| text.to_owned()).collect()
}

fn list_node(items: &[String]) -> StateNode {
    StateNode::Branch(
        items
            .iter()
            .enumerate()
            .map(|(index, item)| (index.to_string(), StateNode::Leaf(item.clone())))
            .collect(),
    )
}

/// 先攻的成員驗證與連帶清理（§2.6）：順序去重去空；順序空了清掉輪到誰與敵人；輪到誰不在順序裡就清空。
/// 每個寫入口在存最後一則快照前呼叫。回傳有沒有變動。
pub fn settle(tree: &mut BTreeMap<String, StateNode>) -> bool {
    let Some(StateNode::Branch(combat)) = tree.get_mut(COMBAT) else {
        return false;
    };
    let before = combat.clone();
    let mut order: Vec<String> = Vec::new();
    for name in list_values(combat.get(ORDER)) {
        if !name.trim().is_empty() && !order.contains(&name) {
            order.push(name);
        }
    }
    combat.insert(ORDER.to_owned(), list_node(&order));
    let turn = match combat.get(TURN) {
        Some(StateNode::Leaf(name)) if order.contains(name) => name.clone(),
        _ => String::new(),
    };
    combat.insert(TURN.to_owned(), StateNode::Leaf(turn));
    if order.is_empty() {
        combat.insert(FOES.to_owned(), StateNode::Branch(BTreeMap::new()));
    }
    *combat != before
}

/// 輪到誰的成員檢查：名字在目前的先攻順序裡。
pub fn in_order(tree: &BTreeMap<String, StateNode>, name: &str) -> bool {
    let combat = match tree.get(COMBAT) {
        Some(StateNode::Branch(combat)) => combat,
        _ => return false,
    };
    list_values(combat.get(ORDER))
        .iter()
        .any(|item| item == name)
}
