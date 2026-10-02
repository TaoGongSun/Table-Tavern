//! 版本庫與桌備份的 Tauri 邊界。規則在 `updater` 與 `data`。

use tauri::AppHandle;

use crate::ui_msg::UiMsg;
use crate::{config_root, data, data_root, updater};

use super::update::versions_dir;

#[tauri::command]
pub(crate) async fn list_versions(app: AppHandle) -> Result<updater::VersionList, String> {
    let platform =
        updater::Platform::current().ok_or_else(|| UiMsg::PlatformUnsupported.to_string())?;
    updater::list_versions_locked(
        &versions_dir(&app)?,
        platform,
        env!("CARGO_PKG_VERSION"),
        updater::bundled_pubkey(),
    )
    .await
}

#[tauri::command]
pub(crate) async fn delete_version(app: AppHandle, version: String) -> Result<(), String> {
    updater::delete_version_locked(&versions_dir(&app)?, &version, env!("CARGO_PKG_VERSION")).await
}

#[tauri::command]
pub(crate) async fn rollback_preview(
    app: AppHandle,
    version: String,
) -> Result<updater::RollbackPreview, String> {
    let platform =
        updater::Platform::current().ok_or_else(|| UiMsg::PlatformUnsupported.to_string())?;
    updater::rollback_preview_locked(
        &data_root(&app)?,
        &versions_dir(&app)?,
        &version,
        platform,
        env!("CARGO_PKG_VERSION"),
        updater::bundled_pubkey(),
    )
    .await
}

#[tauri::command]
pub(crate) async fn rollback_install(app: AppHandle, version: String) -> Result<(), String> {
    let platform =
        updater::Platform::current().ok_or_else(|| UiMsg::PlatformUnsupported.to_string())?;
    let versions = versions_dir(&app)?;
    let config = config_root(&app)?;
    let running = env!("CARGO_PKG_VERSION").to_owned();
    let version_for_prepare = version.clone();
    let version_for_install = version.clone();
    let running_for_prepare = running.clone();
    let running_for_skip = running;
    updater::with_installing(&version, async move {
        data::install_guarded(
            move || {
                let eligible = updater::require_eligible(
                    &versions,
                    &version_for_prepare,
                    platform,
                    &running_for_prepare,
                )?;
                let bytes = updater::reverify_for_install(
                    &versions,
                    &version_for_prepare,
                    &eligible.file,
                    platform.as_str(),
                    updater::bundled_pubkey(),
                )?;
                Ok((bytes, eligible.file))
            },
            move || updater::rollback_before_raise(&config, &running_for_skip),
            move |(bytes, file): (Vec<u8>, String)| {
                install_rollback(&app, &version_for_install, &file, &bytes)
            },
        )
        .await
    })
    .await
}

fn install_rollback(
    app: &AppHandle,
    version: &str,
    file: &str,
    bytes: &[u8],
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let dest = updater::stage_launch_installer(&versions_dir(app)?, file, bytes)?;
        updater::start_nsis_installer(&dest)?;
        let _ = version;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        let _ = file;
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let bundle = tauri_plugin_updater::extract_path_from_executable(&executable)
            .map_err(|error| error.to_string())?;
        updater::replace_installed_app(
            &bundle,
            bytes,
            version,
            updater::parent_is_writable,
            updater::swap_directories,
        )?;
        app.restart();
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (app, version, file, bytes);
        Err(UiMsg::UpdateCannotReplace.into())
    }
}

#[tauri::command]
pub(crate) fn list_world_backups(app: AppHandle) -> Result<data::BackupList, String> {
    data::list_world_backups(&data_root(&app)?)
}

#[tauri::command]
pub(crate) fn delete_world_backup(
    app: AppHandle,
    world_id: String,
    kind: String,
) -> Result<(), String> {
    data::delete_world_backup(&data_root(&app)?, &world_id, &kind)
}
