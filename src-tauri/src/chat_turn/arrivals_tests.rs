use super::{load_hidden_cards, record_card_arrivals, record_person_arrivals};
use crate::commands::{character_card, NEXT_TEMP_ID};
use crate::data;
use std::sync::atomic::Ordering;

/// AI 卡重構包 4a 規格 (c)(d)(e)：present 有新面孔就把世界書全文 append 成一則系統事件；
/// 同一幕重複比對不重複 append；換幕（新場景號、空 events）同名要重新 append 一次。
#[test]
fn record_person_arrivals_appends_once_per_scene_and_resets_on_new_scene() {
    let root = std::env::temp_dir().join(format!(
        "table-tavern-person-arrivals-{}-{}",
        std::process::id(),
        NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let world_id = data::create_world(&root, "測試桌").unwrap();

    let alice = data::WorldbookEntry {
        uid: 1,
        title: "愛麗絲".to_owned(),
        keys: Vec::new(),
        content: "愛麗絲是旅店老闆娘。".to_owned(),
        constant: true,
        order: 0,
        disabled: false,
        visibility: data::Visibility::Public,
        is_person: true,
        locked: false,
    };
    let worldbook = [alice];

    // 第一輪：present 有愛麗絲 → append 一則登場事件
    record_person_arrivals(
        &root,
        &world_id,
        0,
        &worldbook,
        &[],
        &[],
        Some("愛麗絲"),
        "",
        &crate::transport::test_support::plain_fill("阿濤"),
        None,
        Some("act-1"),
    );
    let scene0 = data::read_transcript(&root, &world_id, 0).unwrap();
    assert_eq!(scene0.len(), 1);
    // 登場紀錄蓋上觸發它的動作 id（換幕容量預測切段用）
    assert_eq!(scene0[0].action_id.as_deref(), Some("act-1"));
    assert_eq!(scene0[0].kind, data::TranscriptKind::System);
    assert_eq!(scene0[0].speaker_id, "");
    assert_eq!(scene0[0].speaker_name, "GM");
    assert_eq!(
        scene0[0].marker,
        Some(data::EventMarker::PersonArrival {
            title: "愛麗絲".to_owned()
        })
    );
    assert!(scene0[0].text.contains("愛麗絲是旅店老闆娘。"));

    // 第二輪：present 還是愛麗絲，本幕 events 已含前一則登場事件 → 不重複
    record_person_arrivals(
        &root,
        &world_id,
        0,
        &worldbook,
        &[],
        &scene0,
        Some("愛麗絲"),
        "",
        &crate::transport::test_support::plain_fill("阿濤"),
        None,
        None,
    );
    assert_eq!(data::read_transcript(&root, &world_id, 0).unwrap().len(), 1);

    // 換幕：scene 1 是新 jsonl、events 是空的 → 同名重新 append
    record_person_arrivals(
        &root,
        &world_id,
        1,
        &worldbook,
        &[],
        &[],
        Some("愛麗絲"),
        "",
        &crate::transport::test_support::plain_fill("阿濤"),
        None,
        None,
    );
    let scene1 = data::read_transcript(&root, &world_id, 1).unwrap();
    assert_eq!(scene1.len(), 1);
    assert!(matches!(
        &scene1[0].marker,
        Some(data::EventMarker::PersonArrival { title }) if title == "愛麗絲"
    ));

    std::fs::remove_dir_all(&root).unwrap();
}

/// 回歸候選只看自動隱藏的人物：舞台卡（就算舊桌留著 auto_hidden、或已封存）不回歸、也不進折疊比對名字池
/// （與換幕結算同一規則）；封存的人物照舊留在名字池
#[test]
fn hidden_cards_skip_stage_cards() {
    let root = crate::smart_free::test_root("hidden-cards-stage");
    let world_id = data::create_world(&root, "測試桌").unwrap();
    let stage = character_card(&data::new_id(), "灰岩要塞");
    let fox = character_card(&data::new_id(), "狐狸");
    let owl = character_card(&data::new_id(), "貓頭鷹");
    let tower = character_card(&data::new_id(), "黑塔");
    let ghost = character_card(&data::new_id(), "亡靈");
    for card in [&stage, &fox, &owl, &tower, &ghost] {
        data::write_character(&root, &world_id, card).unwrap();
    }
    data::set_character_stage(&root, &world_id, &stage.id, true).unwrap();
    data::set_character_stage(&root, &world_id, &tower.id, true).unwrap();
    data::set_character_archived(&root, &world_id, &tower.id, true).unwrap();
    data::set_character_archived(&root, &world_id, &ghost.id, true).unwrap();
    data::set_character_auto_hidden(&root, &world_id, &stage.id, true).unwrap();
    data::set_character_auto_hidden(&root, &world_id, &fox.id, true).unwrap();
    let (hidden, others) = load_hidden_cards(&root, &world_id, false).unwrap();
    assert_eq!(
        hidden
            .iter()
            .map(|card| card.name.as_str())
            .collect::<Vec<_>>(),
        ["狐狸"]
    );
    assert_eq!(others, ["貓頭鷹", "亡靈"]);
    let _ = std::fs::remove_dir_all(&root);
}

/// AI 卡重構包 4b，鏡射 4a：present 有隱藏卡的名字就 append 回歸事件——私設先成一則
/// gm_only 事件、公開設定再成一則（card-arrival-private-leak）；同一幕重複比對不重複
/// append；不改 auto_hidden 欄位本身（鐵律，只在換幕結算與退回／分岔復原動）。
#[test]
fn record_card_arrivals_appends_once_per_scene_and_does_not_touch_auto_hidden() {
    let root = std::env::temp_dir().join(format!(
        "table-tavern-card-arrivals-{}-{}",
        std::process::id(),
        NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let world_id = data::create_world(&root, "測試桌").unwrap();

    let mut fox = character_card(&data::new_id(), "狐狸");
    fox.public_md = "尾巴很大。".to_owned();
    fox.private_md = "其實是{{user}}的仇人。".to_owned();
    data::write_character(&root, &world_id, &fox).unwrap();
    data::set_character_auto_hidden(&root, &world_id, &fox.id, true).unwrap();
    let mut owl = character_card(&data::new_id(), "貓頭鷹");
    owl.public_md = "夜裡才醒。".to_owned();
    data::write_character(&root, &world_id, &owl).unwrap();
    data::set_character_auto_hidden(&root, &world_id, &owl.id, true).unwrap();
    let hidden_cards = vec![fox.clone(), owl.clone()];

    // 第一輪：present 有狐狸與貓頭鷹 → 狐狸私設＋公開兩則，貓頭鷹沒私設只有公開一則
    let ids = record_card_arrivals(
        &root,
        &world_id,
        0,
        &hidden_cards,
        &[],
        &[],
        Some("狐狸、貓頭鷹"),
        "",
        &crate::transport::test_support::plain_fill("阿濤"),
        None,
        Some("act-2"),
    );
    assert_eq!(ids, vec![fox.id.clone(), owl.id.clone()]);
    let scene0 = data::read_transcript(&root, &world_id, 0).unwrap();
    assert_eq!(scene0.len(), 3);
    // 卡片登場（含私設）全部蓋上觸發它的動作 id
    assert!(scene0
        .iter()
        .all(|event| event.action_id.as_deref() == Some("act-2")));
    assert!(scene0
        .iter()
        .all(|event| event.kind == data::TranscriptKind::System));
    assert!(scene0[0].gm_only);
    let full = |event: &data::TranscriptEvent| data::event_full_text(event, "zh-TW");
    assert_eq!(
        full(&scene0[0]),
        "（角色私設）〈狐狸〉\n私有設定：\n其實是阿濤的仇人。"
    );
    assert!(!scene0[1].gm_only);
    assert_eq!(
        full(&scene0[1]),
        "（角色回歸）〈狐狸〉\n公開設定：\n尾巴很大。"
    );
    assert!(!scene0[2].gm_only);
    assert_eq!(
        full(&scene0[2]),
        "（角色回歸）〈貓頭鷹〉\n公開設定：\n夜裡才醒。"
    );

    // 第二輪：present 還是狐狸，本幕 events 已含前一則回歸事件 → 不重複
    record_card_arrivals(
        &root,
        &world_id,
        0,
        &hidden_cards,
        &[],
        &scene0,
        Some("狐狸、貓頭鷹"),
        "",
        &crate::transport::test_support::plain_fill("阿濤"),
        None,
        None,
    );
    assert_eq!(data::read_transcript(&root, &world_id, 0).unwrap().len(), 3);

    // 不碰 auto_hidden 欄位本身：磁碟上仍是 true，要等換幕結算才會變 false
    let meta = data::list_characters(&root, &world_id)
        .unwrap()
        .into_iter()
        .find(|meta| meta.id == fox.id)
        .unwrap();
    assert!(meta.auto_hidden);

    std::fs::remove_dir_all(&root).unwrap();
}

/// AI 卡重構包 4b：visibility 非 Public 的世界書人物條目登場時，事件帶 gm_only=true
/// （chars 續聊線只看得到前綴那一行，不洩漏全文）；這是 4a 遺留的洩漏修正。
#[test]
fn record_person_arrivals_marks_gm_only_for_non_public_visibility() {
    let root = std::env::temp_dir().join(format!(
        "table-tavern-person-gm-only-{}-{}",
        std::process::id(),
        NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let world_id = data::create_world(&root, "測試桌").unwrap();

    let spy = data::WorldbookEntry {
        uid: 1,
        title: "密探".to_owned(),
        keys: Vec::new(),
        content: "其實是反派的眼線。".to_owned(),
        constant: true,
        order: 0,
        disabled: false,
        visibility: data::Visibility::Gm,
        is_person: true,
        locked: false,
    };
    let worldbook = [spy];

    record_person_arrivals(
        &root,
        &world_id,
        0,
        &worldbook,
        &[],
        &[],
        Some("密探"),
        "",
        &crate::transport::test_support::plain_fill("阿濤"),
        None,
        None,
    );
    let scene0 = data::read_transcript(&root, &world_id, 0).unwrap();
    assert_eq!(scene0.len(), 1);
    assert!(scene0[0].gm_only);

    std::fs::remove_dir_all(&root).unwrap();
}
