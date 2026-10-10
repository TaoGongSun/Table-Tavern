//! fixture 照 grok 1.0.46 實際落檔的形狀手寫（2026-10-06 實驗擷取），不含憑證。

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
    let error = run(fixture, confidential, reply).expect_err("形狀不符必須丟線");
    assert_eq!(error.reason, reason, "{}", error.detail);
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
    remove_session(&fixture.home, SID).unwrap();
    assert!(!fixture.dir.exists());
    assert!(fixture.dir.parent().unwrap().exists(), "不刪上一層");
    remove_session(&fixture.home, SID).unwrap(); // 0 個＝已刪
    assert!(remove_session(&fixture.home, "../x").is_err());
}
