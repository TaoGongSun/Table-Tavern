//! 新身體圖在臉框計算前裁透明邊；臉層與既有資產讀取不經這裡。
use super::compose;
pub(crate) fn trim_body(png: &[u8]) -> Result<Vec<u8>, String> {
    let image = compose::decode(png)?;
    let (mut x0, mut y0, mut x1, mut y1) = (image.width(), image.height(), 0, 0);
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel[3] != 0 {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x + 1);
            y1 = y1.max(y + 1);
        }
    }
    if x1 <= x0 || y1 <= y0 {
        return Err("VN_SPRITE_EMPTY".into());
    }
    if x1 - x0 > y1 - y0 {
        return Err("VN_SPRITE_LANDSCAPE".into());
    }
    compose::encode_png(&image::imageops::crop_imm(&image, x0, y0, x1 - x0, y1 - y0).to_image())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};
    #[test]
    fn trims_padding_preserves_soft_pixels_and_is_idempotent() {
        let mut padded = RgbaImage::new(160, 90);
        for x in 60..90 {
            for y in 10..80 {
                padded.put_pixel(x, y, Rgba([1, 2, 3, 255]));
            }
        }
        padded.put_pixel(59, 9, Rgba([1, 2, 3, 1]));
        let cropped = trim_body(&compose::encode_png(&padded).unwrap()).unwrap();
        let decoded = compose::decode(&cropped).unwrap();
        assert_eq!(decoded.dimensions(), (31, 71));
        assert_eq!(decoded.get_pixel(0, 0)[3], 1);
        assert_eq!(decoded.get_pixel(30, 70)[3], 255);
        assert_eq!(trim_body(&cropped).unwrap(), cropped);
    }
    #[test]
    fn rejects_empty_and_landscape_but_allows_square() {
        let encode = |w, h, a| {
            compose::encode_png(&RgbaImage::from_pixel(w, h, Rgba([0, 0, 0, a]))).unwrap()
        };
        assert_eq!(
            trim_body(&encode(10, 20, 0)).unwrap_err(),
            "VN_SPRITE_EMPTY"
        );
        assert_eq!(
            trim_body(&encode(20, 10, 255)).unwrap_err(),
            "VN_SPRITE_LANDSCAPE"
        );
        assert!(trim_body(&encode(10, 10, 255)).is_ok());
    }
    #[test]
    fn equally_trimmed_variant_can_share_the_base_canvas() {
        let body = RgbaImage::from_pixel(40, 80, Rgba([40, 50, 60, 255]));
        let mut padded = RgbaImage::new(160, 90);
        image::imageops::replace(&mut padded, &body, 50, 5);
        let cropped =
            compose::decode(&trim_body(&compose::encode_png(&padded).unwrap()).unwrap()).unwrap();
        let face = super::super::store::FaceBox {
            x: 10,
            y: 10,
            w: 20,
            h: 20,
        };
        assert!(compose::shared_body(&body, &padded, face).is_none());
        assert_eq!(
            compose::shared_body(&body, &cropped, face)
                .unwrap()
                .dimensions(),
            body.dimensions()
        );
    }
}
