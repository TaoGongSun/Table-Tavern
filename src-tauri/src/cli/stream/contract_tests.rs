//! 夾具契約測試（plans/cli-version-gate.md 包 1 第 6 點）：`fixtures/` 是各家 CLI 真實輸出的錄音
//! （`capture_tests.rs` 擷取）。逐檔斷言解析得出正文、收尾、用量與下游欄位，每一行都認得；
//! 並用同樣的佔位字重組參數，與錄音當時的參數逐字比對——app 改了旗標卻沒重錄，就紅燈。

use super::super::catalog::{parse_agy_catalog, parse_grok_catalog};
use super::super::request::*;
use super::super::types::{CliLine, CliProtocol, CliSession};
use super::capture_tests::{fixtures_dir, PROFILE, PROMPT_FILE, SESSION_ID, SYSTEM_FILE};
use super::*;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// 一份錄音：`<cli>-<版本>-<情境>`。
struct Fixture {
    cli: String,
    version: String,
    scenario: String,
    record: Value,
    body: String,
}

fn version_key(version: &str) -> Vec<u64> {
    version
        .split(|ch: char| !ch.is_ascii_digit())
        .filter_map(|part| part.parse().ok())
        .collect()
}

fn load() -> Vec<Fixture> {
    let mut out = Vec::new();
    for dir in std::fs::read_dir(fixtures_dir()).unwrap() {
        let dir = dir.unwrap().path();
        if !dir.is_dir() {
            continue;
        }
        let cli = dir.file_name().unwrap().to_string_lossy().into_owned();
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let Some(stem) = name.strip_suffix(".args.json") else {
                continue;
            };
            let (version, scenario) = stem.split_once('-').unwrap();
            let record: Value =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            let ext = if scenario == "models" { "txt" } else { "jsonl" };
            let body = std::fs::read_to_string(dir.join(format!("{stem}.{ext}")))
                .unwrap_or_else(|_| panic!("{cli}/{stem}.{ext} 不見了"));
            out.push(Fixture {
                cli: cli.clone(),
                version: version.to_owned(),
                scenario: scenario.to_owned(),
                record,
                body,
            });
        }
    }
    assert!(!out.is_empty(), "fixtures/ 是空的");
    out
}

/// 每家最新的那一版：參數比對只看它（舊版錄音留著當解析回歸）。
fn newest(fixtures: &[Fixture]) -> BTreeMap<String, String> {
    let mut newest: BTreeMap<String, String> = BTreeMap::new();
    for fixture in fixtures {
        let entry = newest.entry(fixture.cli.clone()).or_default();
        if version_key(&fixture.version) > version_key(entry) {
            *entry = fixture.version.clone();
        }
    }
    newest
}

fn protocol(cli: &str) -> &'static CliProtocol {
    match cli {
        "claude" => &CLAUDE_PROTOCOL,
        "codex" => &CODEX_PROTOCOL,
        "agy" => &AGY_PROTOCOL,
        "grok" => &GROK_PROTOCOL,
        other => panic!("不認得的 CLI：{other}"),
    }
}

fn input<'a>(fixture: &'a Fixture, key: &str) -> Option<&'a str> {
    fixture.record.pointer(&format!("/inputs/{key}"))?.as_str()
}

fn tools(fixture: &Fixture) -> bool {
    fixture
        .record
        .pointer("/inputs/tools")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// 用佔位字重組現行參數組裝函式的輸出。
fn rebuild(fixture: &Fixture) -> Vec<String> {
    let model = input(fixture, "model");
    let effort = input(fixture, "effort").unwrap_or("low");
    let system = Path::new(SYSTEM_FILE);
    let profile = Path::new(PROFILE);
    let prompt = Path::new(PROMPT_FILE);
    let open = CliSession::Open(SESSION_ID);
    let resume = CliSession::Resume(SESSION_ID);
    match (fixture.cli.as_str(), fixture.scenario.as_str()) {
        (_, "models") => vec!["models".to_owned()],
        ("claude", "lane-open") => claude_session_args(model.unwrap(), system, &open),
        ("claude", "lane-resume") => claude_session_args(model.unwrap(), system, &resume),
        ("claude", _) => claude_args(model.unwrap(), system),
        ("codex", "lane-open") => codex_session_args(model, effort, None),
        ("codex", "lane-resume") => codex_session_args(model, effort, Some(SESSION_ID)),
        ("codex", _) => codex_args(model, effort, tools(fixture)),
        ("agy", "lane-open") => agy_session_args(model, None),
        ("agy", "lane-resume") => agy_session_args(model, Some(SESSION_ID)),
        ("agy", _) => agy_args(model, tools(fixture)),
        ("grok", "lane-open") => grok_session_args(model, Some(profile), prompt, &open),
        ("grok", "lane-resume") => grok_session_args(model, Some(profile), prompt, &resume),
        ("grok", "image") => {
            let mut args = grok_args(model, None, prompt, true);
            args.splice(0..0, ["-s".to_owned(), SESSION_ID.to_owned()]);
            args
        }
        ("grok", _) => grok_args(model, Some(profile), prompt, false),
        (cli, scenario) => panic!("沒有 {cli} {scenario} 的重組規則"),
    }
}

#[test]
fn current_args_match_the_newest_recordings() {
    let fixtures = load();
    let newest = newest(&fixtures);
    for fixture in fixtures.iter().filter(|f| newest[&f.cli] == f.version) {
        let recorded: Vec<String> = serde_json::from_value(fixture.record["args"].clone()).unwrap();
        assert_eq!(
            rebuild(fixture),
            recorded,
            "{}-{}-{}：參數和錄音時不同，旗標改了就要重錄夾具（capture_tests.rs）",
            fixture.cli,
            fixture.version,
            fixture.scenario
        );
    }
    // 每家每版都要有 .version，內容的版本號與檔名一致
    for (cli, version) in &newest {
        let line =
            std::fs::read_to_string(fixtures_dir().join(cli).join(format!("{version}.version")))
                .unwrap_or_else(|_| panic!("{cli}/{version}.version 不見了"));
        assert!(line.contains(version.as_str()), "{cli}：{line}");
    }
}

/// 一份錄音解析後的結果。
#[derive(Default)]
struct Parsed {
    text: String,
    thinking: bool,
    done: Option<(String, bool)>,
    unrecognized: Vec<String>,
}

fn replay(fixture: &Fixture) -> Parsed {
    let protocol = protocol(&fixture.cli);
    let mut parsed = Parsed::default();
    for line in fixture.body.lines() {
        if let Some(label) = (protocol.unrecognized)(line) {
            parsed.unrecognized.push(label);
        }
        match (protocol.parse)(line) {
            CliLine::Delta(text) => parsed.text.push_str(&text),
            CliLine::Thinking(_) => parsed.thinking = true,
            CliLine::Done { text, is_error } => {
                if !(protocol.sticky_failed_done && matches!(parsed.done, Some((_, true)))) {
                    parsed.done = Some((text, is_error));
                }
            }
            CliLine::SoftError(_) | CliLine::Other => {}
        }
    }
    parsed
}

fn usage_of(fixture: &Fixture) -> Option<crate::transport::PromptCacheUsage> {
    let parse = match fixture.cli.as_str() {
        "claude" => parse_claude_usage,
        "codex" => parse_codex_usage,
        "agy" => parse_agy_usage,
        _ => parse_grok_usage,
    };
    fixture.body.lines().find_map(parse)
}

#[test]
fn every_recorded_line_is_recognized() {
    for fixture in load().iter().filter(|f| f.scenario != "models") {
        let parsed = replay(fixture);
        assert!(
            parsed.unrecognized.is_empty(),
            "{}-{}-{}：認不得 {:?}（新型別要加進 stream/known.rs）",
            fixture.cli,
            fixture.version,
            fixture.scenario,
            parsed.unrecognized
        );
    }
}

#[test]
fn successful_recordings_yield_text_finish_usage_and_downstream_fields() {
    for fixture in load()
        .iter()
        .filter(|f| !matches!(f.scenario.as_str(), "models" | "badmodel"))
    {
        let label = format!("{}-{}-{}", fixture.cli, fixture.version, fixture.scenario);
        let parsed = replay(fixture);
        let (done_text, failed) = parsed
            .done
            .clone()
            .unwrap_or_else(|| panic!("{label}：沒收尾"));
        assert!(!failed, "{label}：收尾報失敗 {done_text}");
        let reply = match parsed.text.is_empty() {
            true => done_text,
            false => parsed.text.clone(),
        };
        assert!(!reply.trim().is_empty(), "{label}：沒有正文");
        if fixture.scenario == "lane-resume" {
            assert!(
                reply.to_lowercase().contains("lantern"),
                "{label}：續聊沒記得：{reply}"
            );
        }
        let usage = usage_of(fixture).unwrap_or_else(|| panic!("{label}：用量解析不出"));
        assert!(usage.prompt_tokens > 0, "{label}：輸入 token 0");
        assert!(usage.output_tokens > 0, "{label}：輸出 token 0");
        let lines = || fixture.body.lines();
        match fixture.cli.as_str() {
            "claude" => {
                let model = input(fixture, "model").unwrap();
                let identity =
                    lines().find_map(|line| crate::scene_budget::claude_model_usage(line, model));
                let (_, context, _) =
                    identity.unwrap_or_else(|| panic!("{label}：modelUsage 對不上 {model}"));
                assert!(
                    context.is_some_and(|c| c > 0),
                    "{label}：沒有 contextWindow"
                );
                assert!(usage.cost_usd.is_some(), "{label}：沒有 total_cost_usd");
                if fixture.scenario == "think" {
                    assert!(parsed.thinking, "{label}：沒有思考增量");
                }
            }
            "codex" => {
                assert!(
                    lines().find_map(codex_thread_id).is_some(),
                    "{label}：沒有 thread_id"
                );
            }
            "agy" => {
                assert!(
                    lines().find_map(agy_conversation_id).is_some(),
                    "{label}：沒有 conversation_id"
                );
            }
            _ => {
                let stop = lines().find_map(|line| {
                    let value: Value = serde_json::from_str(line).ok()?;
                    (value["type"] == "end")
                        .then(|| value["stopReason"].as_str().map(str::to_owned))?
                });
                assert_eq!(stop.as_deref(), Some("end_turn"), "{label}：stopReason");
            }
        }
    }
}

#[test]
fn bad_model_recordings_never_look_successful() {
    for fixture in load().iter().filter(|f| f.scenario == "badmodel") {
        let label = format!("{}-{}", fixture.cli, fixture.version);
        let parsed = replay(fixture);
        assert!(
            matches!(parsed.done, Some((_, true))),
            "{label}：模型叫不到要解析成失敗收尾"
        );
        assert_ne!(fixture.record["exit"], 0, "{label}：退出碼應非 0");
    }
}

#[test]
fn model_listings_still_parse() {
    for fixture in load().iter().filter(|f| f.scenario == "models") {
        let options = match fixture.cli.as_str() {
            "agy" => parse_agy_catalog(&fixture.body),
            _ => parse_grok_catalog(&fixture.body),
        };
        assert!(
            !options.is_empty(),
            "{}-{}：模型清單解析不出",
            fixture.cli,
            fixture.version
        );
    }
}

/// 錄音不得帶出使用者的路徑、名字或本機暫存目錄：整份檔案、以及把串流正文接起來之後（路徑可能被切成好幾段）都要乾淨。
#[test]
fn recordings_carry_no_user_paths_or_names() {
    let mut banned = vec![
        "/Users/".to_owned(),
        "-Users-".to_owned(),
        "%2FUsers%2F".to_owned(),
        "/home/".to_owned(),
        "\\Users\\".to_owned(),
        "/private/tmp/".to_owned(),
        "/tmp/".to_owned(),
        "/var/folders/".to_owned(),
        "claude-501".to_owned(),
        "spare-".to_owned(),
    ];
    if let Ok(user) = std::env::var("USER") {
        if user.len() >= 3 {
            banned.push(user);
        }
    }
    let check = |label: &str, text: &str| {
        for word in &banned {
            assert!(!text.contains(word.as_str()), "{label} 含 {word}");
        }
    };
    for dir in std::fs::read_dir(fixtures_dir()).unwrap() {
        for entry in std::fs::read_dir(dir.unwrap().path()).unwrap() {
            let path = entry.unwrap().path();
            check(
                &path.display().to_string(),
                &std::fs::read_to_string(&path).unwrap(),
            );
        }
    }
    for fixture in load().iter().filter(|f| f.scenario != "models") {
        let label = format!(
            "{}-{}-{} 正文",
            fixture.cli, fixture.version, fixture.scenario
        );
        check(&label, &replay(fixture).text);
    }
}

/// 每份成功錄音的預期數值（照各家 usage 契約手算：claude 輸入＝input＋建快取＋讀快取、grok 輸入＝input＋讀快取、
/// codex 的 input 已含快取、agy 依 total 判斷 input 是否含快取），以及去識別化後的識別碼。重錄夾具時一起更新。
struct Expected {
    fixture: &'static str,
    prompt: u64,
    cached: u64,
    output: u64,
    cost: Option<f64>,
    context: Option<u64>,
    id: &'static str,
}

const EXPECTED: &[Expected] = &[
    Expected {
        fixture: "agy/1.4.9-image",
        prompt: 42232,
        cached: 0,
        output: 1240,
        cost: None,
        context: None,
        id: "00000000-0000-4000-8000-000000000003",
    },
    Expected {
        fixture: "agy/1.4.9-lane-open",
        prompt: 396,
        cached: 0,
        output: 44,
        cost: None,
        context: None,
        id: "00000000-0000-4000-8000-000000000005",
    },
    Expected {
        fixture: "agy/1.4.9-lane-resume",
        prompt: 1051,
        cached: 0,
        output: 88,
        cost: None,
        context: None,
        id: "00000000-0000-4000-8000-000000000005",
    },
    Expected {
        fixture: "agy/1.4.9-oneshot",
        prompt: 391,
        cached: 0,
        output: 23,
        cost: None,
        context: None,
        id: "00000000-0000-4000-8000-000000000001",
    },
    Expected {
        fixture: "agy/1.4.9-think",
        prompt: 412,
        cached: 0,
        output: 115,
        cost: None,
        context: None,
        id: "00000000-0000-4000-8000-000000000002",
    },
    Expected {
        fixture: "claude/2.1.311-lane-open",
        prompt: 2561,
        cached: 1220,
        output: 4,
        cost: Some(0.00028220000000000003),
        context: Some(1000000),
        id: "00000000-0000-4000-8000-000000000039",
    },
    Expected {
        fixture: "claude/2.1.311-lane-resume",
        prompt: 2610,
        cached: 1220,
        output: 5,
        cost: Some(0.0005747),
        context: Some(1000000),
        id: "00000000-0000-4000-8000-000000000039",
    },
    Expected {
        fixture: "claude/2.1.311-oneshot",
        prompt: 2549,
        cached: 1211,
        output: 4,
        cost: Some(0.00028151),
        context: Some(1000000),
        id: "00000000-0000-4000-8000-000000000001",
    },
    Expected {
        fixture: "claude/2.1.311-think",
        prompt: 2552,
        cached: 954,
        output: 61,
        cost: Some(0.0141868),
        context: Some(1000000),
        id: "00000000-0000-4000-8000-000000000001",
    },
    Expected {
        fixture: "codex/0.164.0-image",
        prompt: 72696,
        cached: 60160,
        output: 171,
        cost: None,
        context: None,
        id: "00000000-0000-4000-8000-000000000003",
    },
    Expected {
        fixture: "codex/0.164.0-lane-open",
        prompt: 11580,
        cached: 11136,
        output: 5,
        cost: None,
        context: None,
        id: "00000000-0000-4000-8000-000000000005",
    },
    Expected {
        fixture: "codex/0.164.0-lane-resume",
        prompt: 23186,
        cached: 22272,
        output: 11,
        cost: None,
        context: None,
        id: "00000000-0000-4000-8000-000000000005",
    },
    Expected {
        fixture: "codex/0.164.0-oneshot",
        prompt: 11575,
        cached: 6912,
        output: 5,
        cost: None,
        context: None,
        id: "00000000-0000-4000-8000-000000000001",
    },
    Expected {
        fixture: "codex/0.164.0-think",
        prompt: 11594,
        cached: 11136,
        output: 19,
        cost: None,
        context: None,
        id: "00000000-0000-4000-8000-000000000002",
    },
    Expected {
        fixture: "grok/1.0.70-image",
        prompt: 77007,
        cached: 57856,
        output: 879,
        cost: Some(0.02465136),
        context: None,
        id: "00000000-0000-4000-8000-000000000008",
    },
    Expected {
        fixture: "grok/1.0.70-lane-open",
        prompt: 4934,
        cached: 1152,
        output: 281,
        cost: Some(0.00334084),
        context: None,
        id: "00000000-0000-4000-8000-000000000011",
    },
    Expected {
        fixture: "grok/1.0.70-lane-resume",
        prompt: 5241,
        cached: 4864,
        output: 48,
        cost: Some(0.00118116),
        context: None,
        id: "00000000-0000-4000-8000-000000000011",
    },
    Expected {
        fixture: "grok/1.0.70-oneshot",
        prompt: 3893,
        cached: 128,
        output: 17,
        cost: Some(0.00261664),
        context: None,
        id: "00000000-0000-4000-8000-000000000001",
    },
    Expected {
        fixture: "grok/1.0.70-think",
        prompt: 4948,
        cached: 1152,
        output: 63,
        cost: Some(0.00290564),
        context: None,
        id: "00000000-0000-4000-8000-000000000003",
    },
];

fn identity(fixture: &Fixture) -> Option<String> {
    let lines = || fixture.body.lines();
    match fixture.cli.as_str() {
        "codex" => lines().find_map(codex_thread_id),
        "agy" => lines().find_map(agy_conversation_id),
        "claude" => lines().find_map(|line| {
            let value: Value = serde_json::from_str(line).ok()?;
            (value["type"] == "result").then(|| value["session_id"].as_str().map(str::to_owned))?
        }),
        _ => lines().find_map(|line| {
            let value: Value = serde_json::from_str(line).ok()?;
            (value["type"] == "end").then(|| value["sessionId"].as_str().map(str::to_owned))?
        }),
    }
}

#[test]
fn recorded_numbers_and_ids_are_locked() {
    let fixtures = load();
    let successful: Vec<&Fixture> = fixtures
        .iter()
        .filter(|f| !matches!(f.scenario.as_str(), "models" | "badmodel"))
        .collect();
    assert_eq!(
        successful.len(),
        EXPECTED.len(),
        "預期表要跟成功錄音一一對應"
    );
    for expected in EXPECTED {
        let fixture = successful
            .iter()
            .find(|f| format!("{}/{}-{}", f.cli, f.version, f.scenario) == expected.fixture)
            .unwrap_or_else(|| panic!("{} 沒有錄音", expected.fixture));
        let label = expected.fixture;
        let usage = usage_of(fixture).unwrap();
        assert_eq!(usage.prompt_tokens, expected.prompt, "{label} prompt");
        assert_eq!(usage.cached_tokens, Some(expected.cached), "{label} cached");
        assert_eq!(usage.output_tokens, expected.output, "{label} output");
        match (usage.cost_usd, expected.cost) {
            (Some(got), Some(want)) => assert!((got - want).abs() < 1e-12, "{label} cost {got}"),
            (got, want) => assert_eq!(got, want, "{label} cost"),
        }
        if fixture.cli == "claude" {
            let model = input(fixture, "model").unwrap();
            let context = fixture
                .body
                .lines()
                .find_map(|line| crate::scene_budget::claude_model_usage(line, model))
                .and_then(|(_, context, _)| context);
            assert_eq!(context, expected.context, "{label} contextWindow");
        }
        assert_eq!(
            identity(fixture).as_deref(),
            Some(expected.id),
            "{label} id"
        );
    }
}
