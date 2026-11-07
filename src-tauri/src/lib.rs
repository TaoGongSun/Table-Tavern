mod card_sandbox;
mod chat_assembly;
mod chat_turn;
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
mod mvu_engine;
mod openrouter_oauth;
mod receipts;
mod refactor;
mod refactor_ai;
mod refactor_assemble;
mod room;
mod scene_budget;
mod smart_free;
mod st_macros;
mod transport;
mod ui_msg;
mod updater;
mod usage;
mod vn_audio;
mod vn_backgrounds;
mod vn_bible;
mod vn_branches;
mod vn_sprites;
mod vn_work;
mod world_info;
mod world_scan;

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
        .register_uri_scheme_protocol(card_sandbox::SCHEME, |_ctx, request| {
            card_sandbox::respond(request.uri().path())
        })
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
            #[cfg(feature = "test-harness")]
            harness::probe::harness_probe,
            #[cfg(feature = "test-harness")]
            harness::probe::harness_probe_count,
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
            commands::character::character_card_gone,
            commands::character::reorder_characters,
            commands::character::read_character,
            commands::character::write_character,
            commands::character::set_character_archived,
            commands::character::set_character_auto_hidden,
            commands::character::set_character_stage,
            commands::character::delete_character,
            commands::character::probe_import,
            commands::character::check_character_image,
            commands::refactor::card_interfaces,
            commands::character::import_character,
            commands::character::import_web_save,
            commands::vn_card::import_vn_card,
            commands::vn_card::export_vn_card,
            commands::vn_work::export_vn_work,
            commands::vn_work::probe_vn_work,
            commands::vn_work::import_vn_work,
            commands::vn_work::vn_replay_goto,
            commands::character::confirm_web_save_import,
            commands::character::discard_web_save_import,
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
            commands::refactor::gm_table_mode,
            commands::refactor::refactor_table_mode,
            commands::refactor::refactor_export_outcome,
            commands::refactor::refactor_export_saved,
            commands::refactor::refactor_card_open,
            commands::refactor::refactor_stage_cards,
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
            commands::card_vars::card_vars_parse,
            commands::card_vars::set_player_card,
            commands::scene::append_player_event,
            commands::scene::discard_unanswered_player,
            commands::scene::post_opening,
            commands::scene::translate_opening,
            commands::scene::translate_tier_models,
            commands::scene::read_transcript,
            commands::scene::scene_appearances,
            commands::scene::pop_transcript_floor,
            commands::scene::restore_transcript_events,
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
            commands::cli_update::update_cli,
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
            commands::room::room_status,
            commands::room::room_enter,
            commands::room::room_exit,
            commands::room::room_pair_code,
            commands::room::room_tables,
            commands::room::room_close,
            commands::room::room_identity,
            commands::room::room_image_gen,
            commands::room::room_roster_refresh,
            commands::room::room_member_open,
            commands::room::room_member_send,
            commands::room::room_member_close,
            commands::vn::read_vn_cells,
            commands::vn::parse_vn_text,
            commands::vn::vn_reroll_choices,
            commands::vn::vn_endings,
            commands::vn::vn_replay_chapter,
            commands::vn::vn_fork_choice,
            commands::vn::vn_switch_branch,
            commands::vn::vn_choice_branches,
            commands::vn::vn_progress_read,
            commands::vn::vn_progress_write,
            commands::vn::vn_save_create,
            commands::vn::vn_save_delete,
            commands::vn::vn_save_load,
            commands::vn::vn_cover,
            commands::vn::vn_new_playthrough,
            commands::vn_bible::vn_bible_read,
            commands::vn_bible::vn_bible_generate,
            commands::vn_bible::vn_bible_save,
            commands::vn_bible::vn_spoiler_set,
            commands::vn_bible::vn_secret_reroll,
            commands::vn_audio::vn_audio_fill,
            commands::vn_audio::vn_audio_prefetch,
            commands::vn_audio::vn_audio_read,
            commands::vn_backgrounds::vn_backgrounds_fill,
            commands::vn_backgrounds::vn_background_read,
            commands::vn_backgrounds::vn_backgrounds_status,
            commands::vn_backgrounds::vn_background_regenerate,
            commands::vn_backgrounds::vn_backgrounds_prefetch,
            commands::vn_backgrounds::vn_background_generate_period,
            commands::vn_backgrounds::vn_backgrounds_list,
            commands::vn_backgrounds::vn_background_upload,
            commands::vn_backgrounds::vn_background_cancel,
            commands::vn_sprites::vn_sprites_read_all,
            commands::vn_sprites::vn_sprite_read,
            commands::vn_sprites::vn_sprite_candidate,
            commands::vn_sprites::vn_sprite_generate,
            commands::vn_sprites::vn_sprite_pick,
            commands::vn_sprites::vn_sprite_upload,
            commands::vn_sprites::vn_sprite_set_face,
            commands::vn_sprites::vn_sprite_reroll,
            commands::vn_sprites::vn_sprite_delete,
            commands::vn_sprites::vn_sprites_fill,
            commands::vn_sprites::vn_sprites_prepare,
            commands::vn_sprites::vn_sprites_status,
            commands::vn_sprites::vn_sprite_variant_generate,
            commands::vn_sprites::vn_sprite_variant_upload,
            commands::vn_sprites::vn_sprite_variant_delete,
            commands::settings::usage_report,
            commands::scene::advance_scene,
            commands::scene::scene_budget,
            commands::scene::revert_scene,
            commands::scene::reset_world_info_timing,
            commands::scene::world_info_notices,
            commands::scene::ack_world_info_notice,
            commands::scene::fork_scene,
            commands::scene::regenerate_scene_summary,
            commands::genesis::generate_table_outline,
            commands::genesis::generate_table_character,
            commands::genesis::generate_table_expand,
            commands::genesis::vn_setup_set,
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
            // CLI 更新的啟動復原：同步立起有標記那幾家的擋板，再背景處理；排在家目錄回收之前，
            // 回收（codex delete 會經過 spawn）在擋板立著時拿到 CliUpdating，不會搶在復原前面
            if let Ok(data) = data_root(app.handle()) {
                commands::cli_update::spawn_startup_recovery(app.handle(), data);
            }
            // app 專用 CLI 家目錄的啟動回收（agy 對話、grok prompt 歷史與搜尋索引）
            if let (Ok(config), Ok(data)) = (config_root(app.handle()), data_root(app.handle())) {
                cli::home_cleanup::spawn_startup_sweep(config, data);
            }
            // 房間模式：上次沒按退出就自動接回同一房間（multiplayer-room 包 2）
            if let Ok(engine) = room::AppEngine::shared(app.handle()) {
                tauri::async_runtime::spawn(room::resume(engine));
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
                // 房間：盡力送「房主暫停」再斷，送不到就算；本機視同關房（下次開不自動接回）
                tauri::async_runtime::block_on(async {
                    let _ =
                        tokio::time::timeout(std::time::Duration::from_secs(3), room::exit()).await;
                });
                inflight::kill_all_children();
                #[cfg(feature = "test-harness")]
                harness::shutdown();
            }
        });
}

#[cfg(test)]
mod test_fake_cli;
#[cfg(all(test, unix))]
mod test_perm;

#[cfg(test)]
mod command_classification {
    /// generate_handler 裡每一個 command 都要落在這三類之一。改清單時兩邊一起改。
    const WRITE_WORLD: &[&str] = &[
        "advance_scene",
        "append_player_event",
        "append_transcript",
        "card_layer_write",
        "card_vars_write",
        "confirm_web_save_import",
        "discard_unanswered_player",
        "discard_web_save_import",
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
        "import_web_save",
        "import_vn_card",
        "import_vn_work",
        "import_worldbook",
        "list_worlds",
        "mark_state_counter",
        "open_world",
        "pop_transcript_floor",
        "post_opening",
        "reclaim_world_if_empty",
        "record_import_rename",
        "refactor_apply",
        "refactor_reset_to_source",
        "regenerate_scene_summary",
        "rename_world",
        "reorder_characters",
        "reorder_worldbook_entries",
        "reset_world_info_timing",
        "ack_world_info_notice",
        "restore_transcript_events",
        "restore_world_backup",
        "revert_scene",
        "save_character_avatar",
        "save_character_image",
        "set_branch_binding",
        "set_player_card",
        "set_character_archived",
        "set_character_auto_hidden",
        "set_character_stage",
        "set_state_path",
        "set_table_state",
        "undo_last_import",
        "upsert_worldbook_entry",
        "vn_reroll_choices",
        "vn_replay_chapter",
        "vn_fork_choice",
        "vn_switch_branch",
        "vn_progress_write",
        "vn_save_create",
        "vn_save_delete",
        "vn_save_load",
        "vn_new_playthrough",
        "vn_replay_goto",
        "vn_bible_generate",
        "vn_setup_set",
        "vn_bible_save",
        "vn_spoiler_set",
        "vn_secret_reroll",
        "vn_background_regenerate",
        "vn_audio_fill",
        "vn_audio_prefetch",
        "vn_backgrounds_fill",
        "vn_backgrounds_prefetch",
        "vn_background_generate_period",
        "vn_background_upload",
        "vn_background_cancel",
        "vn_sprite_delete",
        "vn_sprite_generate",
        "vn_sprite_pick",
        "vn_sprite_reroll",
        "vn_sprite_set_face",
        "vn_sprite_upload",
        "vn_sprite_variant_delete",
        "vn_sprite_variant_generate",
        "vn_sprite_variant_upload",
        "vn_sprites_fill",
        "vn_sprites_prepare",
        "worldbook_entry_to_character",
        "write_character",
        "write_state",
        "write_world_md",
    ];

    const READ_WORLD: &[&str] = &[
        "world_info_notices",
        "branch_bindings",
        "card_interfaces",
        "card_layers",
        "card_vars_parse",
        "card_vars_state",
        "card_openings",
        "export_character",
        "export_scene",
        "export_transcript",
        "export_vn_card",
        "export_vn_work",
        "export_worldbook",
        "gm_table_mode",
        "list_characters",
        "character_card_gone",
        "list_gallery_images",
        "list_world_backups",
        "list_import_receipts",
        "mechanism_ledger",
        "parse_vn_text",
        "read_character",
        "read_character_avatar",
        "read_character_image",
        "read_gallery_image",
        "read_gm_image",
        "read_state",
        "read_transcript",
        "read_vn_cells",
        "vn_endings",
        "vn_choice_branches",
        "vn_progress_read",
        "vn_cover",
        "vn_bible_read",
        "vn_sprite_candidate",
        "vn_sprite_read",
        "vn_sprites_read_all",
        "vn_sprites_status",
        "vn_audio_read",
        "vn_background_read",
        "vn_backgrounds_status",
        "vn_backgrounds_list",
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
        "refactor_stage_cards",
        "refactor_survey",
        "refactor_table_mode",
        "room_enter",
        "room_exit",
        "scene_appearances",
        "scene_budget",
        "world_has_state_bar",
    ];

    const NOT_WORLD: &[&str] = &[
        "harness_probe",
        "harness_probe_count",
        "chat_abort",
        "check_character_image",
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
        "probe_vn_work",
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
        "room_close",
        "room_identity",
        "room_image_gen",
        "room_member_close",
        "room_member_open",
        "room_member_send",
        "room_pair_code",
        "room_roster_refresh",
        "room_status",
        "room_tables",
        "update_check",
        "update_cli",
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
