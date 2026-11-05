//! VN 背景庫（plans/vn-mode-pkg3.md §2）：`[bg: 地點 | 時段]` → 鍵（地點雜湊 slug＋時段鍵）→
//! `worlds/<id>/vn/backgrounds/<slug>-<時段>.png`。路徑只由雜湊與驗過的鍵組成，模型文字不進路徑；
//! `backgrounds.json` 記 slug → 地點顯示名與基底時段。生圖走立繪同一個一次一張的佇列（`vn_sprites::queue`）。
//! VN 卡宣告的地點（plans/vn-mode-pkg9b.md §2.5）先比對：slug 取 `card:<鍵>` 的雜湊、時段用該地點自己的清單。
//! 每地點自動只生一張基底（時段清單第一項的光線），換時段由舞台疊色調遮罩；專屬時段圖由玩家手動生成
//! （plans/vn-setup-wizard.md §3）。
use crate::data::vn::CardOrigin;
use crate::data::vn_art::{valid_art_key, CardLocation};
use crate::transport::vn_background_prompts::{self as prompts, BackgroundReference};
use crate::vn_sprites::{
    self,
    queue::{BgMode, BgSlot},
};
use crate::{data, data_root};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub(crate) mod gallery;

/// 固定四時段（自由地點與沒宣告時段的卡地點）：第一項是預設，也是找光線參考圖的優先序
const PERIODS: [&str; 4] = ["day", "morning", "dusk", "night"];
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

/// 這桌 VN 卡宣告的地點（卡外桌＝空）
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Places(Vec<CardLocation>);

/// 一個地點對到的背景庫位置：slug、顯示名與有效時段清單
pub(crate) struct Located {
    pub slug: String,
    pub name: String,
    periods: Vec<String>,
}

impl Places {
    pub(crate) fn of(card: Option<&CardOrigin>) -> Self {
        Places(card.map(|card| card.locations.clone()).unwrap_or_default())
    }

    pub(crate) fn load(root: &std::path::Path, world: &str) -> Self {
        Self::of(data::vn::read_card(root, world).as_ref())
    }

    /// 地點文字對卡內地點：鍵或名稱（同 `fold` 正規化）；對不到＝自由地點
    pub(crate) fn locate(&self, location: &str) -> Located {
        let folded = fold(location);
        match self
            .0
            .iter()
            .find(|place| fold(&place.key) == folded || fold(&place.name) == folded)
        {
            Some(place) => Located {
                slug: card_slug(&place.key),
                name: place.name.clone(),
                periods: periods_of(Some(place)),
            },
            None => Located {
                slug: slug_of(location),
                name: location.trim().to_owned(),
                periods: periods_of(None),
            },
        }
    }

    pub(crate) fn by_slug(&self, slug: &str) -> Option<&CardLocation> {
        self.0.iter().find(|place| card_slug(&place.key) == slug)
    }

    /// 卡宣告地點的 slug，照卡上順序
    pub(crate) fn slugs(&self) -> Vec<String> {
        self.0.iter().map(|place| card_slug(&place.key)).collect()
    }

    /// 這個背景 slug 的有效時段清單
    pub(crate) fn periods(&self, slug: &str) -> Vec<String> {
        periods_of(self.by_slug(slug))
    }
}

/// 卡地點宣告了時段就是那幾個鍵，否則固定四時段
fn periods_of(place: Option<&CardLocation>) -> Vec<String> {
    match place.filter(|place| !place.times.is_empty()) {
        Some(place) => place.times.clone(),
        None => PERIODS.map(str::to_owned).to_vec(),
    }
}

/// 卡地點的 slug：`card:<鍵>` 的雜湊（與自由地名分開，路徑仍只由雜湊組成）
fn card_slug(key: &str) -> String {
    slug_of(&format!("card:{key}"))
}

impl Located {
    /// 寫明的時段：先比鍵本身，再走同義詞；結果要在這個地點的清單內，否則 None
    pub(crate) fn period_for(&self, time: &str) -> Option<String> {
        let folded = fold(time);
        self.periods
            .iter()
            .find(|period| **period == folded)
            .or_else(|| {
                let synonym = period_of(time)?;
                self.periods.iter().find(|period| *period == synonym)
            })
            .cloned()
    }

    /// 寫明的認得就用；否則沿用（也在清單內才算）；再不行取清單第一項
    fn period(&self, time: Option<&str>, inherited: Option<&str>) -> String {
        time.and_then(|time| self.period_for(time))
            .or_else(|| {
                inherited
                    .filter(|inherited| self.periods.iter().any(|period| period == inherited))
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| self.periods[0].clone())
    }
}

/// 一個背景鍵（已沿用好的時段）
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) struct Backdrop {
    /// 顯示名：卡地點＝卡上的名稱，自由地點＝原文
    pub location: String,
    pub slug: String,
    pub period: String,
}

impl Backdrop {
    pub(crate) fn new(
        location: &str,
        time: Option<&str>,
        inherited: Option<&str>,
        places: &Places,
    ) -> Self {
        let located = places.locate(location);
        Backdrop {
            period: located.period(time, inherited),
            location: located.name,
            slug: located.slug,
        }
    }

    /// 同地點的基底鍵：時段清單第一項
    pub(crate) fn base(&self, places: &Places) -> Self {
        Backdrop {
            period: places.periods(&self.slug).swap_remove(0),
            ..self.clone()
        }
    }
}

/// `backgrounds.json` 一筆：舊桌只有地名，新桌另記基底時段
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
enum Entry {
    Name(String),
    Place { name: String, base: String },
}

/// 舞台讀到的一張背景：`period`＝這張圖實際的時段（不等於要的時段＝基底，前端疊遮罩）
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) struct Shown {
    pub period: String,
    pub png: Vec<u8>,
}

/// 一桌的背景庫
pub(crate) struct Library {
    dir: PathBuf,
    /// 短提交鎖的鍵（`backgrounds.json` 讀改寫用）
    root: PathBuf,
    world: String,
}

impl Library {
    pub(crate) fn of(root: &std::path::Path, world: &str) -> Result<Self, String> {
        Ok(Library {
            dir: data::vn_background_dir(root, world).map_err(|error| error.to_string())?,
            root: root.to_owned(),
            world: world.to_owned(),
        })
    }

    /// slug 與時段都得先驗過（`valid_slug`、該地點的時段清單）；這裡再擋一次格式，不合格的換成不可能是真鍵的名字
    fn path(&self, slug: &str, period: &str) -> PathBuf {
        let (slug, period) = match valid_slug(slug) && valid_art_key(period) {
            true => (slug, period),
            false => ("!", "!"),
        };
        self.dir.join(format!("{slug}-{period}.png"))
    }

    pub(crate) fn has(&self, slug: &str, period: &str) -> bool {
        self.path(slug, period).is_file()
    }

    pub(crate) fn read(&self, slug: &str, period: &str) -> Option<Vec<u8>> {
        std::fs::read(self.path(slug, period)).ok()
    }

    /// 這個地點的基底時段：有記錄就只認記下的那張（還沒生好＝None，等自動補）；
    /// 沒記錄（舊桌與作品包匯入的桌）＝清單第一項 → 清單裡第一張有圖的
    pub(crate) fn base_period(&self, slug: &str, periods: &[String]) -> Option<String> {
        match self.entries().remove(slug) {
            Some(Entry::Place { base, .. }) => Some(base).filter(|base| self.has(slug, base)),
            _ => periods
                .iter()
                .find(|period| self.has(slug, period))
                .cloned(),
        }
    }

    /// 舞台要的時段：有專屬圖用專屬圖，否則基底（前端依 `period` 不同疊遮罩）
    pub(crate) fn shown(&self, slug: &str, period: &str, periods: &[String]) -> Option<Shown> {
        let period = match self.has(slug, period) {
            true => period.to_owned(),
            false => self.base_period(slug, periods)?,
        };
        let png = self.read(slug, &period)?;
        Some(Shown { period, png })
    }

    /// 這張該不該生／寫：基底＝地點還沒有任何圖；重生成與專屬圖一律寫
    fn wants(&self, slot: &BgSlot, periods: &[String]) -> bool {
        match slot.mode {
            BgMode::Base => self.base_period(&slot.backdrop.slug, periods).is_none(),
            BgMode::Regenerate | BgMode::Dedicated => true,
        }
    }

    /// 同地點有效時段清單內別的時段（專屬圖改光線的參考圖），照清單順序（基底優先）
    fn reference(&self, slug: &str, period: &str, periods: &[String]) -> Option<Vec<u8>> {
        periods
            .iter()
            .filter(|other| *other != period)
            .find_map(|other| self.read(slug, other))
    }

    fn names_path(&self) -> PathBuf {
        self.dir.join("backgrounds.json")
    }

    fn entries(&self) -> BTreeMap<String, Entry> {
        std::fs::read(self.names_path())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// 地點還沒有基底時，寫入要記下的基底時段：時段清單第一項（基底一律中性白天光，別的時段的圖只當專屬圖）
    fn intended_base(&self, slug: &str, periods: &[String]) -> Option<String> {
        match self.base_period(slug, periods) {
            Some(_) => None,
            None => periods.first().cloned(),
        }
    }

    /// 寫圖並記下 slug → 原地點文字（已記過的不改）；`base`＝要記的基底時段（舊格式只有地名的升級成帶基底）。
    /// 呼叫端持桌寫入許可，但那是共用的讀鎖（多個寫入者可同時持有）：索引的「讀→合併→寫回」另包在
    /// 這桌的短提交鎖裡，背景工作與上傳同時寫才不會讀到同一份舊索引、後寫的蓋掉先寫的那筆
    fn write(&self, backdrop: &Backdrop, base: Option<&str>, png: &[u8]) -> Result<(), String> {
        data::commit_world_write_atomic(&self.path(&backdrop.slug, &backdrop.period), png)
            .map_err(|error| error.to_string())?;
        data::state_commit::with_commit(&self.root, &self.world, |_| self.record(backdrop, base))
    }

    /// 索引合併一筆（只在短提交鎖內呼叫）
    fn record(&self, backdrop: &Backdrop, base: Option<&str>) -> Result<(), String> {
        let mut entries = self.entries();
        let name = match entries.get(&backdrop.slug) {
            Some(Entry::Place { .. }) => return Ok(()),
            Some(Entry::Name(_)) if base.is_none() => return Ok(()),
            Some(Entry::Name(name)) => name.clone(),
            None => backdrop.location.clone(),
        };
        let entry = match base {
            Some(base) => Entry::Place {
                name,
                base: base.to_owned(),
            },
            None => Entry::Name(name),
        };
        entries.insert(backdrop.slug.clone(), entry);
        let json = serde_json::to_vec_pretty(&entries).map_err(|error| error.to_string())?;
        data::commit_world_write_atomic(&self.names_path(), &json)
            .map_err(|error| error.to_string())
    }
}

/// 一幕的格補上背景：每個 Bg 格的時段照「沒寫或認不得＝沿用目前（要在目的地清單內），再沒有＝清單第一項」填好；
/// 幕首第一格不是 Bg 而 `start`（幕首事件快照的 place／time）有地點，就在最前面補一格。
/// 回這幕用到的背景（依出現順序、去重）
pub(crate) fn resolve_scene<'a>(
    events: impl IntoIterator<Item = &'a mut Vec<crate::chat_turn::VnCell>>,
    start: Option<(&str, Option<&str>)>,
    places: &Places,
) -> Vec<Backdrop> {
    use crate::chat_turn::VnCell;
    let mut events: Vec<_> = events.into_iter().collect();
    let starts_with_bg = events
        .iter()
        .flat_map(|cells| cells.iter())
        .next()
        .is_some_and(|cell| matches!(cell, VnCell::Bg { .. }));
    let mut current: Option<String> =
        start.and_then(|(place, time)| places.locate(place).period_for(time?));
    if let (Some((place, time)), Some(first), false) = (start, events.first_mut(), starts_with_bg) {
        if !place.trim().is_empty() {
            first.insert(0, Backdrop::new(place, time, None, places).cell(time));
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
            let backdrop = Backdrop::new(location, time.as_deref(), current.as_deref(), places);
            *location = backdrop.location.clone();
            *slug = backdrop.slug.clone();
            *period = Some(backdrop.period.clone());
            current = Some(backdrop.period.clone());
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
            period: Some(self.period.clone()),
        }
    }

    fn key(&self) -> (&str, &str) {
        (&self.slug, &self.period)
    }
}

/// 排入這些背景的地點裡還沒有任何圖的基底（冪等：佇列去重；換時段不排，由舞台疊遮罩）
pub(crate) fn fill(app: &tauri::AppHandle, world: &str, used: &[Backdrop]) -> Result<(), String> {
    let root = data_root(app)?;
    // 重播桌進桌也會打：靜默不排（plans/vn-mode-pkg8.md §2.6.1）
    if data::vn::is_replay(&root, world) {
        return Ok(());
    }
    let library = Library::of(&root, world)?;
    let places = Places::load(&root, world);
    let mut missing: Vec<BgSlot> = Vec::new();
    for backdrop in used {
        let slot = BgSlot::new(world, backdrop.base(&places), BgMode::Base);
        let periods = places.periods(&backdrop.slug);
        if library.wants(&slot, &periods) && !missing.contains(&slot) {
            missing.push(slot);
        }
    }
    vn_sprites::queue::enqueue_backgrounds(app, missing);
    Ok(())
}

/// 串流中已寫完的 `[bg]` 行（plans/vn-setup-wizard.md §3 改動 a）：接在正式格後面一起補時段，回串流段用到的背景。
/// 只看換行結尾的行，最後一行還在長就不算；沒寫時段的沿用正式格與本段前文
pub(crate) fn streamed(
    formal: &mut [crate::chat_turn::VnEventCells],
    text: &str,
    source: &crate::chat_turn::VnSource,
    roster: &crate::chat_turn::VnRoster,
) -> Vec<Backdrop> {
    let complete = &text[..text.rfind('\n').unwrap_or(0)];
    let mut stream = crate::chat_turn::parse_text(complete, source, roster).cells;
    if !stream
        .iter()
        .any(|cell| matches!(cell, crate::chat_turn::VnCell::Bg { .. }))
    {
        return Vec::new();
    }
    // 正式格已補好的時段（含幕首快照沿用的）原樣保留：重解析時當成明寫
    for cell in formal.iter_mut().flat_map(|event| event.cells.iter_mut()) {
        if let crate::chat_turn::VnCell::Bg {
            time,
            period: Some(period),
            ..
        } = cell
        {
            *time = Some(period.clone());
        }
    }
    resolve_scene(
        formal
            .iter_mut()
            .map(|event| &mut event.cells)
            .chain(std::iter::once(&mut stream)),
        None,
        &roster.places,
    );
    stream
        .into_iter()
        .filter_map(|cell| match cell {
            crate::chat_turn::VnCell::Bg {
                location,
                slug,
                period,
                ..
            } => Some(Backdrop {
                location,
                slug,
                period: period?,
            }),
            _ => None,
        })
        .fold(Vec::new(), |mut used, backdrop| {
            if !used.contains(&backdrop) {
                used.push(backdrop);
            }
            used
        })
}

/// 一張背景。基底與重生成從零生（卡開了封面參考就以封面取畫風；API 來源不帶參考圖）；
/// 專屬時段圖以同地點既有的圖當參考改光線。卡地點用卡上的描述與尺度，卡畫風與背景規則接在提示詞後。失敗自動重試一次。
/// 寫前重驗桌仍在、基底仍缺（重生成與專屬圖除外）。Ok(false)＝桌已不在或別人先寫好，結果丟掉
pub(crate) async fn job(app: &tauri::AppHandle, slot: &BgSlot) -> Result<bool, String> {
    let root = data_root(app)?;
    let library = Library::of(&root, &slot.world)?;
    let backdrop = &slot.backdrop;
    let card = data::vn::read_card(&root, &slot.world);
    let places = Places::of(card.as_ref());
    let periods = places.periods(&backdrop.slug);
    if !library.wants(slot, &periods) {
        return Ok(true);
    }
    let config = vn_sprites::read_config(app)?;
    let source = vn_sprites::sprite_source(&config);
    let same_place = match (source, slot.mode) {
        ("api", _) | (_, BgMode::Base | BgMode::Regenerate) => None,
        (_, BgMode::Dedicated) => library.reference(&backdrop.slug, &backdrop.period, &periods),
    };
    let (kind, reference) = match same_place {
        Some(png) => (BackgroundReference::SameLocation, Some(png)),
        None => match vn_sprites::cover_reference(&root, &slot.world, card.as_ref(), source) {
            Some(png) => (BackgroundReference::CoverStyle, Some(png)),
            None => (BackgroundReference::None, None),
        },
    };
    let setting: String = data::read_world_md(&root, &slot.world)
        .unwrap_or_default()
        .chars()
        .take(SETTING_CHARS)
        .collect();
    let prompt = prompts::background_prompt(
        &backdrop.location,
        places.by_slug(&backdrop.slug),
        &backdrop.period,
        &setting,
        kind,
        card.as_ref().and_then(|card| card.art.as_ref()),
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
    if gone || !library.wants(slot, &periods) {
        return Ok(false);
    }
    let base = match slot.mode {
        BgMode::Base => Some(backdrop.period.clone()),
        _ => library.intended_base(&backdrop.slug, &periods),
    };
    library.write(backdrop, base.as_deref(), &png)?;
    Ok(true)
}

#[cfg(test)]
mod tests;
