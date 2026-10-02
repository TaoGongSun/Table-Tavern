use crate::data::{self, CharacterCard, TranscriptEvent, TranscriptKind, WorldbookEntry};

use std::borrow::Cow;
use std::collections::BTreeSet;

use super::messages::replace_st_macros;

/// 人物登場事件的固定前綴，接著是〈title〉那一行；`append_transcript` 寫進去的格式，
/// 也是掃「本幕已登場集合」與下一包前端顯示唯一依據的字面。
pub const PERSON_ARRIVAL_PREFIX: &str = "（人物登場）";

/// 從一則事件文字剝出登場標題（前綴＋〈title〉開頭那一行）；不是登場事件就回 `None`。
fn arrival_title(text: &str) -> Option<String> {
    let rest = text
        .strip_prefix(PERSON_ARRIVAL_PREFIX)?
        .strip_prefix('〈')?;
    let end = rest.find('〉')?;
    Some(rest[..end].to_owned())
}

/// 本幕已登場集合：掃 transcript 裡帶登場前綴的 System 事件取出標題。換幕是新 jsonl，
/// 這個集合自然歸零，不必另外存狀態檔。
pub fn appeared_person_titles(events: &[TranscriptEvent]) -> BTreeSet<String> {
    events
        .iter()
        .filter(|event| event.kind == TranscriptKind::System)
        .filter_map(|event| arrival_title(&event.text))
        .collect()
}

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

/// 人物登場事件的內文：固定前綴＋〈title〉一行（原文，供比對回抽），接條目全文
/// （`{{user}}` 已代換——事件一旦落進 transcript 就不會再過巨集代換一次）。
pub fn person_arrival_text(entry: &WorldbookEntry, user_name: &str) -> String {
    format!(
        "{}〈{}〉\n{}",
        PERSON_ARRIVAL_PREFIX,
        entry.title,
        replace_st_macros(&entry.content, user_name, None)
    )
}

// ---------------------------------------------------------------------
// AI 卡重構包 4b：角色卡自動上下場，鏡射上面 4a 的世界書人物在場機制。角色卡的持久
// 隱藏欄位（data::CharacterMeta.auto_hidden）只在換幕結算（data::begin_next_scene）改動，
// 這裡（回合中）只偵測與 append 事件，不碰欄位本身。
// ---------------------------------------------------------------------

/// 本幕已回歸的角色卡集合：掃 transcript 裡帶 `data::CARD_ARRIVAL_PREFIX` 的 System 事件
/// 取出卡名。命名對齊 4a 的 `appeared_person_titles`。
pub fn appeared_card_names(events: &[TranscriptEvent]) -> BTreeSet<String> {
    data::appeared_titles(events, data::CARD_ARRIVAL_PREFIX)
}

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

/// 角色私設事件的固定前綴：角色卡回歸時私設另成一則 `gm_only` 事件，只有 GM 看得到，
/// 角色側（見 `character_visible_text`）整則略過。
pub const CARD_PRIVATE_PREFIX: &str = "（角色私設）";

/// 私設段標頭。舊版回歸事件把它接在公開設定後面，角色側渲染靠它截掉私設段。
const PRIVATE_SECTION: &str = "\n私有設定：";

/// 角色卡回歸事件的內文：固定前綴＋〈name〉一行，接公開設定（`{{user}}`／`{{char}}` 已代換）。
/// 所有線都看得到，呼叫端標 gm_only=false；私設另由 `card_private_text` 成一則 GM 專屬事件。
pub fn card_arrival_text(card: &CharacterCard, user_name: &str) -> String {
    let mut text = format!("{}〈{}〉", data::CARD_ARRIVAL_PREFIX, card.name);
    if !card.public_md.trim().is_empty() {
        text.push_str(&format!(
            "\n公開設定：\n{}",
            replace_st_macros(card.public_md.trim(), user_name, Some(&card.name))
        ));
    }
    text
}

/// 角色卡回歸時的私設事件內文；私設空白回 `None`（不產事件）。呼叫端標 gm_only=true。
pub fn card_private_text(card: &CharacterCard, user_name: &str) -> Option<String> {
    let private = card.private_md.trim();
    (!private.is_empty()).then(|| {
        format!(
            "{CARD_PRIVATE_PREFIX}〈{}〉{PRIVATE_SECTION}\n{}",
            card.name,
            replace_st_macros(private, user_name, Some(&card.name))
        )
    })
}

/// 舊版合併回歸事件：公開設定後面接著私設全文。檔案不改，角色側讀取時截掉私設段。
pub fn is_legacy_card_arrival(event: &TranscriptEvent) -> bool {
    event.kind == TranscriptKind::System
        && event.text.starts_with(data::CARD_ARRIVAL_PREFIX)
        && event.text.contains(PRIVATE_SECTION)
}

/// 非 GM 模型看得到的事件正文；`None`＝整則不給角色看。角色線、共線組裝、換幕摘要
/// （摘要會變成下一幕的公開旁白）與角色側 keyword 觸發共用這一個入口，GM 一律讀原文。
/// - 角色私設事件：整則略過；
/// - 其他 `gm_only` System 事件：只留第一行（前綴＋標題）；
/// - 舊版合併回歸事件：截掉私設段。
pub fn character_visible_text(event: &TranscriptEvent) -> Option<Cow<'_, str>> {
    if event.kind != TranscriptKind::System {
        return Some(Cow::Borrowed(&event.text));
    }
    if event.gm_only {
        if event.text.starts_with(CARD_PRIVATE_PREFIX) {
            return None;
        }
        return Some(Cow::Borrowed(event.text.lines().next().unwrap_or_default()));
    }
    if is_legacy_card_arrival(event) {
        let end = event.text.find(PRIVATE_SECTION).unwrap_or(event.text.len());
        return Some(Cow::Borrowed(&event.text[..end]));
    }
    Some(Cow::Borrowed(&event.text))
}

/// 角色側看到的事件序列（略過與遮罩見 `character_visible_text`）。只給 keyword 觸發這類
/// 「看內容」的用途；lane 水位、指紋與回覆對點一律用原事件序列，不可拿這份頂替。
pub fn character_events(events: &[TranscriptEvent]) -> Vec<TranscriptEvent> {
    events
        .iter()
        .filter_map(|event| {
            character_visible_text(event).map(|text| TranscriptEvent {
                text: text.into_owned(),
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

    /// 規格 (c)(d)(e) 打通：`person_arrival_text` 寫出的格式，`appeared_person_titles`
    /// 要能原樣掃回標題；同幕重複比對會被擋掉，換幕後（另一批空事件）比對又重新命中。
    #[test]
    fn arrival_text_round_trips_through_appeared_titles_and_dedupes_per_scene() {
        let alice = WorldbookEntry {
            is_person: true,
            ..worldbook_entry(1, "愛麗絲", &[], true, 0, false, Visibility::Public)
        };
        let entries = [alice];

        // 本幕：愛麗絲已經登場過一次（transcript 有一則登場事件）
        let arrival_event = event(
            TranscriptKind::System,
            "",
            "GM",
            &person_arrival_text(&entries[0], "阿濤"),
        );
        let this_scene_events = [arrival_event];
        let already_this_scene = appeared_person_titles(&this_scene_events);
        assert_eq!(already_this_scene, BTreeSet::from(["愛麗絲".to_owned()]));

        // 同幕再報 present 有她 → 不重複 append
        let repeat = detect_new_arrivals(&entries, Some("愛麗絲"), "", &already_this_scene);
        assert!(repeat.is_empty());

        // 換幕：新場景的 transcript 是空的，已登場集合自然歸零
        let next_scene_events: [TranscriptEvent; 0] = [];
        let already_next_scene = appeared_person_titles(&next_scene_events);
        let reappear = detect_new_arrivals(&entries, Some("愛麗絲"), "", &already_next_scene);
        assert_eq!(reappear.len(), 1);
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

    /// 登場事件文字格式：固定前綴＋〈title〉一行（原文，供比對），接著條目全文，
    /// `{{user}}` 已代換。
    #[test]
    fn person_arrival_text_has_prefix_title_line_and_macro_replaced_content() {
        let alice = WorldbookEntry {
            is_person: true,
            content: "{{user}} 認識她。".to_owned(),
            ..worldbook_entry(1, "愛麗絲", &[], true, 0, false, Visibility::Public)
        };
        let text = person_arrival_text(&alice, "阿濤");
        assert_eq!(text, "（人物登場）〈愛麗絲〉\n阿濤 認識她。");
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

    /// 回歸事件只帶公開設定；私設另成一則（`{{user}}` 已代換），私設空白不產事件。
    #[test]
    fn card_arrival_splits_public_event_and_private_event() {
        let fox = card(
            "fox-id",
            "狐狸",
            "{{user}} 認識牠。",
            "{{user}} 不知道牠其實是妖狐。",
        );
        assert_eq!(
            card_arrival_text(&fox, "阿濤"),
            "（角色回歸）〈狐狸〉\n公開設定：\n阿濤 認識牠。"
        );
        assert_eq!(
            card_private_text(&fox, "阿濤").as_deref(),
            Some("（角色私設）〈狐狸〉\n私有設定：\n阿濤 不知道牠其實是妖狐。")
        );

        let no_private = card("fox-id", "狐狸", "公開內容", "  \n");
        assert_eq!(card_private_text(&no_private, "阿濤"), None);
        assert_eq!(
            card_arrival_text(&no_private, "阿濤"),
            "（角色回歸）〈狐狸〉\n公開設定：\n公開內容"
        );
    }

    /// 角色側渲染：私設事件整則略過、其他 gm_only 只留第一行、舊合併回歸事件截掉私設段，
    /// 一般事件原文；`character_events` 照同規則產出，只少掉被略過的那則。
    #[test]
    fn character_view_hides_card_private_and_legacy_private_section() {
        let fox = card("fox-id", "狐狸", "尾巴很大。", "其實是妖狐。");
        let mut private = event(
            TranscriptKind::System,
            "",
            "GM",
            &card_private_text(&fox, "阿濤").unwrap(),
        );
        private.gm_only = true;
        let public = event(
            TranscriptKind::System,
            "",
            "GM",
            &card_arrival_text(&fox, "阿濤"),
        );
        let legacy = event(
            TranscriptKind::System,
            "",
            "GM",
            "（角色回歸）〈狐狸〉\n公開設定：\n尾巴很大。\n私有設定：\n其實是妖狐。",
        );
        let mut person = event(
            TranscriptKind::System,
            "",
            "GM",
            "（人物登場）〈密探〉\n全文",
        );
        person.gm_only = true;
        let line = event(
            TranscriptKind::Dialogue,
            "fox-id",
            "狐狸",
            "私有設定：只是台詞",
        );

        assert_eq!(character_visible_text(&private), None);
        assert_eq!(
            character_visible_text(&public).as_deref(),
            Some("（角色回歸）〈狐狸〉\n公開設定：\n尾巴很大。")
        );
        assert!(is_legacy_card_arrival(&legacy));
        assert!(!is_legacy_card_arrival(&public));
        assert_eq!(
            character_visible_text(&legacy).as_deref(),
            Some("（角色回歸）〈狐狸〉\n公開設定：\n尾巴很大。")
        );
        assert_eq!(
            character_visible_text(&person).as_deref(),
            Some("（人物登場）〈密探〉")
        );
        assert_eq!(
            character_visible_text(&line).as_deref(),
            Some("私有設定：只是台詞")
        );

        let view = character_events(&[private, public, legacy, person, line]);
        assert_eq!(view.len(), 4);
        assert!(view.iter().all(|event| !event.text.contains("妖狐")));
        assert!(view.iter().all(|event| !event.text.contains("全文")));
    }

    /// 私設事件不是回歸事件：不進「本幕已回歸」集合，回歸判定與換幕結算照舊只認公開那則。
    #[test]
    fn card_private_event_is_not_counted_as_arrival() {
        let fox = card("fox-id", "狐狸", "尾巴很大。", "其實是妖狐。");
        let mut private = event(
            TranscriptKind::System,
            "",
            "GM",
            &card_private_text(&fox, "阿濤").unwrap(),
        );
        private.gm_only = true;
        assert!(appeared_card_names(&[private.clone()]).is_empty());
        let public = event(
            TranscriptKind::System,
            "",
            "GM",
            &card_arrival_text(&fox, "阿濤"),
        );
        assert_eq!(
            appeared_card_names(&[private, public]),
            BTreeSet::from(["狐狸".to_owned()])
        );
    }
}
