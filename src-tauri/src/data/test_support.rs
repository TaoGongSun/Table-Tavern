use super::{CharacterCard, Tier, Visibility, WorldbookEntry};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

pub(super) struct TestRoot(PathBuf);

impl TestRoot {
    pub(super) fn new(label: &str) -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir()
            .join(format!("table-tavern-{label}-{}-{id}", std::process::id()));
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

pub(super) fn character_card(id: &str, name: &str) -> CharacterCard {
    CharacterCard {
        id: id.to_owned(),
        name: name.to_owned(),
        color: "#333333".to_owned(),
        avatar: "🎭".to_owned(),
        tier: Tier::Balanced,
        show_image: true,
        archived: false,
        gen_prompt: String::new(),
        public_md: String::new(),
        private_md: String::new(),
    }
}

pub(super) fn worldbook_entry(uid: u64, title: &str) -> WorldbookEntry {
    WorldbookEntry {
        uid,
        title: title.to_owned(),
        keys: vec!["霧".to_owned()],
        content: format!("{title}內容"),
        constant: false,
        order: 10,
        disabled: false,
        visibility: Visibility::Gm,
        is_person: false,
        locked: false,
    }
}

pub(super) fn write_worldbook_fixture(root: &TestRoot, world_id: &str, entries: serde_json::Value) {
    fs::write(
        root.path()
            .join(format!("worlds/{world_id}/worldbook.json")),
        serde_json::to_string_pretty(&serde_json::json!({ "entries": entries })).unwrap(),
    )
    .unwrap();
}

pub(super) fn read_worldbook_fixture(root: &TestRoot, world_id: &str) -> serde_json::Value {
    serde_json::from_str(
        &fs::read_to_string(
            root.path()
                .join(format!("worlds/{world_id}/worldbook.json")),
        )
        .unwrap(),
    )
    .unwrap()
}
