//! 重構兩段判官的短命 session（refactor-mode-split 包 2）。
//! 第一段（初判）開線、第二段（盤點）resume 同一線：卡片 context 在 system 與 session
//! 歷史裡成為共用前綴，命中時第二段只付新指示的 token（拍板 3 的省費依據）。
//! 與遊玩 lane（lanes.rs）完全分離：不進 lanes.json、不保溫、不抹寫、跑完即棄——
//! 取消、重跑或卡片異動後舊 session 作廢（指紋由呼叫端核對），resume 失敗由呼叫端降級單發。

use crate::cli;
use crate::lanes::ClaudeCall;

/// 開線（第一段）：帶 --session-id 全量送 system＋prompt，回（回覆原文, session id）。
pub(crate) async fn open_stage(
    call: &ClaudeCall,
    world_id: &str,
    system: &str,
    prompt: &str,
    emit: impl FnMut(&str),
) -> Result<(String, String), String> {
    let session_id = crate::lanes::new_session_id();
    let raw = run_stage(call, world_id, system, prompt, &cli::ClaudeSession::Open(&session_id), emit).await?;
    Ok((raw, session_id))
}

/// 續聊（第二段）：--resume 同一線，只送第二段指示（卡片已在 session 歷史）。
/// Err＝session 認不得、CLI 拒絕 resume 等，由呼叫端降級成單發重送全卡。
pub(crate) async fn resume_stage(
    call: &ClaudeCall,
    world_id: &str,
    session_id: &str,
    system: &str,
    prompt: &str,
    emit: impl FnMut(&str),
) -> Result<String, String> {
    run_stage(call, world_id, system, prompt, &cli::ClaudeSession::Resume(session_id), emit).await
}

async fn run_stage(
    call: &ClaudeCall,
    world_id: &str,
    system: &str,
    prompt: &str,
    session: &cli::ClaudeSession<'_>,
    mut emit: impl FnMut(&str),
) -> Result<String, String> {
    let args = cli::claude_session_args(&call.model, system, session);
    cli::run_cli(
        &call.program,
        &call.working_dir,
        &args,
        prompt,
        &call.envs,
        cli::parse_claude_line,
        true, // 思考增量餵進度字尾：玩家分得出「在想」與「掛了」
        call.usage_log.as_deref().map(|path| cli::UsageLog {
            path,
            world: Some(world_id),
            transport: "claude",
            model: &call.model,
            parse: cli::parse_claude_usage,
            lane: None,
            shape: crate::usage_log::PromptShape::Oneshot,
            prompt_tokens_out: None,
        }),
        &mut emit,
    )
    .await
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// 假 claude CLI：記錄旗標與 prompt；--resume 找不到 session 檔＝非零碼結束（降級鏈用）。
    #[cfg(unix)]
    fn fake_cli(tag: &str) -> (PathBuf, ClaudeCall) {
        let dir = std::env::temp_dir().join(format!("tt-refsess-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fake-claude.py");
        std::fs::write(
            &script,
            r#"#!/usr/bin/env python3
import json, sys, os
args = sys.argv[1:]
def flag(name):
    return args[args.index(name) + 1] if name in args else None
sid, rid = flag('--session-id'), flag('--resume')
prompt = sys.stdin.read()
d = os.environ['FAKE_DIR']
with open(os.path.join(d, 'calls.jsonl'), 'a') as f:
    f.write(json.dumps({'args': args, 'prompt': prompt}) + '\n')
path = os.path.join(d, (sid or rid) + '.marker')
if rid and not os.path.exists(path):
    sys.exit(3)
open(path, 'a').close()
print(json.dumps({'type': 'result', 'is_error': False, 'result': 'RECOMMEND: interface\nEVIDENCE: ok'}))
"#,
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let call = ClaudeCall {
            program: script,
            working_dir: dir.clone(),
            envs: vec![("FAKE_DIR".to_owned(), dir.to_string_lossy().into_owned())],
            model: "opus".to_owned(),
            usage_log: None,
            claude_home: dir.clone(),
        };
        (dir, call)
    }

    #[cfg(unix)]
    fn calls(dir: &PathBuf, index: usize) -> (Vec<String>, String) {
        let text = std::fs::read_to_string(dir.join("calls.jsonl")).unwrap();
        let line: serde_json::Value = serde_json::from_str(text.lines().nth(index).unwrap()).unwrap();
        (
            line["args"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_owned()).collect(),
            line["prompt"].as_str().unwrap().to_owned(),
        )
    }

    /// 開線帶 --session-id 全量、續聊帶 --resume 同 id 且只送第二段指示；
    /// session 檔消失＝resume 回 Err（由呼叫端降級單發），不自行重開。
    #[cfg(unix)]
    #[tokio::test]
    async fn open_then_resume_shares_session_and_resume_fails_loud() {
        let _serial = crate::inflight::lock_real_process_tests();
        let (dir, call) = fake_cli("e2e");

        let (raw, sid) = open_stage(&call, "w1", "系統文", "第一段指示", |_| {}).await.unwrap();
        assert!(raw.contains("RECOMMEND"));
        let (args0, prompt0) = calls(&dir, 0);
        assert!(args0.windows(2).any(|w| w == ["--session-id", sid.as_str()]));
        assert!(args0.windows(2).any(|w| w == ["--system-prompt", "系統文"]));
        assert_eq!(prompt0, "第一段指示");

        let raw2 = resume_stage(&call, "w1", &sid, "系統文", "第二段指示", |_| {}).await.unwrap();
        assert!(raw2.contains("RECOMMEND"));
        let (args1, prompt1) = calls(&dir, 1);
        assert!(args1.windows(2).any(|w| w == ["--resume", sid.as_str()]));
        assert_eq!(prompt1, "第二段指示"); // 卡片 context 不重送

        std::fs::remove_file(dir.join(format!("{sid}.marker"))).unwrap();
        assert!(resume_stage(&call, "w1", &sid, "系統文", "第二段指示", |_| {}).await.is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
