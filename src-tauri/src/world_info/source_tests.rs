//! 觸發來源（公開／私密）：網頁版沒有這個概念，這裡只有 Rust 單元測試。
//! 兩件事：分類正確；加了來源追蹤，觸發集合與代換呼叫和沒有私密片段時逐條相同。

use serde_json::{json, Value};

use super::entry::{from_world_file, WiEntry};
use super::scan::{
    check_world_info, GlobalScan, ScanField, ScanHooks, ScanInput, Substituted, WiResult,
};
use super::settings::ST_WI_SETTINGS;
use super::sort::sort_entries;
use super::timed::{TimedEffect, WiTimed};

#[derive(Default)]
struct Hooks {
    calls: usize,
    /// 代換結果算私密的輸入
    private_inputs: Vec<&'static str>,
}

impl ScanHooks for Hooks {
    fn substitute(&mut self, text: &str) -> Substituted {
        self.calls += 1;
        Substituted {
            text: text.to_owned(),
            private: self.private_inputs.contains(&text),
        }
    }

    fn count_tokens(&mut self, text: &str) -> f64 {
        text.chars().count() as f64
    }

    fn random(&mut self) -> f64 {
        0.0
    }
}

fn entry(id: &str, raw: Value, limited: bool) -> WiEntry {
    let mut raw = raw;
    raw.as_object_mut()
        .expect("object")
        .entry("content")
        .or_insert(json!(id));
    let mut entry = from_world_file(raw.as_object().expect("object"));
    entry.id = id.to_owned();
    entry.limited = limited;
    entry
}

fn scan(
    entries: &[WiEntry],
    chat: &[&str],
    description: ScanField,
    timed: WiTimed,
    hooks: &mut Hooks,
) -> WiResult {
    let mut entries = entries.to_vec();
    sort_entries(&mut entries);
    let chat: Vec<String> = chat.iter().map(|line| (*line).to_owned()).collect();
    let global = GlobalScan {
        character_description: description,
        ..GlobalScan::default()
    };
    check_world_info(
        &entries,
        ScanInput {
            chat: &chat,
            max_context: f64::INFINITY,
            global_scan: &global,
            trigger: "normal",
            timed,
            settings: &ST_WI_SETTINGS,
        },
        hooks,
    )
}

const PUBLIC_MD: &str = "The night watcher keeps the lantern.";
const PRIVATE_MD: &str = "She hides a secret map.";

fn description() -> ScanField {
    ScanField {
        full: format!("{PUBLIC_MD}\n{PRIVATE_MD}"),
        public: PUBLIC_MD.to_owned(),
    }
}

fn book() -> Vec<WiEntry> {
    vec![
        // 只命中公開描述
        entry(
            "watcher",
            json!({ "key": ["watcher"], "matchCharacterDescription": true }),
            false,
        ),
        // 只命中私設
        entry(
            "map",
            json!({ "key": ["map"], "matchCharacterDescription": true, "content": "the old tower" }),
            false,
        ),
        // 被私密條目的內文遞迴觸發的 Public 條目，再往下遞迴到 outlet
        entry(
            "tower",
            json!({ "key": ["tower"], "content": "the bell rings" }),
            false,
        ),
        entry(
            "bell",
            json!({ "key": ["bell"], "position": 7, "outletName": "hud" }),
            false,
        ),
        // 被公開條目遞迴觸發
        entry("keeper", json!({ "key": ["keeper"] }), false),
        entry(
            "watcher2",
            json!({ "key": ["door"], "content": "keeper" }),
            false,
        ),
        // 限定條目（常駐）的內文觸發的 Public 條目
        entry(
            "limited",
            json!({ "constant": true, "content": "hidden cellar" }),
            true,
        ),
        entry("cellar", json!({ "key": ["cellar"] }), false),
        // 聊天訊息觸發
        entry("chat", json!({ "key": ["door"] }), false),
    ]
}

#[test]
fn classifies_public_and_private_triggers() {
    let result = scan(
        &book(),
        &["Alice: I open the door."],
        description(),
        WiTimed::default(),
        &mut Hooks::default(),
    );
    let private: Vec<&str> = result.private_ids.iter().map(String::as_str).collect();
    assert_eq!(private, ["bell", "cellar", "map", "tower"]);
    for public in ["watcher", "watcher2", "keeper", "chat", "limited"] {
        assert!(
            result.activated.iter().any(|id| id == public),
            "{public} fired"
        );
        assert!(!result.private_ids.contains(public), "{public} is public");
    }
}

#[test]
fn source_tracking_does_not_change_what_fires() {
    let mut tracked = Hooks::default();
    let with_sources = scan(
        &book(),
        &["Alice: I open the door."],
        description(),
        WiTimed::default(),
        &mut tracked,
    );
    let mut untracked = Hooks::default();
    let plain: Vec<WiEntry> = book()
        .into_iter()
        .map(|mut entry| {
            entry.limited = false;
            entry
        })
        .collect();
    let without = scan(
        &plain,
        &["Alice: I open the door."],
        ScanField::public(format!("{PUBLIC_MD}\n{PRIVATE_MD}")),
        WiTimed::default(),
        &mut untracked,
    );
    assert_eq!(with_sources.activated, without.activated);
    assert_eq!(with_sources.before, without.before);
    assert_eq!(with_sources.outlets, without.outlets);
    assert!(without.private_ids.is_empty());
    // 判來源不重跑代換（不得重跑巨集副作用）
    assert_eq!(tracked.calls, untracked.calls);
}

#[test]
fn not_any_with_its_own_content_stays_public() {
    // 主鍵 a、NOT_ANY 次要鍵 x、內文含 x：公開觸發後把自己的內文加進緩衝也不會改判
    let entries = vec![entry(
        "self",
        json!({ "key": ["alpha"], "keysecondary": ["xeno"], "selectiveLogic": 2, "content": "xeno" }),
        false,
    )];
    let result = scan(
        &entries,
        &["Alice: alpha"],
        ScanField::default(),
        WiTimed::default(),
        &mut Hooks::default(),
    );
    assert_eq!(result.activated, ["self"]);
    assert!(result.private_ids.is_empty());
}

#[test]
fn private_key_substitution_makes_the_trigger_private() {
    let entries = vec![entry("door", json!({ "key": ["door"] }), false)];
    let mut hooks = Hooks {
        private_inputs: vec!["door"],
        ..Hooks::default()
    };
    let result = scan(
        &entries,
        &["Alice: door"],
        ScanField::default(),
        WiTimed::default(),
        &mut hooks,
    );
    assert_eq!(result.activated, ["door"]);
    assert!(result.private_ids.contains("door"));
}

#[test]
fn sticky_keeps_the_source_it_was_recorded_with() {
    let entries = vec![
        entry(
            "map",
            json!({ "key": ["map"], "matchCharacterDescription": true, "sticky": 3 }),
            false,
        ),
        entry("old", json!({ "key": ["zzz"], "sticky": 3 }), false),
    ];
    let first = scan(
        &entries,
        &["Alice: hello"],
        description(),
        WiTimed {
            sticky: [(
                "old".to_owned(),
                TimedEffect {
                    start: 0.0,
                    end: 5.0,
                    protected: false,
                    confidential: true,
                },
            )]
            .into(),
            ..WiTimed::default()
        },
        &mut Hooks::default(),
    );
    assert!(first.private_ids.contains("map"));
    assert!(first.private_ids.contains("old"));
    assert!(first.timed.sticky["map"].confidential);
}
