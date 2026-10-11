//! 尾段世界書累積到門檻再重開（tail-worldbook-erase）：舊尾段先留在歷史吃快取，份數到 8、
//! 或至少 4 份且佔固定容量 15% 就以 `tail-worldbook` 重開壓平；舊 lanes.json 沒有計數就重開一次。

use super::*;
use crate::scene_budget::Unit;
use crate::world_scan::Budget;
use serde_json::Value;

const GM_HEADING: &str = "## 世界書（只進你的上下文）\n";
const SMALL_NOTE: &str = "## 世界書（只進你的上下文）\n夜裡的燈塔會說話。\n";
/// 夠大的容量：小條目的比例永遠不會先觸發
const ROOMY: Option<Budget> = Some(Budget {
    unit: Unit::Tokens,
    max: 1_000_000.0,
});

fn stale(copies: u32, tokens: u64) -> Option<TailStale> {
    Some(TailStale {
        copies,
        tokens,
        bytes: tokens * 3,
    })
}

fn gm_input<'a>(events: &'a [TranscriptEvent], capacity: Option<Budget>) -> TurnInput<'a> {
    let mut input = turn_input(events, 0);
    input.lane = Lane::Gm;
    input.prefix = None;
    input.echo = ReplyEcho::Narration;
    input.capacity = capacity;
    input
}

fn reason_of(plan: TurnPlan) -> Option<&'static str> {
    match plan {
        TurnPlan::Reopen { reason } => Some(reason.as_str()),
        TurnPlan::Resume { .. } => None,
    }
}

fn plan(
    state: &LaneState,
    input: &TurnInput<'_>,
    now: u64,
    provider: LaneProvider,
) -> Option<&'static str> {
    reason_of(plan_turn(Some(state), input, now, provider))
}

const LIVE: u64 = 1_001;
const EXPIRED: u64 = 1_000 + LEGACY_CACHE_TTL_SECS + 1;

#[test]
fn copy_cap_reopens_at_eight_on_every_resume_exit() {
    let events = [event(TranscriptKind::Player, "", "阿濤", "你好")];
    let mut state = lane_state(&events, 0);
    let cases: [(Lane, LaneProvider, u64); 5] = [
        (Lane::Gm, LaneProvider::Claude, LIVE),
        (Lane::Gm, LaneProvider::Grok, LIVE),
        (Lane::Chars, LaneProvider::Grok, LIVE),
        (Lane::Chars, LaneProvider::Claude, LIVE), // 補丁出口
        (Lane::Chars, LaneProvider::Claude, EXPIRED), // 追平出口
    ];
    for (lane, provider, now) in cases {
        state.provider = provider.as_str().to_owned();
        let mut input = match lane {
            Lane::Gm => gm_input(&events, ROOMY),
            Lane::Chars => turn_input(&events, 0),
        };
        input.capacity = ROOMY;
        if lane == Lane::Chars && provider == LaneProvider::Claude && now == LIVE {
            input.frozen_system = "凍結B".to_owned(); // 會走補丁的那條
        }
        state.tail_stale = stale(7, 70);
        assert_eq!(
            plan(&state, &input, now, provider),
            None,
            "{lane:?}/{provider:?}@{now}"
        );
        state.tail_stale = stale(8, 80);
        assert_eq!(
            plan(&state, &input, now, provider),
            Some("tail-worldbook"),
            "{lane:?}/{provider:?}@{now}"
        );
    }
}

#[test]
fn ratio_needs_four_copies_and_fifteen_percent_of_fixed_capacity() {
    let events = [event(TranscriptKind::Player, "", "阿濤", "你好")];
    let mut state = lane_state(&events, 0);
    let capacity = Some(Budget {
        unit: Unit::Tokens,
        max: 1_000.0,
    });
    let input = gm_input(&events, capacity);
    // 3 份即使佔一半也不觸發（間隔下限）
    state.tail_stale = stale(3, 500);
    assert_eq!(plan(&state, &input, LIVE, LaneProvider::Claude), None);
    state.tail_stale = stale(4, 149);
    assert_eq!(plan(&state, &input, LIVE, LaneProvider::Claude), None);
    state.tail_stale = stale(4, 150);
    assert_eq!(
        plan(&state, &input, LIVE, LaneProvider::Claude),
        Some("tail-worldbook")
    );
    // 容量未知：只看份數
    let unknown = gm_input(&events, None);
    state.tail_stale = stale(7, 1_000_000);
    assert_eq!(plan(&state, &unknown, LIVE, LaneProvider::Claude), None);
    // bytes 單位（Agy）照 bytes 算
    let bytes = gm_input(
        &events,
        Some(Budget {
            unit: Unit::Bytes,
            max: 1_000.0,
        }),
    );
    state.provider = "agy".to_owned();
    state.tail_stale = Some(TailStale {
        copies: 4,
        tokens: 1,
        bytes: 150,
    });
    assert_eq!(
        plan(&state, &bytes, LIVE, LaneProvider::Agy),
        Some("tail-worldbook")
    );
    state.tail_stale = Some(TailStale {
        copies: 4,
        tokens: 10_000,
        bytes: 149,
    });
    assert_eq!(plan(&state, &bytes, LIVE, LaneProvider::Agy), None);
}

/// 計數未知（升級前的舊線）：會帶尾段世界書的線重開一次；看線的形狀（`tail_tracked`），
/// `Hoist::All` 的線即使計數爆表或未知也照常續聊。
#[test]
fn unknown_count_reopens_only_tracked_lanes() {
    let events = [event(TranscriptKind::Player, "", "阿濤", "你好")];
    let mut state = lane_state(&events, 0);
    state.tail_stale = None;
    assert_eq!(
        plan(
            &state,
            &gm_input(&events, ROOMY),
            LIVE,
            LaneProvider::Claude
        ),
        Some("tail-worldbook")
    );
    assert_eq!(
        plan(&state, &turn_input(&events, 0), LIVE, LaneProvider::Claude),
        Some("tail-worldbook")
    );
    let mut untracked = turn_input(&events, 0);
    untracked.tail_tracked = false;
    assert_eq!(plan(&state, &untracked, LIVE, LaneProvider::Claude), None);
    state.tail_stale = stale(100, 1_000_000);
    assert_eq!(plan(&state, &untracked, LIVE, LaneProvider::Claude), None);
}

/// 既有重開原因與門檻同時成立：記原本那個原因。
#[test]
fn existing_reasons_win_over_the_threshold() {
    let before = [event(TranscriptKind::Player, "", "阿濤", "你好")];
    let mut base = lane_state(&before, 0);
    base.tail_stale = stale(8, 80);
    let events = [
        before[0].clone(),
        event(TranscriptKind::Player, "", "阿濤", "再一句"),
    ];
    let gm = gm_input(&events, ROOMY);
    let chars = turn_input(&events, 0);
    let check = |state: &LaneState, input: &TurnInput<'_>, provider, expected: &str| {
        assert_eq!(
            plan(state, input, LIVE, provider),
            Some(expected),
            "{expected}"
        );
    };
    check(&base, &chars, LaneProvider::Grok, "provider-changed");
    let mut pending = base.clone();
    pending.pending_rewrite = Some(PendingRewrite {
        confidential: None,
        prefix: None,
    });
    check(&pending, &chars, LaneProvider::Claude, "pending-rewrite");
    check(
        &base,
        &turn_input(&events, 1),
        LaneProvider::Claude,
        "scene-changed",
    );
    let mut ahead = base.clone();
    ahead.sent_events = 5;
    check(&ahead, &chars, LaneProvider::Claude, "history-rewound");
    let mut edited = base.clone();
    edited.sent_hash = "x".to_owned();
    check(&edited, &chars, LaneProvider::Claude, "history-edited");
    let mut diverged = base.clone();
    diverged.expected_reply = Some(ExpectedReply {
        speaker_id: "fox-id".to_owned(),
        kind: TranscriptKind::Dialogue,
        text: "不是這句".to_owned(),
    });
    check(&diverged, &chars, LaneProvider::Claude, "reply-diverged");
    let mut owned = base.clone();
    owned.unerased_owner = Some("knight-id".to_owned());
    check(&owned, &chars, LaneProvider::Claude, "owner-changed");
    owned.unerased_owner = Some("fox-id".to_owned());
    check(&owned, &chars, LaneProvider::Claude, "mode-changed");
    let mut solo = turn_input(&events, 0);
    solo.single_owner = Some("fox-id".to_owned());
    owned.had_state_block = true;
    check(&owned, &solo, LaneProvider::Claude, "state-block-gone");
    let mut hoisted = turn_input(&events, 0);
    hoisted.hoisted_worldbook = Some("新的世界書".to_owned());
    check(&base, &hoisted, LaneProvider::Claude, "worldbook-changed");
    let mut changed = gm_input(&events, ROOMY);
    changed.frozen_system = "凍結B".to_owned();
    check(&base, &changed, LaneProvider::Claude, "system-changed");
    let mut grok = base.clone();
    grok.provider = "grok".to_owned();
    check(&grok, &changed, LaneProvider::Grok, "system-changed");
    // 沒有其他原因：門檻；Claude 角色線快取過期也記門檻（不是追平）
    check(&base, &gm, LaneProvider::Claude, "tail-worldbook");
    assert_eq!(
        plan(&base, &chars, EXPIRED, LaneProvider::Claude),
        Some("tail-worldbook")
    );
}

/// session 檔 user 行裡某段標出現的次數（system 不在 jsonl 的 user 行，同名段標不會算進來）。
fn user_line_count(fake: &FakeCli, session_id: &str, marker: &str) -> usize {
    let path = session_file::session_file_path(&fake.claude_home, &fake.working_dir, session_id);
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|row| row["type"] == "user")
        .map(|row| {
            row.pointer("/message/content")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .matches(marker)
                .count()
        })
        .sum()
}

fn ledger_rows(fake: &FakeCli) -> Vec<Value> {
    std::fs::read_to_string(fake.call.usage_log.as_deref().unwrap())
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|row| row.get("event").is_none())
        .collect()
}

fn calls_of(fake: &FakeCli) -> Vec<Value> {
    std::fs::read_to_string(fake.session_dir.join("calls.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn session_of(call: &Value) -> String {
    let args: Vec<&str> = call["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|arg| arg.as_str().unwrap())
        .collect();
    let index = args
        .iter()
        .position(|arg| *arg == "--resume" || *arg == "--session-id")
        .unwrap();
    args[index + 1].to_owned()
}

fn stored(fake: &FakeCli) -> LaneState {
    read_store(&data::lanes_path(&fake.root, &fake.world_id).unwrap())
        .into_values()
        .next()
        .unwrap()
}

/// GM 一輪：尾段＝世界書段＋導演指示；回覆落成旁白、接下一句玩家發言。
async fn gm_round(
    call: &LaneCall,
    fake: &FakeCli,
    events: &mut Vec<TranscriptEvent>,
    worldbook: &str,
    capacity: Option<Budget>,
    round: usize,
) -> Result<(), String> {
    let mut input = gm_input(events, capacity);
    input.tail = format!("{worldbook}\n請推進劇情。");
    input.tail_worldbook = Some(worldbook.to_owned());
    let reply = run_turn(call, &fake.root, &fake.world_id, input, None, |_| {})
        .await?
        .text;
    events.push(event(TranscriptKind::Narration, "", "", &reply));
    events.push(event(
        TranscriptKind::Player,
        "",
        "阿濤",
        &format!("第{}句", round + 1),
    ));
    Ok(())
}

/// GM 線每輪同一條小作者註記跑 10 輪：第 9 輪 `tail-worldbook` 重開、其餘續聊；
/// 每輪結束後 session 的 user 行裡世界書段 ≤ 8 份；重開那輪只帶本輪一份、舊檔已刪；帳本記清理前的計數。
#[cfg(unix)]
#[tokio::test]
async fn gm_lane_reopens_at_the_copy_cap_and_keeps_history_bounded() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_claude("tail-cap");
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第1句")];
    for round in 1..=10 {
        gm_round(&fake.call, &fake, &mut events, SMALL_NOTE, ROOMY, round)
            .await
            .unwrap();
        let sid = session_of(calls_of(&fake).last().unwrap());
        let copies = user_line_count(&fake, &sid, GM_HEADING);
        assert!(copies <= 8, "第 {round} 輪後有 {copies} 份");
        assert_eq!(stored(&fake).tail_stale.unwrap().copies as usize, copies);
    }
    let calls = calls_of(&fake);
    let sessions: Vec<String> = calls.iter().map(session_of).collect();
    let rows = ledger_rows(&fake);
    assert_eq!(rows.len(), 10);
    for (index, row) in rows.iter().enumerate() {
        let expected = match index {
            0 => Value::from("first-turn"),
            8 => Value::from("tail-worldbook"),
            _ => Value::Null,
        };
        assert_eq!(row["reason"], expected, "第 {} 輪", index + 1);
    }
    // 清理前的計數照記：第 2–9 輪是 1–8，第 10 輪重開後只剩 1
    assert!(rows[0].get("stale_copies").is_none(), "首輪沒有前一條線");
    for index in 1..=8 {
        assert_eq!(rows[index]["stale_copies"], index as u64);
    }
    assert_eq!(rows[9]["stale_copies"], 1);
    assert!(rows[9]["stale_capacity"].as_u64().is_some());
    assert!(rows[9]["stale_ratio"].as_f64().is_some());
    let reopened = calls[8]["prompt"].as_str().unwrap();
    assert_eq!(reopened.matches(GM_HEADING).count(), 1);
    assert_ne!(sessions[8], sessions[7]);
    assert!(
        !session_file::session_file_path(&fake.claude_home, &fake.working_dir, &sessions[7])
            .exists()
    );
    assert!(ledger_rows(&fake)
        .iter()
        .all(|row| row["reason"] != "cleanup-failed"));
    std::fs::remove_dir_all(&fake.dir).unwrap();
}

/// 大條目（單份就超過容量 15%）跑 20 輪：靠最少 4 份，兩次重開至少隔 4 輪；容量未知時帳本比例記 null。
#[cfg(unix)]
#[tokio::test]
async fn large_entries_reopen_no_more_often_than_every_four_rounds() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_claude("tail-large");
    let big = format!("{GM_HEADING}{}\n", "龍".repeat(400));
    let capacity = Some(Budget {
        unit: Unit::Tokens,
        max: crate::scene_budget::budget_tokens(&big) as f64 * 2.0,
    });
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第1句")];
    for round in 1..=20 {
        gm_round(&fake.call, &fake, &mut events, &big, capacity, round)
            .await
            .unwrap();
    }
    let reopens: Vec<usize> = ledger_rows(&fake)
        .iter()
        .enumerate()
        .filter(|(_, row)| row["reason"] == "tail-worldbook")
        .map(|(index, _)| index + 1)
        .collect();
    assert_eq!(reopens, vec![5, 9, 13, 17]);

    // 容量未知：比例欄位記 null，不是 0%
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第1句")];
    let unknown = fake_claude("tail-unknown");
    for round in 1..=2 {
        gm_round(&unknown.call, &unknown, &mut events, &big, None, round)
            .await
            .unwrap();
    }
    let rows = ledger_rows(&unknown);
    assert_eq!(rows[1]["stale_copies"], 1);
    assert!(rows[1]["stale_capacity"].is_null() && rows[1]["stale_ratio"].is_null());
    std::fs::remove_dir_all(&fake.dir).unwrap();
    std::fs::remove_dir_all(&unknown.dir).unwrap();
}

/// 角色共線：公開段計數、機密段照舊每輪抹（大小不算進計數）；到門檻重開時舊 claude session 檔也刪。
#[cfg(unix)]
#[tokio::test]
async fn shared_lane_counts_only_the_public_segment_and_retires_old_session() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_claude("tail-chars");
    let public = "## 你知道的世界情報\n### 寶箱\n寶箱在地窖。\n";
    let secret = format!("## 狐狸的私有設定\n{}\n", "通緝犯".repeat(500));
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第1句")];
    for round in 1..=9 {
        let mut input = turn_input(&events, 0);
        input.tail = format!("{public}\n{secret}\n現在你是「狐狸」。");
        input.confidential = Some(secret.clone());
        input.tail_worldbook = Some(public.to_owned());
        input.capacity = ROOMY;
        let reply = run_turn(&fake.call, &fake.root, &fake.world_id, input, None, |_| {})
            .await
            .unwrap()
            .text;
        events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", &reply));
        events.push(event(
            TranscriptKind::Player,
            "",
            "阿濤",
            &format!("第{}句", round + 1),
        ));
        let sid = session_of(calls_of(&fake).last().unwrap());
        assert_eq!(user_line_count(&fake, &sid, "通緝犯"), 0, "機密段每輪都抹");
    }
    let rows = ledger_rows(&fake);
    assert_eq!(rows[8]["reason"], "tail-worldbook");
    assert_eq!(
        rows[8]["stale_size"],
        crate::scene_budget::budget_tokens(public) * 8,
        "只計公開段"
    );
    let sessions: Vec<String> = calls_of(&fake).iter().map(session_of).collect();
    assert!(
        !session_file::session_file_path(&fake.claude_home, &fake.working_dir, &sessions[7])
            .exists()
    );
    std::fs::remove_dir_all(&fake.dir).unwrap();
}

/// 續聊失敗降級重開那輪：計數只剩本輪一份。失控（比照中止）不累加。
#[cfg(unix)]
#[tokio::test]
async fn fallback_reopen_resets_and_runaway_does_not_count() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut fake = fake_claude("tail-fallback");
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第1句")];
    for round in 1..=3 {
        gm_round(&fake.call, &fake, &mut events, SMALL_NOTE, ROOMY, round)
            .await
            .unwrap();
    }
    assert_eq!(stored(&fake).tail_stale.unwrap().copies, 3);
    fake.call
        .envs
        .push(("FAKE_RUNAWAY_RESUME".to_owned(), "1".to_owned()));
    assert!(
        gm_round(&fake.call, &fake, &mut events, SMALL_NOTE, ROOMY, 4)
            .await
            .is_err()
    );
    assert_eq!(stored(&fake).tail_stale.unwrap().copies, 3, "失控不累加");
    fake.call.envs.pop();
    // pending 留著，下一輪重開：從零算本輪
    gm_round(&fake.call, &fake, &mut events, SMALL_NOTE, ROOMY, 5)
        .await
        .unwrap();
    assert_eq!(stored(&fake).tail_stale.unwrap().copies, 1);
    gm_round(&fake.call, &fake, &mut events, SMALL_NOTE, ROOMY, 6)
        .await
        .unwrap();
    assert_eq!(stored(&fake).tail_stale.unwrap().copies, 2);
    fake.call
        .envs
        .push(("FAKE_FAIL_RESUME".to_owned(), "1".to_owned()));
    gm_round(&fake.call, &fake, &mut events, SMALL_NOTE, ROOMY, 7)
        .await
        .unwrap();
    assert!(ledger_rows(&fake)
        .iter()
        .any(|row| row["reason"] == "resume-failed"));
    assert_eq!(
        stored(&fake).tail_stale.unwrap().copies,
        1,
        "降級重開只剩本輪"
    );
    std::fs::remove_dir_all(&fake.dir).unwrap();
}

/// 舊 lanes.json（沒有計數欄）：照讀不壞；會帶尾段世界書的線以 `tail-worldbook` 重開一次、之後照常續聊；
/// `tail_tracked` 為 false 的線照常續聊。
#[cfg(unix)]
#[tokio::test]
async fn legacy_store_without_counts_reopens_once() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_claude("tail-legacy");
    let store_path = data::lanes_path(&fake.root, &fake.world_id).unwrap();
    let strip = || {
        let mut store: Value =
            serde_json::from_str(&std::fs::read_to_string(&store_path).unwrap()).unwrap();
        for state in store.as_object_mut().unwrap().values_mut() {
            state.as_object_mut().unwrap().remove("tail_stale");
        }
        std::fs::write(&store_path, serde_json::to_string(&store).unwrap()).unwrap();
        assert!(stored(&fake).tail_stale.is_none(), "缺欄讀成未知");
    };
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第1句")];
    for round in 1..=3 {
        if round == 2 {
            strip();
        }
        gm_round(&fake.call, &fake, &mut events, SMALL_NOTE, ROOMY, round)
            .await
            .unwrap();
    }
    let reasons: Vec<Value> = ledger_rows(&fake)
        .iter()
        .map(|row| row["reason"].clone())
        .collect();
    assert_eq!(
        reasons,
        vec![
            Value::from("first-turn"),
            Value::from("tail-worldbook"),
            Value::Null
        ]
    );

    // 不帶尾段世界書的線：計數未知也照常續聊
    let other = fake_claude("tail-legacy-untracked");
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第1句")];
    for round in 1..=2 {
        if round == 2 {
            let path = data::lanes_path(&other.root, &other.world_id).unwrap();
            let mut store: Value =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            for state in store.as_object_mut().unwrap().values_mut() {
                state.as_object_mut().unwrap().remove("tail_stale");
            }
            std::fs::write(&path, serde_json::to_string(&store).unwrap()).unwrap();
        }
        let mut input = turn_input(&events, 0);
        input.tail_tracked = false;
        let reply = run_turn(
            &other.call,
            &other.root,
            &other.world_id,
            input,
            None,
            |_| {},
        )
        .await
        .unwrap()
        .text;
        events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", &reply));
        events.push(event(TranscriptKind::Player, "", "阿濤", "下一句"));
    }
    assert!(ledger_rows(&other)[1]["reason"].is_null());
    assert!(
        ledger_rows(&other)[1].get("stale_copies").is_none(),
        "不累計的線不寫無資訊的 stale 欄"
    );
    std::fs::remove_dir_all(&fake.dir).unwrap();
    std::fs::remove_dir_all(&other.dir).unwrap();
}

/// 到門檻重開時落檔失敗：回錯、沒送出新的一輪、舊檔不刪。刪舊檔失敗：記 cleanup-failed、本輪照常。
#[cfg(unix)]
#[tokio::test]
async fn threshold_reopen_store_and_cleanup_failures() {
    use std::os::unix::fs::PermissionsExt;
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_claude("tail-store-fail");
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第1句")];
    for round in 1..=8 {
        gm_round(&fake.call, &fake, &mut events, SMALL_NOTE, ROOMY, round)
            .await
            .unwrap();
    }
    let old = session_of(calls_of(&fake).last().unwrap());
    let store = data::lanes_path(&fake.root, &fake.world_id).unwrap();
    let world_dir = store.parent().unwrap().to_path_buf();
    std::fs::set_permissions(&store, std::fs::Permissions::from_mode(0o444)).unwrap();
    std::fs::set_permissions(&world_dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let result = gm_round(&fake.call, &fake, &mut events, SMALL_NOTE, ROOMY, 9).await;
    std::fs::set_permissions(&world_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::set_permissions(&store, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(result.is_err());
    assert_eq!(calls_of(&fake).len(), 8, "沒有送出新的一輪");
    let path = session_file::session_file_path(&fake.claude_home, &fake.working_dir, &old);
    assert!(path.exists(), "落檔失敗不刪舊檔");

    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir_all(path.join("blocker")).unwrap(); // 換成目錄，remove_file 刪不掉
    gm_round(&fake.call, &fake, &mut events, SMALL_NOTE, ROOMY, 9)
        .await
        .unwrap();
    let text = std::fs::read_to_string(fake.call.usage_log.as_deref().unwrap()).unwrap();
    assert!(text.contains("cleanup-failed"));
    assert!(text.contains("tail-worldbook"));
    std::fs::remove_dir_all(&fake.dir).unwrap();
}

/// Agy GM 線：同一套計數（容量以 bytes 計），到門檻開新對話；尾段世界書仍在 prompt 尾段、不進 system。
#[cfg(unix)]
#[tokio::test]
async fn agy_gm_lane_reopens_at_the_threshold_with_worldbook_in_tail() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_agy("tail");
    let capacity = Some(Budget {
        unit: Unit::Bytes,
        max: 190_000.0, // Agy 單則 body 上限（scene_budget::limits::AGY_BODY_BYTES）
    });
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第1句")];
    for round in 1..=9 {
        let mut input = gm_input(&events, capacity);
        input.tail = format!("{SMALL_NOTE}\n請推進劇情。");
        input.tail_worldbook = Some(SMALL_NOTE.to_owned());
        let reply = run_turn(&fake.call, &fake.root, &fake.world_id, input, None, |_| {})
            .await
            .unwrap()
            .text;
        events.push(event(TranscriptKind::Narration, "", "", &reply));
        events.push(event(
            TranscriptKind::Player,
            "",
            "阿濤",
            &format!("第{}句", round + 1),
        ));
    }
    let calls: Vec<Value> = std::fs::read_to_string(fake.dir.join("calls.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let resumed = |call: &Value| {
        call["args"]
            .as_array()
            .unwrap()
            .iter()
            .any(|arg| arg == "--conversation")
    };
    assert!(
        !resumed(&calls[0]) && !resumed(&calls[8]),
        "首輪與第 9 輪開新對話"
    );
    assert!((1..8).all(|index| resumed(&calls[index])));
    let reopened = calls[8]["prompt"].as_str().unwrap();
    assert_eq!(reopened.matches(GM_HEADING).count(), 1);
    assert!(reopened.trim_end().ends_with("請推進劇情。"));
    assert!(
        reopened.find(GM_HEADING).unwrap() > reopened.find("凍結A").unwrap(),
        "世界書在尾段、不在 system"
    );
    let rows = ledger_rows(&fake);
    assert_eq!(rows[8]["reason"], "tail-worldbook");
    assert_eq!(
        rows[8]["stale_size"],
        SMALL_NOTE.len() as u64 * 8,
        "bytes 計"
    );
    assert_eq!(
        stored(&fake).tail_stale.unwrap().copies,
        1,
        "重開後只剩本輪"
    );
    std::fs::remove_dir_all(&fake.dir).unwrap();
}

/// 玩家中止的回合、沒觸發尾段世界書的回合都不加份數。
#[cfg(unix)]
#[tokio::test]
async fn aborted_and_worldbook_free_rounds_do_not_count() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut fake = fake_claude("tail-abort");
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "第1句")];
    for round in 1..=2 {
        gm_round(&fake.call, &fake, &mut events, SMALL_NOTE, ROOMY, round)
            .await
            .unwrap();
    }
    assert_eq!(stored(&fake).tail_stale.unwrap().copies, 2);
    // 沒有尾段世界書的一輪：續聊、不加
    let mut input = gm_input(&events, ROOMY);
    input.tail = "請推進劇情。".to_owned();
    let reply = run_turn(&fake.call, &fake.root, &fake.world_id, input, None, |_| {})
        .await
        .unwrap()
        .text;
    events.push(event(TranscriptKind::Narration, "", "", &reply));
    events.push(event(TranscriptKind::Player, "", "阿濤", "第4句"));
    let state = stored(&fake);
    assert!(state.pending_rewrite.is_none());
    assert_eq!(state.tail_stale.unwrap().copies, 2, "沒世界書不加");
    // 玩家中止：不加
    fake.call
        .envs
        .push(("FAKE_HANG_RESUME".to_owned(), "1".to_owned()));
    let (guard, mut cancel) = crate::inflight::register_turn(&fake.world_id, "tail-abort");
    let mut input = gm_input(&events, ROOMY);
    input.tail = format!("{SMALL_NOTE}\n請推進劇情。");
    input.tail_worldbook = Some(SMALL_NOTE.to_owned());
    let turn = async {
        let _guard = guard;
        run_turn(
            &fake.call,
            &fake.root,
            &fake.world_id,
            input,
            Some(&mut cancel),
            |_| {},
        )
        .await
    };
    let marker = fake.session_dir.join("hanging");
    let abort = async {
        let step = std::time::Duration::from_millis(10);
        let mut waited = std::time::Duration::ZERO;
        while !marker.exists() {
            assert!(waited < std::time::Duration::from_secs(20));
            tokio::time::sleep(step).await;
            waited += step;
        }
        crate::inflight::abort_turn(&fake.world_id, "tail-abort");
    };
    let (outcome, ()) = tokio::join!(turn, abort);
    assert!(outcome.unwrap().aborted);
    assert_eq!(stored(&fake).tail_stale.unwrap().copies, 2, "中止不加");
    std::fs::remove_dir_all(&fake.dir).unwrap();
}
