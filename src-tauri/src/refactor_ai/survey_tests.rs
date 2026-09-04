use super::types::PrescanSignal;
use super::*;

#[test]
fn recommend_messages_share_survey_system_byte_identical() {
    let context = "## 世界書條目\n測試";
    let recommend = recommend_messages(context, "zh-TW");
    let survey = survey_messages(context, &[], "zh-TW", "interface");
    // 兩段同 session 承前綴快取的前提：system 逐位元組相同（characters 版也同一份 system）
    assert_eq!(recommend[0].content, survey[0].content);
    assert_eq!(
        recommend[0].content,
        survey_messages(context, &[], "zh-TW", "characters")[0].content
    );
    assert_eq!(recommend[0].role, "system");
    assert!(recommend[1].content.contains("RECOMMEND:"));
}

/// 模式段只動 user 訊息端：interface 版禁 person route＋PERSONS 留空；characters 版
/// 人物照認；兩版都要求 MODE 回聲。
#[test]
fn survey_messages_carry_mode_specific_user_instructions() {
    let interface = &survey_messages("ctx", &[], "zh-TW", "interface")[1].content;
    assert!(interface.contains("## MODE: interface"));
    assert!(interface.contains("PERSONS 區塊固定留空"));
    assert!(interface.contains("`person name:` 這個 route 本次不可使用"));
    let characters = &survey_messages("ctx", &[], "zh-TW", "characters")[1].content;
    assert!(characters.contains("## MODE: characters"));
    assert!(characters.contains("人物照常認"));
}

#[test]
fn survey_messages_injects_prescan_signals_into_user_message() {
    let with_signals = survey_messages(
        "ctx",
        &[PrescanSignal {
            uid: "3".to_owned(),
            span: "3#s2".to_owned(),
            pattern: "trigger:".to_owned(),
        }],
        "zh-TW",
        "interface",
    );
    assert!(with_signals[1].content.contains("uid=3#s2"));
    assert!(with_signals[1].content.contains("trigger:"));

    let without_signals = survey_messages("ctx", &[], "zh-TW", "interface");
    assert!(without_signals[1].content.contains("結構預掃訊號：（無）"));
}
