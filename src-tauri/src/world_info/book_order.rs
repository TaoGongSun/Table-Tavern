//! 卡內世界書條目在 ST 裡的先後（網頁版 `world-info-book.ts` 的 `stOrder`）：同 order 時的掃描、插入、
//! 預算取捨與抽選都照它，匯入時新 uid 也依此配發。條目要從原文解析才保得住鍵的出現順序
//! （`serde_json::Value` 的物件會把鍵排序）。

use std::collections::HashMap;

use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};

use super::js_semantics::{js_key_order, js_string};

/// 世界書 `entries` 的原文解析結果；物件形保留鍵的出現順序（重複鍵照 JS：值取後者、位置留在前者）。
#[derive(Debug, Clone, PartialEq)]
pub enum SourceEntries {
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

/// 物件形的鍵值對，依出現順序。
struct OrderedMap(Vec<(String, Value)>);

impl<'de> Deserialize<'de> for OrderedMap {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Collect;
        impl<'de> Visitor<'de> for Collect {
            type Value = OrderedMap;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a JSON object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<OrderedMap, A::Error> {
                let mut items: Vec<(String, Value)> = Vec::new();
                let mut seen: HashMap<String, usize> = HashMap::new();
                while let Some((key, value)) = map.next_entry::<String, Value>()? {
                    match seen.get(&key) {
                        Some(&at) => items[at].1 = value,
                        None => {
                            seen.insert(key.clone(), items.len());
                            items.push((key, value));
                        }
                    }
                }
                Ok(OrderedMap(items))
            }
        }
        deserializer.deserialize_map(Collect)
    }
}

impl SourceEntries {
    /// 解析 `entries` 的 JSON 原文；不是陣列也不是物件回 `None`。
    pub fn parse(text: &str) -> Result<Option<Self>, serde_json::Error> {
        match text.trim_start().as_bytes().first() {
            Some(b'[') => Ok(Some(Self::Array(serde_json::from_str(text)?))),
            Some(b'{') => Ok(Some(Self::Object(
                serde_json::from_str::<OrderedMap>(text)?.0,
            ))),
            _ => {
                serde_json::from_str::<Value>(text)?;
                Ok(None)
            }
        }
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Self::Array(_))
    }

    /// 卡片契約的 key（陣列索引或物件鍵）配條目；值不是物件的不算條目，略過。
    fn keyed(&self) -> Vec<(String, &Map<String, Value>)> {
        match self {
            Self::Array(items) => items
                .iter()
                .enumerate()
                .filter_map(|(index, value)| Some((index.to_string(), value.as_object()?)))
                .collect(),
            Self::Object(items) => items
                .iter()
                .filter_map(|(key, value)| Some((key.clone(), value.as_object()?)))
                .collect(),
        }
    }
}

/// ST 載入後的條目先後。
pub struct StOrdered<'a> {
    /// 留下的條目：（卡片契約的 key，條目）
    pub entries: Vec<(String, &'a Map<String, Value>)>,
    /// `id` 重複時被後一條蓋掉的條目：（被蓋掉那條的 key，留下那條的 key）
    pub dropped: Vec<(String, String)>,
}

/// `stOrder`：ST 載入世界書是 `Object.keys(data.entries)`。陣列形經 `convertCharacterBook` 以 `entry.id`
/// （缺則陣列索引）當鍵建物件，所以是整數鍵由小到大、其餘照出現順序，id 重複時後面那條蓋掉前面、
/// 位置留在前面那條；物件形就是原物件的鍵順序。
pub fn st_order(source: &SourceEntries) -> StOrdered<'_> {
    let keyed = source.keyed();
    if !source.is_array() {
        let slots = js_key_order(keyed.iter().map(|(key, _)| key.clone()));
        let by_key: HashMap<&str, &Map<String, Value>> = keyed
            .iter()
            .map(|(key, entry)| (key.as_str(), *entry))
            .collect();
        return StOrdered {
            entries: slots
                .into_iter()
                .map(|key| {
                    let entry = by_key[key.as_str()];
                    (key, entry)
                })
                .collect(),
            dropped: Vec::new(),
        };
    }
    // 槽名＝`String(entry.id)`（缺則索引）；同槽後面的蓋前面的，槽的位置留在第一次出現處
    let slot_of = |key: &str, entry: &Map<String, Value>| match entry.get("id") {
        Some(id) => js_string(id),
        None => key.to_owned(),
    };
    let mut slots: Vec<String> = Vec::new();
    let mut holder: HashMap<String, (String, &Map<String, Value>)> = HashMap::new();
    for (key, entry) in &keyed {
        let slot = slot_of(key, entry);
        if holder.insert(slot.clone(), (key.clone(), *entry)).is_none() {
            slots.push(slot);
        }
    }
    let dropped = keyed
        .iter()
        .filter_map(|(key, entry)| {
            let kept = &holder[&slot_of(key, entry)].0;
            (kept != key).then(|| (key.clone(), kept.clone()))
        })
        .collect();
    StOrdered {
        entries: js_key_order(slots)
            .into_iter()
            .map(|slot| holder.remove(&slot).expect("slot filled above"))
            .collect(),
        dropped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(text: &str) -> Vec<String> {
        let source = SourceEntries::parse(text).unwrap().unwrap();
        st_order(&source)
            .entries
            .into_iter()
            .map(|(key, _)| key)
            .collect()
    }

    #[test]
    fn object_form_keeps_file_order_after_integer_keys() {
        assert_eq!(
            keys(r#"{"b":{},"10":{},"a":{},"2":{},"x":5}"#),
            ["2", "10", "b", "a"]
        );
    }

    #[test]
    fn duplicate_object_keys_keep_first_position_last_value() {
        let source = SourceEntries::parse(r#"{"b":{"n":1},"a":{},"b":{"n":2}}"#)
            .unwrap()
            .unwrap();
        let ordered = st_order(&source);
        assert_eq!(ordered.entries[0].0, "b");
        assert_eq!(ordered.entries[0].1["n"], 2);
    }

    #[test]
    fn array_form_orders_by_id_and_drops_duplicates() {
        let source = SourceEntries::parse(
            r#"[{"id":5},{"id":2},{"id":5,"n":1},{},"x",{"id":"b"},{"id":2.0}]"#,
        )
        .unwrap()
        .unwrap();
        let ordered = st_order(&source);
        // 槽 2（索引 1、6）、槽 5（索引 0、2）、"b"（索引 5）、索引 3 缺 id 用索引當槽、索引 4 不是物件
        let kept: Vec<&str> = ordered
            .entries
            .iter()
            .map(|(key, _)| key.as_str())
            .collect();
        assert_eq!(kept, ["6", "3", "2", "5"]);
        assert_eq!(
            ordered.dropped,
            [
                ("0".to_owned(), "2".to_owned()),
                ("1".to_owned(), "6".to_owned())
            ]
        );
    }
}
