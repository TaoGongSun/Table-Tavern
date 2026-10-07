//! 網頁存檔匯入的逐字稿落地（桌檔契約 src/shared/contracts/web-save/）：整幕一次寫成一個 jsonl，不逐則
//! 走追加；有種子就同一把短提交鎖內發布變數模式。只給剛建好、這一幕還沒有事件的新桌用。
use super::super::message_vars::{self, Json, Macros, Source};
use super::super::state::{read_state_cache, write_state};
use super::super::state_commit::with_commit;
use super::super::{invalid_data, DataResult};
use super::transcript::{serialize_events, transcript_path, TranscriptEvent};
use std::path::Path;

/// 一則要落地的事件與它帶的變數表（變數模式才有）。`event` 的 id、表、版本 token、epoch、快照由這裡配。
pub struct ImportedEvent {
    pub event: TranscriptEvent,
    pub table: Option<Json>,
}

/// `seed`：Some＝這一幕走變數模式（開場完整表），None＝不動模式、事件不帶表。
pub struct ImportedScene {
    pub macros: Option<Macros>,
    pub seed: Option<Json>,
    pub events: Vec<ImportedEvent>,
}

/// 寫入這一幕，回每則落檔事件的 id（與 `events` 同序）。快照：變數模式時樹＝該則當下的有效表，
/// 其餘照目前檯面；檯面快取換成最後一則的快照。
pub fn write_imported_scene(
    root: &Path,
    world_id: &str,
    scene: u64,
    imported: ImportedScene,
) -> DataResult<Vec<String>> {
    with_commit(root, world_id, |tx| {
        let path = transcript_path(root, world_id, scene)?;
        if std::fs::metadata(&path).is_ok_and(|meta| meta.len() > 0) {
            return Err(invalid_data("web-save: 這一幕已經有事件，不能整段匯入"));
        }
        let epoch = match &imported.seed {
            Some(seed) => Some(message_vars::publish_imported_seed(
                tx,
                scene,
                imported.macros.clone(),
                seed,
            )?),
            None => None,
        };
        let mut world = read_state_cache(root, world_id)?;
        let base = world.state.clone();
        let mut effective = imported.seed.clone();
        let mut events = Vec::with_capacity(imported.events.len());
        for ImportedEvent { mut event, table } in imported.events {
            event.id = Some(message_vars::new_token());
            match (&epoch, table) {
                (Some(epoch), Some(table)) => {
                    event.message_vars = Some(message_vars::VarsTable::from_json(&table));
                    event.vars_rev = Some(message_vars::new_token());
                    event.vars_epoch = Some(epoch.clone());
                    effective = Some(table);
                }
                (None, Some(_)) => {
                    return Err(invalid_data("web-save: 帶變數表的訊息需要開場種子"));
                }
                (_, None) => {
                    event.message_vars = None;
                    event.vars_rev = None;
                    event.vars_epoch = None;
                }
            }
            let mut snapshot = base.clone();
            if let Some(table) = &effective {
                snapshot.tree = Source::Seed {
                    table: table.clone(),
                }
                .tree();
            }
            event.state = Some(snapshot);
            events.push(event);
        }
        super::super::world_file::commit_world_write_atomic(&path, &serialize_events(&events)?)?;
        if let Some(last) = events.last().and_then(|event| event.state.clone()) {
            world.state = last;
            write_state(root, world_id, &world)?;
        }
        message_vars::refresh_cache(tx);
        Ok(events
            .into_iter()
            .map(|event| event.id.unwrap_or_default())
            .collect())
    })
}
