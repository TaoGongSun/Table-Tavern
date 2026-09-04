use super::*;

#[test]
fn all_stage_system_messages_are_byte_identical_for_same_context() {
    let context = "測試脈絡";
    let survey = survey_messages(context, &[], "zh-TW", "interface");
    let expand = expand_messages(context, "1", "條目全文", EntryKind::Interface, &[], "zh-TW");
    let person = person_expand_messages(
        context,
        "亞瑟",
        &[("1".to_owned(), "條目全文".to_owned())],
        "zh-TW",
    );
    let absorb = absorb_messages(context, "1", "條目全文", &[], "zh-TW");
    let group = group_messages(
        context,
        "格式與行為",
        GroupKind::Setting,
        &[("1#s1".to_owned(), "段落".to_owned())],
        &[],
        "zh-TW",
    );
    assert_eq!(survey[0].role, "system");
    for messages in [&expand, &person, &absorb, &group] {
        assert_eq!(messages[0].role, "system");
        assert_eq!(survey[0].content, messages[0].content);
    }
}
