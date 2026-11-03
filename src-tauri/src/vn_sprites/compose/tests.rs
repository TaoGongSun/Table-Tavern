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
    assert!(aligned(&before, &before.clone(), inner));
    let mut shifted = RgbaImage::new(160, 160);
    imageops::overlay(&mut shifted, &before, 12, 0);
    assert!(!aligned(&before, &shifted, inner));
    // 只平移 3 px：RGB 差比例可能還沒超標，但平移搜尋會抓到最佳位移不是 0
    let mut nudged = before.clone();
    imageops::overlay(&mut nudged, &before, 3, 0);
    assert!(!aligned(&before, &nudged, inner));
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
    assert!(aligned(&before, &after, inner));
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
    assert!(aligned(&empty, &empty.clone(), inner));
    let mut smeared = empty.clone();
    for y in 0..40 {
        for x in 0..160 {
            smeared.put_pixel(x, y, Rgba([9, 9, 9, 100]));
        }
    }
    assert!(!aligned(&empty, &smeared, inner));
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
    assert!(shared_body(&base, &textured(300, 300), face()).is_none());
}
