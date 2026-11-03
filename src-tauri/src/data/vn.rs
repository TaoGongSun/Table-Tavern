//! VN 桌的劇本聖經檔 `worlds/<id>/vn/vn.json`（plans/vn-mode-pkg4.md §2.1）：結構、驗證正規化、讀寫、
//! 補種與狀態樹規則。讀改寫一律在短提交鎖內（`edit_vn` 收 `CommitTx`），只改這次負責的欄位。
use super::paths::world_dir;
use super::state_commit::CommitTx;
use super::vn_art::{CardArt, CardLocation, CastArt};
use super::{Condition, DataResult, FieldKind, FieldRule, InjectLevel, StateNode, UpdateMode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// 章數上限 M 的保底值與夾值範圍（總綱 §8）。
pub const CHAPTER_FALLBACK: u32 = 8;
const CHAPTER_RANGE: (u32, u32) = (3, 30);
/// 字數上限（總綱 §4）：常駐段、單章當章包、單角色私設。
const RESIDENT_CAP: usize = 12_000;
const CHAPTER_CAP: usize = 4_000;
pub const PRIVATE_CAP: usize = 2_000;
/// 聖經在狀態樹佔的三個頂層鍵：只收表內欄位逐葉更新（`Mechanism::locked_roots`）。
pub const LOCKED_ROOTS: [&str; 3] = ["flags", "revealed", "chapter"];
pub const DEFAULT_ENDING: &str = "default";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VnFile {
    #[serde(default = "version")]
    pub version: u32,
    /// 劇透鎖，預設鎖〔作者裁決 2026-11-01〕；讀不到檔的舊桌也當鎖住。
    #[serde(default = "locked")]
    pub spoiler_lock: bool,
    #[serde(default)]
    pub bible: Option<Bible>,
    /// 注入用的凍結副本：幕中改聖經不動 system，換幕才重抄（§2.4）。
    #[serde(default)]
    pub frozen: Option<Frozen>,
    /// 從 VN 卡建的桌才有：卡原件（plans/vn-mode-pkg9.md §2.4）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<CardOrigin>,
}

fn version() -> u32 {
    1
}

fn locked() -> bool {
    true
}

impl Default for VnFile {
    fn default() -> Self {
        Self {
            version: version(),
            spoiler_lock: true,
            bible: None,
            frozen: None,
            card: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frozen {
    pub scene: u64,
    pub chapter: u32,
    pub bible: Bible,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Branching {
    None,
    Light,
    #[default]
    Full,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Bible {
    pub logline: String,
    pub style: String,
    pub branching: Branching,
    pub chapter_max: u32,
    pub chapter_reason: String,
    /// VN 卡作者鎖住章數：劇本分頁與精靈的章數欄唯讀，存檔拒收章數變更〔作者裁決 2026-11-01 拍板 17〕
    pub chapter_lock: bool,
    pub chapters: Vec<Chapter>,
    pub secrets: Vec<Secret>,
    pub flags: Vec<Flag>,
    pub endings: Vec<Ending>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Chapter {
    pub title: String,
    pub goal: String,
    pub beats: String,
    pub end_condition: String,
    pub plant: Vec<String>,
    pub payoff: Vec<String>,
    pub anchors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Secret {
    pub id: String,
    /// 角色 id；對不到角色＝null（世界層的秘密）
    #[serde(default)]
    pub holder: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub text: String,
    #[serde(default, deserialize_with = "loose_reveal")]
    pub reveal: Reveal,
    #[serde(default)]
    pub hint: String,
}

/// 揭露時機：旗標成立時、到某章、或 `"never"`（只暗示永不明說）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Reveal {
    Flag { flag: String },
    Chapter { chapter: u32 },
    Never(String),
}

impl Default for Reveal {
    fn default() -> Self {
        Self::Never("never".to_owned())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FlagKind {
    Bool,
    Counter,
    Route,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Flag {
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(rename = "type")]
    pub kind: FlagKind,
    #[serde(default, deserialize_with = "loose_text")]
    pub initial: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "loose_texts"
    )]
    pub values: Vec<String>,
    #[serde(default)]
    pub trigger: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ending {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub when: Vec<Condition>,
}

/// VN 卡原件：卡頂層的分級、旁白人稱、主角模式、匯入時的聖經與美術欄位（匯出帶回）。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CardOrigin {
    pub lang: String,
    pub rating: Rating,
    pub pov: Option<Pov>,
    pub protagonist_fixed: bool,
    /// 匯入時正規化後的聖經（「恢復卡內原版」用）
    pub original_bible: Option<Bible>,
    /// 桌內角色 id → 逐角色欄位；角色刪掉後匯出略過
    pub cast: BTreeMap<String, CastOrigin>,
    /// 卡層級畫風；卡上沒寫＝None
    pub art: Option<CardArt>,
    pub locations: Vec<CardLocation>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rating {
    pub level: RatingLevel,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RatingLevel {
    #[default]
    General,
    Teen,
    Adult,
}

/// 旁白人稱
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pov {
    First,
    Second,
    Third,
}

/// 出場性質；沒填＝一般登場
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CastRole {
    Main,
    Support,
    Memory,
    Deity,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CastOrigin {
    pub role: Option<CastRole>,
    /// 出場限制白話（如「只在夢境出現」）
    pub appears: String,
    pub art: CastArt,
}

/// 字串、數字、布林都收成文字：模型寫 `"initial": false` 或 `0` 都對得上。
fn loose_text<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(text_of(&serde_json::Value::deserialize(deserializer)?).unwrap_or_default())
}

/// 揭露時機寫壞了不連累整條秘密：退回 `never`。
fn loose_reveal<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Reveal, D::Error> {
    Ok(serde_json::from_value(serde_json::Value::deserialize(deserializer)?).unwrap_or_default())
}

fn loose_texts<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(value
        .as_array()
        .map(|items| items.iter().filter_map(text_of).collect())
        .unwrap_or_default())
}

fn text_of(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.trim().to_owned()),
        serde_json::Value::Bool(flag) => Some(flag.to_string()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

/// 模型產出的 JSON 寬鬆讀進來：清單逐項讀，壞的那項丟掉、不連累整份；章數寫成字串也認。
/// 不是物件回 None（算解析失敗）。讀完還要 `normalize`。
pub fn parse_bible(value: &serde_json::Value) -> Option<Bible> {
    let object = value.as_object()?;
    let text = |key: &str| object.get(key).and_then(text_of).unwrap_or_default();
    Some(Bible {
        logline: text("logline"),
        style: text("style"),
        branching: object
            .get("branching")
            .and_then(|value| serde_json::from_value(value.clone()).ok())
            .unwrap_or_default(),
        chapter_max: text("chapter_max")
            .parse::<f64>()
            .map_or(0, |max| max as u32),
        chapter_reason: text("chapter_reason"),
        chapter_lock: false,
        chapters: items(value, "chapters"),
        secrets: items(value, "secrets"),
        flags: items(value, "flags"),
        endings: items(value, "endings"),
    })
}

pub fn items<T: DeserializeOwned>(value: &serde_json::Value, key: &str) -> Vec<T> {
    value
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|item| serde_json::from_value(item.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// id 一律 `[a-z0-9_]{1,32}`。
pub fn valid_id(id: &str) -> bool {
    (1..=32).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// 驗證與正規化（§2.1，一處；包 9 VN 卡匯入重用）。`cast`＝(角色 id, 名字)：秘密的 holder 寫 id 或名字都對得上。
pub fn normalize(mut bible: Bible, cast: &[(String, String)]) -> Bible {
    bible.chapter_max = match bible.chapter_max {
        0 => CHAPTER_FALLBACK,
        max => max.clamp(CHAPTER_RANGE.0, CHAPTER_RANGE.1),
    };
    let mut seen = HashSet::new();
    bible
        .flags
        .retain_mut(|flag| valid_id(&flag.id) && seen.insert(flag.id.clone()) && fix_flag(flag));
    let flag_ids: HashSet<String> = bible.flags.iter().map(|flag| flag.id.clone()).collect();
    let max = bible.chapter_max;
    let mut seen = HashSet::new();
    bible.secrets.retain_mut(|secret| {
        if !valid_id(&secret.id) || !seen.insert(secret.id.clone()) {
            return false;
        }
        secret.holder = secret.holder.as_deref().and_then(|holder| {
            cast.iter()
                .find(|(id, name)| id == holder || name.trim() == holder.trim())
                .map(|(id, _)| id.clone())
        });
        secret.reveal = match &secret.reveal {
            Reveal::Flag { flag } if flag_ids.contains(flag) => secret.reveal.clone(),
            Reveal::Chapter { chapter } => Reveal::Chapter {
                chapter: (*chapter).clamp(1, max),
            },
            _ => Reveal::default(),
        };
        true
    });
    let mut seen = HashSet::new();
    bible.endings.retain(|ending| {
        valid_id(&ending.id)
            && seen.insert(ending.id.clone())
            && ending.when.iter().all(|condition| {
                let path = condition_path(condition);
                path == "chapter"
                    || path
                        .strip_prefix("flags.")
                        .is_some_and(|id| flag_ids.contains(id))
            })
    });
    match bible
        .endings
        .iter_mut()
        .find(|ending| ending.id == DEFAULT_ENDING)
    {
        Some(default) => default.when.clear(),
        None => bible.endings.push(Ending {
            id: DEFAULT_ENDING.to_owned(),
            title: String::new(),
            when: Vec::new(),
        }),
    }
    if bible.branching == Branching::None {
        for ending in bible
            .endings
            .iter_mut()
            .filter(|ending| ending.id != DEFAULT_ENDING)
        {
            if !ending
                .when
                .iter()
                .any(|condition| condition_path(condition) == "chapter")
            {
                ending.when.push(Condition::Range {
                    path: "chapter".to_owned(),
                    min: Some(max as f64),
                    max: None,
                    min_exclusive: false,
                    max_exclusive: false,
                    default: None,
                });
            }
        }
    }
    cap_text(&mut bible);
    bible
}

fn condition_path(condition: &Condition) -> &str {
    match condition {
        Condition::Range { path, .. }
        | Condition::Contains { path, .. }
        | Condition::Flag { path, .. } => path,
    }
}

/// 旗標型別各自的合法形狀；不合格回 false（整筆丟掉）。
fn fix_flag(flag: &mut Flag) -> bool {
    match flag.kind {
        FlagKind::Bool => {
            flag.initial = (flag.initial == "true").to_string();
            flag.values.clear();
            (flag.min, flag.max) = (None, None);
        }
        FlagKind::Counter => {
            if let (Some(min), Some(max)) = (flag.min, flag.max) {
                if min > max {
                    (flag.min, flag.max) = (None, None);
                }
            }
            let mut initial = flag.initial.parse::<f64>().unwrap_or(0.0);
            if let Some(min) = flag.min {
                initial = initial.max(min);
            }
            if let Some(max) = flag.max {
                initial = initial.min(max);
            }
            flag.initial = number_text(initial);
            flag.values.clear();
        }
        FlagKind::Route => {
            let mut seen = HashSet::new();
            flag.values
                .retain(|value| !value.is_empty() && seen.insert(value.clone()));
            if !(2..=8).contains(&flag.values.len()) {
                return false;
            }
            if !flag.values.contains(&flag.initial) {
                flag.initial = flag.values[0].clone();
            }
            (flag.min, flag.max) = (None, None);
        }
    }
    true
}

fn number_text(value: f64) -> String {
    if value == value.trunc() && value.abs() < 1e15 {
        (value as i64).to_string()
    } else {
        value.to_string()
    }
}

/// 剩餘字數預算：超出就從這一欄截短，之後的欄位清空。
struct Budget(usize);

impl Budget {
    fn take(&mut self, text: &mut String) {
        let count = text.chars().count();
        if count > self.0 {
            *text = text.chars().take(self.0).collect();
            self.0 = 0;
        } else {
            self.0 -= count;
        }
    }

    fn take_all(&mut self, list: &mut Vec<String>) {
        list.iter_mut().for_each(|item| self.take(item));
        list.retain(|item| !item.is_empty());
    }
}

/// 常駐段 12,000 字、單章 4,000 字，照注入順序截短。
fn cap_text(bible: &mut Bible) {
    let mut resident = Budget(RESIDENT_CAP);
    resident.take(&mut bible.logline);
    resident.take(&mut bible.style);
    for chapter in &mut bible.chapters {
        resident.take(&mut chapter.title);
        resident.take(&mut chapter.goal);
    }
    for secret in &mut bible.secrets {
        resident.take(&mut secret.title);
        resident.take(&mut secret.text);
        resident.take(&mut secret.hint);
    }
    for flag in &mut bible.flags {
        resident.take(&mut flag.label);
        resident.take(&mut flag.trigger);
    }
    for ending in &mut bible.endings {
        resident.take(&mut ending.title);
    }
    for chapter in &mut bible.chapters {
        let mut pack = Budget(CHAPTER_CAP);
        pack.take(&mut chapter.beats);
        pack.take(&mut chapter.end_condition);
        pack.take_all(&mut chapter.plant);
        pack.take_all(&mut chapter.payoff);
        pack.take_all(&mut chapter.anchors);
    }
}

/// 線性作品的章門檻跟著改過的章數上限走：門檻等於舊上限的 `chapter` 區間改成新上限。
pub fn move_chapter_gate(bible: &mut Bible, old_max: u32) {
    let new_max = bible.chapter_max as f64;
    for ending in bible.endings.iter_mut() {
        for condition in &mut ending.when {
            if let Condition::Range { path, min, .. } = condition {
                if path == "chapter" && *min == Some(old_max as f64) {
                    *min = Some(new_max);
                }
            }
        }
    }
}

/// 截到 `cap` 字（私設 2,000 字用）。
pub fn cap_chars(text: &str, cap: usize) -> String {
    text.chars().take(cap).collect()
}

impl Bible {
    /// 狀態樹初值：`flags.<id>`、`revealed.<秘密id>`＝false、`chapter`＝1。
    pub fn seeds(&self) -> Vec<(Vec<String>, String)> {
        let path = |root: &str, id: &str| vec![root.to_owned(), id.to_owned()];
        self.flags
            .iter()
            .map(|flag| (path("flags", &flag.id), flag.initial.clone()))
            .chain(
                self.secrets
                    .iter()
                    .map(|secret| (path("revealed", &secret.id), "false".to_owned())),
            )
            .chain(std::iter::once((
                vec!["chapter".to_owned()],
                "1".to_owned(),
            )))
            .collect()
    }

    /// 機制規則（§2.3）：每條都 `inject: turn`（不靠 `for_kind` 的預設，唯讀與文字欄的預設不進回合尾）。
    pub fn rules(&self) -> BTreeMap<String, FieldRule> {
        let choice = |allowed: Vec<String>| FieldRule {
            allowed,
            inject: InjectLevel::Turn,
            ..FieldRule::for_kind(FieldKind::Text)
        };
        let yes_no = || vec!["true".to_owned(), "false".to_owned()];
        let mut rules: BTreeMap<String, FieldRule> = self
            .flags
            .iter()
            .map(|flag| {
                let rule = match flag.kind {
                    FlagKind::Bool => choice(yes_no()),
                    FlagKind::Route => choice(flag.values.clone()),
                    FlagKind::Counter => FieldRule {
                        min: flag.min,
                        max: flag.max,
                        ..FieldRule::for_kind(FieldKind::Counter)
                    },
                };
                (format!("flags.{}", flag.id), rule)
            })
            .collect();
        for secret in &self.secrets {
            rules.insert(format!("revealed.{}", secret.id), choice(yes_no()));
        }
        rules.insert(
            "chapter".to_owned(),
            FieldRule {
                update: UpdateMode::Reject,
                inject: InjectLevel::Turn,
                ..FieldRule::for_kind(FieldKind::ReadOnly)
            },
        );
        rules
    }
}

/// 補種：表內但樹上缺的路徑補成初值，只補不覆蓋。回傳有沒有補。
pub fn fill_missing(
    tree: &mut BTreeMap<String, StateNode>,
    seeds: &[(Vec<String>, String)],
) -> bool {
    let mut filled = false;
    for (path, initial) in seeds {
        if super::node_at(tree, path).is_none() {
            filled |= super::set_tree_value(tree, path, initial);
        }
    }
    filled
}

fn vn_path(root: &Path, world_id: &str) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?.join("vn").join("vn.json"))
}

/// VN 卡匯入時存的封面（`worlds/<id>/vn/cover.png`）。
pub fn cover_path(root: &Path, world_id: &str) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?.join("vn").join("cover.png"))
}

/// 讀 vn.json；沒有檔＝預設（鎖住、沒有聖經）。檔壞了回錯，不拿預設蓋掉。
pub fn read_vn(root: &Path, world_id: &str) -> DataResult<VnFile> {
    match std::fs::read(vn_path(root, world_id)?) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(VnFile::default()),
        Err(error) => Err(error.into()),
    }
}

/// 卡原件（生圖管線讀美術欄位用）；不是卡桌或讀不到＝None
pub fn read_card(root: &Path, world_id: &str) -> Option<CardOrigin> {
    read_vn(root, world_id).ok()?.card
}

/// 精靈 ② 一進來就建檔（帶 ① 選的劇透鎖；沒給＝預設鎖）；已有檔不動。
pub fn create_if_missing(tx: &CommitTx<'_>, spoiler_lock: Option<bool>) -> DataResult<()> {
    if vn_path(tx.root, tx.world_id)?.exists() {
        return Ok(());
    }
    edit_vn(tx, |file| file.spoiler_lock = spoiler_lock.unwrap_or(true))
}

/// 鎖內讀改寫 vn.json（原子寫入）：先重讀最新檔，`edit` 只改這次負責的欄位。
pub fn edit_vn<T>(tx: &CommitTx<'_>, edit: impl FnOnce(&mut VnFile) -> T) -> DataResult<T> {
    let mut file = read_vn(tx.root, tx.world_id)?;
    let result = edit(&mut file);
    // 父目錄由共用寫入函式建
    let path = vn_path(tx.root, tx.world_id)?;
    super::commit_world_write_atomic(&path, &serde_json::to_vec_pretty(&file)?)?;
    Ok(result)
}

#[cfg(test)]
#[path = "vn_tests.rs"]
mod tests;
