//! GM 回覆拆解：狀態區塊、點名行、照抄段與旁白標籤只從落檔正文剝。
use super::gm::GmNarration;
use crate::{data, mechanism, transport};

/// 完成回合的 GM 回覆拆解：機制吃原文剝出的 `block`，點名行，落檔正文與原文。
pub(super) struct GmReplyParts {
    pub(super) block: transport::StateBlock,
    pub(super) next_raw: Option<String>,
    pub(super) display: String,
    /// 正文與原文不同時存原文（卡片介面重畫、回溯用）
    pub(super) raw: Option<String>,
    /// 正文剝掉、提交後要核對有沒有收進狀態的格子（照抄段的新值；退場桌另加本回合沒正式回報的卡片狀態欄格子）
    pub(super) pending: Vec<mechanism::Cell>,
    /// 照抄段新值合成的更新區塊（已併進 `block.updates`）；zod 桌接在送引擎的回覆後面
    pub(super) echo_patch: Option<String>,
}

/// 剝狀態區塊→剝點名行→剝尾端照抄的狀態變動段→剝開頭照抄的旁白標籤（只動顯示；raw 與 `block` 都來自原文）。
pub(super) fn gm_reply_parts(
    reply: &str,
    tree: &std::collections::BTreeMap<String, data::StateNode>,
    retired: bool,
) -> GmReplyParts {
    let mut block = transport::extract_state_block(reply);
    let (next_raw, display) = transport::extract_next_speaker(&block.display);
    let (display, echo_lines) = match transport::strip_state_echo(&display, tree) {
        Some(echo) => (echo.text, echo.lines),
        None => (display, Vec::new()),
    };
    // 照抄段裡本回合沒報的新值走各桌的提交路徑補收（playdata-loss-fixes 11a）
    let (mut pending, echo_patch) = mechanism::echo_patch(&echo_lines, &block, tree);
    if let Some(patch) = &echo_patch {
        block.updates.push(patch.clone());
    }
    // 退場桌照舊印的卡片狀態欄：正文剝掉，提交後沒收進樹的格子留進狀態更新事件（playdata-loss-fixes D1）
    if retired {
        pending.extend(mechanism::retired_card_pending(&block, tree));
    }
    let display = transport::strip_narrator_label(&display).unwrap_or(display);
    let raw = (reply != display).then(|| reply.to_owned());
    GmReplyParts {
        block,
        next_raw,
        display,
        pending,
        echo_patch,
        raw,
    }
}

/// 中止的旁白：半截原文照正常剝法留成 text，其餘寫入一律空著。
pub(super) fn aborted_narration(reply: &str) -> GmNarration {
    // 自閉合標籤先拿掉（免得配到後面的閉標籤），剝完照中止規則切掉沒閉合的尾巴
    let block = transport::extract_state_block(&transport::strip_self_closing_controls(reply));
    let (_next, display) = transport::extract_next_speaker(&block.display);
    GmNarration {
        text: {
            let text = transport::cut_unclosed_tail(&display, true).trim_end();
            transport::strip_narrator_label(text).unwrap_or_else(|| text.to_owned())
        },
        raw: None,
        next: None,
        state_updates: Vec::new(),
        arrived_characters: Vec::new(),
        arrived_persons: Vec::new(),
        aborted: true,
        state_error: None,
    }
}

#[cfg(test)]
#[path = "gm_reply_tests.rs"]
mod tests;
