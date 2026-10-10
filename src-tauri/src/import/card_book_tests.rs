//! 卡內世界書的可見度（worldbook-character-visibility）：角色卡路的條目給這張卡的角色、照觸發規則送；
//! 世界書路給 GM；去重合併可見度、撤銷只還原可見度、匯出往返。
//! 計數一律對整份送出內容串起來算，不從段標往後切；條目文字避開逐字稿裡出現的字。

use super::test_support::TestRoot;
use super::{export_character, import_character, import_character_file, import_worldbook_file};
use crate::data::{self, TranscriptEvent, TranscriptKind, Visibility};
use crate::lanes::{chars_lane_shape, LaneProvider};
use crate::{chat_assembly, receipts};
use serde_json::{json, Value};
use std::path::Path;

const LANG: &str = "zh-TW";

/// 六類條目：常駐、沒命中的 keyword、命中的 keyword、停用、明寫 gm、常駐的輸出格式條目。
fn book_card(name: &str) -> String {
    json!({"data": {
        "name": name,
        "description": "旅人",
        "character_book": {"entries": [
            {"comment": "常駐", "keys": [], "content": "甲常駐內容甲", "constant": true, "enabled": true},
            {"comment": "沙漠", "keys": ["沙漠"], "content": "乙未命中內容乙", "constant": false, "enabled": true},
            {"comment": "燈塔", "keys": ["燈塔"], "content": "丙命中內容丙", "constant": false, "enabled": true},
            {"comment": "停用", "keys": [], "content": "丁停用內容丁", "constant": true, "enabled": false},
            {"comment": "祕密", "keys": [], "content": "戊GM內容戊", "constant": true, "enabled": true,
             "extensions": {"table_tavern": {"visibility": "gm"}}},
            {"comment": "[mvu_update]变量输出格式", "keys": [], "content": "己格式內容己 <StatusBlock>",
             "constant": true, "enabled": true}
        ]}
    }})
    .to_string()
}

fn plain_card(name: &str) -> String {
    json!({"data": {"name": name, "description": "路人"}}).to_string()
}

fn say(root: &Path, world_id: &str, text: &str) {
    let event = TranscriptEvent {
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
    };
    let scene = data::read_state(root, world_id).unwrap().current_scene;
    data::append_transcript(root, world_id, scene, &event).unwrap();
}

fn count(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

/// 角色線一輪（凍結 system＋回合尾段）的整份文字。
fn lane_text(root: &Path, world_id: &str, character_id: &str, hoist: bool) -> (String, String) {
    let card = data::read_character(root, world_id, character_id).unwrap();
    let cards = chat_assembly::active_cards(root, world_id).unwrap();
    let player = data::read_player_card(root, world_id).unwrap();
    let state = data::read_state(root, world_id).unwrap();
    let events = data::read_transcript(root, world_id, state.current_scene).unwrap();
    let (scan, snapshot) =
        chat_assembly::test_character_scan(root, world_id, &card, player.as_ref(), &events, LANG);
    let (frozen, turn) = chat_assembly::character_lane_parts(
        &card,
        &cards,
        player.as_ref(),
        &scan,
        &snapshot,
        &state,
        None,
        LANG,
        hoist,
    );
    let history: String = events
        .iter()
        .map(|event| event.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    (format!("{frozen}\n{history}"), turn.tail)
}

/// API 共線的整份 messages。
fn shared_text(root: &Path, world_id: &str, character_id: &str) -> (String, String) {
    let card = data::read_character(root, world_id, character_id).unwrap();
    let cards = chat_assembly::active_cards(root, world_id).unwrap();
    let player = data::read_player_card(root, world_id).unwrap();
    let state = data::read_state(root, world_id).unwrap();
    let events = data::read_transcript(root, world_id, state.current_scene).unwrap();
    let (scan, snapshot) =
        chat_assembly::test_character_scan(root, world_id, &card, player.as_ref(), &events, LANG);
    let messages = crate::transport::assemble_shared_messages(
        &card,
        &cards,
        player.as_ref(),
        &events,
        &scan,
        &snapshot,
        &state.state,
        &state.mechanism,
        None,
        LANG,
    );
    let all = messages
        .iter()
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    (messages[0].content.clone(), all)
}

/// GM 線兩種組裝（單發 messages、lane 凍結＋尾段）各自的整份文字。
fn gm_texts(root: &Path, world_id: &str) -> [String; 2] {
    let materials = chat_assembly::gm_materials(root, world_id).unwrap();
    let (scope, _) = chat_assembly::gm_scope(&materials);
    let scan = chat_assembly::test_gm_scan(root, world_id, &materials, LANG);
    let single = crate::transport::assemble_gm_messages(
        &materials.world_md,
        &materials.cards,
        materials.player.as_ref(),
        &materials.events,
        &scan,
        &materials.state.state,
        &materials.state.mechanism,
        &scope,
        LANG,
    )
    .iter()
    .map(|message| message.content.clone())
    .collect::<Vec<_>>()
    .join("\n");
    let (frozen, tail) = chat_assembly::gm_lane_parts(&materials, &scan, &scope, "", LANG);
    [single, format!("{frozen}\n{tail}")]
}

/// 角色線看得到常駐、命中、格式條目各恰好一次；看不到沒命中、停用、明寫 GM 的。
fn assert_character_sees_book(text: &str) {
    assert_eq!(count(text, "甲常駐內容甲"), 1, "{text}");
    assert_eq!(count(text, "丙命中內容丙"), 1, "{text}");
    assert_eq!(count(text, "己格式內容己"), 1, "{text}");
    for hidden in ["乙未命中內容乙", "丁停用內容丁", "戊GM內容戊"] {
        assert!(!text.contains(hidden), "{hidden} 不該送: {text}");
    }
}

#[test]
fn character_route_next_turn_sends_card_book_by_trigger_rules() {
    let root = TestRoot::new("card-book-lanes");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let meta = import_character(
        root.path(),
        &world_id,
        book_card("莉亞").as_bytes(),
        "#3366ff",
        LANG,
    )
    .unwrap();
    say(root.path(), &world_id, "我們去看看那座燈塔吧。");
    // 私設不再傾印條目
    let card = data::read_character(root.path(), &world_id, &meta.id).unwrap();
    assert!(!card.private_md.contains("甲常駐內容甲"));

    // 單卡：API 共線、單人在場的 Claude 線、Agy 線都把常駐條目提進 system
    let (system, all) = shared_text(root.path(), &world_id, &meta.id);
    assert_character_sees_book(&all);
    assert!(system.contains("甲常駐內容甲"));
    for provider in [LaneProvider::Claude, LaneProvider::Agy] {
        let hoist = chars_lane_shape(provider, true).hoist_private;
        assert!(hoist);
        let (frozen, tail) = lane_text(root.path(), &world_id, &meta.id, hoist);
        assert_character_sees_book(&format!("{frozen}\n{tail}"));
        assert!(frozen.contains("甲常駐內容甲") && !tail.contains("甲常駐內容甲"));
    }

    // Agy 續聊：stdin 只有本輪 prompt、system 在第一輪——不抹尾段的線，本輪觸發的世界書全在 system、
    // 尾段不放（不然每輪疊一份）；世界書一變 system 就變、整線重開，所以兩輪串起來各條恰好一次
    let hoist = chars_lane_shape(LaneProvider::Agy, false).hoist_private;
    let (frozen, first_tail) = lane_text(root.path(), &world_id, &meta.id, hoist);
    say(root.path(), &world_id, "燈塔上好像有人。");
    let (_, second_tail) = lane_text(root.path(), &world_id, &meta.id, hoist);
    let agy = format!("{frozen}\n{first_tail}\n{second_tail}");
    assert_eq!(count(&agy, "甲常駐內容甲"), 1, "{agy}");
    assert_eq!(count(&agy, "丙命中內容丙"), 1, "{agy}");
    assert!(!first_tail.contains("丙命中內容丙") && !second_tail.contains("丙命中內容丙"));

    // GM 線：常駐、命中、明寫 GM、格式條目都在，常駐恰好一次；沒命中與停用的不送
    for gm in gm_texts(root.path(), &world_id) {
        assert_eq!(count(&gm, "甲常駐內容甲"), 1, "{gm}");
        for seen in ["丙命中內容丙", "戊GM內容戊", "己格式內容己"] {
            assert!(gm.contains(seen), "GM 該看到 {seen}: {gm}");
        }
        assert!(
            !gm.contains("乙未命中內容乙") && !gm.contains("丁停用內容丁"),
            "{gm}"
        );
    }

    // 多卡桌：不 hoist，限定條目走回合尾段機密段；另一張卡看不到這本書
    let other = import_character(
        root.path(),
        &world_id,
        plain_card("路人").as_bytes(),
        "#000000",
        LANG,
    )
    .unwrap();
    let hoist = chars_lane_shape(LaneProvider::Claude, false).hoist_private;
    assert!(!hoist);
    let (frozen, tail) = lane_text(root.path(), &world_id, &meta.id, hoist);
    assert_character_sees_book(&format!("{frozen}\n{tail}"));
    assert!(tail.contains("甲常駐內容甲"));
    let (_, all) = shared_text(root.path(), &world_id, &meta.id);
    assert_character_sees_book(&all);
    let (frozen, tail) = lane_text(root.path(), &world_id, &other.id, hoist);
    let (_, shared) = shared_text(root.path(), &world_id, &other.id);
    for text in [format!("{frozen}\n{tail}"), shared] {
        for hidden in ["甲常駐內容甲", "丙命中內容丙", "己格式內容己"] {
            assert!(!text.contains(hidden), "別的角色看不到 {hidden}");
        }
    }
}

/// 世界書路對等：同一張卡當世界書匯入＝GM 演這張卡，條目給 GM；角色看不到、GM 看得到。
#[test]
fn worldbook_route_gives_the_book_to_the_gm() {
    let root = TestRoot::new("card-book-worldbook-route");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    import_worldbook_file(
        root.path(),
        &world_id,
        book_card("燈塔守").as_bytes(),
        "燈塔守",
        &data::test_exclusive(&world_id),
    )
    .unwrap();
    let entries = data::read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(entries.len(), 6);
    assert!(entries
        .iter()
        .all(|entry| entry.visibility == Visibility::Gm));
    let other = import_character(
        root.path(),
        &world_id,
        plain_card("路人").as_bytes(),
        "#000000",
        LANG,
    )
    .unwrap();
    say(root.path(), &world_id, "我們去看看那座燈塔吧。");
    let (_, shared) = shared_text(root.path(), &world_id, &other.id);
    let (frozen, tail) = lane_text(root.path(), &world_id, &other.id, true);
    for text in [shared, format!("{frozen}\n{tail}")] {
        assert!(!text.contains("甲常駐內容甲") && !text.contains("丙命中內容丙"));
    }
    for gm in gm_texts(root.path(), &world_id) {
        assert_eq!(count(&gm, "甲常駐內容甲"), 1);
        assert!(gm.contains("丙命中內容丙"));
    }
}

fn single_entry_card(name: &str, mut entry: Value) -> String {
    // V2 缺 `enabled` 算停用（照 ST）；測試資料沒寫的當啟用
    entry
        .as_object_mut()
        .unwrap()
        .entry("enabled")
        .or_insert(json!(true));
    // 位置也影響去重；缺欄時 V2 是 after_char、物件形是前，測試資料統一成前
    entry
        .as_object_mut()
        .unwrap()
        .entry("position")
        .or_insert(json!("before_char"));
    json!({"data": {"name": name, "character_book": {"entries": [entry]}}}).to_string()
}

/// 角色卡路：缺欄位、格式壞掉、名單全是別桌的 id，一律當沒寫，給這張卡自己的角色。
#[test]
fn card_route_treats_missing_broken_and_foreign_visibility_as_unset() {
    for (label, extensions) in [
        ("missing", json!({})),
        ("broken", json!({"table_tavern": {"visibility": 42}})),
        (
            "foreign",
            json!({"table_tavern": {"visibility": {"characters": ["別桌的角色"]}}}),
        ),
    ] {
        let root = TestRoot::new(&format!("card-book-visibility-{label}"));
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let card = single_entry_card(
            "莉亞",
            json!({"keys": [], "content": "設定", "constant": true, "extensions": extensions}),
        );
        let meta =
            import_character(root.path(), &world_id, card.as_bytes(), "#3366ff", LANG).unwrap();
        let entries = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(
            entries[0].visibility,
            Visibility::Characters(vec![meta.id.clone()]),
            "{label}"
        );
    }
}

/// 同一張卡匯兩次：去重合併，兩張卡的角色線都拿得到；先世界書路再角色卡路：預設 GM 被收成角色名單。
#[test]
fn duplicate_entries_merge_visibility_across_cards_and_routes() {
    let root = TestRoot::new("card-book-merge");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let first = import_character(
        root.path(),
        &world_id,
        book_card("莉亞").as_bytes(),
        "#3366ff",
        LANG,
    )
    .unwrap();
    let second = import_character(
        root.path(),
        &world_id,
        book_card("莉亞").as_bytes(),
        "#3366ff",
        LANG,
    )
    .unwrap();
    let entries = data::read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(entries.len(), 6);
    let constant = entries
        .iter()
        .find(|entry| entry.content == "甲常駐內容甲")
        .unwrap();
    assert_eq!(
        constant.visibility,
        Visibility::Characters(vec![first.id.clone(), second.id.clone()])
    );
    // 明寫 gm 的那條：兩次都維持 GM
    let secret = entries
        .iter()
        .find(|entry| entry.content == "戊GM內容戊")
        .unwrap();
    assert_eq!(secret.visibility, Visibility::Gm);
    for id in [&first.id, &second.id] {
        let (_, all) = shared_text(root.path(), &world_id, id);
        assert_eq!(count(&all, "甲常駐內容甲"), 1);
    }

    let root = TestRoot::new("card-book-merge-route");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let held = data::test_exclusive(&world_id);
    import_worldbook_file(
        root.path(),
        &world_id,
        book_card("莉亞").as_bytes(),
        "莉亞",
        &held,
    )
    .unwrap();
    let card = import_character(
        root.path(),
        &world_id,
        book_card("莉亞").as_bytes(),
        "#3366ff",
        LANG,
    )
    .unwrap();
    let entries = data::read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(entries.len(), 6);
    let constant = entries
        .iter()
        .find(|entry| entry.content == "甲常駐內容甲")
        .unwrap();
    assert_eq!(
        constant.visibility,
        Visibility::Characters(vec![card.id.clone()])
    );
    // 來源明寫 gm 的重複條目不擴大
    let secret = entries
        .iter()
        .find(|entry| entry.content == "戊GM內容戊")
        .unwrap();
    assert_eq!(secret.visibility, Visibility::Gm);
}

/// 落定的明示 GM（角色卡路明寫 gm、帶來源卡）被同指紋、沒寫可見度的條目命中：維持 GM，只多一張來源卡。
#[test]
fn settled_gm_stays_gm_when_another_card_brings_the_same_entry() {
    let root = TestRoot::new("card-book-settled-gm");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let entry = |extensions: Value| {
        json!({"comment": "祕密", "keys": [], "content": "只有導演知道", "constant": true,
               "extensions": extensions})
    };
    let first = import_character(
        root.path(),
        &world_id,
        single_entry_card("甲", entry(json!({"table_tavern": {"visibility": "gm"}}))).as_bytes(),
        "#3366ff",
        LANG,
    )
    .unwrap();
    let second = import_character(
        root.path(),
        &world_id,
        single_entry_card("乙", entry(json!({}))).as_bytes(),
        "#3366ff",
        LANG,
    )
    .unwrap();
    let raw = data::read_worldbook_raw(root.path(), &world_id).unwrap();
    assert_eq!(raw.len(), 1);
    let value = raw.values().next().unwrap();
    assert_eq!(value["extensions"]["table_tavern"]["visibility"], "gm");
    assert_eq!(
        data::source_cards_of(value),
        vec![first.id.clone(), second.id.clone()]
    );
}

/// 同一段文字但常駐設定不同的兩條各自保留；玩家改過停用與順序後再匯同一張卡，不多出條目。
#[test]
fn fingerprint_keeps_trigger_variants_and_ignores_player_toggles() {
    let root = TestRoot::new("card-book-fingerprint");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let card = json!({"data": {"name": "莉亞", "character_book": {"entries": [
        {"enabled": true, "comment": "傳說", "keys": ["月"], "content": "月下的傳說", "constant": true},
        {"enabled": true, "comment": "傳說", "keys": ["月"], "content": "月下的傳說", "constant": false}
    ]}}})
    .to_string();
    import_character(root.path(), &world_id, card.as_bytes(), "#3366ff", LANG).unwrap();
    let entries = data::read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(entries.len(), 2);
    for entry in entries {
        data::upsert_worldbook_entry(
            root.path(),
            &world_id,
            data::WorldbookEntry {
                disabled: true,
                order: entry.order + 50,
                ..entry
            },
        )
        .unwrap();
    }
    import_character(root.path(), &world_id, card.as_bytes(), "#3366ff", LANG).unwrap();
    assert_eq!(
        data::read_worldbook(root.path(), &world_id).unwrap().len(),
        2
    );
}

fn scaffold_card() -> String {
    json!({"data": {"name": "莉亞", "character_book": {"entries": [
        {"comment": "[initvar]初始", "keys": [], "content": "World:\n  Gold: 100", "enabled": true},
        {"comment": "[initvar]初始", "keys": [], "content": "World:\n  Gold: 100", "enabled": false}
    ]}}})
    .to_string()
}

fn exported_book(root: &Path, world_id: &str, character_id: &str) -> Vec<Value> {
    let path = root.join(format!("{character_id}.json"));
    export_character(root, world_id, character_id, &path).unwrap();
    let value: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    value["data"]["character_book"]["entries"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

/// 鷹架條目一啟一停：匯入後都停用但各自保留，匯出各自還原原卡啟停。帳本開關翻過的照現值，且再匯同卡
/// 不多出條目；啟用後又停用，匯出就是停用。
#[test]
fn scaffold_pairs_keep_their_source_switch_until_the_player_toggles() {
    let root = TestRoot::new("card-book-scaffold");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let meta = import_character(
        root.path(),
        &world_id,
        scaffold_card().as_bytes(),
        "#3366ff",
        LANG,
    )
    .unwrap();
    let entries = data::read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|entry| entry.disabled));
    let mut enabled: Vec<bool> = exported_book(root.path(), &world_id, &meta.id)
        .iter()
        .map(|entry| entry["enabled"].as_bool().unwrap())
        .collect();
    enabled.sort();
    assert_eq!(enabled, vec![false, true]);

    // 帳本開關（upsert 反轉 disabled）：把兩條都打開
    for entry in data::read_worldbook(root.path(), &world_id).unwrap() {
        data::upsert_worldbook_entry(
            root.path(),
            &world_id,
            data::WorldbookEntry {
                disabled: false,
                ..entry
            },
        )
        .unwrap();
    }
    import_character(
        root.path(),
        &world_id,
        scaffold_card().as_bytes(),
        "#3366ff",
        LANG,
    )
    .unwrap();
    assert_eq!(
        data::read_worldbook(root.path(), &world_id).unwrap().len(),
        2
    );
    assert!(exported_book(root.path(), &world_id, &meta.id)
        .iter()
        .all(|entry| entry["enabled"] == true));
    // 啟用後又停用：還原標記已清，匯出照現值（停用）
    for entry in data::read_worldbook(root.path(), &world_id).unwrap() {
        data::upsert_worldbook_entry(
            root.path(),
            &world_id,
            data::WorldbookEntry {
                disabled: true,
                ..entry
            },
        )
        .unwrap();
    }
    assert!(exported_book(root.path(), &world_id, &meta.id)
        .iter()
        .all(|entry| entry["enabled"] == false));
}

fn entry_value(root: &Path, world_id: &str, content: &str) -> (u64, Value) {
    data::read_worldbook_raw(root, world_id)
        .unwrap()
        .into_iter()
        .find(|(_, value)| value["content"] == content)
        .unwrap()
}

/// 擴大既有條目後，玩家改了內容再撤銷：內容保留、可見度與來源卡還原；再撤前一筆，條目照常刪掉。
#[test]
fn undo_restores_only_visibility_of_widened_entries() {
    let root = TestRoot::new("card-book-undo");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let held = data::test_exclusive(&world_id);
    import_worldbook_file(
        root.path(),
        &world_id,
        book_card("莉亞").as_bytes(),
        "莉亞",
        &held,
    )
    .unwrap();
    let (uid, before) = entry_value(root.path(), &world_id, "甲常駐內容甲");
    import_character_file(
        root.path(),
        &world_id,
        book_card("莉亞").as_bytes(),
        "#3366ff",
        LANG,
        &held,
    )
    .unwrap();
    let mut widened = data::read_worldbook(root.path(), &world_id)
        .unwrap()
        .into_iter()
        .find(|entry| entry.uid == uid)
        .unwrap();
    assert!(matches!(widened.visibility, Visibility::Characters(_)));
    widened.content = "玩家改寫的常駐內容".to_owned();
    data::upsert_worldbook_entry(root.path(), &world_id, widened).unwrap();

    receipts::undo_last_import(root.path(), &world_id, &held).unwrap();
    let (_, after) = entry_value(root.path(), &world_id, "玩家改寫的常駐內容");
    for field in ["visibility", "source_cards"] {
        assert_eq!(
            after["extensions"]["table_tavern"].get(field),
            before["extensions"]["table_tavern"].get(field),
            "{field}"
        );
    }
    // 其他沒改過的條目：撤銷後跟匯入前一模一樣，前一筆（世界書路）照常撤得掉
    let report = receipts::undo_last_import(root.path(), &world_id, &held).unwrap();
    assert_eq!(report.kept_entries, 1, "玩家改過內容的那條保留");
    assert_eq!(
        data::read_worldbook(root.path(), &world_id).unwrap().len(),
        1
    );
}

/// 只多了來源卡（可見度沒變）也要記：撤銷後歸屬還原。
#[test]
fn undo_restores_source_cards_when_visibility_did_not_change() {
    let root = TestRoot::new("card-book-undo-cards");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let held = data::test_exclusive(&world_id);
    let entry = |extensions: Value| {
        json!({"comment": "常識", "keys": [], "content": "人人都知道", "constant": true,
               "extensions": extensions})
    };
    let first = import_character_file(
        root.path(),
        &world_id,
        single_entry_card(
            "甲",
            entry(json!({"table_tavern": {"visibility": "public"}})),
        )
        .as_bytes(),
        "#3366ff",
        LANG,
        &held,
    )
    .unwrap();
    import_character_file(
        root.path(),
        &world_id,
        single_entry_card("乙", entry(json!({}))).as_bytes(),
        "#3366ff",
        LANG,
        &held,
    )
    .unwrap();
    let (_, value) = entry_value(root.path(), &world_id, "人人都知道");
    assert_eq!(value["extensions"]["table_tavern"]["visibility"], "public");
    assert_eq!(data::source_cards_of(&value).len(), 2);
    receipts::undo_last_import(root.path(), &world_id, &held).unwrap();
    let (_, value) = entry_value(root.path(), &world_id, "人人都知道");
    assert_eq!(data::source_cards_of(&value), vec![first.value.id.clone()]);
}

/// 同一批多次命中同一條既有條目：收據記最初的值，撤銷回到最初；同批先新建、後被合併的條目整條屬於
/// 這次匯入，撤銷時整條刪、不進可見度收據。
#[test]
fn undo_after_repeated_hits_in_one_batch() {
    let root = TestRoot::new("card-book-undo-batch");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let held = data::test_exclusive(&world_id);
    let book = json!({"entries": {"0": {"uid": 0, "comment": "舊", "key": [], "content": "既有條目",
        "constant": true}}});
    import_worldbook_file(
        root.path(),
        &world_id,
        book.to_string().as_bytes(),
        "書",
        &held,
    )
    .unwrap();
    let (_, before) = entry_value(root.path(), &world_id, "既有條目");
    let public = json!({"table_tavern": {"visibility": "public"}});
    let card = json!({"data": {"name": "莉亞", "character_book": {"entries": [
        {"enabled": true, "comment": "舊", "keys": [], "content": "既有條目", "position": "before_char", "constant": true},
        {"enabled": true, "comment": "舊", "keys": [], "content": "既有條目", "position": "before_char", "constant": true, "extensions": public},
        {"enabled": true, "comment": "新", "keys": [], "content": "這次新建", "position": "before_char", "constant": true},
        {"enabled": true, "comment": "新", "keys": [], "content": "這次新建", "position": "before_char", "constant": true, "extensions": public}
    ]}}})
    .to_string();
    import_character_file(
        root.path(),
        &world_id,
        card.as_bytes(),
        "#3366ff",
        LANG,
        &held,
    )
    .unwrap();
    let (_, merged) = entry_value(root.path(), &world_id, "既有條目");
    assert_eq!(merged["extensions"]["table_tavern"]["visibility"], "public");
    let (_, fresh) = entry_value(root.path(), &world_id, "這次新建");
    assert_eq!(fresh["extensions"]["table_tavern"]["visibility"], "public");

    receipts::undo_last_import(root.path(), &world_id, &held).unwrap();
    let raw = data::read_worldbook_raw(root.path(), &world_id).unwrap();
    assert_eq!(raw.len(), 1, "同批新建的整條撤掉");
    let (_, after) = entry_value(root.path(), &world_id, "既有條目");
    assert_eq!(after, before);
}

/// 玩家改過可見度：撤銷不還原、計入保留；接著撤前一筆，那條被當玩家改過而保留。
#[test]
fn undo_keeps_visibility_the_player_changed() {
    let root = TestRoot::new("card-book-undo-player");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let held = data::test_exclusive(&world_id);
    import_worldbook_file(
        root.path(),
        &world_id,
        book_card("莉亞").as_bytes(),
        "莉亞",
        &held,
    )
    .unwrap();
    import_character_file(
        root.path(),
        &world_id,
        book_card("莉亞").as_bytes(),
        "#3366ff",
        LANG,
        &held,
    )
    .unwrap();
    let (uid, _) = entry_value(root.path(), &world_id, "甲常駐內容甲");
    let mut entry = data::read_worldbook(root.path(), &world_id)
        .unwrap()
        .into_iter()
        .find(|entry| entry.uid == uid)
        .unwrap();
    entry.visibility = Visibility::Public;
    data::upsert_worldbook_entry(root.path(), &world_id, entry).unwrap();
    let report = receipts::undo_last_import(root.path(), &world_id, &held).unwrap();
    assert_eq!(report.kept_entries, 1);
    let (_, value) = entry_value(root.path(), &world_id, "甲常駐內容甲");
    assert_eq!(value["extensions"]["table_tavern"]["visibility"], "public");
    receipts::undo_last_import(root.path(), &world_id, &held).unwrap();
    let left: Vec<String> = data::read_worldbook(root.path(), &world_id)
        .unwrap()
        .into_iter()
        .map(|entry| entry.content)
        .collect();
    assert_eq!(left, vec!["甲常駐內容甲".to_owned()]);
}

/// 世界書路只有合併改寫、沒有新增（來源明寫 public 併進既有條目）：仍留有效收據，撤銷後可見度還原。
#[test]
fn worldbook_route_rewrite_only_import_leaves_a_receipt() {
    let root = TestRoot::new("card-book-rewrite-only");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let held = data::test_exclusive(&world_id);
    let meta = import_character(
        root.path(),
        &world_id,
        single_entry_card(
            "莉亞",
            json!({"comment": "常識", "keys": [], "content": "人人都知道", "constant": true}),
        )
        .as_bytes(),
        "#3366ff",
        LANG,
    )
    .unwrap();
    let book = json!({"entries": {"0": {"uid": 0, "comment": "常識", "key": [],
        "content": "人人都知道", "constant": true,
        "extensions": {"table_tavern": {"visibility": "public"}}}}});
    let imported = import_worldbook_file(
        root.path(),
        &world_id,
        book.to_string().as_bytes(),
        "書",
        &held,
    )
    .unwrap();
    assert_eq!(imported.value.imported, 0);
    assert!(imported.source.is_some(), "只有改寫也要留收據");
    let entries = data::read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(entries[0].visibility, Visibility::Public);
    // 對照：同一本書再匯一次，什麼都沒變＝不留收據（上面那筆收據確實是改寫撐起來的）
    let again = import_worldbook_file(
        root.path(),
        &world_id,
        book.to_string().as_bytes(),
        "書",
        &held,
    )
    .unwrap();
    assert!(again.source.is_none());
    receipts::undo_last_import(root.path(), &world_id, &held).unwrap();
    let entries = data::read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(entries[0].visibility, Visibility::Characters(vec![meta.id]));
}

/// A 桌匯入→匯出→B 桌（新桌）匯入：B 的新角色線看得到條目；V2 欄位照 ST 的寫法。
#[test]
fn exported_card_reimports_into_a_new_table_with_the_book_visible() {
    let root = TestRoot::new("card-book-export");
    let world_a = data::create_world(root.path(), "甲桌").unwrap();
    let card = json!({"data": {
        "name": "莉亞",
        "alternate_greetings": ["第二次見面。", "雨天再訪。"],
        "character_book": {"entries": [
            {"enabled": true, "comment": "常駐", "keys": [], "content": "甲常駐內容甲", "constant": true},
            {"enabled": true, "comment": "月", "keys": ["月"], "secondary_keys": ["夜"], "content": "月夜傳說",
             "selective": true, "position": "after_char", "case_sensitive": true},
            {"enabled": true, "comment": "祕密", "keys": [], "content": "戊GM內容戊", "constant": true,
             "extensions": {"table_tavern": {"visibility": "gm"}}}
        ]}
    }})
    .to_string();
    let source = import_character(root.path(), &world_a, card.as_bytes(), "#3366ff", LANG).unwrap();
    // 編輯器新建的條目（數字位置 0、caseSensitive）與數字非零位置的條目
    data::upsert_worldbook_entry(
        root.path(),
        &world_a,
        data::WorldbookEntry {
            uid: u64::MAX,
            title: "手寫".to_owned(),
            keys: Vec::new(),
            content: "玩家手寫的設定".to_owned(),
            constant: true,
            order: 5,
            disabled: false,
            visibility: Visibility::Characters(vec![source.id.clone()]),
            is_person: false,
            locked: false,
        },
    )
    .unwrap();
    let mut deep = data::worldbook_entry_value(&data::WorldbookEntry {
        uid: 0,
        title: "深處".to_owned(),
        keys: Vec::new(),
        content: "深處的設定".to_owned(),
        constant: true,
        order: 6,
        disabled: false,
        visibility: Visibility::Characters(vec![source.id.clone()]),
        is_person: false,
        locked: false,
    });
    deep["position"] = json!(4);
    data::insert_worldbook_entry_raw(root.path(), &world_a, deep).unwrap();

    let book = exported_book(root.path(), &world_a, &source.id);
    let find = |content: &str| {
        book.iter()
            .find(|entry| entry["content"] == content)
            .cloned()
            .unwrap()
    };
    let moon = find("月夜傳說");
    assert_eq!(moon["secondary_keys"], json!(["夜"]));
    assert_eq!(moon["position"], "after_char");
    assert_eq!(moon["case_sensitive"], true);
    assert_eq!(moon["selective"], true);
    assert_eq!(moon["enabled"], true);
    assert!(
        moon["extensions"].get("table_tavern").is_none(),
        "角色名單不寫出去"
    );
    assert_eq!(
        find("戊GM內容戊")["extensions"]["table_tavern"]["visibility"],
        "gm"
    );
    let written = find("玩家手寫的設定");
    assert_eq!(written["position"], "before_char");
    assert_eq!(written["case_sensitive"], false);
    assert!(written.get("caseSensitive").is_none());
    let deep = find("深處的設定");
    assert_eq!(deep["position"], "after_char");
    assert_eq!(deep["extensions"]["position"], 4);
    for entry in &book {
        let table_tavern = &entry["extensions"]["table_tavern"];
        assert!(table_tavern.get("source_cards").is_none());
        assert!(table_tavern.get("source_disable").is_none());
    }
    let path = root.path().join(format!("{}.json", source.id));
    let exported = std::fs::read(&path).unwrap();
    let value: Value = serde_json::from_slice(&exported).unwrap();
    assert_eq!(
        value["data"]["alternate_greetings"],
        json!(["第二次見面。", "雨天再訪。"])
    );

    let world_b = data::create_world(root.path(), "乙桌").unwrap();
    let target = import_character(root.path(), &world_b, &exported, "#000000", LANG).unwrap();
    let (_, all) = shared_text(root.path(), &world_b, &target.id);
    for seen in ["甲常駐內容甲", "玩家手寫的設定", "深處的設定"] {
        assert_eq!(count(&all, seen), 1, "{seen}: {all}");
    }
    assert!(!all.contains("戊GM內容戊"));
    let reimported = data::read_character(root.path(), &world_b, &target.id).unwrap();
    assert!(reimported.private_md.contains("雨天再訪。"));
}

/// MVU 卡匯出再匯入：規則、初始樹、增量協定都不變；規則條目匯出時回到原卡的啟用值。
#[test]
fn mvu_card_survives_export_and_reimport() {
    let root = TestRoot::new("card-book-mvu");
    let world_a = data::create_world(root.path(), "甲桌").unwrap();
    let card = json!({"data": {"name": "莉亞", "character_book": {"entries": [
        {"comment": "[initvar]初始", "enabled": false, "keys": [], "content": "World:\n  Gold: 100"},
        {"comment": "[mvu_update]变量更新规则", "enabled": true, "keys": [],
         "content": "规则:\n  World:\n    Gold:\n      type: number\n      range: 0-1000\n"}
    ]}}})
    .to_string();
    let source = import_character(root.path(), &world_a, card.as_bytes(), "#3366ff", LANG).unwrap();
    let book = exported_book(root.path(), &world_a, &source.id);
    let rule = book
        .iter()
        .find(|entry| entry["comment"] == "[mvu_update]变量更新规则")
        .unwrap();
    assert_eq!(rule["enabled"], true);
    let initvar = book
        .iter()
        .find(|entry| entry["comment"] == "[initvar]初始")
        .unwrap();
    assert_eq!(initvar["enabled"], false);

    let exported = std::fs::read(root.path().join(format!("{}.json", source.id))).unwrap();
    let world_b = data::create_world(root.path(), "乙桌").unwrap();
    import_character(root.path(), &world_b, &exported, "#000000", LANG).unwrap();
    let a = data::read_state(root.path(), &world_a).unwrap();
    let b = data::read_state(root.path(), &world_b).unwrap();
    assert_eq!(a.mechanism.rules, b.mechanism.rules);
    assert_eq!(a.mechanism.incremental, b.mechanism.incremental);
    assert_eq!(a.state.tree, b.state.tree);
    assert!(!b.state.tree.is_empty());
}

/// 書壞掉：整本讀不了時角色照建、結果帶旗標；單條 enabled 是怪值時照 JS 真假值處理、其餘照匯，重複的略過回報。
#[test]
fn broken_books_are_reported_not_swallowed() {
    let root = TestRoot::new("card-book-broken");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let held = data::test_exclusive(&world_id);
    let broken = json!({"data": {"name": "莉亞", "character_book": {"entries": 5}}}).to_string();
    let imported = import_character_file(
        root.path(),
        &world_id,
        broken.as_bytes(),
        "#3366ff",
        LANG,
        &held,
    )
    .unwrap();
    assert!(imported.book_failed);
    assert!(data::read_character(root.path(), &world_id, &imported.value.id).is_ok());

    let partly = json!({"data": {"name": "凱恩", "character_book": {"entries": [
        {"keys": [], "content": "好的條目", "constant": true, "enabled": true},
        {"keys": [], "content": "怪的條目", "constant": true, "enabled": "yes"},
        {"keys": [], "content": "好的條目", "constant": true, "enabled": true}
    ]}}})
    .to_string();
    let imported = import_character_file(
        root.path(),
        &world_id,
        partly.as_bytes(),
        "#3366ff",
        LANG,
        &held,
    )
    .unwrap();
    assert!(!imported.book_failed);
    assert_eq!(
        imported.book,
        Some(data::WorldbookImport {
            imported: 2,
            skipped: 1
        })
    );
    // enabled 照 JS 真假值："yes" 算啟用
    let entries = data::read_worldbook(root.path(), &world_id).unwrap();
    let contents: Vec<&str> = entries.iter().map(|entry| entry.content.as_str()).collect();
    assert_eq!(contents, ["好的條目", "怪的條目"]);
    assert!(entries.iter().all(|entry| !entry.disabled));
}

/// 這桌匯出的世界書再匯回來：鷹架條目沿用原本的來源停用值——同一桌不多出重複，新桌一啟一停兩條各自
/// 保留，匯出時各自還原。
#[test]
fn exported_worldbook_reimports_scaffolds_without_merging_the_pair() {
    let root = TestRoot::new("card-book-worldbook-roundtrip");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let held = data::test_exclusive(&world_id);
    import_character(
        root.path(),
        &world_id,
        scaffold_card().as_bytes(),
        "#3366ff",
        LANG,
    )
    .unwrap();
    let path = root.path().join("book.json");
    data::export_worldbook(root.path(), &world_id, &path).unwrap();
    let exported = std::fs::read(&path).unwrap();

    import_worldbook_file(root.path(), &world_id, &exported, "書", &held).unwrap();
    assert_eq!(
        data::read_worldbook(root.path(), &world_id).unwrap().len(),
        2
    );

    let fresh = data::create_world(root.path(), "新桌").unwrap();
    import_worldbook_file(
        root.path(),
        &fresh,
        &exported,
        "書",
        &data::test_exclusive(&fresh),
    )
    .unwrap();
    let mut source_disable: Vec<bool> = data::read_worldbook_raw(root.path(), &fresh)
        .unwrap()
        .values()
        .map(|value| {
            value["extensions"]["table_tavern"]["source_disable"]
                .as_bool()
                .unwrap()
        })
        .collect();
    source_disable.sort();
    assert_eq!(source_disable, vec![false, true]);
}

/// 世界書壞掉：匯出整次失敗、不產出檔案（吞掉的話匯出照樣成功，整本角色設定卻不見）；沒有世界書檔不算錯。
#[test]
fn broken_worldbook_fails_the_character_export() {
    let root = TestRoot::new("card-book-export-broken");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let meta = import_character(
        root.path(),
        &world_id,
        plain_card("路人").as_bytes(),
        "#3366ff",
        LANG,
    )
    .unwrap();
    let path = root.path().join("路人.json");
    export_character(root.path(), &world_id, &meta.id, &path).unwrap();
    std::fs::remove_file(&path).unwrap();

    std::fs::write(
        root.path()
            .join(format!("worlds/{world_id}/worldbook.json")),
        "{broken",
    )
    .unwrap();
    assert!(export_character(root.path(), &world_id, &meta.id, &path).is_err());
    assert!(!path.exists());
}
