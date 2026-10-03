//! 卡片變數的非 message 層（計畫 8.7，包 2b）：chat／character／global／preset／script／extension。
//! 每層一個檔 `{ id, rev, vars }`，`rev` 是版本 token：寫入帶目標 rev，不符回 stale 附權威值（跨桌改
//! global 會這樣）。六層寫入都在這桌的短提交鎖內核對桌世代，再取同檔鎖核對 rev；資料根目錄的層
//! （global、preset、extension）另查更新閘門。全部原子替換寫入。
use super::message_vars::{self, new_token, parse_table, VarsTable};
use super::paths::world_dir;
use super::state_commit::with_commit;
use super::world_file::{with_file_lock, with_root_file_lock, LockedFile};
use super::{invalid_data, DataResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// 原 ID 長度上限（字元）
pub const MAX_ID_CHARS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    Chat,
    Character,
    Global,
    Preset,
    Script,
    Extension,
}

impl Layer {
    /// 這一層需要原 ID（角色卡、腳本、擴充）
    fn takes_id(self) -> bool {
        matches!(self, Layer::Character | Layer::Script | Layer::Extension)
    }

    /// 落在桌目錄裡
    fn in_world(self) -> bool {
        matches!(self, Layer::Chat | Layer::Character | Layer::Script)
    }

    fn name(self) -> &'static str {
        match self {
            Layer::Chat => "chat",
            Layer::Character => "character",
            Layer::Global => "global",
            Layer::Preset => "preset",
            Layer::Script => "script",
            Layer::Extension => "extension",
        }
    }
}

/// 磁碟上的檔：檔內保留原 ID，讀到時核對（檔名只是 ID 的雜湊）。
#[derive(Serialize, Deserialize)]
struct LayerFile {
    id: String,
    rev: String,
    vars: VarsTable,
}

/// 一層的現況：沒有檔案＝`rev: None`、`vars: "{}"`。`vars` 是原始 JSON 文字，保住鍵順序。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LayerDoc {
    pub rev: Option<String>,
    pub vars: String,
}

impl LayerDoc {
    fn empty() -> Self {
        Self {
            rev: None,
            vars: "{}".to_owned(),
        }
    }
}

/// 身分檔名：原 ID 的 SHA-256 前 32 個十六進位字元。
pub fn identity_name(id: &str) -> String {
    let digest = Sha256::digest(id.as_bytes());
    digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn check_id(layer: Layer, id: Option<&str>) -> DataResult<Option<&str>> {
    match (layer.takes_id(), id) {
        (true, Some(id)) if !id.is_empty() && id.chars().count() <= MAX_ID_CHARS => Ok(Some(id)),
        (true, _) => Err(invalid_data(format!(
            "card-vars: {} 層需要 1～{MAX_ID_CHARS} 字元的 ID",
            layer.name()
        ))),
        (false, _) => Ok(None),
    }
}

fn card_vars_dir(root: &Path, world_id: &str) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?.join("card-vars"))
}

fn root_dir(root: &Path) -> PathBuf {
    root.join("card-vars")
}

/// 這一層的檔案路徑與檔內該存的 ID。ID 一律先驗長度，檔名是雜湊，不會有路徑逃逸。
fn layer_path(
    root: &Path,
    world_id: &str,
    layer: Layer,
    id: Option<&str>,
) -> DataResult<(PathBuf, String)> {
    let id = check_id(layer, id)?;
    Ok(match layer {
        Layer::Chat => (
            card_vars_dir(root, world_id)?.join("chat.json"),
            "chat".to_owned(),
        ),
        Layer::Character => {
            let id = id.unwrap_or_default();
            (
                card_vars_dir(root, world_id)?
                    .join("character")
                    .join(format!("{}.json", identity_name(id))),
                id.to_owned(),
            )
        }
        Layer::Script => {
            let id = id.unwrap_or_default();
            (
                card_vars_dir(root, world_id)?
                    .join("script")
                    .join(format!("{}.json", identity_name(id))),
                id.to_owned(),
            )
        }
        Layer::Global => (root_dir(root).join("global.json"), "global".to_owned()),
        Layer::Preset => (root_dir(root).join("preset.json"), "app".to_owned()),
        Layer::Extension => {
            let id = id.unwrap_or_default();
            (
                root_dir(root)
                    .join("extension")
                    .join(format!("{}.json", identity_name(id))),
                id.to_owned(),
            )
        }
    })
}

/// 一個層檔的狀態。只有「確認不存在」才能當空表；損壞與身分衝突都不是空表，讀回報錯、寫入拒絕並保留原檔。
enum Slot {
    Missing,
    Present(LayerFile),
    /// 檔案存在但不是合法的層檔（內容壞掉）
    Corrupt,
    /// 檔內 ID 與要讀寫的 ID 不同（雜湊撞名或被改）
    IdMismatch,
}

impl Slot {
    /// 不能當作「不存在」的狀態的錯誤代碼
    fn unusable(&self) -> Option<&'static str> {
        match self {
            Slot::Corrupt => Some("corrupt-file"),
            Slot::IdMismatch => Some("id-mismatch"),
            _ => None,
        }
    }
}

/// 讀檔（鎖內）。IO 錯誤（除了不存在）原樣回錯。
fn read_slot(file: &LockedFile<'_>, doc_id: &str) -> DataResult<Slot> {
    let Some(bytes) = file.read()? else {
        return Ok(Slot::Missing);
    };
    Ok(match serde_json::from_slice::<LayerFile>(&bytes) {
        Err(_) => Slot::Corrupt,
        Ok(parsed) if parsed.id != doc_id => Slot::IdMismatch,
        Ok(parsed) => Slot::Present(parsed),
    })
}

fn doc_of(file: Option<LayerFile>) -> LayerDoc {
    match file {
        Some(file) => LayerDoc {
            rev: Some(file.rev),
            vars: file.vars.text().to_owned(),
        },
        None => LayerDoc::empty(),
    }
}

fn with_lock<T>(path: &Path, in_world: bool, work: impl FnOnce(&LockedFile<'_>) -> T) -> T {
    if in_world {
        with_file_lock(path, work)
    } else {
        with_root_file_lock(path, work)
    }
}

/// 讀一層的現況：不存在回空表，損壞、身分衝突、讀不了都回錯（不當空表）。
pub fn read_layer(
    root: &Path,
    world_id: &str,
    layer: Layer,
    id: Option<&str>,
) -> DataResult<LayerDoc> {
    let (path, doc_id) = layer_path(root, world_id, layer, id)?;
    with_lock(&path, layer.in_world(), |file| {
        match read_slot(file, &doc_id)? {
            Slot::Missing => Ok(LayerDoc::empty()),
            Slot::Present(parsed) => Ok(doc_of(Some(parsed))),
            other => Err(invalid_data(format!(
                "card-vars: {}（{}）",
                other.unusable().unwrap_or("unreadable"),
                path.display()
            ))),
        }
    })
}

/// 面板掛載時一次讀回的層：`key` 與沙盒的寫入目標同一套（`chat`、`character:<id>`、`global`、
/// `preset`、`script:<原 ID>`、`extension:<原 ID>`）。script／extension 列出已存在的全部（卡片讀值是
/// 同步的，沙盒得先拿到）；確認沒有檔案的層沙盒當空表、版本「無」。讀不了的層帶 `error`（未知不等於
/// 不存在：沙盒讀取拋錯、寫入拒絕）；script／extension 資料夾裡有讀不了的檔時，整個類別以
/// `script:`／`extension:`（空 ID）這個 key 標錯，因為壞檔的 ID 不可知。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LayerEntry {
    pub key: String,
    #[serde(flatten)]
    pub doc: LayerDoc,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl LayerEntry {
    fn ok(key: String, doc: LayerDoc) -> Self {
        Self {
            key,
            doc,
            error: None,
        }
    }

    fn failed(key: String, error: impl ToString) -> Self {
        Self {
            key,
            doc: LayerDoc::empty(),
            error: Some(error.to_string()),
        }
    }
}

pub fn load_layers(
    root: &Path,
    world_id: &str,
    character_id: Option<&str>,
) -> DataResult<Vec<LayerEntry>> {
    let mut entries = Vec::new();
    let mut single = |layer: Layer, id: Option<&str>, key: String| {
        entries.push(match read_layer(root, world_id, layer, id) {
            Ok(doc) => LayerEntry::ok(key, doc),
            Err(error) => LayerEntry::failed(key, error),
        });
    };
    single(Layer::Chat, None, "chat".to_owned());
    if let Some(id) = character_id.filter(|id| check_id(Layer::Character, Some(id)).is_ok()) {
        single(Layer::Character, Some(id), format!("character:{id}"));
    }
    single(Layer::Global, None, "global".to_owned());
    single(Layer::Preset, None, "preset".to_owned());
    let script_dir = card_vars_dir(root, world_id)?.join("script");
    entries.extend(list_dir(&script_dir, true, "script"));
    entries.extend(list_dir(
        &root_dir(root).join("extension"),
        false,
        "extension",
    ));
    Ok(entries)
}

/// 掃一個「身分檔名」資料夾：資料夾不存在就是沒有；讀不了、壞掉、檔內 ID 的雜湊對不上檔名的檔，
/// 整個類別標錯（不能把它們當不存在）。
fn list_dir(dir: &Path, in_world: bool, prefix: &str) -> Vec<LayerEntry> {
    let blocked = |error: String| vec![LayerEntry::failed(format!("{prefix}:"), error)];
    let read = match std::fs::read_dir(dir) {
        Ok(read) => read,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => return blocked(error.to_string()),
    };
    let mut names = Vec::new();
    for entry in read {
        match entry {
            Ok(entry) => names.push(entry.path()),
            Err(error) => return blocked(error.to_string()),
        }
    }
    names.retain(|path| path.extension().is_some_and(|ext| ext == "json"));
    names.sort();
    let mut entries = Vec::new();
    for path in names {
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default()
            .to_owned();
        let read = with_lock(&path, in_world, |file| file.read());
        let bytes = match read {
            Ok(Some(bytes)) => bytes,
            Ok(None) => continue,
            Err(error) => return blocked(format!("{}: {error}", path.display())),
        };
        match serde_json::from_slice::<LayerFile>(&bytes) {
            Ok(file) if identity_name(&file.id) == stem => entries.push(LayerEntry::ok(
                format!("{prefix}:{}", file.id),
                doc_of(Some(file)),
            )),
            Ok(_) => return blocked(format!("id-mismatch: {}", path.display())),
            Err(_) => return blocked(format!("corrupt-file: {}", path.display())),
        }
    }
    entries
}

/// 寫入結果；被拒時附權威值，宿主拿它推回沙盒。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LayerWrite {
    /// 寫成了，`rev` 是新版本
    LayerOk { rev: String },
    Stale {
        found: bool,
        rev: Option<String>,
        table: Option<String>,
    },
    Rejected {
        code: String,
        found: bool,
        rev: Option<String>,
        table: Option<String>,
    },
}

fn authority(doc: LayerDoc) -> (bool, Option<String>, Option<String>) {
    (true, doc.rev, Some(doc.vars))
}

fn stale(doc: LayerDoc) -> LayerWrite {
    let (found, rev, table) = authority(doc);
    LayerWrite::Stale { found, rev, table }
}

/// 在同檔鎖內核對 rev 並原子替換寫入。檔案損壞或身分衝突一律拒絕、原檔不動。
fn write_locked(
    file: &LockedFile<'_>,
    doc_id: &str,
    expected_rev: Option<&str>,
    vars: &VarsTable,
) -> DataResult<LayerWrite> {
    let current = match read_slot(file, doc_id)? {
        Slot::Present(parsed) => Some(parsed),
        Slot::Missing => None,
        other => {
            return Ok(LayerWrite::Rejected {
                code: other.unusable().unwrap_or("unreadable").to_owned(),
                found: false,
                rev: None,
                table: None,
            })
        }
    };
    if current.as_ref().map(|doc| doc.rev.as_str()) != expected_rev {
        return Ok(stale(doc_of(current)));
    }
    let rev = new_token();
    let bytes = serde_json::to_vec(&LayerFile {
        id: doc_id.to_owned(),
        rev: rev.clone(),
        vars: vars.clone(),
    })?;
    file.write_atomic(&bytes)?;
    Ok(LayerWrite::LayerOk { rev })
}

/// 寫一層的整張表（`vars_json` 的上限與 message 層同一組，8.8）。六層一律在這桌的短提交鎖內核對桌世代
/// （根目錄的層也是：整桌還原後送達的舊請求不能落檔），不符回 stale、不改檔。
pub fn write_layer(
    root: &Path,
    world_id: &str,
    layer: Layer,
    id: Option<&str>,
    generation: u64,
    expected_rev: Option<&str>,
    vars_json: &str,
) -> DataResult<LayerWrite> {
    let (path, doc_id) = layer_path(root, world_id, layer, id)?;
    let current = || match read_layer(root, world_id, layer, id) {
        Ok(doc) => authority(doc),
        Err(_) => (false, None, None),
    };
    let table = match parse_table(vars_json) {
        Ok(table) => table,
        Err(limit) => {
            let (found, rev, table) = current();
            return Ok(LayerWrite::Rejected {
                code: limit.code.to_owned(),
                found,
                rev,
                table,
            });
        }
    };
    let vars = VarsTable::from_json(&table);
    with_commit(root, world_id, |tx| {
        if generation != message_vars::generation(tx) {
            return Ok(match read_layer(root, world_id, layer, id) {
                Ok(doc) => stale(doc),
                Err(_) => LayerWrite::Stale {
                    found: false,
                    rev: None,
                    table: None,
                },
            });
        }
        with_lock(&path, layer.in_world(), |file| {
            write_locked(file, &doc_id, expected_rev, &vars)
        })
    })
}

#[cfg(test)]
mod tests;
