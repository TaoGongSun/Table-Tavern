mod cli;
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
mod refactor;
mod refactor_ai;
mod refactor_assemble;
mod session_file;
mod snapshot_patch;
mod translate;
mod transport;
mod usage_log;
mod usage_report;

use data::{AppConfig, CharacterCard, CharacterMeta, TranscriptEvent, WorldState};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[cfg(not(target_os = "windows"))]
use std::process::Command;
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

/// CLI 的工作目錄。Finder 啟動的 macOS app 工作目錄可能是根目錄；CLI 若繼承後做專案探索，
/// 會掃到桌面、下載項目等受 TCC 保護的位置。固定在專用空目錄，避免無關權限彈窗。
fn cli_workspace(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let workspace = config_root(app)?.join("cli-workspace");
    std::fs::create_dir_all(&workspace)
        .map_err(|error| format!("無法準備 CLI 工作目錄：{error}"))?;
    Ok(workspace)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallMessages {
    start: String,
    login_hint: String,
    success: String,
    fail: String,
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn cli_install_script(provider: &str, messages: &InstallMessages) -> Result<String, String> {
    let start = shell_quote(&messages.start);
    let login_hint = shell_quote(&messages.login_hint);
    let success = shell_quote(&messages.success);
    let fail = shell_quote(&messages.fail);
    let (install_command, login_command, probe_command, poll_seconds) = match provider {
        "claude" => (
            "curl -fsSL https://claude.ai/install.sh | bash",
            Some("claude auth login"),
            "claude -p \"ok\"",
            120,
        ),
        "codex" => (
            "curl -fsSL https://chatgpt.com/codex/install.sh | sh",
            Some("codex login"),
            // codex exec 在非 git 目錄會拒跑，probe 改用即時且不耗額度的 login status
            "codex login status",
            120,
        ),
        "agy" => (
            "curl -fsSL https://antigravity.google/cli/install.sh | bash",
            None,
            "agy -p \"ok\"",
            600,
        ),
        "grok" => (
            "curl -fsSL https://x.ai/cli/install.sh | bash",
            Some("grok login"),
            // grok -p 會真的跑一次 grok-4.5 推理（實測 26 秒又燒額度）；models 只讀本機憑證，0.8 秒。
            // 未登入時它是否照樣 exit 0 無法驗證，故以登入字串判定，判錯也只是多要求登入一次。
            "grok models 2>/dev/null | grep -q '^You are logged in'",
            120,
        ),
        _ => return Err(format!("unsupported CLI provider: {provider}")),
    };
    let login_flow = login_command
        .map(|command| format!("  {command} || {{ echo {fail}; exit 1; }}\n"))
        .unwrap_or_default();
    let sentinel = cli_sentinel_name(provider);
    Ok(format!(
        r#"#!/bin/bash
echo {start}
export PATH="$HOME/.local/bin:$HOME/.grok/bin:$HOME/.codex/bin:$PATH"
if ! command -v {provider} >/dev/null 2>&1; then
  {install_command} || {{ echo {fail}; exit 1; }}
fi
echo {login_hint}
verified=0
if {probe_command} >/dev/null 2>&1; then
  verified=1
else
{login_flow}  elapsed=0
  while [ "$elapsed" -lt {poll_seconds} ]; do
    sleep 5
    elapsed=$((elapsed + 5))
    if {probe_command} >/dev/null 2>&1; then
      verified=1
      break
    fi
  done
fi
if [ "$verified" -ne 1 ]; then
  echo {fail}
  exit 1
fi
touch "$(dirname "$0")/{sentinel}"
echo ""
echo {success}
"#
    ))
}

// 驗證結果的唯一回傳通道：Mac 腳本跑在獨立終端機裡，只能靠這個檔案讓 app 知道登入成功
fn cli_sentinel_name(provider: &str) -> String {
    format!(".verified-{provider}")
}

#[tauri::command]
fn cli_verified(app: tauri::AppHandle, provider: String) -> Result<bool, String> {
    Ok(data_root(&app)?.join(cli_sentinel_name(&provider)).exists())
}

#[tauri::command]
fn sponsor_status(app: tauri::AppHandle) -> Result<bool, String> {
    Ok(data::sponsor_pack_active(&data_root(&app)?))
}

#[tauri::command]
fn import_sponsor_pack(app: tauri::AppHandle, data: Vec<u8>) -> Result<(), String> {
    data::install_sponsor_pack(&data_root(&app)?, &data).map_err(|error| error.to_string())
}

#[tauri::command]
fn install_cli(
    app: tauri::AppHandle,
    provider: String,
    messages: InstallMessages,
) -> Result<(), String> {
    let directory = data_root(&app)?;
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let _ = &messages;
    // 先清掉上一輪的驗證印記，避免輪詢讀到舊結果就把「已連結」點亮
    let sentinel_path = directory.join(cli_sentinel_name(&provider));
    let _ = std::fs::remove_file(&sentinel_path);
    #[cfg(target_os = "windows")]
    {
        use std::time::Duration;
        use tauri::Emitter;

        let spec = install::windows_specs()?
            .into_iter()
            .find(|spec| spec.id == provider)
            .ok_or_else(|| format!("unsupported CLI provider: {provider}"))?;
        let token = match install::try_begin(&provider, Duration::from_secs(60)) {
            install::BeginOutcome::Started(token) => token,
            install::BeginOutcome::AlreadyRunning => {
                install::raise_login_window(&spec.window_title);
                return Ok(());
            }
            install::BeginOutcome::Cooldown(seconds) => {
                return Err(format!("login-cooldown:{seconds}"))
            }
        };
        let task_app = app.clone();
        tauri::async_runtime::spawn(async move {
            let _token = token;
            let emit_app = task_app.clone();
            let _ = install::run_install(spec, &directory, cli::find_binary, move |progress| {
                if progress.stage == "done" {
                    let _ = std::fs::write(&sentinel_path, b"");
                }
                let _ = emit_app.emit("cli-install-progress", progress);
            })
            .await;
        });
    }
    #[cfg(not(target_os = "windows"))]
    {
        use std::time::Duration;

        if install::mac_cooldown(&provider, Duration::from_secs(60)).is_some() {
            Command::new("open")
                .args(["-a", "Terminal"])
                .spawn()
                .map_err(|error| error.to_string())?;
            return Ok(());
        }
        let script = cli_install_script(&provider, &messages)?;
        let script_path = directory.join(format!("install-{provider}.command"));
        std::fs::write(&script_path, script).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))
                .map_err(|error| error.to_string())?;
        }
        Command::new("open")
            .args(["-a", "Terminal"])
            .arg(&script_path)
            .spawn()
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn list_worlds(app: tauri::AppHandle) -> Result<Vec<data::WorldMeta>, String> {
    data::list_worlds(&data_root(&app)?).map_err(|error| error.to_string())
}

#[tauri::command]
fn create_world(app: tauri::AppHandle, name: String) -> Result<String, String> {
    data::create_world(&data_root(&app)?, &name).map_err(|error| error.to_string())
}

#[tauri::command]
fn create_sample_world(app: tauri::AppHandle, lang: String) -> Result<String, String> {
    data::create_sample_world(&data_root(&app)?, &lang).map_err(|error| error.to_string())
}

/// 前端建立新世界／新角色前先要一個代碼：草稿期生圖就能落在正確的路徑，存檔用同一個 id
#[tauri::command]
fn new_id() -> String {
    data::new_id()
}

#[tauri::command]
fn reclaim_world_if_empty(app: tauri::AppHandle, world_id: String) -> Result<bool, String> {
    data::reclaim_world_if_empty(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_world(app: tauri::AppHandle, world_id: String) -> Result<(), String> {
    data::delete_world(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

#[tauri::command]
fn rename_world(app: tauri::AppHandle, world_id: String, new_name: String) -> Result<(), String> {
    data::rename_world(&data_root(&app)?, &world_id, &new_name).map_err(|error| error.to_string())
}

#[tauri::command]
fn read_world_md(app: tauri::AppHandle, world_id: String) -> Result<String, String> {
    data::read_world_md(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

#[tauri::command]
fn write_world_md(app: tauri::AppHandle, world_id: String, content: String) -> Result<(), String> {
    data::write_world_md(&data_root(&app)?, &world_id, &content).map_err(|error| error.to_string())
}

#[tauri::command]
fn read_worldbook(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<Vec<data::WorldbookEntry>, String> {
    data::read_worldbook(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

#[tauri::command]
fn upsert_worldbook_entry(
    app: tauri::AppHandle,
    world_id: String,
    entry: data::WorldbookEntry,
) -> Result<u64, String> {
    data::upsert_worldbook_entry(&data_root(&app)?, &world_id, entry)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn reorder_worldbook_entries(
    app: tauri::AppHandle,
    world_id: String,
    uids: Vec<u64>,
) -> Result<(), String> {
    data::reorder_worldbook_entries(&data_root(&app)?, &world_id, &uids)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_worldbook_entry(app: tauri::AppHandle, world_id: String, uid: u64) -> Result<(), String> {
    data::delete_worldbook_entry(&data_root(&app)?, &world_id, uid)
        .map_err(|error| error.to_string())
}

/// 世界書分頁「機制帳本」面板：哪些條目被本地機制接管／跳過，供玩家切回「照原文送模型」。
#[tauri::command]
fn mechanism_ledger(app: tauri::AppHandle, world_id: String) -> Result<mechanism::Ledger, String> {
    Ok(mechanism::read_ledger(&data_root(&app)?, &world_id))
}

#[tauri::command]
fn worldbook_entry_to_character(
    app: tauri::AppHandle,
    world_id: String,
    uid: u64,
    color: String,
    as_player: bool,
) -> Result<CharacterMeta, String> {
    data::worldbook_entry_to_character(&data_root(&app)?, &world_id, uid, color, as_player)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn character_to_worldbook_entry(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<(), String> {
    data::character_to_worldbook_entry(&data_root(&app)?, &world_id, &character_id)
        .map_err(|error| error.to_string())
}

/// 狀態列是否顯示：沒有匯入狀態列規則的桌，整條狀態列不掛上去。
#[tauri::command]
fn world_has_state_bar(app: tauri::AppHandle, world_id: String) -> Result<bool, String> {
    data::world_has_state_bar(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

#[tauri::command]
fn import_worldbook(
    app: tauri::AppHandle,
    world_id: String,
    data: Vec<u8>,
    label: String,
) -> Result<data::WorldbookImport, String> {
    let json_text = import::worldbook_json(&data).map_err(|error| error.to_string())?;
    let root = data_root(&app)?;
    let before = receipts::snapshot(&root, &world_id);
    let result =
        data::import_worldbook(&root, &world_id, &json_text).map_err(|error| error.to_string())?;
    import::save_world_card(&root, &world_id, &data);
    import::save_gm_image(&root, &world_id, &data);
    if let Ok(book) = serde_json::from_str(&json_text) {
        import::import_mechanism(&root, &world_id, &book);
    }
    import::import_card_extension(&root, &world_id, &label, &data);
    receipts::record_worldbook_import(&root, &world_id, &label, before);
    Ok(result)
}

/// 選項要先換成當桌實名，前端貼入逐字稿時才不會留下卡片巨集。
#[tauri::command]
fn card_openings(
    app: tauri::AppHandle,
    world_id: String,
    data: Vec<u8>,
    lang: String,
) -> Result<Vec<String>, String> {
    let Some((name, openings)) = import::card_openings(&data) else {
        return Ok(Vec::new());
    };
    let root = data_root(&app)?;
    let player = data::read_player_card(&root, &world_id).map_err(|error| error.to_string())?;
    Ok(openings
        .iter()
        .map(|opening| {
            transport::resolve_display_macros(
                opening,
                player.as_ref().map(|card| card.name.as_str()),
                &name,
                &lang,
            )
        })
        .collect())
}

#[tauri::command]
fn dedupe_worldbook(app: tauri::AppHandle, world_id: String) -> Result<usize, String> {
    data::dedupe_worldbook(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

// 存檔位置由前端的「另存新檔」對話框決定
#[tauri::command]
fn export_worldbook(app: tauri::AppHandle, world_id: String, path: String) -> Result<(), String> {
    data::export_worldbook(&data_root(&app)?, &world_id, std::path::Path::new(&path))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_characters(app: tauri::AppHandle, world_id: String) -> Result<Vec<CharacterMeta>, String> {
    data::list_characters(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

#[tauri::command]
fn reorder_characters(
    app: tauri::AppHandle,
    world_id: String,
    ids: Vec<String>,
) -> Result<(), String> {
    data::reorder_characters(&data_root(&app)?, &world_id, &ids).map_err(|error| error.to_string())
}

#[tauri::command]
fn read_character(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<CharacterCard, String> {
    data::read_character(&data_root(&app)?, &world_id, &character_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn write_character(
    app: tauri::AppHandle,
    world_id: String,
    card: CharacterCard,
) -> Result<(), String> {
    data::write_character(&data_root(&app)?, &world_id, &card).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_character_archived(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    archived: bool,
) -> Result<(), String> {
    data::set_character_archived(&data_root(&app)?, &world_id, &character_id, archived)
        .map_err(|error| error.to_string())
}

/// 玩家從隱藏區手動拉回自動隱藏的卡（或手動收進去）。玩家意志優先於自動結算，
/// 幕中按下快取代價玩家自付——與 set_character_archived 同款語意。
#[tauri::command]
fn set_character_auto_hidden(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    auto_hidden: bool,
) -> Result<(), String> {
    data::set_character_auto_hidden(&data_root(&app)?, &world_id, &character_id, auto_hidden)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_character(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<(), String> {
    data::delete_character(&data_root(&app)?, &world_id, &character_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn import_character(
    app: tauri::AppHandle,
    world_id: String,
    data: Vec<u8>,
    color: String,
) -> Result<CharacterImport, String> {
    let root = data_root(&app)?;
    let before = receipts::snapshot(&root, &world_id);
    let entries_before = data::read_worldbook(&root, &world_id).map_or(0, |entries| entries.len());
    let meta = import::import_character(&root, &world_id, &data, &color)
        .map_err(|error| error.to_string())?;
    receipts::record_character_import(&root, &world_id, &meta.id, &meta.name, before);
    // 卡片隨身的世界書條目也要跟世界書路徑一樣回報進來幾條、重複跳過幾條
    let imported =
        data::read_worldbook(&root, &world_id).map_or(0, |entries| entries.len() - entries_before);
    let skipped = import::probe_import(&data).book_entries.saturating_sub(imported);
    Ok(CharacterImport {
        meta,
        book: data::WorldbookImport { imported, skipped },
    })
}

/// 角色卡匯入的完整結果：新角色本體＋卡片隨身世界書的收編數字。
#[derive(serde::Serialize)]
struct CharacterImport {
    meta: CharacterMeta,
    book: data::WorldbookImport,
}

#[tauri::command]
fn probe_import(data: Vec<u8>) -> Result<import::ImportProbe, String> {
    Ok(import::probe_import(&data))
}

/// 側欄按鈕判斷要不要顯示「復原上次匯入」；未來路由框也靠這份摘要判身分。
#[tauri::command]
fn list_import_receipts(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<Vec<receipts::ImportReceiptSummary>, String> {
    Ok(receipts::list_import_receipts(&data_root(&app)?, &world_id))
}

/// 逆向最後一筆匯入收據：刪角色、刪未經玩家修改的世界書條目、退回機制寫入與桌名。
#[tauri::command]
fn undo_last_import(app: tauri::AppHandle, world_id: String) -> Result<receipts::UndoReport, String> {
    receipts::undo_last_import(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

/// adoptImportName 改名成功後呼叫：把舊桌名補進最後一筆收據，undo 才能把桌名退回去。
#[tauri::command]
fn record_import_rename(app: tauri::AppHandle, world_id: String, old_name: String) -> Result<(), String> {
    receipts::record_last_import_rename(&data_root(&app)?, &world_id, &old_name);
    Ok(())
}

/// AI 卡重構中止時的錯誤字串 sentinel：前端靠它分流「玩家主動取消」與其他失敗，一字不差。
pub(crate) const REFACTOR_ABORTED: &str = "refactor-aborted";

/// AI 卡重構套用：玩家勾選的角色／介面／機制落檔，收據記「實際套用的那份」供一鍵倒退。
#[tauri::command]
fn refactor_apply(
    app: tauri::AppHandle,
    world_id: String,
    outcome: refactor::RefactorOutcome,
    selection: refactor::RefactorSelection,
    record_receipt: Option<bool>,
) -> Result<refactor::RefactorApplySummary, String> {
    let root = data_root(&app)?;
    let before = receipts::snapshot(&root, &world_id);
    let result =
        refactor::apply(&root, &world_id, &outcome, &selection).map_err(|error| error.to_string())?;
    if record_receipt.unwrap_or(true) {
        receipts::record_refactor_apply(
            &root,
            &world_id,
            "AI 卡重構",
            result.character_ids,
            result.rewritten_entries,
            result.deleted_entries,
            before,
        );
    }
    Ok(result.summary)
}

/// AI 卡重構讀卡（盤點階段）：AI 讀整張卡的世界書，認出人物（可能散在好幾條裡）／介面／機制
/// 三類候選。
#[tauri::command]
async fn refactor_survey(
    app: tauri::AppHandle,
    world_id: String,
    on_delta: tauri::ipc::Channel<String>,
) -> Result<refactor_ai::RefactorSurveyOutcome, String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let root = data_root(&app)?;
    let context =
        refactor_ai::assemble_card_context(&root, &world_id).map_err(|error| error.to_string())?;
    let entries = data::read_worldbook(&root, &world_id).map_err(|error| error.to_string())?;
    let signals = refactor_ai::prescan_worldbook(&entries);
    let messages = refactor_ai::survey_messages(&context, &signals, &lang);
    let (_guard, mut cancel) = inflight::register(&world_id);
    let raw = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(REFACTOR_ABORTED.to_owned()),
        result = stream_via_transport(
            &app,
            &config,
            None,
            false,
            transport::gm_tier(&config),
            Some(&world_id),
            "GM",
            "Output exactly in the requested marker format, nothing else.",
            &messages,
            true, // 思考增量餵進度字尾：玩家分得出「在想」與「掛了」
            |delta| {
                let _ = on_delta.send(delta.to_owned());
            },
        ) => result?,
    };
    let outcome = refactor_ai::parse_survey(&raw);
    // 臨時水印（驗完即刪）：判官對每個人實際寫的 mode，分辨「沒寫」與「明判 tangled」。
    for person in &outcome.persons {
        eprintln!(
            "[survey-persons] name={} mode={:?} uids={:?} spans={:?}",
            person.name, person.mode, person.uids, person.spans
        );
    }
    Ok(outcome)
}

/// AI 卡重構本地組裝（小抄合約 v1）：判官定案後，carry／drop 整條／split 逐段路由／clean
/// 人物這幾類不必再問 AI，App 本地零呼叫組裝＋四項機械稽核。純本地、無 AI 呼叫、不落檔——
/// 產物由前端後續彙整進 RefactorOutcome 送 refactor_apply。
#[tauri::command]
fn refactor_assemble_local(
    app: tauri::AppHandle,
    world_id: String,
    survey: refactor_ai::RefactorSurveyOutcome,
) -> Result<refactor_assemble::RefactorLocalAssembly, String> {
    let root = data_root(&app)?;
    refactor_assemble::assemble_local(&root, &world_id, &survey).map_err(|error| error.to_string())
}

/// AI 卡重構讀卡（展開階段，介面）：system 與盤點同一字串（快取命中），逐條展開成
/// 結構化產物。人物展開走專屬的 refactor_expand_person（一人一次呼叫、可能帶多條來源）。
#[tauri::command]
async fn refactor_expand(
    app: tauri::AppHandle,
    world_id: String,
    entry_uid: String,
    kind: String,
    known_fields: Option<Vec<String>>,
    on_delta: tauri::ipc::Channel<String>,
) -> Result<refactor_ai::RefactorExpandOutcome, String> {
    let entry_kind = refactor_ai::EntryKind::parse(&kind)?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let root = data_root(&app)?;
    let context =
        refactor_ai::assemble_card_context(&root, &world_id).map_err(|error| error.to_string())?;
    let entry_text = refactor_ai::entry_full_text(&root, &world_id, &entry_uid)
        .map_err(|error| error.to_string())?;
    let known_fields = known_fields.unwrap_or_default();
    let messages = refactor_ai::expand_messages(
        &context,
        &entry_uid,
        &entry_text,
        entry_kind,
        &known_fields,
        &lang,
    );
    let (_guard, mut cancel) = inflight::register(&world_id);
    let raw = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(REFACTOR_ABORTED.to_owned()),
        result = stream_via_transport(
            &app,
            &config,
            None,
            false,
            transport::refactor_expand_tier(&config, &chat_transport(&config)),
            Some(&world_id),
            "GM",
            "Output exactly in the requested marker format, nothing else.",
            &messages,
            true, // 思考增量餵進度字尾：玩家分得出「在想」與「掛了」
            |delta| {
                let _ = on_delta.send(delta.to_owned());
            },
        ) => result?,
    };
    Ok(refactor_ai::parse_expand(entry_kind, &entry_uid, &raw))
}

/// AI 卡重構讀卡（展開階段，人物）：一人一次呼叫，帶上他名下全部來源條目全文（要點 8）；
/// is_player 由盤點結果直接帶過來，不是這裡自己判斷。
#[tauri::command]
async fn refactor_expand_person(
    app: tauri::AppHandle,
    world_id: String,
    name: String,
    uids: Vec<String>,
    is_player: bool,
    on_delta: tauri::ipc::Channel<String>,
) -> Result<refactor_ai::RefactorPersonExpandOutcome, String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let root = data_root(&app)?;
    let context =
        refactor_ai::assemble_card_context(&root, &world_id).map_err(|error| error.to_string())?;
    let mut sources = Vec::with_capacity(uids.len());
    for uid in &uids {
        let text =
            refactor_ai::entry_full_text(&root, &world_id, uid).map_err(|error| error.to_string())?;
        sources.push((uid.clone(), text));
    }
    let messages = refactor_ai::person_expand_messages(&context, &name, &sources, &lang);
    let (_guard, mut cancel) = inflight::register(&world_id);
    let raw = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(REFACTOR_ABORTED.to_owned()),
        result = stream_via_transport(
            &app,
            &config,
            None,
            false,
            transport::refactor_expand_tier(&config, &chat_transport(&config)),
            Some(&world_id),
            "GM",
            "Output exactly in the requested marker format, nothing else.",
            &messages,
            true, // 思考增量餵進度字尾：玩家分得出「在想」與「掛了」
            |delta| {
                let _ = on_delta.send(delta.to_owned());
            },
        ) => result?,
    };
    Ok(refactor_ai::parse_person_expand(&raw, &name, &uids, is_player))
}

/// `expand_span_placeholders` 的查表：接 `refactor_assemble::resolve_span` 找段落原文（trim
/// 過）；找不到（uid／段號無效）就回 None，讓佔位符原樣保留。absorb／split_group 共用。
fn span_lookup<'a>(
    by_uid: &'a std::collections::BTreeMap<u64, &'a data::WorldbookEntry>,
) -> impl Fn(&str) -> Option<String> + 'a {
    move |span_ref: &str| {
        refactor_assemble::resolve_span(by_uid, span_ref)
            .map(|(entry, span)| entry.content[span.start..span.end].trim().to_owned())
    }
}

/// AI 卡重構讀卡（接管階段）：ENTRIES 判 absorb 的條目一條一次呼叫。本文由 App 原文照搬＋
/// 鎖定，AI 只補可本地執行的 RULES／TRIGGERS——輸出天生短，取代舊「條目重寫」機制分支。
/// 觸發敘事裡的 `{{span:uid#sN}}` 指位在這裡換回原文全文；解析全空（抽不出規則）也照樣回
/// entry，本文照搬仍然成立，套用端看 rules／triggers 是否非空決定要不要鎖。
#[tauri::command]
async fn refactor_absorb_entry(
    app: tauri::AppHandle,
    world_id: String,
    entry_uid: String,
    known_fields: Option<Vec<String>>,
    on_delta: tauri::ipc::Channel<String>,
) -> Result<refactor_ai::RefactorRewriteOutcome, String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let root = data_root(&app)?;
    let context =
        refactor_ai::assemble_card_context(&root, &world_id).map_err(|error| error.to_string())?;
    let worldbook = data::read_worldbook(&root, &world_id).map_err(|error| error.to_string())?;
    let source = worldbook
        .iter()
        .find(|entry| entry.uid.to_string() == entry_uid)
        .ok_or_else(|| format!("找不到 uid={entry_uid} 的世界書條目"))?;
    let entry_text = refactor_ai::entry_full_text(&root, &world_id, &entry_uid)
        .map_err(|error| error.to_string())?;
    let known_fields = known_fields.unwrap_or_default();
    let messages =
        refactor_ai::absorb_messages(&context, &entry_uid, &entry_text, &known_fields, &lang);
    let (_guard, mut cancel) = inflight::register(&world_id);
    let raw = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(REFACTOR_ABORTED.to_owned()),
        result = stream_via_transport(
            &app,
            &config,
            None,
            false,
            transport::refactor_expand_tier(&config, &chat_transport(&config)),
            Some(&world_id),
            "GM",
            "Output exactly in the requested marker format, nothing else.",
            &messages,
            true, // 思考增量餵進度字尾：玩家分得出「在想」與「掛了」
            |delta| {
                let _ = on_delta.send(delta.to_owned());
            },
        ) => result?,
    };
    let outcome = refactor_ai::parse_absorb(&raw);
    let by_uid: std::collections::BTreeMap<u64, &data::WorldbookEntry> =
        worldbook.iter().map(|entry| (entry.uid, entry)).collect();
    let lookup = span_lookup(&by_uid);
    let triggers = outcome
        .triggers
        .into_iter()
        .map(|mut trigger| {
            trigger.preamble = refactor_ai::expand_span_placeholders(&trigger.preamble, &lookup);
            for case in &mut trigger.cases {
                case.text = refactor_ai::expand_span_placeholders(&case.text, &lookup);
            }
            trigger
        })
        .collect();
    Ok(refactor_ai::RefactorRewriteOutcome {
        entry: Some(refactor_ai::RefactorNewEntry {
            title: source.title.clone(),
            kind: "mechanism".to_owned(),
            content: source.content.clone(),
            source_uids: vec![entry_uid.clone()],
            rules: outcome.rules,
            triggers,
            meta: Some(refactor_assemble::build_meta(source)),
        }),
        raw: outcome.raw,
    })
}

/// AI 卡重構讀卡（合組階段）：SPLITS 標 group 的 span 們合組成一條新條目——一組一次呼叫，拆出
/// 屬於這個主題的資訊、合併改寫（小抄合約 v1 GROUPS 區塊）。CONTENT 裡的 `{{span:uid#sN}}`
/// 指位（大組保險）在這裡換回原文全文。
#[tauri::command]
async fn refactor_split_group(
    app: tauri::AppHandle,
    world_id: String,
    group_id: String,
    title: String,
    kind: String,
    spans: Vec<String>,
    known_fields: Option<Vec<String>>,
    on_delta: tauri::ipc::Channel<String>,
) -> Result<refactor_ai::RefactorRewriteOutcome, String> {
    let group_kind = refactor_ai::GroupKind::parse(&kind)?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let root = data_root(&app)?;
    let context =
        refactor_ai::assemble_card_context(&root, &world_id).map_err(|error| error.to_string())?;
    let worldbook = data::read_worldbook(&root, &world_id).map_err(|error| error.to_string())?;
    let by_uid: std::collections::BTreeMap<u64, &data::WorldbookEntry> =
        worldbook.iter().map(|entry| (entry.uid, entry)).collect();
    let mut materials = Vec::with_capacity(spans.len());
    let mut source_uids: Vec<String> = Vec::new();
    for span_ref in &spans {
        let (entry, span) = refactor_assemble::resolve_span(&by_uid, span_ref)
            .ok_or_else(|| format!("合組 {group_id}（{title}）找不到段落引用：{span_ref}"))?;
        materials.push((
            span_ref.clone(),
            entry.content[span.start..span.end].trim().to_owned(),
        ));
        let uid = entry.uid.to_string();
        if !source_uids.contains(&uid) {
            source_uids.push(uid);
        }
    }
    let known_fields = known_fields.unwrap_or_default();
    let messages = refactor_ai::group_messages(
        &context,
        &title,
        group_kind,
        &materials,
        &known_fields,
        &lang,
    );
    let (_guard, mut cancel) = inflight::register(&world_id);
    let raw = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(REFACTOR_ABORTED.to_owned()),
        result = stream_via_transport(
            &app,
            &config,
            None,
            false,
            transport::refactor_expand_tier(&config, &chat_transport(&config)),
            Some(&world_id),
            "GM",
            "Output exactly in the requested marker format, nothing else.",
            &messages,
            true, // 思考增量餵進度字尾：玩家分得出「在想」與「掛了」
            |delta| {
                let _ = on_delta.send(delta.to_owned());
            },
        ) => result?,
    };
    let mut outcome = refactor_ai::parse_group(&raw, &title, group_kind, &source_uids);
    if let Some(entry) = outcome.entry.as_mut() {
        let lookup = span_lookup(&by_uid);
        entry.content = refactor_ai::expand_span_placeholders(&entry.content, &lookup);
    }
    Ok(outcome)
}

/// AI 卡重構讀卡（展開階段，statusbar 段）：SPLITS route=statusbar 的段落材料＝該條全部
/// statusbar 段原文串接，走既有 interface 型呼叫（只抽 STATE、永不產殼——這些段落本來就只是
/// 介面格式，不是完整可玩介面）。spans 內每個引用共享同一個來源 uid（route=statusbar 不跨
/// 條目），entry_uid 只用來標記結果的 source_uids。
#[tauri::command]
async fn refactor_expand_spans(
    app: tauri::AppHandle,
    world_id: String,
    entry_uid: String,
    spans: Vec<String>,
    known_fields: Option<Vec<String>>,
    on_delta: tauri::ipc::Channel<String>,
) -> Result<refactor_ai::RefactorExpandOutcome, String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let root = data_root(&app)?;
    let context =
        refactor_ai::assemble_card_context(&root, &world_id).map_err(|error| error.to_string())?;
    let worldbook = data::read_worldbook(&root, &world_id).map_err(|error| error.to_string())?;
    let by_uid: std::collections::BTreeMap<u64, &data::WorldbookEntry> =
        worldbook.iter().map(|entry| (entry.uid, entry)).collect();
    let mut parts = Vec::with_capacity(spans.len());
    for span_ref in &spans {
        let (entry, span) = refactor_assemble::resolve_span(&by_uid, span_ref)
            .ok_or_else(|| format!("找不到段落引用：{span_ref}"))?;
        parts.push(entry.content[span.start..span.end].trim().to_owned());
    }
    let material = parts.join("\n\n");
    let known_fields = known_fields.unwrap_or_default();
    let messages = refactor_ai::expand_messages(
        &context,
        &entry_uid,
        &material,
        refactor_ai::EntryKind::Interface,
        &known_fields,
        &lang,
    );
    let (_guard, mut cancel) = inflight::register(&world_id);
    let raw = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(REFACTOR_ABORTED.to_owned()),
        result = stream_via_transport(
            &app,
            &config,
            None,
            false,
            transport::refactor_expand_tier(&config, &chat_transport(&config)),
            Some(&world_id),
            "GM",
            "Output exactly in the requested marker format, nothing else.",
            &messages,
            true, // 思考增量餵進度字尾：玩家分得出「在想」與「掛了」
            |delta| {
                let _ = on_delta.send(delta.to_owned());
            },
        ) => result?,
    };
    Ok(refactor_ai::parse_expand(
        refactor_ai::EntryKind::Interface,
        &entry_uid,
        &raw,
    ))
}

/// AI 卡重構中止：立即殺該桌全部在途呼叫（CLI 殺子程序、API 斷線即停止計費）。
#[tauri::command]
fn refactor_abort(world_id: String) {
    inflight::abort_world(&world_id);
}

/// 讀 AI 卡重構套用介面時可能順便產的靜態渲染殼（interface-shell.html）；沒套用過或那次沒
/// 產出殼就回 None，前端退回保底狀態欄／卡片自帶殼（既有兩層，零改動）。
#[tauri::command]
fn refactor_interface_shell(app: tauri::AppHandle, world_id: String) -> Result<Option<String>, String> {
    data::read_interface_shell(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

/// AI 卡重構匯出（結果卡摘要頁用）：產物來自前端 state（就算還沒套用過也能匯出），
/// 直接序列化寫到玩家選的路徑，供之後用「匯入重構產物」讀回重玩。
#[tauri::command]
fn refactor_export_outcome(outcome: refactor::RefactorOutcome, path: String) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&outcome).map_err(|error| error.to_string())?;
    std::fs::write(&path, json).map_err(|error| error.to_string())
}

/// AI 卡重構匯出（世界書工具列用）：讀 apply() 套用成功時桌內落下的存檔；沒有就回固定錯誤
/// 字串（前端比對 "refactor-export-none" 顯示對應提示）。
#[tauri::command]
fn refactor_export_saved(app: tauri::AppHandle, world_id: String, path: String) -> Result<(), String> {
    let root = data_root(&app)?;
    let content = data::read_refactor_outcome(&root, &world_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "refactor-export-none".to_owned())?;
    std::fs::write(&path, content).map_err(|error| error.to_string())
}

#[tauri::command]
fn refactor_outcome_exists(app: tauri::AppHandle, world_id: String) -> Result<bool, String> {
    Ok(data::read_refactor_outcome(&data_root(&app)?, &world_id)
        .map_err(|e| e.to_string())?
        .is_some())
}

#[tauri::command]
fn card_interfaces(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<Vec<import::CardInterface>, String> {
    import::read_card_interfaces(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

// 存檔位置由前端的「另存新檔」對話框決定；副檔名決定 PNG 或 JSON
#[tauri::command]
fn export_character(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    path: String,
) -> Result<(), String> {
    import::export_character(
        &data_root(&app)?,
        &world_id,
        &character_id,
        std::path::Path::new(&path),
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn read_character_image(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<Option<String>, String> {
    import::character_image(&data_root(&app)?, &world_id, &character_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_character_image(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    import::save_character_image(&data_root(&app)?, &world_id, &character_id, &data)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_character_image(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<(), String> {
    import::delete_character_image(&data_root(&app)?, &world_id, &character_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn read_character_avatar(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<Option<String>, String> {
    import::character_avatar(&data_root(&app)?, &world_id, &character_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_character_avatar(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    import::save_character_avatar(&data_root(&app)?, &world_id, &character_id, &data)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_character_avatar(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<(), String> {
    import::delete_character_avatar(&data_root(&app)?, &world_id, &character_id)
        .map_err(|error| error.to_string())
}

/// GM 卡的圖：世界書匯入 PNG 卡時存下的那張，沒有回 None（前端回退內建書本圖）
#[tauri::command]
fn read_gm_image(app: tauri::AppHandle, world_id: String) -> Result<Option<String>, String> {
    import::gm_image(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

#[tauri::command]
fn append_transcript(
    app: tauri::AppHandle,
    world_id: String,
    scene: u64,
    event: TranscriptEvent,
) -> Result<(), String> {
    data::append_transcript(&data_root(&app)?, &world_id, scene, &event)
        .map_err(|error| error.to_string())
}

/// 貼開場白＝GM 旁白，但狀態區塊要走與 GM 回覆同一條解析。剝除、併進檯面、事件帶快照
/// 三件事由這一個指令一次做完——前端各寫一半會破壞「目前值＝最後一則事件快照」的不變式。
#[tauri::command]
fn post_opening(
    app: tauri::AppHandle,
    world_id: String,
    scene: u64,
    ts: String,
    text: String,
) -> Result<TranscriptEvent, String> {
    let block = transport::extract_state_block(&text);
    let root = data_root(&app)?;
    let config = data::read_config(&config_root(&app)?).unwrap_or_default();
    let lang = transport::ui_language(&config);
    let player_name = data::read_player_card(&root, &world_id)
        .ok()
        .flatten()
        .map(|card| card.name);
    let user_name = player_name
        .as_deref()
        .unwrap_or_else(|| transport::player_fallback_name(&lang));
    let (event, outcome) =
        data::append_opening(&root, &world_id, scene, &ts, &text, &block, user_name)
            .map_err(|error| error.to_string())?;
    mechanism::append_log(&root, &world_id, scene, &outcome.records);
    // 掛到剛才那筆匯入收據上：復原匯入時這則開場白要跟著收掉，
    // 不然重匯同一張卡想改挑一則時，舊的那則還壓在開局上
    receipts::record_posted_opening(&root, &world_id, scene, &ts);
    Ok(event)
}

/// 開場白翻譯：選擇視窗按下「翻譯」時呼叫，把單則開場白譯成玩家語言方便挑選、貼出。
/// 一律走 fast 檔（單則翻譯用不到 GM 檔的推理力，要點 3）；API 模式沒設定 fast 模型時
/// 退回 GM 檔，讓按鈕在任何設定下都能用。lang 由前端帶入（玩家介面語言），這裡不再另外查。
#[tauri::command]
async fn translate_opening(
    app: tauri::AppHandle,
    world_id: String,
    text: String,
    lang: String,
) -> Result<String, String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let messages = translate::opening_messages(&text, &lang);
    let tier = if chat_transport(&config) == "api"
        && transport::resolve_model(data::Tier::Fast, &config).is_err()
    {
        transport::gm_tier(&config)
    } else {
        data::Tier::Fast
    };
    let raw = stream_via_transport(
        &app,
        &config,
        None,
        false,
        tier,
        Some(&world_id),
        "GM",
        "Output only the translated text itself, nothing else.",
        &messages,
        false,
        |_| {},
    )
    .await?;
    Ok(raw.trim().to_owned())
}

#[tauri::command]
fn read_transcript(
    app: tauri::AppHandle,
    world_id: String,
    scene: u64,
) -> Result<Vec<TranscriptEvent>, String> {
    data::read_transcript(&data_root(&app)?, &world_id, scene).map_err(|error| error.to_string())
}

/// 這一幕已經出場的角色卡與世界書人物（AI 卡重構包 4b）：前端載入時用來初始化本地分區，
/// 不必自己重掃 transcript 猜前綴。卡登場記的是名字，這裡拿現有卡清單反查回 id。
#[derive(Serialize)]
struct SceneAppearances {
    character_ids: Vec<String>,
    person_titles: Vec<String>,
}

fn scene_appearances_at(
    root: &std::path::Path,
    world_id: &str,
) -> Result<SceneAppearances, String> {
    let state = data::read_state(root, world_id).map_err(|error| error.to_string())?;
    let events = data::read_transcript(root, world_id, state.current_scene)
        .map_err(|error| error.to_string())?;
    let person_titles = transport::appeared_person_titles(&events).into_iter().collect();
    let card_names = data::appeared_titles(&events, data::CARD_ARRIVAL_PREFIX);
    let character_ids = data::list_characters(root, world_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|meta| card_names.iter().any(|name| data::name_matches(name, &meta.name)))
        .map(|meta| meta.id)
        .collect();
    Ok(SceneAppearances {
        character_ids,
        person_titles,
    })
}

#[tauri::command]
fn scene_appearances(app: tauri::AppHandle, world_id: String) -> Result<SceneAppearances, String> {
    scene_appearances_at(&data_root(&app)?, &world_id)
}

// 收回上一句：只砍當前這一幕的最後一筆，可連按；回傳 false＝這一幕已經收乾淨了
#[tauri::command]
fn pop_transcript(app: tauri::AppHandle, world_id: String, scene: u64) -> Result<bool, String> {
    data::pop_transcript(&data_root(&app)?, &world_id, scene).map_err(|error| error.to_string())
}

// 存檔位置由前端的「另存新檔」對話框決定，這裡只負責產內容寫入該路徑
#[tauri::command]
fn export_transcript(app: tauri::AppHandle, world_id: String, path: String) -> Result<(), String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let markdown = data::export_transcript_markdown(&data_root(&app)?, &world_id, &lang)
        .map_err(|error| error.to_string())?;
    std::fs::write(&path, markdown).map_err(|error| error.to_string())
}

// 單場匯出：格式與 export_transcript 一致，但只匯一場，供「過去的場」單場檢視使用
#[tauri::command]
fn export_scene(
    app: tauri::AppHandle,
    world_id: String,
    scene: u64,
    path: String,
) -> Result<(), String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let markdown = data::export_scene_markdown(&data_root(&app)?, &world_id, scene, &lang)
        .map_err(|error| error.to_string())?;
    std::fs::write(&path, markdown).map_err(|error| error.to_string())
}

#[tauri::command]
fn read_state(app: tauri::AppHandle, world_id: String) -> Result<WorldState, String> {
    data::read_state(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

#[tauri::command]
fn write_state(app: tauri::AppHandle, world_id: String, state: WorldState) -> Result<(), String> {
    data::write_state(&data_root(&app)?, &world_id, &state).map_err(|error| error.to_string())
}

#[tauri::command]
async fn set_table_state(
    app: tauri::AppHandle,
    world_id: String,
    fields: std::collections::BTreeMap<String, String>,
) -> Result<(), String> {
    let root = data_root(&app)?;
    let mut state = data::read_state(&root, &world_id).map_err(|error| error.to_string())?;
    for (key, value) in fields {
        if value.is_empty() {
            state.state.table.remove(&key);
        } else {
            state.state.table.insert(key, value);
        }
    }
    data::write_state(&root, &world_id, &state).map_err(|error| error.to_string())?;
    data::set_last_transcript_state(&root, &world_id, state.current_scene, &state.state)
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
async fn set_state_path(
    app: tauri::AppHandle,
    world_id: String,
    path: Vec<String>,
    value: String,
) -> Result<(), String> {
    let root = data_root(&app)?;
    let mut state = data::read_state(&root, &world_id).map_err(|error| error.to_string())?;
    if !data::set_tree_value(&mut state.state.tree, &path, &value) {
        return Ok(());
    }
    data::write_state(&root, &world_id, &state).map_err(|error| error.to_string())?;
    data::set_last_transcript_state(&root, &world_id, state.current_scene, &state.state)
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// 面板指認：把角色卡綁到狀態樹的某個分支；path 為 None／空陣列＝解除綁定。
/// 一支分支只屬於一個角色，換綁時把指到同一條路徑的舊綁定一併移除。
/// branch_bindings 在 WorldState 上、不在 TableState 裡，不需要同步 transcript 快照。
#[tauri::command]
fn set_branch_binding(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    path: Option<Vec<String>>,
) -> Result<(), String> {
    let root = data_root(&app)?;
    let mut state = data::read_state(&root, &world_id).map_err(|error| error.to_string())?;
    match path.filter(|path| !path.is_empty()) {
        Some(path) => {
            // 一支分支只屬於一個角色：先清掉其他卡指到同一條路徑的舊綁定。
            state
                .branch_bindings
                .retain(|other_id, bound| *other_id == character_id || *bound != path);
            state.branch_bindings.insert(character_id, path);
        }
        None => {
            state.branch_bindings.remove(&character_id);
        }
    }
    data::write_state(&root, &world_id, &state).map_err(|error| error.to_string())
}

/// 面板記號：玩家把某欄標成計數器（例如卡片自訂的「第 N 天」，時間跳躍是那張卡的明文
/// 功能），以後全量桌跳動比對不再對它示警。寫一條 Counter 規則釘死，並清掉這一輪
/// 已經標出來的那筆警示（不然要等下一輪重算才會消失）。
#[tauri::command]
fn mark_state_counter(
    app: tauri::AppHandle,
    world_id: String,
    path: Vec<String>,
) -> Result<(), String> {
    let root = data_root(&app)?;
    let mut state = data::read_state(&root, &world_id).map_err(|error| error.to_string())?;
    let Some(first) = path.first() else {
        return Ok(());
    };
    let mut rule = data::FieldRule::for_kind(data::FieldKind::Counter);
    rule.branch = Some(first.clone());
    let key = path.join(".");
    state.mechanism.rules.insert(key.clone(), rule);
    state.state.jumps.remove(&key);
    data::write_state(&root, &world_id, &state).map_err(|error| error.to_string())
}

/// 面板要畫的有效綁定（含自動同名比對的結果）；解析不到分支的卡不進清單。
#[tauri::command]
fn branch_bindings(app: tauri::AppHandle, world_id: String) -> Result<Vec<BranchBinding>, String> {
    let root = data_root(&app)?;
    let state = data::read_state(&root, &world_id).map_err(|error| error.to_string())?;
    let cards = load_active_cards(&root, &world_id)?;
    Ok(cards
        .into_iter()
        .filter_map(|card| {
            let path = transport::resolve_branch(
                &state.state.tree,
                &state.branch_bindings,
                &card.id,
                &card.name,
            )?;
            let auto = state.branch_bindings.get(&card.id) != Some(&path);
            Some(BranchBinding {
                path,
                character_id: card.id,
                character_name: card.name,
                auto,
            })
        })
        .collect())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BranchBinding {
    /// 狀態樹路徑
    path: Vec<String>,
    character_id: String,
    character_name: String,
    /// true＝同名自動比對的結果（沒存進 state.json）
    auto: bool,
}

#[tauri::command]
fn read_config(app: tauri::AppHandle) -> Result<AppConfig, String> {
    data::read_config(&config_root(&app)?).map_err(|error| error.to_string())
}

#[tauri::command]
fn write_config(app: tauri::AppHandle, config: AppConfig) -> Result<(), String> {
    data::write_config(&config_root(&app)?, &config).map_err(|error| error.to_string())
}

#[tauri::command]
async fn detect_clis() -> Vec<cli::CliInfo> {
    cli::detect_clis().await
}

/// 設定 UI 下拉用：列出 CLI 訂閱模式可選的模型（讀 CLI 本機快取，見 cli::cli_model_catalog）
#[tauri::command]
fn list_cli_models(cli: String) -> Vec<cli::ModelOption> {
    cli::cli_model_catalog(&cli)
}

/// 使用者選定的聊天傳輸層（preferences.transport，預設 api）。
fn chat_transport(config: &data::AppConfig) -> String {
    config
        .preferences
        .get("transport")
        .and_then(|value| value.as_str())
        .unwrap_or("api")
        .to_owned()
}

/// claude CLI 的環境變數：沙盒告知＋（設了相容端點時）BASE_URL 與 token。
/// 單發（stream_via_transport）與 lane 續聊共用。
fn claude_cli_envs(config: &data::AppConfig) -> Vec<(String, String)> {
    // Claude Code 開場會自建 macOS 沙盒，系統因此以「Table Tavern」的名義向玩家要
    // 桌面／音樂資料夾權限（tccd 日誌實證：accessing=claude-code、responsible=本 app）。
    // 這個變數告訴它「你已經在沙盒裡」，想省掉那組彈窗；實測仍會被要求媒體資料庫權限，
    // 效果未定但無害（我們給的是 --tools ""，它本來就不需要那些資料夾）。
    // 彈窗文案改由 Info.plist 的 NSAppleMusicUsageDescription 等鍵說明。
    let mut envs = vec![("CLAUDE_CODE_SANDBOXED".to_owned(), "1".to_owned())];
    let base_url = config
        .preferences
        .get("claude_base_url")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .unwrap_or("");
    if !base_url.is_empty() {
        envs.push(("ANTHROPIC_BASE_URL".to_owned(), base_url.to_owned()));
        if let Some(api_key) = config
            .api_keys
            .get("claude_compat")
            .map(String::as_str)
            .map(str::trim)
            .filter(|key| !key.is_empty())
        {
            envs.push(("ANTHROPIC_AUTH_TOKEN".to_owned(), api_key.to_owned()));
        }
    }
    envs
}

/// claude CLI 的 session 檔目錄（resume 續聊的抹寫要直接讀寫它）。
fn claude_home_dir() -> PathBuf {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".claude")))
        .or_else(|| std::env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join(".claude")))
        .unwrap_or_else(|| PathBuf::from(".claude"))
}

/// 準備 claude lane 呼叫素材：風險告知檢查＋CLI 偵測＋模型解析＋env。
/// lane 續聊（chat／narrate／suggest）專用；其餘呼叫照走 stream_via_transport 單發。
async fn prepare_claude_call(
    app: &tauri::AppHandle,
    config: &data::AppConfig,
    tier: data::Tier,
) -> Result<lanes::ClaudeCall, String> {
    if config.preferences.get("cli_risk_accepted") != Some(&serde_json::Value::Bool(true)) {
        return Err("尚未確認 CLI 訂閱模式的風險告知，請到設定完成確認".to_owned());
    }
    let info = cli::detect_clis()
        .await
        .into_iter()
        .find(|info| info.id == "claude")
        .ok_or_else(|| "找不到 claude CLI，請確認已安裝並登入".to_owned())?;
    let model = cli::tier_override(&config.tier_models, "claude", tier)
        .unwrap_or_else(|| cli::claude_model_for(tier))
        .to_owned();
    Ok(lanes::ClaudeCall {
        program: PathBuf::from(info.path),
        working_dir: cli_workspace(app)?,
        envs: claude_cli_envs(config),
        model,
        usage_log: data_root(app)
            .ok()
            .map(|root| root.join("prompt-cache.jsonl")),
        claude_home: claude_home_dir(),
    })
}

/// 依 preferences.transport 把組裝好的訊息分流到 API 或 CLI，增量經 emit 回呼。
/// assistant_label／cli_closing 供 CLI 攤平使用：角色對話與 GM 導演共用同一條路。
async fn stream_via_transport(
    app: &tauri::AppHandle,
    config: &data::AppConfig,
    transport_override: Option<&str>,
    allow_cli_tools: bool,
    tier: data::Tier,
    world: Option<&str>,
    assistant_label: &str,
    cli_closing: &str,
    messages: &[transport::ChatMessage],
    // 思考增量進不進 emit：只有卡重構的進度字尾開 true（emit＝正文串流的呼叫端一律 false）。
    thinking_to_delta: bool,
    emit: impl FnMut(&str),
) -> Result<String, String> {
    // transport_override：生圖等功能可指定與聊天不同的連線（None＝跟隨 preferences.transport）。
    // allow_cli_tools：只有生圖呼叫為 true——CLI 生圖工具要寫檔／跑指令，聊天一律鎖死工具。
    let transport_kind = transport_override
        .map(str::to_owned)
        .unwrap_or_else(|| chat_transport(config));
    // 每次呼叫的用量落成一行 JSONL（資料目錄的 prompt-cache.jsonl），供額度分頁讀。
    // API 與 CLI 兩條路共用同一份檔案，靠行內的 transport 欄位分辨。
    let usage_log = data_root(app)
        .ok()
        .map(|root| root.join("prompt-cache.jsonl"));
    if transport_kind == "api" {
        let model = transport::resolve_model(tier, config)?;
        return transport::stream_chat(config, &model, messages, usage_log.as_deref(), world, emit)
            .await
            .map_err(|error| error.to_string());
    }

    // CLI 訂閱模式：風險告知未確認前後端直接擋（NewPlan §4.2）
    if config.preferences.get("cli_risk_accepted") != Some(&serde_json::Value::Bool(true)) {
        return Err("尚未確認 CLI 訂閱模式的風險告知，請到設定完成確認".to_owned());
    }
    let info = cli::detect_clis()
        .await
        .into_iter()
        .find(|info| info.id == transport_kind)
        .ok_or_else(|| format!("找不到 {transport_kind} CLI，請確認已安裝並登入"))?;
    let cli_working_dir = cli_workspace(app)?;

    let (system, prompt) = cli::flatten_messages(assistant_label, cli_closing, messages);
    let program = std::path::PathBuf::from(&info.path);
    match transport_kind.as_str() {
        "claude" => {
            let model = cli::tier_override(&config.tier_models, "claude", tier)
                .unwrap_or_else(|| cli::claude_model_for(tier));
            let args = cli::claude_args(model, &system);
            let envs = claude_cli_envs(config);
            cli::run_cli(
                &program,
                &cli_working_dir,
                &args,
                &prompt,
                &envs,
                cli::parse_claude_line,
                thinking_to_delta,
                usage_log.as_deref().map(|path| cli::UsageLog {
                    path,
                    world,
                    transport: "claude",
                    model,
                    parse: cli::parse_claude_usage,
                    lane: None,
                    prompt_tokens_out: None,
                }),
                emit,
            )
            .await
        }
        "codex" => {
            // codex 沒有 system prompt 旗標，併進 prompt 開頭；未覆寫時模型用 CLI 預設
            let model = cli::tier_override(&config.tier_models, "codex", tier);
            let args = cli::codex_args(model, cli::codex_effort_for(tier), allow_cli_tools);
            let combined = format!("{system}\n\n{prompt}");
            cli::run_cli(
                &program,
                &cli_working_dir,
                &args,
                &combined,
                &[],
                cli::parse_codex_line,
                false, // 只有 claude 解析器會產思考增量
                usage_log.as_deref().map(|path| cli::UsageLog {
                    path,
                    world,
                    transport: "codex",
                    model: model.unwrap_or("(CLI 預設)"),
                    parse: cli::parse_codex_usage,
                    lane: None,
                    prompt_tokens_out: None,
                }),
                emit,
            )
            .await
        }
        "agy" => {
            // agy 沒有 system prompt 旗標，併進 prompt 開頭；未覆寫時使用 CLI 預設模型
            let model = cli::tier_override(&config.tier_models, "agy", tier);
            let combined = format!("{system}\n\n{prompt}");
            let args = cli::agy_args(model, &combined, allow_cli_tools);
            let reply = cli::run_cli(
                &program,
                &cli_working_dir,
                &args,
                "",
                &[],
                cli::parse_agy_line,
                false,
                None, // agy 走純文字輸出，拿不到用量（換 JSON 格式才有，未拍板）
                emit,
            )
            .await;
            // 額度分頁至少要看得到「這一輪發生過」，否則 agy 整條路在分頁上等於不存在
            if reply.is_ok() {
                if let Some(path) = usage_log.as_deref() {
                    usage_log::append_unreported(path, world, "agy", model.unwrap_or("(CLI 預設)"));
                }
            }
            reply
        }
        "grok" => {
            // grok 沒有 system prompt 旗標，併進 prompt 開頭；未覆寫時使用 CLI 預設模型
            let model = cli::tier_override(&config.tier_models, "grok", tier);
            let combined = format!("{system}\n\n{prompt}");
            let args = cli::grok_args(model, &combined, allow_cli_tools);
            cli::run_cli(
                &program,
                &cli_working_dir,
                &args,
                "",
                &[],
                cli::parse_grok_line,
                false,
                usage_log.as_deref().map(|path| cli::UsageLog {
                    path,
                    world,
                    transport: "grok",
                    model: model.unwrap_or("(CLI 預設)"),
                    parse: cli::parse_grok_usage,
                    lane: None,
                    prompt_tokens_out: None,
                }),
                emit,
            )
            .await
        }
        other => Err(format!("未知傳輸層：{other}").into()),
    }
    .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageRef {
    DataUrl(String),
    Path(PathBuf),
}

/// 一行裡從路徑起點（POSIX 的「/」或 Windows 的「C:\」）切到最後一個圖片副檔名結尾。
/// 兩邊的工作資料夾都可能帶空格（macOS 的「Application Support」、Windows 帶空格的使用者名），
/// 逐詞切會把路徑攔腰切斷，改用這個切法連空格與尾隨標點一起處理。
fn path_span(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    // 磁碟機字母前不接英數，才不會把「https://…」的「s:/」當成路徑開頭
    let drive = (0..bytes.len()).find(|&index| {
        bytes[index].is_ascii_alphabetic()
            && (index == 0 || !bytes[index - 1].is_ascii_alphanumeric())
            && bytes.get(index + 1) == Some(&b':')
            && matches!(bytes.get(index + 2), Some(b'\\' | b'/'))
    });
    let start = [drive, line.find('/')].into_iter().flatten().min()?;
    let lowered = line.to_ascii_lowercase();
    let end = [".png", ".jpg", ".jpeg", ".webp"]
        .into_iter()
        .filter_map(|extension| lowered.rfind(extension).map(|at| at + extension.len()))
        .filter(|end| *end > start)
        .max()?;
    Some(&line[start..end])
}

/// 回覆裡的圖片候選，依出現順序去重；呼叫端挑第一個真的讀得到的。
/// 先整行切（吃得下含空格的路徑），再退回逐詞切；
/// 前導說明可能緊貼著路徑（「…浮水印。/Users/…png」），所以每個詞另外補一個「從斜線起算」的切法。
pub fn extract_image_refs(text: &str) -> Vec<ImageRef> {
    if let Some(start) = text.find("data:image/") {
        let data_url = text[start..]
            .split(|character: char| {
                character.is_whitespace() || matches!(character, '\'' | '"' | '`')
            })
            .next()
            .unwrap_or("");
        if !data_url.is_empty() {
            return vec![ImageRef::DataUrl(data_url.to_owned())];
        }
    }
    let mut refs = Vec::new();
    for candidate in text
        .lines()
        .filter_map(path_span)
        .chain(text.split_whitespace())
        .flat_map(|token| {
            let token = token.trim_matches(|character: char| {
                matches!(
                    character,
                    '\'' | '"' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | ';'
                )
            });
            let from_slash = token
                .find('/')
                .filter(|start| *start > 0)
                .map(|start| &token[start..]);
            [Some(token), from_slash].into_iter().flatten()
        })
        .filter(|candidate| is_image_extension(std::path::Path::new(candidate)))
    {
        let found = ImageRef::Path(PathBuf::from(candidate));
        if !refs.contains(&found) {
            refs.push(found);
        }
    }
    refs
}

/// 沒抓到圖時附上 CLI 的最後一句（截 200 字）：模型不照暗號時，拒絕理由通常寫在那裡
fn last_sentence(reply: &str) -> Option<String> {
    let line = reply
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .next_back()?;
    Some(line.chars().take(200).collect())
}

fn is_image_extension(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .is_some_and(|extension| matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "webp"))
}

/// CLI 沙盒只能寫工作目錄，生圖時會把圖搬進來，只為了回一個 app 讀得到的路徑。
/// 圖讀進圖庫後這份就沒用了：三家 CLI 一律在生圖收尾清掉（含失敗那次留下的），免得越堆越多。
/// CLI 會自己開子目錄（codex 的 output/imagegen/），所以往下遞迴；清空的目錄順手移除。
fn clear_cli_workspace_images(workspace: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(workspace) else {
        return;
    };
    for path in entries.flatten().map(|entry| entry.path()) {
        if path.is_dir() {
            clear_cli_workspace_images(&path);
            let _ = std::fs::remove_dir(&path); // 只有真的空了才成功，留有其他檔案的目錄不動
        } else if is_image_extension(&path) {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        output.push(TABLE[((value >> 18) & 0x3f) as usize] as char);
        output.push(TABLE[((value >> 12) & 0x3f) as usize] as char);
        output.push(if chunk.len() > 1 {
            TABLE[((value >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(value & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    output
}

fn decode_base64(value: &str) -> Result<Vec<u8>, String> {
    fn sextet(byte: u8) -> Option<u8> {
        match byte {
            b'A'..=b'Z' => Some(byte - b'A'),
            b'a'..=b'z' => Some(byte - b'a' + 26),
            b'0'..=b'9' => Some(byte - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }

    let bytes = value.as_bytes();
    if bytes.len() % 4 != 0 {
        return Err("非法 base64 資料".to_owned());
    }
    let mut output = Vec::with_capacity(bytes.len() / 4 * 3);
    for (index, chunk) in bytes.chunks_exact(4).enumerate() {
        let padding = chunk.iter().rev().take_while(|&&byte| byte == b'=').count();
        if padding > 2 || (padding > 0 && index + 1 != bytes.len() / 4) {
            return Err("非法 base64 資料".to_owned());
        }
        let a = sextet(chunk[0]).ok_or_else(|| "非法 base64 資料".to_owned())?;
        let b = sextet(chunk[1]).ok_or_else(|| "非法 base64 資料".to_owned())?;
        let c = if padding >= 2 {
            0
        } else {
            sextet(chunk[2]).ok_or_else(|| "非法 base64 資料".to_owned())?
        };
        let d = if padding >= 1 {
            0
        } else {
            sextet(chunk[3]).ok_or_else(|| "非法 base64 資料".to_owned())?
        };
        if (padding >= 1 && chunk[3] != b'=') || (padding >= 2 && chunk[2] != b'=') {
            return Err("非法 base64 資料".to_owned());
        }
        let decoded =
            (u32::from(a) << 18) | (u32::from(b) << 12) | (u32::from(c) << 6) | u32::from(d);
        output.push((decoded >> 16) as u8);
        if padding < 2 {
            output.push((decoded >> 8) as u8);
        }
        if padding == 0 {
            output.push(decoded as u8);
        }
    }
    Ok(output)
}

fn validate_gallery_component(value: &str, require_png: bool) -> Result<(), String> {
    if value.is_empty()
        || value.contains("..")
        || value.contains('/')
        || value.contains('\\')
        || (require_png && !value.ends_with(".png"))
    {
        return Err("非法檔名".to_owned());
    }
    Ok(())
}

fn gallery_directory(
    root: &std::path::Path,
    world_id: &str,
    character_id: &str,
) -> Result<PathBuf, String> {
    data::gallery_dir(root, world_id, character_id).map_err(|error| error.to_string())
}

fn list_gallery_image_files(
    root: &std::path::Path,
    world_id: &str,
    character_id: &str,
) -> Result<Vec<String>, String> {
    let directory = gallery_directory(root, world_id, character_id)?;
    let mut files = match std::fs::read_dir(directory) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|file| file.ends_with(".png"))
            .collect::<Vec<_>>(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.to_string()),
    };
    files.sort_unstable_by(|left, right| right.cmp(left));
    Ok(files)
}

fn save_generated_gallery_image(
    root: &std::path::Path,
    world_id: &str,
    character_id: &str,
    data_url: &str,
) -> Result<(), String> {
    let Some((header, encoded)) = data_url.split_once(',') else {
        return Ok(());
    };
    if !header.starts_with("data:") || !header.ends_with(";base64") {
        return Ok(());
    }
    let directory = gallery_directory(root, world_id, character_id)?;
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    std::fs::write(
        directory.join(format!("{timestamp}.png")),
        decode_base64(encoded)?,
    )
    .map_err(|error| error.to_string())
}

fn image_file_data_url(path: &std::path::Path) -> Result<String, String> {
    let mime = match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => return Err("不支援的圖片格式".to_owned()),
    };
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    Ok(format!("data:{mime};base64,{}", encode_base64(&bytes)))
}

/// 角色名與描述由編輯器直接傳進來（不讀也不寫卡片檔）：新卡還沒存檔就能生圖，
/// 且吃到的是編輯器裡的當下內容；追加描寫由前端存進草稿，跟其他欄位一起按儲存才落地。
/// character_id 前端已先跟 new_id 要好，決定圖庫路徑；name 只進提示詞。
#[tauri::command]
async fn generate_character_image(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    name: String,
    description: String,
    extra_prompt: String,
    source: Option<String>,
    framing: Option<String>,
) -> Result<String, String> {
    let root = data_root(&app)?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    // 構圖二選一：half＝半身特寫，其餘一律全身（含舊前端沒傳的情況）
    let shot = match framing.as_deref() {
        Some("half") => "waist-up half-body",
        _ => "full-body",
    };
    let mut prompt = format!(
        "Generate a single {shot} character illustration, portrait orientation 2:3. No text, no watermark, plain background.\nCharacter name: {name}\nCharacter description:\n{description}"
    );
    if !extra_prompt.trim().is_empty() {
        prompt.push_str(&format!(
            "\nAdditional art direction (takes priority over the defaults above): {extra_prompt}"
        ));
    }
    // 生圖來源可與聊天連線分開選（source 覆寫；空值＝跟隨 preferences.transport）
    let transport_kind = source
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            config
                .preferences
                .get("transport")
                .and_then(|value| value.as_str())
                .unwrap_or("api")
                .to_owned()
        });
    if transport_kind == "api" {
        let image = transport::generate_image(&config, &prompt).await?;
        save_generated_gallery_image(&root, &world_id, &character_id, &image)?;
        return Ok(image);
    }
    // CLI 一律照送：能生圖的家（codex $imagegen／agy／grok）會存檔回路徑，其餘掃不到圖就失敗
    // 兩個暗號分開問：生不出（沒能力／沒額度）與不肯生（內容規範）要給玩家不同的下一步
    prompt.push_str(
        "\nIf you are able to generate images, generate it now, save it as a PNG file, and reply with the absolute file path of the saved image. If you cannot generate images at all, reply exactly: NO_IMAGE. If you decline this particular request, reply exactly: REFUSED",
    );
    if transport_kind == "codex" {
        prompt = format!("$imagegen {prompt}");
    }
    let messages = [transport::ChatMessage {
        role: "user".to_owned(),
        content: prompt,
    }];
    let reply = stream_via_transport(
        &app,
        &config,
        Some(&transport_kind),
        true,
        transport::gm_tier(&config),
        Some(&world_id),
        "",
        "",
        &messages,
        false,
        |_| {},
    )
    .await?;
    let workspace = cli_workspace(&app)?;
    let found = extract_image_refs(&reply)
        .into_iter()
        .find_map(|found| match found {
            ImageRef::DataUrl(data_url) => Some(Ok(data_url)),
            // CLI 常回相對於自己工作目錄的路徑（codex 的 imagegen 存進 output/imagegen/）；
            // 補上基準才讀得到，絕對路徑 join 後維持原樣
            ImageRef::Path(path) => {
                let path = workspace.join(path);
                std::fs::metadata(&path)
                    .is_ok()
                    .then(|| image_file_data_url(&path))
            }
        })
        // REFUSED／NO_IMAGE 是上面 prompt 跟 CLI 約好的暗號，前端據此各換一句人話；
        // 兩個都沒對上時附最後一句原話，模型不照暗號時的拒絕理由通常就寫在那
        .unwrap_or_else(|| {
            Err(if reply.contains("REFUSED") {
                "REFUSED：來源拒絕生成這段內容".to_owned()
            } else if reply.contains("NO_IMAGE") {
                "NO_IMAGE：來源回報無法生圖".to_owned()
            } else {
                match last_sentence(&reply) {
                    Some(tail) => format!("回覆中沒有圖片：{tail}"),
                    None => "回覆中沒有圖片".to_owned(),
                }
            })
        });
    // 圖已經讀進記憶體，中轉檔失去用途；成功與失敗都清
    clear_cli_workspace_images(&workspace);
    let image = found?;
    save_generated_gallery_image(&root, &world_id, &character_id, &image)?;
    Ok(image)
}

#[tauri::command]
fn list_gallery_images(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
) -> Result<Vec<String>, String> {
    list_gallery_image_files(&data_root(&app)?, &world_id, &character_id)
}

#[tauri::command]
fn read_gallery_image(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    file: String,
) -> Result<String, String> {
    validate_gallery_component(&file, true)?;
    let directory = gallery_directory(&data_root(&app)?, &world_id, &character_id)?;
    image_file_data_url(&directory.join(file))
}

#[tauri::command]
fn delete_gallery_image(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    file: String,
) -> Result<(), String> {
    validate_gallery_component(&file, true)?;
    let directory = gallery_directory(&data_root(&app)?, &world_id, &character_id)?;
    std::fs::remove_file(directory.join(file)).map_err(|error| error.to_string())
}

/// 上下文組裝→單發呼叫→串流回傳（KICKOFF §4）。
/// 上下文完全由本機正典（角色卡＋可見世界書＋公開 transcript）經 assemble_messages 組裝，
/// 再依 preferences.transport 分流到 API 或 CLI；增量文字經 on_delta channel 回前端。
#[tauri::command]
async fn chat_with_character(
    app: tauri::AppHandle,
    world_id: String,
    character_id: String,
    on_delta: tauri::ipc::Channel<String>,
) -> Result<String, String> {
    let root = data_root(&app)?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let card =
        data::read_character(&root, &world_id, &character_id).map_err(|error| error.to_string())?;
    let state = data::read_state(&root, &world_id).map_err(|error| error.to_string())?;
    let events = data::read_transcript(&root, &world_id, state.current_scene)
        .map_err(|error| error.to_string())?;
    let worldbook = data::read_worldbook(&root, &world_id).map_err(|error| error.to_string())?;

    let player = data::read_player_card(&root, &world_id).map_err(|error| error.to_string())?;
    // 角色自己那支的狀態（面板指認優先，其次同名比對），只有這條分支會塞進提示詞。
    let branch = transport::resolve_branch(
        &state.state.tree,
        &state.branch_bindings,
        &character_id,
        &card.name,
    );
    let emit = |delta: &str| {
        let _ = on_delta.send(delta.to_owned());
    };
    // claude 訂閱走 resume 續聊線（prompt-cache-optimization 包 2）：全角色共用一條 session，
    // 凍結 system 逐輪重帶、只送新事件；私設回合注入、回合後從 session 檔抹掉（案 C）。
    if chat_transport(&config) == "claude" {
        let lang = transport::ui_language(&config);
        let cards = load_active_cards(&root, &world_id)?;
        let frozen = transport::chars_lane_system(&cards, player.as_ref(), &worldbook, &lang);
        let turn = transport::chars_lane_turn(
            &card,
            player.as_ref(),
            &events,
            &worldbook,
            &state.state,
            &state.mechanism,
            branch.as_deref(),
            &lang,
        );
        let call = prepare_claude_call(&app, &config, card.tier).await?;
        return lanes::run_turn(
            &call,
            &root,
            &world_id,
            lanes::TurnInput {
                lane: lanes::Lane::Chars,
                scene: state.current_scene,
                events: &events,
                frozen_system: frozen,
                tail: turn.tail,
                confidential: turn.confidential,
                prefix: Some(format!("{}：", card.name)),
                echo: lanes::ReplyEcho::Dialogue {
                    speaker_id: card.id.clone(),
                },
            },
            emit,
        )
        .await;
    }
    let messages = transport::assemble_messages(
        &card,
        player.as_ref(),
        &events,
        &worldbook,
        &state.state,
        &state.mechanism,
        branch.as_deref(),
        &transport::ui_language(&config),
    );
    let closing = format!(
        "現在輪到「{}」回應。請直接以角色視角輸出台詞、動作與心理描寫，不要加名字前綴、不要任何角色之外的說明。",
        card.name
    );
    stream_via_transport(
        &app,
        &config,
        None,
        false,
        card.tier,
        Some(&world_id),
        &card.name,
        &closing,
        &messages,
        false,
        emit,
    )
    .await
}

/// 這一桌未封存、也沒被自動隱藏的角色卡（GM 上下文與 chars 續聊線的快照都要全卡）；
/// auto_hidden 的卡在別桌上場前先不進凍結快照，見 record_card_arrivals／load_hidden_cards。
fn load_active_cards(
    root: &std::path::Path,
    world_id: &str,
) -> Result<Vec<data::CharacterCard>, String> {
    data::list_characters(root, world_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|meta| !meta.archived && !meta.auto_hidden)
        .map(|meta| {
            data::read_character(root, world_id, &meta.id).map_err(|error| error.to_string())
        })
        .collect()
}

/// 這一桌自動隱藏、且未手動封存的角色卡（回合登場檢測用，AI 卡重構包 4b）；
/// 與 load_active_cards 互補，present 名單命中就是「回歸」。
fn load_hidden_cards(
    root: &std::path::Path,
    world_id: &str,
) -> Result<Vec<data::CharacterCard>, String> {
    data::list_characters(root, world_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|meta| meta.auto_hidden && !meta.archived)
        .map(|meta| {
            data::read_character(root, world_id, &meta.id).map_err(|error| error.to_string())
        })
        .collect()
}

/// GM 上下文素材＝world.md＋世界書＋全部角色卡（含私有）＋公開 transcript（NewPlan §7.0）。
/// 單發組裝與 claude lane 續聊共用同一份素材。
struct GmMaterials {
    world_md: String,
    worldbook: Vec<data::WorldbookEntry>,
    state: data::WorldState,
    events: Vec<data::TranscriptEvent>,
    cards: Vec<data::CharacterCard>,
    player: Option<data::CharacterCard>,
}

fn gm_materials(root: &std::path::Path, world_id: &str) -> Result<GmMaterials, String> {
    let state = data::read_state(root, world_id).map_err(|error| error.to_string())?;
    let events = data::read_transcript(root, world_id, state.current_scene)
        .map_err(|error| error.to_string())?;
    Ok(GmMaterials {
        world_md: data::read_world_md(root, world_id).map_err(|error| error.to_string())?,
        worldbook: data::read_worldbook(root, world_id).map_err(|error| error.to_string())?,
        state,
        events,
        cards: load_active_cards(root, world_id)?,
        player: data::read_player_card(root, world_id).map_err(|error| error.to_string())?,
    })
}

/// GM lane 的一輪：凍結 system（GM 指示＋world.md＋全 constant＋全卡）＋回合尾段
/// （keyword 條目＋狀態＋導演指示）。narrate 與 suggest 共用，差別只在指示與 echo。
#[allow(clippy::too_many_arguments)]
async fn gm_lane_reply(
    app: &tauri::AppHandle,
    config: &data::AppConfig,
    root: &std::path::Path,
    world_id: &str,
    materials: &GmMaterials,
    scope: &transport::StateScope,
    instruction: &str,
    echo: lanes::ReplyEcho,
    lang: &str,
    emit: impl FnMut(&str),
) -> Result<String, String> {
    let frozen = transport::gm_lane_system(
        &materials.world_md,
        &materials.cards,
        materials.player.as_ref(),
        &materials.worldbook,
        &materials.state.mechanism,
        lang,
    );
    let turn = transport::gm_lane_turn(
        &materials.events,
        &materials.worldbook,
        materials.player.as_ref(),
        &materials.state.state,
        &materials.state.mechanism,
        scope,
        instruction,
        lang,
    );
    let call = prepare_claude_call(app, config, transport::gm_tier(config)).await?;
    lanes::run_turn(
        &call,
        root,
        world_id,
        lanes::TurnInput {
            lane: lanes::Lane::Gm,
            scene: materials.state.current_scene,
            events: &materials.events,
            frozen_system: frozen,
            tail: turn.tail,
            confidential: None,
            prefix: None,
            echo,
        },
        emit,
    )
    .await
}

/// gm_narrate 回傳：剝乾淨的旁白顯示文字＋下一位發言者（角色 id 或玩家哨兵）。
/// GM 沒點名或名字對不上名單＝None，前端就地停下、不當錯誤。
#[derive(Serialize)]
struct GmNarration {
    text: String,
    /// 剝殼前的模型原文；與 text 相同就是 None，前端照樣存進事件裡
    raw: Option<String>,
    next: Option<String>,
    /// 長文字欄這一輪的新值；前端接到就補一則 system 事件進 transcript。空的就不補。
    state_updates: Vec<StateUpdate>,
    /// 這輪剛回歸（登場）的角色卡 id（AI 卡重構包 4b）；前端拿它把卡從隱藏區搬回主區。
    #[serde(default)]
    arrived_characters: Vec<String>,
    /// 這輪剛登場的世界書人物 title（4a 就有登場事件，4b 才在回傳裡帶出來）。
    #[serde(default)]
    arrived_persons: Vec<String>,
}

#[derive(Serialize)]
struct StateUpdate {
    path: String,
    value: String,
}

/// 簡易導演：GM 旁白＋點名一次呼叫完成（NewPlan §6.1＋快取包 5）。
/// 旁白串流回前端後由前端落 transcript；點名行與狀態欄在這裡剝掉，不進顯示文字。
#[tauri::command]
async fn gm_narrate(
    app: tauri::AppHandle,
    world_id: String,
    on_delta: tauri::ipc::Channel<String>,
) -> Result<GmNarration, String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let root = data_root(&app)?;
    let materials = gm_materials(&root, &world_id)?;
    let roster: Vec<String> = materials
        .cards
        .iter()
        .map(|card| card.name.clone())
        .collect();
    let player_name = materials.player.as_ref().map(|card| card.name.as_str());
    // 換幕後第一輪 GM 回合送整棵樹對齊；之後每輪只送在場分支＋變動標記（狀態欄二期包 5）。
    let align = materials.state.mechanism.incremental
        && materials.state.aligned_scene != Some(materials.state.current_scene);
    let scope = transport::state_scope(
        &materials.state.state,
        &materials.state.mechanism,
        &materials.cards,
        materials.player.as_ref(),
        &materials.state.branch_bindings,
        align,
    );
    // 卡片自帶介面時，卡片自己規定了輸出格式，導演指示要讓路，否則模型會照我們的旁白規矩寫，介面永遠對不上。
    let card_scripts: Vec<import::InterfaceScript> = import::read_card_interfaces(&root, &world_id)
        .unwrap_or_default()
        .into_iter()
        .filter(|interface| interface.unsupported.is_none())
        .flat_map(|interface| interface.scripts)
        .collect();
    let (instruction_message, closing) = if card_scripts.is_empty() {
        let instruction_message = transport::narrate_instruction(&lang, &roster, player_name);
        let closing = if roster.is_empty() {
            "現在請以 GM 身分執行上述導演指示，只輸出旁白本文與要求的狀態欄，不要加名字前綴。"
        } else {
            "現在請以 GM 身分執行上述導演指示，只輸出旁白本文、要求的狀態欄與「下一位」行，不要加名字前綴。"
        };
        (instruction_message, closing)
    } else {
        let entry_title = import::card_format_entry(&card_scripts, &materials.worldbook);
        let instruction_message = transport::card_format_instruction(&lang, entry_title.as_deref());
        let closing =
            "現在請以 GM 身分，完全依照上述輸出格式產生本回合的回覆，不要加名字前綴，也不要輸出格式以外的任何內容。";
        (instruction_message, closing)
    };
    let emit = |delta: &str| {
        let _ = on_delta.send(delta.to_owned());
    };
    let reply = if chat_transport(&config) == "claude" {
        let instruction = format!("{}\n{closing}", instruction_message.content);
        gm_lane_reply(
            &app,
            &config,
            &root,
            &world_id,
            &materials,
            &scope,
            &instruction,
            lanes::ReplyEcho::Narration,
            &lang,
            emit,
        )
        .await?
    } else {
        let mut messages = transport::assemble_gm_messages(
            &materials.world_md,
            &materials.cards,
            materials.player.as_ref(),
            &materials.events,
            &materials.worldbook,
            &materials.state.state,
            &materials.state.mechanism,
            &scope,
            &lang,
        );
        messages.push(instruction_message);
        stream_via_transport(
            &app,
            &config,
            None,
            false,
            transport::gm_tier(&config),
            Some(&world_id),
            "GM",
            closing,
            &messages,
            false,
            emit,
        )
        .await?
    };
    let block = transport::extract_state_block(&reply);
    let (next_raw, display) = transport::extract_next_speaker(&block.display);
    let mut state_updates = Vec::new();
    let mut arrived_persons = Vec::new();
    let mut arrived_characters = Vec::new();
    // 狀態更新一律盡力而為：模型格式壞掉或存檔寫不進去，都不該害玩家丟掉整段旁白。
    // 骰值要每回合重擲，就算這一輪模型完全沒吐更新也要跑一次。
    if !block.fields.is_empty()
        || !block.updates.is_empty()
        || materials.state.mechanism.incremental
    {
        let user_name = player_name.unwrap_or_else(|| transport::player_fallback_name(&lang));
        if let Ok(mut state) = data::read_state(&root, &world_id) {
            let scene = state.current_scene;
            let outcome = mechanism::apply_block(&mut state, &block, user_name);
            if align {
                state.aligned_scene = Some(scene);
            }
            if data::write_state(&root, &world_id, &state).is_ok() {
                mechanism::append_log(&root, &world_id, scene, &outcome.records);
                state_updates =
                    transport::snapshot_updates(&state.state, &state.mechanism, user_name)
                        .into_iter()
                        .map(|(path, value)| StateUpdate { path, value })
                        .collect();
                let present = state.state.table.get("present").map(String::as_str);
                // 人物在場登場（AI 卡重構包 4a）：present 套用後檢查新面孔，
                // 命中就把世界書全文記進歷史，system 那邊只留一行名冊。
                arrived_persons = record_person_arrivals(
                    &root,
                    &world_id,
                    scene,
                    &materials.worldbook,
                    &materials.events,
                    present,
                    &display,
                    user_name,
                );
                // 角色卡自動回歸（AI 卡重構包 4b）：鏡射人物登場，鍵換成卡名；
                // auto_hidden 欄位本身不在這裡動，只在換幕結算（data::begin_next_scene）。
                if let Ok(hidden_cards) = load_hidden_cards(&root, &world_id) {
                    arrived_characters = record_card_arrivals(
                        &root,
                        &world_id,
                        scene,
                        &hidden_cards,
                        &materials.events,
                        present,
                        &display,
                        user_name,
                    );
                }
            }
        }
    }
    // LLM 只認名字，點名後對回角色 id（同名取第一個）；玩家哨兵原樣回傳
    let next = next_raw
        .and_then(|raw| transport::pick_speaker(&raw, &roster, player_name))
        .and_then(|picked| {
            if picked == transport::PLAYER_SENTINEL {
                return Some(picked);
            }
            materials
                .cards
                .iter()
                .find(|card| card.name == picked)
                .map(|card| card.id.clone())
        });
    Ok(GmNarration {
        raw: (reply != display).then_some(reply),
        text: display,
        next,
        state_updates,
        arrived_characters,
        arrived_persons,
    })
}

/// 世界書人物條目首次在場（AI 卡重構包 4a）：present 名單（缺席就退回本文比對）比對得上、
/// 本幕還沒登場過的 is_person 條目，逐一把全文 append 成一則系統事件；同一人本幕只記一次，
/// 離場不拔。寫檔失敗一律吞掉，登場記錄不該反過來中斷旁白。回傳這輪實際記上的標題清單
/// （成功寫檔才算），供 gm_narrate 回傳給前端本地移區。
/// visibility 非 Public 的條目帶 gm_only=true（包 4b）：這種條目原本就限定 GM 或特定角色
/// 看得到，全文登場事件不能透過 chars 續聊線的共用歷史洩漏給所有角色。
#[allow(clippy::too_many_arguments)]
fn record_person_arrivals(
    root: &std::path::Path,
    world_id: &str,
    scene: u64,
    worldbook: &[data::WorldbookEntry],
    events: &[data::TranscriptEvent],
    present: Option<&str>,
    reply_body: &str,
    user_name: &str,
) -> Vec<String> {
    let already = transport::appeared_person_titles(events);
    let arrivals = transport::detect_new_arrivals(worldbook, present, reply_body, &already);
    if arrivals.is_empty() {
        return Vec::new();
    }
    let ts = data::local_timestamp().unwrap_or_default();
    let mut titles = Vec::new();
    for entry in arrivals {
        let event = data::TranscriptEvent {
            ts: ts.clone(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: data::TranscriptKind::System,
            text: transport::person_arrival_text(entry, user_name),
            raw: None,
            state: None,
            gm_only: !matches!(entry.visibility, data::Visibility::Public),
        };
        if data::append_transcript(root, world_id, scene, &event).is_ok() {
            titles.push(entry.title.clone());
        }
    }
    titles
}

/// 角色卡自動回歸（AI 卡重構包 4b）：present 名單（缺席就退回本文比對）比對得上、
/// 本幕還沒回歸過的 auto_hidden 卡，逐一把完整設定 append 成一則系統事件；同一張卡
/// 本幕只記一次。鏡射 record_person_arrivals，鍵從世界書 title 換成卡片 name；
/// **不改 auto_hidden 欄位本身**（鐵律：持久欄位只在換幕結算，見 data::begin_next_scene）。
/// chars 快照本來就含全卡，回歸事件不算新洩漏，一律 gm_only=false。
/// 回傳這輪實際記上的卡 id 清單（成功寫檔才算）。
#[allow(clippy::too_many_arguments)]
fn record_card_arrivals(
    root: &std::path::Path,
    world_id: &str,
    scene: u64,
    hidden_cards: &[data::CharacterCard],
    events: &[data::TranscriptEvent],
    present: Option<&str>,
    reply_body: &str,
    user_name: &str,
) -> Vec<String> {
    let already = transport::appeared_card_names(events);
    let arrivals = transport::detect_new_card_arrivals(hidden_cards, present, reply_body, &already);
    if arrivals.is_empty() {
        return Vec::new();
    }
    let ts = data::local_timestamp().unwrap_or_default();
    let mut ids = Vec::new();
    for card in arrivals {
        let event = data::TranscriptEvent {
            ts: ts.clone(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: data::TranscriptKind::System,
            text: transport::card_arrival_text(card, user_name),
            raw: None,
            state: None,
            gm_only: false,
        };
        if data::append_transcript(root, world_id, scene, &event).is_ok() {
            ids.push(card.id.clone());
        }
    }
    ids
}

/// 目前設定實際會用到的（來源, 模型），供額度分頁標出「使用中」。
/// 看的是解析後真正傳給連線的模型字串，與 log 的欄位同一把尺。
fn current_models(config: &data::AppConfig) -> Vec<(String, String)> {
    let source = chat_transport(config);
    let tiers = [data::Tier::Best, data::Tier::Balanced, data::Tier::Fast];
    tiers
        .into_iter()
        .filter_map(|tier| {
            let model = match source.as_str() {
                "api" => transport::resolve_model(tier, config).ok()?,
                "claude" => cli::tier_override(&config.tier_models, "claude", tier)
                    .unwrap_or_else(|| cli::claude_model_for(tier))
                    .to_owned(),
                cli_id => cli::tier_override(&config.tier_models, cli_id, tier)
                    .unwrap_or("(CLI 預設)")
                    .to_owned(),
            };
            Some((source.clone(), model))
        })
        .collect()
}

/// 額度分頁（包 6）：把資料目錄的 prompt-cache.jsonl 彙總成報表。
/// world_id 為 None＝所有桌總計；空字串＝未標桌（加桌欄位之前的舊紀錄）。
#[tauri::command]
fn usage_report(
    app: tauri::AppHandle,
    world_id: Option<String>,
) -> Result<usage_report::UsageReport, String> {
    let root = data_root(&app)?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    // 讀不到（還沒跑過任何呼叫）就當空檔，分頁顯示「還沒有紀錄」
    let log = std::fs::read_to_string(root.join("prompt-cache.jsonl")).unwrap_or_default();
    let names: Vec<(String, String)> = data::list_worlds(&root)
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|world| (world.id, world.name))
        .collect();
    Ok(usage_report::summarize(
        &log,
        world_id.as_deref(),
        &names,
        &current_models(&config),
    ))
}

/// 保溫 ping（包 7）：玩家還在、快取快到期時由前端呼叫，替這桌每條活著的線刷新五分鐘壽命。
/// 回傳實際保溫的線數——claude 以外的傳輸、或這桌還沒開過線時回 0，前端據此不再重試。
#[tauri::command]
async fn keepalive_lanes(app: tauri::AppHandle, world_id: String) -> Result<usize, String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    if chat_transport(&config) != "claude" {
        return Ok(0);
    }
    let root = data_root(&app)?;
    let call = prepare_claude_call(&app, &config, transport::gm_tier(&config)).await?;
    lanes::keepalive(&call, &root, &world_id).await
}

/// 換場：把當前場景公開紀錄壓成一則摘要，寫進新場景開頭，current_scene +1（NewPlan 換場＋場景摘要）。
/// 摘要走既有 stream_via_transport＋GM 檔位，不新開連線路徑、不新增設定項。
#[tauri::command]
async fn advance_scene(app: tauri::AppHandle, world_id: String) -> Result<u64, String> {
    let root = data_root(&app)?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let state = data::read_state(&root, &world_id).map_err(|error| error.to_string())?;
    let events = data::read_transcript(&root, &world_id, state.current_scene)
        .map_err(|error| error.to_string())?;
    if events.is_empty() {
        return Err("這個場景還沒有任何紀錄，沒東西可以換場".to_owned());
    }

    let messages = transport::summary_messages(&events, &lang);
    let reply = stream_via_transport(
        &app,
        &config,
        None,
        false,
        transport::gm_tier(&config),
        Some(&world_id),
        "GM",
        "現在請執行上述導演指示，只輸出摘要本文，不要加名字前綴。",
        &messages,
        false,
        |_| {},
    )
    .await?;

    // 換幕順手取幕名：回覆第一行「標題：…」／「Title: …」解析不到就整段當摘要，不報錯
    let (title, summary) = transport::extract_scene_title(&reply);
    data::begin_next_scene(&root, &world_id, &summary, &lang, title.as_deref())
        .map_err(|error| error.to_string())
}

/// 退回前幕：換幕的精確反向操作，純本地檔案處理不必等模型回覆。
#[tauri::command]
fn revert_scene(app: tauri::AppHandle, world_id: String) -> Result<u64, String> {
    let root = data_root(&app)?;
    data::revert_scene(&root, &world_id).map_err(|error| error.to_string())
}

/// 從前幕分岔：把那一幕的紀錄複製成新的一幕接著玩，純本地檔案處理不必等模型回覆。
#[tauri::command]
fn fork_scene(app: tauri::AppHandle, world_id: String, scene: u64) -> Result<u64, String> {
    let root = data_root(&app)?;
    data::fork_scene(&root, &world_id, scene).map_err(|error| error.to_string())
}

/// 重寫前情提要：結構照 advance_scene，差別是摘要對象換成「前一幕」的紀錄，
/// 換出來的文字覆寫目前這幕既有的那則摘要，而不是開一個新場景。
#[tauri::command]
async fn regenerate_scene_summary(app: tauri::AppHandle, world_id: String) -> Result<(), String> {
    let root = data_root(&app)?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let state = data::read_state(&root, &world_id).map_err(|error| error.to_string())?;
    let scene = state.current_scene;
    let label = data::scene_label(&state, scene);
    let Some(previous_scene) = label.parent else {
        return Err("第一幕沒有前情提要可以重寫".to_owned());
    };
    if label.forked {
        return Err("這一幕是從前幕接續來的，開頭不是前情提要".to_owned());
    }
    let current_events =
        data::read_transcript(&root, &world_id, scene).map_err(|error| error.to_string())?;
    if current_events.len() != 1 {
        // 早退：這一幕已經有新內容，不值得先花一次模型呼叫才發現不能用
        return Err("這一幕已經有新內容，不能重寫前情提要".to_owned());
    }

    let previous_events = data::read_transcript(&root, &world_id, previous_scene)
        .map_err(|error| error.to_string())?;
    if previous_events.is_empty() {
        return Err("前一幕還沒有任何紀錄，沒東西可以重新摘要".to_owned());
    }

    let messages = transport::summary_messages(&previous_events, &lang);
    let reply = stream_via_transport(
        &app,
        &config,
        None,
        false,
        transport::gm_tier(&config),
        Some(&world_id),
        "GM",
        "現在請執行上述導演指示，只輸出摘要本文，不要加名字前綴。",
        &messages,
        false,
        |_| {},
    )
    .await?;

    let (title, summary) = transport::extract_scene_title(&reply);
    data::replace_scene_summary(&root, &world_id, &summary, &lang, title.as_deref())
        .map_err(|error| error.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OutlineOutcome {
    parsed: Option<genesis::Outline>,
    raw: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CharacterOutcome {
    parsed: Option<genesis::OutlineCharacter>,
    raw: String,
}

#[tauri::command]
async fn generate_table_outline(
    app: tauri::AppHandle,
    input: String,
    genres: Vec<String>,
) -> Result<OutlineOutcome, String> {
    let input = input.trim();
    if input.is_empty() && genres.is_empty() {
        return Err("EMPTY_INPUT".to_owned());
    }
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let messages = genesis::outline_messages(input, &genres, &lang);
    let raw = stream_via_transport(
        &app,
        &config,
        None,
        false,
        transport::gm_tier(&config),
        None,
        "GM",
        "Generate the campaign outline exactly in the requested structure.",
        &messages,
        false,
        |_| {},
    )
    .await?;
    Ok(OutlineOutcome {
        parsed: genesis::parse_outline(&raw),
        raw,
    })
}

#[tauri::command]
async fn generate_table_character(
    app: tauri::AppHandle,
    input: String,
    genres: Vec<String>,
    outline_raw: String,
    hint: String,
) -> Result<CharacterOutcome, String> {
    let input = input.trim();
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let messages = genesis::character_messages(input, &genres, &outline_raw, &hint, &lang);
    let raw = stream_via_transport(
        &app,
        &config,
        None,
        false,
        transport::gm_tier(&config),
        None,
        "GM",
        "Generate exactly one character in the requested structure.",
        &messages,
        false,
        |_| {},
    )
    .await?;
    Ok(CharacterOutcome {
        parsed: genesis::parse_character(&raw),
        raw,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExpandOutcome {
    world_id: Option<String>,
    raw: String,
}

#[tauri::command]
async fn generate_table_expand(
    app: tauri::AppHandle,
    input: String,
    genres: Vec<String>,
    outline_raw: String,
) -> Result<ExpandOutcome, String> {
    let input = input.trim();
    if input.is_empty() && genres.is_empty() {
        return Err("EMPTY_INPUT".to_owned());
    }
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let messages = genesis::expand_messages(input, &genres, &outline_raw, &lang);
    let raw = stream_via_transport(
        &app,
        &config,
        None,
        false,
        transport::gm_tier(&config),
        None,
        "GM",
        "Generate the full campaign materials exactly in the requested structure.",
        &messages,
        false,
        |_| {},
    )
    .await?;
    let world_id = genesis::parse_expand(&raw)
        .map(|expanded| genesis::materialize(&data_root(&app)?, &expanded))
        .transpose()
        .map_err(|error| error.to_string())?;
    // 開桌這幾次呼叫的額度就是為這桌花的：桌一建好就把剛才那些行認領回來（見 usage_log）
    if let (Some(id), Ok(root)) = (&world_id, data_root(&app)) {
        usage_log::assign_pending_world(&root.join("prompt-cache.jsonl"), id);
    }
    Ok(ExpandOutcome { world_id, raw })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            list_worlds,
            create_world,
            create_sample_world,
            new_id,
            reclaim_world_if_empty,
            rename_world,
            delete_world,
            read_world_md,
            write_world_md,
            read_worldbook,
            upsert_worldbook_entry,
            reorder_worldbook_entries,
            delete_worldbook_entry,
            mechanism_ledger,
            worldbook_entry_to_character,
            character_to_worldbook_entry,
            world_has_state_bar,
            card_openings,
            import_worldbook,
            dedupe_worldbook,
            export_worldbook,
            list_characters,
            reorder_characters,
            read_character,
            write_character,
            set_character_archived,
            set_character_auto_hidden,
            delete_character,
            probe_import,
            card_interfaces,
            import_character,
            list_import_receipts,
            undo_last_import,
            record_import_rename,
            refactor_apply,
            refactor_survey,
            refactor_assemble_local,
            refactor_expand,
            refactor_expand_person,
            refactor_expand_spans,
            refactor_absorb_entry,
            refactor_split_group,
            refactor_abort,
            refactor_interface_shell,
            refactor_export_outcome,
            refactor_export_saved,
            refactor_outcome_exists,
            export_character,
            read_character_image,
            save_character_image,
            delete_character_image,
            read_character_avatar,
            save_character_avatar,
            delete_character_avatar,
            read_gm_image,
            append_transcript,
            post_opening,
            translate_opening,
            read_transcript,
            scene_appearances,
            pop_transcript,
            export_transcript,
            export_scene,
            read_state,
            write_state,
            set_table_state,
            set_state_path,
            set_branch_binding,
            branch_bindings,
            mark_state_counter,
            read_config,
            write_config,
            detect_clis,
            install_cli,
            cli_verified,
            sponsor_status,
            import_sponsor_pack,
            list_cli_models,
            chat_with_character,
            generate_character_image,
            list_gallery_images,
            read_gallery_image,
            delete_gallery_image,
            gm_narrate,
            keepalive_lanes,
            usage_report,
            advance_scene,
            revert_scene,
            fork_scene,
            regenerate_scene_summary,
            generate_table_outline,
            generate_table_character,
            generate_table_expand
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

#[cfg(test)]
mod tests {
    use super::{
        clear_cli_workspace_images, cli_install_script, decode_base64, encode_base64,
        extract_image_refs, list_gallery_image_files, load_active_cards, record_card_arrivals,
        record_person_arrivals, scene_appearances_at, validate_gallery_component, ImageRef,
        InstallMessages, PathBuf,
    };
    use crate::data;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    fn messages() -> InstallMessages {
        InstallMessages {
            start: "start".to_owned(),
            login_hint: "login hint".to_owned(),
            success: "success".to_owned(),
            fail: "fail".to_owned(),
        }
    }

    #[test]
    fn extract_image_refs_returns_data_url() {
        assert_eq!(
            extract_image_refs("圖片：`data:image/png;base64,cG5n`"),
            vec![ImageRef::DataUrl("data:image/png;base64,cG5n".to_owned())]
        );
    }

    #[test]
    fn extract_image_refs_returns_existing_temp_file_path() {
        let path = std::env::temp_dir().join(format!(
            "table-tavern-image-{}-{}.png",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, b"png").unwrap();
        assert_eq!(
            extract_image_refs(&format!("已生成 {}", path.display())),
            vec![ImageRef::Path(path.clone())]
        );
        std::fs::remove_file(path).unwrap();
    }

    /// 真實 codex 輸出：前導說明與路徑之間沒有空白，整段當路徑會讀不到檔
    #[test]
    fn extract_image_refs_recovers_path_glued_to_preceding_sentence() {
        assert!(
            extract_image_refs("不含浮水印。/Users/me/.codex/generated_images/abc/call_x.png")
                .contains(&ImageRef::Path(PathBuf::from(
                    "/Users/me/.codex/generated_images/abc/call_x.png"
                )))
        );
    }

    /// macOS 的 CLI 工作資料夾在「Application Support」底下，逐詞切會把路徑攔腰切斷
    #[test]
    fn extract_image_refs_keeps_path_with_spaces() {
        assert!(extract_image_refs(
            "已存到 /Users/me/Library/Application Support/TableTavern/cli-workspace/fox.png，請查收"
        )
        .contains(&ImageRef::Path(PathBuf::from(
            "/Users/me/Library/Application Support/TableTavern/cli-workspace/fox.png"
        ))));
    }

    /// codex 的 imagegen 把圖存進工作目錄的子目錄，回覆給的是相對路徑
    #[test]
    fn extract_image_refs_keeps_relative_path() {
        assert!(extract_image_refs("Saved to output/imagegen/fox.png")
            .contains(&ImageRef::Path(PathBuf::from("output/imagegen/fox.png"))));
    }

    /// Windows 路徑沒有斜線可切，使用者名稱帶空格時同樣會斷
    #[test]
    fn extract_image_refs_keeps_windows_path_with_spaces() {
        assert!(extract_image_refs(
            "Saved to C:\\Users\\John Smith\\AppData\\Roaming\\TableTavern\\cli-workspace\\fox.PNG"
        )
        .contains(&ImageRef::Path(PathBuf::from(
            "C:\\Users\\John Smith\\AppData\\Roaming\\TableTavern\\cli-workspace\\fox.PNG"
        ))));
    }

    /// 中轉檔清理連 CLI 自開的子目錄一起掃，非圖片與還有東西的目錄留著
    #[test]
    fn clear_cli_workspace_images_removes_images_and_empty_dirs() {
        let workspace = std::env::temp_dir().join(format!(
            "table-tavern-workspace-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(workspace.join("output/imagegen")).unwrap();
        std::fs::create_dir_all(workspace.join("keep")).unwrap();
        std::fs::write(workspace.join("fox.PNG"), b"png").unwrap();
        std::fs::write(workspace.join("output/imagegen/deep.png"), b"png").unwrap();
        std::fs::write(workspace.join("note.txt"), b"keep").unwrap();
        std::fs::write(workspace.join("keep/data.txt"), b"keep").unwrap();
        clear_cli_workspace_images(&workspace);
        assert!(!workspace.join("fox.PNG").exists());
        assert!(!workspace.join("output").exists());
        assert!(workspace.join("note.txt").exists());
        assert!(workspace.join("keep/data.txt").exists());
        std::fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn extract_image_refs_returns_empty_without_image() {
        assert_eq!(extract_image_refs("沒有附圖。"), Vec::new());
        assert_eq!(encode_base64(b"png"), "cG5n");
    }

    #[test]
    fn decode_base64_roundtrip_restores_bytes() {
        let bytes = [0, 1, 2, 127, 128, 255];
        assert_eq!(decode_base64(&encode_base64(&bytes)).unwrap(), bytes);
    }

    #[test]
    fn decode_base64_rejects_invalid_input() {
        assert!(decode_base64("not base64!").is_err());
    }

    #[test]
    fn gallery_component_validation_allows_plain_png_name() {
        assert!(validate_gallery_component("1720000000000.png", true).is_ok());
    }

    #[test]
    fn gallery_component_validation_rejects_parent_path() {
        assert!(validate_gallery_component("../secret.png", true).is_err());
    }

    #[test]
    fn gallery_component_validation_rejects_path_separator() {
        assert!(validate_gallery_component("folder/image.png", true).is_err());
    }

    #[test]
    fn list_gallery_image_files_sorts_newest_first() {
        let root = std::env::temp_dir().join(format!(
            "table-tavern-gallery-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let world_id = data::new_id();
        let character_id = data::new_id();
        let directory = root
            .join("worlds")
            .join(&world_id)
            .join("gen-gallery")
            .join(&character_id);
        std::fs::create_dir_all(&directory).unwrap();
        for file in [
            "1720000000000.png",
            "1730000000000.png",
            "1710000000000.png",
        ] {
            std::fs::write(directory.join(file), b"png").unwrap();
        }
        assert_eq!(
            list_gallery_image_files(&root, &world_id, &character_id).unwrap(),
            [
                "1730000000000.png",
                "1720000000000.png",
                "1710000000000.png"
            ]
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    fn assert_messages(script: &str) {
        for text in ["start", "login hint", "success", "fail"] {
            assert!(script.contains(text));
        }
    }

    #[test]
    fn claude_install_script_contains_messages_and_flow() {
        let script = cli_install_script("claude", &messages()).unwrap();
        assert_messages(&script);
        assert!(script.contains("curl -fsSL https://claude.ai/install.sh | bash"));
        assert!(script.contains("claude auth login"));
        assert!(script.contains("claude -p \"ok\" >/dev/null 2>&1"));
        assert!(script.contains("while [ \"$elapsed\" -lt 120 ]"));
    }

    #[test]
    fn codex_install_script_contains_messages_and_flow() {
        let script = cli_install_script("codex", &messages()).unwrap();
        assert_messages(&script);
        assert!(script.contains("curl -fsSL https://chatgpt.com/codex/install.sh | sh"));
        assert!(script.contains("codex login"));
        assert!(script.contains("codex login status >/dev/null 2>&1"));
        assert!(script.contains("while [ \"$elapsed\" -lt 120 ]"));
    }

    #[test]
    fn agy_provider_script_contains_messages_and_flow() {
        let script = cli_install_script("agy", &messages()).unwrap();
        assert_messages(&script);
        assert!(script.contains("curl -fsSL https://antigravity.google/cli/install.sh | bash"));
        assert!(!script.contains("claude auth login"));
        assert!(!script.contains("codex login"));
        assert!(!script.contains("grok login"));
        assert!(script.contains("agy -p \"ok\" >/dev/null 2>&1"));
        assert!(script.contains("while [ \"$elapsed\" -lt 600 ]"));
        assert!(script.contains("sleep 5"));
    }

    #[test]
    fn grok_install_script_contains_messages_and_flow() {
        let script = cli_install_script("grok", &messages()).unwrap();
        assert_messages(&script);
        assert!(script.contains("curl -fsSL https://x.ai/cli/install.sh | bash"));
        assert!(script.contains("grok login"));
        assert!(script.contains("grok models 2>/dev/null | grep -q '^You are logged in'"));
        assert!(script.contains("while [ \"$elapsed\" -lt 120 ]"));
    }

    #[test]
    fn install_script_touches_sentinel_only_after_verification_passes() {
        let script = cli_install_script("claude", &messages()).unwrap();
        let touch = script
            .find("touch \"$(dirname \"$0\")/.verified-claude\"")
            .unwrap();
        assert!(touch > script.find("exit 1").unwrap());
        assert!(touch < script.rfind("success").unwrap());
    }

    #[test]
    fn cli_install_script_escapes_single_quotes_and_rejects_unknown_provider() {
        let quoted_messages = InstallMessages {
            start: "don't".to_owned(),
            login_hint: "login".to_owned(),
            success: "success".to_owned(),
            fail: "fail".to_owned(),
        };
        assert!(cli_install_script("agy", &quoted_messages)
            .unwrap()
            .contains("'don'\"'\"'t'"));
        assert!(cli_install_script("unknown", &messages()).is_err());
    }

    /// AI 卡重構包 4a 規格 (c)(d)(e)：present 有新面孔就把世界書全文 append 成一則系統事件；
    /// 同一幕重複比對不重複 append；換幕（新場景號、空 events）同名要重新 append 一次。
    #[test]
    fn record_person_arrivals_appends_once_per_scene_and_resets_on_new_scene() {
        let root = std::env::temp_dir().join(format!(
            "table-tavern-person-arrivals-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let world_id = data::create_world(&root, "測試桌").unwrap();

        let alice = data::WorldbookEntry {
            uid: 1,
            title: "愛麗絲".to_owned(),
            keys: Vec::new(),
            content: "愛麗絲是旅店老闆娘。".to_owned(),
            constant: true,
            order: 0,
            disabled: false,
            visibility: data::Visibility::Public,
            is_person: true,
            locked: false,
        };
        let worldbook = [alice];

        // 第一輪：present 有愛麗絲 → append 一則登場事件
        record_person_arrivals(&root, &world_id, 0, &worldbook, &[], Some("愛麗絲"), "", "阿濤");
        let scene0 = data::read_transcript(&root, &world_id, 0).unwrap();
        assert_eq!(scene0.len(), 1);
        assert_eq!(scene0[0].kind, data::TranscriptKind::System);
        assert_eq!(scene0[0].speaker_id, "");
        assert_eq!(scene0[0].speaker_name, "GM");
        assert!(scene0[0].text.starts_with("（人物登場）〈愛麗絲〉\n"));
        assert!(scene0[0].text.contains("愛麗絲是旅店老闆娘。"));

        // 第二輪：present 還是愛麗絲，本幕 events 已含前一則登場事件 → 不重複
        record_person_arrivals(
            &root, &world_id, 0, &worldbook, &scene0, Some("愛麗絲"), "", "阿濤",
        );
        assert_eq!(data::read_transcript(&root, &world_id, 0).unwrap().len(), 1);

        // 換幕：scene 1 是新 jsonl、events 是空的 → 同名重新 append
        record_person_arrivals(&root, &world_id, 1, &worldbook, &[], Some("愛麗絲"), "", "阿濤");
        let scene1 = data::read_transcript(&root, &world_id, 1).unwrap();
        assert_eq!(scene1.len(), 1);
        assert!(scene1[0].text.starts_with("（人物登場）〈愛麗絲〉\n"));

        std::fs::remove_dir_all(&root).unwrap();
    }

    fn character_card(id: &str, name: &str) -> data::CharacterCard {
        data::CharacterCard {
            id: id.to_owned(),
            name: name.to_owned(),
            color: "#336699".to_owned(),
            avatar: "🦊".to_owned(),
            tier: data::Tier::Balanced,
            show_image: true,
            archived: false,
            gen_prompt: String::new(),
            public_md: String::new(),
            private_md: String::new(),
        }
    }

    /// AI 卡重構包 4b：load_active_cards 濾掉 auto_hidden（跟既有的 archived 並列），
    /// 只有沒被隱藏、也沒被封存的卡才進 GM／chars 凍結快照。
    #[test]
    fn load_active_cards_filters_auto_hidden_and_archived() {
        let root = std::env::temp_dir().join(format!(
            "table-tavern-load-active-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let world_id = data::create_world(&root, "測試桌").unwrap();

        let visible = character_card(&data::new_id(), "在場");
        let hidden = character_card(&data::new_id(), "隱藏");
        let archived = character_card(&data::new_id(), "封存");
        data::write_character(&root, &world_id, &visible).unwrap();
        data::write_character(&root, &world_id, &hidden).unwrap();
        data::write_character(&root, &world_id, &archived).unwrap();
        data::set_character_auto_hidden(&root, &world_id, &hidden.id, true).unwrap();
        data::set_character_archived(&root, &world_id, &archived.id, true).unwrap();

        let active = load_active_cards(&root, &world_id).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, visible.id);

        std::fs::remove_dir_all(&root).unwrap();
    }

    /// AI 卡重構包 4b，鏡射 4a：present 有隱藏卡的名字就把完整設定 append 成一則回歸事件；
    /// 同一幕重複比對不重複 append；不改 auto_hidden 欄位本身（鐵律，換幕才結算）。
    #[test]
    fn record_card_arrivals_appends_once_per_scene_and_does_not_touch_auto_hidden() {
        let root = std::env::temp_dir().join(format!(
            "table-tavern-card-arrivals-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let world_id = data::create_world(&root, "測試桌").unwrap();

        let mut fox = character_card(&data::new_id(), "狐狸");
        fox.public_md = "尾巴很大。".to_owned();
        data::write_character(&root, &world_id, &fox).unwrap();
        data::set_character_auto_hidden(&root, &world_id, &fox.id, true).unwrap();
        let hidden_cards = vec![fox.clone()];

        // 第一輪：present 有狐狸 → append 一則回歸事件
        record_card_arrivals(&root, &world_id, 0, &hidden_cards, &[], Some("狐狸"), "", "阿濤");
        let scene0 = data::read_transcript(&root, &world_id, 0).unwrap();
        assert_eq!(scene0.len(), 1);
        assert_eq!(scene0[0].kind, data::TranscriptKind::System);
        assert!(!scene0[0].gm_only);
        assert!(scene0[0].text.starts_with("（角色回歸）〈狐狸〉\n"));
        assert!(scene0[0].text.contains("尾巴很大。"));

        // 第二輪：present 還是狐狸，本幕 events 已含前一則回歸事件 → 不重複
        record_card_arrivals(
            &root, &world_id, 0, &hidden_cards, &scene0, Some("狐狸"), "", "阿濤",
        );
        assert_eq!(data::read_transcript(&root, &world_id, 0).unwrap().len(), 1);

        // 不碰 auto_hidden 欄位本身：磁碟上仍是 true，要等換幕結算才會變 false
        let meta = data::list_characters(&root, &world_id)
            .unwrap()
            .into_iter()
            .find(|meta| meta.id == fox.id)
            .unwrap();
        assert!(meta.auto_hidden);

        std::fs::remove_dir_all(&root).unwrap();
    }

    /// AI 卡重構包 4b：visibility 非 Public 的世界書人物條目登場時，事件帶 gm_only=true
    /// （chars 續聊線只看得到前綴那一行，不洩漏全文）；這是 4a 遺留的洩漏修正。
    #[test]
    fn record_person_arrivals_marks_gm_only_for_non_public_visibility() {
        let root = std::env::temp_dir().join(format!(
            "table-tavern-person-gm-only-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let world_id = data::create_world(&root, "測試桌").unwrap();

        let spy = data::WorldbookEntry {
            uid: 1,
            title: "密探".to_owned(),
            keys: Vec::new(),
            content: "其實是反派的眼線。".to_owned(),
            constant: true,
            order: 0,
            disabled: false,
            visibility: data::Visibility::Gm,
            is_person: true,
            locked: false,
        };
        let worldbook = [spy];

        record_person_arrivals(&root, &world_id, 0, &worldbook, &[], Some("密探"), "", "阿濤");
        let scene0 = data::read_transcript(&root, &world_id, 0).unwrap();
        assert_eq!(scene0.len(), 1);
        assert!(scene0[0].gm_only);

        std::fs::remove_dir_all(&root).unwrap();
    }

    /// AI 卡重構包 4b：scene_appearances 掃現在這幕的 transcript，角色卡回歸事件反查回 id，
    /// 世界書人物登場事件直接回 title；兩種前綴互不干擾。
    #[test]
    fn scene_appearances_at_scans_both_prefixes() {
        let root = std::env::temp_dir().join(format!(
            "table-tavern-scene-appearances-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let world_id = data::create_world(&root, "測試桌").unwrap();

        let fox = character_card(&data::new_id(), "狐狸");
        data::write_character(&root, &world_id, &fox).unwrap();

        data::append_transcript(
            &root,
            &world_id,
            0,
            &data::TranscriptEvent {
                ts: "now".to_owned(),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: data::TranscriptKind::System,
                text: "（角色回歸）〈狐狸〉\n尾巴很大。".to_owned(),
                raw: None,
                state: None,
                gm_only: false,
            },
        )
        .unwrap();
        data::append_transcript(
            &root,
            &world_id,
            0,
            &data::TranscriptEvent {
                ts: "now".to_owned(),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: data::TranscriptKind::System,
                text: "（人物登場）〈愛麗絲〉\n旅店老闆娘。".to_owned(),
                raw: None,
                state: None,
                gm_only: false,
            },
        )
        .unwrap();

        let result = scene_appearances_at(&root, &world_id).unwrap();
        assert_eq!(result.character_ids, vec![fox.id.clone()]);
        assert_eq!(result.person_titles, vec!["愛麗絲".to_owned()]);

        std::fs::remove_dir_all(&root).unwrap();
    }
}
