use super::*;
use std::io::{Read, Write};

const ALPHA_PNG: &[u8] = include_bytes!("fixtures/alpha.png");
const OPAQUE_JPG: &[u8] = include_bytes!("fixtures/opaque.jpg");
const ALPHA_WEBP: &[u8] = include_bytes!("fixtures/alpha.webp");
const TINY_SVG: &[u8] = include_bytes!("fixtures/tiny.svg");
const BROKEN_JPG: &[u8] = include_bytes!("fixtures/broken.jpg");

fn decode_png(bytes: &[u8]) -> image::DynamicImage {
    assert_eq!(sniff(bytes), Some(Format::Png));
    image::load_from_memory_with_format(bytes, image::ImageFormat::Png).unwrap()
}

/// 本機一次性 HTTP 伺服器：回固定的狀態列、標頭與本體；delay 模擬慢回應。
fn serve(status: &str, headers: &str, body: Vec<u8>, delay: Duration) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let status = status.to_owned();
    let headers = headers.to_owned();
    std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = [0u8; 4096];
        let _ = socket.read(&mut request);
        std::thread::sleep(delay);
        let head = format!("HTTP/1.1 {status}\r\n{headers}Connection: close\r\n\r\n");
        let _ = socket.write_all(head.as_bytes());
        let _ = socket.write_all(&body);
    });
    format!("http://{address}/image")
}

fn with_length(body: &[u8]) -> String {
    format!("Content-Length: {}\r\n", body.len())
}

#[test]
fn png_is_kept_byte_for_byte() {
    assert_eq!(to_png(ALPHA_PNG.to_vec()).unwrap(), ALPHA_PNG);
}

#[test]
fn jpeg_is_reencoded_as_png() {
    let png = to_png(OPAQUE_JPG.to_vec()).unwrap();
    let image = decode_png(&png);
    assert_eq!((image.width(), image.height()), (4, 6));
}

#[test]
fn webp_is_reencoded_as_png_keeping_alpha() {
    let png = to_png(ALPHA_WEBP.to_vec()).unwrap();
    let image = decode_png(&png).to_rgba8();
    assert_eq!(image.dimensions(), (4, 6));
    let alpha = image.get_pixel(0, 0)[3];
    assert!(alpha > 0 && alpha < 255, "alpha={alpha}");
}

#[test]
fn svg_is_rejected_as_unsupported() {
    let error = to_png(TINY_SVG.to_vec()).unwrap_err();
    assert!(
        error.starts_with("AI_IMAGE_UNSUPPORTED_FORMAT: svg"),
        "{error}"
    );
}

#[test]
fn unknown_bytes_are_rejected_as_unsupported() {
    let error = to_png(b"GIF89a....".to_vec()).unwrap_err();
    assert!(
        error.starts_with("AI_IMAGE_UNSUPPORTED_FORMAT: head=47 49 46"),
        "{error}"
    );
}

#[test]
fn broken_jpeg_reports_decode_failure() {
    let error = to_png(BROKEN_JPG.to_vec()).unwrap_err();
    assert!(error.starts_with("AI_IMAGE_DECODE_FAILED: "), "{error}");
}

/// 宣稱 PNG、實際是 JPEG：照位元組認，轉成真的 PNG
#[tokio::test]
async fn data_url_mime_is_ignored_in_favour_of_bytes() {
    let data_url = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(OPAQUE_JPG)
    );
    let png = normalize(GeneratedImage::Encoded(data_url)).await.unwrap();
    assert_ne!(png, OPAQUE_JPG);
    decode_png(&png);
}

/// 宣稱 PNG、實際是 SVG：一樣拒收
#[tokio::test]
async fn data_url_claiming_png_with_svg_bytes_is_rejected() {
    let data_url = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(TINY_SVG)
    );
    let error = normalize(GeneratedImage::Encoded(data_url))
        .await
        .unwrap_err();
    assert!(
        error.starts_with("AI_IMAGE_UNSUPPORTED_FORMAT: "),
        "{error}"
    );
}

#[tokio::test]
async fn remote_webp_is_downloaded_and_converted() {
    let url = serve(
        "200 OK",
        &format!("Content-Type: image/png\r\n{}", with_length(ALPHA_WEBP)),
        ALPHA_WEBP.to_vec(),
        Duration::ZERO,
    );
    let png = normalize(GeneratedImage::Encoded(url)).await.unwrap();
    decode_png(&png);
}

#[tokio::test]
async fn download_rejects_non_success_status() {
    let url = serve(
        "404 Not Found",
        &with_length(b"nope"),
        b"nope".to_vec(),
        Duration::ZERO,
    );
    let error = download(&url, Duration::from_secs(5), 1024)
        .await
        .unwrap_err();
    assert!(
        error.starts_with("AI_IMAGE_DOWNLOAD_FAILED: status=404"),
        "{error}"
    );
}

#[tokio::test]
async fn download_times_out() {
    let url = serve(
        "200 OK",
        &with_length(ALPHA_PNG),
        ALPHA_PNG.to_vec(),
        Duration::from_millis(1500),
    );
    let error = download(&url, Duration::from_millis(200), 1024)
        .await
        .unwrap_err();
    assert!(error.starts_with("AI_IMAGE_DOWNLOAD_FAILED: "), "{error}");
}

#[tokio::test]
async fn download_rejects_declared_oversize_body() {
    let url = serve(
        "200 OK",
        &with_length(ALPHA_PNG),
        ALPHA_PNG.to_vec(),
        Duration::ZERO,
    );
    let error = download(&url, Duration::from_secs(5), 10)
        .await
        .unwrap_err();
    assert!(error.contains("larger than 10 bytes"), "{error}");
}

/// 沒給 Content-Length 也要算實際讀到的位元組
#[tokio::test]
async fn download_rejects_undeclared_oversize_body() {
    let url = serve("200 OK", "", ALPHA_PNG.to_vec(), Duration::ZERO);
    let error = download(&url, Duration::from_secs(5), 10)
        .await
        .unwrap_err();
    assert!(error.contains("larger than 10 bytes"), "{error}");
}

#[test]
fn png_data_url_uses_png_mime() {
    assert_eq!(png_data_url(b"png"), "data:image/png;base64,cG5n");
}

/// 標頭先送出、body 送一半就停住：逾時要涵蓋讀 body 的階段
#[tokio::test]
async fn download_times_out_when_body_stalls_after_headers() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = [0u8; 4096];
        let _ = socket.read(&mut request);
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            ALPHA_PNG.len()
        );
        socket.write_all(head.as_bytes()).unwrap();
        socket.write_all(&ALPHA_PNG[..10]).unwrap();
        socket.flush().unwrap();
        std::thread::sleep(Duration::from_secs(5));
    });
    let error = download(
        &format!("http://{address}/image"),
        Duration::from_millis(300),
        1024,
    )
    .await
    .unwrap_err();
    // 是我們的逾時觸發，不是伺服器斷線後的 body 解碼錯誤
    assert!(
        error.starts_with("AI_IMAGE_DOWNLOAD_FAILED: timed out"),
        "{error}"
    );
}

/// 遠端圖片網址是第三方主機：下載請求不能帶任何認證標頭
#[tokio::test]
async fn download_sends_no_authorization_header() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = [0u8; 4096];
        let read = socket.read(&mut request).unwrap_or(0);
        sender
            .send(String::from_utf8_lossy(&request[..read]).to_string())
            .unwrap();
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            ALPHA_PNG.len()
        );
        socket.write_all(head.as_bytes()).unwrap();
        socket.write_all(ALPHA_PNG).unwrap();
    });
    let bytes = download(
        &format!("http://{address}/image"),
        Duration::from_secs(5),
        1024,
    )
    .await
    .unwrap();
    assert_eq!(bytes, ALPHA_PNG);
    let request = receiver.recv().unwrap().to_ascii_lowercase();
    assert!(request.starts_with("get /image"), "{request}");
    assert!(!request.contains("authorization:"), "{request}");
}
