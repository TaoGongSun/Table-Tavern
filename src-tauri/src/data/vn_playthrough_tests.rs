use super::*;
use crate::data::test_support::*;
use crate::data::*;

const TEMPLATE: &str = "{name}（第 {n} 周目）";

fn event(text: &str, opening: bool) -> TranscriptEvent {
    TranscriptEvent {
        dice_pool: None,
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
        raw: None,
        ts: "2026-11-02T00:00:00Z".to_owned(),
        speaker_id: String::new(),
        speaker_name: "GM".to_owned(),
        kind: TranscriptKind::Narration,
        text: text.to_owned(),
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening,
    }
}

/// 玩過的 VN 桌：開場＋一句、換過一章（第 0 幕留下角色隱藏快照）、有立繪背景與本機狀態檔
fn played_table(root: &Path) -> (String, String, String) {
    let world_id = create_world_of_kind(root, "霧港", TableKind::Vn).unwrap();
    let hero = new_id();
    write_character(root, &world_id, &character_card(&hero, "艾琳")).unwrap();
    set_player_card(root, &world_id, Some(hero.clone())).unwrap();
    let npc = new_id();
    write_character(root, &world_id, &character_card(&npc, "港務官")).unwrap();
    write_world_md(root, &world_id, "港口設定").unwrap();
    append_transcript(root, &world_id, 0, &event("霧裡的港口。", true)).unwrap();
    append_transcript(root, &world_id, 0, &event("你走上碼頭。", false)).unwrap();
    begin_next_scene_as(root, &world_id, "摘要", None, true).unwrap();
    set_character_auto_hidden(root, &world_id, &npc, true).unwrap();
    let dir = world_dir(root, &world_id).unwrap();
    for (path, body) in [
        ("vn/sprites/x/neutral.png", "立繪"),
        ("vn/backgrounds/bg.png", "背景"),
        ("lanes.json", "{}"),
        ("world-info/0.json", "{}"),
    ] {
        commit_world_write_atomic(&dir.join(path), body.as_bytes()).unwrap();
    }
    (world_id, hero, npc)
}

#[test]
fn new_playthrough_copies_the_whitelist_and_reseeds_the_story() {
    let root = TestRoot::new("vn-playthrough");
    let (world_id, hero, npc) = played_table(root.path());
    let second = new_playthrough(root.path(), &world_id, TEMPLATE).unwrap();
    let state = read_state(root.path(), &second).unwrap();
    assert_eq!(state.name, "霧港（第 2 周目）");
    assert_eq!(
        (state.current_scene, state.player_card_id.as_deref()),
        (0, Some(hero.as_str()))
    );
    assert_eq!(vn::chapter_in(&state.state.tree), 1);
    assert_eq!(vn::ended_in(&state.state.tree), None);
    assert_eq!(read_world_md(root.path(), &second).unwrap(), "港口設定");
    // 角色 id 沿用（主角卡不在名單裡）、隱藏照第 0 幕快照還原
    let cast = list_characters(root.path(), &second).unwrap();
    assert_eq!(cast.len(), 1);
    assert!(cast[0].id == npc && !cast[0].auto_hidden);
    // 第 0 幕只留開場，快照重蓋成新桌狀態
    let opening = read_transcript(root.path(), &second, 0).unwrap();
    assert_eq!(opening.len(), 1);
    assert_eq!(opening[0].text, "霧裡的港口。");
    assert_eq!(
        opening[0].state.as_ref().map(|s| vn::chapter_in(&s.tree)),
        Some(1)
    );
    let dir = world_dir(root.path(), &second).unwrap();
    assert!(
        dir.join("vn/sprites/x/neutral.png").exists() && dir.join("vn/backgrounds/bg.png").exists()
    );
    for left in ["lanes.json", "world-info", "transcript/1.jsonl"] {
        assert!(!dir.join(left).exists(), "{left} 不該帶到新周目");
    }
    let file = vn::read_vn(root.path(), &second).unwrap();
    assert!(file.frozen.is_none());
    // 第三周目的桌名從第 1 周目桌名數起
    let third = new_playthrough(root.path(), &second, TEMPLATE).unwrap();
    assert_eq!(
        read_state(root.path(), &third).unwrap().name,
        "霧港（第 3 周目）"
    );
}

#[test]
fn new_playthrough_failure_leaves_no_table_and_the_source_untouched() {
    let root = TestRoot::new("vn-playthrough-fail");
    let (world_id, _, _) = played_table(root.path());
    let before = read_state(root.path(), &world_id).unwrap();
    for step in ["files", "characters", "vn", "state", "opening"] {
        let failed = new_playthrough_with(root.path(), &world_id, TEMPLATE, &mut |at| {
            if at == step {
                Err(invalid_data("故障注入"))
            } else {
                Ok(())
            }
        });
        assert!(failed.is_err(), "{step}");
        assert_eq!(
            list_worlds(root.path()).unwrap().len(),
            1,
            "{step} 留下半桌"
        );
    }
    assert_eq!(read_state(root.path(), &world_id).unwrap(), before);
}

#[test]
fn new_playthrough_keeps_variable_mode_from_the_opening_table() {
    let root = TestRoot::new("vn-playthrough-vars");
    let world_id = create_world_of_kind(root.path(), "公會", TableKind::Vn).unwrap();
    let table = message_vars::parse_json(r#"{"stat_data":{"hp":"5"}}"#).unwrap();
    let epoch = state_commit::with_commit(root.path(), &world_id, |tx| {
        message_vars::publish_imported_seed(tx, 0, None, &table)
    })
    .unwrap();
    let mut opening = event("大廳。", true);
    opening.message_vars = Some(message_vars::VarsTable::from_json(&table));
    opening.vars_epoch = Some(epoch.clone());
    append_transcript(root.path(), &world_id, 0, &opening).unwrap();
    let second = new_playthrough(root.path(), &world_id, TEMPLATE).unwrap();
    let control = message_vars::read_control(root.path(), &second).unwrap();
    let scene = control.active(0).expect("新桌仍是變數模式");
    assert_ne!(scene.epoch, epoch);
    let first = &read_transcript(root.path(), &second, 0).unwrap()[0];
    assert_eq!(first.vars_epoch.as_deref(), Some(scene.epoch.as_str()));
    let state = read_state(root.path(), &second).unwrap();
    assert!(matches!(state.state.tree.get("hp"), Some(StateNode::Leaf(hp)) if hp == "5"));
}

#[test]
fn new_playthrough_keeps_the_refactored_play_style() {
    let root = TestRoot::new("vn-playthrough-refactor");
    let (world_id, hero, _) = played_table(root.path());
    write_interface_shell(root.path(), &world_id, "<UI>{{本回合.正文}}</UI>").unwrap();
    update_state(root.path(), &world_id, |state| {
        state.refactor_mode = Some("characters".to_owned());
        state.statusbar_retired = true;
        state.regex_allowed = false;
        state
            .branch_bindings
            .insert(hero.clone(), vec!["角色".to_owned()]);
        Ok(Some(()))
    })
    .unwrap();
    let second = new_playthrough(root.path(), &world_id, TEMPLATE).unwrap();
    let state = read_state(root.path(), &second).unwrap();
    assert_eq!(state.refactor_mode.as_deref(), Some("characters"));
    assert!(state.statusbar_retired && !state.regex_allowed);
    assert_eq!(
        state.branch_bindings.get(&hero),
        Some(&vec!["角色".to_owned()])
    );
    assert_eq!(
        read_interface_shell(root.path(), &second)
            .unwrap()
            .as_deref(),
        Some("<UI>{{本回合.正文}}</UI>")
    );
}
