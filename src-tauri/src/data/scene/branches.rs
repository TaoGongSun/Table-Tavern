//! 分支（plans/vn-mode-pkg6.md §2.1–§2.4）：幕號水位、劇情祖先鏈、源頭對應、尾幕、選項邊界、切換分支。
use std::collections::HashMap;
use std::path::Path;

use super::super::message_vars;
use super::super::state::{read_state, write_state, WorldState};
use super::super::state_commit::{with_commit, CommitTx};
use super::super::DataResult;
use super::lifecycle::{fork_scene_tx, scene_label};
use super::transcript::{read_transcript, TranscriptEvent};
use super::visibility::{record_leaving_scene, restore_scene_visibility};
use crate::ui_msg::UiMsg;

/// 全桌已發布的最大幕號：新幕號一律取它＋1。切換分支後目前幕不一定最大，不能再用 `current + 1`。
pub fn scene_high(state: &WorldState) -> u64 {
    state
        .scene_labels
        .keys()
        .filter_map(|key| key.parse::<u64>().ok())
        .chain(std::iter::once(state.current_scene))
        .max()
        .unwrap_or_default()
}

/// 劇情上的前一幕：分岔幕走被複製的來源幕（內容等於它），換幕來的幕走 `parent`。
pub fn story_previous(state: &WorldState, scene: u64) -> Option<u64> {
    let label = scene_label(state, scene);
    match (label.forked, label.source) {
        (true, Some(source)) => Some(source),
        _ => label.parent,
    }
}

/// 換幕來的子幕（每幕最多一個：換幕只在目前幕做、切換只准切到尾幕）。只認有標籤的幕。
fn next_child(state: &WorldState, scene: u64) -> Option<u64> {
    state
        .scene_labels
        .iter()
        .find(|(_, label)| !label.forked && label.parent == Some(scene))
        .and_then(|(key, _)| key.parse().ok())
}

/// 尾幕＝沒有換幕來的子幕；只有尾幕能接著玩（往下寫不會和子幕的摘要分岔）。
fn is_tip(state: &WorldState, scene: u64) -> bool {
    next_child(state, scene).is_none()
}

/// 順著換幕子幕走到底。
pub fn tip_of(state: &WorldState, scene: u64) -> u64 {
    let mut at = scene;
    for _ in 0..=scene_high(state) {
        match next_child(state, at) {
            Some(child) => at = child,
            None => break,
        }
    }
    at
}

/// 逐字稿讀一次就留著（查分支會反覆讀同幾幕）。
#[derive(Default)]
pub struct Transcripts(HashMap<u64, Vec<TranscriptEvent>>);

impl Transcripts {
    pub fn get(
        &mut self,
        root: &Path,
        world_id: &str,
        scene: u64,
    ) -> DataResult<&[TranscriptEvent]> {
        if !self.0.contains_key(&scene) {
            let events = read_transcript(root, world_id, scene)?;
            self.0.insert(scene, events);
        }
        Ok(&self.0[&scene])
    }
}

/// 源頭對應：`scene` 的第 `k` 則（逐字稿索引）若是分岔時複製來、且來源那段沒被改寫（索引 `keep − 1`
/// 仍是 `cut`），換成來源幕的同一位置，一路往上；回最早寫下它的幕。
pub fn origin(
    root: &Path,
    world_id: &str,
    state: &WorldState,
    book: &mut Transcripts,
    scene: u64,
    k: usize,
) -> DataResult<u64> {
    let mut at = scene;
    for _ in 0..=scene_high(state) {
        let label = scene_label(state, at);
        let (true, Some(source), Some(keep), Some(cut)) =
            (label.forked, label.source, label.keep, label.cut.as_deref())
        else {
            break;
        };
        let keep = keep as usize;
        if k >= keep || source == at {
            break;
        }
        let events = book.get(root, world_id, source)?;
        if events.get(keep - 1).and_then(|event| event.id.as_deref()) != Some(cut) {
            break;
        }
        at = source;
    }
    Ok(at)
}

/// 選項邊界的輸入：逐字稿每一則是選項事件、玩家句或其他（角色線、系統、沒有選項的旁白）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Beat {
    Choice,
    Player,
    Other,
}

/// 選項點的狀態：其後第一則玩家句之前又出了一組選項＝被後來的取代（沒人選它），不算選項點。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChoiceState {
    Live,
    /// 玩家選它的那則（逐字稿索引）；中間的角色線屬於選項點這一側
    Answered(usize),
    Superseded,
}

/// 選項邊界（§2.2）：已選／未選、分岔截點、分支查詢、面板全部用這一支。`k` 必須是 `Beat::Choice`。
pub fn choice_state(beats: &[Beat], k: usize) -> ChoiceState {
    for (index, beat) in beats.iter().enumerate().skip(k + 1) {
        match beat {
            Beat::Player => return ChoiceState::Answered(index),
            Beat::Choice => return ChoiceState::Superseded,
            Beat::Other => {}
        }
    }
    ChoiceState::Live
}

/// 從目前幕的選項點分岔：只留玩家選它之前的事件（`keep`＝玩家句的索引）。提交鎖內再對一次
/// 那則玩家句的 id，判斷到分岔之間逐字稿被改過就拒（`SceneStale`）。
pub fn fork_before(root: &Path, world_id: &str, keep: usize, answer_id: &str) -> DataResult<u64> {
    let forked = with_commit(root, world_id, |tx| {
        let scene = read_state(root, world_id)?.current_scene;
        let events = read_transcript(root, world_id, scene)?;
        if events.get(keep).and_then(|event| event.id.as_deref()) != Some(answer_id) {
            return Err(UiMsg::SceneStale.into_error());
        }
        fork_scene_tx(tx, scene, Some(keep))
    });
    crate::data::bump_capacity_epoch();
    forked
}

/// 切到另一條分支的尾幕（§2.4）：不碰任何逐字稿，只換目前幕與跟著幕走的狀態。
pub fn switch_scene(root: &Path, world_id: &str, target: u64) -> DataResult<u64> {
    let switched = with_commit(root, world_id, |tx| switch_scene_tx(tx, target, true));
    crate::data::bump_capacity_epoch();
    switched
}

/// 重播桌（VN 作品包）換到任一已發布幕（plans/vn-mode-pkg8.md §2.6）：不會往下寫，所以不要求尾幕；
/// 已在那幕就不動。一般桌拒。
pub fn replay_goto(root: &Path, world_id: &str, target: u64) -> DataResult<u64> {
    let moved = with_commit(root, world_id, |tx| replay_goto_tx(tx, target));
    crate::data::bump_capacity_epoch();
    moved
}

pub(super) fn replay_goto_tx(tx: &CommitTx<'_>, target: u64) -> DataResult<u64> {
    let state = read_state(tx.root, tx.world_id)?;
    if !state.replay {
        return Err(crate::data::invalid_data(
            "replay goto needs a replay table",
        ));
    }
    match target == state.current_scene {
        true => Ok(target),
        false => switch_scene_tx(tx, target, false),
    }
}

/// 兩段提交照分岔：先落地被離開那幕的快照與 `pending_leave`，再照目標幕快照改卡；任一步失敗
/// `current_scene` 不動，重試沿用那份快照。
fn switch_scene_tx(tx: &CommitTx<'_>, target: u64, require_tip: bool) -> DataResult<u64> {
    let (root, world_id) = (tx.root, tx.world_id);
    message_vars::refuse_during_turn(tx)?;
    let mut state = read_state(root, world_id)?;
    let current = state.current_scene;
    if target == current || target > scene_high(&state) || (require_tip && !is_tip(&state, target))
    {
        return Err(UiMsg::VnBranchNotTip.into_error());
    }
    let events = read_transcript(root, world_id, target)?;
    if events.is_empty() {
        return Err(UiMsg::VnBranchNotTip.into_error());
    }
    // 世界書計時：兩幕的 pending 先結算，目標幕自己的計時檔原本就在
    super::super::world_info_store::settle_pending(root, world_id, current)?;
    super::super::world_info_store::settle_pending(root, world_id, target)?;
    record_leaving_scene(root, world_id, &mut state)?;
    state.pending_leave = Some(current);
    write_state(root, world_id, &state)?;
    restore_scene_visibility(root, world_id, &state, target)?;
    state.current_scene = target;
    state.pending_leave = None;
    state.state = events
        .iter()
        .rev()
        .find_map(|event| event.state.clone())
        .unwrap_or_default();
    #[cfg(test)]
    if super::lifecycle::fail_publish::take() {
        return Err(crate::data::invalid_data("故障注入：切換發布失敗"));
    }
    write_state(root, world_id, &state)?;
    message_vars::refresh_cache(tx);
    Ok(target)
}

#[cfg(test)]
#[path = "branches_tests.rs"]
mod tests;
