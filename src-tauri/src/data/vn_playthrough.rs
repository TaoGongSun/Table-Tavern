//! VN 新周目（plans/vn-mode-pkg5.md §2.10）：照白名單把原桌的設定與素材抄進一張新 VN 桌、狀態樹重新播種、
//! 第 0 幕只留開場事件。角色 id 一律沿用（立繪、聖經持有者、世界書可見性都以 id 為鍵）。
//! 拿到新桌獨占後一次建完，任何一步失敗就刪掉整桌，不留半桌；原桌只讀不寫。
use std::path::Path;

use super::message_vars;
use super::paths::world_dir;
use super::state_commit::with_commit;
use super::vn::{self, Playthrough, VnFile};
use super::{
    append_transcript, commit_world_write_atomic, create_world_exclusive, discard_new_world,
    invalid_data, list_characters, read_state, read_transcript, set_character_auto_hidden,
    write_state, DataResult, TableKind, TranscriptEvent, WorldState,
};

/// 從原桌抄的檔與目錄（相對桌目錄，缺就略過）：設定、素材、重構骨架與產物、跨聊天的卡片變數層。
/// 逐字稿、計時、續聊線、機制記帳、聊天層變數、匯入收據與原檔、網頁存檔旁檔都不帶；
/// 變數模式控制檔不抄、照第 0 幕重建（`build`）。
const CARRIED: [&str; 14] = [
    "world.md",
    "characters",
    "worldbook.json",
    "gm.png",
    "source-card.png",
    "source-card.import.json",
    "vn/sprites",
    "vn/backgrounds",
    "vn/cover.png",
    "interface-shell.html",
    "refactor-outcome.json",
    "gen-gallery",
    "card-vars/character",
    "card-vars/script",
];

/// 開新周目，回新桌 id。`name_template` 是前端在地化的桌名樣板（`{name}`＝第 1 周目桌名、`{n}`＝周目）。
pub fn new_playthrough(root: &Path, world_id: &str, name_template: &str) -> DataResult<String> {
    new_playthrough_with(root, world_id, name_template, &mut |_| Ok(()))
}

/// 測試掛點：每做完一步呼叫一次（步驟名），回錯＝模擬之後的步驟失敗。
pub(crate) fn new_playthrough_with(
    root: &Path,
    world_id: &str,
    name_template: &str,
    hook: &mut dyn FnMut(&str) -> DataResult<()>,
) -> DataResult<String> {
    let source = read_state(root, world_id)?;
    if source.table_kind != TableKind::Vn {
        return Err(invalid_data("new playthrough needs a VN table"));
    }
    let mut file = vn::read_vn(root, world_id)?;
    let playthrough = Playthrough {
        number: file.playthrough.as_ref().map_or(1, |done| done.number) + 1,
        base_name: file
            .playthrough
            .as_ref()
            .map_or_else(|| source.name.clone(), |done| done.base_name.clone()),
    };
    let name = name_template
        .replace("{name}", &playthrough.base_name)
        .replace("{n}", &playthrough.number.to_string());
    file.frozen = None;
    file.playthrough = Some(playthrough);
    let opening: Vec<TranscriptEvent> = read_transcript(root, world_id, 0)?
        .into_iter()
        .filter(|event| event.opening)
        .collect();
    let (new_id, held) =
        create_world_exclusive(root, &name, TableKind::Vn).map_err(|failed| failed.error)?;
    match build(root, world_id, &new_id, &source, file, opening, hook) {
        Ok(()) => Ok(new_id),
        Err(error) => {
            if let Err(cleanup) = discard_new_world(root, &new_id, &held) {
                log::warn!("新周目建桌失敗後收不掉新桌 {new_id}：{cleanup}");
            }
            Err(error)
        }
    }
}

fn build(
    root: &Path,
    from: &str,
    to: &str,
    source: &WorldState,
    file: VnFile,
    opening: Vec<TranscriptEvent>,
    hook: &mut dyn FnMut(&str) -> DataResult<()>,
) -> DataResult<()> {
    let (from_dir, to_dir) = (world_dir(root, from)?, world_dir(root, to)?);
    for entry in CARRIED {
        carry(&from_dir.join(entry), &to_dir.join(entry))?;
    }
    hook("files")?;
    // 遊玩中被自動隱藏的角色照第 0 幕快照還原（離開過第 0 幕才有快照；沒有＝現值就是第 0 幕的）
    if let Some(snapshot) = source.scene_card_visibility.get("0") {
        for meta in list_characters(root, to)? {
            match snapshot.get(&meta.id) {
                Some(&hidden) if !meta.archived && !meta.stage && hidden != meta.auto_hidden => {
                    set_character_auto_hidden(root, to, &meta.id, hidden)?
                }
                _ => {}
            }
        }
    }
    hook("characters")?;
    let seeds = file
        .bible
        .as_ref()
        .map_or_else(vn::engine_seeds, vn::Bible::seeds);
    // 變數模式：照原桌的模式與代換值重建控制檔，第 0 幕新 epoch、種子＝開場那則的表（沒有就第 0 幕種子）
    let control = message_vars::read_control(root, from)?;
    let seed = match control.active(0) {
        Some(scene) => Some(
            opening
                .iter()
                .find_map(|event| event.message_vars.as_ref())
                .unwrap_or(&scene.seed)
                .parse()?,
        ),
        None => None,
    };
    let epoch = with_commit(root, to, |tx| {
        vn::edit_vn(tx, |target| *target = file)?;
        seed.as_ref()
            .map(|seed| message_vars::publish_imported_seed(tx, 0, control.macros.clone(), seed))
            .transpose()
    })?;
    hook("vn")?;
    let mut state = read_state(root, to)?;
    state.model_bindings = source.model_bindings.clone();
    state.player_card_id = source.player_card_id.clone();
    state.mechanism = source.mechanism.clone();
    // 玩法標記照原桌（重構選的玩法、淘汰的狀態欄格式、regex 允許與否、面板指認的分支綁定）
    state.refactor_mode = source.refactor_mode.clone();
    state.statusbar_retired = source.statusbar_retired;
    state.regex_allowed = source.regex_allowed;
    state.branch_bindings = source.branch_bindings.clone();
    vn::fill_missing(&mut state.state.tree, &seeds);
    write_state(root, to, &state)?;
    hook("state")?;
    // 開場文字原樣，快照、id 重蓋（append 照新桌的狀態蓋快照）；變數模式第一則帶第 0 幕種子
    for (index, event) in opening.into_iter().enumerate() {
        let table = seed.as_ref().filter(|_| index == 0);
        let event = TranscriptEvent {
            id: None,
            message_vars: table.map(message_vars::VarsTable::from_json),
            vars_rev: None,
            vars_epoch: table.and(epoch.clone()),
            turn_key: None,
            action_id: None,
            state: None,
            ..event
        };
        append_transcript(root, to, 0, &event)?;
    }
    hook("opening")
}

/// 檔案逐一原子寫進新桌（目錄遞迴；新桌預建的空目錄照用）；來源不在就略過。
fn carry(path: &Path, target: &Path) -> DataResult<()> {
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let name = entry?.file_name();
            carry(&path.join(&name), &target.join(&name))?;
        }
    } else if path.is_file() {
        commit_world_write_atomic(target, &std::fs::read(path)?)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "vn_playthrough_tests.rs"]
mod tests;
