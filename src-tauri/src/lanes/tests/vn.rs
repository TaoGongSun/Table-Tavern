//! vn-mode 包 1：VN 桌角色線的預期回聲與落檔同一個收尾；送模型的歷史裡角色台詞不加名字前綴。

use super::*;

/// 回聲對帳與落檔（`chat_turn::finish_vn_character_reply`）逐字相同：前綴不剝、控制標記照剝
#[test]
fn vn_echo_matches_landed_text() {
    let reply =
        "狐狸: 「嗨」\n[bg: 港口]\n[choices]\n- 甲\n- 乙\n<UpdateVariable>x</UpdateVariable>";
    let echo = ReplyEcho::VnDialogue {
        speaker_id: "fox-id".to_owned(),
    };
    let expected = expected_reply_for(&echo, reply);
    let landed = crate::chat_turn::finish_vn_character_reply(reply, false).text;
    assert_eq!(expected.text, landed);
    assert_eq!(landed, "狐狸: 「嗨」");
    assert_eq!(expected.kind, TranscriptKind::Dialogue);
}

/// VN 桌：角色台詞原文直送（本文已自帶「名字 (表情):」），GM 旁白照舊標「GM：」、玩家照舊帶名字
#[test]
fn vn_history_skips_dialogue_prefix_only() {
    let events = [
        event(TranscriptKind::Narration, "", "GM", "海風吹過。"),
        event(
            TranscriptKind::Dialogue,
            "fox-id",
            "狐狸",
            "狐狸 (happy): 「嗨」",
        ),
        event(TranscriptKind::Player, "", "阿濤", "你好"),
    ];
    let render = |side, vn| {
        build_prompt_as(
            &events,
            0,
            "尾",
            false,
            side,
            "zh-TW",
            &Default::default(),
            vn,
        )
    };
    let vn = render(transport::Side::Gm, true);
    assert!(vn.contains("GM：海風吹過。"), "{vn}");
    assert!(vn.contains("\n\n狐狸 (happy): 「嗨」\n\n"), "{vn}");
    assert!(vn.contains("阿濤：你好"), "{vn}");
    assert!(render(transport::Side::Gm, false).contains("狐狸：狐狸 (happy)"));
    assert!(render(transport::Side::Character, true).contains("\n\n狐狸 (happy): 「嗨」\n\n"));
}

/// 角色回合尾：VN 桌改成「演誰＋沿用劇本格式」（lane 尾段與無狀態 messages 兩路），聊天桌照舊
#[test]
fn vn_character_tail_uses_script_format() {
    use crate::transport::test_support::legacy::prepared;
    let fox = crate::commands::character_card("fox-id", "狐狸");
    let cards = std::slice::from_ref(&fox);
    let scan = prepared(
        &[],
        crate::world_scan::Viewer::Character(&fox),
        "",
        cards,
        None,
        &[],
        "zh-TW",
    );
    let root = crate::smart_free::test_root("vn-char-tail");
    for (kind, vn) in [(data::TableKind::Vn, true), (data::TableKind::Chat, false)] {
        let world_id = data::create_world_of_kind(&root, "港口", kind).unwrap();
        let state = data::read_state(&root, &world_id).unwrap();
        let (_, lane) = crate::chat_assembly::character_lane_parts(
            &fox, cards, None, &scan, &state, None, "zh-TW", false,
        );
        let api = transport::assemble_shared_messages_with(
            &fox,
            cards,
            None,
            &[],
            &scan,
            &state.state,
            &state.mechanism,
            None,
            "zh-TW",
            &Default::default(),
            vn,
        );
        let api_tail = &api.last().unwrap().content;
        for tail in [&lane.tail, api_tail] {
            assert_eq!(tail.contains("照上面的視覺小說劇本格式"), vn, "{tail}");
            assert_eq!(tail.contains("不要加名字前綴"), !vn, "{tail}");
        }
    }
}
