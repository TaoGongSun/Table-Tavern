use super::*;
use crate::import::png_image::test_png::{assemble, real_png, rgba_ihdr};
use crate::refactor::card_file::{RefactorApplied, RefactorAppliedCharacter};
use crate::refactor::types::RefactorOutcome;
use serde_json::json;

type Chunks = Vec<([u8; 4], Vec<u8>)>;

fn outcome_of(count: usize) -> RefactorOutcome {
    serde_json::from_value(json!({
        "characters": (0..count).map(|index| json!({
            "name": format!("角色{index}"), "emoji": "🙂", "public_md": "", "private_md": "",
            "source_uids": ["1"], "solo_entry_md": "",
        })).collect::<Vec<_>>(),
    }))
    .unwrap()
}

fn sample_card() -> RefactorCardFile {
    RefactorCardFile::new(
        outcome_of(2),
        Some(RefactorApplied {
            characters: vec![
                RefactorAppliedCharacter {
                    outcome_index: 0,
                    character_id: Some("c0".to_owned()),
                },
                RefactorAppliedCharacter {
                    outcome_index: 1,
                    character_id: None,
                },
            ],
            player_index: Some(0),
            player_card_id: Some("c0".to_owned()),
        }),
    )
}

fn sample_assets() -> Vec<CardAsset> {
    vec![
        CardAsset {
            outcome_index: 0,
            kind: AssetKind::Portrait,
            bytes: real_png(12, 10),
        },
        CardAsset {
            outcome_index: 0,
            kind: AssetKind::Avatar,
            bytes: real_png(6, 6),
        },
    ]
}

fn sample() -> Vec<u8> {
    encode(&sample_card(), &sample_assets(), &real_png(8, 8)).unwrap()
}

fn chunks_of(bytes: &[u8]) -> Chunks {
    let mut out = Vec::new();
    for_each_chunk(bytes, |chunk| {
        out.push((chunk.kind, chunk.data.to_vec()));
        Ok(())
    })
    .unwrap();
    out
}

fn rebuild(chunks: &Chunks) -> Vec<u8> {
    let mut out = PNG_MAGIC.to_vec();
    for (kind, data) in chunks {
        out.extend_from_slice(&png_chunk(kind, data));
    }
    out
}

fn mutate_chunks(f: impl FnOnce(&mut Chunks)) -> Vec<u8> {
    let mut chunks = chunks_of(&sample());
    f(&mut chunks);
    rebuild(&chunks)
}

fn mutate_manifest(f: impl FnOnce(&mut Value)) -> Vec<u8> {
    mutate_chunks(|chunks| {
        let (_, payload) = chunks
            .iter_mut()
            .find(|(kind, _)| kind == MANIFEST_CHUNK)
            .unwrap();
        let mut manifest: Value = serde_json::from_slice(&payload[5..]).unwrap();
        f(&mut manifest);
        payload.truncate(5);
        payload.extend_from_slice(&serde_json::to_vec(&manifest).unwrap());
    })
}

fn asset_chunk(id: &str, bytes: &[u8]) -> ([u8; 4], Vec<u8>) {
    let mut payload = ASSET_MAGIC.to_vec();
    payload.push(PAYLOAD_VERSION);
    payload.push(id.len() as u8);
    payload.extend_from_slice(id.as_bytes());
    payload.extend_from_slice(bytes);
    (*ASSET_CHUNK, payload)
}

fn before_iend(chunks: &mut Chunks, chunk: ([u8; 4], Vec<u8>)) {
    let at = chunks.len() - 1;
    chunks.insert(at, chunk);
}

fn rejected(bytes: &[u8], needle: &str) {
    let message = decode(bytes).unwrap_err().to_string();
    assert!(
        message.contains("refactor_card_invalid") && message.contains(needle),
        "{message} 應含 {needle}"
    );
}

#[test]
fn round_trip_keeps_card_and_assets_and_drops_local_id() {
    let decoded = decode(&sample()).unwrap();
    let card = sample_card();
    assert_eq!(decoded.card.outcome, card.outcome);
    let applied = decoded.card.applied.unwrap();
    assert_eq!(applied.player_index, Some(0));
    assert_eq!(applied.player_card_id, None);
    assert_eq!(decoded.assets, sample_assets());
}

#[test]
fn chunk_order_does_not_matter() {
    let shuffled = mutate_chunks(|chunks| {
        let manifest = chunks
            .iter()
            .position(|(kind, _)| kind == MANIFEST_CHUNK)
            .unwrap();
        let moved = chunks.remove(manifest);
        let at = chunks.len() - 1;
        chunks.insert(at, moved);
        let first_asset = chunks
            .iter()
            .position(|(kind, _)| kind == ASSET_CHUNK)
            .unwrap();
        chunks.swap(first_asset, first_asset + 1);
    });
    assert_eq!(decode(&shuffled).unwrap().assets, sample_assets());
}

#[test]
fn cover_loses_st_card_and_old_own_chunks() {
    let mut cover = chunks_of(&real_png(8, 8));
    before_iend(&mut cover, (*b"tEXt", b"chara\0e30=".to_vec()));
    before_iend(&mut cover, (*b"tEXt", b"ccv3\0e30=".to_vec()));
    before_iend(&mut cover, (*b"tEXt", b"Comment\0keep".to_vec()));
    let old = chunks_of(&sample());
    for chunk in old
        .iter()
        .filter(|(kind, _)| kind == MANIFEST_CHUNK || kind == ASSET_CHUNK)
    {
        before_iend(&mut cover, chunk.clone());
    }
    let out = encode(&sample_card(), &[], &rebuild(&cover)).unwrap();
    let chunks = chunks_of(&out);
    let texts: Vec<_> = chunks
        .iter()
        .filter(|(kind, _)| kind == b"tEXt")
        .map(|(_, data)| data.clone())
        .collect();
    assert_eq!(texts, vec![b"Comment\0keep".to_vec()]);
    assert_eq!(
        chunks
            .iter()
            .filter(|(kind, _)| kind == MANIFEST_CHUNK)
            .count(),
        1
    );
    assert_eq!(
        chunks
            .iter()
            .filter(|(kind, _)| kind == ASSET_CHUNK)
            .count(),
        0
    );
}

#[test]
fn png_layer_and_manifest_chunk_violations() {
    let mut bad_crc = sample();
    let at = bad_crc.len() - 40;
    bad_crc[at] ^= 0xff;
    rejected(&bad_crc, "CRC");

    rejected(
        &mutate_chunks(|chunks| chunks.retain(|(kind, _)| kind != MANIFEST_CHUNK)),
        "exactly one manifest",
    );
    rejected(
        &mutate_chunks(|chunks| {
            let manifest = chunks
                .iter()
                .find(|(kind, _)| kind == MANIFEST_CHUNK)
                .unwrap()
                .clone();
            before_iend(chunks, manifest);
        }),
        "exactly one manifest",
    );
    rejected(
        &mutate_chunks(|chunks| {
            let (_, payload) = chunks
                .iter_mut()
                .find(|(kind, _)| kind == MANIFEST_CHUNK)
                .unwrap();
            payload[0] = b'X';
        }),
        "manifest magic",
    );
    rejected(
        &mutate_chunks(|chunks| {
            let (_, payload) = chunks
                .iter_mut()
                .find(|(kind, _)| kind == MANIFEST_CHUNK)
                .unwrap();
            payload[4] = 2;
        }),
        "manifest payload version",
    );
    let newer = mutate_manifest(|manifest| manifest["version"] = json!(2));
    assert_eq!(
        decode(&newer).unwrap_err().to_string(),
        UiMsg::RefactorCardNewer.to_string()
    );
    rejected(
        &mutate_manifest(|manifest| manifest["format"] = json!("other")),
        "unknown format",
    );
    rejected(
        &mutate_manifest(|manifest| manifest["applied"]["player_index"] = json!(1)),
        "player index",
    );
}

#[test]
fn manifest_asset_field_violations() {
    let cases: Vec<(Box<dyn FnOnce(&mut Value)>, &str)> = vec![
        (
            Box::new(|m| m["assets"][0]["asset_id"] = json!("A!")),
            "asset id",
        ),
        (
            Box::new(|m| m["assets"][1]["asset_id"] = json!("a0")),
            "asset id",
        ),
        (
            Box::new(|m| m["assets"][1]["kind"] = json!("portrait")),
            "duplicated",
        ),
        (
            Box::new(|m| m["assets"][0]["outcome_index"] = json!(2)),
            "out of range",
        ),
        (
            Box::new(|m| m["assets"][0]["kind"] = json!("banner")),
            "unknown variant",
        ),
        (
            Box::new(|m| m["assets"][0]["mime"] = json!("image/jpeg")),
            "mime",
        ),
        (
            Box::new(|m| m["assets"][0]["length"] = json!(CARD_LIMITS.image + 1)),
            "too large",
        ),
        (
            Box::new(|m| m["assets"][0]["length"] = json!(3)),
            "length or hash",
        ),
        (
            Box::new(|m| m["assets"][0]["hash"] = json!("0".repeat(64))),
            "length or hash",
        ),
        (
            Box::new(|m| m["assets"][0]["hash"] = json!("XYZ")),
            "hash malformed",
        ),
        (
            Box::new(|m| m["assets"][0]["file_name"] = json!("../../evil.png")),
            "unknown field",
        ),
        (
            Box::new(|m| {
                let extra = m["assets"][0].clone();
                let list = m["assets"].as_array_mut().unwrap();
                for index in 0..4 {
                    let mut item = extra.clone();
                    item["asset_id"] = json!(format!("x{index}"));
                    list.push(item);
                }
            }),
            "too many assets",
        ),
    ];
    for (mutate, needle) in cases {
        rejected(&mutate_manifest(mutate), needle);
    }
}

#[test]
fn asset_chunk_pairing_violations() {
    let png = real_png(6, 6);
    rejected(
        &mutate_chunks(|chunks| before_iend(chunks, asset_chunk("zz", &png))),
        "not listed",
    );
    rejected(
        &mutate_chunks(|chunks| {
            let first = chunks
                .iter()
                .find(|(kind, _)| kind == ASSET_CHUNK)
                .unwrap()
                .clone();
            before_iend(chunks, first);
        }),
        "duplicate asset chunk",
    );
    rejected(
        &mutate_chunks(|chunks| {
            let first = chunks
                .iter()
                .position(|(kind, _)| kind == ASSET_CHUNK)
                .unwrap();
            chunks.remove(first);
        }),
        "missing",
    );
    for (patch, needle) in [
        ((0usize, b'X'), "asset magic"),
        ((4, 2), "asset payload version"),
        ((5, 0), "id length"),
        ((5, 250), "id length"),
    ] {
        rejected(
            &mutate_chunks(|chunks| {
                let (_, payload) = chunks
                    .iter_mut()
                    .find(|(kind, _)| kind == ASSET_CHUNK)
                    .unwrap();
                if patch.1 == 250 {
                    payload.truncate(10);
                }
                payload[patch.0] = patch.1;
            }),
            needle,
        );
    }
}

#[test]
fn asset_must_be_a_fully_valid_png() {
    // length 與 hash 都對得上，但內容不是能解碼的 PNG
    let mut broken = real_png(6, 6);
    let idat = broken.len() - 30;
    broken[idat] ^= 0xff;
    let card = sample_card();
    let mut manifest = serde_json::to_value(card.for_export()).unwrap();
    manifest["assets"] = json!([{
        "asset_id": "a0", "outcome_index": 0, "kind": "portrait", "mime": "image/png",
        "length": broken.len(), "hash": sha256_hex(&broken),
    }]);
    let mut payload = MANIFEST_MAGIC.to_vec();
    payload.push(PAYLOAD_VERSION);
    payload.extend_from_slice(&serde_json::to_vec(&manifest).unwrap());
    let mut chunks = chunks_of(&real_png(8, 8));
    before_iend(&mut chunks, (*MANIFEST_CHUNK, payload));
    before_iend(&mut chunks, asset_chunk("a0", &broken));
    rejected(&rebuild(&chunks), "asset a0");
}

fn with_cover(cover: &[u8]) -> Vec<u8> {
    // 封面換成指定的圖、自家 chunk 照搬（CRC 都重算過）
    let own: Chunks = chunks_of(&sample())
        .into_iter()
        .filter(|(kind, _)| kind == MANIFEST_CHUNK || kind == ASSET_CHUNK)
        .collect();
    let mut chunks = chunks_of(cover);
    for chunk in own {
        before_iend(&mut chunks, chunk);
    }
    rebuild(&chunks)
}

#[test]
fn cover_itself_must_be_a_valid_png() {
    // 壞 zlib（CRC 重算過）
    let mut broken = chunks_of(&real_png(8, 8));
    let idat = broken.iter().position(|(kind, _)| kind == b"IDAT").unwrap();
    broken[idat].1 = vec![0x78, 0x9c, 0xff, 0xff, 0xff];
    rejected(&with_cover(&rebuild(&broken)), "cover: zlib");
    // 零寬
    rejected(
        &with_cover(&assemble(rgba_ihdr(0, 2), &[0, 0])),
        "cover: image size",
    );
    // bit depth 3
    let mut depth3 = rgba_ihdr(2, 2);
    depth3[8] = 3;
    rejected(
        &with_cover(&assemble(depth3, &[0; 6])),
        "cover: invalid color type",
    );
    // IDAT 尾端截 2 bytes
    let mut cut = chunks_of(&real_png(8, 8));
    let len = cut[idat].1.len();
    cut[idat].1.truncate(len - 2);
    rejected(&with_cover(&rebuild(&cut)), "cover: zlib");
    // 合法封面照過
    assert!(decode(&with_cover(&real_png(9, 9))).is_ok());
}

#[test]
fn limits_are_enforced_on_decode() {
    let bytes = sample();
    let tight = |change: fn(&mut CardLimits)| {
        let mut limits = CARD_LIMITS;
        change(&mut limits);
        decode_with(&bytes, &limits).unwrap_err().to_string()
    };
    assert!(tight(|l| l.file = 100).contains("file too large"));
    assert!(tight(|l| l.manifest = 10).contains("manifest too large"));
    assert!(tight(|l| l.image = 50).contains("asset too large"));
    assert!(tight(|l| l.assets_total = 200).contains("too large in total"));
    assert!(tight(|l| l.cover = 50).contains("cover too large"));
    assert!(tight(|l| l.pixels.max_side = 4).contains("out of range"));
}

#[test]
fn encode_checks_limits_before_copying() {
    let huge = CardAsset {
        outcome_index: 0,
        kind: AssetKind::Portrait,
        bytes: vec![0; CARD_LIMITS.image + 1],
    };
    assert!(encode(&sample_card(), &[huge], &real_png(8, 8))
        .unwrap_err()
        .to_string()
        .contains("asset too large"));
    let five: Vec<CardAsset> = (0..5).map(|_| sample_assets()[0].clone()).collect();
    assert!(encode(&sample_card(), &five, &real_png(8, 8))
        .unwrap_err()
        .to_string()
        .contains("too many assets"));
}

#[test]
fn manifest_text_must_be_utf8_json_envelope() {
    let replace_json = |json: &[u8]| {
        mutate_chunks(|chunks| {
            let (_, payload) = chunks
                .iter_mut()
                .find(|(kind, _)| kind == MANIFEST_CHUNK)
                .unwrap();
            payload.truncate(5);
            payload.extend_from_slice(json);
        })
    };
    rejected(&replace_json(b"{\"format\":\"\xff\"}"), "not UTF-8");
    rejected(&replace_json(b"{not json"), "key must be a string");
    rejected(&replace_json(b"[]"), "not an envelope");
    rejected(
        &mutate_manifest(|manifest| manifest["version"] = json!(0)),
        "unsupported version",
    );
}

#[test]
fn ztxt_and_itxt_card_chunks_are_stripped_too() {
    let mut cover = chunks_of(&real_png(8, 8));
    before_iend(&mut cover, (*b"zTXt", b"chara\0\0x".to_vec()));
    before_iend(&mut cover, (*b"iTXt", b"ccv3\0\0\0\0\0x".to_vec()));
    before_iend(&mut cover, (*b"iTXt", b"Title\0\0\0\0\0keep".to_vec()));
    let out = encode(&sample_card(), &[], &rebuild(&cover)).unwrap();
    let texts: Vec<_> = chunks_of(&out)
        .into_iter()
        .filter(|(kind, _)| kind == b"zTXt" || kind == b"iTXt")
        .map(|(_, data)| data)
        .collect();
    assert_eq!(texts, vec![b"Title\0\0\0\0\0keep".to_vec()]);
}

/// 8-bit 索引色、1 筆 palette、像素引用 index=1（CRC 與 zlib 都正確）。
fn bad_palette_png() -> Vec<u8> {
    let mut ihdr = rgba_ihdr(2, 1);
    ihdr[9] = 3;
    let mut chunks = chunks_of(&assemble(ihdr, &[0, 0, 1]));
    chunks.insert(1, (*b"PLTE", vec![0; 3]));
    rebuild(&chunks)
}

#[test]
fn palette_index_out_of_range_is_refused_in_cover_and_assets() {
    rejected(&with_cover(&bad_palette_png()), "cover: palette index");
    let bad = CardAsset {
        outcome_index: 0,
        kind: AssetKind::Avatar,
        bytes: bad_palette_png(),
    };
    assert!(encode(&sample_card(), &[bad], &real_png(8, 8))
        .unwrap_err()
        .to_string()
        .contains("asset a0: palette index"));
}
