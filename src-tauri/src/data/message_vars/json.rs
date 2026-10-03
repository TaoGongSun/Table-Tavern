//! 卡片變數表用的 JSON 值：物件保留鍵的原始順序（卡片照 `Object.entries` 的順序畫清單，排序過的表
//! 會讓畫面順序跳動），重複鍵照 `JSON.parse`——位置取第一次出現、值取最後一次。上限檢查（計畫 8.8）
//! 也在這裡。
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};
use serde_json::Number;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    pub fn empty_object() -> Self {
        Json::Object(Vec::new())
    }

    pub fn as_object(&self) -> Option<&Vec<(String, Json)>> {
        match self {
            Json::Object(entries) => Some(entries),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&Json> {
        self.as_object()?
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Json> {
        match self {
            Json::Object(entries) => entries
                .iter_mut()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    /// 已有這個鍵就原位換值，否則接在最後（同 JS 物件指定）。不是物件就不動、回 false。
    pub fn insert(&mut self, key: &str, value: Json) -> bool {
        let Json::Object(entries) = self else {
            return false;
        };
        match entries.iter_mut().find(|(name, _)| name == key) {
            Some((_, slot)) => *slot = value,
            None => entries.push((key.to_owned(), value)),
        }
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<Json> {
        let Json::Object(entries) = self else {
            return None;
        };
        let index = entries.iter().position(|(name, _)| name == key)?;
        Some(entries.remove(index).1)
    }

    pub fn to_text(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "null".to_owned())
    }
}

pub fn parse(text: &str) -> Result<Json, String> {
    serde_json::from_str(text).map_err(|error| error.to_string())
}

impl Serialize for Json {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Json::Null => serializer.serialize_unit(),
            Json::Bool(value) => serializer.serialize_bool(*value),
            Json::Number(value) => value.serialize(serializer),
            Json::String(value) => serializer.serialize_str(value),
            Json::Array(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Json::Object(entries) => {
                let mut map = serializer.serialize_map(Some(entries.len()))?;
                for (key, value) in entries {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
    }
}

struct JsonVisitor;

impl<'de> Visitor<'de> for JsonVisitor {
    type Value = Json;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("any JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Json, E> {
        Ok(Json::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Json, E> {
        Ok(Json::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Json, E> {
        Ok(Json::Number(value.into()))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Json, E> {
        Number::from_f64(value)
            .map(Json::Number)
            .ok_or_else(|| E::custom("non-finite number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Json, E> {
        Ok(Json::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Json, E> {
        Ok(Json::String(value))
    }

    fn visit_unit<E>(self) -> Result<Json, E> {
        Ok(Json::Null)
    }

    fn visit_none<E>(self) -> Result<Json, E> {
        Ok(Json::Null)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Json, D::Error> {
        Json::deserialize(deserializer)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Json, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Json::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Json, A::Error> {
        let mut object = Json::empty_object();
        while let Some((key, value)) = map.next_entry::<String, Json>()? {
            object.insert(&key, value);
        }
        Ok(object)
    }
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Json, D::Error> {
        deserializer.deserialize_any(JsonVisitor)
    }
}

/// 卡寫一張表的上限（計畫 8.8，依真卡量測預留約百倍）。
pub const MAX_TABLE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_DEPTH: usize = 32;
pub const MAX_STRING_BYTES: usize = 64 * 1024;
pub const MAX_CHILDREN: usize = 10_000;
pub const MAX_NODES: usize = 200_000;
pub const MAX_KEY_CHARS: usize = 256;

/// 不符上限的原因；前端照代碼顯示，`detail` 只給除錯。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LimitError {
    pub code: &'static str,
    pub detail: String,
}

fn limit(code: &'static str, detail: impl Into<String>) -> LimitError {
    LimitError {
        code,
        detail: detail.into(),
    }
}

/// 解析並驗一張要寫入的表：頂層必須是物件，任一條不符整批拒絕。數值非有限在解析時就擋掉
/// （JSON 本身寫不出 Infinity，`1e999` 解析失敗）。
pub fn parse_table(text: &str) -> Result<Json, LimitError> {
    if text.len() > MAX_TABLE_BYTES {
        return Err(limit("too-large", format!("{} bytes", text.len())));
    }
    let value = parse(text).map_err(|error| limit("invalid-json", error))?;
    validate_table(&value)?;
    Ok(value)
}

pub fn validate_table(value: &Json) -> Result<(), LimitError> {
    if !matches!(value, Json::Object(_)) {
        return Err(limit("not-object", "table must be an object"));
    }
    let mut nodes = 0usize;
    walk(value, 1, &mut nodes)
}

fn walk(value: &Json, depth: usize, nodes: &mut usize) -> Result<(), LimitError> {
    *nodes += 1;
    if *nodes > MAX_NODES {
        return Err(limit("too-many-nodes", format!("> {MAX_NODES}")));
    }
    if depth > MAX_DEPTH {
        return Err(limit("too-deep", format!("> {MAX_DEPTH}")));
    }
    match value {
        Json::String(text) if text.len() > MAX_STRING_BYTES => {
            Err(limit("string-too-long", format!("{} bytes", text.len())))
        }
        Json::Number(number) if number.as_f64().is_some_and(|value| !value.is_finite()) => {
            Err(limit("non-finite", number.to_string()))
        }
        Json::Array(items) => {
            if items.len() > MAX_CHILDREN {
                return Err(limit("too-many-children", format!("{}", items.len())));
            }
            items
                .iter()
                .try_for_each(|item| walk(item, depth + 1, nodes))
        }
        Json::Object(entries) => {
            if entries.len() > MAX_CHILDREN {
                return Err(limit("too-many-children", format!("{}", entries.len())));
            }
            for (key, child) in entries {
                if key.is_empty() {
                    return Err(limit("empty-key", ""));
                }
                if key.chars().count() > MAX_KEY_CHARS {
                    return Err(limit(
                        "key-too-long",
                        key.chars().take(32).collect::<String>(),
                    ));
                }
                walk(child, depth + 1, nodes)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objects_keep_key_order_and_duplicate_keys_follow_json_parse() {
        let value = parse(r#"{"z":1,"a":{"y":2,"b":3},"__proto__":4,"z":5}"#).unwrap();
        assert_eq!(
            value.to_text(),
            r#"{"z":5,"a":{"y":2,"b":3},"__proto__":4}"#
        );
    }

    #[test]
    fn numbers_and_scalars_round_trip() {
        let text = r#"{"i":-3,"f":2.5,"e":1e3,"t":true,"n":null,"s":"字","l":[1,[2]]}"#;
        let value = parse(text).unwrap();
        assert_eq!(parse(&value.to_text()).unwrap(), value);
        assert!(parse(r#"{"x":1e999}"#).is_err());
    }

    fn code(text: &str) -> &'static str {
        parse_table(text).unwrap_err().code
    }

    #[test]
    fn limits_reject_whole_table() {
        assert!(parse_table(r#"{"stat_data":{"a":[1,"說明"]}}"#).is_ok());
        assert_eq!(code("[1]"), "not-object");
        assert_eq!(code(r#"{"x":1e999}"#), "invalid-json");
        assert_eq!(code(r#"{"":1}"#), "empty-key");
        assert_eq!(
            code(&format!(r#"{{"{}":1}}"#, "k".repeat(257))),
            "key-too-long"
        );
        assert!(parse_table(&format!(r#"{{"{}":1}}"#, "鍵".repeat(256))).is_ok());
        assert_eq!(
            code(&format!(
                r#"{{"s":"{}"}}"#,
                "a".repeat(MAX_STRING_BYTES + 1)
            )),
            "string-too-long"
        );
        let deep = format!("{}1{}", "{\"a\":".repeat(32), "}".repeat(32));
        assert_eq!(code(&deep), "too-deep");
        let ok_deep = format!("{}1{}", "{\"a\":".repeat(31), "}".repeat(31));
        assert!(parse_table(&ok_deep).is_ok());
        let wide = format!("{{\"l\":[{}]}}", vec!["0"; MAX_CHILDREN + 1].join(","));
        assert_eq!(code(&wide), "too-many-children");
        let many = format!(
            "{{{}}}",
            (0..20)
                .map(|i| format!("\"k{i}\":[{}]", vec!["0"; MAX_CHILDREN].join(",")))
                .collect::<Vec<_>>()
                .join(",")
        );
        assert_eq!(code(&many), "too-many-nodes");
        let big = format!(r#"{{"s":"{}"}}"#, "a".repeat(MAX_TABLE_BYTES));
        assert_eq!(code(&big), "too-large");
    }
}
