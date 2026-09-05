use super::super::*;
use super::super::test_support::*;
use crate::data::{self, FieldRule, Visibility};
use crate::mechanism;
use crate::receipts;
use std::collections::BTreeMap;


    /// 整條淘汰（rule 5／判官 rule 1–4，span 空）且玩家沒放回的既有條目：套用時停用、
    /// 快照進 rewritten_entries，undo 覆寫回原樣；半條淘汰（span 非空）的條目其餘段落
    /// 還在服役，不停用（GUI 回歸：A 桌「格式」「COT」constant 條目照常注入）。
    #[test]
    fn apply_disables_whole_entry_drops_and_undo_restores() {
        let root = TestRoot::new("disable-dropped");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let whole_uid = seed_entry(root.path(), &world_id, "格式", "<mainPage>卡片介面輸出協定</mainPage>");
        let partial_uid = seed_entry(root.path(), &world_id, "混合條目", "第一段。\n\n第二段。");

        let outcome = RefactorOutcome {
            mode: Some("characters".to_owned()),
            characters: Vec::new(),
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: vec![
                crate::refactor_assemble::RefactorDroppedEntry {
                    uid: whole_uid.to_string(),
                    span: String::new(),
                    title: "格式".to_owned(),
                    content: "<mainPage>卡片介面輸出協定</mainPage>".to_owned(),
                    rule: 5,
                },
                crate::refactor_assemble::RefactorDroppedEntry {
                    uid: partial_uid.to_string(),
                    span: "s1".to_owned(),
                    title: "混合條目".to_owned(),
                    content: "第二段。".to_owned(),
                    rule: 2,
                },
            ],
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = no_player_selection(Vec::new());

        let result = apply_recorded(root.path(), &world_id, &outcome, &selection);
        assert_eq!(result.summary.rewritten_entries, 1);
        assert_eq!(result.rewritten_entries.len(), 1);
        assert_eq!(result.rewritten_entries[0].uid, whole_uid);
        assert!(!result.rewritten_entries[0].disabled);

        let worldbook = data::read_worldbook(root.path(), &world_id).unwrap();
        let whole = worldbook.iter().find(|entry| entry.uid == whole_uid).unwrap();
        assert!(whole.disabled);
        assert_eq!(whole.content, "<mainPage>卡片介面輸出協定</mainPage>");
        let partial = worldbook.iter().find(|entry| entry.uid == partial_uid).unwrap();
        assert!(!partial.disabled);

        receipts::undo_last_import(root.path(), &world_id).unwrap();
        let restored = data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .into_iter()
            .find(|entry| entry.uid == whole_uid)
            .unwrap();
        assert!(!restored.disabled);
    }
    #[test]
    fn apply_selected_rewritten_entries_creates_locked_mechanism_merges_rules_logs_and_deletes_shared_source() {
        let root = TestRoot::new("rewritten-entries-full");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "舊設定", "舊世界書全文");
        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: None,
            mechanisms: Vec::new(),
            entries: vec![
                crate::refactor_ai::RefactorNewEntry {
                    title: "新世界觀".to_owned(),
                    kind: "setting".to_owned(),
                    content: "重寫的世界觀".to_owned(),
                    source_uids: vec![source_uid.to_string()],
                    rules: BTreeMap::new(),
                    triggers: Vec::new(),
                    meta: None,
                },
                crate::refactor_ai::RefactorNewEntry {
                    title: "新戰鬥規則".to_owned(),
                    kind: "mechanism".to_owned(),
                    content: "重寫的戰鬥說明".to_owned(),
                    source_uids: vec![source_uid.to_string()],
                    rules: BTreeMap::from([(
                        "World.戰鬥值".to_owned(),
                        FieldRule::for_kind(data::FieldKind::Number),
                    )]),
                    triggers: Vec::new(),
                    meta: None,
                },
            ],
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: false,
            mechanism_indices: Vec::new(),
            entry_indices: vec![0, 1],
            player_index: None,
        };

        let result = apply(root.path(), &world_id, &outcome, &selection).unwrap();
        assert_eq!(result.summary.new_entries, 2);
        assert_eq!(result.summary.deleted_entries, 1);
        let entries = data::read_worldbook(root.path(), &world_id).unwrap();
        assert!(!entries.iter().any(|entry| entry.uid == source_uid));
        assert!(entries.iter().any(|entry| entry.title == "新世界觀" && !entry.locked));
        assert!(entries.iter().any(|entry| entry.title == "新戰鬥規則" && entry.locked));
        assert!(data::read_state(root.path(), &world_id)
            .unwrap()
            .mechanism
            .rules
            .contains_key("World.戰鬥值"));
        assert_eq!(
            mechanism::read_ledger(root.path(), &world_id)
                .entries
                .iter()
                .find(|entry| entry.title == "新戰鬥規則")
                .unwrap()
                .kind,
            mechanism::RecordKind::Absorbed
        );
    }
    #[test]
    fn apply_partially_selected_rewritten_entries_keeps_shared_source() {
        let root = TestRoot::new("rewritten-entries-partial");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "舊設定", "舊世界書全文");
        let entry = |title: &str| crate::refactor_ai::RefactorNewEntry {
            title: title.to_owned(),
            kind: "setting".to_owned(),
            content: format!("{title} 重寫內容"),
            source_uids: vec![source_uid.to_string()],
            rules: BTreeMap::new(),
            triggers: Vec::new(),
            meta: None,
        };
        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: None,
            mechanisms: Vec::new(),
            entries: vec![entry("新設定甲"), entry("新設定乙")],
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: false,
            mechanism_indices: Vec::new(),
            entry_indices: vec![0],
            player_index: None,
        };

        let result = apply(root.path(), &world_id, &outcome, &selection).unwrap();
        assert_eq!(result.summary.new_entries, 1);
        assert_eq!(result.summary.deleted_entries, 0);
        let entries = data::read_worldbook(root.path(), &world_id).unwrap();
        assert!(entries.iter().any(|entry| entry.uid == source_uid));
        assert!(entries.iter().any(|entry| entry.title == "新設定甲"));
    }
    #[test]
    fn legacy_outcome_without_entries_deserializes_and_applies() {
        let outcome: RefactorOutcome = serde_json::from_value(serde_json::json!({
            "characters": [],
            "interface": null,
            "mechanisms": [],
            "deletable_shared_uids": []
        }))
        .unwrap();
        assert!(outcome.entries.is_empty());
        let root = TestRoot::new("legacy-outcome-no-entries");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let selection: RefactorSelection = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(apply(root.path(), &world_id, &outcome, &selection).unwrap().summary.new_entries, 0);
    }
    /// 匯出重構產物包 (a)：apply 成功後 data::read_refactor_outcome 讀得回來，serde 反序列化
    /// 回 RefactorOutcome 與送進去的一致（round-trip）。
    #[test]
    fn apply_writes_refactor_outcome_file_readable_and_round_trips() {
        let root = TestRoot::new("export-outcome-roundtrip");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let uid = seed_entry(root.path(), &world_id, "亞瑟人物设定", "亞瑟：劍術高超。");

        let outcome = RefactorOutcome {
            mode: None,
            characters: vec![character("亞瑟", &[uid])],
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = no_player_selection(vec![0]);

        apply(root.path(), &world_id, &outcome, &selection).unwrap();

        let saved = data::read_refactor_outcome(root.path(), &world_id).unwrap().unwrap();
        let round_tripped: RefactorOutcome = serde_json::from_str(&saved).unwrap();
        assert_eq!(round_tripped, outcome);
    }
    /// 匯出重構產物包 (b)：apply 後跑既有 undo 流程，產物檔仍然存在——undo 與收據不動這個檔
    /// （零改動）。
    #[test]
    fn apply_then_undo_keeps_refactor_outcome_file() {
        let root = TestRoot::new("export-outcome-undo-keeps-file");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let uid = seed_entry(root.path(), &world_id, "亞瑟人物设定", "亞瑟：劍術高超。");

        let outcome = RefactorOutcome {
            mode: None,
            characters: vec![character("亞瑟", &[uid])],
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = no_player_selection(vec![0]);

        apply_recorded(root.path(), &world_id, &outcome, &selection);
        assert!(data::read_refactor_outcome(root.path(), &world_id).unwrap().is_some());

        receipts::undo_last_import(root.path(), &world_id).unwrap();
        assert!(data::read_refactor_outcome(root.path(), &world_id).unwrap().is_some());
    }
    /// 重構卡匯到新桌（來源 uid 在這桌不存在）：來源刪除不得誤刪剛落地的新條目（uid 撞號），
    /// undo 要把新條目（含 locked 機制條目）整批收回，不得靠「已刪來源」快照把它們插回來——
    /// 插回來的 locked 條目沒有編輯／刪除鈕，會變成玩家永遠動不了的孤兒。
    #[test]
    fn undo_removes_new_entries_including_locked() {
        let root = TestRoot::new("undo-removes-new-entries");
        let world_id = data::create_world(root.path(), "酒館").unwrap();

        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: None,
            mechanisms: Vec::new(),
            entries: vec![
                crate::refactor_ai::RefactorNewEntry {
                    title: "獸人部落文化".to_owned(),
                    kind: "setting".to_owned(),
                    content: "氏族階級制。".to_owned(),
                    source_uids: vec!["1".to_owned()],
                    rules: std::collections::BTreeMap::new(),
                    triggers: Vec::new(),
                    meta: None,
                },
                crate::refactor_ai::RefactorNewEntry {
                    title: "天數計時".to_owned(),
                    kind: "mechanism".to_owned(),
                    content: "每日推進。".to_owned(),
                    source_uids: vec!["1".to_owned()],
                    rules: std::collections::BTreeMap::from([(
                        "World.淪陷天數".to_owned(),
                        FieldRule::for_kind(crate::data::FieldKind::Number),
                    )]),
                    triggers: Vec::new(),
                    meta: None,
                },
            ],
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: false,
            mechanism_indices: Vec::new(),
            entry_indices: vec![0, 1],
            player_index: None,
        };

        apply_recorded(root.path(), &world_id, &outcome, &selection);
        let applied = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(applied.len(), 2);
        assert!(applied.iter().any(|entry| entry.locked));

        receipts::undo_last_import(root.path(), &world_id).unwrap();
        let after = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(
            after.iter().map(|entry| entry.title.as_str()).collect::<Vec<_>>(),
            Vec::<&str>::new(),
            "undo 後新條目應整批收回"
        );
    }
    /// 包 2：entries[].meta 有值時，套用後的世界書條目直接照抄 keys/constant/order/disabled/
    /// visibility/is_person；沒有 meta 的條目走現行預設（keys=[]／constant=false／order 用
    /// 遞增計數／visibility=Gm／is_person=false）——兩種條目同一次套用互不干擾。
    #[test]
    fn apply_entry_with_meta_preserves_fields_without_meta_uses_defaults() {
        let root = TestRoot::new("entry-meta-preservation");
        let world_id = data::create_world(root.path(), "酒館").unwrap();

        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: None,
            mechanisms: Vec::new(),
            entries: vec![
                crate::refactor_ai::RefactorNewEntry {
                    title: "照搬條目".to_owned(),
                    kind: "setting".to_owned(),
                    content: "原文照搬。".to_owned(),
                    source_uids: vec!["1".to_owned()],
                    rules: BTreeMap::new(),
                    triggers: Vec::new(),
                    meta: Some(crate::refactor_ai::RefactorEntryMeta {
                        keys: vec!["關鍵字".to_owned()],
                        constant: true,
                        order: 99,
                        disabled: true,
                        visibility: Visibility::Characters(vec!["char-1".to_owned()]),
                        is_person: true,
                    }),
                },
                crate::refactor_ai::RefactorNewEntry {
                    title: "新組裝條目".to_owned(),
                    kind: "setting".to_owned(),
                    content: "本地組裝的新內容。".to_owned(),
                    source_uids: vec!["2".to_owned()],
                    rules: BTreeMap::new(),
                    triggers: Vec::new(),
                    meta: None,
                },
            ],
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: false,
            mechanism_indices: Vec::new(),
            entry_indices: vec![0, 1],
            player_index: None,
        };

        apply(root.path(), &world_id, &outcome, &selection).unwrap();
        let entries = data::read_worldbook(root.path(), &world_id).unwrap();

        let with_meta = entries
            .iter()
            .find(|entry| entry.title == "照搬條目")
            .unwrap();
        assert_eq!(with_meta.keys, vec!["關鍵字".to_owned()]);
        assert!(with_meta.constant);
        assert_eq!(with_meta.order, 99);
        assert!(with_meta.disabled);
        assert_eq!(
            with_meta.visibility,
            Visibility::Characters(vec!["char-1".to_owned()])
        );
        assert!(with_meta.is_person);

        let without_meta = entries
            .iter()
            .find(|entry| entry.title == "新組裝條目")
            .unwrap();
        assert!(without_meta.keys.is_empty());
        assert!(!without_meta.constant);
        assert!(!without_meta.disabled);
        assert_eq!(without_meta.visibility, Visibility::Gm);
        assert!(!without_meta.is_person);
    }
