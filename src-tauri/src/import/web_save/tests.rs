//! 網頁存檔匯入（包 3）：讀契約 fixture（src/shared/contracts/web-save/），驗四類落地、D16 可見度、D18 只補缺、
//! 失敗不留半桌與跨桌回滾、容量鎖驗收鏈、下一輪送出的 messages。
use super::*;
use crate::data::card_vars::read_layer;
use crate::data::message_vars::read_control;
use crate::data::{StateNode, Visibility, WorldbookEntry};
use crate::import::test_support::TestRoot;
use base64::Engine;
use serde_json::json;

const LANG: &str = "zh-TW";

pub(super) fn fixture(name: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../src/shared/contracts/web-save")
        .join(name);
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

pub(super) fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}

fn leaf(world: &data::WorldState, key: &str) -> Option<String> {
    match world.state.tree.get(key) {
        Some(StateNode::Leaf(value)) => Some(value.clone()),
        _ => None,
    }
}

fn entry<'a>(entries: &'a [WorldbookEntry], title: &str) -> &'a WorldbookEntry {
    entries
        .iter()
        .find(|entry| entry.title == title)
        .unwrap_or_else(|| panic!("沒有條目 {title}"))
}

pub(super) fn world_ids(root: &TestRoot) -> Vec<String> {
    data::list_worlds(root.path())
        .unwrap()
        .into_iter()
        .map(|world| world.id)
        .collect()
}

pub(super) fn layer_json(root: &TestRoot, world_id: &str, layer: Layer, id: Option<&str>) -> Value {
    serde_json::from_str(&read_layer(root.path(), world_id, layer, id).unwrap().vars).unwrap()
}

fn mode_of(root: &TestRoot, world_id: &str) -> Value {
    serde_json::to_value(read_control(root.path(), world_id).unwrap()).unwrap()["mode"].clone()
}

#[test]
fn short_save_lands_transcript_card_mvu_and_sidecar() {
    let root = TestRoot::new("web-save-short");
    let save = fixture("short.json");
    let imported = import_web_save(root.path(), &bytes(&save), LANG).unwrap();
    let w = imported.world_id.clone();
    let char_id = imported.character_id.clone().expect("角色卡路有角色");

    // 卡與玩家
    let character = data::read_character(root.path(), &w, &char_id).unwrap();
    assert_eq!(character.name, "灰燼旅店的莫拉");
    assert_eq!(
        data::read_player_card(root.path(), &w)
            .unwrap()
            .unwrap()
            .name,
        "旅人"
    );

    // 逐字稿：種類、說話者、截斷、事件 id 重配
    let events = data::read_transcript(root.path(), &w, 0).unwrap();
    let kinds: Vec<_> = events.iter().map(|event| event.kind.clone()).collect();
    assert_eq!(
        kinds,
        [
            TranscriptKind::Narration,
            TranscriptKind::Player,
            TranscriptKind::Dialogue,
            TranscriptKind::Player,
            TranscriptKind::Dialogue,
        ]
    );
    assert!(events[0].opening);
    assert_eq!(events[1].speaker_name, "旅人");
    assert_eq!(events[2].speaker_id, char_id);
    assert!(events[2]
        .raw
        .as_deref()
        .unwrap()
        .contains("<UpdateVariable>"));
    assert!(events[4].truncated);
    let ids: std::collections::HashSet<_> = events
        .iter()
        .map(|event| event.id.clone().unwrap())
        .collect();
    assert_eq!(ids.len(), 5);
    assert!(events
        .iter()
        .all(|event| event.id.as_deref().unwrap().len() == 26));

    // MVU：變數模式、新 epoch 掛在帶表事件上、有效表＝最後一張（錢包 15）
    let control = read_control(root.path(), &w).unwrap();
    assert_eq!(mode_of(&root, &w), json!("events"));
    let scene = control.scene(0).expect("第 0 幕有種子");
    let seed: Value = serde_json::from_str(scene.seed.text()).unwrap();
    assert_eq!(seed["stat_data"]["錢包"], json!(30));
    assert_eq!(control.macros.as_ref().unwrap().user, "旅人");
    for index in [0, 2, 4] {
        assert_eq!(
            events[index].vars_epoch.as_deref(),
            Some(scene.epoch.as_str())
        );
        assert!(events[index].vars_rev.is_some());
    }
    assert!(events[1].message_vars.is_none());
    let world = data::read_state(root.path(), &w).unwrap();
    assert_eq!(leaf(&world, "錢包").as_deref(), Some("15"));
    assert_eq!(leaf(&world, "名字").as_deref(), Some("旅人"));

    // 桌內三層
    assert_eq!(layer_json(&root, &w, Layer::Chat, None), json!({"好感": 1}));
    assert_eq!(
        layer_json(&root, &w, Layer::Character, Some(&char_id)),
        json!({"見過面": true})
    );
    assert_eq!(
        layer_json(&root, &w, Layer::Script, Some("card-view-mvu")),
        json!({"loaded": 1})
    );

    // 旁檔：世界書觸發狀態原樣、穩定 ID → UID、訊息 id → 事件 id
    let sidecar: Value = serde_json::from_slice(
        &std::fs::read(data::web_save_sidecar_path(root.path(), &w).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(sidecar["world_info"], save["world_info"]);
    assert_eq!(sidecar["regex_allowed"], json!(true));
    let book = data::read_worldbook(root.path(), &w).unwrap();
    let inn = entry(&book, "旅店");
    assert_eq!(sidecar["entry_uids"]["wi-inn"], json!(inn.uid));
    assert_eq!(
        sidecar["message_ids"]["m-4"],
        json!(events[4].id.clone().unwrap())
    );
    // 桌面版只保存：sticky 條目不會被改成 constant
    assert!(!inn.constant);

    // 卡片 storage 交回前端；這張卡的條目數
    assert_eq!(imported.card_storage["theme"], json!("dark"));
    assert_eq!(imported.worldbook_entries, 5);
}

#[test]
fn own_card_entries_become_character_visible_unless_the_card_says_otherwise() {
    let root = TestRoot::new("web-save-visibility");
    let imported = import_web_save(root.path(), &bytes(&fixture("short.json")), LANG).unwrap();
    let char_id = imported.character_id.unwrap();
    let book = data::read_worldbook(root.path(), &imported.world_id).unwrap();
    let mine = Visibility::Characters(vec![char_id]);
    for title in ["旅店", "莫拉的過去", "世界觀"] {
        assert_eq!(entry(&book, title).visibility, mine, "{title}");
    }
    // 原卡明示的可見度照原樣
    assert_eq!(entry(&book, "門口告示").visibility, Visibility::Public);
    // 機制鷹架照桌面版規則停用
    assert!(entry(&book, "[initvar]變數初始化勿開").disabled);
}

#[test]
fn next_character_turn_carries_constant_and_triggered_entries() {
    let root = TestRoot::new("web-save-next-turn");
    let imported = import_web_save(root.path(), &bytes(&fixture("short.json")), LANG).unwrap();
    let (w, char_id) = (imported.world_id, imported.character_id.unwrap());
    let card = data::read_character(root.path(), &w, &char_id).unwrap();
    let cards = crate::chat_assembly::active_cards(root.path(), &w).unwrap();
    let player = data::read_player_card(root.path(), &w).unwrap();
    let events = data::read_transcript(root.path(), &w, 0).unwrap();
    let book = data::read_worldbook(root.path(), &w).unwrap();
    let world = data::read_state(root.path(), &w).unwrap();
    let messages = crate::transport::assemble_shared_messages(
        &card,
        &cards,
        player.as_ref(),
        &events,
        &book,
        &world.state,
        &world.mechanism,
        None,
        LANG,
    );
    // 本輪尾段（實際送出的最後一則）：constant 條目、最近訊息提到「旅店」觸發的條目都在「只有他知道的世界情報」
    let tail = &messages.last().unwrap().content;
    let section = &tail[tail.find("知道的世界情報").expect("有限定條目段")..];
    assert!(section.contains("這是一個魔法逐漸消失的世界。"), "{tail}");
    assert!(section.contains("灰燼旅店在王都南門外"), "{tail}");
    // 沒被觸發的 keyword 條目不進這一段
    assert!(!section.contains("本店不賒帳"), "{tail}");
    assert!(!section.contains("莫拉年輕時當過傭兵。"), "{tail}");
}

#[test]
fn worldbook_route_keeps_gm_visibility_and_has_no_character() {
    let root = TestRoot::new("web-save-worldbook");
    let imported =
        import_web_save(root.path(), &bytes(&fixture("worldbook-route.json")), LANG).unwrap();
    let w = imported.world_id;
    assert!(imported.character_id.is_none());
    assert!(data::list_characters(root.path(), &w).unwrap().is_empty());
    assert!(data::read_player_card(root.path(), &w).unwrap().is_none());
    let book = data::read_worldbook(root.path(), &w).unwrap();
    assert_eq!(entry(&book, "世界觀").visibility, Visibility::Gm);
    let events = data::read_transcript(root.path(), &w, 0).unwrap();
    // 世界書路沒有角色：回覆是 GM 旁白
    assert_eq!(events[2].kind, TranscriptKind::Narration);
    assert_eq!(events[2].speaker_name, "GM");
    // 沒有卡片變數：模式不動
    assert_eq!(mode_of(&root, &w), json!("tree"));
    assert!(events.iter().all(|event| event.message_vars.is_none()));
}

#[test]
fn mvu_opening_seed_empty_table_and_no_table() {
    // 開場初始化：只有開場、沒有逐則表 → 有效表＝種子
    let root = TestRoot::new("web-save-mvu");
    let mut save = fixture("short.json");
    save["messages"] = json!([{
        "id": "m-open", "role": "char", "text": "開場", "ts": "2026-10-07T21:00:00+08:00", "opening": true
    }]);
    save["world_info"]["last_message_id"] = Value::Null;
    let opening = import_web_save(root.path(), &bytes(&save), LANG).unwrap();
    let world = data::read_state(root.path(), &opening.world_id).unwrap();
    assert_eq!(leaf(&world, "錢包").as_deref(), Some("30"));
    assert_eq!(mode_of(&root, &opening.world_id), json!("events"));

    // 空表：變數模式、種子是空物件
    let mut empty = save.clone();
    empty["mvu"]["seed"] = json!({});
    let empty = import_web_save(root.path(), &bytes(&empty), LANG).unwrap();
    let control = read_control(root.path(), &empty.world_id).unwrap();
    assert_eq!(mode_of(&root, &empty.world_id), json!("events"));
    assert_eq!(control.scene(0).unwrap().seed.text(), "{}");
    assert!(data::read_state(root.path(), &empty.world_id)
        .unwrap()
        .state
        .tree
        .get("錢包")
        .is_none());

    // 尚無表：mvu.seed 是 null → 模式不動、沒有控制檔
    let mut none = save.clone();
    none["mvu"]["seed"] = Value::Null;
    let none = import_web_save(root.path(), &bytes(&none), LANG).unwrap();
    assert_eq!(mode_of(&root, &none.world_id), json!("tree"));
    assert!(read_control(root.path(), &none.world_id)
        .unwrap()
        .scenes
        .is_empty());
}

#[test]
fn object_shaped_entries_map_stable_ids_in_uid_key_order() {
    let root = TestRoot::new("web-save-object");
    let mut save = fixture("short.json");
    save["card"]["data"]["character_book"]["entries"] = json!({
        "10": { "uid": 10, "key": ["十"], "comment": "十號", "content": "第十條", "constant": false, "disable": false },
        "2": { "uid": 2, "key": ["二"], "comment": "二號", "content": "第二條", "constant": true, "disable": false },
        "bad": "不是物件"
    });
    save["world_info"]["entries"] =
        json!([{ "id": "s-ten", "key": "10" }, { "id": "s-two", "key": "2" }]);
    let imported = import_web_save(root.path(), &bytes(&save), LANG).unwrap();
    let w = imported.world_id;
    let book = data::read_worldbook(root.path(), &w).unwrap();
    let (two, ten) = (entry(&book, "二號"), entry(&book, "十號"));
    assert!(two.uid < ten.uid, "照 uid 鍵的數字順序配發");
    let sidecar: Value = serde_json::from_slice(
        &std::fs::read(data::web_save_sidecar_path(root.path(), &w).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        sidecar["entry_uids"],
        json!({ "s-ten": ten.uid, "s-two": two.uid })
    );
    assert_eq!(imported.worldbook_entries, 2);
}

#[test]
fn rejects_unknown_versions_and_broken_saves_without_leaving_a_table() {
    let root = TestRoot::new("web-save-reject");
    let error = import_web_save(root.path(), &bytes(&fixture("future-version.json")), LANG)
        .unwrap_err()
        .to_string();
    assert!(error.contains("web_save_version"), "{error}");

    let mut broken = fixture("short.json");
    broken["messages"][1]["opening"] = json!(true);
    let error = import_web_save(root.path(), &bytes(&broken), LANG)
        .unwrap_err()
        .to_string();
    assert!(error.contains("web_save_invalid"), "{error}");

    // 必填可 null 的欄缺鍵也拒收（與前端檢查一致）
    let mut missing = fixture("short.json");
    missing.as_object_mut().unwrap().remove("mvu");
    assert!(import_web_save(root.path(), &bytes(&missing), LANG).is_err());

    let mut vars_without_seed = fixture("short.json");
    vars_without_seed["mvu"]["seed"] = Value::Null;
    assert!(import_web_save(root.path(), &bytes(&vars_without_seed), LANG).is_err());

    // 角色卡路但卡名有換行：桌面版這條路拒收，整份拒收
    let mut bad_card = fixture("short.json");
    bad_card["card"]["data"]["name"] = json!("兩\n行");
    assert!(import_web_save(root.path(), &bytes(&bad_card), LANG).is_err());

    assert!(world_ids(&root).is_empty(), "拒收不留任何桌");
}

#[test]
fn shared_layers_only_fill_missing_keys() {
    let root = TestRoot::new("web-save-d18");
    let other = data::create_world(root.path(), "原有的桌").unwrap();
    card_vars::fill_missing(
        root.path(),
        &other,
        Layer::Global,
        None,
        &message_vars::parse_table(r#"{"web_global_shared":"desktop-value"}"#).unwrap(),
        &mut |_| Ok(()),
    )
    .unwrap();
    let imported = import_web_save(root.path(), &bytes(&fixture("short.json")), LANG).unwrap();
    assert_eq!(
        layer_json(&root, &imported.world_id, Layer::Global, None),
        json!({"web_global_shared": "desktop-value", "web_global_new": "from-web"})
    );
    assert_eq!(
        layer_json(
            &root,
            &imported.world_id,
            Layer::Extension,
            Some("ext.example")
        ),
        json!({"theme": "dark"})
    );
    assert_eq!(imported.shared_kept, 1);
}

/// 先在別的桌把某個跨桌層寫成 `json`（模擬桌面版原本就有的值）。
pub(super) fn seed_layer(
    root: &TestRoot,
    world_id: &str,
    layer: Layer,
    id: Option<&str>,
    json: &str,
) {
    card_vars::fill_missing(
        root.path(),
        world_id,
        layer,
        id,
        &message_vars::parse_table(json).unwrap(),
        &mut |_| Ok(()),
    )
    .unwrap();
}

/// 照卡片寫入的同一條路：帶目前 rev 整張寫入。
pub(super) fn legit_write(
    root: &TestRoot,
    world_id: &str,
    layer: Layer,
    id: Option<&str>,
    edit: impl FnOnce(&mut Value),
) {
    let current = read_layer(root.path(), world_id, layer, id).unwrap();
    let mut table: Value = serde_json::from_str(&current.vars).unwrap();
    edit(&mut table);
    let generation = with_commit(root.path(), world_id, message_vars::generation);
    let written = card_vars::write_layer(
        root.path(),
        world_id,
        layer,
        id,
        generation,
        current.rev.as_deref(),
        &table.to_string(),
    )
    .unwrap();
    assert!(matches!(written, LayerWrite::LayerOk { .. }));
}

/// 具名反例：第一個跨桌層鍵寫入成功、後續步驟失敗 → 新桌不存在、跨桌層回到匯入前、
/// 匯入期間另一筆合法跨桌寫入（別的層）保留。
#[test]
fn failure_after_the_first_shared_key_rolls_back_but_keeps_concurrent_writes() {
    let root = TestRoot::new("web-save-rollback");
    let other = data::create_world(root.path(), "原有的桌").unwrap();
    seed_layer(
        &root,
        &other,
        Layer::Global,
        None,
        r#"{"web_global_shared":"desktop-value"}"#,
    );
    let mut first: Option<Filled> = None;
    let result = import_with(
        root.path(),
        &bytes(&fixture("short.json")),
        LANG,
        &mut |count, filled| {
            assert_eq!(count, 1, "第一個跨桌層之後就失敗");
            first = Some(filled.clone());
            legit_write(&root, &other, Layer::Preset, None, |table| {
                table["concurrent"] = json!("yes")
            });
            Err(data::invalid_data(
                "injected failure after the first shared key",
            ))
        },
    );
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("injected failure"),
        "清理做完就只報原錯：{error}"
    );
    assert!(!error.contains("web_save_cleanup_incomplete"), "{error}");
    let filled = first.expect("第一個跨桌層有寫進去");
    assert_eq!((filled.layer, filled.added.len()), (Layer::Global, 1));
    assert_eq!(world_ids(&root), vec![other.clone()], "新桌不存在");
    assert_eq!(
        layer_json(&root, &other, Layer::Global, None),
        json!({"web_global_shared": "desktop-value"})
    );
    assert_eq!(
        layer_json(&root, &other, Layer::Preset, None),
        json!({"concurrent": "yes"})
    );
    assert_eq!(
        layer_json(&root, &other, Layer::Extension, Some("ext.example")),
        json!({})
    );
}

/// 具名反例（ABA）：匯入補 k=v → 別桌合法改成別的值 → 又改回 v → 匯入失敗。rev 已變，回滾一個都不動，
/// 合法寫入保留；沒撤回的鍵與清理未完成一起回報。
#[test]
fn aba_write_between_fill_and_retract_is_preserved() {
    let root = TestRoot::new("web-save-aba");
    let other = data::create_world(root.path(), "原有的桌").unwrap();
    let result = import_with(
        root.path(),
        &bytes(&fixture("short.json")),
        LANG,
        &mut |_, filled| {
            assert_eq!(filled.layer, Layer::Global);
            legit_write(&root, &other, Layer::Global, None, |table| {
                table["web_global_new"] = json!("changed")
            });
            legit_write(&root, &other, Layer::Global, None, |table| {
                table["web_global_new"] = json!("from-web")
            });
            Err(data::invalid_data("injected failure"))
        },
    );
    let error = result.unwrap_err().to_string();
    assert!(error.contains("web_save_cleanup_incomplete"), "{error}");
    assert!(error.contains("global:web_global_new"), "{error}");
    assert!(error.contains("injected failure"), "{error}");
    assert_eq!(world_ids(&root), vec![other.clone()], "新桌照樣收掉");
    assert_eq!(
        layer_json(&root, &other, Layer::Global, None),
        json!({"web_global_new": "from-web", "web_global_shared": "web-value"})
    );
}

#[test]
fn retract_is_compare_and_set_on_the_layer_rev() {
    let root = TestRoot::new("web-save-retract");
    let w = data::create_world(root.path(), "桌").unwrap();
    let fill = |layer, id, json: &str| {
        card_vars::fill_missing(
            root.path(),
            &w,
            layer,
            id,
            &message_vars::parse_table(json).unwrap(),
            &mut |_| Ok(()),
        )
        .unwrap()
    };
    // 沒人動過：退完變空、原本沒有檔 → 刪檔回到「不存在」
    let filled = fill(Layer::Extension, Some("x"), r#"{"k":true}"#);
    assert!(filled.created && filled.rev.is_some());
    assert_eq!(
        card_vars::retract_filled(root.path(), &w, &filled).unwrap(),
        Vec::<String>::new()
    );
    assert_eq!(
        read_layer(root.path(), &w, Layer::Extension, Some("x"))
            .unwrap()
            .rev,
        None
    );
    // 匯入後別人寫過（哪怕寫了別的鍵）：rev 變了，一個都不退，回報沒撤回的鍵
    let filled = fill(Layer::Preset, None, r#"{"a":1}"#);
    legit_write(&root, &w, Layer::Preset, None, |table| {
        table["b"] = json!(2)
    });
    assert_eq!(
        card_vars::retract_filled(root.path(), &w, &filled).unwrap(),
        vec!["preset:a".to_owned()]
    );
    assert_eq!(
        layer_json(&root, &w, Layer::Preset, None),
        json!({"a": 1, "b": 2})
    );
    // 既有層只補了一部分：rev 沒變就只退補的鍵、原本的鍵留著
    seed_layer(&root, &w, Layer::Global, None, r#"{"old":0}"#);
    let filled = fill(Layer::Global, None, r#"{"old":9,"new":1}"#);
    assert_eq!(filled.added.len(), 1);
    card_vars::retract_filled(root.path(), &w, &filled).unwrap();
    assert_eq!(
        layer_json(&root, &w, Layer::Global, None),
        json!({"old": 0})
    );
}

/// 建桌途中 I/O 失敗：半成品目錄清掉，不留拿不到 id 的殘桌。
#[test]
fn a_world_creation_failure_leaves_no_directory() {
    let root = TestRoot::new("web-save-create-fail");
    {
        let _guard = data::WriteFailGuard::partial_ending("state.json", 1);
        assert!(import_web_save(root.path(), &bytes(&fixture("short.json")), LANG).is_err());
    }
    let worlds = root.path().join("worlds");
    let left: Vec<_> = std::fs::read_dir(&worlds)
        .map(|entries| entries.flatten().map(|entry| entry.file_name()).collect())
        .unwrap_or_default();
    assert!(left.is_empty(), "留下了 {left:?}");
}

/// 桌內落檔失敗一律回錯、整桌不留：世界書路的原卡、兩條路的機制寫入。
#[test]
fn in_table_write_failures_are_not_swallowed() {
    let root = TestRoot::new("web-save-strict");
    for (fixture_name, failing) in [
        ("worldbook-route.json", "source-card.import.json"),
        ("worldbook-route.json", "state.json.tmp"),
    ] {
        let _guard = data::WriteFailGuard::partial_ending(failing, 1);
        let result = import_web_save(root.path(), &bytes(&fixture(fixture_name)), LANG);
        assert!(result.is_err(), "{fixture_name} 的 {failing} 寫壞了卻成功");
    }
    // 角色卡路：沒有玩家名、旗標照舊時，第一個 state.json 原子寫入就是機制寫入
    let mut save = fixture("short.json");
    save["user_name"] = json!("");
    {
        let _guard = data::WriteFailGuard::partial_ending("state.json.tmp", 1);
        assert!(import_web_save(root.path(), &bytes(&save), LANG).is_err());
    }
    assert!(world_ids(&root).is_empty(), "整桌不留");
}

#[test]
fn every_shared_invalid_fixture_is_rejected_with_the_same_class() {
    let root = TestRoot::new("web-save-invalid");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../src/shared/contracts/web-save/invalid");
    let mut count = 0;
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let expected = match name.split("--").next().unwrap() {
            "version" => "web_save_version",
            "invalid" => "web_save_invalid",
            other => panic!("不認得的分類 {other}"),
        };
        let error = match import_web_save(root.path(), &std::fs::read(entry.path()).unwrap(), LANG)
        {
            Ok(_) => panic!("{name} 應該被拒收"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains(expected), "{name}: {error}");
        count += 1;
    }
    assert!(count >= 20);
    assert!(world_ids(&root).is_empty(), "拒收不留桌");
    // 那份負例的底本本身是合法的
    import_web_save(root.path(), &bytes(&fixture("minimal.json")), LANG).unwrap();
}

#[test]
fn the_three_size_limits_reject_without_leaving_a_table() {
    let root = TestRoot::new("web-save-limits");
    let mut too_big = bytes(&fixture("minimal.json"));
    too_big.resize(parse::MAX_SAVE_BYTES + 1, b' ');
    let mut too_many = fixture("minimal.json");
    too_many["messages"] = Value::Array(
        (0..50_001)
            .map(|index| json!({ "id": format!("m{index}"), "role": "user", "text": "x", "ts": "2026-10-07T21:00:00Z" }))
            .collect(),
    );
    too_many["mvu"] = Value::Null;
    let mut too_long = fixture("minimal.json");
    too_long["messages"][1]["text"] = json!("x".repeat(2 * 1024 * 1024 + 1));
    for (label, data) in [
        ("64 MiB", too_big),
        ("50,000 則", bytes(&too_many)),
        ("單則 2 MiB", bytes(&too_long)),
    ] {
        let error = import_web_save(root.path(), &data, LANG)
            .unwrap_err()
            .to_string();
        assert!(error.contains("web_save_invalid"), "{label}: {error}");
    }
    assert!(world_ids(&root).is_empty());
}

/// 長存檔匯入不擋容量；下一句被換幕容量鎖擋下 → 玩家換幕（摘要本身不被容量擋）→ 新幕實際送出下一句，
/// 新幕 MVU 種子＝舊幕最後一張對得上 epoch 的完整表，桌內三層與跨桌層都還在。
#[test]
fn long_save_hits_the_capacity_lock_then_advances_and_sends() {
    let root = TestRoot::new("web-save-capacity");
    let mut config = data::AppConfig::default();
    config.preferences.insert("transport".into(), json!("agy"));
    std::fs::write(
        root.path().join("config.json"),
        serde_json::to_string(&config).unwrap(),
    )
    .unwrap();
    let mut save = fixture("short.json");
    let last_table = save["messages"][4]["message_vars"].clone();
    let messages = save["messages"].as_array_mut().unwrap();
    messages.insert(
        3,
        json!({ "id": "m-long", "role": "char", "text": "雨".repeat(60_000), "ts": "2026-10-07T21:01:45+08:00" }),
    );
    let imported = import_web_save(root.path(), &bytes(&save), LANG).unwrap();
    let (w, char_id) = (imported.world_id, imported.character_id.unwrap());
    let (r, c) = (root.path(), root.path());

    let blocked = crate::scene_budget::check_capacity(c, r, &w, Some("next"), "我再說一句。");
    assert!(blocked.unwrap_err().contains("scene_capacity_full"));

    struct Summary;
    impl crate::scene_budget::summarize::SummaryCaller for Summary {
        async fn call(&mut self, _: Vec<crate::transport::ChatMessage>) -> Result<String, String> {
            Ok("旅人在灰燼旅店喝了湯與麥酒。".to_owned())
        }
    }
    let cap = crate::scene_budget::summary_capacity(c, r, &config);
    let scene = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(crate::scene_budget::summarize::advance_locked(
            r,
            &w,
            &cap,
            &mut Summary,
        ))
        .unwrap();
    assert_eq!(scene, 1);
    crate::scene_budget::check_capacity(c, r, &w, Some("after"), "我再說一句。").unwrap();

    // 實際送出下一句：玩家句與角色回覆照一般路徑落進新幕
    let player = TranscriptEvent {
        ts: "2026-10-07T22:00:00+08:00".to_owned(),
        speaker_id: String::new(),
        speaker_name: "旅人".to_owned(),
        kind: TranscriptKind::Player,
        text: "我再說一句。".to_owned(),
        ..event_template()
    };
    let reply = TranscriptEvent {
        speaker_id: char_id.clone(),
        speaker_name: "灰燼旅店的莫拉".to_owned(),
        kind: TranscriptKind::Dialogue,
        text: "「說吧。」".to_owned(),
        ..player.clone()
    };
    data::append_event(r, &w, 1, &player, None).unwrap();
    data::append_event(r, &w, 1, &reply, None).unwrap();
    let events = data::read_transcript(r, &w, 1).unwrap();
    assert!(events.iter().any(|event| event.text == "我再說一句。"));
    assert!(events.iter().any(|event| event.text == "「說吧。」"));

    // MVU：新幕種子＝舊幕最後一張完整表、新 epoch；有效狀態照舊
    let control = read_control(r, &w).unwrap();
    let (old, new) = (control.scene(0).unwrap(), control.scene(1).unwrap());
    assert_ne!(old.epoch, new.epoch);
    assert_eq!(
        serde_json::from_str::<Value>(new.seed.text()).unwrap(),
        last_table
    );
    let world = data::read_state(r, &w).unwrap();
    assert_eq!(leaf(&world, "錢包").as_deref(), Some("15"));
    // 桌內三層與跨桌層延續
    assert_eq!(layer_json(&root, &w, Layer::Chat, None), json!({"好感": 1}));
    assert_eq!(
        layer_json(&root, &w, Layer::Character, Some(&char_id)),
        json!({"見過面": true})
    );
    assert_eq!(
        layer_json(&root, &w, Layer::Script, Some("card-view-mvu")),
        json!({"loaded": 1})
    );
    assert_eq!(
        layer_json(&root, &w, Layer::Global, None)["web_global_new"],
        json!("from-web")
    );
    assert_eq!(
        layer_json(&root, &w, Layer::Extension, Some("ext.example")),
        json!({"theme": "dark"})
    );
}

fn event_template() -> TranscriptEvent {
    TranscriptEvent {
        ts: String::new(),
        speaker_id: String::new(),
        speaker_name: String::new(),
        kind: TranscriptKind::Player,
        text: String::new(),
        raw: None,
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
    }
}

#[test]
fn a_matching_png_brings_the_card_image() {
    let root = TestRoot::new("web-save-png");
    let mut save = fixture("short.json");
    let png = crate::import::test_support::card_png(&save["card"].to_string());
    save["card_png"] = json!(base64::engine::general_purpose::STANDARD.encode(&png));
    let imported = import_web_save(root.path(), &bytes(&save), LANG).unwrap();
    let image = crate::import::character_image(
        root.path(),
        &imported.world_id,
        &imported.character_id.unwrap(),
    );
    assert!(
        image.unwrap().is_some(),
        "PNG 與 card 一致：用 PNG 匯入、帶上卡圖"
    );
}

#[test]
fn a_mismatching_png_is_ignored_and_the_card_json_wins() {
    let root = TestRoot::new("web-save-png-mismatch");
    let mut save = fixture("short.json");
    let mut other_card = save["card"].clone();
    other_card["data"]["name"] = json!("別張卡");
    let png = crate::import::test_support::card_png(&other_card.to_string());
    save["card_png"] = json!(base64::engine::general_purpose::STANDARD.encode(&png));
    let imported = import_web_save(root.path(), &bytes(&save), LANG).unwrap();
    let char_id = imported.character_id.unwrap();
    let character = data::read_character(root.path(), &imported.world_id, &char_id).unwrap();
    assert_eq!(character.name, "灰燼旅店的莫拉", "以 card 為準");
    let image = crate::import::character_image(root.path(), &imported.world_id, &char_id).unwrap();
    assert!(image.is_none(), "不一致的 PNG 不拿來匯入");
}

/// 契約：內容重複的來源條目映到桌上保留的那一條。
#[test]
fn duplicate_entries_map_to_the_kept_uid() {
    let root = TestRoot::new("web-save-dedupe");
    let mut save = fixture("short.json");
    let entries = save["card"]["data"]["character_book"]["entries"]
        .as_array_mut()
        .unwrap();
    let copy = entries[0].clone();
    entries.push(copy);
    save["world_info"]["entries"] =
        json!([{ "id": "first", "key": "0" }, { "id": "dup", "key": "5" }]);
    let imported = import_web_save(root.path(), &bytes(&save), LANG).unwrap();
    let sidecar: Value = serde_json::from_slice(
        &std::fs::read(data::web_save_sidecar_path(root.path(), &imported.world_id).unwrap())
            .unwrap(),
    )
    .unwrap();
    assert!(sidecar["entry_uids"]["first"].is_u64());
    assert_eq!(sidecar["entry_uids"]["dup"], sidecar["entry_uids"]["first"]);
}

/// D24：regex_allowed=false 的存檔 → 桌旗標 false、卡片介面不套顯示腳本；true 照套。
/// 桌面版本來就沒有送模前的 regex，兩種旗標下角色線送出的歷史都是原文（佔位標記不被拿掉）。
#[test]
fn regex_refusal_becomes_a_table_flag_that_drops_display_scripts() {
    let root = TestRoot::new("web-save-d24");
    let allowed = import_web_save(root.path(), &bytes(&fixture("short.json")), LANG).unwrap();
    let mut refused_save = fixture("short.json");
    refused_save["regex_allowed"] = json!(false);
    let refused = import_web_save(root.path(), &bytes(&refused_save), LANG).unwrap();

    let flag = |w: &str| data::read_state(root.path(), w).unwrap().regex_allowed;
    assert!(flag(&allowed.world_id));
    assert!(!flag(&refused.world_id));
    let scripts = |w: &str| {
        crate::import::read_card_interfaces(root.path(), w)
            .unwrap()
            .iter()
            .map(|card| card.scripts.len())
            .sum::<usize>()
    };
    assert!(scripts(&allowed.world_id) > 0, "允許：照套顯示腳本");
    assert_eq!(scripts(&refused.world_id), 0, "不允許：顯示腳本整組不套");
    let state_before: Value = serde_json::from_str(
        &std::fs::read_to_string(
            root.path()
                .join("worlds")
                .join(&allowed.world_id)
                .join("state.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(
        state_before.get("regex_allowed").is_none(),
        "一般桌不寫這欄（預設 true）"
    );

    for imported in [&allowed, &refused] {
        let sent = character_turn(
            &root,
            &imported.world_id,
            imported.character_id.as_deref().unwrap(),
        );
        let history: String = sent[1..sent.len() - 1]
            .iter()
            .map(|m| m.content.clone())
            .collect();
        assert!(
            history.contains("<StatusPlaceHolderImpl/>"),
            "送模前不跑 promptOnly 腳本"
        );
    }
}

fn character_turn(root: &TestRoot, w: &str, char_id: &str) -> Vec<crate::transport::ChatMessage> {
    let card = data::read_character(root.path(), w, char_id).unwrap();
    let cards = crate::chat_assembly::active_cards(root.path(), w).unwrap();
    let player = data::read_player_card(root.path(), w).unwrap();
    let events = data::read_transcript(root.path(), w, 0).unwrap();
    let book = data::read_worldbook(root.path(), w).unwrap();
    let world = data::read_state(root.path(), w).unwrap();
    crate::transport::assemble_shared_messages(
        &card,
        &cards,
        player.as_ref(),
        &events,
        &book,
        &world.state,
        &world.mechanism,
        None,
        LANG,
    )
}

/// 世界書路：沒有角色，下一輪是 GM 線；constant 與最近訊息觸發的條目都進提示。
#[test]
fn worldbook_route_next_gm_turn_carries_constant_and_triggered_entries() {
    let root = TestRoot::new("web-save-wb-turn");
    let imported =
        import_web_save(root.path(), &bytes(&fixture("worldbook-route.json")), LANG).unwrap();
    let materials = crate::chat_assembly::gm_materials(root.path(), &imported.world_id).unwrap();
    let (scope, _) = crate::chat_assembly::gm_scope(&materials);
    let messages = crate::transport::assemble_gm_messages(
        &materials.world_md,
        &materials.cards,
        materials.player.as_ref(),
        &materials.events,
        &materials.worldbook,
        &materials.state.state,
        &materials.state.mechanism,
        &scope,
        LANG,
    );
    let sent: String = messages
        .iter()
        .map(|message| message.content.clone())
        .collect();
    assert!(sent.contains("這是一個魔法逐漸消失的世界。"), "constant");
    assert!(sent.contains("灰燼旅店在王都南門外"), "最近訊息提到旅店");
    assert!(!sent.contains("本店不賒帳"), "沒被觸發的不進");
}

/// 世界書路＋非空 MVU：character 層不落地，原樣留在旁檔；其餘層照常。
#[test]
fn worldbook_route_keeps_the_character_layer_only_in_the_sidecar() {
    let root = TestRoot::new("web-save-wb-mvu");
    let imported = import_web_save(
        root.path(),
        &bytes(&fixture("worldbook-route-mvu.json")),
        LANG,
    )
    .unwrap();
    let w = imported.world_id;
    let sidecar: Value = serde_json::from_slice(
        &std::fs::read(data::web_save_sidecar_path(root.path(), &w).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(sidecar["character_layer"], json!({"見過面": true}));
    assert!(!root
        .path()
        .join("worlds")
        .join(&w)
        .join("card-vars")
        .join("character")
        .exists());
    assert_eq!(layer_json(&root, &w, Layer::Chat, None), json!({"好感": 1}));
    assert_eq!(mode_of(&root, &w), json!("events"));
}
