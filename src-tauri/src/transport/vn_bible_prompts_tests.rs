use super::*;

/// 圍欄或裸 JSON 都認；沒有標記、不是物件算失敗
#[test]
fn parses_fenced_or_bare_bible_and_secret_replies() {
    let fenced = "前言\n## BIBLE\n```json\n{\"logline\": \"主線\", \"chapter_max\": 6}\n```\n";
    assert_eq!(parse_bible_reply(fenced).unwrap().logline, "主線");
    let bare = "## BIBLE\n{\"style\": \"冷硬\"} 結尾";
    assert_eq!(parse_bible_reply(bare).unwrap().style, "冷硬");
    assert!(parse_bible_reply("{\"logline\": \"x\"}").is_none());
    assert!(parse_bible_reply("## BIBLE\n```json\n[1]\n```").is_none());

    let reply = "## PRIVATE\n新私設\n## SECRETS\n```json\n[{\"id\": \"a\"}, {\"bad\": 1}]\n```";
    let (private, secrets) = parse_secret_reply(reply).unwrap();
    assert_eq!(private, "新私設");
    assert_eq!(secrets.len(), 1);
    assert!(parse_secret_reply("## PRIVATE\n\n## SECRETS\n[]").is_none());
}

/// 常駐段列出旗標可用值與揭露寫法；當章包只帶該章，章號超出回空
#[test]
fn resident_and_chapter_pack_render_in_both_scaffolds() {
    let bible = crate::data::vn::normalize(
        crate::data::vn::parse_bible(&serde_json::json!({
            "logline": "主線",
            "chapters": [{"title": "序", "goal": "相遇", "beats": "拍點"}],
            "flags": [{"id": "route", "type": "route", "values": ["a", "b"]}],
            "secrets": [{"id": "s1", "holder": "c1", "reveal": "never", "hint": "眼神"}]
        }))
        .unwrap(),
        &[("c1".to_owned(), "艾琳".to_owned())],
    );
    let names = |_: &str| "艾琳".to_owned();
    let zh = bible_resident("zh-TW", &bible, &names);
    assert!(zh.contains("flags.route") && zh.contains("[a / b]"), "{zh}");
    assert!(
        zh.contains("revealed.s1 (艾琳)") && zh.contains("永不明說"),
        "{zh}"
    );
    assert!(zh.contains("revealed.<秘密id> 設為 \"true\""), "{zh}");
    assert!(bible_resident("en", &bible, &names).contains("Story bible"));
    assert!(bible_chapter("zh-TW", &bible, 1).contains("節拍：拍點"));
    assert!(bible_chapter("zh-TW", &bible, 2).is_empty());
}

/// 精靈偏好進聖經提示詞（首次生成與重寫劇本共用 Materials）：題材、長度區間、分支句、主角名
#[test]
fn bible_messages_carry_preferences() {
    use crate::data::vn::{Length, Preferences};
    let preferences = Preferences {
        genres: vec!["校園".to_owned()],
        mood: None,
        length: Some(Length::Long),
        branching: Some(Branching::Full),
        name: "阿哲".to_owned(),
    };
    let source = BibleSource {
        world_md: "霧港",
        cast: Vec::new(),
        gm_entries: Vec::new(),
        preferences: Some(&preferences),
    };
    let text = &bible_messages(&source, "zh-TW")[0].content;
    assert!(text.contains("- Genres: 校園"), "{text}");
    assert!(
        text.contains("\"chapter_max\" must be between 12 and 18"),
        "{text}"
    );
    assert!(
        text.contains("\"branching\" must be \"full\". Choices: they change the ending"),
        "{text}"
    );
    assert!(
        text.contains("- The player character is named 阿哲."),
        "{text}"
    );
    let plain = BibleSource {
        preferences: None,
        ..source
    };
    assert!(!bible_messages(&plain, "zh-TW")[0]
        .content
        .contains("Player preferences"));
}
