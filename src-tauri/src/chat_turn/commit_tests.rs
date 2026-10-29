use super::super::gm_reply::gm_reply_parts;
use crate::data;

/// playdata-loss-fixes 11a：照抄段裡本回合沒報的新值，合成更新區塊走一般桌與接管桌真正的提交路徑收進樹；
/// 本回合有 delta 的路徑不合成（不重算）；收不進去的值列給狀態更新事件。
#[test]
fn echoed_new_values_are_committed_on_plain_and_takeover_tables() {
    for takeover in [false, true] {
        let mut state: data::WorldState = serde_json::from_value(serde_json::json!({
            "id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "name": "照抄桌",
            "current_scene": 0
        }))
        .unwrap();
        for (key, value) in [("驻守天数", "2"), ("心情", "平静"), ("好感", "10")] {
            data::set_tree_value(&mut state.state.tree, &[key.to_owned()], value);
        }
        state.mechanism.incremental = true;
        let tree = state.state.tree.clone();
        let reply = "托倫點頭。\n\n<UpdateVariable>\n<JSONPatch>\n[{\"op\":\"delta\",\"path\":\"/好感\",\"value\":5}]\n</JSONPatch>\n</UpdateVariable>\n\n（系統）狀態更新\n驻守天数：3\n心情：平静\n好感：15";
        let parts = gm_reply_parts(reply, &tree, false);
        assert_eq!(parts.display, "托倫點頭。");
        assert_eq!(
            parts.pending,
            vec![(vec!["驻守天数".to_owned()], "3".to_owned())]
        );
        let loaded = state.mechanism.clone();
        super::gm_commit_apply(
            &mut state,
            None,
            &parts.block,
            "玩家",
            &loaded,
            takeover,
            false,
        );
        let at = |key: &str| data::node_at(&state.state.tree, &[key.to_owned()]).cloned();
        assert_eq!(
            at("驻守天数"),
            Some(data::StateNode::Leaf("3".to_owned())),
            "{takeover}"
        );
        assert_eq!(
            at("好感"),
            Some(data::StateNode::Leaf("15".to_owned())),
            "delta 不重算"
        );
        assert!(crate::mechanism::unapplied(&state, &parts.pending).is_empty());
        // 樹上沒有的路徑收不進去 → 列給狀態更新事件
        let missing = vec![(vec!["軍糧".to_owned()], "三百石".to_owned())];
        assert_eq!(
            crate::mechanism::unapplied(&state, &missing),
            vec![("軍糧".to_owned(), "三百石".to_owned())]
        );
    }
}

fn leftover_world(retired: bool, cells: &[(&str, &str)]) -> data::WorldState {
    let mut state: data::WorldState = serde_json::from_value(serde_json::json!({
        "id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "name": "照抄桌",
        "current_scene": 0
    }))
    .unwrap();
    for (path, value) in cells {
        let path: Vec<String> = path.split('.').map(str::to_owned).collect();
        data::set_tree_value(&mut state.state.tree, &path, value);
    }
    state.mechanism.incremental = true;
    state.statusbar_retired = retired;
    state.refactor_mode = retired.then(|| "characters".to_owned());
    state
}

/// 拆回覆 → 提交 → 狀態更新事件的行，同 gm_narrate 的接線。
fn run_turn(state: &mut data::WorldState, reply: &str) -> Vec<(String, String)> {
    let tree = state.state.tree.clone();
    let parts = gm_reply_parts(reply, &tree, state.statusbar_retired);
    let loaded = state.mechanism.clone();
    super::gm_commit_apply(state, None, &parts.block, "玩家", &loaded, false, false);
    super::state_update_lines(state, "玩家", &parts.pending)
        .into_iter()
        .map(|update| (update.path, update.value))
        .collect()
}

fn leaf_of(state: &data::WorldState, path: &str) -> Option<String> {
    let path: Vec<String> = path.split('.').map(str::to_owned).collect();
    match data::node_at(&state.state.tree, &path) {
        Some(data::StateNode::Leaf(value)) => Some(value.clone()),
        _ => None,
    }
}

/// Sol 驗收 1：GM 用繁體別名 delta、照抄段寫簡體新值，或整項更新父路徑：照抄段不再補收，數值不算兩次。
#[test]
fn echoes_of_alias_or_parent_reports_are_not_counted_twice() {
    let mut state = leftover_world(false, &[("体力", "10"), ("人物.林遠.心情", "平静")]);
    let lines = run_turn(
            &mut state,
            "林遠喘口氣。\n<UpdateVariable><JSONPatch>[{\"op\":\"delta\",\"path\":\"/體力\",\"value\":5},{\"op\":\"replace\",\"path\":\"/人物/林遠\",\"value\":{\"心情\":\"愉快\"}}]</JSONPatch></UpdateVariable>\n\n（系統）狀態更新\n体力：15\n人物.林遠.心情：愉快",
        );
    assert_eq!(leaf_of(&state, "体力").as_deref(), Some("15"));
    assert_eq!(leaf_of(&state, "人物.林遠.心情").as_deref(), Some("愉快"));
    assert!(lines.iter().all(|(path, _)| path != "体力"), "{lines:?}");
}

/// Sol 驗收 3：退場桌卡片寫舊值、GM 本回合正式更新同一格：以正式更新為準，舊卡片值不追加進狀態更新；
/// 真的沒收的格子（樹上沒有）整值追加。
#[test]
fn card_values_superseded_by_formal_reports_are_not_appended() {
    let mut state = leftover_world(true, &[("体力", "10")]);
    let lines = run_turn(
            &mut state,
            "林遠喘口氣。\n<details><summary>状态栏</summary>\n体力: 10\n军需: 三百石\n</details>\n<UpdateVariable><JSONPatch>[{\"op\":\"delta\",\"path\":\"/体力\",\"value\":5}]</JSONPatch></UpdateVariable>",
        );
    assert_eq!(leaf_of(&state, "体力").as_deref(), Some("15"));
    assert_eq!(lines, vec![("军需".to_owned(), "三百石".to_owned())]);
}

/// 只有清單（`鍵: []`）的狀態區塊也要套用：非增量桌照樣清空那支清單
#[test]
fn list_only_status_block_still_applies() {
    let mut state: data::WorldState = serde_json::from_value(serde_json::json!({
        "id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "name": "清單桌",
        "current_scene": 0
    }))
    .unwrap();
    data::set_tree_value(
        &mut state.state.tree,
        &["名單".to_owned(), "0".to_owned()],
        "林远",
    );
    let block = crate::transport::extract_state_block("旁白<Status_block>名單: []</Status_block>");
    assert!(block.fields.is_empty() && block.updates.is_empty());
    let loaded = state.mechanism.clone();
    let (outcome, _) =
        super::gm_commit_apply(&mut state, None, &block, "阿濤", &loaded, false, false);
    assert!(outcome.is_some());
    assert_eq!(
        state.state.tree.get("名單"),
        Some(&data::StateNode::Branch(Default::default()))
    );
}
