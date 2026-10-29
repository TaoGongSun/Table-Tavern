//! 套用＋收據的整段：成功照記收據；中途失敗時（某位角色寫卡失敗之類）把已落地的部分記成收據
//! 立刻撤銷，回到零寫入再回原錯——不留下沒有收據的半套用，重試也不會再建一份。
//! 自動回滾用嚴格撤銷（任一域失敗就停、收據不彈出）：沒完整退回就留著這次的收據，
//! 回 RefactorApplyPartial（提示可手動撤銷）；這次的收據根本沒寫成時回
//! RefactorApplyPartialNoReceipt，不提示撤銷——那時「上一筆」是別次匯入，撤了會傷到它。
//!
//! AI 結果卡的套用無還原〔作者裁決 2026-08-11〕：套用前先在收據清單放屏障，成功後不記完整收據，
//! 復原撤到屏障就停〔作者裁決 2026-10-29〕。屏障寫不進去就不套用；套用沒留下變動（寫入前被拒、
//! 中途失敗已回滾成功）時拿掉這次的屏障，不無故擋住更早的匯入。

use super::apply::apply_with_assets;
use super::card_png::CardAsset;
use super::types::{RefactorApplyOrigin, RefactorApplySummary, RefactorOutcome, RefactorSelection};
use crate::data::{self, DataResult};
use crate::receipts::{self, RefactorRecord};
use crate::ui_msg::UiMsg;
use std::path::Path;

pub fn apply_and_record(
    root: &Path,
    world_id: &str,
    lang: &str,
    outcome: &RefactorOutcome,
    selection: &RefactorSelection,
    assets: &[CardAsset],
    origin: RefactorApplyOrigin,
    held: &data::WorldExclusive,
) -> DataResult<RefactorApplySummary> {
    let before = receipts::snapshot_refactor(root, world_id)?;
    let label = UiMsg::ReceiptRefactorApply.to_string();
    let barrier = origin == RefactorApplyOrigin::Ai;
    if barrier {
        receipts::push_refactor_barrier(root, world_id, &label, held)?;
    }
    // 這次的套用沒留下任何變動：拿掉剛放的屏障（彈不掉只記 log——多一道屏障只擋住更早的復原，不壞桌）
    let drop_barrier = || {
        if barrier {
            if let Err(error) = receipts::pop_refactor_barrier(root, world_id, held) {
                log::warn!("refactor barrier pop failed: {error}");
            }
        }
    };
    match apply_with_assets(root, world_id, lang, outcome, selection, assets) {
        Ok(result) => {
            if !barrier {
                receipts::record_refactor_apply(
                    root,
                    world_id,
                    &label,
                    result.character_ids,
                    result.rewritten_entries,
                    result.deleted_entries,
                    result.deleted_entries_raw,
                    result.stage_changes,
                    before,
                    held,
                );
            }
            Ok(result.summary)
        }
        Err(failure) => {
            let progress = failure.progress;
            let recorded = receipts::record_refactor_apply(
                root,
                world_id,
                &label,
                progress.character_ids,
                progress.rewritten_entries,
                progress.deleted_entries,
                progress.deleted_entries_raw,
                progress.stage_changes,
                before,
                held,
            );
            match recorded {
                RefactorRecord::Nothing => {
                    drop_barrier();
                    Err(failure.error)
                }
                RefactorRecord::Recorded => {
                    match receipts::rollback_last_import(root, world_id, held) {
                        Ok(_) => {
                            drop_barrier();
                            Err(failure.error)
                        }
                        Err(undo_error) => {
                            log::warn!("refactor apply rollback failed: {undo_error}");
                            Err(UiMsg::RefactorApplyPartial {
                                error: failure.error.to_string(),
                            }
                            .into_error())
                        }
                    }
                }
                RefactorRecord::Failed => Err(UiMsg::RefactorApplyPartialNoReceipt {
                    error: failure.error.to_string(),
                }
                .into_error()),
            }
        }
    }
}

#[cfg(test)]
mod barrier_tests;
#[cfg(test)]
mod tests;
