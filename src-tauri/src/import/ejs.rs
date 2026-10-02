//! 固定型 EJS 辨識器：只把可靜態還原的條件分支收成觸發表，從不執行腳本。

use crate::data::{Condition, Trigger, TriggerCase, TriggerMode};
use std::collections::BTreeMap;

#[derive(Clone)]
struct Variable {
    path: String,
    default: Option<f64>,
}

enum Token {
    Text(String),
    If(String),
    ElseIf(String),
    Else,
    Close,
}

enum Node {
    Text(String),
    If {
        branches: Vec<(String, Vec<Node>)>,
        otherwise: Option<Vec<Node>>,
    },
}

/// 嘗試把一條 SillyTavern EJS 條目變成純資料觸發表；任何不在白名單內的形狀都回 `None`。
pub fn parse_triggers(title: &str, content: &str) -> Option<Trigger> {
    if ["_.random", "for (", ".split(", "Math."]
        .iter()
        .any(|needle| content.contains(needle))
    {
        return None;
    }
    let variables = collect_variables(content)?;
    if variables.is_empty() {
        return None;
    }
    let tokens = tokenize(content, &variables)?;
    let (nodes, consumed) = parse_nodes(&tokens, 0)?;
    if consumed != tokens.len() {
        return None;
    }
    let mut guarded = Vec::new();
    let nodes = flatten_guards(nodes, &mut guarded)?;
    // 守衛拆掉了，但它擋的事情要留住：那個變數不存在時整段腳本不該送
    // （原生 ST 就是這樣），所以每個 case 都補一條「這條路徑得有數字」。
    let guard_conditions: Vec<Condition> = guarded
        .iter()
        .filter_map(|name| variables.get(name))
        .map(|variable| Condition::Range {
            path: variable.path.clone(),
            min: None,
            max: None,
            min_exclusive: false,
            max_exclusive: false,
            default: None,
        })
        .collect();
    let mut prefix = String::new();
    let mut root = None;
    for node in nodes {
        match node {
            Node::Text(text) if root.is_none() => prefix.push_str(&text),
            Node::Text(text) if text.trim().is_empty() => {}
            Node::If { .. } if root.is_none() => root = Some(node),
            _ => return None,
        }
    }
    let Node::If {
        branches,
        otherwise,
    } = root?
    else {
        return None;
    };
    let mut cases = Vec::new();
    for (condition, body) in branches {
        let mut when = guard_conditions.clone();
        when.extend(parse_condition(&condition, &variables)?);
        collect_branch_cases(&body, when, &variables, &mut cases)?;
    }
    if let Some(body) = otherwise {
        collect_branch_cases(&body, guard_conditions.clone(), &variables, &mut cases)?;
    }
    if cases.is_empty() || cases.iter().all(|case| case.text.trim().is_empty()) {
        return None;
    }
    let flags: Vec<String> = cases
        .iter()
        .flat_map(|case| case.when.iter())
        .filter_map(|condition| match condition {
            Condition::Flag {
                path,
                expect: false,
            } => Some(path.clone()),
            _ => None,
        })
        .collect();
    let flag = flags.first().cloned();
    if flags.iter().any(|path| Some(path) != flag.as_ref()) {
        return None;
    }
    let all_paths: Vec<String> = cases
        .iter()
        .flat_map(|case| case.when.iter())
        .map(condition_path)
        .map(str::to_owned)
        .collect();
    let id = trigger_id(title);
    Some(Trigger {
        id,
        title: title.to_owned(),
        mode: if flag.is_some() {
            TriggerMode::Once
        } else {
            TriggerMode::Range
        },
        cases,
        preamble: prefix.trim().to_owned(),
        scope: shared_scope(&all_paths.iter().map(String::as_str).collect::<Vec<_>>()),
        flag,
    })
}

fn collect_variables(content: &str) -> Option<BTreeMap<String, Variable>> {
    let mut variables = BTreeMap::new();
    let mut cursor = 0;
    while let Some(open) = content[cursor..].find("<%") {
        let open = cursor + open;
        let body_start = open
            + if content[open..].starts_with("<%_") {
                3
            } else {
                2
            };
        let close = content[body_start..].find("%>")? + body_start;
        let code = strip_comments(&content[body_start..close]);
        let mut at = 0;
        while let Some(found) = code[at..].find("var ") {
            let start = at + found + 4;
            let (name, after_name) = identifier_at(&code, start)?;
            let rest = code[after_name..].trim_start();
            if let Some(rest) = rest.strip_prefix('=') {
                let rest = rest.trim_start();
                if let Some(arguments) = rest.strip_prefix("getvar(") {
                    let (path, default) = parse_getvar(arguments)?;
                    variables.insert(name.to_owned(), Variable { path, default });
                } else if let Some(source) = number_alias_source(rest) {
                    let source = variables.get(source)?.clone();
                    variables.insert(name.to_owned(), source);
                }
            }
            at = after_name;
        }
        cursor = close + 2;
    }
    Some(variables)
}

fn parse_getvar(arguments: &str) -> Option<(String, Option<f64>)> {
    let arguments = arguments.trim_start();
    let quote = arguments.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let end = arguments[1..].find(quote)? + 1;
    let raw_path = &arguments[1..end];
    let path = raw_path.strip_prefix("stat_data.")?;
    if path.split('.').count() < 2 || path.split('.').any(str::is_empty) {
        return None;
    }
    let tail = arguments[end + 1..].trim_start();
    let default = tail.find("defaults:").and_then(|at| {
        let value = tail[at + "defaults:".len()..].trim_start();
        read_number(value)
    });
    Some((path.to_owned(), default))
}

fn number_alias_source(value: &str) -> Option<&str> {
    let value = value.strip_prefix("Number(")?;
    let end = value.find(')')?;
    if !value[end + 1..].trim_start().starts_with(';') {
        return None;
    }
    let source = value[..end].trim();
    is_identifier(source).then_some(source)
}

fn tokenize(content: &str, variables: &BTreeMap<String, Variable>) -> Option<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut cursor = 0;
    while let Some(relative_open) = content[cursor..].find("<%") {
        let open = cursor + relative_open;
        push_text(&mut tokens, content[cursor..open].to_owned());
        if content[open..].starts_with("<%=") {
            let body_start = open + 3;
            let close = content[body_start..].find("%>")? + body_start;
            let expression = content[body_start..close].trim();
            let variable = variables.get(expression)?;
            push_text(&mut tokens, format!("{{{{state:{}}}}}", variable.path));
            cursor = close + 2;
            continue;
        }
        if !content[open..].starts_with("<%_") && !content[open..].starts_with("<%") {
            return None;
        }
        let body_start = open
            + if content[open..].starts_with("<%_") {
                3
            } else {
                2
            };
        let close = content[body_start..].find("%>")? + body_start;
        let code = strip_comments(&content[body_start..close]);
        if let Some(token) = control_token(&code)? {
            tokens.push(token);
        }
        cursor = close + 2;
    }
    push_text(&mut tokens, content[cursor..].to_owned());
    Some(tokens)
}

fn push_text(tokens: &mut Vec<Token>, text: String) {
    if text.is_empty() {
        return;
    }
    match tokens.last_mut() {
        Some(Token::Text(previous)) => previous.push_str(&text),
        _ => tokens.push(Token::Text(text)),
    }
}

fn control_token(code: &str) -> Option<Option<Token>> {
    let code = code.trim().trim_matches('_').trim();
    if code.is_empty() {
        return Some(None);
    }
    if let Some(position) = code.find("else if") {
        let condition = if_condition(&code[position + "else ".len()..])?;
        return Some(Some(Token::ElseIf(condition)));
    }
    if let Some(position) = code.find("else") {
        let tail = code[position + "else".len()..].trim_start();
        if tail.starts_with('{') {
            return Some(Some(Token::Else));
        }
    }
    let mut at = 0;
    while let Some(position) = code[at..].find("if") {
        let position = at + position;
        let before = code[..position].chars().next_back();
        let after = code[position + 2..].chars().next();
        if !before.is_some_and(is_identifier_char) && !after.is_some_and(is_identifier_char) {
            if let Some(condition) = if_condition(&code[position..]) {
                return Some(Some(Token::If(condition)));
            }
        }
        at = position + 2;
    }
    if code.starts_with('}') || code.ends_with('}') {
        return Some(Some(Token::Close));
    }
    Some(None)
}

fn if_condition(code: &str) -> Option<String> {
    let after_if = code.strip_prefix("if")?.trim_start();
    let inside = parenthesized(after_if)?;
    let after = after_if[inside.1..].trim_start();
    after.starts_with('{').then(|| inside.0.to_owned())
}

fn parse_nodes(tokens: &[Token], mut index: usize) -> Option<(Vec<Node>, usize)> {
    let mut nodes = Vec::new();
    while index < tokens.len() {
        match &tokens[index] {
            Token::Text(text) => {
                nodes.push(Node::Text(text.clone()));
                index += 1;
            }
            Token::If(condition) => {
                let (first_body, mut next) = parse_nodes(tokens, index + 1)?;
                let mut branches = vec![(condition.clone(), first_body)];
                let mut otherwise = None;
                loop {
                    match tokens.get(next) {
                        Some(Token::ElseIf(condition)) => {
                            let (body, after) = parse_nodes(tokens, next + 1)?;
                            branches.push((condition.clone(), body));
                            next = after;
                        }
                        Some(Token::Else) => {
                            let (body, after) = parse_nodes(tokens, next + 1)?;
                            otherwise = Some(body);
                            next = after;
                            break;
                        }
                        _ => break,
                    }
                }
                if !matches!(tokens.get(next), Some(Token::Close)) {
                    return None;
                }
                nodes.push(Node::If {
                    branches,
                    otherwise,
                });
                index = next + 1;
            }
            Token::ElseIf(_) | Token::Else | Token::Close => return Some((nodes, index)),
        }
    }
    Some((nodes, index))
}

/// 剝掉「變數存在嗎」那層守衛，把被守的變數名收進 `guarded`——條件本身不能丟，
/// 交給呼叫端補回每個 case（不然變數不存在時整段腳本照樣被注入）。
fn flatten_guards(nodes: Vec<Node>, guarded: &mut Vec<String>) -> Option<Vec<Node>> {
    let mut flattened = Vec::new();
    for node in nodes {
        match node {
            Node::Text(text) => flattened.push(Node::Text(text)),
            Node::If {
                branches,
                otherwise,
            } => {
                if branches.len() == 1
                    && otherwise.is_none()
                    && existence_guard_target(&branches[0].0).is_some()
                {
                    if let Some(name) = existence_guard_target(&branches[0].0) {
                        guarded.push(name);
                    }
                    flattened.extend(flatten_guards(branches.into_iter().next()?.1, guarded)?);
                } else {
                    let mut mapped_branches = Vec::with_capacity(branches.len());
                    for (condition, body) in branches {
                        mapped_branches.push((condition, flatten_guards(body, guarded)?));
                    }
                    let otherwise = match otherwise {
                        Some(body) => Some(flatten_guards(body, guarded)?),
                        None => None,
                    };
                    flattened.push(Node::If {
                        branches: mapped_branches,
                        otherwise,
                    });
                }
            }
        }
    }
    Some(flattened)
}

/// `typeof X !== 'undefined' && X !== null` 這種存在性守衛，回傳被守的變數名。
fn existence_guard_target(condition: &str) -> Option<String> {
    let parts = split_top_level(condition, "&&");
    if parts.len() != 2 {
        return None;
    }
    let name = parts[0]
        .trim()
        .strip_prefix("typeof")
        .map(str::trim)
        .and_then(|value| value.split_once("!=="))
        .map(|(name, value)| (name.trim(), value.trim()))
        .filter(|(_, value)| matches!(*value, "'undefined'" | "\"undefined\""))
        .map(|(name, _)| name)?;
    parts[1]
        .trim()
        .strip_suffix("!== null")
        .map(str::trim)
        .filter(|other| *other == name)
        .map(|name| name.to_owned())
}

fn collect_branch_cases(
    body: &[Node],
    inherited: Vec<Condition>,
    variables: &BTreeMap<String, Variable>,
    cases: &mut Vec<TriggerCase>,
) -> Option<()> {
    let nested: Vec<&Node> = body
        .iter()
        .filter(|node| matches!(node, Node::If { .. }))
        .collect();
    if nested.is_empty() {
        let text: String = body
            .iter()
            .filter_map(|node| match node {
                Node::Text(text) => Some(text.as_str()),
                Node::If { .. } => None,
            })
            .collect();
        if text.trim().is_empty() {
            return None;
        }
        cases.push(TriggerCase {
            when: inherited,
            text: text.trim().to_owned(),
        });
        return Some(());
    }
    if nested.len() != 1
        || body
            .iter()
            .any(|node| matches!(node, Node::Text(text) if !text.trim().is_empty()))
    {
        return None;
    }
    let Node::If {
        branches,
        otherwise,
    } = nested[0]
    else {
        return None;
    };
    for (condition, branch) in branches {
        let mut when = inherited.clone();
        when.extend(parse_condition(condition, variables)?);
        collect_branch_cases(branch, when, variables, cases)?;
    }
    if let Some(branch) = otherwise {
        collect_branch_cases(branch, inherited, variables, cases)?;
    }
    Some(())
}

fn parse_condition(
    condition: &str,
    variables: &BTreeMap<String, Variable>,
) -> Option<Vec<Condition>> {
    let condition = trim_outer_parentheses(condition.trim());
    let parts = split_top_level(condition, "&&");
    if parts.len() > 1 {
        return parts
            .into_iter()
            .map(|part| parse_condition(part, variables))
            .collect::<Option<Vec<_>>>()
            .map(|groups| groups.into_iter().flatten().collect());
    }
    parse_contains(condition, variables)
        .or_else(|| parse_range(condition, variables))
        .or_else(|| parse_flag(condition, variables))
        .map(|condition| vec![condition])
}

fn parse_contains(condition: &str, variables: &BTreeMap<String, Variable>) -> Option<Condition> {
    let parts = split_top_level(trim_outer_parentheses(condition), "||");
    let mut path = None;
    let mut any = Vec::new();
    for part in parts {
        let part = trim_outer_parentheses(part.trim());
        let (name, tail) = part.split_once(".includes(")?;
        let name = name.trim();
        let variable = variables.get(name)?;
        let inside = tail.strip_suffix(')')?.trim();
        let quote = inside.chars().next()?;
        if !matches!(quote, '\'' | '"') || !inside.ends_with(quote) {
            return None;
        }
        let value = inside[1..inside.len() - quote.len_utf8()].to_owned();
        if path
            .replace(variable.path.clone())
            .is_some_and(|old| old != variable.path)
        {
            return None;
        }
        any.push(value);
    }
    if any.is_empty() {
        return None;
    }
    Some(Condition::Contains { path: path?, any })
}

fn parse_range(condition: &str, variables: &BTreeMap<String, Variable>) -> Option<Condition> {
    let condition = trim_outer_parentheses(condition);
    for operator in [">=", "<=", "===", "==", ">", "<"] {
        let Some((name, value)) = condition.split_once(operator) else {
            continue;
        };
        let variable = variables.get(name.trim())?;
        let value = value.trim().parse::<f64>().ok()?;
        let (min, max, min_exclusive, max_exclusive) = match operator {
            ">=" => (Some(value), None, false, false),
            ">" => (Some(value), None, true, false),
            "<=" => (None, Some(value), false, false),
            "<" => (None, Some(value), false, true),
            "==" | "===" => (Some(value), Some(value), false, false),
            _ => return None,
        };
        return Some(Condition::Range {
            path: variable.path.clone(),
            min,
            max,
            min_exclusive,
            max_exclusive,
            default: variable.default,
        });
    }
    None
}

fn parse_flag(condition: &str, variables: &BTreeMap<String, Variable>) -> Option<Condition> {
    let condition = trim_outer_parentheses(condition).trim();
    if let Some(name) = condition.strip_prefix('!') {
        let variable = variables.get(name.trim())?;
        return Some(Condition::Flag {
            path: variable.path.clone(),
            expect: false,
        });
    }
    for (suffix, expect) in [("=== false", false), ("=== true", true)] {
        if let Some(name) = condition.strip_suffix(suffix) {
            let variable = variables.get(name.trim())?;
            return Some(Condition::Flag {
                path: variable.path.clone(),
                expect,
            });
        }
    }
    None
}

fn condition_path(condition: &Condition) -> &str {
    match condition {
        Condition::Range { path, .. }
        | Condition::Contains { path, .. }
        | Condition::Flag { path, .. } => path,
    }
}

fn shared_scope(paths: &[&str]) -> Vec<String> {
    let parents: Vec<Vec<&str>> = paths
        .iter()
        .map(|path| {
            let mut parts: Vec<&str> = path.split('.').collect();
            parts.pop();
            parts
        })
        .collect();
    let Some(first) = parents.first() else {
        return Vec::new();
    };
    first
        .iter()
        .enumerate()
        .take_while(|(index, segment)| parents.iter().all(|path| path.get(*index) == Some(segment)))
        .map(|(_, segment)| (*segment).to_owned())
        .collect()
}

fn trigger_id(title: &str) -> String {
    let original = title.trim();
    let mut rest = original;
    while let Some(after_open) = rest.strip_prefix('[') {
        let Some((label, after)) = after_open.split_once(']') else {
            break;
        };
        if !matches!(
            label.trim().to_ascii_lowercase().as_str(),
            "script" | "event" | "environment"
        ) {
            break;
        }
        rest = after.trim_start();
    }
    if rest.is_empty() {
        original.to_owned()
    } else {
        rest.to_owned()
    }
}

fn strip_comments(code: &str) -> String {
    let mut result = String::new();
    let mut chars = code.chars().peekable();
    let mut quote = None;
    while let Some(ch) = chars.next() {
        if let Some(active_quote) = quote {
            result.push(ch);
            if ch == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(ch, '\'' | '"') {
            quote = Some(ch);
            result.push(ch);
        } else if ch == '/' && chars.peek() == Some(&'/') {
            chars.next();
            while chars.next().is_some_and(|next| next != '\n') {}
            result.push('\n');
        } else if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut previous = '\0';
            for next in chars.by_ref() {
                if previous == '*' && next == '/' {
                    break;
                }
                previous = next;
            }
        } else {
            result.push(ch);
        }
    }
    result
}

fn identifier_at(text: &str, start: usize) -> Option<(&str, usize)> {
    let tail = &text[start..];
    let end = tail
        .find(|ch: char| !is_identifier_char(ch))
        .unwrap_or(tail.len());
    let name = &tail[..end];
    is_identifier(name).then_some((name, start + end))
}

fn is_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(is_identifier_char)
        && !value.starts_with(char::is_numeric)
}

fn is_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn parenthesized(text: &str) -> Option<(&str, usize)> {
    let text = text.strip_prefix('(')?;
    let mut depth = 1usize;
    let mut quote = None;
    for (index, ch) in text.char_indices() {
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(ch, '\'' | '"') {
            quote = Some(ch);
        } else if ch == '(' {
            depth += 1;
        } else if ch == ')' {
            depth -= 1;
            if depth == 0 {
                return Some((&text[..index], index + 2));
            }
        }
    }
    None
}

fn split_top_level<'a>(text: &'a str, separator: &str) -> Vec<&'a str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut quote = None;
    let mut index = 0;
    while index < text.len() {
        let ch = text[index..]
            .chars()
            .next()
            .expect("索引永遠落在 UTF-8 邊界");
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            }
        } else if matches!(ch, '\'' | '"') {
            quote = Some(ch);
        } else if ch == '(' {
            depth += 1;
        } else if ch == ')' {
            depth = depth.saturating_sub(1);
        } else if depth == 0 && text[index..].starts_with(separator) {
            parts.push(&text[start..index]);
            index += separator.len();
            start = index;
            continue;
        }
        index += ch.len_utf8();
    }
    parts.push(&text[start..]);
    parts
}

fn trim_outer_parentheses(mut text: &str) -> &str {
    loop {
        let trimmed = text.trim();
        let Some((inside, consumed)) = parenthesized(trimmed) else {
            return trimmed;
        };
        if consumed != trimmed.len() {
            return trimmed;
        }
        text = inside;
    }
}

fn read_number(value: &str) -> Option<f64> {
    let end = value
        .find(|ch: char| !(ch.is_ascii_digit() || matches!(ch, '.' | '-' | '+')))
        .unwrap_or(value.len());
    value[..end].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flattens_nested_relation_cases_in_document_order() {
        let content = r#"
<%_ if (typeof affection === 'undefined') var affection = getvar('stat_data.Heroes.亞瑟.Affection', { defaults: 0 });
if (typeof desire === 'undefined') var desire = getvar('stat_data.Heroes.亞瑟.Desire', { defaults: 0 }); _%>
前言 <%= affection %>
<%_ if (affection >= 60 && desire >= 20) { _%>
  <%_ if (affection >= 160 && desire >= 80) { _%>誓約<%_ } else { _%>萌芽<%_ } _%>
<%_ } else { _%>觀察<%_ } _%>"#;
        let trigger = parse_triggers("[Script] 亞瑟", content).unwrap();
        assert_eq!(trigger.id, "亞瑟");
        assert_eq!(trigger.preamble, "前言 {{state:Heroes.亞瑟.Affection}}");
        assert_eq!(
            trigger
                .cases
                .iter()
                .map(|case| case.text.as_str())
                .collect::<Vec<_>>(),
            vec!["誓約", "萌芽", "觀察"]
        );
        assert_eq!(trigger.cases[0].when.len(), 4);
        assert_eq!(trigger.cases[1].when.len(), 2);
        assert!(trigger.cases[2].when.is_empty());
        assert_eq!(trigger.scope, vec!["Heroes", "亞瑟"]);
    }

    /// 「變數存在嗎」的守衛剝掉後條件不能跟著蒸發：每個 case 都要補一條存在性檢查，
    /// 不然那個欄位還沒出現在樹上時，兜底分支會無條件把一段空值文本注進去。
    #[test]
    fn existence_guard_becomes_a_condition_on_every_case() {
        let content = r#"<%_ var raw = getvar('stat_data.World.AshCorruption');
if (typeof raw !== 'undefined' && raw !== null) {
  var current = Number(raw); _%>
濃度 <%= current %>
<%_ if (current >= 50) { _%>高<%_ } else { _%>低<%_ } _%>
<%_ } _%>"#;
        let trigger = parse_triggers("[Environment] 灰烬", content).unwrap();
        let existence = Condition::Range {
            path: "World.AshCorruption".to_owned(),
            min: None,
            max: None,
            min_exclusive: false,
            max_exclusive: false,
            default: None,
        };
        assert_eq!(trigger.cases.len(), 2);
        assert_eq!(trigger.cases[0].when[0], existence);
        assert_eq!(trigger.cases[1].when, vec![existence]);
    }

    #[test]
    fn parses_range_and_replaces_state_interpolation() {
        let content = r#"<%_ var invasion = getvar('stat_data.World.Invasion', { defaults: 0 }); _%>
基準 <%= invasion %>
<%_ if (invasion >= 50) { _%>高<%= invasion %><%_ } else { _%>低<%_ } _%>"#;
        let trigger = parse_triggers("[Environment] 侵略", content).unwrap();
        assert_eq!(trigger.mode, TriggerMode::Range);
        assert_eq!(trigger.cases[0].text, "高{{state:World.Invasion}}");
        assert_eq!(
            trigger.cases[0].when,
            vec![Condition::Range {
                path: "World.Invasion".to_owned(),
                min: Some(50.0),
                max: None,
                min_exclusive: false,
                max_exclusive: false,
                default: Some(0.0),
            }]
        );
    }

    #[test]
    fn parses_once_event_with_range_contains_and_flag() {
        let content = r#"<%_
var invasion = getvar('stat_data.World.Invasion', { defaults: 0 });
var location = getvar('stat_data.World.Location', { defaults: '' });
var done = getvar('stat_data.Events.事件', { defaults: false });
if (invasion > 50 && (location.includes('甲') || location.includes('乙')) && done === false) { _%>
事件 <%= invasion %><%_ } _%>"#;
        let trigger = parse_triggers("[Event] 事件", content).unwrap();
        assert_eq!(trigger.mode, TriggerMode::Once);
        assert_eq!(trigger.flag.as_deref(), Some("Events.事件"));
        assert_eq!(trigger.cases[0].when.len(), 3);
        assert!(matches!(
            trigger.cases[0].when[1],
            Condition::Contains { .. }
        ));
        assert_eq!(trigger.scope, Vec::<String>::new());
    }

    #[test]
    fn skips_six_unsupported_shapes() {
        let fixtures = [
            "<%_ var value = getvar('stat_data.World.Value'); _%><%_ if (value >= 1) { _%><%= 5 - value %><%_ } _%>",
            "<%_ var value = 0; _%><%_ if (value >= 1) { _%>文字<%_ } _%>",
            "<%_ var value = getvar('stat_data.World.Value'); value.split('/'); _%><%_ if (value >= 1) { _%>文字<%_ } _%>",
            "<%_ var value = _.random(1, 2); _%><%_ if (value === 1) { _%>文字<%_ } _%>",
            "<%_ var world = getvar('stat_data.World', { defaults: {} }); _%><%_ if (world.Value >= 1) { _%>文字<%_ } _%>",
            "<%_ var value = getvar('stat_data.World.Value'); for (var key in value) {} _%><%_ if (value >= 1) { _%>文字<%_ } _%>",
        ];
        for content in fixtures {
            assert!(parse_triggers("跳過", content).is_none(), "{content}");
        }
    }
}
