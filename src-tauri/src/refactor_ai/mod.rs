mod context;
mod expand;
mod frame;
mod parse_common;
mod prompt_common;
mod result_parse;
mod rewrite;
pub(crate) mod session;
pub(crate) mod shell_sections;
mod survey;
mod survey_parse;
pub(crate) mod types;

#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod result_parse_tests;
#[cfg(test)]
mod survey_parse_tests;
#[cfg(test)]
mod survey_tests;
#[cfg(test)]
mod tree_prompt_tests;
#[cfg(test)]
mod types_tests;

pub use context::{assemble_card_context, entry_full_text, prescan_worldbook, segment_spans};
pub use expand::{expand_messages, person_expand_messages, SPANS_EXPAND_KIND};
pub use frame::{frame_candidate_tags, RefactorFrameCandidate};
pub(crate) use prompt_common::existing_tree_section;
pub use result_parse::{
    expand_span_placeholders, parse_absorb, parse_expand, parse_group, parse_person_expand,
};
pub use rewrite::{absorb_messages, group_messages};
pub use survey::{recommend_messages, survey_messages};
pub use survey_parse::{normalize_survey_for_mode, parse_recommend, parse_survey};
pub use types::{
    EntryKind, EntrySpan, GroupKind, RefactorEntryMeta, RefactorEntryVerdict,
    RefactorExpandOutcome, RefactorNewEntry, RefactorPersonExpandOutcome, RefactorRecommendOutcome,
    RefactorRewriteOutcome, RefactorSpanRoute, RefactorSurveyOutcome,
};

/// 這桌既有狀態樹段（經卡片變數投影讀樹）：盤點、接管、合組、介面展開的 user 訊息共用。
/// 介面模式（`mode` 是 interface）而且不是 MVU 來源的桌回空段：那棵樹只是開場白狀態區塊解析出來的原卡
/// 渲染輸出（有損），照它命名會讓骨架沿用帶裝飾的路徑；套用時也以產物為準（見 refactor/apply.rs）。
/// characters 模式與 MVU 桌照附。讀卡失敗回錯，不預設成非 MVU。
pub fn table_tree_section(
    root: &std::path::Path,
    world_id: &str,
    mode: Option<&str>,
) -> crate::data::DataResult<String> {
    let state = crate::data::read_state(root, world_id)?;
    if mode == Some("interface") && !crate::mechanism::mvu_source(root, world_id, &state)? {
        return Ok(String::new());
    }
    // 開場狀態只落在平欄的卡（狀態區塊不是 YAML）：time／place／present 以外的鍵也當頂層欄位列出，
    // AI 才會照原字命名規則路徑，不會自取另一種字形（驻守天数 vs 駐守天數）
    let mut tree = state.state.tree.clone();
    for (key, value) in &state.state.table {
        if !matches!(key.as_str(), "time" | "place" | "present") {
            tree.entry(key.clone())
                .or_insert_with(|| crate::data::StateNode::Leaf(value.clone()));
        }
    }
    Ok(existing_tree_section(&tree))
}
