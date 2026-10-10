//! 照搬網頁版（JS）時會碰到的語意差異：空白集合、trim、數字轉換、真假值、物件鍵順序、Math.round。
//! 掃描核心逐行照 `world-info-scan.ts` 移植，這些地方 Rust 標準庫的行為與 JS 不同，集中在這裡。

use serde_json::Value;

/// JS 的空白與行終止字元（`\s` 與 `String.prototype.trim` 共用的集合）。
pub fn is_js_whitespace(ch: char) -> bool {
    matches!(
        ch,
        '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

/// `String.prototype.trim`（Rust 的 `trim` 不去 U+FEFF）。
pub fn js_trim(text: &str) -> &str {
    text.trim_matches(is_js_whitespace)
}

/// JS 的 `\w`（不帶 `u` 或帶 `u` 都只有 ASCII）。
pub fn is_js_word(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

/// JS 真假值。
pub fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(flag)) => *flag,
        Some(Value::Number(number)) => number.as_f64().is_some_and(|n| n != 0.0 && !n.is_nan()),
        Some(Value::String(text)) => !text.is_empty(),
        Some(Value::Array(_)) | Some(Value::Object(_)) => true,
    }
}

/// JS `Math.round`：.5 一律往正無限大進位（Rust `round` 是遠離零）。
pub fn js_round(value: f64) -> f64 {
    if !value.is_finite() {
        return value;
    }
    let floor = value.floor();
    if value - floor >= 0.5 {
        floor + 1.0
    } else {
        floor
    }
}

/// JS `ToNumber`（給 order 相減用）；`None`＝undefined。
pub fn to_number(value: Option<&Value>) -> f64 {
    match value {
        None => f64::NAN,
        Some(Value::Null) => 0.0,
        Some(Value::Bool(flag)) => f64::from(u8::from(*flag)),
        Some(Value::Number(number)) => number.as_f64().unwrap_or(f64::NAN),
        Some(Value::String(text)) => string_to_number(text),
        Some(Value::Array(_)) => string_to_number(&to_js_string(value.expect("checked"))),
        Some(Value::Object(_)) => f64::NAN,
    }
}

/// 陣列 ToPrimitive 用的 `String(value)`（物件一律 `[object Object]`）。
fn to_js_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.as_f64().map(js_number_string).unwrap_or_default(),
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .map(|item| match item {
                Value::Null => String::new(),
                other => to_js_string(other),
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_owned(),
    }
}

fn js_number_string(number: f64) -> String {
    if number.fract() == 0.0 && number.abs() < 1e21 {
        format!("{number:.0}")
    } else {
        number.to_string()
    }
}

/// JS `StringToNumber`：去空白後空字串是 0；十進位、`Infinity`、`0x`／`0o`／`0b`；其餘 NaN。
pub fn string_to_number(text: &str) -> f64 {
    let text = js_trim(text);
    if text.is_empty() {
        return 0.0;
    }
    match text {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    for (prefix, radix) in [
        ("0x", 16),
        ("0X", 16),
        ("0o", 8),
        ("0O", 8),
        ("0b", 2),
        ("0B", 2),
    ] {
        if let Some(digits) = text.strip_prefix(prefix) {
            if digits.is_empty() || !digits.chars().all(|ch| ch.is_digit(radix)) {
                return f64::NAN;
            }
            return digits.chars().fold(0.0, |acc, ch| {
                acc * f64::from(radix) + f64::from(ch.to_digit(radix).expect("checked"))
            });
        }
    }
    let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
    let (mantissa, exponent) = match unsigned.find(['e', 'E']) {
        Some(at) => (&unsigned[..at], Some(&unsigned[at + 1..])),
        None => (unsigned, None),
    };
    let mantissa_ok = {
        let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        let digits = |part: &str| part.chars().all(|ch| ch.is_ascii_digit());
        (!int.is_empty() || !frac.is_empty()) && digits(int) && digits(frac)
    };
    let exponent_ok = exponent.is_none_or(|exp| {
        let exp = exp.strip_prefix(['+', '-']).unwrap_or(exp);
        !exp.is_empty() && exp.chars().all(|ch| ch.is_ascii_digit())
    });
    if !mantissa_ok || !exponent_ok {
        return f64::NAN;
    }
    text.parse().unwrap_or(f64::NAN)
}

/// JS 物件的鍵順序：陣列索引形的鍵（0–2³²−2 的正規整數字串）由小到大在前，其餘照插入順序。
pub fn js_key_order(keys: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut indices = Vec::new();
    let mut others = Vec::new();
    for key in keys {
        match array_index(&key) {
            Some(index) => indices.push((index, key)),
            None => others.push(key),
        }
    }
    indices.sort_by_key(|(index, _)| *index);
    indices
        .into_iter()
        .map(|(_, key)| key)
        .chain(others)
        .collect()
}

fn array_index(key: &str) -> Option<u64> {
    if key.is_empty() || (key.len() > 1 && key.starts_with('0')) {
        return None;
    }
    if !key.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    key.parse::<u64>()
        .ok()
        .filter(|index| *index <= 4_294_967_294)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn to_number_follows_js() {
        assert_eq!(to_number(Some(&json!("7"))), 7.0);
        assert_eq!(to_number(Some(&json!(" 0x1A "))), 26.0);
        assert_eq!(to_number(Some(&json!(""))), 0.0);
        assert!(to_number(Some(&json!("inf"))).is_nan());
        assert!(to_number(Some(&json!("1e"))).is_nan());
        assert_eq!(to_number(Some(&json!(".5"))), 0.5);
        assert_eq!(to_number(Some(&json!(true))), 1.0);
        assert_eq!(to_number(Some(&json!(null))), 0.0);
        assert_eq!(to_number(Some(&json!([]))), 0.0);
        assert_eq!(to_number(Some(&json!([5]))), 5.0);
        assert!(to_number(Some(&json!([1, 2]))).is_nan());
        assert!(to_number(Some(&json!({}))).is_nan());
        assert!(to_number(None).is_nan());
    }

    #[test]
    fn js_round_rounds_half_up() {
        assert_eq!(js_round(2.5), 3.0);
        assert_eq!(js_round(-2.5), -2.0);
        assert_eq!(js_round(f64::INFINITY), f64::INFINITY);
    }

    #[test]
    fn key_order_puts_array_indices_first() {
        let keys = ["zz", "10", "2", "01", "a"].map(str::to_owned);
        assert_eq!(js_key_order(keys), ["2", "10", "zz", "01", "a"]);
    }

    #[test]
    fn trim_removes_bom() {
        assert_eq!(js_trim("\u{FEFF} a \u{3000}"), "a");
    }
}
