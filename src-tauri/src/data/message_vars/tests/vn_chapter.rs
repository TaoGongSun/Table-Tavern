//! VN 章結束在變數模式桌（plans/vn-mode-pkg5.md §2.3，Sol 方案審必改 1）：換幕交易內加的章號同步進
//! 新幕的權威表，`read_state`、摘要事件快照、退幕與分岔還原都對。
use super::*;

fn vn_mvu_world(root: &TestRoot) -> String {
    let world_id = mvu_world(root);
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    state.table_kind = data::TableKind::Vn;
    data::write_state(root.path(), &world_id, &state).unwrap();
    world_id
}

#[test]
fn chapter_end_syncs_the_variable_table_and_reverts_with_the_scene() {
    let root = TestRoot::new("vars-vn-chapter");
    let world_id = vn_mvu_world(&root);
    opening(root.path(), &world_id);
    gm(root.path(), &world_id, "t1", &[("角色.好感", "40")]);
    assert_eq!(tree_leaf(root.path(), &world_id, &["chapter"]), None);
    let next = data::begin_next_scene_as(root.path(), &world_id, "摘要", None, true).unwrap();
    assert_eq!(next, 1);
    assert_eq!(
        tree_leaf(root.path(), &world_id, &["chapter"]).as_deref(),
        Some("2")
    );
    assert!(effective_stat(root.path(), &world_id).contains("chapter"));
    let summary = &data::read_transcript(root.path(), &world_id, 1).unwrap()[0];
    assert_eq!(
        summary
            .state
            .as_ref()
            .map(|state| data::vn::chapter_in(&state.tree)),
        Some(2)
    );
    // 新幕第一個 GM 回合讀到的仍是新章號（表沒被舊幕種子蓋回）
    gm(root.path(), &world_id, "t2", &[("角色.好感", "41")]);
    assert_eq!(
        tree_leaf(root.path(), &world_id, &["chapter"]).as_deref(),
        Some("2")
    );
    // 不是章結束的換幕不動章號
    data::begin_next_scene_as(root.path(), &world_id, "摘要", None, false).unwrap();
    assert_eq!(
        tree_leaf(root.path(), &world_id, &["chapter"]).as_deref(),
        Some("2")
    );
    data::revert_scene(root.path(), &world_id).unwrap();
    assert_eq!(
        tree_leaf(root.path(), &world_id, &["chapter"]).as_deref(),
        Some("2")
    );
    // 分岔回第 0 幕：章號跟著來源幕回到沒加章的狀態
    data::fork_scene(root.path(), &world_id, 0).unwrap();
    assert_eq!(
        data::vn::chapter_in(&data::read_state(root.path(), &world_id).unwrap().state.tree),
        1
    );
}

#[test]
fn chapter_end_publish_failure_keeps_the_variable_table_then_retries_once() {
    let root = TestRoot::new("vars-vn-chapter-fail");
    let world_id = vn_mvu_world(&root);
    opening(root.path(), &world_id);
    gm(root.path(), &world_id, "t1", &[("角色.好感", "40")]);
    data::fail_publish::arm();
    assert!(data::begin_next_scene_as(root.path(), &world_id, "摘要", None, true).is_err());
    assert_eq!(
        data::read_state(root.path(), &world_id)
            .unwrap()
            .current_scene,
        0
    );
    assert_eq!(tree_leaf(root.path(), &world_id, &["chapter"]), None);
    assert!(!effective_stat(root.path(), &world_id).contains("chapter"));
    assert_eq!(
        data::begin_next_scene_as(root.path(), &world_id, "摘要", None, true).unwrap(),
        1
    );
    assert_eq!(
        tree_leaf(root.path(), &world_id, &["chapter"]).as_deref(),
        Some("2")
    );
}
