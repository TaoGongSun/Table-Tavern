//! 重構判定的舞台卡（stage-card-in-speaker-list 包 3）：套用寫旗標、收據撤銷、中途失敗回滾、
//! 名字對不上就跳過。

use super::super::test_support::*;
use super::super::*;
use crate::data::{self, CharacterCard, StageFlags, Tier};
use crate::receipts;
use std::path::Path;

fn seed_card(root: &Path, world_id: &str, name: &str) -> String {
    let card = CharacterCard {
        id: data::new_id(),
        name: name.to_owned(),
        color: "#fff".to_owned(),
        avatar: "🏰".to_owned(),
        tier: Tier::Balanced,
        show_image: true,
        archived: false,
        gen_prompt: String::new(),
        public_md: format!("{name}的設定"),
        private_md: String::new(),
        stage: false,
    };
    data::write_character(root, world_id, &card).unwrap();
    card.id
}

fn stage_outcome(mode: Option<&str>, names: &[&str]) -> RefactorOutcome {
    serde_json::from_value(serde_json::json!({
        "mode": mode,
        "stage_cards": names,
    }))
    .unwrap()
}

fn stage_selection(stage_indices: Vec<usize>) -> RefactorSelection {
    RefactorSelection {
        stage_indices,
        ..no_player_selection(Vec::new())
    }
}

fn flags(root: &Path, world_id: &str, id: &str) -> StageFlags {
    data::character_stage_flags(root, world_id, id).unwrap()
}

const OFF: StageFlags = StageFlags {
    stage: false,
    auto_hidden: false,
};
const ON: StageFlags = StageFlags {
    stage: true,
    auto_hidden: false,
};

fn apply_recorded_summary(
    root: &Path,
    world_id: &str,
    outcome: &RefactorOutcome,
    selection: &RefactorSelection,
) -> RefactorApplySummary {
    apply_and_record(
        root,
        world_id,
        "zh-TW",
        outcome,
        selection,
        &[],
        crate::refactor::RefactorApplyOrigin::Card,
        &data::test_exclusive(world_id),
    )
    .unwrap()
}

/// 只勾舞台卡也能套用、留收據；撤銷把旗標退回。舞台卡標上時清掉自動隱藏，撤銷照收據前值寫回。
#[test]
fn stage_only_selection_applies_and_undo_restores_both_flags() {
    let root = TestRoot::new("stage-only");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let stage_id = seed_card(root.path(), &world_id, "灰岩要塞");
    let selection = stage_selection(vec![0]);
    assert!(!selection.is_empty());

    let summary = apply_recorded_summary(
        root.path(),
        &world_id,
        &stage_outcome(Some("characters"), &["灰岩要塞"]),
        &selection,
    );
    assert_eq!(summary.stage_cards, 1);
    assert_eq!(flags(root.path(), &world_id, &stage_id), ON);
    assert_eq!(
        receipts::list_import_receipts(root.path(), &world_id).len(),
        1
    );

    receipts::undo_last_import(root.path(), &world_id, &data::test_exclusive(&world_id)).unwrap();
    assert_eq!(flags(root.path(), &world_id, &stage_id), OFF);
    assert!(data::list_characters(root.path(), &world_id)
        .unwrap()
        .iter()
        .any(|meta| meta.id == stage_id));
}

/// 套用前是自動隱藏的卡：標舞台時清掉，撤銷時兩個旗標一起回到套用前。
#[test]
fn undo_restores_auto_hidden_from_receipt() {
    let root = TestRoot::new("stage-auto-hidden");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let stage_id = seed_card(root.path(), &world_id, "灰岩要塞");
    data::set_character_auto_hidden(root.path(), &world_id, &stage_id, true).unwrap();
    // 自動隱藏中的卡不算可發言角色，盤點與套用都不收——這裡先驗不收
    let summary = apply_recorded_summary(
        root.path(),
        &world_id,
        &stage_outcome(Some("characters"), &["灰岩要塞"]),
        &stage_selection(vec![0]),
    );
    assert_eq!(summary.stage_cards, 0);
    assert_eq!(
        flags(root.path(), &world_id, &stage_id),
        StageFlags {
            stage: false,
            auto_hidden: true,
        }
    );

    // 收據層：兩個旗標都等於 after 才寫回 before（含 auto_hidden）
    data::set_character_stage(root.path(), &world_id, &stage_id, true).unwrap();
    let before = StageFlags {
        stage: false,
        auto_hidden: true,
    };
    assert!(data::restore_stage_flags(root.path(), &world_id, &stage_id, ON, before).unwrap());
    assert_eq!(flags(root.path(), &world_id, &stage_id), before);
}

/// 套用後玩家手動改回角色：撤銷保留玩家的選擇，不把舞台蓋回去。
#[test]
fn undo_keeps_flags_changed_by_player_after_apply() {
    let root = TestRoot::new("stage-manual");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let stage_id = seed_card(root.path(), &world_id, "灰岩要塞");
    apply_recorded_summary(
        root.path(),
        &world_id,
        &stage_outcome(Some("characters"), &["灰岩要塞"]),
        &stage_selection(vec![0]),
    );
    data::set_character_stage(root.path(), &world_id, &stage_id, false).unwrap();
    data::set_character_auto_hidden(root.path(), &world_id, &stage_id, true).unwrap();

    receipts::undo_last_import(root.path(), &world_id, &data::test_exclusive(&world_id)).unwrap();
    assert_eq!(
        flags(root.path(), &world_id, &stage_id),
        StageFlags {
            stage: false,
            auto_hidden: true,
        }
    );
}

/// 第二張寫旗標失敗：第一張回滾，沒有收據、沒有存檔。
#[test]
fn second_stage_write_failure_rolls_back_first() {
    let root = TestRoot::new("stage-rollback");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let first = seed_card(root.path(), &world_id, "灰岩要塞");
    let second = seed_card(root.path(), &world_id, "旁白");
    let outcome = stage_outcome(Some("characters"), &["灰岩要塞", "旁白"]);
    let error = {
        let _guard = data::WriteFailGuard::partial_ending_after(".md.tmp", 1, 1);
        apply_and_record(
            root.path(),
            &world_id,
            "zh-TW",
            &outcome,
            &stage_selection(vec![0, 1]),
            &[],
            crate::refactor::RefactorApplyOrigin::Card,
            &data::test_exclusive(&world_id),
        )
        .unwrap_err()
        .to_string()
    };
    assert!(error.contains("injected partial write"), "{error}");
    assert_eq!(flags(root.path(), &world_id, &first), OFF);
    assert_eq!(flags(root.path(), &world_id, &second), OFF);
    assert!(receipts::list_import_receipts(root.path(), &world_id).is_empty());
    assert!(data::read_refactor_outcome(root.path(), &world_id)
        .unwrap()
        .is_none());
}

/// 跨桌：名字對不上、重名、玩家卡、封存的卡都跳過；這次新建的同名人物卡也對不到。
#[test]
fn unresolvable_names_are_skipped() {
    let root = TestRoot::new("stage-skip");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let twin_a = seed_card(root.path(), &world_id, "雙子");
    let twin_b = seed_card(root.path(), &world_id, "雙子");
    let archived = seed_card(root.path(), &world_id, "封存的");
    data::set_character_archived(root.path(), &world_id, &archived, true).unwrap();
    let player = seed_card(root.path(), &world_id, "玩家");
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    state.player_card_id = Some(player.clone());
    data::write_state(root.path(), &world_id, &state).unwrap();
    let uid = seed_entry(root.path(), &world_id, "新人", "新人的設定");

    let mut outcome = stage_outcome(
        Some("characters"),
        &["不存在", "雙子", "封存的", "玩家", "新人"],
    );
    outcome.characters = vec![character("新人", &[uid])];
    let summary = apply_recorded_summary(
        root.path(),
        &world_id,
        &outcome,
        &RefactorSelection {
            stage_indices: vec![0, 1, 2, 3, 4, 99],
            ..no_player_selection(vec![0])
        },
    );
    assert_eq!(summary.stage_cards, 0);
    assert_eq!(summary.new_characters, 1);
    for id in [&twin_a, &twin_b, &archived, &player] {
        assert!(!flags(root.path(), &world_id, id).stage);
    }
    assert!(data::list_characters(root.path(), &world_id)
        .unwrap()
        .iter()
        .all(|meta| !meta.stage));
}

/// 只有明確的 characters 玩法才套舞台：interface 或舊產物（沒標玩法）不動旗標。
#[test]
fn stage_cards_need_characters_mode() {
    let root = TestRoot::new("stage-mode");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let stage_id = seed_card(root.path(), &world_id, "灰岩要塞");
    for mode in [Some("interface"), None] {
        let uid = seed_entry(root.path(), &world_id, "設定", "內容");
        let mut outcome = stage_outcome(mode, &["灰岩要塞"]);
        outcome.characters = vec![character("路人", &[uid])];
        let summary = apply_recorded_summary(
            root.path(),
            &world_id,
            &outcome,
            &RefactorSelection {
                stage_indices: vec![0],
                ..no_player_selection(vec![0])
            },
        );
        assert_eq!(summary.stage_cards, 0);
        assert_eq!(flags(root.path(), &world_id, &stage_id), OFF);
    }
}

/// 舊重構卡（沒有 stage_cards 欄、選擇沒有 stage_indices）照舊讀得回來。
#[test]
fn legacy_outcome_and_selection_without_stage_fields_deserialize() {
    let outcome: RefactorOutcome = serde_json::from_str(r#"{"characters":[]}"#).unwrap();
    assert!(outcome.stage_cards.is_empty());
    let selection: RefactorSelection = serde_json::from_str(
        r#"{"character_indices":[],"apply_interface":false,"mechanism_indices":[],"entry_indices":[],"player_index":null}"#,
    )
    .unwrap();
    assert!(selection.stage_indices.is_empty());
    assert!(selection.is_empty());
}
