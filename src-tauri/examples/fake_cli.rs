//! 測試用假 CLI（跨平台）：`cargo test` 會順便編 examples，正式 `cargo build`／`tauri build` 不編。
//! 測試從 `src/test_fake_cli.rs` 找到它。角色由 `TT_FAKE_CLI_MODE` 選：
//! - `claude`：假 claude，照真檔格式寫 session JSONL、把旗標與 prompt 記進 calls.jsonl（`FAKE_*` 旋鈕見 claude()）。
//! - `agy`：假 agy，開線回固定 conversation ID，續聊只認同一 ID。
//! - `script`：`TT_FAKE_CLI_SCRIPT` 逐行指令（見 script()），給 runner 測試模擬各種 CLI 行為。

use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::Path;
use std::process::exit;
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match var("TT_FAKE_CLI_MODE").as_deref() {
        Some("claude") => claude(&args),
        Some("agy") => agy(&args),
        Some("script") => script(&var("TT_FAKE_CLI_SCRIPT").unwrap_or_default()),
        other => panic!("未知 TT_FAKE_CLI_MODE：{other:?}"),
    }
}

/// 空字串當沒設（照 python `os.environ.get(...) or ...` 的語意）。
fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn flag(args: &[String], name: &str) -> Option<String> {
    let at = args.iter().position(|arg| arg == name)?;
    args.get(at + 1).cloned()
}

fn read_stdin() -> String {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    input
}

/// 印一行並 flush；讀端關了就安靜結束（輸出失控的測試會先關管線）。
fn emit(line: impl std::fmt::Display) {
    let mut out = std::io::stdout().lock();
    if writeln!(out, "{line}").and_then(|_| out.flush()).is_err() {
        exit(0);
    }
}

fn append(path: &Path, lines: &[Value]) {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    for line in lines {
        writeln!(file, "{line}").unwrap();
    }
}

/// 忽略 SIGTERM（Windows 沒有這回事，強殺擋不掉）。
fn ignore_term() {
    #[cfg(unix)]
    // SAFETY: 只把 SIGTERM 設成忽略
    unsafe {
        libc::signal(libc::SIGTERM, libc::SIG_IGN);
    }
}

fn sleep_secs(secs: u64) {
    std::thread::sleep(Duration::from_secs(secs));
}

fn claude(args: &[String]) {
    let sid = flag(args, "--session-id");
    let rid = flag(args, "--resume");
    let prompt = read_stdin();
    let system = flag(args, "--system-prompt-file").map(|f| std::fs::read_to_string(f).unwrap());
    let dir = std::path::PathBuf::from(var("FAKE_SESSION_DIR").unwrap());
    append(
        &dir.join("calls.jsonl"),
        &[json!({"args": args, "prompt": prompt, "system": system,
                 "reminder_env": var("CLAUDE_CODE_TOTAL_TOKENS_REMINDER")})],
    );
    let path = dir.join(format!("{}.jsonl", sid.as_ref().or(rid.as_ref()).unwrap()));
    let stream = var("FAKE_STREAM").as_deref() == Some("1");
    let prefix = var("FAKE_REPLY_PREFIX").unwrap_or_default();
    let delta = |text: &str| {
        emit(
            json!({"type": "stream_event", "event": {"delta": {"type": "text_delta", "text": text}}}),
        )
    };
    let (mut last, mut users) = (Value::Null, 0);
    if rid.is_some() {
        let Ok(existing) = std::fs::read_to_string(&path) else {
            // FAKE_STREAM：先吐一段才失敗（續聊失敗重開時，上一試已經吐過字）
            if stream {
                delta(&format!("{prefix}舊"));
            }
            exit(3);
        };
        for line in existing.lines().filter(|l| !l.trim().is_empty()) {
            let node: Value = serde_json::from_str(line).unwrap();
            if node["uuid"].is_string() {
                last = node["uuid"].clone();
            }
            users += usize::from(node["type"] == "user");
        }
    }
    // 照真 CLI（2.1.287）的形狀只追加：queue-operation、user、attachment 夾雜項行、
    // thinking 與 text 拆成兩則 assistant、assistant 後再接 attachment
    let mut out = vec![json!({"type": "queue-operation", "operation": "enqueue"})];
    let mut node = |out: &mut Vec<Value>, kind: &str, key: &str, body: Value| {
        let me = Value::String(ulid::Ulid::generate().to_string());
        out.push(json!({key: body, "type": kind, "uuid": me, "parentUuid": last}));
        last = me;
    };
    node(
        &mut out,
        "user",
        "message",
        json!({"role": "user", "content": prompt}),
    );
    node(
        &mut out,
        "attachment",
        "attachment",
        json!({"type": "environment"}),
    );
    // FAKE_REMINDER：照真 CLI 存一筆 total_tokens 提醒（關提醒的環境變數失效時的樣子）
    if var("FAKE_REMINDER").as_deref() == Some("1") {
        let text = "<total_tokens>15000000 tokens left</total_tokens>";
        node(
            &mut out,
            "attachment",
            "attachment",
            json!({"type": "total_tokens_reminder", "text": text}),
        );
    }
    out.push(json!({"type": "atis-latch"}));
    node(
        &mut out,
        "attachment",
        "attachment",
        json!({"type": "date"}),
    );
    // FAKE_REPLY_SUFFIX：回覆尾端接控制區塊；FAKE_REPLY_RESUME：續聊那試整段換成這個
    let reply = rid
        .as_ref()
        .and_then(|_| var("FAKE_REPLY_RESUME"))
        .unwrap_or_else(|| {
            let suffix = var("FAKE_REPLY_SUFFIX").unwrap_or_default();
            format!("{prefix}回覆{}{suffix}", users + 1)
        });
    let content = |block: Value| json!({"role": "assistant", "content": [block]});
    node(
        &mut out,
        "assistant",
        "message",
        content(json!({"type": "thinking", "thinking": "…"})),
    );
    node(
        &mut out,
        "assistant",
        "message",
        content(json!({"type": "text", "text": reply})),
    );
    node(
        &mut out,
        "attachment",
        "attachment",
        json!({"type": "prompt_snapshot"}),
    );
    out.push(json!({"type": "last-prompt"}));
    // FAKE_NO_SESSION：不寫 session 檔（回合後讀檔就會失敗）
    if var("FAKE_NO_SESSION").as_deref() != Some("1") {
        append(&path, &out);
    }
    // 下列旋鈕名稱可加 _RESUME／_OPEN 後綴，只套在那種嘗試：
    // FAKE_USAGE＝收尾用量 JSON；FAKE_RATE_BEFORE／AFTER＝在 result 前後插 rate_limit_event；
    // FAKE_FAIL＝回 is_error 的 result 後以非零碼結束；FAKE_HANG＝吐完 BEFORE 事件後卡住等中止
    let kind = if rid.is_some() { "RESUME" } else { "OPEN" };
    let knob = |name: &str| var(&format!("{name}_{kind}")).or_else(|| var(name));
    let usage: Value = knob("FAKE_USAGE").map_or_else(
        || {
            json!({"input_tokens": 3, "cache_read_input_tokens": 0,
                   "cache_creation_input_tokens": 1000, "output_tokens": 10,
                   "cache_creation": {"ephemeral_1h_input_tokens": 1000, "ephemeral_5m_input_tokens": 0}})
        },
        |raw| serde_json::from_str(&raw).unwrap(),
    );
    let rate = |name: &str| {
        if let Some(info) = knob(name) {
            let info: Value = serde_json::from_str(&info).unwrap();
            emit(json!({"type": "rate_limit_event", "rate_limit_info": info}));
        }
    };
    rate("FAKE_RATE_BEFORE");
    if knob("FAKE_HANG").is_some() {
        std::fs::write(dir.join("hanging"), "").unwrap();
        sleep_secs(30);
    }
    if knob("FAKE_FAIL").is_some() {
        emit(json!({"type": "result", "is_error": true, "result": "boom", "usage": usage}));
        exit(1);
    }
    // FAKE_RUNAWAY：吐不停的空白（輸出失控）；FAKE_RUNAWAY_BLOCK 另把 session 檔換成目錄，抹寫與棄用都會失敗
    if knob("FAKE_RUNAWAY").is_some() {
        if knob("FAKE_RUNAWAY_BLOCK").is_some() {
            std::fs::remove_file(&path).unwrap();
            std::fs::create_dir_all(&path).unwrap();
        }
        loop {
            delta(" ");
        }
    }
    if stream {
        for ch in reply.chars() {
            delta(&ch.to_string());
        }
    }
    emit(json!({"type": "result", "is_error": false, "result": reply, "usage": usage}));
    rate("FAKE_RATE_AFTER");
}

fn agy(args: &[String]) {
    let cid = flag(args, "--conversation");
    let prompt = read_stdin();
    let dir = std::path::PathBuf::from(var("FAKE_AGY_DIR").unwrap());
    append(
        &dir.join("calls.jsonl"),
        &[json!({"args": args, "prompt": prompt})],
    );
    let on = |name: &str| var(name).as_deref() == Some("1");
    if let Some(cid) = &cid {
        if cid != "agy-conversation-1" {
            exit(3);
        }
        if on("FAKE_STALL_RESUME") {
            emit(json!({"event": "init", "conversation_id": cid}));
            sleep_secs(60);
        }
        if on("FAKE_RUNAWAY_RESUME") {
            emit(json!({"event": "init", "conversation_id": cid}));
            loop {
                let update = json!({"step_type": "agent_response", "text_delta": " "});
                emit(json!({"event": "step_update", "step_update": update}));
            }
        }
    }
    let resumed = cid.is_some();
    let turn = if resumed { 2 } else { 1 };
    let (input, output, cached) = if resumed {
        (1200, 250, 900)
    } else {
        (1000, 100, 0)
    };
    let reply = format!("{}回覆{turn}", var("FAKE_REPLY_PREFIX").unwrap_or_default());
    emit(json!({"event": "init", "conversation_id": "agy-conversation-1"}));
    emit(json!({"event": "result", "result": {
        "status": "SUCCESS", "response": reply, "num_turns": turn,
        "usage": {"input_tokens": input, "output_tokens": output, "thinking_tokens": 20,
                  "cache_read_tokens": cached, "total_tokens": input + output}}}));
    if on("FAKE_HANG_AFTER_RESULT") {
        ignore_term();
        sleep_secs(60);
    }
}

/// 逐行指令（第一個空白前是指令，其後是參數）：
/// `pid` 把 pid 寫進 `$PID_FILE`；`stdin` 讀完 stdin；`out`／`err` 印一行到 stdout／stderr
/// （`{stdin_len}` 換成已讀 stdin 的位元組數）；`out-bytes N` 印 N 個 x 不換行；`sleep MS`；
/// `hang` 忽略 SIGTERM 後睡 120 秒；`exit CODE`；`need-file CODE PATH`、`need-stdin CODE TEXT`、
/// `need-env CODE NAME=VALUE`（VALUE 空＝必須沒設）不符就以 CODE 結束。
fn script(lines: &str) {
    let mut input = String::new();
    for line in lines.lines() {
        let (op, rest) = line.split_once(' ').unwrap_or((line, ""));
        let (code, arg) = rest.split_once(' ').unwrap_or((rest, ""));
        let fail_unless = |ok: bool| {
            if !ok {
                exit(code.parse().unwrap());
            }
        };
        match op {
            "pid" => {
                std::fs::write(var("PID_FILE").unwrap(), std::process::id().to_string()).unwrap()
            }
            "stdin" => input = read_stdin(),
            "out" => emit(rest.replace("{stdin_len}", &input.len().to_string())),
            "err" => eprintln!("{rest}"),
            "out-bytes" => {
                let bytes = vec![b'x'; rest.parse().unwrap()];
                std::io::stdout().lock().write_all(&bytes).unwrap();
            }
            "sleep" => std::thread::sleep(Duration::from_millis(rest.parse().unwrap())),
            "hang" => {
                ignore_term();
                sleep_secs(120);
            }
            "exit" => exit(rest.parse().unwrap()),
            "need-file" => fail_unless(Path::new(arg).exists()),
            "need-stdin" => fail_unless(input == arg),
            "need-env" => {
                let (name, value) = arg.split_once('=').unwrap();
                fail_unless(var(name).unwrap_or_default() == value);
            }
            other => panic!("未知指令：{other}"),
        }
    }
}
