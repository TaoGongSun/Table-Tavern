//! Grok 臉框的看圖輸入：Grok 看整張直式立繪回的座標整體偏移、每次落點也飄（2026-11-14 實測），
//! 改送頂端正方形（頭一定在裡面）放大到 768，疊洋紅格線與 0–1000 刻度讓它對著格線讀座標。
//! 專案沒有字型依賴，刻度用 3×5 點陣數字畫。
use super::compose::BACKDROP;
use image::imageops::{self, FilterType};
use image::{Rgba, RgbaImage};

const SIDE: u32 = 768;
const INK: Rgba<u8> = Rgba([255, 0, 255, 255]);
/// 0–9 的 3×5 點陣，每列 3 位元（高位在左）
const DIGITS: [[u8; 5]; 10] = [
    [7, 5, 5, 5, 7],
    [2, 6, 2, 2, 7],
    [7, 1, 7, 4, 7],
    [7, 1, 7, 1, 7],
    [5, 5, 7, 1, 1],
    [7, 4, 7, 1, 7],
    [7, 4, 7, 5, 7],
    [7, 1, 1, 1, 1],
    [7, 5, 7, 5, 7],
    [7, 5, 7, 1, 7],
];
/// 點陣放大倍數：一個數字 6×10 px
const DOT: u32 = 2;

/// 頂端正方形（邊長＝圖寬）的看圖輸入；回 (圖, 正方形在原圖的高)，臉框照這個高換算回原圖
pub(crate) fn top_square_with_grid(body: &RgbaImage) -> (RgbaImage, u32) {
    let side = body.width().min(body.height());
    let top = imageops::crop_imm(body, 0, 0, body.width(), side).to_image();
    let mut out = RgbaImage::from_pixel(SIDE, SIDE, BACKDROP);
    imageops::overlay(
        &mut out,
        &imageops::resize(&top, SIDE, SIDE, FilterType::Lanczos3),
        0,
        0,
    );
    for step in 1..10 {
        let at = step * SIDE / 10;
        for i in 0..SIDE {
            out.put_pixel(at, i, INK);
            out.put_pixel(i, at, INK);
        }
        let text = (step * 100).to_string();
        label(&mut out, &text, at + 3, 3);
        label(&mut out, &text, 3, at + 3);
    }
    (out, side)
}

fn label(image: &mut RgbaImage, text: &str, left: u32, top: u32) {
    for (index, digit) in text.bytes().enumerate() {
        let glyph = DIGITS[usize::from(digit - b'0')];
        let origin = left + index as u32 * 4 * DOT;
        for (row, bits) in glyph.iter().enumerate() {
            for column in 0..3 {
                if bits >> (2 - column) & 1 == 0 {
                    continue;
                }
                for dy in 0..DOT {
                    for dx in 0..DOT {
                        image.put_pixel(
                            origin + column * DOT + dx,
                            top + row as u32 * DOT + dy,
                            INK,
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 只取頂端正方形、輸出固定 768；格線落在每 1/10 處
    #[test]
    fn grid_input_is_the_top_square_with_lines_every_tenth() {
        let mut body = RgbaImage::from_pixel(200, 500, Rgba([0, 0, 255, 255]));
        for y in 200..500 {
            for x in 0..200 {
                body.put_pixel(x, y, Rgba([0, 255, 0, 255]));
            }
        }
        let (image, side) = top_square_with_grid(&body);
        assert_eq!(side, 200);
        assert_eq!(image.dimensions(), (SIDE, SIDE));
        assert_eq!(*image.get_pixel(SIDE / 10, 500), INK);
        assert_eq!(*image.get_pixel(500, SIDE * 9 / 10), INK);
        // 正方形以下（綠）不進來
        assert!(image.pixels().all(|pixel| pixel[1] < 200));
    }
}
