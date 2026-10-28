#[allow(unused_imports)]
use super::super::arrivals::*;
#[allow(unused_imports)]
use super::super::assemble::*;
#[allow(unused_imports)]
use super::super::client::*;
#[allow(unused_imports)]
use super::super::context::*;
#[allow(unused_imports)]
use super::super::messages::*;
#[allow(unused_imports)]
use super::super::state_view::*;
#[allow(unused_imports)]
use super::super::test_support::{card, event, worldbook_entry};
#[allow(unused_imports)]
use super::super::turns::*;
use super::*;
#[allow(unused_imports)]
use crate::data::{
    self, AppConfig, CharacterCard, DataResult, FieldKind, FieldRule, InjectLevel, Mechanism,
    StateNode, TableState, Tier, TranscriptEvent, TranscriptKind, Visibility, WorldbookEntry,
};
#[allow(unused_imports)]
use crate::mechanism;
#[allow(unused_imports)]
use std::collections::{BTreeMap, BTreeSet};

/// 卡片格式導演指示只給「卡有顯示腳本、介面沒被 App 接管」的桌；有骨架的桌走接管指示（正文＋
/// UpdateVariable），角色優先桌與沒有顯示腳本的桌走一般旁白。
#[test]
fn gm_turn_format_follows_shell_and_mode() {
    use GmTurnFormat::*;
    // 沒重構過的卡（含 MVU 桌：開了增量但沒有骨架）照卡片格式
    assert_eq!(gm_turn_format(true, false, None), CardFormat);
    assert_eq!(gm_turn_format(true, false, Some("interface")), CardFormat);
    // 有骨架＝App 接管，不管卡有沒有顯示腳本
    assert_eq!(
        gm_turn_format(true, true, Some("interface")),
        InterfaceTakeover
    );
    assert_eq!(gm_turn_format(false, true, None), InterfaceTakeover);
    // 角色優先桌、卡片沒有顯示腳本都走一般旁白
    assert_eq!(gm_turn_format(true, true, Some("characters")), Narration);
    assert_eq!(gm_turn_format(true, false, Some("characters")), Narration);
    assert_eq!(gm_turn_format(false, false, None), Narration);
}

/// 接管指示要 UpdateVariable、不要 state 圍欄；只走 GM 敘事線：不點名、名單角色由 GM 演
#[test]
fn takeover_instruction_asks_for_update_block_not_state_fence() {
    for lang in ["zh-TW", "en"] {
        let text = takeover_instruction(lang).content;
        assert!(text.contains("<UpdateVariable>"));
        assert!(!text.contains("present"));
        assert!(!text.contains("time"));
        assert!(!text.contains("下一位") && !text.contains("Next:"));
        assert!(!text.contains(PLAYER_SENTINEL));
        let closing = gm_closing(GmTurnFormat::InterfaceTakeover, true, lang);
        assert!(!closing.contains("下一位") && !closing.contains("Next:"));
    }
    assert!(takeover_instruction("zh-TW")
        .content
        .contains("登場角色」名單上的角色）都由你扮演"));
}

/// 卡片自帶介面時的導演指示：點名世界書那條格式規定的標題，且不再要求舊版的
/// state 圍欄／下一位點名——那是兩邊指令打架的根因。
#[test]
fn card_format_instruction_points_to_worldbook_entry_and_drops_old_format_asks() {
    let zh_with_title = card_format_instruction("zh-TW", Some("回复规则")).content;
    assert!(zh_with_title.contains("回复规则"));
    assert!(!zh_with_title.contains("```state"));
    assert!(!zh_with_title.contains("下一位"));

    let zh_without_title = card_format_instruction("zh-TW", None).content;
    assert!(!zh_without_title.contains("見世界書"));

    let en_with_title = card_format_instruction("en", Some("Response Rules")).content;
    assert!(en_with_title.contains("Response Rules"));
    assert!(!en_with_title.contains("```"));
    assert!(!en_with_title.contains("Next:"));

    let en_without_title = card_format_instruction("en", None).content;
    assert!(!en_without_title.contains("see the worldbook entry"));
}

#[test]
fn extract_next_speaker_strips_trailing_line_and_tolerates_variants() {
    // 標準形：旁白＋狀態欄剝除後尾端剩點名行
    let (name, display) = extract_next_speaker("夜更深了。\n下一位：狐狸");
    assert_eq!(name.as_deref(), Some("狐狸"));
    assert_eq!(display, "夜更深了。");
    // 英文標記與半形冒號、前後空白
    let (name, display) = extract_next_speaker("The night deepens.\nNext:  Fox ");
    assert_eq!(name.as_deref(), Some("Fox"));
    assert_eq!(display, "The night deepens.");
    // 玩家哨兵原文帶回，由 pick_speaker 對回代號
    let (name, _) = extract_next_speaker(format!("門開了。\n下一位：{PLAYER_SENTINEL}").as_str());
    assert_eq!(name.as_deref(), Some(PLAYER_SENTINEL));
    // 沒有點名行＝原樣返回；行首是普通英文 Next 不誤判
    let plain = "夜更深了。\nNext, the door opened.";
    assert_eq!(extract_next_speaker(plain), (None, plain.to_owned()));
}

/// 驗收：換幕順手取幕名——有標題行／無標題行／en 前綴，都不能報錯
#[test]
fn extract_scene_title_reads_zh_and_en_prefixes_and_falls_back_without_one() {
    let (title, rest) = extract_scene_title("標題：酒館夜話\n\n地點與時間：酒館");
    assert_eq!(title.as_deref(), Some("酒館夜話"));
    assert_eq!(rest, "地點與時間：酒館");

    let (title_en, rest_en) = extract_scene_title("Title: Tavern Talk\n\nLocation: the tavern");
    assert_eq!(title_en.as_deref(), Some("Tavern Talk"));
    assert_eq!(rest_en, "Location: the tavern");

    // 大小寫寬鬆
    let (title_mixed, _) = extract_scene_title("title: Mixed Case\n\nbody");
    assert_eq!(title_mixed.as_deref(), Some("Mixed Case"));

    // 沒有標題行：整段原文當摘要，不報錯
    let (none_title, whole) = extract_scene_title("地點與時間：酒館\n關鍵事件：無");
    assert_eq!(none_title, None);
    assert_eq!(whole, "地點與時間：酒館\n關鍵事件：無");
}

/// 英文骨架下的非中文回覆：state 圍欄鍵、`Next:` 點名、`Title:` 幕名都照固定標記解析，
/// 內容是該語系母語也一樣；骨架也不把固定標記翻成該語系。
#[test]
fn english_scaffold_markers_parse_native_language_replies() {
    let roster = vec!["Renard".to_owned()];
    for (lang, body, place, title) in [
        ("ja", "雨が止んだ。", "旧港", "酒場の夜"),
        ("fr", "La pluie cesse.", "Vieux port", "Nuit à la taverne"),
        ("ru", "Дождь стих.", "Старый порт", "Ночь в таверне"),
    ] {
        let instruction = narrate_instruction(lang, &roster, None, false, false).content;
        assert!(instruction.contains("`Next: <name>`") && instruction.contains("```state"));
        let reply = format!(
            "{body}\n```state\ntime: 0:00\nplace: {place}\npresent: Renard\n```\nNext: {PLAYER_SENTINEL}"
        );
        let StateBlock {
            fields, display, ..
        } = extract_state_block(&reply);
        assert_eq!(fields[1], (vec!["place".to_owned()], place.to_owned()));
        let (name, display) = extract_next_speaker(&display);
        assert_eq!(name.as_deref(), Some(PLAYER_SENTINEL));
        assert_eq!(display, body);
        assert_eq!(
            pick_speaker(&name.unwrap(), &roster, None).unwrap(),
            PLAYER_SENTINEL
        );

        let (parsed, rest) = extract_scene_title(&format!("Title: {title}\n\n- {body}"));
        assert_eq!(parsed.as_deref(), Some(title));
        assert_eq!(rest, format!("- {body}"));
    }
}

#[test]
fn pick_speaker_handles_exact_verbose_and_player_sentinel() {
    let roster = vec!["狐狸".to_owned(), "騎士".to_owned()];
    assert_eq!(pick_speaker("狐狸", &roster, None).unwrap(), "狐狸");
    assert_eq!(pick_speaker("「騎士」。", &roster, None).unwrap(), "騎士");
    assert_eq!(
        pick_speaker("玩家", &roster, None).unwrap(),
        PLAYER_SENTINEL
    );
    // 輪到玩家時 GM 怎麼講都算：代號本身、「玩家」、英文 Player、玩家卡上的名字
    for reply in [PLAYER_SENTINEL, "玩家", "Player", "阿濤"] {
        assert_eq!(
            pick_speaker(reply, &roster, Some("阿濤")).unwrap(),
            PLAYER_SENTINEL
        );
    }
    // 模型多話：取回覆中最先出現的候選名
    assert_eq!(
        pick_speaker("下一位應該由騎士發言，逼問狐狸。", &roster, None).unwrap(),
        "騎士"
    );
    assert_eq!(pick_speaker("酒保", &roster, None), None);
}

#[test]
fn extract_state_fence_returns_fields_and_hides_fence() {
    let StateBlock {
        fields, display, ..
    } = extract_state_block(
        "雨停了。\n```state\ntime: 午夜\nplace：舊碼頭\npresent: 阿濤、船長\n```",
    );
    assert_eq!(
        fields,
        vec![
            (vec!["time".to_owned()], "午夜".to_owned()),
            (vec!["place".to_owned()], "舊碼頭".to_owned()),
            (vec!["present".to_owned()], "阿濤、船長".to_owned()),
        ]
    );
    assert_eq!(display, "雨停了。");
}

/// 單一清單項底下帶子層 → 清單（`World/0/城市`）；項目裡清單項與具名鍵混用 → 照舊攤平成具名鍵，
/// 純量項丟掉。
#[test]
fn extract_state_block_collects_nested_yaml_and_flattens_mixed_items() {
    let block = extract_state_block(
        "<Status_block>World:\n  - 城市:\n      名稱: \"晨港\"\n      - 純清單項\n      人口: '1200'\n</Status_block>",
    );
    let path = |segments: &[&str]| segments.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(
        block.fields,
        vec![
            (path(&["World", "0", "城市", "名稱"]), "晨港".to_owned()),
            (path(&["World", "0", "城市", "人口"]), "1200".to_owned()),
        ]
    );
    assert_eq!(block.lists, vec![path(&["World"])]);
    assert!(block.maps.contains(&path(&["World", "0", "城市"])));
}

/// 縮排行與空字典都要保留成分支標記，葉子則保留實際值。
#[test]
fn parse_indented_fields_marks_branch_lines() {
    assert_eq!(
        parse_indented_fields("World:\n  Time: 清晨\n  Inventory: {}"),
        vec![
            (vec!["World".to_owned()], None),
            (
                vec!["World".to_owned(), "Time".to_owned()],
                Some("清晨".to_owned()),
            ),
            (vec!["World".to_owned(), "Inventory".to_owned()], None),
        ]
    );
}

#[test]
fn extract_state_discards_bad_lines_without_losing_valid_fields() {
    let StateBlock {
        fields, display, ..
    } = extract_state_block("旁白\n```state\n- time: 清晨\n沒有冒號\nplace:   \n# 自訂：有效\n```");
    assert_eq!(
        fields,
        vec![
            (vec!["time".to_owned()], "清晨".to_owned()),
            (vec!["自訂".to_owned()], "有效".to_owned()),
        ]
    );
    assert_eq!(display, "旁白");
}

#[test]
fn extract_state_details_summary_is_parsed_and_hidden() {
    let StateBlock {
        fields, display, ..
    } = extract_state_block(
        "港口傳來鐘聲。<details><summary>状态栏</summary>时间：黃昏\n地点：港口</details>",
    );
    assert_eq!(
        fields,
        vec![
            (vec!["time".to_owned()], "黃昏".to_owned()),
            (vec!["place".to_owned()], "港口".to_owned()),
        ]
    );
    assert_eq!(display, "港口傳來鐘聲。");
}

#[test]
fn extract_status_tag_is_parsed_and_hidden() {
    let StateBlock {
        fields, display, ..
    } = extract_state_block("門開了。<STATUS>time: 午夜\nplace: 走廊</status>剩下的話。");
    assert_eq!(
        fields,
        vec![
            (vec!["time".to_owned()], "午夜".to_owned()),
            (vec!["place".to_owned()], "走廊".to_owned()),
        ]
    );
    assert_eq!(display, "門開了。剩下的話。");
}

#[test]
fn extract_update_variable_hides_json_without_parsing_it() {
    let StateBlock {
        fields,
        updates,
        display,
        ..
    } = extract_state_block("她點頭。<UpdateVariable>{\"time\":\"午夜\"}</UpdateVariable>");
    assert!(fields.is_empty());
    assert_eq!(updates, vec!["{\"time\":\"午夜\"}".to_owned()]);
    assert_eq!(display, "她點頭。");
}

/// 各家標籤名不同（donass 的 `<StatusData>`、鎮北王府的 `<Status_block>`），
/// 開頭是 status 就認；同名才配對，`<statusdata>` 不會被 `</status_block>` 收掉。
#[test]
fn extract_state_accepts_any_status_prefixed_tag() {
    let StateBlock {
        fields, display, ..
    } = extract_state_block(
        "地下城。<StatusData>体力:60\n好感:20</StatusData>之後。\
         <Status_block>时间: 戌时\n地点: 浴房</Status_block>",
    );
    assert_eq!(
        fields,
        vec![
            (vec!["体力".to_owned()], "60".to_owned()),
            (vec!["好感".to_owned()], "20".to_owned()),
            (vec!["time".to_owned()], "戌时".to_owned()),
            (vec!["place".to_owned()], "浴房".to_owned()),
        ]
    );
    assert_eq!(display, "地下城。之後。");
}

/// 沒有配對收尾的標籤整段留著：寧可讓玩家看到半截標籤，也不吞掉後面的旁白。
#[test]
fn extract_state_leaves_unclosed_status_tag_alone() {
    let reply = "他開口。<StatusData>体力:60\n後面還有很多話。";
    assert_eq!(
        extract_state_block(reply),
        StateBlock {
            fields: Vec::new(),
            updates: Vec::new(),
            display: reply.to_owned(),
            ..Default::default()
        }
    );
}

/// 名字裡帶 status 但不是開頭的（`<combatStatus>`）是卡片自訂欄位，不能當狀態區塊剝掉。
#[test]
fn extract_state_ignores_tags_merely_containing_status() {
    let reply = "他喘著氣。<combatStatus>負傷</combatStatus>";
    assert_eq!(
        extract_state_block(reply),
        StateBlock {
            fields: Vec::new(),
            updates: Vec::new(),
            display: reply.to_owned(),
            ..Default::default()
        }
    );
}

#[test]
fn extract_state_unwraps_maintext_into_narration() {
    let StateBlock {
        fields, display, ..
    } = extract_state_block(
        "<maintext>\n夜色濃重。\n</maintext>\n<Status_block>时间: 戌时</Status_block>",
    );
    assert_eq!(fields, vec![(vec!["time".to_owned()], "戌时".to_owned())]);
    assert_eq!(display, "\n夜色濃重。");
}

/// 只有正文外殼、沒有狀態區塊時，一樣要拆掉外殼。
#[test]
fn extract_state_unwraps_maintext_without_state_block() {
    let StateBlock {
        fields, display, ..
    } = extract_state_block("<mainText>夜色濃重。</mainText>");
    assert!(fields.is_empty());
    assert_eq!(display, "夜色濃重。");
}

#[test]
fn extract_state_keeps_unwrapped_narration_byte_for_byte() {
    let reply = "純旁白\n保留尾端空行\n\n";
    assert_eq!(
        extract_state_block(reply),
        StateBlock {
            fields: Vec::new(),
            updates: Vec::new(),
            display: reply.to_owned(),
            ..Default::default()
        }
    );
}

#[test]
fn extract_state_keeps_middle_code_fence_but_removes_trailing_plain_fence() {
    let reply = "提示：\n```rust\nlet time = 1;\n```\n旁白\n```\ntime: 午夜\n```";
    let StateBlock {
        fields, display, ..
    } = extract_state_block(reply);
    assert_eq!(fields, vec![(vec!["time".to_owned()], "午夜".to_owned())]);
    assert_eq!(display, "提示：\n```rust\nlet time = 1;\n```\n旁白");
}

/// 增量的旁白桌：更新區塊的說明插在「下一位」那句之前，收尾句也允許更新區塊
#[test]
fn narration_with_updates_asks_for_the_block_before_the_next_speaker_line() {
    for lang in ["zh-TW", "en"] {
        let text = narrate_instruction(lang, &["狐狸".to_owned()], None, true, false).content;
        let note = text.find("UpdateVariable").expect(lang);
        let next = text
            .find(if lang == "en" { "Next:" } else { "下一位" })
            .expect(lang);
        assert!(note < next, "{lang}");
        assert!(!narrate_instruction(lang, &[], None, false, false)
            .content
            .contains("UpdateVariable"));
        assert!(
            narration_closing_with_updates(true, lang).contains(if lang == "en" {
                "update block"
            } else {
                "更新區塊"
            })
        );
    }
}

/// present 指示：每輪重判、名冊只供辨識、可留空；名字照原字（語言規範的例外，顯示字形由 app 轉）
#[test]
fn narration_present_rule_recomputes_each_turn_and_keeps_original_names() {
    let zh = narrate_instruction("zh-CN", &[], None, false, false).content;
    assert!(zh.contains("time、place 用玩家的語言寫，沒有變化就照抄目前值"));
    assert!(zh.contains("以旁白結束當下為準") && zh.contains("人物名冊只供辨識是誰"));
    assert!(
        zh.contains("「present:」後面留空") && zh.contains("不換簡繁字形（這是語言規範的例外）")
    );
    assert!(!zh.contains("沒有變化的欄位也照抄目前值"));
    for lang in ["en", "ja", "ru"] {
        let en = narrate_instruction(lang, &[], None, false, false).content;
        assert!(
            en.contains("repeat them if unchanged") && en.contains("at the end of the narration")
        );
        assert!(
            en.contains("it is not a list of who is here") && en.contains("leave `present:` empty")
        );
        assert!(
            en.contains("exception to the language rule")
                && en.contains("no translation, transliteration, or script change")
        );
        assert!(!en.contains("repeat unchanged values too"));
    }
}

/// 在場人物寫成裸空值＝沒有人在場：進平欄成空字串；時地與其他鍵照舊只標空分支
#[test]
fn bare_empty_present_clears_the_table_field_but_other_keys_keep_their_meaning() {
    let StateBlock { fields, maps, .. } =
        extract_state_block("人都走了。\n```state\ntime: 夜\n地點:\n在場人物:\n状态栏:\n```");
    assert_eq!(
        fields,
        vec![
            (vec!["time".to_owned()], "夜".to_owned()),
            (vec!["present".to_owned()], String::new()),
        ]
    );
    assert_eq!(
        maps,
        vec![vec!["地點".to_owned()], vec!["状态栏".to_owned()]]
    );
}

/// gm-no-state-fence-worldbook-table：世界書自帶狀態欄（```json 包的四段）與 ```state 圍欄並存時，
/// 抽取只拿走圍欄與點名行，卡片狀態欄原樣留在顯示文字（樣本取自 Haiku 實測輸出，節錄）。
#[test]
fn worldbook_status_blocks_survive_state_extraction() {
    let reply = "接待員把登記冊推到一旁：「新人？從這幾件開始吧。」\n\n\
                 ```json\n💖场景信息栏\n\n时间：上午｜地点：边境小镇冒险者公会\n剧情：目标（完成首个委托）\n```\n\n---\n\n\
                 ```json\n推荐行动：接下草药委托。\n推荐行动：打听北方商路。\n```\n\n---\n\n\
                 ```json\n💙玩家属性栏\n💫玩家｜待设定\n```\n\n---\n\n\
                 ```json\n💜互动角色状态栏\n💫艾琳｜女｜人族｜19岁｜见习法师\n```\n\n\
                 ```state\ntime: 上午\nplace: 边境小镇冒险者公会\npresent: 艾琳\n```\n\n下一位：艾琳";
    let (next, without_next) = extract_next_speaker(reply);
    assert_eq!(next.as_deref(), Some("艾琳"));
    let block = extract_state_block(&without_next);
    let field = |key: &str| {
        block
            .fields
            .iter()
            .find(|(path, _)| path == &vec![key.to_owned()])
            .map(|(_, value)| value.as_str())
    };
    assert_eq!(field("time"), Some("上午"));
    assert_eq!(field("place"), Some("边境小镇冒险者公会"));
    assert_eq!(field("present"), Some("艾琳"));
    for header in [
        "💖场景信息栏",
        "推荐行动：接下草药委托。",
        "💙玩家属性栏",
        "💜互动角色状态栏",
    ] {
        assert!(block.display.contains(header), "{header}");
    }
    assert!(!block.display.contains("```state"));
    assert!(!block.display.contains("present:"));
}

/// 卡片狀態欄退場的桌：導演指示不再舉「狀態欄」當設定要附的內容，改明說已淘汰不要印；其餘句子不變
#[test]
fn narration_on_retired_statusbar_table_says_not_to_print_status_bar() {
    for (lang, retired_text, active_text) in [
        (
            "zh-TW",
            "卡片原本的狀態欄已淘汰",
            "（例如狀態欄、推薦行動）",
        ),
        (
            "en",
            "status bar has been retired",
            "(such as a status bar or suggested actions)",
        ),
    ] {
        let active = narrate_instruction(lang, &["狐狸".to_owned()], None, true, false).content;
        let retired = narrate_instruction(lang, &["狐狸".to_owned()], None, true, true).content;
        assert!(active.contains(active_text) && !active.contains(retired_text));
        assert!(retired.contains(retired_text) && !retired.contains(active_text));
        assert_eq!(
            active.replace(narration_extra_content(lang, false), ""),
            retired.replace(narration_extra_content(lang, true), "")
        );
    }
}
