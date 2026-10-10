//! 開場白的巨集與副作用落地（方案三之 7、三之 8）：顯示中性、落檔求值前先結算、落地日誌與崩潰恢復、寫不成退掉。
use super::macro_tests::{layer, set_op, write};
use super::tests::{TempRoot, LANG};
use super::*;
use crate::data;
use crate::data::card_vars::Layer;
use crate::data::world_info_store::{self as store, Perspective};
use crate::st_macros::variables::ScopeKind;
use serde_json::json;

/// 開場白：清單顯示中性、不寫變數；落檔完整求值，副作用走落地日誌（寫完清掉）。
#[test]
fn openings_display_neutrally_and_land_side_effects() {
    let root = TempRoot::new("macro-opening");
    let world = data::create_world(root.path(), "開場").unwrap();
    let path = crate::scene_budget::PathLimits::default();
    let macros = opening::OpeningMacros::load(root.path(), &world, "莫拉", &path, LANG, false);
    let raw_opening = "{{setvar::來客::{{user}}}}歡迎，{{getvar::來客}}。我是 {{char}}。";
    assert_eq!(macros.display(raw_opening), "歡迎，玩家。我是 莫拉。");
    let (text, ops) = macros.evaluate(raw_opening);
    assert_eq!(text, "歡迎，玩家。我是 莫拉。");
    assert_eq!(ops.len(), 1);
    assert_eq!(layer(&root, &world, Layer::Chat).rev, None, "求值本身不寫");
    let held = data::try_world_exclusive(&world).unwrap();
    crate::import::post_opening_text_with(
        root.path(),
        &world,
        0,
        "2026-10-11 12:00",
        &text,
        LANG,
        None,
        None,
        &held,
        &mut opening::OpeningLanding::new(root.path(), &world, 0, ops),
    )
    .unwrap();
    assert_eq!(layer(&root, &world, Layer::Chat).vars, r#"{"來客":"玩家"}"#);
    assert!(store::read_scene(root.path(), &world, 0)
        .unwrap()
        .pending
        .is_none());
}

/// 開場白落地途中崩潰的三個點，下一次結算都辨識得出：
/// - 寫完變數、追加之前：寫入中、逐字稿沒有回合鍵——自己寫的撤回，別人之後寫過的保留並回報；
/// - 追加之後、標記之前，或標記之後、清掉之前：逐字稿有回合鍵——判成功，副作用保留、不回報。
#[test]
fn opening_crash_is_recognised_by_the_next_settle() {
    use crate::import::OpeningEffects;
    /// 照常開日誌、寫變數；追加後「崩潰」（`mark`＝崩潰前已標記已送出）。
    struct Crashing<'a> {
        inner: opening::OpeningLanding<'a>,
        mark: bool,
        turn: String,
    }
    impl OpeningEffects for Crashing<'_> {
        fn begin(&mut self) -> Result<(), String> {
            self.inner.begin()
        }
        fn turn_key(&self) -> Option<crate::data::message_vars::TurnKey> {
            self.inner.turn_key()
        }
        fn commit(&mut self) {
            self.turn = self.inner.turn_key().unwrap().turn_id;
        }
        fn abort(&mut self) {}
    }
    #[derive(Clone, Copy, PartialEq)]
    enum Point {
        BeforeAppend { overwrite: bool },
        AfterAppend,
        AfterMark,
    }
    let crash = |label: &str, point: Point| {
        let root = TempRoot::new(label);
        let world = data::create_world(root.path(), "開場崩潰").unwrap();
        let ops = vec![set_op(ScopeKind::Local, "來客", "玩家")];
        let landing = opening::OpeningLanding::new(root.path(), &world, 0, ops);
        let mut effects = Crashing {
            inner: landing,
            mark: point == Point::AfterMark,
            turn: String::new(),
        };
        match point {
            Point::BeforeAppend { overwrite } => {
                effects.begin().unwrap();
                let pending = store::read_scene(root.path(), &world, 0)
                    .unwrap()
                    .pending
                    .unwrap();
                assert!(pending.turn_key.starts_with("opening:"));
                assert!(pending.vars[0].after_rev.is_some());
                if overwrite {
                    let rev = layer(&root, &world, Layer::Chat).rev;
                    write(&root, &world, Layer::Chat, rev.as_deref(), r#"{"別人":1}"#);
                }
            }
            Point::AfterAppend | Point::AfterMark => {
                let held = data::try_world_exclusive(&world).unwrap();
                crate::import::post_opening_text_with(
                    root.path(),
                    &world,
                    0,
                    "2026-10-11 12:00",
                    "開場",
                    LANG,
                    None,
                    None,
                    &held,
                    &mut effects,
                )
                .unwrap();
                if effects.mark {
                    store::mark_sent(root.path(), &world, 0, &effects.turn).unwrap();
                }
                let pending = store::read_scene(root.path(), &world, 0)
                    .unwrap()
                    .pending
                    .unwrap();
                assert_eq!(pending.turn_key, effects.turn);
            }
        }
        store::settle_pending(root.path(), &world, 0).unwrap();
        assert!(store::read_scene(root.path(), &world, 0)
            .unwrap()
            .pending
            .is_none());
        (
            layer(&root, &world, Layer::Chat),
            store::read_notices(root.path(), &world).unwrap(),
        )
    };
    let (chat, notices) = crash(
        "macro-opening-crash",
        Point::BeforeAppend { overwrite: false },
    );
    assert_eq!(chat.rev, None, "自己寫的撤回成沒有檔");
    assert!(notices.is_empty());
    let (chat, notices) = crash(
        "macro-opening-crash-kept",
        Point::BeforeAppend { overwrite: true },
    );
    assert_eq!(chat.vars, r#"{"別人":1}"#);
    assert_eq!(notices.len(), 1);
    assert!(notices[0].turn_key.starts_with("opening:"));
    for (label, point) in [
        ("macro-opening-crash-appended", Point::AfterAppend),
        ("macro-opening-crash-marked", Point::AfterMark),
    ] {
        let (chat, notices) = crash(label, point);
        assert_eq!(chat.vars, r#"{"來客":"玩家"}"#, "開場白已落檔：副作用保留");
        assert!(notices.is_empty());
    }
}

/// 崩潰日誌：chat 層前像 x=0、暫寫 x=42，pending 停在寫入中。
fn crash_log_x42(root: &TempRoot, world: &str) {
    let before_rev = write(root, world, Layer::Chat, None, r#"{"x":0}"#);
    store::begin_landing(
        root.path(),
        world,
        0,
        "t1",
        &Perspective::Gm,
        &Default::default(),
    )
    .unwrap();
    store::push_var_intent(
        root.path(),
        world,
        0,
        "t1",
        store::VarIntent {
            layer: Layer::Chat,
            id: None,
            expected_rev: Some(before_rev.clone()),
            before: r#"{"x":0}"#.to_owned(),
            ops: json!([]),
            after_rev: None,
            restore_rev: None,
        },
    )
    .unwrap();
    let after_rev = write(root, world, Layer::Chat, Some(&before_rev), r#"{"x":42}"#);
    store::update_var_intent(root.path(), world, 0, "t1", 0, |intent| {
        intent.after_rev = Some(after_rev.clone());
    })
    .unwrap();
}

/// 開場白求值之前先結算：崩潰留下的寫入中落地（x 暫寫 42、前像 0）先撤回，`{{getvar::x}}` 讀到的是 0。
#[test]
fn opening_settles_before_evaluating() {
    let root = TempRoot::new("macro-opening-settle-first");
    let world = data::create_world(root.path(), "開場先結算").unwrap();
    crash_log_x42(&root, &world);
    let path = crate::scene_budget::PathLimits::default();
    let (_, ops) = opening::evaluate_for_posting(
        root.path(),
        &world,
        0,
        "莫拉",
        "{{setvar::y::{{getvar::x}}}}",
        &path,
        LANG,
    )
    .unwrap();
    assert_eq!(layer(&root, &world, Layer::Chat).vars, r#"{"x":0}"#);
    let held = data::try_world_exclusive(&world).unwrap();
    crate::import::post_opening_text_with(
        root.path(),
        &world,
        0,
        "2026-10-11 12:00",
        "開場",
        LANG,
        None,
        None,
        &held,
        &mut opening::OpeningLanding::new(root.path(), &world, 0, ops),
    )
    .unwrap();
    assert_eq!(layer(&root, &world, Layer::Chat).vars, r#"{"x":0,"y":"0"}"#);
}

/// 開場白副作用寫不成：開場白不留在逐字稿，落地日誌也撤掉。
#[test]
fn opening_hook_failure_rolls_the_opening_back() {
    let root = TempRoot::new("macro-opening-fail");
    let world = data::create_world(root.path(), "開場失敗").unwrap();
    let held = data::try_world_exclusive(&world).unwrap();
    let before = data::read_transcript(root.path(), &world, 0).unwrap().len();
    // global 層壞掉：已寫的 chat 撤回、日誌撤掉、開場白不貼
    let global = root.path().join("card-vars").join("global.json");
    std::fs::create_dir_all(global.parent().unwrap()).unwrap();
    std::fs::write(&global, "{壞掉").unwrap();
    let ops = vec![
        set_op(ScopeKind::Local, "x", "1"),
        set_op(ScopeKind::Global, "g", "1"),
    ];
    let result = crate::import::post_opening_text_with(
        root.path(),
        &world,
        0,
        "2026-10-11 12:00",
        "開場",
        LANG,
        None,
        None,
        &held,
        &mut opening::OpeningLanding::new(root.path(), &world, 0, ops),
    );
    assert!(result.is_err());
    assert_eq!(
        data::read_transcript(root.path(), &world, 0).unwrap().len(),
        before
    );
    assert!(store::read_scene(root.path(), &world, 0)
        .unwrap()
        .pending
        .is_none());
    assert_eq!(
        layer(&root, &world, Layer::Chat).rev,
        None,
        "已寫的 chat 撤回"
    );
}

/// 只有 `{{getvar}}`（沒有變數操作）的開場白也先結算：讀到恢復後的 0，崩潰日誌清掉。
#[test]
fn getvar_only_openings_also_settle_first() {
    let root = TempRoot::new("macro-opening-getvar-settle");
    let world = data::create_world(root.path(), "只讀變數").unwrap();
    crash_log_x42(&root, &world);
    let path = crate::scene_budget::PathLimits::default();
    let (text, ops) = opening::evaluate_for_posting(
        root.path(),
        &world,
        0,
        "莫拉",
        "x 是 {{getvar::x}}",
        &path,
        LANG,
    )
    .unwrap();
    assert_eq!(text, "x 是 0");
    assert!(ops.is_empty());
    assert!(store::read_scene(root.path(), &world, 0)
        .unwrap()
        .pending
        .is_none());
}

/// 追加失敗、逐字稿也回不到貼之前：不撤回、pending 留著；之後結算看逐字稿——沒有這個回合鍵就撤回變數，
/// 有（追加其實寫成了）就判成功、保留副作用。
#[test]
fn failed_append_without_rollback_leaves_the_log_to_the_transcript() {
    for landed in [false, true] {
        let root = TempRoot::new(if landed {
            "macro-opening-unrestorable-kept"
        } else {
            "macro-opening-unrestorable"
        });
        let world = data::create_world(root.path(), "回不去").unwrap();
        let transcript = root
            .path()
            .join("worlds")
            .join(&world)
            .join("transcript")
            .join("0.jsonl");
        assert!(!transcript.exists());
        let held = data::try_world_exclusive(&world).unwrap();
        let ops = vec![set_op(ScopeKind::Local, "來客", "玩家")];
        {
            let _append = data::AppendFailGuard::partial(1);
            let _remove = data::RemoveFailGuard::fail_ending("0.jsonl", 1);
            assert!(crate::import::post_opening_text_with(
                root.path(),
                &world,
                0,
                "2026-10-11 12:00",
                "開場",
                LANG,
                None,
                None,
                &held,
                &mut opening::OpeningLanding::new(root.path(), &world, 0, ops),
            )
            .is_err());
        }
        let pending = store::read_scene(root.path(), &world, 0)
            .unwrap()
            .pending
            .expect("回不去就不撤，pending 留著");
        assert_eq!(layer(&root, &world, Layer::Chat).vars, r#"{"來客":"玩家"}"#);
        std::fs::remove_file(&transcript).unwrap();
        if landed {
            data::append_opening_keyed(
                root.path(),
                &world,
                0,
                "2026-10-11 12:00",
                "開場",
                &crate::transport::extract_state_block("開場"),
                "玩家",
                Some(crate::data::message_vars::TurnKey {
                    turn_id: pending.turn_key.clone(),
                    part: crate::data::message_vars::PART_OPENING.to_owned(),
                }),
            )
            .unwrap();
        }
        store::settle_pending(root.path(), &world, 0).unwrap();
        assert!(store::read_scene(root.path(), &world, 0)
            .unwrap()
            .pending
            .is_none());
        match landed {
            true => assert_eq!(layer(&root, &world, Layer::Chat).vars, r#"{"來客":"玩家"}"#),
            false => assert_eq!(layer(&root, &world, Layer::Chat).rev, None),
        }
        assert!(store::read_notices(root.path(), &world).unwrap().is_empty());
    }
}
