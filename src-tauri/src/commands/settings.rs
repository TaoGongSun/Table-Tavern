use crate::ai_transport::{chat_transport, cli_envs};
use crate::data::AppConfig;
use crate::{cli, config_root, data, data_root, transport, usage_report};

#[tauri::command]
pub(crate) fn sponsor_status(app: tauri::AppHandle) -> Result<bool, String> {
    Ok(data::sponsor_pack_active(&data_root(&app)?))
}

#[tauri::command]
pub(crate) fn import_sponsor_pack(app: tauri::AppHandle, data: Vec<u8>) -> Result<(), String> {
    data::install_sponsor_pack(&data_root(&app)?, &data).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn read_config(app: tauri::AppHandle) -> Result<AppConfig, String> {
    data::read_config(&config_root(&app)?).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn write_config(app: tauri::AppHandle, config: AppConfig) -> Result<(), String> {
    data::write_config(&config_root(&app)?, &config).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn detect_clis() -> Vec<cli::CliInfo> {
    cli::detect_clis().await
}

/// 設定 UI 下拉用：列出 CLI 訂閱模式可選的模型（見 cli::cli_model_catalog）。
/// 必須是 async：同步 command 跑在主執行緒，抓取期間整個視窗會凍住。
#[tauri::command]
pub(crate) async fn list_cli_models(app: tauri::AppHandle, cli: String) -> Vec<cli::ModelOption> {
    // 環境備不出來就回空清單。退回無環境等於讓 grok 摸回使用者真正的 ~/.grok，
    // 設定頁會顯示終端機的登入態，與旁白實跑的 app profile 對不起來。
    let Ok(envs) = cli_envs(&app, &cli) else {
        return Vec::new();
    };
    cli::cli_model_catalog(&cli, &envs).await
}

/// 模型清單快取：開 app 時先拿上次的結果直接顯示，背景抓到新的再覆蓋。
#[tauri::command]
pub(crate) fn read_model_catalog(
    app: tauri::AppHandle,
) -> Result<std::collections::BTreeMap<String, Vec<cli::ModelOption>>, String> {
    data::read_model_catalog(&config_root(&app)?).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn write_model_catalog(
    app: tauri::AppHandle,
    catalog: std::collections::BTreeMap<String, Vec<cli::ModelOption>>,
) -> Result<(), String> {
    data::write_model_catalog(&config_root(&app)?, &catalog).map_err(|error| error.to_string())
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
pub(crate) fn usage_report(
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
