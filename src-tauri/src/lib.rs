mod ai_transport;
mod cli;
mod commands;
mod data;
mod ejs;
mod evaluator;
mod genesis;
mod import;
#[allow(dead_code)]
mod install;
mod inflight;
mod lanes;
mod mechanism;
mod proxy;
mod receipts;
mod responses_transport;
mod refactor;
mod refactor_ai;
mod refactor_assemble;
mod refactor_session;
mod session_file;
mod snapshot_patch;
mod translate;
mod transport;
mod usage_log;
mod usage_report;

use std::path::PathBuf;
use tauri::Manager;

fn data_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .document_dir()
        .map(|path| path.join("TableTavern"))
        .map_err(|error| error.to_string())
}

fn config_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .config_dir()
        .map(|path| path.join("TableTavern"))
        .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
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
            commands::refactor::refactor_outcome_exists,
            commands::character::export_character,
            commands::image::read_character_image,
            commands::image::save_character_image,
            commands::image::delete_character_image,
            commands::image::read_character_avatar,
            commands::image::save_character_avatar,
            commands::image::delete_character_avatar,
            commands::image::read_gm_image,
            commands::scene::append_transcript,
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
            commands::settings::write_config,
            commands::settings::detect_clis,
            commands::cli_setup::install_cli,
            commands::cli_setup::cli_verified,
            commands::settings::sponsor_status,
            commands::settings::import_sponsor_pack,
            commands::settings::list_cli_models,
            commands::settings::read_model_catalog,
            commands::settings::write_model_catalog,
            commands::chat::chat_with_character,
            commands::image::generate_character_image,
            commands::image::list_gallery_images,
            commands::image::read_gallery_image,
            commands::image::delete_gallery_image,
            commands::chat::gm_narrate,
            commands::chat::keepalive_lanes,
            commands::settings::usage_report,
            commands::scene::advance_scene,
            commands::scene::revert_scene,
            commands::scene::fork_scene,
            commands::scene::regenerate_scene_summary,
            commands::genesis::generate_table_outline,
            commands::genesis::generate_table_character,
            commands::genesis::generate_table_expand
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_handle, event| {
            // app 退出：殺全部在途 CLI 子程序，避免孤兒繼續跑、繼續燒錢。
            if let tauri::RunEvent::Exit = event {
                inflight::kill_all_children();
            }
        });
}
