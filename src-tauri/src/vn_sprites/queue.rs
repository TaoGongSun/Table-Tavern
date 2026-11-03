//! 臉層背景佇列（plans/vn-mode-pkg7.md §4 第 5 步）：狀態不落檔，由「有定稿但缺哪些臉層」推導，
//! 重開 app 再補一次就續生；失敗只記在記憶體。一次一張（同時只用一個立繪來源，agy 並行會 429）。
use super::store::SpriteDir;
use crate::data_root;
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};
use tauri::Emitter;

pub(crate) const STATUS_EVENT: &str = "vn-sprite-status";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Slot {
    world: String,
    character: String,
    /// None＝共用表情組；變體鍵＝該變體的獨立組
    variant: Option<String>,
    expression: &'static str,
}

#[derive(Default)]
struct Queue {
    waiting: VecDeque<Slot>,
    running: Option<Slot>,
    failed: HashMap<Slot, String>,
    worker: bool,
}

fn queue() -> std::sync::MutexGuard<'static, Queue> {
    static QUEUE: OnceLock<Mutex<Queue>> = OnceLock::new();
    QUEUE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SlotStatus {
    pub world_id: String,
    pub character_id: String,
    pub variant: Option<String>,
    pub expression: String,
    /// queued／running／done／failed
    pub state: &'static str,
    pub reason: Option<String>,
}

fn status(slot: &Slot, state: &'static str, reason: Option<String>) -> SlotStatus {
    SlotStatus {
        world_id: slot.world.clone(),
        character_id: slot.character.clone(),
        variant: slot.variant.clone(),
        expression: slot.expression.to_owned(),
        state,
        reason,
    }
}

fn emit(app: &tauri::AppHandle, payload: SlotStatus) {
    let _ = app.emit(STATUS_EVENT, payload);
}

/// 這桌目前在佇列、生成中、失敗的格
pub(crate) fn snapshot(world: &str) -> Vec<SlotStatus> {
    let queue = queue();
    let running = queue
        .running
        .iter()
        .map(|slot| status(slot, "running", None));
    let waiting = queue
        .waiting
        .iter()
        .map(|slot| status(slot, "queued", None));
    let failed = queue
        .failed
        .iter()
        .map(|(slot, reason)| status(slot, "failed", Some(reason.clone())));
    running
        .chain(waiting)
        .chain(failed)
        .filter(|item| item.world_id == world)
        .collect()
}

/// 推導並排入這桌缺的臉層（冪等：已在佇列、生成中、失敗過的不重排）
pub(crate) fn fill(app: &tauri::AppHandle, world: &str, characters: &[String]) {
    let Ok(root) = data_root(app) else {
        return;
    };
    let mut added = Vec::new();
    {
        let mut queue = queue();
        for character in characters {
            let Ok(dir) = SpriteDir::of(&root, world, character) else {
                continue;
            };
            for (variant, expression) in dir.missing_faces() {
                let slot = Slot {
                    world: world.to_owned(),
                    character: character.clone(),
                    variant,
                    expression,
                };
                if queue.running.as_ref() != Some(&slot)
                    && !queue.waiting.contains(&slot)
                    && !queue.failed.contains_key(&slot)
                {
                    queue.waiting.push_back(slot.clone());
                    added.push(slot);
                }
            }
        }
    }
    for slot in &added {
        emit(app, status(slot, "queued", None));
    }
    start_worker(app);
}

/// 換定稿、改框、刪立繪後：清掉這角色的失敗記錄與排隊格（在途那張寫回時會被重驗擋下）
pub(crate) fn forget(world: &str, character: &str) {
    let mut queue = queue();
    let mine = |slot: &Slot| slot.world == world && slot.character == character;
    queue.waiting.retain(|slot| !mine(slot));
    queue.failed.retain(|slot, _| !mine(slot));
}

/// 玩家重骰一格：清掉失敗記錄，交給下一次 fill 排入
pub(crate) fn forget_slot(world: &str, character: &str, set: Option<&str>, expression: &str) {
    queue().failed.retain(|slot, _| {
        !(slot.world == world
            && slot.character == character
            && slot.variant.as_deref() == set
            && slot.expression == expression)
    });
}

fn start_worker(app: &tauri::AppHandle) {
    {
        let mut queue = queue();
        if queue.worker || queue.waiting.is_empty() {
            return;
        }
        queue.worker = true;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let slot = {
                let mut queue = queue();
                match queue.waiting.pop_front() {
                    Some(slot) => {
                        queue.running = Some(slot.clone());
                        slot
                    }
                    None => {
                        queue.running = None;
                        queue.worker = false;
                        break;
                    }
                }
            };
            emit(&app, status(&slot, "running", None));
            let outcome = super::face_job(
                &app,
                &slot.world,
                &slot.character,
                slot.variant.as_deref(),
                slot.expression,
            )
            .await;
            let mut requeue = false;
            {
                let mut queue = queue();
                queue.running = None;
                match &outcome {
                    Ok(true) => {}
                    // 開工後定稿、框或該格被換掉（成功或失敗都一樣）：丟掉結果，缺的格交給下面重新推導
                    Ok(false) => requeue = true,
                    Err(reason) => {
                        queue.failed.insert(slot.clone(), reason.clone());
                    }
                }
            }
            match outcome {
                Ok(_) => emit(&app, status(&slot, "done", None)),
                Err(reason) => emit(&app, status(&slot, "failed", Some(reason))),
            }
            // 過期任務：角色仍在、這格仍缺才依目前定稿重排（fill 當時因它在跑而略過）
            let still_missing = requeue
                && data_root(&app).ok().is_some_and(|root| {
                    super::character_exists(&root, &slot.world, &slot.character)
                        && SpriteDir::of(&root, &slot.world, &slot.character).is_ok_and(|dir| {
                            dir.missing_faces()
                                .contains(&(slot.variant.clone(), slot.expression))
                        })
                });
            if still_missing {
                let mut queue = queue();
                if !queue.waiting.contains(&slot) {
                    queue.waiting.push_back(slot);
                }
            }
        }
    });
}
