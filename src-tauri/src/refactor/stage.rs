//! 重構判定的舞台卡（stage-card-in-speaker-list 包 3）：盤點 AI 列出的「其實是舞台的既有角色卡」名字，
//! 對回本桌的卡、套用時寫舞台旗標。產物只存名字（重構卡會跨桌套用，id 換一桌就對不上），
//! 盤點收結果與套用前各解析一次，同一套過濾。

use super::types::{RefactorOutcome, RefactorSelection};
use crate::data::{self, CharacterMeta, DataResult, StageChange, StageFlags};
use std::collections::BTreeSet;
use std::path::Path;

/// 名字對到的卡：名字前後空白不計、整桌（含玩家卡）只有一張叫這個名字，而且這張卡目前是可發言的
/// 一般角色——玩家卡、封存、自動隱藏、已是舞台的卡都不收。名字缺或重名一律跳過（罕見，不硬猜）。
pub(crate) fn resolve_stage_card<'a>(
    existing: &'a [CharacterMeta],
    player_name: Option<&str>,
    name: &str,
) -> Option<&'a CharacterMeta> {
    let name = name.trim();
    if name.is_empty() || player_name.is_some_and(|player| player.trim() == name) {
        return None;
    }
    let mut matches = existing.iter().filter(|meta| meta.name.trim() == name);
    let meta = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    (meta.cast_role() == data::CastRole::Speaker).then_some(meta)
}

/// 玩家卡名字（list_characters 不含玩家卡，重名檢查要另外算進去）；讀不到當沒有。
pub(crate) fn player_card_name(root: &Path, world_id: &str) -> Option<String> {
    data::read_player_card(root, world_id)
        .ok()
        .flatten()
        .map(|card| card.name)
}

/// 盤點收結果時只留本桌對得到的名字（去重、照 AI 列的順序）。
pub(crate) fn keep_resolvable(
    existing: &[CharacterMeta],
    player_name: Option<&str>,
    names: Vec<String>,
) -> Vec<String> {
    let mut seen = BTreeSet::new();
    names
        .into_iter()
        .map(|name| name.trim().to_owned())
        .filter(|name| resolve_stage_card(existing, player_name, name).is_some())
        .filter(|name| seen.insert(name.clone()))
        .collect()
}

/// 這次要標成舞台的卡 id：只有明確的 characters 玩法才套；existing 必須是套用寫入前的快照，
/// 這次新建的同名人物卡才對不到。
pub(super) fn stage_targets(
    existing: &[CharacterMeta],
    player_name: Option<&str>,
    outcome: &RefactorOutcome,
    selection: &RefactorSelection,
    mode: Option<&str>,
) -> Vec<String> {
    if mode != Some("characters") {
        return Vec::new();
    }
    let mut ids: Vec<String> = Vec::new();
    for &index in &selection.stage_indices {
        let Some(name) = outcome.stage_cards.get(index) else {
            continue;
        };
        if let Some(meta) = resolve_stage_card(existing, player_name, name) {
            if !ids.contains(&meta.id) {
                ids.push(meta.id.clone());
            }
        }
    }
    ids
}

/// 逐張寫舞台旗標，寫成一張就記一張（前後值），中途失敗時已寫的那幾張由收據回滾。
pub(super) fn write_stage_flags(
    root: &Path,
    world_id: &str,
    ids: &[String],
    changes: &mut Vec<StageChange>,
) -> DataResult<()> {
    for id in ids {
        let before = data::character_stage_flags(root, world_id, id)?;
        data::set_character_stage(root, world_id, id, true)?;
        changes.push(StageChange {
            character_id: id.clone(),
            before,
            after: StageFlags {
                stage: true,
                auto_hidden: false,
            },
        });
    }
    Ok(())
}
