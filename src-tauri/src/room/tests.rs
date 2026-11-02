//! 房間模式執行端（multiplayer-room 包 2）：測試內起本機假伺服器（tokio＋WebSocket），模型呼叫用替身引擎，
//! 桌上落檔用真的資料層。驗：建房→連線→welcome→派工→gen.part.*→gen.done、同派工重送不重落、
//! 斷線重連三種 InflightReport、換幕工作 ack／progress／result、job.cancel 照幕號回、遠端控制先回結果再動作。
use super::*;
use futures_util::{SinkExt, StreamExt};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

const CODE: &str = "ABCD2345";

/// 替身引擎的一步
enum Act {
    /// 旁白（正文、點名的卡 id）
    Say(&'static str, Option<String>),
    /// 角色接話
    Line(&'static str),
    Fail(&'static str),
    /// 先吐「半句」，等放行（回整段）或中止（回半截）
    Wait,
    /// 換幕：等放行才提交摘要；中止就回失敗
    Advance,
}

struct Fake {
    root: PathBuf,
    script: Mutex<VecDeque<Act>>,
    release: AtomicBool,
    aborted: AtomicBool,
    settings: Mutex<Option<(String, String)>>,
}

impl Fake {
    fn new(root: &Path, script: Vec<Act>) -> Arc<Fake> {
        Arc::new(Fake {
            root: root.to_owned(),
            script: Mutex::new(script.into()),
            release: AtomicBool::new(false),
            aborted: AtomicBool::new(false),
            settings: Mutex::new(None),
        })
    }

    fn next(&self) -> Act {
        lock(&self.script)
            .pop_front()
            .expect("替身劇本已用完：不該再呼叫模型")
    }

    /// 等放行或中止；回 true＝被中止
    async fn wait(&self) -> bool {
        loop {
            if self.aborted.swap(false, Ordering::SeqCst) {
                return true;
            }
            if self.release.swap(false, Ordering::SeqCst) {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
}

fn narration(text: &str, next: Option<String>, aborted: bool) -> Narration {
    Narration {
        text: text.to_owned(),
        raw: None,
        next,
        next_player: None,
        state_updates: String::new(),
        arrived: Vec::new(),
        aborted,
    }
}

impl Engine for Fake {
    fn roots(&self) -> (PathBuf, PathBuf) {
        (self.root.clone(), self.root.clone())
    }

    fn narrate(
        &self,
        world: String,
        turn: String,
        _: String,
        delta: Delta,
    ) -> BoxFut<'_, Result<Narration, String>> {
        Box::pin(async move {
            // 真的 gm_narrate 會開回合紀錄、結束時留在可落檔的狀態；替身照做，正文才能帶回合鍵落檔
            data::state_commit::with_commit(&self.root, &world, |tx| {
                data::message_vars::begin_turn(tx, &turn, None)?;
                data::message_vars::finish_turn(tx, &turn, None)
            })
            .unwrap();
            match self.next() {
                Act::Say(text, next) => {
                    let half = text.char_indices().nth(2).map_or(text.len(), |(i, _)| i);
                    delta(&text[..half]);
                    delta(&text[half..]);
                    // 剝殼前的原文（狀態區塊等）只留在桌上，不得上送
                    let mut n = narration(text, next, false);
                    n.raw = Some(format!("{text}<state>RAW原文</state>"));
                    Ok(n)
                }
                Act::Fail(error) => Err(error.to_owned()),
                Act::Wait => {
                    delta("半句");
                    match self.wait().await {
                        true => Ok(narration("半句", None, true)),
                        false => Ok(narration("半句之後的整段。", None, false)),
                    }
                }
                _ => panic!("劇本這一步不是旁白"),
            }
        })
    }

    fn reply(
        &self,
        _: String,
        _: String,
        _: String,
        _: String,
        delta: Delta,
    ) -> BoxFut<'_, Result<Spoken, String>> {
        Box::pin(async move {
            let Act::Line(text) = self.next() else {
                panic!("劇本這一步不是角色接話")
            };
            delta(text);
            Ok(Spoken {
                text: text.to_owned(),
                raw: None,
                aborted: false,
            })
        })
    }

    fn advance(&self, world: String, _: String) -> BoxFut<'_, Result<u64, String>> {
        Box::pin(async move {
            let Act::Advance = self.next() else {
                panic!("劇本這一步不是換幕")
            };
            if self.wait().await {
                return Err(crate::ui_msg::UiMsg::SceneSummaryStopped.to_string());
            }
            data::begin_next_scene(&self.root, &world, "前情：眾人走進酒館。", None)
                .map_err(|e| e.to_string())
        })
    }

    fn abort(&self, _: &str, _: &str) {
        self.aborted.store(true, Ordering::SeqCst);
    }

    fn use_settings(&self, settings: Option<(String, String)>) {
        *lock(&self.settings) = settings;
    }

    fn capabilities(&self) -> Value {
        json!({
            "routes": [{ "route": "api", "label": "API", "models": [{ "id": "m1", "label": "m1", "laxity": null }] }],
            "current": { "route": "api", "model": "m1", "modelLabel": "m1" },
            "maxRoundSpeakers": 3,
        })
    }

    fn table_changed(&self, _: &str) {}
}

/// 一桌兩張卡：阿濤（玩家）、老闆（NPC）
fn table(label: &str) -> (PathBuf, String, String, String) {
    let root = crate::smart_free::test_root(&format!("room-{label}"));
    let world = data::create_world(&root, "房間桌").unwrap();
    let ids = ["阿濤", "老闆"].map(|name| {
        let mut card = crate::commands::character_card(&data::new_id(), name);
        card.public_md = format!("{name}的公開設定。");
        card.private_md = format!("{name}的祕密。");
        data::write_character(&root, &world, &card).unwrap();
        card.id
    });
    let [tao, boss] = ids;
    (root, world, tao, boss)
}

fn saved(server: &str, world: &str) -> Saved {
    Saved {
        active: true,
        server: server.to_owned(),
        world_id: world.to_owned(),
        room_code: CODE.to_owned(),
        executor_token: "t".repeat(43),
        invite_url: format!("{server}/r/{CODE}#k=secret"),
        ..Saved::default()
    }
}

const FAST: Timing = Timing {
    unit: Duration::from_millis(5),
};

fn dispatch(id: &str, tao: &str, actions: Value) -> Value {
    json!({
        "dispatchId": id, "round": 1, "kind": "play", "scene": 1,
        "settings": { "route": "api", "model": "m1", "modelLabel": "M1" },
        "actions": actions,
        "players": [{ "cardId": tao, "memberId": "m1", "name": "阿濤", "connected": true }],
        "arrivals": [], "departures": [],
    })
}

fn act(tao: &str, text: &str) -> Value {
    json!([{ "memberId": "m1", "cardId": tao, "name": "阿濤", "kind": "act", "text": text }])
}

fn transcript(root: &Path, world: &str) -> Vec<data::TranscriptEvent> {
    let scene = data::read_state(root, world).unwrap().current_scene;
    data::read_transcript(root, world, scene).unwrap()
}

/// 假伺服器的一條執行端連線：`auto_ok` 時對帶 id 的訊框回 ok（配對碼另帶結果）
struct Conn {
    ws: WebSocketStream<TcpStream>,
    auto_ok: bool,
}

async fn accept(listener: &TcpListener) -> Conn {
    let (stream, _) = tokio::time::timeout(Duration::from_secs(10), listener.accept())
        .await
        .expect("等不到執行端連線")
        .unwrap();
    Conn {
        ws: tokio_tungstenite::accept_async(stream).await.unwrap(),
        auto_ok: true,
    }
}

impl Conn {
    async fn send(&mut self, t: &str, p: Value) {
        self.ws
            .send(Message::text(json!({ "t": t, "p": p }).to_string()))
            .await
            .unwrap();
    }

    /// 下一則（略過心跳與增量），整體 10 秒內要等到
    async fn recv(&mut self) -> Value {
        let until = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let message = tokio::time::timeout_at(until, self.ws.next())
                .await
                .expect("等不到訊框");
            let Some(Ok(Message::Text(text))) = message else {
                panic!("連線斷了：{message:?}")
            };
            let frame: Value = serde_json::from_str(text.as_str()).unwrap();
            if let (true, Some(id)) = (self.auto_ok, frame["id"].as_str()) {
                if id != "hello" {
                    let result = if id == "pair" {
                        json!({ "pairCode": "WXYZ2345", "expiresAt": 0 })
                    } else {
                        Value::Null
                    };
                    self.send("ok", json!({ "id": id, "result": result })).await;
                }
            }
            if frame["t"] != "hb" && frame["t"] != "gen.delta" {
                return frame;
            }
        }
    }

    async fn expect(&mut self, t: &str) -> Value {
        let start = std::time::Instant::now();
        loop {
            let frame = self.recv().await;
            if frame["t"] == t {
                return frame;
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "等不到 {t}，最後收到 {frame}"
            );
        }
    }

    /// 這段時間內沒有心跳以外的訊框
    async fn quiet(&mut self, ms: u64) {
        let until = tokio::time::Instant::now() + Duration::from_millis(ms);
        while let Ok(Some(Ok(Message::Text(text)))) =
            tokio::time::timeout_at(until, self.ws.next()).await
        {
            let frame: Value = serde_json::from_str(text.as_str()).unwrap();
            assert_eq!(frame["t"], "hb", "不該再有訊框：{frame}");
        }
    }

    /// 收 hello、回 welcome（大廳或遊戲中），回 hello 內容
    async fn welcome(&mut self, lobby: bool, dispatch: Value, job: Value) -> Value {
        let hello = self.expect("hello").await;
        let status = if lobby { "lobby" } else { "playing" };
        let snapshot = json!({ "room": { "status": status, "payerId": if lobby { Value::Null } else { json!("m1") }, "hostId": null },
            "members": [{ "memberId": "m1", "nickname": "阿濤的玩家", "connected": true }], "cards": [] });
        self.send(
            "welcome",
            json!({ "snapshot": snapshot, "dispatch": dispatch, "job": job }),
        )
        .await;
        hello["p"].clone()
    }

    /// 一段 begin→end，回 end 的內容
    async fn part(&mut self, n: u64, speaker: &str) -> Value {
        let begin = self.expect("gen.part.begin").await;
        assert_eq!(
            (
                begin["p"]["part"].as_u64(),
                begin["p"]["speaker"]["kind"].as_str()
            ),
            (Some(n), Some(speaker))
        );
        let end = self.expect("gen.part.end").await;
        assert_eq!(end["p"]["part"].as_u64(), Some(n));
        end["p"].clone()
    }
}

/// 建房 HTTP：拿到四樣東西；伺服器回錯時帶狀態碼與錯誤碼
#[tokio::test]
async fn create_room_posts_and_keeps_credentials() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let serve = tokio::spawn(async move {
        for reply in [
            format!(
                r#"{{"roomCode":"{CODE}","joinSecret":"s","executorToken":"{}","inviteUrl":"x/r/{CODE}#k=s"}}"#,
                "t".repeat(43)
            ),
            r#"{"code":"E_RATE_LIMITED"}"#.to_owned(),
        ] {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = vec![0u8; 4096];
            let n = stream.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..n]).to_string();
            assert!(request.starts_with("POST /api/rooms "), "{request}");
            let status = if reply.contains("executorToken") {
                "201 Created"
            } else {
                "429 Too Many Requests"
            };
            let response = format!("HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}", reply.len());
            stream.write_all(response.as_bytes()).await.unwrap();
        }
    });
    let made = link::create_room(&server).await.unwrap();
    assert_eq!(
        (
            made.room_code.as_str(),
            made.executor_token.len(),
            made.join_secret.as_str()
        ),
        (CODE, 43, "s")
    );
    assert!(!made.active && made.queue.is_empty());
    let refused = link::create_room(&server).await.unwrap_err();
    assert!(
        refused.contains("429") && refused.contains("E_RATE_LIMITED"),
        "{refused}"
    );
    serve.await.unwrap();
}

/// 建房後第一次連線：hello 帶憑證 → welcome → capabilities／狀態／公開名冊（不帶私設、GM 專屬事件、原文）／
/// grantHost 配對碼；派工 → 玩家句落桌 → 旁白點名老闆 → 老闆接話 → 再一段旁白沒點名 → gen.done（段落不帶原文）。
/// 之後同一派工重送不重跑
#[tokio::test]
async fn dispatch_runs_gm_chain_and_resent_dispatch_is_not_landed_twice() {
    let (root, world, tao, boss) = table("flow");
    for (text, gm_only, raw) in [
        ("GM 筆記：兇手是老闆。", true, None),
        ("雨下著。", false, Some("雨下著。<state>RAW舊原文</state>")),
    ] {
        let event: data::TranscriptEvent = serde_json::from_value(json!({
            "ts": "", "speaker_id": "", "speaker_name": "GM", "kind": "narration", "text": text, "gm_only": gm_only, "raw": raw,
        }))
        .unwrap();
        data::append_event(&root, &world, 0, &event, None).unwrap();
    }
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let fake = Fake::new(
        &root,
        vec![
            Act::Say("店裡很安靜。", Some(boss.clone())),
            Act::Line("歡迎光臨。"),
            Act::Say("老闆擦著杯子。", None),
        ],
    );
    let running = start(fake.clone(), saved(&server, &world), FAST, false).await;
    let mut conn = accept(&listener).await;
    let hello = conn.welcome(true, Value::Null, Value::Null).await;
    assert_eq!(
        (
            hello["role"].as_str(),
            hello["executorToken"].as_str().map(str::len)
        ),
        (Some("executor"), Some(43))
    );
    assert!(hello["inflight"]["dispatch"].is_null());
    assert_eq!(
        conn.expect("capabilities").await["p"]["current"]["model"],
        "m1"
    );
    assert_eq!(conn.expect("status").await["p"]["keepAwake"], Value::Null);
    let seed = conn.expect("room.seed").await["p"].clone();
    assert_eq!(
        (seed["part"].as_u64(), seed["total"].as_u64()),
        (Some(0), Some(1))
    );
    let cards = seed["cards"].as_array().unwrap();
    assert_eq!(cards.len(), 2);
    let seeded = seed.to_string();
    assert!(
        !seeded.contains("祕密") && !seeded.contains("兇手") && !seeded.contains("RAW"),
        "私設、GM 專屬、原文不得上送：{seed}"
    );
    assert!(seeded.contains("雨下著。"), "公開旁白照送");
    assert_eq!(conn.expect("pair.create").await["p"]["grantHost"], true);

    conn.send("dispatch", dispatch("d1", &tao, act(&tao, "我推門進去。")))
        .await;
    conn.expect("dispatch.ack").await;
    let first = conn.part(0, "gm").await;
    assert_eq!(first["text"], "店裡很安靜。");
    assert!(!first.to_string().contains("RAW"), "段落只帶落檔正文");
    let reply = conn.part(1, "npc").await;
    assert_eq!(reply["text"], "歡迎光臨。");
    assert_eq!(conn.part(2, "gm").await["text"], "老闆擦著杯子。");
    let done = conn.expect("gen.done").await["p"].clone();
    assert_eq!(
        (done["dispatchId"].as_str(), done["aborted"].as_bool()),
        (Some("d1"), Some(false))
    );
    assert_eq!(
        *lock(&fake.settings),
        Some(("api".to_owned(), "m1".to_owned()))
    );

    let events = transcript(&root, &world).split_off(2);
    let shape: Vec<(data::TranscriptKind, String)> = events
        .iter()
        .map(|e| (e.kind.clone(), e.text.clone()))
        .collect();
    use data::TranscriptKind::*;
    assert_eq!(
        shape,
        vec![
            (Player, "我推門進去。".to_owned()),
            (Narration, "店裡很安靜。".to_owned()),
            (System, String::new()),
            (Dialogue, "歡迎光臨。".to_owned()),
            (Narration, "老闆擦著杯子。".to_owned()),
        ]
    );
    assert!(events.iter().all(|e| e.action_id.as_deref() == Some("d1")));
    assert_eq!(
        events[2].marker,
        Some(data::EventMarker::GmCall {
            name: "老闆".to_owned()
        })
    );
    assert_eq!(events[3].speaker_id, boss);
    let status = running.room.panel();
    assert_eq!(status.pair_code.as_deref(), Some("WXYZ-2345"));
    assert!(status.payer_link.unwrap().ends_with("#k=secret&p=WXYZ2345"));

    conn.expect("status").await; // 鏈跑完回報不忙
                                 // 同一筆再送（已做完）：不回 ack、什麼都不重跑
    conn.send("dispatch", dispatch("d1", &tao, act(&tao, "我推門進去。")))
        .await;
    conn.quiet(200).await;
    running.stop().await;
    assert!(
        read_saved(&root).queue.is_empty(),
        "全部收到 ok 後佇列是空的"
    );
}

/// 斷線重連的三種 InflightReport（協定 §6.5）：同一筆 running → 不重落、接著跑完；同一筆 done → 只重送佇列；
/// 不同一筆 → 當新的跑
#[tokio::test]
async fn reconnect_follows_inflight_report() {
    let (root, world, tao, _) = table("reconnect");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let fake = Fake::new(
        &root,
        vec![
            Act::Wait,
            Act::Say("第二回合。", None),
            Act::Say("第三回合。", None),
        ],
    );
    let mut first = saved(&server, &world);
    first.seeded = true;
    let running = start(fake.clone(), first, FAST, false).await;

    // running：生成到一半斷線
    let mut conn = accept(&listener).await;
    conn.welcome(false, Value::Null, Value::Null).await;
    conn.send("dispatch", dispatch("d2", &tao, act(&tao, "我等著。")))
        .await;
    conn.expect("gen.part.begin").await;
    drop(conn);
    let mut conn = accept(&listener).await;
    let hello = conn
        .welcome(
            false,
            dispatch("d2", &tao, act(&tao, "我等著。")),
            Value::Null,
        )
        .await;
    assert_eq!(
        hello["inflight"]["dispatch"],
        json!({ "dispatchId": "d2", "state": "running", "lastEndedPart": null })
    );
    conn.expect("dispatch.ack").await;
    fake.release.store(true, Ordering::SeqCst);
    assert_eq!(
        conn.expect("gen.part.end").await["p"]["text"],
        "半句之後的整段。"
    );
    conn.expect("gen.done").await;
    let players = transcript(&root, &world)
        .iter()
        .filter(|e| e.kind == data::TranscriptKind::Player)
        .count();
    assert_eq!(players, 1, "重連後不重落玩家句");

    // done：gen.done 沒收到 ok 就斷 → 重連報 done，只重送佇列裡那則
    conn.send("dispatch", dispatch("d3", &tao, act(&tao, "第二句。")))
        .await;
    conn.auto_ok = false;
    conn.expect("gen.done").await;
    drop(conn);
    // 斷線期間記下的連線錯誤：重連成功就清掉
    running.room.note_error(&format!(
        "{}: Connection refused",
        super::link::CONNECT_FAILED
    ));
    // 伺服器還沒收到 gen.done，welcome 照樣帶著 d3：只重送佇列、不回 ack（ack 晚到會撞上已開的下一回合）
    let mut conn = accept(&listener).await;
    let hello = conn
        .welcome(
            false,
            dispatch("d3", &tao, act(&tao, "第二句。")),
            Value::Null,
        )
        .await;
    assert_eq!(hello["inflight"]["dispatch"]["state"], "done");
    let resent = conn.expect("gen.done").await;
    assert_eq!(resent["p"]["dispatchId"], "d3");
    conn.expect("status").await;
    conn.quiet(300).await;
    assert_eq!(
        running.room.panel().last_error,
        None,
        "重連後不留過時的連線錯誤"
    );
    tokio::time::sleep(Duration::from_millis(100)).await; // 讓重送那幾則的 ok 先被收下

    // 不同一筆：當新的跑；生成類錯誤重連後照樣留著
    running.room.note_error("AI_CONTENT_FILTERED：拒答");
    drop(conn);
    let mut conn = accept(&listener).await;
    conn.welcome(
        false,
        dispatch("d4", &tao, act(&tao, "第三句。")),
        Value::Null,
    )
    .await;
    assert_eq!(conn.part(0, "gm").await["text"], "第三回合。");
    assert_eq!(conn.expect("gen.done").await["p"]["dispatchId"], "d4");
    assert_eq!(
        running.room.panel().last_error.as_deref(),
        Some("AI_CONTENT_FILTERED：拒答")
    );
    running.stop().await;
}

/// 遠端控制：先回 ctl.result 再動手。中止 → 半截落檔標截斷、gen.part.end{interrupted}＋gen.done{aborted}；
/// 失敗 → gen.failed（錯誤分類）→ retry 先回結果、從同一段續跑
#[tokio::test]
async fn remote_control_replies_first_then_acts() {
    let (root, world, tao, _) = table("ctl");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let fake = Fake::new(
        &root,
        vec![
            Act::Wait,
            Act::Fail("AI_HTTP_STATUS_429: slow down"),
            Act::Say("重試成功。", None),
        ],
    );
    let mut first = saved(&server, &world);
    first.seeded = true;
    let running = start(fake.clone(), first, FAST, false).await;
    let mut conn = accept(&listener).await;
    conn.welcome(false, Value::Null, Value::Null).await;

    conn.send("dispatch", dispatch("d5", &tao, act(&tao, "我拔劍。")))
        .await;
    conn.expect("gen.part.begin").await;
    conn.send(
        "ctl",
        json!({ "ctlId": "c1", "by": "m1", "cmd": { "kind": "abort" } }),
    )
    .await;
    let after = conn.recv().await;
    assert_eq!(
        (after["t"].as_str(), after["p"]["ok"].as_bool()),
        (Some("ctl.result"), Some(true)),
        "先回結果：{after}"
    );
    let end = conn.expect("gen.part.end").await["p"].clone();
    assert_eq!(
        (end["text"].as_str(), end["interrupted"].as_bool()),
        (Some("半句"), Some(true))
    );
    assert_eq!(conn.expect("gen.done").await["p"]["aborted"], true);
    let last = transcript(&root, &world).pop().unwrap();
    assert!(last.truncated && last.text == "半句");

    conn.send("dispatch", dispatch("d6", &tao, act(&tao, "我再拔一次。")))
        .await;
    let failed = conn.expect("gen.failed").await["p"].clone();
    assert_eq!(
        (failed["atPart"].as_u64(), failed["error"]["kind"].as_str()),
        (Some(0), Some("quota"))
    );
    assert_eq!(failed["error"]["code"], "AI_HTTP_STATUS_429");
    assert!(
        !failed.to_string().contains("slow down"),
        "錯誤回應本文不上送"
    );
    conn.expect("status").await; // 失敗後的狀態回報
    conn.send(
        "ctl",
        json!({ "ctlId": "c2", "by": "m1", "cmd": { "kind": "switch_model", "model": "m9" } }),
    )
    .await;
    let after = conn.recv().await;
    assert_eq!(
        (after["t"].as_str(), after["p"]["ok"].as_bool()),
        (Some("ctl.result"), Some(true)),
        "先回結果：{after}"
    );
    assert_eq!(conn.part(0, "gm").await["text"], "重試成功。");
    assert_eq!(conn.expect("gen.done").await["p"]["dispatchId"], "d6");
    assert_eq!(
        *lock(&fake.settings),
        Some(("api".to_owned(), "m9".to_owned())),
        "幕中切模型只進覆寫槽"
    );
    // 沒東西可續時 retry 照實回失敗
    conn.send(
        "ctl",
        json!({ "ctlId": "c3", "by": "m1", "cmd": { "kind": "retry" } }),
    )
    .await;
    assert_eq!(conn.expect("ctl.result").await["p"]["ok"], false);
    running.stop().await;
}

/// 換幕工作：job.ack → 摘要期間 job.progress → job.result{ok, summary}（下一幕設定隨之後的派工帶來）。
/// job.cancel 而本機沒在跑：幕號已推進回 ok（帶已落摘要）、沒推進或不認得回失敗，一定回
#[tokio::test]
async fn scene_job_reports_progress_and_cancel_answers_by_scene() {
    let (root, world, _, _) = table("scene");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let fake = Fake::new(&root, vec![Act::Advance, Act::Advance]);
    let mut first = saved(&server, &world);
    first.seeded = true;
    let running = start(fake.clone(), first, FAST, false).await;
    let mut conn = accept(&listener).await;
    conn.welcome(false, Value::Null, Value::Null).await;
    let next = json!({ "route": "api", "model": "m2", "modelLabel": "M2" });
    conn.send(
        "job.scene_advance",
        json!({ "jobId": "j1", "kind": "scene_advance", "scene": 1, "nextSettings": next }),
    )
    .await;
    conn.expect("job.ack").await;
    conn.expect("job.progress").await;
    fake.release.store(true, Ordering::SeqCst);
    let result = conn.expect("job.result").await["p"].clone();
    assert_eq!(
        (result["jobId"].as_str(), result["ok"].as_bool()),
        (Some("j1"), Some(true))
    );
    assert_eq!(result["result"]["summary"], "前情：眾人走進酒館。");

    // 已推進的那筆：照幕號回 ok；不認得的：回失敗
    conn.send("job.cancel", json!({ "jobId": "j1" })).await;
    assert_eq!(conn.expect("job.result").await["p"]["ok"], true);
    conn.send("job.cancel", json!({ "jobId": "jx" })).await;
    assert_eq!(conn.expect("job.result").await["p"]["ok"], false);

    // 跑到一半被取消：中止那次摘要、照實回失敗
    conn.send(
        "job.scene_advance",
        json!({ "jobId": "j2", "kind": "scene_advance", "scene": 2, "nextSettings": next }),
    )
    .await;
    conn.expect("job.ack").await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    conn.send("job.cancel", json!({ "jobId": "j2" })).await;
    let result = conn.expect("job.result").await["p"].clone();
    assert_eq!(
        (result["jobId"].as_str(), result["ok"].as_bool()),
        (Some("j2"), Some(false))
    );
    running.stop().await;

    // 程序重啟後本機仍記 running、但桌上沒推進：job.cancel 回失敗（不得不回）
    let mut restarted = saved(&server, &world);
    restarted.seeded = true;
    let scene = data::read_state(&root, &world).unwrap().current_scene;
    restarted.job = Some(JobRun {
        id: "j3".to_owned(),
        state: RunState::Running,
        scene_before: scene,
    });
    let again = start(Fake::new(&root, Vec::new()), restarted, FAST, false).await;
    let mut conn = accept(&listener).await;
    let hello = conn.welcome(false, Value::Null, Value::Null).await;
    assert_eq!(
        hello["inflight"]["job"],
        json!({ "jobId": "j3", "state": "running" })
    );
    conn.send("job.cancel", json!({ "jobId": "j3" })).await;
    assert_eq!(conn.expect("job.result").await["p"]["ok"], false);
    again.stop().await;
}

/// 錯誤分類（協定 §7.1 表左欄）、幕設定覆寫只蓋 GM 檔位、capabilities 照設定產生
#[test]
fn errors_overrides_and_capabilities() {
    let kind = |error: &str| run::classify(error)["kind"].as_str().unwrap().to_owned();
    assert_eq!(kind("AI_CONTENT_FILTERED: blocked"), "refusal");
    assert_eq!(kind("AI_FREE_MODEL_BUSY: x"), "quota");
    assert_eq!(kind("AI_STREAM_STALLED: x"), "provider_unreachable");
    assert_eq!(kind("AI_CONTEXT_TOO_LONG"), "scene_full");
    assert_eq!(
        kind(&crate::ui_msg::UiMsg::SceneCapacityFull.to_string()),
        "scene_full"
    );
    assert_eq!(
        kind(&crate::ui_msg::UiMsg::CliRiskNotAccepted.to_string()),
        "not_ready"
    );
    assert_eq!(kind("AI_INCOMPLETE_RESPONSE: cut"), "tool_format");
    assert_eq!(kind("讀不到檔案 /home/someone/x"), "other");
    assert_eq!(
        run::classify("讀不到檔案 /home/someone/x")["code"],
        "OTHER",
        "路徑不進代碼"
    );

    let mut config = data::AppConfig::default();
    config
        .preferences
        .insert("gm_tier".to_owned(), json!("balanced"));
    config
        .tier_models
        .insert("best".to_owned(), "keep".to_owned());
    data::apply_play_override(&mut config, "api", "vendor/m2");
    assert_eq!(
        (
            config.tier_models["balanced"].as_str(),
            config.tier_models["best"].as_str()
        ),
        ("vendor/m2", "keep")
    );
    data::apply_play_override(&mut config, "claude", "claude-x");
    assert_eq!(config.preferences["transport"], "claude");
    assert_eq!(config.tier_models["claude:balanced"], "claude-x");

    config
        .api_keys
        .insert("openrouter".to_owned(), "k".to_owned());
    let caps = seed::capabilities(&config);
    let routes: Vec<&str> = caps["routes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["route"].as_str().unwrap())
        .collect();
    assert_eq!(routes, ["claude", "api"]);
    assert_eq!(caps["current"]["model"], "claude-x");
    assert_eq!(caps["maxRoundSpeakers"], 3);

    // 矽基流動：目前路線顯示 siliconflow、模型列整份免費清單；OpenRouter 路線照自己的檔位模型
    config
        .api_keys
        .insert("siliconflow".to_owned(), "sk-sf".to_owned());
    data::apply_play_override(&mut config, "siliconflow", "default");
    let caps = seed::capabilities(&config);
    let route = |id: &str| {
        caps["routes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["route"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(caps["current"]["route"], "siliconflow");
    assert_eq!(
        caps["current"]["model"],
        crate::transport::siliconflow::FREE_MODELS[0]
    );
    assert_eq!(
        route("siliconflow")["models"].as_array().unwrap().len(),
        crate::transport::siliconflow::FREE_MODELS.len()
    );
    assert_eq!(route("api")["models"][1]["id"], "vendor/m2");
}

/// 明文 http 只放行本機與私網，其餘必須 https（憑證不走明文）；本機不走代理
#[test]
fn plain_http_only_for_local_servers() {
    for ok in [
        "https://rooms.example.test",
        "http://127.0.0.1:8787",
        "http://localhost:8787/",
        "http://192.168.1.20:8787",
    ] {
        assert!(server_allowed(ok), "{ok}");
    }
    for bad in [
        "http://rooms.example.test",
        "http://8.8.8.8",
        "ftp://127.0.0.1",
        "rooms.example.test",
    ] {
        assert!(!server_allowed(bad), "{bad}");
    }
    assert!(
        local_server("http://127.0.0.1:1")
            && !local_server("http://192.168.1.20")
            && !local_server("https://rooms.example.test")
    );
}

/// 房主關房（4410）：連線迴圈結束前照退出收尾——桌子回到單人、覆寫槽清掉、憑證清掉、不再算房間模式
#[tokio::test]
async fn closed_room_restores_solo_table() {
    let (root, world, tao, boss) = table("closed");
    data::set_player_card(&root, &world, Some(boss.clone())).unwrap();
    data::set_party(&root, &world, std::slice::from_ref(&tao), &[], &[]).unwrap();
    assert_eq!(
        data::read_state(&root, &world).unwrap().player_card_ids,
        vec![tao.clone()]
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let fake = Fake::new(&root, Vec::new());
    let mut first = saved(&server, &world);
    first.seeded = true;
    first.settings = Some(("api".to_owned(), "m1".to_owned()));
    let running = start(fake.clone(), first, FAST, false).await;
    let mut conn = accept(&listener).await;
    conn.welcome(false, Value::Null, Value::Null).await;
    conn.expect("capabilities").await;
    let close = tokio_tungstenite::tungstenite::protocol::CloseFrame {
        code: 4410.into(),
        reason: "".into(),
    };
    conn.ws.close(Some(close)).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        while !running.task.is_finished() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("4410 後連線迴圈要結束、不再重連");
    let state = data::read_state(&root, &world).unwrap();
    assert!(state.player_card_ids.is_empty(), "還原成單人桌");
    assert_eq!(
        state.player_card_id.as_deref(),
        Some(boss.as_str()),
        "單人時期的玩家卡回來"
    );
    assert_eq!(*lock(&fake.settings), None, "覆寫槽清掉");
    let left = read_saved(&root);
    assert!(!left.active && left.executor_token.is_empty() && left.world_id.is_empty());
    assert_eq!(left.server, server, "伺服器網址留著");
}

/// 系統代理：有 http 代理時先送 CONNECT 打通，再在上面做 WebSocket 握手
#[tokio::test]
async fn websocket_goes_through_http_proxy_connect() {
    let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_addr = target.local_addr().unwrap();
    let proxy = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_url = format!("http://{}", proxy.local_addr().unwrap());
    let relay = tokio::spawn(async move {
        let (mut client, _) = proxy.accept().await.unwrap();
        let mut head = Vec::new();
        while !head.ends_with(b"\r\n\r\n") {
            let mut byte = [0u8; 1];
            client.read_exact(&mut byte).await.unwrap();
            head.push(byte[0]);
        }
        let head = String::from_utf8(head).unwrap();
        assert!(
            head.starts_with(&format!("CONNECT {target_addr} HTTP/1.1\r\n")),
            "{head}"
        );
        client
            .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
            .await
            .unwrap();
        let mut upstream = TcpStream::connect(target_addr).await.unwrap();
        let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
    });
    let server = tokio::spawn(async move {
        let mut conn = accept(&target).await;
        conn.send("welcome", json!({})).await;
        conn
    });
    let url = format!("ws://{target_addr}/api/rooms/{CODE}/ws?role=executor");
    let mut ws = link::connect(&url, Some(proxy_url)).await.unwrap();
    let Some(Ok(Message::Text(text))) = ws.next().await else {
        panic!("經代理收不到訊框")
    };
    assert_eq!(
        serde_json::from_str::<Value>(text.as_str()).unwrap()["t"],
        "welcome"
    );
    drop(ws);
    drop(server.await.unwrap());
    relay.await.unwrap();
}

/// 房間模式中的桌：桌面的玩家句、GM 旁白、角色接話 command 拒收（回 room_mode_active）；別桌照常
#[tokio::test]
async fn desktop_commands_refuse_the_room_table() {
    let (root, world, _, _) = table("refuse");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let running = start(
        Fake::new(&root, Vec::new()),
        saved(&server, &world),
        FAST,
        false,
    )
    .await;
    *lock(&RUNNING) = Some(running);
    let refused = refuse_if_active(&world).unwrap_err();
    assert!(refused.contains("room_mode_active"), "{refused}");
    assert!(refuse_if_active("another-world").is_ok());
    let running = lock(&RUNNING).take().unwrap();
    running.stop().await;
    assert!(refuse_if_active(&world).is_ok(), "退出後照常");
}
