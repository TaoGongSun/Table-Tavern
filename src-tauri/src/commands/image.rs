use crate::ai_transport::{cli_workspace, stream_via_transport};
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
    import::save_character_image(&data_root(&app)?, &world_id, &character_id, &data)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn delete_character_image(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<(), String> {
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
    import::save_character_avatar(&data_root(&app)?, &world_id, &character_id, &data)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn delete_character_avatar(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<(), String> {
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
        .filter(|candidate| is_image_extension(std::path::Path::new(candidate)))
    {
        let found = ImageRef::Path(PathBuf::from(candidate));
        if !refs.contains(&found) {
            refs.push(found);
        }
    }
    refs
}

/// 沒抓到圖時附上 CLI 的最後一句（截 200 字）：模型不照暗號時，拒絕理由通常寫在那裡
fn last_sentence(reply: &str) -> Option<String> {
    let line = reply
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .next_back()?;
    Some(line.chars().take(200).collect())
}

fn is_image_extension(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .is_some_and(|extension| matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "webp"))
}

/// CLI 沙盒只能寫工作目錄，生圖時會把圖搬進來，只為了回一個 app 讀得到的路徑。
/// 圖讀進圖庫後這份就沒用了：三家 CLI 一律在生圖收尾清掉（含失敗那次留下的），免得越堆越多。
/// CLI 會自己開子目錄（codex 的 output/imagegen/），所以往下遞迴；清空的目錄順手移除。
fn clear_cli_workspace_images(workspace: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(workspace) else {
        return;
    };
    for path in entries.flatten().map(|entry| entry.path()) {
        if path.is_dir() {
            clear_cli_workspace_images(&path);
            let _ = std::fs::remove_dir(&path); // 只有真的空了才成功，留有其他檔案的目錄不動
        } else if is_image_extension(&path) {
            let _ = std::fs::remove_file(path);
        }
    }
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

fn decode_base64(value: &str) -> Result<Vec<u8>, String> {
    fn sextet(byte: u8) -> Option<u8> {
        match byte {
            b'A'..=b'Z' => Some(byte - b'A'),
            b'a'..=b'z' => Some(byte - b'a' + 26),
            b'0'..=b'9' => Some(byte - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }

    let bytes = value.as_bytes();
    if bytes.len() % 4 != 0 {
        return Err("非法 base64 資料".to_owned());
    }
    let mut output = Vec::with_capacity(bytes.len() / 4 * 3);
    for (index, chunk) in bytes.chunks_exact(4).enumerate() {
        let padding = chunk.iter().rev().take_while(|&&byte| byte == b'=').count();
        if padding > 2 || (padding > 0 && index + 1 != bytes.len() / 4) {
            return Err("非法 base64 資料".to_owned());
        }
        let a = sextet(chunk[0]).ok_or_else(|| "非法 base64 資料".to_owned())?;
        let b = sextet(chunk[1]).ok_or_else(|| "非法 base64 資料".to_owned())?;
        let c = if padding >= 2 {
            0
        } else {
            sextet(chunk[2]).ok_or_else(|| "非法 base64 資料".to_owned())?
        };
        let d = if padding >= 1 {
            0
        } else {
            sextet(chunk[3]).ok_or_else(|| "非法 base64 資料".to_owned())?
        };
        if (padding >= 1 && chunk[3] != b'=') || (padding >= 2 && chunk[2] != b'=') {
            return Err("非法 base64 資料".to_owned());
        }
        let decoded =
            (u32::from(a) << 18) | (u32::from(b) << 12) | (u32::from(c) << 6) | u32::from(d);
        output.push((decoded >> 16) as u8);
        if padding < 2 {
            output.push((decoded >> 8) as u8);
        }
        if padding == 0 {
            output.push(decoded as u8);
        }
    }
    Ok(output)
}

fn validate_gallery_component(value: &str, require_png: bool) -> Result<(), String> {
    if value.is_empty()
        || value.contains("..")
        || value.contains('/')
        || value.contains('\\')
        || (require_png && !value.ends_with(".png"))
    {
        return Err("非法檔名".to_owned());
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
    data_url: &str,
) -> Result<(), String> {
    let Some((header, encoded)) = data_url.split_once(',') else {
        return Ok(());
    };
    if !header.starts_with("data:") || !header.ends_with(";base64") {
        return Ok(());
    }
    let directory = gallery_directory(root, world_id, character_id)?;
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    std::fs::write(
        directory.join(format!("{timestamp}.png")),
        decode_base64(encoded)?,
    )
    .map_err(|error| error.to_string())
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
        _ => return Err("不支援的圖片格式".to_owned()),
    };
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    Ok(format!("data:{mime};base64,{}", encode_base64(&bytes)))
}

/// 角色名與描述由編輯器直接傳進來（不讀也不寫卡片檔）：新卡還沒存檔就能生圖，
/// 且吃到的是編輯器裡的當下內容；追加描寫由前端存進草稿，跟其他欄位一起按儲存才落地。
/// character_id 前端已先跟 new_id 要好，決定圖庫路徑；name 只進提示詞。
#[tauri::command]
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
    let root = data_root(&app)?;
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
        save_generated_gallery_image(&root, &world_id, &character_id, &image)?;
        return Ok(image);
    }
    // CLI 一律照送：能生圖的家（codex $imagegen／agy／grok）會存檔回路徑，其餘掃不到圖就失敗
    // 兩個暗號分開問：生不出（沒能力／沒額度）與不肯生（內容規範）要給玩家不同的下一步
    prompt.push_str(
        "\nIf you are able to generate images, generate it now, save it as a PNG file, and reply with the absolute file path of the saved image. If you cannot generate images at all, reply exactly: NO_IMAGE. If you decline this particular request, reply exactly: REFUSED",
    );
    if transport_kind == "codex" {
        prompt = format!("$imagegen {prompt}");
    }
    let messages = [transport::ChatMessage {
        role: "user".to_owned(),
        content: prompt,
    }];
    let reply = stream_via_transport(
        &app,
        &config,
        Some(&transport_kind),
        true,
        transport::gm_tier(&config),
        Some(&world_id),
        "",
        "",
        &messages,
        false,
        |_| {},
    )
    .await?;
    let workspace = cli_workspace(&app)?;
    let found = extract_image_refs(&reply)
        .into_iter()
        .find_map(|found| match found {
            ImageRef::DataUrl(data_url) => Some(Ok(data_url)),
            // CLI 常回相對於自己工作目錄的路徑（codex 的 imagegen 存進 output/imagegen/）；
            // 補上基準才讀得到，絕對路徑 join 後維持原樣
            ImageRef::Path(path) => {
                let path = workspace.join(path);
                std::fs::metadata(&path)
                    .is_ok()
                    .then(|| image_file_data_url(&path))
            }
        })
        // REFUSED／NO_IMAGE 是上面 prompt 跟 CLI 約好的暗號，前端據此各換一句人話；
        // 兩個都沒對上時附最後一句原話，模型不照暗號時的拒絕理由通常就寫在那
        .unwrap_or_else(|| {
            Err(if reply.contains("REFUSED") {
                "REFUSED：來源拒絕生成這段內容".to_owned()
            } else if reply.contains("NO_IMAGE") {
                "NO_IMAGE：來源回報無法生圖".to_owned()
            } else {
                match last_sentence(&reply) {
                    Some(tail) => format!("回覆中沒有圖片：{tail}"),
                    None => "回覆中沒有圖片".to_owned(),
                }
            })
        });
    // 圖已經讀進記憶體，中轉檔失去用途；成功與失敗都清
    clear_cli_workspace_images(&workspace);
    let image = found?;
    save_generated_gallery_image(&root, &world_id, &character_id, &image)?;
    Ok(image)
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
    validate_gallery_component(&file, true)?;
    let directory = gallery_directory(&data_root(&app)?, &world_id, &character_id)?;
    std::fs::remove_file(directory.join(file)).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        clear_cli_workspace_images, decode_base64, encode_base64, extract_image_refs,
        list_gallery_image_files, validate_gallery_component, ImageRef,
    };
    use crate::commands::NEXT_TEMP_ID;
    use crate::data;
    use std::path::PathBuf;
    use std::sync::atomic::Ordering;

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

    /// 中轉檔清理連 CLI 自開的子目錄一起掃，非圖片與還有東西的目錄留著
    #[test]
    fn clear_cli_workspace_images_removes_images_and_empty_dirs() {
        let workspace = std::env::temp_dir().join(format!(
            "table-tavern-workspace-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(workspace.join("output/imagegen")).unwrap();
        std::fs::create_dir_all(workspace.join("keep")).unwrap();
        std::fs::write(workspace.join("fox.PNG"), b"png").unwrap();
        std::fs::write(workspace.join("output/imagegen/deep.png"), b"png").unwrap();
        std::fs::write(workspace.join("note.txt"), b"keep").unwrap();
        std::fs::write(workspace.join("keep/data.txt"), b"keep").unwrap();
        clear_cli_workspace_images(&workspace);
        assert!(!workspace.join("fox.PNG").exists());
        assert!(!workspace.join("output").exists());
        assert!(workspace.join("note.txt").exists());
        assert!(workspace.join("keep/data.txt").exists());
        std::fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn extract_image_refs_returns_empty_without_image() {
        assert_eq!(extract_image_refs("沒有附圖。"), Vec::new());
        assert_eq!(encode_base64(b"png"), "cG5n");
    }

    #[test]
    fn decode_base64_roundtrip_restores_bytes() {
        let bytes = [0, 1, 2, 127, 128, 255];
        assert_eq!(decode_base64(&encode_base64(&bytes)).unwrap(), bytes);
    }

    #[test]
    fn decode_base64_rejects_invalid_input() {
        assert!(decode_base64("not base64!").is_err());
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
}
