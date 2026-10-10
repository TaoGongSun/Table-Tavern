//! 實送落地與 `pending` 回滾（三之 4「落地時機」「失敗回滾」、三之 8）。
//!
//! 一次實送：`begin_landing`（寫新表＋pending 寫入中）→ 變數層各筆 `push_var_intent`／`update_var_intent`
//! （包 5b）→ `mark_sent` → 交給傳輸層。成功＝這個回合的正文確實落檔（`reply_landed` 清 pending）；
//! 確定失敗由持許可的路徑叫 `fail_turn`；其餘留給下一次實送掃描前、換幕、分岔的 `settle_pending`。
use super::notices::{append_notices, Notice};
use super::{modify_scene, read_scene, Pending, Perspective, Stage, VarIntent};
use crate::data::card_vars::{self, CasRestore, Layer};
use crate::data::state_commit::with_commit;
use crate::data::{invalid_data, message_vars, read_transcript, DataResult};
use crate::world_info::timed::WiTimed;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 撤回變數層時沒能還原的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictReason {
    /// 寫成之後層又被別人寫過：保留別人的寫入
    Overwritten,
    /// 不知道寫成沒有、而層已經不是寫入前的版本：分不出是不是自己寫的，保留
    Unknown,
}

/// 一筆沒還原的變數層寫入（要回報玩家）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VarConflict {
    pub layer: Layer,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer_id: Option<String>,
    pub reason: ConflictReason,
}

/// 撤回時沒還原的變數寫入怎麼回報：寫進待回報檔（換幕、分岔、結算、回合收尾沒有回傳可附），
/// 或交回呼叫端連同原錯一起回報（聊天呼叫當場失敗）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Report {
    Notices,
    Inline,
}

fn matching<'a>(pending: &'a mut Option<Pending>, turn_id: &str) -> DataResult<&'a mut Pending> {
    pending
        .as_mut()
        .filter(|pending| pending.turn_key == turn_id)
        .ok_or_else(|| invalid_data("world-info: 沒有這個回合的落地紀錄"))
}

/// 落地第一步：該視角換成本輪掃描後的表，同一次原子寫記下 pending（寫入中、落地前的表）。
/// 還有沒結的 pending 就拒絕（實送前一定先結算）。
pub fn begin_landing(
    root: &Path,
    world_id: &str,
    scene: u64,
    turn_id: &str,
    perspective: &Perspective,
    landed: &WiTimed,
) -> DataResult<()> {
    with_commit(root, world_id, |_| {
        modify_scene(root, world_id, scene, |timed| {
            if timed.pending.is_some() {
                return Err(invalid_data("world-info: 上一次落地還沒結算"));
            }
            let key = perspective.key();
            let before = timed.perspectives.get(&key).cloned().unwrap_or_default();
            timed
                .perspectives
                .insert(key.clone(), super::StoredTimed::from_scan(landed));
            timed.pending = Some(Pending {
                turn_key: turn_id.to_owned(),
                perspective: key,
                stage: Stage::Writing,
                before,
                vars: Vec::new(),
            });
            Ok(())
        })
    })
}

/// 記一筆變數層寫入意圖（寫層之前落檔），回它的序號。
pub fn push_var_intent(
    root: &Path,
    world_id: &str,
    scene: u64,
    turn_id: &str,
    intent: VarIntent,
) -> DataResult<usize> {
    with_commit(root, world_id, |_| {
        modify_scene(root, world_id, scene, |timed| {
            let pending = matching(&mut timed.pending, turn_id)?;
            if pending.stage != Stage::Writing {
                return Err(invalid_data("world-info: 已送出的落地不能再記變數意圖"));
            }
            pending.vars.push(intent);
            Ok(pending.vars.len() - 1)
        })
    })
}

/// 改一筆變數意圖（補寫入後 rev、CAS 重試前更新預期 rev 與寫入前內容）並落檔。
pub fn update_var_intent(
    root: &Path,
    world_id: &str,
    scene: u64,
    turn_id: &str,
    index: usize,
    work: impl FnOnce(&mut VarIntent),
) -> DataResult<()> {
    with_commit(root, world_id, |_| {
        modify_scene(root, world_id, scene, |timed| {
            let pending = matching(&mut timed.pending, turn_id)?;
            let intent = pending
                .vars
                .get_mut(index)
                .ok_or_else(|| invalid_data("world-info: 沒有這筆變數意圖"))?;
            work(intent);
            Ok(())
        })
    })
}

/// 交給傳輸層之前：階段改成已送出。
pub fn mark_sent(root: &Path, world_id: &str, scene: u64, turn_id: &str) -> DataResult<()> {
    with_commit(root, world_id, |_| {
        modify_scene(root, world_id, scene, |timed| {
            let pending = matching(&mut timed.pending, turn_id)?;
            if pending.stage != Stage::Writing {
                return Err(invalid_data("world-info: 這次落地已經送出過"));
            }
            pending.stage = Stage::Sent;
            Ok(())
        })
    })
}

/// 回合正文落檔（GM 正文 append、角色回覆的 `character_turn` 路）：pending 是這個回合、已送出就清掉。
/// 寫入中的不動（不會發生：送出前不會有正文）。沒有對應的 pending（已結算、不是這回合）什麼都不做。
pub fn reply_landed(root: &Path, world_id: &str, scene: u64, turn_id: &str) -> DataResult<()> {
    with_commit(root, world_id, |_| {
        let current = read_scene(root, world_id, scene)?;
        let landed = current
            .pending
            .as_ref()
            .is_some_and(|pending| pending.turn_key == turn_id && pending.stage == Stage::Sent);
        if !landed {
            return Ok(());
        }
        modify_scene(root, world_id, scene, |timed| {
            if timed
                .pending
                .as_ref()
                .is_some_and(|pending| pending.turn_key == turn_id)
            {
                timed.pending = None;
            }
            Ok(())
        })
    })
}

/// 確定失敗（持許可的路徑當場判得出：呼叫回錯、沒有任何正文的中止）：這個回合的 pending 照失敗撤回。
/// 不是這個回合的 pending 不動。
pub fn fail_turn(
    root: &Path,
    world_id: &str,
    scene: u64,
    turn_id: &str,
    report: Report,
) -> DataResult<Vec<VarConflict>> {
    with_commit(root, world_id, |_| {
        match read_scene(root, world_id, scene)?.pending {
            Some(pending) if pending.turn_key == turn_id => {
                undo(root, world_id, scene, &pending, report)
            }
            _ => Ok(Vec::new()),
        }
    })
}

/// 結算 pending（下一次持許可的實送掃描之前、換幕、分岔；量測不跑）：
/// - 寫入中（還沒送出就中斷）：計時還原，變數意圖逐筆撤回，沒還原的先寫進待回報檔才清 pending。
/// - 已送出：逐字稿裡有這個回合鍵＝成功，清掉；沒有＝失敗，只還原計時（變數副作用保留）。
///
/// 失敗一律包成 `WorldInfoSettleFailed`（玩家看得懂、前端給重設出路，見 `reset_scene`）。
pub fn settle_pending(root: &Path, world_id: &str, scene: u64) -> DataResult<()> {
    settle_inner(root, world_id, scene).map_err(|error| {
        crate::ui_msg::UiMsg::WorldInfoSettleFailed {
            error: error.to_string(),
        }
        .into_error()
    })
}

fn settle_inner(root: &Path, world_id: &str, scene: u64) -> DataResult<()> {
    with_commit(root, world_id, |_| {
        let Some(pending) = read_scene(root, world_id, scene)?.pending else {
            return Ok(());
        };
        if pending.stage == Stage::Sent && turn_in_transcript(root, world_id, scene, &pending)? {
            return modify_scene(root, world_id, scene, |timed| {
                timed.pending = None;
                Ok(())
            });
        }
        undo(root, world_id, scene, &pending, Report::Notices).map(|_| ())
    })
}

/// 判斷成敗唯一看逐字稿的地方：只認回合鍵，不從內容推斷；只有正文算（GM `main`、角色 `character`），
/// `state_update` 這類附屬事件不算。
fn turn_in_transcript(
    root: &Path,
    world_id: &str,
    scene: u64,
    pending: &Pending,
) -> DataResult<bool> {
    Ok(read_transcript(root, world_id, scene)?.iter().any(|event| {
        event.turn_key.as_ref().is_some_and(|key| {
            key.turn_id == pending.turn_key
                && (key.part == message_vars::PART_MAIN || key.part == message_vars::PART_CHARACTER)
        })
    }))
}

/// 失敗撤回：寫入中先逐筆處理變數意圖；回報落檔（或交回呼叫端）之後，同一次原子寫還原計時、清 pending。
fn undo(
    root: &Path,
    world_id: &str,
    scene: u64,
    pending: &Pending,
    report: Report,
) -> DataResult<Vec<VarConflict>> {
    let mut conflicts = Vec::new();
    let mut notices = Vec::new();
    if pending.stage == Stage::Writing {
        for (index, intent) in pending.vars.iter().enumerate() {
            if let Some(reason) = undo_intent(root, world_id, scene, pending, index, intent)? {
                let conflict = VarConflict {
                    layer: intent.layer,
                    layer_id: intent.id.clone(),
                    reason,
                };
                notices.push(Notice {
                    id: format!("{}:{index}", pending.turn_key),
                    turn_key: pending.turn_key.clone(),
                    conflict: conflict.clone(),
                });
                conflicts.push(conflict);
            }
        }
    }
    if report == Report::Notices && !notices.is_empty() {
        append_notices(root, world_id, &notices)?;
    }
    modify_scene(root, world_id, scene, |timed| {
        if timed
            .pending
            .as_ref()
            .is_some_and(|current| current.turn_key == pending.turn_key)
        {
            timed
                .perspectives
                .insert(pending.perspective.clone(), pending.before.clone());
            timed.pending = None;
        }
        Ok(())
    })?;
    Ok(conflicts)
}

/// 一筆變數意圖的四種情形（三之 8「崩潰恢復」）；回 `Some` 是沒還原、要回報。
fn undo_intent(
    root: &Path,
    world_id: &str,
    scene: u64,
    pending: &Pending,
    index: usize,
    intent: &VarIntent,
) -> DataResult<Option<ConflictReason>> {
    let layer_id = intent.id.as_deref();
    let Some(after_rev) = &intent.after_rev else {
        // 沒有結果：層還是預期 rev＝沒寫成，不動；否則分不出是不是自己寫的
        let current = card_vars::read_layer(root, world_id, intent.layer, layer_id)?.rev;
        return Ok((current != intent.expected_rev).then_some(ConflictReason::Unknown));
    };
    let restore_rev = match &intent.restore_rev {
        Some(rev) => rev.clone(),
        None => {
            let rev = message_vars::new_token();
            let chosen = rev.clone();
            update_var_intent(root, world_id, scene, &pending.turn_key, index, |stored| {
                stored.restore_rev = Some(chosen);
            })?;
            rev
        }
    };
    let before = intent.expected_rev.as_ref().map(|_| intent.before.as_str());
    Ok(
        match card_vars::restore_if_rev(
            root,
            world_id,
            intent.layer,
            layer_id,
            after_rev,
            before,
            &restore_rev,
        )? {
            CasRestore::Restored | CasRestore::AlreadyRestored => None,
            CasRestore::Moved => Some(ConflictReason::Overwritten),
        },
    )
}
