use super::{
    downscale_png, extract_image_refs, grok_reference, missing_reason, take_image, CliJob, Homes,
    ImageRef,
};
use crate::commands::NEXT_TEMP_ID;
use crate::generated_image::GeneratedImage;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

fn temp_root(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "table-tavern-{label}-{}-{}",
        std::process::id(),
        NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Fixture {
    base: PathBuf,
    homes: Homes,
    workspace: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let base = temp_root(label);
        Fixture {
            homes: Homes {
                codex: base.join("codex-home"),
                agy: base.join("agy-home"),
                grok: base.join("grok-home"),
            },
            workspace: base.join("cli-workspace"),
            base,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

fn write(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn bytes(image: Result<GeneratedImage, String>) -> Vec<u8> {
    match image.unwrap() {
        GeneratedImage::Bytes(bytes) => bytes,
        GeneratedImage::Encoded(text) => panic!("expected bytes, got {text}"),
    }
}

const THREAD: &str = "01a195a2-d66b-72b2-9e5e-830aed324d4f";
const CONVERSATION: &str = "0895dff7-b1f3-44c5-8c7a-e9966f888c21";

/// codex：讀 thread 目錄裡最新那張（回覆說 NO_IMAGE 也不採信），讀完刪目錄、工作目錄跟著工作消失
#[test]
fn codex_reads_newest_image_from_thread_dir_and_cleans_up() {
    let fx = Fixture::new("fetch-codex");
    let dir = fx.homes.codex.join("generated_images").join(THREAD);
    write(&dir.join("a.png"), b"old");
    std::thread::sleep(std::time::Duration::from_millis(20));
    write(&dir.join("b.png"), b"new");
    let job = CliJob::create(&fx.workspace, "codex", Some(b"ref")).unwrap();
    assert!(job.reference.as_ref().unwrap().starts_with(&job.dir));
    *job.codex_thread.lock().unwrap() = Some(THREAD.to_owned());
    assert_eq!(
        bytes(take_image("codex", &job, &fx.homes, "NO_IMAGE")),
        b"new"
    );
    assert!(!dir.exists());
    let job_dir = job.dir.clone();
    drop(job);
    assert!(!job_dir.exists());
}

/// thread 目錄裡指向外部的連結不收（不跟隨連結），回覆提到它也不收
#[cfg(unix)]
#[test]
fn symlink_in_thread_dir_is_rejected() {
    let fx = Fixture::new("fetch-symlink");
    let outside = fx.base.join("secret.png");
    write(&outside, b"secret");
    let dir = fx.homes.codex.join("generated_images").join(THREAD);
    std::fs::create_dir_all(&dir).unwrap();
    std::os::unix::fs::symlink(&outside, dir.join("link.png")).unwrap();
    let job = CliJob::create(&fx.workspace, "codex", None).unwrap();
    *job.codex_thread.lock().unwrap() = Some(THREAD.to_owned());
    let reply = format!("{}\nNO_IMAGE", dir.join("link.png").display());
    assert_eq!(
        take_image("codex", &job, &fx.homes, &reply)
            .err()
            .as_deref(),
        Some("NO_IMAGE")
    );
    assert!(outside.exists());
}

/// 退回抓路徑：本次工作目錄內的收；並行中別的工作目錄的圖不收，改回暗號
#[test]
fn reply_path_is_accepted_only_inside_this_job() {
    let fx = Fixture::new("fetch-reply");
    let job = CliJob::create(&fx.workspace, "codex", None).unwrap();
    let other = CliJob::create(&fx.workspace, "codex", None).unwrap();
    write(&job.dir.join("output/imagegen/mine.png"), b"mine");
    write(&other.dir.join("theirs.png"), b"theirs");
    assert_eq!(
        bytes(take_image(
            "codex",
            &job,
            &fx.homes,
            "Saved to output/imagegen/mine.png"
        )),
        b"mine"
    );
    let reply = format!(
        "Saved to {}\nNO_IMAGE",
        other.dir.join("theirs.png").display()
    );
    assert_eq!(
        take_image("codex", &job, &fx.homes, &reply)
            .err()
            .as_deref(),
        Some("NO_IMAGE")
    );
}

/// thread id 不是 UUID 形狀就不拿來組路徑
#[test]
fn malformed_thread_id_is_ignored() {
    let fx = Fixture::new("fetch-bad-id");
    write(&fx.homes.codex.join("generated_images/x.png"), b"x");
    let job = CliJob::create(&fx.workspace, "codex", None).unwrap();
    *job.codex_thread.lock().unwrap() = Some("../generated_images".to_owned());
    assert_eq!(
        take_image("codex", &job, &fx.homes, "REFUSED")
            .err()
            .as_deref(),
        Some("AI_IMAGE_REFUSED")
    );
}

/// agy：讀該次對話 brain 目錄裡的圖，讀完整個對話刪掉
#[test]
fn agy_reads_brain_dir_and_removes_conversation() {
    let fx = Fixture::new("fetch-agy");
    let base = fx.homes.agy.join(".gemini/antigravity-cli");
    write(
        &base.join("brain").join(CONVERSATION).join("hero_1.jpg"),
        b"jpg",
    );
    write(
        &base
            .join("conversations")
            .join(format!("{CONVERSATION}.db")),
        b"db",
    );
    let job = CliJob::create(&fx.workspace, "agy", None).unwrap();
    *job.agy_conversation.lock().unwrap() = Some(CONVERSATION.to_owned());
    assert_eq!(bytes(take_image("agy", &job, &fx.homes, "")), b"jpg");
    assert!(!base.join("brain").join(CONVERSATION).exists());
    assert!(!base
        .join(format!("conversations/{CONVERSATION}.db"))
        .exists());
}

/// grok：模型回報 session 裡的原圖路徑也收；拿到圖後整份 session 刪掉，群組層留著
#[test]
fn grok_session_image_is_read_then_session_removed() {
    let fx = Fixture::new("fetch-grok");
    let job = CliJob::create(&fx.workspace, "grok", None).unwrap();
    let session_id = job.grok_session.clone().unwrap();
    let group = fx
        .homes
        .grok
        .join("sessions")
        .join("%2Ftmp%2Fcli-workspace");
    let session = group.join(&session_id);
    write(&session.join("images/1.jpg"), b"grok");
    write(&group.join("prompt_history.jsonl"), b"{}\n");
    let reply = format!("Saved: {}", session.join("images/1.jpg").display());
    assert_eq!(bytes(take_image("grok", &job, &fx.homes, &reply)), b"grok");
    assert!(!session.exists());
    assert!(group.join("prompt_history.jsonl").exists());
}

/// grok 沒拿到圖：session 不刪，留給定期清理
#[test]
fn grok_session_is_kept_when_no_image() {
    let fx = Fixture::new("fetch-grok-fail");
    let job = CliJob::create(&fx.workspace, "grok", None).unwrap();
    let session = fx
        .homes
        .grok
        .join("sessions/group")
        .join(job.grok_session.as_deref().unwrap());
    std::fs::create_dir_all(session.join("images")).unwrap();
    assert_eq!(
        take_image("grok", &job, &fx.homes, "REFUSED")
            .err()
            .as_deref(),
        Some("AI_IMAGE_REFUSED")
    );
    assert!(session.exists());
}

#[test]
fn extract_image_refs_returns_data_url() {
    assert_eq!(
        extract_image_refs("圖片：`data:image/png;base64,cG5n`"),
        vec![ImageRef::DataUrl("data:image/png;base64,cG5n".to_owned())]
    );
}

#[test]
fn extract_image_refs_returns_existing_temp_file_path() {
    let path = std::env::temp_dir().join(format!(
        "table-tavern-image-{}-{}.png",
        std::process::id(),
        NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, b"png").unwrap();
    assert_eq!(
        extract_image_refs(&format!("已生成 {}", path.display())),
        vec![ImageRef::Path(path.clone())]
    );
    std::fs::remove_file(path).unwrap();
}

/// 真實 codex 輸出：前導說明與路徑之間沒有空白，整段當路徑會讀不到檔
#[test]
fn extract_image_refs_recovers_path_glued_to_preceding_sentence() {
    assert!(
        extract_image_refs("不含浮水印。/Users/me/.codex/generated_images/abc/call_x.png")
            .contains(&ImageRef::Path(PathBuf::from(
                "/Users/me/.codex/generated_images/abc/call_x.png"
            )))
    );
}

/// macOS 的 CLI 工作資料夾在「Application Support」底下，逐詞切會把路徑攔腰切斷
#[test]
fn extract_image_refs_keeps_path_with_spaces() {
    assert!(extract_image_refs(
        "已存到 /Users/me/Library/Application Support/TableTavern/cli-workspace/fox.png，請查收"
    )
    .contains(&ImageRef::Path(PathBuf::from(
        "/Users/me/Library/Application Support/TableTavern/cli-workspace/fox.png"
    ))));
}

/// codex 的 imagegen 把圖存進工作目錄的子目錄，回覆給的是相對路徑
#[test]
fn extract_image_refs_keeps_relative_path() {
    assert!(extract_image_refs("Saved to output/imagegen/fox.png")
        .contains(&ImageRef::Path(PathBuf::from("output/imagegen/fox.png"))));
}

/// Windows 路徑沒有斜線可切，使用者名稱帶空格時同樣會斷
#[test]
fn extract_image_refs_keeps_windows_path_with_spaces() {
    assert!(extract_image_refs(
        "Saved to C:\\Users\\John Smith\\AppData\\Roaming\\TableTavern\\cli-workspace\\fox.PNG"
    )
    .contains(&ImageRef::Path(PathBuf::from(
        "C:\\Users\\John Smith\\AppData\\Roaming\\TableTavern\\cli-workspace\\fox.PNG"
    ))));
}

#[test]
fn extract_image_refs_returns_empty_without_image() {
    assert_eq!(extract_image_refs("沒有附圖。"), Vec::new());
}

/// grok 參考圖：透明處壓成淺灰底的 JPEG；看圖前長邊縮到 768，本來就小的原樣回
#[test]
fn grok_reference_is_gray_jpeg_and_vision_image_is_downscaled() {
    let png = |width, height| {
        let mut bytes = Vec::new();
        image::RgbaImage::new(width, height)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        bytes
    };
    let jpeg = grok_reference(&png(40, 60)).unwrap();
    assert_eq!(
        image::guess_format(&jpeg).unwrap(),
        image::ImageFormat::Jpeg
    );
    let pixel = image::load_from_memory(&jpeg).unwrap().to_rgb8()[(20, 30)];
    assert!(pixel.0.iter().all(|v| v.abs_diff(217) <= 2), "{pixel:?}");
    let small = downscale_png(&png(1000, 1500), 768).unwrap();
    let small = image::load_from_memory(&small).unwrap();
    assert_eq!((small.width(), small.height()), (512, 768));
    let tiny = png(300, 300);
    assert_eq!(downscale_png(&tiny, 768).unwrap(), tiny);
}

/// 拒絕三種回法都歸成 AI_IMAGE_REFUSED：暗號、沒照暗號的拒絕說明（agy 實測，截斷前整段比對）；
/// NO_IMAGE 與一般沒圖不算
#[test]
fn refusals_map_to_one_code() {
    assert_eq!(missing_reason("REFUSED"), "AI_IMAGE_REFUSED");
    let prose = format!(
        "I cannot fulfill this request. {} My safety constraints prevent it.",
        "x".repeat(300)
    );
    assert!(missing_reason(&prose).starts_with("AI_IMAGE_REFUSED: I cannot fulfill"));
    assert_eq!(missing_reason("NO_IMAGE"), "NO_IMAGE");
    assert_eq!(missing_reason("Sorry.\nREFUSED."), "AI_IMAGE_REFUSED");
    assert!(
        !missing_reason("The tool failed: ECONNREFUSED 127.0.0.1").starts_with("AI_IMAGE_REFUSED")
    );
    assert!(!missing_reason("Done, but the file was lost.").starts_with("AI_IMAGE_REFUSED"));
    assert!(crate::generated_image::is_refused(
        "AI_IMAGE_REFUSED: AI_CONTENT_FILTERED: agy"
    ));
    assert!(!crate::generated_image::is_refused(
        "AI_CALL_FAILED: ECONNREFUSED"
    ));
}
