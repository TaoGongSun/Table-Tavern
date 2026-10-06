mod chat_assembly;
mod cli;
mod commands;
mod data;
mod evaluator;
mod generated_image;
mod genesis;
#[cfg(feature = "test-harness")]
mod harness;
mod import;
mod inflight;
mod lanes;
mod mechanism;
mod openrouter_oauth;
mod receipts;
mod refactor;
mod refactor_ai;
mod refactor_assemble;
mod scene_budget;
mod smart_free;
mod transport;
mod ui_msg;
mod updater;
mod usage;

use std::path::PathBuf;
use tauri::Manager;

/// 測試包改指向 `TT_HARNESS_ROOT` 底下，絕不碰正式資料。
#[cfg(feature = "test-harness")]
fn data_root(_app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(harness::data_root())
}

#[cfg(not(feature = "test-harness"))]
fn data_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .document_dir()
        .map(|path| path.join("TableTavern"))
        .map_err(|error| error.to_string())
}

/// 測試包改指向 `TT_HARNESS_ROOT` 底下，絕不碰正式資料。
#[cfg(feature = "test-harness")]
fn config_root(_app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(harness::config_root())
}

#[cfg(not(feature = "test-harness"))]
fn config_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .config_dir()
        .map(|path| path.join("TableTavern"))
        .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let context = tauri::generate_context!();
    // 測試包：建 Tauri 前先驗 identifier／root、取鎖，不合就結束 process。
    #[cfg(feature = "test-harness")]
    harness::boot(&context.config().identifier);
    let builder = tauri::Builder::default().plugin(tauri_plugin_opener::init());
    #[cfg(not(feature = "test-harness"))]
    let builder = builder.plugin(tauri_plugin_dialog::init());
    // 測試包以同名假 plugin 接手原生對話窗，交給控制埠回答。
    #[cfg(feature = "test-harness")]
    let builder = builder.plugin(harness::dialog::init());
    #[cfg(feature = "test-harness")]
    let builder = builder.plugin(harness::motion::init());
    builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updater::PendingUpdate::default())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 第二次啟動只把既有視窗帶到前面。不做每桌鎖檔。
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![
            commands::world::list_worlds,
            commands::world::create_world,
            commands::world::create_sample_world,
            commands::world::new_id,
            commands::world::reclaim_world_if_empty,
            commands::world::rename_world,
            commands::world::delete_world,
            commands::world::read_world_md,
            commands::world::write_world_md,
            commands::world::read_worldbook,
            commands::world::upsert_worldbook_entry,
            commands::world::reorder_worldbook_entries,
            commands::world::delete_worldbook_entry,
            commands::state::mechanism_ledger,
            commands::world::worldbook_entry_to_character,
            commands::world::character_to_worldbook_entry,
            commands::world::world_has_state_bar,
            commands::world::card_openings,
            commands::world::import_worldbook,
            commands::world::dedupe_worldbook,
            commands::world::export_worldbook,
            commands::world::open_world,
            commands::world::read_world_readonly,
            commands::world::restore_world_backup,
            commands::character::list_characters,
            commands::character::reorder_characters,
            commands::character::read_character,
            commands::character::write_character,
            commands::character::set_character_archived,
            commands::character::set_character_auto_hidden,
            commands::character::delete_character,
            commands::character::probe_import,
            commands::refactor::card_interfaces,
            commands::character::import_character,
            commands::character::list_import_receipts,
            commands::character::undo_last_import,
            commands::character::record_import_rename,
            commands::refactor::refactor_apply,
            commands::refactor::refactor_recommend,
            commands::refactor::refactor_survey,
            commands::refactor::refactor_assemble_local,
            commands::refactor::refactor_expand,
            commands::refactor::refactor_expand_person,
            commands::refactor::refactor_expand_spans,
            commands::refactor::refactor_absorb_entry,
            commands::refactor::refactor_split_group,
            commands::refactor::refactor_abort,
            commands::refactor::refactor_interface_shell,
            commands::refactor::refactor_table_mode,
            commands::refactor::refactor_export_outcome,
            commands::refactor::refactor_export_saved,
            commands::refactor::refactor_card_open,
            commands::refactor::refactor_card_release,
            commands::refactor::refactor_rerun_status,
            commands::refactor::refactor_reset_to_source,
            commands::character::export_character,
            commands::image::read_character_image,
            commands::image::save_character_image,
            commands::image::delete_character_image,
            commands::image::read_character_avatar,
            commands::image::save_character_avatar,
            commands::image::delete_character_avatar,
            commands::image::read_gm_image,
            commands::scene::append_transcript,
            commands::card_vars::card_layer_write,
            commands::card_vars::card_layers,
            commands::card_vars::card_vars_state,
            commands::card_vars::card_vars_write,
            commands::card_vars::set_player_card,
            commands::scene::append_player_event,
            commands::scene::discard_unanswered_player,
            commands::scene::post_opening,
            commands::scene::translate_opening,
            commands::scene::translate_tier_models,
            commands::scene::read_transcript,
            commands::scene::scene_appearances,
            commands::scene::pop_transcript,
            commands::scene::export_transcript,
            commands::scene::export_scene,
            commands::state::read_state,
            commands::state::write_state,
            commands::state::set_table_state,
            commands::state::set_state_path,
            commands::state::set_branch_binding,
            commands::state::branch_bindings,
            commands::state::mark_state_counter,
            commands::settings::read_config,
            commands::settings::update_config,
            commands::openrouter_key::openrouter_key_tier,
            openrouter_oauth::save_openrouter_key,
            openrouter_oauth::connect_openrouter,
            commands::settings::detect_clis,
            commands::cli_setup::install_cli,
            commands::cli_setup::cli_verified,
            commands::settings::sponsor_status,
            commands::settings::import_sponsor_pack,
            commands::settings::list_cli_models,
            commands::settings::read_model_catalog,
            commands::settings::write_model_catalog,
            commands::settings::smart_free_status,
            commands::settings::smart_free_recommendations,
            commands::settings::smart_free_new_models,
            commands::settings::smart_free_dismiss_recommendations,
            commands::chat::chat_with_character,
            commands::image::generate_character_image,
            commands::image::list_gallery_images,
            commands::image::read_gallery_image,
            commands::image::delete_gallery_image,
            commands::chat::gm_narrate,
            commands::chat::chat_abort,
            commands::chat::keepalive_lanes,
            commands::settings::usage_report,
            commands::scene::advance_scene,
            commands::scene::scene_budget,
            commands::scene::revert_scene,
            commands::scene::fork_scene,
            commands::scene::regenerate_scene_summary,
            commands::genesis::generate_table_outline,
            commands::genesis::generate_table_character,
            commands::genesis::generate_table_expand,
            commands::update::update_check,
            commands::update::update_download,
            commands::update::update_install,
            commands::update::update_post_launch,
            commands::versions::list_versions,
            commands::versions::delete_version,
            commands::versions::rollback_preview,
            commands::versions::rollback_install,
            commands::versions::list_world_backups,
            commands::versions::delete_world_backup
        ])
        .setup(|app| {
            match app.path().app_local_data_dir() {
                Ok(dir) => {
                    if let Err(error) = updater::clear_all_residue(&dir.join("versions")) {
                        log::warn!("清版本庫殘留失敗：{error}");
                    }
                }
                Err(error) => log::warn!("找不到本機資料目錄，略過版本庫殘留清理：{error}"),
            }
            if let Ok(root) = config_root(app.handle()) {
                smart_free::spawn_background_refresh(app.handle().clone(), root);
            }
            #[cfg(feature = "test-harness")]
            harness::start(app.handle());
            Ok(())
        })
        .build(context)
        .expect("error while building tauri application")
        .run(|_handle, event| {
            // app 退出：殺全部在途 CLI 子程序，避免孤兒繼續跑、繼續燒錢。
            if let tauri::RunEvent::Exit = event {
                inflight::kill_all_children();
                #[cfg(feature = "test-harness")]
                harness::shutdown();
            }
        });
}

#[cfg(test)]
mod command_classification {
    /// generate_handler 裡每一個 command 都要落在這三類之一。改清單時兩邊一起改。
    const WRITE_WORLD: &[&str] = &[
        "advance_scene",
        "append_player_event",
        "append_transcript",
        "card_layer_write",
        "card_vars_write",
        "discard_unanswered_player",
        "character_to_worldbook_entry",
        "chat_with_character",
        "create_sample_world",
        "create_world",
        "dedupe_worldbook",
        "delete_character",
        "delete_character_avatar",
        "delete_character_image",
        "delete_gallery_image",
        "delete_world",
        "delete_world_backup",
        "delete_worldbook_entry",
        "fork_scene",
        "generate_character_image",
        "generate_table_expand",
        "gm_narrate",
        "import_character",
        "import_worldbook",
        "keepalive_lanes",
        "list_worlds",
        "mark_state_counter",
        "open_world",
        "pop_transcript",
        "post_opening",
        "reclaim_world_if_empty",
        "record_import_rename",
        "refactor_apply",
        "refactor_reset_to_source",
        "regenerate_scene_summary",
        "rename_world",
        "reorder_characters",
        "reorder_worldbook_entries",
        "restore_world_backup",
        "revert_scene",
        "save_character_avatar",
        "save_character_image",
        "set_branch_binding",
        "set_player_card",
        "set_character_archived",
        "set_character_auto_hidden",
        "set_state_path",
        "set_table_state",
        "undo_last_import",
        "upsert_worldbook_entry",
        "worldbook_entry_to_character",
        "write_character",
        "write_state",
        "write_world_md",
    ];

    const READ_WORLD: &[&str] = &[
        "branch_bindings",
        "card_interfaces",
        "card_layers",
        "card_vars_state",
        "card_openings",
        "export_character",
        "export_scene",
        "export_transcript",
        "export_worldbook",
        "list_characters",
        "list_gallery_images",
        "list_world_backups",
        "list_import_receipts",
        "mechanism_ledger",
        "read_character",
        "read_character_avatar",
        "read_character_image",
        "read_gallery_image",
        "read_gm_image",
        "read_state",
        "read_transcript",
        "read_world_md",
        "rollback_preview",
        "read_world_readonly",
        "read_worldbook",
        "refactor_absorb_entry",
        "refactor_assemble_local",
        "refactor_expand",
        "refactor_expand_person",
        "refactor_expand_spans",
        "refactor_export_outcome",
        "refactor_export_saved",
        "refactor_interface_shell",
        "refactor_rerun_status",
        "refactor_recommend",
        "refactor_split_group",
        "refactor_survey",
        "refactor_table_mode",
        "scene_appearances",
        "scene_budget",
        "world_has_state_bar",
    ];

    const NOT_WORLD: &[&str] = &[
        "chat_abort",
        "cli_verified",
        "connect_openrouter",
        "detect_clis",
        "generate_table_character",
        "generate_table_outline",
        "import_sponsor_pack",
        "install_cli",
        "list_cli_models",
        "new_id",
        "openrouter_key_tier",
        "probe_import",
        "read_config",
        "read_model_catalog",
        "refactor_abort",
        "refactor_card_open",
        "refactor_card_release",
        "save_openrouter_key",
        "smart_free_dismiss_recommendations",
        "smart_free_new_models",
        "smart_free_recommendations",
        "smart_free_status",
        "sponsor_status",
        "translate_opening",
        "translate_tier_models",
        "delete_version",
        "list_versions",
        "rollback_install",
        "update_check",
        "update_config",
        "update_download",
        "update_install",
        "update_post_launch",
        "usage_report",
        "write_model_catalog",
    ];

    #[test]
    fn every_generate_handler_command_is_classified() {
        let source = include_str!("lib.rs");
        let start = source.find("generate_handler![").expect("handler");
        let body = &source[start..];
        let end = body.find("])").expect("handler end");
        let mut found = Vec::new();
        for line in body[..end].lines() {
            let line = line.trim().trim_end_matches(',');
            let Some(name) = line.rsplit("::").next() else {
                continue;
            };
            if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                continue;
            }
            found.push(name);
        }
        found.sort_unstable();
        let mut expected = Vec::new();
        expected.extend_from_slice(WRITE_WORLD);
        expected.extend_from_slice(READ_WORLD);
        expected.extend_from_slice(NOT_WORLD);
        expected.sort_unstable();
        assert_eq!(found, expected, "分類表與 generate_handler 不一致");
        assert!(!found.contains(&"write_config"));
        assert_eq!(
            WRITE_WORLD.len() + READ_WORLD.len() + NOT_WORLD.len(),
            found.len()
        );
    }
}
