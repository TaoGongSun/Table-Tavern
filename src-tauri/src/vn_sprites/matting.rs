//! 去背後備（plans/vn-mode-pkg7.md §4「去背後備」）：不透明的定稿或變體身體（agy／API 來源、玩家上傳）
//! 用 BiRefNet-lite（MIT，官方 GitHub release）在本機推論出 alpha。權重第一次需要時下載到設定目錄，
//! SHA-256 寫死在這裡，下載完與每次載入前都驗，驗不過刪檔拒用。原生透明的圖原樣通過。
use crate::config_root;
use image::imageops::{self, FilterType};
use image::{GrayImage, RgbaImage};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};

const MODEL_URL: &str = "https://github.com/ZhengPeng7/BiRefNet/releases/download/v1/BiRefNet-general-bb_swin_v1_tiny-epoch_232.onnx";
const MODEL_SHA256: &str = "5600024376f572a557870a5eb0afb1e5961636bef4e1e22132025467d0f03333";
const MODEL_FILE: &str = "birefnet-lite-general-epoch_232.onnx";
const SIDE: u32 = 1024;
const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];
/// 半透明以下的像素不到 1%＝不透明圖（原生透明定稿背景佔 35–51%）
const OPAQUE_RATIO: f64 = 0.01;

/// 下載與推論同時只跑一份（權重 224 MB、推論吃滿 CPU）
fn gate() -> &'static tokio::sync::Mutex<()> {
    static GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    &GATE
}

pub(crate) fn is_opaque(image: &RgbaImage) -> bool {
    let clear = image.pixels().filter(|pixel| pixel[3] < 128).count();
    (clear as f64) < OPAQUE_RATIO * image.pixels().len().max(1) as f64
}

/// 不透明就去背，原生透明原樣回傳
pub(crate) async fn ensure_alpha(app: &tauri::AppHandle, png: Vec<u8>) -> Result<Vec<u8>, String> {
    let image = super::compose::decode(&png)?;
    if !is_opaque(&image) {
        return Ok(png);
    }
    let _gate = gate().lock().await;
    let path = config_root(app)?.join("models").join(MODEL_FILE);
    if !path.is_file() {
        download(&path).await?;
    }
    tokio::task::spawn_blocking(move || {
        verify(&path)?;
        let mask = infer(&path, &image)?;
        super::compose::encode_png(&apply_mask(image, &mask))
    })
    .await
    .map_err(|error| error.to_string())?
}

/// 驗權重雜湊；不符就刪檔（下次需要時重新下載）
fn verify(path: &Path) -> Result<(), String> {
    let mut hasher = Sha256::new();
    std::io::copy(
        &mut std::fs::File::open(path).map_err(|error| error.to_string())?,
        &mut hasher,
    )
    .map_err(|error| error.to_string())?;
    if hex(&hasher.finalize()) == MODEL_SHA256 {
        return Ok(());
    }
    let _ = std::fs::remove_file(path);
    Err("VN_SPRITE_MATTING_MODEL_INVALID".to_owned())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// 串流下載到暫存檔，雜湊對了才改名就位
async fn download(path: &Path) -> Result<(), String> {
    use futures_util::StreamExt;
    let dir = path.parent().ok_or("VN_SPRITE_MATTING_DOWNLOAD")?;
    std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let partial = PathBuf::from(format!("{}.part", path.display()));
    let response = reqwest::get(MODEL_URL)
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| format!("VN_SPRITE_MATTING_DOWNLOAD: {error}"))?;
    let mut file = std::fs::File::create(&partial).map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| format!("VN_SPRITE_MATTING_DOWNLOAD: {error}"))?;
        hasher.update(&chunk);
        file.write_all(&chunk).map_err(|error| error.to_string())?;
    }
    drop(file);
    if hex(&hasher.finalize()) != MODEL_SHA256 {
        let _ = std::fs::remove_file(&partial);
        return Err("VN_SPRITE_MATTING_MODEL_INVALID".to_owned());
    }
    std::fs::rename(&partial, path).map_err(|error| error.to_string())
}

/// 1024² ImageNet 正規化的 NCHW
fn preprocess(image: &RgbaImage) -> Vec<f32> {
    let scaled = imageops::resize(image, SIDE, SIDE, FilterType::Triangle);
    let plane = (SIDE * SIDE) as usize;
    let mut data = vec![0.0; plane * 3];
    for (index, pixel) in scaled.pixels().enumerate() {
        for channel in 0..3 {
            data[channel * plane + index] =
                (f32::from(pixel[channel]) / 255.0 - MEAN[channel]) / STD[channel];
        }
    }
    data
}

/// 模型輸出是 logits：sigmoid 成 0–255 的遮罩
fn mask_from_logits(logits: &[f32]) -> GrayImage {
    let bytes = logits
        .iter()
        .map(|logit| ((1.0 / (1.0 + (-logit).exp())) * 255.0).round() as u8)
        .collect();
    GrayImage::from_raw(SIDE, SIDE, bytes).unwrap_or_else(|| GrayImage::new(SIDE, SIDE))
}

fn infer(path: &Path, image: &RgbaImage) -> Result<GrayImage, String> {
    let fail = |error: ort::Error| format!("VN_SPRITE_MATTING_FAILED: {error}");
    // ONNX Runtime 環境整個行程一份
    static ENVIRONMENT: std::sync::OnceLock<ort::environment::Environment> =
        std::sync::OnceLock::new();
    let environment = match ENVIRONMENT.get() {
        Some(environment) => environment,
        None => {
            let built = ort::environment::init().build().map_err(fail)?;
            ENVIRONMENT.get_or_init(|| built)
        }
    };
    let mut session = ort::session::Session::builder(environment)
        .map_err(fail)?
        .commit_from_file(path)
        .map_err(fail)?;
    let input = ort::value::Tensor::from_array((
        [1usize, 3, SIDE as usize, SIDE as usize],
        preprocess(image),
    ))
    .map_err(fail)?;
    let outputs = session.run(ort::inputs![input]).map_err(fail)?;
    let (_, logits) = outputs[outputs.len() - 1]
        .try_extract_tensor::<f32>()
        .map_err(fail)?;
    Ok(mask_from_logits(logits))
}

/// 遮罩縮回原圖尺寸當 alpha
fn apply_mask(mut image: RgbaImage, mask: &GrayImage) -> RgbaImage {
    let (width, height) = image.dimensions();
    let mask = imageops::resize(mask, width, height, FilterType::Triangle);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        pixel[3] = mask.get_pixel(x, y)[0];
    }
    image
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn opaque_detection() {
        let mut image = RgbaImage::from_pixel(100, 100, Rgba([200, 200, 200, 255]));
        assert!(is_opaque(&image));
        for x in 0..100 {
            image.put_pixel(x, 0, Rgba([0, 0, 0, 0]));
            image.put_pixel(x, 1, Rgba([0, 0, 0, 0]));
        }
        assert!(!is_opaque(&image));
    }

    #[test]
    fn tampered_weights_are_rejected_and_removed() {
        let dir = std::env::temp_dir().join(format!("matting-{}", crate::data::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(MODEL_FILE);
        std::fs::write(&path, b"not the model").unwrap();
        assert_eq!(
            verify(&path).unwrap_err(),
            "VN_SPRITE_MATTING_MODEL_INVALID"
        );
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn preprocess_normalizes_planes_and_mask_becomes_alpha() {
        let data = preprocess(&RgbaImage::from_pixel(10, 20, Rgba([255, 0, 0, 255])));
        let plane = (SIDE * SIDE) as usize;
        assert_eq!(data.len(), plane * 3);
        assert!((data[0] - (1.0 - MEAN[0]) / STD[0]).abs() < 1e-5);
        assert!((data[plane] + MEAN[1] / STD[1]).abs() < 1e-5);
        let mut logits = vec![-20.0; plane];
        logits[..plane / 2].fill(20.0);
        let mask = mask_from_logits(&logits);
        let cut = apply_mask(RgbaImage::from_pixel(8, 8, Rgba([1, 2, 3, 255])), &mask);
        assert_eq!(cut.get_pixel(4, 0)[3], 255);
        assert_eq!(cut.get_pixel(4, 7)[3], 0);
        assert_eq!(cut.get_pixel(4, 7)[0], 1);
    }
}
