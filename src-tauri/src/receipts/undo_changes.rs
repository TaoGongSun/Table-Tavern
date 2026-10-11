//! 撤銷匯入實際改了哪些世界書條目、哪些欄組：給前端開著的條目草稿定基用（undo-import-editor-guard）。
//! 撤銷持整桌獨占，前後兩份快照之間只有撤銷自己寫世界書，比對結果就是撤銷的實際改動。
use crate::data::{self, WorldbookEntry};
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct EntryChanges {
    pub changed: Vec<EntryChange>,
    /// 撤銷刪掉的 uid；同一 uid 被換成另一條也算刪除
    pub deleted: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EntryChange {
    pub uid: u64,
    /// 編輯器草稿的欄組：title、keys、content、constant、enabled、order、visibility（含名單）
    pub groups: Vec<&'static str>,
}

/// `removed` 是撤銷刪掉的角色：前快照先剪掉它們再比，純剪名單的差異不算撤銷改欄（前端照刪角色修剪）。
/// `step_deleted` 是撤銷刪條目那步實際刪掉的 uid，之後可能被插回的條目佔走，仍算刪除。
pub(super) fn diff(
    before: &[WorldbookEntry],
    after: &[WorldbookEntry],
    step_deleted: &HashSet<u64>,
    removed: &[String],
) -> EntryChanges {
    let mut changes = EntryChanges::default();
    for old in before {
        let Some(new) = after.iter().find(|entry| entry.uid == old.uid) else {
            changes.deleted.push(old.uid);
            continue;
        };
        if step_deleted.contains(&old.uid) {
            changes.deleted.push(old.uid);
            continue;
        }
        let mut old = old.clone();
        data::scrub_entry(&mut old, removed);
        let groups = changed_groups(&old, new);
        if !groups.is_empty() {
            changes.changed.push(EntryChange {
                uid: old.uid,
                groups,
            });
        }
    }
    changes
}

fn changed_groups(old: &WorldbookEntry, new: &WorldbookEntry) -> Vec<&'static str> {
    [
        ("title", old.title != new.title),
        ("keys", old.keys != new.keys),
        ("content", old.content != new.content),
        ("constant", old.constant != new.constant),
        ("enabled", old.disabled != new.disabled),
        ("order", old.order != new.order),
        ("visibility", old.visibility != new.visibility),
    ]
    .into_iter()
    .filter(|(_, differs)| *differs)
    .map(|(group, _)| group)
    .collect()
}

#[cfg(test)]
mod tests;
