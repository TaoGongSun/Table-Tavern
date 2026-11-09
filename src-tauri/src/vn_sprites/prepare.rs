//! 自動定稿（plans/vn-setup-wizard.md §2 ③）：建桌後在背景把每位沒定稿的角色「生候選 → 裝成定稿 → 通知舞台 → 補臉層」，
//! 已有候選的直接裝最新那張；精靈關掉也照做。玩家手動挑定、上傳、刪除優先：開工時記下定稿世代，
//! 裝之前搶位（世代沒變才佔住）、提交前再驗，世代一變就放棄。精靈裡按開始時正在再骰的角色，
//! 由 `adopt` 標記：那張候選一生好就裝成定稿。狀態只在記憶體：重開 app 時沒有在途工作。
use super::queue::{SlotStatus, STATUS_EVENT};
use super::store::SpriteDir;
use crate::{data, data_root};
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, MutexGuard, OnceLock};
use tauri::Emitter;

/// 定稿事件的 expression 欄（臉層事件是表情鍵，`base` 不是合法表情鍵）
pub(crate) const BASE_EXPRESSION: &str = "base";

type Key = (String, String);

#[derive(Default)]
struct Book {
    /// 正在生候選的角色（同一角色可能同時有兩次，所以計數）
    generating: HashMap<Key, u32>,
    /// 候選生好就裝成定稿：值是標記當下的定稿世代
    adopt: HashMap<Key, u64>,
    /// 正在跑自動定稿的桌
    running: HashSet<String>,
    /// 跑的期間又被要求：跑完再掃一輪（重試失敗的角色）
    again: HashSet<String>,
    /// 自動定稿排著、生成中或失敗的角色與原因（畫面重掛時補讀，事件可能早於監聽）
    pending: HashMap<Key, (&'static str, Option<String>)>,
}

fn book() -> MutexGuard<'static, Book> {
    static BOOK: OnceLock<Mutex<Book>> = OnceLock::new();
    BOOK.get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn key(world: &str, character: &str) -> Key {
    (world.to_owned(), character.to_owned())
}

/// 生候選期間持有；放掉就不算在生
pub(crate) struct Generating(Key);

impl Drop for Generating {
    fn drop(&mut self) {
        let mut book = book();
        if let Some(count) = book.generating.get_mut(&self.0) {
            *count -= 1;
            if *count == 0 {
                book.generating.remove(&self.0);
            }
        }
    }
}

pub(crate) fn generating(world: &str, character: &str) -> Generating {
    let key = key(world, character);
    *book().generating.entry(key.clone()).or_default() += 1;
    Generating(key)
}

fn is_generating(world: &str, character: &str) -> bool {
    book().generating.contains_key(&key(world, character))
}

fn mark_adopt(world: &str, character: &str) {
    let epoch = super::base_epoch(world, character);
    book().adopt.insert(key(world, character), epoch);
}

/// 候選剛寫好：被標記過就在背景裝成定稿（標記後玩家又手動換過就放棄）
pub(crate) fn adopt_if_marked(app: &tauri::AppHandle, world: &str, character: &str, file: &str) {
    let Some(epoch) = book().adopt.remove(&key(world, character)) else {
        return;
    };
    let (app, world, character, file) = (
        app.clone(),
        world.to_owned(),
        character.to_owned(),
        file.to_owned(),
    );
    tauri::async_runtime::spawn(async move {
        install(&app, &world, &character, &file, epoch).await;
    });
}

fn emit(
    app: &tauri::AppHandle,
    world: &str,
    character: &str,
    state: &'static str,
    reason: Option<String>,
) {
    {
        let mut book = book();
        match state {
            "done" => book.pending.remove(&key(world, character)),
            _ => book
                .pending
                .insert(key(world, character), (state, reason.clone())),
        };
    }
    let _ = app.emit(
        STATUS_EVENT,
        SlotStatus {
            world_id: world.to_owned(),
            character_id: character.to_owned(),
            variant: None,
            expression: BASE_EXPRESSION.to_owned(),
            state,
            reason,
        },
    );
}

/// 這桌自動定稿排著、生成中或失敗的角色（併進 `vn_sprites_status`）
pub(crate) fn snapshot(world: &str) -> Vec<SlotStatus> {
    book()
        .pending
        .iter()
        .filter(|((owner, _), _)| owner == world)
        .map(|((_, character), (state, reason))| SlotStatus {
            world_id: world.to_owned(),
            character_id: character.clone(),
            variant: None,
            expression: BASE_EXPRESSION.to_owned(),
            state,
            reason: reason.clone(),
        })
        .collect()
}

/// 排這桌的自動定稿（冪等：同桌已在跑就跑完再掃一輪，失敗的角色＝重試）。`adopt`＝按開始時正在再骰的角色，再骰完用新圖
pub(crate) fn start(app: &tauri::AppHandle, world: &str, adopt: &[String]) -> Result<(), String> {
    let root = data_root(app)?;
    data::vn::refuse_if_replay(&root, world)?;
    if !data::vn::art_flow(&root, world)
        .map_err(|e| e.to_string())?
        .started
    {
        return Ok(());
    }
    for character in adopt {
        if is_generating(world, character) {
            mark_adopt(world, character);
        }
    }
    {
        let mut book = book();
        if !book.running.insert(world.to_owned()) {
            book.again.insert(world.to_owned());
            return Ok(());
        }
    }
    let (app, world) = (app.clone(), world.to_owned());
    tauri::async_runtime::spawn(async move {
        loop {
            if let Err(reason) = run(&app, &world).await {
                if super::pause::is_pause(&reason) {
                    let mut held = book();
                    held.running.remove(&world);
                    held.again.remove(&world);
                    break;
                }
                log::warn!("自動定稿中止：{reason}");
            }
            let mut book = book();
            if !book.again.remove(&world) {
                book.running.remove(&world);
                break;
            }
        }
    });
    Ok(())
}

async fn run(app: &tauri::AppHandle, world: &str) -> Result<(), String> {
    let root = data_root(app)?;
    let cast: Vec<String> = data::list_characters(&root, world)
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|meta| !meta.archived && !meta.stage)
        .map(|meta| meta.id)
        .collect();
    // 先把要做的都標成排隊（③ 每列一打開就看得到）；開工時的世代：之後玩家手動換過定稿（含生候選這段期間）就放棄
    let mut todo = Vec::new();
    for character in cast {
        let Ok(dir) = SpriteDir::of(&root, world, &character) else {
            continue;
        };
        if book()
            .pending
            .get(&key(world, &character))
            .is_some_and(|(state, _)| *state == "failed")
        {
            continue;
        }
        if dir.read().is_some() {
            continue;
        }
        // 玩家正在 ③ 生這位的候選：生好就裝那張，不另生
        if is_generating(world, &character) {
            mark_adopt(world, &character);
            continue;
        }
        emit(app, world, &character, "queued", None);
        todo.push((character, dir));
    }
    for (character, dir) in todo {
        let epoch = super::base_epoch(world, &character);
        if is_generating(world, &character) {
            mark_adopt(world, &character);
        }
        if dir.read().is_some() || is_generating(world, &character) {
            emit(app, world, &character, "done", None);
            continue;
        }
        emit(app, world, &character, "running", None);
        let file = match dir.candidates().into_iter().next() {
            Some(file) => file,
            None => match super::generate_candidate(app, world, &character).await {
                Ok(file) => file,
                Err(reason) => {
                    if super::pause::is_pause(&reason) {
                        emit(app, world, &character, "queued", None);
                        return Err(reason);
                    }
                    emit(app, world, &character, "failed", Some(reason));
                    continue;
                }
            },
        };
        install(app, world, &character, &file, epoch).await;
    }
    Ok(())
}

/// 搶位後裝候選；設定已走完（開演了）就接著補臉層。被手動結果取代也發 done，讓畫面重讀
async fn install(app: &tauri::AppHandle, world: &str, character: &str, file: &str, epoch: u64) {
    let outcome = match super::claim_base(world, character, epoch) {
        None => Ok(false),
        Some(claimed) => {
            let png = data_root(app)
                .and_then(|root| SpriteDir::of(&root, world, character))
                .and_then(|dir| dir.candidate_bytes(file));
            match png {
                Ok(png) => super::install_at(app, world, character, png, Some(file), claimed).await,
                Err(reason) => Err(reason),
            }
        }
    };
    match outcome {
        Ok(installed) => {
            emit(app, world, character, "done", None);
            let started = data_root(app).is_ok_and(|root| {
                data::vn::art_flow(&root, world)
                    .is_ok_and(|art| art.started && art.appeared.iter().any(|id| id == character))
            });
            if installed && started {
                super::queue::fill(app, world, &[character.to_owned()]);
            }
        }
        Err(reason) => emit(app, world, character, "failed", Some(reason)),
    }
}

pub(crate) fn resume_pending(app: &tauri::AppHandle) {
    let worlds: HashSet<_> = book()
        .pending
        .iter()
        .filter(|(_, (state, _))| *state == "queued")
        .map(|((world, _), _)| world.clone())
        .collect();
    for world in worlds {
        let _ = start(app, &world, &[]);
    }
}
pub(crate) fn retry_world(world: &str) {
    book()
        .pending
        .retain(|(owner, _), (state, _)| owner != world || *state != "failed");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 手動挑定、上傳、刪除讓世代變：開工時記下的世代搶不到位，自動定稿放棄
    #[test]
    fn manual_change_beats_auto_install() {
        let (world, character) = ("prepare-w", "c");
        let started = crate::vn_sprites::base_epoch(world, character);
        crate::vn_sprites::supersede_base(world, character);
        assert_eq!(
            crate::vn_sprites::claim_base(world, character, started),
            None
        );
        let now = crate::vn_sprites::base_epoch(world, character);
        let claimed = crate::vn_sprites::claim_base(world, character, now).unwrap();
        // 搶到位後玩家再手動換：提交前的世代檢查擋下
        crate::vn_sprites::supersede_base(world, character);
        assert_ne!(crate::vn_sprites::base_epoch(world, character), claimed);
    }

    /// 再骰中標記：同一角色兩次生成都結束才算生完；標記記的是當下世代
    #[test]
    fn generating_counts_and_adopt_marks_epoch() {
        let (world, character) = ("prepare-g", "c");
        let first = generating(world, character);
        let second = generating(world, character);
        drop(first);
        assert!(is_generating(world, character));
        mark_adopt(world, character);
        assert_eq!(
            book().adopt.get(&key(world, character)).copied(),
            Some(crate::vn_sprites::base_epoch(world, character))
        );
        drop(second);
        assert!(!is_generating(world, character));
    }
}
