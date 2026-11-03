use std::path::Path;

use super::super::message_vars;
use super::super::state::{read_state, write_state, SceneLabel, WorldState};
use super::super::state_commit::{with_commit, CommitTx};
use super::super::{local_timestamp, DataResult};
use super::branches::{scene_high, story_previous};
use super::marker::EventMarker;
use super::presence::settle_card_visibility;
use super::transcript::{
    append_unpublished_tx, parse_transcript, read_transcript, transcript_path, TranscriptEvent,
    TranscriptKind,
};
use super::visibility::{record_leaving_scene, restore_scene_visibility};
use crate::ui_msg::UiMsg;

/// 沒進 scene_labels 的幕＝原線（舊存檔也走這條）：顯示編號就是內部幕號，第 1 版，上一幕是前一號。
pub fn scene_label(state: &WorldState, scene: u64) -> SceneLabel {
    state
        .scene_labels
        .get(&scene.to_string())
        .cloned()
        .unwrap_or(SceneLabel {
            base: scene,
            version: 1,
            parent: scene.checked_sub(1),
            forked: false,
            source: None,
            keep: None,
            cut: None,
        })
}

/// 算「某個 base 目前該排第幾個版本」：掃 0..=upto（幕號水位）每一幕的顯示 base，數出撞號的幕數再 +1。
/// begin_next_scene 與 fork_scene 都靠它算新標籤，掃描範圍在插入新標籤之前的呼叫端已經固定。
fn next_scene_version(state: &WorldState, upto: u64, base: u64) -> u32 {
    (0..=upto)
        .filter(|&scene| scene_label(state, scene).base == base)
        .count() as u32
        + 1
}

/// 分岔：把某一幕的紀錄原樣複製成新的一幕接著玩，原本歷史一個字都不動。
/// 顯示編號跟隨來源幕（從分岔幕再分岔＝跟著源頭走，不是跟著內部號走），
/// parent 記分岔當下所在的幕，退回時回到這裡而不是來源幕。
pub fn fork_scene(root: &Path, world_id: &str, from_scene: u64) -> DataResult<u64> {
    let forked = with_commit(root, world_id, |tx| fork_scene_tx(tx, from_scene, None));
    crate::data::bump_capacity_epoch();
    forked
}

/// 發布順序（計畫 8.4）：①新幕逐字稿（事件 id 與表版本重新配發、來源幕有效 epoch 的表改成新幕 epoch）
/// ②控制檔寫新幕種子 ③寫 current_scene。任一步失敗就停；新幕號仍是「目前幕＋1」，重試覆寫上次的殘留。
/// `keep`＝只留來源幕前幾則（VN 從某一章重玩截幕首、選項點分岔截到玩家句前，包 5 §2.9、包 6 §2.3）；
/// 截幕首可以從目前這幕分岔。來源只要已發布（不超過幕號水位、逐字稿非空）就收，不綁目前幕號（包 6 §2.1）。
pub(super) fn fork_scene_tx(
    tx: &CommitTx<'_>,
    from_scene: u64,
    keep: Option<usize>,
) -> DataResult<u64> {
    let (root, world_id) = (tx.root, tx.world_id);
    message_vars::refuse_during_turn(tx)?;
    let mut state = read_state(root, world_id)?;
    let high = scene_high(&state);
    if from_scene > high || (keep.is_none() && from_scene == state.current_scene) {
        return Err(UiMsg::SceneForkNotEarlier.into_error());
    }
    let mut events = read_transcript(root, world_id, from_scene)?;
    if let Some(keep) = keep {
        events.truncate(keep);
    }
    if events.is_empty() {
        return Err(UiMsg::SceneNothingToContinue.into_error());
    }

    // 截點：來源幕最後一則被複製的事件在分岔當下的 id（事件 id 下面會重發）
    let copied = events.len();
    let cut = events.last().and_then(|event| event.id.clone());
    let current_scene = state.current_scene;
    let new_scene = high + 1;
    let plan = message_vars::scene_seed_for_fork(tx, from_scene)?;
    // 角色卡隱藏（兩段提交）：先落地被離開那幕的快照與 pending_leave，再照來源幕快照改卡；
    // 改卡或後面任一步失敗，重試會沿用這份快照，不把改了一半的卡當成原值
    record_leaving_scene(root, world_id, &mut state)?;
    state.pending_leave = Some(current_scene);
    write_state(root, world_id, &state)?;
    restore_scene_visibility(root, world_id, &state, from_scene)?;
    // 世界書計時：目前幕（被離開的）與來源幕的 pending 先結算，來源幕的檔複製成新幕的（不帶 pending）
    super::super::world_info_store::copy_for_fork(
        root,
        world_id,
        current_scene,
        from_scene,
        new_scene,
    )?;
    for event in events.iter_mut() {
        event.id = Some(message_vars::new_token());
        event.turn_key = None;
        if event.message_vars.is_some() {
            event.vars_rev = Some(message_vars::new_token());
            if let Some(plan) = &plan {
                if event.vars_epoch.as_deref() == Some(plan.source_epoch.as_str()) {
                    event.vars_epoch = Some(plan.vars.epoch.clone());
                }
            }
        }
    }
    let mut buffer = String::new();
    for event in &events {
        buffer.push_str(&serde_json::to_string(event)?);
        buffer.push('\n');
    }
    super::super::world_file::commit_world_write(
        &transcript_path(root, world_id, new_scene)?,
        buffer.as_bytes(),
    )?;
    if let Some(plan) = plan {
        message_vars::publish_scene_seed(tx, new_scene, plan.vars)?;
    }

    let base = scene_label(&state, from_scene).base;
    let version = next_scene_version(&state, high, base);
    state.scene_labels.insert(
        new_scene.to_string(),
        SceneLabel {
            base,
            version,
            parent: Some(current_scene),
            forked: true,
            source: Some(from_scene),
            keep: Some(copied as u32),
            cut,
        },
    );
    state.current_scene = new_scene;
    state.pending_leave = None;
    state.state = events
        .iter()
        .rev()
        .find_map(|event| event.state.clone())
        .unwrap_or_default();
    #[cfg(test)]
    if fail_publish::take() {
        return Err(crate::data::invalid_data("故障注入：分岔發布失敗"));
    }
    write_state(root, world_id, &state)?;
    message_vars::refresh_cache(tx);
    Ok(new_scene)
}

/// 換場：把摘要包成一則 GM 旁白 append 到下一場景開頭，再把 current_scene +1 並存檔。
/// 回傳新場景號。摘要文字本身由呼叫端（單發 LLM）產生，這裡只負責落地與推進場次。
/// title 有值就存進「舊場景」（bump 前的 current_scene）的 scene_titles，與場次 +1 同一次 write_state。
#[cfg(test)]
pub fn begin_next_scene(
    root: &Path,
    world_id: &str,
    summary_text: &str,
    title: Option<&str>,
) -> DataResult<u64> {
    begin_next_scene_as(root, world_id, summary_text, title, false)
}

/// 同 `begin_next_scene`；`chapter_end` 為真＝VN 桌的章結束入口（GM `[chapter]`、「下一章」鈕）：
/// 章號加一或轉強制收尾與 `current_scene` 同一次提交（包 5 §2.3）。
pub fn begin_next_scene_as(
    root: &Path,
    world_id: &str,
    summary_text: &str,
    title: Option<&str>,
    chapter_end: bool,
) -> DataResult<u64> {
    let next = with_commit(root, world_id, |tx| {
        begin_next_scene_tx(tx, summary_text, title, chapter_end)
    });
    crate::data::bump_capacity_epoch();
    next
}

/// 發布順序（計畫 8.4）：①控制檔寫新幕種子（舊幕結束時的初始化來源、新 epoch）與新幕的世界書計時檔
/// ②新幕開頭的摘要事件 ③寫 current_scene。失敗就停、回錯；重試時新幕號仍是「目前幕＋1」，①② 覆寫上次沒發布的殘留。
fn begin_next_scene_tx(
    tx: &CommitTx<'_>,
    summary_text: &str,
    title: Option<&str>,
    chapter_end: bool,
) -> DataResult<u64> {
    let (root, world_id) = (tx.root, tx.world_id);
    message_vars::refuse_during_turn(tx)?;
    let mut state = read_state(root, world_id)?;
    // VN 章結束：新章號／強制收尾先寫進記憶體的樹，摘要事件明帶這份快照（補快照讀的是磁碟上的舊樹）
    let step = crate::data::vn::chapter_step(root, world_id, &state, chapter_end)?;
    for (path, value) in &step {
        crate::data::set_tree_value(&mut state.state.tree, path, value);
    }
    // VN 桌的摘要事件明帶快照：新章號（補快照讀磁碟上的舊樹），且不帶上一回合的變動標記（否則格邊旗標提示重播）
    let vn_snapshot = (state.table_kind == crate::data::TableKind::Vn).then(|| {
        let mut snapshot = state.state.clone();
        snapshot.changes.clear();
        snapshot
    });
    let old_scene = state.current_scene;
    let high = scene_high(&state);
    let next_scene = high + 1;
    // 角色卡隱藏快照在結算前拍（跟 current_scene 同一次 write_state 落地），退回／分岔回這幕時照它寫回
    record_leaving_scene(root, world_id, &mut state)?;
    state.pending_leave = None;
    // 變數模式：新幕開頭的摘要帶一份種子表，第一個 GM 回覆前卡片照樣讀得到值、補得到狀態欄占位
    let seed = message_vars::scene_seed_for_next(tx, old_scene, next_scene, |tree| {
        step.iter().fold(false, |changed, (path, value)| {
            crate::data::set_tree_value(tree, path, value) | changed
        })
    })?;
    // 世界書計時：舊幕 pending 先結算（GM 半截已由上面的交接代落），未到期的平移成新幕的檔（P4）
    super::super::world_info_store::carry_into_next_scene(root, world_id, old_scene, next_scene)?;
    // 上次沒發布的殘留（摘要寫了、current_scene 沒寫成）：新幕逐字稿重來
    super::super::world_file::commit_world_remove(&transcript_path(root, world_id, next_scene)?)?;
    // 摘要事件不回寫目前狀態：新章號只在下面 current_scene 那一次 write_state 發布
    append_unpublished_tx(
        tx,
        next_scene,
        &TranscriptEvent {
            id: None,
            message_vars: seed.as_ref().map(|vars| vars.seed.clone()),
            vars_rev: None,
            vars_epoch: seed.as_ref().map(|vars| vars.epoch.clone()),
            turn_key: None,
            action_id: None,
            raw: None,
            ts: local_timestamp()?,
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: TranscriptKind::Narration,
            text: summary_text.to_owned(),
            state: vn_snapshot,
            truncated: false,
            gm_only: false,
            marker: Some(EventMarker::SceneSummary),
            opening: false,
        },
    )?;
    if let Some(name) = title.map(str::trim).filter(|name| !name.is_empty()) {
        state
            .scene_titles
            .insert(old_scene.to_string(), name.to_owned());
    }
    let base = scene_label(&state, old_scene).base + 1;
    let version = next_scene_version(&state, high, base);
    state.scene_labels.insert(
        next_scene.to_string(),
        SceneLabel {
            base,
            version,
            parent: Some(old_scene),
            forked: false,
            source: None,
            keep: None,
            cut: None,
        },
    );
    state.current_scene = next_scene;
    #[cfg(test)]
    if fail_publish::take() {
        return Err(crate::data::invalid_data("故障注入：換幕發布失敗"));
    }
    write_state(root, world_id, &state)?;
    let requires_present_report =
        crate::data::requires_present_report(root, world_id, state.refactor_mode.as_deref());
    settle_card_visibility(
        root,
        world_id,
        old_scene,
        state.state.table.get("present").map(String::as_str),
        requires_present_report,
        crate::refactor::refactored(root, world_id, &state).unwrap_or(false),
    );
    Ok(next_scene)
}

/// 退回前幕：換幕的精確反向操作，純本地檔案處理不必呼叫模型。
/// 前一幕看 scene_labels 的 parent（原線／分岔都適用），不再假設一定是「幕號 -1」。
/// 只認「這一幕剛好一則事件」——begin_next_scene 保證新幕開頭就是那則摘要，
/// 多於一則代表玩家已經在這一幕行動過，退回會悄悄吃掉那些內容，所以直接擋，
/// 且擋下時故意先不動任何檔案／狀態（讀完才判斷），錯誤路徑不留副作用。
pub fn revert_scene(root: &Path, world_id: &str) -> DataResult<u64> {
    let reverted = with_commit(root, world_id, |tx| revert_scene_tx(tx));
    crate::data::bump_capacity_epoch();
    reverted
}

/// 退幕（計畫 8.4）：①寫 current_scene 切回父幕（父幕事件、epoch、種子原封不動）②成功後才移除子幕種子；
/// ② 失敗只留下用不到的殘留，下次換幕到同一號時會被覆寫。
fn revert_scene_tx(tx: &CommitTx<'_>) -> DataResult<u64> {
    let (root, world_id) = (tx.root, tx.world_id);
    let mut state = read_state(root, world_id)?;
    let scene = state.current_scene;
    let Some(previous_scene) = scene_label(&state, scene).parent else {
        return Err(UiMsg::SceneFirstNoPrevious.into_error());
    };
    message_vars::refuse_during_turn(tx)?;
    // 逐字稿的追加都走短提交鎖：這裡判完到刪檔之間不會插進新的一句
    if read_transcript(root, world_id, scene)?.len() != 1 {
        return Err(UiMsg::SceneRewindHasNewContent.into_error());
    }
    // 切換分支之後才遇得到（包 6 §2.4）：父幕已換幕往下走過，退回去會長出第二個換幕子幕；
    // 這幕被別幕當來源或分岔起點，刪了祖先鏈與源頭對應會指向不存在的幕
    let others = || {
        state
            .scene_labels
            .iter()
            .filter(|(key, _)| **key != scene.to_string())
    };
    if others().any(|(_, label)| !label.forked && label.parent == Some(previous_scene)) {
        return Err(UiMsg::SceneRewindNotTip.into_error());
    }
    if others().any(|(_, label)| label.source == Some(scene) || label.parent == Some(scene)) {
        return Err(UiMsg::SceneRewindReferenced.into_error());
    }
    // 世界書計時：子幕的 pending 先結算（變數意圖寫在不分幕的層，檔刪了就撤不回）；結算失敗就整個不退
    super::super::world_info_store::settle_pending(root, world_id, scene)?;
    // 角色卡隱藏照父幕快照寫回，全部寫成才切回父幕；失敗回錯、current_scene 不動，重試冪等
    restore_scene_visibility(root, world_id, &state, previous_scene)?;
    state.scene_card_visibility.remove(&scene.to_string());
    state.pending_leave = None;
    state.current_scene = previous_scene;
    state.scene_titles.remove(&previous_scene.to_string());
    // 自己這筆標籤跟著檔案一起消失，不留退回後查不到來源、卻還佔著 key 的殭屍紀錄。
    state.scene_labels.remove(&scene.to_string());
    // current_scene 落回前幕，前幕本來就對齊過了，aligned_scene 不用跟著動。
    state.state = read_transcript(root, world_id, previous_scene)?
        .iter()
        .rev()
        .find_map(|event| event.state.clone())
        .unwrap_or_default();
    // 先發布切回父幕，成功後才清子幕：清不掉只留下用不到的殘留（下次換到同一號時覆寫）
    write_state(root, world_id, &state)?;
    let _ = super::super::world_file::commit_world_remove(&transcript_path(root, world_id, scene)?);
    let _ = message_vars::drop_scene_seed(tx, scene);
    let _ = super::super::world_info_store::drop_scene(root, world_id, scene);
    message_vars::refresh_cache(tx);
    Ok(previous_scene)
}

/// 重寫目前這幕唯一那則摘要：摘要不滿意可以直接原地覆寫，不必先退回再重新換幕一次。
pub fn replace_scene_summary(
    root: &Path,
    world_id: &str,
    summary_text: &str,
    title: Option<&str>,
) -> DataResult<()> {
    let replaced = with_commit(root, world_id, |_| {
        replace_scene_summary_locked(root, world_id, summary_text, title)
    });
    crate::data::bump_capacity_epoch();
    replaced
}

fn replace_scene_summary_locked(
    root: &Path,
    world_id: &str,
    summary_text: &str,
    title: Option<&str>,
) -> DataResult<()> {
    let mut state = read_state(root, world_id)?;
    let scene = state.current_scene;
    let label = scene_label(&state, scene);
    let Some(previous_scene) = label.parent else {
        return Err(UiMsg::SummaryFirstScene.into_error());
    };
    // 分岔幕開頭那則是複製來的真實對話，不是摘要。源頭幕剛好只有一則時
    // 「只有一則」這道守門會放行，覆寫下去就把玩家的對話換成摘要了。
    if label.forked {
        return Err(UiMsg::SummaryContinuedScene.into_error());
    }
    let ts = local_timestamp()?;
    // 判斷與覆寫在同一把逐字稿鎖內：判完到寫回之間不會插進新的一句
    super::super::world_file::with_file_lock(
        &transcript_path(root, world_id, scene)?,
        |file| -> DataResult<()> {
            let mut events = match file.read()? {
                Some(bytes) => parse_transcript(&bytes)?,
                None => Vec::new(),
            };
            if events.len() != 1 {
                return Err(UiMsg::SummaryHasNewContent.into_error());
            }
            // 重寫的只有文字，其餘欄位原樣留著——尤其 state 那份快照：
            // 摘要是這一幕唯一一則，快照掉了之後退回這一幕會把狀態欄清成空的。
            let event = &mut events[0];
            event.text = summary_text.to_owned();
            event.marker = Some(EventMarker::SceneSummary);
            event.ts = ts;
            file.write_atomic(format!("{}\n", serde_json::to_string(event)?).as_bytes())
        },
    )?;

    match title.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => {
            state
                .scene_titles
                .insert(previous_scene.to_string(), name.to_owned());
        }
        None => {
            state.scene_titles.remove(&previous_scene.to_string());
        }
    }
    write_state(root, world_id, &state)?;
    Ok(())
}

/// 本幕之前、本分支最近一則開場事件（greeting-context-st-parity：換幕後 GM 的格式範例）。沿劇情祖先鏈
/// （分岔幕走來源幕、換幕來的走 parent）往回找，開場被收回的幕就再往上一層。
pub fn branch_opening(
    root: &Path,
    world_id: &str,
    state: &WorldState,
    scene: u64,
) -> DataResult<Option<TranscriptEvent>> {
    let mut next = story_previous(state, scene);
    // parent 一定比本幕小（換幕、分岔都是往後開新幕），上限只防壞檔成環
    let mut budget = scene;
    while let Some(previous) = next {
        if let Some(opening) = scene_opening(root, world_id, previous)? {
            return Ok(Some(opening));
        }
        if budget == 0 {
            break;
        }
        budget -= 1;
        next = story_previous(state, previous);
    }
    Ok(None)
}

/// 某一幕最後一則開場事件。先在原始位元組找開場旗標，沒有就不解析整幕。
fn scene_opening(root: &Path, world_id: &str, scene: u64) -> DataResult<Option<TranscriptEvent>> {
    let path = transcript_path(root, world_id, scene)?;
    let Some(bytes) = super::super::world_file::with_file_lock(&path, |file| file.read())? else {
        return Ok(None);
    };
    const FLAG: &[u8] = b"\"opening\":true";
    if !bytes.windows(FLAG.len()).any(|window| window == FLAG) {
        return Ok(None);
    }
    Ok(parse_transcript(&bytes)?
        .into_iter()
        .rev()
        .find(|event| event.opening))
}

#[cfg(test)]
mod tests;

/// 測試用故障注入：下一次換幕或分岔在發布 current_scene 前失敗（新幕的檔都已寫好）。
#[cfg(test)]
pub mod fail_publish {
    use std::cell::Cell;
    thread_local!(static ARMED: Cell<bool> = const { Cell::new(false) });
    pub fn arm() {
        ARMED.with(|armed| armed.set(true));
    }
    pub(in crate::data::scene) fn take() -> bool {
        ARMED.with(|armed| armed.replace(false))
    }
}
