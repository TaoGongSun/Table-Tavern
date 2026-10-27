//! 桌面接線（mvu-zod-semantics 規格 2.4、2.5 節）：zod 桌判定、保序種子、GM 回合的三段提交（短鎖固定輸入 →
//! 鎖外跑引擎與後處理 → 短鎖重驗才寫）、開場白同一條路。整次失敗時狀態整批不寫、正文照落。
use super::{
    EngineFailure, FailureKind, Limits, Macros as EngineMacros, MvuEngine, SchemaScript, SeedBase,
};
use crate::data::message_vars::{self, Json, Macros, PendingMain, TurnSide};
use crate::data::state_commit::{with_commit, CommitTx};
use crate::data::{self, DataResult, WorldState};
use crate::mechanism::engine_records::{
    engine_changes, engine_notice, engine_records, finish_engine, state_block_ignored,
};
use crate::mechanism::{Outcome, Record};
use crate::transport::StateBlock;
use crate::ui_msg::UiMsg;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

/// 全 app 一份引擎（每桌一條執行緒、全機名額 2）
pub fn shared() -> &'static MvuEngine {
    static ENGINE: OnceLock<MvuEngine> = OnceLock::new();
    ENGINE.get_or_init(|| MvuEngine::new(Limits::default()))
}

/// zod 桌：沒重構、而且有卡帶 zod 變數 schema（規格 2.5 節）
#[derive(Clone)]
pub struct ZodTable {
    pub schema: Arc<SchemaScript>,
    pub initvars: Vec<String>,
    /// 帶 schema 的卡名（`{{char}}` 代換）
    pub char_name: String,
    /// 第二張以後也帶 schema 的卡：不執行，明白告知玩家（規格 1.5 節）
    pub skipped: Vec<String>,
}

/// 這桌是不是 zod 桌：重構過、角色優先、讀不到卡都不是；帶 schema 的卡取最先匯入的那張（規格 1.5 節）。
/// 判定失敗（讀不到 state、卡清單、重構標記）也當不是；要分清楚的呼叫端用 `zod_table_checked`。
pub fn zod_table(root: &Path, world_id: &str) -> Option<ZodTable> {
    zod_table_checked(root, world_id).ok().flatten()
}

/// 同 `zod_table`，但分得出「確認不是 zod 桌」（`Ok(None)`）與「判定失敗」（`Err`）。
/// 匯入順序看角色 id（ULID，匯入當下配發、拖曳排序不動它）；世界層原卡（建桌時匯入）排最前。
pub fn zod_table_checked(root: &Path, world_id: &str) -> DataResult<Option<ZodTable>> {
    let state = data::read_state(root, world_id)?;
    if crate::refactor::refactored(root, world_id, &state)? {
        return Ok(None);
    }
    let mut interfaces: Vec<_> = crate::import::read_card_interfaces(root, world_id)?
        .into_iter()
        .filter(|card| card.unsupported.is_none())
        .collect();
    interfaces.sort_by(|left, right| left.character_id.cmp(&right.character_id));
    let mut cards = interfaces
        .into_iter()
        .filter_map(|card| card.zod.map(|zod| (card.character_name, zod)));
    let Some((char_name, zod)) = cards.next() else {
        return Ok(None);
    };
    Ok(Some(ZodTable {
        schema: Arc::new(SchemaScript::new(zod.source)),
        initvars: zod.initvars,
        char_name,
        skipped: cards.map(|(name, _)| name).collect(),
    }))
}

/// 卡片前端自己呼叫 `Mvu.parseMessage`（card-shim-no-zod）：zod 桌交同一台引擎只算不寫，回結果表的 JSON 文字
/// （保留鍵序）；確認不是 zod 桌回 `Ok(None)`（沙盒照舊自己跑）。判定失敗、整次失敗都回 `Err`（沙盒 reject，不退回
/// MagVarUpdate 語意）；錯誤文字開頭是失敗種類。
pub fn card_parse(
    root: &Path,
    world_id: &str,
    message: &str,
    old_json: &str,
    macros: &Macros,
) -> Result<Option<String>, String> {
    let zod = zod_table_checked(root, world_id).map_err(|error| format!("detect: {error}"))?;
    let Some(zod) = zod else {
        return Ok(None);
    };
    let old = match serde_json::from_str::<Value>(old_json) {
        Ok(Value::Object(map)) => map,
        _ => return Err("bad-input: old 必須是物件".to_owned()),
    };
    let engine = shared();
    let batch = engine
        .run_batch(
            world_id,
            &zod.schema,
            message,
            &old,
            &engine_macros(macros, &zod),
            engine.deadline(),
        )
        .map_err(|failure| format!("{:?}: {}", failure.kind, failure.detail))?;
    serde_json::to_string(&batch.detailed.table)
        .map(Some)
        .map_err(|error| format!("bad-output: {error}"))
}

fn engine_macros(macros: &Macros, zod: &ZodTable) -> EngineMacros {
    EngineMacros {
        user: macros.user.clone(),
        char: macros
            .char
            .clone()
            .or_else(|| (!zod.char_name.is_empty()).then(|| zod.char_name.clone())),
    }
}

/// 整次失敗給玩家的那句（UiMsg，前端照語系翻成一句白話）
pub fn failure_message(failure: &EngineFailure) -> String {
    let message = match &failure.kind {
        FailureKind::Busy => UiMsg::MvuEngineBusy,
        FailureKind::Deadline => UiMsg::MvuUpdateFailedTimeout,
        FailureKind::Memory => UiMsg::MvuUpdateFailedMemory,
        FailureKind::Stack => UiMsg::MvuUpdateFailedStack,
        FailureKind::SchemaLoad => UiMsg::MvuUpdateFailedSchemaLoad,
        FailureKind::InputTooLarge => UiMsg::MvuUpdateFailedTooLarge,
        FailureKind::Script { code: Some(code) } => match code.as_str() {
            "deadline" | "eval-timeout" => UiMsg::MvuUpdateFailedTimeout,
            "too-many-commands" => UiMsg::MvuUpdateFailedTooManyCommands,
            "message-too-large" | "too-many-children" | "too-deep" => {
                UiMsg::MvuUpdateFailedTooLarge
            }
            _ => UiMsg::MvuUpdateFailedEngine,
        },
        FailureKind::Script { code: None } | FailureKind::Crashed | FailureKind::BadOutput => {
            UiMsg::MvuUpdateFailedEngine
        }
    };
    message.to_string()
}

/// 建種子失敗給玩家的那句：initvar 讀不懂（引擎丟的腳本錯）單獨一句，其餘同一般失敗
fn seed_failure_message(failure: &EngineFailure) -> String {
    match failure.kind {
        FailureKind::Script { code: None } => UiMsg::MvuUpdateFailedInitvar.to_string(),
        _ => failure_message(failure),
    }
}

fn json_of(value: &Value) -> DataResult<Json> {
    message_vars::parse_json(&value.to_string()).map_err(crate::data::invalid_data)
}

fn value_of(json: &Json) -> DataResult<Value> {
    serde_json::from_str(&json.to_text())
        .map_err(|error| crate::data::invalid_data(error.to_string()))
}

/// 種子的結果：這一幕已啟用（不必建）、建好的表、或建不成（整次失敗，不改從樹物化）。
pub(super) enum Seeded {
    NotNeeded,
    Table(Json),
    /// 給玩家的那句（UiMsg）
    Failed(String),
}

/// 種子（鎖外）：這一幕還沒啟用變數模式才建，照上游 initCheck（規格 2.4a）：卡上各條 `[initvar]` 在引擎裡保序
/// 解析合併（卡上沒有 initvar 就以狀態樹物化的表為底）→ 開場白的 `<initvar>` 整份取代 → cleanUpMetadata → 過 schema。
/// 引擎失敗（initvar 讀不懂、觸限…）＝整次失敗；開局沒過 schema 照原值開局並記一筆。第二張以後的 schema 卡也在這裡告知一次。
#[allow(clippy::too_many_arguments)]
pub(super) fn seed(
    root: &Path,
    world_id: &str,
    zod: &ZodTable,
    macros: &Macros,
    opening: &str,
    deadline: Instant,
    records: &mut Vec<Record>,
) -> DataResult<Seeded> {
    if !with_commit(root, world_id, message_vars::needs_seed)? {
        return Ok(Seeded::NotNeeded);
    }
    for name in &zod.skipped {
        records.push(engine_notice(
            name,
            format!("{name} 也帶變數 schema，同一桌只用最先匯入那張卡的，這張的 schema 沒有執行。"),
        ));
    }
    // 卡上沒有 initvar：以樹物化的表為底（沿用上一份表的其他鍵），stat_data 換成過了 schema 的
    let (mut table, tree_stat) = if zod.initvars.is_empty() {
        let Some(table) = with_commit(root, world_id, |tx| {
            message_vars::tree_seed(tx, Some(macros))
        })?
        else {
            return Ok(Seeded::NotNeeded);
        };
        let stat = value_of(table.get("stat_data").unwrap_or(&Json::Null))?;
        (table, Some(stat))
    } else {
        (Json::empty_object(), None)
    };
    let base = match &tree_stat {
        Some(stat) => SeedBase::Tree(stat),
        None => SeedBase::Initvars(&zod.initvars),
    };
    let output = match shared().seed(
        world_id,
        &zod.schema,
        base,
        opening,
        &engine_macros(macros, zod),
        deadline,
    ) {
        Ok(output) => output,
        Err(failure) => {
            records.push(engine_notice(
                "開局",
                format!("開局變數沒能建立，這次變數沒有更新：{}", failure.detail),
            ));
            let message = seed_failure_message(&failure);
            return Ok(Seeded::Failed(message));
        }
    };
    if output.duplicates > 0 {
        records.push(engine_notice(
            "開局",
            format!(
                "卡片 schema 腳本重複註冊了 {} 次，只用第一次註冊的 schema。",
                output.duplicates
            ),
        ));
    }
    if let Some((issues, count)) = &output.invalid {
        records.push(engine_notice(
            "開局",
            format!(
                "開局變數沒過卡片 schema，照原值開局（共 {count} 則）：{}",
                issues.join("；")
            ),
        ));
    }
    message_vars::derive_display(&mut table, json_of(&output.stat_data)?);
    Ok(Seeded::Table(table))
}

/// 段 2 的結果：新表（None＝整次失敗）、後處理後的狀態、記帳；失敗時給玩家的那句。
pub struct EngineRun {
    pub pending: Option<Json>,
    pub world: WorldState,
    pub outcome: Outcome,
    pub failure: Option<String>,
}

/// 整次失敗的段 2 結果：狀態照讀到的不動、只帶回饋，新表 None。
fn failed_run(
    mut world: WorldState,
    user_name: &str,
    records: Vec<Record>,
    message: String,
) -> EngineRun {
    let outcome = finish_engine(&mut world, user_name, records, BTreeMap::new(), false);
    EngineRun {
        pending: None,
        world,
        outcome,
        failure: Some(message),
    }
}

/// 段 2（鎖外）：引擎跑整則訊息 → 逐條結果記帳 → 本地後處理（重擲、觸發、衍生）。後處理改到樹就把結果再送引擎
/// 過一次 schema（共用同一個期限）：沒過、或 transform 又改了值（巢狀剝鍵也算），整組後處理退回引擎原表，
/// 觸發、旗標與記錄一併撤回。
#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    root: &Path,
    world_id: &str,
    zod: &ZodTable,
    input: &Json,
    text: &str,
    block: &StateBlock,
    user_name: &str,
    macros: &Macros,
    deadline: Instant,
    mut records: Vec<Record>,
) -> DataResult<EngineRun> {
    let mut world = data::read_state(root, world_id)?;
    records.extend(state_block_ignored(block));
    let failed = |world: WorldState, mut records: Vec<Record>, failure: EngineFailure| {
        records.push(engine_notice(
            "變數更新",
            format!("變數更新失敗，狀態沒有寫入：{}", failure.detail),
        ));
        failed_run(world, user_name, records, failure_message(&failure))
    };
    let old: Map<String, Value> = match value_of(input)? {
        Value::Object(map) => map,
        _ => return Err(crate::data::invalid_data("mvu-engine-input-not-object")),
    };
    let engine = shared();
    let batch = match engine.run_batch(
        world_id,
        &zod.schema,
        text,
        &old,
        &engine_macros(macros, zod),
        deadline,
    ) {
        Ok(batch) => batch,
        Err(failure) => return Ok(failed(world, records, failure)),
    };
    records.extend(engine_records(&batch.detailed.results, &batch.feedback));
    let changes = engine_changes(&batch.detailed.changes);
    let table = json_of(&Value::Object(batch.detailed.table))?;
    world.state.tree = message_vars::table_tree(&table);
    let before = world.state.tree.clone();
    let mut post = world.clone();
    let outcome = finish_engine(&mut post, user_name, records.clone(), changes.clone(), true);
    if post.state.tree == before {
        return Ok(EngineRun {
            pending: Some(table),
            world: post,
            outcome,
            failure: None,
        });
    }
    let candidate = message_vars::merge_post_process(
        &table,
        &before,
        &post.state.tree,
        &post.mechanism.value_types,
    );
    let candidate_stat = value_of(candidate.get("stat_data").unwrap_or(&Json::Null))?;
    match engine.initialize(world_id, &zod.schema, &candidate_stat, deadline) {
        Ok(verdict) if verdict.invalid.is_none() && verdict.stat_data == candidate_stat => {
            Ok(EngineRun {
                pending: Some(candidate),
                world: post,
                outcome,
                failure: None,
            })
        }
        Ok(_) => {
            records.push(engine_notice(
                "後處理",
                "重擲、觸發或衍生改出的值沒過卡片 schema（或被 schema 的 transform 改動），這輪後處理整組退回，變數照引擎結果。"
                    .to_owned(),
            ));
            let outcome = finish_engine(&mut world, user_name, records, changes, false);
            Ok(EngineRun {
                pending: Some(table),
                world,
                outcome,
                failure: None,
            })
        }
        Err(failure) => Ok(failed(world, records, failure)),
    }
}

/// 段 3 用：把段 2 算好的樹與回饋放進鎖內讀到的最新狀態（其他欄位以最新為準）。
fn adopt(state: &mut WorldState, run: &EngineRun) {
    state.state.tree = run.world.state.tree.clone();
    state.state.triggers = run.world.state.triggers.clone();
    state.state.notes = run.world.state.notes.clone();
    state.state.changes = run.world.state.changes.clone();
}

/// GM 回合的 zod 路徑（chat.rs 的 gm_narrate 呼叫）：回傳同 `TurnGuard::commit`——回合不符或結果過期回
/// `Ok(None)`；整次失敗時正文照常登記、狀態不寫，`error` 帶給玩家的那句。
#[allow(clippy::too_many_arguments)]
pub async fn gm_commit(
    root: &Path,
    world_id: &str,
    turn_id: &str,
    zod: ZodTable,
    main: PendingMain,
    reply: &str,
    block: &StateBlock,
    user_name: &str,
    macros: Macros,
    sides: impl FnOnce(&WorldState, &Option<Outcome>) -> Vec<TurnSide>,
) -> Result<Option<message_vars::GmCommit<Option<Outcome>>>, String> {
    let deadline = shared().deadline();
    let schema = zod.schema.hash;
    let (owned_root, owned_world, owned_turn) =
        (root.to_path_buf(), world_id.to_owned(), turn_id.to_owned());
    let (text, owned_block, owned_user) = (reply.to_owned(), block.clone(), user_name.to_owned());
    // 段 0（鎖外建種子）＋段 1（鎖內固定輸入）＋段 2（鎖外跑引擎）：引擎同步等待，放到 blocking 執行緒
    let prepared = tokio::task::spawn_blocking(move || {
        prepare_turn(
            &owned_root,
            &owned_world,
            &owned_turn,
            &zod,
            &text,
            &owned_block,
            &owned_user,
            &macros,
            deadline,
        )
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())?;
    let Some((input, run)) = prepared else {
        return Ok(None);
    };
    commit_turn(root, world_id, turn_id, schema, input, run, main, sides)
        .map_err(|error| error.to_string())
}

/// 段 0–2：建種子（失敗＝整次失敗，不走段 1、不物化）→ 段 1 固定輸入 → 段 2 跑引擎與後處理。
/// 回（段 1 的輸入，段 2 結果）；回合不符回 None。
#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_turn(
    root: &Path,
    world_id: &str,
    turn_id: &str,
    zod: &ZodTable,
    text: &str,
    block: &StateBlock,
    user_name: &str,
    macros: &Macros,
    deadline: Instant,
) -> DataResult<Option<(Option<Json>, EngineRun)>> {
    let mut records = Vec::new();
    let seeded = match seed(root, world_id, zod, macros, "", deadline, &mut records)? {
        Seeded::Failed(message) => {
            let world = data::read_state(root, world_id)?;
            return Ok(Some((None, failed_run(world, user_name, records, message))));
        }
        Seeded::Table(table) => Some(table),
        Seeded::NotNeeded => None,
    };
    let input = with_commit(root, world_id, |tx| {
        message_vars::engine_turn_input(tx, turn_id, seeded.as_ref())
    })?;
    let Some(input) = input else {
        return Ok(None);
    };
    let run = run(
        root, world_id, zod, &input, text, block, user_name, macros, deadline, records,
    )?;
    Ok(Some((Some(input), run)))
}

/// 這桌目前生效的 schema 憑證（雜湊）；不再是 zod 桌回 None
fn current_schema(root: &Path, world_id: &str) -> Option<[u8; 32]> {
    zod_table(root, world_id).map(|zod| zod.schema.hash)
}

/// 段 3（鎖內）：生效的 schema 仍是段 0–2 用的那份、回合與輸入也對得上才寫，否則回 None（過期結果不提交）。
#[allow(clippy::too_many_arguments)]
pub(super) fn commit_turn(
    root: &Path,
    world_id: &str,
    turn_id: &str,
    schema: [u8; 32],
    input: Option<Json>,
    run: EngineRun,
    main: PendingMain,
    sides: impl FnOnce(&WorldState, &Option<Outcome>) -> Vec<TurnSide>,
) -> DataResult<Option<message_vars::GmCommit<Option<Outcome>>>> {
    let committed = with_commit(root, world_id, |tx| {
        if current_schema(root, world_id) != Some(schema) {
            return Ok(None);
        }
        message_vars::apply_engine_turn(
            tx,
            turn_id,
            input.as_ref(),
            main,
            |state| {
                adopt(state, &run);
                (Some(run.outcome.clone()), run.pending.clone())
            },
            sides,
        )
    })?;
    Ok(committed.map(|mut commit| {
        if commit.error.is_none() {
            commit.error = run.failure.clone();
        }
        commit
    }))
}

/// 開場白的 zod 路徑，鎖外先做好：建種子（開場白的 `<initvar>` 整份取代）、啟用變數模式、跑引擎與後處理。
/// 不是 zod 桌、或啟用不了變數模式回 None（照舊走一般路徑）。憑證：段 1 的輸入、桌世代、schema 雜湊。
pub struct OpeningRun {
    /// None＝建種子失敗（沒有啟用變數模式）
    input: Option<Json>,
    generation: u64,
    schema: [u8; 32],
    run: EngineRun,
}

pub fn prepare_opening(
    root: &Path,
    world_id: &str,
    raw: &str,
    block: &StateBlock,
    user_name: &str,
) -> DataResult<Option<OpeningRun>> {
    let Some(zod) = zod_table(root, world_id) else {
        return Ok(None);
    };
    let deadline = shared().deadline();
    let macros = Macros {
        user: user_name.to_owned(),
        char: None,
    };
    let mut records = Vec::new();
    let seeded = match seed(root, world_id, &zod, &macros, raw, deadline, &mut records)? {
        Seeded::Failed(message) => {
            let (world, generation) = with_commit(root, world_id, |tx| -> DataResult<_> {
                message_vars::settle_before_append(tx, None)?;
                Ok((
                    data::read_state(root, world_id)?,
                    message_vars::generation(tx),
                ))
            })?;
            return Ok(Some(OpeningRun {
                input: None,
                generation,
                schema: zod.schema.hash,
                run: failed_run(world, user_name, records, message),
            }));
        }
        Seeded::Table(table) => Some(table),
        Seeded::NotNeeded => None,
    };
    let activated = with_commit(root, world_id, |tx| -> DataResult<_> {
        message_vars::settle_before_append(tx, None)?;
        let input = message_vars::activate_for_opening(tx, &macros, seeded.as_ref())?;
        Ok(input.map(|input| (input, message_vars::generation(tx))))
    })?;
    let Some((input, generation)) = activated else {
        return Ok(None);
    };
    let run = run(
        root, world_id, &zod, &input, raw, block, user_name, &macros, deadline, records,
    )?;
    Ok(Some(OpeningRun {
        input: Some(input),
        generation,
        schema: zod.schema.hash,
        run,
    }))
}

/// 開場白鎖內：憑證（初始化來源、桌世代、生效 schema）都還是鎖外那份才用算好的結果；任一換掉（中途有別的寫入）
/// 算整次失敗，正文照落、不帶表。建種子失敗的那份照樣只落正文，這一幕不啟用變數模式（下一輪再建）。
pub fn opening_result(
    tx: &CommitTx<'_>,
    prepared: &OpeningRun,
    world: &mut WorldState,
    user_name: &str,
) -> DataResult<(Option<Json>, Outcome)> {
    let current = message_vars::active_table(tx)?;
    let valid = current == prepared.input
        && message_vars::generation(tx) == prepared.generation
        && current_schema(tx.root, tx.world_id) == Some(prepared.schema);
    if !valid {
        let records = vec![engine_notice(
            "變數更新",
            "變數更新失敗，狀態沒有寫入：開場白處理途中變數來源、桌或卡片 schema 被換掉".to_owned(),
        )];
        return Ok((
            None,
            finish_engine(world, user_name, records, BTreeMap::new(), false),
        ));
    }
    if prepared.run.pending.is_some() {
        adopt(world, &prepared.run);
    }
    Ok((prepared.run.pending.clone(), prepared.run.outcome.clone()))
}
