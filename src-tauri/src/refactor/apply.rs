use super::card_file::{RefactorApplied, RefactorAppliedCharacter, RefactorCardFile};
use super::card_png::{AssetKind, CardAsset};
use super::interface::{normalize_interface_paths, rebuild_state_fields};
use super::sources::{new_entry_shape, Sources};
use super::types::{
    ApplyFailure, ApplyProgress, RefactorApplyResult, RefactorApplySummary, RefactorOutcome,
    RefactorSelection,
};
use crate::data::{
    self, CharacterCard, DataResult, FieldKind, FieldRule, Tier, Visibility, WorldbookEntry,
};
use crate::import;
use crate::mechanism;
use crate::ui_msg::UiMsg;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// 新角色卡色票，跟前端 App.tsx 的 PALETTE 同一組；新卡依桌上目前角色數輪替。
const PALETTE: [&str; 6] = [
    "#e07a5f", "#3d84a8", "#81b29a", "#f2a541", "#9b5de5", "#e56399",
];

/// 新條目的 uid 哨兵：upsert 找不到這個 uid 就新建、由它分配實際 uid。一律交給 upsert 分配，
/// 不自己預算——同一次套用先建的人物條目會佔掉預算的號碼，後面的新條目撞上就把它覆寫掉
/// （refactor-apply-count-mismatch）。
const NEW_ENTRY_UID: u64 = u64::MAX;

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
/// - 重構卡附的角色圖：其餘套用全部成立、存檔之後，依 outcome_index → 新卡 id 映射逐張寫圖。
///   寫圖逐張進行、一張失敗不停下其餘，也不回 Err（收據照記，撤銷刪角色時連圖一起刪）；
///   失敗的角色名進 summary.images_failed。映射到沒建卡的 index 的圖略過。
pub fn apply_with_assets(
    root: &Path,
    world_id: &str,
    outcome: &RefactorOutcome,
    selection: &RefactorSelection,
    assets: &[CardAsset],
) -> Result<RefactorApplyResult, ApplyFailure> {
    let mut progress = ApplyProgress::default();
    match apply_into(root, world_id, outcome, selection, assets, &mut progress) {
        Ok(summary) => Ok(RefactorApplyResult {
            summary,
            character_ids: progress.character_ids,
            rewritten_entries: progress.rewritten_entries,
            deleted_entries: progress.deleted_entries,
            deleted_entries_raw: progress.deleted_entries_raw,
        }),
        Err(error) => Err(ApplyFailure { error, progress }),
    }
}

fn apply_into(
    root: &Path,
    world_id: &str,
    outcome: &RefactorOutcome,
    selection: &RefactorSelection,
    assets: &[CardAsset],
    progress: &mut ApplyProgress,
) -> DataResult<RefactorApplySummary> {
    let mut state = data::read_state(root, world_id)?;
    // 無效索引（沒同時勾選成卡）靜默當作沒指定；桌上已有玩家卡就整批失敗、不寫入任何東西。
    let player_index = selection
        .player_index
        .filter(|index| selection.character_indices.contains(index));
    if player_index.is_some() && state.player_card_id.is_some() {
        return Err(UiMsg::PlayerCardExists.into_error());
    }
    let existing_characters = data::list_characters(root, world_id)?;
    let existing_character_count = existing_characters.len();

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

    let existing_entries = data::read_worldbook(root, world_id)?;
    // 已核對來源表：產物引用的 uid 一律經它換成這桌實際的條目 uid；核對不過的視同這桌沒有這條來源
    // ——不刪、不停用、不記帳本、不算歸屬（見 refactor/sources.rs）。快照取在任何寫入之前。
    let sources = Sources::build(
        outcome,
        &existing_entries,
        data::read_worldbook_raw(root, world_id)?,
    );

    // uid → 引用它的角色 index 清單：判斷一條來源條目是「專屬」還是「共用」的依據，
    // 不看選取狀態（選取只決定「刪不刪」，不決定「算不算共用」）。
    let mut uid_owners: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
    for (index, character) in outcome.characters.iter().enumerate() {
        for uid_str in &character.source_uids {
            if let Some(uid) = sources.resolve(uid_str) {
                uid_owners.entry(uid).or_default().push(index);
            }
        }
    }
    let deletable_shared: BTreeSet<u64> = outcome
        .deletable_shared_uids
        .iter()
        .filter_map(|uid| sources.resolve(uid))
        .collect();

    // 每個來源 uid 的所有產物是否都被套用。角色的共用合集仍額外受
    // deletable_shared_uids 保護；其餘產物只要有一個沒勾，就絕不刪來源。
    let mut source_consumers: BTreeMap<u64, Vec<bool>> = BTreeMap::new();
    let mut deletion_candidates: BTreeSet<u64> = BTreeSet::new();
    let mut add_consumer = |uid_str: &str, applied: bool, candidate: bool| {
        let Some(uid) = sources.resolve(uid_str) else {
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
            let Some(actual_uid) = sources.resolve(uid) else {
                continue;
            };
            let owners = uid_owners
                .get(&actual_uid)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let character_deletable = owners.len() <= 1
                || (deletable_shared.contains(&actual_uid)
                    && owners
                        .iter()
                        .all(|owner| selection.character_indices.contains(owner)));
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

    // 套用前就存在的 uid 集合：來源刪除只准刪這裡面的條目。產物的來源 uid 在這桌不存在時
    // （例如重構卡匯到新桌），剛落地的新條目會拿到同一批小號 uid，不設這道閘會被誤刪，
    // 且誤刪快照進收據後，undo 會把它們當「被消耗的來源」原樣插回，鎖定條目變成孤兒。
    let preexisting_uids: BTreeSet<u64> = existing_entries.iter().map(|entry| entry.uid).collect();
    // 新條目的 uid 由 upsert 分配 max+1；上限已用掉就在任何寫入前整批拒絕，
    // 也避免 NEW_ENTRY_UID 哨兵撞上既有那條、把它覆寫掉。
    if preexisting_uids.last() == Some(&NEW_ENTRY_UID) {
        return Err(data::invalid_data("worldbook uid overflow"));
    }
    let mut next_entry_order = existing_entries
        .iter()
        .map(|entry| entry.order)
        .max()
        .map(|order| {
            order
                .checked_add(1)
                .ok_or_else(|| data::invalid_data("worldbook order overflow"))
        })
        .transpose()?
        .unwrap_or(0);

    // 這次會寫的重構殼：下方寫殼分支用同一個值，純空白殼不會先刪舊殼又寫進一份空白。
    let new_shell = match (&outcome.interface, &normalized_interface) {
        (Some(interface), Some(_)) => interface
            .shell
            .as_deref()
            .filter(|shell| !shell.trim().is_empty()),
        _ => None,
    };
    // 帶 mode 的產物套用後沒有新殼（characters、介面沒勾、判定不接管）→ 桌上的舊殼一律清掉：
    // 前端 interface 桌沒殼就不給面板，留著上一輪的殼會拿對不上新狀態樹的骨架繼續畫
    // 〔作者裁決 2026-10-02〕。放在全部 preflight 之後、第一筆寫入之前：刪不掉就整批失敗、
    // 桌上零改動，不回報套用成功。沒有 mode 的產物照舊不碰殼檔。
    if mode.is_some() && new_shell.is_none() {
        data::commit_world_remove(&data::interface_shell_path(root, world_id)?)?;
    }

    // outcome_index → 這次建的卡 id（沒勾成卡是 None）：落檔的套用映射，不靠 character_ids 順序
    let mut character_map: Vec<Option<String>> = vec![None; outcome.characters.len()];
    let mut new_entries = 0usize;
    let mut player_assigned = false;

    for (index, character) in outcome.characters.iter().enumerate() {
        if !selection.character_indices.contains(&index) {
            // 沒勾：維持現行機制，獨立成一條 is_person 條目；來源條目不動，資料不會憑空消失。
            // uid 用 NEW_ENTRY_UID 哨兵，實際落檔的 uid 由 upsert_worldbook_entry 分配。
            data::upsert_worldbook_entry(
                root,
                world_id,
                WorldbookEntry {
                    uid: NEW_ENTRY_UID,
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
            color: PALETTE
                [(existing_character_count + progress.character_ids.len()) % PALETTE.len()]
            .to_owned(),
            avatar: character.emoji.clone(),
            tier: Tier::Balanced,
            show_image: true,
            archived: false,
            gen_prompt: String::new(),
            public_md: character.public_md.clone(),
            private_md: character.private_md.clone(),
        };
        // 先登記再寫：寫到一半失敗時收據也涵蓋這張，撤銷能把半截檔清掉
        progress.character_ids.push(card.id.clone());
        data::write_character(root, world_id, &card)?;
        if player_index == Some(index) {
            state.player_card_id = Some(card.id.clone());
            player_assigned = true;
        }
        character_map[index] = Some(card.id.clone());
    }

    let mut state_dirty = player_assigned;

    let mut ledger_records = Vec::new();
    for &index in &selection.entry_indices {
        let Some(entry) = outcome.entries.get(index) else {
            continue;
        };
        let locked =
            entry.kind == "mechanism" && (!entry.rules.is_empty() || !entry.triggers.is_empty());
        let value = new_entry_value(
            entry,
            locked,
            next_entry_order,
            &sources,
            &existing_characters,
        );
        data::insert_worldbook_entry_raw(root, world_id, value)?;
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
            state
                .mechanism
                .triggers
                .extend(entry.triggers.iter().cloned());
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
        if let Some(shell) = new_shell {
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
            state.mechanism.value_types = value_types_of(state_fields, rules);
        }
        interface_applied = true;
    }

    // 玩法標記持久化（refactor-mode-split）：兩模式都寫進桌面狀態；舊殼已在 preflight 後清掉。
    // 只收二值列舉（函式開頭解析）：手改匯入檔的 "Characters"／尾空白等非常值不落地。
    if let Some(mode) = mode {
        state.refactor_mode = Some(mode.to_owned());
        state_dirty = true;
    }

    let mut mechanisms_applied = 0usize;
    for &index in &selection.mechanism_indices {
        let Some(mechanism) = outcome.mechanisms.get(index) else {
            continue;
        };
        for (path, rule) in &mechanism.rules {
            state.mechanism.rules.insert(path.clone(), rule.clone());
        }
        state
            .mechanism
            .triggers
            .extend(mechanism.triggers.iter().cloned());
        state_dirty = true;
        if let Some(source) = sources.entry(&mechanism.source_uid) {
            ledger_records.push(absorbed_ledger_record_for_title(&source.title));
        }
        mechanisms_applied += 1;
    }

    if state_dirty {
        // 卡片變數模式交接（events → tree，計畫 8.1）：先把有效值投影寫進狀態樹、控制檔切回 tree，
        // 再寫重建後的樹；`state` 一開頭就是經投影入口讀的，重建建立在有效值上。
        data::state_commit::with_commit(root, world_id, data::message_vars::handover_to_tree)?;
        data::write_state(root, world_id, &state)?;
        // 重建狀態樹沒有經過逐字稿，事件快照還停在套用前的舊欄位——不補上去，
        // 玩家一按收回（或換幕）就把重構剛建好的樹換回去。
        data::sync_scene_state_tree(root, world_id, &state)?;
    }
    if !ledger_records.is_empty() {
        mechanism::append_log(root, world_id, state.current_scene, &ledger_records);
    }
    let preserved: BTreeSet<u64> = outcome
        .preserve_source_uids
        .iter()
        .filter_map(|uid| sources.resolve(uid))
        .collect();
    for (uid, consumers) in source_consumers {
        if preexisting_uids.contains(&uid)
            && !preserved.contains(&uid)
            && deletion_candidates.contains(&uid)
            && consumers.iter().all(|applied| *applied)
        {
            delete_source_entry(root, world_id, uid, progress)?;
        }
    }
    #[cfg(test)]
    if fail_point::fire() {
        return Err(data::invalid_data("injected failure after source deletion"));
    }

    // 整條淘汰的既有條目停用：dropped 是玩家沒放回的最終清單（放回的已在前端轉成 entries
    // 勾選）。不停用的話 constant 條目照常每輪注入，characters 桌的 GM 仍照卡片介面協定
    // 輸出。只停整條（span 空）：半條的條目其餘段落還在服役。停用前原樣快照走
    // rewritten_entries 進收據（undo 覆寫復原）；條目留在世界書掛停用徽章，玩家看得到全文。
    let deleted_uids: BTreeSet<u64> = progress
        .deleted_entries
        .iter()
        .map(|entry| entry.uid)
        .collect();
    let whole_entry_drops: BTreeSet<u64> = outcome
        .dropped
        .iter()
        .filter(|item| item.span.is_empty())
        .filter_map(|item| sources.resolve(&item.uid))
        .filter(|uid| {
            preexisting_uids.contains(uid)
                && !deleted_uids.contains(uid)
                && !preserved.contains(uid)
        })
        .collect();
    if !whole_entry_drops.is_empty() {
        for entry in data::read_worldbook(root, world_id)? {
            if whole_entry_drops.contains(&entry.uid) && !entry.disabled {
                progress.rewritten_entries.push(entry.clone());
                data::upsert_worldbook_entry(
                    root,
                    world_id,
                    WorldbookEntry {
                        disabled: true,
                        ..entry
                    },
                )?;
            }
        }
    }

    // 套用成功後存一份完整產物（封套＋套用映射），供玩家之後直接匯出重玩、不必重燒 AI 額度
    // 重新展開同一張卡，含角色圖匯出也靠映射配回改過名的角色；undo 與收據不動這個檔，
    // 二次套用整份覆寫。
    let applied = RefactorApplied {
        characters: character_map
            .iter()
            .enumerate()
            .map(|(outcome_index, character_id)| RefactorAppliedCharacter {
                outcome_index,
                character_id: character_id.clone(),
            })
            .collect(),
        player_index: player_index.filter(|_| player_assigned),
        player_card_id: state.player_card_id.clone().filter(|_| player_assigned),
    };
    // 存檔失敗不回 Err：角色、條目、來源刪除到這裡都已成立，回 Err 會讓呼叫端跳過收據、
    // 玩家撤銷不了。照常回 Ok（收據照記），summary 標 card_save_failed 讓前端明說存檔沒寫成；
    // 原子寫入保證舊存檔原樣留著。
    let saved = RefactorCardFile::new(outcome.clone(), Some(applied));
    let card_save_failed = serde_json::to_string_pretty(&saved)
        .map_err(Into::into)
        .and_then(|json| data::write_refactor_outcome(root, world_id, &json))
        .inspect_err(|error| log::warn!("refactor card save failed: {error}"))
        .is_err();

    let mut images_applied = 0usize;
    let mut images_failed: Vec<String> = Vec::new();
    for asset in assets {
        let Some(Some(character_id)) = character_map.get(asset.outcome_index) else {
            continue;
        };
        let written = match asset.kind {
            AssetKind::Portrait => {
                import::save_character_image(root, world_id, character_id, &asset.bytes)
            }
            AssetKind::Avatar => {
                import::save_character_avatar(root, world_id, character_id, &asset.bytes)
            }
        };
        match written {
            Ok(()) => images_applied += 1,
            Err(error) => {
                log::warn!("refactor card image write failed: {error}");
                let name = &outcome.characters[asset.outcome_index].name;
                if !images_failed.contains(name) {
                    images_failed.push(name.clone());
                }
            }
        }
    }

    Ok(RefactorApplySummary {
        new_characters: progress.character_ids.len(),
        new_entries,
        deleted_entries: progress.deleted_entries.len(),
        rewritten_entries: progress.rewritten_entries.len(),
        interface_applied,
        mechanisms_applied,
        player_assigned,
        card_save_failed,
        images_applied,
        images_failed,
    })
}

/// 不附角色圖的套用（測試用簡寫）。
#[cfg(test)]
pub fn apply(
    root: &Path,
    world_id: &str,
    outcome: &RefactorOutcome,
    selection: &RefactorSelection,
) -> DataResult<RefactorApplyResult> {
    apply_with_assets(root, world_id, outcome, selection, &[]).map_err(|failure| failure.error)
}

/// 骨架填值用的欄位型別表（點分路徑→"number"｜"bool"｜"list"）。狀態樹會把值轉成字串，型別只能在套用時
/// 從產物記下來：number／bool 取自模型產出的 STATE 初始 JSON 葉子型別；list 取自同一份產物的欄位規則
/// `kind: list`——只有這種欄位在行內集合 `[{{x}}]` 裡當「集合片段」拆成多個元素（見 refactor-shell.ts）。
fn value_types_of(
    state_fields: &serde_json::Value,
    rules: &BTreeMap<String, FieldRule>,
) -> BTreeMap<String, String> {
    fn walk(prefix: &str, value: &serde_json::Value, out: &mut BTreeMap<String, String>) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, child) in map {
                    let path = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    walk(&path, child, out);
                }
            }
            serde_json::Value::Number(_) => {
                out.insert(prefix.to_owned(), "number".to_owned());
            }
            serde_json::Value::Bool(_) => {
                out.insert(prefix.to_owned(), "bool".to_owned());
            }
            _ => {}
        }
    }
    let mut out = BTreeMap::new();
    walk("", state_fields, &mut out);
    for (path, rule) in rules {
        if rule.kind == FieldKind::List {
            out.insert(path.clone(), "list".to_owned());
        }
    }
    out
}

/// 被套用產物消耗掉的來源條目：整條刪除，原文（精簡與原始 JSON）記進 progress——匯入路徑的 undo 要
/// 無條件照原始值插回（見 receipts::undo_last_import）。條目已經不在就略過。
fn delete_source_entry(
    root: &Path,
    world_id: &str,
    uid: u64,
    progress: &mut ApplyProgress,
) -> DataResult<()> {
    let Some(entry) = data::read_worldbook(root, world_id)?
        .into_iter()
        .find(|entry| entry.uid == uid)
    else {
        return Ok(());
    };
    let raw = data::read_worldbook_raw(root, world_id)?.remove(&uid);
    data::delete_worldbook_entry(root, world_id, uid)?;
    progress.deleted_entries.push(entry.clone());
    let mut raw = raw.unwrap_or_else(|| data::worldbook_entry_value(&entry));
    if let Some(object) = raw.as_object_mut() {
        object.insert("uid".to_owned(), serde_json::json!(uid));
    }
    progress.deleted_entries_raw.push(raw);
    Ok(())
}

/// 重構新條目的原始值：
/// - 照搬（carry，帶 meta）：來源核對成功就整條複製來源原始值（次要鍵、位置、可見度、來源卡全帶），
///   只換內文與套用端判定的旗標；可見度取來源目前的值（跨桌反查對到別桌的同卡條目時才是那桌的角色）。
///   核對失敗退回用 meta 建，名單裡的角色都不在本桌就給 GM。
/// - 機制條目（locked）：不套觸發政策、不沿用可見度，維持無主鍵、非常駐、GM。
/// - 其餘沒帶 meta 的 setting 條目：觸發、可見度、來源卡照核對成功的來源推（sources::new_entry_shape）。
fn new_entry_value(
    entry: &crate::refactor_ai::RefactorNewEntry,
    locked: bool,
    next_order: i64,
    sources: &Sources,
    characters: &[data::CharacterMeta],
) -> serde_json::Value {
    // 機制條目優先於 meta：重寫路徑會產出帶 meta 的機制條目，也一律 GM、非常駐、無主鍵
    if locked {
        return data::worldbook_entry_value(&WorldbookEntry {
            uid: 0,
            title: entry.title.clone(),
            keys: Vec::new(),
            content: entry.content.clone(),
            constant: false,
            order: entry.meta.as_ref().map_or(next_order, |meta| meta.order),
            disabled: false,
            visibility: Visibility::Gm,
            is_person: false,
            locked: true,
        });
    }
    if let Some(meta) = &entry.meta {
        let source = (entry.source_uids.len() == 1)
            .then(|| sources.raw(&entry.source_uids[0]))
            .flatten();
        if let Some(raw) = source {
            let mut value = raw.clone();
            if let Some(object) = value.as_object_mut() {
                object.insert(
                    "content".to_owned(),
                    serde_json::Value::String(entry.content.clone()),
                );
                object.insert(
                    "comment".to_owned(),
                    serde_json::Value::String(entry.title.clone()),
                );
            }
            data::set_entry_flags(&mut value, false, meta.is_person);
            return value;
        }
        let visibility = match &meta.visibility {
            Visibility::Characters(ids)
                if !ids
                    .iter()
                    .any(|id| characters.iter().any(|meta| &meta.id == id)) =>
            {
                Visibility::Gm
            }
            visibility => visibility.clone(),
        };
        return data::worldbook_entry_value(&WorldbookEntry {
            uid: 0,
            title: entry.title.clone(),
            keys: meta.keys.clone(),
            content: entry.content.clone(),
            constant: meta.constant,
            order: meta.order,
            disabled: meta.disabled,
            visibility,
            is_person: meta.is_person,
            locked: false,
        });
    }
    let resolved: Vec<(&WorldbookEntry, &serde_json::Value)> = entry
        .source_uids
        .iter()
        .filter_map(|uid| Some((sources.entry(uid)?, sources.raw(uid)?)))
        .collect();
    let shape = new_entry_shape(&resolved);
    let mut value = data::worldbook_entry_value(&WorldbookEntry {
        uid: 0,
        title: entry.title.clone(),
        keys: shape.keys,
        content: entry.content.clone(),
        constant: shape.constant,
        order: next_order,
        disabled: shape.disabled,
        visibility: shape.visibility,
        is_person: false,
        locked: false,
    });
    data::set_source_cards(&mut value, &shape.source_cards);
    value
}

fn absorbed_ledger_record_for_title(title: &str) -> mechanism::Record {
    mechanism::Record {
        kind: mechanism::RecordKind::Absorbed,
        path: title.to_owned(),
        detail: UiMsg::LedgerRefactorMechanism.to_string(),
    }
}

/// 測試用注入點：刪完來源之後、下一次寫入之前失敗一次（不寫壞任何檔），驗自動回滾那條 Err 分支。
#[cfg(test)]
pub(super) mod fail_point {
    use std::cell::Cell;

    thread_local! {
        static ARMED: Cell<bool> = const { Cell::new(false) };
    }

    pub(in crate::refactor) struct Armed;

    pub(in crate::refactor) fn arm() -> Armed {
        ARMED.with(|cell| cell.set(true));
        Armed
    }

    impl Drop for Armed {
        fn drop(&mut self) {
            ARMED.with(|cell| cell.set(false));
        }
    }

    pub(super) fn fire() -> bool {
        ARMED.with(|cell| cell.replace(false))
    }
}
