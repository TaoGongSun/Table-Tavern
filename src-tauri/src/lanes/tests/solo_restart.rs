//! claude 單角色線（不抹、私設在凍結 system）凍結 system 一變就整線重開（solo-private-dynamic-reopen）：
//! 不走補丁、不追平；被取代的舊 claude session 檔任何原因重開都刪。共線範圍不變。

use super::*;
use serde_json::Value;

const PLAIN: &str = "## 世界\n霧口鎮的旅店酒館。\n";
const PRIVATE_A: &str = "## 世界\n霧口鎮的旅店酒館。\n\n## 「狐狸」的私有設定\n其實是王子。\n";
const PRIVATE_B: &str = "## 世界\n霧口鎮的旅店酒館。\n\n## 「狐狸」的私有設定\n其實是公主。\n";
const LIVE: u64 = 1_001;
const EXPIRED: u64 = 1_000 + LEGACY_CACHE_TTL_SECS + 1;

/// 狐狸的一輪：`single` 時是單角色線（不抹），否則是共線。
fn fox_turn<'a>(
    events: &'a [TranscriptEvent],
    system: &str,
    single: bool,
    worldbook: Option<&str>,
) -> TurnInput<'a> {
    let mut input = turn_input(events, 0);
    input.frozen_system = system.to_owned();
    input.single_owner = single.then(|| "fox-id".to_owned());
    input.hoisted_worldbook = worldbook.map(str::to_owned);
    input.tail_tracked = !single;
    input
}

fn solo_state(events: &[TranscriptEvent], system: &str) -> LaneState {
    let mut state = lane_state(events, 0);
    state.snapshot = system.to_owned();
    state.applied = system.to_owned();
    state.unerased_owner = Some("fox-id".to_owned());
    state
}

fn reason(plan: TurnPlan) -> Option<&'static str> {
    match plan {
        TurnPlan::Reopen { reason } => Some(reason.as_str()),
        TurnPlan::Resume { .. } => None,
    }
}

#[test]
fn solo_plan_reopens_on_any_system_change() {
    let events = [event(TranscriptKind::Player, "", "阿濤", "你好")];
    let state = solo_state(&events, PRIVATE_A);
    for now in [LIVE, EXPIRED] {
        assert_eq!(
            reason(plan_turn(
                Some(&state),
                &fox_turn(&events, PRIVATE_B, true, None),
                now,
                LaneProvider::Claude,
                None
            )),
            Some("system-changed"),
            "@{now}：不走補丁、也不追平"
        );
    }
    // 升級前快取內補過丁、尚未追平的舊單人線：素材已追上也重開
    let mut patched = solo_state(&events, PRIVATE_B);
    patched.snapshot = PRIVATE_A.to_owned();
    assert_eq!(
        reason(plan_turn(
            Some(&patched),
            &fox_turn(&events, PRIVATE_B, true, None),
            LIVE,
            LaneProvider::Claude,
            None
        )),
        Some("system-changed")
    );
    // 世界書一起變：記較精確的 worldbook-changed
    assert_eq!(
        reason(plan_turn(
            Some(&state),
            &fox_turn(&events, PRIVATE_B, true, Some("## 世界書\n燈塔\n")),
            LIVE,
            LaneProvider::Claude,
            None
        )),
        Some("worldbook-changed")
    );
}

#[test]
fn solo_plan_resumes_when_system_is_unchanged() {
    let before = [event(TranscriptKind::Player, "", "阿濤", "你好")];
    let mut state = solo_state(&before, PRIVATE_A);
    state.expected_reply = Some(ExpectedReply {
        speaker_id: "fox-id".to_owned(),
        kind: TranscriptKind::Dialogue,
        text: "嗯。".to_owned(),
    });
    let events = [
        before[0].clone(),
        event(TranscriptKind::Dialogue, "fox-id", "狐狸", "嗯。"),
        event(TranscriptKind::Player, "", "阿濤", "然後呢"),
    ];
    for now in [LIVE, EXPIRED] {
        match plan_turn(
            Some(&state),
            &fox_turn(&events, PRIVATE_A, true, None),
            now,
            LaneProvider::Claude,
            None,
        ) {
            TurnPlan::Resume {
                session_id,
                base,
                system,
                patch,
                rebased,
            } => {
                assert_eq!(session_id, "sid-1");
                assert_eq!(base, 2);
                assert_eq!(system, PRIVATE_A);
                assert!(patch.is_none() && !rebased, "@{now}");
            }
            TurnPlan::Reopen { .. } => panic!("@{now}：system 沒變要續聊"),
        }
    }
}

/// 共線（每輪抹）不受影響：改卡照舊快取內補丁、過期追平。Agy 一角一線本來就一變就重開。
#[test]
fn erased_lane_still_patches_and_agy_still_reopens() {
    let events = [event(TranscriptKind::Player, "", "阿濤", "你好")];
    let mut shared = lane_state(&events, 0);
    shared.snapshot = PLAIN.to_owned();
    shared.applied = PLAIN.to_owned();
    let changed = format!("{PLAIN}\n## 新角色\n");
    match plan_turn(
        Some(&shared),
        &fox_turn(&events, &changed, false, None),
        LIVE,
        LaneProvider::Claude,
        None,
    ) {
        TurnPlan::Resume { patch, .. } => assert!(patch.is_some(), "共線快取內走補丁"),
        TurnPlan::Reopen { reason } => panic!("共線不該重開：{}", reason.as_str()),
    }
    match plan_turn(
        Some(&shared),
        &fox_turn(&events, &changed, false, None),
        EXPIRED,
        LaneProvider::Claude,
        None,
    ) {
        TurnPlan::Resume { rebased, .. } => assert!(rebased, "共線過期追平"),
        TurnPlan::Reopen { reason } => panic!("共線不該重開：{}", reason.as_str()),
    }
    let mut agy = lane_state(&events, 0);
    agy.provider = "agy".to_owned();
    agy.snapshot = PRIVATE_A.to_owned();
    agy.applied = PRIVATE_A.to_owned();
    let mut input = fox_turn(&events, PRIVATE_B, false, None);
    input.scope = Some("fox-id".to_owned());
    assert_eq!(
        reason(plan_turn(Some(&agy), &input, LIVE, LaneProvider::Agy, None)),
        Some("system-changed")
    );
}

fn calls(fake: &FakeCli) -> Vec<Value> {
    std::fs::read_to_string(fake.session_dir.join("calls.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// (是不是 resume, session id)
fn session_of(call: &Value) -> (bool, String) {
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

fn exists(fake: &FakeCli, session_id: &str) -> bool {
    session_file::session_file_path(&fake.claude_home, &fake.working_dir, session_id).exists()
}

/// 帳本呼叫行的重開原因（續聊為 null）
fn reasons(fake: &FakeCli) -> Vec<Value> {
    std::fs::read_to_string(fake.call.usage_log.as_deref().unwrap())
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|row| row.get("event").is_none())
        .map(|row| row["reason"].clone())
        .collect()
}

/// 狐狸跑一輪，回覆落成正典台詞並接一句玩家發言。
async fn speak(
    call: &LaneCall,
    fake: &FakeCli,
    events: &mut Vec<TranscriptEvent>,
    system: &str,
    single: bool,
    worldbook: Option<&str>,
) -> Result<(), String> {
    let reply = run_turn(
        call,
        &fake.root,
        &fake.world_id,
        fox_turn(events, system, single, worldbook),
        None,
        |_| {},
    )
    .await?
    .text;
    events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", &reply));
    events.push(event(TranscriptKind::Player, "", "阿濤", "然後呢"));
    Ok(())
}

fn start() -> Vec<TranscriptEvent> {
    vec![event(TranscriptKind::Player, "", "阿濤", "晚安")]
}

/// 單人線換私設三輪：每輪重開、不帶補丁、舊檔刪；第四輪 system 不變續用。帳本無 patched／rebased。
#[cfg(unix)]
#[tokio::test]
async fn solo_lane_reopens_each_private_change_and_deletes_the_old_session() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_claude("solo-private-rounds");
    let mut events = start();
    for system in [PRIVATE_A, PRIVATE_B, PLAIN, PLAIN] {
        speak(&fake.call, &fake, &mut events, system, true, None)
            .await
            .unwrap();
    }
    let calls = calls(&fake);
    let ids: Vec<(bool, String)> = calls.iter().map(session_of).collect();
    for (index, system) in [PRIVATE_A, PRIVATE_B, PLAIN].iter().enumerate() {
        assert!(!ids[index].0, "第 {index} 輪重開");
        assert_eq!(calls[index]["system"], *system);
        assert!(!calls[index]["prompt"].as_str().unwrap().contains("其實是"));
    }
    assert_eq!(ids[3], (true, ids[2].1.clone()), "system 不變續用");
    assert!(!exists(&fake, &ids[0].1) && !exists(&fake, &ids[1].1));
    assert!(exists(&fake, &ids[2].1));
    assert_eq!(
        reasons(&fake),
        vec![
            Value::from("first-turn"),
            Value::from("system-changed"),
            Value::from("system-changed"),
            Value::Null
        ]
    );
    let ledger = std::fs::read_to_string(fake.call.usage_log.as_deref().unwrap()).unwrap();
    assert!(!ledger.contains("\"patched\"") && !ledger.contains("\"rebased\""));
    assert!(!ledger.contains("cleanup-failed"));
    std::fs::remove_dir_all(&fake.dir).unwrap();
}

/// 共線→單人：私設實際進 system、開新線、被取代的舊共線檔刪掉、不帶補丁。
/// 有世界書時原因是 worldbook-changed（舊共線沒有世界書指紋），沒有世界書時是 system-changed。
#[cfg(unix)]
#[tokio::test]
async fn shared_to_solo_reopens_with_private_in_system() {
    let _serial = crate::inflight::lock_real_process_tests();
    for (tag, worldbook, expected) in [
        ("plain", None, "system-changed"),
        ("book", Some("## 世界書\n燈塔\n"), "worldbook-changed"),
    ] {
        let fake = fake_claude(&format!("shared-to-solo-{tag}"));
        let mut events = start();
        speak(&fake.call, &fake, &mut events, PLAIN, false, None)
            .await
            .unwrap();
        let solo_system = format!("{PRIVATE_A}{}", worldbook.unwrap_or_default());
        speak(
            &fake.call,
            &fake,
            &mut events,
            &solo_system,
            true,
            worldbook,
        )
        .await
        .unwrap();
        let calls = calls(&fake);
        let (_, shared) = session_of(&calls[0]);
        let (resumed, solo) = session_of(&calls[1]);
        assert!(!resumed && solo != shared, "{tag}：開新線");
        assert_eq!(calls[1]["system"], solo_system.as_str());
        assert!(!calls[1]["prompt"].as_str().unwrap().contains("其實是"));
        assert!(!exists(&fake, &shared), "{tag}：舊共線檔已刪");
        assert_eq!(reasons(&fake)[1], expected);
        std::fs::remove_dir_all(&fake.dir).unwrap();
    }
}

/// 單人線因 pending-rewrite、轉多人、換到別家 CLI（本輪 single_owner 歸零，靠舊線 owner）重開：舊檔都刪。
#[cfg(unix)]
#[tokio::test]
async fn solo_lane_deletes_the_old_session_for_other_reopen_reasons() {
    let _serial = crate::inflight::lock_real_process_tests();
    // pending-rewrite
    let fake = fake_claude("solo-pending");
    let mut events = start();
    speak(&fake.call, &fake, &mut events, PRIVATE_A, true, None)
        .await
        .unwrap();
    let path = data::lanes_path(&fake.root, &fake.world_id).unwrap();
    let mut store = read_store(&path);
    let old = {
        let state = store.values_mut().next().unwrap();
        state.pending_rewrite = Some(PendingRewrite {
            confidential: None,
            prefix: Some("狐狸：".to_owned()),
        });
        state.session_id.clone()
    };
    write_store(&path, &store).unwrap();
    speak(&fake.call, &fake, &mut events, PRIVATE_A, true, None)
        .await
        .unwrap();
    assert!(!exists(&fake, &old), "pending-rewrite：舊檔已刪");
    assert_eq!(reasons(&fake)[1], "pending-rewrite");
    std::fs::remove_dir_all(&fake.dir).unwrap();

    // 轉多人
    let fake = fake_claude("solo-to-multi");
    let mut events = start();
    speak(&fake.call, &fake, &mut events, PRIVATE_A, true, None)
        .await
        .unwrap();
    let (_, old) = session_of(&calls(&fake)[0]);
    speak(&fake.call, &fake, &mut events, PLAIN, false, None)
        .await
        .unwrap();
    assert!(!exists(&fake, &old), "mode-changed：舊檔已刪");
    assert_eq!(reasons(&fake)[1], "mode-changed");
    std::fs::remove_dir_all(&fake.dir).unwrap();

    // Claude → Agy（同一個線名）
    let claude = fake_claude("solo-to-agy");
    let agy = fake_agy("solo-from-claude");
    let mut events = start();
    speak(&claude.call, &claude, &mut events, PRIVATE_A, true, None)
        .await
        .unwrap();
    let (_, old) = session_of(&calls(&claude)[0]);
    let call = LaneCall {
        model: Some("sonnet".to_owned()), // 同一個線名，才會撞到 claude 開的線
        claude_home: claude.claude_home.clone(),
        working_dir: claude.working_dir.clone(),
        program: agy.call.program.clone(),
        prompt_dir: agy.call.prompt_dir.clone(),
        envs: agy.call.envs.clone(),
        usage_log: agy.call.usage_log.clone(),
        provider: LaneProvider::Agy,
        config_root: None,
        on_overage: None,
        effort: None,
    };
    let mut input = fox_turn(&events, PRIVATE_A, true, None);
    input.prefix = None;
    run_turn(&call, &claude.root, &claude.world_id, input, None, |_| {})
        .await
        .unwrap();
    assert!(!exists(&claude, &old), "provider-changed：舊 claude 檔已刪");
    let store = read_store(&data::lanes_path(&claude.root, &claude.world_id).unwrap());
    assert_eq!(store.values().next().unwrap().provider, "agy");
    std::fs::remove_dir_all(&claude.dir).unwrap();
    std::fs::remove_dir_all(&agy.dir).unwrap();
}

/// 續聊失敗降級重開：單人線舊檔刪掉。
#[cfg(unix)]
#[tokio::test]
async fn solo_resume_failure_deletes_the_old_session() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_claude("solo-resume-fail");
    let mut events = start();
    speak(&fake.call, &fake, &mut events, PRIVATE_A, true, None)
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
        config_root: None,
        on_overage: None,
        effort: None,
    };
    failing
        .envs
        .push(("FAKE_FAIL_RESUME".to_owned(), "1".to_owned()));
    speak(&failing, &fake, &mut events, PRIVATE_A, true, None)
        .await
        .unwrap();
    let ids: Vec<(bool, String)> = calls(&fake).iter().map(session_of).collect();
    assert_eq!(ids.len(), 3);
    assert_eq!(ids[1], (true, ids[0].1.clone()));
    assert!(!ids[2].0);
    assert!(!exists(&fake, &ids[0].1), "續聊失敗的舊檔已刪");
    assert!(exists(&fake, &ids[2].1));
    std::fs::remove_dir_all(&fake.dir).unwrap();
}

/// 舊檔刪不掉：本輪照常完成，帳本記 cleanup-failed。
#[cfg(unix)]
#[tokio::test]
async fn solo_cleanup_failure_is_logged_without_failing_the_turn() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_claude("solo-cleanup-fail");
    let mut events = start();
    speak(&fake.call, &fake, &mut events, PRIVATE_A, true, None)
        .await
        .unwrap();
    let (_, old) = session_of(&calls(&fake)[0]);
    let path = session_file::session_file_path(&fake.claude_home, &fake.working_dir, &old);
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir_all(path.join("blocker")).unwrap(); // 換成目錄，remove_file 刪不掉
    speak(&fake.call, &fake, &mut events, PRIVATE_B, true, None)
        .await
        .unwrap();
    let ledger = std::fs::read_to_string(fake.call.usage_log.as_deref().unwrap()).unwrap();
    assert!(ledger.contains("cleanup-failed"));
    assert_eq!(calls(&fake).len(), 2);
    std::fs::remove_dir_all(&fake.dir).unwrap();
}

/// 共線因已送段被改重開：舊檔照舊留著（範圍守住，只有單角色線與 GM 線任何原因都清）。
#[cfg(unix)]
#[tokio::test]
async fn shared_lane_reopen_still_keeps_the_old_session() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_claude("shared-keeps");
    let mut events = start();
    speak(&fake.call, &fake, &mut events, PLAIN, false, None)
        .await
        .unwrap();
    let (_, old) = session_of(&calls(&fake)[0]);
    events[0].text = "早安".to_owned();
    speak(&fake.call, &fake, &mut events, PLAIN, false, None)
        .await
        .unwrap();
    assert_eq!(reasons(&fake)[1], "history-edited");
    assert!(exists(&fake, &old), "共線舊檔不刪");
    std::fs::remove_dir_all(&fake.dir).unwrap();
}

/// 走正式的掃描＋組裝：私設含每輪必變的巨集（`{{lastMessageId}}`）時每輪都 system-changed；
/// 私設只有 `{{user}}` 時逐字不變、續用。
#[cfg(unix)]
#[tokio::test]
async fn dynamic_private_macro_reopens_every_turn_through_real_assembly() {
    use crate::transport::test_support::legacy::scan_of;
    let _serial = crate::inflight::lock_real_process_tests();
    let state_root = std::env::temp_dir().join(format!(
        "tt-solo-private-{}-{}",
        std::process::id(),
        ulid::Ulid::generate()
    ));
    std::fs::create_dir_all(&state_root).unwrap();
    let world = data::create_world(&state_root, "私設").unwrap();
    let state = data::read_state(&state_root, &world).unwrap();
    for (tag, private, expected) in [
        (
            "dynamic",
            "第 {{lastMessageId}} 則之後才說實話。",
            [
                Value::from("first-turn"),
                "system-changed".into(),
                "system-changed".into(),
            ],
        ),
        (
            "static",
            "其實欠了 {{user}} 一筆債。",
            [Value::from("first-turn"), Value::Null, Value::Null],
        ),
    ] {
        let fox = crate::lanes::scaffold_tests::card("fox-id", "狐狸", "狡猾。", private);
        let fake = fake_claude(&format!("solo-macro-{tag}"));
        let mut events = start();
        let mut systems = Vec::new();
        for _ in 0..3 {
            let scan = scan_of(
                &[],
                crate::world_scan::Viewer::Character(&fox),
                std::slice::from_ref(&fox),
                None,
                &events,
                "zh-TW",
            );
            let (frozen, turn) = crate::chat_assembly::character_lane_parts(
                &fox,
                std::slice::from_ref(&fox),
                None,
                &scan,
                &state,
                None,
                "zh-TW",
                true,
            );
            assert!(!frozen.contains("{{"), "{frozen}");
            speak(
                &fake.call,
                &fake,
                &mut events,
                &frozen,
                true,
                turn.hoisted_worldbook.as_deref(),
            )
            .await
            .unwrap();
            systems.push(frozen);
        }
        assert_eq!(reasons(&fake), expected.to_vec(), "{tag}");
        match tag {
            "dynamic" => assert!(systems[0] != systems[1] && systems[1] != systems[2]),
            _ => assert!(systems[0] == systems[1] && systems[1] == systems[2]),
        }
        std::fs::remove_dir_all(&fake.dir).unwrap();
    }
    let _ = std::fs::remove_dir_all(&state_root);
}
