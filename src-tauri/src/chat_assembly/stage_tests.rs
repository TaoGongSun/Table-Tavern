//! 舞台卡（stage-card-in-speaker-list）：集中分流、點名、在場判定、拒收與兩條提示詞的「舞台」段。
use super::*;

fn card(name: &str, public_md: &str, private_md: &str) -> data::CharacterCard {
    data::CharacterCard {
        id: data::new_id(),
        name: name.to_owned(),
        color: "#336699".to_owned(),
        avatar: "🏰".to_owned(),
        tier: data::Tier::Balanced,
        show_image: true,
        archived: false,
        gen_prompt: String::new(),
        public_md: public_md.to_owned(),
        private_md: private_md.to_owned(),
        stage: false,
    }
}

fn world_files(dir: &std::path::Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(world_files(&path));
        } else {
            files.push((path.clone(), std::fs::read(&path).unwrap()));
        }
    }
    files.sort();
    files
}

/// 一桌：舞台卡「灰岩要塞」排第一、角色「托伦」第二；回傳 (root, world_id, 舞台, 托伦)。
fn table(
    label: &str,
) -> (
    std::path::PathBuf,
    String,
    data::CharacterCard,
    data::CharacterCard,
) {
    let root = crate::smart_free::test_root(label);
    let world_id = data::create_world(&root, "灰岩要塞").unwrap();
    let stage = card("灰岩要塞", "北境山口的要塞。", "地窖藏著密道。");
    let toren = card("托伦", "要塞指揮官。", "怕冷。");
    data::write_character(&root, &world_id, &stage).unwrap();
    data::write_character(&root, &world_id, &toren).unwrap();
    data::set_character_stage(&root, &world_id, &stage.id, true).unwrap();
    (root, world_id, stage, toren)
}

/// 點名名單不含舞台；模型寫「灰岩要塞的指揮官托伦」也解成托伦（退路取最先出現的候選，舞台不在候選裡）
#[test]
fn stage_is_never_nominated() {
    let mut stage = card("灰岩要塞", "", "");
    stage.stage = true;
    let toren = card("托伦", "", "");
    let cards = [stage, toren.clone()];
    assert_eq!(speaker_roster(&cards), ["托伦"]);
    assert_eq!(
        next_speaker(false, &cards, Some("灰岩要塞的指揮官托伦".to_owned()), None).as_deref(),
        Some(toren.id.as_str())
    );
    assert_eq!(
        next_speaker(false, &cards, Some("灰岩要塞".to_owned()), None),
        None
    );
}

/// GM：導演指示的下一位名單只有托伦；system 裡舞台卡在「舞台」段（含私設，GM 看得到），不在「登場角色」段
#[test]
fn gm_prompt_puts_stage_in_its_own_section() {
    let (root, world_id, _, _) = table("stage-gm-prompt");
    let materials = gm_materials(&root, &world_id).unwrap();
    assert_eq!(materials.cards.len(), 2, "舞台卡的設定照送");
    let scan = test_gm_scan(&root, &world_id, &materials, "zh-TW");
    let (instruction, _) = gm_instruction(&root, &world_id, &materials, &scan, "zh-TW");
    assert!(
        instruction.content.contains("最適合發言的角色：托伦。"),
        "{}",
        instruction.content
    );
    let (scope, _) = gm_scope(&materials);
    let system = gm_messages(&materials, &scan, &scope, instruction, "zh-TW")[0]
        .content
        .clone();
    let stage_at = system.find("## 舞台").expect("有舞台段");
    let cast_at = system.find("## 登場角色").expect("有登場角色段");
    let stage_part = &system[stage_at..cast_at];
    assert!(stage_part.contains("### 灰岩要塞"), "{system}");
    assert!(stage_part.contains("北境山口的要塞。") && stage_part.contains("地窖藏著密道。"));
    assert!(!system[cast_at..].contains("### 灰岩要塞"), "{system}");
    assert!(system[cast_at..].contains("### 托伦"));
    let _ = std::fs::remove_dir_all(&root);
}

/// 角色線：舞台卡的公開設定在「舞台」段，私設不進；「登場角色」名單只有托伦
#[test]
fn character_lane_puts_stage_public_setting_in_its_own_section() {
    let (root, world_id, _, toren) = table("stage-chars-prompt");
    let cards = active_cards(&root, &world_id).unwrap();
    let toren = cards.iter().find(|card| card.id == toren.id).unwrap();
    let scan = test_character_scan(&root, &world_id, toren, None, &[], "zh-TW");
    let system = transport::chars_lane_system(&cards, None, &scan, "zh-TW");
    let stage_at = system.find("## 舞台").expect("有舞台段");
    let cast_at = system.find("## 登場角色").expect("有登場角色段");
    assert!(
        system[stage_at..cast_at].contains("### 灰岩要塞\n北境山口的要塞。"),
        "{system}"
    );
    assert!(!system.contains("地窖藏著密道。"), "舞台私設不進角色線");
    assert!(!system[cast_at..].contains("灰岩要塞"), "{system}");
    let _ = std::fs::remove_dir_all(&root);
}

/// 「1 角色＋1 舞台」算單角色桌：{{char}}／世界書帶名字掃描用托伦、Claude 單人線成立；
/// 舞台卡就算被收進隱藏區（舊桌）也照送設定、不算在場
#[test]
fn one_character_plus_stage_counts_as_solo() {
    let (root, world_id, stage, toren) = table("stage-solo");
    data::set_character_auto_hidden(&root, &world_id, &stage.id, true).unwrap();
    let metas = data::list_characters(&root, &world_id).unwrap();
    assert_eq!(sole_card_name(&metas, &[]).as_deref(), Some("托伦"));
    assert!(sole_present(&metas, &toren.id, &[]));
    let cards = active_cards(&root, &world_id).unwrap();
    assert!(cards.iter().any(|card| card.id == stage.id && card.stage));
    let _ = std::fs::remove_dir_all(&root);
}

/// 舞台卡的角色回合在任何副作用之前拒收、不動任何檔；一般角色放行
#[test]
fn stage_character_turn_is_rejected_without_side_effects() {
    let (root, world_id, stage, toren) = table("stage-reject");
    let before = world_files(&root);
    let error = reject_stage_character_turn(&root, &world_id, &stage.id).unwrap_err();
    assert!(error.contains("stage_card_no_turn"), "{error}");
    assert_eq!(world_files(&root), before);
    assert!(reject_stage_character_turn(&root, &world_id, &toren.id).is_ok());
    let _ = std::fs::remove_dir_all(&root);
}

/// API 單人判定：「1 角色＋1 舞台」比照單卡，私設提進 system（與 lane 的 sole_present、用量 solo 同一判定）；
/// 兩位發言角色時私設留在尾段
#[test]
fn api_shared_messages_treat_one_character_plus_stage_as_solo() {
    let (root, world_id, _, toren) = table("stage-api-solo");
    let state = data::read_state(&root, &world_id).unwrap();
    let system_of = |cards: &[data::CharacterCard]| {
        let toren = cards.iter().find(|card| card.id == toren.id).unwrap();
        let scan = test_character_scan(&root, &world_id, toren, None, &[], "zh-TW");
        transport::assemble_shared_messages(
            toren,
            cards,
            None,
            &[],
            &scan,
            &state.state,
            &state.mechanism,
            None,
            "zh-TW",
        )[0]
        .content
        .clone()
    };
    let cards = active_cards(&root, &world_id).unwrap();
    assert!(system_of(&cards).contains("怕冷。"), "單人：私設進 system");
    let mut both_speak = cards.clone();
    for card in &mut both_speak {
        card.stage = false;
    }
    assert!(
        !system_of(&both_speak).contains("怕冷。"),
        "兩位發言角色：私設不進 system"
    );
    let _ = std::fs::remove_dir_all(&root);
}
