use super::super::*;
use super::super::test_support::*;
use super::super::types::RefactorMechanism;
use crate::data::{self, FieldRule};
use crate::mechanism;
use crate::receipts;
use std::collections::BTreeMap;


    /// (e) 帳本轉換：來源條目原本在帳本裡是 Skipped（例如認不出的 EJS），套用機制後帳本要
    /// 改記 Absorbed——玩家在帳本分頁看到的是「已被收編」，不再是「跳過」。
    #[test]
    fn apply_mechanism_deletes_source_after_recording_absorption() {
        let root = TestRoot::new("ledger-skipped-to-absorbed");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "詭異的機制腳本", "<% 認不出的 EJS %>");
        mechanism::append_log(
            root.path(),
            &world_id,
            0,
            &[mechanism::Record {
                kind: mechanism::RecordKind::Skipped,
                path: "詭異的機制腳本".to_owned(),
                detail: "卡片腳本認不出來，沒轉成觸發表，預設不送模型。".to_owned(),
            }],
        );

        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: None,
            mechanisms: vec![RefactorMechanism {
                source_uid: source_uid.to_string(),
                rules: BTreeMap::from([(
                    "World.詭異值".to_owned(),
                    FieldRule::for_kind(data::FieldKind::Number),
                )]),
                triggers: Vec::new(),
            }],
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: false,
            mechanism_indices: vec![0],
            entry_indices: Vec::new(),
            player_index: None,
        };

        apply(root.path(), &world_id, &outcome, &selection).unwrap();

        assert!(!data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .iter()
            .any(|entry| entry.uid == source_uid));
    }
    /// (f) 帳本新增：純散文機制條目（帳本裡原本沒有這條）套用後要新增一筆 Absorbed 記錄。
    #[test]
    fn apply_mechanism_deletes_source_with_no_prior_ledger_record() {
        let root = TestRoot::new("ledger-new-absorbed");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "純散文機制", "打鬥時擲骰決勝負。");

        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: None,
            mechanisms: vec![RefactorMechanism {
                source_uid: source_uid.to_string(),
                rules: BTreeMap::from([(
                    "World.戰鬥值".to_owned(),
                    FieldRule::for_kind(data::FieldKind::Number),
                )]),
                triggers: Vec::new(),
            }],
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: false,
            mechanism_indices: vec![0],
            entry_indices: Vec::new(),
            player_index: None,
        };

        assert!(mechanism::read_ledger(root.path(), &world_id).entries.is_empty());

        apply(root.path(), &world_id, &outcome, &selection).unwrap();

        assert!(!data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .iter()
            .any(|entry| entry.uid == source_uid));
    }
    /// (g) undo 帳本回退：套用前是 Skipped，套用後變 Absorbed，undo 之後帳本要退回原本的
    /// Skipped 記錄。
    #[test]
    fn apply_mechanism_then_undo_restores_ledger_to_previous_state() {
        let root = TestRoot::new("ledger-undo");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "詭異的機制腳本二號", "<% 認不出的 EJS %>");
        mechanism::append_log(
            root.path(),
            &world_id,
            0,
            &[mechanism::Record {
                kind: mechanism::RecordKind::Skipped,
                path: "詭異的機制腳本二號".to_owned(),
                detail: "卡片腳本認不出來，沒轉成觸發表，預設不送模型。".to_owned(),
            }],
        );

        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: None,
            mechanisms: vec![RefactorMechanism {
                source_uid: source_uid.to_string(),
                rules: BTreeMap::from([(
                    "World.詭異值二".to_owned(),
                    FieldRule::for_kind(data::FieldKind::Number),
                )]),
                triggers: Vec::new(),
            }],
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: false,
            mechanism_indices: vec![0],
            entry_indices: Vec::new(),
            player_index: None,
        };

        apply_recorded(root.path(), &world_id, &outcome, &selection);
        assert!(!data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .iter()
            .any(|entry| entry.uid == source_uid));

        receipts::undo_last_import(root.path(), &world_id).unwrap();

        let after_undo = mechanism::read_ledger(root.path(), &world_id);
        let entry = after_undo
            .entries
            .iter()
            .find(|entry| entry.title == "詭異的機制腳本二號")
            .unwrap();
        assert_eq!(entry.kind, mechanism::RecordKind::Skipped);
    }
