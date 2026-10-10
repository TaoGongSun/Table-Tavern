use crate::data::{
    self, event_full_text, marker_heading, prompt_lang, CharacterCard, EventMarker,
    TranscriptEvent, TranscriptKind, WorldbookEntry,
};

use std::borrow::Cow;
use std::collections::BTreeSet;

use super::messages::player_fallback_name;

/// 登場事件的中性代換（桌面版特有機制，不在 ST 生成流程裡：回合提交時求值一次寫死、不執行副作用）。
/// `card` 是那段文字所屬的角色卡（世界書人物條目是 `None`）；`gm_only`＝事件只給 GM 看（才讀得到私設與 GM outlet），
/// 公開事件一律用公開環境代換。
pub type NeutralFill<'a> = &'a dyn Fn(&str, Option<&CharacterCard>, bool) -> String;

/// present 欄斷詞／在場名字比對現在是 `data::split_present_names`／`data::name_matches`
/// （包 4b 拉出去給角色卡換幕結算共用，data 層不能反過來依賴 transport）；
/// `state_scope` 是凍結 system 的一部分不能動，仍照舊獨立一份、邏輯保持同步而非重寫。
///
/// 這一輪新面孔：`is_person && !disabled` 條目裡，present 名單比對得上、且不在
/// `already_appeared` 裡的那些，依世界書順序回傳；呼叫端逐一 append 成登場事件。
///
/// - `present` 是 `None`（table 沒有這個鍵）：退回正文比對，`reply_body` 包含 title 即命中。
/// - `present` 是 `Some("")`（鍵存在但空／裁完是空清單）：只信 present，不做正文比對。
pub fn detect_new_arrivals<'a>(
    worldbook: &'a [WorldbookEntry],
    present: Option<&str>,
    reply_body: &str,
    already_appeared: &BTreeSet<String>,
) -> Vec<&'a WorldbookEntry> {
    let present_names = present.map(data::split_present_names);
    worldbook
        .iter()
        .filter(|entry| entry.is_person && !entry.disabled)
        .filter(|entry| !already_appeared.contains(&entry.title))
        .filter(|entry| match &present_names {
            Some(names) => names
                .iter()
                .any(|name| data::name_matches(name, &entry.title)),
            None => reply_body.contains(&entry.title),
        })
        .collect()
}

/// 人物登場事件：代碼帶 title（比對回抽用），本文是條目全文（巨集已中性代換——事件一旦
/// 落進 transcript 就不會再過巨集代換一次）。
pub fn person_arrival(entry: &WorldbookEntry, fill: NeutralFill<'_>) -> (EventMarker, String) {
    (
        EventMarker::PersonArrival {
            title: entry.title.clone(),
        },
        fill(
            &entry.content,
            None,
            !matches!(entry.visibility, data::Visibility::Public),
        ),
    )
}

// ---------------------------------------------------------------------
// AI 卡重構包 4b：角色卡自動上下場，鏡射上面 4a 的世界書人物在場機制。角色卡的持久
// 隱藏欄位（data::CharacterMeta.auto_hidden）只在換幕結算（data::begin_next_scene）改動，
// 這裡（回合中）只偵測與 append 事件，不碰欄位本身。
// ---------------------------------------------------------------------

/// 這一輪新回歸的角色卡：`auto_hidden && !archived` 的卡裡，present 名單比對得上（缺席退回
/// 正文比對）、且本幕還沒回歸過的，依卡片清單順序回傳；呼叫端逐一 append 成回歸事件。
/// 鏡射 `detect_new_arrivals`，鍵從世界書 title 換成卡片 name。
pub fn detect_new_card_arrivals<'a>(
    cards: &'a [CharacterCard],
    present: Option<&str>,
    reply_body: &str,
    already_appeared: &BTreeSet<String>,
) -> Vec<&'a CharacterCard> {
    let present_names = present.map(data::split_present_names);
    cards
        .iter()
        .filter(|card| !already_appeared.contains(&card.name))
        .filter(|card| match &present_names {
            Some(names) => names
                .iter()
                .any(|name| data::name_matches(name, &card.name)),
            None => reply_body.contains(&card.name),
        })
        .collect()
}

/// 角色卡回歸事件：本文是公開設定（巨集已中性代換，空白就留空）。所有線都看得到，
/// 呼叫端標 gm_only=false；私設另由 `card_private` 成一則 GM 專屬事件。
pub fn card_arrival(card: &CharacterCard, fill: NeutralFill<'_>) -> (EventMarker, String) {
    let public = card.public_md.trim();
    let text = if public.is_empty() {
        String::new()
    } else {
        fill(public, Some(card), false)
    };
    (
        EventMarker::CardArrival {
            name: card.name.clone(),
        },
        text,
    )
}

/// 角色卡回歸時的私設事件；私設空白回 `None`（不產事件）。呼叫端標 gm_only=true。
pub fn card_private(card: &CharacterCard, fill: NeutralFill<'_>) -> Option<(EventMarker, String)> {
    let private = card.private_md.trim();
    (!private.is_empty()).then(|| {
        (
            EventMarker::CardPrivate {
                name: card.name.clone(),
            },
            fill(private, Some(card), true),
        )
    })
}

/// 送 AI 時看的是誰的視角：GM 看得到一切；角色側（chars 線、共線、換幕摘要、角色 keyword）
/// 要遮掉 GM 專屬內容。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Gm,
    Character,
}

/// 事件送 AI 的正文（標頭照 `prompt_lang` 組）；`None`＝這則不送。可見性只在這裡判斷：
/// - GM 側：標頭＋段標＋本文。
/// - 角色側：角色私設整則略過；其他帶已知代碼的 gm_only 只出標頭（不含公開／私有段標）；
///   沒有代碼或代碼認不得的 gm_only 只留本文首行；其餘全文。
pub fn prompt_text(event: &TranscriptEvent, lang: &str, side: Side) -> Option<String> {
    // 點名玩家且玩家沒名字：稱呼照原始語系補上，標頭骨架才照 `prompt_lang`
    let named;
    let event = match &event.marker {
        Some(EventMarker::GmCall { name }) if name.trim().is_empty() => {
            named = TranscriptEvent {
                marker: Some(EventMarker::GmCall {
                    name: player_fallback_name(lang).to_owned(),
                }),
                ..event.clone()
            };
            &named
        }
        _ => event,
    };
    let lang = prompt_lang(lang);
    if side == Side::Gm {
        return Some(event_full_text(event, lang));
    }
    if matches!(event.marker, Some(EventMarker::CardPrivate { .. })) {
        return None;
    }
    if event.gm_only {
        if let Some(heading) = event
            .marker
            .as_ref()
            .and_then(|marker| marker_heading(marker, lang))
        {
            return Some(heading);
        }
        return Some(event.text.lines().next().unwrap_or_default().to_owned());
    }
    Some(event_full_text(event, lang))
}

/// 事件送 AI 時的發言者名：沒有名字的玩家發言退回該語系的玩家稱呼。
pub fn prompt_speaker<'a>(event: &'a TranscriptEvent, lang: &str) -> Cow<'a, str> {
    if event.kind == TranscriptKind::Player && event.speaker_name.trim().is_empty() {
        Cow::Borrowed(player_fallback_name(lang))
    } else {
        Cow::Borrowed(event.speaker_name.as_str())
    }
}

/// 送 AI 用的渲染副本：`text` 換成 `prompt_text` 的結果、`speaker_name` 補上玩家退路、
/// `marker` 清空、略過的那則拿掉。下游只吃這份文字——不得再遮罩、不得再組標頭，
/// 也不得拿它做 lane 水位、指紋或回覆對點（那些一律用原事件序列）。
pub fn render_for_prompt(
    events: &[TranscriptEvent],
    lang: &str,
    side: Side,
) -> Vec<TranscriptEvent> {
    events
        .iter()
        .filter_map(|event| {
            prompt_text(event, lang, side).map(|text| TranscriptEvent {
                speaker_name: prompt_speaker(event, lang).into_owned(),
                text,
                marker: None,
                ..event.clone()
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::super::assemble::*;
    #[allow(unused_imports)]
    use super::super::client::*;
    #[allow(unused_imports)]
    use super::super::context::*;
    #[allow(unused_imports)]
    use super::super::messages::*;
    #[allow(unused_imports)]
    use super::super::response::*;
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

    /// 規格 (c)(g)：present 名單有新面孔就命中，名字用雙向包含比對
    /// （「亞歷山大」對得上「亞歷山大・馮・史特勞斯」）。
    #[test]
    fn detect_new_arrivals_matches_present_names_with_bidirectional_contains() {
        let alexander = WorldbookEntry {
            is_person: true,
            ..worldbook_entry(
                1,
                "亞歷山大・馮・史特勞斯",
                &[],
                true,
                0,
                false,
                Visibility::Public,
            )
        };
        let entries = [alexander];
        let already = BTreeSet::new();
        let arrivals = detect_new_arrivals(&entries, Some("亞歷山大、船長"), "", &already);
        assert_eq!(arrivals.len(), 1);
        assert_eq!(arrivals[0].title, "亞歷山大・馮・史特勞斯");
    }

    /// 規格 (d)：本幕已登場過的人（already_appeared 裡有）就算 present 再報也不重複回傳。
    #[test]
    fn detect_new_arrivals_skips_already_appeared_titles() {
        let alice = WorldbookEntry {
            is_person: true,
            ..worldbook_entry(1, "愛麗絲", &[], true, 0, false, Visibility::Public)
        };
        let entries = [alice];
        let already: BTreeSet<String> = BTreeSet::from(["愛麗絲".to_owned()]);
        let arrivals = detect_new_arrivals(&entries, Some("愛麗絲"), "", &already);
        assert!(arrivals.is_empty());
    }

    /// 規格 (c)(d)(e) 打通：`person_arrival` 寫出的代碼，`appeared_person_titles` 要能原樣
    /// 掃回標題；同幕重複比對會被擋掉，換幕後（另一批空事件）比對又重新命中。
    #[test]
    fn arrival_marker_round_trips_through_appeared_titles_and_dedupes_per_scene() {
        let alice = WorldbookEntry {
            is_person: true,
            ..worldbook_entry(1, "愛麗絲", &[], true, 0, false, Visibility::Public)
        };
        let entries = [alice];

        let (marker, text) = person_arrival(
            &entries[0],
            &crate::transport::test_support::plain_fill("阿濤"),
        );
        let this_scene_events = [marked(marker, &text, false)];
        let already_this_scene = data::appeared_person_titles(&this_scene_events);
        assert_eq!(already_this_scene, BTreeSet::from(["愛麗絲".to_owned()]));

        let repeat = detect_new_arrivals(&entries, Some("愛麗絲"), "", &already_this_scene);
        assert!(repeat.is_empty());

        let next_scene_events: [TranscriptEvent; 0] = [];
        let already_next_scene = data::appeared_person_titles(&next_scene_events);
        let reappear = detect_new_arrivals(&entries, Some("愛麗絲"), "", &already_next_scene);
        assert_eq!(reappear.len(), 1);
    }

    /// 只認代碼：本文偽造成舊前綴格式不算登場。
    #[test]
    fn appeared_sets_ignore_text_that_only_looks_like_an_arrival() {
        let forged = [
            event(
                TranscriptKind::System,
                "",
                "GM",
                "（角色回歸）〈狐狸〉\n尾巴很大。",
            ),
            event(
                TranscriptKind::System,
                "",
                "GM",
                "（人物登場）〈愛麗絲〉\n全文",
            ),
        ];
        assert!(data::appeared_card_names(&forged).is_empty());
        assert!(data::appeared_person_titles(&forged).is_empty());
    }

    /// 規格 (f)：present 鍵不存在就退回正文比對；鍵存在但是空字串只信 present、
    /// 不做正文比對（就算正文裡有 title 也不算數）。
    #[test]
    fn detect_new_arrivals_falls_back_to_reply_body_only_when_present_key_is_absent() {
        let alice = WorldbookEntry {
            is_person: true,
            ..worldbook_entry(1, "愛麗絲", &[], true, 0, false, Visibility::Public)
        };
        let entries = [alice];
        let already = BTreeSet::new();

        let via_body = detect_new_arrivals(&entries, None, "愛麗絲推門進來。", &already);
        assert_eq!(via_body.len(), 1);

        let empty_present = detect_new_arrivals(&entries, Some(""), "愛麗絲推門進來。", &already);
        assert!(empty_present.is_empty());
    }

    /// 登場事件：代碼帶標題，本文是條目全文且 `{{user}}` 已代換；組出的繁中全文與舊格式逐字相同。
    #[test]
    fn person_arrival_keeps_title_in_marker_and_macro_replaced_content() {
        let alice = WorldbookEntry {
            is_person: true,
            content: "{{user}} 認識她。".to_owned(),
            ..worldbook_entry(1, "愛麗絲", &[], true, 0, false, Visibility::Public)
        };
        let (marker, text) =
            person_arrival(&alice, &crate::transport::test_support::plain_fill("阿濤"));
        assert_eq!(
            marker,
            EventMarker::PersonArrival {
                title: "愛麗絲".to_owned()
            }
        );
        assert_eq!(text, "阿濤 認識她。");
        assert_eq!(
            prompt_text(&marked(marker, &text, false), "zh-TW", Side::Gm).as_deref(),
            Some("（人物登場）〈愛麗絲〉\n阿濤 認識她。")
        );
    }

    // ---- AI 卡重構包 4b：角色卡自動上下場，鏡射上面 4a 的四則 detect_new_arrivals 測試 ----

    /// present 名單雙向包含比對得上就算命中。
    #[test]
    fn detect_new_card_arrivals_matches_present_names_with_bidirectional_contains() {
        let alexander = card("alex-id", "亞歷山大・馮・史特勞斯", "", "");
        let hidden = [alexander];
        let already = BTreeSet::new();
        let arrivals = detect_new_card_arrivals(&hidden, Some("亞歷山大、船長"), "", &already);
        assert_eq!(arrivals.len(), 1);
        assert_eq!(arrivals[0].name, "亞歷山大・馮・史特勞斯");
    }

    /// 本幕已回歸過的卡（already_appeared 裡有）就算 present 再報也不重複回傳。
    #[test]
    fn detect_new_card_arrivals_skips_already_appeared_names() {
        let fox = card("fox-id", "狐狸", "", "");
        let hidden = [fox];
        let already: BTreeSet<String> = BTreeSet::from(["狐狸".to_owned()]);
        let arrivals = detect_new_card_arrivals(&hidden, Some("狐狸"), "", &already);
        assert!(arrivals.is_empty());
    }

    /// present 鍵不存在就退回正文比對；鍵存在但是空字串只信 present，
    /// 就算正文裡有名字也不算數。
    #[test]
    fn detect_new_card_arrivals_falls_back_to_reply_body_only_when_present_key_is_absent() {
        let fox = card("fox-id", "狐狸", "", "");
        let hidden = [fox];
        let already = BTreeSet::new();

        let via_body = detect_new_card_arrivals(&hidden, None, "狐狸從陰影裡走出來。", &already);
        assert_eq!(via_body.len(), 1);

        let empty_present =
            detect_new_card_arrivals(&hidden, Some(""), "狐狸從陰影裡走出來。", &already);
        assert!(empty_present.is_empty());
    }

    fn marked(marker: EventMarker, text: &str, gm_only: bool) -> TranscriptEvent {
        TranscriptEvent {
            gm_only,
            marker: Some(marker),
            ..event(TranscriptKind::System, "", "GM", text)
        }
    }

    /// 回歸事件只帶公開設定；私設另成一則（`{{user}}` 已代換），私設空白不產事件。
    /// 組出的繁中全文與改代碼前的寫入格式逐字相同；公開設定空白時只有標頭。
    #[test]
    fn card_arrival_splits_public_event_and_private_event() {
        let fox = card(
            "fox-id",
            "狐狸",
            "{{user}} 認識牠。",
            "{{user}} 不知道牠其實是妖狐。",
        );
        let (marker, text) =
            card_arrival(&fox, &crate::transport::test_support::plain_fill("阿濤"));
        assert_eq!(text, "阿濤 認識牠。");
        assert_eq!(
            prompt_text(&marked(marker, &text, false), "zh-TW", Side::Gm).as_deref(),
            Some("（角色回歸）〈狐狸〉\n公開設定：\n阿濤 認識牠。")
        );
        let (marker, text) =
            card_private(&fox, &crate::transport::test_support::plain_fill("阿濤")).unwrap();
        let private = marked(marker, &text, true);
        assert_eq!(
            prompt_text(&private, "zh-CN", Side::Gm).as_deref(),
            Some("（角色私設）〈狐狸〉\n私有設定：\n阿濤 不知道牠其實是妖狐。")
        );
        assert_eq!(
            prompt_text(&private, "ja", Side::Gm),
            prompt_text(&private, "en", Side::Gm)
        );

        let no_private = card("fox-id", "狐狸", "  \n", "  \n");
        assert_eq!(
            card_private(
                &no_private,
                &crate::transport::test_support::plain_fill("阿濤")
            ),
            None
        );
        let (marker, text) = card_arrival(
            &no_private,
            &crate::transport::test_support::plain_fill("阿濤"),
        );
        assert_eq!(text, "");
        assert_eq!(
            prompt_text(&marked(marker, &text, false), "ru", Side::Character).as_deref(),
            Some("(Character returns) “狐狸”")
        );
    }

    /// 角色側渲染：私設事件整則略過、帶代碼的 gm_only 只留標頭、無代碼與未知代碼的 gm_only
    /// 留本文首行；GM 側全文；英文介面出英文標頭。
    #[test]
    fn character_side_hides_card_private_and_gm_only_bodies() {
        let fox = card("fox-id", "狐狸", "尾巴很大。", "其實是妖狐。");
        let (marker, text) =
            card_private(&fox, &crate::transport::test_support::plain_fill("阿濤")).unwrap();
        let private = marked(marker, &text, true);
        let (marker, text) =
            card_arrival(&fox, &crate::transport::test_support::plain_fill("阿濤"));
        let public = marked(marker, &text, false);
        let person = marked(
            EventMarker::PersonArrival {
                title: "密探".to_owned(),
            },
            "全文",
            true,
        );
        let mut plain = event(TranscriptKind::System, "", "GM", "擲骰 3\n只有 GM 知道");
        plain.gm_only = true;
        let unknown = marked(EventMarker::Unknown, "未知本文\n第二行", true);
        let line = event(
            TranscriptKind::Dialogue,
            "fox-id",
            "狐狸",
            "私有設定：只是台詞",
        );

        let side = Side::Character;
        assert_eq!(prompt_text(&private, "zh-TW", side), None);
        assert_eq!(
            prompt_text(&public, "zh-TW", side).as_deref(),
            Some("（角色回歸）〈狐狸〉\n公開設定：\n尾巴很大。")
        );
        assert_eq!(
            prompt_text(&person, "zh-TW", side).as_deref(),
            Some("（人物登場）〈密探〉")
        );
        assert_eq!(
            prompt_text(&plain, "zh-TW", side).as_deref(),
            Some("擲骰 3")
        );
        assert_eq!(
            prompt_text(&unknown, "zh-TW", side).as_deref(),
            Some("未知本文")
        );
        assert_eq!(
            prompt_text(&unknown, "zh-TW", Side::Gm).as_deref(),
            Some("未知本文\n第二行")
        );
        assert_eq!(
            prompt_text(&line, "zh-TW", side).as_deref(),
            Some("私有設定：只是台詞")
        );
        assert_eq!(
            prompt_text(&person, "en", Side::Gm).as_deref(),
            Some("(New arrival) “密探”\n全文")
        );

        let view = render_for_prompt(
            &[private, public, person, plain, unknown, line],
            "zh-TW",
            side,
        );
        assert_eq!(view.len(), 5);
        assert!(view.iter().all(|event| event.marker.is_none()));
        assert!(view.iter().all(|event| !event.text.contains("妖狐")));
        assert!(view.iter().all(|event| !event.text.contains("全文")));
    }

    /// 提示詞模板：zh* 出繁中標頭、其餘出英文標頭；沒名字的玩家發言與點名退回原始語系稱呼。
    #[test]
    fn prompt_headings_follow_prompt_language_with_player_fallbacks() {
        let summary = TranscriptEvent {
            kind: TranscriptKind::Narration,
            ..marked(EventMarker::SceneSummary, "摘要", false)
        };
        let state = marked(EventMarker::StateUpdate, "hp：3", false);
        let call = marked(
            EventMarker::GmCall {
                name: String::new(),
            },
            "",
            false,
        );
        let named_call = marked(
            EventMarker::GmCall {
                name: "狐狸".to_owned(),
            },
            "",
            false,
        );
        let player = event(TranscriptKind::Player, "", "", "你好");
        for lang in ["zh-TW", "zh-CN"] {
            let side = Side::Character;
            assert_eq!(
                prompt_text(&summary, lang, side).as_deref(),
                Some("【前情提要】\n摘要")
            );
            assert_eq!(
                prompt_text(&state, lang, side).as_deref(),
                Some("狀態更新\nhp：3")
            );
            assert_eq!(
                prompt_text(&named_call, lang, side).as_deref(),
                Some("GM 請「狐狸」發言")
            );
        }
        assert_eq!(
            prompt_text(&call, "zh-TW", Side::Gm).as_deref(),
            Some("GM 請「玩家」發言")
        );
        assert_eq!(
            prompt_text(&summary, "en", Side::Gm).as_deref(),
            Some("Previously:\n摘要")
        );
        assert_eq!(
            prompt_text(&call, "en", Side::Gm).as_deref(),
            Some("GM asks “Player” to speak")
        );
        for lang in ["ja", "ru", "fr"] {
            for event in [&summary, &state, &named_call] {
                assert_eq!(
                    prompt_text(event, lang, Side::Character),
                    prompt_text(event, "en", Side::Character)
                );
            }
        }
        assert_eq!(
            prompt_text(&call, "ja", Side::Gm).as_deref(),
            Some("GM asks “プレイヤー” to speak")
        );
        assert_eq!(prompt_speaker(&player, "ja"), "プレイヤー");
        assert_eq!(prompt_speaker(&player, "en"), "Player");
        let rendered = render_for_prompt(&[player], "de", Side::Gm);
        assert_eq!(rendered[0].speaker_name, "Spieler");
    }

    /// 私設事件不是回歸事件：不進「本幕已回歸」集合，回歸判定與換幕結算照舊只認公開那則。
    #[test]
    fn card_private_event_is_not_counted_as_arrival() {
        let fox = card("fox-id", "狐狸", "尾巴很大。", "其實是妖狐。");
        let (marker, text) =
            card_private(&fox, &crate::transport::test_support::plain_fill("阿濤")).unwrap();
        let private = marked(marker, &text, true);
        assert!(data::appeared_card_names(std::slice::from_ref(&private)).is_empty());
        let (marker, text) =
            card_arrival(&fox, &crate::transport::test_support::plain_fill("阿濤"));
        let public = marked(marker, &text, false);
        assert_eq!(
            data::appeared_card_names(&[private, public]),
            BTreeSet::from(["狐狸".to_owned()])
        );
    }
}
