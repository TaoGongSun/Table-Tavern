use super::super::*;
use super::super::test_support::*;
use crate::data::{self, FieldKind, FieldRule, InjectLevel, StateNode, UpdateMode};
use crate::receipts;
use std::collections::BTreeMap;

use super::super::interface::normalize_interface_paths;

    /// 玩法標記持久化（refactor-mode-split）：套用把 outcome.mode 寫進桌面狀態；
    /// characters 並清掉舊 interface 套用殘留的殼檔（fallback 抑制第一層）。
    #[test]
    fn apply_persists_refactor_mode_and_characters_removes_stale_shell() {
        let root = TestRoot::new("mode-persist");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        data::write_interface_shell(root.path(), &world_id, "<html>舊殼</html>").unwrap();
        let outcome = RefactorOutcome {
            mode: Some("characters".to_owned()),
            characters: Vec::new(),
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        apply(root.path(), &world_id, &outcome, &no_player_selection(Vec::new())).unwrap();
        let state = data::read_state(root.path(), &world_id).unwrap();
        assert_eq!(state.refactor_mode.as_deref(), Some("characters"));
        assert!(data::read_interface_shell(root.path(), &world_id).unwrap().is_none());

        // interface 模式：mode 照寫、殼不動（這輪沒產殼也不清別輪的——interface 殼由套用介面那段管）
        let mut interface_outcome = outcome.clone();
        interface_outcome.mode = Some("interface".to_owned());
        apply(root.path(), &world_id, &interface_outcome, &no_player_selection(Vec::new())).unwrap();
        assert_eq!(
            data::read_state(root.path(), &world_id).unwrap().refactor_mode.as_deref(),
            Some("interface")
        );

        // 舊產物（mode 缺席）：不動既有標記
        let mut legacy = outcome.clone();
        legacy.mode = None;
        apply(root.path(), &world_id, &legacy, &no_player_selection(Vec::new())).unwrap();
        assert_eq!(
            data::read_state(root.path(), &world_id).unwrap().refactor_mode.as_deref(),
            Some("interface")
        );
    }
    /// mode 只收二值列舉：手改匯入檔的大小寫／未知值不落地，trim 後合法照收。
    #[test]
    fn apply_ignores_invalid_mode_values() {
        let root = TestRoot::new("mode-enum");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let mut outcome = RefactorOutcome {
            mode: Some("Characters".to_owned()),
            characters: Vec::new(),
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        apply(root.path(), &world_id, &outcome, &no_player_selection(Vec::new())).unwrap();
        assert_eq!(data::read_state(root.path(), &world_id).unwrap().refactor_mode, None);

        outcome.mode = Some(" characters ".to_owned());
        apply(root.path(), &world_id, &outcome, &no_player_selection(Vec::new())).unwrap();
        assert_eq!(
            data::read_state(root.path(), &world_id).unwrap().refactor_mode.as_deref(),
            Some("characters")
        );
    }
    /// 讀取端正規化：舊版落地的 "Characters"／空白修成合法值，未知值回 Err（fail-closed）。
    #[test]
    fn normalize_stored_mode_fixes_legacy_case_and_rejects_unknown() {
        assert_eq!(normalize_stored_mode(None), Ok(None));
        assert_eq!(
            normalize_stored_mode(Some(" Characters ".to_owned())),
            Ok(Some("characters".to_owned()))
        );
        assert_eq!(
            normalize_stored_mode(Some("interface".to_owned())),
            Ok(Some("interface".to_owned()))
        );
        assert!(normalize_stored_mode(Some("both".to_owned())).is_err());
    }
    /// (d) 介面套用重建狀態樹：同名頂層鍵保留進度、舊 schema 殘渣清掉 → undo 後逐鍵回復。
    #[test]
    fn apply_interface_rebuilds_dirty_state_and_undo_restores_every_key() {
        let root = TestRoot::new("interface");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "介面腳本", "描述如何顯示狀態欄的散文");
        let mut before_state = data::read_state(root.path(), &world_id).unwrap();
        before_state.state.tree = BTreeMap::from([
            ("沦陷天数".to_owned(), StateNode::Leaf("7".to_owned())),
            ("淪陷天數".to_owned(), StateNode::Leaf("42".to_owned())),
            ("舊欄位".to_owned(), StateNode::Leaf("殘渣".to_owned())),
        ]);
        before_state.state.jumps = BTreeMap::from([
            ("沦陷天数".to_owned(), "+1".to_owned()),
            ("淪陷天數".to_owned(), "+1".to_owned()),
        ]);
        let before_tree = before_state.state.tree.clone();
        data::write_state(root.path(), &world_id, &before_state).unwrap();

        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: Some(RefactorInterface {
                state_fields: serde_json::json!({
                    "淪陷天數": "0",
                    "世界": { "時間": "清晨" }
                }),
                source_uids: vec![source_uid.to_string()],
                raw: "描述如何顯示狀態欄的散文".to_owned(),
                shell: None,
                rules: BTreeMap::new(),
                guide: String::new(),
            }),
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: true,
            mechanism_indices: Vec::new(),
            entry_indices: Vec::new(),
            player_index: None,
        };

        let result = apply_recorded(root.path(), &world_id, &outcome, &selection);
        assert!(result.summary.interface_applied);
        assert_eq!(result.summary.deleted_entries, 1);
        assert_eq!(result.summary.rewritten_entries, 0);

        let state = data::read_state(root.path(), &world_id).unwrap();
        assert_eq!(
            state.state.tree,
            BTreeMap::from([
                ("淪陷天數".to_owned(), StateNode::Leaf("42".to_owned())),
                ("世界".to_owned(), StateNode::Branch(BTreeMap::from([("時間".to_owned(), StateNode::Leaf("清晨".to_owned()))]))),
            ])
        );
        assert_eq!(state.state.jumps, BTreeMap::from([("淪陷天數".to_owned(), "+1".to_owned())]));
        assert!(!data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .iter()
            .any(|entry| entry.uid == source_uid));

        receipts::undo_last_import(root.path(), &world_id).unwrap();
        let state_after = data::read_state(root.path(), &world_id).unwrap();
        assert_eq!(state_after.state.tree, before_tree);
        let source_entry_after = data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .into_iter()
            .find(|entry| entry.uid == source_uid)
            .unwrap();
        assert_eq!(source_entry_after.content, "描述如何顯示狀態欄的散文");
    }
    /// characters 產物即使 selection 勾了介面（匯入路徑會全勾）也整段不套：狀態樹不動、
    /// 不開增量協定、不寫殼檔、介面來源條目不因「已消耗」被刪——mode 閘門必須在來源消耗
    /// 判定之前生效（GUI 回歸：C2 桌套出五棵介面狀態樹）。
    #[test]
    fn apply_characters_mode_skips_interface_and_keeps_sources() {
        let root = TestRoot::new("characters-skips-interface");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "介面腳本", "描述如何顯示狀態欄的散文");

        let outcome = RefactorOutcome {
            mode: Some("characters".to_owned()),
            characters: Vec::new(),
            interface: Some(RefactorInterface {
                state_fields: serde_json::json!({ "世界": { "時間": "清晨" } }),
                source_uids: vec![source_uid.to_string()],
                raw: "描述如何顯示狀態欄的散文".to_owned(),
                shell: Some("<div>{{世界.時間}}</div>".to_owned()),
                rules: BTreeMap::from([(
                    "世界.時間".to_owned(),
                    FieldRule {
                        kind: FieldKind::Text,
                        min: None,
                        max: None,
                        update: UpdateMode::Replace,
                        inject: InjectLevel::Turn,
                        branch: None,
                        formula: None,
                    },
                )]),
                guide: "每回合報時間".to_owned(),
            }),
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: true,
            mechanism_indices: Vec::new(),
            entry_indices: Vec::new(),
            player_index: None,
        };

        let result = apply(root.path(), &world_id, &outcome, &selection).unwrap();
        assert!(!result.summary.interface_applied);
        assert_eq!(result.summary.deleted_entries, 0);

        let state = data::read_state(root.path(), &world_id).unwrap();
        assert_eq!(state.refactor_mode.as_deref(), Some("characters"));
        assert!(state.state.tree.is_empty());
        assert!(!state.mechanism.incremental);
        assert!(state.mechanism.rules.is_empty());
        assert!(!data::interface_shell_path(root.path(), &world_id).unwrap().exists());
        assert!(data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .iter()
            .any(|entry| entry.uid == source_uid));
    }
    /// NorthHall 實測樣態縮小版：模型把狀態欄輸出成頂層＋「状态栏」鏡像分支兩套，殼只綁
    /// 頂層。正規化要把分支的非空初始值收進頂層、多出的葉改掛頂層路徑、rules 全部 remap
    /// 去重，分支整個消失——否則 GM 照分支路徑寫，殼綁的頂層值永遠空字串。
    #[test]
    fn normalize_collapses_mirror_branch_with_shell_deciding_canon() {
        let rule = |kind: FieldKind| FieldRule {
            kind,
            min: None,
            max: None,
            update: UpdateMode::Replace,
            inject: InjectLevel::Turn,
            branch: None,
            formula: None,
        };
        let state_fields = serde_json::json!({
            "地點": "",
            "日期時間": "",
            "霍玄": { "行動": "" },
            "行動選項": { "選項1": "" },
            "状态栏": {
                "地點": "📍 霍府",
                "日期時間": "⏰ 大梁年間",
                "霍玄": { "行動": "站著", "名字": "霍玄" },
                "行動選項": { "選項1": "選A" }
            }
        });
        let shell = "<div>{{地點}}{{日期時間}}{{霍玄.行動}}{{行動選項.選項1}}{{本回合.正文}}</div>";
        let rules = BTreeMap::from([
            ("地點".to_owned(), rule(FieldKind::Text)),
            ("状态栏.地點".to_owned(), rule(FieldKind::Text)),
            ("状态栏.霍玄.名字".to_owned(), rule(FieldKind::ReadOnly)),
        ]);

        let (fields, rules) = normalize_interface_paths(&state_fields, Some(shell), &rules).unwrap();

        assert_eq!(
            fields,
            serde_json::json!({
                "地點": "📍 霍府",
                "日期時間": "⏰ 大梁年間",
                "霍玄": { "行動": "站著", "名字": "霍玄" },
                "行動選項": { "選項1": "選A" }
            })
        );
        assert_eq!(
            rules.keys().cloned().collect::<Vec<_>>(),
            vec!["地點".to_owned(), "霍玄.名字".to_owned()]
        );
        assert_eq!(rules["霍玄.名字"].kind, FieldKind::ReadOnly);
    }
    /// 同一欄位兩套非空初始值不一致＝產物自相矛盾，Err 拒套不猜；殼同時綁兩套路徑同理。
    #[test]
    fn normalize_rejects_conflicting_values_and_double_referenced_shell() {
        let state_fields = serde_json::json!({
            "地點": "王府",
            "日期時間": "清晨",
            "状态栏": { "地點": "霍府", "日期時間": "清晨" }
        });
        let error = normalize_interface_paths(&state_fields, Some("<div>{{地點}}</div>"), &BTreeMap::new())
            .unwrap_err();
        assert!(error.contains("初始值不一致"), "{error}");

        let mirrored = serde_json::json!({
            "地點": "",
            "日期時間": "",
            "状态栏": { "地點": "霍府", "日期時間": "清晨" }
        });
        let error = normalize_interface_paths(
            &mirrored,
            Some("<div>{{地點}}{{状态栏.日期時間}}</div>"),
            &BTreeMap::new(),
        )
        .unwrap_err();
        assert!(error.contains("自相矛盾"), "{error}");
    }
    /// 無殼＝state_fields 是權威：完整鏡像（分支每葉根層都有對應）才折疊；分支多一個
    /// 根層沒有的葉就不是鏡像，整份不動——沒有殼表態時不做任何有損猜測。
    #[test]
    fn normalize_without_shell_folds_only_complete_mirror() {
        let mirror = serde_json::json!({
            "地點": "",
            "日期時間": "清晨",
            "状态栏": { "地點": "霍府", "日期時間": "" }
        });
        let (fields, _) = normalize_interface_paths(&mirror, None, &BTreeMap::new()).unwrap();
        assert_eq!(fields, serde_json::json!({ "地點": "霍府", "日期時間": "清晨" }));

        let not_mirror = serde_json::json!({
            "地點": "",
            "日期時間": "",
            "状态栏": { "地點": "霍府", "日期時間": "", "額外欄": "值" }
        });
        let (fields, _) = normalize_interface_paths(&not_mirror, None, &BTreeMap::new()).unwrap();
        assert_eq!(fields, not_mirror);
    }
    /// 衝突產物在 preflight 就拒套：apply 回 Err，世界書、狀態樹、殼檔零變化——
    /// preflight 晚於任何寫入的話會變成沒有收據的半套用。
    #[test]
    fn apply_rejects_conflicting_interface_before_any_write() {
        let root = TestRoot::new("normalize-preflight");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "介面腳本", "狀態欄散文");

        let outcome = RefactorOutcome {
            mode: Some("interface".to_owned()),
            characters: vec![character("亞瑟", &[source_uid])],
            interface: Some(RefactorInterface {
                state_fields: serde_json::json!({
                    "地點": "王府",
                    "日期時間": "清晨",
                    "状态栏": { "地點": "霍府", "日期時間": "清晨" }
                }),
                source_uids: vec![source_uid.to_string()],
                raw: "狀態欄散文".to_owned(),
                shell: Some("<div>{{地點}}</div>".to_owned()),
                rules: BTreeMap::new(),
                guide: String::new(),
            }),
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: vec![0],
            apply_interface: true,
            mechanism_indices: Vec::new(),
            entry_indices: Vec::new(),
            player_index: None,
        };

        assert!(apply(root.path(), &world_id, &outcome, &selection).is_err());
        assert!(data::list_characters(root.path(), &world_id).unwrap().is_empty());
        assert!(data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .iter()
            .any(|entry| entry.uid == source_uid));
        assert!(data::read_state(root.path(), &world_id).unwrap().state.tree.is_empty());
        assert!(!data::interface_shell_path(root.path(), &world_id).unwrap().exists());
    }
    /// 介面套用要把新樹補進這一幕的事件快照：否則玩家一按收回，檯面就退回套用前的空樹，
    /// 佔位符全部填空、面板欄位一片空白（2026-08-12 實測踩到）。
    #[test]
    fn apply_interface_syncs_new_tree_into_scene_snapshots() {
        let root = TestRoot::new("interface-snapshot-sync");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "介面腳本", "描述如何顯示狀態欄的散文");
        for text in ["開場白", "玩家選角"] {
            data::append_transcript(
                root.path(),
                &world_id,
                0,
                &data::TranscriptEvent {
                    ts: "2026-08-12T10:00:00.000Z".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: data::TranscriptKind::Narration,
                    text: text.to_owned(),
                    raw: None,
                    state: None,
                    gm_only: false,
                },
            )
            .unwrap();
        }

        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: Some(RefactorInterface {
                state_fields: serde_json::json!({ "世界": { "時間": "清晨" } }),
                source_uids: vec![source_uid.to_string()],
                raw: "描述如何顯示狀態欄的散文".to_owned(),
                shell: None,
                rules: BTreeMap::new(),
                guide: String::new(),
            }),
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: true,
            mechanism_indices: Vec::new(),
            entry_indices: Vec::new(),
            player_index: None,
        };
        apply_recorded(root.path(), &world_id, &outcome, &selection);

        let expected = data::read_state(root.path(), &world_id).unwrap().state.tree;
        assert!(expected.contains_key("世界"));
        let events = data::read_transcript(root.path(), &world_id, 0).unwrap();
        assert_eq!(events.len(), 2);
        for event in &events {
            assert_eq!(event.state.as_ref().unwrap().tree, expected);
        }

        // 收回上一句後檯面退回前一則的快照，樹要還在
        assert!(data::pop_transcript(root.path(), &world_id, 0).unwrap());
        assert_eq!(data::read_state(root.path(), &world_id).unwrap().state.tree, expected);
    }
    #[test]
    fn apply_interface_with_non_object_state_fields_leaves_tree_unchanged() {
        let root = TestRoot::new("interface-invalid-state-fields");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let mut state = data::read_state(root.path(), &world_id).unwrap();
        state.state.tree = BTreeMap::from([("既有欄位".to_owned(), StateNode::Leaf("進度".to_owned()))]);
        let before_tree = state.state.tree.clone();
        data::write_state(root.path(), &world_id, &state).unwrap();
        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: Some(RefactorInterface {
                state_fields: serde_json::json!(["壞產物"]),
                source_uids: Vec::new(),
                raw: String::new(),
                shell: None,
                rules: BTreeMap::new(),
                guide: String::new(),
            }),
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: true,
            mechanism_indices: Vec::new(),
            entry_indices: Vec::new(),
            player_index: None,
        };

        apply(root.path(), &world_id, &outcome, &selection).unwrap();

        assert_eq!(data::read_state(root.path(), &world_id).unwrap().state.tree, before_tree);
    }
    /// 契約相容：AI 展開產物落地成 JSON 沒有 shell 鍵（舊版產物）照樣要能反序列化，shell 落
    /// None，不能因為多了新欄位就 fail closed。
    #[test]
    fn refactor_interface_deserializes_legacy_json_without_shell_field() {
        let legacy = serde_json::json!({
            "state_fields": { "World": { "Time": "清晨" } },
            "source_uids": ["7"],
            "raw": "原文",
        });
        let interface: RefactorInterface = serde_json::from_value(legacy).unwrap();
        assert!(interface.shell.is_none());
        assert_eq!(interface.source_uids, vec!["7"]);
        assert_eq!(interface.state_fields["World"]["Time"].as_str(), Some("清晨"));
    }
    /// 介面套用帶殼：world 目錄落一份 interface-shell.html，data::read_interface_shell（讀
    /// command 背後的邏輯層）讀得回來、內容一致。
    #[test]
    fn apply_interface_with_shell_writes_file_readable_via_data_layer() {
        let root = TestRoot::new("interface-shell-write");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "介面腳本", "描述如何顯示狀態欄的散文");
        let shell_html = "<!DOCTYPE html><html><body>{{World.Time}}</body></html>";

        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: Some(RefactorInterface {
                state_fields: serde_json::json!({ "World": { "Time": "清晨" } }),
                source_uids: vec![source_uid.to_string()],
                raw: "描述如何顯示狀態欄的散文".to_owned(),
                shell: Some(shell_html.to_owned()),
                rules: BTreeMap::from([(
                    "World.Time".to_owned(),
                    FieldRule::for_kind(data::FieldKind::Text),
                )]),
                guide: "每回合都要重報 World.Time。".to_owned(),
            }),
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: true,
            mechanism_indices: Vec::new(),
            entry_indices: Vec::new(),
            player_index: None,
        };

        apply(root.path(), &world_id, &outcome, &selection).unwrap();

        let read_back = data::read_interface_shell(root.path(), &world_id).unwrap();
        assert_eq!(read_back.as_deref(), Some(shell_html));
        // 接管卡的每一格都靠 GM 回報才會動：增量協定要開，卡自訂的欄位規則與回報指引要落檔
        let mechanism = data::read_state(root.path(), &world_id).unwrap().mechanism;
        assert!(mechanism.incremental);
        assert_eq!(
            mechanism.rules.get("World.Time").map(|rule| rule.kind),
            Some(data::FieldKind::Text)
        );
        assert_eq!(mechanism.guide, "每回合都要重報 World.Time。");
    }
    /// 介面套用沒帶殼（shell=None）：不落任何殼檔。
    #[test]
    fn apply_interface_without_shell_creates_no_shell_file() {
        let root = TestRoot::new("interface-shell-absent");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "介面腳本", "描述如何顯示狀態欄的散文");

        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: Some(RefactorInterface {
                state_fields: serde_json::json!({ "World": { "Time": "清晨" } }),
                source_uids: vec![source_uid.to_string()],
                raw: "描述如何顯示狀態欄的散文".to_owned(),
                shell: None,
                rules: BTreeMap::new(),
                guide: String::new(),
            }),
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: true,
            mechanism_indices: Vec::new(),
            entry_indices: Vec::new(),
            player_index: None,
        };

        apply(root.path(), &world_id, &outcome, &selection).unwrap();

        assert!(data::read_interface_shell(root.path(), &world_id).unwrap().is_none());
    }
    /// 介面套用帶殼 → undo：殼檔是這次套用新建的，undo 要把它刪掉（比照 world_card_created
    /// 的參考模式：只刪這次新建的，套用前就有的不動——這裡每次都是新桌，天然滿足這個條件）。
    #[test]
    fn apply_interface_shell_then_undo_deletes_shell_file() {
        let root = TestRoot::new("interface-shell-undo");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "介面腳本", "描述如何顯示狀態欄的散文");

        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: Some(RefactorInterface {
                state_fields: serde_json::json!({ "World": { "Time": "清晨" } }),
                source_uids: vec![source_uid.to_string()],
                raw: "描述如何顯示狀態欄的散文".to_owned(),
                shell: Some("<!DOCTYPE html><html><body>{{World.Time}}</body></html>".to_owned()),
                rules: BTreeMap::new(),
                guide: String::new(),
            }),
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: true,
            mechanism_indices: Vec::new(),
            entry_indices: Vec::new(),
            player_index: None,
        };

        apply_recorded(root.path(), &world_id, &outcome, &selection);
        assert!(data::read_interface_shell(root.path(), &world_id).unwrap().is_some());

        receipts::undo_last_import(root.path(), &world_id).unwrap();
        assert!(data::read_interface_shell(root.path(), &world_id).unwrap().is_none());
    }
