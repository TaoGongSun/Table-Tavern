use super::{
    installed_untested, mac_cooldown, run_install_with_interval, try_begin, BeginOutcome,
    InstallProgress, InstallSpec,
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
fn command(script: &Path, action: &str, state: &Path, probes: &Path, login: &Path) -> Vec<String> {
    vec![
        script.to_string_lossy().into_owned(),
        action.to_owned(),
        state.to_string_lossy().into_owned(),
        probes.to_string_lossy().into_owned(),
        login.to_string_lossy().into_owned(),
    ]
}
#[cfg(windows)]
fn command(script: &Path, action: &str, state: &Path, probes: &Path, login: &Path) -> Vec<String> {
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
        install_envs: Vec::new(),
        install_fallback: None,
        force_install: false,
        login: command(script, login_action, &state, &probes, &login),
        logout: Vec::new(),
        probe: command(script, "probe", &state, &probes, &login),
        probe_expect: None,
        envs: Vec::new(),
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
 install-fail) echo x >> "$3.install"; exit 1 ;;
 install-latest) echo latest >> "$3.install"; exit 0 ;;
 probe) echo x >> "$3"; echo signed-in; [ -f "$2" ] ;;
 logout) rm -f "$2"; touch "$2.logout" ;;
 logout-fail) touch "$2.logout"; exit 1 ;;
 login-ok) touch "$2"; touch "$4" ;;
 login-no-state) touch "$4" ;;
 login-fail) touch "$4"; exit 1 ;;
 login-sleep) touch "$4"; sleep 3 ;;
esac"#;
#[cfg(windows)]
const SCRIPT: &str = r#"if "%1"=="install" exit /b 0
if "%1"=="probe" (echo x>> "%3" & echo signed-in & if exist "%2" (exit /b 0) else (exit /b 1))
if "%1"=="logout" (del /q "%2" 2>nul & type nul > "%2.logout" & exit /b 0)
if "%1"=="logout-fail" (type nul > "%2.logout" & exit /b 1)
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
    let (result, events) = run_case(&root.0, spec(&script, &root.0, false, "login-ok", 1)).await;
    assert!(result.is_ok());
    assert_eq!(
        stages(&events),
        ["detect", "install", "login", "verify", "done"]
    );
}
// 換帳號：舊憑證還在（state 存在）也要先登出再重登，否則 CLI 會直接說「已登入」
#[tokio::test]
async fn switch_account_logs_out_before_login() {
    let root = TestDir::new("switch");
    let script = write_stub(&root.0, SCRIPT);
    std::fs::write(root.0.join("state"), "ready").unwrap();
    let mut spec = spec(&script, &root.0, false, "login-ok", 1);
    spec.logout = command(
        &script,
        "logout",
        &root.0.join("state"),
        &root.0.join("probes"),
        &root.0.join("login"),
    );
    let (result, events) = run_case(&root.0, spec).await;
    assert!(result.is_ok());
    assert!(root.0.join("state.logout").exists());
    assert!(stages(&events).contains(&"login"));
}
// 登出失敗＝舊憑證還在，這時候登入只會回「已登入」，硬跑下去會假報切換成功
#[tokio::test]
async fn failed_logout_aborts_before_login() {
    let root = TestDir::new("switch-logout-fail");
    let script = write_stub(&root.0, SCRIPT);
    std::fs::write(root.0.join("state"), "ready").unwrap();
    let mut spec = spec(&script, &root.0, false, "login-ok", 1);
    spec.logout = command(
        &script,
        "logout-fail",
        &root.0.join("state"),
        &root.0.join("probes"),
        &root.0.join("login"),
    );
    let (result, events) = run_case(&root.0, spec).await;
    assert!(result.is_err());
    assert!(!root.0.join("login").exists());
    assert!(!stages(&events).contains(&"done"));
}
#[tokio::test]
async fn login_failure_never_probes() {
    let root = TestDir::new("login-fail");
    let script = write_stub(&root.0, SCRIPT);
    let (result, events) = run_case(&root.0, spec(&script, &root.0, false, "login-fail", 1)).await;
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
    let (result, events) = run_case(&root.0, spec(&script, &root.0, false, "login-sleep", 1)).await;
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
/// 已測版下載不到：改裝最新版，完成時註明「非已測版本」。
#[cfg(unix)]
#[tokio::test]
async fn pinned_install_failure_falls_back_to_latest_and_says_untested() {
    let root = TestDir::new("fallback");
    let script = write_stub(&root.0, SCRIPT);
    let mut spec = spec(&script, &root.0, false, "login-ok", 1);
    let state = root.0.join("state");
    let probes = root.0.join("probes");
    let login = root.0.join("login");
    spec.install = command(&script, "install-fail", &state, &probes, &login);
    spec.install_fallback = Some((
        command(&script, "install-latest", &state, &probes, &login),
        Vec::new(),
    ));
    let (result, events) = run_case(&root.0, spec).await;
    assert!(result.is_ok(), "{result:?}");
    let installs = std::fs::read_to_string(root.0.join("probes.install")).unwrap();
    assert_eq!(installs, "x\nlatest\n");
    let done = events.last().unwrap();
    assert_eq!(done.stage, "done");
    assert!(done
        .detail
        .as_deref()
        .unwrap()
        .contains("cli_installed_untested"));
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

/// 實際裝上的版號跟已測版比：不同（或解析不出來）＝非已測版本；沒有已測版可比的不判。
#[test]
fn installed_version_is_compared_with_the_tested_one() {
    let root = std::env::temp_dir();
    let mut spec = spec(Path::new("stub"), &root, false, "login-ok", 1);
    spec.id = "claude".to_owned();
    spec.install_fallback = Some((Vec::new(), Vec::new()));
    let tested = crate::cli::tested_versions::tested("claude").unwrap();
    assert_eq!(
        installed_untested(&spec, format!("{tested} (Claude Code)\n").as_bytes()),
        Some(false)
    );
    assert_eq!(
        installed_untested(&spec, b"9.9.9 (Claude Code)\n"),
        Some(true)
    );
    assert_eq!(installed_untested(&spec, b"nightly\n"), Some(true));
    spec.install_fallback = None;
    assert_eq!(installed_untested(&spec, b"9.9.9\n"), None);
}
