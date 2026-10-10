//! 幕的生命週期（三之 4）：換幕平移、分岔複製、退幕刪檔，與網頁存檔匯入轉成計時檔。
//! 換幕與分岔都在各自的 tx 裡、寫 `current_scene` 之前呼叫，先結算 pending 再寫新幕；新幕的檔一律覆寫殘留。
use super::landing::settle_pending;
use super::{
    chat_length, read_scene, scene_path, write_scene, Perspective, SceneTimed, StoredEffect,
    StoredTimed,
};
use crate::data::world_file::commit_world_remove;
use crate::data::{read_transcript, DataResult};
use std::collections::BTreeMap;
use std::path::Path;

/// 換幕（P4）：先結算舊幕的 pending，再把各視角的計時 start、end 各減去舊幕則數寫成新幕的檔
/// （新幕從摘要之後算起，摘要不計）。全部平移、不先挑掉到期的：到期與 sticky 接冷卻交給下一次掃描的檢查，
/// 結果與 ST 不換幕一路數下去相同。沒有計時也寫空檔。
pub fn carry_into_next_scene(
    root: &Path,
    world_id: &str,
    old_scene: u64,
    new_scene: u64,
) -> DataResult<()> {
    settle_pending(root, world_id, old_scene)?;
    let length = chat_length(&read_transcript(root, world_id, old_scene)?) as i64;
    let mut timed = read_scene(root, world_id, old_scene)?;
    for table in timed.perspectives.values_mut() {
        table.shift(-length);
    }
    timed.pending = None;
    write_scene(root, world_id, new_scene, &timed)
}

/// 分岔：先結算目前幕（被離開、之後只能靠退幕回來的那一幕；它的變數意圖寫在不分幕的 chat／global 層，
/// 留著就撤不回）與來源幕的 pending，再把來源幕結算後的計時檔複製成新幕的檔（不帶 pending）。逐字稿整幕
/// 複製，則數不變，不用平移。
pub fn copy_for_fork(
    root: &Path,
    world_id: &str,
    current_scene: u64,
    from_scene: u64,
    new_scene: u64,
) -> DataResult<()> {
    settle_pending(root, world_id, current_scene)?;
    settle_pending(root, world_id, from_scene)?;
    let mut timed = read_scene(root, world_id, from_scene)?;
    timed.pending = None;
    write_scene(root, world_id, new_scene, &timed)
}

/// 退幕：子幕的計時檔跟逐字稿一起刪（呼叫端盡力而為，事先已用 `settle_pending` 結算過）；父幕的檔原封不動。
pub fn drop_scene(root: &Path, world_id: &str, scene: u64) -> DataResult<()> {
    commit_world_remove(&scene_path(root, world_id, scene)?)
}

/// 網頁存檔的一筆計時，穩定 ID 已換成桌面 uid。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebTimed {
    pub cooldown: bool,
    pub uid: u64,
    pub start: i64,
    pub end: i64,
    pub protected: bool,
}

/// 網頁存檔匯入：存檔的 `timed` 轉成這一幕這個視角的計時檔（交給演這張卡的人，P1）。
/// 兩個穩定 ID 對到同一 uid 時，同一類取 end 較大的那筆，end 相同時 protected 為真者優先。
/// 網頁訊息與這一幕的事件一對一、都不是系統事件，兩邊則數相同，start／end 照用。
pub fn import_web_timed(
    root: &Path,
    world_id: &str,
    scene: u64,
    perspective: &Perspective,
    effects: &[WebTimed],
) -> DataResult<()> {
    let mut table = StoredTimed::default();
    for effect in effects {
        let target = if effect.cooldown {
            &mut table.cooldown
        } else {
            &mut table.sticky
        };
        let incoming = StoredEffect {
            start: effect.start,
            end: effect.end,
            protected: effect.protected,
            confidential: false,
        };
        let key = effect.uid.to_string();
        let wins = target
            .get(&key)
            .is_none_or(|kept| (incoming.end, incoming.protected) > (kept.end, kept.protected));
        if wins {
            target.insert(key, incoming);
        }
    }
    write_scene(
        root,
        world_id,
        scene,
        &SceneTimed {
            perspectives: BTreeMap::from([(perspective.key(), table)]),
            ..SceneTimed::default()
        },
    )
}
