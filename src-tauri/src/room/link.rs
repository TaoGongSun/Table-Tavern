//! 執行端連線（協定 §4.2、§6.1、§6.5、§7.4）：建房 HTTP、WebSocket、`hello`／`welcome`、心跳、退避重連、
//! 重送佇列，以及把收到的訊框交給 `run`。
use super::{run, seed, Room, Saved};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

const CLIENT: &str = "desktop";
/// 退避（協定 §7.4）：1、2、5、10、30 秒，之後每 30 秒
const BACKOFF: [u32; 5] = [1, 2, 5, 10, 30];

/// `POST /api/rooms`（協定 §6.1）：拿房間碼、加入密鑰、執行端憑證與邀請連結
pub(super) async fn create_room(server: &str) -> Result<Saved, String> {
    let body = json!({
        "protocol": 1, "label": "Table Tavern",
        "client": { "name": CLIENT, "version": env!("CARGO_PKG_VERSION") },
    });
    let response = reqwest::Client::new()
        .post(format!("{server}/api/rooms"))
        .timeout(Duration::from_secs(20))
        .json(&body)
        .send()
        .await
        .map_err(|error| format!("ROOM_CREATE_FAILED: {error}"))?;
    let status = response.status();
    let reply: Value = response.json().await.unwrap_or(Value::Null);
    let field = |key: &str| reply[key].as_str().unwrap_or_default().to_owned();
    if !status.is_success() || field("executorToken").is_empty() {
        return Err(format!(
            "ROOM_CREATE_FAILED: HTTP {} {}",
            status.as_u16(),
            field("code")
        ));
    }
    Ok(Saved {
        server: server.to_owned(),
        room_code: field("roomCode"),
        join_secret: field("joinSecret"),
        executor_token: field("executorToken"),
        invite_url: field("inviteUrl"),
        ..Saved::default()
    })
}

fn ws_url(server: &str, code: &str) -> String {
    let base = server
        .replacen("https://", "wss://", 1)
        .replacen("http://", "ws://", 1);
    format!("{base}/api/rooms/{code}/ws?role=executor")
}

/// 連 WebSocket。tokio-tungstenite 不吃系統代理（建房的 reqwest 會）：有 http 代理就先開 TCP 送
/// `CONNECT host:port`，再在上面做 TLS 與握手。socks 代理不支援，直連
pub(super) async fn connect(url: &str, proxy: Option<String>) -> Result<Ws, String> {
    let request = url
        .into_client_request()
        .map_err(|error| error.to_string())?;
    let uri = request.uri();
    let host = uri.host().unwrap_or_default().to_owned();
    let port = uri
        .port_u16()
        .unwrap_or(if uri.scheme_str() == Some("wss") {
            443
        } else {
            80
        });
    let tunnel = proxy
        .as_deref()
        .and_then(|proxy| proxy.strip_prefix("http://"));
    let stream = match tunnel {
        // 帳密（user:pass@）不支援，只取位址
        Some(address) => {
            let address = address
                .trim_end_matches('/')
                .rsplit('@')
                .next()
                .unwrap_or_default();
            through(address, &host, port).await
        }
        None => TcpStream::connect((host.as_str(), port)).await,
    }
    .map_err(|error| error.to_string())?;
    let (ws, _) = tokio_tungstenite::client_async_tls(request, stream)
        .await
        .map_err(|error| error.to_string())?;
    Ok(ws)
}

async fn through(proxy: &str, host: &str, port: u16) -> std::io::Result<TcpStream> {
    let mut stream = TcpStream::connect(proxy).await?;
    let target = format!("{host}:{port}");
    stream
        .write_all(format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n").as_bytes())
        .await?;
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") {
        let mut byte = [0u8; 1];
        if stream.read(&mut byte).await? == 0 || head.len() > 8192 {
            return Err(std::io::Error::other("proxy closed during CONNECT"));
        }
        head.push(byte[0]);
    }
    let status = String::from_utf8_lossy(&head);
    match status.split_whitespace().nth(1) == Some("200") {
        true => Ok(stream),
        false => Err(std::io::Error::other(format!(
            "proxy refused CONNECT: {}",
            status.lines().next().unwrap_or_default()
        ))),
    }
}

enum End {
    /// 斷線（可重連）；帶「這次有沒有收到 welcome」決定退避要不要歸零
    Lost(bool),
    Stop,
    /// 不再重連：房間不在了、憑證失效（清掉憑證）或被同憑證的新連線取代（保留）
    Fatal {
        message: &'static str,
        forget: bool,
    },
}

pub(super) const CONNECT_FAILED: &str = "ROOM_CONNECT_FAILED";

pub(super) async fn run(
    room: Arc<Room>,
    mut rx: mpsc::UnboundedReceiver<Value>,
    mut stop: watch::Receiver<bool>,
) {
    let mut attempt = 0usize;
    loop {
        if *stop.borrow() {
            return;
        }
        let saved = room.saved();
        let url = ws_url(&saved.server, &saved.room_code);
        let proxy = crate::cli::proxy_url().filter(|_| !super::local_server(&saved.server));
        let end = match connect(&url, proxy).await {
            Ok(ws) => session(&room, ws, &mut rx, &mut stop).await,
            Err(error) => {
                room.note_error(&format!("{CONNECT_FAILED}: {error}"));
                End::Lost(false)
            }
        };
        room.status().online = false;
        match end {
            End::Stop => return,
            // 房間關了、憑證失效或被取代：照退出收尾（還原單人桌、清覆寫槽），再停用；房間不在了才清憑證
            End::Fatal { message, forget } => {
                room.note_error(message);
                if let Err(error) = super::release_table(&room, &room.world()) {
                    room.note_error(&error);
                }
                room.update(|saved| {
                    saved.active = false;
                    if forget {
                        *saved = Saved {
                            server: saved.server.clone(),
                            ..Saved::default()
                        }
                    }
                });
                return;
            }
            End::Lost(true) => attempt = 0,
            End::Lost(false) => {}
        }
        let wait = room.timing.unit * BACKOFF[attempt.min(BACKOFF.len() - 1)];
        attempt += 1;
        // 退避期間照樣收走出站訊框：要等 ok 的已在本機佇列，重連後重送；即時的本來就可丟
        let sleep = tokio::time::sleep(wait);
        tokio::pin!(sleep);
        loop {
            tokio::select! {
                _ = &mut sleep => break,
                _ = stop.changed() => return,
                frame = rx.recv() => if frame.is_none() { return },
            }
        }
    }
}

pub(super) type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn session(
    room: &Arc<Room>,
    ws: Ws,
    rx: &mut mpsc::UnboundedReceiver<Value>,
    stop: &mut watch::Receiver<bool>,
) -> End {
    let (mut sink, mut stream) = ws.split();
    let saved = room.saved();
    let hello = json!({ "t": "hello", "id": "hello", "p": {
        "protocol": 1, "role": "executor", "roomCode": saved.room_code,
        "executorToken": saved.executor_token, "lastSeq": 0, "inflight": room.inflight(),
        "client": { "name": CLIENT, "version": env!("CARGO_PKG_VERSION") },
    }});
    if sink.send(Message::text(hello.to_string())).await.is_err() {
        return End::Lost(false);
    }
    let mut welcomed = false;
    let mut beat = tokio::time::interval(room.timing.unit * 15);
    beat.tick().await;
    loop {
        let outgoing: Vec<Value> = tokio::select! {
            _ = stop.changed() => {
                let _ = sink.send(Message::Close(None)).await;
                return End::Stop;
            }
            _ = beat.tick() => match welcomed {
                true => vec![json!({ "t": "hb", "p": {} })],
                false => Vec::new(),
            },
            frame = rx.recv() => match (frame, welcomed) {
                (None, _) => return End::Stop,
                // welcome 之前不送：要等 ok 的會在 welcome 後隨佇列重送
                (Some(frame), true) => vec![frame],
                (Some(_), false) => Vec::new(),
            },
            message = stream.next() => match message {
                Some(Ok(Message::Text(text))) => {
                    let Ok(frame) = serde_json::from_str::<Value>(text.as_str()) else { continue };
                    if frame["t"] == "welcome" {
                        welcomed = true;
                        welcome(room, &frame["p"])
                    } else {
                        on_frame(room, &frame);
                        Vec::new()
                    }
                }
                Some(Ok(Message::Close(close))) => {
                    return closed(close.map(|frame| u16::from(frame.code)), welcomed)
                }
                Some(Ok(_)) => Vec::new(),
                Some(Err(_)) | None => return End::Lost(welcomed),
            },
        };
        for frame in outgoing {
            if sink.send(Message::text(frame.to_string())).await.is_err() {
                return End::Lost(welcomed);
            }
        }
    }
}

/// 伺服器關閉碼（協定 §4.2）
fn closed(code: Option<u16>, welcomed: bool) -> End {
    match code {
        Some(4409) => End::Fatal {
            message: "E_EXECUTOR_REPLACED",
            forget: false,
        },
        Some(4401) => End::Fatal {
            message: "E_AUTH_FAILED",
            forget: true,
        },
        Some(4404) | Some(4410) => End::Fatal {
            message: "E_ROOM_NOT_FOUND",
            forget: true,
        },
        _ => End::Lost(welcomed),
    }
}

/// 連上了：重送佇列 → capabilities／狀態／額度 → 還沒送過就送公開名冊與歷史 → 沒有出額度者就發配對碼
/// → 照 `welcome` 帶的派工與工作續跑或開工（協定 §6.5、§14-14）
fn welcome(room: &Arc<Room>, p: &Value) -> Vec<Value> {
    let snapshot = &p["snapshot"];
    let saved = room.saved();
    {
        // 連上了：斷線期間的連線錯誤已過時，其他錯誤（gen.failed 等）留著
        let mut status = room.status();
        status.online = true;
        if status
            .last_error
            .as_deref()
            .is_some_and(|e| e.starts_with(CONNECT_FAILED))
        {
            status.last_error = None;
        }
    }
    let mut frames = saved.queue.clone();
    let caps = room.engine.capabilities();
    room.status().model_label = caps["current"]["modelLabel"].as_str().map(str::to_owned);
    frames.push(json!({ "t": "capabilities", "p": caps }));
    frames.push(run::status_frame(room));
    let lobby = snapshot["room"]["status"] == "lobby";
    let (_, root) = room.engine.roots();
    if !saved.seeded && lobby {
        match seed::seed_frames(&root, &saved.world_id) {
            Ok((parts, ids)) => {
                room.work().roster = ids;
                frames.extend(parts);
            }
            Err(error) => room.note_error(&format!("room.seed：{error}")),
        }
    } else {
        room.work().roster = snapshot["cards"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|card| card["cardId"].as_str().map(str::to_owned))
            .collect();
    }
    if snapshot["room"]["payerId"].is_null() && room.status().pair_code.is_none() {
        frames.push(pair_frame(snapshot["room"]["hostId"].is_null()));
    }
    if p["dispatch"].is_object() {
        run::on_dispatch(room, &p["dispatch"]);
    }
    if p["job"].is_object() {
        run::on_job(room, &p["job"]);
    }
    frames
}

fn pair_frame(grant_host: bool) -> Value {
    json!({ "t": "pair.create", "id": "pair", "p": { "grantHost": grant_host } })
}

pub(super) fn request_pair(room: &Room, grant_host: bool) {
    let _ = room.out.send(pair_frame(grant_host));
}

fn on_frame(room: &Arc<Room>, frame: &Value) {
    let p = &frame["p"];
    match frame["t"].as_str().unwrap_or_default() {
        "dispatch" => run::on_dispatch(room, p),
        "abort" => run::abort(room),
        "ctl" => run::on_ctl(room, p),
        "job.scene_advance" | "job.card_create" => run::on_job(room, p),
        "job.cancel" => run::on_cancel(room, p),
        "ok" => on_reply(room, p, None),
        "err" => on_reply(room, p, p["code"].as_str()),
        // 出額度者配對好了：配對碼用掉，面板不再顯示
        "ev" if p["e"]["type"] == "pair.bound" => {
            let mut status = room.status();
            status.pair_code = None;
            status.payer_link = None;
        }
        _ => {}
    }
}

/// `ok`／`err`：佇列裡的那則收下了（錯誤也收：協定 §8 `E_DISPATCH_UNKNOWN` 記日誌丟掉、不重送）
fn on_reply(room: &Room, p: &Value, error: Option<&str>) {
    let id = p["id"].as_str().unwrap_or_default();
    if room.saved().queue.iter().any(|frame| frame["id"] == id) {
        room.update(|saved| saved.queue.retain(|frame| frame["id"] != id));
    }
    if let Some(code) = error {
        room.note_error(&format!(
            "{code} {}",
            p["detail"].as_str().unwrap_or_default()
        ));
    }
    // seed-<片>-<總片數>：最後一片 ok 才算送過；伺服器回衝突（已送過、已開局）也不再送
    if let Some(rest) = id.strip_prefix("seed-") {
        let nums: Vec<usize> = rest.split('-').filter_map(|n| n.parse().ok()).collect();
        let last = matches!(nums[..], [part, total] if part + 1 == total);
        let restart = p["detail"] == "seed_restart";
        if (error.is_none() && last) || (error.is_some() && !restart) {
            room.update(|saved| saved.seeded = true);
        }
    }
    if id == "pair" && error.is_none() {
        let code = p["result"]["pairCode"].as_str().unwrap_or_default();
        if code.len() == 8 {
            let link = format!("{}&p={code}", room.saved().invite_url);
            let mut status = room.status();
            status.pair_code = Some(format!("{}-{}", &code[..4], &code[4..]));
            status.payer_link = Some(link);
        }
    }
}
