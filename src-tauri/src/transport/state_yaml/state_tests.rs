//! statusbar-yaml-list-flatten：狀態區塊的 YAML 清單解析。
use super::{parse_state_fields, ParsedFields};
use crate::transport::response::{extract_state_block, StateBlock};

fn p(segments: &[&str]) -> Vec<String> {
    segments.iter().map(|segment| segment.to_string()).collect()
}

fn field(segments: &[&str], value: &str) -> (Vec<String>, String) {
    (p(segments), value.to_owned())
}

/// 北境驿站形狀（中性文字自寫）：物件清單三項、純量清單四項。
const ROSTER: &str = "状态栏:
  地点: \"驿站 前院\"
  人物列表:
    - 人物:
        名字: \"林远\"
        身份: \"驿丞\"
    - 人物:
        名字: \"贺铮\"
        身份: \"老驿长\"
    - 人物:
        名字: \"周掌柜\"
        身份: \"军需官\"
  行动选项:
    名字: \"林远\"
    选项:
      - \"1. 去驿丞房安顿\"
      - \"2. 核对马匹：先看马厩\"
      - 3. 去门外看商队
      - '4. 向贺铮请教'
";

#[test]
fn object_and_scalar_lists_keep_every_item() {
    let parsed = parse_state_fields(ROSTER);
    assert_eq!(
        parsed.fields,
        vec![
            field(&["状态栏", "地点"], "驿站 前院"),
            field(&["状态栏", "人物列表", "0", "人物", "名字"], "林远"),
            field(&["状态栏", "人物列表", "0", "人物", "身份"], "驿丞"),
            field(&["状态栏", "人物列表", "1", "人物", "名字"], "贺铮"),
            field(&["状态栏", "人物列表", "1", "人物", "身份"], "老驿长"),
            field(&["状态栏", "人物列表", "2", "人物", "名字"], "周掌柜"),
            field(&["状态栏", "人物列表", "2", "人物", "身份"], "军需官"),
            field(&["状态栏", "行动选项", "名字"], "林远"),
            field(&["状态栏", "行动选项", "选项", "0"], "1. 去驿丞房安顿"),
            field(
                &["状态栏", "行动选项", "选项", "1"],
                "2. 核对马匹：先看马厩"
            ),
            field(&["状态栏", "行动选项", "选项", "2"], "3. 去门外看商队"),
            field(&["状态栏", "行动选项", "选项", "3"], "4. 向贺铮请教"),
        ]
    );
    assert_eq!(
        parsed.lists,
        vec![
            p(&["状态栏", "人物列表"]),
            p(&["状态栏", "行动选项", "选项"])
        ]
    );
}

#[test]
fn quoted_item_is_scalar_unless_a_colon_follows_the_closing_quote() {
    let parsed =
        parse_state_fields("名單:\n  - \"名字: 林远\"\n  - \"名字\": \"贺铮\"\n    身份: 驿长\n");
    assert_eq!(
        parsed.fields,
        vec![
            field(&["名單", "0"], "名字: 林远"),
            field(&["名單", "1", "名字"], "贺铮"),
            field(&["名單", "1", "身份"], "驿长"),
        ]
    );
}

#[test]
fn nested_lists_and_dash_on_its_own_line() {
    let parsed =
        parse_state_fields("格子:\n  - - a\n    - b\n  -\n    名字: 林远\n    身份: 驿丞\n");
    assert_eq!(
        parsed.fields,
        vec![
            field(&["格子", "0", "0"], "a"),
            field(&["格子", "0", "1"], "b"),
            field(&["格子", "1", "名字"], "林远"),
            field(&["格子", "1", "身份"], "驿丞"),
        ]
    );
    assert_eq!(parsed.lists, vec![p(&["格子"]), p(&["格子", "0"])]);
}

#[test]
fn continuation_lines_under_list_items_make_no_keys() {
    let parsed = parse_state_fields(
        "紀錄:\n  - 一段話\n      接著寫的續行\n  - 描述: |\n      第一行\n      第二行: 也不是鍵\n    名字: 林远\n  - |\n      整項多行\n      第二行\n",
    );
    assert_eq!(
        parsed.fields,
        vec![
            field(&["紀錄", "0"], "一段話"),
            field(&["紀錄", "1", "描述"], "第一行\n第二行: 也不是鍵"),
            field(&["紀錄", "1", "名字"], "林远"),
            field(&["紀錄", "2"], "整項多行\n第二行"),
        ]
    );
}

/// 無冒號的標題項底下有 `- ` 或 `鍵: 值`：當標題鍵（等同 `- 林远:`），粗體記號剝掉
#[test]
fn title_items_with_nested_entries_become_keys() {
    let root = parse_state_fields("- **林远**\n  - 体力: 50\n  - 心情: 警觉\n- 贺铮\n  体力: 70\n");
    assert_eq!(
        root.fields,
        vec![
            field(&["林远", "体力"], "50"),
            field(&["林远", "心情"], "警觉"),
            field(&["贺铮", "体力"], "70"),
        ]
    );
    let nested = parse_state_fields("状态栏:\n  - 林远\n    - 体力: 50\n  - 贺铮\n    体力: 70\n");
    assert_eq!(
        nested.fields,
        vec![
            field(&["状态栏", "0", "林远", "体力"], "50"),
            field(&["状态栏", "1", "贺铮", "体力"], "70"),
        ]
    );
    assert_eq!(nested.lists, vec![p(&["状态栏"])]);
}

/// 帶引號的 `"{}"`、`"[]"` 是字串，不是空容器
#[test]
fn quoted_braces_and_brackets_are_strings() {
    let parsed = parse_state_fields("名單:\n  - \"{}\"\n  - \"[]\"\n標記: \"[]\"\n空: \"{}\"\n");
    assert_eq!(
        parsed.fields,
        vec![
            field(&["名單", "0"], "{}"),
            field(&["名單", "1"], "[]"),
            field(&["標記"], "[]"),
            field(&["空"], "{}"),
        ]
    );
    assert_eq!(parsed.lists, vec![p(&["名單"])]);
}

#[test]
fn dashes_in_the_same_column_as_the_parent_key_belong_to_it() {
    let parsed = parse_state_fields(
        "人物:\n- 名字: 林远\n  身份: 驿丞\n- 名字: 贺铮\n  身份: 老驿长\n地点: 前院\n",
    );
    assert_eq!(
        parsed.fields,
        vec![
            field(&["人物", "0", "名字"], "林远"),
            field(&["人物", "0", "身份"], "驿丞"),
            field(&["人物", "1", "名字"], "贺铮"),
            field(&["人物", "1", "身份"], "老驿长"),
            field(&["地点"], "前院"),
        ]
    );
}

#[test]
fn single_key_scalar_items_with_distinct_keys_stay_named_keys() {
    let parsed = parse_state_fields("资源:\n  - 粮草: 400\n  - 马匹: 37\n");
    assert_eq!(
        parsed.fields,
        vec![
            field(&["资源", "粮草"], "400"),
            field(&["资源", "马匹"], "37")
        ]
    );
    assert!(parsed.lists.is_empty());
    assert_eq!(parsed.maps, vec![p(&["资源"])]);

    // 鍵名重複、多鍵物件、帶子層 → 清單
    for (block, expected) in [
        (
            "名單:\n  - 名字: 林远\n  - 名字: 贺铮\n",
            vec![
                field(&["名單", "0", "名字"], "林远"),
                field(&["名單", "1", "名字"], "贺铮"),
            ],
        ),
        (
            "名單:\n  - 名字: 林远\n    身份: 驿丞\n",
            vec![
                field(&["名單", "0", "名字"], "林远"),
                field(&["名單", "0", "身份"], "驿丞"),
            ],
        ),
        (
            "名單:\n  - 人物:\n      名字: 林远\n",
            vec![field(&["名單", "0", "人物", "名字"], "林远")],
        ),
    ] {
        let parsed = parse_state_fields(block);
        assert_eq!(parsed.fields, expected, "{block}");
        assert_eq!(parsed.lists, vec![p(&["名單"])], "{block}");
    }
}

#[test]
fn root_level_dash_items_are_named_keys() {
    let parsed = parse_state_fields("- 天气：小雪\n- 心情：警觉\n- 没有冒号的条目\n");
    assert_eq!(
        parsed,
        ParsedFields {
            fields: vec![field(&["天气"], "小雪"), field(&["心情"], "警觉")],
            ..Default::default()
        }
    );
}

#[test]
fn empty_lists_and_skipped_items() {
    let parsed =
        parse_state_fields("在场:\n  名單: []\n  空項:\n    - \"\"\n    - {}\n    - 林远\n");
    assert_eq!(parsed.fields, vec![field(&["在场", "空項", "0"], "林远")]);
    assert_eq!(
        parsed.lists,
        vec![p(&["在场", "名單"]), p(&["在场", "空項"])]
    );
}

#[test]
fn contiguous_numeric_keys_are_a_list_but_years_are_not() {
    let parsed = parse_state_fields("名單:\n  0: 林远\n  1: 贺铮\n年度:\n  2025: 平\n  2026: 丰\n");
    assert_eq!(parsed.lists, vec![p(&["名單"])]);
    assert_eq!(parsed.maps, vec![p(&["年度"])]);
}

#[test]
fn mixed_container_flattens_to_named_keys() {
    let parsed = parse_state_fields("驿站:\n  粮草: 400\n  - 马匹: 37\n  - 散句\n");
    assert_eq!(
        parsed.fields,
        vec![
            field(&["驿站", "粮草"], "400"),
            field(&["驿站", "马匹"], "37")
        ]
    );
    assert!(parsed.lists.is_empty());
}

#[test]
fn blocks_without_lists_match_the_old_parser() {
    for block in [
        "World:\n  Time: 清晨\n  Inventory: {}\n",
        "A:\n    B: 1\n  C: 2\n# 自訂：有效\n沒有冒號\n",
        "- time: 清晨\nplace:   \n",
        "体力:60\n好感:20\n",
    ] {
        let old: Vec<(Vec<String>, String)> = crate::transport::parse_indented_fields(block)
            .into_iter()
            .filter_map(|(path, value)| Some((path, value?)))
            .collect();
        assert_eq!(parse_state_fields(block).fields, old, "{block}");
    }
}

#[test]
fn fence_with_root_dash_time_still_normalises() {
    let block = extract_state_block("旁白\n```state\n- time: 清晨\n- 在场人物：林远\n```");
    assert_eq!(
        block.fields,
        vec![field(&["time"], "清晨"), field(&["present"], "林远")]
    );
}

#[test]
fn present_scalar_list_joins_into_the_table_field() {
    let block = extract_state_block("<Status_block>在场人物:\n  - 林远\n  - 贺铮\n</Status_block>");
    assert_eq!(block.fields, vec![field(&["present"], "林远、贺铮")]);
    assert!(block.lists.is_empty());
}

#[test]
fn later_block_replaces_the_same_list_including_nested_marks() {
    let three = "<Status_block>人物列表:\n  - 名字: A\n    装备:\n      - 剑\n  - 名字: B\n    装备: []\n  - 名字: C\n</Status_block>";
    let StateBlock { fields, lists, .. } = extract_state_block(&format!(
        "{three}\n```state\n人物列表:\n  - 名字: D\n    身份: 新人\n```"
    ));
    assert_eq!(
        fields,
        vec![
            field(&["人物列表", "0", "名字"], "D"),
            field(&["人物列表", "0", "身份"], "新人")
        ]
    );
    assert_eq!(lists, vec![p(&["人物列表"])]);

    let StateBlock { fields, lists, .. } =
        extract_state_block(&format!("{three}\n```state\n人物列表: []\n```"));
    assert!(fields.is_empty());
    assert_eq!(lists, vec![p(&["人物列表"])]);
}

/// 前塊報成清單（兩個 hp）、後塊改報成具名形（一個 hp）：前塊在這支底下的欄位與清單標記作廢
#[test]
fn later_block_reporting_the_list_as_a_map_wins() {
    let block = extract_state_block(
        "<Status_block>L:\n  - hp: 1\n  - hp: 2\n</Status_block>\n```state\nL:\n  - hp: 3\n```",
    );
    assert_eq!(block.fields, vec![field(&["L", "hp"], "3")]);
    assert!(block.lists.is_empty());
    assert_eq!(block.maps, vec![p(&["L"])]);
}

#[test]
fn empty_present_list_clears_the_table_field() {
    let block = extract_state_block("<Status_block>在场人物: []\n</Status_block>");
    assert_eq!(block.fields, vec![field(&["present"], "")]);
    assert!(block.lists.is_empty());
}

/// 清單項的區塊純量先判：`>` 不被當行首記號剝掉，續行有冒號或條列也不變成標題項
#[test]
fn block_scalar_items_win_over_titles() {
    let parsed = parse_state_fields(
        "L:\n  - |\n    first\n    second: content\n  - >\n    一行\n    两行\n  - |-\n    他说：走吧\n    - 条列\n",
    );
    assert_eq!(
        parsed.fields,
        vec![
            field(&["L", "0"], "first\nsecond: content"),
            field(&["L", "1"], "一行\n两行"),
            field(&["L", "2"], "他说：走吧\n- 条列"),
        ]
    );
    assert_eq!(parsed.lists, vec![p(&["L"])]);
}

/// 鍵值 `|`／`>` 底下沒有續行：當沒寫值，不產生欄位也不記容器
#[test]
fn block_scalar_key_without_lines_writes_nothing() {
    let parsed = parse_state_fields("状态:\n  描述: |\n  地点: 前院\n");
    assert_eq!(parsed.fields, vec![field(&["状态", "地点"], "前院")]);
    assert_eq!(parsed.maps, vec![p(&["状态"])]);
}

/// 不同外殼的區塊依原文位置合併：原文後面的那塊勝出（不論外殼種類收集順序）
#[test]
fn blocks_merge_in_source_order_across_shells() {
    let details_last = extract_state_block(
        "旁白\n```state\nL:\n  - a\n```\n中段\n<details><summary>状态</summary>\nL: []\n</details>",
    );
    assert!(details_last.fields.is_empty());
    assert_eq!(details_last.lists, vec![p(&["L"])]);

    let fence_last = extract_state_block(
        "旁白\n<details><summary>状态</summary>\nL: []\n</details>\n中段\n```state\nL:\n  - a\n```",
    );
    assert_eq!(fence_last.fields, vec![field(&["L", "0"], "a")]);
    assert_eq!(fence_last.lists, vec![p(&["L"])]);
}

/// 同一外殼 A→B→A（A＝一項、B＝空清單）：原文最後一塊 A 勝出（內容相同也照實際位置排）
#[test]
fn identical_blocks_keep_their_real_positions() {
    let a = "<Status_block>L:\n  - a\n</Status_block>";
    let b = "<Status_block>L: []\n</Status_block>";
    let block = extract_state_block(&format!("一{a}\n二{b}\n三{a}"));
    assert_eq!(block.fields, vec![field(&["L", "0"], "a")]);

    // 不同外殼也一樣：圍欄 A、details B、狀態標籤 A
    let block = extract_state_block(
        "一\n```state\nL:\n  - a\n```\n二<details><summary>状态</summary>\nL: []\n</details>\n三<Status_block>L:\n  - a\n</Status_block>",
    );
    assert_eq!(block.fields, vec![field(&["L", "0"], "a")]);
}

/// 後一塊的內容恰好出現在前面另一塊裡：位置照擷取處，不被內容搜尋拉到前面
#[test]
fn block_contained_in_an_earlier_block_still_wins() {
    let block = extract_state_block(
        "<Status_block>备注: 下面这段\nL: []\nL2: x\n</Status_block>\n旁白\n```state\nL: []\n```\n<details><summary>状态</summary>\nL:\n  - b\n</details>\n尾\n```state\nL: []\n```",
    );
    assert!(block.fields.iter().all(|(path, _)| path[0] != "L"));
    assert!(block.lists.contains(&p(&["L"])));
}
