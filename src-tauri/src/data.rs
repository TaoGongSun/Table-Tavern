use crate::mechanism::{self, Outcome, Record, RecordKind};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
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

/// 本機時間的 (年, 月, 日, 時, 分, 秒)。
fn local_time_parts() -> DataResult<(i32, i32, i32, i32, i32, i32)> {
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
        return Ok((
            local.tm_year + 1900,
            local.tm_mon + 1,
            local.tm_mday,
            local.tm_hour,
            local.tm_min,
            local.tm_sec,
        ));
    }

    #[cfg(not(unix))]
    {
        // Tauri's supported Unix targets use localtime_r above. Keep a dependency-free fallback
        // for other targets; its value is UTC when no platform local-time API is available.
        let minutes = seconds / 60;
        Ok((
            1970,
            1,
            1,
            ((minutes / 60) % 24) as i32,
            (minutes % 60) as i32,
            (seconds % 60) as i32,
        ))
    }
}

pub fn local_timestamp() -> DataResult<String> {
    let (year, month, day, hour, minute, _) = local_time_parts()?;
    Ok(format!(
        "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}"
    ))
}

/// 秒級時間戳。給需要判斷短間隔的紀錄用——快取命中率 log 要看得出兩次呼叫相隔幾秒，
/// 分鐘精度分不出是否踩到 Anthropic 的 5 分鐘過期線。
pub fn local_timestamp_seconds() -> DataResult<String> {
    let (year, month, day, hour, minute, second) = local_time_parts()?;
    Ok(format!(
        "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}"
    ))
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
    /// 自動隱藏（AI 卡重構包 4b）：換幕結算時系統判斷「這幕沒出現」才打開，劇情拉回來就
    /// 自動關掉；跟 `archived`（玩家手動封存，系統永不自動改動）是獨立的兩軸，見
    /// `settle_card_visibility` 與 `set_character_auto_hidden`。
    #[serde(default)]
    pub auto_hidden: bool,
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
    /// 剝殼前的模型原文：狀態區塊與點名行都還在，供卡片自帶的面板重畫歷史訊息用。
    /// 與 text 相同（沒剝到東西）時不存，舊檔沒有這欄也照樣讀得起來。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<TableState>,
    /// 這則系統事件的全文只給 GM 看；chars 續聊線遇到只留第一行（AI 卡重構包 4b，
    /// 補 4a 遺留的 visibility 洩漏——非 Public 世界書人物的登場全文不該流進扮演引擎）。
    #[serde(default)]
    pub gm_only: bool,
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
    /// AI 卡重構切出來、玩家選擇「不升格為角色卡」的人物條目標記；一般條目一律 false。
    #[serde(default)]
    pub is_person: bool,
    /// 被 app 接管的機制條目唯讀標記；資料層只負責原樣保存。
    #[serde(default)]
    pub locked: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableState {
    #[serde(default)]
    pub table: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tree: BTreeMap<String, StateNode>,
    /// 拒收回饋句：上一輪被系統擋下的更新，跟著逐則快照走，供下一輪模型自癒。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// 上一輪本地套用的變動：路徑（點分）→ 顯示標記（"+5"／"-80"／"更新"）。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub changes: BTreeMap<String, String>,
    /// 上一輪觸發表命中的文本：trigger id → 已代換好的文本，跟著逐則快照走。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub triggers: BTreeMap<String, String>,
    /// 全量桌的跳動警示：路徑（點分）→ 顯示標記（"+40"／"-80"）。只給玩家看，不進提示詞；
    /// 每回合重算，跟著逐則快照走，不需另外處理回滾。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub jumps: BTreeMap<String, String>,
}

/// 狀態樹節點保留自然 JSON 形狀，讓匯出的初始值仍可由人閱讀與手改。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StateNode {
    Leaf(String),
    Branch(BTreeMap<String, StateNode>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    #[default]
    Number,
    Pair,
    Roll,
    Text,
    List,
    Counter,
    ReadOnly,
    Derived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateMode {
    Delta,
    Replace,
    Local,
    Reject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InjectLevel {
    Snapshot,
    Turn,
    Rare,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldRule {
    pub kind: FieldKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    pub update: UpdateMode,
    pub inject: InjectLevel,
    /// 角色卡只帶走自己的分支，避免把整桌的規則偷偷塞進單張卡。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// `derived` 專用：算式（見 `evaluator.rs`），以狀態樹為取值來源算出這一欄的值。
    /// 規範書 v1 對 `derived` 只定義「schema 預留，未實作」，沒定公式欄位名——沿用最
    /// 直覺的 `formula`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
}

impl FieldRule {
    pub fn for_kind(kind: FieldKind) -> Self {
        let (update, inject) = match kind {
            FieldKind::Number | FieldKind::Pair | FieldKind::Counter => {
                (UpdateMode::Delta, InjectLevel::Turn)
            }
            FieldKind::Roll => (UpdateMode::Local, InjectLevel::Turn),
            FieldKind::Text => (UpdateMode::Replace, InjectLevel::Snapshot),
            FieldKind::List => (UpdateMode::Replace, InjectLevel::Turn),
            FieldKind::ReadOnly | FieldKind::Derived => (UpdateMode::Reject, InjectLevel::Rare),
        };
        Self {
            kind,
            min: None,
            max: None,
            update,
            inject,
            branch: None,
            formula: None,
        }
    }
}

/// 觸發條件三型（拍板 13 的四種：計數器門檻與數值區間判斷邏輯逐字相同，
/// 差別只在欄位型別，共用 `Range` 不另立型別）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Condition {
    /// 數值區間：`min`／`max` 任一可省，`*_exclusive` 為真＝嚴格大於／小於。
    Range {
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<f64>,
        #[serde(default, skip_serializing_if = "is_false")]
        min_exclusive: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        max_exclusive: bool,
        /// 路徑不存在時當成這個值（來源腳本的 `{defaults: 0}`）；沒填就當條件不成立。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default: Option<f64>,
    },
    /// 字串包含：任一命中即成立。路徑不存在視為空字串。
    Contains { path: String, any: Vec<String> },
    /// 旗標：`expect` 為 false＝「還沒發生過」。路徑不存在視為 false。
    Flag { path: String, expect: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerMode {
    /// 區間型：條件成立就持續注入（關係階段、環境氛圍）。
    Range,
    /// 一次性事件：命中後把旗標釘成 true，模型翻不了案——事件演過就是演過。
    Once,
}

/// 一組觸發：來源是一條卡片腳本，`cases` 保留它 if／else-if 鏈的順序語意。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trigger {
    /// 穩定 id（來源條目標題正規化），`TableState.triggers` 與面板都用它對帳。
    pub id: String,
    pub title: String,
    pub mode: TriggerMode,
    /// 依序求值，第一個命中就停；空 `when` 的那筆＝else 兜底。
    pub cases: Vec<TriggerCase>,
    /// 命中文本前固定加的一段（來源腳本前言，通常是「當隱藏背景、別複述」）。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub preamble: String,
    /// 這組觸發看的是哪一支分支（點分路徑分段）——該支不在場就不注入，沿用包 5 的裁切。
    /// 空＝桌級，永遠注入。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope: Vec<String>,
    /// `Once` 專用：命中後要釘成 `true` 的旗標路徑（點分）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TriggerCase {
    /// 全部成立才算命中（AND）；空＝else 兜底。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub when: Vec<Condition>,
    /// 命中要注入的文本。`{{state:World.Invasion}}` 這種佔位在注入前換成現值。
    pub text: String,
}

fn mechanism_version() -> u32 {
    1
}

/// 規則留在桌級設定，逐則快照只存會隨對話改變的狀態值。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mechanism {
    #[serde(default = "mechanism_version")]
    pub version: u32,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rules: BTreeMap<String, FieldRule>,
    /// 觸發表：每回合本地求值，命中文本進下一輪回合尾。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggers: Vec<Trigger>,
    /// 這桌的數值走增量協定、由本地記帳（MVU 卡匯入後開啟）。
    #[serde(default, skip_serializing_if = "is_false")]
    pub incremental: bool,
    /// 這張卡自己的回報指引：介面接管後跟在通用協定後面進系統提示詞，講這張卡每回合
    /// 必報哪些欄位、哪些只在變動時報。空＝這桌沒有卡專屬規矩，只走通用協定。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub guide: String,
}

pub(crate) fn is_false(value: &bool) -> bool {
    !value
}

impl Default for Mechanism {
    fn default() -> Self {
        Self {
            version: mechanism_version(),
            rules: BTreeMap::new(),
            triggers: Vec::new(),
            incremental: false,
            guide: String::new(),
        }
    }
}

impl Mechanism {
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty() && self.triggers.is_empty() && !self.incremental
    }
}

/// 手動編輯撞到既有葉子時不覆寫資料夾，避免壞路徑把完整子樹吃掉。
pub fn set_tree_value(
    tree: &mut BTreeMap<String, StateNode>,
    path: &[String],
    value: &str,
) -> bool {
    if path.is_empty() {
        return false;
    }

    fn write(
        branch: &mut BTreeMap<String, StateNode>,
        path: &[String],
        value: &str,
    ) -> Option<bool> {
        let key = &path[0];
        if path.len() == 1 {
            if value.is_empty() {
                return Some(branch.remove(key).is_some());
            }
            let next = StateNode::Leaf(value.to_owned());
            let changed = branch.get(key) != Some(&next);
            if changed {
                branch.insert(key.clone(), next);
            }
            return Some(changed);
        }

        let child = match branch.get_mut(key) {
            Some(StateNode::Branch(child)) => child,
            Some(StateNode::Leaf(_)) => return None,
            None if value.is_empty() => return Some(false),
            None => {
                branch.insert(key.clone(), StateNode::Branch(BTreeMap::new()));
                let Some(StateNode::Branch(child)) = branch.get_mut(key) else {
                    return None;
                };
                child
            }
        };
        let changed = write(child, &path[1..], value)?;
        if changed && child.is_empty() {
            branch.remove(key);
        }
        Some(changed)
    }

    write(tree, path, value).unwrap_or(false)
}

/// 取路徑上的節點；路徑不存在或中途撞到葉子就是 None。
pub fn node_at<'a>(
    tree: &'a BTreeMap<String, StateNode>,
    path: &[String],
) -> Option<&'a StateNode> {
    let (first, rest) = path.split_first()?;
    let node = tree.get(first)?;
    if rest.is_empty() {
        return Some(node);
    }
    match node {
        StateNode::Branch(children) => node_at(children, rest),
        StateNode::Leaf(_) => None,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// 分岔用的顯示編號：key 是內部幕號字串，比照 scene_titles。沒進這個表的幕＝原線，查詢一律走 scene_label。
    #[serde(default)]
    pub scene_labels: BTreeMap<String, SceneLabel>,
    #[serde(default)]
    pub state: TableState,
    #[serde(default, skip_serializing_if = "Mechanism::is_empty")]
    pub mechanism: Mechanism,
    /// 已做過全樹對齊的場景號；與 current_scene 不同＝這一幕還沒對齊，下一輪 GM 回合送全樹。
    #[serde(default)]
    pub aligned_scene: Option<u64>,
    /// 面板指認的分支綁定：角色卡 id → 狀態樹路徑。沒指認的靠同名自動比對。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub branch_bindings: BTreeMap<String, Vec<String>>,
}

/// 分岔幕的顯示編號：內部幕號單調遞增不變，玩家看到的編號靠這個脫鉤。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneLabel {
    /// 玩家看到的幕號，內部 0 起算（前端顯示時再 +1）。
    pub base: u64,
    /// 同一個 base 的第幾個版本，1＝不顯示括號。
    pub version: u32,
    /// 上一幕的內部幕號；退回前幕靠它，不能再假設一定是「幕號 -1」。
    pub parent: Option<u64>,
    /// 這一幕是分岔複製來的（開頭那則是真實對話，不是前情提要）。
    /// 換幕來的幕開頭一定是摘要，分岔來的不是——凡是「改寫開頭那則」的操作都得先問這一格。
    #[serde(default)]
    pub forked: bool,
}

/// 沒進 scene_labels 的幕＝原線（舊存檔也走這條）：顯示編號就是內部幕號，第 1 版，上一幕是前一號。
pub fn scene_label(state: &WorldState, scene: u64) -> SceneLabel {
    state
        .scene_labels
        .get(&scene.to_string())
        .copied()
        .unwrap_or(SceneLabel {
            base: scene,
            version: 1,
            parent: scene.checked_sub(1),
            forked: false,
        })
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

/// claude lane 續聊狀態檔（prompt-cache-optimization 包 2）：worlds/<world_id>/lanes.json。
/// 本機工具狀態，壞檔或缺檔都只是重開續聊線，不影響正典資料。
pub(crate) fn lanes_path(root: &Path, world_id: &str) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?.join("lanes.json"))
}

/// 機制記帳落檔：worlds/<world_id>/mechanism-log.jsonl。
pub(crate) fn mechanism_log_path(root: &Path, world_id: &str) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?.join("mechanism-log.jsonl"))
}

/// 匯入收據落檔：worlds/<world_id>/import-receipts.json（JSON 陣列，append）。
pub(crate) fn import_receipts_path(root: &Path, world_id: &str) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?.join("import-receipts.json"))
}

/// 世界書路徑匯入的原始卡檔：worlds/<world_id>/source-card.<png|import.json>。
/// 卡片自帶介面要靠它，角色卡路徑則是留在角色檔旁邊。
pub(crate) fn world_card_path(root: &Path, world_id: &str, extension: &str) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?.join(format!("source-card.{extension}")))
}

/// GM 卡的圖：worlds/<world_id>/gm.png。世界書匯入的若是 PNG 卡就存這張，側欄 GM 卡改用它
/// 取代內建書本圖；沒有這檔就回退書本圖。
pub(crate) fn gm_image_path(root: &Path, world_id: &str) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?.join("gm.png"))
}

/// 介面渲染殼檔：worlds/<world_id>/interface-shell.html。AI 卡重構展開介面規則時，除了狀態樹
/// 初始值（state_fields）還可能多產一份自包含 HTML 殼；前端拿狀態樹的值替換殼內 `{{路徑}}`
/// 佔位符後塞進既有卡片沙盒 iframe（interface-card.ts buildShellDocument，下一包串接）。
pub(crate) fn interface_shell_path(root: &Path, world_id: &str) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?.join("interface-shell.html"))
}

/// AI 卡重構產物存檔：worlds/<world_id>/refactor-outcome.json。套用成功後落一份完整產物，供
/// 玩家之後從世界書工具列直接匯出重玩，不必重燒 AI 額度重新展開同一張卡；二次套用直接覆寫。
pub(crate) fn refactor_outcome_path(root: &Path, world_id: &str) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?.join("refactor-outcome.json"))
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
        scene_labels: BTreeMap::new(),
        state: TableState::default(),
        mechanism: Mechanism::default(),
        aligned_scene: None,
        branch_bindings: BTreeMap::new(),
    };
    fs::write(
        directory.join("state.json"),
        serde_json::to_string_pretty(&state)?,
    )?;
    Ok(id)
}

#[derive(Deserialize)]
struct SampleCharacterText {
    name: String,
    public_md: String,
    private_md: String,
}

#[derive(Deserialize)]
struct SampleWorldText {
    world_name: String,
    world_md: String,
    opening: String,
    characters: Vec<SampleCharacterText>,
}

fn sample_world_text(lang: &str) -> DataResult<SampleWorldText> {
    // 新增語系時只需新增 JSON 檔，並在這張對應表加一行；範例內容會隨執行檔靜態內嵌。
    let source = match lang {
        "zh-CN" => include_str!("../samples/zh-CN.json"),
        "en" => include_str!("../samples/en.json"),
        "ja" => include_str!("../samples/ja.json"),
        "ko" => include_str!("../samples/ko.json"),
        "es" => include_str!("../samples/es.json"),
        "pt-BR" => include_str!("../samples/pt-BR.json"),
        "de" => include_str!("../samples/de.json"),
        "fr" => include_str!("../samples/fr.json"),
        "ru" => include_str!("../samples/ru.json"),
        _ => include_str!("../samples/zh-TW.json"),
    };
    serde_json::from_str(source)
        .map_err(|error| invalid_data(format!("invalid embedded sample world JSON: {error}")))
}

/// 範例桌內容依語系產生（首開先選語言再建桌）；lang 非 en 一律走 zh-TW
pub fn create_sample_world(root: &Path, lang: &str) -> DataResult<String> {
    let sample = sample_world_text(lang)?;
    // 冪等：範例桌已在就直接沿用，避免重複呼叫（dev 的 StrictMode 雙跑）噴重複資料
    if let Some(existing) = list_worlds(root)?
        .into_iter()
        .find(|meta| meta.name == sample.world_name)
    {
        return Ok(existing.id);
    }
    let world_id = create_world(root, &sample.world_name)?;
    write_world_md(root, &world_id, &sample.world_md)?;

    let style = [
        ("#e07a5f", "🦊", Tier::Balanced),
        ("#3d84a8", "🛡️", Tier::Balanced),
        ("#f2a541", "🪕", Tier::Fast),
    ];
    if sample.characters.len() != style.len() {
        return Err(invalid_data(
            "sample world must contain exactly three characters",
        ));
    }
    for (text, (color, avatar, tier)) in sample.characters.into_iter().zip(style) {
        write_character(
            root,
            &world_id,
            &CharacterCard {
                id: new_id(),
                name: text.name,
                color: color.to_owned(),
                avatar: avatar.to_owned(),
                tier,
                show_image: true,
                archived: false,
                gen_prompt: String::new(),
                public_md: text.public_md,
                private_md: text.private_md,
            },
        )?;
    }

    append_transcript(
        root,
        &world_id,
        0,
        &TranscriptEvent {
            raw: None,
            ts: "2026-07-20T00:00:00+08:00".to_owned(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: TranscriptKind::Narration,
            text: sample.opening,
            state: None,
            gm_only: false,
        },
    )?;

    Ok(world_id)
}

/// 空桌回收（NewPlan §9.3）：只回收完全未動過的桌——零訊息、零角色、零世界書條目、
/// world.md 空白；任一項有內容即保留，防資料遺失。回傳是否真的刪了。
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
    // 世界書讀不動（檔案壞了）就當作有內容，寧可留著讓使用者自己處理，也不要刪掉
    let has_worldbook = read_worldbook_value(root, world_id)
        .map(|value| {
            value
                .get("entries")
                .and_then(serde_json::Value::as_object)
                .is_none_or(|entries| !entries.is_empty())
        })
        .unwrap_or(true);
    let world_md = fs::read_to_string(directory.join("world.md")).unwrap_or_default();
    if has_messages || has_characters || has_worldbook || !world_md.trim().is_empty() {
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

/// 讀介面渲染殼檔；沒產過或還沒套用就是 None（前端退回既有沙盒殼／保底狀態欄，不是錯誤）。
pub fn read_interface_shell(root: &Path, world_id: &str) -> DataResult<Option<String>> {
    let path = interface_shell_path(root, world_id)?;
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(fs::read_to_string(path)?))
}

pub fn write_interface_shell(root: &Path, world_id: &str, content: &str) -> DataResult<()> {
    fs::write(interface_shell_path(root, world_id)?, content)?;
    Ok(())
}

/// 讀 AI 卡重構套用成功時落下的完整產物（已是 to_string_pretty 過的 JSON 原文）；沒套用過
/// 就是 None（前端匯出鈕靠這個判斷要不要顯示「這桌還沒有重構產物」）。
pub fn read_refactor_outcome(root: &Path, world_id: &str) -> DataResult<Option<String>> {
    let path = refactor_outcome_path(root, world_id)?;
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(fs::read_to_string(path)?))
}

pub fn write_refactor_outcome(root: &Path, world_id: &str, content: &str) -> DataResult<()> {
    fs::write(refactor_outcome_path(root, world_id)?, content)?;
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

fn is_person_from_value(value: &serde_json::Value) -> bool {
    value
        .get("extensions")
        .and_then(|value| value.get("table_tavern"))
        .and_then(|value| value.get("is_person"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn set_is_person(value: &mut serde_json::Value, is_person: bool) {
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
        .insert("is_person".to_owned(), serde_json::Value::Bool(is_person));
}

fn locked_from_value(value: &serde_json::Value) -> bool {
    value
        .get("extensions")
        .and_then(|value| value.get("table_tavern"))
        .and_then(|value| value.get("locked"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn set_locked(value: &mut serde_json::Value, locked: bool) {
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
        .insert("locked".to_owned(), serde_json::Value::Bool(locked));
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
        is_person: is_person_from_value(value),
        locked: locked_from_value(value),
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
    set_is_person(value, entry.is_person);
    set_locked(value, entry.locked);
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
    set_is_person(&mut value, entry.is_person);
    set_locked(&mut value, entry.locked);
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

/// 狀態列的顯示格式一律由匯入的內容自己帶。比對詞跟 transport::extract_state_block
/// 認得的區塊一致——認得的才剝得出欄位，也才有東西可顯示。
const STATE_BAR_MARKERS: [&str; 12] = [
    "状态栏",
    "狀態欄",
    "状态条",
    "狀態條",
    "状态面板",
    "狀態面板",
    "status bar",
    "statusbar",
    // 標籤名各家自取，`<status` 開頭一律算（`<StatusData>`、`<Status_block>`）
    "<status",
    "<updatevariable",
    "```state",
    "```status",
];

fn declares_state_bar(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    STATE_BAR_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
}

/// 這桌要不要顯示狀態列：世界設定、世界書條目、角色卡任一處講到狀態列就顯示。
/// 匯入的卡有可能把狀態列規則放在世界書、也可能放在卡片內文，三處都掃才不會漏。
pub fn world_has_state_bar(root: &Path, world_id: &str) -> DataResult<bool> {
    if read_world_md(root, world_id).is_ok_and(|world_md| declares_state_bar(&world_md)) {
        return Ok(true);
    }
    if read_worldbook(root, world_id)?.iter().any(|entry| {
        !entry.disabled && (declares_state_bar(&entry.content) || declares_state_bar(&entry.title))
    }) {
        return Ok(true);
    }
    Ok(list_characters(root, world_id)?.iter().any(|meta| {
        read_character(root, world_id, &meta.id).is_ok_and(|card| {
            declares_state_bar(&card.public_md) || declares_state_bar(&card.private_md)
        })
    }))
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

/// 把世界書條目搬成可上桌的角色卡。
pub fn worldbook_entry_to_character(
    root: &Path,
    world_id: &str,
    uid: u64,
    color: String,
    as_player: bool,
) -> DataResult<CharacterMeta> {
    let worldbook = read_worldbook_value(root, world_id)?;
    let entries = entries_object(&worldbook)?;
    let entry = entries
        .iter()
        .find(|(key, value)| entry_uid(key, value) == Some(uid))
        .map(|(key, value)| entry_view(value, key.parse().ok()))
        .ok_or_else(|| invalid_data("找不到世界書條目"))?;
    if entry.title.trim().is_empty() {
        return Err(invalid_data("條目沒有標題，先給標題再轉"));
    }
    validate_single_line("name", &entry.title)?;

    let mut state = if as_player {
        let state = read_state(root, world_id)?;
        if state.player_card_id.is_some() {
            return Err(invalid_data("這桌已經有玩家卡"));
        }
        Some(state)
    } else {
        None
    };
    let card = CharacterCard {
        id: new_id(),
        name: entry.title,
        color,
        avatar: "🎭".to_owned(),
        tier: Tier::Balanced,
        show_image: true,
        archived: false,
        gen_prompt: String::new(),
        public_md: entry.content.trim().to_owned(),
        private_md: String::new(),
    };

    write_character(root, world_id, &card)?;
    if let Some(state) = state.as_mut() {
        state.player_card_id = Some(card.id.clone());
        write_state(root, world_id, state)?;
    }
    delete_worldbook_entry(root, world_id, uid)?;

    Ok(CharacterMeta {
        id: card.id,
        name: card.name,
        color: card.color,
        avatar: card.avatar,
        tier: card.tier,
        show_image: card.show_image,
        archived: card.archived,
        auto_hidden: false,
        display_index: None,
    })
}

/// 把封存角色卡搬回世界書。
pub fn character_to_worldbook_entry(
    root: &Path,
    world_id: &str,
    character_id: &str,
) -> DataResult<()> {
    let card = read_character(root, world_id, character_id)?;
    if !card.archived {
        return Err(invalid_data("這張卡還在桌上"));
    }
    let state = read_state(root, world_id)?;
    if state.player_card_id.as_deref() == Some(character_id) {
        return Err(invalid_data("玩家卡不能轉"));
    }

    let content = match (card.public_md.is_empty(), card.private_md.is_empty()) {
        (false, false) => format!("{}\n\n## 私有\n{}", card.public_md, card.private_md),
        (false, true) => card.public_md,
        (true, false) => card.private_md,
        (true, true) => String::new(),
    };
    let entry = WorldbookEntry {
        uid: 0,
        title: card.name,
        keys: Vec::new(),
        content,
        constant: true,
        order: 100,
        disabled: false,
        visibility: Visibility::Gm,
        is_person: false,
        locked: false,
    };
    let mut worldbook = read_worldbook_value(root, world_id)?;
    let entries = entries_object_mut(&mut worldbook)?;
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
    write_worldbook_value(root, world_id, &worldbook)?;
    delete_character(root, world_id, character_id)
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
    if is_mechanism_scaffold(&value) {
        if let Some(object) = value.as_object_mut() {
            object.insert("disable".to_owned(), serde_json::Value::Bool(true));
        }
    }
    Ok(value)
}

/// 機制鷹架條目：`[initvar]`／`[mvu_update]` 規則表、原生 EJS 腳本，或 ST 把整棵變數樹塞回提示詞的巨集。
/// 本地已接管或原本就不會交給模型的內容，不該再送進模型上下文燒字數。
fn is_mechanism_scaffold(entry: &serde_json::Value) -> bool {
    let marker = entry
        .get("comment")
        .and_then(serde_json::Value::as_str)
        .or_else(|| entry.get("title").and_then(serde_json::Value::as_str))
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if marker.starts_with("[initvar]") || marker.starts_with("[mvu_update]") {
        return true;
    }
    entry
        .get("content")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|content| {
            content.contains("{{format_message_variable::") || content.contains("<%")
        })
}

/// 條目的實質內容指紋：同一份世界書重複匯入時用它認出「一模一樣的條目」。
/// 只看標題、內文與兩組關鍵字——uid、順序、可見度等隨匯入產生的欄位不算差異。
fn entry_fingerprint(entry: &serde_json::Value) -> String {
    let text = |field: &str| {
        entry
            .get(field)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_owned()
    };
    let keys = |field: &str| {
        let mut items: Vec<String> = entry
            .get(field)
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(|key| key.trim().to_owned())
                    .collect()
            })
            .unwrap_or_default();
        items.sort();
        items.join("\u{1f}")
    };
    format!(
        "{}\u{1e}{}\u{1e}{}\u{1e}{}",
        text("comment"),
        text("content"),
        keys("key"),
        keys("keysecondary"),
    )
}

/// 匯入結果：`imported`＝真的寫進去的條數，`skipped`＝內容重複被略過的條數。
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct WorldbookImport {
    pub imported: usize,
    pub skipped: usize,
}

pub fn import_worldbook(
    root: &Path,
    world_id: &str,
    json_text: &str,
) -> DataResult<WorldbookImport> {
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
    let total = source_entries.len();
    let mut seen: HashSet<String> = entries.values().map(entry_fingerprint).collect();
    let mut uid = next_uid(entries)?;
    let mut imported = 0;
    let mut absorbed = Vec::new();
    for source_entry in source_entries {
        let entry = normalize_imported_entry(source_entry, character_book, uid)?;
        // 已經有一模一樣的條目就跳過，重複匯入同一份書不會塞出兩套內容
        if !seen.insert(entry_fingerprint(&entry)) {
            continue;
        }
        if is_mechanism_scaffold(&entry) {
            let title = entry
                .get("comment")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned();
            absorbed.push(Record {
                kind: RecordKind::Absorbed,
                path: title,
                detail: "機制鷹架條目，已由本地機制接管，不再送入提示詞。".to_owned(),
            });
        }
        entries.insert(uid.to_string(), entry);
        uid = uid
            .checked_add(1)
            .ok_or_else(|| invalid_data("worldbook uid overflow"))?;
        imported += 1;
    }
    write_worldbook_value(root, world_id, &worldbook)?;
    if !absorbed.is_empty() {
        let scene = read_state(root, world_id)
            .map(|state| state.current_scene)
            .unwrap_or(0);
        crate::mechanism::append_log(root, world_id, scene, &absorbed);
    }
    Ok(WorldbookImport {
        imported,
        skipped: total - imported,
    })
}

/// 清掉內容重複的條目：同一份指紋只留顯示順序最前的那條，回傳刪掉幾條。
/// 給去重上線前就已經重複匯入的桌收拾用。
pub fn dedupe_worldbook(root: &Path, world_id: &str) -> DataResult<usize> {
    let mut worldbook = read_worldbook_value(root, world_id)?;
    let entries = entries_object_mut(&mut worldbook)?;
    let mut seen = HashSet::new();
    let duplicates: Vec<String> = sorted_entry_keys(entries)
        .into_iter()
        .filter(|key| {
            entries
                .get(key)
                .is_some_and(|entry| !seen.insert(entry_fingerprint(entry)))
        })
        .collect();
    for key in &duplicates {
        entries.remove(key);
    }
    if !duplicates.is_empty() {
        write_worldbook_value(root, world_id, &worldbook)?;
    }
    Ok(duplicates.len())
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
    let mut auto_hidden = false;
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
            "auto_hidden" => auto_hidden = value == "true",
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
            auto_hidden,
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

/// `auto_hidden` 不是 `CharacterCard` 的欄位（那樣每個手動建卡的呼叫端都要補這個跟編輯
/// 無關的欄位）：呼叫端自己決定要延續舊值（`write_character`）還是寫新值
/// （`set_character_auto_hidden`），見兩者呼叫這支的方式。
fn serialize_character(card: &CharacterCard, display_index: u32, auto_hidden: bool) -> String {
    // frontmatter 逐行解析，生成提示詞中的換行須在寫入前攤平。
    let gen_prompt = card.gen_prompt.replace(['\n', '\r'], " ");
    format!(
        "---\nid: {}\nname: {}\ncolor: {}\navatar: {}\ntier: {}\nshow_image: {}\narchived: {}\nauto_hidden: {}\ndisplay_index: {}\ngen_prompt: {}\n---\n## 公開\n{}\n## 私有\n{}",
        card.id,
        card.name,
        card.color,
        card.avatar,
        card.tier.as_str(),
        card.show_image,
        card.archived,
        auto_hidden,
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
        let path = character_path(root, world_id, id)?;
        // 拖曳排序只改 display_index，跟 write_character 一樣延續磁碟上原有的 auto_hidden。
        let auto_hidden = existing_auto_hidden(&path);
        fs::write(path, serialize_character(&card, index, auto_hidden))?;
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

/// 這張卡目前落地的 auto_hidden 值；檔案不存在或解析失敗（新卡）一律當 false。
fn existing_auto_hidden(path: &Path) -> bool {
    let Ok(contents) = fs::read_to_string(path) else {
        return false;
    };
    parse_frontmatter(&contents)
        .map(|(meta, _, _)| meta.auto_hidden)
        .unwrap_or(false)
}

/// id 由呼叫端先跟 new_id 要好（草稿期生圖需要落在正確的圖庫路徑）；空 id 直接回錯。
/// `CharacterCard` 不帶 auto_hidden（AI 卡重構包 4b：那是換幕結算的持久欄位，不是編輯表單
/// 的一部分），這裡改寫其他欄位時，延續磁碟上原有的 auto_hidden，不會被前端編輯捎帶清掉。
pub fn write_character(root: &Path, world_id: &str, card: &CharacterCard) -> DataResult<()> {
    validate_id(&card.id)?;
    validate_single_line("name", &card.name)?;
    validate_single_line("color", &card.color)?;
    validate_single_line("avatar", &card.avatar)?;
    let path = character_path(root, world_id, &card.id)?;
    let auto_hidden = existing_auto_hidden(&path);
    ensure_display_indices(root, world_id)?;
    let display_index = display_index_for(root, world_id, &path)?;
    fs::write(path, serialize_character(card, display_index, auto_hidden))?;
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

/// 換幕結算（`settle_card_visibility`）專用：直接寫入新的 auto_hidden 值，其餘欄位原樣保留。
/// 不走 `write_character`（那支會延續磁碟舊值，寫不進新值）。
pub fn set_character_auto_hidden(
    root: &Path,
    world_id: &str,
    character_id: &str,
    auto_hidden: bool,
) -> DataResult<()> {
    let card = read_character(root, world_id, character_id)?;
    let path = character_path(root, world_id, character_id)?;
    let display_index = display_index_for(root, world_id, &path)?;
    fs::write(path, serialize_character(&card, display_index, auto_hidden))?;
    Ok(())
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
    let mut event = event.clone();
    if event.state.is_none() {
        // 復原舊句子會帶回當時快照，只有新事件才借用目前檯面。
        event.state = read_state(root, world_id).ok().map(|state| state.state);
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(transcript_path(root, world_id, scene)?)?;
    serde_json::to_writer(&mut file, &event)?;
    file.write_all(b"\n")?;
    // 目前值恆等於最後一則事件的快照，復原舊句時狀態才會跟著回到那一刻。
    // 快取寫失敗不該把「事件已經寫進去了」這件事變成錯誤，權威在 transcript。
    if let Some(snapshot) = event.state {
        if let Ok(mut world) = read_state(root, world_id) {
            if world.state != snapshot {
                world.state = snapshot;
                let _ = write_state(root, world_id, &world);
            }
        }
    }
    Ok(())
}

/// 開場白也要存成快照，收回時檯面才能回到貼上前的最後一句；狀態區塊走與 GM 回覆同一條
/// 本地權威（mechanism::apply_block），增量桌的數值一開場就是本機在算。
pub fn append_opening(
    root: &Path,
    world_id: &str,
    scene: u64,
    ts: &str,
    raw: &str,
    block: &crate::transport::StateBlock,
    user_name: &str,
) -> DataResult<(TranscriptEvent, Outcome)> {
    let mut world = read_state(root, world_id)?;
    let outcome = mechanism::apply_block(&mut world, block, user_name);
    let event = TranscriptEvent {
        ts: ts.to_owned(),
        speaker_id: String::new(),
        speaker_name: "GM".to_owned(),
        kind: TranscriptKind::Narration,
        text: block.display.clone(),
        raw: (raw != block.display).then(|| raw.to_owned()),
        state: Some(world.state),
        gm_only: false,
    };
    append_transcript(root, world_id, scene, &event)?;
    Ok((event, outcome))
}

/// 整檔重寫這一幕，並把檯面退回剩下事件的最後一份快照（這一幕沒了就往前一幕找）。
/// 刪事件的兩條路（收回上一句、復原匯入收掉開場白）共用。
fn rewrite_scene(
    root: &Path,
    world_id: &str,
    scene: u64,
    events: &[TranscriptEvent],
) -> DataResult<()> {
    let mut buffer = String::new();
    for event in events {
        buffer.push_str(&serde_json::to_string(event)?);
        buffer.push('\n');
    }
    fs::write(transcript_path(root, world_id, scene)?, buffer)?;
    let mut state = read_state(root, world_id)?;
    state.state = events
        .iter()
        .rev()
        .find_map(|entry| entry.state.clone())
        .or_else(|| {
            scene.checked_sub(1).and_then(|previous_scene| {
                read_transcript(root, world_id, previous_scene)
                    .ok()
                    .and_then(|previous_events| {
                        previous_events
                            .iter()
                            .rev()
                            .find_map(|entry| entry.state.clone())
                    })
            })
        })
        .unwrap_or_default();
    write_state(root, world_id, &state)?;
    Ok(())
}

/// 狀態樹被逐字稿以外的路徑換掉（重構套用重建欄位）之後，把新樹補進這一幕每一則事件的快照。
/// 收回上一句與換幕都拿事件快照當回捲基準，不補的話玩家一收回，介面就被打回重構前的舊欄位。
/// 補整幕而不是只補最後一則：連按收回會一路往前吃，任何一則留著舊欄位都會在那一下現形。
/// 只換 tree／jumps——劇情面的欄位（table、changes、notes）照舊跟著各自那一刻走。
pub fn sync_scene_state_tree(root: &Path, world_id: &str, state: &WorldState) -> DataResult<()> {
    let scene = state.current_scene;
    let mut events = read_transcript(root, world_id, scene)?;
    let mut touched = false;
    for event in events.iter_mut() {
        let Some(snapshot) = event.state.as_mut() else {
            continue;
        };
        if snapshot.tree != state.state.tree || snapshot.jumps != state.state.jumps {
            snapshot.tree = state.state.tree.clone();
            snapshot.jumps = state.state.jumps.clone();
            touched = true;
        }
    }
    if touched {
        rewrite_scene(root, world_id, scene, &events)?;
    }
    Ok(())
}

/// 收回上一句（可連按）：砍掉這一幕最後一筆事件後整檔重寫。
/// 回傳是否真的刪了——這一幕已經空了就是 false，收不會倒退咬到上一幕。
pub fn pop_transcript(root: &Path, world_id: &str, scene: u64) -> DataResult<bool> {
    let mut events = read_transcript(root, world_id, scene)?;
    if events.pop().is_none() {
        return Ok(false);
    }
    rewrite_scene(root, world_id, scene, &events)?;
    Ok(true)
}

/// 復原匯入用：從這一幕刪掉時間戳相符的那一則（貼出的開場白），其餘事件原位不動。
/// 回傳是否真的刪到——玩家自己先收回過就是 false。
pub fn remove_transcript_event(
    root: &Path,
    world_id: &str,
    scene: u64,
    ts: &str,
) -> DataResult<bool> {
    let mut events = read_transcript(root, world_id, scene)?;
    let before = events.len();
    events.retain(|event| event.ts != ts);
    if events.len() == before {
        return Ok(false);
    }
    rewrite_scene(root, world_id, scene, &events)?;
    Ok(true)
}

pub fn set_last_transcript_state(
    root: &Path,
    world_id: &str,
    scene: u64,
    state: &TableState,
) -> DataResult<bool> {
    let mut events = read_transcript(root, world_id, scene)?;
    let Some(entry) = events.last_mut() else {
        return Ok(false);
    };
    entry.state = Some(state.clone());
    let mut buffer = String::new();
    for entry in &events {
        buffer.push_str(&serde_json::to_string(entry)?);
        buffer.push('\n');
    }
    fs::write(transcript_path(root, world_id, scene)?, buffer)?;
    Ok(true)
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

// ---------------------------------------------------------------------
// AI 卡重構包 4b：角色卡自動上下場共用的登場掃描原語。人物（transport::PERSON_ARRIVAL_PREFIX）
// 與角色卡（CARD_ARRIVAL_PREFIX）登場比對邏輯相同、鍵不同，這裡放兩邊都用得到、且
// 換幕結算（本檔 begin_next_scene）必須直接呼叫、不能反過來依賴 transport 的最小共用集合。
// ---------------------------------------------------------------------

/// 角色卡回歸事件的固定前綴，接著是〈name〉那一行——跟世界書人物的登場前綴
/// （transport::PERSON_ARRIVAL_PREFIX）分開，掃 transcript 或前端呈現時才分得出兩種來源。
pub const CARD_ARRIVAL_PREFIX: &str = "（角色回歸）";

/// 從一則事件文字剝出前綴後的〈title〉；prefix 不符或沒有〈〉包住就回 None。
pub(crate) fn bracket_title(text: &str, prefix: &str) -> Option<String> {
    let rest = text.strip_prefix(prefix)?.strip_prefix('〈')?;
    let end = rest.find('〉')?;
    Some(rest[..end].to_owned())
}

/// 本幕已登場（依指定前綴）集合：掃 System 事件取出〈title〉。
pub(crate) fn appeared_titles(events: &[TranscriptEvent], prefix: &str) -> BTreeSet<String> {
    events
        .iter()
        .filter(|event| event.kind == TranscriptKind::System)
        .filter_map(|event| bracket_title(&event.text, prefix))
        .collect()
}

/// present 欄的斷詞規則：頓號／逗號／斜線／分號，trim 後濾空。
pub(crate) fn split_present_names(raw: &str) -> Vec<String> {
    raw.split(['、', '，', ',', '／', '/', '；', ';'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect()
}

/// 在場名字跟標題比對：雙向包含，「亞歷山大」對得上「亞歷山大・馮・史特勞斯」。
pub(crate) fn name_matches(name: &str, title: &str) -> bool {
    title.contains(name) || name.contains(title)
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

/// 換幕摘要固定前綴：新幕開頭與重寫前情提要共用同一套語系文案，避免兩處各自維護。
fn format_scene_summary(summary_text: &str, lang: &str) -> String {
    if lang == "en" {
        format!("Previously:\n{summary_text}")
    } else {
        format!("【前情提要】\n{summary_text}")
    }
}

/// 算「某個 base 目前該排第幾個版本」：掃 0..=upto 每一幕的顯示 base，數出撞號的幕數再 +1。
/// begin_next_scene 與 fork_scene 都靠它算新標籤，掃描範圍在插入新標籤之前的呼叫端已經固定。
fn next_scene_version(state: &WorldState, upto: u64, base: u64) -> u32 {
    (0..=upto)
        .filter(|&scene| scene_label(state, scene).base == base)
        .count() as u32
        + 1
}

/// 分岔：把某一幕的紀錄原樣複製成新的一幕接著玩，原本歷史一個字都不動。
/// 顯示編號跟隨來源幕（從分岔幕再分岔＝跟著源頭走，不是跟著內部號走），
/// parent 記分岔當下所在的幕，退回時回到這裡而不是來源幕。
pub fn fork_scene(root: &Path, world_id: &str, from_scene: u64) -> DataResult<u64> {
    let mut state = read_state(root, world_id)?;
    if from_scene >= state.current_scene {
        return Err(invalid_data("只能從前面的幕分岔"));
    }
    let events = read_transcript(root, world_id, from_scene)?;
    if events.is_empty() {
        return Err(invalid_data("這一幕沒有紀錄可以接續"));
    }

    let current_scene = state.current_scene;
    let new_scene = current_scene + 1;
    let mut buffer = String::new();
    for event in &events {
        buffer.push_str(&serde_json::to_string(event)?);
        buffer.push('\n');
    }
    fs::write(transcript_path(root, world_id, new_scene)?, buffer)?;

    let base = scene_label(&state, from_scene).base;
    let version = next_scene_version(&state, current_scene, base);
    state.scene_labels.insert(
        new_scene.to_string(),
        SceneLabel {
            base,
            version,
            parent: Some(current_scene),
            forked: true,
        },
    );
    state.current_scene = new_scene;
    state.state = events
        .iter()
        .rev()
        .find_map(|event| event.state.clone())
        .unwrap_or_default();
    write_state(root, world_id, &state)?;
    Ok(new_scene)
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
    append_transcript(
        root,
        world_id,
        next_scene,
        &TranscriptEvent {
            raw: None,
            ts: local_timestamp()?,
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: TranscriptKind::Narration,
            text: format_scene_summary(summary_text, lang),
            state: None,
            gm_only: false,
        },
    )?;
    if let Some(name) = title.map(str::trim).filter(|name| !name.is_empty()) {
        state
            .scene_titles
            .insert(old_scene.to_string(), name.to_owned());
    }
    let base = scene_label(&state, old_scene).base + 1;
    let version = next_scene_version(&state, old_scene, base);
    state.scene_labels.insert(
        next_scene.to_string(),
        SceneLabel {
            base,
            version,
            parent: Some(old_scene),
            forked: false,
        },
    );
    state.current_scene = next_scene;
    write_state(root, world_id, &state)?;
    settle_card_visibility(
        root,
        world_id,
        old_scene,
        state.state.table.get("present").map(String::as_str),
    );
    Ok(next_scene)
}

/// 換幕結算角色卡自動隱藏（AI 卡重構包 4b；鐵律：auto_hidden 這個持久欄位只在換幕動，
/// 幕中回合的登場偵測只 append 事件不改欄位，見 lib.rs record_card_arrivals）。
///
/// 出現過＝(a) 剛結束那幕的角色卡回歸事件集合 ∪ (b) 換幕當下 present 名單比對命中。
/// 出現過→auto_hidden=false（拉回主區）；沒出現過→auto_hidden=true（收進隱藏區）；
/// archived（手動封存）的卡完全不動，自動判斷不能覆蓋玩家的手動決定。
///
/// 已知限制：幕開始就在主區、全程活躍，但最後一輪 GM 忘記把它列進 present、
/// 這幕本文也沒有登場事件（因為它本來就沒被隱藏過）的卡，會在這裡被判定「沒出現」
/// 而轉為隱藏——(a)(b) 都掃不到這種情況；真正掃「正文有沒有提到名字」(c) 成本較高
/// （要跑完整幕全部旁白文字），先不做，之後真的常誤判再考慮補。
///
/// 結算失敗一律吞掉：換幕本身已經成功，auto_hidden 記帳不該反過來讓換幕報錯。
fn settle_card_visibility(root: &Path, world_id: &str, ended_scene: u64, present: Option<&str>) {
    let Ok(characters) = list_characters(root, world_id) else {
        return;
    };
    let events = read_transcript(root, world_id, ended_scene).unwrap_or_default();
    let arrived = appeared_titles(&events, CARD_ARRIVAL_PREFIX);
    let present_names = present.map(split_present_names);
    for meta in characters {
        if meta.archived {
            continue;
        }
        let appeared = arrived.iter().any(|name| name_matches(name, &meta.name))
            || present_names
                .as_ref()
                .is_some_and(|names| names.iter().any(|name| name_matches(name, &meta.name)));
        let _ = set_character_auto_hidden(root, world_id, &meta.id, !appeared);
    }
}

/// 退回前幕：換幕的精確反向操作，純本地檔案處理不必呼叫模型。
/// 前一幕看 scene_labels 的 parent（原線／分岔都適用），不再假設一定是「幕號 -1」。
/// 只認「這一幕剛好一則事件」——begin_next_scene 保證新幕開頭就是那則摘要，
/// 多於一則代表玩家已經在這一幕行動過，退回會悄悄吃掉那些內容，所以直接擋，
/// 且擋下時故意先不動任何檔案／狀態（讀完才判斷），錯誤路徑不留副作用。
pub fn revert_scene(root: &Path, world_id: &str) -> DataResult<u64> {
    let mut state = read_state(root, world_id)?;
    let scene = state.current_scene;
    let Some(previous_scene) = scene_label(&state, scene).parent else {
        return Err(invalid_data("已經是第一幕，沒有前幕可以退回"));
    };
    let events = read_transcript(root, world_id, scene)?;
    if events.len() != 1 {
        return Err(invalid_data("這一幕已經有新內容，不能退回前幕"));
    }

    fs::remove_file(transcript_path(root, world_id, scene)?)?;
    state.current_scene = previous_scene;
    state.scene_titles.remove(&previous_scene.to_string());
    // 自己這筆標籤跟著檔案一起消失，不留退回後查不到來源、卻還佔著 key 的殭屍紀錄。
    state.scene_labels.remove(&scene.to_string());
    // current_scene 落回前幕，前幕本來就對齊過了，aligned_scene 不用跟著動。
    state.state = read_transcript(root, world_id, previous_scene)?
        .iter()
        .rev()
        .find_map(|event| event.state.clone())
        .unwrap_or_default();
    write_state(root, world_id, &state)?;
    Ok(previous_scene)
}

/// 重寫目前這幕唯一那則摘要：摘要不滿意可以直接原地覆寫，不必先退回再重新換幕一次。
pub fn replace_scene_summary(
    root: &Path,
    world_id: &str,
    summary_text: &str,
    lang: &str,
    title: Option<&str>,
) -> DataResult<()> {
    let mut state = read_state(root, world_id)?;
    let scene = state.current_scene;
    let label = scene_label(&state, scene);
    let Some(previous_scene) = label.parent else {
        return Err(invalid_data("第一幕沒有前情提要可以重寫"));
    };
    // 分岔幕開頭那則是複製來的真實對話，不是摘要。源頭幕剛好只有一則時
    // 「只有一則」這道守門會放行，覆寫下去就把玩家的對話換成摘要了。
    if label.forked {
        return Err(invalid_data("這一幕是從前幕接續來的，開頭不是前情提要"));
    }
    let mut events = read_transcript(root, world_id, scene)?;
    if events.len() != 1 {
        return Err(invalid_data("這一幕已經有新內容，不能重寫前情提要"));
    }

    // 重寫的只有文字，其餘欄位原樣留著——尤其 state 那份快照：
    // 摘要是這一幕唯一一則，快照掉了之後退回這一幕會把狀態欄清成空的。
    let event = &mut events[0];
    event.text = format_scene_summary(summary_text, lang);
    event.ts = local_timestamp()?;
    fs::write(
        transcript_path(root, world_id, scene)?,
        format!("{}\n", serde_json::to_string(event)?),
    )?;

    match title.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => {
            state
                .scene_titles
                .insert(previous_scene.to_string(), name.to_owned());
        }
        None => {
            state.scene_titles.remove(&previous_scene.to_string());
        }
    }
    write_state(root, world_id, &state)?;
    Ok(())
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

    if object.get("type").and_then(serde_json::Value::as_str) != Some("table-tavern-sponsor-pack") {
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
        entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "ttpack")
            && fs::read(entry.path()).is_ok_and(|bytes| validate_sponsor_pack(&bytes).is_ok())
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
            is_person: false,
            locked: false,
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
        assert!(
            validate_sponsor_pack(br#"{"type":"table-tavern-sponsor-pack","format":1}"#).is_ok()
        );
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
            import_worldbook(root.path(), &source, &imported.to_string())
                .unwrap()
                .imported,
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
            import_worldbook(root.path(), &destination, &exported_text)
                .unwrap()
                .imported,
            entries.len()
        );
        assert_eq!(
            read_worldbook(root.path(), &destination).unwrap().len(),
            entries.len()
        );
    }

    #[test]
    fn import_skips_entries_identical_to_existing_ones() {
        let root = TestRoot::new("worldbook-dedupe");
        let world_id = create_world(root.path(), "世界").unwrap();
        let book = serde_json::json!({
            "entries": {
                "0": {
                    "uid": 0,
                    "key": ["城門", "夜"],
                    "comment": "城門",
                    "content": "城門已關。",
                    "constant": false,
                    "order": 1,
                    "disable": false
                },
                "1": {
                    "uid": 1,
                    "key": ["市集"],
                    "comment": "市集",
                    "content": "市集喧鬧。",
                    "constant": false,
                    "order": 2,
                    "disable": false
                }
            }
        });
        let first = import_worldbook(root.path(), &world_id, &book.to_string()).unwrap();
        assert_eq!(
            first,
            WorldbookImport {
                imported: 2,
                skipped: 0
            }
        );

        // 同一份書再匯一次：內容一模一樣，全部略過
        let again = import_worldbook(root.path(), &world_id, &book.to_string()).unwrap();
        assert_eq!(
            again,
            WorldbookImport {
                imported: 0,
                skipped: 2
            }
        );
        assert_eq!(read_worldbook(root.path(), &world_id).unwrap().len(), 2);

        // 關鍵字順序不同、內文前後有空白＝同一條；改過內文的才算新條目
        let mixed = serde_json::json!({
            "entries": {
                "0": {
                    "uid": 0,
                    "key": ["夜", "城門"],
                    "comment": "城門",
                    "content": "  城門已關。  ",
                    "constant": false,
                    "order": 1,
                    "disable": false
                },
                "1": {
                    "uid": 1,
                    "key": ["市集"],
                    "comment": "市集",
                    "content": "市集已散。",
                    "constant": false,
                    "order": 2,
                    "disable": false
                }
            }
        });
        let third = import_worldbook(root.path(), &world_id, &mixed.to_string()).unwrap();
        assert_eq!(
            third,
            WorldbookImport {
                imported: 1,
                skipped: 1
            }
        );
        assert_eq!(read_worldbook(root.path(), &world_id).unwrap().len(), 3);
    }

    /// 機制鷹架條目（[initvar]／[mvu_update]／整棵樹重送巨集）匯入後要被系統關掉，
    /// 不再送模型；一般條目完全不受影響。
    #[test]
    fn import_worldbook_disables_mechanism_scaffold_entries_and_leaves_others_alone() {
        let root = TestRoot::new("worldbook-absorb");
        let world_id = create_world(root.path(), "世界").unwrap();
        let book = serde_json::json!({
            "entries": [
                {
                    "keys": ["初始"],
                    "comment": "[initvar] 初始值",
                    "content": "World:\n  Time: 清晨",
                    "enabled": false
                },
                {
                    "keys": [],
                    "comment": "[mvu_update] 規則",
                    "content": "规则:\n  World:\n    HP:\n      type: number",
                    "enabled": true
                },
                {
                    "keys": [],
                    "comment": "整棵樹重送",
                    "content": "{{format_message_variable::World}}",
                    "enabled": true
                },
                {
                    "keys": ["城門"],
                    "comment": "城門",
                    "content": "城門已關。",
                    "enabled": true
                }
            ]
        });
        import_worldbook(root.path(), &world_id, &book.to_string()).unwrap();
        let entries = read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(entries.len(), 4);
        for entry in &entries {
            let should_be_disabled = entry.title != "城門";
            assert_eq!(entry.disabled, should_be_disabled, "{}", entry.title);
        }
    }

    #[test]
    fn dedupe_keeps_first_of_each_duplicate_group() {
        let root = TestRoot::new("worldbook-dedupe-command");
        let world_id = create_world(root.path(), "世界").unwrap();
        let entry = |uid: u64, comment: &str, content: &str, order: u64| {
            serde_json::json!({
                "uid": uid,
                "key": ["k"],
                "comment": comment,
                "content": content,
                "constant": false,
                "order": order,
                "disable": false
            })
        };
        let book = serde_json::json!({
            "entries": {
                "0": entry(0, "城門", "城門已關。", 1),
                "1": entry(1, "市集", "市集喧鬧。", 2),
                "2": entry(2, "城門", "城門已關。", 3),
                "3": entry(3, "城門", "城門大開。", 4)
            }
        });
        write_worldbook_value(root.path(), &world_id, &book).unwrap();

        assert_eq!(dedupe_worldbook(root.path(), &world_id).unwrap(), 1);
        let entries = read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(entries.len(), 3);
        // 留下的是排在最前面那條，被留下的內容一條不少
        assert_eq!(entries[0].uid, 0);
        assert_eq!(entries[1].uid, 1);
        assert_eq!(entries[2].content, "城門大開。");
        // 再按一次沒東西可清
        assert_eq!(dedupe_worldbook(root.path(), &world_id).unwrap(), 0);
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
            import_worldbook(root.path(), &world_id, &character_book.to_string())
                .unwrap()
                .imported,
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
    fn worldbook_entry_to_character_moves_content_and_keeps_other_entries() {
        let root = TestRoot::new("worldbook-entry-to-character");
        let world_id = create_world(root.path(), "世界").unwrap();
        let mut source = worldbook_entry(u64::MAX, "霧港船長");
        source.content = "第一段\n\n第二段".to_owned();
        let source_uid = upsert_worldbook_entry(root.path(), &world_id, source).unwrap();
        let other_uid = upsert_worldbook_entry(
            root.path(),
            &world_id,
            worldbook_entry(u64::MAX, "留下的條目"),
        )
        .unwrap();

        let meta = worldbook_entry_to_character(
            root.path(),
            &world_id,
            source_uid,
            "#123456".to_owned(),
            false,
        )
        .unwrap();

        assert_eq!(meta.name, "霧港船長");
        assert_eq!(
            read_character(root.path(), &world_id, &meta.id)
                .unwrap()
                .public_md,
            "第一段\n\n第二段"
        );
        assert_eq!(
            read_worldbook(root.path(), &world_id)
                .unwrap()
                .iter()
                .map(|entry| entry.uid)
                .collect::<Vec<_>>(),
            [other_uid]
        );
    }

    #[test]
    fn worldbook_entry_to_player_card_sets_state_and_rejects_second_card() {
        let root = TestRoot::new("worldbook-entry-to-player");
        let world_id = create_world(root.path(), "世界").unwrap();
        let first_uid =
            upsert_worldbook_entry(root.path(), &world_id, worldbook_entry(u64::MAX, "玩家"))
                .unwrap();
        let second_uid = upsert_worldbook_entry(
            root.path(),
            &world_id,
            worldbook_entry(u64::MAX, "候補玩家"),
        )
        .unwrap();

        let player = worldbook_entry_to_character(
            root.path(),
            &world_id,
            first_uid,
            "#abcdef".to_owned(),
            true,
        )
        .unwrap();
        assert_eq!(
            read_state(root.path(), &world_id).unwrap().player_card_id,
            Some(player.id)
        );

        assert_eq!(
            worldbook_entry_to_character(
                root.path(),
                &world_id,
                second_uid,
                "#abcdef".to_owned(),
                true,
            )
            .unwrap_err()
            .to_string(),
            "這桌已經有玩家卡"
        );
        assert!(read_worldbook(root.path(), &world_id)
            .unwrap()
            .iter()
            .any(|entry| entry.uid == second_uid));
    }

    /// 狀態列只跟著匯入內容走：光提到「狀態」不算，要有狀態列輸出格式才算。
    #[test]
    fn state_bar_follows_imported_content() {
        let root = TestRoot::new("state-bar-detection");
        let world_id = create_world(root.path(), "世界").unwrap();
        assert!(!world_has_state_bar(root.path(), &world_id).unwrap());

        let mut prose = worldbook_entry(u64::MAX, "獵物狀態設定");
        prose.content = "User 的身體狀態具備超高耐受。".to_owned();
        upsert_worldbook_entry(root.path(), &world_id, prose).unwrap();
        assert!(!world_has_state_bar(root.path(), &world_id).unwrap());

        let mut rules = worldbook_entry(u64::MAX, "Day Counter");
        rules.content =
            "<details>\n<summary>状态栏</summary>\n- 沦陷天数：第 [X] 天\n</details>".to_owned();
        upsert_worldbook_entry(root.path(), &world_id, rules).unwrap();
        assert!(world_has_state_bar(root.path(), &world_id).unwrap());
    }

    /// 狀態列規則也可能寫在卡片內文（匯入角色卡時世界書會併進私有段）
    #[test]
    fn state_bar_detected_in_character_card() {
        let root = TestRoot::new("state-bar-character");
        let world_id = create_world(root.path(), "世界").unwrap();
        let mut card = character_card(&new_id(), "教官");
        card.private_md = "每次回覆結尾輸出 <UpdateVariable> 區塊。".to_owned();
        write_character(root.path(), &world_id, &card).unwrap();

        assert!(world_has_state_bar(root.path(), &world_id).unwrap());
    }

    #[test]
    fn worldbook_entry_to_character_rejects_empty_title_without_deleting() {
        let root = TestRoot::new("worldbook-entry-empty-title");
        let world_id = create_world(root.path(), "世界").unwrap();
        let uid = upsert_worldbook_entry(root.path(), &world_id, worldbook_entry(u64::MAX, "  "))
            .unwrap();

        assert_eq!(
            worldbook_entry_to_character(root.path(), &world_id, uid, "#abcdef".to_owned(), false,)
                .unwrap_err()
                .to_string(),
            "條目沒有標題，先給標題再轉"
        );
        assert!(read_worldbook(root.path(), &world_id)
            .unwrap()
            .iter()
            .any(|entry| entry.uid == uid));
    }

    #[test]
    fn character_to_worldbook_entry_moves_archived_card_and_private_content() {
        let root = TestRoot::new("character-to-worldbook-entry");
        let world_id = create_world(root.path(), "世界").unwrap();
        let mut card = character_card(&new_id(), "封存船長");
        card.archived = true;
        card.public_md = "公開設定".to_owned();
        card.private_md = "GM 秘密".to_owned();
        write_character(root.path(), &world_id, &card).unwrap();

        character_to_worldbook_entry(root.path(), &world_id, &card.id).unwrap();

        let entries = read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "封存船長");
        assert_eq!(entries[0].content, "公開設定\n\n## 私有\nGM 秘密");
        assert!(entries[0].constant);
        assert_eq!(entries[0].visibility, Visibility::Gm);
        assert_eq!(entries[0].order, 100);
        assert!(read_character(root.path(), &world_id, &card.id).is_err());
    }

    #[test]
    fn character_to_worldbook_entry_rejects_active_and_player_cards() {
        let root = TestRoot::new("character-to-worldbook-rejects");
        let world_id = create_world(root.path(), "世界").unwrap();
        let active = character_card(&new_id(), "還在桌上");
        write_character(root.path(), &world_id, &active).unwrap();
        assert_eq!(
            character_to_worldbook_entry(root.path(), &world_id, &active.id)
                .unwrap_err()
                .to_string(),
            "這張卡還在桌上"
        );

        let mut player = character_card(&new_id(), "玩家");
        player.archived = true;
        write_character(root.path(), &world_id, &player).unwrap();
        let mut state = read_state(root.path(), &world_id).unwrap();
        state.player_card_id = Some(player.id.clone());
        write_state(root.path(), &world_id, &state).unwrap();
        assert_eq!(
            character_to_worldbook_entry(root.path(), &world_id, &player.id)
                .unwrap_err()
                .to_string(),
            "玩家卡不能轉"
        );
        assert!(read_character(root.path(), &world_id, &player.id).is_ok());
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
                auto_hidden: false,
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

    /// 驗收：每個上架語系都有自己的範例桌內容且建得起來——
    /// 少一個 samples/<lang>.json、欄位漏了、或桌名忘了翻，都會在這裡爆
    #[test]
    fn sample_world_ready_in_every_language() {
        let zh_root = TestRoot::new("sample-world-lang-zh");
        let zh_id = create_sample_world(zh_root.path(), "zh-TW").unwrap();
        let zh_name = read_state(zh_root.path(), &zh_id).unwrap().name;

        for lang in ["zh-CN", "en", "ja", "ko", "es", "pt-BR", "de", "fr", "ru"] {
            let root = TestRoot::new(&format!("sample-world-lang-{lang}"));
            let world_id = create_sample_world(root.path(), lang).unwrap();

            let characters = list_characters(root.path(), &world_id).unwrap();
            assert_eq!(characters.len(), 3, "{lang} 角色數不對");
            for meta in &characters {
                assert!(!meta.name.trim().is_empty(), "{lang} 角色沒名字");
                let card = read_character(root.path(), &world_id, &meta.id).unwrap();
                assert!(!card.public_md.trim().is_empty(), "{lang} 缺公開設定");
                assert!(!card.private_md.trim().is_empty(), "{lang} 缺 GM 秘密");
            }

            assert!(
                !read_world_md(root.path(), &world_id)
                    .unwrap()
                    .trim()
                    .is_empty(),
                "{lang} 世界設定是空的"
            );

            let transcript = read_transcript(root.path(), &world_id, 0).unwrap();
            assert_eq!(transcript.len(), 1, "{lang} 開場旁白數不對");
            assert!(
                !transcript[0].text.trim().is_empty(),
                "{lang} 開場旁白是空的"
            );

            assert_ne!(
                read_state(root.path(), &world_id).unwrap().name,
                zh_name,
                "{lang} 桌名沒翻"
            );
        }
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
            raw: None,
            ts: "2026-07-19T00:00:00Z".to_owned(),
            speaker_id: String::new(),
            speaker_name: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "你好".to_owned(),
            state: None,
            gm_only: false,
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
            raw: None,
            ts: "2026-07-19T00:00:00Z".to_owned(),
            speaker_id: String::new(),
            speaker_name: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "留著".to_owned(),
            state: None,
            gm_only: false,
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

        // 匯入世界書但還沒改桌名、也還沒開聊，一樣算動過（回歸：曾被誤刪整桌）
        let has_worldbook = create_world(root.path(), "有世界書").unwrap();
        upsert_worldbook_entry(root.path(), &has_worldbook, worldbook_entry(1, "霧之港")).unwrap();
        assert!(!reclaim_world_if_empty(root.path(), &has_worldbook).unwrap());

        // 世界書檔案壞掉時保守保留，不刪
        let broken_worldbook = create_world(root.path(), "壞世界書").unwrap();
        fs::write(
            root.path()
                .join(format!("worlds/{broken_worldbook}/worldbook.json")),
            "{ not json",
        )
        .unwrap();
        assert!(!reclaim_world_if_empty(root.path(), &broken_worldbook).unwrap());
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
                auto_hidden: false,
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
                "auto_hidden",
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
                raw: None,
                ts: "2026-07-27 12:00".to_owned(),
                speaker_id: card.id.clone(),
                speaker_name: "舊名".to_owned(),
                kind: TranscriptKind::Dialogue,
                text: "舊名說了一句話".to_owned(),
                state: None,
                gm_only: false,
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
                raw: None,
                ts: "2026-07-19T10:00:00+08:00".to_owned(),
                speaker_id: String::new(),
                speaker_name: "旁白".to_owned(),
                kind: TranscriptKind::Narration,
                text: "序幕".to_owned(),
                state: None,
                gm_only: false,
            },
            TranscriptEvent {
                raw: None,
                ts: "2026-07-19T10:00:01+08:00".to_owned(),
                speaker_id: String::new(),
                speaker_name: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "第一行\n仍是同一事件".to_owned(),
                state: None,
                gm_only: false,
            },
            TranscriptEvent {
                raw: None,
                ts: "2026-07-19T10:00:02+08:00".to_owned(),
                speaker_id: "角色代碼".to_owned(),
                speaker_name: "角色".to_owned(),
                kind: TranscriptKind::Dialogue,
                text: "你好".to_owned(),
                state: None,
                gm_only: false,
            },
        ];
        for event in &events {
            append_transcript(root.path(), &world_id, 7, event).unwrap();
        }
        let expected: Vec<_> = events
            .iter()
            .cloned()
            .map(|mut event| {
                event.state = Some(TableState::default());
                event
            })
            .collect();
        assert_eq!(
            read_transcript(root.path(), &world_id, 7).unwrap(),
            expected
        );

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
    fn pop_transcript_removes_last_event_until_scene_is_empty() {
        let root = TestRoot::new("transcript-pop");
        let world_id = create_world(root.path(), "收回桌").unwrap();
        let events: Vec<TranscriptEvent> = ["序幕", "我推開門", "誰在那裡？"]
            .iter()
            .enumerate()
            .map(|(index, text)| TranscriptEvent {
                raw: None,
                ts: format!("2026-08-01T10:00:0{index}+08:00"),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: TranscriptKind::Narration,
                text: (*text).to_owned(),
                state: None,
                gm_only: false,
            })
            .collect();
        for event in &events {
            append_transcript(root.path(), &world_id, 0, event).unwrap();
        }

        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        let expected: Vec<_> = events[..2]
            .iter()
            .cloned()
            .map(|mut event| {
                event.state = Some(TableState::default());
                event
            })
            .collect();
        assert_eq!(
            read_transcript(root.path(), &world_id, 0).unwrap(),
            expected
        );
        // 重寫後仍是合法 JSONL：行數對齊事件數，沒有殘留的半行
        let path = root
            .path()
            .join(format!("worlds/{world_id}/transcript/0.jsonl"));
        assert_eq!(fs::read_to_string(&path).unwrap().lines().count(), 2);

        // 連按到底：收乾淨後再按回 false，不會倒退咬到別的幕
        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert!(!pop_transcript(root.path(), &world_id, 0).unwrap());
        assert!(read_transcript(root.path(), &world_id, 0)
            .unwrap()
            .is_empty());

        // 沒開始過的幕：不建檔也不報錯
        assert!(!pop_transcript(root.path(), &world_id, 9).unwrap());
        assert!(!root
            .path()
            .join(format!("worlds/{world_id}/transcript/9.jsonl"))
            .exists());
    }

    #[test]
    fn append_transcript_uses_current_snapshot_without_overwriting_supplied_state() {
        let root = TestRoot::new("transcript-state-snapshot");
        let world_id = create_world(root.path(), "狀態桌").unwrap();
        let mut state = read_state(root.path(), &world_id).unwrap();
        state
            .state
            .table
            .insert("time".to_owned(), "清晨".to_owned());
        write_state(root.path(), &world_id, &state).unwrap();
        let event = TranscriptEvent {
            raw: None,
            ts: "now".to_owned(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: TranscriptKind::Narration,
            text: "第一句".to_owned(),
            state: None,
            gm_only: false,
        };
        append_transcript(root.path(), &world_id, 0, &event).unwrap();
        assert_eq!(
            read_transcript(root.path(), &world_id, 0).unwrap()[0]
                .state
                .as_ref()
                .unwrap()
                .table
                .get("time"),
            Some(&"清晨".to_owned())
        );

        let supplied = TableState {
            table: BTreeMap::from([("time".to_owned(), "午夜".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                raw: None,
                ts: "later".to_owned(),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: TranscriptKind::Narration,
                text: "第二句".to_owned(),
                state: Some(supplied.clone()),
                gm_only: false,
            },
        )
        .unwrap();
        assert_eq!(
            read_transcript(root.path(), &world_id, 0).unwrap()[1].state,
            Some(supplied)
        );
    }

    #[test]
    fn append_opening_skips_raw_when_nothing_was_stripped() {
        let root = TestRoot::new("opening-raw");
        let world_id = create_world(root.path(), "純正文桌").unwrap();
        let raw = "只有旁白，沒有狀態欄。";
        let (event, _) = append_opening(
            root.path(),
            &world_id,
            0,
            "opening",
            raw,
            &crate::transport::extract_state_block(raw),
            "阿濤",
        )
        .unwrap();
        assert_eq!(event.text, raw);
        assert_eq!(event.raw, None);
        // 舊檔沒有 raw 欄位也讀得起來，序列化時同樣不憑空多一欄
        let line = serde_json::to_string(&event).unwrap();
        assert!(!line.contains("\"raw\""));
    }

    #[test]
    fn append_opening_merges_state_and_pop_restores_previous_snapshot() {
        let root = TestRoot::new("opening-state");
        let world_id = create_world(root.path(), "開場狀態桌").unwrap();
        let previous = TableState {
            table: BTreeMap::from([("place".to_owned(), "酒館".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                raw: None,
                ts: "before".to_owned(),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: TranscriptKind::Narration,
                text: "前一則".to_owned(),
                state: Some(previous.clone()),
                gm_only: false,
            },
        )
        .unwrap();

        let raw = "開場旁白<status>place: 碼頭\ntime: 午夜</status>";
        let (event, outcome) = append_opening(
            root.path(),
            &world_id,
            0,
            "opening",
            raw,
            &crate::transport::extract_state_block(raw),
            "阿濤",
        )
        .unwrap();
        assert!(outcome.records.is_empty());
        // 畫面只留正文，模型原文整段另存一份（面板要靠它重畫歷史訊息）
        assert_eq!(event.text, "開場旁白");
        assert_eq!(event.raw.as_deref(), Some(raw));
        let expected = TableState {
            table: BTreeMap::from([
                ("place".to_owned(), "碼頭".to_owned()),
                ("time".to_owned(), "午夜".to_owned()),
            ]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        assert_eq!(event.state, Some(expected.clone()));
        assert_eq!(read_state(root.path(), &world_id).unwrap().state, expected);
        assert_eq!(
            read_transcript(root.path(), &world_id, 0).unwrap()[1],
            event
        );

        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert_eq!(read_state(root.path(), &world_id).unwrap().state, previous);
    }

    #[test]
    fn pop_transcript_restores_the_previous_event_snapshot() {
        let root = TestRoot::new("transcript-state-pop");
        let world_id = create_world(root.path(), "回收狀態桌").unwrap();
        let first = TableState {
            table: BTreeMap::from([("place".to_owned(), "酒館".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        let second = TableState {
            table: BTreeMap::from([("place".to_owned(), "碼頭".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        for (text, snapshot) in [("第一句", first.clone()), ("第二句", second.clone())] {
            append_transcript(
                root.path(),
                &world_id,
                0,
                &TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: TranscriptKind::Narration,
                    text: text.to_owned(),
                    state: Some(snapshot),
                    gm_only: false,
                },
            )
            .unwrap();
        }
        let mut state = read_state(root.path(), &world_id).unwrap();
        state.state = second;
        write_state(root.path(), &world_id, &state).unwrap();

        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert_eq!(read_state(root.path(), &world_id).unwrap().state, first);
    }

    /// 復原＝把帶著自身快照的舊事件原樣寫回，目前值要跟著回到那一刻
    /// （否則狀態欄會停在收回後的舊值，跟桌上最後一句對不起來）
    #[test]
    fn restoring_an_undone_event_puts_its_snapshot_back() {
        let root = TestRoot::new("transcript-state-restore");
        let world_id = create_world(root.path(), "復原狀態桌").unwrap();
        let snapshots = ["清晨", "午夜"].map(|time| TableState {
            table: BTreeMap::from([("time".to_owned(), time.to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        });
        let event = |text: &str, snapshot: &TableState| TranscriptEvent {
            raw: None,
            ts: "now".to_owned(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: TranscriptKind::Narration,
            text: text.to_owned(),
            state: Some(snapshot.clone()),
            gm_only: false,
        };
        for (text, snapshot) in [("第一句", &snapshots[0]), ("第二句", &snapshots[1])] {
            append_transcript(root.path(), &world_id, 0, &event(text, snapshot)).unwrap();
        }
        assert_eq!(
            read_state(root.path(), &world_id).unwrap().state,
            snapshots[1]
        );

        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert_eq!(
            read_state(root.path(), &world_id).unwrap().state,
            snapshots[0]
        );

        append_transcript(root.path(), &world_id, 0, &event("第二句", &snapshots[1])).unwrap();
        assert_eq!(
            read_state(root.path(), &world_id).unwrap().state,
            snapshots[1]
        );
    }

    #[test]
    fn exports_all_transcript_scenes_as_localized_markdown() {
        let root = TestRoot::new("transcript-export");
        let world_id = create_world(root.path(), "海風桌").unwrap();
        for (scene, event) in [
            (
                1,
                TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: "船長代碼".to_owned(),
                    speaker_name: "船長".to_owned(),
                    kind: TranscriptKind::Dialogue,
                    text: "我們啟航。".to_owned(),
                    state: None,
                    gm_only: false,
                },
            ),
            (
                0,
                TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: TranscriptKind::Narration,
                    text: "霧氣升起。\n港口安靜。".to_owned(),
                    state: None,
                    gm_only: false,
                },
            ),
            (
                1,
                TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "玩家".to_owned(),
                    kind: TranscriptKind::Player,
                    text: "我登上甲板。".to_owned(),
                    state: None,
                    gm_only: false,
                },
            ),
            (
                0,
                TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: TranscriptKind::System,
                    text: "第一幕開始".to_owned(),
                    state: None,
                    gm_only: false,
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
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: TranscriptKind::Narration,
                    text: "霧氣升起。".to_owned(),
                    state: None,
                    gm_only: false,
                },
            ),
            (
                1,
                TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: "船長代碼".to_owned(),
                    speaker_name: "船長".to_owned(),
                    kind: TranscriptKind::Dialogue,
                    text: "我們啟航。".to_owned(),
                    state: None,
                    gm_only: false,
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
            raw: None,
            ts: "2026-07-19T00:00:00Z".to_owned(),
            speaker_id: String::new(),
            speaker_name: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "第一場的對話".to_owned(),
            state: None,
            gm_only: false,
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
            raw: None,
            ts: "2026-07-24T00:00:00Z".to_owned(),
            speaker_id: String::new(),
            speaker_name: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "第一幕的對話".to_owned(),
            state: None,
            gm_only: false,
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
    fn revert_scene_returns_to_previous_scene_and_drops_title() {
        let root = TestRoot::new("revert-scene");
        let world_id = create_world(root.path(), "退幕桌").unwrap();
        let snapshot = TableState {
            table: BTreeMap::from([("place".to_owned(), "酒館".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                raw: None,
                ts: "2026-08-06T00:00:00Z".to_owned(),
                speaker_id: String::new(),
                speaker_name: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "第一幕的對話".to_owned(),
                state: Some(snapshot.clone()),
                gm_only: false,
            },
        )
        .unwrap();
        begin_next_scene(root.path(), &world_id, "摘要", "zh-TW", Some("酒館夜話")).unwrap();
        assert_eq!(read_state(root.path(), &world_id).unwrap().current_scene, 1);

        let previous = revert_scene(root.path(), &world_id).unwrap();
        assert_eq!(previous, 0);

        let state = read_state(root.path(), &world_id).unwrap();
        assert_eq!(state.current_scene, 0);
        // 前幕最後一則帶快照事件的 state 要跟著回來，不是砍完就放著預設值
        assert_eq!(state.state, snapshot);
        assert!(!state.scene_titles.contains_key("0"));
        assert!(!root
            .path()
            .join(format!("worlds/{world_id}/transcript/1.jsonl"))
            .exists());
        // 舊幕本身完全沒被動過
        assert_eq!(read_transcript(root.path(), &world_id, 0).unwrap().len(), 1);
    }

    #[test]
    fn revert_scene_rejects_extra_events_without_touching_anything() {
        let root = TestRoot::new("revert-scene-blocked");
        let world_id = create_world(root.path(), "退幕擋桌").unwrap();
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                raw: None,
                ts: "2026-08-06T00:00:00Z".to_owned(),
                speaker_id: String::new(),
                speaker_name: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "序幕".to_owned(),
                state: None,
                gm_only: false,
            },
        )
        .unwrap();
        begin_next_scene(root.path(), &world_id, "摘要", "zh-TW", None).unwrap();
        // 這一幕除了摘要之外，玩家已經多說了一句——不是「剛好一則」了
        append_transcript(
            root.path(),
            &world_id,
            1,
            &TranscriptEvent {
                raw: None,
                ts: "2026-08-06T00:01:00Z".to_owned(),
                speaker_id: String::new(),
                speaker_name: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "新的一句".to_owned(),
                state: None,
                gm_only: false,
            },
        )
        .unwrap();

        let before_state = read_state(root.path(), &world_id).unwrap();
        let before_events = read_transcript(root.path(), &world_id, 1).unwrap();

        let error = revert_scene(root.path(), &world_id).unwrap_err().to_string();
        assert!(error.contains("不能退回前幕"));

        // 擋下時檔案與 state 都沒被動過
        assert_eq!(read_state(root.path(), &world_id).unwrap(), before_state);
        assert_eq!(
            read_transcript(root.path(), &world_id, 1).unwrap(),
            before_events
        );
    }

    #[test]
    fn revert_scene_rejects_at_first_scene() {
        let root = TestRoot::new("revert-scene-first");
        let world_id = create_world(root.path(), "第一幕桌").unwrap();
        let error = revert_scene(root.path(), &world_id).unwrap_err().to_string();
        assert!(error.contains("沒有前幕可以退回"));
    }

    #[test]
    fn replace_scene_summary_overwrites_text_and_drops_title_when_none() {
        let root = TestRoot::new("replace-scene-summary");
        let world_id = create_world(root.path(), "重寫摘要桌").unwrap();
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                raw: None,
                ts: "2026-08-06T00:00:00Z".to_owned(),
                speaker_id: String::new(),
                speaker_name: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "序幕".to_owned(),
                state: None,
                gm_only: false,
            },
        )
        .unwrap();
        begin_next_scene(root.path(), &world_id, "舊摘要", "zh-TW", Some("舊標題")).unwrap();
        assert_eq!(
            read_state(root.path(), &world_id)
                .unwrap()
                .scene_titles
                .get("0")
                .map(String::as_str),
            Some("舊標題")
        );

        replace_scene_summary(root.path(), &world_id, "新摘要", "zh-TW", None).unwrap();

        let events = read_transcript(root.path(), &world_id, 1).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].text, "【前情提要】\n新摘要");
        assert_eq!(events[0].speaker_name, "GM");
        assert_eq!(events[0].kind, TranscriptKind::Narration);

        // title 傳 None：舊幕名被移除，不留上一次的殘留
        assert!(!read_state(root.path(), &world_id)
            .unwrap()
            .scene_titles
            .contains_key("0"));
    }

    /// 分岔幕開頭那則是複製來的真實對話，不是前情提要。源頭幕剛好只有一則時，
    /// 「這幕只有一則」那道守門會放行——沒有 forked 這一格擋著，重寫就把玩家的對話覆寫掉了。
    #[test]
    fn replace_scene_summary_refuses_a_forked_scene() {
        let root = TestRoot::new("replace-summary-forked");
        let world_id = create_world(root.path(), "分岔守門桌").unwrap();
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                raw: None,
                ts: "now".to_owned(),
                speaker_id: String::new(),
                speaker_name: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "玩家的第一句".to_owned(),
                state: None,
                gm_only: false,
            },
        )
        .unwrap();
        begin_next_scene(root.path(), &world_id, "第一幕摘要", "zh-TW", None).unwrap();

        // 幕 0 只有一則，分岔出來的這一幕同樣只有那一則——正是守門會誤放的形狀
        let forked = fork_scene(root.path(), &world_id, 0).unwrap();
        assert_eq!(
            read_transcript(root.path(), &world_id, forked).unwrap().len(),
            1
        );

        assert!(
            replace_scene_summary(root.path(), &world_id, "不該蓋掉", "zh-TW", None).is_err()
        );
        let events = read_transcript(root.path(), &world_id, forked).unwrap();
        assert_eq!(events[0].text, "玩家的第一句");
    }

    /// 重寫摘要只換文字：那則的狀態快照要留著。摘要是這一幕唯一一則，
    /// 快照掉了的話，之後退回這一幕就只能把狀態欄清成空的。
    #[test]
    fn replace_scene_summary_keeps_snapshot_for_later_revert() {
        let root = TestRoot::new("replace-scene-summary-snapshot");
        let world_id = create_world(root.path(), "快照保留桌").unwrap();
        let snapshot = TableState {
            table: BTreeMap::from([("place".to_owned(), "酒館".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                raw: None,
                ts: "now".to_owned(),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: TranscriptKind::Narration,
                text: "序幕".to_owned(),
                state: Some(snapshot.clone()),
                gm_only: false,
            },
        )
        .unwrap();
        let mut state = read_state(root.path(), &world_id).unwrap();
        state.state = snapshot.clone();
        write_state(root.path(), &world_id, &state).unwrap();

        begin_next_scene(root.path(), &world_id, "舊摘要", "zh-TW", None).unwrap();
        replace_scene_summary(root.path(), &world_id, "新摘要", "zh-TW", None).unwrap();
        // 再換一幕：這時第 1 幕那則摘要成了回推狀態的唯一來源
        begin_next_scene(root.path(), &world_id, "第二幕摘要", "zh-TW", None).unwrap();

        assert_eq!(revert_scene(root.path(), &world_id).unwrap(), 1);
        assert_eq!(read_state(root.path(), &world_id).unwrap().state, snapshot);
    }

    /// 驗收劇本：原線三幕分岔、續玩換幕、退回吃 parent、再分岔看 version 疊加。
    #[test]
    fn fork_scene_copies_history_and_relabels_through_continue_and_revert() {
        let root = TestRoot::new("fork-scene-scenario");
        let world_id = create_world(root.path(), "分岔桌").unwrap();

        // 原線三幕（內部 0、1、2），人在幕 2
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                raw: None,
                ts: "2026-08-06T00:00:00Z".to_owned(),
                speaker_id: "船長代碼".to_owned(),
                speaker_name: "船長".to_owned(),
                kind: TranscriptKind::Dialogue,
                text: "啟航前的最後一夜。".to_owned(),
                state: None,
                gm_only: false,
            },
        )
        .unwrap();
        begin_next_scene(root.path(), &world_id, "第一幕摘要", "zh-TW", None).unwrap();
        begin_next_scene(root.path(), &world_id, "第二幕摘要", "zh-TW", None).unwrap();
        assert_eq!(read_state(root.path(), &world_id).unwrap().current_scene, 2);

        let scene0_before = read_transcript(root.path(), &world_id, 0).unwrap();
        let scene1_before = read_transcript(root.path(), &world_id, 1).unwrap();
        let scene2_before = read_transcript(root.path(), &world_id, 2).unwrap();

        // 從幕 0 分岔
        let forked = fork_scene(root.path(), &world_id, 0).unwrap();
        assert_eq!(forked, 3);
        assert_eq!(
            read_transcript(root.path(), &world_id, 3).unwrap(),
            scene0_before
        );
        // 舊幕一個字都沒被動過
        assert_eq!(
            read_transcript(root.path(), &world_id, 0).unwrap(),
            scene0_before
        );
        assert_eq!(
            read_transcript(root.path(), &world_id, 1).unwrap(),
            scene1_before
        );
        assert_eq!(
            read_transcript(root.path(), &world_id, 2).unwrap(),
            scene2_before
        );

        let state = read_state(root.path(), &world_id).unwrap();
        assert_eq!(state.current_scene, 3);
        assert_eq!(
            state.scene_labels.get("3").copied(),
            Some(SceneLabel {
                base: 0,
                version: 2,
                parent: Some(2),
                forked: true
            })
        );

        // 在幕 3 續玩一句，再換幕
        append_transcript(
            root.path(),
            &world_id,
            3,
            &TranscriptEvent {
                raw: None,
                ts: "2026-08-06T00:01:00Z".to_owned(),
                speaker_id: "船長代碼".to_owned(),
                speaker_name: "船長".to_owned(),
                kind: TranscriptKind::Dialogue,
                text: "這次我們往南走。".to_owned(),
                state: None,
                gm_only: false,
            },
        )
        .unwrap();
        let advanced =
            begin_next_scene(root.path(), &world_id, "分岔後摘要", "zh-TW", Some("南航夜話"))
                .unwrap();
        assert_eq!(advanced, 4);

        let state = read_state(root.path(), &world_id).unwrap();
        assert_eq!(
            state.scene_labels.get("4").copied(),
            Some(SceneLabel {
                base: 1,
                version: 2,
                parent: Some(3),
                forked: false
            })
        );
        assert_eq!(
            state.scene_titles.get("3").map(String::as_str),
            Some("南航夜話")
        );

        // 退回幕 4：回到 parent（3），不是算術上的 4-1=3 巧合——這裡故意驗證的是「回到分岔前所在幕」
        let reverted = revert_scene(root.path(), &world_id).unwrap();
        assert_eq!(reverted, 3);
        assert!(!root
            .path()
            .join(format!("worlds/{world_id}/transcript/4.jsonl"))
            .exists());
        let state = read_state(root.path(), &world_id).unwrap();
        assert_eq!(state.current_scene, 3);
        assert!(!state.scene_titles.contains_key("3"));
        assert!(!state.scene_labels.contains_key("4"));

        // 再從幕 0 分岔一次：幕 0 與幕 3 都是 base 0，這次該排第 3 個版本
        let forked_again = fork_scene(root.path(), &world_id, 0).unwrap();
        assert_eq!(forked_again, 4);
        let state = read_state(root.path(), &world_id).unwrap();
        assert_eq!(
            state.scene_labels.get("4").copied(),
            Some(SceneLabel {
                base: 0,
                version: 3,
                parent: Some(3),
                forked: true
            })
        );
    }

    #[test]
    fn fork_scene_rejects_current_or_future_scene() {
        let root = TestRoot::new("fork-scene-rejects-current");
        let world_id = create_world(root.path(), "分岔擋桌").unwrap();
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                raw: None,
                ts: "2026-08-06T00:00:00Z".to_owned(),
                speaker_id: String::new(),
                speaker_name: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "序幕".to_owned(),
                state: None,
                gm_only: false,
            },
        )
        .unwrap();

        // from_scene == current_scene：還沒換幕，不能從自己這幕分岔
        let error = fork_scene(root.path(), &world_id, 0).unwrap_err().to_string();
        assert!(error.contains("只能從前面的幕分岔"));

        // from_scene > current_scene：幕號還沒出現過
        let error = fork_scene(root.path(), &world_id, 5).unwrap_err().to_string();
        assert!(error.contains("只能從前面的幕分岔"));
    }

    #[test]
    fn fork_scene_rejects_a_scene_with_no_events() {
        let root = TestRoot::new("fork-scene-rejects-empty");
        let world_id = create_world(root.path(), "分岔空幕桌").unwrap();
        let mut state = read_state(root.path(), &world_id).unwrap();
        state.current_scene = 1; // 幕 0 從沒寫過任何事件，模擬空幕
        write_state(root.path(), &world_id, &state).unwrap();

        let error = fork_scene(root.path(), &world_id, 0).unwrap_err().to_string();
        assert!(error.contains("這一幕沒有紀錄可以接續"));
    }

    #[test]
    fn scene_label_falls_back_to_original_line_for_unlabeled_scene() {
        let root = TestRoot::new("scene-label-fallback");
        let world_id = create_world(root.path(), "原線桌").unwrap();
        let state = read_state(root.path(), &world_id).unwrap();

        assert_eq!(
            scene_label(&state, 5),
            SceneLabel {
                base: 5,
                version: 1,
                parent: Some(4),
                forked: false
            }
        );
        // 幕 0 沒有前幕：fallback 的 parent 也要是 None，跟 revert_scene 的邊界檢查對得起來
        assert_eq!(
            scene_label(&state, 0),
            SceneLabel {
                base: 0,
                version: 1,
                parent: None,
                forked: false
            }
        );
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
    fn mechanism_round_trips_and_old_state_defaults_to_empty() {
        let root = TestRoot::new("mechanism-state");
        let world_id = create_world(root.path(), "機制桌").unwrap();
        let mut state = read_state(root.path(), &world_id).unwrap();
        let mut rule = FieldRule::for_kind(FieldKind::Pair);
        rule.min = Some(0.0);
        rule.max = Some(500.0);
        rule.branch = Some("亞瑟".to_owned());
        state.mechanism.rules.insert("亞瑟.HP".to_owned(), rule);
        write_state(root.path(), &world_id, &state).unwrap();
        assert_eq!(read_state(root.path(), &world_id).unwrap(), state);

        fs::write(
            root.path().join(format!("worlds/{world_id}/state.json")),
            r#"{"id":"old","name":"舊桌"}"#,
        )
        .unwrap();
        assert!(read_state(root.path(), &world_id)
            .unwrap()
            .mechanism
            .is_empty());
    }

    #[test]
    fn set_tree_value_creates_overwrites_prunes_and_preserves_leaf_collisions() {
        let path = ["World", "城市", "聲望"].map(str::to_owned);
        let mut tree = BTreeMap::new();
        assert!(!set_tree_value(&mut tree, &path, ""));
        assert!(tree.is_empty());
        assert!(set_tree_value(&mut tree, &path, "10"));
        assert!(set_tree_value(&mut tree, &path, "12"));
        assert_eq!(
            tree["World"],
            StateNode::Branch(BTreeMap::from([(
                "城市".to_owned(),
                StateNode::Branch(BTreeMap::from([(
                    "聲望".to_owned(),
                    StateNode::Leaf("12".to_owned()),
                )])),
            )]))
        );
        assert!(set_tree_value(&mut tree, &path, ""));
        assert!(tree.is_empty());

        tree.insert("World".to_owned(), StateNode::Leaf("不可展開".to_owned()));
        let before = tree.clone();
        assert!(!set_tree_value(&mut tree, &path, "13"));
        assert_eq!(tree, before);
    }

    #[test]
    fn pop_transcript_restores_entire_nested_tree_snapshot() {
        let root = TestRoot::new("nested-state-pop");
        let world_id = create_world(root.path(), "巢狀桌").unwrap();
        let first = TableState {
            table: BTreeMap::new(),
            tree: BTreeMap::from([(
                "World".to_owned(),
                StateNode::Branch(BTreeMap::from([(
                    "城市".to_owned(),
                    StateNode::Branch(BTreeMap::from([(
                        "聲望".to_owned(),
                        StateNode::Leaf("10".to_owned()),
                    )])),
                )])),
            )]),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        let second = TableState {
            table: BTreeMap::new(),
            tree: BTreeMap::from([(
                "World".to_owned(),
                StateNode::Branch(BTreeMap::from([(
                    "城市".to_owned(),
                    StateNode::Branch(BTreeMap::from([(
                        "聲望".to_owned(),
                        StateNode::Leaf("20".to_owned()),
                    )])),
                )])),
            )]),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        for snapshot in [first.clone(), second.clone()] {
            append_transcript(
                root.path(),
                &world_id,
                0,
                &TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: TranscriptKind::Narration,
                    text: "旁白".to_owned(),
                    state: Some(snapshot),
                    gm_only: false,
                },
            )
            .unwrap();
        }
        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert_eq!(read_state(root.path(), &world_id).unwrap().state, first);
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

    /// 舊存檔沒有 aligned_scene／branch_bindings（WorldState）與 changes（TableState）
    /// 三個新欄位也要讀得起來，且各自落回預設值（狀態欄二期包 5）。
    #[test]
    fn old_state_json_without_pack5_fields_still_deserializes() {
        let json = r#"{
            "id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "name": "舊桌",
            "state": { "table": { "time": "清晨" } }
        }"#;
        let state: WorldState = serde_json::from_str(json).unwrap();
        assert_eq!(state.aligned_scene, None);
        assert!(state.branch_bindings.is_empty());
        assert!(state.state.changes.is_empty());
        assert_eq!(state.state.table.get("time"), Some(&"清晨".to_owned()));
    }

    /// 舊角色卡 meta 沒有 auto_hidden（AI 卡重構包 4b 新欄位，封存三態的其中一態）
    /// 也要讀得起來，落回預設值 false。
    #[test]
    fn old_character_meta_json_without_auto_hidden_still_deserializes() {
        let json = r##"{
            "id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "name": "阿藍",
            "color": "#3366ff",
            "avatar": "avatars/blue.png",
            "tier": "balanced"
        }"##;
        let meta: CharacterMeta = serde_json::from_str(json).unwrap();
        assert!(!meta.auto_hidden);
        assert!(!meta.archived);
    }

    /// AI 卡重構包 4b：write_character（前端編輯表單走的路徑，CharacterCard 本身不帶
    /// auto_hidden）改其他欄位時，延續磁碟上原有的 auto_hidden，不會被編輯表單悄悄清掉。
    #[test]
    fn write_character_preserves_auto_hidden_across_unrelated_edit() {
        let root = TestRoot::new("preserve-auto-hidden");
        let world_id = create_world(root.path(), "測試桌").unwrap();
        let mut card = character_card(&new_id(), "狐狸");
        write_character(root.path(), &world_id, &card).unwrap();
        set_character_auto_hidden(root.path(), &world_id, &card.id, true).unwrap();

        card.color = "#ff0000".to_owned();
        write_character(root.path(), &world_id, &card).unwrap();

        let meta = list_characters(root.path(), &world_id)
            .unwrap()
            .into_iter()
            .find(|meta| meta.id == card.id)
            .unwrap();
        assert!(meta.auto_hidden);
        assert_eq!(meta.color, "#ff0000");
    }

    /// AI 卡重構包 4b：換幕結算角色卡自動隱藏。出現過＝本幕有回歸事件 (a) 或換幕當下
    /// present 名單命中 (b)；兩者都沒有（就算幕開始時本來在主區）結算成隱藏；
    /// archived 的卡完全不受結算影響。
    #[test]
    fn begin_next_scene_settles_card_auto_hidden() {
        let root = TestRoot::new("card-settlement");
        let world_id = create_world(root.path(), "測試桌").unwrap();

        let fox = character_card(&new_id(), "狐狸"); // (a) 本幕有回歸事件
        let bear = character_card(&new_id(), "熊"); // (b) present 命中
        let badger = character_card(&new_id(), "獾"); // 兩者都沒有 → 結算成隱藏
        let ghost = character_card(&new_id(), "亡靈"); // archived → 完全不動
        for card in [&fox, &bear, &badger, &ghost] {
            write_character(root.path(), &world_id, card).unwrap();
        }
        set_character_auto_hidden(root.path(), &world_id, &fox.id, true).unwrap();
        set_character_auto_hidden(root.path(), &world_id, &bear.id, true).unwrap();
        set_character_auto_hidden(root.path(), &world_id, &badger.id, false).unwrap();
        set_character_auto_hidden(root.path(), &world_id, &ghost.id, false).unwrap();
        set_character_archived(root.path(), &world_id, &ghost.id, true).unwrap();

        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                ts: "now".to_owned(),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: TranscriptKind::System,
                text: "（角色回歸）〈狐狸〉\n尾巴很大。".to_owned(),
                raw: None,
                state: None,
                gm_only: false,
            },
        )
        .unwrap();

        let mut state = read_state(root.path(), &world_id).unwrap();
        state
            .state
            .table
            .insert("present".to_owned(), "狐狸、熊".to_owned());
        write_state(root.path(), &world_id, &state).unwrap();

        begin_next_scene(root.path(), &world_id, "摘要", "zh-TW", None).unwrap();

        let metas = list_characters(root.path(), &world_id).unwrap();
        let auto_hidden_of = |id: &str| metas.iter().find(|meta| meta.id == id).unwrap().auto_hidden;
        assert!(!auto_hidden_of(&fox.id), "本幕有回歸事件的卡應該結算成主區");
        assert!(!auto_hidden_of(&bear.id), "present 命中的卡應該結算成主區");
        assert!(
            auto_hidden_of(&badger.id),
            "沒出現過的卡（就算原本在主區）應該結算成隱藏"
        );
        assert!(!auto_hidden_of(&ghost.id), "archived 的卡完全不受結算影響");
    }
}
