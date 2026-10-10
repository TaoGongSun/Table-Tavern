//! 一桌的世界書（掃描用）：原始條目照 `fromWorldFile` 讀成 `WiEntry`、配上精簡檢視（標題、可見度、人物），
//! 外加 P9 設定與帶名字掃描要的原卡名。

use super::{Placed, Viewer};
use crate::data::{self, Visibility, WorldbookEntry};
use crate::import::CardInterface;
use crate::world_info::entry::{from_world_file, position, WiEntry};
use crate::world_info::settings::{WiSettings, MVU_WI_SETTINGS, ST_WI_SETTINGS};
use crate::world_info::sort::sort_entries;
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Clone)]
struct BookEntry {
    view: WorldbookEntry,
    wi: WiEntry,
}

#[derive(Clone, Default)]
pub struct TableBook {
    /// uid 遞增（ST 載入順序：物件形整數鍵遞增）
    entries: Vec<BookEntry>,
    /// 桌上任一張卡（含世界書路的原卡）載 MVU（P9 的 GM 視角）
    mvu_any: bool,
    /// 載 MVU 的角色卡 id
    mvu_cards: BTreeSet<String>,
    /// 世界書路的原卡名（桌上存了原卡介面檔才有）
    world_card: Option<String>,
}

/// 「載 MVU」＝卡片介面 `mvu` 為真且 `unsupported` 為空（同網頁版 `play-card.ts:78`）。
fn loads_mvu(interface: &CardInterface) -> bool {
    interface.mvu && interface.unsupported.is_none()
}

impl TableBook {
    /// 讀整本書與卡片介面。介面是盡力而為（讀不到當沒有 MVU、沒有原卡名），書讀不了回錯。
    pub fn load(root: &Path, world_id: &str) -> Result<Self, String> {
        let entries =
            data::read_worldbook_scan_entries(root, world_id).map_err(|error| error.to_string())?;
        let interfaces = crate::import::read_card_interfaces(root, world_id).unwrap_or_default();
        Ok(Self::from_parts(entries, &interfaces))
    }

    pub fn from_parts(
        entries: Vec<(WorldbookEntry, Map<String, Value>)>,
        interfaces: &[CardInterface],
    ) -> Self {
        let entries = entries
            .into_iter()
            .map(|(view, raw)| {
                let mut wi = from_world_file(&raw);
                wi.id = view.uid.to_string();
                wi.limited = matches!(view.visibility, Visibility::Characters(_));
                BookEntry { view, wi }
            })
            .collect();
        Self {
            entries,
            mvu_any: interfaces.iter().any(loads_mvu),
            mvu_cards: interfaces
                .iter()
                .filter(|interface| !interface.character_id.is_empty() && loads_mvu(interface))
                .map(|interface| interface.character_id.clone())
                .collect(),
            world_card: interfaces
                .iter()
                .find(|interface| interface.character_id.is_empty())
                .map(|interface| interface.character_name.trim().to_owned())
                .filter(|name| !name.is_empty()),
        }
    }

    /// 測試用：精簡條目照編輯器新建的原始值展開。
    #[cfg(test)]
    pub fn from_views(entries: &[WorldbookEntry]) -> Self {
        let mut entries: Vec<_> = entries
            .iter()
            .map(|entry| {
                let mut raw = data::worldbook_entry_value(entry);
                raw["uid"] = entry.uid.into();
                let Value::Object(raw) = raw else {
                    unreachable!("條目原始值是物件")
                };
                (entry.clone(), raw)
            })
            .collect();
        entries.sort_by_key(|(entry, _)| entry.uid);
        Self::from_parts(entries, &[])
    }

    /// 測試用：直接給原始條目（ST 物件形欄位）。
    #[cfg(test)]
    pub fn from_raw(entries: Vec<(WorldbookEntry, Map<String, Value>)>) -> Self {
        Self::from_parts(entries, &[])
    }

    pub fn world_card_name(&self) -> Option<&str> {
        self.world_card.as_deref()
    }

    /// P9：角色視角看該卡、GM 視角看桌上任一張卡或世界書路的卡。
    pub(super) fn settings(&self, viewer: &Viewer<'_>) -> &'static WiSettings {
        let mvu = match viewer {
            Viewer::Gm => self.mvu_any,
            Viewer::Character(card) => self.mvu_cards.contains(&card.id),
        };
        match mvu {
            true => &MVU_WI_SETTINGS,
            false => &ST_WI_SETTINGS,
        }
    }

    fn visible(view: &WorldbookEntry, viewer: &Viewer<'_>) -> bool {
        match (viewer, &view.visibility) {
            (Viewer::Gm, _) => true,
            (Viewer::Character(_), Visibility::Gm) => false,
            (Viewer::Character(_), Visibility::Public) => true,
            (Viewer::Character(card), Visibility::Characters(ids)) => ids.contains(&card.id),
        }
    }

    /// 這個視角的條目池（先依可見度過濾再掃，預算、群組、遞迴都只在看得到的條目之間運作）。
    pub(super) fn pool(&self, viewer: &Viewer<'_>) -> Vec<WiEntry> {
        self.entries
            .iter()
            .filter(|entry| Self::visible(&entry.view, viewer))
            .map(|entry| entry.wi.clone())
            .collect()
    }

    pub(super) fn view(&self, id: &str) -> Option<&WorldbookEntry> {
        self.entries
            .iter()
            .find(|entry| entry.wi.id == id)
            .map(|entry| &entry.view)
    }

    /// 角色共線共用快照的靜態集合（三之 3）：啟用、constant、`Public`、穩定、位置是前／後／範例上下。
    /// 不看任何一位角色的掃描結果。
    fn snapshot_entries(&self) -> Vec<&BookEntry> {
        self.entries
            .iter()
            .filter(|entry| {
                !entry.wi.disable
                    && matches!(entry.view.visibility, Visibility::Public)
                    && super::placement::stable(&entry.wi, &entry.view.title)
                    && [
                        position::BEFORE,
                        position::AFTER,
                        position::EM_TOP,
                        position::EM_BOTTOM,
                    ]
                    .contains(&entry.wi.position)
            })
            .collect()
    }

    pub(super) fn snapshot_uids(&self) -> BTreeSet<u64> {
        self.snapshot_entries()
            .into_iter()
            .map(|entry| entry.view.uid)
            .collect()
    }

    /// 共用快照的條目（內文是原文，`prepare` 以中性脈絡代換），依放置前的排序。
    pub fn snapshot(&self) -> Vec<Placed> {
        let mut entries: Vec<WiEntry> = self
            .snapshot_entries()
            .into_iter()
            .map(|entry| entry.wi.clone())
            .collect();
        sort_entries(&mut entries);
        entries
            .iter()
            .filter_map(|wi| {
                let view = self.view(&wi.id)?;
                Some(Placed::new(view, wi, wi.content.clone(), false))
            })
            .collect()
    }
}
