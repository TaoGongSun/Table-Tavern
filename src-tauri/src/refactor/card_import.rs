//! 重構卡匯入：玩家選的檔（信任邊界）嗅探分流＋驗證，PNG 附的角色圖暫存在記憶體單槽，
//! 等套用時由 refactor_apply 憑 world＋token 取走。
//!
//! 單槽規則：開新卡覆蓋舊槽（舊 token 失效）；釋放要 world＋token 都相符；套用在取整桌鎖之前
//! 就把素材取走、由那次呼叫持有到結束，之後關卡、開新卡、換桌都碰不到它。取走後在寫入任何
//! 東西之前就被拒套（玩家卡已存在、介面 preflight、名字不符）時放回——只在槽是空的時候，
//! 槽裡已有較新的卡就丟棄，回 RefactorAssetsGone 讓前端提示重新開檔。

use super::card_file::{parse_card, RefactorCardFile};
use super::card_png::{decode, AssetKind, CardAsset, CARD_LIMITS, MANIFEST_CHUNK};
use super::types::RefactorOutcome;
use crate::data::DataResult;
use crate::import::card_io::PNG_MAGIC;
use crate::ui_msg::UiMsg;
use serde::Serialize;
use std::sync::Mutex;

/// 嗅探：有沒有 `ttRd` chunk（只走結構不驗 CRC，分流用；真驗證在 decode）。
pub fn has_manifest_chunk(bytes: &[u8]) -> bool {
    if !bytes.starts_with(PNG_MAGIC) {
        return false;
    }
    let mut offset = PNG_MAGIC.len();
    while bytes.len().saturating_sub(offset) >= 12 {
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        if &bytes[offset + 4..offset + 8] == MANIFEST_CHUNK {
            return true;
        }
        match offset
            .checked_add(12)
            .and_then(|end| end.checked_add(length))
        {
            Some(end) if end <= bytes.len() => offset = end,
            _ => return false,
        }
    }
    false
}

pub struct OpenedCard {
    pub card: RefactorCardFile,
    pub assets: Vec<CardAsset>,
}

/// 依內容分流：PNG 有 ttRd → 自家重構卡（整包原子驗證）；PNG 沒有 → 這是 ST 角色卡或一般圖片，
/// 回 RefactorCardIsCharacter；非 PNG → 當 JSON 封套／舊裸產物。
pub fn open_card(bytes: &[u8]) -> DataResult<OpenedCard> {
    if bytes.starts_with(PNG_MAGIC) {
        if !has_manifest_chunk(bytes) {
            return Err(UiMsg::RefactorCardIsCharacter.into_error());
        }
        let decoded = decode(bytes)?;
        return Ok(OpenedCard {
            card: decoded.card,
            assets: decoded.assets,
        });
    }
    if bytes.len() > CARD_LIMITS.manifest {
        return Err(UiMsg::RefactorCardInvalid {
            detail: "file too large".to_owned(),
        }
        .into_error());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| {
        UiMsg::RefactorCardInvalid {
            detail: "not UTF-8 text".to_owned(),
        }
        .into_error()
    })?;
    Ok(OpenedCard {
        card: parse_card(text)?,
        assets: Vec::new(),
    })
}

/// 暫存的素材：角色名清單用來在套用時確認前端送來的 outcome 還是同一份。
pub struct Staged {
    pub world_id: String,
    pub token: String,
    pub names: Vec<String>,
    pub assets: Vec<CardAsset>,
}

static SLOT: Mutex<Option<Staged>> = Mutex::new(None);

fn slot() -> std::sync::MutexGuard<'static, Option<Staged>> {
    SLOT.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 開卡結果的素材摘要（前端摘要行用；圖片本體不過 IPC）。
#[derive(Debug, Clone, Serialize)]
pub struct StagedAssetInfo {
    pub outcome_index: usize,
    pub kind: AssetKind,
}

/// 暫存素材並回新 token；覆蓋舊槽。沒有素材就不佔槽、回 None。
pub fn stage(world_id: &str, card: &RefactorCardFile, assets: Vec<CardAsset>) -> Option<String> {
    if assets.is_empty() {
        return None;
    }
    let token = crate::data::new_id();
    *slot() = Some(Staged {
        world_id: world_id.to_owned(),
        token: token.clone(),
        names: card
            .outcome
            .characters
            .iter()
            .map(|character| character.name.clone())
            .collect(),
        assets,
    });
    Some(token)
}

/// 結果卡關閉時釋放：world＋token 都相符才清，舊卡關閉清不到新卡。
pub fn release(world_id: &str, token: &str) {
    let mut guard = slot();
    if guard
        .as_ref()
        .is_some_and(|staged| staged.world_id == world_id && staged.token == token)
    {
        *guard = None;
    }
}

/// 套用取走：world＋token 相符才給，否則 RefactorAssetsGone（已被新卡覆蓋或已釋放）。
pub fn take(world_id: &str, token: &str) -> DataResult<Staged> {
    let mut guard = slot();
    match guard.take() {
        Some(staged) if staged.world_id == world_id && staged.token == token => Ok(staged),
        other => {
            *guard = other;
            Err(UiMsg::RefactorAssetsGone.into_error())
        }
    }
}

/// 拒套後放回：只在槽是空的時候；槽裡已有較新的卡就丟棄並回 false。
pub fn put_back(staged: Staged) -> bool {
    let mut guard = slot();
    if guard.is_some() {
        return false;
    }
    *guard = Some(staged);
    true
}

/// 前端送來套用的 outcome 角色名逐一相符才用這份素材。
pub fn matches_outcome(staged: &Staged, names: &[String]) -> bool {
    staged.names == names
}

/// refactor_card_open 的回傳：封套（serde 形狀，前端再驗）、圖片摘要、暫存 token（沒圖是 None）。
#[derive(Debug, Serialize)]
pub struct OpenCardResult {
    pub card: RefactorCardFile,
    pub assets: Vec<StagedAssetInfo>,
    pub token: Option<String>,
}

pub fn open_and_stage(world_id: &str, bytes: &[u8]) -> DataResult<OpenCardResult> {
    let opened = open_card(bytes)?;
    let assets = opened
        .assets
        .iter()
        .map(|asset| StagedAssetInfo {
            outcome_index: asset.outcome_index,
            kind: asset.kind,
        })
        .collect();
    let token = stage(world_id, &opened.card, opened.assets);
    Ok(OpenCardResult {
        card: opened.card,
        assets,
        token,
    })
}

pub fn release_assets(world_id: &str, token: &str) {
    release(world_id, token);
}

/// 套用前取走素材（取整桌鎖之前）：沒 token 回 None；角色名對不上就放回並拒套。
pub fn claim_assets(
    world_id: &str,
    token: Option<&str>,
    outcome: &RefactorOutcome,
) -> DataResult<Option<Staged>> {
    let Some(token) = token else {
        return Ok(None);
    };
    let staged = take(world_id, token)?;
    let names: Vec<String> = outcome
        .characters
        .iter()
        .map(|character| character.name.clone())
        .collect();
    if !matches_outcome(&staged, &names) {
        put_back(staged);
        return Err(UiMsg::RefactorCardInvalid {
            detail: "outcome does not match the opened card".to_owned(),
        }
        .into_error());
    }
    Ok(Some(staged))
}

/// 套用在寫入前被拒：素材放回槽讓玩家修正後重試；槽已被新卡佔走就丟棄並改回 RefactorAssetsGone。
pub fn return_assets(claimed: Option<Staged>, error: String) -> String {
    if let Some(staged) = claimed {
        if !put_back(staged) {
            return UiMsg::RefactorAssetsGone.to_string();
        }
    }
    error
}

#[cfg(test)]
pub(crate) fn clear_for_test() {
    *slot() = None;
}

#[cfg(test)]
mod tests;
