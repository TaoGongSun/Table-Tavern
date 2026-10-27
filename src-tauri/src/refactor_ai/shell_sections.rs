//! 骨架的集合區段（takeover-list-structured）：`{{#集合}}…{{/集合}}` 是項目模板，區段內 `{{欄位}}` 相對項目、
//! `{{.}}` 是純量項目、`{{@key}}` 是具名項目的名字、`{{@n}}` 是從 1 起的序號。前端填值見
//! src/features/refactor/refactor-shell-sections.ts，這裡管重構產物的拆解與驗證，套用端（refactor/）也用它找集合。

use crate::data::{FieldKind, FieldRule};
use std::collections::BTreeMap;

use super::result_parse::{classify_placeholder, PlaceholderKind};

/// 一個區段：集合路徑與區段內的佔位符（trim 過，依出現順序）。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Section {
    pub path: String,
    pub tokens: Vec<String>,
}

/// 骨架拆成區段與區段外的文字（區段整段拿掉，含標記）。
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Sections {
    pub sections: Vec<Section>,
    pub outside: String,
}

/// 佔位符只認 `{{...}}` 內不含花括號與換行的形式（與前端填值相同）。回傳（起點、終點、內容）。
fn placeholders(text: &str) -> Vec<(usize, usize, &str)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(open) = text[from..].find("{{").map(|at| at + from) {
        let Some(close) = text[open + 2..].find("}}").map(|at| at + open + 2) else {
            break;
        };
        let token = &text[open + 2..close];
        if token.contains(['{', '}', '\n']) {
            from = open + 2;
            continue;
        }
        out.push((open, close + 2, token));
        from = close + 2;
    }
    out
}

/// 拆區段：成對、不巢狀；開頭 `{{#路徑}}`、收尾 `{{/路徑}}`（酒館註解 `{{// …}}` 不算收尾）。
pub(crate) fn split_sections(shell: &str) -> Result<Sections, String> {
    let mut result = Sections::default();
    let mut cursor = 0;
    let mut open: Option<Section> = None;
    for (start, end, raw) in placeholders(shell) {
        let token = raw.trim();
        if let Some(path) = token.strip_prefix('#') {
            let path = path.trim();
            if let Some(current) = &open {
                return Err(format!(
                    "集合 {} 的區段裡又開了區段 {path}（區段不能巢狀）",
                    current.path
                ));
            }
            if path.is_empty() {
                return Err("區段開頭 {{#}} 沒寫集合路徑".to_owned());
            }
            result.outside.push_str(&shell[cursor..start]);
            open = Some(Section {
                path: path.to_owned(),
                tokens: Vec::new(),
            });
        } else if let Some(path) = token.strip_prefix('/').filter(|_| !token.starts_with("//")) {
            let path = path.trim();
            match open.take() {
                Some(current) if current.path == path => {
                    result.sections.push(current);
                    cursor = end;
                }
                Some(current) => {
                    return Err(format!(
                        "集合 {} 的區段用 {{{{/{path}}}}} 收尾，對不上",
                        current.path
                    ))
                }
                None => return Err(format!("{{{{/{path}}}}} 前面沒有對應的 {{{{#{path}}}}}")),
            }
        } else if let Some(current) = &mut open {
            current.tokens.push(token.to_owned());
        }
    }
    if let Some(current) = open {
        return Err(format!(
            "集合 {} 的區段沒有收尾 {{{{/{}}}}}",
            current.path, current.path
        ));
    }
    result.outside.push_str(&shell[cursor..]);
    Ok(result)
}

/// STATE 裡點分路徑的值（只走物件）。
pub(crate) fn value_at<'a>(
    state: &'a serde_json::Value,
    path: &str,
) -> Option<&'a serde_json::Value> {
    path.split('.')
        .try_fold(state, |node, key| node.as_object()?.get(key))
}

fn scalar(value: &serde_json::Value) -> bool {
    !value.is_object() && !value.is_array()
}

/// 項目名綁不成點分路徑與殼的佔位符：空的、`-`、底線開頭、含 `.`、`{`、`}`、換行（套用端同一套）。
fn bad_item_key(key: &str) -> bool {
    key.is_empty() || key == "-" || key.starts_with('_') || key.contains(['.', '{', '}', '\n'])
}

/// 一個區段的集合合不合格：STATE 是物件（具名）或陣列（序列）、有 `kind: list` 規則；具名項目一律是物件、
/// 序列項目全是純量或全是物件（空的看模板有沒有 `{{.}}`）；區段內每個欄位有 `集合.*.欄位` 規則且每個項目
/// 都有、純量項目有 `集合.*` 規則；`{{@key}}` 只在具名集合。不合格回原因（指名集合）。
pub(crate) fn check_section(
    section: &Section,
    state: &serde_json::Value,
    rules: &BTreeMap<String, FieldRule>,
) -> Result<(), String> {
    let path = &section.path;
    let fail = |reason: String| Err(format!("集合 {path}：{reason}"));
    let items: Vec<(String, &serde_json::Value)> = match value_at(state, path) {
        Some(serde_json::Value::Object(map)) => map
            .iter()
            .map(|(key, value)| (key.clone(), value))
            .collect(),
        Some(serde_json::Value::Array(list)) => list
            .iter()
            .enumerate()
            .map(|(index, value)| (index.to_string(), value))
            .collect(),
        _ => return fail("STATE 裡要是以名字為鍵的物件（具名）或陣列（序列）".to_owned()),
    };
    let keyed = value_at(state, path).is_some_and(serde_json::Value::is_object);
    if rules.get(path).map(|rule| rule.kind) != Some(FieldKind::List) {
        return fail("RULES 要有這個路徑的 kind: list 規則".to_owned());
    }
    if let Some((key, _)) = items.iter().find(|(key, _)| keyed && bad_item_key(key)) {
        return fail(format!(
            "項目名「{key}」不能是空的、-、底線開頭，或含 .、{{、}}、換行"
        ));
    }
    let scalar_items = if items.is_empty() {
        section.tokens.iter().any(|token| token == ".")
    } else {
        items.iter().all(|(_, value)| scalar(value))
    };
    if !scalar_items && !items.iter().all(|(_, value)| value.is_object()) {
        return fail("項目要全是物件或全是單一值，不能混用".to_owned());
    }
    if keyed && scalar_items && !items.is_empty() {
        return fail("具名集合的項目要是物件（名字當鍵，欄位放在物件裡）".to_owned());
    }
    if scalar_items && !rules.contains_key(&format!("{path}.*")) {
        return fail(format!("單一值項目要有 RULES {path}.*"));
    }
    for token in &section.tokens {
        match token.as_str() {
            "@n" => {}
            "@key" if keyed => {}
            "@key" => return fail("{{@key}} 只能用在具名集合（序列沒有名字）".to_owned()),
            "." if scalar_items => {}
            "." => return fail("物件項目不能用 {{.}}，請寫欄位 {{欄位}}".to_owned()),
            field => {
                let rule = format!("{path}.*.{field}");
                // 不是欄位的正文槽與酒館巨集照原樣保留
                let probe = serde_json::Value::Object(Default::default());
                if !rules.contains_key(&rule)
                    && classify_placeholder(field, &probe) != PlaceholderKind::Path
                {
                    continue;
                }
                if scalar_items {
                    return fail(format!(
                        "單一值項目沒有欄位 {{{{{field}}}}}，請用 {{{{.}}}}"
                    ));
                }
                if !rules.contains_key(&rule) {
                    return fail(format!("欄位 {field} 缺 RULES {path}.*.{field}"));
                }
                if let Some((key, _)) = items
                    .iter()
                    .find(|(_, item)| !item.get(field).is_some_and(scalar))
                {
                    return fail(format!("項目 {key} 缺欄位 {field}（或不是單一值）"));
                }
            }
        }
    }
    Ok(())
}

/// 舊式字串清單：`kind: list` 欄位的佔位符獨佔一行（不在行內集合 `[…]` 裡），是把一筆一筆重複寫法的清單
/// 做成了一整串字串，回那個路徑。自由文字的多行欄不是 `kind: list`，不受影響。
pub(crate) fn whole_line_list(
    outside: &str,
    rules: &BTreeMap<String, FieldRule>,
) -> Option<String> {
    outside.lines().find_map(|line| {
        let found = placeholders(line);
        let [(start, end, token)] = found.as_slice() else {
            return None;
        };
        let alone = line[..*start].trim().is_empty() && line[*end..].trim().is_empty();
        let path = token.trim();
        (alone
            && rules
                .get(path)
                .is_some_and(|rule| rule.kind == FieldKind::List))
        .then(|| path.to_owned())
    })
}

/// 區段外的佔位符（trim 過）。
pub(crate) fn outside_tokens(outside: &str) -> Vec<String> {
    placeholders(outside)
        .into_iter()
        .map(|(_, _, token)| token.trim().to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::FieldRule;
    use serde_json::json;

    fn rules(paths: &[(&str, FieldKind)]) -> BTreeMap<String, FieldRule> {
        paths
            .iter()
            .map(|(path, kind)| ((*path).to_owned(), FieldRule::for_kind(*kind)))
            .collect()
    }

    const SHELL: &str = "<S>\n人物列表:\n{{#在場人物}}\n  - 名字: \"{{@key}}\"\n    身份: \"{{身份}}\"\n{{/在場人物}}\n选项:\n{{#行動選項}}\n  - \"{{@n}}. {{.}}\"\n{{/行動選項}}\n地点: {{地點}} {{// 註解}}\n</S>";

    #[test]
    fn splits_sections_and_keeps_the_outside_text() {
        let split = split_sections(SHELL).unwrap();
        assert_eq!(
            split.sections,
            vec![
                Section {
                    path: "在場人物".to_owned(),
                    tokens: vec!["@key".to_owned(), "身份".to_owned()]
                },
                Section {
                    path: "行動選項".to_owned(),
                    tokens: vec!["@n".to_owned(), ".".to_owned()]
                },
            ]
        );
        assert_eq!(
            split.outside,
            "<S>\n人物列表:\n\n选项:\n\n地点: {{地點}} {{// 註解}}\n</S>"
        );
        assert_eq!(outside_tokens(&split.outside), vec!["地點", "// 註解"]);
    }

    #[test]
    fn unbalanced_or_nested_sections_name_the_collection() {
        let nested = split_sections("{{#甲}}{{#乙}}{{/乙}}{{/甲}}").unwrap_err();
        assert!(
            nested.contains("集合 甲") && nested.contains("巢狀"),
            "{nested}"
        );
        assert!(split_sections("{{#甲}}x").unwrap_err().contains("集合 甲"));
        assert!(split_sections("{{#甲}}x{{/乙}}")
            .unwrap_err()
            .contains("對不上"));
        assert!(split_sections("x{{/乙}}").unwrap_err().contains("{{#乙}}"));
    }

    fn north_hall_rules() -> BTreeMap<String, FieldRule> {
        rules(&[
            ("在場人物", FieldKind::List),
            ("在場人物.*.身份", FieldKind::Text),
            ("行動選項", FieldKind::List),
            ("行動選項.*", FieldKind::Text),
            ("地點", FieldKind::Text),
        ])
    }

    fn check_all(
        state: serde_json::Value,
        rules: &BTreeMap<String, FieldRule>,
    ) -> Result<(), String> {
        let split = split_sections(SHELL).unwrap();
        split
            .sections
            .iter()
            .try_for_each(|section| check_section(section, &state, rules))
    }

    #[test]
    fn north_hall_collections_pass_including_empty_ones() {
        let rules = north_hall_rules();
        let state = json!({
            "在場人物": { "林远": { "身份": "驿丞" }, "贺铮": { "身份": "驿长" } },
            "行動選項": ["稳妥", "调查"],
            "地點": "前院",
        });
        assert_eq!(check_all(state, &rules), Ok(()));
        let empty = json!({ "在場人物": {}, "行動選項": [], "地點": "前院" });
        assert_eq!(check_all(empty, &rules), Ok(()));
    }

    #[test]
    fn each_failure_names_the_collection_and_the_reason() {
        let rules = north_hall_rules();
        let cases = [
            (
                json!({ "在場人物": "    - 人物: …", "行動選項": [] }),
                "集合 在場人物：STATE 裡要是",
            ),
            (
                json!({ "在場人物": { "林远": { "职务": "x" } }, "行動選項": [] }),
                "項目 林远 缺欄位 身份",
            ),
            (
                json!({ "在場人物": { "林远": "驿丞" }, "行動選項": [] }),
                "集合 在場人物：具名集合的項目要是物件",
            ),
            (
                json!({ "在場人物": { "a.b": { "身份": "x" } }, "行動選項": [] }),
                "項目名「a.b」",
            ),
            (
                json!({ "在場人物": {}, "行動選項": ["a", { "b": 1 }] }),
                "集合 行動選項：項目要全是物件或全是單一值",
            ),
        ];
        for (state, expected) in cases {
            let error = check_all(state, &rules).unwrap_err();
            assert!(error.contains(expected), "{error}");
        }
        let mut missing = north_hall_rules();
        missing.remove("在場人物.*.身份");
        let error = check_all(json!({ "在場人物": {}, "行動選項": [] }), &missing).unwrap_err();
        assert!(
            error.contains("欄位 身份 缺 RULES 在場人物.*.身份"),
            "{error}"
        );
        missing.remove("行動選項.*");
        missing.insert(
            "在場人物.*.身份".to_owned(),
            FieldRule::for_kind(FieldKind::Text),
        );
        let error = check_all(json!({ "在場人物": {}, "行動選項": [] }), &missing).unwrap_err();
        assert!(error.contains("單一值項目要有 RULES 行動選項.*"), "{error}");
    }

    #[test]
    fn key_and_dot_only_where_they_mean_something() {
        let rules = north_hall_rules();
        let state = json!({ "行動選項": ["a"] });
        let error = check_section(
            &Section {
                path: "行動選項".to_owned(),
                tokens: vec!["@key".to_owned()],
            },
            &state,
            &rules,
        )
        .unwrap_err();
        assert!(error.contains("只能用在具名集合"), "{error}");
    }

    #[test]
    fn a_list_placeholder_alone_on_its_line_is_the_old_string_form() {
        let rules = rules(&[
            ("在場人物", FieldKind::List),
            ("技能", FieldKind::List),
            ("簡介", FieldKind::Text),
        ]);
        assert_eq!(
            whole_line_list("人物列表:\n    {{在場人物}}\n", &rules),
            Some("在場人物".to_owned())
        );
        // 行內集合片段與自由文字的多行欄不算
        assert_eq!(
            whole_line_list("技能: [{{技能}}]\n{{簡介}}\n", &rules),
            None
        );
    }
}
