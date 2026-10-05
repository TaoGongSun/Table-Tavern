use super::test_png::{assemble, real_png, rgba_ihdr};
use super::*;
use crate::import::card_io::png_chunk;

const LIMITS: ImageLimits = ImageLimits {
    max_side: 64,
    max_pixels: 2048,
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

fn rebuild(chunks: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut out = PNG_MAGIC.to_vec();
    for (kind, data) in chunks {
        out.extend_from_slice(&png_chunk(kind, data));
    }
    out
}

fn idat_index(chunks: &Chunks) -> usize {
    chunks.iter().position(|(kind, _)| kind == b"IDAT").unwrap()
}

fn rejects(bytes: &[u8], needle: &str) {
    let error = validate_png_image(bytes, LIMITS).unwrap_err();
    assert!(error.contains(needle), "{error} 應含 {needle}");
}

#[test]
fn real_png_passes_and_reports_size() {
    assert_eq!(validate_png_image(&real_png(16, 8), LIMITS), Ok((16, 8)));
}

#[test]
fn structural_violations_are_rejected() {
    let good = chunks_of(&real_png(16, 8));

    let no_idat: Vec<_> = good
        .iter()
        .filter(|(kind, _)| kind != b"IDAT")
        .cloned()
        .collect();
    rejects(&rebuild(&no_idat), "no IDAT");

    let mut iend_data = good.clone();
    iend_data.last_mut().unwrap().1 = vec![1];
    rejects(&rebuild(&iend_data), "IEND is not empty");

    let mut trailing = real_png(16, 8);
    trailing.push(0);
    rejects(&trailing, "data after IEND");

    let mut two_iend = real_png(16, 8);
    two_iend.extend_from_slice(&png_chunk(b"IEND", &[]));
    rejects(&two_iend, "data after IEND");

    let mut no_iend = rebuild(&good[..good.len() - 1]);
    rejects(&no_iend, "missing IEND");
    no_iend.truncate(no_iend.len() - 3);
    rejects(&no_iend, "past end");

    let mut bad_crc = real_png(16, 8);
    let at = bad_crc.len() - 20;
    bad_crc[at] ^= 0xff;
    rejects(&bad_crc, "bad CRC");

    let mut ihdr_late = good.clone();
    ihdr_late.swap(0, 1);
    rejects(&rebuild(&ihdr_late), "IHDR");

    let mut two_ihdr = good.clone();
    two_ihdr.insert(1, good[0].clone());
    rejects(&rebuild(&two_ihdr), "IHDR");

    // IDAT 拆兩段、中間夾一個 tEXt
    let at = idat_index(&good);
    let data = good[at].1.clone();
    let mut split = good.clone();
    split[at].1 = data[..data.len() / 2].to_vec();
    split.insert(at + 1, (*b"tEXt", b"k\0v".to_vec()));
    split.insert(at + 2, (*b"IDAT", data[data.len() / 2..].to_vec()));
    rejects(&rebuild(&split), "not contiguous");

    let mut critical = good.clone();
    critical.insert(1, (*b"XXXX", vec![]));
    rejects(&rebuild(&critical), "unknown critical chunk");
}

#[test]
fn ihdr_combinations_and_sizes() {
    let raw_for = |ihdr: [u8; 13], row: usize| {
        let height = u32::from_be_bytes(ihdr[4..8].try_into().unwrap()) as usize;
        assemble(ihdr, &vec![0; (row + 1) * height])
    };
    let mut zero_width = rgba_ihdr(0, 4);
    rejects(&raw_for(zero_width, 0), "out of range");
    zero_width[0..4].copy_from_slice(&4u32.to_be_bytes());

    let mut depth3 = rgba_ihdr(4, 4);
    depth3[8] = 3;
    rejects(&raw_for(depth3, 16), "bit depth");

    let mut bad_method = rgba_ihdr(4, 4);
    bad_method[10] = 1;
    rejects(&raw_for(bad_method, 16), "compression");

    rejects(&real_png(65, 1), "out of range");
    rejects(&real_png(64, 64), "too many pixels");
}

#[test]
fn zlib_stream_must_end_exactly_with_valid_checksum() {
    let good = chunks_of(&real_png(16, 8));
    let at = idat_index(&good);

    // 尾端截掉 1–4 bytes（Adler-32 的一部分）
    for cut in 1..=4 {
        let mut truncated = good.clone();
        let len = truncated[at].1.len();
        truncated[at].1.truncate(len - cut);
        rejects(&rebuild(&truncated), "zlib");
    }

    // Adler-32 錯
    let mut adler = good.clone();
    let last = adler[at].1.len() - 1;
    adler[at].1[last] ^= 0x01;
    rejects(&rebuild(&adler), "zlib");

    // 壓縮流後面多塞資料
    let mut extra = good.clone();
    extra[at].1.extend_from_slice(&[0, 0]);
    rejects(&rebuild(&extra), "after end of zlib stream");

    // 壞壓縮流
    let mut garbage = good.clone();
    garbage[at].1 = vec![0x78, 0x9c, 0xff, 0xff, 0xff, 0xff];
    rejects(&rebuild(&garbage), "zlib");

    // 截半
    let mut half = good.clone();
    let len = half[at].1.len();
    half[at].1.truncate(len / 2);
    rejects(&rebuild(&half), "zlib");
}

#[test]
fn scanlines_must_match_ihdr() {
    let ihdr = rgba_ihdr(4, 2);
    let row = 4 * 4 + 1;
    assert!(validate_png_image(&assemble(ihdr, &vec![0; row * 2]), LIMITS).is_ok());
    rejects(&assemble(ihdr, &vec![0; row * 2 - 1]), "shorter");
    rejects(&assemble(ihdr, &vec![0; row * 2 + 1]), "longer");
    let mut bad_filter = vec![0; row * 2];
    bad_filter[row] = 5;
    rejects(&assemble(ihdr, &bad_filter), "filter type");

    // 交錯圖：7 個 pass 的列長加總
    let mut interlaced = rgba_ihdr(5, 3);
    interlaced[12] = 1;
    // pass 寬×高：(1×1)(1×1)(2×0 跳過)(1×1)(3×1)(2×2)(5×1)
    let raw_len = (4 + 1) + (4 + 1) + (4 + 1) + (12 + 1) + 2 * (8 + 1) + (20 + 1);
    assert!(validate_png_image(&assemble(interlaced, &vec![0; raw_len]), LIMITS).is_ok());
    rejects(&assemble(interlaced, &vec![0; raw_len - 1]), "shorter");
}

#[test]
fn palette_rules() {
    let mut ihdr = rgba_ihdr(2, 2);
    ihdr[9] = 3; // palette, 8-bit
    let raw = vec![0u8; (2 + 1) * 2];
    let without = assemble(ihdr, &raw);
    rejects(&without, "without PLTE");
    let mut chunks = chunks_of(&without);
    chunks.insert(1, (*b"PLTE", vec![0; 6]));
    assert!(validate_png_image(&rebuild(&chunks), LIMITS).is_ok());
    chunks[1].1 = vec![0; 7];
    rejects(&rebuild(&chunks), "PLTE length");
    let gray = chunks_of(&assemble(rgba_ihdr(2, 2), &vec![0; 9 * 2]));
    let mut gray = gray;
    gray[0].1[9] = 0;
    gray[0].1[8] = 8;
    gray.insert(1, (*b"PLTE", vec![0; 3]));
    rejects(&rebuild(&gray), "grayscale");
}

#[test]
fn many_small_chunks_stream_without_a_chunk_table() {
    let mut chunks = chunks_of(&real_png(4, 4));
    let filler: Chunks = (0..100_000).map(|_| (*b"zzZz", Vec::new())).collect();
    chunks.splice(1..1, filler);
    assert!(validate_png_image(&rebuild(&chunks), LIMITS).is_ok());
}

/// 索引色圖：給位元深度、寬高、交錯、PLTE 筆數與原始掃描線（含濾波器位元組）。
pub(crate) fn palette_png(
    depth: u8,
    size: (u32, u32),
    interlaced: bool,
    entries: usize,
    raw: &[u8],
) -> Vec<u8> {
    let mut ihdr = rgba_ihdr(size.0, size.1);
    ihdr[8] = depth;
    ihdr[9] = 3;
    ihdr[12] = u8::from(interlaced);
    let mut chunks = chunks_of(&assemble(ihdr, raw));
    chunks.insert(1, (*b"PLTE", vec![0; entries * 3]));
    rebuild(&chunks)
}

#[test]
fn palette_entries_must_fit_bit_depth() {
    // Sol 反例①：1-bit 索引色配 3 筆 palette
    rejects(
        &palette_png(1, (8, 1), false, 3, &[0, 0]),
        "more than bit depth",
    );
    assert!(validate_png_image(&palette_png(1, (8, 1), false, 2, &[0, 0]), LIMITS).is_ok());
}

#[test]
fn palette_indices_must_be_in_range_after_unfiltering() {
    // Sol 反例②：8-bit 只有 1 筆 palette，像素引用 index=1
    rejects(
        &palette_png(8, (2, 1), false, 1, &[0, 0, 1]),
        "palette index 1 out of range",
    );
    // 濾波還原後才越界：第二列 Up（2）把 [0,1] 加上上一列 [0,1] → [0,2]
    rejects(
        &palette_png(8, (2, 2), false, 2, &[0, 0, 1, 2, 0, 1]),
        "palette index 2",
    );
    // 還原後在範圍內：第二列 Sub（1）[1,0] → [1,1]；Average、Paeth 也走過
    assert!(validate_png_image(
        &palette_png(8, (2, 4), false, 2, &[0, 0, 1, 1, 1, 0, 3, 0, 0, 4, 0, 0]),
        LIMITS
    )
    .is_ok());
    // 2-bit 打包：第三個像素是 3，palette 只有 3 筆
    rejects(
        &palette_png(2, (3, 1), false, 3, &[0, 0b0001_1100]),
        "palette index 3",
    );
    assert!(
        validate_png_image(&palette_png(2, (3, 1), false, 3, &[0, 0b0001_1000]), LIMITS).is_ok()
    );
}

#[test]
fn interlaced_palette_rows_are_checked_per_pass() {
    // 5×3 交錯 8-bit：7 個 pass 的列長（含濾波器位元組）2,2,2,4,3+3,6
    let mut raw = vec![0u8; 2 + 2 + 2 + 4 + 2 * 3 + 6];
    assert!(validate_png_image(&palette_png(8, (5, 3), true, 2, &raw), LIMITS).is_ok());
    let last = raw.len() - 1;
    raw[last] = 5;
    rejects(&palette_png(8, (5, 3), true, 2, &raw), "palette index 5");
}

#[test]
fn animated_png_chunks_are_refused() {
    let mut chunks = chunks_of(&real_png(4, 4));
    chunks.insert(1, (*b"acTL", vec![0, 0, 0, 1, 0, 0, 0, 0]));
    rejects(&rebuild(&chunks), "animated");
}

/// 回歸：每個 Adam7 pass 的第一列用 Up 濾波，上一列必須從零開始（沿用上一個 pass 的
/// 最後一列會把索引加成 2，超出 2 筆 palette）。
#[test]
fn up_filter_does_not_carry_across_adam7_passes() {
    let raw: Vec<u8> = [
        vec![2, 1],             // pass 1：1×1
        vec![2, 1],             // pass 2：1×1
        vec![2, 1],             // pass 4：1×1
        vec![2, 1, 1, 1],       // pass 5：3×1
        vec![2, 1, 1],          // pass 6 第一列：2×2
        vec![0, 1, 1],          // pass 6 第二列（不再用 Up，免得同 pass 內累加）
        vec![2, 1, 1, 1, 1, 1], // pass 7：5×1
    ]
    .concat();
    assert!(validate_png_image(&palette_png(8, (5, 3), true, 2, &raw), LIMITS).is_ok());
}

/// 回歸：解壓輸出跨過 64 KiB 緩衝邊界時，列與像素索引照樣接得上。
#[test]
fn rows_spanning_the_64k_output_buffer_are_checked() {
    let wide = ImageLimits {
        max_side: 1024,
        max_pixels: 1 << 20,
    };
    assert_eq!(
        validate_png_image(&real_png(200, 100), wide),
        Ok((200, 100))
    );
    let (width, height) = (300usize, 300usize);
    let mut raw = vec![0u8; (width + 1) * height];
    assert!(validate_png_image(&palette_png(8, (300, 300), false, 2, &raw), wide).is_ok());
    let last = raw.len() - 1;
    raw[last] = 9;
    assert!(
        validate_png_image(&palette_png(8, (300, 300), false, 2, &raw), wide)
            .unwrap_err()
            .contains("palette index 9")
    );
}
