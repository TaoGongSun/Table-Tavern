//! 實送的落地呼叫點（方案三之 4）：掃描前結算、交給傳輸層那刻落地、確定失敗當場撤回。
//! 量測不走這裡（唯讀，見 `scene_budget::measure`）。

use crate::data::world_info_store::{self as store, Perspective, Report};
use crate::ui_msg::UiMsg;
use crate::world_info::timed::WiTimed;
use std::path::Path;

/// 實送掃描之前（持整桌寫入許可、回合交接 `settle_previous_turn` 之後，否則「GM 已提交、正文還沒落檔」的
/// 回合會被誤判成失敗）：結算這一幕的 pending，再讀這個視角的計時表。失敗回 `WorldInfoSettleFailed`。
/// 同桌還有別的對話輪在途就擋下（`WorldBusy`）：整桌寫入許可是共用讀鎖，GM 與角色回合在後端不互斥，
/// 並行時結算會把對方已送出、還沒落檔的落地誤撤。
pub fn before_scan(
    root: &Path,
    world_id: &str,
    scene: u64,
    turn_id: &str,
    perspective: &Perspective,
) -> Result<WiTimed, String> {
    if crate::inflight::other_turn_in_flight(world_id, turn_id) {
        return Err(UiMsg::WorldBusy.to_string());
    }
    store::settle_pending(root, world_id, scene).map_err(|error| error.to_string())?;
    store::read_timed(root, world_id, scene, perspective).map_err(|error| {
        UiMsg::WorldInfoSettleFailed {
            error: error.to_string(),
        }
        .to_string()
    })
}

/// 交給傳輸層那一刻：本輪掃完的計時表落地（寫入中）→ 已送出。中途失敗就撤回、不送出。
pub fn land(
    root: &Path,
    world_id: &str,
    scene: u64,
    turn_id: &str,
    perspective: &Perspective,
    timed: &WiTimed,
) -> Result<(), String> {
    let landed = store::begin_landing(root, world_id, scene, turn_id, perspective, timed)
        .and_then(|()| store::mark_sent(root, world_id, scene, turn_id));
    if let Err(error) = landed {
        fail(root, world_id, scene, turn_id);
        return Err(error.to_string());
    }
    Ok(())
}

/// 持許可當場判得出的確定失敗（角色呼叫回錯、角色中止沒有半截）：撤回這回合的落地。
/// 撤回本身失敗只記 log，留給下一次實送前的結算（那裡失敗會明確回報）。
pub fn fail(root: &Path, world_id: &str, scene: u64, turn_id: &str) {
    if let Err(error) = store::fail_turn(root, world_id, scene, turn_id, Report::Notices) {
        log::warn!("world-info: 撤回回合 {turn_id} 的落地失敗，留給下一次結算：{error}");
    }
}
