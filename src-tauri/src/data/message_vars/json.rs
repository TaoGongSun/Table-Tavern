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

/// 卡寫一張表的上限（計畫 8.8，依真卡量測預留約百倍），前端 src/shared/contracts/vars-table.ts 同一組數字
/// 與先後。整張表的大小有兩種量法：卡片寫入（有原文）原文位元組或 `JSON.stringify` 緊湊寫法（[`js_len`]）
/// 任一不超過就收——舊版只量原文，這樣只放寬、不收窄；網頁存檔的表只量緊湊寫法（兩端拿得到的都只有值）。
pub const MAX_TABLE_BYTES: usize = 2 * 1024 * 1024;
/// 原文在解析前先擋的上限（整張上限的四倍，防超大字串）
pub const MAX_RAW_TABLE_BYTES: usize = 4 * MAX_TABLE_BYTES;
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
    if text.len() > MAX_RAW_TABLE_BYTES {
        return Err(limit("too-large", format!("{} bytes", text.len())));
    }
    let value = parse(text).map_err(|error| limit("invalid-json", error))?;
    check_table(&value, Some(text.len()))?;
    Ok(value)
}

/// 只有值（網頁存檔的表）：大小只量緊湊寫法。
pub fn validate_table(value: &Json) -> Result<(), LimitError> {
    check_table(value, None)
}

/// 先後：頂層物件 → 整張大小 → 逐節點。`raw_len`＝原文位元組（有原文時兩種量法任一通過即可）。
fn check_table(value: &Json, raw_len: Option<usize>) -> Result<(), LimitError> {
    if !matches!(value, Json::Object(_)) {
        return Err(limit("not-object", "table must be an object"));
    }
    if !raw_len.is_some_and(|raw| raw <= MAX_TABLE_BYTES) {
        let size = js_len(value);
        if size > MAX_TABLE_BYTES {
            return Err(limit("too-large", format!("{size} bytes")));
        }
    }
    let mut nodes = 0usize;
    walk(value, 1, &mut nodes)
}

/// 這個值經 `JSON.stringify` 寫成緊湊 JSON 的 UTF-8 位元組數（字串跳脫與 serde_json 相同；數字照 JS 的
/// Number#toString）。
pub fn js_len(value: &Json) -> usize {
    match value {
        Json::Null => 4,
        Json::Bool(true) => 4,
        Json::Bool(false) => 5,
        // JS 的數字都是 f64：超過 2^53 的整數先變成最接近的 f64 再照 Number#toString 寫
        Json::Number(number) => match (number.as_i64(), number.as_u64()) {
            (Some(int), _) if int.unsigned_abs() <= MAX_SAFE_INTEGER => number.to_string().len(),
            (None, Some(int)) if int <= MAX_SAFE_INTEGER => number.to_string().len(),
            _ => number.as_f64().map_or(0, |float| js_number(float).len()),
        },
        Json::String(text) => string_len(text),
        Json::Array(items) => {
            2 + items.len().saturating_sub(1) + items.iter().map(js_len).sum::<usize>()
        }
        Json::Object(entries) => {
            2 + entries.len().saturating_sub(1)
                + entries
                    .iter()
                    .map(|(key, child)| string_len(key) + 1 + js_len(child))
                    .sum::<usize>()
        }
    }
}

/// 2^53：JS 能精確表示的整數上限
const MAX_SAFE_INTEGER: u64 = 1 << 53;

fn string_len(text: &str) -> usize {
    serde_json::to_string(text).map_or(0, |quoted| quoted.len())
}

/// JS 的 Number#toString（ECMA-262 Number::toString，基數 10）：最短還原位數，位數與小數點位置照規格擺。
pub fn js_number(value: f64) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }
    let sign = if value < 0.0 { "-" } else { "" };
    let scientific = format!("{:e}", value.abs());
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i64;
    let n = exponent.parse::<i64>().unwrap_or(0) + 1;
    let body = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let exp = n - 1;
        let mantissa = if k == 1 {
            digits.clone()
        } else {
            format!("{}.{}", &digits[..1], &digits[1..])
        };
        format!(
            "{mantissa}e{}{}",
            if exp >= 0 { "+" } else { "-" },
            exp.abs()
        )
    };
    format!("{sign}{body}")
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

    /// 緊湊寫法剛好 `size` 位元組的表：`{"k0":"aaa…","k1":"…",…}`，每個字串不超過單字串上限。
    fn table_of_size(size: usize) -> String {
        let mut entries: Vec<String> = Vec::new();
        let mut used = 2;
        let mut index = 0;
        loop {
            let key = format!("k{index}");
            let fixed = usize::from(index > 0) + key.len() + 2 + 1 + 2;
            let room = size - used - fixed;
            let take = room.min(60_000);
            entries.push(format!(r#""{key}":"{}""#, "a".repeat(take)));
            used += fixed + take;
            index += 1;
            if take == room {
                break;
            }
        }
        let text = format!("{{{}}}", entries.join(","));
        assert_eq!(text.len(), size);
        text
    }

    #[test]
    fn table_size_counts_the_compact_js_form_not_the_raw_whitespace() {
        let exact = table_of_size(MAX_TABLE_BYTES);
        assert_eq!(js_len(&parse(&exact).unwrap()), MAX_TABLE_BYTES);
        // 原文縮排過（超過上限）照樣收：量的是緊湊寫法
        let padded = exact.replace(',', " ,\n   ").replace(':', " : ");
        assert!(padded.len() > MAX_TABLE_BYTES);
        assert!(parse_table(&padded).is_ok());
        let over = table_of_size(MAX_TABLE_BYTES + 1).replace(',', " , ");
        assert_eq!(code(&over), "too-large");
        // 原文本身超過四倍上限：解析前就擋
        let huge = format!("{{{}\"a\":1}}", " ".repeat(MAX_RAW_TABLE_BYTES));
        assert_eq!(code(&huge), "too-large");
        // 孤立代理字元：JSON 解析不收
        assert_eq!(code(r#"{"s":"\ud800"}"#), "invalid-json");
    }

    /// 照 vars-table-boundary.json 的造法：原文剛好 `total` 位元組（前端 vars-table.test.ts 同一個造法）。
    fn boundary_table(number: &str, spaces: usize, total: usize) -> String {
        let mut text = format!("{{{}\"n\":{number}", " ".repeat(spaces));
        let mut used = text.len() + 1;
        let mut index = 0;
        while used < total {
            let key = format!("k{index}");
            let room = total - used - (key.len() + 6);
            let take = if room <= 60_000 {
                room
            } else {
                60_000.min(room - 20)
            };
            text.push_str(&format!(r#","{key}":"{}""#, "a".repeat(take)));
            used += key.len() + 6 + take;
            index += 1;
        }
        text.push('}');
        assert_eq!(text.len(), total);
        text
    }

    /// 前端 vars-table.test.ts 讀同一份：卡片寫入（有原文，兩種量法任一通過）與網頁存檔的表（只量緊湊寫法）。
    #[test]
    fn shared_size_boundary_cases_match_the_frontend() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../src/shared/contracts/vars-table-boundary.json");
        let spec: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let outcome = |result: Result<(), LimitError>| match result {
            Ok(()) => "ok".to_owned(),
            Err(error) => error.code.to_owned(),
        };
        for case in spec["cases"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            let total = (MAX_TABLE_BYTES as i64 + case["raw_offset"].as_i64().unwrap()) as usize;
            let text = boundary_table(
                case["number"].as_str().unwrap(),
                case["spaces"].as_u64().unwrap() as usize,
                total,
            );
            assert_eq!(
                outcome(parse_table(&text).map(|_| ())),
                case["text"].as_str().unwrap(),
                "{name}（卡片寫入）"
            );
            assert_eq!(
                outcome(validate_table(&parse(&text).unwrap())),
                case["value"].as_str().unwrap(),
                "{name}（網頁存檔的表）"
            );
        }
    }

    /// 舊版只量原文：原文剛好上限、數字寫法在 JS 會變長的表，舊版收的現在照樣收。
    #[test]
    fn a_table_the_old_measure_accepted_is_still_accepted() {
        let text = boundary_table("1e20", 0, MAX_TABLE_BYTES);
        assert!(parse_table(&text).is_ok());
    }

    #[test]
    fn numbers_are_measured_like_js_number_to_string() {
        for (text, js) in [
            ("1.0", "1"),
            ("-2.5", "-2.5"),
            ("1e3", "1000"),
            ("1e20", "100000000000000000000"),
            ("1e21", "1e+21"),
            ("1.5e300", "1.5e+300"),
            ("0.000001", "0.000001"),
            ("1e-7", "1e-7"),
            ("-1.25e-10", "-1.25e-10"),
            ("123.456", "123.456"),
            ("0.1", "0.1"),
            ("-0.0", "0"),
        ] {
            let value = parse(&format!(r#"{{"n":{text}}}"#)).unwrap();
            assert_eq!(js_len(&value), r#"{"n":}"#.len() + js.len(), "{text}");
        }
        assert_eq!(js_len(&parse(r#"{"i":12345678901234567890}"#).unwrap()), 26);
        // 超過 2^53 的整數照 JS 先變 f64：999999999999999999 → 1000000000000000000
        assert_eq!(
            js_len(&parse(r#"{"i":999999999999999999}"#).unwrap()),
            r#"{"i":1000000000000000000}"#.len()
        );
        assert_eq!(
            js_len(&parse(r#"{"i":-9007199254740993}"#).unwrap()),
            r#"{"i":-9007199254740992}"#.len()
        );
        assert_eq!(
            js_len(&parse(r#"{"i":9007199254740992}"#).unwrap()),
            r#"{"i":9007199254740992}"#.len()
        );
        assert_eq!(
            js_len(&parse(r#"{"s":"a\"\n\u0001字"}"#).unwrap()),
            r#"{"s":"a\"\n\u0001字"}"#.len()
        );
    }
}
