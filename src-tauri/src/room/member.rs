//! 桌面版自己的成員連線（room-host-desktop-play §4.2）：webview 的 CSP 不放房間伺服器，房間畫面（party 元件）的
//! WebSocket 改由這裡開，訊框原樣經 IPC 轉手。重連、心跳、請求配對全在前端的 `RoomSocket`，這裡只是一條管子：
//! 網址由執行端憑證組出（前端不能指定連去哪），文字訊框兩邊各限 1 MiB。
use super::{link, running};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message;

const MAX_TEXT: usize = 1024 * 1024;

enum Out {
    Text(String),
    Close(u16),
}

type Sink = Arc<dyn Fn(Value) + Send + Sync>;

static NEXT: AtomicU32 = AtomicU32::new(1);
static SOCKETS: Mutex<Option<HashMap<u32, mpsc::UnboundedSender<Out>>>> = Mutex::new(None);

fn sockets<R>(f: impl FnOnce(&mut HashMap<u32, mpsc::UnboundedSender<Out>>) -> R) -> R {
    let mut slot = SOCKETS.lock().unwrap_or_else(|poison| poison.into_inner());
    f(slot.get_or_insert_with(HashMap::new))
}

/// 開一條連線，回連線編號；事件經 `sink`：`{k:"open"}`、`{k:"msg",d}`、`{k:"close",code}`（之後不再有事件）
pub(crate) fn member_open(sink: Sink) -> Result<u32, String> {
    let (url, proxy) = {
        let slot = running();
        let room = &slot.as_ref().ok_or("ROOM_NOT_ACTIVE")?.room;
        let saved = room.saved();
        let proxy = crate::cli::proxy_url().filter(|_| !super::local_server(&saved.server));
        let url = format!(
            "{}/api/rooms/{}/ws?role=member",
            link::ws_base(&saved.server),
            saved.room_code
        );
        (url, proxy)
    };
    Ok(open_url(url, proxy, sink))
}

pub(super) fn open_url(url: String, proxy: Option<String>, sink: Sink) -> u32 {
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = mpsc::unbounded_channel();
    sockets(|map| map.insert(id, tx));
    tauri::async_runtime::spawn(async move {
        let code = pipe(&url, proxy, rx, &sink).await;
        sockets(|map| map.remove(&id));
        sink(json!({ "k": "close", "code": code }));
    });
    id
}

async fn pipe(
    url: &str,
    proxy: Option<String>,
    mut rx: mpsc::UnboundedReceiver<Out>,
    sink: &Sink,
) -> u16 {
    let Ok(ws) = link::connect(url, proxy).await else {
        return 1006;
    };
    sink(json!({ "k": "open" }));
    let (mut tx, mut stream) = ws.split();
    loop {
        tokio::select! {
            out = rx.recv() => match out {
                Some(Out::Text(text)) => if tx.send(Message::text(text)).await.is_err() { return 1006 },
                Some(Out::Close(code)) => return close(&mut tx, code).await,
                None => return close(&mut tx, 1000).await,
            },
            message = stream.next() => match message {
                Some(Ok(Message::Text(text))) if text.len() <= MAX_TEXT => sink(json!({ "k": "msg", "d": text.as_str() })),
                Some(Ok(Message::Text(_))) => return 1009,
                Some(Ok(Message::Close(frame))) => return frame.map_or(1005, |frame| u16::from(frame.code)),
                Some(Ok(_)) => {}
                Some(Err(_)) | None => return 1006,
            },
        }
    }
}

async fn close(tx: &mut futures_util::stream::SplitSink<link::Ws, Message>, code: u16) -> u16 {
    let frame = CloseFrame {
        code: CloseCode::from(code),
        reason: "".into(),
    };
    let _ = tx.send(Message::Close(Some(frame))).await;
    code
}

pub(crate) fn member_send(id: u32, text: String) -> Result<(), String> {
    if text.len() > MAX_TEXT {
        return Err("ROOM_FRAME_TOO_LARGE".to_owned());
    }
    sockets(|map| map.get(&id).map(|tx| tx.send(Out::Text(text))))
        .ok_or("ROOM_SOCKET_CLOSED")?
        .map_err(|_| "ROOM_SOCKET_CLOSED".to_owned())
}

/// 前端關連線：只收 1000 與 4000–4999（應用自訂碼）；已經斷了就當沒事
pub(crate) fn member_close(id: u32, code: u16) {
    let code = if code == 1000 || (4000..5000).contains(&code) {
        code
    } else {
        1000
    };
    sockets(|map| map.get(&id).map(|tx| tx.send(Out::Close(code))));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::net::TcpListener;

    /// 一條管子：開起來先報 open、雙向轉文字、前端關就送關閉碼給伺服器、伺服器關就報 close
    async fn next(rx: &mut mpsc::UnboundedReceiver<Value>) -> Value {
        tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .unwrap()
            .unwrap()
    }

    #[tokio::test]
    async fn pipes_frames_both_ways_and_reports_close() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
            let Some(Ok(Message::Text(hello))) = ws.next().await else {
                panic!("要先收到 hello")
            };
            assert_eq!(hello.as_str(), "hello");
            ws.send(Message::text("welcome")).await.unwrap();
            let Some(Ok(Message::Close(Some(frame)))) = ws.next().await else {
                panic!("要收到關閉")
            };
            u16::from(frame.code)
        });
        let (tx, mut rx) = mpsc::unbounded_channel::<Value>();
        let id = open_url(
            format!("ws://{addr}/"),
            None,
            Arc::new(move |event| {
                let _ = tx.send(event);
            }),
        );
        assert_eq!(next(&mut rx).await["k"], "open");
        member_send(id, "hello".to_owned()).unwrap();
        let event = next(&mut rx).await;
        assert_eq!(
            (event["k"].as_str(), event["d"].as_str()),
            (Some("msg"), Some("welcome"))
        );
        member_close(id, 4000);
        assert_eq!(server.await.unwrap(), 4000);
        let event = next(&mut rx).await;
        assert_eq!(
            (event["k"].as_str(), event["code"].as_u64()),
            (Some("close"), Some(4000))
        );
        assert!(
            member_send(id, "late".to_owned()).is_err(),
            "關掉之後送不出去"
        );
    }

    #[tokio::test]
    async fn unreachable_server_reports_abnormal_close() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let (tx, mut rx) = mpsc::unbounded_channel::<Value>();
        open_url(
            format!("ws://{addr}/"),
            None,
            Arc::new(move |event| drop(tx.send(event))),
        );
        let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (event["k"].as_str(), event["code"].as_u64()),
            (Some("close"), Some(1006))
        );
    }
}
