//! 卡片開場白怎麼進提示詞（greeting-context-st-parity，規格見 .ai/plans/greeting-context-st-parity.md）。
//! ST 只把選中的開場白當聊天第 0 則送原文、人設不含開場白；我們匯入時把開場白存進卡片的段落
//! （`### 開場白`、`### 備用開場白 n`），組提示詞時照這裡的政策剝掉，卡檔與匯出不動。

use crate::{data, transport};

/// 這一桌這一刻的開場白政策。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GreetingPolicy {
    /// 送進提示詞的卡（GM、角色線、回歸事件）剝掉開場白段。
    pub strip_cards: bool,
    /// GM 送出層：本幕開場送原文、換幕後的格式範例。
    pub gm: transport::GmOpening,
}

/// - 接管桌與卡片狀態欄退場的桌：只剝卡，開場照 text、沒有範例（作者裁決 2026-10-28）。
/// - 本幕有開場事件：剝卡，GM 那份開場送原文。
/// - 本幕沒有、本分支更早的幕有：剝卡，GM system 放那則原文當格式範例。
/// - 這條分支完全沒貼過開場白：維持卡片段落（GM 沒有別的開場內容可看）。
pub(crate) fn policy(
    root: &std::path::Path,
    world_id: &str,
    state: &data::WorldState,
    events: &[data::TranscriptEvent],
    takeover: bool,
) -> GreetingPolicy {
    if takeover || state.statusbar_retired {
        return GreetingPolicy {
            strip_cards: true,
            gm: transport::GmOpening::default(),
        };
    }
    if events.iter().any(|event| event.opening) {
        return GreetingPolicy {
            strip_cards: true,
            gm: transport::GmOpening {
                raw_history: true,
                example: None,
            },
        };
    }
    // 讀不到更早的幕：當作沒貼過，維持卡片段落（不會比現況差）
    match data::branch_opening(root, world_id, state, state.current_scene) {
        Ok(Some(opening)) => GreetingPolicy {
            strip_cards: true,
            gm: transport::GmOpening {
                raw_history: false,
                example: Some(opening.raw.unwrap_or(opening.text)),
            },
        },
        Ok(None) => GreetingPolicy::default(),
        Err(error) => {
            log::warn!("開場白政策：讀不到更早的幕，維持卡片段落：{error}");
            GreetingPolicy::default()
        }
    }
}

/// 只要「剝不剝卡」的呼叫端（角色回合、回歸事件、量測以外的卡清單）：自己讀狀態與本幕逐字稿。
/// 讀不到狀態時照不剝（現況）。
pub(crate) fn strip_card_greetings(root: &std::path::Path, world_id: &str) -> bool {
    let Ok(state) = data::read_state(root, world_id) else {
        return false;
    };
    let events = data::read_transcript(root, world_id, state.current_scene).unwrap_or_default();
    let takeover = data::is_interface_takeover(root, world_id, state.refactor_mode.as_deref());
    policy(root, world_id, &state, &events, takeover).strip_cards
}

#[cfg(test)]
mod tests;
