#[cfg(target_os = "windows")]
use crate::cli;
use crate::cli::install;
use crate::data_root;
use crate::transport::dispatch::cli_envs;
use serde::Deserialize;
#[cfg(not(target_os = "windows"))]
use std::process::Command;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstallMessages {
    start: String,
    login_hint: String,
    success: String,
    fail: String,
    /// 已測版下載不到、改裝最新版時印的一句（「非已測版本」）
    #[serde(default)]
    untested: String,
}

/// codex 的登入共用使用者自己的 `~/.codex/auth.json`（app 的 codex-home 只放一條連結過去），
/// 登入、登出、探針都要跑在使用者原本的環境；帶 CODEX_HOME 會把登入寫進 codex-home、佔掉連結位置。
fn runs_in_app_profile(provider: &str) -> bool {
    provider != "codex"
}

fn setup_envs(app: &tauri::AppHandle, provider: &str) -> Result<Vec<(String, String)>, String> {
    match runs_in_app_profile(provider) {
        true => cli_envs(app, provider),
        false => Ok(Vec::new()),
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

/// 官方安裝指令（`cli::update::recipe::first_install`）轉成終端腳本的一行：`env K='V' bash -c '…'`。
fn step_line(step: &crate::cli::update::recipe::Step) -> String {
    let envs: Vec<String> = step
        .envs
        .iter()
        .map(|(key, value)| format!("{key}={}", shell_quote(value)))
        .collect();
    let argv: Vec<String> = step.argv.iter().map(|part| shell_quote(part)).collect();
    match envs.is_empty() {
        true => argv.join(" "),
        false => format!("env {} {}", envs.join(" "), argv.join(" ")),
    }
}

/// `force_install`：上次 app 內更新還原失敗（CLI 跑不起來）：不看「已經裝了」，一律重裝。
/// `envs`：登入與探針要跑在哪組環境（grok 用 app 專用 profile）。
/// 安裝那步刻意不套前綴——安裝腳本要把 binary 裝進使用者真正的家目錄。
/// `switch_account`：換帳號。舊憑證沒過期時前置探針會直接判定「已登入」收工，
/// 登入那行根本跑不到，所以這條要略過前置探針並先登出。
fn cli_install_script(
    provider: &str,
    messages: &InstallMessages,
    envs: &[(String, String)],
    switch_account: bool,
    force_install: bool,
) -> Result<String, String> {
    let env_prefix = if envs.is_empty() {
        String::new()
    } else {
        let pairs: Vec<String> = envs
            .iter()
            .map(|(key, value)| format!("{key}={}", shell_quote(value)))
            .collect();
        format!("env {} ", pairs.join(" "))
    };
    let start = shell_quote(&messages.start);
    let login_hint = shell_quote(&messages.login_hint);
    let success = shell_quote(&messages.success);
    let fail = shell_quote(&messages.fail);
    let untested = shell_quote(&messages.untested);
    // logout：換帳號時登入前先清憑證。agy 只有 TUI 內的 /logout，沒有非互動指令，故為 None——
    // 它的換帳號要玩家自己在 agy 裡 /logout 後再回來按登入。
    let (login_command, logout_command, probe_command, poll_seconds) = match provider {
        "claude" => (
            Some("claude auth login"),
            Some("claude auth logout"),
            "claude -p \"ok\"",
            120,
        ),
        "codex" => (
            Some("codex login"),
            Some("codex logout"),
            // codex exec 在非 git 目錄會拒跑，probe 改用即時且不耗額度的 login status
            "codex login status",
            120,
        ),
        "agy" => (None, None, "agy -p \"ok\"", 600),
        "grok" => (
            Some("grok login"),
            Some("grok logout"),
            // grok -p 會真的跑一次 grok-4.5 推理（實測 26 秒又燒額度）；models 只讀本機憑證，0.8 秒。
            // 未登入時它是否照樣 exit 0 無法驗證，故以登入字串判定，判錯也只是多要求登入一次。
            "grok models 2>/dev/null | grep -q '^You are logged in'",
            120,
        ),
        _ => return Err(format!("unsupported CLI provider: {provider}")),
    };
    // 首次安裝裝已測版（claude、codex、grok）；下載不到就改裝最新版；agy 只能裝最新版。
    // 裝完以 --version 確認跑得起來，再比對實際裝上的版號，不是已測版就印「非已測版本」。
    let tested = crate::cli::tested_versions::tested(provider);
    let (pinned, latest) = crate::cli::update::recipe::first_install(provider, tested)
        .ok_or_else(|| format!("unsupported CLI provider: {provider}"))?;
    let latest_line = step_line(&latest);
    let (install_command, tested_check) = match (pinned, tested) {
        (Some(pinned), Some(tested)) => (
            format!(
                "{} || {{ {latest_line} || {{ echo {fail}; exit 1; }}; }}",
                step_line(&pinned)
            ),
            // 版號只含數字與點（tested_versions 常數），前後不能再接數字或點，免得 2.1.311 配到 2.1.3110
            format!(
                "\n  {provider} --version 2>/dev/null | head -n 1 | grep -Eq '(^|[^0-9.]){}([^0-9.]|$)' || echo {untested}",
                tested.replace('.', "\\.")
            ),
        ),
        _ => (
            format!("{latest_line} || {{ echo {fail}; exit 1; }}"),
            String::new(),
        ),
    };
    let need_install = match force_install {
        true => "true".to_owned(),
        false => format!("! command -v {provider} >/dev/null 2>&1"),
    };
    let login_flow = login_command
        .map(|command| format!("  {env_prefix}{command} || {{ echo {fail}; exit 1; }}\n"))
        .unwrap_or_default();
    let probe_command = format!("{env_prefix}{probe_command}");
    // 沒有登入指令的（agy）換不了帳號，照原本的前置探針走，免得空等一輪輪詢
    let switch_account = switch_account && login_command.is_some();
    // 登出失敗就停：舊憑證還在的話，接下來的 login 會回一句「已登入」什麼都不做，
    // 探針照樣過，整條流程會假報「已切換」但人根本沒換。輸出不轉進黑洞，終端機看得到原因。
    let logout_line = match (switch_account, logout_command) {
        (true, Some(command)) => {
            format!("{env_prefix}{command} || {{ echo {fail}; exit 1; }}\n")
        }
        _ => String::new(),
    };
    // 換帳號那條把前置探針短路成 false：舊憑證還有效也一定要重跑登入
    let pre_probe = match switch_account {
        true => "false".to_owned(),
        false => format!("{probe_command} >/dev/null 2>&1"),
    };
    let sentinel = cli_sentinel_name(provider);
    Ok(format!(
        r#"#!/bin/bash
echo {start}
export PATH="$HOME/.local/bin:$HOME/.grok/bin:$HOME/.codex/bin:$PATH"
if {need_install}; then
  {install_command}
  {provider} --version >/dev/null 2>&1 || {{ echo {fail}; exit 1; }}{tested_check}
fi
# 安裝之後的登入與探針一律關掉 CLI 自動更新（版本由 app 掌控；agy 的關閉變數在 envs 裡）
export DISABLE_AUTOUPDATER=1 GROK_DISABLE_AUTOUPDATER=1
echo {login_hint}
verified=0
{logout_line}if {pre_probe}; then
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
pub(crate) fn cli_verified(app: tauri::AppHandle, provider: String) -> Result<bool, String> {
    Ok(data_root(&app)?.join(cli_sentinel_name(&provider)).exists())
}

#[tauri::command]
pub(crate) fn install_cli(
    app: tauri::AppHandle,
    provider: String,
    messages: InstallMessages,
    switch_account: bool,
) -> Result<(), String> {
    crate::data::refuse_if_updating()?;
    let directory = data_root(&app)?;
    // world-write-exempt: 建立資料根目錄放安裝腳本，不是桌目錄
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let _ = &messages;
    // 上一輪的驗證印記要清掉，避免輪詢讀到舊結果就把「已連結」點亮；但要等冷卻／併發那關
    // 過了再清——沒真的開始跑就先清，前端會誤判成「這次沒驗過」而把徽章打掉。
    let sentinel_path = directory.join(cli_sentinel_name(&provider));
    #[cfg(target_os = "windows")]
    {
        use std::time::Duration;
        use tauri::Emitter;

        let mut spec = install::windows_specs()?
            .into_iter()
            .find(|spec| spec.id == provider)
            .ok_or_else(|| format!("unsupported CLI provider: {provider}"))?;
        // 登入與探針都跑在 app 自己的 profile，否則會登在使用者的 ~/.grok 卻用不到；codex 例外（setup_envs）
        spec.envs = setup_envs(&app, &provider)?;
        // 登入視窗經 cmd 起 CLI，spawn 認不出是哪家：關閉自動更新的變數在這裡補上（同 mac 腳本的 export）
        spec.envs.extend(
            cli::auto_update_off(&provider)
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned())),
        );
        // 上次 app 內更新還原失敗（CLI 跑不起來）：不看已安裝，一律重裝
        spec.force_install = cli::update::needs_reinstall(&directory, &provider);
        // 換帳號：略過前置探針（舊憑證有效也要重登）並保留 logout；平常那條把 logout 清掉
        match switch_account && !spec.logout.is_empty() {
            true => spec.pre_probe = false,
            false => spec.logout.clear(),
        }
        let token = match install::try_begin(&provider, Duration::from_secs(60)) {
            install::BeginOutcome::Started(token) => {
                // world-write-exempt: 清安裝驗證印記，在資料根目錄，不是桌目錄
                let _ = std::fs::remove_file(&sentinel_path);
                token
            }
            install::BeginOutcome::AlreadyRunning => {
                install::raise_login_window(&spec.window_title);
                return Ok(());
            }
            install::BeginOutcome::Cooldown(seconds) => {
                return Err(format!("login-cooldown:{seconds}"))
            }
        };
        let task_app = app.clone();
        let directory_for_marker = directory.clone();
        let provider_for_marker = provider.clone();
        tauri::async_runtime::spawn(async move {
            let _token = token;
            let emit_app = task_app.clone();
            let _ = install::run_install(spec, &directory, cli::find_binary, move |progress| {
                if progress.stage == "done" && crate::data::update_gate_raised() {
                    // 閘門起來後不再寫印記。註解必須留在寫入的上一行，掃描才認豁免。
                } else if progress.stage == "done" {
                    // world-write-exempt: 寫安裝驗證印記，在資料根目錄，不是桌目錄
                    let _ = std::fs::write(&sentinel_path, b"");
                    // 重新安裝成功（--version 驗證過）：清掉更新還原失敗的標記
                    cli::update::clear_after_reinstall(&directory_for_marker, &provider_for_marker);
                }
                let _ = emit_app.emit("cli-install-progress", progress);
            })
            .await;
        });
    }
    #[cfg(not(target_os = "windows"))]
    {
        use std::time::Duration;

        // 冷卻中＝這次沒有真的開跑（前一輪可能還開著，也可能剛結束）。回錯誤讓前端知道，
        // 別把它當成「破壞性流程已啟動」而清掉還有效的已連結徽章。
        if let Some(seconds) = install::mac_cooldown(&provider, Duration::from_secs(60)) {
            Command::new("open")
                .args(["-a", "Terminal"])
                .spawn()
                .map_err(|error| error.to_string())?;
            return Err(format!("login-cooldown:{seconds}"));
        }
        // world-write-exempt: 清安裝驗證印記，在資料根目錄，不是桌目錄
        let _ = std::fs::remove_file(&sentinel_path);
        let script = cli_install_script(
            &provider,
            &messages,
            &setup_envs(&app, &provider)?,
            switch_account,
            crate::cli::update::needs_reinstall(&directory, &provider),
        )?;
        let script_path = directory.join(format!("install-{provider}.command"));
        // world-write-exempt: 寫 CLI 安裝腳本，在資料根目錄，不是桌目錄
        std::fs::write(&script_path, script).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))
                .map_err(|error| error.to_string())?;
        }
        // 終端腳本在 app 外跑，派送點看不到：在 app 啟動腳本時記一行，標明腳本是否含 AI 探測
        #[cfg(feature = "test-harness")]
        let harness_dispatch = crate::harness::ai_dispatch(
            &format!("cli-setup-terminal:{provider}"),
            "（終端腳本，app 外執行）",
            serde_json::json!({
                "aiProbe": matches!(provider.as_str(), "claude" | "agy"),
                "script": script_path,
            }),
        );
        let opened = Command::new("open")
            .args(["-a", "Terminal"])
            .arg(&script_path)
            .spawn();
        #[cfg(feature = "test-harness")]
        crate::harness::ai_event(
            &harness_dispatch,
            if opened.is_ok() {
                "spawned"
            } else {
                "spawn-failed"
            },
            serde_json::Value::Null,
        );
        opened.map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{cli_install_script, runs_in_app_profile, InstallMessages};
    use std::path::PathBuf;

    fn messages() -> InstallMessages {
        InstallMessages {
            start: "start".to_owned(),
            login_hint: "login hint".to_owned(),
            success: "success".to_owned(),
            fail: "fail".to_owned(),
            untested: "untested".to_owned(),
        }
    }

    fn assert_messages(script: &str) {
        for text in ["start", "login hint", "success", "fail"] {
            assert!(script.contains(text));
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn claude_install_script_contains_messages_and_flow() {
        let script = cli_install_script("claude", &messages(), &[], false, false).unwrap();
        assert_messages(&script);
        assert!(script.contains("curl -fsSL https://claude.ai/install.sh | bash"));
        assert!(script.contains("claude auth login"));
        assert!(script.contains("claude -p \"ok\" >/dev/null 2>&1"));
        assert!(script.contains("while [ \"$elapsed\" -lt 120 ]"));
    }

    #[cfg(not(windows))]
    #[test]
    fn codex_install_script_contains_messages_and_flow() {
        let script = cli_install_script("codex", &messages(), &[], false, false).unwrap();
        assert_messages(&script);
        assert!(script.contains("curl -fsSL https://chatgpt.com/codex/install.sh | sh"));
        assert!(script.contains("codex login"));
        assert!(script.contains("codex login status >/dev/null 2>&1"));
        assert!(script.contains("while [ \"$elapsed\" -lt 120 ]"));
    }

    /// codex 共用使用者的登入：腳本跑在使用者原本的環境，沒登入也照常產生，不帶 CODEX_HOME。
    #[test]
    fn codex_login_runs_in_the_users_own_home() {
        assert!(!runs_in_app_profile("codex"));
        assert!(runs_in_app_profile("grok") && runs_in_app_profile("agy"));
        for switch in [false, true] {
            let script = cli_install_script("codex", &messages(), &[], switch, false).unwrap();
            assert!(!script.contains("CODEX_HOME"));
            assert!(script.contains("codex login"));
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn agy_provider_script_contains_messages_and_flow() {
        let script = cli_install_script("agy", &messages(), &[], false, false).unwrap();
        assert_messages(&script);
        assert!(script.contains("curl -fsSL https://antigravity.google/cli/install.sh | bash"));
        assert!(!script.contains("claude auth login"));
        assert!(!script.contains("codex login"));
        assert!(!script.contains("grok login"));
        assert!(script.contains("agy -p \"ok\" >/dev/null 2>&1"));
        assert!(script.contains("while [ \"$elapsed\" -lt 600 ]"));
        assert!(script.contains("sleep 5"));
    }

    #[cfg(not(windows))]
    #[test]
    fn grok_install_script_contains_messages_and_flow() {
        let script = cli_install_script("grok", &messages(), &[], false, false).unwrap();
        assert_messages(&script);
        assert!(script.contains("curl -fsSL https://x.ai/cli/install.sh | bash"));
        assert!(script.contains("grok login"));
        assert!(script.contains("grok models 2>/dev/null | grep -q '^You are logged in'"));
        assert!(script.contains("while [ \"$elapsed\" -lt 120 ]"));
    }

    #[cfg(not(windows))]
    #[test]
    fn grok_install_script_runs_login_and_probe_in_the_app_profile() {
        let envs = crate::cli::grok_envs(
            &PathBuf::from("/app/cli-home"),
            &PathBuf::from("/app/grok-home"),
        );
        let script = cli_install_script("grok", &messages(), &envs, false, false).unwrap();
        // 登入與探針都要掛上 env 前綴，否則會登在使用者的 ~/.grok、或探到終端機的登入態
        assert!(script.contains(
            "env HOME='/app/cli-home' USERPROFILE='/app/cli-home' GROK_HOME='/app/grok-home' GROK_CONFIG='{\"models\":{\"temperature\":1.1,\"top_p\":1.0},\"features\":{\"backend_tools\":false}}' GROK_CAMPAIGNS='0' GROK_BACKEND_SEARCH='0' grok login"
        ));
        assert!(script.contains(
            "GROK_CONFIG='{\"models\":{\"temperature\":1.1,\"top_p\":1.0},\"features\":{\"backend_tools\":false}}' GROK_CAMPAIGNS='0' GROK_BACKEND_SEARCH='0' grok models 2>/dev/null"
        ));
        // 安裝那行不帶：安裝腳本要把 binary 裝進使用者真正的家目錄
        assert!(script.contains(
            "  'bash' '-o' 'pipefail' '-c' 'curl -fsSL https://x.ai/cli/install.sh | bash -s 1.0.70' ||"
        ));
    }

    /// 換帳號：舊憑證沒過期時前置探針會直接判定已登入，所以那條必須短路成 false 並先登出
    #[test]
    fn switch_account_script_logs_out_and_skips_the_pre_probe() {
        let script = cli_install_script("grok", &messages(), &[], true, false).unwrap();
        // 登出失敗要當場中止，不能帶著舊憑證往下登入
        assert!(script.contains("grok logout || { echo 'fail'; exit 1; }"));
        assert!(script.contains("if false; then"));
        assert!(script.contains("grok login"));
        // 登出要在登入之前，否則清掉的是剛換上的新帳號
        assert!(script.find("grok logout").unwrap() < script.find("grok login").unwrap());
        // 平常那條完全不碰登出，前置探針照舊
        let plain = cli_install_script("grok", &messages(), &[], false, false).unwrap();
        assert!(!plain.contains("grok logout"));
        assert!(
            plain.contains("if env grok models 2>/dev/null | grep -q '^You are logged in'")
                || plain.contains("if grok models 2>/dev/null | grep -q '^You are logged in'")
        );
        // agy 沒有非互動登出指令，換帳號旗標對它無效——維持前置探針，不空等一輪輪詢
        let agy = cli_install_script("agy", &messages(), &[], true, false).unwrap();
        assert!(!agy.contains("if false; then"));
        assert!(agy.contains("agy -p \"ok\" >/dev/null 2>&1"));
    }

    /// 首次安裝裝已測版、下載不到改裝最新版；裝完驗 --version，實際版號不是已測版就印「非已測版本」；
    /// agy 只能裝最新版、不比；下載失敗靠 pipefail 讓退路觸發得了；安裝之後的登入與探針關掉自動更新。
    #[cfg(not(windows))]
    #[test]
    fn first_install_pins_tested_versions_with_latest_fallback() {
        let claude = cli_install_script("claude", &messages(), &[], false, false).unwrap();
        assert!(claude.contains(
            "'bash' '-o' 'pipefail' '-c' 'curl -fsSL https://claude.ai/install.sh | bash -s 2.1.311' || { 'bash' '-o' 'pipefail' '-c' 'curl -fsSL https://claude.ai/install.sh | bash' || { echo 'fail'; exit 1; }; }"
        ));
        assert!(claude.contains(
            "claude --version >/dev/null 2>&1 || { echo 'fail'; exit 1; }\n  claude --version 2>/dev/null | head -n 1 | grep -Eq '(^|[^0-9.])2\\.1\\.311([^0-9.]|$)' || echo 'untested'\nfi"
        ));
        let codex = cli_install_script("codex", &messages(), &[], false, false).unwrap();
        assert!(codex.contains(
            "env CODEX_NON_INTERACTIVE='1' CODEX_RELEASE='0.164.0' 'bash' '-o' 'pipefail' '-c' 'curl -fsSL https://chatgpt.com/codex/install.sh | sh'"
        ));
        let agy = cli_install_script("agy", &messages(), &[], false, false).unwrap();
        assert!(agy.contains(
            "  'bash' '-o' 'pipefail' '-c' 'curl -fsSL https://antigravity.google/cli/install.sh | bash' || { echo 'fail'; exit 1; }\n"
        ));
        assert!(!agy.contains("untested"));
        let export = claude
            .find("export DISABLE_AUTOUPDATER=1 GROK_DISABLE_AUTOUPDATER=1")
            .unwrap();
        assert!(export > claude.find("install.sh").unwrap());
        assert!(export < claude.find("claude auth login").unwrap());
        // 上次更新還原失敗：不看已安裝，一律重裝
        assert!(claude.contains("if ! command -v claude >/dev/null 2>&1; then"));
        let forced = cli_install_script("claude", &messages(), &[], false, true).unwrap();
        assert!(forced.contains("if true; then\n  'bash' '-o' 'pipefail' '-c'"));
    }

    #[test]
    fn install_script_touches_sentinel_only_after_verification_passes() {
        let script = cli_install_script("claude", &messages(), &[], false, false).unwrap();
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
            untested: "untested".to_owned(),
        };
        assert!(
            cli_install_script("agy", &quoted_messages, &[], false, false)
                .unwrap()
                .contains("'don'\"'\"'t'")
        );
        assert!(cli_install_script("unknown", &messages(), &[], false, false).is_err());
    }
}
