//! 內建規則書（5e SRD 5.1 精簡裁決版）：匯入、十語系關鍵字觸發、只餵 GM，與注入實測（`--ignored`）。
use super::*;

const BOOK: &str = include_str!("../../../src/features/dice/rules/srd-5.1.json");

fn card(name: &str, public_md: &str) -> data::CharacterCard {
    data::CharacterCard {
        id: data::new_id(),
        name: name.to_owned(),
        color: "#336699".to_owned(),
        avatar: "🐗".to_owned(),
        tier: data::Tier::Balanced,
        show_image: true,
        archived: false,
        gen_prompt: String::new(),
        public_md: public_md.to_owned(),
        private_md: String::new(),
        stage: false,
    }
}

/// 一桌：玩家卡沃恩（雄性獸人戰士）、NPC 雷克（雄性獸人盜賊）；`book` 決定有沒有加內建書。
fn table(label: &str, book: bool) -> (std::path::PathBuf, String, data::CharacterCard) {
    let root = crate::smart_free::test_root(label);
    let world_id = data::create_world(&root, "灰岩酒館").unwrap();
    let vaughn = card("沃恩", "雄性獸人戰士，揮戰斧、拿圓盾。");
    let rake = card("雷克", "雄性獸人盜賊，帶彎刀與手弩，會一點毒術與法術。");
    data::write_character(&root, &world_id, &vaughn).unwrap();
    data::write_character(&root, &world_id, &rake).unwrap();
    data::set_player_card(&root, &world_id, Some(vaughn.id.clone())).unwrap();
    if book {
        let imported =
            data::import_worldbook_as(&root, &world_id, BOOK, &data::BookOwner::Gm).unwrap();
        assert_eq!(imported.summary.imported, 51);
        assert!(
            imported.summary.dice_detected,
            "書裡有 1d6 等骰式，要走骰子偵測提示"
        );
    }
    (root, world_id, rake)
}

fn append(root: &std::path::Path, world_id: &str, kind: &str, name: &str, text: &str) {
    let scene = data::read_state(root, world_id).unwrap().current_scene;
    let event: data::TranscriptEvent = serde_json::from_value(serde_json::json!({
        "ts": "2026-11-08T12:00:00", "speaker_id": "", "speaker_name": name, "kind": kind, "text": text
    }))
    .unwrap();
    data::append_transcript(root, world_id, scene, &event).unwrap();
}

fn book_titles() -> Vec<String> {
    let value: serde_json::Value = serde_json::from_str(BOOK).unwrap();
    value["entries"]
        .as_object()
        .unwrap()
        .values()
        .map(|entry| entry["comment"].as_str().unwrap().to_owned())
        .collect()
}

/// 觸發的規則條目（uid, order, 標題）
fn triggered(root: &std::path::Path, world_id: &str, materials: &GmMaterials) -> Vec<String> {
    let titles = book_titles();
    let scan = test_gm_scan(root, world_id, materials, "zh-TW");
    scan.placed
        .iter()
        .filter(|placed| titles.contains(&placed.title))
        .map(|placed| placed.title.clone())
        .collect()
}

/// 中文對話也要觸發得到英文條目（關鍵字收十語系說法）；署名條目永不注入。
#[test]
fn chinese_dialogue_triggers_english_rule_entries() {
    let (root, world_id, _) = table("rulebook-zh-trigger", true);
    append(
        &root,
        &world_id,
        "player",
        "沃恩",
        "我從側面偷襲，這樣有優勢嗎？",
    );
    let materials = gm_materials(&root, &world_id).unwrap();
    let hits = triggered(&root, &world_id, &materials);
    assert!(
        hits.contains(&"Advantage and Disadvantage".to_owned()),
        "{hits:?}"
    );
    assert!(hits.contains(&"Surprise".to_owned()), "{hits:?}");
    assert!(!hits
        .iter()
        .any(|title| title.starts_with("Source & License")));
}

/// 規則條目只餵 GM：角色線（雷克）掃同一段對話不得帶進任何規則內文。
#[test]
fn rule_entries_never_reach_character_lane() {
    let (root, world_id, rake) = table("rulebook-gm-only", true);
    append(
        &root,
        &world_id,
        "player",
        "沃恩",
        "我攻擊雷克，擲先攻後造成傷害，他要做豁免。",
    );
    let materials = gm_materials(&root, &world_id).unwrap();
    assert!(!triggered(&root, &world_id, &materials).is_empty());
    let scan = character_scan(
        &materials.book,
        &rake,
        &materials.cards,
        None,
        materials.player.as_ref(),
        &materials.events,
        Default::default(),
        None,
        MacroInputs::empty(),
        Randomness::Measure,
        "zh-TW",
    );
    let titles = book_titles();
    assert!(scan
        .placed
        .iter()
        .all(|placed| !titles.contains(&placed.title)));
    assert!(scan
        .snapshot_entries
        .iter()
        .all(|placed| !titles.contains(&placed.title)));
}

/// 一次 GM 請求的全文：(無狀態訊息串接, lane 凍結 system＋回合尾段)
fn request(
    root: &std::path::Path,
    world_id: &str,
    materials: &GmMaterials,
    dice: bool,
) -> (String, String) {
    let mut materials = materials.clone();
    materials.state.dice_enabled = dice;
    let scan = test_gm_scan(root, world_id, &materials, "zh-TW");
    let (scope, _) = gm_scope(&materials);
    let (mut instruction, closing) = gm_instruction(root, world_id, &materials, &scan, "zh-TW");
    if dice {
        instruction.content.push('\n');
        instruction.content.push_str(&crate::dice::instruction(
            &crate::dice::measured_pool(),
            "zh-TW",
        ));
    }
    let joined = format!("{}\n{closing}", instruction.content);
    let (frozen, turn) = gm_lane_parts(&materials, &scan, &scope, &joined, "zh-TW");
    let stateless: String = gm_messages(&materials, &scan, &scope, instruction, "zh-TW")
        .into_iter()
        .map(|message| message.content)
        .collect();
    (stateless, format!("{frozen}{}", turn.tail))
}

const BATTLE: [(&str, &str); 8] = [
    (
        "我拔出戰斧，盯著雷克。要開打了嗎？",
        "雷克抽出彎刀，戰鬥開始！兩人同時擲先攻，雷克搶先動作(d20#1=15)。",
    ),
    (
        "我擲先攻 1d20+2，然後衝上去砍他。",
        "沃恩的戰斧劈中雷克(d20#2=17)，造成傷害(d8#1=6)，雷克肩甲裂開。",
    ),
    (
        "我趁他踉蹌從側面再攻擊一次，這樣有優勢嗎？",
        "雷克躲到木箱後面取得掩護，反手射出一支弩箭(d20#3=12)，擦過圓盾。",
    ),
    (
        "我舉盾閃避，然後衝過去把他推倒。",
        "雷克被撞倒在地，倒地前朝你吐出一口毒霧，請做體質豁免。",
    ),
    (
        "豁免失敗了，我中毒了嗎？",
        "你中毒了，攻擊檢定有劣勢。雷克趴在地上念起咒語，準備施展法術。",
    ),
    (
        "我打斷他的施法，用戰斧重擊！",
        "大成功的暴擊！雷克的生命值降到 0，倒地昏迷。",
    ),
    (
        "我不殺他，幫他急救穩定傷勢。",
        "雷克的呼吸平穩下來，被你綁在木樁上。戰鬥結束。",
    ),
    ("我們找個地方紮營，長休一晚。", "夜深了，營火噼啪作響。"),
];

/// 注入實測：同一場戰鬥、同一組裝路徑，每回合列「有書－無書」與「擲骰開－關」的增量。
/// `cargo test rulebook_injection_measure -- --ignored --nocapture`
#[test]
#[ignore]
fn rulebook_injection_measure() {
    let (root_a, world_a, _) = table("rulebook-measure-book", true);
    let (root_b, world_b, _) = table("rulebook-measure-none", false);
    let tok = crate::scene_budget::budget_tokens;
    println!("回合|觸發條目|條數|無狀態 Δ字元/Δbytes/Δtoken|lane Δ字元/Δtoken|骰池 Δ字元/Δtoken（無狀態）");
    for (index, (player, gm)) in BATTLE.iter().enumerate() {
        for (root, world) in [(&root_a, &world_a), (&root_b, &world_b)] {
            append(root, world, "player", "沃恩", player);
        }
        let with = gm_materials(&root_a, &world_a).unwrap();
        let without = gm_materials(&root_b, &world_b).unwrap();
        let hits = triggered(&root_a, &world_a, &with);
        let (book_api, book_lane) = request(&root_a, &world_a, &with, false);
        let (none_api, none_lane) = request(&root_b, &world_b, &without, false);
        let (dice_api, _) = request(&root_b, &world_b, &without, true);
        let delta = |a: &str, b: &str| {
            (
                a.chars().count() as i64 - b.chars().count() as i64,
                a.len() as i64 - b.len() as i64,
                tok(a) as i64 - tok(b) as i64,
            )
        };
        let (api_chars, api_bytes, api_tok) = delta(&book_api, &none_api);
        let (lane_chars, _, lane_tok) = delta(&book_lane, &none_lane);
        let (dice_chars, _, dice_tok) = delta(&dice_api, &none_api);
        println!(
            "{}|{}|{}|{api_chars}/{api_bytes}/{api_tok}|{lane_chars}/{lane_tok}|{dice_chars}/{dice_tok}",
            index + 1,
            hits.join("、"),
            hits.len(),
        );
        for (root, world) in [(&root_a, &world_a), (&root_b, &world_b)] {
            append(root, world, "narration", "GM", gm);
        }
    }
}

/// 移除規則書走批次刪除：一次提交刪掉指定的條目，其餘（含玩家自己的）不動。
#[test]
fn batch_delete_removes_only_listed_entries() {
    let (root, world_id, _) = table("rulebook-remove", true);
    let before = data::read_worldbook(&root, &world_id).unwrap();
    let doomed: Vec<u64> = before.iter().take(3).map(|entry| entry.uid).collect();
    data::delete_worldbook_entries(&root, &world_id, &doomed).unwrap();
    let after = data::read_worldbook(&root, &world_id).unwrap();
    assert_eq!(after.len(), before.len() - 3);
    assert!(after.iter().all(|entry| !doomed.contains(&entry.uid)));
}
