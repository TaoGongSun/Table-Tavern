use super::*;
use crate::commands::character_card;
use crate::transport::ChatMessage;

/// 依序回覆；呼叫次數記在 `calls`
struct Fake {
    replies: Vec<String>,
    calls: usize,
}

impl SummaryCaller for Fake {
    async fn call(&mut self, _messages: Vec<ChatMessage>) -> Result<String, String> {
        self.calls += 1;
        Ok(self.replies.remove(0))
    }
}

fn fake(replies: &[&str]) -> Fake {
    Fake {
        replies: replies.iter().map(|reply| reply.to_string()).collect(),
        calls: 0,
    }
}

fn bible_reply(flag: &str, token: &str) -> String {
    format!(
        "## BIBLE\n```json\n{}\n```",
        serde_json::json!({
            "logline": token,
            "chapters": [{"title": "序", "goal": "相遇", "beats": "BEATS_TOKEN"}],
            "flags": [{"id": flag, "label": "好感", "type": "counter", "initial": 1}],
            "secrets": [
                {"id": "s_irene", "holder": "艾琳", "title": "公主", "text": "她是公主"},
                {"id": "s_world", "title": "古城", "text": "城下有龍"}
            ]
        })
    )
}

fn vn_table(label: &str) -> (std::path::PathBuf, String, String) {
    let root = crate::smart_free::test_root(label);
    let world_id = data::create_world_of_kind(&root, "港口", data::TableKind::Vn).unwrap();
    let card = character_card(&data::new_id(), "艾琳");
    data::write_character(&root, &world_id, &card).unwrap();
    (root, world_id, card.id)
}

fn leaf(root: &Path, world_id: &str, path: &str) -> Option<String> {
    let state = data::read_state(root, world_id).unwrap();
    let path: Vec<String> = path.split('.').map(str::to_owned).collect();
    match data::node_at(&state.state.tree, &path) {
        Some(StateNode::Leaf(value)) => Some(value.clone()),
        _ => None,
    }
}

/// 解析失敗自動重試一次；再失敗回 FAILED、檔已建（帶 ① 的鎖）、聖經仍是 null、狀態不動
#[tokio::test]
async fn generate_retries_once_then_leaves_bible_null() {
    let (root, world_id, _) = vn_table("vn-bible-fail");
    let before = data::read_state(&root, &world_id).unwrap();
    let mut caller = fake(&["胡言亂語", "## BIBLE\n```json\n[1]\n```"]);
    let error = generate(&root, &world_id, "zh-TW", Some(false), &mut caller).await;
    assert_eq!(error, Err(FAILED.to_owned()));
    assert_eq!(caller.calls, 2);
    let file = vn::read_vn(&root, &world_id).unwrap();
    assert!(!file.spoiler_lock && file.bible.is_none());
    assert_eq!(data::read_state(&root, &world_id).unwrap(), before);
}

/// 生成：規則、子樹、凍結落檔；整份重骰把旗標表整批換掉（舊 id 刪除）；已遊玩的桌拒絕且不動
#[tokio::test]
async fn generate_installs_and_reroll_replaces_the_flag_table() {
    let (root, world_id, irene) = vn_table("vn-bible-install");
    let first = bible_reply("trust", "第一版");
    let mut caller = fake(&["壞的", &first]);
    generate(&root, &world_id, "zh-TW", None, &mut caller)
        .await
        .unwrap();
    let file = vn::read_vn(&root, &world_id).unwrap();
    assert!(file.spoiler_lock);
    let bible = file.bible.unwrap();
    assert_eq!(bible.secrets[0].holder.as_deref(), Some(irene.as_str()));
    assert_eq!(file.frozen.unwrap().bible, bible);
    assert_eq!(leaf(&root, &world_id, "flags.trust").as_deref(), Some("1"));
    assert_eq!(
        leaf(&root, &world_id, "revealed.s_irene").as_deref(),
        Some("false")
    );
    assert_eq!(leaf(&root, &world_id, "chapter").as_deref(), Some("1"));
    let mechanism = data::read_state(&root, &world_id).unwrap().mechanism;
    assert!(mechanism.incremental && mechanism.rules.contains_key("flags.trust"));

    let second = bible_reply("courage", "第二版");
    generate(&root, &world_id, "zh-TW", None, &mut fake(&[&second]))
        .await
        .unwrap();
    assert_eq!(leaf(&root, &world_id, "flags.trust"), None);
    assert_eq!(
        leaf(&root, &world_id, "flags.courage").as_deref(),
        Some("1")
    );
    let rules = data::read_state(&root, &world_id).unwrap().mechanism.rules;
    assert!(!rules.contains_key("flags.trust") && rules.contains_key("flags.courage"));

    let player = data::TranscriptEvent {
        ts: "2026-11-01T00:00:00Z".to_owned(),
        speaker_id: String::new(),
        speaker_name: "玩家".to_owned(),
        kind: data::TranscriptKind::Player,
        text: "你好".to_owned(),
        raw: None,
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
    };
    data::append_transcript(&root, &world_id, 0, &player).unwrap();
    let before = vn::read_vn(&root, &world_id).unwrap();
    let mut third = fake(&[&first]);
    let refused = generate(&root, &world_id, "zh-TW", None, &mut third).await;
    assert_eq!(refused, Err(PLAYED.to_owned()));
    assert_eq!(third.calls, 0);
    assert_eq!(vn::read_vn(&root, &world_id).unwrap(), before);
}

/// 角色秘密重骰：只換他的私設與他持有的秘密，揭露葉子與規則同步（舊 id 刪、新 id 建成 false）
#[tokio::test]
async fn secret_reroll_swaps_only_that_holder() {
    let (root, world_id, irene) = vn_table("vn-bible-secret");
    generate(
        &root,
        &world_id,
        "zh-TW",
        None,
        &mut fake(&[&bible_reply("trust", "主線")]),
    )
    .await
    .unwrap();
    let reply = "## PRIVATE\n她其實是龍。\n## SECRETS\n```json\n[{\"id\": \"s_dragon\", \"title\": \"龍\", \"text\": \"她是龍\"}]\n```";
    reroll_secrets(&root, &world_id, &irene, "zh-TW", &mut fake(&[reply]))
        .await
        .unwrap();
    let bible = vn::read_vn(&root, &world_id).unwrap().bible.unwrap();
    let ids: Vec<&str> = bible
        .secrets
        .iter()
        .map(|secret| secret.id.as_str())
        .collect();
    assert_eq!(ids, ["s_world", "s_dragon"]);
    assert_eq!(bible.secrets[1].holder.as_deref(), Some(irene.as_str()));
    assert_eq!(
        data::read_character(&root, &world_id, &irene)
            .unwrap()
            .private_md,
        "她其實是龍。"
    );
    assert_eq!(leaf(&root, &world_id, "revealed.s_irene"), None);
    assert_eq!(
        leaf(&root, &world_id, "revealed.s_dragon").as_deref(),
        Some("false")
    );
    let rules = data::read_state(&root, &world_id).unwrap().mechanism.rules;
    assert!(rules.contains_key("revealed.s_dragon") && !rules.contains_key("revealed.s_irene"));
}

/// 凍結以幕為鍵：同一幕改聖經，GM system 不變；換幕才重抄。聖經只進 GM 線，角色線沒有它的 token
#[tokio::test]
async fn frozen_copy_changes_only_on_new_scene_and_stays_out_of_character_lane() {
    let (root, world_id, _) = vn_table("vn-bible-frozen");
    generate(
        &root,
        &world_id,
        "zh-TW",
        None,
        &mut fake(&[&bible_reply("trust", "LOGLINE_TOKEN")]),
    )
    .await
    .unwrap();
    let system = |root: &Path| {
        let materials = crate::chat_assembly::gm_materials(root, &world_id).unwrap();
        let scan = crate::chat_assembly::test_gm_scan(root, &world_id, &materials, "zh-TW");
        let (scope, _) = crate::chat_assembly::gm_scope(&materials);
        let instruction = ChatMessage {
            role: "user".to_owned(),
            content: String::new(),
        };
        crate::chat_assembly::gm_messages(&materials, &scan, &scope, instruction, "zh-TW")[0]
            .content
            .clone()
    };
    let before = system(&root);
    assert!(
        before.contains("LOGLINE_TOKEN") && before.contains("BEATS_TOKEN"),
        "{before}"
    );
    let mut edited = vn::read_vn(&root, &world_id).unwrap().bible.unwrap();
    edited.logline = "改過的主線".to_owned();
    save(&root, &world_id, edited).unwrap();
    assert_eq!(system(&root), before);
    data::update_state(&root, &world_id, |state| {
        state.current_scene = 1;
        Ok(Some(()))
    })
    .unwrap();
    assert!(system(&root).contains("改過的主線"));

    let cards = crate::chat_assembly::active_cards(&root, &world_id).unwrap();
    let state = data::read_state(&root, &world_id).unwrap();
    let scan =
        crate::chat_assembly::test_character_scan(&root, &world_id, &cards[0], None, &[], "zh-TW");
    let (frozen, turn) = crate::chat_assembly::character_lane_parts(
        &cards[0], &cards, None, &scan, &state, None, "zh-TW", true,
    );
    let lane = format!("{frozen}{}{:?}", turn.tail, turn.hoisted_private);
    for token in ["主線", "BEATS_TOKEN", "城下有龍", "flags.", "revealed."] {
        assert!(!lane.contains(token), "{token}: {lane}");
    }
}

/// 格邊提示：計數取變動標記、其他取樹上新值；揭露只列設成 true 的
#[test]
fn flag_hints_read_marks_values_and_reveals() {
    let bible = vn::normalize(
        vn::parse_bible(&serde_json::json!({
            "flags": [
                {"id": "trust", "label": "好感", "type": "counter"},
                {"id": "route", "type": "route", "values": ["a", "b"]}
            ],
            "secrets": [{"id": "s1", "title": "公主"}, {"id": "s2", "title": "龍"}]
        }))
        .unwrap(),
        &[],
    );
    let mut state = TableState::default();
    for (path, value) in [
        ("flags.route", "b"),
        ("revealed.s1", "true"),
        ("revealed.s2", "false"),
    ] {
        let segments: Vec<String> = path.split('.').map(str::to_owned).collect();
        data::set_tree_value(&mut state.tree, &segments, value);
        state.changes.insert(path.to_owned(), "更新".to_owned());
    }
    state
        .changes
        .insert("flags.trust".to_owned(), "+1".to_owned());
    assert_eq!(
        flag_hints(&bible, &state),
        [
            FlagHint::Flag {
                label: "route".to_owned(),
                mark: None,
                value: Some("b".to_owned())
            },
            FlagHint::Flag {
                label: "好感".to_owned(),
                mark: Some("+1".to_owned()),
                value: None
            },
            FlagHint::Reveal {
                title: "公主".to_owned()
            },
        ]
    );
}

/// 目前幕落一則 GM 旁白（快照蓋目前狀態）
fn gm_line(root: &Path, world_id: &str, text: &str) {
    let scene = data::read_state(root, world_id).unwrap().current_scene;
    let event = data::TranscriptEvent {
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
        raw: None,
        ts: "now".to_owned(),
        speaker_id: String::new(),
        speaker_name: "GM".to_owned(),
        kind: data::TranscriptKind::Narration,
        text: text.to_owned(),
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
    };
    data::append_event(root, world_id, scene, &event, None).unwrap();
}

fn set_leaf(root: &Path, world_id: &str, key: &str, value: &str) {
    let mut state = data::read_state(root, world_id).unwrap();
    data::set_tree_value(&mut state.state.tree, &[key.to_owned()], value);
    data::write_state(root, world_id, &state).unwrap();
}

/// 章結束換幕（包 5 §2.3）：加章與幕號同次提交、摘要事件快照帶新章號、退幕還原；到上限改設收尾不加章；
/// 不是章結束只換幕；完結後任何換幕都拒
#[tokio::test]
async fn chapter_end_advances_chapter_then_finale_then_refuses_when_ended() {
    let (root, world_id, _) = vn_table("vn-chapter-end");
    generate(
        &root,
        &world_id,
        "zh-TW",
        None,
        &mut fake(&[&bible_reply("trust", "主線")]),
    )
    .await
    .unwrap();
    data::state_commit::with_commit(&root, &world_id, |tx| {
        vn::edit_vn(tx, |file| file.bible.as_mut().unwrap().chapter_max = 3)
    })
    .unwrap();
    assert_eq!(leaf(&root, &world_id, "chapter").as_deref(), Some("1"));
    assert_eq!(
        leaf(&root, &world_id, "ended").as_deref(),
        Some(vn::NOT_ENDED)
    );
    let mut state = data::read_state(&root, &world_id).unwrap();
    state
        .state
        .changes
        .insert("flags.trust".to_owned(), "+1".to_owned());
    data::write_state(&root, &world_id, &state).unwrap();
    gm_line(&root, &world_id, "第一章。\n[chapter]");
    data::begin_next_scene_as(&root, &world_id, "摘要", None, true).unwrap();
    assert_eq!(leaf(&root, &world_id, "chapter").as_deref(), Some("2"));
    let summary = &data::read_transcript(&root, &world_id, 1).unwrap()[0];
    assert_eq!(vn::chapter_in(&summary.state.as_ref().unwrap().tree), 2);
    // 上一回合的變動標記不跟到摘要事件（格邊旗標提示不重播）
    assert!(summary.state.as_ref().unwrap().changes.is_empty());
    data::revert_scene(&root, &world_id).unwrap();
    assert_eq!(leaf(&root, &world_id, "chapter").as_deref(), Some("1"));

    set_leaf(&root, &world_id, "chapter", "3");
    data::begin_next_scene_as(&root, &world_id, "摘要", None, false).unwrap();
    assert_eq!(leaf(&root, &world_id, "chapter").as_deref(), Some("3"));
    assert_eq!(leaf(&root, &world_id, "finale").as_deref(), Some("false"));
    data::begin_next_scene_as(&root, &world_id, "摘要", None, true).unwrap();
    assert_eq!(leaf(&root, &world_id, "chapter").as_deref(), Some("3"));
    assert_eq!(leaf(&root, &world_id, "finale").as_deref(), Some("true"));

    set_leaf(&root, &world_id, "ended", "default");
    let scene = data::read_state(&root, &world_id).unwrap().current_scene;
    for chapter_end in [true, false] {
        let error = data::begin_next_scene_as(&root, &world_id, "摘要", None, chapter_end)
            .unwrap_err()
            .to_string();
        assert!(error.contains("vn_ended"), "{error}");
    }
    assert_eq!(
        data::read_state(&root, &world_id).unwrap().current_scene,
        scene
    );
    assert!(vn::refuse_if_ended(&root, &world_id)
        .unwrap_err()
        .contains("vn_ended"));
}

/// 結局一覽：任一幕事件快照出現過的 `ended` 算達成（含已離開的分支），未達成不帶標題
#[tokio::test]
async fn endings_overview_scans_every_scene() {
    let (root, world_id, _) = vn_table("vn-endings-overview");
    generate(
        &root,
        &world_id,
        "zh-TW",
        None,
        &mut fake(&[&bible_reply("trust", "主線")]),
    )
    .await
    .unwrap();
    data::state_commit::with_commit(&root, &world_id, |tx| {
        vn::edit_vn(tx, |file| {
            file.bible.as_mut().unwrap().endings[0].title = "保底".to_owned()
        })
    })
    .unwrap();
    let overview = endings_overview(&root, &world_id).unwrap();
    assert!(overview
        .endings
        .iter()
        .all(|ending| !ending.reached && ending.title.is_empty()));
    set_leaf(&root, &world_id, "ended", "default");
    gm_line(&root, &world_id, "完。\n[ending: default]");
    // 分岔出新幕離開結局那條分支：一覽照樣算達成，目前分支沒有完結
    data::begin_next_scene_as(&root, &world_id, "摘要", None, false).unwrap_err();
    set_leaf(&root, &world_id, "ended", vn::NOT_ENDED);
    data::begin_next_scene_as(&root, &world_id, "摘要", None, false).unwrap();
    let overview = endings_overview(&root, &world_id).unwrap();
    let default = overview
        .endings
        .iter()
        .find(|ending| ending.id == vn::DEFAULT_ENDING)
        .unwrap();
    assert!(default.reached);
    assert_eq!(default.title, "保底");
    assert_eq!(overview.current, None);
}

/// 換幕摘要照抄的終止標記不算（包 5 實測：摘要模型把角色線原文裡的 `[ending: …]` 抄進摘要）
#[test]
fn summary_events_never_yield_terminal_cells() {
    let (root, world_id, _) = vn_table("vn-summary-terminals");
    gm_line(&root, &world_id, "開場。");
    data::begin_next_scene_as(
        &root,
        &world_id,
        "前情。\n[ending: default]\n[chapter]",
        None,
        false,
    )
    .unwrap();
    let (events, _) = crate::chat_turn::read_scene(&root, &world_id, 1).unwrap();
    assert_eq!(events.len(), 1);
    assert!(events[0].recap);
    assert!(events[0]
        .cells
        .iter()
        .all(|cell| matches!(cell, crate::chat_turn::VnCell::Narration { .. })));
}

#[test]
fn chapter_end_publish_failure_leaves_scene_and_chapter_then_retries_once() {
    let (root, world_id, _) = vn_table("vn-chapter-publish-fail");
    gm_line(&root, &world_id, "開場。");
    data::fail_publish::arm();
    assert!(data::begin_next_scene_as(&root, &world_id, "前情。", None, true).is_err());
    let state = data::read_state(&root, &world_id).unwrap();
    assert_eq!(state.current_scene, 0);
    assert_eq!(data::vn::chapter_in(&state.state.tree), 1);
    assert_eq!(
        data::begin_next_scene_as(&root, &world_id, "前情。", None, true).unwrap(),
        1
    );
    let state = data::read_state(&root, &world_id).unwrap();
    assert_eq!(data::vn::chapter_in(&state.state.tree), 2);
    assert_eq!(data::read_transcript(&root, &world_id, 1).unwrap().len(), 1);
}
