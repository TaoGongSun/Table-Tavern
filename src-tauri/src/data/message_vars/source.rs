//! 初始化來源（MVU `getLastValidVariable`，計畫 8.2）：只看目前這一幕——依位置最新一則 `vars_epoch` 等於
//! 這一幕 epoch 的帶表事件；沒有就用控制檔裡這一幕的種子。逐樓 API 不走這裡。
use super::control::{read_control, SceneVars};
use super::convert::{stat_to_tree, Tree};
use super::json::Json;
use crate::data::scene::transcript_path;
use crate::data::world_file::with_file_lock;
use crate::data::DataResult;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// 這一幕第 `index` 則事件（逐字稿檔裡的位置）帶的表
    Event {
        index: usize,
        id: Option<String>,
        rev: Option<String>,
        table: Json,
    },
    /// 控制檔裡這一幕的種子
    Seed { table: Json },
}

impl Source {
    pub fn table(&self) -> &Json {
        match self {
            Source::Event { table, .. } | Source::Seed { table } => table,
        }
    }

    pub fn tree(&self) -> Tree {
        stat_to_tree(self.table().get("stat_data"))
    }
}

/// 從檔尾往前找，只解析到命中的那一行為止（輕量解析，不展開其他欄位）。
pub fn find_in_bytes(bytes: &[u8], epoch: &str) -> DataResult<Option<Source>> {
    let mut found = None;
    crate::data::scene::find_rev(bytes, Some(epoch), |index, head| {
        if head.message_vars.is_some() && head.vars_epoch.as_deref() == Some(epoch) {
            found = Some((index, head));
            true
        } else {
            false
        }
    })?;
    match found {
        Some((index, head)) => Ok(Some(Source::Event {
            index,
            id: head.id,
            rev: head.vars_rev,
            table: head.message_vars.expect("命中時有表").parse()?,
        })),
        None => Ok(None),
    }
}

pub fn init_source(
    root: &Path,
    world_id: &str,
    scene: u64,
    vars: &SceneVars,
) -> DataResult<Source> {
    let path = transcript_path(root, world_id, scene)?;
    let bytes = with_file_lock(&path, |file| file.read())?;
    if let Some(bytes) = bytes {
        if let Some(found) = find_in_bytes(&bytes, &vars.epoch)? {
            return Ok(found);
        }
    }
    Ok(Source::Seed {
        table: vars.seed.parse()?,
    })
}

/// 目前這一幕在變數模式下的初始化來源；不是變數模式（或這一幕還沒有種子）回 None。
pub fn current_source(root: &Path, world_id: &str, scene: u64) -> DataResult<Option<Source>> {
    let control = read_control(root, world_id)?;
    match control.active(scene) {
        Some(vars) => Ok(Some(init_source(root, world_id, scene, vars)?)),
        None => Ok(None),
    }
}

/// `read_state` 的投影：變數模式時回有效 stat_data 的狀態樹投影。
pub fn projected_tree(root: &Path, world_id: &str, scene: u64) -> DataResult<Option<Tree>> {
    Ok(current_source(root, world_id, scene)?.map(|source| source.tree()))
}
