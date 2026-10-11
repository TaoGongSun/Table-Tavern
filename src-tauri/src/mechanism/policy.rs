//! 數字欄更新策略（mvu-replace-numeric）：沒重構的 MVU 卡照上游 MVU 收 replace，其餘維持本 app 的
//! 強制 delta。只在載入層算一次，套用（`apply_block`）與提示詞（`gm_system_prompt`）拿同一份。
//! 重構另有 `mvu_source`（非 MVU 桌的介面重構不把既有樹當權威），判定比這裡多排除不支援的卡。
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

/// 重構用：這桌（未重構時）的狀態樹是否來自 MVU 來源——世界書匯入過 MVU 鷹架（未重構桌的 `incremental`
/// 只有 `[initvar]`／`[mvu_update]` 會開），或有角色卡載入 MVU 腳本且沒被判不支援（與卡片變數模式的啟用條件
/// 一致，樹才真的是 MVU 變數）。不是 MVU 來源的桌，樹只是從原卡渲染出來的狀態區塊解析來的。讀卡失敗回錯。
/// 數字欄更新策略（`resolve_numeric_update`）另有自己的判定，不共用這個。
pub fn mvu_source(root: &Path, world_id: &str, state: &data::WorldState) -> data::DataResult<bool> {
    if state.mechanism.incremental {
        return Ok(true);
    }
    Ok(crate::import::read_card_interfaces(root, world_id)?
        .iter()
        .any(|card| card.mvu && card.unsupported.is_none()))
}
