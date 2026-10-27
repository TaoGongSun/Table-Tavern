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

/// present 欄斷詞／在場名字比對是 `data::split_present_names`／`data::match_present`（原文優先、簡繁折疊兜底；
/// 放 data 層給角色卡換幕結算共用）；`state_scope` 用同一套的單向包含版 `match_present_with`。
///
/// 這一輪新面孔：`is_person && !disabled` 條目裡，present 名單比對得上、且不在
/// `already_appeared` 裡的那些，依世界書順序回傳；呼叫端逐一 append 成登場事件。
/// `card_names`：桌上全部角色卡的名字，只拿來判折疊歧義——present 寫的其實是角色卡「沈郁」時，
/// 人物條目「沈鬱」不能因為折疊後同名就登場、送出全文（整桌人物同一個候選池）。
///
/// - `present` 是 `None`（table 沒有這個鍵）：退回正文比對，`reply_body` 包含 title 即命中。
/// - `present` 是 `Some("")`（鍵存在但空／裁完是空清單）：只信 present，不做正文比對。
pub fn detect_new_arrivals<'a>(
    worldbook: &'a [WorldbookEntry],
    card_names: &[String],
    present: Option<&str>,
    reply_body: &str,
    already_appeared: &BTreeSet<String>,
) -> Vec<&'a WorldbookEntry> {
    let pool: Vec<&WorldbookEntry> = worldbook
        .iter()
        .filter(|entry| entry.is_person && !entry.disabled)
        .collect();
    let titles: Vec<&str> = pool
        .iter()
        .map(|entry| entry.title.as_str())
        .chain(card_names.iter().map(String::as_str))
        .collect();
    let present_match =
        present.map(|raw| data::match_present(&data::split_present_names(raw), &titles));
    pool.into_iter()
        .enumerate()
        .filter(|(_, entry)| !already_appeared.contains(&entry.title))
        // 折疊歧義的不送全文（data::match_present）
        .filter(|(index, entry)| match &present_match {
            Some(found) => found.matched.contains(index),
            None => reply_body.contains(&entry.title),
        })
        .map(|(_, entry)| entry)
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
// 隱藏欄位（data::CharacterMeta.auto_hidden）只在換幕結算（data::begin_next_scene）與退回／分岔復原改動，
// 這裡（回合中）只偵測與 append 事件，不碰欄位本身。
// ---------------------------------------------------------------------

/// 這一輪新回歸的角色卡：`auto_hidden && !archived` 的卡裡，present 名單比對得上（缺席退回
/// 正文比對）、且本幕還沒回歸過的，依卡片清單順序回傳；呼叫端逐一 append 成回歸事件。
/// 鏡射 `detect_new_arrivals`，鍵從世界書 title 換成卡片 name。
/// `other_names`：桌上其他人物的名字（主區與封存的卡、`is_person && !disabled` 的人物條目），只拿來判
/// 折疊歧義——present 寫的其實是主區那張或某個人物條目時，隱藏卡不能因為折疊後同名就被當成回歸、送出全文。
pub fn detect_new_card_arrivals<'a>(
    cards: &'a [CharacterCard],
    other_names: &[String],
    present: Option<&str>,
    reply_body: &str,
    already_appeared: &BTreeSet<String>,
) -> Vec<&'a CharacterCard> {
    let names: Vec<&str> = cards
        .iter()
        .map(|card| card.name.as_str())
        .chain(other_names.iter().map(String::as_str))
        .collect();
    let present_match =
        present.map(|raw| data::match_present(&data::split_present_names(raw), &names));
    cards
        .iter()
        .enumerate()
        .filter(|(_, card)| !already_appeared.contains(&card.name))
        .filter(|(index, card)| match &present_match {
            Some(found) => found.matched.contains(index),
            None => reply_body.contains(&card.name),
        })
        .map(|(_, card)| card)
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

    /// 真流程：舊名單非空，GM 回覆的圍欄寫裸 `present:` → 套用後平欄是空字串（Some("")）→
    /// 只信 present、不退回正文比對：正文提到名字也不登場、不回歸。
    #[test]
    fn bare_present_clears_the_old_list_and_keeps_body_fallback_off() {
        let mut world: data::WorldState =
            serde_json::from_value(serde_json::json!({"id": "w", "name": "桌"})).unwrap();
        world
            .state
            .table
            .insert("present".to_owned(), "林远、阿古拉".to_owned());
        let reply =
            "阿古拉和铁三都走了，院子裡只剩風聲。\n```state\ntime: 夜\nplace: 前院\npresent:\n```";
        let block = extract_state_block(reply);
        mechanism::apply_block(&mut world, &block, "林远");
        let present = world.state.table.get("present").map(String::as_str);
        assert_eq!(present, Some(""));
        assert!(world.state.tree.get("present").is_none(), "不長出空分支");

        let person = WorldbookEntry {
            is_person: true,
            ..worldbook_entry(1, "阿古拉", &[], true, 0, false, Visibility::Public)
        };
        let iron = card("iron", "铁三", "", "");
        let already = BTreeSet::new();
        assert!(detect_new_arrivals(&[person], &[], present, &block.display, &already).is_empty());
        assert!(
            detect_new_card_arrivals(&[iron], &[], present, &block.display, &already).is_empty()
        );
    }

    /// A/B 重放的實際輸出（haiku，繁中介面）：present 換成繁體後，簡體名冊人物照樣登場、
    /// 隱藏的簡體角色卡照樣回歸；沒條目的「商隊領頭」不誤觸任何人。
    #[test]
    fn interface_script_present_from_a_real_reply_still_triggers_arrivals() {
        let person = |uid: u64, title: &str| WorldbookEntry {
            is_person: true,
            ..worldbook_entry(uid, title, &[], true, 0, false, Visibility::Gm)
        };
        let entries = [person(1, "沈书吏"), person(2, "阿古拉")];
        let cards = [card("he", "贺铮", "", ""), card("zhou", "周掌柜", "", "")];
        let present = Some("林遠、賀錚、鐵三、阿古拉、周掌櫃、商隊領頭");
        let already = BTreeSet::new();
        let titles: Vec<&str> = detect_new_arrivals(&entries, &[], present, "", &already)
            .into_iter()
            .map(|entry| entry.title.as_str())
            .collect();
        assert_eq!(titles, vec!["阿古拉"]);
        let names: Vec<&str> = detect_new_card_arrivals(&cards, &[], present, "", &already)
            .into_iter()
            .map(|card| card.name.as_str())
            .collect();
        assert_eq!(names, vec!["贺铮", "周掌柜"]);
    }

    /// 已知限制（現況記錄）：原字精確優先。人物「沈鬱」在簡中介面被寫成「沈郁」，同桌另有「沈郁」時，
    /// 精確命中另一人；同桌只有「沈鬱」時折疊唯一命中、照常登場。
    #[test]
    fn interface_script_name_hits_the_exact_namesake_before_folding() {
        let person = |uid: u64, title: &str| WorldbookEntry {
            is_person: true,
            ..worldbook_entry(uid, title, &[], true, 0, false, Visibility::Gm)
        };
        let already = BTreeSet::new();
        let both = [person(1, "沈鬱"), person(2, "沈郁")];
        let titles: Vec<&str> = detect_new_arrivals(&both, &[], Some("沈郁"), "", &already)
            .into_iter()
            .map(|entry| entry.title.as_str())
            .collect();
        assert_eq!(titles, vec!["沈郁"], "算到另一個沈郁頭上");
        let alone = [person(1, "沈鬱")];
        let titles: Vec<&str> = detect_new_arrivals(&alone, &[], Some("沈郁"), "", &already)
            .into_iter()
            .map(|entry| entry.title.as_str())
            .collect();
        assert_eq!(titles, vec!["沈鬱"]);
    }

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
        let arrivals = detect_new_arrivals(&entries, &[], Some("亞歷山大、船長"), "", &already);
        assert_eq!(arrivals.len(), 1);
        assert_eq!(arrivals[0].title, "亞歷山大・馮・史特勞斯");
    }

    /// 簡繁折疊：繁體 present 也能讓簡體標題登場；折疊後分不出是誰（沈郁／沈鬱）就誰都不送全文
    #[test]
    fn detect_new_arrivals_folds_glyphs_but_sends_nobody_on_ambiguity() {
        let person = |uid: u64, title: &str| WorldbookEntry {
            is_person: true,
            ..worldbook_entry(uid, title, &[], true, 0, false, Visibility::Gm)
        };
        let entries = [
            person(1, "阿古拉"),
            person(2, "铁三"),
            person(3, "沈郁"),
            person(4, "沈鬱"),
        ];
        let already = BTreeSet::new();
        let titles: Vec<&str> =
            detect_new_arrivals(&entries, &[], Some("林遠、鐵三、瀋鬱"), "", &already)
                .into_iter()
                .map(|entry| entry.title.as_str())
                .collect();
        assert_eq!(titles, vec!["铁三"]);
        let titles: Vec<&str> = detect_new_arrivals(&entries, &[], Some("沈鬱"), "", &already)
            .into_iter()
            .map(|entry| entry.title.as_str())
            .collect();
        assert_eq!(titles, vec!["沈鬱"], "原文精確優先");
    }

    /// 整桌一個候選池：角色卡 沈郁 被點名時，人物條目 沈鬱 不登場；人物條目 沈郁 被點名時，
    /// 隱藏卡 沈鬱 不回歸——兩邊都不替錯的人送全文。
    #[test]
    fn person_and_card_arrivals_share_one_name_pool() {
        let person = WorldbookEntry {
            is_person: true,
            ..worldbook_entry(1, "沈鬱", &[], true, 0, false, Visibility::Gm)
        };
        let entries = [person];
        let already = BTreeSet::new();
        assert!(
            detect_new_arrivals(&entries, &["沈郁".to_owned()], Some("沈郁"), "", &already)
                .is_empty()
        );
        assert_eq!(
            detect_new_arrivals(&entries, &["沈郁".to_owned()], Some("沈鬱"), "", &already).len(),
            1
        );
        let hidden = [card("hidden", "沈鬱", "", "")];
        assert!(detect_new_card_arrivals(
            &hidden,
            &["沈郁".to_owned()],
            Some("沈郁"),
            "",
            &already
        )
        .is_empty());
    }

    /// 主區的 沈郁 被點名時，隱藏的 沈鬱 不能因為折疊後同名被當成回歸（不可替錯的人送全文）
    #[test]
    fn card_arrival_does_not_steal_a_name_that_exactly_matches_a_visible_card() {
        let hidden = [card("hidden", "沈鬱", "", ""), card("iron", "铁三", "", "")];
        let others = ["沈郁".to_owned()];
        let already = BTreeSet::new();
        let names: Vec<&str> =
            detect_new_card_arrivals(&hidden, &others, Some("沈郁、鐵三"), "", &already)
                .into_iter()
                .map(|card| card.name.as_str())
                .collect();
        assert_eq!(names, vec!["铁三"]);
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
        let arrivals = detect_new_arrivals(&entries, &[], Some("愛麗絲"), "", &already);
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

        let repeat = detect_new_arrivals(&entries, &[], Some("愛麗絲"), "", &already_this_scene);
        assert!(repeat.is_empty());

        let next_scene_events: [TranscriptEvent; 0] = [];
        let already_next_scene = data::appeared_person_titles(&next_scene_events);
        let reappear = detect_new_arrivals(&entries, &[], Some("愛麗絲"), "", &already_next_scene);
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

        let via_body = detect_new_arrivals(&entries, &[], None, "愛麗絲推門進來。", &already);
        assert_eq!(via_body.len(), 1);

        let empty_present =
            detect_new_arrivals(&entries, &[], Some(""), "愛麗絲推門進來。", &already);
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
        let arrivals = detect_new_card_arrivals(&hidden, &[], Some("亞歷山大、船長"), "", &already);
        assert_eq!(arrivals.len(), 1);
        assert_eq!(arrivals[0].name, "亞歷山大・馮・史特勞斯");
    }

    /// 本幕已回歸過的卡（already_appeared 裡有）就算 present 再報也不重複回傳。
    #[test]
    fn detect_new_card_arrivals_skips_already_appeared_names() {
        let fox = card("fox-id", "狐狸", "", "");
        let hidden = [fox];
        let already: BTreeSet<String> = BTreeSet::from(["狐狸".to_owned()]);
        let arrivals = detect_new_card_arrivals(&hidden, &[], Some("狐狸"), "", &already);
        assert!(arrivals.is_empty());
    }

    /// present 鍵不存在就退回正文比對；鍵存在但是空字串只信 present，
    /// 就算正文裡有名字也不算數。
    #[test]
    fn detect_new_card_arrivals_falls_back_to_reply_body_only_when_present_key_is_absent() {
        let fox = card("fox-id", "狐狸", "", "");
        let hidden = [fox];
        let already = BTreeSet::new();

        let via_body =
            detect_new_card_arrivals(&hidden, &[], None, "狐狸從陰影裡走出來。", &already);
        assert_eq!(via_body.len(), 1);

        let empty_present =
            detect_new_card_arrivals(&hidden, &[], Some(""), "狐狸從陰影裡走出來。", &already);
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
