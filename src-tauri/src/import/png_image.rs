//! PNG 嚴格驗證（信任邊界共用）：一次串流走過整個檔，不建 chunk 表——
//! chunk 邊界與每個 CRC、IHDR 合法組合、PLTE 規則、IDAT 連續、恰一個零長 IEND 且其後無資料；
//! IDAT 串成的 zlib 流逐段解壓（固定 64 KiB 緩衝），必須正好結束、Adler-32 相符、
//! 解出長度等於 IHDR 推算的原始長度，每列濾波器位元組 0–4；索引色圖逐列反濾波（含 Adam7）
//! 核對每個像素的 palette 索引都在 PLTE 筆數內。動畫 PNG（acTL／fcTL／fdAT）不收。
//! 只查 magic 或用容錯解碼器會放過沒 IDAT、尾端截斷、壞壓縮流、錯 checksum 與越界索引的假圖。

use super::card_io::{crc32, PNG_MAGIC};
use flate2::{Decompress, FlushDecompress, Status};

/// 一個 chunk 在原檔中的位置：`start..end` 是含長度／型別／CRC 的整段，`data` 是內容。
pub(crate) struct PngChunk<'a> {
    pub kind: [u8; 4],
    pub data: &'a [u8],
    pub start: usize,
    pub end: usize,
}

/// 尺寸上限：寬高各自上限與像素總數上限（解壓只用固定緩衝，記憶體不隨圖變大）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ImageLimits {
    pub max_side: u32,
    pub max_pixels: u64,
}

/// 存進資料夾的圖（角色圖、頭像、GM 圖、圖庫、重構卡素材）一律只認這組上限。
pub(crate) const STORED_IMAGE_LIMITS: ImageLimits = ImageLimits {
    max_side: 8192,
    max_pixels: 24_000_000,
};

/// 要縮圖的來源上限：64M 像素 × 8 位元組（RGBA16）≈ 512 MB，與解碼器配置上限對齊；
/// 超過一律當救不回，不解碼。
pub(crate) const REENCODE_SOURCE_LIMITS: ImageLimits = ImageLimits {
    max_side: 16384,
    max_pixels: 64_000_000,
};

/// 嚴驗不過的原因：動畫與過大可以處理（剝動畫、縮圖），其餘是壞檔。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PngReject {
    Animated,
    TooLarge { width: u32, height: u32 },
    Invalid(String),
}

impl std::fmt::Display for PngReject {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Animated => formatter.write_str(ANIMATED),
            Self::TooLarge { width, height } => {
                write!(formatter, "image {width}x{height} is too large")
            }
            Self::Invalid(detail) => formatter.write_str(detail),
        }
    }
}

const ANIMATED: &str = "animated PNG is not supported";

/// 串流走訪每個 chunk 並做結構檢查：magic、長度邊界、CRC；首 chunk 是 IHDR 且只有一個；
/// IDAT 至少一個且連續；IEND 恰一個、零長、是最後一個 chunk，之後沒有任何資料。
/// 不配置與 chunk 數成正比的記憶體；visit 回 Err 就整個停下。
pub(crate) fn for_each_chunk<'a>(
    bytes: &'a [u8],
    mut visit: impl FnMut(&PngChunk<'a>) -> Result<(), String>,
) -> Result<(), String> {
    if !bytes.starts_with(PNG_MAGIC) {
        return Err("not a PNG".to_owned());
    }
    let mut offset = PNG_MAGIC.len();
    let mut first = true;
    // 0＝還沒看到 IDAT、1＝正在 IDAT 段、2＝IDAT 段已結束
    let mut idat_phase = 0u8;
    loop {
        if bytes.len() - offset < 12 {
            return Err("chunk header truncated or missing IEND".to_owned());
        }
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let end = offset
            .checked_add(12)
            .and_then(|end| end.checked_add(length))
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| "chunk runs past end of file".to_owned())?;
        let kind: [u8; 4] = bytes[offset + 4..offset + 8].try_into().unwrap();
        let stored = u32::from_be_bytes(bytes[end - 4..end].try_into().unwrap());
        if crc32(&bytes[offset + 4..end - 4]) != stored {
            return Err(format!(
                "bad CRC in {} chunk",
                String::from_utf8_lossy(&kind)
            ));
        }
        if first != (&kind == b"IHDR") {
            return Err("IHDR must be the first and only header chunk".to_owned());
        }
        first = false;
        idat_phase = match (&kind == b"IDAT", idat_phase) {
            (true, 2) => return Err("IDAT chunks are not contiguous".to_owned()),
            (true, _) => 1,
            (false, 1) => 2,
            (false, phase) => phase,
        };
        let chunk = PngChunk {
            kind,
            data: &bytes[offset + 8..end - 4],
            start: offset,
            end,
        };
        if &kind == b"IEND" {
            if !chunk.data.is_empty() {
                return Err("IEND is not empty".to_owned());
            }
            if idat_phase == 0 {
                return Err("no IDAT chunk".to_owned());
            }
            if end != bytes.len() {
                return Err("data after IEND".to_owned());
            }
            return visit(&chunk);
        }
        visit(&chunk)?;
        offset = end;
    }
}

struct Header {
    width: u32,
    height: u32,
    color_type: u8,
    depth: u8,
    /// 每像素位元數
    bits_per_pixel: u64,
    interlaced: bool,
}

fn parse_ihdr(data: &[u8]) -> Result<Header, String> {
    if data.len() != 13 {
        return Err("IHDR length is not 13".to_owned());
    }
    let width = u32::from_be_bytes(data[0..4].try_into().unwrap());
    let height = u32::from_be_bytes(data[4..8].try_into().unwrap());
    let (depth, color_type) = (data[8], data[9]);
    let channels: u64 = match (color_type, depth) {
        (0, 1 | 2 | 4 | 8 | 16) => 1,
        (2, 8 | 16) => 3,
        (3, 1 | 2 | 4 | 8) => 1,
        (4, 8 | 16) => 2,
        (6, 8 | 16) => 4,
        _ => {
            return Err(format!(
                "invalid color type {color_type} / bit depth {depth}"
            ))
        }
    };
    if data[10] != 0 || data[11] != 0 || data[12] > 1 {
        return Err("invalid compression, filter or interlace method".to_owned());
    }
    Ok(Header {
        width,
        height,
        color_type,
        depth,
        bits_per_pixel: channels * u64::from(depth),
        interlaced: data[12] == 1,
    })
}

/// 一段掃描線（非交錯圖一段、交錯圖 7 個 pass 中非空的那幾段）。
#[derive(Clone, Copy)]
struct Pass {
    rows: u64,
    /// 每列位元組（含濾波器位元組）
    row_bytes: u64,
    /// 每列像素數
    width: u64,
}

/// IHDR 推算的各段列數、每列位元組與像素數。
fn scanline_passes(header: &Header) -> Vec<Pass> {
    let pass = |width: u64, rows: u64| Pass {
        rows,
        row_bytes: (width * header.bits_per_pixel).div_ceil(8) + 1,
        width,
    };
    let (width, height) = (u64::from(header.width), u64::from(header.height));
    if !header.interlaced {
        return vec![pass(width, height)];
    }
    const ADAM7: [(u64, u64, u64, u64); 7] = [
        (0, 0, 8, 8),
        (4, 0, 8, 8),
        (0, 4, 4, 8),
        (2, 0, 4, 4),
        (0, 2, 2, 4),
        (1, 0, 2, 2),
        (0, 1, 1, 2),
    ];
    ADAM7
        .iter()
        .filter_map(|&(x0, y0, dx, dy)| {
            let pass_width = if width > x0 {
                (width - x0).div_ceil(dx)
            } else {
                0
            };
            let pass_height = if height > y0 {
                (height - y0).div_ceil(dy)
            } else {
                0
            };
            (pass_width > 0 && pass_height > 0).then(|| pass(pass_width, pass_height))
        })
        .collect()
}

/// 索引色圖的核對資料：PLTE 筆數與位元深度，以及逐列反濾波用的本列／上一列。
struct PaletteCheck {
    entries: usize,
    depth: u8,
    row: Vec<u8>,
    prev: Vec<u8>,
}

impl PaletteCheck {
    /// 一整列（含濾波器位元組）到齊：反濾波（索引色每像素 1 位元組以內，左鄰距離固定 1）後
    /// 逐像素取索引，必須小於 PLTE 筆數。
    fn finish_row(&mut self, width: u64) -> Result<(), String> {
        let filter = self.row[0];
        let data = &mut self.row[1..];
        for i in 0..data.len() {
            let left = if i > 0 { data[i - 1] } else { 0 };
            let up = self.prev.get(i).copied().unwrap_or(0);
            let up_left = if i > 0 {
                self.prev.get(i - 1).copied().unwrap_or(0)
            } else {
                0
            };
            let predicted = match filter {
                0 => 0,
                1 => left,
                2 => up,
                3 => ((u16::from(left) + u16::from(up)) / 2) as u8,
                _ => paeth(left, up, up_left),
            };
            data[i] = data[i].wrapping_add(predicted);
        }
        let depth = u64::from(self.depth);
        let mask = (1u16 << self.depth) - 1;
        for pixel in 0..width {
            let bit = pixel * depth;
            let byte = data[(bit / 8) as usize];
            let shift = 8 - depth - bit % 8;
            let index = usize::from((u16::from(byte) >> shift) & mask);
            if index >= self.entries {
                return Err(format!(
                    "palette index {index} out of range ({} entries)",
                    self.entries
                ));
            }
        }
        self.prev.clear();
        self.prev.extend_from_slice(data);
        self.row.clear();
        Ok(())
    }
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = i16::from(a) + i16::from(b) - i16::from(c);
    let (pa, pb, pc) = (
        (p - i16::from(a)).abs(),
        (p - i16::from(b)).abs(),
        (p - i16::from(c)).abs(),
    );
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// 解壓出來的原始掃描線：逐位元組檢查每列開頭的濾波器位元組，總長不得超過 IHDR 推算值；
/// 索引色圖另外逐列反濾波核對索引。
struct ScanlineCheck {
    passes: Vec<Pass>,
    pass: usize,
    rows_left: u64,
    in_row: u64,
    palette: Option<PaletteCheck>,
}

impl ScanlineCheck {
    fn new(passes: Vec<Pass>) -> Self {
        let rows_left = passes.first().map_or(0, |pass| pass.rows);
        Self {
            passes,
            pass: 0,
            rows_left,
            in_row: 0,
            palette: None,
        }
    }

    fn feed(&mut self, mut data: &[u8]) -> Result<(), String> {
        while !data.is_empty() {
            let Some(&Pass {
                row_bytes, width, ..
            }) = self.passes.get(self.pass)
            else {
                return Err("image data longer than IHDR allows".to_owned());
            };
            if self.in_row == 0 && data[0] > 4 {
                return Err(format!("invalid filter type {}", data[0]));
            }
            let take = (row_bytes - self.in_row).min(data.len() as u64) as usize;
            if let Some(palette) = self.palette.as_mut() {
                palette.row.extend_from_slice(&data[..take]);
            }
            self.in_row += take as u64;
            data = &data[take..];
            if self.in_row == row_bytes {
                if let Some(palette) = self.palette.as_mut() {
                    palette.finish_row(width)?;
                }
                self.in_row = 0;
                self.rows_left -= 1;
                if self.rows_left == 0 {
                    self.pass += 1;
                    self.rows_left = self.passes.get(self.pass).map_or(0, |pass| pass.rows);
                    // 交錯圖每個 pass 自成一張小圖：上一列從零開始
                    if let Some(palette) = self.palette.as_mut() {
                        palette.prev.clear();
                    }
                }
            }
        }
        Ok(())
    }

    fn complete(&self) -> bool {
        self.pass == self.passes.len()
    }
}

/// IDAT 串成的 zlib 流：逐段餵進解壓器、固定緩衝輸出，必須在最後一段剛好結束（StreamEnd、
/// 沒有剩餘輸入），Adler-32 由解壓器核對。
struct ZlibCheck {
    inflater: Decompress,
    buffer: Vec<u8>,
    ended: bool,
    scanlines: ScanlineCheck,
}

impl ZlibCheck {
    fn feed(&mut self, mut input: &[u8]) -> Result<(), String> {
        while !input.is_empty() {
            if self.ended {
                return Err("data after end of zlib stream".to_owned());
            }
            let (before_in, before_out) = (self.inflater.total_in(), self.inflater.total_out());
            let status = self
                .inflater
                .decompress(input, &mut self.buffer, FlushDecompress::None)
                .map_err(|error| format!("zlib: {error}"))?;
            let consumed = (self.inflater.total_in() - before_in) as usize;
            let produced = (self.inflater.total_out() - before_out) as usize;
            self.scanlines.feed(&self.buffer[..produced])?;
            input = &input[consumed..];
            match status {
                Status::StreamEnd => self.ended = true,
                _ if consumed == 0 && produced == 0 => {
                    return Err("zlib stream made no progress".to_owned())
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// 輸入用完後還有待輸出的資料：持續抽到解壓器沒有進展為止。
    fn finish(&mut self) -> Result<(), String> {
        while !self.ended {
            let before_out = self.inflater.total_out();
            let status = self
                .inflater
                .decompress(&[], &mut self.buffer, FlushDecompress::Finish)
                .map_err(|error| format!("zlib: {error}"))?;
            let produced = (self.inflater.total_out() - before_out) as usize;
            self.scanlines.feed(&self.buffer[..produced])?;
            if status == Status::StreamEnd {
                self.ended = true;
            } else if produced == 0 {
                return Err("zlib stream truncated".to_owned());
            }
        }
        if !self.scanlines.complete() {
            return Err("image data shorter than IHDR requires".to_owned());
        }
        Ok(())
    }
}

/// 圖片嚴格驗證：結構＋IHDR 組合＋尺寸上限＋PLTE 規則＋整條 zlib 流與掃描線。回傳寬高。
pub(crate) fn validate_png_image(bytes: &[u8], limits: ImageLimits) -> Result<(u32, u32), String> {
    validate_inner(bytes, limits, &mut None)
}

/// 同 validate_png_image，但分出動畫與過大（呼叫端可剝動畫、縮圖），其餘歸壞檔。
pub(crate) fn classify_png_image(
    bytes: &[u8],
    limits: ImageLimits,
) -> Result<(u32, u32), PngReject> {
    let mut kind = None;
    validate_inner(bytes, limits, &mut kind)
        .map_err(|detail| kind.unwrap_or(PngReject::Invalid(detail)))
}

fn validate_inner(
    bytes: &[u8],
    limits: ImageLimits,
    kind: &mut Option<PngReject>,
) -> Result<(u32, u32), String> {
    let mut header: Option<Header> = None;
    let mut zlib: Option<ZlibCheck> = None;
    let mut palette: Option<usize> = None;
    for_each_chunk(bytes, |chunk| {
        match &chunk.kind {
            b"IHDR" => {
                let parsed = parse_ihdr(chunk.data)?;
                let (width, height) = (parsed.width, parsed.height);
                if width == 0 || height == 0 {
                    return Err(format!("image size {width}x{height} out of range"));
                }
                if width > limits.max_side || height > limits.max_side {
                    *kind = Some(PngReject::TooLarge { width, height });
                    return Err(format!("image size {width}x{height} out of range"));
                }
                if u64::from(width) * u64::from(height) > limits.max_pixels {
                    *kind = Some(PngReject::TooLarge { width, height });
                    return Err(format!("image {width}x{height} has too many pixels"));
                }
                zlib = Some(ZlibCheck {
                    inflater: Decompress::new(true),
                    buffer: vec![0; 64 * 1024],
                    ended: false,
                    scanlines: ScanlineCheck::new(scanline_passes(&parsed)),
                });
                header = Some(parsed);
            }
            b"PLTE" => {
                let header = header.as_ref().expect("IHDR comes first");
                if palette.is_some()
                    || zlib
                        .as_ref()
                        .is_some_and(|zlib| zlib.inflater.total_in() > 0)
                {
                    return Err("PLTE duplicated or after IDAT".to_owned());
                }
                if matches!(header.color_type, 0 | 4) {
                    return Err("PLTE not allowed for grayscale".to_owned());
                }
                if chunk.data.is_empty() || chunk.data.len() % 3 != 0 || chunk.data.len() > 768 {
                    return Err("invalid PLTE length".to_owned());
                }
                let entries = chunk.data.len() / 3;
                if header.color_type == 3 && entries > 1 << header.depth {
                    return Err(format!(
                        "PLTE has {entries} entries, more than bit depth {} allows",
                        header.depth
                    ));
                }
                palette = Some(entries);
            }
            b"IDAT" => {
                let header = header.as_ref().expect("IHDR comes first");
                let zlib = zlib.as_mut().expect("IHDR comes first");
                if header.color_type == 3 {
                    let Some(entries) = palette else {
                        return Err("palette image without PLTE".to_owned());
                    };
                    zlib.scanlines.palette.get_or_insert_with(|| PaletteCheck {
                        entries,
                        depth: header.depth,
                        row: Vec::new(),
                        prev: Vec::new(),
                    });
                }
                zlib.feed(chunk.data)?;
            }
            b"acTL" | b"fcTL" | b"fdAT" => {
                *kind = Some(PngReject::Animated);
                return Err(ANIMATED.to_owned());
            }
            kind if kind[0].is_ascii_uppercase() && !matches!(kind, b"IEND") => {
                return Err(format!(
                    "unknown critical chunk {}",
                    String::from_utf8_lossy(kind)
                ));
            }
            _ => {}
        }
        Ok(())
    })?;
    zlib.as_mut().expect("IHDR comes first").finish()?;
    let header = header.expect("IHDR comes first");
    Ok((header.width, header.height))
}

#[cfg(test)]
pub(crate) mod test_png {
    use crate::import::card_io::{png_chunk, PNG_MAGIC};
    use std::io::Write;

    /// 組一張 PNG：給 IHDR 欄位與尚未壓縮的掃描線（每列含濾波器位元組）。
    pub(crate) fn assemble(ihdr: [u8; 13], raw: &[u8]) -> Vec<u8> {
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(raw).unwrap();
        let idat = encoder.finish().unwrap();
        let mut out = PNG_MAGIC.to_vec();
        out.extend_from_slice(&png_chunk(b"IHDR", &ihdr));
        out.extend_from_slice(&png_chunk(b"IDAT", &idat));
        out.extend_from_slice(&png_chunk(b"IEND", &[]));
        out
    }

    pub(crate) fn rgba_ihdr(width: u32, height: u32) -> [u8; 13] {
        let mut ihdr = [0u8; 13];
        ihdr[0..4].copy_from_slice(&width.to_be_bytes());
        ihdr[4..8].copy_from_slice(&height.to_be_bytes());
        ihdr[8] = 8;
        ihdr[9] = 6;
        ihdr
    }

    /// 測試用真 PNG：width×height RGBA，像素依座標變化。
    pub(crate) fn real_png(width: u32, height: u32) -> Vec<u8> {
        let mut raw = Vec::new();
        for y in 0..height {
            raw.push(0);
            for x in 0..width * 4 {
                raw.push(((x * 31 + y * 7) % 251) as u8);
            }
        }
        assemble(rgba_ihdr(width, height), &raw)
    }
}

#[cfg(test)]
mod tests;
