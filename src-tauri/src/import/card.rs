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

/// 角色卡匯入的結果：新角色＋卡圖是否沒存成（PNG 卡的圖救不回，原檔改存成 .import.json）＋
/// 卡片隨身世界書收編結果（`book_failed`＝卡帶了書卻沒匯成，角色照建）。
pub struct ImportedCharacter {
    pub meta: CharacterMeta,
    pub image_dropped: bool,
    pub book: Option<data::BookImport>,
    pub book_failed: bool,
}

/// 匯入永遠是全新一張卡：mint 新 id，name 照卡片原值（不再擋特殊字元，只擋換行）。
/// `lang`：介面語系，決定寫進內文的段標語言（寫入後就是玩家內文，不隨語系變）。
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
    let image_dropped = store_card_source(&md_path, bytes, &json_bytes)?;
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
        // 卡片隨身的設定條目進這桌世界書，照觸發規則送給這張卡的角色（同名條目由去重合併）
        let text = serde_json::to_string(book_value)?;
        let owner = data::BookOwner::Character(id.clone());
        match data::import_worldbook_as(root, world_id, &text, &owner) {
            Ok(imported) if strict && imported.summary.invalid > 0 => {
                return Err(data::invalid_data(
                    "character_book enabled must be a boolean",
                ));
            }
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

/// 卡原檔落地（`.png` 與 `.import.json` 只寫其一，原子寫）。卡片介面從這份讀卡資料，所以：
/// - JSON 卡：原檔照存 `.import.json`。
/// - PNG 卡：圖走救圖版；截過或原樣的檔卡文字還在，重編的把 chara／ccv3 搬過去。最終檔要過嚴驗、
///   讀回的卡 JSON 與匯入時相同才存 `.png`；否則（救不回、搬不到）改存解出的卡 JSON 成 `.import.json`。
///
/// 回傳圖是否沒存成。
fn store_card_source(md_path: &Path, bytes: &[u8], json_bytes: &[u8]) -> DataResult<bool> {
    if !bytes.starts_with(PNG_MAGIC) {
        data::commit_world_write_atomic(&md_path.with_extension("import.json"), bytes)?;
        return Ok(false);
    }
    let png = match png_clean::salvage_stored_png(bytes) {
        Some(Stored::AsIs) => Some(bytes.to_vec()),
        Some(Stored::Rewritten(png)) => Some(png),
        Some(Stored::Reencoded(clean)) => png_clean::transplant_card_text(bytes, &clean),
        None => None,
    }
    .filter(|png| {
        validate_png_image(png, STORED_IMAGE_LIMITS).is_ok()
            && decode_png_character(png).is_ok_and(|decoded| decoded == json_bytes)
    });
    match png {
        Some(png) => {
            data::commit_world_write_atomic(&md_path.with_extension("png"), &png)?;
            Ok(false)
        }
        None => {
            data::commit_world_write_atomic(&md_path.with_extension("import.json"), json_bytes)?;
            Ok(true)
        }
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
    if let Some(book) = card_data.get("character_book").filter(|b| has_entries(b)) {
        return Ok(book.to_string());
    }
    // 頂層就是世界書本體（V2 獨立書 JSON，entries 是物件不是陣列）
    if card_data.get("entries").is_some() {
        return Ok(card_data.to_string());
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
mod tests {
    use super::*;
    use crate::import::test_support::{card_png, minimal_png, TestRoot};
    use std::fs;

    #[test]
    fn imports_v2_json_and_preserves_original() {
        let root = TestRoot::new("v2");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let raw = r#"{"spec":"chara_card_v2","spec_version":"2.0","data":{"name":"莉亞","description":"精靈遊俠","personality":"冷靜","scenario":"雨夜","first_mes":"妳來了。","mes_example":"<START>","character_book":{"entries":[{"keys":["森林","月亮"],"content":"古老盟約","enabled":true},{"keys":["略過"],"content":""}]}}}"#.as_bytes();

        let meta = import_character(root.path(), &world_id, raw, "#3366ff", "zh-TW").unwrap();
        assert_eq!(meta.name, "莉亞");
        let markdown = fs::read_to_string(
            root.path()
                .join(format!("worlds/{world_id}/characters/{}.md", meta.id)),
        )
        .unwrap();
        assert!(markdown.contains(&format!(
            "---\nid: {}\nname: 莉亞\ncolor: #3366ff\navatar: 🎭\ntier: balanced\nshow_image: true\narchived: false\nauto_hidden: false\ndisplay_index: 0\ngen_prompt: \n---",
            meta.id
        )));
        for section in [
            "### 簡介\n精靈遊俠",
            "### 人格與語氣\n冷靜",
            "### 場景\n雨夜",
            "### 開場白\n妳來了。",
            "### 語氣範例\n<START>",
        ] {
            assert!(markdown.contains(section), "missing {section}");
        }
        assert!(!markdown.contains("古老盟約"), "條目不傾印進私設");
        assert_eq!(
            fs::read(root.path().join(format!(
                "worlds/{world_id}/characters/{}.import.json",
                meta.id
            )))
            .unwrap(),
            raw
        );
    }

    #[test]
    fn imports_png_text_chunk_and_preserves_original() {
        let root = TestRoot::new("png");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let png = card_png(r#"{"data":{"name":"凱恩","description":"騎士"}}"#);

        let meta = import_character(root.path(), &world_id, &png, "#111111", "zh-TW").unwrap();
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
            "zh-TW",
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

        import_character(root.path(), &world_id, card.as_bytes(), "#ffffff", "zh-TW").unwrap();

        let entries = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|entry| entry.content == "北境的漁村"));

        // 同一張卡再匯一次：條目由去重擋下，不會長出第二份
        import_character(root.path(), &world_id, card.as_bytes(), "#ffffff", "zh-TW").unwrap();
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

        let meta =
            import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff", "zh-TW").unwrap();
        let private_md = data::read_character(root.path(), &world_id, &meta.id)
            .unwrap()
            .private_md;
        for expected in [
            "### 備用開場白 1",
            "第二次見面。",
            "### 備用開場白 2",
            "雨天再訪。",
        ] {
            assert!(private_md.contains(expected), "missing {expected}");
        }
        // 卡內世界書條目不再傾印進私設：進這桌世界書、給這張卡的角色看
        assert!(!private_md.contains("古老盟約"), "{private_md}");
        let entries = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].visibility,
            data::Visibility::Characters(vec![meta.id.clone()])
        );
    }

    /// 測試清單 #12：匯入 ST 角色卡產生新 id、name 照原值（含原本會被擋的字元）；
    /// 同名再匯入一次也會成功，各自拿到不同 id 且互不影響
    #[test]
    fn importing_the_same_name_twice_mints_distinct_ids_and_keeps_first_card_intact() {
        let root = TestRoot::new("duplicate-name");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let odd_name = r#"{"name":"a/b/../重名","description":"第一張"}"#;
        let first = import_character(
            root.path(),
            &world_id,
            odd_name.as_bytes(),
            "#000000",
            "zh-TW",
        )
        .unwrap();
        assert_eq!(first.name, "a/b/../重名");

        let second = import_character(
            root.path(),
            &world_id,
            r#"{"name":"a/b/../重名","description":"第二張"}"#.as_bytes(),
            "#ffffff",
            "zh-TW",
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
                skipped: 2,
                invalid: 0
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

    /// 物件形 entries（ST 獨立世界書格式塞進 character_book）一律算有條目、照 uid 鍵展開，
    /// 不能掉進人設欄轉換（卡片契約）
    #[test]
    fn object_shaped_character_book_counts_as_entries() {
        let card = json!({"data": {
            "name": "北境驛站",
            "description": "短介紹",
            "character_book": {"name": "北境", "entries": {
                "10": {"uid": 10, "key": ["驛站"], "content": "驛".repeat(40)},
                "2": {"uid": 2, "key": ["王府"], "content": "府".repeat(40)},
                "0": {"uid": 0, "key": [], "content": "常駐", "constant": true},
            }},
        }})
        .to_string();

        let probe = probe_import(card.as_bytes());
        assert_eq!(probe.book_entries, 3);
        assert!(probe.lorebook_heavy);

        let book: Value = serde_json::from_str(&worldbook_json(card.as_bytes()).unwrap()).unwrap();
        assert_eq!(book["name"], "北境");
        let uids: Vec<_> = book_entry_values(&book["entries"])
            .iter()
            .map(|entry| entry["uid"].as_u64().unwrap())
            .collect();
        assert_eq!(uids, [0, 2, 10]);

        let root = TestRoot::new("object-book");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let json = worldbook_json(card.as_bytes()).unwrap();
        assert_eq!(
            data::import_worldbook(root.path(), &world_id, &json)
                .unwrap()
                .imported,
            3
        );
        // 落地順序與新配的 UID 照原鍵 0、2、10，不是字串序 0、10、2
        let landed: Vec<_> = data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .into_iter()
            .map(|entry| (entry.uid, entry.content.chars().next().unwrap()))
            .collect();
        assert_eq!(landed, [(0, '常'), (1, '府'), (2, '驛')]);
    }

    /// 不是物件的條目值（字串、null）不算條目、匯入時略過，整本不會半路失敗（卡片契約）
    #[test]
    fn non_object_entry_values_are_skipped_not_fatal() {
        let card = json!({"data": {
            "name": "壞值卡",
            "character_book": {"entries": {"0": {"key": ["甲"], "content": "有效"}, "1": "字串", "2": null}},
        }})
        .to_string();
        assert_eq!(probe_import(card.as_bytes()).book_entries, 1);
        let root = TestRoot::new("non-object-entries");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let json = worldbook_json(card.as_bytes()).unwrap();
        let result = data::import_worldbook(root.path(), &world_id, &json).unwrap();
        assert_eq!(result.imported, 1);
        let array_book =
            json!({"entries": [{"keys": ["乙"], "content": "陣列有效"}, "字串", null]}).to_string();
        assert_eq!(
            data::import_worldbook(root.path(), &world_id, &array_book)
                .unwrap()
                .imported,
            1
        );
        assert_eq!(
            data::read_worldbook(root.path(), &world_id).unwrap().len(),
            2
        );
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
                skipped: 0,
                invalid: 0
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
}
