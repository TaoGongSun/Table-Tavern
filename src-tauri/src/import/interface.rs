use super::card_io::{decode_png_character, string_field, PNG_MAGIC};
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
}

/// 掃描每張已匯入卡的原始卡檔（PNG／.import.json），抽出可渲染的顯示腳本。
/// 面板是選配功能：壞卡、非匯入卡、解析失敗一律跳過該角色，絕不讓錯誤擋住呼叫端。
pub fn read_card_interfaces(root: &Path, world_id: &str) -> DataResult<Vec<CardInterface>> {
    let mut result = Vec::new();
    for meta in data::list_characters(root, world_id)? {
        if meta.archived || meta.auto_hidden {
            continue;
        }
        let md_path = data::character_path(root, world_id, &meta.id)?;
        let raw_path = [
            md_path.with_extension("png"),
            md_path.with_extension("import.json"),
        ]
        .into_iter()
        .find(|path| path.exists());
        let Some(raw_path) = raw_path else { continue };
        let Ok(bytes) = fs::read(&raw_path) else {
            continue;
        };
        let json_bytes = if bytes.starts_with(PNG_MAGIC) {
            match decode_png_character(&bytes) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            }
        } else {
            bytes
        };
        let Ok(value) = serde_json::from_slice::<Value>(&json_bytes) else {
            continue;
        };
        let card_data = value
            .get("data")
            .filter(|data| data.is_object())
            .unwrap_or(&value);
        result.push(card_interface(&meta.id, &meta.name, card_data));
    }
    for extension in ["png", "import.json"] {
        let Ok(raw_path) = data::world_card_path(root, world_id, extension) else {
            continue;
        };
        let Ok(bytes) = fs::read(&raw_path) else {
            continue;
        };
        let json_bytes = if bytes.starts_with(PNG_MAGIC) {
            match decode_png_character(&bytes) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            }
        } else {
            bytes
        };
        let Ok(value) = serde_json::from_slice::<Value>(&json_bytes) else {
            continue;
        };
        let card_data = value
            .get("data")
            .filter(|data| data.is_object())
            .unwrap_or(&value);
        let name = string_field(card_data, "name").unwrap_or("");
        result.push(card_interface("", name, card_data));
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
    fs::write(path, bytes).is_ok()
}

fn card_interface(character_id: &str, character_name: &str, card_data: &Value) -> CardInterface {
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
    }
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
pub fn card_format_entry(
    scripts: &[InterfaceScript],
    worldbook: &[data::WorldbookEntry],
) -> Option<String> {
    let tags = format_tags(scripts);
    if tags.is_empty() {
        return None;
    }
    worldbook
        .iter()
        .filter(|entry| !entry.disabled)
        .filter_map(|entry| {
            let hits = tags
                .iter()
                .filter(|tag| entry.content.contains(tag.as_str()))
                .count();
            (hits > 0).then_some((entry, hits))
        })
        .max_by_key(|(entry, hits)| (*hits, entry.content.len()))
        .map(|(entry, _)| entry.title.clone())
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

        let meta = import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff").unwrap();
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

        import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff").unwrap();
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

        import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff").unwrap();
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

        import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff").unwrap();
        let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
        assert_eq!(interfaces.len(), 1);
        assert!(interfaces[0].scripts.is_empty());
        assert_eq!(interfaces[0].unsupported, None);
    }

    /// 原始卡檔壞掉（非法 JSON）只能略過該角色，不能讓整份清單報錯。
    #[test]
    fn read_card_interfaces_skips_characters_with_corrupted_raw_file() {
        let root = TestRoot::new("interfaces-corrupt");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let raw = json!({"data": {"name": "莉亞"}}).to_string();
        let meta = import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff").unwrap();

        let raw_path = root.path().join(format!(
            "worlds/{world_id}/characters/{}.import.json",
            meta.id
        ));
        fs::write(&raw_path, b"\xff\xfe not json").unwrap();

        let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
        assert!(interfaces.is_empty());
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

    /// 世界層級的原始卡檔壞掉，一樣只能略過，不能讓整份清單報錯。
    #[test]
    fn read_card_interfaces_skips_corrupted_world_level_card() {
        let root = TestRoot::new("world-card-corrupt");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let path = data::world_card_path(root.path(), &world_id, "import.json").unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"\xff\xfe not json").unwrap();

        let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
        assert!(interfaces.is_empty());
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

        assert_eq!(
            card_format_entry(&scripts, &worldbook),
            Some("回复规则".to_owned())
        );
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

        assert_eq!(card_format_entry(&scripts, &worldbook), None);
    }
}
