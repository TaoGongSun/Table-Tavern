use super::card_io::{decode_png_character, string_field, PNG_MAGIC};
use super::mechanism::{
    import_mechanism, import_mechanism_strict, import_table_tavern_extension,
    import_table_tavern_extension_strict,
};
use super::png_clean::{self, Stored};
use super::png_image::{validate_png_image, STORED_IMAGE_LIMITS};
use crate::data::{self, CharacterCard, CharacterMeta, DataResult, Tier};
use crate::ui_msg::UiMsg;
use serde_json::{json, Value};
use std::path::Path;

/// 人設欄：既是 lorebook_heavy 秤重時的「人設份量」，也是沒有條目的卡轉成世界書時要收的內容。
/// 不含 first_mes——開場白走 card_openings 讓玩家挑，收進條目會每回合重複注入。
const PERSONA_FIELDS: [&str; 4] = ["description", "personality", "scenario", "mes_example"];

/// 段標的語系順序（與 `SECTION_LABELS` 各列對齊）。
const LANGS: [&str; 10] = [
    "zh-TW", "zh-CN", "en", "ja", "ko", "es", "pt-BR", "de", "fr", "ru",
];

/// 公開段落與 SillyTavern 欄位的對照表：匯入時照介面語系拆成 `### 段標`（之後就是玩家內文），
/// 匯出時認得十語系任一版本的段標再併回欄位。
pub(super) const PUBLIC_SECTIONS: [(&str, [&str; 10]); 5] = [
    (
        "description",
        [
            "簡介",
            "简介",
            "Description",
            "紹介",
            "소개",
            "Descripción",
            "Descrição",
            "Beschreibung",
            "Description",
            "Описание",
        ],
    ),
    (
        "personality",
        [
            "人格與語氣",
            "人格与语气",
            "Personality",
            "性格と口調",
            "성격과 말투",
            "Personalidad y tono",
            "Personalidade e tom",
            "Persönlichkeit und Tonfall",
            "Personnalité et ton",
            "Характер и манера речи",
        ],
    ),
    (
        "scenario",
        [
            "場景",
            "场景",
            "Scenario",
            "シナリオ",
            "시나리오",
            "Escenario",
            "Cenário",
            "Szenario",
            "Scénario",
            "Сценарий",
        ],
    ),
    (
        "first_mes",
        [
            "開場白",
            "开场白",
            "First message",
            "最初のメッセージ",
            "첫 메시지",
            "Primer mensaje",
            "Primeira mensagem",
            "Erste Nachricht",
            "Premier message",
            "Первое сообщение",
        ],
    ),
    (
        "mes_example",
        [
            "語氣範例",
            "语气范例",
            "Example dialogue",
            "会話例",
            "대화 예시",
            "Ejemplos de diálogo",
            "Exemplos de diálogo",
            "Dialogbeispiele",
            "Exemples de dialogue",
            "Примеры диалога",
        ],
    ),
];

/// 私有段的備用開場白段標（`{n}` 從 1 起）；匯出時照它反解回 alternate_greetings。
pub(super) const ALTERNATE_GREETING: [&str; 10] = [
    "備用開場白 {n}",
    "备用开场白 {n}",
    "Alternate greeting {n}",
    "別の書き出し {n}",
    "대체 첫 메시지 {n}",
    "Saludo alternativo {n}",
    "Saudação alternativa {n}",
    "Alternative Begrüßung {n}",
    "Message d’accueil alternatif {n}",
    "Альтернативное приветствие {n}",
];

/// 這個介面語系在 `LANGS` 裡的位置（zh* 退繁中、未知退英文）。
fn lang_index(lang: &str) -> usize {
    let key = data::lang_key(lang);
    LANGS.iter().position(|item| *item == key).unwrap_or(2)
}

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
        .map(book_entry_values);
    probe.book_entries = book_entries.as_ref().map_or(0, Vec::len);
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

/// 世界書條目一律照這支展開（卡片契約 src/shared/contracts/card-view/card-view.md）：陣列形（V2
/// character_book）照原順序、鍵是索引；物件形（ST 獨立世界書）照 uid 鍵的數字順序，非數字鍵排最後。
/// 不是物件的值（字串、null…）不算條目，直接略過——匯入不會因為一條壞值整本失敗。
pub(crate) fn book_entries_keyed(entries: &Value) -> Vec<(String, &Value)> {
    match entries {
        Value::Array(items) => items
            .iter()
            .enumerate()
            .filter(|(_, value)| value.is_object())
            .map(|(index, value)| (index.to_string(), value))
            .collect(),
        Value::Object(map) => {
            let mut keyed: Vec<(String, &Value)> = map
                .iter()
                .filter(|(_, value)| value.is_object())
                .map(|(key, value)| (key.clone(), value))
                .collect();
            keyed.sort_by(
                |(a, _), (b, _)| match (a.parse::<u64>(), b.parse::<u64>()) {
                    (Ok(x), Ok(y)) => x.cmp(&y),
                    (Ok(_), Err(_)) => std::cmp::Ordering::Less,
                    (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
                    (Err(_), Err(_)) => a.cmp(b),
                },
            );
            keyed
        }
        _ => Vec::new(),
    }
}

pub(crate) fn book_entry_values(entries: &Value) -> Vec<&Value> {
    book_entries_keyed(entries)
        .into_iter()
        .map(|(_, value)| value)
        .collect()
}

/// 角色卡檔解得開、有合法名字：回（卡 JSON、原檔副檔名、名字）。匯入在動任何資料前先過這關。
/// 卡檔解析結果：卡 JSON（PNG 卡是 tEXt 解出來的那份）、解析後的值、卡名。
struct ParsedCard {
    json_bytes: Vec<u8>,
    value: Value,
    name: String,
}

fn parse_character(bytes: &[u8]) -> DataResult<ParsedCard> {
    let json_bytes = if bytes.starts_with(PNG_MAGIC) {
        decode_png_character(bytes)?
    } else {
        bytes.to_vec()
    };
    let value: Value = serde_json::from_slice(&json_bytes).map_err(|error| {
        UiMsg::CardJsonInvalid {
            error: error.to_string(),
        }
        .into_error()
    })?;
    let card_data = value
        .get("data")
        .filter(|data| data.is_object())
        .unwrap_or(&value);
    let name = string_field(card_data, "name")
        .ok_or_else(|| UiMsg::CardMissingName.into_error())?
        .trim()
        .to_owned();
    data::validate_single_line("name", &name)?;
    Ok(ParsedCard {
        json_bytes,
        value,
        name,
    })
}

/// 只驗卡檔（不寫任何東西）：匯入本體在寫「未完成」標記前先驗，壞檔不會留下標記。
pub fn check_character_bytes(bytes: &[u8]) -> DataResult<()> {
    parse_character(bytes).map(|_| ())
}

/// 角色卡匯入的結果：新角色＋卡圖是否沒存成（PNG 卡的圖救不回，沒有顯示圖）＋
/// 卡片隨身世界書收編結果（`book_failed`＝卡帶了書卻沒匯成，角色照建）。
pub struct ImportedCharacter {
    pub meta: CharacterMeta,
    pub image_dropped: bool,
    pub book: Option<data::BookImport>,
    pub book_failed: bool,
}

/// 匯入永遠是全新一張卡：mint 新 id，name 照卡片原值（不再擋特殊字元，只擋換行）。
/// `lang`：介面語系，決定寫進內文的段標語言（寫入後就是玩家內文，不隨語系變）。
/// 正式路徑都走 import_character_reporting；這支只剩測試用。
#[cfg(test)]
pub fn import_character(
    root: &Path,
    world_id: &str,
    bytes: &[u8],
    color: &str,
    lang: &str,
) -> DataResult<CharacterMeta> {
    import_character_reporting(root, world_id, bytes, color, lang).map(|imported| imported.meta)
}

/// 同 import_character，另回報卡圖有沒有存成與隨身世界書的收編結果。
pub fn import_character_reporting(
    root: &Path,
    world_id: &str,
    bytes: &[u8],
    color: &str,
    lang: &str,
) -> DataResult<ImportedCharacter> {
    // 卡片隨身世界書盡力而為：寫不進去不擋角色本體，回報給玩家
    import_character_placing(root, world_id, bytes, color, lang, BookWrite::BestEffort)
}

/// 卡片隨身世界書（與機制、擴充欄位）寫不進去時怎麼辦。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BookWrite {
    /// 桌面版：略過，角色照建，結果帶 `book_failed`
    BestEffort,
    /// 網頁存檔匯入：整次回錯（失敗不留半桌）
    Strict,
}

/// 角色卡路的本體。卡內條目沒指定可見度就給這張卡自己的角色（「這張卡由誰演就給誰看」；世界書路
/// 的卡由 GM 演，見 files.rs 的 import_worldbook_file）。
pub(crate) fn import_character_placing(
    root: &Path,
    world_id: &str,
    bytes: &[u8],
    color: &str,
    lang: &str,
    book_write: BookWrite,
) -> DataResult<ImportedCharacter> {
    let ParsedCard {
        json_bytes,
        value,
        name,
    } = parse_character(bytes)?;
    let card_data = value
        .get("data")
        .filter(|data| data.is_object())
        .unwrap_or(&value);

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
        public_md: public_markdown(card_data, lang),
        private_md: private_markdown(card_data, lang),
    };
    data::write_character(root, world_id, &card)?;
    // 原卡或顯示圖寫不進去：整次回錯，剛建的角色 md 與已寫的檔一併收掉，不留半匯入的角色
    let image_dropped = match store_card_source(&md_path, bytes, &json_bytes) {
        Ok(dropped) => dropped,
        Err(error) => return Err(roll_back_character(root, world_id, &id, error)),
    };
    let strict = book_write == BookWrite::Strict;
    if strict {
        import_table_tavern_extension_strict(root, world_id, &name, card_data)?;
    } else {
        import_table_tavern_extension(root, world_id, &name, card_data);
    }
    let mut book = None;
    let mut book_failed = false;
    if let Some(book_value) = card_data.get("character_book") {
        if strict {
            import_mechanism_strict(root, world_id, book_value)?;
        } else {
            import_mechanism(root, world_id, book_value);
        }
        // 卡片隨身的設定條目進這桌世界書，照觸發規則送給這張卡的角色（同名條目由去重合併）。
        // 交原文才保得住物件形條目的鍵順序
        let text = match character_book_text(&json_bytes, &value) {
            Some(text) => text.to_owned(),
            None => serde_json::to_string(book_value)?,
        };
        let owner = data::BookOwner::Character(id.clone());
        match data::import_worldbook_as(root, world_id, &text, &owner) {
            Ok(imported) => book = Some(imported),
            Err(error) if strict => return Err(error),
            Err(error) => {
                log::warn!("character book import failed: {error}");
                book_failed = true;
            }
        }
    }

    Ok(ImportedCharacter {
        meta: CharacterMeta {
            id,
            name,
            color: color.to_owned(),
            avatar: "🎭".to_owned(),
            tier: Tier::Balanced,
            show_image: true,
            archived: false,
            auto_hidden: false,
            display_index: None,
        },
        image_dropped,
        book,
        book_failed,
    })
}

/// 匯入寫檔失敗後收掉剛建的角色，連同這次寫入可能留下的暫存檔（原子寫失敗時刪暫存檔的錯被吞掉，
/// 由 delete_character 再清一次）。
/// 清乾淨就回原錯；清不乾淨回 `CharacterImportRollbackFailed`，同時帶原錯、角色 id 與留下的檔名，
/// 讓玩家知道有殘檔要清。整個工作區會整批收掉的呼叫端（網頁存檔、重構清回）用
/// [`original_import_error`] 換回原錯。
fn roll_back_character(
    root: &Path,
    world_id: &str,
    id: &str,
    error: Box<dyn std::error::Error + Send + Sync>,
) -> Box<dyn std::error::Error + Send + Sync> {
    // delete_character 連這次寫入可能留下的暫存檔一起清
    let cleanup = data::delete_character(root, world_id, id);
    let left = data::character_files_present(root, world_id, id).unwrap_or_default();
    if cleanup.is_ok() && left.is_empty() {
        return error;
    }
    if let Err(cleanup) = cleanup {
        log::warn!("rollback of half-imported character failed: {cleanup}");
    }
    UiMsg::CharacterImportRollbackFailed {
        error: error.to_string(),
        id: id.to_owned(),
        files: left.join(", "),
    }
    .into_error()
}

/// 呼叫端失敗後整批收掉工作區（網頁存檔的新桌、重構清回的臨時根）時，殘檔會一起清掉，
/// 不必叫玩家手動刪：`CharacterImportRollbackFailed` 換回裡面的原錯，其他錯原樣回傳。
pub(crate) fn original_import_error(
    error: Box<dyn std::error::Error + Send + Sync>,
) -> Box<dyn std::error::Error + Send + Sync> {
    match UiMsg::parse(&error.to_string()) {
        Some(UiMsg::CharacterImportRollbackFailed { error: inner, .. }) => {
            data::invalid_data(inner)
        }
        _ => error,
    }
}

/// 卡原檔與顯示圖落地（都原子寫）。原卡 `<id>.card.png` 與 `<id>.import.json` 只寫其一，之後沒有路徑會
/// 改它；顯示圖 `<id>.png` 另存一份剝掉卡片文字的圖，換圖、刪圖只動這份。
/// - JSON 卡：原檔照存 `.import.json`，沒有顯示圖。
/// - PNG 卡：圖走救圖版；截過或原樣的檔卡文字還在，重編的把 chara／ccv3 搬過去。最終檔要過嚴驗、
///   讀回的卡 JSON 與匯入時相同才存 `.card.png`；否則改存解出的卡 JSON 成 `.import.json`。
///   顯示圖只要救得回就存（搬卡文字失敗也照存）。
///
/// 回傳顯示圖是否沒存成（PNG 卡的圖救不回）；寫檔失敗回 Err。
fn store_card_source(md_path: &Path, bytes: &[u8], json_bytes: &[u8]) -> DataResult<bool> {
    if !bytes.starts_with(PNG_MAGIC) {
        data::commit_world_write_atomic(&md_path.with_extension("import.json"), bytes)?;
        return Ok(false);
    }
    let (image, card) = match png_clean::salvage_stored_png(bytes) {
        Some(Stored::AsIs) => (Some(bytes.to_vec()), Some(bytes.to_vec())),
        Some(Stored::Rewritten(png)) => (Some(png.clone()), Some(png)),
        Some(Stored::Reencoded(clean)) => {
            let card = png_clean::transplant_card_text(bytes, &clean);
            (Some(clean), card)
        }
        None => (None, None),
    };
    let card = card.filter(|png| {
        validate_png_image(png, STORED_IMAGE_LIMITS).is_ok()
            && decode_png_character(png).is_ok_and(|decoded| decoded == json_bytes)
    });
    let display = image
        .and_then(|png| png_clean::strip_card_text(&png).ok())
        .filter(|png| validate_png_image(png, STORED_IMAGE_LIMITS).is_ok());
    match card {
        Some(png) => data::commit_world_write_atomic(&md_path.with_extension("card.png"), &png)?,
        None => {
            data::commit_world_write_atomic(&md_path.with_extension("import.json"), json_bytes)?
        }
    }
    match display {
        Some(png) => {
            data::commit_world_write_atomic(&md_path.with_extension("png"), &png)?;
            Ok(false)
        }
        None => Ok(true),
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

fn public_markdown(data: &Value, lang: &str) -> String {
    let index = lang_index(lang);
    PUBLIC_SECTIONS
        .into_iter()
        .filter_map(|(field, labels)| {
            let content = string_field(data, field)?;
            (!content.trim().is_empty()).then(|| format!("### {}\n{content}", labels[index]))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// 私設欄只收備用開場白；卡內世界書條目進這桌世界書（照觸發規則送），不再傾印進私設。
fn private_markdown(data: &Value, lang: &str) -> String {
    let mut sections: Vec<String> = Vec::new();
    sections.extend(
        data.get("alternate_greetings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|greeting| !greeting.is_empty())
            .enumerate()
            .map(|(index, greeting)| {
                let heading = ALTERNATE_GREETING[lang_index(lang)].replacen(
                    "{n}",
                    &(index + 1).to_string(),
                    1,
                );
                format!("### {heading}\n{greeting}")
            }),
    );
    sections.join("\n\n")
}

/// 物件第一層成員 `name` 的原文。
fn raw_member<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let mut members: std::collections::HashMap<String, &'a serde_json::value::RawValue> =
        serde_json::from_str(text).ok()?;
    members.remove(name).map(serde_json::value::RawValue::get)
}

/// 卡 JSON 原文裡 `card_data` 那層的原文：`data` 是物件就是它，否則整份。
fn card_data_text<'a>(json_bytes: &'a [u8], value: &Value) -> Option<&'a str> {
    let text = std::str::from_utf8(json_bytes).ok()?;
    match value.get("data").is_some_and(Value::is_object) {
        true => raw_member(text, "data"),
        false => Some(text),
    }
}

/// 實際匯入會用的那本書（`worldbook_json` 的判定：`character_book`、`data.entries` 或頂層 `entries`）
/// 的物件形 `entries` 鍵順序（原文順序）；陣列形或沒有條目回 `None`。
pub(crate) fn book_object_key_order(json_bytes: &[u8]) -> Option<Vec<String>> {
    let book = worldbook_json(json_bytes).ok()?;
    let entries = raw_member(&book, "entries")?;
    match crate::world_info::book_order::SourceEntries::parse(entries).ok()?? {
        crate::world_info::book_order::SourceEntries::Object(items) => {
            Some(items.into_iter().map(|(key, _)| key).collect())
        }
        crate::world_info::book_order::SourceEntries::Array(_) => None,
    }
}

/// `card_data.character_book` 的原文。
fn character_book_text<'a>(json_bytes: &'a [u8], value: &Value) -> Option<&'a str> {
    raw_member(card_data_text(json_bytes, value)?, "character_book")
}

/// 世界書匯入的前處理：PNG 卡先解出內嵌 JSON；整包若是角色卡（社群發佈的世界書卡），
/// 剝到 character_book 那層再交給 data::import_worldbook；卡上沒有條目時改走人設欄轉換
pub fn worldbook_json(bytes: &[u8]) -> DataResult<String> {
    let json_bytes = if bytes.starts_with(PNG_MAGIC) {
        decode_png_character(bytes)?
    } else {
        bytes.to_vec()
    };
    let value: Value = serde_json::from_slice(&json_bytes).map_err(|error| {
        UiMsg::WorldbookJsonInvalid {
            error: error.to_string(),
        }
        .into_error()
    })?;
    let card_data = value
        .get("data")
        .filter(|data| data.is_object())
        .unwrap_or(&value);
    let has_entries = |book: &Value| {
        book.get("entries")
            .is_some_and(|entries| !book_entry_values(entries).is_empty())
    };
    // 回原文（不經 Value 重新序列化）：物件形條目的鍵順序決定新 UID 與 ST 載入先後
    if let Some(book) = card_data.get("character_book").filter(|b| has_entries(b)) {
        return Ok(character_book_text(&json_bytes, &value)
            .map_or_else(|| book.to_string(), str::to_owned));
    }
    // 頂層就是世界書本體（V2 獨立書 JSON，entries 是物件不是陣列）
    if card_data.get("entries").is_some() {
        return Ok(card_data_text(&json_bytes, &value)
            .map_or_else(|| card_data.to_string(), str::to_owned));
    }
    persona_as_worldbook(card_data)
        .map(|book| book.to_string())
        .ok_or_else(|| UiMsg::CardNothingToImport.into_error())
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

#[cfg(test)]
#[path = "card_tests.rs"]
mod tests;
