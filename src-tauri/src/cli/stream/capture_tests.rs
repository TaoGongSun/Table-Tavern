//! 開發者用的夾具擷取（檔名帶 _tests：只在測試建置裡編譯，spawn 掃描也不算它）（plans/cli-version-gate.md 包 1 第 6 點）：用 app 自己的參數組裝函式真的跑 CLI，
//! 把輸出（去識別化）、當次參數（`.args.json`）與 `--version`（`.version`）寫進 `fixtures/`。
//! 只在手動指定時跑，而且一律跑執行檔副本、帶關閉自動更新的設定與測試 HOME：
//!
//! ```text
//! CVG_ROOT=<暫存根> CVG_GROK_BIN=<副本> CVG_GROK_HOME=<已登入的測試 GROK_HOME> \
//!   CVG_GROK_FAKE_HOME=<測試 HOME> CVG_AGY_BIN=<副本> CVG_AGY_HOME=<含 keychain 連結的測試 HOME> \
//!   CVG_CODEX_BIN=<副本> CVG_CODEX_HOME=<含 auth 連結的測試 CODEX_HOME> CVG_CLAUDE_BIN=<副本> \
//!   cargo test --lib capture_cli_fixtures -- --ignored --nocapture
//! ```
//!
//! 只擷取有給 `CVG_<CLI>_BIN` 的那幾家；`CVG_SCENARIOS` 可用逗號限定情境。claude 沿用使用者
//! 自己的登入（HOME 不換），靠 `DISABLE_AUTOUPDATER=1` 與副本保護真執行檔。

use super::super::agy_home;
use super::super::request::*;
use super::super::types::CliSession;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const SYSTEM: &str = "You are a terse assistant in a test harness.";
const OK_PROMPT: &str = "Reply with exactly: OK";
const THINK_PROMPT: &str =
    "Think carefully and check your work before answering: how many prime numbers are there between 100 and 200? Reply with only the number.";
const OPEN_PROMPT: &str = "Remember the word lantern. Reply with exactly: OK";
const RESUME_PROMPT: &str = "What word did I ask you to remember? Reply with just that word.";
/// 生圖情境：app 實際會要模型回圖檔路徑，錄音改回固定字樣，免得串流把路徑切成好幾段、逐行去識別化擋不住。
const IMAGE_PROMPT: &str = "A small red apple on a white table.\nIf you are able to generate images, generate it now and save it as a PNG file, then reply with exactly: IMAGE_SAVED. If you cannot generate images at all, reply exactly: NO_IMAGE. If you decline this particular request, reply exactly: REFUSED";
pub(super) const BAD_MODEL: &str = "no-such-model-xyz";

/// 參數裡的路徑與 id 一律換成這些佔位字，契約測試用同樣的佔位字重組參數比對。
pub(super) const PROMPT_FILE: &str = "<PROMPT_FILE>";
pub(super) const SYSTEM_FILE: &str = "<SYSTEM_FILE>";
pub(super) const PROFILE: &str = "<PROFILE>";
pub(super) const SESSION_ID: &str = "<SESSION_ID>";

pub(super) fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/cli/stream/fixtures")
}

/// 一家一個子目錄：`fixtures/<cli>/<版本>-<情境>.*`、`fixtures/<cli>/<版本>.version`。
fn cli_dir(cli: &str) -> PathBuf {
    let dir = fixtures_dir().join(cli);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Run<'a> {
    cli: &'static str,
    scenario: &'a str,
    /// 給 `.args.json` 記的組裝輸入（模型、effort 等），契約測試據此重組
    inputs: Value,
    args: Vec<String>,
    stdin: String,
    /// 參數裡真正的路徑／id → 佔位字
    placeholders: Vec<(String, &'static str)>,
}

struct Env {
    root: PathBuf,
    workspace: PathBuf,
    bin: PathBuf,
    envs: Vec<(String, String)>,
    /// 真 id → 假 id：同一家各情境共用，開線與續聊的關聯留著
    ids: std::cell::RefCell<std::collections::HashMap<String, String>>,
}

/// 錄音裡的長字串（例如工具讀進來的第三方 SKILL.md 全文）截到這麼長
const MAX_STRING: usize = 2000;

/// UUID 形狀的 id（session、conversation、thread、request、訊息 uuid）換成依出現順序編號的假值。
fn fake_ids(text: &str, env: &Env) -> String {
    let pattern = regex::Regex::new(
        r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}",
    )
    .unwrap();
    let mut ids = env.ids.borrow_mut();
    pattern
        .replace_all(text, |found: &regex::Captures<'_>| {
            let next = ids.len() + 1;
            ids.entry(found[0].to_lowercase())
                .or_insert_with(|| format!("00000000-0000-4000-8000-{next:012}"))
                .clone()
        })
        .into_owned()
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key).map(PathBuf::from)
}

fn wanted(scenario: &str) -> bool {
    match std::env::var("CVG_SCENARIOS") {
        Ok(list) => list.split(',').any(|item| item.trim() == scenario),
        Err(_) => true,
    }
}

fn run_once(env: &Env, args: &[String], stdin: &str) -> (String, String, Option<i32>) {
    let mut command = Command::new(&env.bin);
    command
        .args(args)
        .current_dir(&env.workspace)
        .envs(env.envs.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("ANTHROPIC_") {
            command.env_remove(&key);
        }
    }
    let mut child = command.spawn().expect("spawn CLI copy");
    {
        use std::io::Write;
        let mut pipe = child.stdin.take().unwrap();
        let _ = pipe.write_all(stdin.as_bytes());
    }
    let output = child.wait_with_output().expect("wait CLI copy");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code(),
    )
}

/// 去識別化：暫存根（含它的上一層，測試 HOME 都在那裡）與家目錄換成佔位字，路徑的幾種寫法都換
/// （原樣、去掉 macOS 的 `/private`、URL 編碼、claude 專案目錄的 `-` 寫法）；claude init 裡使用者自己的
/// 外掛、技能、指令清單清空；簽章遮掉；其餘系統暫存路徑整段收成 `<TMP>/最後一段`。
fn sanitize(line: &str, env: &Env) -> String {
    let mut text = line.to_owned();
    let scratch = env
        .root
        .parent()
        .unwrap_or(&env.root)
        .to_string_lossy()
        .into_owned();
    let home = std::env::var("HOME").unwrap_or_default();
    for (path, token) in [(scratch, "<ROOT>"), (home, "<HOME>")] {
        if path.len() < 2 {
            continue;
        }
        for variant in path_variants(&path) {
            text = text.replace(&variant, token);
        }
    }
    if let Ok(user) = std::env::var("USER") {
        if user.len() >= 3 {
            text = text.replace(&user, "<USER>");
        }
    }
    let text = collapse_tmp_paths(&fake_ids(&text, env));
    let Ok(mut value) = serde_json::from_str::<Value>(&text) else {
        return text;
    };
    if value.get("type").and_then(Value::as_str) == Some("system") {
        for key in [
            "slash_commands",
            "terminal_slash_commands",
            "skills",
            "plugins",
            "agents",
            "mcp_servers",
        ] {
            if let Some(field) = value.get_mut(key) {
                *field = json!([]);
            }
        }
    }
    redact(&mut value);
    value.to_string()
}

/// `/private/tmp/…`、`/tmp/…`、`/var/folders/…` 開頭的整段路徑只留最後一段（帶出 uid 與本機目錄結構）；
/// 路徑前面可以是 JSON 跳脫的換行。
pub(super) fn collapse_tmp_paths(text: &str) -> String {
    static PATTERN: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(
            r#"(^|\\[nrt]|[^A-Za-z0-9._>/-])(?:/private)?/(?:tmp|var/folders)/[^"\s\\']*"#,
        )
        .unwrap()
    });
    PATTERN
        .replace_all(text, |caps: &regex::Captures| {
            let path = &caps[0][caps[1].len()..];
            let tail = path.rsplit('/').next().unwrap_or_default();
            format!("{}<TMP>/{tail}", &caps[1])
        })
        .into_owned()
}

pub(super) fn path_variants(path: &str) -> Vec<String> {
    let mut bases = vec![path.to_owned()];
    if let Some(stripped) = path.strip_prefix("/private") {
        bases.push(stripped.to_owned());
    }
    let mut out = Vec::new();
    for base in bases {
        out.push(base.replace('/', "%2F"));
        out.push(base.replace('/', "-"));
        out.push(base);
    }
    // 長的先換，免得短的先吃掉一部分
    out.sort_by_key(|variant| std::cmp::Reverse(variant.len()));
    out
}

fn redact(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                if matches!(key.as_str(), "signature" | "encrypted_content") && child.is_string() {
                    *child = json!("<redacted>");
                } else {
                    redact(child);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(redact),
        Value::String(text) if text.chars().count() > MAX_STRING => {
            let total = text.chars().count();
            let head: String = text.chars().take(200).collect();
            *text = format!("{head}…<truncated {total} chars>");
        }
        _ => {}
    }
}

fn version(env: &Env) -> String {
    let (stdout, _, _) = run_once(env, &["--version".to_owned()], "");
    stdout.lines().next().unwrap_or_default().trim().to_owned()
}

fn file_version(version: &str) -> String {
    version
        .split_whitespace()
        .find(|part| part.chars().next().is_some_and(|ch| ch.is_ascii_digit()))
        .unwrap_or("unknown")
        .to_owned()
}

/// 跑一個情境並落檔；回 stdout 原文給續聊情境抓 id。
fn capture(env: &Env, version_line: &str, run: Run<'_>) -> String {
    let (stdout, stderr, code) = run_once(env, &run.args, &run.stdin);
    let stem = format!("{}-{}", file_version(version_line), run.scenario);
    let out = cli_dir(run.cli);
    let body: String = stdout
        .lines()
        .map(|line| sanitize(line, env) + "\n")
        .collect();
    std::fs::write(out.join(format!("{stem}.jsonl")), body).unwrap();
    let mut args = run.args.clone();
    for arg in args.iter_mut() {
        for (real, placeholder) in &run.placeholders {
            if arg == real {
                *arg = (*placeholder).to_owned();
            }
        }
    }
    let record = json!({
        "cli": run.cli,
        "scenario": run.scenario,
        "inputs": run.inputs,
        "args": args,
        "exit": code,
        "stderr_tail": sanitize_tail(&stderr, env),
    });
    std::fs::write(
        out.join(format!("{stem}.args.json")),
        serde_json::to_string_pretty(&record).unwrap() + "\n",
    )
    .unwrap();
    eprintln!("captured {} {stem} (exit {code:?})", run.cli);
    stdout
}

fn sanitize_tail(stderr: &str, env: &Env) -> String {
    let tail: Vec<&str> = stderr.lines().rev().take(5).collect();
    let tail: Vec<&str> = tail.into_iter().rev().collect();
    sanitize(&tail.join("\n"), env)
}

fn write_version(cli: &str, version_line: &str) {
    std::fs::write(
        cli_dir(cli).join(format!("{}.version", file_version(version_line))),
        format!("{version_line}\n"),
    )
    .unwrap();
}

fn first_string(stdout: &str, pointers: &[&str]) -> Option<String> {
    stdout.lines().find_map(|line| {
        let value: Value = serde_json::from_str(line).ok()?;
        pointers
            .iter()
            .find_map(|pointer| value.pointer(pointer)?.as_str().map(str::to_owned))
            .filter(|id| !id.is_empty())
    })
}

fn root() -> PathBuf {
    let root = env_path("CVG_ROOT").expect("CVG_ROOT");
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn new_env(root: &Path, cli: &str, bin: PathBuf, envs: Vec<(String, String)>) -> Env {
    let workspace = root.join(format!("ws-{cli}"));
    std::fs::create_dir_all(&workspace).unwrap();
    Env {
        root: root.to_path_buf(),
        workspace,
        bin,
        envs,
        ids: Default::default(),
    }
}

fn capture_claude(root: &Path) {
    let Some(bin) = env_path("CVG_CLAUDE_BIN") else {
        return;
    };
    let env = new_env(
        root,
        "claude",
        bin,
        vec![("DISABLE_AUTOUPDATER".into(), "1".into())],
    );
    let ver = version(&env);
    write_version("claude", &ver);
    let system_file = root.join("claude-system.txt");
    std::fs::write(&system_file, SYSTEM).unwrap();
    let sys = system_file.to_string_lossy().into_owned();
    let ph = || vec![(sys.clone(), SYSTEM_FILE)];
    for (scenario, model, prompt) in [
        ("oneshot", "haiku", OK_PROMPT),
        ("think", "opus", THINK_PROMPT),
        ("badmodel", BAD_MODEL, OK_PROMPT),
    ] {
        if wanted(scenario) {
            capture(
                &env,
                &ver,
                Run {
                    cli: "claude",
                    scenario,
                    inputs: json!({ "model": model }),
                    args: claude_args(model, &system_file),
                    stdin: prompt.to_owned(),
                    placeholders: ph(),
                },
            );
        }
    }
    if wanted("lane") {
        let id = crate::lanes::new_session_id();
        let mut placeholders = ph();
        placeholders.push((id.clone(), SESSION_ID));
        for (scenario, session, prompt) in [
            ("lane-open", CliSession::Open(&id), OPEN_PROMPT),
            ("lane-resume", CliSession::Resume(&id), RESUME_PROMPT),
        ] {
            capture(
                &env,
                &ver,
                Run {
                    cli: "claude",
                    scenario,
                    inputs: json!({ "model": "haiku" }),
                    args: claude_session_args("haiku", &system_file, &session),
                    stdin: prompt.to_owned(),
                    placeholders: placeholders.clone(),
                },
            );
        }
    }
}

fn capture_codex(root: &Path) {
    let Some(bin) = env_path("CVG_CODEX_BIN") else {
        return;
    };
    let home = env_path("CVG_CODEX_HOME").expect("CVG_CODEX_HOME");
    let env = new_env(
        root,
        "codex",
        bin,
        super::super::codex_home::codex_envs(&home),
    );
    let ver = version(&env);
    write_version("codex", &ver);
    let body = |prompt: &str| format!("{SYSTEM}\n\n{prompt}");
    for (scenario, model, effort, tools, prompt) in [
        ("oneshot", None, "low", false, OK_PROMPT.to_owned()),
        ("think", None, "high", false, THINK_PROMPT.to_owned()),
        (
            "image",
            None,
            "medium",
            true,
            format!("$imagegen {IMAGE_PROMPT}"),
        ),
        (
            "badmodel",
            Some(BAD_MODEL),
            "low",
            false,
            OK_PROMPT.to_owned(),
        ),
    ] {
        if wanted(scenario) {
            capture(
                &env,
                &ver,
                Run {
                    cli: "codex",
                    scenario,
                    inputs: json!({ "model": model, "effort": effort, "tools": tools }),
                    args: codex_args(model, effort, tools),
                    stdin: body(&prompt),
                    placeholders: Vec::new(),
                },
            );
        }
    }
    if wanted("lane") {
        let opened = capture(
            &env,
            &ver,
            Run {
                cli: "codex",
                scenario: "lane-open",
                inputs: json!({ "model": null, "effort": "low" }),
                args: codex_session_args(None, "low", None),
                stdin: body(OPEN_PROMPT),
                placeholders: Vec::new(),
            },
        );
        let thread = first_string(&opened, &["/thread_id"]).expect("codex thread id");
        capture(
            &env,
            &ver,
            Run {
                cli: "codex",
                scenario: "lane-resume",
                inputs: json!({ "model": null, "effort": "low" }),
                args: codex_session_args(None, "low", Some(&thread)),
                stdin: RESUME_PROMPT.to_owned(),
                placeholders: vec![(thread.clone(), SESSION_ID)],
            },
        );
    }
}

fn capture_agy(root: &Path) {
    let Some(bin) = env_path("CVG_AGY_BIN") else {
        return;
    };
    let home = env_path("CVG_AGY_HOME").expect("CVG_AGY_HOME");
    agy_home::ensure_text_agent(&home).unwrap();
    let env = new_env(root, "agy", bin, agy_home::agy_envs(&home));
    let ver = version(&env);
    write_version("agy", &ver);
    let model = "gemini-3.8-flash-medium";
    for (scenario, model, tools, prompt) in [
        ("oneshot", model, false, OK_PROMPT),
        ("think", model, false, THINK_PROMPT),
        ("image", model, true, IMAGE_PROMPT),
        ("badmodel", BAD_MODEL, false, OK_PROMPT),
    ] {
        if wanted(scenario) {
            capture(
                &env,
                &ver,
                Run {
                    cli: "agy",
                    scenario,
                    inputs: json!({ "model": model, "tools": tools }),
                    args: agy_args(Some(model), tools),
                    stdin: agy_body(SYSTEM, prompt),
                    placeholders: Vec::new(),
                },
            );
        }
    }
    if wanted("lane") {
        let opened = capture(
            &env,
            &ver,
            Run {
                cli: "agy",
                scenario: "lane-open",
                inputs: json!({ "model": model }),
                args: agy_session_args(Some(model), None),
                stdin: agy_session_body(SYSTEM, OPEN_PROMPT, None),
                placeholders: Vec::new(),
            },
        );
        let id = first_string(&opened, &["/conversation_id", "/result/conversation_id"])
            .expect("agy conversation id");
        capture(
            &env,
            &ver,
            Run {
                cli: "agy",
                scenario: "lane-resume",
                inputs: json!({ "model": model }),
                args: agy_session_args(Some(model), Some(&id)),
                stdin: agy_session_body(SYSTEM, RESUME_PROMPT, Some(&id)),
                placeholders: vec![(id.clone(), SESSION_ID)],
            },
        );
    }
    if wanted("models") {
        capture_models(&env, "agy", &ver);
    }
}

fn capture_grok(root: &Path) {
    let Some(bin) = env_path("CVG_GROK_BIN") else {
        return;
    };
    let grok_home = env_path("CVG_GROK_HOME").expect("CVG_GROK_HOME");
    let fake_home = env_path("CVG_GROK_FAKE_HOME").expect("CVG_GROK_FAKE_HOME");
    let mut envs = grok_envs(&fake_home, &grok_home);
    envs.push(("GROK_DISABLE_AUTOUPDATER".into(), "1".into()));
    let env = new_env(root, "grok", bin, envs);
    let ver = version(&env);
    write_version("grok", &ver);
    let profile_path = root.join("grok-system.md");
    let prompt_path = root.join("grok-prompt.txt");
    let profile = profile_path.to_string_lossy().into_owned();
    let prompt_file = prompt_path.to_string_lossy().into_owned();
    let ph = || {
        vec![
            (profile.clone(), PROFILE),
            (prompt_file.clone(), PROMPT_FILE),
        ]
    };
    for (scenario, model, tools, prompt) in [
        ("oneshot", None, false, OK_PROMPT),
        ("think", None, false, THINK_PROMPT),
        ("image", None, true, IMAGE_PROMPT),
        ("badmodel", Some(BAD_MODEL), false, OK_PROMPT),
    ] {
        if !wanted(scenario) {
            continue;
        }
        let (system, body) = grok_payload(SYSTEM, prompt, tools);
        if let Some(system) = &system {
            std::fs::write(&profile_path, system).unwrap();
        }
        std::fs::write(&prompt_path, body).unwrap();
        let mut args = grok_args(
            model,
            system.as_ref().map(|_| profile_path.as_path()),
            &prompt_path,
            tools,
        );
        let mut placeholders = ph();
        if tools {
            // 生圖由 app 指定 session id（transport/dispatch.rs）
            let id = crate::lanes::new_session_id();
            args.splice(0..0, ["-s".to_owned(), id.clone()]);
            placeholders.push((id, SESSION_ID));
        }
        capture(
            &env,
            &ver,
            Run {
                cli: "grok",
                scenario,
                inputs: json!({ "model": model, "tools": tools }),
                args,
                stdin: String::new(),
                placeholders,
            },
        );
    }
    if wanted("lane") {
        let id = crate::lanes::new_session_id();
        let mut placeholders = ph();
        placeholders.push((id.clone(), SESSION_ID));
        let (system, body) = grok_payload(SYSTEM, OPEN_PROMPT, false);
        std::fs::write(&profile_path, system.unwrap()).unwrap();
        std::fs::write(&prompt_path, body).unwrap();
        capture(
            &env,
            &ver,
            Run {
                cli: "grok",
                scenario: "lane-open",
                inputs: json!({ "model": null }),
                args: grok_session_args(
                    None,
                    Some(&profile_path),
                    &prompt_path,
                    &CliSession::Open(&id),
                ),
                stdin: String::new(),
                placeholders: placeholders.clone(),
            },
        );
        std::fs::write(&prompt_path, RESUME_PROMPT).unwrap();
        capture(
            &env,
            &ver,
            Run {
                cli: "grok",
                scenario: "lane-resume",
                inputs: json!({ "model": null }),
                args: grok_session_args(
                    None,
                    Some(&profile_path),
                    &prompt_path,
                    &CliSession::Resume(&id),
                ),
                stdin: String::new(),
                placeholders,
            },
        );
    }
    if wanted("models") {
        capture_models(&env, "grok", &ver);
    }
}

/// `models` 不是串流：輸出存成 `.txt`，參數照樣記。
fn capture_models(env: &Env, cli: &'static str, version_line: &str) {
    let args = vec!["models".to_owned()];
    let (stdout, _, code) = run_once(env, &args, "");
    let stem = format!("{}-models", file_version(version_line));
    std::fs::write(
        cli_dir(cli).join(format!("{stem}.txt")),
        sanitize(&stdout, env),
    )
    .unwrap();
    let record =
        json!({ "cli": cli, "scenario": "models", "inputs": {}, "args": args, "exit": code });
    std::fs::write(
        cli_dir(cli).join(format!("{stem}.args.json")),
        serde_json::to_string_pretty(&record).unwrap() + "\n",
    )
    .unwrap();
}

#[test]
#[ignore = "手動擷取：會真的呼叫 CLI，見檔頭"]
fn capture_cli_fixtures() {
    let root = root();
    std::fs::create_dir_all(fixtures_dir()).unwrap();
    capture_claude(&root);
    capture_codex(&root);
    capture_agy(&root);
    capture_grok(&root);
}

#[test]
fn tmp_paths_collapse_to_their_last_segment() {
    let line = r#"{"a":"/private/tmp/x-501/y/scratchpad","b":"/tmp/s/1.sock","c":"see /var/folders/ab/T/f.txt now","d":"<ROOT>/tmp/keep","e":"/private/tmp/dir/","f":"exit: 0\n/tmp/a/b.png"}"#;
    assert_eq!(
        collapse_tmp_paths(line),
        r#"{"a":"<TMP>/scratchpad","b":"<TMP>/1.sock","c":"see <TMP>/f.txt now","d":"<ROOT>/tmp/keep","e":"<TMP>/","f":"exit: 0\n<TMP>/b.png"}"#
    );
}
