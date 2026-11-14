use super::*;

/// 有紋理的不透明圖：平移幾格就對不上
fn textured(width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_fn(width, height, |x, y| {
        let v = x
            .wrapping_mul(2_654_435_761)
            .wrapping_add(y.wrapping_mul(40_503))
            >> 7;
        Rgba([
            (v % 251) as u8,
            (v / 3 % 241) as u8,
            (v / 7 % 239) as u8,
            255,
        ])
    })
}

fn face() -> FaceBox {
    FaceBox {
        x: 40,
        y: 20,
        w: 50,
        h: 60,
    }
}

/// 臉框每邊外擴 40% 的正方形；出界部分透明補齊
#[test]
fn crop_expands_and_pads_transparent() {
    let crop = Crop::around(face());
    assert_eq!(
        crop,
        Crop {
            left: 11,
            top: -4,
            side: 108
        }
    );
    assert_eq!(
        crop.inner(face()),
        FaceBox {
            x: 29,
            y: 24,
            w: 50,
            h: 60
        }
    );
    let cut = cut(&textured(200, 300), crop);
    assert_eq!(cut.dimensions(), (108, 108));
    assert_eq!(cut.get_pixel(50, 2)[3], 0, "定稿上緣以外補透明");
    assert_eq!(cut.get_pixel(50, 10)[3], 255);
}

/// 回來的圖要近正方形（差 ≤2%），任意尺寸縮回裁切大小
#[test]
fn fit_back_requires_square_and_resizes() {
    assert_eq!(
        fit_back(&textured(1254, 1254), 108).unwrap().dimensions(),
        (108, 108)
    );
    assert!(fit_back(&textured(1024, 1100), 108).is_none());
}

/// 對齊門檻：沒動過的過；整塊平移 12 px 不過
#[test]
fn alignment_passes_identical_and_rejects_shift() {
    let before = textured(160, 160);
    let inner = FaceBox {
        x: 50,
        y: 50,
        w: 60,
        h: 60,
    };
    assert!(aligned(&before, &before.clone(), inner).is_some());
    let mut shifted = RgbaImage::new(160, 160);
    imageops::overlay(&mut shifted, &before, 12, 0);
    assert!(aligned(&before, &shifted, inner).is_none());
    // 3 px 處完全吻合、超出補正範圍 2 px：拒收；2 px 照樣吻合則補正收下
    let mut nudged = before.clone();
    imageops::overlay(&mut nudged, &before, 3, 0);
    assert!(aligned(&before, &nudged, inner).is_none());
    let mut nudged = before.clone();
    imageops::overlay(&mut nudged, &before, 2, 0);
    assert!(aligned(&before, &nudged, inner).is_some());
}

/// 小位移補正的方向：內容往右 1、往上 2，補正後環帶（扣掉平移空出的 2 px 邊）跟原圖一樣
#[test]
fn alignment_corrects_small_shift_in_the_right_direction() {
    let before = textured(160, 160);
    let inner = FaceBox {
        x: 50,
        y: 50,
        w: 60,
        h: 60,
    };
    let mut moved = RgbaImage::new(160, 160);
    imageops::overlay(&mut moved, &before, 1, -2);
    let fixed = aligned(&before, &moved, inner).unwrap();
    for (x, y, pixel) in before.enumerate_pixels() {
        if (2..158).contains(&x) && (2..158).contains(&y) && !in_box(inner, x, y) {
            assert_eq!(fixed.get_pixel(x, y), pixel, "({x}, {y})");
        }
    }
}

/// 原位最合（原本就會過）的圖原樣交回、一個位元都不動
#[test]
fn alignment_returns_in_place_result_untouched() {
    let before = textured(160, 160);
    let inner = FaceBox {
        x: 50,
        y: 50,
        w: 60,
        h: 60,
    };
    let mut after = before.clone();
    for (x, y, pixel) in after.enumerate_pixels_mut() {
        if in_box(inner, x, y) {
            *pixel = Rgba([0, 0, 0, 255]);
        } else if (x + y) % 7 == 0 {
            pixel[0] = pixel[0].saturating_add(30);
        }
    }
    assert_eq!(aligned(&before, &after, inner), Some(after));
}

/// 取樣雜訊級的差距：最合處只贏一點點，在補正範圍內當原位（不平移），範圍外照舊拒收
#[test]
fn alignment_treats_noise_level_drift_as_in_place() {
    let ring = |x: u32, y: u32| !(50..110).contains(&x) || !(50..110).contains(&y);
    let gain = |before: &RgbaImage, after: &RgbaImage| {
        let still = mean_diff(before, after, &ring, (0, 0)).unwrap();
        let (score, shift) = (-SHIFT_RANGE..=SHIFT_RANGE)
            .flat_map(|dy| (-SHIFT_RANGE..=SHIFT_RANGE).map(move |dx| (dx, dy)))
            .filter_map(|shift| mean_diff(before, after, &ring, shift).map(|s| (s, shift)))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .unwrap();
        (1.0 - score / still, shift)
    };
    // 平滑紋理、內容只挪過去略多於一半（半像素位移）：(1, 0) 只比原位好幾 %
    let smooth = |x: f64, y: f64| {
        let v = 128.0 + 60.0 * (x * 0.35).sin() + 40.0 * (y * 0.27).sin();
        Rgba([v as u8, (255.0 - v) as u8, (v * 0.5) as u8, 255])
    };
    let before = RgbaImage::from_fn(160, 160, |x, y| smooth(f64::from(x), f64::from(y)));
    let half = RgbaImage::from_fn(160, 160, |x, y| smooth(f64::from(x) - 0.505, f64::from(y)));
    let (won, shift) = gain(&before, &half);
    assert!(
        won > 0.0 && won < 1.0 - DRIFT_GAIN && shift == (1, 0),
        "{won} {shift:?}"
    );
    assert_eq!(settle(&before, &half, &ring, true), Some((0, 0)));
    // 平坦底色加雜訊：最合處落在 2 px 外、只贏一點點，仍拒收（跟改版前一樣）
    let flat = RgbaImage::from_pixel(160, 160, Rgba([128, 128, 128, 255]));
    let noisy = RgbaImage::from_fn(160, 160, |x, y| {
        let n = (x.wrapping_mul(2_654_435_761) ^ y.wrapping_mul(40_503)) >> 9;
        let v = 98 + (n % 61) as u8;
        Rgba([v, v, v, 255])
    });
    let (won, shift) = gain(&flat, &noisy);
    assert!(won > 0.0 && won < 1.0 - DRIFT_GAIN, "{won}");
    assert!(
        shift.0.abs() > NUDGE_MAX || shift.1.abs() > NUDGE_MAX,
        "{shift:?}"
    );
    assert_eq!(settle(&flat, &noisy, &ring, true), None);
}

/// 臉框內怎麼改都不影響（只比環帶）
#[test]
fn alignment_ignores_changes_inside_face() {
    let before = textured(160, 160);
    let inner = FaceBox {
        x: 50,
        y: 50,
        w: 60,
        h: 60,
    };
    let mut after = before.clone();
    for y in 50..110 {
        for x in 50..110 {
            after.put_pixel(x, y, Rgba([0, 0, 0, 255]));
        }
    }
    assert!(aligned(&before, &after, inner).is_some());
}

/// 環帶幾乎全透明（有效像素不足）：改比 alpha 差
#[test]
fn alignment_falls_back_to_alpha_when_ring_is_transparent() {
    let inner = FaceBox {
        x: 50,
        y: 50,
        w: 60,
        h: 60,
    };
    let empty = RgbaImage::new(160, 160);
    assert!(aligned(&empty, &empty.clone(), inner).is_some());
    let mut smeared = empty.clone();
    for y in 0..40 {
        for x in 0..160 {
            smeared.put_pixel(x, y, Rgba([9, 9, 9, 100]));
        }
    }
    assert!(aligned(&empty, &smeared, inner).is_none());
}

/// 臉層＝內框大小，邊緣羽化、中心保持原 alpha
#[test]
fn face_layer_is_inner_box_with_feathered_edge() {
    let image = textured(160, 160);
    let layer = face_layer(
        &image,
        &image,
        FaceBox {
            x: 50,
            y: 50,
            w: 60,
            h: 60,
        },
    );
    assert_eq!(layer.dimensions(), (60, 60));
    assert_eq!(layer.get_pixel(0, 30)[3], 26);
    assert_eq!(layer.get_pixel(30, 30)[3], 255);
}

/// 上傳整張表情圖：比例對才收，縮成定稿尺寸後取臉框
#[test]
fn uploaded_full_sprite_is_cut_by_face_box() {
    let base = textured(200, 300);
    let layer = face_from_full(&textured(400, 600), &base, face()).unwrap();
    assert_eq!(layer.dimensions(), (50, 60));
    assert!(face_from_full(&textured(400, 400), &base, face()).is_none());
}

/// 定稿透明處臉層也透明：模型在背景多畫的雜點不會疊上去
#[test]
fn face_layer_is_clipped_to_base_silhouette() {
    let generated = textured(160, 160);
    let mut base = textured(160, 160);
    for y in 0..160 {
        for x in 0..80 {
            base.put_pixel(x, y, Rgba([0, 0, 0, 0]));
        }
    }
    let layer = face_layer(
        &generated,
        &base,
        FaceBox {
            x: 50,
            y: 50,
            w: 60,
            h: 60,
        },
    );
    assert_eq!(layer.get_pixel(20, 30)[3], 0, "定稿透明處");
    assert_eq!(layer.get_pixel(45, 30)[3], 255, "定稿不透明處照舊");
}

/// 變體身體：臉框內對得上＝共用（回定稿尺寸），平移或比例不對＝獨立
#[test]
fn variant_body_shares_faces_only_when_face_window_aligns() {
    let base = textured(200, 300);
    assert_eq!(
        shared_body(&base, &base.clone(), face()).map(|body| body.dimensions()),
        Some((200, 300))
    );
    let mut shifted = RgbaImage::new(200, 300);
    imageops::overlay(&mut shifted, &base, 12, 0);
    assert!(shared_body(&base, &shifted, face()).is_none());
    // 臉框補正不套用到變體身體：差 1 px 也獨立
    let mut nudged = base.clone();
    imageops::overlay(&mut nudged, &base, 1, 0);
    assert!(shared_body(&base, &nudged, face()).is_none());
    assert!(shared_body(&base, &textured(300, 300), face()).is_none());
}

/// 方圖補上下、太窄的圖補左右，都置中、不縮放，補的是淺灰底
#[test]
fn portrait_canvas_pads_to_two_by_three_without_scaling() {
    let red = Rgba([200, 0, 0, 255]);
    let square = encode_png(&RgbaImage::from_pixel(100, 100, red)).unwrap();
    let padded = decode(&portrait_canvas(&square).unwrap()).unwrap();
    assert_eq!(padded.dimensions(), (100, 150));
    assert_eq!(*padded.get_pixel(50, 10), BACKDROP);
    assert_eq!(*padded.get_pixel(50, 75), red);
    assert_eq!(*padded.get_pixel(50, 140), BACKDROP);
    let narrow = encode_png(&RgbaImage::from_pixel(100, 300, red)).unwrap();
    let padded = decode(&portrait_canvas(&narrow).unwrap()).unwrap();
    assert_eq!(padded.dimensions(), (200, 300));
    assert_eq!(*padded.get_pixel(10, 150), BACKDROP);
    assert_eq!(*padded.get_pixel(100, 150), red);
}
