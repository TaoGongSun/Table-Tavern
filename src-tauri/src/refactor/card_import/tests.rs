use super::*;
use crate::data;
use crate::import::png_image::test_png::real_png;
use crate::refactor::apply::apply_with_assets;
use crate::refactor::card_file::{RefactorApplied, RefactorAppliedCharacter};
use crate::refactor::card_png::encode;
use crate::refactor::test_support::*;
use crate::refactor::RefactorSelection;
use crate::ui_msg::UiMsg;
use serde_json::json;
use std::sync::Mutex;

/// 暫存單槽是全域狀態：動到它的測試排隊跑。
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    let guard = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    clear_for_test();
    guard
}

fn outcome_of(names: &[&str]) -> RefactorOutcome {
    serde_json::from_value(json!({
        "characters": names.iter().map(|name| json!({
            "name": name, "emoji": "🙂", "public_md": "", "private_md": "",
            "source_uids": [], "solo_entry_md": "",
        })).collect::<Vec<_>>(),
    }))
    .unwrap()
}

fn asset(index: usize, kind: AssetKind, seed: u32) -> CardAsset {
    CardAsset {
        outcome_index: index,
        kind,
        bytes: real_png(10 + seed, 10),
    }
}

fn card_png(names: &[&str], assets: &[CardAsset]) -> Vec<u8> {
    let count = names.len();
    let card = RefactorCardFile::new(
        outcome_of(names),
        Some(RefactorApplied {
            characters: (0..count)
                .map(|index| RefactorAppliedCharacter {
                    outcome_index: index,
                    character_id: Some(format!("src{index}")),
                })
                .collect(),
            player_index: None,
            player_card_id: None,
        }),
    );
    encode(&card, assets, &real_png(8, 8)).unwrap()
}

fn assets_gone(error: &str) -> bool {
    error == UiMsg::RefactorAssetsGone.to_string()
}

#[test]
fn sniffing_routes_own_st_and_json() {
    let _serial = serial();
    let own = card_png(&["甲"], &[]);
    assert!(has_manifest_chunk(&own));
    assert!(open_card(&own).is_ok());

    let plain = real_png(4, 4);
    assert!(!has_manifest_chunk(&plain));
    assert_eq!(
        open_card(&plain).err().unwrap().to_string(),
        UiMsg::RefactorCardIsCharacter.to_string()
    );

    let json = serde_json::to_vec(&outcome_of(&["甲"])).unwrap();
    let opened = open_card(&json).unwrap();
    assert!(opened.card.applied.is_none() && opened.assets.is_empty());
    assert!(open_card(b"\xff\xfe not text").is_err());
}

#[test]
fn slot_release_needs_world_and_token() {
    let _serial = serial();
    let png = card_png(&["甲"], &[asset(0, AssetKind::Avatar, 1)]);
    let a = open_and_stage("w1", &png).unwrap().token.unwrap();
    let b = open_and_stage("w1", &png).unwrap().token.unwrap();
    // 開 A → 開 B → 關 A：B 仍在、A 已失效
    release_assets("w1", &a);
    assert!(assets_gone(&take("w1", &a).err().unwrap().to_string()));
    release_assets("w2", &b);
    let staged = take("w1", &b).unwrap();
    assert_eq!(staged.assets.len(), 1);
    // 沒附圖的卡不佔槽
    assert!(open_and_stage("w1", &card_png(&["甲"], &[]))
        .unwrap()
        .token
        .is_none());
}

#[test]
fn claimed_assets_survive_close_and_new_card() {
    let _serial = serial();
    let root = TestRoot::new("claim-race");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let outcome = outcome_of(&["甲", "乙"]);
    let png = card_png(&["甲", "乙"], &[asset(1, AssetKind::Portrait, 2)]);
    let token = open_and_stage(&world_id, &png).unwrap().token.unwrap();

    // 套用一進來就取走；之後結果卡關閉、開了新卡，都碰不到這份素材
    let claimed = claim_assets(&world_id, Some(&token), &outcome)
        .unwrap()
        .unwrap();
    release_assets(&world_id, &token);
    let newer = open_and_stage(&world_id, &png).unwrap().token.unwrap();
    let result = apply_with_assets(
        root.path(),
        &world_id,
        &outcome,
        &no_player_selection(vec![0, 1]),
        &claimed.assets,
    )
    .unwrap();
    assert_eq!(result.summary.images_applied, 1);
    // 新卡的槽不受影響
    assert_eq!(take(&world_id, &newer).unwrap().assets.len(), 1);
}

#[test]
fn rejected_apply_puts_assets_back_unless_a_newer_card_took_the_slot() {
    let _serial = serial();
    let outcome = outcome_of(&["甲"]);
    let png = card_png(&["甲"], &[asset(0, AssetKind::Avatar, 3)]);

    // 拒套 → 放回 → 重試取得到
    let token = open_and_stage("w1", &png).unwrap().token.unwrap();
    let claimed = claim_assets("w1", Some(&token), &outcome).unwrap();
    assert_eq!(
        return_assets(claimed, "player-card-exists".to_owned()),
        "player-card-exists"
    );
    assert!(claim_assets("w1", Some(&token), &outcome)
        .unwrap()
        .is_some());

    // 拒套期間開了新卡 → 丟棄並回 AssetsGone，新卡槽不受影響
    let token = open_and_stage("w1", &png).unwrap().token.unwrap();
    let claimed = claim_assets("w1", Some(&token), &outcome).unwrap();
    let newer = open_and_stage("w1", &png).unwrap().token.unwrap();
    assert!(assets_gone(&return_assets(claimed, "x".to_owned())));
    assert!(assets_gone(
        &claim_assets("w1", Some(&token), &outcome)
            .err()
            .unwrap()
            .to_string()
    ));
    assert!(take("w1", &newer).is_ok());
}

#[test]
fn outcome_mismatch_is_refused_and_keeps_assets() {
    let _serial = serial();
    let png = card_png(&["甲"], &[asset(0, AssetKind::Avatar, 4)]);
    let token = open_and_stage("w1", &png).unwrap().token.unwrap();
    let other = outcome_of(&["改過的名字"]);
    assert!(claim_assets("w1", Some(&token), &other)
        .err()
        .unwrap()
        .to_string()
        .contains("refactor_card_invalid"));
    assert!(claim_assets("w1", Some(&token), &outcome_of(&["甲"]))
        .unwrap()
        .is_some());
}

/// 不連續勾選：圖依 outcome_index → 新卡 id 落位，沒建卡的 index 的圖略過。
#[test]
fn images_land_on_mapped_characters() {
    let root = TestRoot::new("images-land");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let outcome = outcome_of(&["甲", "乙", "丙"]);
    let assets = vec![
        asset(0, AssetKind::Portrait, 1),
        asset(1, AssetKind::Avatar, 2),
        asset(2, AssetKind::Avatar, 3),
    ];
    let result = apply_with_assets(
        root.path(),
        &world_id,
        &outcome,
        &RefactorSelection {
            ..no_player_selection(vec![0, 2])
        },
        &assets,
    )
    .unwrap();
    assert_eq!(result.summary.images_applied, 2);
    assert!(result.summary.images_failed.is_empty());
    let [first, third] = result.character_ids.as_slice() else {
        panic!("兩張卡")
    };
    let path = |id: &str, ext: &str| {
        data::character_path(root.path(), &world_id, id)
            .unwrap()
            .with_extension(ext)
    };
    assert_eq!(std::fs::read(path(first, "png")).unwrap(), assets[0].bytes);
    assert_eq!(
        std::fs::read(path(third, "avatar.png")).unwrap(),
        assets[2].bytes
    );
    assert!(!path(first, "avatar.png").exists());
}

/// 第二張圖寫入失敗：其餘照寫（第一、三張都在）、不回 Err、收據照記，撤銷連圖收乾淨。
#[test]
fn second_image_failure_does_not_stop_the_rest() {
    let root = TestRoot::new("image-fail");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let outcome = outcome_of(&["甲", "乙", "丙"]);
    // 第二張是唯一的頭像：只讓頭像的原子寫暫存檔停在半截
    let assets = vec![
        asset(0, AssetKind::Portrait, 1),
        asset(1, AssetKind::Avatar, 2),
        asset(2, AssetKind::Portrait, 3),
    ];
    let before = crate::receipts::snapshot_refactor(root.path(), &world_id).unwrap();
    let result = {
        let _guard = data::WriteFailGuard::partial_ending(".avatar.png.tmp", 1);
        apply_with_assets(
            root.path(),
            &world_id,
            &outcome,
            &no_player_selection(vec![0, 1, 2]),
            &assets,
        )
        .unwrap()
    };
    assert_eq!(result.summary.images_applied, 2);
    assert_eq!(result.summary.images_failed, vec!["乙".to_owned()]);
    let path = |id: &str, ext: &str| {
        data::character_path(root.path(), &world_id, id)
            .unwrap()
            .with_extension(ext)
    };
    let ids = result.character_ids.clone();
    assert_eq!(
        std::fs::read(path(&ids[0], "png")).unwrap(),
        assets[0].bytes
    );
    assert!(!path(&ids[1], "avatar.png").exists());
    assert!(!path(&ids[1], "avatar.png.tmp").exists());
    assert_eq!(
        std::fs::read(path(&ids[2], "png")).unwrap(),
        assets[2].bytes
    );

    crate::receipts::record_refactor_apply(
        root.path(),
        &world_id,
        &UiMsg::ReceiptRefactorApply.to_string(),
        result.character_ids,
        result.rewritten_entries,
        result.deleted_entries,
        before,
        &data::test_exclusive(&world_id),
    );
    crate::receipts::undo_last_import(root.path(), &world_id, &data::test_exclusive(&world_id))
        .unwrap();
    assert!(data::list_characters(root.path(), &world_id)
        .unwrap()
        .is_empty());
    assert!(!path(&ids[0], "png").exists());
    assert!(!path(&ids[2], "png").exists());
}
