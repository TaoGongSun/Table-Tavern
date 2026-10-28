//! greeting-context-st-parity：GM 線經假 claude CLI 實送——一般桌開場送原文（狀態欄在），
//! 接管桌與狀態欄退場桌照 text。素材照 `commands::chat::gm_lane_reply` 的組法從真桌讀。

use super::*;
use crate::chat_assembly;

const OPENING: &str =
    "風雪拍打著城門。\n\n<details>\n<summary>状态栏</summary>\n- 驻守天数：第 1 天\n</details>";

#[derive(Clone, Copy, Debug)]
enum Table {
    Plain,
    Takeover,
    Retired,
}

/// 建桌、貼開場、跑一輪 GM lane，回傳假 CLI 收到的 prompt。
async fn sent_prompt(table: Table) -> String {
    let fake = fake_claude(&format!("opening-raw-{table:?}"));
    let world_id = data::create_world(&fake.root, "要塞").unwrap();
    data::append_opening(
        &fake.root,
        &world_id,
        0,
        "2026-10-28T00:00:00.000Z",
        OPENING,
        &transport::extract_state_block(OPENING),
        "阿濤",
    )
    .unwrap();
    match table {
        Table::Plain => {}
        Table::Takeover => {
            data::write_interface_shell(&fake.root, &world_id, "<UI>{{本回合.正文}}</UI>").unwrap()
        }
        Table::Retired => {
            let mut state = data::read_state(&fake.root, &world_id).unwrap();
            state.statusbar_retired = true;
            data::write_state(&fake.root, &world_id, &state).unwrap();
        }
    }
    let materials = chat_assembly::gm_materials(&fake.root, &world_id).unwrap();
    let scan = chat_assembly::test_gm_scan(&fake.root, &world_id, &materials, "zh-TW");
    let (scope, _) = chat_assembly::gm_scope(&materials);
    let (frozen, turn) =
        chat_assembly::gm_lane_parts(&materials, &scan, &scope, "請推進劇情。", "zh-TW");
    let mut input = turn_input(&materials.events, 0);
    input.lane = Lane::Gm;
    input.frozen_system = frozen;
    input.tail = turn.tail;
    input.prefix = None;
    input.echo = ReplyEcho::Narration;
    input.opening_raw = materials.opening.raw_history;
    run_turn(&fake.call, &fake.root, &world_id, input, None, |_| {})
        .await
        .unwrap();
    let calls: Vec<serde_json::Value> =
        std::fs::read_to_string(fake.session_dir.join("calls.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
    assert_eq!(calls.len(), 1);
    let prompt = calls[0]["prompt"].as_str().unwrap().to_owned();
    std::fs::remove_dir_all(&fake.dir).unwrap();
    prompt
}

#[cfg(unix)]
#[tokio::test]
async fn gm_lane_sends_raw_opening_only_on_plain_tables() {
    let _serial = crate::inflight::lock_real_process_tests();
    let plain = sent_prompt(Table::Plain).await;
    assert!(plain.contains(&format!("GM：{OPENING}")), "{plain}");
    // GM 線旁白標「GM：」，不再出現會被照抄的「（旁白）」（narrator-prefix-echo）
    assert!(!plain.contains("（旁白）"), "{plain}");
    assert!(plain.contains("<summary>状态栏</summary>"), "{plain}");
    for table in [Table::Takeover, Table::Retired] {
        let prompt = sent_prompt(table).await;
        assert!(prompt.contains("風雪拍打著城門"), "{table:?}：{prompt}");
        assert!(!prompt.contains("<details>"), "{table:?}：{prompt}");
    }
}
