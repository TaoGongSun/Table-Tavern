use crate::generated_image::cli_fetch::run_cli_image;
use crate::generated_image::{self, GeneratedImage};
use crate::ui_msg::UiMsg;
use crate::{config_root, data, data_root, import, transport};
use std::path::PathBuf;

#[tauri::command]
pub(crate) fn read_character_image(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<Option<String>, String> {
    import::character_image(&data_root(&app)?, &world_id, &character_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn save_character_image(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    let _permit = data::world_write_permit(&world_id)?;
    import::save_character_image(&data_root(&app)?, &world_id, &character_id, &data)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn delete_character_image(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<(), String> {
    let _permit = data::world_write_permit(&world_id)?;
    import::delete_character_image(&data_root(&app)?, &world_id, &character_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn read_character_avatar(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<Option<String>, String> {
    import::character_avatar(&data_root(&app)?, &world_id, &character_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn save_character_avatar(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    let _permit = data::world_write_permit(&world_id)?;
    import::save_character_avatar(&data_root(&app)?, &world_id, &character_id, &data)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn delete_character_avatar(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<(), String> {
    let _permit = data::world_write_permit(&world_id)?;
    import::delete_character_avatar(&data_root(&app)?, &world_id, &character_id)
        .map_err(|error| error.to_string())
}

/// GM 卡的圖：世界書匯入 PNG 卡時存下的那張，沒有回 None（前端回退內建書本圖）
#[tauri::command]
pub(crate) fn read_gm_image(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<Option<String>, String> {
    import::gm_image(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        output.push(TABLE[((value >> 18) & 0x3f) as usize] as char);
        output.push(TABLE[((value >> 12) & 0x3f) as usize] as char);
        output.push(if chunk.len() > 1 {
            TABLE[((value >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(value & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    output
}

fn validate_gallery_component(value: &str, require_png: bool) -> Result<(), String> {
    if value.is_empty()
        || value.contains("..")
        || value.contains('/')
        || value.contains('\\')
        || (require_png && !value.ends_with(".png"))
    {
        return Err(UiMsg::InvalidFileName.into());
    }
    Ok(())
}

fn gallery_directory(
    root: &std::path::Path,
    world_id: &str,
    character_id: &str,
) -> Result<PathBuf, String> {
    data::gallery_dir(root, world_id, character_id).map_err(|error| error.to_string())
}

fn list_gallery_image_files(
    root: &std::path::Path,
    world_id: &str,
    character_id: &str,
) -> Result<Vec<String>, String> {
    let directory = gallery_directory(root, world_id, character_id)?;
    let mut files = match std::fs::read_dir(directory) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|file| file.ends_with(".png"))
            .collect::<Vec<_>>(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.to_string()),
    };
    files.sort_unstable_by(|left, right| right.cmp(left));
    Ok(files)
}

fn save_generated_gallery_image(
    root: &std::path::Path,
    world_id: &str,
    character_id: &str,
    png: &[u8],
) -> Result<(), String> {
    let directory = gallery_directory(root, world_id, character_id)?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    data::commit_world_write(&directory.join(format!("{timestamp}.png")), png)
        .map_err(|error| error.to_string())
}

/// 生圖輸出統一轉 PNG（見 generated_image）再進圖庫；任一步失敗就不寫圖庫。
async fn store_generated_image(
    root: &std::path::Path,
    world_id: &str,
    character_id: &str,
    image: GeneratedImage,
) -> Result<String, String> {
    let png = generated_image::normalize(image).await?;
    save_generated_gallery_image(root, world_id, character_id, &png)?;
    Ok(generated_image::png_data_url(&png))
}

fn image_file_data_url(path: &std::path::Path) -> Result<String, String> {
    let mime = match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => return Err(UiMsg::UnsupportedImageFormat.into()),
    };
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    Ok(format!("data:{mime};base64,{}", encode_base64(&bytes)))
}

/// 角色名與描述由編輯器直接傳進來（不讀也不寫卡片檔）：新卡還沒存檔就能生圖，
/// 且吃到的是編輯器裡的當下內容；追加描寫由前端存進草稿，跟其他欄位一起按儲存才落地。
/// character_id 前端已先跟 new_id 要好，決定圖庫路徑；name 只進提示詞。
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn generate_character_image(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    name: String,
    description: String,
    extra_prompt: String,
    source: Option<String>,
    framing: Option<String>,
) -> Result<String, String> {
    let _permit = data::world_write_permit_async(&world_id).await?;
    let root = data_root(&app)?;
    data::vn::refuse_if_replay(&root, &world_id)?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    // 構圖二選一：half＝半身特寫，其餘一律全身（含舊前端沒傳的情況）
    let shot = match framing.as_deref() {
        Some("half") => "waist-up half-body",
        _ => "full-body",
    };
    let mut prompt = format!(
        "Generate a single {shot} character illustration, portrait orientation 2:3. No text, no watermark, plain background.\nCharacter name: {name}\nCharacter description:\n{description}"
    );
    if !extra_prompt.trim().is_empty() {
        prompt.push_str(&format!(
            "\nAdditional art direction (takes priority over the defaults above): {extra_prompt}"
        ));
    }
    // 生圖來源可與聊天連線分開選（source 覆寫；空值＝跟隨 preferences.transport）
    let transport_kind = source
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            config
                .preferences
                .get("transport")
                .and_then(|value| value.as_str())
                .unwrap_or("api")
                .to_owned()
        });
    if transport_kind == "api" {
        let image = transport::generate_image(&config, &prompt).await?;
        return store_generated_image(
            &root,
            &world_id,
            &character_id,
            GeneratedImage::Encoded(image),
        )
        .await;
    }
    // CLI 一律照送：能生圖的家（codex $imagegen／agy／grok）存檔，取圖層從各家落點拿；其餘拿不到圖就失敗
    let image = run_cli_image(
        &app,
        &config,
        &transport_kind,
        Some(&world_id),
        &prompt,
        None,
    )
    .await?;
    store_generated_image(&root, &world_id, &character_id, image).await
}

#[tauri::command]
pub(crate) fn list_gallery_images(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<Vec<String>, String> {
    list_gallery_image_files(&data_root(&app)?, &world_id, &character_id)
}

#[tauri::command]
pub(crate) fn read_gallery_image(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    file: String,
) -> Result<String, String> {
    validate_gallery_component(&file, true)?;
    let directory = gallery_directory(&data_root(&app)?, &world_id, &character_id)?;
    image_file_data_url(&directory.join(file))
}

#[tauri::command]
pub(crate) fn delete_gallery_image(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    file: String,
) -> Result<(), String> {
    let _permit = data::world_write_permit(&world_id)?;
    validate_gallery_component(&file, true)?;
    let directory = gallery_directory(&data_root(&app)?, &world_id, &character_id)?;
    data::commit_world_remove(&directory.join(file)).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        encode_base64, image_file_data_url, list_gallery_image_files, store_generated_image,
        validate_gallery_component,
    };
    use crate::commands::NEXT_TEMP_ID;
    use crate::data;
    use crate::generated_image::GeneratedImage;
    use std::path::PathBuf;
    use std::sync::atomic::Ordering;

    #[test]
    fn encode_base64_matches_standard_alphabet() {
        assert_eq!(encode_base64(b"png"), "cG5n");
    }

    #[test]
    fn gallery_component_validation_allows_plain_png_name() {
        assert!(validate_gallery_component("1720000000000.png", true).is_ok());
    }

    #[test]
    fn gallery_component_validation_rejects_parent_path() {
        assert!(validate_gallery_component("../secret.png", true).is_err());
    }

    #[test]
    fn gallery_component_validation_rejects_path_separator() {
        assert!(validate_gallery_component("folder/image.png", true).is_err());
    }

    #[test]
    fn list_gallery_image_files_sorts_newest_first() {
        let root = std::env::temp_dir().join(format!(
            "table-tavern-gallery-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let world_id = data::new_id();
        let character_id = data::new_id();
        let directory = root
            .join("worlds")
            .join(&world_id)
            .join("gen-gallery")
            .join(&character_id);
        std::fs::create_dir_all(&directory).unwrap();
        for file in [
            "1720000000000.png",
            "1730000000000.png",
            "1710000000000.png",
        ] {
            std::fs::write(directory.join(file), b"png").unwrap();
        }
        assert_eq!(
            list_gallery_image_files(&root, &world_id, &character_id).unwrap(),
            [
                "1730000000000.png",
                "1720000000000.png",
                "1710000000000.png"
            ]
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "table-tavern-{label}-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    /// 遠端網址的圖：下載、轉 PNG、進圖庫，之後從圖庫重讀拿到的是 PNG
    #[tokio::test]
    async fn remote_image_lands_in_gallery_as_png_and_reads_back() {
        use std::io::{Read, Write};
        const JPEG: &[u8] = include_bytes!("../generated_image/fixtures/opaque.jpg");
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0u8; 4096];
            let _ = socket.read(&mut request);
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                JPEG.len()
            );
            socket.write_all(head.as_bytes()).unwrap();
            socket.write_all(JPEG).unwrap();
        });
        let root = temp_root("gallery-remote");
        let world_id = data::create_world(&root, "生圖").unwrap();
        let character_id = data::new_id();
        let returned = store_generated_image(
            &root,
            &world_id,
            &character_id,
            GeneratedImage::Encoded(format!("http://{address}/fox.jpg")),
        )
        .await
        .unwrap();
        assert!(returned.starts_with("data:image/png;base64,"));
        let files = list_gallery_image_files(&root, &world_id, &character_id).unwrap();
        assert_eq!(files.len(), 1);
        let directory = root
            .join("worlds")
            .join(&world_id)
            .join("gen-gallery")
            .join(&character_id);
        let stored = std::fs::read(directory.join(&files[0])).unwrap();
        assert!(stored.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(
            image_file_data_url(&directory.join(&files[0])).unwrap(),
            returned
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    /// 格式不支援就整個失敗，圖庫不留檔
    #[tokio::test]
    async fn unsupported_image_is_not_written_to_gallery() {
        const SVG: &[u8] = include_bytes!("../generated_image/fixtures/tiny.svg");
        let root = temp_root("gallery-reject");
        let (world_id, character_id) = (data::new_id(), data::new_id());
        let error = store_generated_image(
            &root,
            &world_id,
            &character_id,
            GeneratedImage::Bytes(SVG.to_vec()),
        )
        .await
        .unwrap_err();
        assert!(
            error.starts_with("AI_IMAGE_UNSUPPORTED_FORMAT: "),
            "{error}"
        );
        assert!(list_gallery_image_files(&root, &world_id, &character_id)
            .unwrap()
            .is_empty());
        let _ = std::fs::remove_dir_all(root);
    }
}
