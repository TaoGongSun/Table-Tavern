//! 世界書觸發狀態的存放與落地（worldbook-st-trigger-parity 三之 4，三之 8 的計時部分）。
//! `worlds/<id>/world-info/<幕號>.json`：每幕一份，分視角（GM 一份、每位角色各一份）存 sticky／cooldown，
//! 另記一筆還沒結的落地 `pending`。讀寫一律在同檔鎖內、寫入走原子替換；讀不了或解析失敗回錯，不當空表覆寫。
//! 實送的結算與落地由 `crate::world_scan::landing` 呼叫，待回報檔由 `commands::scene` 的指令讀與確認。

mod landing;
mod notices;
mod scenes;

pub use landing::{
    begin_landing, fail_turn, mark_sent, push_var_intent, reply_landed, settle_pending,
    update_var_intent, Report, VarConflict,
};
pub use notices::{ack_notice, read_notices, Notice};
pub use scenes::{carry_into_next_scene, copy_for_fork, drop_scene, import_web_timed, WebTimed};

use super::card_vars::Layer;
use super::paths::world_info_dir;
use super::world_file::{commit_world_write_atomic, with_file_lock, LockedFile};
use super::{invalid_data, is_false, DataResult, EventMarker, TranscriptEvent, TranscriptKind};
use crate::world_info::timed::{TimedEffect, WiTimed};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// 計時檔的格式版本；讀到別的版本回錯。
pub const FORMAT_VERSION: u32 = 1;

/// 掃描視角（P1）：GM 一份、每位角色各一份。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Perspective {
    Gm,
    Character(String),
}

impl Perspective {
    /// 計時檔 `perspectives` 的鍵：`gm`、`char:<角色 id>`。
    pub fn key(&self) -> String {
        match self {
            Perspective::Gm => "gm".to_owned(),
            Perspective::Character(id) => format!("char:{id}"),
        }
    }
}

/// 一筆計時（單位：訊息則數）。start／end 存有號整數：換幕平移後可以是負的。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredEffect {
    pub start: i64,
    pub end: i64,
    pub protected: bool,
    /// 這條 sticky 被記下時是私密觸發（三之 3）
    #[serde(default, skip_serializing_if = "is_false")]
    pub confidential: bool,
}

/// 一個視角的計時表，形狀同網頁存檔契約的 `timed`，鍵是 uid 字串。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredTimed {
    #[serde(default)]
    pub sticky: BTreeMap<String, StoredEffect>,
    #[serde(default)]
    pub cooldown: BTreeMap<String, StoredEffect>,
}

/// 掃描的 f64 轉成存檔整數：start 一定是則數（整數）；end＝start＋條目設定值，設定值可以是小數，
/// 而掃描只拿整數則數做 `則數 >= end` 比較，所以往上取整等價；超出範圍的飽和。
fn stored_bound(value: f64) -> i64 {
    value.ceil() as i64
}

impl StoredTimed {
    pub fn from_scan(timed: &WiTimed) -> Self {
        let convert = |table: &BTreeMap<String, TimedEffect>| {
            table
                .iter()
                .map(|(id, effect)| {
                    (
                        id.clone(),
                        StoredEffect {
                            start: stored_bound(effect.start),
                            end: stored_bound(effect.end),
                            protected: effect.protected,
                            confidential: effect.confidential,
                        },
                    )
                })
                .collect()
        };
        Self {
            sticky: convert(&timed.sticky),
            cooldown: convert(&timed.cooldown),
        }
    }

    pub fn to_scan(&self) -> WiTimed {
        let convert = |table: &BTreeMap<String, StoredEffect>| {
            table
                .iter()
                .map(|(id, effect)| {
                    (
                        id.clone(),
                        TimedEffect {
                            start: effect.start as f64,
                            end: effect.end as f64,
                            protected: effect.protected,
                            confidential: effect.confidential,
                        },
                    )
                })
                .collect()
        };
        WiTimed {
            sticky: convert(&self.sticky),
            cooldown: convert(&self.cooldown),
        }
    }

    /// 拿掉不在 `live` 裡的 uid；有拿掉回 true。
    fn retain_uids(&mut self, live: &BTreeSet<String>) -> bool {
        let before = self.sticky.len() + self.cooldown.len();
        self.sticky.retain(|uid, _| live.contains(uid));
        self.cooldown.retain(|uid, _| live.contains(uid));
        before != self.sticky.len() + self.cooldown.len()
    }

    /// start、end 一起加 `by`（換幕平移傳負數）。
    fn shift(&mut self, by: i64) {
        for effect in self.sticky.values_mut().chain(self.cooldown.values_mut()) {
            effect.start = effect.start.saturating_add(by);
            effect.end = effect.end.saturating_add(by);
        }
    }
}

/// 落地進行到哪一步（三之 8）：寫入中＝計時與變數還在寫、還沒交給傳輸層；已送出＝請求已交出去。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Writing,
    Sent,
}

/// 變數層寫入意圖（三之 8）：先記意圖再寫層、寫成再補 `after_rev`。結算只看 rev 與寫入前內容。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VarIntent {
    pub layer: Layer,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// 寫入前的 rev；`None`＝寫入前沒有檔
    pub expected_rev: Option<String>,
    /// 寫入前內容（變數表 JSON 原文）；`expected_rev` 是 `None` 時不用（撤回就刪檔）
    pub before: String,
    /// 操作序列（包 5b 定格式；結算不讀、只為留證）
    #[serde(default)]
    pub ops: serde_json::Value,
    /// 寫成後的 rev；`None`＝還不知道寫成沒有
    #[serde(default)]
    pub after_rev: Option<String>,
    /// 結算撤回時用的新 rev：撤回前先落檔，崩潰重跑時認得出已撤回過
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restore_rev: Option<String>,
}

/// 一次實送落地還沒結的紀錄。`before` 是落地前該視角的表，失敗時整張寫回。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pending {
    pub turn_key: String,
    pub perspective: String,
    pub stage: Stage,
    pub before: StoredTimed,
    #[serde(default)]
    pub vars: Vec<VarIntent>,
}

/// 一幕的計時檔。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneTimed {
    pub version: u32,
    #[serde(default)]
    pub perspectives: BTreeMap<String, StoredTimed>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<Pending>,
}

impl Default for SceneTimed {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            perspectives: BTreeMap::new(),
            pending: None,
        }
    }
}

impl SceneTimed {
    /// 各視角與 pending 前像一起拿掉不在 `live` 裡的 uid；有拿掉回 true。
    fn retain_uids(&mut self, live: &BTreeSet<String>) -> bool {
        let mut changed = false;
        for table in self.perspectives.values_mut() {
            changed |= table.retain_uids(live);
        }
        if let Some(pending) = self.pending.as_mut() {
            changed |= pending.before.retain_uids(live);
        }
        changed
    }
}

/// 則數規則：本幕事件扣掉 `System` 類與換幕摘要（與掃描清單同一個排除規則，渲染之前判）。
pub fn counts_toward_timing(event: &TranscriptEvent) -> bool {
    event.kind != TranscriptKind::System && event.marker != Some(EventMarker::SceneSummary)
}

/// 一幕的則數。
pub fn chat_length(events: &[TranscriptEvent]) -> usize {
    events
        .iter()
        .filter(|event| counts_toward_timing(event))
        .count()
}

fn scene_path(root: &Path, world_id: &str, scene: u64) -> DataResult<PathBuf> {
    Ok(world_info_dir(root, world_id)?.join(format!("{scene}.json")))
}

fn parse_scene(bytes: &[u8], path: &Path) -> DataResult<SceneTimed> {
    let parsed: SceneTimed = serde_json::from_slice(bytes)
        .map_err(|error| invalid_data(format!("world-info: {}：{error}", path.display())))?;
    if parsed.version != FORMAT_VERSION {
        return Err(invalid_data(format!(
            "world-info: {} 版本 {} 不認得",
            path.display(),
            parsed.version
        )));
    }
    Ok(parsed)
}

/// 鎖內讀：沒有檔＝空表；讀不了、壞掉回錯。
fn read_locked(file: &LockedFile<'_>, path: &Path) -> DataResult<SceneTimed> {
    match file.read()? {
        Some(bytes) => parse_scene(&bytes, path),
        None => Ok(SceneTimed::default()),
    }
}

/// 讀一幕的計時檔原樣（不清 uid、不看 pending）。
pub fn read_scene(root: &Path, world_id: &str, scene: u64) -> DataResult<SceneTimed> {
    let path = scene_path(root, world_id, scene)?;
    with_file_lock(&path, |file| read_locked(file, &path))
}

fn write_scene(root: &Path, world_id: &str, scene: u64, timed: &SceneTimed) -> DataResult<()> {
    let path = scene_path(root, world_id, scene)?;
    commit_world_write_atomic(&path, &serde_json::to_vec_pretty(timed)?)
}

/// 同檔鎖內讀、改、原子寫回。`work` 回錯就不寫。
fn modify_scene<T>(
    root: &Path,
    world_id: &str,
    scene: u64,
    work: impl FnOnce(&mut SceneTimed) -> DataResult<T>,
) -> DataResult<T> {
    let path = scene_path(root, world_id, scene)?;
    with_file_lock(&path, |file| {
        let mut timed = read_locked(file, &path)?;
        let result = work(&mut timed)?;
        file.write_atomic(&serde_json::to_vec_pretty(&timed)?)?;
        Ok(result)
    })
}

/// 書裡現有條目的 uid 字串。
fn live_uids(root: &Path, world_id: &str) -> DataResult<BTreeSet<String>> {
    let book = super::worldbook::read_worldbook_value(root, world_id)?;
    Ok(super::worldbook::worldbook_uids(&book)
        .into_iter()
        .map(|uid| uid.to_string())
        .collect())
}

/// 掃描要用的計時表（唯讀，實送與量測共用）：書上已經沒有的 uid 只在記憶體裡清掉（刪條目後清計時沒做成
/// 的補清，不寫回）；還有未結的 `pending` 落在這個視角時，以落地前的表計算（量測與回合並行時才會看到，
/// 實送前已先結算）。
pub fn read_timed(
    root: &Path,
    world_id: &str,
    scene: u64,
    perspective: &Perspective,
) -> DataResult<WiTimed> {
    let mut timed = read_scene(root, world_id, scene)?;
    timed.retain_uids(&live_uids(root, world_id)?);
    let key = perspective.key();
    let table = match timed.pending {
        Some(pending) if pending.perspective == key => pending.before,
        _ => timed.perspectives.remove(&key).unwrap_or_default(),
    };
    Ok(table.to_scan())
}

/// 結算一直回錯時給玩家的出路（「重設世界書觸發紀錄」）：這一幕的計時檔、其他讀不懂或留著未結落地的幕檔與
/// 讀不懂的 `notices.json` 一律改名成 `<檔名>.broken-<時間>` 留備份（不刪、不蓋舊備份），之後當成沒有紀錄重新算。
/// 移開前，讀得懂的幕檔裡「寫入中」的落地先撤回能撤的變數意圖，撤不回或撤回出錯的寫進待回報檔（三之 8：不得整筆
/// 丟掉日誌）；已送出的變數副作用照規則保留。失去的是 sticky／冷卻的進度。回傳備份檔名。
pub fn reset_scene(root: &Path, world_id: &str, scene: u64) -> DataResult<Vec<String>> {
    let suffix = format!(
        "broken-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    );
    reset_scene_as(root, world_id, scene, &suffix)
}

/// `reset_scene` 本體；備份後綴由呼叫端給（測試用固定後綴驗撞名）。
pub(crate) fn reset_scene_as(
    root: &Path,
    world_id: &str,
    scene: u64,
    suffix: &str,
) -> DataResult<Vec<String>> {
    super::state_commit::with_commit(root, world_id, |_| {
        let dir = world_info_dir(root, world_id)?;
        let current = scene_path(root, world_id, scene)?;
        let mut scenes = vec![(scene, current.clone())];
        let mut notices_broken = None;
        if let Ok(read) = std::fs::read_dir(&dir) {
            for entry in read {
                let path = entry?.path();
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("");
                let Some(stem) = name.strip_suffix(".json") else {
                    continue;
                };
                if stem == "notices" {
                    if notices::check(root, world_id).is_err() {
                        notices_broken = Some(path);
                    }
                    continue;
                }
                // 其他幕檔：讀不懂、或留著沒結的落地（分岔時來源幕結算失敗會卡在這裡）都一起移開
                let Ok(other) = stem.parse::<u64>() else {
                    continue;
                };
                if path != current
                    && with_file_lock(&path, |file| read_locked(file, &path))
                        .map_or(true, |timed| timed.pending.is_some())
                {
                    scenes.push((other, path));
                }
            }
        }
        let mut moved = Vec::new();
        // 待回報檔先移開（之後的撤回回報寫進新檔）
        if let Some(path) = notices_broken {
            if let Some(name) = with_file_lock(&path, |file| file.move_aside(suffix))? {
                moved.push(name);
            }
        }
        for (number, path) in &scenes {
            if let Ok(Some(pending)) =
                with_file_lock(path, |file| read_locked(file, path)).map(|timed| timed.pending)
            {
                landing::release_before_reset(root, world_id, *number, &pending)?;
            }
        }
        for (_, path) in scenes {
            if let Some(name) = with_file_lock(&path, |file| file.move_aside(suffix))? {
                moved.push(name);
            }
        }
        Ok(moved)
    })
}

/// 各幕計時檔與 pending 前像裡，不在 `live` 的 uid 一律清掉（只有真的拿掉東西的檔才寫）。
/// 解析不了的檔不動（它本來就讀不了、也寫不進），讀檔的 IO 錯回錯。
pub(crate) fn prune_uids(root: &Path, world_id: &str, live: &BTreeSet<u64>) -> DataResult<()> {
    let dir = world_info_dir(root, world_id)?;
    let names = match std::fs::read_dir(&dir) {
        Ok(read) => read,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let live: BTreeSet<String> = live.iter().map(u64::to_string).collect();
    for entry in names {
        let path = entry?.path();
        let is_scene = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .is_some_and(|stem| stem.parse::<u64>().is_ok())
            && path.extension().is_some_and(|ext| ext == "json");
        if !is_scene {
            continue;
        }
        with_file_lock(&path, |file| -> DataResult<()> {
            let Some(bytes) = file.read()? else {
                return Ok(());
            };
            let Ok(mut timed) = parse_scene(&bytes, &path) else {
                return Ok(());
            };
            if timed.retain_uids(&live) {
                file.write_atomic(&serde_json::to_vec_pretty(&timed)?)?;
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
