use std::time::Duration;

use serde_json::json;
use tauri_plugin_updater::{Error as PluginError, UpdaterExt};

use super::*;
use crate::transport::test_support::{hang_server, stall_server_after, watchdog};

const TEST_FETCH: FetchConfig = FetchConfig {
    idle: Duration::from_millis(300),
    https_only: false,
    accept_invalid_certs: false,
};

const TEST_TIMEOUTS: NetTimeouts = NetTimeouts {
    check_total: Duration::from_millis(400),
    idle: Duration::from_millis(500),
};

/// 假伺服器撐住連線的時間，要遠長於 watchdog（10 秒），舊碼才會被抓紅。
const HOLD_MS: u64 = 30_000;

fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

fn ok_reply(body: &str) -> &'static str {
    leak(format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    ))
}

fn redirect_to(next: &str) -> String {
    hang_server(
        leak(format!(
            "HTTP/1.1 302 Found\r\nLocation: {next}/asset\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        )),
        0,
    )
}

fn assert_timeout(result: Result<Vec<u8>, FetchFailure>) {
    match result {
        Err(FetchFailure::Http(error)) => assert!(error.is_timeout(), "不是逾時：{error:?}"),
        other => panic!("應該逾時：{other:?}"),
    }
}

/// 本機 https 假伺服器的自簽憑證（只給測試，`accept_invalid_certs` 才收）。
const TEST_CERT: &str = "-----BEGIN CERTIFICATE-----\nMIIBnDCCAUGgAwIBAgIUZl7g4j2/Se7n/o42+zLUii+K75gwCgYIKoZIzj0EAwIw\nFDESMBAGA1UEAwwJbG9jYWxob3N0MCAXDTI2MTAxMTA5Mzc1M1oYDzIxMjYwOTE3\nMDkzNzUzWjAUMRIwEAYDVQQDDAlsb2NhbGhvc3QwWTATBgcqhkjOPQIBBggqhkjO\nPQMBBwNCAASA+7y8eYtKQaHDoJx5FZVzNBJHE/4UEzdtYgqBsgXl6ftKBoQK1gno\nlp/BrX0/aXn+/SjLdYwY8siYIweTBTnJo28wbTAdBgNVHQ4EFgQUxVsO9CbwDcJQ\n368fNTlOPKC+Lt0wHwYDVR0jBBgwFoAUxVsO9CbwDcJQ368fNTlOPKC+Lt0wDwYD\nVR0TAQH/BAUwAwEB/zAaBgNVHREEEzARhwR/AAABgglsb2NhbGhvc3QwCgYIKoZI\nzj0EAwIDSQAwRgIhAPwmMfzRTlosH4/1JPMJI1ak9gjKCXPhmQyBPMBRP/zBAiEA\njDOBWNCvyY2efGN1GKwWnNgGXXS+M/Z4wbK6IO8Jct0=\n-----END CERTIFICATE-----";
const TEST_KEY: &str = "-----BEGIN PRIVATE KEY-----\nMIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQg2+qgcmiREgs8445L\ndDyXTwiliMKzx3X4/AsOint46smhRANCAASA+7y8eYtKQaHDoJx5FZVzNBJHE/4U\nEzdtYgqBsgXl6ftKBoQK1gnolp/BrX0/aXn+/SjLdYwY8siYIweTBTnJ\n-----END PRIVATE KEY-----";

/// 一次性的 https 伺服器：TLS 握手、讀一次請求、寫 `reply` 後關閉。回傳 base URL。
fn https_once(reply: String) -> String {
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};
    use std::io::{Read, Write};
    use std::sync::Arc;

    let certs = vec![CertificateDer::from_pem_slice(TEST_CERT.as_bytes()).expect("cert")];
    let key = PrivateKeyDer::from_pem_slice(TEST_KEY.as_bytes()).expect("key");
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("protocols")
    .with_no_client_auth()
    .with_single_cert(certs, key)
    .expect("server config");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let Ok((socket, _)) = listener.accept() else {
            return;
        };
        let _ = socket.set_read_timeout(Some(Duration::from_secs(5)));
        let connection = rustls::ServerConnection::new(Arc::new(config)).expect("connection");
        let mut stream = rustls::StreamOwned::new(connection, socket);
        let mut request = [0u8; 8192];
        let _ = stream.read(&mut request);
        let _ = stream.write_all(reply.as_bytes());
        let _ = stream.flush();
        stream.conn.send_close_notify();
        let _ = stream.flush();
    });
    format!("https://{address}")
}

fn https_redirect_to_http() -> String {
    let target = hang_server(ok_reply("hello"), 0);
    https_once(format!(
        "HTTP/1.1 302 Found\r\nLocation: {target}/asset\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    ))
}

#[tokio::test]
async fn https_only_rejects_redirect_to_http() {
    let start = https_redirect_to_http();
    let config = FetchConfig {
        https_only: true,
        accept_invalid_certs: true,
        ..TEST_FETCH
    };
    match watchdog(fetch_with(&format!("{start}/asset"), config)).await {
        Err(FetchFailure::Http(error)) => assert!(error.is_redirect(), "{error:?}"),
        other => panic!("轉址降級到 http 應被拒：{other:?}"),
    }
}

#[tokio::test]
async fn redirect_to_http_works_without_https_only() {
    // 對照組：同樣的轉址在 https_only 關掉時會成功，證明上一項是 https_only 擋的。
    let start = https_redirect_to_http();
    let config = FetchConfig {
        accept_invalid_certs: true,
        ..TEST_FETCH
    };
    let bytes = watchdog(fetch_with(&format!("{start}/asset"), config))
        .await
        .expect("對照組應成功");
    assert_eq!(bytes, b"hello");
}

#[test]
fn production_values() {
    assert_eq!(NetTimeouts::PRODUCTION.idle, Duration::from_secs(60));
    assert_eq!(NetTimeouts::PRODUCTION.check_total, Duration::from_secs(60));
    assert_eq!(PRODUCTION_FETCH.idle, Duration::from_secs(60));
    assert!(PRODUCTION_FETCH.https_only);
}

#[tokio::test]
async fn production_fetch_refuses_http() {
    let url = hang_server(ok_reply("hello"), 0);
    match watchdog(fetch_with(&format!("{url}/asset"), PRODUCTION_FETCH)).await {
        Err(FetchFailure::Http(_)) => {}
        other => panic!("https_only 應拒絕 http：{other:?}"),
    }
}

#[tokio::test]
async fn fetch_times_out_when_head_never_comes() {
    let url = hang_server("", HOLD_MS);
    assert_timeout(watchdog(fetch_with(&format!("{url}/asset"), TEST_FETCH)).await);
}

#[tokio::test]
async fn fetch_times_out_when_body_stalls() {
    let url = hang_server(
        "HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\nabc",
        HOLD_MS,
    );
    assert_timeout(watchdog(fetch_with(&format!("{url}/asset"), TEST_FETCH)).await);
}

#[tokio::test]
async fn fetch_keeps_slow_steady_body() {
    // 每 60 ms 一段（閒置窗的 1/5），總長約 2.4 秒＝閒置窗 8 倍：證明是閒置期限不是總期限。
    let script: Vec<(u64, String)> = (0..40).map(|_| (60, "x".repeat(10))).collect();
    let url = stall_server_after(0, script, 0);
    let bytes = watchdog(fetch_with(&format!("{url}/asset"), TEST_FETCH))
        .await
        .expect("慢但持續的本文應成功");
    assert_eq!(bytes.len(), 400);
}

#[tokio::test]
async fn fetch_follows_redirect() {
    let target = hang_server(ok_reply("hello"), 0);
    let start = redirect_to(&target);
    let bytes = watchdog(fetch_with(&format!("{start}/asset"), TEST_FETCH))
        .await
        .expect("302 後應拿到本文");
    assert_eq!(bytes, b"hello");
}

#[tokio::test]
async fn fetch_stops_after_five_redirects() {
    let mut next = hang_server(ok_reply("hello"), 0);
    for _ in 0..6 {
        next = redirect_to(&next);
    }
    match watchdog(fetch_with(&format!("{next}/asset"), TEST_FETCH)).await {
        Err(FetchFailure::Http(error)) => assert!(error.is_redirect(), "{error:?}"),
        other => panic!("超過 5 跳應失敗：{other:?}"),
    }
}

// ── 外掛：mock app 掛 updater 外掛，endpoint 指向假伺服器。

fn mock_updater(
    pubkey: &str,
    endpoint: &str,
) -> (
    tauri::App<tauri::test::MockRuntime>,
    tauri_plugin_updater::Updater,
) {
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    context.config_mut().plugins.0.insert(
        "updater".to_owned(),
        json!({ "pubkey": pubkey, "dangerousInsecureTransportProtocol": true }),
    );
    let app = tauri::test::mock_builder()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .build(context)
        .expect("mock app");
    let builder = app
        .updater_builder()
        .endpoints(vec![endpoint.parse().expect("endpoint")])
        .expect("endpoints")
        .target("test")
        .no_proxy();
    let updater = with_timeouts(builder, TEST_TIMEOUTS)
        .build()
        .expect("updater");
    (app, updater)
}

fn manifest(asset_url: &str, signature: &str) -> String {
    json!({
        "version": "9.9.9",
        "platforms": { "test": { "url": asset_url, "signature": signature } }
    })
    .to_string()
}

fn assert_plugin_timeout(error: PluginError) {
    match &error {
        PluginError::Reqwest(inner) => assert!(inner.is_timeout(), "不是逾時：{inner:?}"),
        other => panic!("應該逾時：{other:?}"),
    }
    assert_eq!(
        plugin_error_text(error),
        UiMsg::UpdateNetworkFailed.to_string()
    );
}

#[tokio::test]
async fn check_times_out_when_head_never_comes() {
    let (pubkey, _) = crate::updater::verify::sign_fixture(b"x");
    let endpoint = hang_server("", HOLD_MS);
    let (_app, updater) = mock_updater(&pubkey, &endpoint);
    let error = watchdog(updater.check()).await.err().expect("應逾時");
    assert_plugin_timeout(error);
}

#[tokio::test]
async fn check_times_out_on_trickling_manifest() {
    // 每段間隔 100 ms 都短於閒置窗（500 ms），整段超過 400 ms：只有總期限殺得掉。
    let (pubkey, _) = crate::updater::verify::sign_fixture(b"x");
    let body = manifest("http://127.0.0.1:9/asset", "sig");
    let script: Vec<(u64, String)> = body.chars().map(|c| (100, c.to_string())).collect();
    let endpoint = stall_server_after(0, script, 0);
    let (_app, updater) = mock_updater(&pubkey, &endpoint);
    let error = watchdog(updater.check()).await.err().expect("應逾時");
    assert_plugin_timeout(error);
}

#[tokio::test]
async fn download_times_out_when_body_stalls() {
    let (pubkey, signature) = crate::updater::verify::sign_fixture(b"x");
    let asset = hang_server(
        "HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\nabc",
        HOLD_MS,
    );
    let endpoint = hang_server(
        ok_reply(&manifest(&format!("{asset}/asset"), &signature)),
        0,
    );
    let (_app, updater) = mock_updater(&pubkey, &endpoint);
    let update = watchdog(updater.check())
        .await
        .expect("check")
        .expect("有新版");
    let error = watchdog(update.download(|_, _| {}, || {}))
        .await
        .expect_err("應逾時");
    assert_plugin_timeout(error);
}

#[tokio::test]
async fn download_is_not_capped_by_check_total() {
    // 每 100 ms 一段（閒置窗 1/5），總長約 3 秒＝檢查整段 7 倍多、閒置窗 6 倍：
    // 下完後應是驗簽失敗（簽的是別的內容），不是逾時。
    let (pubkey, signature) = crate::updater::verify::sign_fixture(b"other bytes");
    let script: Vec<(u64, String)> = (0..30).map(|_| (100, "y".repeat(10))).collect();
    let asset = stall_server_after(0, script, 0);
    let endpoint = hang_server(
        ok_reply(&manifest(&format!("{asset}/asset"), &signature)),
        0,
    );
    let (_app, updater) = mock_updater(&pubkey, &endpoint);
    let update = watchdog(updater.check())
        .await
        .expect("check")
        .expect("有新版");
    let mut received = 0usize;
    let error = watchdog(update.download(|chunk, _| received += chunk, || {}))
        .await
        .expect_err("簽章不符");
    assert!(
        matches!(error, PluginError::Minisign(_)),
        "應是驗簽錯誤：{error:?}"
    );
    assert_eq!(received, 300);
}
