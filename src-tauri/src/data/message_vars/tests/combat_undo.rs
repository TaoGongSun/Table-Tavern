//! ttrpg-combat-tracker 包 1：戰鬥欄跟著逐則快照——收回 GM 那一樓，先攻、HP、敵人回到前一則；復原再回來。
use super::*;

fn combat_turn(root: &Path, world_id: &str, turn_id: &str, ops: &str) {
    begin(root, world_id, turn_id);
    let reply =
        format!("旁白。\n<UpdateVariable>\n<JSONPatch>\n[{ops}]\n</JSONPatch>\n</UpdateVariable>");
    let block = crate::transport::extract_state_block(&reply);
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
                crate::mechanism::apply_block(state, &block, "沃恩");
                ((), None)
            },
            |_, _| Vec::new(),
        )
    })
    .unwrap();
    assert!(committed.is_some());
    append_part(root, world_id, turn_id, PART_MAIN, &format!("GM {turn_id}")).unwrap();
}

#[test]
fn undo_and_restore_carry_combat_fields() {
    let root = TestRoot::new("combat-undo");
    let world_id = data::create_world(root.path(), "戰鬥桌").unwrap();
    for name in ["沃恩", "雷克"] {
        let card = data::CharacterCard {
            id: data::new_id(),
            name: name.to_owned(),
            color: "#336699".to_owned(),
            avatar: "🐗".to_owned(),
            tier: data::Tier::Balanced,
            show_image: true,
            archived: false,
            gen_prompt: String::new(),
            public_md: "獸人戰士。".to_owned(),
            private_md: String::new(),
            stage: false,
        };
        data::write_character(root.path(), &world_id, &card).unwrap();
    }
    data::update_state(root.path(), &world_id, |state| {
        state.dice_enabled = true;
        Ok(Some(state.clone()))
    })
    .unwrap();
    assert!(data::default_panel::ensure(root.path(), &world_id).unwrap());
    data::append_transcript(
        root.path(),
        &world_id,
        0,
        &event(TranscriptKind::Narration, "開場白"),
    )
    .unwrap();
    combat_turn(
        root.path(),
        &world_id,
        "t1",
        r#"{"op":"replace","path":"/characters/雷克/hp","value":"20/20"}"#,
    );
    player(root.path(), &world_id, "雷克拔斧");
    combat_turn(
        root.path(),
        &world_id,
        "t2",
        r#"{"op":"replace","path":"/combat/order","value":["雷克","哥布林A"]},
           {"op":"insert","path":"/combat/foes/哥布林A","value":{"hp":"7/7"}},
           {"op":"replace","path":"/combat/turn","value":"哥布林A"},
           {"op":"delta","path":"/characters/雷克/hp","value":-6}"#,
    );
    let after = |root: &Path| {
        (
            tree_leaf(root, &world_id, &["characters", "雷克", "hp"]),
            tree_leaf(root, &world_id, &["combat", "turn"]),
            tree_leaf(root, &world_id, &["combat", "foes", "哥布林A", "hp"]),
        )
    };
    let fought = (
        Some("14/20".to_owned()),
        Some("哥布林A".to_owned()),
        Some("7/7".to_owned()),
    );
    assert_eq!(after(root.path()), fought);
    let removed = data::pop_transcript_floor(root.path(), &world_id, 0).unwrap();
    assert_eq!(
        after(root.path()),
        (Some("20/20".to_owned()), Some(String::new()), None)
    );
    data::restore_transcript_events(root.path(), &world_id, 0, &removed).unwrap();
    assert_eq!(after(root.path()), fought);
}
