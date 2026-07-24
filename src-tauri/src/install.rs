use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::AsyncReadExt;
use tokio::process::Child;
use tokio::process::Command;
use tokio::task::JoinHandle;

#[derive(Debug, Clone)]
pub enum LoginMode {
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    Terminal(Vec<String>),
    Headless {
        trigger: Option<Vec<String>>,
    },
}

#[derive(Debug, Clone)]
pub struct InstallSpec {
    pub id: String,
    pub install: Vec<String>,
    pub login: LoginMode,
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
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_path: Option<String>,
}

impl InstallProgress {
    fn new(provider: &str, stage: &'static str, log_path: &Path) -> Self {
        Self {
            provider: provider.to_owned(),
            stage,
            detail: None,
            url: None,
            log_path: Some(log_path.to_string_lossy().into_owned()),
        }
    }
}

struct CommandOutput {
    success: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

struct StreamingChild {
    child: Child,
    output: Arc<Mutex<Vec<u8>>>,
    readers: Vec<JoinHandle<()>>,
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
            install: argv(
                "powershell",
                &[
                    "-NoProfile",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-Command",
                    "irm https://claude.ai/install.ps1 | iex",
                ],
            ),
            login: LoginMode::Terminal(argv("cmd", &["/C", "start", "", claude.as_str()])),
            probe: argv(claude, &["-p", "ok"]),
            poll_seconds: 120,
        },
        InstallSpec {
            id: "codex".to_owned(),
            install: argv(
                "powershell",
                &[
                    "-NoProfile",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-Command",
                    "irm https://chatgpt.com/codex/install.ps1 | iex",
                ],
            ),
            login: LoginMode::Headless {
                trigger: Some(argv(codex.clone(), &["login"])),
            },
            probe: argv(codex, &["login", "status"]),
            poll_seconds: 120,
        },
        InstallSpec {
            id: "agy".to_owned(),
            install: argv(
                "powershell",
                &[
                    "-NoProfile",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-Command",
                    "irm https://antigravity.google/cli/install.ps1 | iex",
                ],
            ),
            login: LoginMode::Headless { trigger: None },
            probe: argv(agy, &["-p", "ok"]),
            poll_seconds: 600,
        },
        InstallSpec {
            id: "grok".to_owned(),
            install: argv(
                "powershell",
                &[
                    "-NoProfile",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-Command",
                    "irm https://x.ai/cli/install.ps1 | iex",
                ],
            ),
            login: LoginMode::Headless {
                trigger: Some(argv(grok.clone(), &["login"])),
            },
            probe: argv(grok, &["-p", "ok"]),
            poll_seconds: 120,
        },
    ])
}

pub fn extract_first_url(bytes: &[u8]) -> Option<String> {
    const PREFIX: &[u8] = b"https://";
    let start = bytes
        .windows(PREFIX.len())
        .position(|window| window == PREFIX)?;
    let end = bytes[start..]
        .iter()
        .position(|byte| !matches!(*byte, b'!'..=b'~') || matches!(*byte, b'\'' | b'"'))
        .map(|offset| start + offset)
        .unwrap_or(bytes.len());
    String::from_utf8(bytes[start..end].to_vec()).ok()
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
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    child.creation_flags(0x08000000);
    let output = child.output().await.map_err(|error| error.to_string())?;
    Ok(CommandOutput {
        success: output.status.success(),
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

async fn copy_stream<R>(mut stream: R, output: Arc<Mutex<Vec<u8>>>)
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut chunk = [0; 4096];
    loop {
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(count) => output.lock().unwrap().extend_from_slice(&chunk[..count]),
        }
    }
}

fn spawn_streaming(command: &[String]) -> Result<StreamingChild, String> {
    let (program, args) = command
        .split_first()
        .ok_or_else(|| "empty command argv".to_owned())?;
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    command.creation_flags(0x08000000);
    let mut child = command.spawn().map_err(|error| error.to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture command stdout".to_owned())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "failed to capture command stderr".to_owned())?;
    let output = Arc::new(Mutex::new(Vec::new()));
    let readers = vec![
        tokio::spawn(copy_stream(stdout, Arc::clone(&output))),
        tokio::spawn(copy_stream(stderr, Arc::clone(&output))),
    ];
    Ok(StreamingChild {
        child,
        output,
        readers,
    })
}

impl StreamingChild {
    async fn wait_for_readers(&mut self) {
        for reader in self.readers.drain(..) {
            let _ = reader.await;
        }
    }

    fn output(&self) -> Vec<u8> {
        self.output.lock().unwrap().clone()
    }

    async fn stop(mut self) -> CommandOutput {
        let _ = self.child.start_kill();
        let status = self.child.wait().await.ok();
        for reader in self.readers.drain(..) {
            reader.abort();
            let _ = reader.await;
        }
        CommandOutput {
            success: status.is_some_and(|status| status.success()),
            stdout: self.output(),
            stderr: Vec::new(),
        }
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
    open_url: impl FnMut(&str) -> Result<(), String>,
) -> Result<PathBuf, String> {
    run_install_with_interval(
        spec,
        data_root,
        detect,
        emit,
        open_url,
        Duration::from_secs(5),
    )
    .await
}

async fn run_install_with_interval(
    spec: InstallSpec,
    data_root: &Path,
    mut detect: impl FnMut(&str) -> Option<PathBuf>,
    mut emit: impl FnMut(InstallProgress),
    mut open_url: impl FnMut(&str) -> Result<(), String>,
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
    let initial_probe = match run_hidden(&spec.probe).await {
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
    let login_command = match &spec.login {
        LoginMode::Terminal(command) => command,
        LoginMode::Headless { trigger } => trigger.as_ref().unwrap_or(&spec.probe),
    };
    if let LoginMode::Terminal(_) = spec.login {
        let login_output = match run_terminal(login_command).await {
            Ok(output) => output,
            Err(error) => return Err(emit_error(&spec.id, &log_path, error, &mut emit)),
        };
        append_output(&mut log, login_command, &login_output)?;
    } else {
        let mut login = match spawn_streaming(login_command) {
            Ok(login) => login,
            Err(error) => return Err(emit_error(&spec.id, &log_path, error, &mut emit)),
        };
        let scan_timeout = Duration::from_secs(60);
        let scan_interval = poll_interval.min(Duration::from_millis(500));
        let mut scan_elapsed = Duration::ZERO;
        let mut exited = false;
        while scan_elapsed < scan_timeout {
            if let Some(url) = extract_first_url(&login.output()) {
                let login_output = CommandOutput {
                    success: true,
                    stdout: login.output(),
                    stderr: Vec::new(),
                };
                let mut progress = InstallProgress::new(&spec.id, "login", &log_path);
                progress.detail = output_detail(&login_output);
                progress.url = Some(url.clone());
                emit(progress);
                if let Err(error) = open_url(&url) {
                    let mut progress = InstallProgress::new(&spec.id, "login", &log_path);
                    progress.detail = Some(error);
                    progress.url = Some(url);
                    emit(progress);
                }
                break;
            }
            match login.child.try_wait() {
                Ok(Some(_)) => {
                    login.wait_for_readers().await;
                    exited = true;
                    break;
                }
                Ok(None) => {}
                Err(error) => {
                    let login_output = login.stop().await;
                    append_output(&mut log, login_command, &login_output)?;
                    return Err(emit_error(
                        &spec.id,
                        &log_path,
                        error.to_string(),
                        &mut emit,
                    ));
                }
            }
            let delay = scan_interval.min(scan_timeout - scan_elapsed);
            tokio::time::sleep(delay).await;
            scan_elapsed += delay;
        }
        if exited {
            if let Some(url) = extract_first_url(&login.output()) {
                let login_output = CommandOutput {
                    success: true,
                    stdout: login.output(),
                    stderr: Vec::new(),
                };
                let mut progress = InstallProgress::new(&spec.id, "login", &log_path);
                progress.detail = output_detail(&login_output);
                progress.url = Some(url.clone());
                emit(progress);
                if let Err(error) = open_url(&url) {
                    let mut progress = InstallProgress::new(&spec.id, "login", &log_path);
                    progress.detail = Some(error);
                    progress.url = Some(url);
                    emit(progress);
                }
            } else if !login.output().is_empty() {
                let mut progress = InstallProgress::new(&spec.id, "login", &log_path);
                progress.detail = Some(String::from_utf8_lossy(&login.output()).trim().to_owned());
                emit(progress);
            }
        }

        let timeout = Duration::from_secs(spec.poll_seconds);
        let mut elapsed = Duration::ZERO;
        let result = loop {
            if elapsed >= timeout {
                break Err(format!(
                    "verification timed out after {} seconds",
                    spec.poll_seconds
                ));
            }
            let delay = poll_interval.min(timeout - elapsed);
            tokio::time::sleep(delay).await;
            elapsed += delay;
            emit(InstallProgress::new(&spec.id, "verify", &log_path));
            let output = match run_hidden(&spec.probe).await {
                Ok(output) => output,
                Err(error) => break Err(error),
            };
            if let Err(error) = append_output(&mut log, &spec.probe, &output) {
                break Err(error);
            }
            if output.success {
                break Ok(());
            }
        };
        let login_output = login.stop().await;
        append_output(&mut log, login_command, &login_output)?;
        match result {
            Ok(()) => {
                emit(InstallProgress::new(&spec.id, "done", &log_path));
                return Ok(log_path);
            }
            Err(error) => return Err(emit_error(&spec.id, &log_path, error, &mut emit)),
        }
    }

    let timeout = Duration::from_secs(spec.poll_seconds);
    let mut elapsed = Duration::ZERO;
    while elapsed < timeout {
        let delay = poll_interval.min(timeout - elapsed);
        tokio::time::sleep(delay).await;
        elapsed += delay;
        emit(InstallProgress::new(&spec.id, "verify", &log_path));
        let output = match run_hidden(&spec.probe).await {
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
    use super::{
        extract_first_url, run_install_with_interval, InstallProgress, InstallSpec, LoginMode,
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
            login: LoginMode::Headless {
                trigger: Some(command(script, login_action, state)),
            },
            probe: command(script, "probe", state),
            poll_seconds,
        }
    }

    async fn run_case(
        root: &Path,
        spec: InstallSpec,
    ) -> (Result<PathBuf, String>, Vec<InstallProgress>, Vec<String>) {
        let mut events = Vec::new();
        let mut urls = Vec::new();
        let result = run_install_with_interval(
            spec,
            root,
            |_| None,
            |event| events.push(event),
            |url| {
                urls.push(url.to_owned());
                Ok(())
            },
            Duration::from_millis(10),
        )
        .await;
        (result, events, urls)
    }

    #[test]
    fn extracts_url_from_cp932_noise_without_decoding() {
        let bytes = b"\x82\xa0\x82\xa2 https://example.com/oauth?x=1 \x83\x65";
        assert_eq!(
            extract_first_url(bytes).as_deref(),
            Some("https://example.com/oauth?x=1")
        );
    }

    #[test]
    fn extracts_url_around_ansi_control_sequences() {
        let bytes = b"\x1b[31mhttps://example.com/login\x1b[0m";
        assert_eq!(
            extract_first_url(bytes).as_deref(),
            Some("https://example.com/login")
        );
    }

    #[test]
    fn url_stops_at_whitespace_quotes_controls_and_non_ascii() {
        for (suffix, expected) in [
            (&b" next"[..], "https://x.test/a"),
            (&b"\"next"[..], "https://x.test/a"),
            (&b"'next"[..], "https://x.test/a"),
            (&b"\rnext"[..], "https://x.test/a"),
            (&b"\x82next"[..], "https://x.test/a"),
        ] {
            let mut bytes = b"https://x.test/a".to_vec();
            bytes.extend_from_slice(suffix);
            assert_eq!(extract_first_url(&bytes).as_deref(), Some(expected));
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stub_happy_path_installs_logs_url_verifies_and_finishes() {
        let root = TestDir::new("happy");
        let state = root.0.join("ready");
        let script = write_stub(
            &root.0,
            r#"case "$1" in
  install) exit 0 ;;
  probe) [ -f "$2" ] ;;
  login) printf '\377noise https://example.com/oauth?code=ok\n'; touch "$2" ;;
esac"#,
        );
        let (result, events, urls) = run_case(&root.0, spec(&script, &state, "login", 1)).await;
        let log = result.unwrap();
        assert!(log.exists());
        assert_eq!(urls, ["https://example.com/oauth?code=ok"]);
        let stages: Vec<&str> = events.iter().map(|event| event.stage).collect();
        assert_eq!(
            stages,
            ["detect", "install", "verify", "login", "login", "verify", "done"]
        );
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn stub_happy_path_installs_logs_url_verifies_and_finishes() {
        let root = TestDir::new("happy");
        let state = root.0.join("ready");
        let script = write_stub(
            &root.0,
            r#"if "%1"=="install" exit /b 0
if "%1"=="probe" if exist "%2" (exit /b 0) else (exit /b 1)
if "%1"=="login" (echo https://example.com/oauth?code=ok& type nul > "%2"& exit /b 0)"#,
        );
        let (result, events, urls) = run_case(&root.0, spec(&script, &state, "login", 1)).await;
        let log = result.unwrap();
        assert!(log.exists());
        assert_eq!(urls, ["https://example.com/oauth?code=ok"]);
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
        let (result, events, _) = run_case(&root.0, spec(&script, &state, "login", 1)).await;
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
        let (result, events, _) = run_case(&root.0, spec(&script, &state, "login", 1)).await;
        assert!(result.is_err());
        assert_eq!(events.last().unwrap().stage, "error");
        assert!(Path::new(events.last().unwrap().log_path.as_ref().unwrap()).exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stub_login_without_url_still_polls_until_timeout_error() {
        let root = TestDir::new("no-url");
        let state = root.0.join("ready");
        let script = write_stub(
            &root.0,
            r#"case "$1" in
  install) exit 0 ;;
  probe) exit 1 ;;
  login) echo waiting-for-browser; exit 0 ;;
esac"#,
        );
        let (result, events, urls) = run_case(&root.0, spec(&script, &state, "login", 1)).await;
        assert!(result.is_err());
        assert!(urls.is_empty());
        assert!(
            events
                .iter()
                .filter(|event| event.stage == "verify")
                .count()
                > 1
        );
        assert_eq!(events.last().unwrap().stage, "error");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn stub_login_without_url_still_polls_until_timeout_error() {
        let root = TestDir::new("no-url");
        let state = root.0.join("ready");
        let script = write_stub(
            &root.0,
            r#"if "%1"=="install" exit /b 0
if "%1"=="probe" exit /b 1
if "%1"=="login" (echo waiting-for-browser& exit /b 0)"#,
        );
        let (result, events, urls) = run_case(&root.0, spec(&script, &state, "login", 1)).await;
        assert!(result.is_err());
        assert!(urls.is_empty());
        assert!(
            events
                .iter()
                .filter(|event| event.stage == "verify")
                .count()
                > 1
        );
        assert_eq!(events.last().unwrap().stage, "error");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stub_probe_never_green_times_out_after_url_and_logs_error() {
        let root = TestDir::new("probe-timeout");
        let state = root.0.join("ready");
        let script = write_stub(
            &root.0,
            r#"case "$1" in
  install) exit 0 ;;
  probe) exit 1 ;;
  login) echo https://example.com/oauth; exit 0 ;;
esac"#,
        );
        let (result, events, urls) = run_case(&root.0, spec(&script, &state, "login", 1)).await;
        assert!(result.is_err());
        assert_eq!(urls, ["https://example.com/oauth"]);
        assert_eq!(events.last().unwrap().stage, "error");
        assert!(Path::new(events.last().unwrap().log_path.as_ref().unwrap()).exists());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn stub_probe_never_green_times_out_after_url_and_logs_error() {
        let root = TestDir::new("probe-timeout");
        let state = root.0.join("ready");
        let script = write_stub(
            &root.0,
            r#"if "%1"=="install" exit /b 0
if "%1"=="probe" exit /b 1
if "%1"=="login" (echo https://example.com/oauth& exit /b 0)"#,
        );
        let (result, events, urls) = run_case(&root.0, spec(&script, &state, "login", 1)).await;
        assert!(result.is_err());
        assert_eq!(urls, ["https://example.com/oauth"]);
        assert_eq!(events.last().unwrap().stage, "error");
        assert!(Path::new(events.last().unwrap().log_path.as_ref().unwrap()).exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stub_login_streams_url_before_trigger_exits() {
        let root = TestDir::new("streaming-login");
        let state = root.0.join("ready");
        let script = write_stub(
            &root.0,
            r#"case "$1" in
  install) exit 0 ;;
  probe) [ -f "$2" ] ;;
  login) echo https://example.com/oauth; touch "$2"; sleep 300 ;;
esac"#,
        );
        let (result, events, urls) = run_case(&root.0, spec(&script, &state, "login", 5)).await;
        assert!(result.is_ok());
        assert_eq!(urls, ["https://example.com/oauth"]);
        assert_eq!(events.last().unwrap().stage, "done");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn stub_login_streams_url_before_trigger_exits() {
        let root = TestDir::new("streaming-login");
        let state = root.0.join("ready");
        let script = write_stub(
            &root.0,
            r#"if "%1"=="install" exit /b 0
if "%1"=="probe" if exist "%2" (exit /b 0) else (exit /b 1)
if "%1"=="login" (echo https://example.com/oauth& type nul > "%2"& ping -n 300 127.0.0.1 >nul)"#,
        );
        let (result, events, urls) = run_case(&root.0, spec(&script, &state, "login", 5)).await;
        assert!(result.is_ok());
        assert_eq!(urls, ["https://example.com/oauth"]);
        assert_eq!(events.last().unwrap().stage, "done");
    }

    #[cfg(windows)]
    async fn real_install_smoke(provider: &str, expects_url: bool) {
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
        let mut urls = Vec::new();
        let result = run_install_with_interval(
            spec,
            &data_root,
            |_| None,
            |event| events.push(event),
            |url| {
                urls.push(url.to_owned());
                Ok(())
            },
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
                if expects_url {
                    assert!(!urls.is_empty(), "{provider}: OAuth URL was not captured");
                }
            }
        }
    }

    #[cfg(windows)]
    #[tokio::test]
    #[ignore]
    async fn real_install_claude() {
        real_install_smoke("claude", false).await;
    }

    #[cfg(windows)]
    #[tokio::test]
    #[ignore]
    async fn real_install_codex() {
        real_install_smoke("codex", true).await;
    }

    #[cfg(windows)]
    #[tokio::test]
    #[ignore]
    async fn real_install_agy() {
        real_install_smoke("agy", true).await;
    }

    #[cfg(windows)]
    #[tokio::test]
    #[ignore]
    async fn real_install_grok() {
        real_install_smoke("grok", true).await;
    }
}
