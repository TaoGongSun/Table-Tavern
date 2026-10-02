//! 轉換前備份與改用備份時另存的內容。清單不拿桌鎖；刪除要先拿到該桌獨占。

use std::fs;
use std::path::Path;

use serde::Serialize;

use super::super::paths::validate_id;
use super::super::world_file;
use super::super::world_lock::try_world_exclusive;
use super::commit::{self, combo_clean};
use super::marker::{self, FormatVersion};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BackupKind {
    Pre,
    Newer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct BackupRow {
    pub world_id: String,
    pub name: String,
    pub kind: BackupKind,
    pub size: u64,
    pub format_version: Option<u64>,
    pub deletable: bool,
    pub needs_repair: bool,
    /// 這份備份目錄的絕對路徑，給「打開資料夾」用。
    pub directory: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct BackupList {
    pub backups: Vec<BackupRow>,
    pub total_bytes: u64,
}

pub(crate) fn list_world_backups(root: &Path) -> Result<BackupList, String> {
    let ids = commit::discover_ids(root).map_err(|error| error.to_string())?;
    let mut backups = Vec::new();
    for id in ids {
        let repair = !combo_clean(root, &id);
        for kind in [BackupKind::Pre, BackupKind::Newer] {
            let dir = backup_dir(root, &id, kind);
            if !dir.is_dir() {
                continue;
            }
            backups.push(BackupRow {
                name: marker::loose_name(&dir, &id),
                size: dir_bytes(&dir),
                format_version: match marker::read_format(&dir).version {
                    FormatVersion::Known(version) => Some(version),
                    FormatVersion::Unknown => None,
                },
                deletable: !repair,
                needs_repair: repair,
                directory: dir.to_string_lossy().into_owned(),
                world_id: id.clone(),
                kind,
            });
        }
    }
    backups.sort_by(|left, right| {
        left.world_id
            .cmp(&right.world_id)
            .then_with(|| kind_ord(left.kind).cmp(&kind_ord(right.kind)))
    });
    let total_bytes = backups.iter().map(|row| row.size).sum();
    Ok(BackupList {
        backups,
        total_bytes,
    })
}

pub(crate) fn delete_world_backup(root: &Path, world_id: &str, kind: &str) -> Result<(), String> {
    validate_id(world_id).map_err(|error| error.to_string())?;
    let kind = parse_kind(kind).ok_or_else(|| "沒有這個備份".to_owned())?;
    let _exclusive =
        try_world_exclusive(world_id).ok_or_else(|| "這張桌正在處理中，請稍候再試".to_owned())?;
    if !commit::live_dir(root, world_id).is_dir() {
        return Err("主資料夾不在，需要修復".to_owned());
    }
    if !combo_clean(root, world_id) {
        return Err("目錄組合不乾淨，需要修復".to_owned());
    }
    let dir = backup_dir(root, world_id, kind);
    if !dir.is_dir() {
        return Err("沒有這個備份".to_owned());
    }
    world_file::remove_path_raw(&dir).map_err(|error| error.to_string())?;
    if let Some(parent) = dir.parent() {
        world_file::fsync_dir(parent).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn parse_kind(kind: &str) -> Option<BackupKind> {
    match kind {
        "pre" => Some(BackupKind::Pre),
        "newer" => Some(BackupKind::Newer),
        _ => None,
    }
}

fn kind_ord(kind: BackupKind) -> u8 {
    match kind {
        BackupKind::Pre => 0,
        BackupKind::Newer => 1,
    }
}

fn backup_dir(root: &Path, id: &str, kind: BackupKind) -> std::path::PathBuf {
    match kind {
        BackupKind::Pre => commit::pre_dir(root, id),
        BackupKind::Newer => commit::newer_dir(root, id),
    }
}

fn dir_bytes(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    let mut total = 0u64;
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            total = total.saturating_add(dir_bytes(&path));
        } else {
            total = total.saturating_add(meta.len());
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::super::super::world_lock::try_world_exclusive;
    use super::*;

    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("tt-bak-{}", ulid::Ulid::generate()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn id() -> String {
        ulid::Ulid::generate().to_string()
    }

    fn write_live(root: &Path, id: &str, name: &str) {
        let dir = commit::live_dir(root, id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("state.json"),
            format!(r#"{{"id":"{id}","name":"{name}"}}"#),
        )
        .unwrap();
    }

    fn write_backup(root: &Path, id: &str, kind: BackupKind, name: &str, bytes: &[u8]) {
        let dir = backup_dir(root, id, kind);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("state.json"),
            format!(r#"{{"id":"{id}","name":"{name}"}}"#),
        )
        .unwrap();
        fs::write(dir.join("blob"), bytes).unwrap();
    }

    #[test]
    fn list_names_the_backup_itself_and_delete_removes_only_that_kind() {
        let root = TempDir::new();
        let desk = id();
        write_live(&root.0, &desk, "現在");
        write_backup(&root.0, &desk, BackupKind::Pre, "轉換前", b"pre");
        write_backup(&root.0, &desk, BackupKind::Newer, "另存", b"newer-bytes");
        let list = list_world_backups(&root.0).unwrap();
        assert_eq!(list.backups.len(), 2);
        assert!(list
            .backups
            .iter()
            .all(|row| row.deletable && !row.needs_repair));
        assert_eq!(list.backups[0].name, "轉換前");
        assert_eq!(list.backups[1].name, "另存");
        assert!(list.total_bytes >= 4 + 11);
        assert_eq!(
            Path::new(&list.backups[0].directory),
            commit::pre_dir(&root.0, &desk)
        );
        assert_eq!(
            Path::new(&list.backups[1].directory),
            commit::newer_dir(&root.0, &desk)
        );
        assert!(Path::new(&list.backups[0].directory).is_absolute());
        delete_world_backup(&root.0, &desk, "pre").unwrap();
        let list = list_world_backups(&root.0).unwrap();
        assert_eq!(list.backups.len(), 1);
        assert_eq!(list.backups[0].kind, BackupKind::Newer);
        assert!(commit::live_dir(&root.0, &desk).is_dir());
    }

    #[test]
    fn missing_live_dir_dirty_combo_and_a_busy_table_cannot_be_deleted() {
        let root = TempDir::new();
        let gone = id();
        write_backup(&root.0, &gone, BackupKind::Pre, "只剩備份", b"x");
        let listed = list_world_backups(&root.0).unwrap();
        assert!(!listed.backups[0].deletable);
        assert!(
            Path::new(&listed.backups[0].directory).is_dir(),
            "需要修復的列也帶得出資料夾"
        );
        assert!(listed.backups[0].needs_repair);
        assert_eq!(
            delete_world_backup(&root.0, &gone, "pre").unwrap_err(),
            "主資料夾不在，需要修復"
        );

        let dirty = id();
        write_live(&root.0, &dirty, "半套");
        write_backup(&root.0, &dirty, BackupKind::Newer, "另存", b"y");
        fs::create_dir_all(root.0.join("worlds").join(format!(".tt-staging-{dirty}"))).unwrap();
        let row = list_world_backups(&root.0)
            .unwrap()
            .backups
            .into_iter()
            .find(|row| row.world_id == dirty)
            .unwrap();
        assert!(!row.deletable);
        assert!(row.needs_repair);
        assert_eq!(
            delete_world_backup(&root.0, &dirty, "newer").unwrap_err(),
            "目錄組合不乾淨，需要修復"
        );

        let busy = id();
        write_live(&root.0, &busy, "忙");
        write_backup(&root.0, &busy, BackupKind::Pre, "備份", b"z");
        let _held = try_world_exclusive(&busy).unwrap();
        assert_eq!(
            delete_world_backup(&root.0, &busy, "pre").unwrap_err(),
            "這張桌正在處理中，請稍候再試"
        );
        assert!(backup_dir(&root.0, &busy, BackupKind::Pre).is_dir());
        assert_eq!(
            delete_world_backup(&root.0, &busy, "side").unwrap_err(),
            "沒有這個備份"
        );
    }
}
