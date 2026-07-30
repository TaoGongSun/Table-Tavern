//! 傳輸層共用介面：上下文組裝→單發呼叫→串流回傳。
//! API 直連與（之後的）CLI 傳輸都必須經由 assemble_messages 取得上下文（KICKOFF §4）。

use crate::data::{
    AppConfig, CharacterCard, DataResult, Tier, TranscriptEvent, TranscriptKind, Visibility,
    WorldbookEntry,
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
pub fn assemble_messages(
    card: &CharacterCard,
    player: Option<&CharacterCard>,
    events: &[TranscriptEvent],
    worldbook: &[WorldbookEntry],
    lang: &str,
) -> Vec<ChatMessage> {
    let mut system = format!(
        "你正在一場多人桌上角色扮演中扮演「{name}」。\
         請一律以「{name}」的第一人稱視角與口吻回應，只輸出這個角色的台詞與動作描寫；\
         不要跳出角色、不要以 AI 助理的身分說話、不要替其他角色或玩家代言。\
         {language_rule}\n\n\
         ## 你的公開設定（其他人也認識的你）\n{public}\n",
        name = card.name,
        language_rule = language_rule(lang),
        public = card.public_md.trim(),
    );
    if !card.private_md.trim().is_empty() {
        system.push_str(&format!(
            "\n## 你的私有設定（只有你自己知道；除非劇情走到，不要主動說破）\n{}\n",
            card.private_md.trim()
        ));
    }
    if let Some(player) = player {
        system.push_str(&format!(
            "\n## 同桌的玩家（真人扮演的角色，逐字稿裡的「{}」就是他）",
            player.name
        ));
        if !player.public_md.trim().is_empty() {
            system.push_str(&format!("\n{}\n", player.public_md.trim()));
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
    let active = active_worldbook_entries(&visible, events);
    if !active.is_empty() {
        system.push_str("\n## 你知道的世界情報\n");
        for entry in active {
            system.push_str(&format!("### {}\n{}\n", entry.title, entry.content));
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
    messages
}

/// 點名時「輪到玩家」的內部代號；前端以它停下 GM 推進回合。
/// 刻意用不可能當人名的字串：玩家卡或某張 NPC 卡都可能就叫「玩家」。
pub const PLAYER_SENTINEL: &str = "__PLAYER__";

/// 組裝 GM 上下文：world.md（只有 GM 看得到）＋全部角色卡（含私有，NewPlan §7.0）
/// ＋公開 transcript。GM 自己的旁白是 assistant，其餘事件是 user。
pub fn assemble_gm_messages(
    world_md: &str,
    cards: &[CharacterCard],
    player: Option<&CharacterCard>,
    events: &[TranscriptEvent],
    worldbook: &[WorldbookEntry],
    lang: &str,
) -> Vec<ChatMessage> {
    let mut system = format!(
        "你是這場多人桌上角色扮演的 GM（導演兼旁白）。你負責描述場景與世界反應、\
         推進劇情節奏、決定下一位發言者，並防止對話停滯或重複。\
         旁白是所有人都聽得到的公開敘事：不要替任何角色或玩家代言；\
         世界設定與角色私有設定只有你知道全貌，劇情尚未揭露的內容不要說破。\
         {language_rule}\n",
        language_rule = language_rule(lang),
    );
    if !world_md.trim().is_empty() {
        system.push_str(&format!(
            "\n## 世界設定（只進你的上下文，角色只知道你說出口的內容）\n{}\n",
            world_md.trim()
        ));
    }
    let active = active_worldbook_entries(worldbook, events);
    if !active.is_empty() {
        system.push_str("\n## 世界書（只進你的上下文）\n");
        for entry in active {
            system.push_str(&format!("### {}\n{}\n", entry.title, entry.content));
        }
    }
    if !cards.is_empty() {
        system.push_str("\n## 登場角色\n");
        for card in cards {
            system.push_str(&format!("### {}\n", card.name));
            if !card.public_md.trim().is_empty() {
                system.push_str(&format!("公開設定：\n{}\n", card.public_md.trim()));
            }
            if !card.private_md.trim().is_empty() {
                system.push_str(&format!(
                    "私有設定（僅你與該角色知道）：\n{}\n",
                    card.private_md.trim()
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
            system.push_str(&format!("\n{}\n", player.public_md.trim()));
        }
    }

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
    messages
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

/// 導演指示：插入旁白（附加在 GM 上下文最後）
pub fn narrate_instruction() -> ChatMessage {
    message(
        "user",
        "（導演指示）請插入一段旁白：描述場景變化、世界反應或劇情推進，\
         篇幅不設限，依劇情需要自由發揮。只輸出旁白本文，不要替任何角色說話。"
            .to_owned(),
    )
}

/// 導演指示：從名單選出下一位發言者
pub fn suggest_instruction(roster: &[String], player_name: Option<&str>) -> ChatMessage {
    message(
        "user",
        format!(
            "（導演指示）根據目前劇情，從名單中選出下一位最適合發言的角色：{}。\
             若現在應該輪到玩家{player}行動，就輸出：{PLAYER_SENTINEL}。只輸出名字，不要任何其他文字。",
            roster.join("、"),
            // 沒玩家卡時是空字串，句子與加玩家卡前逐字相同
            player = player_name.map_or(String::new(), |name| format!("（{name}）")),
        ),
    )
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
pub async fn stream_chat(
    config: &AppConfig,
    model: &str,
    messages: &[ChatMessage],
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
        .json(&serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": true,
        }));
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
    'outer: while let Some(chunk) = stream.next().await {
        for payload in parser.push(&chunk?) {
            if payload == "[DONE]" {
                break 'outer;
            }
            if let Some(delta) = extract_delta(&payload) {
                on_delta(&delta);
                full_text.push_str(&delta);
            }
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
            &assemble_gm_messages("世界總覽", &[fox], Some(&player), &[], &[], "zh-TW")[0].content;
        assert!(gm_system.contains("阿濤"));
        assert!(gm_system.contains("遠道而來的商隊護衛"));

        // 點名 prompt 要告知玩家名字，GM 才知道喊誰；哨兵本身不變
        let instruction = suggest_instruction(&["狐狸".to_owned()], Some("阿濤")).content;
        assert!(instruction.contains("阿濤"));
        assert!(instruction.contains(PLAYER_SENTINEL));
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

        let gm = assemble_gm_messages("世界總覽", &[fox], None, &events, &[], "zh-TW");
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
            &assemble_gm_messages("世界總覽", &[], None, &[], &entries, "zh-TW")[0].content;
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
            assemble_gm_messages("酒館位於邊境小鎮", &cards, None, &events, &[], "zh-TW");

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

        let gm_en = assemble_gm_messages("", &[], None, &[], &[], "en");
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
            let gm = assemble_gm_messages("", &[], None, &[], &[], lang);
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
        let error = stream_chat(&config, "test/model", &messages, |_| {})
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
        let full = stream_chat(&config, "test/model", &messages, |delta| {
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
}
