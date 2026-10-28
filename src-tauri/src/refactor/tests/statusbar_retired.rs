//! gm-reprints-retired-statusbar：characters 重構真的淘汰了卡片狀態欄格式才標記退場，撤銷還原；
//! 退場桌送進提示詞的卡拿掉開場白段，卡檔不動。
use super::super::test_support::*;
use super::super::*;
use crate::data;
use crate::receipts;
use crate::refactor_assemble::RefactorDroppedEntry;

fn outcome(mode: &str, dropped: Vec<RefactorDroppedEntry>) -> RefactorOutcome {
    RefactorOutcome {
        mode: Some(mode.to_owned()),
        characters: Vec::new(),
        interface: None,
        mechanisms: Vec::new(),
        entries: Vec::new(),
        deletable_shared_uids: Vec::new(),
        dropped,
        unabsorbed: Vec::new(),
        audit: Vec::new(),
        preserve_source_uids: Vec::new(),
        source_fingerprints: Default::default(),
        mirrors: Default::default(),
        stage_cards: Vec::new(),
    }
}

fn dropped(uid: u64, span: &str, statusbar: bool) -> RefactorDroppedEntry {
    RefactorDroppedEntry {
        uid: uid.to_string(),
        span: span.to_owned(),
        title: "天數與狀態欄".to_owned(),
        content: "<details><summary>状态栏</summary>…</details>".to_owned(),
        rule: 5,
        statusbar,
    }
}

fn retired(root: &std::path::Path, world_id: &str) -> bool {
    data::read_state(root, world_id).unwrap().statusbar_retired
}

#[test]
fn characters_statusbar_drop_marks_retired_and_undo_restores() {
    let root = TestRoot::new("statusbar-retired");
    let world_id = data::create_world(root.path(), "要塞").unwrap();
    let uid = seed_entry(root.path(), &world_id, "天數與狀態欄", "段一\n\n段二");
    let product = outcome("characters", vec![dropped(uid, &format!("{uid}#s2"), true)]);
    apply_recorded(
        root.path(),
        &world_id,
        &product,
        &no_player_selection(Vec::new()),
    );
    assert!(retired(root.path(), &world_id));

    receipts::undo_last_import(root.path(), &world_id, &data::test_exclusive(&world_id)).unwrap();
    assert!(!retired(root.path(), &world_id));
}

#[test]
fn only_statusbar_drops_on_characters_tables_count() {
    for (mode, flag) in [("characters", false), ("interface", true)] {
        let root = TestRoot::new("statusbar-not-retired");
        let world_id = data::create_world(root.path(), "要塞").unwrap();
        let uid = seed_entry(root.path(), &world_id, "介面", "<UI>整頁介面</UI>");
        // characters 桌淘汰的是可完全遊玩的介面（statusbar=false）、interface 桌的狀態欄段都不算
        let product = outcome(mode, vec![dropped(uid, "", flag)]);
        apply_recorded(
            root.path(),
            &world_id,
            &product,
            &no_player_selection(Vec::new()),
        );
        assert!(!retired(root.path(), &world_id), "{mode}");
    }
}

#[test]
fn retired_tables_send_cards_without_greetings() {
    let root = TestRoot::new("statusbar-retired-cards");
    let world_id = data::create_world(root.path(), "要塞").unwrap();
    let card = data::CharacterCard {
        id: "01ARZ3NDEKTSV4RRFFQ69G5FAW".to_owned(),
        name: "灰岩要塞".to_owned(),
        color: "#336699".to_owned(),
        avatar: "🏰".to_owned(),
        tier: data::Tier::Balanced,
        show_image: true,
        archived: false,
        gen_prompt: String::new(),
        public_md: "### 簡介\n北境要塞\n\n### 開場白\n風雪\n<details><summary>状态栏</summary>天數</details>\n\n### 場景\n冬季".to_owned(),
        private_md: "玩家筆記\n\n### 備用開場白 1\n城門\n<details><summary>状态栏</summary>第 1 天</details>".to_owned(),
        stage: false,
    };
    data::write_character(root.path(), &world_id, &card).unwrap();
    let sent = |root: &std::path::Path| {
        crate::chat_assembly::active_cards(root, &world_id)
            .unwrap()
            .remove(0)
    };
    assert_eq!(
        sent(root.path()).public_md,
        card.public_md,
        "沒退場的桌照原樣送"
    );

    let mut state = data::read_state(root.path(), &world_id).unwrap();
    state.statusbar_retired = true;
    data::write_state(root.path(), &world_id, &state).unwrap();
    let stripped = sent(root.path());
    assert_eq!(stripped.public_md, "### 簡介\n北境要塞\n\n### 場景\n冬季");
    assert_eq!(stripped.private_md, "玩家筆記");
    // 卡檔不動：匯出、編輯照舊讀得到開場白
    let stored = data::read_character(root.path(), &world_id, &card.id).unwrap();
    assert_eq!(stored.public_md, card.public_md);
    assert_eq!(stored.private_md, card.private_md);
}
