use super::*;
use crate::data;
use crate::refactor::test_support::*;
use crate::refactor::{apply, RefactorSelection};
use serde_json::json;

fn outcome_of(names: &[&str]) -> RefactorOutcome {
    serde_json::from_value(json!({
        "characters": names.iter().map(|name| json!({
            "name": name, "emoji": "🙂", "public_md": "", "private_md": "",
            "source_uids": [], "solo_entry_md": format!("{name} 的條目"),
        })).collect::<Vec<_>>(),
    }))
    .unwrap()
}

fn saved_card(root: &std::path::Path, world_id: &str) -> RefactorCardFile {
    parse_card(
        &data::read_refactor_outcome(root, world_id)
            .unwrap()
            .unwrap(),
    )
    .unwrap()
}

/// 不連續勾選（0、2）＋玩家指定 2：落檔封套的映射逐 index 顯式列出，沒建卡的是 None，
/// id 對得上桌上的卡，玩家卡資訊同時留下邏輯 index 與本地 id。
#[test]
fn apply_saves_envelope_with_explicit_index_map() {
    let root = TestRoot::new("card-file-map");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let outcome = outcome_of(&["亞瑟", "梅林", "桂妮薇兒"]);
    let selection = RefactorSelection {
        player_index: Some(2),
        ..no_player_selection(vec![0, 2])
    };
    apply(root.path(), &world_id, &outcome, &selection).unwrap();

    let card = saved_card(root.path(), &world_id);
    assert_eq!(card.format, CARD_FORMAT);
    assert_eq!(card.version, CARD_VERSION);
    assert_eq!(card.outcome, outcome);
    let applied = card.applied.unwrap();
    let ids: Vec<Option<String>> = applied
        .characters
        .iter()
        .map(|item| item.character_id.clone())
        .collect();
    assert_eq!(
        applied
            .characters
            .iter()
            .map(|item| item.outcome_index)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert!(ids[1].is_none());
    for index in [0, 2] {
        let id = ids[index].as_deref().unwrap();
        let card = data::read_character(root.path(), &world_id, id).unwrap();
        assert_eq!(card.name, outcome.characters[index].name);
    }
    assert_eq!(applied.player_index, Some(2));
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(applied.player_card_id, state.player_card_id);
    assert_eq!(applied.player_card_id, ids[2]);
}

/// 二次套用整份覆寫：映射換成第二次建的卡，沒指定玩家就沒有玩家資訊。
#[test]
fn second_apply_overwrites_saved_envelope() {
    let root = TestRoot::new("card-file-overwrite");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let outcome = outcome_of(&["亞瑟"]);
    apply(
        root.path(),
        &world_id,
        &outcome,
        &no_player_selection(vec![0]),
    )
    .unwrap();
    let first = saved_card(root.path(), &world_id).applied.unwrap();

    apply(
        root.path(),
        &world_id,
        &outcome,
        &no_player_selection(vec![0]),
    )
    .unwrap();
    let second = saved_card(root.path(), &world_id).applied.unwrap();
    assert_ne!(
        first.characters[0].character_id,
        second.characters[0].character_id
    );
    assert_eq!(second.player_index, None);
    assert_eq!(second.player_card_id, None);
}

/// 舊版落檔是裸 RefactorOutcome：照樣讀得回來，applied 為 None；對外輸出成封套。
#[test]
fn legacy_bare_outcome_still_parses() {
    let outcome = outcome_of(&["亞瑟"]);
    let card = parse_card(&serde_json::to_string_pretty(&outcome).unwrap()).unwrap();
    assert_eq!(card.outcome, outcome);
    assert_eq!(card.applied, None);
    let exported: serde_json::Value = serde_json::to_value(card.for_export()).unwrap();
    assert_eq!(exported["format"], CARD_FORMAT);
    assert!(exported.get("applied").is_none());
}

/// 對外版本去掉來源桌本地的 player_card_id，保留邏輯 player_index。
#[test]
fn export_drops_local_player_card_id() {
    let card = RefactorCardFile::new(
        outcome_of(&["亞瑟"]),
        Some(RefactorApplied {
            characters: vec![RefactorAppliedCharacter {
                outcome_index: 0,
                character_id: Some("c1".to_owned()),
            }],
            player_index: Some(0),
            player_card_id: Some("c1".to_owned()),
        }),
    );
    let text = serde_json::to_string(&card.for_export()).unwrap();
    assert!(!text.contains("player_card_id"));
    let back = parse_card(&text).unwrap();
    assert_eq!(back.applied.unwrap().player_index, Some(0));
}

fn envelope(applied: serde_json::Value) -> String {
    json!({
        "format": CARD_FORMAT,
        "version": 1,
        "outcome": outcome_of(&["甲", "乙"]),
        "applied": applied,
    })
    .to_string()
}

fn is_invalid(text: &str) -> bool {
    matches!(
        parse_card(text).map_err(|error| error.to_string()),
        Err(message) if message.contains("refactor_card_invalid")
    )
}

#[test]
fn applied_integrity_violations_are_rejected() {
    let ok = json!({ "characters": [
        { "outcome_index": 0, "character_id": "a" },
        { "outcome_index": 1, "character_id": null },
    ], "player_index": 0 });
    assert!(parse_card(&envelope(ok)).is_ok());

    for bad in [
        // 沒覆蓋全部 index
        json!({ "characters": [{ "outcome_index": 0, "character_id": "a" }], "player_index": null }),
        // index 重複
        json!({ "characters": [
            { "outcome_index": 0, "character_id": "a" },
            { "outcome_index": 0, "character_id": "b" },
        ], "player_index": null }),
        // index 越界
        json!({ "characters": [
            { "outcome_index": 0, "character_id": "a" },
            { "outcome_index": 2, "character_id": "b" },
        ], "player_index": null }),
        // 空字串 id
        json!({ "characters": [
            { "outcome_index": 0, "character_id": " " },
            { "outcome_index": 1, "character_id": null },
        ], "player_index": null }),
        // id 重複
        json!({ "characters": [
            { "outcome_index": 0, "character_id": "a" },
            { "outcome_index": 1, "character_id": "a" },
        ], "player_index": null }),
        // 玩家 index 沒有卡
        json!({ "characters": [
            { "outcome_index": 0, "character_id": "a" },
            { "outcome_index": 1, "character_id": null },
        ], "player_index": 1 }),
        // id 型別不對
        json!({ "characters": [
            { "outcome_index": 0, "character_id": 7 },
            { "outcome_index": 1, "character_id": null },
        ], "player_index": null }),
    ] {
        assert!(is_invalid(&envelope(bad.clone())), "應拒收：{bad}");
    }
}

#[test]
fn envelope_format_and_version_are_checked() {
    let outcome = outcome_of(&["甲"]);
    let with = |format: serde_json::Value, version: serde_json::Value| {
        json!({ "format": format, "version": version, "outcome": outcome }).to_string()
    };
    assert!(parse_card(&with(json!(CARD_FORMAT), json!(1))).is_ok());
    assert!(is_invalid(&with(json!("other-format"), json!(1))));
    assert!(is_invalid(&with(json!(CARD_FORMAT), json!(0))));
    assert!(is_invalid(&with(json!(CARD_FORMAT), json!("1"))));
    assert_eq!(
        parse_card(&with(json!(CARD_FORMAT), json!(2)))
            .unwrap_err()
            .to_string(),
        UiMsg::RefactorCardNewer.to_string()
    );
    assert!(is_invalid("[]"));
    assert!(is_invalid("not json"));
}

/// 存檔原子寫：覆寫途中部分寫入或改名失敗，舊封套逐位元不變。
#[test]
fn saved_card_write_is_atomic() {
    let root = TestRoot::new("card-file-atomic");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    data::write_refactor_outcome(root.path(), &world_id, "{\"old\":1}").unwrap();
    {
        let _guard = data::WriteFailGuard::partial_ending("refactor-outcome.json.tmp", 1);
        assert!(data::write_refactor_outcome(root.path(), &world_id, "{\"new\":2}").is_err());
    }
    {
        let _guard = data::RenameFailGuard::fail(1);
        assert!(data::write_refactor_outcome(root.path(), &world_id, "{\"new\":2}").is_err());
    }
    assert_eq!(
        data::read_refactor_outcome(root.path(), &world_id)
            .unwrap()
            .as_deref(),
        Some("{\"old\":1}")
    );
}

/// 回歸（Sol 反例）：套用尾端存檔寫一半失敗時，舊存檔不變、套用照樣回 Ok 並標 card_save_failed、
/// 收據照記——撤銷把多出來的卡收回。
#[test]
fn save_failure_keeps_old_card_and_receipt() {
    let root = TestRoot::new("card-file-save-fail");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let outcome = outcome_of(&["亞瑟"]);
    apply(
        root.path(),
        &world_id,
        &outcome,
        &no_player_selection(vec![0]),
    )
    .unwrap();
    let before = data::read_refactor_outcome(root.path(), &world_id)
        .unwrap()
        .unwrap();
    let cards_before = data::list_characters(root.path(), &world_id).unwrap().len();

    let result = {
        let _guard = data::WriteFailGuard::partial_ending("refactor-outcome.json.tmp", 1);
        apply_recorded(
            root.path(),
            &world_id,
            &outcome,
            &no_player_selection(vec![0]),
        )
    };
    assert!(result.summary.card_save_failed);
    assert_eq!(
        data::read_refactor_outcome(root.path(), &world_id)
            .unwrap()
            .unwrap(),
        before
    );
    assert_eq!(
        data::list_characters(root.path(), &world_id).unwrap().len(),
        cards_before + 1
    );
    crate::receipts::undo_last_import(
        root.path(),
        &world_id,
        &crate::data::test_exclusive(&world_id),
    )
    .unwrap();
    assert_eq!(
        data::list_characters(root.path(), &world_id).unwrap().len(),
        cards_before
    );
}
