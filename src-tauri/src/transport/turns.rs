use crate::data::{
    CharacterCard, Mechanism, TableState, TranscriptEvent, TranscriptKind, Visibility,
    WorldbookEntry,
};

use super::messages::{
    language_rule, message, narration_line, player_fallback_name, push_merged, replace_st_macros,
    scaffold_en, speaker_prefix, system_line, ChatMessage,
};

use super::arrivals::{prompt_speaker, prompt_text, render_for_prompt, Side};

use super::context::{
    active_worldbook_entries, gm_system_prompt, player_heading, split_person_roster,
};

use super::state_view::{character_state_block, gm_dynamic_block, StateScope};

/// resume 續聊線（prompt-cache-optimization 包 2）的回合尾段。
/// tail 是跟在新事件後送出的動態文字；confidential 是 tail 內回合結束後
/// 要從 session 檔抹掉的子段（chars 線的私設＋限定條目，防洩漏給下一個被點的角色）。
pub struct LaneTurn {
    pub tail: String,
    pub confidential: Option<String>,
    /// `hoist_private` 為真時，本輪角色的私設改由這裡回傳、不進 tail——單角色桌把它放進
    /// system 用。**只有 `private_md`**：限定世界書的 keyword 條目隨最近事件翻動、
    /// 狀態每輪變，那兩樣進了 system 會把前綴打散，正好毀掉共線要修的東西。
    pub hoisted_private: Option<String>,
}

/// 事件在 lane prompt 裡的一行；`None`＝這則不送。續聊線的歷史全部以名字標注成純文字
/// （誰說的靠「X：」前綴分辨，不靠 role），與 session 內既有歷史逐字銜接。
/// 可見性與標頭照 `prompt_text`：chars 線傳 `Side::Character`，GM 線傳 `Side::Gm`。
pub fn lane_event_line(event: &TranscriptEvent, lang: &str, side: Side) -> Option<String> {
    let text = prompt_text(event, lang, side)?;
    Some(match event.kind {
        TranscriptKind::Dialogue | TranscriptKind::Player => {
            format!(
                "{}{text}",
                speaker_prefix(&prompt_speaker(event, lang), lang)
            )
        }
        TranscriptKind::Narration => narration_line(&text, lang),
        TranscriptKind::System => system_line(&text, lang),
    })
}

/// chars 線「你知道的世界情報」段標（凍結 system 與回合尾共用），結尾帶換行。
fn known_world_heading(lang: &str) -> &'static str {
    match scaffold_en(lang) {
        true => "## World knowledge you have\n",
        false => "## 你知道的世界情報\n",
    }
}

/// chars 線凍結 system（快照）：中性扮演引擎指示＋全部公開角色卡＋玩家卡＋Public constant 條目。
/// 全角色共用一條 session，這一輪演誰由回合尾段指定；私設與限定條目不進快照
/// （E7：凍結 system 動一字整條快取全滅，快照只能放全員共通且穩定的素材）。
/// Public constant 裡的 is_person 條目改走名冊行，不進全文（包 4a，見 split_person_roster）。
pub fn chars_lane_system(
    cards: &[CharacterCard],
    player: Option<&CharacterCard>,
    worldbook: &[WorldbookEntry],
    lang: &str,
) -> String {
    let user_name = player
        .map(|player| player.name.as_str())
        .unwrap_or_else(|| player_fallback_name(lang));
    let en = scaffold_en(lang);
    let intro = match en {
        true => {
            "You are the roleplay engine for this multiplayer tabletop RPG session. Any character on the \
             \"Characters\" list may be played by you, and the end of each turn tells you whom to play this turn. \
             Always narrate in the third person: actions and inner thoughts take the assigned character's name or \
             \"he/she\" as the subject, and spoken words go in quotation marks in that character's own voice. \
             Keep the viewpoint with the assigned character: you may describe what they see around them and what \
             they feel, but not what they do not know. Do not use \"I\" as the narrative subject, do not break \
             character, do not speak as an AI assistant, and do not speak for other characters or the player."
        }
        false => {
            "你是這場多人桌上角色扮演的扮演引擎，「登場角色」名單上的角色都可能由你扮演，\
             每一輪的結尾會指定你這一輪演誰。請一律用第三人稱敘事：動作與心理描寫都以\
             被指定角色的名字或「他／她」當主詞，說出口的話寫在引號裡、維持這個角色自己的口吻；\
             視角只跟著被指定的角色，可以寫他眼中所見的環境與心裡的感受，不要寫他不知道的事；\
             敘述不要用「我」當主詞、不要跳出角色、不要以 AI 助理的身分說話、\
             不要替其他角色或玩家代言。"
        }
    };
    let separator = if en { " " } else { "" };
    let mut system = format!("{intro}{separator}{}\n", language_rule(lang));
    if !cards.is_empty() {
        system.push_str(match en {
            true => "\n## Characters (public profiles)\n",
            false => "\n## 登場角色（公開設定）\n",
        });
        for card in cards {
            system.push_str(&format!("### {}\n", card.name));
            if !card.public_md.trim().is_empty() {
                system.push_str(&format!(
                    "{}\n",
                    replace_st_macros(card.public_md.trim(), user_name, Some(&card.name))
                ));
            }
        }
    }
    if let Some(player) = player {
        system.push_str(&player_heading(&player.name, lang));
        if !player.public_md.trim().is_empty() {
            system.push_str(&format!(
                "\n{}\n",
                replace_st_macros(player.public_md.trim(), user_name, Some(&player.name))
            ));
        }
    }
    let mut constants: Vec<&WorldbookEntry> = worldbook
        .iter()
        .filter(|entry| {
            !entry.disabled && entry.constant && matches!(entry.visibility, Visibility::Public)
        })
        .collect();
    constants.sort_by_key(|entry| (entry.order, entry.uid));
    let (constants, roster) = split_person_roster(&constants, lang);
    if !constants.is_empty() || roster.is_some() {
        system.push('\n');
        system.push_str(known_world_heading(lang));
        for entry in constants {
            system.push_str(&format!(
                "### {}\n{}\n",
                replace_st_macros(&entry.title, user_name, None),
                replace_st_macros(&entry.content, user_name, None)
            ));
        }
        if let Some(roster) = roster {
            system.push_str(&roster);
            system.push('\n');
        }
    }
    system
}

/// chars 線回合尾段：公開 keyword 條目＋機密段（本輪角色的私設＋限定可見條目）＋本輪指定。
/// 機密段回合結束後從 session 檔抹掉；Public constant 條目已在凍結快照，不重複。
#[allow(clippy::too_many_arguments)]
pub fn chars_lane_turn(
    card: &CharacterCard,
    player: Option<&CharacterCard>,
    events: &[TranscriptEvent],
    worldbook: &[WorldbookEntry],
    state: &TableState,
    mechanism: &Mechanism,
    branch: Option<&[String]>,
    lang: &str,
    hoist_private: bool,
) -> LaneTurn {
    let user_name = player
        .map(|player| player.name.as_str())
        .unwrap_or_else(|| player_fallback_name(lang));
    let visible: Vec<WorldbookEntry> = worldbook
        .iter()
        .filter(|entry| match &entry.visibility {
            Visibility::Gm => false,
            Visibility::Public => true,
            Visibility::Characters(ids) => ids.iter().any(|id| id == &card.id),
        })
        .cloned()
        .collect();
    let mut public_keyword = Vec::new();
    let mut limited = Vec::new();
    // 觸發看角色側看得到的事件：私設裡的關鍵字不能替角色啟用條目，被略過的事件也不佔最近四則
    let visible_events = render_for_prompt(events, lang, Side::Character);
    for entry in active_worldbook_entries(&visible, &visible_events) {
        match &entry.visibility {
            Visibility::Public if entry.constant => {} // 已在凍結快照
            Visibility::Public => public_keyword.push(entry),
            _ => limited.push(entry), // Characters 限定：不論 constant 都走回合注入
        }
    }

    let mut tail = String::new();
    if !public_keyword.is_empty() {
        tail.push_str(known_world_heading(lang));
        for entry in public_keyword {
            tail.push_str(&format!(
                "### {}\n{}\n",
                replace_st_macros(&entry.title, user_name, Some(&card.name)),
                replace_st_macros(&entry.content, user_name, Some(&card.name))
            ));
        }
        tail.push('\n');
    }
    let mut confidential = String::new();
    let mut hoisted_private = None;
    if !card.private_md.trim().is_empty() {
        let heading = match scaffold_en(lang) {
            true => format!(
                "## {}'s private profile (only they know this; do not reveal it unless the story gets there)",
                card.name
            ),
            false => format!(
                "## 「{}」的私有設定（只有他自己知道；除非劇情走到，不要主動說破）",
                card.name
            ),
        };
        let block = format!(
            "{heading}\n{}\n",
            replace_st_macros(card.private_md.trim(), user_name, Some(&card.name))
        );
        match hoist_private {
            true => hoisted_private = Some(block),
            false => confidential.push_str(&block),
        }
    }
    if !limited.is_empty() {
        confidential.push_str(&match scaffold_en(lang) {
            true => format!("## World knowledge only {} has\n", card.name),
            false => format!("## 只有「{}」知道的世界情報\n", card.name),
        });
        for entry in limited {
            confidential.push_str(&format!(
                "### {}\n{}\n",
                replace_st_macros(&entry.title, user_name, Some(&card.name)),
                replace_st_macros(&entry.content, user_name, Some(&card.name))
            ));
        }
    }
    if let Some(block) = branch.and_then(|branch| {
        character_state_block(state, mechanism, branch, &card.name, user_name, lang)
    }) {
        confidential.push_str(&block);
        confidential.push('\n');
    }
    if !confidential.is_empty() {
        tail.push_str(&confidential);
        tail.push('\n');
    }
    tail.push_str(&match scaffold_en(lang) {
        true => format!(
            "You are now \"{name}\". Write {name}'s actions, lines, and inner thoughts directly in the third person, \
             with \"{name}\" or \"he/she\" as the subject and never \"I\"; put spoken words in quotation marks. \
             Do not add a name prefix or any explanation outside the character.",
            name = card.name
        ),
        false => format!(
            "現在你是「{name}」。請直接用第三人稱輸出「{name}」的動作、台詞與心理描寫，\
             敘述主詞是「{name}」或「他／她」、不要用「我」，說出口的話寫在引號裡；\
             不要加名字前綴、不要任何角色之外的說明。",
            name = card.name
        ),
    });
    LaneTurn {
        tail,
        confidential: (!confidential.is_empty()).then_some(confidential),
        hoisted_private,
    }
}

/// gm 線凍結 system（快照）：GM 指示＋world.md＋全 constant 條目＋全卡（含私設）＋玩家卡。
/// GM 看得到一切，constant 條目不分可見性全部進快照。
pub fn gm_lane_system(
    world_md: &str,
    cards: &[CharacterCard],
    player: Option<&CharacterCard>,
    worldbook: &[WorldbookEntry],
    mechanism: &Mechanism,
    lang: &str,
) -> String {
    let user_name = player
        .map(|player| player.name.as_str())
        .unwrap_or_else(|| player_fallback_name(lang));
    let mut constants: Vec<&WorldbookEntry> = worldbook
        .iter()
        .filter(|entry| !entry.disabled && entry.constant)
        .collect();
    constants.sort_by_key(|entry| (entry.order, entry.uid));
    gm_system_prompt(
        world_md, cards, player, &constants, user_name, mechanism, lang,
    )
}

/// gm 線回合尾段：keyword 條目＋目前狀態＋導演指示（旁白＋點名合併版，由呼叫端組好傳入）。
#[allow(clippy::too_many_arguments)]
pub fn gm_lane_turn(
    events: &[TranscriptEvent],
    worldbook: &[WorldbookEntry],
    player: Option<&CharacterCard>,
    state: &TableState,
    mechanism: &Mechanism,
    scope: &StateScope,
    instruction: &str,
    lang: &str,
) -> LaneTurn {
    let user_name = player
        .map(|player| player.name.as_str())
        .unwrap_or_else(|| player_fallback_name(lang));
    let rendered = render_for_prompt(events, lang, Side::Gm);
    let keyword_entries: Vec<&WorldbookEntry> = active_worldbook_entries(worldbook, &rendered)
        .into_iter()
        .filter(|entry| !entry.constant)
        .collect();
    let dynamic = gm_dynamic_block(&keyword_entries, state, user_name, mechanism, scope, lang);
    let mut tail = String::new();
    if !dynamic.is_empty() {
        tail.push_str(&dynamic);
        tail.push_str("\n\n");
    }
    tail.push_str(instruction);
    LaneTurn {
        tail,
        confidential: None,
        hoisted_private: None, // GM 線沒有「本輪角色的私設」這個概念
    }
}

/// 組裝「換場摘要」上下文：GM 檔位讀角色側看得到的 transcript，把本場景壓成一則前情提要。
/// 不含 world.md／角色卡——摘要只需壓縮已發生的公開事件，不需要世界觀全貌。
pub fn summary_messages(events: &[TranscriptEvent], lang: &str) -> Vec<ChatMessage> {
    let instruction = if scaffold_en(lang) {
        format!(
            "You are the GM of a multiplayer tabletop RPG session that is about to change scenes. \
             The first line of your reply must be exactly \"Title: <act name, 10 words or fewer>\", \
             followed by a blank line before the recap. \
             Summarize everything that happened in this scene as a recap, covering: \
             location and time, who is present and their state, key events, relationship changes, \
             and unresolved threads — as a compact bulleted list. \
             Output only the summary body. {language_rule}",
            language_rule = language_rule(lang),
        )
    } else {
        format!(
            "你是這場多人桌上角色扮演的 GM，現在要換場。\
             回覆第一行固定輸出「標題：〈10 字內的幕名〉」，空一行後才是摘要條列。\
             請把本場景發生的一切壓成一則前情提要，條列涵蓋：\
             地點與時間、在場人物與狀態、關鍵事件、關係變化、未解懸念。\
             {language_rule}",
            language_rule = language_rule(lang),
        )
    };

    let mut messages = vec![message("system", instruction)];
    // 摘要會變成下一幕的公開旁白、回到每個角色的上下文，只能讀角色側看得到的內容
    for line in events
        .iter()
        .filter_map(|event| lane_event_line(event, lang, Side::Character))
    {
        push_merged(&mut messages, "user", line);
    }
    messages
}

#[cfg(test)]
mod tests {
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
    use super::super::response::*;
    #[allow(unused_imports)]
    use super::super::state_view::*;
    #[allow(unused_imports)]
    use super::super::test_support::{card, event, worldbook_entry};
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

    /// 驗收：換場摘要指示依語系切換，且 transcript 事件正確攤平成 user 訊息
    #[test]
    fn summary_messages_follow_lang_and_include_transcript() {
        let events = [
            event(TranscriptKind::Narration, "", "GM", "夜幕低垂"),
            event(TranscriptKind::Player, "", "玩家", "老闆，來杯麥酒"),
            event(TranscriptKind::Dialogue, "fox-id", "狐狸", "馬上來！"),
        ];
        let zh = summary_messages(&events, "zh-TW");
        assert_eq!(zh[0].role, "system");
        assert!(zh[0].content.contains("前情提要"));
        let joined: String = zh.iter().map(|m| m.content.as_str()).collect();
        assert!(joined.contains("（旁白）夜幕低垂"));
        assert!(joined.contains("玩家：老闆，來杯麥酒"));
        assert!(joined.contains("狐狸：馬上來！"));

        let en = summary_messages(&events, "en");
        assert!(en[0].content.contains("recap"));
    }

    /// chars 線凍結快照只能有全員共通且穩定的素材：公開卡＋玩家卡＋Public constant。
    /// 私設、GM 專有、角色限定、keyword 條目一律不進快照（回合注入或不可見）。
    #[test]
    fn chars_lane_snapshot_holds_shared_public_material_only() {
        let fox = card("fox-id", "狐狸", "旅店老闆", "其實是通緝犯");
        let knight = card("knight-id", "騎士", "遊歷的騎士", "");
        let player = card("player-id", "阿濤", "商隊護衛", "");
        let entries = [
            worldbook_entry(1, "公開常識", &[], true, 0, false, Visibility::Public),
            worldbook_entry(2, "GM專有", &[], true, 0, false, Visibility::Gm),
            worldbook_entry(
                3,
                "狐狸限定",
                &[],
                true,
                0,
                false,
                Visibility::Characters(vec!["fox-id".to_owned()]),
            ),
            worldbook_entry(
                4,
                "關鍵字條目",
                &["寶箱"],
                false,
                0,
                false,
                Visibility::Public,
            ),
        ];
        let snapshot = chars_lane_system(
            &[fox.clone(), knight.clone()],
            Some(&player),
            &entries,
            "zh-TW",
        );
        assert!(snapshot.contains("扮演引擎"));
        assert!(snapshot.contains("旅店老闆"));
        assert!(snapshot.contains("遊歷的騎士"));
        assert!(snapshot.contains("阿濤"));
        assert!(snapshot.contains("公開常識內容"));
        assert!(!snapshot.contains("通緝犯"));
        assert!(!snapshot.contains("GM專有"));
        assert!(!snapshot.contains("狐狸限定"));
        assert!(!snapshot.contains("關鍵字條目"));
        // 快照不依賴 events，本質上逐輪穩定；再組一次逐字相同
        assert_eq!(
            snapshot,
            chars_lane_system(&[fox, knight], Some(&player), &entries, "zh-TW")
        );
    }

    /// chars 線回合尾段：公開 keyword 條目留在 tail、私設與限定條目集中在 confidential；
    /// confidential 在 tail 中恰好出現一次（回合後靠這個子段從 session 檔抹掉）。
    #[test]
    fn chars_lane_turn_isolates_confidential_segment() {
        let fox = card("fox-id", "狐狸", "旅店老闆", "其實是通緝犯");
        let events = [event(
            TranscriptKind::Player,
            "",
            "阿濤",
            "打開寶箱，讀羊皮卷",
        )];
        let entries = [
            worldbook_entry(1, "公開常識", &[], true, 0, false, Visibility::Public),
            worldbook_entry(
                2,
                "寶箱情報",
                &["寶箱"],
                false,
                0,
                false,
                Visibility::Public,
            ),
            worldbook_entry(
                3,
                "羊皮卷密文",
                &["羊皮卷"],
                false,
                0,
                false,
                Visibility::Characters(vec!["fox-id".to_owned()]),
            ),
            worldbook_entry(
                4,
                "狐狸長設",
                &[],
                true,
                0,
                false,
                Visibility::Characters(vec!["fox-id".to_owned()]),
            ),
        ];
        let turn = chars_lane_turn(
            &fox,
            None,
            &events,
            &entries,
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
            false,
        );
        let confidential = turn.confidential.expect("私設＋限定條目必須進機密段");
        assert!(confidential.contains("通緝犯"));
        assert!(confidential.contains("羊皮卷密文內容"));
        assert!(confidential.contains("狐狸長設內容")); // 限定 constant 也走回合注入
        assert!(!confidential.contains("寶箱情報"));
        assert_eq!(turn.tail.matches(confidential.as_str()).count(), 1);
        // 抹掉機密段後，公開條目與本輪指定仍在（session 歷史剩這些）
        let erased = turn.tail.replacen(confidential.as_str(), "", 1);
        assert!(erased.contains("寶箱情報內容"));
        assert!(erased.contains("現在你是「狐狸」"));
        assert!(!erased.contains("公開常識")); // constant 已在快照，不重複
                                               // 沒有私設也沒有限定條目時不產生機密段
        let knight = card("knight-id", "騎士", "遊歷的騎士", "");
        let plain = chars_lane_turn(
            &knight,
            None,
            &events,
            &entries[..2],
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
            false,
        );
        assert!(plain.confidential.is_none());
        assert!(plain.tail.contains("現在你是「騎士」"));
    }

    /// gm 線凍結快照＝GM 單發 system 的同等素材（全 constant＋全卡含私設＋world.md）；
    /// 回合尾段＝keyword 條目＋目前狀態＋導演指示。
    #[test]
    fn gm_lane_snapshot_and_turn_cover_gm_material() {
        let fox = card("fox-id", "狐狸", "旅店老闆", "其實是通緝犯");
        let events = [event(TranscriptKind::Player, "", "阿濤", "打開寶箱")];
        let entries = [
            worldbook_entry(1, "GM專有", &[], true, 0, false, Visibility::Gm),
            worldbook_entry(
                2,
                "寶箱情報",
                &["寶箱"],
                false,
                0,
                false,
                Visibility::Public,
            ),
        ];
        let snapshot = gm_lane_system(
            "世界總覽",
            &[fox],
            None,
            &entries,
            &Mechanism::default(),
            "zh-TW",
        );
        assert!(snapshot.contains("世界總覽"));
        assert!(snapshot.contains("GM專有內容"));
        assert!(snapshot.contains("通緝犯"));
        assert!(!snapshot.contains("寶箱情報"));

        let mut state = TableState::default();
        state.table.insert("place".to_owned(), "酒館".to_owned());
        let turn = gm_lane_turn(
            &events,
            &entries,
            None,
            &state,
            &Mechanism::default(),
            &StateScope::default(),
            "（導演指示）請插入旁白。",
            "zh-TW",
        );
        assert!(turn.confidential.is_none());
        assert!(turn.tail.contains("寶箱情報內容"));
        assert!(turn.tail.contains("地點：酒館"));
        assert!(turn.tail.ends_with("（導演指示）請插入旁白。"));
    }

    #[test]
    fn lane_event_line_labels_every_kind_by_name() {
        assert_eq!(
            lane_event_line(
                &event(TranscriptKind::Dialogue, "fox-id", "狐狸", "晚安"),
                "zh-TW",
                Side::Gm
            )
            .as_deref(),
            Some("狐狸：晚安")
        );
        assert_eq!(
            lane_event_line(
                &event(TranscriptKind::Player, "", "阿濤", "好啊"),
                "zh-TW",
                Side::Gm
            )
            .as_deref(),
            Some("阿濤：好啊")
        );
        assert_eq!(
            lane_event_line(
                &event(TranscriptKind::Narration, "", "GM", "夜深了"),
                "zh-TW",
                Side::Gm
            )
            .as_deref(),
            Some("（旁白）夜深了")
        );
        assert_eq!(
            lane_event_line(
                &event(TranscriptKind::System, "", "", "擲骰 3"),
                "zh-TW",
                Side::Gm
            )
            .as_deref(),
            Some("（系統）擲骰 3")
        );
    }

    fn marked(marker: data::EventMarker, text: &str, gm_only: bool) -> TranscriptEvent {
        TranscriptEvent {
            gm_only,
            marker: Some(marker),
            ..event(TranscriptKind::System, "", "GM", text)
        }
    }

    /// visibility 洩漏修正（包 4b）：gm_only 事件在 chars 線只留標頭；GM 線與非 gm_only 事件
    /// 一律全文；沒有玩家名的發言退回該語系稱呼。
    #[test]
    fn lane_event_line_redacts_gm_only_text_for_chars_lane_only() {
        let secret = marked(
            data::EventMarker::PersonArrival {
                title: "密探".to_owned(),
            },
            "只有 GM 知道的全文。",
            true,
        );
        assert_eq!(
            lane_event_line(&secret, "zh-TW", Side::Character).as_deref(),
            Some("（系統）（人物登場）〈密探〉")
        );
        assert_eq!(
            lane_event_line(&secret, "zh-TW", Side::Gm).as_deref(),
            Some("（系統）（人物登場）〈密探〉\n只有 GM 知道的全文。")
        );

        let public = event(TranscriptKind::System, "", "GM", "擲骰 3");
        assert_eq!(
            lane_event_line(&public, "zh-TW", Side::Character).as_deref(),
            Some("（系統）擲骰 3")
        );
        let nameless = event(TranscriptKind::Player, "", "", "你好");
        assert_eq!(
            lane_event_line(&nameless, "ko", Side::Character).as_deref(),
            Some("플레이어: 你好")
        );
    }

    /// 角色私設：角色側 keyword 觸發看不到、也不佔最近四則；GM 側照樣觸發。換幕摘要只讀角色側
    /// 內容，回歸標頭只出現一次。
    #[test]
    fn character_side_keyword_and_summary_ignore_card_private() {
        let fox = card("fox-id", "狐狸", "尾巴很大。", "身上藏著龍鱗。");
        let (marker, text) = card_private(&fox, "阿濤").unwrap();
        let private = marked(marker, &text, true);
        let knight = card("knight-id", "騎士", "王國騎士", "");
        let (marker, text) = card_arrival(&knight, "阿濤");
        let arrival = marked(marker, &text, false);
        let entries = [worldbook_entry(
            1,
            "龍鱗傳說",
            &["龍鱗"],
            false,
            0,
            false,
            Visibility::Public,
        )];
        let events = [
            event(TranscriptKind::Player, "", "阿濤", "聽說有寶箱"),
            event(TranscriptKind::Narration, "", "GM", "夜深了"),
            arrival,
            private,
            event(TranscriptKind::Narration, "", "GM", "風起"),
        ];
        let turn = chars_lane_turn(
            &fox,
            None,
            &events,
            &entries,
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
            false,
        );
        assert!(!turn.tail.contains("龍鱗傳說"));
        let gm_turn = gm_lane_turn(
            &events,
            &entries,
            None,
            &TableState::default(),
            &Mechanism::default(),
            &StateScope::default(),
            "導演指示",
            "zh-TW",
        );
        assert!(gm_turn.tail.contains("龍鱗傳說"));

        let joined: String = summary_messages(&events, "zh-TW")
            .iter()
            .map(|message| message.content.as_str())
            .collect();
        assert_eq!(
            joined
                .matches("（系統）（角色回歸）〈騎士〉\n公開設定：\n王國騎士")
                .count(),
            1
        );
        assert!(!joined.contains("龍鱗"));
        assert!(!joined.contains("角色私設"));
    }
}
