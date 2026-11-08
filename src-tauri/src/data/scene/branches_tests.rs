use super::*;
use crate::data::test_support::*;
use crate::data::*;

fn line(root: &Path, world_id: &str, scene: u64, kind: TranscriptKind, text: &str) {
    let event = TranscriptEvent {
        dice_pool: None,
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
        raw: None,
        ts: "2026-11-02 00:00".to_owned(),
        speaker_id: String::new(),
        speaker_name: "GM".to_owned(),
        kind,
        text: text.to_owned(),
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
    };
    append_transcript(root, world_id, scene, &event).unwrap();
}

fn texts(root: &Path, world_id: &str, scene: u64) -> Vec<String> {
    read_transcript(root, world_id, scene)
        .unwrap()
        .into_iter()
        .map(|event| event.text)
        .collect()
}

fn state(root: &Path, world_id: &str) -> WorldState {
    read_state(root, world_id).unwrap()
}

/// 0（兩句）→ 換幕 1；從 0 整幕分岔成 2。1 與 2 都是尾幕，目前在 2。
fn two_branches(name: &str) -> (TestRoot, String) {
    let root = TestRoot::new(name);
    let world_id = create_world(root.path(), "分支桌").unwrap();
    let path = root.path();
    line(path, &world_id, 0, TranscriptKind::Narration, "開場");
    line(path, &world_id, 0, TranscriptKind::Player, "甲");
    begin_next_scene(path, &world_id, "第一幕摘要", None).unwrap();
    line(path, &world_id, 1, TranscriptKind::Narration, "原線續");
    assert_eq!(fork_scene(path, &world_id, 0).unwrap(), 2);
    (root, world_id)
}

/// 資料遺失路徑（§2.1）：切回低號分支後換幕或分岔，新幕號取水位＋1，另一條分支原封。
#[test]
fn new_scenes_after_switching_back_never_reuse_another_branch() {
    let (root, world_id) = two_branches("branches-high-water");
    let path = root.path();
    line(path, &world_id, 2, TranscriptKind::Narration, "分支續");
    let branch = texts(path, &world_id, 2);

    assert_eq!(switch_scene(path, &world_id, 1).unwrap(), 1);
    assert_eq!(
        begin_next_scene(path, &world_id, "第二幕摘要", None).unwrap(),
        3
    );
    assert_eq!(texts(path, &world_id, 2), branch);
    assert_eq!(scene_label(&state(path, &world_id), 3).parent, Some(1));

    // 切回 2 再分岔 0：同樣取水位＋1，換幕線原封
    assert_eq!(switch_scene(path, &world_id, 2).unwrap(), 2);
    let main = texts(path, &world_id, 3);
    assert_eq!(fork_scene(path, &world_id, 0).unwrap(), 4);
    assert_eq!(texts(path, &world_id, 3), main);
    assert_eq!(texts(path, &world_id, 2), branch);
}

/// §2.1 必改：目前幕號小於分岔來源（切回低號分支後從高號分支分岔）照收，兩條分支都原封。
#[test]
fn forks_from_a_higher_scene_than_the_current_one() {
    let (root, world_id) = two_branches("branches-fork-higher");
    let path = root.path();
    switch_scene(path, &world_id, 1).unwrap();
    let (one, two) = (texts(path, &world_id, 1), texts(path, &world_id, 2));
    assert_eq!(fork_scene(path, &world_id, 2).unwrap(), 3);
    assert_eq!(texts(path, &world_id, 3), two);
    assert_eq!(
        (texts(path, &world_id, 1), texts(path, &world_id, 2)),
        (one, two)
    );
    // 未發布的號（水位＋1）照舊拒
    assert!(fork_scene(path, &world_id, 9).is_err());
}

#[test]
fn switch_only_goes_to_another_published_tip() {
    let (root, world_id) = two_branches("branches-switch-guard");
    let path = root.path();
    for target in [2, 0, 3] {
        let error = switch_scene(path, &world_id, target)
            .unwrap_err()
            .to_string();
        assert_eq!(error, UiMsg::VnBranchNotTip.to_string(), "{target}");
    }
    assert_eq!(state(path, &world_id).current_scene, 2);
}

/// 切換發布失敗：current_scene 不動，重試成功；狀態樹換成目標幕最後一則快照。
#[test]
fn switch_publish_failure_keeps_the_current_scene() {
    let (root, world_id) = two_branches("branches-switch-fail");
    let path = root.path();
    let mut snapshot = TableState::default();
    snapshot.table.insert("place".to_owned(), "港口".to_owned());
    set_last_transcript_state(path, &world_id, 1, &snapshot).unwrap();

    super::super::lifecycle::fail_publish::arm();
    assert!(switch_scene(path, &world_id, 1).is_err());
    assert_eq!(state(path, &world_id).current_scene, 2);
    assert_eq!(switch_scene(path, &world_id, 1).unwrap(), 1);
    let now = state(path, &world_id);
    assert_eq!(now.current_scene, 1);
    assert_eq!(
        now.state.table.get("place").map(String::as_str),
        Some("港口")
    );
}

/// 退幕守門兩條：父幕已換幕往下走過；這幕被別幕當分岔起點。拒時什麼都不動。
#[test]
fn revert_refuses_after_switching_breaks_the_line() {
    let root = TestRoot::new("branches-revert-guard");
    let world_id = create_world(root.path(), "退幕桌").unwrap();
    let path = root.path();
    line(path, &world_id, 0, TranscriptKind::Narration, "開場");
    begin_next_scene(path, &world_id, "摘要", None).unwrap(); // 1
    line(path, &world_id, 1, TranscriptKind::Player, "甲");
    let answer = read_transcript(path, &world_id, 1).unwrap()[1]
        .id
        .clone()
        .unwrap();
    assert_eq!(fork_before(path, &world_id, 1, &answer).unwrap(), 2); // 2：只有摘要，parent 1

    // 2 的父幕 1 之後又換幕出 3：退 2 會讓 1 長出第二條換幕線
    switch_scene(path, &world_id, 1).unwrap();
    assert_eq!(
        begin_next_scene(path, &world_id, "摘要二", None).unwrap(),
        3
    ); // parent 1
    switch_scene(path, &world_id, 2).unwrap();
    let error = revert_scene(path, &world_id).unwrap_err().to_string();
    assert_eq!(error, UiMsg::SceneRewindNotTip.to_string());
    assert_eq!(state(path, &world_id).current_scene, 2);
    assert_eq!(texts(path, &world_id, 2), vec!["摘要".to_owned()]);

    // 3 只有摘要，但 4 從它分出去（parent 3）：拒刪
    switch_scene(path, &world_id, 3).unwrap();
    assert_eq!(fork_scene(path, &world_id, 0).unwrap(), 4);
    switch_scene(path, &world_id, 3).unwrap();
    let error = revert_scene(path, &world_id).unwrap_err().to_string();
    assert_eq!(error, UiMsg::SceneRewindReferenced.to_string());
    assert_eq!(texts(path, &world_id, 3), vec!["摘要二".to_owned()]);
}

/// 選項點分岔：截到玩家句前、記截點；提交前逐字稿被改過就拒；源頭對應走到來源、來源被改寫就停。
#[test]
fn fork_before_records_the_cut_and_origin_follows_it() {
    let root = TestRoot::new("branches-origin");
    let world_id = create_world(root.path(), "源頭桌").unwrap();
    let path = root.path();
    line(path, &world_id, 0, TranscriptKind::Narration, "選項");
    line(path, &world_id, 0, TranscriptKind::Dialogue, "角色插話");
    line(path, &world_id, 0, TranscriptKind::Player, "甲");
    let events = read_transcript(path, &world_id, 0).unwrap();
    let error = fork_before(path, &world_id, 2, "別人")
        .unwrap_err()
        .to_string();
    assert_eq!(error, UiMsg::SceneStale.to_string());
    assert_eq!(state(path, &world_id).current_scene, 0);

    let answer = events[2].id.clone().unwrap();
    assert_eq!(fork_before(path, &world_id, 2, &answer).unwrap(), 1);
    assert_eq!(
        texts(path, &world_id, 1),
        vec!["選項".to_owned(), "角色插話".to_owned()]
    );
    let label = scene_label(&state(path, &world_id), 1);
    assert_eq!(
        (label.keep, label.cut.as_deref()),
        (Some(2), events[1].id.as_deref())
    );

    let mut book = Transcripts::default();
    let now = state(path, &world_id);
    assert_eq!(origin(path, &world_id, &now, &mut book, 1, 0).unwrap(), 0);
    assert_eq!(origin(path, &world_id, &now, &mut book, 1, 2).unwrap(), 1);

    // 來源被收回改寫（截點那則換了 id）：不再對應
    switch_scene(path, &world_id, 0).unwrap();
    pop_transcript(path, &world_id, 0).unwrap();
    pop_transcript(path, &world_id, 0).unwrap();
    line(path, &world_id, 0, TranscriptKind::Dialogue, "改寫");
    let mut book = Transcripts::default();
    let now = state(path, &world_id);
    assert_eq!(origin(path, &world_id, &now, &mut book, 1, 0).unwrap(), 1);
}

#[test]
fn choice_state_uses_the_first_player_line_after_the_choice() {
    use Beat::{Choice as C, Other as O, Player as P};
    let cases: &[(&[Beat], usize, ChoiceState)] = &[
        (&[C, P], 0, ChoiceState::Answered(1)),
        (&[C, O, O, P], 0, ChoiceState::Answered(3)),
        (&[C], 0, ChoiceState::Live),
        (&[C, O], 0, ChoiceState::Live),
        (&[C, O, C, P], 0, ChoiceState::Superseded),
        (&[C, O, C, P], 2, ChoiceState::Answered(3)),
    ];
    for (beats, k, expected) in cases {
        assert_eq!(choice_state(beats, *k), *expected, "{beats:?} {k}");
    }
}

/// 玩家句只寫目前幕：舊幕號（畫面還沒換手）拒收、逐字稿不動。
#[test]
fn player_lines_only_land_in_the_current_scene() {
    let (root, world_id) = two_branches("branches-stale-player");
    let path = root.path();
    let before = texts(path, &world_id, 1);
    let mut event = read_transcript(path, &world_id, 0).unwrap()[1].clone();
    event.id = None;
    let error = append_event(path, &world_id, 1, &event, None)
        .unwrap_err()
        .to_string();
    assert_eq!(error, UiMsg::SceneStale.to_string());
    assert_eq!(texts(path, &world_id, 1), before);
    assert!(append_event(path, &world_id, 2, &event, None).is_ok());
}

#[test]
fn tips_and_story_chain_follow_labels() {
    let (root, world_id) = two_branches("branches-tips");
    let now = state(root.path(), &world_id);
    assert_eq!(scene_high(&now), 2);
    assert_eq!(tip_of(&now, 0), 1);
    assert_eq!(tip_of(&now, 2), 2);
    assert_eq!(story_previous(&now, 2), Some(0));
    assert_eq!(story_previous(&now, 1), Some(0));
}
