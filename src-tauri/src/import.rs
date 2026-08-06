use crate::data::{
    self, CharacterCard, CharacterMeta, DataResult, FieldKind, FieldRule, StateNode, Tier,
};
use crate::mechanism::{self, Record, RecordKind};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const PNG_MAGIC: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

/// 人設欄：既是 lorebook_heavy 秤重時的「人設份量」，也是沒有條目的卡轉成世界書時要收的內容。
/// 不含 first_mes——開場白走 card_openings 讓玩家挑，收進條目會每回合重複注入。
const PERSONA_FIELDS: [&str; 4] = ["description", "personality", "scenario", "mes_example"];

/// 公開段落與 SillyTavern 欄位的對照表：匯入時拆成 `### 標題`，匯出時再併回欄位
const PUBLIC_SECTIONS: [(&str, &str); 5] = [
    ("簡介", "description"),
    ("人格與語氣", "personality"),
    ("場景", "scenario"),
    ("開場白", "first_mes"),
    ("語氣範例", "mes_example"),
];

#[derive(serde::Serialize, Default, Debug, PartialEq)]
pub struct ImportProbe {
    pub lorebook_heavy: bool,
    /// 卡名（世界書卡也有）：匯入後自動名桌拿它當桌名
    pub name: Option<String>,
    /// 頂層就是世界書本體（V2 獨立書 JSON 自帶 name＋entries）：有 name 也要走世界書
    pub book_shaped: bool,
    /// JSON／PNG 成功解析才 true：前端靠這個分辨「格式錯誤」與「解析成功但沒有名字」
    pub parsed: bool,
    /// character_book.entries 的條目數，沒有這個欄位就是 0
    pub book_entries: usize,
    /// 備用開場白數：一張卡備了好幾個開局＝這是一座舞台不是一個人。
    /// 不看 first_mes——每張角色卡都有，零鑑別力（TestCards 22 檔實測 18／18 有值）。
    pub alternate_greetings: usize,
}

/// 匯入前只提示可能無法保留的內容；真正的格式錯誤仍交給匯入處理。
pub fn probe_import(bytes: &[u8]) -> ImportProbe {
    let json_bytes = if bytes.starts_with(PNG_MAGIC) {
        match decode_png_character(bytes) {
            Ok(bytes) => bytes,
            Err(_) => return ImportProbe::default(),
        }
    } else {
        bytes.to_vec()
    };
    let value: Value = match serde_json::from_slice(&json_bytes) {
        Ok(value) => value,
        Err(_) => return ImportProbe::default(),
    };
    let card_data = value
        .get("data")
        .filter(|data| data.is_object())
        .unwrap_or(&value);
    let mut probe = ImportProbe {
        parsed: true,
        ..ImportProbe::default()
    };
    let book_entries = card_data
        .get("character_book")
        .and_then(|book| book.get("entries"))
        .and_then(Value::as_array);
    probe.book_entries = book_entries.map_or(0, Vec::len);
    // 世界書卡＝內容重心壓倒性地在世界書條目上，看比重而非人設絕對字數：
    // 這種卡匯成角色卡會把整包條目（含輸出格式規定）丟掉，卡就玩不動了。
    // 真卡實測：西幻卡人設 988 字、世界書 21,678 字（22 倍），舊的「人設少於 200 字」條件漏判它。
    probe.lorebook_heavy = book_entries.is_some_and(|entries| {
        let book: usize = entries
            .iter()
            .filter_map(|entry| entry.get("content").and_then(Value::as_str))
            .map(|content| content.chars().count())
            .sum();
        let persona: usize = PERSONA_FIELDS
            .into_iter()
            .map(|field| {
                string_field(card_data, field)
                    .unwrap_or("")
                    .trim()
                    .chars()
                    .count()
            })
            .sum();
        entries.len() >= 3 && book >= persona.saturating_mul(3)
    });
    probe.alternate_greetings = card_data
        .get("alternate_greetings")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    probe.name = string_field(card_data, "name")
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty());
    // V2 規格的獨立世界書 JSON 頂層就是書本體：自帶 name（書名）＋entries。
    // 角色卡的 entries 只會在 character_book 底下，頂層有 entries＝這包是世界書，別當角色卡。
    probe.book_shaped =
        card_data.get("character_book").is_none() && card_data.get("entries").is_some();
    probe
}

/// 匯入永遠是全新一張卡：mint 新 id，name 照卡片原值（不再擋特殊字元，只擋換行）。
pub fn import_character(
    root: &Path,
    world_id: &str,
    bytes: &[u8],
    color: &str,
) -> DataResult<CharacterMeta> {
    let (json_bytes, raw_extension) = if bytes.starts_with(PNG_MAGIC) {
        (decode_png_character(bytes)?, "png")
    } else {
        (bytes.to_vec(), "import.json")
    };
    let value: Value = serde_json::from_slice(&json_bytes)
        .map_err(|error| data::invalid_data(format!("角色卡 JSON 無法解析：{error}")))?;
    let card_data = value
        .get("data")
        .filter(|data| data.is_object())
        .unwrap_or(&value);
    let name = string_field(card_data, "name")
        .ok_or_else(|| data::invalid_data("角色卡缺少 name"))?
        .trim()
        .to_owned();
    data::validate_single_line("name", &name)?;

    let id = data::new_id();
    let md_path = data::character_path(root, world_id, &id)?;

    let card = CharacterCard {
        id: id.clone(),
        name: name.clone(),
        color: color.to_owned(),
        avatar: "🎭".to_owned(),
        tier: Tier::Balanced,
        show_image: true,
        archived: false,
        gen_prompt: String::new(),
        public_md: public_markdown(card_data),
        private_md: private_markdown(card_data),
    };
    data::write_character(root, world_id, &card)?;
    fs::write(md_path.with_extension(raw_extension), bytes)?;
    import_table_tavern_extension(root, world_id, &name, card_data);
    if let Some(book) = card_data.get("character_book") {
        import_mechanism(root, world_id, book);
        // 卡片隨身的設定條目也帶進這桌世界書：以前整包丟掉，模型看不到這角色的家鄉家人秘密，
        // 卡片自訂的輸出格式規定也一併消失（同名條目由 import_worldbook 自行去重）
        if let Ok(text) = serde_json::to_string(book) {
            let _ = data::import_worldbook(root, world_id, &text);
        }
    }

    Ok(CharacterMeta {
        id,
        name,
        color: color.to_owned(),
        avatar: "🎭".to_owned(),
        tier: Tier::Balanced,
        show_image: true,
        archived: false,
        display_index: None,
    })
}

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
        if meta.archived {
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

/// 匯出成 SillyTavern chara_card_v2：內容一律由現在的卡重建（匯入後改過的字才會跟著出去）。
/// 副檔名 .json 直接寫 JSON，其餘寫 PNG——把 JSON 塞進 tEXt chara chunk，底圖用這張卡的圖。
pub fn export_character(
    root: &Path,
    world_id: &str,
    character_id: &str,
    path: &Path,
) -> DataResult<()> {
    let card = data::read_character(root, world_id, character_id)?;
    let json = serde_json::to_vec_pretty(&character_card_v2(root, world_id, &card))?;
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        fs::write(path, json)?;
    } else {
        fs::write(
            path,
            embed_chara_chunk(&export_base_png(root, world_id, character_id)?, &json)?,
        )?;
    }
    Ok(())
}

fn character_card_v2(root_dir: &Path, world_id: &str, card: &CharacterCard) -> Value {
    let sections = split_public_markdown(&card.public_md);
    let mut data = serde_json::Map::new();
    for ((_, field), content) in PUBLIC_SECTIONS.into_iter().zip(sections) {
        data.insert(field.to_owned(), Value::String(content));
    }
    let mut root = data.clone();
    data.insert("name".to_owned(), Value::String(card.name.clone()));
    for (field, value) in [
        ("creator_notes", json!("")),
        ("system_prompt", json!("")),
        ("post_history_instructions", json!("")),
        ("alternate_greetings", json!([])),
        ("tags", json!([])),
        ("creator", json!("")),
        ("character_version", json!("")),
        (
            "extensions",
            table_tavern_extension(root_dir, world_id, &card.name),
        ),
    ] {
        data.insert(field.to_owned(), value);
    }
    if let Some(book) = character_book(&card.private_md, &card.name) {
        data.insert("character_book".to_owned(), book);
    }
    // 頂層同時放 V1 欄位：只吃舊格式的工具也讀得到（SillyTavern 自己匯出時也這樣寫）
    root.insert("name".to_owned(), Value::String(card.name.clone()));
    root.insert("spec".to_owned(), json!("chara_card_v2"));
    root.insert("spec_version".to_owned(), json!("2.0"));
    root.insert("data".to_owned(), Value::Object(data));
    Value::Object(root)
}

/// 機制資料是附加資訊；讀不到這桌狀態時仍要讓玩家帶走可用的普通角色卡。
fn table_tavern_extension(root: &Path, world_id: &str, name: &str) -> Value {
    let Ok(world) = data::read_state(root, world_id) else {
        return json!({});
    };
    let prefix = format!("{name}.");
    let rules: BTreeMap<_, _> = world
        .mechanism
        .rules
        .iter()
        .filter_map(|(path, rule)| {
            (rule.branch.as_deref() == Some(name))
                .then(|| {
                    path.strip_prefix(&prefix)
                        .map(|relative| (relative.to_owned(), rule.clone()))
                })
                .flatten()
        })
        .collect();
    let initial = world.state.tree.get(name).cloned();
    if rules.is_empty() && initial.is_none() {
        return json!({});
    }
    let mut table_tavern = serde_json::Map::new();
    table_tavern.insert("version".to_owned(), json!(1));
    if !rules.is_empty() {
        table_tavern.insert(
            "rules".to_owned(),
            serde_json::to_value(rules).unwrap_or_default(),
        );
    }
    if let Some(initial) = initial {
        table_tavern.insert(
            "initial".to_owned(),
            serde_json::to_value(initial).unwrap_or_default(),
        );
    }
    json!({ "table_tavern": table_tavern })
}

/// 壞掉的擴充資料只略過，因為角色本體已經安全存檔，不能被可選機制拖垮。
/// 世界書路徑用：從原始卡檔（PNG／JSON）取出本 app 自己匯出的 `extensions.table_tavern` 再套用。
/// 角色卡路徑在 import_character 內已直接套過；兩條路徑都要收，同一張卡不會因為換個身分匯入就少半套機制。
pub fn import_card_extension(root: &Path, world_id: &str, name: &str, bytes: &[u8]) {
    let json_bytes = if bytes.starts_with(PNG_MAGIC) {
        match decode_png_character(bytes) {
            Ok(json_bytes) => json_bytes,
            Err(_) => return,
        }
    } else {
        bytes.to_vec()
    };
    let Ok(value) = serde_json::from_slice::<Value>(&json_bytes) else {
        return;
    };
    let card_data = value
        .get("data")
        .filter(|data| data.is_object())
        .unwrap_or(&value);
    import_table_tavern_extension(root, world_id, name, card_data);
}

fn import_table_tavern_extension(root: &Path, world_id: &str, name: &str, card_data: &Value) {
    let Some(extension) = card_data
        .get("extensions")
        .and_then(|extensions| extensions.get("table_tavern"))
    else {
        return;
    };
    let Ok(mut world) = data::read_state(root, world_id) else {
        return;
    };
    let mut changed = false;
    if let Some(rules) = extension.get("rules") {
        if let Ok(rules) = serde_json::from_value::<BTreeMap<String, FieldRule>>(rules.clone()) {
            for (relative, mut rule) in rules {
                if relative.is_empty() {
                    continue;
                }
                rule.branch = Some(name.to_owned());
                world
                    .mechanism
                    .rules
                    .insert(format!("{name}.{relative}"), rule);
                changed = true;
            }
        }
    }
    if let Some(initial) = extension.get("initial") {
        if let Ok(initial) = serde_json::from_value::<StateNode>(initial.clone()) {
            match world.state.tree.get_mut(name) {
                Some(existing) => merge_state_node(existing, initial, true),
                None => {
                    world.state.tree.insert(name.to_owned(), initial);
                }
            }
            changed = true;
        }
    }
    if changed {
        let _ = data::write_state(root, world_id, &world);
    }
}

fn merge_state_node(existing: &mut StateNode, incoming: StateNode, overwrite: bool) {
    match (existing, incoming) {
        (StateNode::Branch(existing), StateNode::Branch(incoming)) => {
            for (key, node) in incoming {
                match existing.get_mut(&key) {
                    Some(current) => merge_state_node(current, node, overwrite),
                    None => {
                        existing.insert(key, node);
                    }
                }
            }
        }
        (existing, incoming) if overwrite => *existing = incoming,
        _ => {}
    }
}

/// 匯入 MVU 機制鷹架與固定型 EJS：`[initvar]` 停用條目給初始狀態樹、`[mvu_update]` 條目給欄位規則表，
/// EJS 只收成可本地求值的觸發表；一次掃完只寫一次 state.json。壞格式一律略過，不阻斷正常匯入。
pub fn import_mechanism(root: &Path, world_id: &str, book: &Value) {
    let Some(entries) = book.get("entries") else {
        return;
    };
    let entries: Vec<&Value> = match entries {
        Value::Array(entries) => entries.iter().collect(),
        Value::Object(entries) => entries.values().collect(),
        _ => return,
    };

    let initial_tree = extract_initial_tree(&entries);
    let (rules, mvu_seen) = extract_field_rules(&entries);
    let (triggers, skipped) = extract_triggers(&entries);
    let incremental = initial_tree.is_some() || mvu_seen;
    if initial_tree.is_none() && rules.is_empty() && triggers.is_empty() && !incremental {
        return;
    }

    let Ok(mut world) = data::read_state(root, world_id) else {
        return;
    };
    if let Some(tree) = initial_tree {
        for (key, node) in tree {
            match world.state.tree.get_mut(&key) {
                Some(existing) => merge_state_node(existing, node, false),
                None => {
                    world.state.tree.insert(key, node);
                }
            }
        }
    }
    for (path, rule) in rules {
        world.mechanism.rules.insert(path, rule);
    }
    for trigger in triggers {
        if let Some(flag) = trigger.flag.as_ref() {
            let mut rule = FieldRule::for_kind(FieldKind::ReadOnly);
            rule.branch = flag.split('.').next().map(str::to_owned);
            world.mechanism.rules.insert(flag.clone(), rule);
        }
        if let Some(index) = world
            .mechanism
            .triggers
            .iter()
            .position(|existing| existing.id == trigger.id)
        {
            world.mechanism.triggers[index] = trigger;
        } else {
            world.mechanism.triggers.push(trigger);
        }
    }
    if incremental {
        world.mechanism.incremental = true;
    }
    let _ = data::write_state(root, world_id, &world);
    mechanism::append_log(root, world_id, world.current_scene, &skipped);
}

/// EJS 原文從不送模型：可辨識的才轉成觸發表，其餘留一筆記帳讓玩家知道沒有偷偷執行。
fn extract_triggers(entries: &[&Value]) -> (Vec<data::Trigger>, Vec<Record>) {
    let mut triggers = Vec::new();
    let mut skipped = Vec::new();
    for entry in entries {
        let Some(content) = entry.get("content").and_then(Value::as_str) else {
            continue;
        };
        if !content.contains("<%") {
            continue;
        }
        let title = entry
            .get("comment")
            .and_then(Value::as_str)
            .or_else(|| entry.get("title").and_then(Value::as_str))
            .unwrap_or_default();
        if let Some(trigger) = crate::ejs::parse_triggers(title, content) {
            if let Some(index) = triggers
                .iter()
                .position(|existing: &data::Trigger| existing.id == trigger.id)
            {
                triggers[index] = trigger;
            } else {
                triggers.push(trigger);
            }
        } else {
            skipped.push(Record {
                kind: RecordKind::Skipped,
                path: title.to_owned(),
                detail: "卡片腳本認不出來，沒轉成觸發表，預設不送模型。".to_owned(),
            });
        }
    }
    (triggers, skipped)
}

fn entry_marker(entry: &Value) -> Option<String> {
    entry
        .get("comment")
        .and_then(Value::as_str)
        .or_else(|| entry.get("title").and_then(Value::as_str))
        .map(|marker| marker.trim().to_ascii_lowercase())
}

/// `[initvar]` 初始狀態樹：只認第一條「停用且標記匹配」的條目，內容壞掉就整段放棄。
fn extract_initial_tree(entries: &[&Value]) -> Option<BTreeMap<String, StateNode>> {
    let entry = entries.iter().find(|entry| {
        let Some(marker) = entry_marker(entry) else {
            return false;
        };
        let disabled = entry.get("enabled").and_then(Value::as_bool) == Some(false)
            || entry.get("disable").and_then(Value::as_bool) == Some(true);
        disabled && marker.starts_with("[initvar]")
    })?;
    let content = entry.get("content").and_then(Value::as_str)?;
    if content.trim().is_empty() {
        return None;
    }
    let mut tree = BTreeMap::new();
    for (path, value) in crate::transport::parse_indented_fields(content) {
        insert_initial_tree_node(&mut tree, &path, value);
    }
    (!tree.is_empty()).then_some(tree)
}

/// `[mvu_update]` 欄位規則表：逐條處理（可能不只一條）；回傳規則表與是否掃到過標記
/// （掃到但抽不出任何規則，這桌仍要標成增量桌——模型已經在照協定回覆）。
fn extract_field_rules(entries: &[&Value]) -> (BTreeMap<String, FieldRule>, bool) {
    let mut rules = BTreeMap::new();
    let mut seen = false;
    for entry in entries {
        let Some(marker) = entry_marker(entry) else {
            continue;
        };
        if !marker.starts_with("[mvu_update]") {
            continue;
        }
        seen = true;
        if let Some(content) = entry.get("content").and_then(Value::as_str) {
            collect_field_rules(content, &mut rules);
        }
    }
    (rules, seen)
}

/// 一條規則路徑抽到的三個屬性值（type, range, format），同一條路徑的多筆屬性合併於此。
type RuleAttrValues = (Option<String>, Option<String>, Option<String>);

/// 從一段規則表文字抽欄位規則：只收路徑長度 ≥ 3 且最後一段是 type／range／format 的項目
/// （這條過濾就把同前綴的「輸出格式」說明條目整條擋掉，不必特判）；規則路徑＝去掉根與屬性名，
/// 任何一段是 check 就丟掉；同一條路徑的多個屬性合併成一條規則。
fn collect_field_rules(content: &str, rules: &mut BTreeMap<String, FieldRule>) {
    let mut attrs: BTreeMap<Vec<String>, RuleAttrValues> = BTreeMap::new();
    for (path, value) in crate::transport::parse_indented_fields(content) {
        let Some(value) = value else { continue };
        if path.len() < 3 || path.iter().any(|segment| segment == "check") {
            continue;
        }
        let attribute = path[path.len() - 1].as_str();
        if !matches!(attribute, "type" | "range" | "format") {
            continue;
        }
        let Some(expanded) = expand_rule_paths(&path[1..path.len() - 1]) else {
            continue;
        };
        for rule_path in expanded {
            let entry = attrs.entry(rule_path).or_insert((None, None, None));
            match attribute {
                "type" => entry.0 = Some(value.clone()),
                "range" => entry.1 = Some(value.clone()),
                "format" => entry.2 = Some(value.clone()),
                _ => unreachable!("已由上面的 matches! 篩過"),
            }
        }
    }
    for (rule_path, (type_value, range_value, format_value)) in attrs {
        let Some(rule) = build_field_rule(
            &rule_path,
            type_value.as_deref(),
            range_value.as_deref(),
            format_value.as_deref(),
        ) else {
            continue;
        };
        rules.insert(rule_path.join("."), rule);
    }
}

/// ST 怪癖正規化（只准關在這裡）：剝引號、拆 `.` 合併段、展開 `${a|b}`（多選一）／
/// `${x}`（換成萬用段 `*`）／`a/b/c`（同層併寫的多個欄位）；組合數超過 32 就整條放棄。
fn expand_rule_paths(raw_segments: &[String]) -> Option<Vec<Vec<String>>> {
    let mut segments = Vec::new();
    for raw in raw_segments {
        for part in strip_enclosing_quotes(raw).split('.') {
            if !part.is_empty() {
                segments.push(part.to_owned());
            }
        }
    }
    if segments.is_empty() {
        return None;
    }
    let choices: Vec<Vec<String>> = segments
        .iter()
        .map(|segment| expand_segment(segment))
        .collect();
    let total = choices
        .iter()
        .try_fold(1usize, |acc, choice| acc.checked_mul(choice.len()))?;
    if total == 0 || total > 32 {
        return None;
    }
    let mut results = vec![Vec::new()];
    for choice in &choices {
        let mut next = Vec::with_capacity(results.len() * choice.len());
        for existing in &results {
            for candidate in choice {
                let mut combo = existing.clone();
                combo.push(candidate.clone());
                next.push(combo);
            }
        }
        results = next;
    }
    Some(results)
}

fn strip_enclosing_quotes(segment: &str) -> String {
    let trimmed = segment.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        return trimmed[1..trimmed.len() - 1].to_owned();
    }
    trimmed.to_owned()
}

fn expand_segment(segment: &str) -> Vec<String> {
    if let Some(inner) = segment
        .strip_prefix("${")
        .and_then(|rest| rest.strip_suffix('}'))
    {
        if inner.contains('|') {
            let parts: Vec<String> = inner
                .split('|')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(str::to_owned)
                .collect();
            if !parts.is_empty() {
                return parts;
            }
        }
        return vec!["*".to_owned()];
    }
    if segment.contains('/') {
        let parts: Vec<&str> = segment.split('/').collect();
        if parts
            .iter()
            .all(|part| !part.is_empty() && !part.contains(char::is_whitespace))
        {
            return parts.into_iter().map(str::to_owned).collect();
        }
    }
    vec![segment.to_owned()]
}

/// 屬性三元組（type／range／format）→ FieldRule；完全推不出 kind 的路徑不建規則。
fn build_field_rule(
    rule_path: &[String],
    type_value: Option<&str>,
    range_value: Option<&str>,
    format_value: Option<&str>,
) -> Option<FieldRule> {
    let is_pair_format = format_value
        .is_some_and(|value| value.contains("当前值/上限值") || value.contains("當前值/上限值"));
    let kind = if is_pair_format {
        Some(FieldKind::Pair)
    } else {
        match type_value {
            Some("number") => Some(FieldKind::Number),
            Some("string") => Some(FieldKind::Text),
            _ => None,
        }
    };
    let mut kind = kind?;
    let last = rule_path.last()?.to_ascii_lowercase();
    if kind == FieldKind::Number
        && last
            .strip_prefix("roll")
            .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_digit()))
    {
        kind = FieldKind::Roll;
    }
    let mut rule = FieldRule::for_kind(kind);
    if let Some((min, max)) = range_value.and_then(|range| range.split_once('-')) {
        if let (Ok(min), Ok(max)) = (min.trim().parse::<f64>(), max.trim().parse::<f64>()) {
            rule.min = Some(min);
            rule.max = Some(max);
        }
    }
    rule.branch = rule_path.first().cloned();
    Some(rule)
}

fn insert_initial_tree_node(
    children: &mut BTreeMap<String, StateNode>,
    path: &[String],
    value: Option<String>,
) {
    let Some((key, rest)) = path.split_first() else {
        return;
    };
    if rest.is_empty() {
        match value {
            Some(value) => {
                children.insert(key.clone(), StateNode::Leaf(value));
            }
            None => {
                children
                    .entry(key.clone())
                    .or_insert_with(|| StateNode::Branch(BTreeMap::new()));
            }
        }
        return;
    }
    let child = children
        .entry(key.clone())
        .or_insert_with(|| StateNode::Branch(BTreeMap::new()));
    if let StateNode::Branch(children) = child {
        insert_initial_tree_node(children, rest, value);
    }
}

/// 世界書卡不會先建成角色，仍要直接從匯入檔取得所有可選開場白。
pub fn card_openings(bytes: &[u8]) -> Option<(String, Vec<String>)> {
    let json_bytes = if bytes.starts_with(PNG_MAGIC) {
        decode_png_character(bytes).ok()?
    } else {
        bytes.to_vec()
    };
    let value: Value = serde_json::from_slice(&json_bytes).ok()?;
    let card_data = value
        .get("data")
        .filter(|data| data.is_object())
        .unwrap_or(&value);
    let mut openings = Vec::new();
    if let Some(opening) = string_field(card_data, "first_mes") {
        let opening = opening.trim();
        if !opening.is_empty() {
            openings.push(opening.to_owned());
        }
    }
    if let Some(alternates) = card_data
        .get("alternate_greetings")
        .and_then(Value::as_array)
    {
        openings.extend(
            alternates
                .iter()
                .filter_map(Value::as_str)
                .filter_map(|opening| {
                    let opening = opening.trim();
                    (!opening.is_empty()).then(|| opening.to_owned())
                }),
        );
    }
    if openings.is_empty() {
        return None;
    }
    let name = string_field(card_data, "name")
        .unwrap_or_default()
        .trim()
        .to_owned();
    Some((name, openings))
}

/// public_markdown 的反向：認得的 `### 標題` 各自回原欄位，其餘（App 內手寫的卡）全歸簡介
fn split_public_markdown(markdown: &str) -> [String; PUBLIC_SECTIONS.len()] {
    let mut sections: [String; PUBLIC_SECTIONS.len()] = Default::default();
    let mut current = 0;
    for line in markdown.lines() {
        if let Some(index) = PUBLIC_SECTIONS
            .iter()
            .position(|(heading, _)| line.trim_end() == format!("### {heading}"))
        {
            current = index;
            continue;
        }
        if !sections[current].is_empty() {
            sections[current].push('\n');
        }
        sections[current].push_str(line);
    }
    sections.map(|section| section.trim().to_owned())
}

/// private_markdown 的反向：`- **關鍵字**：內容` 回成有關鍵字的條目，
/// 其餘私有筆記併成一條沒有關鍵字的常駐條目（ST 那邊 constant 才會固定注入）
fn character_book(private_md: &str, name: &str) -> Option<Value> {
    if private_md.trim().is_empty() {
        return None;
    }
    let mut entries: Vec<(Vec<&str>, String)> = Vec::new();
    let mut loose: Vec<&str> = Vec::new();
    for line in private_md.lines() {
        match line
            .strip_prefix("- **")
            .and_then(|rest| rest.split_once("**："))
        {
            Some((keys, content)) if !content.trim().is_empty() => entries.push((
                keys.split('、')
                    .map(str::trim)
                    .filter(|key| !key.is_empty())
                    .collect(),
                content.trim().to_owned(),
            )),
            _ => loose.push(line),
        }
    }
    let loose = loose.join("\n").trim().to_owned();
    if !loose.is_empty() {
        entries.push((Vec::new(), loose));
    }
    let entries = entries
        .into_iter()
        .enumerate()
        .map(|(index, (keys, content))| {
            json!({
                "id": index,
                "keys": keys,
                "secondary_keys": [],
                "comment": "",
                "content": content,
                "constant": keys.is_empty(),
                "selective": !keys.is_empty(),
                "insertion_order": index,
                "enabled": true,
                "position": "before_char",
                "case_sensitive": false,
                "extensions": {},
            })
        })
        .collect::<Vec<_>>();
    Some(json!({ "name": name, "entries": entries, "extensions": {} }))
}

/// 匯出底圖：優先用卡片圖，其次頭像，都沒有就給一張 1×1 透明 PNG（ST 讀的是 tEXt，圖只是外觀）
fn export_base_png(root: &Path, world_id: &str, character_id: &str) -> DataResult<Vec<u8>> {
    let path = data::character_path(root, world_id, character_id)?;
    for extension in ["png", "avatar.png"] {
        let candidate = path.with_extension(extension);
        if candidate.is_file() {
            let bytes = fs::read(candidate)?;
            if bytes.starts_with(PNG_MAGIC) {
                return Ok(bytes);
            }
        }
    }
    Ok(blank_png())
}

fn blank_png() -> Vec<u8> {
    let mut png = PNG_MAGIC.to_vec();
    let mut header = 1u32.to_be_bytes().to_vec();
    header.extend_from_slice(&1u32.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
    png.extend_from_slice(&png_chunk(b"IHDR", &header));
    // zlib（stored deflate）：一列 filter 0 + 一個全透明像素
    const PIXEL: &[u8] = &[
        0x78, 0x01, 0x01, 0x05, 0x00, 0xfa, 0xff, 0, 0, 0, 0, 0, 0x00, 0x05, 0x00, 0x01,
    ];
    png.extend_from_slice(&png_chunk(b"IDAT", PIXEL));
    png.extend_from_slice(&png_chunk(b"IEND", &[]));
    png
}

/// 把角色卡 JSON 寫成 tEXt chara chunk 放進 IEND 前；底圖原有的卡片 chunk（含 V3 的 ccv3）
/// 先清掉，不然 ST 會讀到匯入當下那份舊資料
fn embed_chara_chunk(base: &[u8], json: &[u8]) -> DataResult<Vec<u8>> {
    if !base.starts_with(PNG_MAGIC) {
        return Err(data::invalid_data("匯出底圖必須是 PNG"));
    }
    let mut output = PNG_MAGIC.to_vec();
    let mut offset = PNG_MAGIC.len();
    while offset < base.len() {
        if base.len() - offset < 12 {
            return Err(data::invalid_data("PNG chunk 格式不完整"));
        }
        let length = u32::from_be_bytes(base[offset..offset + 4].try_into().unwrap()) as usize;
        let chunk_end = offset
            .checked_add(12)
            .and_then(|end| end.checked_add(length))
            .ok_or_else(|| data::invalid_data("PNG chunk 長度無效"))?;
        if chunk_end > base.len() {
            return Err(data::invalid_data("PNG chunk 長度超出檔案範圍"));
        }
        let kind = &base[offset + 4..offset + 8];
        let chunk_data = &base[offset + 8..offset + 8 + length];
        if kind == b"IEND" {
            let mut text = b"chara\0".to_vec();
            text.extend_from_slice(base64_encode(json).as_bytes());
            output.extend_from_slice(&png_chunk(b"tEXt", &text));
            output.extend_from_slice(&base[offset..chunk_end]);
            return Ok(output);
        }
        let is_card_text = matches!(kind, b"tEXt" | b"zTXt" | b"iTXt")
            && chunk_data
                .split(|byte| *byte == 0)
                .next()
                .is_some_and(|keyword| keyword == b"chara" || keyword == b"ccv3");
        if !is_card_text {
            output.extend_from_slice(&base[offset..chunk_end]);
        }
        offset = chunk_end;
    }
    Err(data::invalid_data("PNG 缺少 IEND chunk"))
}

fn png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut chunk = (data.len() as u32).to_be_bytes().to_vec();
    chunk.extend_from_slice(kind);
    chunk.extend_from_slice(data);
    chunk.extend_from_slice(&crc32(&chunk[4..]).to_be_bytes());
    chunk
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in bytes {
        crc ^= *byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 == 0 {
                crc >> 1
            } else {
                (crc >> 1) ^ 0xedb8_8320
            };
        }
    }
    !crc
}

/// 匯入時存下的原 PNG（characters/<id>.png）；沒有圖回 None，前端拿 base64 組 data URL 顯示
pub fn character_image(
    root: &Path,
    world_id: &str,
    character_id: &str,
) -> DataResult<Option<String>> {
    let path = data::character_path(root, world_id, character_id)?.with_extension("png");
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(base64_encode(&fs::read(path)?)))
}

pub fn save_character_image(
    root: &Path,
    world_id: &str,
    character_id: &str,
    bytes: &[u8],
) -> DataResult<()> {
    save_character_png(root, world_id, character_id, bytes, "png")
}

pub fn delete_character_image(root: &Path, world_id: &str, character_id: &str) -> DataResult<()> {
    delete_character_png(root, world_id, character_id, "png")
}

pub fn character_avatar(
    root: &Path,
    world_id: &str,
    character_id: &str,
) -> DataResult<Option<String>> {
    let path = data::character_path(root, world_id, character_id)?.with_extension("avatar.png");
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(base64_encode(&fs::read(path)?)))
}

pub fn save_character_avatar(
    root: &Path,
    world_id: &str,
    character_id: &str,
    bytes: &[u8],
) -> DataResult<()> {
    save_character_png(root, world_id, character_id, bytes, "avatar.png")
}

pub fn delete_character_avatar(root: &Path, world_id: &str, character_id: &str) -> DataResult<()> {
    delete_character_png(root, world_id, character_id, "avatar.png")
}

fn save_character_png(
    root: &Path,
    world_id: &str,
    character_id: &str,
    bytes: &[u8],
    extension: &str,
) -> DataResult<()> {
    if !bytes.starts_with(PNG_MAGIC) {
        return Err(data::invalid_data("圖片必須是 PNG"));
    }
    let path = data::character_path(root, world_id, character_id)?;
    if !path.exists() {
        return Err(data::invalid_data(format!("角色 {character_id} 不存在")));
    }
    fs::write(path.with_extension(extension), bytes)?;
    Ok(())
}

fn delete_character_png(
    root: &Path,
    world_id: &str,
    character_id: &str,
    extension: &str,
) -> DataResult<()> {
    let path = data::character_path(root, world_id, character_id)?.with_extension(extension);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn string_field<'a>(data: &'a Value, field: &str) -> Option<&'a str> {
    data.get(field).and_then(Value::as_str)
}

fn public_markdown(data: &Value) -> String {
    PUBLIC_SECTIONS
        .into_iter()
        .filter_map(|(heading, field)| {
            let content = string_field(data, field)?;
            (!content.trim().is_empty()).then(|| format!("### {heading}\n{content}"))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn private_markdown(data: &Value) -> String {
    // 條目維持單換行緊湊排列；備用開場白各成一段，段間空行
    let entry_block = data
        .get("character_book")
        .and_then(|book| book.get("entries"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let content = string_field(entry, "content")?;
            if content.trim().is_empty() {
                return None;
            }
            let keys = entry
                .get("keys")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("、");
            Some(format!("- **{keys}**：{content}"))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut sections = if entry_block.is_empty() {
        Vec::new()
    } else {
        vec![entry_block]
    };
    sections.extend(
        data.get("alternate_greetings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|greeting| !greeting.is_empty())
            .enumerate()
            .map(|(index, greeting)| format!("### 備用開場白 {}\n{greeting}", index + 1)),
    );
    sections.join("\n\n")
}

/// 世界書匯入的前處理：PNG 卡先解出內嵌 JSON；整包若是角色卡（社群發佈的世界書卡），
/// 剝到 character_book 那層再交給 data::import_worldbook；卡上沒有條目時改走人設欄轉換
pub fn worldbook_json(bytes: &[u8]) -> DataResult<String> {
    let json_bytes = if bytes.starts_with(PNG_MAGIC) {
        decode_png_character(bytes)?
    } else {
        bytes.to_vec()
    };
    let value: Value = serde_json::from_slice(&json_bytes)
        .map_err(|error| data::invalid_data(format!("世界書 JSON 無法解析：{error}")))?;
    let card_data = value
        .get("data")
        .filter(|data| data.is_object())
        .unwrap_or(&value);
    let has_entries = |book: &Value| {
        book.get("entries")
            .and_then(Value::as_array)
            .is_some_and(|entries| !entries.is_empty())
    };
    if let Some(book) = card_data.get("character_book").filter(|b| has_entries(b)) {
        return Ok(book.to_string());
    }
    // 頂層就是世界書本體（V2 獨立書 JSON，entries 是物件不是陣列）
    if card_data.get("entries").is_some() {
        return Ok(card_data.to_string());
    }
    persona_as_worldbook(card_data)
        .map(|book| book.to_string())
        .ok_or_else(|| data::invalid_data("這張卡沒有世界書條目，人設欄也是空的"))
}

/// 世界書內容被作者寫在人設欄、`character_book` 卻是空的那種卡（實例：furry-male-scenarios，
/// 1873 字全在 description）：把非空人設欄合成一條沒有關鍵字的常駐條目。
/// 開場白不收——那條走 card_openings 讓玩家挑，收進來會每回合重複注入。
fn persona_as_worldbook(card_data: &Value) -> Option<Value> {
    let content = PERSONA_FIELDS
        .iter()
        .filter_map(|field| {
            let text = string_field(card_data, field)?.trim();
            (!text.is_empty()).then_some(text)
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    if content.is_empty() {
        return None;
    }
    let name = string_field(card_data, "name").unwrap_or_default().trim();
    Some(json!({
        "name": name,
        "entries": [{
            "id": 0,
            "keys": [],
            "secondary_keys": [],
            "comment": name,
            "content": content,
            "constant": true,
            "selective": false,
            "insertion_order": 0,
            "enabled": true,
            "position": "before_char",
            "case_sensitive": false,
            "extensions": {},
        }],
        "extensions": {},
    }))
}

fn decode_png_character(bytes: &[u8]) -> DataResult<Vec<u8>> {
    let mut offset = PNG_MAGIC.len();
    while offset < bytes.len() {
        if bytes.len() - offset < 12 {
            return Err(data::invalid_data("PNG chunk 格式不完整"));
        }
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let chunk_end = offset
            .checked_add(12)
            .and_then(|end| end.checked_add(length))
            .ok_or_else(|| data::invalid_data("PNG chunk 長度無效"))?;
        if chunk_end > bytes.len() {
            return Err(data::invalid_data("PNG chunk 長度超出檔案範圍"));
        }
        let kind = &bytes[offset + 4..offset + 8];
        let chunk_data = &bytes[offset + 8..offset + 8 + length];
        if kind == b"tEXt" {
            if let Some(separator) = chunk_data.iter().position(|byte| *byte == 0) {
                if &chunk_data[..separator] == b"chara" {
                    return decode_base64(&chunk_data[separator + 1..]);
                }
            }
        }
        offset = chunk_end;
    }
    Err(data::invalid_data("PNG 找不到 chara tEXt chunk"))
}

fn decode_base64(input: &[u8]) -> DataResult<Vec<u8>> {
    if input.len() % 4 != 0 {
        return Err(data::invalid_data("chara base64 長度無效"));
    }
    let mut output = Vec::with_capacity(input.len() / 4 * 3);
    for group in input.chunks_exact(4) {
        let padding = group.iter().filter(|byte| **byte == b'=').count();
        if padding > 2
            || (padding > 0
                && (group[..4 - padding].iter().any(|byte| *byte == b'=')
                    || group[4 - padding..4].iter().any(|byte| *byte != b'=')))
        {
            return Err(data::invalid_data("chara base64 padding 無效"));
        }
        if padding > 0 && group != &input[input.len() - 4..] {
            return Err(data::invalid_data("chara base64 padding 位置無效"));
        }
        let mut values = [0u8; 4];
        for (index, byte) in group.iter().enumerate() {
            values[index] = if *byte == b'=' {
                0
            } else {
                base64_value(*byte).ok_or_else(|| data::invalid_data("chara base64 含非法字元"))?
            };
        }
        output.push((values[0] << 2) | (values[1] >> 4));
        if padding < 2 {
            output.push((values[1] << 4) | (values[2] >> 2));
        }
        if padding == 0 {
            output.push((values[2] << 6) | values[3]);
        }
    }
    Ok(output)
}

fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for group in bytes.chunks(3) {
        let a = group[0];
        let b = *group.get(1).unwrap_or(&0);
        let c = *group.get(2).unwrap_or(&0);
        output.push(ALPHABET[(a >> 2) as usize] as char);
        output.push(ALPHABET[(((a & 0x03) << 4) | (b >> 4)) as usize] as char);
        output.push(if group.len() > 1 {
            ALPHABET[(((b & 0x0f) << 2) | (c >> 6)) as usize] as char
        } else {
            '='
        });
        output.push(if group.len() > 2 {
            ALPHABET[(c & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    output
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(label: &str) -> Self {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "table-tavern-import-{label}-{}-{id}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn minimal_png(chara_json: &str) -> Vec<u8> {
        let mut png = PNG_MAGIC.to_vec();
        let text = format!("chara\0{}", base64_encode(chara_json.as_bytes()));
        png.extend_from_slice(&(text.len() as u32).to_be_bytes());
        png.extend_from_slice(b"tEXt");
        png.extend_from_slice(text.as_bytes());
        png.extend_from_slice(&[0; 4]);
        png
    }

    #[test]
    fn imports_v2_json_and_preserves_original() {
        let root = TestRoot::new("v2");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let raw = r#"{"spec":"chara_card_v2","spec_version":"2.0","data":{"name":"莉亞","description":"精靈遊俠","personality":"冷靜","scenario":"雨夜","first_mes":"妳來了。","mes_example":"<START>","character_book":{"entries":[{"keys":["森林","月亮"],"content":"古老盟約","enabled":true},{"keys":["略過"],"content":""}]}}}"#.as_bytes();

        let meta = import_character(root.path(), &world_id, raw, "#3366ff").unwrap();
        assert_eq!(meta.name, "莉亞");
        let markdown = fs::read_to_string(
            root.path()
                .join(format!("worlds/{world_id}/characters/{}.md", meta.id)),
        )
        .unwrap();
        assert!(markdown.contains(&format!(
            "---\nid: {}\nname: 莉亞\ncolor: #3366ff\navatar: 🎭\ntier: balanced\nshow_image: true\narchived: false\ndisplay_index: 0\ngen_prompt: \n---",
            meta.id
        )));
        for section in [
            "### 簡介\n精靈遊俠",
            "### 人格與語氣\n冷靜",
            "### 場景\n雨夜",
            "### 開場白\n妳來了。",
            "### 語氣範例\n<START>",
            "## 私有\n- **森林、月亮**：古老盟約",
        ] {
            assert!(markdown.contains(section), "missing {section}");
        }
        assert_eq!(
            fs::read(root.path().join(format!(
                "worlds/{world_id}/characters/{}.import.json",
                meta.id
            )))
            .unwrap(),
            raw
        );
    }

    /// [initvar] 初始樹要保留巢狀容器、引號內值與玩家巨集字面。
    #[test]
    fn initvar_entry_becomes_initial_tree() {
        let root = TestRoot::new("initvar-tree");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let raw = json!({
            "data": {
                "name": "莉亞",
                "character_book": {
                    "entries": [{
                        "comment": "[initvar]变量初始化勿开",
                        "enabled": false,
                        "content": "World:\n  Time: \"清晨\"\n  Invasion: 1\nPlayer:\n  Name: \"{{user}}\"\n  HP: \"10/10\"\n  Inventory: {}\nHeroes:\n  亞瑟:\n    Outfit:\n      上装: \"白袍\""
                    }]
                }
            }
        })
        .to_string();

        import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff").unwrap();
        let state = data::read_state(root.path(), &world_id).unwrap();
        let world = match state.state.tree.get("World") {
            Some(StateNode::Branch(children)) => children,
            _ => panic!("缺少 World 分支"),
        };
        let player = match state.state.tree.get("Player") {
            Some(StateNode::Branch(children)) => children,
            _ => panic!("缺少 Player 分支"),
        };
        let heroes = match state.state.tree.get("Heroes") {
            Some(StateNode::Branch(children)) => children,
            _ => panic!("缺少 Heroes 分支"),
        };
        let arthur = match heroes.get("亞瑟") {
            Some(StateNode::Branch(children)) => children,
            _ => panic!("缺少亞瑟分支"),
        };
        let outfit = match arthur.get("Outfit") {
            Some(StateNode::Branch(children)) => children,
            _ => panic!("缺少 Outfit 分支"),
        };
        assert_eq!(world.get("Time"), Some(&StateNode::Leaf("清晨".to_owned())));
        assert_eq!(
            player.get("Name"),
            Some(&StateNode::Leaf("{{user}}".to_owned()))
        );
        assert_eq!(
            player.get("Inventory"),
            Some(&StateNode::Branch(BTreeMap::new()))
        );
        assert_eq!(
            outfit.get("上装"),
            Some(&StateNode::Leaf("白袍".to_owned()))
        );
    }

    /// 重複匯入初始樹只能補空缺，已經演進的狀態不可倒回初始值。
    #[test]
    fn initvar_fills_gaps_without_overwriting_existing_values() {
        let root = TestRoot::new("initvar-merge");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let mut state = data::read_state(root.path(), &world_id).unwrap();
        state.state.tree.insert(
            "World".to_owned(),
            StateNode::Branch(BTreeMap::from([(
                "Time".to_owned(),
                StateNode::Leaf("深夜".to_owned()),
            )])),
        );
        data::write_state(root.path(), &world_id, &state).unwrap();
        let raw = json!({
            "data": {
                "name": "莉亞",
                "character_book": {
                    "entries": [{
                        "comment": "[initvar] 初始值",
                        "disable": true,
                        "content": "World:\n  Time: 清晨\n  Invasion: 1\nPlayer:\n  HP: 10/10"
                    }]
                }
            }
        })
        .to_string();

        import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff").unwrap();
        let state = data::read_state(root.path(), &world_id).unwrap();
        let world = match state.state.tree.get("World") {
            Some(StateNode::Branch(children)) => children,
            _ => panic!("缺少 World 分支"),
        };
        assert_eq!(world.get("Time"), Some(&StateNode::Leaf("深夜".to_owned())));
        assert_eq!(
            world.get("Invasion"),
            Some(&StateNode::Leaf("1".to_owned()))
        );
        assert_eq!(
            state.state.tree.get("Player"),
            Some(&StateNode::Branch(BTreeMap::from([(
                "HP".to_owned(),
                StateNode::Leaf("10/10".to_owned()),
            )])))
        );
    }

    /// 壞掉的 [initvar] 內容不可阻斷角色與世界書條目的正常匯入。
    #[test]
    fn broken_initvar_never_blocks_import() {
        let root = TestRoot::new("initvar-broken");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let raw = json!({
            "data": {
                "name": "莉亞",
                "character_book": {
                    "entries": [{
                        "comment": "[initvar] 初始值",
                        "enabled": false,
                        "keys": ["初始"],
                        "content": "這不是 YAML"
                    }]
                }
            }
        })
        .to_string();

        let meta = import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff").unwrap();
        assert!(data::read_character(root.path(), &world_id, &meta.id)
            .unwrap()
            .private_md
            .contains("- **初始**：這不是 YAML"));
        assert!(data::read_state(root.path(), &world_id)
            .unwrap()
            .state
            .tree
            .is_empty());
    }

    /// 啟用中的同名條目不是 MVU 初始狀態樹，必須完全略過。
    #[test]
    fn enabled_initvar_entry_is_ignored() {
        let root = TestRoot::new("initvar-enabled");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let raw = json!({
            "data": {
                "name": "莉亞",
                "character_book": {
                    "entries": [{
                        "comment": "[initvar] 初始值",
                        "enabled": true,
                        "content": "World:\n  Time: 清晨"
                    }]
                }
            }
        })
        .to_string();

        import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff").unwrap();
        assert!(data::read_state(root.path(), &world_id)
            .unwrap()
            .state
            .tree
            .is_empty());
    }

    /// 真實形狀的 `[mvu_update]` 規則表：type／range／format 三個屬性各種組合都要抽對，
    /// 抽不出數字上下限的路徑（非數字 range）不填 min/max，匯入後這桌要標成增量桌。
    #[test]
    fn mvu_update_entry_extracts_field_rules_and_marks_incremental_table() {
        let root = TestRoot::new("mvu-rules");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let content = "变量更新规则:\n\
             \x20 World:\n\
             \x20   Invasion:\n\
             \x20     type: number\n\
             \x20     range: 0-100\n\
             \x20     check:\n\
             \x20       - 初始为1% (魔王降临前兆)。\n\
             \x20   Roll100:\n\
             \x20     type: number\n\
             \x20     range: 1-100\n\
             \x20     check:\n\
             \x20       - 每次剧情输出开始时输出的第一个随机数\n\
             \x20 Player:\n\
             \x20   Level:\n\
             \x20     type: string\n\
             \x20     range: 零阶-六阶\n\
             \x20   HP/SP/MP:\n\
             \x20     type: string\n\
             \x20     format: \"当前值/上限值\"\n\
             \x20 Heroes:\n\
             \x20   '${勇者姓名}':\n\
             \x20     Affection:\n\
             \x20       type: number\n\
             \x20       range: 0-200\n";
        let raw = json!({
            "data": {
                "name": "莉亞",
                "character_book": {
                    "entries": [{
                        "comment": "[mvu_update]变量更新规则",
                        "enabled": true,
                        "content": content
                    }]
                }
            }
        })
        .to_string();

        import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff").unwrap();
        let state = data::read_state(root.path(), &world_id).unwrap();
        assert!(state.mechanism.incremental);
        let rules = &state.mechanism.rules;

        let invasion = &rules["World.Invasion"];
        assert_eq!(invasion.kind, data::FieldKind::Number);
        assert_eq!(invasion.min, Some(0.0));
        assert_eq!(invasion.max, Some(100.0));

        assert_eq!(rules["World.Roll100"].kind, data::FieldKind::Roll);

        for field in ["HP", "SP", "MP"] {
            assert_eq!(
                rules[&format!("Player.{field}")].kind,
                data::FieldKind::Pair
            );
        }

        let level = &rules["Player.Level"];
        assert_eq!(level.kind, data::FieldKind::Text);
        assert_eq!(level.min, None);
        assert_eq!(level.max, None);

        let affection = &rules["Heroes.*.Affection"];
        assert_eq!(affection.kind, data::FieldKind::Number);
        assert_eq!(affection.min, Some(0.0));
        assert_eq!(affection.max, Some(200.0));
    }

    /// EJS 只收可靜態還原的觸發表；認不出的原文仍停用並留 skipped 記帳。
    #[test]
    fn ejs_entries_become_triggers_and_log_unrecognized_scripts() {
        let root = TestRoot::new("ejs-triggers");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let supported = r#"<%_
var invasion = getvar('stat_data.World.Invasion', { defaults: 0 });
var done = getvar('stat_data.Events.鐘聲', { defaults: false });
if (invasion >= 50 && done === false) { _%>
鐘聲響起 <%= invasion %><%_ setvar('stat_data.Events.鐘聲', true); } _%>"#;
        let unsupported =
            r#"<%_ var roll = _.random(1, 2); _%><%_ if (roll === 1) { _%>隨機<%_ } _%>"#;
        let raw = json!({
            "data": {
                "name": "莉亞",
                "character_book": {
                    "entries": [
                        {"comment": "[Event] 鐘聲", "content": supported},
                        {"comment": "[Script] 隨機", "content": unsupported}
                    ]
                }
            }
        })
        .to_string();

        import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff").unwrap();
        let state = data::read_state(root.path(), &world_id).unwrap();
        assert_eq!(state.mechanism.triggers.len(), 1);
        assert_eq!(
            state.mechanism.triggers[0].flag.as_deref(),
            Some("Events.鐘聲")
        );
        assert_eq!(
            state.mechanism.rules["Events.鐘聲"].kind,
            data::FieldKind::ReadOnly
        );
        assert!(!state.mechanism.incremental);
        let log =
            fs::read_to_string(data::mechanism_log_path(root.path(), &world_id).unwrap()).unwrap();
        assert_eq!(log.matches("\"kind\":\"skipped\"").count(), 1);
    }

    /// ST 的 `${a|b}` 多選一寫法：同一條規則要展開成各自獨立的規則。
    #[test]
    fn wildcard_pipe_segment_expands_into_multiple_rules() {
        let root = TestRoot::new("mvu-expand");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let raw = json!({
            "data": {
                "name": "莉亞",
                "character_book": {
                    "entries": [{
                        "comment": "[mvu_update]规则",
                        "enabled": true,
                        "content": "规则:\n  Player:\n    Outfit.${上装|下装}:\n      type: string\n"
                    }]
                }
            }
        })
        .to_string();

        import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff").unwrap();
        let state = data::read_state(root.path(), &world_id).unwrap();
        assert_eq!(
            state.mechanism.rules["Player.Outfit.上装"].kind,
            data::FieldKind::Text
        );
        assert_eq!(
            state.mechanism.rules["Player.Outfit.下装"].kind,
            data::FieldKind::Text
        );
    }

    #[test]
    fn imports_png_text_chunk_and_preserves_original() {
        let root = TestRoot::new("png");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let png = minimal_png(r#"{"data":{"name":"凱恩","description":"騎士"}}"#);

        let meta = import_character(root.path(), &world_id, &png, "#111111").unwrap();
        assert!(root
            .path()
            .join(format!("worlds/{world_id}/characters/{}.md", meta.id))
            .is_file());
        assert_eq!(
            fs::read(
                root.path()
                    .join(format!("worlds/{world_id}/characters/{}.png", meta.id))
            )
            .unwrap(),
            png
        );
        assert_eq!(
            data::list_characters(root.path(), &world_id).unwrap().len(),
            1
        );
    }

    #[test]
    fn imports_v1_top_level_fields() {
        let root = TestRoot::new("v1");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let meta = import_character(
            root.path(),
            &world_id,
            r#"{"name":"舊卡","personality":"直率"}"#.as_bytes(),
            "#222222",
        )
        .unwrap();
        let card = data::read_character(root.path(), &world_id, &meta.id).unwrap();
        assert_eq!(card.public_md, "### 人格與語氣\n直率");
    }

    #[test]
    fn probe_ignores_invalid_bytes() {
        assert_eq!(probe_import(b"not a card"), ImportProbe::default());
    }

    #[test]
    fn probe_identifies_lorebook_heavy_cards() {
        let entries = json!([{"content":"一"}, {"content":"二"}, {"content":"三"}]);
        let sparse = probe_import(
            json!({"data":{"character_book":{"entries":entries},"description":" ","personality":"","scenario":""}})
                .to_string()
                .as_bytes(),
        );
        assert!(sparse.lorebook_heavy);

        // 西幻真卡的比例：人設 988 字、世界書 21,678 字。人設不算短，重心仍壓倒性在世界書
        let simulator = probe_import(
            json!({"data":{
                "character_book":{"entries":[
                    {"content":"世".repeat(7000)},
                    {"content":"界".repeat(7000)},
                    {"content":"書".repeat(7678)},
                ]},
                "description":"長".repeat(988),
            }})
            .to_string()
            .as_bytes(),
        );
        assert!(simulator.lorebook_heavy);

        // 一般角色卡：帶著自己的隨身設定，但重心還在人設上
        let character = probe_import(
            json!({"data":{
                "character_book":{"entries":[
                    {"content":"故鄉".repeat(200)},
                    {"content":"家人".repeat(200)},
                    {"content":"秘密".repeat(200)},
                ]},
                "description":"人".repeat(2000),
                "mes_example":"例".repeat(1000),
            }})
            .to_string()
            .as_bytes(),
        );
        assert!(!character.lorebook_heavy);

        let detailed = probe_import(
            json!({"data":{"character_book":{"entries":[{}, {}, {}]},"description":"長".repeat(300)}})
                .to_string()
                .as_bytes(),
        );
        assert!(!detailed.lorebook_heavy);
    }

    /// 前端匯入分流靠 parsed／name／book_entries 三個欄位判斷純世界書、純角色卡、
    /// 兩種身分都有料、格式錯誤這四種情況，四種都要顧到。
    #[test]
    fn probe_reports_parsed_state_and_book_entry_count() {
        // 純世界書 JSON：頂層就是書本體（entries 在最外層），沒有 name 欄位
        let worldbook = probe_import(
            json!({"entries": [{"keys": ["森林"], "content": "古老盟約"}]})
                .to_string()
                .as_bytes(),
        );
        assert!(worldbook.parsed);
        assert_eq!(worldbook.name, None);
        assert!(worldbook.book_shaped);

        // V2 獨立世界書 JSON：頂層是書本體，自帶 name（書名）＋entries——有 name 也是世界書
        let named_book = probe_import(
            json!({"name": "北境設定集", "entries": [{"keys": ["漁村"], "content": "北境的漁村"}]})
                .to_string()
                .as_bytes(),
        );
        assert!(named_book.parsed);
        assert_eq!(named_book.name.as_deref(), Some("北境設定集"));
        assert!(named_book.book_shaped);

        // 損毀 JSON：解析不了，parsed 維持 false（沿用原本走角色路徑報格式錯誤那條）
        let broken = probe_import(b"{not json");
        assert!(!broken.parsed);

        // 有 name 但沒有 character_book：純角色卡
        let character_only = probe_import(
            json!({"data": {"name": "莉亞", "description": "精靈遊俠"}})
                .to_string()
                .as_bytes(),
        );
        assert!(character_only.parsed);
        assert_eq!(character_only.name.as_deref(), Some("莉亞"));
        assert_eq!(character_only.book_entries, 0);
        assert!(!character_only.book_shaped);
        assert_eq!(character_only.alternate_greetings, 0);

        // 情境卡：零條目、人設全塞在 description，靠備用開場白數認出來（實例 furry-male-scenarios）
        let scenarios = probe_import(
            json!({"data": {
                "name": "Furry male Scenarios",
                "description": "{{char}} is not a person but a scenario",
                "first_mes": "開場",
                "alternate_greetings": ["公車站", "海灘", "廚房"],
            }})
            .to_string()
            .as_bytes(),
        );
        assert_eq!(scenarios.book_entries, 0);
        assert!(!scenarios.lorebook_heavy);
        assert_eq!(scenarios.alternate_greetings, 3);

        // 有 name 也帶 character_book：角色與世界書兩種身分都有料
        let both = probe_import(
            json!({"data": {
                "name": "薇拉",
                "character_book": {"entries": [{"content": "北境的漁村"}, {"content": "雙親早逝"}]},
            }})
            .to_string()
            .as_bytes(),
        );
        assert!(both.parsed);
        assert_eq!(both.book_entries, 2);
    }

    #[test]
    fn character_card_brings_its_own_lorebook_entries_to_the_table() {
        let card = r#"{"data":{"name":"薇拉","description":"北境來的斥候","character_book":{"entries":[{"keys":["故鄉"],"content":"北境的漁村","comment":"故鄉"},{"keys":["家人"],"content":"雙親早逝","comment":"家人"}]}}}"#;
        let root = TestRoot::new("character-lorebook");
        let world_id = data::create_world(root.path(), "酒館").unwrap();

        import_character(root.path(), &world_id, card.as_bytes(), "#ffffff").unwrap();

        let entries = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|entry| entry.content == "北境的漁村"));

        // 同一張卡再匯一次：條目由去重擋下，不會長出第二份
        import_character(root.path(), &world_id, card.as_bytes(), "#ffffff").unwrap();
        assert_eq!(
            data::read_worldbook(root.path(), &world_id).unwrap().len(),
            2
        );
    }

    #[test]
    fn imports_alternate_greetings_into_private_markdown() {
        let root = TestRoot::new("alternate-greetings");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let raw = r#"{"data":{"name":"莉亞","character_book":{"entries":[{"keys":["森林"],"content":"古老盟約"}]},"alternate_greetings":["第二次見面。","雨天再訪。"]}}"#;

        let meta = import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff").unwrap();
        let private_md = data::read_character(root.path(), &world_id, &meta.id)
            .unwrap()
            .private_md;
        for expected in [
            "- **森林**：古老盟約",
            "### 備用開場白 1",
            "第二次見面。",
            "### 備用開場白 2",
            "雨天再訪。",
        ] {
            assert!(private_md.contains(expected), "missing {expected}");
        }
    }

    /// 測試清單 #12：匯入 ST 角色卡產生新 id、name 照原值（含原本會被擋的字元）；
    /// 同名再匯入一次也會成功，各自拿到不同 id 且互不影響
    #[test]
    fn importing_the_same_name_twice_mints_distinct_ids_and_keeps_first_card_intact() {
        let root = TestRoot::new("duplicate-name");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let odd_name = r#"{"name":"a/b/../重名","description":"第一張"}"#;
        let first =
            import_character(root.path(), &world_id, odd_name.as_bytes(), "#000000").unwrap();
        assert_eq!(first.name, "a/b/../重名");

        let second = import_character(
            root.path(),
            &world_id,
            r#"{"name":"a/b/../重名","description":"第二張"}"#.as_bytes(),
            "#ffffff",
        )
        .unwrap();

        assert_ne!(first.id, second.id);
        assert_eq!(
            data::read_character(root.path(), &world_id, &first.id)
                .unwrap()
                .public_md,
            "### 簡介\n第一張"
        );
        assert_eq!(
            data::read_character(root.path(), &world_id, &second.id)
                .unwrap()
                .public_md,
            "### 簡介\n第二張"
        );
        assert_eq!(
            data::list_characters(root.path(), &world_id).unwrap().len(),
            2
        );
    }

    /// 匯出→再匯入要拿回一模一樣的內容：公開五段照原欄位歸位，私有條目回到 character_book
    #[test]
    fn exported_png_reimports_with_identical_content() {
        let root = TestRoot::new("export-png");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let raw = r#"{"data":{"name":"莉亞","description":"精靈遊俠","personality":"冷靜","scenario":"雨夜","first_mes":"妳來了。","mes_example":"<START>","character_book":{"entries":[{"keys":["森林","月亮"],"content":"古老盟約"}]}}}"#;
        let source = import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff").unwrap();
        let target = root.path().join("莉亞.png");

        export_character(root.path(), &world_id, &source.id, &target).unwrap();

        let exported = fs::read(&target).unwrap();
        let value: Value =
            serde_json::from_slice(&decode_png_character(&exported).unwrap()).unwrap();
        assert_eq!(value["spec"], "chara_card_v2");
        assert_eq!(value["name"], "莉亞");
        assert_eq!(value["data"]["personality"], "冷靜");
        assert_eq!(
            value["data"]["character_book"]["entries"][0]["keys"][1],
            "月亮"
        );
        assert_eq!(
            value["data"]["character_book"]["entries"][0]["content"],
            "古老盟約"
        );

        let round_trip = import_character(root.path(), &world_id, &exported, "#000000").unwrap();
        assert_eq!(
            data::read_character(root.path(), &world_id, &round_trip.id)
                .unwrap()
                .public_md,
            data::read_character(root.path(), &world_id, &source.id)
                .unwrap()
                .public_md
        );
        assert_eq!(
            data::read_character(root.path(), &world_id, &round_trip.id)
                .unwrap()
                .private_md,
            "- **森林、月亮**：古老盟約"
        );
    }

    #[test]
    fn character_export_import_round_trips_rules_and_initial_tree() {
        let root = TestRoot::new("mechanism-round-trip");
        let source_world = data::create_world(root.path(), "來源桌").unwrap();
        let source = import_character(
            root.path(),
            &source_world,
            r#"{"data":{"name":"亞瑟","description":"騎士"}}"#.as_bytes(),
            "#3366ff",
        )
        .unwrap();
        let mut state = data::read_state(root.path(), &source_world).unwrap();
        let mut rule = data::FieldRule::for_kind(data::FieldKind::Pair);
        rule.branch = Some("亞瑟".to_owned());
        state
            .mechanism
            .rules
            .insert("亞瑟.能力.HP".to_owned(), rule);
        let initial = StateNode::Branch(BTreeMap::from([(
            "能力".to_owned(),
            StateNode::Branch(BTreeMap::from([(
                "HP".to_owned(),
                StateNode::Leaf("50/50".to_owned()),
            )])),
        )]));
        state.state.tree.insert("亞瑟".to_owned(), initial.clone());
        data::write_state(root.path(), &source_world, &state).unwrap();

        let export_path = root.path().join("亞瑟.json");
        export_character(root.path(), &source_world, &source.id, &export_path).unwrap();
        let exported = fs::read(&export_path).unwrap();
        let target_world = data::create_world(root.path(), "目標桌").unwrap();
        import_character(root.path(), &target_world, &exported, "#000000").unwrap();
        let target = data::read_state(root.path(), &target_world).unwrap();
        assert_eq!(
            target.mechanism.rules["亞瑟.能力.HP"].branch.as_deref(),
            Some("亞瑟")
        );
        assert_eq!(target.state.tree["亞瑟"], initial);

        // 同一份匯出檔改用世界書身分匯入，機制照樣要收進來（兩條路徑對等）
        let book_world = data::create_world(root.path(), "世界書桌").unwrap();
        import_card_extension(root.path(), &book_world, "亞瑟", &exported);
        let book_state = data::read_state(root.path(), &book_world).unwrap();
        assert_eq!(
            book_state.mechanism.rules["亞瑟.能力.HP"].branch.as_deref(),
            Some("亞瑟")
        );
        assert_eq!(book_state.state.tree["亞瑟"], initial);
    }

    /// App 內手寫的卡沒有那五個標題，全部併進 description；私有筆記進常駐條目
    #[test]
    fn exports_freeform_card_as_json() {
        let root = TestRoot::new("export-json");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let id = data::new_id();
        data::write_character(
            root.path(),
            &world_id,
            &CharacterCard {
                id: id.clone(),
                name: "凱恩".to_owned(),
                color: "#111111".to_owned(),
                avatar: "🎭".to_owned(),
                tier: Tier::Balanced,
                show_image: true,
                archived: false,
                gen_prompt: String::new(),
                public_md: "騎士，話少。\n### 開場白\n有事？".to_owned(),
                private_md: "其實是逃兵".to_owned(),
            },
        )
        .unwrap();
        let target = root.path().join("凱恩.json");

        export_character(root.path(), &world_id, &id, &target).unwrap();

        let value: Value = serde_json::from_slice(&fs::read(&target).unwrap()).unwrap();
        assert_eq!(value["data"]["description"], "騎士，話少。");
        assert_eq!(value["data"]["first_mes"], "有事？");
        let entry = &value["data"]["character_book"]["entries"][0];
        assert_eq!(entry["content"], "其實是逃兵");
        assert_eq!(entry["constant"], true);
    }

    /// 沒有圖的卡也匯得出 PNG：底圖是自己組的 1×1，chunk 長度與 CRC 都要對
    #[test]
    fn blank_png_is_a_valid_png_container() {
        let png = blank_png();
        assert!(png.starts_with(PNG_MAGIC));
        let mut offset = PNG_MAGIC.len();
        let mut kinds = Vec::new();
        while offset < png.len() {
            let length = u32::from_be_bytes(png[offset..offset + 4].try_into().unwrap()) as usize;
            let crc_at = offset + 8 + length;
            assert_eq!(
                u32::from_be_bytes(png[crc_at..crc_at + 4].try_into().unwrap()),
                crc32(&png[offset + 4..crc_at])
            );
            kinds.push(String::from_utf8_lossy(&png[offset + 4..offset + 8]).into_owned());
            offset = crc_at + 4;
        }
        assert_eq!(kinds, ["IHDR", "IDAT", "IEND"]);
        assert_eq!(offset, png.len());
    }

    /// 底圖若還留著匯入當下那份 chara，匯出後必須被換掉而不是疊上去
    #[test]
    fn export_replaces_stale_chara_chunk_in_the_image() {
        let root = TestRoot::new("export-stale");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let mut base = blank_png();
        base.truncate(base.len() - 12); // 拆掉 IEND，插入舊 chara 後再補回
        let stale = format!(
            "chara\0{}",
            base64_encode(r#"{"data":{"name":"舊名"}}"#.as_bytes())
        );
        base.extend_from_slice(&png_chunk(b"tEXt", stale.as_bytes()));
        base.extend_from_slice(&png_chunk(b"IEND", &[]));
        let meta = import_character(root.path(), &world_id, &base, "#111111").unwrap();
        let mut card = data::read_character(root.path(), &world_id, &meta.id).unwrap();
        card.name = "新名".to_owned();
        data::write_character(root.path(), &world_id, &card).unwrap();
        let target = root.path().join("新名.png");

        export_character(root.path(), &world_id, &meta.id, &target).unwrap();

        let exported = fs::read(&target).unwrap();
        let value: Value =
            serde_json::from_slice(&decode_png_character(&exported).unwrap()).unwrap();
        assert_eq!(value["name"], "新名");
        assert_eq!(
            exported
                .windows(6)
                .filter(|window| *window == b"chara\0")
                .count(),
            1
        );
    }

    #[test]
    fn decodes_base64_and_rejects_invalid_input() {
        assert_eq!(decode_base64(b"SGVsbG8=").unwrap(), b"Hello");
        assert!(decode_base64(b"SGVsbG8!").is_err());
        assert!(decode_base64(b"AA=A").is_err());
    }

    #[test]
    fn character_image_returns_png_base64_or_none() {
        let root = TestRoot::new("image");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let png = minimal_png(r#"{"data":{"name":"凱恩"}}"#);
        let meta = import_character(root.path(), &world_id, &png, "#111111").unwrap();

        let encoded = character_image(root.path(), &world_id, &meta.id)
            .unwrap()
            .unwrap();
        assert_eq!(decode_base64(encoded.as_bytes()).unwrap(), png);
        assert_eq!(
            character_image(root.path(), &world_id, &data::new_id()).unwrap(),
            None
        );
    }

    #[test]
    fn saves_and_reads_character_image_and_avatar() {
        let root = TestRoot::new("save-images");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let png = minimal_png(r#"{"data":{"name":"凱恩"}}"#);
        let meta = import_character(root.path(), &world_id, &png, "#111111").unwrap();
        let image = PNG_MAGIC.iter().copied().chain([1, 2]).collect::<Vec<_>>();
        let avatar = PNG_MAGIC.iter().copied().chain([3, 4]).collect::<Vec<_>>();

        save_character_image(root.path(), &world_id, &meta.id, &image).unwrap();
        save_character_avatar(root.path(), &world_id, &meta.id, &avatar).unwrap();

        assert_eq!(
            decode_base64(
                character_image(root.path(), &world_id, &meta.id)
                    .unwrap()
                    .unwrap()
                    .as_bytes()
            )
            .unwrap(),
            image
        );
        assert_eq!(
            decode_base64(
                character_avatar(root.path(), &world_id, &meta.id)
                    .unwrap()
                    .unwrap()
                    .as_bytes()
            )
            .unwrap(),
            avatar
        );
    }

    #[test]
    fn save_character_images_reject_invalid_png_and_missing_character() {
        let root = TestRoot::new("reject-images");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let missing_id = data::new_id();

        assert_eq!(
            save_character_image(root.path(), &world_id, &missing_id, b"not png")
                .unwrap_err()
                .to_string(),
            "圖片必須是 PNG"
        );
        assert_eq!(
            save_character_avatar(root.path(), &world_id, &missing_id, PNG_MAGIC)
                .unwrap_err()
                .to_string(),
            format!("角色 {missing_id} 不存在")
        );
    }

    #[test]
    fn delete_character_images_is_idempotent() {
        let root = TestRoot::new("delete-images");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let png = minimal_png(r#"{"data":{"name":"凱恩"}}"#);
        let meta = import_character(root.path(), &world_id, &png, "#111111").unwrap();

        delete_character_image(root.path(), &world_id, &meta.id).unwrap();
        delete_character_image(root.path(), &world_id, &meta.id).unwrap();
        delete_character_avatar(root.path(), &world_id, &meta.id).unwrap();
        delete_character_avatar(root.path(), &world_id, &meta.id).unwrap();
    }

    /// 世界書卡不會建成角色，開場白仍須保留給前端選擇。
    #[test]
    fn card_openings_reads_all_greetings_from_import_bytes() {
        let card = r#"{"spec":"chara_card_v3","data":{"name":"兽人的洞穴","first_mes":" 夜色落下。 ","alternate_greetings":["另一個開場。","  ","最後一個開場。"],"character_book":{"entries":[]}}}"#;
        assert_eq!(
            card_openings(card.as_bytes()),
            Some((
                "兽人的洞穴".to_owned(),
                vec![
                    "夜色落下。".to_owned(),
                    "另一個開場。".to_owned(),
                    "最後一個開場。".to_owned(),
                ],
            ))
        );
        // PNG 卡與 JSON 卡必須走同一條解析路徑。
        assert_eq!(
            card_openings(&minimal_png(card)),
            Some((
                "兽人的洞穴".to_owned(),
                vec![
                    "夜色落下。".to_owned(),
                    "另一個開場。".to_owned(),
                    "最後一個開場。".to_owned(),
                ],
            ))
        );
        // 有些卡只把真正開場寫在替代開場白。
        assert_eq!(
            card_openings(
                r#"{"data":{"name":"莉亞","first_mes":"  ","alternate_greetings":["在這裡。"]}}"#
                    .as_bytes(),
            ),
            Some(("莉亞".to_owned(), vec!["在這裡。".to_owned()]))
        );
        // 完全沒有可顯示的開場白時，前端不應開啟選擇面板。
        assert_eq!(
            card_openings(r#"{"data":{"name":"莉亞"}}"#.as_bytes()),
            None
        );
        assert_eq!(card_openings(b"not a card"), None);
    }

    /// 世界書卡（PNG 或 JSON 的假角色卡）要能整包匯進世界書；keys 為 null 的常駐條目不能炸
    #[test]
    fn worldbook_json_unwraps_lorebook_cards() {
        let card = r#"{"spec":"chara_card_v3","spec_version":"3.0","data":{"name":"根源重塑","character_book":{"name":"根源重塑","entries":[{"keys":["森林"],"content":"古老盟約","comment":"盟約","enabled":true,"insertion_order":3},{"keys":null,"constant":true,"content":"世界觀常駐","comment":"世界觀","enabled":true}]}}}"#;

        let root = TestRoot::new("worldbook-card");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        // PNG 卡與純 JSON 卡走同一條路，各匯一次；同一份書第二次全被當重複略過
        let png = minimal_png(card);
        let mut results = Vec::new();
        for bytes in [png.as_slice(), card.as_bytes()] {
            let json = worldbook_json(bytes).unwrap();
            results.push(data::import_worldbook(root.path(), &world_id, &json).unwrap());
        }
        assert_eq!(results[0].imported, 2);
        assert_eq!(
            results[1],
            data::WorldbookImport {
                imported: 0,
                skipped: 2
            }
        );
        let entries = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title, "盟約");
        assert_eq!(entries[0].keys, ["森林"]);
        assert_eq!(entries[0].order, 3);
        assert!(entries[1].constant);
        assert!(entries[1].keys.is_empty());
        assert_eq!(entries[1].content, "世界觀常駐");

        // 不是卡的一般世界書 JSON 原樣通過
        let plain = r#"{"entries":{"0":{"uid":0,"key":["龍"],"content":"沉睡"}}}"#;
        let round_trip: Value =
            serde_json::from_str(&worldbook_json(plain.as_bytes()).unwrap()).unwrap();
        assert_eq!(round_trip, serde_json::from_str::<Value>(plain).unwrap());
    }

    /// 世界書內容寫在人設欄、character_book 空著的卡：轉成一條沒關鍵字的常駐條目才進得來
    #[test]
    fn worldbook_json_converts_persona_fields_when_card_has_no_entries() {
        let card = json!({"spec": "chara_card_v2", "spec_version": "2.0", "data": {
            "name": "Furry male Scenarios",
            "description": "{{char}} is not a person but a scenario",
            "personality": "",
            "scenario": "毛毛大陸",
            "first_mes": "開場白不該進條目",
            "alternate_greetings": ["公車站", "海灘"],
        }})
        .to_string();

        let root = TestRoot::new("persona-as-book");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let json = worldbook_json(card.as_bytes()).unwrap();
        assert_eq!(
            data::import_worldbook(root.path(), &world_id, &json).unwrap(),
            data::WorldbookImport {
                imported: 1,
                skipped: 0
            }
        );
        let entries = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Furry male Scenarios");
        assert!(entries[0].constant);
        assert!(entries[0].keys.is_empty());
        // 非空人設欄照 PERSONA_FIELDS 順序併起來，開場白留給 card_openings
        assert_eq!(
            entries[0].content,
            "{{char}} is not a person but a scenario\n\n毛毛大陸"
        );
        assert!(!entries[0].content.contains("開場白不該進條目"));

        // character_book 在但條目是空陣列：一樣走轉換，不會匯進一本空書
        let empty_book = json!({"data": {
            "name": "空書卡", "description": "設定都在這裡", "character_book": {"entries": []},
        }})
        .to_string();
        let converted: Value =
            serde_json::from_str(&worldbook_json(empty_book.as_bytes()).unwrap()).unwrap();
        assert_eq!(converted["entries"][0]["content"], "設定都在這裡");

        // 人設欄全空＝真的沒東西可匯，明講而不是靜默塞一本空書
        let hollow = json!({"data": {"name": "空殼", "first_mes": "只有開場白"}}).to_string();
        assert!(worldbook_json(hollow.as_bytes()).is_err());
    }

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
