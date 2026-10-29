//! 集合的寫入（takeover-list-structured）：`mechanism.collections` 登記的清單欄，GM 只用路徑報變動。
//! patch 的 path／from 碰到集合本身、項目、項目欄位或集合的祖先時先經這裡：
//! - 集合本身：整份換（具名要物件、序列要陣列）或清空；
//! - 項目：具名集合以名字為鍵，名字只差字形對回既有項目；序列照 RFC 6902 插入後移、刪除補位；
//! - 項目欄位：只收描述裡的欄位（字形折疊），折好路徑交回一般流程套規則；
//! - 祖先整份替換或刪除、跨集合 move、寫到描述沒有的欄位一律拒收並回饋；move 只准同一具名集合內改名。
//!
//! 具名項目帶 app 管的順序葉子 `_序`：新增取最大值＋1、整份換依 JSON 順序重編、改名跟著走；輸入值裡的
//! 底線鍵一律丟掉。清空後集合保留為空分支。

use crate::data::{
    self, CollectionItem, CollectionSpec, Mechanism, StateNode, COLLECTION_ORDER_KEY,
};
use std::collections::BTreeMap;

use super::note::{plain, shown_pointer};
use super::tree::{insert_node, json_to_node, take_node};
use super::types::{Patch, PatchOp, Record, RecordKind};

/// `route` 的結果：不關集合的事、已處理（含拒收）、或折好路徑交回一般流程（項目欄位）。
pub(super) enum Routed {
    NotCollection,
    Handled,
    Delegate(Patch),
}

/// 項目鍵含這些字元綁不成點分路徑與殼的佔位符；底線開頭是 app 保留鍵。
fn bad_key(key: &str) -> bool {
    key.is_empty() || key == "-" || key.starts_with('_') || key.contains(['.', '{', '}', '\n'])
}

fn reject(records: &mut Vec<Record>, path: &[String], detail: String) {
    records.push(Record::new(
        RecordKind::Rejected,
        path.join("."),
        plain(detail),
    ));
}

/// 路徑落在哪個集合底下（取最長的前綴）：集合路徑與描述。
fn locate<'a>(
    mechanism: &'a Mechanism,
    path: &[String],
) -> Option<(Vec<String>, &'a CollectionSpec)> {
    mechanism
        .collections
        .iter()
        .map(|(name, spec)| (name.split('.').map(str::to_owned).collect::<Vec<_>>(), spec))
        .filter(|(base, _)| base.len() <= path.len() && path[..base.len()] == base[..])
        .max_by_key(|(base, _)| base.len())
}

/// 路徑是某個集合的祖先（整份寫它會繞過集合的形狀檢查）：回那個集合的點分路徑。
fn covered_collection<'a>(mechanism: &'a Mechanism, path: &[String]) -> Option<&'a str> {
    mechanism
        .collections
        .keys()
        .map(String::as_str)
        .find(|name| {
            let base: Vec<&str> = name.split('.').collect();
            base.len() > path.len() && path.iter().zip(&base).all(|(a, b)| a == b)
        })
}

/// 路徑落在集合裡或是集合的祖先：形狀與鍵由集合的描述管（`route`），一般物件值的鍵折疊不插手。
pub(super) fn claims(mechanism: &Mechanism, path: &[String]) -> bool {
    locate(mechanism, path).is_some() || covered_collection(mechanism, path).is_some()
}

pub(super) fn route(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patch: &Patch,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) -> Routed {
    if mechanism.collections.is_empty() {
        return Routed::NotCollection;
    }
    if patch.op == PatchOp::Move {
        return route_move(tree, mechanism, patch, records, changes);
    }
    let Some((base, spec)) = locate(mechanism, &patch.path) else {
        if patch.op == PatchOp::Delta {
            return Routed::NotCollection;
        }
        return match covered_collection(mechanism, &patch.path) {
            Some(name) => {
                reject(
                    records,
                    &patch.path,
                    format!(
                        "{} 底下有集合 {name}，不能整份替換或刪除；請寫到集合或它的項目的路徑。",
                        shown_pointer(&patch.path)
                    ),
                );
                Routed::Handled
            }
            None => Routed::NotCollection,
        };
    };
    let collection = Collection { base, spec };
    match patch.path.len() - collection.base.len() {
        0 => collection.whole(tree, patch, records, changes),
        1 => collection.item(tree, patch, records, changes),
        2 => return collection.field(tree, patch, records),
        _ => reject(
            records,
            &patch.path,
            format!(
                "{} 太深了：集合的項目底下只有一層欄位。",
                shown_pointer(&patch.path)
            ),
        ),
    }
    Routed::Handled
}

/// move 只准同一具名集合內改名；其餘碰到集合（含祖先、跨集合、搬進搬出）一律拒收。先驗證再動樹。
fn route_move(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patch: &Patch,
    records: &mut Vec<Record>,
    changes: &mut BTreeMap<String, String>,
) -> Routed {
    let from = locate(mechanism, &patch.from);
    let to = locate(mechanism, &patch.path);
    let touches = from.is_some()
        || to.is_some()
        || covered_collection(mechanism, &patch.from).is_some()
        || covered_collection(mechanism, &patch.path).is_some();
    if !touches {
        return Routed::NotCollection;
    }
    let rename = match (&from, &to) {
        (Some((from_base, spec)), Some((to_base, _)))
            if from_base == to_base
                && spec.keyed
                && patch.from.len() == from_base.len() + 1
                && patch.path.len() == to_base.len() + 1 =>
        {
            Some(Collection {
                base: from_base.clone(),
                spec,
            })
        }
        _ => None,
    };
    match rename {
        Some(collection) => collection.rename(tree, patch, records, changes),
        None => reject(
            records,
            &patch.path,
            format!(
                "{} → {}：集合只准在同一個具名集合裡改名（move 項目到新名字）；搬進、搬出、跨集合都不行。",
                shown_pointer(&patch.from),
                shown_pointer(&patch.path)
            ),
        ),
    }
    Routed::Handled
}

struct Collection<'a> {
    base: Vec<String>,
    spec: &'a CollectionSpec,
}

impl Collection<'_> {
    fn shown(&self) -> String {
        shown_pointer(&self.base)
    }

    fn items(&self, tree: &BTreeMap<String, StateNode>) -> BTreeMap<String, StateNode> {
        match data::node_at(tree, &self.base) {
            Some(StateNode::Branch(children)) => children.clone(),
            _ => BTreeMap::new(),
        }
    }

    /// 整支換成 `items`（空的也留著分支）。
    fn put(
        &self,
        tree: &mut BTreeMap<String, StateNode>,
        items: BTreeMap<String, StateNode>,
    ) -> bool {
        take_node(tree, &self.base);
        insert_node(tree, &self.base, StateNode::Branch(items)).is_ok()
    }

    fn path_of(&self, key: &str) -> Vec<String> {
        let mut path = self.base.clone();
        path.push(key.to_owned());
        path
    }

    /// 項目該怎麼寫（拒收句的後半）。
    fn item_shape(&self) -> String {
        match self.spec.item {
            CollectionItem::Scalar => "每個項目是一個字串".to_owned(),
            CollectionItem::Object => format!(
                "每個項目是物件 {{\"欄位\": \"值\"}}，欄位：{}",
                self.spec
                    .fields
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("、")
            ),
        }
    }

    /// 集合本身：整份換或清空。
    fn whole(
        &self,
        tree: &mut BTreeMap<String, StateNode>,
        patch: &Patch,
        records: &mut Vec<Record>,
        changes: &mut BTreeMap<String, String>,
    ) {
        let path_str = self.base.join(".");
        match patch.op {
            PatchOp::Remove => {
                if self.put(tree, BTreeMap::new()) {
                    changes.insert(path_str, "移除".to_owned());
                }
            }
            PatchOp::Replace | PatchOp::Insert => {
                let value = patch.value.as_ref().unwrap_or(&serde_json::Value::Null);
                // 任何一項致命（名字不合法、形狀不對、同一份裡只差字形的兩個名字）就整份不套用，樹不動
                let items = match (self.spec.keyed, value) {
                    (true, serde_json::Value::Object(map)) => {
                        self.keyed_items(map, &self.items(tree), records)
                    }
                    (false, serde_json::Value::Array(list)) => self.sequence_items(list, records),
                    _ => {
                        let (whole, part) = if self.spec.keyed {
                            ("以名字為鍵的物件", "名字")
                        } else {
                            ("陣列", "索引")
                        };
                        reject(
                            records,
                            &self.base,
                            format!(
                                "{} 是集合，不是字串：整份換要寫成{whole}（{}），改單一項目請寫到 {}/{part}。",
                                self.shown(),
                                self.item_shape(),
                                self.shown()
                            ),
                        );
                        return;
                    }
                };
                let Some(items) = items else {
                    reject(
                        records,
                        &self.base,
                        format!(
                            "{} 整份沒有套用（原因見同一路徑底下的回饋），請修正後整份重寫。",
                            self.shown()
                        ),
                    );
                    return;
                };
                if self.put(tree, items) {
                    changes.insert(path_str, "更新".to_owned());
                }
            }
            PatchOp::Delta | PatchOp::Move => reject(
                records,
                &self.base,
                format!(
                    "{} 是集合，不能 delta；改某個項目的一格請寫到 {}/名字/欄位。",
                    self.shown(),
                    self.shown()
                ),
            ),
        }
    }

    /// 具名集合整份換：依 JSON 順序重編 `_序`。
    fn keyed_items(
        &self,
        map: &serde_json::Map<String, serde_json::Value>,
        old: &BTreeMap<String, StateNode>,
        records: &mut Vec<Record>,
    ) -> Option<BTreeMap<String, StateNode>> {
        let mut items = BTreeMap::new();
        let mut fatal = false;
        for (index, (key, value)) in map.iter().enumerate() {
            let path = self.path_of(key);
            if bad_key(key) {
                reject(records, &path, bad_key_detail(&path));
                fatal = true;
                continue;
            }
            let twins: Vec<&String> = map
                .keys()
                .take(index)
                .filter(|other| data::glyph_equal(other, key))
                .collect();
            if !twins.is_empty() {
                reject(
                    records,
                    &path,
                    format!(
                        "{} 和 {} 只差字形，是同一個名字寫了兩次；同一個項目只寫一次。",
                        shown_pointer(&path),
                        twins
                            .iter()
                            .map(|twin| twin.as_str())
                            .collect::<Vec<_>>()
                            .join("、")
                    ),
                );
                fatal = true;
                continue;
            }
            let previous = match item_key(old, key) {
                Ok(Some(existing)) => old.get(&existing),
                _ => None,
            };
            match self.item_node(value, &path, Some(index), previous, records) {
                Some(node) => {
                    items.insert(key.clone(), node);
                }
                None => fatal = true,
            }
        }
        (!fatal).then_some(items)
    }

    fn sequence_items(
        &self,
        list: &[serde_json::Value],
        records: &mut Vec<Record>,
    ) -> Option<BTreeMap<String, StateNode>> {
        let mut nodes = Vec::new();
        for (index, value) in list.iter().enumerate() {
            let path = self.path_of(&index.to_string());
            nodes.push(self.item_node(value, &path, None, None, records));
        }
        nodes.into_iter().collect::<Option<Vec<_>>>().map(renumber)
    }

    /// 一個項目的值轉成節點：純量項目要純量；物件項目只收描述裡的欄位（字形折疊），丟掉底線鍵，
    /// 不認得的欄位與缺的欄位記回饋。`order` 是具名項目的 `_序`。形狀不對回 None（已記拒收）。
    fn item_node(
        &self,
        value: &serde_json::Value,
        path: &[String],
        order: Option<usize>,
        previous: Option<&StateNode>,
        records: &mut Vec<Record>,
    ) -> Option<StateNode> {
        let shown = shown_pointer(path);
        if self.spec.item == CollectionItem::Scalar {
            if value.is_object() || value.is_array() {
                reject(records, path, format!("{shown}：{}。", self.item_shape()));
                return None;
            }
            return Some(json_to_node(value));
        }
        let Some(map) = value.as_object() else {
            reject(records, path, format!("{shown}：{}。", self.item_shape()));
            return None;
        };
        let mut fields = BTreeMap::new();
        let mut unknown = Vec::new();
        for (key, field_value) in map {
            if key.starts_with('_') {
                continue;
            }
            match self.field_name(key) {
                Some(name) if !field_value.is_object() && !field_value.is_array() => {
                    fields.insert(name, json_to_node(field_value));
                }
                _ => unknown.push(key.clone()),
            }
        }
        // 既有項目整項寫入：沒帶的欄位沿用舊值，不跟著整項換掉（playdata-loss-fixes #10）
        if let Some(StateNode::Branch(previous)) = previous {
            for (name, node) in previous {
                if !name.starts_with('_') && !fields.contains_key(name) {
                    fields.insert(name.clone(), node.clone());
                }
            }
        }
        let missing: Vec<&String> = self
            .spec
            .fields
            .keys()
            .filter(|name| !fields.contains_key(*name))
            .collect();
        if !unknown.is_empty() {
            reject(
                records,
                path,
                format!(
                    "{shown} 的 {} 不是這個集合的欄位（或值不是單一值），已略過；欄位：{}。",
                    unknown.join("、"),
                    self.spec
                        .fields
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("、")
                ),
            );
        }
        if !missing.is_empty() {
            reject(
                records,
                path,
                format!(
                    "{shown} 缺欄位 {}，請用 replace 補上（例 {}/{}）。",
                    missing
                        .iter()
                        .map(|name| name.as_str())
                        .collect::<Vec<_>>()
                        .join("、"),
                    shown,
                    missing[0]
                ),
            );
        }
        if let Some(order) = order {
            fields.insert(
                COLLECTION_ORDER_KEY.to_owned(),
                StateNode::Leaf(order.to_string()),
            );
        }
        Some(StateNode::Branch(fields))
    }

    /// 描述裡的欄位名：精確優先，否則只差字形且唯一。
    fn field_name(&self, key: &str) -> Option<String> {
        if self.spec.fields.contains_key(key) {
            return Some(key.to_owned());
        }
        let folded: Vec<&String> = self
            .spec
            .fields
            .keys()
            .filter(|name| data::glyph_equal(name, key))
            .collect();
        match folded.as_slice() {
            [name] => Some((*name).clone()),
            _ => None,
        }
    }

    /// 單一項目：新增、換掉、刪除。
    fn item(
        &self,
        tree: &mut BTreeMap<String, StateNode>,
        patch: &Patch,
        records: &mut Vec<Record>,
        changes: &mut BTreeMap<String, String>,
    ) {
        if patch.op == PatchOp::Delta {
            reject(
                records,
                &patch.path,
                format!(
                    "{} 是集合的項目，不能 delta；請寫到它底下的欄位。",
                    shown_pointer(&patch.path)
                ),
            );
            return;
        }
        if self.spec.keyed {
            self.keyed_item(tree, patch, records, changes);
        } else {
            self.sequence_item(tree, patch, records, changes);
        }
    }

    fn keyed_item(
        &self,
        tree: &mut BTreeMap<String, StateNode>,
        patch: &Patch,
        records: &mut Vec<Record>,
        changes: &mut BTreeMap<String, String>,
    ) {
        let written = &patch.path[self.base.len()];
        let mut items = self.items(tree);
        let existing = match item_key(&items, written) {
            Ok(existing) => existing,
            Err(keys) => {
                reject(records, &patch.path, ambiguous_detail(&patch.path, &keys));
                return;
            }
        };
        if patch.op == PatchOp::Remove {
            match existing {
                Some(key) => {
                    items.remove(&key);
                    if self.put(tree, items) {
                        changes.insert(self.path_of(&key).join("."), "移除".to_owned());
                    }
                }
                None => reject(
                    records,
                    &patch.path,
                    self.no_item_detail(&patch.path, &items),
                ),
            }
            return;
        }
        let key = match existing {
            Some(key) => key,
            None if patch.op == PatchOp::Replace => {
                let detail = format!(
                    "{}；新項目請用 insert（{}/名字 ＋ 全部欄位）。",
                    self.no_item_detail(&patch.path, &items)
                        .trim_end_matches('。'),
                    self.shown()
                );
                reject(records, &patch.path, detail);
                return;
            }
            None if written == "-" || bad_key(written) => {
                let detail = if written == "-" {
                    format!(
                        "{} 的新項目要用名字當路徑末段（{}/名字），不能用 -。",
                        self.shown(),
                        self.shown()
                    )
                } else {
                    bad_key_detail(&patch.path)
                };
                reject(records, &patch.path, detail);
                return;
            }
            None => written.clone(),
        };
        // 換掉既有項目保留原名次，新項目接在最後
        let order = items.get(&key).and_then(order_of).unwrap_or_else(|| {
            items
                .values()
                .filter_map(order_of)
                .max()
                .map_or(0, |max| max + 1)
        });
        let path = self.path_of(&key);
        let value = patch.value.as_ref().unwrap_or(&serde_json::Value::Null);
        let Some(node) = self.item_node(value, &path, Some(order), items.get(&key), records) else {
            return;
        };
        items.insert(key, node);
        if self.put(tree, items) {
            changes.insert(path.join("."), "更新".to_owned());
        }
    }

    fn sequence_item(
        &self,
        tree: &mut BTreeMap<String, StateNode>,
        patch: &Patch,
        records: &mut Vec<Record>,
        changes: &mut BTreeMap<String, String>,
    ) {
        let items = self.items(tree);
        let mut nodes: Vec<StateNode> = ordered_sequence(&items);
        let written = &patch.path[self.base.len()];
        let limit = if patch.op == PatchOp::Insert {
            nodes.len()
        } else {
            nodes.len().saturating_sub(1)
        };
        let index = match written.as_str() {
            "-" if patch.op == PatchOp::Insert => Some(nodes.len()),
            text => text.parse::<usize>().ok().filter(|index| {
                *index <= limit && (patch.op == PatchOp::Insert || !nodes.is_empty())
            }),
        };
        let Some(index) = index else {
            reject(
                records,
                &patch.path,
                format!(
                    "{} 沒有這一項（目前 {} 項，索引從 0 起）；整批換請 replace {} 寫成陣列。",
                    shown_pointer(&patch.path),
                    nodes.len(),
                    self.shown()
                ),
            );
            return;
        };
        let path = self.path_of(&index.to_string());
        match patch.op {
            PatchOp::Remove => {
                nodes.remove(index);
            }
            _ => {
                let value = patch.value.as_ref().unwrap_or(&serde_json::Value::Null);
                let Some(node) = self.item_node(value, &path, None, None, records) else {
                    return;
                };
                if patch.op == PatchOp::Insert {
                    nodes.insert(index, node);
                } else {
                    nodes[index] = node;
                }
            }
        }
        let mark = if patch.op == PatchOp::Remove {
            "移除"
        } else {
            "更新"
        };
        if self.put(tree, renumber(nodes)) {
            changes.insert(path.join("."), mark.to_owned());
        }
    }

    /// 項目欄位：項目要在、欄位要是描述裡的（字形折疊），折好路徑交回一般流程套規則。
    fn field(
        &self,
        tree: &BTreeMap<String, StateNode>,
        patch: &Patch,
        records: &mut Vec<Record>,
    ) -> Routed {
        if self.spec.item == CollectionItem::Scalar {
            reject(
                records,
                &patch.path,
                format!(
                    "{} 的項目是單一值，沒有欄位；請寫到 {}。",
                    self.shown(),
                    self.shown()
                ),
            );
            return Routed::Handled;
        }
        let items = self.items(tree);
        let written = &patch.path[self.base.len()];
        let item = if self.spec.keyed {
            item_key(&items, written)
        } else {
            Ok(items.contains_key(written).then(|| written.clone()))
        };
        let item = match item {
            Ok(Some(item)) => item,
            Ok(None) => {
                let path = &patch.path[..self.base.len() + 1];
                let detail = format!(
                    "{}；新項目請用 insert（{}/名字 ＋ 全部欄位）。",
                    self.no_item_detail(path, &items).trim_end_matches('。'),
                    self.shown()
                );
                reject(records, &patch.path, detail);
                return Routed::Handled;
            }
            Err(keys) => {
                reject(records, &patch.path, ambiguous_detail(&patch.path, &keys));
                return Routed::Handled;
            }
        };
        let written_field = &patch.path[self.base.len() + 1];
        let Some(field) = self.field_name(written_field) else {
            reject(
                records,
                &patch.path,
                format!(
                    "{} 不是這個集合的欄位；欄位：{}。",
                    shown_pointer(&patch.path),
                    self.spec
                        .fields
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("、")
                ),
            );
            return Routed::Handled;
        };
        // 欄位是單一值：物件、陣列會長成分支，殼取不到、也繞過深度限制
        if patch
            .value
            .as_ref()
            .is_some_and(|value| value.is_object() || value.is_array())
        {
            reject(
                records,
                &patch.path,
                format!(
                    "{} 是單一值欄位，請寫字串或數字。",
                    shown_pointer(&patch.path)
                ),
            );
            return Routed::Handled;
        }
        let mut path = self.path_of(&item);
        path.push(field);
        // 描述裡有、項目還缺的欄位：replace 直接建立（缺欄位回饋教的就是 replace）
        let op = if patch.op == PatchOp::Replace && data::node_at(tree, &path).is_none() {
            PatchOp::Insert
        } else {
            patch.op
        };
        Routed::Delegate(Patch {
            op,
            path,
            ..patch.clone()
        })
    }

    /// 同一具名集合內改名：來源要在、新名合法且沒被占（含只差字形），名次跟著走。
    fn rename(
        &self,
        tree: &mut BTreeMap<String, StateNode>,
        patch: &Patch,
        records: &mut Vec<Record>,
        changes: &mut BTreeMap<String, String>,
    ) {
        let mut items = self.items(tree);
        let from = match item_key(&items, &patch.from[self.base.len()]) {
            Ok(Some(from)) => from,
            Ok(None) => {
                reject(
                    records,
                    &patch.from,
                    self.no_item_detail(&patch.from, &items),
                );
                return;
            }
            Err(keys) => {
                reject(records, &patch.from, ambiguous_detail(&patch.from, &keys));
                return;
            }
        };
        let to = &patch.path[self.base.len()];
        if bad_key(to) || to == "-" {
            reject(records, &patch.path, bad_key_detail(&patch.path));
            return;
        }
        let taken = items
            .keys()
            .any(|key| *key != from && (key == to || data::glyph_equal(key, to)));
        if taken {
            reject(
                records,
                &patch.path,
                format!(
                    "{} 已經有這個項目，不能改名成它。",
                    shown_pointer(&patch.path)
                ),
            );
            return;
        }
        let Some(node) = items.remove(&from) else {
            return;
        };
        items.insert(to.clone(), node);
        if self.put(tree, items) {
            changes.insert(self.path_of(to).join("."), "搬移".to_owned());
        }
    }

    fn no_item_detail(&self, path: &[String], items: &BTreeMap<String, StateNode>) -> String {
        let names: Vec<&str> = items
            .keys()
            .filter(|key| !key.starts_with('_'))
            .map(String::as_str)
            .collect();
        if names.is_empty() {
            format!(
                "{} 不存在：{} 目前沒有項目。",
                shown_pointer(path),
                self.shown()
            )
        } else {
            format!(
                "{} 不存在：{} 目前的項目有 {}。",
                shown_pointer(path),
                self.shown(),
                names.join("、")
            )
        }
    }
}

/// 項目鍵對回既有項目：精確優先，否則只差字形且唯一；多個回 Err（候選）、沒有回 None。
fn item_key(
    items: &BTreeMap<String, StateNode>,
    written: &str,
) -> Result<Option<String>, Vec<String>> {
    if items.contains_key(written) {
        return Ok(Some(written.to_owned()));
    }
    let folded: Vec<String> = items
        .keys()
        .filter(|key| !key.starts_with('_') && data::glyph_equal(key, written))
        .cloned()
        .collect();
    match folded.len() {
        0 => Ok(None),
        1 => Ok(folded.into_iter().next()),
        _ => Err(folded),
    }
}

fn order_of(node: &StateNode) -> Option<usize> {
    match node {
        StateNode::Branch(children) => match children.get(COLLECTION_ORDER_KEY) {
            Some(StateNode::Leaf(value)) => value.trim().parse().ok(),
            _ => None,
        },
        StateNode::Leaf(_) => None,
    }
}

/// 序列的項目照索引數值排成一列（不照字碼，10 在 9 後面）。
fn ordered_sequence(items: &BTreeMap<String, StateNode>) -> Vec<StateNode> {
    let mut entries: Vec<(usize, &StateNode)> = items
        .iter()
        .filter_map(|(key, node)| key.parse::<usize>().ok().map(|index| (index, node)))
        .collect();
    entries.sort_by_key(|(index, _)| *index);
    entries.into_iter().map(|(_, node)| node.clone()).collect()
}

fn renumber(nodes: Vec<StateNode>) -> BTreeMap<String, StateNode> {
    nodes
        .into_iter()
        .enumerate()
        .map(|(index, node)| (index.to_string(), node))
        .collect()
}

fn bad_key_detail(path: &[String]) -> String {
    format!(
        "{} 的名字不能是空的、-、底線開頭，或含 .、{{、}}、換行。",
        shown_pointer(path)
    )
}

fn ambiguous_detail(path: &[String], keys: &[String]) -> String {
    format!(
        "{} 對到好幾個只差字形的項目（{}），請照狀態裡的原字寫。",
        shown_pointer(path),
        keys.join("、")
    )
}

/// 觸發條件 `contains` 指到集合時比對的文字：項目鍵與項目內的值（欄位名與底線鍵不算），一行一個。
pub(super) fn contains_text(items: &BTreeMap<String, StateNode>) -> String {
    fn values(node: &StateNode, parts: &mut Vec<String>) {
        match node {
            StateNode::Leaf(value) => parts.push(value.clone()),
            StateNode::Branch(children) => children
                .iter()
                .filter(|(key, _)| !key.starts_with('_'))
                .for_each(|(_, child)| values(child, parts)),
        }
    }
    let mut parts = Vec::new();
    for (key, item) in items.iter().filter(|(key, _)| !key.starts_with('_')) {
        parts.push(key.clone());
        values(item, &mut parts);
    }
    parts.join("\n")
}
