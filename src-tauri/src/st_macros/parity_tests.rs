//! 與網頁版對拍：`src/shared/contracts/st-macros/` 的 `st-macro-cases.json`（ST 官方案例）與
//! `web-macro-cases.json`（網頁版實作跑出的預期值）。這裡只重跑、比對；欄位說明見該目錄的 st-macros.md。

use std::cell::RefCell;
use std::collections::BTreeMap;

use serde_json::Value;

use super::engine::{ChatLine, Limits, SourceText};
use super::js_value::JsValue;
use super::library::LIBRARY;
use super::moment::FixedClock;
use super::substitute::{substitute_params, CardText, MacroContext, SubstituteOptions};
use super::variables::{VarScope, Variables};

const ST_CASES: &str = include_str!("../../../src/shared/contracts/st-macros/st-macro-cases.json");
const WEB_CASES: &str =
    include_str!("../../../src/shared/contracts/st-macros/web-macro-cases.json");
const NAMES: &str = include_str!("../../../src/shared/contracts/st-macros/st-macros.json");

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn card_from(base: &Value, over: Option<&Value>) -> CardText {
    let pick = |key: &str| {
        over.and_then(|over| over.get(key))
            .unwrap_or_else(|| &base[key])
            .clone()
    };
    let field = |key: &str| SourceText::public(pick(key).as_str().unwrap_or_default());
    CardText {
        name: pick("name").as_str().unwrap_or_default().to_owned(),
        description: field("description"),
        personality: field("personality"),
        scenario: field("scenario"),
        first_mes: field("first_mes"),
        mes_example: field("mes_example"),
        creator_notes: field("creator_notes"),
        system_prompt: field("system_prompt"),
        post_history_instructions: field("post_history_instructions"),
        alternate_greetings: pick("alternate_greetings")
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|item| SourceText::public(item.as_str().unwrap_or_default()))
                    .collect()
            })
            .unwrap_or_default(),
        character_version: pick("character_version")
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        depth_prompt: field("depth_prompt"),
        // 網頁版沒有人設（{{persona}} 是空字串）
        persona: SourceText::default(),
    }
}

/// 變數表要保住鍵順序：從同一份案例檔以保序的 [`JsValue`] 取（serde_json 的 `Value` 會把鍵排序）。
fn scope(ordered: &JsValue, case_index: usize, path: &[&str]) -> VarScope {
    let mut value = ordered.property("cases").property(&case_index.to_string());
    for key in path {
        value = value.property(key);
    }
    match value {
        JsValue::Undefined => VarScope::default(),
        other => VarScope::from_json_text(&other.stringify().expect("JSON")).expect("變數表"),
    }
}

fn chat_from(value: &Value) -> Vec<ChatLine> {
    value
        .as_array()
        .expect("chat")
        .iter()
        .map(|line| ChatLine {
            is_user: line["isUser"].as_bool().unwrap_or(false),
            text: text(line, "text"),
            sent_at: line.get("sentAt").and_then(Value::as_f64),
        })
        .collect()
}

struct Outcome {
    output: String,
    local: String,
    global: String,
    random_used: usize,
}

fn run_web_case(fixture: &Value, ordered: &JsValue, index: usize, case: &Value) -> Outcome {
    let empty = Value::Null;
    let ctx = case.get("context").unwrap_or(&empty);
    let options = case.get("options").unwrap_or(&empty);
    let card = card_from(&fixture["defaultCard"], ctx.get("card"));
    let chat = chat_from(ctx.get("chat").unwrap_or(&fixture["defaultChat"]));
    let variables = RefCell::new(Variables::new(
        scope(ordered, index, &["context", "variables", "local"]),
        scope(ordered, index, &["context", "variables", "global"]),
    ));
    let sequence: Vec<f64> = ctx
        .get("random")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default();
    let used = std::cell::Cell::new(0usize);
    let random = RefCell::new(|| {
        let value = sequence.get(used.get()).copied().unwrap_or(0.0);
        used.set(used.get() + 1);
        value
    });
    let clock = FixedClock {
        now_ms: ctx
            .get("nowMs")
            .and_then(Value::as_f64)
            .unwrap_or_else(|| fixture["nowMs"].as_f64().expect("nowMs")),
        offset_minutes: fixture["utcOffsetMinutes"]
            .as_f64()
            .expect("utcOffsetMinutes"),
    };
    let outlets: BTreeMap<String, SourceText> = ctx
        .get("outlets")
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .map(|(name, value)| {
                    (
                        name.clone(),
                        SourceText::public(value.as_str().unwrap_or_default()),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let limits = ctx
        .get("limits")
        .map_or(Limits::default(), |limits| Limits {
            max_context: limits["maxContext"].as_f64().unwrap_or(0.0),
            max_response: limits["maxResponse"].as_f64().unwrap_or(0.0),
        });
    let user_name = ctx
        .get("userName")
        .and_then(Value::as_str)
        .unwrap_or("旅人");
    let chat_id = ctx
        .get("chatId")
        .and_then(Value::as_str)
        .unwrap_or("chat-1");
    let generation_type = ctx
        .get("generationType")
        .and_then(Value::as_str)
        .unwrap_or("normal");
    let context = MacroContext {
        card: &card,
        prepared: None,
        user_name,
        chat: &chat,
        variables: &variables,
        chat_id,
        input: ctx.get("input").and_then(Value::as_str).unwrap_or(""),
        generation_type,
        model: ctx.get("model").and_then(Value::as_str).unwrap_or(""),
        limits,
        clock: &clock,
        random: &random,
        outlets: &outlets,
        outlet_reads: None,
        is_mobile: false,
    };
    let original = options
        .get("original")
        .and_then(Value::as_str)
        .map(SourceText::public);
    let result = substitute_params(
        case["input"].as_str().expect("input"),
        &context,
        SubstituteOptions {
            original: original.as_ref(),
            replace_character_card: options
                .get("replaceCharacterCard")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            ..SubstituteOptions::default()
        },
    );
    let variables = variables.borrow();
    Outcome {
        output: result.text,
        local: variables.local.to_json_text(),
        global: variables.global.to_json_text(),
        random_used: used.get(),
    }
}

#[test]
fn web_macro_cases_match() {
    let fixture: Value = serde_json::from_str(WEB_CASES).expect("fixture JSON");
    let ordered = JsValue::parse_json(WEB_CASES).expect("fixture JSON");
    let mut failures = Vec::new();
    for (index, case) in fixture["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .enumerate()
    {
        let name = text(case, "name");
        let expected = &case["expected"];
        let outcome = run_web_case(&fixture, &ordered, index, case);
        let checks = [
            ("output", outcome.output, text(expected, "output")),
            ("local", outcome.local, text(expected, "local")),
            ("global", outcome.global, text(expected, "global")),
            (
                "randomUsed",
                outcome.random_used.to_string(),
                expected["randomUsed"].to_string(),
            ),
        ];
        for (what, actual, wanted) in checks {
            if actual != wanted {
                failures.push(format!(
                    "{name} [{what}]\n  實際 {actual:?}\n  預期 {wanted:?}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn st_macro_cases_match() {
    let fixture: Value = serde_json::from_str(ST_CASES).expect("fixture JSON");
    let ordered = JsValue::parse_json(ST_CASES).expect("fixture JSON");
    let card = CardText {
        name: "Character".to_owned(),
        ..CardText::default()
    };
    let clock = FixedClock {
        now_ms: 0.0,
        offset_minutes: 0.0,
    };
    let outlets = BTreeMap::new();
    let mut failures = Vec::new();
    for (index, case) in fixture["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .enumerate()
    {
        let variables = RefCell::new(Variables::new(
            scope(&ordered, index, &["variables", "local"]),
            scope(&ordered, index, &["variables", "global"]),
        ));
        let random = RefCell::new(|| 0.0);
        let context = MacroContext {
            card: &card,
            prepared: None,
            user_name: "User",
            chat: &[],
            variables: &variables,
            chat_id: "c",
            input: "",
            generation_type: "normal",
            model: "",
            limits: Limits::default(),
            clock: &clock,
            random: &random,
            outlets: &outlets,
            outlet_reads: None,
            is_mobile: false,
        };
        let result = substitute_params(
            case["input"].as_str().expect("input"),
            &context,
            SubstituteOptions {
                replace_character_card: false,
                ..SubstituteOptions::default()
            },
        );
        let wanted = text(case, "expected");
        if result.text != wanted {
            failures.push(format!(
                "{}\n  實際 {:?}\n  預期 {wanted:?}",
                text(case, "name"),
                result.text
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn contract_names_resolve_to_built_ins() {
    let names: Value = serde_json::from_str(NAMES).expect("st-macros.json");
    let missing: Vec<String> = ["names", "argument_names"]
        .iter()
        .flat_map(|key| names[*key].as_array().expect("名單").iter())
        .filter_map(Value::as_str)
        .filter(|name| *name != "charjailbreak" && LIBRARY.get(name).is_none())
        .map(str::to_owned)
        .collect();
    assert!(missing.is_empty(), "{missing:?}");
}
