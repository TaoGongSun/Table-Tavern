//! 重新重構（refactor-statusbar-skeleton 待問 1〔作者裁決 2026-10-03〕）：已重構的桌再按重構，
//! - 已有遊玩資料（見 `played`）：擋下，請玩家開新桌（未重構的桌也一樣〔作者裁決 2026-10-11〕）；
//! - 未遊玩：清回剛匯入原卡的狀態（照匯入原檔重匯），再走一般重構流程——永不拿重構後的資料再重構；
//! - 沒有匯入原檔或來源不完整（舊桌、收據讀壞、原檔記不下來）：擋下，請玩家開新桌。
//!
//! 未重構的桌照現狀直接重構。清回的細節〔模型判斷·未裁決，主線同意〕：保留桌名、模型指定與已貼的
//! 開場白（同一則原文、同一個序號重貼在原本那筆匯入之後），world.md 照抄〔作者裁決 2026-10-11〕，其餘照原路徑依序重匯；
//! 未遊玩＝`played` 為否。
//!
//! 並行〔模型判斷·未裁決〕：整段持這桌的獨占鎖，拿到鎖才檢查 Ready；鎖被佔（匯入、貼訊息、另一次重設）
//! 就回「這桌忙」。臨時根裡的重匯沿用同一張獨占憑證。
//!
//! 失敗復原〔模型判斷·未裁決〕：整桌先在臨時資料根（data::reset_build_root）完整重建，全部成功、讀得
//! 完整、重放出來的來源與開場序號跟預期逐筆一致才換掉原桌（data::replace_world_from_build：搬成 staging、寫日誌、I→T、S→I、刪 T）。重建或交換
//! 前任何一步失敗都刪掉臨時根、原桌原封不動並回錯；交換途中當機由下次開桌照恢復表接續或退回。
use crate::data::{self, DataResult};
use crate::receipts::{self, ImportReplay, ImportRoute};
use crate::ui_msg::UiMsg;
use crate::{import, mechanism, transport};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RerunStatus {
    /// 沒重構過：照一般流程直接重構
    Fresh,
    /// 重構過、已有遊玩資料：擋下
    Played,
    /// 重構過、未遊玩，但沒有匯入原檔或來源不完整：擋下
    NoSource,
    /// 重構過、未遊玩、來源完整：確認後清回重跑
    Ready,
}

/// 這桌重構過沒：有玩法標記，或留有重構產物（舊重構桌沒有標記、清殼後也還在）。
pub(crate) fn refactored(
    root: &Path,
    world_id: &str,
    state: &data::WorldState,
) -> DataResult<bool> {
    Ok(state.refactor_mode.is_some() || data::read_refactor_outcome(root, world_id)?.is_some())
}

/// 已遊玩〔作者裁決 2026-10-11〕：換過幕、第 0 幕有開場白以外的事件（舊紀錄沒有 opening 欄位的照算）、
/// 有生成中或已提交還沒落檔的 GM 回覆（含中止的半截），或是網頁存檔匯入的桌（天生有遊玩紀錄〔作者裁決 2026-10-12〕）。
/// 生成中的回合也算：在途回合即將成為遊玩紀錄；失敗或沒有正文就中止的回合會解除，不會一直擋著。
/// 已遊玩的桌一律不能重構，請玩家開新桌。
pub(crate) fn played(root: &Path, world_id: &str, state: &data::WorldState) -> DataResult<bool> {
    if state.current_scene > 0 || data::web_save_sidecar_path(root, world_id)?.exists() {
        return Ok(true);
    }
    if data::read_transcript(root, world_id, 0)?
        .iter()
        .any(|event| !event.opening)
    {
        return Ok(true);
    }
    // 回合在生成中或已提交等落檔：那一輪的回覆遲早會落進逐字稿
    Ok(data::state_commit::with_commit(root, world_id, |tx| {
        data::message_vars::busy(tx) || data::message_vars::has_unlanded_reply(tx)
    }))
}

/// 已遊玩的桌拒套重構時回的錯（前端靠它換成提醒文案，一字不差）。
pub const REFACTOR_PLAYED: &str = "refactor-played";

/// 一項都沒勾就套用時回的錯：桌子不動〔作者裁決 2026-10-27〕。前端已把套用鈕反灰，這裡守前端以外的入口。
pub const REFACTOR_EMPTY_SELECTION: &str = "refactor-empty-selection";

/// 重構套用的後端關卡（呼叫端已持整桌獨占）：已遊玩就回 `REFACTOR_PLAYED`，什麼都不寫、不換世代
/// （回合紀錄照留，等著落檔的回覆不受影響）；否則照 `apply_and_record` 套用，之後整桌交換（清回合紀錄、
/// 世代加一，舊殼的在途卡寫被擋）。
pub fn apply_to_table(
    root: &Path,
    world_id: &str,
    outcome: &super::RefactorOutcome,
    selection: &super::RefactorSelection,
    assets: &[super::card_png::CardAsset],
    record_receipt: bool,
    held: &data::WorldExclusive,
) -> Result<super::RefactorApplySummary, String> {
    if selection.is_empty() {
        return Err(REFACTOR_EMPTY_SELECTION.to_owned());
    }
    let state = data::read_state(root, world_id).map_err(|error| error.to_string())?;
    if played(root, world_id, &state).map_err(|error| error.to_string())? {
        return Err(REFACTOR_PLAYED.to_owned());
    }
    // 中途失敗會在裡面記收據並立刻撤銷，回到零寫入（見 refactor/apply_record.rs）
    let result = super::apply_and_record(
        root,
        world_id,
        outcome,
        selection,
        assets,
        record_receipt,
        held,
    );
    data::message_vars::world_swapped(root, world_id);
    result.map_err(|error| error.to_string())
}

pub fn rerun_status(root: &Path, world_id: &str) -> DataResult<RerunStatus> {
    let state = data::read_state(root, world_id)?;
    if played(root, world_id, &state)? {
        return Ok(RerunStatus::Played);
    }
    if !refactored(root, world_id, &state)? {
        return Ok(RerunStatus::Fresh);
    }
    Ok(match receipts::import_replays(root, world_id)? {
        Some(replays) if !replays.is_empty() => RerunStatus::Ready,
        _ => RerunStatus::NoSource,
    })
}

/// 重設的結果：交換已提交；臨時根清不掉時是 CleanupPending（前端一樣當成功，殘留由下次開桌或下次
/// 重設清掉）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResetOutcome {
    Committed,
    CommittedCleanupPending,
}

/// 清回剛匯入原卡的狀態。整段持這桌的獨占鎖（匯入、貼訊息、另一次重設都進不來；在途的寫入要等它放開），
/// 拿到鎖才檢查 Ready；失敗回 Err，原桌不動。
pub fn reset_to_import_source(root: &Path, world_id: &str, lang: &str) -> DataResult<ResetOutcome> {
    let Some(exclusive) = data::try_world_exclusive(world_id) else {
        return Err(UiMsg::WorldBusy.into_error());
    };
    let status = rerun_status(root, world_id)?;
    if status != RerunStatus::Ready {
        return Err(data::invalid_data(format!("refactor-rerun-{status:?}")));
    }
    let expected = receipts::import_replays(root, world_id)?
        .ok_or_else(|| data::invalid_data("refactor-rerun-NoSource"))?;
    let state = data::read_state(root, world_id)?;
    let events = data::read_transcript(root, world_id, 0)?;
    // world.md 照抄〔作者裁決 2026-10-11〕：檔不在當空白，其他讀取錯誤照回（不能把讀不到當成清空）
    let world_md = match data::read_world_md(root, world_id) {
        Ok(text) => text,
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound) =>
        {
            String::new()
        }
        Err(error) => return Err(error),
    };
    let build_root = data::reset_build_root(root, world_id);
    // 上一次中斷留下的臨時根先清掉；清不掉就不做（不能在舊殘留上重建）
    data::remove_reset_build_root(root, world_id)?;
    let built = rebuild(
        &build_root,
        world_id,
        &state,
        &world_md,
        &events,
        &expected,
        lang,
        &exclusive,
    )
    .and_then(|()| verify_rebuilt(&build_root, world_id, &expected))
    .and_then(|()| data::replace_world_from_build(root, world_id));
    if let Err(error) = built {
        let _ = data::remove_reset_build_root(root, world_id);
        return Err(error);
    }
    Ok(match data::remove_reset_build_root(root, world_id) {
        Ok(()) => ResetOutcome::Committed,
        Err(_) => ResetOutcome::CommittedCleanupPending,
    })
}

/// 重建桌交換前驗完整：重建時的記帳都要成功（沒有未完成標記、收據讀得到），而且重放出來的來源與開場
/// 序號跟預期逐筆一致（原檔識別每次新發，不比）。任何不符都不交換，原桌不動。
fn verify_rebuilt(build_root: &Path, world_id: &str, expected: &[ImportReplay]) -> DataResult<()> {
    let rebuilt = receipts::import_replays(build_root, world_id)?
        .ok_or_else(|| data::invalid_data("refactor-rerun-rebuild-incomplete"))?;
    let same = rebuilt.len() == expected.len()
        && rebuilt.iter().zip(expected).all(|(got, want)| {
            got.source.route == want.source.route
                && got.source.label == want.source.label
                && got.source.color == want.source.color
                && got.bytes == want.bytes
                && got.opening == want.opening
        });
    if same {
        Ok(())
    } else {
        Err(data::invalid_data("refactor-rerun-rebuild-mismatch"))
    }
}

fn rebuild(
    build_root: &Path,
    world_id: &str,
    state: &data::WorldState,
    world_md: &str,
    events: &[data::TranscriptEvent],
    replays: &[ImportReplay],
    lang: &str,
    held: &data::WorldExclusive,
) -> DataResult<()> {
    data::create_reset_world(
        build_root,
        world_id,
        &state.name,
        state.model_bindings.clone(),
        world_md,
    )?;
    let raw_text = |event: &data::TranscriptEvent| event.raw.clone().unwrap_or(event.text.clone());
    let mut reposted = Vec::new();
    for replay in replays {
        let source = match replay.source.route {
            ImportRoute::Worldbook => {
                import::import_worldbook_file(
                    build_root,
                    world_id,
                    &replay.bytes,
                    &replay.source.label,
                    held,
                )?
                .source
            }
            ImportRoute::Character => {
                // 失敗時整個臨時根會收掉，殘檔不必叫玩家清：換回原錯
                import::import_character_file(
                    build_root,
                    world_id,
                    &replay.bytes,
                    &replay.source.color,
                    lang,
                    held,
                )
                .map_err(import::original_import_error)?
                .source
            }
        };
        // 開場白重貼在原本那筆匯入之後，綁同一筆匯入、同一個序號
        if let Some((scene, ts, index)) = &replay.opening {
            if let Some(event) = events.iter().find(|event| &event.ts == ts) {
                import::post_opening_text(
                    build_root,
                    world_id,
                    *scene,
                    ts,
                    &raw_text(event),
                    lang,
                    *index,
                    source.as_deref(),
                    held,
                )?;
                reposted.push(ts.clone());
            }
        }
    }
    // 沒掛在任何匯入收據上的開場白：原文照貼，不掛收據
    for event in events.iter().filter(|event| !reposted.contains(&event.ts)) {
        let text = raw_text(event);
        let block = transport::extract_state_block(&text);
        let user_name = transport::player_fallback_name(lang);
        let (_, outcome) =
            data::append_opening(build_root, world_id, 0, &event.ts, &text, &block, user_name)?;
        mechanism::append_log(build_root, world_id, 0, &outcome.records);
    }
    Ok(())
}
