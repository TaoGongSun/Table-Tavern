//! 桌面版限定（網頁版沒有對應，不進 fixture）：完整／中性兩種模式、操作序列、讀到私密來源。

use std::cell::RefCell;
use std::collections::BTreeMap;

use super::engine::{Limits, SourceText};
use super::js_value::{JsValue, MAX_TEXT_BYTES, TEXT_LIMIT};
use super::moment::FixedClock;
use super::substitute::{
    base_chat_replace, substitute_params, CardText, MacroContext, Mode, SubstituteOptions,
};
use super::variables::{ScopeKind, VarOp, VarOpKind, VarScope, Variables};

struct Fixture {
    card: CardText,
    variables: RefCell<Variables>,
    outlets: BTreeMap<String, SourceText>,
    clock: FixedClock,
}

impl Fixture {
    fn new() -> Self {
        Self {
            card: CardText {
                name: "莫拉".to_owned(),
                description: SourceText::private("私設：{{user}} 欠債"),
                personality: SourceText::public("愛笑"),
                ..CardText::default()
            },
            variables: RefCell::new(Variables::new(
                VarScope::from_json_text(r#"{"hp":"10"}"#).unwrap(),
                VarScope::default(),
            )),
            outlets: BTreeMap::from([
                ("公開".to_owned(), SourceText::public("門")),
                ("秘密".to_owned(), SourceText::private("密道")),
            ]),
            clock: FixedClock {
                now_ms: 0.0,
                offset_minutes: 0.0,
            },
        }
    }

    fn run(&self, text: &str, mode: Mode) -> (String, bool) {
        let random = RefCell::new(|| 0.0);
        let context = MacroContext {
            card: &self.card,
            user_name: "旅人",
            chat: &[],
            variables: &self.variables,
            chat_id: "table",
            input: "",
            generation_type: "normal",
            model: "",
            limits: Limits::default(),
            clock: &self.clock,
            random: &random,
            outlets: &self.outlets,
            is_mobile: false,
        };
        let result = substitute_params(
            text,
            &context,
            SubstituteOptions {
                mode,
                ..SubstituteOptions::default()
            },
        );
        (result.text, result.private)
    }
}

const SAMPLE: &str =
    "{{setvar::x::1}}{{getvar::x}}|{{.hp-=3}}{{.hp}}|{{setglobalvar::g::甲}}{{getglobalvar::g}}";

#[test]
fn neutral_mode_matches_full_output_but_leaves_variables_alone() {
    let neutral = Fixture::new();
    let full = Fixture::new();
    let (neutral_text, _) = neutral.run(SAMPLE, Mode::Neutral);
    let (full_text, _) = full.run(SAMPLE, Mode::Full);
    assert_eq!(neutral_text, "1|7|甲");
    assert_eq!(neutral_text, full_text);
    let untouched = neutral.variables.borrow();
    assert_eq!(untouched.local.to_json_text(), r#"{"hp":"10"}"#);
    assert_eq!(untouched.global.to_json_text(), "{}");
    assert!(untouched.ops.is_empty());
}

#[test]
fn full_mode_records_operations_in_order_and_replays_to_the_same_table() {
    let fixture = Fixture::new();
    fixture.run(SAMPLE, Mode::Full);
    let written = fixture.variables.borrow();
    assert_eq!(
        written.ops,
        vec![
            VarOp {
                scope: ScopeKind::Local,
                name: "x".to_owned(),
                kind: VarOpKind::Set {
                    value: JsValue::str("1"),
                    index: None,
                },
            },
            VarOp {
                scope: ScopeKind::Local,
                name: "hp".to_owned(),
                kind: VarOpKind::Add {
                    value: JsValue::Num(-3.0),
                },
            },
            VarOp {
                scope: ScopeKind::Global,
                name: "g".to_owned(),
                kind: VarOpKind::Set {
                    value: JsValue::str("甲"),
                    index: None,
                },
            },
        ]
    );
    let mut replayed = Variables::new(
        VarScope::from_json_text(r#"{"hp":"10"}"#).unwrap(),
        VarScope::default(),
    );
    replayed.replay(&written.ops);
    assert_eq!(replayed.local, written.local);
    assert_eq!(replayed.global, written.global);
}

#[test]
fn reports_reading_private_sources() {
    let fixture = Fixture::new();
    assert_eq!(
        fixture.run("{{personality}}{{outlet::公開}}", Mode::Full),
        ("愛笑門".to_owned(), false)
    );
    assert_eq!(
        fixture.run("{{description}}", Mode::Full),
        ("私設：旅人 欠債".to_owned(), true)
    );
    assert!(fixture.run("{{outlet::秘密}}", Mode::Neutral).1);
    assert!(fixture.run("{{charDescription}}", Mode::Neutral).1);
    // 卡欄位裡引用私密 outlet：欄位本身公開也算讀到私密
    let mut nested = Fixture::new();
    nested.card.scenario = SourceText::public("場景{{outlet::秘密}}");
    assert_eq!(
        nested.run("{{scenario}}", Mode::Full),
        ("場景密道".to_owned(), true)
    );
    // 只寫進註解、沒進輸出也算讀到（保守）
    assert!(fixture.run("{{// {{description}} }}", Mode::Full).1);
    // 變數不追蹤私密（作者裁決）
    fixture.run("{{setvar::s::{{description}}}}", Mode::Full);
    assert_eq!(
        fixture.run("{{getvar::s}}", Mode::Full),
        ("私設：旅人 欠債".to_owned(), false)
    );
}

#[test]
fn private_reads_count_even_when_not_selected() {
    // 替代開場白一取就整串代換：第二則讀了私密 outlet，取第一則也算讀到
    let mut greetings = Fixture::new();
    greetings.card.alternate_greetings = vec![
        SourceText::public("safe"),
        SourceText::public("{{setvar::s::{{outlet::秘密}}}}"),
    ];
    for mode in [Mode::Full, Mode::Neutral] {
        assert_eq!(
            greetings.run("{{greeting::1}}{{getvar::s}}", mode),
            ("safe密道".to_owned(), true)
        );
    }
    // if 條件讀到私密要回報；沒走到的分支不回報
    let fixture = Fixture::new();
    assert_eq!(
        fixture.run("{{if {{outlet::秘密}}}}有{{else}}無{{/if}}", Mode::Full),
        ("有".to_owned(), true)
    );
    assert_eq!(
        fixture.run("{{if 0}}{{outlet::秘密}}{{else}}無{{/if}}", Mode::Full),
        ("無".to_owned(), false)
    );
}

#[test]
fn practical_limits_are_lower_than_js() {
    let fixture = Fixture::new();
    let (text, _) = fixture.run("{{setvarkey::a::10000000::x}}{{hasvar::a}}", Mode::Full);
    assert_eq!(text, "false");
    assert_eq!(
        fixture.run("[{{space::1000001}}]", Mode::Full).0,
        "[{{space::1000001}}]"
    );
    assert_eq!(
        fixture.run("[{{space::1000000}}]", Mode::Full).0.len(),
        1_000_002
    );
}

/// 縮小字串上限跑一段測試（上限是每條執行緒各自一份）。
fn with_text_limit<T>(limit: usize, run: impl FnOnce() -> T) -> T {
    TEXT_LIMIT.with(|cell| cell.set(limit));
    let result = run();
    TEXT_LIMIT.with(|cell| cell.set(MAX_TEXT_BYTES));
    result
}

#[test]
fn trailing_text_over_the_limit_returns_the_input() {
    let fixture = Fixture::new();
    let input = "{{space::60}}abcdefghij";
    assert_eq!(
        with_text_limit(70, || fixture.run(input, Mode::Full).0),
        format!("{}abcdefghij", " ".repeat(60))
    );
    assert_eq!(
        with_text_limit(65, || fixture.run(input, Mode::Full).0),
        input
    );
}

#[test]
fn doubling_a_variable_past_the_limit_throws() {
    let fixture = Fixture::new();
    let doubling = "{{addvar::a::{{getvar::a}}}}".repeat(10);
    let output = with_text_limit(64, || {
        fixture
            .run(&format!("{{{{setvar::a::x}}}}{doubling}"), Mode::Full)
            .0
    });
    // 32 → 64 還寫得進去，再加倍超過上限：巨集原樣留著，變數停在 64 字
    assert!(output.contains("{{addvar::a::"), "{output}");
    let written = fixture.variables.borrow();
    assert_eq!(written.local.raw("a"), Some(&JsValue::str("x".repeat(64))));
}

#[test]
fn base_chat_replace_skips_card_fields_and_drops_cr() {
    let fixture = Fixture::new();
    let random = RefCell::new(|| 0.0);
    let context = MacroContext {
        card: &fixture.card,
        user_name: "旅人",
        chat: &[],
        variables: &fixture.variables,
        chat_id: "table",
        input: "",
        generation_type: "normal",
        model: "",
        limits: Limits::default(),
        clock: &fixture.clock,
        random: &random,
        outlets: &fixture.outlets,
        is_mobile: false,
    };
    let result = base_chat_replace("[{{description}}]{{user}}\r\n", &context, Mode::Neutral);
    assert_eq!(result.text, "[]旅人\n");
    assert!(!result.private);
}

#[test]
fn pathological_nesting_gives_up_and_returns_the_input() {
    let fixture = Fixture::new();
    let deep = format!("{}x{}", "{{reverse::".repeat(400), "}}".repeat(400));
    assert_eq!(fixture.run(&deep, Mode::Full).0, deep);
}
