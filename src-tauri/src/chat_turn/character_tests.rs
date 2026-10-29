use super::character_reply;

/// char-line-status-strip：完成回合剝完沒台詞回錯；中止沒台詞照舊交回（settle 撤回落地）
#[test]
fn character_reply_errors_only_when_a_finished_turn_has_no_text() {
    let tag = "<UpdateVariable>x</UpdateVariable>";
    let error = character_reply(&format!("狐狸：{tag}"), "狐狸：", false)
        .err()
        .expect("完成回合沒台詞要回錯");
    assert!(error.starts_with("AI_EMPTY_RESPONSE"), "{error}");
    let aborted = character_reply(tag, "狐狸：", true)
        .ok()
        .expect("中止照舊交回");
    assert!(aborted.aborted && aborted.text.is_empty());
    let reply = character_reply(&format!("狐狸：你好{tag}"), "狐狸：", false)
        .ok()
        .unwrap();
    assert_eq!(reply.text, "你好");
    assert_eq!(reply.raw.as_deref(), Some(format!("你好{tag}").as_str()));
}
