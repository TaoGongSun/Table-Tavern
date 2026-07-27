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
    pub install: Vec<String>,
    pub login: Vec<String>,
    pub probe: Vec<String>,
    // 探針輸出必須含這段字才算過；給那些未登入也可能 exit 0 的指令用。
    pub probe_expect: Option<String>,
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

// $ErrorActionPreference='Stop'：irm|iex 內部錯誤預設不改 exit code，會誤判安裝成功
#[cfg(target_os = "windows")]
fn ps_install(url: &str) -> Vec<String> {
    argv(
        "powershell",
        &[
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &format!("$ErrorActionPreference='Stop'; irm {url} | iex"),
        ],
    )
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
            install: ps_install("https://claude.ai/install.ps1"),
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
            probe: argv(claude, &["-p", "ok"]),
            probe_expect: None,
            pre_probe: true,
            window_title: "Table Tavern - Claude Login".to_owned(),
            poll_seconds: 600,
        },
        InstallSpec {
            id: "codex".to_owned(),
            install: ps_install("https://chatgpt.com/codex/install.ps1"),
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
            probe: argv(codex, &["login", "status"]),
            probe_expect: None,
            pre_probe: true,
            window_title: "Table Tavern - Codex Login".to_owned(),
            poll_seconds: 600,
        },
        // agy 未登入時的探針會啟動 OAuth，有副作用，登入前絕不可執行。
        InstallSpec {
            id: "agy".to_owned(),
            install: ps_install("https://antigravity.google/cli/install.ps1"),
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
            probe: argv(agy, &["-p", "ok"]),
            probe_expect: None,
            pre_probe: false,
            window_title: "Table Tavern - Gemini Login".to_owned(),
            poll_seconds: 600,
        },
        InstallSpec {
            id: "grok".to_owned(),
            install: ps_install("https://x.ai/cli/install.ps1"),
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
            probe: argv(grok, &["models"]),
            probe_expect: Some("You are logged in".to_owned()),
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

async fn run_hidden(command: &[String]) -> Result<CommandOutput, String> {
    let (program, args) = command
        .split_first()
        .ok_or_else(|| "empty command argv".to_owned())?;
    let mut child = Command::new(program);
    child
        .args(args)
        .env_remove("PSModulePath")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(target_os = "windows")]
    child.creation_flags(0x08000000);
    let output = child.output().await.map_err(|error| error.to_string())?;
    Ok(CommandOutput {
        success: output.status.success(),
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

const PROBE_TIMEOUT: Duration = Duration::from_secs(30);
async fn run_probe(command: &[String]) -> Result<CommandOutput, String> {
    match tokio::time::timeout(PROBE_TIMEOUT, run_hidden(command)).await {
        Ok(result) => result,
        Err(_) => Ok(CommandOutput {
            success: false,
            stdout: format!("probe timed out after {}s", PROBE_TIMEOUT.as_secs()).into_bytes(),
            stderr: Vec::new(),
        }),
    }
}

async fn run_terminal(command: &[String], timeout: Duration) -> Result<CommandOutput, String> {
    let (program, args) = command
        .split_first()
        .ok_or_else(|| "empty command argv".to_owned())?;
    let mut child = Command::new(program);
    child.args(args).kill_on_drop(true);
    // 隱藏外層 cmd 自己的黑視窗；使用者看到的登入視窗由內層 start 另開，不受影響
    #[cfg(target_os = "windows")]
    child.creation_flags(0x08000000);
    match tokio::time::timeout(timeout, child.status()).await {
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
    let output = run_probe(&spec.probe).await?;
    append_output(log, &spec.probe, &output)?;
    let expected = spec.probe_expect.as_ref().is_none_or(|needle| {
        String::from_utf8_lossy(&output.stdout).contains(needle.as_str())
    });
    Ok(output.success && expected)
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
    if detect(&spec.id).is_none() {
        emit(InstallProgress::new(&spec.id, "install", &log_path));
        let output = run_hidden(&spec.install)
            .await
            .map_err(|error| emit_error(&spec.id, &log_path, error, &mut emit))?;
        append_output(&mut log, &spec.install, &output)?;
        if !output.success {
            return Err(emit_error(
                &spec.id,
                &log_path,
                output_detail(&output)
                    .unwrap_or_else(|| "install command exited with a non-zero status".to_owned()),
                &mut emit,
            ));
        }
    }
    if spec.pre_probe
        && checked_probe(&spec, &mut log, &log_path, &mut emit)
            .await
            .map_err(|error| emit_error(&spec.id, &log_path, error, &mut emit))?
    {
        emit(InstallProgress::new(&spec.id, "done", &log_path));
        return Ok(log_path);
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
    let login_output = run_terminal(&spec.login, Duration::from_secs(spec.poll_seconds))
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
        emit(InstallProgress::new(&spec.id, "done", &log_path));
        return Ok(log_path);
    }
    tokio::time::sleep(poll_interval).await;
    if checked_probe(&spec, &mut log, &log_path, &mut emit)
        .await
        .map_err(|error| emit_error(&spec.id, &log_path, error, &mut emit))?
    {
        emit(InstallProgress::new(&spec.id, "done", &log_path));
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
mod tests {
    use super::{
        mac_cooldown, run_install_with_interval, try_begin, BeginOutcome, InstallProgress,
        InstallSpec,
    };
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    static TEST_ID: AtomicU64 = AtomicU64::new(0);
    struct TestDir(PathBuf);
    impl TestDir {
        fn new(name: &str) -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let id = TEST_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "table-tavern-install-{name}-{}-{stamp}-{id}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[cfg(unix)]
    fn write_stub(root: &Path, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = root.join("stub.sh");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }
    #[cfg(windows)]
    fn write_stub(root: &Path, body: &str) -> PathBuf {
        let path = root.join("stub.cmd");
        std::fs::write(&path, format!("@echo off\r\n{body}\r\n")).unwrap();
        path
    }
    #[cfg(unix)]
    fn command(
        script: &Path,
        action: &str,
        state: &Path,
        probes: &Path,
        login: &Path,
    ) -> Vec<String> {
        vec![
            script.to_string_lossy().into_owned(),
            action.to_owned(),
            state.to_string_lossy().into_owned(),
            probes.to_string_lossy().into_owned(),
            login.to_string_lossy().into_owned(),
        ]
    }
    #[cfg(windows)]
    fn command(
        script: &Path,
        action: &str,
        state: &Path,
        probes: &Path,
        login: &Path,
    ) -> Vec<String> {
        vec![
            "cmd".to_owned(),
            "/C".to_owned(),
            script.to_string_lossy().into_owned(),
            action.to_owned(),
            state.to_string_lossy().into_owned(),
            probes.to_string_lossy().into_owned(),
            login.to_string_lossy().into_owned(),
        ]
    }
    fn spec(
        script: &Path,
        root: &Path,
        pre_probe: bool,
        login_action: &str,
        poll_seconds: u64,
    ) -> InstallSpec {
        let state = root.join("state");
        let probes = root.join("probes");
        let login = root.join("login");
        InstallSpec {
            id: "stub".to_owned(),
            install: command(script, "install", &state, &probes, &login),
            login: command(script, login_action, &state, &probes, &login),
            probe: command(script, "probe", &state, &probes, &login),
            probe_expect: None,
            pre_probe,
            window_title: "stub".to_owned(),
            poll_seconds,
        }
    }
    async fn run_case(
        root: &Path,
        spec: InstallSpec,
    ) -> (Result<PathBuf, String>, Vec<InstallProgress>) {
        let mut events = Vec::new();
        let result = run_install_with_interval(
            spec,
            root,
            |_| None,
            |event| events.push(event),
            Duration::from_millis(10),
        )
        .await;
        (result, events)
    }
    fn stages(events: &[InstallProgress]) -> Vec<&str> {
        events.iter().map(|event| event.stage).collect()
    }
    #[cfg(unix)]
    const SCRIPT: &str = r#"case "$1" in
 install) exit 0 ;;
 probe) echo x >> "$3"; echo signed-in; [ -f "$2" ] ;;
 login-ok) touch "$2"; touch "$4" ;;
 login-no-state) touch "$4" ;;
 login-fail) touch "$4"; exit 1 ;;
 login-sleep) touch "$4"; sleep 3 ;;
esac"#;
    #[cfg(windows)]
    const SCRIPT: &str = r#"if "%1"=="install" exit /b 0
if "%1"=="probe" (echo x>> "%3" & echo signed-in & if exist "%2" (exit /b 0) else (exit /b 1))
if "%1"=="login-ok" (type nul > "%2" & type nul > "%4" & exit /b 0)
if "%1"=="login-no-state" (type nul > "%4" & exit /b 0)
if "%1"=="login-fail" (type nul > "%4" & exit /b 1)
if "%1"=="login-sleep" (type nul > "%4" & timeout /t 3 /nobreak >nul & exit /b 0)"#;
    #[tokio::test]
    async fn pre_probe_green_skips_login() {
        let root = TestDir::new("pre-green");
        let script = write_stub(&root.0, SCRIPT);
        std::fs::write(root.0.join("state"), "ready").unwrap();
        let (result, events) = run_case(&root.0, spec(&script, &root.0, true, "login-ok", 1)).await;
        assert!(result.is_ok());
        assert_eq!(stages(&events), ["detect", "install", "verify", "done"]);
        assert!(!root.0.join("login").exists());
    }
    // 探針 exit 0 但輸出沒有登入字樣時不得算過（grok models 未登入是否也 exit 0 無法驗證）
    #[tokio::test]
    async fn probe_expect_mismatch_forces_login() {
        let root = TestDir::new("expect-miss");
        let script = write_stub(&root.0, SCRIPT);
        std::fs::write(root.0.join("state"), "ready").unwrap();
        let mut spec = spec(&script, &root.0, true, "login-ok", 1);
        spec.probe_expect = Some("nope".to_owned());
        let (_, events) = run_case(&root.0, spec).await;
        assert!(stages(&events).contains(&"login"));
    }
    #[tokio::test]
    async fn probe_expect_match_skips_login() {
        let root = TestDir::new("expect-hit");
        let script = write_stub(&root.0, SCRIPT);
        std::fs::write(root.0.join("state"), "ready").unwrap();
        let mut spec = spec(&script, &root.0, true, "login-ok", 1);
        spec.probe_expect = Some("signed-in".to_owned());
        let (result, events) = run_case(&root.0, spec).await;
        assert!(result.is_ok());
        assert!(!stages(&events).contains(&"login"));
    }
    #[tokio::test]
    async fn login_success_verifies_once() {
        let root = TestDir::new("login-green");
        let script = write_stub(&root.0, SCRIPT);
        let (result, events) =
            run_case(&root.0, spec(&script, &root.0, false, "login-ok", 1)).await;
        assert!(result.is_ok());
        assert_eq!(
            stages(&events),
            ["detect", "install", "login", "verify", "done"]
        );
    }
    #[tokio::test]
    async fn login_failure_never_probes() {
        let root = TestDir::new("login-fail");
        let script = write_stub(&root.0, SCRIPT);
        let (result, events) =
            run_case(&root.0, spec(&script, &root.0, false, "login-fail", 1)).await;
        assert!(result.is_err());
        assert_eq!(events.last().unwrap().stage, "error");
        assert!(!root.0.join("probes").exists());
    }
    #[tokio::test]
    async fn failed_verification_probes_twice() {
        let root = TestDir::new("verify-fail");
        let script = write_stub(&root.0, SCRIPT);
        let (result, events) =
            run_case(&root.0, spec(&script, &root.0, false, "login-no-state", 1)).await;
        assert!(result.is_err());
        assert_eq!(
            std::fs::read_to_string(root.0.join("probes"))
                .unwrap()
                .lines()
                .count(),
            2
        );
        assert_eq!(events.last().unwrap().stage, "error");
    }
    #[tokio::test]
    async fn login_timeout_reports_timeout() {
        let root = TestDir::new("login-timeout");
        let script = write_stub(&root.0, SCRIPT);
        let (result, events) =
            run_case(&root.0, spec(&script, &root.0, false, "login-sleep", 1)).await;
        assert!(result.unwrap_err().contains("timed out"));
        assert_eq!(events.last().unwrap().stage, "error");
    }
    #[test]
    fn provider_guard_enforces_running_and_cooldown() {
        let provider = format!("guard-{}", TEST_ID.fetch_add(1, Ordering::Relaxed));
        let token = match try_begin(&provider, Duration::from_millis(300)) {
            BeginOutcome::Started(token) => token,
            _ => panic!("expected start"),
        };
        assert!(matches!(
            try_begin(&provider, Duration::from_millis(300)),
            BeginOutcome::AlreadyRunning
        ));
        drop(token);
        assert!(
            matches!(try_begin(&provider, Duration::from_millis(300)), BeginOutcome::Cooldown(seconds) if seconds > 0)
        );
        std::thread::sleep(Duration::from_millis(350));
        assert!(matches!(
            try_begin(&provider, Duration::from_millis(300)),
            BeginOutcome::Started(_)
        ));
    }
    #[test]
    fn mac_guard_enforces_cooldown() {
        let provider = format!("mac-{}", TEST_ID.fetch_add(1, Ordering::Relaxed));
        assert!(mac_cooldown(&provider, Duration::from_millis(300)).is_none());
        assert!(
            matches!(mac_cooldown(&provider, Duration::from_millis(300)), Some(seconds) if seconds > 0)
        );
        std::thread::sleep(Duration::from_millis(350));
        assert!(mac_cooldown(&provider, Duration::from_millis(300)).is_none());
    }
    #[cfg(windows)]
    async fn real_install_smoke(provider: &str) {
        let mut spec = super::windows_specs()
            .unwrap()
            .into_iter()
            .find(|spec| spec.id == provider)
            .unwrap();
        spec.poll_seconds = 60;
        let probe_path = PathBuf::from(&spec.probe[0]);
        let data_root = std::env::temp_dir().join("tt-smoke");
        std::fs::create_dir_all(&data_root).unwrap();
        let mut events = Vec::new();
        let result = run_install_with_interval(
            spec,
            &data_root,
            |_| None,
            |event| events.push(event),
            Duration::from_secs(5),
        )
        .await;
        assert!(
            probe_path.exists(),
            "{provider}: installed binary missing at {probe_path:?}"
        );
        assert!(events.iter().any(|event| event.stage == "login"));
        let last = events.last().unwrap();
        assert!(matches!(last.stage, "done" | "error"));
        assert!(Path::new(last.log_path.as_ref().unwrap()).exists());
        let _ = result;
    }
    #[cfg(windows)]
    #[tokio::test]
    #[ignore]
    async fn real_install_claude() {
        real_install_smoke("claude").await;
    }
    #[cfg(windows)]
    #[tokio::test]
    #[ignore]
    async fn real_install_codex() {
        real_install_smoke("codex").await;
    }
    #[cfg(windows)]
    #[tokio::test]
    #[ignore]
    async fn real_install_agy() {
        real_install_smoke("agy").await;
    }
    #[cfg(windows)]
    #[tokio::test]
    #[ignore]
    async fn real_install_grok() {
        real_install_smoke("grok").await;
    }
}
