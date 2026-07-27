use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

#[cfg(unix)]
#[repr(C)]
struct LocalTime {
    tm_sec: std::os::raw::c_int,
    tm_min: std::os::raw::c_int,
    tm_hour: std::os::raw::c_int,
    tm_mday: std::os::raw::c_int,
    tm_mon: std::os::raw::c_int,
    tm_year: std::os::raw::c_int,
    tm_wday: std::os::raw::c_int,
    tm_yday: std::os::raw::c_int,
    tm_isdst: std::os::raw::c_int,
    tm_gmtoff: std::os::raw::c_long,
    tm_zone: *const std::os::raw::c_char,
}

#[cfg(unix)]
unsafe extern "C" {
    fn localtime_r(timestamp: *const i64, result: *mut LocalTime) -> *mut LocalTime;
}

pub type DataResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub fn local_timestamp() -> DataResult<String> {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map_err(|error| invalid_data(format!("system clock is before the Unix epoch: {error}")))?
        .as_secs() as i64;

    #[cfg(unix)]
    {
        let mut local = std::mem::MaybeUninit::<LocalTime>::uninit();
        // localtime_r writes the supplied storage and has no shared mutable state.
        if unsafe { localtime_r(&seconds, local.as_mut_ptr()) }.is_null() {
            return Err(invalid_data("could not convert local time"));
        }
        let local = unsafe { local.assume_init() };
        return Ok(format!(
            "{:04}-{:02}-{:02} {:02}:{:02}",
            local.tm_year + 1900,
            local.tm_mon + 1,
            local.tm_mday,
            local.tm_hour,
            local.tm_min
        ));
    }

    #[cfg(not(unix))]
    {
        // Tauri's supported Unix targets use localtime_r above. Keep a dependency-free fallback
        // for other targets; its value is UTC when no platform local-time API is available.
        let minutes = seconds / 60;
        Ok(format!(
            "1970-01-01 {:02}:{:02}",
            (minutes / 60) % 24,
            minutes % 60
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Best,
    Balanced,
    Fast,
    Default,
}

impl Tier {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Best => "best",
            Self::Balanced => "balanced",
            Self::Fast => "fast",
            Self::Default => "default",
        }
    }

    pub(crate) fn parse(value: &str) -> DataResult<Self> {
        match value {
            "best" => Ok(Self::Best),
            "balanced" => Ok(Self::Balanced),
            "fast" => Ok(Self::Fast),
            "default" => Ok(Self::Default),
            _ => Err(invalid_data(format!("invalid tier: {value}"))),
        }
    }
}

// 匯入卡附原 PNG 時的顯示開關（NewPlan §5.2）；舊卡與手建卡缺此欄一律視為 true
fn default_show_image() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterMeta {
    pub name: String,
    pub color: String,
    pub avatar: String,
    pub tier: Tier,
    #[serde(default = "default_show_image")]
    pub show_image: bool,
    #[serde(default)]
    pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterCard {
    pub name: String,
    pub color: String,
    pub avatar: String,
    pub tier: Tier,
    #[serde(default = "default_show_image")]
    pub show_image: bool,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub gen_prompt: String,
    pub public_md: String,
    pub private_md: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptKind {
    Dialogue,
    Narration,
    Player,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranscriptEvent {
    pub ts: String,
    pub speaker: String,
    pub kind: TranscriptKind,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "characters", rename_all = "lowercase")]
pub enum Visibility {
    Gm,
    Public,
    Characters(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldbookEntry {
    pub uid: u64,
    pub title: String,
    pub keys: Vec<String>,
    pub content: String,
    pub constant: bool,
    pub order: i64,
    pub disabled: bool,
    pub visibility: Visibility,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldState {
    #[serde(default)]
    pub model_bindings: BTreeMap<String, String>,
    #[serde(default)]
    pub current_scene: u64,
    #[serde(default)]
    pub catchup_summaries: BTreeMap<String, String>,
    // 換幕順手取的幕名：key 是場景號字串（比照 catchup_summaries），沒取到就不進這個表
    #[serde(default)]
    pub scene_titles: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub api_keys: BTreeMap<String, String>,
    #[serde(default)]
    pub tier_models: BTreeMap<String, String>,
    #[serde(default)]
    pub preferences: serde_json::Map<String, serde_json::Value>,
}

pub(crate) fn invalid_data(message: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message.into()).into()
}

pub(crate) fn validate_name(name: &str) -> DataResult<()> {
    if name.is_empty()
        || name.starts_with('.')
        || name.contains("..")
        || name.contains('/')
        || name.contains('\\')
        || name.chars().any(char::is_control)
    {
        return Err(invalid_data(format!("invalid name: {name:?}")));
    }
    Ok(())
}

fn worlds_dir(root: &Path) -> PathBuf {
    root.join("worlds")
}

fn world_dir(root: &Path, world: &str) -> DataResult<PathBuf> {
    validate_name(world)?;
    Ok(worlds_dir(root).join(world))
}

pub(crate) fn character_path(root: &Path, world: &str, name: &str) -> DataResult<PathBuf> {
    validate_name(name)?;
    Ok(world_dir(root, world)?
        .join("characters")
        .join(format!("{name}.md")))
}

/// 最後活動時間＝transcript 內最新檔案 mtime，退而求其次用世界目錄 mtime
fn last_active(world_directory: &Path) -> std::time::SystemTime {
    let mut latest = fs::metadata(world_directory)
        .and_then(|meta| meta.modified())
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
    if let Ok(entries) = fs::read_dir(world_directory.join("transcript")) {
        for entry in entries.flatten() {
            if let Ok(modified) = entry.metadata().and_then(|meta| meta.modified()) {
                latest = latest.max(modified);
            }
        }
    }
    latest
}

/// 依最後活動排序（新的在前），供側欄桌列表用（NewPlan §9.3）
pub fn list_worlds(root: &Path) -> DataResult<Vec<String>> {
    let directory = worlds_dir(root);
    if !directory.exists() {
        return Ok(Vec::new());
    }

    let mut worlds = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| invalid_data("world directory name is not valid UTF-8"))?;
            worlds.push((last_active(&entry.path()), name));
        }
    }
    worlds.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    Ok(worlds.into_iter().map(|(_, name)| name).collect())
}

pub fn create_world(root: &Path, name: &str) -> DataResult<()> {
    validate_name(name)?;
    let directory = worlds_dir(root).join(name);
    fs::create_dir_all(worlds_dir(root))?;
    fs::create_dir(&directory)?;
    fs::create_dir(directory.join("characters"))?;
    fs::create_dir(directory.join("transcript"))?;
    fs::write(directory.join("world.md"), "")?;
    fs::write(
        directory.join("state.json"),
        serde_json::to_string_pretty(&WorldState::default())?,
    )?;
    Ok(())
}

/// 範例桌內容依語系產生（首開先選語言再建桌）；lang 非 en 一律走 zh-TW
pub fn create_sample_world(root: &Path, lang: &str) -> DataResult<String> {
    let english = lang == "en";
    let world = if english {
        "The Misty Tavern (sample)"
    } else {
        "迷霧酒館（範例）"
    };
    // 冪等：範例桌已在就直接沿用，避免重複呼叫（dev 的 StrictMode 雙跑）噴 File exists
    if world_dir(root, world)?.exists() {
        return Ok(world.to_owned());
    }
    create_world(root, world)?;
    write_world_md(
        root,
        world,
        if english {
            "# The Misty Tavern\n\nAn inn-tavern in the border town of Mistmouth; a storm rages outside tonight. A werewolf rumor has spread through town: for three nights running, livestock has gone missing.\n\n## Truths only the GM knows\n- The werewolf is Mayor Glenn — bitten in the mountains half a year ago, he himself doesn't fully know what happens at night.\n- Fox, the tavern keeper, is actually a fugitive from the neighboring kingdom, and a bounty hunter is on the way.\n- Later tonight the mayor will push through the door soaked to the bone, blood on his cuff.\n\n## Directing notes\n- Pacing: slow burn — let the characters feel each other out first.\n- Keep the narration suspenseful; don't reveal the truth too quickly."
        } else {
            "# 迷霧酒館\n\n邊境小鎮「霧口鎮」的一間旅店酒館，今晚外頭下著暴雨。鎮上最近流傳狼人傳說：三天前開始，每晚都有牲口失蹤。\n\n## 只有 GM 知道的真相\n- 狼人是鎮長葛倫——他半年前在山裡被咬傷，自己也不完全清楚夜裡發生的事。\n- 酒館老闆狐狸其實是鄰國的通緝犯，賞金獵人正在路上。\n- 今晚稍後，渾身濕透的鎮長會推門進來，袖口沾著血。\n\n## 導演方針\n- 步調：慢熱，讓角色先互相試探。\n- 旁白保持懸疑，不要太快揭露真相。"
        },
    )?;

    let texts: [(&str, &str, &str); 3] = if english {
        [
            (
                "Fox",
                "Keeper of the inn-tavern — all smiles and smooth talk, with an ear for everything that happens in town. Speaks with a streetwise charm and is great at defusing tension.",
                "Real name \"Ali\", a fugitive from the neighboring kingdom — took the fall for someone three years ago and fled here. Words like \"bounty\" or \"wanted\" make them quietly tense. Goal: live in peace, but stay ready to run.",
            ),
            (
                "Knight",
                "A young knight on patrol, upright to the point of stubbornness; drinks hot tea in the tavern, never ale.",
                "The real mission is to track a fugitive who fled three years ago; the portrait in hand has long since faded. Quietly observes everyone in the tavern. Principle: verify first, act second — never wrong the innocent.",
            ),
            (
                "Bard",
                "A wandering bard who cadges drinks and spins outrageous tales and songs. Remarkably well-informed.",
                "Nine parts of every story are true, one part false — the bard really did see \"that wolf\" walk on two legs in the next town. Too frightened to tell it straight, they only dared weave it into a song.",
            ),
        ]
    } else {
        [
            (
                "狐狸",
                "旅店酒館的老闆，笑口常開、八面玲瓏，對鎮上大小事瞭若指掌。說話帶點江湖氣，擅長打圓場。",
                "真名「阿狸」，是鄰國的通緝犯——三年前替人頂罪後逃亡至此。聽到「賞金」「通緝」等字眼會不動聲色地緊張。目標：安穩活下去，必要時準備隨時跑路。",
            ),
            (
                "騎士",
                "巡邏至此的年輕騎士，個性正直到近乎固執，在酒館喝的是熱茶不是酒。",
                "此行真正任務是追查一名逃亡三年的通緝犯，手上的畫像已經模糊。暗中觀察酒館裡的每個人。原則：先確認再行動，不冤枉好人。",
            ),
            (
                "吟遊詩人",
                "雲遊四方的吟遊詩人，愛蹭酒喝，滿嘴誇張的故事與歌謠。消息靈通。",
                "他的故事九分真一分假——他真的在鄰鎮親眼看過「那頭狼」用兩條腿走路。因為太害怕沒敢說全，只敢把它編進歌裡。",
            ),
        ]
    };
    let style = [
        ("#e07a5f", "🦊", Tier::Default),
        ("#3d84a8", "🛡️", Tier::Default),
        ("#f2a541", "🪕", Tier::Fast),
    ];
    for ((name, public_md, private_md), (color, avatar, tier)) in texts.into_iter().zip(style) {
        write_character(
            root,
            world,
            &CharacterCard {
                name: name.to_owned(),
                color: color.to_owned(),
                avatar: avatar.to_owned(),
                tier,
                show_image: true,
                archived: false,
                gen_prompt: String::new(),
                public_md: public_md.to_owned(),
                private_md: private_md.to_owned(),
            },
        )?;
    }

    append_transcript(
        root,
        world,
        0,
        &TranscriptEvent {
            ts: "2026-07-20T00:00:00+08:00".to_owned(),
            speaker: "GM".to_owned(),
            kind: TranscriptKind::Narration,
            text: if english {
                "Rain hammers the windows of the Misty Tavern; the hearth crackles. Few guests tonight — the keeper polishes glasses behind the bar, a knight sips tea in the corner, and the bard is tuning their strings. Outside, faint through the storm, comes a wolf's howl."
            } else {
                "暴雨拍打著迷霧酒館的窗，爐火劈啪作響。今晚店裡客人不多——老闆在吧檯後擦著杯子，一名騎士坐在角落喝茶，吟遊詩人正調著琴弦。門外，隱約傳來一聲狼嚎。"
            }
            .to_owned(),
        },
    )?;

    Ok(world.to_owned())
}

/// 空桌回收（NewPlan §9.3）：只回收完全未動過的桌——零訊息、零角色、world.md 空白；
/// 任一項有內容即保留，防資料遺失。回傳是否真的刪了。
pub fn reclaim_world_if_empty(root: &Path, world: &str) -> DataResult<bool> {
    let directory = world_dir(root, world)?;
    if !directory.exists() {
        return Ok(false);
    }
    let has_messages = fs::read_dir(directory.join("transcript"))
        .map(|entries| {
            entries
                .flatten()
                .any(|entry| entry.metadata().map(|meta| meta.len() > 0).unwrap_or(true))
        })
        .unwrap_or(false);
    let has_characters = fs::read_dir(directory.join("characters"))
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false);
    let world_md = fs::read_to_string(directory.join("world.md")).unwrap_or_default();
    if has_messages || has_characters || !world_md.trim().is_empty() {
        return Ok(false);
    }
    fs::remove_dir_all(directory)?;
    Ok(true)
}

/// 懶命名的後半：桌名隨時可改（NewPlan §9.3）
pub fn rename_world(root: &Path, world: &str, new_name: &str) -> DataResult<()> {
    let from = world_dir(root, world)?;
    let to = world_dir(root, new_name)?;
    if world == new_name {
        return Ok(());
    }
    if to.exists() {
        return Err(invalid_data(format!("world already exists: {new_name}")));
    }
    fs::rename(from, to)?;
    Ok(())
}

pub fn read_world_md(root: &Path, world: &str) -> DataResult<String> {
    Ok(fs::read_to_string(
        world_dir(root, world)?.join("world.md"),
    )?)
}

pub fn write_world_md(root: &Path, world: &str, content: &str) -> DataResult<()> {
    fs::write(world_dir(root, world)?.join("world.md"), content)?;
    Ok(())
}

fn worldbook_path(root: &Path, world: &str) -> DataResult<PathBuf> {
    Ok(world_dir(root, world)?.join("worldbook.json"))
}

fn empty_worldbook() -> serde_json::Value {
    serde_json::json!({ "entries": {} })
}

fn read_worldbook_value(root: &Path, world: &str) -> DataResult<serde_json::Value> {
    let path = worldbook_path(root, world)?;
    if !path.exists() {
        return Ok(empty_worldbook());
    }
    let text = fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| invalid_data(format!("invalid worldbook JSON: {error}")))?;
    if !value
        .get("entries")
        .is_some_and(serde_json::Value::is_object)
    {
        return Err(invalid_data("worldbook entries must be an object"));
    }
    Ok(value)
}

fn write_worldbook_value(root: &Path, world: &str, value: &serde_json::Value) -> DataResult<()> {
    fs::write(
        worldbook_path(root, world)?,
        serde_json::to_string_pretty(value)?,
    )?;
    Ok(())
}

fn visibility_from_value(value: &serde_json::Value) -> Visibility {
    match value
        .get("extensions")
        .and_then(|value| value.get("table_tavern"))
        .and_then(|value| value.get("visibility"))
    {
        Some(serde_json::Value::String(value)) if value == "public" => Visibility::Public,
        Some(serde_json::Value::Object(value)) => value
            .get("characters")
            .and_then(serde_json::Value::as_array)
            .filter(|names| names.iter().all(serde_json::Value::is_string))
            .map(|names| {
                Visibility::Characters(
                    names
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .map(str::to_owned)
                        .collect(),
                )
            })
            .unwrap_or(Visibility::Gm),
        _ => Visibility::Gm,
    }
}

fn visibility_value(visibility: &Visibility) -> serde_json::Value {
    match visibility {
        Visibility::Gm => serde_json::Value::String("gm".to_owned()),
        Visibility::Public => serde_json::Value::String("public".to_owned()),
        Visibility::Characters(names) => serde_json::json!({ "characters": names }),
    }
}

fn set_visibility(value: &mut serde_json::Value, visibility: &Visibility) {
    let Some(entry) = value.as_object_mut() else {
        return;
    };
    let extensions = entry
        .entry("extensions")
        .or_insert_with(|| serde_json::json!({}));
    if !extensions.is_object() {
        *extensions = serde_json::json!({});
    }
    let extensions = extensions.as_object_mut().expect("object set above");
    let table_tavern = extensions
        .entry("table_tavern")
        .or_insert_with(|| serde_json::json!({}));
    if !table_tavern.is_object() {
        *table_tavern = serde_json::json!({});
    }
    table_tavern
        .as_object_mut()
        .expect("object set above")
        .insert("visibility".to_owned(), visibility_value(visibility));
}

fn entry_view(value: &serde_json::Value, fallback_uid: Option<u64>) -> WorldbookEntry {
    WorldbookEntry {
        uid: value
            .get("uid")
            .and_then(serde_json::Value::as_u64)
            .or(fallback_uid)
            .unwrap_or(0),
        title: value
            .get("comment")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        keys: value
            .get("key")
            .and_then(serde_json::Value::as_array)
            .map(|keys| {
                keys.iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        content: value
            .get("content")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        constant: value
            .get("constant")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        order: value
            .get("order")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0),
        disabled: value
            .get("disable")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        visibility: visibility_from_value(value),
    }
}

fn entries_object(
    value: &serde_json::Value,
) -> DataResult<&serde_json::Map<String, serde_json::Value>> {
    value
        .get("entries")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| invalid_data("worldbook entries must be an object"))
}

fn entries_object_mut(
    value: &mut serde_json::Value,
) -> DataResult<&mut serde_json::Map<String, serde_json::Value>> {
    value
        .get_mut("entries")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or_else(|| invalid_data("worldbook entries must be an object"))
}

fn entry_uid(key: &str, value: &serde_json::Value) -> Option<u64> {
    value
        .get("uid")
        .and_then(serde_json::Value::as_u64)
        .or_else(|| key.parse().ok())
}

fn max_uid(entries: &serde_json::Map<String, serde_json::Value>) -> Option<u64> {
    entries
        .iter()
        .filter_map(|(key, value)| entry_uid(key, value))
        .max()
}

fn next_uid(entries: &serde_json::Map<String, serde_json::Value>) -> DataResult<u64> {
    max_uid(entries)
        .map(|uid| {
            uid.checked_add(1)
                .ok_or_else(|| invalid_data("worldbook uid overflow"))
        })
        .unwrap_or(Ok(0))
}

fn sorted_entry_keys(entries: &serde_json::Map<String, serde_json::Value>) -> Vec<String> {
    let mut keys: Vec<_> = entries.keys().cloned().collect();
    keys.sort_by_key(|key| {
        let value = &entries[key];
        (
            value
                .get("displayIndex")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(u64::MAX),
            entry_uid(key, value).unwrap_or(0),
        )
    });
    keys
}

fn set_display_index(value: &mut serde_json::Value, display_index: u64) -> DataResult<()> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| invalid_data("worldbook entry must be an object"))?;
    object.insert("displayIndex".to_owned(), serde_json::json!(display_index));
    Ok(())
}

fn normalize_display_indices(
    entries: &mut serde_json::Map<String, serde_json::Value>,
    keys: &[String],
) -> DataResult<()> {
    for (index, key) in keys.iter().enumerate() {
        let display_index =
            u64::try_from(index).map_err(|_| invalid_data("worldbook displayIndex overflow"))?;
        let value = entries
            .get_mut(key)
            .ok_or_else(|| invalid_data("worldbook entry disappeared"))?;
        set_display_index(value, display_index)?;
    }
    Ok(())
}

fn update_entry_fields(value: &mut serde_json::Value, entry: &WorldbookEntry) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    object.insert("key".to_owned(), serde_json::json!(entry.keys));
    object.insert(
        "comment".to_owned(),
        serde_json::Value::String(entry.title.clone()),
    );
    object.insert(
        "content".to_owned(),
        serde_json::Value::String(entry.content.clone()),
    );
    object.insert(
        "constant".to_owned(),
        serde_json::Value::Bool(entry.constant),
    );
    object.insert("order".to_owned(), serde_json::json!(entry.order));
    object.insert(
        "disable".to_owned(),
        serde_json::Value::Bool(entry.disabled),
    );
    set_visibility(value, &entry.visibility);
}

fn new_entry_value(entry: &WorldbookEntry, uid: u64, display_index: u64) -> serde_json::Value {
    let mut value = serde_json::json!({
        "uid": uid,
        "key": entry.keys,
        "keysecondary": [],
        "comment": entry.title,
        "content": entry.content,
        "constant": entry.constant,
        "vectorized": false,
        "selective": true,
        "selectiveLogic": 0,
        "addMemo": true,
        "order": entry.order,
        "position": 0,
        "disable": entry.disabled,
        "excludeRecursion": false,
        "preventRecursion": false,
        "delayUntilRecursion": false,
        "probability": 100,
        "useProbability": true,
        "depth": 4,
        "group": "",
        "groupOverride": false,
        "groupWeight": 100,
        "scanDepth": null,
        "caseSensitive": null,
        "matchWholeWords": null,
        "useGroupScoring": null,
        "automationId": "",
        "role": null,
        "sticky": 0,
        "cooldown": 0,
        "delay": 0,
        "displayIndex": display_index
    });
    set_visibility(&mut value, &entry.visibility);
    value
}

pub fn read_worldbook(root: &Path, world: &str) -> DataResult<Vec<WorldbookEntry>> {
    let value = read_worldbook_value(root, world)?;
    let mut entries: Vec<_> = entries_object(&value)?
        .iter()
        .map(|(key, value)| {
            let entry = entry_view(value, key.parse().ok());
            (
                value
                    .get("displayIndex")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(u64::MAX),
                entry.uid,
                entry,
            )
        })
        .collect();
    entries.sort_by_key(|(display_index, uid, _)| (*display_index, *uid));
    Ok(entries.into_iter().map(|(_, _, entry)| entry).collect())
}

pub fn upsert_worldbook_entry(root: &Path, world: &str, entry: WorldbookEntry) -> DataResult<u64> {
    let mut worldbook = read_worldbook_value(root, world)?;
    let entries = entries_object_mut(&mut worldbook)?;
    let existing_key = entries
        .iter()
        .find(|(key, value)| entry_uid(key, value) == Some(entry.uid))
        .map(|(key, _)| key.clone());
    let actual_uid = if let Some(key) = existing_key {
        let value = entries
            .get_mut(&key)
            .ok_or_else(|| invalid_data("worldbook entry disappeared"))?;
        if !value.is_object() {
            return Err(invalid_data("worldbook entry must be an object"));
        }
        update_entry_fields(value, &entry);
        entry.uid
    } else {
        let uid = next_uid(entries)?;
        let keys = sorted_entry_keys(entries);
        let has_missing_display_index = entries.values().any(|value| {
            value
                .get("displayIndex")
                .and_then(serde_json::Value::as_u64)
                .is_none()
        });
        if has_missing_display_index {
            normalize_display_indices(entries, &keys)?;
        }
        for key in keys {
            let value = entries
                .get_mut(&key)
                .ok_or_else(|| invalid_data("worldbook entry disappeared"))?;
            let display_index = value
                .get("displayIndex")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| invalid_data("worldbook displayIndex missing"))?
                .checked_add(1)
                .ok_or_else(|| invalid_data("worldbook displayIndex overflow"))?;
            set_display_index(value, display_index)?;
        }
        entries.insert(uid.to_string(), new_entry_value(&entry, uid, 0));
        uid
    };
    write_worldbook_value(root, world, &worldbook)?;
    Ok(actual_uid)
}

pub fn move_worldbook_entry(root: &Path, world: &str, uid: u64, up: bool) -> DataResult<()> {
    let mut worldbook = read_worldbook_value(root, world)?;
    let entries = entries_object_mut(&mut worldbook)?;
    let keys = sorted_entry_keys(entries);
    let index = keys
        .iter()
        .position(|key| entry_uid(key, &entries[key]) == Some(uid))
        .ok_or_else(|| invalid_data("worldbook entry not found"))?;
    let adjacent = if up {
        let Some(adjacent) = index.checked_sub(1) else {
            return Ok(());
        };
        adjacent
    } else {
        let adjacent = index + 1;
        if adjacent >= keys.len() {
            return Ok(());
        }
        adjacent
    };

    if entries.values().any(|value| {
        value
            .get("displayIndex")
            .and_then(serde_json::Value::as_u64)
            .is_none()
    }) {
        // 舊檔先依目前顯示順序補齊索引，避免第一次移動時跳位。
        normalize_display_indices(entries, &keys)?;
    }

    let current_index = entries[&keys[index]]
        .get("displayIndex")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| invalid_data("worldbook displayIndex missing"))?;
    let adjacent_index = entries[&keys[adjacent]]
        .get("displayIndex")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| invalid_data("worldbook displayIndex missing"))?;
    set_display_index(
        entries
            .get_mut(&keys[index])
            .ok_or_else(|| invalid_data("worldbook entry disappeared"))?,
        adjacent_index,
    )?;
    set_display_index(
        entries
            .get_mut(&keys[adjacent])
            .ok_or_else(|| invalid_data("worldbook entry disappeared"))?,
        current_index,
    )?;
    write_worldbook_value(root, world, &worldbook)
}

pub fn delete_worldbook_entry(root: &Path, world: &str, uid: u64) -> DataResult<()> {
    let path = worldbook_path(root, world)?;
    if !path.exists() {
        return Ok(());
    }
    let mut worldbook = read_worldbook_value(root, world)?;
    let entries = entries_object_mut(&mut worldbook)?;
    let key = entries
        .iter()
        .find(|(key, value)| entry_uid(key, value) == Some(uid))
        .map(|(key, _)| key.clone());
    if let Some(key) = key {
        entries.remove(&key);
        write_worldbook_value(root, world, &worldbook)?;
    }
    Ok(())
}

fn normalize_imported_entry(
    mut value: serde_json::Value,
    character_book: bool,
    uid: u64,
) -> DataResult<serde_json::Value> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| invalid_data("worldbook entry must be an object"))?;
    if character_book {
        if let Some(keys) = object.remove("keys") {
            object.insert("key".to_owned(), keys);
        }
        if let Some(keys) = object.remove("secondary_keys") {
            object.insert("keysecondary".to_owned(), keys);
        }
        if let Some(order) = object.remove("insertion_order") {
            object.insert("order".to_owned(), order);
        }
        if let Some(enabled) = object.remove("enabled") {
            let enabled = enabled
                .as_bool()
                .ok_or_else(|| invalid_data("character_book enabled must be a boolean"))?;
            object.insert("disable".to_owned(), serde_json::Value::Bool(!enabled));
        }
    }
    object.insert("uid".to_owned(), serde_json::json!(uid));
    let has_visibility = value
        .get("extensions")
        .and_then(|value| value.get("table_tavern"))
        .and_then(|value| value.get("visibility"))
        .is_some();
    if !has_visibility {
        set_visibility(&mut value, &Visibility::Gm);
    }
    Ok(value)
}

pub fn import_worldbook(root: &Path, world: &str, json_text: &str) -> DataResult<usize> {
    let imported: serde_json::Value = serde_json::from_str(json_text)
        .map_err(|error| invalid_data(format!("invalid worldbook JSON: {error}")))?;
    let source = imported
        .get("entries")
        .ok_or_else(|| invalid_data("imported worldbook is missing entries"))?;
    let (source_entries, character_book): (Vec<serde_json::Value>, bool) = match source {
        serde_json::Value::Object(entries) => (entries.values().cloned().collect(), false),
        serde_json::Value::Array(entries) => (entries.clone(), true),
        _ => {
            return Err(invalid_data(
                "imported worldbook entries must be an object or array",
            ));
        }
    };

    let mut worldbook = read_worldbook_value(root, world)?;
    let entries = entries_object_mut(&mut worldbook)?;
    let imported_count = source_entries.len();
    let mut uid = next_uid(entries)?;
    for source_entry in source_entries {
        let entry = normalize_imported_entry(source_entry, character_book, uid)?;
        entries.insert(uid.to_string(), entry);
        uid = uid
            .checked_add(1)
            .ok_or_else(|| invalid_data("worldbook uid overflow"))?;
    }
    write_worldbook_value(root, world, &worldbook)?;
    Ok(imported_count)
}

pub fn export_worldbook(root: &Path, world: &str, path: &Path) -> DataResult<()> {
    let source = worldbook_path(root, world)?;
    if source.exists() {
        fs::copy(source, path)?;
    } else {
        fs::write(path, serde_json::to_string_pretty(&empty_worldbook())?)?;
    }
    Ok(())
}

fn parse_frontmatter(contents: &str) -> DataResult<(CharacterMeta, String, &str)> {
    let rest = contents
        .strip_prefix("---\n")
        .ok_or_else(|| invalid_data("character card must start with frontmatter"))?;
    let end = rest
        .find("\n---\n")
        .ok_or_else(|| invalid_data("character card frontmatter is not closed"))?;
    let frontmatter = &rest[..end];
    let body = &rest[end + "\n---\n".len()..];

    let mut name = None;
    let mut color = None;
    let mut avatar = None;
    let mut tier = None;
    let mut show_image = true;
    let mut archived = false;
    let mut gen_prompt = String::new();
    for line in frontmatter.lines() {
        let Some((key, value)) = line.split_once(':') else {
            if line.trim().is_empty() {
                continue;
            }
            return Err(invalid_data(format!("invalid frontmatter line: {line}")));
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "name" => name = Some(value.to_owned()),
            "color" => color = Some(value.to_owned()),
            "avatar" => avatar = Some(value.to_owned()),
            "tier" => tier = Some(Tier::parse(value)?),
            "show_image" => show_image = value != "false",
            "archived" => archived = value == "true",
            "gen_prompt" => gen_prompt = value.to_owned(),
            _ => {}
        }
    }

    let name = name.ok_or_else(|| invalid_data("frontmatter is missing name"))?;
    validate_name(&name)?;
    Ok((
        CharacterMeta {
            name,
            color: color.ok_or_else(|| invalid_data("frontmatter is missing color"))?,
            avatar: avatar.ok_or_else(|| invalid_data("frontmatter is missing avatar"))?,
            tier: tier.ok_or_else(|| invalid_data("frontmatter is missing tier"))?,
            show_image,
            archived,
        },
        gen_prompt,
        body,
    ))
}

fn parse_sections(body: &str) -> (String, String) {
    #[derive(Clone, Copy)]
    enum Section {
        Public,
        Private,
    }

    let mut markers = Vec::new();
    let mut offset = 0;
    for segment in body.split_inclusive('\n') {
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        let line = line.strip_suffix('\r').unwrap_or(line);
        let section = match line {
            "## 公開" => Some(Section::Public),
            "## 私有" => Some(Section::Private),
            _ => None,
        };
        if let Some(section) = section {
            markers.push((offset, offset + segment.len(), section));
        }
        offset += segment.len();
    }

    let mut public_md = String::new();
    let mut private_md = String::new();
    for (index, (_, content_start, section)) in markers.iter().copied().enumerate() {
        let content_end = markers
            .get(index + 1)
            .map(|(heading_start, _, _)| *heading_start)
            .unwrap_or(body.len());
        let mut content = &body[content_start..content_end];
        if index + 1 < markers.len() {
            content = content.strip_suffix('\n').unwrap_or(content);
        }
        match section {
            Section::Public => public_md = content.to_owned(),
            Section::Private => private_md = content.to_owned(),
        }
    }
    (public_md, private_md)
}

fn serialize_character(card: &CharacterCard) -> String {
    // frontmatter 逐行解析，生成提示詞中的換行須在寫入前攤平。
    let gen_prompt = card.gen_prompt.replace(['\n', '\r'], " ");
    format!(
        "---\nname: {}\ncolor: {}\navatar: {}\ntier: {}\nshow_image: {}\narchived: {}\ngen_prompt: {}\n---\n## 公開\n{}\n## 私有\n{}",
        card.name,
        card.color,
        card.avatar,
        card.tier.as_str(),
        card.show_image,
        card.archived,
        gen_prompt,
        card.public_md,
        card.private_md
    )
}

pub fn list_characters(root: &Path, world: &str) -> DataResult<Vec<CharacterMeta>> {
    let directory = world_dir(root, world)?.join("characters");
    if !directory.exists() {
        return Ok(Vec::new());
    }

    let mut characters = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry.path().extension().and_then(|value| value.to_str()) == Some("md")
        {
            let contents = fs::read_to_string(entry.path())?;
            characters.push(parse_frontmatter(&contents)?.0);
        }
    }
    characters.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(characters)
}

pub fn read_character(root: &Path, world: &str, name: &str) -> DataResult<CharacterCard> {
    let contents = fs::read_to_string(character_path(root, world, name)?)?;
    let (meta, gen_prompt, body) = parse_frontmatter(&contents)?;
    let (public_md, private_md) = parse_sections(body);
    Ok(CharacterCard {
        name: meta.name,
        color: meta.color,
        avatar: meta.avatar,
        tier: meta.tier,
        show_image: meta.show_image,
        archived: meta.archived,
        gen_prompt,
        public_md,
        private_md,
    })
}

fn validate_single_line(field: &str, value: &str) -> DataResult<()> {
    if value.contains('\n') || value.contains('\r') {
        return Err(invalid_data(format!("{field} must be a single line")));
    }
    Ok(())
}

pub fn write_character(root: &Path, world: &str, card: &CharacterCard) -> DataResult<()> {
    let path = character_path(root, world, &card.name)?;
    validate_single_line("color", &card.color)?;
    validate_single_line("avatar", &card.avatar)?;
    fs::write(path, serialize_character(card))?;
    Ok(())
}

pub fn set_character_archived(
    root: &Path,
    world: &str,
    name: &str,
    archived: bool,
) -> DataResult<()> {
    let mut card = read_character(root, world, name)?;
    card.archived = archived;
    write_character(root, world, &card)
}

pub fn delete_character(root: &Path, world: &str, name: &str) -> DataResult<()> {
    let path = character_path(root, world, name)?;
    fs::remove_file(&path)?;
    let image_path = path.with_extension("png");
    if image_path.exists() {
        fs::remove_file(image_path)?;
    }
    let avatar_path = path.with_extension("avatar.png");
    if avatar_path.exists() {
        fs::remove_file(avatar_path)?;
    }
    Ok(())
}

fn transcript_path(root: &Path, world: &str, scene: u64) -> DataResult<PathBuf> {
    Ok(world_dir(root, world)?
        .join("transcript")
        .join(format!("{scene}.jsonl")))
}

pub fn append_transcript(
    root: &Path,
    world: &str,
    scene: u64,
    event: &TranscriptEvent,
) -> DataResult<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(transcript_path(root, world, scene)?)?;
    serde_json::to_writer(&mut file, event)?;
    file.write_all(b"\n")?;
    Ok(())
}

pub fn read_transcript(root: &Path, world: &str, scene: u64) -> DataResult<Vec<TranscriptEvent>> {
    let path = transcript_path(root, world, scene)?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = fs::File::open(path)?;
    let mut events = Vec::new();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line_number = index + 1;
        let line = line?;
        let event = serde_json::from_str(&line).map_err(|error| {
            invalid_data(format!("invalid transcript line {line_number}: {error}"))
        })?;
        events.push(event);
    }
    Ok(events)
}

/// 把單一事件渲染成一行（或多行）Markdown，整桌／單場匯出共用同一份格式。
fn render_transcript_entry(event: &TranscriptEvent, english: bool) -> String {
    match event.kind {
        TranscriptKind::Dialogue | TranscriptKind::Player => {
            if english {
                format!("**{}**: {}", event.speaker, event.text)
            } else {
                format!("**{}**：{}", event.speaker, event.text)
            }
        }
        TranscriptKind::Narration => {
            if event.text.is_empty() {
                "> ".to_owned()
            } else {
                event
                    .text
                    .lines()
                    .map(|line| format!("> {line}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        TranscriptKind::System => {
            if english {
                format!("*({})*", event.text)
            } else {
                format!("*（{}）*", event.text)
            }
        }
    }
}

/// 場景標題＋事件列表組成一段章節，整桌匯出把多段章節接起來。
fn render_scene_section(events: &[TranscriptEvent], heading: &str, english: bool) -> String {
    let entries = events
        .iter()
        .map(|event| render_transcript_entry(event, english))
        .collect::<Vec<_>>();
    if entries.is_empty() {
        heading.to_owned()
    } else {
        format!("{heading}\n\n{}", entries.join("\n\n"))
    }
}

pub fn export_transcript_markdown(root: &Path, world: &str, lang: &str) -> DataResult<String> {
    let transcript_dir = world_dir(root, world)?.join("transcript");
    if !transcript_dir.is_dir() {
        return Err(invalid_data("這桌還沒有任何紀錄"));
    }

    let mut scenes = Vec::new();
    for entry in fs::read_dir(transcript_dir)? {
        let entry = entry?;
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
            continue;
        }
        if let Ok(scene) = stem.parse::<u64>() {
            scenes.push(scene);
        }
    }
    scenes.sort_unstable();
    scenes.dedup();
    if scenes.is_empty() {
        return Err(invalid_data("這桌還沒有任何紀錄"));
    }

    let english = lang == "en";
    let timestamp = local_timestamp()?;
    let title = if english {
        format!("# {world} — Session Transcript\n\nExported: {timestamp}")
    } else {
        format!("# {world} 跑團紀錄\n\n匯出時間：{timestamp}")
    };
    let mut sections = Vec::new();
    for scene in scenes {
        let heading = if english {
            format!("## Scene {scene}")
        } else {
            format!("## 場景 {scene}")
        };
        let events = read_transcript(root, world, scene)?;
        sections.push(render_scene_section(&events, &heading, english));
    }

    Ok(format!("{title}\n\n{}\n", sections.join("\n\n")))
}

/// 匯出單一場景的紀錄，格式與整桌匯出一致，供「過去的場」單場匯出使用。
/// 場景不存在（無該檔）視為錯誤，避免誤匯出空白文件。
pub fn export_scene_markdown(
    root: &Path,
    world: &str,
    scene: u64,
    lang: &str,
) -> DataResult<String> {
    let path = transcript_path(root, world, scene)?;
    if !path.exists() {
        return Err(invalid_data(format!("場景 {scene} 不存在")));
    }

    let english = lang == "en";
    let timestamp = local_timestamp()?;
    let title = if english {
        format!("# {world} — Scene {scene}\n\nExported: {timestamp}")
    } else {
        format!("# {world} 場景 {scene}\n\n匯出時間：{timestamp}")
    };
    let events = read_transcript(root, world, scene)?;
    let entries = events
        .iter()
        .map(|event| render_transcript_entry(event, english))
        .collect::<Vec<_>>();
    Ok(format!("{title}\n\n{}\n", entries.join("\n\n")))
}

/// 換場：把摘要包成一則 GM 旁白 append 到下一場景開頭，再把 current_scene +1 並存檔。
/// 回傳新場景號。摘要文字本身由呼叫端（單發 LLM）產生，這裡只負責落地與推進場次。
/// title 有值就存進「舊場景」（bump 前的 current_scene）的 scene_titles，與場次 +1 同一次 write_state。
pub fn begin_next_scene(
    root: &Path,
    world: &str,
    summary_text: &str,
    lang: &str,
    title: Option<&str>,
) -> DataResult<u64> {
    let mut state = read_state(root, world)?;
    let old_scene = state.current_scene;
    let next_scene = old_scene + 1;
    let text = if lang == "en" {
        format!("Previously:\n{summary_text}")
    } else {
        format!("【前情提要】\n{summary_text}")
    };
    append_transcript(
        root,
        world,
        next_scene,
        &TranscriptEvent {
            ts: local_timestamp()?,
            speaker: "GM".to_owned(),
            kind: TranscriptKind::Narration,
            text,
        },
    )?;
    if let Some(name) = title.map(str::trim).filter(|name| !name.is_empty()) {
        state
            .scene_titles
            .insert(old_scene.to_string(), name.to_owned());
    }
    state.current_scene = next_scene;
    write_state(root, world, &state)?;
    Ok(next_scene)
}

pub fn read_state(root: &Path, world: &str) -> DataResult<WorldState> {
    let path = world_dir(root, world)?.join("state.json");
    if !path.exists() {
        return Ok(WorldState::default());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

pub fn write_state(root: &Path, world: &str, state: &WorldState) -> DataResult<()> {
    fs::write(
        world_dir(root, world)?.join("state.json"),
        serde_json::to_string_pretty(state)?,
    )?;
    Ok(())
}

pub fn read_config(root: &Path) -> DataResult<AppConfig> {
    let path = root.join("config.json");
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

pub fn write_config(root: &Path, config: &AppConfig) -> DataResult<()> {
    fs::create_dir_all(root)?;
    let path = root.join("config.json");
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    // 0600 僅限 unix；Windows 的 %APPDATA% 本身即使用者私有目錄，不需 chmod
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&path)?;
    file.write_all(serde_json::to_string_pretty(config)?.as_bytes())?;
    // mode() 只在建檔時生效；補 set_permissions 修復既存檔的過寬權限
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(label: &str) -> Self {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("table-tavern-{label}-{}-{id}", std::process::id()));
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

    fn worldbook_entry(uid: u64, title: &str) -> WorldbookEntry {
        WorldbookEntry {
            uid,
            title: title.to_owned(),
            keys: vec!["霧".to_owned()],
            content: format!("{title}內容"),
            constant: false,
            order: 10,
            disabled: false,
            visibility: Visibility::Gm,
        }
    }

    fn write_worldbook_fixture(root: &TestRoot, world: &str, entries: serde_json::Value) {
        create_world(root.path(), world).unwrap();
        fs::write(
            root.path().join(format!("worlds/{world}/worldbook.json")),
            serde_json::to_string_pretty(&serde_json::json!({ "entries": entries })).unwrap(),
        )
        .unwrap();
    }

    fn read_worldbook_fixture(root: &TestRoot, world: &str) -> serde_json::Value {
        serde_json::from_str(
            &fs::read_to_string(root.path().join(format!("worlds/{world}/worldbook.json")))
                .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn worldbook_missing_returns_empty_and_invalid_json_errors() {
        let root = TestRoot::new("worldbook-missing");
        create_world(root.path(), "舊桌").unwrap();
        assert_eq!(read_worldbook(root.path(), "舊桌").unwrap(), Vec::new());
        assert_eq!(
            serde_json::to_value(Visibility::Gm).unwrap(),
            serde_json::json!({"type": "gm"})
        );
        assert_eq!(
            serde_json::to_value(Visibility::Characters(vec!["狐狸".to_owned()])).unwrap(),
            serde_json::json!({"type": "characters", "characters": ["狐狸"]})
        );

        fs::write(root.path().join("worlds/舊桌/worldbook.json"), "{broken").unwrap();
        assert!(read_worldbook(root.path(), "舊桌").is_err());
    }

    #[test]
    fn imports_st_worldbook_losslessly_and_round_trips_export() {
        let root = TestRoot::new("worldbook-st-import");
        create_world(root.path(), "來源").unwrap();
        let imported = serde_json::json!({
            "entries": {
                "7": {
                    "uid": 7,
                    "key": ["dragon", "wyrm"],
                    "comment": "龍",
                    "content": "古龍沉睡於山下。",
                    "constant": false,
                    "order": 20,
                    "disable": false,
                    "sticky": 4,
                    "probability": 37
                },
                "9": {
                    "uid": 9,
                    "key": [],
                    "comment": "王都",
                    "content": "王都戒嚴。",
                    "constant": true,
                    "order": 5,
                    "disable": false,
                    "extensions": {
                        "foreign_app": {"kept": true},
                        "table_tavern": {"visibility": "public"}
                    }
                }
            }
        });
        assert_eq!(
            import_worldbook(root.path(), "來源", &imported.to_string()).unwrap(),
            2
        );

        let entries = read_worldbook(root.path(), "來源").unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].uid, 0);
        assert_eq!(entries[0].title, "龍");
        assert_eq!(entries[0].keys, ["dragon", "wyrm"]);
        assert_eq!(entries[0].visibility, Visibility::Gm);
        assert_eq!(entries[1].uid, 1);
        assert_eq!(entries[1].visibility, Visibility::Public);

        let raw: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(root.path().join("worlds/來源/worldbook.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(raw["entries"]["0"]["sticky"], 4);
        assert_eq!(raw["entries"]["0"]["probability"], 37);
        assert_eq!(
            raw["entries"]["0"]["extensions"]["table_tavern"]["visibility"],
            "gm"
        );
        assert_eq!(
            raw["entries"]["1"]["extensions"]["foreign_app"]["kept"],
            true
        );

        let exported = root.path().join("exported-worldbook.json");
        export_worldbook(root.path(), "來源", &exported).unwrap();
        create_world(root.path(), "目的").unwrap();
        let exported_text = fs::read_to_string(exported).unwrap();
        assert_eq!(
            import_worldbook(root.path(), "目的", &exported_text).unwrap(),
            entries.len()
        );
        assert_eq!(
            read_worldbook(root.path(), "目的").unwrap().len(),
            entries.len()
        );
    }

    #[test]
    fn imports_character_book_mapping_and_appends_unique_uids() {
        let root = TestRoot::new("worldbook-character-book");
        create_world(root.path(), "世界").unwrap();
        let first = serde_json::json!({
            "entries": {
                "12": {
                    "uid": 12,
                    "key": ["existing"],
                    "comment": "既有",
                    "content": "內容",
                    "constant": false,
                    "order": 1,
                    "disable": false
                }
            }
        });
        import_worldbook(root.path(), "世界", &first.to_string()).unwrap();

        let character_book = serde_json::json!({
            "entries": [
                {
                    "keys": ["gate"],
                    "secondary_keys": ["night"],
                    "comment": "城門",
                    "content": "城門已關。",
                    "constant": false,
                    "insertion_order": 42,
                    "enabled": false,
                    "priority": 8
                },
                {
                    "keys": ["market"],
                    "comment": "市集",
                    "content": "市集喧鬧。",
                    "constant": false,
                    "insertion_order": 43,
                    "enabled": true
                }
            ]
        });
        assert_eq!(
            import_worldbook(root.path(), "世界", &character_book.to_string()).unwrap(),
            2
        );

        let entries = read_worldbook(root.path(), "世界").unwrap();
        assert_eq!(
            entries.iter().map(|entry| entry.uid).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert_eq!(entries[1].keys, ["gate"]);
        assert_eq!(entries[1].order, 42);
        assert!(entries[1].disabled);
        assert!(!entries[2].disabled);

        let raw: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(root.path().join("worlds/世界/worldbook.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(raw["entries"]["1"]["keysecondary"][0], "night");
        assert_eq!(raw["entries"]["1"]["priority"], 8);
        assert!(raw["entries"]["1"].get("keys").is_none());
        assert!(raw["entries"]["1"].get("enabled").is_none());
    }

    #[test]
    fn upsert_preserves_unknown_fields_allocates_uid_and_deletes() {
        let root = TestRoot::new("worldbook-upsert");
        create_world(root.path(), "世界").unwrap();
        let imported = serde_json::json!({
            "entries": {
                "5": {
                    "uid": 5,
                    "key": ["old"],
                    "comment": "舊標題",
                    "content": "舊內容",
                    "constant": false,
                    "order": 1,
                    "disable": false,
                    "sticky": 99
                }
            }
        });
        import_worldbook(root.path(), "世界", &imported.to_string()).unwrap();

        let mut updated = worldbook_entry(0, "新標題");
        updated.visibility = Visibility::Characters(vec!["狐狸".to_owned()]);
        assert_eq!(
            upsert_worldbook_entry(root.path(), "世界", updated.clone()).unwrap(),
            0
        );
        let raw: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(root.path().join("worlds/世界/worldbook.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(raw["entries"]["0"]["sticky"], 99);
        assert_eq!(raw["entries"]["0"]["comment"], "新標題");
        assert_eq!(
            raw["entries"]["0"]["extensions"]["table_tavern"]["visibility"]["characters"][0],
            "狐狸"
        );

        let allocated =
            upsert_worldbook_entry(root.path(), "世界", worldbook_entry(u64::MAX, "新增")).unwrap();
        assert_eq!(allocated, 1);
        let raw: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(root.path().join("worlds/世界/worldbook.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(raw["entries"]["1"]["selective"], true);
        assert_eq!(raw["entries"]["1"]["probability"], 100);
        assert_eq!(raw["entries"]["1"]["useProbability"], true);
        assert_eq!(raw["entries"]["1"]["depth"], 4);
        assert_eq!(raw["entries"]["1"]["displayIndex"], 0);

        delete_worldbook_entry(root.path(), "世界", 0).unwrap();
        assert_eq!(
            read_worldbook(root.path(), "世界")
                .unwrap()
                .into_iter()
                .map(|entry| entry.uid)
                .collect::<Vec<_>>(),
            [1]
        );
    }

    #[test]
    fn new_worldbook_entry_is_first_and_shifts_display_indices() {
        let root = TestRoot::new("worldbook-new-first");
        write_worldbook_fixture(
            &root,
            "世界",
            serde_json::json!({
                "10": {
                    "uid": 10, "comment": "甲", "order": 10, "displayIndex": 0
                },
                "20": {
                    "uid": 20, "comment": "乙", "order": 20, "displayIndex": 1
                }
            }),
        );

        let uid =
            upsert_worldbook_entry(root.path(), "世界", worldbook_entry(u64::MAX, "新增")).unwrap();
        assert_eq!(read_worldbook(root.path(), "世界").unwrap()[0].uid, uid);
        let raw = read_worldbook_fixture(&root, "世界");
        assert_eq!(raw["entries"]["10"]["displayIndex"], 1);
        assert_eq!(raw["entries"]["20"]["displayIndex"], 2);
        assert_eq!(raw["entries"][uid.to_string()]["displayIndex"], 0);
    }

    #[test]
    fn moving_down_then_up_restores_worldbook_order() {
        let root = TestRoot::new("worldbook-move-round-trip");
        write_worldbook_fixture(
            &root,
            "世界",
            serde_json::json!({
                "0": {"uid": 0, "comment": "甲", "displayIndex": 0},
                "1": {"uid": 1, "comment": "乙", "displayIndex": 1},
                "2": {"uid": 2, "comment": "丙", "displayIndex": 2}
            }),
        );

        move_worldbook_entry(root.path(), "世界", 0, false).unwrap();
        assert_eq!(
            read_worldbook(root.path(), "世界")
                .unwrap()
                .iter()
                .map(|entry| entry.uid)
                .collect::<Vec<_>>(),
            [1, 0, 2]
        );
        move_worldbook_entry(root.path(), "世界", 0, true).unwrap();
        assert_eq!(
            read_worldbook(root.path(), "世界")
                .unwrap()
                .iter()
                .map(|entry| entry.uid)
                .collect::<Vec<_>>(),
            [0, 1, 2]
        );
    }

    #[test]
    fn moving_top_worldbook_entry_up_is_no_op() {
        let root = TestRoot::new("worldbook-move-top");
        write_worldbook_fixture(
            &root,
            "世界",
            serde_json::json!({
                "0": {"uid": 0, "comment": "甲", "displayIndex": 0},
                "1": {"uid": 1, "comment": "乙", "displayIndex": 1}
            }),
        );
        let before = read_worldbook_fixture(&root, "世界");

        move_worldbook_entry(root.path(), "世界", 0, true).unwrap();

        assert_eq!(read_worldbook_fixture(&root, "世界"), before);
    }

    #[test]
    fn moving_legacy_worldbook_entries_normalizes_display_indices_stably() {
        let root = TestRoot::new("worldbook-move-legacy");
        write_worldbook_fixture(
            &root,
            "世界",
            serde_json::json!({
                "7": {"uid": 7, "comment": "丙"},
                "3": {"uid": 3, "comment": "甲"},
                "5": {"uid": 5, "comment": "乙"}
            }),
        );

        move_worldbook_entry(root.path(), "世界", 3, false).unwrap();

        assert_eq!(
            read_worldbook(root.path(), "世界")
                .unwrap()
                .iter()
                .map(|entry| entry.uid)
                .collect::<Vec<_>>(),
            [5, 3, 7]
        );
        let raw = read_worldbook_fixture(&root, "世界");
        let mut indices = raw["entries"]
            .as_object()
            .unwrap()
            .values()
            .map(|entry| entry["displayIndex"].as_u64().unwrap())
            .collect::<Vec<_>>();
        indices.sort_unstable();
        assert_eq!(indices, [0, 1, 2]);
    }

    #[test]
    fn moving_worldbook_entry_preserves_order_and_unknown_fields() {
        let root = TestRoot::new("worldbook-move-lossless");
        write_worldbook_fixture(
            &root,
            "世界",
            serde_json::json!({
                "0": {
                    "uid": 0, "comment": "甲", "order": 91, "displayIndex": 0,
                    "foreign": {"nested": true}
                },
                "1": {
                    "uid": 1, "comment": "乙", "order": 7, "displayIndex": 1,
                    "sticky": 42
                }
            }),
        );

        move_worldbook_entry(root.path(), "世界", 0, false).unwrap();

        let raw = read_worldbook_fixture(&root, "世界");
        assert_eq!(raw["entries"]["0"]["order"], 91);
        assert_eq!(raw["entries"]["1"]["order"], 7);
        assert_eq!(
            raw["entries"]["0"]["foreign"],
            serde_json::json!({"nested": true})
        );
        assert_eq!(raw["entries"]["1"]["sticky"], 42);
    }

    #[test]
    fn creates_lists_worlds_and_rejects_duplicates() {
        let root = TestRoot::new("worlds");
        assert!(list_worlds(root.path()).unwrap().is_empty());

        create_world(root.path(), "群島").unwrap();
        assert_eq!(list_worlds(root.path()).unwrap(), vec!["群島"]);
        assert!(create_world(root.path(), "群島").is_err());
        assert!(root.path().join("worlds/群島/characters").is_dir());
        assert!(root.path().join("worlds/群島/transcript").is_dir());
        assert!(root.path().join("worlds/群島/world.md").is_file());
        assert!(root.path().join("worlds/群島/state.json").is_file());
    }

    #[test]
    fn sample_world_is_ready_to_play() {
        let root = TestRoot::new("sample-world");
        let world = create_sample_world(root.path(), "zh-TW").unwrap();

        assert_eq!(world, "迷霧酒館（範例）");
        assert!(list_worlds(root.path()).unwrap().contains(&world));

        let characters = list_characters(root.path(), &world).unwrap();
        assert_eq!(characters.len(), 3);
        for name in ["狐狸", "騎士", "吟遊詩人"] {
            assert!(characters.iter().any(|character| character.name == name));
        }

        let world_md = read_world_md(root.path(), &world).unwrap();
        assert!(!world_md.is_empty());
        assert!(world_md.contains("霧口鎮"));

        let transcript = read_transcript(root.path(), &world, 0).unwrap();
        assert_eq!(transcript.len(), 1);
        assert_eq!(transcript[0].kind, TranscriptKind::Narration);
        assert_eq!(transcript[0].speaker, "GM");

        // 重複呼叫要沿用既有那桌，不噴 File exists、也不重複塞開場旁白
        assert_eq!(create_sample_world(root.path(), "zh-TW").unwrap(), world);
        assert_eq!(read_transcript(root.path(), &world, 0).unwrap().len(), 1);
    }

    #[test]
    fn sample_world_english_content_follows_lang() {
        let root = TestRoot::new("sample-world-en");
        let world = create_sample_world(root.path(), "en").unwrap();

        assert_eq!(world, "The Misty Tavern (sample)");
        let characters = list_characters(root.path(), &world).unwrap();
        assert_eq!(characters.len(), 3);
        for name in ["Fox", "Knight", "Bard"] {
            assert!(characters.iter().any(|character| character.name == name));
        }
        assert!(read_world_md(root.path(), &world)
            .unwrap()
            .contains("Mistmouth"));
        let transcript = read_transcript(root.path(), &world, 0).unwrap();
        assert!(transcript[0].text.starts_with("Rain hammers"));
    }

    #[cfg(unix)]
    #[test]
    fn lists_worlds_by_last_activity_descending() {
        let root = TestRoot::new("activity");
        create_world(root.path(), "甲桌").unwrap();
        create_world(root.path(), "乙桌").unwrap();

        // 兩桌目錄 mtime 撥回一小時前：同時間時按名稱升冪（乙 U+4E59 < 甲 U+7532）
        let hour_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        for name in ["甲桌", "乙桌"] {
            let directory = fs::File::open(root.path().join("worlds").join(name)).unwrap();
            directory.set_modified(hour_ago).unwrap();
        }
        assert_eq!(list_worlds(root.path()).unwrap(), vec!["乙桌", "甲桌"]);

        // 對名稱排序居後的甲桌寫一筆訊息，活動排序應把它推到最前
        let event = TranscriptEvent {
            ts: "2026-07-19T00:00:00Z".to_owned(),
            speaker: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "你好".to_owned(),
        };
        append_transcript(root.path(), "甲桌", 0, &event).unwrap();
        assert_eq!(list_worlds(root.path()).unwrap(), vec!["甲桌", "乙桌"]);
    }

    #[test]
    fn reclaims_only_untouched_worlds() {
        let root = TestRoot::new("reclaim");
        create_world(root.path(), "空桌").unwrap();
        assert!(reclaim_world_if_empty(root.path(), "空桌").unwrap());
        assert!(list_worlds(root.path()).unwrap().is_empty());
        // 已刪的桌再回收一次應為 no-op
        assert!(!reclaim_world_if_empty(root.path(), "空桌").unwrap());

        create_world(root.path(), "有訊息").unwrap();
        let event = TranscriptEvent {
            ts: "2026-07-19T00:00:00Z".to_owned(),
            speaker: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "留著".to_owned(),
        };
        append_transcript(root.path(), "有訊息", 0, &event).unwrap();
        assert!(!reclaim_world_if_empty(root.path(), "有訊息").unwrap());

        create_world(root.path(), "有角色").unwrap();
        let card = CharacterCard {
            name: "旅人".to_owned(),
            color: "#e07a5f".to_owned(),
            avatar: "🎭".to_owned(),
            tier: Tier::Default,
            show_image: true,
            archived: false,
            gen_prompt: String::new(),
            public_md: String::new(),
            private_md: String::new(),
        };
        write_character(root.path(), "有角色", &card).unwrap();
        assert!(!reclaim_world_if_empty(root.path(), "有角色").unwrap());

        create_world(root.path(), "有設定").unwrap();
        write_world_md(root.path(), "有設定", "海島世界").unwrap();
        assert!(!reclaim_world_if_empty(root.path(), "有設定").unwrap());
    }

    #[test]
    fn renames_world_and_rejects_collisions() {
        let root = TestRoot::new("rename");
        create_world(root.path(), "舊名").unwrap();
        create_world(root.path(), "占用").unwrap();

        rename_world(root.path(), "舊名", "新名").unwrap();
        assert!(root.path().join("worlds/新名").is_dir());
        assert!(!root.path().join("worlds/舊名").exists());

        assert!(rename_world(root.path(), "新名", "占用").is_err());
        rename_world(root.path(), "新名", "新名").unwrap();
        assert!(rename_world(root.path(), "新名", "壞/名").is_err());
    }

    #[test]
    fn rejects_multiline_frontmatter_values() {
        let root = TestRoot::new("scalars");
        create_world(root.path(), "世界").unwrap();
        let card = CharacterCard {
            name: "角色".to_owned(),
            color: "#123456\ntier: best".to_owned(),
            avatar: "🧙".to_owned(),
            tier: Tier::Default,
            show_image: true,
            archived: false,
            gen_prompt: String::new(),
            public_md: String::new(),
            private_md: String::new(),
        };
        assert!(write_character(root.path(), "世界", &card).is_err());
    }

    #[test]
    fn rejects_unsafe_world_and_character_names() {
        let root = TestRoot::new("names");
        for name in ["../evil", "a/b", ".hidden", ""] {
            assert!(
                create_world(root.path(), name).is_err(),
                "accepted {name:?}"
            );
        }

        create_world(root.path(), "安全世界").unwrap();
        for name in ["../evil", "a/b", ".hidden", ""] {
            let card = CharacterCard {
                name: name.to_owned(),
                color: "#123456".to_owned(),
                avatar: "🧙".to_owned(),
                tier: Tier::Default,
                show_image: true,
                archived: false,
                gen_prompt: String::new(),
                public_md: String::new(),
                private_md: String::new(),
            };
            assert!(write_character(root.path(), "安全世界", &card).is_err());
        }
    }

    #[test]
    fn character_round_trip_preserves_fields_and_sections() {
        let root = TestRoot::new("character");
        create_world(root.path(), "港灣").unwrap();
        let card = CharacterCard {
            name: "阿藍".to_owned(),
            color: "#3366ff".to_owned(),
            avatar: "avatars/blue.png".to_owned(),
            tier: Tier::Best,
            show_image: true,
            archived: true,
            gen_prompt: "暖色調 水彩風".to_owned(),
            public_md: "第一段\n\n- 公開條目\n".to_owned(),
            private_md: "秘密第一行\n\n秘密第二行".to_owned(),
        };

        write_character(root.path(), "港灣", &card).unwrap();
        assert_eq!(read_character(root.path(), "港灣", "阿藍").unwrap(), card);
        assert_eq!(
            list_characters(root.path(), "港灣").unwrap(),
            vec![CharacterMeta {
                name: "阿藍".to_owned(),
                color: "#3366ff".to_owned(),
                avatar: "avatars/blue.png".to_owned(),
                tier: Tier::Best,
                show_image: true,
                archived: true,
            }]
        );

        let raw = fs::read_to_string(root.path().join("worlds/港灣/characters/阿藍.md")).unwrap();
        let frontmatter = raw
            .strip_prefix("---\n")
            .unwrap()
            .split_once("\n---\n")
            .unwrap()
            .0;
        let keys: Vec<_> = frontmatter
            .lines()
            .map(|line| line.split_once(':').unwrap().0)
            .collect();
        assert_eq!(
            keys,
            [
                "name",
                "color",
                "avatar",
                "tier",
                "show_image",
                "archived",
                "gen_prompt"
            ]
        );
        assert!(raw.contains("\n## 公開\n"));
        assert!(raw.contains("\n## 私有\n"));

        set_character_archived(root.path(), "港灣", "阿藍", false).unwrap();
        assert!(
            !read_character(root.path(), "港灣", "阿藍")
                .unwrap()
                .archived
        );
    }

    #[test]
    fn show_image_false_round_trips_and_missing_key_defaults_to_true() {
        let root = TestRoot::new("show-image");
        create_world(root.path(), "世界").unwrap();
        let card = CharacterCard {
            name: "藏圖".to_owned(),
            color: "#333333".to_owned(),
            avatar: "🎭".to_owned(),
            tier: Tier::Default,
            show_image: false,
            archived: false,
            gen_prompt: String::new(),
            public_md: String::new(),
            private_md: String::new(),
        };
        write_character(root.path(), "世界", &card).unwrap();
        assert!(
            !read_character(root.path(), "世界", "藏圖")
                .unwrap()
                .show_image
        );

        fs::write(
            root.path().join("worlds/世界/characters/舊卡.md"),
            "---\nname: 舊卡\ncolor: #111111\navatar: 🎭\ntier: default\n---\n## 公開\n\n## 私有\n",
        )
        .unwrap();
        let old_card = read_character(root.path(), "世界", "舊卡").unwrap();
        assert!(old_card.show_image);
        assert!(!old_card.archived);
    }

    #[test]
    fn delete_character_removes_card_and_images() {
        let root = TestRoot::new("delete-character");
        create_world(root.path(), "世界").unwrap();
        let card = CharacterCard {
            name: "退場角色".to_owned(),
            color: "#333333".to_owned(),
            avatar: "🎭".to_owned(),
            tier: Tier::Default,
            show_image: true,
            archived: true,
            gen_prompt: String::new(),
            public_md: String::new(),
            private_md: String::new(),
        };
        write_character(root.path(), "世界", &card).unwrap();
        let md_path = character_path(root.path(), "世界", "退場角色").unwrap();
        let png_path = md_path.with_extension("png");
        let avatar_path = md_path.with_extension("avatar.png");
        fs::write(&png_path, b"png").unwrap();
        fs::write(&avatar_path, b"avatar").unwrap();

        delete_character(root.path(), "世界", "退場角色").unwrap();

        assert!(list_characters(root.path(), "世界").unwrap().is_empty());
        assert!(!md_path.exists());
        assert!(!png_path.exists());
        assert!(!avatar_path.exists());
    }

    #[test]
    fn frontmatter_accepts_spacing_and_order_but_rejects_invalid_tier() {
        let root = TestRoot::new("frontmatter");
        create_world(root.path(), "世界").unwrap();
        let path = root.path().join("worlds/世界/characters/角色.md");
        fs::write(
            &path,
            "---\ntier : fast\nunknown: ignored\navatar: 🐕\n color : #abcdef\nname : 角色\n---\n## 私有\n私密",
        )
        .unwrap();
        assert_eq!(
            read_character(root.path(), "世界", "角色").unwrap().tier,
            Tier::Fast
        );

        fs::write(
            path,
            "---\nname: 角色\ncolor: #abcdef\navatar: 🐕\ntier: impossible\n---\n",
        )
        .unwrap();
        assert!(read_character(root.path(), "世界", "角色").is_err());
    }

    #[test]
    fn transcript_round_trip_is_ordered_jsonl_and_rejects_invalid_kind() {
        let root = TestRoot::new("transcript");
        create_world(root.path(), "劇場").unwrap();
        let events = vec![
            TranscriptEvent {
                ts: "2026-07-19T10:00:00+08:00".to_owned(),
                speaker: "旁白".to_owned(),
                kind: TranscriptKind::Narration,
                text: "序幕".to_owned(),
            },
            TranscriptEvent {
                ts: "2026-07-19T10:00:01+08:00".to_owned(),
                speaker: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "第一行\n仍是同一事件".to_owned(),
            },
            TranscriptEvent {
                ts: "2026-07-19T10:00:02+08:00".to_owned(),
                speaker: "角色".to_owned(),
                kind: TranscriptKind::Dialogue,
                text: "你好".to_owned(),
            },
        ];
        for event in &events {
            append_transcript(root.path(), "劇場", 7, event).unwrap();
        }
        assert_eq!(read_transcript(root.path(), "劇場", 7).unwrap(), events);

        let path = root.path().join("worlds/劇場/transcript/7.jsonl");
        let raw = fs::read_to_string(&path).unwrap();
        let lines: Vec<_> = raw.lines().collect();
        assert_eq!(lines.len(), 3);
        for line in lines {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(value.is_object());
            assert!(["dialogue", "narration", "player", "system"]
                .contains(&value["kind"].as_str().unwrap()));
        }

        OpenOptions::new()
            .append(true)
            .open(path)
            .unwrap()
            .write_all(b"{\"ts\":\"now\",\"speaker\":\"x\",\"kind\":\"bad\",\"text\":\"x\"}\n")
            .unwrap();
        let error = read_transcript(root.path(), "劇場", 7)
            .unwrap_err()
            .to_string();
        assert!(error.contains("line 4"), "{error}");
    }

    #[test]
    fn exports_all_transcript_scenes_as_localized_markdown() {
        let root = TestRoot::new("transcript-export");
        create_world(root.path(), "海風桌").unwrap();
        for (scene, event) in [
            (
                1,
                TranscriptEvent {
                    ts: "now".to_owned(),
                    speaker: "船長".to_owned(),
                    kind: TranscriptKind::Dialogue,
                    text: "我們啟航。".to_owned(),
                },
            ),
            (
                0,
                TranscriptEvent {
                    ts: "now".to_owned(),
                    speaker: "GM".to_owned(),
                    kind: TranscriptKind::Narration,
                    text: "霧氣升起。\n港口安靜。".to_owned(),
                },
            ),
            (
                1,
                TranscriptEvent {
                    ts: "now".to_owned(),
                    speaker: "玩家".to_owned(),
                    kind: TranscriptKind::Player,
                    text: "我登上甲板。".to_owned(),
                },
            ),
            (
                0,
                TranscriptEvent {
                    ts: "now".to_owned(),
                    speaker: "GM".to_owned(),
                    kind: TranscriptKind::System,
                    text: "第一幕開始".to_owned(),
                },
            ),
        ] {
            append_transcript(root.path(), "海風桌", scene, &event).unwrap();
        }

        let zh = export_transcript_markdown(root.path(), "海風桌", "zh-TW").unwrap();
        assert!(zh.starts_with("# 海風桌 跑團紀錄\n\n匯出時間："));
        assert!(zh.find("## 場景 0").unwrap() < zh.find("## 場景 1").unwrap());
        assert!(zh.contains("> 霧氣升起。\n> 港口安靜。"));
        assert!(zh.contains("*（第一幕開始）*"));
        assert!(zh.contains("**玩家**：我登上甲板。"));
        assert!(zh.contains("**船長**：我們啟航。"));

        let en = export_transcript_markdown(root.path(), "海風桌", "en").unwrap();
        assert!(en.starts_with("# 海風桌 — Session Transcript\n\nExported: "));
        assert!(en.contains("## Scene 0"));
        assert!(en.contains("## Scene 1"));
        assert!(en.contains("**船長**: 我們啟航。"));
        assert!(en.contains("*(第一幕開始)*"));
    }

    #[test]
    fn transcript_export_rejects_a_world_without_scenes() {
        let root = TestRoot::new("empty-transcript-export");
        create_world(root.path(), "空桌").unwrap();
        assert!(export_transcript_markdown(root.path(), "空桌", "zh-TW").is_err());
    }

    #[test]
    fn scene_export_contains_only_that_scenes_events() {
        let root = TestRoot::new("scene-export");
        create_world(root.path(), "海風桌").unwrap();
        for (scene, event) in [
            (
                0,
                TranscriptEvent {
                    ts: "now".to_owned(),
                    speaker: "GM".to_owned(),
                    kind: TranscriptKind::Narration,
                    text: "霧氣升起。".to_owned(),
                },
            ),
            (
                1,
                TranscriptEvent {
                    ts: "now".to_owned(),
                    speaker: "船長".to_owned(),
                    kind: TranscriptKind::Dialogue,
                    text: "我們啟航。".to_owned(),
                },
            ),
        ] {
            append_transcript(root.path(), "海風桌", scene, &event).unwrap();
        }

        let zh = export_scene_markdown(root.path(), "海風桌", 0, "zh-TW").unwrap();
        assert!(zh.starts_with("# 海風桌 場景 0\n\n匯出時間："));
        assert!(zh.contains("> 霧氣升起。"));
        assert!(!zh.contains("船長"));

        let en = export_scene_markdown(root.path(), "海風桌", 1, "en").unwrap();
        assert!(en.starts_with("# 海風桌 — Scene 1\n\nExported: "));
        assert!(en.contains("**船長**: 我們啟航。"));
        assert!(!en.contains("霧氣升起"));
    }

    #[test]
    fn scene_export_rejects_a_missing_scene() {
        let root = TestRoot::new("scene-export-missing");
        create_world(root.path(), "空桌").unwrap();
        assert!(export_scene_markdown(root.path(), "空桌", 0, "zh-TW").is_err());
    }

    #[test]
    fn begin_next_scene_appends_summary_and_advances_scene() {
        let root = TestRoot::new("begin-next-scene");
        create_world(root.path(), "換場桌").unwrap();
        let event = TranscriptEvent {
            ts: "2026-07-19T00:00:00Z".to_owned(),
            speaker: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "第一場的對話".to_owned(),
        };
        append_transcript(root.path(), "換場桌", 0, &event).unwrap();

        let next = begin_next_scene(root.path(), "換場桌", "壓縮後的摘要", "zh-TW", None).unwrap();
        assert_eq!(next, 1);
        assert_eq!(read_state(root.path(), "換場桌").unwrap().current_scene, 1);

        // 摘要落在新場景檔開頭，舊場景不受影響
        assert_eq!(read_transcript(root.path(), "換場桌", 0).unwrap().len(), 1);
        let new_scene = read_transcript(root.path(), "換場桌", 1).unwrap();
        assert_eq!(new_scene.len(), 1);
        assert_eq!(new_scene[0].speaker, "GM");
        assert_eq!(new_scene[0].kind, TranscriptKind::Narration);
        assert_eq!(new_scene[0].text, "【前情提要】\n壓縮後的摘要");

        // en 語系用英文前綴
        let next_en = begin_next_scene(root.path(), "換場桌", "recap text", "en", None).unwrap();
        assert_eq!(next_en, 2);
        let scene_two = read_transcript(root.path(), "換場桌", 2).unwrap();
        assert_eq!(scene_two[0].text, "Previously:\nrecap text");
    }

    #[test]
    fn begin_next_scene_stores_title_on_old_scene_when_given() {
        let root = TestRoot::new("begin-next-scene-title");
        create_world(root.path(), "取名桌").unwrap();
        let event = TranscriptEvent {
            ts: "2026-07-24T00:00:00Z".to_owned(),
            speaker: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "第一幕的對話".to_owned(),
        };
        append_transcript(root.path(), "取名桌", 0, &event).unwrap();

        begin_next_scene(root.path(), "取名桌", "摘要", "zh-TW", Some("酒館夜話")).unwrap();
        let state = read_state(root.path(), "取名桌").unwrap();
        assert_eq!(state.current_scene, 1);
        assert_eq!(
            state.scene_titles.get("0").map(String::as_str),
            Some("酒館夜話")
        );
        assert!(!state.scene_titles.contains_key("1"));

        // 空字串／None 都不進表
        begin_next_scene(root.path(), "取名桌", "摘要二", "zh-TW", Some("   ")).unwrap();
        begin_next_scene(root.path(), "取名桌", "摘要三", "zh-TW", None).unwrap();
        let state = read_state(root.path(), "取名桌").unwrap();
        assert!(!state.scene_titles.contains_key("1"));
        assert!(!state.scene_titles.contains_key("2"));
    }

    #[test]
    fn state_round_trip_and_missing_file_default() {
        let root = TestRoot::new("state");
        fs::create_dir_all(root.path().join("worlds/無狀態")).unwrap();
        assert_eq!(
            read_state(root.path(), "無狀態").unwrap(),
            WorldState::default()
        );

        let mut state = WorldState {
            current_scene: 12,
            ..WorldState::default()
        };
        state
            .model_bindings
            .insert("船長".to_owned(), "balanced".to_owned());
        state
            .catchup_summaries
            .insert("水手".to_owned(), "錯過了序幕".to_owned());
        write_state(root.path(), "無狀態", &state).unwrap();
        assert_eq!(read_state(root.path(), "無狀態").unwrap(), state);
    }

    #[test]
    fn config_round_trip_and_permissions_are_private() {
        let root = TestRoot::new("config");
        assert_eq!(read_config(root.path()).unwrap(), AppConfig::default());
        let mut config = AppConfig::default();
        config
            .api_keys
            .insert("provider".to_owned(), "secret".to_owned());
        config
            .tier_models
            .insert("best".to_owned(), "model-name".to_owned());
        config.preferences.insert(
            "language".to_owned(),
            serde_json::Value::String("zh-TW".to_owned()),
        );

        write_config(root.path(), &config).unwrap();
        assert_eq!(read_config(root.path()).unwrap(), config);
        #[cfg(unix)]
        {
            let mode = fs::metadata(root.path().join("config.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
    }
}
