use super::*;

/// grok 錯誤字串裡可能出現的路徑：session 目錄（群組名是編碼過的 cwd）、GROK_HOME、工作目錄。
/// 要在刪目錄之前算。
fn grok_known_paths(call: &LaneCall, session_id: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    if let Ok(home) = grok_home(call) {
        for dir in grok_session::find_session_dirs(&home, session_id).unwrap_or_default() {
            pairs.push((dir.display().to_string(), "<session>".to_owned()));
        }
        pairs.push((home.display().to_string(), "<dir>".to_owned()));
    }
    pairs.push((call.working_dir.display().to_string(), "<dir>".to_owned()));
    pairs
}

/// 撤銷 grok 線的緣由。預定重開（換幕、改卡等）不是出事，不記丟線——呼叫行本來就帶 reopen。
pub(super) enum GrokRevoke {
    Reopen,
    Drop(&'static str),
    Failed(RewriteFailure),
}

/// 撤銷一條 grok 線：先從 store 拿掉並落檔，再刪 session 目錄。落檔失敗就回錯——磁碟上的
/// 舊狀態要嘛帶著 pending_rewrite、要嘛已被別的路徑重開，都不會再續用這個 id。
/// 刪目錄失敗只記帳：store 已沒有這個 id，不會再續聊。
pub(super) fn revoke_grok_lane(
    call: &LaneCall,
    world_id: &str,
    key: &str,
    session_id: &str,
    store: &mut LaneStore,
    store_path: &Path,
    why: GrokRevoke,
) -> Result<(), String> {
    let paths = grok_known_paths(call, session_id);
    match &why {
        GrokRevoke::Reopen => {}
        GrokRevoke::Drop(reason) => log_drop(call, world_id, key, reason),
        GrokRevoke::Failed(failure) => {
            log_drop_failure(call, world_id, key, "rewrite-failed", failure, &paths)
        }
    }
    store.remove(key);
    let written = write_store(store_path, store);
    let removed = grok_home(call).and_then(|home| grok_session::remove_session(&home, session_id));
    if let Err(detail) = removed {
        let failure = RewriteFailure {
            stage: "cleanup",
            detail,
        };
        log_drop_failure(call, world_id, key, "cleanup-failed", &failure, &paths);
    }
    written
}

/// 抹寫失敗：session 內容不可信，刪掉檔。檔案本來就不在（NotFound）當已刪。
/// 其他刪除錯誤交回呼叫端，不能當成這條線已經棄用成功。
fn abandon_session(call: &LaneCall, session_id: &str) -> Result<(), String> {
    if call.provider != LaneProvider::Claude {
        return Ok(());
    }
    remove_claude_session(call, session_id)
}

/// 刪一個 claude session 檔，不看本輪 provider（呼叫端已確認那條線是 claude 開的）。
fn remove_claude_session(call: &LaneCall, session_id: &str) -> Result<(), String> {
    let path = session_file::session_file_path(&call.claude_home, &call.working_dir, session_id);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(UiMsg::SessionAbandonFailed {
            path: path.display().to_string(),
            error: error.to_string(),
        }
        .into()),
    }
}

/// 重開前撤掉被取代的 claude session（GM 線、單角色線；共線到尾段世界書門檻時）：先從 store 拿掉並落檔，落檔成功才刪檔
/// （落檔失敗回錯、舊檔留著，磁碟上的舊狀態仍指著它）；刪檔失敗只記帳，store 已不再續用這個 id。
/// 不這樣清，常重開的線每次會留一份整幕逐字稿檔。
pub(super) fn retire_claude_session(
    call: &LaneCall,
    world_id: &str,
    key: &str,
    session_id: &str,
    store: &mut LaneStore,
    store_path: &Path,
) -> Result<(), String> {
    store.remove(key);
    write_store(store_path, store)?;
    if let Err(error) = remove_claude_session(call, session_id) {
        let path =
            session_file::session_file_path(&call.claude_home, &call.working_dir, session_id);
        let paths = rewrite_failure::known_paths(&path, &call.claude_home, &call.working_dir);
        let failure = RewriteFailure {
            stage: "cleanup",
            detail: error,
        };
        log_drop_failure(call, world_id, key, "cleanup-failed", &failure, &paths);
    }
    Ok(())
}

/// 中止回合的收尾。抹寫成功就什麼都不改——呼叫前已經寫下 pending_rewrite、expected_reply 仍是 None。
pub(super) fn settle_abort(
    call: &LaneCall,
    world_id: &str,
    key: &str,
    session_id: &str,
    confidential: Option<&str>,
    prefix: Option<&str>,
    store: &mut LaneStore,
    store_path: &Path,
) -> Result<(), String> {
    // grok 中止時檔案可能停在半截（有 user、沒回覆），不去抹，整條撤銷
    if call.provider == LaneProvider::Grok {
        return revoke_grok_lane(
            call,
            world_id,
            key,
            session_id,
            store,
            store_path,
            GrokRevoke::Drop("aborted"),
        );
    }
    let rewrite = match call.provider {
        // 中止與失控：只抹機密、補前綴，不換寫收尾文字（半截內容不再續用，下一輪重開）
        LaneProvider::Claude => {
            apply_rewrite(call, session_id, confidential, prefix, None).map(|_| ())
        }
        LaneProvider::Agy | LaneProvider::Grok => Ok(()),
    };
    if let Err(failure) = rewrite {
        record_rewrite_failure(
            call,
            world_id,
            key,
            "rewrite-failed",
            session_id,
            &failure,
            confidential,
            prefix,
        );
        // 刪檔失敗也要先清線再回錯：呼叫端看到 Err 就不會把這一輪報成中止成功。
        let abandon = abandon_session(call, session_id);
        store.remove(key);
        write_store(store_path, store)?;
        if let Err(error) = abandon {
            let path =
                session_file::session_file_path(&call.claude_home, &call.working_dir, session_id);
            let paths = rewrite_failure::known_paths(&path, &call.claude_home, &call.working_dir);
            let failure = RewriteFailure {
                stage: "cleanup",
                detail: error.clone(),
            };
            log_drop_failure(call, world_id, key, "cleanup-failed", &failure, &paths);
            return Err(error);
        }
    }
    Ok(())
}
