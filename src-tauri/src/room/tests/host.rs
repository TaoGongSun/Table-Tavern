//! 房主 GM 鈕的三種模式（room-host-actions 包 1）：落檔對照單人版——旁白不寫點名列、點到的 NPC 回報 `called`；
//! 請發言直接接一句，`call` 時先寫點名列。
use super::*;

fn host_dispatch(id: &str, tao: &str, host: Value) -> Value {
    let mut d = dispatch(id, tao, json!([]));
    d["kind"] = json!("host");
    d["host"] = host;
    d
}

#[tokio::test]
async fn host_gm_modes_land_like_the_solo_buttons() {
    let (root, world, tao, boss) = table("host-gm");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let fake = Fake::new(
        &root,
        vec![
            Act::Say("老闆抬起頭。", Some(boss.clone())),
            Act::Line("要喝什麼？"),
            Act::Line("今天有燉肉。"),
        ],
    );
    let mut first = saved(&server, &world);
    first.seeded = true;
    let running = start(fake.clone(), first, FAST, false).await;
    let mut conn = accept(&listener).await;
    conn.welcome(false, Value::Null, Value::Null).await;
    let shape = |from: usize| -> Vec<(data::TranscriptKind, Option<data::EventMarker>, String)> {
        transcript(&root, &world)
            .split_off(from)
            .into_iter()
            .map(|e| (e.kind, e.marker, e.text))
            .collect()
    };
    use data::TranscriptKind::*;

    // 旁白：只落正文，點到老闆不接話、不寫點名列，gen.done 帶 called
    conn.send(
        "dispatch",
        host_dispatch("h1", &tao, json!({ "mode": "narrate" })),
    )
    .await;
    assert_eq!(conn.part(0, "gm").await["text"], "老闆抬起頭。");
    let done = conn.expect("gen.done").await["p"].clone();
    assert_eq!(
        (done["dispatchId"].as_str(), done["called"].as_str()),
        (Some("h1"), Some(boss.as_str()))
    );
    assert_eq!(shape(0), vec![(Narration, None, "老闆抬起頭。".to_owned())]);

    // 讓老闆接話：先寫點名列再接一句就停（不再回 GM）
    conn.send(
        "dispatch",
        host_dispatch(
            "h2",
            &tao,
            json!({ "mode": "call", "cardId": boss, "call": true }),
        ),
    )
    .await;
    assert_eq!(conn.part(0, "npc").await["text"], "要喝什麼？");
    let done = conn.expect("gen.done").await["p"].clone();
    assert!(done["called"].is_null());
    let call = data::EventMarker::GmCall {
        name: "老闆".to_owned(),
    };
    assert_eq!(
        shape(1),
        vec![
            (System, Some(call), String::new()),
            (Dialogue, None, "要喝什麼？".to_owned())
        ]
    );

    // 請老闆發言：不寫點名列
    conn.send(
        "dispatch",
        host_dispatch("h3", &tao, json!({ "mode": "call", "cardId": boss })),
    )
    .await;
    assert_eq!(conn.part(0, "npc").await["text"], "今天有燉肉。");
    conn.expect("gen.done").await;
    assert_eq!(shape(3), vec![(Dialogue, None, "今天有燉肉。".to_owned())]);
    assert!(transcript(&root, &world)[1..]
        .iter()
        .all(|e| e.action_id.is_some()));
    running.stop().await;
}
