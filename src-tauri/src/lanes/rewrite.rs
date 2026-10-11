use super::*;

/// grok 的 session 根目錄＝呼叫 CLI 時給的 GROK_HOME（cli::grok_envs），兩邊一定指同一處。
pub(super) fn grok_home(call: &LaneCall) -> Result<PathBuf, String> {
    call.envs
        .iter()
        .find(|(key, _)| key == "GROK_HOME")
        .map(|(_, value)| PathBuf::from(value))
        .ok_or_else(|| "grok lane 缺 GROK_HOME".to_owned())
}

/// grok 角色線回合後抹寫。stage：locate（找不到 GROK_HOME）／rewrite／compacted／lock-timeout。
pub(super) fn rewrite_grok(
    call: &LaneCall,
    session_id: &str,
    confidential: Option<&str>,
    prefix: &str,
    reply: &str,
    cleaned: &str,
) -> Result<(), RewriteFailure> {
    let home = at("locate", grok_home(call))?;
    grok_session::rewrite(&home, session_id, confidential, prefix, reply, cleaned).map_err(
        |error| RewriteFailure {
            stage: error.reason.stage(),
            detail: error.detail,
        },
    )
}

/// 回合後抹寫：機密段從注入的 user 行抹掉、最後一則 assistant 補名字前綴，
/// 原子寫＋回讀驗證（session_file::write_atomic）。
/// 回傳 Some(檔內有沒有 CLI 的 total_tokens 提醒)；沒東西要改、根本沒讀檔時回 None。
pub(super) fn apply_rewrite(
    call: &LaneCall,
    session_id: &str,
    confidential: Option<&str>,
    prefix: Option<&str>,
    reply: Option<session_file::ReplyRewrite<'_>>,
) -> Result<Option<bool>, RewriteFailure> {
    if confidential.is_none() && prefix.is_none() {
        return Ok(None);
    }
    let path = session_file::session_file_path(&call.claude_home, &call.working_dir, session_id);
    let mut file = at("load", session_file::load(&path))?;
    let reminder = session_file::has_total_tokens_reminder(&file);
    if let Some(segment) = confidential {
        let uuid = at(
            "find-segment",
            session_file::find_user_line_with_segment(&file, segment),
        )?;
        at(
            "erase-segment",
            session_file::erase_user_segment(&mut file, &uuid, segment),
        )?;
    }
    if let Some(prefix) = prefix {
        at(
            "prefix-assistant",
            session_file::prefix_last_assistant(&mut file, prefix, reply),
        )?;
    }
    at("write", session_file::write_atomic(&path, &file))?;
    Ok(Some(reminder))
}

/// 沒有抹寫的 claude 線（GM）另做只讀掃描；讀不到只回錯、由呼叫端記診斷，不丟線。
pub(super) fn scan_reminder_readonly(call: &LaneCall, session_id: &str) -> Result<bool, String> {
    let path = session_file::session_file_path(&call.claude_home, &call.working_dir, session_id);
    session_file::read_text(&path).map(|text| session_file::text_has_total_tokens_reminder(&text))
}

/// D 偵測的診斷：只記帳不改線。同一 session、同一種結果在一次 app 執行內只記一次。
pub(super) fn note_reminder(
    call: &LaneCall,
    world_id: &str,
    key: &str,
    session_id: &str,
    scan: Result<bool, String>,
) {
    let reason = match scan {
        Ok(false) => return,
        Ok(true) => "seen",
        Err(_) => "scan-failed",
    };
    static NOTED: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::OnceLock::new();
    let first = NOTED
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(format!("{session_id}\u{1f}{reason}"));
    if !first {
        return;
    }
    if let Some(path) = call.usage_log.as_deref() {
        usage_log::append_event(
            path,
            call.provider.as_str(),
            Some(world_id),
            key,
            usage_log::Event::CliReminderSeen,
            reason,
        );
    }
}

/// 抹寫／截尾失敗落帳：哪一步壞、原錯誤字串（遮路徑、截斷）。測試包另在丟線與刪檔之前
/// 把 session 檔與要抹的片段留證。只記錄，不改丟線策略。
#[allow(clippy::too_many_arguments)]
pub(super) fn record_rewrite_failure(
    call: &LaneCall,
    world_id: &str,
    key: &str,
    reason: &str,
    session_id: &str,
    failure: &RewriteFailure,
    confidential: Option<&str>,
    prefix: Option<&str>,
) {
    let path = session_file::session_file_path(&call.claude_home, &call.working_dir, session_id);
    #[cfg(feature = "test-harness")]
    crate::harness::rewrite_evidence(key, reason, failure, &path, confidential, prefix);
    #[cfg(not(feature = "test-harness"))]
    let _ = (confidential, prefix);
    let paths = rewrite_failure::known_paths(&path, &call.claude_home, &call.working_dir);
    log_drop_failure(call, world_id, key, reason, failure, &paths);
}
