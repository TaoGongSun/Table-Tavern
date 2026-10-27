//! 收回一樓／整組復原（undo-two-clicks-summary-line）：用真實 marker 與 action_id 排出 GM 回合的
//! 登場塊（多卡私設、登場交錯）＋正文＋變動摘要，一次收回後樹值與平欄都回到玩家句那一刻，復原整組放回。
use super::*;
use crate::data::EventMarker;

const ACTION: &str = "act-1";

fn leaf(root: &Path, world_id: &str) -> Option<String> {
    tree_leaf(root, world_id, &["角色", "好感"])
}

fn flat(root: &Path, world_id: &str) -> Option<String> {
    data::read_state(root, world_id)
        .unwrap()
        .state
        .table
        .get("time")
        .cloned()
}

fn marked(marker: EventMarker, text: &str) -> TranscriptEvent {
    TranscriptEvent {
        marker: Some(marker),
        action_id: Some(ACTION.to_owned()),
        ..event(TranscriptKind::System, text)
    }
}

fn player_line(root: &Path, world_id: &str) {
    let scene = scene_of(root, world_id);
    let line = TranscriptEvent {
        action_id: Some(ACTION.to_owned()),
        ..event(TranscriptKind::Player, "玩家一句")
    };
    data::append_event(root, world_id, scene, &line, None).unwrap();
}

/// 一整輪 GM：提交（改樹與平欄）→ 登場塊（人物登場、私設、登場、私設、登場）→ 正文 → 變動摘要，全帶同一個動作 id。
fn gm_turn(root: &Path, world_id: &str, turn_id: &str) {
    let scene = scene_of(root, world_id);
    let ticket = with_commit(root, world_id, |tx| {
        begin_turn(tx, turn_id, Some(&macros()))
    })
    .unwrap();
    let committed = with_commit(root, world_id, |tx| {
        let main = PendingMain {
            text: format!("GM {turn_id}"),
            raw: None,
            truncated: false,
        };
        apply_gm_block(
            tx,
            turn_id,
            main,
            |state, _| {
                data::set_tree_value(&mut state.state.tree, &["角色".into(), "好感".into()], "42");
                state
                    .state
                    .table
                    .insert("time".to_owned(), "深夜".to_owned());
                ((), None)
            },
            |_, _| {
                vec![TurnSide {
                    part: "state_update".to_owned(),
                    kind: TranscriptKind::System,
                    text: "角色.地點：酒館".to_owned(),
                    marker: Some(EventMarker::StateUpdate),
                }]
            },
        )
    })
    .unwrap();
    assert!(committed.is_some());
    for line in [
        marked(
            EventMarker::PersonArrival {
                title: "老闆".into(),
            },
            "人物登場",
        ),
        marked(EventMarker::CardPrivate { name: "甲".into() }, "甲私設"),
        marked(EventMarker::CardArrival { name: "甲".into() }, "甲登場"),
        marked(EventMarker::CardPrivate { name: "乙".into() }, "乙私設"),
        marked(EventMarker::CardArrival { name: "乙".into() }, "乙登場"),
    ] {
        data::append_within_turn(root, world_id, scene, &line, &ticket).unwrap();
    }
    let main = TranscriptEvent {
        action_id: Some(ACTION.to_owned()),
        ..event(TranscriptKind::Narration, &format!("GM {turn_id}"))
    };
    data::append_event(root, world_id, scene, &main, Some(&key(turn_id, PART_MAIN))).unwrap();
    let summary = marked(EventMarker::StateUpdate, "角色.地點：酒館");
    data::append_event(
        root,
        world_id,
        scene,
        &summary,
        Some(&key(turn_id, "state_update")),
    )
    .unwrap();
}

const FULL: [&str; 9] = [
    "開場白",
    "玩家一句",
    "人物登場",
    "甲私設",
    "甲登場",
    "乙私設",
    "乙登場",
    "GM t1",
    "角色.地點：酒館",
];

fn opening_line(root: &Path, world_id: &str, mvu: bool) {
    if mvu {
        opening(root, world_id);
    } else {
        data::append_transcript(
            root,
            world_id,
            0,
            &event(TranscriptKind::Narration, "開場白"),
        )
        .unwrap();
    }
}

fn one_click_rewinds_and_restore_puts_the_floor_back(mvu: bool) {
    let root = TestRoot::new(if mvu {
        "undo-floor-mvu"
    } else {
        "undo-floor-tree"
    });
    let world_id = if mvu {
        mvu_world(&root)
    } else {
        data::create_world(root.path(), "樹桌").unwrap()
    };
    opening_line(root.path(), &world_id, mvu);
    player_line(root.path(), &world_id);
    let (leaf_before, flat_before) = (leaf(root.path(), &world_id), flat(root.path(), &world_id));
    gm_turn(root.path(), &world_id, "t1");
    assert_eq!(texts(root.path(), &world_id), FULL);
    assert_eq!(leaf(root.path(), &world_id).as_deref(), Some("42"));
    assert_eq!(flat(root.path(), &world_id).as_deref(), Some("深夜"));

    // 一下收整樓：登場塊、正文、摘要一起走，玩家句留下；樹值與平欄回到玩家句那一刻
    let removed = data::pop_transcript_floor(root.path(), &world_id, 0).unwrap();
    assert_eq!(
        removed
            .iter()
            .map(|event| event.text.as_str())
            .collect::<Vec<_>>(),
        FULL[2..]
    );
    assert_eq!(texts(root.path(), &world_id), FULL[..2]);
    assert_eq!(leaf(root.path(), &world_id), leaf_before);
    assert_eq!(flat(root.path(), &world_id), flat_before);

    // 整組放回：原序、狀態回到回合後；帶表的正文換新版本 token
    let restored = data::restore_transcript_events(root.path(), &world_id, 0, &removed).unwrap();
    assert_eq!(texts(root.path(), &world_id), FULL);
    assert_eq!(leaf(root.path(), &world_id).as_deref(), Some("42"));
    assert_eq!(flat(root.path(), &world_id).as_deref(), Some("深夜"));
    if mvu {
        assert!(restored[5].message_vars.is_some());
        assert_ne!(restored[5].vars_rev, removed[5].vars_rev);
        assert!(effective_stat(root.path(), &world_id).contains("42"));
    }

    // 連按：先收 GM 那樓、再收玩家句、再收開場白，見底回空
    assert_eq!(
        data::pop_transcript_floor(root.path(), &world_id, 0)
            .unwrap()
            .len(),
        7
    );
    assert_eq!(
        data::pop_transcript_floor(root.path(), &world_id, 0)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(texts(root.path(), &world_id), ["開場白"]);
    assert_eq!(
        data::pop_transcript_floor(root.path(), &world_id, 0)
            .unwrap()
            .len(),
        1
    );
    assert!(data::pop_transcript_floor(root.path(), &world_id, 0)
        .unwrap()
        .is_empty());
}

#[test]
fn variable_mode_one_click_takes_the_whole_gm_floor() {
    one_click_rewinds_and_restore_puts_the_floor_back(true);
}

#[test]
fn tree_mode_one_click_takes_the_whole_gm_floor() {
    one_click_rewinds_and_restore_puts_the_floor_back(false);
}

#[test]
fn legacy_arrival_without_action_id_stays_its_own_floor() {
    let root = TestRoot::new("undo-floor-legacy");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    let legacy = TranscriptEvent {
        action_id: None,
        ..marked(EventMarker::CardArrival { name: "甲".into() }, "舊登場")
    };
    data::append_transcript(root.path(), &world_id, 0, &legacy).unwrap();
    gm(root.path(), &world_id, "t1", &[("角色.好感", "20")]);
    assert_eq!(
        data::pop_transcript_floor(root.path(), &world_id, 0)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(texts(root.path(), &world_id), ["開場白", "舊登場"]);
}

#[test]
fn failed_restore_writes_nothing_and_a_retry_does_not_duplicate() {
    let root = TestRoot::new("undo-floor-retry");
    let world_id = mvu_world(&root);
    opening(root.path(), &world_id);
    player_line(root.path(), &world_id);
    gm_turn(root.path(), &world_id, "t1");
    let removed = data::pop_transcript_floor(root.path(), &world_id, 0).unwrap();
    {
        let _fail = RenameFailGuard::fail_ending(".jsonl", 1);
        assert!(data::restore_transcript_events(root.path(), &world_id, 0, &removed).is_err());
    }
    assert_eq!(texts(root.path(), &world_id), FULL[..2]);
    data::restore_transcript_events(root.path(), &world_id, 0, &removed).unwrap();
    assert_eq!(texts(root.path(), &world_id), FULL);
}
