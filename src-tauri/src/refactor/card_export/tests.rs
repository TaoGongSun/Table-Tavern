use super::*;
use crate::import::card_io::{png_chunk, PNG_MAGIC};
use crate::import::png_image::for_each_chunk;
use crate::import::png_image::test_png::real_png;
use crate::import::{save_character_avatar, save_character_image, save_gm_image};
use crate::refactor::card_png::decode;
use crate::refactor::test_support::*;
use crate::refactor::{apply, RefactorSelection};
use serde_json::json;
use std::path::PathBuf;

fn outcome_of(names: &[&str]) -> RefactorOutcome {
    serde_json::from_value(json!({
        "characters": names.iter().map(|name| json!({
            "name": name, "emoji": "🙂", "public_md": "", "private_md": "",
            "source_uids": [], "solo_entry_md": "",
        })).collect::<Vec<_>>(),
    }))
    .unwrap()
}

struct Table {
    root: TestRoot,
    world_id: String,
    ids: Vec<Option<String>>,
}

impl Table {
    fn out(&self, name: &str) -> PathBuf {
        self.root.path().join(name)
    }
}

/// 三位角色勾 0、2 成卡，玩家是 2。
fn applied_table(label: &str) -> Table {
    let root = TestRoot::new(label);
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let selection = RefactorSelection {
        player_index: Some(2),
        ..no_player_selection(vec![0, 2])
    };
    apply(
        root.path(),
        &world_id,
        &outcome_of(&["亞瑟", "梅林", "桂妮薇兒"]),
        &selection,
    )
    .unwrap();
    let ids = parse_card(
        &data::read_refactor_outcome(root.path(), &world_id)
            .unwrap()
            .unwrap(),
    )
    .unwrap()
    .applied
    .unwrap()
    .characters
    .into_iter()
    .map(|item| item.character_id)
    .collect();
    Table {
        root,
        world_id,
        ids,
    }
}

fn ihdr_width(png: &[u8]) -> u32 {
    let mut width = 0;
    for_each_chunk(png, |chunk| {
        if &chunk.kind == b"IHDR" {
            width = u32::from_be_bytes(chunk.data[0..4].try_into().unwrap());
        }
        Ok(())
    })
    .unwrap();
    width
}

fn has_chara(png: &[u8]) -> bool {
    let mut found = false;
    for_each_chunk(png, |chunk| {
        found |= chunk.data.starts_with(b"chara\0");
        Ok(())
    })
    .unwrap();
    found
}

fn with_st_chunk(png: &[u8]) -> Vec<u8> {
    let mut out = PNG_MAGIC.to_vec();
    for_each_chunk(png, |chunk| {
        if &chunk.kind == b"IEND" {
            out.extend_from_slice(&png_chunk(b"tEXt", b"chara\0e30="));
        }
        out.extend_from_slice(&png[chunk.start..chunk.end]);
        Ok(())
    })
    .unwrap();
    out
}

#[test]
fn json_export_is_envelope_without_local_id() {
    let table = applied_table("export-json");
    let path = table.out("卡.json");
    let size = export_saved(table.root.path(), &table.world_id, &path, false).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert_eq!(size, text.len() as u64);
    assert!(!text.contains("player_card_id"));
    let card = parse_card(&text).unwrap();
    assert_eq!(card.applied.unwrap().player_index, Some(2));

    let outcome_path = table.out("結果卡.json");
    export_outcome(
        table.root.path(),
        &table.world_id,
        &outcome_of(&["甲"]),
        &outcome_path,
    )
    .unwrap();
    let value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(outcome_path).unwrap()).unwrap();
    assert_eq!(value["format"], "table-tavern-refactor-card");
}

#[test]
fn images_need_png_and_a_mapping() {
    let table = applied_table("export-guards");
    assert_eq!(
        export_saved(
            table.root.path(),
            &table.world_id,
            &table.out("卡.json"),
            true
        )
        .unwrap_err()
        .to_string(),
        UiMsg::RefactorExportImagesNeedPng.to_string()
    );

    let legacy = serde_json::to_string(&outcome_of(&["甲"])).unwrap();
    data::write_refactor_outcome(table.root.path(), &table.world_id, &legacy).unwrap();
    assert_eq!(
        export_saved(
            table.root.path(),
            &table.world_id,
            &table.out("卡.png"),
            true
        )
        .unwrap_err()
        .to_string(),
        UiMsg::RefactorExportNoMap.to_string()
    );
    // 舊存檔不附圖照樣匯得出
    export_saved(
        table.root.path(),
        &table.world_id,
        &table.out("卡.png"),
        false,
    )
    .unwrap();

    let empty = data::create_world(table.root.path(), "空桌").unwrap();
    assert!(
        export_saved(table.root.path(), &empty, &table.out("x.png"), false)
            .unwrap_err()
            .to_string()
            .contains(EXPORT_NONE)
    );
}

#[test]
fn image_card_follows_mapping_after_rename_and_skips_deleted() {
    let table = applied_table("export-images");
    let (root, world) = (table.root.path(), table.world_id.as_str());
    let first = table.ids[0].clone().unwrap();
    let third = table.ids[2].clone().unwrap();
    let (portrait, avatar, third_avatar) = (real_png(20, 30), real_png(8, 8), real_png(9, 9));
    save_character_image(root, world, &first, &portrait).unwrap();
    save_character_avatar(root, world, &first, &avatar).unwrap();
    save_character_avatar(root, world, &third, &third_avatar).unwrap();
    let mut card = data::read_character(root, world, &first).unwrap();
    card.name = "改名後的亞瑟".to_owned();
    data::write_character(root, world, &card).unwrap();

    let path = table.out("卡+角色圖.png");
    let size = export_saved(root, world, &path, true).unwrap();
    let bytes = fs::read(&path).unwrap();
    assert_eq!(size, bytes.len() as u64);
    let decoded = decode(&bytes).unwrap();
    let got: Vec<_> = decoded
        .assets
        .iter()
        .map(|asset| (asset.outcome_index, asset.kind, asset.bytes.clone()))
        .collect();
    assert_eq!(
        got,
        vec![
            (0, AssetKind::Portrait, portrait.clone()),
            (0, AssetKind::Avatar, avatar),
            (2, AssetKind::Avatar, third_avatar),
        ]
    );
    // 沒有 GM 圖：封面退到第一位已建卡角色的全身圖
    assert_eq!(ihdr_width(&bytes), 20);

    data::delete_character(root, world, &third).unwrap();
    export_saved(root, world, &path, true).unwrap();
    let decoded = decode(&fs::read(&path).unwrap()).unwrap();
    assert!(decoded.assets.iter().all(|asset| asset.outcome_index == 0));
}

#[test]
fn broken_character_image_blocks_export_by_name() {
    let table = applied_table("export-broken");
    let (root, world) = (table.root.path(), table.world_id.as_str());
    let first = table.ids[0].clone().unwrap();
    let fake: Vec<u8> = PNG_MAGIC.iter().copied().chain([1, 2, 3]).collect();
    save_character_image(root, world, &first, &fake).unwrap();
    let message = export_saved(root, world, &table.out("卡.png"), true)
        .unwrap_err()
        .to_string();
    assert!(message.contains("refactor_export_image_invalid") && message.contains("亞瑟"));
    // 不含圖版的封面跳過壞圖，退到透明圖
    let path = table.out("卡2.png");
    export_saved(root, world, &path, false).unwrap();
    assert_eq!(ihdr_width(&fs::read(path).unwrap()), 1);
}

#[test]
fn gm_cover_is_used_and_stripped() {
    let table = applied_table("export-cover");
    let (root, world) = (table.root.path(), table.world_id.as_str());
    assert!(save_gm_image(root, world, &with_st_chunk(&real_png(40, 4))));
    let path = table.out("卡.png");
    export_outcome(root, world, &outcome_of(&["甲"]), &path).unwrap();
    let bytes = fs::read(&path).unwrap();
    assert_eq!(ihdr_width(&bytes), 40);
    assert!(!has_chara(&bytes));
    let decoded = decode(&bytes).unwrap();
    assert!(decoded.card.applied.is_none());
    assert!(decoded.assets.is_empty());
}

#[test]
fn only_png_or_json_paths_are_written() {
    let table = applied_table("export-ext");
    let (root, world) = (table.root.path(), table.world_id.as_str());
    for name in ["卡.jpg", "卡"] {
        let path = table.out(name);
        assert_eq!(
            export_saved(root, world, &path, false)
                .unwrap_err()
                .to_string(),
            UiMsg::RefactorExportNeedPngOrJson.to_string()
        );
        assert_eq!(
            export_saved(root, world, &path, true)
                .unwrap_err()
                .to_string(),
            UiMsg::RefactorExportImagesNeedPng.to_string()
        );
        assert!(export_outcome(root, world, &outcome_of(&["甲"]), &path).is_err());
        assert!(!path.exists());
    }
    export_saved(root, world, &table.out("卡.PNG"), true).unwrap();
}

#[test]
fn oversized_character_image_is_refused_without_reading_it_all() {
    let table = applied_table("export-oversize");
    let (root, world) = (table.root.path(), table.world_id.as_str());
    let first = table.ids[0].clone().unwrap();
    let path = data::character_path(root, world, &first)
        .unwrap()
        .with_extension("png");
    let mut big = real_png(4, 4);
    big.resize(CARD_LIMITS.image + 10, 0);
    fs::write(&path, big).unwrap();
    let message = export_saved(root, world, &table.out("卡.png"), true)
        .unwrap_err()
        .to_string();
    assert!(message.contains("image too large") && message.contains("亞瑟"));
}
