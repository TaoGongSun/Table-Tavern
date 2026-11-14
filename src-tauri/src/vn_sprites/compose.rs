//! 裁頭重繪的本地影像處理（plans/vn-mode-pkg7.md §4 第 3–4 步；數值是包 7 前補測值）：
//! 臉框每邊外擴 40% 取正方形（出界補透明）→ 放大到 1024 送出 → 回來須近正方形、縮回裁切大小 →
//! 對齊檢查（小位移平移補正）→ 取內層臉框、邊緣羽化 → 臉層。
use super::store::FaceBox;
use image::imageops::{self, FilterType};
use image::{Rgba, RgbaImage};

/// 送給模型的裁切邊長
const REQUEST_SIDE: u32 = 1024;
/// 臉框每邊外擴 40%：邊長＝臉框長邊 × 1.8
const CROP_SCALE: f64 = 1.8;
/// 回來的長寬比差上限
const ASPECT_TOLERANCE: f64 = 0.02;
const FEATHER_PX: f32 = 10.0;
/// 兩邊都不透明的像素：RGB 任一通道差超過這個算有變
const RGB_DIFF: i32 = 48;
const RGB_RATIO: f64 = 0.05;
/// 環帶有效像素不足時改比 alpha
const MIN_VALID: usize = 500;
const ALPHA_DIFF: i32 = 16;
const ALPHA_RATIO: f64 = 0.12;
const SHIFT_RANGE: i64 = 8;
/// 別處的分數要低到原位的這個比例以下才算真位移，否則當取樣雜訊
const DRIFT_GAIN: f64 = 0.95;
/// 真位移在這個範圍內平移補正，超過拒收
const NUDGE_MAX: i64 = 2;
const OPAQUE: u8 = 128;
/// 灰底來源的底色（跟定稿提示詞的淺灰底同一種）
pub(crate) const BACKDROP: Rgba<u8> = Rgba([217, 217, 217, 255]);

/// 裁切正方形在定稿上的位置（可能出界）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Crop {
    pub left: i64,
    pub top: i64,
    pub side: u32,
}

impl Crop {
    pub(crate) fn around(face: FaceBox) -> Self {
        let side = (f64::from(face.w.max(face.h)) * CROP_SCALE).round() as u32;
        let center_x = f64::from(face.x) + f64::from(face.w) / 2.0;
        let center_y = f64::from(face.y) + f64::from(face.h) / 2.0;
        Crop {
            left: (center_x - f64::from(side) / 2.0).round() as i64,
            top: (center_y - f64::from(side) / 2.0).round() as i64,
            side,
        }
    }

    /// 臉框在裁切內的位置
    pub(crate) fn inner(&self, face: FaceBox) -> FaceBox {
        FaceBox {
            x: (i64::from(face.x) - self.left) as u32,
            y: (i64::from(face.y) - self.top) as u32,
            w: face.w,
            h: face.h,
        }
    }
}

/// 從定稿取裁切，出界部分透明
pub(crate) fn cut(base: &RgbaImage, crop: Crop) -> RgbaImage {
    let mut out = RgbaImage::new(crop.side, crop.side);
    imageops::overlay(&mut out, base, -crop.left, -crop.top);
    out
}

/// 送出的參考圖：裁切放大到 1024；不出透明圖的來源先壓到淺灰底（跟定稿提示詞同一種底）
pub(crate) fn request_png(base: &RgbaImage, crop: Crop, flatten: bool) -> Result<Vec<u8>, String> {
    let mut piece = cut(base, crop);
    if flatten {
        let mut gray = RgbaImage::from_pixel(crop.side, crop.side, BACKDROP);
        imageops::overlay(&mut gray, &piece, 0, 0);
        piece = gray;
    }
    let scaled = imageops::resize(&piece, REQUEST_SIDE, REQUEST_SIDE, FilterType::Lanczos3);
    encode_png(&scaled)
}

/// 置中貼到 2:3 淺灰畫布（只補邊、不縮放）：改圖類工具照參考圖比例出圖，卡圖、封面不一定是直式
pub(crate) fn portrait_canvas(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let image = decode(bytes)?;
    let (width, height) = image.dimensions();
    let (canvas_w, canvas_h) = match u64::from(width) * 3 > u64::from(height) * 2 {
        true => (width, (width * 3).div_ceil(2)),
        false => ((height * 2).div_ceil(3), height),
    };
    let mut canvas = RgbaImage::from_pixel(canvas_w, canvas_h, BACKDROP);
    let (left, top) = ((canvas_w - width) / 2, (canvas_h - height) / 2);
    imageops::overlay(&mut canvas, &image, i64::from(left), i64::from(top));
    encode_png(&canvas)
}

/// 模型回來的圖：須近正方形（否則算位移），任意尺寸縮回裁切大小
pub(crate) fn fit_back(result: &RgbaImage, side: u32) -> Option<RgbaImage> {
    let (width, height) = result.dimensions();
    let ratio = f64::from(width) / f64::from(height.max(1));
    ((ratio - 1.0).abs() <= ASPECT_TOLERANCE)
        .then(|| imageops::resize(result, side, side, FilterType::Lanczos3))
}

fn in_box(face: FaceBox, x: u32, y: u32) -> bool {
    x >= face.x && x < face.x + face.w && y >= face.y && y < face.y + face.h
}

fn rgb_diff(a: &Rgba<u8>, b: &Rgba<u8>) -> i32 {
    (0..3)
        .map(|channel| (i32::from(a[channel]) - i32::from(b[channel])).abs())
        .max()
        .unwrap_or(0)
}

/// 對齊門檻：只比裁切內、臉框外那一圈。兩邊都不透明處 RGB 差 >48 的像素 ≤5%；±8 px 平移搜尋的最合處
/// 在 2 px 內就平移補正（回補正後的圖），更遠拒收。有效像素不足 500 時改比 alpha 差
/// （>16）≤12%。
pub(crate) fn aligned(before: &RgbaImage, after: &RgbaImage, inner: FaceBox) -> Option<RgbaImage> {
    settle(before, after, &|x, y| !in_box(inner, x, y), true).map(|shift| translate(after, shift))
}

/// 變體身體：比例跟定稿一樣才縮成定稿尺寸，再比臉框內（門檻同上）；過＝共用表情組，回縮好的身體
pub(crate) fn shared_body(
    base: &RgbaImage,
    variant: &RgbaImage,
    face: FaceBox,
) -> Option<RgbaImage> {
    let (width, height) = base.dimensions();
    let (other_width, other_height) = variant.dimensions();
    let ratio = (f64::from(other_width) / f64::from(other_height.max(1)))
        / (f64::from(width) / f64::from(height.max(1)));
    if (ratio - 1.0).abs() > ASPECT_TOLERANCE {
        return None;
    }
    let resized = imageops::resize(variant, width, height, FilterType::Lanczos3);
    let window =
        |image: &RgbaImage| imageops::crop_imm(image, face.x, face.y, face.w, face.h).to_image();
    settle(&window(base), &window(&resized), &|_, _| true, false).map(|_| resized)
}

/// 平移後的圖：`out(x, y) = image(x + dx, y + dy)`，出界透明
fn translate(image: &RgbaImage, (dx, dy): (i64, i64)) -> RgbaImage {
    if (dx, dy) == (0, 0) {
        return image.clone();
    }
    let mut out = RgbaImage::new(image.width(), image.height());
    imageops::overlay(&mut out, image, -dx, -dy);
    out
}

/// 環帶計數：(兩邊都不透明, 其中 RGB 有變, 環帶總數, alpha 有變)
fn ring_counts(
    before: &RgbaImage,
    after: &RgbaImage,
    ring: &impl Fn(u32, u32) -> bool,
) -> (usize, usize, usize, usize) {
    let (mut valid, mut changed, mut ring_total, mut alpha_changed) = (0, 0, 0, 0);
    for (x, y, pixel) in before.enumerate_pixels() {
        if !ring(x, y) {
            continue;
        }
        let other = after.get_pixel(x, y);
        ring_total += 1;
        if (i32::from(pixel[3]) - i32::from(other[3])).abs() > ALPHA_DIFF {
            alpha_changed += 1;
        }
        if pixel[3] > OPAQUE && other[3] > OPAQUE {
            valid += 1;
            if rgb_diff(pixel, other) > RGB_DIFF {
                changed += 1;
            }
        }
    }
    (valid, changed, ring_total, alpha_changed)
}

/// `after` 平移 (dx, dy) 後環帶的平均 RGB 差：每隔一格取樣，只算兩邊都不透明處
fn mean_diff(
    before: &RgbaImage,
    after: &RgbaImage,
    ring: &impl Fn(u32, u32) -> bool,
    (dx, dy): (i64, i64),
) -> Option<f64> {
    let (width, height) = before.dimensions();
    let (mut sum, mut count) = (0i64, 0i64);
    for y in (0..height).step_by(2) {
        for x in (0..width).step_by(2) {
            let (sx, sy) = (i64::from(x) + dx, i64::from(y) + dy);
            if !ring(x, y) || sx < 0 || sy < 0 || sx >= i64::from(width) || sy >= i64::from(height)
            {
                continue;
            }
            let (a, b) = (
                before.get_pixel(x, y),
                after.get_pixel(sx as u32, sy as u32),
            );
            if a[3] > OPAQUE && b[3] > OPAQUE {
                sum += i64::from(rgb_diff(a, b));
                count += 1;
            }
        }
    }
    (count > 0).then(|| sum as f64 / count as f64)
}

/// 對齊檢查本體：只比 `ring` 圈到的像素（兩張圖同尺寸的正方形或臉框）。回要補正的位移，`None`＝拒收。
/// 原位就是最合的位置（並列算過）＝照環帶比例判、位移 0。`nudge` 關＝別處更合一律拒收（變體身體）；
/// 開＝最合處超出 `NUDGE_MAX` 照樣拒收；範圍內贏不到 `DRIFT_GAIN` 當取樣雜訊仍算原位，明顯更合就以
/// 補正後的環帶比例判。
fn settle(
    before: &RgbaImage,
    after: &RgbaImage,
    ring: &impl Fn(u32, u32) -> bool,
    nudge: bool,
) -> Option<(i64, i64)> {
    let (valid, changed, ring_total, alpha_changed) = ring_counts(before, after, ring);
    if valid < MIN_VALID {
        return ((alpha_changed as f64) <= ALPHA_RATIO * ring_total.max(1) as f64)
            .then_some((0, 0));
    }
    let within = |changed: usize, valid: usize| changed as f64 <= RGB_RATIO * valid as f64;
    let in_place = || within(changed, valid).then_some((0, 0));
    let Some(still) = mean_diff(before, after, ring, (0, 0)) else {
        return in_place();
    };
    let best = (-SHIFT_RANGE..=SHIFT_RANGE)
        .flat_map(|dy| (-SHIFT_RANGE..=SHIFT_RANGE).map(move |dx| (dx, dy)))
        .filter(|shift| *shift != (0, 0))
        .filter_map(|shift| mean_diff(before, after, ring, shift).map(|score| (score, shift)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .filter(|(score, _)| *score < still);
    match best {
        None => in_place(),
        Some(_) if !nudge => None,
        Some((_, (dx, dy))) if dx.abs() > NUDGE_MAX || dy.abs() > NUDGE_MAX => None,
        Some((score, _)) if score > still * DRIFT_GAIN => in_place(),
        Some((_, shift)) => {
            let (valid, changed, ..) = ring_counts(before, &translate(after, shift), ring);
            within(changed, valid).then_some(shift)
        }
    }
}

/// 臉層：取內層臉框、邊緣羽化 10 px，再以定稿 alpha 裁剪——定稿透明處不畫，輪廓永遠是定稿的
/// （模型在背景處多畫的髮絲雜點、半透明殘影不會疊到背景上）。`base` 與 `image` 同一座標系。
pub(crate) fn face_layer(image: &RgbaImage, base: &RgbaImage, inner: FaceBox) -> RgbaImage {
    let mut layer = imageops::crop_imm(image, inner.x, inner.y, inner.w, inner.h).to_image();
    let (width, height) = layer.dimensions();
    for (x, y, pixel) in layer.enumerate_pixels_mut() {
        let edge = x.min(width - 1 - x).min(y).min(height - 1 - y) as f32;
        let mask = ((edge + 1.0) / FEATHER_PX).clamp(0.0, 1.0);
        let body = f32::from(base.get_pixel(inner.x + x, inner.y + y)[3]) / 255.0;
        pixel[3] = (f32::from(pixel[3]) * mask * body).round() as u8;
    }
    layer
}

pub(crate) fn decode(bytes: &[u8]) -> Result<RgbaImage, String> {
    image::load_from_memory(bytes)
        .map(|image| image.to_rgba8())
        .map_err(|error| error.to_string())
}

pub(crate) fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}

/// 玩家上傳的整張表情圖：縮成定稿尺寸後照臉框取臉層
pub(crate) fn face_from_full(
    full: &RgbaImage,
    base: &RgbaImage,
    face: FaceBox,
) -> Option<RgbaImage> {
    let size = base.dimensions();
    let (width, height) = full.dimensions();
    let expected = f64::from(size.0) / f64::from(size.1);
    let actual = f64::from(width) / f64::from(height.max(1));
    ((actual / expected - 1.0).abs() <= ASPECT_TOLERANCE).then(|| {
        let scaled = imageops::resize(full, size.0, size.1, FilterType::Lanczos3);
        face_layer(&scaled, base, face)
    })
}

#[cfg(test)]
mod tests;
