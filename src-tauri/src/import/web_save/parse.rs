//! 網頁存檔（桌檔契約 v1，src/shared/contracts/web-save/web-save.md）的讀取與檢查：信任邊界，動任何資料前
//! 整份驗完。前端 `web-save.ts` 的 `parseWebSave` 是同一套規則。
use crate::data::message_vars::{parse_json, validate_table, Json, Macros};
use crate::data::DataResult;
use crate::ui_msg::UiMsg;
use base64::Engine;
use serde::Deserialize;
use serde_json::value::RawValue;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

pub const FORMAT: &str = "table-tavern-web-save";
pub const VERSION: u64 = 1;
pub const MAX_SAVE_BYTES: usize = 64 * 1024 * 1024;
const MAX_MESSAGES: usize = 50_000;
const MAX_TEXT_BYTES: usize = 2 * 1024 * 1024;
const MAX_ID_CHARS: usize = 128;
const MAX_USER_NAME_CHARS: usize = 256;
const MAX_LAYER_ID_CHARS: usize = crate::data::card_vars::MAX_ID_CHARS;
const MAX_CARD_STORAGE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Character,
    Worldbook,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Char,
}

pub struct Message {
    pub id: String,
    pub role: Role,
    pub text: String,
    pub raw: Option<String>,
    pub ts: String,
    pub opening: bool,
    pub interrupted: bool,
    pub table: Option<Json>,
}

pub struct WorldInfo {
    /// 原樣（旁檔照這份寫，一個字都不改）
    pub raw: Box<RawValue>,
    /// 條目穩定 ID → 卡片契約的條目 key
    pub entries: Vec<(String, String)>,
}

pub struct Layers {
    pub chat: Json,
    pub character: Json,
    pub global: Json,
    pub preset: Json,
    pub script: Vec<(String, Json)>,
    pub extension: Vec<(String, Json)>,
}

pub struct Mvu {
    pub macros: Option<Macros>,
    pub seed: Option<Json>,
    pub layers: Layers,
}

pub struct WebSave {
    pub card: Value,
    /// `card` 的原文：物件形世界書條目的鍵順序要從這裡讀（`Value` 會把鍵排序）
    pub card_text: String,
    pub card_png: Option<Vec<u8>>,
    pub route: Route,
    pub regex_allowed: bool,
    pub user_name: String,
    pub opening_index: Option<u64>,
    pub messages: Vec<Message>,
    pub world_info: WorldInfo,
    pub mvu: Option<Mvu>,
    pub card_storage: serde_json::Map<String, Value>,
}

fn invalid(detail: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
    UiMsg::WebSaveInvalid {
        detail: detail.into(),
    }
    .into_error()
}

/// 只看外框：是不是網頁存檔、版號認不認得。
#[derive(Deserialize)]
struct Head {
    format: Option<Value>,
    version: Option<Value>,
}

/// 必填但可為 null 的欄：serde 對 `Option` 欄位缺鍵會默默當 None，這裡要求鍵一定在（與前端檢查一致）。
struct Nullable<T>(Option<T>);

// 不能直接用 Option<T>：serde 對缺鍵會餵 None。先收原文（缺鍵就報 missing field），是 null 才當 None。
impl<'de, T: serde::de::DeserializeOwned> Deserialize<'de> for Nullable<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let raw = Box::<RawValue>::deserialize(deserializer)?;
        if raw.get() == "null" {
            return Ok(Nullable(None));
        }
        serde_json::from_str(raw.get())
            .map(|value| Nullable(Some(value)))
            .map_err(D::Error::custom)
    }
}

/// 可省但不可為 null 的欄：鍵不在＝None（配 `#[serde(default)]`），在就必須是該型別（null 拒收）。
fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Deserialize)]
struct RawSave {
    card: Box<RawValue>,
    #[serde(default, deserialize_with = "present")]
    card_png: Option<String>,
    import_route: String,
    regex_allowed: bool,
    user_name: String,
    opening_index: Nullable<u64>,
    messages: Vec<RawMessage>,
    world_info: Box<RawValue>,
    mvu: Nullable<RawMvu>,
    card_storage: serde_json::Map<String, Value>,
    exported_at: String,
}

#[derive(Deserialize)]
struct RawMessage {
    id: String,
    role: String,
    text: String,
    #[serde(default, deserialize_with = "present")]
    raw: Option<String>,
    ts: String,
    #[serde(default)]
    opening: bool,
    #[serde(default)]
    interrupted: bool,
    #[serde(default, deserialize_with = "present")]
    message_vars: Option<Box<RawValue>>,
}

#[derive(Deserialize)]
struct RawMvu {
    macros: Nullable<RawMacros>,
    seed: Nullable<Box<RawValue>>,
    layers: RawLayers,
}

#[derive(Deserialize)]
struct RawMacros {
    user: String,
    char: Nullable<String>,
}

#[derive(Deserialize)]
struct RawLayers {
    chat: Box<RawValue>,
    character: Box<RawValue>,
    global: Box<RawValue>,
    preset: Box<RawValue>,
    script: BTreeMap<String, Box<RawValue>>,
    extension: BTreeMap<String, Box<RawValue>>,
}

#[derive(Deserialize)]
struct RawWorldInfo {
    entries: Vec<RawWorldInfoEntry>,
    timed: serde_json::Map<String, Value>,
    last_message_id: Nullable<String>,
    message_effects: serde_json::Map<String, Value>,
}

#[derive(Deserialize)]
struct RawWorldInfoEntry {
    id: String,
    key: String,
}

/// 存檔裡的變數表：只量值（緊湊寫法），與前端 `tableProblem` 同一套；存檔原文的縮排不影響收不收。
fn table(raw: &RawValue, field: &str) -> DataResult<Json> {
    let value = parse_json(raw.get()).map_err(|_| invalid(format!("{field}: invalid-json")))?;
    validate_table(&value).map_err(|limit| invalid(format!("{field}: {}", limit.code)))?;
    Ok(value)
}

fn check_id(id: &str, field: &str, max: usize) -> DataResult<()> {
    let chars = id.chars().count();
    if chars == 0 || chars > max {
        return Err(invalid(format!("{field}: 長度要 1～{max} 字元")));
    }
    Ok(())
}

/// RFC 3339：日期、`T`、時分（秒與小數可省）、`Z` 或 ±hh:mm，而且日曆上真的存在（13 月、2 月 30 日、
/// 25 點、60 秒、偏移 24 小時都不收）。前端 web-save.ts 的 `rfc3339Millis` 同一套規則。
fn is_rfc3339(text: &str) -> bool {
    static PATTERN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let pattern = PATTERN.get_or_init(|| {
        regex::Regex::new(
            r"^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})(?::(\d{2})(?:\.\d{1,9})?)?(?:Z|[+-](\d{2}):(\d{2}))$",
        )
        .expect("固定的正則")
    });
    let Some(caps) = pattern.captures(text) else {
        return false;
    };
    let number = |index: usize| {
        caps.get(index)
            .map_or(0, |found| found.as_str().parse::<u32>().unwrap_or(u32::MAX))
    };
    let (year, month, day) = (number(1), number(2), number(3));
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
        && number(4) <= 23
        && number(5) <= 59
        && number(6) <= 59
        && number(7) <= 23
        && number(8) <= 59
}

/// 整份讀進來並檢查。版號不認得回 `WebSaveVersion`，其餘不合契約回 `WebSaveInvalid`。
pub fn parse(bytes: &[u8]) -> DataResult<WebSave> {
    if bytes.len() > MAX_SAVE_BYTES {
        return Err(invalid(format!(
            "檔案超過 {} MiB",
            MAX_SAVE_BYTES / 1024 / 1024
        )));
    }
    let head: Head =
        serde_json::from_slice(bytes).map_err(|error| invalid(format!("JSON: {error}")))?;
    if head.format.as_ref().and_then(Value::as_str) != Some(FORMAT) {
        return Err(invalid("format"));
    }
    match &head.version {
        Some(Value::Number(number)) if number.as_f64() == Some(VERSION as f64) => {}
        Some(other) => {
            return Err(UiMsg::WebSaveVersion {
                version: other.to_string(),
            }
            .into_error())
        }
        None => return Err(invalid("version")),
    }
    let raw: RawSave =
        serde_json::from_slice(bytes).map_err(|error| invalid(format!("{error}")))?;
    if !is_rfc3339(&raw.exported_at) {
        return Err(invalid("exported_at"));
    }
    let card_text = raw.card.get().to_owned();
    let card: Value = serde_json::from_str(&card_text).map_err(|_| invalid("card"))?;
    if !card.is_object() {
        return Err(invalid("card"));
    }
    let card_png = match raw.card_png {
        Some(text) => Some(
            base64::engine::general_purpose::STANDARD
                .decode(text.as_bytes())
                .map_err(|_| invalid("card_png"))?,
        ),
        None => None,
    };
    let route = match raw.import_route.as_str() {
        "character" => Route::Character,
        "worldbook" => Route::Worldbook,
        _ => return Err(invalid("import_route")),
    };
    if raw.user_name.contains(['\n', '\r']) || raw.user_name.chars().count() > MAX_USER_NAME_CHARS {
        return Err(invalid("user_name"));
    }

    let messages = parse_messages(raw.messages)?;
    let world_info = parse_world_info(raw.world_info, &messages)?;
    let mvu = match raw.mvu.0 {
        Some(mvu) => Some(parse_mvu(mvu)?),
        None => None,
    };
    let has_seed = mvu.as_ref().is_some_and(|mvu| mvu.seed.is_some());
    if !has_seed && messages.iter().any(|message| message.table.is_some()) {
        return Err(invalid("messages[].message_vars 需要 mvu.seed"));
    }
    if raw.card_storage.values().any(|value| !value.is_string())
        || serde_json::to_vec(&raw.card_storage)?.len() > MAX_CARD_STORAGE_BYTES
    {
        return Err(invalid("card_storage"));
    }
    Ok(WebSave {
        card,
        card_text,
        card_png,
        route,
        regex_allowed: raw.regex_allowed,
        user_name: raw.user_name,
        opening_index: raw.opening_index.0,
        messages,
        world_info,
        mvu,
        card_storage: raw.card_storage,
    })
}

fn parse_messages(raw: Vec<RawMessage>) -> DataResult<Vec<Message>> {
    if raw.len() > MAX_MESSAGES {
        return Err(invalid("messages: 太多則"));
    }
    let mut seen = HashSet::new();
    let mut messages = Vec::with_capacity(raw.len());
    for (index, message) in raw.into_iter().enumerate() {
        let at = |field: &str| format!("messages[{index}].{field}");
        check_id(&message.id, &at("id"), MAX_ID_CHARS)?;
        if !seen.insert(message.id.clone()) {
            return Err(invalid(at("id 重複")));
        }
        let role = match message.role.as_str() {
            "user" => Role::User,
            "char" => Role::Char,
            _ => return Err(invalid(at("role"))),
        };
        let too_long = |text: &str| text.len() > MAX_TEXT_BYTES;
        if too_long(&message.text) || message.raw.as_deref().is_some_and(too_long) {
            return Err(invalid(at("text")));
        }
        if !is_rfc3339(&message.ts) {
            return Err(invalid(at("ts")));
        }
        if message.opening && (index != 0 || role != Role::Char) {
            return Err(invalid(at("opening")));
        }
        let table = match &message.message_vars {
            Some(raw) => Some(table(raw, &at("message_vars"))?),
            None => None,
        };
        messages.push(Message {
            id: message.id,
            role,
            text: message.text,
            raw: message.raw,
            ts: message.ts,
            opening: message.opening,
            interrupted: message.interrupted,
            table,
        });
    }
    Ok(messages)
}

fn parse_world_info(raw: Box<RawValue>, messages: &[Message]) -> DataResult<WorldInfo> {
    let parsed: RawWorldInfo =
        serde_json::from_str(raw.get()).map_err(|error| invalid(format!("world_info: {error}")))?;
    let mut seen = HashSet::new();
    let mut entries = Vec::with_capacity(parsed.entries.len());
    for entry in parsed.entries {
        check_id(&entry.id, "world_info.entries[].id", MAX_ID_CHARS)?;
        if !seen.insert(entry.id.clone()) {
            return Err(invalid("world_info.entries[].id 重複"));
        }
        entries.push((entry.id, entry.key));
    }
    if let Some(last) = &parsed.last_message_id.0 {
        if !messages.iter().any(|message| &message.id == last) {
            return Err(invalid("world_info.last_message_id"));
        }
    }
    check_timed(&parsed.timed, &seen)?;
    let _ = parsed.message_effects;
    Ok(WorldInfo { raw, entries })
}

/// `timed`：只有 sticky／cooldown 兩張表，鍵是 entries 裡的穩定 ID，值是 `{start, end, protected}`
/// （start／end 為 0–2^53−1 的整數）。
fn check_timed(timed: &serde_json::Map<String, Value>, ids: &HashSet<String>) -> DataResult<()> {
    const MAX_SAFE: u64 = (1 << 53) - 1;
    let count = |value: Option<&Value>| {
        value.is_some_and(|value| match value.as_u64() {
            Some(n) => n <= MAX_SAFE,
            None => value
                .as_f64()
                .is_some_and(|f| f.fract() == 0.0 && f >= 0.0 && f <= MAX_SAFE as f64),
        })
    };
    for (kind, table) in timed {
        let Some(table) = table
            .as_object()
            .filter(|_| kind == "sticky" || kind == "cooldown")
        else {
            return Err(invalid("world_info.timed"));
        };
        for (id, effect) in table {
            if !ids.contains(id) {
                return Err(invalid(format!(
                    "world_info.timed.{kind} 的條目不在 entries"
                )));
            }
            let ok = effect.as_object().is_some_and(|e| {
                e.keys()
                    .all(|key| matches!(key.as_str(), "start" | "end" | "protected"))
                    && count(e.get("start"))
                    && count(e.get("end"))
                    && e.get("protected").is_some_and(Value::is_boolean)
            });
            if !ok {
                return Err(invalid(format!("world_info.timed.{kind}")));
            }
        }
    }
    Ok(())
}

fn parse_mvu(raw: RawMvu) -> DataResult<Mvu> {
    let layers = raw.layers;
    let keyed =
        |map: BTreeMap<String, Box<RawValue>>, field: &str| -> DataResult<Vec<(String, Json)>> {
            map.into_iter()
                .map(|(id, value)| {
                    check_id(&id, &format!("mvu.layers.{field}"), MAX_LAYER_ID_CHARS)?;
                    let parsed = table(&value, &format!("mvu.layers.{field}.{id}"))?;
                    Ok((id, parsed))
                })
                .collect()
        };
    let macros = match raw.macros.0 {
        Some(macros) => {
            if macros.user.contains(['\n', '\r'])
                || macros
                    .char
                    .0
                    .as_deref()
                    .is_some_and(|name| name.contains(['\n', '\r']))
            {
                return Err(invalid("mvu.macros"));
            }
            Some(Macros {
                user: macros.user,
                char: macros.char.0,
            })
        }
        None => None,
    };
    Ok(Mvu {
        macros,
        seed: match &raw.seed.0 {
            Some(seed) => Some(table(seed, "mvu.seed")?),
            None => None,
        },
        layers: Layers {
            chat: table(&layers.chat, "mvu.layers.chat")?,
            character: table(&layers.character, "mvu.layers.character")?,
            global: table(&layers.global, "mvu.layers.global")?,
            preset: table(&layers.preset, "mvu.layers.preset")?,
            script: keyed(layers.script, "script")?,
            extension: keyed(layers.extension, "extension")?,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::is_rfc3339;

    /// 與前端 web-save.test.ts 的 `rfc3339Millis` 同一組。
    #[test]
    fn times_must_exist_on_the_calendar() {
        for good in [
            "2026-10-07T21:00:00Z",
            "2026-10-07T21:00:00+08:00",
            "2026-10-07T21:00-01:30",
            "2026-10-07T21:00:00.123456789Z",
            "2028-02-29T00:00:00Z",
            "0050-01-01T00:00:00Z",
        ] {
            assert!(is_rfc3339(good), "{good}");
        }
        for bad in [
            "2026-99-99T99:99:99Z",
            "2026-02-30T00:00:00Z",
            "2027-02-29T00:00:00Z",
            "2100-02-29T00:00:00Z",
            "2026-00-10T00:00:00Z",
            "2026-10-07T24:00:00Z",
            "2026-10-07T21:60:00Z",
            "2026-10-07T21:00:60Z",
            "2026-10-07T21:00:00+24:00",
            "2026-10-07T21:00:00+08:60",
            "2026-10-07 21:00:00Z",
        ] {
            assert!(!is_rfc3339(bad), "{bad}");
        }
    }
}
