//! AI 卡重構套用：AI 讀整張匯入卡，把內容拆成角色／介面／機制三類產物（RefactorOutcome），
//! 玩家人審勾選（RefactorSelection）後套用落檔，可一鍵倒退。AI 呼叫是下一包的事，這裡只管
//! 「已經有一份 RefactorOutcome，怎麼套用、怎麼復原」——手寫 JSON 餵進 apply() 就能驗證整條路。

mod apply;
mod apply_mechanism;
mod apply_record;
mod card_export;
mod card_file;
mod card_import;
mod card_png;
mod collections;
mod flat_migration;
mod interface;
mod opening_fields;
mod opening_notes;
mod reset;
mod rule_merge;
mod sources;
mod stage;
mod survey_fields;
mod types;

#[cfg(test)]
pub use apply::apply;
pub use apply_record::apply_and_record;
pub use card_export::{export_outcome, export_saved};
pub use card_import::{
    claim_assets, open_and_stage, release_assets, return_assets, OpenCardResult,
};
pub(crate) use reset::refactored;
pub use reset::{apply_to_table, rerun_status, reset_to_import_source, RerunStatus, ResetOutcome};
pub(crate) use stage::{keep_resolvable as keep_stage_cards, player_card_name};
pub use types::{
    normalize_stored_mode, RefactorApplyOrigin, RefactorApplySummary, RefactorCharacter,
    RefactorInterface, RefactorOutcome, RefactorSelection,
};

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
