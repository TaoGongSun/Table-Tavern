//! 清單欄的 GM 報法（接在卡專屬指引／通用協定後面，進凍結快照）：
//! - 集合（`mechanism.collections`，takeover-list-structured）：每個項目在樹上各有路徑，只報變動的那一格；
//! - 其餘 `kind: list` 欄位（值是多行或行內集合字串）：不能 delta，改一行也要 replace 整個字串。
//!
//! 只讀規則與集合描述、不讀狀態樹——system 是凍結快照，讀樹會讓樹形一變整條 GM 線重開。範例的名字用
//! 佔位字，不用樹上的真名。沒有清單規則的桌逐字不變。

use crate::data::{CollectionItem, FieldKind, Mechanism};

use super::messages::scaffold_en;

pub(super) fn push_list_protocols(system: &mut String, mechanism: &Mechanism, lang: &str) {
    let en = scaffold_en(lang);
    if let Some(text) = collection_protocol(mechanism, en) {
        system.push_str("\n\n");
        system.push_str(&text);
    }
    let string_list = mechanism.rules.iter().any(|(path, rule)| {
        rule.kind == FieldKind::List && !mechanism.collections.contains_key(path)
    });
    if string_list {
        system.push_str("\n\n");
        system.push_str(if en {
            "List fields (values that are multi-line strings) never take delta, even when a line \
             contains numbers: to change any line, use replace and write the field's complete string \
             (copy unchanged lines as they are)."
        } else {
            "值是多行字串的清單欄，就算行內有數字也不能用 delta；改任何一行都用 replace 寫出整個欄位的完整字串（沒變的行也照抄）。"
        });
    }
}

/// JSON Pointer：點分路徑逐段跳脫（`~`→`~0`、`/`→`~1`）。
fn pointer(path: &str) -> String {
    path.split('.').map(segment).collect()
}

/// 指標的一段（含開頭的 `/`）：欄位名、項目名裡的 `/`、`~` 也要跳脫，否則會被讀成多一層。
fn segment(key: &str) -> String {
    format!("/{}", key.replace('~', "~0").replace('/', "~1"))
}

fn quoted(text: &str) -> String {
    serde_json::Value::String(text.to_owned()).to_string()
}

/// 集合的報法與範例：具名集合取第一個（依路徑排序），序列集合取第一個；兩種都沒有回 None。
fn collection_protocol(mechanism: &Mechanism, en: bool) -> Option<String> {
    let keyed = mechanism
        .collections
        .iter()
        .find(|(_, spec)| spec.keyed && spec.item == CollectionItem::Object);
    let sequence = mechanism.collections.iter().find(|(_, spec)| !spec.keyed);
    if keyed.is_none() && sequence.is_none() {
        return None;
    }
    let (name, old_name, new_name, new_value, items) = if en {
        (
            "(name)",
            "(old name)",
            "(new name)",
            "(new value)",
            "\"…\", \"…\"",
        )
    } else {
        (
            "（名字）",
            "（舊名）",
            "（新名）",
            "（新的值）",
            "\"…\", \"…\"",
        )
    };
    let mut lines = vec![if en {
        "Collection fields (lists such as who is present) are not strings: every item has its own \
         path in the state tree. Report only the cells that changed; the app writes the list's \
         layout, numbering and decoration itself."
            .to_owned()
    } else {
        "集合欄（在場人物這類清單）不是字串：每個項目在狀態樹上各有路徑，只報有變動的那一格；清單的排版、序號與裝飾由 App 補上。"
            .to_owned()
    }];
    if let Some((path, spec)) = keyed {
        let base = pointer(path);
        let field = spec.fields.keys().next();
        if let Some(field) = field {
            lines.push(format!(
                "{} {{\"op\": \"replace\", \"path\": {}, \"value\": {}}}",
                if en {
                    "- Change one field of an item:"
                } else {
                    "- 改某個項目的一格："
                },
                quoted(&format!("{base}{}{}", segment(name), segment(field))),
                quoted(new_value),
            ));
        }
        let object = spec
            .fields
            .keys()
            .map(|field| format!("{}: \"…\"", quoted(field)))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!(
            "{} {{\"op\": \"insert\", \"path\": {}, \"value\": {{{object}}}}}{}",
            if en {
                "- A new item (someone arrives, something is gained) — also how to add one when the list is empty:"
            } else {
                "- 新項目（有人進場、取得東西），集合目前沒有項目時也這樣新增："
            },
            quoted(&format!("{base}{}", segment(name))),
            if en {
                ". Give every field at once. The name is the last path segment — do not add a separate name field; two items with the same name need distinguishable names (e.g. \"Guard A\")."
            } else {
                "。一次給齊全部欄位；名字就是路徑末段，不要另寫名字欄，同名的要取可區分的名字（例「守卫甲」）。"
            },
        ));
        lines.push(format!(
            "{} {{\"op\": \"remove\", \"path\": {}}}{} {{\"op\": \"move\", \"from\": {}, \"path\": {}}}",
            if en { "- An item leaves:" } else { "- 項目離開：" },
            quoted(&format!("{base}{}", segment(name))),
            if en { "; renamed:" } else { "；改名：" },
            quoted(&format!("{base}{}", segment(old_name))),
            quoted(&format!("{base}{}", segment(new_name))),
        ));
    }
    if let Some((path, spec)) = sequence {
        lines.push(format!(
            "{} {{\"op\": \"replace\", \"path\": {}, \"value\": [{}]}}{}",
            if en {
                "- A numbered list (e.g. action options) is replaced as a whole:"
            } else {
                "- 序列（例如行動選項）整批換："
            },
            quoted(&pointer(path)),
            if spec.item == CollectionItem::Scalar {
                items.to_owned()
            } else {
                "{…}, {…}".to_owned()
            },
            if en {
                ". Write only the content of each entry — no numbering."
            } else {
                "，值只寫內容，不寫序號。"
            },
        ));
    }
    Some(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::FieldRule;

    fn north_hall() -> Mechanism {
        let mut mechanism = Mechanism::default();
        mechanism
            .rules
            .insert("在場人物".to_owned(), FieldRule::for_kind(FieldKind::List));
        mechanism
            .rules
            .insert("行動選項".to_owned(), FieldRule::for_kind(FieldKind::List));
        mechanism.collections = serde_json::from_value(serde_json::json!({
            "在場人物": { "keyed": true, "item": "object", "fields": { "行动": "text", "身份": "text" } },
            "行動選項": { "keyed": false, "item": "scalar", "type": "text" },
        }))
        .unwrap();
        mechanism
    }

    #[test]
    fn collections_get_path_protocol_and_no_whole_string_reminder() {
        let mut system = String::new();
        push_list_protocols(&mut system, &north_hall(), "zh-TW");
        assert!(
            system.contains(r#""path": "/在場人物/（名字）/行动""#),
            "{system}"
        );
        assert!(system.contains(r#"{"op": "insert", "path": "/在場人物/（名字）", "value": {"行动": "…", "身份": "…"}}"#), "{system}");
        assert!(system.contains("集合目前沒有項目時也這樣新增"));
        assert!(system.contains(r#"{"op": "replace", "path": "/行動選項", "value": ["…", "…"]}，值只寫內容，不寫序號。"#), "{system}");
        // 協定只教整批 replace：逐筆 remove 會因索引從 0、殼上序號從 1 刪錯
        assert!(!system.contains("/行動選項/"));
        assert!(!system.contains("完整字串"));
    }

    #[test]
    fn string_list_fields_keep_the_whole_string_reminder() {
        let mut mechanism = north_hall();
        mechanism
            .rules
            .insert("道具".to_owned(), FieldRule::for_kind(FieldKind::List));
        let mut system = String::new();
        push_list_protocols(&mut system, &mechanism, "zh-TW");
        assert!(system.contains("完整字串"));
    }

    #[test]
    fn tables_without_list_rules_are_untouched() {
        let mut system = String::new();
        push_list_protocols(&mut system, &Mechanism::default(), "en");
        assert!(system.is_empty());
    }

    #[test]
    fn english_scaffold_uses_english_placeholders() {
        let mut system = String::new();
        push_list_protocols(&mut system, &north_hall(), "en");
        assert!(system.contains(r#""/在場人物/(name)/行动""#), "{system}");
        assert!(system.contains("no numbering"));
    }

    #[test]
    fn field_names_with_slash_or_tilde_are_escaped_per_segment() {
        let mut mechanism = Mechanism::default();
        mechanism
            .rules
            .insert("隊/伍".to_owned(), FieldRule::for_kind(FieldKind::List));
        mechanism.collections = serde_json::from_value(serde_json::json!({
            "隊/伍": { "keyed": true, "item": "object", "fields": { "生命/上限": "text", "a~b": "text" } },
        }))
        .unwrap();
        let mut system = String::new();
        push_list_protocols(&mut system, &mechanism, "zh-TW");
        assert!(
            system.contains(r#""path": "/隊~1伍/（名字）/a~0b""#),
            "{system}"
        );
        assert!(
            system.contains(r#"{"a~b": "…", "生命/上限": "…"}"#),
            "{system}"
        );
        assert!(!system.contains("/生命/上限"));
    }
}
