mod context;
mod expand;
mod frame;
mod parse_common;
mod prompt_common;
mod result_parse;
mod rewrite;
pub(crate) mod session;
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
pub fn table_tree_section(
    root: &std::path::Path,
    world_id: &str,
) -> crate::data::DataResult<String> {
    Ok(existing_tree_section(
        &crate::data::read_state(root, world_id)?.state.tree,
    ))
}
