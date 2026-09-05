use super::super::*;
use super::super::test_support::*;
use crate::data::{self, CharacterCard, Tier};
use crate::receipts;


    /// (a) 合併升格＋玩家指定：兩條專屬來源併成一張卡、指定為玩家 → 兩條來源條目整條刪除、
    /// 玩家卡指定寫進 state → undo → 角色卡、來源條目（原樣回來，不是新造的 is_person 條目）、
    /// 玩家卡指定都回原樣。
    #[test]
    fn apply_merges_multi_source_person_deletes_exclusive_entries_and_sets_player_then_undo_restores() {
        let root = TestRoot::new("merge-player");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let bio_uid = seed_entry(root.path(), &world_id, "亞瑟人物设定", "亞瑟：劍術高超。");
        let personality_uid = seed_entry(root.path(), &world_id, "亞瑟性格", "亞瑟：沉默寡言。");

        let outcome = RefactorOutcome {
            mode: None,
            characters: vec![character("亞瑟", &[bio_uid, personality_uid])],
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            player_index: Some(0),
            ..no_player_selection(vec![0])
        };

        let before_len = data::read_worldbook(root.path(), &world_id).unwrap().len();
        let result = apply_recorded(root.path(), &world_id, &outcome, &selection);
        assert_eq!(result.summary.new_characters, 1);
        assert_eq!(result.summary.deleted_entries, 2);
        assert!(result.summary.player_assigned);

        let character_id = result.character_ids[0].clone();
        assert!(data::read_character(root.path(), &world_id, &character_id).is_ok());
        let state = data::read_state(root.path(), &world_id).unwrap();
        assert_eq!(state.player_card_id.as_deref(), Some(character_id.as_str()));
        let entries = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(entries.len(), before_len - 2);
        assert!(entries.iter().all(|entry| entry.uid != bio_uid && entry.uid != personality_uid));

        receipts::undo_last_import(root.path(), &world_id).unwrap();
        assert!(data::read_character(root.path(), &world_id, &character_id).is_err());
        let state_after = data::read_state(root.path(), &world_id).unwrap();
        assert!(state_after.player_card_id.is_none());
        let restored = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(restored.len(), before_len);
        // 刪除後靠 upsert 插回，新 uid 跟原本不同——原樣回來看內容，不看 uid 是否相同。
        let restored_bio = restored.iter().find(|entry| entry.content == "亞瑟：劍術高超。").unwrap();
        assert!(!restored_bio.is_person);
        assert!(restored.iter().any(|entry| entry.content == "亞瑟：沉默寡言。"));
    }
    /// (b) 玩家卡限制：桌上已有玩家卡時，指定第二張要整批失敗、不寫入任何東西
    /// （沿用 data.rs 既有的一桌一張限制與錯誤訊息）。
    #[test]
    fn apply_rejects_second_player_card_and_writes_nothing() {
        let root = TestRoot::new("second-player");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let existing_player = CharacterCard {
            id: data::new_id(),
            name: "既有玩家".to_owned(),
            color: "#fff".to_owned(),
            avatar: "🧑".to_owned(),
            tier: Tier::Balanced,
            show_image: true,
            archived: false,
            gen_prompt: String::new(),
            public_md: String::new(),
            private_md: String::new(),
        };
        data::write_character(root.path(), &world_id, &existing_player).unwrap();
        let mut state = data::read_state(root.path(), &world_id).unwrap();
        state.player_card_id = Some(existing_player.id.clone());
        data::write_state(root.path(), &world_id, &state).unwrap();

        let uid = seed_entry(root.path(), &world_id, "新來的人", "新來的人的設定");
        let outcome = RefactorOutcome {
            mode: None,
            characters: vec![character("新來的人", &[uid])],
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            player_index: Some(0),
            ..no_player_selection(vec![0])
        };

        let error = apply(root.path(), &world_id, &outcome, &selection).unwrap_err();
        assert_eq!(error.to_string(), "這桌已經有玩家卡");
        assert_eq!(data::list_characters(root.path(), &world_id).unwrap().len(), 0); // list_characters 排除玩家卡本人，新卡也沒寫入
        assert!(data::read_worldbook(root.path(), &world_id).unwrap().iter().any(|entry| entry.uid == uid)); // 沒被刪
    }
    /// (c) 沒勾的人一律維持現行機制，各自生一條獨立 is_person 條目；勾了的人專屬來源條目
    /// 整條刪除。兩件事對每個人各自獨立成立，不因為同一次套用裡有人選中有人沒選就互相影響。
    #[test]
    fn apply_unselected_person_gets_independent_person_entry_selected_persons_source_deleted() {
        let root = TestRoot::new("zero-selected");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let picked_uid = seed_entry(root.path(), &world_id, "被選中的條目", "會被升格的人");
        let ignored_uid = seed_entry(root.path(), &world_id, "沒被勾的條目", "沒人被勾的人");

        let outcome = RefactorOutcome {
            mode: None,
            characters: vec![character("阿明", &[picked_uid]), character("小華", &[ignored_uid])],
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = no_player_selection(vec![0]); // 只勾阿明；小華（index 1）沒勾

        let before_len = data::read_worldbook(root.path(), &world_id).unwrap().len();
        let result = apply(root.path(), &world_id, &outcome, &selection).unwrap();
        assert_eq!(result.summary.new_characters, 1);
        assert_eq!(result.summary.new_entries, 1); // 小華獨立成一條 is_person 條目
        assert_eq!(result.summary.deleted_entries, 1); // 阿明的專屬來源條目整條刪除

        let entries = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(entries.len(), before_len); // 少一條（阿明來源刪除）多一條（小華新增），淨零
        assert!(!entries.iter().any(|entry| entry.uid == picked_uid));
        let ignored_entry = entries.iter().find(|entry| entry.uid == ignored_uid).unwrap();
        assert_eq!(ignored_entry.content, "沒人被勾的人"); // 小華的原始專屬條目原樣不動
        let person_entry = entries.iter().find(|entry| entry.is_person).unwrap();
        assert_eq!(person_entry.title, "小華");
    }
    /// (b2) 七人共用一條合集，只勾兩人：兩張角色卡＋五條獨立 is_person 條目；沒有收尾判定，
    /// 合集條目原樣保留（基準是優先保留）→ undo → 角色卡與新條目都回原樣。
    #[test]
    fn apply_partial_group_selection_creates_person_entries_for_the_rest_and_keeps_shared_source() {
        let root = TestRoot::new("partial-group");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "旅團", "七人旅團的合集設定");

        let names = ["甲", "乙", "丙", "丁", "戊", "己", "庚"];
        let characters: Vec<RefactorCharacter> =
            names.iter().map(|name| character(name, &[source_uid])).collect();
        let outcome = RefactorOutcome {
            mode: None,
            characters,
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = no_player_selection(vec![0, 1]); // 甲、乙

        let before_entries = data::read_worldbook(root.path(), &world_id).unwrap().len();
        let result = apply_recorded(root.path(), &world_id, &outcome, &selection);
        assert_eq!(result.summary.new_characters, 2);
        assert_eq!(result.summary.new_entries, 5);
        assert_eq!(result.summary.deleted_entries, 0);
        assert_eq!(data::list_characters(root.path(), &world_id).unwrap().len(), 2);

        let entries = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(entries.len(), before_entries + 5); // 合集條目原樣保留，沒被刪也沒被改寫
        let person_names: Vec<&str> = entries
            .iter()
            .filter(|entry| entry.is_person)
            .map(|entry| entry.title.as_str())
            .collect();
        assert_eq!(person_names.len(), 5);
        for name in ["丙", "丁", "戊", "己", "庚"] {
            assert!(person_names.contains(&name));
        }
        let source_entry = entries.iter().find(|entry| entry.uid == source_uid).unwrap();
        assert_eq!(source_entry.content, "七人旅團的合集設定");

        receipts::undo_last_import(root.path(), &world_id).unwrap();
        assert_eq!(data::list_characters(root.path(), &world_id).unwrap().len(), 0);
        let after_undo = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(after_undo.len(), before_entries);
    }
    /// (h) 共用合集條目，finishing 判定可刪，但只有一位共用者被勾：條目原樣保留
    /// （要點 7：判斷不出來或還有人沒勾就不動）。
    #[test]
    fn apply_shared_uid_kept_when_not_all_owners_selected() {
        let root = TestRoot::new("shared-uid-partial");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let shared_uid = seed_entry(root.path(), &world_id, "角色速览", "霍玄：……長老：……");

        let outcome = RefactorOutcome {
            mode: None,
            characters: vec![character("霍玄", &[shared_uid]), character("長老", &[shared_uid])],
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: vec![shared_uid.to_string()],
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = no_player_selection(vec![0]); // 只勾霍玄

        let result = apply(root.path(), &world_id, &outcome, &selection).unwrap();
        assert_eq!(result.summary.deleted_entries, 0);
        assert!(data::read_worldbook(root.path(), &world_id).unwrap().iter().any(|entry| entry.uid == shared_uid));
    }
    /// (i) 共用合集條目，finishing 判定可刪、全部共用者都被勾：整條刪除，且只刪一次
    /// （兩人都指到同一 uid，不因此刪兩次）。
    #[test]
    fn apply_shared_uid_deleted_once_when_all_owners_selected_and_verdict_deletable() {
        let root = TestRoot::new("shared-uid-full");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let shared_uid = seed_entry(root.path(), &world_id, "角色速览", "霍玄：……長老：……");

        let outcome = RefactorOutcome {
            mode: None,
            characters: vec![character("霍玄", &[shared_uid]), character("長老", &[shared_uid])],
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: vec![shared_uid.to_string()],
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = no_player_selection(vec![0, 1]);

        let result = apply(root.path(), &world_id, &outcome, &selection).unwrap();
        assert_eq!(result.summary.deleted_entries, 1);
        assert!(!data::read_worldbook(root.path(), &world_id).unwrap().iter().any(|entry| entry.uid == shared_uid));
    }
    /// (j) 共用合集條目沒有收尾判定（不在 deletable_shared_uids）：即使全部共用者都被勾，
    /// 一樣原樣保留——沒把握就不動是保底，不是漏做。
    #[test]
    fn apply_shared_uid_kept_without_finish_verdict_even_if_all_owners_selected() {
        let root = TestRoot::new("shared-uid-no-verdict");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let shared_uid = seed_entry(root.path(), &world_id, "角色速览", "霍玄：……長老：……");

        let outcome = RefactorOutcome {
            mode: None,
            characters: vec![character("霍玄", &[shared_uid]), character("長老", &[shared_uid])],
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = no_player_selection(vec![0, 1]);

        let result = apply(root.path(), &world_id, &outcome, &selection).unwrap();
        assert_eq!(result.summary.deleted_entries, 0);
    }
