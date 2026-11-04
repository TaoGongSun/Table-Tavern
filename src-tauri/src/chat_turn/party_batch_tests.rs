//! 多人房間的行動批次（multiplayer-room 包 3）：落檔順序與去重、觀望與在場標記、GM／角色／換幕摘要提示、點名真人玩家、
//! 一般桌提示不變。
use super::*;
use crate::{chat_assembly, transport};

fn card(name: &str) -> data::CharacterCard {
    data::CharacterCard {
        id: data::new_id(),
        name: name.to_owned(),
        color: "#336699".to_owned(),
        avatar: "🎲".to_owned(),
        tier: data::Tier::Balanced,
        show_image: true,
        archived: false,
        gen_prompt: String::new(),
        public_md: format!("{name}的公開設定。"),
        private_md: String::new(),
        stage: false,
    }
}

/// 一桌五張卡：阿濤、小雨、阿明、守衛（之後由玩家認領）與 NPC 老闆。回傳 (root, world_id, 卡 id 依上述順序)。
fn table(label: &str) -> (std::path::PathBuf, String, [String; 5]) {
    let root = crate::smart_free::test_root(label);
    let world_id = data::create_world(&root, "多人桌").unwrap();
    let ids = ["阿濤", "小雨", "阿明", "守衛", "老闆"].map(|name| {
        let card = card(name);
        data::write_character(&root, &world_id, &card).unwrap();
        card.id
    });
    (root, world_id, ids)
}

fn act(card_id: &str, text: &str) -> PartyAction {
    PartyAction::Act {
        card_id: card_id.to_owned(),
        text: text.to_owned(),
    }
}

fn batch(dispatch_id: &str, players: &[&String], actions: Vec<PartyAction>) -> PartyBatch {
    PartyBatch {
        dispatch_id: dispatch_id.to_owned(),
        actions,
        players: players.iter().map(|id| (*id).clone()).collect(),
        arrivals: Vec::new(),
        departures: Vec::new(),
        returns: Vec::new(),
    }
}

fn land(root: &std::path::Path, world_id: &str, batch: &PartyBatch) -> BatchOutcome {
    append_party_batch(root, root, world_id, batch).unwrap()
}

fn transcript(root: &std::path::Path, world_id: &str) -> Vec<data::TranscriptEvent> {
    data::read_transcript(root, world_id, 0).unwrap()
}

/// 第一回合：阿濤行動、小雨觀望、守衛行動；同一個 dispatch_id 再送一次整批跳過
fn first_round(root: &std::path::Path, world_id: &str, ids: &[String; 5]) -> PartyBatch {
    let [tao, yu, _, guard, _] = ids;
    let first = batch(
        "d1",
        &[tao, yu, guard],
        vec![
            act(tao, "我拔劍。"),
            PartyAction::Watch {
                card_id: yu.clone(),
            },
            act(guard, "我守著門。"),
        ],
    );
    assert_eq!(land(root, world_id, &first), BatchOutcome::Appended(3));
    first
}

/// 依序落成玩家句與觀望標記、全帶 action_id；在場玩家換上、主玩家＝第一張；重送同一批什麼都不寫
#[test]
fn batch_lands_in_order_and_same_dispatch_is_skipped() {
    let (root, world_id, ids) = table("party-order");
    let first = first_round(&root, &world_id, &ids);
    let events = transcript(&root, &world_id);
    let shape: Vec<_> = events
        .iter()
        .map(|event| {
            (
                event.kind.clone(),
                event.speaker_name.as_str(),
                event.text.as_str(),
            )
        })
        .collect();
    assert_eq!(
        shape,
        [
            (data::TranscriptKind::Player, "阿濤", "我拔劍。"),
            (data::TranscriptKind::System, "GM", ""),
            (data::TranscriptKind::Player, "守衛", "我守著門。"),
        ]
    );
    assert_eq!(
        events[1].marker,
        Some(data::EventMarker::PlayerWatch {
            name: "小雨".to_owned()
        })
    );
    assert!(events
        .iter()
        .all(|event| event.action_id.as_deref() == Some("d1") && event.state.is_some()));
    let state = data::read_state(&root, &world_id).unwrap();
    assert_eq!(
        state.player_card_ids,
        [&ids[0], &ids[1], &ids[3]].map(String::clone)
    );
    assert_eq!(state.player_card_id.as_ref(), Some(&ids[0]));
    assert_eq!(land(&root, &world_id, &first), BatchOutcome::Duplicate);
    assert_eq!(transcript(&root, &world_id), events);
    let _ = std::fs::remove_dir_all(&root);
}

/// 離席（標離席／改當 NPC）與中途登場：標記依序在行動前，名單照派工換；標離席的不點名、改當 NPC 的回到點名名單
#[test]
fn arrivals_and_departures_change_presence() {
    let (root, world_id, ids) = table("party-presence");
    let [tao, yu, ming, guard, boss] = &ids;
    first_round(&root, &world_id, &ids);
    let mut second = batch(
        "d2",
        &[tao, ming],
        vec![act(tao, "我追上去。"), act(ming, "我來了！")],
    );
    second.arrivals = vec![ming.clone()];
    second.departures = vec![
        (yu.clone(), DepartureMode::Absent),
        (guard.clone(), DepartureMode::Npc),
    ];
    assert_eq!(land(&root, &world_id, &second), BatchOutcome::Appended(5));
    let markers: Vec<_> = transcript(&root, &world_id)[3..]
        .iter()
        .map(|event| event.marker.clone())
        .collect();
    let name = |name: &str| name.to_owned();
    assert_eq!(
        markers,
        [
            Some(data::EventMarker::PlayerAbsent {
                name: name("小雨")
            }),
            Some(data::EventMarker::PlayerToNpc {
                name: name("守衛")
            }),
            Some(data::EventMarker::PlayerArrival {
                name: name("阿明")
            }),
            None,
            None,
        ]
    );
    let state = data::read_state(&root, &world_id).unwrap();
    assert_eq!(state.player_card_ids, [tao.clone(), ming.clone()]);
    assert_eq!(state.absent_card_ids, [yu.clone()]);
    let materials = chat_assembly::gm_materials(&root, &world_id).unwrap();
    let call = |raw: &str| chat_assembly::gm_next_speaker(&materials, Some(raw.to_owned()));
    assert_eq!(
        call("守衛"),
        (Some(guard.clone()), None),
        "改當 NPC 的可點名"
    );
    assert_eq!(call("老闆"), (Some(boss.clone()), None));
    assert_eq!(call("小雨"), (None, None), "標離席的不點名");
    // 小雨回座：離席名單移出
    let third = batch("d3", &[tao, ming, yu], Vec::new());
    land(&root, &world_id, &third);
    assert!(data::read_state(&root, &world_id)
        .unwrap()
        .absent_card_ids
        .is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

/// GM 點名：點到玩家名回哨兵＋那張卡（前端照舊停下、房間知道是誰），只寫哨兵不指定是誰，NPC 照舊回卡 id
#[test]
fn gm_call_on_a_real_player_names_the_card() {
    let (root, world_id, ids) = table("party-next");
    first_round(&root, &world_id, &ids);
    let materials = chat_assembly::gm_materials(&root, &world_id).unwrap();
    let call = |raw: &str| chat_assembly::gm_next_speaker(&materials, Some(raw.to_owned()));
    let sentinel = Some(transport::PLAYER_SENTINEL.to_owned());
    assert_eq!(call("小雨"), (sentinel.clone(), Some(ids[1].clone())));
    assert_eq!(
        call("阿濤"),
        (sentinel.clone(), Some(ids[0].clone())),
        "主玩家也帶卡 id"
    );
    assert_eq!(call(transport::PLAYER_SENTINEL), (sentinel.clone(), None));
    assert_eq!(call("玩家"), (sentinel, None));
    assert_eq!(call("老闆"), (Some(ids[4].clone()), None));
    assert_eq!(
        chat_assembly::gm_next_speaker(&materials, None),
        (None, None)
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// GM 提示看得到多人段（在場玩家依序）與本回合依序的行動和觀望；NPC 接話看得到每位玩家的名字；換幕摘要逐一列玩家
#[test]
fn prompts_carry_the_party_round() {
    let (root, world_id, ids) = table("party-prompt");
    first_round(&root, &world_id, &ids);
    let materials = chat_assembly::gm_materials(&root, &world_id).unwrap();
    let scan = chat_assembly::test_gm_scan(&root, &world_id, &materials, "zh-TW");
    let (instruction, _) =
        chat_assembly::gm_instruction(&root, &world_id, &materials, &scan, "zh-TW");
    let content = &instruction.content;
    assert!(
        content.contains("最適合發言的角色：阿明、老闆、阿濤、小雨、守衛。"),
        "{content}"
    );
    assert!(
        content.contains("（多人桌）本桌有 3 位真人玩家，各自扮演自己的角色：阿濤、小雨、守衛。")
    );
    assert!(content.contains("「下一位」就寫他的角色名"));
    assert!(!content.contains("輪到玩家（"), "多人桌不再單點主玩家");
    let (scope, _) = chat_assembly::gm_scope(&materials);
    let gm = chat_assembly::gm_messages(&materials, &scan, &scope, instruction.clone(), "zh-TW");
    let history: String = gm.iter().map(|message| message.content.as_str()).collect();
    let order = [
        "阿濤：我拔劍。",
        "（系統）〈小雨〉這回合觀望",
        "守衛：我守著門。",
    ];
    let positions: Vec<usize> = order
        .iter()
        .map(|line| history.find(line).unwrap())
        .collect();
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "{history}"
    );
    let en = transport::party_instruction("en", &["A".to_owned(), "B".to_owned()], true);
    assert!(en.starts_with(" (Multiplayer table) This table has 2 real players"));
    assert!(en.contains("`Next: __PLAYER__`"));
    // NPC 老闆接話：玩家句帶各自的名字
    let boss = data::read_character(&root, &world_id, &ids[4]).unwrap();
    let events = transcript(&root, &world_id);
    let character_scan = chat_assembly::test_character_scan(
        &root,
        &world_id,
        &boss,
        materials.player.as_ref(),
        &events,
        "zh-TW",
    );
    let shared = transport::assemble_shared_messages(
        &boss,
        &materials.cards,
        materials.player.as_ref(),
        &events,
        &character_scan,
        &materials.state.state,
        &materials.state.mechanism,
        None,
        "zh-TW",
        &materials.prompt_regex,
    );
    let seen: String = shared
        .iter()
        .map(|message| message.content.as_str())
        .collect();
    assert!(
        seen.contains("阿濤：我拔劍。") && seen.contains("守衛：我守著門。"),
        "{seen}"
    );
    // 換幕摘要：多位玩家逐一交代
    let summary = &transport::summary_messages(&events, "zh-TW", false)[0].content;
    assert!(
        summary.contains("本場景有多位真人玩家的角色：阿濤、守衛；每一位都當成在場人物分別交代")
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// 一般桌（單一玩家卡、沒走房間批次）：導演指示與直接組的單人版逐字相同、點名照舊、換幕摘要不加多人句
#[test]
fn single_player_table_prompt_is_unchanged() {
    let (root, world_id, ids) = table("party-solo");
    data::set_player_card(&root, &world_id, Some(ids[0].clone())).unwrap();
    let materials = chat_assembly::gm_materials(&root, &world_id).unwrap();
    let scan = chat_assembly::test_gm_scan(&root, &world_id, &materials, "zh-TW");
    let (instruction, _) =
        chat_assembly::gm_instruction(&root, &world_id, &materials, &scan, "zh-TW");
    let roster = chat_assembly::speaker_roster(&materials.cards);
    let expected = transport::narrate_instruction("zh-TW", &roster, Some("阿濤"), false, false);
    assert_eq!(instruction, expected);
    let raw = || Some("阿濤".to_owned());
    assert_eq!(
        chat_assembly::gm_next_speaker(&materials, raw()),
        (
            chat_assembly::next_speaker(false, &materials.cards, raw(), Some("阿濤")),
            None
        )
    );
    // 單人玩家中途改名（同一個 action 底下只有一句）不算多人
    let mut events: Vec<data::TranscriptEvent> = ["阿濤", "濤哥"]
        .iter()
        .enumerate()
        .map(|(index, name)| data::TranscriptEvent {
            action_id: Some(format!("a{index}")),
            ..first_event(name)
        })
        .collect();
    let solo = transport::summary_messages(&events, "zh-TW", false);
    assert!(!solo[0].content.contains("真人玩家"));
    events.push(data::TranscriptEvent {
        action_id: Some("a1".to_owned()),
        ..first_event("小雨")
    });
    assert!(transport::summary_messages(&events, "zh-TW", false)[0]
        .content
        .contains("真人玩家的角色：阿濤、濤哥、小雨"));
    let _ = std::fs::remove_dir_all(&root);
}

fn first_event(name: &str) -> data::TranscriptEvent {
    data::TranscriptEvent {
        ts: String::new(),
        speaker_id: String::new(),
        speaker_name: name.to_owned(),
        kind: data::TranscriptKind::Player,
        text: "你好。".to_owned(),
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
    }
}

/// 驗證不過（卡不在桌上、行動者不在場、文字過長或空白）與換幕容量鎖擋下：回錯、逐字稿與在場名單都不動
#[test]
fn rejected_batch_writes_nothing() {
    let (root, world_id, ids) = table("party-reject");
    let [tao, yu, ..] = &ids;
    let ghost = "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned();
    let cases = [
        batch("x1", &[tao, &ghost], Vec::new()),
        batch("x2", &[tao], vec![act(yu, "我不在場。")]),
        batch("x3", &[tao], vec![act(tao, &"長".repeat(2001))]),
        batch("x4", &[tao], vec![act(tao, "   ")]),
        batch("x6", &[tao, tao], Vec::new()),
    ];
    for case in &cases {
        assert!(append_party_batch(&root, &root, &world_id, case).is_err());
    }
    assert!(transcript(&root, &world_id).is_empty());
    assert!(data::read_state(&root, &world_id)
        .unwrap()
        .player_card_ids
        .is_empty());
    // agy 長幕：容量鎖擋下整批
    let mut config = data::AppConfig::default();
    config
        .preferences
        .insert("transport".into(), serde_json::json!("agy"));
    std::fs::write(
        root.join("config.json"),
        serde_json::to_string(&config).unwrap(),
    )
    .unwrap();
    let long = data::TranscriptEvent {
        kind: data::TranscriptKind::Narration,
        text: "雨".repeat(60_000),
        ..first_event("GM")
    };
    data::append_transcript(&root, &world_id, 0, &long).unwrap();
    let blocked = append_party_batch(
        &root,
        &root,
        &world_id,
        &batch("x5", &[tao], vec![act(tao, "再一句。")]),
    );
    assert!(blocked.unwrap_err().contains("scene_capacity_full"));
    assert_eq!(transcript(&root, &world_id).len(), 1);
    assert!(data::read_state(&root, &world_id)
        .unwrap()
        .player_card_ids
        .is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

/// 新標頭代碼的落檔 JSON 形狀（持久契約）與十語系標頭
#[test]
fn party_markers_round_trip_and_localize() {
    let watch = data::EventMarker::PlayerWatch {
        name: "X".to_owned(),
    };
    let json = r#"{"type":"player_watch","name":"X"}"#;
    assert_eq!(serde_json::to_string(&watch).unwrap(), json);
    assert_eq!(
        serde_json::from_str::<data::EventMarker>(json).unwrap(),
        watch
    );
    assert_eq!(
        data::marker_heading(&watch, "en").unwrap(),
        "“X” sits this round out"
    );
    let absent = data::EventMarker::PlayerAbsent {
        name: "X".to_owned(),
    };
    assert_eq!(
        serde_json::to_string(&absent).unwrap(),
        r#"{"type":"player_absent","name":"X"}"#
    );
    assert_eq!(
        data::marker_heading(&absent, "zh-TW").unwrap(),
        "（玩家離席）〈X〉暫不登場"
    );
}

/// 進房前的單人玩家卡：房間期間不點名；退出（clear_party）後 state 與 GM 提示和進房前全等。
/// 上一批在場、這批不在也沒處置的預設標離席（協定 §3.4）；行動文字去頭尾空白才落檔
#[test]
fn leaving_the_room_restores_the_solo_table() {
    let (root, world_id, ids) = table("party-restore");
    let [tao, yu, ming, guard, _] = &ids;
    data::set_player_card(&root, &world_id, Some(tao.clone())).unwrap();
    let prompt = || {
        let materials = chat_assembly::gm_materials(&root, &world_id).unwrap();
        let scan = chat_assembly::test_gm_scan(&root, &world_id, &materials, "zh-TW");
        let (instruction, _) =
            chat_assembly::gm_instruction(&root, &world_id, &materials, &scan, "zh-TW");
        let (scope, _) = chat_assembly::gm_scope(&materials);
        chat_assembly::gm_messages(&materials, &scan, &scope, instruction, "zh-TW")
    };
    let before = (data::read_state(&root, &world_id).unwrap(), prompt());
    data::set_party(&root, &world_id, &[yu.clone(), guard.clone()], &[], &[]).unwrap();
    let materials = chat_assembly::gm_materials(&root, &world_id).unwrap();
    let call = |raw: &str| chat_assembly::gm_next_speaker(&materials, Some(raw.to_owned()));
    assert_eq!(call("阿濤"), (None, None), "進房前的玩家卡沒人認領也不點名");
    data::set_party(&root, &world_id, &[], &[], &[]).unwrap();
    let state = data::read_state(&root, &world_id).unwrap();
    assert_eq!(state.absent_card_ids, [yu.clone(), guard.clone()]);
    assert_eq!(state.solo_player_card_id.as_ref(), Some(tao));
    data::clear_party(&root, &world_id).unwrap();
    assert_eq!(
        (data::read_state(&root, &world_id).unwrap(), prompt()),
        before
    );
    data::clear_party(&root, &world_id).unwrap();
    assert_eq!(
        data::read_state(&root, &world_id).unwrap(),
        before.0,
        "不在房間裡不動"
    );
    // 去頭尾空白
    let spaced = batch("t1", &[ming], vec![act(ming, "  我來了。\n")]);
    land(&root, &world_id, &spaced);
    assert_eq!(transcript(&root, &world_id)[0].text, "我來了。");
    let _ = std::fs::remove_dir_all(&root);
}

/// 伺服器正常會送的兩種組合：離席後又選回同卡（同卡在 departures 與 players）、中途加入又在結算前離開
/// （arrivals 有、players 沒有）——略過那筆標記、其餘照落，名單以 players 為準
#[test]
fn stale_arrival_and_departure_are_skipped() {
    let (root, world_id, ids) = table("party-stale");
    let [tao, yu, ..] = &ids;
    let back = PartyBatch {
        departures: vec![(tao.clone(), DepartureMode::Absent)],
        ..batch("x8", &[tao], vec![act(tao, "我回來了。")])
    };
    assert_eq!(land(&root, &world_id, &back), BatchOutcome::Appended(1));
    let gone = PartyBatch {
        arrivals: vec![yu.clone()],
        ..batch("x7", &[tao], Vec::new())
    };
    assert_eq!(land(&root, &world_id, &gone), BatchOutcome::Appended(0));
    let events = transcript(&root, &world_id);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, data::TranscriptKind::Player);
    let state = data::read_state(&root, &world_id).unwrap();
    assert_eq!(state.player_card_ids, [tao.clone()]);
    assert!(state.absent_card_ids.is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

/// AI 代管（room-seat-delegate）：暫離寫「暫離」、卡進 NPC 池可點名；接回時在 returns 裡的寫「回來」而不是「登場」
#[test]
fn delegate_and_return_markers() {
    let (root, world_id, ids) = table("party-delegate");
    let [tao, yu, ..] = &ids;
    first_round(&root, &world_id, &ids);
    let away = PartyBatch {
        departures: vec![(yu.clone(), DepartureMode::Delegate)],
        ..batch("e1", &[tao], vec![act(tao, "我先走。")])
    };
    assert_eq!(land(&root, &world_id, &away), BatchOutcome::Appended(2));
    let markers = |from: usize| -> Vec<_> {
        transcript(&root, &world_id)[from..]
            .iter()
            .map(|event| event.marker.clone())
            .collect()
    };
    let name = |name: &str| name.to_owned();
    assert_eq!(
        markers(3),
        [
            Some(data::EventMarker::PlayerDelegated {
                name: name("小雨")
            }),
            None,
        ]
    );
    let state = data::read_state(&root, &world_id).unwrap();
    assert_eq!(state.player_card_ids, [tao.clone()]);
    assert!(!state.absent_card_ids.contains(yu), "代管不是離席");
    let materials = chat_assembly::gm_materials(&root, &world_id).unwrap();
    let call = |raw: &str| chat_assembly::gm_next_speaker(&materials, Some(raw.to_owned()));
    assert_eq!(
        call("小雨"),
        (Some(yu.clone()), None),
        "代管中由 GM 當 NPC 演"
    );
    let back = PartyBatch {
        arrivals: vec![yu.clone()],
        returns: vec![yu.clone()],
        ..batch("e2", &[tao, yu], Vec::new())
    };
    land(&root, &world_id, &back);
    assert_eq!(
        markers(5),
        [Some(data::EventMarker::PlayerReturned {
            name: name("小雨")
        })]
    );
    assert_eq!(
        data::read_state(&root, &world_id).unwrap().player_card_ids,
        [tao.clone(), yu.clone()]
    );
    let _ = std::fs::remove_dir_all(&root);
}
