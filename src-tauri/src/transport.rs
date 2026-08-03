//! 傳輸層共用介面：上下文組裝→單發呼叫→串流回傳。
//! API 直連與（之後的）CLI 傳輸都必須經由 assemble_messages 取得上下文（KICKOFF §4）。

use crate::data::{
    AppConfig, CharacterCard, DataResult, TableState, Tier, TranscriptEvent, TranscriptKind,
    Visibility, WorldbookEntry,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

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
fn player_fallback_name(lang: &str) -> &'static str {
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
fn replace_st_macros(text: &str, user_name: &str, char_name: Option<&str>) -> String {
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
pub fn assemble_messages(
    card: &CharacterCard,
    player: Option<&CharacterCard>,
    events: &[TranscriptEvent],
    worldbook: &[WorldbookEntry],
    lang: &str,
) -> Vec<ChatMessage> {
    let user_name = player
        .map(|player| player.name.as_str())
        .unwrap_or_else(|| player_fallback_name(lang));
    let mut system = format!(
        "你正在一場多人桌上角色扮演中扮演「{name}」。\
         請一律以「{name}」的第一人稱視角與口吻回應，輸出這個角色的台詞、動作與心理描寫，\
         也可以寫他眼中所見的環境與感受；\
         不要跳出角色、不要以 AI 助理的身分說話、不要替其他角色或玩家代言。\
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
            TranscriptKind::System => ("user", format!("（系統）{}", event.text)),
        };
        push_merged(&mut messages, role, line);
    }
    if !keyword_entries.is_empty() {
        let mut block = "## 你知道的世界情報\n".to_owned();
        for entry in keyword_entries {
            block.push_str(&format!(
                "### {}\n{}\n",
                replace_st_macros(&entry.title, user_name, Some(&card.name)),
                replace_st_macros(&entry.content, user_name, Some(&card.name))
            ));
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
pub fn assemble_gm_messages(
    world_md: &str,
    cards: &[CharacterCard],
    player: Option<&CharacterCard>,
    events: &[TranscriptEvent],
    worldbook: &[WorldbookEntry],
    state: &TableState,
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
    let system = gm_system_prompt(world_md, cards, player, &constant_entries, user_name, lang);

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
    let dynamic = gm_dynamic_block(&keyword_entries, state, user_name, lang);
    if !dynamic.is_empty() {
        // 刻意不走 push_merged：動態塊維持獨立一則的語意邊界，不黏進最後一則發言
        messages.push(message("user", dynamic));
    }
    messages
}

/// GM 的 system prompt 本體：GM 指示＋world.md＋constant 條目＋全卡（含私設）＋玩家卡。
/// assemble_gm_messages（單發）與 gm_lane_system（resume 續聊凍結快照）共用。
fn gm_system_prompt(
    world_md: &str,
    cards: &[CharacterCard],
    player: Option<&CharacterCard>,
    constant_entries: &[&WorldbookEntry],
    user_name: &str,
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
    if !constant_entries.is_empty() {
        system.push_str("\n## 世界書（只進你的上下文）\n");
        for entry in constant_entries {
            system.push_str(&format!(
                "### {}\n{}\n",
                replace_st_macros(&entry.title, user_name, None),
                replace_st_macros(&entry.content, user_name, None)
            ));
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
    system
}

/// GM 的回合動態塊：keyword 條目＋「目前狀態」。
/// assemble_gm_messages（尾端獨立訊息）與 gm_lane_turn（resume 續聊回合尾段）共用。
fn gm_dynamic_block(
    keyword_entries: &[&WorldbookEntry],
    state: &TableState,
    user_name: &str,
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
    if !state.table.is_empty() {
        if !dynamic.is_empty() {
            dynamic.push('\n');
        }
        dynamic.push_str("## 目前狀態（這桌的檯面，接續它往下演）\n");
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
    }
    dynamic.trim_end().to_owned()
}

/// resume 續聊線（prompt-cache-optimization 包 2）的回合尾段。
/// tail 是跟在新事件後送出的動態文字；confidential 是 tail 內回合結束後
/// 要從 session 檔抹掉的子段（chars 線的私設＋限定條目，防洩漏給下一個被點的角色）。
pub struct LaneTurn {
    pub tail: String,
    pub confidential: Option<String>,
}

/// 事件在 lane prompt 裡的一行。續聊線的歷史全部以名字標注成純文字
/// （誰說的靠「X：」前綴分辨，不靠 role），與 session 內既有歷史逐字銜接。
pub fn lane_event_line(event: &TranscriptEvent) -> String {
    match event.kind {
        TranscriptKind::Dialogue | TranscriptKind::Player => {
            format!("{}：{}", event.speaker_name, event.text)
        }
        TranscriptKind::Narration => format!("（旁白）{}", event.text),
        TranscriptKind::System => format!("（系統）{}", event.text),
    }
}

/// chars 線凍結 system（快照）：中性扮演引擎指示＋全部公開角色卡＋玩家卡＋Public constant 條目。
/// 全角色共用一條 session，這一輪演誰由回合尾段指定；私設與限定條目不進快照
/// （E7：凍結 system 動一字整條快取全滅，快照只能放全員共通且穩定的素材）。
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
         每一輪的結尾會指定你這一輪演誰。請一律以被指定角色的第一人稱視角與口吻回應，\
         輸出這個角色的台詞、動作與心理描寫，也可以寫他眼中所見的環境與感受；\
         不要跳出角色、不要以 AI 助理的身分說話、不要替其他角色或玩家代言。\
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
    if !constants.is_empty() {
        system.push_str("\n## 你知道的世界情報\n");
        for entry in constants {
            system.push_str(&format!(
                "### {}\n{}\n",
                replace_st_macros(&entry.title, user_name, None),
                replace_st_macros(&entry.content, user_name, None)
            ));
        }
    }
    system
}

/// chars 線回合尾段：公開 keyword 條目＋機密段（本輪角色的私設＋限定可見條目）＋本輪指定。
/// 機密段回合結束後從 session 檔抹掉；Public constant 條目已在凍結快照，不重複。
pub fn chars_lane_turn(
    card: &CharacterCard,
    player: Option<&CharacterCard>,
    events: &[TranscriptEvent],
    worldbook: &[WorldbookEntry],
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
    if !confidential.is_empty() {
        tail.push_str(&confidential);
        tail.push('\n');
    }
    tail.push_str(&format!(
        "現在你是「{name}」。請直接以「{name}」的第一人稱視角輸出台詞、動作與心理描寫，\
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
    gm_system_prompt(world_md, cards, player, &constants, user_name, lang)
}

/// gm 線回合尾段：keyword 條目＋目前狀態＋導演指示（旁白＋點名合併版，由呼叫端組好傳入）。
pub fn gm_lane_turn(
    events: &[TranscriptEvent],
    worldbook: &[WorldbookEntry],
    player: Option<&CharacterCard>,
    state: &TableState,
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
    let dynamic = gm_dynamic_block(&keyword_entries, state, user_name, lang);
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
pub fn narrate_instruction(lang: &str, roster: &[String], player_name: Option<&str>) -> ChatMessage {
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
    message(
        "user",
        instruction,
    )
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

/// 從 GM 回覆剝出狀態區塊：回傳（欄位對, 剝除後的顯示文字）。
/// 標籤比對一律走 to_ascii_lowercase——full lowercase 會改變某些字母的長度（如土耳其文 İ），
/// 算出的位移拿回原字串切片就會切在非字元邊界上 panic。
pub fn extract_state_block(reply: &str) -> (Vec<(String, String)>, String) {
    let mut display = reply.to_owned();
    let mut blocks = Vec::new();
    let mut removed = false;

    let mut details_cursor = 0;
    while let Some(offset) = display[details_cursor..].to_ascii_lowercase().find("<details") {
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

    for tag in ["status", "updatevariable"] {
        loop {
            let lower = display.to_ascii_lowercase();
            let opening = format!("<{tag}>");
            let closing = format!("</{tag}>");
            let Some(start) = lower.find(&opening) else {
                break;
            };
            let content_start = start + opening.len();
            let Some(end_start) = lower[content_start..]
                .find(&closing)
                .map(|index| content_start + index)
            else {
                break;
            };
            // UpdateVariable 是 MVU 的 JSON patch，第二期才解數值；這期只把它從旁白裡拿掉。
            if tag == "status" {
                blocks.push(display[content_start..end_start].to_owned());
            }
            display.replace_range(start..end_start + closing.len(), "");
            removed = true;
        }
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
        let is_state = matches!(info_lower.as_str(), "state" | "status" | "状态栏" | "狀態欄");
        let is_trailing_plain = info.is_empty() && display[end_start + 3..].trim().is_empty();
        if is_state || is_trailing_plain {
            fences.push((start, end_start + 3, display[header_end + 1..end_start].to_owned()));
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
        return (Vec::new(), reply.to_owned());
    }

    let mut fields = Vec::new();
    for block in blocks {
        for line in block.lines() {
            let line = line.trim_start_matches(|character: char| {
                matches!(character, '-' | '*' | '#' | '+' | '>' | ' ' | '\t')
            });
            let Some((index, _)) = line
                .char_indices()
                .find(|(_, character)| matches!(character, ':' | '：'))
            else {
                continue;
            };
            let key = line[..index].trim();
            let value = line[index + line[index..].chars().next().unwrap().len_utf8()..].trim();
            if key.is_empty() || value.is_empty() {
                continue;
            }
            let normalized = match key.to_ascii_lowercase().as_str() {
                "time" | "時間" | "时间" => "time".to_owned(),
                "place" | "location" | "地點" | "地点" => "place".to_owned(),
                "present" | "在場" | "在场" | "在場人物" | "在场人物" => "present".to_owned(),
                _ => key.to_owned(),
            };
            fields.push((normalized, value.to_owned()));
        }
    }
    (fields, display.trim_end().to_owned())
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
/// created_tokens（寫入快取）是診斷關鍵：命中率 0 時，它 >0 代表「有建但沒讀到」
/// （前綴變了或過了 5 分鐘），=0 代表「根本沒建快取」。
/// OpenRouter 不回報寫入數（官方文件明言不支援），該路徑恆為 0。
/// output_tokens 與 cost_usd 供額度分頁算花費；只有 claude 直接回報金額，其餘為 None。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PromptCacheUsage {
    pub prompt_tokens: u64,
    pub cached_tokens: u64,
    pub created_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: Option<f64>,
}

impl PromptCacheUsage {
    /// 讀自快取的輸入佔總輸入的百分比；沒有輸入時算 0。
    pub fn hit_rate(&self) -> f64 {
        if self.prompt_tokens == 0 {
            0.0
        } else {
            self.cached_tokens as f64 * 100.0 / self.prompt_tokens as f64
        }
    }
}

/// 從一則 SSE payload 取出 usage 統計；增量塊的 `"usage": null` 與缺欄位一律回 None。
/// 欄位名依 OpenRouter usage accounting：usage.prompt_tokens、
/// usage.prompt_tokens_details.cached_tokens（https://openrouter.ai/docs/use-cases/usage-accounting）。
pub fn extract_usage(payload: &str) -> Option<PromptCacheUsage> {
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;
    let usage = value.get("usage")?;
    let prompt_tokens = usage.get("prompt_tokens")?.as_u64()?;
    let cached_tokens = usage
        .get("prompt_tokens_details")
        .and_then(|details| details.get("cached_tokens"))
        .and_then(|tokens| tokens.as_u64())
        .unwrap_or(0);
    Some(PromptCacheUsage {
        prompt_tokens,
        cached_tokens,
        created_tokens: 0, // OpenRouter 不回報寫入數
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

/// chat/completions 請求本體。usage accounting 是 OpenRouter 專屬參數，
/// 只對 OpenRouter 端點帶上：其他 OpenAI-compatible 端點不認得頂層 "usage"，
/// 寬鬆的（ollama／LM Studio）會忽略，嚴格的（OpenAI 官方）會直接拒絕請求。
/// anthropic/ 系模型另走顯式快取斷點（見 anthropic_messages）；
/// 兩者皆不適用時，請求形狀必須與加這些功能前逐位元相同。
fn chat_request_body(
    model: &str,
    messages: &[ChatMessage],
    include_usage: bool,
) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": true,
    });
    if model.starts_with("anthropic/") {
        body["messages"] = anthropic_messages(messages);
    }
    if include_usage {
        body["usage"] = serde_json::json!({ "include": true });
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

/// 單發呼叫 OpenAI-compatible chat/completions（SSE 串流），
/// 每個增量經 on_delta 回傳，結束後回傳完整文字。
/// usage_log 給路徑就把這次呼叫的用量追加成一行 JSONL（見 crate::usage_log）。
pub async fn stream_chat(
    config: &AppConfig,
    model: &str,
    messages: &[ChatMessage],
    usage_log: Option<&std::path::Path>,
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

    // 命中率量測（prompt-cache-optimization C）：只對 OpenRouter 端點開 usage accounting
    let include_usage = base.contains("openrouter.ai");
    let mut request = reqwest::Client::new()
        .post(format!("{base}/chat/completions"))
        .json(&chat_request_body(model, messages, include_usage));
    if let Some(key) = api_key {
        request = request.bearer_auth(key);
    }

    let response = request.send().await?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API 回應 {status}：{body}").into());
    }

    let mut stream = response.bytes_stream();
    let mut parser = SseParser::default();
    let mut full_text = String::new();
    let mut usage = None;
    'outer: while let Some(chunk) = stream.next().await {
        for payload in parser.push(&chunk?) {
            if payload == "[DONE]" {
                break 'outer;
            }
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
            "[prompt-cache] transport=api model={model} prompt_tokens={} cached_tokens={} created_tokens={} hit_rate={:.0}%",
            usage.prompt_tokens,
            usage.cached_tokens,
            usage.created_tokens,
            usage.hit_rate(),
        );
        if let Some(path) = usage_log {
            crate::usage_log::append_call(path, "api", model, None, usage);
        }
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
        return Err(format!(
            "API 回應 {status}：{}",
            body.chars().take(200).collect::<String>()
        ));
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
    use crate::data::Tier;

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
            ts: "2026-07-19T12:00:00+08:00".to_owned(),
            speaker_id: speaker_id.to_owned(),
            speaker_name: speaker_name.to_owned(),
            kind,
            text: text.to_owned(),
            state: None,
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
        }
    }

    /// 有玩家卡時，角色與 GM 都要認得玩家的名字與公開身份（本功能的核心）
    #[test]
    fn player_card_enters_character_and_gm_context() {
        let fox = card("fox-id", "狐狸", "旅店老闆", "通緝犯");
        let player = card("player-id", "阿濤", "遠道而來的商隊護衛", "");

        let character_system = &assemble_messages(&fox, Some(&player), &[], &[], "zh-TW")[0].content;
        assert!(character_system.contains("阿濤"));
        assert!(character_system.contains("遠道而來的商隊護衛"));

        let gm_system =
            &assemble_gm_messages(
                "世界總覽",
                &[fox],
                Some(&player),
                &[],
                &[],
                &TableState::default(),
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
        let (name, _) = extract_next_speaker(format!("門開了。\n下一位：{PLAYER_SENTINEL}").as_str());
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
        let mut entry = worldbook_entry(0, "{{USER}} 的情報", &[], true, 0, false, Visibility::Public);
        entry.content = "{{user}} 來過這裡。".to_owned();

        let character = assemble_messages(&fox, Some(&player), &[], &[entry.clone()], "zh-TW");
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
            "zh-TW",
        );
        assert!(gm[0].content.contains("### 阿濤 的情報\n阿濤 來過這裡。"));
        assert!(gm[0].content.contains("世界 {{CHAR}}"));
    }

    #[test]
    fn st_macros_fall_back_to_localized_player_name_without_player_card() {
        let fox = card("fox-id", "狐狸", "{{user}}", "");

        let zh = assemble_messages(&fox, None, &[], &[], "zh-TW");
        assert!(zh[0].content.contains("你的公開設定（其他人也認識的你）\n玩家\n"));

        let en = assemble_messages(&fox, None, &[], &[], "en");
        assert!(en[0].content.contains("你的公開設定（其他人也認識的你）\nPlayer\n"));
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
        let character = assemble_messages(&fox, None, &events, &[], "zh-TW");
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
        let fox_messages = assemble_messages(&fox, None, &[], &entries, "zh-TW");
        let fox_system = &fox_messages[0].content;
        assert!(fox_system.contains("\n## 你知道的世界情報\n"));
        assert!(fox_system.contains("### 公開情報\n公開情報內容\n"));
        assert!(fox_system.contains("### 狐狸情報\n狐狸情報內容\n"));
        assert!(!fox_system.contains("GM 祕密"));
        assert!(!fox_system.contains("騎士情報"));

        let knight = card("knight-id", "騎士", "公開", "私有");
        let knight_system = &assemble_messages(&knight, None, &[], &entries, "zh-TW")[0].content;
        assert!(knight_system.contains("公開情報"));
        assert!(knight_system.contains("騎士情報"));
        assert!(!knight_system.contains("狐狸情報"));

        let gm_system =
            &assemble_gm_messages(
                "世界總覽",
                &[],
                None,
                &[],
                &entries,
                &TableState::default(),
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
        let messages = assemble_messages(&fox, None, &events, &[], "zh-TW");

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
        let messages = assemble_messages(&fox, None, &events, &[], "zh-TW");
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

        let first_messages = assemble_messages(&first, None, &events, &[], "zh-TW");
        let roles: Vec<&str> = first_messages.iter().map(|m| m.role.as_str()).collect();
        // 自己的那句是 assistant，對方同名的那句仍是 user（不會相鄰合併成一則）
        assert_eq!(roles, ["system", "assistant", "user"]);
        assert_eq!(first_messages[1].content, "我是第一位");
        assert_eq!(first_messages[2].content, "重名：我是第二位");

        let second = card(second_id, "重名", "第二位", "");
        let second_messages = assemble_messages(&second, None, &events, &[], "zh-TW");
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
        let messages =
            assemble_gm_messages(
                "酒館位於邊境小鎮",
                &cards,
                None,
                &events,
                &[],
                &TableState::default(),
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
        let zh = assemble_messages(&fox, None, &[], &[], "zh-TW");
        assert!(zh[0].content.contains("繁體中文"));
        let en = assemble_messages(&fox, None, &[], &[], "en");
        assert!(en[0].content.contains("in natural, fluent English"));
        assert!(!en[0].content.contains("繁體中文"));

        let gm_en = assemble_gm_messages(
            "",
            &[],
            None,
            &[],
            &[],
            &TableState::default(),
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
            let messages = assemble_messages(&fox, None, &[], &[], lang);
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
        let error = stream_chat(&config, "test/model", &messages, None, |_| {})
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
        let full = stream_chat(&config, "test/model", &messages, None, |delta| {
            deltas.push(delta.to_owned());
        })
        .await
        .unwrap();
        assert_eq!(full, "你好");
        assert_eq!(deltas, ["你", "好"]);
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

    /// 命中率量測（prompt-cache-optimization C）：usage accounting 只對 OpenRouter 開；
    /// 其他端點的請求本體必須與加此功能前逐位元相同（嚴格端點會拒絕未知參數）。
    #[test]
    fn chat_request_body_adds_usage_only_when_asked_and_stays_bytewise_identical_otherwise() {
        let messages = [message("user", "嗨".to_owned())];
        let plain = chat_request_body("test/model", &messages, false);
        // 與舊版 stream_chat 內聯的 json! 完全同一種構造：內容相等即代表位元組相同
        assert_eq!(
            plain,
            serde_json::json!({
                "model": "test/model",
                "messages": [{"role": "user", "content": "嗨"}],
                "stream": true,
            })
        );
        assert!(plain.get("usage").is_none());

        let with_usage = chat_request_body("test/model", &messages, true);
        assert_eq!(with_usage["usage"], serde_json::json!({ "include": true }));
        assert_eq!(with_usage["model"], "test/model");
        assert_eq!(with_usage["stream"], serde_json::Value::Bool(true));
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
        let body = chat_request_body("anthropic/claude-sonnet-4.5", &messages, true);
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
        let plain = chat_request_body("test/model", &messages, false);
        assert!(plain["messages"][0]["content"].is_string());

        // 開桌第一輪沒有 assistant：只標 system，不出錯
        let fresh = [
            message("system", "設定".to_owned()),
            message("user", "嗨".to_owned()),
        ];
        let fresh_body = chat_request_body("anthropic/claude-haiku", &fresh, false);
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
                cached_tokens: 150,
                created_tokens: 0, // OpenRouter 不回報寫入數
                output_tokens: 2,
                cost_usd: None, // 金額只有 claude CLI 直接回報
            }
        );

        // 缺 details（隱式快取沒命中時部分供應商省略）：cached 記 0，不當錯誤
        let without_details =
            extract_usage(r#"{"usage":{"prompt_tokens":10,"completion_tokens":1}}"#).unwrap();
        assert_eq!(without_details.cached_tokens, 0);

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
        let full = stream_chat(&config, "test/model", &messages, Some(&log_path), |delta| {
            deltas.push(delta.to_owned());
        })
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
            characters: std::collections::BTreeMap::new(),
        };
        let gm = assemble_gm_messages("", &[fox.clone()], None, &[], &[], &state, "zh-TW");
        // 快取友善：狀態每輪更新，搬到尾端獨立 user 訊息，system 不再內嵌
        assert!(!gm[0].content.contains("目前狀態"));
        let tail = gm.last().unwrap();
        assert_eq!(tail.role, "user");
        assert!(tail.content.contains("## 目前狀態"));
        assert!(tail.content.contains("時間：午夜"));
        assert!(tail.content.contains("沦陷天数：第 3 天"));

        let character = assemble_messages(&fox, None, &[], &[], "zh-TW");
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
            worldbook_entry(1, "龍的傳說", &["dragon"], false, 1, false, Visibility::Public),
        ];
        let events = [event(TranscriptKind::Player, "", "玩家", "we saw a DRAGON")];
        let fox = card("fox-id", "狐狸", "公開", "");

        let character = assemble_messages(&fox, None, &events, &entries, "zh-TW");
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
        let gm = assemble_gm_messages("世界", &[fox], None, &events, &entries, &state, "zh-TW");
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
        round1_state.table.insert("time".to_owned(), "黃昏".to_owned());
        let mut round2_state = TableState::default();
        round2_state.table.insert("time".to_owned(), "午夜".to_owned());

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
            "zh-TW",
        );
        let gm2 = assemble_gm_messages(
            "世界",
            std::slice::from_ref(&fox),
            None,
            &round2_events,
            &entries,
            &round2_state,
            "zh-TW",
        );
        // gm1 去尾端動態塊；gm2 去尾端動態塊＋最新事件——前綴逐字相同
        assert_eq!(gm1[..gm1.len() - 1], gm2[..gm2.len() - 2]);
        // 兩輪動態塊確實不同（條目進出＋狀態更新），只影響尾端一則
        assert_ne!(gm1.last(), gm2.last());

        // 角色路徑同理；新事件用自己的台詞（assistant），才不會與前一則 user 合併
        let mut round2_character_events = round1_events.clone();
        round2_character_events.push(event(TranscriptKind::Dialogue, "fox-id", "狐狸", "撤到 dock 去"));
        let character1 = assemble_messages(&fox, None, &round1_events, &entries, "zh-TW");
        let character2 = assemble_messages(&fox, None, &round2_character_events, &entries, "zh-TW");
        assert_eq!(character1[..character1.len() - 1], character2[..character2.len() - 2]);
        assert_ne!(character1.last(), character2.last());
    }

    #[test]
    fn extract_state_fence_returns_fields_and_hides_fence() {
        let (fields, display) = extract_state_block(
            "雨停了。\n```state\ntime: 午夜\nplace：舊碼頭\npresent: 阿濤、船長\n```",
        );
        assert_eq!(
            fields,
            vec![
                ("time".to_owned(), "午夜".to_owned()),
                ("place".to_owned(), "舊碼頭".to_owned()),
                ("present".to_owned(), "阿濤、船長".to_owned()),
            ]
        );
        assert_eq!(display, "雨停了。");
    }

    #[test]
    fn extract_state_discards_bad_lines_without_losing_valid_fields() {
        let (fields, display) = extract_state_block(
            "旁白\n```state\n- time: 清晨\n沒有冒號\nplace:   \n# 自訂：有效\n```",
        );
        assert_eq!(
            fields,
            vec![
                ("time".to_owned(), "清晨".to_owned()),
                ("自訂".to_owned(), "有效".to_owned()),
            ]
        );
        assert_eq!(display, "旁白");
    }

    #[test]
    fn extract_state_details_summary_is_parsed_and_hidden() {
        let (fields, display) = extract_state_block(
            "港口傳來鐘聲。<details><summary>状态栏</summary>时间：黃昏\n地点：港口</details>",
        );
        assert_eq!(
            fields,
            vec![
                ("time".to_owned(), "黃昏".to_owned()),
                ("place".to_owned(), "港口".to_owned()),
            ]
        );
        assert_eq!(display, "港口傳來鐘聲。");
    }

    #[test]
    fn extract_status_tag_is_parsed_and_hidden() {
        let (fields, display) =
            extract_state_block("門開了。<STATUS>time: 午夜\nplace: 走廊</status>剩下的話。");
        assert_eq!(
            fields,
            vec![
                ("time".to_owned(), "午夜".to_owned()),
                ("place".to_owned(), "走廊".to_owned()),
            ]
        );
        assert_eq!(display, "門開了。剩下的話。");
    }

    #[test]
    fn extract_update_variable_hides_json_without_parsing_it() {
        let (fields, display) = extract_state_block(
            "她點頭。<UpdateVariable>{\"time\":\"午夜\"}</UpdateVariable>",
        );
        assert!(fields.is_empty());
        assert_eq!(display, "她點頭。");
    }

    #[test]
    fn extract_state_keeps_unwrapped_narration_byte_for_byte() {
        let reply = "純旁白\n保留尾端空行\n\n";
        assert_eq!(extract_state_block(reply), (Vec::new(), reply.to_owned()));
    }

    #[test]
    fn extract_state_keeps_middle_code_fence_but_removes_trailing_plain_fence() {
        let reply = "提示：\n```rust\nlet time = 1;\n```\n旁白\n```\ntime: 午夜\n```";
        let (fields, display) = extract_state_block(reply);
        assert_eq!(fields, vec![("time".to_owned(), "午夜".to_owned())]);
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
            worldbook_entry(4, "關鍵字條目", &["寶箱"], false, 0, false, Visibility::Public),
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
        let events = [event(TranscriptKind::Player, "", "阿濤", "打開寶箱，讀羊皮卷")];
        let entries = [
            worldbook_entry(1, "公開常識", &[], true, 0, false, Visibility::Public),
            worldbook_entry(2, "寶箱情報", &["寶箱"], false, 0, false, Visibility::Public),
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
        let turn = chars_lane_turn(&fox, None, &events, &entries, "zh-TW");
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
        let plain = chars_lane_turn(&knight, None, &events, &entries[..2], "zh-TW");
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
            worldbook_entry(2, "寶箱情報", &["寶箱"], false, 0, false, Visibility::Public),
        ];
        let snapshot = gm_lane_system("世界總覽", &[fox], None, &entries, "zh-TW");
        assert!(snapshot.contains("世界總覽"));
        assert!(snapshot.contains("GM專有內容"));
        assert!(snapshot.contains("通緝犯"));
        assert!(!snapshot.contains("寶箱情報"));

        let mut state = TableState::default();
        state
            .table
            .insert("place".to_owned(), "酒館".to_owned());
        let turn = gm_lane_turn(&events, &entries, None, &state, "（導演指示）請插入旁白。", "zh-TW");
        assert!(turn.confidential.is_none());
        assert!(turn.tail.contains("寶箱情報內容"));
        assert!(turn.tail.contains("地點：酒館"));
        assert!(turn.tail.ends_with("（導演指示）請插入旁白。"));
    }

    #[test]
    fn lane_event_line_labels_every_kind_by_name() {
        assert_eq!(
            lane_event_line(&event(TranscriptKind::Dialogue, "fox-id", "狐狸", "晚安")),
            "狐狸：晚安"
        );
        assert_eq!(
            lane_event_line(&event(TranscriptKind::Player, "", "阿濤", "好啊")),
            "阿濤：好啊"
        );
        assert_eq!(
            lane_event_line(&event(TranscriptKind::Narration, "", "GM", "夜深了")),
            "（旁白）夜深了"
        );
        assert_eq!(
            lane_event_line(&event(TranscriptKind::System, "", "", "擲骰 3")),
            "（系統）擲骰 3"
        );
    }
}
