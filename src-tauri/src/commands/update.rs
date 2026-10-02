//! 更新的 Tauri 邊界。檢查、下載、安裝、啟動後整理。規則在 `updater`。

use std::path::PathBuf;

use serde_json::{Map, Value};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::UpdaterExt;

use crate::{config_root, data, updater};

#[derive(Clone, serde::Serialize)]
struct ProgressPayload {
    downloaded: u64,
    total: Option<u64>,
}

#[tauri::command]
pub(crate) async fn update_check(
    app: AppHandle,
    state: State<'_, updater::PendingUpdate>,
    manual: bool,
) -> Result<Option<updater::UpdateOffer>, String> {
    // 下載或安裝進行中不打網路，也不覆寫槽裡那一份。回目前的結果，前端才不會把進度洗掉。
    if let Some(held) = {
        let slot = state.inner.lock().await;
        snapshot_if_busy(&slot)
    } {
        return Ok(offer_of_held(&app, Some(held)));
    }

    let auto_preferences = if manual {
        None
    } else {
        match read_preferences(&app) {
            Ok(preferences) => Some(preferences),
            Err(error) => {
                log::warn!("自動檢查讀設定失敗：{error}");
                return Ok(None);
            }
        }
    };
    if !manual && !updater::allow_auto_check(auto_preferences.as_ref()) {
        return Ok(None);
    }

    let plugin = match app.updater() {
        Ok(plugin) => plugin,
        Err(error) => return check_failed(manual, error.to_string()),
    };
    match plugin.check().await {
        Ok(Some(update)) => {
            let preferences = if manual {
                read_preferences(&app).ok()
            } else {
                auto_preferences
            };
            let busy = {
                let mut slot = state.inner.lock().await;
                if let Some(current) = snapshot_if_busy(&slot) {
                    Some(current)
                } else {
                    let version = update.version.clone();
                    let skipped = skipped_flag(preferences.as_ref(), &version);
                    let offer = offer_from(&update, skipped);
                    slot.store_check(Some(update), &version);
                    return Ok(Some(offer));
                }
            };
            Ok(offer_of_held(&app, busy))
        }
        Ok(None) => {
            let busy = {
                let mut slot = state.inner.lock().await;
                if let Some(current) = snapshot_if_busy(&slot) {
                    Some(current)
                } else {
                    slot.store_check(None, "");
                    return Ok(None);
                }
            };
            Ok(offer_of_held(&app, busy))
        }
        Err(error) => check_failed(manual, error.to_string()),
    }
}

fn snapshot_if_busy(
    slot: &updater::UpdateSlot<tauri_plugin_updater::Update>,
) -> Option<Option<tauri_plugin_updater::Update>> {
    slot.is_busy().then(|| slot.current().cloned())
}

fn offer_of_held(
    app: &AppHandle,
    held: Option<Option<tauri_plugin_updater::Update>>,
) -> Option<updater::UpdateOffer> {
    let update = held.flatten()?;
    let preferences = read_preferences(app).ok();
    let skipped = skipped_flag(preferences.as_ref(), &update.version);
    Some(offer_from(&update, skipped))
}

fn check_failed(manual: bool, error: String) -> Result<Option<updater::UpdateOffer>, String> {
    if manual {
        Err(error)
    } else {
        log::warn!("自動檢查更新失敗：{error}");
        Ok(None)
    }
}

fn read_preferences(app: &AppHandle) -> Result<Map<String, Value>, String> {
    let config = data::read_config(&config_root(app)?).map_err(|error| error.to_string())?;
    Ok(config.preferences)
}

fn skipped_flag(preferences: Option<&Map<String, Value>>, version: &str) -> bool {
    let Some(preferences) = preferences else {
        return false;
    };
    updater::is_skipped(
        preferences
            .get("update_skipped_version")
            .and_then(Value::as_str),
        version,
    )
}

fn offer_from(update: &tauri_plugin_updater::Update, skipped: bool) -> updater::UpdateOffer {
    updater::UpdateOffer {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        notes: update.body.clone(),
        pub_date: update.date.map(|date| date.to_string()),
        level: updater::reminder_level(
            &update.current_version,
            &update.version,
            updater::remote_format(&update.raw_json),
            data::current_format(),
        ),
        skipped,
    }
}

#[tauri::command]
pub(crate) async fn update_download(
    app: AppHandle,
    state: State<'_, updater::PendingUpdate>,
) -> Result<String, String> {
    let update = {
        let mut slot = state.inner.lock().await;
        slot.begin_download()?
    };
    match download_prepared(&app, &update).await {
        Ok(version) => {
            state.inner.lock().await.finish_download(version.clone());
            Ok(version)
        }
        Err(error) => {
            state.inner.lock().await.abort_download();
            Err(error)
        }
    }
}

async fn download_prepared(
    app: &AppHandle,
    update: &tauri_plugin_updater::Update,
) -> Result<String, String> {
    let platform = updater::Platform::current().ok_or_else(|| "這個平台沒有安裝檔".to_owned())?;
    let file = updater::artifact_name(update.download_url.as_str())?;
    if !platform.suffix_ok(&file) {
        return Err("安裝檔副檔名不符合這個平台".to_owned());
    }
    let version = updater::version_dir_name(&update.version)?.to_owned();
    let versions = versions_dir(app)?;
    if updater::reuse_if_valid(
        &versions,
        &version,
        &file,
        platform.as_str(),
        updater::bundled_pubkey(),
    )? {
        return Ok(version);
    }
    updater::clear_version_residue(&versions, &version)?;
    let mut downloaded = 0u64;
    let emit_app = app.clone();
    let bytes = update
        .download(
            |chunk, total| {
                downloaded = downloaded.saturating_add(chunk as u64);
                let _ = emit_app.emit("update-progress", ProgressPayload { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|error| error.to_string())?;
    updater::commit_download(
        &versions,
        &version,
        &file,
        &bytes,
        &update.signature,
        platform.as_str(),
        updater::remote_format(&update.raw_json),
    )?;
    Ok(version)
}

#[tauri::command]
pub(crate) async fn update_install(
    app: AppHandle,
    state: State<'_, updater::PendingUpdate>,
    version: String,
) -> Result<(), String> {
    let update = {
        let mut slot = state.inner.lock().await;
        slot.begin_install(&version)?
    };
    // 只裝剛剛下載、而且版本字串對得上的那一份。對不上就不改階段以外的狀態。
    let outcome = install_pinned(&app, &update, &version).await;
    if outcome.is_err() {
        state.inner.lock().await.abort_install(&version);
    }
    outcome
}

async fn install_pinned(
    app: &AppHandle,
    update: &tauri_plugin_updater::Update,
    version: &str,
) -> Result<(), String> {
    if update.version != version {
        return Err("下載的版本與要安裝的版本不同".to_owned());
    }
    let platform = updater::Platform::current().ok_or_else(|| "這個平台沒有安裝檔".to_owned())?;
    let file = updater::artifact_name(update.download_url.as_str())?;
    if !platform.suffix_ok(&file) {
        return Err("安裝檔副檔名不符合這個平台".to_owned());
    }
    let dir_name = updater::version_dir_name(&update.version)?.to_owned();
    if dir_name != version {
        return Err("下載的版本與要安裝的版本不同".to_owned());
    }
    let versions = versions_dir(app)?;
    let platform_name = platform.as_str().to_owned();
    let update = update.clone();
    let app = app.clone();
    data::install_guarded(
        move || {
            updater::reverify_for_install(
                &versions,
                &dir_name,
                &file,
                &platform_name,
                updater::bundled_pubkey(),
            )
        },
        {
            let app = app.clone();
            move || clear_skipped_if_present(&app)
        },
        {
            let app = app.clone();
            move |bytes: Vec<u8>| install_prepared(&app, &update, &bytes)
        },
    )
    .await
}

/// 重驗通過、開閘之前才清。鍵不存在就不重寫。清掉之後安裝若失敗，略過不會自動寫回來。
/// 設定鎖這時還沒被安裝持有。前端在安裝失敗時再讀一次設定，才看得到清掉的鍵。
fn clear_skipped_if_present(app: &AppHandle) -> Result<(), String> {
    let root = config_root(app)?;
    let config = data::read_config(&root).map_err(|error| error.to_string())?;
    if !config.preferences.contains_key("update_skipped_version") {
        return Ok(());
    }
    data::update_config(
        &root,
        &serde_json::json!({ "preferences": { "update_skipped_version": Value::Null } }),
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn versions_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let root = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?;
    Ok(updater::versions_root(&root))
}

fn install_prepared(
    app: &AppHandle,
    update: &tauri_plugin_updater::Update,
    bytes: &[u8],
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let _ = app;
        update.install(bytes).map_err(|error| error.to_string())?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let bundle = tauri_plugin_updater::extract_path_from_executable(&executable)
            .map_err(|error| error.to_string())?;
        updater::replace_installed_app(
            &bundle,
            bytes,
            &update.version,
            updater::parent_is_writable,
            updater::swap_directories,
        )?;
        // 對調已經成功。restart 是 `!`，不會回到這裡放閘。
        app.restart();
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (app, update, bytes);
        Err(updater::CANNOT_REPLACE.to_owned())
    }
}

#[tauri::command]
pub(crate) fn update_post_launch() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let bundle = tauri_plugin_updater::extract_path_from_executable(&executable)
            .map_err(|error| error.to_string())?;
        updater::cleanup_after_launch(&bundle)?;
    }
    Ok(())
}
