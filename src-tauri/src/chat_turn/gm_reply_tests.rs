use super::{aborted_narration, gm_reply_parts};
use crate::data;

/// state-update-echo-in-story：照抄的狀態變動段只從落檔正文剝，raw 與機制輸入（block）照用原文
#[test]
fn gm_reply_parts_strip_state_echo_from_text_only() {
    let tree = std::collections::BTreeMap::from([(
        "驻守天数".to_owned(),
        data::StateNode::Leaf("2".to_owned()),
    )]);
    let reply = "托倫點頭。\n\n（系統）狀態更新\n驻守天数：3\n\n```state\nplace: 作戰室\n```\n\n<UpdateVariable>\n<JSONPatch>\n[{\"op\":\"delta\",\"path\":\"/驻守天数\",\"value\":1}]\n</JSONPatch>\n</UpdateVariable>\n\n下一位：托伦";
    let parts = gm_reply_parts(reply, &tree, false);
    assert_eq!(parts.display, "托倫點頭。");
    assert_eq!(parts.raw.as_deref(), Some(reply));
    assert_eq!(parts.next_raw.as_deref(), Some("托伦"));
    assert_eq!(parts.block.updates.len(), 1);
    assert!(parts
        .block
        .fields
        .iter()
        .any(|(path, value)| path == &["place".to_owned()] && value == "作戰室"));
    // 不是照抄段（路徑認不得）就整段留在正文
    let kept = gm_reply_parts("托倫點頭。\n\n（系統）狀態更新\n好感度：5", &tree, false);
    assert_eq!(kept.display, "托倫點頭。\n\n（系統）狀態更新\n好感度：5");
    assert_eq!(kept.raw, None);
}

/// narrator-prefix-echo：開頭照抄的旁白標籤只從落檔正文剝，raw、機制輸入與點名照原文；剝完沒正文由 gm_narrate 擋
#[test]
fn gm_reply_parts_strip_narrator_label_from_text_only() {
    let tree = std::collections::BTreeMap::new();
    let reply = "（旁白）（旁白）夜更深了。\n\n```state\nplace: 酒館\n```\n\n下一位：狐狸";
    let parts = gm_reply_parts(reply, &tree, false);
    assert_eq!(parts.display, "夜更深了。");
    assert_eq!(parts.raw.as_deref(), Some(reply));
    assert_eq!(parts.next_raw.as_deref(), Some("狐狸"));
    assert!(parts
        .block
        .fields
        .iter()
        .any(|(path, value)| path == &["place".to_owned()] && value == "酒館"));
    let empty = gm_reply_parts("GM：\n```state\nplace: 酒館\n```", &tree, false);
    assert!(empty.display.trim().is_empty());
    let middle = gm_reply_parts("夜更深了。\n\n（旁白）風停了。", &tree, false);
    assert_eq!(middle.display, "夜更深了。\n\n（旁白）風停了。");
    assert_eq!(middle.raw, None);
    assert_eq!(aborted_narration("（旁白）夜更深").text, "夜更深");
}

/// GM 中止半截照中止規則：沒閉合的標籤、標記前半、沒閉合的普通圍欄都切；自閉合不吞正文
#[test]
fn aborted_narration_cuts_unclosed_tails() {
    for (reply, expected) in [
        ("夜深了。<UpdateVariable>{\"a\"", "夜深了。"),
        ("夜深了。<Upd", "夜深了。"),
        ("夜深了。\n```\ncode", "夜深了。"),
        (
            "<status/>夜深了。<status>hp: 3</status>雨停了",
            "夜深了。雨停了",
        ),
    ] {
        assert_eq!(aborted_narration(reply).text, expected, "{reply}");
    }
}
