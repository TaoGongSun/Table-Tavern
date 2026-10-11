use super::card_io::{decode_png_character, find_png_card, string_field, PNG_MAGIC};
use crate::data::{self, DataResult};
use serde_json::Value;
use std::fs;
use std::path::Path;

/// 卡片自帶介面：SillyTavern 的 `extensions.regex_scripts` 裡「模型輸出後套用」的顯示腳本，
/// 交給前端在沙盒 iframe 渲染；DRM 加密卡與雲端載入器卡不解密、不繞過，只回報不支援。
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct InterfaceScript {
    pub name: String,
    pub find_regex: String,
    pub replace_string: String,
    pub trim_strings: Vec<String>,
    pub min_depth: Option<i64>,
    pub max_depth: Option<i64>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct CardInterface {
    pub character_id: String,
    pub character_name: String,
    pub scripts: Vec<InterfaceScript>,
    pub unsupported: Option<String>,
    /// 卡片自帶的開場白原文。空桌（一則訊息都還沒有）時，面板拿它當來源，
    /// 這樣「開場就是一整頁角色選擇畫面」的卡片一匯入就看得到入口。
    pub opening: Option<String>,
    /// 卡的酒館助手腳本有載入 MVU（MagVarUpdate）：沙盒要墊 MVU 讀變數函式、GM 樓比照 MVU 補狀態欄占位。
    #[serde(default)]
    pub mvu: bool,
}

/// 掃描每張已匯入卡的原始卡檔（PNG／.import.json），抽出可渲染的顯示腳本。
/// 面板是選配功能：單張卡出錯絕不讓整份清單報錯。非匯入卡（沒有原卡檔）略過；原卡檔在但讀不出
/// （IO 錯誤、PNG 結構壞、卡片資料解不開）回一筆 `unsupported = "unreadable"`，重構拿它擋下，
/// 不讓讀不出的卡被當成「沒有介面」。
pub fn read_card_interfaces(root: &Path, world_id: &str) -> DataResult<Vec<CardInterface>> {
    let mut result = read_card_interfaces_raw(root, world_id)?;
    // D24：這桌不套卡內 regex 腳本（網頁存檔匯入時玩家沒允許）→ 顯示腳本整組清空，其餘照舊
    if !data::read_state(root, world_id).map_or(true, |state| state.regex_allowed) {
        for card in &mut result {
            card.scripts.clear();
        }
    }
    Ok(result)
}

const UNREADABLE: &str = "unreadable";

/// 讀一份原卡檔，回卡片資料（`data` 物件，沒有就整份）。PNG 結構正常但沒有卡片資料（一般圖片）
/// 回 `Ok(None)`；IO 錯誤、PNG 結構壞、base64／JSON 解不開回 `Err`。
fn read_raw_card(path: &Path) -> Result<Option<Value>, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let json_bytes = if bytes.starts_with(PNG_MAGIC) {
        match find_png_card(&bytes).map_err(|error| error.to_string())? {
            Some(json_bytes) => json_bytes,
            None => return Ok(None),
        }
    } else {
        bytes
    };
    let mut value =
        serde_json::from_slice::<Value>(&json_bytes).map_err(|error| error.to_string())?;
    Ok(Some(
        match value.get_mut("data").filter(|data| data.is_object()) {
            Some(data) => data.take(),
            None => value,
        },
    ))
}

fn unreadable(character_id: &str, character_name: &str, path: &Path, error: &str) -> CardInterface {
    eprintln!("原卡檔讀不出 {}: {error}", path.display());
    CardInterface {
        character_id: character_id.to_owned(),
        character_name: character_name.to_owned(),
        scripts: Vec::new(),
        unsupported: Some(UNREADABLE.to_owned()),
        opening: None,
        mvu: false,
    }
}

fn read_card_interfaces_raw(root: &Path, world_id: &str) -> DataResult<Vec<CardInterface>> {
    let mut result = Vec::new();
    // 角色層：`<id>.png` 不一定是原卡（換角色圖、重構套用的立繪都寫這檔），所以 PNG 沒有卡片資料
    // 就改看 `.import.json`；兩份都沒有卡片資料＝非匯入卡，略過。任一份讀得出就用它，都讀不出
    // 而且至少一份是損毀才算 unreadable。
    for meta in data::list_characters(root, world_id)? {
        if meta.archived || meta.auto_hidden {
            continue;
        }
        let md_path = data::character_path(root, world_id, &meta.id)?;
        let mut card_data = None;
        let mut broken = None;
        for path in [
            md_path.with_extension("png"),
            md_path.with_extension("import.json"),
        ] {
            if !path.exists() {
                continue;
            }
            match read_raw_card(&path) {
                Ok(Some(value)) => {
                    card_data = Some(value);
                    break;
                }
                Ok(None) => {}
                Err(error) => broken = broken.or(Some((path, error))),
            }
        }
        match (card_data, broken) {
            (Some(card_data), _) => result.push(card_interface(&meta.id, &meta.name, &card_data)),
            (None, Some((path, error))) => {
                result.push(unreadable(&meta.id, &meta.name, &path, &error))
            }
            (None, None) => {}
        }
    }
    // 世界層：兩種副檔名各自是一份原卡（只有帶卡片資料的卡才會存下來），各自成一筆；
    // 沒有卡片資料的 PNG 照舊略過，讀不出才是 unreadable。
    for extension in ["png", "import.json"] {
        let Ok(path) = data::world_card_path(root, world_id, extension) else {
            continue;
        };
        if !path.exists() {
            continue;
        }
        match read_raw_card(&path) {
            Ok(Some(card_data)) => {
                let name = string_field(&card_data, "name").unwrap_or("");
                result.push(card_interface("", name, &card_data));
            }
            Ok(None) => {}
            Err(error) => result.push(unreadable("", "", &path, &error)),
        }
    }
    Ok(result)
}

/// 世界書路徑也把原始卡檔留下來——那張卡的介面殼還在裡面。
/// 只有真的帶顯示腳本的卡才留，純世界書檔不必白存一份。回傳有沒有留。
pub fn save_world_card(root: &Path, world_id: &str, bytes: &[u8]) -> bool {
    let (json_bytes, extension): (Vec<u8>, &str) = if bytes.starts_with(PNG_MAGIC) {
        match decode_png_character(bytes) {
            Ok(json_bytes) => (json_bytes, "png"),
            Err(_) => return false,
        }
    } else {
        (bytes.to_vec(), "import.json")
    };
    let Ok(value) = serde_json::from_slice::<Value>(&json_bytes) else {
        return false;
    };
    let card_data = value
        .get("data")
        .filter(|data| data.is_object())
        .unwrap_or(&value);
    let has_regex_scripts = card_data
        .get("extensions")
        .and_then(|extensions| extensions.get("regex_scripts"))
        .and_then(Value::as_array)
        .is_some_and(|scripts| !scripts.is_empty());
    if !has_regex_scripts {
        return false;
    }
    let Ok(path) = data::world_card_path(root, world_id, extension) else {
        return false;
    };
    data::commit_world_write(&path, bytes).is_ok()
}

/// 網頁存檔的世界書路：權威原卡一律保存（不看有沒有介面腳本，未知外殼欄位也要留著），落檔失敗回錯。
pub(super) fn save_world_card_strict(root: &Path, world_id: &str, bytes: &[u8]) -> DataResult<()> {
    let extension = if bytes.starts_with(PNG_MAGIC) {
        decode_png_character(bytes)?;
        "png"
    } else {
        serde_json::from_slice::<Value>(bytes)?;
        "import.json"
    };
    data::commit_world_write(&data::world_card_path(root, world_id, extension)?, bytes)
}

pub(super) fn card_interface(
    character_id: &str,
    character_name: &str,
    card_data: &Value,
) -> CardInterface {
    let opening = string_field(card_data, "first_mes")
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned);
    let is_scrypt = [
        string_field(card_data, "first_mes"),
        string_field(card_data, "description"),
    ]
    .into_iter()
    .flatten()
    .any(|text| text.to_ascii_uppercase().contains("SCRYPT"));
    if is_scrypt {
        return CardInterface {
            character_id: character_id.to_owned(),
            character_name: character_name.to_owned(),
            scripts: Vec::new(),
            unsupported: Some("scrypt".to_owned()),
            opening,
            mvu: false,
        };
    }

    let scripts: Vec<InterfaceScript> = card_data
        .get("extensions")
        .and_then(|extensions| extensions.get("regex_scripts"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|script| is_display_script(script))
        .map(interface_script)
        .collect();
    let is_remote_loader = scripts.iter().any(is_remote_loader_script);

    CardInterface {
        character_id: character_id.to_owned(),
        character_name: character_name.to_owned(),
        scripts: if is_remote_loader {
            Vec::new()
        } else {
            scripts
        },
        unsupported: is_remote_loader.then(|| "remote_loader".to_owned()),
        opening,
        mvu: loads_mvu(card_data),
    }
}

/// 酒館助手腳本（新版 `tavern_helper`、舊版 `TavernHelper_scripts`，形狀各版不同，一律遞迴找）裡
/// 有沒有啟用中、內容載入 MagVarUpdate 的腳本。
fn loads_mvu(card_data: &Value) -> bool {
    fn walk(value: &Value) -> bool {
        match value {
            Value::Object(map) => {
                let enabled = map.get("enabled").and_then(Value::as_bool).unwrap_or(true);
                let hit = map
                    .get("content")
                    .and_then(Value::as_str)
                    .is_some_and(|content| content.contains("MagVarUpdate"));
                (enabled && hit) || (enabled && map.values().any(walk))
            }
            Value::Array(items) => items.iter().any(walk),
            _ => false,
        }
    }
    let Some(extensions) = card_data.get("extensions") else {
        return false;
    };
    ["tavern_helper", "TavernHelper_scripts"]
        .iter()
        .filter_map(|key| extensions.get(*key))
        .any(walk)
}

/// 只留「輸出後套用」且啟用中的顯示腳本：關閉、僅套 prompt、或不作用在模型輸出（placement 沒有 2）都不算。
fn is_display_script(script: &Value) -> bool {
    !script
        .get("disabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && !script
            .get("promptOnly")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && script
            .get("placement")
            .and_then(Value::as_array)
            .is_some_and(|placement| placement.iter().any(|value| value.as_i64() == Some(2)))
}

fn interface_script(script: &Value) -> InterfaceScript {
    InterfaceScript {
        name: string_field(script, "scriptName").unwrap_or("").to_owned(),
        find_regex: string_field(script, "findRegex").unwrap_or("").to_owned(),
        replace_string: string_field(script, "replaceString")
            .unwrap_or("")
            .to_owned(),
        trim_strings: script
            .get("trimStrings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        min_depth: script.get("minDepth").and_then(Value::as_i64),
        max_depth: script.get("maxDepth").and_then(Value::as_i64),
    }
}

/// 雲端載入器流派：整段輸出被換成一行從外部網址載入前端 app，沙盒 iframe 天生跑不起來。
fn is_remote_loader_script(script: &InterfaceScript) -> bool {
    script.replace_string.len() < 2000
        && script.replace_string.contains(".load(")
        && is_catch_all_regex(&script.find_regex)
}

/// find_regex 去掉 `/…/flags` 外殼、去掉前後 `^`／`$` 後，是否為「吞掉整段輸出」的萬用式。
fn is_catch_all_regex(find_regex: &str) -> bool {
    let body = find_regex
        .strip_prefix('/')
        .and_then(|rest| rest.rfind('/').map(|end| &rest[..end]))
        .unwrap_or(find_regex);
    let body = body.strip_prefix('^').unwrap_or(body);
    let body = body.strip_suffix('$').unwrap_or(body);
    matches!(body, ".+" | ".*" | r"[\s\S]*" | r"[\s\S]+")
}

/// 卡片的顯示腳本期待模型吐出哪些標籤，教模型那個格式的就是世界書裡提到同樣標籤的條目。
/// 回合尾要點名它，模型才不會照我們自己的旁白規矩寫。回傳那條的標題。
/// `prompt_entries` 只能是全文實際進入本輪提示的條目（`transport::gm_prompt_full_entries`）：
/// 點名一條模型看不到的條目，模型只會回「沒附規格」或亂抄範例。
pub fn card_format_entry(
    scripts: &[InterfaceScript],
    prompt_entries: &[crate::transport::PromptEntry],
) -> Option<String> {
    let tags = format_tags(scripts);
    if tags.is_empty() {
        return None;
    }
    prompt_entries
        .iter()
        .filter_map(|entry| {
            let hits = tags
                .iter()
                .filter(|tag| entry.content.contains(tag.as_str()))
                .count();
            (hits > 0).then_some((entry, hits))
        })
        .max_by_key(|(entry, hits)| (*hits, entry.content.len()))
        .map(|(entry, _)| entry.title.to_owned())
}

/// 從顯示腳本的 find_regex 掃出字面的開頭標籤（`<Xxx>` 形式），跳過收尾 `</…>` 標籤；
/// 先去掉反斜線轉義（`\/`、`\<` 等），標籤才會露出原樣。
fn format_tags(scripts: &[InterfaceScript]) -> Vec<String> {
    let mut tags = Vec::new();
    for script in scripts {
        let chars: Vec<char> = script.find_regex.chars().filter(|c| *c != '\\').collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] != '<' {
                i += 1;
                continue;
            }
            let mut j = i + 1;
            if chars.get(j) == Some(&'/') {
                i += 1; // 收尾標籤，跳過
                continue;
            }
            let Some(&first) = chars.get(j) else { break };
            if !(first.is_ascii_alphabetic() || first == '_') {
                i += 1;
                continue;
            }
            j += 1;
            while chars
                .get(j)
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_')
            {
                j += 1;
            }
            if chars.get(j) == Some(&'>') {
                let tag: String = chars[i..=j].iter().collect();
                if !tags.contains(&tag) {
                    tags.push(tag);
                }
                i = j + 1;
            } else {
                i += 1;
            }
        }
    }
    tags
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::import_character;
    use crate::import::test_support::TestRoot;
    use serde_json::json;

    #[test]
    fn read_card_interfaces_filters_to_output_display_scripts() {
        let root = TestRoot::new("interfaces-filter");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let raw = json!({
            "data": {
                "name": "莉亞",
                "extensions": {
                    "regex_scripts": [
                        {
                            "scriptName": "顯示介面",
                            "findRegex": "/.+/s",
                            "replaceString": "<div>ok</div>",
                            "trimStrings": ["a"],
                            "minDepth": 1,
                            "maxDepth": 2,
                            "placement": [2]
                        },
                        {
                            "scriptName": "已停用",
                            "findRegex": "/x/",
                            "replaceString": "y",
                            "disabled": true,
                            "placement": [2]
                        },
                        {
                            "scriptName": "只套提示詞",
                            "findRegex": "/x/",
                            "replaceString": "y",
                            "promptOnly": true,
                            "placement": [2]
                        },
                        {
                            "scriptName": "作用在使用者輸入",
                            "findRegex": "/x/",
                            "replaceString": "y",
                            "placement": [1]
                        }
                    ]
                }
            }
        })
        .to_string();

        let meta =
            import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff", "zh-TW").unwrap();
        let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
        assert_eq!(interfaces.len(), 1);
        assert_eq!(interfaces[0].character_id, meta.id);
        assert_eq!(interfaces[0].unsupported, None);
        assert_eq!(interfaces[0].scripts.len(), 1);
        let script = &interfaces[0].scripts[0];
        assert_eq!(script.name, "顯示介面");
        assert_eq!(script.find_regex, "/.+/s");
        assert_eq!(script.replace_string, "<div>ok</div>");
        assert_eq!(script.trim_strings, vec!["a".to_owned()]);
        assert_eq!(script.min_depth, Some(1));
        assert_eq!(script.max_depth, Some(2));
    }

    #[test]
    fn read_card_interfaces_detects_scrypt_cards() {
        let root = TestRoot::new("interfaces-scrypt");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let raw = json!({
            "data": {
                "name": "莉亞",
                "first_mes": "<!--SCRYPT PROTECTED-->"
            }
        })
        .to_string();

        import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff", "zh-TW").unwrap();
        let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
        assert_eq!(interfaces.len(), 1);
        assert_eq!(interfaces[0].unsupported, Some("scrypt".to_owned()));
        assert!(interfaces[0].scripts.is_empty());
    }

    #[test]
    fn read_card_interfaces_detects_remote_loader_cards() {
        let root = TestRoot::new("interfaces-remote-loader");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let raw = json!({
            "data": {
                "name": "莉亞",
                "extensions": {
                    "regex_scripts": [{
                        "scriptName": "雲端介面",
                        "findRegex": "/.+/s",
                        "replaceString": "```\n<body>\n<script>\n$('body').load('https://example.github.io/x/index.html')\n</script>\n</body>\n```",
                        "placement": [2]
                    }]
                }
            }
        })
        .to_string();

        import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff", "zh-TW").unwrap();
        let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
        assert_eq!(interfaces.len(), 1);
        assert_eq!(interfaces[0].unsupported, Some("remote_loader".to_owned()));
        assert!(interfaces[0].scripts.is_empty());
    }

    #[test]
    fn read_card_interfaces_handles_plain_cards_without_extensions() {
        let root = TestRoot::new("interfaces-plain");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let raw = json!({"data": {"name": "莉亞"}}).to_string();

        import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff", "zh-TW").unwrap();
        let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
        assert_eq!(interfaces.len(), 1);
        assert!(interfaces[0].scripts.is_empty());
        assert_eq!(interfaces[0].unsupported, None);
        assert!(!interfaces[0].mvu);
    }

    /// 載入 MVU 的判斷：新版 tavern_helper 與舊版 TavernHelper_scripts 都認，停用的腳本不算。
    #[test]
    fn card_interface_detects_mvu_script() {
        let script = |enabled: bool| {
            json!({"name": "MVU基础脚本", "enabled": enabled, "type": "script",
                "content": "import'https://testingcf.jsdelivr.net/gh/MagicalAstrogy/MagVarUpdate/artifact/bundle.js';"})
        };
        let card = |extensions: Value| json!({"name": "卡", "extensions": extensions});
        let mvu = |extensions: Value| card_interface("c", "卡", &card(extensions)).mvu;
        assert!(mvu(
            json!({"tavern_helper": {"scripts": [script(true)], "variables": {}}})
        ));
        assert!(mvu(
            json!({"TavernHelper_scripts": [{"type": "script", "value": script(true)}]})
        ));
        assert!(!mvu(json!({"tavern_helper": {"scripts": [script(false)]}})));
        assert!(!mvu(
            json!({"tavern_helper": {"scripts": [{"name": "別的", "content": "console.log(1)"}]}})
        ));
        assert!(!mvu(json!({"regex_scripts": []})));
    }

    fn scripted_card(name: &str) -> String {
        json!({"data": {"name": name, "extensions": {"regex_scripts": [{
            "scriptName": "顯示介面", "findRegex": "/.+/s",
            "replaceString": "<div>ok</div>", "placement": [2]
        }]}}})
        .to_string()
    }

    fn raw_file(root: &Path, world_id: &str, id: &str, extension: &str) -> std::path::PathBuf {
        data::character_path(root, world_id, id)
            .unwrap()
            .with_extension(extension)
    }

    fn assert_unreadable(card: &CardInterface, id: &str, name: &str) {
        assert_eq!(card.character_id, id);
        assert_eq!(card.character_name, name);
        assert_eq!(card.unsupported.as_deref(), Some(UNREADABLE));
        assert!(card.scripts.is_empty());
        assert!(!card.mvu);
        assert_eq!(card.opening, None);
    }

    /// 原始卡檔壞掉（非法 JSON）：不讓整份清單報錯，回一筆 unreadable，重構才擋得下。
    #[test]
    fn read_card_interfaces_reports_corrupted_raw_file_as_unreadable() {
        let root = TestRoot::new("interfaces-corrupt");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let raw = json!({"data": {"name": "莉亞"}}).to_string();
        let meta =
            import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff", "zh-TW").unwrap();
        fs::write(
            raw_file(root.path(), &world_id, &meta.id, "import.json"),
            b"\xff\xfe not json",
        )
        .unwrap();

        let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
        assert_eq!(interfaces.len(), 1);
        assert_unreadable(&interfaces[0], &meta.id, "莉亞");
    }

    /// JSON 匯入後換了角色圖（一般 PNG、沒有卡片資料）：改讀 .import.json，介面照樣讀得到。
    #[test]
    fn read_card_interfaces_falls_back_to_import_json_behind_plain_png() {
        let root = TestRoot::new("interfaces-plain-png-json");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let meta = import_character(
            root.path(),
            &world_id,
            scripted_card("莉亞").as_bytes(),
            "#3366ff",
            "zh-TW",
        )
        .unwrap();
        crate::import::save_character_image(
            root.path(),
            &world_id,
            &meta.id,
            &crate::import::png_image::test_png::real_png(4, 3),
        )
        .unwrap();

        let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
        assert_eq!(interfaces.len(), 1);
        assert_eq!(interfaces[0].unsupported, None);
        assert_eq!(interfaces[0].scripts.len(), 1);
    }

    /// PNG 匯入後換成一般圖（原卡資料沒了、也沒有 .import.json）：不是損毀，照舊當沒有原卡略過。
    #[test]
    fn read_card_interfaces_skips_plain_png_without_card_data() {
        let root = TestRoot::new("interfaces-plain-png");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let png = crate::import::test_support::card_png(&scripted_card("莉亞"));
        let meta = import_character(root.path(), &world_id, &png, "#3366ff", "zh-TW").unwrap();
        assert!(!raw_file(root.path(), &world_id, &meta.id, "import.json").exists());
        crate::import::save_character_image(
            root.path(),
            &world_id,
            &meta.id,
            &crate::import::png_image::test_png::real_png(4, 3),
        )
        .unwrap();

        assert!(read_card_interfaces(root.path(), &world_id)
            .unwrap()
            .is_empty());
    }

    /// PNG 結構壞掉（截斷）＝損毀，回 unreadable。
    #[test]
    fn read_card_interfaces_reports_truncated_png_as_unreadable() {
        let root = TestRoot::new("interfaces-truncated-png");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let png = crate::import::test_support::card_png(&scripted_card("莉亞"));
        let meta = import_character(root.path(), &world_id, &png, "#3366ff", "zh-TW").unwrap();
        let mut truncated = PNG_MAGIC.to_vec();
        truncated.extend_from_slice(&[0, 0, 0, 9]);
        fs::write(raw_file(root.path(), &world_id, &meta.id, "png"), truncated).unwrap();

        let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
        assert_eq!(interfaces.len(), 1);
        assert_unreadable(&interfaces[0], &meta.id, "莉亞");
    }

    /// 兩份原卡檔的組合：一般圖＋壞 JSON＝unreadable；有效 PNG 原卡＋壞 JSON＝用 PNG；壞 PNG＋好 JSON＝用 JSON。
    #[test]
    fn read_card_interfaces_combines_png_and_import_json() {
        let root = TestRoot::new("interfaces-combos");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let plain = crate::import::png_image::test_png::real_png(4, 3);
        let mut truncated = PNG_MAGIC.to_vec();
        truncated.extend_from_slice(&[0, 0, 0, 9]);
        let import = |raw: &[u8]| {
            import_character(root.path(), &world_id, raw, "#3366ff", "zh-TW")
                .unwrap()
                .id
        };

        let plain_bad = import(scripted_card("甲").as_bytes());
        crate::import::save_character_image(root.path(), &world_id, &plain_bad, &plain).unwrap();
        fs::write(
            raw_file(root.path(), &world_id, &plain_bad, "import.json"),
            b"not json",
        )
        .unwrap();

        let card_bad = import(&crate::import::test_support::card_png(&scripted_card("乙")));
        fs::write(
            raw_file(root.path(), &world_id, &card_bad, "import.json"),
            b"not json",
        )
        .unwrap();

        let bad_good = import(scripted_card("丙").as_bytes());
        fs::write(
            raw_file(root.path(), &world_id, &bad_good, "png"),
            &truncated,
        )
        .unwrap();

        let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
        let find = |id: &str| {
            interfaces
                .iter()
                .find(|card| card.character_id == id)
                .unwrap()
        };
        assert_unreadable(find(&plain_bad), &plain_bad, "甲");
        for id in [&card_bad, &bad_good] {
            assert_eq!(find(id).unsupported, None);
            assert_eq!(find(id).scripts.len(), 1);
        }
    }

    /// 讀檔本身失敗（路徑是資料夾）也算 unreadable；同桌正常的卡照樣列出。
    #[test]
    fn read_card_interfaces_reports_read_failure_beside_healthy_card() {
        let root = TestRoot::new("interfaces-read-failure");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let broken = import_character(
            root.path(),
            &world_id,
            json!({"data": {"name": "莉亞"}}).to_string().as_bytes(),
            "#3366ff",
            "zh-TW",
        )
        .unwrap();
        let healthy = import_character(
            root.path(),
            &world_id,
            scripted_card("艾倫").as_bytes(),
            "#3366ff",
            "zh-TW",
        )
        .unwrap();
        let path = raw_file(root.path(), &world_id, &broken.id, "import.json");
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();

        let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
        assert_eq!(interfaces.len(), 2);
        let broken_card = interfaces
            .iter()
            .find(|card| card.character_id == broken.id)
            .unwrap();
        assert_unreadable(broken_card, &broken.id, "莉亞");
        let healthy_card = interfaces
            .iter()
            .find(|card| card.character_id == healthy.id)
            .unwrap();
        assert_eq!(healthy_card.unsupported, None);
        assert_eq!(healthy_card.scripts.len(), 1);
    }

    /// 世界書路徑：帶顯示腳本的卡存下來後，讀出的介面與角色卡路徑篩選結果一致。
    #[test]
    fn save_world_card_persists_display_scripts_for_worldbook_path() {
        let root = TestRoot::new("world-card-scripts");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let raw = json!({
            "data": {
                "name": "莉亞",
                "extensions": {
                    "regex_scripts": [{
                        "scriptName": "顯示介面",
                        "findRegex": "/.+/s",
                        "replaceString": "<div>ok</div>",
                        "placement": [2]
                    }]
                }
            }
        })
        .to_string();

        assert!(save_world_card(root.path(), &world_id, raw.as_bytes()));

        let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
        assert_eq!(interfaces.len(), 1);
        assert_eq!(interfaces[0].character_id, "");
        assert_eq!(interfaces[0].character_name, "莉亞");
        assert_eq!(interfaces[0].unsupported, None);
        assert_eq!(interfaces[0].scripts.len(), 1);
        assert_eq!(interfaces[0].scripts[0].name, "顯示介面");
    }

    /// 純世界書 JSON（沒有 regex_scripts）不必白存一份原始卡檔。
    #[test]
    fn save_world_card_skips_plain_worldbook_json() {
        let root = TestRoot::new("world-card-plain");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let raw = r#"{"entries":{"0":{"uid":0,"key":["龍"],"content":"沉睡"}}}"#;

        assert!(!save_world_card(root.path(), &world_id, raw.as_bytes()));

        let png_path = data::world_card_path(root.path(), &world_id, "png").unwrap();
        let json_path = data::world_card_path(root.path(), &world_id, "import.json").unwrap();
        assert!(!png_path.exists());
        assert!(!json_path.exists());
    }

    /// 卡片自帶開場白：有值就填入、空字串當沒有。
    #[test]
    fn card_interface_fills_opening_from_first_mes() {
        let with_opening = card_interface(
            "id-1",
            "莉亞",
            &json!({"name": "莉亞", "first_mes": "妳來了。"}),
        );
        assert_eq!(with_opening.opening, Some("妳來了。".to_owned()));

        let without_opening =
            card_interface("id-2", "莉亞", &json!({"name": "莉亞", "first_mes": ""}));
        assert_eq!(without_opening.opening, None);
    }

    /// 世界層級的原始卡檔壞掉：一樣回一筆 unreadable（character_id 與名字都空）。
    #[test]
    fn read_card_interfaces_reports_corrupted_world_level_card_as_unreadable() {
        let root = TestRoot::new("world-card-corrupt");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let path = data::world_card_path(root.path(), &world_id, "import.json").unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"\xff\xfe not json").unwrap();

        let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
        assert_eq!(interfaces.len(), 1);
        assert_unreadable(&interfaces[0], "", "");
    }

    fn interface_script_for_test(find_regex: &str) -> InterfaceScript {
        InterfaceScript {
            name: "顯示".to_owned(),
            find_regex: find_regex.to_owned(),
            replace_string: String::new(),
            trim_strings: Vec::new(),
            min_depth: None,
            max_depth: None,
        }
    }

    /// 走正式的提示選取，測試不另寫一份近似判定。
    fn named(scripts: &[InterfaceScript], worldbook: &[data::WorldbookEntry]) -> Option<String> {
        let scan = crate::transport::test_support::legacy::scan_of(
            worldbook,
            crate::world_scan::Viewer::Gm,
            &[],
            None,
            &[],
            "zh-TW",
        );
        card_format_entry(
            scripts,
            &crate::transport::gm_prompt_full_entries(&scan, &[]),
        )
    }

    fn worldbook_entry_for_test(
        uid: u64,
        title: &str,
        content: &str,
        disabled: bool,
    ) -> data::WorldbookEntry {
        data::WorldbookEntry {
            uid,
            title: title.to_owned(),
            keys: Vec::new(),
            content: content.to_owned(),
            constant: true,
            order: 0,
            disabled,
            visibility: data::Visibility::Gm,
            is_person: false,
            locked: false,
        }
    }

    /// 西幻卡的真實 find_regex：世界書裡含同款標籤的條目才是模型該照的格式規定；
    /// 停用的條目就算含標籤也不能選中；命中標籤數較多的條目優先。
    #[test]
    fn card_format_entry_picks_matching_worldbook_entry() {
        let find_regex = r"/<GoldenRPG_UI>.*?<CurrentView>([\s\S]*?)<\/CurrentView>.*?<WorldSystem>([\s\S]*?)<\/WorldSystem>.*?<LocalSystem>([\s\S]*?)<\/LocalSystem>.*?<GuildBoard>([\s\S]*?)<\/GuildBoard>.*?<CharSheet>([\s\S]*?)<\/CharSheet>.*?<\/GoldenRPG_UI>/s";
        let scripts = vec![interface_script_for_test(find_regex)];
        let worldbook = vec![
            worldbook_entry_for_test(
                1,
                "回复规则",
                "你的每次回复【必须且只能】输出一个标准的XML格式数据块：<GoldenRPG_UI><CurrentView>…</CurrentView></GoldenRPG_UI>",
                false,
            ),
            worldbook_entry_for_test(2, "世界觀", "這是一個劍與魔法的大陸。", false),
            worldbook_entry_for_test(
                3,
                "停用格式條目",
                "<GoldenRPG_UI><CurrentView>備用格式</CurrentView></GoldenRPG_UI>",
                true,
            ),
        ];

        assert_eq!(named(&scripts, &worldbook), Some("回复规则".to_owned()));
    }

    /// 世界書完全沒提到卡片格式要的標籤，就沒有可點名的條目。
    #[test]
    fn card_format_entry_none_when_no_worldbook_entry_matches() {
        let find_regex = r"/<GoldenRPG_UI>.*?<\/GoldenRPG_UI>/s";
        let scripts = vec![interface_script_for_test(find_regex)];
        let worldbook = vec![worldbook_entry_for_test(
            1,
            "世界觀",
            "這是一個劍與魔法的大陸。",
            false,
        )];

        assert_eq!(named(&scripts, &worldbook), None);
    }
}
