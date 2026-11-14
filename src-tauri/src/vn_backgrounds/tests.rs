use super::*;
use crate::chat_turn::VnCell;
use crate::vn_sprites::queue::{BgMode, BgSlot};

fn bg(location: &str, time: Option<&str>) -> VnCell {
    VnCell::Bg {
        location: location.to_owned(),
        time: time.map(str::to_owned),
        slug: slug_of(location),
        period: time.and_then(period_of).map(str::to_owned),
        weather: None,
    }
}

fn sky(location: &str, weather: &str) -> VnCell {
    match bg(location, None) {
        VnCell::Bg {
            location,
            time,
            slug,
            period,
            ..
        } => VnCell::Bg {
            location,
            time,
            slug,
            period,
            weather: Some(weather.to_owned()),
        },
        cell => cell,
    }
}

fn weathers(events: &[Vec<VnCell>]) -> Vec<String> {
    events
        .iter()
        .flatten()
        .filter_map(|cell| match cell {
            VnCell::Bg { weather, .. } => weather.clone(),
            _ => None,
        })
        .collect()
}

/// 天氣同義詞：雪暫顯示陰天（snowstorm 不落到雨）；認不得回 None
#[test]
fn weather_words_map_to_fixed_keys() {
    for (text, weather) in [
        ("大雨", Some("rain")),
        ("Thunderstorm", Some("rain")),
        ("海霧", Some("fog")),
        ("多雲", Some("overcast")),
        ("暴風雪", Some("overcast")),
        ("snowstorm", Some("overcast")),
        ("晴", Some("clear")),
        ("心情很差", None),
    ] {
        assert_eq!(weather_of(text), weather, "{text}");
    }
}

/// 沒寫天氣：同地點（只換時段）沿用，換地點回晴；幕首補的格也是晴
#[test]
fn weather_carries_within_a_place_and_clears_on_a_new_place() {
    let mut events = vec![
        vec![sky("港口", "rain"), bg("港口", Some("夜晚"))],
        vec![bg("酒館", None), bg("港口", None)],
    ];
    resolve_scene(events.iter_mut(), None, &Places::default());
    assert_eq!(weathers(&events), ["rain", "rain", "clear", "clear"]);
    let mut events = vec![vec![text("x")]];
    resolve_scene(events.iter_mut(), Some(("港口", None)), &Places::default());
    assert_eq!(weathers(&events), ["clear"]);
    // 同地點雨停：明寫晴才回晴，之後沒寫照樣沿用晴
    let mut events = vec![vec![
        sky("港口", "rain"),
        bg("港口", None),
        sky("港口", "clear"),
        bg("港口", Some("夜晚")),
    ]];
    resolve_scene(events.iter_mut(), None, &Places::default());
    assert_eq!(weathers(&events), ["rain", "rain", "clear", "clear"]);
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
        root: PathBuf::from("root"),
        world: "w".to_owned(),
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

fn slot(backdrop: &Backdrop, mode: BgMode) -> BgSlot {
    BgSlot::new("w", backdrop.clone(), mode)
}

/// 基底：地點有任何圖就不再生（換時段不排）；重生成與專屬圖照寫。地點名稱只記第一次、基底時段記第一張基底
#[test]
fn library_keeps_one_base_per_place() {
    let root = std::env::temp_dir().join(format!("tt-vn-bg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let world = data::create_world(&root, "背景").unwrap();
    let library = Library::of(&root, &world).unwrap();
    let places = Places::default();
    let all = periods_of(None);
    let dusk = Backdrop::new("港口", Some("黃昏"), None, &places);
    let base = dusk.base(&places);
    assert_eq!(base.period, "day");
    assert!(library.wants(&slot(&base, BgMode::Base), &all));
    library.write(&base, Some("day"), b"day").unwrap();
    assert!(!library.wants(&slot(&dusk.base(&places), BgMode::Base), &all));
    assert!(library.wants(&slot(&base, BgMode::Regenerate), &all));
    // 沒有專屬圖的時段讀到基底（period＝day，前端疊遮罩）；有專屬圖就用專屬圖
    let shown = library.shown(&base.slug, "dusk", &all).unwrap();
    assert_eq!(
        (shown.period.as_str(), shown.png.as_slice()),
        ("day", &b"day"[..])
    );
    library
        .write(
            &Backdrop::new(" 港口", Some("夜"), None, &places),
            None,
            b"night",
        )
        .unwrap();
    assert_eq!(
        library.shown(&base.slug, "night", &all).unwrap().png,
        b"night"
    );
    assert_eq!(
        library.reference(&base.slug, "dusk", &all).as_deref(),
        Some(&b"day"[..])
    );
    let names = std::fs::read_to_string(library.names_path()).unwrap();
    assert!(names.contains("\"港口\"") && !names.contains("\" 港口\""));
    assert!(names.contains("\"base\": \"day\""));
    let _ = std::fs::remove_dir_all(&root);
}

/// 舊桌只有時段圖、backgrounds.json 只記地名：清單裡第一張有圖的當基底，舊格式照讀
#[test]
fn legacy_library_falls_back_to_existing_period() {
    let root = std::env::temp_dir().join(format!("tt-vn-bg-old-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let world = data::create_world(&root, "舊背景").unwrap();
    let library = Library::of(&root, &world).unwrap();
    let all = periods_of(None);
    let night = Backdrop::new("港口", Some("夜"), None, &Places::default());
    library.write(&night, None, b"night").unwrap();
    assert_eq!(
        library.base_period(&night.slug, &all).as_deref(),
        Some("night")
    );
    assert!(!library.wants(&slot(&night.base(&Places::default()), BgMode::Base), &all));
    assert_eq!(
        library.shown(&night.slug, "day", &all).unwrap().period,
        "night"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// 串流 `[bg]`：只算換行結尾的行，沒寫時段沿用正式格，同鍵去重
#[test]
fn streamed_backdrops_inherit_formal_period() {
    let mut formal = vec![crate::chat_turn::VnEventCells {
        event_id: None,
        kind: data::TranscriptKind::Narration,
        cells: vec![bg("港口", Some("黃昏")), text("a")],
        hints: Vec::new(),
        chapter: None,
        recap: false,
        choice: None,
        index: 0,
    }];
    let roster = crate::chat_turn::VnRoster::default();
    let text = "走了很久。\n[bg: 碼頭]\n說話。\n[bg: 碼頭]\n[bg: 教室 | 夜]";
    let used = streamed(&mut formal, text, &crate::chat_turn::VnSource::Gm, &roster);
    assert_eq!(
        used,
        vec![Backdrop::new(
            "碼頭",
            Some("黃昏"),
            None,
            &Places::default()
        )]
    );
    assert!(streamed(
        &mut formal,
        "還在寫 [bg",
        &crate::chat_turn::VnSource::Gm,
        &roster
    )
    .is_empty());
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
    library
        .write(&moonlight, Some("moonlight"), b"moon")
        .unwrap();
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
    use crate::transport::vn_sprite_prompts::ArtStyle;
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
        ArtStyle::of(Some(&art), None),
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
    let plain = background_prompt(
        "港口",
        None,
        "dusk",
        "",
        BackgroundReference::None,
        ArtStyle::default(),
    );
    assert!(
        plain.contains("Clean anime") && plain.contains("sunset") && !plain.contains("card author")
    );
    // 玩家畫風取代卡作者畫風與避免項，卡作者的背景規則照留
    let player = crate::data::vn::ArtFlow {
        style: "pixel art".to_owned(),
        ..Default::default()
    };
    let styled = background_prompt(
        "港口",
        None,
        "dusk",
        "",
        BackgroundReference::None,
        ArtStyle::of(Some(&art), Some(&player)),
    );
    assert!(styled.contains("pixel art") && styled.contains("few props"));
    assert!(!styled.contains("rough watercolor") && !styled.contains("3D render"));
}

/// 上傳的鍵：slug 要合格、時段要在該地點的清單內（卡自訂時段照卡，自由地點固定四時段）
#[test]
fn upload_keys_follow_the_place_periods() {
    use gallery::checked;
    let places = card_places();
    let cave = places.locate("cave").slug;
    assert!(checked(&places, &cave, "crisis").is_ok());
    assert!(checked(&places, &cave, "night").is_err());
    assert!(checked(&places, &slug_of("港口"), "night").is_ok());
    assert!(checked(&places, &slug_of("港口"), "crisis").is_err());
    assert!(checked(&places, "../../etc/passwd", "day").is_err());
    assert!(checked(&places, &"g".repeat(16), "day").is_err());
}

/// 壞圖在寫入前就被 normalize 的轉檔擋下（不是圖、PNG 檔頭後面壞掉）
#[test]
fn upload_rejects_broken_images() {
    use crate::generated_image::to_png;
    assert!(to_png(b"not an image".to_vec()).is_err());
    let mut broken = b"\x89PNG\r\n\x1a\n".to_vec();
    broken.extend_from_slice(&[0; 32]);
    assert!(to_png(broken).is_err());
}

/// 一覽清單與上傳：卡地點沒圖也列在前；上傳到沒圖的地點成基底、到有基底的地點成專屬圖；
/// 清單回的地點文字送回生成命令會對到同一個 slug；認不出名字的 slug 拒收；重播桌拒寫
#[test]
fn gallery_lists_places_and_uploads_replace_images() {
    use gallery::commit_upload;
    let root = std::env::temp_dir().join(format!("tt-vn-bg-gallery-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let world = data::create_world(&root, "一覽").unwrap();
    let library = Library::of(&root, &world).unwrap();
    let places = card_places();
    let harbor = Backdrop::new("港口", Some("白天"), None, &places);
    library.write(&harbor, Some("day"), b"day").unwrap();
    let cave = places.locate("cave").slug;

    commit_upload(&root, &world, &places, &[], &harbor.slug, "night", b"night").unwrap();
    commit_upload(&root, &world, &places, &[], &cave, "crisis", b"crisis").unwrap();
    // 沒圖的卡地點上傳非第一項時段：只當專屬圖，基底仍是清單第一項（moonlight），自動補照排
    let cave_periods = places.periods(&cave);
    assert_eq!(library.base_period(&cave, &cave_periods), None);
    let cave_base = Backdrop::new("cave", Some("crisis"), None, &places).base(&places);
    assert_eq!(cave_base.period, "moonlight");
    assert!(library.wants(&slot(&cave_base, BgMode::Base), &cave_periods));
    assert_eq!(
        library.read(&harbor.slug, "night").as_deref(),
        Some(&b"night"[..])
    );
    let listed = library.places(&places, &[]);
    assert_eq!(listed.len(), 4);
    assert_eq!(listed[0].slug, cave);
    assert_eq!(listed[0].base, None);
    assert_eq!(listed[0].dedicated, ["crisis"]);
    assert_eq!(listed[1].dedicated, Vec::<String>::new());
    let free = listed
        .iter()
        .find(|place| place.slug == harbor.slug)
        .unwrap();
    assert_eq!(
        (free.base.as_deref(), free.dedicated.clone()),
        (Some("day"), vec!["day".to_owned(), "night".to_owned()])
    );
    for place in &listed {
        let location = place.location.as_deref().unwrap();
        let period = place.periods.last().unwrap();
        assert_eq!(
            Backdrop::new(location, None, Some(period), &places).slug,
            place.slug
        );
    }

    let unnamed = "0123456789abcdef";
    assert!(commit_upload(&root, &world, &places, &[], unnamed, "day", b"x").is_err());
    let mut state = data::read_state(&root, &world).unwrap();
    state.replay = true;
    data::write_state(&root, &world, &state).unwrap();
    assert!(commit_upload(&root, &world, &places, &[], &harbor.slug, "dusk", b"x").is_err());
    assert!(!library.has(&harbor.slug, "dusk"));
    let _ = std::fs::remove_dir_all(&root);
}

/// 一覽的生成目標以 slug 為身分：自由地點與卡地點同名時各對各的；只在佇列裡的地點（第一次生成失敗）也找得到名字
#[test]
fn gallery_targets_follow_slug_not_name() {
    let root = std::env::temp_dir().join(format!("tt-vn-bg-target-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let world = data::create_world(&root, "同名").unwrap();
    let library = Library::of(&root, &world).unwrap();
    let places = card_places();
    // 卡地點叫「碼頭」，自由地點的舊圖也叫「碼頭」（卡匯入前就生過）
    let free = Backdrop {
        location: "碼頭".to_owned(),
        slug: slug_of("碼頭"),
        period: "day".to_owned(),
    };
    library.write(&free, Some("day"), b"free").unwrap();
    let card = places.locate("碼頭").slug;
    assert_ne!(card, free.slug);
    let target = library.target(&places, &[], &free.slug, "night").unwrap();
    assert_eq!(
        (target.slug.as_str(), target.location.as_str()),
        (free.slug.as_str(), "碼頭")
    );
    assert_eq!(
        library.target(&places, &[], &card, "night").unwrap().slug,
        card
    );
    // 只在佇列裡的自由地點：清單列得出、目標找得到名字
    let queued = Backdrop::new("燈塔頂", None, None, &places);
    let listed = library.places(&places, std::slice::from_ref(&queued));
    let place = listed
        .iter()
        .find(|place| place.slug == queued.slug)
        .unwrap();
    assert_eq!(
        (place.location.as_deref(), place.base.clone()),
        (Some("燈塔頂"), None)
    );
    assert!(library.target(&places, &[], &queued.slug, "day").is_err());
    assert_eq!(
        library
            .target(&places, &[queued.clone()], &queued.slug, "day")
            .unwrap()
            .location,
        "燈塔頂"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// 被模型拒絕的背景不自動重試；一般失敗照舊重試一次
#[test]
fn refused_background_is_not_retried() {
    assert!(!super::retries("AI_IMAGE_REFUSED"));
    assert!(!super::retries(
        "AI_IMAGE_REFUSED: I cannot fulfill this request."
    ));
    assert!(super::retries("AI_CALL_FAILED: timeout"));
}
