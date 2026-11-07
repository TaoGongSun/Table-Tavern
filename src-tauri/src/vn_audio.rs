//! VN 音訊（plans/vn-audio.md）：Incompetech 免金鑰 BGM、Freesound 選填音效。
//! 索引與曲子都落本機；抓不到索引就用舊檔。網路等待不持世界鎖。
use crate::{chat_turn, data, data_root};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const MOODS: [&str; 19] = [
    "Dark",
    "Grooving",
    "Relaxed",
    "Bright",
    "Calming",
    "Bouncy",
    "Driving",
    "Mysterious",
    "Intense",
    "Eerie",
    "Unnerving",
    "Somber",
    "Mystical",
    "Uplifting",
    "Humorous",
    "Epic",
    "Action",
    "Suspenseful",
    "Aggressive",
];

const PIECES_URL: &str = "https://incompetech.com/music/royalty-free/pieces.json";
const PIECE_URL: &str = "https://incompetech.com/music/royalty-free/mp3-royaltyfree/";
/// 官方現行搜尋是 `GET /apiv2/search/`（`/apiv1/search/text` 已標棄用）。filter 用 Solr：
/// 數字區間 `[start TO end]`、多詞授權值加引號、OR 包在括號裡。授權名對 Sound Instance 表。
const SEARCH_URL: &str = "https://freesound.org/apiv2/search/";
const SEARCH_FILTER: &str = r#"duration:[0 TO 15] license:("Creative Commons 0" OR "Attribution")"#;
const CC_BY: &str = "https://creativecommons.org/licenses/by/4.0/";
const AUTHOR: &str = "Kevin MacLeod";

const INDEX_MAX: usize = 4 * 1024 * 1024;
const SEARCH_MAX: usize = 256 * 1024;
const BGM_MAX: usize = 32 * 1024 * 1024;
const SFX_MAX: usize = 4 * 1024 * 1024;
const INDEX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);
const STATUS_EVENT: &str = "vn-audio-status";

/// `None`＝整格丟掉。`Some([])`＝`[bgm: off]`。詞不在表上、不是一或兩個，都丟掉。
pub(crate) fn parse_bgm(raw: &str) -> Option<Vec<String>> {
    let text = raw.trim();
    if text.eq_ignore_ascii_case("off") {
        return Some(Vec::new());
    }
    let parts: Vec<&str> = text
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.is_empty() || parts.len() > 2 {
        return None;
    }
    let mut moods = Vec::with_capacity(parts.len());
    for part in parts {
        let Some(mood) = MOODS
            .iter()
            .copied()
            .find(|mood| mood.eq_ignore_ascii_case(part))
        else {
            return None;
        };
        moods.push(mood.to_owned());
    }
    Some(moods)
}

/// 小寫、連續空白收成一格。空的、有控制字元、超過 200 字＝丟掉。
pub(crate) fn parse_sfx(raw: &str) -> Option<(String, String)> {
    let mut query = String::new();
    for ch in raw.chars() {
        if ch.is_control() {
            return None;
        }
        if ch.is_whitespace() {
            if !query.is_empty() && !query.ends_with(' ') {
                query.push(' ');
            }
        } else {
            query.push(ch);
        }
    }
    let query = query.trim().to_lowercase();
    if query.is_empty() || query.chars().count() > 200 {
        return None;
    }
    Some((query.clone(), hex16(&Sha256::digest(query.as_bytes()))))
}

pub(crate) fn bgm_key(moods: &[String]) -> String {
    moods
        .iter()
        .map(|mood| mood.to_lowercase())
        .collect::<Vec<_>>()
        .join("+")
}

pub(crate) fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && key
            .bytes()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'+'))
}

/// 音效檔名鍵：`parse_sfx` 的 SHA-256 前 16 個小寫 hex。
pub(crate) fn valid_sfx_key(key: &str) -> bool {
    key.len() == 16
        && key
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// ≥4 bytes；`ID3` 且版本位元組 ≤4，或 MPEG frame sync（前 11 位為 1、layer ≠ 00、bitrate ≠ 0xF）。
pub(crate) fn mp3_ok(bytes: &[u8]) -> bool {
    if bytes.len() < 4 {
        return false;
    }
    if bytes.starts_with(b"ID3") {
        return bytes[3] <= 4;
    }
    let (b0, b1, b2) = (bytes[0], bytes[1], bytes[2]);
    if b0 != 0xFF || b1 & 0xE0 != 0xE0 {
        return false;
    }
    let layer = (b1 >> 1) & 0b11;
    let bitrate = (b2 >> 4) & 0xF;
    layer != 0 && bitrate != 0xF
}

fn valid_filename(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && name.ends_with(".mp3")
        && !name.contains(['/', '\\'])
        && !name.contains("..")
        && !name.chars().any(char::is_control)
}

fn hex16(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Piece {
    title: String,
    filename: String,
    feel: Vec<String>,
}

/// 整份必須是陣列才算驗過；單筆不合格跳過。
pub(crate) fn parse_pieces(bytes: &[u8]) -> Result<Vec<Piece>, String> {
    if bytes.len() > INDEX_MAX {
        return Err("AUDIO_INDEX_TOO_LARGE".to_owned());
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    let Some(items) = value.as_array() else {
        return Err("AUDIO_INDEX_NOT_ARRAY".to_owned());
    };
    let mut pieces = Vec::new();
    for item in items {
        let Some(title) = item.get("title").and_then(Value::as_str) else {
            continue;
        };
        let Some(filename) = item.get("filename").and_then(Value::as_str) else {
            continue;
        };
        let Some(feel) = item.get("feel").and_then(Value::as_str) else {
            continue;
        };
        if !valid_filename(filename) {
            continue;
        }
        pieces.push(Piece {
            title: title.to_owned(),
            filename: filename.to_owned(),
            feel: feel
                .split(',')
                .map(|tag| tag.trim().to_lowercase())
                .filter(|tag| !tag.is_empty())
                .collect(),
        });
    }
    Ok(pieces)
}

/// 候選＝feel 含全部情緒；沒有交集就退回第一個情緒。同桌同情緒用雜湊固定一首。
pub(crate) fn pick_piece<'a>(
    world: &str,
    moods: &[String],
    pieces: &'a [Piece],
) -> Option<&'a Piece> {
    let want: Vec<String> = moods.iter().map(|mood| mood.to_lowercase()).collect();
    if want.is_empty() {
        return None;
    }
    let mut pool: Vec<&Piece> = pieces
        .iter()
        .filter(|piece| {
            want.iter()
                .all(|mood| piece.feel.iter().any(|tag| tag == mood))
        })
        .collect();
    if pool.is_empty() && want.len() > 1 {
        pool = pieces
            .iter()
            .filter(|piece| piece.feel.iter().any(|tag| tag == &want[0]))
            .collect();
    }
    if pool.is_empty() {
        return None;
    }
    let digest = Sha256::digest(format!("{world}\u{0}{}", want.join("+")).as_bytes());
    let index = u64::from_be_bytes(digest[..8].try_into().unwrap()) as usize % pool.len();
    Some(pool[index])
}

fn theme_moods(mood: Option<data::vn::Mood>) -> Vec<String> {
    match mood {
        Some(data::vn::Mood::Light) => vec!["Bright".to_owned()],
        Some(data::vn::Mood::Dark) => vec!["Mysterious".to_owned()],
        Some(data::vn::Mood::Warm) | None => vec!["Calming".to_owned()],
    }
}

#[derive(Clone, Debug)]
struct Job {
    world: String,
    kind: String,
    key: String,
    /// bgm：要挑的情緒（主題曲空著，下載當下再對 preferences）。sfx：搜尋句。
    detail: String,
    theme: bool,
}

impl Job {
    fn id(&self) -> String {
        format!("{}\u{0}{}", self.kind, self.key)
    }
}

fn jobs_from(world: &str, cells: &[chat_turn::VnCell], theme: bool, sfx: bool) -> Vec<Job> {
    let mut jobs = Vec::new();
    if theme {
        jobs.push(Job {
            world: world.to_owned(),
            kind: "bgm".to_owned(),
            key: "theme".to_owned(),
            detail: String::new(),
            theme: true,
        });
    }
    for cell in cells {
        match cell {
            chat_turn::VnCell::Bgm { moods } if !moods.is_empty() => {
                let key = bgm_key(moods);
                if valid_key(&key) && !jobs.iter().any(|job| job.kind == "bgm" && job.key == key) {
                    jobs.push(Job {
                        world: world.to_owned(),
                        kind: "bgm".to_owned(),
                        key,
                        detail: moods.join(","),
                        theme: false,
                    });
                }
            }
            // 每則事件最多兩個音效已在 parse_event 擋住；這裡一幕有很多事件，不能再設整幕上限。
            chat_turn::VnCell::Sfx { query, key } if sfx && valid_key(key) => {
                if !jobs.iter().any(|job| job.kind == "sfx" && job.key == *key) {
                    jobs.push(Job {
                        world: world.to_owned(),
                        kind: "sfx".to_owned(),
                        key: key.clone(),
                        detail: query.clone(),
                        theme: false,
                    });
                }
            }
            _ => {}
        }
    }
    jobs
}

fn token_of(app: &AppHandle) -> Option<String> {
    let config = crate::vn_sprites::read_config(app).ok()?;
    let token = config.api_keys.get("freesound")?.trim().to_owned();
    (!token.is_empty()).then_some(token)
}

struct Queue {
    /// 每桌一個待辦集合。忙碌時照樣加入，迴圈做到空。
    pending: HashMap<String, BTreeMap<String, Job>>,
    busy: BTreeSet<String>,
}

fn queue() -> std::sync::MutexGuard<'static, Queue> {
    static QUEUE: OnceLock<Mutex<Queue>> = OnceLock::new();
    QUEUE
        .get_or_init(|| {
            Mutex::new(Queue {
                pending: HashMap::new(),
                busy: BTreeSet::new(),
            })
        })
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

fn enqueue(app: &AppHandle, world: &str, jobs: Vec<Job>) {
    if jobs.is_empty() {
        return;
    }
    let spawn = {
        let mut queue = queue();
        let pending = queue.pending.entry(world.to_owned()).or_default();
        for job in jobs {
            pending.entry(job.id()).or_insert(job);
        }
        queue.busy.insert(world.to_owned())
    };
    if spawn {
        let app = app.clone();
        let world = world.to_owned();
        tauri::async_runtime::spawn(async move { worker(app, world).await });
    }
}

async fn worker(app: AppHandle, world: String) {
    loop {
        let job = {
            let mut queue = queue();
            let next = queue
                .pending
                .get_mut(&world)
                .and_then(|pending| pending.pop_first())
                .map(|(_, job)| job);
            if next.is_none() {
                queue.busy.remove(&world);
                if queue
                    .pending
                    .get(&world)
                    .map(|pending| pending.is_empty())
                    .unwrap_or(true)
                {
                    queue.pending.remove(&world);
                }
            }
            next
        };
        let Some(job) = job else { break };
        if let Err(error) = run_job(&app, &job).await {
            log::warn!("VN 音訊沒抓到 {} {}: {error}", job.kind, job.key);
        }
    }
}

async fn run_job(app: &AppHandle, job: &Job) -> Result<(), String> {
    let root = data_root(app)?;
    if data::vn::is_replay(&root, &job.world) || data::read_state(&root, &job.world).is_err() {
        return Ok(());
    }
    let path = audio_path(&root, &job.world, &job.kind, &job.key)?;
    if path.is_file() {
        return Ok(());
    }
    let fetched = match job.kind.as_str() {
        "bgm" => fetch_bgm(app, job).await?,
        "sfx" => fetch_sfx(app, job).await?,
        _ => return Ok(()),
    };
    let Some((bytes, entry)) = fetched else {
        return Ok(());
    };
    if !mp3_ok(&bytes) {
        return Err("AUDIO_NOT_MP3".to_owned());
    }
    let _permit = data::world_write_permit_async(&job.world).await?;
    if data::vn::is_replay(&root, &job.world) || data::read_state(&root, &job.world).is_err() {
        return Ok(());
    }
    if path.is_file() {
        return Ok(());
    }
    write_part(&path, &bytes)?;
    if upsert_entry(&root, &job.world, &entry).is_err() {
        let _ = std::fs::remove_file(&path);
        return Err("AUDIO_INDEX_WRITE".to_owned());
    }
    let _ = app.emit(
        STATUS_EVENT,
        AudioStatus {
            world_id: job.world.clone(),
            kind: job.kind.clone(),
            key: job.key.clone(),
            state: "done".to_owned(),
        },
    );
    Ok(())
}

async fn fetch_bgm(app: &AppHandle, job: &Job) -> Result<Option<(Vec<u8>, Entry)>, String> {
    let root = data_root(app)?;
    let moods = if job.theme {
        let mood = data::vn::read_vn(&root, &job.world)
            .ok()
            .and_then(|file| file.preferences)
            .and_then(|preferences| preferences.mood);
        theme_moods(mood)
    } else {
        job.detail
            .split(',')
            .filter(|mood| !mood.is_empty())
            .map(str::to_owned)
            .collect()
    };
    let config = crate::config_root(app)?;
    let pieces = load_pieces(&config).await;
    let Some(piece) = pick_piece(&job.world, &moods, &pieces) else {
        return Ok(None);
    };
    let url = format!("{PIECE_URL}{}", encode(&piece.filename));
    let bytes = download(&url, &[], BGM_MAX).await?;
    Ok(Some((
        bytes,
        Entry {
            kind: "bgm".to_owned(),
            key: job.key.clone(),
            title: piece.title.clone(),
            author: AUTHOR.to_owned(),
            source_url: url,
            license: CC_BY.to_owned(),
        },
    )))
}

async fn fetch_sfx(app: &AppHandle, job: &Job) -> Result<Option<(Vec<u8>, Entry)>, String> {
    let Some(token) = token_of(app) else {
        return Ok(None);
    };
    let hit = freesound_first(&token, &job.detail, |url, token| async move {
        download(
            &url,
            &[("Authorization", &format!("Token {token}"))],
            SEARCH_MAX,
        )
        .await
    })
    .await;
    let Some(hit) = hit else {
        return Ok(None);
    };
    let bytes = download(&hit.preview, &[], SFX_MAX).await?;
    Ok(Some((
        bytes,
        Entry {
            kind: "sfx".to_owned(),
            key: job.key.clone(),
            title: hit.title,
            author: hit.author,
            source_url: hit.page,
            license: hit.license,
        },
    )))
}

/// 先寫 `.part`，檔頭不對就刪掉，對了才改名。
fn write_part(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let partial = PathBuf::from(format!("{}.part", path.display()));
    if !mp3_ok(bytes) {
        let _ = std::fs::remove_file(&partial);
        return Err("AUDIO_NOT_MP3".to_owned());
    }
    if let Err(error) = std::fs::write(&partial, bytes) {
        let _ = std::fs::remove_file(&partial);
        return Err(error.to_string());
    }
    if let Err(error) = std::fs::rename(&partial, path) {
        let _ = std::fs::remove_file(&partial);
        return Err(error.to_string());
    }
    Ok(())
}

fn audio_path(root: &Path, world: &str, kind: &str, key: &str) -> Result<PathBuf, String> {
    if !valid_key(key) || (kind != "bgm" && kind != "sfx") {
        return Err("AUDIO_KEY".to_owned());
    }
    Ok(data::vn_audio_dir(root, world)
        .map_err(|error| error.to_string())?
        .join(format!("{kind}-{key}.mp3")))
}

#[derive(Clone, Debug, Serialize)]
struct AudioStatus {
    #[serde(rename = "worldId")]
    world_id: String,
    kind: String,
    key: String,
    state: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Entry {
    pub kind: String,
    pub key: String,
    pub title: String,
    pub author: String,
    #[serde(default)]
    pub source_url: String,
    #[serde(default)]
    pub license: String,
}

fn upsert_entry(root: &Path, world: &str, entry: &Entry) -> Result<(), String> {
    let path = data::vn_audio_dir(root, world)
        .map_err(|error| error.to_string())?
        .join("audio.json");
    let mut entries = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Vec<Entry>>(&bytes).ok())
        .unwrap_or_default();
    if let Some(found) = entries
        .iter_mut()
        .find(|item| item.kind == entry.kind && item.key == entry.key)
    {
        *found = entry.clone();
    } else {
        entries.push(entry.clone());
    }
    let bytes = serde_json::to_vec(&entries).map_err(|error| error.to_string())?;
    data::commit_world_write_atomic(&path, &bytes).map_err(|error| error.to_string())
}

/// 匯出：只留這次放進包的 bgm／sfx 條目。`keys` 是 (kind, key)。
pub(crate) fn export_entries(dir: &Path, keys: &BTreeSet<(String, String)>) -> Vec<Entry> {
    let Ok(bytes) = std::fs::read(dir.join("audio.json")) else {
        return Vec::new();
    };
    serde_json::from_slice::<Vec<Entry>>(&bytes)
        .unwrap_or_default()
        .into_iter()
        .filter(|entry| index_keeps(entry, keys))
        .map(clip_entry)
        .collect()
}

/// 匯入：超過 `max` 筆拒收；其餘不合的筆丟掉，留下的必須對得到已落地的音訊檔。
pub(crate) fn import_entries(
    bytes: &[u8],
    keys: &BTreeSet<(String, String)>,
    max: usize,
) -> Result<Vec<Entry>, String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    let Some(items) = value.as_array() else {
        return Err("AUDIO_INDEX_NOT_ARRAY".to_owned());
    };
    if items.len() > max {
        return Err("AUDIO_INDEX_TOO_MANY".to_owned());
    }
    let mut entries = Vec::new();
    for item in items {
        let Some(entry) = item_entry(item) else {
            continue;
        };
        if index_keeps(&entry, keys) {
            entries.push(clip_entry(entry));
        }
    }
    Ok(entries)
}

fn index_keeps(entry: &Entry, keys: &BTreeSet<(String, String)>) -> bool {
    let key_ok = match entry.kind.as_str() {
        "bgm" => valid_key(&entry.key),
        "sfx" => valid_sfx_key(&entry.key),
        _ => false,
    };
    key_ok && keys.contains(&(entry.kind.clone(), entry.key.clone()))
}

fn item_entry(item: &Value) -> Option<Entry> {
    Some(Entry {
        kind: item.get("kind")?.as_str()?.to_owned(),
        key: item.get("key")?.as_str()?.to_owned(),
        title: item
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        author: item
            .get("author")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        source_url: item
            .get("source_url")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        license: item
            .get("license")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
    })
}

fn clip_entry(mut entry: Entry) -> Entry {
    entry.title = clip_chars(&entry.title, 200);
    entry.author = clip_chars(&entry.author, 200);
    entry.source_url = clip_chars(&entry.source_url, 500);
    entry.license = clip_chars(&entry.license, 500);
    entry
}

fn clip_chars(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

fn pieces_path(config: &Path) -> PathBuf {
    config.join("cache").join("incompetech-pieces.json")
}

fn index_stale(path: &Path) -> bool {
    let Ok(modified) = std::fs::metadata(path).and_then(|meta| meta.modified()) else {
        return true;
    };
    // 超過 7 天才重抓；讀不到時間就當過期
    modified
        .elapsed()
        .map(|age| age > INDEX_AGE)
        .unwrap_or(true)
}

async fn load_pieces(config: &Path) -> Vec<Piece> {
    let path = pieces_path(config);
    if !index_stale(&path) {
        if let Some(pieces) = std::fs::read(&path)
            .ok()
            .and_then(|bytes| parse_pieces(&bytes).ok())
        {
            return pieces;
        }
    }
    if let Ok(bytes) = download(PIECES_URL, &[], INDEX_MAX).await {
        if let Ok(pieces) = parse_pieces(&bytes) {
            let _ = store_index(&path, &bytes);
            return pieces;
        }
    }
    std::fs::read(&path)
        .ok()
        .and_then(|bytes| parse_pieces(&bytes).ok())
        .unwrap_or_default()
}

fn store_index(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let partial = PathBuf::from(format!("{}.{suffix}.part", path.display()));
    std::fs::write(&partial, bytes).map_err(|error| error.to_string())?;
    std::fs::rename(&partial, path).map_err(|error| {
        let _ = std::fs::remove_file(&partial);
        error.to_string()
    })
}

fn http() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        // 跨主機重新導向不跟（Freesound preview 必須停在 freesound 的主機）
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                let Some(prev) = attempt.previous().last() else {
                    return attempt.follow();
                };
                let next = attempt.url();
                if prev.host() == next.host() && prev.scheme() == next.scheme() {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

async fn download(url: &str, headers: &[(&str, &str)], max: usize) -> Result<Vec<u8>, String> {
    let mut request = http().get(url);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let pending = request.send();
    match tokio::time::timeout(DOWNLOAD_TIMEOUT, read_body(pending, max)).await {
        Ok(result) => result,
        Err(_) => Err("AUDIO_DOWNLOAD: timed out".to_owned()),
    }
}

async fn read_body(
    pending: impl std::future::Future<Output = Result<reqwest::Response, reqwest::Error>>,
    max: usize,
) -> Result<Vec<u8>, String> {
    let mut response = pending.await.map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("AUDIO_DOWNLOAD: status={}", response.status()));
    }
    if response
        .content_length()
        .is_some_and(|length| length > max as u64)
    {
        return Err("AUDIO_DOWNLOAD: too large".to_owned());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
        if bytes.len() + chunk.len() > max {
            return Err("AUDIO_DOWNLOAD: too large".to_owned());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SfxHit {
    title: String,
    author: String,
    page: String,
    preview: String,
    license: String,
}

/// 無 token 不打網路。沒有合格結果回 None，不當錯誤。
async fn freesound_first<F, Fut>(token: &str, query: &str, mut fetch: F) -> Option<SfxHit>
where
    F: FnMut(String, String) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<u8>, String>>,
{
    if token.trim().is_empty() {
        return None;
    }
    let url = format!(
        "{SEARCH_URL}?query={}&fields=id,name,previews,license,username,url,duration&filter={}&page_size=5&sort=rating_desc",
        encode(query),
        encode(SEARCH_FILTER),
    );
    let bytes = fetch(url, token.to_owned()).await.ok()?;
    first_sfx(&bytes)
}

pub(crate) fn first_sfx(bytes: &[u8]) -> Option<SfxHit> {
    if bytes.len() > SEARCH_MAX {
        return None;
    }
    let value: Value = serde_json::from_slice(bytes).ok()?;
    let results = value.get("results")?.as_array()?;
    for item in results {
        let license = item.get("license").and_then(Value::as_str).unwrap_or("");
        if !license_ok(license) {
            continue;
        }
        let preview = item
            .pointer("/previews/preview-hq-mp3")
            .and_then(Value::as_str)
            .unwrap_or("");
        if !preview_ok(preview) {
            continue;
        }
        return Some(SfxHit {
            title: item
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            author: item
                .get("username")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            page: item
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            preview: preview.to_owned(),
            license: license.to_owned(),
        });
    }
    None
}

/// 文件確認的名稱只有這兩個。不收 URL：路徑裡夾關鍵字的偽造網址會被 contains 放行。
fn license_ok(license: &str) -> bool {
    matches!(license.trim(), "Creative Commons 0" | "Attribution")
}

fn preview_ok(url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(url) else {
        return false;
    };
    url.scheme() == "https" && matches!(url.host_str(), Some("freesound.org" | "cdn.freesound.org"))
}

fn scene_cells(root: &Path, world: &str, scene: u64) -> Vec<chat_turn::VnCell> {
    chat_turn::read_scene(root, world, scene)
        .map(|(events, _)| events.into_iter().flat_map(|event| event.cells).collect())
        .unwrap_or_default()
}

pub(crate) fn fill(app: &AppHandle, world: &str, scene: Option<u64>) -> Result<(), String> {
    let root = data_root(app)?;
    if data::vn::is_replay(&root, world) {
        return Ok(());
    }
    // 建桌當下狀態可能還沒讀得到：格子當空，主題曲仍要排，不能等玩家開桌。
    let cells = match scene {
        Some(scene) => scene_cells(&root, world, scene),
        None => match data::read_state(&root, world) {
            Ok(state) => scene_cells(&root, world, state.current_scene),
            Err(_) => Vec::new(),
        },
    };
    enqueue(
        app,
        world,
        jobs_from(world, &cells, true, token_of(app).is_some()),
    );
    Ok(())
}

pub(crate) fn prefetch(
    app: &AppHandle,
    world: &str,
    source: &chat_turn::VnSource,
    text: &str,
) -> Result<(), String> {
    let root = data_root(app)?;
    if data::vn::is_replay(&root, world) {
        return Ok(());
    }
    let roster =
        chat_turn::VnRoster::load(&root, world, data::vn::read_card(&root, world).as_ref())?;
    let complete = &text[..text.rfind('\n').unwrap_or(0)];
    let parsed = chat_turn::parse_text(complete, source, &roster);
    enqueue(
        app,
        world,
        jobs_from(world, &parsed.cells, false, token_of(app).is_some()),
    );
    Ok(())
}

#[derive(Serialize)]
pub(crate) struct AudioFile {
    kind: String,
    key: String,
    path: String,
}

#[derive(Serialize)]
pub(crate) struct AudioView {
    files: Vec<AudioFile>,
    entries: Vec<Entry>,
}

pub(crate) fn read(app: &AppHandle, world: &str) -> Result<AudioView, String> {
    let root = data_root(app)?;
    let dir = data::vn_audio_dir(&root, world).map_err(|error| error.to_string())?;
    let mut files = Vec::new();
    let mut entries = Vec::new();
    if dir.is_dir() {
        app.asset_protocol_scope()
            .allow_directory(&dir, false)
            .map_err(|error| error.to_string())?;
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let Some((kind, key)) = file_key(&name) else {
                    continue;
                };
                files.push(AudioFile {
                    kind,
                    key,
                    path: entry.path().display().to_string(),
                });
            }
        }
        entries = std::fs::read(dir.join("audio.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
    }
    files.sort_by(|left, right| (&left.kind, &left.key).cmp(&(&right.kind, &right.key)));
    Ok(AudioView { files, entries })
}

pub(crate) fn file_key(name: &str) -> Option<(String, String)> {
    let (kind, rest) = if let Some(rest) = name.strip_prefix("bgm-") {
        ("bgm", rest)
    } else if let Some(rest) = name.strip_prefix("sfx-") {
        ("sfx", rest)
    } else {
        return None;
    };
    let key = rest.strip_suffix(".mp3")?;
    valid_key(key).then(|| (kind.to_owned(), key.to_owned()))
}

/// 給測試看搜尋網址與 filter 是否對得上文件。
#[cfg(test)]
pub(crate) fn search_filter() -> &'static str {
    SEARCH_FILTER
}

#[cfg(test)]
#[path = "vn_audio_tests.rs"]
mod tests;
