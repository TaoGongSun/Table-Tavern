//! CLI 生圖的取圖層（plans/vn-mode-pkg7.md §4）：每個工作一個子工作目錄；圖先從各家自己的落點讀
//! （codex `generated_images/<thread>/`、agy `brain/<對話>/`），找不到才退回回覆裡的路徑，且只收
//! 落在本次目錄內的圖——並行中的別的工作的圖不會被錯抓。NO_IMAGE／REFUSED 只在沒圖時採信。
use super::GeneratedImage;
use crate::cli::home_cleanup::{self, is_uuid, real_dir, remove_entry};
use crate::transport::dispatch::{cli_workspace, grok_home_dir, stream_image_via_transport};
use crate::ui_msg::UiMsg;
use crate::{config_root, data, transport};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// 一次 CLI 呼叫的工作：自己的工作目錄（結束就整個刪掉）、參考圖、各家回填的 id。
pub(crate) struct CliJob {
    pub dir: PathBuf,
    pub reference: Option<PathBuf>,
    pub grok_session: Option<String>,
    pub codex_thread: Mutex<Option<String>>,
    pub agy_conversation: Mutex<Option<String>>,
}

impl CliJob {
    pub(crate) fn create(
        workspace: &Path,
        kind: &str,
        reference: Option<&[u8]>,
    ) -> Result<Self, String> {
        let dir = workspace.join("jobs").join(data::new_id());
        std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        let mut job = CliJob {
            dir,
            reference: None,
            // grok 會把原圖連同整份 session 存在 grok-home：自己給 id，圖讀到後刪掉
            grok_session: (kind == "grok").then(crate::lanes::new_session_id),
            codex_thread: Mutex::new(None),
            agy_conversation: Mutex::new(None),
        };
        if let Some(bytes) = reference {
            // grok：透明處壓淺灰底（變體送的是透明身體圖），存 JPEG——看圖那條用 read_file 讀圖，
            // 串流事件會把整張圖 base64 兩份塞進同一行，PNG 立繪會撞單行 1 MB 上限
            let (name, bytes) = match kind {
                "grok" => ("reference.jpg", grok_reference(bytes)?),
                _ => ("reference.png", bytes.to_vec()),
            };
            let path = job.dir.join(name);
            std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
            job.reference = Some(path);
        }
        Ok(job)
    }
}

impl Drop for CliJob {
    fn drop(&mut self) {
        let _ = remove_entry(&self.dir);
    }
}

/// 回填的 id 只收 UUID 形狀（單一路徑元件），才拿來組路徑
fn checked_id(slot: &Mutex<Option<String>>) -> Option<String> {
    slot.lock().ok()?.clone().filter(|id| is_uuid(id))
}

pub(crate) struct Homes {
    pub codex: PathBuf,
    pub agy: PathBuf,
    pub grok: PathBuf,
}

impl Homes {
    fn of(config_root: &Path) -> Self {
        Homes {
            codex: home_cleanup::codex_home_dir(config_root),
            agy: home_cleanup::agy_home_dir(config_root),
            grok: grok_home_dir(config_root),
        }
    }
}

/// 兩個暗號分開問：生不出（沒能力／沒額度）與不肯生（內容規範）要給玩家不同的下一步
const REPLY_RULE: &str = "\nIf you are able to generate images, generate it now, save it as a PNG file, and reply with the absolute file path of the saved image. If you cannot generate images at all, reply exactly: NO_IMAGE. If you decline this particular request, reply exactly: REFUSED";

/// CLI 生一張圖：補上回覆暗號（codex 前綴 `$imagegen`、agy 指名參考圖檔），跑完照取圖層拿圖。
pub(crate) async fn run_cli_image(
    app: &tauri::AppHandle,
    config: &data::AppConfig,
    kind: &str,
    world: Option<&str>,
    prompt: &str,
    reference: Option<&[u8]>,
) -> Result<GeneratedImage, String> {
    let job = CliJob::create(&cli_workspace(app)?, kind, reference)?;
    let mut prompt = prompt.to_owned();
    // agy 的 view_file 只收絕對路徑；圖像代理不帶預設組件，模型不知道 cwd（plans/agy-image-permissions.md §2.3）
    if let (true, Some(reference)) = (kind == "agy", &job.reference) {
        let path = reference.display();
        prompt = format!("Reference image file: {path} (look at it before generating). {prompt}");
    }
    // grok headless 沒有附圖旗標，image_gen 也不收參考圖：指名用 image_edit 帶這張圖的路徑
    if let (true, Some(reference)) = (kind == "grok", &job.reference) {
        let path = reference.display();
        prompt = format!("Reference image file: {path}. Use the image_edit tool with this file as its image (image_gen cannot take a reference image). {prompt}");
    }
    prompt.push_str(REPLY_RULE);
    if kind == "codex" {
        prompt = format!("$imagegen {prompt}");
    }
    let reply = stream_image_via_transport(
        app,
        config,
        Some(kind),
        transport::gm_tier(config),
        world,
        &[user(prompt)],
        &job,
        true,
    )
    .await;
    let homes = Homes::of(&config_root(app)?);
    match reply {
        Ok(reply) => take_image(kind, &job, &homes, &reply),
        Err(error) => {
            if kind == "agy" {
                if let Some(id) = checked_id(&job.agy_conversation) {
                    let _ = home_cleanup::remove_conversation(&homes.agy, &id);
                }
            }
            Err(error)
        }
    }
}

/// 看圖來源不跨供應商；API 目前文字傳輸不支援圖片，直接由呼叫端用預設框。
pub(crate) fn vision_source(source: &str) -> Option<&'static str> {
    match source {
        "codex" => Some("codex"),
        "agy" => Some("agy"),
        "grok" => Some("grok"),
        _ => None,
    }
}

/// 看圖回文字：codex 維持 -i；agy 用 view_file 專用代理、grok 只開 read_file，都不開生圖工具。
pub(crate) async fn run_cli_vision(
    app: &tauri::AppHandle,
    config: &data::AppConfig,
    source: &str,
    world: Option<&str>,
    prompt: &str,
    image: &[u8],
) -> Result<String, String> {
    // grok 進模型前本來就把圖縮到長邊約 768：先縮好，讀圖那行串流事件更小（座標用 0–1000 相對值，不受縮圖影響）
    let image = match source {
        "grok" => downscale_png(image, 768)?,
        _ => image.to_vec(),
    };
    let mut job = CliJob::create(&cli_workspace(app)?, source, Some(&image))?;
    // grok 看圖是一般單發：不指定 session，交給單發那條收完就刪
    job.grok_session = None;
    let prompt = match (source, &job.reference) {
        ("agy", Some(path)) => format!(
            "Use view_file to inspect the image at {}. {prompt}",
            path.display()
        ),
        ("grok", Some(path)) => format!(
            "Use read_file to inspect the image at {}. {prompt}",
            path.display()
        ),
        _ => prompt.to_owned(),
    };
    // agy 走快速檔：玩家有設快速檔型號時 6–13 秒；沒設則用 CLI 預設（實測中檔 34–49 秒，一樣準）
    let tier = match source {
        "agy" => data::Tier::Fast,
        _ => transport::gm_tier(config),
    };
    stream_image_via_transport(
        app,
        config,
        Some(source),
        tier,
        world,
        &[user(prompt)],
        &job,
        false,
    )
    .await
}

/// 長邊縮到 `long_edge` 以內的 PNG（本來就夠小就原樣回）
fn downscale_png(png: &[u8], long_edge: u32) -> Result<Vec<u8>, String> {
    let image = image::load_from_memory(png).map_err(|error| error.to_string())?;
    if image.width().max(image.height()) <= long_edge {
        return Ok(png.to_vec());
    }
    let mut bytes = Vec::new();
    image
        .resize(long_edge, long_edge, image::imageops::FilterType::Lanczos3)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}

/// grok 的參考圖：透明處壓淺灰底（217，同 compose::request_png）的 JPEG
fn grok_reference(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let rgba = image::load_from_memory(bytes)
        .map_err(|error| error.to_string())?
        .to_rgba8();
    let mut flat = image::RgbaImage::from_pixel(
        rgba.width(),
        rgba.height(),
        image::Rgba([217, 217, 217, 255]),
    );
    image::imageops::overlay(&mut flat, &rgba, 0, 0);
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 90)
        .encode_image(&image::DynamicImage::ImageRgba8(flat).to_rgb8())
        .map_err(|error| error.to_string())?;
    Ok(jpeg)
}

fn user(content: String) -> transport::ChatMessage {
    transport::ChatMessage {
        role: "user".to_owned(),
        content,
    }
}

/// 取圖：本家落點 → 回覆路徑（限本次目錄）→ 暗號。本家落點讀完就清（codex 刪 thread 圖目錄、
/// agy 刪整個生圖對話）；grok 只在拿到圖後刪 session（失敗那份留給 grok 自帶的定期清理）。
pub(crate) fn take_image(
    kind: &str,
    job: &CliJob,
    homes: &Homes,
    reply: &str,
) -> Result<GeneratedImage, String> {
    let agy_id = (kind == "agy")
        .then(|| checked_id(&job.agy_conversation))
        .flatten();
    let vendor_dir = match kind {
        "codex" => checked_id(&job.codex_thread)
            .and_then(|id| real_dir(&homes.codex, &["generated_images", &id]).ok()?),
        "agy" => agy_id.as_deref().and_then(|id| {
            real_dir(&homes.agy, &[".gemini", "antigravity-cli", "brain", id]).ok()?
        }),
        _ => None,
    };
    let found = vendor_dir
        .as_deref()
        .and_then(newest_image)
        .map(|path| std::fs::read(path).map_err(|error| error.to_string()))
        .map(|read| read.map(GeneratedImage::Bytes))
        .or_else(|| reply_image(reply, job, vendor_dir.as_deref(), &homes.grok));
    if let Some(dir) = vendor_dir.as_deref().filter(|_| kind == "codex") {
        if let Err(error) = remove_entry(dir) {
            log::warn!("刪不掉 codex 生圖目錄 {}：{error}", dir.display());
        }
    }
    if let Some(id) = &agy_id {
        if let Err(error) = home_cleanup::remove_conversation(&homes.agy, id) {
            log::warn!("刪不掉 agy 生圖對話 {id}：{error}");
        }
    }
    if let (Some(Ok(_)), Some(session)) = (&found, &job.grok_session) {
        match home_cleanup::remove_session(&homes.grok, session) {
            Ok(0) => log::warn!("找不到 grok 生圖 session {session}，沒有可清的目錄"),
            Ok(_) => {}
            Err(error) => log::warn!("刪不掉 grok 生圖 session {session}：{error}"),
        }
    }
    found.unwrap_or_else(|| Err(missing_reason(reply)))
}

/// 目錄裡最新的一張圖；多張只取最新並記一行（只呼叫一次的指示沒被照做）
fn newest_image(dir: &Path) -> Option<PathBuf> {
    let mut images: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| is_image_extension(path))
        // 不跟隨連結：落點裡指向外部的 .png 不收
        .filter_map(|path| {
            let meta = std::fs::symlink_metadata(&path).ok()?;
            meta.file_type()
                .is_file()
                .then_some((meta.modified().ok()?, path))
        })
        .collect();
    if images.len() > 1 {
        log::warn!("{} 有 {} 張圖，取最新一張", dir.display(), images.len());
    }
    images.sort();
    images.pop().map(|(_, path)| path)
}

/// 回覆裡的圖：data URL 直接收；路徑要解析後落在本次工作目錄、本家落點或本次 grok session 內
fn reply_image(
    reply: &str,
    job: &CliJob,
    vendor_dir: Option<&Path>,
    grok_home: &Path,
) -> Option<Result<GeneratedImage, String>> {
    let roots: Vec<PathBuf> = [Some(job.dir.as_path()), vendor_dir]
        .into_iter()
        .flatten()
        .filter_map(|dir| dir.canonicalize().ok())
        .collect();
    let grok_home = grok_home.canonicalize().ok();
    extract_image_refs(reply)
        .into_iter()
        .find_map(|found| match found {
            ImageRef::DataUrl(data_url) => Some(Ok(GeneratedImage::Encoded(data_url))),
            // codex 常回相對於工作目錄的路徑；絕對路徑 join 後維持原樣
            ImageRef::Path(path) => {
                let real = job.dir.join(path).canonicalize().ok()?;
                let in_session = job.grok_session.as_deref().is_some_and(|session| {
                    grok_home
                        .as_ref()
                        .is_some_and(|home| real.starts_with(home))
                        && real.components().any(|part| part.as_os_str() == session)
                });
                (roots.iter().any(|root| real.starts_with(root)) || in_session)
                    .then(|| std::fs::read(&real).map(GeneratedImage::Bytes))
                    .map(|read| read.map_err(|error| error.to_string()))
            }
        })
}

/// 沒圖時的錯誤：暗號換成前端認得的碼，都沒對上時附最後一句原話（模型不照暗號時拒絕理由多半在那）
fn missing_reason(reply: &str) -> String {
    if reply.contains("REFUSED") {
        "REFUSED".to_owned()
    } else if reply.contains("NO_IMAGE") {
        "NO_IMAGE".to_owned()
    } else {
        match last_sentence(reply) {
            Some(tail) => UiMsg::ImageMissingInReplyTail { tail }.into(),
            None => UiMsg::ImageMissingInReply.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageRef {
    DataUrl(String),
    Path(PathBuf),
}

/// 一行裡從路徑起點（POSIX 的「/」或 Windows 的「C:\」）切到最後一個圖片副檔名結尾。
/// 兩邊的工作資料夾都可能帶空格（macOS 的「Application Support」、Windows 帶空格的使用者名），
/// 逐詞切會把路徑攔腰切斷，改用這個切法連空格與尾隨標點一起處理。
fn path_span(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    // 磁碟機字母前不接英數，才不會把「https://…」的「s:/」當成路徑開頭
    let drive = (0..bytes.len()).find(|&index| {
        bytes[index].is_ascii_alphabetic()
            && (index == 0 || !bytes[index - 1].is_ascii_alphanumeric())
            && bytes.get(index + 1) == Some(&b':')
            && matches!(bytes.get(index + 2), Some(b'\\' | b'/'))
    });
    let start = [drive, line.find('/')].into_iter().flatten().min()?;
    let lowered = line.to_ascii_lowercase();
    let end = [".png", ".jpg", ".jpeg", ".webp"]
        .into_iter()
        .filter_map(|extension| lowered.rfind(extension).map(|at| at + extension.len()))
        .filter(|end| *end > start)
        .max()?;
    Some(&line[start..end])
}

/// 回覆裡的圖片候選，依出現順序去重；呼叫端挑第一個真的讀得到的。
/// 先整行切（吃得下含空格的路徑），再退回逐詞切；
/// 前導說明可能緊貼著路徑（「…浮水印。/Users/…png」），所以每個詞另外補一個「從斜線起算」的切法。
pub fn extract_image_refs(text: &str) -> Vec<ImageRef> {
    if let Some(start) = text.find("data:image/") {
        let data_url = text[start..]
            .split(|character: char| {
                character.is_whitespace() || matches!(character, '\'' | '"' | '`')
            })
            .next()
            .unwrap_or("");
        if !data_url.is_empty() {
            return vec![ImageRef::DataUrl(data_url.to_owned())];
        }
    }
    let mut refs = Vec::new();
    for candidate in text
        .lines()
        .filter_map(path_span)
        .chain(text.split_whitespace())
        .flat_map(|token| {
            let token = token.trim_matches(|character: char| {
                matches!(
                    character,
                    '\'' | '"' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | ';'
                )
            });
            let from_slash = token
                .find('/')
                .filter(|start| *start > 0)
                .map(|start| &token[start..]);
            [Some(token), from_slash].into_iter().flatten()
        })
        .filter(|candidate| is_image_extension(Path::new(candidate)))
    {
        let found = ImageRef::Path(PathBuf::from(candidate));
        if !refs.contains(&found) {
            refs.push(found);
        }
    }
    refs
}

/// 回覆最後一句（截 200 字）
fn last_sentence(reply: &str) -> Option<String> {
    let line = reply
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())?;
    Some(line.chars().take(200).collect())
}

fn is_image_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .is_some_and(|extension| matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "webp"))
}

#[cfg(test)]
mod tests;
