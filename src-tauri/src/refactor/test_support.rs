use super::types::RefactorApplyResult;
use super::{apply, RefactorCharacter, RefactorOutcome, RefactorSelection};
use crate::data::{self, Visibility, WorldbookEntry};
use crate::receipts;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

pub(super) struct TestRoot(PathBuf);

impl TestRoot {
    pub(super) fn new(label: &str) -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "table-tavern-refactor-{label}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) fn seed_entry(root: &Path, world_id: &str, title: &str, content: &str) -> u64 {
    data::upsert_worldbook_entry(
        root,
        world_id,
        WorldbookEntry {
            uid: u64::MAX,
            title: title.to_owned(),
            keys: Vec::new(),
            content: content.to_owned(),
            constant: false,
            order: 1,
            disabled: false,
            visibility: Visibility::Gm,
            is_person: false,
            locked: false,
        },
    )
    .unwrap()
}

pub(super) fn character(name: &str, source_uids: &[u64]) -> RefactorCharacter {
    RefactorCharacter {
        name: name.to_owned(),
        emoji: "🙂".to_owned(),
        public_md: format!("{name}的公開設定"),
        private_md: format!("{name}的私密設定"),
        source_uids: source_uids.iter().map(u64::to_string).collect(),
        solo_entry_md: format!("{name}的獨立條目"),
        suspected_player: false,
    }
}

pub(super) fn no_player_selection(character_indices: Vec<usize>) -> RefactorSelection {
    RefactorSelection {
        character_indices,
        apply_interface: false,
        mechanism_indices: Vec::new(),
        entry_indices: Vec::new(),
        player_index: None,
    }
}

/// 比照 receipts.rs 既有測試的作法：套用前先 snapshot，套用後記收據，undo 走 receipts 那條路。
pub(super) fn apply_recorded(
    root: &Path,
    world_id: &str,
    outcome: &RefactorOutcome,
    selection: &RefactorSelection,
) -> RefactorApplyResult {
    let before = receipts::snapshot(root, world_id);
    let result = apply(root, world_id, outcome, selection).unwrap();
    receipts::record_refactor_apply(
        root,
        world_id,
        "AI 卡重構",
        result.character_ids.clone(),
        result.rewritten_entries.clone(),
        result.deleted_entries.clone(),
        before,
    );
    result
}
