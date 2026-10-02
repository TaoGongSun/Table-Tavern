//! 控制埠：只綁 127.0.0.1、隨機埠、每次啟動隨機 token。手寫最小 HTTP/1.1，只收
//! Content-Length、不支援 keep-alive。每連線獨立 task，eval 等對話窗時 answer 仍進得來。

use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const MAX_HEADER: usize = 16 * 1024;
const MAX_BODY: usize = 32 * 1024 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_EVAL_TIMEOUT_MS: u64 = 30_000;
const MAX_EVAL_TIMEOUT_MS: u64 = 600_000;

#[derive(Debug, PartialEq)]
pub(super) struct Request {
    pub method: String,
    pub path: String,
    pub body: Vec<u8>,
}

#[derive(Debug, PartialEq)]
pub(super) struct Rejection {
    pub status: u16,
    pub reason: String,
}

fn reject(status: u16, reason: impl Into<String>) -> Rejection {
    Rejection {
        status,
        reason: reason.into(),
    }
}

/// 驗 header 區塊（不含 body）。回傳 (method, path, content_length)。
pub(super) fn check_head(
    head: &str,
    port: u16,
    token: &str,
) -> Result<(String, String, usize), Rejection> {
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split(' ');
    let (Some(method), Some(path), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(reject(400, "request line 格式錯誤"));
    };
    if version != "HTTP/1.1" && version != "HTTP/1.0" {
        return Err(reject(400, "只支援 HTTP/1.x"));
    }
    let mut host = None;
    let mut auth = None;
    let mut length = None;
    for line in lines.filter(|l| !l.is_empty()) {
        let Some((name, value)) = line.split_once(':') else {
            return Err(reject(400, "header 格式錯誤"));
        };
        let value = value.trim();
        let slot = match name.trim().to_ascii_lowercase().as_str() {
            "host" => &mut host,
            "authorization" => &mut auth,
            "content-length" => &mut length,
            "transfer-encoding" => return Err(reject(400, "不接受 Transfer-Encoding")),
            "origin" => return Err(reject(403, "不接受帶 Origin 的請求")),
            _ => continue,
        };
        if slot.is_some() {
            return Err(reject(400, format!("重複的 {name} header")));
        }
        *slot = Some(value.to_owned());
    }
    if host.as_deref() != Some(format!("127.0.0.1:{port}").as_str()) {
        return Err(reject(403, "Host 不符"));
    }
    let expected = format!("Bearer {token}");
    if !constant_time_eq(
        auth.as_deref().unwrap_or("").as_bytes(),
        expected.as_bytes(),
    ) {
        return Err(reject(401, "token 錯誤"));
    }
    let length = match length {
        None => 0,
        Some(v) => v
            .parse::<usize>()
            .map_err(|_| reject(400, "Content-Length 不是數字"))?,
    };
    if length > MAX_BODY {
        return Err(reject(413, "body 太大"));
    }
    Ok((method.to_owned(), path.to_owned(), length))
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

async fn read_request(
    stream: &mut TcpStream,
    port: u16,
    token: &str,
) -> Result<Request, Rejection> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos;
        }
        if buf.len() > MAX_HEADER {
            return Err(reject(431, "header 太大"));
        }
        let n = tokio::time::timeout(READ_TIMEOUT, stream.read(&mut chunk))
            .await
            .map_err(|_| reject(408, "讀取逾時"))?
            .map_err(|e| reject(400, e.to_string()))?;
        if n == 0 {
            return Err(reject(400, "連線提早關閉"));
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    if head_end > MAX_HEADER {
        return Err(reject(431, "header 太大"));
    }
    let head = std::str::from_utf8(&buf[..head_end]).map_err(|_| reject(400, "header 非 UTF-8"))?;
    let (method, path, length) = check_head(head, port, token)?;
    let mut body = buf[head_end + 4..].to_vec();
    if body.len() > length {
        return Err(reject(400, "body 長度與 Content-Length 不符"));
    }
    while body.len() < length {
        let n = tokio::time::timeout(READ_TIMEOUT, stream.read(&mut chunk))
            .await
            .map_err(|_| reject(408, "讀取逾時"))?
            .map_err(|e| reject(400, e.to_string()))?;
        if n == 0 {
            return Err(reject(400, "body 不完整"));
        }
        body.extend_from_slice(&chunk[..n]);
        if body.len() > length {
            return Err(reject(400, "body 長度與 Content-Length 不符"));
        }
    }
    Ok(Request { method, path, body })
}

async fn respond(stream: &mut TcpStream, status: u16, body: &Value) {
    let text = body.to_string();
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        408 => "Request Timeout",
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        text.len()
    );
    let _ = stream.write_all(head.as_bytes()).await;
    let _ = stream.write_all(text.as_bytes()).await;
    let _ = stream.shutdown().await;
}

pub(super) async fn serve(app: tauri::AppHandle) -> Result<(), String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let token = super::random_hex(32);
    super::write_discovery(port, &token)?;
    loop {
        let Ok((mut stream, peer)) = listener.accept().await else {
            continue;
        };
        if !peer.ip().is_loopback() {
            continue;
        }
        let app = app.clone();
        let token = token.clone();
        tauri::async_runtime::spawn(async move {
            match read_request(&mut stream, port, &token).await {
                Ok(request) => {
                    let (status, body, quit) = route(&app, request).await;
                    respond(&mut stream, status, &body).await;
                    if quit {
                        app.exit(0);
                    }
                }
                Err(rejection) => {
                    respond(
                        &mut stream,
                        rejection.status,
                        &json!({ "ok": false, "error": rejection.reason }),
                    )
                    .await;
                }
            }
        });
    }
}

fn body_json(request: &Request) -> Result<Value, String> {
    if request.body.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_slice(&request.body).map_err(|e| format!("body 不是 JSON：{e}"))
}

fn ok(value: Value) -> (u16, Value, bool) {
    (200, json!({ "ok": true, "value": value }), false)
}

fn err(status: u16, message: String) -> (u16, Value, bool) {
    (status, json!({ "ok": false, "error": message }), false)
}

async fn route(app: &tauri::AppHandle, request: Request) -> (u16, Value, bool) {
    let body = match body_json(&request) {
        Ok(body) => body,
        Err(message) => return err(400, message),
    };
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/status") => ok(super::status_json()),
        ("POST", "/eval") => {
            let Some(source) = body.get("js").and_then(Value::as_str) else {
                return err(400, "缺 js".to_owned());
            };
            let ms = body
                .get("timeoutMs")
                .and_then(Value::as_u64)
                .unwrap_or(DEFAULT_EVAL_TIMEOUT_MS)
                .min(MAX_EVAL_TIMEOUT_MS);
            match super::eval::run(app, source, Duration::from_millis(ms)).await {
                Ok(value) => ok(value),
                Err(message) => err(200, message),
            }
        }
        ("GET", "/dialogs") => ok(json!(super::dialog::list())),
        ("POST", "/answer") => {
            let id = body.get("id").and_then(Value::as_str).unwrap_or("next");
            let Some(choice) = body.get("choice").and_then(Value::as_str) else {
                return err(400, "缺 choice".to_owned());
            };
            match super::dialog::answer(id, choice) {
                Ok(info) => ok(json!(info)),
                Err(message) => err(200, message),
            }
        }
        ("GET", "/shot") => match snapshot(app).await {
            Ok(png) => ok(json!({ "pngBase64": png })),
            Err(message) => err(200, message),
        },
        ("POST", "/quit") => {
            super::dialog::cancel_all();
            (200, json!({ "ok": true, "value": "quitting" }), true)
        }
        _ => err(
            404,
            format!("沒有這個端點：{} {}", request.method, request.path),
        ),
    }
}

/// 截 webview 畫面：WKWebView 的 takeSnapshot 只拍自己的內容，不需要螢幕錄製授權。回 PNG base64。
#[cfg(target_os = "macos")]
async fn snapshot(app: &tauri::AppHandle) -> Result<String, String> {
    use base64::Engine;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use std::sync::Mutex;
    use tauri::Manager;

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "找不到 main 視窗".to_owned())?;
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<Vec<u8>, String>>();
    let tx = std::sync::Arc::new(Mutex::new(Some(tx)));
    window
        .with_webview(move |webview| {
            let tx = tx.clone();
            let handler =
                block2::RcBlock::new(move |image: *mut AnyObject, _error: *mut AnyObject| {
                    let result = if image.is_null() {
                        Err("WKWebView 截圖失敗".to_owned())
                    } else {
                        // SAFETY: 主執行緒上的 AppKit 物件；回傳值都是 autoreleased，用完即丟。
                        unsafe {
                            let tiff: *mut AnyObject = msg_send![image, TIFFRepresentation];
                            let rep: *mut AnyObject =
                                msg_send![class!(NSBitmapImageRep), imageRepWithData: tiff];
                            let props: *mut AnyObject = msg_send![class!(NSDictionary), dictionary];
                            // NSBitmapImageFileTypePNG = 4
                            let png: *mut AnyObject =
                                msg_send![rep, representationUsingType: 4usize, properties: props];
                            if png.is_null() {
                                Err("PNG 轉換失敗".to_owned())
                            } else {
                                let len: usize = msg_send![png, length];
                                let bytes: *const u8 = msg_send![png, bytes];
                                Ok(std::slice::from_raw_parts(bytes, len).to_vec())
                            }
                        }
                    };
                    if let Some(tx) = tx.lock().unwrap_or_else(|p| p.into_inner()).take() {
                        let _ = tx.send(result);
                    }
                });
            let wk = webview.inner() as *mut AnyObject;
            // SAFETY: inner() 是這個視窗的 WKWebView；with_webview 在主執行緒執行。
            unsafe {
                let _: () = msg_send![
                    wk,
                    takeSnapshotWithConfiguration: std::ptr::null_mut::<AnyObject>(),
                    completionHandler: &*handler
                ];
            }
        })
        .map_err(|e| e.to_string())?;
    let png = tokio::time::timeout(Duration::from_secs(10), rx)
        .await
        .map_err(|_| "截圖逾時".to_owned())?
        .map_err(|_| "截圖通道關閉".to_owned())??;
    Ok(base64::engine::general_purpose::STANDARD.encode(png))
}

#[cfg(not(target_os = "macos"))]
async fn snapshot(_app: &tauri::AppHandle) -> Result<String, String> {
    Err("shot 只支援 macOS".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "t0k";

    fn head(extra: &str) -> String {
        format!("POST /eval HTTP/1.1\r\nHost: 127.0.0.1:9\r\nAuthorization: Bearer t0k\r\n{extra}")
    }

    #[test]
    fn accepts_well_formed_request() {
        let (m, p, len) = check_head(&head("Content-Length: 5"), 9, TOKEN).unwrap();
        assert_eq!((m.as_str(), p.as_str(), len), ("POST", "/eval", 5));
    }

    #[test]
    fn rejects_wrong_token_host_origin_and_ambiguity() {
        let bad_token = "GET /status HTTP/1.1\r\nHost: 127.0.0.1:9\r\nAuthorization: Bearer nope";
        assert_eq!(check_head(bad_token, 9, TOKEN).unwrap_err().status, 401);
        let bad_host = "GET /status HTTP/1.1\r\nHost: localhost:9\r\nAuthorization: Bearer t0k";
        assert_eq!(check_head(bad_host, 9, TOKEN).unwrap_err().status, 403);
        assert_eq!(
            check_head(&head("Origin: https://evil.test"), 9, TOKEN)
                .unwrap_err()
                .status,
            403
        );
        assert_eq!(
            check_head(&head("Host: 127.0.0.1:9"), 9, TOKEN)
                .unwrap_err()
                .status,
            400
        );
        assert_eq!(
            check_head(&head("Authorization: Bearer t0k"), 9, TOKEN)
                .unwrap_err()
                .status,
            400
        );
        let dup_len = head("Content-Length: 1\r\nContent-Length: 1");
        assert_eq!(check_head(&dup_len, 9, TOKEN).unwrap_err().status, 400);
        assert_eq!(
            check_head(&head("Transfer-Encoding: chunked"), 9, TOKEN)
                .unwrap_err()
                .status,
            400
        );
        assert_eq!(
            check_head(
                &head(&format!("Content-Length: {}", MAX_BODY + 1)),
                9,
                TOKEN
            )
            .unwrap_err()
            .status,
            413
        );
    }

    #[test]
    fn constant_time_eq_basics() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
    }
}
