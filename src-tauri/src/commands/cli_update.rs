//! CLI 更新鈕的 Tauri 邊界（plans/cli-version-gate.md 包 2 第 3 點）：流程在 `cli::update`。

use crate::cli::update::{self, Ctx, Mode, Outcome};
use crate::transport::dispatch::cli_envs;
use crate::{cli, data_root};

/// 更新鈕。`mode`：tested＝先裝已測版（agy 一律最新版）、latest＝裝最新版；
/// `required`＝伺服器要求的最低版本（`cli_too_old` 帶來的），比已測版高就跳過已測版。
#[tauri::command]
pub(crate) async fn update_cli(
    app: tauri::AppHandle,
    cli: String,
    mode: Mode,
    required: Option<String>,
) -> Result<Outcome, String> {
    crate::data::refuse_if_updating()?;
    let cli = match cli.as_str() {
        "claude" => "claude",
        "codex" => "codex",
        "agy" => "agy",
        "grok" => "grok",
        _ => return Err(format!("unsupported CLI provider: {cli}")),
    };
    let ctx = Ctx::official(
        data_root(&app)?,
        cli::agy_home::real_home()?,
        // agy 的更新帶 app 的 agy-home（`agy update` 換的是被呼叫的那支）
        match cli {
            "agy" => cli_envs(&app, "agy")?,
            _ => Vec::new(),
        },
    );
    let required = required.filter(|value| !value.is_empty());
    Ok(update::run(&ctx, cli, mode, required.as_deref()).await)
}

/// app 啟動：同步立起有標記那幾家的擋板，再在背景逐家處理（版本沒動就刪標記、其實成功了也刪、
/// 其他照契約還原）。處理期間該 CLI 的呼叫一律回 CliUpdating。
pub(crate) fn spawn_startup_recovery(app: &tauri::AppHandle, data_root: std::path::PathBuf) {
    let pending = update::raise_pending(&data_root);
    if pending.is_empty() {
        return;
    }
    let agy_envs = cli_envs(app, "agy").unwrap_or_default();
    tauri::async_runtime::spawn(async move {
        let home = match cli::agy_home::real_home() {
            Ok(home) => home,
            Err(error) => {
                // 找不到家目錄：照樣處理（claude 的連結還原會退回官方腳本）
                log::warn!("CLI 更新復原找不到家目錄：{error}");
                std::path::PathBuf::new()
            }
        };
        let ctx = Ctx::official(data_root, home, agy_envs);
        for (marker, barrier) in pending {
            update::recover(&ctx, marker, barrier).await;
        }
    });
}
