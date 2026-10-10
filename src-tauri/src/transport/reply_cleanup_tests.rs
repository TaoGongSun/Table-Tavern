use super::*;

const CASES: &str = include_str!("../../../src/shared/contracts/reply-cleanup/cases.json");
const FOX: &str = "狐狸：";

fn finish(reply: &str) -> CharacterReply {
    finish_character_reply(reply, FOX, false)
}

#[test]
fn shared_cases_match_final_rules() {
    let cases: serde_json::Value = serde_json::from_str(CASES).unwrap();
    for case in cases["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let input = case["input"].as_str().unwrap();
        assert_eq!(
            final_reply_text(input, false),
            case["final_complete"].as_str().unwrap(),
            "{name}（完成）"
        );
        assert_eq!(
            final_reply_text(input, true),
            case["final_aborted"].as_str().unwrap(),
            "{name}（中止）"
        );
    }
}

#[test]
fn strips_control_blocks_and_keeps_raw_without_prefix() {
    for reply in [
        "她點頭。<UpdateVariable>{\"time\":\"午夜\"}</UpdateVariable>",
        "她點頭。<status>hp: 3</status>",
        "她點頭。\n```state\nhp: 3\n```",
        "狐狸：她點頭。<UpdateVariable>x</UpdateVariable>",
    ] {
        let out = finish(reply);
        assert_eq!(out.text, "她點頭。", "{reply}");
        let raw = out.raw.expect("剝過就有 raw");
        assert!(!raw.starts_with(FOX), "{reply}");
        assert_eq!(raw, reply.strip_prefix(FOX).unwrap_or(reply));
    }
}

#[test]
fn plain_reply_has_no_raw() {
    let out = finish("狐狸：你好");
    assert_eq!(out.text, "你好");
    assert_eq!(out.raw, None);
    assert_eq!(finish("你好").raw, None);
}

#[test]
fn tag_before_name_strips_prefix_from_text_and_raw() {
    let out = finish("<UpdateVariable>x</UpdateVariable>\n狐狸：你好");
    assert_eq!(out.text, "你好");
    assert_eq!(
        out.raw.as_deref(),
        Some("<UpdateVariable>x</UpdateVariable>\n你好")
    );
    // 名字後面沒東西＝沒有正文
    assert_eq!(finish("<UpdateVariable>x</UpdateVariable>狐狸：").text, "");
    let english = finish_character_reply("<status>a</status>Fox:", "Fox: ", false);
    assert_eq!(english.text, "");
}

#[test]
fn leading_whitespace_and_double_prefix() {
    assert_eq!(finish("  狐狸：你好").text, "你好");
    // 第一次剝到就不再剝：只剝一層
    assert_eq!(finish("狐狸：狐狸：你好").text, "狐狸：你好");
}

#[test]
fn raw_removes_only_the_visible_prefix() {
    // 控制區塊內也有「狐狸：」：raw 只拿掉正文開頭那一處
    let reply = "<UpdateVariable>狐狸：付錢</UpdateVariable>\n狐狸：你好";
    let out = finish(reply);
    assert_eq!(out.text, "你好");
    assert_eq!(
        out.raw.as_deref(),
        Some("<UpdateVariable>狐狸：付錢</UpdateVariable>\n你好")
    );
    // 自閉合標籤在前：raw 保留標籤、不切進標籤
    let out = finish("<StatusPlaceHolderImpl/>\n狐狸：你好");
    assert_eq!(out.text, "你好");
    assert_eq!(out.raw.as_deref(), Some("<StatusPlaceHolderImpl/>\n你好"));
    // 多個控制區塊、maintext 外殼、多位元組字元
    let reply = "<status>a: 1</status><UpdateVariable>狐狸：</UpdateVariable><maintext>狐狸：🦊你好</maintext>";
    let out = finish(reply);
    assert_eq!(out.text, "🦊你好");
    assert_eq!(
        out.raw.as_deref(),
        Some("<status>a: 1</status><UpdateVariable>狐狸：</UpdateVariable><maintext>🦊你好</maintext>")
    );
}

#[test]
fn self_closing_does_not_swallow_prose() {
    assert_eq!(
        finish("<status/>正文<status>狀態</status>尾文").text,
        "正文尾文"
    );
    assert_eq!(finish("台詞<StatusPlaceHolderImpl/>後文").text, "台詞後文");
}

#[test]
fn unclosed_tail_rules_by_abort() {
    let aborted = |reply: &str| finish_character_reply(reply, FOX, true).text;
    assert_eq!(finish("你好<UpdateVariable>{\"a\"").text, "你好");
    assert_eq!(aborted("你好<UpdateVariable>{\"a\""), "你好");
    assert_eq!(finish("你好<Upd").text, "你好<Upd");
    assert_eq!(aborted("你好<Upd"), "你好");
    assert_eq!(finish("你好<details><summary>x").text, "你好");
    assert_eq!(finish("看\n```\ncode").text, "看\n```\ncode");
    assert_eq!(aborted("看\n```\ncode"), "看");
    assert_eq!(finish("你好<").text, "你好<");
    assert_eq!(aborted("你好`"), "你好`");
    assert_eq!(aborted("你好``"), "你好``");
}

#[test]
fn only_tags_leaves_empty_text() {
    let out = finish("<UpdateVariable>x</UpdateVariable>");
    assert_eq!(out.text, "");
    assert!(out.raw.is_some());
}

#[test]
fn raw_is_kept_whenever_the_text_differs_literally() {
    // 只差頭尾空白也留原文，不遺失
    let out = finish("  你好  ");
    assert_eq!(out.text, "你好");
    assert_eq!(out.raw.as_deref(), Some("  你好  "));
}

#[test]
fn raw_with_repeated_or_unmatched_prefix() {
    // 前綴出現多次：只拿掉正文開頭那一處
    let out = finish("<UpdateVariable>x</UpdateVariable>\n狐狸：你說狐狸：是誰");
    assert_eq!(out.text, "你說狐狸：是誰");
    assert_eq!(
        out.raw.as_deref(),
        Some("<UpdateVariable>x</UpdateVariable>\n你說狐狸：是誰")
    );
    // 前綴是剝殼後才拼出來的、原文裡找不到可拿掉的那一處：raw 保留原樣
    let reply = "狐<status>a: 1</status>狸：你好";
    let out = finish(reply);
    assert_eq!(out.text, "你好");
    assert_eq!(out.raw.as_deref(), Some(reply));
}

#[test]
fn nested_details_are_matched_by_level() {
    let aborted = |reply: &str| finish_character_reply(reply, FOX, true).text;
    let nested = "正文<details><summary>其他</summary><details>內文</details>外層半截";
    assert_eq!(finish(nested).text, "正文");
    assert_eq!(aborted(nested), "正文");
    let split = "正文<details>沒閉<details><summary>其他</summary>x</details>";
    assert_eq!(finish(split).text, "正文");
    assert_eq!(aborted(split), "正文");
    // 中止停在 maintext 外殼的前半也切
    assert_eq!(aborted("<maintext>正文</maint"), "正文");
}
