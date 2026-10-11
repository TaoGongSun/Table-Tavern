//! initvar 清單模式：`[initvar]` 條目建樹模式初始樹。具名分支成 Branch（值空的鍵是空分支），
//! 清單整支成一片 JSON 葉子（同 stat_data 投影的格式），元素照 YAML 推斷型別並剝 MVU 元資料，
//! 切變數模式物化時才還原得成真陣列、GM 的索引路徑也走得了陣列語意。
use super::{flatten, split_quoted, yaml_colon, Accept, Mode, Node, Parser, Style};
use crate::data::message_vars::{clean, restore_leaf, strip_meta_leaf, Json};
use crate::data::StateNode;
use std::collections::BTreeMap;

/// 解析 `[initvar]` 條目內容成初始樹。同一路徑重複出現時沿用舊的合併語意：分支合併、葉子後者為準、
/// 已是葉子的位置不被後到的分支取代。
pub fn parse_initvar_tree(content: &str) -> BTreeMap<String, StateNode> {
    let mut parser = Parser::new(content, Mode::Initvar);
    let entries = parser.entries(Accept::All);
    let mut tree = BTreeMap::new();
    for (key, node) in flatten(entries) {
        merge(&mut tree, key, to_state(node));
    }
    tree
}

fn merge(children: &mut BTreeMap<String, StateNode>, key: String, node: StateNode) {
    match (children.get_mut(&key), node) {
        (Some(StateNode::Branch(existing)), StateNode::Branch(incoming)) => {
            for (child_key, child) in incoming {
                merge(existing, child_key, child);
            }
        }
        (Some(StateNode::Leaf(_)), StateNode::Branch(_)) => {}
        (_, node) => {
            children.insert(key, node);
        }
    }
}

fn to_state(node: Node) -> StateNode {
    match node {
        Node::Map(pairs) => {
            let mut children = BTreeMap::new();
            for (key, child) in pairs {
                merge(&mut children, key, to_state(child));
            }
            StateNode::Branch(children)
        }
        Node::Empty | Node::Null => StateNode::Branch(BTreeMap::new()),
        Node::List(items) => {
            let array = Json::Array(items.into_iter().map(to_json).collect());
            StateNode::Leaf(clean(&array, None).to_text())
        }
        Node::Scalar(text, Style::Plain) => StateNode::Leaf(plain_leaf(text)),
        Node::Scalar(text, _) => StateNode::Leaf(text),
    }
}

/// 清單外的純量葉子：合法 JSON（含 VWD）剝元資料、沒有元資料留原文；單層流式序列轉成 JSON 陣列；
/// 其餘原文。型別留給物化時推斷（同舊行為）。
fn plain_leaf(text: String) -> String {
    if json_literal(&text).is_some() {
        return strip_meta_leaf(&text).unwrap_or(text);
    }
    match flow_sequence(&text) {
        Some(array) => clean(&array, None).to_text(),
        None => text,
    }
}

fn to_json(node: Node) -> Json {
    match node {
        Node::Scalar(text, Style::Plain) => infer(&text),
        Node::Scalar(text, _) => Json::String(text),
        Node::Empty | Node::Null => Json::Null,
        Node::Map(pairs) => {
            let mut object = Json::empty_object();
            for (key, child) in pairs {
                object.insert(&key, to_json(child));
            }
            object
        }
        Node::List(items) => Json::Array(items.into_iter().map(to_json).collect()),
    }
}

/// `[`／`{` 開頭且是合法 JSON。
fn json_literal(text: &str) -> Option<Json> {
    if !text.starts_with(['[', '{']) {
        return None;
    }
    match restore_leaf(text, None) {
        Json::String(_) => None,
        value => Some(value),
    }
}

/// 不帶引號的純量照 YAML 推斷：`~`／`null`、`true`／`false`、數字、合法 JSON、單層流式序列，其餘字串。
fn infer(text: &str) -> Json {
    match text {
        "~" => Json::Null,
        _ if text.starts_with(['[', '{']) => json_literal(text)
            .or_else(|| flow_sequence(text))
            .unwrap_or_else(|| Json::String(text.to_owned())),
        _ => restore_leaf(text, None),
    }
}

/// 單層流式序列 `[火, "冰, 霜", '']`：元素是純量（可帶引號，引號內的逗號照解）；
/// 巢狀容器、流式鍵值對、空元素一律回 None（整段回退原文，不解析一部分）。
fn flow_sequence(text: &str) -> Option<Json> {
    let inner = text.strip_prefix('[')?.strip_suffix(']')?;
    let mut items = Vec::new();
    let mut rest = inner.trim_start();
    if rest.is_empty() {
        return Some(Json::Array(items));
    }
    loop {
        rest = rest.trim_start();
        let (value, after) = if rest.starts_with(['"', '\'']) {
            let (inner, after) = split_quoted(rest)?;
            (Json::String(inner), after)
        } else {
            let end = rest.find(',').unwrap_or(rest.len());
            let token = rest[..end].trim();
            if token.is_empty()
                || token.contains(['[', ']', '{', '}'])
                || yaml_colon(token).is_some()
            {
                return None;
            }
            (infer(token), &rest[end..])
        };
        items.push(value);
        let after = after.trim_start();
        if after.is_empty() {
            break;
        }
        let after = after.strip_prefix(',')?;
        if after.trim().is_empty() {
            break;
        }
        rest = after;
    }
    Some(Json::Array(items))
}
