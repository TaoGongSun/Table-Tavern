use super::*;
use crate::data::Visibility;
use crate::refactor_ai::types::{RefactorSplitGroup, RefactorSurveyPerson};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TestRoot(std::path::PathBuf);

impl TestRoot {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "table-tavern-refactor-assemble-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn seed(root: &Path, world_id: &str, title: &str, content: &str) -> u64 {
    data::upsert_worldbook_entry(
        root,
        world_id,
        WorldbookEntry {
            uid: u64::MAX,
            title: title.to_owned(),
            keys: Vec::new(),
            content: content.to_owned(),
            constant: false,
            order: 0,
            disabled: false,
            visibility: Visibility::Gm,
            is_person: false,
            locked: false,
        },
    )
    .unwrap()
}

fn empty_survey() -> RefactorSurveyOutcome {
    RefactorSurveyOutcome {
        persons: Vec::new(),
        interface_uids: Vec::new(),
        playable_interface_uids: Vec::new(),
        verdicts: Vec::new(),
        splits: Vec::new(),
        groups: Vec::new(),
        fields: Vec::new(),
        mode: String::new(),
        raw: String::new(),
    }
}

fn verdict(uid: u64, action: &str) -> RefactorEntryVerdict {
    RefactorEntryVerdict {
        uid: uid.to_string(),
        action: action.to_owned(),
        rule: None,
        reason: String::new(),
    }
}

// ---- carry：byte 相等＋meta 原樣 ----

#[test]
fn assemble_local_carry_preserves_content_and_meta() {
    let root = TestRoot::new();
    let world_id = data::create_world(&root.0, "測試").unwrap();
    let uid = seed(&root.0, &world_id, "設定條目", "第一段。\n\n第二段。");
    let mut entry = data::read_worldbook(&root.0, &world_id).unwrap().remove(0);
    entry.constant = true;
    entry.order = 42;
    entry.disabled = true;
    entry.keys = vec!["關鍵字".to_owned()];
    data::upsert_worldbook_entry(&root.0, &world_id, entry).unwrap();

    let survey = RefactorSurveyOutcome {
        verdicts: vec![verdict(uid, "carry")],
        ..empty_survey()
    };

    let assembly = assemble_local(&root.0, &world_id, &survey).unwrap();
    assert_eq!(assembly.entries.len(), 1);
    let produced = &assembly.entries[0];
    assert_eq!(produced.content, "第一段。\n\n第二段。");
    assert_eq!(produced.source_uids, vec![uid.to_string()]);
    let meta = produced.meta.as_ref().unwrap();
    assert!(meta.constant);
    assert_eq!(meta.order, 42);
    assert!(meta.disabled);
    assert_eq!(meta.keys, vec!["關鍵字".to_owned()]);
    assert!(assembly.audit.is_empty());
}

// ---- drop：有效編號進淘汰清單，缺編號退回照搬 ----

#[test]
fn assemble_local_drop_with_valid_rule_goes_to_dropped_list() {
    let root = TestRoot::new();
    let world_id = data::create_world(&root.0, "測試").unwrap();
    let uid = seed(&root.0, &world_id, "版本紀錄", "v1.2 更新內容");

    let survey = RefactorSurveyOutcome {
        verdicts: vec![RefactorEntryVerdict {
            uid: uid.to_string(),
            action: "drop".to_owned(),
            rule: Some(2),
            reason: String::new(),
        }],
        ..empty_survey()
    };
    let assembly = assemble_local(&root.0, &world_id, &survey).unwrap();
    assert!(assembly.entries.is_empty());
    assert_eq!(assembly.dropped.len(), 1);
    assert_eq!(assembly.dropped[0].content, "v1.2 更新內容");
    assert_eq!(assembly.dropped[0].rule, 2);
    assert!(assembly.audit.is_empty());
}

#[test]
fn assemble_local_drop_without_valid_rule_falls_back_to_carry_with_audit() {
    let root = TestRoot::new();
    let world_id = data::create_world(&root.0, "測試").unwrap();
    let uid = seed(&root.0, &world_id, "版本紀錄", "v1.2 更新內容");

    let survey = RefactorSurveyOutcome {
        verdicts: vec![RefactorEntryVerdict {
            uid: uid.to_string(),
            action: "drop".to_owned(),
            rule: None, // 缺編號
            reason: String::new(),
        }],
        ..empty_survey()
    };
    let assembly = assemble_local(&root.0, &world_id, &survey).unwrap();
    assert!(assembly.dropped.is_empty());
    assert_eq!(assembly.entries.len(), 1);
    assert_eq!(assembly.entries[0].content, "v1.2 更新內容");
    assert_eq!(assembly.audit.len(), 1);
    assert_eq!(assembly.audit[0].kind, "drop_rule");
}

/// group 宣告不含被 route 的段＝該路由無效：不算已下落，落餘段兜底——否則該段不進
/// 產物（group 呼叫材料只拿宣告清單）也不進餘段，套用刪源後內容就消失了。
#[test]
fn assemble_local_group_route_without_span_in_declaration_falls_to_leftover() {
    let root = TestRoot::new();
    let world_id = data::create_world(&root.0, "測試").unwrap();
    let uid = seed(&root.0, &world_id, "身世條目", "秘密身世。\n\n城市設定。");

    let survey = RefactorSurveyOutcome {
        persons: Vec::new(),
        interface_uids: Vec::new(),
        playable_interface_uids: Vec::new(),
        verdicts: vec![verdict(uid, "split")],
        splits: vec![
            RefactorSpanRoute {
                span: format!("{uid}#s1"),
                route: "group".to_owned(),
                rule: None,
                name: String::new(),
                title: String::new(),
                group: "g1".to_owned(),
                note: String::new(),
            },
            RefactorSpanRoute {
                span: format!("{uid}#s2"),
                route: "group".to_owned(),
                rule: None,
                name: String::new(),
                title: String::new(),
                group: "g1".to_owned(),
                note: String::new(),
            },
        ],
        groups: vec![RefactorSplitGroup {
            id: "g1".to_owned(),
            title: "城市沿革".to_owned(),
            kind: "setting".to_owned(),
            spans: vec![format!("{uid}#s2")], // 宣告漏列 s1
        }],
        fields: Vec::new(),
        mode: String::new(),
        raw: String::new(),
    };

    let assembly = assemble_local(&root.0, &world_id, &survey).unwrap();
    let leftover = assembly
        .entries
        .iter()
        .find(|entry| entry.title == "身世條目（餘段）")
        .unwrap();
    assert_eq!(leftover.content, "秘密身世。");
    assert!(assembly
        .audit
        .iter()
        .any(|item| item.span == format!("{uid}#s1")));
}

// ---- split 全路由組裝：entry 跨條目串接／gm+unabsorbed 合條／person 段併卡／餘段兜底 ----

#[test]
fn assemble_local_split_routes_entry_gm_person_and_leftover_together() {
    let root = TestRoot::new();
    let world_id = data::create_world(&root.0, "測試").unwrap();
    let uid_a = seed(
        &root.0,
        &world_id,
        "條目A",
        "共同段落甲。\n\nGM 段落。\n\n未接管機制段。\n\n沒人要的段。",
    );
    let uid_b = seed(&root.0, &world_id, "條目B", "共同段落乙。");
    let uid_c = seed(
        &root.0,
        &world_id,
        "霍玄設定",
        "霍玄的基本介紹。\n\n霍玄的秘密心事。",
    );
    let uid_d = seed(&root.0, &world_id, "霍玄補充", "霍玄額外的公開段落。");

    let survey = RefactorSurveyOutcome {
        persons: vec![RefactorSurveyPerson {
            name: "霍玄".to_owned(),
            uids: vec![uid_c.to_string()],
            is_player: false,
            mode: "clean".to_owned(),
            spans: vec![format!("{uid_c}#s1"), format!("{uid_c}#s2")],
            private_spans: vec![format!("{uid_c}#s2")],
        }],
        verdicts: vec![
            verdict(uid_a, "split"),
            verdict(uid_b, "split"),
            verdict(uid_d, "split"),
        ],
        splits: vec![
            RefactorSpanRoute {
                span: format!("{uid_a}#s1"),
                route: "entry".to_owned(),
                rule: None,
                name: String::new(),
                title: "共同設定".to_owned(),
                group: String::new(),
                note: String::new(),
            },
            RefactorSpanRoute {
                span: format!("{uid_b}#s1"),
                route: "entry".to_owned(),
                rule: None,
                name: String::new(),
                title: "共同設定".to_owned(),
                group: String::new(),
                note: String::new(),
            },
            RefactorSpanRoute {
                span: format!("{uid_a}#s2"),
                route: "gm".to_owned(),
                rule: None,
                name: String::new(),
                title: String::new(),
                group: String::new(),
                note: String::new(),
            },
            RefactorSpanRoute {
                span: format!("{uid_a}#s3"),
                route: "unabsorbed".to_owned(),
                rule: None,
                name: String::new(),
                title: String::new(),
                group: String::new(),
                note: "擲骰檢定".to_owned(),
            },
            RefactorSpanRoute {
                span: format!("{uid_d}#s1"),
                route: "person".to_owned(),
                rule: None,
                name: "霍玄".to_owned(),
                title: String::new(),
                group: String::new(),
                note: String::new(),
            },
            // uid_a#s4 故意不路由：驗證餘段兜底。
        ],
        ..empty_survey()
    };

    let assembly = assemble_local(&root.0, &world_id, &survey).unwrap();

    let merged = assembly
        .entries
        .iter()
        .find(|e| e.title == "共同設定")
        .unwrap();
    assert_eq!(merged.content, "共同段落甲。\n\n共同段落乙。");
    assert_eq!(
        merged.source_uids,
        vec![uid_a.to_string(), uid_b.to_string()]
    );

    let gm = assembly
        .entries
        .iter()
        .find(|e| e.title == "條目A")
        .unwrap();
    assert_eq!(gm.content, "GM 段落。\n\n未接管機制段。");
    assert_eq!(assembly.unabsorbed.len(), 1);
    assert_eq!(assembly.unabsorbed[0].span, format!("{uid_a}#s3"));
    assert_eq!(assembly.unabsorbed[0].note, "擲骰檢定");

    let leftover = assembly
        .entries
        .iter()
        .find(|e| e.title == "條目A（餘段）")
        .unwrap();
    assert_eq!(leftover.content, "沒人要的段。");
    assert!(leftover.meta.is_none());
    assert_eq!(assembly.audit.len(), 1);
    assert_eq!(assembly.audit[0].kind, "split");
    assert_eq!(assembly.audit[0].span, format!("{uid_a}#s4"));

    let character = assembly
        .characters
        .iter()
        .find(|c| c.name == "霍玄")
        .unwrap();
    assert_eq!(
        character.public_md,
        "霍玄的基本介紹。\n\n霍玄額外的公開段落。"
    );
    assert_eq!(character.private_md, "霍玄的秘密心事。");
    assert!(assembly.clean_person_names.contains(&"霍玄".to_owned()));
}

// ---- clean 人物：壞引用退回展開佇列 ----

#[test]
fn assemble_local_clean_person_with_invalid_span_is_skipped_with_audit() {
    let root = TestRoot::new();
    let world_id = data::create_world(&root.0, "測試").unwrap();
    let uid = seed(&root.0, &world_id, "阿蘭設定", "阿蘭的介紹。");

    let survey = RefactorSurveyOutcome {
        persons: vec![RefactorSurveyPerson {
            name: "阿蘭".to_owned(),
            uids: vec![uid.to_string()],
            is_player: false,
            mode: "clean".to_owned(),
            spans: vec![format!("{uid}#s1"), format!("{uid}#s9")], // s9 越界，無效引用
            private_spans: Vec::new(),
        }],
        ..empty_survey()
    };

    let assembly = assemble_local(&root.0, &world_id, &survey).unwrap();
    assert!(assembly.characters.is_empty());
    assert!(assembly.clean_person_names.is_empty());
    assert_eq!(assembly.audit.len(), 1);
    assert_eq!(assembly.audit[0].kind, "split");
}

// ---- 涵蓋：漏網 uid 自動 carry ----

#[test]
fn assemble_local_uncovered_uid_falls_back_to_carry_with_coverage_audit() {
    let root = TestRoot::new();
    let world_id = data::create_world(&root.0, "測試").unwrap();
    let uid = seed(&root.0, &world_id, "漏網條目", "沒被判官提到的內容。");
    // survey 完全沒提到這個 uid（不在 persons/interface/verdicts 任何一處）。
    let survey = empty_survey();

    let assembly = assemble_local(&root.0, &world_id, &survey).unwrap();
    assert_eq!(assembly.entries.len(), 1);
    assert_eq!(assembly.entries[0].content, "沒被判官提到的內容。");
    assert!(assembly.entries[0].meta.is_some());
    assert_eq!(assembly.audit.len(), 1);
    assert_eq!(assembly.audit[0].kind, "coverage");
    assert_eq!(assembly.audit[0].uid, uid.to_string());
}

// clean 人物的 uids 欄多列了 spans 沒引用的 uid（判官敷衍亂塞）：名義下落不算 covered，
// 必須補 carry＋紅字（2026-08-12 镇北王府實測洞：兩條舊條無聲殘留）
#[test]
fn assemble_local_clean_person_extra_uid_without_span_reference_still_counts_uncovered() {
    let root = TestRoot::new();
    let world_id = data::create_world(&root.0, "測試").unwrap();
    let used_uid = seed(&root.0, &world_id, "人物條目", "霍玄的完整設定。");
    let stray_uid = seed(
        &root.0,
        &world_id,
        "美化状态栏",
        "| 体力 | 心情 |\n| 100 | 好 |",
    );
    let mut survey = empty_survey();
    survey.persons = vec![RefactorSurveyPerson {
        name: "霍玄".to_owned(),
        uids: vec![used_uid.to_string(), stray_uid.to_string()],
        is_player: false,
        mode: "clean".to_owned(),
        spans: vec![format!("{used_uid}#s1")],
        private_spans: Vec::new(),
    }];

    let assembly = assemble_local(&root.0, &world_id, &survey).unwrap();
    // stray uid 被漏網稽核接住：補 carry＋coverage 紅字；used uid 是人物來源不補
    assert!(assembly.entries.iter().any(|e| e.title == "美化状态栏"));
    assert!(assembly
        .audit
        .iter()
        .any(|a| a.kind == "coverage" && a.uid == stray_uid.to_string()));
    assert!(!assembly.entries.iter().any(|e| e.title == "人物條目"));
}

// ---- 機制守恆：carry 無 reason 觸發 audit，有 reason 放行 ----

#[test]
fn assemble_local_mechanism_signal_needs_reason_to_pass_carry() {
    let root = TestRoot::new();
    let world_id = data::create_world(&root.0, "測試").unwrap();
    let flagged_uid = seed(
        &root.0,
        &world_id,
        "無說明條目",
        "trigger: 好感度達到 50 時告白",
    );
    let excused_uid = seed(
        &root.0,
        &world_id,
        "有說明條目",
        "trigger: 這只是歷史紀錄的關鍵字",
    );

    let survey = RefactorSurveyOutcome {
        verdicts: vec![
            verdict(flagged_uid, "carry"),
            RefactorEntryVerdict {
                uid: excused_uid.to_string(),
                action: "carry".to_owned(),
                rule: None,
                reason: "歷史紀錄，非即時機制".to_owned(),
            },
        ],
        ..empty_survey()
    };

    let assembly = assemble_local(&root.0, &world_id, &survey).unwrap();
    assert_eq!(assembly.entries.len(), 2); // 兩條都照搬
    let mechanism_audits: Vec<_> = assembly
        .audit
        .iter()
        .filter(|item| item.kind == "mechanism")
        .collect();
    assert_eq!(mechanism_audits.len(), 1);
    assert_eq!(mechanism_audits[0].uid, flagged_uid.to_string());
    // 附了 reason 的放行照搬要落一筆 excused，理由原文可見（調整階段檢查資料）。
    let excused_audits: Vec<_> = assembly
        .audit
        .iter()
        .filter(|item| item.kind == "excused")
        .collect();
    assert_eq!(excused_audits.len(), 1);
    assert_eq!(excused_audits[0].uid, excused_uid.to_string());
    assert_eq!(excused_audits[0].detail, "照搬理由：歷史紀錄，非即時機制");
    assert!(assembly
        .unabsorbed
        .iter()
        .any(|item| item.uid == flagged_uid.to_string()));
    assert!(!assembly
        .unabsorbed
        .iter()
        .any(|item| item.uid == excused_uid.to_string()));
}

/// characters 模式（refactor-mode-split）：INTERFACE 條目整條、statusbar 段半條都進
/// dropped rule 5（依模式捨棄），零呼叫可放回；混寫條目其他段照樣拆出，不整條陪葬。
#[test]
fn characters_mode_drops_interface_entries_and_statusbar_spans_as_rule5() {
    let root = TestRoot::new();
    let world_id = data::create_world(&root.0, "測試桌").unwrap();
    let ui_uid = seed(&root.0, &world_id, "介面條目", "整頁介面定義");
    let mixed_uid = seed(
        &root.0,
        &world_id,
        "混寫條目",
        "狀態欄欄位定義\n\n世界設定段",
    );
    let survey = RefactorSurveyOutcome {
        interface_uids: vec![ui_uid.to_string()],
        verdicts: vec![verdict(mixed_uid, "split")],
        splits: vec![
            RefactorSpanRoute {
                span: format!("{mixed_uid}#s1"),
                route: "statusbar".to_owned(),
                rule: None,
                name: String::new(),
                title: String::new(),
                group: String::new(),
                note: String::new(),
            },
            RefactorSpanRoute {
                span: format!("{mixed_uid}#s2"),
                route: "entry".to_owned(),
                rule: None,
                name: String::new(),
                title: "世界設定".to_owned(),
                group: String::new(),
                note: String::new(),
            },
        ],
        mode: "characters".to_owned(),
        ..empty_survey()
    };
    let assembly = assemble_local(&root.0, &world_id, &survey).unwrap();
    assert!(assembly
        .dropped
        .iter()
        .any(|item| item.uid == ui_uid.to_string()
            && item.rule == 5
            && item.span.is_empty()
            && item.content == "整頁介面定義"));
    assert!(assembly.dropped.iter().any(|item| item.rule == 5
        && item.span == format!("{mixed_uid}#s1")
        && item.content == "狀態欄欄位定義"));
    assert!(assembly
        .entries
        .iter()
        .any(|entry| entry.title == "世界設定" && entry.content == "世界設定段"));
    assert!(assembly.audit.is_empty());
}

/// interface 模式：person route 停用（提示詞已禁）——判官仍吐出來時兜底成
/// 以人名為題的設定條目照搬，人物設定不丟、不拆卡。
#[test]
fn interface_mode_falls_person_route_back_to_entry_by_name() {
    let root = TestRoot::new();
    let world_id = data::create_world(&root.0, "測試桌").unwrap();
    let uid = seed(&root.0, &world_id, "混寫", "霍玄的人物設定\n\n其他設定");
    let survey = RefactorSurveyOutcome {
        verdicts: vec![verdict(uid, "split")],
        splits: vec![
            RefactorSpanRoute {
                span: format!("{uid}#s1"),
                route: "person".to_owned(),
                rule: None,
                name: "霍玄".to_owned(),
                title: String::new(),
                group: String::new(),
                note: String::new(),
            },
            RefactorSpanRoute {
                span: format!("{uid}#s2"),
                route: "entry".to_owned(),
                rule: None,
                name: String::new(),
                title: "其他".to_owned(),
                group: String::new(),
                note: String::new(),
            },
        ],
        mode: "interface".to_owned(),
        ..empty_survey()
    };
    let assembly = assemble_local(&root.0, &world_id, &survey).unwrap();
    assert!(assembly.characters.is_empty());
    assert!(assembly
        .entries
        .iter()
        .any(|entry| entry.title == "霍玄" && entry.content == "霍玄的人物設定"));
}
