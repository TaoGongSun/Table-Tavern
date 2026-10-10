//! 世界書掃描接線的單元測試（方案四之 2）：放置與網頁版分組一致、共用快照、機密分流、可見度、
//! 帶名字掃描、摘要與系統事件排除、P9 設定、落地呼叫點。
use super::*;
use crate::data::{self, CharacterCard, EventMarker, Tier, Visibility, WorldbookEntry};
use crate::import::CardInterface;
use crate::transport::{self, Hoist};
use crate::world_info::entry::from_world_file;
use crate::world_info::scan::{check_world_info, GlobalScan, ScanInput};
use crate::world_info::settings::ST_WI_SETTINGS;
use serde_json::{json, Map, Value};

pub(super) const LANG: &str = "zh-TW";

/// 暫存資料根（測試結束刪掉）。
pub(super) struct TempRoot(std::path::PathBuf);

impl TempRoot {
    pub(super) fn new(label: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "table-tavern-world-scan-{label}-{}-{id}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub(super) fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(super) fn card(id: &str, name: &str, public_md: &str, private_md: &str) -> CharacterCard {
    CharacterCard {
        id: id.to_owned(),
        name: name.to_owned(),
        color: "#336699".to_owned(),
        avatar: "🦊".to_owned(),
        tier: Tier::Balanced,
        show_image: true,
        archived: false,
        gen_prompt: String::new(),
        public_md: public_md.to_owned(),
        private_md: private_md.to_owned(),
    }
}

pub(super) fn event(kind: TranscriptKind, name: &str, text: &str) -> TranscriptEvent {
    TranscriptEvent {
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
        raw: None,
        ts: "2026-10-11T12:00:00+08:00".to_owned(),
        speaker_id: String::new(),
        speaker_name: name.to_owned(),
        kind,
        text: text.to_owned(),
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
    }
}

/// 一條原始條目（ST 物件形）：標題＝內文前綴，`fields` 蓋過預設。
pub(super) fn raw(
    uid: u64,
    title: &str,
    visibility: Visibility,
    fields: Value,
) -> (WorldbookEntry, Map<String, Value>) {
    let mut value = json!({
        "uid": uid,
        "key": [],
        "comment": title,
        "content": format!("{title}內容"),
        "constant": false,
        "order": 100,
        "position": 0,
    });
    for (key, field) in fields.as_object().unwrap() {
        value[key] = field.clone();
    }
    let Value::Object(map) = value else {
        unreachable!()
    };
    let view = WorldbookEntry {
        uid,
        title: title.to_owned(),
        keys: Vec::new(),
        content: String::new(),
        constant: map
            .get("constant")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        order: 100,
        disabled: false,
        visibility,
        is_person: false,
        locked: false,
    };
    (view, map)
}

pub(super) fn run(
    book: &TableBook,
    viewer: Viewer<'_>,
    cards: &[CharacterCard],
    events: &[TranscriptEvent],
) -> WorldScan {
    transport::test_support::legacy::prepared_book(book, viewer, "", cards, None, events, LANG)
}

/// 帶預算的掃描＋代換（空變數、沒有玩家卡）。
fn run_with_budget(
    book: &TableBook,
    viewer: Viewer<'_>,
    sole_card: Option<&str>,
    events: &[TranscriptEvent],
    max: f64,
) -> WorldScan {
    let session = MacroSession::new(MacroInputs::empty(), &viewer, "", None, None, events, LANG);
    scan(ScanRequest {
        book,
        viewer,
        sole_card,
        player: None,
        events,
        lang: LANG,
        timed: Default::default(),
        budget: Some(Budget {
            unit: crate::scene_budget::Unit::Bytes,
            max,
        }),
        random: Randomness::Measure,
        session: &session,
    })
}

fn uids(placed: &[Placed]) -> Vec<u64> {
    placed.iter().map(|entry| entry.uid).collect()
}

/// 各位置的分組先後與網頁版 `WiResult` 的字串欄位同一套規則（直接拿掃描核心的結果對照）。
#[test]
fn arrangement_matches_the_scan_result_groups() {
    let entries: Vec<(WorldbookEntry, Map<String, Value>)> = vec![
        raw(
            1,
            "前甲",
            Visibility::Gm,
            json!({"constant": true, "order": 5}),
        ),
        raw(
            2,
            "前乙",
            Visibility::Gm,
            json!({"constant": true, "order": 5}),
        ),
        raw(
            3,
            "前丙",
            Visibility::Gm,
            json!({"constant": true, "order": 1}),
        ),
        raw(
            4,
            "後甲",
            Visibility::Gm,
            json!({"constant": true, "position": 1, "order": 3}),
        ),
        raw(
            5,
            "註上",
            Visibility::Gm,
            json!({"constant": true, "position": 2}),
        ),
        raw(
            6,
            "註下",
            Visibility::Gm,
            json!({"constant": true, "position": 3}),
        ),
        raw(
            7,
            "深一系",
            Visibility::Gm,
            json!({"constant": true, "position": 4, "depth": 1, "role": 0}),
        ),
        raw(
            8,
            "深一用",
            Visibility::Gm,
            json!({"constant": true, "position": 4, "depth": 1, "role": 1}),
        ),
        raw(
            9,
            "深一助",
            Visibility::Gm,
            json!({"constant": true, "position": 4, "depth": 1, "role": 2}),
        ),
        raw(
            10,
            "深四系",
            Visibility::Gm,
            json!({"constant": true, "position": 4, "depth": 4, "role": 0}),
        ),
        raw(
            11,
            "深四系二",
            Visibility::Gm,
            json!({"constant": true, "position": 4, "depth": 4, "role": 0, "order": 7}),
        ),
        raw(
            12,
            "例上",
            Visibility::Gm,
            json!({"constant": true, "position": 5}),
        ),
        raw(
            13,
            "例下",
            Visibility::Gm,
            json!({"constant": true, "position": 6}),
        ),
        raw(
            14,
            "出口",
            Visibility::Gm,
            json!({"constant": true, "position": 7, "outletName": "門"}),
        ),
        raw(
            15,
            "深小數",
            Visibility::Gm,
            json!({"constant": true, "position": 4, "depth": 1.5}),
        ),
    ];
    let mut wi: Vec<_> = entries
        .iter()
        .map(|(view, map)| {
            let mut entry = from_world_file(map);
            entry.id = view.uid.to_string();
            entry
        })
        .collect();
    crate::world_info::sort::sort_entries(&mut wi);
    struct Plain;
    impl crate::world_info::scan::ScanHooks for Plain {
        fn substitute(&mut self, text: &str) -> crate::world_info::scan::Substituted {
            crate::world_info::scan::Substituted {
                text: text.to_owned(),
                private: false,
            }
        }
        fn count_tokens(&mut self, _: &str) -> f64 {
            0.0
        }
        fn random(&mut self) -> f64 {
            0.0
        }
    }
    let result = check_world_info(
        &wi,
        ScanInput {
            chat: &[],
            max_context: f64::INFINITY,
            global_scan: &GlobalScan::default(),
            trigger: "normal",
            timed: Default::default(),
            settings: &ST_WI_SETTINGS,
            pinned: &Default::default(),
        },
        &mut Plain,
    );
    let book = TableBook::from_raw(entries);
    let scanned = run(&book, Viewer::Gm, &[], &[]);
    let refs: Vec<&Placed> = scanned.placed.iter().collect();
    let arranged = arrange(&refs);
    let texts = |items: &[&Placed], wanted: f64| {
        items
            .iter()
            .filter(|entry| entry.position == wanted)
            .map(|entry| entry.content.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(texts(&arranged.front, 0.0), result.before);
    assert_eq!(texts(&arranged.back, 1.0), result.after);
    let examples: Vec<&str> = arranged
        .front
        .iter()
        .chain(arranged.back.iter())
        .filter(|entry| entry.position == 5.0 || entry.position == 6.0)
        .map(|entry| entry.content.as_str())
        .collect();
    assert_eq!(examples, ["例上內容", "例下內容"]);
    // 注入：深的在前；同深度 assistant → user → system；同深度同角色鍵名排序（作者註記在依深度條目前）；
    // 深度不是整數的不送；outlet 不在這裡
    let injected: Vec<&str> = arranged
        .injected
        .iter()
        .map(|item| item.text.as_str())
        .collect();
    let depth_four = result
        .depth
        .iter()
        .find(|group| group.depth == 4.0)
        .unwrap()
        .entries
        .join("\n");
    assert_eq!(depth_four, "深四系二內容\n深四系內容");
    assert_eq!(
        injected,
        [
            format!("註上內容\n\n註下內容\n{depth_four}").as_str(),
            "深一助內容",
            "深一用內容",
            "深一系內容",
        ]
    );
    assert_eq!(result.an_top, ["註上內容"]);
    assert!(scanned.placed.iter().any(|entry| entry.uid == 14));
}

fn shared_book() -> TableBook {
    TableBook::from_raw(vec![
        raw(1, "王國", Visibility::Public, json!({"constant": true})),
        raw(
            2,
            "王國後記",
            Visibility::Public,
            json!({"constant": true, "position": 1}),
        ),
        raw(
            3,
            "擲骰常駐",
            Visibility::Public,
            json!({"constant": true, "useProbability": true, "probability": 50}),
        ),
        raw(
            4,
            "黏著常駐",
            Visibility::Public,
            json!({"constant": true, "sticky": 2}),
        ),
        raw(
            5,
            "限生成",
            Visibility::Public,
            json!({"constant": true, "triggers": ["continue"]}),
        ),
        raw(
            6,
            "深度常駐",
            Visibility::Public,
            json!({"constant": true, "position": 4, "depth": 0}),
        ),
        raw(
            7,
            "群組常駐",
            Visibility::Public,
            json!({"constant": true, "group": "甲"}),
        ),
        raw(8, "GM 常駐", Visibility::Gm, json!({"constant": true})),
        raw(
            9,
            "騎士限定",
            Visibility::Characters(vec!["knight".to_owned()]),
            json!({"constant": true}),
        ),
        raw(
            10,
            "狐狸限定",
            Visibility::Characters(vec!["fox".to_owned()]),
            json!({"key": ["酒館"]}),
        ),
        raw(11, "酒館", Visibility::Public, json!({"key": ["酒館"]})),
    ])
}

/// 共用快照＝靜態集合（啟用、constant、`Public`、穩定、前／後／範例上下），不看角色掃描；切換角色逐字不變。
#[test]
fn shared_snapshot_is_static_and_identical_for_every_character() {
    let book = shared_book();
    assert_eq!(uids(&book.snapshot()), [1, 2]);
    let knight = card("knight", "騎士", "高大的騎士。", "其實是王子。");
    let fox = card("fox", "狐狸", "狡猾的狐狸。", "");
    let cards = [knight.clone(), fox.clone()];
    let events = [event(TranscriptKind::Player, "阿濤", "走進酒館")];
    let system = transport::chars_lane_system(
        &cards,
        None,
        &run(&book, Viewer::Character(&knight), &cards, &events),
        LANG,
    );
    assert!(system.contains("## 你知道的世界情報\n### 王國\n王國內容\n"));
    assert!(system.contains("## 你知道的世界情報（續）\n### 王國後記\n王國後記內容\n"));
    for absent in [
        "擲骰常駐",
        "黏著常駐",
        "限生成",
        "深度常駐",
        "群組常駐",
        "GM 常駐",
        "騎士限定",
    ] {
        assert!(!system.contains(absent), "{absent}");
    }
    for speaker in [&knight, &fox, &knight] {
        let scan = run(&book, Viewer::Character(speaker), &cards, &events);
        assert_eq!(
            transport::chars_lane_system(&cards, None, &scan, LANG),
            system
        );
        let turn = transport::chars_lane_turn(
            speaker,
            &cards,
            None,
            &scan,
            &Default::default(),
            &Default::default(),
            None,
            LANG,
            Hoist::None,
        );
        // 靜態條目不在尾段重複；不穩定的 constant 在尾段公開段
        assert!(!turn.tail.contains("王國內容"));
        assert!(turn.tail.contains("黏著常駐內容"));
        assert!(!turn.tail.contains("GM 常駐內容"));
        let limited = speaker.id == "knight";
        assert_eq!(turn.tail.contains("騎士限定內容"), limited);
        assert_eq!(turn.tail.contains("狐狸限定內容"), !limited);
        assert!(turn
            .confidential
            .as_deref()
            .unwrap_or("")
            .contains("限定內容"));
    }
}

/// 共用快照的靜態條目在角色掃描裡不會被預算撤掉，但照網頁版設溢出、停掉後面的條目與遞迴。
#[test]
fn snapshot_entries_survive_budget_overflow_but_stop_the_rest() {
    let book = TableBook::from_raw(vec![
        raw(
            1,
            "大段常駐",
            Visibility::Public,
            json!({"constant": true, "order": 50, "content": "很長很長很長很長很長很長很長很長很長很長"}),
        ),
        raw(
            2,
            "小段常駐",
            Visibility::Public,
            json!({"constant": true, "order": 40}),
        ),
        raw(
            3,
            "關鍵",
            Visibility::Public,
            json!({"key": ["酒館"], "order": 10}),
        ),
    ]);
    let fox = card("fox", "狐狸", "", "");
    let events = [event(TranscriptKind::Player, "阿濤", "走進酒館")];
    let scanned = run_with_budget(&book, Viewer::Character(&fox), Some("狐狸"), &events, 40.0);
    let mut placed = uids(&scanned.placed);
    placed.sort();
    assert_eq!(placed, [1, 2], "靜態條目都留著，溢出後的關鍵字條目停掉");
    // GM 視角沒有釘住：一樣的預算，第一條就溢出撤掉，後面停掉
    assert!(run_budget(&book, 40.0).is_empty());
}

fn run_budget(book: &TableBook, max: f64) -> Vec<u64> {
    let scanned = run_with_budget(
        book,
        Viewer::Gm,
        None,
        &[event(TranscriptKind::Player, "阿濤", "走進酒館")],
        max,
    );
    uids(&scanned.placed)
}

/// 機密分流：只靠 `private_md` 或限定條目遞迴觸發的 `Public` 條目進機密段；只命中 `public_md` 的是公開；
/// 私密沿遞迴往下傳。可見度：看不到 GM 條目與別人的限定條目。
#[test]
fn confidential_triggers_go_to_the_confidential_block() {
    let book = TableBook::from_raw(vec![
        raw(
            1,
            "燈塔",
            Visibility::Public,
            json!({"key": ["燈塔"], "matchCharacterDescription": true}),
        ),
        raw(
            2,
            "地圖",
            Visibility::Public,
            json!({"key": ["藏寶圖"], "matchCharacterDescription": true}),
        ),
        raw(
            3,
            "限定線索",
            Visibility::Characters(vec!["fox".to_owned()]),
            json!({"constant": true, "content": "線索指向礦坑"}),
        ),
        raw(
            4,
            "礦坑",
            Visibility::Public,
            json!({"key": ["礦坑"], "content": "礦坑通往山谷"}),
        ),
        raw(5, "山谷", Visibility::Public, json!({"key": ["山谷"]})),
        raw(6, "GM 秘密", Visibility::Gm, json!({"key": ["燈塔"]})),
        raw(
            7,
            "騎士限定",
            Visibility::Characters(vec!["knight".to_owned()]),
            json!({"key": ["燈塔"]}),
        ),
    ]);
    let fox = card("fox", "狐狸", "守著燈塔。", "藏著藏寶圖。");
    let scanned = run(
        &book,
        Viewer::Character(&fox),
        std::slice::from_ref(&fox),
        &[event(TranscriptKind::Player, "阿濤", "晚安")],
    );
    let confidential = |uid: u64| {
        scanned
            .placed
            .iter()
            .find(|entry| entry.uid == uid)
            .unwrap_or_else(|| panic!("{uid} 沒觸發"))
            .confidential()
    };
    assert!(!confidential(1), "只命中 public_md");
    assert!(confidential(2), "只命中 private_md");
    assert!(confidential(3), "限定條目");
    assert!(confidential(4), "限定條目遞迴觸發");
    assert!(confidential(5), "私密沿遞迴往下傳");
    assert!(!scanned
        .placed
        .iter()
        .any(|entry| entry.uid == 6 || entry.uid == 7));
    let turn = transport::chars_lane_turn(
        &fox,
        std::slice::from_ref(&fox),
        None,
        &scanned,
        &Default::default(),
        &Default::default(),
        None,
        LANG,
        Hoist::None,
    );
    let confidential_text = turn.confidential.unwrap();
    assert!(confidential_text.contains("### 地圖\n地圖內容"));
    assert!(confidential_text.contains("### 山谷\n山谷內容"));
    assert!(turn
        .tail
        .contains("## 你知道的世界情報\n### 燈塔\n燈塔內容\n"));
    assert!(!turn.tail.contains("## 你知道的世界情報\n### 地圖"));
}

/// 不抹尾段的線（`Hoist::All`）：本輪觸發的世界書全進 system，尾段沒有；API 單卡（`StableConfidential`）
/// 只把穩定的機密條目提上去。
#[test]
fn hoist_modes_place_the_worldbook() {
    let book = TableBook::from_raw(vec![
        raw(1, "酒館", Visibility::Public, json!({"key": ["酒館"]})),
        raw(
            2,
            "限定常駐",
            Visibility::Characters(vec!["fox".to_owned()]),
            json!({"constant": true}),
        ),
        raw(
            3,
            "限定關鍵",
            Visibility::Characters(vec!["fox".to_owned()]),
            json!({"key": ["酒館"]}),
        ),
    ]);
    let fox = card("fox", "狐狸", "", "身上有傷。");
    let scanned = run(
        &book,
        Viewer::Character(&fox),
        std::slice::from_ref(&fox),
        &[event(TranscriptKind::Player, "阿濤", "走進酒館")],
    );
    let turn = |hoist| {
        transport::chars_lane_turn(
            &fox,
            std::slice::from_ref(&fox),
            None,
            &scanned,
            &Default::default(),
            &Default::default(),
            None,
            LANG,
            hoist,
        )
    };
    let all = turn(Hoist::All);
    let hoisted = all.hoisted_private.clone().unwrap();
    for text in ["身上有傷。", "酒館內容", "限定常駐內容", "限定關鍵內容"] {
        assert!(hoisted.contains(text), "{text}");
        assert!(!all.tail.contains(text), "{text}");
    }
    assert!(all
        .hoisted_worldbook
        .as_deref()
        .unwrap()
        .contains("酒館內容"));
    let api = turn(Hoist::StableConfidential);
    let hoisted = api.hoisted_private.unwrap();
    assert!(hoisted.contains("限定常駐內容") && !hoisted.contains("限定關鍵內容"));
    assert!(api.tail.contains("限定關鍵內容") && api.tail.contains("酒館內容"));
    assert!(api.hoisted_worldbook.is_none());
}

/// 帶名字掃描：玩家句用玩家名、台詞用說話者；開場白在單角色桌用卡名、多角色桌用 GM；世界書路的旁白用原卡名。
/// 換幕摘要與系統事件先排除，不佔掃描深度。
#[test]
fn scan_names_and_exclusions_follow_the_plan() {
    let book = TableBook::from_raw(vec![raw(
        1,
        "嚮導",
        Visibility::Public,
        json!({"key": ["嚮導:"]}),
    )]);
    let guide = card("guide", "嚮導", "", "");
    let fox = card("fox", "狐狸", "", "");
    let opening = TranscriptEvent {
        opening: true,
        ..event(TranscriptKind::Narration, "GM", "要翻山就早點睡。")
    };
    let hit = |cards: &[CharacterCard], events: &[TranscriptEvent]| {
        !run(&book, Viewer::Character(&cards[0]), cards, events)
            .placed
            .is_empty()
    };
    assert!(hit(
        std::slice::from_ref(&guide),
        std::slice::from_ref(&opening)
    ));
    assert!(!hit(
        &[guide.clone(), fox.clone()],
        std::slice::from_ref(&opening)
    ));
    // 摘要、系統事件不佔深度：開場白仍在最近 2 則裡
    let summary = TranscriptEvent {
        marker: Some(EventMarker::SceneSummary),
        ..event(TranscriptKind::Narration, "GM", "前情")
    };
    let system = event(TranscriptKind::System, "GM", "時間流逝");
    let events = [
        opening.clone(),
        event(TranscriptKind::Player, "阿濤", "好"),
        summary,
        system,
    ];
    assert!(hit(std::slice::from_ref(&guide), &events));
    // 世界書路：旁白用原卡名
    let interface = CardInterface {
        character_id: String::new(),
        character_name: "嚮導".to_owned(),
        scripts: Vec::new(),
        unsupported: None,
        opening: None,
        mvu: false,
    };
    let world = TableBook::from_parts(
        vec![raw(
            1,
            "嚮導",
            Visibility::Public,
            json!({"key": ["嚮導:"]}),
        )],
        &[interface],
    );
    let narration = event(TranscriptKind::Narration, "GM", "風雪變大");
    assert!(
        !run(&world, Viewer::Gm, &[], std::slice::from_ref(&narration))
            .placed
            .is_empty()
    );
}

/// P9：載 MVU 的卡用 MVU 推薦值（不帶名字、不全字比對）；角色看自己那張，GM 看桌上任一張。
#[test]
fn mvu_settings_follow_the_viewer() {
    let mvu = |id: &str| CardInterface {
        character_id: id.to_owned(),
        character_name: id.to_owned(),
        scripts: Vec::new(),
        unsupported: None,
        opening: None,
        mvu: true,
    };
    let entries = || {
        vec![raw(
            1,
            "名字鍵",
            Visibility::Public,
            json!({"key": ["阿濤:"]}),
        )]
    };
    let events = [event(TranscriptKind::Player, "阿濤", "嗨")];
    let book = TableBook::from_parts(entries(), &[mvu("fox")]);
    let fox = card("fox", "狐狸", "", "");
    let knight = card("knight", "騎士", "", "");
    let cards = [fox.clone(), knight.clone()];
    assert!(run(&book, Viewer::Character(&fox), &cards, &events)
        .placed
        .is_empty());
    assert!(!run(&book, Viewer::Character(&knight), &cards, &events)
        .placed
        .is_empty());
    assert!(run(&book, Viewer::Gm, &cards, &events).placed.is_empty());
    let plain = TableBook::from_parts(entries(), &[]);
    assert!(!run(&plain, Viewer::Gm, &cards, &events).placed.is_empty());
}

/// 實送的落地呼叫點：掃描前結算並讀表、交出去那刻寫成已送出、確定失敗撤回成落地前的表。
#[test]
fn landing_writes_on_send_and_undoes_on_failure() {
    use crate::data::world_info_store::{read_scene, Perspective, Report, Stage};
    let root = TempRoot::new("world-scan-landing");
    let world = data::create_world(root.path(), "落地").unwrap();
    let gm = Perspective::Gm;
    let empty = landing::before_scan(root.path(), &world, 0, "t9", &gm).unwrap();
    assert_eq!(empty, Default::default());
    let mut timed = crate::world_info::timed::WiTimed::default();
    timed.sticky.insert(
        "1".to_owned(),
        crate::world_info::timed::TimedEffect {
            start: 0.0,
            end: 3.0,
            protected: false,
            confidential: false,
        },
    );
    let scanned = WorldScan {
        timed,
        ..WorldScan::default()
    };
    landing::land(root.path(), &world, 0, "t1", &gm, &scanned).unwrap();
    let stored = read_scene(root.path(), &world, 0).unwrap();
    assert_eq!(stored.pending.as_ref().unwrap().stage, Stage::Sent);
    assert!(stored.perspectives["gm"].sticky.contains_key("1"));
    landing::fail(root.path(), &world, 0, "t1", Report::Notices);
    let stored = read_scene(root.path(), &world, 0).unwrap();
    assert!(stored.pending.is_none());
    assert!(stored.perspectives["gm"].sticky.is_empty());
    // 已送出、逐字稿沒有回合鍵：下一次掃描前的結算當失敗撤回
    landing::land(root.path(), &world, 0, "t2", &gm, &scanned).unwrap();
    assert_eq!(
        landing::before_scan(root.path(), &world, 0, "t9", &gm).unwrap(),
        Default::default()
    );
}

/// 結算失敗回 `WorldInfoSettleFailed`，重設把壞檔移去備份後就能繼續。
#[test]
fn broken_timing_file_reports_and_reset_recovers() {
    use crate::data::world_info_store::{reset_scene, Perspective};
    let root = TempRoot::new("world-scan-reset");
    let world = data::create_world(root.path(), "重設").unwrap();
    let dir = data::world_info_dir(root.path(), &world).unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("0.json"), "{壞掉").unwrap();
    let error = landing::before_scan(root.path(), &world, 0, "t", &Perspective::Gm).unwrap_err();
    assert!(error.contains("world_info_settle_failed"), "{error}");
    let moved = reset_scene(root.path(), &world, 0).unwrap();
    assert_eq!(moved.len(), 1);
    assert!(moved[0].starts_with("0.json.broken-"), "{moved:?}");
    assert!(dir.join(&moved[0]).exists(), "壞檔留備份");
    assert!(landing::before_scan(root.path(), &world, 0, "t", &Perspective::Gm).is_ok());
    // 同一個後綴（同一秒）連續重設：撞名就接 -2、-3，舊備份不被蓋掉
    use crate::data::world_info_store::reset_scene_as;
    let mut names = Vec::new();
    for (round, text) in ["{一", "{二", "{三"].iter().enumerate() {
        std::fs::write(dir.join("0.json"), text).unwrap();
        let moved = reset_scene_as(root.path(), &world, 0, "broken-7").unwrap();
        assert_eq!(moved.len(), 1, "第 {round} 次");
        names.push(moved[0].clone());
    }
    assert_eq!(
        names,
        ["0.json.broken-7", "0.json.broken-7-2", "0.json.broken-7-3"]
    );
    for (name, text) in names.iter().zip(["{一", "{二", "{三"]) {
        assert_eq!(std::fs::read_to_string(dir.join(name)).unwrap(), text);
    }
    // 別的幕留著沒結的落地（分岔來源幕結算失敗會卡住）：重設一起移開
    landing::land(
        root.path(),
        &world,
        3,
        "t3",
        &Perspective::Gm,
        &Default::default(),
    )
    .unwrap();
    let moved = reset_scene(root.path(), &world, 0).unwrap();
    assert!(
        moved.iter().any(|name| name.starts_with("3.json.broken-")),
        "{moved:?}"
    );
}

/// 格式條目判定只認全文真的送出的條目：outlet 沒被引用不算、名冊不算，作者註記與依深度段落算。
#[test]
fn prompt_entries_count_only_full_text_that_is_sent() {
    let mut person = raw(4, "店主", Visibility::Gm, json!({"constant": true}));
    person.0.is_person = true;
    let book = TableBook::from_raw(vec![
        raw(
            1,
            "出口",
            Visibility::Gm,
            json!({"constant": true, "position": 7, "outletName": "門"}),
        ),
        raw(
            2,
            "註記",
            Visibility::Gm,
            json!({"constant": true, "position": 2}),
        ),
        raw(
            3,
            "深度",
            Visibility::Gm,
            json!({"constant": true, "position": 4, "depth": 2}),
        ),
        person,
        raw(5, "前段", Visibility::Gm, json!({"constant": true})),
    ]);
    let scanned = run(&book, Viewer::Gm, &[], &[]);
    let mut titles: Vec<&str> = transport::gm_prompt_full_entries(&scanned, &[])
        .iter()
        .map(|entry| entry.title)
        .collect();
    titles.sort();
    assert_eq!(titles, ["前段", "深度", "註記"]);
}

/// 同桌別的對話輪還在途：結算前擋下（不誤撤對方已送出的落地）。
#[test]
fn another_turn_in_flight_blocks_the_settle() {
    use crate::data::world_info_store::Perspective;
    let root = TempRoot::new("world-scan-busy");
    let world = data::create_world(root.path(), "並行").unwrap();
    let (_gm, _) = crate::inflight::register_turn(&world, "gm-turn");
    let (_me, _) = crate::inflight::register_turn(&world, "char-turn");
    let error =
        landing::before_scan(root.path(), &world, 0, "char-turn", &Perspective::Gm).unwrap_err();
    assert!(error.contains("world_busy"), "{error}");
    drop(_gm);
    assert!(landing::before_scan(root.path(), &world, 0, "char-turn", &Perspective::Gm).is_ok());
    // 同一個 turn_id 重複登記（同一輪送兩次）也擋
    let (_twin, _) = crate::inflight::register_turn(&world, "char-turn");
    let error =
        landing::before_scan(root.path(), &world, 0, "char-turn", &Perspective::Gm).unwrap_err();
    assert!(error.contains("world_busy"), "{error}");
}
