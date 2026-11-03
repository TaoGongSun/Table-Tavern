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
    assert_eq!(known_period("dusk"), Some("dusk"));
    assert_eq!(known_period("../x"), None);
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
    let used = resolve_scene(events.iter_mut(), None);
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
    let used = resolve_scene(events.iter_mut(), Some(("碼頭", Some("深夜"))));
    assert_eq!(periods(&events)[0], ("碼頭".to_owned(), "night".to_owned()));
    assert_eq!(periods(&events)[1], ("酒館".to_owned(), "night".to_owned()));
    assert_eq!(used[0].slug, slug_of("碼頭"));

    let mut events = vec![vec![bg("酒館", None), text("a")]];
    resolve_scene(events.iter_mut(), Some(("碼頭", Some("清晨"))));
    assert_eq!(
        periods(&events),
        [("酒館".to_owned(), "morning".to_owned())]
    );

    let mut events = vec![vec![text("a")]];
    assert!(resolve_scene(events.iter_mut(), Some(("  ", None))).is_empty());
    assert_eq!(events[0].len(), 1);
}

/// 已存在不覆寫（除非重生成）；地點名稱只記第一次
#[test]
fn library_writes_once_unless_forced() {
    let root = std::env::temp_dir().join(format!("tt-vn-bg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let world = data::create_world(&root, "背景").unwrap();
    let library = Library::of(&root, &world).unwrap();
    let harbor = Backdrop::new("港口", Some("黃昏"), None);
    assert!(library.wants(&harbor, false));
    library.write(&harbor, b"one").unwrap();
    assert!(!library.wants(&harbor, false));
    assert!(library.wants(&harbor, true));
    library
        .write(&Backdrop::new(" 港口", Some("夜"), None), b"two")
        .unwrap();
    assert_eq!(
        library.reference(&harbor.slug, "night").as_deref(),
        Some(&b"one"[..])
    );
    let names = std::fs::read_to_string(library.names_path()).unwrap();
    assert!(names.contains("\"港口\"") && !names.contains("\" 港口\""));
    let _ = std::fs::remove_dir_all(&root);
}
