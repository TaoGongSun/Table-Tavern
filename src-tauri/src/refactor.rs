//! AI 卡重構套用：AI 讀整張匯入卡，把內容拆成角色／介面／機制三類產物（RefactorOutcome），
//! 玩家人審勾選（RefactorSelection）後套用落檔，可一鍵倒退。AI 呼叫是下一包的事，這裡只管
//! 「已經有一份 RefactorOutcome，怎麼套用、怎麼復原」——手寫 JSON 餵進 apply() 就能驗證整條路。

use crate::data::{
    self, CharacterCard, DataResult, FieldRule, StateNode, Tier, Trigger, Visibility,
    WorldbookEntry,
};
use crate::mechanism;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// 新角色卡色票，跟前端 App.tsx 的 PALETTE 同一組；新卡依桌上目前角色數輪替。
const PALETTE: [&str; 6] = [
    "#e07a5f", "#3d84a8", "#81b29a", "#f2a541", "#9b5de5", "#e56399",
];

/// 認人後的一位角色候選：資料可能併自好幾條世界書條目（人物合併，person-promote）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorCharacter {
    pub name: String,
    pub emoji: String,
    pub public_md: String,
    pub private_md: String,
    /// 這位角色的資料來源條目 uid 清單；只有單一專屬來源時長度為 1。
    pub source_uids: Vec<String>,
    /// 此人不升格為角色卡時，自己獨立世界書條目的全文。
    pub solo_entry_md: String,
    /// 盤點階段 AI 標記的疑似玩家本人；整份 RefactorOutcome.characters 至多一筆為 true。
    #[serde(default)]
    pub suspected_player: bool,
}

/// 散文介面指令抽成的狀態樹候選。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorInterface {
    pub state_fields: serde_json::Value,
    pub source_uids: Vec<String>,
    /// 解析失敗退原文的雙軌保底。
    pub raw: String,
    /// AI 順便產的完整 HTML 渲染殼（自包含單檔，佔位符待前端替換）；None＝沒產出或抽不出來，
    /// 不影響 state_fields——渲染殼是錦上添花，不是介面套用成不成立的條件。
    #[serde(default)]
    pub shell: Option<String>,
    /// 這張卡自己的欄位規則（點分路徑→規則）：數值欄要 delta、清單欄整份 replace 都靠它。
    /// 只有接管（interface_shell）變體會產，空的就照現值形狀推定。
    #[serde(default)]
    pub rules: BTreeMap<String, FieldRule>,
    /// 這張卡自己的回報指引：每回合必報哪些欄位、哪些變動才報，照卡原文的規定寫。
    /// 卡與卡的規矩差很多（有的每回合全量重印道具，有的只在變動時提），不能用一套通則蓋過去。
    #[serde(default)]
    pub guide: String,
}

/// 欄位規則＋觸發表候選；rules／triggers 直接複用 data.rs 既有機制型別，不新造平行型別。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorMechanism {
    pub source_uid: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rules: BTreeMap<String, FieldRule>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggers: Vec<Trigger>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorOutcome {
    #[serde(default)]
    pub characters: Vec<RefactorCharacter>,
    #[serde(default)]
    pub interface: Option<RefactorInterface>,
    #[serde(default)]
    pub mechanisms: Vec<RefactorMechanism>,
    #[serde(default)]
    pub entries: Vec<crate::refactor_ai::RefactorNewEntry>,
    /// 收尾階段判定「刪了只剩殘渣」的共用合集條目 uid；套用時還要所有共用這條的人都被勾選
    /// 才會真的刪（要點 7：基準是優先保留而非刪除）。
    #[serde(default)]
    pub deletable_shared_uids: Vec<String>,
    /// 本地零呼叫組裝（refactor_assemble::assemble_local）淘汰的整條／半條內容：預設不套用，
    /// 純粹隨產物保留供玩家展開查看、一鍵放回。apply() 不處理這三欄——落檔與否是前端 UI 的事。
    #[serde(default)]
    pub dropped: Vec<crate::refactor_assemble::RefactorDroppedEntry>,
    /// app 尚無執行機構、原文已照搬進 GM 規則條目的機制清單（資訊性，內容不會遺失）。
    #[serde(default)]
    pub unabsorbed: Vec<crate::refactor_assemble::RefactorUnabsorbedItem>,
    /// 機械稽核紅字：涵蓋漏網／機制守恆／拆組守恆／淘汰稽核，四類之一。
    #[serde(default)]
    pub audit: Vec<crate::refactor_assemble::RefactorAuditItem>,
    /// 產出時玩家選定的玩法："interface"｜"characters"；None＝舊產物，照 interface 行為。
    /// 套用時寫進 WorldState.refactor_mode；characters 並停用卡片介面 fallback。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorSelection {
    #[serde(default)]
    pub character_indices: Vec<usize>,
    #[serde(default)]
    pub apply_interface: bool,
    #[serde(default)]
    pub mechanism_indices: Vec<usize>,
    #[serde(default)]
    pub entry_indices: Vec<usize>,
    /// characters 裡要設成玩家卡的那一位；None＝不指定。不在 character_indices 裡的索引視同
    /// None（沒同時勾選成卡就不可能是玩家卡）。
    #[serde(default)]
    pub player_index: Option<usize>,
}

/// 套用摘要，前端顯示用。
#[derive(Debug, Clone, Default, Serialize)]
pub struct RefactorApplySummary {
    pub new_characters: usize,
    pub new_entries: usize,
    /// 合併升格後整條刪除的來源世界書條目數（專屬條目＋收尾判定可刪的共用合集條目）。
    pub deleted_entries: usize,
    pub rewritten_entries: usize,
    pub interface_applied: bool,
    pub mechanisms_applied: usize,
    pub player_assigned: bool,
}

/// apply() 的完整結果：summary 給前端，其餘給呼叫端組收據（receipts::record_refactor_apply）。
#[derive(Debug)]
pub struct RefactorApplyResult {
    pub summary: RefactorApplySummary,
    pub character_ids: Vec<String>,
    pub rewritten_entries: Vec<WorldbookEntry>,
    /// 整條刪除的來源條目原文快照；undo 時不論 uid 現在還在不在，一律無條件插回。
    pub deleted_entries: Vec<WorldbookEntry>,
}

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

/// 介面產物的路徑正規化：模型會把同一份狀態欄輸出成兩套重複結構（頂層一套＋「状态栏」
/// 之類的頂層分支再鏡像一套），殼佔位符只綁其中一側，GM 執行期挑另一側寫，殼綁那側
/// 永遠空字串、面板死在初始文字。只認精確別名 `W.p ↔ p`，不採相似度：
/// - 有殼：佔位符引用哪側，哪側是正典；兩側都被引用＝產物自相矛盾，Err 拒套不猜。
/// - 無殼（或殼沒引用任何別名對）：分支下每葉在根層都有精確對應（完整鏡像）才折疊，
///   正典取根層短路徑；不是完整鏡像就整份不動。
/// 至少兩對別名才算鏡像分支（單葉同名視為巧合）。值合併：別名側空＝取正典；正典空＝搬
/// 別名值；兩側非空且不同＝Err。別名分支多出的葉改掛正典側路徑；rules 跟著 remap（同
/// key 規則不同＝Err），最後剔除不在正規化後葉集合的懸空規則。呼叫端必須在 apply 寫入
/// 任何檔之前跑（preflight），Err 才能保證零落檔。
fn normalize_interface_paths(
    state_fields: &serde_json::Value,
    shell: Option<&str>,
    rules: &BTreeMap<String, FieldRule>,
) -> Result<(serde_json::Value, BTreeMap<String, FieldRule>), String> {
    let Some(root_object) = state_fields.as_object() else {
        return Ok((state_fields.clone(), rules.clone()));
    };
    let mut leaves = BTreeMap::new();
    flatten_leaves("", state_fields, &mut leaves);
    let placeholders = shell.map(shell_placeholders).unwrap_or_default();

    let mut merged = leaves.clone();
    // 別名葉路徑 → 正典葉路徑；rules remap 靠它。
    let mut alias_of: BTreeMap<String, String> = BTreeMap::new();

    for (branch, value) in root_object {
        if !value.is_object() {
            continue;
        }
        let prefix = format!("{branch}.");
        let branch_leaves: Vec<&String> =
            leaves.keys().filter(|path| path.starts_with(&prefix)).collect();
        // 別名對：剝掉分支前綴後，樹上其他位置有一模一樣的葉路徑。
        let pairs: Vec<(String, String)> = branch_leaves
            .iter()
            .filter_map(|nested| {
                let stripped = &nested[prefix.len()..];
                (!stripped.starts_with(&prefix) && leaves.contains_key(stripped))
                    .then(|| ((*nested).clone(), stripped.to_owned()))
            })
            .collect();
        if pairs.len() < 2 {
            continue;
        }
        let nested_referenced = pairs.iter().any(|(nested, _)| placeholders.contains(nested));
        let root_referenced = pairs.iter().any(|(_, root)| placeholders.contains(root));
        let canon_is_root = match (nested_referenced, root_referenced) {
            (true, true) => {
                return Err(format!(
                    "介面產物自相矛盾：渲染殼同時綁定「{branch}.…」與頂層兩套路徑，請重新執行重構"
                ));
            }
            (true, false) => false,
            (false, true) => true,
            // 殼沒表態：完整鏡像（分支每葉都有對應）才折疊，正典取根層短路徑。
            (false, false) => {
                if pairs.len() != branch_leaves.len() {
                    continue;
                }
                true
            }
        };
        for (nested, root_path) in &pairs {
            let (canon, alias) = if canon_is_root { (root_path, nested) } else { (nested, root_path) };
            let canon_value = merged.get(canon).cloned().unwrap_or(serde_json::Value::Null);
            let alias_value = merged.get(alias).cloned().unwrap_or(serde_json::Value::Null);
            if !is_empty_value(&alias_value) {
                if is_empty_value(&canon_value) {
                    merged.insert(canon.clone(), alias_value);
                } else if canon_value != alias_value {
                    return Err(format!(
                        "介面產物同一欄位兩套初始值不一致：「{alias}」＝{alias_value}、「{canon}」＝{canon_value}，請重新執行重構"
                    ));
                }
            }
            merged.remove(alias);
            alias_of.insert(alias.clone(), canon.clone());
        }
        // 正典在根層時，鏡像分支多出的葉（根層沒有對應的）改掛根層路徑，值不丟。
        if canon_is_root {
            for nested in &branch_leaves {
                let stripped = nested[prefix.len()..].to_owned();
                if let Some(value) = merged.remove(*nested) {
                    if let Some(existing) = merged.get(&stripped) {
                        if !is_empty_value(existing) && !is_empty_value(&value) && *existing != value {
                            return Err(format!(
                                "介面產物同一欄位兩套初始值不一致：「{nested}」＝{value}、「{stripped}」＝{existing}，請重新執行重構"
                            ));
                        }
                        if is_empty_value(existing) && !is_empty_value(&value) {
                            merged.insert(stripped.clone(), value);
                        }
                    } else {
                        merged.insert(stripped.clone(), value);
                    }
                    alias_of.insert((*nested).clone(), stripped);
                }
            }
        }
    }

    let mut normalized_rules: BTreeMap<String, FieldRule> = BTreeMap::new();
    for (path, rule) in rules {
        let target = alias_of.get(path).cloned().unwrap_or_else(|| path.clone());
        if let Some(existing) = normalized_rules.get(&target) {
            if existing != rule {
                return Err(format!(
                    "介面產物同一欄位兩套規則不一致：「{path}」與「{target}」，請重新執行重構"
                ));
            }
            continue;
        }
        normalized_rules.insert(target, rule.clone());
    }
    normalized_rules.retain(|path, _| merged.contains_key(path));

    Ok((unflatten(&merged)?, normalized_rules))
}

/// 深度優先攤平：葉＝任何非物件值（字串／數字／陣列都原樣保留），路徑點分。空物件沒有葉，
/// 攤平後自然消失——空分支對狀態樹沒有意義。
fn flatten_leaves(
    prefix: &str,
    value: &serde_json::Value,
    out: &mut BTreeMap<String, serde_json::Value>,
) {
    match value.as_object() {
        Some(object) => {
            for (key, child) in object {
                let path =
                    if prefix.is_empty() { key.clone() } else { format!("{prefix}.{key}") };
                flatten_leaves(&path, child, out);
            }
        }
        None => {
            out.insert(prefix.to_owned(), value.clone());
        }
    }
}

/// 葉路徑集合還原成巢狀物件。路徑互為前綴（「地點」既是葉又是「地點.x」的分支）＝結構
/// 矛盾，Err——正常攤平不會產生，只有折疊搬移撞到才會。
fn unflatten(leaves: &BTreeMap<String, serde_json::Value>) -> Result<serde_json::Value, String> {
    let mut root = serde_json::Map::new();
    for (path, value) in leaves {
        let mut node = &mut root;
        let mut segments = path.split('.').peekable();
        while let Some(segment) = segments.next() {
            if segments.peek().is_none() {
                if node.get(segment).is_some_and(serde_json::Value::is_object) {
                    return Err(format!("介面產物欄位路徑互相衝突：「{path}」，請重新執行重構"));
                }
                node.insert(segment.to_owned(), value.clone());
            } else {
                let child = node
                    .entry(segment.to_owned())
                    .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
                match child.as_object_mut() {
                    Some(_) => {
                        node = child.as_object_mut().unwrap();
                    }
                    None => {
                        return Err(format!(
                            "介面產物欄位路徑互相衝突：「{path}」，請重新執行重構"
                        ));
                    }
                }
            }
        }
    }
    Ok(serde_json::Value::Object(root))
}

fn is_empty_value(value: &serde_json::Value) -> bool {
    value.is_null() || value.as_str().is_some_and(|text| text.trim().is_empty())
}

/// 殼裡所有 `{{路徑}}` 佔位符（trim 過）。
fn shell_placeholders(shell: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = shell;
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else { break };
        out.insert(after[..end].trim().to_owned());
        rest = &after[end + 2..];
    }
    out
}

/// state_fields 是物件時整份重建狀態樹：同名頂層鍵保留目前值，其餘舊鍵一律捨棄；非物件產物
/// 則不動任何狀態，避免壞產物清空整桌。新欄位的 JSON 轉換集中在 json_to_state_node。
fn rebuild_state_fields(tree: &mut BTreeMap<String, StateNode>, jumps: &mut BTreeMap<String, String>, state_fields: &serde_json::Value) {
    let Some(object) = state_fields.as_object() else {
        return;
    };
    let rebuilt = object.iter().map(|(key, value)| (
        key.clone(),
        tree.get(key).cloned().unwrap_or_else(|| json_to_state_node(value)),
    )).collect();
    *tree = rebuilt;
    jumps.retain(|path, _| tree.contains_key(path.split('.').next().unwrap_or_default()));
}

fn json_to_state_node(value: &serde_json::Value) -> StateNode {
    match value {
        serde_json::Value::Object(object) => StateNode::Branch(
            object
                .iter()
                .map(|(key, value)| (key.clone(), json_to_state_node(value)))
                .collect(),
        ),
        serde_json::Value::Array(items) => StateNode::Branch(
            items
                .iter()
                .enumerate()
                .map(|(index, item)| (index.to_string(), json_to_state_node(item)))
                .collect(),
        ),
        serde_json::Value::String(text) => StateNode::Leaf(text.clone()),
        serde_json::Value::Number(number) => StateNode::Leaf(number.to_string()),
        serde_json::Value::Bool(flag) => StateNode::Leaf(flag.to_string()),
        serde_json::Value::Null => StateNode::Leaf(String::new()),
    }
}

/// 讀取端玩法標記正規化：舊版可能已落地 "Characters"／帶空白的值——合法值就地修正大小寫
/// 與空白，真未知值回 Err 讓 controller 維持未知（fail-closed 不 fallback），不回 None
/// 冒充「確定沒標記」。
pub fn normalize_stored_mode(stored: Option<String>) -> Result<Option<String>, String> {
    let Some(raw) = stored else {
        return Ok(None);
    };
    let mode = raw.trim().to_ascii_lowercase();
    if mode == "interface" || mode == "characters" {
        Ok(Some(mode))
    } else {
        Err("refactor-mode-invalid".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{FieldKind, InjectLevel, UpdateMode};
    use crate::receipts;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(label: &str) -> Self {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "table-tavern-refactor-{label}-{}-{id}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn seed_entry(root: &Path, world_id: &str, title: &str, content: &str) -> u64 {
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

    fn character(name: &str, source_uids: &[u64]) -> RefactorCharacter {
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

    fn no_player_selection(character_indices: Vec<usize>) -> RefactorSelection {
        RefactorSelection {
            character_indices,
            apply_interface: false,
            mechanism_indices: Vec::new(),
            entry_indices: Vec::new(),
            player_index: None,
        }
    }

    /// 比照 receipts.rs 既有測試的作法：套用前先 snapshot，套用後記收據，undo 走 receipts 那條路。
    fn apply_recorded(
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

    /// (a) 合併升格＋玩家指定：兩條專屬來源併成一張卡、指定為玩家 → 兩條來源條目整條刪除、
    /// 玩家卡指定寫進 state → undo → 角色卡、來源條目（原樣回來，不是新造的 is_person 條目）、
    /// 玩家卡指定都回原樣。
    #[test]
    fn apply_merges_multi_source_person_deletes_exclusive_entries_and_sets_player_then_undo_restores() {
        let root = TestRoot::new("merge-player");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let bio_uid = seed_entry(root.path(), &world_id, "亞瑟人物设定", "亞瑟：劍術高超。");
        let personality_uid = seed_entry(root.path(), &world_id, "亞瑟性格", "亞瑟：沉默寡言。");

        let outcome = RefactorOutcome {
            mode: None,
            characters: vec![character("亞瑟", &[bio_uid, personality_uid])],
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            player_index: Some(0),
            ..no_player_selection(vec![0])
        };

        let before_len = data::read_worldbook(root.path(), &world_id).unwrap().len();
        let result = apply_recorded(root.path(), &world_id, &outcome, &selection);
        assert_eq!(result.summary.new_characters, 1);
        assert_eq!(result.summary.deleted_entries, 2);
        assert!(result.summary.player_assigned);

        let character_id = result.character_ids[0].clone();
        assert!(data::read_character(root.path(), &world_id, &character_id).is_ok());
        let state = data::read_state(root.path(), &world_id).unwrap();
        assert_eq!(state.player_card_id.as_deref(), Some(character_id.as_str()));
        let entries = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(entries.len(), before_len - 2);
        assert!(entries.iter().all(|entry| entry.uid != bio_uid && entry.uid != personality_uid));

        receipts::undo_last_import(root.path(), &world_id).unwrap();
        assert!(data::read_character(root.path(), &world_id, &character_id).is_err());
        let state_after = data::read_state(root.path(), &world_id).unwrap();
        assert!(state_after.player_card_id.is_none());
        let restored = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(restored.len(), before_len);
        // 刪除後靠 upsert 插回，新 uid 跟原本不同——原樣回來看內容，不看 uid 是否相同。
        let restored_bio = restored.iter().find(|entry| entry.content == "亞瑟：劍術高超。").unwrap();
        assert!(!restored_bio.is_person);
        assert!(restored.iter().any(|entry| entry.content == "亞瑟：沉默寡言。"));
    }

    /// 玩法標記持久化（refactor-mode-split）：套用把 outcome.mode 寫進桌面狀態；
    /// characters 並清掉舊 interface 套用殘留的殼檔（fallback 抑制第一層）。
    #[test]
    fn apply_persists_refactor_mode_and_characters_removes_stale_shell() {
        let root = TestRoot::new("mode-persist");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        data::write_interface_shell(root.path(), &world_id, "<html>舊殼</html>").unwrap();
        let outcome = RefactorOutcome {
            mode: Some("characters".to_owned()),
            characters: Vec::new(),
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        apply(root.path(), &world_id, &outcome, &no_player_selection(Vec::new())).unwrap();
        let state = data::read_state(root.path(), &world_id).unwrap();
        assert_eq!(state.refactor_mode.as_deref(), Some("characters"));
        assert!(data::read_interface_shell(root.path(), &world_id).unwrap().is_none());

        // interface 模式：mode 照寫、殼不動（這輪沒產殼也不清別輪的——interface 殼由套用介面那段管）
        let mut interface_outcome = outcome.clone();
        interface_outcome.mode = Some("interface".to_owned());
        apply(root.path(), &world_id, &interface_outcome, &no_player_selection(Vec::new())).unwrap();
        assert_eq!(
            data::read_state(root.path(), &world_id).unwrap().refactor_mode.as_deref(),
            Some("interface")
        );

        // 舊產物（mode 缺席）：不動既有標記
        let mut legacy = outcome.clone();
        legacy.mode = None;
        apply(root.path(), &world_id, &legacy, &no_player_selection(Vec::new())).unwrap();
        assert_eq!(
            data::read_state(root.path(), &world_id).unwrap().refactor_mode.as_deref(),
            Some("interface")
        );
    }

    /// mode 只收二值列舉：手改匯入檔的大小寫／未知值不落地，trim 後合法照收。
    #[test]
    fn apply_ignores_invalid_mode_values() {
        let root = TestRoot::new("mode-enum");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let mut outcome = RefactorOutcome {
            mode: Some("Characters".to_owned()),
            characters: Vec::new(),
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        apply(root.path(), &world_id, &outcome, &no_player_selection(Vec::new())).unwrap();
        assert_eq!(data::read_state(root.path(), &world_id).unwrap().refactor_mode, None);

        outcome.mode = Some(" characters ".to_owned());
        apply(root.path(), &world_id, &outcome, &no_player_selection(Vec::new())).unwrap();
        assert_eq!(
            data::read_state(root.path(), &world_id).unwrap().refactor_mode.as_deref(),
            Some("characters")
        );
    }

    /// 讀取端正規化：舊版落地的 "Characters"／空白修成合法值，未知值回 Err（fail-closed）。
    #[test]
    fn normalize_stored_mode_fixes_legacy_case_and_rejects_unknown() {
        assert_eq!(normalize_stored_mode(None), Ok(None));
        assert_eq!(
            normalize_stored_mode(Some(" Characters ".to_owned())),
            Ok(Some("characters".to_owned()))
        );
        assert_eq!(
            normalize_stored_mode(Some("interface".to_owned())),
            Ok(Some("interface".to_owned()))
        );
        assert!(normalize_stored_mode(Some("both".to_owned())).is_err());
    }

    /// (b) 玩家卡限制：桌上已有玩家卡時，指定第二張要整批失敗、不寫入任何東西
    /// （沿用 data.rs 既有的一桌一張限制與錯誤訊息）。
    #[test]
    fn apply_rejects_second_player_card_and_writes_nothing() {
        let root = TestRoot::new("second-player");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let existing_player = CharacterCard {
            id: data::new_id(),
            name: "既有玩家".to_owned(),
            color: "#fff".to_owned(),
            avatar: "🧑".to_owned(),
            tier: Tier::Balanced,
            show_image: true,
            archived: false,
            gen_prompt: String::new(),
            public_md: String::new(),
            private_md: String::new(),
        };
        data::write_character(root.path(), &world_id, &existing_player).unwrap();
        let mut state = data::read_state(root.path(), &world_id).unwrap();
        state.player_card_id = Some(existing_player.id.clone());
        data::write_state(root.path(), &world_id, &state).unwrap();

        let uid = seed_entry(root.path(), &world_id, "新來的人", "新來的人的設定");
        let outcome = RefactorOutcome {
            mode: None,
            characters: vec![character("新來的人", &[uid])],
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            player_index: Some(0),
            ..no_player_selection(vec![0])
        };

        let error = apply(root.path(), &world_id, &outcome, &selection).unwrap_err();
        assert_eq!(error.to_string(), "這桌已經有玩家卡");
        assert_eq!(data::list_characters(root.path(), &world_id).unwrap().len(), 0); // list_characters 排除玩家卡本人，新卡也沒寫入
        assert!(data::read_worldbook(root.path(), &world_id).unwrap().iter().any(|entry| entry.uid == uid)); // 沒被刪
    }

    /// (c) 沒勾的人一律維持現行機制，各自生一條獨立 is_person 條目；勾了的人專屬來源條目
    /// 整條刪除。兩件事對每個人各自獨立成立，不因為同一次套用裡有人選中有人沒選就互相影響。
    #[test]
    fn apply_unselected_person_gets_independent_person_entry_selected_persons_source_deleted() {
        let root = TestRoot::new("zero-selected");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let picked_uid = seed_entry(root.path(), &world_id, "被選中的條目", "會被升格的人");
        let ignored_uid = seed_entry(root.path(), &world_id, "沒被勾的條目", "沒人被勾的人");

        let outcome = RefactorOutcome {
            mode: None,
            characters: vec![character("阿明", &[picked_uid]), character("小華", &[ignored_uid])],
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = no_player_selection(vec![0]); // 只勾阿明；小華（index 1）沒勾

        let before_len = data::read_worldbook(root.path(), &world_id).unwrap().len();
        let result = apply(root.path(), &world_id, &outcome, &selection).unwrap();
        assert_eq!(result.summary.new_characters, 1);
        assert_eq!(result.summary.new_entries, 1); // 小華獨立成一條 is_person 條目
        assert_eq!(result.summary.deleted_entries, 1); // 阿明的專屬來源條目整條刪除

        let entries = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(entries.len(), before_len); // 少一條（阿明來源刪除）多一條（小華新增），淨零
        assert!(!entries.iter().any(|entry| entry.uid == picked_uid));
        let ignored_entry = entries.iter().find(|entry| entry.uid == ignored_uid).unwrap();
        assert_eq!(ignored_entry.content, "沒人被勾的人"); // 小華的原始專屬條目原樣不動
        let person_entry = entries.iter().find(|entry| entry.is_person).unwrap();
        assert_eq!(person_entry.title, "小華");
    }

    /// (b2) 七人共用一條合集，只勾兩人：兩張角色卡＋五條獨立 is_person 條目；沒有收尾判定，
    /// 合集條目原樣保留（基準是優先保留）→ undo → 角色卡與新條目都回原樣。
    #[test]
    fn apply_partial_group_selection_creates_person_entries_for_the_rest_and_keeps_shared_source() {
        let root = TestRoot::new("partial-group");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "旅團", "七人旅團的合集設定");

        let names = ["甲", "乙", "丙", "丁", "戊", "己", "庚"];
        let characters: Vec<RefactorCharacter> =
            names.iter().map(|name| character(name, &[source_uid])).collect();
        let outcome = RefactorOutcome {
            mode: None,
            characters,
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = no_player_selection(vec![0, 1]); // 甲、乙

        let before_entries = data::read_worldbook(root.path(), &world_id).unwrap().len();
        let result = apply_recorded(root.path(), &world_id, &outcome, &selection);
        assert_eq!(result.summary.new_characters, 2);
        assert_eq!(result.summary.new_entries, 5);
        assert_eq!(result.summary.deleted_entries, 0);
        assert_eq!(data::list_characters(root.path(), &world_id).unwrap().len(), 2);

        let entries = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(entries.len(), before_entries + 5); // 合集條目原樣保留，沒被刪也沒被改寫
        let person_names: Vec<&str> = entries
            .iter()
            .filter(|entry| entry.is_person)
            .map(|entry| entry.title.as_str())
            .collect();
        assert_eq!(person_names.len(), 5);
        for name in ["丙", "丁", "戊", "己", "庚"] {
            assert!(person_names.contains(&name));
        }
        let source_entry = entries.iter().find(|entry| entry.uid == source_uid).unwrap();
        assert_eq!(source_entry.content, "七人旅團的合集設定");

        receipts::undo_last_import(root.path(), &world_id).unwrap();
        assert_eq!(data::list_characters(root.path(), &world_id).unwrap().len(), 0);
        let after_undo = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(after_undo.len(), before_entries);
    }

    /// (h) 共用合集條目，finishing 判定可刪，但只有一位共用者被勾：條目原樣保留
    /// （要點 7：判斷不出來或還有人沒勾就不動）。
    #[test]
    fn apply_shared_uid_kept_when_not_all_owners_selected() {
        let root = TestRoot::new("shared-uid-partial");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let shared_uid = seed_entry(root.path(), &world_id, "角色速览", "霍玄：……長老：……");

        let outcome = RefactorOutcome {
            mode: None,
            characters: vec![character("霍玄", &[shared_uid]), character("長老", &[shared_uid])],
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: vec![shared_uid.to_string()],
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = no_player_selection(vec![0]); // 只勾霍玄

        let result = apply(root.path(), &world_id, &outcome, &selection).unwrap();
        assert_eq!(result.summary.deleted_entries, 0);
        assert!(data::read_worldbook(root.path(), &world_id).unwrap().iter().any(|entry| entry.uid == shared_uid));
    }

    /// (i) 共用合集條目，finishing 判定可刪、全部共用者都被勾：整條刪除，且只刪一次
    /// （兩人都指到同一 uid，不因此刪兩次）。
    #[test]
    fn apply_shared_uid_deleted_once_when_all_owners_selected_and_verdict_deletable() {
        let root = TestRoot::new("shared-uid-full");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let shared_uid = seed_entry(root.path(), &world_id, "角色速览", "霍玄：……長老：……");

        let outcome = RefactorOutcome {
            mode: None,
            characters: vec![character("霍玄", &[shared_uid]), character("長老", &[shared_uid])],
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: vec![shared_uid.to_string()],
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = no_player_selection(vec![0, 1]);

        let result = apply(root.path(), &world_id, &outcome, &selection).unwrap();
        assert_eq!(result.summary.deleted_entries, 1);
        assert!(!data::read_worldbook(root.path(), &world_id).unwrap().iter().any(|entry| entry.uid == shared_uid));
    }

    /// (j) 共用合集條目沒有收尾判定（不在 deletable_shared_uids）：即使全部共用者都被勾，
    /// 一樣原樣保留——沒把握就不動是保底，不是漏做。
    #[test]
    fn apply_shared_uid_kept_without_finish_verdict_even_if_all_owners_selected() {
        let root = TestRoot::new("shared-uid-no-verdict");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let shared_uid = seed_entry(root.path(), &world_id, "角色速览", "霍玄：……長老：……");

        let outcome = RefactorOutcome {
            mode: None,
            characters: vec![character("霍玄", &[shared_uid]), character("長老", &[shared_uid])],
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = no_player_selection(vec![0, 1]);

        let result = apply(root.path(), &world_id, &outcome, &selection).unwrap();
        assert_eq!(result.summary.deleted_entries, 0);
    }

    /// (d) 介面套用重建狀態樹：同名頂層鍵保留進度、舊 schema 殘渣清掉 → undo 後逐鍵回復。
    #[test]
    fn apply_interface_rebuilds_dirty_state_and_undo_restores_every_key() {
        let root = TestRoot::new("interface");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "介面腳本", "描述如何顯示狀態欄的散文");
        let mut before_state = data::read_state(root.path(), &world_id).unwrap();
        before_state.state.tree = BTreeMap::from([
            ("沦陷天数".to_owned(), StateNode::Leaf("7".to_owned())),
            ("淪陷天數".to_owned(), StateNode::Leaf("42".to_owned())),
            ("舊欄位".to_owned(), StateNode::Leaf("殘渣".to_owned())),
        ]);
        before_state.state.jumps = BTreeMap::from([
            ("沦陷天数".to_owned(), "+1".to_owned()),
            ("淪陷天數".to_owned(), "+1".to_owned()),
        ]);
        let before_tree = before_state.state.tree.clone();
        data::write_state(root.path(), &world_id, &before_state).unwrap();

        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: Some(RefactorInterface {
                state_fields: serde_json::json!({
                    "淪陷天數": "0",
                    "世界": { "時間": "清晨" }
                }),
                source_uids: vec![source_uid.to_string()],
                raw: "描述如何顯示狀態欄的散文".to_owned(),
                shell: None,
                rules: BTreeMap::new(),
                guide: String::new(),
            }),
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: true,
            mechanism_indices: Vec::new(),
            entry_indices: Vec::new(),
            player_index: None,
        };

        let result = apply_recorded(root.path(), &world_id, &outcome, &selection);
        assert!(result.summary.interface_applied);
        assert_eq!(result.summary.deleted_entries, 1);
        assert_eq!(result.summary.rewritten_entries, 0);

        let state = data::read_state(root.path(), &world_id).unwrap();
        assert_eq!(
            state.state.tree,
            BTreeMap::from([
                ("淪陷天數".to_owned(), StateNode::Leaf("42".to_owned())),
                ("世界".to_owned(), StateNode::Branch(BTreeMap::from([("時間".to_owned(), StateNode::Leaf("清晨".to_owned()))]))),
            ])
        );
        assert_eq!(state.state.jumps, BTreeMap::from([("淪陷天數".to_owned(), "+1".to_owned())]));
        assert!(!data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .iter()
            .any(|entry| entry.uid == source_uid));

        receipts::undo_last_import(root.path(), &world_id).unwrap();
        let state_after = data::read_state(root.path(), &world_id).unwrap();
        assert_eq!(state_after.state.tree, before_tree);
        let source_entry_after = data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .into_iter()
            .find(|entry| entry.uid == source_uid)
            .unwrap();
        assert_eq!(source_entry_after.content, "描述如何顯示狀態欄的散文");
    }

    /// characters 產物即使 selection 勾了介面（匯入路徑會全勾）也整段不套：狀態樹不動、
    /// 不開增量協定、不寫殼檔、介面來源條目不因「已消耗」被刪——mode 閘門必須在來源消耗
    /// 判定之前生效（GUI 回歸：C2 桌套出五棵介面狀態樹）。
    #[test]
    fn apply_characters_mode_skips_interface_and_keeps_sources() {
        let root = TestRoot::new("characters-skips-interface");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "介面腳本", "描述如何顯示狀態欄的散文");

        let outcome = RefactorOutcome {
            mode: Some("characters".to_owned()),
            characters: Vec::new(),
            interface: Some(RefactorInterface {
                state_fields: serde_json::json!({ "世界": { "時間": "清晨" } }),
                source_uids: vec![source_uid.to_string()],
                raw: "描述如何顯示狀態欄的散文".to_owned(),
                shell: Some("<div>{{世界.時間}}</div>".to_owned()),
                rules: BTreeMap::from([(
                    "世界.時間".to_owned(),
                    FieldRule {
                        kind: FieldKind::Text,
                        min: None,
                        max: None,
                        update: UpdateMode::Replace,
                        inject: InjectLevel::Turn,
                        branch: None,
                        formula: None,
                    },
                )]),
                guide: "每回合報時間".to_owned(),
            }),
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: true,
            mechanism_indices: Vec::new(),
            entry_indices: Vec::new(),
            player_index: None,
        };

        let result = apply(root.path(), &world_id, &outcome, &selection).unwrap();
        assert!(!result.summary.interface_applied);
        assert_eq!(result.summary.deleted_entries, 0);

        let state = data::read_state(root.path(), &world_id).unwrap();
        assert_eq!(state.refactor_mode.as_deref(), Some("characters"));
        assert!(state.state.tree.is_empty());
        assert!(!state.mechanism.incremental);
        assert!(state.mechanism.rules.is_empty());
        assert!(!data::interface_shell_path(root.path(), &world_id).unwrap().exists());
        assert!(data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .iter()
            .any(|entry| entry.uid == source_uid));
    }

    /// 整條淘汰（rule 5／判官 rule 1–4，span 空）且玩家沒放回的既有條目：套用時停用、
    /// 快照進 rewritten_entries，undo 覆寫回原樣；半條淘汰（span 非空）的條目其餘段落
    /// 還在服役，不停用（GUI 回歸：A 桌「格式」「COT」constant 條目照常注入）。
    #[test]
    fn apply_disables_whole_entry_drops_and_undo_restores() {
        let root = TestRoot::new("disable-dropped");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let whole_uid = seed_entry(root.path(), &world_id, "格式", "<mainPage>卡片介面輸出協定</mainPage>");
        let partial_uid = seed_entry(root.path(), &world_id, "混合條目", "第一段。\n\n第二段。");

        let outcome = RefactorOutcome {
            mode: Some("characters".to_owned()),
            characters: Vec::new(),
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: vec![
                crate::refactor_assemble::RefactorDroppedEntry {
                    uid: whole_uid.to_string(),
                    span: String::new(),
                    title: "格式".to_owned(),
                    content: "<mainPage>卡片介面輸出協定</mainPage>".to_owned(),
                    rule: 5,
                },
                crate::refactor_assemble::RefactorDroppedEntry {
                    uid: partial_uid.to_string(),
                    span: "s1".to_owned(),
                    title: "混合條目".to_owned(),
                    content: "第二段。".to_owned(),
                    rule: 2,
                },
            ],
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = no_player_selection(Vec::new());

        let result = apply_recorded(root.path(), &world_id, &outcome, &selection);
        assert_eq!(result.summary.rewritten_entries, 1);
        assert_eq!(result.rewritten_entries.len(), 1);
        assert_eq!(result.rewritten_entries[0].uid, whole_uid);
        assert!(!result.rewritten_entries[0].disabled);

        let worldbook = data::read_worldbook(root.path(), &world_id).unwrap();
        let whole = worldbook.iter().find(|entry| entry.uid == whole_uid).unwrap();
        assert!(whole.disabled);
        assert_eq!(whole.content, "<mainPage>卡片介面輸出協定</mainPage>");
        let partial = worldbook.iter().find(|entry| entry.uid == partial_uid).unwrap();
        assert!(!partial.disabled);

        receipts::undo_last_import(root.path(), &world_id).unwrap();
        let restored = data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .into_iter()
            .find(|entry| entry.uid == whole_uid)
            .unwrap();
        assert!(!restored.disabled);
    }

    /// NorthHall 實測樣態縮小版：模型把狀態欄輸出成頂層＋「状态栏」鏡像分支兩套，殼只綁
    /// 頂層。正規化要把分支的非空初始值收進頂層、多出的葉改掛頂層路徑、rules 全部 remap
    /// 去重，分支整個消失——否則 GM 照分支路徑寫，殼綁的頂層值永遠空字串。
    #[test]
    fn normalize_collapses_mirror_branch_with_shell_deciding_canon() {
        let rule = |kind: FieldKind| FieldRule {
            kind,
            min: None,
            max: None,
            update: UpdateMode::Replace,
            inject: InjectLevel::Turn,
            branch: None,
            formula: None,
        };
        let state_fields = serde_json::json!({
            "地點": "",
            "日期時間": "",
            "霍玄": { "行動": "" },
            "行動選項": { "選項1": "" },
            "状态栏": {
                "地點": "📍 霍府",
                "日期時間": "⏰ 大梁年間",
                "霍玄": { "行動": "站著", "名字": "霍玄" },
                "行動選項": { "選項1": "選A" }
            }
        });
        let shell = "<div>{{地點}}{{日期時間}}{{霍玄.行動}}{{行動選項.選項1}}{{本回合.正文}}</div>";
        let rules = BTreeMap::from([
            ("地點".to_owned(), rule(FieldKind::Text)),
            ("状态栏.地點".to_owned(), rule(FieldKind::Text)),
            ("状态栏.霍玄.名字".to_owned(), rule(FieldKind::ReadOnly)),
        ]);

        let (fields, rules) = normalize_interface_paths(&state_fields, Some(shell), &rules).unwrap();

        assert_eq!(
            fields,
            serde_json::json!({
                "地點": "📍 霍府",
                "日期時間": "⏰ 大梁年間",
                "霍玄": { "行動": "站著", "名字": "霍玄" },
                "行動選項": { "選項1": "選A" }
            })
        );
        assert_eq!(
            rules.keys().cloned().collect::<Vec<_>>(),
            vec!["地點".to_owned(), "霍玄.名字".to_owned()]
        );
        assert_eq!(rules["霍玄.名字"].kind, FieldKind::ReadOnly);
    }

    /// 同一欄位兩套非空初始值不一致＝產物自相矛盾，Err 拒套不猜；殼同時綁兩套路徑同理。
    #[test]
    fn normalize_rejects_conflicting_values_and_double_referenced_shell() {
        let state_fields = serde_json::json!({
            "地點": "王府",
            "日期時間": "清晨",
            "状态栏": { "地點": "霍府", "日期時間": "清晨" }
        });
        let error = normalize_interface_paths(&state_fields, Some("<div>{{地點}}</div>"), &BTreeMap::new())
            .unwrap_err();
        assert!(error.contains("初始值不一致"), "{error}");

        let mirrored = serde_json::json!({
            "地點": "",
            "日期時間": "",
            "状态栏": { "地點": "霍府", "日期時間": "清晨" }
        });
        let error = normalize_interface_paths(
            &mirrored,
            Some("<div>{{地點}}{{状态栏.日期時間}}</div>"),
            &BTreeMap::new(),
        )
        .unwrap_err();
        assert!(error.contains("自相矛盾"), "{error}");
    }

    /// 無殼＝state_fields 是權威：完整鏡像（分支每葉根層都有對應）才折疊；分支多一個
    /// 根層沒有的葉就不是鏡像，整份不動——沒有殼表態時不做任何有損猜測。
    #[test]
    fn normalize_without_shell_folds_only_complete_mirror() {
        let mirror = serde_json::json!({
            "地點": "",
            "日期時間": "清晨",
            "状态栏": { "地點": "霍府", "日期時間": "" }
        });
        let (fields, _) = normalize_interface_paths(&mirror, None, &BTreeMap::new()).unwrap();
        assert_eq!(fields, serde_json::json!({ "地點": "霍府", "日期時間": "清晨" }));

        let not_mirror = serde_json::json!({
            "地點": "",
            "日期時間": "",
            "状态栏": { "地點": "霍府", "日期時間": "", "額外欄": "值" }
        });
        let (fields, _) = normalize_interface_paths(&not_mirror, None, &BTreeMap::new()).unwrap();
        assert_eq!(fields, not_mirror);
    }

    /// 衝突產物在 preflight 就拒套：apply 回 Err，世界書、狀態樹、殼檔零變化——
    /// preflight 晚於任何寫入的話會變成沒有收據的半套用。
    #[test]
    fn apply_rejects_conflicting_interface_before_any_write() {
        let root = TestRoot::new("normalize-preflight");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "介面腳本", "狀態欄散文");

        let outcome = RefactorOutcome {
            mode: Some("interface".to_owned()),
            characters: vec![character("亞瑟", &[source_uid])],
            interface: Some(RefactorInterface {
                state_fields: serde_json::json!({
                    "地點": "王府",
                    "日期時間": "清晨",
                    "状态栏": { "地點": "霍府", "日期時間": "清晨" }
                }),
                source_uids: vec![source_uid.to_string()],
                raw: "狀態欄散文".to_owned(),
                shell: Some("<div>{{地點}}</div>".to_owned()),
                rules: BTreeMap::new(),
                guide: String::new(),
            }),
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: vec![0],
            apply_interface: true,
            mechanism_indices: Vec::new(),
            entry_indices: Vec::new(),
            player_index: None,
        };

        assert!(apply(root.path(), &world_id, &outcome, &selection).is_err());
        assert!(data::list_characters(root.path(), &world_id).unwrap().is_empty());
        assert!(data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .iter()
            .any(|entry| entry.uid == source_uid));
        assert!(data::read_state(root.path(), &world_id).unwrap().state.tree.is_empty());
        assert!(!data::interface_shell_path(root.path(), &world_id).unwrap().exists());
    }

    /// 介面套用要把新樹補進這一幕的事件快照：否則玩家一按收回，檯面就退回套用前的空樹，
    /// 佔位符全部填空、面板欄位一片空白（2026-08-12 實測踩到）。
    #[test]
    fn apply_interface_syncs_new_tree_into_scene_snapshots() {
        let root = TestRoot::new("interface-snapshot-sync");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "介面腳本", "描述如何顯示狀態欄的散文");
        for text in ["開場白", "玩家選角"] {
            data::append_transcript(
                root.path(),
                &world_id,
                0,
                &data::TranscriptEvent {
                    ts: "2026-08-12T10:00:00.000Z".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: data::TranscriptKind::Narration,
                    text: text.to_owned(),
                    raw: None,
                    state: None,
                    gm_only: false,
                },
            )
            .unwrap();
        }

        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: Some(RefactorInterface {
                state_fields: serde_json::json!({ "世界": { "時間": "清晨" } }),
                source_uids: vec![source_uid.to_string()],
                raw: "描述如何顯示狀態欄的散文".to_owned(),
                shell: None,
                rules: BTreeMap::new(),
                guide: String::new(),
            }),
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: true,
            mechanism_indices: Vec::new(),
            entry_indices: Vec::new(),
            player_index: None,
        };
        apply_recorded(root.path(), &world_id, &outcome, &selection);

        let expected = data::read_state(root.path(), &world_id).unwrap().state.tree;
        assert!(expected.contains_key("世界"));
        let events = data::read_transcript(root.path(), &world_id, 0).unwrap();
        assert_eq!(events.len(), 2);
        for event in &events {
            assert_eq!(event.state.as_ref().unwrap().tree, expected);
        }

        // 收回上一句後檯面退回前一則的快照，樹要還在
        assert!(data::pop_transcript(root.path(), &world_id, 0).unwrap());
        assert_eq!(data::read_state(root.path(), &world_id).unwrap().state.tree, expected);
    }

    #[test]
    fn apply_interface_with_non_object_state_fields_leaves_tree_unchanged() {
        let root = TestRoot::new("interface-invalid-state-fields");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let mut state = data::read_state(root.path(), &world_id).unwrap();
        state.state.tree = BTreeMap::from([("既有欄位".to_owned(), StateNode::Leaf("進度".to_owned()))]);
        let before_tree = state.state.tree.clone();
        data::write_state(root.path(), &world_id, &state).unwrap();
        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: Some(RefactorInterface {
                state_fields: serde_json::json!(["壞產物"]),
                source_uids: Vec::new(),
                raw: String::new(),
                shell: None,
                rules: BTreeMap::new(),
                guide: String::new(),
            }),
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: true,
            mechanism_indices: Vec::new(),
            entry_indices: Vec::new(),
            player_index: None,
        };

        apply(root.path(), &world_id, &outcome, &selection).unwrap();

        assert_eq!(data::read_state(root.path(), &world_id).unwrap().state.tree, before_tree);
    }

    /// 契約相容：AI 展開產物落地成 JSON 沒有 shell 鍵（舊版產物）照樣要能反序列化，shell 落
    /// None，不能因為多了新欄位就 fail closed。
    #[test]
    fn refactor_interface_deserializes_legacy_json_without_shell_field() {
        let legacy = serde_json::json!({
            "state_fields": { "World": { "Time": "清晨" } },
            "source_uids": ["7"],
            "raw": "原文",
        });
        let interface: RefactorInterface = serde_json::from_value(legacy).unwrap();
        assert!(interface.shell.is_none());
        assert_eq!(interface.source_uids, vec!["7"]);
        assert_eq!(interface.state_fields["World"]["Time"].as_str(), Some("清晨"));
    }

    #[test]
    fn apply_selected_rewritten_entries_creates_locked_mechanism_merges_rules_logs_and_deletes_shared_source() {
        let root = TestRoot::new("rewritten-entries-full");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "舊設定", "舊世界書全文");
        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: None,
            mechanisms: Vec::new(),
            entries: vec![
                crate::refactor_ai::RefactorNewEntry {
                    title: "新世界觀".to_owned(),
                    kind: "setting".to_owned(),
                    content: "重寫的世界觀".to_owned(),
                    source_uids: vec![source_uid.to_string()],
                    rules: BTreeMap::new(),
                    triggers: Vec::new(),
                    meta: None,
                },
                crate::refactor_ai::RefactorNewEntry {
                    title: "新戰鬥規則".to_owned(),
                    kind: "mechanism".to_owned(),
                    content: "重寫的戰鬥說明".to_owned(),
                    source_uids: vec![source_uid.to_string()],
                    rules: BTreeMap::from([(
                        "World.戰鬥值".to_owned(),
                        FieldRule::for_kind(data::FieldKind::Number),
                    )]),
                    triggers: Vec::new(),
                    meta: None,
                },
            ],
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: false,
            mechanism_indices: Vec::new(),
            entry_indices: vec![0, 1],
            player_index: None,
        };

        let result = apply(root.path(), &world_id, &outcome, &selection).unwrap();
        assert_eq!(result.summary.new_entries, 2);
        assert_eq!(result.summary.deleted_entries, 1);
        let entries = data::read_worldbook(root.path(), &world_id).unwrap();
        assert!(!entries.iter().any(|entry| entry.uid == source_uid));
        assert!(entries.iter().any(|entry| entry.title == "新世界觀" && !entry.locked));
        assert!(entries.iter().any(|entry| entry.title == "新戰鬥規則" && entry.locked));
        assert!(data::read_state(root.path(), &world_id)
            .unwrap()
            .mechanism
            .rules
            .contains_key("World.戰鬥值"));
        assert_eq!(
            mechanism::read_ledger(root.path(), &world_id)
                .entries
                .iter()
                .find(|entry| entry.title == "新戰鬥規則")
                .unwrap()
                .kind,
            mechanism::RecordKind::Absorbed
        );
    }

    #[test]
    fn apply_partially_selected_rewritten_entries_keeps_shared_source() {
        let root = TestRoot::new("rewritten-entries-partial");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "舊設定", "舊世界書全文");
        let entry = |title: &str| crate::refactor_ai::RefactorNewEntry {
            title: title.to_owned(),
            kind: "setting".to_owned(),
            content: format!("{title} 重寫內容"),
            source_uids: vec![source_uid.to_string()],
            rules: BTreeMap::new(),
            triggers: Vec::new(),
            meta: None,
        };
        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: None,
            mechanisms: Vec::new(),
            entries: vec![entry("新設定甲"), entry("新設定乙")],
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: false,
            mechanism_indices: Vec::new(),
            entry_indices: vec![0],
            player_index: None,
        };

        let result = apply(root.path(), &world_id, &outcome, &selection).unwrap();
        assert_eq!(result.summary.new_entries, 1);
        assert_eq!(result.summary.deleted_entries, 0);
        let entries = data::read_worldbook(root.path(), &world_id).unwrap();
        assert!(entries.iter().any(|entry| entry.uid == source_uid));
        assert!(entries.iter().any(|entry| entry.title == "新設定甲"));
    }

    #[test]
    fn legacy_outcome_without_entries_deserializes_and_applies() {
        let outcome: RefactorOutcome = serde_json::from_value(serde_json::json!({
            "characters": [],
            "interface": null,
            "mechanisms": [],
            "deletable_shared_uids": []
        }))
        .unwrap();
        assert!(outcome.entries.is_empty());
        let root = TestRoot::new("legacy-outcome-no-entries");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let selection: RefactorSelection = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(apply(root.path(), &world_id, &outcome, &selection).unwrap().summary.new_entries, 0);
    }

    /// 介面套用帶殼：world 目錄落一份 interface-shell.html，data::read_interface_shell（讀
    /// command 背後的邏輯層）讀得回來、內容一致。
    #[test]
    fn apply_interface_with_shell_writes_file_readable_via_data_layer() {
        let root = TestRoot::new("interface-shell-write");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "介面腳本", "描述如何顯示狀態欄的散文");
        let shell_html = "<!DOCTYPE html><html><body>{{World.Time}}</body></html>";

        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: Some(RefactorInterface {
                state_fields: serde_json::json!({ "World": { "Time": "清晨" } }),
                source_uids: vec![source_uid.to_string()],
                raw: "描述如何顯示狀態欄的散文".to_owned(),
                shell: Some(shell_html.to_owned()),
                rules: BTreeMap::from([(
                    "World.Time".to_owned(),
                    FieldRule::for_kind(data::FieldKind::Text),
                )]),
                guide: "每回合都要重報 World.Time。".to_owned(),
            }),
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: true,
            mechanism_indices: Vec::new(),
            entry_indices: Vec::new(),
            player_index: None,
        };

        apply(root.path(), &world_id, &outcome, &selection).unwrap();

        let read_back = data::read_interface_shell(root.path(), &world_id).unwrap();
        assert_eq!(read_back.as_deref(), Some(shell_html));
        // 接管卡的每一格都靠 GM 回報才會動：增量協定要開，卡自訂的欄位規則與回報指引要落檔
        let mechanism = data::read_state(root.path(), &world_id).unwrap().mechanism;
        assert!(mechanism.incremental);
        assert_eq!(
            mechanism.rules.get("World.Time").map(|rule| rule.kind),
            Some(data::FieldKind::Text)
        );
        assert_eq!(mechanism.guide, "每回合都要重報 World.Time。");
    }

    /// 介面套用沒帶殼（shell=None）：不落任何殼檔。
    #[test]
    fn apply_interface_without_shell_creates_no_shell_file() {
        let root = TestRoot::new("interface-shell-absent");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "介面腳本", "描述如何顯示狀態欄的散文");

        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: Some(RefactorInterface {
                state_fields: serde_json::json!({ "World": { "Time": "清晨" } }),
                source_uids: vec![source_uid.to_string()],
                raw: "描述如何顯示狀態欄的散文".to_owned(),
                shell: None,
                rules: BTreeMap::new(),
                guide: String::new(),
            }),
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: true,
            mechanism_indices: Vec::new(),
            entry_indices: Vec::new(),
            player_index: None,
        };

        apply(root.path(), &world_id, &outcome, &selection).unwrap();

        assert!(data::read_interface_shell(root.path(), &world_id).unwrap().is_none());
    }

    /// 介面套用帶殼 → undo：殼檔是這次套用新建的，undo 要把它刪掉（比照 world_card_created
    /// 的參考模式：只刪這次新建的，套用前就有的不動——這裡每次都是新桌，天然滿足這個條件）。
    #[test]
    fn apply_interface_shell_then_undo_deletes_shell_file() {
        let root = TestRoot::new("interface-shell-undo");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "介面腳本", "描述如何顯示狀態欄的散文");

        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: Some(RefactorInterface {
                state_fields: serde_json::json!({ "World": { "Time": "清晨" } }),
                source_uids: vec![source_uid.to_string()],
                raw: "描述如何顯示狀態欄的散文".to_owned(),
                shell: Some("<!DOCTYPE html><html><body>{{World.Time}}</body></html>".to_owned()),
                rules: BTreeMap::new(),
                guide: String::new(),
            }),
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: true,
            mechanism_indices: Vec::new(),
            entry_indices: Vec::new(),
            player_index: None,
        };

        apply_recorded(root.path(), &world_id, &outcome, &selection);
        assert!(data::read_interface_shell(root.path(), &world_id).unwrap().is_some());

        receipts::undo_last_import(root.path(), &world_id).unwrap();
        assert!(data::read_interface_shell(root.path(), &world_id).unwrap().is_none());
    }

    /// (e) 帳本轉換：來源條目原本在帳本裡是 Skipped（例如認不出的 EJS），套用機制後帳本要
    /// 改記 Absorbed——玩家在帳本分頁看到的是「已被收編」，不再是「跳過」。
    #[test]
    fn apply_mechanism_deletes_source_after_recording_absorption() {
        let root = TestRoot::new("ledger-skipped-to-absorbed");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "詭異的機制腳本", "<% 認不出的 EJS %>");
        mechanism::append_log(
            root.path(),
            &world_id,
            0,
            &[mechanism::Record {
                kind: mechanism::RecordKind::Skipped,
                path: "詭異的機制腳本".to_owned(),
                detail: "卡片腳本認不出來，沒轉成觸發表，預設不送模型。".to_owned(),
            }],
        );

        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: None,
            mechanisms: vec![RefactorMechanism {
                source_uid: source_uid.to_string(),
                rules: BTreeMap::from([(
                    "World.詭異值".to_owned(),
                    FieldRule::for_kind(data::FieldKind::Number),
                )]),
                triggers: Vec::new(),
            }],
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: false,
            mechanism_indices: vec![0],
            entry_indices: Vec::new(),
            player_index: None,
        };

        apply(root.path(), &world_id, &outcome, &selection).unwrap();

        assert!(!data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .iter()
            .any(|entry| entry.uid == source_uid));
    }

    /// (f) 帳本新增：純散文機制條目（帳本裡原本沒有這條）套用後要新增一筆 Absorbed 記錄。
    #[test]
    fn apply_mechanism_deletes_source_with_no_prior_ledger_record() {
        let root = TestRoot::new("ledger-new-absorbed");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "純散文機制", "打鬥時擲骰決勝負。");

        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: None,
            mechanisms: vec![RefactorMechanism {
                source_uid: source_uid.to_string(),
                rules: BTreeMap::from([(
                    "World.戰鬥值".to_owned(),
                    FieldRule::for_kind(data::FieldKind::Number),
                )]),
                triggers: Vec::new(),
            }],
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: false,
            mechanism_indices: vec![0],
            entry_indices: Vec::new(),
            player_index: None,
        };

        assert!(mechanism::read_ledger(root.path(), &world_id).entries.is_empty());

        apply(root.path(), &world_id, &outcome, &selection).unwrap();

        assert!(!data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .iter()
            .any(|entry| entry.uid == source_uid));
    }

    /// (g) undo 帳本回退：套用前是 Skipped，套用後變 Absorbed，undo 之後帳本要退回原本的
    /// Skipped 記錄。
    #[test]
    fn apply_mechanism_then_undo_restores_ledger_to_previous_state() {
        let root = TestRoot::new("ledger-undo");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "詭異的機制腳本二號", "<% 認不出的 EJS %>");
        mechanism::append_log(
            root.path(),
            &world_id,
            0,
            &[mechanism::Record {
                kind: mechanism::RecordKind::Skipped,
                path: "詭異的機制腳本二號".to_owned(),
                detail: "卡片腳本認不出來，沒轉成觸發表，預設不送模型。".to_owned(),
            }],
        );

        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: None,
            mechanisms: vec![RefactorMechanism {
                source_uid: source_uid.to_string(),
                rules: BTreeMap::from([(
                    "World.詭異值二".to_owned(),
                    FieldRule::for_kind(data::FieldKind::Number),
                )]),
                triggers: Vec::new(),
            }],
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: false,
            mechanism_indices: vec![0],
            entry_indices: Vec::new(),
            player_index: None,
        };

        apply_recorded(root.path(), &world_id, &outcome, &selection);
        assert!(!data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .iter()
            .any(|entry| entry.uid == source_uid));

        receipts::undo_last_import(root.path(), &world_id).unwrap();

        let after_undo = mechanism::read_ledger(root.path(), &world_id);
        let entry = after_undo
            .entries
            .iter()
            .find(|entry| entry.title == "詭異的機制腳本二號")
            .unwrap();
        assert_eq!(entry.kind, mechanism::RecordKind::Skipped);
    }

    /// 匯出重構產物包 (a)：apply 成功後 data::read_refactor_outcome 讀得回來，serde 反序列化
    /// 回 RefactorOutcome 與送進去的一致（round-trip）。
    #[test]
    fn apply_writes_refactor_outcome_file_readable_and_round_trips() {
        let root = TestRoot::new("export-outcome-roundtrip");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let uid = seed_entry(root.path(), &world_id, "亞瑟人物设定", "亞瑟：劍術高超。");

        let outcome = RefactorOutcome {
            mode: None,
            characters: vec![character("亞瑟", &[uid])],
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = no_player_selection(vec![0]);

        apply(root.path(), &world_id, &outcome, &selection).unwrap();

        let saved = data::read_refactor_outcome(root.path(), &world_id).unwrap().unwrap();
        let round_tripped: RefactorOutcome = serde_json::from_str(&saved).unwrap();
        assert_eq!(round_tripped, outcome);
    }

    /// 匯出重構產物包 (b)：apply 後跑既有 undo 流程，產物檔仍然存在——undo 與收據不動這個檔
    /// （零改動）。
    #[test]
    fn apply_then_undo_keeps_refactor_outcome_file() {
        let root = TestRoot::new("export-outcome-undo-keeps-file");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let uid = seed_entry(root.path(), &world_id, "亞瑟人物设定", "亞瑟：劍術高超。");

        let outcome = RefactorOutcome {
            mode: None,
            characters: vec![character("亞瑟", &[uid])],
            interface: None,
            mechanisms: Vec::new(),
            entries: Vec::new(),
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = no_player_selection(vec![0]);

        apply_recorded(root.path(), &world_id, &outcome, &selection);
        assert!(data::read_refactor_outcome(root.path(), &world_id).unwrap().is_some());

        receipts::undo_last_import(root.path(), &world_id).unwrap();
        assert!(data::read_refactor_outcome(root.path(), &world_id).unwrap().is_some());
    }

    /// 重構卡匯到新桌（來源 uid 在這桌不存在）：來源刪除不得誤刪剛落地的新條目（uid 撞號），
    /// undo 要把新條目（含 locked 機制條目）整批收回，不得靠「已刪來源」快照把它們插回來——
    /// 插回來的 locked 條目沒有編輯／刪除鈕，會變成玩家永遠動不了的孤兒。
    #[test]
    fn undo_removes_new_entries_including_locked() {
        let root = TestRoot::new("undo-removes-new-entries");
        let world_id = data::create_world(root.path(), "酒館").unwrap();

        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: None,
            mechanisms: Vec::new(),
            entries: vec![
                crate::refactor_ai::RefactorNewEntry {
                    title: "獸人部落文化".to_owned(),
                    kind: "setting".to_owned(),
                    content: "氏族階級制。".to_owned(),
                    source_uids: vec!["1".to_owned()],
                    rules: std::collections::BTreeMap::new(),
                    triggers: Vec::new(),
                    meta: None,
                },
                crate::refactor_ai::RefactorNewEntry {
                    title: "天數計時".to_owned(),
                    kind: "mechanism".to_owned(),
                    content: "每日推進。".to_owned(),
                    source_uids: vec!["1".to_owned()],
                    rules: std::collections::BTreeMap::from([(
                        "World.淪陷天數".to_owned(),
                        FieldRule::for_kind(crate::data::FieldKind::Number),
                    )]),
                    triggers: Vec::new(),
                    meta: None,
                },
            ],
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: false,
            mechanism_indices: Vec::new(),
            entry_indices: vec![0, 1],
            player_index: None,
        };

        apply_recorded(root.path(), &world_id, &outcome, &selection);
        let applied = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(applied.len(), 2);
        assert!(applied.iter().any(|entry| entry.locked));

        receipts::undo_last_import(root.path(), &world_id).unwrap();
        let after = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(
            after.iter().map(|entry| entry.title.as_str()).collect::<Vec<_>>(),
            Vec::<&str>::new(),
            "undo 後新條目應整批收回"
        );
    }

    /// 包 2：entries[].meta 有值時，套用後的世界書條目直接照抄 keys/constant/order/disabled/
    /// visibility/is_person；沒有 meta 的條目走現行預設（keys=[]／constant=false／order 用
    /// 遞增計數／visibility=Gm／is_person=false）——兩種條目同一次套用互不干擾。
    #[test]
    fn apply_entry_with_meta_preserves_fields_without_meta_uses_defaults() {
        let root = TestRoot::new("entry-meta-preservation");
        let world_id = data::create_world(root.path(), "酒館").unwrap();

        let outcome = RefactorOutcome {
            mode: None,
            characters: Vec::new(),
            interface: None,
            mechanisms: Vec::new(),
            entries: vec![
                crate::refactor_ai::RefactorNewEntry {
                    title: "照搬條目".to_owned(),
                    kind: "setting".to_owned(),
                    content: "原文照搬。".to_owned(),
                    source_uids: vec!["1".to_owned()],
                    rules: BTreeMap::new(),
                    triggers: Vec::new(),
                    meta: Some(crate::refactor_ai::RefactorEntryMeta {
                        keys: vec!["關鍵字".to_owned()],
                        constant: true,
                        order: 99,
                        disabled: true,
                        visibility: Visibility::Characters(vec!["char-1".to_owned()]),
                        is_person: true,
                    }),
                },
                crate::refactor_ai::RefactorNewEntry {
                    title: "新組裝條目".to_owned(),
                    kind: "setting".to_owned(),
                    content: "本地組裝的新內容。".to_owned(),
                    source_uids: vec!["2".to_owned()],
                    rules: BTreeMap::new(),
                    triggers: Vec::new(),
                    meta: None,
                },
            ],
            deletable_shared_uids: Vec::new(),
            dropped: Vec::new(),
            unabsorbed: Vec::new(),
            audit: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: false,
            mechanism_indices: Vec::new(),
            entry_indices: vec![0, 1],
            player_index: None,
        };

        apply(root.path(), &world_id, &outcome, &selection).unwrap();
        let entries = data::read_worldbook(root.path(), &world_id).unwrap();

        let with_meta = entries
            .iter()
            .find(|entry| entry.title == "照搬條目")
            .unwrap();
        assert_eq!(with_meta.keys, vec!["關鍵字".to_owned()]);
        assert!(with_meta.constant);
        assert_eq!(with_meta.order, 99);
        assert!(with_meta.disabled);
        assert_eq!(
            with_meta.visibility,
            Visibility::Characters(vec!["char-1".to_owned()])
        );
        assert!(with_meta.is_person);

        let without_meta = entries
            .iter()
            .find(|entry| entry.title == "新組裝條目")
            .unwrap();
        assert!(without_meta.keys.is_empty());
        assert!(!without_meta.constant);
        assert!(!without_meta.disabled);
        assert_eq!(without_meta.visibility, Visibility::Gm);
        assert!(!without_meta.is_person);
    }
}
