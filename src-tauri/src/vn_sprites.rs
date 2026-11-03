//! VN 立繪管線（plans/vn-mode-pkg7.md）：透明七分身定稿 → 臉框 → 裁頭重繪四張表情臉層；
//! 變體身體（共用或獨立表情組）。來源 Codex（原生透明）、agy／API（灰底，經 matting 去背）。
//! 生成期間不持桌寫入許可，寫回前重驗（store::Snapshot、世代號）。
mod compose;
pub(crate) mod matting;
pub(crate) mod queue;
pub(crate) mod store;

use crate::generated_image::{self, cli_fetch, png_data_url, GeneratedImage};
use crate::transport::{self, vn_sprite_prompts as prompts};
use crate::{config_root, data, data_root};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use store::{FaceBox, SpriteDir};

/// 立繪來源：偏好 `vn_sprite_source`（codex／agy／api），沒設或不認得（例如 grok）＝codex
pub(crate) fn sprite_source(config: &data::AppConfig) -> &'static str {
    let wanted = config
        .preferences
        .get("vn_sprite_source")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    ["codex", "agy", "api"]
        .into_iter()
        .find(|source| *source == wanted)
        .unwrap_or("codex")
}

/// 只有 Codex 出得了真透明；其他來源要灰底再去背
fn transparent(source: &str) -> bool {
    source == "codex"
}

/// 生一張圖（同一時間一張），回 normalize 過的 PNG。API 的參考圖走 `input_references`
pub(crate) async fn generate_png(
    app: &tauri::AppHandle,
    config: &data::AppConfig,
    world: &str,
    prompt: &str,
    reference: Option<&[u8]>,
    aspect_ratio: &str,
) -> Result<Vec<u8>, String> {
    let source = sprite_source(config);
    let image = {
        let _gate = source_gate().lock().await;
        match source {
            "api" => {
                let reference = reference.map(png_data_url);
                GeneratedImage::Encoded(
                    transport::generate_image_with(
                        config,
                        prompt,
                        aspect_ratio,
                        reference.as_deref(),
                    )
                    .await?,
                )
            }
            kind => {
                cli_fetch::run_cli_image(app, config, kind, Some(world), prompt, reference).await?
            }
        }
    };
    generated_image::normalize(image).await
}

/// 同一來源一次一張（定稿候選與臉層共用）
fn source_gate() -> &'static tokio::sync::Mutex<()> {
    static GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    &GATE
}

pub(crate) fn read_config(app: &tauri::AppHandle) -> Result<data::AppConfig, String> {
    data::read_config(&config_root(app)?).map_err(|error| error.to_string())
}

pub(crate) fn character_exists(root: &std::path::Path, world: &str, character: &str) -> bool {
    data::character_path(root, world, character).is_ok_and(|path| path.is_file())
}

/// 生一張定稿候選：角色名＋公開設定，有卡圖就當外觀參考。回候選檔名
pub(crate) async fn generate_candidate(
    app: &tauri::AppHandle,
    world: &str,
    character: &str,
) -> Result<String, String> {
    let root = data_root(app)?;
    let config = read_config(app)?;
    let card = data::read_character(&root, world, character).map_err(|error| error.to_string())?;
    let reference = data::character_path(&root, world, character)
        .ok()
        .and_then(|path| std::fs::read(path.with_extension("png")).ok());
    let source = sprite_source(&config);
    let prompt = prompts::base_prompt(
        &card.name,
        &card.public_md,
        reference.is_some(),
        transparent(source),
    );
    let png = generate_png(app, &config, world, &prompt, reference.as_deref(), "2:3").await?;
    let png = matting::ensure_alpha(app, png).await?;
    let _permit = data::world_write_permit_async(world).await?;
    if !character_exists(&root, world, character) {
        return Err("VN_SPRITE_CHARACTER_GONE".to_owned());
    }
    SpriteDir::of(&root, world, character)?.add_candidate(&png)
}

/// 視覺模型回的臉框：只認圖內、面積合理的；上緣再往上加框高 25%（挑眉才不會被切）
pub(crate) fn parse_face_box(reply: &str, width: u32, height: u32) -> Option<FaceBox> {
    let start = reply.rfind('{')?;
    let end = start + reply[start..].find('}')?;
    let value: serde_json::Value = serde_json::from_str(&reply[start..=end]).ok()?;
    let field = |key: &str| value.get(key)?.as_i64();
    let (x0, y0, x1, y1) = (field("x0")?, field("y0")?, field("x1")?, field("y1")?);
    let (w, h) = (i64::from(width), i64::from(height));
    if x0 < 0 || y0 < 0 || x1 > w || y1 > h || x0 >= x1 || y0 >= y1 {
        return None;
    }
    let area = ((x1 - x0) * (y1 - y0)) as f64 / (w * h) as f64;
    if !(0.002..=0.2).contains(&area) {
        return None;
    }
    let top = (y0 - ((y1 - y0) as f64 * 0.25).round() as i64).max(0);
    Some(FaceBox {
        x: x0 as u32,
        y: top as u32,
        w: (x1 - x0) as u32,
        h: (y1 - top) as u32,
    })
}

/// 臉框：視覺模型一次，不合格重問一次，再不行用預設框（回 true＝預設框，立繪分頁提示手調）
async fn locate_face(
    app: &tauri::AppHandle,
    config: &data::AppConfig,
    world: &str,
    png: &[u8],
    (width, height): (u32, u32),
) -> (FaceBox, bool) {
    for _ in 0..2 {
        let prompt = prompts::face_box_prompt(width, height);
        match cli_fetch::run_cli_vision(app, config, Some(world), &prompt, png).await {
            Ok(reply) => match parse_face_box(&reply, width, height) {
                Some(face) => return (face, false),
                None => log::warn!(
                    "臉框回覆不合格：{}",
                    reply.chars().take(200).collect::<String>()
                ),
            },
            Err(error) => log::warn!("臉框呼叫失敗：{error}"),
        }
    }
    (FaceBox::fallback(width, height), true)
}

/// 每位角色的定稿世代：挑定／上傳開工時加一並記下，刪除也加一；寫入前世代沒變才提交，
/// 晚回來的舊臉框呼叫不會蓋掉新定稿或讓已刪的立繪復活。只在記憶體：重開 app 時沒有在途請求。
fn base_epochs() -> std::sync::MutexGuard<'static, HashMap<(String, String), u64>> {
    static EPOCHS: OnceLock<Mutex<HashMap<(String, String), u64>>> = OnceLock::new();
    EPOCHS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 讓在途的換定稿作廢，回新世代
pub(crate) fn supersede_base(world: &str, character: &str) -> u64 {
    let mut epochs = base_epochs();
    let epoch = epochs
        .entry((world.to_owned(), character.to_owned()))
        .or_default();
    *epoch += 1;
    *epoch
}

fn epoch_of(world: &str, character: &str) -> u64 {
    base_epochs()
        .get(&(world.to_owned(), character.to_owned()))
        .copied()
        .unwrap_or(0)
}

fn base_is_current(world: &str, character: &str, epoch: u64) -> bool {
    epoch_of(world, character) == epoch
}

/// 變體的世代用「角色/變體鍵」記（角色 id 不含斜線）
fn variant_epoch_key(character: &str, key: &str) -> String {
    format!("{character}/{key}")
}

/// 換定稿（挑定候選或上傳）：定臉框後寫入，舊臉層全部作廢；開工後有更新的換定稿或刪除就丟掉
pub(crate) async fn install_base(
    app: &tauri::AppHandle,
    world: &str,
    character: &str,
    png: Vec<u8>,
) -> Result<(), String> {
    let epoch = supersede_base(world, character);
    let root = data_root(app)?;
    let config = read_config(app)?;
    let png = matting::ensure_alpha(app, png).await?;
    let size = store::png_size(&png).ok_or("AI_IMAGE_DECODE_FAILED")?;
    let (face, fallback) = locate_face(app, &config, world, &png, size).await;
    let _permit = data::world_write_permit_async(world).await?;
    if !base_is_current(world, character, epoch) {
        return Ok(());
    }
    if !character_exists(&root, world, character) {
        return Err("VN_SPRITE_CHARACTER_GONE".to_owned());
    }
    SpriteDir::of(&root, world, character)?.install_base(&png, face, fallback)?;
    queue::forget(world, character);
    Ok(())
}

/// 一格臉層：裁頭 → 生圖 → 對齊檢查（不過自動重骰一次）→ 羽化 → 重驗後寫回。
/// Ok(false)＝開工後定稿、框或該格被換掉（或角色已刪），結果丟掉；過期任務的失敗也算這種，不記失敗。
pub(crate) async fn face_job(
    app: &tauri::AppHandle,
    world: &str,
    character: &str,
    set: Option<&str>,
    expression: &str,
) -> Result<bool, String> {
    let root = data_root(app)?;
    let dir = SpriteDir::of(&root, world, character)?;
    if !character_exists(&root, world, character) {
        return Ok(false);
    }
    let Some((snapshot, body)) = dir.snapshot(set, expression) else {
        return Ok(false);
    };
    match render_face(app, world, expression, snapshot.face(), &body).await {
        Ok(layer) => {
            let _permit = data::world_write_permit_async(world).await?;
            if !character_exists(&root, world, character) {
                return Ok(false);
            }
            dir.write_face_if_current(set, expression, &snapshot, &layer)
        }
        Err(reason) => stale_or_failed(
            reason,
            &snapshot,
            dir.snapshot(set, expression).map(|(now, _)| now),
        ),
    }
}

/// 失敗只在任務仍是目前這格時才算失敗；開工後被換掉＝過期，回 Ok(false) 交給佇列依現況重排
fn stale_or_failed(
    reason: String,
    started: &store::Snapshot,
    now: Option<store::Snapshot>,
) -> Result<bool, String> {
    match now {
        Some(now) if now == *started => Err(reason),
        _ => Ok(false),
    }
}

async fn render_face(
    app: &tauri::AppHandle,
    world: &str,
    expression: &str,
    face: FaceBox,
    body: &[u8],
) -> Result<Vec<u8>, String> {
    let config = read_config(app)?;
    let transparent = transparent(sprite_source(&config));
    let prompt =
        prompts::expression_prompt(expression, transparent).ok_or("VN_SPRITE_EXPRESSION")?;
    let base = compose::decode(body)?;
    let crop = compose::Crop::around(face);
    let inner = crop.inner(face);
    let before = compose::cut(&base, crop);
    // 灰底來源回來的方塊不透明：臉層以定稿 alpha 裁剪，不必去背
    let request = compose::request_png(&base, crop, !transparent)?;
    for _ in 0..2 {
        let png = generate_png(app, &config, world, &prompt, Some(&request), "1:1").await?;
        let result = compose::decode(&png)?;
        let Some(after) = compose::fit_back(&result, crop.side) else {
            continue;
        };
        if compose::aligned(&before, &after, inner) {
            return compose::encode_png(&compose::face_layer(&after, &before, inner));
        }
    }
    Err("VN_SPRITE_MISALIGNED".to_owned())
}

/// 玩家上傳整張表情圖替換一格：比例要跟該組身體圖一樣，照臉框取臉層
pub(crate) fn upload_face(
    root: &std::path::Path,
    world: &str,
    character: &str,
    set: Option<&str>,
    expression: &str,
    png: &[u8],
) -> Result<(), String> {
    let dir = SpriteDir::of(root, world, character)?;
    let (snapshot, body) = dir.snapshot(set, expression).ok_or("VN_SPRITE_MISSING")?;
    let base = compose::decode(&body)?;
    let layer = compose::face_from_full(&compose::decode(png)?, &base, snapshot.face())
        .ok_or("VN_SPRITE_UPLOAD_ASPECT")?;
    dir.write_face(set, expression, &compose::encode_png(&layer)?)?;
    queue::forget_slot(world, character, set, expression);
    Ok(())
}

/// 變體名：1–24 字、不含劇本括號語法用到的字元、不叫 base
pub(crate) fn valid_label(label: &str) -> bool {
    (1..=24).contains(&label.chars().count())
        && !label.eq_ignore_ascii_case("base")
        && !label.contains(['@', '(', ')', '（', '）', ',', '，', ':', '：', '\n'])
}

/// 新增或重骰變體（`key` 有值＝重骰那個）：以定稿為參考圖，變體名就是服裝／姿態描述
pub(crate) async fn generate_variant(
    app: &tauri::AppHandle,
    world: &str,
    character: &str,
    key: Option<String>,
    label: String,
) -> Result<(), String> {
    let config = read_config(app)?;
    let dir = SpriteDir::of(&data_root(app)?, world, character)?;
    let body = dir.read().ok_or("VN_SPRITE_MISSING")?.body;
    let prompt = prompts::variant_prompt(&label, transparent(sprite_source(&config)));
    let epochs = start_variant(&dir, world, character, key, &label)?;
    let png = generate_png(app, &config, world, &prompt, Some(&body), "2:3").await?;
    place_variant(app, world, character, epochs, label, png, true).await
}

/// 玩家上傳變體身體：一律獨立表情組
pub(crate) async fn upload_variant(
    app: &tauri::AppHandle,
    world: &str,
    character: &str,
    key: Option<String>,
    label: String,
    png: Vec<u8>,
) -> Result<(), String> {
    let dir = SpriteDir::of(&data_root(app)?, world, character)?;
    let epochs = start_variant(&dir, world, character, key, &label)?;
    place_variant(app, world, character, epochs, label, png, false).await
}

/// 開工：驗名稱、定鍵、記下定稿世代並作廢同一變體的在途工作
fn start_variant(
    dir: &SpriteDir,
    world: &str,
    character: &str,
    key: Option<String>,
    label: &str,
) -> Result<(String, u64, u64), String> {
    let meta = dir.read().ok_or("VN_SPRITE_MISSING")?.meta;
    let taken = meta
        .variants
        .iter()
        .any(|variant| variant.label == label && Some(&variant.key) != key.as_ref());
    if !valid_label(label) || taken || key.as_ref().is_some_and(|key| meta.variant(key).is_none()) {
        return Err("VN_SPRITE_VARIANT".to_owned());
    }
    let key = key.unwrap_or_else(|| meta.next_variant_key());
    let epoch = supersede_base(world, &variant_epoch_key(character, &key));
    Ok((key, epoch_of(world, character), epoch))
}

/// 去背 → 比臉框內決定共用或獨立表情組（獨立組另定臉框）→ 重驗世代後寫入
async fn place_variant(
    app: &tauri::AppHandle,
    world: &str,
    character: &str,
    (key, base_epoch, epoch): (String, u64, u64),
    label: String,
    png: Vec<u8>,
    may_share: bool,
) -> Result<(), String> {
    let root = data_root(app)?;
    let config = read_config(app)?;
    let png = matting::ensure_alpha(app, png).await?;
    let dir = SpriteDir::of(&root, world, character)?;
    let loaded = dir.read().ok_or("VN_SPRITE_MISSING")?;
    let shared = match may_share {
        true => compose::shared_body(
            &compose::decode(&loaded.body)?,
            &compose::decode(&png)?,
            loaded.meta.face,
        ),
        false => None,
    };
    let (png, face) = match shared {
        Some(body) => (compose::encode_png(&body)?, None),
        None => {
            let size = store::png_size(&png).ok_or("AI_IMAGE_DECODE_FAILED")?;
            let (face, _) = locate_face(app, &config, world, &png, size).await;
            (png, Some(face))
        }
    };
    let _permit = data::world_write_permit_async(world).await?;
    let current = base_is_current(world, character, base_epoch)
        && base_is_current(world, &variant_epoch_key(character, &key), epoch);
    if !current {
        return Ok(());
    }
    if !character_exists(&root, world, character) {
        return Err("VN_SPRITE_CHARACTER_GONE".to_owned());
    }
    dir.install_variant(&key, &label, &png, face)?;
    queue::forget(world, character);
    Ok(())
}

/// 刪變體（在途的重骰一併作廢）。呼叫端持桌寫入許可
pub(crate) fn delete_variant(
    root: &std::path::Path,
    world: &str,
    character: &str,
    key: &str,
) -> Result<(), String> {
    supersede_base(world, &variant_epoch_key(character, key));
    SpriteDir::of(root, world, character)?.remove_variant(key)?;
    queue::forget(world, character);
    Ok(())
}

/// 提示詞用：送進提示詞的各角色可用變體名（只列有身體圖的）
pub(crate) fn variant_roster(
    root: &std::path::Path,
    world: &str,
    cards: &[data::CharacterCard],
) -> Vec<(String, Vec<String>)> {
    cards
        .iter()
        .filter_map(|card| {
            let labels = SpriteDir::of(root, world, &card.id).ok()?.variant_labels();
            (!labels.is_empty()).then(|| (card.name.clone(), labels))
        })
        .collect()
}

#[cfg(test)]
mod tests;
