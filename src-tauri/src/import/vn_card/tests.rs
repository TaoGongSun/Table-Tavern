use super::export::vn_card_json;
use super::materialize::import_with;
use super::schema::{self, NoticeKind};
use super::*;
use crate::data::vn::{self, CastRole, Pov, RatingLevel};
use crate::data::{self, StateNode};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const FIXTURE: &str = include_str!("fixtures/harbor.json");

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).unwrap()
}

fn vn_of(card: &mut Value) -> &mut Value {
    card.pointer_mut("/data/extensions/table_tavern/vn")
        .unwrap()
}

fn bytes(card: &Value) -> Vec<u8> {
    serde_json::to_vec(card).unwrap()
}

fn import(label: &str, card: &Value) -> (PathBuf, VnCardImported) {
    let root = crate::smart_free::test_root(label);
    let imported = import_vn_card(&root, &bytes(card), true).unwrap();
    (root, imported)
}

fn kinds(imported: &VnCardImported) -> Vec<(NoticeKind, String)> {
    imported
        .notices
        .iter()
        .map(|notice| (notice.kind, notice.target.clone()))
        .collect()
}

fn id_of(root: &Path, world_id: &str, name: &str) -> String {
    data::list_characters(root, world_id)
        .unwrap()
        .into_iter()
        .map(|meta| (meta.id, meta.name))
        .chain(
            data::read_player_card(root, world_id)
                .unwrap()
                .map(|card| (card.id, card.name)),
        )
        .find(|(_, found)| found == name)
        .map(|(id, _)| id)
        .unwrap()
}

fn leaf(root: &Path, world_id: &str, path: &str) -> Option<String> {
    let state = data::read_state(root, world_id).unwrap();
    let path: Vec<String> = path.split('.').map(str::to_owned).collect();
    match data::node_at(&state.state.tree, &path) {
        Some(StateNode::Leaf(value)) => Some(value.clone()),
        _ => None,
    }
}

/// 整張卡的建桌結果：VN 桌、角色（不建作品名那張）、主角借玩家卡、世界書可見度重對應、開場、聖經套用、卡原件
#[test]
fn import_builds_the_whole_table() {
    let (root, imported) = import("vn-card-import", &fixture());
    let world_id = &imported.world_id;
    assert!(imported.notices.is_empty(), "{:?}", imported.notices);
    let state = data::read_state(&root, world_id).unwrap();
    assert_eq!(state.table_kind, data::TableKind::Vn);
    assert_eq!(state.name, "港口的燈");
    let names: Vec<String> = data::list_characters(&root, world_id)
        .unwrap()
        .into_iter()
        .map(|meta| meta.name)
        .collect();
    assert_eq!(names, ["米拉"]);
    let kai = data::read_player_card(&root, world_id).unwrap().unwrap();
    assert_eq!((kai.name.as_str(), kai.avatar.as_str()), ("凱", "🧭"));
    let mira = id_of(&root, world_id, "米拉");
    let book = data::read_worldbook(&root, world_id).unwrap();
    assert_eq!(
        book[1].visibility,
        data::Visibility::Characters(vec![mira.clone()])
    );
    assert_eq!(book[0].visibility, data::Visibility::Public);
    let events = data::read_transcript(&root, world_id, 0).unwrap();
    assert!(events[0].opening && events[0].text.starts_with("[bg: 碼頭"));
    assert!(events[0].state.is_some(), "套用聖經把初值寫進開場快照");

    let file = vn::read_vn(&root, world_id).unwrap();
    let bible = file.bible.clone().unwrap();
    assert!(bible.chapter_lock && bible.chapter_max == 3);
    assert_eq!(bible.secrets[0].holder.as_deref(), Some(mira.as_str()));
    assert_eq!(bible.secrets[1].holder.as_deref(), Some(kai.id.as_str()));
    let card = file.card.unwrap();
    assert_eq!(card.original_bible.as_ref(), Some(&bible));
    assert!(card.protagonist_fixed && card.pov == Some(Pov::First));
    assert_eq!(card.rating.level, RatingLevel::Teen);
    assert_eq!(card.cast[&mira].role, Some(CastRole::Memory));
    assert_eq!(
        card.cast[&mira]
            .art
            .appearance
            .as_ref()
            .map(|look| look.spec.as_str()),
        Some("young woman, lighthouse keeper")
    );
    assert_eq!(card.cast[&kai.id].art.height_cm, Some(180));
    assert_eq!(
        card.art.as_ref().map(|art| art.style.as_str()),
        Some("watercolor")
    );
    assert_eq!(card.locations[0].key, "pier");
    // 套用與包 4 生成同一支：規則、初值、增量協定
    let state = data::read_state(&root, world_id).unwrap();
    for (path, rule) in bible.rules() {
        assert_eq!(state.mechanism.rules.get(&path), Some(&rule), "{path}");
    }
    assert!(state.mechanism.incremental);
    assert_eq!(leaf(&root, world_id, "flags.trust").as_deref(), Some("0"));
    assert_eq!(
        leaf(&root, world_id, "revealed.letter").as_deref(),
        Some("false")
    );
    assert_eq!(leaf(&root, world_id, "chapter").as_deref(), Some("1"));
}

/// 驗證夾斷：壞 id、重複 id（留第一個，主角／秘密／可見度不接到別人）、壞色碼、超長、超量角色、未知欄位忽略
#[test]
fn schema_drops_and_clips_with_notices() {
    let mut card = fixture();
    let vn_value = vn_of(&mut card);
    let cast = vn_value["cast"].as_array_mut().unwrap();
    cast[1]["color"] = json!("red");
    cast[1]["public"] = json!("長".repeat(1_600));
    cast.push(json!({ "id": "Bad Id", "name": "壞" }));
    cast.push(json!({ "id": "mira", "name": "冒名", "public": "後來的同 id" }));
    for index in 0..8 {
        cast.push(json!({ "id": format!("extra{index}"), "name": format!("路人{index}") }));
    }
    let (root, imported) = import("vn-card-schema", &card);
    let notices = kinds(&imported);
    for expected in [
        (NoticeKind::ColorReset, "mira".to_owned()),
        (NoticeKind::Clipped, "mira.public".to_owned()),
        (NoticeKind::CastDropped, "Bad Id".to_owned()),
        (NoticeKind::CastDropped, "mira".to_owned()),
        (NoticeKind::CastOver, "2".to_owned()),
    ] {
        assert!(notices.contains(&expected), "{expected:?} in {notices:?}");
    }
    let world_id = &imported.world_id;
    let metas = data::list_characters(&root, world_id).unwrap();
    assert_eq!(metas.len(), 7, "8 人含主角");
    assert!(!metas.iter().any(|meta| meta.name == "冒名"));
    let mira = id_of(&root, world_id, "米拉");
    let card = data::read_character(&root, world_id, &mira).unwrap();
    assert_eq!(card.public_md.chars().count(), 1_500);
    assert_ne!(card.color, "red");
    let bible = vn::read_vn(&root, world_id).unwrap().bible.unwrap();
    assert_eq!(bible.secrets[0].holder.as_deref(), Some(mira.as_str()));
}

/// 懸空的主角、秘密持有者與可見角色：退玩家模式、設 null、剔除（剔空了退 GM）
#[test]
fn dangling_ids_fall_back() {
    let mut card = fixture();
    let vn_value = vn_of(&mut card);
    vn_value["protagonist"] = json!({ "mode": "fixed", "id": "ghost" });
    vn_value["bible"]["secrets"][0]["holder"] = json!("ghost");
    card["data"]["character_book"]["entries"][1]["extensions"]["table_tavern"]["visibility"] =
        json!({ "characters": ["ghost"] });
    let (root, imported) = import("vn-card-dangling", &card);
    let notices = kinds(&imported);
    assert!(notices.contains(&(NoticeKind::ProtagonistMissing, "ghost".to_owned())));
    assert!(notices.contains(&(NoticeKind::HolderDropped, "letter".to_owned())));
    assert!(notices.contains(&(NoticeKind::VisibilityDropped, "米拉的信".to_owned())));
    let world_id = &imported.world_id;
    assert!(data::read_player_card(&root, world_id).unwrap().is_none());
    let file = vn::read_vn(&root, world_id).unwrap();
    assert!(!file.card.unwrap().protagonist_fixed);
    assert_eq!(file.bible.unwrap().secrets[0].holder, None);
    assert_eq!(
        data::read_worldbook(&root, world_id).unwrap()[1].visibility,
        data::Visibility::Gm
    );
}

/// 拒收：沒有合格角色、版本不認得、整份超過上限（先丟美術欄位、還超過才拒收）
#[test]
fn rejects_without_cast_unknown_version_or_oversize() {
    let root = crate::smart_free::test_root("vn-card-reject");
    let refused = |card: &Value| {
        import_vn_card(&root, &bytes(card), true)
            .unwrap_err()
            .to_string()
    };
    let mut card = fixture();
    vn_of(&mut card)["cast"] = json!([{ "id": "", "name": "無 id" }]);
    assert!(refused(&card).contains(schema::NO_CAST));
    let mut card = fixture();
    vn_of(&mut card)["version"] = json!(2);
    assert!(refused(&card).contains(schema::UNSUPPORTED));
    let mut card = fixture();
    vn_of(&mut card)["world"] = json!("字".repeat(100_001));
    assert!(refused(&card).contains(schema::TOO_LARGE));
    assert!(data::list_worlds(&root).unwrap().is_empty());

    let mut card = fixture();
    vn_of(&mut card)["art"] = json!({ "style": "畫".repeat(100_001) });
    let (root, imported) = import("vn-card-extras-dropped", &card);
    assert!(kinds(&imported).contains(&(NoticeKind::ExtrasDropped, String::new())));
    let origin = vn::read_vn(&root, &imported.world_id)
        .unwrap()
        .card
        .unwrap();
    assert!(origin.art.is_none() && origin.locations.is_empty());
    assert!(origin
        .cast
        .values()
        .all(|cast| cast.art == Default::default()));
}

/// 探測：版本不認得標 unsupported；一般卡沒有 vn_card；只有 vn 的 table_tavern 擴充退普通卡匯入不報錯
#[test]
fn probe_and_plain_fallback() {
    let probe = crate::import::probe_import(FIXTURE.as_bytes())
        .vn_card
        .unwrap();
    assert!(probe.supported && probe.title == "港口的燈" && probe.cast == ["凱", "米拉"]);
    assert_eq!(probe.rating.warnings, ["grief"]);
    let mut card = fixture();
    vn_of(&mut card)["version"] = json!(9);
    assert!(
        !crate::import::probe_import(&bytes(&card))
            .vn_card
            .unwrap()
            .supported
    );
    assert!(crate::import::probe_import(br#"{"name":"a"}"#)
        .vn_card
        .is_none());

    let root = crate::smart_free::test_root("vn-card-plain");
    let world_id = data::create_world(&root, "普通桌").unwrap();
    let before = data::read_state(&root, &world_id).unwrap().mechanism;
    crate::import::import_character(&root, &world_id, &bytes(&card), "#336699", "zh-TW").unwrap();
    assert_eq!(
        data::read_state(&root, &world_id).unwrap().mechanism.rules,
        before.rules
    );
}

/// 建桌中途任何一步失敗都不留桌目錄
#[test]
fn failure_midway_leaves_no_table() {
    let root = crate::smart_free::test_root("vn-card-atomic");
    for step in ["characters", "worldbook", "vn", "bible", "cover"] {
        let error = import_with(
            &root,
            FIXTURE.as_bytes(),
            true,
            &mut |at| match at == step {
                true => Err(data::invalid_data("注入失敗")),
                false => Ok(()),
            },
        );
        assert!(error.is_err(), "{step}");
        assert!(data::list_worlds(&root).unwrap().is_empty(), "{step}");
        let left = std::fs::read_dir(root.join("worlds")).map_or(0, |dir| dir.count());
        assert_eq!(left, 0, "{step}");
    }
}

/// 匯出再匯入：聖經（持有者）、角色、可見度、開場、主角、語系分級人稱、逐角色欄位、卡層級美術欄位全部一致
#[test]
fn export_then_import_round_trips() {
    let mut card = fixture();
    let opening = vn_of(&mut card)["opening"].as_str().unwrap().to_owned();
    vn_of(&mut card)["opening"] = json!(format!("[exit: 艾琳]\n{opening}"));
    let (root, first) = import("vn-card-round", &card);
    let exported = vn_card_json(&root, &first.world_id, "en").unwrap();
    let vn_value = &exported["data"]["extensions"]["table_tavern"]["vn"];
    assert_eq!(
        vn_value["protagonist"],
        json!({ "mode": "fixed", "id": "c1" })
    );
    assert_eq!(vn_value["bible"]["secrets"][0]["holder"], json!("c2"));
    assert_eq!(vn_value["bible"]["chapter_lock"], json!(true));
    assert_eq!(
        (&vn_value["lang"], &vn_value["pov"]),
        (&json!("zh-TW"), &json!("first"))
    );
    assert_eq!(vn_value["cast"][1]["role"], json!("memory"));
    assert_eq!(
        vn_value["cast"][1]["expressions_extra"][0]["key"],
        json!("wistful")
    );
    assert_eq!(vn_value["art"], json!({ "style": "watercolor" }));
    assert_eq!(
        vn_value["locations"],
        json!([{ "slug": "pier", "name": "碼頭" }])
    );
    assert_eq!(vn_value["cast"][0]["height_cm"], json!(180));
    let entries = &exported["data"]["character_book"]["entries"];
    assert_eq!(
        entries[1]["extensions"]["table_tavern"]["visibility"],
        json!({ "characters": ["c2"] })
    );
    assert_eq!(exported["data"]["first_mes"], json!("海風吹過碼頭。"));

    let again = import_vn_card(&root, &bytes(&exported), true).unwrap();
    assert!(again.notices.is_empty(), "{:?}", again.notices);
    assert_eq!(
        vn_card_json(&root, &again.world_id, "en").unwrap(),
        exported
    );
}

/// 恢復卡內原版：聖經、初值、規則一起回到卡上那份；已遊玩拒絕
#[test]
fn restore_original_resets_and_only_before_play() {
    let (root, imported) = import("vn-card-restore", &fixture());
    let world_id = &imported.world_id;
    let original = vn::read_vn(&root, world_id).unwrap().bible.unwrap();
    let mut edited = original.clone();
    edited.logline = "改過".to_owned();
    crate::vn_bible::save(&root, world_id, edited).unwrap();
    let path = ["flags".to_owned(), "trust".to_owned()];
    data::update_state(&root, world_id, |state| {
        data::set_tree_value(&mut state.state.tree, &path, "3");
        Ok(Some(()))
    })
    .unwrap();
    crate::vn_bible::restore_original(&root, world_id).unwrap();
    assert_eq!(
        vn::read_vn(&root, world_id).unwrap().bible.unwrap(),
        original
    );
    assert_eq!(leaf(&root, world_id, "flags.trust").as_deref(), Some("0"));

    let mut player = crate::genesis::opening_event("走向燈塔");
    player.opening = false;
    player.kind = data::TranscriptKind::Player;
    data::append_transcript(&root, world_id, 0, &player).unwrap();
    assert_eq!(
        crate::vn_bible::restore_original(&root, world_id),
        Err(crate::vn_bible::PLAYED.to_owned())
    );
}

/// 章數鎖：存檔拒收章數變更，請求帶 chapter_lock: false 也解不了鎖
#[test]
fn chapter_lock_refuses_chapter_changes() {
    let (root, imported) = import("vn-card-lock", &fixture());
    let world_id = &imported.world_id;
    let mut edited = vn::read_vn(&root, world_id).unwrap().bible.unwrap();
    edited.chapter_lock = false;
    edited.chapter_max = 5;
    assert_eq!(
        crate::vn_bible::save(&root, world_id, edited.clone()),
        Err(crate::vn_bible::CHAPTER_LOCKED.to_owned())
    );
    edited.chapter_max = 3;
    edited.logline = "改主線".to_owned();
    crate::vn_bible::save(&root, world_id, edited).unwrap();
    let bible = vn::read_vn(&root, world_id).unwrap().bible.unwrap();
    assert!(bible.chapter_lock && bible.logline == "改主線");
}

/// 卡定主角：整份 GM 提示詞（總則、玩家段、回合尾）都不再禁止代演主角；none 與第一人稱各一句；出場限制進常駐段
#[test]
fn fixed_protagonist_gm_prompt() {
    let (root, imported) = import("vn-card-gm", &fixture());
    let world_id = &imported.world_id;
    let materials = crate::chat_assembly::gm_materials(&root, world_id).unwrap();
    let scan = crate::chat_assembly::test_gm_scan(&root, world_id, &materials, "zh-TW");
    let (scope, _) = crate::chat_assembly::gm_scope(&materials);
    let (instruction, _) =
        crate::chat_assembly::gm_instruction(&root, world_id, &materials, &scan, "zh-TW");
    let tail = instruction.content.clone();
    let system = crate::chat_assembly::gm_messages(&materials, &scan, &scope, instruction, "zh-TW")
        [0]
    .content
    .clone();
    assert!(
        system.contains("主角由你寫台詞、行動與內心獨白"),
        "{system}"
    );
    assert!(!system.contains("名單上的角色與玩家"), "{system}");
    assert!(system.contains("## 主角：凱") && system.contains("他不知道父親還活著"));
    assert!(
        system.contains("- 米拉: 只在回憶中出場；只在回憶中出現"),
        "{system}"
    );
    assert!(
        tail.contains("主角「凱」的台詞與內心獨白由你照劇本格式寫"),
        "{tail}"
    );
    assert!(!tail.contains("名單上的角色或玩家說話"), "{tail}");
    assert!(
        tail.contains("選項只改語氣") && tail.contains("第一人稱"),
        "{tail}"
    );
}

/// 沒有卡的 VN 桌：導演指示照舊，不加主角、人稱、線性句
#[test]
fn plain_vn_direction_is_unchanged() {
    let plain = crate::transport::vn_narrate_instruction(
        "zh-TW",
        &[],
        None,
        false,
        &crate::transport::VnDirection::default(),
    )
    .content;
    assert!(plain.contains("名單上的角色或玩家說話"));
    assert!(!plain.contains("選項只改語氣") && !plain.contains("人稱"));
}

/// 美術欄位：控制字元換空白、超長夾斷；壞鍵、撞固定表情或演出指令、變體叫 base 或名稱不合格、重複、超量、
/// 身高出界都整筆丟並記 notice
#[test]
fn art_fields_are_validated() {
    let mut notices = Vec::new();
    let mut expressions = vec![
        json!({ "key": "tender", "label": "溫柔", "prompt": "soft smile" }),
        json!({ "key": "happy", "label": "撞固定鍵" }),
        json!({ "key": "shake", "label": "撞指令" }),
        json!({ "key": "Bad Key" }),
        json!({ "key": "tender", "label": "重複" }),
    ];
    expressions.extend((1..=6).map(|index| json!({ "key": format!("e{index}") })));
    let member = json!({
        "appearance": { "spec": format!("tall\u{7}{}", "x".repeat(700)) },
        "expressions_extra": expressions,
        "variants": [
            { "key": "base", "label": "保留" },
            { "key": "festival", "label": "祭典(裝)" },
            { "key": "armor", "label": "Armor", "prompt": "plate armor" },
        ],
        "height_cm": 3000,
    });
    let art = super::art::cast_art(&member, "lak", &mut notices);
    let spec = &art.appearance.as_ref().unwrap().spec;
    assert!(spec.starts_with("tall x") && spec.chars().count() == 600);
    let keys: Vec<&str> = art
        .expressions
        .iter()
        .map(|entry| entry.key.as_str())
        .collect();
    assert_eq!(keys, ["tender", "e1", "e2", "e3", "e4", "e5"]);
    assert_eq!(art.expressions[1].label, "e1", "沒寫顯示名退回鍵");
    assert_eq!(art.variants.len(), 1);
    assert_eq!(art.variants[0].description(), "plate armor");
    assert_eq!(art.height_cm, None);
    let dropped: Vec<&str> = notices
        .iter()
        .filter(|notice| notice.kind == NoticeKind::ArtDropped)
        .map(|notice| notice.target.as_str())
        .collect();
    assert_eq!(
        dropped,
        [
            "lak.expressions_extra[1]",
            "lak.expressions_extra[2]",
            "lak.expressions_extra[3]",
            "lak.expressions_extra[4]",
            "lak.expressions_extra[10]",
            "lak.variants[0]",
            "lak.variants[1]",
            "lak.height_cm",
        ]
    );
    assert!(notices.contains(&schema::notice(NoticeKind::Clipped, "lak.appearance.spec")));

    let mut notices = Vec::new();
    let vn = json!({ "locations": [
        { "slug": "cave", "name": "洞穴", "times": ["moonlight", "Crisis", "moonlight", "calm"] },
        { "slug": "cave", "name": "重複" },
        { "slug": "../x", "name": "壞鍵" },
        { "slug": "dock", "name": "" },
    ]});
    let (art, locations) = super::art::card_art(&vn, &mut notices);
    assert!(art.is_none());
    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0].times, ["moonlight", "calm"]);
    assert_eq!(notices.len(), 5);
}
