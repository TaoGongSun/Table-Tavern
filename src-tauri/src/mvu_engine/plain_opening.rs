//! 非 zod 桌開場白的 `<initvar>`（initvar-nonzod-overwrite）：照上游 initCheck，區塊有套上就整份取代樹物化的初始值。
//! 解析借共用核心（無 schema 的 runtime，同 zod 種子的 `Op::Seed`），鎖外先算；鎖內由
//! `message_vars::ensure_active_overridden` 以提交當下的表發布。沒套上或引擎失敗都照舊以樹物化的種子啟用。
use super::wiring::{json_of, shared, value_of, zod_table_checked};
use super::{FailureKind, Macros as EngineMacros, MvuEngine, SchemaScript, SeedBase};
use crate::data::message_vars::{self, Json, Macros};
use crate::data::state_commit::with_commit;
use crate::data::DataResult;
use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// 鎖外算好的結果
pub enum PlainOpening {
    /// 不必覆寫（zod 桌、沒有區塊、已啟用、不符合卡片變數模式、區塊都沒套上）
    Skip,
    /// 覆寫後的 stat_data（已清元資料）
    Override(Json),
    /// 引擎整次失敗的原因：照舊物化，記一筆帳
    Failed(String),
}

impl PlainOpening {
    pub fn stat(&self) -> Option<&Json> {
        match self {
            Self::Override(stat) => Some(stat),
            _ => None,
        }
    }

    pub fn failure(&self) -> Option<&str> {
        match self {
            Self::Failed(detail) => Some(detail),
            _ => None,
        }
    }
}

/// 開場白含 `<initvar>`（不分大小寫，同上游正則）、這一幕還沒啟用且符合卡片變數模式、確認不是 zod 桌，才啟動引擎。
pub fn prepare(
    root: &Path,
    world_id: &str,
    raw: &str,
    user_name: &str,
) -> DataResult<PlainOpening> {
    prepare_with(shared(), root, world_id, raw, user_name)
}

/// 引擎忙（全機名額滿或本桌重開達上限）時，在這次開場的同一個期限內隔一小段再試；不占資料鎖、等待時不握名額。
const BUSY_RETRY: Duration = Duration::from_millis(50);

pub fn prepare_with(
    engine: &MvuEngine,
    root: &Path,
    world_id: &str,
    raw: &str,
    user_name: &str,
) -> DataResult<PlainOpening> {
    let tagged = raw
        .as_bytes()
        .windows(b"<initvar>".len())
        .any(|window| window.eq_ignore_ascii_case(b"<initvar>"));
    if !tagged {
        return Ok(PlainOpening::Skip);
    }
    let given = Macros {
        user: user_name.to_owned(),
        char: None,
    };
    let Some((table, effective)) = with_commit(root, world_id, |tx| {
        message_vars::tree_seed_with_macros(tx, Some(&given))
    })?
    else {
        return Ok(PlainOpening::Skip);
    };
    match zod_table_checked(root, world_id) {
        Ok(None) => {}
        Ok(Some(_)) => return Ok(PlainOpening::Skip),
        // 判定失敗：開場照舊物化（同 zod 路徑把它當非 zod 桌），覆寫沒套用要記帳，不靜默丟掉
        Err(error) => {
            return Ok(PlainOpening::Failed(format!(
                "讀不到這桌的卡片判定：{error}"
            )))
        }
    }
    let stat = value_of(table.get("stat_data").unwrap_or(&Json::Null))?;
    let effective = effective.unwrap_or(given);
    let macros = EngineMacros {
        user: effective.user,
        char: effective.char,
    };
    let schema = Arc::new(SchemaScript::plain());
    let deadline = engine.deadline();
    loop {
        match engine.seed(
            world_id,
            &schema,
            SeedBase::Tree(&stat),
            raw,
            &macros,
            deadline,
        ) {
            Ok(output) if output.applied => {
                return Ok(PlainOpening::Override(json_of(&output.stat_data)?))
            }
            Ok(_) => return Ok(PlainOpening::Skip),
            Err(failure)
                if failure.kind == FailureKind::Busy && Instant::now() + BUSY_RETRY < deadline =>
            {
                thread::sleep(BUSY_RETRY);
            }
            Err(failure) => return Ok(PlainOpening::Failed(failure.detail)),
        }
    }
}
