use super::*;
use crate::chat_turn::VnCell;

fn bg(location: &str, time: Option<&str>) -> VnCell {
    VnCell::Bg {
        location: location.to_owned(),
        time: time.map(str::to_owned),
        slug: slug_of(location),
        period: time.and_then(period_of).map(str::to_owned),
    }
}

fn text(body: &str) -> VnCell {
    VnCell::Narration {
        text: body.to_owned(),
    }
}

fn periods(events: &[Vec<VnCell>]) -> Vec<(String, String)> {
    events
        .iter()
        .flatten()
        .filter_map(|cell| match cell {
            VnCell::Bg {
                location, period, ..
            } => Some((location.clone(), period.clone()?)),
            _ => None,
        })
        .collect()
}

/// 同義寫法（全形、空白、大小寫）同鍵；不同地點不同鍵；路徑只由 hex 與列舉組成
#[test]
fn slug_folds_spelling_and_keeps_model_text_out_of_paths() {
    assert_eq!(slug_of("港口"), slug_of("  港口 "));
    assert_eq!(slug_of("Harbor  Gate"), slug_of("ＨＡＲＢＯＲ　gate"));
    assert_ne!(slug_of("港口"), slug_of("教室"));
    let slug = slug_of("../../etc/passwd");
    assert!(valid_slug(&slug));
    assert!(!valid_slug("../x") && !valid_slug(&"g".repeat(16)));
    let library = Library {
        dir: PathBuf::from("lib"),
    };
    assert_eq!(
        library.path(&slug, "dusk"),
        PathBuf::from(format!("lib/{slug}-dusk.png"))
    );
    assert_eq!(library.path(&slug, "../x"), PathBuf::from("lib/!-!.png"));
}

#[test]
fn period_words_hours_and_unknowns() {
    for (text, period) in [
        ("黃昏", "dusk"),
        ("第三天 傍晚", "dusk"),
        ("深夜", "night"),
        ("晚上", "night"),
        ("午夜", "night"),
        ("清晨", "morning"),
        ("上午", "morning"),
        ("下午", "day"),
        ("白天", "day"),
        ("Evening", "dusk"),
        ("afternoon", "day"),
        ("2024-05-01 18:30", "dusk"),
        ("07：15", "morning"),
    ] {
        assert_eq!(period_of(text), Some(period), "{text}");
    }
    assert_eq!(period_of("不久之後"), None);
}

/// 沒寫或認不得的時段沿用前一格（跨地點也沿用），全幕都沒有＝day
#[test]
fn scene_periods_carry_over_and_default_to_day() {
    let mut events = vec![
        vec![bg("教室", None), text("a")],
        vec![
            bg("港口", Some("黃昏")),
            bg("碼頭", Some("不久之後")),
            bg("港口", None),
        ],
    ];
    let used = resolve_scene(events.iter_mut(), None, &Places::default());
    assert_eq!(
        periods(&events),
        [
            ("教室", "day"),
            ("港口", "dusk"),
            ("碼頭", "dusk"),
            ("港口", "dusk")
        ]
        .map(|(a, b)| (a.to_owned(), b.to_owned()))
    );
    // 去重：港口黃昏只排一次
    assert_eq!(used.len(), 3);
}

/// 幕首不是 [bg]：以幕首快照的 place／time 補一格；幕首就是 [bg]：不補，但沒寫時段時沿用快照的
#[test]
fn scene_start_comes_from_the_first_snapshot() {
    let mut events = vec![vec![text("雨落在碼頭上。")], vec![bg("酒館", None)]];
    let used = resolve_scene(
        events.iter_mut(),
        Some(("碼頭", Some("深夜"))),
        &Places::default(),
    );
    assert_eq!(periods(&events)[0], ("碼頭".to_owned(), "night".to_owned()));
    assert_eq!(periods(&events)[1], ("酒館".to_owned(), "night".to_owned()));
    assert_eq!(used[0].slug, slug_of("碼頭"));

    let mut events = vec![vec![bg("酒館", None), text("a")]];
    resolve_scene(
        events.iter_mut(),
        Some(("碼頭", Some("清晨"))),
        &Places::default(),
    );
    assert_eq!(
        periods(&events),
        [("酒館".to_owned(), "morning".to_owned())]
    );

    let mut events = vec![vec![text("a")]];
    assert!(resolve_scene(events.iter_mut(), Some(("  ", None)), &Places::default()).is_empty());
    assert_eq!(events[0].len(), 1);
}

/// 已存在不覆寫（除非重生成）；地點名稱只記第一次
#[test]
fn library_writes_once_unless_forced() {
    let root = std::env::temp_dir().join(format!("tt-vn-bg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let world = data::create_world(&root, "背景").unwrap();
    let library = Library::of(&root, &world).unwrap();
    let harbor = Backdrop::new("港口", Some("黃昏"), None, &Places::default());
    assert!(library.wants(&harbor, false));
    library.write(&harbor, b"one").unwrap();
    assert!(!library.wants(&harbor, false));
    assert!(library.wants(&harbor, true));
    library
        .write(
            &Backdrop::new(" 港口", Some("夜"), None, &Places::default()),
            b"two",
        )
        .unwrap();
    assert_eq!(
        library
            .reference(&harbor.slug, "night", &periods_of(None))
            .as_deref(),
        Some(&b"one"[..])
    );
    let names = std::fs::read_to_string(library.names_path()).unwrap();
    assert!(names.contains("\"港口\"") && !names.contains("\" 港口\""));
    let _ = std::fs::remove_dir_all(&root);
}

fn card_places() -> Places {
    let place = |key: &str, name: &str, times: &[&str]| CardLocation {
        key: key.to_owned(),
        name: name.to_owned(),
        times: times.iter().map(|time| (*time).to_owned()).collect(),
        prompt: format!("{key} prompt"),
        scale: String::new(),
    };
    Places(vec![
        place(
            "cave",
            "嶺鬼洞穴祭壇",
            &["moonlight", "day", "crisis", "calm"],
        ),
        place("workshop", "拉克的工房", &["day", "dusk"]),
        place("pier", "碼頭", &[]),
    ])
}

/// 卡地點：鍵或名稱（全形、大小寫、空白）都對得上，slug 與自由地名分開；顯示名換成卡上的名稱
#[test]
fn card_places_match_by_key_or_name() {
    let places = card_places();
    let by_key = places.locate("ＷＯＲＫＳＨＯＰ");
    let by_name = places.locate(" 拉克的工房 ");
    assert_eq!(by_key.slug, by_name.slug);
    assert_eq!(by_key.name, "拉克的工房");
    assert!(valid_slug(&by_key.slug));
    assert_ne!(by_key.slug, slug_of("拉克的工房"));
    assert_eq!(
        places.locate("工房").slug,
        slug_of("工房"),
        "名稱要完整才算卡地點"
    );
    assert_eq!(places.periods(&by_key.slug), ["day", "dusk"]);
    assert_eq!(places.periods(&slug_of("工房")).len(), 4);
}

/// 時段：先比鍵、再走同義詞，都要在目的地清單內；沿用只沿用目的地清單內的，否則取清單第一項
#[test]
fn card_periods_follow_the_destination_list() {
    let places = card_places();
    let mut events = vec![vec![
        bg("嶺鬼洞穴祭壇", Some("Moonlight")),
        bg("cave", Some("不久之後")),
        bg("cave", Some("crisis")),
        bg("拉克的工房", None),
        bg("拉克的工房", Some("黃昏")),
        bg("拉克的工房", Some("夜")),
        bg("碼頭", None),
        bg("cave", Some("白天")),
    ]];
    resolve_scene(events.iter_mut(), None, &places);
    assert_eq!(
        periods(&events),
        [
            ("嶺鬼洞穴祭壇", "moonlight"),
            ("嶺鬼洞穴祭壇", "moonlight"),
            ("嶺鬼洞穴祭壇", "crisis"),
            ("拉克的工房", "day"),
            ("拉克的工房", "dusk"),
            ("拉克的工房", "dusk"),
            ("碼頭", "dusk"),
            ("嶺鬼洞穴祭壇", "day"),
        ]
        .map(|(a, b)| (a.to_owned(), b.to_owned()))
    );
}

/// 光線參考圖搜該地點的有效時段清單（卡自訂時段也找得到）
#[test]
fn reference_searches_the_place_periods() {
    let root = std::env::temp_dir().join(format!("tt-vn-bg-card-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let world = data::create_world(&root, "卡地點").unwrap();
    let library = Library::of(&root, &world).unwrap();
    let places = card_places();
    let moonlight = Backdrop::new("cave", Some("moonlight"), None, &places);
    library.write(&moonlight, b"moon").unwrap();
    let periods = places.periods(&moonlight.slug);
    assert_eq!(
        library
            .reference(&moonlight.slug, "crisis", &periods)
            .as_deref(),
        Some(&b"moon"[..])
    );
    let names = std::fs::read_to_string(library.names_path()).unwrap();
    assert!(names.contains("嶺鬼洞穴祭壇"));
    let _ = std::fs::remove_dir_all(&root);
}

/// 背景提示詞：卡地點描述與尺度、卡自訂時段、卡畫風取代通用句、背景規則、避免項、封面參考句
#[test]
fn background_prompt_uses_card_art() {
    use crate::data::vn_art::CardArt;
    use crate::transport::vn_background_prompts::{background_prompt, BackgroundReference};
    let art = CardArt {
        style: "rough watercolor".to_owned(),
        background_rules: "few props".to_owned(),
        negative: "3D render".to_owned(),
        cover_as_style_ref: true,
    };
    let places = card_places();
    let cave = places.by_slug(&places.locate("cave").slug);
    let prompt = background_prompt(
        "嶺鬼洞穴祭壇",
        cave,
        "crisis",
        "",
        BackgroundReference::CoverStyle,
        Some(&art),
    );
    for part in [
        "cave prompt",
        "mood variant: crisis",
        "rough watercolor",
        "few props",
        "Avoid (provided by the card author",
        "cover art of this story",
    ] {
        assert!(prompt.contains(part), "{part}");
    }
    assert!(!prompt.contains("Clean anime"));
    let plain = background_prompt("港口", None, "dusk", "", BackgroundReference::None, None);
    assert!(
        plain.contains("Clean anime") && plain.contains("sunset") && !plain.contains("card author")
    );
}
