//! 卡片契約黃金檔（src/shared/contracts/card-view/）：桌面版把卡算成「卡片正規化檢視」，
//! 跟網頁版 web/src/features/cards/card-view.ts 對同一份 golden.json 比對。檢視一律用桌面版真正的
//! 匯入函式算（PNG 解碼、probe_import、card_openings、worldbook_json、card_interface），
//! 條目欄位的正規化規則照 card-view.md。
use super::card::{
    book_entries_keyed, card_openings, check_character_bytes, probe_import, worldbook_json,
};
use super::card_io::{decode_png_character, find_card_text, string_field, PNG_MAGIC};
use super::interface::card_interface;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const STRING_FIELDS: [&str; 11] = [
    "name",
    "description",
    "personality",
    "scenario",
    "first_mes",
    "mes_example",
    "creator_notes",
    "system_prompt",
    "post_history_instructions",
    "creator",
    "character_version",
];

fn contract_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/shared/contracts/card-view")
}

/// TTMSG 錯誤取出 code（png_invalid、card_png_no_data…），契約裡的錯誤就是這個 code。
fn error_code(error: &(dyn std::error::Error + Send + Sync)) -> String {
    let text = error.to_string();
    text.strip_prefix(crate::ui_msg::MARK)
        .and_then(|json| serde_json::from_str::<Value>(json).ok())
        .and_then(|value| value["code"].as_str().map(str::to_owned))
        .unwrap_or(text)
}

fn decode(bytes: &[u8]) -> Result<(&'static str, Vec<u8>), String> {
    if !bytes.starts_with(PNG_MAGIC) {
        return Ok(("json", bytes.to_vec()));
    }
    let found = match find_card_text(bytes, b"chara") {
        Err(error) => return Err(error_code(&*error)),
        Ok(Some(found)) => ("png:chara", found),
        Ok(None) => match find_card_text(bytes, b"ccv3") {
            Err(error) => return Err(error_code(&*error)),
            Ok(Some(found)) => ("png:ccv3", found),
            Ok(None) => return Err("card_png_no_data".to_owned()),
        },
    };
    // 跟匯入實際走的那支解碼一致
    assert_eq!(
        decode_png_character(bytes).ok().as_deref(),
        Some(&found.1[..])
    );
    Ok(found)
}

fn strings(value: Option<&Value>) -> Vec<&str> {
    value
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

fn entry_view(key: &str, entry: &Value, object_form: bool) -> Value {
    let text = |field: &str| entry.get(field).and_then(Value::as_str).unwrap_or("");
    let flag = |field: &str| entry.get(field).and_then(Value::as_bool);
    let (keys, secondary, order, enabled, uid) = if object_form {
        (
            strings(entry.get("key")),
            strings(entry.get("keysecondary")),
            entry.get("order").and_then(Value::as_i64).unwrap_or(0),
            !flag("disable").unwrap_or(false),
            entry
                .get("uid")
                .and_then(Value::as_i64)
                .or_else(|| key.parse().ok()),
        )
    } else {
        (
            strings(entry.get("keys")),
            strings(entry.get("secondary_keys")),
            entry
                .get("insertion_order")
                .and_then(Value::as_i64)
                .unwrap_or(0),
            // 缺或 null＝啟用，其餘照 JS 真假值（與匯入同一套規則）
            entry.get("enabled").is_none_or(|enabled| {
                enabled.is_null() || crate::world_info::js_semantics::truthy(Some(enabled))
            }),
            entry.get("id").and_then(Value::as_i64),
        )
    };
    json!({
        "key": key,
        "uid": uid,
        "keys": keys,
        "secondary_keys": secondary,
        "comment": text("comment"),
        "content": text("content"),
        "constant": flag("constant").unwrap_or(false),
        "enabled": enabled,
        "order": order,
        "position": entry.get("position").cloned().unwrap_or(Value::Null),
    })
}

fn entries_view(entries: Option<&Value>) -> Vec<Value> {
    let Some(entries) = entries else {
        return Vec::new();
    };
    let object_form = entries.is_object();
    book_entries_keyed(entries)
        .into_iter()
        .filter(|(_, entry)| entry.is_object())
        .map(|(key, entry)| entry_view(&key, entry, object_form))
        .collect()
}

fn form(entries: Option<&Value>) -> &'static str {
    match entries {
        Some(Value::Array(_)) => "array",
        Some(Value::Object(_)) => "object",
        _ => "none",
    }
}

/// 有效性（契約「有效性規則」）：各匯入路桌面版會不會拒收；null＝收。
fn validity(bytes: &[u8], has_name: bool) -> Value {
    let character = match check_character_bytes(bytes) {
        Err(error) => {
            let code = error_code(&*error);
            if code == "name must be a single line" {
                "name_not_single_line".to_owned()
            } else {
                code
            }
        }
        // 空白名字桌面版的身分框直接走世界書路，角色卡路等於收不到
        Ok(()) if !has_name => "card_missing_name".to_owned(),
        Ok(()) => return json!({ "character": null, "worldbook": worldbook_validity(bytes) }),
    };
    json!({ "character": character, "worldbook": worldbook_validity(bytes) })
}

fn worldbook_validity(bytes: &[u8]) -> Value {
    match worldbook_json(bytes) {
        Ok(_) => Value::Null,
        Err(error) => json!(error_code(&*error)),
    }
}

pub(super) fn card_view(bytes: &[u8]) -> Value {
    let (source, json_bytes) = match decode(bytes) {
        Ok(found) => found,
        Err(code) => return json!({ "error": code }),
    };
    let value: Value = match serde_json::from_slice(&json_bytes) {
        Ok(value) => value,
        Err(_) => return json!({ "error": "card_json_invalid" }),
    };
    let wrapped = value.get("data").is_some_and(Value::is_object);
    let card_data = value
        .get("data")
        .filter(|data| data.is_object())
        .unwrap_or(&value);
    let shell_text = |field: &str| value.get(field).and_then(Value::as_str).map(str::to_owned);
    // 依字元序（serde_json 開了 preserve_order，Map 是原序，要自己排）
    let mut extra_keys: Vec<&String> = match (&value, wrapped) {
        (Value::Object(map), true) => map
            .keys()
            .filter(|key| !["spec", "spec_version", "data"].contains(&key.as_str()))
            .collect(),
        _ => Vec::new(),
    };
    extra_keys.sort();
    let fields: serde_json::Map<String, Value> = STRING_FIELDS
        .iter()
        .map(|field| {
            let text = string_field(card_data, field).map_or(Value::Null, |text| json!(text));
            ((*field).to_owned(), text)
        })
        .collect();

    let probe = probe_import(bytes);
    let decision = if probe.name.is_none() || probe.book_shaped {
        "worldbook"
    } else {
        "ask"
    };
    let suggested =
        if decision == "worldbook" || probe.lorebook_heavy || probe.alternate_greetings > 0 {
            "worldbook"
        } else {
            "character"
        };

    let character_book = card_data
        .get("character_book")
        .filter(|book| book.is_object())
        .map(|book| {
            json!({
                "name": book.get("name").and_then(Value::as_str),
                "form": form(book.get("entries")),
                "entries": entries_view(book.get("entries")),
            })
        });
    let worldbook = match worldbook_json(bytes) {
        Err(_) => json!({ "source": "none", "name": null, "form": "none", "entries": [] }),
        Ok(text) => {
            let book: Value = serde_json::from_str(&text).unwrap();
            let source = if card_data.get("character_book") == Some(&book) {
                "character_book"
            } else if &book == card_data {
                "top_level"
            } else {
                "persona"
            };
            json!({
                "source": source,
                "name": book.get("name").and_then(Value::as_str),
                "form": form(book.get("entries")),
                "entries": entries_view(book.get("entries")),
            })
        }
    };

    let name = string_field(card_data, "name").unwrap_or("");
    let interface = card_interface("", name, card_data);

    json!({
        "source": source,
        "shell": {
            "wrapped": wrapped,
            "spec": shell_text("spec"),
            "spec_version": shell_text("spec_version"),
            "extra_keys": extra_keys,
        },
        "fields": fields,
        "tags": strings(card_data.get("tags")),
        "alternate_greetings": strings(card_data.get("alternate_greetings")),
        "extensions": card_data.get("extensions").cloned().unwrap_or(Value::Null),
        "openings": card_openings(bytes).map(|(_, openings)| openings).unwrap_or_default(),
        "route": {
            "name": probe.name,
            "book_shaped": probe.book_shaped,
            "lorebook_heavy": probe.lorebook_heavy,
            "book_entries": probe.book_entries,
            "alternate_greetings": probe.alternate_greetings,
            "decision": decision,
            "suggested": suggested,
        },
        "books": { "character": character_book, "worldbook": worldbook },
        "validity": validity(bytes, probe.name.is_some()),
        "interface": {
            "unsupported": interface.unsupported,
            "mvu": interface.mvu,
            "scripts": serde_json::to_value(&interface.scripts).unwrap(),
        },
    })
}

#[test]
fn card_views_match_the_shared_golden_file() {
    let dir = contract_dir();
    let golden: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("golden.json")).unwrap()).unwrap();
    let cases = golden.as_object().unwrap();
    assert!(cases.len() >= 8, "黃金檔少了 fixture");
    for (file, expected) in cases {
        let bytes = std::fs::read(dir.join(file)).unwrap();
        let actual = card_view(&bytes);
        assert!(
            &actual == expected,
            "{file} 的卡片檢視與黃金檔不同\n實際：{}\n預期：{}",
            serde_json::to_string_pretty(&actual).unwrap(),
            serde_json::to_string_pretty(expected).unwrap()
        );
    }
}

/// 本機實卡（TestCards/，gitignore）：設了 TT_CARD_VIEW_TESTCARDS 與 TT_CARD_VIEW_OUT 才跑，
/// 把每張卡的檢視寫成 `<檔名>.rust.json`，再由 web 那邊的同名測試比對。CI 沒設，直接略過。
#[test]
fn writes_local_testcard_views_when_requested() {
    let (Ok(cards), Ok(out)) = (
        std::env::var("TT_CARD_VIEW_TESTCARDS"),
        std::env::var("TT_CARD_VIEW_OUT"),
    ) else {
        return;
    };
    std::fs::create_dir_all(&out).unwrap();
    let mut written = 0;
    for entry in std::fs::read_dir(&cards).unwrap() {
        let path = entry.unwrap().path();
        let ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        if !["png", "json"].contains(&ext) {
            continue;
        }
        let view = card_view(&std::fs::read(&path).unwrap());
        let name = path.file_name().unwrap().to_string_lossy();
        std::fs::write(
            Path::new(&out).join(format!("{name}.rust.json")),
            serde_json::to_string_pretty(&view).unwrap(),
        )
        .unwrap();
        written += 1;
    }
    assert!(written > 0, "TestCards 目錄裡沒有卡");
}

#[test]
fn v2_enabled_follows_the_import_rule() {
    let enabled = |entry: Value| entry_view("0", &entry, false)["enabled"].clone();
    for on in [
        json!({}),
        json!({"enabled": null}),
        json!({"enabled": true}),
        json!({"enabled": 1}),
        json!({"enabled": "yes"}),
    ] {
        assert_eq!(enabled(on.clone()), true, "{on}");
    }
    for off in [
        json!({"enabled": false}),
        json!({"enabled": 0}),
        json!({"enabled": ""}),
    ] {
        assert_eq!(enabled(off.clone()), false, "{off}");
    }
}
