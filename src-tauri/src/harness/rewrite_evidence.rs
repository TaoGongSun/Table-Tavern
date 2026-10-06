//! 抹寫／截尾失敗留證（usage-cache-audit）：在丟線與刪 session 檔之前，把當下的 session 檔、
//! 要抹的片段與失敗原因寫進 `<root>/rewrite-failures/<毫秒>-<亂數>-<安全化線名>/`，供逐字比對。
//! 留證失敗不擋原本的清線／刪檔，只在 AI log 記一行 `rewrite-evidence-failed`。

use std::path::{Path, PathBuf};

use serde_json::json;

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

fn write_evidence(
    dir: &Path,
    lane: &str,
    reason: &str,
    failure: &RewriteFailure,
    session: &Path,
    segment: Option<&str>,
    prefix: Option<&str>,
) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("建留證目錄失敗：{e}"))?;
    let copied = match std::fs::copy(session, dir.join("session.jsonl")) {
        Ok(_) => None,
        Err(error) => Some(error.to_string()),
    };
    let metadata = json!({
        "lane": lane,
        "reason": reason,
        "stage": failure.stage,
        "detail": failure.detail,
        "session_path": session.display().to_string(),
        "session_copy_error": copied,
        "segment": segment,
        "prefix": prefix,
    });
    let text = serde_json::to_string_pretty(&metadata).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("metadata.json"), text).map_err(|e| format!("寫 metadata 失敗：{e}"))
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
    if let Err(error) = write_evidence(&dir, lane, reason, failure, session, segment, prefix) {
        super::ai_log::ai_note(
            "rewrite-evidence-failed",
            json!({ "lane": lane, "reason": reason, "stage": failure.stage, "error": error }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        write_evidence(
            &first,
            "chars:a/b",
            "rewrite-failed",
            &failure,
            &session,
            Some("機密"),
            Some("A："),
        )
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(first.join("session.jsonl")).unwrap(),
            "{}\n"
        );
        let meta: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(first.join("metadata.json")).unwrap())
                .unwrap();
        assert_eq!(meta["lane"], "chars:a/b");
        assert_eq!(meta["stage"], "load");
        assert_eq!(meta["segment"], "機密");

        // session 檔已不在：照樣留下 metadata，註明複製失敗
        write_evidence(
            &second,
            "chars:a/b",
            "rewrite-failed",
            &failure,
            &root.join("gone"),
            None,
            None,
        )
        .unwrap();
        let meta: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(second.join("metadata.json")).unwrap())
                .unwrap();
        assert!(meta["session_copy_error"].is_string());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
