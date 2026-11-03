//! 結構驗證：版本、幕樹、參照、展開放大、資產（plans/vn-mode-pkg8.md §3）。
use super::*;

fn line(kind: Kind, text: &str) -> Event {
    Event {
        kind,
        text: text.to_owned(),
        stage: Stage {
            chapter: Some(1),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn scene(at: u64, label: Option<Label>, events: Vec<Event>) -> Scene {
    Scene {
        scene: at,
        title: None,
        label,
        events,
    }
}

fn fork(source: u64, keep: u32) -> Option<Label> {
    Some(Label {
        base: 0,
        version: 2,
        parent: Some(source),
        forked: true,
        source: Some(source),
        keep: Some(keep),
    })
}

fn child(parent: u64) -> Option<Label> {
    Some(Label {
        base: 1,
        version: 1,
        parent: Some(parent),
        ..Default::default()
    })
}

fn hero() -> String {
    crate::data::new_id()
}

fn run(
    script: &mut Script,
    cast: &mut Characters,
    assets: &[&str],
    limits: Limits,
) -> Result<Vec<Asset>, String> {
    let mut manifest = Manifest {
        assets: assets.iter().map(|path| path.to_string()).collect(),
        ..Default::default()
    };
    check(
        &mut manifest,
        script,
        cast,
        limits,
        |_| true,
        &mut Vec::new(),
    )
}

fn tree(scenes: Vec<Scene>) -> Script {
    Script {
        scenes,
        endings: vec![EndingName {
            id: "default".to_owned(),
            title: String::new(),
        }],
        ..Default::default()
    }
}

fn broken_of(result: Result<Vec<Asset>, String>) -> String {
    result.err().expect("應該拒收")
}

#[test]
fn header_rejects_wrong_format_and_newer_versions() {
    let manifest = Manifest {
        format: "other".to_owned(),
        version: 1,
        ..Default::default()
    };
    assert_eq!(
        header(&manifest).unwrap_err(),
        UiMsg::VnWorkNotPack.to_string()
    );
    let manifest = Manifest {
        format: FORMAT.to_owned(),
        version: 2,
        ..Default::default()
    };
    assert_eq!(
        header(&manifest).unwrap_err(),
        UiMsg::VnWorkVersion { version: 2 }.to_string()
    );
}

#[test]
fn scene_tree_references_must_line_up() {
    let two = || vec![line(Kind::Narration, "a"), line(Kind::Narration, "b")];
    let cases: Vec<(&str, Vec<Scene>)> = vec![
        (
            "out of order",
            vec![scene(0, None, two()), scene(2, child(0), two())],
        ),
        ("first has label", vec![scene(0, child(0), two())]),
        (
            "keep beyond source",
            vec![scene(0, None, two()), scene(1, fork(0, 3), two())],
        ),
        (
            "source not earlier",
            vec![scene(0, None, two()), scene(1, fork(1, 1), two())],
        ),
        (
            "second story child",
            vec![
                scene(0, None, two()),
                scene(1, child(0), two()),
                scene(2, child(0), two()),
            ],
        ),
        ("empty scene", vec![scene(0, None, Vec::new())]),
    ];
    for (what, scenes) in cases {
        let error = broken_of(run(
            &mut tree(scenes),
            &mut Characters::default(),
            &[],
            LIMITS,
        ));
        assert!(error.starts_with(crate::ui_msg::MARK), "{what}: {error}");
    }
}

#[test]
fn speakers_endings_and_chapters_must_be_known() {
    let mut speaking = line(Kind::Dialogue, "嗨");
    speaking.speaker = Some(hero());
    let mut ending = line(Kind::Narration, "完");
    ending.stage.ended = Some("secret".to_owned());
    let mut chapter = line(Kind::Narration, "章");
    chapter.stage.chapter = Some(31);
    for event in [speaking, ending, chapter] {
        let error = broken_of(run(
            &mut tree(vec![scene(0, None, vec![event])]),
            &mut Characters::default(),
            &[],
            LIMITS,
        ));
        assert!(error.contains("vn_work_broken"), "{error}");
    }
}

#[test]
fn expanded_forks_count_against_the_limits() {
    // 每幕實存只有 1 則，但每個分岔都複製來源的 3 則：展開後全包超限
    let limits = Limits {
        total_events: 10,
        ..LIMITS
    };
    let mut scenes = vec![scene(0, None, vec![line(Kind::Narration, "a"); 3])];
    for at in 1..4 {
        scenes.push(scene(at, fork(0, 3), vec![line(Kind::Narration, "b")]));
    }
    assert_eq!(
        broken_of(run(
            &mut tree(scenes),
            &mut Characters::default(),
            &[],
            limits
        )),
        UiMsg::VnWorkTooLarge.to_string()
    );
    // 同樣的包放寬上限就收：存的事件數本身沒超
    let mut scenes = vec![scene(0, None, vec![line(Kind::Narration, "a"); 3])];
    scenes.push(scene(1, fork(0, 3), vec![line(Kind::Narration, "b")]));
    assert!(run(&mut tree(scenes), &mut Characters::default(), &[], limits).is_ok());
    // 文字量也照展開後算
    let limits = Limits {
        total_text_bytes: 3_500,
        ..LIMITS
    };
    let long = "字".repeat(500);
    let scenes = vec![
        scene(0, None, vec![line(Kind::Narration, &long)]),
        scene(1, fork(0, 1), vec![line(Kind::Narration, "b")]),
    ];
    assert_eq!(
        broken_of(run(
            &mut tree(scenes),
            &mut Characters::default(),
            &[],
            limits
        )),
        UiMsg::VnWorkTooLarge.to_string()
    );
}

#[test]
fn long_text_is_clipped_with_a_notice_and_system_events_are_emptied() {
    let mut private = line(Kind::System, "私設");
    private.speaker_name = "GM".to_owned();
    let mut script = tree(vec![scene(
        0,
        None,
        vec![line(Kind::Narration, &"長".repeat(TEXT_CHARS + 5)), private],
    )]);
    let mut manifest = Manifest::default();
    let mut notices = Vec::new();
    check(
        &mut manifest,
        &mut script,
        &mut Characters::default(),
        LIMITS,
        |_| true,
        &mut notices,
    )
    .unwrap();
    assert_eq!(script.scenes[0].events[0].text.chars().count(), TEXT_CHARS);
    assert!(
        script.scenes[0].events[1].text.is_empty()
            && script.scenes[0].events[1].speaker_name.is_empty()
    );
    assert_eq!(
        notices,
        [Notice {
            kind: NoticeKind::Clipped,
            target: "scene 1".to_owned()
        }]
    );
    assert_eq!(manifest.title, "VN");
}

#[test]
fn assets_must_be_well_formed_owned_and_present() {
    let id = hero();
    let mut cast = Characters {
        characters: vec![Character {
            id: id.clone(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let base = format!("assets/sprites/{id}/body/base.png");
    let meta = format!("assets/sprites/{id}/sprite.json");
    let face = format!("assets/sprites/{id}/face-v1/happy.png");
    let one = || tree(vec![scene(0, None, vec![line(Kind::Narration, "a")])]);
    let found = run(
        &mut one(),
        &mut cast,
        &[
            &meta,
            &base,
            &face,
            "assets/backgrounds/0123456789abcdef-day.png",
            "cover.png",
        ],
        LIMITS,
    )
    .unwrap();
    assert_eq!(found.len(), 5);
    let stranger = format!("assets/characters/{}.png", hero());
    for bad in [
        "assets/sprites/../x.png",
        "assets/backgrounds/zz-day.png",
        "assets/other.png",
        stranger.as_str(),
    ] {
        assert!(run(&mut one(), &mut cast, &[bad], LIMITS).is_err(), "{bad}");
    }
    // 有 sprite.json 沒有定稿；列了但包裡沒有
    assert!(run(&mut one(), &mut cast, &[&meta], LIMITS).is_err());
    let mut manifest = Manifest {
        assets: vec![base.clone()],
        ..Default::default()
    };
    assert!(check(
        &mut manifest,
        &mut one(),
        &mut cast,
        LIMITS,
        |_| false,
        &mut Vec::new()
    )
    .unwrap_err()
    .contains("missing"));
}
