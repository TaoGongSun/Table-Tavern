//! 變數模式的寫入路徑（計畫 8.3）：改初始化來源（面板手改、匯入補值與復原）、卡寫、GM 回合與開場的新表、
//! 幕操作的種子。全部在短提交鎖內。
use super::control::{read_control, write_control, SceneVars};
use super::convert::{
    merge_tree_change, merge_tree_change_typed, stat_to_tree, Macros, NewValues, Tree, TypedBatch,
};
use super::json::Json;
use super::mode::{derive_display, ensure_active, new_token, refresh_cache};
use super::source::{init_source, Source};
use super::turn::{self, PendingMain, PendingPart, Phase, TurnKey, TurnRecord, PART_MAIN};
use super::VarsTable;
use crate::data::scene::{edit_line, find_event_rev, LineHead, TranscriptEvent};
use crate::data::state::{read_state, read_state_cache, write_state, WorldState};
use crate::data::state_commit::CommitTx;
use crate::data::{invalid_data, DataResult};
use std::collections::BTreeMap;

fn stat_of(table: &Json) -> Json {
    table
        .get("stat_data")
        .cloned()
        .unwrap_or_else(Json::empty_object)
}

/// 把新表寫回初始化來源：事件就改那一則（換新 token），種子就改控制檔。
fn write_source(tx: &CommitTx<'_>, scene: u64, source: &Source, table: &Json) -> DataResult<()> {
    match source {
        Source::Event { index, rev, .. } => {
            let found = edit_line(
                tx,
                scene,
                rev.as_deref(),
                |at, head| at == *index && head.vars_rev == *rev,
                |_, mut event| {
                    event.message_vars = Some(VarsTable::from_json(table));
                    event.vars_rev = Some(new_token());
                    Ok(((), Some(event)))
                },
            )?;
            found.ok_or_else(|| invalid_data("card-vars: 初始化來源在鎖內變了"))
        }
        Source::Seed { .. } => {
            let mut control = read_control(tx.root, tx.world_id)?;
            let vars = control
                .scenes
                .get_mut(&scene.to_string())
                .ok_or_else(|| invalid_data("card-vars: 這一幕沒有種子"))?;
            vars.seed = VarsTable::from_json(table);
            write_control(tx, &control)
        }
    }
}

/// 改目前有效狀態（初始化來源那一份）：`edit` 拿投影樹來改，回 false＝沒改。不是變數模式回 None，
/// 呼叫端照舊改 state.json 的樹。
/// - `keep_strings`：面板手改——舊值是字串就存字串，否則照包 1 規則還原型別。
/// - `import`：匯入補值——新值清 metadata、照控制檔的代換值換巨集。
pub fn edit_effective_tree(
    tx: &CommitTx<'_>,
    keep_strings: bool,
    import: bool,
    edit: impl FnOnce(&mut Tree) -> bool,
) -> DataResult<Option<bool>> {
    let control = read_control(tx.root, tx.world_id)?;
    let cache = read_state_cache(tx.root, tx.world_id)?;
    let scene = cache.current_scene;
    let Some(vars) = control.active(scene) else {
        return Ok(None);
    };
    let source = init_source(tx.root, tx.world_id, scene, vars)?;
    let before = source.tree();
    let mut after = before.clone();
    if !edit(&mut after) {
        return Ok(Some(false));
    }
    let rules = NewValues {
        types: &cache.mechanism.value_types,
        keep_strings,
        clean: if import {
            control.macros.as_ref()
        } else {
            None
        },
    };
    let mut table = source.table().clone();
    let stat = merge_tree_change(&stat_of(&table), &before, &after, &rules);
    table.insert("stat_data", stat);
    write_source(tx, scene, &source, &table)?;
    refresh_cache(tx);
    Ok(Some(true))
}

/// 匯入補值與匯入復原：變數模式時只改初始化來源那一份（新值清 metadata、照控制檔代換巨集），回 true；
/// 樹模式回 false，呼叫端照舊改 state.json 的樹。自己取短提交鎖。
pub fn edit_tree_if_events(
    root: &std::path::Path,
    world_id: &str,
    edit: impl FnOnce(&mut Tree) -> bool,
) -> DataResult<bool> {
    crate::data::state_commit::with_commit(root, world_id, |tx| {
        edit_effective_tree(tx, false, true, edit).map(|edited| edited.is_some())
    })
}

/// 卡寫的目標：事件 id；舊事件沒有 id 時以「位置＋整則內容」定位（第一次寫入時補上 id）。
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CardWriteTarget {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub index: Option<usize>,
    #[serde(default)]
    pub legacy: Option<TranscriptEvent>,
}

/// 卡寫結果。被拒時盡量附上目標的權威值與版本，宿主拿它推回沙盒。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CardWrite {
    Ok {
        event: TranscriptEvent,
    },
    Stale {
        found: bool,
        rev: Option<String>,
        table: Option<String>,
    },
    Busy {
        found: bool,
        rev: Option<String>,
        table: Option<String>,
    },
    Rejected {
        code: String,
        found: bool,
        rev: Option<String>,
        table: Option<String>,
    },
}

/// 輕量比對目標那一行：有 id 比 id；舊事件比位置（且那一行還沒有 id），完整內容在 `matches_legacy` 再核。
fn picks(target: &CardWriteTarget, index: usize, head: &LineHead) -> bool {
    match (&target.id, target.index, &target.legacy) {
        (Some(id), _, _) => head.id.as_deref() == Some(id.as_str()),
        (None, Some(at), Some(_)) => at == index && head.id.is_none(),
        _ => false,
    }
}

fn matches_legacy(target: &CardWriteTarget, event: &TranscriptEvent) -> bool {
    match (&target.id, &target.legacy) {
        (Some(_), _) => true,
        (None, Some(legacy)) => event == legacy,
        (None, None) => false,
    }
}

/// 目標目前的權威值：(找到沒, 版本, 表)。
fn authority(
    tx: &CommitTx<'_>,
    scene: u64,
    target: &CardWriteTarget,
) -> (bool, Option<String>, Option<String>) {
    match find_event_rev(
        tx.root,
        tx.world_id,
        scene,
        target.id.as_deref(),
        |index, head| picks(target, index, head),
    ) {
        Ok(Some((_, event))) if matches_legacy(target, &event) => (
            true,
            event.vars_rev.clone(),
            event
                .message_vars
                .as_ref()
                .map(|table| table.text().to_owned()),
        ),
        _ => (false, None, None),
    }
}

/// 卡寫（message 層）：鎖內驗 `{世代, 場, 事件, 預期版本}`，相符才把整張新表寫進那則事件、換新 token。
/// 不動種子；寫哪一樓只影響那一樓。表的上限由呼叫端先驗（`parse_table`）。
#[allow(clippy::too_many_arguments)]
pub fn card_write(
    tx: &CommitTx<'_>,
    macros: Option<&Macros>,
    generation: u64,
    scene: u64,
    target: &CardWriteTarget,
    expected_rev: Option<&str>,
    table: &Json,
) -> DataResult<CardWrite> {
    let stale = |tx: &CommitTx<'_>| {
        let (found, rev, table) = authority(tx, scene, target);
        CardWrite::Stale { found, rev, table }
    };
    if generation != turn::generation(tx) {
        return Ok(stale(tx));
    }
    if turn::busy(tx) {
        let (found, rev, table) = authority(tx, scene, target);
        return Ok(CardWrite::Busy { found, rev, table });
    }
    let current = read_state_cache(tx.root, tx.world_id)?.current_scene;
    if scene != current {
        return Ok(stale(tx));
    }
    if !ensure_active(tx, macros)? {
        let (found, rev, table) = authority(tx, scene, target);
        return Ok(CardWrite::Rejected {
            code: "not-variable-table".to_owned(),
            found,
            rev,
            table,
        });
    }
    let epoch = read_control(tx.root, tx.world_id)?
        .active(scene)
        .map(|vars| vars.epoch.clone())
        .ok_or_else(|| invalid_data("card-vars: 啟用後這一幕仍沒有 epoch"))?;
    let written = edit_line(
        tx,
        scene,
        target.id.as_deref(),
        |index, head| picks(target, index, head),
        |_, mut event| {
            if !matches_legacy(target, &event) || event.vars_rev.as_deref() != expected_rev {
                return Ok((None, None));
            }
            if event.id.is_none() {
                event.id = Some(new_token());
            }
            event.message_vars = Some(VarsTable::from_json(table));
            event.vars_rev = Some(new_token());
            event.vars_epoch = Some(epoch);
            Ok((Some(event.clone()), Some(event)))
        },
    )?
    .flatten();
    match written {
        Some(event) => {
            refresh_cache(tx);
            Ok(CardWrite::Ok { event })
        }
        None => Ok(stale(tx)),
    }
}

/// 回合各部分落成的事件（與前端落檔時的形狀相同）。
fn turn_event(
    kind: crate::data::TranscriptKind,
    text: String,
    raw: Option<String>,
    truncated: bool,
    marker: Option<crate::data::EventMarker>,
) -> DataResult<TranscriptEvent> {
    Ok(TranscriptEvent {
        ts: crate::data::local_timestamp()?,
        speaker_id: String::new(),
        speaker_name: "GM".to_owned(),
        kind,
        text,
        raw,
        state: None,
        truncated,
        gm_only: false,
        marker,
        opening: false,
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
    })
}

fn main_part(main: PendingMain) -> DataResult<PendingPart> {
    Ok(PendingPart {
        part: PART_MAIN.to_owned(),
        event: turn_event(
            crate::data::TranscriptKind::Narration,
            main.text,
            main.raw,
            main.truncated,
            None,
        )?,
    })
}

/// 代落：回合裡前端還沒落成的部分（正文、變動紀錄）依序用同一個冪等鍵落檔；前端晚到的重試回這幾則。
/// 生成中的回合沒有東西可落。任何一則落不成就回錯，呼叫端的新事件不能搶到它前面。
fn land_pending_parts(tx: &CommitTx<'_>) -> DataResult<()> {
    let Some(record) = turn::turn(tx) else {
        return Ok(());
    };
    if record.phase == Phase::Generating {
        return Ok(());
    }
    for pending in record.parts {
        let key = TurnKey {
            turn_id: record.turn_id.clone(),
            part: pending.part,
        };
        crate::data::append_event(
            tx.root,
            tx.world_id,
            record.scene,
            &pending.event,
            Some(&key),
        )?;
    }
    Ok(())
}

/// 換幕容量量測用：上一回合提交了（或中止留下半截）卻還沒落檔的事件與它所屬的幕。
/// 下一筆新事件之前一定會先代落，所以容量關卡要把它們算進本幕，才不會漏量或在擋下前就寫了檔。
pub fn unlanded_events(tx: &CommitTx<'_>) -> Vec<(u64, crate::data::TranscriptEvent)> {
    match turn::turn(tx) {
        Some(record) if record.phase != Phase::Generating => record
            .parts
            .into_iter()
            .map(|part| (record.scene, part.event))
            .collect(),
        _ => Vec::new(),
    }
}

/// 上一回合有回覆（正文或中止的半截）還沒落檔：那句玩家發言已經有人回了，不能當「沒回成」收掉。
pub fn has_unlanded_reply(tx: &CommitTx<'_>) -> bool {
    turn::turn(tx)
        .is_some_and(|record| record.phase != Phase::Generating && !record.parts.is_empty())
}

/// 回合交接（角色回合與 GM 回合開始）：上一回合還在生成中回 busy；提交了（或中止留下半截）卻沒落檔的
/// 部分先由後端代落。
pub fn settle_previous_turn(tx: &CommitTx<'_>) -> DataResult<()> {
    if turn::turn(tx).is_some_and(|record| record.phase == Phase::Generating) {
        return Err(crate::ui_msg::UiMsg::WorldBusy.into_error());
    }
    land_pending_parts(tx)
}

/// 新事件追加前（玩家句、角色台詞、點名、復原……）：上一回合沒落成的部分先代落，順序才不會變成
/// 「新事件→舊 GM 回覆」。回合自己的部分（帶冪等鍵）不在這裡交接：同回合的照鍵落，別回合的晚到重試
/// 只會命中已落的那則或被拒。
pub fn settle_before_append(tx: &CommitTx<'_>, appending: Option<&TurnKey>) -> DataResult<()> {
    if appending.is_some() {
        return Ok(());
    }
    land_pending_parts(tx)
}

/// 進行中回合的憑證：只有 `begin_turn` 發得出來。同一回合內部的追加（登場紀錄等）拿它走
/// `append_within_turn`，不觸發交接；其他新事件沒有憑證，一律先交接。
#[derive(Debug)]
pub struct TurnTicket {
    turn_id: String,
}

/// 憑證對應的回合還在生成中或等落檔（同回合的附屬追加不交接）。
pub fn turn_owns(tx: &CommitTx<'_>, ticket: &TurnTicket) -> bool {
    turn::turn(tx).is_some_and(|record| {
        record.turn_id == ticket.turn_id
            && matches!(record.phase, Phase::Generating | Phase::AwaitingAppend)
    })
}

/// GM 回合開始（鎖內）：先交接上一回合，記下這一回合的幕、世代與固定輸入。不在這裡啟用變數模式——
/// 回合失敗不該改變逐樓讀取的語意，啟用留到提交。
pub fn begin_turn(
    tx: &CommitTx<'_>,
    turn_id: &str,
    macros: Option<&Macros>,
) -> DataResult<TurnTicket> {
    settle_previous_turn(tx)?;
    let scene = read_state_cache(tx.root, tx.world_id)?.current_scene;
    let input = super::source::current_source(tx.root, tx.world_id, scene)?
        .map(|source| source.table().clone());
    turn::set_turn(
        tx,
        TurnRecord {
            turn_id: turn_id.to_owned(),
            phase: Phase::Generating,
            scene,
            generation: turn::generation(tx),
            input,
            macros: macros.cloned(),
            pending: None,
            parts: Vec::new(),
        },
    );
    Ok(TurnTicket {
        turn_id: turn_id.to_owned(),
    })
}

/// GM 回合中止或出錯（核對 turn_id）：還在生成中就改成已中止，不套狀態、不產生表。中止留下的半截正文
/// （`half`）記進紀錄：前端沒落成時照樣由下一筆新事件前的交接代落（不帶表）。
pub fn finish_turn(tx: &CommitTx<'_>, turn_id: &str, half: Option<PendingMain>) -> DataResult<()> {
    let parts = match half {
        Some(main) if !main.text.trim().is_empty() => vec![main_part(main)?],
        _ => Vec::new(),
    };
    turn::update_turn(tx, turn_id, |record| {
        if record.phase == Phase::Generating {
            record.phase = Phase::Aborted;
            record.pending = None;
            record.parts = parts;
        }
    });
    Ok(())
}

/// 回合正文之外的附屬部分（例如 `state_update` 變動紀錄），提交時與正文一起登記。
pub struct TurnSide {
    pub part: String,
    pub kind: crate::data::TranscriptKind,
    pub text: String,
    pub marker: Option<crate::data::EventMarker>,
}

/// 提交結果：`error` 有值＝狀態快取（樹模式下是權威）沒寫成；變數模式的新表已留在回合紀錄、照樣隨正文落檔。
pub struct GmCommit<R> {
    pub state: WorldState,
    pub result: R,
    pub error: Option<String>,
}

/// GM 回合提交（鎖內、核對 turn_id、幕與世代）：`apply` 套在固定輸入上（變數模式時拿到輸入的 stat_data，
/// 回傳的帶型別寫值照原型別合回新表）；樹模式開始的回合在這裡符合條件就
/// 啟用變數模式（以當下樹物化種子）。新表與正文先放進回合紀錄（轉「等落檔」），再寫狀態快取；
/// 快取寫失敗明確回報、不吞掉。turn_id、幕或世代不符、不在生成中回 None、不改狀態。
pub fn apply_gm_block<R>(
    tx: &CommitTx<'_>,
    turn_id: &str,
    main: PendingMain,
    apply: impl FnOnce(&mut WorldState, Option<&Json>) -> (R, Option<TypedBatch>),
    sides: impl FnOnce(&WorldState, &R) -> Vec<TurnSide>,
) -> DataResult<Option<GmCommit<R>>> {
    let Some(mut record) = turn::turn(tx).filter(|record| record.turn_id == turn_id) else {
        return Ok(None);
    };
    let scene = read_state_cache(tx.root, tx.world_id)?.current_scene;
    if record.phase != Phase::Generating
        || record.scene != scene
        || record.generation != turn::generation(tx)
    {
        return Ok(None);
    }
    if record.input.is_none() && ensure_active(tx, record.macros.as_ref())? {
        record.input = super::source::current_source(tx.root, tx.world_id, scene)?
            .map(|source| source.table().clone());
    }
    let mut state = read_state(tx.root, tx.world_id)?;
    if let Some(input) = &record.input {
        state.state.tree = stat_to_tree(input.get("stat_data"));
    }
    let before = state.state.tree.clone();
    let input_stat = record.input.as_ref().map(stat_of);
    let (result, typed) = apply(&mut state, input_stat.as_ref());
    let pending = record.input.as_ref().map(|input| {
        let rules = NewValues {
            types: &state.mechanism.value_types,
            keep_strings: false,
            clean: None,
        };
        let stat = merge_tree_change_typed(
            &stat_of(input),
            &before,
            &state.state.tree,
            &rules,
            typed.as_ref(),
        );
        let mut table = input.clone();
        derive_display(&mut table, stat);
        table
    });
    // 正文與附屬部分在同一次鎖內一起登記：放鎖前待落清單就是完整的，中間不會有交接把附屬部分漏掉
    let mut parts = vec![main_part(main)?];
    for side in sides(&state, &result) {
        if side.part == PART_MAIN || parts.iter().any(|item| item.part == side.part) {
            continue;
        }
        parts.push(PendingPart {
            event: turn_event(side.kind, side.text, None, false, side.marker)?,
            part: side.part,
        });
    }
    turn::update_turn(tx, turn_id, |stored| {
        stored.phase = Phase::AwaitingAppend;
        stored.input = record.input.clone();
        stored.pending = pending;
        stored.parts = parts;
    });
    let error = write_state(tx.root, tx.world_id, &state)
        .err()
        .map(|error| error.to_string());
    Ok(Some(GmCommit {
        state,
        result,
        error,
    }))
}

/// 回合落檔前的判斷。
pub enum TurnAppend {
    /// 同鍵已經落過（前端重試）：回那則，不再追加
    Existing(TranscriptEvent),
    /// 照常追加；`table` 有值＝這則是 main，掛上回合紀錄裡的表
    New { table: Option<Json> },
}

/// `(turn_id, turn_part)` 冪等：先查本幕逐字稿有沒有同鍵的事件（寫檔成功但回傳失敗的重試也認得出），
/// 再核對回合紀錄的 turn_id、幕與世代，不符回錯、不改狀態。main 已落檔卻找不到（被收回了）時不再追加，
/// 免得晚到的重試把收回的樓復活。
pub fn prepare_turn_append(tx: &CommitTx<'_>, scene: u64, key: &TurnKey) -> DataResult<TurnAppend> {
    if let Some((_, existing)) = find_event_rev(
        tx.root,
        tx.world_id,
        scene,
        Some(&key.turn_id),
        |_, head| head.turn_key.as_ref() == Some(key),
    )? {
        return Ok(TurnAppend::Existing(existing));
    }
    let record = turn::turn(tx)
        .filter(|record| {
            record.turn_id == key.turn_id
                && record.scene == scene
                && record.generation == turn::generation(tx)
        })
        .ok_or_else(|| invalid_data("turn-mismatch"))?;
    if key.part != PART_MAIN {
        return Ok(TurnAppend::New { table: None });
    }
    match record.phase {
        Phase::AwaitingAppend => Ok(TurnAppend::New {
            table: record.pending,
        }),
        Phase::Aborted => Ok(TurnAppend::New { table: None }),
        Phase::Appended => Err(invalid_data("turn-already-landed")),
        Phase::Generating => Err(invalid_data("turn-not-committed")),
    }
}

/// 一部分寫成就從待落清單拿掉；`main` 寫成就轉已落檔、解除 busy。
pub fn mark_turn_appended(tx: &CommitTx<'_>, key: &TurnKey) {
    turn::update_turn(tx, &key.turn_id, |record| {
        record.parts.retain(|item| item.part != key.part);
        if key.part == PART_MAIN && matches!(record.phase, Phase::AwaitingAppend | Phase::Aborted) {
            record.phase = Phase::Appended;
            record.pending = None;
        }
    });
}

/// 換幕、分岔、退幕：GM 回合生成中擋下；上一回合還沒落檔的部分先代落進它自己那一幕，再動幕。
pub fn refuse_during_turn(tx: &CommitTx<'_>) -> DataResult<()> {
    if turn::turn(tx).is_some_and(|record| record.phase == Phase::Generating) {
        return Err(crate::ui_msg::UiMsg::SceneChangeDuringTurn.into_error());
    }
    land_pending_parts(tx)
}

/// 開場的底：這一幕初始化來源的 stat_data（變數模式才有），上游 set 取帶型別舊值用。
pub fn opening_stat(tx: &CommitTx<'_>, scene: u64) -> DataResult<Option<Json>> {
    let control = read_control(tx.root, tx.world_id)?;
    let Some(vars) = control.active(scene) else {
        return Ok(None);
    };
    Ok(Some(stat_of(
        init_source(tx.root, tx.world_id, scene, vars)?.table(),
    )))
}

/// 開場：以初始化來源為底，把開場狀態塊套出來的改動做成新表（變數模式才有）；有帶型別的批次結果就以它為底。
pub fn opening_table(
    tx: &CommitTx<'_>,
    scene: u64,
    before: &Tree,
    after: &Tree,
    types: &BTreeMap<String, String>,
    typed: Option<&TypedBatch>,
) -> DataResult<Option<Json>> {
    let control = read_control(tx.root, tx.world_id)?;
    let Some(vars) = control.active(scene) else {
        return Ok(None);
    };
    let source = init_source(tx.root, tx.world_id, scene, vars)?;
    let rules = NewValues {
        types,
        keep_strings: false,
        clean: None,
    };
    let stat = merge_tree_change_typed(&stat_of(source.table()), before, after, &rules, typed);
    let mut table = source.table().clone();
    derive_display(&mut table, stat);
    Ok(Some(table))
}

/// 這一幕在變數模式下的 epoch（給新表的 `vars_epoch`）。
pub fn scene_epoch(tx: &CommitTx<'_>, scene: u64) -> DataResult<Option<String>> {
    Ok(read_control(tx.root, tx.world_id)?
        .active(scene)
        .map(|vars| vars.epoch.clone()))
}

/// 換幕 ①：新幕種子＝舊幕結束時的初始化來源（完整表）、新幕新 epoch，寫進控制檔。
/// 還沒發布的新幕資料可以被重試覆寫；不是變數模式什麼都不做。
pub fn scene_seed_for_next(
    tx: &CommitTx<'_>,
    old_scene: u64,
    new_scene: u64,
) -> DataResult<Option<SceneVars>> {
    let mut control = read_control(tx.root, tx.world_id)?;
    let Some(vars) = control.active(old_scene) else {
        return Ok(None);
    };
    let seed = init_source(tx.root, tx.world_id, old_scene, vars)?
        .table()
        .clone();
    let vars = SceneVars {
        epoch: new_token(),
        seed: VarsTable::from_json(&seed),
    };
    control.scenes.insert(new_scene.to_string(), vars.clone());
    write_control(tx, &control)?;
    Ok(Some(vars))
}

/// 分岔的規劃：來源幕的 epoch 與種子；新幕換新 epoch。不是變數模式（或來源幕沒有種子）回 None。
pub struct ForkPlan {
    pub source_epoch: String,
    pub vars: SceneVars,
}

pub fn scene_seed_for_fork(tx: &CommitTx<'_>, from_scene: u64) -> DataResult<Option<ForkPlan>> {
    let control = read_control(tx.root, tx.world_id)?;
    Ok(control.active(from_scene).map(|vars| ForkPlan {
        source_epoch: vars.epoch.clone(),
        vars: SceneVars {
            epoch: new_token(),
            seed: vars.seed.clone(),
        },
    }))
}

/// 分岔 ②：控制檔寫新幕種子。
pub fn publish_scene_seed(tx: &CommitTx<'_>, scene: u64, vars: SceneVars) -> DataResult<()> {
    let mut control = read_control(tx.root, tx.world_id)?;
    control.scenes.insert(scene.to_string(), vars);
    write_control(tx, &control)
}

/// 退幕 ②：切回父幕成功後才移除子幕種子；失敗只留下用不到的殘留。
pub fn drop_scene_seed(tx: &CommitTx<'_>, scene: u64) -> DataResult<()> {
    let mut control = read_control(tx.root, tx.world_id)?;
    if control.scenes.remove(&scene.to_string()).is_some() {
        write_control(tx, &control)?;
    }
    Ok(())
}
