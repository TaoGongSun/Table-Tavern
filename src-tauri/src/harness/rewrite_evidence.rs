//! 抹寫／截尾失敗留證（usage-cache-audit）：在丟線與刪 session 檔之前，把當下的 session 檔、
//! 要抹的片段與失敗原因寫進 `<root>/rewrite-failures/<毫秒>-<亂數>-<安全化線名>/`，供逐字比對。
//! 留證是同步檔案 I/O，清線會晚這麼一點（AI log 的 `rewrite-evidence` 行附耗時）；
//! 失敗不擋原本的清線／刪檔，只在 AI log 記一行 `rewrite-evidence-failed`。

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::lanes::RewriteFailure;

/// 線名（`chars:<model>`，模型可能含 `/`）→ 只留英數、`-`、`.`，其餘換 `_`。
fn safe_name(lane: &str) -> String {
    lane.chars()
        .map(
            |c| match c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                true => c,
                false => '_',
            },
        )
        .collect()
}

/// 寫留證並回傳要記進 AI log 的那一行（事件名、內容）。session 檔複製失敗仍寫 metadata，
/// 但整體算失敗：只有 metadata 的留證看不出當下 session 長怎樣。
fn write_evidence(
    dir: &Path,
    lane: &str,
    reason: &str,
    failure: &RewriteFailure,
    session: &Path,
    segment: Option<&str>,
    prefix: Option<&str>,
) -> (&'static str, Value) {
    let started = std::time::Instant::now();
    let failed = |error: String, metadata_written: bool| {
        (
            "rewrite-evidence-failed",
            json!({
                "lane": lane,
                "reason": reason,
                "stage": failure.stage,
                "error": error,
                "metadata_written": metadata_written,
            }),
        )
    };
    if let Err(error) = std::fs::create_dir_all(dir) {
        return failed(format!("建留證目錄失敗：{error}"), false);
    }
    let copy_error = std::fs::copy(session, dir.join("session.jsonl"))
        .err()
        .map(|error| format!("複製 session 檔失敗：{error}"));
    let metadata = json!({
        "lane": lane,
        "reason": reason,
        "stage": failure.stage,
        "detail": failure.detail,
        "session_path": session.display().to_string(),
        "session_copy_error": copy_error,
        "segment": segment,
        "prefix": prefix,
    });
    let written = serde_json::to_string_pretty(&metadata)
        .map_err(|error| error.to_string())
        .and_then(|text| {
            std::fs::write(dir.join("metadata.json"), text)
                .map_err(|error| format!("寫 metadata 失敗：{error}"))
        });
    match (written, copy_error) {
        (Err(error), _) => failed(error, false),
        (Ok(()), Some(error)) => failed(error, true),
        (Ok(()), None) => (
            "rewrite-evidence",
            json!({
                "lane": lane,
                "dir": dir.display().to_string(),
                "elapsed_ms": started.elapsed().as_millis() as u64,
            }),
        ),
    }
}

fn evidence_dir(root: &Path, lane: &str) -> PathBuf {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    root.join("rewrite-failures")
        .join(format!("{ms}-{}-{}", super::random_hex(4), safe_name(lane)))
}

pub(crate) fn rewrite_evidence(
    lane: &str,
    reason: &str,
    failure: &RewriteFailure,
    session: &Path,
    segment: Option<&str>,
    prefix: Option<&str>,
) {
    let Some(root) = super::harness_root() else {
        return; // 尚未 boot（單元測試）
    };
    let dir = evidence_dir(&root, lane);
    let (event, detail) = write_evidence(&dir, lane, reason, failure, session, segment, prefix);
    super::ai_log::ai_note(event, detail);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_meta(dir: &Path) -> Value {
        serde_json::from_str(&std::fs::read_to_string(dir.join("metadata.json")).unwrap()).unwrap()
    }

    #[test]
    fn evidence_dir_is_safe_and_unique_and_keeps_the_raw_lane_in_metadata() {
        assert_eq!(
            safe_name("chars:openai/gpt-5.6:free"),
            "chars_openai_gpt-5.6_free"
        );
        let root =
            std::env::temp_dir().join(format!("tt-evidence-{}", super::super::random_hex(4)));
        let first = evidence_dir(&root, "chars:a/b");
        let second = evidence_dir(&root, "chars:a/b");
        assert_ne!(first, second);
        assert_eq!(
            first.parent(),
            Some(root.join("rewrite-failures").as_path())
        );

        let session = root.join("s.jsonl");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&session, "{}\n").unwrap();
        let failure = RewriteFailure {
            stage: "load",
            detail: "壞".to_owned(),
        };
        let (event, note) = write_evidence(
            &first,
            "chars:a/b",
            "rewrite-failed",
            &failure,
            &session,
            Some("機密"),
            Some("A："),
        );
        assert_eq!(event, "rewrite-evidence");
        assert!(note["elapsed_ms"].is_u64());
        assert_eq!(
            std::fs::read_to_string(first.join("session.jsonl")).unwrap(),
            "{}\n"
        );
        let meta = read_meta(&first);
        assert_eq!(meta["lane"], "chars:a/b");
        assert_eq!(meta["stage"], "load");
        assert_eq!(meta["segment"], "機密");

        // session 檔讀不到、目錄可寫：照樣留下 metadata，但整體記成失敗
        let (event, note) = write_evidence(
            &second,
            "chars:a/b",
            "rewrite-failed",
            &failure,
            &root.join("gone"),
            None,
            None,
        );
        assert_eq!(event, "rewrite-evidence-failed");
        assert_eq!(note["metadata_written"], true);
        assert!(read_meta(&second)["session_copy_error"].is_string());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
