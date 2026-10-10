//! 與網頁版對拍：`src/shared/contracts/world-info/` 的案例由網頁版實作跑出預期值，這裡只重跑、比對。
//! 案例與欄位說明見該目錄的 world-info.md；代換、計數、亂數的規則與網頁版 `world-info-parity-runner.ts` 相同。

use serde_json::{json, Map, Value};

use super::entry::{from_character_book, from_world_file, WiEntry};
use super::regex_key::{parse_regex_from_string, regex_test};
use super::scan::{check_world_info, GlobalScan, ScanField, ScanHooks, ScanInput, Substituted};
use super::settings::WiSettings;
use super::sort::{compare_order, sort_entries, v8_sort};
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
        let raw: &Map<String, Value> = case["raw"].as_object().expect("raw");
        let entry = match case["form"].as_str() {
            Some("worldFile") => from_world_file(raw),
            _ => from_character_book(raw),
        };
        let actual = serde_json::to_value(&entry).expect("entry");
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
                entry.order = item.get("order").cloned();
                (index, entry)
            })
            .collect();
        v8_sort(
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
