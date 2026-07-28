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

/// 產生新的定址代碼（ULID）。世界與角色的存檔路徑一律用這個，顯示名只是檔案內的一個欄位。
pub fn new_id() -> String {
    ulid::Ulid::generate().to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Best,
    /// 角色未特別指定時的檔位；舊存檔的 "default" 也讀成這個
    #[serde(alias = "default")]
    Balanced,
    Fast,
}

impl Tier {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Best => "best",
            Self::Balanced => "balanced",
            Self::Fast => "fast",
        }
    }

    pub(crate) fn parse(value: &str) -> DataResult<Self> {
        match value {
            "best" => Ok(Self::Best),
            "balanced" | "default" => Ok(Self::Balanced),
            "fast" => Ok(Self::Fast),
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
    pub id: String,
    pub name: String,
    pub color: String,
    pub avatar: String,
    pub tier: Tier,
    #[serde(default = "default_show_image")]
    pub show_image: bool,
    #[serde(default)]
    pub archived: bool,
    /// 側欄卡片的顯示順序；只在後端流通（前端拿到的已是排好的清單）
    #[serde(skip)]
    pub display_index: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterCard {
    pub id: String,
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
    /// 角色事件存角色 id；GM 旁白／系統訊息／玩家發言存空字串（kind 已足以區分）
    pub speaker_id: String,
    /// 發言當下的顯示名快照——改名後舊事件不動，這是既有拍板行為
    pub speaker_name: String,
    pub kind: TranscriptKind,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "characters", rename_all = "lowercase")]
pub enum Visibility {
    Gm,
    Public,
    /// 存的是角色 id
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldState {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub model_bindings: BTreeMap<String, String>,
    #[serde(default)]
    pub player_card_id: Option<String>,
    #[serde(default)]
    pub current_scene: u64,
    #[serde(default)]
    pub catchup_summaries: BTreeMap<String, String>,
    // 換幕順手取的幕名：key 是場景號字串（比照 catchup_summaries），沒取到就不進這個表
    #[serde(default)]
    pub scene_titles: BTreeMap<String, String>,
}

/// 側欄桌列表用的精簡視圖
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldMeta {
    pub id: String,
    pub name: String,
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

/// 定址代碼格式：26 字 Crockford base32（ulid crate 輸出的格式），擋掉一切路徑逃逸。
/// 所有用 id 組路徑的地方都先過這關。
pub(crate) fn validate_id(id: &str) -> DataResult<()> {
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    if id.len() != 26 || !id.bytes().all(|byte| ALPHABET.contains(&byte)) {
        return Err(invalid_data(format!("invalid id: {id:?}")));
    }
    Ok(())
}

pub(crate) fn validate_single_line(field: &str, value: &str) -> DataResult<()> {
    if value.contains('\n') || value.contains('\r') {
        return Err(invalid_data(format!("{field} must be a single line")));
    }
    Ok(())
}

fn worlds_dir(root: &Path) -> PathBuf {
    root.join("worlds")
}

fn world_dir(root: &Path, world_id: &str) -> DataResult<PathBuf> {
    validate_id(world_id)?;
    Ok(worlds_dir(root).join(world_id))
}

pub(crate) fn character_path(
    root: &Path,
    world_id: &str,
    character_id: &str,
) -> DataResult<PathBuf> {
    validate_id(character_id)?;
    Ok(world_dir(root, world_id)?
        .join("characters")
        .join(format!("{character_id}.md")))
}

/// 生成圖庫目錄，落在世界目錄內：worlds/<world_id>/gen-gallery/<character_id>。
pub(crate) fn gallery_dir(root: &Path, world_id: &str, character_id: &str) -> DataResult<PathBuf> {
    validate_id(character_id)?;
    Ok(world_dir(root, world_id)?
        .join("gen-gallery")
        .join(character_id))
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

/// 依最後活動排序（新的在前），供側欄桌列表用（NewPlan §9.3）。
/// state.json 解析失敗（含舊格式缺 id/name）的桌一律略過，不寫遷移、不做偵測提示。
pub fn list_worlds(root: &Path) -> DataResult<Vec<WorldMeta>> {
    let directory = worlds_dir(root);
    if !directory.exists() {
        return Ok(Vec::new());
    }

    let mut worlds = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let state_path = entry.path().join("state.json");
        let state = fs::read_to_string(&state_path)
            .ok()
            .and_then(|text| serde_json::from_str::<WorldState>(&text).ok());
        match state {
            Some(state) => worlds.push((
                last_active(&entry.path()),
                WorldMeta {
                    id: state.id,
                    name: state.name,
                },
            )),
            None => eprintln!("略過無法解析的桌：{}", entry.path().display()),
        }
    }
    worlds.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.name.cmp(&b.1.name))
            .then_with(|| a.1.id.cmp(&b.1.id))
    });
    Ok(worlds.into_iter().map(|(_, meta)| meta).collect())
}

pub fn create_world(root: &Path, name: &str) -> DataResult<String> {
    validate_single_line("world name", name)?;
    let id = new_id();
    let directory = worlds_dir(root).join(&id);
    fs::create_dir_all(worlds_dir(root))?;
    fs::create_dir(&directory)?;
    fs::create_dir(directory.join("characters"))?;
    fs::create_dir(directory.join("transcript"))?;
    fs::write(directory.join("world.md"), "")?;
    let state = WorldState {
        id: id.clone(),
        name: name.to_owned(),
        model_bindings: BTreeMap::new(),
        player_card_id: None,
        current_scene: 0,
        catchup_summaries: BTreeMap::new(),
        scene_titles: BTreeMap::new(),
    };
    fs::write(
        directory.join("state.json"),
        serde_json::to_string_pretty(&state)?,
    )?;
    Ok(id)
}

/// 範例桌內容依語系產生（首開先選語言再建桌）；lang 非 en 一律走 zh-TW
pub fn create_sample_world(root: &Path, lang: &str) -> DataResult<String> {
    let english = lang == "en";
    let world_name = if english {
        "The Misty Tavern (sample)"
    } else {
        "迷霧酒館（範例）"
    };
    // 冪等：範例桌已在就直接沿用，避免重複呼叫（dev 的 StrictMode 雙跑）噴重複資料
    if let Some(existing) = list_worlds(root)?
        .into_iter()
        .find(|meta| meta.name == world_name)
    {
        return Ok(existing.id);
    }
    let world_id = create_world(root, world_name)?;
    write_world_md(
        root,
        &world_id,
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
        ("#e07a5f", "🦊", Tier::Balanced),
        ("#3d84a8", "🛡️", Tier::Balanced),
        ("#f2a541", "🪕", Tier::Fast),
    ];
    for ((name, public_md, private_md), (color, avatar, tier)) in texts.into_iter().zip(style) {
        write_character(
            root,
            &world_id,
            &CharacterCard {
                id: new_id(),
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
        &world_id,
        0,
        &TranscriptEvent {
            ts: "2026-07-20T00:00:00+08:00".to_owned(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: TranscriptKind::Narration,
            text: if english {
                "Rain hammers the windows of the Misty Tavern; the hearth crackles. Few guests tonight — the keeper polishes glasses behind the bar, a knight sips tea in the corner, and the bard is tuning their strings. Outside, faint through the storm, comes a wolf's howl."
            } else {
                "暴雨拍打著迷霧酒館的窗，爐火劈啪作響。今晚店裡客人不多——老闆在吧檯後擦著杯子，一名騎士坐在角落喝茶，吟遊詩人正調著琴弦。門外，隱約傳來一聲狼嚎。"
            }
            .to_owned(),
        },
    )?;

    Ok(world_id)
}

/// 空桌回收（NewPlan §9.3）：只回收完全未動過的桌——零訊息、零角色、world.md 空白；
/// 任一項有內容即保留，防資料遺失。回傳是否真的刪了。
pub fn reclaim_world_if_empty(root: &Path, world_id: &str) -> DataResult<bool> {
    let directory = world_dir(root, world_id)?;
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

/// 刪桌：世界資料夾整包清掉（生成圖庫已收在世界目錄內，一併刪除）。不可復原。
pub fn delete_world(root: &Path, world_id: &str) -> DataResult<()> {
    let directory = world_dir(root, world_id)?;
    if directory.exists() {
        fs::remove_dir_all(&directory)?;
    }
    Ok(())
}

/// 桌名隨時可改（NewPlan §9.3）：只改 state.json 的 name，目錄路徑（world_id）不動。
pub fn rename_world(root: &Path, world_id: &str, new_name: &str) -> DataResult<()> {
    validate_single_line("world name", new_name)?;
    let mut state = read_state(root, world_id)?;
    state.name = new_name.to_owned();
    write_state(root, world_id, &state)
}

pub fn read_world_md(root: &Path, world_id: &str) -> DataResult<String> {
    Ok(fs::read_to_string(
        world_dir(root, world_id)?.join("world.md"),
    )?)
}

pub fn write_world_md(root: &Path, world_id: &str, content: &str) -> DataResult<()> {
    fs::write(world_dir(root, world_id)?.join("world.md"), content)?;
    Ok(())
}

fn worldbook_path(root: &Path, world_id: &str) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?.join("worldbook.json"))
}

fn empty_worldbook() -> serde_json::Value {
    serde_json::json!({ "entries": {} })
}

fn read_worldbook_value(root: &Path, world_id: &str) -> DataResult<serde_json::Value> {
    let path = worldbook_path(root, world_id)?;
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

fn write_worldbook_value(root: &Path, world_id: &str, value: &serde_json::Value) -> DataResult<()> {
    fs::write(
        worldbook_path(root, world_id)?,
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
            .filter(|ids| ids.iter().all(serde_json::Value::is_string))
            .map(|ids| {
                Visibility::Characters(
                    ids.iter()
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
        Visibility::Characters(ids) => serde_json::json!({ "characters": ids }),
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

pub fn read_worldbook(root: &Path, world_id: &str) -> DataResult<Vec<WorldbookEntry>> {
    let value = read_worldbook_value(root, world_id)?;
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

pub fn upsert_worldbook_entry(
    root: &Path,
    world_id: &str,
    entry: WorldbookEntry,
) -> DataResult<u64> {
    let mut worldbook = read_worldbook_value(root, world_id)?;
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
    write_worldbook_value(root, world_id, &worldbook)?;
    Ok(actual_uid)
}

/// 拖曳排序：uids 就是新的顯示順序，沒送到的條目依原順序接在後面
pub fn reorder_worldbook_entries(root: &Path, world_id: &str, uids: &[u64]) -> DataResult<()> {
    let mut worldbook = read_worldbook_value(root, world_id)?;
    let entries = entries_object_mut(&mut worldbook)?;
    let keys = sorted_entry_keys(entries);

    let mut ordered: Vec<String> = Vec::with_capacity(keys.len());
    for uid in uids {
        let Some(key) = keys
            .iter()
            .find(|key| entry_uid(key, &entries[*key]) == Some(*uid))
        else {
            continue;
        };
        if !ordered.contains(key) {
            ordered.push(key.clone());
        }
    }
    for key in &keys {
        if !ordered.contains(key) {
            ordered.push(key.clone());
        }
    }

    normalize_display_indices(entries, &ordered)?;
    write_worldbook_value(root, world_id, &worldbook)
}

pub fn delete_worldbook_entry(root: &Path, world_id: &str, uid: u64) -> DataResult<()> {
    let path = worldbook_path(root, world_id)?;
    if !path.exists() {
        return Ok(());
    }
    let mut worldbook = read_worldbook_value(root, world_id)?;
    let entries = entries_object_mut(&mut worldbook)?;
    let key = entries
        .iter()
        .find(|(key, value)| entry_uid(key, value) == Some(uid))
        .map(|(key, _)| key.clone());
    if let Some(key) = key {
        entries.remove(&key);
        write_worldbook_value(root, world_id, &worldbook)?;
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

pub fn import_worldbook(root: &Path, world_id: &str, json_text: &str) -> DataResult<usize> {
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

    let mut worldbook = read_worldbook_value(root, world_id)?;
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
    write_worldbook_value(root, world_id, &worldbook)?;
    Ok(imported_count)
}

pub fn export_worldbook(root: &Path, world_id: &str, path: &Path) -> DataResult<()> {
    let source = worldbook_path(root, world_id)?;
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

    let mut id = None;
    let mut name = None;
    let mut color = None;
    let mut avatar = None;
    let mut tier = None;
    let mut show_image = true;
    let mut archived = false;
    let mut display_index = None;
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
            "id" => id = Some(value.to_owned()),
            "name" => name = Some(value.to_owned()),
            "color" => color = Some(value.to_owned()),
            "avatar" => avatar = Some(value.to_owned()),
            "tier" => tier = Some(Tier::parse(value)?),
            "show_image" => show_image = value != "false",
            "archived" => archived = value == "true",
            "display_index" => display_index = value.parse().ok(),
            "gen_prompt" => gen_prompt = value.to_owned(),
            _ => {}
        }
    }

    // 新格式一律要有 id；缺 id 視為解析失敗（舊資料不遷移、不偵測，交給呼叫端略過）
    let id = id.ok_or_else(|| invalid_data("frontmatter is missing id"))?;
    let name = name.ok_or_else(|| invalid_data("frontmatter is missing name"))?;
    Ok((
        CharacterMeta {
            id,
            name,
            color: color.ok_or_else(|| invalid_data("frontmatter is missing color"))?,
            avatar: avatar.ok_or_else(|| invalid_data("frontmatter is missing avatar"))?,
            tier: tier.ok_or_else(|| invalid_data("frontmatter is missing tier"))?,
            show_image,
            archived,
            display_index,
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

fn serialize_character(card: &CharacterCard, display_index: u32) -> String {
    // frontmatter 逐行解析，生成提示詞中的換行須在寫入前攤平。
    let gen_prompt = card.gen_prompt.replace(['\n', '\r'], " ");
    format!(
        "---\nid: {}\nname: {}\ncolor: {}\navatar: {}\ntier: {}\nshow_image: {}\narchived: {}\ndisplay_index: {}\ngen_prompt: {}\n---\n## 公開\n{}\n## 私有\n{}",
        card.id,
        card.name,
        card.color,
        card.avatar,
        card.tier.as_str(),
        card.show_image,
        card.archived,
        display_index,
        gen_prompt,
        card.public_md,
        card.private_md
    )
}

/// 舊卡沒有 display_index：整批依目前顯示順序補齊，免得只有被存到的那張拿到索引而跳到最前
fn ensure_display_indices(root: &Path, world_id: &str) -> DataResult<()> {
    let existing = list_characters(root, world_id)?;
    if existing.iter().all(|meta| meta.display_index.is_some()) {
        return Ok(());
    }
    let ids: Vec<String> = existing.into_iter().map(|meta| meta.id).collect();
    reorder_characters(root, world_id, &ids)
}

/// 已存在的卡保留原位，新卡排到最後
fn display_index_for(root: &Path, world_id: &str, path: &Path) -> DataResult<u32> {
    if path.exists() {
        let contents = fs::read_to_string(path)?;
        if let Some(index) = parse_frontmatter(&contents)?.0.display_index {
            return Ok(index);
        }
    }
    Ok(list_characters(root, world_id)?
        .iter()
        .filter_map(|meta| meta.display_index)
        .max()
        .map_or(0, |max| max.saturating_add(1)))
}

/// 解析失敗（含缺 id 的舊卡）一律略過該檔，不中斷整份清單（舊資料不遷移、不偵測）。
pub fn list_characters(root: &Path, world_id: &str) -> DataResult<Vec<CharacterMeta>> {
    let directory = world_dir(root, world_id)?.join("characters");
    if !directory.exists() {
        return Ok(Vec::new());
    }

    let player_card_id = read_state(root, world_id)
        .ok()
        .and_then(|state| state.player_card_id);
    let mut characters = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry.path().extension().and_then(|value| value.to_str()) == Some("md")
        {
            let contents = fs::read_to_string(entry.path())?;
            match parse_frontmatter(&contents) {
                Ok((meta, _, _)) if player_card_id.as_deref() != Some(&meta.id) => {
                    characters.push(meta)
                }
                Ok(_) => {}
                Err(error) => {
                    eprintln!("略過無法解析的角色卡 {}: {error}", entry.path().display())
                }
            }
        }
    }
    // 沒有 display_index 的舊卡排在有索引的之後，彼此依名字排
    characters.sort_by(|left, right| {
        left.display_index
            .unwrap_or(u32::MAX)
            .cmp(&right.display_index.unwrap_or(u32::MAX))
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(characters)
}

/// 側欄拖曳排序：ids 就是新的顯示順序，沒送到的（如封存角色）依原順序接在後面
pub fn reorder_characters(root: &Path, world_id: &str, ids: &[String]) -> DataResult<()> {
    let existing = list_characters(root, world_id)?;
    let mut ordered: Vec<&str> = Vec::with_capacity(existing.len());
    for id in ids {
        if existing.iter().any(|meta| &meta.id == id) && !ordered.contains(&id.as_str()) {
            ordered.push(id);
        }
    }
    for meta in &existing {
        if !ordered.contains(&meta.id.as_str()) {
            ordered.push(&meta.id);
        }
    }

    for (index, id) in ordered.iter().enumerate() {
        let index =
            u32::try_from(index).map_err(|_| invalid_data("character display_index overflow"))?;
        let card = read_character(root, world_id, id)?;
        fs::write(
            character_path(root, world_id, id)?,
            serialize_character(&card, index),
        )?;
    }
    Ok(())
}

pub fn read_character(
    root: &Path,
    world_id: &str,
    character_id: &str,
) -> DataResult<CharacterCard> {
    let contents = fs::read_to_string(character_path(root, world_id, character_id)?)?;
    let (meta, gen_prompt, body) = parse_frontmatter(&contents)?;
    let (public_md, private_md) = parse_sections(body);
    Ok(CharacterCard {
        id: meta.id,
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

pub fn read_player_card(root: &Path, world_id: &str) -> DataResult<Option<CharacterCard>> {
    let Some(character_id) = read_state(root, world_id)
        .ok()
        .and_then(|state| state.player_card_id)
    else {
        return Ok(None);
    };
    let Ok(path) = character_path(root, world_id, &character_id) else {
        return Ok(None);
    };
    if !path.is_file() {
        return Ok(None);
    }
    read_character(root, world_id, &character_id).map(Some)
}

/// id 由呼叫端先跟 new_id 要好（草稿期生圖需要落在正確的圖庫路徑）；空 id 直接回錯。
pub fn write_character(root: &Path, world_id: &str, card: &CharacterCard) -> DataResult<()> {
    validate_id(&card.id)?;
    validate_single_line("name", &card.name)?;
    validate_single_line("color", &card.color)?;
    validate_single_line("avatar", &card.avatar)?;
    let path = character_path(root, world_id, &card.id)?;
    ensure_display_indices(root, world_id)?;
    let display_index = display_index_for(root, world_id, &path)?;
    fs::write(path, serialize_character(card, display_index))?;
    Ok(())
}

pub fn set_character_archived(
    root: &Path,
    world_id: &str,
    character_id: &str,
    archived: bool,
) -> DataResult<()> {
    let mut card = read_character(root, world_id, character_id)?;
    card.archived = archived;
    write_character(root, world_id, &card)
}

pub fn delete_character(root: &Path, world_id: &str, character_id: &str) -> DataResult<()> {
    let path = character_path(root, world_id, character_id)?;
    fs::remove_file(&path)?;
    let gallery = gallery_dir(root, world_id, character_id)?;
    if gallery.exists() {
        fs::remove_dir_all(gallery)?;
    }
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

fn transcript_path(root: &Path, world_id: &str, scene: u64) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?
        .join("transcript")
        .join(format!("{scene}.jsonl")))
}

pub fn append_transcript(
    root: &Path,
    world_id: &str,
    scene: u64,
    event: &TranscriptEvent,
) -> DataResult<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(transcript_path(root, world_id, scene)?)?;
    serde_json::to_writer(&mut file, event)?;
    file.write_all(b"\n")?;
    Ok(())
}

pub fn read_transcript(
    root: &Path,
    world_id: &str,
    scene: u64,
) -> DataResult<Vec<TranscriptEvent>> {
    let path = transcript_path(root, world_id, scene)?;
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
                format!("**{}**: {}", event.speaker_name, event.text)
            } else {
                format!("**{}**：{}", event.speaker_name, event.text)
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

pub fn export_transcript_markdown(root: &Path, world_id: &str, lang: &str) -> DataResult<String> {
    let world_name = read_state(root, world_id)?.name;
    let transcript_dir = world_dir(root, world_id)?.join("transcript");
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
        format!("# {world_name} — Session Transcript\n\nExported: {timestamp}")
    } else {
        format!("# {world_name} 跑團紀錄\n\n匯出時間：{timestamp}")
    };
    let mut sections = Vec::new();
    for scene in scenes {
        let heading = if english {
            format!("## Scene {scene}")
        } else {
            format!("## 場景 {scene}")
        };
        let events = read_transcript(root, world_id, scene)?;
        sections.push(render_scene_section(&events, &heading, english));
    }

    Ok(format!("{title}\n\n{}\n", sections.join("\n\n")))
}

/// 匯出單一場景的紀錄，格式與整桌匯出一致，供「過去的場」單場匯出使用。
/// 場景不存在（無該檔）視為錯誤，避免誤匯出空白文件。
pub fn export_scene_markdown(
    root: &Path,
    world_id: &str,
    scene: u64,
    lang: &str,
) -> DataResult<String> {
    let path = transcript_path(root, world_id, scene)?;
    if !path.exists() {
        return Err(invalid_data(format!("場景 {scene} 不存在")));
    }

    let world_name = read_state(root, world_id)?.name;
    let english = lang == "en";
    let timestamp = local_timestamp()?;
    let title = if english {
        format!("# {world_name} — Scene {scene}\n\nExported: {timestamp}")
    } else {
        format!("# {world_name} 場景 {scene}\n\n匯出時間：{timestamp}")
    };
    let events = read_transcript(root, world_id, scene)?;
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
    world_id: &str,
    summary_text: &str,
    lang: &str,
    title: Option<&str>,
) -> DataResult<u64> {
    let mut state = read_state(root, world_id)?;
    let old_scene = state.current_scene;
    let next_scene = old_scene + 1;
    let text = if lang == "en" {
        format!("Previously:\n{summary_text}")
    } else {
        format!("【前情提要】\n{summary_text}")
    };
    append_transcript(
        root,
        world_id,
        next_scene,
        &TranscriptEvent {
            ts: local_timestamp()?,
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
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
    write_state(root, world_id, &state)?;
    Ok(next_scene)
}

pub fn read_state(root: &Path, world_id: &str) -> DataResult<WorldState> {
    let path = world_dir(root, world_id)?.join("state.json");
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

pub fn write_state(root: &Path, world_id: &str, state: &WorldState) -> DataResult<()> {
    fs::write(
        world_dir(root, world_id)?.join("state.json"),
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

pub fn validate_sponsor_pack(bytes: &[u8]) -> DataResult<()> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| invalid_data(format!("贊助包不是合法 JSON：{error}")))?;
    let object = value
        .as_object()
        .ok_or_else(|| invalid_data("贊助包必須是 JSON 物件"))?;

    if object.get("type").and_then(serde_json::Value::as_str)
        != Some("table-tavern-sponsor-pack")
    {
        return Err(invalid_data("贊助包的 type 不正確"));
    }

    if object
        .get("format")
        .and_then(serde_json::Value::as_u64)
        .is_none_or(|format| format == 0)
    {
        return Err(invalid_data("贊助包的 format 必須是正整數"));
    }

    Ok(())
}

pub fn sponsor_pack_active(root: &Path) -> bool {
    let Ok(entries) = fs::read_dir(root) else {
        return false;
    };

    entries.flatten().any(|entry| {
        entry.path().extension().is_some_and(|extension| extension == "ttpack")
            && fs::read(entry.path())
                .is_ok_and(|bytes| validate_sponsor_pack(&bytes).is_ok())
    })
}

pub fn install_sponsor_pack(root: &Path, bytes: &[u8]) -> DataResult<()> {
    validate_sponsor_pack(bytes)?;
    fs::create_dir_all(root)?;
    fs::write(root.join("sponsor-pack.ttpack"), bytes)?;
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

    fn character_card(id: &str, name: &str) -> CharacterCard {
        CharacterCard {
            id: id.to_owned(),
            name: name.to_owned(),
            color: "#333333".to_owned(),
            avatar: "🎭".to_owned(),
            tier: Tier::Balanced,
            show_image: true,
            archived: false,
            gen_prompt: String::new(),
            public_md: String::new(),
            private_md: String::new(),
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

    fn write_worldbook_fixture(root: &TestRoot, world_id: &str, entries: serde_json::Value) {
        fs::write(
            root.path()
                .join(format!("worlds/{world_id}/worldbook.json")),
            serde_json::to_string_pretty(&serde_json::json!({ "entries": entries })).unwrap(),
        )
        .unwrap();
    }

    fn read_worldbook_fixture(root: &TestRoot, world_id: &str) -> serde_json::Value {
        serde_json::from_str(
            &fs::read_to_string(
                root.path()
                    .join(format!("worlds/{world_id}/worldbook.json")),
            )
            .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn validates_a_valid_sponsor_pack() {
        assert!(validate_sponsor_pack(br#"{"type":"table-tavern-sponsor-pack","format":1}"#).is_ok());
    }

    #[test]
    fn rejects_sponsor_pack_with_wrong_type() {
        let error = validate_sponsor_pack(br#"{"type":"other-pack","format":1}"#)
            .unwrap_err()
            .to_string();
        assert!(error.contains("type"));
    }

    #[test]
    fn rejects_sponsor_pack_without_format() {
        let error = validate_sponsor_pack(br#"{"type":"table-tavern-sponsor-pack"}"#)
            .unwrap_err()
            .to_string();
        assert!(error.contains("format"));
    }

    #[test]
    fn install_sponsor_pack_activates_only_valid_packages() {
        let root = TestRoot::new("sponsor-pack");
        let empty_root = TestRoot::new("empty-sponsor-pack");
        let pack = br#"{"type":"table-tavern-sponsor-pack","format":1,"edition":"supporter"}"#;

        assert!(!sponsor_pack_active(empty_root.path()));
        install_sponsor_pack(root.path(), pack).unwrap();
        assert!(sponsor_pack_active(root.path()));
    }

    #[test]
    fn worldbook_missing_returns_empty_and_invalid_json_errors() {
        let root = TestRoot::new("worldbook-missing");
        let world_id = create_world(root.path(), "舊桌").unwrap();
        assert_eq!(read_worldbook(root.path(), &world_id).unwrap(), Vec::new());
        assert_eq!(
            serde_json::to_value(Visibility::Gm).unwrap(),
            serde_json::json!({"type": "gm"})
        );
        assert_eq!(
            serde_json::to_value(Visibility::Characters(vec!["角色代碼".to_owned()])).unwrap(),
            serde_json::json!({"type": "characters", "characters": ["角色代碼"]})
        );

        fs::write(
            root.path()
                .join(format!("worlds/{world_id}/worldbook.json")),
            "{broken",
        )
        .unwrap();
        assert!(read_worldbook(root.path(), &world_id).is_err());
    }

    #[test]
    fn imports_st_worldbook_losslessly_and_round_trips_export() {
        let root = TestRoot::new("worldbook-st-import");
        let source = create_world(root.path(), "來源").unwrap();
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
            import_worldbook(root.path(), &source, &imported.to_string()).unwrap(),
            2
        );

        let entries = read_worldbook(root.path(), &source).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].uid, 0);
        assert_eq!(entries[0].title, "龍");
        assert_eq!(entries[0].keys, ["dragon", "wyrm"]);
        assert_eq!(entries[0].visibility, Visibility::Gm);
        assert_eq!(entries[1].uid, 1);
        assert_eq!(entries[1].visibility, Visibility::Public);

        let raw: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(root.path().join(format!("worlds/{source}/worldbook.json")))
                .unwrap(),
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
        export_worldbook(root.path(), &source, &exported).unwrap();
        let destination = create_world(root.path(), "目的").unwrap();
        let exported_text = fs::read_to_string(exported).unwrap();
        assert_eq!(
            import_worldbook(root.path(), &destination, &exported_text).unwrap(),
            entries.len()
        );
        assert_eq!(
            read_worldbook(root.path(), &destination).unwrap().len(),
            entries.len()
        );
    }

    #[test]
    fn imports_character_book_mapping_and_appends_unique_uids() {
        let root = TestRoot::new("worldbook-character-book");
        let world_id = create_world(root.path(), "世界").unwrap();
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
        import_worldbook(root.path(), &world_id, &first.to_string()).unwrap();

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
            import_worldbook(root.path(), &world_id, &character_book.to_string()).unwrap(),
            2
        );

        let entries = read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(
            entries.iter().map(|entry| entry.uid).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert_eq!(entries[1].keys, ["gate"]);
        assert_eq!(entries[1].order, 42);
        assert!(entries[1].disabled);
        assert!(!entries[2].disabled);

        let raw: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(
                root.path()
                    .join(format!("worlds/{world_id}/worldbook.json")),
            )
            .unwrap(),
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
        let world_id = create_world(root.path(), "世界").unwrap();
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
        import_worldbook(root.path(), &world_id, &imported.to_string()).unwrap();

        let mut updated = worldbook_entry(0, "新標題");
        updated.visibility = Visibility::Characters(vec!["角色代碼".to_owned()]);
        assert_eq!(
            upsert_worldbook_entry(root.path(), &world_id, updated.clone()).unwrap(),
            0
        );
        let raw: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(
                root.path()
                    .join(format!("worlds/{world_id}/worldbook.json")),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(raw["entries"]["0"]["sticky"], 99);
        assert_eq!(raw["entries"]["0"]["comment"], "新標題");
        assert_eq!(
            raw["entries"]["0"]["extensions"]["table_tavern"]["visibility"]["characters"][0],
            "角色代碼"
        );

        let allocated =
            upsert_worldbook_entry(root.path(), &world_id, worldbook_entry(u64::MAX, "新增"))
                .unwrap();
        assert_eq!(allocated, 1);
        let raw: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(
                root.path()
                    .join(format!("worlds/{world_id}/worldbook.json")),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(raw["entries"]["1"]["selective"], true);
        assert_eq!(raw["entries"]["1"]["probability"], 100);
        assert_eq!(raw["entries"]["1"]["useProbability"], true);
        assert_eq!(raw["entries"]["1"]["depth"], 4);
        assert_eq!(raw["entries"]["1"]["displayIndex"], 0);

        delete_worldbook_entry(root.path(), &world_id, 0).unwrap();
        assert_eq!(
            read_worldbook(root.path(), &world_id)
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
        let world_id = create_world(root.path(), "世界").unwrap();
        write_worldbook_fixture(
            &root,
            &world_id,
            serde_json::json!({
                "10": {
                    "uid": 10, "comment": "甲", "order": 10, "displayIndex": 0
                },
                "20": {
                    "uid": 20, "comment": "乙", "order": 20, "displayIndex": 1
                }
            }),
        );

        let uid = upsert_worldbook_entry(root.path(), &world_id, worldbook_entry(u64::MAX, "新增"))
            .unwrap();
        assert_eq!(read_worldbook(root.path(), &world_id).unwrap()[0].uid, uid);
        let raw = read_worldbook_fixture(&root, &world_id);
        assert_eq!(raw["entries"]["10"]["displayIndex"], 1);
        assert_eq!(raw["entries"]["20"]["displayIndex"], 2);
        assert_eq!(raw["entries"][uid.to_string()]["displayIndex"], 0);
    }

    #[test]
    fn reordering_worldbook_entries_applies_the_given_order() {
        let root = TestRoot::new("worldbook-reorder");
        let world_id = create_world(root.path(), "世界").unwrap();
        write_worldbook_fixture(
            &root,
            &world_id,
            serde_json::json!({
                "0": {"uid": 0, "comment": "甲", "displayIndex": 0},
                "1": {"uid": 1, "comment": "乙", "displayIndex": 1},
                "2": {"uid": 2, "comment": "丙", "displayIndex": 2}
            }),
        );

        // 跨多格拖曳：最後一筆拉到最前
        reorder_worldbook_entries(root.path(), &world_id, &[2, 0, 1]).unwrap();
        assert_eq!(
            read_worldbook(root.path(), &world_id)
                .unwrap()
                .iter()
                .map(|entry| entry.uid)
                .collect::<Vec<_>>(),
            [2, 0, 1]
        );
        reorder_worldbook_entries(root.path(), &world_id, &[0, 1, 2]).unwrap();
        assert_eq!(
            read_worldbook(root.path(), &world_id)
                .unwrap()
                .iter()
                .map(|entry| entry.uid)
                .collect::<Vec<_>>(),
            [0, 1, 2]
        );
    }

    #[test]
    fn reordering_worldbook_keeps_unlisted_entries_after_the_listed_ones() {
        let root = TestRoot::new("worldbook-reorder-partial");
        let world_id = create_world(root.path(), "世界").unwrap();
        write_worldbook_fixture(
            &root,
            &world_id,
            serde_json::json!({
                "0": {"uid": 0, "comment": "甲", "displayIndex": 0},
                "1": {"uid": 1, "comment": "乙", "displayIndex": 1},
                "2": {"uid": 2, "comment": "丙", "displayIndex": 2}
            }),
        );

        // uid 9 不存在應被忽略；沒送到的 0 依原順序接在後面
        reorder_worldbook_entries(root.path(), &world_id, &[2, 9, 1]).unwrap();

        assert_eq!(
            read_worldbook(root.path(), &world_id)
                .unwrap()
                .iter()
                .map(|entry| entry.uid)
                .collect::<Vec<_>>(),
            [2, 1, 0]
        );
    }

    #[test]
    fn reordering_legacy_worldbook_entries_normalizes_display_indices() {
        let root = TestRoot::new("worldbook-reorder-legacy");
        let world_id = create_world(root.path(), "世界").unwrap();
        write_worldbook_fixture(
            &root,
            &world_id,
            serde_json::json!({
                "7": {"uid": 7, "comment": "丙"},
                "3": {"uid": 3, "comment": "甲"},
                "5": {"uid": 5, "comment": "乙"}
            }),
        );

        reorder_worldbook_entries(root.path(), &world_id, &[5, 3, 7]).unwrap();

        assert_eq!(
            read_worldbook(root.path(), &world_id)
                .unwrap()
                .iter()
                .map(|entry| entry.uid)
                .collect::<Vec<_>>(),
            [5, 3, 7]
        );
        let raw = read_worldbook_fixture(&root, &world_id);
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
    fn reordering_worldbook_entries_preserves_order_and_unknown_fields() {
        let root = TestRoot::new("worldbook-reorder-lossless");
        let world_id = create_world(root.path(), "世界").unwrap();
        write_worldbook_fixture(
            &root,
            &world_id,
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

        reorder_worldbook_entries(root.path(), &world_id, &[1, 0]).unwrap();

        let raw = read_worldbook_fixture(&root, &world_id);
        assert_eq!(raw["entries"]["0"]["order"], 91);
        assert_eq!(raw["entries"]["1"]["order"], 7);
        assert_eq!(
            raw["entries"]["0"]["foreign"],
            serde_json::json!({"nested": true})
        );
        assert_eq!(raw["entries"]["1"]["sticky"], 42);
    }

    /// 測試清單 #1：create_world 回 id；state.json 含 id/name；list_worlds 回 WorldMeta
    #[test]
    fn create_world_returns_id_with_state_and_meta() {
        let root = TestRoot::new("worlds");
        assert!(list_worlds(root.path()).unwrap().is_empty());

        let world_id = create_world(root.path(), "群島").unwrap();
        assert!(root.path().join("worlds").join(&world_id).is_dir());
        assert!(root
            .path()
            .join("worlds")
            .join(&world_id)
            .join("characters")
            .is_dir());
        assert!(root
            .path()
            .join("worlds")
            .join(&world_id)
            .join("transcript")
            .is_dir());
        assert!(root
            .path()
            .join("worlds")
            .join(&world_id)
            .join("world.md")
            .is_file());
        let state_raw: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(
                root.path()
                    .join("worlds")
                    .join(&world_id)
                    .join("state.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(state_raw["id"], world_id);
        assert_eq!(state_raw["name"], "群島");

        let worlds = list_worlds(root.path()).unwrap();
        assert_eq!(
            worlds,
            vec![WorldMeta {
                id: world_id,
                name: "群島".to_owned()
            }]
        );
    }

    #[test]
    fn list_characters_excludes_player_card() {
        let root = TestRoot::new("player-card");
        let world_id = create_world(root.path(), "玩家卡桌").unwrap();
        let player = character_card(&new_id(), "阿濤");
        let npc = character_card(&new_id(), "狐狸");
        write_character(root.path(), &world_id, &player).unwrap();
        write_character(root.path(), &world_id, &npc).unwrap();
        let mut state = read_state(root.path(), &world_id).unwrap();
        state.player_card_id = Some(player.id.clone());
        write_state(root.path(), &world_id, &state).unwrap();

        assert_eq!(
            list_characters(root.path(), &world_id).unwrap(),
            vec![CharacterMeta {
                id: npc.id,
                name: npc.name,
                color: npc.color,
                avatar: npc.avatar,
                tier: npc.tier,
                show_image: npc.show_image,
                archived: npc.archived,
                display_index: Some(1),
            }]
        );
        assert_eq!(
            read_player_card(root.path(), &world_id).unwrap(),
            Some(player)
        );
    }

    /// 測試清單 #2：兩桌同名可並存，各自獨立 id 與內容
    #[test]
    fn two_worlds_with_same_name_coexist_with_independent_ids() {
        let root = TestRoot::new("worlds-same-name");
        let first = create_world(root.path(), "同名桌").unwrap();
        let second = create_world(root.path(), "同名桌").unwrap();
        assert_ne!(first, second);

        write_world_md(root.path(), &first, "第一桌的設定").unwrap();
        write_world_md(root.path(), &second, "第二桌的設定").unwrap();
        assert_eq!(read_world_md(root.path(), &first).unwrap(), "第一桌的設定");
        assert_eq!(read_world_md(root.path(), &second).unwrap(), "第二桌的設定");

        let names: Vec<_> = list_worlds(root.path())
            .unwrap()
            .into_iter()
            .map(|meta| meta.name)
            .collect();
        assert_eq!(names, vec!["同名桌", "同名桌"]);
    }

    /// 測試清單 #3：rename_world 後目錄路徑不變，只有 state.json 的 name 變
    #[test]
    fn rename_world_keeps_directory_and_changes_only_name() {
        let root = TestRoot::new("rename-world");
        let world_id = create_world(root.path(), "舊名").unwrap();
        let directory = root.path().join("worlds").join(&world_id);
        assert!(directory.is_dir());

        rename_world(root.path(), &world_id, "新名").unwrap();

        assert!(directory.is_dir());
        assert_eq!(read_state(root.path(), &world_id).unwrap().name, "新名");
        assert_eq!(read_state(root.path(), &world_id).unwrap().id, world_id);

        assert!(rename_world(root.path(), &world_id, "含換行\n的名字").is_err());
    }

    #[test]
    fn sample_world_is_ready_to_play() {
        let root = TestRoot::new("sample-world");
        let world_id = create_sample_world(root.path(), "zh-TW").unwrap();

        let worlds = list_worlds(root.path()).unwrap();
        assert!(worlds.iter().any(|meta| meta.id == world_id));

        let characters = list_characters(root.path(), &world_id).unwrap();
        assert_eq!(characters.len(), 3);
        for name in ["狐狸", "騎士", "吟遊詩人"] {
            assert!(characters.iter().any(|character| character.name == name));
        }

        let world_md = read_world_md(root.path(), &world_id).unwrap();
        assert!(!world_md.is_empty());
        assert!(world_md.contains("霧口鎮"));

        let transcript = read_transcript(root.path(), &world_id, 0).unwrap();
        assert_eq!(transcript.len(), 1);
        assert_eq!(transcript[0].kind, TranscriptKind::Narration);
        assert_eq!(transcript[0].speaker_name, "GM");
        assert_eq!(transcript[0].speaker_id, "");

        // 測試清單 #13：重複呼叫要沿用既有那桌，不重複塞開場旁白
        assert_eq!(create_sample_world(root.path(), "zh-TW").unwrap(), world_id);
        assert_eq!(read_transcript(root.path(), &world_id, 0).unwrap().len(), 1);
        assert_eq!(list_worlds(root.path()).unwrap().len(), worlds.len());
    }

    #[test]
    fn sample_world_english_content_follows_lang() {
        let root = TestRoot::new("sample-world-en");
        let world_id = create_sample_world(root.path(), "en").unwrap();

        let characters = list_characters(root.path(), &world_id).unwrap();
        assert_eq!(characters.len(), 3);
        for name in ["Fox", "Knight", "Bard"] {
            assert!(characters.iter().any(|character| character.name == name));
        }
        assert!(read_world_md(root.path(), &world_id)
            .unwrap()
            .contains("Mistmouth"));
        let transcript = read_transcript(root.path(), &world_id, 0).unwrap();
        assert!(transcript[0].text.starts_with("Rain hammers"));
    }

    #[cfg(unix)]
    #[test]
    fn lists_worlds_by_last_activity_descending() {
        let root = TestRoot::new("activity");
        let first = create_world(root.path(), "甲桌").unwrap();
        let second = create_world(root.path(), "乙桌").unwrap();

        // 兩桌目錄 mtime 撥回一小時前：同時間時按顯示名升冪（乙 U+4E59 < 甲 U+7532）
        let hour_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        for id in [&first, &second] {
            let directory = fs::File::open(root.path().join("worlds").join(id)).unwrap();
            directory.set_modified(hour_ago).unwrap();
        }
        assert_eq!(
            list_worlds(root.path())
                .unwrap()
                .into_iter()
                .map(|meta| meta.name)
                .collect::<Vec<_>>(),
            vec!["乙桌", "甲桌"]
        );

        // 對名稱排序居後的甲桌寫一筆訊息，活動排序應把它推到最前
        let event = TranscriptEvent {
            ts: "2026-07-19T00:00:00Z".to_owned(),
            speaker_id: String::new(),
            speaker_name: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "你好".to_owned(),
        };
        append_transcript(root.path(), &first, 0, &event).unwrap();
        assert_eq!(
            list_worlds(root.path())
                .unwrap()
                .into_iter()
                .map(|meta| meta.name)
                .collect::<Vec<_>>(),
            vec!["甲桌", "乙桌"]
        );
    }

    #[test]
    fn reclaims_only_untouched_worlds() {
        let root = TestRoot::new("reclaim");
        let empty = create_world(root.path(), "空桌").unwrap();
        assert!(reclaim_world_if_empty(root.path(), &empty).unwrap());
        assert!(list_worlds(root.path()).unwrap().is_empty());
        // 已刪的桌再回收一次應為 no-op
        assert!(!reclaim_world_if_empty(root.path(), &empty).unwrap());

        let has_message = create_world(root.path(), "有訊息").unwrap();
        let event = TranscriptEvent {
            ts: "2026-07-19T00:00:00Z".to_owned(),
            speaker_id: String::new(),
            speaker_name: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "留著".to_owned(),
        };
        append_transcript(root.path(), &has_message, 0, &event).unwrap();
        assert!(!reclaim_world_if_empty(root.path(), &has_message).unwrap());

        let has_character = create_world(root.path(), "有角色").unwrap();
        write_character(
            root.path(),
            &has_character,
            &character_card(&new_id(), "旅人"),
        )
        .unwrap();
        assert!(!reclaim_world_if_empty(root.path(), &has_character).unwrap());

        let has_setting = create_world(root.path(), "有設定").unwrap();
        write_world_md(root.path(), &has_setting, "海島世界").unwrap();
        assert!(!reclaim_world_if_empty(root.path(), &has_setting).unwrap());
    }

    #[test]
    fn delete_world_removes_directory_including_gallery() {
        let root = TestRoot::new("delete-world");
        let to_delete = create_world(root.path(), "要刪的桌").unwrap();
        let to_keep = create_world(root.path(), "留著的桌").unwrap();
        let character_id = new_id();
        let gallery = gallery_dir(root.path(), &to_delete, &character_id).unwrap();
        fs::create_dir_all(&gallery).unwrap();
        fs::write(gallery.join("1.png"), b"gen").unwrap();

        delete_world(root.path(), &to_delete).unwrap();

        assert_eq!(
            list_worlds(root.path())
                .unwrap()
                .into_iter()
                .map(|meta| meta.id)
                .collect::<Vec<_>>(),
            vec![to_keep]
        );
        assert!(!root.path().join("worlds").join(&to_delete).exists());
        // 已刪的桌再刪一次應為 no-op；非法 id 擋下
        delete_world(root.path(), &to_delete).unwrap();
        assert!(delete_world(root.path(), "not-a-valid-ulid").is_err());
    }

    #[test]
    fn rejects_multiline_frontmatter_values() {
        let root = TestRoot::new("scalars");
        let world_id = create_world(root.path(), "世界").unwrap();
        let mut card = character_card(&new_id(), "角色");
        card.color = "#123456\ntier: best".to_owned();
        assert!(write_character(root.path(), &world_id, &card).is_err());
    }

    /// 測試清單 #8：顯示名放行含 /、..、開頭 .、保留字 GM；含換行仍擋
    #[test]
    fn display_names_allow_special_characters_but_reject_newlines() {
        let root = TestRoot::new("display-names");
        let world_id = create_world(root.path(), "世界").unwrap();
        for name in ["../evil", "a/b", ".hidden", "", "GM"] {
            let card = character_card(&new_id(), name);
            write_character(root.path(), &world_id, &card).unwrap();
            let read_back = read_character(root.path(), &world_id, &card.id).unwrap();
            assert_eq!(read_back.name, name);
        }

        let mut newline_card = character_card(&new_id(), "壞名字");
        newline_card.name = "含換行\n的名字".to_owned();
        assert!(write_character(root.path(), &world_id, &newline_card).is_err());

        // 世界名同樣只擋換行
        let odd_world = create_world(root.path(), "../also/fine").unwrap();
        assert_eq!(
            read_state(root.path(), &odd_world).unwrap().name,
            "../also/fine"
        );
    }

    /// 測試清單 #9：world_id／character_id 路徑逃逸一律被 validate_id 擋
    #[test]
    fn validate_id_rejects_path_escaping_ids() {
        let root = TestRoot::new("escape");
        for bad_id in ["../x", "a/b", "", "short", &"A".repeat(27)] {
            assert!(
                read_world_md(root.path(), bad_id).is_err(),
                "accepted world id {bad_id:?}"
            );
        }

        let world_id = create_world(root.path(), "世界").unwrap();
        for bad_id in ["../x", "a/b", "", "short"] {
            assert!(
                read_character(root.path(), &world_id, bad_id).is_err(),
                "accepted character id {bad_id:?}"
            );
            let mut card = character_card(&new_id(), "角色");
            card.id = bad_id.to_owned();
            assert!(
                write_character(root.path(), &world_id, &card).is_err(),
                "accepted character id {bad_id:?} on write"
            );
        }
    }

    #[test]
    fn character_round_trip_preserves_fields_and_sections() {
        let root = TestRoot::new("character");
        let world_id = create_world(root.path(), "港灣").unwrap();
        let character_id = new_id();
        let card = CharacterCard {
            id: character_id.clone(),
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

        write_character(root.path(), &world_id, &card).unwrap();
        assert_eq!(
            read_character(root.path(), &world_id, &character_id).unwrap(),
            card
        );
        assert_eq!(
            list_characters(root.path(), &world_id).unwrap(),
            vec![CharacterMeta {
                id: character_id.clone(),
                name: "阿藍".to_owned(),
                color: "#3366ff".to_owned(),
                avatar: "avatars/blue.png".to_owned(),
                tier: Tier::Best,
                show_image: true,
                archived: true,
                display_index: Some(0),
            }]
        );

        let raw = fs::read_to_string(
            root.path()
                .join(format!("worlds/{world_id}/characters/{character_id}.md")),
        )
        .unwrap();
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
                "id",
                "name",
                "color",
                "avatar",
                "tier",
                "show_image",
                "archived",
                "display_index",
                "gen_prompt"
            ]
        );
        assert!(raw.contains("\n## 公開\n"));
        assert!(raw.contains("\n## 私有\n"));

        set_character_archived(root.path(), &world_id, &character_id, false).unwrap();
        assert!(
            !read_character(root.path(), &world_id, &character_id)
                .unwrap()
                .archived
        );
    }

    #[test]
    fn show_image_false_round_trips() {
        let root = TestRoot::new("show-image");
        let world_id = create_world(root.path(), "世界").unwrap();
        let mut card = character_card(&new_id(), "藏圖");
        card.show_image = false;
        write_character(root.path(), &world_id, &card).unwrap();
        assert!(
            !read_character(root.path(), &world_id, &card.id)
                .unwrap()
                .show_image
        );
    }

    /// 測試清單 #7：舊格式資料（缺 id）被略過，不會炸掉整份清單
    #[test]
    fn legacy_cards_and_worlds_without_id_are_skipped() {
        let root = TestRoot::new("legacy-skip");
        let world_id = create_world(root.path(), "世界").unwrap();

        // 舊卡沒有 id：list_characters 略過該檔，直接讀取也是錯
        fs::write(
            root.path()
                .join(format!("worlds/{world_id}/characters/舊卡.md")),
            "---\nname: 舊卡\ncolor: #111111\navatar: 🎭\ntier: default\n---\n## 公開\n\n## 私有\n",
        )
        .unwrap();
        // 有 id 的正常卡應該仍被列出
        let good_card = character_card(&new_id(), "正常卡");
        write_character(root.path(), &world_id, &good_card).unwrap();

        let characters = list_characters(root.path(), &world_id).unwrap();
        assert_eq!(characters.len(), 1);
        assert_eq!(characters[0].name, "正常卡");

        // 舊桌沒有 id/name：list_worlds 略過該桌
        let legacy_world_dir = root.path().join("worlds").join(new_id());
        fs::create_dir_all(legacy_world_dir.join("characters")).unwrap();
        fs::create_dir_all(legacy_world_dir.join("transcript")).unwrap();
        fs::write(
            legacy_world_dir.join("state.json"),
            serde_json::json!({ "current_scene": 0 }).to_string(),
        )
        .unwrap();

        let worlds = list_worlds(root.path()).unwrap();
        assert_eq!(worlds.len(), 1);
        assert_eq!(worlds[0].id, world_id);
    }

    #[test]
    fn delete_character_removes_card_images_and_gallery() {
        let root = TestRoot::new("delete-character");
        let world_id = create_world(root.path(), "世界").unwrap();
        let card = character_card(&new_id(), "退場角色");
        write_character(root.path(), &world_id, &card).unwrap();
        let md_path = character_path(root.path(), &world_id, &card.id).unwrap();
        let png_path = md_path.with_extension("png");
        let avatar_path = md_path.with_extension("avatar.png");
        fs::write(&png_path, b"png").unwrap();
        fs::write(&avatar_path, b"avatar").unwrap();
        let gallery = gallery_dir(root.path(), &world_id, &card.id).unwrap();
        fs::create_dir_all(&gallery).unwrap();
        fs::write(gallery.join("1.png"), b"gen").unwrap();
        // 生成圖庫收在世界目錄內，不是放錯層的舊路徑
        assert!(gallery.starts_with(root.path().join("worlds").join(&world_id)));

        delete_character(root.path(), &world_id, &card.id).unwrap();

        assert!(list_characters(root.path(), &world_id).unwrap().is_empty());
        assert!(!md_path.exists());
        assert!(!png_path.exists());
        assert!(!avatar_path.exists());
        assert!(!gallery.exists());
    }

    /// 測試清單 #4：兩個同名角色可並存，各自讀寫互不干擾
    #[test]
    fn two_characters_with_same_name_coexist_independently() {
        let root = TestRoot::new("same-name-characters");
        let world_id = create_world(root.path(), "世界").unwrap();
        let mut first = character_card(&new_id(), "重名");
        first.public_md = "第一位".to_owned();
        let mut second = character_card(&new_id(), "重名");
        second.public_md = "第二位".to_owned();
        write_character(root.path(), &world_id, &first).unwrap();
        write_character(root.path(), &world_id, &second).unwrap();

        assert_eq!(
            read_character(root.path(), &world_id, &first.id)
                .unwrap()
                .public_md,
            "第一位"
        );
        assert_eq!(
            read_character(root.path(), &world_id, &second.id)
                .unwrap()
                .public_md,
            "第二位"
        );
        assert_eq!(list_characters(root.path(), &world_id).unwrap().len(), 2);
    }

    /// 測試清單 #5：改名（＝重存卡片）後路徑全部不變，transcript 舊事件保留舊名快照，
    /// model_bindings 不需改動仍指向同一角色
    #[test]
    fn rename_keeps_paths_and_preserves_transcript_snapshot() {
        let root = TestRoot::new("rename-character");
        let world_id = create_world(root.path(), "世界").unwrap();
        let mut card = character_card(&new_id(), "舊名");
        card.tier = Tier::Best;
        card.public_md = "舊名是個吟遊詩人".to_owned();
        write_character(root.path(), &world_id, &card).unwrap();
        let md_path = character_path(root.path(), &world_id, &card.id).unwrap();
        fs::write(md_path.with_extension("png"), b"png").unwrap();
        fs::write(md_path.with_extension("avatar.png"), b"avatar").unwrap();
        let gallery = gallery_dir(root.path(), &world_id, &card.id).unwrap();
        fs::create_dir_all(&gallery).unwrap();
        fs::write(gallery.join("1.png"), b"gen").unwrap();
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                ts: "2026-07-27 12:00".to_owned(),
                speaker_id: card.id.clone(),
                speaker_name: "舊名".to_owned(),
                kind: TranscriptKind::Dialogue,
                text: "舊名說了一句話".to_owned(),
            },
        )
        .unwrap();
        let mut entry = worldbook_entry(0, "條目");
        entry.visibility = Visibility::Characters(vec![card.id.clone(), "別的代碼".to_owned()]);
        upsert_worldbook_entry(root.path(), &world_id, entry).unwrap();
        let mut state = read_state(root.path(), &world_id).unwrap();
        state
            .model_bindings
            .insert(card.id.clone(), "model-x".to_owned());
        write_state(root.path(), &world_id, &state).unwrap();

        // 改名＝存一次卡片（前端就是這條路徑），id 不變所以什麼都不用搬
        let mut renamed_card = read_character(root.path(), &world_id, &card.id).unwrap();
        renamed_card.name = "新名".to_owned();
        write_character(root.path(), &world_id, &renamed_card).unwrap();

        let renamed = read_character(root.path(), &world_id, &card.id).unwrap();
        assert_eq!(renamed.name, "新名");
        assert_eq!(renamed.tier, Tier::Best);
        // 自然語言內文不動（拍板：機械取代會誤傷）
        assert_eq!(renamed.public_md, "舊名是個吟遊詩人");
        // 路徑全部不變——id 沒變，改名不搬檔
        assert!(md_path.exists());
        assert!(md_path.with_extension("png").exists());
        assert!(md_path.with_extension("avatar.png").exists());
        assert!(gallery.join("1.png").exists());

        // 改名後舊對話仍顯示舊名快照（2026-07-27 拍板）
        let events = read_transcript(root.path(), &world_id, 0).unwrap();
        assert_eq!(events[0].speaker_id, card.id);
        assert_eq!(events[0].speaker_name, "舊名");
        assert_eq!(events[0].text, "舊名說了一句話");

        // 世界書可見性存 id，改名後條目不需回填仍可見（測試清單 #11）
        assert_eq!(
            read_worldbook(root.path(), &world_id).unwrap()[0].visibility,
            Visibility::Characters(vec![card.id.clone(), "別的代碼".to_owned()])
        );
        assert_eq!(
            read_state(root.path(), &world_id)
                .unwrap()
                .model_bindings
                .get(&card.id)
                .map(String::as_str),
            Some("model-x")
        );
    }

    #[test]
    fn rename_rejects_bad_id_and_multiline_name_but_allows_duplicate() {
        let root = TestRoot::new("rename-character-guard");
        let world_id = create_world(root.path(), "世界").unwrap();
        let mut first = character_card(&new_id(), "甲");
        let second = character_card(&new_id(), "乙");
        write_character(root.path(), &world_id, &first).unwrap();
        write_character(root.path(), &world_id, &second).unwrap();

        let mut bad_id = first.clone();
        bad_id.id = "not-a-real-id".to_owned();
        assert!(write_character(root.path(), &world_id, &bad_id).is_err());
        let mut multiline = first.clone();
        multiline.name = "含換行\n的名字".to_owned();
        assert!(write_character(root.path(), &world_id, &multiline).is_err());

        // 同名不再擋——甲可以改名成跟乙一樣
        first.name = "乙".to_owned();
        write_character(root.path(), &world_id, &first).unwrap();
        assert_eq!(
            read_character(root.path(), &world_id, &first.id)
                .unwrap()
                .name,
            "乙"
        );
    }

    /// 測試清單 #10：reorder_characters 以 id 排序，封存角色仍接在後面
    #[test]
    fn reordering_characters_by_id_keeps_unlisted_after_listed() {
        let root = TestRoot::new("character-reorder");
        let world_id = create_world(root.path(), "世界").unwrap();
        let cards: Vec<_> = ["甲", "乙", "丙"]
            .into_iter()
            .map(|name| character_card(&new_id(), name))
            .collect();
        for card in &cards {
            write_character(root.path(), &world_id, card).unwrap();
        }
        let ids = |root: &Path| {
            list_characters(root, &world_id)
                .unwrap()
                .into_iter()
                .map(|meta| meta.id)
                .collect::<Vec<_>>()
        };
        // 建卡順序即初始順序
        assert_eq!(
            ids(root.path()),
            vec![
                cards[0].id.clone(),
                cards[1].id.clone(),
                cards[2].id.clone()
            ]
        );

        reorder_characters(
            root.path(),
            &world_id,
            &[cards[2].id.clone(), cards[0].id.clone()],
        )
        .unwrap();
        // 沒送到的「乙」接在後面
        assert_eq!(
            ids(root.path()),
            vec![
                cards[2].id.clone(),
                cards[0].id.clone(),
                cards[1].id.clone()
            ]
        );

        // 改名不動排序位置
        let mut renamed = read_character(root.path(), &world_id, &cards[0].id).unwrap();
        renamed.name = "甲二".to_owned();
        write_character(root.path(), &world_id, &renamed).unwrap();
        assert_eq!(
            ids(root.path()),
            vec![
                cards[2].id.clone(),
                cards[0].id.clone(),
                cards[1].id.clone()
            ]
        );

        // 重存不動位置，新卡排到最後
        set_character_archived(root.path(), &world_id, &cards[2].id, true).unwrap();
        let fourth = character_card(&new_id(), "丁");
        write_character(root.path(), &world_id, &fourth).unwrap();
        assert_eq!(
            ids(root.path()),
            vec![
                cards[2].id.clone(),
                cards[0].id.clone(),
                cards[1].id.clone(),
                fourth.id.clone()
            ]
        );
    }

    #[test]
    fn saving_one_card_without_display_index_does_not_reshuffle_the_others() {
        let root = TestRoot::new("character-legacy-order");
        let world_id = create_world(root.path(), "世界").unwrap();
        let ids: Vec<String> = ["甲", "乙", "丙"]
            .into_iter()
            .map(|name| {
                let id = new_id();
                // 有 id 但沒有 display_index，模擬這個欄位加入前存的卡
                fs::write(
                    root.path()
                        .join(format!("worlds/{world_id}/characters/{id}.md")),
                    format!(
                        "---\nid: {id}\nname: {name}\ncolor: #000000\navatar: 🎭\ntier: default\n---\n## 公開\n"
                    ),
                )
                .unwrap();
                id
            })
            .collect();
        // 沒有 display_index 的卡，顯示順序＝名字排序
        let names = |root: &Path| {
            list_characters(root, &world_id)
                .unwrap()
                .into_iter()
                .map(|meta| meta.name)
                .collect::<Vec<_>>()
        };
        assert_eq!(names(root.path()), ["丙", "乙", "甲"]);

        set_character_archived(root.path(), &world_id, &ids[0], true).unwrap();

        assert_eq!(names(root.path()), ["丙", "乙", "甲"]);
    }

    #[test]
    fn frontmatter_accepts_spacing_and_order_but_rejects_invalid_tier() {
        let root = TestRoot::new("frontmatter");
        let world_id = create_world(root.path(), "世界").unwrap();
        let character_id = new_id();
        let path = root
            .path()
            .join(format!("worlds/{world_id}/characters/{character_id}.md"));
        fs::write(
            &path,
            format!(
                "---\ntier : fast\nunknown: ignored\navatar: 🐕\n color : #abcdef\nname : 角色\nid : {character_id}\n---\n## 私有\n私密"
            ),
        )
        .unwrap();
        assert_eq!(
            read_character(root.path(), &world_id, &character_id)
                .unwrap()
                .tier,
            Tier::Fast
        );

        fs::write(
            path,
            format!(
                "---\nid: {character_id}\nname: 角色\ncolor: #abcdef\navatar: 🐕\ntier: impossible\n---\n"
            ),
        )
        .unwrap();
        assert!(read_character(root.path(), &world_id, &character_id).is_err());
    }

    #[test]
    fn transcript_round_trip_is_ordered_jsonl_and_rejects_invalid_kind() {
        let root = TestRoot::new("transcript");
        let world_id = create_world(root.path(), "劇場").unwrap();
        let events = vec![
            TranscriptEvent {
                ts: "2026-07-19T10:00:00+08:00".to_owned(),
                speaker_id: String::new(),
                speaker_name: "旁白".to_owned(),
                kind: TranscriptKind::Narration,
                text: "序幕".to_owned(),
            },
            TranscriptEvent {
                ts: "2026-07-19T10:00:01+08:00".to_owned(),
                speaker_id: String::new(),
                speaker_name: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "第一行\n仍是同一事件".to_owned(),
            },
            TranscriptEvent {
                ts: "2026-07-19T10:00:02+08:00".to_owned(),
                speaker_id: "角色代碼".to_owned(),
                speaker_name: "角色".to_owned(),
                kind: TranscriptKind::Dialogue,
                text: "你好".to_owned(),
            },
        ];
        for event in &events {
            append_transcript(root.path(), &world_id, 7, event).unwrap();
        }
        assert_eq!(read_transcript(root.path(), &world_id, 7).unwrap(), events);

        let path = root
            .path()
            .join(format!("worlds/{world_id}/transcript/7.jsonl"));
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
            .write_all(b"{\"ts\":\"now\",\"speaker_id\":\"\",\"speaker_name\":\"x\",\"kind\":\"bad\",\"text\":\"x\"}\n")
            .unwrap();
        let error = read_transcript(root.path(), &world_id, 7)
            .unwrap_err()
            .to_string();
        assert!(error.contains("line 4"), "{error}");
    }

    #[test]
    fn exports_all_transcript_scenes_as_localized_markdown() {
        let root = TestRoot::new("transcript-export");
        let world_id = create_world(root.path(), "海風桌").unwrap();
        for (scene, event) in [
            (
                1,
                TranscriptEvent {
                    ts: "now".to_owned(),
                    speaker_id: "船長代碼".to_owned(),
                    speaker_name: "船長".to_owned(),
                    kind: TranscriptKind::Dialogue,
                    text: "我們啟航。".to_owned(),
                },
            ),
            (
                0,
                TranscriptEvent {
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: TranscriptKind::Narration,
                    text: "霧氣升起。\n港口安靜。".to_owned(),
                },
            ),
            (
                1,
                TranscriptEvent {
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "玩家".to_owned(),
                    kind: TranscriptKind::Player,
                    text: "我登上甲板。".to_owned(),
                },
            ),
            (
                0,
                TranscriptEvent {
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: TranscriptKind::System,
                    text: "第一幕開始".to_owned(),
                },
            ),
        ] {
            append_transcript(root.path(), &world_id, scene, &event).unwrap();
        }

        let zh = export_transcript_markdown(root.path(), &world_id, "zh-TW").unwrap();
        assert!(zh.starts_with("# 海風桌 跑團紀錄\n\n匯出時間："));
        assert!(zh.find("## 場景 0").unwrap() < zh.find("## 場景 1").unwrap());
        assert!(zh.contains("> 霧氣升起。\n> 港口安靜。"));
        assert!(zh.contains("*（第一幕開始）*"));
        assert!(zh.contains("**玩家**：我登上甲板。"));
        assert!(zh.contains("**船長**：我們啟航。"));

        let en = export_transcript_markdown(root.path(), &world_id, "en").unwrap();
        assert!(en.starts_with("# 海風桌 — Session Transcript\n\nExported: "));
        assert!(en.contains("## Scene 0"));
        assert!(en.contains("## Scene 1"));
        assert!(en.contains("**船長**: 我們啟航。"));
        assert!(en.contains("*(第一幕開始)*"));
    }

    #[test]
    fn transcript_export_rejects_a_world_without_scenes() {
        let root = TestRoot::new("empty-transcript-export");
        let world_id = create_world(root.path(), "空桌").unwrap();
        assert!(export_transcript_markdown(root.path(), &world_id, "zh-TW").is_err());
    }

    #[test]
    fn scene_export_contains_only_that_scenes_events() {
        let root = TestRoot::new("scene-export");
        let world_id = create_world(root.path(), "海風桌").unwrap();
        for (scene, event) in [
            (
                0,
                TranscriptEvent {
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: TranscriptKind::Narration,
                    text: "霧氣升起。".to_owned(),
                },
            ),
            (
                1,
                TranscriptEvent {
                    ts: "now".to_owned(),
                    speaker_id: "船長代碼".to_owned(),
                    speaker_name: "船長".to_owned(),
                    kind: TranscriptKind::Dialogue,
                    text: "我們啟航。".to_owned(),
                },
            ),
        ] {
            append_transcript(root.path(), &world_id, scene, &event).unwrap();
        }

        let zh = export_scene_markdown(root.path(), &world_id, 0, "zh-TW").unwrap();
        assert!(zh.starts_with("# 海風桌 場景 0\n\n匯出時間："));
        assert!(zh.contains("> 霧氣升起。"));
        assert!(!zh.contains("船長"));

        let en = export_scene_markdown(root.path(), &world_id, 1, "en").unwrap();
        assert!(en.starts_with("# 海風桌 — Scene 1\n\nExported: "));
        assert!(en.contains("**船長**: 我們啟航。"));
        assert!(!en.contains("霧氣升起"));
    }

    #[test]
    fn scene_export_rejects_a_missing_scene() {
        let root = TestRoot::new("scene-export-missing");
        let world_id = create_world(root.path(), "空桌").unwrap();
        assert!(export_scene_markdown(root.path(), &world_id, 0, "zh-TW").is_err());
    }

    #[test]
    fn begin_next_scene_appends_summary_and_advances_scene() {
        let root = TestRoot::new("begin-next-scene");
        let world_id = create_world(root.path(), "換場桌").unwrap();
        let event = TranscriptEvent {
            ts: "2026-07-19T00:00:00Z".to_owned(),
            speaker_id: String::new(),
            speaker_name: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "第一場的對話".to_owned(),
        };
        append_transcript(root.path(), &world_id, 0, &event).unwrap();

        let next = begin_next_scene(root.path(), &world_id, "壓縮後的摘要", "zh-TW", None).unwrap();
        assert_eq!(next, 1);
        assert_eq!(read_state(root.path(), &world_id).unwrap().current_scene, 1);

        // 摘要落在新場景檔開頭，舊場景不受影響
        assert_eq!(read_transcript(root.path(), &world_id, 0).unwrap().len(), 1);
        let new_scene = read_transcript(root.path(), &world_id, 1).unwrap();
        assert_eq!(new_scene.len(), 1);
        assert_eq!(new_scene[0].speaker_name, "GM");
        assert_eq!(new_scene[0].speaker_id, "");
        assert_eq!(new_scene[0].kind, TranscriptKind::Narration);
        assert_eq!(new_scene[0].text, "【前情提要】\n壓縮後的摘要");

        // en 語系用英文前綴
        let next_en = begin_next_scene(root.path(), &world_id, "recap text", "en", None).unwrap();
        assert_eq!(next_en, 2);
        let scene_two = read_transcript(root.path(), &world_id, 2).unwrap();
        assert_eq!(scene_two[0].text, "Previously:\nrecap text");
    }

    #[test]
    fn begin_next_scene_stores_title_on_old_scene_when_given() {
        let root = TestRoot::new("begin-next-scene-title");
        let world_id = create_world(root.path(), "取名桌").unwrap();
        let event = TranscriptEvent {
            ts: "2026-07-24T00:00:00Z".to_owned(),
            speaker_id: String::new(),
            speaker_name: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "第一幕的對話".to_owned(),
        };
        append_transcript(root.path(), &world_id, 0, &event).unwrap();

        begin_next_scene(root.path(), &world_id, "摘要", "zh-TW", Some("酒館夜話")).unwrap();
        let state = read_state(root.path(), &world_id).unwrap();
        assert_eq!(state.current_scene, 1);
        assert_eq!(
            state.scene_titles.get("0").map(String::as_str),
            Some("酒館夜話")
        );
        assert!(!state.scene_titles.contains_key("1"));

        // 空字串／None 都不進表
        begin_next_scene(root.path(), &world_id, "摘要二", "zh-TW", Some("   ")).unwrap();
        begin_next_scene(root.path(), &world_id, "摘要三", "zh-TW", None).unwrap();
        let state = read_state(root.path(), &world_id).unwrap();
        assert!(!state.scene_titles.contains_key("1"));
        assert!(!state.scene_titles.contains_key("2"));
    }

    #[test]
    fn state_round_trips_and_errors_when_file_is_missing() {
        let root = TestRoot::new("state");
        fs::create_dir_all(root.path().join("worlds").join(new_id())).unwrap();
        let missing_id = new_id();
        fs::create_dir_all(root.path().join("worlds").join(&missing_id)).unwrap();
        // 沒有 state.json（不是新流程會出現的情況，但要確定不會誤當空狀態處理）
        assert!(read_state(root.path(), &missing_id).is_err());

        let world_id = create_world(root.path(), "無狀態").unwrap();
        let mut state = read_state(root.path(), &world_id).unwrap();
        state.current_scene = 12;
        state
            .model_bindings
            .insert("船長代碼".to_owned(), "balanced".to_owned());
        state
            .catchup_summaries
            .insert("水手代碼".to_owned(), "錯過了序幕".to_owned());
        write_state(root.path(), &world_id, &state).unwrap();
        assert_eq!(read_state(root.path(), &world_id).unwrap(), state);
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
