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

#[test]
fn parse_expand_interface_valid_json_yields_state_fields() {
    let raw = "## STATE\n```json\n{\"World\": {\"Time\": \"清晨\"}}\n```\n";
    let outcome = parse_expand(EntryKind::Interface, "7", raw);
    let interface = outcome.interface.unwrap();
    assert_eq!(interface.source_uids, vec!["7"]);
    assert_eq!(
        interface.state_fields["World"]["Time"].as_str(),
        Some("清晨")
    );
    assert_eq!(outcome.raw, raw);
}

#[test]
fn parse_expand_interface_broken_json_falls_back_to_none_and_raw() {
    let raw = "## STATE\n```json\n{ this is not valid json\n```\n";
    let outcome = parse_expand(EntryKind::Interface, "7", raw);
    assert!(outcome.interface.is_none());
    assert_eq!(outcome.raw, raw);
}

#[test]
fn parse_expand_interface_shell_kind_extracts_html_shell() {
    let raw = "## STATE\n```json\n{\"World\": {\"Time\": \"清晨\"}}\n```\n\n\
               ## SHELL\n```html\n<!DOCTYPE html><html><body>{{World.Time}}</body></html>\n```\n";
    let outcome = parse_expand(EntryKind::InterfaceShell, "7", raw);
    let interface = outcome.interface.unwrap();
    assert_eq!(
        interface.shell.as_deref(),
        Some("<!DOCTYPE html><html><body>{{World.Time}}</body></html>")
    );
}

#[test]
fn parse_expand_interface_without_shell_marker_yields_none() {
    let raw = "## STATE\n```json\n{\"World\": {\"Time\": \"清晨\"}}\n```\n";
    let outcome = parse_expand(EntryKind::Interface, "7", raw);
    let interface = outcome.interface.unwrap();
    assert!(interface.shell.is_none());
}

#[test]
fn parse_expand_interface_empty_shell_fence_yields_none() {
    let raw =
        "## STATE\n```json\n{\"World\": {\"Time\": \"清晨\"}}\n```\n\n## SHELL\n```html\n```\n";
    let outcome = parse_expand(EntryKind::InterfaceShell, "7", raw);
    let interface = outcome.interface.unwrap();
    assert!(interface.shell.is_none());
}

#[test]
fn parse_expand_interface_truncated_shell_keeps_partial_content_without_panic() {
    let raw = "## STATE\n```json\n{\"World\": {\"Time\": \"清晨\"}}\n```\n\n\
               ## SHELL\n```html\n<!DOCTYPE html><html><body>{{World.Time}} 寫到一半突然斷";
    let outcome = parse_expand(EntryKind::InterfaceShell, "7", raw);
    let interface = outcome.interface.unwrap();
    assert!(interface.shell.unwrap().contains("寫到一半突然斷"));
}

#[test]
fn parse_expand_interface_shell_strips_language_tag_from_fence() {
    let raw = "## STATE\n```json\n{\"World\": {\"Time\": \"清晨\"}}\n```\n\n\
               ## SHELL\n```xml\n<UI>{{World.Time}}</UI>\n```\n";
    let outcome = parse_expand(EntryKind::InterfaceShell, "7", raw);
    assert_eq!(
        outcome.interface.unwrap().shell.as_deref(),
        Some("<UI>{{World.Time}}</UI>")
    );
}

#[test]
fn parse_expand_interface_shell_extracts_card_rules_and_guide() {
    let raw = "## STATE\n```json\n{\"World\": {\"Time\": \"清晨\"}}\n```\n\n\
               ## SHELL\n```xml\n<UI>{{World.Time}}</UI>\n```\n\n\
               ## RULES\n```json\n{\"World.Time\": {\"kind\": \"text\", \"update\": \"replace\", \"inject\": \"turn\"}}\n```\n\n\
               ## GUIDE\n每回合都要重報 World.Time。\n";
    let interface = parse_expand(EntryKind::InterfaceShell, "7", raw)
        .interface
        .unwrap();
    assert_eq!(
        interface.rules.get("World.Time").map(|rule| rule.kind),
        Some(data::FieldKind::Text)
    );
    assert_eq!(interface.guide, "每回合都要重報 World.Time。");

    let without = parse_expand(
        EntryKind::InterfaceShell,
        "7",
        "## STATE\n```json\n{\"World\": {\"Time\": \"清晨\"}}\n```\n",
    )
    .interface
    .unwrap();
    assert!(without.rules.is_empty());
    assert!(without.guide.is_empty());
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
