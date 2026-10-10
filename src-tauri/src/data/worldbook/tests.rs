use super::*;
use crate::data::test_support::*;
use crate::data::*;

#[test]
fn worldbook_missing_returns_empty_and_invalid_json_errors() {
    let root = TestRoot::new("worldbook-missing");
    let world_id = create_world(root.path(), "舊桌").unwrap();
    assert_eq!(read_worldbook(root.path(), &world_id).unwrap(), Vec::new());
    assert_eq!(
        serde_json::to_value(Visibility::Gm).unwrap(),
        serde_json::json!({"type": "gm"})
    );
    assert_eq!(
        serde_json::to_value(Visibility::Characters(vec!["角色代碼".to_owned()])).unwrap(),
        serde_json::json!({"type": "characters", "characters": ["角色代碼"]})
    );

    fs::write(
        root.path()
            .join(format!("worlds/{world_id}/worldbook.json")),
        "{broken",
    )
    .unwrap();
    assert!(read_worldbook(root.path(), &world_id).is_err());
}

#[test]
fn imports_st_worldbook_losslessly_and_round_trips_export() {
    let root = TestRoot::new("worldbook-st-import");
    let source = create_world(root.path(), "來源").unwrap();
    let imported = serde_json::json!({
        "entries": {
            "7": {
                "uid": 7,
                "key": ["dragon", "wyrm"],
                "comment": "龍",
                "content": "古龍沉睡於山下。",
                "constant": false,
                "order": 20,
                "disable": false,
                "sticky": 4,
                "probability": 37
            },
            "9": {
                "uid": 9,
                "key": [],
                "comment": "王都",
                "content": "王都戒嚴。",
                "constant": true,
                "order": 5,
                "disable": false,
                "extensions": {
                    "foreign_app": {"kept": true},
                    "table_tavern": {"visibility": "public"}
                }
            }
        }
    });
    assert_eq!(
        import_worldbook(root.path(), &source, &imported.to_string())
            .unwrap()
            .imported,
        2
    );

    let entries = read_worldbook(root.path(), &source).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].uid, 0);
    assert_eq!(entries[0].title, "龍");
    assert_eq!(entries[0].keys, ["dragon", "wyrm"]);
    assert_eq!(entries[0].visibility, Visibility::Gm);
    assert_eq!(entries[1].uid, 1);
    assert_eq!(entries[1].visibility, Visibility::Public);

    let raw: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.path().join(format!("worlds/{source}/worldbook.json"))).unwrap(),
    )
    .unwrap();
    assert_eq!(raw["entries"]["0"]["sticky"], 4);
    assert_eq!(raw["entries"]["0"]["probability"], 37);
    assert_eq!(
        raw["entries"]["0"]["extensions"]["table_tavern"]["visibility"],
        "gm"
    );
    assert_eq!(
        raw["entries"]["1"]["extensions"]["foreign_app"]["kept"],
        true
    );

    let exported = root.path().join("exported-worldbook.json");
    export_worldbook(root.path(), &source, &exported).unwrap();
    let destination = create_world(root.path(), "目的").unwrap();
    let exported_text = fs::read_to_string(exported).unwrap();
    assert_eq!(
        import_worldbook(root.path(), &destination, &exported_text)
            .unwrap()
            .imported,
        entries.len()
    );
    assert_eq!(
        read_worldbook(root.path(), &destination).unwrap().len(),
        entries.len()
    );
}

#[test]
fn import_skips_entries_identical_to_existing_ones() {
    let root = TestRoot::new("worldbook-dedupe");
    let world_id = create_world(root.path(), "世界").unwrap();
    let book = serde_json::json!({
        "entries": {
            "0": {
                "uid": 0,
                "key": ["城門", "夜"],
                "comment": "城門",
                "content": "城門已關。",
                "constant": false,
                "order": 1,
                "disable": false
            },
            "1": {
                "uid": 1,
                "key": ["市集"],
                "comment": "市集",
                "content": "市集喧鬧。",
                "constant": false,
                "order": 2,
                "disable": false
            }
        }
    });
    let first = import_worldbook(root.path(), &world_id, &book.to_string()).unwrap();
    assert_eq!(
        first,
        WorldbookImport {
            imported: 2,
            skipped: 0
        }
    );

    // 同一份書再匯一次：內容一模一樣，全部略過
    let again = import_worldbook(root.path(), &world_id, &book.to_string()).unwrap();
    assert_eq!(
        again,
        WorldbookImport {
            imported: 0,
            skipped: 2
        }
    );
    assert_eq!(read_worldbook(root.path(), &world_id).unwrap().len(), 2);

    // 內文一樣的略過；改過內文的才算新條目
    let mixed = serde_json::json!({
        "entries": {
            "0": {
                "uid": 0,
                "key": ["城門", "夜"],
                "comment": "城門",
                "content": "城門已關。",
                "constant": false,
                "order": 1,
                "disable": false
            },
            "1": {
                "uid": 1,
                "key": ["市集"],
                "comment": "市集",
                "content": "市集已散。",
                "constant": false,
                "order": 2,
                "disable": false
            }
        }
    });
    let third = import_worldbook(root.path(), &world_id, &mixed.to_string()).unwrap();
    assert_eq!(
        third,
        WorldbookImport {
            imported: 1,
            skipped: 1
        }
    );
    assert_eq!(read_worldbook(root.path(), &world_id).unwrap().len(), 3);

    // 只差空白也是另一條（內文原樣進提示、鍵代換前不 trim）
    for (field, value) in [
        ("content", serde_json::json!("城門已關。\n")),
        ("comment", serde_json::json!(" 城門")),
        ("key", serde_json::json!(["城門", "夜 "])),
    ] {
        let mut entry = book["entries"]["0"].clone();
        entry[field] = value;
        let changed = serde_json::json!({ "entries": { "0": entry } });
        let result = import_worldbook(root.path(), &world_id, &changed.to_string()).unwrap();
        assert_eq!(result.imported, 1, "{field}");
    }
    // 缺 key 與空陣列同一指紋
    let bare = serde_json::json!({ "entries": {
        "0": { "uid": 0, "comment": "空鍵", "content": "空鍵內文", "constant": true },
        "1": { "uid": 1, "key": [], "comment": "空鍵", "content": "空鍵內文", "constant": true }
    } });
    let result = import_worldbook(root.path(), &world_id, &bare.to_string()).unwrap();
    assert_eq!((result.imported, result.skipped), (1, 1));

    // 影響觸發的欄位（鍵順序、機率、計時、群組…）不同＝另一條；停用與 order 不算
    for (field, value) in [
        ("key", serde_json::json!(["夜", "城門"])),
        ("probability", serde_json::json!(50)),
        ("sticky", serde_json::json!(3)),
        ("group", serde_json::json!("門")),
        ("position", serde_json::json!(4)),
        ("keysecondary", serde_json::json!(["門"])),
    ] {
        let mut entry = book["entries"]["0"].clone();
        entry[field] = value;
        let changed = serde_json::json!({ "entries": { "0": entry } });
        let result = import_worldbook(root.path(), &world_id, &changed.to_string()).unwrap();
        assert_eq!(result.imported, 1, "{field}");
    }
    let mut same = book["entries"]["0"].clone();
    same["disable"] = serde_json::json!(true);
    same["order"] = serde_json::json!(99);
    same["sticky"] = serde_json::json!(0);
    same["delayUntilRecursion"] = serde_json::json!(false);
    let result = import_worldbook(
        root.path(),
        &world_id,
        &serde_json::json!({ "entries": { "0": same } }).to_string(),
    )
    .unwrap();
    assert_eq!(result.skipped, 1);
}

/// 機制鷹架條目（[initvar]／[mvu_update]／整棵樹重送巨集）匯入後要被系統關掉，
/// 不再送模型；一般條目完全不受影響。
#[test]
fn import_worldbook_disables_mechanism_scaffold_entries_and_leaves_others_alone() {
    let root = TestRoot::new("worldbook-absorb");
    let world_id = create_world(root.path(), "世界").unwrap();
    let book = serde_json::json!({
        "entries": [
            {
                "keys": ["初始"],
                "comment": "[initvar] 初始值",
                "content": "World:\n  Time: 清晨",
                "enabled": false
            },
            {
                "keys": [],
                "comment": "[mvu_update] 規則",
                "content": "规则:\n  World:\n    HP:\n      type: number",
                "enabled": true
            },
            {
                "keys": [],
                "comment": "整棵樹重送",
                "content": "{{format_message_variable::World}}",
                "enabled": true
            },
            {
                "keys": ["城門"],
                "comment": "城門",
                "content": "城門已關。",
                "enabled": true
            }
        ]
    });
    import_worldbook(root.path(), &world_id, &book.to_string()).unwrap();
    let entries = read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(entries.len(), 4);
    for entry in &entries {
        let should_be_disabled = entry.title != "城門";
        assert_eq!(entry.disabled, should_be_disabled, "{}", entry.title);
    }
}

/// 同前綴 `[mvu_update]` 但抽不出欄位規則的格式說明（变量输出格式／强调），app 協定沒接手，
/// 照原卡啟停：原本啟用的保留啟用，原本停用的（例如「前端」）維持停用；規則表照樣停用。
#[test]
fn import_worldbook_keeps_mvu_format_entries_as_the_card_set_them() {
    let root = TestRoot::new("worldbook-mvu-format");
    let world_id = create_world(root.path(), "世界").unwrap();
    let book = serde_json::json!({
        "entries": [
            {
                "keys": [],
                "comment": "[mvu_update]变量更新规则",
                "content": "---\n变量更新规则:\n  主角:\n    好感度:\n      type: number\n      range: 0~100\n",
                "enabled": true
            },
            {
                "keys": [],
                "comment": "[mvu_update]变量输出格式",
                "content": "---\n变量输出格式:\n  rule:\n    - you must output the update analysis and the actual update commands at once in the end of the next reply\n    - the update commands works like the **JSON Patch (RFC 6902)** standard\n  format: |-\n    <UpdateVariable>\n    <Analysis>$(IN ENGLISH, no more than 80 words)\n    - ${calculate time passed: ...}\n    </Analysis>\n    <JSONPatch>\n    [\n      { \"op\": \"replace\", \"path\": \"${/path/to/variable}\", \"value\": \"${new_value}\" }\n    ]\n    </JSONPatch>\n    </UpdateVariable>\n",
                "enabled": true
            },
            {
                "keys": [],
                "comment": "[mvu_update]变量输出格式强调",
                "content": "<must>\n回复末尾必须输出 <UpdateVariable> 区块。\n</must>",
                "enabled": true
            },
            {
                "keys": [],
                "comment": "[mvu_update]前端",
                "content": "<StatusBar>前端显示用</StatusBar>",
                "enabled": false
            }
        ]
    });
    import_worldbook(root.path(), &world_id, &book.to_string()).unwrap();
    let entries = read_worldbook(root.path(), &world_id).unwrap();
    let disabled = |title: &str| {
        entries
            .iter()
            .find(|entry| entry.title == title)
            .unwrap_or_else(|| panic!("缺 {title}"))
            .disabled
    };
    assert!(disabled("[mvu_update]变量更新规则"));
    assert!(!disabled("[mvu_update]变量输出格式"));
    assert!(!disabled("[mvu_update]变量输出格式强调"));
    assert!(disabled("[mvu_update]前端"));
}

#[test]
fn dedupe_keeps_first_of_each_duplicate_group() {
    let root = TestRoot::new("worldbook-dedupe-command");
    let world_id = create_world(root.path(), "世界").unwrap();
    let entry = |uid: u64, comment: &str, content: &str, order: u64| {
        serde_json::json!({
            "uid": uid,
            "key": ["k"],
            "comment": comment,
            "content": content,
            "constant": false,
            "order": order,
            "disable": false
        })
    };
    let book = serde_json::json!({
        "entries": {
            "0": entry(0, "城門", "城門已關。", 1),
            "1": entry(1, "市集", "市集喧鬧。", 2),
            "2": entry(2, "城門", "城門已關。", 3),
            "3": entry(3, "城門", "城門大開。", 4)
        }
    });
    write_worldbook_value(root.path(), &world_id, &book).unwrap();

    assert_eq!(dedupe_worldbook(root.path(), &world_id).unwrap(), 1);
    let entries = read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(entries.len(), 3);
    // 留下的是排在最前面那條，被留下的內容一條不少
    assert_eq!(entries[0].uid, 0);
    assert_eq!(entries[1].uid, 1);
    assert_eq!(entries[2].content, "城門大開。");
    // 再按一次沒東西可清
    assert_eq!(dedupe_worldbook(root.path(), &world_id).unwrap(), 0);
}

#[test]
fn imports_character_book_mapping_and_appends_unique_uids() {
    let root = TestRoot::new("worldbook-character-book");
    let world_id = create_world(root.path(), "世界").unwrap();
    let first = serde_json::json!({
        "entries": {
            "12": {
                "uid": 12,
                "key": ["existing"],
                "comment": "既有",
                "content": "內容",
                "constant": false,
                "order": 1,
                "disable": false
            }
        }
    });
    import_worldbook(root.path(), &world_id, &first.to_string()).unwrap();

    let character_book = serde_json::json!({
        "entries": [
            {
                "keys": ["gate"],
                "secondary_keys": ["night"],
                "comment": "城門",
                "content": "城門已關。",
                "constant": false,
                "insertion_order": 42,
                "enabled": false,
                "priority": 8
            },
            {
                "keys": ["market"],
                "comment": "市集",
                "content": "市集喧鬧。",
                "constant": false,
                "insertion_order": 43,
                "enabled": true
            }
        ]
    });
    assert_eq!(
        import_worldbook(root.path(), &world_id, &character_book.to_string())
            .unwrap()
            .imported,
        2
    );

    let entries = read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(
        entries.iter().map(|entry| entry.uid).collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert_eq!(entries[1].keys, ["gate"]);
    assert_eq!(entries[1].order, 42);
    assert!(entries[1].disabled);
    assert!(!entries[2].disabled);

    let raw: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            root.path()
                .join(format!("worlds/{world_id}/worldbook.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(raw["entries"]["1"]["keysecondary"][0], "night");
    assert_eq!(raw["entries"]["1"]["priority"], 8);
    assert!(raw["entries"]["1"].get("keys").is_none());
    assert!(raw["entries"]["1"].get("enabled").is_none());
}

#[test]
fn upsert_preserves_unknown_fields_allocates_uid_and_deletes() {
    let root = TestRoot::new("worldbook-upsert");
    let world_id = create_world(root.path(), "世界").unwrap();
    let imported = serde_json::json!({
        "entries": {
            "5": {
                "uid": 5,
                "key": ["old"],
                "comment": "舊標題",
                "content": "舊內容",
                "constant": false,
                "order": 1,
                "disable": false,
                "sticky": 99
            }
        }
    });
    import_worldbook(root.path(), &world_id, &imported.to_string()).unwrap();

    let mut updated = worldbook_entry(0, "新標題");
    updated.visibility = Visibility::Characters(vec!["角色代碼".to_owned()]);
    assert_eq!(
        upsert_worldbook_entry(root.path(), &world_id, updated.clone()).unwrap(),
        0
    );
    let raw: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            root.path()
                .join(format!("worlds/{world_id}/worldbook.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(raw["entries"]["0"]["sticky"], 99);
    assert_eq!(raw["entries"]["0"]["comment"], "新標題");
    assert_eq!(
        raw["entries"]["0"]["extensions"]["table_tavern"]["visibility"]["characters"][0],
        "角色代碼"
    );

    let allocated =
        upsert_worldbook_entry(root.path(), &world_id, worldbook_entry(u64::MAX, "新增")).unwrap();
    assert_eq!(allocated, 1);
    let raw: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            root.path()
                .join(format!("worlds/{world_id}/worldbook.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(raw["entries"]["1"]["selective"], true);
    assert_eq!(raw["entries"]["1"]["probability"], 100);
    assert_eq!(raw["entries"]["1"]["useProbability"], true);
    assert_eq!(raw["entries"]["1"]["depth"], 4);
    assert_eq!(raw["entries"]["1"]["displayIndex"], 0);

    delete_worldbook_entry(root.path(), &world_id, 0).unwrap();
    assert_eq!(
        read_worldbook(root.path(), &world_id)
            .unwrap()
            .into_iter()
            .map(|entry| entry.uid)
            .collect::<Vec<_>>(),
        [1]
    );
}

#[test]
fn worldbook_entry_to_character_moves_content_and_keeps_other_entries() {
    let root = TestRoot::new("worldbook-entry-to-character");
    let world_id = create_world(root.path(), "世界").unwrap();
    let mut source = worldbook_entry(u64::MAX, "霧港船長");
    source.content = "第一段\n\n第二段".to_owned();
    let source_uid = upsert_worldbook_entry(root.path(), &world_id, source).unwrap();
    let other_uid = upsert_worldbook_entry(
        root.path(),
        &world_id,
        worldbook_entry(u64::MAX, "留下的條目"),
    )
    .unwrap();

    let meta = worldbook_entry_to_character(
        root.path(),
        &world_id,
        source_uid,
        "#123456".to_owned(),
        false,
    )
    .unwrap();

    assert_eq!(meta.name, "霧港船長");
    assert_eq!(
        read_character(root.path(), &world_id, &meta.id)
            .unwrap()
            .public_md,
        "第一段\n\n第二段"
    );
    assert_eq!(
        read_worldbook(root.path(), &world_id)
            .unwrap()
            .iter()
            .map(|entry| entry.uid)
            .collect::<Vec<_>>(),
        [other_uid]
    );
}

#[test]
fn worldbook_entry_to_player_card_sets_state_and_rejects_second_card() {
    let root = TestRoot::new("worldbook-entry-to-player");
    let world_id = create_world(root.path(), "世界").unwrap();
    let first_uid =
        upsert_worldbook_entry(root.path(), &world_id, worldbook_entry(u64::MAX, "玩家")).unwrap();
    let second_uid = upsert_worldbook_entry(
        root.path(),
        &world_id,
        worldbook_entry(u64::MAX, "候補玩家"),
    )
    .unwrap();

    let player = worldbook_entry_to_character(
        root.path(),
        &world_id,
        first_uid,
        "#abcdef".to_owned(),
        true,
    )
    .unwrap();
    assert_eq!(
        read_state(root.path(), &world_id).unwrap().player_card_id,
        Some(player.id)
    );

    assert_eq!(
        worldbook_entry_to_character(
            root.path(),
            &world_id,
            second_uid,
            "#abcdef".to_owned(),
            true,
        )
        .unwrap_err()
        .to_string(),
        UiMsg::PlayerCardExists.to_string()
    );
    assert!(read_worldbook(root.path(), &world_id)
        .unwrap()
        .iter()
        .any(|entry| entry.uid == second_uid));
}

#[test]
fn worldbook_entry_to_character_rejects_empty_title_without_deleting() {
    let root = TestRoot::new("worldbook-entry-empty-title");
    let world_id = create_world(root.path(), "世界").unwrap();
    let uid =
        upsert_worldbook_entry(root.path(), &world_id, worldbook_entry(u64::MAX, "  ")).unwrap();

    assert_eq!(
        worldbook_entry_to_character(root.path(), &world_id, uid, "#abcdef".to_owned(), false,)
            .unwrap_err()
            .to_string(),
        UiMsg::EntryUntitled.to_string()
    );
    assert!(read_worldbook(root.path(), &world_id)
        .unwrap()
        .iter()
        .any(|entry| entry.uid == uid));
}

#[test]
fn character_to_worldbook_entry_moves_archived_card_and_private_content() {
    let root = TestRoot::new("character-to-worldbook-entry");
    let world_id = create_world(root.path(), "世界").unwrap();
    let mut card = character_card(&new_id(), "封存船長");
    card.archived = true;
    card.public_md = "公開設定".to_owned();
    card.private_md = "GM 秘密".to_owned();
    write_character(root.path(), &world_id, &card).unwrap();

    character_to_worldbook_entry(root.path(), &world_id, &card.id, "zh-TW").unwrap();

    let entries = read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].title, "封存船長");
    assert_eq!(entries[0].content, "公開設定\n\n## 私有\nGM 秘密");
    assert!(entries[0].constant);
    assert_eq!(entries[0].visibility, Visibility::Gm);
    assert_eq!(entries[0].order, 100);
    assert!(read_character(root.path(), &world_id, &card.id).is_err());
}

/// 卡轉世界書的私有段標照轉換當下的介面語系寫進條目內文。
#[test]
fn character_to_worldbook_entry_writes_private_heading_in_ui_language() {
    for (lang, expected) in [
        ("en", "Public\n\n## Private\nSecret"),
        ("ja", "Public\n\n## 非公開\nSecret"),
        ("fr", "Public\n\n## Privé\nSecret"),
        ("zh-HK", "Public\n\n## 私有\nSecret"),
    ] {
        let root = TestRoot::new(&format!("character-to-worldbook-{lang}"));
        let world_id = create_world(root.path(), "世界").unwrap();
        let mut card = character_card(&new_id(), "船長");
        card.public_md = "Public".to_owned();
        card.private_md = "Secret".to_owned();
        write_character(root.path(), &world_id, &card).unwrap();
        character_to_worldbook_entry(root.path(), &world_id, &card.id, lang).unwrap();
        assert_eq!(
            read_worldbook(root.path(), &world_id).unwrap()[0].content,
            expected
        );
    }
}

#[test]
fn character_to_worldbook_entry_converts_active_card_but_rejects_player_card() {
    let root = TestRoot::new("character-to-worldbook-active");
    let world_id = create_world(root.path(), "世界").unwrap();
    let active = character_card(&new_id(), "還在桌上");
    write_character(root.path(), &world_id, &active).unwrap();
    character_to_worldbook_entry(root.path(), &world_id, &active.id, "zh-TW").unwrap();
    let entries = read_worldbook(root.path(), &world_id).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].title, "還在桌上");
    assert!(read_character(root.path(), &world_id, &active.id).is_err());

    let player = character_card(&new_id(), "玩家");
    write_character(root.path(), &world_id, &player).unwrap();
    let mut state = read_state(root.path(), &world_id).unwrap();
    state.player_card_id = Some(player.id.clone());
    write_state(root.path(), &world_id, &state).unwrap();
    assert_eq!(
        character_to_worldbook_entry(root.path(), &world_id, &player.id, "zh-TW")
            .unwrap_err()
            .to_string(),
        UiMsg::PlayerCardNotConvertible.to_string()
    );
    assert!(read_character(root.path(), &world_id, &player.id).is_ok());
}

#[test]
fn character_to_worldbook_entry_converts_auto_hidden_card() {
    let root = TestRoot::new("character-to-worldbook-auto-hidden");
    let world_id = create_world(root.path(), "世界").unwrap();
    let card = character_card(&new_id(), "下場的配角");
    write_character(root.path(), &world_id, &card).unwrap();
    crate::data::character::set_character_auto_hidden(root.path(), &world_id, &card.id, true)
        .unwrap();

    character_to_worldbook_entry(root.path(), &world_id, &card.id, "zh-TW").unwrap();

    assert_eq!(
        read_worldbook(root.path(), &world_id).unwrap()[0].title,
        "下場的配角"
    );
    assert!(read_character(root.path(), &world_id, &card.id).is_err());
}

/// 回合或換幕持共用許可期間不轉：不寫條目、不刪卡。
#[test]
fn character_to_worldbook_entry_is_rejected_while_a_write_permit_is_held() {
    let root = TestRoot::new("character-to-worldbook-busy");
    let world_id = create_world(root.path(), "世界").unwrap();
    let card = character_card(&new_id(), "說到一半");
    write_character(root.path(), &world_id, &card).unwrap();

    let permit = world_write_permit(&world_id).unwrap();
    assert_eq!(
        character_to_worldbook_entry(root.path(), &world_id, &card.id, "zh-TW")
            .unwrap_err()
            .to_string(),
        UiMsg::WorldBusy.to_string()
    );
    assert!(read_worldbook(root.path(), &world_id).unwrap().is_empty());
    assert!(read_character(root.path(), &world_id, &card.id).is_ok());

    drop(permit);
    character_to_worldbook_entry(root.path(), &world_id, &card.id, "zh-TW").unwrap();
}

/// 轉走之後換幕：present 名單還寫著它的名字，結算也不會把卡檔寫回來，名單不再列出它。
#[test]
fn converted_card_is_not_recreated_by_the_next_scene() {
    let root = TestRoot::new("character-to-worldbook-next-scene");
    let world_id = create_world(root.path(), "世界").unwrap();
    let gone = character_card(&new_id(), "狐狸");
    let stays = character_card(&new_id(), "熊");
    write_character(root.path(), &world_id, &gone).unwrap();
    write_character(root.path(), &world_id, &stays).unwrap();
    let mut state = read_state(root.path(), &world_id).unwrap();
    state
        .state
        .table
        .insert("present".to_owned(), "狐狸、熊".to_owned());
    write_state(root.path(), &world_id, &state).unwrap();
    append_transcript(
        root.path(),
        &world_id,
        state.current_scene,
        &TranscriptEvent {
            id: None,
            message_vars: None,
            vars_rev: None,
            vars_epoch: None,
            turn_key: None,
            action_id: None,
            raw: None,
            ts: "2026-10-02T00:00:00+08:00".to_owned(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: TranscriptKind::Narration,
            text: "狐狸與熊在營火邊。".to_owned(),
            state: None,
            truncated: false,
            gm_only: false,
            marker: None,
            opening: false,
        },
    )
    .unwrap();

    character_to_worldbook_entry(root.path(), &world_id, &gone.id, "zh-TW").unwrap();
    begin_next_scene(root.path(), &world_id, "摘要", None).unwrap();

    assert!(read_character(root.path(), &world_id, &gone.id).is_err());
    let ids: Vec<_> = list_characters(root.path(), &world_id)
        .unwrap()
        .into_iter()
        .map(|meta| meta.id)
        .collect();
    assert_eq!(ids, vec![stays.id]);
}

/// 世界書真的寫不進去（不是讀取失敗）：回錯、卡還在。
#[cfg(unix)]
#[test]
fn character_to_worldbook_entry_keeps_card_when_worldbook_write_fails() {
    use std::os::unix::fs::PermissionsExt;

    let root = TestRoot::new("character-to-worldbook-write-fails");
    let world_id = create_world(root.path(), "世界").unwrap();
    upsert_worldbook_entry(
        root.path(),
        &world_id,
        worldbook_entry(u64::MAX, "既有條目"),
    )
    .unwrap();
    let card = character_card(&new_id(), "保住我");
    write_character(root.path(), &world_id, &card).unwrap();
    let path = worldbook_path(root.path(), &world_id).unwrap();
    let before = fs::read(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();

    let result = character_to_worldbook_entry(root.path(), &world_id, &card.id, "zh-TW");

    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(result.is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(read_character(root.path(), &world_id, &card.id).is_ok());
}

#[test]
fn new_worldbook_entry_is_first_and_shifts_display_indices() {
    let root = TestRoot::new("worldbook-new-first");
    let world_id = create_world(root.path(), "世界").unwrap();
    write_worldbook_fixture(
        &root,
        &world_id,
        serde_json::json!({
            "10": {
                "uid": 10, "comment": "甲", "order": 10, "displayIndex": 0
            },
            "20": {
                "uid": 20, "comment": "乙", "order": 20, "displayIndex": 1
            }
        }),
    );

    let uid =
        upsert_worldbook_entry(root.path(), &world_id, worldbook_entry(u64::MAX, "新增")).unwrap();
    assert_eq!(read_worldbook(root.path(), &world_id).unwrap()[0].uid, uid);
    let raw = read_worldbook_fixture(&root, &world_id);
    assert_eq!(raw["entries"]["10"]["displayIndex"], 1);
    assert_eq!(raw["entries"]["20"]["displayIndex"], 2);
    assert_eq!(raw["entries"][uid.to_string()]["displayIndex"], 0);
}

#[test]
fn reordering_worldbook_entries_applies_the_given_order() {
    let root = TestRoot::new("worldbook-reorder");
    let world_id = create_world(root.path(), "世界").unwrap();
    write_worldbook_fixture(
        &root,
        &world_id,
        serde_json::json!({
            "0": {"uid": 0, "comment": "甲", "displayIndex": 0},
            "1": {"uid": 1, "comment": "乙", "displayIndex": 1},
            "2": {"uid": 2, "comment": "丙", "displayIndex": 2}
        }),
    );

    // 跨多格拖曳：最後一筆拉到最前
    reorder_worldbook_entries(root.path(), &world_id, &[2, 0, 1]).unwrap();
    assert_eq!(
        read_worldbook(root.path(), &world_id)
            .unwrap()
            .iter()
            .map(|entry| entry.uid)
            .collect::<Vec<_>>(),
        [2, 0, 1]
    );
    reorder_worldbook_entries(root.path(), &world_id, &[0, 1, 2]).unwrap();
    assert_eq!(
        read_worldbook(root.path(), &world_id)
            .unwrap()
            .iter()
            .map(|entry| entry.uid)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
}

#[test]
fn reordering_worldbook_keeps_unlisted_entries_after_the_listed_ones() {
    let root = TestRoot::new("worldbook-reorder-partial");
    let world_id = create_world(root.path(), "世界").unwrap();
    write_worldbook_fixture(
        &root,
        &world_id,
        serde_json::json!({
            "0": {"uid": 0, "comment": "甲", "displayIndex": 0},
            "1": {"uid": 1, "comment": "乙", "displayIndex": 1},
            "2": {"uid": 2, "comment": "丙", "displayIndex": 2}
        }),
    );

    // uid 9 不存在應被忽略；沒送到的 0 依原順序接在後面
    reorder_worldbook_entries(root.path(), &world_id, &[2, 9, 1]).unwrap();

    assert_eq!(
        read_worldbook(root.path(), &world_id)
            .unwrap()
            .iter()
            .map(|entry| entry.uid)
            .collect::<Vec<_>>(),
        [2, 1, 0]
    );
}

#[test]
fn reordering_legacy_worldbook_entries_normalizes_display_indices() {
    let root = TestRoot::new("worldbook-reorder-legacy");
    let world_id = create_world(root.path(), "世界").unwrap();
    write_worldbook_fixture(
        &root,
        &world_id,
        serde_json::json!({
            "7": {"uid": 7, "comment": "丙"},
            "3": {"uid": 3, "comment": "甲"},
            "5": {"uid": 5, "comment": "乙"}
        }),
    );

    reorder_worldbook_entries(root.path(), &world_id, &[5, 3, 7]).unwrap();

    assert_eq!(
        read_worldbook(root.path(), &world_id)
            .unwrap()
            .iter()
            .map(|entry| entry.uid)
            .collect::<Vec<_>>(),
        [5, 3, 7]
    );
    let raw = read_worldbook_fixture(&root, &world_id);
    let mut indices = raw["entries"]
        .as_object()
        .unwrap()
        .values()
        .map(|entry| entry["displayIndex"].as_u64().unwrap())
        .collect::<Vec<_>>();
    indices.sort_unstable();
    assert_eq!(indices, [0, 1, 2]);
}

#[test]
fn reordering_worldbook_entries_preserves_order_and_unknown_fields() {
    let root = TestRoot::new("worldbook-reorder-lossless");
    let world_id = create_world(root.path(), "世界").unwrap();
    write_worldbook_fixture(
        &root,
        &world_id,
        serde_json::json!({
            "0": {
                "uid": 0, "comment": "甲", "order": 91, "displayIndex": 0,
                "foreign": {"nested": true}
            },
            "1": {
                "uid": 1, "comment": "乙", "order": 7, "displayIndex": 1,
                "sticky": 42
            }
        }),
    );

    reorder_worldbook_entries(root.path(), &world_id, &[1, 0]).unwrap();

    let raw = read_worldbook_fixture(&root, &world_id);
    assert_eq!(raw["entries"]["0"]["order"], 91);
    assert_eq!(raw["entries"]["1"]["order"], 7);
    assert_eq!(
        raw["entries"]["0"]["foreign"],
        serde_json::json!({"nested": true})
    );
    assert_eq!(raw["entries"]["1"]["sticky"], 42);
}
