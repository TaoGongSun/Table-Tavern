//! GM 線凍結 system 一變就整線重開（gm-line-system-change-restart）：Claude 不再補丁，
//! 被取代的舊 claude session 檔落檔成功後刪掉；system 沒變照常續聊。

use super::*;
use serde_json::Value;

const WORLD_A: &str = "## 世界設定\n燈塔還亮著\n";
const WORLD_B: &str = "## 世界設定\n燈塔熄了\n";
const WORLD_C: &str = "## 世界設定\n燈塔倒了\n";

fn gm_turn<'a>(events: &'a [TranscriptEvent], system: &str) -> TurnInput<'a> {
    let mut input = turn_input(events, 0);
    input.lane = Lane::Gm;
    input.frozen_system = system.to_owned();
    input.tail = "## 目前狀態\n夜晚\n\n請推進劇情。".to_owned();
    input.prefix = None;
    input.echo = ReplyEcho::Narration;
    input
}

fn gm_state(events: &[TranscriptEvent], system: &str) -> LaneState {
    let mut state = lane_state(events, 0);
    state.snapshot = system.to_owned();
    state.applied = system.to_owned();
    state
}

#[test]
fn claude_gm_plan_reopens_on_any_system_change() {
    let events = [event(TranscriptKind::Player, "", "阿濤", "你好")];
    let live = 1_001;
    let expired = 1_000 + LEGACY_CACHE_TTL_SECS + 1;
    assert!(matches!(
        plan_turn(None, &gm_turn(&events, WORLD_A), live, LaneProvider::Claude),
        TurnPlan::Reopen {
            reason: ReopenReason::FirstTurn
        }
    ));
    let state = gm_state(&events, WORLD_A);
    for now in [live, expired] {
        assert!(
            matches!(
                plan_turn(
                    Some(&state),
                    &gm_turn(&events, WORLD_B),
                    now,
                    LaneProvider::Claude
                ),
                TurnPlan::Reopen {
                    reason: ReopenReason::SystemChanged
                }
            ),
            "@{now}：不走補丁、也不追平"
        );
    }
    // 升級前快取內補過丁、尚未追平的舊線：素材已追上也重開，歷史裡的補丁不留
    let mut patched = gm_state(&events, WORLD_B);
    patched.snapshot = WORLD_A.to_owned();
    assert!(matches!(
        plan_turn(
            Some(&patched),
            &gm_turn(&events, WORLD_B),
            live,
            LaneProvider::Claude
        ),
        TurnPlan::Reopen {
            reason: ReopenReason::SystemChanged
        }
    ));
}

#[test]
fn claude_gm_plan_resumes_when_system_is_unchanged() {
    let before = [event(TranscriptKind::Player, "", "阿濤", "你好")];
    let mut state = gm_state(&before, WORLD_A);
    state.expected_reply = Some(ExpectedReply {
        speaker_id: String::new(),
        kind: TranscriptKind::Narration,
        text: "夜色漸深。".to_owned(),
    });
    let events = [
        before[0].clone(),
        event(TranscriptKind::Narration, "", "", "夜色漸深。"),
        event(TranscriptKind::Player, "", "阿濤", "點燈"),
    ];
    let live = 1_001;
    let expired = 1_000 + LEGACY_CACHE_TTL_SECS + 1;
    for now in [live, expired] {
        let mut input = gm_turn(&events, WORLD_A);
        // 只改狀態與導演指示（尾段）不算 system 變動
        input.tail = "## 目前狀態\n深夜\n\n換人說話。".to_owned();
        match plan_turn(Some(&state), &input, now, LaneProvider::Claude) {
            TurnPlan::Resume {
                session_id,
                base,
                system,
                patch,
                rebased,
            } => {
                assert_eq!(session_id, "sid-1");
                assert_eq!(base, 2, "上輪旁白已在 session 裡，水位跳過它");
                assert_eq!(system, WORLD_A);
                assert!(patch.is_none() && !rebased, "@{now}");
            }
            TurnPlan::Reopen { .. } => panic!("@{now}：system 沒變要續聊"),
        }
    }
}

struct Ledger(Vec<Value>);

impl Ledger {
    fn read(path: &Path) -> Self {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        Self(
            text.lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect(),
        )
    }

    /// 呼叫行（不含丟線等事件行）
    fn calls(&self) -> Vec<&Value> {
        self.0
            .iter()
            .filter(|row| row.get("event").is_none())
            .collect()
    }
}

fn recorded_calls(session_dir: &Path) -> Vec<Value> {
    std::fs::read_to_string(session_dir.join("calls.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn session_flag(call: &Value) -> (bool, String) {
    let args: Vec<&str> = call["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|arg| arg.as_str().unwrap())
        .collect();
    for (flag, resumed) in [("--resume", true), ("--session-id", false)] {
        if let Some(index) = args.iter().position(|arg| *arg == flag) {
            return (resumed, args[index + 1].to_owned());
        }
    }
    panic!("沒有 session 旗標：{args:?}");
}

fn session_exists(fake: &FakeCli, session_id: &str) -> bool {
    session_file::session_file_path(&fake.claude_home, &fake.working_dir, session_id).exists()
}

/// 跑一輪 GM，把回覆落成正典旁白並接一句玩家發言（下一輪的增量）。
async fn narrate(
    call: &LaneCall,
    fake: &FakeCli,
    events: &mut Vec<TranscriptEvent>,
    system: &str,
    next_line: &str,
) -> Result<(), String> {
    let reply = run_turn(
        call,
        &fake.root,
        &fake.world_id,
        gm_turn(events, system),
        None,
        |_| {},
    )
    .await?
    .text;
    events.push(event(TranscriptKind::Narration, "", "", &reply));
    events.push(event(TranscriptKind::Player, "", "阿濤", next_line));
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn gm_lane_reopens_on_system_change_and_deletes_the_replaced_session() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_claude("gm-restart");
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第一句")];
    for (index, world) in [WORLD_A, WORLD_B, WORLD_C, WORLD_C].iter().enumerate() {
        narrate(
            &fake.call,
            &fake,
            &mut events,
            world,
            &format!("第{}句", index + 2),
        )
        .await
        .unwrap();
    }
    let calls = recorded_calls(&fake.session_dir);
    assert_eq!(calls.len(), 4);
    let sessions: Vec<(bool, String)> = calls.iter().map(session_flag).collect();
    for (index, world) in [WORLD_A, WORLD_B, WORLD_C].iter().enumerate() {
        assert!(!sessions[index].0, "第 {index} 輪重開");
        assert_eq!(calls[index]["system"], *world, "送出的 system 只有本輪內容");
        let prompt = calls[index]["prompt"].as_str().unwrap();
        assert!(
            !prompt.contains("## 設定更新") && !prompt.contains("燈塔"),
            "{prompt}"
        );
    }
    // 第四輪 system 沒變：續用第三輪 session、只送增量
    assert_eq!(sessions[3], (true, sessions[2].1.clone()));
    let resumed = calls[3]["prompt"].as_str().unwrap();
    assert!(
        resumed.contains("第4句") && !resumed.contains("第3句"),
        "{resumed}"
    );
    assert!(!resumed.starts_with("以下是到目前為止的對話紀錄："));
    // 被取代的舊 session 檔已刪，只剩現行那份
    assert!(!session_exists(&fake, &sessions[0].1));
    assert!(!session_exists(&fake, &sessions[1].1));
    assert!(session_exists(&fake, &sessions[2].1));

    let ledger = Ledger::read(fake.call.usage_log.as_deref().unwrap());
    let rows = ledger.calls();
    assert_eq!(rows.len(), 4);
    let reasons: Vec<&Value> = rows.iter().map(|row| &row["reason"]).collect();
    assert_eq!(reasons[0], "first-turn");
    assert_eq!(reasons[1], "system-changed");
    assert_eq!(reasons[2], "system-changed");
    assert!(reasons[3].is_null(), "續聊不記重開原因");
    for row in &rows {
        assert_eq!(row["lane"], "gm:sonnet");
        assert!(row.get("patched").is_none() && row.get("rebased").is_none());
    }
    assert_ne!(rows[0]["system_hash"], rows[1]["system_hash"]);
    assert_ne!(rows[1]["system_hash"], rows[2]["system_hash"]);
    assert_eq!(rows[2]["system_hash"], rows[3]["system_hash"]);
    assert!(ledger.0.iter().all(|row| row["reason"] != "cleanup-failed"));
    std::fs::remove_dir_all(&fake.dir).unwrap();
}

/// 續聊失敗降級重開：舊 claude session 檔同樣刪掉。
#[cfg(unix)]
#[tokio::test]
async fn gm_resume_failure_deletes_the_old_session_before_reopening() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_claude("gm-resume-fail");
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第一句")];
    narrate(&fake.call, &fake, &mut events, WORLD_A, "第二句")
        .await
        .unwrap();
    let mut failing = LaneCall {
        envs: fake.call.envs.clone(),
        program: fake.call.program.clone(),
        working_dir: fake.call.working_dir.clone(),
        prompt_dir: fake.call.prompt_dir.clone(),
        model: fake.call.model.clone(),
        usage_log: fake.call.usage_log.clone(),
        claude_home: fake.call.claude_home.clone(),
        provider: LaneProvider::Claude,
        on_overage: None,
    };
    failing
        .envs
        .push(("FAKE_FAIL_RESUME".to_owned(), "1".to_owned()));
    narrate(&failing, &fake, &mut events, WORLD_A, "第三句")
        .await
        .unwrap();
    let sessions: Vec<(bool, String)> = recorded_calls(&fake.session_dir)
        .iter()
        .map(session_flag)
        .collect();
    assert_eq!(sessions.len(), 3);
    assert_eq!(sessions[1], (true, sessions[0].1.clone()));
    assert!(!sessions[2].0);
    assert!(!session_exists(&fake, &sessions[0].1), "續聊失敗的舊檔已刪");
    assert!(session_exists(&fake, &sessions[2].1));
    let ledger = Ledger::read(fake.call.usage_log.as_deref().unwrap());
    assert!(ledger
        .calls()
        .iter()
        .any(|row| row["reason"] == "resume-failed"));
    std::fs::remove_dir_all(&fake.dir).unwrap();
}

/// 舊檔刪不掉：本輪照常完成，帳本記 cleanup-failed。
#[cfg(unix)]
#[tokio::test]
async fn gm_cleanup_failure_is_logged_without_failing_the_turn() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_claude("gm-cleanup-fail");
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第一句")];
    narrate(&fake.call, &fake, &mut events, WORLD_A, "第二句")
        .await
        .unwrap();
    let (_, old) = session_flag(&recorded_calls(&fake.session_dir)[0]);
    let path = session_file::session_file_path(&fake.claude_home, &fake.working_dir, &old);
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir_all(path.join("blocker")).unwrap(); // 換成目錄，remove_file 刪不掉
    narrate(&fake.call, &fake, &mut events, WORLD_B, "第三句")
        .await
        .unwrap();
    let ledger = Ledger::read(fake.call.usage_log.as_deref().unwrap());
    let drop = ledger
        .0
        .iter()
        .find(|row| row["reason"] == "cleanup-failed")
        .expect("記下 cleanup-failed");
    assert_eq!(drop["lane"], "gm:sonnet");
    std::fs::remove_dir_all(&fake.dir).unwrap();
}

/// 落檔失敗：本輪回錯，舊檔不刪（磁碟上的舊狀態仍指著它）。
#[cfg(unix)]
#[tokio::test]
async fn gm_store_write_failure_keeps_the_old_session() {
    use std::os::unix::fs::PermissionsExt;
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_claude("gm-store-fail");
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第一句")];
    narrate(&fake.call, &fake, &mut events, WORLD_A, "第二句")
        .await
        .unwrap();
    let (_, old) = session_flag(&recorded_calls(&fake.session_dir)[0]);
    let store = data::lanes_path(&fake.root, &fake.world_id).unwrap();
    let world_dir = store.parent().unwrap().to_path_buf();
    std::fs::set_permissions(&store, std::fs::Permissions::from_mode(0o444)).unwrap();
    std::fs::set_permissions(&world_dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let result = narrate(&fake.call, &fake, &mut events, WORLD_B, "第三句").await;
    std::fs::set_permissions(&world_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::set_permissions(&store, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(result.is_err());
    assert!(session_exists(&fake, &old), "落檔失敗不刪舊檔");
    assert_eq!(
        recorded_calls(&fake.session_dir).len(),
        1,
        "沒有送出新的一輪"
    );
    std::fs::remove_dir_all(&fake.dir).unwrap();
}

/// 走正式的 GM 組裝：world.md 含巨集但兩輪展開結果相同（`{{user}}`），凍結 system 逐字不變、照常續聊；
/// world.md 內容一換就重開。
#[test]
fn macro_with_identical_expansion_keeps_the_gm_lane() {
    let root = std::env::temp_dir().join(format!(
        "tt-gm-restart-macro-{}-{}",
        std::process::id(),
        ulid::Ulid::generate()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let world = data::create_world(&root, "燈塔").unwrap();
    data::write_world_md(&root, &world, "{{user}} 守著燈塔。").unwrap();
    let system_of = |events: &[TranscriptEvent]| {
        let mut materials = crate::chat_assembly::gm_materials(&root, &world).unwrap();
        materials.events = events.to_vec();
        let scan = crate::chat_assembly::test_gm_scan(&root, &world, &materials, "zh-TW");
        let (scope, _) = crate::chat_assembly::gm_scope(&materials);
        crate::chat_assembly::gm_lane_parts(&materials, &scan, &scope, "請推進劇情。", "zh-TW").0
    };
    let first_events = [event(TranscriptKind::Player, "", "阿濤", "第一句")];
    let first = system_of(&first_events);
    assert!(
        first.contains("守著燈塔") && !first.contains("{{user}}"),
        "{first}"
    );
    let second_events = [
        first_events[0].clone(),
        event(TranscriptKind::Narration, "", "", "夜色漸深。"),
        event(TranscriptKind::Player, "", "阿濤", "第二句"),
    ];
    let second = system_of(&second_events);
    assert_eq!(first, second);
    let state = gm_state(&first_events, &first);
    assert!(matches!(
        plan_turn(
            Some(&state),
            &gm_turn(&second_events, &second),
            1_001,
            LaneProvider::Claude
        ),
        TurnPlan::Resume { patch: None, .. }
    ));
    data::write_world_md(&root, &world, "{{user}} 離開了燈塔。").unwrap();
    let changed = system_of(&second_events);
    assert!(matches!(
        plan_turn(
            Some(&state),
            &gm_turn(&second_events, &changed),
            1_001,
            LaneProvider::Claude
        ),
        TurnPlan::Reopen {
            reason: ReopenReason::SystemChanged
        }
    ));
    std::fs::remove_dir_all(&root).unwrap();
}
