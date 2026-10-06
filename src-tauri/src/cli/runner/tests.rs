#![allow(clippy::await_holding_lock)]

use super::super::stream::{parse_claude_line, parse_claude_usage};
use super::*;

/// 以假 CLI 腳本走完 spawn→stdin→逐行解析→增量→收尾整條路（sh 腳本，僅 unix）
#[cfg(unix)]
#[tokio::test]
async fn run_cli_streams_deltas_from_fake_cli_and_reads_stdin() {
    // run_cli 現在會把子程序 pid 登記進 inflight 的全域 children 表；kill_all_children
    // 的測試（inflight.rs）會不分青紅皂白殺表上全部 pid，故用同一把鎖互斥執行。
    let _serial = crate::inflight::lock_real_process_tests();
    let dir = std::env::temp_dir().join(format!("tt-fake-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let working_dir = dir.join("workspace");
    std::fs::create_dir_all(&working_dir).unwrap();
    std::fs::write(working_dir.join("cwd-marker"), "").unwrap();
    let script = dir.join("fake-claude.sh");
    std::fs::write(
        &script,
        concat!(
            "#!/bin/sh\n",
            "input=$(cat)\n", // 必須把 stdin 讀完，證明 prompt 有送達
            "test -f ./cwd-marker || exit 8\n",
            "echo '{\"type\":\"system\",\"subtype\":\"init\"}'\n",
            "echo '{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"想\"}}}'\n",
            "echo '{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"你\"}}}'\n",
            "echo '{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"好\"}}}'\n",
            "echo \"{\\\"type\\\":\\\"result\\\",\\\"is_error\\\":false,\\\"result\\\":\\\"你好\\\",\\\"total_cost_usd\\\":0.0015,\\\"usage\\\":{\\\"input_tokens\\\":1,\\\"cache_creation_input_tokens\\\":0,\\\"cache_read_input_tokens\\\":99,\\\"output_tokens\\\":2}}\"\n",
            "test \"$input\" = \"提示詞\" || exit 9\n",
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let log_path = dir.join("prompt-cache.jsonl");
    let seen = std::sync::atomic::AtomicU64::new(0);
    let mut deltas = Vec::new();
    let full = run_cli(
        &script,
        &working_dir,
        &[],
        "提示詞",
        &[],
        parse_claude_line,
        true,
        Some(UsageLog {
            path: &log_path,
            world: Some("w1"),
            transport: "claude",
            model: "sonnet",
            parse: parse_claude_usage,
            lane: None,
            shape: crate::usage::log::PromptShape::Oneshot,
            prompt_tokens_out: Some(&seen),
            conversation_id_out: None,
            expected_conversation_id: None,
            agy_usage_base: None,
            agy_usage_out: None,
        }),
        |delta: &str| {
            deltas.push(delta.to_owned());
        },
    )
    .await
    .unwrap();
    // 同一份輸出、關掉思考轉發：聊天正文串流不得混進思考
    let mut quiet_deltas = Vec::new();
    let quiet = run_cli(
        &script,
        &working_dir,
        &[],
        "提示詞",
        &[],
        parse_claude_line,
        false,
        None,
        |delta: &str| {
            quiet_deltas.push(delta.to_owned());
        },
    )
    .await
    .unwrap();
    let logged = std::fs::read_to_string(&log_path).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(quiet, "你好");
    assert_eq!(quiet_deltas, ["你", "好"]);
    // 思考增量進顯示流、不進正文
    assert_eq!(full, "你好");
    assert_eq!(deltas, ["想", "你", "好"]);
    // 收尾事件落一行 JSONL：總輸入 100（1＋0＋99）、讀快取 99 → 99%
    assert_eq!(logged.lines().count(), 1);
    let record: serde_json::Value = serde_json::from_str(logged.trim()).unwrap();
    assert_eq!(record["transport"], "claude");
    assert_eq!(record["world"], "w1");
    assert_eq!(record["model"], "sonnet");
    assert_eq!(record["prompt_tokens"], 100);
    assert_eq!(record["cached_tokens"], 99);
    assert_eq!(record["created_tokens"], 0);
    assert_eq!(record["output_tokens"], 2);
    assert_eq!(record["hit_rate"], 99.0);
    assert_eq!(record["cost_usd"], 0.0015);
    // 無狀態路徑照樣判快取結果（本案修的就是這裡以前短路成「單發」）；
    // 時間戳到秒（分鐘精度分不出是否踩到 5 分鐘過期線）
    assert_eq!(record["mode"], "oneshot");
    assert_eq!(record["cache"], "hit");
    assert_eq!(record["ts"].as_str().unwrap().len(), 19);
    // 總輸入回填給呼叫端，續聊線用它當下輪的理論可中量
    assert_eq!(seen.load(std::sync::atomic::Ordering::Relaxed), 100);
}

#[cfg(unix)]
#[tokio::test]
async fn run_cli_aborts_instantly_on_fatal_stderr_api_error_and_shows_it_in_tail() {
    let _serial = crate::inflight::lock_real_process_tests();
    let dir = std::env::temp_dir().join(format!("tt-fake-cli-fatal-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("fake-claude-fatal.sh");
    // stderr 吐設定類 API 錯誤後長睡（模擬 CLI 自己退避重試）；沒有立即中止就會撞測試逾時
    std::fs::write(
        &script,
        concat!(
            "#!/bin/sh\n",
            "cat > /dev/null\n",
            "echo 'API Error: 502 unknown provider for model claude-opus-4-7' >&2\n",
            "sleep 30\n",
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut deltas = Vec::new();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        run_cli(
            &script,
            &dir,
            &[],
            "提示詞",
            &[],
            parse_claude_line,
            true,
            None,
            |delta: &str| deltas.push(delta.to_owned()),
        ),
    )
    .await
    .expect("設定類錯誤必須立即中止，不得等 CLI 睡完");
    let error = result.unwrap_err().to_string();
    assert!(error.contains("unknown provider"), "錯誤要帶原文：{error}");
    // 進度字尾也要同步看到錯誤行
    assert!(deltas.iter().any(|d| d.contains("API Error")));
}

#[cfg(unix)]
#[tokio::test]
async fn run_cli_reports_crash_without_result_event_instead_of_returning_partial_text() {
    let _serial = crate::inflight::lock_real_process_tests();
    let dir = std::env::temp_dir().join(format!("tt-fake-cli-crash-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("fake-claude-crash.sh");
    // 吐一筆正文增量後 crash（無 result 收尾事件）：殘缺正文不得當成功返回
    std::fs::write(
        &script,
        concat!(
            "#!/bin/sh\n",
            "cat > /dev/null\n",
            "echo '{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"殘\"}}}'\n",
            "echo 'proxy connection reset' >&2\n",
            "exit 3\n",
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut deltas = Vec::new();
    let error = run_cli(
        &script,
        &dir,
        &[],
        "提示詞",
        &[],
        parse_claude_line,
        true,
        None,
        |delta: &str| deltas.push(delta.to_owned()),
    )
    .await
    .unwrap_err()
    .to_string();
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(
        error.starts_with(r#"TTMSG:{"code":"cli_reply_error""#)
            && error.contains(r#"\"code\":\"cli_crashed\""#),
        "要報 crash 而非靜默：{error}"
    );
    assert!(
        error.contains("proxy connection reset"),
        "要帶 stderr 尾巴：{error}"
    );
    // 進度字尾同步看到 ⚠，玩家不用等到收尾才知道
    assert!(deltas.iter().any(|d| d.contains('⚠')));
}

#[cfg(unix)]
#[tokio::test]
async fn run_cli_strips_inherited_anthropic_env_but_keeps_explicit_envs() {
    let _serial = crate::inflight::lock_real_process_tests();
    let dir = std::env::temp_dir().join(format!("tt-fake-cli-env-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("fake-claude-env.sh");
    // 繼承的 ANTHROPIC_BASE_URL 必須被拔掉；顯式 envs 傳入的 MARKER 必須到位
    std::fs::write(
        &script,
        concat!(
            "#!/bin/sh\n",
            "cat > /dev/null\n",
            "test -z \"$ANTHROPIC_BASE_URL\" || exit 7\n",
            "test \"$ANTHROPIC_MARKER\" = \"explicit\" || exit 9\n",
            "echo '{\"type\":\"result\",\"is_error\":false,\"result\":\"ok\"}'\n",
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    std::env::set_var("ANTHROPIC_BASE_URL", "http://127.0.0.1:9999");
    let result = run_cli(
        &script,
        &dir,
        &[],
        "提示詞",
        &[("ANTHROPIC_MARKER".to_owned(), "explicit".to_owned())],
        parse_claude_line,
        false,
        None,
        |_: &str| {},
    )
    .await;
    std::env::remove_var("ANTHROPIC_BASE_URL");
    assert_eq!(result.unwrap(), "ok");
}

/// 反例：CLI 先吐 1MB stdout 才開始讀 stdin（2MB）。舊做法「先寫完 stdin 才讀輸出」會讓兩邊
/// 各卡在滿掉的管線上，等到 60 秒逾時；寫入與讀取並行才過得去。
#[cfg(unix)]
#[tokio::test]
async fn run_cli_feeds_long_stdin_while_draining_output() {
    let _serial = crate::inflight::lock_real_process_tests();
    let dir = std::env::temp_dir().join(format!("tt-fake-cli-pipe-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("fake-chatty.sh");
    std::fs::write(
        &script,
        concat!(
            "#!/bin/sh\n",
            "head -c 1048576 /dev/zero | tr '\\0' 'x'\n",
            "echo\n",
            "size=$(wc -c | tr -d ' ')\n",
            "echo \"{\\\"type\\\":\\\"result\\\",\\\"is_error\\\":false,\\\"result\\\":\\\"got $size\\\"}\"\n",
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let payload = "中".repeat(700_000); // 2.1MB
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        run_cli(
            &script,
            &dir,
            &[],
            &payload,
            &[],
            parse_claude_line,
            false,
            None,
            |_: &str| {},
        ),
    )
    .await
    .expect("stdin 與輸出互等：寫入要跟讀取並行");
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(result.unwrap(), format!("got {}", payload.len()));
}

/// 提早 return（這裡是 stderr 致命錯）也要先收屍才返回：呼叫端接著要刪提示詞暫存檔，
/// Windows 上程序還開著檔就刪不掉。
#[cfg(unix)]
#[tokio::test]
async fn run_cli_reaps_child_before_returning_an_error() {
    let _serial = crate::inflight::lock_real_process_tests();
    let dir = std::env::temp_dir().join(format!("tt-fake-cli-reap-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("fake-fatal-pid.sh");
    std::fs::write(
        &script,
        concat!(
            "#!/bin/sh\n",
            "echo $$ > \"$PID_FILE\"\n",
            "cat > /dev/null\n",
            "echo 'API Error: 401 authentication_error' >&2\n",
            "exec sleep 30\n",
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let pid_file = dir.join("pid");
    let error = run_cli(
        &script,
        &dir,
        &[],
        "x",
        &[(
            "PID_FILE".to_owned(),
            pid_file.to_string_lossy().into_owned(),
        )],
        parse_claude_line,
        false,
        None,
        |_: &str| {},
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("authentication_error"));
    let pid = std::fs::read_to_string(&pid_file).unwrap();
    let alive = std::process::Command::new("kill")
        .args(["-0", pid.trim()])
        .status()
        .unwrap()
        .success();
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(!alive, "run_cli 返回時子程序 {pid} 還活著");
}

#[test]
fn api_error_kind_classifies_fatal_vs_transient() {
    // 暫時性：讓 CLI 重試，但 Some(false) 表示要餵進度
    assert_eq!(
        api_error_kind("API Error: 529 overloaded, retrying"),
        Some(false)
    );
    // 設定類：模型不存在／認證，立即中止
    assert_eq!(
        api_error_kind("API Error: 502 unknown provider for model x"),
        Some(true)
    );
    assert_eq!(
        api_error_kind("API Error: 401 authentication_error"),
        Some(true)
    );
    // 非錯誤行不動作
    assert_eq!(api_error_kind("thinking hard..."), None);
}

/// 提示詞暫存檔＋run_cli 的端對端：用 rustc 當場編一支原生假 CLI（Windows 上就是真 .exe，
/// 不經 .cmd／shell），它照真 CLI 的樣子讀 `--system-prompt-file` 與 stdin、把讀到的 bytes
/// 原樣寫回 out 資料夾。路徑刻意含中文與空白。Windows CI 跑這組才驗得到「程序還開著檔時刪不掉」。
mod prompt_file_e2e {
    use super::*;
    use crate::cli::PromptFile;
    use std::path::{Path, PathBuf};

    const FAKE_SRC: &str = r#"
use std::io::Read;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let flag = |name: &str| args.iter().position(|a| a == name).map(|i| args[i + 1].clone());
    let out = std::path::PathBuf::from(flag("--out").unwrap());
    let tag = flag("--tag").unwrap();
    let mut held = std::fs::File::open(flag("--system-prompt-file").unwrap()).unwrap();
    let mut system = Vec::new();
    held.read_to_end(&mut system).unwrap();
    std::fs::write(out.join(format!("{tag}-system")), &system).unwrap();
    let mut stdin = Vec::new();
    std::io::stdin().read_to_end(&mut stdin).unwrap();
    std::fs::write(out.join(format!("{tag}-stdin")), &stdin).unwrap();
    if args.iter().any(|a| a == "--hang") {
        // 一直開著 system 檔不放，模擬程序還沒死就要刪檔
        std::fs::write(out.join(format!("{tag}-ready")), b"").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
    drop(held);
    println!("{{\"type\":\"result\",\"is_error\":false,\"result\":\"ok {tag}\"}}");
}
"#;

    struct Fixture {
        base: PathBuf,
        program: PathBuf,
        prompts: PathBuf,
        out: PathBuf,
    }

    fn fixture(tag: &str) -> Fixture {
        let base = std::env::temp_dir().join(format!("tt 提示檔 {tag} {}", ulid::Ulid::generate()));
        let prompts = base.join("cli 暫存");
        let out = base.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let src = base.join("fake.rs");
        std::fs::write(&src, FAKE_SRC).unwrap();
        let program = base.join(format!("假 cli{}", std::env::consts::EXE_SUFFIX));
        let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let status = std::process::Command::new(rustc)
            .args(["--edition", "2021", "-o"])
            .arg(&program)
            .arg(&src)
            .status()
            .expect("需要 rustc 來編假 CLI");
        assert!(status.success());
        Fixture {
            base,
            program,
            prompts,
            out,
        }
    }

    fn args(fx: &Fixture, tag: &str, system: &Path, hang: bool) -> Vec<String> {
        let mut args = vec![
            "--out".to_owned(),
            fx.out.to_string_lossy().into_owned(),
            "--tag".to_owned(),
            tag.to_owned(),
            "--system-prompt-file".to_owned(),
            system.to_string_lossy().into_owned(),
        ];
        if hang {
            args.push("--hang".to_owned());
        }
        args
    }

    async fn wait_for(path: &Path) {
        for _ in 0..200 {
            if path.exists() {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        panic!("等不到 {}", path.display());
    }

    /// 長內容（超過兩個平台的命令列上限）經檔案與 stdin 逐 byte 到達；四支並發不串檔；
    /// run_cli 返回後檔案立刻刪得掉。
    #[tokio::test]
    async fn long_prompts_arrive_intact_concurrently_and_files_are_removed() {
        let _serial = crate::inflight::lock_real_process_tests();
        let fx = fixture("並發");
        let jobs: Vec<(String, String, String)> = (0..4)
            .map(|index| {
                (
                    format!("t{index}"),
                    format!("第{index}份 system\r\n") + &"中文長桌測試。".repeat(60_000),
                    format!("第{index}份正文 \"引號\" ") + &"玩家說話。".repeat(150_000),
                )
            })
            .collect();
        let files: Vec<PromptFile> = jobs
            .iter()
            .map(|(_, system, _)| PromptFile::create(&fx.prompts, "system.txt", system).unwrap())
            .collect();
        let runs = jobs.iter().zip(&files).map(|((tag, _, stdin), file)| {
            let args = args(&fx, tag, file.path(), false);
            let program = fx.program.clone();
            let base = fx.base.clone();
            async move {
                run_cli(
                    &program,
                    &base,
                    &args,
                    stdin,
                    &[],
                    parse_claude_line,
                    false,
                    None,
                    |_: &str| {},
                )
                .await
            }
        });
        let results = futures_util::future::join_all(runs).await;
        for ((tag, system, stdin), result) in jobs.iter().zip(results) {
            assert_eq!(result.unwrap(), format!("ok {tag}"));
            assert_eq!(
                std::fs::read(fx.out.join(format!("{tag}-system"))).unwrap(),
                system.as_bytes()
            );
            assert_eq!(
                std::fs::read(fx.out.join(format!("{tag}-stdin"))).unwrap(),
                stdin.as_bytes()
            );
        }
        let paths: Vec<PathBuf> = files.iter().map(|f| f.path().to_path_buf()).collect();
        drop(files);
        for path in paths {
            assert!(!path.exists(), "收屍後應立刻刪得掉：{}", path.display());
        }
        std::fs::remove_dir_all(&fx.base).unwrap();
    }

    /// 停止鍵：程序還開著檔時取消，run_cli 先收屍才返回，檔案立刻刪得掉。
    #[tokio::test]
    async fn cancel_reaps_child_so_the_file_can_be_removed_right_away() {
        let _serial = crate::inflight::lock_real_process_tests();
        let fx = fixture("取消");
        let file = PromptFile::create(&fx.prompts, "system.txt", "取消測試").unwrap();
        let path = file.path().to_path_buf();
        let args = args(&fx, "c", &path, true);
        let (tx, rx) = tokio::sync::watch::channel(false);
        let ready = fx.out.join("c-ready");
        let trigger = async {
            wait_for(&ready).await;
            tx.send(true).unwrap();
        };
        let run = run_cli_cancellable(
            &fx.program,
            &fx.base,
            &args,
            "x",
            &[],
            parse_claude_line,
            false,
            None,
            |_: &str| {},
            Some(rx),
        );
        let (finish, ()) = tokio::join!(run, trigger);
        assert!(matches!(finish.unwrap(), CliFinish::Aborted(_)));
        drop(file);
        assert!(!path.exists(), "取消後應立刻刪得掉");
        std::fs::remove_dir_all(&fx.base).unwrap();
    }

    /// 外層 future 被 drop（中止在途呼叫）：等不到收屍，只靠 kill_on_drop；Windows 上當下可能
    /// 刪不掉，背景重試要在幾秒內補刪成功。
    #[tokio::test]
    async fn dropped_future_still_gets_its_file_removed_by_the_retry() {
        let _serial = crate::inflight::lock_real_process_tests();
        let fx = fixture("丟棄");
        let file = PromptFile::create(&fx.prompts, "system.txt", "丟棄測試").unwrap();
        let path = file.path().to_path_buf();
        let args = args(&fx, "d", &path, true);
        let program = fx.program.clone();
        let base = fx.base.clone();
        let task = tokio::spawn(async move {
            let _file = file; // 跟著 future 一起被 drop
            run_cli(
                &program,
                &base,
                &args,
                "x",
                &[],
                parse_claude_line,
                false,
                None,
                |_: &str| {},
            )
            .await
        });
        wait_for(&fx.out.join("d-ready")).await;
        task.abort();
        let _ = task.await;
        for _ in 0..240 {
            if !path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        assert!(!path.exists(), "背景重試沒有補刪：{}", path.display());
        let _ = std::fs::remove_dir_all(&fx.base);
    }
}

/// 輸出 EOF 之後的三種收場：迴圈不能只看兩條輸出，stdin 還沒寫完、程序還沒退出時，
/// 逾時、取消、收屍都要照樣有效。
#[cfg(unix)]
mod output_closed_first {
    use super::*;

    struct Case {
        dir: std::path::PathBuf,
        script: std::path::PathBuf,
        pid_file: std::path::PathBuf,
    }

    /// 假 CLI：先記下 pid、關掉 stdout／stderr，再跑 `body`。
    fn case(tag: &str, body: &str) -> Case {
        let dir = std::env::temp_dir().join(format!(
            "tt-fake-cli-eof-{tag}-{}-{}",
            std::process::id(),
            ulid::Ulid::generate()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fake.sh");
        let pid_file = dir.join("pid");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\necho $$ > '{}'\nexec >&- 2>&-\n{body}\n",
                pid_file.display()
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        Case {
            dir,
            script,
            pid_file,
        }
    }

    fn reaped(case: &Case) -> bool {
        let pid = std::fs::read_to_string(&case.pid_file).unwrap();
        !std::process::Command::new("kill")
            .args(["-0", pid.trim()])
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success()
    }

    fn payload() -> String {
        "中".repeat(700_000) // 2.1MB，遠大於管線緩衝，寫入一定還掛著
    }

    #[tokio::test]
    async fn reads_full_stdin_after_closing_output_then_reports_no_reply() {
        let _serial = crate::inflight::lock_real_process_tests();
        let payload = payload();
        // 工作目錄就是 case.dir：讀滿整份 stdin 才留下標記檔
        let case = case(
            "read",
            &format!(
                "size=$(wc -c | tr -d ' ')\n[ \"$size\" = {} ] && touch got-all\nexit 0",
                payload.len()
            ),
        );
        let marker = case.dir.join("got-all");
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            run_cli(
                &case.script,
                &case.dir,
                &[],
                &payload,
                &[],
                parse_claude_line,
                false,
                None,
                |_: &str| {},
            ),
        )
        .await
        .expect("輸出關掉後仍要把 stdin 寫完並收場");
        let error = result.unwrap_err().to_string();
        assert!(error.contains("cli_no_reply"), "應回無回覆：{error}");
        assert!(marker.exists(), "CLI 沒讀到完整 stdin");
        assert!(reaped(&case));
        std::fs::remove_dir_all(&case.dir).unwrap();
    }

    #[tokio::test]
    async fn never_reading_stdin_after_closing_output_hits_the_write_timeout() {
        let _serial = crate::inflight::lock_real_process_tests();
        let case = case("timeout", "exec sleep 30");
        let started = std::time::Instant::now();
        let result = tokio::time::timeout(
            STDIN_WRITE_TIMEOUT + std::time::Duration::from_secs(5),
            run_cli(
                &case.script,
                &case.dir,
                &[],
                &payload(),
                &[],
                parse_claude_line,
                false,
                None,
                |_: &str| {},
            ),
        )
        .await
        .expect("寫入逾時要生效，不能卡到 CLI 睡完");
        let error = result.unwrap_err().to_string();
        assert!(
            error.contains("cli_stdin_timeout"),
            "應回 stdin 逾時：{error}"
        );
        assert!(started.elapsed() >= STDIN_WRITE_TIMEOUT);
        assert!(reaped(&case), "返回前要收屍");
        std::fs::remove_dir_all(&case.dir).unwrap();
    }

    #[tokio::test]
    async fn cancel_after_output_closed_still_aborts_and_reaps() {
        let _serial = crate::inflight::lock_real_process_tests();
        let case = case("cancel", "exec sleep 30");
        let payload = payload();
        let (tx, rx) = tokio::sync::watch::channel(false);
        // 等 CLI 起來並關掉輸出（pid 檔在關輸出前寫好），再過 300ms 才按停止
        let pid_file = case.pid_file.clone();
        let trigger = async move {
            while !pid_file.exists() {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            tx.send(true).unwrap();
        };
        let run = run_cli_cancellable(
            &case.script,
            &case.dir,
            &[],
            &payload,
            &[],
            parse_claude_line,
            false,
            None,
            |_: &str| {},
            Some(rx),
        );
        let (finish, ()) = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            tokio::join!(run, trigger)
        })
        .await
        .expect("輸出關掉後取消仍要生效");
        assert!(matches!(finish.unwrap(), CliFinish::Aborted(_)));
        assert!(reaped(&case), "返回前要收屍");
        std::fs::remove_dir_all(&case.dir).unwrap();
    }
}
