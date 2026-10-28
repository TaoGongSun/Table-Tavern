//! 匯入記帳（import_character／import_worldbook／refactor_apply 成功後呼叫）：拿呼叫前的快照
//! 與呼叫後的現況比對，只記「這次實際新增或改動了什麼」，寫成一筆收據。復原見 `undo`。
//! 記帳失敗不得影響匯入成功與否。

use super::{
    append_receipt, append_receipt_atomic, read_receipts, read_receipts_checked,
    worldbook_entry_fingerprint, write_receipts, ImportReceipt, MechanismUndo, ModeUndo,
    PostedOpening, ReceiptWorldbookEntry, Recorded, Snapshot,
};
use super::{discard_import_source, ImportSource};
use crate::data::{self, Trigger, WorldState, WorldbookEntry};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

/// post_opening 成功後呼叫：把剛貼上檯面的開場白（含玩家挑的序號）掛到它所屬那筆匯入的收據，undo 時
/// 一併收掉。`import` 是那次匯入的原檔識別（匯入回傳、前端帶回）；沒帶識別時退回最後一筆收據，但帶了
/// 序號就必須有識別——不能用「最後一筆」代替，重複匯入時會掛到別張卡。
/// 回 false＝序號歸屬不了或記不下來：呼叫端不解除「未完成」標記，來源判不完整。
pub fn record_posted_opening(
    root: &Path,
    world_id: &str,
    scene: u64,
    ts: &str,
    index: Option<usize>,
    import: Option<&str>,
    _held: &data::WorldExclusive,
) -> bool {
    let Some(mut receipts) = read_receipts_checked(root, world_id) else {
        return false;
    };
    let target = match import {
        Some(file) => receipts.iter_mut().rev().find(|receipt| {
            receipt
                .import_source
                .as_ref()
                .is_some_and(|source| source.file == file)
        }),
        None if index.is_some() => None,
        None => receipts.last_mut(),
    };
    let Some(target) = target else {
        // 沒有收據可掛（什麼都沒新增的匯入）：沒挑序號就沒東西要記；挑了序號卻歸屬不了＝不完整
        return index.is_none();
    };
    target.opening = Some(PostedOpening {
        scene,
        ts: ts.to_owned(),
        index,
    });
    write_receipts(root, world_id, &receipts).is_ok()
}

/// 一般化前後 diff：新增的鍵（undo 時整條移除）、被覆寫的鍵（undo 時整條恢復成舊值）。
/// 沒被這次匯入動到的鍵完全不會出現在任一份結果裡。
fn diff_added_and_overwritten<K, V>(
    before: &BTreeMap<K, V>,
    after: &BTreeMap<K, V>,
) -> (Vec<K>, BTreeMap<K, V>)
where
    K: Ord + Clone,
    V: PartialEq + Clone,
{
    let added = after
        .keys()
        .filter(|key| !before.contains_key(*key))
        .cloned()
        .collect();
    let overwritten = before
        .iter()
        .filter(|(key, value)| after.get(*key).is_some_and(|new_value| new_value != *value))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    (added, overwritten)
}

/// 匯入呼叫之後比對 mechanism／狀態樹前後差異；`before` 是這桌在匯入前的完整快照。
fn diff_mechanism(
    before: Option<&WorldState>,
    root: &Path,
    world_id: &str,
) -> Option<MechanismUndo> {
    let before = before?;
    let after = data::read_state(root, world_id).ok()?;

    let (added_rule_keys, restored_rules) =
        diff_added_and_overwritten(&before.mechanism.rules, &after.mechanism.rules);

    let as_map = |triggers: &[Trigger]| -> BTreeMap<String, Trigger> {
        triggers
            .iter()
            .cloned()
            .map(|t| (t.id.clone(), t))
            .collect()
    };
    let (added_trigger_ids, restored_triggers) = diff_added_and_overwritten(
        &as_map(&before.mechanism.triggers),
        &as_map(&after.mechanism.triggers),
    );

    let incremental_before = (before.mechanism.incremental != after.mechanism.incremental)
        .then_some(before.mechanism.incremental);

    let (added_state_keys, mut restored_state) =
        diff_added_and_overwritten(&before.state.tree, &after.state.tree);
    // 介面套用會整份重建狀態樹；被新 schema 淘汰的舊頂層鍵也要記回原值，讓 undo 能插回。
    restored_state.extend(
        before
            .state
            .tree
            .iter()
            .filter(|(key, _)| !after.state.tree.contains_key(*key))
            .map(|(key, value)| (key.clone(), value.clone())),
    );

    let player_card_assigned = before.player_card_id.is_none() && after.player_card_id.is_some();
    let guide_before =
        (before.mechanism.guide != after.mechanism.guide).then(|| before.mechanism.guide.clone());
    let value_types_before = (before.mechanism.value_types != after.mechanism.value_types)
        .then(|| before.mechanism.value_types.clone());
    let display_formats_before = (before.mechanism.display_formats
        != after.mechanism.display_formats)
        .then(|| before.mechanism.display_formats.clone());
    let display_prefixes_before = (before.mechanism.display_prefixes
        != after.mechanism.display_prefixes)
        .then(|| before.mechanism.display_prefixes.clone());
    let opening_before = (before.mechanism.opening != after.mechanism.opening)
        .then(|| before.mechanism.opening.clone());
    let collections_before = (before.mechanism.collections != after.mechanism.collections)
        .then(|| before.mechanism.collections.clone());
    let mirrors_before = (before.mechanism.mirrors != after.mechanism.mirrors)
        .then(|| before.mechanism.mirrors.clone());
    let statusbar_retired_before =
        (before.statusbar_retired != after.statusbar_retired).then_some(before.statusbar_retired);

    let removed_table: BTreeMap<String, String> = before
        .state
        .table
        .iter()
        .filter(|(key, _)| !after.state.table.contains_key(*key))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();

    let undo = MechanismUndo {
        removed_table,
        added_rule_keys,
        restored_rules,
        added_trigger_ids,
        restored_triggers,
        incremental_before,
        added_state_keys,
        restored_state,
        player_card_assigned,
        guide_before,
        value_types_before,
        display_formats_before,
        display_prefixes_before,
        opening_before,
        collections_before,
        mirrors_before,
        statusbar_retired_before,
    };
    (!undo.is_empty()).then_some(undo)
}

/// 機制帳本是純 append 檔，這次操作新增的內容＝目前檔案內容扣掉快照時的舊內容那段前綴。
/// 對不上前綴（檔案被外力改過，理論上不會發生）就當沒有新增，undo 那端找不到片段時
/// 本來就會靜默放棄，不會誤刪玩家的合法紀錄。
fn diff_ledger_suffix(before: &str, root: &Path, world_id: &str) -> String {
    let Ok(path) = data::mechanism_log_path(root, world_id) else {
        return String::new();
    };
    let after = fs::read_to_string(path).unwrap_or_default();
    after.strip_prefix(before).unwrap_or_default().to_owned()
}

fn new_worldbook_entries(
    root: &Path,
    world_id: &str,
    before_uids: &HashSet<u64>,
) -> Vec<ReceiptWorldbookEntry> {
    data::read_worldbook(root, world_id)
        .unwrap_or_default()
        .into_iter()
        .filter(|entry| !before_uids.contains(&entry.uid))
        .map(|entry| ReceiptWorldbookEntry {
            uid: entry.uid,
            fingerprint: worldbook_entry_fingerprint(&entry),
        })
        .collect()
}

/// 這次匯入是否新建了一份這桌等級的卡片介面殼（source-card.*）；匯入前就有的不算。
fn detect_world_card_created(root: &Path, world_id: &str, before: &Snapshot) -> Option<String> {
    for (extension, existed_before) in [
        ("png", before.world_card.0),
        ("import.json", before.world_card.1),
    ] {
        if existed_before {
            continue;
        }
        if data::world_card_path(root, world_id, extension).is_ok_and(|path| path.exists()) {
            return Some(extension.to_owned());
        }
    }
    None
}

/// 這次匯入是否新建了 GM 卡的圖（gm.png）；匯入前就有的不算（含被覆寫掉的舊圖）。
fn detect_gm_image_created(root: &Path, world_id: &str, before: &Snapshot) -> bool {
    !before.gm_image_existed && data::gm_image_path(root, world_id).is_ok_and(|path| path.exists())
}

/// 這次操作是否新建了介面骨架檔（interface-shell.html）；套用前就有的不算。
fn detect_interface_shell_created(root: &Path, world_id: &str, before: &Snapshot) -> bool {
    !before.interface_shell_existed
        && data::interface_shell_path(root, world_id).is_ok_and(|path| path.exists())
}

/// import_character 指令成功後呼叫：新角色卡本身永遠是這次匯入的具體產物，一律留一筆收據
/// （即使卡片沒帶世界書／機制資料，「刪掉這張新卡」仍然是有意義的復原動作）。
pub fn record_character_import(
    root: &Path,
    world_id: &str,
    character_id: &str,
    label: &str,
    before: Snapshot,
    visibility_restores: Vec<data::VisibilityRestore>,
    import_source: Option<ImportSource>,
) -> Recorded {
    let worldbook_entries = new_worldbook_entries(root, world_id, &before.worldbook_uids);
    let mechanism = diff_mechanism(before.state.as_ref(), root, world_id);
    let recorded = append_receipt(
        root,
        world_id,
        ImportReceipt {
            kind: "character".to_owned(),
            label: label.to_owned(),
            timestamp: data::local_timestamp().unwrap_or_default(),
            character_id: Some(character_id.to_owned()),
            character_ids: Vec::new(),
            worldbook_entries,
            mechanism,
            world_card_created: None,
            gm_image_created: false,
            opening: None,
            renamed_from: None,
            rewritten_entries: Vec::new(),
            deleted_entries: Vec::new(),
            deleted_entries_raw: Vec::new(),
            visibility_restores,
            stage_restores: Vec::new(),
            added_ledger_lines: String::new(),
            interface_shell_created: false,
            interface_shell_restore: None,
            refactor_mode: None,
            import_source,
        },
    );
    if recorded {
        Recorded::Yes
    } else {
        Recorded::Failed
    }
}

/// import_worldbook 指令成功後呼叫：這條路徑沒有角色卡本體，實際新增為零（entries／機制／
/// 介面殼皆無變化，例如整份都跟既有內容重複）就不留空收據——按鈕不該對「什麼都沒發生」的
/// 匯入也亮出來，連按復原時也不該吃到一筆空操作。
pub fn record_worldbook_import(
    root: &Path,
    world_id: &str,
    label: &str,
    before: Snapshot,
    visibility_restores: Vec<data::VisibilityRestore>,
    import_source: Option<ImportSource>,
) -> Recorded {
    let worldbook_entries = new_worldbook_entries(root, world_id, &before.worldbook_uids);
    let mechanism = diff_mechanism(before.state.as_ref(), root, world_id);
    let world_card_created = detect_world_card_created(root, world_id, &before);
    let gm_image_created = detect_gm_image_created(root, world_id, &before);
    // 只有合併改寫、沒有新增的匯入也是有效收據：撤銷要把可見度退回去
    if worldbook_entries.is_empty()
        && mechanism.is_none()
        && world_card_created.is_none()
        && !gm_image_created
        && visibility_restores.is_empty()
    {
        // 什麼都沒新增：不留收據，這次存下的原檔也沒有用（重匯它不會多出任何東西）
        if let Some(source) = &import_source {
            discard_import_source(root, world_id, &source.file);
        }
        return Recorded::NothingNew;
    }
    let recorded = append_receipt(
        root,
        world_id,
        ImportReceipt {
            kind: "worldbook".to_owned(),
            label: label.to_owned(),
            timestamp: data::local_timestamp().unwrap_or_default(),
            character_id: None,
            character_ids: Vec::new(),
            worldbook_entries,
            mechanism,
            world_card_created,
            gm_image_created,
            opening: None,
            renamed_from: None,
            rewritten_entries: Vec::new(),
            deleted_entries: Vec::new(),
            deleted_entries_raw: Vec::new(),
            visibility_restores,
            stage_restores: Vec::new(),
            added_ledger_lines: String::new(),
            interface_shell_created: false,
            interface_shell_restore: None,
            refactor_mode: None,
            import_source,
        },
    );
    if recorded {
        Recorded::Yes
    } else {
        Recorded::Failed
    }
}

/// refactor_apply 指令成功後呼叫：AI 卡重構可能一次新增多張角色卡、多條世界書條目，
/// 並改寫或停用既有條目——收據記「實際套用的那份」，undo 才能逐項退回。
/// 跟 record_worldbook_import 一樣：實際套用為零就不留空收據。
/// record_refactor_apply 的結果：沒有任何變動可記／已記下（最後一筆就是這次）／寫不進去。
#[derive(Debug, PartialEq, Eq)]
pub enum RefactorRecord {
    Nothing,
    Recorded,
    Failed,
}

pub fn record_refactor_apply(
    root: &Path,
    world_id: &str,
    label: &str,
    character_ids: Vec<String>,
    rewritten_entries: Vec<WorldbookEntry>,
    deleted_entries: Vec<WorldbookEntry>,
    deleted_entries_raw: Vec<serde_json::Value>,
    stage_restores: Vec<data::StageChange>,
    before: Snapshot,
    _held: &data::WorldExclusive,
) -> RefactorRecord {
    let worldbook_entries = new_worldbook_entries(root, world_id, &before.worldbook_uids);
    let mechanism = diff_mechanism(before.state.as_ref(), root, world_id);
    let added_ledger_lines = diff_ledger_suffix(&before.mechanism_log_before, root, world_id);
    let interface_shell_created = detect_interface_shell_created(root, world_id, &before);
    let interface_shell_restore = before.interface_shell_before.clone().filter(|original| {
        !matches!(data::read_interface_shell(root, world_id), Ok(Some(now)) if now == *original)
    });
    let refactor_mode = before.state.as_ref().and_then(|state_before| {
        let after = data::read_state(root, world_id).ok()?;
        (after.refactor_mode != state_before.refactor_mode).then(|| ModeUndo {
            before: state_before.refactor_mode.clone(),
        })
    });
    if character_ids.is_empty()
        && worldbook_entries.is_empty()
        && mechanism.is_none()
        && rewritten_entries.is_empty()
        && deleted_entries.is_empty()
        && added_ledger_lines.is_empty()
        && !interface_shell_created
        && interface_shell_restore.is_none()
        && refactor_mode.is_none()
        && stage_restores.is_empty()
    {
        return RefactorRecord::Nothing;
    }
    let written = append_receipt_atomic(
        root,
        world_id,
        ImportReceipt {
            kind: "refactor".to_owned(),
            label: label.to_owned(),
            timestamp: data::local_timestamp().unwrap_or_default(),
            character_id: None,
            character_ids,
            worldbook_entries,
            mechanism,
            world_card_created: None,
            gm_image_created: false,
            opening: None,
            renamed_from: None,
            rewritten_entries,
            deleted_entries,
            deleted_entries_raw,
            visibility_restores: Vec::new(),
            stage_restores,
            added_ledger_lines,
            interface_shell_created,
            interface_shell_restore,
            refactor_mode,
            import_source: None,
        },
    );
    if written {
        RefactorRecord::Recorded
    } else {
        RefactorRecord::Failed
    }
}

/// adoptImportName 改名成功後呼叫：把舊桌名補進最後一筆收據，undo 時桌名才退得回去。
/// 這桌還沒有任何收據（例如那次匯入本身沒留收據）就悄悄放棄——改名已經成功了，
/// 不該因為記帳補不上而報錯。
pub fn record_last_import_rename(
    root: &Path,
    world_id: &str,
    old_name: &str,
    _held: &data::WorldExclusive,
) {
    let mut receipts = read_receipts(root, world_id);
    let Some(last) = receipts.last_mut() else {
        return;
    };
    last.renamed_from = Some(old_name.to_owned());
    let _ = write_receipts(root, world_id, &receipts);
}
