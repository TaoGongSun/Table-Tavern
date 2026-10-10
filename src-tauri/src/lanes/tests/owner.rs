//! claude 單角色線不抹（claude-resume-tail-cache）：unerased_owner 的隔離、判定順序、
//! 失敗後重開、偵測 CLI 提醒。假 CLI 記下實際收到的 session id 與素材，斷言含未抹機密的
//! session 絕不再被別人或多角色模式 resume。

use super::*;
use serde_json::Value;

const SONNET: &str = "sonnet";

/// 一張測試桌：假 CLI＋正典事件（每輪成功後補上回覆與下一句玩家話，讓下輪對得上水位）。
struct Table {
    fake: FakeCli,
    events: Vec<TranscriptEvent>,
}

/// 一輪的素材。`secret` 是這輪該角色的機密（私設／限定條目／狀態）：
/// 單角色時照實送時的樣子留在 tail 不抹；多角色時同時當 confidential 回合後抹掉。
struct Turn<'a> {
    speaker: &'a str,
    name: &'a str,
    single: bool,
    has_state_block: bool,
    secret: &'a str,
    frozen: &'a str,
    /// 提進凍結 system 的世界書段（不抹尾段的線）
    worldbook: Option<&'a str>,
}

impl<'a> Turn<'a> {
    fn single(speaker: &'a str, name: &'a str, secret: &'a str) -> Self {
        Self {
            speaker,
            name,
            single: true,
            has_state_block: false,
            secret,
            frozen: "凍結",
            worldbook: None,
        }
    }

    fn multi(speaker: &'a str, name: &'a str, secret: &'a str) -> Self {
        Self {
            single: false,
            ..Self::single(speaker, name, secret)
        }
    }

    fn with_state(mut self, has_state_block: bool) -> Self {
        self.has_state_block = has_state_block;
        self
    }
}

impl Table {
    fn new(tag: &str) -> Self {
        Self {
            fake: fake_claude(&format!("owner-{tag}")),
            events: vec![event(TranscriptKind::Player, "", "阿濤", "晚安")],
        }
    }

    fn input(&self, turn: &Turn<'_>) -> TurnInput<'_> {
        TurnInput {
            lane: Lane::Chars,
            scene: 0,
            events: &self.events,
            lang: "zh-TW",
            frozen_system: turn.frozen.to_owned(),
            tail: format!("{}\n現在你是「{}」。", turn.secret, turn.name),
            confidential: (!turn.single).then(|| turn.secret.to_owned()),
            prefix: Some(format!("{}：", turn.name)),
            echo: ReplyEcho::Dialogue {
                speaker_id: turn.speaker.to_owned(),
                prefix: format!("{}：", turn.name),
            },
            scope: None,
            single_owner: turn.single.then(|| turn.speaker.to_owned()),
            has_state_block: turn.has_state_block,
            hoisted_worldbook: turn.worldbook.map(str::to_owned),
        }
    }

    async fn run(&mut self, turn: Turn<'_>) -> Result<String, String> {
        let outcome = run_turn(
            &self.fake.call,
            &self.fake.root,
            &self.fake.world_id,
            self.input(&turn),
            None,
            |_| {},
        )
        .await?;
        self.events.push(event(
            TranscriptKind::Dialogue,
            turn.speaker,
            turn.name,
            &outcome.text,
        ));
        self.events
            .push(event(TranscriptKind::Player, "", "阿濤", "然後呢"));
        Ok(outcome.text)
    }

    fn set_env(&mut self, key: &str, value: Option<&str>) {
        self.fake.call.envs.retain(|(name, _)| name != key);
        if let Some(value) = value {
            self.fake.call.envs.push((key.to_owned(), value.to_owned()));
        }
    }

    fn calls(&self) -> Vec<Value> {
        std::fs::read_to_string(self.fake.session_dir.join("calls.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn store(&self) -> LaneStore {
        read_store(&data::lanes_path(&self.fake.root, &self.fake.world_id).unwrap())
    }

    fn state(&self) -> Option<LaneState> {
        self.store()
            .get(&lane_key(Lane::Chars, self.fake.call.model_label(), None))
            .cloned()
    }
}

impl Drop for Table {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.fake.dir);
    }
}

/// (是不是 resume, session id)
fn session_of(call: &Value) -> (bool, String) {
    let args: Vec<String> = serde_json::from_value(call["args"].clone()).unwrap();
    let at = |flag: &str| {
        args.iter()
            .position(|arg| arg == flag)
            .map(|index| args[index + 1].clone())
    };
    match (at("--resume"), at("--session-id")) {
        (Some(id), None) => (true, id),
        (None, Some(id)) => (false, id),
        other => panic!("一次呼叫只能是開線或續聊其一：{other:?}"),
    }
}

fn sent_text(call: &Value) -> String {
    format!(
        "{}\n{}",
        call["system"].as_str().unwrap_or_default(),
        call["prompt"].as_str().unwrap_or_default()
    )
}

// ---------- 判定：plan_turn ----------

fn owned_state(events: &[TranscriptEvent], owner: Option<&str>) -> LaneState {
    let mut state = lane_state(events, 0);
    state.unerased_owner = owner.map(str::to_owned);
    state
}

fn speaking<'a>(
    events: &'a [TranscriptEvent],
    speaker: &str,
    single: bool,
    has_state_block: bool,
) -> TurnInput<'a> {
    let mut input = turn_input(events, 0);
    input.echo = ReplyEcho::Dialogue {
        speaker_id: speaker.to_owned(),
        prefix: "狐狸：".to_owned(),
    };
    input.single_owner = single.then(|| speaker.to_owned());
    input.has_state_block = has_state_block;
    input
}

fn reopen_reason(plan: TurnPlan) -> Option<&'static str> {
    match plan {
        TurnPlan::Reopen { reason } => Some(reason.as_str()),
        TurnPlan::Resume { .. } => None,
    }
}

#[test]
fn plan_turn_owner_rules_in_fixed_order() {
    let events = [event(TranscriptKind::Player, "", "阿濤", "晚安")];
    let claude = LaneProvider::Claude;
    let plan = |state: &LaneState, input: &TurnInput<'_>| {
        reopen_reason(plan_turn(Some(state), input, 1_010, claude))
    };
    let fox = owned_state(&events, Some("fox-id"));
    // 同一人、單角色 → 續用
    assert_eq!(plan(&fox, &speaking(&events, "fox-id", true, false)), None);
    // 換人（含別的角色解析到同一模型而共用同一 key）
    assert_eq!(
        plan(&fox, &speaking(&events, "knight-id", true, false)),
        Some("owner-changed")
    );
    assert_eq!(
        plan(&fox, &speaking(&events, "knight-id", false, false)),
        Some("owner-changed")
    );
    // 同一人但在場變多
    assert_eq!(
        plan(&fox, &speaking(&events, "fox-id", false, false)),
        Some("mode-changed")
    );
    // 狀態區塊從有到無：不被「同一人續用」短路
    let mut with_block = fox.clone();
    with_block.had_state_block = true;
    assert_eq!(
        plan(&with_block, &speaking(&events, "fox-id", true, false)),
        Some("state-block-gone")
    );
    assert_eq!(
        plan(&with_block, &speaking(&events, "fox-id", true, true)),
        None
    );
    // owner 為 None：多角色、單角色都照原規則續用
    let erased = owned_state(&events, None);
    assert_eq!(
        plan(&erased, &speaking(&events, "fox-id", false, false)),
        None
    );
    assert_eq!(
        plan(&erased, &speaking(&events, "knight-id", true, false)),
        None
    );
    // 既有條件優先：pending 先於 owner 判定
    let mut pending = fox.clone();
    pending.pending_rewrite = Some(PendingRewrite {
        confidential: None,
        prefix: None,
    });
    assert_eq!(
        plan(&pending, &speaking(&events, "knight-id", false, false)),
        Some("pending-rewrite")
    );
}

/// 私設變動走現成補丁（起點：已傳達的素材就含舊私設）。對假 CLI 實收的 system／prompt 逐字比對：
/// TTL 內替換、TTL 內移除都續用舊 system＋補丁；TTL 外整份追平；更新後換角開新線。
#[cfg(unix)]
#[tokio::test]
async fn private_changes_patch_within_ttl_rebase_after_and_never_cross_speakers() {
    let _serial = crate::inflight::lock_real_process_tests();
    const BASE: &str = "## 世界\n霧口鎮的旅店酒館。\n";
    const OLD: &str = "## 世界\n霧口鎮的旅店酒館。\n\n## 「狐狸」的私有設定\n其實是王子。\n";
    const REPLACED: &str = "## 世界\n霧口鎮的旅店酒館。\n\n## 「狐狸」的私有設定\n其實是公主。\n";
    const KNIGHT: &str = "## 世界\n霧口鎮的旅店酒館。\n\n## 「騎士」的私有設定\n在追捕通緝犯。\n";
    let fox = |frozen: &'static str| Turn {
        frozen,
        ..Turn::single("fox-id", "狐狸", "")
    };
    let patch_of = |applied: &str, current: &str| {
        super::super::snapshot_patch::render_patch(applied, current, "zh-TW").unwrap()
    };
    let prompt_of = |call: &Value| call["prompt"].as_str().unwrap().to_owned();
    let system_of = |call: &Value| call["system"].as_str().unwrap().to_owned();
    let mut table = Table::new("private-patch");

    // 開線：已傳達的素材＝含舊私設
    table.run(fox(OLD)).await.unwrap();
    let opened = session_of(&table.calls()[0]).1;
    let state = table.state().unwrap();
    assert_eq!(state.snapshot, OLD);
    assert_eq!(state.applied, OLD);

    // TTL 內私設替換：續用、system 仍是舊字串、prompt 帶「舊→新」補丁
    table.run(fox(REPLACED)).await.unwrap();
    let replaced = table.calls()[1].clone();
    let replaced_patch = patch_of(OLD, REPLACED);
    assert!(replaced_patch.contains("其實是公主。"));
    assert_eq!(session_of(&replaced), (true, opened.clone()), "替換：續用");
    assert_eq!(system_of(&replaced), OLD, "替換：TTL 內 system 不換");
    assert!(
        prompt_of(&replaced).contains(&format!("——\n{replaced_patch}\n\n")),
        "替換：補丁原文接在本輪 tail 前"
    );
    assert_eq!(table.state().unwrap().applied, REPLACED);

    // TTL 內私設移除：續用、system 仍是舊字串、prompt 帶「替換後→移除」補丁
    table.run(fox(BASE)).await.unwrap();
    let removed = table.calls()[2].clone();
    let removed_patch = patch_of(REPLACED, BASE);
    assert_ne!(removed_patch, replaced_patch);
    assert_eq!(session_of(&removed), (true, opened.clone()), "移除：續用");
    assert_eq!(system_of(&removed), OLD, "移除：TTL 內 system 不換");
    assert!(
        prompt_of(&removed).contains(&format!("——\n{removed_patch}\n\n")),
        "移除：補丁原文接在本輪 tail 前"
    );
    assert_eq!(table.state().unwrap().applied, BASE);

    // TTL 外：整份追平，system 換成最新素材，不帶補丁
    let path = data::lanes_path(&table.fake.root, &table.fake.world_id).unwrap();
    let mut store = read_store(&path);
    for state in store.values_mut() {
        state.last_call_epoch = now_epoch() - state.cache_ttl_secs - 60;
    }
    write_store(&path, &store).unwrap();
    table.run(fox(BASE)).await.unwrap();
    let rebased = table.calls()[3].clone();
    assert_eq!(
        session_of(&rebased),
        (true, opened.clone()),
        "TTL 外：續用不重開"
    );
    assert_eq!(system_of(&rebased), BASE, "TTL 外：system 換成最新");
    let rebased_prompt = prompt_of(&rebased);
    assert!(
        !rebased_prompt.contains(&replaced_patch),
        "TTL 外：不帶補丁"
    );
    assert!(!rebased_prompt.contains(&removed_patch), "TTL 外：不帶補丁");
    assert!(!rebased_prompt.contains("其實是"));

    // 更新後換角：owner-changed 開新線，system 只有騎士素材，沒有狐狸的私設或補丁
    table
        .run(Turn {
            frozen: KNIGHT,
            ..Turn::single("knight-id", "騎士", "")
        })
        .await
        .unwrap();
    let swapped = table.calls()[4].clone();
    let (resumed, id) = session_of(&swapped);
    assert!(!resumed, "換角：開新線");
    assert_ne!(id, opened);
    assert_eq!(system_of(&swapped), KNIGHT, "換角：system 只有騎士素材");
    assert!(
        !sent_text(&swapped).contains("其實是"),
        "換角：沒有狐狸的私設或補丁"
    );
    let state = table.state().unwrap();
    assert_eq!(state.unerased_owner.as_deref(), Some("knight-id"));
    assert_eq!(state.applied, KNIGHT);
}

/// 舊 lanes.json 沒有新欄位：整份照讀，claude 角色線讀成「照舊每輪抹」。
#[test]
fn legacy_store_without_owner_fields_still_loads() {
    let dir = std::env::temp_dir().join(format!("tt-lanes-owner-legacy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let legacy = |provider: &str| {
        serde_json::json!({
            "session_id": "sid", "scene": 0, "sent_events": 0, "sent_hash": "h",
            "snapshot": "s", "applied": "s", "pending_rewrite": null, "expected_reply": null,
            "last_call_epoch": 1, "provider": provider, "model": "m"
        })
    };
    let store = serde_json::json!({
        "chars:sonnet": legacy("claude"),
        "gm:sonnet": legacy("claude"),
        "chars:gemini:fox-id": legacy("agy"),
        "chars:grok-4": legacy("grok"),
    });
    let path = dir.join("lanes.json");
    std::fs::write(&path, store.to_string()).unwrap();
    let read = read_store(&path);
    assert_eq!(read.len(), 4);
    for state in read.values() {
        assert_eq!(state.unerased_owner, None);
        assert!(!state.had_state_block);
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn chars_lane_shape_matches_the_plan_table() {
    let shape = |provider, sole| {
        let s = chars_lane_shape(provider, sole);
        (
            s.hoist_private,
            s.erase,
            s.prefix,
            s.scope_by_card,
            s.single_owner,
        )
    };
    for sole in [true, false] {
        assert_eq!(
            shape(LaneProvider::Agy, sole),
            (true, false, false, true, false)
        );
        assert_eq!(
            shape(LaneProvider::Grok, sole),
            (false, true, true, false, false)
        );
    }
    assert_eq!(
        shape(LaneProvider::Claude, true),
        (true, false, true, false, true)
    );
    assert_eq!(
        shape(LaneProvider::Claude, false),
        (false, true, true, false, false)
    );
}

fn meta(id: &str, name: &str, archived: bool, auto_hidden: bool) -> data::CharacterMeta {
    data::CharacterMeta {
        id: id.to_owned(),
        name: name.to_owned(),
        color: String::new(),
        avatar: String::new(),
        tier: data::Tier::Balanced,
        show_image: true,
        archived,
        auto_hidden,
        display_index: None,
    }
}

#[test]
fn sole_presence_counts_arrivals_and_leans_to_multi() {
    use crate::chat_assembly::sole_present;
    let arrival = |name: &str| {
        let mut arrived = event(TranscriptKind::System, "system", "系統", name);
        arrived.marker = Some(data::EventMarker::CardArrival {
            name: name.to_owned(),
        });
        arrived
    };
    let fox = meta("fox", "狐狸", false, false);
    // 單卡、零卡、開口者不在 cards 裡：只有開口者本人
    assert!(sole_present(std::slice::from_ref(&fox), "fox", &[]));
    assert!(sole_present(&[], "fox", &[]));
    assert!(sole_present(
        &[meta("gone", "舊人", true, false)],
        "fox",
        &[]
    ));
    assert!(!sole_present(std::slice::from_ref(&fox), "stranger", &[]));
    // auto_hidden：沒回歸不算、本幕回歸就算
    let hidden = meta("knight", "騎士", false, true);
    let cards = [fox.clone(), hidden];
    assert!(sole_present(&cards, "fox", &[]));
    assert!(!sole_present(&cards, "fox", &[arrival("騎士")]));
    // 同名不同 id：名字比對多算，偏向多角色
    let twin = [fox.clone(), meta("fox-2", "狐狸", false, true)];
    assert!(!sole_present(&twin, "fox", &[arrival("狐狸")]));
    // 兩張都在場
    assert!(!sole_present(
        &[fox.clone(), meta("knight", "騎士", false, false)],
        "fox",
        &[]
    ));
    // 世界書掃描的「單角色桌」用同一套：本幕回歸的隱藏卡算進去
    use crate::chat_assembly::sole_card_name;
    assert_eq!(sole_card_name(&cards, &[]).as_deref(), Some("狐狸"));
    assert_eq!(sole_card_name(&cards, &[arrival("騎士")]), None);
    assert_eq!(sole_card_name(&[], &[]), None);
}

// ---------- 假 CLI：實際送出什麼 ----------

/// 單角色連兩輪續用；換人開新 session，新素材沒有前一個角色的機密；之後絕不 resume 舊線。
#[cfg(unix)]
#[tokio::test]
async fn other_speaker_never_resumes_an_unerased_session() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut table = Table::new("swap");
    table
        .run(Turn::single("fox-id", "狐狸", "狐狸私設甲"))
        .await
        .unwrap();
    table
        .run(Turn::single("fox-id", "狐狸", "狐狸私設甲"))
        .await
        .unwrap();
    assert_eq!(
        table.state().unwrap().unerased_owner.as_deref(),
        Some("fox-id")
    );
    // 第二個角色解析到同一模型，掛在同一個 key 上
    table
        .run(Turn::single("knight-id", "騎士", "騎士私設乙"))
        .await
        .unwrap();
    table
        .run(Turn::multi("fox-id", "狐狸", "狐狸私設甲"))
        .await
        .unwrap();
    let calls = table.calls();
    let sessions: Vec<_> = calls.iter().map(session_of).collect();
    let fox_id = &sessions[0].1;
    assert_eq!(sessions[0], (false, fox_id.clone()));
    assert_eq!(sessions[1], (true, fox_id.clone()));
    assert!(!sessions[2].0, "換人開新 session");
    assert_ne!(&sessions[2].1, fox_id);
    assert!(!sent_text(&calls[2]).contains("狐狸私設甲"));
    // 騎士的未抹線遇到狐狸（多角色）也開新的，騎士的機密不在新素材
    assert!(!sessions[3].0);
    assert!(!sent_text(&calls[3]).contains("騎士私設乙"));
    for (resumed, id) in &sessions[2..] {
        assert!(!(*resumed && id == fox_id), "狐狸未抹線不得再被 resume");
    }
    for call in &calls {
        assert_eq!(
            call["reminder_env"],
            Value::Null,
            "LaneCall.envs 沒帶時不該憑空出現"
        );
    }
}

/// 同一人從單角色變多角色：開新 session；新 stdin 合法含本輪機密，但沒有舊輪殘留。
/// 之後抹乾淨的多角色線換角續用是合法的。
#[cfg(unix)]
#[tokio::test]
async fn single_to_multi_reopens_without_history_residue() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut table = Table::new("to-multi");
    table
        .run(Turn::single("fox-id", "狐狸", "狐狸舊狀態HP80"))
        .await
        .unwrap();
    table
        .run(Turn::multi("fox-id", "狐狸", "狐狸新狀態HP70"))
        .await
        .unwrap();
    assert_eq!(table.state().unwrap().unerased_owner, None);
    table
        .run(Turn::multi("knight-id", "騎士", "騎士私設乙"))
        .await
        .unwrap();
    let calls = table.calls();
    let sessions: Vec<_> = calls.iter().map(session_of).collect();
    assert!(!sessions[1].0, "單角色→多角色要開新 session");
    assert_ne!(sessions[1].1, sessions[0].1);
    let second = sent_text(&calls[1]);
    assert!(
        second.contains("狐狸新狀態HP70"),
        "本輪機密照送（回合後抹）"
    );
    assert!(
        !second.contains("狐狸舊狀態HP80"),
        "舊輪未抹內容不可帶進新線"
    );
    assert_eq!(
        sessions[2],
        (true, sessions[1].1.clone()),
        "抹乾淨的共線換角續用"
    );
}

/// owner 為 None 的多角色線照舊 Resume 並寫 None；接著單角色續用、呼叫前就記下新 owner。
#[cfg(unix)]
#[tokio::test]
async fn erased_lane_resumes_and_single_turn_claims_owner_before_call() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut table = Table::new("claim");
    table
        .run(Turn::multi("fox-id", "狐狸", "狐狸私設甲"))
        .await
        .unwrap();
    table
        .run(Turn::multi("knight-id", "騎士", "騎士私設乙"))
        .await
        .unwrap();
    assert_eq!(table.state().unwrap().unerased_owner, None);
    // 呼叫前就落檔：CLI 卡在半途時讀 store，owner 與 pending 都已寫好
    table.set_env("FAKE_HANG_RESUME", Some("1"));
    let world_id = table.fake.world_id.clone();
    let marker = table.fake.session_dir.join("hanging");
    let store_path = data::lanes_path(&table.fake.root, &world_id).unwrap();
    let (guard, mut cancel) = crate::inflight::register_turn(&world_id, "owner-claim");
    let turn = async {
        let _guard = guard;
        run_turn(
            &table.fake.call,
            &table.fake.root,
            &table.fake.world_id,
            table.input(&Turn::single("knight-id", "騎士", "騎士私設乙").with_state(true)),
            Some(&mut cancel),
            |_| {},
        )
        .await
    };
    let inspect = async {
        let step = std::time::Duration::from_millis(10);
        let mut waited = std::time::Duration::ZERO;
        while !marker.exists() {
            assert!(waited < std::time::Duration::from_secs(20));
            tokio::time::sleep(step).await;
            waited += step;
        }
        let mid = read_store(&store_path)
            .get(&lane_key(Lane::Chars, SONNET, None))
            .cloned()
            .unwrap();
        crate::inflight::abort_turn(&world_id, "owner-claim");
        mid
    };
    let (outcome, mid) = tokio::join!(turn, inspect);
    assert!(outcome.unwrap().aborted);
    assert_eq!(
        mid.unerased_owner.as_deref(),
        Some("knight-id"),
        "呼叫前就記下新 owner"
    );
    assert!(mid.had_state_block);
    assert!(mid.pending_rewrite.is_some(), "崩潰保護照舊");
    table.set_env("FAKE_HANG_RESUME", None);
    let calls = table.calls();
    let sessions: Vec<_> = calls.iter().map(session_of).collect();
    assert_eq!(
        sessions[1],
        (true, sessions[0].1.clone()),
        "None＋多角色→Resume"
    );
    assert_eq!(
        sessions[2],
        (true, sessions[0].1.clone()),
        "None＋單角色→Resume"
    );

    // 成功版：None＋單角色續用後 owner 是騎士
    let mut table = Table::new("claim-ok");
    table
        .run(Turn::multi("fox-id", "狐狸", "狐狸私設甲"))
        .await
        .unwrap();
    table
        .run(Turn::single("knight-id", "騎士", "騎士私設乙").with_state(true))
        .await
        .unwrap();
    let state = table.state().unwrap();
    assert_eq!(state.unerased_owner.as_deref(), Some("knight-id"));
    assert!(state.had_state_block);
    let sessions: Vec<_> = table.calls().iter().map(session_of).collect();
    assert_eq!(sessions[1], (true, sessions[0].1.clone()));
}

/// 同一 owner 在各種失敗之後，下一輪一律是新的 --session-id。
#[cfg(unix)]
#[tokio::test]
async fn same_owner_reopens_after_every_failure() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fox = || Turn::single("fox-id", "狐狸", "狐狸私設甲");
    let last_two = |table: &Table| {
        let sessions: Vec<_> = table.calls().iter().map(session_of).collect();
        let n = sessions.len();
        (sessions[n - 2].clone(), sessions[n - 1].clone())
    };

    // runaway（續聊那輪失控）
    let mut table = Table::new("fail-runaway");
    table.run(fox()).await.unwrap();
    table.set_env("FAKE_RUNAWAY_RESUME", Some("1"));
    assert!(table.run(fox()).await.is_err());
    table.set_env("FAKE_RUNAWAY_RESUME", None);
    table
        .events
        .push(event(TranscriptKind::Player, "", "阿濤", "再說一次"));
    table.run(fox()).await.unwrap();
    let (failed, next) = last_two(&table);
    assert!(failed.0);
    assert!(!next.0, "runaway 後開新 session");
    assert_ne!(next.1, failed.1);

    // 開線失敗
    let mut table = Table::new("fail-open");
    table.set_env("FAKE_FAIL_OPEN", Some("1"));
    assert!(table.run(fox()).await.is_err());
    table.set_env("FAKE_FAIL_OPEN", None);
    table.run(fox()).await.unwrap();
    let (failed, next) = last_two(&table);
    assert!(!next.0);
    assert_ne!(next.1, failed.1);

    // 前綴改寫失敗（session 檔不在）：丟線，下一輪開新
    let mut table = Table::new("fail-prefix");
    table.set_env("FAKE_NO_SESSION", Some("1"));
    table.run(fox()).await.unwrap();
    assert!(table.state().is_none(), "抹寫／前綴失敗照原本丟線");
    table.set_env("FAKE_NO_SESSION", None);
    table.run(fox()).await.unwrap();
    let (failed, next) = last_two(&table);
    assert!(!next.0);
    assert_ne!(next.1, failed.1);

    // resume 失敗：同一輪降級開新
    let mut table = Table::new("fail-resume");
    table.run(fox()).await.unwrap();
    table.set_env("FAKE_FAIL_RESUME", Some("1"));
    table.run(fox()).await.unwrap();
    let (failed, next) = last_two(&table);
    assert!(failed.0);
    assert!(!next.0);
    assert_ne!(next.1, failed.1);
    assert_eq!(
        table.state().unwrap().unerased_owner.as_deref(),
        Some("fox-id")
    );

    // 留著 pending 重啟：同一人與換人都開新
    for speaker in ["fox-id", "knight-id"] {
        let mut table = Table::new(&format!("pending-{speaker}"));
        table.run(fox()).await.unwrap();
        let path = data::lanes_path(&table.fake.root, &table.fake.world_id).unwrap();
        let mut store = read_store(&path);
        for state in store.values_mut() {
            state.pending_rewrite = Some(PendingRewrite {
                confidential: None,
                prefix: Some("狐狸：".to_owned()),
            });
        }
        write_store(&path, &store).unwrap();
        let name = if speaker == "fox-id" {
            "狐狸"
        } else {
            "騎士"
        };
        table
            .run(Turn::single(speaker, name, "本輪機密"))
            .await
            .unwrap();
        let (first, next) = last_two(&table);
        assert!(!next.0, "{speaker}：pending 後開新");
        assert_ne!(next.1, first.1);
        if speaker != "fox-id" {
            assert!(!sent_text(&table.calls()[1]).contains("狐狸私設甲"));
        }
    }
}

/// 中止：留 pending，下一輪同一人也開新 session。
#[cfg(unix)]
#[tokio::test]
async fn same_owner_reopens_after_abort() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut table = Table::new("abort");
    table
        .run(Turn::single("fox-id", "狐狸", "狐狸私設甲"))
        .await
        .unwrap();
    table.set_env("FAKE_HANG", Some("1"));
    let world_id = table.fake.world_id.clone();
    let marker = table.fake.session_dir.join("hanging");
    let (guard, mut cancel) = crate::inflight::register_turn(&world_id, "owner-abort");
    let turn = async {
        let _guard = guard;
        run_turn(
            &table.fake.call,
            &table.fake.root,
            &table.fake.world_id,
            table.input(&Turn::single("fox-id", "狐狸", "狐狸私設甲")),
            Some(&mut cancel),
            |_| {},
        )
        .await
    };
    let abort = async {
        let step = std::time::Duration::from_millis(10);
        let mut waited = std::time::Duration::ZERO;
        while !marker.exists() {
            assert!(waited < std::time::Duration::from_secs(20));
            tokio::time::sleep(step).await;
            waited += step;
        }
        crate::inflight::abort_turn(&world_id, "owner-abort");
    };
    let (outcome, ()) = tokio::join!(turn, abort);
    assert!(outcome.unwrap().aborted);
    table.set_env("FAKE_HANG", None);
    table
        .events
        .push(event(TranscriptKind::Player, "", "阿濤", "剛剛說到哪"));
    table
        .run(Turn::single("fox-id", "狐狸", "狐狸私設甲"))
        .await
        .unwrap();
    let sessions: Vec<_> = table.calls().iter().map(session_of).collect();
    let aborted = &sessions[1];
    let next = &sessions[2];
    assert!(aborted.0);
    assert!(!next.0, "中止後開新 session");
    assert_ne!(next.1, aborted.1);
}

/// Sonnet 留下未抹線 → 換 Opus 進多角色 → 切回 Sonnet 仍是多角色：舊 Sonnet 線不得 resume。
#[cfg(unix)]
#[tokio::test]
async fn returning_to_an_old_model_lane_in_multi_mode_reopens() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut table = Table::new("models");
    table
        .run(Turn::single("fox-id", "狐狸", "狐狸私設甲"))
        .await
        .unwrap();
    table.fake.call.model = Some("opus".to_owned());
    table
        .run(Turn::multi("fox-id", "狐狸", "狐狸私設甲"))
        .await
        .unwrap();
    table.fake.call.model = Some(SONNET.to_owned());
    table
        .run(Turn::multi("fox-id", "狐狸", "狐狸私設甲"))
        .await
        .unwrap();
    let sessions: Vec<_> = table.calls().iter().map(session_of).collect();
    let sonnet_id = &sessions[0].1;
    assert!(!sessions[2].0, "切回 Sonnet 多角色要開新 session");
    assert_ne!(&sessions[2].1, sonnet_id);
    assert!(sessions
        .iter()
        .skip(1)
        .all(|(resumed, id)| !(*resumed && id == sonnet_id)));
}

/// 用真的狀態資料組 tail 跑一輪單角色（騎士）。
async fn run_knight(
    table: &mut Table,
    knight: &crate::data::CharacterCard,
    turn: crate::transport::LaneTurn,
) {
    let mut input = table.input(&Turn::single(&knight.id, &knight.name, ""));
    input.tail = turn.tail;
    input.has_state_block = turn.has_state_block;
    let reply = run_turn(
        &table.fake.call,
        &table.fake.root,
        &table.fake.world_id,
        input,
        None,
        |_| {},
    )
    .await
    .unwrap()
    .text;
    table.events.push(event(
        TranscriptKind::Dialogue,
        &knight.id,
        &knight.name,
        &reply,
    ));
}

/// 狀態區塊整塊消失（用真的狀態資料組 tail）：實際開新 id；欄位部分刪除則照常續用。
#[cfg(unix)]
#[tokio::test]
async fn state_block_from_real_state_reopens_only_when_whole_block_vanishes() {
    use crate::data::StateNode;
    let _serial = crate::inflight::lock_real_process_tests();
    let fixture = super::super::scaffold_tests::fixture();
    let knight = fixture.cards[0].clone();
    let branch = vec!["Heroes".to_owned(), knight.name.clone()];
    let lane_turn = |state: &crate::data::TableState| {
        crate::transport::test_support::legacy::chars_lane_turn(
            &knight,
            Some(&fixture.player),
            &fixture.events,
            &fixture.worldbook,
            state,
            &fixture.incremental,
            Some(&branch),
            "zh-TW",
            true,
        )
    };
    let mut table = Table::new("state-block");
    // 第 1 輪：完整狀態，新標題在
    let full = lane_turn(&fixture.state);
    assert!(full.has_state_block);
    assert!(full
        .tail
        .contains("以本區為準，取代先前對話中所有同名狀態區塊"));
    run_knight(&mut table, &knight, full).await;
    // 第 2 輪：刪掉 Mood 欄位，區塊仍在 → 續用，新區塊不含已刪欄位
    let mut partial = fixture.state.clone();
    if let Some(StateNode::Branch(heroes)) = partial.tree.get_mut("Heroes") {
        if let Some(StateNode::Branch(fields)) = heroes.get_mut(&knight.name) {
            fields.remove("Mood");
        }
    }
    partial.changes.clear();
    let trimmed = lane_turn(&partial);
    assert!(trimmed.has_state_block);
    assert!(trimmed.tail.contains("Affection"));
    assert!(!trimmed.tail.contains("Mood"));
    run_knight(&mut table, &knight, trimmed).await;
    // 第 3 輪：分支整個清空 → 實際開新 id
    let mut cleared = partial.clone();
    if let Some(StateNode::Branch(heroes)) = cleared.tree.get_mut("Heroes") {
        heroes.insert(knight.name.clone(), StateNode::Branch(Default::default()));
    }
    let gone = lane_turn(&cleared);
    assert!(!gone.has_state_block);
    run_knight(&mut table, &knight, gone).await;
    let sessions: Vec<_> = table.calls().iter().map(session_of).collect();
    assert_eq!(sessions[1], (true, sessions[0].1.clone()), "部分刪除續用");
    assert!(!sessions[2].0, "整塊消失開新 session");
    assert_ne!(sessions[2].1, sessions[0].1);
}

/// 解除綁定（沒有分支 → 區塊從有到無）：開新 id。
#[cfg(unix)]
#[tokio::test]
async fn unbinding_the_branch_reopens() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fixture = super::super::scaffold_tests::fixture();
    let knight = fixture.cards[0].clone();
    let branch = vec!["Heroes".to_owned(), knight.name.clone()];
    let lane_turn = |branch: Option<&[String]>| {
        crate::transport::test_support::legacy::chars_lane_turn(
            &knight,
            Some(&fixture.player),
            &fixture.events,
            &fixture.worldbook,
            &fixture.state,
            &fixture.incremental,
            branch,
            "zh-TW",
            true,
        )
    };
    let mut table = Table::new("unbind");
    run_knight(&mut table, &knight, lane_turn(Some(&branch))).await;
    let unbound = lane_turn(None);
    assert!(!unbound.has_state_block);
    run_knight(&mut table, &knight, unbound).await;
    let sessions: Vec<_> = table.calls().iter().map(session_of).collect();
    assert!(!sessions[1].0, "解除綁定後開新 session");
    assert_ne!(sessions[1].1, sessions[0].1);
}

/// agy 一角一線：就算呼叫端填了 single_owner，也不套未抹線判定（狀態消失、在場變多都照常續用）。
#[cfg(unix)]
#[tokio::test]
async fn agy_lane_ignores_owner_rules() {
    let _serial = crate::inflight::lock_real_process_tests();
    let fake = fake_agy("owner");
    let mut events = vec![event(TranscriptKind::Player, "", "阿濤", "晚安")];
    fn input(events: &[TranscriptEvent], single: bool, has_state_block: bool) -> TurnInput<'_> {
        let mut input = turn_input(events, 0);
        input.prefix = None;
        input.scope = Some("fox-id".to_owned());
        input.single_owner = single.then(|| "fox-id".to_owned());
        input.has_state_block = has_state_block;
        input
    }
    for (single, has_state_block) in [(true, true), (true, false), (false, false)] {
        let reply = run_turn(
            &fake.call,
            &fake.root,
            &fake.world_id,
            input(&events, single, has_state_block),
            None,
            |_| {},
        )
        .await
        .unwrap()
        .text;
        events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", &reply));
        events.push(event(TranscriptKind::Player, "", "阿濤", "然後呢"));
    }
    let store = read_store(&data::lanes_path(&fake.root, &fake.world_id).unwrap());
    let state = store.values().next().unwrap();
    assert_eq!(state.unerased_owner, None);
    assert!(!state.had_state_block);
    let calls = std::fs::read_to_string(fake.dir.join("calls.jsonl")).unwrap();
    let resumed = calls
        .lines()
        .filter(|line| line.contains("--conversation"))
        .count();
    assert_eq!(resumed, 2, "第 2、3 輪都續用");
    std::fs::remove_dir_all(&fake.dir).unwrap();
}

// ---------- 偵測與環境變數 ----------

fn diag_lines(table: &Table) -> Vec<Value> {
    let log = table.fake.call.usage_log.clone().unwrap();
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|line| line["event"] == "cli-reminder-seen")
        .collect()
}

/// 角色線：共用抹寫已載入的內容認出提醒，同一 session 只記一次；報表不把它算成一輪。
#[cfg(unix)]
#[tokio::test]
async fn reminder_in_session_is_noted_once_and_report_skips_it() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut table = Table::new("reminder");
    table.set_env("FAKE_REMINDER", Some("1"));
    table
        .run(Turn::single("fox-id", "狐狸", "狐狸私設甲"))
        .await
        .unwrap();
    table
        .run(Turn::single("fox-id", "狐狸", "狐狸私設甲"))
        .await
        .unwrap();
    let diags = diag_lines(&table);
    assert_eq!(diags.len(), 1, "同一 session 只記一次");
    assert_eq!(diags[0]["reason"], "seen");
    assert!(diags[0].get("model").is_none());
    let log = std::fs::read_to_string(table.fake.call.usage_log.clone().unwrap()).unwrap();
    let world = table.fake.world_id.clone();
    let report = crate::usage::report::summarize(
        &log,
        Some(&world),
        &[(world.clone(), "桌".to_owned())],
        &[],
    );
    let rounds = report
        .worlds
        .iter()
        .find(|option| option.id == world)
        .unwrap()
        .rounds;
    let real_rounds = log
        .lines()
        .filter(|line| line.contains("\"model\""))
        .count() as u64;
    assert_eq!(rounds, real_rounds, "診斷事件不加輪數");
    assert!(report.latest.unwrap().event.is_none(), "不佔最近一輪");
    assert_eq!(report.events, 0);
}

/// GM 線沒有抹寫：另做只讀掃描，讀不到只記診斷、不丟線；角色線抹寫失敗照原本丟線、不被偵測吞掉。
#[cfg(unix)]
#[tokio::test]
async fn scan_failure_only_diagnoses_on_gm_but_rewrite_failure_still_drops() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut table = Table::new("scan-fail");
    table.set_env("FAKE_NO_SESSION", Some("1"));
    let mut gm = turn_input(&table.events, 0);
    gm.lane = Lane::Gm;
    gm.prefix = None;
    gm.echo = ReplyEcho::Narration;
    run_turn(
        &table.fake.call,
        &table.fake.root,
        &table.fake.world_id,
        gm,
        None,
        |_| {},
    )
    .await
    .unwrap();
    let gm_key = lane_key(Lane::Gm, SONNET, None);
    assert!(table.store().contains_key(&gm_key), "GM 掃描失敗不丟線");
    let diags = diag_lines(&table);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0]["reason"], "scan-failed");

    table
        .run(Turn::single("fox-id", "狐狸", "狐狸私設甲"))
        .await
        .unwrap();
    assert!(table.state().is_none(), "角色線載入失敗照原本丟線");
    assert_eq!(diag_lines(&table).len(), 1, "抹寫失敗不改記成診斷");
}

#[test]
fn reminder_matching_is_exact_and_scan_is_read_only() {
    let dir = std::env::temp_dir().join(format!("tt-lanes-owner-scan-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mention = r#"{"type":"user","uuid":"u1","parentUuid":null,"message":{"role":"user","content":"total_tokens_reminder 只是一般文字"}}"#;
    let wrong_level = r#"{"type":"total_tokens_reminder","uuid":"x"}"#;
    let real = r#"{"type":"attachment","uuid":"a1","parentUuid":"u1","attachment":{"type":"total_tokens_reminder","text":"<total_tokens>1 tokens left</total_tokens>"}}"#;
    assert!(!session_file::text_has_total_tokens_reminder(&format!(
        "{mention}\n{wrong_level}\nnot json\n"
    )));
    let text = format!("{mention}\n{real}\n");
    assert!(session_file::text_has_total_tokens_reminder(&text));
    let parsed = session_file::parse(&format!("{mention}\n")).unwrap();
    assert!(!session_file::has_total_tokens_reminder(&parsed));
    let parsed = session_file::parse(&text).unwrap();
    assert!(session_file::has_total_tokens_reminder(&parsed));

    let fake = fake_claude("owner-scan-bytes");
    let path = session_file::session_file_path(&fake.claude_home, &fake.working_dir, "sid-scan");
    std::fs::write(&path, &text).unwrap();
    assert_eq!(scan_reminder_readonly(&fake.call, "sid-scan"), Ok(true));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        text,
        "掃描不改位元組"
    );
    assert!(scan_reminder_readonly(&fake.call, "missing").is_err());
    std::fs::remove_dir_all(&fake.dir).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}

/// 共用 envs 帶上關提醒的變數，子程序實際收到 off。
#[cfg(unix)]
#[tokio::test]
async fn child_process_receives_reminder_off() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut table = Table::new("env");
    table
        .fake
        .call
        .envs
        .extend(crate::transport::dispatch::claude_cli_envs(
            &data::AppConfig::default(),
        ));
    table
        .run(Turn::single("fox-id", "狐狸", "狐狸私設甲"))
        .await
        .unwrap();
    assert_eq!(table.calls()[0]["reminder_env"], "off");
}

/// GM 線與角色線不共用 session。
#[cfg(unix)]
#[tokio::test]
async fn gm_and_character_lanes_never_share_sessions() {
    let _serial = crate::inflight::lock_real_process_tests();
    let mut table = Table::new("gm-chars");
    table
        .run(Turn::single("fox-id", "狐狸", "狐狸私設甲"))
        .await
        .unwrap();
    let mut gm = turn_input(&table.events, 0);
    gm.lane = Lane::Gm;
    gm.prefix = None;
    gm.echo = ReplyEcho::Narration;
    gm.single_owner = Some("fox-id".to_owned()); // 填了也會歸零
    run_turn(
        &table.fake.call,
        &table.fake.root,
        &table.fake.world_id,
        gm,
        None,
        |_| {},
    )
    .await
    .unwrap();
    let sessions: Vec<_> = table.calls().iter().map(session_of).collect();
    assert!(!sessions[1].0);
    assert_ne!(sessions[1].1, sessions[0].1);
    let gm_state = table
        .store()
        .get(&lane_key(Lane::Gm, SONNET, None))
        .cloned()
        .unwrap();
    assert_eq!(gm_state.unerased_owner, None);
    assert!(!sent_text(&table.calls()[1]).contains("狐狸私設甲"));
}

/// 不抹尾段的線把本輪世界書提進 system（worldbook-st-trigger-parity 三之 3）：世界書一變就整線重開、不走補丁
/// （補丁回合後不抹，舊世界書會留在 session 歷史裡）；連續三輪換世界書，每輪送出的只有本輪那份。
/// 世界書沒變、只有別的素材變時照舊續用。
#[cfg(unix)]
#[tokio::test]
async fn hoisted_worldbook_change_reopens_instead_of_patching() {
    let _serial = crate::inflight::lock_real_process_tests();
    let books = [
        "## 世界書\n燈塔一\n",
        "## 世界書\n燈塔二\n",
        "## 世界書\n燈塔三\n",
    ];
    let frozen: Vec<String> = books.iter().map(|book| format!("凍結\n{book}")).collect();
    let mut table = Table::new("hoisted-worldbook");
    for (index, book) in books.iter().enumerate() {
        table
            .run(Turn {
                frozen: &frozen[index],
                worldbook: Some(book),
                ..Turn::single("fox-id", "狐狸", "")
            })
            .await
            .unwrap();
    }
    let calls = table.calls();
    let ids: Vec<(bool, String)> = calls.iter().map(session_of).collect();
    assert!(
        ids.iter().all(|(resumed, _)| !resumed),
        "每輪都重開：{ids:?}"
    );
    for (index, call) in calls.iter().enumerate() {
        let sent = sent_text(call);
        for (other, book) in books.iter().enumerate() {
            assert_eq!(sent.contains(book), other == index, "第 {index} 輪：{sent}");
        }
    }
    // 世界書沒變、凍結素材別處變了：照舊續用（claude 快取內走補丁）
    table
        .run(Turn {
            frozen: &format!("{}另一段\n", frozen[2]),
            worldbook: Some(books[2]),
            ..Turn::single("fox-id", "狐狸", "")
        })
        .await
        .unwrap();
    assert_eq!(session_of(&table.calls()[3]), (true, ids[2].1.clone()));
}

/// 走正式的掃描＋組裝：單人 Claude 線把共用快照的常駐條目改掉、再刪掉，都要重開（指紋涵蓋 system 裡全部的世界書），
/// 不能走補丁把舊世界書留在不抹的歷史裡。
#[cfg(unix)]
#[tokio::test]
async fn editing_or_deleting_a_snapshot_entry_reopens_the_unerased_lane() {
    use crate::transport::test_support::legacy::scan_of;
    let _serial = crate::inflight::lock_real_process_tests();
    let fox = crate::lanes::scaffold_tests::card("fox-id", "狐狸", "狡猾。", "其實是公主。");
    let entry = |content: &str| crate::data::WorldbookEntry {
        uid: 1,
        title: "王國".to_owned(),
        keys: Vec::new(),
        content: content.to_owned(),
        constant: true,
        order: 0,
        disabled: false,
        visibility: crate::data::Visibility::Public,
        is_person: false,
        locked: false,
    };
    let books = [vec![entry("王國舊設定")], vec![entry("王國新設定")], vec![]];
    let state_root = table_state_root();
    let world = crate::data::create_world(&state_root, "快照").unwrap();
    let state = crate::data::read_state(&state_root, &world).unwrap();
    let mut table = Table::new("snapshot-edit");
    for book in &books {
        let scan = scan_of(
            book,
            crate::world_scan::Viewer::Character(&fox),
            std::slice::from_ref(&fox),
            None,
            &table.events,
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
        table
            .run(Turn {
                frozen: &frozen,
                worldbook: turn.hoisted_worldbook.as_deref(),
                ..Turn::single("fox-id", "狐狸", "")
            })
            .await
            .unwrap();
    }
    let calls = table.calls();
    assert!(calls.iter().all(|call| !session_of(call).0), "每輪都重開");
    assert!(!sent_text(&calls[1]).contains("王國舊設定"));
    assert!(!sent_text(&calls[2]).contains("王國"));
    let _ = std::fs::remove_dir_all(&state_root);
}

fn table_state_root() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "table-tavern-owner-state-{}-{}",
        std::process::id(),
        ulid::Ulid::generate()
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}
