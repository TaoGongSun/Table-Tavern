#[allow(unused_imports)]
use super::super::arrivals::*;
#[allow(unused_imports)]
use super::super::assemble::*;
#[allow(unused_imports)]
use super::super::client::*;
#[allow(unused_imports)]
use super::super::messages::*;
#[allow(unused_imports)]
use super::super::response::*;
#[allow(unused_imports)]
use super::super::state_view::*;
#[allow(unused_imports)]
use super::super::test_support::{card, event, worldbook_entry};
#[allow(unused_imports)]
use super::super::turns::*;
use super::*;
#[allow(unused_imports)]
use crate::data::{
    self, AppConfig, CharacterCard, DataResult, FieldKind, FieldRule, InjectLevel, Mechanism,
    StateNode, TableState, Tier, TranscriptEvent, TranscriptKind, Visibility, WorldbookEntry,
};
#[allow(unused_imports)]
use crate::mechanism;
#[allow(unused_imports)]
use crate::transport::test_support::legacy::{
    assemble_gm_messages, assemble_shared_messages, chars_lane_system, chars_lane_turn,
    gm_lane_system, gm_lane_turn,
};
#[allow(unused_imports)]
use std::collections::{BTreeMap, BTreeSet};

/// 介面接管桌的 GM 開頭：唯一敘事者、名單角色也由 GM 演、不提點名；保留公開旁白與不說破
#[test]
fn takeover_gm_intro_drops_roll_call_and_keeps_secrecy() {
    let texts = crate::world_scan::PromptTexts::default();
    let mechanism = Mechanism::default();
    for (lang, sole, call, secret) in [
        ("zh-TW", "唯一的敘事者", "決定下一位發言者", "不要說破"),
        (
            "en",
            "only narrator",
            "decide who speaks next",
            "do not reveal",
        ),
    ] {
        let normal = super::gm_system_prompt(
            &texts,
            &[],
            None,
            &[],
            &[],
            &mechanism,
            lang,
            false,
            &Default::default(),
        );
        let takeover = super::gm_system_prompt(
            &texts,
            &[],
            None,
            &[],
            &[],
            &mechanism,
            lang,
            true,
            &Default::default(),
        );
        assert!(normal.contains(call) && !normal.contains(sole), "{lang}");
        assert!(
            takeover.contains(sole) && !takeover.contains(call),
            "{lang}"
        );
        assert!(
            takeover.contains(secret) && normal.contains(secret),
            "{lang}"
        );
    }
}

/// GM 視角：穩定的 constant 進 system（order 小的在前，同 order 照 ST 排序的反向）；觸發的 keyword
/// 條目進回合尾段；掃描深度照 ST 預設 2 則、帶名字、不分大小寫；停用的不觸發。
#[test]
fn gm_scan_places_stable_constants_in_system_and_keywords_in_tail() {
    let entries = [
        worldbook_entry(4, "常駐", &[], true, 20, false, Visibility::Gm),
        worldbook_entry(1, "同序", &[], true, 20, false, Visibility::Gm),
        worldbook_entry(3, "近期", &["DrAgOn"], false, 10, false, Visibility::Gm),
        worldbook_entry(2, "太舊", &["ancient"], false, 0, false, Visibility::Gm),
        worldbook_entry(5, "停用", &[], true, -10, true, Visibility::Gm),
    ];
    let events = [
        event(TranscriptKind::Narration, "", "GM", "ancient history"),
        event(TranscriptKind::Player, "", "玩家", "one"),
        event(TranscriptKind::Narration, "", "GM", "A DRAGON wakes"),
        event(TranscriptKind::Player, "", "玩家", "four"),
    ];
    let scan = crate::transport::test_support::legacy::scan_of(
        &entries,
        crate::world_scan::Viewer::Gm,
        &[],
        None,
        &events,
        "zh-TW",
    );
    let split = gm_worldbook(&scan);
    let uids = |group: &[&Placed]| group.iter().map(|entry| entry.uid).collect::<Vec<_>>();
    // 放置前的排序（order 大的在前、同 order 照載入順序）；送出時反過來
    assert_eq!(uids(&split.system_front), [1, 4]);
    assert_eq!(uids(&split.tail), [3]);
    let system = gm_lane_system("", &[], None, &entries, &Mechanism::default(), "zh-TW");
    assert!(system.contains("### 常駐\n常駐內容\n### 同序\n同序內容\n"));
    assert!(!system.contains("停用內容"));
}

// ---- AI 卡重構包 4a：世界書人物條目在場過濾 ----

/// 規格 (a)：沒有 is_person 條目時，三處 system 組裝都不能多出名冊行——
/// 既有桌（沒有人物條目）的輸出必須逐字不變。
#[test]
fn person_roster_absent_without_is_person_entries() {
    let entries = [
        worldbook_entry(1, "公開常識", &[], true, 0, false, Visibility::Public),
        worldbook_entry(2, "GM專有", &[], true, 1, false, Visibility::Gm),
    ];
    let gm_system = &assemble_gm_messages(
        "世界總覽",
        &[],
        None,
        &[],
        &entries,
        &TableState::default(),
        &Mechanism::default(),
        &StateScope::default(),
        "zh-TW",
    )[0]
    .content;
    assert!(!gm_system.contains("人物名冊（不代表目前在場）"));

    let lane_system = gm_lane_system(
        "世界總覽",
        &[],
        None,
        &entries,
        &Mechanism::default(),
        "zh-TW",
    );
    assert!(!lane_system.contains("人物名冊（不代表目前在場）"));

    let chars_system = chars_lane_system(&[], None, &entries, "zh-TW");
    assert!(!chars_system.contains("人物名冊（不代表目前在場）"));
}

/// 規格 (b)：is_person 條目不進 system 全文，改收進名冊行；disabled 的人物條目不列進名冊。
#[test]
fn person_entries_become_roster_line_not_full_text() {
    let normal = worldbook_entry(1, "小鎮傳說", &[], true, 0, false, Visibility::Public);
    let alice = WorldbookEntry {
        is_person: true,
        ..worldbook_entry(2, "愛麗絲", &[], true, 1, false, Visibility::Public)
    };
    let bob = WorldbookEntry {
        is_person: true,
        ..worldbook_entry(3, "鮑伯", &[], true, 2, false, Visibility::Public)
    };
    let disabled_person = WorldbookEntry {
        is_person: true,
        ..worldbook_entry(4, "已停用的人", &[], true, 3, true, Visibility::Public)
    };
    let entries = [normal, alice, bob, disabled_person];

    let gm_system = &assemble_gm_messages(
        "",
        &[],
        None,
        &[],
        &entries,
        &TableState::default(),
        &Mechanism::default(),
        &StateScope::default(),
        "zh-TW",
    )[0]
    .content;
    assert!(gm_system.contains("小鎮傳說內容"));
    assert!(!gm_system.contains("愛麗絲內容"));
    assert!(!gm_system.contains("鮑伯內容"));
    assert!(!gm_system.contains("已停用的人"));
    assert!(gm_system.contains("人物名冊（不代表目前在場）：愛麗絲、鮑伯"));

    let chars_system = chars_lane_system(&[], None, &entries, "zh-TW");
    assert!(chars_system.contains("小鎮傳說內容"));
    assert!(!chars_system.contains("愛麗絲內容"));
    assert!(chars_system.contains("人物名冊（不代表目前在場）：愛麗絲、鮑伯"));
    assert!(chars_lane_system(&[], None, &entries, "en")
        .contains("Character roster (does not imply current presence): 愛麗絲, 鮑伯"));
}

/// 停用的人物條目不觸發，名冊行也不列。
#[test]
fn disabled_person_entries_stay_off_the_roster() {
    let alice = WorldbookEntry {
        is_person: true,
        ..worldbook_entry(1, "愛麗絲", &[], true, 0, false, Visibility::Public)
    };
    let disabled = WorldbookEntry {
        is_person: true,
        ..worldbook_entry(2, "隱藏人物", &[], true, 1, true, Visibility::Public)
    };
    let system = gm_lane_system(
        "",
        &[],
        None,
        &[alice, disabled],
        &Mechanism::default(),
        "zh-TW",
    );
    assert!(system.contains("人物名冊（不代表目前在場）：愛麗絲\n"));
    assert!(!system.contains("隱藏人物"));
}

/// 邊界情況：constant 條目清一色是人物時，非人物清單濾完是空的，但名冊行仍要印出來
/// （標頭不能因為「沒有全文條目」就整段消失）。
#[test]
fn person_roster_line_appears_even_when_all_constant_entries_are_people() {
    let alice = WorldbookEntry {
        is_person: true,
        ..worldbook_entry(1, "愛麗絲", &[], true, 0, false, Visibility::Gm)
    };
    let entries = [alice];
    let gm_system = &assemble_gm_messages(
        "",
        &[],
        None,
        &[],
        &entries,
        &TableState::default(),
        &Mechanism::default(),
        &StateScope::default(),
        "zh-TW",
    )[0]
    .content;
    assert!(
        gm_system.contains("\n## 世界書（只進你的上下文）\n人物名冊（不代表目前在場）：愛麗絲\n")
    );
}

// ---- mvu-replace-numeric：協定的數字欄兩條依更新策略分版 ----

fn protocol_systems(numeric: NumericUpdate, lang: &str) -> (String, String) {
    let mechanism = Mechanism {
        incremental: true,
        numeric_update: numeric,
        ..Mechanism::default()
    };
    let assembled = assemble_gm_messages(
        "",
        &[],
        None,
        &[],
        &[],
        &TableState::default(),
        &mechanism,
        &StateScope::default(),
        lang,
    )[0]
    .content
    .clone();
    let lane = gm_lane_system("", &[], None, &[], &mechanism, lang);
    (assembled, lane)
}

/// 原規則（重構卡、非 MVU 桌）：協定原文不動，數字欄一律 delta、會被擋下。
#[test]
fn delta_only_protocol_keeps_the_original_wording() {
    let (assembled, lane) = protocol_systems(NumericUpdate::DeltaOnly, "zh-TW");
    assert_eq!(assembled, lane);
    assert!(assembled.contains(delta_only_protocol("zh-TW")));
    assert!(assembled.contains(
        "數字欄一律用 delta 給增減量（例 5、-10），不要給絕對值——給絕對值會被系統擋下。"
    ));
    assert!(assembled.contains("只有上限改變（升級）才用 replace 寫成 \"480/600\"。"));
    let (en, _) = protocol_systems(NumericUpdate::DeltaOnly, "en");
    assert!(en.contains(delta_only_protocol("en")));
    assert!(en.contains("absolute values will be rejected"));
}

/// 沒重構的 MVU 卡：數字欄與「現值/上限」欄兩條換成照上游收 replace，其餘原文照舊；兩條 system 一致。
#[test]
fn upstream_protocol_allows_replace_for_numbers_and_pairs() {
    for (lang, gone, added) in [
        (
            "zh-TW",
            [
                "會被系統擋下",
                "只有上限改變（升級）",
                "你只要說「這一幕變動了多少」",
                "上下限與拒收由系統把關",
            ],
            [
                "數字欄可用 delta 給增減量（例 5、-10），也可以用 replace 直接寫新值（例 85）。",
                "用 replace 就整個寫成「現值/上限」（例 \"450/600\"）。",
                "用 delta 給增減量，或用 replace 直接寫新值。",
                "系統不會幫你夾在上下限內",
            ],
        ),
        (
            "en",
            [
                "absolute values will be rejected",
                "when the max changes (a level-up)",
                "how much changed this scene",
                "Bounds and rejections are enforced",
            ],
            [
                "or replace with the new value (e.g. 85).",
                "replace rewrites the whole value as \"current/max\" (e.g. \"450/600\").",
                "delta for the change amount, or replace to write the new value",
                "does not clamp it to any bounds",
            ],
        ),
    ] {
        let (assembled, lane) = protocol_systems(NumericUpdate::Upstream, lang);
        assert_eq!(assembled, lane, "{lang}");
        for text in gone {
            assert!(!assembled.contains(text), "{lang}: {text}");
        }
        for text in added {
            assert!(assembled.contains(text), "{lang}: {text}");
        }
        let (delta_only, _) = protocol_systems(NumericUpdate::DeltaOnly, lang);
        assert_ne!(assembled, delta_only, "{lang}");
        // 其餘條目照舊（文字欄、骰值欄）
        let shared = if lang == "en" {
            "Dice fields are rolled locally"
        } else {
            "骰值欄由系統每回合擲"
        };
        assert!(assembled.contains(shared), "{lang}");
    }
}

// ---- gm-format-directive-missing-target：導演指示只點名全文真的進了本輪提示的格式條目 ----

fn format_scripts() -> Vec<crate::import::InterfaceScript> {
    vec![crate::import::InterfaceScript {
        name: "顯示".to_owned(),
        find_regex: r"/<StatusBlock>([\s\S]*?)<\/StatusBlock>/s".to_owned(),
        replace_string: String::new(),
        trim_strings: Vec::new(),
        min_depth: None,
        max_depth: None,
        opening_only: false,
    }]
}

fn format_entry(constant: bool, keys: &[&str], disabled: bool) -> WorldbookEntry {
    WorldbookEntry {
        content: "每回合結尾輸出 <StatusBlock>地點｜時間</StatusBlock>".to_owned(),
        ..worldbook_entry(7, "輸出格式", keys, constant, 0, disabled, Visibility::Gm)
    }
}

fn named_format(worldbook: &[WorldbookEntry], events: &[TranscriptEvent]) -> Option<String> {
    let scan = crate::transport::test_support::legacy::scan_of(
        worldbook,
        crate::world_scan::Viewer::Gm,
        &[],
        None,
        events,
        "zh-TW",
    );
    crate::import::card_format_entry(&format_scripts(), &gm_prompt_full_entries(&scan, events))
}

/// 兩條 GM 路徑各自的完整提示：API 單發 messages；CLI lane 凍結 system＋從頭組的完整歷史＋回合尾段。
fn both_gm_prompts(worldbook: &[WorldbookEntry], events: &[TranscriptEvent]) -> (String, String) {
    let api = assemble_gm_messages(
        "",
        &[],
        None,
        events,
        worldbook,
        &TableState::default(),
        &Mechanism::default(),
        &StateScope::default(),
        "zh-TW",
    )
    .into_iter()
    .map(|message| message.content)
    .collect::<Vec<_>>()
    .join("\n");
    let lane_system = gm_lane_system("", &[], None, worldbook, &Mechanism::default(), "zh-TW");
    let lane_turn = gm_lane_turn(
        events,
        worldbook,
        None,
        &TableState::default(),
        &Mechanism::default(),
        &StateScope::default(),
        "",
        "zh-TW",
    );
    let lane_prompt = crate::lanes::build_prompt(
        events,
        0,
        &lane_turn.tail,
        true,
        crate::lanes::Lane::Gm,
        "zh-TW",
    );
    (api, format!("{lane_system}\n{lane_prompt}"))
}

/// 有格式：constant 條目全文在 system，點名它，兩條路徑都真的帶著格式全文。
#[test]
fn format_entry_named_when_constant_entry_in_prompt() {
    let worldbook = [format_entry(true, &[], false)];
    assert_eq!(named_format(&worldbook, &[]), Some("輸出格式".to_owned()));
    let (api, lane) = both_gm_prompts(&worldbook, &[]);
    assert!(api.contains("<StatusBlock>地點｜時間</StatusBlock>"));
    assert!(lane.contains("<StatusBlock>地點｜時間</StatusBlock>"));
}

/// 無格式：世界書沒有提到卡片標籤的條目。
#[test]
fn format_entry_absent_without_matching_entry() {
    let worldbook = [worldbook_entry(
        1,
        "世界觀",
        &[],
        true,
        0,
        false,
        Visibility::Gm,
    )];
    assert_eq!(named_format(&worldbook, &[]), None);
}

/// 停用：條目還在世界書，但不進提示，不能點名。
#[test]
fn format_entry_absent_when_disabled() {
    let worldbook = [format_entry(true, &[], true)];
    assert_eq!(named_format(&worldbook, &[]), None);
    let (api, lane) = both_gm_prompts(&worldbook, &[]);
    assert!(!api.contains("<StatusBlock>"));
    assert!(!lane.contains("<StatusBlock>"));
}

/// 未停用但 keyword 沒命中就不點名；命中那輪才點名，且兩條路徑的回合尾都帶著全文。
#[test]
fn keyword_format_entry_named_only_when_hit() {
    let worldbook = [format_entry(false, &["地牢"], false)];
    let quiet = [event(TranscriptKind::Player, "", "玩家", "走進酒館")];
    assert_eq!(named_format(&worldbook, &quiet), None);
    let (api, lane) = both_gm_prompts(&worldbook, &quiet);
    assert!(!api.contains("<StatusBlock>"));
    assert!(!lane.contains("<StatusBlock>"));

    let hit = [event(TranscriptKind::Player, "", "玩家", "走進地牢")];
    assert_eq!(named_format(&worldbook, &hit), Some("輸出格式".to_owned()));
    let (api, lane) = both_gm_prompts(&worldbook, &hit);
    assert!(api.contains("<StatusBlock>地點｜時間</StatusBlock>"));
    assert!(lane.contains("<StatusBlock>地點｜時間</StatusBlock>"));
}

fn person_format_entry() -> WorldbookEntry {
    WorldbookEntry {
        is_person: true,
        ..format_entry(true, &[], false)
    }
}

fn arrival_event(entry: &WorldbookEntry) -> TranscriptEvent {
    let (marker, text) = person_arrival(entry, &crate::transport::test_support::plain_fill("玩家"));
    TranscriptEvent {
        marker: Some(marker),
        gm_only: true,
        ..event(TranscriptKind::System, "", "", &text)
    }
}

/// constant 人物條目只剩名冊、全文不在 system，不能點名；登場事件把全文帶進歷史後才算。
#[test]
fn constant_person_format_entry_named_only_after_arrival() {
    let worldbook = [person_format_entry()];
    assert_eq!(named_format(&worldbook, &[]), None);
    let (api, lane) = both_gm_prompts(&worldbook, &[]);
    assert!(!api.contains("<StatusBlock>"));
    assert!(!lane.contains("<StatusBlock>"));
    assert!(api.contains("人物名冊（不代表目前在場）：輸出格式"));
    assert!(lane.contains("人物名冊（不代表目前在場）：輸出格式"));

    let arrived = [arrival_event(&worldbook[0])];
    assert_eq!(
        named_format(&worldbook, &arrived),
        Some("輸出格式".to_owned())
    );
    let (api, lane) = both_gm_prompts(&worldbook, &arrived);
    assert!(api.contains("<StatusBlock>地點｜時間</StatusBlock>"));
    assert!(lane.contains("<StatusBlock>地點｜時間</StatusBlock>"));
}

/// 反例：人物登場時本文沒有格式，之後同幕編輯條目加進格式標籤——歷史裡仍是舊本文、
/// system 只有名冊，不能點名。
#[test]
fn person_entry_edited_after_arrival_is_not_named() {
    let before = WorldbookEntry {
        content: "旅店老闆娘，愛說閒話。".to_owned(),
        ..person_format_entry()
    };
    let arrived = [arrival_event(&before)];
    let edited = [person_format_entry()];
    assert_eq!(named_format(&edited, &arrived), None);
    let (api, lane) = both_gm_prompts(&edited, &arrived);
    assert!(!api.contains("<StatusBlock>"));
    assert!(!lane.contains("<StatusBlock>"));
}

/// 反例：同名不同內容——登場的是沒有格式的那條，另一條同名人物條目帶格式但只進名冊，不能點名。
#[test]
fn same_title_person_entry_with_other_content_is_not_named() {
    let plain = WorldbookEntry {
        uid: 8,
        content: "旅店老闆娘，愛說閒話。".to_owned(),
        ..person_format_entry()
    };
    let worldbook = [person_format_entry(), plain.clone()];
    let arrived = [arrival_event(&plain)];
    assert_eq!(named_format(&worldbook, &arrived), None);
    let (api, lane) = both_gm_prompts(&worldbook, &arrived);
    assert!(!api.contains("<StatusBlock>"));
    assert!(!lane.contains("<StatusBlock>"));
}

/// 指示與收尾句同一判定：找得到點名現行文案；找不到用中性版，不指向缺席的格式、
/// 不要求「只輸出正文」、不禁狀態欄或更新區塊。
#[test]
fn card_format_turn_pairs_instruction_with_closing() {
    for lang in ["zh-TW", "en"] {
        let (named, closing) = card_format_turn(lang, Some("輸出格式"));
        assert!(named.content.contains("輸出格式"));
        assert_eq!(closing, gm_closing(GmTurnFormat::CardFormat, false, lang));

        let (neutral, closing) = card_format_turn(lang, None);
        assert_eq!(
            closing,
            gm_closing(GmTurnFormat::CardFormatAbsent, false, lang)
        );
        for text in [neutral.content.as_str(), closing] {
            for banned in [
                "已經規定了回覆的輸出格式",
                "上述輸出格式",
                "只輸出",
                "狀態欄",
                "already defines the reply format",
                "output format above",
                "Output only",
            ] {
                assert!(!text.contains(banned), "{lang}: {text}");
            }
        }
    }
}
