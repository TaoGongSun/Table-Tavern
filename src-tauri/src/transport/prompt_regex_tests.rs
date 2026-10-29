//! 樣本是 TestCards 裡實際出現的三種 promptOnly 形狀（盤點見 .ai/plans/card-prompt-only-regex.md）。

use super::*;
use crate::transport::test_support::event;
use serde_json::json;

const MVU: &str =
    r"/<(update(?:variable)?)>(?:(?!.*<\/\1>)(?:(?!<\1>).)*$|(?:(?!<\1>).)*<\/\1?>)/gsi";
const NORTH_HALL: &str = r".*?<Status_block>([\s\S]*?)<\/Status_block>";
const RPG_STATUS: &str = r"<Status_block>[\s\S]*?<\/Status_block>";

fn script(find: &str, placement: &[i64], min_depth: Option<i64>) -> Value {
    json!({
        "scriptName": "樣本", "findRegex": find, "replaceString": "", "trimStrings": [],
        "placement": placement, "disabled": false, "markdownOnly": true, "promptOnly": true,
        "substituteRegex": 0, "minDepth": min_depth, "maxDepth": null
    })
}

fn regex(scripts: &[Value]) -> PromptRegex {
    PromptRegex::from_scripts(scripts)
}

#[test]
fn mvu_strips_closed_and_unclosed_updates_from_both_sides() {
    let mvu = regex(&[script(MVU, &[1, 2], None)]);
    let reply =
        "她點頭。\n<UpdateVariable>\n_.set('錢', 3);\n</UpdateVariable>\n離開。<update>半截";
    assert_eq!(
        mvu.apply(&TranscriptKind::Narration, reply.to_owned(), 0),
        "她點頭。\n\n離開。"
    );
    assert_eq!(
        mvu.apply(
            &TranscriptKind::Player,
            "我<updatevariable>x</updatevariable>走".to_owned(),
            0
        ),
        "我走"
    );
    assert_eq!(
        mvu.apply(
            &TranscriptKind::System,
            "<UpdateVariable>x</UpdateVariable>".to_owned(),
            0
        ),
        "<UpdateVariable>x</UpdateVariable>",
        "系統事件不套"
    );
}

/// 無斜線＝無旗標：只剝第一個，`.*?` 不跨行（吃掉同一行的前文）。只作用於 AI 輸出。
#[test]
fn north_hall_bare_pattern_strips_first_block_only() {
    let north = regex(&[script(NORTH_HALL, &[2], None)]);
    let reply = "風起。\n前綴<Status_block>\n時間：夜\n</Status_block>\n後文<Status_block>二</Status_block>";
    assert_eq!(
        north.apply(&TranscriptKind::Narration, reply.to_owned(), 5),
        "風起。\n\n後文<Status_block>二</Status_block>"
    );
    let said = "<Status_block>玩家</Status_block>";
    assert_eq!(
        north.apply(&TranscriptKind::Player, said.to_owned(), 5),
        said
    );
}

/// minDepth 2：最近兩則（深度 0、1）保留，更舊的剝掉；深度不算系統事件。
#[test]
fn rpg_min_depth_keeps_the_latest_two_messages() {
    let rpg = regex(&[
        script(RPG_STATUS, &[2], Some(2)),
        script(r"<BGM>[\s\S]*?<\/BGM>", &[2], Some(2)),
    ]);
    let reply = || "戰鬥。<Status_block>HP 3</Status_block><BGM>鼓</BGM>".to_owned();
    assert_eq!(rpg.apply(&TranscriptKind::Narration, reply(), 1), reply());
    assert_eq!(rpg.apply(&TranscriptKind::Narration, reply(), 2), "戰鬥。");

    let events = [
        event(TranscriptKind::Narration, "", "", "舊"),
        event(TranscriptKind::System, "", "", "登場"),
        event(TranscriptKind::Narration, "", "", "近"),
        event(TranscriptKind::Player, "", "", "說"),
    ];
    assert_eq!(depths(&events), vec![2, 2, 1, 0]);
}

#[test]
fn unsupported_shapes_are_skipped() {
    let mut replaced = script(RPG_STATUS, &[2], None);
    replaced["replaceString"] = json!("[狀態]");
    let mut disabled = script(RPG_STATUS, &[2], None);
    disabled["disabled"] = json!(true);
    let mut display = script(RPG_STATUS, &[2], None);
    display["promptOnly"] = json!(false);
    let skipped = regex(&[replaced, disabled, display]);
    let text = "<Status_block>x</Status_block>";
    assert_eq!(
        skipped.apply(&TranscriptKind::Narration, text.to_owned(), 9),
        text
    );
}
