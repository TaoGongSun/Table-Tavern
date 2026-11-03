//! VN 選項點的分岔與分支查詢、存檔書籤（plans/vn-mode-pkg6.md §2.3、§2.5、§2.7）：解析事件找選項點與玩家選的那句，
//! 截點與源頭對應交給 `data::scene` 的共用規則（`choice_state`、`origin`），包 8 打包分岔樹用同一套。
use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::chat_turn::{beats, read_scene, VnEventCells};
use crate::data::{self, ChoiceState, Transcripts};
use crate::ui_msg::UiMsg;

fn text<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

/// 一幕的格＋逐字稿長度＋選項邊界輸入。
struct Scene {
    cells: Vec<VnEventCells>,
    beats: Vec<data::Beat>,
}

fn load(root: &Path, world_id: &str, book: &mut Transcripts, scene: u64) -> Result<Scene, String> {
    let total = book.get(root, world_id, scene).map_err(text)?.len();
    let (cells, _) = read_scene(root, world_id, scene)?;
    let beats = beats(&cells, total);
    Ok(Scene { cells, beats })
}

impl Scene {
    /// 在 `k` 的選項點，玩家選它的那則：（索引, 文字, 事件 id）。
    fn answer(&self, k: usize) -> Option<(usize, String, Option<String>)> {
        if self.beats.get(k) != Some(&data::Beat::Choice) {
            return None;
        }
        let ChoiceState::Answered(p) = data::choice_state(&self.beats, k) else {
            return None;
        };
        let event = self.cells.iter().find(|event| event.index == p)?;
        let text = match event.cells.first() {
            Some(crate::chat_turn::VnCell::Narration { text }) => text.clone(),
            _ => String::new(),
        };
        Some((p, text, event.event_id.clone()))
    }

    fn index_of(&self, event_id: &str) -> Option<usize> {
        self.cells
            .iter()
            .find(|event| event.event_id.as_deref() == Some(event_id))
            .map(|event| event.index)
    }
}

/// 從目前幕已選過的選項點分岔（§2.3）：只留玩家選它之前的事件（含其間的角色線），回新幕號。
pub(crate) fn fork_choice(root: &Path, world_id: &str, event_id: &str) -> Result<u64, String> {
    let state = data::read_state(root, world_id).map_err(text)?;
    let mut book = Transcripts::default();
    let scene = load(root, world_id, &mut book, state.current_scene)?;
    let (keep, _, answer_id) = scene
        .index_of(event_id)
        .and_then(|k| scene.answer(k))
        .ok_or_else(|| UiMsg::VnChoiceNotPast.to_string())?;
    let answer_id = answer_id.ok_or_else(|| UiMsg::VnChoiceNotPast.to_string())?;
    data::fork_before(root, world_id, keep, &answer_id).map_err(text)
}

/// 選項點上玩過的一條分支：玩家選的那句、可切過去的尾幕、尾幕是否已完結。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ChoiceBranch {
    pub option: String,
    pub tip: u64,
    pub ended: bool,
}

/// 目前幕某個選項點上玩過的各條分支（§2.5）：源頭幕自己選的那條，加上在同一點截分岔、自己選了的分岔幕；
/// 截得更後面的分岔只是複製了別條的選擇，不列。同一句留尾幕號大的。
pub(crate) fn choice_branches(
    root: &Path,
    world_id: &str,
    event_id: &str,
) -> Result<Vec<ChoiceBranch>, String> {
    let state = data::read_state(root, world_id).map_err(text)?;
    let mut book = Transcripts::default();
    let here = load(root, world_id, &mut book, state.current_scene)?;
    let k = here
        .index_of(event_id)
        .filter(|&k| here.beats.get(k) == Some(&data::Beat::Choice))
        .ok_or_else(|| UiMsg::VnChoiceNotPast.to_string())?;
    let source =
        data::origin(root, world_id, &state, &mut book, state.current_scene, k).map_err(text)?;
    let mut found: BTreeMap<String, u64> = BTreeMap::new();
    let mut take = |answer: Option<(usize, String, Option<String>)>, scene: u64| {
        if let Some((_, option, _)) = answer {
            let tip = data::tip_of(&state, scene);
            let slot = found.entry(option).or_insert(tip);
            *slot = (*slot).max(tip);
        }
    };
    let origin_scene = load(root, world_id, &mut book, source)?;
    let boundary = origin_scene.answer(k).map(|(p, _, _)| p);
    take(origin_scene.answer(k), source);
    if let Some(boundary) = boundary {
        for (key, label) in &state.scene_labels {
            let (Ok(fork), true, Some(from), Some(keep)) =
                (key.parse::<u64>(), label.forked, label.source, label.keep)
            else {
                continue;
            };
            if keep as usize != boundary
                || data::origin(root, world_id, &state, &mut book, from, k).map_err(text)? != source
            {
                continue;
            }
            let scene = load(root, world_id, &mut book, fork)?;
            take(scene.answer(k), fork);
        }
    }
    let mut branches = Vec::new();
    for (option, tip) in found {
        let ended = book
            .get(root, world_id, tip)
            .map_err(text)?
            .iter()
            .rev()
            .find_map(|event| event.state.as_ref())
            .and_then(|snapshot| data::vn::ended_in(&snapshot.tree))
            .is_some();
        branches.push(ChoiceBranch { option, tip, ended });
    }
    Ok(branches)
}

/// 存檔：目前幕某則事件的某一格（`cell` 要在那則的格數內），章號與預覽取自那則。
pub(crate) fn create_save(
    root: &Path,
    world_id: &str,
    name: &str,
    event_id: &str,
    cell: u32,
) -> Result<data::vn_progress::Save, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > data::vn_progress::NAME_MAX {
        let max = data::vn_progress::NAME_MAX as u32;
        return Err(UiMsg::VnSaveNameInvalid { max }.to_string());
    }
    let scene = data::read_state(root, world_id)
        .map_err(text)?
        .current_scene;
    let (cells, _) = read_scene(root, world_id, scene)?;
    let event = cells
        .iter()
        .find(|event| event.event_id.as_deref() == Some(event_id))
        .filter(|event| (cell as usize) < event.cells.len())
        .ok_or_else(|| UiMsg::VnSaveStale.to_string())?;
    let preview = match &event.cells[cell as usize] {
        crate::chat_turn::VnCell::Narration { text }
        | crate::chat_turn::VnCell::Dialogue { text, .. } => text.chars().take(40).collect(),
        _ => String::new(),
    };
    let save = data::vn_progress::Save {
        id: data::new_id(),
        name: name.to_owned(),
        scene,
        event_id: event_id.to_owned(),
        cell,
        chapter: event.chapter.unwrap_or(1),
        ts: data::local_timestamp().map_err(text)?,
        preview,
    };
    data::vn_progress::add_save(root, world_id, save.clone()).map_err(text)?;
    Ok(save)
}

/// 讀檔落點：之後播放器停在這一格。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Loaded {
    pub scene: u64,
    pub event_id: String,
    pub cell: u32,
}

/// 讀檔（§2.7）：存檔的幕是目前幕＝只回位置；是別條分支的尾幕＝切過去；那幕之後已換章（不是尾幕）＝整幕
/// 分岔一份（讀檔後往前讀得到原本寫好的內容），回新幕裡同位置的新事件 id。幕、事件或格不在＝失效。
pub(crate) fn load_save(root: &Path, world_id: &str, id: &str) -> Result<Loaded, String> {
    let stale = || UiMsg::VnSaveStale.to_string();
    let save = data::vn_progress::read_progress(root, world_id)
        .map_err(text)?
        .saves
        .into_iter()
        .find(|save| save.id == id)
        .ok_or_else(stale)?;
    let state = data::read_state(root, world_id).map_err(text)?;
    if save.scene > data::scene_high(&state) {
        return Err(stale());
    }
    let (cells, _) = read_scene(root, world_id, save.scene)?;
    let index = cells
        .iter()
        .find(|event| event.event_id.as_deref() == Some(save.event_id.as_str()))
        .filter(|event| (save.cell as usize) < event.cells.len())
        .map(|event| event.index)
        .ok_or_else(stale)?;
    let at = |scene: u64, event_id: String| Loaded {
        scene,
        event_id,
        cell: save.cell,
    };
    if save.scene == state.current_scene {
        return Ok(at(save.scene, save.event_id));
    }
    if data::tip_of(&state, save.scene) == save.scene {
        let scene = data::switch_scene(root, world_id, save.scene).map_err(text)?;
        return Ok(at(scene, save.event_id));
    }
    let scene = data::fork_scene(root, world_id, save.scene).map_err(text)?;
    let event_id = data::read_transcript(root, world_id, scene)
        .map_err(text)?
        .get(index)
        .and_then(|event| event.id.clone())
        .ok_or_else(stale)?;
    Ok(at(scene, event_id))
}

#[cfg(test)]
#[path = "vn_branches_tests.rs"]
mod tests;
