use super::*;
use crate::data::TranscriptKind;

fn event(marker: Option<EventMarker>, text: &str) -> TranscriptEvent {
    TranscriptEvent {
        ts: String::new(),
        speaker_id: String::new(),
        speaker_name: "GM".to_owned(),
        kind: TranscriptKind::System,
        text: text.to_owned(),
        raw: None,
        state: None,
        truncated: false,
        gm_only: false,
        marker,
        opening: false,
    }
}

/// 代碼 JSON 形狀是持久契約：逐字鎖住，並能讀回。
#[test]
fn marker_json_is_tagged_snake_case_and_round_trips() {
    let cases = [
        (EventMarker::SceneSummary, r#"{"type":"scene_summary"}"#),
        (
            EventMarker::CardArrival {
                name: "X".to_owned(),
            },
            r#"{"type":"card_arrival","name":"X"}"#,
        ),
        (
            EventMarker::PersonArrival {
                title: "X".to_owned(),
            },
            r#"{"type":"person_arrival","title":"X"}"#,
        ),
        (
            EventMarker::CardPrivate {
                name: "X".to_owned(),
            },
            r#"{"type":"card_private","name":"X"}"#,
        ),
        (EventMarker::StateUpdate, r#"{"type":"state_update"}"#),
        (
            EventMarker::GmCall {
                name: String::new(),
            },
            r#"{"type":"gm_call","name":""}"#,
        ),
    ];
    for (marker, json) in cases {
        assert_eq!(serde_json::to_string(&marker).unwrap(), json);
        assert_eq!(serde_json::from_str::<EventMarker>(json).unwrap(), marker);
    }
}

/// 沒有 marker 的事件不寫這欄、照讀；帶欄位的未知 type 讀成 Unknown；已知 type 缺必要欄位是壞行。
#[test]
fn missing_unknown_and_malformed_markers() {
    let plain = event(None, "本文");
    let json = serde_json::to_string(&plain).unwrap();
    assert!(!json.contains("marker"));
    assert_eq!(
        serde_json::from_str::<TranscriptEvent>(&json).unwrap(),
        plain
    );

    let unknown = json.replace(
        r#""gm_only":false"#,
        r#""gm_only":false,"marker":{"type":"future_thing","who":"X","n":3}"#,
    );
    let read: TranscriptEvent = serde_json::from_str(&unknown).unwrap();
    assert_eq!(read.marker, Some(EventMarker::Unknown));
    assert_eq!(event_full_text(&read, "zh-TW"), "本文");
    assert!(appeared_card_names(std::slice::from_ref(&read)).is_empty());

    let malformed = json.replace(
        r#""gm_only":false"#,
        r#""gm_only":false,"marker":{"type":"card_arrival"}"#,
    );
    assert!(serde_json::from_str::<TranscriptEvent>(&malformed).is_err());
}

/// 繁中全文與改代碼前的寫入格式逐字相同；公開設定空白不多生段標與換行。
#[test]
fn full_text_restores_section_labels_and_line_breaks() {
    let name = || "狐狸".to_owned();
    let cases = [
        (
            event(
                Some(EventMarker::CardArrival { name: name() }),
                "尾巴很大。",
            ),
            "（角色回歸）〈狐狸〉\n公開設定：\n尾巴很大。",
        ),
        (
            event(Some(EventMarker::CardArrival { name: name() }), ""),
            "（角色回歸）〈狐狸〉",
        ),
        (
            event(
                Some(EventMarker::CardPrivate { name: name() }),
                "其實是妖狐。",
            ),
            "（角色私設）〈狐狸〉\n私有設定：\n其實是妖狐。",
        ),
        (
            event(
                Some(EventMarker::PersonArrival {
                    title: "密探".to_owned(),
                }),
                "全文",
            ),
            "（人物登場）〈密探〉\n全文",
        ),
        (
            event(Some(EventMarker::SceneSummary), "摘要"),
            "【前情提要】\n摘要",
        ),
        (
            event(Some(EventMarker::StateUpdate), "hp：3\nmp：1"),
            "狀態更新\nhp：3\nmp：1",
        ),
        (
            event(Some(EventMarker::GmCall { name: name() }), ""),
            "GM 請「狐狸」發言",
        ),
        (
            event(
                Some(EventMarker::GmCall {
                    name: String::new(),
                }),
                "",
            ),
            "GM 請「玩家」發言",
        ),
        (event(None, "原文"), "原文"),
    ];
    for (event, expected) in cases {
        assert_eq!(event_full_text(&event, "zh-TW"), expected);
    }
}

/// 十語系標頭：各語系都有自己的字，fr 冒號前與 « » 內側用 U+00A0；未知語系退英文、zh* 退繁中。
#[test]
fn headings_cover_ten_languages() {
    let arrival = EventMarker::CardArrival {
        name: "X".to_owned(),
    };
    let langs = [
        "zh-TW", "zh-CN", "en", "ja", "ko", "es", "pt-BR", "de", "fr", "ru",
    ];
    let headings: BTreeSet<String> = langs
        .iter()
        .map(|lang| marker_heading(&arrival, lang).unwrap())
        .collect();
    assert_eq!(headings.len(), langs.len());
    assert_eq!(
        marker_heading(&arrival, "fr").unwrap(),
        "(Retour d’un personnage) «\u{a0}X\u{a0}»"
    );
    assert_eq!(
        marker_heading(&EventMarker::SceneSummary, "fr").unwrap(),
        "Précédemment\u{a0}:"
    );
    assert_eq!(
        marker_heading(&arrival, "it"),
        marker_heading(&arrival, "en")
    );
    assert_eq!(
        marker_heading(&arrival, "zh-HK"),
        marker_heading(&arrival, "zh-TW")
    );
    assert_eq!(marker_heading(&EventMarker::Unknown, "en"), None);
    for lang in ["zh-TW", "zh-CN", "zh-HK"] {
        assert_eq!(prompt_lang(lang), "zh-TW");
    }
    for lang in ["en", "ja", "ko", "es", "pt-BR", "de", "fr", "ru", "it", ""] {
        assert_eq!(prompt_lang(lang), "en");
    }
}

/// 共用字典十語系齊全、鍵一致、佔位符正確（前端讀同一份 JSON）。
#[test]
fn shared_dictionary_is_complete() {
    let dictionary = dictionary();
    let langs = [
        "zh-TW", "zh-CN", "en", "ja", "ko", "es", "pt-BR", "de", "fr", "ru",
    ];
    assert_eq!(dictionary.len(), langs.len());
    let keys: BTreeSet<&String> = dictionary["zh-TW"].keys().collect();
    assert_eq!(keys.len(), 8);
    for lang in langs {
        let entries = &dictionary[lang];
        assert_eq!(entries.keys().collect::<BTreeSet<_>>(), keys, "{lang}");
        for (key, text) in entries {
            assert!(!text.trim().is_empty(), "{lang}.{key}");
            let placeholder = match key.as_str() {
                "card_arrival" | "card_private" | "gm_call" => Some("{name}"),
                "person_arrival" => Some("{title}"),
                _ => None,
            };
            match placeholder {
                Some(placeholder) => {
                    assert_eq!(text.matches(placeholder).count(), 1, "{lang}.{key}")
                }
                None => assert!(!text.contains('{'), "{lang}.{key}"),
            }
        }
    }
}
