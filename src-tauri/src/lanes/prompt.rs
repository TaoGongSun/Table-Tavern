//! 續聊線本輪 prompt 的組字（開線全量、續聊增量）與各線的渲染視角。

use super::Lane;
use crate::data::TranscriptEvent;
use crate::transport;

/// 組本輪 prompt：水位之後的新事件＋回合尾段。開線（全量重建）帶對話紀錄標頭，
/// 形狀比照單發 flatten；續聊只送增量，與 session 內既有歷史逐字銜接。
/// `lane`：chars 線走角色側渲染（略過角色私設事件、遮 gm_only），
/// GM 線一律全文。略過只發生在渲染，水位與指紋仍以原事件序列計。
pub(crate) fn build_prompt(
    events: &[TranscriptEvent],
    base: usize,
    tail: &str,
    opening: bool,
    lane: Lane,
    lang: &str,
) -> String {
    build_prompt_as(events, base, tail, opening, lane_side(lane, false), lang)
}

/// 線的渲染視角：chars 線走角色側；GM 線全文，`opening_raw` 時開場事件送原文（`transport::GmOpening`）。
pub(crate) fn lane_side(lane: Lane, opening_raw: bool) -> transport::Side {
    match (lane, opening_raw) {
        (Lane::Chars, _) => transport::Side::Character,
        (Lane::Gm, false) => transport::Side::Gm,
        (Lane::Gm, true) => transport::Side::GmOpeningRaw,
    }
}

/// `build_prompt`，視角由呼叫端給（GM 線依開場白政策）。
pub(crate) fn build_prompt_as(
    events: &[TranscriptEvent],
    base: usize,
    tail: &str,
    opening: bool,
    side: transport::Side,
    lang: &str,
) -> String {
    let lines: Vec<String> = events[base..]
        .iter()
        .filter_map(|event| transport::lane_event_line(event, lang, side))
        .collect();
    if lines.is_empty() {
        return tail.to_owned();
    }
    let header = if opening {
        transport::history_header(lang)
    } else {
        ""
    };
    format!("{header}{}\n\n——\n{tail}", lines.join("\n\n"))
}
