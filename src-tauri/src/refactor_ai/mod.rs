mod context;
mod expand;
mod parse_common;
mod prompt_common;
mod result_parse;
mod rewrite;
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
mod types_tests;

pub use context::{assemble_card_context, entry_full_text, prescan_worldbook, segment_spans};
pub use expand::{expand_messages, person_expand_messages};
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
