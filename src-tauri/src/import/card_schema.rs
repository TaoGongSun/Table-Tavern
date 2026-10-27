//! zod 版 MVU 卡的變數 schema（mvu-zod-semantics 規格 1.5、2.5 節）：酒館助手腳本裡啟用中、內容含
//! `registerMvuSchema` 的那一支存原文；同卡的 `[initvar]` 原文一併帶著，引擎拿它建保序的種子。
//! 只偵測、不執行：執行在 `mvu_engine` 的隔離 QuickJS 裡。
use crate::world_info::book_order::{st_order, SourceEntries};
use crate::world_info::entry::{from_character_book, from_world_file, parse_decorators};
use serde_json::Value;

/// 卡片自帶的 zod schema 腳本與 `[initvar]` 原文
#[derive(Debug, Clone, PartialEq)]
pub struct ZodSchema {
    pub source: String,
    /// 卡內世界書標題含 `[initvar]` 的條目內文（停用的也算，照條目順序；同網頁宿主與上游 initCheck）
    pub initvars: Vec<String>,
}

/// 照腳本順序取第一支啟用中、含 `registerMvuSchema` 的腳本；停用的不算（上游照樣不跑）。
pub(super) fn zod_schema(card_data: &Value) -> Option<ZodSchema> {
    let extensions = card_data.get("extensions")?;
    let source = ["tavern_helper", "TavernHelper_scripts"]
        .iter()
        .filter_map(|key| extensions.get(*key))
        .find_map(first_schema_script)?;
    Some(ZodSchema {
        source,
        initvars: initvar_texts(card_data),
    })
}

/// 腳本形狀各版不同：一律依文件順序遞迴找帶 `content` 的物件；停用的物件連同底下一起略過。
fn first_schema_script(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            if map.get("enabled").and_then(Value::as_bool) == Some(false) {
                return None;
            }
            if let Some(content) = map.get("content").and_then(Value::as_str) {
                if content.contains("registerMvuSchema") {
                    return Some(content.to_owned());
                }
            }
            map.values().find_map(first_schema_script)
        }
        Value::Array(items) => items.iter().find_map(first_schema_script),
        _ => None,
    }
}

/// 卡內世界書（角色書）標題含 `[initvar]` 的條目內文，照 ST 載入後的先後（同網頁宿主 `initvarEntries`／`bookEntries`）：
/// 陣列形照 `id`（缺則索引）整數由小到大、id 重複後蓋前；物件形照 JS 鍵序。標題只看 comment，內文先剝 `@@` 裝飾。
/// 停用的條目照收（上游 initCheck 不看 enabled）。
fn initvar_texts(card_data: &Value) -> Vec<String> {
    let source = match card_data
        .get("character_book")
        .and_then(|book| book.get("entries"))
    {
        Some(Value::Array(items)) => SourceEntries::Array(items.clone()),
        Some(Value::Object(map)) => SourceEntries::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        ),
        _ => return Vec::new(),
    };
    let array = source.is_array();
    st_order(&source)
        .entries
        .into_iter()
        .filter_map(|(_, entry)| {
            let fields = match array {
                true => from_character_book(entry),
                false => from_world_file(entry),
            };
            fields
                .comment
                .to_lowercase()
                .contains("[initvar]")
                .then(|| parse_decorators(&fields.content).1)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn script(enabled: bool, content: &str) -> Value {
        json!({ "name": "腳本", "enabled": enabled, "content": content })
    }

    #[test]
    fn first_enabled_schema_script_wins_and_initvar_comes_along() {
        let card = json!({
            "extensions": { "tavern_helper": { "scripts": [
                script(false, "registerMvuSchema(停用)"),
                script(true, "console.log(1)"),
                script(true, "registerMvuSchema(第一)"),
                script(true, "registerMvuSchema(第二)"),
            ]}},
            "character_book": { "entries": [
                { "comment": "[initvar]啟用中的也算", "enabled": true, "content": "a: 1" },
                { "comment": "地點", "content": "晨港" },
                { "comment": "[InitVar]初始", "enabled": false, "content": "b: 2" },
            ]}
        });
        assert_eq!(
            zod_schema(&card),
            Some(ZodSchema {
                source: "registerMvuSchema(第一)".into(),
                initvars: vec!["a: 1".into(), "b: 2".into()],
            })
        );
    }

    #[test]
    fn disabled_or_missing_schema_is_not_a_zod_card() {
        let disabled = json!({ "extensions": { "tavern_helper": { "scripts": [script(false, "registerMvuSchema(x)")] }}});
        assert_eq!(zod_schema(&disabled), None);
        assert_eq!(zod_schema(&json!({ "extensions": {} })), None);
        let legacy = json!({ "extensions": { "TavernHelper_scripts": [{ "type": "script", "value": { "enabled": true, "content": "registerMvuSchema(舊)" } }] }});
        assert_eq!(
            zod_schema(&legacy).map(|found| found.source),
            Some("registerMvuSchema(舊)".into())
        );
    }

    /// 與網頁宿主同一份案例（src/shared/contracts/mvu-initvar-order.json）：收集順序照 ST 載入後的先後
    #[test]
    fn initvar_order_matches_the_web_host() {
        let contract: Value = serde_json::from_str(include_str!(
            "../../../src/shared/contracts/mvu-initvar-order.json"
        ))
        .unwrap();
        for case in contract["cases"].as_array().unwrap() {
            let card = json!({ "character_book": { "entries": case["entries"].clone() } });
            let expected: Vec<String> = serde_json::from_value(case["expected"].clone()).unwrap();
            assert_eq!(initvar_texts(&card), expected, "{}", case["name"]);
        }
    }
}
