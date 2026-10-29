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

fn echo_tree() -> BTreeMap<String, StateNode> {
    let leaf = |value: &str| StateNode::Leaf(value.to_owned());
    BTreeMap::from([
        ("驻守天数".to_owned(), leaf("2")),
        ("当前环境".to_owned(), leaf("作戰室")),
        (
            "Heroes".to_owned(),
            StateNode::Branch(BTreeMap::from([(
                "艾德温".to_owned(),
                StateNode::Branch(BTreeMap::from([("Status".to_owned(), leaf("平稳"))])),
            )])),
        ),
    ])
}

/// 正例：尾端「系統前綴＋狀態變動標頭」段，每行都是本桌路徑——剝到只剩正文（舊標頭、送模型標頭、英文骨架都認）。
#[test]
fn strip_state_echo_removes_trailing_ledger_paragraph() {
    let tree = echo_tree();
    for heading in [
        "（系統）狀態更新".to_owned(),
        "（系统）状态更新".to_owned(),
        "(System) State update".to_owned(),
        format!("（系統）{}", crate::data::prompt_state_heading("zh-TW")),
        format!("(System) {}", crate::data::prompt_state_heading("en")),
    ] {
        let display = format!("托倫看著地圖。\n\n{heading}\n驻守天数：3\nHeroes.艾德温.Status: 戒备\n当前环境：作戰室\n");
        // 前面有已閉合的圍欄不影響
        let fenced =
            format!("```\n代碼\n```\n~~~\n更多\n~~~\n托倫看著地圖。\n\n{heading}\n驻守天数：3");
        assert_eq!(
            strip_state_echo(&fenced, &tree)
                .map(|echo| echo.text)
                .as_deref(),
            Some("```\n代碼\n```\n~~~\n更多\n~~~\n托倫看著地圖。"),
            "{heading}"
        );
        assert_eq!(
            strip_state_echo(&display, &tree)
                .map(|echo| echo.text)
                .as_deref(),
            Some("托倫看著地圖。"),
            "{heading}"
        );
    }
}

/// 反例：任何一處對不上就整段保留。
#[test]
fn strip_state_echo_keeps_anything_not_exactly_an_echo() {
    let tree = echo_tree();
    let cases = [
        // 卡片自訂的狀態段（沒有系統標頭）
        "正文。\n\n状态：平稳\n驻守天数：3",
        // 跨行值：第二行不是路徑行
        "正文。\n\n（系統）狀態更新\n当前环境：作戰室，\n窗外仍在下雪",
        // 對話裡引用「（系統）」
        "正文。\n\n他低聲說：「（系統）狀態更新」\n驻守天数：3",
        // 混入不認得的路徑
        "正文。\n\n（系統）狀態更新\n驻守天数：3\n好感度：5",
        // 在程式碼區塊裡
        "正文。\n\n```\n（系統）狀態更新\n驻守天数：3",
        // 四空白縮排的程式碼區塊
        "正文。\n\n    （系統）狀態更新\n    驻守天数：3",
        // 未閉合的 ~~~ 區塊
        "正文。\n\n~~~\n（系統）狀態更新\n驻守天数：3",
        // ``` 開著時遇到 ~~~ 不算關閉
        "正文。\n\n```\n~~~\n（系統）狀態更新\n驻守天数：3",
        // 四個反引號開的圍欄，三個反引號關不掉
        "正文。\n\n````\n```\n（系統）狀態更新\n驻守天数：3",
        // 關閉記號後面有文字不算關閉
        "正文。\n\n```\n```text\n（系統）狀態更新\n驻守天数：3",
        // 縮排三格的圍欄開頭仍算圍欄
        "正文。\n\n   ~~~\n（系統）狀態更新\n驻守天数：3",
        // 標頭之後還有正文（不在尾端）
        "正文。\n\n（系統）狀態更新\n驻守天数：3\n\n托倫點了點頭。",
        // 只有標頭
        "正文。\n\n（系統）狀態更新",
        // 標頭行不是完整一行
        "正文。\n\n（系統）狀態更新：驻守天数：3",
    ];
    for display in cases {
        assert_eq!(strip_state_echo(display, &tree), None, "{display}");
    }
}

/// narrator-prefix-echo：只剝顯示正文開頭的旁白標籤（新舊、中英、帶不帶空白、連續多個）
#[test]
fn strip_narrator_label_only_at_start() {
    for (input, want) in [
        ("（旁白）夜深了。", "夜深了。"),
        ("(Narration) The night deepens.", "The night deepens."),
        ("(Narration)The night deepens.", "The night deepens."),
        ("GM：夜深了。", "夜深了。"),
        ("GM: The night deepens.", "The night deepens."),
        ("GM:The night deepens.", "The night deepens."),
        ("\n  （旁白） 夜深了。", "夜深了。"),
        ("（旁白）（旁白）夜深了。", "夜深了。"),
        (
            "GM：（旁白）夜深了。\n\n（旁白）第二段",
            "夜深了。\n\n（旁白）第二段",
        ),
        ("（旁白）", ""),
        ("（旁白）  \n", ""),
    ] {
        assert_eq!(
            strip_narrator_label(input).as_deref(),
            Some(want),
            "{input}"
        );
    }
    for kept in [
        "夜深了。\n\n（旁白）第二段",
        "旁白：夜深了。",
        "（旁白者）夜深了。",
        "(旁白)夜深了。",
        "【旁白】夜深了。",
        "GMing is hard.",
        "gm: lower case",
        "Narration) 少了左括號",
        "",
    ] {
        assert_eq!(strip_narrator_label(kept), None, "{kept}");
    }
}
