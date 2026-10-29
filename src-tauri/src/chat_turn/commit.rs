//! GM 回合的提交：回合紀錄（TurnGuard）、狀態套用與狀態更新事件。
use super::gm::GmNarration;
use crate::{data, mechanism, transport};
use serde::Serialize;

/// gm_narrate 提交時套用這一輪的狀態（鎖內、套在回合固定的輸入上）。狀態更新一律盡力而為：模型格式壞掉或
/// 存檔寫不進去，都不該害玩家丟掉整段旁白；骰值要每回合重擲，就算這一輪模型完全沒吐更新也要跑一次。
/// `loaded` 是回合開始時載入的機制設定（數字欄更新策略與 incremental 以它為準）；`takeover` 與這一輪導演指示
/// 同一個判定：介面接管桌只認 `<UpdateVariable>`，狀態區塊不收並回饋。要不要套用看過濾前的區塊——接管桌
/// incremental 一律為真、必套，先後不影響結果。
pub(crate) fn gm_commit_apply(
    state: &mut data::WorldState,
    stat: Option<&data::message_vars::Json>,
    block: &transport::StateBlock,
    user_name: &str,
    loaded: &data::Mechanism,
    takeover: bool,
    align: bool,
) -> (
    Option<mechanism::Outcome>,
    Option<data::message_vars::TypedBatch>,
) {
    let apply = !block.fields.is_empty()
        || !block.lists.is_empty()
        || !block.updates.is_empty()
        || loaded.incremental;
    let outcome = apply.then(|| {
        let scene = state.current_scene;
        state.mechanism.numeric_update = loaded.numeric_update;
        let scope = if takeover {
            mechanism::BlockScope::TakeoverReply
        } else if state.statusbar_retired {
            mechanism::BlockScope::RetiredCardBlocks
        } else {
            mechanism::BlockScope::Table
        };
        let outcome = mechanism::apply_block_typed(state, block, user_name, stat, scope);
        if align {
            state.aligned_scene = Some(scene);
        }
        outcome
    });
    let typed = outcome.as_ref().and_then(|outcome| outcome.typed.clone());
    (outcome, typed)
}

#[derive(Serialize)]
pub(super) struct StateUpdate {
    path: String,
    value: String,
}

/// 一輪 GM 回合的回合紀錄（計畫 8.3）。開始時記下、提交時核對 turn_id；沒走到提交就離開（中止、出錯、
/// 空回覆）時，drop 把還在生成中的紀錄改成已中止，卡寫與面板手改隨即放行。
pub(super) struct TurnGuard {
    root: std::path::PathBuf,
    world_id: String,
    turn_id: String,
    /// 同回合內部追加（登場紀錄）用的憑證
    pub(super) ticket: data::message_vars::TurnTicket,
    /// 回合開始時的巨集代換值（zod 引擎路徑用）
    macros: data::message_vars::Macros,
}

/// 變動紀錄（與前端落的那則同一個寫法）；沒有變動就沒有這一部分。
/// 這回合要記進狀態更新事件的行：長文字欄的新值，加上正文剝掉、提交後仍沒收進狀態的格子（整值）。
pub(super) fn state_update_lines(
    state: &data::WorldState,
    user_name: &str,
    pending: &[mechanism::Cell],
) -> Vec<StateUpdate> {
    transport::snapshot_updates(&state.state, &state.mechanism, user_name)
        .into_iter()
        .chain(mechanism::unapplied(state, pending))
        .map(|(path, value)| StateUpdate { path, value })
        .collect()
}

pub(super) fn state_update_side(updates: &[StateUpdate]) -> Option<data::message_vars::TurnSide> {
    if updates.is_empty() {
        return None;
    }
    Some(data::message_vars::TurnSide {
        part: "state_update".to_owned(),
        kind: data::TranscriptKind::System,
        text: updates
            .iter()
            .map(|update| format!("{}：{}", update.path, update.value))
            .collect::<Vec<_>>()
            .join("\n"),
        marker: Some(data::EventMarker::StateUpdate),
    })
}

impl TurnGuard {
    pub(super) fn begin(
        root: &std::path::Path,
        world_id: &str,
        turn_id: &str,
        lang: &str,
    ) -> Result<Self, String> {
        let user = data::read_player_card(root, world_id)
            .ok()
            .flatten()
            .map(|card| card.name)
            .unwrap_or_else(|| transport::player_fallback_name(lang).to_owned());
        let macros = data::message_vars::Macros { user, char: None };
        let ticket = data::state_commit::with_commit(root, world_id, |tx| {
            data::message_vars::begin_turn(tx, turn_id, Some(&macros))
        })
        .map_err(|error| error.to_string())?;
        Ok(Self {
            root: root.to_path_buf(),
            world_id: world_id.to_owned(),
            turn_id: turn_id.to_owned(),
            ticket,
            macros,
        })
    }

    /// zod 卡的提交：同 `commit` 的回傳；整次失敗時 `error` 帶給玩家的那句、正文照落。
    pub(super) async fn commit_engine(
        &self,
        zod: crate::mvu_engine::wiring::ZodTable,
        main: data::message_vars::PendingMain,
        reply: &str,
        block: &transport::StateBlock,
        user_name: &str,
        sides: impl FnOnce(
            &data::WorldState,
            &Option<mechanism::Outcome>,
        ) -> Vec<data::message_vars::TurnSide>,
    ) -> Result<Option<data::message_vars::GmCommit<Option<mechanism::Outcome>>>, String> {
        crate::mvu_engine::wiring::gm_commit(
            &self.root,
            &self.world_id,
            &self.turn_id,
            zod,
            main,
            reply,
            block,
            user_name,
            self.macros.clone(),
            sides,
        )
        .await
    }

    /// turn_id／幕／世代已不符回 Ok(None)；讀寫錯誤回 Err（呼叫端回報，不害玩家丟掉整段旁白）。
    pub(super) fn commit<R>(
        &self,
        main: data::message_vars::PendingMain,
        apply: impl FnOnce(
            &mut data::WorldState,
            Option<&data::message_vars::Json>,
        ) -> (R, Option<data::message_vars::TypedBatch>),
        sides: impl FnOnce(&data::WorldState, &R) -> Vec<data::message_vars::TurnSide>,
    ) -> Result<Option<data::message_vars::GmCommit<R>>, String> {
        data::state_commit::with_commit(&self.root, &self.world_id, |tx| {
            data::message_vars::apply_gm_block(tx, &self.turn_id, main, apply, sides)
        })
        .map_err(|error| error.to_string())
    }

    /// 中止：半截正文記進回合紀錄（前端沒落成時由下一筆新事件前的交接代落），回原樣的中止結果。
    pub(super) fn abort(&self, narration: GmNarration) -> GmNarration {
        let half = data::message_vars::PendingMain {
            text: narration.text.clone(),
            raw: None,
            truncated: true,
        };
        let _ = data::state_commit::with_commit(&self.root, &self.world_id, |tx| {
            data::message_vars::finish_turn(tx, &self.turn_id, Some(half))
        });
        narration
    }
}

impl Drop for TurnGuard {
    fn drop(&mut self) {
        let _ = data::state_commit::with_commit(&self.root, &self.world_id, |tx| {
            data::message_vars::finish_turn(tx, &self.turn_id, None)
        });
    }
}

#[cfg(test)]
#[path = "commit_tests.rs"]
mod tests;
