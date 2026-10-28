//! 顯示前綴（statusbar-only-fields）：原卡狀態欄格子開頭的「表情符號＋空白」（「⏰ 」「👤 」）只是裝飾，
//! 重構時剝掉、存進 `mechanism.display_prefixes`，樹上與 GM 看到的都是不帶符號的值，狀態列顯示時再接回。
//! 寫進樹的入口（GM 回報、狀態圍欄、鏡像同步）都經這裡剝掉開頭相同的前綴，避免存成雙重前綴。

use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::data::{self, StateNode};

/// 開頭的表情符號區段：必須以 Extended_Pictographic 起頭，其後只接非 ASCII 的 Emoji_Component（變體選擇字元、
/// 零寬連接字元、膚色修飾）與更多 Extended_Pictographic，最後接空白。Emoji_Component 含 ASCII 數字與 `#`、`*`，
/// 不排除的話「55 赶了三天路」的 55 會被誤剝。
fn prefix_regex() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r"^\p{Extended_Pictographic}(?:\p{Extended_Pictographic}|[\p{Emoji_Component}&&[^\x00-\x7F]])*[ \t\u{3000}]+",
        )
        .expect("prefix regex")
    })
}

/// 值開頭的前綴（含結尾空白）與剩下的值；沒有前綴或剩下是空的回 None。
pub fn split_prefix(value: &str) -> Option<(&str, &str)> {
    let found = prefix_regex().find(value)?;
    let rest = &value[found.end()..];
    (!rest.trim().is_empty()).then(|| (found.as_str(), rest))
}

/// 路徑對應的前綴：先精確比對，再找段數相同、含 `*` 的鍵（萬用段最少者），比照 `rules::rule_for`。
pub fn prefix_for<'a>(prefixes: &'a BTreeMap<String, String>, path: &[String]) -> Option<&'a str> {
    if prefixes.is_empty() {
        return None;
    }
    if let Some(prefix) = prefixes.get(&path.join(".")) {
        return Some(prefix);
    }
    prefixes
        .iter()
        .filter_map(|(key, prefix)| {
            let segments: Vec<&str> = key.split('.').collect();
            (segments.len() == path.len()
                && segments.contains(&"*")
                && segments
                    .iter()
                    .zip(path)
                    .all(|(pattern, actual)| *pattern == "*" || pattern == actual))
            .then(|| (segments.iter().filter(|s| **s == "*").count(), prefix))
        })
        .min_by_key(|(wild, _)| *wild)
        .map(|(_, prefix)| prefix.as_str())
}

/// 寫入前剝掉：這條路徑有前綴、值又正好以它開頭（比較時忽略前綴結尾空白的多寡）就去掉。
pub fn strip<'a>(prefixes: &BTreeMap<String, String>, path: &[String], value: &'a str) -> &'a str {
    let Some(prefix) = prefix_for(prefixes, path) else {
        return value;
    };
    let head = prefix.trim_end();
    match value.strip_prefix(head) {
        Some(rest) if !rest.trim().is_empty() => rest.trim_start(),
        _ => value,
    }
}

/// JSON 值（GM 的 replace／insert）：字串照路徑剝；物件逐鍵往下剝（集合 add 整項時欄位各自剝）。
pub(super) fn strip_json(
    prefixes: &BTreeMap<String, String>,
    path: &[String],
    value: &mut serde_json::Value,
) {
    match value {
        serde_json::Value::String(text) => {
            let stripped = strip(prefixes, path, text);
            if stripped.len() != text.len() {
                *text = stripped.to_owned();
            }
        }
        serde_json::Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                let mut child_path = path.to_vec();
                child_path.push(key.clone());
                strip_json(prefixes, &child_path, child);
            }
        }
        _ => {}
    }
}

/// 套完一批更新後整理精確路徑的前綴：集合項目改名（move 成功）時跟著搬，路徑已不存在（remove）就清掉。
/// 萬用鍵不動。
pub(super) fn reconcile(
    prefixes: &mut BTreeMap<String, String>,
    tree: &BTreeMap<String, StateNode>,
    moves: &[(Vec<String>, Vec<String>)],
) {
    if prefixes.is_empty() {
        return;
    }
    for (from, to) in moves {
        if data::node_at(tree, from).is_some() || data::node_at(tree, to).is_none() {
            continue;
        }
        let from_key = from.join(".");
        let to_key = to.join(".");
        let moved: Vec<(String, String)> = prefixes
            .iter()
            .filter(|(key, _)| *key == &from_key || key.starts_with(&format!("{from_key}.")))
            .map(|(key, prefix)| (key.clone(), prefix.clone()))
            .collect();
        for (key, prefix) in moved {
            prefixes.remove(&key);
            prefixes.insert(format!("{to_key}{}", &key[from_key.len()..]), prefix);
        }
    }
    prefixes.retain(|key, _| {
        key.split('.').any(|segment| segment == "*") || {
            let segments: Vec<String> = key.split('.').map(str::to_owned).collect();
            data::node_at(tree, &segments).is_some()
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(text: &str) -> Vec<String> {
        text.split('.').map(str::to_owned).collect()
    }

    #[test]
    fn emoji_prefixes_are_recognised() {
        for (value, prefix, rest) in [
            (
                "⏰ 大雍永和三年十月初七 酉时",
                "⏰ ",
                "大雍永和三年十月初七 酉时",
            ),
            ("🪪 新任驿丞", "🪪 ", "新任驿丞"),
            ("👤 林远", "👤 ", "林远"),
            ("💪 55 赶了三天路，疲惫", "💪 ", "55 赶了三天路，疲惫"),
            (
                "🌨\u{fe0f} 北风小雪 零下八度",
                "🌨\u{fe0f} ",
                "北风小雪 零下八度",
            ),
        ] {
            assert_eq!(split_prefix(value), Some((prefix, rest)), "{value}");
        }
    }

    #[test]
    fn plain_text_and_digits_are_not_prefixes() {
        for value in [
            "55 赶了三天路",
            "# 標題",
            "* 星號",
            "1️⃣ 選項",
            "第 1 天",
            "👤",
            "👤 ",
        ] {
            assert_eq!(split_prefix(value), None, "{value}");
        }
    }

    #[test]
    fn exact_path_wins_over_wildcard_and_fewest_wildcards_win() {
        let prefixes: BTreeMap<String, String> = [
            ("人物.*.行动", "📝 "),
            ("人物.林远.行动", "🏃 "),
            ("*.*.行动", "❓ "),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
        assert_eq!(prefix_for(&prefixes, &path("人物.林远.行动")), Some("🏃 "));
        assert_eq!(prefix_for(&prefixes, &path("人物.贺铮.行动")), Some("📝 "));
        assert_eq!(prefix_for(&prefixes, &path("别处.贺铮.行动")), Some("❓ "));
        assert_eq!(prefix_for(&prefixes, &path("人物.贺铮")), None);
    }

    #[test]
    fn writes_drop_a_repeated_prefix_once() {
        let prefixes: BTreeMap<String, String> = [("地点".to_owned(), "📍 ".to_owned())].into();
        assert_eq!(
            strip(&prefixes, &path("地点"), "📍 北境驿站 仓房"),
            "北境驿站 仓房"
        );
        assert_eq!(strip(&prefixes, &path("地点"), "📍北境驿站"), "北境驿站");
        assert_eq!(strip(&prefixes, &path("地点"), "北境驿站"), "北境驿站");
        assert_eq!(strip(&prefixes, &path("天气"), "📍 北境"), "📍 北境");
        let mut item = serde_json::json!({ "地点": "📍 城门", "其他": 3 });
        strip_json(&prefixes, &[], &mut item);
        assert_eq!(item, serde_json::json!({ "地点": "城门", "其他": 3 }));
    }

    #[test]
    fn exact_prefixes_follow_moves_and_vanish_with_removed_paths() {
        let mut tree = BTreeMap::new();
        data::set_tree_value(&mut tree, &path("人物.秦岚.行动"), "巡山");
        data::set_tree_value(&mut tree, &path("人物.贺铮.行动"), "抽烟");
        let mut prefixes: BTreeMap<String, String> = [
            ("人物.岚.行动", "🏃 "),
            ("人物.岚", "🧭 "),
            ("人物.周掌柜.行动", "📝 "),
            ("人物.*.心情", "🙂 "),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
        reconcile(
            &mut prefixes,
            &tree,
            &[(path("人物.岚"), path("人物.秦岚"))],
        );
        assert_eq!(
            prefixes.get("人物.秦岚.行动").map(String::as_str),
            Some("🏃 ")
        );
        assert_eq!(prefixes.get("人物.秦岚").map(String::as_str), Some("🧭 "));
        assert!(!prefixes.contains_key("人物.岚.行动"));
        assert!(
            !prefixes.contains_key("人物.周掌柜.行动"),
            "已刪除的項目清掉"
        );
        assert!(prefixes.contains_key("人物.*.心情"), "萬用鍵不動");
    }
}
