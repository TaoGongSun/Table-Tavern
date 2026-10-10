//! char-line-status-strip：網頁訊息照 ST 存原文（MVU 佔位、`<UpdateVariable>` 都在 text），匯進桌面版時
//! 模型回的樓照桌面版收尾剝乾淨，原文留在 raw；中斷的樓照中止規則切尾巴。
use super::tests::{bytes, fixture};
use super::*;
use crate::import::test_support::TestRoot;
use serde_json::json;

const LANG: &str = "zh-TW";
const TAG: &str = "<UpdateVariable>_.set('錢包', 30, 25);</UpdateVariable>";

fn import(root: &TestRoot, save: &Value) -> Vec<TranscriptEvent> {
    let imported = import_web_save(root.path(), &bytes(save), LANG).unwrap();
    data::read_transcript(root.path(), &imported.world_id, 0).unwrap()
}

#[test]
fn character_route_messages_are_stripped_and_raw_kept() {
    let root = TestRoot::new("web-save-strip");
    let mut save = fixture("short.json");
    save["messages"][4]["text"] = json!("「麥酒十個銅板。」她把杯子推過來<Upd");
    let events = import(&root, &save);
    // 開場（網頁沒給 raw）：佔位剝掉，原文當 raw
    assert_eq!(
        events[0].text,
        "「歡迎光臨，旅人。」灰燼旅店的莫拉 放下帳本。"
    );
    assert_eq!(
        events[0].raw.as_deref(),
        save["messages"][0]["text"].as_str()
    );
    // 已有 raw：用網頁的 raw
    assert_eq!(events[2].text, "「晚安，親愛的。來碗熱湯？五個銅板。」");
    assert_eq!(
        events[2].raw.as_deref(),
        save["messages"][2]["raw"].as_str()
    );
    // 中斷的樓：照中止規則切掉標記前半
    assert!(events[4].truncated);
    assert_eq!(events[4].text, "「麥酒十個銅板。」她把杯子推過來");
    // 玩家句不動
    assert_eq!(events[1].text, "（揮手） 晚安");
    assert_eq!(events[1].raw, None);

    // 沒標中斷：照完成規則，標記前半保留
    let mut save = fixture("short.json");
    save["messages"][4]["text"] = json!("「麥酒十個銅板。」<Upd");
    save["messages"][4]["interrupted"] = json!(false);
    let events = import(&TestRoot::new("web-save-strip-complete"), &save);
    assert_eq!(events[4].text, "「麥酒十個銅板。」<Upd");
    assert_eq!(events[4].raw, None);
}

#[test]
fn control_only_message_lands_as_empty_text() {
    let root = TestRoot::new("web-save-strip-empty");
    let mut save = fixture("short.json");
    save["messages"][2]["text"] = json!(format!("<StatusPlaceHolderImpl/>\n{TAG}"));
    save["messages"][2].as_object_mut().unwrap().remove("raw");
    let events = import(&root, &save);
    assert_eq!(events[2].text, "");
    assert_eq!(
        events[2].raw.as_deref(),
        save["messages"][2]["text"].as_str()
    );
}

#[test]
fn worldbook_route_narration_is_stripped_too() {
    let root = TestRoot::new("web-save-strip-gm");
    let events = import(&root, &fixture("worldbook-route.json"));
    assert_eq!(events[2].kind, TranscriptKind::Narration);
    assert_eq!(events[2].text, "「晚安，親愛的。來碗熱湯？五個銅板。」");
    assert!(events[2]
        .raw
        .as_deref()
        .is_some_and(|raw| raw.contains("<UpdateVariable>")));
    assert_eq!(
        events[0].text,
        "「歡迎光臨，旅人。」灰燼旅店的莫拉 放下帳本。"
    );
}

#[test]
fn nested_details_and_exact_raw_on_import() {
    let root = TestRoot::new("web-save-strip-nested");
    let mut save = fixture("short.json");
    // 中斷的樓停在巢狀 details 外層裡：照中止規則從外層切
    save["messages"][4]["text"] =
        json!("「麥酒十個銅板。」<details><summary>其他</summary><details>內文</details>外層半截");
    // 完成的樓只差頭尾空白：text 去空白、原文照樣留在 raw
    save["messages"][2]["text"] = json!("  「晚安。」  ");
    save["messages"][2].as_object_mut().unwrap().remove("raw");
    let events = import(&root, &save);
    assert_eq!(events[4].text, "「麥酒十個銅板。」");
    assert_eq!(events[2].text, "「晚安。」");
    assert_eq!(events[2].raw.as_deref(), Some("  「晚安。」  "));
}
