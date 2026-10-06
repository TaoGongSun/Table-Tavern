use super::card_io::{base64_encode, PNG_MAGIC};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

pub(super) struct TestRoot(PathBuf);

impl TestRoot {
    pub(super) fn new(label: &str) -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "table-tavern-import-{label}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 過得了嚴驗的角色卡 PNG：真圖＋IEND 前一個 tEXt chara。
pub(crate) fn card_png(chara_json: &str) -> Vec<u8> {
    let image = super::png_image::test_png::real_png(4, 3);
    let text = format!("chara\0{}", base64_encode(chara_json.as_bytes()));
    let iend = image.len() - 12;
    let mut png = image[..iend].to_vec();
    png.extend_from_slice(&super::card_io::png_chunk(b"tEXt", text.as_bytes()));
    png.extend_from_slice(&image[iend..]);
    png
}

/// 只有 tEXt 的假 PNG（沒有 IHDR／IDAT、CRC 是零）：卡資料讀得到，圖救不回。
pub(super) fn minimal_png(chara_json: &str) -> Vec<u8> {
    let mut png = PNG_MAGIC.to_vec();
    let text = format!("chara\0{}", base64_encode(chara_json.as_bytes()));
    png.extend_from_slice(&(text.len() as u32).to_be_bytes());
    png.extend_from_slice(b"tEXt");
    png.extend_from_slice(text.as_bytes());
    png.extend_from_slice(&[0; 4]);
    png
}
