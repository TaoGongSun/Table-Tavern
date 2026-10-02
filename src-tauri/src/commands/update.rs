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
) -> Result<updater::CheckResult, String> {
    // 下載或安裝進行中不打網路，也不覆寫槽裡那一份。回目前的結果，前端才不會把進度洗掉。
    if let Some(held) = {
        let slot = state.inner.lock().await;
        snapshot_if_busy(&slot)
    } {
        return Ok(held_result(&app, held));
    }

    let auto_preferences = if manual {
        None
    } else {
        match read_preferences(&app) {
            Ok(preferences) => Some(preferences),
            Err(error) => return Ok(updater::CheckResult::failed(false, error)),
        }
    };
    if !manual && !updater::allow_auto_check(auto_preferences.as_ref()) {
        return Ok(updater::CheckResult::Failed {
            message: "自動檢查已關閉".to_owned(),
        });
    }

    let plugin = match app.updater() {
        Ok(plugin) => plugin,
        Err(error) => return Ok(updater::CheckResult::failed(manual, error.to_string())),
    };
    // 失敗不碰待裝槽：前端保留原本的更新資訊，槽裡那一份也留著。
    let found = match plugin.check().await {
        Ok(found) => found,
        Err(error) => return Ok(updater::CheckResult::failed(manual, error.to_string())),
    };
    let preferences = if manual {
        read_preferences(&app).ok()
    } else {
        auto_preferences
    };
    let busy = {
        let mut slot = state.inner.lock().await;
        if let Some(current) = snapshot_if_busy(&slot) {
            current
        } else {
            return Ok(match found {
                Some(update) => {
                    let version = update.version.clone();
                    let skipped = skipped_flag(preferences.as_ref(), &version);
                    let offer = offer_from(&update, skipped);
                    slot.store_check(Some(update), &version);
                    updater::CheckResult::Available { offer }
                }
                None => {
                    slot.store_check(None, "");
                    updater::CheckResult::None
                }
            });
        }
    };
    Ok(held_result(&app, busy))
}

fn snapshot_if_busy(
    slot: &updater::UpdateSlot<tauri_plugin_updater::Update>,
) -> Option<Option<tauri_plugin_updater::Update>> {
    slot.is_busy().then(|| slot.current().cloned())
}

fn held_result(
    app: &AppHandle,
    held: Option<tauri_plugin_updater::Update>,
) -> updater::CheckResult {
    let offer = held.map(|update| {
        let preferences = read_preferences(app).ok();
        let skipped = skipped_flag(preferences.as_ref(), &update.version);
        offer_from(&update, skipped)
    });
    updater::CheckResult::from_held(offer)
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
    version: String,
) -> Result<updater::DownloadResult, String> {
    let update = {
        let mut slot = state.inner.lock().await;
        slot.begin_download(&version, |update: &tauri_plugin_updater::Update| {
            update.version.as_str()
        })?
    };
    if let Err(error) = updater::mark_downloading(&update.version).await {
        state.inner.lock().await.abort_download();
        return Err(error);
    }
    let outcome = download_prepared(&app, &update).await;
    updater::clear_downloading().await;
    match outcome {
        Ok(result) => {
            state
                .inner
                .lock()
                .await
                .finish_download(result.version.clone());
            Ok(result)
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
) -> Result<updater::DownloadResult, String> {
    let platform = updater::Platform::current().ok_or_else(|| "這個平台沒有安裝檔".to_owned())?;
    let file = updater::artifact_name(update.download_url.as_str())?;
    if !platform.suffix_ok(&file) {
        return Err("安裝檔副檔名不符合這個平台".to_owned());
    }
    let version = updater::version_dir_name(&update.version)?.to_owned();
    let versions = versions_dir(app)?;
    let pubkey = updater::bundled_pubkey();
    let reused =
        updater::prepare_download_reuse(&versions, &version, &file, platform.as_str(), pubkey)
            .await?;
    let staged = if reused {
        None
    } else {
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
        Some((bytes, update.signature.clone()))
    };
    let endpoint = updater::rollback_point_endpoint();
    let new_release = staged
        .as_ref()
        .map(|(bytes, signature)| updater::NewRelease {
            file: &file,
            bytes,
            signature,
            format_version: updater::remote_format(&update.raw_json),
        });
    let rollback_ready = updater::store_downloaded_release(
        &versions,
        &version,
        platform,
        new_release,
        env!("CARGO_PKG_VERSION"),
        pubkey,
        data::CURRENT_FORMAT,
        &endpoint,
        fetch_release_asset,
    )
    .await?;
    Ok(updater::DownloadResult {
        version,
        rollback_ready,
    })
}

async fn fetch_release_asset(url: String) -> Result<Vec<u8>, String> {
    if !url.starts_with("https://") {
        return Err("回退點下載失敗".to_owned());
    }
    let response = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| error.to_string())?
        .get(&url)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("回退點下載失敗：{}", response.status()));
    }
    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|error| error.to_string())
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
    let outcome = updater::with_installing(&version, install_pinned(&app, &update, &version)).await;
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
    let pending_versions = versions.clone();
    let platform_name = platform.as_str().to_owned();
    let update = update.clone();
    let installing = app.clone();
    let config = config_root(app)?;
    let target = version.to_owned();
    // 略過鍵在重驗通過、開閘前清掉。安裝失敗時前端重讀設定，才看得到清掉的鍵。
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
        move || {
            updater::forward_before_raise(
                &pending_versions,
                &config,
                env!("CARGO_PKG_VERSION"),
                &target,
            )
        },
        move |bytes: Vec<u8>| install_prepared(&installing, &update, &bytes),
    )
    .await
}

pub(crate) fn versions_dir(app: &AppHandle) -> Result<PathBuf, String> {
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
pub(crate) async fn update_post_launch(app: AppHandle) -> Result<(), String> {
    let versions = versions_dir(&app)?;
    let pubkey = updater::bundled_pubkey().to_owned();
    let platform = updater::Platform::current();
    let bundle = mac_bundle();
    updater::settle_launch_locked(
        &versions,
        env!("CARGO_PKG_VERSION"),
        bundle.as_deref(),
        |version| {
            let Some(platform) = platform else {
                return false;
            };
            updater::has_verified_rollback_point(&versions, version, platform, &pubkey)
        },
        swap_for_launch,
    )
    .await
}

fn mac_bundle() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let executable = match std::env::current_exe() {
            Ok(path) => path,
            Err(error) => {
                log::warn!("找不到執行檔，略過啟動後整理：{error}");
                return None;
            }
        };
        match tauri_plugin_updater::extract_path_from_executable(&executable) {
            Ok(path) => Some(path),
            Err(error) => {
                log::warn!("找不到 App，略過啟動後整理：{error}");
                None
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

fn swap_for_launch(new_app: &std::path::Path, old_app: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        updater::swap_directories(new_app, old_app)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (new_app, old_app);
        Err(updater::CANNOT_REPLACE.to_owned())
    }
}
