use serde::{Deserialize, Serialize};



/// 角色與 GM system prompt 共用的語言規範，依使用者語系（config.preferences.language）注入。
/// 規範一律用該語言本身書寫，模型才不會被中文提示詞模板帶成中文輸出；
/// 兩岸中文互相加反向禁令，防止字體與用語飄移。
/// 新增語系：在此加一條 match arm，並補前端字典（src/i18n/）與範例桌（src-tauri/samples/）。
pub(super) fn language_rule(lang: &str) -> &'static str {
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

pub(super) fn message(role: &str, content: String) -> ChatMessage {
    ChatMessage {
        role: role.to_owned(),
        content,
    }
}

/// 相鄰同 role 合併成一則，維持 user/assistant 交錯
pub(super) fn push_merged(messages: &mut Vec<ChatMessage>, role: &str, line: String) {
    match messages.last_mut() {
        Some(last) if last.role == role => {
            last.content.push('\n');
            last.content.push_str(&line);
        }
        _ => messages.push(message(role, line)),
    }
}
