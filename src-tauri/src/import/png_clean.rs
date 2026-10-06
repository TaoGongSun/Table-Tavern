//! 存圖前把圖整理成過嚴驗的 PNG。兩支流程：
//! - 嚴格版（AI 生圖）：合格原樣、APNG 剝成預設靜態圖、過大縮圖，壞檔拒收不修。
//! - 救圖版（角色卡、GM 圖匯入）：先截到 IEND 走嚴格版，能保留原位元組就不重編；不行才重編，
//!   重編也失敗才算救不回。
//!
//! 縮圖前先只讀檔頭查來源尺寸（`REENCODE_SOURCE_LIMITS`），超過不解碼，解碼工作量有界。
use super::card_io::{png_chunk, PNG_MAGIC};
use super::png_image::{
    classify_png_image, for_each_chunk, ImageLimits, PngReject, REENCODE_SOURCE_LIMITS,
    STORED_IMAGE_LIMITS,
};
use std::io::Cursor;

const MAX_DECODE_ALLOC: u64 = 512 * 1024 * 1024;
/// 取整誤差讓輸出仍判過大時，最多再縮幾階
const SHRINK_RETRIES: u32 = 4;

/// 要存的結果。`AsIs`＝傳入那份本來就合格，呼叫端存自己手上的位元組；
/// 截過、剝過、重編過的一律帶要存的位元組回來，呼叫端不會把尾隨寫回去。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Stored {
    AsIs,
    Rewritten(Vec<u8>),
    Reencoded(Vec<u8>),
}

impl Stored {
    pub(crate) fn bytes<'a>(&'a self, original: &'a [u8]) -> &'a [u8] {
        match self {
            Self::AsIs => original,
            Self::Rewritten(bytes) | Self::Reencoded(bytes) => bytes,
        }
    }
}

/// 存圖上限與縮圖來源上限；正式呼叫用預設，測試換小上限。
#[derive(Clone, Copy)]
pub(crate) struct CleanLimits {
    pub stored: ImageLimits,
    pub source: ImageLimits,
}

pub(crate) const CLEAN_LIMITS: CleanLimits = CleanLimits {
    stored: STORED_IMAGE_LIMITS,
    source: REENCODE_SOURCE_LIMITS,
};

fn is_animation(kind: &[u8; 4]) -> bool {
    matches!(kind, b"acTL" | b"fcTL" | b"fdAT")
}

/// 原檔先過完整結構／CRC 檢查（IDAT 連續、恰一個 IEND、IEND 後無資料），再丟掉動畫 chunk。
/// 沒有動畫 chunk 回 None。剝後留下的就是 IDAT 預設圖。
pub(crate) fn strip_animation(bytes: &[u8]) -> Result<Option<Vec<u8>>, String> {
    let mut out = PNG_MAGIC.to_vec();
    let mut stripped = false;
    for_each_chunk(bytes, |chunk| {
        if is_animation(&chunk.kind) {
            stripped = true;
        } else {
            out.extend_from_slice(&bytes[chunk.start..chunk.end]);
        }
        Ok(())
    })?;
    Ok(stripped.then_some(out))
}

/// 不驗 CRC 的逐 chunk 掃描，掃到 IEND（含）或遇到不完整的 chunk 就停。
/// 回傳 IEND 結尾位置（沒掃到完整 IEND 時 None）。
fn scan_until_iend(bytes: &[u8], mut visit: impl FnMut(&[u8; 4], &[u8])) -> Option<usize> {
    if !bytes.starts_with(PNG_MAGIC) {
        return None;
    }
    let mut offset = PNG_MAGIC.len();
    while bytes.len() - offset >= 12 {
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let end = offset
            .checked_add(12)
            .and_then(|end| end.checked_add(length))
            .filter(|end| *end <= bytes.len())?;
        let kind: [u8; 4] = bytes[offset + 4..offset + 8].try_into().unwrap();
        visit(&kind, &bytes[offset + 8..end - 4]);
        if &kind == b"IEND" {
            return Some(end);
        }
        offset = end;
    }
    None
}

/// 截到 IEND 為止；IEND 後本來就沒東西或掃不到完整 IEND 時回 None。
fn truncate_at_iend(bytes: &[u8]) -> Option<&[u8]> {
    scan_until_iend(bytes, |_, _| {})
        .filter(|end| *end < bytes.len())
        .map(|end| &bytes[..end])
}

/// 等比縮到兩邊 ≤ max_side 且像素 ≤ max_pixels；已在上限內原樣回傳。各邊至少 1。
pub(crate) fn fit_size(width: u32, height: u32, limits: ImageLimits) -> (u32, u32) {
    let fits = |w: u32, h: u32| {
        w <= limits.max_side
            && h <= limits.max_side
            && u64::from(w) * u64::from(h) <= limits.max_pixels
    };
    if fits(width, height) {
        return (width, height);
    }
    let (w, h) = (f64::from(width), f64::from(height));
    let scale = (f64::from(limits.max_side) / w)
        .min(f64::from(limits.max_side) / h)
        .min((limits.max_pixels as f64 / (w * h)).sqrt());
    let mut target_w = ((w * scale).floor() as u32).max(1);
    let mut target_h = ((h * scale).floor() as u32).max(1);
    // 取整後仍可能差一點：逐步縮小較長邊直到合格
    while !fits(target_w, target_h) {
        if target_w >= target_h && target_w > 1 {
            target_w -= 1;
        } else if target_h > 1 {
            target_h -= 1;
        } else {
            break;
        }
    }
    (target_w, target_h)
}

/// 解碼（先只讀檔頭查來源上限）→ 超出 `limits.stored` 就縮 → 編 PNG → 嚴驗輸出。
pub(crate) fn reencode(
    bytes: &[u8],
    format: image::ImageFormat,
    limits: CleanLimits,
) -> Result<Vec<u8>, String> {
    use image::ImageDecoder;
    let mut decode_limits = image::Limits::default();
    decode_limits.max_image_width = Some(limits.source.max_side);
    decode_limits.max_image_height = Some(limits.source.max_side);
    decode_limits.max_alloc = Some(MAX_DECODE_ALLOC);
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(decode_limits);
    let decoder = reader.into_decoder().map_err(|error| error.to_string())?;
    let (width, height) = decoder.dimensions();
    if width == 0
        || height == 0
        || width > limits.source.max_side
        || height > limits.source.max_side
        || u64::from(width) * u64::from(height) > limits.source.max_pixels
    {
        return Err(format!(
            "source image {width}x{height} is too large to process"
        ));
    }
    let decoded = image::DynamicImage::from_decoder(decoder).map_err(|error| error.to_string())?;
    let mut target = fit_size(width, height, limits.stored);
    for _ in 0..=SHRINK_RETRIES {
        let resized = if target == (width, height) {
            None
        } else {
            Some(decoded.resize_exact(target.0, target.1, image::imageops::FilterType::CatmullRom))
        };
        let mut png = Vec::new();
        resized
            .as_ref()
            .unwrap_or(&decoded)
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|error| error.to_string())?;
        match classify_png_image(&png, limits.stored) {
            Ok(_) => return Ok(png),
            Err(PngReject::TooLarge { .. }) => {
                target = (
                    (target.0 - target.0 / 100).max(1),
                    (target.1 - target.1 / 100).max(1),
                );
            }
            Err(other) => return Err(other.to_string()),
        }
    }
    Err("re-encoded image still exceeds limits".to_owned())
}

/// 嚴格版：合格原樣；APNG（原檔結構合格）剝成預設靜態圖；過大且結構合格就縮圖；其餘回錯。
pub(crate) fn strict_stored_png(bytes: &[u8]) -> Result<Stored, String> {
    strict_with(bytes, CLEAN_LIMITS)
}

pub(crate) fn strict_with(bytes: &[u8], limits: CleanLimits) -> Result<Stored, String> {
    let mut has_animation_chunk = false;
    scan_until_iend(bytes, |kind, _| has_animation_chunk |= is_animation(kind));
    let stripped = if has_animation_chunk {
        strip_animation(bytes)?
    } else {
        None
    };
    let candidate = stripped.as_deref().unwrap_or(bytes);
    match classify_png_image(candidate, limits.stored) {
        Ok(_) => Ok(match stripped {
            Some(bytes) => Stored::Rewritten(bytes),
            None => Stored::AsIs,
        }),
        Err(PngReject::TooLarge { .. }) => {
            // 結構仍嚴格，只放寬到來源上限；過了才縮
            classify_png_image(candidate, limits.source).map_err(|error| error.to_string())?;
            reencode(candidate, image::ImageFormat::Png, limits).map(Stored::Reencoded)
        }
        Err(error) => Err(error.to_string()),
    }
}

/// 救圖版：截到 IEND 走嚴格版，保得住原位元組就不重編；不行才重編。救不回回 None。
pub(crate) fn salvage_stored_png(bytes: &[u8]) -> Option<Stored> {
    salvage_with(bytes, CLEAN_LIMITS)
}

pub(crate) fn salvage_with(bytes: &[u8], limits: CleanLimits) -> Option<Stored> {
    let truncated = truncate_at_iend(bytes);
    let base = truncated.unwrap_or(bytes);
    match strict_with(base, limits) {
        Ok(Stored::AsIs) => Some(match truncated {
            Some(base) => Stored::Rewritten(base.to_vec()),
            None => Stored::AsIs,
        }),
        Ok(stored) => Some(stored),
        Err(_) => reencode(base, image::ImageFormat::Png, limits)
            .ok()
            .map(Stored::Reencoded),
    }
}

/// 把原檔 IEND 前 keyword 為 chara／ccv3 的 tEXt 照原順序（重算 CRC）插到乾淨圖的 IEND 前。
/// 遇到不完整的 chunk 就停；乾淨圖必須以 IEND 結尾。
pub(crate) fn transplant_card_text(original: &[u8], clean: &[u8]) -> Option<Vec<u8>> {
    let mut texts = Vec::new();
    scan_until_iend(original, |kind, data| {
        if kind == b"tEXt" {
            let keyword = data.split(|byte| *byte == 0).next().unwrap_or_default();
            if keyword == b"chara" || keyword == b"ccv3" {
                texts.push(png_chunk(b"tEXt", data));
            }
        }
    });
    let iend = clean.len().checked_sub(12)?;
    if &clean[iend + 4..iend + 8] != b"IEND" {
        return None;
    }
    let mut out = clean[..iend].to_vec();
    for text in texts {
        out.extend_from_slice(&text);
    }
    out.extend_from_slice(&clean[iend..]);
    Some(out)
}

#[cfg(test)]
pub(crate) mod tests;
