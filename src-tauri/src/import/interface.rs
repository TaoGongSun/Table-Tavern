use super::card_io::{decode_png_character, find_png_card, string_field, PNG_MAGIC};
use crate::data::{self, DataResult};
use serde_json::Value;
use std::fs;
use std::path::Path;

mod opening;

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
    /// 開場白美化腳本（`opening::is_opening_decoration`），只在讀原卡（`card_interface`）時算；
    /// 不序列化，反序列化回來一律 false——要用就從 `read_card_interfaces` 的結果拿（`reply_format_scripts`）。
    #[serde(skip)]
    pub opening_only: bool,
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
    /// zod 版 MVU 卡的變數 schema 腳本與 `[initvar]` 原文（card_schema.rs）；只給後端引擎用，不送前端
    #[serde(skip)]
    pub zod: Option<super::card_schema::ZodSchema>,
    /// 卡內 promptOnly 正則腳本原樣（送模型前套，`transport::PromptRegex`）；只給後端，不送前端
    #[serde(skip)]
    pub prompt_scripts: Vec<Value>,
}

/// 掃描每張已匯入卡的原卡檔（`<id>.card.png`／`.import.json`，世界層 `source-card.*`），抽出可渲染的顯示腳本。
/// 面板是選配功能：單張卡出錯絕不讓整份清單報錯。非匯入卡（沒有原卡檔）略過；原卡檔在但讀不出
/// （IO 錯誤、PNG 結構壞、卡片資料解不開）回一筆 `unsupported = "unreadable"`，重構拿它擋下，
/// 不讓讀不出的卡被當成「沒有介面」。
pub fn read_card_interfaces(root: &Path, world_id: &str) -> DataResult<Vec<CardInterface>> {
    let mut result = read_card_interfaces_raw(root, world_id)?;
    // D24：這桌不套卡內 regex 腳本（網頁存檔匯入時玩家沒允許）→ 顯示腳本整組清空，其餘照舊
    if !data::read_state(root, world_id).map_or(true, |state| state.regex_allowed) {
        for card in &mut result {
            card.scripts.clear();
            card.prompt_scripts.clear();
        }
    }
    Ok(result)
}

/// 會作用在模型回覆上的卡片顯示腳本：可用（沒被判不支援）且不是開場白美化。GM 回合格式
/// （`chat_assembly::gm_turn_instruction`）與「有沒有被要求報 present」（`data::requires_present_report`）
/// 共用這一份，兩邊判定才不會分岔。讀不到卡當沒有腳本。
pub fn reply_format_scripts(root: &Path, world_id: &str) -> Vec<InterfaceScript> {
    read_card_interfaces(root, world_id)
        .unwrap_or_default()
        .into_iter()
        .filter(|interface| interface.unsupported.is_none())
        .flat_map(|interface| interface.scripts)
        .filter(|script| !script.opening_only)
        .collect()
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
        zod: None,
        prompt_scripts: Vec::new(),
    }
}

fn read_card_interfaces_raw(root: &Path, world_id: &str) -> DataResult<Vec<CardInterface>> {
    let mut result = Vec::new();
    // 角色層：原卡只認 `<id>.card.png`／`.import.json`（顯示圖 `<id>.png` 從不當原卡）。兩份都沒有＝
    // 非匯入卡，略過；任一份讀得出就用它，都讀不出才算 unreadable。`.card.png` 只在匯入時寫、必帶
    // 卡片資料，所以讀出是一般圖也算損毀。
    for meta in data::list_characters(root, world_id)? {
        // 主區的發言角色與舞台卡（舞台卡常是介面／MVU 載體）；封存與自動隱藏的略過
        if !matches!(
            meta.cast_role(),
            data::CastRole::Speaker | data::CastRole::Stage
        ) {
            continue;
        }
        let md_path = data::character_path(root, world_id, &meta.id)?;
        let mut card_data = None;
        let mut broken = None;
        for path in [
            md_path.with_extension("card.png"),
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
                Ok(None) => broken = broken.or(Some((path, "PNG has no card data".to_owned()))),
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
            zod: None,
            prompt_scripts: Vec::new(),
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
    let greetings: Vec<&str> = string_field(card_data, "first_mes")
        .into_iter()
        .chain(
            card_data
                .get("alternate_greetings")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str),
        )
        .collect();
    let scripts: Vec<InterfaceScript> = scripts
        .into_iter()
        .map(|mut script| {
            script.opening_only = opening::is_opening_decoration(&script, &greetings);
            script
        })
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
        zod: super::card_schema::zod_schema(card_data),
        prompt_scripts: card_data
            .get("extensions")
            .and_then(|extensions| extensions.get("regex_scripts"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|script| script.get("promptOnly").and_then(Value::as_bool) == Some(true))
            .cloned()
            .collect(),
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
        opening_only: false,
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
mod tests;
