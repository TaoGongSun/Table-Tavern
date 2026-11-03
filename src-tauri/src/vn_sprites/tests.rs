use super::store::FaceBox;
use super::{base_is_current, parse_face_box, stale_or_failed, supersede_base};

/// 視覺模型的框：上緣再往上加框高 25%；前後夾雜文字也找得到
#[test]
fn face_box_reply_is_parsed_and_lifted() {
    assert_eq!(
        parse_face_box(
            "框如下\n{\"x0\":400,\"y0\":200,\"x1\":600,\"y1\":400}",
            1024,
            1536
        ),
        Some(FaceBox {
            x: 400,
            y: 150,
            w: 200,
            h: 250
        })
    );
    // 往上加超出圖頂就貼齊 0
    assert_eq!(
        parse_face_box("{\"x0\":400,\"y0\":20,\"x1\":600,\"y1\":220}", 1024, 1536)
            .unwrap()
            .y,
        0
    );
}

/// 出界、反向、面積不合理、不是 JSON 都不收
#[test]
fn unreasonable_face_box_is_rejected() {
    for reply in [
        "{\"x0\":900,\"y0\":200,\"x1\":1100,\"y1\":400}",
        "{\"x0\":600,\"y0\":200,\"x1\":400,\"y1\":400}",
        "{\"x0\":500,\"y0\":500,\"x1\":510,\"y1\":510}",
        "{\"x0\":0,\"y0\":0,\"x1\":1024,\"y1\":1000}",
        "NO_BOX",
    ] {
        assert_eq!(parse_face_box(reply, 1024, 1536), None, "{reply}");
    }
}

/// 換定稿世代：後開工的換定稿或刪除讓先開工的那次作廢
#[test]
fn later_base_change_supersedes_earlier_one() {
    let first = supersede_base("W-epoch", "C");
    assert!(base_is_current("W-epoch", "C", first));
    let second = supersede_base("W-epoch", "C");
    assert!(!base_is_current("W-epoch", "C", first));
    supersede_base("W-epoch", "C"); // 刪除
    assert!(!base_is_current("W-epoch", "C", second));
    assert!(!base_is_current("W-epoch", "other", 1));
}

/// 過期任務的失敗不記失敗（改框、換定稿後由佇列重排）；仍是目前這格才記
#[test]
fn stale_face_job_failure_is_not_recorded() {
    use crate::commands::NEXT_TEMP_ID;
    use crate::data;
    let root = std::env::temp_dir().join(format!(
        "table-tavern-stale-{}-{}",
        std::process::id(),
        NEXT_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let world = data::create_world(&root, "過期").unwrap();
    let dir = super::SpriteDir::of(&root, &world, &data::new_id()).unwrap();
    let mut png = Vec::new();
    image::RgbaImage::new(100, 150)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    dir.install_base(
        &png,
        FaceBox {
            x: 30,
            y: 10,
            w: 40,
            h: 40,
        },
        false,
    )
    .unwrap();
    let (started, _) = dir.snapshot(None, "sad").unwrap();
    let now = || dir.snapshot(None, "sad").map(|(now, _)| now);
    assert_eq!(
        stale_or_failed("X".into(), &started, now()),
        Err("X".into())
    );
    dir.set_face(
        None,
        FaceBox {
            x: 20,
            y: 10,
            w: 40,
            h: 40,
        },
    )
    .unwrap();
    assert_eq!(stale_or_failed("X".into(), &started, now()), Ok(false));
    dir.remove_all().unwrap();
    assert_eq!(stale_or_failed("X".into(), &started, now()), Ok(false));
    let _ = std::fs::remove_dir_all(root);
}

/// 變體名：劇本括號語法用到的字元、base、空字串與過長都不收；提示詞只列有變體的角色
#[test]
fn variant_labels_and_prompt_note() {
    assert!(super::valid_label("泳裝") && super::valid_label("battle armor"));
    for bad in ["", "Base", "a@b", "戰甲（破）", "x,y", &"長".repeat(25)] {
        assert!(!super::valid_label(bad), "{bad}");
    }
    let roster = vec![
        (
            "艾琳".to_owned(),
            vec!["泳裝".to_owned(), "戰甲".to_owned()],
        ),
        ("路人".to_owned(), Vec::new()),
    ];
    let note = crate::transport::vn_variant_note("zh-TW", &roster);
    assert!(
        note.contains("- 艾琳: 泳裝, 戰甲") && !note.contains("路人") && note.contains("@base")
    );
    assert!(crate::transport::vn_variant_note("en", &roster).contains("@variant"));
    assert_eq!(crate::transport::vn_variant_note("en", &roster[1..]), "");
}
