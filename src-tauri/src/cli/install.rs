use serde::Serialize;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct InstallSpec {
    pub id: String,
    /// 首次安裝：claude、codex、grok 裝已測版（plans/cli-version-gate.md 包 2 第 4 點），agy 裝最新版
    pub install: Vec<String>,
    /// 安裝那步的環境（codex 的 CODEX_RELEASE／CODEX_NON_INTERACTIVE）；不套 `envs`：安裝要進真家目錄
    pub install_envs: Vec<(String, String)>,
    /// 已測版下載不到時改裝最新版（指令, 環境）；agy 沒有（`install` 本身就是最新版）
    pub install_fallback: Option<(Vec<String>, Vec<(String, String)>)>,
    /// 上次 app 內更新還原失敗：執行檔在但跑不起來，不看「已經裝了」一律重裝
    pub force_install: bool,
    pub login: Vec<String>,
    // 切換帳號才會帶：登入前先清掉舊憑證，否則 CLI 看到 token 還沒過期就直接說「已登入」。
    // 非切換的情境由呼叫端清空這欄。
    pub logout: Vec<String>,
    pub probe: Vec<String>,
    // 探針輸出必須含這段字才算過；給那些未登入也可能 exit 0 的指令用。
    pub probe_expect: Option<String>,
    // 登入與探針要跑在哪組環境下（grok 走 app 專用 profile，其餘留空）。
    // 安裝那步刻意不套：安裝腳本要把 binary 裝進使用者真正的家目錄。
    pub envs: Vec<(String, String)>,
    pub pre_probe: bool,
    #[allow(dead_code)]
    pub window_title: String,
    // 登入視窗的等待上限；不是探針輪詢間隔。
    pub poll_seconds: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgress {
    pub provider: String,
    pub stage: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_path: Option<String>,
}

impl InstallProgress {
    fn new(provider: &str, stage: &'static str, log_path: &Path) -> Self {
        Self {
            provider: provider.to_owned(),
            stage,
            detail: None,
            log_path: Some(log_path.to_string_lossy().into_owned()),
        }
    }
}

struct CommandOutput {
    success: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

#[derive(Default)]
struct ProviderGuard {
    running: bool,
    last_start: Option<Instant>,
}

static PROVIDER_GUARDS: LazyLock<Mutex<HashMap<String, ProviderGuard>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub enum BeginOutcome {
    Started(RunToken),
    AlreadyRunning,
    Cooldown(u64),
}

pub struct RunToken {
    provider: String,
}

impl Drop for RunToken {
    fn drop(&mut self) {
        if let Ok(mut guards) = PROVIDER_GUARDS.lock() {
            if let Some(guard) = guards.get_mut(&self.provider) {
                guard.running = false;
            }
        }
    }
}

fn remaining_seconds(elapsed: Duration, cooldown: Duration) -> u64 {
    let remaining = cooldown.saturating_sub(elapsed);
    remaining.as_secs().max(1) + u64::from(remaining.subsec_nanos() > 0)
}

pub fn try_begin(provider: &str, cooldown: Duration) -> BeginOutcome {
    let now = Instant::now();
    let mut guards = PROVIDER_GUARDS
        .lock()
        .expect("provider guard mutex poisoned");
    let guard = guards.entry(provider.to_owned()).or_default();
    if guard.running {
        return BeginOutcome::AlreadyRunning;
    }
    if let Some(last_start) = guard.last_start {
        if now.duration_since(last_start) < cooldown {
            return BeginOutcome::Cooldown(remaining_seconds(
                now.duration_since(last_start),
                cooldown,
            ));
        }
    }
    guard.running = true;
    guard.last_start = Some(now);
    BeginOutcome::Started(RunToken {
        provider: provider.to_owned(),
    })
}

// macOS 的 Terminal 腳本無法回報結束，因此只記錄開始時間來避免重複喚起認證。
pub fn mac_cooldown(provider: &str, cooldown: Duration) -> Option<u64> {
    let now = Instant::now();
    let mut guards = PROVIDER_GUARDS
        .lock()
        .expect("provider guard mutex poisoned");
    let guard = guards.entry(provider.to_owned()).or_default();
    match guard.last_start {
        Some(last_start) if now.duration_since(last_start) < cooldown => {
            Some(remaining_seconds(now.duration_since(last_start), cooldown))
        }
        _ => {
            guard.last_start = Some(now);
            None
        }
    }
}

#[cfg(target_os = "windows")]
fn argv(program: impl Into<String>, args: &[&str]) -> Vec<String> {
    std::iter::once(program.into())
        .chain(args.iter().map(|value| (*value).to_owned()))
        .collect()
}

#[cfg(target_os = "windows")]
fn windows_binary(
    variable: &str,
    base: Option<std::ffi::OsString>,
    parts: &[&str],
) -> Result<String, String> {
    let mut path = PathBuf::from(
        base.ok_or_else(|| format!("Windows environment variable {variable} is missing"))?,
    );
    for part in parts {
        path.push(part);
    }
    Ok(path.to_string_lossy().into_owned())
}

/// 首次安裝指令：已測版（沒有就最新版）＋退路。
#[cfg(target_os = "windows")]
fn first_install_steps(
    id: &str,
) -> (
    Vec<String>,
    Vec<(String, String)>,
    Option<(Vec<String>, Vec<(String, String)>)>,
) {
    let (pinned, latest) =
        super::update::recipe::first_install(id, super::tested_versions::tested(id))
            .expect("四家都有安裝指令");
    match pinned {
        Some(pinned) => (pinned.argv, pinned.envs, Some((latest.argv, latest.envs))),
        None => (latest.argv, latest.envs, None),
    }
}

#[cfg(target_os = "windows")]
pub fn windows_specs() -> Result<Vec<InstallSpec>, String> {
    let profile = std::env::var_os("USERPROFILE");
    let local = std::env::var_os("LOCALAPPDATA");
    let claude = windows_binary(
        "USERPROFILE",
        profile.clone(),
        &[".local", "bin", "claude.exe"],
    )?;
    let codex = windows_binary(
        "LOCALAPPDATA",
        local.clone(),
        &["Programs", "OpenAI", "Codex", "bin", "codex.exe"],
    )?;
    let agy = windows_binary("LOCALAPPDATA", local, &["agy", "bin", "agy.exe"])?;
    let grok = windows_binary("USERPROFILE", profile, &[".grok", "bin", "grok.exe"])?;
    Ok(vec![
        InstallSpec {
            id: "claude".to_owned(),
            install: first_install_steps("claude").0,
            install_envs: first_install_steps("claude").1,
            install_fallback: first_install_steps("claude").2,
            force_install: false,
            login: argv(
                "cmd",
                &[
                    "/C",
                    "start",
                    "/WAIT",
                    "Table Tavern - Claude Login",
                    claude.as_str(),
                ],
            ),
            logout: argv(claude.clone(), &["auth", "logout"]),
            probe: argv(claude, &["-p", "ok"]),
            probe_expect: None,
            envs: Vec::new(),
            pre_probe: true,
            window_title: "Table Tavern - Claude Login".to_owned(),
            poll_seconds: 600,
        },
        InstallSpec {
            id: "codex".to_owned(),
            install: first_install_steps("codex").0,
            install_envs: first_install_steps("codex").1,
            install_fallback: first_install_steps("codex").2,
            force_install: false,
            login: argv(
                "cmd",
                &[
                    "/C",
                    "start",
                    "/WAIT",
                    "Table Tavern - Codex Login",
                    codex.as_str(),
                    "login",
                ],
            ),
            logout: argv(codex.clone(), &["logout"]),
            probe: argv(codex, &["login", "status"]),
            probe_expect: None,
            envs: Vec::new(),
            pre_probe: true,
            window_title: "Table Tavern - Codex Login".to_owned(),
            poll_seconds: 600,
        },
        // agy 未登入時的探針會啟動 OAuth，有副作用，登入前絕不可執行。
        InstallSpec {
            id: "agy".to_owned(),
            install: first_install_steps("agy").0,
            install_envs: first_install_steps("agy").1,
            install_fallback: first_install_steps("agy").2,
            force_install: false,
            login: argv(
                "cmd",
                &[
                    "/C",
                    "start",
                    "/WAIT",
                    "Table Tavern - Gemini Login",
                    agy.as_str(),
                    "-p",
                    "ok",
                ],
            ),
            // agy 沒有登入指令，也就沒有登出可清
            logout: Vec::new(),
            probe: argv(agy, &["-p", "ok"]),
            probe_expect: None,
            envs: Vec::new(),
            pre_probe: false,
            window_title: "Table Tavern - Gemini Login".to_owned(),
            poll_seconds: 600,
        },
        InstallSpec {
            id: "grok".to_owned(),
            install: first_install_steps("grok").0,
            install_envs: first_install_steps("grok").1,
            install_fallback: first_install_steps("grok").2,
            force_install: false,
            login: argv(
                "cmd",
                &[
                    "/C",
                    "start",
                    "/WAIT",
                    "Table Tavern - Grok Login",
                    grok.as_str(),
                    "login",
                ],
            ),
            // grok -p 會實跑一次 grok-4.5 推理（實測 26 秒，逼近 30 秒探針上限）；
            // models 只讀本機憑證、無 OAuth 副作用，故可在登入前先探。
            logout: argv(grok.clone(), &["logout"]),
            probe: argv(grok, &["models"]),
            probe_expect: Some("You are logged in".to_owned()),
            envs: Vec::new(),
            pre_probe: true,
            window_title: "Table Tavern - Grok Login".to_owned(),
            poll_seconds: 600,
        },
    ])
}

#[cfg(target_os = "windows")]
pub fn raise_login_window(title: &str) -> bool {
    use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, FindWindowW, GetWindowTextW, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };
    unsafe extern "system" fn find_containing(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let target = &mut *(lparam as *mut (&str, Option<HWND>));
        let mut text = [0_u16; 512];
        let length = GetWindowTextW(hwnd, text.as_mut_ptr(), text.len() as i32);
        if length > 0 && String::from_utf16_lossy(&text[..length as usize]).contains(target.0) {
            target.1 = Some(hwnd);
            return 0;
        }
        1
    }
    let wide: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
    let mut hwnd = unsafe { FindWindowW(std::ptr::null(), wide.as_ptr()) };
    if hwnd.is_null() {
        let mut target: (&str, Option<HWND>) = (title, None);
        unsafe {
            EnumWindows(Some(find_containing), &mut target as *mut _ as LPARAM);
        }
        hwnd = target.1.unwrap_or(std::ptr::null_mut());
    }
    !hwnd.is_null()
        && unsafe {
            ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd) != 0
        }
}

fn create_log(data_root: &Path, provider: &str) -> Result<(PathBuf, File), String> {
    crate::data::refuse_if_updating()?;
    let directory = data_root.join("install-logs");
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let path = directory.join(format!("install-{provider}-{timestamp}.log"));
    let file = OpenOptions::new()
        .create_new(true)
        .append(true)
        .open(&path)
        .map_err(|error| error.to_string())?;
    Ok((path, file))
}

fn append_output(log: &mut File, command: &[String], output: &CommandOutput) -> Result<(), String> {
    writeln!(log, "\n$ {}", command.join(" ")).map_err(|error| error.to_string())?;
    log.write_all(&output.stdout)
        .and_then(|_| log.write_all(&output.stderr))
        .and_then(|_| log.flush())
        .map_err(|error| error.to_string())
}

fn output_detail(output: &CommandOutput) -> Option<String> {
    let mut bytes = output.stdout.clone();
    bytes.extend_from_slice(&output.stderr);
    let detail = String::from_utf8_lossy(&bytes).trim().to_owned();
    (!detail.is_empty()).then_some(detail)
}

async fn run_hidden(
    command: &[String],
    envs: &[(String, String)],
) -> Result<CommandOutput, String> {
    let (program, args) = command
        .split_first()
        .ok_or_else(|| "empty command argv".to_owned())?;
    let mut child = Command::new(program);
    crate::cli::proxy::apply_system_proxy(&mut child);
    child
        .args(args)
        .envs(
            envs.iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        )
        .env_remove("PSModulePath")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    child.creation_flags(0x08000000);
    // 逾時（run_probe）時 spawned 被丟棄：grok 的在途票等收屍完才交回（cli::spawn）
    let mut spawned = super::spawn::spawn(&mut child, envs).map_err(|error| error.to_string())?;
    let output = spawned
        .wait_with_output()
        .await
        .map_err(|error| error.to_string())?;
    Ok(CommandOutput {
        success: output.status.success(),
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

const PROBE_TIMEOUT: Duration = Duration::from_secs(30);
async fn run_probe(command: &[String], envs: &[(String, String)]) -> Result<CommandOutput, String> {
    // 安裝／登入後的探測：claude／agy 的 `-p ok` 會真的跑一次推理，一律記進測試通道的 AI log
    #[cfg(feature = "test-harness")]
    let harness_dispatch = command.split_first().map(|(program, args)| {
        crate::harness::ai_dispatch(
            &format!("cli-probe:{program}"),
            &crate::harness::cli_model(args),
            serde_json::json!({ "aiProbe": crate::harness::is_ai_probe(args), "argv": args }),
        )
    });
    let result = tokio::time::timeout(PROBE_TIMEOUT, run_hidden(command, envs)).await;
    #[cfg(feature = "test-harness")]
    if let Some(id) = &harness_dispatch {
        let event = match &result {
            Ok(Err(_)) => "spawn-failed",
            Ok(Ok(_)) => "exited",
            Err(_) => "timed-out",
        };
        crate::harness::ai_event(id, event, serde_json::Value::Null);
    }
    match result {
        Ok(result) => result,
        Err(_) => Ok(CommandOutput {
            success: false,
            stdout: format!("probe timed out after {}s", PROBE_TIMEOUT.as_secs()).into_bytes(),
            stderr: Vec::new(),
        }),
    }
}

async fn run_terminal(
    command: &[String],
    timeout: Duration,
    envs: &[(String, String)],
) -> Result<CommandOutput, String> {
    let (program, args) = command
        .split_first()
        .ok_or_else(|| "empty command argv".to_owned())?;
    let mut child = Command::new(program);
    crate::cli::proxy::apply_system_proxy(&mut child);
    child.args(args).envs(
        envs.iter()
            .map(|(key, value)| (key.as_str(), value.as_str())),
    );
    // 隱藏外層 cmd 自己的黑視窗；使用者看到的登入視窗由內層 start 另開，不受影響
    #[cfg(target_os = "windows")]
    child.creation_flags(0x08000000);
    // 登入視窗：agy 的登入就是 `agy -p ok`，會真的跑一次推理
    #[cfg(feature = "test-harness")]
    if crate::harness::is_ai_probe(args) {
        crate::harness::ai_dispatch(
            &format!("cli-login-window:{program}"),
            &crate::harness::cli_model(args),
            serde_json::json!({ "aiProbe": true, "argv": args }),
        );
    }
    let mut spawned = super::spawn::spawn(&mut child, envs).map_err(|error| error.to_string())?;
    match tokio::time::timeout(timeout, spawned.wait()).await {
        Ok(status) => Ok(CommandOutput {
            success: status.map_err(|error| error.to_string())?.success(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        }),
        Err(_) => Ok(CommandOutput {
            success: false,
            stdout: b"login window timed out".to_vec(),
            stderr: Vec::new(),
        }),
    }
}

fn emit_error(
    provider: &str,
    log_path: &Path,
    detail: String,
    emit: &mut impl FnMut(InstallProgress),
) -> String {
    let mut progress = InstallProgress::new(provider, "error", log_path);
    progress.detail = Some(detail.clone());
    emit(progress);
    detail
}

#[cfg(target_os = "windows")]
pub async fn run_install(
    spec: InstallSpec,
    data_root: &Path,
    detect: impl FnMut(&str) -> Option<PathBuf>,
    emit: impl FnMut(InstallProgress),
) -> Result<PathBuf, String> {
    run_install_with_interval(spec, data_root, detect, emit, Duration::from_secs(5)).await
}

async fn checked_probe(
    spec: &InstallSpec,
    log: &mut File,
    log_path: &Path,
    emit: &mut impl FnMut(InstallProgress),
) -> Result<bool, String> {
    emit(InstallProgress::new(&spec.id, "verify", log_path));
    let output = run_probe(&spec.probe, &spec.envs).await?;
    append_output(log, &spec.probe, &output)?;
    let expected = spec
        .probe_expect
        .as_ref()
        .is_none_or(|needle| String::from_utf8_lossy(&output.stdout).contains(needle.as_str()));
    Ok(output.success && expected)
}

/// 有已測版可裝（帶退路）的家：實際裝上的版號不等於已測版＝非已測版本（版號解析不出來也算）。
/// 沒有可比的（agy 只能裝最新版）回 None，維持「有沒有走退路」的判斷。
fn installed_untested(spec: &InstallSpec, version_stdout: &[u8]) -> Option<bool> {
    let tested = spec
        .install_fallback
        .as_ref()
        .and(super::tested_versions::tested(&spec.id))?;
    let line = String::from_utf8_lossy(version_stdout);
    let installed = line.lines().next().and_then(super::tested_versions::parse);
    Some(installed.is_none_or(|installed| installed.to_string() != tested))
}

async fn run_install_with_interval(
    spec: InstallSpec,
    data_root: &Path,
    mut detect: impl FnMut(&str) -> Option<PathBuf>,
    mut emit: impl FnMut(InstallProgress),
    poll_interval: Duration,
) -> Result<PathBuf, String> {
    let (log_path, mut log) = create_log(data_root, &spec.id)?;
    emit(InstallProgress::new(&spec.id, "detect", &log_path));
    // 實際裝上的不是已測版（已測版下載不到改裝最新版，或腳本裝了別版）：完成時註明「非已測版本」
    let mut untested = false;
    if spec.force_install || detect(&spec.id).is_none() {
        emit(InstallProgress::new(&spec.id, "install", &log_path));
        let mut output = run_hidden(&spec.install, &spec.install_envs)
            .await
            .map_err(|error| emit_error(&spec.id, &log_path, error, &mut emit))?;
        append_output(&mut log, &spec.install, &output)?;
        if let (false, Some((latest, envs))) = (output.success, &spec.install_fallback) {
            untested = true;
            output = run_hidden(latest, envs)
                .await
                .map_err(|error| emit_error(&spec.id, &log_path, error, &mut emit))?;
            append_output(&mut log, latest, &output)?;
        }
        if !output.success {
            return Err(emit_error(
                &spec.id,
                &log_path,
                output_detail(&output)
                    .unwrap_or_else(|| "install command exited with a non-zero status".to_owned()),
                &mut emit,
            ));
        }
        // 裝完以 --version 確認 app 會用的那支跑得起來
        if let Some(binary) = detect(&spec.id) {
            let command = vec![
                binary.to_string_lossy().into_owned(),
                "--version".to_owned(),
            ];
            let output = run_probe(&command, &spec.envs)
                .await
                .map_err(|error| emit_error(&spec.id, &log_path, error, &mut emit))?;
            append_output(&mut log, &command, &output)?;
            if !output.success {
                return Err(emit_error(
                    &spec.id,
                    &log_path,
                    output_detail(&output)
                        .unwrap_or_else(|| "installed CLI does not run".to_owned()),
                    &mut emit,
                ));
            }
            if let Some(differs) = installed_untested(&spec, &output.stdout) {
                untested = differs;
            }
        }
    }
    let done = |log_path: &Path| {
        let mut progress = InstallProgress::new(&spec.id, "done", log_path);
        if untested {
            progress.detail = Some(crate::ui_msg::UiMsg::CliInstalledUntested.to_string());
        }
        progress
    };
    if spec.pre_probe
        && checked_probe(&spec, &mut log, &log_path, &mut emit)
            .await
            .map_err(|error| emit_error(&spec.id, &log_path, error, &mut emit))?
    {
        emit(done(&log_path));
        return Ok(log_path);
    }
    // 切換帳號那條會帶 logout：舊憑證還沒過期時，login 會直接說「已登入」什麼都不做，
    // 先清掉才換得了人。登出失敗就停在這裡——硬往下走只會拿舊帳號假報一次「切換成功」。
    if !spec.logout.is_empty() {
        let logout_output = run_terminal(&spec.logout, Duration::from_secs(60), &spec.envs)
            .await
            .map_err(|error| emit_error(&spec.id, &log_path, error, &mut emit))?;
        append_output(&mut log, &spec.logout, &logout_output)?;
        if !logout_output.success {
            let suffix = output_detail(&logout_output)
                .unwrap_or_else(|| "logout command exited with a non-zero status".to_owned());
            return Err(emit_error(
                &spec.id,
                &log_path,
                format!("account switch aborted, old credentials are still in place: {suffix}"),
                &mut emit,
            ));
        }
    }
    emit(InstallProgress::new(&spec.id, "login", &log_path));
    #[cfg(target_os = "windows")]
    {
        let title = spec.window_title.clone();
        tokio::spawn(async move {
            for _ in 0..10 {
                if raise_login_window(&title) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        });
    }
    let login_output = run_terminal(
        &spec.login,
        Duration::from_secs(spec.poll_seconds),
        &spec.envs,
    )
    .await
    .map_err(|error| emit_error(&spec.id, &log_path, error, &mut emit))?;
    append_output(&mut log, &spec.login, &login_output)?;
    if !login_output.success {
        let suffix = output_detail(&login_output)
            .unwrap_or_else(|| "login command exited with a non-zero status".to_owned());
        return Err(emit_error(
            &spec.id,
            &log_path,
            format!("login window closed or timed out: {suffix}"),
            &mut emit,
        ));
    }
    if checked_probe(&spec, &mut log, &log_path, &mut emit)
        .await
        .map_err(|error| emit_error(&spec.id, &log_path, error, &mut emit))?
    {
        emit(done(&log_path));
        return Ok(log_path);
    }
    tokio::time::sleep(poll_interval).await;
    if checked_probe(&spec, &mut log, &log_path, &mut emit)
        .await
        .map_err(|error| emit_error(&spec.id, &log_path, error, &mut emit))?
    {
        emit(done(&log_path));
        return Ok(log_path);
    }
    Err(emit_error(
        &spec.id,
        &log_path,
        "login window closed or timed out: verification failed after login".to_owned(),
        &mut emit,
    ))
}

#[cfg(test)]
mod tests;
