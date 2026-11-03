use super::*;
use crate::commands::NEXT_TEMP_ID;
use std::sync::atomic::Ordering;

struct Fixture {
    root: PathBuf,
    world: String,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "table-tavern-sprites-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let world = data::create_world(&root, "立繪").unwrap();
        Fixture { root, world }
    }

    fn dir(&self) -> SpriteDir {
        SpriteDir::of(&self.root, &self.world, &data::new_id()).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    image::RgbaImage::new(width, height)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    bytes
}

const FACE: FaceBox = FaceBox {
    x: 30,
    y: 10,
    w: 40,
    h: 40,
};

/// 挑定定稿：寫身體圖與框、修訂號起跳，四格臉層都缺
#[test]
fn install_base_writes_meta_and_reports_missing_faces() {
    let fx = Fixture::new();
    let dir = fx.dir();
    assert!(dir.read().is_none() && dir.missing_faces().is_empty());
    dir.install_base(&png(100, 150), FACE, false).unwrap();
    let Loaded { meta, size, .. } = dir.read().unwrap();
    assert_eq!(size, (100, 150));
    assert_eq!(
        (meta.face, meta.version, meta.default.as_str()),
        (FACE, 1, "base")
    );
    assert_eq!((meta.rev("base"), meta.rev("face/happy")), (1, 1));
    assert_eq!(base_missing(&dir), ["happy", "sad", "angry", "surprised"]);
    dir.write_face(None, "happy", &png(40, 40)).unwrap();
    assert_eq!(base_missing(&dir), ["sad", "angry", "surprised"]);
    // 換定稿：舊臉層全部作廢
    dir.install_base(&png(100, 150), FACE, false).unwrap();
    assert_eq!(dir.missing_faces().len(), 4);
}

/// 寫回前重驗：改框、重骰（修訂號）、換定稿後，舊工作的結果都丟掉
#[test]
fn stale_face_job_is_discarded() {
    let fx = Fixture::new();
    let dir = fx.dir();
    dir.install_base(&png(100, 150), FACE, false).unwrap();
    let (fresh, _) = dir.snapshot(None, "sad").unwrap();
    assert!(dir
        .write_face_if_current(None, "sad", &fresh, &png(40, 40))
        .unwrap());

    let (before_move, _) = dir.snapshot(None, "happy").unwrap();
    dir.set_face(None, FaceBox { x: 20, ..FACE }).unwrap();
    assert!(!dir
        .write_face_if_current(None, "happy", &before_move, &png(40, 40))
        .unwrap());
    assert!(!dir.face_path(None, "sad").exists(), "改框＝整組臉層失效");

    let (before_reroll, _) = dir.snapshot(None, "angry").unwrap();
    dir.drop_face(None, "angry").unwrap();
    assert!(!dir
        .write_face_if_current(None, "angry", &before_reroll, &png(40, 40))
        .unwrap());

    let (before_upload, _) = dir.snapshot(None, "surprised").unwrap();
    dir.install_base(&png(100, 150), FACE, false).unwrap();
    assert!(!dir
        .write_face_if_current(None, "surprised", &before_upload, &png(40, 40))
        .unwrap());
}

/// 改框只收定稿範圍內的
#[test]
fn set_face_rejects_out_of_range() {
    let fx = Fixture::new();
    let dir = fx.dir();
    dir.install_base(&png(100, 150), FACE, true).unwrap();
    assert!(dir.set_face(None, FaceBox { x: 80, ..FACE }).is_err());
    dir.set_face(None, FaceBox { x: 60, ..FACE }).unwrap();
    assert!(!dir.read().unwrap().meta.face_fallback);
}

/// 載入驗證：框出界退預設框、變體鍵不合格或重複的丟掉、預設變體不存在退 base
#[test]
fn loaded_meta_is_validated() {
    let fx = Fixture::new();
    let dir = fx.dir();
    dir.install_base(&png(100, 150), FACE, false).unwrap();
    let raw = serde_json::json!({
        "version": 1,
        "face": {"x": 90, "y": 0, "w": 40, "h": 40},
        "variants": [
            {"key": "school", "label": "制服", "face": null},
            {"key": "school", "label": "重複", "face": null},
            {"key": "base", "label": "保留", "face": null},
            {"key": "../x", "label": "壞", "face": null}
        ],
        "default": "swim",
        "revs": {}
    });
    std::fs::write(dir.0.join("sprite.json"), raw.to_string()).unwrap();
    let meta = dir.read().unwrap().meta;
    assert_eq!(meta.face, FaceBox::fallback(100, 150));
    assert!(meta.face_fallback);
    assert_eq!(meta.variants.len(), 1);
    assert_eq!(meta.default, "base");
}

/// 候選最多留 6 張，新的在前；檔名只收時間戳
#[test]
fn candidates_are_capped_and_names_checked() {
    let fx = Fixture::new();
    let dir = fx.dir();
    for _ in 0..7 {
        dir.add_candidate(&png(10, 15)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(3));
    }
    let files = dir.candidates();
    assert_eq!(files.len(), 6);
    assert!(files[0] > files[5]);
    assert!(dir.candidate_bytes(&files[0]).is_ok());
    assert!(dir.candidate_bytes("../sprite.json").is_err());
}

fn base_missing(dir: &SpriteDir) -> Vec<&'static str> {
    dir.missing_faces()
        .into_iter()
        .map(|(set, expression)| {
            assert!(set.is_none());
            expression
        })
        .collect()
}

/// 變體：共用組不另補臉層；獨立組有自己的缺格、改框與修訂號；刪變體連臉層一起清，提示詞名單跟著變
#[test]
fn variants_track_their_own_face_sets() {
    let fx = Fixture::new();
    let dir = fx.dir();
    dir.install_base(&png(100, 150), FACE, false).unwrap();
    dir.install_variant("v1", "Armor", &png(100, 150), None)
        .unwrap();
    dir.install_variant("v2", "泳裝", &png(80, 160), Some(FACE))
        .unwrap();
    assert!(dir
        .install_variant("base", "x", &png(10, 10), None)
        .is_err());
    assert_eq!(dir.variant_labels(), ["Armor", "泳裝"]);
    let own: Vec<_> = dir
        .missing_faces()
        .into_iter()
        .filter(|(set, _)| set.is_some())
        .collect();
    assert_eq!(own.len(), 4);
    assert!(own.iter().all(|(set, _)| set.as_deref() == Some("v2")));
    let (started, body) = dir.snapshot(Some("v2"), "sad").unwrap();
    assert_eq!((started.face(), png_size(&body)), (FACE, Some((80, 160))));
    assert!(
        dir.snapshot(Some("v1"), "sad").is_none(),
        "共用組沒有自己的臉層格"
    );
    dir.write_face(Some("v2"), "sad", &png(40, 40)).unwrap();
    assert!(dir.face_path(Some("v2"), "sad").is_file() && !dir.face_path(None, "sad").exists());
    assert!(
        dir.set_face(Some("v2"), FaceBox { x: 60, ..FACE }).is_err(),
        "框超出變體圖"
    );
    dir.set_face(Some("v2"), FaceBox { x: 10, ..FACE }).unwrap();
    assert!(!dir.face_path(Some("v2"), "sad").exists());
    assert!(!dir
        .write_face_if_current(Some("v2"), "sad", &started, &png(40, 40))
        .unwrap());
    dir.remove_variant("v2").unwrap();
    assert_eq!(dir.variant_labels(), ["Armor"]);
    assert!(dir.missing_faces().iter().all(|(set, _)| set.is_none()));
    assert_eq!(dir.read().unwrap().meta.next_variant_key(), "v2");
}

/// 鍵帶 `../` 時路徑不得跳出立繪資料夾：拒收，外面的檔不會被刪
#[test]
fn traversal_keys_never_leave_the_sprite_folder() {
    let fx = Fixture::new();
    let dir = fx.dir();
    dir.install_base(&png(100, 150), FACE, false).unwrap();
    let outside = fx.root.join("outside.png");
    std::fs::write(&outside, b"keep").unwrap();
    for key in ["../../../outside", "../x", "a/../../b"] {
        assert!(dir.drop_face(Some(key), "sad").is_err());
        assert!(dir.set_face(Some(key), FACE).is_err());
        assert!(dir.write_face(Some(key), "sad", &png(4, 4)).is_err());
        assert!(dir.install_variant(key, "x", &png(4, 4), None).is_err());
        assert!(dir.remove_variant(key).is_err());
        assert!(dir.face_path(Some(key), "sad").starts_with(&dir.0));
        assert!(dir.variant_body(key).is_none());
    }
    assert!(outside.is_file());
}

/// 換定稿（尺寸與臉框都不同）：共用組變體轉獨立組，保留舊臉框與已有臉層，缺的格待補
#[test]
fn base_change_detaches_shared_variants() {
    let fx = Fixture::new();
    let dir = fx.dir();
    dir.install_base(&png(100, 150), FACE, false).unwrap();
    dir.install_variant("v1", "Armor", &png(100, 150), None)
        .unwrap();
    dir.write_face(None, "happy", &png(40, 40)).unwrap();
    let moved = FaceBox {
        x: 5,
        y: 60,
        w: 30,
        h: 30,
    };
    dir.install_base(&png(80, 200), moved, false).unwrap();
    let meta = dir.read().unwrap().meta;
    assert_eq!(
        (meta.face, meta.variant("v1").unwrap().face),
        (moved, Some(FACE))
    );
    assert!(dir.face_path(Some("v1"), "happy").is_file() && !dir.face_path(None, "happy").exists());
    let (snapshot, body) = dir.snapshot(Some("v1"), "sad").unwrap();
    assert_eq!((snapshot.face(), png_size(&body)), (FACE, Some((100, 150))));
    let own: Vec<_> = dir
        .missing_faces()
        .into_iter()
        .filter(|(set, _)| set.is_some())
        .collect();
    assert_eq!(own.len(), 3);
}
