use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct InstallSpec {
    pub id: String,
    pub install: Vec<String>,
    pub login: Vec<String>,
    pub probe: Vec<String>,
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
            login: argv("cmd", &["/C", "start", "", claude.as_str()]),
            probe: argv(claude, &["-p", "ok"]),
            poll_seconds: 600,
        },
        InstallSpec {
            id: "codex".to_owned(),
            install: ps_install("https://chatgpt.com/codex/install.ps1"),
            login: argv("cmd", &["/C", "start", "", codex.as_str(), "login"]),
            probe: argv(codex, &["login", "status"]),
            poll_seconds: 600,
        },
        InstallSpec {
            id: "agy".to_owned(),
            install: ps_install("https://antigravity.google/cli/install.ps1"),
            login: argv("cmd", &["/C", "start", "", agy.as_str(), "-p", "ok"]),
            probe: argv(agy, &["-p", "ok"]),
            poll_seconds: 600,
        },
        InstallSpec {
            id: "grok".to_owned(),
            install: ps_install("https://x.ai/cli/install.ps1"),
            login: argv("cmd", &["/C", "start", "", grok.as_str(), "login"]),
            probe: argv(grok, &["-p", "ok"]),
            poll_seconds: 600,
        },
    ])
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
        // pwsh 7 會把 PSModulePath 指到自己的模組庫，Windows PowerShell 5.1 繼承後
        // 連 Get-FileHash 等內建 cmdlet 都解析不到；清掉讓它重建預設值
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

// grok -p 未登入行為官方無載：若探針阻塞等互動，30 秒斷頭視同未綠，避免吊死輪詢
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

async fn run_terminal(command: &[String]) -> Result<CommandOutput, String> {
    let (program, args) = command
        .split_first()
        .ok_or_else(|| "empty command argv".to_owned())?;
    let status = Command::new(program)
        .args(args)
        .status()
        .await
        .map_err(|error| error.to_string())?;
    Ok(CommandOutput {
        success: status.success(),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
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
    run_install_with_interval(
        spec,
        data_root,
        detect,
        emit,
        Duration::from_secs(5),
    )
    .await
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
        let output = match run_hidden(&spec.install).await {
            Ok(output) => output,
            Err(error) => {
                return Err(emit_error(&spec.id, &log_path, error, &mut emit));
            }
        };
        append_output(&mut log, &spec.install, &output)?;
        if !output.success {
            let detail = output_detail(&output)
                .unwrap_or_else(|| "install command exited with a non-zero status".to_owned());
            return Err(emit_error(&spec.id, &log_path, detail, &mut emit));
        }
    }

    emit(InstallProgress::new(&spec.id, "verify", &log_path));
    let initial_probe = match run_probe(&spec.probe).await {
        Ok(output) => output,
        Err(error) => {
            return Err(emit_error(&spec.id, &log_path, error, &mut emit));
        }
    };
    append_output(&mut log, &spec.probe, &initial_probe)?;
    if initial_probe.success {
        emit(InstallProgress::new(&spec.id, "done", &log_path));
        return Ok(log_path);
    }

    emit(InstallProgress::new(&spec.id, "login", &log_path));
    let login_output = match run_terminal(&spec.login).await {
        Ok(output) => output,
        Err(error) => return Err(emit_error(&spec.id, &log_path, error, &mut emit)),
    };
    append_output(&mut log, &spec.login, &login_output)?;

    let timeout = Duration::from_secs(spec.poll_seconds);
    let mut elapsed = Duration::ZERO;
    while elapsed < timeout {
        let delay = poll_interval.min(timeout - elapsed);
        tokio::time::sleep(delay).await;
        elapsed += delay;
        emit(InstallProgress::new(&spec.id, "verify", &log_path));
        let output = match run_probe(&spec.probe).await {
            Ok(output) => output,
            Err(error) => return Err(emit_error(&spec.id, &log_path, error, &mut emit)),
        };
        append_output(&mut log, &spec.probe, &output)?;
        if output.success {
            emit(InstallProgress::new(&spec.id, "done", &log_path));
            return Ok(log_path);
        }
    }

    Err(emit_error(
        &spec.id,
        &log_path,
        format!("verification timed out after {} seconds", spec.poll_seconds),
        &mut emit,
    ))
}

#[cfg(test)]
mod tests {
    use super::{run_install_with_interval, InstallProgress, InstallSpec};
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
    fn command(script: &Path, action: &str, state: &Path) -> Vec<String> {
        vec![
            script.to_string_lossy().into_owned(),
            action.to_owned(),
            state.to_string_lossy().into_owned(),
        ]
    }

    #[cfg(windows)]
    fn command(script: &Path, action: &str, state: &Path) -> Vec<String> {
        vec![
            "cmd".to_owned(),
            "/C".to_owned(),
            script.to_string_lossy().into_owned(),
            action.to_owned(),
            state.to_string_lossy().into_owned(),
        ]
    }

    fn spec(script: &Path, state: &Path, login_action: &str, poll_seconds: u64) -> InstallSpec {
        InstallSpec {
            id: "stub".to_owned(),
            install: command(script, "install", state),
            login: command(script, login_action, state),
            probe: command(script, "probe", state),
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

    #[cfg(unix)]
    #[tokio::test]
    async fn stub_happy_path_installs_logs_verifies_and_finishes() {
        let root = TestDir::new("happy");
        let state = root.0.join("ready");
        let script = write_stub(
            &root.0,
            r#"case "$1" in
  install) exit 0 ;;
  probe) [ -f "$2" ] ;;
  login) touch "$2" ;;
esac"#,
        );
        let (result, events) = run_case(&root.0, spec(&script, &state, "login", 1)).await;
        let log = result.unwrap();
        assert!(log.exists());
        let stages: Vec<&str> = events.iter().map(|event| event.stage).collect();
        assert_eq!(stages, ["detect", "install", "verify", "login", "verify", "done"]);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn stub_happy_path_installs_logs_verifies_and_finishes() {
        let root = TestDir::new("happy");
        let state = root.0.join("ready");
        let script = write_stub(
            &root.0,
            r#"if "%1"=="install" exit /b 0
if "%1"=="probe" if exist "%2" (exit /b 0) else (exit /b 1)
if "%1"=="login" (type nul > "%2"& exit /b 0)"#,
        );
        let (result, events) = run_case(&root.0, spec(&script, &state, "login", 1)).await;
        let log = result.unwrap();
        assert!(log.exists());
        assert_eq!(events.last().unwrap().stage, "done");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stub_install_nonzero_emits_error_and_keeps_log() {
        let root = TestDir::new("install-fail");
        let state = root.0.join("ready");
        let script = write_stub(
            &root.0,
            r#"case "$1" in
  install) echo install-failed >&2; exit 7 ;;
  *) exit 1 ;;
esac"#,
        );
        let (result, events) = run_case(&root.0, spec(&script, &state, "login", 1)).await;
        assert!(result.is_err());
        assert_eq!(
            events.iter().map(|event| event.stage).collect::<Vec<_>>(),
            ["detect", "install", "error"]
        );
        assert!(Path::new(events.last().unwrap().log_path.as_ref().unwrap()).exists());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn stub_install_nonzero_emits_error_and_keeps_log() {
        let root = TestDir::new("install-fail");
        let state = root.0.join("ready");
        let script = write_stub(
            &root.0,
            r#"if "%1"=="install" (echo install-failed 1>&2& exit /b 7)
exit /b 1"#,
        );
        let (result, events) = run_case(&root.0, spec(&script, &state, "login", 1)).await;
        assert!(result.is_err());
        assert_eq!(events.last().unwrap().stage, "error");
        assert!(Path::new(events.last().unwrap().log_path.as_ref().unwrap()).exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stub_probe_never_green_times_out_and_logs_error() {
        let root = TestDir::new("probe-timeout");
        let state = root.0.join("ready");
        let script = write_stub(
            &root.0,
            r#"case "$1" in
  install) exit 0 ;;
  probe) exit 1 ;;
  login) exit 0 ;;
esac"#,
        );
        let (result, events) = run_case(&root.0, spec(&script, &state, "login", 1)).await;
        assert!(result.is_err());
        assert!(
            events
                .iter()
                .filter(|event| event.stage == "verify")
                .count()
                > 1
        );
        assert_eq!(events.last().unwrap().stage, "error");
        assert!(Path::new(events.last().unwrap().log_path.as_ref().unwrap()).exists());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn stub_probe_never_green_times_out_and_logs_error() {
        let root = TestDir::new("probe-timeout");
        let state = root.0.join("ready");
        let script = write_stub(
            &root.0,
            r#"if "%1"=="install" exit /b 0
if "%1"=="probe" exit /b 1
if "%1"=="login" exit /b 0"#,
        );
        let (result, events) = run_case(&root.0, spec(&script, &state, "login", 1)).await;
        assert!(result.is_err());
        assert!(
            events
                .iter()
                .filter(|event| event.stage == "verify")
                .count()
                > 1
        );
        assert_eq!(events.last().unwrap().stage, "error");
        assert!(Path::new(events.last().unwrap().log_path.as_ref().unwrap()).exists());
    }

    #[cfg(windows)]
    async fn real_install_smoke(provider: &str) {
        let specs = super::windows_specs();
        assert!(
            specs.is_ok(),
            "{provider}: unable to load Windows install specs: {specs:?}"
        );
        let mut spec = match specs {
            Ok(specs) => match specs.into_iter().find(|spec| spec.id == provider) {
                Some(spec) => spec,
                None => panic!("{provider}: no matching Windows install spec"),
            },
            Err(_) => return,
        };
        spec.poll_seconds = 60;
        let probe_path = PathBuf::from(&spec.probe[0]);
        let data_root = std::env::temp_dir().join("tt-smoke");
        let created = std::fs::create_dir_all(&data_root);
        assert!(
            created.is_ok(),
            "{provider}: unable to create {data_root:?}: {created:?}"
        );

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
        assert!(
            events.iter().any(|event| event.stage == "login"),
            "{provider}: login stage was not emitted: {events:?}"
        );
        let last = match events.last() {
            Some(event) => event,
            None => panic!("{provider}: no install events were emitted"),
        };
        match result {
            Ok(_) => assert_eq!(
                last.stage, "done",
                "{provider}: successful run did not finish"
            ),
            Err(error) => {
                assert_eq!(
                    last.stage, "error",
                    "{provider}: failed run did not report error: {error}"
                );
                let log_path = match last.log_path.as_ref() {
                    Some(path) => Path::new(path),
                    None => panic!("{provider}: error event had no log path"),
                };
                assert!(log_path.exists(), "{provider}: missing log at {log_path:?}");
            }
        }
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
