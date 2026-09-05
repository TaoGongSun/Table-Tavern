use super::interface::{normalize_interface_paths, rebuild_state_fields};
use super::types::{RefactorApplyResult, RefactorApplySummary, RefactorOutcome, RefactorSelection};
use crate::data::{
    self, CharacterCard, DataResult, Tier, Visibility, WorldbookEntry,
};
use crate::mechanism;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// 新角色卡色票，跟前端 App.tsx 的 PALETTE 同一組；新卡依桌上目前角色數輪替。
const PALETTE: [&str; 6] = [
    "#e07a5f", "#3d84a8", "#81b29a", "#f2a541", "#9b5de5", "#e56399",
];

/// 套用一份重構產物。落檔規則：
/// - 勾中的人合併成一張卡（emoji 進頭像欄，其餘欄位比照舊有預設）；同時指定為玩家的人設玩家卡
///   （沿用一桌一張限制：桌上已有玩家卡就整批失敗、不寫入，讓玩家看得懂為什麼沒套用）。
/// - 勾中的人名下每條來源條目：只他專屬（沒有別人共用）的整條刪除；跟別人共用的合集條目，
///   只有收尾階段判斷「刪了只剩殘渣」且所有共用這條的人都被勾了才刪，其餘一律原樣保留
///   （要點 7：基準是優先保留而非刪除，判斷不出來或還有人沒勾就不動）。
/// - 沒勾的人：維持現行機制，各自新增一條獨立世界書條目（is_person=true，內容是 solo_entry_md），
///   來源條目不動——即使他的資料原本散在好幾條裡，未升格就不觸碰原始條目。
/// - 勾中的重寫條目直接新增；帶規則／觸發表的機制條目同時併入本地機制。
/// - 來源條目只在所有引用它的產物都已套用時整條刪除，絕不再停用留墓地。
/// - 勾中介面時，狀態樹整份重建為新欄位集；同名頂層鍵保留目前遊玩中的整支節點。
pub fn apply(
    root: &Path,
    world_id: &str,
    outcome: &RefactorOutcome,
    selection: &RefactorSelection,
) -> DataResult<RefactorApplyResult> {
    let mut state = data::read_state(root, world_id)?;
    // 無效索引（沒同時勾選成卡）靜默當作沒指定；桌上已有玩家卡就整批失敗、不寫入任何東西。
    let player_index = selection
        .player_index
        .filter(|index| selection.character_indices.contains(index));
    if player_index.is_some() && state.player_card_id.is_some() {
        return Err(data::invalid_data("這桌已經有玩家卡"));
    }
    let existing_character_count = data::list_characters(root, world_id)?.len();

    // 玩法閘門：mode 必須在來源消耗判定與任何寫入之前解析成單一有效值——characters 產物
    // 即使 selection 勾了介面也整段不套。晚一步解析的話，來源條目會先被記成「已被介面
    // 消耗」而刪除，介面卻沒套，條目憑空消失。非二值常值同讀取端語意：當 None（舊產物
    // 照 interface 行為）。
    let mode = outcome
        .mode
        .as_deref()
        .map(str::trim)
        .filter(|mode| matches!(*mode, "interface" | "characters"));
    let apply_interface = selection.apply_interface && mode != Some("characters");

    // 介面產物 preflight：路徑正規化（雙套鏡像折疊）在任何寫入之前跑，衝突拒套時零落檔
    // ——interface 段在函式中段才跑的話，Err 當下角色卡已落檔，變成沒有收據的半套用。
    let normalized_interface = match &outcome.interface {
        Some(interface) if apply_interface => Some(
            normalize_interface_paths(
                &interface.state_fields,
                interface.shell.as_deref(),
                &interface.rules,
            )
            .map_err(data::invalid_data)?,
        ),
        _ => None,
    };

    // uid → 引用它的角色 index 清單：判斷一條來源條目是「專屬」還是「共用」的依據，
    // 不看選取狀態（選取只決定「刪不刪」，不決定「算不算共用」）。
    let mut uid_owners: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
    for (index, character) in outcome.characters.iter().enumerate() {
        for uid_str in &character.source_uids {
            if let Ok(uid) = uid_str.parse::<u64>() {
                uid_owners.entry(uid).or_default().push(index);
            }
        }
    }

    // 每個來源 uid 的所有產物是否都被套用。角色的共用合集仍額外受
    // deletable_shared_uids 保護；其餘產物只要有一個沒勾，就絕不刪來源。
    let mut source_consumers: BTreeMap<u64, Vec<bool>> = BTreeMap::new();
    let mut deletion_candidates: BTreeSet<u64> = BTreeSet::new();
    let mut add_consumer = |uid_str: &str, applied: bool, candidate: bool| {
        let Ok(uid) = uid_str.parse::<u64>() else {
            return;
        };
        source_consumers.entry(uid).or_default().push(applied);
        if candidate {
            deletion_candidates.insert(uid);
        }
    };
    for (index, character) in outcome.characters.iter().enumerate() {
        let applied = selection.character_indices.contains(&index);
        for uid in &character.source_uids {
            let Ok(parsed_uid) = uid.parse::<u64>() else {
                continue;
            };
            let owners = uid_owners.get(&parsed_uid).map(Vec::as_slice).unwrap_or_default();
            let character_deletable = owners.len() <= 1
                || (outcome.deletable_shared_uids.iter().any(|shared| shared == uid)
                    && owners.iter().all(|owner| selection.character_indices.contains(owner)));
            add_consumer(uid, applied, applied && character_deletable);
        }
    }
    for (index, entry) in outcome.entries.iter().enumerate() {
        let applied = selection.entry_indices.contains(&index);
        for uid in &entry.source_uids {
            add_consumer(uid, applied, applied);
        }
    }
    if let Some(interface) = &outcome.interface {
        for uid in &interface.source_uids {
            add_consumer(uid, apply_interface, apply_interface);
        }
    }
    for (index, mechanism) in outcome.mechanisms.iter().enumerate() {
        let applied = selection.mechanism_indices.contains(&index);
        add_consumer(&mechanism.source_uid, applied, applied);
    }

    let existing_entries = data::read_worldbook(root, world_id)?;
    // 套用前就存在的 uid 集合：來源刪除只准刪這裡面的條目。產物的來源 uid 在這桌不存在時
    // （例如重構卡匯到新桌），剛落地的新條目會拿到同一批小號 uid，不設這道閘會被誤刪，
    // 且誤刪快照進收據後，undo 會把它們當「被消耗的來源」原樣插回，鎖定條目變成孤兒。
    let preexisting_uids: BTreeSet<u64> = existing_entries.iter().map(|entry| entry.uid).collect();
    let mut next_entry_uid = existing_entries
        .iter()
        .map(|entry| entry.uid)
        .max()
        .map(|uid| uid.checked_add(1).ok_or_else(|| data::invalid_data("worldbook uid overflow")))
        .transpose()?
        .unwrap_or(0);
    let mut next_entry_order = existing_entries
        .iter()
        .map(|entry| entry.order)
        .max()
        .map(|order| order.checked_add(1).ok_or_else(|| data::invalid_data("worldbook order overflow")))
        .transpose()?
        .unwrap_or(0);

    let mut character_ids = Vec::new();
    let mut new_entries = 0usize;
    let mut deleted_entries: Vec<WorldbookEntry> = Vec::new();
    let mut player_assigned = false;

    for (index, character) in outcome.characters.iter().enumerate() {
        if !selection.character_indices.contains(&index) {
            // 沒勾：維持現行機制，獨立成一條 is_person 條目；來源條目不動，資料不會憑空消失。
            // uid: u64::MAX 是「一定不會撞到既有條目」的哨兵——upsert 找不到既有 uid 才會
            // 真的新建，實際落檔的 uid 由 upsert_worldbook_entry 內部重新分配（見 data.rs）。
            data::upsert_worldbook_entry(
                root,
                world_id,
                WorldbookEntry {
                    uid: u64::MAX,
                    title: character.name.clone(),
                    keys: Vec::new(),
                    content: character.solo_entry_md.clone(),
                    constant: false,
                    order: 100,
                    disabled: false,
                    visibility: Visibility::Gm,
                    is_person: true,
                    locked: false,
                },
            )?;
            new_entries += 1;
            continue;
        }

        let card = CharacterCard {
            id: data::new_id(),
            name: character.name.clone(),
            color: PALETTE[(existing_character_count + character_ids.len()) % PALETTE.len()]
                .to_owned(),
            avatar: character.emoji.clone(),
            tier: Tier::Balanced,
            show_image: true,
            archived: false,
            gen_prompt: String::new(),
            public_md: character.public_md.clone(),
            private_md: character.private_md.clone(),
        };
        data::write_character(root, world_id, &card)?;
        if player_index == Some(index) {
            state.player_card_id = Some(card.id.clone());
            player_assigned = true;
        }
        character_ids.push(card.id);

    }

    let mut state_dirty = player_assigned;

    let mut ledger_records = Vec::new();
    for &index in &selection.entry_indices {
        let Some(entry) = outcome.entries.get(index) else {
            continue;
        };
        let locked = entry.kind == "mechanism" && (!entry.rules.is_empty() || !entry.triggers.is_empty());
        // carry 型條目帶 meta：keys/constant/order/disabled/visibility/is_person 原樣照抄，
        // order 直接用 meta 的值、不吃 next_entry_order 遞增（那個號碼留給沒有 meta 的真新條目）；
        // 沒帶 meta（AI 重寫／本地合組的新條目）→ 現行預設不變。
        let (keys, constant, order, disabled, visibility, is_person) = match &entry.meta {
            Some(meta) => (
                meta.keys.clone(),
                meta.constant,
                meta.order,
                meta.disabled,
                meta.visibility.clone(),
                meta.is_person,
            ),
            None => (
                Vec::new(),
                false,
                next_entry_order,
                false,
                Visibility::Gm,
                false,
            ),
        };
        data::upsert_worldbook_entry(
            root,
            world_id,
            WorldbookEntry {
                uid: next_entry_uid,
                title: entry.title.clone(),
                keys,
                content: entry.content.clone(),
                constant,
                order,
                disabled,
                visibility,
                is_person,
                locked,
            },
        )?;
        next_entry_uid = next_entry_uid
            .checked_add(1)
            .ok_or_else(|| data::invalid_data("worldbook uid overflow"))?;
        if entry.meta.is_none() {
            next_entry_order = next_entry_order
                .checked_add(1)
                .ok_or_else(|| data::invalid_data("worldbook order overflow"))?;
        }
        new_entries += 1;
        if locked {
            for (path, rule) in &entry.rules {
                state.mechanism.rules.insert(path.clone(), rule.clone());
            }
            state.mechanism.triggers.extend(entry.triggers.iter().cloned());
            state_dirty = true;
            ledger_records.push(absorbed_ledger_record_for_title(&entry.title));
        }
    }

    let mut interface_applied = false;
    if let (Some(interface), Some((state_fields, rules))) =
        (&outcome.interface, &normalized_interface)
    {
        rebuild_state_fields(&mut state.state.tree, &mut state.state.jumps, state_fields);
        state_dirty = true;
        if let Some(shell) = interface.shell.as_deref().filter(|shell| !shell.is_empty()) {
            data::write_interface_shell(root, world_id, shell)?;
            // 接管後畫面上的每個欄位都靠模型回報才會動：開增量協定讓它拿得到更新語法，
            // 併入卡自訂的欄位規則與回報指引，卡的規矩才不會被通則蓋掉。
            state.mechanism.incremental = true;
            for (path, rule) in rules {
                state.mechanism.rules.insert(path.clone(), rule.clone());
            }
            if !interface.guide.trim().is_empty() {
                state.mechanism.guide = interface.guide.trim().to_owned();
            }
        }
        interface_applied = true;
    }

    // 玩法標記持久化（refactor-mode-split）：兩模式都寫進桌面狀態；characters 順手清掉舊
    // interface 套用殘留的殼檔（fallback 抑制第一層；controller 讀 mode 是第二層）。
    // 只收二值列舉（函式開頭解析）：手改匯入檔的 "Characters"／尾空白等非常值不落地。
    if let Some(mode) = mode {
        state.refactor_mode = Some(mode.to_owned());
        state_dirty = true;
        if mode == "characters" {
            if let Ok(path) = data::interface_shell_path(root, world_id) {
                let _ = std::fs::remove_file(path);
            }
        }
    }

    let mut mechanisms_applied = 0usize;
    for &index in &selection.mechanism_indices {
        let Some(mechanism) = outcome.mechanisms.get(index) else {
            continue;
        };
        for (path, rule) in &mechanism.rules {
            state.mechanism.rules.insert(path.clone(), rule.clone());
        }
        state.mechanism.triggers.extend(mechanism.triggers.iter().cloned());
        state_dirty = true;
        if let Some(record) = absorbed_ledger_record(root, world_id, &mechanism.source_uid) {
            ledger_records.push(record);
        }
        mechanisms_applied += 1;
    }

    if state_dirty {
        data::write_state(root, world_id, &state)?;
        // 重建狀態樹沒有經過逐字稿，事件快照還停在套用前的舊欄位——不補上去，
        // 玩家一按收回（或換幕）就把重構剛建好的樹換回去。
        data::sync_scene_state_tree(root, world_id, &state)?;
    }
    if !ledger_records.is_empty() {
        mechanism::append_log(root, world_id, state.current_scene, &ledger_records);
    }
    for (uid, consumers) in source_consumers {
        if preexisting_uids.contains(&uid)
            && deletion_candidates.contains(&uid)
            && consumers.iter().all(|applied| *applied)
        {
            delete_source_entry(root, world_id, uid, &mut deleted_entries)?;
        }
    }

    // 整條淘汰的既有條目停用：dropped 是玩家沒放回的最終清單（放回的已在前端轉成 entries
    // 勾選）。不停用的話 constant 條目照常每輪注入，characters 桌的 GM 仍照卡片介面協定
    // 輸出。只停整條（span 空）：半條的條目其餘段落還在服役。停用前原樣快照走
    // rewritten_entries 進收據（undo 覆寫復原）；條目留在世界書掛停用徽章，玩家看得到全文。
    let deleted_uids: BTreeSet<u64> = deleted_entries.iter().map(|entry| entry.uid).collect();
    let mut rewritten_entries: Vec<WorldbookEntry> = Vec::new();
    let whole_entry_drops: BTreeSet<u64> = outcome
        .dropped
        .iter()
        .filter(|item| item.span.is_empty())
        .filter_map(|item| item.uid.parse().ok())
        .filter(|uid| preexisting_uids.contains(uid) && !deleted_uids.contains(uid))
        .collect();
    if !whole_entry_drops.is_empty() {
        for entry in data::read_worldbook(root, world_id)? {
            if whole_entry_drops.contains(&entry.uid) && !entry.disabled {
                rewritten_entries.push(entry.clone());
                data::upsert_worldbook_entry(
                    root,
                    world_id,
                    WorldbookEntry { disabled: true, ..entry },
                )?;
            }
        }
    }

    // 套用成功後存一份完整產物，供玩家之後直接匯出重玩、不必重燒 AI 額度重新展開同一張卡；
    // undo 與收據不動這個檔（零改動），二次套用直接覆寫。
    data::write_refactor_outcome(root, world_id, &serde_json::to_string_pretty(outcome)?)?;

    Ok(RefactorApplyResult {
        summary: RefactorApplySummary {
            new_characters: character_ids.len(),
            new_entries,
            deleted_entries: deleted_entries.len(),
            rewritten_entries: rewritten_entries.len(),
            interface_applied,
            mechanisms_applied,
            player_assigned,
        },
        character_ids,
        rewritten_entries,
        deleted_entries,
    })
}

/// 被套用產物消耗掉的來源條目：整條刪除，原文記進 `deleted_entries`——匯入路徑的 undo 要
/// 無條件插回（見 receipts::undo_last_import）。條目已經不在就略過。
fn delete_source_entry(
    root: &Path,
    world_id: &str,
    uid: u64,
    deleted_entries: &mut Vec<WorldbookEntry>,
) -> DataResult<()> {
    let Some(entry) = data::read_worldbook(root, world_id)?
        .into_iter()
        .find(|entry| entry.uid == uid)
    else {
        return Ok(());
    };
    data::delete_worldbook_entry(root, world_id, uid)?;
    deleted_entries.push(entry);
    Ok(())
}

/// 機制套用後記一筆已接管：來源條目原本在帳本裡是 Skipped，append_log 落檔後 read_ledger
/// 取「同標題最新一筆」會直接蓋成 Absorbed；原本不在帳本裡的純散文條目則等於新增一筆，
/// 讓玩家在帳本分頁看得到這條被收編了。uid 解不出來或條目已經不在就不記。
fn absorbed_ledger_record(root: &Path, world_id: &str, uid_str: &str) -> Option<mechanism::Record> {
    let uid: u64 = uid_str.parse().ok()?;
    let entry = data::read_worldbook(root, world_id)
        .ok()?
        .into_iter()
        .find(|entry| entry.uid == uid)?;
    Some(absorbed_ledger_record_for_title(&entry.title))
}

fn absorbed_ledger_record_for_title(title: &str) -> mechanism::Record {
    mechanism::Record {
        kind: mechanism::RecordKind::Absorbed,
        path: title.to_owned(),
        detail: "AI 卡重構產生的機制條目：欄位規則／觸發表由 App 本地執行，說明文留在世界書（唯讀）照常可讀。".to_owned(),
    }
}
