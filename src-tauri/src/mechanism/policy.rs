//! 數字欄更新策略（mvu-replace-numeric）：沒重構的 MVU 卡照上游 MVU 收 replace，其餘維持本 app 的
//! 強制 delta。只在載入層算一次，套用（`apply_block`）與提示詞（`gm_system_prompt`）拿同一份。
use crate::data::{self, NumericUpdate};
use std::path::Path;

/// 沒重構（比照重新重構的判定）而且有 MVU 來源依據才走上游；讀不到就當重構過，維持原規則。
/// MVU 來源依據：世界書匯入過 MVU 鷹架（未重構桌的 `incremental` 只有 `[initvar]`／`[mvu_update]`
/// 會開），或有角色卡載入 MVU 腳本。
pub fn resolve_numeric_update(
    root: &Path,
    world_id: &str,
    state: &data::WorldState,
) -> NumericUpdate {
    if crate::refactor::refactored(root, world_id, state).unwrap_or(true) {
        return NumericUpdate::DeltaOnly;
    }
    let mvu_source = state.mechanism.incremental
        || crate::import::read_card_interfaces(root, world_id)
            .is_ok_and(|cards| cards.iter().any(|card| card.mvu));
    match mvu_source {
        true => NumericUpdate::Upstream,
        false => NumericUpdate::DeltaOnly,
    }
}
