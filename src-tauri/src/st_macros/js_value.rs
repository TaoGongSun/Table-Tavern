//! 網頁版巨集引擎與變數層用到的 JS 值語意：`String()`、`Number()`、真假值、`JSON.parse`／`JSON.stringify`
//! （物件保住鍵順序）、`Number.prototype.toString`、ST `getStringHash`（cyrb53）。
//! 不模仿 JS 原型：物件、陣列、字串上的原型屬性（`constructor`、`toString`…）一律當不存在。

use std::fmt;

use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::Deserialize;

use crate::world_info::js_semantics::{js_key_order, js_trim, string_to_number};

/// 字串長度的實用上限（UTF-8 位元組）：一段求值結果、字串相加、索引寫入後的 JSON 超過就當丟例外。
/// JS 要到 V8 字串上限（約 5 億碼元）才失敗，途中可吃掉 GB 級記憶體；這裡做得比 ST 好。
pub const MAX_TEXT_BYTES: usize = 10 << 20;

#[cfg(test)]
thread_local! {
    /// 測試可縮小上限（每個測試各自一條執行緒）
    pub static TEXT_LIMIT: std::cell::Cell<usize> = const { std::cell::Cell::new(MAX_TEXT_BYTES) };
}

/// 目前的字串長度上限（正式建置恆為 [`MAX_TEXT_BYTES`]）。
pub fn text_limit() -> usize {
    #[cfg(test)]
    return TEXT_LIMIT.with(std::cell::Cell::get);
    #[cfg(not(test))]
    MAX_TEXT_BYTES
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsValue {
    Undefined,
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    /// `Undefined` 元素＝空位（`JSON.stringify` 寫成 null）
    Array(Vec<JsValue>),
    /// 鍵依插入順序；輸出時照 JS 鍵順序（整數鍵在前）
    Object(Vec<(String, JsValue)>),
}

impl JsValue {
    pub fn str(text: impl Into<String>) -> Self {
        JsValue::Str(text.into())
    }

    /// `String(value)`
    pub fn to_js_string(&self) -> String {
        match self {
            JsValue::Undefined => "undefined".to_owned(),
            JsValue::Null => "null".to_owned(),
            JsValue::Bool(flag) => flag.to_string(),
            JsValue::Num(number) => number_to_string(*number),
            JsValue::Str(text) => text.clone(),
            JsValue::Array(items) => items
                .iter()
                .map(|item| match item {
                    JsValue::Undefined | JsValue::Null => String::new(),
                    other => other.to_js_string(),
                })
                .collect::<Vec<_>>()
                .join(","),
            JsValue::Object(_) => "[object Object]".to_owned(),
        }
    }

    /// `Number(value)`
    pub fn to_number(&self) -> f64 {
        match self {
            JsValue::Undefined | JsValue::Object(_) => f64::NAN,
            JsValue::Null => 0.0,
            JsValue::Bool(flag) => f64::from(u8::from(*flag)),
            JsValue::Num(number) => *number,
            JsValue::Str(text) => string_to_number(text),
            JsValue::Array(_) => string_to_number(&self.to_js_string()),
        }
    }

    /// JS 真假值
    pub fn truthy(&self) -> bool {
        match self {
            JsValue::Undefined | JsValue::Null => false,
            JsValue::Bool(flag) => *flag,
            JsValue::Num(number) => *number != 0.0 && !number.is_nan(),
            JsValue::Str(text) => !text.is_empty(),
            JsValue::Array(_) | JsValue::Object(_) => true,
        }
    }

    /// `typeof value === "object"`（含 null）
    pub fn is_object_like(&self) -> bool {
        matches!(self, JsValue::Null | JsValue::Array(_) | JsValue::Object(_))
    }

    /// `JSON.stringify(value)`；頂層 undefined 回 `None`。
    pub fn stringify(&self) -> Option<String> {
        let mut out = String::new();
        if matches!(self, JsValue::Undefined) {
            return None;
        }
        write_json(self, &mut out);
        Some(out)
    }

    /// `JSON.parse(text)`；失敗回 `None`。
    pub fn parse_json(text: &str) -> Option<JsValue> {
        serde_json::from_str(text).ok()
    }

    /// `JSON.parse(value)`：參數先轉字串（`String(value)`）。
    pub fn parse_coerced(&self) -> Option<JsValue> {
        match self {
            JsValue::Str(text) => JsValue::parse_json(text),
            other => JsValue::parse_json(&other.to_js_string()),
        }
    }

    /// 巨集結果正規化（網頁版 `normalizeResult`）：null／undefined 是空字串、物件與陣列 JSON 化。
    pub fn normalize(&self) -> String {
        match self {
            JsValue::Undefined | JsValue::Null => String::new(),
            JsValue::Array(_) | JsValue::Object(_) => self.stringify().unwrap_or_default(),
            other => other.to_js_string(),
        }
    }

    /// `value[key]`（只看自有屬性與 `length`）。
    pub fn property(&self, key: &str) -> JsValue {
        match self {
            JsValue::Str(text) => {
                if key == "length" {
                    return JsValue::Num(text.encode_utf16().count() as f64);
                }
                match array_index(key) {
                    Some(index) => text
                        .encode_utf16()
                        .nth(index)
                        .map(|unit| {
                            JsValue::Str(
                                char::from_u32(u32::from(unit))
                                    .unwrap_or(char::REPLACEMENT_CHARACTER)
                                    .to_string(),
                            )
                        })
                        .unwrap_or(JsValue::Undefined),
                    None => JsValue::Undefined,
                }
            }
            JsValue::Array(items) => {
                if key == "length" {
                    return JsValue::Num(items.len() as f64);
                }
                array_index(key)
                    .and_then(|index| items.get(index).cloned())
                    .unwrap_or(JsValue::Undefined)
            }
            JsValue::Object(entries) => entries
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.clone())
                .unwrap_or(JsValue::Undefined),
            _ => JsValue::Undefined,
        }
    }
}

/// 物件的自有屬性寫入：有就原地換值，沒有就加在最後。
pub fn put_entry(entries: &mut Vec<(String, JsValue)>, key: &str, value: JsValue) {
    match entries.iter_mut().find(|(name, _)| name == key) {
        Some(slot) => slot.1 = value,
        None => entries.push((key.to_owned(), value)),
    }
}

/// 正規的陣列索引字串（0–2³²−2，沒有前導零）。
pub fn array_index(key: &str) -> Option<usize> {
    if key.is_empty() || (key.len() > 1 && key.starts_with('0')) {
        return None;
    }
    if !key.bytes().all(|byte| byte.is_ascii_digit()) || key.len() > 10 {
        return None;
    }
    key.parse::<u64>()
        .ok()
        .filter(|index| *index <= 4_294_967_294)
        .and_then(|index| usize::try_from(index).ok())
}

/// 最短往返位數的科學記號。位數 k 由 Rust 的最短表示決定；同為 k 位的候選有兩個一樣近時，Rust 不一定
/// 取 JS 規定的那個（ECMAScript Number::toString：取最接近原值者，一樣近取偶數末位），所以改用精確的
/// k 位四捨六入五成雙重排，仍能往返就用它。
fn shortest_exponential(number: f64) -> String {
    let shortest = format!("{number:e}");
    let digits = shortest.split_once('e').map_or(0, |(mantissa, _)| {
        mantissa.chars().filter(char::is_ascii_digit).count()
    });
    let rounded = format!("{number:.*e}", digits.saturating_sub(1));
    if rounded.parse::<f64>() == Ok(number) {
        rounded
    } else {
        shortest
    }
}

/// `Number.prototype.toString()`（十進位）：最短往返位數，照 ECMAScript Number::toString 的排版。
pub fn number_to_string(number: f64) -> String {
    if number.is_nan() {
        return "NaN".to_owned();
    }
    if number == 0.0 {
        return "0".to_owned();
    }
    if number.is_infinite() {
        return if number > 0.0 {
            "Infinity"
        } else {
            "-Infinity"
        }
        .to_owned();
    }
    let sci = shortest_exponential(number.abs());
    let (mantissa, exponent) = sci.split_once('e').expect("LowerExp 一定有 e");
    let digits: String = mantissa.chars().filter(|ch| *ch != '.').collect();
    let k = digits.len() as i64;
    let n = exponent.parse::<i64>().expect("指數是整數") + 1;
    let body = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let sign = if e >= 0 { '+' } else { '-' };
        if k == 1 {
            format!("{digits}e{sign}{}", e.abs())
        } else {
            format!("{}.{}e{sign}{}", &digits[..1], &digits[1..], e.abs())
        }
    };
    if number < 0.0 {
        format!("-{body}")
    } else {
        body
    }
}

/// `String.prototype.trim` 的別名（巨集引擎各處都用 JS 的空白集合）。
pub fn trim(text: &str) -> &str {
    js_trim(text)
}

/// ST `utils.getStringHash`（cyrb53，逐 UTF-16 碼元）。
pub fn string_hash(text: &str, seed: u32) -> f64 {
    let mut h1: u32 = 0xdead_beef ^ seed;
    let mut h2: u32 = 0x41c6_ce57 ^ seed;
    for unit in text.encode_utf16() {
        let ch = u32::from(unit);
        h1 = (h1 ^ ch).wrapping_mul(2_654_435_761);
        h2 = (h2 ^ ch).wrapping_mul(1_597_334_677);
    }
    h1 = (h1 ^ (h1 >> 16)).wrapping_mul(2_246_822_507)
        ^ (h2 ^ (h2 >> 13)).wrapping_mul(3_266_489_909);
    h2 = (h2 ^ (h2 >> 16)).wrapping_mul(2_246_822_507)
        ^ (h1 ^ (h1 >> 13)).wrapping_mul(3_266_489_909);
    4_294_967_296.0 * f64::from(0x1F_FFFF & h2) + f64::from(h1)
}

/// UTF-16 長度（網頁版的位置都以碼元計）。
pub fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

fn write_json(value: &JsValue, out: &mut String) {
    match value {
        JsValue::Undefined | JsValue::Null => out.push_str("null"),
        JsValue::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
        JsValue::Num(number) if number.is_finite() => out.push_str(&number_to_string(*number)),
        JsValue::Num(_) => out.push_str("null"),
        JsValue::Str(text) => write_json_string(text, out),
        JsValue::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_json(item, out);
            }
            out.push(']');
        }
        JsValue::Object(entries) => {
            out.push('{');
            let mut first = true;
            for key in js_key_order(entries.iter().map(|(key, _)| key.clone())) {
                let value = &entries
                    .iter()
                    .find(|(name, _)| *name == key)
                    .expect("鍵來自同一份清單")
                    .1;
                if matches!(value, JsValue::Undefined) {
                    continue;
                }
                if !first {
                    out.push(',');
                }
                first = false;
                write_json_string(&key, out);
                out.push(':');
                write_json(value, out);
            }
            out.push('}');
        }
    }
}

fn write_json_string(text: &str, out: &mut String) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if (ch as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
}

impl<'de> Deserialize<'de> for JsValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(JsValueVisitor)
    }
}

struct JsValueVisitor;

impl<'de> Visitor<'de> for JsValueVisitor {
    type Value = JsValue;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("JSON")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<JsValue, E> {
        Ok(JsValue::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<JsValue, E> {
        Ok(JsValue::Num(value as f64))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<JsValue, E> {
        Ok(JsValue::Num(value as f64))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<JsValue, E> {
        Ok(JsValue::Num(value))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<JsValue, E> {
        Ok(JsValue::Str(value.to_owned()))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<JsValue, E> {
        Ok(JsValue::Str(value))
    }

    fn visit_unit<E: de::Error>(self) -> Result<JsValue, E> {
        Ok(JsValue::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<JsValue, E> {
        Ok(JsValue::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<JsValue, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element::<JsValue>()? {
            items.push(item);
        }
        Ok(JsValue::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<JsValue, A::Error> {
        let mut entries = Vec::new();
        while let Some((key, value)) = map.next_entry::<String, JsValue>()? {
            // JSON.parse：重複的鍵後蓋前、位置留在第一次出現處
            put_entry(&mut entries, &key, value);
        }
        Ok(JsValue::Object(entries))
    }
}

/// `string.trim() === ""`
pub fn is_blank(text: &str) -> bool {
    trim(text).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_to_string_follows_js() {
        let cases = [
            (1.0, "1"),
            (-0.0, "0"),
            (0.1 + 0.2, "0.30000000000000004"),
            (1e21, "1e+21"),
            (1e20, "100000000000000000000"),
            (1.5e-7, "1.5e-7"),
            (0.000001, "0.000001"),
            (123.456, "123.456"),
            (-2.5, "-2.5"),
            (f64::NAN, "NaN"),
            (f64::NEG_INFINITY, "-Infinity"),
            (5e-324, "5e-324"),
            // 同位數兩個候選一樣近：取偶數末位
            (3.62262725830078125, "3.6226272583007812"),
            (83897829055786.125, "83897829055786.12"),
            (1.7976931348623157e308, "1.7976931348623157e+308"),
        ];
        for (number, expected) in cases {
            assert_eq!(number_to_string(number), expected, "{number}");
        }
    }

    #[test]
    fn doubles_round_trip_through_json_exactly() {
        // serde_json 沒開 float_roundtrip 時這個會差 1 ULP
        let known = 1.5515404846519232e-9;
        let mut samples = vec![known, 0.1, 1e-300, 1.7976931348623157e308, 5e-324];
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        while samples.len() < 20_000 {
            // xorshift64*：隨機位元組成的有限雙精度數
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            let bits = state.wrapping_mul(0x2545_F491_4F6C_DD1D);
            let number = f64::from_bits(bits);
            if number.is_finite() {
                samples.push(number);
            }
        }
        for number in samples {
            let text = JsValue::Num(number).stringify().unwrap();
            assert_eq!(
                JsValue::parse_json(&text),
                Some(JsValue::Num(number)),
                "{text}"
            );
        }
    }

    #[test]
    fn json_keeps_key_order_like_js() {
        let value =
            JsValue::parse_json(r#"{"b":1,"a":[1,"x",null],"2":true,"1":{"z":"\n"},"b":2}"#)
                .unwrap();
        assert_eq!(
            value.stringify().unwrap(),
            r#"{"1":{"z":"\n"},"2":true,"b":2,"a":[1,"x",null]}"#
        );
        assert_eq!(
            JsValue::str("\u{1}\"").stringify().unwrap(),
            "\"\\u0001\\\"\""
        );
    }

    #[test]
    fn coercions_follow_js() {
        let array = JsValue::parse_json("[1,[2,null],{}]").unwrap();
        assert_eq!(array.to_js_string(), "1,2,,[object Object]");
        assert!(array.to_number().is_nan());
        assert_eq!(JsValue::parse_json("[5]").unwrap().to_number(), 5.0);
        assert_eq!(JsValue::Null.to_number(), 0.0);
        assert_eq!(JsValue::str(" 0x10 ").to_number(), 16.0);
        assert!(!JsValue::Num(f64::NAN).truthy());
        assert_eq!(JsValue::str("abc").property("1"), JsValue::str("b"));
        assert_eq!(JsValue::str("abc").property("length"), JsValue::Num(3.0));
    }

    #[test]
    fn string_hash_matches_st() {
        // 網頁版 getStringHash 的輸出
        assert_eq!(string_hash("", 0), 3_338_908_027_751_811.0);
        assert_eq!(string_hash("c", 0), 217_965_353_842_102.0);
        assert_eq!(string_hash("莫拉😀", 0), 4_372_635_856_761_939.0);
    }
}
