use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::sync::OnceLock;

use super::transcript::TranscriptEvent;

/// 逐字稿事件的固定標頭代碼：事件 `text` 只存本文，標頭在顯示、匯出、送 AI 時才照語系組出來，
/// 程式比對（本幕誰登場過）只認這個代碼。代碼落檔即持久契約，不得改名。
/// 認不得的 type（新版資料、外部修改）讀成 `Unknown`：不出標頭、不參與比對，重寫整行時原欄位會流失。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventMarker {
    SceneSummary,
    CardArrival {
        name: String,
    },
    PersonArrival {
        title: String,
    },
    CardPrivate {
        name: String,
    },
    StateUpdate,
    /// name 空字串＝點名玩家且玩家沒名字，組字時退回該語系的玩家稱呼。
    GmCall {
        name: String,
    },
    #[serde(other)]
    Unknown,
}

/// 沒有玩家名時的稱呼（提示詞、匯出、點名標頭共用）。
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

/// 送 AI 的骨架語系〔作者裁決 2026-10-03〕：zh* 出繁中、其餘（含未知）出英文。
/// 只用來挑骨架字；輸出語言與玩家稱呼一律吃原始語系。
pub fn prompt_lang(lang: &str) -> &'static str {
    if lang.starts_with("zh") {
        "zh-TW"
    } else {
        "en"
    }
}

/// 收斂成十語系之一：zh* 退繁中、未知退英文。
pub(crate) fn lang_key(lang: &str) -> &'static str {
    match lang {
        "zh-CN" => "zh-CN",
        "ja" => "ja",
        "ko" => "ko",
        "es" => "es",
        "pt-BR" => "pt-BR",
        "de" => "de",
        "fr" => "fr",
        "ru" => "ru",
        _ if lang.starts_with("zh") => "zh-TW",
        _ => "en",
    }
}

/// 十語系標頭字典，前端（`src/i18n/features/transcript-marker.ts`）讀同一份。
const DICTIONARY: &str = include_str!("../../../../src/shared/contracts/transcript-marker.json");

type Dictionary = HashMap<String, HashMap<String, String>>;

fn dictionary() -> &'static Dictionary {
    static PARSED: OnceLock<Dictionary> = OnceLock::new();
    PARSED.get_or_init(|| serde_json::from_str(DICTIONARY).unwrap_or_default())
}

/// 取該語系的模板並單次代入 `{name}`／`{title}`；字典缺字回空字串（測試保證十語系齊全）。
fn template(lang: &str, key: &str, value: Option<(&str, &str)>) -> String {
    let text = dictionary()
        .get(lang_key(lang))
        .and_then(|entries| entries.get(key))
        .map(String::as_str)
        .unwrap_or_default();
    match value {
        Some((placeholder, value)) => text.replacen(&format!("{{{placeholder}}}"), value, 1),
        None => text.to_owned(),
    }
}

/// 標頭第一行；`Unknown` 沒有標頭。
pub fn marker_heading(marker: &EventMarker, lang: &str) -> Option<String> {
    Some(match marker {
        EventMarker::SceneSummary => template(lang, "scene_summary", None),
        EventMarker::CardArrival { name } => template(lang, "card_arrival", Some(("name", name))),
        EventMarker::PersonArrival { title } => {
            template(lang, "person_arrival", Some(("title", title)))
        }
        EventMarker::CardPrivate { name } => template(lang, "card_private", Some(("name", name))),
        EventMarker::StateUpdate => template(lang, "state_update", None),
        EventMarker::GmCall { name } => {
            let name = if name.trim().is_empty() {
                player_fallback_name(lang)
            } else {
                name
            };
            template(lang, "gm_call", Some(("name", name)))
        }
        EventMarker::Unknown => return None,
    })
}

/// 標頭＋還原的段標與換行＋本文；沒有標頭就是本文原樣。
pub fn event_full_text(event: &TranscriptEvent, lang: &str) -> String {
    let Some(marker) = &event.marker else {
        return event.text.clone();
    };
    let Some(heading) = marker_heading(marker, lang) else {
        return event.text.clone();
    };
    let text = &event.text;
    match marker {
        EventMarker::CardArrival { .. } if text.trim().is_empty() => heading,
        EventMarker::CardArrival { .. } => {
            format!(
                "{heading}\n{}\n{text}",
                template(lang, "public_profile", None)
            )
        }
        EventMarker::CardPrivate { .. } => {
            format!(
                "{heading}\n{}\n{text}",
                template(lang, "private_profile", None)
            )
        }
        EventMarker::GmCall { .. } => heading,
        _ if text.is_empty() => heading,
        _ => format!("{heading}\n{text}"),
    }
}

/// 本幕已回歸的角色卡名。
pub fn appeared_card_names(events: &[TranscriptEvent]) -> BTreeSet<String> {
    events
        .iter()
        .filter_map(|event| match &event.marker {
            Some(EventMarker::CardArrival { name }) => Some(name.clone()),
            _ => None,
        })
        .collect()
}

/// 本幕已登場的世界書人物標題。
pub fn appeared_person_titles(events: &[TranscriptEvent]) -> BTreeSet<String> {
    events
        .iter()
        .filter_map(|event| match &event.marker {
            Some(EventMarker::PersonArrival { title }) => Some(title.clone()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests;
