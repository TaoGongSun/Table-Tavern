use super::*;
use crate::data::{self, FieldKind, TriggerMode};

#[test]
fn parse_person_expand_full_output_yields_one_character_with_all_source_uids() {
    let raw = "## EMOJI\n🗡️\n## PUBLIC\n公開的亞瑟。\n## PRIVATE\n私密的亞瑟。\n";
    let outcome = parse_person_expand(raw, "亞瑟", &["101".to_owned(), "102".to_owned()], true);
    let character = outcome.character.unwrap();
    assert_eq!(character.name, "亞瑟");
    assert_eq!(character.emoji, "🗡️");
    assert_eq!(character.public_md, "公開的亞瑟。");
    assert_eq!(character.private_md, "私密的亞瑟。");
    assert_eq!(character.source_uids, vec!["101", "102"]);
    assert!(character.suspected_player);
    assert_eq!(character.solo_entry_md, "公開的亞瑟。\n\n私密的亞瑟。");
    assert_eq!(outcome.raw, raw);
}

#[test]
fn parse_person_expand_not_suspected_player_stays_false() {
    let raw = "## EMOJI\n🍺\n## PUBLIC\n公開設定。\n## PRIVATE\n";
    let outcome = parse_person_expand(raw, "酒館老闆", &["55".to_owned()], false);
    assert!(!outcome.character.unwrap().suspected_player);
}

#[test]
fn parse_person_expand_truncated_mid_stream_keeps_partial_content_without_panic() {
    let raw = "## EMOJI\n🛡️\n## PUBLIC\n公開的莫斯。\n## PRIVATE\n私密的莫斯，寫到一半突然斷";
    let outcome = parse_person_expand(raw, "莫斯", &["7".to_owned()], false);
    let character = outcome.character.unwrap();
    assert_eq!(character.public_md, "公開的莫斯。");
    assert_eq!(character.private_md, "私密的莫斯，寫到一半突然斷");
}

#[test]
fn parse_person_expand_without_any_marker_falls_back_to_none_and_raw() {
    let raw = "抱歉，我沒辦法處理這個請求。";
    let outcome = parse_person_expand(raw, "亞瑟", &["1".to_owned()], false);
    assert!(outcome.character.is_none());
    assert_eq!(outcome.raw, raw);
}

const FULL_STATE: &str = "## STATE\n```json\n{\"World\": {\"Time\": \"清晨\"}}\n```\n\n";
const FULL_SHELL: &str =
    "## SHELL\n```xml\n<UI><Time>{{World.Time}}</Time>{{本回合.正文}}</UI>\n```\n\n";
const FULL_RULES: &str = "## RULES\n```json\n{\"World.Time\": {\"kind\": \"text\", \"update\": \"replace\", \"inject\": \"turn\"}}\n```\n\n";
const FULL_GUIDE: &str = "## GUIDE\n每回合都要報 World.Time。\n";

fn full_raw() -> String {
    format!("{FULL_STATE}{FULL_SHELL}{FULL_RULES}{FULL_GUIDE}")
}

#[test]
fn parse_expand_interface_complete_output_yields_all_four_parts() {
    for kind in [EntryKind::InterfaceShell, EntryKind::InterfaceStatusbar] {
        let raw = full_raw();
        let outcome = parse_expand(kind, "7", &raw);
        let interface = outcome.interface.unwrap();
        assert_eq!(interface.source_uids, vec!["7"]);
        assert_eq!(
            interface.state_fields["World"]["Time"].as_str(),
            Some("清晨")
        );
        // 圍欄的語言標記一起剝掉，不會寫進骨架第一行
        assert_eq!(
            interface.shell.as_deref(),
            Some("<UI><Time>{{World.Time}}</Time>{{本回合.正文}}</UI>")
        );
        assert_eq!(
            interface.rules.get("World.Time").map(|rule| rule.kind),
            Some(data::FieldKind::Text)
        );
        assert_eq!(interface.guide, "每回合都要報 World.Time。");
        assert_eq!(outcome.raw, raw);
    }
}

#[test]
fn parse_expand_interface_incomplete_output_fails_instead_of_state_only() {
    let two_rules = "## RULES\n```json\n{\"World.Time\": {\"kind\": \"text\", \"update\": \"replace\", \"inject\": \"turn\"}, \"World\": {\"kind\": \"text\", \"update\": \"replace\", \"inject\": \"turn\"}}\n```\n\n";
    let cases = [
        (
            "STATE 壞掉",
            format!("## STATE\n```json\n{{ 壞掉\n```\n\n{FULL_SHELL}{FULL_RULES}{FULL_GUIDE}"),
        ),
        (
            "STATE 不是物件",
            format!("## STATE\n```json\n[1]\n```\n\n{FULL_SHELL}{FULL_RULES}{FULL_GUIDE}"),
        ),
        ("SHELL 缺席", format!("{FULL_STATE}{FULL_RULES}{FULL_GUIDE}")),
        (
            "SHELL 空圍欄",
            format!("{FULL_STATE}## SHELL\n```xml\n```\n\n{FULL_RULES}{FULL_GUIDE}"),
        ),
        ("RULES 缺席", format!("{FULL_STATE}{FULL_SHELL}{FULL_GUIDE}")),
        (
            "RULES 空白",
            format!("{FULL_STATE}{FULL_SHELL}## RULES\n\n{FULL_GUIDE}"),
        ),
        (
            "RULES 壞 JSON",
            format!("{FULL_STATE}{FULL_SHELL}## RULES\n```json\n{{ 壞\n```\n\n{FULL_GUIDE}"),
        ),
        ("GUIDE 缺席", format!("{FULL_STATE}{FULL_SHELL}{FULL_RULES}")),
        (
            "GUIDE 空白",
            format!("{FULL_STATE}{FULL_SHELL}{FULL_RULES}## GUIDE\n\n"),
        ),
        (
            "佔位符不在 STATE",
            format!(
                "{FULL_STATE}## SHELL\n```xml\n<UI>{{{{World.Place}}}}</UI>\n```\n\n{FULL_RULES}{FULL_GUIDE}"
            ),
        ),
        (
            "佔位符落在分支",
            format!(
                "{FULL_STATE}## SHELL\n```xml\n<UI>{{{{World}}}}</UI>\n```\n\n{two_rules}{FULL_GUIDE}"
            ),
        ),
        (
            "佔位符沒有規則",
            format!(
                "## STATE\n```json\n{{\"World\": {{\"Time\": \"清晨\", \"Place\": \"港口\"}}}}\n```\n\n## SHELL\n```xml\n<UI>{{{{World.Time}}}}{{{{World.Place}}}}</UI>\n```\n\n{FULL_RULES}{FULL_GUIDE}"
            ),
        ),
        (
            "截斷在骨架中間",
            format!(
                "{FULL_STATE}## SHELL\n```xml\n<UI><Time>{{{{World.Time}}}}</Time> 寫到一半突然斷"
            ),
        ),
    ];
    for (label, raw) in cases {
        for kind in [EntryKind::InterfaceShell, EntryKind::InterfaceStatusbar] {
            let outcome = parse_expand(kind, "7", &raw);
            assert!(outcome.interface.is_none(), "{label} 應該失敗");
            assert_eq!(outcome.raw, raw);
        }
    }
}

#[test]
fn placeholder_contract_separates_body_macros_and_paths() {
    use super::result_parse::{classify_placeholder, PlaceholderKind};
    let state = serde_json::json!({ "Time": "黃昏", "World": { "Time": "清晨" } });
    assert_eq!(
        classify_placeholder("本回合.正文", &state),
        PlaceholderKind::Body
    );
    for macro_token in [
        "user",
        "Char",
        "lastMessageId",
        "time",
        "random::a::b",
        "roll:1d6",
        "getvar::hp",
    ] {
        assert_eq!(
            classify_placeholder(macro_token, &state),
            PlaceholderKind::Macro,
            "{macro_token}"
        );
    }
    // 狀態葉子優先於同名巨集；未知的冒號寫法照樣是路徑
    for path in ["Time", "World.Time", "World:Missing", "状态栏.地点"] {
        assert_eq!(
            classify_placeholder(path, &state),
            PlaceholderKind::Path,
            "{path}"
        );
    }
}

#[test]
fn st_macro_list_is_shared_with_frontend() {
    // 前後端讀同一份 src/shared/contracts/st-macros.json：這裡驗證它讀得到、內容合理
    use super::result_parse::is_st_macro;
    let raw: serde_json::Value =
        serde_json::from_str(include_str!("../../../src/shared/contracts/st-macros.json")).unwrap();
    let names = raw["names"].as_array().unwrap();
    assert!(names.iter().any(|name| name == "user"));
    for name in names {
        assert!(is_st_macro(name.as_str().unwrap()));
    }
    for name in raw["argument_names"].as_array().unwrap() {
        assert!(is_st_macro(&format!("{}::x", name.as_str().unwrap())));
    }
    assert!(!is_st_macro("World:Missing"));
}

#[test]
fn parse_expand_accepts_card_macros_but_still_rejects_unknown_paths() {
    let with_macros = format!(
        "{FULL_STATE}## SHELL\n```xml\n<UI>{{{{user}}}} 對 {{{{char}}}}：{{{{World.Time}}}} {{{{random::甲::乙}}}}{{{{本回合.正文}}}}</UI>\n```\n\n{FULL_RULES}{FULL_GUIDE}"
    );
    let interface = parse_expand(EntryKind::InterfaceStatusbar, "7", &with_macros)
        .interface
        .unwrap();
    assert!(interface.shell.unwrap().contains("{{user}}"));
    // 不是巨集、也不在 STATE 的未知路徑照樣拒絕：不能把所有未知路徑當巨集放過
    let unknown = format!(
        "{FULL_STATE}## SHELL\n```xml\n<UI>{{{{user}}}} {{{{World.Weather}}}}</UI>\n```\n\n{FULL_RULES}{FULL_GUIDE}"
    );
    assert!(parse_expand(EntryKind::InterfaceStatusbar, "7", &unknown)
        .interface
        .is_none());
    // 未知冒號寫法不能免驗
    let unknown_colon = format!(
        "{FULL_STATE}## SHELL\n```xml\n<UI>{{{{World:Missing}}}} {{{{World.Time}}}}</UI>\n```\n\n{FULL_RULES}{FULL_GUIDE}"
    );
    assert!(
        parse_expand(EntryKind::InterfaceStatusbar, "7", &unknown_colon)
            .interface
            .is_none()
    );
    // 狀態葉子與巨集同名（Time）時是狀態引用，要有規則
    let leaf_named_like_macro = "## STATE\n```json\n{\"Time\": \"黃昏\"}\n```\n\n## SHELL\n```xml\n<UI>{{Time}}</UI>\n```\n\n## RULES\n```json\n{\"Time\": {\"kind\": \"text\", \"update\": \"replace\", \"inject\": \"turn\"}}\n```\n\n## GUIDE\n每回合報 Time。\n";
    assert!(
        parse_expand(EntryKind::InterfaceStatusbar, "7", leaf_named_like_macro)
            .interface
            .is_some()
    );
    let leaf_without_rule =
        leaf_named_like_macro.replace("\"Time\": {\"kind\"", "\"Other\": {\"kind\"");
    assert!(
        parse_expand(EntryKind::InterfaceStatusbar, "7", &leaf_without_rule)
            .interface
            .is_none()
    );
}

#[test]
fn parse_expand_statusbar_yaml_skeleton_keeps_fixed_text() {
    let raw = "## STATE\n```json\n{\"状态栏\": {\"地点\": \"北境驿站\", \"粮草\": \"320\"}}\n```\n\n\
               ## SHELL\n```xml\n<Status_block>\n状态栏:\n  地点: \"📍 {{状态栏.地点}}\"\n  粮草: \"🌾 {{状态栏.粮草}}\"\n  地图: \"固定矩陣不挖\"\n</Status_block>\n```\n\n\
               ## RULES\n```json\n{\"状态栏.地点\": {\"kind\": \"text\", \"update\": \"replace\", \"inject\": \"turn\"}, \"状态栏.粮草\": {\"kind\": \"number\", \"update\": \"delta\", \"inject\": \"turn\"}}\n```\n\n\
               ## GUIDE\n地点每回合必報；粮草變動才報。\n";
    let interface = parse_expand(EntryKind::InterfaceStatusbar, "9", raw)
        .interface
        .unwrap();
    let shell = interface.shell.unwrap();
    assert!(shell.starts_with("<Status_block>\n状态栏:\n  地点: \"📍 {{状态栏.地点}}\""));
    assert!(shell.contains("地图: \"固定矩陣不挖\""));
}

#[test]
fn parse_absorb_full_output_yields_rules_and_triggers() {
    let raw = "## RULES\n```json\n\
               { \"淪陷天數\": { \"kind\": \"counter\", \"update\": \"delta\", \"inject\": \"turn\", \"min\": 0.0 } }\n\
               ```\n\
               ## TRIGGERS\n```json\n\
               [ { \"id\": \"day7\", \"title\": \"第七天\", \"mode\": \"once\", \"flag\": \"旗標.第七天\",\n\
                   \"cases\": [ { \"when\": [], \"text\": \"引用 {{span:9#s3}}\" } ] } ]\n\
               ```\n";
    let outcome = parse_absorb(raw);
    assert_eq!(
        outcome.rules.get("淪陷天數").unwrap().kind,
        FieldKind::Counter
    );
    assert_eq!(outcome.triggers.len(), 1);
    assert_eq!(outcome.triggers[0].mode, TriggerMode::Once);
    assert_eq!(outcome.triggers[0].cases[0].text, "引用 {{span:9#s3}}");
    assert_eq!(outcome.raw, raw);
}

#[test]
fn parse_absorb_broken_json_falls_back_to_empty_sets() {
    let raw = "## RULES\n```json\n{ broken\n```\n## TRIGGERS\n```json\n[ also broken\n```\n";
    let outcome = parse_absorb(raw);
    assert!(outcome.rules.is_empty());
    assert!(outcome.triggers.is_empty());
    assert_eq!(outcome.raw, raw);
}

#[test]
fn parse_absorb_empty_output_yields_empty_sets_not_failure() {
    let outcome = parse_absorb("抱歉，這條我抽不出規則。");
    assert!(outcome.rules.is_empty());
    assert!(outcome.triggers.is_empty());
}

#[test]
fn parse_group_setting_yields_content_only() {
    let raw = "## CONTENT\n格式與行為併成一條。\n";
    let outcome = parse_group(
        raw,
        "格式與行為",
        GroupKind::Setting,
        &["16".to_owned(), "18".to_owned()],
    );
    let entry = outcome.entry.unwrap();
    assert_eq!(entry.kind, "setting");
    assert_eq!(entry.content, "格式與行為併成一條。");
    assert_eq!(entry.source_uids, vec!["16", "18"]);
    assert!(entry.rules.is_empty() && entry.triggers.is_empty());
    assert!(entry.meta.is_none());
}

#[test]
fn parse_group_mechanism_yields_content_rules_and_triggers() {
    let raw = "## CONTENT\n合併後的機制說明。\n\
               ## RULES\n```json\n\
               { \"好感度\": { \"kind\": \"number\", \"update\": \"delta\", \"inject\": \"turn\" } }\n\
               ```\n\
               ## TRIGGERS\n```json\n[]\n```\n";
    let outcome = parse_group(raw, "好感度機制", GroupKind::Mechanism, &["16".to_owned()]);
    let entry = outcome.entry.unwrap();
    assert_eq!(entry.kind, "mechanism");
    assert!(entry.content.contains("合併後的機制說明"));
    assert_eq!(entry.rules.get("好感度").unwrap().kind, FieldKind::Number);
}

#[test]
fn parse_group_without_content_falls_back_to_none_and_raw() {
    let raw = "抱歉，拆不出來。";
    let outcome = parse_group(raw, "格式與行為", GroupKind::Setting, &["16".to_owned()]);
    assert!(outcome.entry.is_none());
    assert_eq!(outcome.raw, raw);
}

#[test]
fn expand_span_placeholders_replaces_valid_and_keeps_invalid() {
    let lookup = |span_ref: &str| -> Option<String> {
        match span_ref {
            "9#s3" => Some("  原文段落內容。  ".to_owned()),
            _ => None,
        }
    };
    let text = "命中時提到 {{span:9#s3}}，還有 {{span:99#s9}} 找不到。";
    let expanded = expand_span_placeholders(text, &lookup);
    assert_eq!(
        expanded,
        "命中時提到 原文段落內容。，還有 {{span:99#s9}} 找不到。"
    );
}

#[test]
fn expand_span_placeholders_handles_multiple_placeholders() {
    let lookup = |span_ref: &str| -> Option<String> {
        match span_ref {
            "1#s1" => Some("甲".to_owned()),
            "2#s2" => Some("乙".to_owned()),
            _ => None,
        }
    };
    let text = "{{span:1#s1}}與{{span:2#s2}}";
    assert_eq!(expand_span_placeholders(text, &lookup), "甲與乙");
}

#[test]
fn expand_span_placeholders_without_any_placeholder_returns_text_unchanged() {
    let lookup = |_: &str| -> Option<String> { None };
    let text = "沒有任何佔位符的純文字。";
    assert_eq!(expand_span_placeholders(text, &lookup), text);
}
