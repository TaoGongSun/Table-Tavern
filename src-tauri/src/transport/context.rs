use crate::data::{CharacterCard, Mechanism, TranscriptEvent, WorldbookEntry};

use super::messages::{language_rule, replace_st_macros};



pub fn active_worldbook_entries<'a>(
    entries: &'a [WorldbookEntry],
    events: &[TranscriptEvent],
) -> Vec<&'a WorldbookEntry> {
    let recent_text: Vec<String> = events
        .iter()
        .rev()
        .take(4)
        .map(|event| event.text.to_lowercase())
        .collect();
    let mut active: Vec<_> = entries
        .iter()
        .filter(|entry| {
            !entry.disabled
                && (entry.constant
                    || (!entry.keys.is_empty()
                        && entry.keys.iter().any(|key| {
                            let key = key.to_lowercase();
                            !key.is_empty() && recent_text.iter().any(|text| text.contains(&key))
                        })))
        })
        .collect();
    active.sort_by_key(|entry| (entry.order, entry.uid));
    active
}

/// GM 的 system prompt 本體：GM 指示＋world.md＋constant 條目＋全卡（含私設）＋玩家卡。
/// assemble_gm_messages（單發）與 gm_lane_system（resume 續聊凍結快照）共用。
/// constant 條目裡的 is_person 條目改走名冊行，不進全文（包 4a，見 split_person_roster）。
pub(super) fn gm_system_prompt(
    world_md: &str,
    cards: &[CharacterCard],
    player: Option<&CharacterCard>,
    constant_entries: &[&WorldbookEntry],
    user_name: &str,
    mechanism: &Mechanism,
    lang: &str,
) -> String {
    let mut system = format!(
        "你是這場多人桌上角色扮演的 GM（導演兼旁白）。你負責描述場景與世界反應、\
         推進劇情節奏、決定下一位發言者，並防止對話停滯或重複。\
         旁白是所有人都聽得到的公開敘事。沒有角色卡的配角（路人、店主、反派等）\
         由你全權扮演，可自由出場、說話、行動；「登場角色」名單上的角色與玩家\
         各有扮演者，不要替他們代言。\
         世界設定與角色私有設定只有你知道全貌，劇情尚未揭露的內容不要說破。\
         {language_rule}\n",
        language_rule = language_rule(lang),
    );
    if !world_md.trim().is_empty() {
        system.push_str(&format!(
            "\n## 世界設定（只進你的上下文，角色只知道你說出口的內容）\n{}\n",
            replace_st_macros(world_md.trim(), user_name, None)
        ));
    }
    let (constant_entries, roster) = split_person_roster(constant_entries);
    if !constant_entries.is_empty() || roster.is_some() {
        system.push_str("\n## 世界書（只進你的上下文）\n");
        for entry in constant_entries {
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
    if !cards.is_empty() {
        system.push_str("\n## 登場角色\n");
        for card in cards {
            system.push_str(&format!("### {}\n", card.name));
            if !card.public_md.trim().is_empty() {
                system.push_str(&format!(
                    "公開設定：\n{}\n",
                    replace_st_macros(card.public_md.trim(), user_name, Some(&card.name))
                ));
            }
            if !card.private_md.trim().is_empty() {
                system.push_str(&format!(
                    "私有設定（僅你與該角色知道）：\n{}\n",
                    replace_st_macros(card.private_md.trim(), user_name, Some(&card.name))
                ));
            }
        }
    }
    if let Some(player) = player {
        system.push_str(&format!(
            "\n## 玩家角色（真人扮演，逐字稿裡的「{}」就是他）",
            player.name
        ));
        if !player.public_md.trim().is_empty() {
            system.push_str(&format!(
                "\n{}\n",
                replace_st_macros(player.public_md.trim(), user_name, Some(&player.name))
            ));
        }
    }
    if mechanism.incremental {
        system.push('\n');
        system.push_str(mechanism_protocol(lang));
        // 卡專屬規矩接在通用協定後面：這張卡哪些欄位每回合必報、哪些只在變動時報，
        // 由重構照卡原文產出，通用協定的「只寫變動的欄位」到這裡以卡的規定為準。
        // 介面歸屬聲明壓在最後——卡原文往往要求模型每回合重印整包狀態才畫得出畫面，
        // 這裡畫面由 App 組，不講清楚模型會照卡的老規矩再印一份；而且要壓在欄位說明
        // 之後，否則模型會照說明的 markdown 排版把狀態寫進正文（兩者實測都踩過）。
        if !mechanism.guide.trim().is_empty() {
            system.push_str("\n\n");
            system.push_str(mechanism.guide.trim());
            system.push_str("\n\n");
            system.push_str(interface_owned_notice(lang));
        }
    }
    system
}

/// 增量桌統一協定聲明：數值由系統本地記帳，模型只回報這一幕的變動量。
/// 原文照貼進凍結快照，不要改寫措辭——改一字整條快取全滅。
fn mechanism_protocol(lang: &str) -> &'static str {
    if lang == "en" {
        "## State Update Protocol v1 (this table's numbers are system-managed)\n\n\
         Numbers are computed and tracked locally by the system — you only need to say \
         \"how much changed this scene.\" End every reply with an update block:\n\n\
         <UpdateVariable>\n\
         <JSONPatch>\n\
         [\n\
         \x20 { \"op\": \"delta\", \"path\": \"/Heroes/Arthur/Affection\", \"value\": 5 },\n\
         \x20 { \"op\": \"replace\", \"path\": \"/World/Location\", \"value\": \"Dawnport Docks\" }\n\
         ]\n\
         </JSONPatch>\n\
         </UpdateVariable>\n\n\
         - Paths are `/`-separated; only include fields that actually changed this scene.\n\
         - Number fields always use delta for the change amount (e.g. 5, -10), never an \
         absolute value — absolute values will be rejected.\n\
         - \"current/max\" fields (e.g. \"480/500\"): delta moves the current value; only use \
         replace (e.g. \"480/600\") when the max changes (a level-up).\n\
         - Text fields use replace with the new value; use insert to add, remove to delete, \
         move to relocate.\n\
         - Dice fields are rolled locally by the system each turn — you only read them, never \
         write them.\n\
         - Bounds and rejections are enforced by the system; rejected fields will tell you the \
         current value next turn."
    } else {
        "## 狀態更新協定 v1（這桌的數值由系統保管）\n\n\
         數值由系統本地計算與記帳，你只要說「這一幕變動了多少」。每次回覆的最後附上更新區塊：\n\n\
         <UpdateVariable>\n\
         <JSONPatch>\n\
         [\n\
         \x20 { \"op\": \"delta\", \"path\": \"/Heroes/亞瑟/Affection\", \"value\": 5 },\n\
         \x20 { \"op\": \"replace\", \"path\": \"/World/Location\", \"value\": \"晨港碼頭\" }\n\
         ]\n\
         </JSONPatch>\n\
         </UpdateVariable>\n\n\
         - path 用 `/` 分層，只寫這一幕真的變動的欄位，沒變的不要寫。\n\
         - 數字欄一律用 delta 給增減量（例 5、-10），不要給絕對值——給絕對值會被系統擋下。\n\
         - 「現值/上限」欄（例 \"480/500\"）：delta 動現值；只有上限改變（升級）才用 replace 寫成 \"480/600\"。\n\
         - 文字欄用 replace 寫新值；新增項目用 insert、刪除用 remove、搬移用 move。\n\
         - 骰值欄由系統每回合擲，你只讀不寫。\n\
         - 上下限與拒收由系統把關，被擋下的欄位會在下一輪告訴你目前值。"
    }
}

/// 介面歸屬聲明：只有介面被 App 接管的桌（mechanism.guide 非空）才附，而且排在欄位說明之後——
/// 模型會模仿最後讀到的排版，欄位說明擺最後它就照那份說明的 markdown 逐條寫在正文裡（實測踩過）。
/// 與 mechanism_protocol 一樣是凍結快照的一部分，措辭不要隨手改。
fn interface_owned_notice(lang: &str) -> &'static str {
    if lang == "en" {
        "## Who draws the interface\n\n\
         The field spec above is a specification of what each value looks like — its layout is not \
         your output format.\n\
         This table's game interface (status panels, maps, lists) is rendered by the app from the \
         state tree it keeps. If the card's own rules tell you to reprint a whole interface block \
         every turn (a multi-module `<...UI>` dump), do not — the app draws that itself.\n\
         Your reply is exactly two things: this turn's story, then the `<UpdateVariable>` update \
         block. That block is bookkeeping for the system, not interface output — every state value \
         goes inside it as JSONPatch. Never report state as prose, lists, or bold headings in the \
         story body."
    } else {
        "## 介面由誰畫\n\n\
         上面的欄位說明是規格書，講的是每個欄位的值長什麼樣；**它的排版不是你的輸出格式**。\n\
         這桌的遊戲介面（狀態面板、地圖、清單）由 App 拿系統帳上的狀態樹組裝並渲染。卡原文若要求\
         你每回合重印整包介面（`<...UI>` 那種多模块資料塊），一律不要照做——那些 App 自己會畫。\n\
         你的回覆就兩樣東西：這一回合的劇情正文，然後是 `<UpdateVariable>` 更新區塊。那個區塊不是\
         介面輸出、是給系統記帳用的，**每一個狀態值都寫在裡面**（照上面協定的 JSONPatch 寫法）。\
         不要改用條列、粗體標題或任何其他形式把狀態寫在劇情正文裡。"
    }
}

// ---------------------------------------------------------------------
// AI 卡重構包 4a：世界書人物條目在場過濾。人物條目全部常駐 system 會吃爆快取
// 前綴，改成 system 只留一行名冊，某人首次在場才把全文 append 進歷史當系統事件
// （進場付一次全文，之後吃快取價；離場不拔——拔會改動已快取的 system 前綴）。
// ---------------------------------------------------------------------

/// 世界書條目分流：`is_person && !disabled` 的不進 system 全文，只收 title 湊一行名冊；
/// `is_person && disabled` 兩邊都不進（呼叫端本來就會先濾掉，這裡防禦性地跟著蓋掉，
/// 不能讓停用的人物條目落回全文那邊）；沒有人物條目時 roster 是 `None`——呼叫端據此
/// 完全不印這一行，既有輸出逐字不變。
pub(super) fn split_person_roster<'a>(
    entries: &[&'a WorldbookEntry],
) -> (Vec<&'a WorldbookEntry>, Option<String>) {
    let mut rest = Vec::new();
    let mut names = Vec::new();
    for entry in entries {
        if !entry.is_person {
            rest.push(*entry);
        } else if !entry.disabled {
            names.push(entry.title.as_str());
        }
    }
    let roster = (!names.is_empty()).then(|| format!("這桌還有這些人：{}", names.join("、")));
    (rest, roster)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::data::{self, AppConfig, CharacterCard, DataResult, FieldKind, FieldRule, InjectLevel, Mechanism, StateNode, TableState, Tier, TranscriptEvent, TranscriptKind, Visibility, WorldbookEntry};
    #[allow(unused_imports)]
    use crate::mechanism;
    #[allow(unused_imports)]
    use std::collections::{BTreeMap, BTreeSet};
    #[allow(unused_imports)]
    use super::super::test_support::{card, event, worldbook_entry};
    #[allow(unused_imports)]
    use super::super::messages::*;
    #[allow(unused_imports)]
    use super::super::assemble::*;
    #[allow(unused_imports)]
    use super::super::state_view::*;
    #[allow(unused_imports)]
    use super::super::arrivals::*;
    #[allow(unused_imports)]
    use super::super::turns::*;
    #[allow(unused_imports)]
    use super::super::response::*;
    #[allow(unused_imports)]
    use super::super::client::*;

    #[test]
    fn active_worldbook_entries_use_constant_recent_four_keys_and_sorting() {
        let entries = [
            worldbook_entry(4, "常駐", &[], true, 20, false, Visibility::Gm),
            worldbook_entry(1, "同序先排", &[], true, 20, false, Visibility::Gm),
            worldbook_entry(3, "近期", &["DrAgOn"], false, 10, false, Visibility::Gm),
            worldbook_entry(2, "太舊", &["ancient"], false, 0, false, Visibility::Gm),
            worldbook_entry(1, "停用", &[], true, -10, true, Visibility::Gm),
            worldbook_entry(0, "空關鍵字", &[], false, -20, false, Visibility::Gm),
        ];
        let events = [
            event(TranscriptKind::Narration, "", "GM", "ancient history"),
            event(TranscriptKind::Player, "", "玩家", "one"),
            event(TranscriptKind::Dialogue, "fox-id", "狐狸", "two"),
            event(TranscriptKind::Narration, "", "GM", "A DRAGON wakes"),
            event(TranscriptKind::Player, "", "玩家", "four"),
        ];

        let active = active_worldbook_entries(&entries, &events);
        assert_eq!(
            active.iter().map(|entry| entry.uid).collect::<Vec<_>>(),
            [3, 1, 4]
        );
    }

    // ---- AI 卡重構包 4a：世界書人物條目在場過濾 ----

    /// 規格 (a)：沒有 is_person 條目時，三處 system 組裝都不能多出名冊行——
    /// 既有桌（沒有人物條目）的輸出必須逐字不變。
    #[test]
    fn person_roster_absent_without_is_person_entries() {
        let entries = [
            worldbook_entry(1, "公開常識", &[], true, 0, false, Visibility::Public),
            worldbook_entry(2, "GM專有", &[], true, 1, false, Visibility::Gm),
        ];
        let gm_system = &assemble_gm_messages(
            "世界總覽",
            &[],
            None,
            &[],
            &entries,
            &TableState::default(),
            &Mechanism::default(),
            &StateScope::default(),
            "zh-TW",
        )[0]
        .content;
        assert!(!gm_system.contains("這桌還有這些人"));

        let lane_system =
            gm_lane_system("世界總覽", &[], None, &entries, &Mechanism::default(), "zh-TW");
        assert!(!lane_system.contains("這桌還有這些人"));

        let chars_system = chars_lane_system(&[], None, &entries, "zh-TW");
        assert!(!chars_system.contains("這桌還有這些人"));
    }

    /// 規格 (b)：is_person 條目不進 system 全文，改收進名冊行；disabled 的人物條目不列進名冊。
    #[test]
    fn person_entries_become_roster_line_not_full_text() {
        let normal = worldbook_entry(1, "小鎮傳說", &[], true, 0, false, Visibility::Public);
        let alice = WorldbookEntry {
            is_person: true,
            ..worldbook_entry(2, "愛麗絲", &[], true, 1, false, Visibility::Public)
        };
        let bob = WorldbookEntry {
            is_person: true,
            ..worldbook_entry(3, "鮑伯", &[], true, 2, false, Visibility::Public)
        };
        let disabled_person = WorldbookEntry {
            is_person: true,
            ..worldbook_entry(4, "已停用的人", &[], true, 3, true, Visibility::Public)
        };
        let entries = [normal, alice, bob, disabled_person];

        let gm_system = &assemble_gm_messages(
            "",
            &[],
            None,
            &[],
            &entries,
            &TableState::default(),
            &Mechanism::default(),
            &StateScope::default(),
            "zh-TW",
        )[0]
        .content;
        assert!(gm_system.contains("小鎮傳說內容"));
        assert!(!gm_system.contains("愛麗絲內容"));
        assert!(!gm_system.contains("鮑伯內容"));
        assert!(!gm_system.contains("已停用的人"));
        assert!(gm_system.contains("這桌還有這些人：愛麗絲、鮑伯"));

        let chars_system = chars_lane_system(&[], None, &entries, "zh-TW");
        assert!(chars_system.contains("小鎮傳說內容"));
        assert!(!chars_system.contains("愛麗絲內容"));
        assert!(chars_system.contains("這桌還有這些人：愛麗絲、鮑伯"));
    }

    /// split_person_roster 自己也擋 disabled——縱使目前三個呼叫端都已經先濾過一次，
    /// helper 本身仍要對規格「is_person && !disabled」單獨成立，不依賴呼叫端不出錯。
    #[test]
    fn split_person_roster_excludes_disabled_person_entries_defensively() {
        let alice = WorldbookEntry {
            is_person: true,
            ..worldbook_entry(1, "愛麗絲", &[], true, 0, false, Visibility::Public)
        };
        let disabled = WorldbookEntry {
            is_person: true,
            ..worldbook_entry(2, "隱藏人物", &[], true, 1, true, Visibility::Public)
        };
        let refs: Vec<&WorldbookEntry> = vec![&alice, &disabled];
        let (rest, roster) = split_person_roster(&refs);
        assert!(rest.is_empty());
        assert_eq!(roster, Some("這桌還有這些人：愛麗絲".to_owned()));
    }

    /// 邊界情況：constant 條目清一色是人物時，非人物清單濾完是空的，但名冊行仍要印出來
    /// （標頭不能因為「沒有全文條目」就整段消失）。
    #[test]
    fn person_roster_line_appears_even_when_all_constant_entries_are_people() {
        let alice = WorldbookEntry {
            is_person: true,
            ..worldbook_entry(1, "愛麗絲", &[], true, 0, false, Visibility::Gm)
        };
        let entries = [alice];
        let gm_system = &assemble_gm_messages(
            "",
            &[],
            None,
            &[],
            &entries,
            &TableState::default(),
            &Mechanism::default(),
            &StateScope::default(),
            "zh-TW",
        )[0]
        .content;
        assert!(gm_system.contains("\n## 世界書（只進你的上下文）\n這桌還有這些人：愛麗絲\n"));
    }

}
