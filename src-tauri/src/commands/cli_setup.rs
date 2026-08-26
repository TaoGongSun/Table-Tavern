use crate::ai_transport::cli_envs;
#[cfg(target_os = "windows")]
use crate::cli;
use crate::{data_root, install};
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
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

/// `envs`：登入與探針要跑在哪組環境（grok 用 app 專用 profile）。
/// 安裝那步刻意不套前綴——安裝腳本要把 binary 裝進使用者真正的家目錄。
/// `switch_account`：換帳號。舊憑證沒過期時前置探針會直接判定「已登入」收工，
/// 登入那行根本跑不到，所以這條要略過前置探針並先登出。
fn cli_install_script(
    provider: &str,
    messages: &InstallMessages,
    envs: &[(String, String)],
    switch_account: bool,
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
    // logout：換帳號時登入前先清憑證。agy 只有 TUI 內的 /logout，沒有非互動指令，故為 None——
    // 它的換帳號要玩家自己在 agy 裡 /logout 後再回來按登入。
    let (install_command, login_command, logout_command, probe_command, poll_seconds) = match provider
    {
        "claude" => (
            "curl -fsSL https://claude.ai/install.sh | bash",
            Some("claude auth login"),
            Some("claude auth logout"),
            "claude -p \"ok\"",
            120,
        ),
        "codex" => (
            "curl -fsSL https://chatgpt.com/codex/install.sh | sh",
            Some("codex login"),
            Some("codex logout"),
            // codex exec 在非 git 目錄會拒跑，probe 改用即時且不耗額度的 login status
            "codex login status",
            120,
        ),
        "agy" => (
            "curl -fsSL https://antigravity.google/cli/install.sh | bash",
            None,
            None,
            "agy -p \"ok\"",
            600,
        ),
        "grok" => (
            "curl -fsSL https://x.ai/cli/install.sh | bash",
            Some("grok login"),
            Some("grok logout"),
            // grok -p 會真的跑一次 grok-4.5 推理（實測 26 秒又燒額度）；models 只讀本機憑證，0.8 秒。
            // 未登入時它是否照樣 exit 0 無法驗證，故以登入字串判定，判錯也只是多要求登入一次。
            "grok models 2>/dev/null | grep -q '^You are logged in'",
            120,
        ),
        _ => return Err(format!("unsupported CLI provider: {provider}")),
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
if ! command -v {provider} >/dev/null 2>&1; then
  {install_command} || {{ echo {fail}; exit 1; }}
fi
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
    let directory = data_root(&app)?;
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
        // 登入與探針都跑在 app 自己的 profile，否則會登在使用者的 ~/.grok 卻用不到
        spec.envs = cli_envs(&app, &provider)?;
        // 換帳號：略過前置探針（舊憑證有效也要重登）並保留 logout；平常那條把 logout 清掉
        match switch_account && !spec.logout.is_empty() {
            true => spec.pre_probe = false,
            false => spec.logout.clear(),
        }
        let token = match install::try_begin(&provider, Duration::from_secs(60)) {
            install::BeginOutcome::Started(token) => {
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

        // 冷卻中＝這次沒有真的開跑（前一輪可能還開著，也可能剛結束）。回錯誤讓前端知道，
        // 別把它當成「破壞性流程已啟動」而清掉還有效的已連結徽章。
        if let Some(seconds) = install::mac_cooldown(&provider, Duration::from_secs(60)) {
            Command::new("open")
                .args(["-a", "Terminal"])
                .spawn()
                .map_err(|error| error.to_string())?;
            return Err(format!("login-cooldown:{seconds}"));
        }
        let _ = std::fs::remove_file(&sentinel_path);
        let script =
            cli_install_script(&provider, &messages, &cli_envs(&app, &provider)?, switch_account)?;
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

#[cfg(test)]
mod tests {
    use super::{cli_install_script, InstallMessages};
    use std::path::PathBuf;

    fn messages() -> InstallMessages {
        InstallMessages {
            start: "start".to_owned(),
            login_hint: "login hint".to_owned(),
            success: "success".to_owned(),
            fail: "fail".to_owned(),
        }
    }

    fn assert_messages(script: &str) {
        for text in ["start", "login hint", "success", "fail"] {
            assert!(script.contains(text));
        }
    }

    #[test]
    fn claude_install_script_contains_messages_and_flow() {
        let script = cli_install_script("claude", &messages(), &[], false).unwrap();
        assert_messages(&script);
        assert!(script.contains("curl -fsSL https://claude.ai/install.sh | bash"));
        assert!(script.contains("claude auth login"));
        assert!(script.contains("claude -p \"ok\" >/dev/null 2>&1"));
        assert!(script.contains("while [ \"$elapsed\" -lt 120 ]"));
    }

    #[test]
    fn codex_install_script_contains_messages_and_flow() {
        let script = cli_install_script("codex", &messages(), &[], false).unwrap();
        assert_messages(&script);
        assert!(script.contains("curl -fsSL https://chatgpt.com/codex/install.sh | sh"));
        assert!(script.contains("codex login"));
        assert!(script.contains("codex login status >/dev/null 2>&1"));
        assert!(script.contains("while [ \"$elapsed\" -lt 120 ]"));
    }

    #[test]
    fn agy_provider_script_contains_messages_and_flow() {
        let script = cli_install_script("agy", &messages(), &[], false).unwrap();
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
        let script = cli_install_script("grok", &messages(), &[], false).unwrap();
        assert_messages(&script);
        assert!(script.contains("curl -fsSL https://x.ai/cli/install.sh | bash"));
        assert!(script.contains("grok login"));
        assert!(script.contains("grok models 2>/dev/null | grep -q '^You are logged in'"));
        assert!(script.contains("while [ \"$elapsed\" -lt 120 ]"));
    }

    #[test]
    fn grok_install_script_runs_login_and_probe_in_the_app_profile() {
        let envs = crate::cli::grok_envs(
            &PathBuf::from("/app/cli-home"),
            &PathBuf::from("/app/grok-home"),
        );
        let script = cli_install_script("grok", &messages(), &envs, false).unwrap();
        // 登入與探針都要掛上 env 前綴，否則會登在使用者的 ~/.grok、或探到終端機的登入態
        assert!(script.contains(
            "env HOME='/app/cli-home' USERPROFILE='/app/cli-home' GROK_HOME='/app/grok-home' GROK_CONFIG='{\"models\":{\"temperature\":1.1,\"top_p\":1.0}}' grok login"
        ));
        assert!(script.contains(
            "GROK_CONFIG='{\"models\":{\"temperature\":1.1,\"top_p\":1.0}}' grok models 2>/dev/null"
        ));
        // 安裝那行不帶：安裝腳本要把 binary 裝進使用者真正的家目錄
        assert!(script.contains("  curl -fsSL https://x.ai/cli/install.sh | bash ||"));
    }

    /// 換帳號：舊憑證沒過期時前置探針會直接判定已登入，所以那條必須短路成 false 並先登出
    #[test]
    fn switch_account_script_logs_out_and_skips_the_pre_probe() {
        let script = cli_install_script("grok", &messages(), &[], true).unwrap();
        // 登出失敗要當場中止，不能帶著舊憑證往下登入
        assert!(script.contains("grok logout || { echo 'fail'; exit 1; }"));
        assert!(script.contains("if false; then"));
        assert!(script.contains("grok login"));
        // 登出要在登入之前，否則清掉的是剛換上的新帳號
        assert!(script.find("grok logout").unwrap() < script.find("grok login").unwrap());
        // 平常那條完全不碰登出，前置探針照舊
        let plain = cli_install_script("grok", &messages(), &[], false).unwrap();
        assert!(!plain.contains("grok logout"));
        assert!(plain.contains("if env grok models 2>/dev/null | grep -q '^You are logged in'")
            || plain.contains("if grok models 2>/dev/null | grep -q '^You are logged in'"));
        // agy 沒有非互動登出指令，換帳號旗標對它無效——維持前置探針，不空等一輪輪詢
        let agy = cli_install_script("agy", &messages(), &[], true).unwrap();
        assert!(!agy.contains("if false; then"));
        assert!(agy.contains("agy -p \"ok\" >/dev/null 2>&1"));
    }

    #[test]
    fn install_script_touches_sentinel_only_after_verification_passes() {
        let script = cli_install_script("claude", &messages(), &[], false).unwrap();
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
        assert!(cli_install_script("agy", &quoted_messages, &[], false)
            .unwrap()
            .contains("'don'\"'\"'t'"));
        assert!(cli_install_script("unknown", &messages(), &[], false).is_err());
    }
}
