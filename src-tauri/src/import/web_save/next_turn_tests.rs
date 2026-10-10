//! 網頁存檔端對端（worldbook-st-trigger-parity 方案四之 4）：匯入 `web-export-world-info.json`、補上網頁版
//! 同一句玩家句，角色視角下一輪的觸發結果（內文進提示的條目、掃完的計時表）等於網頁版跑出的
//! `src/shared/contracts/world-info/web-save-next-turn.json`。
use super::tests::{bytes, fixture};
use super::*;
use crate::import::test_support::TestRoot;
use std::collections::BTreeSet;

const LANG: &str = "zh-TW";

fn expected() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../src/shared/contracts/world-info/web-save-next-turn.json");
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn next_character_turn_matches_the_web_version() {
    let expected = expected();
    let root = TestRoot::new("web-save-next-turn-parity");
    let save = fixture(expected["save"].as_str().unwrap());
    let imported = import_web_save(root.path(), &bytes(&save), LANG).unwrap();
    let (w, char_id) = (imported.world_id, imported.character_id.unwrap());
    let sidecar: Value = serde_json::from_slice(
        &std::fs::read(data::web_save_sidecar_path(root.path(), &w).unwrap()).unwrap(),
    )
    .unwrap();
    let uid_of = |stable: &str| sidecar["entry_uids"][stable].as_u64().unwrap();
    let player = data::read_player_card(root.path(), &w).unwrap();
    let user = TranscriptEvent {
        speaker_name: save["user_name"].as_str().unwrap().to_owned(),
        kind: TranscriptKind::Player,
        text: expected["nextUser"].as_str().unwrap().to_owned(),
        ..data::read_transcript(root.path(), &w, 0).unwrap()[1].clone()
    };
    data::append_transcript(root.path(), &w, 0, &user).unwrap();
    let events = data::read_transcript(root.path(), &w, 0).unwrap();
    let card = data::read_character(root.path(), &w, &char_id).unwrap();
    let scan = crate::chat_assembly::test_character_scan(
        root.path(),
        &w,
        &card,
        player.as_ref(),
        &events,
        LANG,
    );

    let placed: BTreeSet<u64> = scan.placed.iter().map(|entry| entry.uid).collect();
    let wanted: BTreeSet<u64> = expected["inPrompt"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| uid_of(id.as_str().unwrap()))
        .collect();
    assert_eq!(placed, wanted);

    let as_uids = |table: &Value| -> Value {
        Value::Object(
            table
                .as_object()
                .unwrap()
                .iter()
                .map(|(stable, effect)| (uid_of(stable).to_string(), effect.clone()))
                .collect(),
        )
    };
    // 落地存的就是這個形狀（start／end 整數）
    let timed =
        serde_json::to_value(data::world_info_store::StoredTimed::from_scan(&scan.timed)).unwrap();
    assert_eq!(timed["sticky"], as_uids(&expected["timed"]["sticky"]));
    assert_eq!(timed["cooldown"], as_uids(&expected["timed"]["cooldown"]));
}
