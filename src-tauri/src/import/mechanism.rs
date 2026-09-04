use super::card_io::{decode_png_character, PNG_MAGIC};
use crate::data::{self, FieldKind, FieldRule, StateNode};
use crate::mechanism::{self, Record, RecordKind};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

/// 機制資料是附加資訊；讀不到這桌狀態時仍要讓玩家帶走可用的普通角色卡。
pub(super) fn table_tavern_extension(root: &Path, world_id: &str, name: &str) -> Value {
    let Ok(world) = data::read_state(root, world_id) else {
        return json!({});
    };
    let prefix = format!("{name}.");
    let rules: BTreeMap<_, _> = world
        .mechanism
        .rules
        .iter()
        .filter_map(|(path, rule)| {
            (rule.branch.as_deref() == Some(name))
                .then(|| {
                    path.strip_prefix(&prefix)
                        .map(|relative| (relative.to_owned(), rule.clone()))
                })
                .flatten()
        })
        .collect();
    let initial = world.state.tree.get(name).cloned();
    if rules.is_empty() && initial.is_none() {
        return json!({});
    }
    let mut table_tavern = serde_json::Map::new();
    table_tavern.insert("version".to_owned(), json!(1));
    if !rules.is_empty() {
        table_tavern.insert(
            "rules".to_owned(),
            serde_json::to_value(rules).unwrap_or_default(),
        );
    }
    if let Some(initial) = initial {
        table_tavern.insert(
            "initial".to_owned(),
            serde_json::to_value(initial).unwrap_or_default(),
        );
    }
    json!({ "table_tavern": table_tavern })
}

/// 壞掉的擴充資料只略過，因為角色本體已經安全存檔，不能被可選機制拖垮。
/// 世界書路徑用：從原始卡檔（PNG／JSON）取出本 app 自己匯出的 `extensions.table_tavern` 再套用。
/// 角色卡路徑在 import_character 內已直接套過；兩條路徑都要收，同一張卡不會因為換個身分匯入就少半套機制。
pub fn import_card_extension(root: &Path, world_id: &str, name: &str, bytes: &[u8]) {
    let json_bytes = if bytes.starts_with(PNG_MAGIC) {
        match decode_png_character(bytes) {
            Ok(json_bytes) => json_bytes,
            Err(_) => return,
        }
    } else {
        bytes.to_vec()
    };
    let Ok(value) = serde_json::from_slice::<Value>(&json_bytes) else {
        return;
    };
    let card_data = value
        .get("data")
        .filter(|data| data.is_object())
        .unwrap_or(&value);
    import_table_tavern_extension(root, world_id, name, card_data);
}

pub(super) fn import_table_tavern_extension(root: &Path, world_id: &str, name: &str, card_data: &Value) {
    let Some(extension) = card_data
        .get("extensions")
        .and_then(|extensions| extensions.get("table_tavern"))
    else {
        return;
    };
    let Ok(mut world) = data::read_state(root, world_id) else {
        return;
    };
    let mut changed = false;
    if let Some(rules) = extension.get("rules") {
        if let Ok(rules) = serde_json::from_value::<BTreeMap<String, FieldRule>>(rules.clone()) {
            for (relative, mut rule) in rules {
                if relative.is_empty() {
                    continue;
                }
                rule.branch = Some(name.to_owned());
                world
                    .mechanism
                    .rules
                    .insert(format!("{name}.{relative}"), rule);
                changed = true;
            }
        }
    }
    if let Some(initial) = extension.get("initial") {
        if let Ok(initial) = serde_json::from_value::<StateNode>(initial.clone()) {
            match world.state.tree.get_mut(name) {
                Some(existing) => merge_state_node(existing, initial, true),
                None => {
                    world.state.tree.insert(name.to_owned(), initial);
                }
            }
            changed = true;
        }
    }
    if changed {
        let _ = data::write_state(root, world_id, &world);
    }
}

fn merge_state_node(existing: &mut StateNode, incoming: StateNode, overwrite: bool) {
    match (existing, incoming) {
        (StateNode::Branch(existing), StateNode::Branch(incoming)) => {
            for (key, node) in incoming {
                match existing.get_mut(&key) {
                    Some(current) => merge_state_node(current, node, overwrite),
                    None => {
                        existing.insert(key, node);
                    }
                }
            }
        }
        (existing, incoming) if overwrite => *existing = incoming,
        _ => {}
    }
}

/// 匯入 MVU 機制鷹架與固定型 EJS：`[initvar]` 停用條目給初始狀態樹、`[mvu_update]` 條目給欄位規則表，
/// EJS 只收成可本地求值的觸發表；一次掃完只寫一次 state.json。壞格式一律略過，不阻斷正常匯入。
pub fn import_mechanism(root: &Path, world_id: &str, book: &Value) {
    let Some(entries) = book.get("entries") else {
        return;
    };
    let entries: Vec<&Value> = match entries {
        Value::Array(entries) => entries.iter().collect(),
        Value::Object(entries) => entries.values().collect(),
        _ => return,
    };

    let initial_tree = extract_initial_tree(&entries);
    let (rules, mvu_seen) = extract_field_rules(&entries);
    let (triggers, skipped) = extract_triggers(&entries);
    let incremental = initial_tree.is_some() || mvu_seen;
    if initial_tree.is_none() && rules.is_empty() && triggers.is_empty() && !incremental {
        return;
    }

    let Ok(mut world) = data::read_state(root, world_id) else {
        return;
    };
    if let Some(tree) = initial_tree {
        for (key, node) in tree {
            match world.state.tree.get_mut(&key) {
                Some(existing) => merge_state_node(existing, node, false),
                None => {
                    world.state.tree.insert(key, node);
                }
            }
        }
    }
    for (path, rule) in rules {
        world.mechanism.rules.insert(path, rule);
    }
    for trigger in triggers {
        if let Some(flag) = trigger.flag.as_ref() {
            let mut rule = FieldRule::for_kind(FieldKind::ReadOnly);
            rule.branch = flag.split('.').next().map(str::to_owned);
            world.mechanism.rules.insert(flag.clone(), rule);
        }
        if let Some(index) = world
            .mechanism
            .triggers
            .iter()
            .position(|existing| existing.id == trigger.id)
        {
            world.mechanism.triggers[index] = trigger;
        } else {
            world.mechanism.triggers.push(trigger);
        }
    }
    if incremental {
        world.mechanism.incremental = true;
    }
    let _ = data::write_state(root, world_id, &world);
    mechanism::append_log(root, world_id, world.current_scene, &skipped);
}

/// EJS 原文從不送模型：可辨識的才轉成觸發表，其餘留一筆記帳讓玩家知道沒有偷偷執行。
fn extract_triggers(entries: &[&Value]) -> (Vec<data::Trigger>, Vec<Record>) {
    let mut triggers = Vec::new();
    let mut skipped = Vec::new();
    for entry in entries {
        let Some(content) = entry.get("content").and_then(Value::as_str) else {
            continue;
        };
        if !content.contains("<%") {
            continue;
        }
        let title = entry
            .get("comment")
            .and_then(Value::as_str)
            .or_else(|| entry.get("title").and_then(Value::as_str))
            .unwrap_or_default();
        if let Some(trigger) = crate::ejs::parse_triggers(title, content) {
            if let Some(index) = triggers
                .iter()
                .position(|existing: &data::Trigger| existing.id == trigger.id)
            {
                triggers[index] = trigger;
            } else {
                triggers.push(trigger);
            }
        } else {
            skipped.push(Record {
                kind: RecordKind::Skipped,
                path: title.to_owned(),
                detail: "卡片腳本認不出來，沒轉成觸發表，預設不送模型。".to_owned(),
            });
        }
    }
    (triggers, skipped)
}

fn entry_marker(entry: &Value) -> Option<String> {
    entry
        .get("comment")
        .and_then(Value::as_str)
        .or_else(|| entry.get("title").and_then(Value::as_str))
        .map(|marker| marker.trim().to_ascii_lowercase())
}

/// `[initvar]` 初始狀態樹：只認第一條「停用且標記匹配」的條目，內容壞掉就整段放棄。
fn extract_initial_tree(entries: &[&Value]) -> Option<BTreeMap<String, StateNode>> {
    let entry = entries.iter().find(|entry| {
        let Some(marker) = entry_marker(entry) else {
            return false;
        };
        let disabled = entry.get("enabled").and_then(Value::as_bool) == Some(false)
            || entry.get("disable").and_then(Value::as_bool) == Some(true);
        disabled && marker.starts_with("[initvar]")
    })?;
    let content = entry.get("content").and_then(Value::as_str)?;
    if content.trim().is_empty() {
        return None;
    }
    let mut tree = BTreeMap::new();
    for (path, value) in crate::transport::parse_indented_fields(content) {
        insert_initial_tree_node(&mut tree, &path, value);
    }
    (!tree.is_empty()).then_some(tree)
}

/// `[mvu_update]` 欄位規則表：逐條處理（可能不只一條）；回傳規則表與是否掃到過標記
/// （掃到但抽不出任何規則，這桌仍要標成增量桌——模型已經在照協定回覆）。
fn extract_field_rules(entries: &[&Value]) -> (BTreeMap<String, FieldRule>, bool) {
    let mut rules = BTreeMap::new();
    let mut seen = false;
    for entry in entries {
        let Some(marker) = entry_marker(entry) else {
            continue;
        };
        if !marker.starts_with("[mvu_update]") {
            continue;
        }
        seen = true;
        if let Some(content) = entry.get("content").and_then(Value::as_str) {
            collect_field_rules(content, &mut rules);
        }
    }
    (rules, seen)
}

/// 一條規則路徑抽到的三個屬性值（type, range, format），同一條路徑的多筆屬性合併於此。
type RuleAttrValues = (Option<String>, Option<String>, Option<String>);

/// 從一段規則表文字抽欄位規則：只收路徑長度 ≥ 3 且最後一段是 type／range／format 的項目
/// （這條過濾就把同前綴的「輸出格式」說明條目整條擋掉，不必特判）；規則路徑＝去掉根與屬性名，
/// 任何一段是 check 就丟掉；同一條路徑的多個屬性合併成一條規則。
fn collect_field_rules(content: &str, rules: &mut BTreeMap<String, FieldRule>) {
    let mut attrs: BTreeMap<Vec<String>, RuleAttrValues> = BTreeMap::new();
    for (path, value) in crate::transport::parse_indented_fields(content) {
        let Some(value) = value else { continue };
        if path.len() < 3 || path.iter().any(|segment| segment == "check") {
            continue;
        }
        let attribute = path[path.len() - 1].as_str();
        if !matches!(attribute, "type" | "range" | "format") {
            continue;
        }
        let Some(expanded) = expand_rule_paths(&path[1..path.len() - 1]) else {
            continue;
        };
        for rule_path in expanded {
            let entry = attrs.entry(rule_path).or_insert((None, None, None));
            match attribute {
                "type" => entry.0 = Some(value.clone()),
                "range" => entry.1 = Some(value.clone()),
                "format" => entry.2 = Some(value.clone()),
                _ => unreachable!("已由上面的 matches! 篩過"),
            }
        }
    }
    for (rule_path, (type_value, range_value, format_value)) in attrs {
        let Some(rule) = build_field_rule(
            &rule_path,
            type_value.as_deref(),
            range_value.as_deref(),
            format_value.as_deref(),
        ) else {
            continue;
        };
        rules.insert(rule_path.join("."), rule);
    }
}

/// ST 怪癖正規化（只准關在這裡）：剝引號、拆 `.` 合併段、展開 `${a|b}`（多選一）／
/// `${x}`（換成萬用段 `*`）／`a/b/c`（同層併寫的多個欄位）；組合數超過 32 就整條放棄。
fn expand_rule_paths(raw_segments: &[String]) -> Option<Vec<Vec<String>>> {
    let mut segments = Vec::new();
    for raw in raw_segments {
        for part in strip_enclosing_quotes(raw).split('.') {
            if !part.is_empty() {
                segments.push(part.to_owned());
            }
        }
    }
    if segments.is_empty() {
        return None;
    }
    let choices: Vec<Vec<String>> = segments
        .iter()
        .map(|segment| expand_segment(segment))
        .collect();
    let total = choices
        .iter()
        .try_fold(1usize, |acc, choice| acc.checked_mul(choice.len()))?;
    if total == 0 || total > 32 {
        return None;
    }
    let mut results = vec![Vec::new()];
    for choice in &choices {
        let mut next = Vec::with_capacity(results.len() * choice.len());
        for existing in &results {
            for candidate in choice {
                let mut combo = existing.clone();
                combo.push(candidate.clone());
                next.push(combo);
            }
        }
        results = next;
    }
    Some(results)
}

fn strip_enclosing_quotes(segment: &str) -> String {
    let trimmed = segment.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        return trimmed[1..trimmed.len() - 1].to_owned();
    }
    trimmed.to_owned()
}

fn expand_segment(segment: &str) -> Vec<String> {
    if let Some(inner) = segment
        .strip_prefix("${")
        .and_then(|rest| rest.strip_suffix('}'))
    {
        if inner.contains('|') {
            let parts: Vec<String> = inner
                .split('|')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(str::to_owned)
                .collect();
            if !parts.is_empty() {
                return parts;
            }
        }
        return vec!["*".to_owned()];
    }
    if segment.contains('/') {
        let parts: Vec<&str> = segment.split('/').collect();
        if parts
            .iter()
            .all(|part| !part.is_empty() && !part.contains(char::is_whitespace))
        {
            return parts.into_iter().map(str::to_owned).collect();
        }
    }
    vec![segment.to_owned()]
}

/// 屬性三元組（type／range／format）→ FieldRule；完全推不出 kind 的路徑不建規則。
fn build_field_rule(
    rule_path: &[String],
    type_value: Option<&str>,
    range_value: Option<&str>,
    format_value: Option<&str>,
) -> Option<FieldRule> {
    let is_pair_format = format_value
        .is_some_and(|value| value.contains("当前值/上限值") || value.contains("當前值/上限值"));
    let kind = if is_pair_format {
        Some(FieldKind::Pair)
    } else {
        match type_value {
            Some("number") => Some(FieldKind::Number),
            Some("string") => Some(FieldKind::Text),
            _ => None,
        }
    };
    let mut kind = kind?;
    let last = rule_path.last()?.to_ascii_lowercase();
    if kind == FieldKind::Number
        && last
            .strip_prefix("roll")
            .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_digit()))
    {
        kind = FieldKind::Roll;
    }
    let mut rule = FieldRule::for_kind(kind);
    if let Some((min, max)) = range_value.and_then(|range| range.split_once('-')) {
        if let (Ok(min), Ok(max)) = (min.trim().parse::<f64>(), max.trim().parse::<f64>()) {
            rule.min = Some(min);
            rule.max = Some(max);
        }
    }
    rule.branch = rule_path.first().cloned();
    Some(rule)
}

fn insert_initial_tree_node(
    children: &mut BTreeMap<String, StateNode>,
    path: &[String],
    value: Option<String>,
) {
    let Some((key, rest)) = path.split_first() else {
        return;
    };
    if rest.is_empty() {
        match value {
            Some(value) => {
                children.insert(key.clone(), StateNode::Leaf(value));
            }
            None => {
                children
                    .entry(key.clone())
                    .or_insert_with(|| StateNode::Branch(BTreeMap::new()));
            }
        }
        return;
    }
    let child = children
        .entry(key.clone())
        .or_insert_with(|| StateNode::Branch(BTreeMap::new()));
    if let StateNode::Branch(children) = child {
        insert_initial_tree_node(children, rest, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::import_character;
    use crate::import::test_support::TestRoot;
    use std::fs;

    /// [initvar] 初始樹要保留巢狀容器、引號內值與玩家巨集字面。
    #[test]
    fn initvar_entry_becomes_initial_tree() {
        let root = TestRoot::new("initvar-tree");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let raw = json!({
            "data": {
                "name": "莉亞",
                "character_book": {
                    "entries": [{
                        "comment": "[initvar]变量初始化勿开",
                        "enabled": false,
                        "content": "World:\n  Time: \"清晨\"\n  Invasion: 1\nPlayer:\n  Name: \"{{user}}\"\n  HP: \"10/10\"\n  Inventory: {}\nHeroes:\n  亞瑟:\n    Outfit:\n      上装: \"白袍\""
                    }]
                }
            }
        })
        .to_string();

        import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff").unwrap();
        let state = data::read_state(root.path(), &world_id).unwrap();
        let world = match state.state.tree.get("World") {
            Some(StateNode::Branch(children)) => children,
            _ => panic!("缺少 World 分支"),
        };
        let player = match state.state.tree.get("Player") {
            Some(StateNode::Branch(children)) => children,
            _ => panic!("缺少 Player 分支"),
        };
        let heroes = match state.state.tree.get("Heroes") {
            Some(StateNode::Branch(children)) => children,
            _ => panic!("缺少 Heroes 分支"),
        };
        let arthur = match heroes.get("亞瑟") {
            Some(StateNode::Branch(children)) => children,
            _ => panic!("缺少亞瑟分支"),
        };
        let outfit = match arthur.get("Outfit") {
            Some(StateNode::Branch(children)) => children,
            _ => panic!("缺少 Outfit 分支"),
        };
        assert_eq!(world.get("Time"), Some(&StateNode::Leaf("清晨".to_owned())));
        assert_eq!(
            player.get("Name"),
            Some(&StateNode::Leaf("{{user}}".to_owned()))
        );
        assert_eq!(
            player.get("Inventory"),
            Some(&StateNode::Branch(BTreeMap::new()))
        );
        assert_eq!(
            outfit.get("上装"),
            Some(&StateNode::Leaf("白袍".to_owned()))
        );
    }

    /// 重複匯入初始樹只能補空缺，已經演進的狀態不可倒回初始值。
    #[test]
    fn initvar_fills_gaps_without_overwriting_existing_values() {
        let root = TestRoot::new("initvar-merge");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let mut state = data::read_state(root.path(), &world_id).unwrap();
        state.state.tree.insert(
            "World".to_owned(),
            StateNode::Branch(BTreeMap::from([(
                "Time".to_owned(),
                StateNode::Leaf("深夜".to_owned()),
            )])),
        );
        data::write_state(root.path(), &world_id, &state).unwrap();
        let raw = json!({
            "data": {
                "name": "莉亞",
                "character_book": {
                    "entries": [{
                        "comment": "[initvar] 初始值",
                        "disable": true,
                        "content": "World:\n  Time: 清晨\n  Invasion: 1\nPlayer:\n  HP: 10/10"
                    }]
                }
            }
        })
        .to_string();

        import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff").unwrap();
        let state = data::read_state(root.path(), &world_id).unwrap();
        let world = match state.state.tree.get("World") {
            Some(StateNode::Branch(children)) => children,
            _ => panic!("缺少 World 分支"),
        };
        assert_eq!(world.get("Time"), Some(&StateNode::Leaf("深夜".to_owned())));
        assert_eq!(
            world.get("Invasion"),
            Some(&StateNode::Leaf("1".to_owned()))
        );
        assert_eq!(
            state.state.tree.get("Player"),
            Some(&StateNode::Branch(BTreeMap::from([(
                "HP".to_owned(),
                StateNode::Leaf("10/10".to_owned()),
            )])))
        );
    }

    /// 壞掉的 [initvar] 內容不可阻斷角色與世界書條目的正常匯入。
    #[test]
    fn broken_initvar_never_blocks_import() {
        let root = TestRoot::new("initvar-broken");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let raw = json!({
            "data": {
                "name": "莉亞",
                "character_book": {
                    "entries": [{
                        "comment": "[initvar] 初始值",
                        "enabled": false,
                        "keys": ["初始"],
                        "content": "這不是 YAML"
                    }]
                }
            }
        })
        .to_string();

        let meta = import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff").unwrap();
        assert!(data::read_character(root.path(), &world_id, &meta.id)
            .unwrap()
            .private_md
            .contains("- **初始**：這不是 YAML"));
        assert!(data::read_state(root.path(), &world_id)
            .unwrap()
            .state
            .tree
            .is_empty());
    }

    /// 啟用中的同名條目不是 MVU 初始狀態樹，必須完全略過。
    #[test]
    fn enabled_initvar_entry_is_ignored() {
        let root = TestRoot::new("initvar-enabled");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let raw = json!({
            "data": {
                "name": "莉亞",
                "character_book": {
                    "entries": [{
                        "comment": "[initvar] 初始值",
                        "enabled": true,
                        "content": "World:\n  Time: 清晨"
                    }]
                }
            }
        })
        .to_string();

        import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff").unwrap();
        assert!(data::read_state(root.path(), &world_id)
            .unwrap()
            .state
            .tree
            .is_empty());
    }

    /// 真實形狀的 `[mvu_update]` 規則表：type／range／format 三個屬性各種組合都要抽對，
    /// 抽不出數字上下限的路徑（非數字 range）不填 min/max，匯入後這桌要標成增量桌。
    #[test]
    fn mvu_update_entry_extracts_field_rules_and_marks_incremental_table() {
        let root = TestRoot::new("mvu-rules");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let content = "变量更新规则:\n\
             \x20 World:\n\
             \x20   Invasion:\n\
             \x20     type: number\n\
             \x20     range: 0-100\n\
             \x20     check:\n\
             \x20       - 初始为1% (魔王降临前兆)。\n\
             \x20   Roll100:\n\
             \x20     type: number\n\
             \x20     range: 1-100\n\
             \x20     check:\n\
             \x20       - 每次剧情输出开始时输出的第一个随机数\n\
             \x20 Player:\n\
             \x20   Level:\n\
             \x20     type: string\n\
             \x20     range: 零阶-六阶\n\
             \x20   HP/SP/MP:\n\
             \x20     type: string\n\
             \x20     format: \"当前值/上限值\"\n\
             \x20 Heroes:\n\
             \x20   '${勇者姓名}':\n\
             \x20     Affection:\n\
             \x20       type: number\n\
             \x20       range: 0-200\n";
        let raw = json!({
            "data": {
                "name": "莉亞",
                "character_book": {
                    "entries": [{
                        "comment": "[mvu_update]变量更新规则",
                        "enabled": true,
                        "content": content
                    }]
                }
            }
        })
        .to_string();

        import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff").unwrap();
        let state = data::read_state(root.path(), &world_id).unwrap();
        assert!(state.mechanism.incremental);
        let rules = &state.mechanism.rules;

        let invasion = &rules["World.Invasion"];
        assert_eq!(invasion.kind, data::FieldKind::Number);
        assert_eq!(invasion.min, Some(0.0));
        assert_eq!(invasion.max, Some(100.0));

        assert_eq!(rules["World.Roll100"].kind, data::FieldKind::Roll);

        for field in ["HP", "SP", "MP"] {
            assert_eq!(
                rules[&format!("Player.{field}")].kind,
                data::FieldKind::Pair
            );
        }

        let level = &rules["Player.Level"];
        assert_eq!(level.kind, data::FieldKind::Text);
        assert_eq!(level.min, None);
        assert_eq!(level.max, None);

        let affection = &rules["Heroes.*.Affection"];
        assert_eq!(affection.kind, data::FieldKind::Number);
        assert_eq!(affection.min, Some(0.0));
        assert_eq!(affection.max, Some(200.0));
    }

    /// EJS 只收可靜態還原的觸發表；認不出的原文仍停用並留 skipped 記帳。
    #[test]
    fn ejs_entries_become_triggers_and_log_unrecognized_scripts() {
        let root = TestRoot::new("ejs-triggers");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let supported = r#"<%_
var invasion = getvar('stat_data.World.Invasion', { defaults: 0 });
var done = getvar('stat_data.Events.鐘聲', { defaults: false });
if (invasion >= 50 && done === false) { _%>
鐘聲響起 <%= invasion %><%_ setvar('stat_data.Events.鐘聲', true); } _%>"#;
        let unsupported =
            r#"<%_ var roll = _.random(1, 2); _%><%_ if (roll === 1) { _%>隨機<%_ } _%>"#;
        let raw = json!({
            "data": {
                "name": "莉亞",
                "character_book": {
                    "entries": [
                        {"comment": "[Event] 鐘聲", "content": supported},
                        {"comment": "[Script] 隨機", "content": unsupported}
                    ]
                }
            }
        })
        .to_string();

        import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff").unwrap();
        let state = data::read_state(root.path(), &world_id).unwrap();
        assert_eq!(state.mechanism.triggers.len(), 1);
        assert_eq!(
            state.mechanism.triggers[0].flag.as_deref(),
            Some("Events.鐘聲")
        );
        assert_eq!(
            state.mechanism.rules["Events.鐘聲"].kind,
            data::FieldKind::ReadOnly
        );
        assert!(!state.mechanism.incremental);
        let log =
            fs::read_to_string(data::mechanism_log_path(root.path(), &world_id).unwrap()).unwrap();
        assert_eq!(log.matches("\"kind\":\"skipped\"").count(), 1);
    }

    /// ST 的 `${a|b}` 多選一寫法：同一條規則要展開成各自獨立的規則。
    #[test]
    fn wildcard_pipe_segment_expands_into_multiple_rules() {
        let root = TestRoot::new("mvu-expand");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let raw = json!({
            "data": {
                "name": "莉亞",
                "character_book": {
                    "entries": [{
                        "comment": "[mvu_update]规则",
                        "enabled": true,
                        "content": "规则:\n  Player:\n    Outfit.${上装|下装}:\n      type: string\n"
                    }]
                }
            }
        })
        .to_string();

        import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff").unwrap();
        let state = data::read_state(root.path(), &world_id).unwrap();
        assert_eq!(
            state.mechanism.rules["Player.Outfit.上装"].kind,
            data::FieldKind::Text
        );
        assert_eq!(
            state.mechanism.rules["Player.Outfit.下装"].kind,
            data::FieldKind::Text
        );
    }
}
