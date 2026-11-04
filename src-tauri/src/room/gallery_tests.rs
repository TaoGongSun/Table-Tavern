//! 手機圖庫（room-phone-gallery）：只列自己座位那張卡、翻頁與整則預算、壞檔跳過、取單張只認清單裡的檔名；
//! 從圖庫存圖可帶裁切框，寫入前重驗座位；來源標記只在圖庫那張套上時留著，別條路換圖、刪圖就清掉。
use super::*;
use crate::import;
use base64::Engine as _;

/// 雜訊圖：縮圖壓不小，用來撐爆整則預算
fn noise(px: u32, seed: u32) -> Vec<u8> {
    let mut n = seed.wrapping_mul(2_654_435_761) | 1;
    let image = image::RgbImage::from_fn(px, px, |_, _| {
        n ^= n << 13;
        n ^= n >> 17;
        n ^= n << 5;
        image::Rgb([n as u8, (n >> 8) as u8, (n >> 16) as u8])
    });
    let mut out = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

fn put(root: &std::path::Path, world: &str, card: &str, file: &str, bytes: &[u8]) {
    let dir = data::gallery_dir(root, world, card).unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(file), bytes).unwrap();
}

fn files(reply: &Value) -> Vec<String> {
    reply["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["file"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn phone_gallery_lists_own_card_pages_and_crops() {
    let (root, world, tao, boss) = table("phone-gallery");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let _running = start(
        Fake::new(&root, vec![]),
        saved(&server, &world),
        FAST,
        false,
    )
    .await;
    let mut conn = accept(&listener).await;
    conn.expect("hello").await;
    let snapshot = json!({ "room": { "status": "lobby", "payerId": null, "hostId": null },
        "members": [{ "memberId": "m1", "cardId": tao }, { "memberId": "m2", "cardId": boss }], "cards": [] });
    conn.send(
        "welcome",
        json!({ "snapshot": snapshot, "dispatch": null, "job": null }),
    )
    .await;
    conn.expect("status").await;

    // 14 張好圖＋1 張壞檔夾在中間；老闆的圖庫另有一張
    for i in 0..14u32 {
        put(
            &root,
            &world,
            &tao,
            &format!("17000000000{:02}.png", 10 + i),
            &super::card_tests::png(40),
        );
    }
    put(&root, &world, &tao, "1700000000016b.png", b"not a png");
    put(
        &root,
        &world,
        &boss,
        "1800000000000.png",
        &super::card_tests::png(40),
    );
    let ask = |id: &str, extra: Value| {
        let mut p = json!({ "reqId": id, "memberId": "m1", "cardId": tao });
        p.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        p
    };

    // 讀卡帶圖庫張數；還沒從圖庫套過圖就沒有來源
    conn.send("card.read", ask("r1", json!({}))).await;
    let read = conn.expect("card.read.result").await["p"].clone();
    assert_eq!(
        (
            read["card"]["gallery"].clone(),
            read["card"]["galleryFile"].clone()
        ),
        (json!(15), Value::Null)
    );

    // 第一頁：新的在前、12 張、壞檔跳過不佔頁數也不算進 more
    conn.send("card.gallery", ask("g1", json!({}))).await;
    let page = conn.expect("card.gallery.result").await["p"].clone();
    let first = files(&page);
    assert_eq!(first.len(), 12);
    assert_eq!(first[0], "1700000000023.png");
    assert!(!first.contains(&"1700000000016b.png".to_owned()));
    assert_eq!(page["more"], 2);
    assert!(page["items"][0]["thumb"]
        .as_str()
        .unwrap()
        .starts_with("data:image/png;base64,"));
    // 第二頁接著最後一筆，看到底 more = 0
    conn.send("card.gallery", ask("g2", json!({ "before": first[11] })))
        .await;
    let page = conn.expect("card.gallery.result").await["p"].clone();
    assert_eq!(
        (files(&page), page["more"].clone()),
        (
            vec![
                "1700000000011.png".to_owned(),
                "1700000000010.png".to_owned()
            ],
            json!(0)
        )
    );
    // 游標在最舊的後面：空頁、more 0
    conn.send(
        "card.gallery",
        ask("g3", json!({ "before": "1700000000010.png" })),
    )
    .await;
    let page = conn.expect("card.gallery.result").await["p"].clone();
    assert_eq!((files(&page).len(), page["more"].clone()), (0, json!(0)));

    // 別人的卡、別人圖庫的檔名都拒收
    conn.send(
        "card.gallery",
        json!({ "reqId": "g4", "memberId": "m1", "cardId": boss }),
    )
    .await;
    assert_eq!(
        conn.expect("card.gallery.result").await["p"]["code"],
        "E_FORBIDDEN"
    );
    conn.send(
        "card.gallery",
        ask("g5", json!({ "file": "1800000000000.png" })),
    )
    .await;
    assert_eq!(
        conn.expect("card.gallery.result").await["p"]["code"],
        "E_FORBIDDEN"
    );
    // 取單張：一筆、較大的預覽
    conn.send(
        "card.gallery",
        ask("g6", json!({ "file": "1700000000023.png" })),
    )
    .await;
    let one = conn.expect("card.gallery.result").await["p"].clone();
    assert_eq!(files(&one), vec!["1700000000023.png".to_owned()]);
    assert!(one.get("more").is_none());

    // 帶裁切框存檔：只在原圖範圍內、要跟圖庫圖一起
    let wide = image::RgbImage::from_pixel(3000, 1000, image::Rgb([10, 20, 30]));
    let mut bytes = Vec::new();
    wide.write_to(
        &mut std::io::Cursor::new(&mut bytes),
        image::ImageFormat::Png,
    )
    .unwrap();
    put(&root, &world, &tao, "1700000000099.png", &bytes);
    for (id, crop) in [
        ("c1", json!({ "x": 0.5, "y": 0, "w": 0.6, "h": 1 })),
        ("c2", json!({ "x": 0, "y": 0, "w": 0, "h": 1 })),
        ("c3", json!({ "x": -0.1, "y": 0, "w": 0.5, "h": 1 })),
        ("c4", json!({ "x": 0, "y": 0, "w": "0.5", "h": 1 })),
        ("c5", json!({ "x": 0, "y": 0, "h": 1 })),
        // 不足一像素：寬 0.0001×3000 = 0.3 像素
        ("c6", json!({ "x": 0.2, "y": 0, "w": 0.0001, "h": 1 })),
    ] {
        conn.send(
            "card.save",
            ask(
                id,
                json!({ "imageFromGallery": "1700000000099.png", "crop": crop }),
            ),
        )
        .await;
        assert_eq!(
            conn.expect("card.save.result").await["p"]["code"],
            "E_TEXT_INVALID",
            "{id}"
        );
    }
    conn.send(
        "card.save",
        ask("c7", json!({ "crop": { "x": 0, "y": 0, "w": 1, "h": 1 } })),
    )
    .await;
    assert_eq!(
        conn.expect("card.save.result").await["p"]["code"],
        "E_TEXT_INVALID"
    );
    // 容差邊界：x + w = 1.0001 收；3000×1000 取右邊 2:3 → 667×1000，寬不到 1024 不縮
    conn.send("card.save", ask("c8", json!({ "imageFromGallery": "1700000000099.png", "crop": { "x": 0.7777, "y": 0, "w": 0.2224, "h": 1 } }))).await;
    assert_eq!(conn.expect("card.save.result").await["p"]["ok"], true);
    let saved_png = import::character_image(&root, &world, &tao)
        .unwrap()
        .unwrap();
    let saved_png = base64::engine::general_purpose::STANDARD
        .decode(saved_png)
        .unwrap();
    let saved_img = image::load_from_memory(&saved_png).unwrap();
    assert_eq!((saved_img.width(), saved_img.height()), (667, 1000));
    assert!(import::character_avatar(&root, &world, &tao)
        .unwrap()
        .is_some());
    conn.send("card.read", ask("r2", json!({}))).await;
    assert_eq!(
        conn.expect("card.read.result").await["p"]["card"]["galleryFile"],
        "1700000000099.png"
    );
    // 寬超過 1024 就縮到 1024
    conn.send("card.save", ask("c9", json!({ "imageFromGallery": "1700000000099.png", "crop": { "x": 0, "y": 0, "w": 0.6, "h": 1 } }))).await;
    assert_eq!(conn.expect("card.save.result").await["p"]["ok"], true);
    let saved_png = base64::engine::general_purpose::STANDARD
        .decode(
            import::character_image(&root, &world, &tao)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
    assert_eq!(image::load_from_memory(&saved_png).unwrap().width(), 1024);

    // 換圖驗證失敗：舊圖、舊標記都在；桌面版換圖成功：標記清掉；再套一次後移除圖：也清掉
    assert!(import::save_character_image(&root, &world, &tao, b"bad").is_err());
    assert_eq!(
        import::character_image_source(&root, &world, &tao).as_deref(),
        Some("1700000000099.png")
    );
    import::save_character_image(&root, &world, &tao, &super::card_tests::png(30)).unwrap();
    assert_eq!(import::character_image_source(&root, &world, &tao), None);
    conn.send(
        "card.save",
        ask("c10", json!({ "imageFromGallery": "1700000000099.png" })),
    )
    .await;
    assert_eq!(conn.expect("card.save.result").await["p"]["ok"], true);
    assert!(import::character_image_source(&root, &world, &tao).is_some());
    conn.send("card.save", ask("c11", json!({ "removeImage": true })))
        .await;
    assert_eq!(conn.expect("card.save.result").await["p"]["ok"], true);
    assert_eq!(import::character_image_source(&root, &world, &tao), None);

    // 換座後才到的存檔：寫入前重驗座位，卡不動
    conn.send("ev", json!({ "seq": 9, "at": 0, "e": { "type": "seat.released", "memberId": "m1", "cardId": tao } })).await;
    conn.send(
        "card.save",
        ask("c12", json!({ "imageFromGallery": "1700000000099.png" })),
    )
    .await;
    assert_eq!(
        conn.expect("card.save.result").await["p"]["code"],
        "E_FORBIDDEN"
    );
    assert!(import::character_image(&root, &world, &tao)
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn phone_gallery_page_stays_inside_frame_budget() {
    let (root, world, tao, _) = table("phone-gallery-budget");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let _running = start(
        Fake::new(&root, vec![]),
        saved(&server, &world),
        FAST,
        false,
    )
    .await;
    let mut conn = accept(&listener).await;
    conn.expect("hello").await;
    let snapshot = json!({ "room": { "status": "lobby", "payerId": null, "hostId": null },
        "members": [{ "memberId": "m1", "cardId": tao }], "cards": [] });
    conn.send(
        "welcome",
        json!({ "snapshot": snapshot, "dispatch": null, "job": null }),
    )
    .await;
    conn.expect("status").await;
    // 雜訊縮圖一張 base64 約 100 KB：12 張放不下 1000 KiB
    for i in 0..13u32 {
        put(
            &root,
            &world,
            &tao,
            &format!("17000000000{:02}.png", 10 + i),
            &noise(160, i + 1),
        );
    }
    conn.send(
        "card.gallery",
        json!({ "reqId": "g1", "memberId": "m1", "cardId": tao }),
    )
    .await;
    let frame = conn.expect("card.gallery.result").await;
    let shown = files(&frame["p"]).len();
    assert!((1..12).contains(&shown), "{shown}");
    assert_eq!(frame["p"]["more"], 13 - shown);
    assert!(frame.to_string().len() <= 1000 * 1024);
}

#[tokio::test]
async fn crop_save_rechecks_seat_after_cropping() {
    let (root, world, tao, _) = table("phone-gallery-race");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let _running = start(
        Fake::new(&root, vec![]),
        saved(&server, &world),
        FAST,
        false,
    )
    .await;
    let mut conn = accept(&listener).await;
    conn.expect("hello").await;
    let snapshot = json!({ "room": { "status": "lobby", "payerId": null, "hostId": null },
        "members": [{ "memberId": "m1", "cardId": tao }], "cards": [] });
    conn.send(
        "welcome",
        json!({ "snapshot": snapshot, "dispatch": null, "job": null }),
    )
    .await;
    conn.expect("status").await;
    put(
        &root,
        &world,
        &tao,
        "1700000000001.png",
        &super::card_tests::png(60),
    );

    // 裁切做完、寫入前停住：這段時間換座，放行後寫入前重驗擋下
    let (reached_tx, reached_rx) = std::sync::mpsc::channel();
    let (resume_tx, resume_rx) = std::sync::mpsc::channel();
    *super::cards::HOLD_BEFORE_WRITE.lock().unwrap() = Some((tao.clone(), reached_tx, resume_rx));
    conn.send(
        "card.save",
        json!({ "reqId": "s1", "memberId": "m1", "cardId": tao,
        "imageFromGallery": "1700000000001.png", "crop": { "x": 0, "y": 0, "w": 0.5, "h": 1 } }),
    )
    .await;
    tokio::task::spawn_blocking(move || reached_rx.recv_timeout(Duration::from_secs(10)).unwrap())
        .await
        .unwrap();
    conn.send("ev", json!({ "seq": 9, "at": 0, "e": { "type": "seat.released", "memberId": "m1", "cardId": tao } })).await;
    // 換座事件已處理：讀卡被拒
    conn.send(
        "card.read",
        json!({ "reqId": "r1", "memberId": "m1", "cardId": tao }),
    )
    .await;
    assert_eq!(
        conn.expect("card.read.result").await["p"]["code"],
        "E_FORBIDDEN"
    );
    resume_tx.send(()).unwrap();
    let result = conn.expect("card.save.result").await["p"].clone();
    assert_eq!(
        (result["reqId"].clone(), result["code"].clone()),
        (json!("s1"), json!("E_FORBIDDEN"))
    );
    assert!(import::character_image(&root, &world, &tao)
        .unwrap()
        .is_none());
    assert_eq!(import::character_image_source(&root, &world, &tao), None);
}

#[test]
fn image_source_marker_follows_gallery_name_rule() {
    for (name, ok) in [
        ("1700000000001.png", true),
        ("a.b-c_d.png", true),
        (".hidden.png", false),
        ("a..png", false),
        ("a/b.png", false),
        ("a b.png", false),
        ("1700.jpg", false),
        (".png", false),
    ] {
        assert_eq!(import::gallery_file_name(name), ok, "{name}");
    }
    assert!(!import::gallery_file_name(&format!(
        "{}.png",
        "a".repeat(81)
    )));
}
