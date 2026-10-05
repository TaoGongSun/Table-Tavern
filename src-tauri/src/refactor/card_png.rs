//! 重構卡 PNG 封裝：自家私有 chunk `ttRd`（manifest）＋`ttAs`（圖片原始 bytes，可重複）。
//! 配對只認 asset_id，不依 chunk 順序；manifest 不含檔名欄位。匯入是信任邊界：decode()
//! 整包原子驗證（含封面本身的嚴格 PNG 驗證），任一項不過整包拒收。匯出在組 chunk 前先檢查
//! 上限，寫檔前再把產出走一次 decode()，不產出自己會拒收的卡。走訪全程串流，不建 chunk 表。

use super::card_file::{parse_card_value, RefactorCardFile};
use crate::data::DataResult;
use crate::import::card_io::{png_chunk, PNG_MAGIC};
use crate::import::png_image::{for_each_chunk, validate_png_image, ImageLimits, PngChunk};
use crate::ui_msg::UiMsg;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const MANIFEST_CHUNK: &[u8; 4] = b"ttRd";
pub const ASSET_CHUNK: &[u8; 4] = b"ttAs";
const MANIFEST_MAGIC: &[u8; 4] = b"TTRC";
const ASSET_MAGIC: &[u8; 4] = b"TTAS";
const PAYLOAD_VERSION: u8 = 1;

/// 匯入、匯出共用的上限。cover＝封面（自家 chunk 以外的全部 chunk）總長。
#[derive(Clone, Copy)]
pub struct CardLimits {
    pub file: usize,
    pub manifest: usize,
    pub image: usize,
    pub cover: usize,
    pub assets_total: usize,
    pub pixels: ImageLimits,
}

pub const CARD_LIMITS: CardLimits = CardLimits {
    file: 300 << 20,
    manifest: 32 << 20,
    image: 32 << 20,
    cover: 32 << 20,
    assets_total: 256 << 20,
    pixels: ImageLimits {
        max_side: 8192,
        max_pixels: 24_000_000,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    /// 全身圖（characters/<id>.png）
    Portrait,
    /// 裁切頭像（characters/<id>.avatar.png）
    Avatar,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardAsset {
    pub outcome_index: usize,
    pub kind: AssetKind,
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
pub struct DecodedCard {
    pub card: RefactorCardFile,
    pub assets: Vec<CardAsset>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestAsset {
    asset_id: String,
    outcome_index: usize,
    kind: AssetKind,
    mime: String,
    length: u64,
    hash: String,
}

fn invalid(detail: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
    UiMsg::RefactorCardInvalid {
        detail: detail.into(),
    }
    .into_error()
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn valid_asset_id(id: &str) -> bool {
    (1..=32).contains(&id.len())
        && id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
        })
}

/// 文字 chunk 的關鍵字是 chara／ccv3＝ST 卡資料。
fn is_st_card_chunk(kind: &[u8; 4], data: &[u8]) -> bool {
    matches!(kind, b"tEXt" | b"zTXt" | b"iTXt")
        && data
            .split(|byte| *byte == 0)
            .next()
            .is_some_and(|keyword| keyword == b"chara" || keyword == b"ccv3")
}

/// 底圖剝掉 ST 卡 chunk 與舊版自家 chunk，回傳 IEND 之前的部分（不含 IEND）。
fn stripped_base(base: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = PNG_MAGIC.to_vec();
    for_each_chunk(base, |chunk| {
        if !(is_own(chunk) || &chunk.kind == b"IEND" || is_st_card_chunk(&chunk.kind, chunk.data)) {
            out.extend_from_slice(&base[chunk.start..chunk.end]);
        }
        Ok(())
    })?;
    Ok(out)
}

fn is_own(chunk: &PngChunk) -> bool {
    &chunk.kind == MANIFEST_CHUNK || &chunk.kind == ASSET_CHUNK
}

/// 封裝：先在複製任何素材之前檢查張數、單張、總量與封面上限；card 先轉對外版本；
/// 產出自檢走 decode()。
pub fn encode(card: &RefactorCardFile, assets: &[CardAsset], cover: &[u8]) -> DataResult<Vec<u8>> {
    let limits = CARD_LIMITS;
    if cover.len() > limits.cover {
        return Err(invalid("cover too large"));
    }
    if assets.len() > card.outcome.characters.len() * 2 {
        return Err(invalid("too many assets"));
    }
    let mut total = 0usize;
    for asset in assets {
        if asset.bytes.len() > limits.image {
            return Err(invalid("asset too large"));
        }
        total += asset.bytes.len();
        if total > limits.assets_total {
            return Err(invalid("assets too large in total"));
        }
    }
    let mut out = stripped_base(cover).map_err(invalid)?;
    let mut manifest_assets = Vec::new();
    for (index, asset) in assets.iter().enumerate() {
        manifest_assets.push(ManifestAsset {
            asset_id: format!("a{index}"),
            outcome_index: asset.outcome_index,
            kind: asset.kind,
            mime: "image/png".to_owned(),
            length: asset.bytes.len() as u64,
            hash: sha256_hex(&asset.bytes),
        });
    }
    let mut manifest = serde_json::to_value(card.for_export())?;
    manifest["assets"] = serde_json::to_value(&manifest_assets)?;
    let mut payload = MANIFEST_MAGIC.to_vec();
    payload.push(PAYLOAD_VERSION);
    payload.extend_from_slice(&serde_json::to_vec(&manifest)?);
    if payload.len() - 5 > limits.manifest {
        return Err(invalid("manifest too large"));
    }
    out.extend_from_slice(&png_chunk(MANIFEST_CHUNK, &payload));
    for (entry, asset) in manifest_assets.iter().zip(assets) {
        let mut payload = ASSET_MAGIC.to_vec();
        payload.push(PAYLOAD_VERSION);
        payload.push(entry.asset_id.len() as u8);
        payload.extend_from_slice(entry.asset_id.as_bytes());
        payload.extend_from_slice(&asset.bytes);
        out.extend_from_slice(&png_chunk(ASSET_CHUNK, &payload));
    }
    out.extend_from_slice(&png_chunk(b"IEND", &[]));
    let decoded = decode(&out)?;
    if decoded.card.outcome != card.outcome || decoded.assets.len() != assets.len() {
        return Err(invalid("self-check mismatch"));
    }
    Ok(out)
}

/// 整包原子驗證並解出封套與素材；任一項不過回 RefactorCardInvalid（版本較新回 RefactorCardNewer）。
pub fn decode(bytes: &[u8]) -> DataResult<DecodedCard> {
    decode_with(bytes, &CARD_LIMITS)
}

pub(crate) fn decode_with(bytes: &[u8], limits: &CardLimits) -> DataResult<DecodedCard> {
    if bytes.len() > limits.file {
        return Err(invalid("file too large"));
    }
    // 整個檔當圖驗：全部 CRC、結構、封面的 IHDR／PLTE／zlib 流與掃描線（自家 chunk 是
    // ancillary，解壓只走 IDAT）
    validate_png_image(bytes, limits.pixels).map_err(|error| invalid(format!("cover: {error}")))?;

    // manifest：恰一個 ttRd；封面（自家以外的 chunk）總長上限
    let mut manifest_chunk: Option<&[u8]> = None;
    let mut manifests = 0usize;
    let mut cover = 0usize;
    for_each_chunk(bytes, |chunk| {
        if &chunk.kind == MANIFEST_CHUNK {
            manifests += 1;
            manifest_chunk.get_or_insert(chunk.data);
        } else if !is_own(chunk) {
            cover += chunk.end - chunk.start;
        }
        Ok(())
    })
    .map_err(invalid)?;
    if cover > limits.cover {
        return Err(invalid("cover too large"));
    }
    let (1, Some(payload)) = (manifests, manifest_chunk) else {
        return Err(invalid(format!(
            "expected exactly one manifest chunk, found {manifests}"
        )));
    };
    if payload.len() < 5 || &payload[..4] != MANIFEST_MAGIC {
        return Err(invalid("manifest magic mismatch"));
    }
    if payload[4] != PAYLOAD_VERSION {
        return Err(invalid("manifest payload version"));
    }
    let json = &payload[5..];
    if json.len() > limits.manifest {
        return Err(invalid("manifest too large"));
    }
    let json = std::str::from_utf8(json).map_err(|_| invalid("manifest is not UTF-8"))?;
    let mut manifest: Value =
        serde_json::from_str(json).map_err(|error| invalid(error.to_string()))?;
    let raw_assets = manifest
        .as_object_mut()
        .and_then(|object| object.remove("assets"))
        .unwrap_or(Value::Array(Vec::new()));
    if manifest.get("format").is_none() {
        return Err(invalid("manifest is not an envelope"));
    }
    let card = parse_card_value(manifest)?;
    let character_count = card.outcome.characters.len();

    // asset 清單：id 格式與唯一、(index, kind) 唯一且在範圍、kind／mime、length／hash 形狀、張數與總和上限
    let entries: Vec<ManifestAsset> =
        serde_json::from_value(raw_assets).map_err(|error| invalid(error.to_string()))?;
    if entries.len() > character_count * 2 {
        return Err(invalid("too many assets"));
    }
    let mut by_id: BTreeMap<&str, &ManifestAsset> = BTreeMap::new();
    let mut slots = BTreeSet::new();
    let mut total = 0usize;
    for entry in &entries {
        if !valid_asset_id(&entry.asset_id) || by_id.insert(&entry.asset_id, entry).is_some() {
            return Err(invalid("asset id malformed or duplicated"));
        }
        if entry.outcome_index >= character_count
            || !slots.insert((entry.outcome_index, entry.kind))
        {
            return Err(invalid("asset index out of range or duplicated"));
        }
        if entry.mime != "image/png" {
            return Err(invalid("asset mime must be image/png"));
        }
        let length = usize::try_from(entry.length)
            .ok()
            .filter(|length| *length <= limits.image)
            .ok_or_else(|| invalid("asset too large"))?;
        total += length;
        if total > limits.assets_total {
            return Err(invalid("assets too large in total"));
        }
        if entry.hash.len() != 64
            || !entry
                .hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(invalid("asset hash malformed"));
        }
    }

    // 圖片 chunk 與 manifest 一對一：無孤兒、無重複、無缺漏；長度與 SHA-256 相符
    let mut found: BTreeMap<&str, &[u8]> = BTreeMap::new();
    for_each_chunk(bytes, |chunk| {
        if &chunk.kind != ASSET_CHUNK {
            return Ok(());
        }
        let data = chunk.data;
        if data.len() < 6 || &data[..4] != ASSET_MAGIC {
            return Err("asset magic mismatch".to_owned());
        }
        if data[4] != PAYLOAD_VERSION {
            return Err("asset payload version".to_owned());
        }
        let id_len = data[5] as usize;
        let id_end = 6 + id_len;
        if id_len == 0 || id_end > data.len() {
            return Err("asset id length out of bounds".to_owned());
        }
        let id = std::str::from_utf8(&data[6..id_end]).map_err(|_| "asset id not UTF-8")?;
        let Some(entry) = by_id.get(id) else {
            return Err("asset chunk not listed in manifest".to_owned());
        };
        let image = &data[id_end..];
        if found.insert(entry.asset_id.as_str(), image).is_some() {
            return Err("duplicate asset chunk".to_owned());
        }
        if image.len() as u64 != entry.length || sha256_hex(image) != entry.hash {
            return Err("asset length or hash mismatch".to_owned());
        }
        Ok(())
    })
    .map_err(invalid)?;
    if found.len() != entries.len() {
        return Err(invalid("asset chunk missing"));
    }

    let mut assets = Vec::with_capacity(entries.len());
    for entry in &entries {
        let image = found[entry.asset_id.as_str()];
        validate_png_image(image, limits.pixels)
            .map_err(|error| invalid(format!("asset {}: {error}", entry.asset_id)))?;
        assets.push(CardAsset {
            outcome_index: entry.outcome_index,
            kind: entry.kind,
            bytes: image.to_vec(),
        });
    }
    Ok(DecodedCard { card, assets })
}

#[cfg(test)]
mod tests;
