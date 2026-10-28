//! fixture 照 grok 1.0.46 實際落檔的形狀手寫（2026-10-06 實驗擷取），不含憑證。
//! grok 1.0.70 另見回合尾提醒（2026-10-28 新 grok-home 第一通實測形狀）。

use super::*;
use serde_json::json;

const SID: &str = "0f60d654-b226-4fd1-a551-8bd629150c81";
const PREFIX: &str = "狐狸：";
/// 含換行與引號：JSONL 裡是 `\n`、`\"` 跳脫，位元組層找不到全文
const SECRET: &str = "## 「狐狸」的私有設定\n暗號是「藍\"鳶尾」七七三一\n\n";

struct Fixture {
    home: PathBuf,
    dir: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.home);
    }
}

fn update(kind: &str, extra: Value) -> Value {
    let mut update = json!({ "sessionUpdate": kind });
    if let (Some(target), Some(extra)) = (update.as_object_mut(), extra.as_object()) {
        target.extend(extra.clone());
    }
    json!({ "timestamp": 1_791_299_432u64, "method": "session/update",
            "params": { "sessionId": SID, "update": update } })
}

fn text_update(kind: &str, text: &str) -> Value {
    update(kind, json!({ "content": { "type": "text", "text": text } }))
}

struct Turn {
    prompt: String,
    reply_chunks: Vec<&'static str>,
    chat_extra: Vec<Value>,
    updates_extra: Vec<Value>,
}

fn turn(confidential: Option<&str>) -> Turn {
    Turn {
        prompt: format!(
            "玩家：狐狸，你今晚怎麼這麼安靜？\n\n——\n{}現在你是「狐狸」。",
            confidential.unwrap_or_default()
        ),
        reply_chunks: vec!["狐狸把酒杯轉了半圈，", "「今晚話少些。」"],
        chat_extra: Vec::new(),
        updates_extra: Vec::new(),
    }
}

fn reply_of(turn: &Turn) -> String {
    turn.reply_chunks.concat()
}

fn write_jsonl(path: &Path, rows: &[Value]) {
    let text: String = rows
        .iter()
        .map(|row| serde_json::to_string(row).unwrap() + "\n")
        .collect();
    fs::write(path, text).unwrap();
}

fn fixture(tag: &str, turn: &Turn) -> Fixture {
    let home = std::env::temp_dir().join(format!("tt-grok-session-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&home);
    let dir = home
        .join("sessions")
        .join("%2Ftmp%2Fcli-workspace")
        .join(SID);
    fs::create_dir_all(&dir).unwrap();
    let reply = reply_of(turn);
    let mut chat = vec![
        json!({ "type": "system", "content": "你是多角色扮演引擎。" }),
        json!({ "type": "user", "content": [{ "type": "text", "text": "<user_info>\nOS Version: macos\n</user_info>" }] }),
        json!({ "type": "user", "content": [{ "type": "text", "text": "<system-reminder>skills</system-reminder>" }],
                "synthetic_reason": "system_reminder" }),
        json!({ "type": "user", "content": [{ "type": "text", "text": format!("<user_query>\n{}\n</user_query>", turn.prompt) }],
                "prompt_index": 0 }),
        // 摘要為空、只剩 encrypted_content 的 reasoning：updates 不會有 thought chunk
        json!({ "type": "reasoning", "id": "rs_1", "summary": [], "encrypted_content": "bsd7eIRA", "status": "completed" }),
    ];
    chat.extend(turn.chat_extra.clone());
    chat.push(json!({ "type": "assistant", "content": reply, "model_id": "grok-4.6-build", "reasoning_effort": "low" }));
    write_jsonl(&dir.join("chat_history.jsonl"), &chat);

    let mut updates = vec![
        text_update("user_message_chunk", &turn.prompt),
        text_update("agent_thought_chunk", "正在以「狐狸」身份扮演。"),
    ];
    for chunk in &turn.reply_chunks {
        updates.push(text_update("agent_message_chunk", chunk));
    }
    updates.extend(turn.updates_extra.clone());
    updates.push(update(
        "turn_completed",
        json!({ "stop_reason": "end_turn" }),
    ));
    updates.push(update("background_tasks", json!({ "tasks": [] })));
    write_jsonl(&dir.join("updates.jsonl"), &updates);

    for name in [
        "chat_history.jsonl.lock",
        "updates.jsonl.lock",
        "summary.json.lock",
    ] {
        fs::write(dir.join(name), "").unwrap();
    }
    fs::write(
        dir.join("signals.json"),
        r#"{"turnCount":1,"compactionCount":0}"#,
    )
    .unwrap();
    fs::write(
        dir.join("summary.json"),
        r#"{"session_summary":"Roleplay fox"}"#,
    )
    .unwrap();
    fs::write(dir.join("system_prompt.txt"), "你是多角色扮演引擎。").unwrap();
    Fixture { home, dir }
}

fn read_rows(path: &Path) -> Vec<Value> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn run(fixture: &Fixture, confidential: Option<&str>, reply: &str) -> Result<(), RewriteError> {
    let cleaned = reply.strip_prefix(PREFIX).unwrap_or(reply).trim();
    rewrite(&fixture.home, SID, confidential, PREFIX, reply, cleaned)
}

fn expect_drop(fixture: &Fixture, confidential: Option<&str>, reply: &str, reason: DropReason) {
    expect_drop_error(fixture, confidential, reply, reason);
}

fn expect_drop_error(
    fixture: &Fixture,
    confidential: Option<&str>,
    reply: &str,
    reason: DropReason,
) -> RewriteError {
    let error = run(fixture, confidential, reply).expect_err("形狀不符必須丟線");
    assert_eq!(error.reason, reason, "{}", error.detail);
    error
}

/// 回覆帶控制區塊：兩檔的本輪回覆整段換成「前綴＋收尾台詞」，updates 只留第一個回覆 chunk。
#[test]
fn control_blocks_are_rewritten_out_of_both_files() {
    let mut turn = turn(Some(SECRET));
    turn.reply_chunks = vec![
        "「今晚話少些。」",
        "<UpdateVariable>",
        "_.set('錢包', 3);</UpdateVariable>",
    ];
    let cleaned_fixture = fixture("cleaned", &turn);
    let reply = reply_of(&turn);
    rewrite(
        &cleaned_fixture.home,
        SID,
        Some(SECRET),
        PREFIX,
        &reply,
        "「今晚話少些。」",
    )
    .unwrap();

    let chat = read_rows(&cleaned_fixture.dir.join("chat_history.jsonl"));
    assert_eq!(
        chat.last().unwrap()["content"],
        json!("狐狸：「今晚話少些。」")
    );
    let updates = read_rows(&cleaned_fixture.dir.join("updates.jsonl"));
    let replies: Vec<&Value> = updates
        .iter()
        .filter(|row| update_kind(row) == Some("agent_message_chunk"))
        .collect();
    assert_eq!(replies.len(), 1);
    assert_eq!(
        replies[0]["params"]["update"]["content"]["text"],
        json!("狐狸：「今晚話少些。」")
    );
    assert!(!value_contains(
        &Value::Array(updates.clone()),
        "UpdateVariable"
    ));
    assert!(!value_contains(&Value::Array(chat), SECRET));

    // 核對仍用原文：檔內回覆不是本輪原文就丟線
    let mismatch = fixture("cleaned-mismatch", &turn);
    let error = rewrite(
        &mismatch.home,
        SID,
        Some(SECRET),
        PREFIX,
        "別的話<status>a</status>",
        "別的話",
    )
    .expect_err("對不上必須丟線");
    assert_eq!(error.reason, DropReason::RewriteFailed, "{}", error.detail);
}

#[test]
fn erases_segment_strips_reasoning_and_prefixes_both_files() {
    let turn = turn(Some(SECRET));
    let fixture = fixture("ok", &turn);
    let system_raw = fs::read_to_string(fixture.dir.join("chat_history.jsonl"))
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .to_owned();
    run(&fixture, Some(SECRET), &reply_of(&turn)).unwrap();

    let chat = read_rows(&fixture.dir.join("chat_history.jsonl"));
    assert!(chat.iter().all(|row| row["type"] != "reasoning"));
    assert!(!value_contains(&Value::Array(chat.clone()), SECRET));
    assert_eq!(
        chat.last().unwrap()["content"],
        json!(format!("{PREFIX}{}", reply_of(&turn)))
    );
    // 沒動的行逐字保留
    let raw = fs::read_to_string(fixture.dir.join("chat_history.jsonl")).unwrap();
    assert_eq!(raw.lines().next().unwrap(), system_raw);

    let updates = read_rows(&fixture.dir.join("updates.jsonl"));
    let kinds: Vec<&str> = updates.iter().filter_map(update_kind).collect();
    assert_eq!(
        kinds,
        [
            "user_message_chunk",
            "agent_message_chunk",
            "agent_message_chunk",
            "turn_completed",
            "background_tasks"
        ]
    );
    assert_eq!(
        updates[1]["params"]["update"]["content"]["text"],
        json!(format!("{PREFIX}{}", turn.reply_chunks[0]))
    );
    assert!(!value_contains(&Value::Array(updates), SECRET));
    // 暫存檔不留在 session 目錄，也不留在暫存目錄
    let tmp = fixture.home.join(TMP_DIR);
    assert_eq!(fs::read_dir(tmp).unwrap().count(), 0);
}

#[test]
fn turn_without_confidential_still_strips_reasoning_and_never_drops() {
    let turn = turn(None);
    let fixture = fixture("noconf", &turn);
    // 目錄裡有壞 JSON：沒送機密段就不做殘留掃描，不會因此丟線
    fs::write(fixture.dir.join("plan.json"), "{壞").unwrap();
    run(&fixture, None, &reply_of(&turn)).unwrap();
    let chat = read_rows(&fixture.dir.join("chat_history.jsonl"));
    assert!(chat.iter().all(|row| row["type"] != "reasoning"));
    // 空字串機密段等同沒送（空字串會命中每個檔）
    let fixture = self::fixture("empty", &turn);
    run(&fixture, Some(""), &reply_of(&turn)).unwrap();
}

#[test]
fn segment_must_appear_exactly_once_when_sent() {
    let turn = turn(None);
    let fixture = fixture("miss", &turn);
    expect_drop(
        &fixture,
        Some(SECRET),
        &reply_of(&turn),
        DropReason::RewriteFailed,
    );

    let doubled = format!("{SECRET}{SECRET}");
    let turn = self::turn(Some(&doubled));
    let fixture = self::fixture("twice", &turn);
    expect_drop(
        &fixture,
        Some(SECRET),
        &reply_of(&turn),
        DropReason::RewriteFailed,
    );
}

#[test]
fn tool_items_anywhere_in_the_turn_drop_the_lane() {
    let cases: Vec<(&str, Vec<Value>, Vec<Value>)> = vec![
        (
            "backend",
            vec![
                json!({ "type": "backend_tool_call", "kind": { "tool_type": "x_search", "input": "{}" } }),
            ],
            vec![],
        ),
        (
            "result",
            vec![json!({ "type": "tool_result", "tool_call_id": "c0", "content": "denied" })],
            vec![],
        ),
        (
            "tool_call",
            vec![],
            vec![update("tool_call", json!({ "toolCallId": "c0" }))],
        ),
        (
            "tool_update",
            vec![],
            vec![update("tool_call_update", json!({ "toolCallId": "c0" }))],
        ),
        ("hook", vec![], vec![update("hook_execution", json!({}))]),
        ("retry", vec![], vec![update("retry_state", json!({}))]),
    ];
    for (tag, chat_extra, updates_extra) in cases {
        let mut turn = turn(Some(SECRET));
        turn.chat_extra = chat_extra;
        turn.updates_extra = updates_extra;
        let fixture = fixture(&format!("tool-{tag}"), &turn);
        expect_drop(
            &fixture,
            Some(SECRET),
            &reply_of(&turn),
            DropReason::RewriteFailed,
        );
    }

    // assistant 自己帶 tool_calls
    let turn = turn(Some(SECRET));
    let fixture = fixture("tool-calls", &turn);
    let path = fixture.dir.join("chat_history.jsonl");
    let mut rows = read_rows(&path);
    rows.last_mut().unwrap()["tool_calls"] = json!([{ "id": "c0", "name": "send_feedback" }]);
    write_jsonl(&path, &rows);
    expect_drop(
        &fixture,
        Some(SECRET),
        &reply_of(&turn),
        DropReason::RewriteFailed,
    );
}

#[test]
fn any_compaction_trace_drops_as_compacted() {
    let turn = turn(Some(SECRET));
    let reply = reply_of(&turn);

    let fixture = self::fixture("compact-dir", &turn);
    fs::create_dir_all(fixture.dir.join("compaction_checkpoints")).unwrap();
    expect_drop(&fixture, Some(SECRET), &reply, DropReason::Compacted);

    let fixture = self::fixture("compact-signal", &turn);
    fs::write(fixture.dir.join("signals.json"), r#"{"compactionCount":1}"#).unwrap();
    expect_drop(&fixture, Some(SECRET), &reply, DropReason::Compacted);

    let fixture = self::fixture("compact-meta", &turn);
    let path = fixture.dir.join("chat_history.jsonl");
    let mut rows = read_rows(&path);
    rows.insert(1, json!({ "type": "user", "content": [{ "type": "text", "text": "Summary" }], "synthetic_reason": "compaction_meta" }));
    write_jsonl(&path, &rows);
    expect_drop(&fixture, Some(SECRET), &reply, DropReason::Compacted);

    let fixture = self::fixture("compact-update", &turn);
    let path = fixture.dir.join("updates.jsonl");
    let mut rows = read_rows(&path);
    rows.insert(0, update("compaction_checkpoint", json!({})));
    write_jsonl(&path, &rows);
    expect_drop(&fixture, Some(SECRET), &reply, DropReason::Compacted);
}

#[test]
fn cross_file_mismatch_or_foreign_prefix_drops() {
    let turn = turn(Some(SECRET));
    let reply = reply_of(&turn);

    // updates 的 user 與 chat_history 不同
    let fixture = self::fixture("mismatch-user", &turn);
    let path = fixture.dir.join("updates.jsonl");
    let mut rows = read_rows(&path);
    rows[0] = text_update("user_message_chunk", &format!("{}多一句", turn.prompt));
    write_jsonl(&path, &rows);
    expect_drop(&fixture, Some(SECRET), &reply, DropReason::RewriteFailed);

    // 串流收到的回覆和檔案不同
    let fixture = self::fixture("mismatch-reply", &turn);
    expect_drop(
        &fixture,
        Some(SECRET),
        "別的回覆",
        DropReason::RewriteFailed,
    );

    // 回覆本身含「狐狸：」：前綴不只一次（已知只損快取）
    let mut turn = self::turn(Some(SECRET));
    turn.reply_chunks = vec!["他說：「", "狐狸：在此。」"];
    let fixture = self::fixture("prefix-twice", &turn);
    expect_drop(
        &fixture,
        Some(SECRET),
        &reply_of(&turn),
        DropReason::RewriteFailed,
    );
}

/// 模型模仿歷史格式自己寫了「狐狸：」（實機 2026-10-07 遇到，前綴還被切在兩個 chunk）：
/// 不重補、不丟線。
#[test]
fn reply_that_already_carries_the_prefix_is_not_prefixed_twice() {
    let mut turn = turn(Some(SECRET));
    turn.reply_chunks = vec!["狐", "狸：狐狸擦杯子的手頓了半息。"];
    let fixture = fixture("self-prefixed", &turn);
    run(&fixture, Some(SECRET), &reply_of(&turn)).unwrap();
    let chat = read_rows(&fixture.dir.join("chat_history.jsonl"));
    assert_eq!(chat.last().unwrap()["content"], json!(reply_of(&turn)));
    let updates = read_rows(&fixture.dir.join("updates.jsonl"));
    assert_eq!(
        updates[1]["params"]["update"]["content"]["text"],
        json!("狐")
    );
}

#[test]
fn residue_scan_decodes_json_before_matching() {
    let turn = turn(Some(SECRET));
    let fixture = fixture("residue", &turn);
    // summary.json 裡以 JSON 跳脫存著機密段：位元組層看不到全文，解碼後才看得到
    let summary = serde_json::to_string(&json!({ "session_summary": SECRET })).unwrap();
    assert!(!summary.contains(SECRET));
    fs::write(fixture.dir.join("summary.json"), summary).unwrap();
    expect_drop(
        &fixture,
        Some(SECRET),
        &reply_of(&turn),
        DropReason::RewriteFailed,
    );

    // 子目錄也掃；壞 JSON 算掃描失敗
    let fixture = self::fixture("residue-bad-json", &turn);
    fs::create_dir_all(fixture.dir.join("subagents")).unwrap();
    fs::write(fixture.dir.join("subagents").join("meta.json"), "{壞").unwrap();
    expect_drop(
        &fixture,
        Some(SECRET),
        &reply_of(&turn),
        DropReason::RewriteFailed,
    );
}

#[test]
fn held_lock_times_out_instead_of_waiting_forever() {
    let turn = turn(Some(SECRET));
    let fixture = fixture("lock", &turn);
    let holder = File::open(fixture.dir.join("updates.jsonl.lock")).unwrap();
    holder.lock().unwrap();
    let started = Instant::now();
    expect_drop(
        &fixture,
        Some(SECRET),
        &reply_of(&turn),
        DropReason::LockTimeout,
    );
    assert!(started.elapsed() < LOCK_BUDGET + Duration::from_secs(2));
    // 檔案沒被動過
    let chat = read_rows(&fixture.dir.join("chat_history.jsonl"));
    assert!(chat.iter().any(|row| row["type"] == "reasoning"));
}

#[test]
fn second_file_failure_reports_error_after_first_file_written() {
    let turn = turn(Some(SECRET));
    let fixture = fixture("second-fail", &turn);
    // 第二檔的暫存路徑先被目錄佔住：chat_history 已換掉、updates 寫不進去
    let tmp = fixture.home.join(TMP_DIR);
    fs::create_dir_all(tmp.join(format!("{SID}-updates.jsonl-{}", std::process::id()))).unwrap();
    expect_drop(
        &fixture,
        Some(SECRET),
        &reply_of(&turn),
        DropReason::RewriteFailed,
    );
    let updates = fs::read_to_string(fixture.dir.join("updates.jsonl")).unwrap();
    assert!(updates.contains("agent_thought_chunk"), "第二檔維持原樣");
}

#[test]
fn remove_session_only_deletes_a_unique_match() {
    let turn = turn(None);
    let fixture = fixture("remove", &turn);
    let twin = fixture.home.join("sessions").join("slug-abc123").join(SID);
    fs::create_dir_all(&twin).unwrap();
    assert!(remove_session(&fixture.home, SID).is_err());
    assert!(fixture.dir.exists() && twin.exists(), "多個目錄一個都不刪");

    fs::remove_dir_all(&twin).unwrap();
    assert_eq!(remove_session(&fixture.home, SID).unwrap(), 1);
    assert!(!fixture.dir.exists());
    assert!(fixture.dir.parent().unwrap().exists(), "不刪上一層");
    assert_eq!(remove_session(&fixture.home, SID).unwrap(), 0); // 0 個＝已刪
    assert!(remove_session(&fixture.home, "../x").is_err());
}

/// 新 grok-home 第一通（2026-10-28 grok 1.0.70 實測形狀）：技能清單補在回合尾的 assistant 之後。
fn trailing_reminder(text: &str) -> Value {
    json!({ "type": "user", "content": [{ "type": "text", "text": text }],
            "synthetic_reason": "system_reminder" })
}

const SKILLS_REMINDER: &str =
    "<system-reminder>\nThe following skills are available for use:\n\n- pdf: PDF\n</system-reminder>";

/// 在 chat_history 檔尾追加幾列，回傳追加後的原始行。
fn append_chat(fixture: &Fixture, rows: &[Value]) -> Vec<String> {
    let path = fixture.dir.join("chat_history.jsonl");
    let mut all = read_rows(&path);
    all.extend(rows.iter().cloned());
    write_jsonl(&path, &all);
    rows.iter()
        .map(|row| serde_json::to_string(row).unwrap())
        .collect()
}

fn chat_lines(fixture: &Fixture) -> Vec<String> {
    fs::read_to_string(fixture.dir.join("chat_history.jsonl"))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn trailing_skills_reminder_is_kept_in_place_and_next_turn_still_rewrites() {
    let turn = turn(Some(SECRET));
    let fixture = fixture("trailing-reminder", &turn);
    let reminder = append_chat(&fixture, &[trailing_reminder(SKILLS_REMINDER)]).remove(0);
    run(&fixture, Some(SECRET), &reply_of(&turn)).expect("回合尾提醒要放行");
    let lines = chat_lines(&fixture);
    assert_eq!(lines.last(), Some(&reminder), "提醒逐 byte 留在檔尾");
    let rows = read_rows(&fixture.dir.join("chat_history.jsonl"));
    assert_eq!(rows[rows.len() - 2]["type"], "assistant");

    // 下一輪：真 user 接在提醒後面，抹寫照常，提醒仍原樣
    let next_reply = "狐狸：「還不到說的時候。」";
    append_chat(
        &fixture,
        &[
            json!({ "type": "user", "content": [{ "type": "text", "text": "現在你是「狐狸」。" }], "prompt_index": 1 }),
            json!({ "type": "reasoning", "id": "rs_2", "summary": [], "encrypted_content": "x", "status": "completed" }),
            json!({ "type": "assistant", "content": "「還不到說的時候。」" }),
        ],
    );
    let updates_path = fixture.dir.join("updates.jsonl");
    let mut updates = read_rows(&updates_path);
    updates.push(text_update("user_message_chunk", "現在你是「狐狸」。"));
    updates.push(text_update("agent_message_chunk", "「還不到說的時候。」"));
    updates.push(update(
        "turn_completed",
        json!({ "stop_reason": "end_turn" }),
    ));
    write_jsonl(&updates_path, &updates);
    run(&fixture, None, next_reply).expect("下一輪照常抹寫");
    assert!(chat_lines(&fixture).contains(&reminder));
    let rows = read_rows(&fixture.dir.join("chat_history.jsonl"));
    assert!(rows.iter().all(|row| row["type"] != "reasoning"));
    assert_eq!(rows.last().unwrap()["content"], next_reply);
}

#[test]
fn reminder_shapes_outside_the_allowance_drop_the_lane() {
    let plain_user =
        json!({ "type": "user", "content": [{ "type": "text", "text": SKILLS_REMINDER }] });
    let interjection = json!({ "type": "user", "content": [{ "type": "text", "text": SKILLS_REMINDER }],
                               "synthetic_reason": "interjection" });
    let untagged = trailing_reminder("The following skills are available for use");
    let reasoning =
        json!({ "type": "reasoning", "id": "rs_9", "summary": [], "status": "completed" });
    let reminder = trailing_reminder(SKILLS_REMINDER);
    let cases: Vec<(&str, Vec<Value>)> = vec![
        ("plain-user", vec![plain_user]),
        ("interjection", vec![interjection]),
        ("no-tag", vec![untagged]),
        ("then-reasoning", vec![reminder.clone(), reasoning]),
        ("two-reminders", vec![reminder.clone(), reminder.clone()]),
    ];
    for (tag, tail) in cases {
        let turn = turn(Some(SECRET));
        let fixture = fixture(&format!("reminder-{tag}"), &turn);
        append_chat(&fixture, &tail);
        expect_drop(
            &fixture,
            Some(SECRET),
            &reply_of(&turn),
            DropReason::RewriteFailed,
        );
    }

    // 提醒夾在 assistant 之前
    let mut turn = turn(Some(SECRET));
    turn.chat_extra = vec![reminder];
    let fixture = fixture("reminder-before-assistant", &turn);
    expect_drop(
        &fixture,
        Some(SECRET),
        &reply_of(&turn),
        DropReason::RewriteFailed,
    );
}

#[test]
fn trailing_reminder_carrying_the_secret_drops_the_lane() {
    let turn = turn(Some(SECRET));
    let fixture = fixture("reminder-secret", &turn);
    let text = format!("<system-reminder>\n{SECRET}</system-reminder>");
    let line = append_chat(&fixture, &[trailing_reminder(&text)]).remove(0);
    assert!(!line.contains(SECRET), "JSON 跳脫後位元組層看不到全文");
    let error = expect_drop_error(
        &fixture,
        Some(SECRET),
        &reply_of(&turn),
        DropReason::RewriteFailed,
    );
    assert!(error.detail.contains("仍含機密段"), "{}", error.detail);
}

/// 帶 prompt_index 鍵（即使是 null）的提醒會被當成本輪 user，後面沒有 assistant 而丟線。
#[test]
fn reminder_with_prompt_index_key_is_taken_as_the_turn_user_and_drops() {
    let turn = turn(Some(SECRET));
    let fixture = fixture("reminder-prompt-index-key", &turn);
    let mut indexed = trailing_reminder(SKILLS_REMINDER);
    indexed["prompt_index"] = Value::Null;
    append_chat(&fixture, &[indexed]);
    let error = expect_drop_error(
        &fixture,
        Some(SECRET),
        &reply_of(&turn),
        DropReason::RewriteFailed,
    );
    assert!(
        error.detail.contains("本輪沒有 assistant"),
        "{}",
        error.detail
    );
}

/// 找 session 時略過沒有這個 id 的群組與散檔，繼續找到真正那份
#[test]
fn search_skips_groups_without_the_id() {
    let turn = turn(None);
    let fixture = fixture("search-skip", &turn);
    let sessions = fixture.home.join("sessions");
    fs::create_dir_all(sessions.join("aaa-other-group")).unwrap();
    fs::write(sessions.join("session_search.sqlite"), b"").unwrap();
    assert_eq!(
        find_session_dirs(&fixture.home, SID).unwrap(),
        vec![fixture.dir.clone()]
    );
}

/// 符號連結一律不跟隨（sessions、群組、`<id>` 三層）：找不到、刪不到、抹寫丟線，連結指向的目錄原封不動
#[cfg(unix)]
#[test]
fn symlinks_are_never_followed() {
    use std::os::unix::fs::symlink;
    let turn = turn(Some(SECRET));
    let fixture = fixture("symlink", &turn);
    let outside = fixture.home.with_extension("outside");
    let _ = fs::remove_dir_all(&outside);
    fs::create_dir_all(&outside).unwrap();
    let target = outside.join(SID);
    fs::rename(&fixture.dir, &target).unwrap();
    let chat_before = fs::read(target.join("chat_history.jsonl")).unwrap();

    // <id> 是連結
    symlink(&target, &fixture.dir).unwrap();
    assert!(find_session_dirs(&fixture.home, SID).unwrap().is_empty());
    assert_eq!(remove_session(&fixture.home, SID).unwrap(), 0);
    expect_drop(
        &fixture,
        Some(SECRET),
        &reply_of(&turn),
        DropReason::RewriteFailed,
    );
    fs::remove_file(&fixture.dir).unwrap();

    // 群組是連結
    symlink(&outside, fixture.home.join("sessions").join("linked-group")).unwrap();
    assert!(find_session_dirs(&fixture.home, SID).unwrap().is_empty());
    assert_eq!(remove_session(&fixture.home, SID).unwrap(), 0);

    // sessions 本身是連結
    let sessions = fixture.home.join("sessions");
    fs::remove_dir_all(&sessions).unwrap();
    let fake_sessions = fixture.home.with_extension("sessions");
    let _ = fs::remove_dir_all(&fake_sessions);
    let real_behind_link = fake_sessions.join("group").join(SID);
    fs::create_dir_all(&real_behind_link).unwrap();
    symlink(&fake_sessions, &sessions).unwrap();
    assert!(find_session_dirs(&fixture.home, SID).unwrap().is_empty());
    assert_eq!(remove_session(&fixture.home, SID).unwrap(), 0);
    assert!(real_behind_link.exists());

    assert_eq!(
        fs::read(target.join("chat_history.jsonl")).unwrap(),
        chat_before
    );
    fs::remove_dir_all(&outside).unwrap();
    fs::remove_dir_all(&fake_sessions).unwrap();
}
