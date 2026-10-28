//! 認不得的輸出行與「上次正常版本」（plans/cli-version-gate.md 包 1 第 3、4 點）。
//! 認不得的行：成功的輪只記一次 log（行程記憶體內依 CLI＋版本＋種類去重，重開 app 重記）；
//! 這輪沒收尾、或成功收尾但正文與收尾文字都空時，才升格成 `CliOutputUnrecognized*`。
//! 上次正常版本＝這版曾完整跑完一輪，存在資料根目錄的 `cli-last-ok.json`，只在最終成功後寫。

use super::super::types::{CliProtocol, UsageLog};
use super::VersionProbe;
use crate::ui_msg::UiMsg;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

pub(super) const LAST_OK_FILE: &str = "cli-last-ok.json";

/// 一輪內見到的認不得的行，依種類收成集合。
#[derive(Default)]
pub(super) struct Unrecognized {
    labels: BTreeSet<String>,
}

impl Unrecognized {
    pub(super) fn note(&mut self, label: String) {
        self.labels.insert(label);
    }

    pub(super) fn seen(&self) -> bool {
        !self.labels.is_empty()
    }

    /// 寫 log：同一 CLI、同一版本、同一種類在這次 app 執行內只記一次（只在行程記憶體裡去重）。
    pub(super) fn log_once(&self, cli: &str, version: Option<&str>) {
        static SEEN: OnceLock<Mutex<HashSet<(String, String, String)>>> = OnceLock::new();
        for label in &self.labels {
            let key = (
                cli.to_owned(),
                version.unwrap_or("?").to_owned(),
                label.clone(),
            );
            let fresh = SEEN
                .get_or_init(Default::default)
                .lock()
                .map(|mut seen| seen.insert(key))
                .unwrap_or(true);
            if fresh {
                log::warn!(
                    "[cli] {cli} {} emitted an unrecognized line ({label})",
                    version.unwrap_or("(version unknown)")
                );
            }
        }
    }
}

fn store_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(Default::default)
}

fn read_map(path: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub(super) fn last_ok_path(usage_log: &Path) -> PathBuf {
    usage_log.with_file_name(LAST_OK_FILE)
}

pub(super) fn read_last_ok(path: &Path, cli: &str) -> Option<String> {
    let _guard = store_lock().lock().ok()?;
    read_map(path).remove(cli)
}

/// 最終成功後記下版本：一把行程內的鎖包住讀→改→寫臨時檔→rename，四家排隊；
/// 版本沒變就不寫，寫失敗只記 log，不影響這輪結果。
pub(super) fn record_last_ok(path: &Path, cli: &str, version: &str) {
    let Ok(_guard) = store_lock().lock() else {
        return;
    };
    let mut map = read_map(path);
    if map.get(cli).map(String::as_str) == Some(version) {
        return;
    }
    map.insert(cli.to_owned(), version.to_owned());
    let result = serde_json::to_string_pretty(&map)
        .map_err(std::io::Error::other)
        .and_then(|text| {
            let temp = path.with_extension("json.tmp");
            // world-write-exempt: CLI 上次正常版本，在資料根目錄，不是桌目錄
            std::fs::write(&temp, text)?;
            std::fs::rename(&temp, path)
        });
    if let Err(error) = result {
        log::warn!("[cli] could not record last good {cli} version: {error}");
    }
}

/// `--version` 原文裡的版本號（`grok 1.0.70 (f5)`→`1.0.70`、`codex-cli 0.164.0`→`0.164.0`），
/// 訊息裡已經有 CLI 名，原文照放會疊字；找不到就用原文。
fn version_number(line: &str) -> String {
    line.split(|ch: char| ch.is_whitespace() || ch == '(' || ch == ')')
        .find(|part| part.starts_with(|ch: char| ch.is_ascii_digit()) && part.contains('.'))
        .unwrap_or(line)
        .to_owned()
}

/// 格式認不得時給玩家的訊息：依目前版本與上次正常版本分四種。
pub(super) fn message(
    cli: &str,
    version: Option<&str>,
    last_ok: Option<&str>,
    tail: String,
) -> UiMsg {
    let cli = cli.to_owned();
    let version = version.map(version_number);
    let last_ok = last_ok.map(version_number);
    match (version.as_deref(), last_ok.as_deref()) {
        (None, _) => UiMsg::CliOutputUnrecognizedUnknownVersion { cli, tail },
        (Some(version), Some(last)) if last != version => UiMsg::CliOutputUnrecognizedChanged {
            cli,
            version: version.to_owned(),
            last_ok: last.to_owned(),
            tail,
        },
        (Some(version), Some(_)) => UiMsg::CliOutputUnrecognizedSame {
            cli,
            version: version.to_owned(),
            tail,
        },
        (Some(version), None) => UiMsg::CliOutputUnrecognizedNever {
            cli,
            version: version.to_owned(),
            tail,
        },
    }
}

pub(super) fn stderr_tail(stderr_text: &str) -> String {
    stderr_text
        .lines()
        .rev()
        .take(5)
        .collect::<Vec<_>>()
        .join("\n")
}

/// 格式認不得的失敗：帶目前版本與上次正常版本（帳本路徑旁的 cli-last-ok.json）。
pub(super) async fn failure(
    protocol: &CliProtocol,
    seen: &Unrecognized,
    version: &mut VersionProbe,
    usage_log: &Option<UsageLog<'_>>,
    tail: String,
) -> Box<dyn std::error::Error + Send + Sync> {
    let current = version.finish().await;
    seen.log_once(protocol.id, current.as_deref());
    let last_ok = usage_log
        .as_ref()
        .and_then(|log| read_last_ok(&last_ok_path(log.path), protocol.id));
    message(protocol.name, current.as_deref(), last_ok.as_deref(), tail).into_error()
}

/// 這輪最終成功：記一次認不得的行（若有），並把版本記成上次正常版本（帳本路徑旁的檔）。
pub(super) async fn completed(
    protocol: &CliProtocol,
    seen: &Unrecognized,
    version: &mut VersionProbe,
    usage_log: &Option<UsageLog<'_>>,
) {
    let Some(path) = usage_log.as_ref().map(|log| log.path) else {
        seen.log_once(protocol.id, None);
        return;
    };
    let finished = version.finish().await;
    seen.log_once(protocol.id, finished.as_deref());
    if let Some(finished) = finished {
        record_last_ok(&last_ok_path(path), protocol.id, &finished);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tt-last-ok-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(LAST_OK_FILE)
    }

    #[test]
    fn last_ok_keeps_each_cli_and_skips_same_version() {
        let path = temp("keep");
        assert_eq!(read_last_ok(&path, "grok"), None);
        record_last_ok(&path, "grok", "grok 1.0.70 (f5)");
        record_last_ok(&path, "agy", "1.4.9");
        assert_eq!(
            read_last_ok(&path, "grok").as_deref(),
            Some("grok 1.0.70 (f5)")
        );
        assert_eq!(read_last_ok(&path, "agy").as_deref(), Some("1.4.9"));
        let before = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        record_last_ok(&path, "agy", "1.4.9");
        assert_eq!(
            std::fs::metadata(&path).unwrap().modified().unwrap(),
            before
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn concurrent_writers_queue_up() {
        let path = temp("race");
        std::thread::scope(|scope| {
            for (cli, version) in [
                ("claude", "2"),
                ("codex", "0.1"),
                ("agy", "1"),
                ("grok", "9"),
            ] {
                let path = path.clone();
                scope.spawn(move || record_last_ok(&path, cli, version));
            }
        });
        for cli in ["claude", "codex", "agy", "grok"] {
            assert!(read_last_ok(&path, cli).is_some(), "{cli} lost");
        }
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn version_number_drops_the_cli_name() {
        assert_eq!(
            version_number("grok 1.0.70 (f50904eaee6c) [stable]"),
            "1.0.70"
        );
        assert_eq!(version_number("codex-cli 0.164.0"), "0.164.0");
        assert_eq!(version_number("2.1.311 (Claude Code)"), "2.1.311");
        assert_eq!(version_number("weird"), "weird");
    }

    #[test]
    fn message_picks_neutral_wording_for_the_same_version() {
        let tail = String::new;
        assert!(matches!(
            message("Grok", Some("1.0.71"), Some("1.0.70"), tail()),
            UiMsg::CliOutputUnrecognizedChanged { .. }
        ));
        assert!(matches!(
            message("Grok", Some("1.0.70"), Some("1.0.70"), tail()),
            UiMsg::CliOutputUnrecognizedSame { .. }
        ));
        assert!(matches!(
            message("Grok", Some("1.0.70"), None, tail()),
            UiMsg::CliOutputUnrecognizedNever { .. }
        ));
        assert!(matches!(
            message("Grok", None, Some("1.0.70"), tail()),
            UiMsg::CliOutputUnrecognizedUnknownVersion { .. }
        ));
    }
}
