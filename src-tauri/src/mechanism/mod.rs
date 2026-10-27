//! MVU 機制格式：解析模型吐出的 `<UpdateVariable>` JSON Patch，依欄位規則本地決定收不收。
//! 模型只說「這一幕變動多少」，加減／夾邊界／擲骰全在這裡做——模型算數字會幻覺，
//! 本地帳才是真相。容錯是紅線：格式壞的那一筆丟掉沿用舊值，絕不 panic、絕不中斷整批更新。

mod apply;
mod collection;
mod derive;
pub mod engine_records;
mod ledger;
mod mirror;
mod note;
mod parse;
mod policy;
mod rules;
mod tree;
mod triggers;
mod types;
pub(crate) mod upstream_set;
mod value_keys;

#[cfg(test)]
mod block_scope_tests;
#[cfg(test)]
mod collection_tests;
#[cfg(test)]
mod feedback_e2e_tests;
#[cfg(test)]
mod glyph_path_tests;
#[cfg(test)]
mod list_block_tests;
#[cfg(test)]
mod object_key_glyph_tests;
#[cfg(test)]
mod once_flag_tests;
#[cfg(test)]
mod parse_feedback_tests;
#[cfg(test)]
mod slash_key_tests;
#[cfg(test)]
mod test_support;

#[cfg(test)]
pub use ledger::apply_block;
pub use ledger::{append_log, apply_block_typed, read_ledger, BlockScope, Ledger};
pub use mirror::{adopt_mirror_marks, mark_survey_mirrors, sync_mirrors_from_edit};
pub use policy::{mvu_source, resolve_numeric_update};
pub use rules::{has_rule, leaf_shown, rule_for_path};
pub(crate) use tree::canonical_path;
#[cfg(test)]
pub(crate) use triggers::evaluate_triggers;
pub(crate) use triggers::map_state_placeholders;
pub use types::{Outcome, Record, RecordKind};
