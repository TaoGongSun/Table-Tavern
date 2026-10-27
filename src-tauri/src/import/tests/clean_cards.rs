//! 乾淨替代測試卡（`fixtures/clean-*.json`，說明見 `fixtures/README.md`）的回歸快照：每張卡走角色卡、
//! 世界書兩路各匯一次，把穩定的結果寫成文字，兩路必須逐字相同，再跟 `fixtures/<卡>.expected.txt` 比對。
//!
//! 快照**不含**：帳本原始行（帶時間戳）、角色 id、條目可見度與來源卡（兩路本來就不同）、
//! `World.Roll100`／`World.Roll20` 的值（每輪真隨機重擲）、`assemble_card_context` 字數（隨 i18n 文案變）。
//!
//! 預期值要改時人工明確執行：`UPDATE_EXPECTED=1 cargo test --lib clean_cards`，再審 diff 後 commit。

use crate::data::{self, StateNode};
use crate::import::test_support::TestRoot;
use crate::import::{import_character_file, import_worldbook_file, read_card_interfaces};
use crate::mechanism::{self, RecordKind};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/import/fixtures")
        .join(name)
}

fn leaf(tree: &BTreeMap<String, StateNode>, path: &str) -> String {
    let mut map = tree;
    let mut found = None;
    for segment in path.split('.') {
        match map.get(segment) {
            Some(StateNode::Branch(children)) => {
                found = None;
                map = children;
            }
            Some(other) => found = Some(other),
            None => return "<missing>".to_owned(),
        }
    }
    match found {
        Some(StateNode::Leaf(value)) => value.clone(),
        _ => "<branch>".to_owned(),
    }
}

fn outline(out: &mut String, tree: &BTreeMap<String, StateNode>) {
    for (key, node) in tree {
        match node {
            StateNode::Branch(children) => {
                let names: Vec<&str> = children.keys().map(String::as_str).collect();
                let _ = writeln!(out, "  {key}: [{}]", names.join(", "));
            }
            StateNode::Leaf(value) => {
                let _ = writeln!(out, "  {key} = {value}");
            }
        }
    }
}

/// 匯入一次，回穩定的結果文字與帳本 (接管, 跳過) 條數
fn snapshot(bytes: &[u8], route: &str) -> Snapshot {
    let root = TestRoot::new(&format!("clean-{route}"));
    let path = root.path();
    let world_id = data::create_world(path, "測試桌").unwrap();
    let held = data::test_exclusive(&world_id);
    if route == "character" {
        import_character_file(path, &world_id, bytes, "#3366ff", "zh-TW", &held).unwrap();
    } else {
        import_worldbook_file(path, &world_id, bytes, "clean", &held).unwrap();
    }
    let mut out = String::new();

    let book = data::read_worldbook(path, &world_id).unwrap();
    let _ = writeln!(out, "## worldbook: {} entries", book.len());
    for entry in &book {
        let _ = writeln!(
            out,
            "  uid={} disabled={} constant={} keys={:?} title={}",
            entry.uid, entry.disabled, entry.constant, entry.keys, entry.title
        );
    }

    let state = data::read_state(path, &world_id).unwrap();
    let mechanism_state = &state.mechanism;
    let tree = &state.state.tree;
    let _ = writeln!(
        out,
        "## mechanism: incremental={}",
        mechanism_state.incremental
    );
    let _ = writeln!(out, "## tree");
    outline(&mut out, tree);
    for leaf_path in [
        "Player.Name",
        "Heroes.艾德温·晨盾.HP",
        "Heroes.艾德温·晨盾.Affection",
        "World.Title",
        "Player.Inventory",
        "World.Invasion",
    ] {
        let _ = writeln!(out, "  leaf {leaf_path} = {}", leaf(tree, leaf_path));
    }
    let _ = writeln!(out, "## rules: {}", mechanism_state.rules.len());
    for (rule_path, rule) in &mechanism_state.rules {
        let _ = writeln!(
            out,
            "  {rule_path} => {}",
            serde_json::to_string(rule).unwrap()
        );
    }
    let _ = writeln!(out, "## triggers: {}", mechanism_state.triggers.len());
    for trigger in &mechanism_state.triggers {
        let _ = writeln!(
            out,
            "  id={} mode={:?} cases={} flag={:?} scope={:?} preamble_len={}",
            trigger.id,
            trigger.mode,
            trigger.cases.len(),
            trigger.flag,
            trigger.scope,
            trigger.preamble.chars().count()
        );
    }

    let ledger = mechanism::read_ledger(path, &world_id);
    let count = |kind| ledger.entries.iter().filter(|e| e.kind == kind).count();
    let (absorbed, skipped) = (count(RecordKind::Absorbed), count(RecordKind::Skipped));
    // 機制條目匯入即停用，帳本列的一律還沒照原文送模型
    assert!(ledger.entries.iter().all(|entry| !entry.sent), "{route}");
    let _ = writeln!(out, "## ledger: absorbed={absorbed} skipped={skipped}");
    for entry in &ledger.entries {
        let _ = writeln!(
            out,
            "  {:?} uid={} sent={} {}",
            entry.kind, entry.uid, entry.sent, entry.title
        );
    }

    let signals = crate::refactor_ai::prescan_worldbook(&book);
    let _ = writeln!(out, "## prescan signals: {}", signals.len());
    for signal in &signals {
        let _ = writeln!(out, "  {} {}", signal.span, signal.pattern);
    }

    if mechanism_state.incremental {
        simulate_round(&mut out, path, &world_id);
    }

    // 卡片介面分路記：世界書路只在卡帶顯示腳本時才留原卡（save_world_card），沒腳本的卡這路本來就沒有
    let mut interfaces = String::new();
    for card in read_card_interfaces(path, &world_id).unwrap() {
        let _ = writeln!(
            interfaces,
            "  scripts={} mvu={} unsupported={:?} opening={}",
            card.scripts.len(),
            card.mvu,
            card.unsupported,
            card.opening.is_some()
        );
    }
    Snapshot {
        shared: out,
        interfaces,
        absorbed,
        skipped,
    }
}

struct Snapshot {
    /// 兩路必須逐字相同的部分
    shared: String,
    interfaces: String,
    absorbed: usize,
    skipped: usize,
}

/// 模擬一輪狀態更新＋一輪空更新（只看非隨機欄位與觸發命中）
fn simulate_round(out: &mut String, path: &Path, world_id: &str) {
    let mut world = data::read_state(path, world_id).unwrap();
    let reply = "艾德温握紧了盾。\n<UpdateVariable>\n<JSONPatch>\n[\n\
        { \"op\": \"delta\", \"path\": \"/Heroes/艾德温·晨盾/Affection\", \"value\": 55 },\n\
        { \"op\": \"delta\", \"path\": \"/Heroes/艾德温·晨盾/Rivalry\", \"value\": 20 },\n\
        { \"op\": \"delta\", \"path\": \"/World/Invasion\", \"value\": 55 },\n\
        { \"op\": \"replace\", \"path\": \"/World/Location\", \"value\": \"北境 · 白垒城\" }\n\
        ]\n</JSONPatch>\n</UpdateVariable>";
    let block = crate::transport::extract_state_block(reply);
    let outcome = mechanism::apply_block(&mut world, &block, "阿濤");
    let _ = writeln!(out, "## round 1 records");
    for record in &outcome.records {
        let _ = writeln!(out, "  {:?} {}", record.kind, record.path);
    }
    let tree = &world.state.tree;
    for leaf_path in [
        "Heroes.艾德温·晨盾.Affection",
        "Heroes.艾德温·晨盾.Rivalry",
        "World.Invasion",
        "World.Location",
        "Events.Northhold_Siege_Triggered",
    ] {
        let _ = writeln!(out, "  leaf {leaf_path} = {}", leaf(tree, leaf_path));
    }
    // 命中的觸發與它挑中的那段文字（關係階段／事件標題／環境指令那一行），看得出換到哪一格
    for (id, text) in &world.state.triggers {
        let line = text
            .lines()
            .find(|line| {
                line.contains("当前关系") || line.contains("突发事件") || line.contains('[')
            })
            .unwrap_or("")
            .trim();
        let _ = writeln!(
            out,
            "  hit {id}: {}",
            line.chars().take(40).collect::<String>()
        );
    }
    let block = crate::transport::extract_state_block(
        "什么都没变。\n<UpdateVariable>\n<JSONPatch>\n[]\n</JSONPatch>\n</UpdateVariable>",
    );
    mechanism::apply_block(&mut world, &block, "阿濤");
    let _ = writeln!(
        out,
        "## round 2 hits: {:?}",
        world.state.triggers.keys().collect::<Vec<_>>()
    );
}

/// 兩路逐字相同，再比預期檔；`UPDATE_EXPECTED` 精確等於 `1` 才改寫預期檔（其他值或殘留的空值都不寫）
fn check(card: &str) -> (usize, usize) {
    let bytes = std::fs::read(fixture_path(&format!("{card}.json"))).unwrap();
    let character = snapshot(&bytes, "character");
    let worldbook = snapshot(&bytes, "worldbook");
    assert_eq!(
        character.shared, worldbook.shared,
        "{card}：角色卡路與世界書路結果不同"
    );
    let actual = format!(
        "{}## interface via character\n{}## interface via worldbook\n{}",
        character.shared, character.interfaces, worldbook.interfaces
    );

    let expected_path = fixture_path(&format!("{card}.expected.txt"));
    if std::env::var("UPDATE_EXPECTED").as_deref() == Ok("1") {
        std::fs::write(&expected_path, &actual).unwrap();
    }
    let expected = std::fs::read_to_string(&expected_path).unwrap_or_default();
    assert!(
        actual == expected,
        "{card}：結果與 {} 不同（確認是預期的改變才用 UPDATE_EXPECTED=1 重寫）\n--- 實際 ---\n{actual}",
        expected_path.display()
    );
    (character.absorbed, character.skipped)
}

#[test]
fn clean_orc_cave_matches_expected_on_both_routes() {
    assert_eq!(check("clean-orc-cave"), (0, 0));
}

#[test]
fn clean_hero_roster_matches_expected_on_both_routes() {
    // 接管 12＝9 條觸發表＋initvar＋变量更新规则＋变量列表；跳過 6＝認不出的 EJS
    assert_eq!(check("clean-hero-roster"), (12, 6));
}
