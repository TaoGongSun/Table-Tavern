//! 巨集×世界書掃描與網頁版對拍（方案四之 1 `integration-cases`）：同一份案例在桌面版照實送的先後跑
//! （擁有文字第一輪 → 掃描 → `prepare`），比中間產物、跑完的變數表與變數寫入序列。案例由網頁版跑出
//! （`web/src/features/chat/integration-parity-runner.ts`），這裡只重跑、比對。
use super::tests::event;
use super::*;
use crate::data::{Visibility, WorldbookEntry};
use crate::st_macros::js_value::JsValue;
use crate::st_macros::variables::{ScopeKind, VarOp, VarOpKind, VarScope, Variables};
use serde_json::{Map, Value};

const CASES: &str = include_str!("../../../src/shared/contracts/world-info/integration-cases.json");

/// 變數表要保住鍵順序：從保序的 [`JsValue`] 取。
fn scope(ordered: &JsValue, index: usize, key: &str) -> VarScope {
    match ordered
        .property("cases")
        .property(&index.to_string())
        .property("variables")
        .property(key)
    {
        JsValue::Undefined => VarScope::default(),
        other => VarScope::from_json_text(&other.stringify().expect("JSON")).expect("變數表"),
    }
}

/// 網頁版 runner 的寫入序列格式：`<local|global>:set:<名稱>[<index>]=<值>`、`…:add:<名稱>=<加數>`、`…:del:<名稱>`，
/// 值是 `JSON.stringify` 原文。
fn op_label(op: &VarOp) -> String {
    let scope = match op.scope {
        ScopeKind::Local => "local",
        ScopeKind::Global => "global",
    };
    let json = |value: &JsValue| value.stringify().unwrap_or_else(|| "undefined".to_owned());
    match &op.kind {
        VarOpKind::Set { value, index } => {
            let index = index
                .as_ref()
                .map(|index| format!("[{index}]"))
                .unwrap_or_default();
            format!("{scope}:set:{}{index}={}", op.name, json(value))
        }
        VarOpKind::Add { value } => format!("{scope}:add:{}={}", op.name, json(value)),
        VarOpKind::Delete => format!("{scope}:del:{}", op.name),
    }
}

fn text(value: &Value) -> String {
    value.as_str().expect("string").to_owned()
}

#[test]
fn integration_cases_match_the_web_version() {
    let parsed: Value = serde_json::from_str(CASES).expect("fixture JSON");
    let ordered = JsValue::parse_json(CASES).expect("fixture JSON");
    let cases = parsed["cases"].as_array().expect("cases");
    assert!(!cases.is_empty());
    for (index, case) in cases.iter().enumerate() {
        let name = text(&case["name"]);
        let user = text(&case["userName"]);
        let card = super::tests::card(
            "card",
            case["card"]["name"].as_str().unwrap(),
            case["card"]["description"].as_str().unwrap(),
            "",
        );
        let player = super::tests::card("player", &user, "", "");
        let events: Vec<TranscriptEvent> = case["chat"]
            .as_array()
            .unwrap()
            .iter()
            .map(|line| {
                let is_user = line["isUser"].as_bool().unwrap_or(false);
                let mut event = event(
                    match is_user {
                        true => crate::data::TranscriptKind::Player,
                        false => crate::data::TranscriptKind::Dialogue,
                    },
                    if is_user { &user } else { &card.name },
                    line["text"].as_str().unwrap(),
                );
                event.ts = String::new();
                event
            })
            .collect();
        let entries: Vec<(WorldbookEntry, Map<String, Value>)> = case["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| {
                let uid: u64 = entry["id"].as_str().unwrap().parse().unwrap();
                let Value::Object(mut raw) = entry["raw"].clone() else {
                    panic!("raw")
                };
                raw.insert("uid".to_owned(), uid.into());
                let view = WorldbookEntry {
                    uid,
                    title: raw["comment"].as_str().unwrap_or_default().to_owned(),
                    keys: Vec::new(),
                    content: String::new(),
                    constant: raw
                        .get("constant")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    order: 100,
                    disabled: false,
                    visibility: Visibility::Public,
                    is_person: false,
                    locked: false,
                };
                (view, raw)
            })
            .collect();
        let book = TableBook::from_raw(entries);
        let prev_outlets = case["prevOutlets"]
            .as_object()
            .map(|outlets| {
                outlets
                    .iter()
                    .map(|(name, value)| (name.clone(), SourceText::public(text(value))))
                    .collect()
            })
            .unwrap_or_default();
        let viewer = Viewer::Character(&card);
        let session = MacroSession::new(
            MacroInputs {
                variables: Variables::new(
                    scope(&ordered, index, "local"),
                    scope(&ordered, index, "global"),
                ),
                prev_outlets,
                ..MacroInputs::empty()
            },
            &viewer,
            "",
            None,
            Some(&player),
            &events,
            super::tests::LANG,
        );
        let mut scanned = scan(ScanRequest {
            book: &book,
            viewer,
            sole_card: None,
            player: Some(&player),
            events: &events,
            lang: super::tests::LANG,
            timed: Default::default(),
            budget: None,
            random: Randomness::Measure,
            session: &session,
        });
        let mut first: Vec<(String, String)> = scanned
            .placed
            .iter()
            .map(|entry| (entry.uid.to_string(), entry.content.clone()))
            .collect();
        first.sort();
        prepare(
            &mut scanned,
            &session,
            &book,
            &viewer,
            std::slice::from_ref(&card),
            Some(&player),
        );
        let placed: Vec<&Placed> = scanned.placed.iter().collect();
        let arranged = arrange(&placed);
        let join = |group: &[&Placed]| {
            group
                .iter()
                .map(|entry| entry.content.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        };
        let injections: Vec<String> = arranged
            .injected
            .iter()
            .map(|injection| trimmed(&injection.text).to_owned())
            .filter(|text| !text.is_empty())
            .collect();
        let outlets: Vec<(String, String)> = scanned
            .outlets
            .iter()
            .map(|(name, outlet)| (name.clone(), outlet.text.clone()))
            .collect();
        let variables = session.variables();
        let expected = &case["expected"];
        let pairs = |value: &Value| -> Vec<(String, String)> {
            value
                .as_array()
                .unwrap()
                .iter()
                .map(|pair| (text(&pair[0]), text(&pair[1])))
                .collect()
        };
        assert_eq!(first, pairs(&expected["scanned"]), "{name}: scanned");
        assert_eq!(
            join(&arranged.front),
            text(&expected["before"]),
            "{name}: before"
        );
        assert_eq!(
            join(&arranged.back),
            text(&expected["after"]),
            "{name}: after"
        );
        let wanted: Vec<String> = expected["injections"]
            .as_array()
            .unwrap()
            .iter()
            .map(text)
            .collect();
        assert_eq!(injections, wanted, "{name}: injections");
        assert_eq!(outlets, pairs(&expected["outlets"]), "{name}: outlets");
        assert_eq!(
            variables.local.to_json_text(),
            text(&expected["local"]),
            "{name}: local"
        );
        assert_eq!(
            variables.global.to_json_text(),
            text(&expected["global"]),
            "{name}: global"
        );
        let ops: Vec<String> = scanned.var_ops.iter().map(op_label).collect();
        let wanted: Vec<String> = expected["ops"]
            .as_array()
            .unwrap()
            .iter()
            .map(text)
            .collect();
        assert_eq!(ops, wanted, "{name}: ops");
    }
}
