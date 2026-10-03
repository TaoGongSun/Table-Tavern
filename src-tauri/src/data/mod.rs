use serde::{Deserialize, Serialize};
use std::error::Error;

mod character;
mod config;
mod format;
mod paths;
mod scene;
mod state;
#[cfg(test)]
mod test_support;
mod world;
mod world_file;
mod world_lock;
mod worldbook;

pub use character::{
    delete_character, list_characters, read_character, read_player_card, reorder_characters,
    set_character_archived, set_character_auto_hidden, write_character, CharacterCard,
    CharacterMeta,
};
pub use config::{
    install_sponsor_pack, migrate_legacy_config, read_config, read_model_catalog,
    sponsor_pack_active, update_config, update_config_with, write_model_catalog, AppConfig,
};
pub use format::{open_world, read_world_readonly, restore_world_backup, OpenWorld, ReadonlyWorld};
pub(crate) use paths::{
    character_path, gallery_dir, gm_image_path, import_pending_path, import_receipts_path,
    import_source_file_path, interface_shell_path, lanes_path, mechanism_log_path,
    validate_single_line, world_card_path,
};
pub use scene::{
    appeared_card_names, appeared_person_titles, event_full_text, marker_heading, prompt_lang,
    EventMarker,
};
pub use scene::{
    append_opening, append_transcript, begin_next_scene, export_scene_markdown,
    export_transcript_markdown, fork_scene, opening_checkpoint, pop_transcript, read_transcript,
    remove_transcript_event, replace_scene_summary, revert_scene, scene_label,
    set_last_transcript_state, sync_scene_state_tree, TranscriptEvent, TranscriptKind,
};
pub(crate) use scene::{lang_key, name_matches, player_fallback_name, split_present_names};
pub(crate) use state::is_false;
pub use state::{
    node_at, read_state, set_tree_value, write_state, Condition, FieldKind, FieldRule, InjectLevel,
    Mechanism, StateNode, TableState, Trigger, TriggerCase, TriggerMode, UpdateMode, WorldState,
};
pub use world::{
    create_reset_world, create_sample_world, create_world, delete_world, list_worlds,
    read_interface_shell, read_refactor_outcome, read_world_md, reclaim_world_if_empty,
    rename_world, world_has_state_bar, write_interface_shell, write_refactor_outcome,
    write_world_md, WorldMeta,
};
pub(crate) use world_file::{commit_world_append, commit_world_remove, commit_world_write};
#[cfg(test)]
pub(crate) use world_file::{write_hook, AppendFailGuard, RemoveFailGuard, RenameFailGuard};
#[cfg(test)]
pub(crate) use world_lock::test_exclusive;
pub(crate) use world_lock::{
    install_guarded, refuse_if_updating, update_gate_raised, UpdateGateClosed,
};
pub use world_lock::{
    try_world_exclusive, world_exclusive_async, world_write_permit, world_write_permit_async,
    WorldExclusive,
};
pub use worldbook::{
    character_to_worldbook_entry, dedupe_worldbook, delete_worldbook_entry, export_worldbook,
    import_worldbook, read_worldbook, reorder_worldbook_entries, upsert_worldbook_entry,
    worldbook_entry_to_character, Visibility, WorldbookEntry, WorldbookImport,
};

// 這幾項在 data 之外沒有引用者：同檔時不觸發 lint，改成 re-export 才會，
// 拿掉又會讓 facade 對外少掉路徑，所以單獨成行標 allow。
#[allow(unused_imports)]
pub use config::validate_sponsor_pack;
pub(crate) use format::marker::current_format;
pub(crate) use format::{
    delete_world_backup, discover_ids, list_world_backups, live_dir, loose_name, read_format,
    BackupList, FormatVersion, CURRENT_FORMAT,
};
pub(crate) use format::{remove_reset_build_root, replace_world_from_build, reset_build_root};
#[allow(unused_imports)]
pub(crate) use paths::{refactor_outcome_path, validate_id};
#[allow(unused_imports)]
pub use state::SceneLabel;
#[cfg(test)]
pub(crate) use world_lock::run_gated_local;

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
        Ok((
            local.tm_year + 1900,
            local.tm_mon + 1,
            local.tm_mday,
            local.tm_hour,
            local.tm_min,
            local.tm_sec,
        ))
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

pub(crate) fn invalid_data(message: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message.into()).into()
}
