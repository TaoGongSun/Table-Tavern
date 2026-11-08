//! 重開補送（room-host-actions 包 3）：房間故事串跟桌上公開投影比最長共同前綴；純追加、單人收回過、
//! 單人換過幕（摘要只當換幕項、不重複）、對不到名冊的角色改 GM 段。
use super::*;
use data::{EventMarker, TranscriptEvent, TranscriptKind};

fn line(kind: TranscriptKind, speaker: (&str, &str), text: &str) -> TranscriptEvent {
    TranscriptEvent {
        dice_pool: None,
        ts: String::new(),
        speaker_id: speaker.0.to_owned(),
        speaker_name: speaker.1.to_owned(),
        kind,
        text: text.to_owned(),
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

fn append(root: &Path, world: &str, events: &[TranscriptEvent]) {
    let scene = data::read_state(root, world).unwrap().current_scene;
    for event in events {
        data::append_event(root, world, scene, event, None).unwrap();
    }
}

/// 房間故事串的一段（快照格式）
fn part(round: i64, part: i64, speaker: Value, text: &str) -> Value {
    json!({ "round": round, "part": part, "speaker": speaker, "text": text, "interrupted": false, "at": 0 })
}

/// 暫停過的房間重開：收 hello（要帶 paused）、回帶故事串的 welcome，收名冊與所有 story.sync 片
async fn reopen(conn: &mut Conn, story: Vec<Value>, tao: &str) -> Vec<Value> {
    let hello = conn.expect("hello").await;
    assert_eq!(hello["p"]["paused"], true);
    let snapshot = json!({ "room": { "status": "playing", "payerId": "m1", "hostId": "m1" },
        "members": [{ "memberId": "m1", "nickname": "阿濤的玩家", "cardId": tao, "connected": true }],
        "cards": [], "story": story, "round": { "round": 3, "dispatchId": "old" } });
    conn.send(
        "welcome",
        json!({ "snapshot": snapshot, "dispatch": null, "job": null }),
    )
    .await;
    conn.expect("roster.update").await;
    let first = conn.expect("story.sync").await;
    let total = first["p"]["total"].as_u64().unwrap();
    let mut parts = vec![first];
    for _ in 1..total {
        parts.push(conn.expect("story.sync").await);
    }
    parts
}

fn texts(parts: &[Value]) -> Vec<String> {
    parts
        .iter()
        .flat_map(|frame| frame["p"]["items"].as_array().cloned().unwrap_or_default())
        .map(|item| match item.get("summary") {
            Some(summary) => format!("摘要:{}", summary.as_str().unwrap()),
            None => {
                let p = &item["part"];
                let sp = &p["speaker"];
                match sp["kind"].as_str().unwrap() {
                    "system" => format!("[{}]", sp["marker"].as_str().unwrap()),
                    kind => format!("{kind}:{}", p["text"].as_str().unwrap()),
                }
            }
        })
        .collect()
}

#[tokio::test]
async fn reopen_sends_solo_play_after_the_common_prefix() {
    let (root, world, tao, boss) = table("sync-append");
    use TranscriptKind::*;
    append(
        &root,
        &world,
        &[
            line(Narration, ("", "GM"), "開場"),
            line(Player, ("", "阿濤"), "我拔劍"),
            line(Narration, ("", "GM"), "門開了"),
        ],
    );
    // 單人玩：玩家句、老闆接話、旁白，外加一個名冊沒有的路人
    append(
        &root,
        &world,
        &[
            line(Player, ("", "阿濤"), "  我搜身 "),
            line(Dialogue, (&boss, "老闆"), "要喝什麼？"),
            line(Dialogue, ("", "路人"), "讓讓"),
            line(Narration, ("", "GM"), "找到鑰匙"),
        ],
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let mut first = saved(&server, &world);
    first.seeded = true;
    first.paused = true;
    let running = start(Fake::new(&root, Vec::new()), first, FAST, false).await;
    let mut conn = accept(&listener).await;
    let player = json!({ "kind": "player", "memberId": "m1", "cardId": tao, "name": "阿濤" });
    let story = vec![
        part(1, 0, json!({ "kind": "gm" }), "開場"),
        part(2, -1, player, "我拔劍"),
        part(2, 0, json!({ "kind": "gm" }), "門開了"),
        part(
            2,
            33,
            json!({ "kind": "system", "marker": "round_undone" }),
            "",
        ),
    ];
    let parts = reopen(&mut conn, story, &tao).await;
    assert_eq!(
        parts[0]["p"]["keepThrough"],
        json!({ "round": 2, "part": 33 }),
        "全對上：整幕都留（含尾端的系統段）"
    );
    assert_eq!(
        texts(&parts),
        [
            "[solo_paused]",
            "player:  我搜身 ",
            "npc:要喝什麼？",
            "gm:路人：讓讓",
            "gm:找到鑰匙",
            "[room_reopened]"
        ]
    );
    let item = &parts[0]["p"]["items"][1]["part"]["speaker"];
    assert_eq!(
        (item["memberId"].as_str(), item["cardId"].as_str()),
        (Some("m1"), Some(tao.as_str())),
        "玩家句照座位填成員"
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    let left = running.room.saved();
    assert!(!left.paused, "最後一片 ok 才清掉暫停");
    assert_eq!(
        left.synced_scene,
        Some(data::read_state(&root, &world).unwrap().current_scene)
    );
    running.stop().await;
}

#[tokio::test]
async fn reopen_cuts_what_solo_undo_removed_and_bridges_scenes_without_repeating_summaries() {
    let (root, world, tao, _) = table("sync-scenes");
    use TranscriptKind::*;
    // 房間的最後一回合被單人收回、改寫成別的
    append(
        &root,
        &world,
        &[
            line(Narration, ("", "GM"), "開場"),
            line(Player, ("", "阿濤"), "我拔劍"),
            line(Narration, ("", "GM"), "門沒開"),
        ],
    );
    let scene = data::read_state(&root, &world).unwrap().current_scene;
    let mut state = data::read_state(&root, &world).unwrap();
    state.current_scene = scene + 1;
    data::write_state(&root, &world, &state).unwrap();
    let mut summary = line(System, ("", "GM"), "第一幕摘要");
    summary.marker = Some(EventMarker::SceneSummary);
    append(
        &root,
        &world,
        &[summary, line(Narration, ("", "GM"), "第二幕")],
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let mut first = saved(&server, &world);
    first.seeded = true;
    first.paused = true;
    first.synced_scene = Some(scene);
    let running = start(Fake::new(&root, Vec::new()), first, FAST, false).await;
    let mut conn = accept(&listener).await;
    let player = json!({ "kind": "player", "memberId": "m1", "cardId": tao, "name": "阿濤" });
    let story = vec![
        part(1, 0, json!({ "kind": "gm" }), "開場"),
        part(2, -1, player, "我拔劍"),
        part(2, 0, json!({ "kind": "gm" }), "門開了"),
        part(2, 1, json!({ "kind": "gm" }), "裡面很暗"),
    ];
    let parts = reopen(&mut conn, story, &tao).await;
    assert_eq!(
        parts[0]["p"]["keepThrough"],
        json!({ "round": 2, "part": -1 })
    );
    assert_eq!(
        texts(&parts),
        [
            "[solo_paused]",
            "gm:門沒開",
            "摘要:第一幕摘要",
            "[solo_paused]",
            "gm:第二幕",
            "[room_reopened]"
        ]
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(running.room.saved().synced_scene, Some(scene + 1));
    running.stop().await;
}

/// 關房時中止的那段還在重送佇列：伺服器會先收它，比對基準要接上（沒有 begin 的只比正文）
#[test]
fn queued_parts_join_the_room_baseline() {
    let snapshot = json!({ "story": [part(1, 0, json!({ "kind": "gm" }), "開場")], "round": { "round": 3, "dispatchId": "d" } });
    let queue = [
        json!({ "t": "gen.part.begin", "p": { "dispatchId": "d", "part": 0, "speaker": { "kind": "gm" } } }),
        json!({ "t": "gen.part.end", "p": { "dispatchId": "d", "part": 0, "text": "半句", "interrupted": true } }),
        json!({ "t": "gen.part.end", "p": { "dispatchId": "d", "part": 1, "text": "接著", "interrupted": true } }),
        json!({ "t": "gen.part.end", "p": { "dispatchId": "other", "part": 0, "text": "別回合", "interrupted": false } }),
    ];
    let lines = crate::room::sync::room_lines(&snapshot, &queue);
    let got: Vec<_> = lines
        .iter()
        .map(|l| (l.at.clone(), l.key.clone()))
        .collect();
    let gm = Some(("gm".to_owned(), String::new()));
    assert_eq!(
        got,
        vec![
            (
                json!({ "round": 1, "part": 0 }),
                Some((gm.clone(), "開場".to_owned()))
            ),
            (
                json!({ "round": 3, "part": 0 }),
                Some((gm, "半句".to_owned()))
            ),
            (
                json!({ "round": 3, "part": 1 }),
                Some((None, "接著".to_owned()))
            ),
        ]
    );
}

/// 玩過的桌開房：分隔放在開場白之後、第一個玩出來的段落前（開場白不是房主玩的）
#[test]
fn seed_marks_solo_play_after_the_opening() {
    let (root, world, _, _) = table("seed-opening");
    use TranscriptKind::*;
    let mut opening = line(Narration, ("", "GM"), "雨夜的酒館。");
    opening.opening = true;
    append(
        &root,
        &world,
        &[opening, line(Narration, ("", "GM"), "門被推開了。")],
    );
    let (frames, _) = crate::room::seed::seed_frames(&root, &world).unwrap();
    let history = frames[0]["p"]["history"].as_array().unwrap().clone();
    let shape: Vec<String> = history
        .iter()
        .map(|h| match h["speaker"]["marker"].as_str() {
            Some(marker) => format!("[{marker}]"),
            None => h["text"].as_str().unwrap().to_owned(),
        })
        .collect();
    assert_eq!(shape, ["雨夜的酒館。", "[solo_before]", "門被推開了。"]);
}
