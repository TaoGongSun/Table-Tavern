//! VN 背景庫（plans/vn-mode-pkg3.md §2）：`[bg: 地點 | 時段]` → 鍵（地點雜湊 slug＋固定四時段）→
//! `worlds/<id>/vn/backgrounds/<slug>-<時段>.png`。路徑只由雜湊與列舉組成，模型文字不進路徑；
//! `backgrounds.json` 記 slug → 原地點文字。生圖走立繪同一個一次一張的佇列（`vn_sprites::queue`）。
use crate::transport::vn_background_prompts as prompts;
use crate::vn_sprites::{self, queue::BgSlot};
use crate::{data, data_root};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub(crate) const PERIODS: [&str; 4] = ["morning", "day", "dusk", "night"];
const DEFAULT_PERIOD: &str = "day";
const SLUG_LEN: usize = 16;
/// 世界設定截多少字當場景風格依據
const SETTING_CHARS: usize = 600;

/// 時段同義詞，依序比對（「午夜」要先於「午」、「傍晚」先於「晚」）
const PERIOD_WORDS: [(&str, &[&str]); 4] = [
    (
        "dusk",
        &[
            "黃昏", "黄昏", "傍晚", "夕陽", "夕阳", "日落", "薄暮", "dusk", "sunset", "evening",
            "twilight",
        ],
    ),
    ("night", &["夜", "晚上", "凌晨", "night", "midnight"]),
    (
        "morning",
        &[
            "清晨", "早上", "早晨", "黎明", "拂曉", "拂晓", "上午", "morning", "dawn", "sunrise",
        ],
    ),
    (
        "day",
        &[
            "白天",
            "中午",
            "正午",
            "午後",
            "午后",
            "下午",
            "白晝",
            "白昼",
            "日間",
            "day",
            "noon",
            "afternoon",
        ],
    ),
];

/// 時間文字 → 固定時段；認不得回 None（由呼叫端沿用）。先比同義詞，再看 `HH:MM` 的鐘點
pub(crate) fn period_of(text: &str) -> Option<&'static str> {
    let lower = fold(text);
    PERIOD_WORDS
        .iter()
        .find(|(_, words)| words.iter().any(|word| lower.contains(word)))
        .map(|(period, _)| *period)
        .or_else(|| hour_of(&lower).map(period_at))
}

fn hour_of(text: &str) -> Option<u32> {
    let colon = text.find(':')?;
    let digits: String = text[..colon]
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .take(2)
        .collect();
    digits
        .chars()
        .rev()
        .collect::<String>()
        .parse()
        .ok()
        .filter(|hour| *hour < 24)
}

fn period_at(hour: u32) -> &'static str {
    match hour {
        5..=10 => "morning",
        11..=16 => "day",
        17..=18 => "dusk",
        _ => "night",
    }
}

pub(crate) fn known_period(value: &str) -> Option<&'static str> {
    PERIODS.into_iter().find(|period| *period == value)
}

/// 全形轉半形（含全形空白）、去頭尾空白、連續空白收一、轉小寫
fn fold(text: &str) -> String {
    let halfwidth: String = text
        .chars()
        .map(|c| match c {
            '\u{3000}' => ' ',
            '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            c => c,
        })
        .collect();
    halfwidth
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// 地點 → slug：正規化後 SHA-256 取前 16 個 hex
pub(crate) fn slug_of(location: &str) -> String {
    let digest = Sha256::digest(fold(location).as_bytes());
    digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()[..SLUG_LEN]
        .to_owned()
}

pub(crate) fn valid_slug(slug: &str) -> bool {
    slug.len() == SLUG_LEN
        && slug
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// 一個背景鍵（已沿用好的時段）
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) struct Backdrop {
    pub location: String,
    pub slug: String,
    pub period: &'static str,
}

impl Backdrop {
    pub(crate) fn new(location: &str, time: Option<&str>, inherited: Option<&'static str>) -> Self {
        Backdrop {
            location: location.trim().to_owned(),
            slug: slug_of(location),
            period: time
                .and_then(period_of)
                .or(inherited)
                .unwrap_or(DEFAULT_PERIOD),
        }
    }
}

/// 一桌的背景庫
pub(crate) struct Library {
    dir: PathBuf,
}

impl Library {
    pub(crate) fn of(root: &std::path::Path, world: &str) -> Result<Self, String> {
        Ok(Library {
            dir: data::vn_background_dir(root, world).map_err(|error| error.to_string())?,
        })
    }

    /// slug 與時段都得先驗過（呼叫端從 `valid_slug`／`known_period` 來）
    fn path(&self, slug: &str, period: &str) -> PathBuf {
        self.dir.join(format!("{slug}-{period}.png"))
    }

    pub(crate) fn has(&self, slug: &str, period: &str) -> bool {
        self.path(slug, period).is_file()
    }

    /// 這張該不該生／寫：庫裡沒有，或是重生成
    fn wants(&self, backdrop: &Backdrop, force: bool) -> bool {
        force || !self.has(&backdrop.slug, backdrop.period)
    }

    pub(crate) fn read(&self, slug: &str, period: &str) -> Option<Vec<u8>> {
        std::fs::read(self.path(slug, period)).ok()
    }

    /// 同地點別的時段（改光線的參考圖），白天優先
    fn reference(&self, slug: &str, period: &str) -> Option<Vec<u8>> {
        ["day", "morning", "dusk", "night"]
            .into_iter()
            .filter(|other| *other != period)
            .find_map(|other| self.read(slug, other))
    }

    fn names_path(&self) -> PathBuf {
        self.dir.join("backgrounds.json")
    }

    /// 寫圖並記下 slug → 原地點文字（已記過的不改）。呼叫端持桌寫入許可
    fn write(&self, backdrop: &Backdrop, png: &[u8]) -> Result<(), String> {
        data::commit_world_write_atomic(&self.path(&backdrop.slug, backdrop.period), png)
            .map_err(|error| error.to_string())?;
        let mut names: BTreeMap<String, String> = std::fs::read(self.names_path())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        if names.contains_key(&backdrop.slug) {
            return Ok(());
        }
        names.insert(backdrop.slug.clone(), backdrop.location.clone());
        let json = serde_json::to_vec_pretty(&names).map_err(|error| error.to_string())?;
        data::commit_world_write_atomic(&self.names_path(), &json)
            .map_err(|error| error.to_string())
    }
}

/// 一幕的格補上背景：每個 Bg 格的時段照「沒寫或認不得＝沿用目前，再沒有＝day」填好；
/// 幕首第一格不是 Bg 而 `start`（幕首事件快照的 place／time）有地點，就在最前面補一格。
/// 回這幕用到的背景（依出現順序、去重）
pub(crate) fn resolve_scene<'a>(
    events: impl IntoIterator<Item = &'a mut Vec<crate::chat_turn::VnCell>>,
    start: Option<(&str, Option<&str>)>,
) -> Vec<Backdrop> {
    use crate::chat_turn::VnCell;
    let mut events: Vec<_> = events.into_iter().collect();
    let starts_with_bg = events
        .iter()
        .flat_map(|cells| cells.iter())
        .next()
        .is_some_and(|cell| matches!(cell, VnCell::Bg { .. }));
    let mut current = start.and_then(|(_, time)| time.and_then(period_of));
    if let (Some((place, time)), Some(first), false) = (start, events.first_mut(), starts_with_bg) {
        if !place.trim().is_empty() {
            first.insert(0, Backdrop::new(place, time, None).cell(time));
        }
    }
    let mut used: Vec<Backdrop> = Vec::new();
    for cell in events.iter_mut().flat_map(|cells| cells.iter_mut()) {
        if let VnCell::Bg {
            location,
            time,
            slug,
            period,
        } = cell
        {
            let backdrop = Backdrop::new(location, time.as_deref(), current);
            *slug = backdrop.slug.clone();
            *period = Some(backdrop.period.to_owned());
            current = Some(backdrop.period);
            if !used.iter().any(|seen| seen.key() == backdrop.key()) {
                used.push(backdrop);
            }
        }
    }
    used
}

impl Backdrop {
    fn cell(&self, time: Option<&str>) -> crate::chat_turn::VnCell {
        crate::chat_turn::VnCell::Bg {
            location: self.location.clone(),
            time: time.map(str::to_owned),
            slug: self.slug.clone(),
            period: Some(self.period.to_owned()),
        }
    }

    fn key(&self) -> (&str, &str) {
        (&self.slug, self.period)
    }
}

/// 排入庫裡還沒有的背景（冪等：佇列去重）
pub(crate) fn fill(app: &tauri::AppHandle, world: &str, used: &[Backdrop]) -> Result<(), String> {
    let library = Library::of(&data_root(app)?, world)?;
    let missing: Vec<BgSlot> = used
        .iter()
        .filter(|backdrop| !library.has(&backdrop.slug, backdrop.period))
        .map(|backdrop| BgSlot::new(world, backdrop.clone(), false))
        .collect();
    vn_sprites::queue::enqueue_backgrounds(app, missing);
    Ok(())
}

/// 一張背景：庫裡同地點有別的時段就當參考圖改光線（API 來源參考圖未驗，從零生）；失敗自動重試一次。
/// 寫前重驗桌仍在、鍵仍空（重生成除外）。Ok(false)＝桌已不在或別人先寫好，結果丟掉
pub(crate) async fn job(app: &tauri::AppHandle, slot: &BgSlot) -> Result<bool, String> {
    let root = data_root(app)?;
    let library = Library::of(&root, &slot.world)?;
    let backdrop = &slot.backdrop;
    if !library.wants(backdrop, slot.force) {
        return Ok(true);
    }
    let config = vn_sprites::read_config(app)?;
    let reference = match vn_sprites::sprite_source(&config) {
        "api" => None,
        _ => library.reference(&backdrop.slug, backdrop.period),
    };
    let setting: String = data::read_world_md(&root, &slot.world)
        .unwrap_or_default()
        .chars()
        .take(SETTING_CHARS)
        .collect();
    let prompt = prompts::background_prompt(
        &backdrop.location,
        backdrop.period,
        &setting,
        reference.is_some(),
    );
    let generate = || {
        vn_sprites::generate_png(
            app,
            &config,
            &slot.world,
            &prompt,
            reference.as_deref(),
            "16:9",
        )
    };
    let png = match generate().await {
        Ok(png) => png,
        Err(error) => {
            log::warn!("背景生成失敗，重試一次：{error}");
            generate().await?
        }
    };
    let _permit = data::world_write_permit_async(&slot.world).await?;
    let gone = data::read_state(&root, &slot.world).is_err();
    if gone || !library.wants(backdrop, slot.force) {
        return Ok(false);
    }
    library.write(backdrop, &png)?;
    Ok(true)
}

#[cfg(test)]
mod tests;
