//! 玩家卡（room-player-cards 包 2）：讀卡與存卡只認伺服器說的座位、只覆寫送來的欄位；生圖照本機開關、
//! 進圖庫後可套用；玩家匯入卡檔只建角色本身（沒有原卡、來源、擴充、世界書、介面）；換圖連名冊頭像一起換。
use super::*;
use crate::import;

/// 能過嚴驗的小 PNG
pub(super) fn png(px: u32) -> Vec<u8> {
    let mut out = Vec::new();
    image::RgbImage::from_pixel(px, px, image::Rgb([200, 80, 40]))
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

/// 收執行端的一個 GET：驗路徑與憑證，回 `body`
async fn serve_upload(listener: &TcpListener, upload: &str, body: &[u8]) {
    let (mut stream, _) = tokio::time::timeout(Duration::from_secs(10), listener.accept())
        .await
        .unwrap()
        .unwrap();
    let mut request = Vec::new();
    let mut buf = [0u8; 1024];
    while !request.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = stream.read(&mut buf).await.unwrap();
        request.extend_from_slice(&buf[..n]);
    }
    let request = String::from_utf8_lossy(&request).to_lowercase();
    assert!(
        request.starts_with(&format!(
            "get /api/rooms/{}/cards/{} ",
            CODE.to_lowercase(),
            upload.to_lowercase()
        )),
        "{request}"
    );
    assert!(
        request.contains(&format!("authorization: bearer {}", "t".repeat(43))),
        "{request}"
    );
    let head = format!("HTTP/1.1 200 OK\r\ncontent-type: application/octet-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", body.len());
    stream.write_all(head.as_bytes()).await.unwrap();
    stream.write_all(body).await.unwrap();
}

#[tokio::test]
async fn player_cards_follow_seats_and_import_only_the_character() {
    let (root, world, tao, boss) = table("player-cards");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let running = start(
        Fake::new(&root, vec![]),
        saved(&server, &world),
        FAST,
        false,
    )
    .await;
    let mut conn = accept(&listener).await;
    let hello = conn.expect("hello").await;
    assert_eq!(hello["p"]["role"], "executor");
    let snapshot = json!({ "room": { "status": "lobby", "payerId": null, "hostId": null },
        "members": [{ "memberId": "m1", "cardId": tao, "nickname": "阿濤的玩家" }, { "memberId": "m2", "cardId": null }], "cards": [] });
    conn.send(
        "welcome",
        json!({ "snapshot": snapshot, "dispatch": null, "job": null }),
    )
    .await;
    let status = conn.expect("status").await;
    assert_eq!(status["p"]["imageGen"], true);

    // 讀卡：只回自己座位上的卡，含秘密段；別人的卡、舞台卡拒收
    conn.send(
        "card.read",
        json!({ "reqId": "r1", "memberId": "m1", "cardId": tao }),
    )
    .await;
    let read = conn.expect("card.read.result").await["p"].clone();
    assert_eq!(
        (
            read["ok"].clone(),
            read["card"]["privateMd"].clone(),
            read["card"]["image"].clone()
        ),
        (json!(true), json!("阿濤的祕密。"), Value::Null)
    );
    conn.send(
        "card.read",
        json!({ "reqId": "r2", "memberId": "m1", "cardId": boss }),
    )
    .await;
    assert_eq!(
        conn.expect("card.read.result").await["p"]["code"],
        "E_FORBIDDEN"
    );

    // 存卡：只覆寫送來的欄位、名冊重送；別人的卡拒收
    conn.send("card.save", json!({ "reqId": "s1", "memberId": "m1", "cardId": tao, "name": "阿濤二", "privateMd": "新祕密\r\n\t第二行" })).await;
    assert_eq!(
        conn.expect("roster.update").await["p"]["cards"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(conn.expect("card.save.result").await["p"]["ok"], true);
    let card = data::read_character(&root, &world, &tao).unwrap();
    assert_eq!(
        (
            card.name.as_str(),
            card.public_md.as_str(),
            card.private_md.as_str()
        ),
        ("阿濤二", "阿濤的公開設定。", "新祕密\n\t第二行")
    );
    conn.send(
        "card.save",
        json!({ "reqId": "s2", "memberId": "m1", "cardId": boss, "privateMd": "偷改" }),
    )
    .await;
    assert_eq!(
        conn.expect("card.save.result").await["p"]["code"],
        "E_FORBIDDEN"
    );

    // 生圖：本機關掉就拒；開著就進圖庫、記一筆，從圖庫套用時連名冊頭像一起換；不在圖庫的檔名拒收
    set_image_gen_for(&running.room, false);
    conn.send("image.gen", json!({ "reqId": "g1", "memberId": "m1", "cardId": tao, "name": "阿濤", "description": "", "extraPrompt": "", "framing": "half" })).await;
    assert_eq!(
        conn.expect("image.gen.result").await["p"]["code"],
        "E_FORBIDDEN"
    );
    set_image_gen_for(&running.room, true);
    // 本機生圖來源是 CLI：房間不生（CLI 生圖的工具沒鎖到只剩生圖）
    data::update_config(&root, &json!({ "preferences": { "image_source": "agy" } })).unwrap();
    let frame = crate::room::run::status_frame(&running.room);
    assert_eq!(
        (
            frame["p"]["imageGen"].clone(),
            frame["p"]["imageGenRoute"].clone()
        ),
        (json!(false), json!(false))
    );
    conn.send("image.gen", json!({ "reqId": "g1b", "memberId": "m1", "cardId": tao, "name": "阿濤", "description": "", "extraPrompt": "", "framing": "half" })).await;
    assert_eq!(
        conn.expect("image.gen.result").await["p"]["code"],
        "E_GEN_UNSUPPORTED"
    );
    data::update_config(&root, &json!({ "preferences": { "image_source": "api" } })).unwrap();
    conn.send("image.gen", json!({ "reqId": "g2", "memberId": "m1", "cardId": tao, "name": "阿濤", "description": "", "extraPrompt": "", "framing": "half" })).await;
    let generated = conn.expect("image.gen.result").await["p"].clone();
    assert_eq!(generated["file"], "1700000000000.png");
    assert!(generated["preview"]
        .as_str()
        .unwrap()
        .starts_with("data:image/png;base64,"));
    assert!(running
        .room
        .status()
        .gen_log
        .last()
        .is_some_and(|log| log.ok && log.name == "阿濤二"));
    conn.send(
        "card.save",
        json!({ "reqId": "s3", "memberId": "m1", "cardId": tao, "imageFromGallery": "9.png" }),
    )
    .await;
    assert_eq!(
        conn.expect("card.save.result").await["p"]["code"],
        "E_FORBIDDEN"
    );
    conn.send("card.save", json!({ "reqId": "s4", "memberId": "m1", "cardId": tao, "imageFromGallery": "1700000000000.png" })).await;
    assert_eq!(conn.expect("card.save.result").await["p"]["ok"], true);
    assert!(import::character_avatar(&root, &world, &tao)
        .unwrap()
        .is_some());

    // 匯入卡檔：只建角色本身，送名冊、回卡 id；桌上沒有原卡、來源、介面、世界書條目
    let chara = r#"{"spec":"chara_card_v2","data":{"name":"匯入者","description":"匯入的描述","alternate_greetings":["備用"],
        "character_book":{"entries":[{"keys":["x"],"content":"卡上世界書","enabled":true}]},
        "extensions":{"regex_scripts":[{"scriptName":"r","findRegex":"a","replaceString":"b"}]}}}"#;
    let bytes = crate::import::test_support::card_png(chara);
    let upload = "01ABCDEFGHJKMNPQRSTVWXYZ01";
    conn.send("job.card_import", json!({ "jobId": "j1", "kind": "card_import", "memberId": "m2", "uploadId": upload, "size": bytes.len() })).await;
    conn.expect("job.ack").await;
    serve_upload(&listener, upload, &bytes).await;
    conn.expect("roster.update").await;
    let result = conn.expect("job.result").await["p"].clone();
    let made = result["result"]["cardId"].as_str().unwrap().to_owned();
    let card = data::read_character(&root, &world, &made).unwrap();
    assert_eq!(
        (card.name.as_str(), card.private_md.as_str()),
        ("匯入者", "")
    );
    assert!(card.public_md.contains("匯入的描述") && !card.public_md.contains("備用"));
    let md = data::character_path(&root, &world, &made).unwrap();
    assert!(!md.with_extension("card.png").exists() && !md.with_extension("import.json").exists());
    assert!(import::character_image(&root, &world, &made)
        .unwrap()
        .is_some());
    assert!(import::read_card_interfaces(&root, &world)
        .unwrap_or_default()
        .is_empty());
    assert!(
        !serde_json::to_string(&data::read_worldbook(&root, &world).unwrap_or_default())
            .unwrap()
            .contains("卡上世界書")
    );

    // 換圖：上傳者還沒坐上這張卡就拒；坐上後收 PNG、名冊頭像跟著換
    let image = png(8);
    conn.send("job.card_image", json!({ "jobId": "j2", "kind": "card_image", "memberId": "m2", "cardId": made, "uploadId": upload, "size": image.len() })).await;
    conn.expect("job.ack").await;
    assert_eq!(
        conn.expect("job.result").await["p"]["error"]["code"],
        "E_FORBIDDEN"
    );
    conn.send("ev", json!({ "seq": 9, "at": 0, "e": { "type": "seat.claimed", "memberId": "m2", "cardId": made } })).await;
    conn.send("job.card_image", json!({ "jobId": "j3", "kind": "card_image", "memberId": "m2", "cardId": made, "uploadId": upload, "size": image.len() })).await;
    conn.expect("job.ack").await;
    serve_upload(&listener, upload, &image).await;
    conn.expect("roster.update").await;
    assert_eq!(conn.expect("job.result").await["p"]["ok"], true);
    assert!(import::character_avatar(&root, &world, &made)
        .unwrap()
        .is_some());

    // 座位釋出後就讀不到
    conn.send("ev", json!({ "seq": 10, "at": 0, "e": { "type": "seat.released", "memberId": "m1", "cardId": tao } })).await;
    conn.send(
        "card.read",
        json!({ "reqId": "r3", "memberId": "m1", "cardId": tao }),
    )
    .await;
    assert_eq!(
        conn.expect("card.read.result").await["p"]["code"],
        "E_FORBIDDEN"
    );
    running.stop().await;
}

fn set_image_gen_for(room: &Room, enabled: bool) {
    room.update(|saved| saved.image_gen_off = !enabled);
}

#[test]
fn player_import_reads_only_name_public_and_image() {
    let json = r#"{"data":{"name":"  阿明 ","description":"描述","personality":"個性","alternate_greetings":["不收"]}}"#;
    let card = import::player_card_from_bytes(json.as_bytes(), "zh-TW").unwrap();
    assert_eq!((card.name.as_str(), card.image.is_none()), ("阿明", true));
    assert!(
        card.public_md.contains("描述")
            && card.public_md.contains("個性")
            && !card.public_md.contains("不收")
    );
    assert!(import::player_card_from_bytes(b"{\"data\":{}}", "zh-TW").is_err());
}
