//! 來回測試：網頁版真匯出的存檔（`web-export.json`，由 web/e2e 以 `TT_WEB_EXPORT_OUT` 另存：範例卡、
//! 幾輪假端點對話、停止並保留的中斷回覆、玩家句裡 setvar 的聊天變數）匯進桌面版：逐字稿照存檔落地、
//! 下一輪角色線的歷史帶得到整段對話；ST 聊天變數只落在 chat 層檔案。網頁版包 6 之前不產生 MVU 種子與
//! 每則變數表，桌面版維持「尚無表」，下一輪提示讀的是桌狀態、不讀 chat 層——變數進提示等包 6。
//! `web-export-world-info.json`（`TT_WEB_EXPORT_WI_OUT`）：帶世界書的卡玩兩輪（sticky 觸發後跨回合）的真匯出，
//! 桌面版只保存觸發狀態（旁檔原樣＋映射表），消費等 worldbook-st-trigger-parity。
use super::tests::{bytes, character_turn, fixture, layer_json, mode_of};
use super::*;
use crate::import::test_support::TestRoot;
use serde_json::json;

const LANG: &str = "zh-TW";

#[test]
fn a_real_web_export_puts_transcript_in_history_and_chat_vars_in_the_chat_layer() {
    let root = TestRoot::new("web-save-real-export");
    let save = fixture("web-export.json");
    let imported = import_web_save(root.path(), &bytes(&save), LANG).unwrap();
    let w = imported.world_id.clone();
    let char_id = imported.character_id.clone().expect("範例卡走角色卡路");
    assert_eq!(
        data::read_character(root.path(), &w, &char_id)
            .unwrap()
            .name,
        "瑟拉"
    );
    assert_eq!(
        data::read_player_card(root.path(), &w)
            .unwrap()
            .unwrap()
            .name,
        "玩家"
    );

    // 逐字稿：每則的文字、說話者、中斷旗標照存檔
    let messages = save["messages"].as_array().unwrap();
    let events = data::read_transcript(root.path(), &w, 0).unwrap();
    assert_eq!(events.len(), messages.len());
    for (event, message) in events.iter().zip(messages) {
        assert_eq!(event.text, message["text"].as_str().unwrap());
        assert_eq!(event.ts, message["ts"].as_str().unwrap());
        assert_eq!(
            event.truncated,
            message["interrupted"].as_bool().unwrap_or(false)
        );
        let expected = match (message["role"].as_str().unwrap(), event.opening) {
            ("user", _) => TranscriptKind::Player,
            (_, true) => TranscriptKind::Narration,
            _ => TranscriptKind::Dialogue,
        };
        assert_eq!(event.kind, expected);
    }
    assert!(events[0].opening);
    assert!(events[4].truncated, "停止並保留的那則標回應中斷");
    assert_eq!(events[5].text, "我付了錢");

    // 變數：玩家句的 setvar 落在 chat 層檔案；存檔沒有 MVU 種子與每則變數表，桌面版不進變數模式
    assert_eq!(
        layer_json(&root, &w, Layer::Chat, None),
        json!({"錢包": "15"})
    );
    assert!(events.iter().all(|event| event.message_vars.is_none()));
    assert_ne!(mode_of(&root, &w), json!("events"), "沒有種子不進變數模式");
    // 卡片 storage：網頁版這張卡沒有介面，空的照樣交回
    assert!(imported.card_storage.is_empty());
    assert_eq!(imported.shared_kept, 0);

    // 旁檔：每則訊息都對到桌面事件
    let sidecar: Value = serde_json::from_slice(
        &std::fs::read(data::web_save_sidecar_path(root.path(), &w).unwrap()).unwrap(),
    )
    .unwrap();
    for (event, message) in events.iter().zip(messages) {
        assert_eq!(
            sidecar["message_ids"][message["id"].as_str().unwrap()],
            json!(event.id.clone().unwrap())
        );
    }
    assert_eq!(sidecar["regex_allowed"], json!(false));
    assert!(!data::read_state(root.path(), &w).unwrap().regex_allowed);

    // 接著玩：下一輪角色線的歷史帶得到最後一句玩家句與最後一則回覆；chat 層的變數不在提示裡（包 6）
    let sent = character_turn(&root, &w, &char_id);
    let history: String = sent.iter().map(|message| message.content.clone()).collect();
    assert!(history.contains("我付了錢"), "{history}");
    assert!(history.contains(events[6].text.as_str()));
    assert!(!history.contains("錢包"), "chat 層變數目前不進提示");
}

#[test]
fn a_real_web_export_with_world_info_keeps_the_trigger_state_in_the_sidecar() {
    let root = TestRoot::new("web-save-real-export-wi");
    let save = fixture("web-export-world-info.json");
    // 這份真匯出確實帶著 sticky 與冷卻（不是空結構）
    assert_eq!(
        save["world_info"]["timed"]["sticky"]["wi-0"],
        json!({"start": 2, "end": 5, "protected": false})
    );
    let imported = import_web_save(root.path(), &bytes(&save), LANG).unwrap();
    let w = imported.world_id.clone();
    let sidecar: Value = serde_json::from_slice(
        &std::fs::read(data::web_save_sidecar_path(root.path(), &w).unwrap()).unwrap(),
    )
    .unwrap();
    // 觸發狀態原樣保存
    assert_eq!(sidecar["world_info"], save["world_info"]);
    // 穩定 ID → 桌面 UID：每條都對得到桌上的條目；sticky 條目不會被改成常駐
    let book = data::read_worldbook(root.path(), &w).unwrap();
    for (stable, content, constant) in [
        ("wi-0", "雪崩過後三天內山路封閉。", false),
        ("wi-1", "嚮導從不在夜裡出發。", true),
        ("wi-2", "山腰有狼群。", false),
    ] {
        let entry = book.iter().find(|entry| entry.content == content).unwrap();
        assert_eq!(sidecar["entry_uids"][stable], json!(entry.uid), "{stable}");
        assert_eq!(entry.constant, constant, "{stable}");
    }
    // 計時算到的那則對得到桌面事件
    let events = data::read_transcript(root.path(), &w, 0).unwrap();
    let last = save["world_info"]["last_message_id"].as_str().unwrap();
    assert_eq!(
        sidecar["message_ids"][last],
        json!(events[3].id.clone().unwrap())
    );
}
