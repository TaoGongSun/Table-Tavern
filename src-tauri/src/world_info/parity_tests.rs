//! 與網頁版對拍：`src/shared/contracts/world-info/` 的案例由網頁版實作跑出預期值，這裡只重跑、比對。
//! 案例與欄位說明見該目錄的 world-info.md；代換、計數、亂數的規則與網頁版 `world-info-parity-runner.ts` 相同。

use serde_json::{json, Map, Value};

use super::book_order::{st_order, SourceEntries};
use super::entry::{
    character_book_to_world_object, from_character_book, from_world_file, WiEntry, EXTENSION_FIELDS,
};
use super::regex_key::{parse_regex_from_string, regex_test};
use super::scan::{check_world_info, GlobalScan, ScanField, ScanHooks, ScanInput, Substituted};
use super::settings::WiSettings;
use super::sort::{compare_order, sort_entries, stable_sort};
use super::timed::WiTimed;

const SCAN_CASES: &str = include_str!("../../../src/shared/contracts/world-info/scan-cases.json");
const REGEX_CASES: &str = include_str!("../../../src/shared/contracts/world-info/regex-cases.json");
const ENTRY_CASES: &str = include_str!("../../../src/shared/contracts/world-info/entry-cases.json");
const SORT_CASES: &str = include_str!("../../../src/shared/contracts/world-info/sort-cases.json");

fn cases(text: &str) -> Vec<Value> {
    let body: Value = serde_json::from_str(text).expect("fixture JSON");
    body["cases"].as_array().expect("cases").clone()
}

/// JSON 相等，數字以數值比（網頁版的 3 與 Rust 的 3.0 相同）。陣列比順序；物件不看鍵順序
/// （Rust 讀回 JSON 物件時本來就不保序，要比順序的結構——outlets——在兩邊都寫成陣列）。
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(x, y)| same(x, y))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(key, x)| y.get(key).is_some_and(|y| same(x, y)))
        }
        _ => a == b,
    }
}

struct Recording {
    calls: Vec<String>,
    random: Vec<f64>,
    used: usize,
}

impl ScanHooks for Recording {
    fn substitute(&mut self, text: &str) -> Substituted {
        self.calls.push(text.to_owned());
        Substituted {
            text: text.replace("{{user}}", "Alice").replace("{{char}}", "Bob"),
            private: false,
        }
    }

    fn count_tokens(&mut self, text: &str) -> f64 {
        text.chars().count() as f64
    }

    fn random(&mut self) -> f64 {
        let value = *self
            .random
            .get(self.used)
            .expect("random sequence exhausted");
        self.used += 1;
        value
    }
}

fn field(scan: &Value, name: &str) -> ScanField {
    ScanField::public(scan.get(name).and_then(Value::as_str).unwrap_or(""))
}

fn run_scan_case(case: &Value) -> Value {
    let mut entries: Vec<WiEntry> = case["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .map(|item| {
            let mut entry = from_world_file(item["raw"].as_object().expect("raw"));
            entry.id = item["id"].as_str().expect("id").to_owned();
            entry
        })
        .collect();
    sort_entries(&mut entries);
    let chat: Vec<String> = serde_json::from_value(case["chat"].clone()).expect("chat");
    let settings: WiSettings = serde_json::from_value(case["settings"].clone()).expect("settings");
    let timed: WiTimed = case
        .get("timed")
        .map(|timed| serde_json::from_value(timed.clone()).expect("timed"))
        .unwrap_or_default();
    let scan = case.get("globalScan").cloned().unwrap_or(json!({}));
    let global = GlobalScan {
        persona_description: field(&scan, "personaDescription"),
        character_description: field(&scan, "characterDescription"),
        character_personality: field(&scan, "characterPersonality"),
        character_depth_prompt: field(&scan, "characterDepthPrompt"),
        scenario: field(&scan, "scenario"),
        creator_notes: field(&scan, "creatorNotes"),
    };
    let mut hooks = Recording {
        calls: Vec::new(),
        random: serde_json::from_value(case.get("random").cloned().unwrap_or(json!([])))
            .expect("random"),
        used: 0,
    };
    let result = check_world_info(
        &entries,
        ScanInput {
            chat: &chat,
            max_context: case["maxContext"].as_f64().unwrap_or(f64::INFINITY),
            global_scan: &global,
            trigger: case
                .get("trigger")
                .and_then(Value::as_str)
                .unwrap_or("normal"),
            timed,
            settings: &settings,
            pinned: &std::collections::BTreeSet::new(),
        },
        &mut hooks,
    );
    let mut value = serde_json::to_value(&result).expect("result");
    let object = value.as_object_mut().expect("object");
    object.insert("substituteCalls".to_owned(), json!(hooks.calls));
    object.insert("randomCalls".to_owned(), json!(hooks.used));
    value
}

#[test]
fn scan_cases_match_the_web_version() {
    let mut failures = Vec::new();
    for case in cases(SCAN_CASES) {
        let actual = run_scan_case(&case);
        if !same(&actual, &case["expected"]) {
            failures.push(format!(
                "{}\n  expected {}\n  actual   {}",
                case["name"], case["expected"], actual
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn regex_cases_match_the_web_version() {
    let mut failures = Vec::new();
    for case in cases(REGEX_CASES) {
        let key = case["key"].as_str().expect("key");
        let haystack = case["haystack"].as_str().expect("haystack");
        let actual = match parse_regex_from_string(key) {
            Some(regex) => json!({ "parsed": true, "matches": regex_test(&regex, haystack) }),
            None => json!({ "parsed": false, "matches": null }),
        };
        // 刻意與 JS 不同的案例（方案七）比對 knownDifference
        let expected = case
            .get("knownDifference")
            .map(|known| json!({ "parsed": known["parsed"], "matches": known["matches"] }))
            .unwrap_or_else(|| case["expected"].clone());
        if !same(&actual, &expected) {
            failures.push(format!(
                "{} {key}: expected {expected}, actual {actual}",
                case["name"]
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn entry_cases_match_the_web_version() {
    let mut failures = Vec::new();
    for case in cases(ENTRY_CASES) {
        let actual = match case["form"].as_str() {
            Some("bookOrder") => {
                let source = SourceEntries::parse(case["entries"].as_str().expect("entries"))
                    .expect("entries JSON")
                    .expect("array or object");
                json!(st_order(&source)
                    .entries
                    .into_iter()
                    .map(|(key, _)| key)
                    .collect::<Vec<_>>())
            }
            form => {
                let raw: &Map<String, Value> = case["raw"].as_object().expect("raw");
                let entry = match form {
                    Some("worldFile") => from_world_file(raw),
                    _ => from_character_book(raw),
                };
                serde_json::to_value(&entry).expect("entry")
            }
        };
        if !same(&actual, &case["expected"]) {
            failures.push(format!(
                "{}\n  expected {}\n  actual   {}",
                case["name"], case["expected"], actual
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// 角色卡路匯入落檔（轉成物件形）後再讀出，要得到跟網頁版直接讀 V2 條目同一個 `WiEntry`——
/// 含缺 `insertion_order`（補 100）、缺 `selective`（V2 是 false）、缺 `enabled`（啟用）。
#[test]
fn character_book_entries_read_back_the_same_after_conversion() {
    let mut failures = Vec::new();
    let mut checked = 0;
    for case in cases(ENTRY_CASES) {
        if case["form"] != "characterBook" {
            continue;
        }
        let raw = case["raw"].as_object().expect("raw");
        let stored = character_book_to_world_object(raw);
        // 落檔再讀：經過 JSON 文字，確認 `order` 缺鍵與旗標都能留在檔案裡
        let stored: Map<String, Value> =
            serde_json::from_str(&Value::Object(stored).to_string()).expect("stored JSON");
        let read_back = from_world_file(&stored);
        let direct = from_character_book(raw);
        checked += 1;
        if read_back != direct {
            failures.push(format!(
                "{}\n  direct   {:?}\n  readback {:?}",
                case["name"], direct, read_back
            ));
        }
    }
    assert!(checked >= 10, "{checked}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// 轉換表與反向表同一份：V2 欄位轉成物件形、再收回 `extensions` 的 snake_case 後，用 V2 讀法讀出的
/// `WiEntry` 不變（匯出給 ST 再匯入不丟欄位）。
#[test]
fn extension_fields_round_trip_through_the_object_form() {
    let mut checked = 0;
    for case in cases(ENTRY_CASES) {
        if case["form"] != "characterBook" {
            continue;
        }
        let raw = case["raw"].as_object().expect("raw");
        let mut object = character_book_to_world_object(raw);
        let mut extensions = object
            .remove("extensions")
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        for (field, snake) in EXTENSION_FIELDS {
            if let Some(value) = object.remove(field) {
                extensions.insert(snake.to_owned(), value);
            }
        }
        extensions.insert("position".to_owned(), object["position"].clone());
        if let Some(value) = object.remove("caseSensitive") {
            extensions.insert("case_sensitive".to_owned(), value);
        }
        let mut v2 = Map::new();
        v2.insert("keys".to_owned(), object["key"].clone());
        v2.insert("secondary_keys".to_owned(), object["keysecondary"].clone());
        for field in ["comment", "content", "constant", "selective"] {
            v2.insert(field.to_owned(), object[field].clone());
        }
        v2.insert(
            "enabled".to_owned(),
            json!(object["disable"] == json!(false)),
        );
        if let Some(order) = object.get("order") {
            v2.insert("insertion_order".to_owned(), order.clone());
        }
        v2.insert("extensions".to_owned(), Value::Object(extensions));
        let expected = from_character_book(raw);
        let mut actual = from_character_book(&v2);
        actual.id = expected.id.clone();
        assert_eq!(actual, expected, "{}", case["name"]);
        checked += 1;
    }
    assert!(checked >= 10, "{checked}");
}

/// sort-cases 的條目經「V2 → 物件形落檔 → 讀回」後排序結果不變。
#[test]
fn sort_cases_survive_the_v2_conversion() {
    for case in cases(SORT_CASES) {
        let mut entries: Vec<(usize, WiEntry)> = case["items"]
            .as_array()
            .expect("items")
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let mut v2 = Map::new();
                v2.insert("insertion_order".to_owned(), item["order"].clone());
                let stored = character_book_to_world_object(&v2);
                let stored: Map<String, Value> =
                    serde_json::from_str(&Value::Object(stored).to_string()).expect("stored");
                (index, from_world_file(&stored))
            })
            .collect();
        stable_sort(
            &mut entries,
            &|a: &(usize, WiEntry), b: &(usize, WiEntry)| compare_order(&a.1, &b.1),
        );
        let actual = json!(entries.iter().map(|(index, _)| *index).collect::<Vec<_>>());
        assert!(same(&actual, &case["expected"]), "{}", case["name"]);
    }
}

#[test]
fn sort_cases_match_v8() {
    let mut failures = Vec::new();
    for case in cases(SORT_CASES) {
        let mut entries: Vec<(usize, WiEntry)> = case["items"]
            .as_array()
            .expect("items")
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let mut entry = from_world_file(&Map::new());
                entry.order = item["order"].as_f64().expect("order");
                (index, entry)
            })
            .collect();
        stable_sort(
            &mut entries,
            &|a: &(usize, WiEntry), b: &(usize, WiEntry)| compare_order(&a.1, &b.1),
        );
        let actual = json!(entries.iter().map(|(index, _)| *index).collect::<Vec<_>>());
        if !same(&actual, &case["expected"]) {
            failures.push(format!(
                "{}\n  expected {}\n  actual   {}",
                case["name"], case["expected"], actual
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
