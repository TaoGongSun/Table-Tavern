//! Mac 殘留整理。決定下一步是純函式；真的改目錄在 `cleanup_residue`，對調由呼叫端注入。
//! `renamex_np` 不在這裡。從 `Table Tavern.app` 以外的位置啟動時什麼都不做。

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::macos::{
    is_official_bundle, read_bundle_info, same_bundle, BUNDLE_ID, PREVIOUS_NAME, UPDATE_NAME,
};
use super::semver_util::versions_equal;
use super::store::{replace_synced, sync_dir};

pub(crate) const RECORD_NAME: &str = ".TableTavern-update.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CleanupMode {
    /// 安裝步驟 2 與步驟 5。不刪救援副本。
    BeforeExtract,
    /// 啟動後。救援副本只在有可用回退點時刪。
    AfterLaunch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CleanupReport {
    pub update_remains: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Stage {
    Extracting,
    Swapped,
    PreviousSwapped,
}

/// 同磁碟改名不變的目錄識別。Unix 是 dev＋inode；Windows 用磁碟序號與 file index，
/// 否則這條測試在 Windows CI 上無法分辨兩份同版 App。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct AppId {
    dev: u64,
    ino: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct RecordFile {
    from: String,
    target: String,
    stage: Stage,
    /// 要放進 previous 的那份 App。舊紀錄沒有這個欄位。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous_app: Option<AppId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ValidRecord {
    from: String,
    target: String,
    stage: Stage,
    previous_app: Option<AppId>,
}

/// 純函式的下一步。改階段與動目錄分成兩步，推論時才會先寫紀錄再動目錄。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Decision {
    DeleteExtract,
    AdvanceTo(Stage),
    RenameToPrevious,
    SwapWithPrevious,
    DeleteDotted,
    /// `block_previous`：無法判定，救援副本也不動。
    Stop {
        block_previous: bool,
    },
    UpdateAbsent,
}

struct Snap {
    update_exists: bool,
    update_readable: bool,
    update_bundle_ok: bool,
    update_is_running: bool,
    update_version: Option<String>,
    previous_exists: bool,
    previous_is_ours: bool,
    previous_version: Option<String>,
    running_version: String,
    update_id: Option<AppId>,
    previous_id: Option<AppId>,
    record: Option<ValidRecord>,
}

pub(crate) fn write_record(
    parent: &Path,
    from: &str,
    target: &str,
    stage: Stage,
) -> Result<(), String> {
    write_record_file(parent, from, target, stage, None)
}

fn write_record_file(
    parent: &Path,
    from: &str,
    target: &str,
    stage: Stage,
    previous_app: Option<AppId>,
) -> Result<(), String> {
    let body = serde_json::to_vec_pretty(&RecordFile {
        from: from.to_owned(),
        target: target.to_owned(),
        stage,
        previous_app,
    })
    .map_err(|error| error.to_string())?;
    replace_synced(&parent.join(RECORD_NAME), &body)
}

pub(crate) fn delete_record(parent: &Path) -> Result<(), String> {
    let path = parent.join(RECORD_NAME);
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(&path).map_err(|error| error.to_string())?;
    sync_dir(parent)?;
    Ok(())
}

pub(crate) fn cleanup_residue(
    running_app: &Path,
    mode: CleanupMode,
    rollback_ready: impl Fn(&str) -> bool,
    swap: impl Fn(&Path, &Path) -> Result<(), String>,
) -> Result<CleanupReport, String> {
    if !is_official_bundle(running_app) {
        return Ok(CleanupReport {
            update_remains: false,
        });
    }
    let Some(parent) = running_app.parent() else {
        return Ok(CleanupReport {
            update_remains: false,
        });
    };
    let info = read_bundle_info(running_app);
    if info.bundle_id.as_deref() != Some(BUNDLE_ID) {
        return Ok(CleanupReport {
            update_remains: false,
        });
    }
    let Some(running_version) = info.version else {
        return Ok(CleanupReport {
            update_remains: false,
        });
    };

    let mut block_previous = false;
    let mut finished = false;
    for _ in 0..8 {
        let snap = snapshot(parent, running_app, &running_version);
        match decide(&snap) {
            Decision::DeleteExtract | Decision::DeleteDotted => {
                remove_dir(&parent.join(UPDATE_NAME))?;
            }
            Decision::AdvanceTo(stage) => {
                let record = snap.record.ok_or_else(|| "沒有替換紀錄".to_owned())?;
                // 進 swapped 時先記下要放進 previous 的那份，fsync 之後下一輪才對調。
                // 新流程取不到識別就停，不寫 swapped、不對調，讓下次再試。
                let previous_app = if stage == Stage::Swapped {
                    let Some(id) = app_id(&parent.join(UPDATE_NAME)) else {
                        return Err("無法取得要留下的 App 識別".to_owned());
                    };
                    Some(id)
                } else {
                    record.previous_app
                };
                write_record_file(parent, &record.from, &record.target, stage, previous_app)?;
            }
            Decision::RenameToPrevious => {
                let update = parent.join(UPDATE_NAME);
                let previous = parent.join(PREVIOUS_NAME);
                fs::rename(&update, &previous).map_err(|error| error.to_string())?;
            }
            Decision::SwapWithPrevious => {
                swap(&parent.join(UPDATE_NAME), &parent.join(PREVIOUS_NAME))?;
            }
            Decision::Stop {
                block_previous: block,
            } => {
                block_previous = block;
                finished = true;
                break;
            }
            Decision::UpdateAbsent => {
                delete_record(parent)?;
                finished = true;
                break;
            }
        }
    }
    if !finished {
        return Err("殘留整理無法收斂".to_owned());
    }
    if !block_previous {
        maybe_delete_previous(parent, running_app, mode, rollback_ready)?;
    }
    Ok(CleanupReport {
        update_remains: parent.join(UPDATE_NAME).exists(),
    })
}

fn decide(snap: &Snap) -> Decision {
    if !snap.update_exists {
        return Decision::UpdateAbsent;
    }
    // 點開頭那份解析後就是正在跑的 bundle（常見是 symlink）。刪目錄可能跟著本體走，所以留著。
    if snap.update_is_running {
        return Decision::Stop {
            block_previous: false,
        };
    }
    if !snap.update_readable {
        return Decision::DeleteExtract;
    }
    if !snap.update_bundle_ok {
        return Decision::Stop {
            block_previous: false,
        };
    }
    let Some(version) = snap.update_version.as_deref() else {
        return Decision::DeleteExtract;
    };
    let Some(record) = snap.record.as_ref() else {
        return if versions_equal(version, &snap.running_version) {
            Decision::DeleteExtract
        } else {
            Decision::Stop {
                block_previous: true,
            }
        };
    };
    match record.stage {
        Stage::Extracting => {
            if versions_equal(version, &record.target)
                && !versions_equal(&snap.running_version, &record.target)
            {
                Decision::DeleteExtract
            } else if versions_equal(version, &record.from)
                && versions_equal(&snap.running_version, &record.target)
            {
                Decision::AdvanceTo(Stage::Swapped)
            } else {
                Decision::Stop {
                    block_previous: true,
                }
            }
        }
        Stage::Swapped => match record.previous_app {
            Some(want) => {
                let on_previous = snap.previous_id == Some(want);
                let on_update = snap.update_id == Some(want);
                if on_previous && on_update {
                    Decision::Stop {
                        block_previous: true,
                    }
                } else if on_previous {
                    Decision::AdvanceTo(Stage::PreviousSwapped)
                } else if on_update {
                    place_into_previous(snap)
                } else {
                    Decision::Stop {
                        block_previous: true,
                    }
                }
            }
            None => {
                if versions_equal(version, &record.from) {
                    place_into_previous(snap)
                } else if snap
                    .previous_version
                    .as_deref()
                    .is_some_and(|previous| versions_equal(previous, &record.from))
                {
                    Decision::AdvanceTo(Stage::PreviousSwapped)
                } else {
                    Decision::Stop {
                        block_previous: true,
                    }
                }
            }
        },
        Stage::PreviousSwapped => {
            if snap
                .previous_version
                .as_deref()
                .is_some_and(|previous| versions_equal(previous, &record.from))
            {
                Decision::DeleteDotted
            } else {
                Decision::Stop {
                    block_previous: true,
                }
            }
        }
    }
}

fn place_into_previous(snap: &Snap) -> Decision {
    if !snap.previous_exists {
        Decision::RenameToPrevious
    } else if snap.previous_is_ours {
        Decision::SwapWithPrevious
    } else {
        // previous 不是我們的，不能對調。
        Decision::Stop {
            block_previous: true,
        }
    }
}

fn app_id(path: &Path) -> Option<AppId> {
    #[cfg(test)]
    if FAIL_APP_ID.with(|flag| flag.get()) {
        return None;
    }
    let meta = fs::symlink_metadata(path).ok()?;
    file_id_of(&meta)
}

#[cfg(test)]
thread_local! {
    static FAIL_APP_ID: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
struct FailAppId;

#[cfg(test)]
impl FailAppId {
    fn arm() -> Self {
        FAIL_APP_ID.with(|flag| flag.set(true));
        Self
    }
}

#[cfg(test)]
impl Drop for FailAppId {
    fn drop(&mut self) {
        FAIL_APP_ID.with(|flag| flag.set(false));
    }
}

#[cfg(unix)]
fn file_id_of(meta: &fs::Metadata) -> Option<AppId> {
    use std::os::unix::fs::MetadataExt;
    Some(AppId {
        dev: meta.dev(),
        ino: meta.ino(),
    })
}

/// Mac 替換流程只在 macOS 跑；其他平台取不到識別，流程照規格停下。
#[cfg(not(unix))]
fn file_id_of(_meta: &fs::Metadata) -> Option<AppId> {
    None
}

fn maybe_delete_previous(
    parent: &Path,
    running_app: &Path,
    mode: CleanupMode,
    rollback_ready: impl Fn(&str) -> bool,
) -> Result<(), String> {
    if mode != CleanupMode::AfterLaunch {
        return Ok(());
    }
    let previous = parent.join(PREVIOUS_NAME);
    if !previous.exists() || same_bundle(&previous, running_app) {
        return Ok(());
    }
    let info = read_bundle_info(&previous);
    if !info.plist_readable || info.bundle_id.as_deref() != Some(BUNDLE_ID) {
        return Ok(());
    }
    let Some(version) = info.version else {
        return Ok(());
    };
    if rollback_ready(&version) {
        remove_dir(&previous)?;
    }
    Ok(())
}

fn snapshot(parent: &Path, running_app: &Path, running_version: &str) -> Snap {
    let update = parent.join(UPDATE_NAME);
    let previous = parent.join(PREVIOUS_NAME);
    let update_info = read_bundle_info(&update);
    let previous_info = read_bundle_info(&previous);
    let previous_is_ours = previous.exists()
        && !same_bundle(&previous, running_app)
        && previous_info.plist_readable
        && previous_info.bundle_id.as_deref() == Some(BUNDLE_ID)
        && previous_info.version.is_some();
    Snap {
        update_exists: update.exists(),
        update_readable: update_info.plist_readable,
        update_bundle_ok: update_info.bundle_id.as_deref() == Some(BUNDLE_ID),
        update_is_running: update.exists() && same_bundle(&update, running_app),
        update_version: update_info.version,
        previous_exists: previous.exists(),
        previous_is_ours,
        previous_version: previous_is_ours.then_some(previous_info.version).flatten(),
        running_version: running_version.to_owned(),
        update_id: update.exists().then(|| app_id(&update)).flatten(),
        previous_id: previous.exists().then(|| app_id(&previous)).flatten(),
        record: load_record(parent),
    }
}

fn load_record(parent: &Path) -> Option<ValidRecord> {
    let text = fs::read_to_string(parent.join(RECORD_NAME)).ok()?;
    let file: RecordFile = serde_json::from_str(&text).ok()?;
    if file.from.is_empty() || file.target.is_empty() {
        return None;
    }
    Some(ValidRecord {
        from: file.from,
        target: file.target,
        stage: file.stage,
        previous_app: file.previous_app,
    })
}

fn remove_dir(path: &Path) -> Result<(), String> {
    fs::remove_dir_all(path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::super::macos::{replace_installed_app, APP_NAME, BUNDLE_ID, CANNOT_REPLACE};
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(label: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("tt-residue-{label}-{}", ulid::Ulid::generate()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_app(dir: &Path, version: &str, bundle_id: &str) {
        let plist_dir = dir.join("Contents");
        fs::create_dir_all(&plist_dir).unwrap();
        let plist = format!(
            "<plist><dict><key>CFBundleIdentifier</key><string>{bundle_id}</string>\
<key>CFBundleShortVersionString</key><string>{version}</string></dict></plist>"
        );
        fs::write(plist_dir.join("Info.plist"), plist).unwrap();
    }

    fn fake_swap(new_app: &Path, old_app: &Path) -> Result<(), String> {
        let parent = old_app.parent().unwrap();
        let holding = parent.join(".tt-swap-holding");
        fs::rename(old_app, &holding).map_err(|error| error.to_string())?;
        fs::rename(new_app, old_app).map_err(|error| error.to_string())?;
        fs::rename(&holding, new_app).map_err(|error| error.to_string())?;
        Ok(())
    }

    fn stamp(path: &Path, when: std::time::SystemTime) {
        fs::OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(when)
            .unwrap();
    }

    fn version_of(app: &Path) -> Option<String> {
        read_bundle_info(app).version
    }

    fn stage_of(parent: &Path) -> Option<String> {
        let text = fs::read_to_string(parent.join(RECORD_NAME)).ok()?;
        let value: serde_json::Value = serde_json::from_str(&text).ok()?;
        value.get("stage")?.as_str().map(str::to_owned)
    }

    fn run(running: &Path, mode: CleanupMode, ready: bool) -> CleanupReport {
        cleanup_residue(running, mode, |_| ready, fake_swap).unwrap()
    }

    fn snap_base() -> Snap {
        Snap {
            update_exists: true,
            update_readable: true,
            update_bundle_ok: true,
            update_is_running: false,
            update_version: Some("0.9.0".to_owned()),
            previous_exists: false,
            previous_is_ours: false,
            previous_version: None,
            running_version: "0.10.0".to_owned(),
            update_id: None,
            previous_id: None,
            record: None,
        }
    }

    #[test]
    fn decisions_advance_the_stage_before_the_directory_changes() {
        let mut snap = snap_base();
        snap.record = Some(ValidRecord {
            from: "0.9.0".to_owned(),
            target: "0.10.0".to_owned(),
            stage: Stage::Extracting,
            previous_app: None,
        });
        assert_eq!(decide(&snap), Decision::AdvanceTo(Stage::Swapped));
        snap.record.as_mut().unwrap().stage = Stage::Swapped;
        assert_eq!(decide(&snap), Decision::RenameToPrevious);

        snap.previous_exists = true;
        snap.previous_is_ours = true;
        snap.previous_version = Some("0.8.0".to_owned());
        assert_eq!(decide(&snap), Decision::SwapWithPrevious);
        snap.update_version = Some("0.8.0".to_owned());
        snap.previous_version = Some("0.9.0".to_owned());
        assert_eq!(decide(&snap), Decision::AdvanceTo(Stage::PreviousSwapped));
        snap.record.as_mut().unwrap().stage = Stage::PreviousSwapped;
        assert_eq!(decide(&snap), Decision::DeleteDotted);

        snap.record = None;
        snap.update_version = Some("0.10.0".to_owned());
        snap.running_version = "0.10.0".to_owned();
        assert_eq!(decide(&snap), Decision::DeleteExtract);
        snap.update_version = Some("0.8.0".to_owned());
        assert_eq!(
            decide(&snap),
            Decision::Stop {
                block_previous: true
            }
        );
        snap.update_exists = false;
        assert_eq!(decide(&snap), Decision::UpdateAbsent);
    }

    #[test]
    fn each_interrupted_stage_continues_and_unknown_states_stop() {
        let root = TempDir::new("stages");
        let running = root.0.join(APP_NAME);
        // 解壓殘留：版本已是目標、執行中還是舊版。
        write_app(&running, "0.9.0", BUNDLE_ID);
        write_app(&root.0.join(UPDATE_NAME), "0.10.0", BUNDLE_ID);
        write_record(&root.0, "0.9.0", "0.10.0", Stage::Extracting).unwrap();
        run(&running, CleanupMode::BeforeExtract, true);
        assert!(!root.0.join(UPDATE_NAME).exists());
        assert!(!root.0.join(RECORD_NAME).exists());

        // 主對調成功但階段還寫著 extracting。previous 已在，對調前紀錄必須已是 swapped。
        write_app(&running, "0.10.0", BUNDLE_ID);
        write_app(&root.0.join(UPDATE_NAME), "0.9.0", BUNDLE_ID);
        write_app(&root.0.join(PREVIOUS_NAME), "0.8.0", BUNDLE_ID);
        write_record(&root.0, "0.9.0", "0.10.0", Stage::Extracting).unwrap();
        let parent = root.0.clone();
        cleanup_residue(
            &running,
            CleanupMode::BeforeExtract,
            |_| true,
            |update, previous| {
                assert_eq!(stage_of(&parent).as_deref(), Some("swapped"));
                fake_swap(update, previous)
            },
        )
        .unwrap();
        assert_eq!(
            version_of(&root.0.join(PREVIOUS_NAME)).as_deref(),
            Some("0.9.0")
        );
        assert!(!root.0.join(UPDATE_NAME).exists());
        assert!(!root.0.join(RECORD_NAME).exists());

        // previous 對調已成功、階段仍是 swapped：只刪點開頭那份，不再對調。
        write_app(&root.0.join(UPDATE_NAME), "0.8.0", BUNDLE_ID);
        write_app(&root.0.join(PREVIOUS_NAME), "0.9.0", BUNDLE_ID);
        write_record(&root.0, "0.9.0", "0.10.0", Stage::Swapped).unwrap();
        cleanup_residue(
            &running,
            CleanupMode::BeforeExtract,
            |_| false,
            |_update, _previous| Err("不該再對調".to_owned()),
        )
        .unwrap();
        assert_eq!(
            version_of(&root.0.join(PREVIOUS_NAME)).as_deref(),
            Some("0.9.0")
        );
        assert!(!root.0.join(UPDATE_NAME).exists());

        // 階段已是 previous_swapped，刪到一半：同樣只重試刪除。
        write_app(&root.0.join(UPDATE_NAME), "0.8.0", BUNDLE_ID);
        write_app(&root.0.join(PREVIOUS_NAME), "0.9.0", BUNDLE_ID);
        write_record(&root.0, "0.9.0", "0.10.0", Stage::PreviousSwapped).unwrap();
        cleanup_residue(
            &running,
            CleanupMode::AfterLaunch,
            |_| false,
            |_a, _b| Err("不該再對調".to_owned()),
        )
        .unwrap();
        assert!(!root.0.join(UPDATE_NAME).exists());
        assert_eq!(
            version_of(&root.0.join(PREVIOUS_NAME)).as_deref(),
            Some("0.9.0")
        );
    }

    #[test]
    fn upgrade_and_downgrade_both_keep_the_swapped_out_app() {
        let root = TempDir::new("both-ways");
        let running = root.0.join(APP_NAME);
        write_app(&running, "0.10.0", BUNDLE_ID);
        write_app(&root.0.join(UPDATE_NAME), "0.9.0", BUNDLE_ID);
        write_record(&root.0, "0.9.0", "0.10.0", Stage::Swapped).unwrap();
        run(&running, CleanupMode::BeforeExtract, true);
        assert_eq!(
            version_of(&root.0.join(PREVIOUS_NAME)).as_deref(),
            Some("0.9.0")
        );

        fs::remove_dir_all(root.0.join(PREVIOUS_NAME)).unwrap();
        write_app(&running, "0.9.0", BUNDLE_ID);
        write_app(&root.0.join(UPDATE_NAME), "0.10.0", BUNDLE_ID);
        write_record(&root.0, "0.10.0", "0.9.0", Stage::Swapped).unwrap();
        run(&running, CleanupMode::BeforeExtract, true);
        assert_eq!(
            version_of(&root.0.join(PREVIOUS_NAME)).as_deref(),
            Some("0.10.0"),
            "降版換下來的較新 App 也要留成 previous"
        );
    }

    #[test]
    fn same_from_version_interrupted_after_swap_drops_the_older_copy() {
        let root = TempDir::new("same-from");
        let running = root.0.join(APP_NAME);
        write_app(&running, "0.10.0", BUNDLE_ID);
        let update = root.0.join(UPDATE_NAME);
        let previous = root.0.join(PREVIOUS_NAME);
        write_app(&update, "0.9.0", BUNDLE_ID);
        write_app(&previous, "0.9.0", BUNDLE_ID);
        let newer_mark = update.join("Contents/which");
        let older_mark = previous.join("Contents/which");
        fs::write(&newer_mark, b"newer").unwrap();
        fs::write(&older_mark, b"older").unwrap();
        let newer_at =
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let older_at =
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
        stamp(&newer_mark, newer_at);
        stamp(&older_mark, older_at);
        let newer_id = app_id(&update).unwrap();
        write_record(&root.0, "0.9.0", "0.10.0", Stage::Extracting).unwrap();

        let parent = root.0.clone();
        let error = cleanup_residue(
            &running,
            CleanupMode::BeforeExtract,
            |_| false,
            |update, previous| {
                let text = fs::read_to_string(parent.join(RECORD_NAME)).unwrap();
                let value: serde_json::Value = serde_json::from_str(&text).unwrap();
                assert_eq!(value["stage"], "swapped");
                let id = app_id(update).unwrap();
                assert_eq!(value["previous_app"]["dev"], id.dev);
                assert_eq!(value["previous_app"]["ino"], id.ino);
                fake_swap(update, previous)?;
                Err("對調後、寫下一階段前中斷".to_owned())
            },
        )
        .unwrap_err();
        assert_eq!(error, "對調後、寫下一階段前中斷");

        cleanup_residue(
            &running,
            CleanupMode::BeforeExtract,
            |_| false,
            |_update, _previous| Err("不該再對調".to_owned()),
        )
        .unwrap();
        let kept = previous.join("Contents/which");
        assert_eq!(fs::read(&kept).unwrap(), b"newer");
        assert_eq!(app_id(&previous), Some(newer_id));
        assert_eq!(fs::metadata(&kept).unwrap().modified().unwrap(), newer_at);
        assert!(!update.exists(), "較舊的那份要刪掉");
        assert!(!root.0.join(RECORD_NAME).exists());
    }

    #[test]
    fn leftover_temp_record_is_ignored_and_interrupted_rewrite_keeps_the_old_file() {
        let root = TempDir::new("atomic");
        let running = root.0.join(APP_NAME);
        write_app(&running, "0.10.0", BUNDLE_ID);
        write_app(&root.0.join(UPDATE_NAME), "0.9.0", BUNDLE_ID);
        write_record(&root.0, "0.9.0", "0.10.0", Stage::Extracting).unwrap();
        let tmp = root.0.join(format!("{RECORD_NAME}.tmp"));
        fs::write(&tmp, b"{").unwrap();
        run(&running, CleanupMode::BeforeExtract, false);
        assert_eq!(
            version_of(&root.0.join(PREVIOUS_NAME)).as_deref(),
            Some("0.9.0"),
            "殘留暫存不是紀錄，整理仍照正式檔把舊版放進 previous"
        );
        assert!(!root.0.join(UPDATE_NAME).exists());

        write_app(&root.0.join(UPDATE_NAME), "0.9.0", BUNDLE_ID);
        write_record(&root.0, "0.9.0", "0.10.0", Stage::Extracting).unwrap();
        let official = fs::read(root.0.join(RECORD_NAME)).unwrap();
        let _hold = super::super::store::FailBeforeRename::arm();
        let error = write_record(&root.0, "0.9.0", "0.10.0", Stage::Swapped).unwrap_err();
        assert_eq!(error, "改寫中斷");
        assert_eq!(fs::read(root.0.join(RECORD_NAME)).unwrap(), official);
        assert_eq!(stage_of(&root.0).as_deref(), Some("extracting"));
        assert!(tmp.is_file(), "中斷後暫存留著，正式檔不被蓋掉");
    }

    #[test]
    fn missing_app_id_stops_without_swapping_or_advancing() {
        let root = TempDir::new("no-id");
        let running = root.0.join(APP_NAME);
        write_app(&running, "0.10.0", BUNDLE_ID);
        let update = root.0.join(UPDATE_NAME);
        let previous = root.0.join(PREVIOUS_NAME);
        write_app(&update, "0.9.0", BUNDLE_ID);
        write_app(&previous, "0.9.0", BUNDLE_ID);
        fs::write(update.join("Contents/which"), b"dotted").unwrap();
        fs::write(previous.join("Contents/which"), b"rescue").unwrap();
        write_record(&root.0, "0.9.0", "0.10.0", Stage::Extracting).unwrap();
        let before = fs::read(root.0.join(RECORD_NAME)).unwrap();
        let _fail = super::FailAppId::arm();
        let error = cleanup_residue(
            &running,
            CleanupMode::BeforeExtract,
            |_| false,
            |_update, _previous| Err("不該對調".to_owned()),
        )
        .unwrap_err();
        assert_eq!(error, "無法取得要留下的 App 識別");
        assert_eq!(fs::read(root.0.join(RECORD_NAME)).unwrap(), before);
        assert_eq!(stage_of(&root.0).as_deref(), Some("extracting"));
        assert_eq!(fs::read(update.join("Contents/which")).unwrap(), b"dotted");
        assert_eq!(
            fs::read(previous.join("Contents/which")).unwrap(),
            b"rescue"
        );
    }

    #[test]
    fn missing_record_deletes_only_the_same_version_and_stops_otherwise() {
        let root = TempDir::new("norecord");
        let running = root.0.join(APP_NAME);
        write_app(&running, "0.10.0", BUNDLE_ID);
        write_app(&root.0.join(UPDATE_NAME), "0.10.0", BUNDLE_ID);
        write_app(&root.0.join(PREVIOUS_NAME), "0.8.0", BUNDLE_ID);
        run(&running, CleanupMode::AfterLaunch, false);
        assert!(!root.0.join(UPDATE_NAME).exists());
        assert!(root.0.join(PREVIOUS_NAME).exists());

        write_app(&root.0.join(UPDATE_NAME), "0.7.0", BUNDLE_ID);
        let report = run(&running, CleanupMode::AfterLaunch, true);
        assert!(report.update_remains);
        assert_eq!(
            version_of(&root.0.join(UPDATE_NAME)).as_deref(),
            Some("0.7.0")
        );
        assert!(
            root.0.join(PREVIOUS_NAME).exists(),
            "無法判定時救援副本也不刪"
        );
    }

    #[test]
    fn unreadable_plist_is_deleted_and_a_foreign_bundle_is_left() {
        let root = TempDir::new("plist");
        let running = root.0.join(APP_NAME);
        write_app(&running, "0.10.0", BUNDLE_ID);
        fs::create_dir_all(root.0.join(UPDATE_NAME).join("Contents")).unwrap();
        write_record(&root.0, "0.9.0", "0.10.0", Stage::Swapped).unwrap();
        run(&running, CleanupMode::BeforeExtract, false);
        assert!(!root.0.join(UPDATE_NAME).exists());
        assert!(!root.0.join(RECORD_NAME).exists());

        write_app(&root.0.join(UPDATE_NAME), "0.11.0", "com.other.app");
        write_record(&root.0, "0.9.0", "0.10.0", Stage::Extracting).unwrap();
        let report = run(&running, CleanupMode::BeforeExtract, false);
        assert!(report.update_remains);
        assert!(root.0.join(RECORD_NAME).is_file());
    }

    #[test]
    fn absent_update_app_drops_the_record() {
        let root = TempDir::new("gone");
        let running = root.0.join(APP_NAME);
        write_app(&running, "0.10.0", BUNDLE_ID);
        write_record(&root.0, "0.9.0", "0.10.0", Stage::Extracting).unwrap();
        run(&running, CleanupMode::BeforeExtract, false);
        assert!(!root.0.join(RECORD_NAME).exists());
    }

    #[test]
    fn previous_is_removed_only_after_launch_when_a_rollback_point_exists() {
        let root = TempDir::new("rescue");
        let running = root.0.join(APP_NAME);
        write_app(&running, "0.10.0", BUNDLE_ID);
        write_app(&root.0.join(PREVIOUS_NAME), "0.9.0", BUNDLE_ID);
        run(&running, CleanupMode::BeforeExtract, true);
        assert!(
            root.0.join(PREVIOUS_NAME).exists(),
            "安裝步驟 2 不刪 previous"
        );

        run(&running, CleanupMode::AfterLaunch, false);
        assert!(root.0.join(PREVIOUS_NAME).exists());

        run(&running, CleanupMode::AfterLaunch, true);
        assert!(!root.0.join(PREVIOUS_NAME).exists());

        write_app(&root.0.join(PREVIOUS_NAME), "0.9.0", BUNDLE_ID);
        fs::create_dir_all(root.0.join(PREVIOUS_NAME).join("Contents")).unwrap();
        // 蓋掉 plist，讀不到就不刪。
        fs::remove_file(root.0.join(PREVIOUS_NAME).join("Contents/Info.plist")).unwrap();
        run(&running, CleanupMode::AfterLaunch, true);
        assert!(root.0.join(PREVIOUS_NAME).is_dir());
    }

    #[test]
    fn launching_from_previous_or_a_symlink_does_not_touch_the_apps() {
        let root = TempDir::new("elsewhere");
        let previous = root.0.join(PREVIOUS_NAME);
        write_app(&previous, "0.2.0", BUNDLE_ID);
        let update = root.0.join(UPDATE_NAME);
        write_app(&update, "0.1.0", BUNDLE_ID);
        cleanup_residue(&previous, CleanupMode::AfterLaunch, |_| true, fake_swap).unwrap();
        assert!(update.join("Contents/Info.plist").is_file());
        assert!(previous.join("Contents/Info.plist").is_file());

        #[cfg(unix)]
        {
            let running = root.0.join(APP_NAME);
            write_app(&running, "0.10.0", BUNDLE_ID);
            fs::remove_dir_all(&update).unwrap();
            std::os::unix::fs::symlink(&running, &update).unwrap();
            cleanup_residue(&running, CleanupMode::AfterLaunch, |_| true, fake_swap).unwrap();
            assert!(running.join("Contents/Info.plist").is_file());
            assert!(update.exists());
        }
    }

    #[test]
    fn a_foreign_running_bundle_id_is_left_alone() {
        let root = TempDir::new("foreign-id");
        let running = root.0.join(APP_NAME);
        write_app(&running, "0.2.0", "com.other.app");
        write_app(&root.0.join(UPDATE_NAME), "0.1.0", BUNDLE_ID);
        write_app(&root.0.join(PREVIOUS_NAME), "0.0.9", BUNDLE_ID);
        write_record(&root.0, "0.1.0", "0.2.0", Stage::Extracting).unwrap();
        cleanup_residue(&running, CleanupMode::AfterLaunch, |_| true, fake_swap).unwrap();
        assert!(root
            .0
            .join(UPDATE_NAME)
            .join("Contents/Info.plist")
            .is_file());
        assert!(root
            .0
            .join(PREVIOUS_NAME)
            .join("Contents/Info.plist")
            .is_file());
        assert!(root.0.join(RECORD_NAME).is_file());
        assert_eq!(
            replace_installed_app(&running, &app_tar("0.3.0"), "0.3.0", |_| true, fake_swap)
                .unwrap_err(),
            CANNOT_REPLACE
        );
        assert_eq!(version_of(&running).as_deref(), Some("0.2.0"));
        assert!(root.0.join(UPDATE_NAME).exists());
        assert!(root.0.join(PREVIOUS_NAME).exists());
    }

    #[test]
    fn an_indeterminate_update_app_stops_the_next_install() {
        let root = TempDir::new("block");
        let running = root.0.join(APP_NAME);
        write_app(&running, "0.2.0", BUNDLE_ID);
        write_app(&root.0.join(UPDATE_NAME), "0.1.0", BUNDLE_ID);
        let error =
            replace_installed_app(&running, &app_tar("0.3.0"), "0.3.0", |_| true, fake_swap)
                .unwrap_err();
        assert_eq!(error, CANNOT_REPLACE);
        assert_eq!(version_of(&running).as_deref(), Some("0.2.0"));
        assert_eq!(
            version_of(&root.0.join(UPDATE_NAME)).as_deref(),
            Some("0.1.0")
        );
    }

    fn app_tar(version: &str) -> Vec<u8> {
        let plist = format!(
            "<plist><dict><key>CFBundleIdentifier</key><string>{BUNDLE_ID}</string>\
<key>CFBundleShortVersionString</key><string>{version}</string></dict></plist>"
        );
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_size(plist.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(
                &mut header,
                "Table Tavern.app/Contents/Info.plist",
                plist.as_bytes(),
            )
            .unwrap();
        let raw = builder.into_inner().unwrap();
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gzip.write_all(&raw).unwrap();
        gzip.finish().unwrap()
    }
}
