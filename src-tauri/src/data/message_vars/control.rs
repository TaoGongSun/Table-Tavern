//! 卡片變數控制檔 `worlds/<id>/card-vars/control.json`：模式與每一幕種子的唯一權威（計畫 8.1），整檔原子寫。
use super::convert::Macros;
use super::VarsTable;
use crate::data::paths::world_dir;
use crate::data::state_commit::CommitTx;
use crate::data::DataResult;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// 狀態樹是權威（沒有 MVU 卡、重構接管、或還沒啟用）
    #[default]
    Tree,
    /// 卡片變數存在逐字稿事件上，狀態樹只是投影快取
    Events,
}

/// 一幕的 epoch 與種子（那一幕開頭的完整表）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneVars {
    pub epoch: String,
    pub seed: VarsTable,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Control {
    #[serde(default)]
    pub mode: Mode,
    /// 啟用當下的 `{{user}}`／`{{char}}` 代換值：之後匯入補值照同一份代換
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub macros: Option<Macros>,
    /// 幕號字串 → 那一幕的 epoch 與種子
    #[serde(default)]
    pub scenes: BTreeMap<String, SceneVars>,
}

impl Control {
    pub fn scene(&self, scene: u64) -> Option<&SceneVars> {
        self.scenes.get(&scene.to_string())
    }

    /// 變數模式、而且這一幕已有 epoch 與種子：這一幕的有效狀態從事件來。
    pub fn active(&self, scene: u64) -> Option<&SceneVars> {
        (self.mode == Mode::Events)
            .then(|| self.scene(scene))
            .flatten()
    }
}

pub fn control_path(root: &Path, world_id: &str) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?
        .join("card-vars")
        .join("control.json"))
}

/// 沒有控制檔＝從沒啟用過，等於 `Tree` 模式。檔案壞掉回錯（不能悄悄當成沒啟用，有效狀態會換掉）。
pub fn read_control(root: &Path, world_id: &str) -> DataResult<Control> {
    let path = control_path(root, world_id)?;
    match std::fs::read(&path) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Control::default()),
        Err(error) => Err(error.into()),
    }
}

pub fn write_control(tx: &CommitTx<'_>, control: &Control) -> DataResult<()> {
    let bytes = serde_json::to_vec(control)?;
    crate::data::world_file::commit_world_write_atomic(&control_path(tx.root, tx.world_id)?, &bytes)
}
