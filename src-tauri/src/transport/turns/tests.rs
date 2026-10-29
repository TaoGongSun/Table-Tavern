#[allow(unused_imports)]
use super::super::arrivals::*;
#[allow(unused_imports)]
use super::super::assemble::*;
#[allow(unused_imports)]
use super::super::client::*;
#[allow(unused_imports)]
use super::super::context::*;
#[allow(unused_imports)]
use super::super::messages::*;
#[allow(unused_imports)]
use super::super::response::*;
#[allow(unused_imports)]
use super::super::state_view::*;
#[allow(unused_imports)]
use super::super::test_support::{card, event, worldbook_entry};
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

/// 驗收：換場摘要指示依語系切換，且 transcript 事件正確攤平成 user 訊息
#[test]
fn summary_messages_follow_lang_and_include_transcript() {
    let events = [
        event(TranscriptKind::Narration, "", "GM", "夜幕低垂"),
        event(TranscriptKind::Player, "", "玩家", "老闆，來杯麥酒"),
        event(TranscriptKind::Dialogue, "fox-id", "狐狸", "馬上來！"),
    ];
    let zh = summary_messages(&events, "zh-TW", false);
    assert_eq!(zh[0].role, "system");
    assert!(zh[0].content.contains("前情提要"));
    let joined: String = zh.iter().map(|m| m.content.as_str()).collect();
    assert!(joined.contains("（旁白）夜幕低垂"));
    assert!(joined.contains("玩家：老闆，來杯麥酒"));
    assert!(joined.contains("狐狸：馬上來！"));

    let en = summary_messages(&events, "en", false);
    assert!(en[0].content.contains("recap"));
}

/// 接管桌摘要只多一句禁標記（插在語系規則前），其餘逐字相同；transcript 部分不受影響
#[test]
fn summary_messages_takeover_adds_only_plain_text_rule() {
    let events = [event(TranscriptKind::Narration, "", "GM", "夜幕低垂")];
    for (lang, rule) in [
            ("zh-TW", "只寫純文字：不得輸出 XML／HTML 標籤、角括號標記或程式碼圍欄。"),
            (
                "en",
                "Write plain narrative text only: no XML or HTML tags, no angle-bracket markup, no code fences. ",
            ),
        ] {
            let plain = summary_messages(&events, lang, false);
            let takeover = summary_messages(&events, lang, true);
            assert!(!plain[0].content.contains(rule));
            let at = plain[0].content.rfind(language_rule(lang)).unwrap();
            let mut expected = plain[0].content.clone();
            expected.insert_str(at, rule);
            assert_eq!(takeover[0].content, expected);
            assert!(takeover[0].content.ends_with(language_rule(lang)));
            assert_eq!(plain[1..], takeover[1..]);
        }
}

/// chars 線凍結快照只能有全員共通且穩定的素材：公開卡＋玩家卡＋Public constant。
/// 私設、GM 專有、角色限定、keyword 條目一律不進快照（回合注入或不可見）。
#[test]
fn chars_lane_snapshot_holds_shared_public_material_only() {
    let fox = card("fox-id", "狐狸", "旅店老闆", "其實是通緝犯");
    let knight = card("knight-id", "騎士", "遊歷的騎士", "");
    let player = card("player-id", "阿濤", "商隊護衛", "");
    let entries = [
        worldbook_entry(1, "公開常識", &[], true, 0, false, Visibility::Public),
        worldbook_entry(2, "GM專有", &[], true, 0, false, Visibility::Gm),
        worldbook_entry(
            3,
            "狐狸限定",
            &[],
            true,
            0,
            false,
            Visibility::Characters(vec!["fox-id".to_owned()]),
        ),
        worldbook_entry(
            4,
            "關鍵字條目",
            &["寶箱"],
            false,
            0,
            false,
            Visibility::Public,
        ),
    ];
    let snapshot = chars_lane_system(
        &[fox.clone(), knight.clone()],
        Some(&player),
        &entries,
        "zh-TW",
    );
    assert!(snapshot.contains("扮演引擎"));
    assert!(snapshot.contains("旅店老闆"));
    assert!(snapshot.contains("遊歷的騎士"));
    assert!(snapshot.contains("阿濤"));
    assert!(snapshot.contains("公開常識內容"));
    assert!(!snapshot.contains("通緝犯"));
    assert!(!snapshot.contains("GM專有"));
    assert!(!snapshot.contains("狐狸限定"));
    assert!(!snapshot.contains("關鍵字條目"));
    // 快照不依賴 events，本質上逐輪穩定；再組一次逐字相同
    assert_eq!(
        snapshot,
        chars_lane_system(&[fox, knight], Some(&player), &entries, "zh-TW")
    );
}

/// chars 線回合尾段：公開 keyword 條目留在 tail、私設與限定條目集中在 confidential；
/// confidential 在 tail 中恰好出現一次（回合後靠這個子段從 session 檔抹掉）。
#[test]
fn chars_lane_turn_isolates_confidential_segment() {
    let fox = card("fox-id", "狐狸", "旅店老闆", "其實是通緝犯");
    let events = [event(
        TranscriptKind::Player,
        "",
        "阿濤",
        "打開寶箱，讀羊皮卷",
    )];
    let entries = [
        worldbook_entry(1, "公開常識", &[], true, 0, false, Visibility::Public),
        worldbook_entry(
            2,
            "寶箱情報",
            &["寶箱"],
            false,
            0,
            false,
            Visibility::Public,
        ),
        worldbook_entry(
            3,
            "羊皮卷密文",
            &["羊皮卷"],
            false,
            0,
            false,
            Visibility::Characters(vec!["fox-id".to_owned()]),
        ),
        worldbook_entry(
            4,
            "狐狸長設",
            &[],
            true,
            0,
            false,
            Visibility::Characters(vec!["fox-id".to_owned()]),
        ),
    ];
    let turn = chars_lane_turn(
        &fox,
        None,
        &events,
        &entries,
        &TableState::default(),
        &Mechanism::default(),
        None,
        "zh-TW",
        false,
    );
    let confidential = turn.confidential.expect("私設＋限定條目必須進機密段");
    assert!(confidential.contains("通緝犯"));
    assert!(confidential.contains("羊皮卷密文內容"));
    assert!(confidential.contains("狐狸長設內容")); // 限定 constant 也走回合注入
    assert!(!confidential.contains("寶箱情報"));
    assert_eq!(turn.tail.matches(confidential.as_str()).count(), 1);
    // 抹掉機密段後，公開條目與本輪指定仍在（session 歷史剩這些）
    let erased = turn.tail.replacen(confidential.as_str(), "", 1);
    assert!(erased.contains("寶箱情報內容"));
    assert!(erased.contains("現在你是「狐狸」"));
    assert!(!erased.contains("公開常識")); // constant 已在快照，不重複
                                           // 沒有私設也沒有限定條目時不產生機密段
    let knight = card("knight-id", "騎士", "遊歷的騎士", "");
    let plain = chars_lane_turn(
        &knight,
        None,
        &events,
        &entries[..2],
        &TableState::default(),
        &Mechanism::default(),
        None,
        "zh-TW",
        false,
    );
    assert!(plain.confidential.is_none());
    assert!(plain.tail.contains("現在你是「騎士」"));
}

/// gm 線凍結快照＝GM 單發 system 的同等素材（全 constant＋全卡含私設＋world.md）；
/// 回合尾段＝keyword 條目＋目前狀態＋導演指示。
#[test]
fn gm_lane_snapshot_and_turn_cover_gm_material() {
    let fox = card("fox-id", "狐狸", "旅店老闆", "其實是通緝犯");
    let events = [event(TranscriptKind::Player, "", "阿濤", "打開寶箱")];
    let entries = [
        worldbook_entry(1, "GM專有", &[], true, 0, false, Visibility::Gm),
        worldbook_entry(
            2,
            "寶箱情報",
            &["寶箱"],
            false,
            0,
            false,
            Visibility::Public,
        ),
    ];
    let snapshot = gm_lane_system(
        "世界總覽",
        &[fox],
        None,
        &entries,
        &Mechanism::default(),
        "zh-TW",
    );
    assert!(snapshot.contains("世界總覽"));
    assert!(snapshot.contains("GM專有內容"));
    assert!(snapshot.contains("通緝犯"));
    assert!(!snapshot.contains("寶箱情報"));

    let mut state = TableState::default();
    state.table.insert("place".to_owned(), "酒館".to_owned());
    let turn = gm_lane_turn(
        &events,
        &entries,
        None,
        &state,
        &Mechanism::default(),
        &StateScope::default(),
        "（導演指示）請插入旁白。",
        "zh-TW",
    );
    assert!(turn.confidential.is_none());
    assert!(turn.tail.contains("寶箱情報內容"));
    assert!(turn.tail.contains("地點：酒館"));
    assert!(turn.tail.ends_with("（導演指示）請插入旁白。"));
}

/// 尾段世界書（tail-worldbook-erase）：交出的那段與 tail 裡逐字相同、恰好一次；不在機密段裡（回合後留在歷史）。
/// `Hoist::All` 的線世界書在 system，尾段沒有世界書段，必為 None；沒有尾段條目時也是 None。
#[test]
fn tail_worldbook_is_the_exact_public_segment_left_in_history() {
    use crate::transport::test_support::legacy::scan_of;
    use crate::world_scan::Viewer;
    let fox = card("fox-id", "狐狸", "旅店老闆", "其實是通緝犯");
    let events = [event(TranscriptKind::Player, "", "阿濤", "打開寶箱")];
    let entries = [
        worldbook_entry(1, "公開常識", &[], true, 0, false, Visibility::Public),
        worldbook_entry(
            2,
            "寶箱情報",
            &["寶箱"],
            false,
            0,
            false,
            Visibility::Public,
        ),
    ];
    let cards = std::slice::from_ref(&fox);
    let scan = scan_of(
        &entries,
        Viewer::Character(&fox),
        cards,
        None,
        &events,
        "zh-TW",
    );
    let run = |hoist| {
        super::chars_lane_turn(
            &fox,
            cards,
            None,
            &scan,
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
            hoist,
        )
    };
    let shared = run(Hoist::None);
    let worldbook = shared.tail_worldbook.expect("公開觸發條目是尾段世界書");
    assert!(worldbook.contains("寶箱情報內容"));
    assert_eq!(shared.tail.matches(worldbook.as_str()).count(), 1);
    let confidential = shared.confidential.expect("私設在機密段");
    assert!(!confidential.contains("寶箱情報"));
    assert!(run(Hoist::All).tail_worldbook.is_none());
    let quiet = [event(TranscriptKind::Player, "", "阿濤", "坐下")];
    let scan = scan_of(
        &entries,
        Viewer::Character(&fox),
        cards,
        None,
        &quiet,
        "zh-TW",
    );
    let none = super::chars_lane_turn(
        &fox,
        cards,
        None,
        &scan,
        &TableState::default(),
        &Mechanism::default(),
        None,
        "zh-TW",
        Hoist::None,
    );
    assert!(none.tail_worldbook.is_none());

    let mut state = TableState::default();
    state.table.insert("place".to_owned(), "酒館".to_owned());
    let gm = |events: &[TranscriptEvent]| {
        gm_lane_turn(
            events,
            &entries,
            None,
            &state,
            &Mechanism::default(),
            &StateScope::default(),
            "請推進。",
            "zh-TW",
        )
    };
    let turn = gm(&events);
    let worldbook = turn.tail_worldbook.expect("GM 尾段世界書段");
    assert!(worldbook.starts_with("## 世界書"));
    assert!(worldbook.contains("寶箱情報內容"));
    assert!(!worldbook.contains("酒館")); // 目前狀態不算世界書
    assert!(turn.tail.starts_with(worldbook.as_str()));
    assert_eq!(turn.tail.matches(worldbook.as_str()).count(), 1);
    assert!(gm(&quiet).tail_worldbook.is_none());
}

#[test]
fn lane_event_line_labels_every_kind_by_name() {
    assert_eq!(
        lane_event_line(
            &event(TranscriptKind::Dialogue, "fox-id", "狐狸", "晚安"),
            "zh-TW",
            Side::Gm
        )
        .as_deref(),
        Some("狐狸：晚安")
    );
    assert_eq!(
        lane_event_line(
            &event(TranscriptKind::Player, "", "阿濤", "好啊"),
            "zh-TW",
            Side::Gm
        )
        .as_deref(),
        Some("阿濤：好啊")
    );
    assert_eq!(
        lane_event_line(
            &event(TranscriptKind::Narration, "", "GM", "夜深了"),
            "zh-TW",
            Side::Gm
        )
        .as_deref(),
        Some("GM：夜深了")
    );
    let night = event(TranscriptKind::Narration, "", "GM", "夜深了");
    assert_eq!(
        lane_event_line(&night, "zh-TW", Side::GmOpeningRaw).as_deref(),
        Some("GM：夜深了")
    );
    assert_eq!(
        lane_event_line(&night, "en", Side::Gm).as_deref(),
        Some("GM: 夜深了")
    );
    // 修正前落檔、開頭帶照抄標籤的旁白：GM 側送出前剝掉
    assert_eq!(
        lane_event_line(
            &event(
                TranscriptKind::Narration,
                "",
                "GM",
                "（旁白）（旁白）夜深了"
            ),
            "zh-TW",
            Side::Gm
        )
        .as_deref(),
        Some("GM：夜深了")
    );
    // 角色側照舊「（旁白）」
    assert_eq!(
        lane_event_line(&night, "zh-TW", Side::Character).as_deref(),
        Some("（旁白）夜深了")
    );
    assert_eq!(
        lane_event_line(&night, "en", Side::Character).as_deref(),
        Some("(Narration) 夜深了")
    );
    assert_eq!(
        lane_event_line(
            &event(TranscriptKind::System, "", "", "擲骰 3"),
            "zh-TW",
            Side::Gm
        )
        .as_deref(),
        Some("（系統）擲骰 3")
    );
}

fn marked(marker: data::EventMarker, text: &str, gm_only: bool) -> TranscriptEvent {
    TranscriptEvent {
        gm_only,
        marker: Some(marker),
        ..event(TranscriptKind::System, "", "GM", text)
    }
}

/// visibility 洩漏修正（包 4b）：gm_only 事件在 chars 線只留標頭；GM 線與非 gm_only 事件
/// 一律全文；沒有玩家名的發言退回該語系稱呼。
#[test]
fn lane_event_line_redacts_gm_only_text_for_chars_lane_only() {
    let secret = marked(
        data::EventMarker::PersonArrival {
            title: "密探".to_owned(),
        },
        "只有 GM 知道的全文。",
        true,
    );
    assert_eq!(
        lane_event_line(&secret, "zh-TW", Side::Character).as_deref(),
        Some("（系統）（人物登場）〈密探〉")
    );
    assert_eq!(
        lane_event_line(&secret, "zh-TW", Side::Gm).as_deref(),
        Some("（系統）（人物登場）〈密探〉\n只有 GM 知道的全文。")
    );

    let public = event(TranscriptKind::System, "", "GM", "擲骰 3");
    assert_eq!(
        lane_event_line(&public, "zh-TW", Side::Character).as_deref(),
        Some("（系統）擲骰 3")
    );
    let nameless = event(TranscriptKind::Player, "", "", "你好");
    assert_eq!(
        lane_event_line(&nameless, "ko", Side::Character).as_deref(),
        Some("플레이어: 你好")
    );
}

/// 角色私設（系統事件）：世界書掃描一律先排除系統事件（方案二「掃描範圍」，同 ST coreChat），
/// 兩個視角都不靠它觸發。換幕摘要只讀角色側內容，回歸標頭只出現一次。
#[test]
fn character_side_keyword_and_summary_ignore_card_private() {
    let fox = card("fox-id", "狐狸", "尾巴很大。", "身上藏著龍鱗。");
    let (marker, text) =
        card_private(&fox, &crate::transport::test_support::plain_fill("阿濤")).unwrap();
    let private = marked(marker, &text, true);
    let knight = card("knight-id", "騎士", "王國騎士", "");
    let (marker, text) = card_arrival(&knight, &crate::transport::test_support::plain_fill("阿濤"));
    let arrival = marked(marker, &text, false);
    let entries = [worldbook_entry(
        1,
        "龍鱗傳說",
        &["龍鱗"],
        false,
        0,
        false,
        Visibility::Public,
    )];
    let events = [
        event(TranscriptKind::Player, "", "阿濤", "聽說有寶箱"),
        event(TranscriptKind::Narration, "", "GM", "夜深了"),
        arrival,
        private,
        event(TranscriptKind::Narration, "", "GM", "風起"),
    ];
    let turn = chars_lane_turn(
        &fox,
        None,
        &events,
        &entries,
        &TableState::default(),
        &Mechanism::default(),
        None,
        "zh-TW",
        false,
    );
    assert!(!turn.tail.contains("龍鱗傳說"));
    let gm_turn = gm_lane_turn(
        &events,
        &entries,
        None,
        &TableState::default(),
        &Mechanism::default(),
        &StateScope::default(),
        "導演指示",
        "zh-TW",
    );
    assert!(!gm_turn.tail.contains("龍鱗傳說"));

    let joined: String = summary_messages(&events, "zh-TW", false)
        .iter()
        .map(|message| message.content.as_str())
        .collect();
    assert_eq!(
        joined
            .matches("（系統）（角色回歸）〈騎士〉\n公開設定：\n王國騎士")
            .count(),
        1
    );
    assert!(!joined.contains("龍鱗"));
    assert!(!joined.contains("角色私設"));
}
