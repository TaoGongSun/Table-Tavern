//! 退場桌開場補齊的附帶資料（statusbar-only-fields 第 11 版）：欄位註記與時間格子。只在套用時從淘汰的狀態欄原文與
//! 開場樹算一次，之後遊戲中只讀；解析不了的部分略過、記 warn，不影響欄位追蹤。

use super::apply_mechanism::explicit_number;
use crate::data::{self, StateNode, TimePart, WorldState};
use crate::mechanism::prefix::split_prefix;
use crate::mechanism::time_sync::time_parts;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

/// 會跟著圍欄 time 同步的時間格子欄名（完整欄名，比對前折成簡體字形）。倒數、期限、歷史日期、排程類不在這裡。
const TIME_CELL_NAMES: [&str; 7] = [
    "时间",
    "日期",
    "时辰",
    "日期和时间",
    "日期时间",
    "当前时间",
    "当前日期",
];

/// 註記最多幾個字。
const NOTE_LIMIT: usize = 40;

/// 時間格子：集合以外的葉子、欄名在白名單、開場值不是數字、拆得出時間成分。`leaves` 是剝過前綴的開場值。
pub(super) fn time_cells(leaves: &[(String, String)]) -> BTreeMap<String, BTreeSet<TimePart>> {
    leaves
        .iter()
        .filter(|(path, value)| {
            let name = path.rsplit('.').next().unwrap_or(path);
            let folded = data::fold_glyphs(name);
            TIME_CELL_NAMES.contains(&folded.as_ref()) && explicit_number(value).is_none()
        })
        .filter_map(|(path, value)| {
            let parts = time_parts(value);
            (!parts.is_empty()).then(|| (path.clone(), parts))
        })
        .collect()
}

fn line_regex() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r#"^(\s*)(-\s+)?([^\s:："'{}$-][^:：]*?)\s*[:：]\s*(.*)$"#)
            .expect("template line regex")
    })
}

fn placeholder_regex() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"\$\{([^}]*)\}").expect("placeholder regex"))
}

/// `${…}` 裡的說明清成一行「追蹤什麼」：去前綴與「按照格式输出」，「示例／例如／比如／例：」之後整段刪掉。
fn clean_note(raw: &str) -> Option<String> {
    static CUT: OnceLock<regex::Regex> = OnceLock::new();
    let cut = CUT.get_or_init(|| regex::Regex::new(r"示例|例如|比如|例[:：]").expect("cut regex"));
    let mut text = raw.trim().to_owned();
    if let Some(found) = cut.find(&text) {
        text.truncate(found.start());
    }
    let text = text.replace("按照格式输出", "").replace("按照格式輸出", "");
    let text = text.trim().to_owned();
    let text = match split_prefix(&text) {
        Some((_, rest)) => rest.to_owned(),
        None => text,
    };
    let text = text
        .trim_matches(|ch: char| ch.is_whitespace() || ",，、:：;；。.".contains(ch))
        .to_owned();
    if text.is_empty() {
        return None;
    }
    Some(text.chars().take(NOTE_LIMIT).collect())
}

/// 模板範圍：原文裡最長的一段 `<Status_block>…</Status_block>` 這類成對標籤內文；沒有就整段。
fn template_body(text: &str) -> &str {
    static TAG: OnceLock<regex::Regex> = OnceLock::new();
    let tag = TAG.get_or_init(|| regex::Regex::new(r"<([A-Za-z_][\w-]*)>").expect("tag regex"));
    let mut best: Option<&str> = None;
    for open in tag.captures_iter(text) {
        let (Some(whole), Some(name)) = (open.get(0), open.get(1)) else {
            continue;
        };
        let close = format!("</{}>", name.as_str());
        if let Some(end) = text[whole.end()..].find(&close) {
            let body = &text[whole.end()..whole.end() + end];
            if best.is_none_or(|current| body.len() > current.len()) {
                best = Some(body);
            }
        }
    }
    best.filter(|body| body.contains("${")).unwrap_or(text)
}

/// 逐行解析模板：縮排決定父層，`- 键:` 是序列項目（路徑記成 `*`）。回傳（模板路徑、清好的說明）。
fn template_notes(text: &str) -> Vec<(Vec<String>, String)> {
    let mut stack: Vec<(usize, Vec<String>)> = Vec::new();
    let mut notes = Vec::new();
    for line in template_body(text).lines() {
        let Some(found) = line_regex().captures(line) else {
            continue;
        };
        let indent = found.get(1).map_or(0, |m| m.as_str().chars().count());
        let item = found.get(2).is_some();
        let key = found[3]
            .trim()
            .trim_matches(|ch| ch == '"' || ch == '\'')
            .to_owned();
        let value = found[4].trim();
        while stack.last().is_some_and(|(level, _)| *level >= indent) {
            stack.pop();
        }
        let mut segments: Vec<String> = stack.iter().flat_map(|(_, s)| s.clone()).collect();
        let mut own = Vec::new();
        if item {
            own.push("*".to_owned());
        }
        own.push(key);
        segments.extend(own.iter().cloned());
        if value.is_empty() || value == "|-" || value == "|" {
            stack.push((indent, own));
            continue;
        }
        if let Some(placeholder) = placeholder_regex().captures(value) {
            if let Some(note) = clean_note(&placeholder[1]) {
                notes.push((segments, note));
            }
        }
    }
    notes
}

/// 欄位註記：模板路徑對到套用後的狀態（規則鍵或集合外的葉子，字形折疊比對）；序列項目底下的包裝層（`- 人物:`）
/// 對不上就拿掉再試。對不上的丟掉。
pub(super) fn field_notes(state: &WorldState, texts: &[String]) -> BTreeMap<String, String> {
    let mut known: BTreeMap<String, String> = BTreeMap::new();
    for path in state.mechanism.rules.keys() {
        known.insert(data::fold_glyphs(path).into_owned(), path.clone());
    }
    fn leaves(tree: &BTreeMap<String, StateNode>, prefix: &str, out: &mut Vec<String>) {
        for (key, node) in tree {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            match node {
                StateNode::Leaf(_) => out.push(path),
                StateNode::Branch(children) => leaves(children, &path, out),
            }
        }
    }
    let mut leaf_paths = Vec::new();
    leaves(&state.state.tree, "", &mut leaf_paths);
    for path in leaf_paths {
        let collection = state
            .mechanism
            .collections
            .keys()
            .any(|base| path.starts_with(&format!("{base}.")));
        if !collection {
            known
                .entry(data::fold_glyphs(&path).into_owned())
                .or_insert(path);
        }
    }
    let mut notes = BTreeMap::new();
    for text in texts {
        for (segments, note) in template_notes(text) {
            // 候選：原樣、拿掉序列項目底下的包裝層（`*.人物.体力` → `*.体力`）、條列的 `-` 不是序列（拿掉 `*`）
            let without_wrapper: Vec<&str> = segments
                .iter()
                .enumerate()
                .filter(|(index, _)| {
                    !(*index > 0 && segments[index - 1] == "*" && index + 1 < segments.len())
                })
                .map(|(_, segment)| segment.as_str())
                .collect();
            let without_items: Vec<&str> = segments
                .iter()
                .map(String::as_str)
                .filter(|segment| *segment != "*")
                .collect();
            let hit = [
                segments.join("."),
                without_wrapper.join("."),
                without_items.join("."),
            ]
            .into_iter()
            .find_map(|candidate| known.get(data::fold_glyphs(&candidate).as_ref()).cloned());
            match hit {
                Some(path) => {
                    notes.entry(path).or_insert(note);
                }
                None => log::debug!("狀態欄模板格子 {} 對不上狀態，不記註記", segments.join(".")),
            }
        }
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_drop_examples_and_format_boilerplate() {
        assert_eq!(clean_note("按照格式输出,示例:🌨️ 小雪 零下八度 "), None);
        assert_eq!(
            clean_note("💪 0-100 的数值与简述 "),
            Some("0-100 的数值与简述".to_owned())
        );
        assert_eq!(
            clean_note("🌾 剩余石数,示例:🌾 三百二十石 "),
            Some("剩余石数".to_owned())
        );
        assert_eq!(
            clean_note("当前伤势，例如：左臂擦伤"),
            Some("当前伤势".to_owned())
        );
        assert_eq!(clean_note("   "), None);
    }

    #[test]
    fn bullet_and_yaml_lines_both_parse() {
        let yaml = "<Status_block>\n状态栏:\n  天气: \"${按照格式输出,示例:🌨️ 小雪 }\"\n  人物列表:\n    - 人物:\n        体力: \"${💪 0-100 的数值与简述 }\"\n</Status_block>";
        let notes = template_notes(yaml);
        assert_eq!(
            notes,
            vec![(
                ["状态栏", "人物列表", "*", "人物", "体力"]
                    .map(str::to_owned)
                    .to_vec(),
                "0-100 的数值与简述".to_owned()
            )]
        );
        let bullets =
            "- 体力：${当前体力与疲劳}\n- 地点：${所在位置}\n这一行没有冒号\n- 坏行：没有占位";
        let notes = template_notes(bullets);
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].0, vec!["*".to_owned(), "体力".to_owned()]);
    }
}
