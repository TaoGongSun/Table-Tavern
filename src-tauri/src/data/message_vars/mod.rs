//! 卡片變數（MVU message 層）只存一處：逐字稿每則事件存完整表，桌面有效狀態經投影入口取
//! （計畫第 8 節，包 2a）。
//! - `json`：保留鍵順序的 JSON 與寫入上限
//! - `convert`：狀態樹 ↔ 變數表
//! - `control`：模式與每幕種子的控制檔
//! - `source`：初始化來源與 `read_state` 投影
//! - `mode`：模式交接與快取重建
//! - `turn`：桌世代與 GM 回合紀錄
//! - `write`：初始化來源的改寫、卡寫、GM 回合與開場的新表
mod control;
mod convert;
mod json;
mod mode;
mod source;
mod turn;
mod write;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::value::RawValue;

pub use control::{control_path, read_control};
pub use convert::Macros;
pub use json::{parse_table, Json};
pub use mode::{ensure_active, handover_to_tree, new_token, refresh_cache};
pub use source::{projected_tree, Source};
pub use turn::{bump_generation, busy, generation, world_swapped, PendingMain, TurnKey};
pub use write::{
    apply_gm_block, begin_turn, card_write, drop_scene_seed, edit_effective_tree,
    edit_tree_if_events, finish_turn, has_unlanded_reply, mark_turn_appended, opening_table,
    prepare_turn_append, publish_scene_seed, refuse_during_turn, scene_epoch, scene_seed_for_fork,
    scene_seed_for_next, settle_before_append, settle_previous_turn, turn_owns, CardWrite,
    CardWriteTarget, GmCommit, TurnAppend, TurnSide, TurnTicket,
};

#[cfg(test)]
use control::Mode;
#[cfg(test)]
use mode::eligible_char;
#[cfg(test)]
use source::current_source;
#[cfg(test)]
use turn::{phases, Phase, PART_MAIN};

/// 事件上存的表：原始 JSON 文字照存（鍵順序、數字寫法都不動），讀逐字稿不必展開整棵表。
#[derive(Debug, Clone)]
pub struct VarsTable(Box<RawValue>);

impl VarsTable {
    pub fn from_json(value: &Json) -> Self {
        Self(RawValue::from_string(value.to_text()).expect("Json 序列化必然是合法 JSON"))
    }

    pub fn text(&self) -> &str {
        self.0.get()
    }

    pub fn parse(&self) -> crate::data::DataResult<Json> {
        json::parse(self.text()).map_err(crate::data::invalid_data)
    }
}

impl PartialEq for VarsTable {
    fn eq(&self, other: &Self) -> bool {
        self.text() == other.text()
    }
}

impl Eq for VarsTable {}

impl Serialize for VarsTable {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// 讀逐字稿時是原始 JSON 物件（照存不展開）；前端復原送回時是 JSON 文字（Tauri 參數經 serde Value 會把
/// 物件鍵排序，送文字才保得住原順序），這裡解開成同一張表。
impl<'de> Deserialize<'de> for VarsTable {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let raw = Box::<RawValue>::deserialize(deserializer)?;
        if !raw.get().starts_with('"') {
            return Ok(Self(raw));
        }
        let text: String = serde_json::from_str(raw.get()).map_err(D::Error::custom)?;
        let table = json::parse(&text).map_err(D::Error::custom)?;
        if !matches!(table, Json::Object(_)) {
            return Err(D::Error::custom("message_vars must be an object"));
        }
        Ok(Self::from_json(&table))
    }
}

#[cfg(test)]
mod tests;
