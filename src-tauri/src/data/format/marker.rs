//! `worlds/<id>/format.json` 的判讀與寫入。平常存檔不重寫標記。
#[cfg(test)]
use std::cell::Cell;
use std::fs;
use std::path::Path;

use serde_json::Value;

use super::super::state::WorldState;
use super::super::DataResult;

/// 這個程式認得、也會寫進新桌的格式。轉換鏈目前是空的。
pub const CURRENT_FORMAT: u64 = 1;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
thread_local! {
    static TEST_CURRENT: Cell<Option<u64>> = const { Cell::new(None) };
}

/// 測試把「本版」暫時抬高，才能用假轉換驗提交機制。正式版永遠是 [`CURRENT_FORMAT`]。
pub fn current_format() -> u64 {
    #[cfg(test)]
    {
        TEST_CURRENT.with(|cell| cell.get().unwrap_or(CURRENT_FORMAT))
    }
    #[cfg(not(test))]
    {
        CURRENT_FORMAT
    }
}

#[cfg(test)]
pub(crate) struct CurrentOverride {
    previous: Option<u64>,
}

#[cfg(test)]
impl CurrentOverride {
    pub(crate) fn set(version: u64) -> Self {
        let previous = TEST_CURRENT.with(|cell| cell.replace(Some(version)));
        Self { previous }
    }
}

#[cfg(test)]
impl Drop for CurrentOverride {
    fn drop(&mut self) {
        TEST_CURRENT.with(|cell| cell.set(self.previous));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatVersion {
    /// 正整數，或「缺標記且 state.json 能用現行 WorldState 解開」時的 1。
    Known(u64),
    /// 解不開、不是正整數，或缺檔且 state 也解不開。當成比本版新、版本不明。
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatInfo {
    pub version: FormatVersion,
    pub app_version: Option<String>,
}

pub fn read_format(dir: &Path) -> FormatInfo {
    let marker_path = dir.join("format.json");
    if marker_path.is_file() {
        let Ok(text) = fs::read_to_string(&marker_path) else {
            return FormatInfo {
                version: FormatVersion::Unknown,
                app_version: None,
            };
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            return FormatInfo {
                version: FormatVersion::Unknown,
                app_version: None,
            };
        };
        let app_version = value
            .get("app_version")
            .and_then(Value::as_str)
            .map(str::to_owned);
        // 正整數才算版本。1.5、2.0（as_u64 不收）與字串都當讀不懂。
        let Some(number) = value.get("format_version").and_then(Value::as_u64) else {
            return FormatInfo {
                version: FormatVersion::Unknown,
                app_version,
            };
        };
        if number == 0 {
            return FormatInfo {
                version: FormatVersion::Unknown,
                app_version,
            };
        }
        return FormatInfo {
            version: FormatVersion::Known(number),
            app_version,
        };
    }

    let state_path = dir.join("state.json");
    let Ok(text) = fs::read_to_string(&state_path) else {
        return FormatInfo {
            version: FormatVersion::Unknown,
            app_version: None,
        };
    };
    if serde_json::from_str::<WorldState>(&text).is_ok() {
        FormatInfo {
            version: FormatVersion::Known(1),
            app_version: None,
        }
    } else {
        FormatInfo {
            version: FormatVersion::Unknown,
            app_version: None,
        }
    }
}

/// 備份能不能拿來玩：版本已知且不大於本版。不明或更新的都不算。
pub fn version_playable(dir: &Path) -> bool {
    matches!(read_format(dir).version, FormatVersion::Known(v) if v <= current_format())
}

pub fn loose_name(dir: &Path, fallback_id: &str) -> String {
    fs::read_to_string(dir.join("state.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| {
            value
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| fallback_id.to_owned())
}

pub fn marker_json(version: u64) -> DataResult<String> {
    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "format_version": version,
        "app_version": APP_VERSION,
    }))?)
}

pub fn write_marker(dir: &Path, version: u64) -> DataResult<()> {
    let path = dir.join("format.json");
    super::super::world_file::write_bytes_raw(&path, marker_json(version)?.as_bytes())?;
    super::super::world_file::fsync_file(&path)?;
    super::super::world_file::fsync_dir(dir)?;
    Ok(())
}
