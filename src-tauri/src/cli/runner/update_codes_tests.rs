//! cli-version-gate 包 2 第 2 點：需要更新 CLI 的錯誤經 run_cli 掛上自己的碼（四家都用真實錄音）。
#![cfg(unix)]
#![allow(clippy::await_holding_lock)]

use super::super::stream::{AGY_PROTOCOL, CLAUDE_PROTOCOL, CODEX_PROTOCOL, GROK_PROTOCOL};
use super::*;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

struct Fake {
    dir: PathBuf,
    script: PathBuf,
}

impl Drop for Fake {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fake(body: &str) -> Fake {
    let dir = std::env::temp_dir().join(format!("tt-fake-update-codes-{}", ulid::Ulid::generate()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("fake.sh");
    std::fs::write(&script, format!("#!/bin/sh\ncat > /dev/null\n{body}")).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    Fake { dir, script }
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/stream/fixtures")
}

async fn run(fx: &Fake, protocol: &CliProtocol, args: &[String]) -> String {
    match run_cli_cancellable(
        &fx.script,
        &fx.dir,
        args,
        "x",
        &[],
        protocol,
        false,
        RunawayPolicy::Off,
        None,
        |_: &str| {},
        None,
        SessionWatch::default(),
    )
    .await
    {
        Err(error) => error.to_string(),
        Ok(_) => panic!("{}：應該失敗", protocol.id),
    }
}

const PROTOCOLS: [&CliProtocol; 4] = [
    &CLAUDE_PROTOCOL,
    &CODEX_PROTOCOL,
    &AGY_PROTOCOL,
    &GROK_PROTOCOL,
];

fn version_of(cli: &str) -> &'static str {
    crate::cli::tested_versions::tested(cli).unwrap()
}

#[tokio::test]
async fn recorded_bad_model_turns_become_cli_model_unavailable() {
    let _serial = crate::inflight::lock_real_process_tests();
    for protocol in PROTOCOLS {
        let cli = protocol.id;
        let stem = format!("{}-badmodel", version_of(cli));
        let record: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(fixtures().join(cli).join(format!("{stem}.args.json")))
                .unwrap(),
        )
        .unwrap();
        let body = fixtures().join(cli).join(format!("{stem}.jsonl"));
        let tail = record["stderr_tail"].as_str().unwrap_or_default();
        let fx = fake(&format!(
            "cat '{}'\ncat >&2 <<'TAIL'\n{tail}\nTAIL\nexit 1\n",
            body.display()
        ));
        let args: Vec<String> = ["--model", "no-such-model-xyz"].map(str::to_owned).to_vec();
        let error = run(&fx, protocol, &args).await;
        assert!(error.contains("cli_model_unavailable"), "{cli}: {error}");
        assert!(error.contains("no-such-model-xyz"), "{cli}: {error}");
    }
}

#[tokio::test]
async fn unknown_flag_crash_becomes_cli_too_old() {
    let _serial = crate::inflight::lock_real_process_tests();
    for (protocol, stderr) in [
        (&CLAUDE_PROTOCOL, "error: unknown option '--safe-mode'"),
        (
            &CODEX_PROTOCOL,
            "error: unexpected argument '--ignore-user-config' found",
        ),
        (&AGY_PROTOCOL, "flags provided but not defined: -agent"),
        (
            &GROK_PROTOCOL,
            "error: unexpected argument '--prompt-file' found",
        ),
    ] {
        let fx = fake(&format!("echo \"{stderr}\" >&2\nexit 2\n"));
        let error = run(&fx, protocol, &[]).await;
        assert!(error.contains("cli_too_old"), "{}: {error}", protocol.id);
    }
}

/// 一般的崩潰不歸類：留給「無法分類」那條（前端放「檢查 CLI 更新」）。
#[tokio::test]
async fn plain_crash_keeps_cli_crashed() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fx = fake("echo 'segfault somewhere' >&2\nexit 139\n");
    let error = run(&fx, &CLAUDE_PROTOCOL, &[]).await;
    assert!(error.contains("cli_crashed"), "{error}");
}
