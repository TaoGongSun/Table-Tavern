use super::*;
use crate::import::png_image::test_png::{assemble, real_png, rgba_ihdr};
use crate::import::png_image::validate_png_image;
use std::io::Write;

const SMALL: CleanLimits = CleanLimits {
    stored: ImageLimits {
        max_side: 64,
        max_pixels: 2048,
    },
    source: ImageLimits {
        max_side: 256,
        max_pixels: 16_384,
    },
};

type Chunks = Vec<([u8; 4], Vec<u8>)>;

pub(crate) fn chunks_of(bytes: &[u8]) -> Chunks {
    let mut out = Vec::new();
    for_each_chunk(bytes, |chunk| {
        out.push((chunk.kind, chunk.data.to_vec()));
        Ok(())
    })
    .unwrap();
    out
}

pub(crate) fn rebuild(chunks: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut out = PNG_MAGIC.to_vec();
    for (kind, data) in chunks {
        out.extend_from_slice(&png_chunk(kind, data));
    }
    out
}

fn zlib(raw: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(raw).unwrap();
    encoder.finish().unwrap()
}

/// 單色 RGBA 掃描線（每列含濾波器位元組）
fn solid_raw(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
    let mut raw = Vec::new();
    for _ in 0..height {
        raw.push(0);
        for _ in 0..width {
            raw.extend_from_slice(&rgba);
        }
    }
    raw
}

fn fctl(sequence: u32, width: u32, height: u32) -> Vec<u8> {
    let mut data = sequence.to_be_bytes().to_vec();
    data.extend_from_slice(&width.to_be_bytes());
    data.extend_from_slice(&height.to_be_bytes());
    data.extend_from_slice(&[0; 8]);
    data.extend_from_slice(&[0, 1, 0, 10, 0, 0]);
    data
}

pub(crate) const RED: [u8; 4] = [255, 0, 0, 255];
pub(crate) const BLUE: [u8; 4] = [0, 0, 255, 255];

/// APNG：IDAT 紅、fdAT 藍。`default_is_frame`＝IDAT 前有 fcTL（預設圖就是第一格）。
pub(crate) fn apng(default_is_frame: bool) -> Vec<u8> {
    let (width, height) = (4, 3);
    let mut chunks: Chunks = vec![(*b"IHDR", rgba_ihdr(width, height).to_vec())];
    let frames: u32 = if default_is_frame { 2 } else { 1 };
    let mut actl = frames.to_be_bytes().to_vec();
    actl.extend_from_slice(&0u32.to_be_bytes());
    chunks.push((*b"acTL", actl));
    let mut sequence = 0;
    if default_is_frame {
        chunks.push((*b"fcTL", fctl(sequence, width, height)));
        sequence += 1;
    }
    chunks.push((*b"IDAT", zlib(&solid_raw(width, height, RED))));
    chunks.push((*b"fcTL", fctl(sequence, width, height)));
    let mut fdat = (sequence + 1).to_be_bytes().to_vec();
    fdat.extend_from_slice(&zlib(&solid_raw(width, height, BLUE)));
    chunks.push((*b"fdAT", fdat));
    chunks.push((*b"IEND", Vec::new()));
    rebuild(&chunks)
}

pub(crate) fn first_pixel(png: &[u8]) -> [u8; 4] {
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png).unwrap();
    image.to_rgba8().get_pixel(0, 0).0
}

fn kinds(png: &[u8]) -> Vec<[u8; 4]> {
    chunks_of(png).into_iter().map(|(kind, _)| kind).collect()
}

pub(crate) fn with_trailing(mut png: Vec<u8>) -> Vec<u8> {
    png.extend_from_slice(b"TRAILING-JUNK\0\x01\x02");
    png
}

/// 把第 index 個 chunk 的 CRC 最後一個位元組翻掉
pub(crate) fn break_crc(png: &[u8], kind: &[u8; 4]) -> Vec<u8> {
    let mut out = png.to_vec();
    let mut offset = PNG_MAGIC.len();
    loop {
        let length = u32::from_be_bytes(out[offset..offset + 4].try_into().unwrap()) as usize;
        let end = offset + 12 + length;
        if &out[offset + 4..offset + 8] == kind {
            out[end - 1] ^= 1;
            return out;
        }
        offset = end;
    }
}

fn insert_before_idat(png: &[u8], kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut chunks = chunks_of(png);
    let index = chunks.iter().position(|(k, _)| k == b"IDAT").unwrap();
    chunks.insert(index, (*kind, data.to_vec()));
    rebuild(&chunks)
}

/// 8 位元索引色圖，掃描線全用 `index`
pub(crate) fn palette_png(width: u32, height: u32, entries: usize, index: u8) -> Vec<u8> {
    let mut ihdr = rgba_ihdr(width, height);
    ihdr[9] = 3;
    let mut raw = Vec::new();
    for _ in 0..height {
        raw.push(0);
        raw.extend(std::iter::repeat_n(index, width as usize));
    }
    let mut chunks = chunks_of(&assemble(ihdr, &raw));
    chunks.insert(1, (*b"PLTE", (0..entries * 3).map(|i| i as u8).collect()));
    rebuild(&chunks)
}

/// IDAT 內容換成 CRC 合法但 zlib 壞掉的資料
pub(crate) fn broken_zlib(png: &[u8]) -> Vec<u8> {
    let mut chunks = chunks_of(png);
    for (kind, data) in &mut chunks {
        if kind == b"IDAT" {
            let middle = data.len() / 2;
            data[middle] ^= 0xFF;
            data[middle + 1] ^= 0xFF;
        }
    }
    rebuild(&chunks)
}

fn encode(format: image::ImageFormat, image: &image::DynamicImage) -> Vec<u8> {
    let mut out = Vec::new();
    match format {
        image::ImageFormat::WebP => image
            .write_with_encoder(image::codecs::webp::WebPEncoder::new_lossless(&mut out))
            .unwrap(),
        format => image.write_to(&mut Cursor::new(&mut out), format).unwrap(),
    }
    out
}

fn size_of(png: &[u8]) -> (u32, u32) {
    validate_png_image(png, REENCODE_SOURCE_LIMITS).unwrap()
}

#[test]
fn fit_size_stays_within_both_limits_after_rounding() {
    let limits = STORED_IMAGE_LIMITS;
    assert_eq!(fit_size(4000, 6000, limits), (4000, 6000));
    assert_eq!(fit_size(6000, 4000, limits), (6000, 4000));
    for (width, height) in [
        (6000, 5000),
        (8192, 8192),
        (16384, 100),
        (100, 16384),
        (16384, 1),
        (1, 16384),
        (4899, 4899),
        (12345, 6789),
        (8193, 2929),
    ] {
        let (w, h) = fit_size(width, height, limits);
        assert!(w >= 1 && h >= 1, "{width}x{height} -> {w}x{h}");
        assert!(
            w <= limits.max_side && h <= limits.max_side,
            "{width}x{height} -> {w}x{h}"
        );
        assert!(
            u64::from(w) * u64::from(h) <= limits.max_pixels,
            "{width}x{height} -> {w}x{h}"
        );
    }
    // 剛好 24M 像素原樣保留
    assert_eq!(fit_size(6000, 4000, limits), (6000, 4000));
    assert_eq!(fit_size(100, 20, SMALL.stored), (64, 12));
}

#[test]
fn png_jpeg_and_webp_shrink_to_the_same_size() {
    let source = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(120, 40, |x, y| {
        image::Rgb([x as u8, y as u8, 7])
    }));
    let expected = fit_size(120, 40, SMALL.stored);
    for format in [
        image::ImageFormat::Png,
        image::ImageFormat::Jpeg,
        image::ImageFormat::WebP,
    ] {
        let out = reencode(&encode(format, &source), format, SMALL).unwrap();
        assert_eq!(size_of(&out), expected, "{format:?}");
        assert!(validate_png_image(&out, SMALL.stored).is_ok());
    }
}

#[test]
fn jpeg_and_webp_over_source_pixels_are_refused_before_decoding() {
    // 邊長在來源上限內（200 ≤ 256），像素數超過（200×100 > 16384）
    let gray = image::DynamicImage::ImageLuma8(image::GrayImage::new(200, 100));
    let jpeg = encode(image::ImageFormat::Jpeg, &gray);
    let error = reencode(&jpeg, image::ImageFormat::Jpeg, SMALL).unwrap_err();
    assert!(error.contains("too large to process"), "{error}");
    let rgba = image::DynamicImage::ImageRgba8(image::RgbaImage::new(200, 100));
    let webp = encode(image::ImageFormat::WebP, &rgba);
    let error = reencode(&webp, image::ImageFormat::WebP, SMALL).unwrap_err();
    assert!(error.contains("too large to process"), "{error}");
}

#[test]
fn extremely_wide_palette_png_is_refused_at_ihdr() {
    let mut ihdr = rgba_ihdr(100_000, 1);
    ihdr[9] = 3;
    // IDAT 不必是真資料：IHDR 就擋下，不配置列緩衝、不解壓
    let png = rebuild(&[
        (*b"IHDR", ihdr.to_vec()),
        (*b"PLTE", vec![0; 6]),
        (*b"IDAT", zlib(&[0; 16])),
        (*b"IEND", Vec::new()),
    ]);
    let error = strict_stored_png(&png).unwrap_err();
    assert!(error.contains("too large"), "{error}");
    assert_eq!(salvage_stored_png(&png), None);
}

#[test]
fn oversized_png_is_shrunk_and_over_source_is_refused() {
    let big = real_png(100, 30);
    let Stored::Reencoded(out) = strict_with(&big, SMALL).unwrap() else {
        panic!("expected re-encode");
    };
    assert_eq!(size_of(&out), fit_size(100, 30, SMALL.stored));
    let too_big = real_png(300, 2);
    assert!(strict_with(&too_big, SMALL).is_err());
    assert_eq!(salvage_with(&too_big, SMALL), None);
}

#[test]
fn valid_png_is_kept_as_is() {
    let png = real_png(16, 8);
    assert_eq!(strict_with(&png, SMALL), Ok(Stored::AsIs));
    assert_eq!(salvage_with(&png, SMALL), Some(Stored::AsIs));
}

#[test]
fn apng_keeps_the_idat_default_image_in_both_layouts() {
    for default_is_frame in [true, false] {
        let original = apng(default_is_frame);
        let Stored::Rewritten(out) = strict_with(&original, SMALL).unwrap() else {
            panic!("expected stripped bytes");
        };
        assert!(validate_png_image(&out, SMALL.stored).is_ok());
        let kinds = kinds(&out);
        assert!(!kinds.iter().any(|kind| is_animation(kind)), "{kinds:?}");
        assert_eq!(
            first_pixel(&out),
            RED,
            "default_is_frame={default_is_frame}"
        );
    }
}

#[test]
fn strict_refuses_broken_apngs() {
    // CRC 合法但 IDAT zlib 壞
    assert!(strict_with(&broken_zlib(&apng(true)), SMALL).is_err());
    // IDAT → fcTL → IDAT：剝掉 fcTL 會把不連續的 IDAT 洗成合法，必須在剝除前就擋
    let mut chunks = chunks_of(&real_png(4, 3));
    let idat = chunks.iter().position(|(kind, _)| kind == b"IDAT").unwrap();
    let data = chunks[idat].1.clone();
    let (head, tail) = data.split_at(data.len() / 2);
    chunks.splice(
        idat..=idat,
        [
            (*b"IDAT", head.to_vec()),
            (*b"fcTL", fctl(0, 4, 3)),
            (*b"IDAT", tail.to_vec()),
        ],
    );
    let mut with_actl = chunks.clone();
    with_actl.insert(1, (*b"acTL", vec![0, 0, 0, 1, 0, 0, 0, 0]));
    let error = strict_with(&rebuild(&with_actl), SMALL).unwrap_err();
    assert!(error.contains("not contiguous"), "{error}");
    // APNG 帶 IEND 後尾隨
    let error = strict_with(&with_trailing(apng(true)), SMALL).unwrap_err();
    assert!(error.contains("after IEND"), "{error}");
}

#[test]
fn strict_refuses_broken_pngs() {
    let png = real_png(16, 8);
    let truncated = png[..png.len() - 20].to_vec();
    let palette_out_of_range = palette_png(4, 3, 2, 5);
    for (name, bytes) in [
        ("truncated", truncated),
        ("bad IDAT CRC", break_crc(&png, b"IDAT")),
        ("trailing", with_trailing(png.clone())),
        ("palette out of range", palette_out_of_range),
        ("broken zlib", broken_zlib(&png)),
    ] {
        assert!(strict_with(&bytes, SMALL).is_err(), "{name}");
    }
}

#[test]
fn salvage_keeps_original_bytes_up_to_iend() {
    for png in [real_png(16, 8), palette_png(4, 3, 2, 1)] {
        let trailing = with_trailing(png.clone());
        assert_eq!(salvage_with(&trailing, SMALL), Some(Stored::Rewritten(png)));
    }
    // 截斷後是動畫：剝成靜態
    let Some(Stored::Rewritten(out)) = salvage_with(&with_trailing(apng(false)), SMALL) else {
        panic!("expected stripped bytes");
    };
    assert_eq!(first_pixel(&out), RED);
}

#[test]
fn salvage_reencodes_what_the_decoder_can_read() {
    let png = real_png(16, 8);
    let text_crc = break_crc(&insert_before_idat(&png, b"tEXt", b"note\0hi"), b"tEXt");
    let broken_apng = {
        // 結構壞的 APNG（acTL 在 IDAT 之後被嚴驗擋），預設圖解得開
        let mut chunks = chunks_of(&apng(false));
        let actl = chunks.remove(1);
        let iend = chunks.len() - 1;
        chunks.insert(iend, actl);
        let mut bytes = rebuild(&chunks);
        bytes = break_crc(&bytes, b"fdAT");
        bytes
    };
    for (name, bytes) in [
        ("tEXt bad CRC", text_crc),
        ("palette out of range", palette_png(4, 3, 2, 5)),
        ("broken apng", broken_apng),
    ] {
        let Some(Stored::Reencoded(out)) = salvage_with(&bytes, SMALL) else {
            panic!("{name}: expected re-encode");
        };
        assert!(validate_png_image(&out, SMALL.stored).is_ok(), "{name}");
    }
}

#[test]
fn salvage_gives_up_when_idat_is_broken() {
    let png = real_png(16, 8);
    assert_eq!(salvage_with(&break_crc(&png, b"IDAT"), SMALL), None);
    assert_eq!(salvage_with(&broken_zlib(&png), SMALL), None);
    assert_eq!(salvage_with(b"\x89PNG\r\n\x1a\nfirst", SMALL), None);
}

fn text(keyword: &str, value: &str) -> Vec<u8> {
    format!("{keyword}\0{value}").into_bytes()
}

#[test]
fn transplant_moves_card_text_in_order_and_stops_at_iend() {
    let mut chunks = chunks_of(&real_png(4, 3));
    chunks.insert(1, (*b"tEXt", text("ccv3", "V3")));
    chunks.insert(2, (*b"tEXt", text("note", "skip")));
    let iend = chunks.len() - 1;
    chunks.insert(iend, (*b"tEXt", text("chara", "V2")));
    let mut original = rebuild(&chunks);
    // IEND 後再一個 chara 與不完整的 chunk：都不搬
    original.extend_from_slice(&png_chunk(b"tEXt", &text("chara", "AFTER")));
    original.extend_from_slice(&[0, 0, 0, 9, b't']);
    let clean = real_png(2, 2);
    let out = transplant_card_text(&original, &clean).unwrap();
    assert!(validate_png_image(&out, SMALL.stored).is_ok());
    let texts: Vec<Vec<u8>> = chunks_of(&out)
        .into_iter()
        .filter(|(kind, _)| kind == b"tEXt")
        .map(|(_, data)| data)
        .collect();
    assert_eq!(texts, vec![text("ccv3", "V3"), text("chara", "V2")]);
}

#[test]
fn transplant_tolerates_broken_original_tail() {
    let mut chunks = chunks_of(&real_png(4, 3));
    chunks.insert(1, (*b"tEXt", text("chara", "V2")));
    let mut original = rebuild(&chunks);
    // 砍掉 IEND，留一段不完整的 chunk
    original.truncate(original.len() - 12);
    original.extend_from_slice(&[0, 0, 1, 0, b'I', b'D']);
    let out = transplant_card_text(&original, &real_png(2, 2)).unwrap();
    assert_eq!(
        chunks_of(&out)
            .into_iter()
            .filter(|(kind, _)| kind == b"tEXt")
            .count(),
        1
    );
}
