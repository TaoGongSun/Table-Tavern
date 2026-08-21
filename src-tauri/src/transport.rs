//! 傳輸層共用介面：上下文組裝→單發呼叫→串流回傳。
//! API 直連與（之後的）CLI 傳輸都必須經由 assemble_messages 取得上下文（KICKOFF §4）。

use crate::data::{
    self, AppConfig, CharacterCard, DataResult, InjectLevel, Mechanism, StateNode, TableState,
    Tier, TranscriptEvent, TranscriptKind, Visibility, WorldbookEntry,
};
use crate::mechanism;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";
pub const DEFAULT_IMAGE_MODEL: &str = "google/gemini-3.1-flash-image";

/// 角色與 GM system prompt 共用的語言規範，依使用者語系（config.preferences.language）注入。
/// 規範一律用該語言本身書寫，模型才不會被中文提示詞模板帶成中文輸出；
/// 兩岸中文互相加反向禁令，防止字體與用語飄移。
/// 新增語系：在此加一條 match arm，並補前端字典（src/i18n/）與範例桌（src-tauri/samples/）。
fn language_rule(lang: &str) -> &'static str {
    match lang {
        "zh-CN" => {
            "所有输出一律使用简体中文与中国大陆通行用语，禁止繁体字与台湾用语\
             （例如：说「视频」不说「影片」、说「质量」不说「品质」、说「信息」不说「讯息」）。"
        }
        "ja" => "出力はすべて自然で流暢な日本語で書くこと。",
        "ko" => "모든 출력은 자연스럽고 유창한 한국어로 작성할 것.",
        "es" => "Todo tu texto debe estar en español natural y fluido.",
        "pt-BR" => "Todo o seu texto deve estar em português do Brasil natural e fluente.",
        "de" => "Alle Ausgaben müssen in natürlichem, flüssigem Deutsch erfolgen.",
        "fr" => "Tout ton texte doit être rédigé dans un français naturel et fluide.",
        "ru" => "Весь твой текст должен быть на естественном, беглом русском языке.",
        _ if lang.starts_with("zh") => {
            "所有輸出一律使用繁體中文與台灣慣用語，禁止中國大陸用語與簡體字\
             （例如：說「影片」不說「視頻」、說「品質」不說「質量」、說「訊息」不說「信息」）。"
        }
        _ => "All of your output must be in natural, fluent English.",
    }
}

/// 沒有玩家卡時，依語系補上玩家稱呼。
pub(crate) fn player_fallback_name(lang: &str) -> &'static str {
    match lang {
        "zh-CN" => "玩家",
        "ja" => "プレイヤー",
        "ko" => "플레이어",
        "es" => "Jugador",
        "pt-BR" => "Jogador",
        "de" => "Spieler",
        "fr" => "Joueur",
        "ru" => "Игрок",
        _ if lang.starts_with("zh") => "玩家",
        _ => "Player",
    }
}

/// 要直接貼上畫面的文字（開場白）先把巨集換成當桌實名——存進 transcript 的就是玩家看到的樣子。
pub fn resolve_display_macros(
    text: &str,
    player_name: Option<&str>,
    char_name: &str,
    lang: &str,
) -> String {
    let user = player_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| player_fallback_name(lang));
    replace_st_macros(text, user, Some(char_name))
}

/// 只替換 SillyTavern 的玩家與角色巨集，其餘巨集保持原樣。
pub(crate) fn replace_st_macros(text: &str, user_name: &str, char_name: Option<&str>) -> String {
    let mut result = String::with_capacity(text.len());
    let mut index = 0;
    while index < text.len() {
        let rest = &text[index..];
        if rest.len() >= 8 && rest.as_bytes()[..8].eq_ignore_ascii_case(b"{{user}}") {
            result.push_str(user_name);
            index += 8;
        } else if rest.len() >= 8 && rest.as_bytes()[..8].eq_ignore_ascii_case(b"{{char}}") {
            if let Some(char_name) = char_name {
                result.push_str(char_name);
            } else {
                result.push_str(&rest[..8]);
            }
            index += 8;
        } else {
            let character = rest.chars().next().expect("index 必在字串範圍內");
            result.push(character);
            index += character.len_utf8();
        }
    }
    result
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

fn message(role: &str, content: String) -> ChatMessage {
    ChatMessage {
        role: role.to_owned(),
        content,
    }
}

/// 相鄰同 role 合併成一則，維持 user/assistant 交錯
fn push_merged(messages: &mut Vec<ChatMessage>, role: &str, line: String) {
    match messages.last_mut() {
        Some(last) if last.role == role => {
            last.content.push('\n');
            last.content.push_str(&line);
        }
        _ => messages.push(message(role, line)),
    }
}

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

/// 組裝單一角色的上下文。只餵入該角色自己的卡、可見世界書條目與公開 transcript：
/// 他人私有設定與 world.md 不在介面中；GM 專有條目組裝時一律排除，永不傳入模型。
/// keyword 觸發的條目放 transcript 尾端獨立 user 訊息（快取友善），constant 條目留在 system。
#[allow(clippy::too_many_arguments)]
pub fn assemble_messages(
    card: &CharacterCard,
    player: Option<&CharacterCard>,
    events: &[TranscriptEvent],
    worldbook: &[WorldbookEntry],
    state: &TableState,
    mechanism: &Mechanism,
    branch: Option<&[String]>,
    lang: &str,
) -> Vec<ChatMessage> {
    let user_name = player
        .map(|player| player.name.as_str())
        .unwrap_or_else(|| player_fallback_name(lang));
    let mut system = format!(
        "你正在一場多人桌上角色扮演中扮演「{name}」。\
         請一律用第三人稱敘事：動作與心理描寫都以「{name}」或「他／她」當主詞，\
         說出口的話寫在引號裡、維持這個角色自己的口吻；\
         視角只跟著「{name}」，可以寫他眼中所見的環境與心裡的感受，不要寫他不知道的事；\
         敘述不要用「我」當主詞、不要跳出角色、不要以 AI 助理的身分說話、\
         不要替其他角色或玩家代言。\
         {language_rule}\n\n\
         ## 你的公開設定（其他人也認識的你）\n{public}\n",
        name = card.name,
        language_rule = language_rule(lang),
        public = replace_st_macros(card.public_md.trim(), user_name, Some(&card.name)),
    );
    if !card.private_md.trim().is_empty() {
        system.push_str(&format!(
            "\n## 你的私有設定（只有你自己知道；除非劇情走到，不要主動說破）\n{}\n",
            replace_st_macros(card.private_md.trim(), user_name, Some(&card.name))
        ));
    }
    if let Some(player) = player {
        system.push_str(&format!(
            "\n## 同桌的玩家（真人扮演的角色，逐字稿裡的「{}」就是他）",
            player.name
        ));
        if !player.public_md.trim().is_empty() {
            system.push_str(&format!(
                "\n{}\n",
                replace_st_macros(player.public_md.trim(), user_name, Some(&player.name))
            ));
        }
    }
    let visible: Vec<_> = worldbook
        .iter()
        .filter(|entry| match &entry.visibility {
            Visibility::Gm => false,
            Visibility::Public => true,
            Visibility::Characters(ids) => ids.iter().any(|id| id == &card.id),
        })
        .cloned()
        .collect();
    // 快取友善（prompt-cache-optimization A）：constant 條目穩定，留在 system；
    // keyword 條目隨最近事件進出，若拼在 system 會從 context 第一段打破前綴，
    // 改放 transcript 尾端的一則獨立 user 訊息（最新事件附近，條目翻動只影響尾端）。
    let (constant_entries, keyword_entries): (Vec<_>, Vec<_>) =
        active_worldbook_entries(&visible, events)
            .into_iter()
            .partition(|entry| entry.constant);
    if !constant_entries.is_empty() {
        system.push_str("\n## 你知道的世界情報\n");
        for entry in constant_entries {
            system.push_str(&format!(
                "### {}\n{}\n",
                replace_st_macros(&entry.title, user_name, Some(&card.name)),
                replace_st_macros(&entry.content, user_name, Some(&card.name))
            ));
        }
    }

    let mut messages = vec![message("system", system)];
    for event in events {
        let (role, line) = match event.kind {
            TranscriptKind::Dialogue if event.speaker_id == card.id => {
                ("assistant", event.text.clone())
            }
            TranscriptKind::Dialogue => ("user", format!("{}：{}", event.speaker_name, event.text)),
            TranscriptKind::Player => ("user", format!("{}：{}", event.speaker_name, event.text)),
            TranscriptKind::Narration => ("user", format!("（旁白）{}", event.text)),
            TranscriptKind::System => ("user", format!("（系統）{}", system_event_text(event, true))),
        };
        push_merged(&mut messages, role, line);
    }
    let state_block = branch
        .and_then(|branch| character_state_block(state, mechanism, branch, &card.name, user_name));
    if !keyword_entries.is_empty() || state_block.is_some() {
        let mut block = String::new();
        if !keyword_entries.is_empty() {
            block.push_str("## 你知道的世界情報\n");
            for entry in keyword_entries {
                block.push_str(&format!(
                    "### {}\n{}\n",
                    replace_st_macros(&entry.title, user_name, Some(&card.name)),
                    replace_st_macros(&entry.content, user_name, Some(&card.name))
                ));
            }
        }
        if let Some(state_block) = &state_block {
            if !block.is_empty() {
                block.push('\n');
            }
            block.push_str(state_block);
        }
        // 刻意不走 push_merged：動態情報維持獨立一則的語意邊界，不黏進玩家發言
        messages.push(message("user", block.trim_end().to_owned()));
    }
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

/// GM 的 system prompt 本體：GM 指示＋world.md＋constant 條目＋全卡（含私設）＋玩家卡。
/// assemble_gm_messages（單發）與 gm_lane_system（resume 續聊凍結快照）共用。
/// constant 條目裡的 is_person 條目改走名冊行，不進全文（包 4a，見 split_person_roster）。
fn gm_system_prompt(
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

/// GM 的回合動態塊：keyword 條目＋「目前狀態」。
/// assemble_gm_messages（尾端獨立訊息）與 gm_lane_turn（resume 續聊回合尾段）共用。
/// 增量桌（mechanism.incremental）依 scope 裁切分支＋過濾葉子＋加變動標記；
/// 全量桌逐字維持現狀（不裁、不濾、不標）。
fn gm_dynamic_block(
    keyword_entries: &[&WorldbookEntry],
    state: &TableState,
    user_name: &str,
    mechanism: &Mechanism,
    scope: &StateScope,
    lang: &str,
) -> String {
    let mut dynamic = String::new();
    if !keyword_entries.is_empty() {
        dynamic.push_str("## 世界書（只進你的上下文）\n");
        for entry in keyword_entries {
            dynamic.push_str(&format!(
                "### {}\n{}\n",
                replace_st_macros(&entry.title, user_name, None),
                replace_st_macros(&entry.content, user_name, None)
            ));
        }
    }
    let mut tree_text = String::new();
    render_state_tree(
        &mut tree_text,
        &state.tree,
        &TreeRender {
            mechanism,
            changes: &state.changes,
            hidden: &scope.hidden,
            align: scope.align,
            user_name,
            base: 0,
        },
        &mut Vec::new(),
    );
    if !state.table.is_empty() || !tree_text.is_empty() {
        if !dynamic.is_empty() {
            dynamic.push('\n');
        }
        let header = if mechanism.incremental && scope.align {
            "## 目前狀態（完整對齊，以下是系統帳上的真值，請以此為準）\n"
        } else {
            "## 目前狀態（這桌的檯面，接續它往下演）\n"
        };
        dynamic.push_str(header);
        for (key, value) in &state.table {
            let display_name = match (lang, key.as_str()) {
                ("en", "time") => "Time",
                ("en", "place") => "Place",
                ("en", "present") => "Present",
                (_, "time") => "時間",
                (_, "place") => "地點",
                (_, "present") => "在場人物",
                _ => key,
            };
            dynamic.push_str(&format!("{display_name}：{value}\n"));
        }
        dynamic.push_str(&tree_text);
    }
    if mechanism.incremental && !state.triggers.is_empty() {
        let lines: Vec<&str> = mechanism
            .triggers
            .iter()
            .filter(|trigger| scope.align || !trigger_scope_hidden(&trigger.scope, &scope.hidden))
            .filter_map(|trigger| state.triggers.get(&trigger.id))
            .map(String::as_str)
            .collect();
        if !lines.is_empty() {
            if !dynamic.is_empty() {
                dynamic.push('\n');
            }
            dynamic.push_str("## 當前情境（系統依狀態表判定的隱藏背景，不要在回覆裡複述本段）\n");
            for (index, text) in lines.iter().enumerate() {
                if index > 0 {
                    dynamic.push('\n');
                }
                dynamic.push_str(text);
                dynamic.push('\n');
            }
        }
    }
    if !state.notes.is_empty() {
        if !dynamic.is_empty() {
            dynamic.push('\n');
        }
        dynamic.push_str("## 上一輪被系統擋下的更新（請照這些現值修正）\n");
        for note in &state.notes {
            dynamic.push_str(&format!("{note}\n"));
        }
    }
    dynamic.trim_end().to_owned()
}

/// 一次注入要用的渲染參數；路徑與輸出隨遞迴走，其餘整趟固定。
struct TreeRender<'a> {
    mechanism: &'a Mechanism,
    changes: &'a BTreeMap<String, String>,
    hidden: &'a [Vec<String>],
    align: bool,
    user_name: &'a str,
    /// 縮排基準：從第幾層開始算第一級（角色線只印自己那支，要從行首印起）
    base: usize,
}

/// 樹沿用模型最容易產生的 YAML 形狀，讓本期全量注入不因資料升級漏掉任何狀態。
/// 全量桌（!mechanism.incremental）：逐字維持現狀，不裁不濾不標。
/// 增量桌：`hidden` 之外的分支才印（align 時忽略 hidden，整棵樹都印）；
/// 葉子依 inject 過濾——Turn 一律印，Snapshot 只有 align 時才印，Rare 一律不印；
/// `changes` 有值的葉子在值後面加全形括號標記。過濾後變空的分支不留空標題。
fn render_state_tree(
    output: &mut String,
    tree: &BTreeMap<String, StateNode>,
    render: &TreeRender,
    path: &mut Vec<String>,
) {
    let incremental = render.mechanism.incremental;
    for (key, node) in tree {
        path.push(key.clone());
        let indent = "  ".repeat(path.len() - 1 - render.base);
        let branch_hidden = incremental && !render.align && render.hidden.iter().any(|h| h == path);
        if !branch_hidden {
            match node {
                StateNode::Leaf(value) => {
                    let show = if incremental {
                        let rule = mechanism::rule_for_path(render.mechanism, path, Some(value));
                        match rule.inject {
                            InjectLevel::Rare => false,
                            InjectLevel::Snapshot => render.align,
                            InjectLevel::Turn => true,
                        }
                    } else {
                        true
                    };
                    if show {
                        let mark = if incremental {
                            render
                                .changes
                                .get(&path.join("."))
                                .map(|mark| format!("（{mark}）"))
                                .unwrap_or_default()
                        } else {
                            String::new()
                        };
                        output.push_str(&format!(
                            "{indent}{key}：{}{mark}\n",
                            replace_st_macros(value, render.user_name, None),
                        ));
                    }
                }
                StateNode::Branch(children) => {
                    let header = format!("{indent}{key}：\n");
                    let before = output.len();
                    output.push_str(&header);
                    render_state_tree(output, children, render, path);
                    if incremental && output.len() == before + header.len() {
                        output.truncate(before);
                    }
                }
            }
        }
        path.pop();
    }
}

/// 觸發表裁切：`trigger_scope` 正好是 `hidden` 某一條、或是其後代路徑，就該裁掉
/// （不在場角色的關係階段文本不該送）。空 `trigger_scope`＝桌級，永遠不裁。
fn trigger_scope_hidden(trigger_scope: &[String], hidden: &[Vec<String>]) -> bool {
    !trigger_scope.is_empty()
        && hidden.iter().any(|branch| {
            trigger_scope.len() >= branch.len() && trigger_scope[..branch.len()] == branch[..]
        })
}

/// 這一輪要裁掉哪些分支、要不要送全樹對齊——回合尾注入策略（包 5）。
#[derive(Debug, Clone, Default)]
pub struct StateScope {
    /// 本輪不送的分支路徑（不在場角色的那一支）。空＝不裁切。
    pub hidden: Vec<Vec<String>>,
    /// 這一輪送全樹對齊（換幕後第一輪 GM 回合）。
    pub align: bool,
}

/// 算這一輪的狀態視角：全量桌完全不裁；增量桌依在場名單裁掉不在場角色的分支
/// （玩家那支永遠送）；在場欄空著就寧可全送，不要因為模型沒報 present 就裁瞎了。
///
/// 認得出來的角色分支有兩種：綁到角色卡的，以及**與它同一個容器的手足**——
/// 一張 MVU 卡的 15 個英雄只會有幾張角色卡，剩下的手足照樣是人、照樣該裁。
/// 手足規則只在容器不是樹根時生效：頂層放的是 World／Player 這類桌級分支，掃進去會把整桌裁掉。
pub fn state_scope(
    state: &TableState,
    mechanism: &Mechanism,
    cards: &[CharacterCard],
    player: Option<&CharacterCard>,
    bindings: &BTreeMap<String, Vec<String>>,
    align: bool,
) -> StateScope {
    if !mechanism.incremental {
        return StateScope::default();
    }
    let present: Vec<String> = state
        .table
        .get("present")
        .map(String::as_str)
        .unwrap_or("")
        .split(['、', '，', ',', '／', '/', '；', ';'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect();
    if present.is_empty() {
        return StateScope {
            hidden: Vec::new(),
            align,
        };
    }
    let player_id = player.map(|player| player.id.as_str());
    let is_present = |name: &str| {
        present
            .iter()
            .any(|item| item == name || item.contains(name))
    };
    let mut hidden: Vec<Vec<String>> = Vec::new();
    let mut keep: Vec<Vec<String>> = Vec::new(); // 玩家那支永遠送，手足規則也不准碰
    let mut containers: Vec<Vec<String>> = Vec::new();
    for card in cards {
        let Some(branch) = resolve_branch(&state.tree, bindings, &card.id, &card.name) else {
            continue; // 沒有分支就沒東西可裁
        };
        if branch.len() > 1 {
            let container = branch[..branch.len() - 1].to_vec();
            if !containers.contains(&container) {
                containers.push(container);
            }
        }
        if Some(card.id.as_str()) == player_id {
            keep.push(branch);
        } else if !is_present(&card.name) {
            hidden.push(branch);
        }
    }
    // 手足：同容器裡沒有角色卡的分支，名字沒出現在在場名單就一起裁
    for container in &containers {
        let Some(StateNode::Branch(children)) = data::node_at(&state.tree, container) else {
            continue;
        };
        for (name, node) in children {
            if !matches!(node, StateNode::Branch(_)) {
                continue;
            }
            let mut path = container.clone();
            path.push(name.clone());
            if keep.contains(&path) || hidden.contains(&path) || is_present(name) {
                continue;
            }
            hidden.push(path);
        }
    }
    StateScope { hidden, align }
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
fn split_person_roster<'a>(
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

/// 人物登場事件的固定前綴，接著是〈title〉那一行；`append_transcript` 寫進去的格式，
/// 也是掃「本幕已登場集合」與下一包前端顯示唯一依據的字面。
pub const PERSON_ARRIVAL_PREFIX: &str = "（人物登場）";

/// 從一則事件文字剝出登場標題（前綴＋〈title〉開頭那一行）；不是登場事件就回 `None`。
fn arrival_title(text: &str) -> Option<String> {
    let rest = text.strip_prefix(PERSON_ARRIVAL_PREFIX)?.strip_prefix('〈')?;
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

/// 角色卡回歸事件的內文：固定前綴＋〈name〉一行，接公開設定＋私有設定全文
/// （`{{user}}`／`{{char}}` 已代換）。格式對照 gm_system_prompt 的全卡呈現；
/// chars 快照本來就含全卡（lib.rs load_active_cards 註解），回歸事件不算新洩漏，
/// 呼叫端一律標 gm_only=false。
pub fn card_arrival_text(card: &CharacterCard, user_name: &str) -> String {
    let mut text = format!("{}〈{}〉", data::CARD_ARRIVAL_PREFIX, card.name);
    if !card.public_md.trim().is_empty() {
        text.push_str(&format!(
            "\n公開設定：\n{}",
            replace_st_macros(card.public_md.trim(), user_name, Some(&card.name))
        ));
    }
    if !card.private_md.trim().is_empty() {
        text.push_str(&format!(
            "\n私有設定：\n{}",
            replace_st_macros(card.private_md.trim(), user_name, Some(&card.name))
        ));
    }
    text
}

/// 角色卡對應的狀態樹分支：面板指認優先，其次全樹同名比對。
/// 指認的路徑若在樹裡不存在或不是分支，視為失效、退回自動比對。
pub fn resolve_branch(
    tree: &BTreeMap<String, StateNode>,
    bindings: &BTreeMap<String, Vec<String>>,
    card_id: &str,
    card_name: &str,
) -> Option<Vec<String>> {
    if let Some(path) = bindings.get(card_id) {
        if !path.is_empty() && matches!(data::node_at(tree, path), Some(StateNode::Branch(_))) {
            return Some(path.clone());
        }
    }
    auto_match_branch(tree, card_name)
}

/// 廣度優先找 key 完全等於卡名的分支節點（葉子不算），深度上限 3，取最淺的一筆。
fn auto_match_branch(tree: &BTreeMap<String, StateNode>, card_name: &str) -> Option<Vec<String>> {
    let mut level: Vec<(Vec<String>, &BTreeMap<String, StateNode>)> = vec![(Vec::new(), tree)];
    for _ in 0..3 {
        let mut next = Vec::new();
        for (path, branch) in &level {
            for (key, node) in *branch {
                let StateNode::Branch(children) = node else {
                    continue;
                };
                let mut candidate = path.clone();
                candidate.push(key.clone());
                if key == card_name {
                    return Some(candidate);
                }
                next.push((candidate, children));
            }
        }
        level = next;
    }
    None
}

/// 角色自己那支的狀態（唯讀，給扮演參考）。沒綁到分支或該支空的就是 None。
/// 排除 `inject == Rare` 的葉子，帶變動標記，`{{user}}` 照舊代換。
pub fn character_state_block(
    state: &TableState,
    mechanism: &Mechanism,
    branch: &[String],
    card_name: &str,
    user_name: &str,
) -> Option<String> {
    if branch.is_empty() {
        return None;
    }
    let StateNode::Branch(children) = data::node_at(&state.tree, branch)? else {
        return None; // 分支路徑指到葉子，視同沒有分支
    };
    if children.is_empty() {
        return None;
    }
    let mut body = String::new();
    let mut path = branch.to_vec();
    // align=true：除了 Rare，全部印出（角色要看自己完整的檯面，不是只看這輪變動）。
    render_state_tree(
        &mut body,
        children,
        &TreeRender {
            mechanism,
            changes: &state.changes,
            hidden: &[],
            align: true,
            user_name,
            base: branch.len(),
        },
        &mut path,
    );
    if body.is_empty() {
        return None;
    }
    Some(format!(
        "## 「{card_name}」目前的狀態（系統帳，唯讀；可以拿來演，但不要輸出任何狀態欄或更新區塊）\n{}",
        body.trim_end()
    ))
}

/// 長文字欄（inject == Snapshot）這一輪的新值：(點分路徑, 值)。
/// 只回 `state.changes` 裡有的那些；`{{user}}` 在這裡就代換掉——這批要落成 transcript
/// 系統事件（不再進回合尾的動態塊），事件文字之後不會再過巨集代換，留字面會直接漏進提示詞。
/// 全量桌（!mechanism.incremental）一律回空。
pub fn snapshot_updates(
    state: &TableState,
    mechanism: &Mechanism,
    user_name: &str,
) -> Vec<(String, String)> {
    if !mechanism.incremental || state.changes.is_empty() {
        return Vec::new();
    }
    let mut updates = Vec::new();
    collect_snapshot_updates(
        &state.tree,
        mechanism,
        &state.changes,
        user_name,
        &mut Vec::new(),
        &mut updates,
    );
    updates
}

fn collect_snapshot_updates(
    tree: &BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    changes: &BTreeMap<String, String>,
    user_name: &str,
    path: &mut Vec<String>,
    updates: &mut Vec<(String, String)>,
) {
    for (key, node) in tree {
        path.push(key.clone());
        match node {
            StateNode::Leaf(value) => {
                // 路徑就地 join 去比對 changes，不用 split('.') 反推——欄位名本身可能含 '.'。
                let path_key = path.join(".");
                if changes.contains_key(&path_key) {
                    let rule = mechanism::rule_for_path(mechanism, path, Some(value));
                    if rule.inject == InjectLevel::Snapshot {
                        updates.push((path_key, replace_st_macros(value, user_name, None)));
                    }
                }
            }
            StateNode::Branch(children) => {
                collect_snapshot_updates(children, mechanism, changes, user_name, path, updates);
            }
        }
        path.pop();
    }
}

/// resume 續聊線（prompt-cache-optimization 包 2）的回合尾段。
/// tail 是跟在新事件後送出的動態文字；confidential 是 tail 內回合結束後
/// 要從 session 檔抹掉的子段（chars 線的私設＋限定條目，防洩漏給下一個被點的角色）。
pub struct LaneTurn {
    pub tail: String,
    pub confidential: Option<String>,
}

/// System 事件的顯示文字：`redact_gm_only` 為真且該事件 `gm_only` 時只留第一行（前綴＋標題），
/// 不含全文；其餘一律原文（AI 卡重構包 4b）。chars 線（單發 assemble_messages／chars lane）
/// 傳真，GM 線一律傳假——GM 看得到一切，這是既有可見性憲法。
fn system_event_text(event: &TranscriptEvent, redact_gm_only: bool) -> String {
    if redact_gm_only && event.gm_only {
        event.text.lines().next().unwrap_or_default().to_owned()
    } else {
        event.text.clone()
    }
}

/// 事件在 lane prompt 裡的一行。續聊線的歷史全部以名字標注成純文字
/// （誰說的靠「X：」前綴分辨，不靠 role），與 session 內既有歷史逐字銜接。
/// `redact_gm_only`：chars lane 傳真（洩漏修正），GM lane 傳假（全文）。
pub fn lane_event_line(event: &TranscriptEvent, redact_gm_only: bool) -> String {
    match event.kind {
        TranscriptKind::Dialogue | TranscriptKind::Player => {
            format!("{}：{}", event.speaker_name, event.text)
        }
        TranscriptKind::Narration => format!("（旁白）{}", event.text),
        TranscriptKind::System => format!("（系統）{}", system_event_text(event, redact_gm_only)),
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
    let mut system = format!(
        "你是這場多人桌上角色扮演的扮演引擎，「登場角色」名單上的角色都可能由你扮演，\
         每一輪的結尾會指定你這一輪演誰。請一律用第三人稱敘事：動作與心理描寫都以\
         被指定角色的名字或「他／她」當主詞，說出口的話寫在引號裡、維持這個角色自己的口吻；\
         視角只跟著被指定的角色，可以寫他眼中所見的環境與心裡的感受，不要寫他不知道的事；\
         敘述不要用「我」當主詞、不要跳出角色、不要以 AI 助理的身分說話、\
         不要替其他角色或玩家代言。\
         {language_rule}\n",
        language_rule = language_rule(lang),
    );
    if !cards.is_empty() {
        system.push_str("\n## 登場角色（公開設定）\n");
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
    let mut constants: Vec<&WorldbookEntry> = worldbook
        .iter()
        .filter(|entry| {
            !entry.disabled && entry.constant && matches!(entry.visibility, Visibility::Public)
        })
        .collect();
    constants.sort_by_key(|entry| (entry.order, entry.uid));
    let (constants, roster) = split_person_roster(&constants);
    if !constants.is_empty() || roster.is_some() {
        system.push_str("\n## 你知道的世界情報\n");
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
    for entry in active_worldbook_entries(&visible, events) {
        match &entry.visibility {
            Visibility::Public if entry.constant => {} // 已在凍結快照
            Visibility::Public => public_keyword.push(entry),
            _ => limited.push(entry), // Characters 限定：不論 constant 都走回合注入
        }
    }

    let mut tail = String::new();
    if !public_keyword.is_empty() {
        tail.push_str("## 你知道的世界情報\n");
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
    if !card.private_md.trim().is_empty() {
        confidential.push_str(&format!(
            "## 「{}」的私有設定（只有他自己知道；除非劇情走到，不要主動說破）\n{}\n",
            card.name,
            replace_st_macros(card.private_md.trim(), user_name, Some(&card.name))
        ));
    }
    if !limited.is_empty() {
        confidential.push_str(&format!("## 只有「{}」知道的世界情報\n", card.name));
        for entry in limited {
            confidential.push_str(&format!(
                "### {}\n{}\n",
                replace_st_macros(&entry.title, user_name, Some(&card.name)),
                replace_st_macros(&entry.content, user_name, Some(&card.name))
            ));
        }
    }
    if let Some(block) = branch
        .and_then(|branch| character_state_block(state, mechanism, branch, &card.name, user_name))
    {
        confidential.push_str(&block);
        confidential.push('\n');
    }
    if !confidential.is_empty() {
        tail.push_str(&confidential);
        tail.push('\n');
    }
    tail.push_str(&format!(
        "現在你是「{name}」。請直接用第三人稱輸出「{name}」的動作、台詞與心理描寫，\
         敘述主詞是「{name}」或「他／她」、不要用「我」，說出口的話寫在引號裡；\
         不要加名字前綴、不要任何角色之外的說明。",
        name = card.name
    ));
    LaneTurn {
        tail,
        confidential: (!confidential.is_empty()).then_some(confidential),
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
    let keyword_entries: Vec<&WorldbookEntry> = active_worldbook_entries(worldbook, events)
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
    }
}

/// 組裝「換場摘要」上下文：GM 檔位讀公開 transcript，把本場景壓成一則前情提要。
/// 不含 world.md／角色卡——摘要只需壓縮已發生的公開事件，不需要世界觀全貌。
pub fn summary_messages(events: &[TranscriptEvent], lang: &str) -> Vec<ChatMessage> {
    let instruction = if lang == "en" {
        "You are the GM of a multiplayer tabletop RPG session that is about to change scenes. \
         The first line of your reply must be exactly \"Title: <act name, 10 words or fewer>\", \
         followed by a blank line before the recap. \
         Summarize everything that happened in this scene as a recap, covering: \
         location and time, who is present and their state, key events, relationship changes, \
         and unresolved threads — as a compact bulleted list. \
         Output only the summary body, in English."
            .to_owned()
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
    for event in events {
        let line = match event.kind {
            TranscriptKind::Narration => format!("（旁白）{}", event.text),
            TranscriptKind::Dialogue | TranscriptKind::Player => {
                format!("{}：{}", event.speaker_name, event.text)
            }
            TranscriptKind::System => format!("（系統）{}", event.text),
        };
        push_merged(&mut messages, "user", line);
    }
    messages
}

/// 從換場摘要回覆的第一行取幕名：以「標題：」或「Title:」開頭（大小寫寬鬆）才算，
/// 取到就把該行（含後面的空行）從摘要文字中拿掉；解析不到就回傳 None、原文整段當摘要。
pub fn extract_scene_title(reply: &str) -> (Option<String>, String) {
    let mut lines = reply.splitn(2, '\n');
    let first_line = lines.next().unwrap_or("").trim();
    let remainder = lines.next().unwrap_or("");

    let title = first_line
        .strip_prefix("標題：")
        .or_else(|| first_line.strip_prefix("標題:"))
        .map(str::to_owned)
        .or_else(|| {
            let lower = first_line.to_lowercase();
            lower
                .strip_prefix("title:")
                .map(|_| first_line[6..].to_owned())
        })
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty());

    match title {
        Some(name) => (Some(name), remainder.trim_start_matches('\n').to_owned()),
        None => (None, reply.to_owned()),
    }
}

/// 導演指示：插入旁白（附加在 GM 上下文最後）；有名單時尾端固定要一行「下一位：」點名，
/// 旁白與點名一次呼叫完成（包 5 拍板）。
pub fn narrate_instruction(
    lang: &str,
    roster: &[String],
    player_name: Option<&str>,
) -> ChatMessage {
    let mut instruction = if lang == "en" {
        "(Director instruction) Insert a narration: describe scene changes, the world's response, or plot progress. \
         Use any length the story needs. You may portray supporting characters without character cards, but do not speak for listed characters or the player. \
         After the narration, start a new line and output one ```state fence. Inside it, write one `key: value` field per line. \
         Always output `time`, `place`, and `present`; write values in the player's language, separate multiple present characters with `、`, and repeat unchanged values too. \
         Do not add any explanation outside the narration and fence."
            .to_owned()
    } else {
        "（導演指示）請插入一段旁白：描述場景變化、世界反應或劇情推進，\
         篇幅不設限，依劇情需要自由發揮。沒有角色卡的配角由你扮演，可以出場與說話；\
         不要替「登場角色」名單上的角色或玩家說話。旁白本文結束後另起一行，輸出一個 ```state 圍欄，\
         圍欄內一行一欄、格式為「鍵: 值」，固定輸出 time、place、present 三個鍵；\
         值用玩家的語言寫，present 的多人用頓號分隔，沒有變化的欄位也照抄目前值。\
         圍欄以外不要有多餘說明。"
            .to_owned()
    };
    if !roster.is_empty() {
        let call = if lang == "en" {
            format!(
                " After the fence, end with exactly one final line `Next: <name>`, choosing the next speaker from: {}. \
                 If it is the player{player}'s turn to act, write `Next: {PLAYER_SENTINEL}` instead.",
                roster.join("、"),
                player = player_name.map_or(String::new(), |name| format!(" ({name})")),
            )
        } else {
            format!(
                "圍欄之後最後再另起一行，固定輸出「下一位：〈名字〉」，\
                 從名單中選出下一位最適合發言的角色：{}。\
                 若現在應該輪到玩家{player}行動，就寫「下一位：{PLAYER_SENTINEL}」。",
                roster.join("、"),
                // 沒玩家卡時是空字串，句子與加玩家卡前逐字相同
                player = player_name.map_or(String::new(), |name| format!("（{name}）")),
            )
        };
        instruction.push_str(&call);
    }
    message("user", instruction)
}

/// 卡片自帶介面的桌：卡片自己規定了輸出格式，我們不再要求旁白＋state 圍欄，
/// 否則兩套指令打架、模型會照我們的寫，卡片的介面就永遠對不上。
pub fn card_format_instruction(lang: &str, entry_title: Option<&str>) -> ChatMessage {
    let instruction = if lang == "en" {
        let format_source = entry_title
            .map(|title| format!(" (see the worldbook entry \"{title}\")"))
            .unwrap_or_default();
        format!(
            "(Director instruction) This table uses the interface that ships with the card, and the card already defines the reply format{format_source}. \
             Follow that specification exactly for this turn: same tags, same block order, same counts, same required fields, with the content advancing the story. \
             Do not rewrite it as ordinary narration, and do not output anything outside that format."
        )
    } else {
        let format_source = entry_title
            .map(|title| format!("（見世界書「{title}」）"))
            .unwrap_or_default();
        format!(
            "（導演指示）這桌使用卡片自帶的介面，卡片已經規定了回覆的輸出格式{format_source}。\
             請完全依照那份規定產生本回合的回覆：標籤、區塊順序、數量與必填欄位都照規定，內容依劇情推進。\
             不要改寫成一般旁白，也不要輸出規定格式以外的任何說明或狀態欄。"
        )
    };
    message("user", instruction)
}

/// 從旁白剝出尾端的「下一位：」點名行：回傳（點名原文, 剝除後的顯示文字）。
/// 與 extract_state_block 同族：只認整行、行首標記，掃到多行取最後一行；沒有就原樣返回。
pub fn extract_next_speaker(reply: &str) -> (Option<String>, String) {
    let hit = reply.lines().rev().find_map(|line| {
        let trimmed = line.trim();
        let rest = ["下一位", "next"].iter().find_map(|marker| {
            if trimmed.len() < marker.len() || !trimmed.is_char_boundary(marker.len()) {
                return None;
            }
            let (head, tail) = trimmed.split_at(marker.len());
            if head.to_ascii_lowercase() != *marker {
                return None;
            }
            let tail = tail.trim_start();
            tail.strip_prefix('：').or_else(|| tail.strip_prefix(':'))
        })?;
        let name = rest.trim();
        if name.is_empty() {
            return None;
        }
        Some((line.to_owned(), name.to_owned()))
    });
    match hit {
        Some((line, name)) => {
            let display = reply.replacen(&line, "", 1).trim_end().to_owned();
            (Some(name), display)
        }
        None => (None, reply.to_owned()),
    }
}

struct StateTag {
    start: usize,
    content_start: usize,
    content_end: usize,
    end: usize,
    /// UpdateVariable 只剝不收；狀態標籤才收欄位。
    collect: bool,
}

/// 掃出下一個狀態標籤。標籤名走前綴比對——`<StatusData>`、`<Status_block>` 各家自己取名，
/// 開頭是 status 就算；開閉標籤要同名才配對，免得吃掉後面不相干的內容。
fn find_state_tag(display: &str) -> Option<StateTag> {
    let lower = display.to_ascii_lowercase();
    let mut cursor = 0;
    while let Some(offset) = lower[cursor..].find('<') {
        let start = cursor + offset;
        cursor = start + 1;
        let Some(name_end) = lower[cursor..]
            .find(|character: char| {
                character == '>' || character == '/' || character.is_whitespace()
            })
            .map(|index| cursor + index)
        else {
            break;
        };
        let name = &lower[cursor..name_end];
        let collect = name.starts_with("status");
        if !collect && name != "updatevariable" {
            continue;
        }
        let Some(open_end) = lower[name_end..].find('>').map(|index| name_end + index) else {
            break;
        };
        let closing = format!("</{name}>");
        let Some(close_start) = lower[open_end + 1..]
            .find(&closing)
            .map(|index| open_end + 1 + index)
        else {
            continue;
        };
        return Some(StateTag {
            start,
            content_start: open_end + 1,
            content_end: close_start,
            end: close_start + closing.len(),
            collect,
        });
    }
    None
}

/// extract_state_block 的回傳：欄位對、原始 `<UpdateVariable>` 內容（供 mechanism 解析）、
/// 剝除後的顯示文字。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateBlock {
    pub fields: Vec<(Vec<String>, String)>,
    pub updates: Vec<String>,
    pub display: String,
}

/// 從 GM 回覆剝出狀態區塊。
/// 標籤比對一律走 to_ascii_lowercase——full lowercase 會改變某些字母的長度（如土耳其文 İ），
/// 算出的位移拿回原字串切片就會切在非字元邊界上 panic。
pub fn extract_state_block(reply: &str) -> StateBlock {
    let mut display = reply.to_owned();
    let mut blocks = Vec::new();
    let mut updates = Vec::new();
    let mut removed = false;

    let mut details_cursor = 0;
    while let Some(offset) = display[details_cursor..]
        .to_ascii_lowercase()
        .find("<details")
    {
        let start = details_cursor + offset;
        let Some(open_end) = display[start..].find('>').map(|index| start + index) else {
            break;
        };
        let lower = display.to_ascii_lowercase();
        let Some(end_start) = lower[open_end + 1..]
            .find("</details>")
            .map(|index| open_end + 1 + index)
        else {
            break;
        };
        let inner = &display[open_end + 1..end_start];
        let inner_lower = inner.to_ascii_lowercase();
        let Some(summary_start) = inner_lower.find("<summary") else {
            details_cursor = end_start + "</details>".len();
            continue;
        };
        let Some(summary_open_end) = inner[summary_start..]
            .find('>')
            .map(|index| summary_start + index)
        else {
            details_cursor = end_start + "</details>".len();
            continue;
        };
        let Some(summary_end) = inner_lower[summary_open_end + 1..]
            .find("</summary>")
            .map(|index| summary_open_end + 1 + index)
        else {
            details_cursor = end_start + "</details>".len();
            continue;
        };
        let summary = &inner[summary_open_end + 1..summary_end];
        let summary_lower = summary.to_ascii_lowercase();
        if !(summary_lower.contains("状态")
            || summary_lower.contains("狀態")
            || summary_lower.contains("status"))
        {
            details_cursor = end_start + "</details>".len();
            continue;
        }
        blocks.push(inner[summary_end + "</summary>".len()..].to_owned());
        display.replace_range(start..end_start + "</details>".len(), "");
        details_cursor = start;
        removed = true;
    }

    while let Some(tag) = find_state_tag(&display) {
        let content = display[tag.content_start..tag.content_end].to_owned();
        if tag.collect {
            blocks.push(content);
        } else {
            // UpdateVariable 是 MVU 的 JSON patch，原始內容交給 mechanism::parse_updates 解析。
            updates.push(content);
        }
        display.replace_range(tag.start..tag.end, "");
        removed = true;
    }

    // 鎮北王府那類把正文包在 <maintext> 裡：拆掉外殼留正文，標籤不裸露在畫面上。
    loop {
        let lower = display.to_ascii_lowercase();
        let Some(start) = lower.find("<maintext>") else {
            break;
        };
        let content_start = start + "<maintext>".len();
        if let Some(close_start) = lower[content_start..]
            .find("</maintext>")
            .map(|index| content_start + index)
        {
            display.replace_range(close_start..close_start + "</maintext>".len(), "");
        }
        display.replace_range(start..content_start, "");
        removed = true;
    }

    let mut fences = Vec::new();
    let mut cursor = 0;
    while let Some(open_offset) = display[cursor..].find("```") {
        let start = cursor + open_offset;
        let opening_end = start + 3;
        let Some(close_offset) = display[opening_end..].find("```") else {
            break;
        };
        let end_start = opening_end + close_offset;
        let header_end = display[opening_end..end_start]
            .find('\n')
            .map(|index| opening_end + index);
        let Some(header_end) = header_end else {
            cursor = end_start + 3;
            continue;
        };
        let info = display[opening_end..header_end].trim();
        let info_lower = info.to_ascii_lowercase();
        let is_state = matches!(
            info_lower.as_str(),
            "state" | "status" | "状态栏" | "狀態欄"
        );
        let is_trailing_plain = info.is_empty() && display[end_start + 3..].trim().is_empty();
        if is_state || is_trailing_plain {
            fences.push((
                start,
                end_start + 3,
                display[header_end + 1..end_start].to_owned(),
            ));
        }
        cursor = end_start + 3;
    }
    let fence_blocks: Vec<_> = fences.iter().map(|(_, _, block)| block.clone()).collect();
    for (start, end, _) in fences.into_iter().rev() {
        display.replace_range(start..end, "");
        removed = true;
    }
    blocks.extend(fence_blocks);

    if !removed {
        return StateBlock {
            fields: Vec::new(),
            updates: Vec::new(),
            display: reply.to_owned(),
        };
    }

    let fields = blocks
        .iter()
        .flat_map(|block| parse_indented_fields(block))
        .filter_map(|(mut path, value)| {
            let value = value?;
            if path.len() == 1 {
                path[0] = match path[0].to_ascii_lowercase().as_str() {
                    "time" | "時間" | "时间" => "time".to_owned(),
                    "place" | "location" | "地點" | "地点" => "place".to_owned(),
                    "present" | "在場" | "在场" | "在場人物" | "在场人物" => {
                        "present".to_owned()
                    }
                    _ => path[0].clone(),
                };
            }
            Some((path, value))
        })
        .collect();
    StateBlock {
        fields,
        updates,
        display: display.trim_end().to_owned(),
    }
}

/// 將縮排區塊解析成路徑和值；壞行略過，空值與空字典只標記分支而不終止解析。
pub fn parse_indented_fields(block: &str) -> Vec<(Vec<String>, Option<String>)> {
    let mut fields = Vec::new();
    let mut stack = Vec::<(usize, String)>::new();
    for line in block.lines() {
        let mut indent = 0;
        let mut offset = 0;
        for (index, character) in line.char_indices() {
            match character {
                ' ' => indent += 1,
                '\t' => indent += 4,
                _ => {
                    offset = index;
                    break;
                }
            }
            offset = index + character.len_utf8();
        }
        let mut line = &line[offset..];
        if let Some(stripped) = line.strip_prefix("- ") {
            line = stripped;
            indent += 2;
        }
        line = line.trim_start_matches(['#', '*', '+', '>']).trim_start();
        if line.is_empty() {
            continue;
        }
        let Some((index, separator)) = line
            .char_indices()
            .find(|(_, character)| matches!(character, ':' | '：'))
        else {
            continue;
        };
        let key = line[..index].trim();
        if key.is_empty() {
            continue;
        }
        let mut value = line[index + separator.len_utf8()..].trim();
        if value.len() >= 2
            && ((value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\'')))
        {
            value = &value[1..value.len() - 1];
        }
        while stack
            .last()
            .is_some_and(|(parent_indent, _)| *parent_indent >= indent)
        {
            stack.pop();
        }
        let mut path: Vec<String> = stack.iter().map(|(_, parent)| parent.clone()).collect();
        path.push(key.to_owned());
        if value.is_empty() || matches!(value, "{}" | "{ }") {
            fields.push((path, None));
            stack.push((indent, key.to_owned()));
            continue;
        }
        fields.push((path, Some(value.to_owned())));
    }
    fields
}

/// 從 GM 點名回覆解析出角色名（或玩家代號）。
/// 先試整句精確比對；模型多話時退回「回覆中最先出現的候選名」。
/// GM 是語言模型，輪到玩家時可能吐代號、也可能吐「玩家」或玩家卡上的名字，全部對回代號；
/// NPC 名字排在前面，撞名時算 NPC。
pub fn pick_speaker(reply: &str, roster: &[String], player_name: Option<&str>) -> Option<String> {
    let mut player_words = vec![PLAYER_SENTINEL, "玩家", "Player"];
    player_words.extend(player_name);
    let mut candidates: Vec<&str> = roster.iter().map(String::as_str).collect();
    candidates.extend(&player_words);

    let as_speaker = |name: &str| {
        if player_words.contains(&name) {
            PLAYER_SENTINEL.to_owned()
        } else {
            name.to_owned()
        }
    };
    let trimmed = reply.trim().trim_matches(|character: char| {
        character.is_whitespace() || "「」『』。．：:，,！!？?".contains(character)
    });
    if let Some(hit) = candidates.iter().find(|name| trimmed == **name) {
        return Some(as_speaker(hit));
    }
    candidates
        .iter()
        .filter_map(|name| reply.find(*name).map(|position| (position, *name)))
        .min_by_key(|(position, _)| *position)
        .map(|(_, name)| as_speaker(name))
}

/// 使用者語系：preferences.language，預設 zh-TW；決定 system prompt 注入哪份語言規範
pub fn ui_language(config: &AppConfig) -> String {
    config
        .preferences
        .get("language")
        .and_then(|value| value.as_str())
        .unwrap_or("zh-TW")
        .to_owned()
}

/// GM 檔位：preferences.gm_tier，預設 best（GM 需掌握整體資訊，NewPlan §6.3）
pub fn gm_tier(config: &AppConfig) -> Tier {
    config
        .preferences
        .get("gm_tier")
        .and_then(|value| value.as_str())
        .and_then(|value| Tier::parse(value).ok())
        .unwrap_or(Tier::Best)
}

/// 重構展開檔位：展開／重寫照盤點規格產出，下放 balanced 省費（survey 留 GM 檔）；
/// API 模式未設 balanced 模型時退 GM 檔讓按鈕照常能用（同 translate_opening 慣例），
/// CLI 檔位一律有內建對應不用退。
pub fn refactor_expand_tier(config: &AppConfig, transport_kind: &str) -> Tier {
    if transport_kind == "api" && resolve_model(Tier::Balanced, config).is_err() {
        gm_tier(config)
    } else {
        Tier::Balanced
    }
}

/// 檔位→模型解析。模型 id 一律來自設定檔（config.tier_models），程式不內建。
pub fn resolve_model(tier: Tier, config: &AppConfig) -> Result<String, String> {
    let key = tier.as_str();
    config
        .tier_models
        .get(key)
        .cloned()
        .filter(|model| !model.is_empty())
        .ok_or_else(|| format!("尚未設定「{key}」檔位對應的模型，請先到設定填寫"))
}

/// 開場白翻譯的檔位挑選器要顯示的「這一檔實際會叫哪個模型」。解析與 `stream_via_transport`
/// 同源：tier_models 有覆寫就是覆寫值，沒有才是 CLI 內建對應——前端自己拼會拼錯（同樣是
/// 「低」檔，設了 claude:fast 的機器跑 claude-haiku-4-5，沒設的跑別名 haiku）。
/// 文案留給前端組：model=None 代表走 CLI 預設模型，後端不吐中文。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TierModel {
    /// 被問的檔位
    pub tier: String,
    /// 實際生效的檔位：API 模式該檔位沒設模型時會退 GM 檔（translate_opening 既有慣例）
    pub effective_tier: String,
    /// 實際送出的模型 id；None＝用 CLI 預設模型（codex／agy／grok 未覆寫時）
    pub model: Option<String>,
    /// codex 專用：檔位映射到的 reasoning effort，其他傳輸層為 None
    pub effort: Option<String>,
}

pub fn tier_model(config: &AppConfig, transport_kind: &str, tier: Tier) -> TierModel {
    if transport_kind == "api" {
        let (effective, model) = match resolve_model(tier, config) {
            Ok(model) => (tier, Some(model)),
            // 該檔沒設就退 GM 檔；GM 檔也沒設時 model 留 None，前端顯示「未設定」
            Err(_) => {
                let fallback = gm_tier(config);
                (fallback, resolve_model(fallback, config).ok())
            }
        };
        return TierModel {
            tier: tier.as_str().to_owned(),
            effective_tier: effective.as_str().to_owned(),
            model,
            effort: None,
        };
    }
    let override_model =
        crate::cli::tier_override(&config.tier_models, transport_kind, tier).map(str::to_owned);
    let model = match transport_kind {
        // claude 未覆寫時有內建別名，永遠有值
        "claude" => Some(override_model.unwrap_or_else(|| crate::cli::claude_model_for(tier).to_owned())),
        _ => override_model,
    };
    TierModel {
        tier: tier.as_str().to_owned(),
        effective_tier: tier.as_str().to_owned(),
        model,
        effort: (transport_kind == "codex").then(|| crate::cli::codex_effort_for(tier).to_owned()),
    }
}

pub fn base_url(config: &AppConfig) -> String {
    config
        .preferences
        .get("base_url")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim_end_matches('/').to_owned())
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_owned())
}

/// SSE 逐塊解析器。以位元組緩衝避免 UTF-8 字元被 chunk 邊界切斷。
#[derive(Default)]
pub struct SseParser {
    buffer: Vec<u8>,
}

impl SseParser {
    /// 餵入一塊原始位元組，回傳所有完整行的 `data:` 承載內容。
    pub fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buffer.extend_from_slice(chunk);
        let mut payloads = Vec::new();
        while let Some(index) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.buffer.drain(..=index).collect();
            let line = String::from_utf8_lossy(&line);
            let line = line.trim_end_matches(['\n', '\r']);
            if let Some(payload) = line.strip_prefix("data:") {
                payloads.push(payload.trim_start().to_owned());
            }
            // 其餘：空行（事件分隔）與 ": comment"（OpenRouter 的處理中心跳）一律忽略
        }
        payloads
    }
}

/// 一次呼叫的用量（prompt-cache-optimization C）。API 走 OpenRouter usage accounting，
/// CLI 走各家收尾事件（見 cli::parse_*_usage）。
/// prompt_tokens 一律是「總輸入」（含快取部分），各家語意差異在抽取時就換算掉。
/// cached_tokens／created_tokens 是 Option，`None` 與 `Some(0)` 是**兩件事**：
/// None＝這條路沒回報快取欄位（量不到，不能宣稱沒命中）；Some(0)＝量到了，這輪沒中。
/// 兩者曾被 `unwrap_or(0)` 壓成同一個 0，額度分頁因此對 API 路顯示假的 0.0%
/// （2026-08-21 取證，見 .ai/plans/api-cache-visibility.md）。
/// created_tokens（寫入快取）是診斷關鍵：命中 0 時，它 >0 代表「有建但沒讀到」
/// （前綴變了或過期），=0 代表「根本沒建快取」；回報寫入數的來源才有值。
/// output_tokens 與 cost_usd 供額度分頁算花費；只有 claude 直接回報金額，其餘為 None。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PromptCacheUsage {
    pub prompt_tokens: u64,
    pub cached_tokens: Option<u64>,
    pub created_tokens: Option<u64>,
    pub output_tokens: u64,
    pub cost_usd: Option<f64>,
}

impl PromptCacheUsage {
    /// 讀自快取的輸入佔總輸入的百分比。沒回報快取欄位＝None（前端顯示「—」），
    /// 不是 0——「量不到」與「沒中」混講會讓玩家去修一個不存在的問題。
    pub fn hit_rate(&self) -> Option<f64> {
        match self.cached_tokens {
            Some(cached) if self.prompt_tokens > 0 => {
                Some(cached as f64 * 100.0 / self.prompt_tokens as f64)
            }
            Some(_) => Some(0.0),
            None => None,
        }
    }

    /// 這條路回不回報快取欄位。log 與報表據此把「量不到」與「沒中」分開。
    pub fn reported(&self) -> bool {
        self.cached_tokens.is_some()
    }
}

/// 診斷輸出用：沒回報就印「—」，不印 0。
pub(crate) fn describe(tokens: Option<u64>) -> String {
    tokens.map_or_else(|| "—".to_owned(), |value| value.to_string())
}

/// 快取欄位在各家 OpenAI-compatible 端點叫不同名字，有哪組抓哪組（回傳 `(讀, 寫)`）。
/// 一組都沒有＝這條路不回報，回 `(None, None)`——不可退成 0（那正是本案根因）。
/// 只認實際會走到這條路的三組：中轉站照抄上游 schema，光認 OpenRouter 那組不夠。
fn cache_tokens(usage: &serde_json::Value) -> (Option<u64>, Option<u64>) {
    let field = |value: &serde_json::Value, key: &str| value.get(key).and_then(|v| v.as_u64());
    let details = usage.get("prompt_tokens_details");
    let nested = |key: &str| details.and_then(|details| field(details, key));
    // 讀與寫各自挑第一個有值的來源：整組提前返回會讓「details 只有寫入數」的回應
    // 遮蔽掉同一包裡的 prompt_cache_hit_tokens（Sol 驗收 2026-08-21）。
    // 順序＝OpenRouter（usage accounting）→ DeepSeek 原生（中轉站照抄這組）→ Anthropic 原生。
    let read = nested("cached_tokens")
        .or_else(|| field(usage, "prompt_cache_hit_tokens"))
        .or_else(|| field(usage, "cache_read_input_tokens"));
    let write = nested("cache_write_tokens")
        .or_else(|| field(usage, "cache_creation_input_tokens")); // DeepSeek 不回寫入數
    (read, write)
}

/// 從一則 SSE payload 取出 usage 統計；增量塊的 `"usage": null` 與缺欄位一律回 None。
pub fn extract_usage(payload: &str) -> Option<PromptCacheUsage> {
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;
    let usage = value.get("usage")?;
    let prompt_tokens = usage.get("prompt_tokens")?.as_u64()?;
    let (cached_tokens, created_tokens) = cache_tokens(usage);
    Some(PromptCacheUsage {
        prompt_tokens,
        cached_tokens,
        created_tokens,
        output_tokens: usage
            .get("completion_tokens")
            .and_then(|tokens| tokens.as_u64())
            .unwrap_or(0),
        cost_usd: None,
    })
}

/// Anthropic 系模型走顯式快取（prompt-cache-optimization B）：未標 cache_control＝完全不快取。
/// content 轉 multipart 陣列（斷點只能掛在 content 分段上），在穩定前綴尾標
/// `cache_control: {"type": "ephemeral"}`——具體是 system（角色卡／world.md／constant 條目，
/// 換卡前不變）與最後一則 assistant（其後只剩會變動的東西：可能被 push_merged 續寫的
/// 最後一則 user、每輪翻動的動態塊、導演指示）。transcript 逐輪增長，斷點位置跟著前移；
/// Anthropic 查快取時會回看斷點前約 20 個 content block，前一輪寫下的快取點仍在回看範圍內，
/// 逐輪增量命中。斷點上限 4 個，這裡用 2 個。
fn anthropic_messages(messages: &[ChatMessage]) -> serde_json::Value {
    let last_assistant = messages
        .iter()
        .rposition(|message| message.role == "assistant");
    let entries = messages
        .iter()
        .enumerate()
        .map(|(index, message)| {
            let mut part = serde_json::json!({ "type": "text", "text": message.content });
            if message.role == "system" || Some(index) == last_assistant {
                part["cache_control"] = serde_json::json!({ "type": "ephemeral" });
            }
            serde_json::json!({ "role": message.role, "content": [part] })
        })
        .collect();
    serde_json::Value::Array(entries)
}

/// chat/completions 請求本體。曾對 OpenRouter 端點附掛 `usage:{include:true}`，
/// 官方已將該參數（與 `stream_options:{include_usage:true}`）標為 deprecated 且無作用——
/// 完整 usage 一律自動回在尾塊，帶著只會讓嚴格的端點（OpenAI 官方）拒絕請求。
/// anthropic/ 系模型另走顯式快取斷點（見 anthropic_messages）；不適用時請求形狀維持素樸。
fn chat_request_body(model: &str, messages: &[ChatMessage]) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": true,
    });
    if model.starts_with("anthropic/") {
        body["messages"] = anthropic_messages(messages);
    }
    body
}

/// 從一則 SSE payload 取出增量文字；非增量塊（usage、空 choices）回 None。
pub fn extract_delta(payload: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;
    let text = value
        .get("choices")?
        .get(0)?
        .get("delta")?
        .get("content")?
        .as_str()?;
    if text.is_empty() {
        None
    } else {
        Some(text.to_owned())
    }
}

/// 串流全程累積的收工訊號。判「這次呼叫算不算成功」靠的是 content 以外的欄位：
/// 供應商中途塞的 error 塊、finish_reason、有沒有見到 [DONE]。
/// 這些現在全被丟掉，於是「思考完但零內容」會冒充成功（見 .ai/plans/stream-failure-visible.md）。
#[derive(Default)]
pub struct StreamOutcome {
    /// 供應商中途送的錯誤原話（頂層 error，與 finish_reason="error" 同時出現）
    pub error: Option<String>,
    /// choices[0].finish_reason，取最後一則有值的
    pub finish_reason: Option<String>,
    /// usage.completion_tokens_details.reasoning_tokens，只進錯誤診斷小字
    pub reasoning_tokens: Option<u64>,
    /// 有沒有收到 [DONE]：沒有就 EOF＝串流被截斷
    pub saw_done: bool,
}

impl StreamOutcome {
    /// 吸收一則 payload 的訊號。error 與 finish_reason 都取最後一則有值的
    /// （增量塊的 finish_reason 是 null，真正的收尾原因在最後一塊）。
    pub fn absorb(&mut self, payload: &str) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
            return;
        };
        if let Some(error) = value.get("error").filter(|error| !error.is_null()) {
            // message 缺了或不是字串就序列化整包——供應商的錯誤不能靜默吞掉
            self.error = Some(
                error
                    .get("message")
                    .and_then(|message| message.as_str())
                    .filter(|message| !message.trim().is_empty())
                    .map(str::to_owned)
                    .unwrap_or_else(|| error.to_string()),
            );
        }
        if let Some(reason) = value
            .get("choices")
            .and_then(|choices| choices.get(0))
            .and_then(|choice| choice.get("finish_reason"))
            .and_then(|reason| reason.as_str())
        {
            self.finish_reason = Some(reason.to_owned());
        }
        if let Some(tokens) = value
            .get("usage")
            .and_then(|usage| usage.get("completion_tokens_details"))
            .and_then(|details| details.get("reasoning_tokens"))
            .and_then(|tokens| tokens.as_u64())
        {
            self.reasoning_tokens = Some(tokens);
        }
    }

    /// 收工判定。優先序固定：供應商原話 → 內容過濾 → 不完整 → 正文空 → 成功。
    /// 順序決定歸類：length 又零正文歸 INCOMPLETE（原因是被截斷），不歸 EMPTY。
    /// 供應商原話不加碼原樣拋——免費層 429 的原話能被 ai-error.ts 既有的額度正則接住，
    /// 玩家看到「額度用完」比看到一句籠統的串流錯誤有用。
    /// 回傳 Some(錯誤字串)＝失敗，None＝成功。
    pub fn failure(&self, text: &str, model: &str) -> Option<String> {
        if let Some(error) = &self.error {
            return Some(error.clone());
        }
        let reason = self.finish_reason.as_deref();
        let diagnosis = format!(
            "model={model} finish_reason={}{}",
            reason.unwrap_or("(無)"),
            self.reasoning_tokens
                .map(|tokens| format!(" reasoning_tokens={tokens}"))
                .unwrap_or_default(),
        );
        match reason {
            Some("content_filter") => Some(format!("AI_CONTENT_FILTERED: {diagnosis}")),
            // stop 以外的收尾原因（length／tool_calls／沒見過的）這個 app 都接不下去
            Some(reason) if reason != "stop" => Some(format!("AI_INCOMPLETE_RESPONSE: {diagnosis}")),
            // 沒收尾原因又沒見到 [DONE]＝串流被中途截斷
            None if !self.saw_done => Some(format!("AI_INCOMPLETE_RESPONSE: {diagnosis}")),
            _ if text.trim().is_empty() => Some(format!("AI_EMPTY_RESPONSE: {diagnosis}")),
            _ => None,
        }
    }
}

/// 非 2xx 的錯誤字串：開頭掛穩定碼給前端分流（比照 AI_EMPTY_RESPONSE 慣例），
/// 後面照舊附人看得懂的狀態與原文。前端只認開頭那個碼、不解析 body——
/// 聚合 router 常把上游錯誤整包塞進 body，body 裡的數字（如轉包的 429）
/// 不該蓋掉真正的 HTTP 狀態。
///
/// 原文留到 2000 字：request id、欄位細節、說明網址常在後段，玩家要拿這串去問供應商。
/// 真的超長才截，並且明講截了——看似完整其實殘缺的 JSON 比明說截斷更難查。
fn http_error(status: reqwest::StatusCode, body: &str) -> String {
    const LIMIT: usize = 2000;
    let kept: String = body.chars().take(LIMIT).collect();
    let cut = if body.chars().nth(LIMIT).is_some() {
        "…（原始回應已截斷）"
    } else {
        ""
    };
    format!(
        "AI_HTTP_STATUS_{}: API 回應 {status}：{kept}{cut}",
        status.as_u16(),
    )
}

/// 單發呼叫 OpenAI-compatible chat/completions（SSE 串流），
/// 每個增量經 on_delta 回傳，結束後回傳完整文字。
/// usage_log 給路徑就把這次呼叫的用量追加成一行 JSONL（見 crate::usage_log）。
pub async fn stream_chat(
    config: &AppConfig,
    model: &str,
    messages: &[ChatMessage],
    usage_log: Option<&std::path::Path>,
    world: Option<&str>,
    mut on_delta: impl FnMut(&str),
) -> DataResult<String> {
    let base = base_url(config);
    let api_key = config
        .api_keys
        .get("openrouter")
        .filter(|key| !key.is_empty());
    if api_key.is_none() && base == DEFAULT_BASE_URL {
        return Err("尚未設定 OpenRouter API key，請先到設定貼上".into());
    }

    let mut request = reqwest::Client::new()
        .post(format!("{base}/chat/completions"))
        .json(&chat_request_body(model, messages));
    if let Some(key) = api_key {
        request = request.bearer_auth(key);
    }

    let response = request.send().await?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(http_error(status, &body).into());
    }

    let mut stream = response.bytes_stream();
    let mut parser = SseParser::default();
    let mut full_text = String::new();
    let mut usage = None;
    let mut outcome = StreamOutcome::default();
    'outer: while let Some(chunk) = stream.next().await {
        for payload in parser.push(&chunk?) {
            if payload == "[DONE]" {
                outcome.saw_done = true;
                break 'outer;
            }
            outcome.absorb(&payload);
            if let Some(parsed) = extract_usage(&payload) {
                usage = Some(parsed);
            }
            if let Some(delta) = extract_delta(&payload) {
                on_delta(&delta);
                full_text.push_str(&delta);
            }
        }
    }
    if let Some(usage) = usage {
        // stderr 一行（終端機啟動時直接看）＋落檔一行（事後隨時查）
        eprintln!(
            "[prompt-cache] transport=api model={model} prompt_tokens={} cached_tokens={} created_tokens={} hit_rate={}",
            usage.prompt_tokens,
            describe(usage.cached_tokens),
            describe(usage.created_tokens),
            usage
                .hit_rate()
                .map_or_else(|| "—（這條路不回報快取）".to_owned(), |rate| format!("{rate:.0}%")),
        );
        if let Some(path) = usage_log {
            crate::usage_log::append_call(path, world, "api", model, None, usage);
        }
    }
    // 用量照記再判成敗：失敗的呼叫一樣燒了 token，額度分頁不能少算這一筆
    if let Some(failure) = outcome.failure(&full_text, model) {
        return Err(failure.into());
    }
    Ok(full_text)
}

/// OpenRouter 專用 Images API（POST {base}/images）；回傳 data URL 或遠端圖片網址。
pub async fn generate_image(config: &AppConfig, prompt: &str) -> Result<String, String> {
    let api_key = config
        .api_keys
        .get("openrouter")
        .map(String::as_str)
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| "尚未設定 OpenRouter API key".to_owned())?;
    let model = config
        .preferences
        .get("image_model")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(DEFAULT_IMAGE_MODEL);
    let response = reqwest::Client::new()
        .post(format!("{}/images", base_url(config)))
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "model": model,
            "prompt": prompt,
            "aspect_ratio": "2:3",
            "resolution": "1K",
        }))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(http_error(status, &body));
    }
    let value: serde_json::Value = response.json().await.map_err(|error| error.to_string())?;
    let image = value.get("data").and_then(|data| data.get(0));
    if let Some(b64) = image
        .and_then(|entry| entry.get("b64_json"))
        .and_then(|value| value.as_str())
    {
        return Ok(format!("data:image/png;base64,{b64}"));
    }
    if let Some(url) = image
        .and_then(|entry| entry.get("url"))
        .and_then(|value| value.as_str())
    {
        if url.starts_with("http") {
            return Ok(url.to_owned());
        }
    }
    Err("模型沒有回傳圖片".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{FieldKind, FieldRule, Tier};

    fn card(id: &str, name: &str, public_md: &str, private_md: &str) -> CharacterCard {
        CharacterCard {
            id: id.to_owned(),
            name: name.to_owned(),
            color: "#336699".to_owned(),
            avatar: "🦊".to_owned(),
            tier: Tier::Balanced,
            show_image: true,
            archived: false,
            gen_prompt: String::new(),
            public_md: public_md.to_owned(),
            private_md: private_md.to_owned(),
        }
    }

    fn event(
        kind: TranscriptKind,
        speaker_id: &str,
        speaker_name: &str,
        text: &str,
    ) -> TranscriptEvent {
        TranscriptEvent {
            raw: None,
            ts: "2026-07-19T12:00:00+08:00".to_owned(),
            speaker_id: speaker_id.to_owned(),
            speaker_name: speaker_name.to_owned(),
            kind,
            text: text.to_owned(),
            state: None,
            gm_only: false,
        }
    }

    fn worldbook_entry(
        uid: u64,
        title: &str,
        keys: &[&str],
        constant: bool,
        order: i64,
        disabled: bool,
        visibility: Visibility,
    ) -> WorldbookEntry {
        WorldbookEntry {
            uid,
            title: title.to_owned(),
            keys: keys.iter().map(|key| (*key).to_owned()).collect(),
            content: format!("{title}內容"),
            constant,
            order,
            disabled,
            visibility,
            is_person: false,
            locked: false,
        }
    }

    /// 有玩家卡時，角色與 GM 都要認得玩家的名字與公開身份（本功能的核心）
    #[test]
    fn player_card_enters_character_and_gm_context() {
        let fox = card("fox-id", "狐狸", "旅店老闆", "通緝犯");
        let player = card("player-id", "阿濤", "遠道而來的商隊護衛", "");

        let character_system = &assemble_messages(
            &fox,
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

    /// 卡片自帶介面時的導演指示：點名世界書那條格式規定的標題，且不再要求舊版的
    /// state 圍欄／下一位點名——那是兩邊指令打架的根因。
    #[test]
    fn card_format_instruction_points_to_worldbook_entry_and_drops_old_format_asks() {
        let zh_with_title = card_format_instruction("zh-TW", Some("回复规则")).content;
        assert!(zh_with_title.contains("回复规则"));
        assert!(!zh_with_title.contains("```state"));
        assert!(!zh_with_title.contains("下一位"));

        let zh_without_title = card_format_instruction("zh-TW", None).content;
        assert!(!zh_without_title.contains("見世界書"));

        let en_with_title = card_format_instruction("en", Some("Response Rules")).content;
        assert!(en_with_title.contains("Response Rules"));
        assert!(!en_with_title.contains("```"));
        assert!(!en_with_title.contains("Next:"));

        let en_without_title = card_format_instruction("en", None).content;
        assert!(!en_without_title.contains("see the worldbook entry"));
    }

    #[test]
    fn extract_next_speaker_strips_trailing_line_and_tolerates_variants() {
        // 標準形：旁白＋狀態欄剝除後尾端剩點名行
        let (name, display) = extract_next_speaker("夜更深了。\n下一位：狐狸");
        assert_eq!(name.as_deref(), Some("狐狸"));
        assert_eq!(display, "夜更深了。");
        // 英文標記與半形冒號、前後空白
        let (name, display) = extract_next_speaker("The night deepens.\nNext:  Fox ");
        assert_eq!(name.as_deref(), Some("Fox"));
        assert_eq!(display, "The night deepens.");
        // 玩家哨兵原文帶回，由 pick_speaker 對回代號
        let (name, _) =
            extract_next_speaker(format!("門開了。\n下一位：{PLAYER_SENTINEL}").as_str());
        assert_eq!(name.as_deref(), Some(PLAYER_SENTINEL));
        // 沒有點名行＝原樣返回；行首是普通英文 Next 不誤判
        let plain = "夜更深了。\nNext, the door opened.";
        assert_eq!(extract_next_speaker(plain), (None, plain.to_owned()));
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

        let character = assemble_messages(
            &fox,
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
    fn st_macros_fall_back_to_localized_player_name_without_player_card() {
        let fox = card("fox-id", "狐狸", "{{user}}", "");

        let zh = assemble_messages(
            &fox,
            None,
            &[],
            &[],
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
        );
        assert!(zh[0]
            .content
            .contains("你的公開設定（其他人也認識的你）\n玩家\n"));

        let en = assemble_messages(
            &fox,
            None,
            &[],
            &[],
            &TableState::default(),
            &Mechanism::default(),
            None,
            "en",
        );
        assert!(en[0]
            .content
            .contains("你的公開設定（其他人也認識的你）\nPlayer\n"));
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
        let character = assemble_messages(
            &fox,
            None,
            &events,
            &[],
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
        );
        assert_eq!(character.len(), 2);
        assert!(character[0].content.contains("## 你的公開設定"));
        assert!(character[0].content.contains("## 你的私有設定"));
        assert!(!character[0].content.contains("同桌的玩家"));
        assert!(!character[0].content.contains("## 你知道的世界情報"));
        assert_eq!(character[1], message("user", "玩家：你好".to_owned()));

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
        let fox_messages = assemble_messages(
            &fox,
            None,
            &[],
            &entries,
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
        );
        let fox_system = &fox_messages[0].content;
        assert!(fox_system.contains("\n## 你知道的世界情報\n"));
        assert!(fox_system.contains("### 公開情報\n公開情報內容\n"));
        assert!(fox_system.contains("### 狐狸情報\n狐狸情報內容\n"));
        assert!(!fox_system.contains("GM 祕密"));
        assert!(!fox_system.contains("騎士情報"));

        let knight = card("knight-id", "騎士", "公開", "私有");
        let knight_system = &assemble_messages(
            &knight,
            None,
            &[],
            &entries,
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
        )[0]
        .content;
        assert!(knight_system.contains("公開情報"));
        assert!(knight_system.contains("騎士情報"));
        assert!(!knight_system.contains("狐狸情報"));

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

    /// 驗收：上下文只含本角色可見內容——含自己的公開＋私有，
    /// 且介面上根本收不到他人的卡或 world.md。
    #[test]
    fn context_contains_own_card_only_and_public_transcript() {
        let fox = card("fox-id", "狐狸", "旅店老闆，笑口常開", "其實是通緝犯");
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
        let messages = assemble_messages(
            &fox,
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
        assert!(system.content.contains("旅店老闆"));
        assert!(system.content.contains("其實是通緝犯"));

        // 他人（騎士）的私有設定不存在於任何訊息——組裝介面收不到它
        let joined: String = messages.iter().map(|m| m.content.as_str()).collect();
        assert!(!joined.contains("world"));
        assert!(joined.contains("（旁白）夜幕低垂"));
        assert!(joined.contains("玩家：老闆，來杯麥酒"));
        assert!(joined.contains("騎士：我在找一名通緝犯。"));
        assert!(joined.contains("（系統）騎士 加入本桌"));
    }

    #[test]
    fn own_dialogue_becomes_assistant_and_adjacent_roles_merge() {
        let fox = card("fox-id", "狐狸", "公開", "");
        let events = [
            event(TranscriptKind::Player, "", "玩家", "第一句"),
            event(TranscriptKind::Narration, "", "GM", "旁白一句"),
            event(TranscriptKind::Dialogue, "fox-id", "狐狸", "我的回答"),
            event(TranscriptKind::Player, "", "玩家", "第二句"),
        ];
        let messages = assemble_messages(
            &fox,
            None,
            &events,
            &[],
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
        );
        let roles: Vec<&str> = messages.iter().map(|m| m.role.as_str()).collect();
        assert_eq!(roles, ["system", "user", "assistant", "user"]);
        assert_eq!(messages[1].content, "玩家：第一句\n（旁白）旁白一句");
        assert_eq!(messages[2].content, "我的回答");
        // 空的私有節不產生私有段落
        assert!(!messages[0].content.contains("私有設定"));
    }

    /// 測試清單 #14：同名兩角色各自只把自己的台詞當 assistant——用 speaker_id 判斷，
    /// 不會因為顯示名相同就把對方的台詞誤認成自己說的
    #[test]
    fn assemble_messages_uses_speaker_id_not_name_for_same_named_characters() {
        let first = card("id-a", "重名", "第一位", "");
        let second_id = "id-b";
        let events = [
            event(TranscriptKind::Dialogue, "id-a", "重名", "我是第一位"),
            event(TranscriptKind::Dialogue, second_id, "重名", "我是第二位"),
        ];

        let first_messages = assemble_messages(
            &first,
            None,
            &events,
            &[],
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
        );
        let roles: Vec<&str> = first_messages.iter().map(|m| m.role.as_str()).collect();
        // 自己的那句是 assistant，對方同名的那句仍是 user（不會相鄰合併成一則）
        assert_eq!(roles, ["system", "assistant", "user"]);
        assert_eq!(first_messages[1].content, "我是第一位");
        assert_eq!(first_messages[2].content, "重名：我是第二位");

        let second = card(second_id, "重名", "第二位", "");
        let second_messages = assemble_messages(
            &second,
            None,
            &events,
            &[],
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
        );
        let roles: Vec<&str> = second_messages.iter().map(|m| m.role.as_str()).collect();
        assert_eq!(roles, ["system", "user", "assistant"]);
        assert_eq!(second_messages[1].content, "重名：我是第一位");
        assert_eq!(second_messages[2].content, "我是第二位");
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
        let zh = assemble_messages(
            &fox,
            None,
            &[],
            &[],
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
        );
        assert!(zh[0].content.contains("繁體中文"));
        let en = assemble_messages(
            &fox,
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
            let messages = assemble_messages(
                &fox,
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

    /// 驗收：換幕順手取幕名——有標題行／無標題行／en 前綴，都不能報錯
    #[test]
    fn extract_scene_title_reads_zh_and_en_prefixes_and_falls_back_without_one() {
        let (title, rest) = extract_scene_title("標題：酒館夜話\n\n地點與時間：酒館");
        assert_eq!(title.as_deref(), Some("酒館夜話"));
        assert_eq!(rest, "地點與時間：酒館");

        let (title_en, rest_en) = extract_scene_title("Title: Tavern Talk\n\nLocation: the tavern");
        assert_eq!(title_en.as_deref(), Some("Tavern Talk"));
        assert_eq!(rest_en, "Location: the tavern");

        // 大小寫寬鬆
        let (title_mixed, _) = extract_scene_title("title: Mixed Case\n\nbody");
        assert_eq!(title_mixed.as_deref(), Some("Mixed Case"));

        // 沒有標題行：整段原文當摘要，不報錯
        let (none_title, whole) = extract_scene_title("地點與時間：酒館\n關鍵事件：無");
        assert_eq!(none_title, None);
        assert_eq!(whole, "地點與時間：酒館\n關鍵事件：無");
    }

    #[test]
    fn pick_speaker_handles_exact_verbose_and_player_sentinel() {
        let roster = vec!["狐狸".to_owned(), "騎士".to_owned()];
        assert_eq!(pick_speaker("狐狸", &roster, None).unwrap(), "狐狸");
        assert_eq!(pick_speaker("「騎士」。", &roster, None).unwrap(), "騎士");
        assert_eq!(
            pick_speaker("玩家", &roster, None).unwrap(),
            PLAYER_SENTINEL
        );
        // 輪到玩家時 GM 怎麼講都算：代號本身、「玩家」、英文 Player、玩家卡上的名字
        for reply in [PLAYER_SENTINEL, "玩家", "Player", "阿濤"] {
            assert_eq!(
                pick_speaker(reply, &roster, Some("阿濤")).unwrap(),
                PLAYER_SENTINEL
            );
        }
        // 模型多話：取回覆中最先出現的候選名
        assert_eq!(
            pick_speaker("下一位應該由騎士發言，逼問狐狸。", &roster, None).unwrap(),
            "騎士"
        );
        assert_eq!(pick_speaker("酒保", &roster, None), None);
    }

    #[test]
    fn gm_tier_defaults_to_best_and_reads_preference() {
        let mut config = AppConfig::default();
        assert_eq!(gm_tier(&config), Tier::Best);
        config.preferences.insert(
            "gm_tier".to_owned(),
            serde_json::Value::String("fast".to_owned()),
        );
        assert_eq!(gm_tier(&config), Tier::Fast);
        // 亂值退回預設 best
        config.preferences.insert(
            "gm_tier".to_owned(),
            serde_json::Value::String("impossible".to_owned()),
        );
        assert_eq!(gm_tier(&config), Tier::Best);
    }

    #[test]
    fn refactor_expand_tier_falls_back_only_on_api_without_balanced_model() {
        let mut config = AppConfig::default();
        // API 模式未設 balanced 模型 → 退 GM 檔（預設 best）
        assert_eq!(refactor_expand_tier(&config, "api"), Tier::Best);
        // CLI 模式一律 balanced（CLI 有內建檔位對應，不用退）
        assert_eq!(refactor_expand_tier(&config, "claude"), Tier::Balanced);
        // API 模式設了 balanced 模型 → balanced
        config
            .tier_models
            .insert("balanced".to_owned(), "vendor/mid-model".to_owned());
        assert_eq!(refactor_expand_tier(&config, "api"), Tier::Balanced);
    }

    #[test]
    fn tier_model_matches_what_actually_gets_sent() {
        let mut config = AppConfig::default();
        // claude 未覆寫：內建別名，永遠有值
        let fast = tier_model(&config, "claude", Tier::Fast);
        assert_eq!(fast.model.as_deref(), Some("haiku"));
        assert_eq!(fast.effective_tier, "fast");
        assert!(fast.effort.is_none());
        // claude 有覆寫：顯示覆寫後的實際 id（同樣是「低」檔，兩台機器送的不一樣）
        config
            .tier_models
            .insert("claude:fast".to_owned(), "claude-haiku-4-5".to_owned());
        assert_eq!(
            tier_model(&config, "claude", Tier::Fast).model.as_deref(),
            Some("claude-haiku-4-5")
        );
        // codex 未覆寫：走 CLI 預設模型（model=None），檔位落在 reasoning effort
        let codex = tier_model(&config, "codex", Tier::Best);
        assert_eq!(codex.model, None);
        assert_eq!(codex.effort.as_deref(), Some("high"));
        // API 模式該檔沒設模型 → 照實反映會退到 GM 檔
        config
            .tier_models
            .insert("best".to_owned(), "vendor/big-model".to_owned());
        let api_fast = tier_model(&config, "api", Tier::Fast);
        assert_eq!(api_fast.effective_tier, "best");
        assert_eq!(api_fast.model.as_deref(), Some("vendor/big-model"));
    }

    #[test]
    fn resolve_model_reads_config() {
        let mut config = AppConfig::default();
        assert!(resolve_model(Tier::Best, &config).is_err());

        config
            .tier_models
            .insert("best".to_owned(), "vendor/big-model".to_owned());
        config
            .tier_models
            .insert("balanced".to_owned(), "vendor/mid-model".to_owned());
        config
            .tier_models
            .insert("fast".to_owned(), "vendor/small-model".to_owned());
        assert_eq!(
            resolve_model(Tier::Best, &config).unwrap(),
            "vendor/big-model"
        );
        assert_eq!(
            resolve_model(Tier::Balanced, &config).unwrap(),
            "vendor/mid-model"
        );
        assert_eq!(
            resolve_model(Tier::Fast, &config).unwrap(),
            "vendor/small-model"
        );
    }

    #[test]
    fn base_url_defaults_and_trims_trailing_slash() {
        let mut config = AppConfig::default();
        assert_eq!(base_url(&config), DEFAULT_BASE_URL);
        config.preferences.insert(
            "base_url".to_owned(),
            serde_json::Value::String("http://localhost:11434/v1/".to_owned()),
        );
        assert_eq!(base_url(&config), "http://localhost:11434/v1");
    }

    #[test]
    fn sse_parser_handles_split_chunks_comments_and_multibyte_boundaries() {
        let mut parser = SseParser::default();
        assert!(parser.push(b": OPENROUTER PROCESSING\n\n").is_empty());

        // 一則 payload 被切成兩塊，且切點落在多位元組字元中間
        let payload = r#"data: {"choices":[{"delta":{"content":"你好"}}]}"#;
        let bytes = payload.as_bytes();
        let split = payload.find("你").unwrap() + 1; // 「你」的第 2 個位元組處
        let mut collected = parser.push(&bytes[..split]);
        assert!(collected.is_empty());
        collected.extend(parser.push(&bytes[split..]));
        collected.extend(parser.push(b"\ndata: [DONE]\n"));
        assert_eq!(collected.len(), 2);
        assert_eq!(extract_delta(&collected[0]).unwrap(), "你好");
        assert_eq!(collected[1], "[DONE]");
    }

    #[tokio::test]
    async fn stream_chat_streams_deltas_from_mock_server_and_requires_key_for_openrouter() {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0u8; 4096];
            let _ = socket.read(&mut request);
            let body = concat!(
                ": OPENROUTER PROCESSING\n\n",
                "data: {\"choices\":[{\"delta\":{\"content\":\"你\"}}]}\n\n",
                "data: {\"choices\":[{\"delta\":{\"content\":\"好\"}}]}\n\n",
                "data: [DONE]\n\n",
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).unwrap();
        });

        // 預設 OpenRouter endpoint 且沒 key：呼叫前就擋下
        let mut config = AppConfig::default();
        let messages = [message("user", "嗨".to_owned())];
        let error = stream_chat(&config, "test/model", &messages, None, None, |_| {})
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("API key"), "{error}");

        // 自訂 base URL（無 key）：走 mock server，增量與全文一致
        config.preferences.insert(
            "base_url".to_owned(),
            serde_json::Value::String(format!("http://{address}")),
        );
        let mut deltas = Vec::new();
        let full = stream_chat(&config, "test/model", &messages, None, None, |delta| {
            deltas.push(delta.to_owned());
        })
        .await
        .unwrap();
        assert_eq!(full, "你好");
        assert_eq!(deltas, ["你", "好"]);
    }

    /// 收工判定的優先序（stream-failure-visible）：實測 2026-08-21 免費 DeepSeek
    /// 「思考完但零內容」時串流是正常走完 [DONE] 的，靠 content 判不出失敗。
    #[test]
    fn stream_outcome_ranks_failures_by_priority() {
        // 供應商中途 error：原話原樣拋，不加碼——交給 ai-error.ts 既有的額度正則分流
        let mut outcome = StreamOutcome::default();
        outcome.absorb(
            r#"{"error":{"code":429,"message":"Rate limit exceeded"},"choices":[{"delta":{"content":""},"finish_reason":"error"}]}"#,
        );
        assert_eq!(
            outcome.failure("", "test/model").unwrap(),
            "Rate limit exceeded"
        );

        // error.message 不是字串：整包序列化，不靜默吞掉
        let mut outcome = StreamOutcome::default();
        outcome.absorb(r#"{"error":{"code":500}}"#);
        assert!(outcome.failure("", "test/model").unwrap().contains("500"));

        // error.message 是空字串：等同缺失，一樣回退整包——Err("") 在前端等於什麼都沒顯示
        let mut outcome = StreamOutcome::default();
        outcome.absorb(r#"{"error":{"code":500,"message":""}}"#);
        let failure = outcome.failure("", "test/model").unwrap();
        assert!(!failure.trim().is_empty() && failure.contains("500"), "{failure}");

        // 有正文＋[DONE]＋供應商沒給 finish_reason＝成功：共用 OpenAI-compatible 路徑
        // 不強迫所有供應商都回收尾原因，[DONE] 本身就是完成訊號
        let mut outcome = StreamOutcome::default();
        outcome.absorb(r#"{"choices":[{"delta":{"content":"旁白"}}]}"#);
        outcome.saw_done = true;
        assert_eq!(outcome.failure("旁白", "test/model"), None);

        // content_filter 有自己的碼（玩家的下一步是換說法，不是重試）
        let mut outcome = StreamOutcome::default();
        outcome.absorb(r#"{"choices":[{"finish_reason":"content_filter"}]}"#);
        outcome.saw_done = true;
        assert!(outcome
            .failure("", "test/model")
            .unwrap()
            .starts_with("AI_CONTENT_FILTERED:"));

        // length 又零正文：歸 INCOMPLETE 不歸 EMPTY——原因是被截斷，不是模型沒話說
        let mut outcome = StreamOutcome::default();
        outcome.absorb(r#"{"choices":[{"finish_reason":"length"}]}"#);
        outcome.absorb(r#"{"usage":{"completion_tokens_details":{"reasoning_tokens":4437}}}"#);
        outcome.saw_done = true;
        let failure = outcome.failure("", "test/model").unwrap();
        assert!(failure.starts_with("AI_INCOMPLETE_RESPONSE:"), "{failure}");
        assert!(failure.contains("reasoning_tokens=4437"), "{failure}");

        // length 但正文非空：第一版一樣當失敗（共用層不知道半截內容安不安全）
        let mut outcome = StreamOutcome::default();
        outcome.absorb(r#"{"choices":[{"finish_reason":"length"}]}"#);
        outcome.saw_done = true;
        assert!(outcome
            .failure("半截旁白", "test/model")
            .unwrap()
            .starts_with("AI_INCOMPLETE_RESPONSE:"));

        // 沒收尾原因又沒見到 [DONE]＝串流被截斷
        let outcome = StreamOutcome::default();
        assert!(outcome
            .failure("有字", "test/model")
            .unwrap()
            .starts_with("AI_INCOMPLETE_RESPONSE:"));

        // 正常收尾但正文只有空白：這就是實測那兩次的形狀
        let mut outcome = StreamOutcome::default();
        outcome.absorb(r#"{"choices":[{"finish_reason":"stop"}]}"#);
        outcome.saw_done = true;
        assert!(outcome
            .failure(" \n ", "test/model")
            .unwrap()
            .starts_with("AI_EMPTY_RESPONSE:"));

        // 正常收尾且有正文＝成功
        let mut outcome = StreamOutcome::default();
        outcome.absorb(r#"{"choices":[{"finish_reason":"stop"}]}"#);
        outcome.saw_done = true;
        assert_eq!(outcome.failure("旁白", "test/model"), None);
    }

    /// 非 2xx 一律掛開頭碼給前端分流：碼取自真正的 HTTP 狀態，
    /// 不受 body 裡那些上游轉包的數字影響（今天實測的 503 body 就長這樣）
    #[test]
    fn http_error_prefixes_real_status_not_body_digits() {
        let real = r#"{"error":{"message":"openai_error","type":"bad_response_status_code"},"id":157975}"#;
        let text = http_error(reqwest::StatusCode::SERVICE_UNAVAILABLE, real);
        assert!(text.starts_with("AI_HTTP_STATUS_503: "), "{text}");
        assert!(text.contains("bad_response_status_code"), "{text}");

        // body 自稱 429，狀態是 503：碼必須跟著狀態走
        let lying = r#"{"error":{"message":"upstream said 429 rate limit"}}"#;
        let text = http_error(reqwest::StatusCode::SERVICE_UNAVAILABLE, lying);
        assert!(text.starts_with("AI_HTTP_STATUS_503: "), "{text}");

        // 沒超過上限就不留截斷字樣（玩家複製到的是完整原文）
        let short = "毒".repeat(2000);
        let text = http_error(reqwest::StatusCode::BAD_GATEWAY, &short);
        assert_eq!(text.matches('毒').count(), 2000);
        assert!(!text.contains("已截斷"), "{text}");

        // 超長才截，且一定標記出來：看似完整其實殘缺的 JSON 比明說截斷更難查
        let long = "毒".repeat(2500);
        let text = http_error(reqwest::StatusCode::BAD_GATEWAY, &long);
        assert!(text.starts_with("AI_HTTP_STATUS_502: "), "{text}");
        assert_eq!(text.matches('毒').count(), 2000);
        assert!(text.ends_with("…（原始回應已截斷）"), "{text}");
    }

    /// 增量塊的 finish_reason 是 null，真正的收尾原因在最後一塊：取最後一則有值的
    #[test]
    fn stream_outcome_absorbs_last_finish_reason_and_ignores_nulls() {
        let mut outcome = StreamOutcome::default();
        outcome.absorb(r#"{"choices":[{"delta":{"content":"嗨"},"finish_reason":null}]}"#);
        assert_eq!(outcome.finish_reason, None);
        outcome.absorb(r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#);
        assert_eq!(outcome.finish_reason.as_deref(), Some("stop"));
        // 壞掉的 JSON 不該讓整條串流爆掉
        outcome.absorb("{不是 JSON");
        assert_eq!(outcome.finish_reason.as_deref(), Some("stop"));
    }

    /// 端到端：串流正常走完 [DONE] 但一個字都沒有，現在回 Err 而不是 Ok("")
    #[tokio::test]
    async fn stream_chat_fails_when_stream_completes_with_no_content() {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0u8; 4096];
            let _ = socket.read(&mut request);
            let body = concat!(
                ": OPENROUTER PROCESSING\n\n",
                "data: {\"choices\":[{\"delta\":{\"content\":\"\"},\"finish_reason\":\"stop\"}]}\n\n",
                "data: [DONE]\n\n",
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).unwrap();
        });

        let mut config = AppConfig::default();
        config.preferences.insert(
            "base_url".to_owned(),
            serde_json::Value::String(format!("http://{address}")),
        );
        let messages = [message("user", "嗨".to_owned())];
        let error = stream_chat(&config, "test/model", &messages, None, None, |_| {})
            .await
            .unwrap_err()
            .to_string();
        assert!(error.starts_with("AI_EMPTY_RESPONSE:"), "{error}");
    }

    #[tokio::test]
    async fn generate_image_returns_data_url_from_b64_json() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0u8; 4096];
            let _ = socket.read(&mut request);
            let body = r#"{"data":[{"b64_json":"cG5n"}]}"#;
            let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            socket.write_all(response.as_bytes()).unwrap();
        });
        let mut config = AppConfig::default();
        config
            .api_keys
            .insert("openrouter".to_owned(), "key".to_owned());
        config.preferences.insert(
            "base_url".to_owned(),
            serde_json::Value::String(format!("http://{address}")),
        );
        assert_eq!(
            generate_image(&config, "畫一位角色").await.unwrap(),
            "data:image/png;base64,cG5n"
        );
    }

    #[tokio::test]
    async fn generate_image_rejects_empty_data() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0u8; 4096];
            let _ = socket.read(&mut request);
            let body = r#"{"data":[]}"#;
            let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            socket.write_all(response.as_bytes()).unwrap();
        });
        let mut config = AppConfig::default();
        config
            .api_keys
            .insert("openrouter".to_owned(), "key".to_owned());
        config.preferences.insert(
            "base_url".to_owned(),
            serde_json::Value::String(format!("http://{address}")),
        );
        assert_eq!(
            generate_image(&config, "畫一位角色").await.unwrap_err(),
            "模型沒有回傳圖片"
        );
    }

    /// 請求本體維持素樸：usage accounting 參數已被 OpenRouter 官方廢止（帶了無效，
    /// 嚴格端點還會拒絕），一個多餘的鍵都不能有。
    #[test]
    fn chat_request_body_stays_bytewise_identical_for_plain_models() {
        let messages = [message("user", "嗨".to_owned())];
        let plain = chat_request_body("test/model", &messages);
        assert_eq!(
            plain,
            serde_json::json!({
                "model": "test/model",
                "messages": [{"role": "user", "content": "嗨"}],
                "stream": true,
            })
        );
        assert!(plain.get("usage").is_none());
    }

    /// Claude 顯式斷點（prompt-cache-optimization B）：anthropic/ 系模型 content 轉 multipart，
    /// 斷點恰好兩個——system 與最後一則 assistant；其他模型維持純字串 content。
    #[test]
    fn anthropic_models_get_multipart_content_with_two_breakpoints() {
        let messages = [
            message("system", "設定".to_owned()),
            message("assistant", "旁白一".to_owned()),
            message("user", "玩家：嗨".to_owned()),
            message("assistant", "旁白二".to_owned()),
            message("user", "動態塊".to_owned()),
        ];
        let body = chat_request_body("anthropic/claude-sonnet-4.5", &messages);
        let out = body["messages"].as_array().unwrap();
        assert_eq!(out.len(), 5);
        // multipart：每則 content 是單一 text 分段，role 與文字照舊
        assert_eq!(out[2]["role"], "user");
        assert_eq!(out[2]["content"][0]["type"], "text");
        assert_eq!(out[2]["content"][0]["text"], "玩家：嗨");
        // 斷點恰好兩個：system（index 0）與最後一則 assistant（index 3）
        let marked: Vec<usize> = out
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry["content"][0].get("cache_control").is_some())
            .map(|(index, _)| index)
            .collect();
        assert_eq!(marked, [0, 3]);
        assert_eq!(
            out[0]["content"][0]["cache_control"],
            serde_json::json!({ "type": "ephemeral" })
        );

        // 非 anthropic 模型：content 維持純字串（形狀逐位元不變由上一條測試保證）
        let plain = chat_request_body("test/model", &messages);
        assert!(plain["messages"][0]["content"].is_string());

        // 開桌第一輪沒有 assistant：只標 system，不出錯
        let fresh = [
            message("system", "設定".to_owned()),
            message("user", "嗨".to_owned()),
        ];
        let fresh_body = chat_request_body("anthropic/claude-haiku", &fresh);
        let fresh_out = fresh_body["messages"].as_array().unwrap();
        assert!(fresh_out[0]["content"][0].get("cache_control").is_some());
        assert!(fresh_out[1]["content"][0].get("cache_control").is_none());
    }

    #[test]
    fn extract_usage_reads_final_chunk_and_ignores_delta_chunks() {
        // OpenRouter 尾塊：prompt_tokens_details.cached_tokens 是快取命中數
        let usage = extract_usage(
            r#"{"choices":[],"usage":{"prompt_tokens":194,"prompt_tokens_details":{"cached_tokens":150,"audio_tokens":0},"completion_tokens":2,"total_tokens":196}}"#,
        )
        .unwrap();
        assert_eq!(
            usage,
            PromptCacheUsage {
                prompt_tokens: 194,
                cached_tokens: Some(150),
                created_tokens: None, // 這則沒有 cache_write_tokens：沒回報，不是 0
                output_tokens: 2,
                cost_usd: None, // 金額只有 claude CLI 直接回報
            }
        );

        // OpenRouter 也回寫入數時照收
        let with_write = extract_usage(
            r#"{"usage":{"prompt_tokens":300,"prompt_tokens_details":{"cached_tokens":100,"cache_write_tokens":200},"completion_tokens":5}}"#,
        )
        .unwrap();
        assert_eq!(
            (with_write.cached_tokens, with_write.created_tokens),
            (Some(100), Some(200))
        );

        // 混合 schema（相容層改版或雙格式轉送都可能同時吐出 normalized 與 upstream-native
        // 欄位）：讀、寫各自挑第一個有值的來源，寫入數不可遮蔽掉另一組的讀取數
        let mixed = extract_usage(
            r#"{"usage":{"prompt_tokens":100,"prompt_tokens_details":{"cache_write_tokens":20},"prompt_cache_hit_tokens":80,"completion_tokens":1}}"#,
        )
        .unwrap();
        assert_eq!((mixed.cached_tokens, mixed.created_tokens), (Some(80), Some(20)));

        // 兩組讀取欄位同時存在＝第一順位（OpenRouter）勝出，不做衝突偵測
        let both_reads = extract_usage(
            r#"{"usage":{"prompt_tokens":100,"prompt_tokens_details":{"cached_tokens":70},"prompt_cache_hit_tokens":50,"completion_tokens":1}}"#,
        )
        .unwrap();
        assert_eq!(both_reads.cached_tokens, Some(70));

        // DeepSeek 原生欄位（中轉站照抄這組、不回 prompt_tokens_details）：
        // 讀錯這裡正是額度分頁對 API 路顯示假 0.0% 的根因，2026-08-21 對 tokenrouter 實測取證
        let deepseek = extract_usage(
            r#"{"usage":{"prompt_tokens":2495,"completion_tokens":16,"prompt_cache_hit_tokens":0,"prompt_cache_miss_tokens":2495}}"#,
        )
        .unwrap();
        assert_eq!(deepseek.cached_tokens, Some(0)); // 量到了、這輪沒中
        assert!(deepseek.reported());
        assert_eq!(deepseek.hit_rate(), Some(0.0));

        // Anthropic 原生欄位直通
        let anthropic = extract_usage(
            r#"{"usage":{"prompt_tokens":900,"cache_read_input_tokens":800,"cache_creation_input_tokens":100,"completion_tokens":3}}"#,
        )
        .unwrap();
        assert_eq!(
            (anthropic.cached_tokens, anthropic.created_tokens),
            (Some(800), Some(100))
        );

        // 一組欄位都沒有＝這條路不回報：cached 為 None、命中率不存在，**不可退成 0**
        let without_details =
            extract_usage(r#"{"usage":{"prompt_tokens":10,"completion_tokens":1}}"#).unwrap();
        assert_eq!(without_details.cached_tokens, None);
        assert!(!without_details.reported());
        assert_eq!(without_details.hit_rate(), None);

        // 增量塊：usage 為 null 或不存在，一律回 None
        assert_eq!(
            extract_usage(r#"{"choices":[{"delta":{"content":"嗨"}}],"usage":null}"#),
            None
        );
        assert_eq!(
            extract_usage(r#"{"choices":[{"delta":{"content":"嗨"}}]}"#),
            None
        );
        assert_eq!(extract_usage("not json"), None);
    }

    /// 尾端 usage 塊混在串流裡：增量文字照常回傳，usage 塊不產生任何 delta
    #[tokio::test]
    async fn stream_chat_passes_usage_chunk_through_without_breaking_deltas() {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0u8; 4096];
            let _ = socket.read(&mut request);
            let body = concat!(
                "data: {\"choices\":[{\"delta\":{\"content\":\"你\"}}],\"usage\":null}\n\n",
                "data: {\"choices\":[{\"delta\":{\"content\":\"好\"}}],\"usage\":null}\n\n",
                "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":20,\"prompt_tokens_details\":{\"cached_tokens\":12}}}\n\n",
                "data: [DONE]\n\n",
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).unwrap();
        });

        let mut config = AppConfig::default();
        config.preferences.insert(
            "base_url".to_owned(),
            serde_json::Value::String(format!("http://{address}")),
        );
        let messages = [message("user", "嗨".to_owned())];
        let log_path =
            std::env::temp_dir().join(format!("tt-prompt-cache-test-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&log_path);
        let mut deltas = Vec::new();
        let full = stream_chat(
            &config,
            "test/model",
            &messages,
            Some(&log_path),
            Some("w1"),
            |delta| {
                deltas.push(delta.to_owned());
            },
        )
        .await
        .unwrap();
        assert_eq!(full, "你好");
        assert_eq!(deltas, ["你", "好"]);

        // usage 落檔：一行 JSONL 含時間戳、模型、token 數與命中率（12/20 = 60%）
        let logged = std::fs::read_to_string(&log_path).unwrap();
        assert_eq!(logged.lines().count(), 1);
        let record: serde_json::Value = serde_json::from_str(logged.trim()).unwrap();
        assert_eq!(record["transport"], "api");
        assert_eq!(record["model"], "test/model");
        assert_eq!(record["prompt_tokens"], 20);
        assert_eq!(record["cached_tokens"], 12);
        assert_eq!(record["hit_rate"], 60.0);
        assert_eq!(record["diag"], "single");
        let _ = std::fs::remove_file(&log_path);
    }

    #[test]
    fn extract_delta_ignores_non_delta_payloads() {
        assert_eq!(extract_delta(r#"{"choices":[]}"#), None);
        assert_eq!(extract_delta(r#"{"usage":{"total_tokens":9}}"#), None);
        assert_eq!(
            extract_delta(r#"{"choices":[{"delta":{"content":""}}]}"#),
            None
        );
        assert_eq!(
            extract_delta(r#"{"choices":[{"delta":{"content":"嗨"}}]}"#).unwrap(),
            "嗨"
        );
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

        let character = assemble_messages(
            &fox,
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

        let character = assemble_messages(
            &fox,
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
        let character1 = assemble_messages(
            &fox,
            None,
            &round1_events,
            &entries,
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
        );
        let character2 = assemble_messages(
            &fox,
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

    #[test]
    fn extract_state_fence_returns_fields_and_hides_fence() {
        let StateBlock {
            fields, display, ..
        } = extract_state_block(
            "雨停了。\n```state\ntime: 午夜\nplace：舊碼頭\npresent: 阿濤、船長\n```",
        );
        assert_eq!(
            fields,
            vec![
                (vec!["time".to_owned()], "午夜".to_owned()),
                (vec!["place".to_owned()], "舊碼頭".to_owned()),
                (vec!["present".to_owned()], "阿濤、船長".to_owned()),
            ]
        );
        assert_eq!(display, "雨停了。");
    }

    #[test]
    fn extract_state_block_collects_nested_yaml_and_skips_plain_list_items() {
        let StateBlock { fields, .. } = extract_state_block(
            "<Status_block>World:\n  - 城市:\n      名稱: \"晨港\"\n      - 純清單項\n      人口: '1200'\n</Status_block>",
        );
        assert_eq!(
            fields,
            vec![
                (
                    vec!["World".to_owned(), "城市".to_owned(), "名稱".to_owned()],
                    "晨港".to_owned(),
                ),
                (
                    vec!["World".to_owned(), "城市".to_owned(), "人口".to_owned()],
                    "1200".to_owned(),
                ),
            ]
        );
    }

    /// 縮排行與空字典都要保留成分支標記，葉子則保留實際值。
    #[test]
    fn parse_indented_fields_marks_branch_lines() {
        assert_eq!(
            parse_indented_fields("World:\n  Time: 清晨\n  Inventory: {}"),
            vec![
                (vec!["World".to_owned()], None),
                (
                    vec!["World".to_owned(), "Time".to_owned()],
                    Some("清晨".to_owned()),
                ),
                (vec!["World".to_owned(), "Inventory".to_owned()], None),
            ]
        );
    }

    #[test]
    fn gm_dynamic_block_renders_nested_tree() {
        let state = TableState {
            table: std::collections::BTreeMap::new(),
            tree: std::collections::BTreeMap::from([(
                "World".to_owned(),
                StateNode::Branch(std::collections::BTreeMap::from([(
                    "城市".to_owned(),
                    StateNode::Leaf("晨港".to_owned()),
                )])),
            )]),
            notes: Vec::new(),
            changes: std::collections::BTreeMap::new(),
            triggers: std::collections::BTreeMap::new(),
            jumps: std::collections::BTreeMap::new(),
        };
        let dynamic = gm_dynamic_block(
            &[],
            &state,
            "阿濤",
            &Mechanism::default(),
            &StateScope::default(),
            "zh-TW",
        );
        assert!(dynamic.contains("World：\n  城市：晨港"));
    }

    /// 初始樹保留的玩家巨集只在送進這桌模型上下文前才換成實名。
    #[test]
    fn gm_dynamic_block_replaces_user_macro_in_tree_leaves() {
        let state = TableState {
            table: std::collections::BTreeMap::new(),
            tree: std::collections::BTreeMap::from([(
                "Player".to_owned(),
                StateNode::Branch(std::collections::BTreeMap::from([(
                    "Name".to_owned(),
                    StateNode::Leaf("{{user}}".to_owned()),
                )])),
            )]),
            notes: Vec::new(),
            changes: std::collections::BTreeMap::new(),
            triggers: std::collections::BTreeMap::new(),
            jumps: std::collections::BTreeMap::new(),
        };

        let dynamic = gm_dynamic_block(
            &[],
            &state,
            "阿濤",
            &Mechanism::default(),
            &StateScope::default(),
            "zh-TW",
        );
        assert!(dynamic.contains("Name：阿濤"));
        assert!(!dynamic.contains("{{user}}"));
    }

    #[test]
    fn gm_dynamic_block_prints_notes_after_current_state_and_hides_when_empty() {
        let with_notes = TableState {
            table: std::collections::BTreeMap::new(),
            tree: std::collections::BTreeMap::new(),
            notes: vec!["World.HP 已夾在範圍內，目前值 100。".to_owned()],
            changes: std::collections::BTreeMap::new(),
            triggers: std::collections::BTreeMap::new(),
            jumps: std::collections::BTreeMap::new(),
        };
        let dynamic = gm_dynamic_block(
            &[],
            &with_notes,
            "阿濤",
            &Mechanism::default(),
            &StateScope::default(),
            "zh-TW",
        );
        assert!(dynamic.contains("## 上一輪被系統擋下的更新（請照這些現值修正）"));
        assert!(dynamic.contains("World.HP 已夾在範圍內，目前值 100。"));

        let without_notes = TableState::default();
        let dynamic = gm_dynamic_block(
            &[],
            &without_notes,
            "阿濤",
            &Mechanism::default(),
            &StateScope::default(),
            "zh-TW",
        );
        assert!(!dynamic.contains("上一輪被系統擋下的更新"));
    }

    // ---- gm_dynamic_block：觸發表命中文本的「當前情境」段 ----

    fn trigger_with_scope(id: &str, scope: &[&str]) -> data::Trigger {
        data::Trigger {
            id: id.to_owned(),
            title: id.to_owned(),
            mode: data::TriggerMode::Range,
            cases: Vec::new(),
            preamble: String::new(),
            scope: scope.iter().map(|segment| (*segment).to_owned()).collect(),
            flag: None,
        }
    }

    #[test]
    fn gm_dynamic_block_prints_trigger_hits_after_current_state() {
        let mechanism = Mechanism {
            version: 1,
            rules: BTreeMap::new(),
            triggers: vec![trigger_with_scope("侵略", &[])],
            incremental: true,
            guide: String::new(),
        };
        let state = TableState {
            table: BTreeMap::from([("time".to_owned(), "黃昏".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::from([("侵略".to_owned(), "戰雲密布".to_owned())]),
            jumps: BTreeMap::new(),
        };
        let dynamic = gm_dynamic_block(
            &[],
            &state,
            "阿濤",
            &mechanism,
            &StateScope::default(),
            "zh-TW",
        );
        let state_pos = dynamic.find("## 目前狀態").expect("目前狀態應該有印");
        let trigger_pos = dynamic.find("## 當前情境").expect("當前情境應該有印");
        assert!(trigger_pos > state_pos);
        assert!(dynamic.contains("戰雲密布"));
    }

    #[test]
    fn gm_dynamic_block_hides_trigger_section_when_state_triggers_is_empty() {
        let mechanism = Mechanism {
            version: 1,
            rules: BTreeMap::new(),
            triggers: vec![trigger_with_scope("侵略", &[])],
            incremental: true,
            guide: String::new(),
        };
        let state = TableState {
            table: BTreeMap::from([("time".to_owned(), "黃昏".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        let dynamic = gm_dynamic_block(
            &[],
            &state,
            "阿濤",
            &mechanism,
            &StateScope::default(),
            "zh-TW",
        );
        assert!(!dynamic.contains("當前情境"));
    }

    /// 不在場角色那支被裁掉時，牽到那支的觸發文本不該送；`align`（換幕全樹對齊）
    /// 忽略裁切，照樣全印。
    #[test]
    fn gm_dynamic_block_hides_trigger_scoped_to_a_hidden_branch_but_prints_when_aligned() {
        let mechanism = Mechanism {
            version: 1,
            rules: BTreeMap::new(),
            triggers: vec![
                trigger_with_scope("亞瑟關係", &["Heroes", "亞瑟"]),
                trigger_with_scope("世界氛圍", &[]),
            ],
            incremental: true,
            guide: String::new(),
        };
        let state = TableState {
            table: BTreeMap::from([("time".to_owned(), "黃昏".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::from([
                ("亞瑟關係".to_owned(), "亞瑟關係文本".to_owned()),
                ("世界氛圍".to_owned(), "世界氛圍文本".to_owned()),
            ]),
            jumps: BTreeMap::new(),
        };
        let hidden = vec![vec!["Heroes".to_owned(), "亞瑟".to_owned()]];
        let scope = StateScope {
            hidden: hidden.clone(),
            align: false,
        };
        let dynamic = gm_dynamic_block(&[], &state, "阿濤", &mechanism, &scope, "zh-TW");
        assert!(!dynamic.contains("亞瑟關係文本"));
        assert!(dynamic.contains("世界氛圍文本"));

        let aligned = StateScope {
            hidden,
            align: true,
        };
        let dynamic = gm_dynamic_block(&[], &state, "阿濤", &mechanism, &aligned, "zh-TW");
        assert!(dynamic.contains("亞瑟關係文本"));
    }

    /// scope 比隱藏分支更深（後代路徑）也要跟著裁——亞瑟底下的好感細節同樣屬於他那支。
    #[test]
    fn gm_dynamic_block_hides_trigger_scoped_to_a_descendant_of_a_hidden_branch() {
        let mechanism = Mechanism {
            version: 1,
            rules: BTreeMap::new(),
            triggers: vec![trigger_with_scope(
                "亞瑟細節",
                &["Heroes", "亞瑟", "Affection"],
            )],
            incremental: true,
            guide: String::new(),
        };
        let state = TableState {
            table: BTreeMap::from([("time".to_owned(), "黃昏".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::from([("亞瑟細節".to_owned(), "亞瑟細節文本".to_owned())]),
            jumps: BTreeMap::new(),
        };
        let scope = StateScope {
            hidden: vec![vec!["Heroes".to_owned(), "亞瑟".to_owned()]],
            align: false,
        };
        let dynamic = gm_dynamic_block(&[], &state, "阿濤", &mechanism, &scope, "zh-TW");
        assert!(!dynamic.contains("亞瑟細節文本"));
    }

    #[test]
    fn gm_dynamic_block_orders_triggers_by_mechanism_list_not_by_map_key() {
        let mechanism = Mechanism {
            version: 1,
            rules: BTreeMap::new(),
            // 刻意讓清單順序跟字典序相反，確認印出順序跟著 Vec 走。
            triggers: vec![trigger_with_scope("乙", &[]), trigger_with_scope("甲", &[])],
            incremental: true,
            guide: String::new(),
        };
        let state = TableState {
            table: BTreeMap::from([("time".to_owned(), "黃昏".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::from([
                ("甲".to_owned(), "甲文本".to_owned()),
                ("乙".to_owned(), "乙文本".to_owned()),
            ]),
            jumps: BTreeMap::new(),
        };
        let dynamic = gm_dynamic_block(
            &[],
            &state,
            "阿濤",
            &mechanism,
            &StateScope::default(),
            "zh-TW",
        );
        let pos_b = dynamic.find("乙文本").unwrap();
        let pos_a = dynamic.find("甲文本").unwrap();
        assert!(pos_b < pos_a);
    }

    /// 全量桌（`!mechanism.incremental`）逐字維持現狀：就算 `state.triggers` 有值也不印。
    #[test]
    fn gm_dynamic_block_never_prints_trigger_section_for_a_full_snapshot_table() {
        let state = TableState {
            table: BTreeMap::from([("time".to_owned(), "黃昏".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::from([("侵略".to_owned(), "戰雲密布".to_owned())]),
            jumps: BTreeMap::new(),
        };
        let dynamic = gm_dynamic_block(
            &[],
            &state,
            "阿濤",
            &Mechanism::default(),
            &StateScope::default(),
            "zh-TW",
        );
        assert!(!dynamic.contains("當前情境"));
        assert!(!dynamic.contains("戰雲密布"));
    }

    #[test]
    fn extract_state_discards_bad_lines_without_losing_valid_fields() {
        let StateBlock {
            fields, display, ..
        } = extract_state_block(
            "旁白\n```state\n- time: 清晨\n沒有冒號\nplace:   \n# 自訂：有效\n```",
        );
        assert_eq!(
            fields,
            vec![
                (vec!["time".to_owned()], "清晨".to_owned()),
                (vec!["自訂".to_owned()], "有效".to_owned()),
            ]
        );
        assert_eq!(display, "旁白");
    }

    #[test]
    fn extract_state_details_summary_is_parsed_and_hidden() {
        let StateBlock {
            fields, display, ..
        } = extract_state_block(
            "港口傳來鐘聲。<details><summary>状态栏</summary>时间：黃昏\n地点：港口</details>",
        );
        assert_eq!(
            fields,
            vec![
                (vec!["time".to_owned()], "黃昏".to_owned()),
                (vec!["place".to_owned()], "港口".to_owned()),
            ]
        );
        assert_eq!(display, "港口傳來鐘聲。");
    }

    #[test]
    fn extract_status_tag_is_parsed_and_hidden() {
        let StateBlock {
            fields, display, ..
        } = extract_state_block("門開了。<STATUS>time: 午夜\nplace: 走廊</status>剩下的話。");
        assert_eq!(
            fields,
            vec![
                (vec!["time".to_owned()], "午夜".to_owned()),
                (vec!["place".to_owned()], "走廊".to_owned()),
            ]
        );
        assert_eq!(display, "門開了。剩下的話。");
    }

    #[test]
    fn extract_update_variable_hides_json_without_parsing_it() {
        let StateBlock {
            fields,
            updates,
            display,
        } = extract_state_block("她點頭。<UpdateVariable>{\"time\":\"午夜\"}</UpdateVariable>");
        assert!(fields.is_empty());
        assert_eq!(updates, vec!["{\"time\":\"午夜\"}".to_owned()]);
        assert_eq!(display, "她點頭。");
    }

    /// 各家標籤名不同（donass 的 `<StatusData>`、鎮北王府的 `<Status_block>`），
    /// 開頭是 status 就認；同名才配對，`<statusdata>` 不會被 `</status_block>` 收掉。
    #[test]
    fn extract_state_accepts_any_status_prefixed_tag() {
        let StateBlock {
            fields, display, ..
        } = extract_state_block(
            "地下城。<StatusData>体力:60\n好感:20</StatusData>之後。\
             <Status_block>时间: 戌时\n地点: 浴房</Status_block>",
        );
        assert_eq!(
            fields,
            vec![
                (vec!["体力".to_owned()], "60".to_owned()),
                (vec!["好感".to_owned()], "20".to_owned()),
                (vec!["time".to_owned()], "戌时".to_owned()),
                (vec!["place".to_owned()], "浴房".to_owned()),
            ]
        );
        assert_eq!(display, "地下城。之後。");
    }

    /// 沒有配對收尾的標籤整段留著：寧可讓玩家看到半截標籤，也不吞掉後面的旁白。
    #[test]
    fn extract_state_leaves_unclosed_status_tag_alone() {
        let reply = "他開口。<StatusData>体力:60\n後面還有很多話。";
        assert_eq!(
            extract_state_block(reply),
            StateBlock {
                fields: Vec::new(),
                updates: Vec::new(),
                display: reply.to_owned(),
            }
        );
    }

    /// 名字裡帶 status 但不是開頭的（`<combatStatus>`）是卡片自訂欄位，不能當狀態區塊剝掉。
    #[test]
    fn extract_state_ignores_tags_merely_containing_status() {
        let reply = "他喘著氣。<combatStatus>負傷</combatStatus>";
        assert_eq!(
            extract_state_block(reply),
            StateBlock {
                fields: Vec::new(),
                updates: Vec::new(),
                display: reply.to_owned(),
            }
        );
    }

    #[test]
    fn extract_state_unwraps_maintext_into_narration() {
        let StateBlock {
            fields, display, ..
        } = extract_state_block(
            "<maintext>\n夜色濃重。\n</maintext>\n<Status_block>时间: 戌时</Status_block>",
        );
        assert_eq!(fields, vec![(vec!["time".to_owned()], "戌时".to_owned())]);
        assert_eq!(display, "\n夜色濃重。");
    }

    /// 只有正文外殼、沒有狀態區塊時，一樣要拆掉外殼。
    #[test]
    fn extract_state_unwraps_maintext_without_state_block() {
        let StateBlock {
            fields, display, ..
        } = extract_state_block("<mainText>夜色濃重。</mainText>");
        assert!(fields.is_empty());
        assert_eq!(display, "夜色濃重。");
    }

    #[test]
    fn extract_state_keeps_unwrapped_narration_byte_for_byte() {
        let reply = "純旁白\n保留尾端空行\n\n";
        assert_eq!(
            extract_state_block(reply),
            StateBlock {
                fields: Vec::new(),
                updates: Vec::new(),
                display: reply.to_owned(),
            }
        );
    }

    #[test]
    fn extract_state_keeps_middle_code_fence_but_removes_trailing_plain_fence() {
        let reply = "提示：\n```rust\nlet time = 1;\n```\n旁白\n```\ntime: 午夜\n```";
        let StateBlock {
            fields, display, ..
        } = extract_state_block(reply);
        assert_eq!(fields, vec![(vec!["time".to_owned()], "午夜".to_owned())]);
        assert_eq!(display, "提示：\n```rust\nlet time = 1;\n```\n旁白");
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
            lane_event_line(&event(TranscriptKind::Dialogue, "fox-id", "狐狸", "晚安"), false),
            "狐狸：晚安"
        );
        assert_eq!(
            lane_event_line(&event(TranscriptKind::Player, "", "阿濤", "好啊"), false),
            "阿濤：好啊"
        );
        assert_eq!(
            lane_event_line(&event(TranscriptKind::Narration, "", "GM", "夜深了"), false),
            "（旁白）夜深了"
        );
        assert_eq!(
            lane_event_line(&event(TranscriptKind::System, "", "", "擲骰 3"), false),
            "（系統）擲骰 3"
        );
    }

    // ---- 狀態欄二期包 5：注入策略＋分支切割 ----

    #[test]
    fn state_scope_hides_absent_characters_keeps_player_and_present_and_is_disabled_by_empty_present(
    ) {
        let arthur = card("arthur-id", "亞瑟", "", "");
        let crow = card("crow-id", "鴉", "", "");
        let player = card("player-id", "阿濤", "", "");
        let tree = BTreeMap::from([
            (
                "亞瑟".to_owned(),
                StateNode::Branch(BTreeMap::from([(
                    "HP".to_owned(),
                    StateNode::Leaf("100".to_owned()),
                )])),
            ),
            (
                "鴉".to_owned(),
                StateNode::Branch(BTreeMap::from([(
                    "HP".to_owned(),
                    StateNode::Leaf("50".to_owned()),
                )])),
            ),
        ]);
        let mechanism = Mechanism {
            incremental: true,
            ..Mechanism::default()
        };
        let bindings = BTreeMap::new();

        let mut present_state = TableState {
            tree: tree.clone(),
            ..TableState::default()
        };
        present_state
            .table
            .insert("present".to_owned(), "亞瑟".to_owned());
        let scope = state_scope(
            &present_state,
            &mechanism,
            &[arthur.clone(), crow.clone()],
            Some(&player),
            &bindings,
            false,
        );
        assert_eq!(scope.hidden, vec![vec!["鴉".to_owned()]]);
        assert!(!scope.align);

        // present 欄空著＝寧可全送，不要因為模型沒報 present 就裁瞎了
        let empty_present_state = TableState {
            tree,
            ..TableState::default()
        };
        let scope = state_scope(
            &empty_present_state,
            &mechanism,
            &[arthur, crow],
            Some(&player),
            &bindings,
            true,
        );
        assert!(scope.hidden.is_empty());
        assert!(scope.align);

        // 全量桌完全不裁、不對齊
        let scope = state_scope(
            &present_state,
            &Mechanism::default(),
            &[],
            None,
            &bindings,
            true,
        );
        assert!(scope.hidden.is_empty());
        assert!(!scope.align);
    }

    /// 手足規則：容器裡有一支綁到角色卡，同容器其餘分支就一律當人看——
    /// MVU 卡 15 個英雄只會有幾張角色卡，剩下的沒卡也該裁。頂層不套這條（會把 World 裁掉）。
    #[test]
    fn state_scope_hides_uncarded_siblings_in_the_same_container_but_never_at_the_top_level() {
        let hero = |hp: &str| {
            StateNode::Branch(BTreeMap::from([(
                "HP".to_owned(),
                StateNode::Leaf(hp.to_owned()),
            )]))
        };
        let mut state = TableState {
            tree: BTreeMap::from([
                (
                    "World".to_owned(),
                    StateNode::Branch(BTreeMap::from([(
                        "Invasion".to_owned(),
                        StateNode::Leaf("35".to_owned()),
                    )])),
                ),
                (
                    "Heroes".to_owned(),
                    StateNode::Branch(BTreeMap::from([
                        ("亞瑟".to_owned(), hero("100")),
                        ("鴉".to_owned(), hero("50")),
                        ("諾亞".to_owned(), hero("70")),
                    ])),
                ),
            ]),
            ..TableState::default()
        };
        state.table.insert("present".to_owned(), "諾亞".to_owned());
        let mechanism = Mechanism {
            incremental: true,
            ..Mechanism::default()
        };
        // 只有亞瑟有角色卡：他不在場所以被裁，沒卡的鴉跟著被裁，在場的諾亞留著
        let scope = state_scope(
            &state,
            &mechanism,
            &[card("arthur-id", "亞瑟", "", "")],
            None,
            &BTreeMap::new(),
            false,
        );
        assert!(scope
            .hidden
            .contains(&vec!["Heroes".to_owned(), "亞瑟".to_owned()]));
        assert!(scope
            .hidden
            .contains(&vec!["Heroes".to_owned(), "鴉".to_owned()]));
        assert!(!scope
            .hidden
            .contains(&vec!["Heroes".to_owned(), "諾亞".to_owned()]));
        // 桌級分支不受手足規則波及
        assert!(!scope.hidden.contains(&vec!["World".to_owned()]));
        assert!(!scope.hidden.contains(&vec!["Heroes".to_owned()]));
    }

    #[test]
    fn incremental_round_tail_hides_absent_branch_snapshot_and_rare_but_shows_turn_with_marks() {
        let mechanism = Mechanism {
            incremental: true,
            guide: String::new(),
            rules: BTreeMap::from([(
                "World.Secret".to_owned(),
                FieldRule::for_kind(FieldKind::ReadOnly),
            )]),
            ..Mechanism::default()
        };
        let mut state = TableState::default();
        state.table.insert("present".to_owned(), "亞瑟".to_owned());
        state.tree = BTreeMap::from([
            (
                "World".to_owned(),
                StateNode::Branch(BTreeMap::from([
                    ("HP".to_owned(), StateNode::Leaf("100".to_owned())),
                    ("Desc".to_owned(), StateNode::Leaf("晨港".to_owned())),
                    ("Secret".to_owned(), StateNode::Leaf("藏寶圖".to_owned())),
                ])),
            ),
            (
                "亞瑟".to_owned(),
                StateNode::Branch(BTreeMap::from([(
                    "HP".to_owned(),
                    StateNode::Leaf("80".to_owned()),
                )])),
            ),
            (
                "鴉".to_owned(),
                StateNode::Branch(BTreeMap::from([(
                    "HP".to_owned(),
                    StateNode::Leaf("50".to_owned()),
                )])),
            ),
        ]);
        state.changes.insert("World.HP".to_owned(), "+5".to_owned());

        // 平常輪：不在場的「鴉」整支不印；Snapshot（Desc）不印；Rare（Secret）不印；
        // Turn（HP）印，且帶變動標記。
        let scope = StateScope {
            hidden: vec![vec!["鴉".to_owned()]],
            align: false,
        };
        let dynamic = gm_dynamic_block(&[], &state, "阿濤", &mechanism, &scope, "zh-TW");
        assert!(dynamic.contains("## 目前狀態（這桌的檯面，接續它往下演）"));
        assert!(dynamic.contains("HP：100（+5）"));
        assert!(!dynamic.contains("Desc"));
        assert!(!dynamic.contains("Secret"));
        assert!(!dynamic.contains("鴉"));
        assert!(dynamic.contains("亞瑟"));

        // 對齊輪：忽略 hidden、Snapshot 也印，只有 Rare 還是不印；標題換成對齊版。
        let align_scope = StateScope {
            hidden: vec![vec!["鴉".to_owned()]],
            align: true,
        };
        let aligned = gm_dynamic_block(&[], &state, "阿濤", &mechanism, &align_scope, "zh-TW");
        assert!(aligned.contains("## 目前狀態（完整對齊，以下是系統帳上的真值，請以此為準）"));
        assert!(aligned.contains("Desc：晨港"));
        assert!(!aligned.contains("Secret"));
        assert!(aligned.contains("鴉"));
    }

    /// 全量桌（!mechanism.incremental）逐字維持現狀：不管 mechanism.rules、changes、
    /// scope 塞了什麼，輸出都跟本包以前的行為一樣——不裁、不濾、不標。
    #[test]
    fn full_scale_table_renders_everything_verbatim_ignoring_rules_scope_and_changes() {
        let mechanism = Mechanism {
            incremental: false,
            guide: String::new(),
            rules: BTreeMap::from([(
                "World.Secret".to_owned(),
                FieldRule::for_kind(FieldKind::ReadOnly),
            )]),
            ..Mechanism::default()
        };
        let mut state = TableState {
            tree: BTreeMap::from([(
                "World".to_owned(),
                StateNode::Branch(BTreeMap::from([
                    ("HP".to_owned(), StateNode::Leaf("100".to_owned())),
                    ("Desc".to_owned(), StateNode::Leaf("晨港".to_owned())),
                    ("Secret".to_owned(), StateNode::Leaf("藏寶圖".to_owned())),
                ])),
            )]),
            ..TableState::default()
        };
        state.changes.insert("World.HP".to_owned(), "+5".to_owned());
        let scope = StateScope {
            hidden: vec![vec!["World".to_owned()]],
            align: false,
        };

        let dynamic = gm_dynamic_block(&[], &state, "阿濤", &mechanism, &scope, "zh-TW");
        assert_eq!(
            dynamic,
            "## 目前狀態（這桌的檯面，接續它往下演）\n\
             World：\n  Desc：晨港\n  HP：100\n  Secret：藏寶圖"
        );
    }

    #[test]
    fn character_state_block_shows_only_own_branch_with_marks_and_excludes_rare() {
        let mechanism = Mechanism {
            incremental: true,
            guide: String::new(),
            rules: BTreeMap::from([(
                "Heroes.亞瑟.Hidden".to_owned(),
                FieldRule::for_kind(FieldKind::ReadOnly),
            )]),
            ..Mechanism::default()
        };
        let mut state = TableState {
            tree: BTreeMap::from([(
                "Heroes".to_owned(),
                StateNode::Branch(BTreeMap::from([
                    (
                        "亞瑟".to_owned(),
                        StateNode::Branch(BTreeMap::from([
                            ("HP".to_owned(), StateNode::Leaf("80".to_owned())),
                            ("Hidden".to_owned(), StateNode::Leaf("秘密".to_owned())),
                        ])),
                    ),
                    (
                        "鴉".to_owned(),
                        StateNode::Branch(BTreeMap::from([(
                            "HP".to_owned(),
                            StateNode::Leaf("50".to_owned()),
                        )])),
                    ),
                ])),
            )]),
            ..TableState::default()
        };
        state
            .changes
            .insert("Heroes.亞瑟.HP".to_owned(), "-10".to_owned());

        let branch = vec!["Heroes".to_owned(), "亞瑟".to_owned()];
        let block = character_state_block(&state, &mechanism, &branch, "亞瑟", "阿濤").unwrap();
        assert!(block.starts_with(
            "## 「亞瑟」目前的狀態（系統帳，唯讀；可以拿來演，但不要輸出任何狀態欄或更新區塊）"
        ));
        assert!(block.contains("HP：80（-10）"));
        assert!(!block.contains("Hidden"));
        assert!(!block.contains("鴉"));

        // 沒綁到分支、分支其實是葉子、分支不存在——都是 None
        assert!(character_state_block(&state, &mechanism, &[], "亞瑟", "阿濤").is_none());
        assert!(character_state_block(
            &state,
            &mechanism,
            &["Heroes".to_owned(), "亞瑟".to_owned(), "HP".to_owned()],
            "亞瑟",
            "阿濤"
        )
        .is_none());
        assert!(
            character_state_block(&state, &mechanism, &["不存在".to_owned()], "亞瑟", "阿濤")
                .is_none()
        );
    }

    #[test]
    fn resolve_branch_prefers_binding_falls_back_when_invalid_and_finds_nested_names() {
        let tree = BTreeMap::from([
            (
                "Heroes".to_owned(),
                StateNode::Branch(BTreeMap::from([(
                    "亞瑟".to_owned(),
                    StateNode::Branch(BTreeMap::from([(
                        "HP".to_owned(),
                        StateNode::Leaf("80".to_owned()),
                    )])),
                )])),
            ),
            ("鴉".to_owned(), StateNode::Branch(BTreeMap::new())),
        ]);

        // 指認優先：卡名明明是「鴉」，指認路徑照樣贏
        let bindings = BTreeMap::from([(
            "card-crow".to_owned(),
            vec!["Heroes".to_owned(), "亞瑟".to_owned()],
        )]);
        assert_eq!(
            resolve_branch(&tree, &bindings, "card-crow", "鴉"),
            Some(vec!["Heroes".to_owned(), "亞瑟".to_owned()])
        );

        // 指認路徑不存在＝失效，退回同名比對
        let invalid = BTreeMap::from([(
            "card-crow".to_owned(),
            vec!["Not".to_owned(), "Exist".to_owned()],
        )]);
        assert_eq!(
            resolve_branch(&tree, &invalid, "card-crow", "鴉"),
            Some(vec!["鴉".to_owned()])
        );

        // 指認路徑指到葉子（不是分支）也視為失效
        let points_at_leaf = BTreeMap::from([(
            "card-arthur".to_owned(),
            vec!["Heroes".to_owned(), "亞瑟".to_owned(), "HP".to_owned()],
        )]);
        assert_eq!(
            resolve_branch(&tree, &points_at_leaf, "card-arthur", "亞瑟"),
            Some(vec!["Heroes".to_owned(), "亞瑟".to_owned()])
        );

        // 沒有指認：巢狀（Heroes/亞瑟）也找得到
        assert_eq!(
            resolve_branch(&tree, &BTreeMap::new(), "card-arthur", "亞瑟"),
            Some(vec!["Heroes".to_owned(), "亞瑟".to_owned()])
        );

        // 完全找不到
        assert_eq!(
            resolve_branch(&tree, &BTreeMap::new(), "card-x", "不存在的人"),
            None
        );
    }

    #[test]
    fn snapshot_updates_returns_only_snapshot_level_changes_with_user_macro_replaced() {
        let mechanism = Mechanism {
            incremental: true,
            ..Mechanism::default()
        };
        let state = TableState {
            tree: BTreeMap::from([(
                "World".to_owned(),
                StateNode::Branch(BTreeMap::from([
                    ("HP".to_owned(), StateNode::Leaf("100".to_owned())),
                    (
                        "Desc".to_owned(),
                        StateNode::Leaf("{{user}} 的家鄉".to_owned()),
                    ),
                ])),
            )]),
            changes: BTreeMap::from([
                ("World.HP".to_owned(), "+5".to_owned()),
                ("World.Desc".to_owned(), "更新".to_owned()),
            ]),
            ..TableState::default()
        };

        let updates = snapshot_updates(&state, &mechanism, "阿濤");
        assert_eq!(
            updates,
            vec![("World.Desc".to_owned(), "阿濤 的家鄉".to_owned())]
        );

        // 全量桌一律回空
        assert!(snapshot_updates(&state, &Mechanism::default(), "阿濤").is_empty());
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

    /// 回歸事件文字格式：固定前綴＋〈name〉一行，接公開設定與私有設定全文（`{{user}}` 已代換）；
    /// 空的欄位不印該段標題。
    #[test]
    fn card_arrival_text_has_prefix_title_line_and_sections() {
        let fox = card("fox-id", "狐狸", "{{user}} 認識牠。", "其實是隻妖狐。");
        let text = card_arrival_text(&fox, "阿濤");
        assert_eq!(
            text,
            "（角色回歸）〈狐狸〉\n公開設定：\n阿濤 認識牠。\n私有設定：\n其實是隻妖狐。"
        );

        let no_private = card("fox-id", "狐狸", "公開內容", "");
        assert_eq!(
            card_arrival_text(&no_private, "阿濤"),
            "（角色回歸）〈狐狸〉\n公開設定：\n公開內容"
        );
    }

    /// visibility 洩漏修正（包 4b）：gm_only 事件在 chars 線（`redact_gm_only=true`）只留
    /// 前綴＋標題那一行；GM 線（`redact_gm_only=false`）與非 gm_only 事件一律全文。
    #[test]
    fn lane_event_line_redacts_gm_only_text_for_chars_lane_only() {
        let mut secret = event(
            TranscriptKind::System,
            "",
            "GM",
            "（人物登場）〈密探〉\n只有 GM 知道的全文。",
        );
        secret.gm_only = true;
        assert_eq!(
            lane_event_line(&secret, true),
            "（系統）（人物登場）〈密探〉"
        );
        assert_eq!(
            lane_event_line(&secret, false),
            "（系統）（人物登場）〈密探〉\n只有 GM 知道的全文。"
        );

        let public = event(TranscriptKind::System, "", "GM", "擲骰 3");
        assert_eq!(lane_event_line(&public, true), "（系統）擲骰 3");
    }
}
