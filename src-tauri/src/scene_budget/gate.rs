//! 換幕容量鎖的後端關卡（計畫 §3.6–3.7）。前端停用按鈕只是提示，這裡才是防線：
//! 會寫進本幕的 AI 動作（玩家句、角色接話、GM 旁白／推進、重試、卡片介面送出）寫入前都過一次。
//!
//! 同一個玩家動作只算一次（動作收據）：玩家句先落檔、再叫回覆，第二次若再加完整的回覆預測，
//! 會把剛通過的回覆錯擋。第一個進來的指令通過就發收據，同一 `action_id` 的後續指令見到有效收據直接放行。
//! 收據失效：容量世代（`data::capacity_epoch`，換幕／退回／分岔／重寫提要／改設定都推進）變了、
//! 幕或設定指紋不同、同桌來了別的動作（含沒帶 id 的舊呼叫端）、超過 10 分鐘。
use super::SummaryBudget;
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const RECEIPT_TTL: Duration = Duration::from_secs(600);

#[derive(Debug, Clone)]
struct Receipt {
    action_id: String,
    epoch: u64,
    scene: u64,
    config_gen: String,
    issued: Instant,
}

static RECEIPTS: Mutex<BTreeMap<String, Receipt>> = Mutex::new(BTreeMap::new());

/// 判定的輸入：容量世代、目前的幕、設定指紋、換幕容量（None＝拿不到上限，不鎖）。
pub struct Snapshot {
    pub epoch: u64,
    pub scene: u64,
    pub config_gen: String,
    pub summary: Option<SummaryBudget>,
}

/// 同一動作已有有效收據就不必重算（呼叫端據此省掉整幕組裝）。
pub fn has_receipt(
    world_id: &str,
    action_id: &str,
    epoch: u64,
    scene: u64,
    config_gen: &str,
) -> bool {
    let Ok(receipts) = RECEIPTS.lock() else {
        return false;
    };
    receipts.get(world_id).is_some_and(|receipt| {
        receipt.action_id == action_id
            && receipt.epoch == epoch
            && receipt.scene == scene
            && receipt.config_gen == config_gen
            && receipt.issued.elapsed() < RECEIPT_TTL
    })
}

/// 關卡本體：`draft` 是這次要落的玩家句（無玩家句的動作傳空字串）。
/// 同桌只留一張收據：新動作（或沒帶 id 的呼叫）一進來就撤掉舊的；通過且帶 id 才發新收據。
pub fn admit(world_id: &str, action_id: Option<&str>, draft: &str, snapshot: &Snapshot) -> bool {
    if let Some(action_id) = action_id {
        if has_receipt(
            world_id,
            action_id,
            snapshot.epoch,
            snapshot.scene,
            &snapshot.config_gen,
        ) {
            return true;
        }
    }
    let Ok(mut receipts) = RECEIPTS.lock() else {
        return snapshot
            .summary
            .as_ref()
            .is_none_or(|summary| !summary.would_overflow(summary.draft(draft)));
    };
    receipts.remove(world_id);
    if let Some(summary) = &snapshot.summary {
        if summary.would_overflow(summary.draft(draft)) {
            return false;
        }
    }
    if let Some(action_id) = action_id {
        receipts.insert(
            world_id.to_owned(),
            Receipt {
                action_id: action_id.to_owned(),
                epoch: snapshot.epoch,
                scene: snapshot.scene,
                config_gen: snapshot.config_gen.clone(),
                issued: Instant::now(),
            },
        );
    }
    true
}

#[cfg(test)]
pub(super) fn clear_for_test(world_id: &str) {
    if let Ok(mut receipts) = RECEIPTS.lock() {
        receipts.remove(world_id);
    }
}
