//! 重構套用的可見度、觸發與來源核對（worldbook-character-visibility 二之 7）：
//! 都驗實際送出的 messages，不只驗欄位。

use super::super::test_support::{apply_recorded, no_player_selection, TestRoot};
use super::super::{apply, RefactorOutcome, RefactorSelection};
use crate::data::{self, FieldKind, FieldRule, TranscriptEvent, TranscriptKind, Visibility};
use crate::refactor_ai::{RefactorEntryMeta, RefactorNewEntry};
use crate::{chat_assembly, receipts};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

const LANG: &str = "zh-TW";

fn outcome(
    entries: Vec<RefactorNewEntry>,
    fingerprints: BTreeMap<String, String>,
) -> RefactorOutcome {
    RefactorOutcome {
        mode: None,
        characters: Vec::new(),
        interface: None,
        mechanisms: Vec::new(),
        entries,
        deletable_shared_uids: Vec::new(),
        dropped: Vec::new(),
        unabsorbed: Vec::new(),
        audit: Vec::new(),
        preserve_source_uids: Vec::new(),
        source_fingerprints: fingerprints,
    }
}

fn merged(title: &str, content: &str, sources: &[u64]) -> RefactorNewEntry {
    RefactorNewEntry {
        title: title.to_owned(),
        kind: "setting".to_owned(),
        content: content.to_owned(),
        source_uids: sources.iter().map(u64::to_string).collect(),
        rules: BTreeMap::new(),
        triggers: Vec::new(),
        meta: None,
    }
}

fn carry(entry: &data::WorldbookEntry) -> RefactorNewEntry {
    RefactorNewEntry {
        title: entry.title.clone(),
        kind: "setting".to_owned(),
        content: entry.content.clone(),
        source_uids: vec![entry.uid.to_string()],
        rules: BTreeMap::new(),
        triggers: Vec::new(),
        meta: Some(RefactorEntryMeta {
            keys: entry.keys.clone(),
            constant: entry.constant,
            order: entry.order,
            disabled: entry.disabled,
            visibility: entry.visibility.clone(),
            is_person: entry.is_person,
        }),
    }
}

fn entries_selection(count: usize) -> RefactorSelection {
    RefactorSelection {
        entry_indices: (0..count).collect(),
        ..no_player_selection(Vec::new())
    }
}

fn import_card(root: &Path, world_id: &str, name: &str, mut entries: Value) -> String {
    // V2 缺 `enabled` 算停用（照 ST）；測試資料沒寫的當啟用
    for entry in entries.as_array_mut().into_iter().flatten() {
        entry
            .as_object_mut()
            .unwrap()
            .entry("enabled")
            .or_insert(json!(true));
    }
    let card = json!({"data": {"name": name, "character_book": {"entries": entries}}}).to_string();
    crate::import::import_character(root, world_id, card.as_bytes(), "#3366ff", LANG)
        .unwrap()
        .id
}

fn uid_of(root: &Path, world_id: &str, content: &str) -> u64 {
    data::read_worldbook(root, world_id)
        .unwrap()
        .into_iter()
        .find(|entry| entry.content == content)
        .unwrap()
        .uid
}

fn event(text: &str) -> TranscriptEvent {
    TranscriptEvent {
        ts: "2026-10-10T12:00:00+08:00".to_owned(),
        speaker_id: String::new(),
        speaker_name: "玩家".to_owned(),
        kind: TranscriptKind::Player,
        text: text.to_owned(),
        raw: None,
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
    }
}

/// 角色線（API 共線）整份送出的文字。
fn character_sent(root: &Path, world_id: &str, character_id: &str, said: &str) -> String {
    let card = data::read_character(root, world_id, character_id).unwrap();
    let cards = chat_assembly::active_cards(root, world_id).unwrap();
    let state = data::read_state(root, world_id).unwrap();
    let events = [event(said)];
    let scan = chat_assembly::test_character_scan(root, world_id, &card, None, &events, LANG);
    crate::transport::assemble_shared_messages(
        &card,
        &cards,
        None,
        &events,
        &scan,
        &state.state,
        &state.mechanism,
        None,
        LANG,
    )
    .iter()
    .map(|message| message.content.clone())
    .collect::<Vec<_>>()
    .join("\n")
}

/// GM 線（單發）整份送出的文字。
fn gm_sent(root: &Path, world_id: &str, said: &str) -> String {
    let state = data::read_state(root, world_id).unwrap();
    let book = crate::world_scan::TableBook::load(root, world_id).unwrap();
    let cards = chat_assembly::active_cards(root, world_id).unwrap();
    let events = [event(said)];
    let scan = crate::transport::test_support::legacy::prepared_book(
        &book,
        crate::world_scan::Viewer::Gm,
        "",
        &cards,
        None,
        &events,
        LANG,
    );
    crate::transport::assemble_gm_messages(
        &cards,
        None,
        &events,
        &scan,
        &state.state,
        &state.mechanism,
        &crate::transport::StateScope::default(),
        LANG,
    )
    .iter()
    .map(|message| message.content.clone())
    .collect::<Vec<_>>()
    .join("\n")
}

fn new_entry(root: &Path, world_id: &str, content: &str) -> data::WorldbookEntry {
    data::read_worldbook(root, world_id)
        .unwrap()
        .into_iter()
        .find(|entry| entry.content == content)
        .unwrap()
}

/// 角色卡路的桌一重構：合組條目沿用角色名單與觸發——有啟用的常駐來源就常駐、全 keyword 來源要命中
/// 來源主鍵才送；來源條目已刪。
#[test]
fn merged_entries_keep_the_owner_and_the_triggers() {
    let root = TestRoot::new("refactor-visibility-merge");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let card = import_card(
        root.path(),
        &world_id,
        "莉亞",
        json!([
            {"comment": "常駐", "keys": [], "content": "甲常駐甲", "constant": true},
            {"comment": "燈塔", "keys": ["燈塔"], "content": "丙燈塔丙", "constant": false},
            {"comment": "沙漠", "keys": ["沙漠"], "content": "乙沙漠乙", "constant": false}
        ]),
    );
    let constant = uid_of(root.path(), &world_id, "甲常駐甲");
    let lighthouse = uid_of(root.path(), &world_id, "丙燈塔丙");
    let desert = uid_of(root.path(), &world_id, "乙沙漠乙");
    let fingerprints = data::identity_fingerprints(root.path(), &world_id).unwrap();
    let outcome = outcome(
        vec![
            merged("合組常駐", "合組常駐的新內容", &[constant, lighthouse]),
            merged("合組地點", "合組地點的新內容", &[lighthouse, desert]),
        ],
        fingerprints,
    );
    apply_recorded(root.path(), &world_id, &outcome, &entries_selection(2));

    let remaining = data::read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(remaining.len(), 2, "來源條目被吸收刪除");
    let always = new_entry(root.path(), &world_id, "合組常駐的新內容");
    assert_eq!(
        always.visibility,
        Visibility::Characters(vec![card.clone()])
    );
    assert!(always.constant);
    let places = new_entry(root.path(), &world_id, "合組地點的新內容");
    assert!(!places.constant);
    assert_eq!(places.keys, vec!["燈塔".to_owned(), "沙漠".to_owned()]);

    let quiet = character_sent(root.path(), &world_id, &card, "今天天氣不錯。");
    assert_eq!(quiet.matches("合組常駐的新內容").count(), 1, "{quiet}");
    assert!(!quiet.contains("合組地點的新內容"), "{quiet}");
    let hit = character_sent(root.path(), &world_id, &card, "沙漠裡起風了。");
    assert_eq!(hit.matches("合組地點的新內容").count(), 1, "{hit}");
}

/// 混合來源（角色名單＋世界書路的 GM 條目）給 GM：角色看不到、GM 照觸發政策看得到。
/// 停用的常駐來源＋啟用的 keyword 來源：不常駐；來源全停用：新條目停用。
#[test]
fn mixed_and_disabled_sources() {
    let root = TestRoot::new("refactor-visibility-mixed");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let card = import_card(
        root.path(),
        &world_id,
        "莉亞",
        json!([
            {"comment": "常駐", "keys": [], "content": "甲常駐甲", "constant": true},
            {"comment": "停用常駐", "keys": [], "content": "丁停用丁", "constant": true, "enabled": false},
            {"comment": "燈塔", "keys": ["燈塔"], "content": "丙燈塔丙", "constant": false},
            {"comment": "停用二", "keys": ["雪"], "content": "庚停用庚", "constant": false, "enabled": false}
        ]),
    );
    let book = json!({"entries": {"0": {"uid": 0, "comment": "世界", "key": [], "content": "世界常識",
        "constant": true}}});
    data::import_worldbook_as(
        root.path(),
        &world_id,
        &book.to_string(),
        &data::BookOwner::Gm,
    )
    .unwrap();
    let uid = |content: &str| uid_of(root.path(), &world_id, content);
    let fingerprints = data::identity_fingerprints(root.path(), &world_id).unwrap();
    let outcome = outcome(
        vec![
            merged(
                "混合",
                "混合後的新內容",
                &[uid("甲常駐甲"), uid("世界常識")],
            ),
            merged(
                "半停用",
                "半停用的新內容",
                &[uid("丁停用丁"), uid("丙燈塔丙")],
            ),
            merged(
                "全停用",
                "全停用的新內容",
                &[uid("丁停用丁"), uid("庚停用庚")],
            ),
        ],
        fingerprints,
    );
    apply(root.path(), &world_id, &outcome, &entries_selection(3)).unwrap();

    let mixed = new_entry(root.path(), &world_id, "混合後的新內容");
    assert_eq!(mixed.visibility, Visibility::Gm);
    assert!(mixed.constant);
    assert!(!character_sent(root.path(), &world_id, &card, "你好。").contains("混合後的新內容"));
    assert_eq!(
        gm_sent(root.path(), &world_id, "你好。")
            .matches("混合後的新內容")
            .count(),
        1
    );

    let half = new_entry(root.path(), &world_id, "半停用的新內容");
    assert!(!half.constant && !half.disabled);
    assert_eq!(half.keys, vec!["燈塔".to_owned()]);
    assert!(!character_sent(root.path(), &world_id, &card, "你好。").contains("半停用的新內容"));
    assert!(character_sent(root.path(), &world_id, &card, "燈塔亮了。").contains("半停用的新內容"));

    let off = new_entry(root.path(), &world_id, "全停用的新內容");
    assert!(off.disabled);
    assert!(!gm_sent(root.path(), &world_id, "雪下了。").contains("全停用的新內容"));
}

/// 世界書路的重構桌：合組條目照來源觸發，GM 看得到（過去無主鍵、非常駐，GM 也觸發不到）。
#[test]
fn worldbook_route_merged_entries_reach_the_gm() {
    let root = TestRoot::new("refactor-visibility-gm");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let book = json!({"entries": {
        "0": {"uid": 0, "comment": "城", "key": [], "content": "城的設定", "constant": true},
        "1": {"uid": 1, "comment": "港", "key": ["港口"], "content": "港的設定", "constant": false}
    }});
    data::import_worldbook_as(
        root.path(),
        &world_id,
        &book.to_string(),
        &data::BookOwner::Gm,
    )
    .unwrap();
    let fingerprints = data::identity_fingerprints(root.path(), &world_id).unwrap();
    let outcome = outcome(
        vec![merged("城港", "城與港的合組內容", &[0, 1])],
        fingerprints,
    );
    apply(root.path(), &world_id, &outcome, &entries_selection(1)).unwrap();
    assert_eq!(
        gm_sent(root.path(), &world_id, "你好。")
            .matches("城與港的合組內容")
            .count(),
        1
    );
}

/// 帶欄位規則的機制條目照來源觸發、可見度固定 GM；carry 條目的 locked／is_person 以套用端判定為準。
#[test]
fn locked_mechanism_entries_and_carry_flags() {
    let root = TestRoot::new("refactor-visibility-locked");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let card = import_card(
        root.path(),
        &world_id,
        "莉亞",
        json!([
            {"comment": "常駐", "keys": ["王都"], "content": "甲常駐甲", "constant": true},
            {"comment": "照搬", "keys": ["月"], "secondary_keys": ["夜"], "content": "照搬的內容",
             "constant": true, "position": "after_char"}
        ]),
    );
    let constant = uid_of(root.path(), &world_id, "甲常駐甲");
    let carried_source = new_entry(root.path(), &world_id, "照搬的內容");
    // 來源原始值帶舊旗標：套用端判定要蓋掉
    let mut stale = carried_source.clone();
    stale.locked = true;
    stale.is_person = true;
    data::upsert_worldbook_entry(root.path(), &world_id, stale).unwrap();
    let fingerprints = data::identity_fingerprints(root.path(), &world_id).unwrap();
    let mut rule_entry = merged("規則", "規則說明內容", &[constant]);
    rule_entry.kind = "mechanism".to_owned();
    rule_entry.rules = BTreeMap::from([(
        "莉亞.好感".to_owned(),
        FieldRule::for_kind(FieldKind::Number),
    )]);
    // 重寫路徑（commands/refactor.rs 的 rewrite）會產出帶 meta 的機制條目：meta 照抄來源的
    // 角色名單／常駐／主鍵，套用時一樣不能沿用
    let mut meta_rule = carry(&new_entry(root.path(), &world_id, "甲常駐甲"));
    meta_rule.title = "帶meta規則".to_owned();
    meta_rule.content = "帶meta規則內容".to_owned();
    meta_rule.kind = "mechanism".to_owned();
    meta_rule.rules = BTreeMap::from([(
        "莉亞.信任".to_owned(),
        FieldRule::for_kind(FieldKind::Number),
    )]);
    let outcome = outcome(
        vec![rule_entry, carry(&carried_source), meta_rule],
        fingerprints,
    );
    apply(root.path(), &world_id, &outcome, &entries_selection(3)).unwrap();

    // 帶欄位規則的機制條目照來源觸發（來源常駐就常駐），但可見度一律 GM、不進角色線
    let meta_rule = new_entry(root.path(), &world_id, "帶meta規則內容");
    assert!(meta_rule.locked && !meta_rule.is_person);
    assert_eq!(meta_rule.visibility, Visibility::Gm);
    assert!(meta_rule.constant);
    assert!(!character_sent(root.path(), &world_id, &card, "你好。").contains("帶meta規則內容"));
    assert!(gm_sent(root.path(), &world_id, "你好。").contains("帶meta規則內容"));

    let rule = new_entry(root.path(), &world_id, "規則說明內容");
    assert!(rule.locked);
    assert_eq!(rule.visibility, Visibility::Gm);
    assert!(rule.constant);
    assert!(!character_sent(root.path(), &world_id, &card, "你好。").contains("規則說明內容"));
    assert!(gm_sent(root.path(), &world_id, "你好。").contains("規則說明內容"));

    let carried = new_entry(root.path(), &world_id, "照搬的內容");
    assert!(!carried.locked && !carried.is_person);
    assert_eq!(
        carried.visibility,
        Visibility::Characters(vec![card.clone()])
    );
    let raw = data::read_worldbook_raw(root.path(), &world_id)
        .unwrap()
        .remove(&carried.uid)
        .unwrap();
    assert_eq!(raw["keysecondary"], json!(["夜"]));
    assert_eq!(raw["position"], json!(1));
    assert_eq!(data::source_cards_of(&raw), vec![card.clone()]);
}

/// 跨桌套用：B 桌有一條同 UID 但內容無關的條目，套用前後原樣保留（不刪、不停用、不記帳本、不複製它）；
/// 同一張卡在 B 桌的條目 UID 不同，反查對上——carry 的可見度是 B 角色，B 角色線看得到。
#[test]
fn cross_table_apply_verifies_sources() {
    let root = TestRoot::new("refactor-visibility-cross");
    let world_a = data::create_world(root.path(), "甲桌").unwrap();
    let entries = json!([
        {"comment": "常駐", "keys": [], "content": "甲常駐甲", "constant": true},
        {"comment": "燈塔", "keys": ["燈塔"], "content": "丙燈塔丙", "constant": false}
    ]);
    import_card(root.path(), &world_a, "莉亞", entries.clone());
    let a_constant = new_entry(root.path(), &world_a, "甲常駐甲");
    let a_lighthouse = uid_of(root.path(), &world_a, "丙燈塔丙");
    let mut outcome = outcome(
        vec![
            carry(&a_constant),
            merged("合組", "合組的新內容", &[a_lighthouse]),
        ],
        data::identity_fingerprints(root.path(), &world_a).unwrap(),
    );
    outcome.dropped = vec![crate::refactor_assemble::RefactorDroppedEntry {
        uid: a_constant.uid.to_string(),
        span: String::new(),
        title: "常駐".to_owned(),
        content: "甲常駐甲".to_owned(),
        rule: 0,
    }];
    outcome.mechanisms = vec![super::super::types::RefactorMechanism {
        source_uid: a_constant.uid.to_string(),
        rules: BTreeMap::new(),
        triggers: Vec::new(),
    }];

    // B 桌：先放一條無關條目佔掉 A 桌來源的 UID，再匯同一張卡
    let world_b = data::create_world(root.path(), "乙桌").unwrap();
    let book = json!({"entries": {"0": {"uid": 0, "comment": "無關條目", "key": [],
        "content": "跟卡無關的內容", "constant": true}}});
    data::import_worldbook_as(
        root.path(),
        &world_b,
        &book.to_string(),
        &data::BookOwner::Gm,
    )
    .unwrap();
    let card_b = import_card(root.path(), &world_b, "莉亞", entries);
    let unrelated_uid = uid_of(root.path(), &world_b, "跟卡無關的內容");
    assert_eq!(unrelated_uid, a_constant.uid, "同 UID、內容不同");
    let without_display = |mut value: Value| {
        value.as_object_mut().unwrap().remove("displayIndex");
        value
    };
    let unrelated_before = without_display(
        data::read_worldbook_raw(root.path(), &world_b).unwrap()[&unrelated_uid].clone(),
    );
    let selection = RefactorSelection {
        entry_indices: vec![0, 1],
        mechanism_indices: vec![0],
        ..no_player_selection(Vec::new())
    };
    apply(root.path(), &world_b, &outcome, &selection).unwrap();

    let raw_b = data::read_worldbook_raw(root.path(), &world_b).unwrap();
    assert_eq!(
        without_display(raw_b[&unrelated_uid].clone()),
        unrelated_before,
        "無關條目原樣保留（顯示順序以外）"
    );
    let log = std::fs::read_to_string(data::mechanism_log_path(root.path(), &world_b).unwrap())
        .unwrap_or_default();
    assert!(!log.contains("無關條目"), "{log}");
    assert_eq!(
        raw_b
            .values()
            .filter(|value| value["content"] == "跟卡無關的內容")
            .count(),
        1,
        "不複製無關條目"
    );
    let carried: Vec<&Value> = raw_b
        .values()
        .filter(|value| value["content"] == "甲常駐甲")
        .collect();
    assert_eq!(carried.len(), 1, "來源被吸收、照搬的新條目留下");
    assert_eq!(
        carried[0]["extensions"]["table_tavern"]["visibility"],
        json!({"characters": [card_b.clone()]})
    );
    let sent = character_sent(root.path(), &world_b, &card_b, "燈塔亮了。");
    assert_eq!(sent.matches("甲常駐甲").count(), 1, "{sent}");
    assert_eq!(sent.matches("合組的新內容").count(), 1, "{sent}");

    // C 桌（空桌）：缺來源——carry 退回 meta（名單裡的角色不在本桌給 GM），合組條目給 GM 並常駐
    let world_c = data::create_world(root.path(), "丙桌").unwrap();
    apply(root.path(), &world_c, &outcome, &entries_selection(2)).unwrap();
    let c_carried = new_entry(root.path(), &world_c, "甲常駐甲");
    assert_eq!(c_carried.visibility, Visibility::Gm);
    let c_merged = new_entry(root.path(), &world_c, "合組的新內容");
    assert_eq!(c_merged.visibility, Visibility::Gm);
    assert!(c_merged.constant);
}

/// 一對一：兩個產物 uid 對到同一條實際條目，兩者都核對失敗、條目保留。
/// 映射後聚合：兩個產物引用同一個（反查後的）來源，只勾一個不刪來源，兩個都勾才刪。
#[test]
fn verified_sources_are_one_to_one_and_aggregate_by_actual_uid() {
    let root = TestRoot::new("refactor-visibility-one-to-one");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    import_card(
        root.path(),
        &world_id,
        "莉亞",
        json!([{"comment": "常駐", "keys": [], "content": "甲常駐甲", "constant": true}]),
    );
    let uid = uid_of(root.path(), &world_id, "甲常駐甲");
    let fingerprint =
        data::identity_fingerprints(root.path(), &world_id).unwrap()[&uid.to_string()].clone();
    // "0"（直接對上）與 "77"（反查對上）都指向同一條
    let both = outcome(
        vec![
            merged("一", "一的內容", &[uid]),
            merged("二", "二的內容", &[77]),
        ],
        BTreeMap::from([
            (uid.to_string(), fingerprint.clone()),
            ("77".to_owned(), fingerprint.clone()),
        ]),
    );
    apply(root.path(), &world_id, &both, &entries_selection(2)).unwrap();
    assert!(data::read_worldbook(root.path(), &world_id)
        .unwrap()
        .iter()
        .any(|entry| entry.content == "甲常駐甲"));

    let root = TestRoot::new("refactor-visibility-aggregate");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    import_card(
        root.path(),
        &world_id,
        "莉亞",
        json!([{"comment": "常駐", "keys": [], "content": "甲常駐甲", "constant": true}]),
    );
    // 產物裡的 uid "55" 在這桌不存在，反查對上；兩個產物都引用它
    let shared = |count: usize| {
        let outcome = outcome(
            vec![
                merged("一", "一的內容", &[55]),
                merged("二", "二的內容", &[55]),
            ],
            BTreeMap::from([("55".to_owned(), fingerprint.clone())]),
        );
        let world = data::create_world(root.path(), "桌").unwrap();
        import_card(
            root.path(),
            &world,
            "莉亞",
            json!([{"comment": "常駐", "keys": [], "content": "甲常駐甲", "constant": true}]),
        );
        let selection = RefactorSelection {
            entry_indices: (0..count).collect(),
            ..no_player_selection(Vec::new())
        };
        apply(root.path(), &world, &outcome, &selection).unwrap();
        data::read_worldbook(root.path(), &world)
            .unwrap()
            .iter()
            .any(|entry| entry.content == "甲常駐甲")
    };
    assert!(shared(1), "只勾一個：來源保留");
    assert!(!shared(2), "兩個都勾：來源刪除");
}

/// 同桌盤點後玩家改了來源內容：來源保留、新條目照建；只改開關或順序：照常吸收。
#[test]
fn edits_after_survey_keep_the_source() {
    for (label, edit_content) in [("content", true), ("toggle", false)] {
        let root = TestRoot::new(&format!("refactor-visibility-edited-{label}"));
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        import_card(
            root.path(),
            &world_id,
            "莉亞",
            json!([{"comment": "常駐", "keys": [], "content": "甲常駐甲", "constant": true}]),
        );
        let uid = uid_of(root.path(), &world_id, "甲常駐甲");
        let fingerprints = data::identity_fingerprints(root.path(), &world_id).unwrap();
        let mut entry = new_entry(root.path(), &world_id, "甲常駐甲");
        if edit_content {
            entry.content = "玩家改過的內容".to_owned();
        } else {
            entry.disabled = true;
            entry.order += 3;
        }
        data::upsert_worldbook_entry(root.path(), &world_id, entry).unwrap();
        let outcome = outcome(vec![merged("新", "新的內容", &[uid])], fingerprints);
        apply(root.path(), &world_id, &outcome, &entries_selection(1)).unwrap();
        let source_left = data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .iter()
            .any(|entry| entry.uid == uid);
        assert_eq!(source_left, edit_content, "{label}");
    }
}

fn export_book(root: &Path, world_id: &str, character_id: &str) -> Vec<Value> {
    let path = root.join(format!("{character_id}.json"));
    crate::import::export_character(root, world_id, character_id, &path).unwrap();
    let value: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let mut entries = value["data"]["character_book"]["entries"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    entries.sort_by_key(|entry| entry["content"].as_str().unwrap_or_default().to_owned());
    for entry in &mut entries {
        entry.as_object_mut().unwrap().remove("id");
    }
    entries
}

/// 重構後匯出：照搬條目保有次要鍵與位置；撤銷重構後匯出與重構前相同（插回的來源保有原始欄位）。
#[test]
fn export_after_refactor_and_after_undo() {
    let root = TestRoot::new("refactor-visibility-export");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let card = import_card(
        root.path(),
        &world_id,
        "莉亞",
        json!([
            {"comment": "照搬", "keys": ["月"], "secondary_keys": ["夜"], "content": "照搬的內容",
             "constant": false, "position": "after_char"},
            {"comment": "合組", "keys": ["港口"], "secondary_keys": ["霧"], "content": "被合組的內容",
             "constant": false, "position": "after_char"}
        ]),
    );
    let before = export_book(root.path(), &world_id, &card);
    let carried = new_entry(root.path(), &world_id, "照搬的內容");
    let absorbed = uid_of(root.path(), &world_id, "被合組的內容");
    let outcome = outcome(
        vec![carry(&carried), merged("合組", "合組的新內容", &[absorbed])],
        data::identity_fingerprints(root.path(), &world_id).unwrap(),
    );
    apply_recorded(root.path(), &world_id, &outcome, &entries_selection(2));
    let after = export_book(root.path(), &world_id, &card);
    let carried = after
        .iter()
        .find(|entry| entry["content"] == "照搬的內容")
        .unwrap();
    assert_eq!(carried["secondary_keys"], json!(["夜"]));
    assert_eq!(carried["position"], "after_char");
    assert!(after.iter().any(|entry| entry["content"] == "合組的新內容"));

    receipts::undo_last_import(root.path(), &world_id, &data::test_exclusive(&world_id)).unwrap();
    assert_eq!(export_book(root.path(), &world_id, &card), before);
}

/// 刪完來源、下一次寫入之前失敗（測試注入點，不寫壞檔）：走 apply_and_record 的 Err 分支自動回滾，
/// 失敗收據帶著原始 JSON，插回的來源條目保有原始欄位。
#[test]
fn apply_failure_after_source_deletion_rolls_back_with_raw_fields() {
    let root = TestRoot::new("refactor-visibility-err-branch");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let held = data::test_exclusive(&world_id);
    let card = import_card(
        root.path(),
        &world_id,
        "莉亞",
        json!([{"comment": "合組", "keys": ["港口"], "secondary_keys": ["霧"],
                "content": "被合組的內容", "position": "after_char"}]),
    );
    let raw_before = data::read_worldbook_raw(root.path(), &world_id).unwrap();
    let absorbed = uid_of(root.path(), &world_id, "被合組的內容");
    let outcome = outcome(
        vec![merged("合組", "合組的新內容", &[absorbed])],
        data::identity_fingerprints(root.path(), &world_id).unwrap(),
    );
    let error = {
        let _armed = super::super::apply::fail_point::arm();
        super::super::apply_and_record(
            root.path(),
            &world_id,
            &outcome,
            &entries_selection(1),
            &[],
            true,
            &held,
        )
        .unwrap_err()
        .to_string()
    };
    assert!(
        error.contains("injected failure after source deletion"),
        "{error}"
    );
    let raw_after = data::read_worldbook_raw(root.path(), &world_id).unwrap();
    assert_eq!(raw_after.len(), 1, "新條目撤掉、來源插回");
    let restored = raw_after.values().next().unwrap();
    let original = &raw_before[&absorbed];
    for field in ["keysecondary", "position", "content"] {
        assert_eq!(restored[field], original[field], "{field}");
    }
    assert_eq!(data::source_cards_of(restored), vec![card]);
    assert!(receipts::list_import_receipts(root.path(), &world_id)
        .iter()
        .all(|receipt| receipt.kind != "refactor"));
}

fn mechanism_entry(
    source: &data::WorldbookEntry,
    content: &str,
    rules: &[&str],
    triggers: Vec<data::Trigger>,
) -> RefactorNewEntry {
    let mut entry = carry(source);
    entry.title = format!("{}（機制）", source.title);
    entry.content = content.to_owned();
    entry.kind = "mechanism".to_owned();
    entry.rules = rules
        .iter()
        .map(|path| ((*path).to_owned(), FieldRule::for_kind(FieldKind::Number)))
        .collect();
    entry.triggers = triggers;
    entry
}

fn range_trigger(id: &str, path: &str) -> data::Trigger {
    data::Trigger {
        id: id.to_owned(),
        title: id.to_owned(),
        mode: data::TriggerMode::Range,
        cases: vec![data::TriggerCase {
            when: vec![data::Condition::Range {
                path: path.to_owned(),
                min: Some(1.0),
                max: None,
                min_exclusive: false,
                max_exclusive: false,
                default: None,
            }],
            text: "命中".to_owned(),
        }],
        preamble: String::new(),
        scope: Vec::new(),
        flag: None,
    }
}

fn import_gm_book(root: &Path, world_id: &str, entries: Value) {
    data::import_worldbook_as(root, world_id, &entries.to_string(), &data::BookOwner::Gm).unwrap();
}

/// 帶規則（含規則＋觸發混合）的機制條目：單一來源核對得到就複製來源原始值（位置、次要鍵照原卡），
/// keyword 來源要命中主鍵才送 GM；只有觸發表的條目維持隱藏（防劇透）。
#[test]
fn rule_mechanism_entries_follow_source_position_and_keywords() {
    let root = TestRoot::new("refactor-visibility-mechanism-keyword");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    import_gm_book(
        root.path(),
        &world_id,
        json!({"entries": {
            "0": {"uid": 0, "comment": "規則", "key": ["迷宮"], "keysecondary": ["房間"],
                  "content": "變數規則原文", "constant": false, "position": 4, "depth": 2},
            "1": {"uid": 1, "comment": "劇本", "key": [], "content": "劇本原文", "constant": true}
        }}),
    );
    let fingerprints = data::identity_fingerprints(root.path(), &world_id).unwrap();
    let rule_source = new_entry(root.path(), &world_id, "變數規則原文");
    let script_source = new_entry(root.path(), &world_id, "劇本原文");
    let outcome = outcome(
        vec![
            mechanism_entry(
                &rule_source,
                "變數規則接管",
                &["迷宮.威脅"],
                vec![range_trigger("威脅", "迷宮.威脅")],
            ),
            mechanism_entry(
                &script_source,
                "劇本接管",
                &[],
                vec![range_trigger("劇本", "天數")],
            ),
        ],
        fingerprints,
    );
    apply(root.path(), &world_id, &outcome, &entries_selection(2)).unwrap();

    let rule = new_entry(root.path(), &world_id, "變數規則接管");
    assert!(rule.locked);
    assert_eq!(rule.visibility, Visibility::Gm);
    assert_eq!(rule.keys, vec!["迷宮".to_owned()]);
    let raw = data::read_worldbook_raw(root.path(), &world_id)
        .unwrap()
        .remove(&rule.uid)
        .unwrap();
    assert_eq!(raw["position"], json!(4));
    assert_eq!(raw["depth"], json!(2));
    assert_eq!(raw["keysecondary"], json!(["房間"]));
    assert!(gm_sent(root.path(), &world_id, "走進迷宮的房間。").contains("變數規則接管"));
    assert!(!gm_sent(root.path(), &world_id, "你好。").contains("變數規則接管"));

    let script = new_entry(root.path(), &world_id, "劇本接管");
    assert!(script.locked);
    assert!(!script.constant && script.keys.is_empty());
    assert!(!gm_sent(root.path(), &world_id, "你好。").contains("劇本接管"));
}

/// 篩選後規則與觸發全空：不鎖、照來源觸發，仍是 GM；停用的來源照樣停用。MVU 與非 MVU 桌都一樣——
/// 沒套介面時既有樹一律算數（非 MVU 只在套介面時不算，見 tree_authority.rs）。
#[test]
fn mechanism_entry_filtered_to_empty_unlocks_and_follows_source() {
    filtered_to_empty_case(true);
    filtered_to_empty_case(false);
}

fn filtered_to_empty_case(mvu: bool) {
    let root = TestRoot::new("refactor-visibility-mechanism-empty");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    import_gm_book(
        root.path(),
        &world_id,
        json!({"entries": {
            "0": {"uid": 0, "comment": "規則", "key": [], "content": "繁體規則原文", "constant": true},
            "1": {"uid": 1, "comment": "停用", "key": [], "content": "停用規則原文", "constant": true,
                  "disable": true}
        }}),
    );
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    // MVU 來源的桌（世界書 MVU 鷹架會開 incremental）
    state.mechanism.incremental = mvu;
    state.state.tree = BTreeMap::from([(
        "基础信息".to_owned(),
        data::StateNode::Branch(BTreeMap::from([(
            "时间".to_owned(),
            data::StateNode::Leaf("未知".to_owned()),
        )])),
    )]);
    data::write_state(root.path(), &world_id, &state).unwrap();
    let fingerprints = data::identity_fingerprints(root.path(), &world_id).unwrap();
    let source = new_entry(root.path(), &world_id, "繁體規則原文");
    let disabled = new_entry(root.path(), &world_id, "停用規則原文");
    let outcome = outcome(
        vec![
            mechanism_entry(
                &source,
                "對不上的規則",
                &["迷宮之主位置"],
                vec![range_trigger("撤退", "撤退進度")],
            ),
            mechanism_entry(&disabled, "停用的規則", &["基础信息.时间"], Vec::new()),
        ],
        fingerprints,
    );
    apply(root.path(), &world_id, &outcome, &entries_selection(2)).unwrap();

    let entry = new_entry(root.path(), &world_id, "對不上的規則");
    assert!(!entry.locked && !entry.is_person);
    assert_eq!(entry.visibility, Visibility::Gm);
    assert!(entry.constant);
    assert!(gm_sent(root.path(), &world_id, "你好。").contains("對不上的規則"));
    let mechanism = data::read_state(root.path(), &world_id).unwrap().mechanism;
    assert!(!mechanism.rules.contains_key("迷宮之主位置"));
    assert!(mechanism.triggers.is_empty());

    let disabled = new_entry(root.path(), &world_id, "停用的規則");
    assert!(disabled.locked && disabled.disabled);
    assert!(!gm_sent(root.path(), &world_id, "你好。").contains("停用的規則"));
}

/// 無 meta 的多來源合組機制條目：觸發照 sources::new_entry_shape（有常駐來源就常駐）。
#[test]
fn merged_rule_mechanism_without_meta_uses_source_shape() {
    let root = TestRoot::new("refactor-visibility-mechanism-merged");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    import_gm_book(
        root.path(),
        &world_id,
        json!({"entries": {
            "0": {"uid": 0, "comment": "甲", "key": [], "content": "甲機制", "constant": true},
            "1": {"uid": 1, "comment": "乙", "key": ["乙"], "content": "乙機制", "constant": false}
        }}),
    );
    let fingerprints = data::identity_fingerprints(root.path(), &world_id).unwrap();
    let mut entry = merged("合組機制", "合組機制說明", &[0, 1]);
    entry.kind = "mechanism".to_owned();
    entry.rules = BTreeMap::from([("好感".to_owned(), FieldRule::for_kind(FieldKind::Number))]);
    apply(
        root.path(),
        &world_id,
        &outcome(vec![entry], fingerprints),
        &entries_selection(1),
    )
    .unwrap();
    let entry = new_entry(root.path(), &world_id, "合組機制說明");
    assert!(entry.locked && entry.constant);
    assert_eq!(entry.visibility, Visibility::Gm);
    assert!(gm_sent(root.path(), &world_id, "你好。").contains("合組機制說明"));
}

/// 來源是角色專屬的人物條目、篩完規則與觸發全空：條目不鎖但強制 GM、非人物條目，不進角色線。
#[test]
fn filtered_empty_mechanism_from_character_person_entry_is_forced_gm() {
    let root = TestRoot::new("refactor-visibility-mechanism-person");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let card = import_card(
        root.path(),
        &world_id,
        "莉亞",
        json!([{"comment": "人物規則", "keys": [], "content": "人物規則原文", "constant": true}]),
    );
    let mut source = new_entry(root.path(), &world_id, "人物規則原文");
    source.is_person = true;
    data::upsert_worldbook_entry(root.path(), &world_id, source.clone()).unwrap();
    assert_eq!(
        source.visibility,
        Visibility::Characters(vec![card.clone()])
    );
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    // MVU 來源的桌（世界書 MVU 鷹架會開 incremental）
    state.mechanism.incremental = true;
    state.state.tree = BTreeMap::from([(
        "基础信息".to_owned(),
        data::StateNode::Leaf("未知".to_owned()),
    )]);
    data::write_state(root.path(), &world_id, &state).unwrap();
    let fingerprints = data::identity_fingerprints(root.path(), &world_id).unwrap();
    let outcome = outcome(
        vec![mechanism_entry(
            &source,
            "人物規則接管",
            &["莉亞.好感"],
            Vec::new(),
        )],
        fingerprints,
    );
    apply(root.path(), &world_id, &outcome, &entries_selection(1)).unwrap();

    let entry = new_entry(root.path(), &world_id, "人物規則接管");
    assert!(!entry.locked && !entry.is_person);
    assert_eq!(entry.visibility, Visibility::Gm);
    assert!(!character_sent(root.path(), &world_id, &card, "你好。").contains("人物規則接管"));
    assert!(gm_sent(root.path(), &world_id, "你好。").contains("人物規則接管"));
}
