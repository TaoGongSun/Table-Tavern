//! VN 從某一章重玩（plans/vn-mode-pkg5.md §2.9）：沿目前幕的劇情祖先鏈找章頭幕，截幕首分岔成新的一幕。
use std::path::Path;

use super::super::state::{read_state, WorldState};
use super::super::state_commit::with_commit;
use super::super::DataResult;
use super::lifecycle::{fork_scene_tx, scene_label};
use super::transcript::read_transcript;
use crate::ui_msg::UiMsg;

/// 劇情上的前一幕：分岔幕走被複製的來源幕（內容等於它），換幕來的幕走 `parent`。
fn story_previous(state: &WorldState, scene: u64) -> Option<u64> {
    let label = scene_label(state, scene);
    match (label.forked, label.source) {
        (true, Some(source)) => Some(source),
        _ => label.parent,
    }
}

/// 第 `chapter` 章的章頭幕：鏈上幕首快照章號等於它的最早一幕（同章多幕取最早）；第 1 章＝鏈上最早的幕。
fn chapter_head(root: &Path, world_id: &str, state: &WorldState, chapter: u32) -> DataResult<u64> {
    let mut chain = vec![state.current_scene];
    while let Some(previous) = chain.last().and_then(|&scene| story_previous(state, scene)) {
        chain.push(previous);
    }
    if chapter == 1 {
        return Ok(*chain.last().expect("鏈至少有目前幕"));
    }
    let mut head = None;
    for &scene in &chain {
        let first = read_transcript(root, world_id, scene)?.into_iter().next();
        let at = first
            .and_then(|event| event.state)
            .map(|state| super::super::vn::chapter_in(&state.tree));
        if at == Some(chapter) {
            head = Some(scene);
        }
    }
    head.ok_or_else(|| UiMsg::VnReplayNoChapter { chapter }.into_error())
}

/// 重玩第 `chapter` 章：章頭幕只留幕首那則（摘要或開場）分岔成新幕，回新幕號。原本的歷史不動。
pub fn replay_chapter(root: &Path, world_id: &str, chapter: u32) -> DataResult<u64> {
    let replayed = with_commit(root, world_id, |tx| {
        let state = read_state(root, world_id)?;
        let head = chapter_head(root, world_id, &state, chapter)?;
        fork_scene_tx(tx, head, Some(1))
    });
    crate::data::bump_capacity_epoch();
    replayed
}

#[cfg(test)]
#[path = "replay_tests.rs"]
mod tests;
