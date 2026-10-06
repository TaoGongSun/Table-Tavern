//! 生圖輸出統一存成通過嚴驗的 PNG：API 回的 data URL／遠端網址、CLI 存的檔案，都先變成位元組，
//! 依 magic bytes 認格式（不信宣稱的 MIME）。PNG 走嚴格版（合格原樣、APNG 留預設靜態圖、過大縮圖，
//! 壞檔拒收不修）；JPEG／WebP 解碼重編 PNG，過大一樣縮。
//! 失敗一律回帶穩定前綴的錯誤，呼叫端失敗就不寫圖庫。
use crate::import::png_clean::{self, Stored, CLEAN_LIMITS};
use base64::Engine;
use std::time::Duration;

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);
/// 實際讀到的位元組上限（不只看 Content-Length，伺服器可以不給或說謊）
const MAX_DOWNLOAD_BYTES: usize = 32 * 1024 * 1024;

const UNSUPPORTED: &str = "AI_IMAGE_UNSUPPORTED_FORMAT";
const DECODE_FAILED: &str = "AI_IMAGE_DECODE_FAILED";
const DOWNLOAD_FAILED: &str = "AI_IMAGE_DOWNLOAD_FAILED";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Png,
    Jpeg,
    WebP,
}

fn sniff(bytes: &[u8]) -> Option<Format> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(Format::Png)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(Format::Jpeg)
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(Format::WebP)
    } else {
        None
    }
}

/// 認不出的格式附一點線索：SVG 直說，其餘給開頭幾個位元組
fn describe_unknown(bytes: &[u8]) -> String {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).to_ascii_lowercase();
    if head.contains("<svg") {
        return "svg".to_owned();
    }
    let hex: Vec<String> = bytes
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("head={}", hex.join(" "))
}

/// 同步轉檔：PNG 走嚴格版整理，JPEG／WebP 解碼（先查來源尺寸）後重編 PNG。
pub(crate) fn to_png(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    let format = match sniff(&bytes) {
        Some(Format::Png) => {
            return match png_clean::strict_stored_png(&bytes) {
                Ok(Stored::AsIs) => Ok(bytes),
                Ok(Stored::Rewritten(png) | Stored::Reencoded(png)) => Ok(png),
                Err(error) => Err(format!("{DECODE_FAILED}: {error}")),
            };
        }
        Some(Format::Jpeg) => image::ImageFormat::Jpeg,
        Some(Format::WebP) => image::ImageFormat::WebP,
        None => return Err(format!("{UNSUPPORTED}: {}", describe_unknown(&bytes))),
    };
    png_clean::reencode(&bytes, format, CLEAN_LIMITS)
        .map_err(|error| format!("{DECODE_FAILED}: {error}"))
}

/// 轉檔吃 CPU，丟到 blocking 執行緒，不卡住 async runtime。
pub(crate) async fn to_png_blocking(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    tokio::task::spawn_blocking(move || to_png(bytes))
        .await
        .map_err(|error| format!("{DECODE_FAILED}: {error}"))?
}

/// 下載遠端圖片：不帶任何認證、要 2xx、整段（含讀 body）逾時、實際位元組數設上限。
pub(crate) async fn download(
    url: &str,
    timeout: Duration,
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    // reqwest 的 client timeout 管不到 body 讀到一半停住，整段（送出＋讀 body）另外包一層
    tokio::time::timeout(timeout, download_body(url, max_bytes))
        .await
        .unwrap_or_else(|_| Err(format!("{DOWNLOAD_FAILED}: timed out after {timeout:?}")))
}

async fn download_body(url: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
    let failed = |detail: String| format!("{DOWNLOAD_FAILED}: {detail}");
    let mut response = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .map_err(|error| failed(error.to_string()))?;
    let status = response.status();
    if !status.is_success() {
        return Err(failed(format!("status={status}")));
    }
    let too_large = || failed(format!("larger than {max_bytes} bytes"));
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| failed(error.to_string()))?
    {
        if bytes.len() + chunk.len() > max_bytes {
            return Err(too_large());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// 生圖回來的字串（`data:…;base64,…` 或 http(s) 網址）轉成位元組。
async fn encoded_bytes(image: &str) -> Result<Vec<u8>, String> {
    if image.starts_with("http://") || image.starts_with("https://") {
        return download(image, DOWNLOAD_TIMEOUT, MAX_DOWNLOAD_BYTES).await;
    }
    let payload = image
        .strip_prefix("data:")
        .and_then(|rest| rest.split_once(','))
        .filter(|(header, _)| header.ends_with(";base64"))
        .map(|(_, payload)| payload)
        .ok_or_else(|| format!("{UNSUPPORTED}: not a base64 data URL"))?;
    base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .map_err(|error| format!("{DECODE_FAILED}: {error}"))
}

/// 生圖輸出的來源：API／CLI 回的字串，或已讀進記憶體的 CLI 存檔。
pub(crate) enum GeneratedImage {
    Encoded(String),
    Bytes(Vec<u8>),
}

/// 任一來源 → PNG 位元組。
pub(crate) async fn normalize(image: GeneratedImage) -> Result<Vec<u8>, String> {
    let bytes = match image {
        GeneratedImage::Encoded(text) => encoded_bytes(&text).await?,
        GeneratedImage::Bytes(bytes) => bytes,
    };
    to_png_blocking(bytes).await
}

pub(crate) fn png_data_url(png: &[u8]) -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    )
}

#[cfg(test)]
mod tests;
