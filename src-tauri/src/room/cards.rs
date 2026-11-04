//! 玩家卡（room-player-cards §3.1–§3.4、§3.6）：自建卡與玩家匯入卡檔（取檔、重驗、只收角色本身）、換圖、
//! 讀卡與存卡（含秘密段，只回給伺服器轉本人）、玩家生圖（花這台電腦上設定的 AI 帳號，本機開關與紀錄）。
//! 手機列自己卡的圖庫、從圖庫選圖（可帶裁切框）見 room-phone-gallery §3。
//! 讀卡、存卡、生圖的結果用即時訊框：執行端斷線時伺服器已把等著的請求判失敗，重送沒有意義。
use super::{run, seed, GenLog, Room};
use crate::commands::image::ImageRequest;
use crate::{data, import};
use base64::Engine as _;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

const CARD_MAX: usize = 8 << 20;
const IMAGE_MAX: usize = 2 << 20;
const TEXT_MAX: usize = 64_000;
/// 伺服器轉給手機時會再包一層，整則留在 1 MiB 訊框以內（§3.6）
const READ_FRAME_MAX: usize = 1000 * 1024;
const PREVIEW_PX: u32 = 320;
/// 圖庫縮圖：320px 一張 base64 約 187 KB，一則只放得下 5 張；160px 約 51 KB（room-phone-gallery §3.4）
const THUMB_PX: u32 = 160;
/// 同桌面版生圖窗 `GALLERY_PAGE_SIZE`
const GALLERY_PAGE: usize = 12;
/// 同桌面版裁切窗：角色圖寬最多 1024
const CROP_MAX_WIDTH: u32 = 1024;

fn text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_owned()
}

fn roots(room: &Room) -> (std::path::PathBuf, String) {
    (room.engine.roots().1, room.world())
}

// ── 座位：伺服器說的誰坐哪張（welcome 快照＋座位事件） ──

pub(super) fn seats_from_snapshot(room: &Room, snapshot: &Value) {
    room.work().seats = snapshot["members"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| Some((m["cardId"].as_str()?.to_owned(), text(&m["memberId"]))))
        .collect();
}

pub(super) fn on_seat_event(room: &Room, e: &Value) {
    let card = text(&e["cardId"]);
    let mut work = room.work();
    match e["type"].as_str().unwrap_or_default() {
        "seat.claimed" => {
            work.seats.insert(card, text(&e["memberId"]));
        }
        "seat.released" => {
            work.seats.remove(&card);
        }
        "seat.reassigned" => {
            work.seats.insert(card, text(&e["to"]));
        }
        _ => {}
    }
}

/// 請求裡的卡要是這位成員現在坐的那張、還在桌上、不是舞台卡
fn seated_card(room: &Room, member: &str, card: &str) -> Result<data::CharacterCard, &'static str> {
    if room.work().seats.get(card).map(String::as_str) != Some(member) || member.is_empty() {
        return Err("E_FORBIDDEN");
    }
    let (root, world) = roots(room);
    let found = data::read_character(&root, &world, card).map_err(|_| "E_STATE_CONFLICT")?;
    match found.stage {
        true => Err("E_FORBIDDEN"),
        false => Ok(found),
    }
}

/// 名字、公開段或頭像變了：照桌上現況整份重送名冊
fn resend_roster(room: &Room) {
    let (root, world) = roots(room);
    match seed::roster(&root, &world) {
        Ok(cards) => {
            room.work().roster = seed::card_ids(&cards);
            room.send("roster.update", json!({ "cards": cards }));
        }
        Err(error) => room.note_error(&format!("roster.update：{error}")),
    }
    room.engine.table_changed(&world);
}

/// 角色圖與名冊頭像一起換（名冊頭像讀 `<id>.avatar.png`，手機沒有另外裁頭像）
fn save_image(root: &Path, world: &str, card: &str, png: &[u8]) -> Result<(), &'static str> {
    import::save_character_image(root, world, card, png)
        .and_then(|()| import::save_character_avatar(root, world, card, png))
        .map_err(|_| "E_UNSUPPORTED_FORMAT")
}

// ── 工作：自建卡、匯入卡檔、換圖 ──

pub(super) fn on_job(room: &Arc<Room>, id: String, job: Value) {
    if room.saved().cards_made.contains(&id) {
        return;
    }
    let room = room.clone();
    tokio::spawn(async move {
        let result = match job["kind"].as_str().unwrap_or_default() {
            "card_create" => make_card(&room, text(&job["name"]), text(&job["blurb"]), None),
            "card_import" => import_card(&room, &job).await,
            _ => change_image(&room, &job).await,
        };
        let p = match result {
            Ok(card) => json!({ "jobId": id, "ok": true, "result": { "cardId": card } }),
            Err(code) => json!({ "jobId": id, "ok": false, "error": { "code": code } }),
        };
        room.durable("job.result", p, |saved| {
            saved.cards_made.push(id);
            let excess = saved.cards_made.len().saturating_sub(20);
            saved.cards_made.drain(..excess);
        });
    });
}

/// 在桌上建一張玩家卡（名字＋公開段，可帶顯示圖）、送新名冊、回卡 id；圖寫不進去就收掉剛建的卡
fn make_card(
    room: &Room,
    name: String,
    public_md: String,
    image: Option<&[u8]>,
) -> Result<String, &'static str> {
    let (root, world) = roots(room);
    let card = data::CharacterCard {
        id: data::new_id(),
        name,
        color: "#6b7280".to_owned(),
        avatar: "🙂".to_owned(),
        tier: data::Tier::Balanced,
        show_image: true,
        archived: false,
        gen_prompt: String::new(),
        public_md,
        private_md: String::new(),
        stage: false,
    };
    {
        let _permit = data::world_write_permit(&world).map_err(|_| "E_INTERNAL")?;
        data::write_character(&root, &world, &card).map_err(|_| "E_NAME_INVALID")?;
        if let Some(png) = image {
            if let Err(code) = save_image(&root, &world, &card.id, png) {
                let _ = data::delete_character(&root, &world, &card.id);
                return Err(code);
            }
        }
    }
    resend_roster(room);
    Ok(card.id)
}

/// 帶執行端憑證跟伺服器取上傳本體（取一次就刪）；長度要等於派工說的、也在上限內
async fn fetch(room: &Room, job: &Value, max: usize) -> Result<Vec<u8>, &'static str> {
    let upload = text(&job["uploadId"]);
    let size = job["size"].as_u64().unwrap_or(0) as usize;
    if upload.len() != 26
        || !upload.chars().all(|c| c.is_ascii_alphanumeric())
        || size == 0
        || size > max
    {
        return Err("E_TOO_LARGE");
    }
    let saved = room.saved();
    let url = format!(
        "{}/api/rooms/{}/cards/{upload}",
        saved.server, saved.room_code
    );
    let mut response = reqwest::Client::new()
        .get(url)
        .bearer_auth(&saved.executor_token)
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .ok()
        .filter(|response| response.status().is_success())
        .ok_or("E_STATE_CONFLICT")?;
    let mut bytes = Vec::with_capacity(size);
    while let Some(chunk) = response.chunk().await.map_err(|_| "E_STATE_CONFLICT")? {
        if bytes.len() + chunk.len() > size {
            return Err("E_TOO_LARGE");
        }
        bytes.extend_from_slice(&chunk);
    }
    match bytes.len() == size {
        true => Ok(bytes),
        false => Err("E_STATE_CONFLICT"),
    }
}

/// 玩家匯入：不走桌面版匯入（那條會先存原始來源、原卡、擴充與世界書），只建一張玩家卡
async fn import_card(room: &Room, job: &Value) -> Result<String, &'static str> {
    let bytes = fetch(room, job, CARD_MAX).await?;
    let lang = crate::transport::ui_language(
        &data::read_config(&room.engine.roots().0).unwrap_or_default(),
    );
    let card = import::player_card_from_bytes(&bytes, &lang).map_err(|_| "E_CARD_INVALID")?;
    make_card(room, card.name, card.public_md, card.image.as_deref())
}

async fn change_image(room: &Room, job: &Value) -> Result<String, &'static str> {
    let card = text(&job["cardId"]);
    seated_card(room, &text(&job["memberId"]), &card)?;
    let png = fetch(room, job, IMAGE_MAX).await?;
    import::check_character_image(&png).map_err(|_| "E_UNSUPPORTED_FORMAT")?;
    let (root, world) = roots(room);
    {
        let _permit = data::world_write_permit(&world).map_err(|_| "E_INTERNAL")?;
        seated_card(room, &text(&job["memberId"]), &card)?;
        save_image(&root, &world, &card, &png)?;
    }
    resend_roster(room);
    Ok(card)
}

// ── 讀卡、存卡 ──

fn preview(png: &[u8]) -> Option<String> {
    thumbnail(png, PREVIEW_PX)
}

fn thumbnail(png: &[u8], px: u32) -> Option<String> {
    let image = image::load_from_memory(png).ok()?.thumbnail(px, px);
    let mut out = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(out)
    ))
}

pub(super) fn on_read(room: &Room, p: &Value) {
    let req = text(&p["reqId"]);
    let reply = match seated_card(room, &text(&p["memberId"]), &text(&p["cardId"])) {
        Err(code) => json!({ "reqId": req, "ok": false, "code": code }),
        Ok(card) => {
            let (root, world) = roots(room);
            let image = import::character_image(&root, &world, &card.id)
                .ok()
                .flatten()
                .and_then(|b64| base64::engine::general_purpose::STANDARD.decode(b64).ok())
                .and_then(|png| preview(&png));
            let files = crate::commands::image::list_gallery_image_files(&root, &world, &card.id)
                .unwrap_or_default();
            let source = import::character_image_source(&root, &world, &card.id)
                .filter(|file| files.contains(file));
            json!({ "reqId": req, "ok": true, "card": {
                "name": card.name, "avatar": card.avatar, "publicMd": card.public_md,
                "privateMd": card.private_md, "image": image,
                "gallery": files.len(), "galleryFile": source,
            }})
        }
    };
    let frame = json!({ "t": "card.read.result", "p": reply });
    match frame.to_string().len() <= READ_FRAME_MAX {
        true => room.send("card.read.result", frame["p"].clone()),
        false => room.send(
            "card.read.result",
            json!({ "reqId": req, "ok": false, "code": "E_TEXT_INVALID" }),
        ),
    }
}

/// 整則序列化後的 UTF-8 位元組數（伺服器轉給手機還會再包一層，所以上限是 `READ_FRAME_MAX`）
fn frame_len(t: &str, p: &Value) -> usize {
    json!({ "t": t, "p": p }).to_string().len()
}

/// 名字要在這張卡的圖庫清單裡才給路徑：手機送回的檔名只認執行端自己列得出來的
fn gallery_file(
    root: &Path,
    world: &str,
    card: &str,
    name: &str,
) -> Result<std::path::PathBuf, &'static str> {
    let files = crate::commands::image::list_gallery_image_files(root, world, card)
        .map_err(|_| "E_STATE_CONFLICT")?;
    if !files.iter().any(|known| known == name) {
        return Err("E_FORBIDDEN");
    }
    Ok(data::gallery_dir(root, world, card)
        .map_err(|_| "E_INTERNAL")?
        .join(name))
}

/// 列自己座位那張卡的圖庫（一頁）或取單張預覽；解縮圖吃 CPU，整段在 blocking 工作裡跑
pub(super) fn on_gallery(room: &Arc<Room>, p: &Value) {
    let (room, p) = (room.clone(), p.clone());
    tokio::task::spawn_blocking(move || {
        let req = text(&p["reqId"]);
        let reply = gallery(&room, &p, &req)
            .unwrap_or_else(|code| json!({ "reqId": req, "ok": false, "code": code }));
        room.send("card.gallery.result", reply);
    });
}

fn gallery(room: &Room, p: &Value, req: &str) -> Result<Value, &'static str> {
    const T: &str = "card.gallery.result";
    let card = seated_card(room, &text(&p["memberId"]), &text(&p["cardId"]))?.id;
    let (root, world) = roots(room);
    if let Some(file) = p["file"].as_str() {
        let png = std::fs::read(gallery_file(&root, &world, &card, file)?)
            .map_err(|_| "E_STATE_CONFLICT")?;
        let thumb = preview(&png).ok_or("E_UNSUPPORTED_FORMAT")?;
        let reply =
            json!({ "reqId": req, "ok": true, "items": [{ "file": file, "thumb": thumb }] });
        return match frame_len(T, &reply) <= READ_FRAME_MAX {
            true => Ok(reply),
            false => Err("E_TOO_LARGE"),
        };
    }
    let files = crate::commands::image::list_gallery_image_files(&root, &world, &card)
        .map_err(|_| "E_STATE_CONFLICT")?;
    let dir = data::gallery_dir(&root, &world, &card).map_err(|_| "E_INTERNAL")?;
    // 新的在前：`before` 只拿來比大小，不組路徑
    let mut next = match p["before"].as_str() {
        Some(before) => files
            .iter()
            .position(|f| f.as_str() < before)
            .unwrap_or(files.len()),
        None => 0,
    };
    // 固定包裝先用最大的 more 算進去；之後每加一筆算它序列化後的長度（含逗號）
    let mut used = frame_len(
        T,
        &json!({ "reqId": req, "ok": true, "items": [], "more": files.len() }),
    );
    let mut items = Vec::new();
    while next < files.len() && items.len() < GALLERY_PAGE {
        let file = &files[next];
        // 解不開的檔跳過：不佔頁數、不算進 more
        let Some(thumb) = std::fs::read(dir.join(file))
            .ok()
            .and_then(|png| thumbnail(&png, THUMB_PX))
        else {
            next += 1;
            continue;
        };
        let item = json!({ "file": file, "thumb": thumb });
        let size = item.to_string().len() + 1;
        if used + size > READ_FRAME_MAX {
            if items.is_empty() {
                next += 1; // 單獨一張就放不進空頁：跳過
                continue;
            }
            break; // 這張留到下一頁
        }
        used += size;
        items.push(item);
        next += 1;
    }
    // 空頁只會發生在看到底的時候，所以 more 一定是 0
    let reply = json!({ "reqId": req, "ok": true, "items": items, "more": files.len() - next });
    match frame_len(T, &reply) <= READ_FRAME_MAX {
        true => Ok(reply),
        false => Err("E_TOO_LARGE"),
    }
}

/// 裁切框（原圖比例 0–1）：沒帶回 None；帶了就要四欄都是有限數字、在原圖範圍內（伺服器驗過，這裡再驗一次）
fn crop_box(value: &Value) -> Result<Option<[f64; 4]>, &'static str> {
    if value.is_null() {
        return Ok(None);
    }
    let field = |k: &str| {
        value[k]
            .as_f64()
            .filter(|n| n.is_finite())
            .ok_or("E_TEXT_INVALID")
    };
    let [x, y, w, h] = [field("x")?, field("y")?, field("w")?, field("h")?];
    match x >= 0.0 && y >= 0.0 && w > 0.0 && h > 0.0 && x + w <= 1.0001 && y + h <= 1.0001 {
        true => Ok(Some([x, y, w, h])),
        false => Err("E_TEXT_INVALID"),
    }
}

/// 照桌面版裁切窗：依框切原圖、寬超過 1024 就縮，存 PNG
fn cropped(png: &[u8], [x, y, w, h]: [f64; 4]) -> Result<Vec<u8>, &'static str> {
    let image = image::load_from_memory(png).map_err(|_| "E_UNSUPPORTED_FORMAT")?;
    let (iw, ih) = (f64::from(image.width()), f64::from(image.height()));
    let (left, top) = ((x * iw).round().min(iw), (y * ih).round().min(ih));
    let (right, bottom) = (
        ((x + w) * iw).round().min(iw),
        ((y + h) * ih).round().min(ih),
    );
    if right - left < 1.0 || bottom - top < 1.0 {
        return Err("E_TEXT_INVALID");
    }
    let mut out = image.crop_imm(
        left as u32,
        top as u32,
        (right - left) as u32,
        (bottom - top) as u32,
    );
    if out.width() > CROP_MAX_WIDTH {
        out = out.resize(
            CROP_MAX_WIDTH,
            u32::MAX,
            image::imageops::FilterType::Lanczos3,
        );
    }
    let mut bytes = Vec::new();
    out.write_to(
        &mut std::io::Cursor::new(&mut bytes),
        image::ImageFormat::Png,
    )
    .map_err(|_| "E_INTERNAL")?;
    import::check_character_image(&bytes).map_err(|_| "E_UNSUPPORTED_FORMAT")?;
    Ok(bytes)
}

/// 測試用：讓「裁切做完、寫入前」停在這裡，測試在這段換座再放行（只攔指定那張卡）
#[cfg(test)]
pub(super) type WriteHold = (
    String,
    std::sync::mpsc::Sender<()>,
    std::sync::mpsc::Receiver<()>,
);
#[cfg(test)]
pub(super) static HOLD_BEFORE_WRITE: std::sync::Mutex<Option<WriteHold>> =
    std::sync::Mutex::new(None);
#[cfg(test)]
fn hold_before_write(card: &str) {
    let mut slot = HOLD_BEFORE_WRITE.lock().unwrap();
    if slot.as_ref().is_some_and(|(held, _, _)| held == card) {
        let (_, reached, resume) = slot.take().unwrap();
        drop(slot);
        let _ = reached.send(());
        let _ = resume.recv_timeout(Duration::from_secs(10));
    }
}

/// 長文欄：≤ 64,000 字元、`\r\n` 正規化、控制字元只允許 `\n` 與 `\t`（伺服器驗過，這裡再驗一次）
fn long_text(value: &Value) -> Result<Option<String>, &'static str> {
    let Some(raw) = value.as_str() else {
        return match value.is_null() {
            true => Ok(None),
            false => Err("E_TEXT_INVALID"),
        };
    };
    let text = raw.replace("\r\n", "\n");
    let bad = text
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t');
    match bad || text.chars().count() > TEXT_MAX {
        true => Err("E_TEXT_INVALID"),
        false => Ok(Some(text)),
    }
}

/// 帶圖庫圖的存檔（解碼、裁切、PNG 驗證都吃 CPU）連寫入整段在 blocking 工作裡跑，不卡連線迴圈；
/// 只改文字或移除圖維持同步
pub(super) fn on_save(room: &Arc<Room>, p: &Value) {
    let reply = |room: &Room, p: &Value| {
        let req = text(&p["reqId"]);
        let reply = match save(room, p) {
            Ok(()) => json!({ "reqId": req, "ok": true }),
            Err(code) => json!({ "reqId": req, "ok": false, "code": code }),
        };
        room.send("card.save.result", reply);
    };
    if p["imageFromGallery"].is_string() {
        let (room, p) = (room.clone(), p.clone());
        tokio::task::spawn_blocking(move || reply(&room, &p));
    } else {
        reply(room, p);
    }
}

/// 只覆寫送來的欄位，沒送的照磁碟原值
fn save(room: &Room, p: &Value) -> Result<(), &'static str> {
    let public = long_text(&p["publicMd"])?;
    let private = long_text(&p["privateMd"])?;
    let name = p["name"].as_str().map(str::trim);
    if name.is_some_and(|name| {
        name.is_empty() || name.chars().count() > 40 || name.contains(['\n', '\r'])
    }) {
        return Err("E_NAME_INVALID");
    }
    let avatar = p["avatar"].as_str();
    if avatar.is_some_and(|a| {
        a.is_empty()
            || a.chars().count() > 16
            || a.chars().any(|c| c.is_whitespace() || c.is_control())
    }) {
        return Err("E_TEXT_INVALID");
    }
    let crop = crop_box(&p["crop"])?;
    let member = text(&p["memberId"]);
    let (root, world) = roots(room);
    // 鎖外先做完：驗座位、驗檔名、讀檔、裁切；寫入前在鎖底下再驗一次座位
    let gallery = match p["imageFromGallery"].as_str() {
        None if crop.is_some() => return Err("E_TEXT_INVALID"),
        None => None,
        Some(file) => {
            let card = seated_card(room, &member, &text(&p["cardId"]))?.id;
            let png = std::fs::read(gallery_file(&root, &world, &card, file)?)
                .map_err(|_| "E_STATE_CONFLICT")?;
            let png = match crop {
                Some(crop) => cropped(&png, crop)?,
                None => png,
            };
            Some((file, png))
        }
    };
    #[cfg(test)]
    hold_before_write(&text(&p["cardId"]));
    {
        let _permit = data::world_write_permit(&world).map_err(|_| "E_INTERNAL")?;
        let mut card = seated_card(room, &member, &text(&p["cardId"]))?;
        if let Some(name) = name {
            card.name = name.to_owned();
        }
        if let Some(avatar) = avatar {
            card.avatar = avatar.to_owned();
        }
        card.public_md = public.unwrap_or(card.public_md);
        card.private_md = private.unwrap_or(card.private_md);
        data::write_character(&root, &world, &card).map_err(|_| "E_NAME_INVALID")?;
        if p["removeImage"] == true {
            import::delete_character_image(&root, &world, &card.id)
                .and_then(|()| import::delete_character_avatar(&root, &world, &card.id))
                .map_err(|_| "E_INTERNAL")?;
        }
        if let Some((file, png)) = gallery {
            save_image(&root, &world, &card.id, &png)?;
            import::save_image_source(&root, &world, &card.id, file).map_err(|_| "E_INTERNAL")?;
        }
    }
    resend_roster(room);
    Ok(())
}

// ── 生圖 ──

/// 生圖錯誤 → 手機看得懂的三類（沒額度、來源不能生圖、被拒），其餘當內部錯
fn gen_code(error: &str) -> &'static str {
    let code = run::classify(error)["code"]
        .as_str()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let has = |needles: &[&str]| needles.iter().any(|needle| code.contains(needle));
    if has(&["FREE_KEY", "STATUS_402", "STATUS_429", "FREE_MODEL_BUSY"]) {
        "E_GEN_QUOTA"
    } else if has(&["CONTENT_FILTERED"]) {
        "E_GEN_REFUSED"
    } else if has(&[
        "IMAGE_MISSING",
        "NO_IMAGE",
        "UNSUPPORTED",
        "CLI_NOT_FOUND",
        "STATUS_401",
        "STATUS_403",
    ]) {
        "E_GEN_UNSUPPORTED"
    } else {
        "E_INTERNAL"
    }
}

/// 本機偏好的生圖來源：桌面版生圖窗存的 `image_source`，沒存過就跟聊天連線（聊天走不會生圖的 CLI 時退 API，同生圖窗）
fn image_source(config: &data::AppConfig) -> String {
    let pref = |key: &str| {
        config
            .preferences
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    match pref("image_source") {
        saved if !saved.is_empty() => saved,
        _ => match pref("transport").as_str() {
            "" | "claude" => "api".to_owned(),
            other => other.to_owned(),
        },
    }
}

/// 房間生圖有沒有路線：本機生圖來源是 API 才有（見 `on_gen`）；沒有就桌面不給開關、手機不給生圖段
pub(super) fn gen_route(room: &Room) -> bool {
    let config = data::read_config(&room.engine.roots().0).unwrap_or_default();
    image_source(&config) == "api"
}

pub(super) fn on_gen(room: &Arc<Room>, p: &Value) {
    let req = text(&p["reqId"]);
    let fail = |code: &str| {
        room.send(
            "image.gen.result",
            json!({ "reqId": req, "ok": false, "code": code }),
        )
    };
    if room.saved().image_gen_off {
        return fail("E_FORBIDDEN");
    }
    let card = match seated_card(room, &text(&p["memberId"]), &text(&p["cardId"])) {
        Ok(card) => card,
        Err(code) => return fail(code),
    };
    // 房間來的生圖只走 API：CLI 生圖路線（codex workspace-write、agy 略過權限、grok 全核准）工具沒鎖到只剩生圖，
    // 遠端玩家的文字不能交給它們。本機偏好的生圖來源不是 API 就不生（不偷偷換成別的帳號）
    if !gen_route(room) {
        return fail("E_GEN_UNSUPPORTED");
    }
    if std::mem::replace(&mut room.work().generating, true) {
        return fail("E_STATE_CONFLICT");
    }
    let request = ImageRequest {
        world_id: room.world(),
        character_id: card.id,
        name: card.name.clone(),
        description: text(&p["description"]).chars().take(3000).collect(),
        extra_prompt: text(&p["extraPrompt"]).chars().take(1000).collect(),
        source: Some("api".to_owned()),
        framing: p["framing"].as_str().map(str::to_owned),
        untrusted: true,
    };
    let room = room.clone();
    tokio::spawn(async move {
        let result = room.engine.generate_image(request).await;
        room.work().generating = false;
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        {
            let mut status = room.status();
            status.gen_log.push(GenLog {
                at,
                name: card.name,
                ok: result.is_ok(),
            });
            let excess = status.gen_log.len().saturating_sub(20);
            status.gen_log.drain(..excess);
        }
        let reply = match result.map(|(file, png)| (file, preview(&png))) {
            Ok((file, Some(preview))) => {
                json!({ "reqId": req, "ok": true, "file": file, "preview": preview })
            }
            Ok((_, None)) => json!({ "reqId": req, "ok": false, "code": "E_INTERNAL" }),
            Err(error) => json!({ "reqId": req, "ok": false, "code": gen_code(&error) }),
        };
        room.send("image.gen.result", reply);
    });
}
