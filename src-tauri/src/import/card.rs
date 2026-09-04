use super::card_io::{decode_png_character, string_field, PNG_MAGIC};
use super::mechanism::{import_mechanism, import_table_tavern_extension};
use crate::data::{self, CharacterCard, CharacterMeta, DataResult, Tier};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

/// 人設欄：既是 lorebook_heavy 秤重時的「人設份量」，也是沒有條目的卡轉成世界書時要收的內容。
/// 不含 first_mes——開場白走 card_openings 讓玩家挑，收進條目會每回合重複注入。
const PERSONA_FIELDS: [&str; 4] = ["description", "personality", "scenario", "mes_example"];

/// 公開段落與 SillyTavern 欄位的對照表：匯入時拆成 `### 標題`，匯出時再併回欄位
pub(super) const PUBLIC_SECTIONS: [(&str, &str); 5] = [
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
        auto_hidden: false,
        display_index: None,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::test_support::{minimal_png, TestRoot};

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
            "---\nid: {}\nname: 莉亞\ncolor: #3366ff\navatar: 🎭\ntier: balanced\nshow_image: true\narchived: false\nauto_hidden: false\ndisplay_index: 0\ngen_prompt: \n---",
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
}
