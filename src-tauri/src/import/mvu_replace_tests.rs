//! mvu-replace-numeric 整合：匯入 → 載入層算更新策略 → 開場白提交（apply_block）。
use crate::data::{self, NumericUpdate};
use crate::import::import_character;
use crate::import::test_support::TestRoot;
use crate::mechanism::{self, RecordKind};
use serde_json::{json, Value};

const RULES: &str = "变量更新规则:\n\
     \x20 World:\n\
     \x20   Invasion:\n\
     \x20     type: number\n\
     \x20     range: 0-100\n";
const INITVAR: &str = "World:\n  Invasion: 1\n  Gold: 100000\nPlayer:\n  HP: \"480/500\"";

fn card(entries: Value, extensions: Value) -> String {
    json!({
        "data": {
            "name": "莉亞",
            "extensions": extensions,
            "character_book": { "entries": entries }
        }
    })
    .to_string()
}

fn mvu_book() -> Value {
    json!([
        { "comment": "[initvar]变量初始化勿开", "enabled": false, "content": INITVAR },
        { "comment": "[mvu_update]变量更新规则", "enabled": true, "content": RULES }
    ])
}

fn table(label: &str, raw: &str) -> (TestRoot, String) {
    let root = TestRoot::new(label);
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff", "zh-TW").unwrap();
    (root, world_id)
}

fn policy(root: &TestRoot, world_id: &str) -> NumericUpdate {
    let state = data::read_state(root.path(), world_id).unwrap();
    mechanism::resolve_numeric_update(root.path(), world_id, &state)
}

fn set_mode(root: &TestRoot, world_id: &str, mode: Option<&str>) {
    let mut state = data::read_state(root.path(), world_id).unwrap();
    state.refactor_mode = mode.map(str::to_owned);
    data::write_state(root.path(), world_id, &state).unwrap();
}

const OPENING: &str = "開場旁白\n<UpdateVariable><JSONPatch>[\
    {\"op\":\"replace\",\"path\":\"/World/Invasion\",\"value\":150},\
    {\"op\":\"replace\",\"path\":\"/World/Gold\",\"value\":\"99000\"},\
    {\"op\":\"replace\",\"path\":\"/Player/HP\",\"value\":\"450/600\"}\
    ]</JSONPatch></UpdateVariable>";

fn post_opening(root: &TestRoot, world_id: &str) -> mechanism::Outcome {
    let (_, outcome) = data::append_opening(
        root.path(),
        world_id,
        0,
        "opening",
        OPENING,
        &crate::transport::extract_state_block(OPENING),
        "阿濤",
    )
    .unwrap();
    outcome
}

fn value(root: &TestRoot, world_id: &str, path: &str) -> Option<String> {
    let state = data::read_state(root.path(), world_id).unwrap();
    let segments: Vec<String> = path.split('.').map(str::to_owned).collect();
    match data::node_at(&state.state.tree, &segments)? {
        data::StateNode::Leaf(text) => Some(text.clone()),
        data::StateNode::Branch(_) => None,
    }
}

/// 沒重構的 MVU 鷹架桌：明確規則（帶範圍不夾）、無規則推型（數字字串）、Pair 整值，開場白一次全收。
#[test]
fn unrefactored_mvu_worldbook_opening_accepts_numeric_replace() {
    let (root, world_id) = table("mvu-replace-open", &card(mvu_book(), json!({})));
    assert_eq!(policy(&root, &world_id), NumericUpdate::Upstream);
    let outcome = post_opening(&root, &world_id);
    assert!(
        !outcome
            .records
            .iter()
            .any(|record| record.kind == RecordKind::Rejected),
        "{:?}",
        outcome.records
    );
    assert_eq!(
        value(&root, &world_id, "World.Invasion").as_deref(),
        Some("150")
    );
    assert_eq!(
        value(&root, &world_id, "World.Gold").as_deref(),
        Some("99000")
    );
    assert_eq!(
        value(&root, &world_id, "Player.HP").as_deref(),
        Some("450/600")
    );
}

/// 重構過的同一張卡（任何玩法標記）：開場白的數字 replace 照舊擋下，Pair 只改上限。
#[test]
fn refactored_mvu_table_opening_still_rejects_numeric_replace() {
    for mode in ["interface", "characters"] {
        let (root, world_id) = table("mvu-replace-refactored", &card(mvu_book(), json!({})));
        set_mode(&root, &world_id, Some(mode));
        assert_eq!(policy(&root, &world_id), NumericUpdate::DeltaOnly, "{mode}");
        let outcome = post_opening(&root, &world_id);
        let rejected: Vec<&str> = outcome
            .records
            .iter()
            .filter(|record| record.kind == RecordKind::Rejected)
            .map(|record| record.path.as_str())
            .collect();
        assert_eq!(
            rejected,
            ["World.Invasion", "World.Gold", "Player.HP"],
            "{mode}"
        );
        assert_eq!(
            value(&root, &world_id, "World.Invasion").as_deref(),
            Some("1")
        );
        assert_eq!(
            value(&root, &world_id, "World.Gold").as_deref(),
            Some("100000")
        );
        assert_eq!(
            value(&root, &world_id, "Player.HP").as_deref(),
            Some("480/600")
        );
    }
}

/// 舊重構桌（沒有玩法標記但留有重構產物）、清殼後的重構桌：一律維持原規則。
#[test]
fn refactor_outcome_without_mode_keeps_delta_only() {
    let (root, world_id) = table("mvu-replace-old-refactor", &card(mvu_book(), json!({})));
    data::write_refactor_outcome(root.path(), &world_id, "{}").unwrap();
    assert_eq!(policy(&root, &world_id), NumericUpdate::DeltaOnly);
}

/// MVU 來源依據：只有世界書鷹架（含只掃到 [mvu_update]、或只有 [initvar]）或卡載入 MVU 腳本才算；一般卡維持原規則。
#[test]
fn mvu_source_evidence_decides_upstream() {
    let only_rules = json!([{ "comment": "[mvu_update]规则", "enabled": true, "content": RULES }]);
    let (root, world_id) = table("mvu-replace-rules-only", &card(only_rules, json!({})));
    assert_eq!(policy(&root, &world_id), NumericUpdate::Upstream);

    let only_init = json!([{ "comment": "[initvar]初始", "enabled": false, "content": INITVAR }]);
    let (root, world_id) = table("mvu-replace-init-only", &card(only_init, json!({})));
    assert_eq!(policy(&root, &world_id), NumericUpdate::Upstream);

    let script = json!({"tavern_helper": {"scripts": [{
        "name": "MVU", "enabled": true, "type": "script",
        "content": "import'https://testingcf.jsdelivr.net/gh/MagicalAstrogy/MagVarUpdate/artifact/bundle.js';"
    }]}});
    let (root, world_id) = table("mvu-replace-script", &card(json!([]), script.clone()));
    assert!(
        !data::read_state(root.path(), &world_id)
            .unwrap()
            .mechanism
            .incremental
    );
    assert_eq!(policy(&root, &world_id), NumericUpdate::Upstream);
    // 同一張腳本卡重構成 characters（沒有殼）：維持原規則
    set_mode(&root, &world_id, Some("characters"));
    assert_eq!(policy(&root, &world_id), NumericUpdate::DeltaOnly);

    let plain = json!([{ "comment": "地點", "enabled": true, "content": "晨港" }]);
    let (root, world_id) = table("mvu-replace-plain", &card(plain, json!({})));
    assert_eq!(policy(&root, &world_id), NumericUpdate::DeltaOnly);
}

/// 策略不落檔：寫回 state.json 再讀，欄位回到預設（每次由載入層重算）。
#[test]
fn numeric_update_is_not_persisted() {
    let (root, world_id) = table("mvu-replace-skip", &card(mvu_book(), json!({})));
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    state.mechanism.numeric_update = NumericUpdate::Upstream;
    data::write_state(root.path(), &world_id, &state).unwrap();
    let reread = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(reread.mechanism.numeric_update, NumericUpdate::DeltaOnly);
}
