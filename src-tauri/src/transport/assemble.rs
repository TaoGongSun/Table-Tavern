use crate::data::{CharacterCard, Mechanism, TableState, TranscriptEvent, TranscriptKind, WorldbookEntry};

use super::messages::{ChatMessage, message, player_fallback_name, push_merged};

use super::context::{active_worldbook_entries, gm_system_prompt};

use super::state_view::{StateScope, gm_dynamic_block};

use super::turns::{chars_lane_system, chars_lane_turn, system_event_text};



/// 共線組裝（api-shared-lane 包 B）：claude 以外的四條路共用一份**與「這輪是誰」無關**的前綴。
/// system 走 `chars_lane_system`（扮演引擎前言＋全部公開角色卡），歷史裡所有角色台詞一律
/// `assistant` 並帶「名字：」前綴，本輪指定與私設放尾端一則 `user`。前綴因此逐字穩定，
/// 換角色不再整包重算——實測前三條路（api 底線 64、codex 底線 9,984、grok）都是「換角色全滅、
/// 同角色連續全中」，共線把「全滅」那一半救回來。
///
/// role 序列只跟事件種類有關、與角色無關，`push_merged` 的分組因此也與角色無關；
/// CLI 三條攤平後的 `(system, prompt)` 連帶逐字相同（見 `cli::flatten_messages`）。
///
/// 單角色桌（`cards` 只有一張）把**私設**提回 system：只有一個角色不會洩漏，而 system 的
/// 指令權重高於尾端 user。這是唯一的分支，不另立第二條組裝路徑。提上去的只有 `private_md`
/// 這段穩定內容——限定世界書的 keyword 條目與狀態每輪翻動，進 system 會把前綴打散。
///
/// API 無狀態，上一輪注入的私設不會出現在下一輪的 messages 裡，
/// 不需要 claude lane 那套「回合後從 session 檔抹掉」。
#[allow(clippy::too_many_arguments)]
pub fn assemble_shared_messages(
    card: &CharacterCard,
    cards: &[CharacterCard],
    player: Option<&CharacterCard>,
    events: &[TranscriptEvent],
    worldbook: &[WorldbookEntry],
    state: &TableState,
    mechanism: &Mechanism,
    branch: Option<&[String]>,
    lang: &str,
) -> Vec<ChatMessage> {
    let mut system = chars_lane_system(cards, player, worldbook, lang);
    let turn = chars_lane_turn(
        card,
        player,
        events,
        worldbook,
        state,
        mechanism,
        branch,
        lang,
        cards.len() <= 1,
    );
    if let Some(private) = &turn.hoisted_private {
        system.push('\n');
        system.push_str(private);
    }
    let tail = turn.tail;

    let mut messages = vec![message("system", system)];
    for event in events {
        // 台詞一律 assistant＋名字前綴：對白對誰都是同一則，前綴才穩得住
        let (role, line) = match event.kind {
            TranscriptKind::Dialogue => (
                "assistant",
                format!("{}：{}", event.speaker_name, event.text),
            ),
            TranscriptKind::Player => ("user", format!("{}：{}", event.speaker_name, event.text)),
            TranscriptKind::Narration => ("user", format!("（旁白）{}", event.text)),
            TranscriptKind::System => {
                ("user", format!("（系統）{}", system_event_text(event, true)))
            }
        };
        push_merged(&mut messages, role, line);
    }
    // 刻意不走 push_merged：本輪指定維持獨立一則，不黏進歷史
    messages.push(message("user", tail.trim_end().to_owned()));
    messages
}

/// 點名時「輪到玩家」的內部代號；前端以它停下 GM 推進回合。
/// 刻意用不可能當人名的字串：玩家卡或某張 NPC 卡都可能就叫「玩家」。
pub const PLAYER_SENTINEL: &str = "__PLAYER__";

/// 組裝 GM 上下文：world.md（只有 GM 看得到）＋全部角色卡（含私有，NewPlan §7.0）
/// ＋公開 transcript。GM 自己的旁白是 assistant，其餘事件是 user。
/// keyword 條目與「目前狀態」放 transcript 尾端獨立 user 訊息（快取友善），
/// constant 條目與角色卡留在 system（穩定且需要高遵循度）。
#[allow(clippy::too_many_arguments)]
pub fn assemble_gm_messages(
    world_md: &str,
    cards: &[CharacterCard],
    player: Option<&CharacterCard>,
    events: &[TranscriptEvent],
    worldbook: &[WorldbookEntry],
    state: &TableState,
    mechanism: &Mechanism,
    scope: &StateScope,
    lang: &str,
) -> Vec<ChatMessage> {
    let user_name = player
        .map(|player| player.name.as_str())
        .unwrap_or_else(|| player_fallback_name(lang));
    // 快取友善（prompt-cache-optimization A）：keyword 條目與「目前狀態」每輪翻動，
    // 移到 transcript 尾端的一則獨立 user 訊息；constant 條目穩定，留在 system。
    let (constant_entries, keyword_entries): (Vec<_>, Vec<_>) =
        active_worldbook_entries(worldbook, events)
            .into_iter()
            .partition(|entry| entry.constant);
    let system = gm_system_prompt(
        world_md,
        cards,
        player,
        &constant_entries,
        user_name,
        mechanism,
        lang,
    );

    let mut messages = vec![message("system", system)];
    for event in events {
        let (role, line) = match event.kind {
            TranscriptKind::Narration => ("assistant", event.text.clone()),
            TranscriptKind::Dialogue | TranscriptKind::Player => {
                ("user", format!("{}：{}", event.speaker_name, event.text))
            }
            TranscriptKind::System => ("user", format!("（系統）{}", event.text)),
        };
        push_merged(&mut messages, role, line);
    }
    let dynamic = gm_dynamic_block(&keyword_entries, state, user_name, mechanism, scope, lang);
    if !dynamic.is_empty() {
        // 刻意不走 push_merged：動態塊維持獨立一則的語意邊界，不黏進最後一則發言
        messages.push(message("user", dynamic));
    }
    messages
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
    use super::super::context::*;
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

    /// `{{user}}` 沒有玩家卡時退回語系預設名（共線的 system 一樣要做這個代換）。
    #[test]
    fn shared_lane_st_macros_fall_back_to_localized_player_name() {
        let fox = card("fox-id", "狐狸", "{{user}}", "");
        let cards = vec![fox.clone()];
        let build = |lang: &str| {
            assemble_shared_messages(
                &fox,
                &cards,
                None,
                &[],
                &[],
                &TableState::default(),
                &Mechanism::default(),
                None,
                lang,
            )[0]
                .content
                .clone()
        };
        assert!(build("zh-TW").contains("### 狐狸\n玩家\n"));
        assert!(build("en").contains("### 狐狸\nPlayer\n"));
    }

    /// 共線後 system 含全部角色的**公開**設定（那正是共用前綴），
    /// 但私設只有本輪那位的、且在尾端；world.md 一如既往碰不到。
    /// 逐字稿的三種前綴（旁白／玩家／系統）格式不隨共線改變。
    #[test]
    fn shared_lane_shares_public_cards_but_not_others_private() {
        let fox = card("fox-id", "狐狸", "旅店老闆，笑口常開", "其實是通緝犯");
        let knight = card("knight-id", "騎士", "王國騎士", "奉密令而來");
        let cards = vec![fox.clone(), knight.clone()];
        let events = [
            event(TranscriptKind::Narration, "", "GM", "夜幕低垂"),
            event(TranscriptKind::Player, "", "玩家", "老闆，來杯麥酒"),
            event(TranscriptKind::Dialogue, "fox-id", "狐狸", "馬上來！"),
            event(
                TranscriptKind::Dialogue,
                "knight-id",
                "騎士",
                "我在找一名通緝犯。",
            ),
            event(TranscriptKind::System, "", "系統", "騎士 加入本桌"),
        ];
        let messages = assemble_shared_messages(
            &fox,
            &cards,
            None,
            &events,
            &[],
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
        );

        let system = &messages[0];
        assert_eq!(system.role, "system");
        assert!(system.content.contains("旅店老闆")); // 自己的公開
        assert!(system.content.contains("王國騎士")); // 別人的公開也在共用前綴裡
        assert!(!system.content.contains("其實是通緝犯")); // 私設不進共用前綴
        assert!(!system.content.contains("奉密令而來"));

        let joined: String = messages.iter().map(|m| m.content.as_str()).collect();
        assert!(!joined.contains("world"));
        assert!(joined.contains("（旁白）夜幕低垂"));
        assert!(joined.contains("玩家：老闆，來杯麥酒"));
        assert!(joined.contains("騎士：我在找一名通緝犯。"));
        assert!(joined.contains("（系統）騎士 加入本桌"));
        // 本輪那位的私設在尾端，別人的哪裡都沒有
        assert!(messages.last().unwrap().content.contains("其實是通緝犯"));
        assert!(!joined.contains("奉密令而來"));
    }

    /// 共線的 role 規則：**所有**角色台詞都是 assistant（不再分是不是自己說的），
    /// 相鄰同 role 仍合併。role 序列因此只跟事件種類有關，與「這輪是誰」無關。
    #[test]
    fn shared_lane_makes_every_dialogue_assistant_and_merges_adjacent() {
        let fox = card("fox-id", "狐狸", "公開", "");
        let cards = vec![fox.clone(), card("owl-id", "貓頭鷹", "公開", "")];
        let events = [
            event(TranscriptKind::Player, "", "玩家", "第一句"),
            event(TranscriptKind::Narration, "", "GM", "旁白一句"),
            event(TranscriptKind::Dialogue, "fox-id", "狐狸", "我的回答"),
            event(TranscriptKind::Dialogue, "owl-id", "貓頭鷹", "牠的回答"),
            event(TranscriptKind::Player, "", "玩家", "第二句"),
        ];
        let messages = assemble_shared_messages(
            &fox,
            &cards,
            None,
            &events,
            &[],
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
        );
        let roles: Vec<&str> = messages.iter().map(|m| m.role.as_str()).collect();
        assert_eq!(roles, ["system", "user", "assistant", "user", "user"]);
        assert_eq!(messages[1].content, "玩家：第一句\n（旁白）旁白一句");
        // 兩位不同角色的台詞併成同一則 assistant，各自帶名字
        assert_eq!(messages[2].content, "狐狸：我的回答\n貓頭鷹：牠的回答");
        assert_eq!(messages[3].content, "玩家：第二句");
        // 空的私有節不產生私有段落
        assert!(!messages.last().unwrap().content.contains("私有設定"));
    }

    /// (a) 的回歸測試：單角色桌提進 system 的只能是**穩定**的私設。限定世界書的 keyword
    /// 條目隨最近事件翻動、狀態每輪變，那兩樣若跟著上去，system 每輪都不同，
    /// 前綴照樣被打散——等於在主流玩法（一桌一張）上親手製造共線要修的那個病。
    #[test]
    fn shared_lane_single_character_system_survives_state_and_keyword_churn() {
        let solo = card("solo", "獨角", "獨角公開", "獨角私設");
        let cards = vec![solo.clone()];
        let worldbook = vec![worldbook_entry(
            1,
            "密室",
            &["密室"],
            false, // keyword：靠最近事件觸發，回合之間會進出
            0,
            false,
            Visibility::Characters(vec!["solo".to_owned()]),
        )];
        let build = |events: &[TranscriptEvent], state: &TableState| {
            assemble_shared_messages(
                &solo,
                &cards,
                None,
                events,
                &worldbook,
                state,
                &Mechanism::default(),
                Some(&["獨角".to_owned()]),
                "zh-TW",
            )[0]
                .content
                .clone()
        };
        let quiet = [event(TranscriptKind::Player, "p", "玩家", "我走在路上。")];
        let triggered = [event(TranscriptKind::Player, "p", "玩家", "我走進密室。")];
        let later = TableState {
            tree: std::collections::BTreeMap::from([(
                "獨角".to_owned(),
                StateNode::Branch(std::collections::BTreeMap::from([(
                    "體力".to_owned(),
                    StateNode::Leaf("3".to_owned()),
                )])),
            )]),
            ..TableState::default()
        };

        let baseline = build(&quiet, &TableState::default());
        assert!(baseline.contains("獨角私設")); // 穩定私設要在 system
        assert!(!baseline.contains("密室內容")); // keyword 條目不准上去
        // keyword 被觸發、狀態變動，system 都必須一字不差
        assert_eq!(build(&triggered, &TableState::default()), baseline);
        assert_eq!(build(&quiet, &later), baseline);
        assert_eq!(build(&triggered, &later), baseline);

        // 但資料不能因此消失：keyword 與狀態要確實送到尾端，
        // 否則「system 穩定」只是因為整包被丟掉，這條測試會白白放行
        let tail = |events: &[TranscriptEvent], state: &TableState| {
            assemble_shared_messages(
                &solo,
                &cards,
                None,
                events,
                &worldbook,
                state,
                &Mechanism::default(),
                Some(&["獨角".to_owned()]),
                "zh-TW",
            )
            .last()
            .unwrap()
            .content
            .clone()
        };
        assert!(tail(&triggered, &TableState::default()).contains("密室內容"));
        assert!(!tail(&quiet, &TableState::default()).contains("密室內容"));
        assert!(tail(&quiet, &later).contains("體力：3"));
        assert!(!tail(&quiet, &TableState::default()).contains("體力"));
    }

    /// 兩位角色各自帶不同的 branch 與角色限定條目時，共用前綴仍要逐字相同，
    /// 且誰也看不到對方的限定情報。
    #[test]
    fn shared_lane_prefix_holds_with_per_character_branches_and_limited_entries() {
        let gal = card("gal", "加爾", "加爾公開", "加爾私設");
        let ray = card("ray", "雷恩", "雷恩公開", "雷恩私設");
        let cards = vec![gal.clone(), ray.clone()];
        let worldbook = vec![
            worldbook_entry(
                1,
                "加爾的舊傷",
                &[],
                true, // constant，但限定可見：仍走回合注入，不進共用前綴
                0,
                false,
                Visibility::Characters(vec!["gal".to_owned()]),
            ),
            worldbook_entry(
                2,
                "雷恩的密令",
                &["密令"],
                false,
                1,
                false,
                Visibility::Characters(vec!["ray".to_owned()]),
            ),
        ];
        let events = [
            event(TranscriptKind::Player, "p", "玩家", "有人提到密令。"),
            event(TranscriptKind::Dialogue, "gal", "加爾", "加爾皺眉。"),
        ];
        // 兩人各有一條真的狀態分支，branch 才會真的產出內容（只傳不同名字而 state 是空的，
        // character_state_block 會回 None，等於這條測試沒驗到 branch）
        let state = TableState {
            tree: std::collections::BTreeMap::from([(
                "Heroes".to_owned(),
                StateNode::Branch(std::collections::BTreeMap::from([
                    (
                        "加爾".to_owned(),
                        StateNode::Branch(std::collections::BTreeMap::from([(
                            "體力".to_owned(),
                            StateNode::Leaf("加爾八成".to_owned()),
                        )])),
                    ),
                    (
                        "雷恩".to_owned(),
                        StateNode::Branch(std::collections::BTreeMap::from([(
                            "體力".to_owned(),
                            StateNode::Leaf("雷恩五成".to_owned()),
                        )])),
                    ),
                ])),
            )]),
            ..TableState::default()
        };
        let build = |card: &CharacterCard, branch: &[String]| {
            assemble_shared_messages(
                card,
                &cards,
                None,
                &events,
                &worldbook,
                &state,
                &Mechanism::default(),
                Some(branch),
                "zh-TW",
            )
        };
        let for_gal = build(&gal, &["Heroes".to_owned(), "加爾".to_owned()]);
        let for_ray = build(&ray, &["Heroes".to_owned(), "雷恩".to_owned()]);
        assert_eq!(for_gal.len(), for_ray.len());
        for (index, (left, right)) in for_gal
            .iter()
            .zip(for_ray.iter())
            .take(for_gal.len() - 1)
            .enumerate()
        {
            assert_eq!(left.role, right.role);
            assert_eq!(left.content, right.content, "第 {index} 則不該隨角色改變");
        }
        // 限定條目只出現在本人的尾端，共用前綴與對方都碰不到
        let gal_prefix: String = for_gal[..for_gal.len() - 1]
            .iter()
            .map(|message| message.content.as_str())
            .collect();
        assert!(!gal_prefix.contains("加爾的舊傷內容"));
        assert!(for_gal.last().unwrap().content.contains("加爾的舊傷內容"));
        assert!(!for_gal.last().unwrap().content.contains("雷恩的密令內容"));
        assert!(for_ray.last().unwrap().content.contains("雷恩的密令內容"));
        assert!(!for_ray.last().unwrap().content.contains("加爾的舊傷內容"));
        // 狀態同理：各自只看得到自己那條分支，共用前綴一律碰不到
        assert!(for_gal.last().unwrap().content.contains("加爾八成"));
        assert!(!for_gal.last().unwrap().content.contains("雷恩五成"));
        assert!(for_ray.last().unwrap().content.contains("雷恩五成"));
        assert!(!for_ray.last().unwrap().content.contains("加爾八成"));
        assert!(!gal_prefix.contains("加爾八成"));
    }

    /// api-shared-lane 包 B 的核心保證：換角色只動最後一則，前面每一則逐字不變。
    /// 這條一破，前綴快取就從分岔點整段失效——那正是共線要修的病。
    #[test]
    fn shared_lane_prefix_is_identical_across_characters() {
        let gal = card("gal", "加爾", "加爾公開", "加爾私設");
        let ray = card("ray", "雷恩", "雷恩公開", "雷恩私設");
        let bran = card("bran", "布蘭德", "布蘭德公開", "");
        let cards = vec![gal.clone(), ray.clone(), bran.clone()];
        let player = card("p", "玩家", "玩家公開", "");
        let events = vec![
            event(TranscriptKind::Player, "p", "玩家", "我推開門。"),
            event(TranscriptKind::Dialogue, "gal", "加爾", "加爾抬起頭。"),
            event(TranscriptKind::Dialogue, "ray", "雷恩", "雷恩笑了。"),
            event(TranscriptKind::Narration, "gm", "GM", "燈光暗下。"),
            event(TranscriptKind::Dialogue, "bran", "布蘭德", "布蘭德聳肩。"),
            event(TranscriptKind::Dialogue, "gal", "加爾", "加爾又說了一句。"),
        ];
        let build = |card: &CharacterCard| {
            assemble_shared_messages(
                card,
                &cards,
                Some(&player),
                &events,
                &[],
                &TableState::default(),
                &Mechanism::default(),
                None,
                "zh-TW",
            )
        };
        let for_gal = build(&gal);
        let for_ray = build(&ray);
        assert_eq!(for_gal.len(), for_ray.len());
        for (index, (left, right)) in for_gal.iter().zip(for_ray.iter()).enumerate() {
            match index + 1 == for_gal.len() {
                // 最後一則是本輪指定＋私設，本來就該不同
                true => assert_ne!(left.content, right.content),
                false => {
                    assert_eq!(left.role, right.role);
                    assert_eq!(left.content, right.content, "第 {index} 則不該隨角色改變");
                }
            }
        }
        // 台詞一律 assistant 且帶名字前綴；私設不在共用前綴裡
        let prefix: String = for_gal[..for_gal.len() - 1]
            .iter()
            .map(|message| message.content.as_str())
            .collect();
        assert!(prefix.contains("加爾：加爾抬起頭。"));
        assert!(prefix.contains("雷恩：雷恩笑了。"));
        assert!(!prefix.contains("加爾私設"));
        assert!(!prefix.contains("雷恩私設"));
        assert!(for_gal.last().unwrap().content.contains("加爾私設"));
        assert!(!for_gal.last().unwrap().content.contains("雷恩私設"));
    }

    /// CLI 三條（codex／agy／grok）攤平後也要逐字相同。共線前這裡只差「空行位置」——
    /// `push_merged` 的分組隨「這輪是誰」變動，`join("\n\n")` 的斷句就跟著移，
    /// 實測共同前綴只有 25 字元／全長 97，等於一開頭就分岔。
    #[test]
    fn shared_lane_flattens_identically_for_cli_paths() {
        let gal = card("gal", "加爾", "加爾公開", "加爾私設");
        let ray = card("ray", "雷恩", "雷恩公開", "雷恩私設");
        let cards = vec![gal.clone(), ray.clone()];
        let events = vec![
            event(TranscriptKind::Player, "p", "玩家", "我推開門。"),
            event(TranscriptKind::Dialogue, "gal", "加爾", "加爾抬起頭。"),
            event(TranscriptKind::Dialogue, "ray", "雷恩", "雷恩笑了。"),
            event(TranscriptKind::Narration, "gm", "GM", "燈光暗下。"),
            event(TranscriptKind::Dialogue, "gal", "加爾", "加爾又說了一句。"),
        ];
        let flatten = |card: &CharacterCard| {
            let messages = assemble_shared_messages(
                card,
                &cards,
                None,
                &events,
                &[],
                &TableState::default(),
                &Mechanism::default(),
                None,
                "zh-TW",
            );
            crate::cli::flatten_messages("", "", &messages)
        };
        let (gal_system, gal_prompt) = flatten(&gal);
        let (ray_system, ray_prompt) = flatten(&ray);
        assert_eq!(gal_system, ray_system);
        // 尾端那則本輪指定不同，之前的部分必須一字不差
        let shared = gal_prompt
            .chars()
            .zip(ray_prompt.chars())
            .take_while(|(left, right)| left == right)
            .count();
        assert!(
            gal_prompt[..gal_prompt
                .char_indices()
                .nth(shared)
                .map(|(index, _)| index)
                .unwrap_or(gal_prompt.len())]
                .contains("加爾又說了一句。"),
            "分岔點必須落在最後一則本輪指定，不能提前到歷史裡"
        );
        // 名字前綴只補一次，沒有「加爾：雷恩：」
        assert!(gal_prompt.contains("雷恩：雷恩笑了。"));
        assert!(!gal_prompt.contains("加爾：雷恩："));
    }

    /// 單角色桌：私設提回 system（只有一個角色不會洩漏，system 指令權重較高），尾端不重複。
    #[test]
    fn shared_lane_single_character_keeps_private_in_system() {
        let solo = card("solo", "獨角", "獨角公開", "獨角私設");
        let cards = vec![solo.clone()];
        let events = vec![event(TranscriptKind::Player, "p", "玩家", "嗨。")];
        let messages = assemble_shared_messages(
            &solo,
            &cards,
            None,
            &events,
            &[],
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
        );
        assert!(messages[0].content.contains("獨角私設"));
        assert!(!messages.last().unwrap().content.contains("獨角私設"));
        // 本輪指定仍在尾端
        assert!(messages.last().unwrap().content.contains("現在你是「獨角」"));
    }

    /// 有玩家卡時，角色與 GM 都要認得玩家的名字與公開身份（本功能的核心）
    #[test]
    fn player_card_enters_character_and_gm_context() {
        let fox = card("fox-id", "狐狸", "旅店老闆", "通緝犯");
        let player = card("player-id", "阿濤", "遠道而來的商隊護衛", "");

        let character_system = &assemble_shared_messages(
            &fox,
            std::slice::from_ref(&fox),
            Some(&player),
            &[],
            &[],
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
        )[0]
        .content;
        assert!(character_system.contains("阿濤"));
        assert!(character_system.contains("遠道而來的商隊護衛"));

        let gm_system = &assemble_gm_messages(
            "世界總覽",
            &[fox],
            Some(&player),
            &[],
            &[],
            &TableState::default(),
            &Mechanism::default(),
            &StateScope::default(),
            "zh-TW",
        )[0]
        .content;
        assert!(gm_system.contains("阿濤"));
        assert!(gm_system.contains("遠道而來的商隊護衛"));

        // 旁白指示併入點名（包 5）：要告知名單與玩家名字，GM 才知道喊誰；哨兵本身不變
        let instruction = narrate_instruction("zh-TW", &["狐狸".to_owned()], Some("阿濤")).content;
        assert!(instruction.contains("狐狸"));
        assert!(instruction.contains("阿濤"));
        assert!(instruction.contains(PLAYER_SENTINEL));
        assert!(instruction.contains("下一位"));
        // 名單空（純世界書開局等）＝退回純旁白，不要求點名行
        let solo = narrate_instruction("zh-TW", &[], None).content;
        assert!(!solo.contains("下一位"));
    }

    #[test]
    fn st_macros_use_player_name_and_keep_other_macros() {
        let fox = card(
            "fox-id",
            "狐狸",
            "我是 {{char}}，認識 {{user}}，{{random}} 保留。",
            "",
        );
        let player = card("player-id", "阿濤", "", "");
        let mut entry = worldbook_entry(
            0,
            "{{USER}} 的情報",
            &[],
            true,
            0,
            false,
            Visibility::Public,
        );
        entry.content = "{{user}} 來過這裡。".to_owned();

        let character = assemble_shared_messages(
            &fox,
            std::slice::from_ref(&fox),
            Some(&player),
            &[],
            &[entry.clone()],
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
        );
        let character_system = &character[0].content;
        assert!(character_system.contains("我是 狐狸，認識 阿濤，{{random}} 保留。"));
        assert!(character_system.contains("### 阿濤 的情報\n阿濤 來過這裡。"));

        let gm = assemble_gm_messages(
            "世界 {{CHAR}}",
            &[fox],
            Some(&player),
            &[],
            &[entry],
            &TableState::default(),
            &Mechanism::default(),
            &StateScope::default(),
            "zh-TW",
        );
        assert!(gm[0].content.contains("### 阿濤 的情報\n阿濤 來過這裡。"));
        assert!(gm[0].content.contains("世界 {{CHAR}}"));
    }

    #[test]
    fn gm_card_macros_use_each_cards_own_name() {
        let fox = card("fox-id", "狐狸", "A {{char}}", "");
        let knight = card("knight-id", "騎士", "B {{char}}", "");
        let gm = assemble_gm_messages(
            "",
            &[fox, knight],
            None,
            &[],
            &[],
            &TableState::default(),
            &Mechanism::default(),
            &StateScope::default(),
            "zh-TW",
        );
        let system = &gm[0].content;

        assert!(system.contains("公開設定：\nA 狐狸\n"));
        assert!(system.contains("公開設定：\nB 騎士\n"));
        assert!(!system.contains("公開設定：\nA 騎士\n"));
    }

    #[test]
    fn empty_worldbook_keeps_character_and_gm_context_unchanged() {
        let fox = card("fox-id", "狐狸", "旅店老闆", "通緝犯");
        let events = [event(TranscriptKind::Player, "", "玩家", "你好")];
        let character = assemble_shared_messages(
            &fox,
            std::slice::from_ref(&fox),
            None,
            &events,
            &[],
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
        );
        // 共線後多一則尾端本輪指定：system／逐字稿／本輪指定
        assert_eq!(character.len(), 3);
        assert!(character[0].content.contains("## 登場角色（公開設定）"));
        assert!(!character[0].content.contains("## 玩家角色")); // 沒有玩家卡就不佔段落
        assert!(!character[0].content.contains("## 你知道的世界情報")); // 世界書是空的
        assert_eq!(character[1], message("user", "玩家：你好".to_owned()));
        // 單角色桌：私設提回 system；本輪指定仍在尾端
        assert!(character[0].content.contains("的私有設定"));
        assert!(character[2].content.contains("現在你是「狐狸」"));

        let gm = assemble_gm_messages(
            "世界總覽",
            &[fox],
            None,
            &events,
            &[],
            &TableState::default(),
            &Mechanism::default(),
            &StateScope::default(),
            "zh-TW",
        );
        assert_eq!(gm.len(), 2);
        assert!(gm[0].content.contains("## 世界設定"));
        assert!(gm[0].content.contains("## 登場角色"));
        assert!(!gm[0].content.contains("玩家角色"));
        assert!(!gm[0].content.contains("## 世界書（只進你的上下文）"));
        assert_eq!(gm[1], message("user", "玩家：你好".to_owned()));
    }

    #[test]
    fn worldbook_visibility_separates_gm_public_and_character_contexts() {
        let entries = [
            worldbook_entry(0, "GM 祕密", &[], true, 0, false, Visibility::Gm),
            worldbook_entry(1, "公開情報", &[], true, 1, false, Visibility::Public),
            worldbook_entry(
                2,
                "狐狸情報",
                &[],
                true,
                2,
                false,
                Visibility::Characters(vec!["fox-id".to_owned()]),
            ),
            worldbook_entry(
                3,
                "騎士情報",
                &[],
                true,
                3,
                false,
                Visibility::Characters(vec!["knight-id".to_owned()]),
            ),
        ];
        let fox = card("fox-id", "狐狸", "公開", "私有");
        let fox_messages = assemble_shared_messages(
            &fox,
            std::slice::from_ref(&fox),
            None,
            &[],
            &entries,
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
        );
        let fox_system = &fox_messages[0].content;
        let fox_tail = &fox_messages.last().unwrap().content;
        // 公開 constant 進共用前綴；角色限定的走回合注入，留在尾端
        assert!(fox_system.contains("\n## 你知道的世界情報\n"));
        assert!(fox_system.contains("### 公開情報\n公開情報內容\n"));
        assert!(!fox_system.contains("狐狸情報"));
        assert!(fox_tail.contains("### 狐狸情報\n狐狸情報內容\n"));
        assert!(!fox_system.contains("GM 祕密"));
        assert!(!fox_tail.contains("GM 祕密"));
        assert!(!fox_system.contains("騎士情報"));
        assert!(!fox_tail.contains("騎士情報"));

        let knight = card("knight-id", "騎士", "公開", "私有");
        let knight_messages = assemble_shared_messages(
            &knight,
            std::slice::from_ref(&knight),
            None,
            &[],
            &entries,
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
        );
        let knight_system = &knight_messages[0].content;
        let knight_tail = &knight_messages.last().unwrap().content;
        assert!(knight_system.contains("公開情報"));
        assert!(!knight_system.contains("騎士情報"));
        assert!(knight_tail.contains("騎士情報"));
        assert!(!knight_system.contains("狐狸情報"));
        assert!(!knight_tail.contains("狐狸情報"));

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
        assert!(gm_system.contains("\n## 世界書（只進你的上下文）\n"));
        for title in ["GM 祕密", "公開情報", "狐狸情報", "騎士情報"] {
            assert!(gm_system.contains(title));
        }
        assert!(
            gm_system.find("世界總覽").unwrap() < gm_system.find("## 世界書").unwrap(),
            "世界書段落必須接在 world.md 段落之後"
        );
    }

    /// 驗收：GM 上下文含 world.md＋全部角色卡（含私有）＋公開歷史；
    /// GM 自己的旁白是 assistant，其餘事件是 user 且相鄰合併。
    #[test]
    fn gm_context_contains_world_all_cards_and_marks_own_narration() {
        let cards = [
            card("fox-id", "狐狸", "旅店老闆", "其實是通緝犯"),
            card("knight-id", "騎士", "巡邏騎士", "暗中追查狐狸"),
        ];
        let events = [
            event(TranscriptKind::Narration, "", "GM", "夜幕低垂"),
            event(TranscriptKind::Player, "", "玩家", "老闆，來杯麥酒"),
            event(TranscriptKind::Dialogue, "fox-id", "狐狸", "馬上來！"),
            event(TranscriptKind::Narration, "", "GM", "門外傳來馬蹄聲"),
        ];
        let messages = assemble_gm_messages(
            "酒館位於邊境小鎮",
            &cards,
            None,
            &events,
            &[],
            &TableState::default(),
            &Mechanism::default(),
            &StateScope::default(),
            "zh-TW",
        );

        let system = &messages[0];
        assert_eq!(system.role, "system");
        assert!(system.content.contains("酒館位於邊境小鎮"));
        assert!(system.content.contains("旅店老闆"));
        assert!(system.content.contains("其實是通緝犯"));
        assert!(system.content.contains("暗中追查狐狸"));

        let roles: Vec<&str> = messages.iter().map(|m| m.role.as_str()).collect();
        assert_eq!(roles, ["system", "assistant", "user", "assistant"]);
        assert_eq!(messages[1].content, "夜幕低垂");
        assert_eq!(messages[2].content, "玩家：老闆，來杯麥酒\n狐狸：馬上來！");
        assert_eq!(messages[3].content, "門外傳來馬蹄聲");
    }

    /// 驗收：語言規範依語系切換——zh-TW 注入繁中規範，en 注入英文規範
    #[test]
    fn language_rule_follows_ui_language() {
        let fox = card("fox-id", "狐狸", "旅店老闆", "");
        let zh = assemble_shared_messages(
            &fox,
            std::slice::from_ref(&fox),
            None,
            &[],
            &[],
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
        );
        assert!(zh[0].content.contains("繁體中文"));
        let en = assemble_shared_messages(
            &fox,
            std::slice::from_ref(&fox),
            None,
            &[],
            &[],
            &TableState::default(),
            &Mechanism::default(),
            None,
            "en",
        );
        assert!(en[0].content.contains("in natural, fluent English"));
        assert!(!en[0].content.contains("繁體中文"));

        let gm_en = assemble_gm_messages(
            "",
            &[],
            None,
            &[],
            &[],
            &TableState::default(),
            &Mechanism::default(),
            &StateScope::default(),
            "en",
        );
        assert!(gm_en[0].content.contains("in natural, fluent English"));

        // 其餘八個語系各自注入自己語言寫的規範，且不殘留繁中規範
        for (lang, needle) in [
            ("zh-CN", "简体中文"),
            ("ja", "日本語"),
            ("ko", "한국어"),
            ("es", "español"),
            ("pt-BR", "português do Brasil"),
            ("de", "Deutsch"),
            ("fr", "français"),
            ("ru", "русском"),
        ] {
            let messages = assemble_shared_messages(
            &fox,
            std::slice::from_ref(&fox),
                None,
                &[],
                &[],
                &TableState::default(),
                &Mechanism::default(),
                None,
                lang,
            );
            assert!(messages[0].content.contains(needle), "{lang} 規範沒注入");
            assert!(
                !messages[0].content.contains("繁體中文"),
                "{lang} 誤注入繁中規範"
            );
            let gm = assemble_gm_messages(
                "",
                &[],
                None,
                &[],
                &[],
                &TableState::default(),
                &Mechanism::default(),
                &StateScope::default(),
                lang,
            );
            assert!(gm[0].content.contains(needle), "{lang} GM 規範沒注入");
        }

        let mut config = AppConfig::default();
        assert_eq!(ui_language(&config), "zh-TW");
        config.preferences.insert(
            "language".to_owned(),
            serde_json::Value::String("en".to_owned()),
        );
        assert_eq!(ui_language(&config), "en");
    }

    /// 統一協定聲明只在這桌是增量桌（mechanism.incremental）時才出現在 GM 的 system prompt。
    #[test]
    fn mechanism_protocol_only_appears_when_table_is_incremental() {
        let fox = card("fox-id", "狐狸", "公開", "");
        let plain = assemble_gm_messages(
            "",
            std::slice::from_ref(&fox),
            None,
            &[],
            &[],
            &TableState::default(),
            &Mechanism::default(),
            &StateScope::default(),
            "zh-TW",
        );
        assert!(!plain[0].content.contains("狀態更新協定"));

        let incremental = Mechanism {
            incremental: true,
            ..Mechanism::default()
        };
        let with_protocol = assemble_gm_messages(
            "",
            &[fox],
            None,
            &[],
            &[],
            &TableState::default(),
            &incremental,
            &StateScope::default(),
            "zh-TW",
        );
        assert!(with_protocol[0].content.contains("## 狀態更新協定 v1"));
        assert!(with_protocol[0].content.contains("<UpdateVariable>"));

        // 卡自訂的回報指引接在通用協定後面（介面接管的卡才有）
        let with_guide = Mechanism {
            incremental: true,
            guide: "每回合都要重報 CurrentView.Time 與 CurrentView.SuggestedActions。".to_owned(),
            ..Mechanism::default()
        };
        let carded = assemble_gm_messages(
            "",
            &[],
            None,
            &[],
            &[],
            &TableState::default(),
            &with_guide,
            &StateScope::default(),
            "zh-TW",
        );
        // 順序：協定 → 卡的欄位說明 → 介面歸屬（歸屬壓最後，模型才不會模仿說明的排版）
        let protocol_at = carded[0].content.find("## 狀態更新協定 v1").unwrap();
        let guide_at = carded[0].content.find("CurrentView.SuggestedActions").unwrap();
        let notice_at = carded[0].content.find("## 介面由誰畫").unwrap();
        assert!(protocol_at < guide_at && guide_at < notice_at);
        // 介面歸屬只在接管桌出現：沒有卡專屬指引的增量桌不該看到這段
        assert!(!with_protocol[0].content.contains("## 介面由誰畫"));

        let en = assemble_gm_messages(
            "",
            &[],
            None,
            &[],
            &[],
            &TableState::default(),
            &incremental,
            &StateScope::default(),
            "en",
        );
        assert!(en[0].content.contains("State Update Protocol v1"));
    }

    /// 狀態是 GM 的檯面：角色只知道自己被說出口的部分，提示詞裡不該出現整張狀態表
    #[test]
    fn table_state_reaches_the_gm_only() {
        let fox = card("fox-id", "狐狸", "公開", "");
        let state = TableState {
            table: std::collections::BTreeMap::from([
                ("time".to_owned(), "午夜".to_owned()),
                ("沦陷天数".to_owned(), "第 3 天".to_owned()),
            ]),
            tree: std::collections::BTreeMap::new(),
            notes: Vec::new(),
            changes: std::collections::BTreeMap::new(),
            triggers: std::collections::BTreeMap::new(),
            jumps: std::collections::BTreeMap::new(),
        };
        let gm = assemble_gm_messages(
            "",
            &[fox.clone()],
            None,
            &[],
            &[],
            &state,
            &Mechanism::default(),
            &StateScope::default(),
            "zh-TW",
        );
        // 快取友善：狀態每輪更新，搬到尾端獨立 user 訊息，system 不再內嵌
        assert!(!gm[0].content.contains("目前狀態"));
        let tail = gm.last().unwrap();
        assert_eq!(tail.role, "user");
        assert!(tail.content.contains("## 目前狀態"));
        assert!(tail.content.contains("時間：午夜"));
        assert!(tail.content.contains("沦陷天数：第 3 天"));

        let character = assemble_shared_messages(
            &fox,
            std::slice::from_ref(&fox),
            None,
            &[],
            &[],
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
        );
        let joined: String = character.iter().map(|m| m.content.as_str()).collect();
        assert!(!joined.contains("午夜"));
        assert!(!joined.contains("目前狀態"));
    }

    /// 快取友善（prompt-cache-optimization A）：keyword 條目搬到尾端獨立 user 訊息，
    /// constant 條目留在 system；GM 的動態塊還併入「目前狀態」。
    #[test]
    fn keyword_entries_move_to_tail_message_constant_stay_in_system() {
        let entries = [
            worldbook_entry(0, "常駐情報", &[], true, 0, false, Visibility::Public),
            worldbook_entry(
                1,
                "龍的傳說",
                &["dragon"],
                false,
                1,
                false,
                Visibility::Public,
            ),
        ];
        let events = [event(TranscriptKind::Player, "", "玩家", "we saw a DRAGON")];
        let fox = card("fox-id", "狐狸", "公開", "");

        let character = assemble_shared_messages(
            &fox,
            std::slice::from_ref(&fox),
            None,
            &events,
            &entries,
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
        );
        assert!(character[0].content.contains("### 常駐情報"));
        assert!(!character[0].content.contains("龍的傳說"));
        let tail = character.last().unwrap();
        assert_eq!(tail.role, "user");
        assert!(tail.content.starts_with("## 你知道的世界情報"));
        assert!(tail.content.contains("### 龍的傳說\n龍的傳說內容"));
        // 動態塊獨立一則，不與前一則玩家發言合併
        assert_eq!(
            character[character.len() - 2].content,
            "玩家：we saw a DRAGON"
        );

        let mut state = TableState::default();
        state.table.insert("time".to_owned(), "午夜".to_owned());
        let gm = assemble_gm_messages(
            "世界",
            &[fox],
            None,
            &events,
            &entries,
            &state,
            &Mechanism::default(),
            &StateScope::default(),
            "zh-TW",
        );
        assert!(gm[0].content.contains("### 常駐情報"));
        assert!(!gm[0].content.contains("龍的傳說"));
        assert!(!gm[0].content.contains("目前狀態"));
        let gm_tail = gm.last().unwrap();
        assert_eq!(gm_tail.role, "user");
        assert!(gm_tail.content.starts_with("## 世界書（只進你的上下文）"));
        assert!(gm_tail.content.contains("### 龍的傳說"));
        // 同一則動態塊內：世界書在前、目前狀態在後
        assert!(
            gm_tail.content.find("龍的傳說").unwrap()
                < gm_tail.content.find("## 目前狀態").unwrap()
        );
        assert!(gm_tail.content.contains("時間：午夜"));
    }

    /// 快取友善驗收：連續兩輪組裝，去掉尾端動態塊與最新事件後，前綴逐字相同——
    /// 條目進出與狀態更新只影響尾端，不再從 context 第一段打破快取前綴。
    #[test]
    fn consecutive_rounds_share_verbatim_prefix_except_tail() {
        let entries = [
            worldbook_entry(0, "常駐", &[], true, 0, false, Visibility::Public),
            worldbook_entry(1, "龍", &["dragon"], false, 1, false, Visibility::Public),
            worldbook_entry(2, "碼頭", &["dock"], false, 2, false, Visibility::Public),
        ];
        let fox = card("fox-id", "狐狸", "公開", "私有");
        let mut round1_state = TableState::default();
        round1_state
            .table
            .insert("time".to_owned(), "黃昏".to_owned());
        let mut round2_state = TableState::default();
        round2_state
            .table
            .insert("time".to_owned(), "午夜".to_owned());

        let round1_events = vec![
            event(TranscriptKind::Narration, "", "GM", "你們遇見 dragon"),
            event(TranscriptKind::Player, "", "玩家", "先撤退"),
        ];
        let mut round2_events = round1_events.clone();
        round2_events.push(event(TranscriptKind::Narration, "", "GM", "你們逃到 dock"));

        let gm1 = assemble_gm_messages(
            "世界",
            std::slice::from_ref(&fox),
            None,
            &round1_events,
            &entries,
            &round1_state,
            &Mechanism::default(),
            &StateScope::default(),
            "zh-TW",
        );
        let gm2 = assemble_gm_messages(
            "世界",
            std::slice::from_ref(&fox),
            None,
            &round2_events,
            &entries,
            &round2_state,
            &Mechanism::default(),
            &StateScope::default(),
            "zh-TW",
        );
        // gm1 去尾端動態塊；gm2 去尾端動態塊＋最新事件——前綴逐字相同
        assert_eq!(gm1[..gm1.len() - 1], gm2[..gm2.len() - 2]);
        // 兩輪動態塊確實不同（條目進出＋狀態更新），只影響尾端一則
        assert_ne!(gm1.last(), gm2.last());

        // 角色路徑同理；新事件用自己的台詞（assistant），才不會與前一則 user 合併
        let mut round2_character_events = round1_events.clone();
        round2_character_events.push(event(
            TranscriptKind::Dialogue,
            "fox-id",
            "狐狸",
            "撤到 dock 去",
        ));
        let character1 = assemble_shared_messages(
            &fox,
            std::slice::from_ref(&fox),
            None,
            &round1_events,
            &entries,
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
        );
        let character2 = assemble_shared_messages(
            &fox,
            std::slice::from_ref(&fox),
            None,
            &round2_character_events,
            &entries,
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
        );
        assert_eq!(
            character1[..character1.len() - 1],
            character2[..character2.len() - 2]
        );
        assert_ne!(character1.last(), character2.last());
    }

}
