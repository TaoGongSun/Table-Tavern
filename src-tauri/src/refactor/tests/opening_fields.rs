//! statusbar-only-fields：卡片狀態欄退場的非 MVU 桌，開場白狀態欄的每一格照酒館追蹤——補每回合規則、
//! 前綴剝進顯示前綴表、物件序列轉成以名字為鍵的集合、巢狀鏡像、沒有規則與觸發也開回報。
use super::super::test_support::*;
use super::super::*;
use crate::data::{self, FieldKind, FieldRule, InjectLevel, StateNode};
use crate::refactor_assemble::RefactorDroppedEntry;
use crate::transport::StateBlock;
use crate::{import, receipts};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::Path;

/// 北境驛站（NorthHall-structure）的開場白狀態欄原文
const NORTH_OPENING: &str = "<maintext>\n林远牵着瘦马走进北境驿站。\n</maintext>\n<Status_block>\n状态栏:\n  日期和时间: \"⏰ 大雍永和三年十月初七 酉时\"\n  地点: \"📍 北境驿站 前院\"\n  天气: \"🌨️ 北风小雪 零下八度\"\n  驿站物资:\n    粮草: \"🌾 账载四百四十石 实存待查\"\n    马匹: \"🐎 在册四十 实有三十七\"\n  人物列表:\n    - 人物:\n        名字: \"👤 林远\"\n        身份: \"🪪 新任驿丞\"\n        行动: \"📝 牵马站在院中，盯着墙上的驿马簿\"\n        装备: \"🧥 青色旧官袍、铜印、记事小册\"\n        体力: \"💪 55 赶了三天路，疲惫\"\n        心情: \"🙂 警觉\"\n        内心: \"💭 少了三匹马，账也未必对得上。\"\n    - 人物:\n        名字: \"👤 贺铮\"\n        身份: \"🪪 老驿长\"\n        行动: \"📝 倚着门框抽烟斗，打量新人\"\n        装备: \"🧥 旧羊皮袄、铁木拐杖\"\n        体力: \"💪 70 腿伤畏寒\"\n        心情: \"🙂 冷淡\"\n        内心: \"💭 又一个熬不过冬天的文官。\"\n    - 人物:\n        名字: \"👤 周掌柜\"\n        身份: \"🪪 军需官\"\n        行动: \"📝 捧着热茶迎上前\"\n        装备: \"🧥 厚棉袍、算盘、钥匙串\"\n        体力: \"💪 80\"\n        心情: \"🙂 殷勤\"\n        内心: \"💭 今晚得先把仓房那本账收好。\"\n  行动选项:\n    名字: \"👤 林远\"\n    选项:\n      - \"1. 接过热茶，先去驿丞房安顿，明早按规矩交接\"\n      - \"2. 当场请周掌柜打开马厩，核对马匹与驿马簿\"\n      - \"3. 冒雪走到大门外，看清那支无旗号商队的来历\"\n      - \"4. 向贺铮请教北境的规矩，顺便打听丢马的事\"\n</Status_block>";

pub(super) const PEOPLE: &str = "状态栏.人物列表";

fn card(extensions: serde_json::Value) -> String {
    json!({"spec":"chara_card_v3","data":{
        "name":"北境驿站","first_mes":"十月初七。","extensions": extensions,
        "character_book":{"name":"驿站","entries": [{"keys":[],"constant":true,"content":"每回合輸出 <Status_block> YAML","comment":"美化状态栏","enabled":true}]}
    }})
    .to_string()
}

/// 開場白貼上桌（`post_opening_text` 真流程：解析狀態區塊落樹／平欄）。
pub(super) fn post_opening(root: &Path, world_id: &str, text: &str, source: Option<&str>) {
    import::post_opening_text(
        root,
        world_id,
        0,
        "2026-10-28T19:00:00",
        text,
        "zh-TW",
        Some(0),
        source,
        &data::test_exclusive(world_id),
    )
    .unwrap();
}

/// 北境驛站：世界書卡（非 MVU）匯入後貼開場白，樹上長出 状态栏.*
pub(super) fn north_world(root: &Path) -> String {
    let world_id = data::create_world(root, "北境驛站").unwrap();
    let source = import::import_worldbook_file(
        root,
        &world_id,
        card(json!({})).as_bytes(),
        "驿站",
        &data::test_exclusive(&world_id),
    )
    .unwrap()
    .source;
    post_opening(root, &world_id, NORTH_OPENING, source.as_deref());
    world_id
}

fn statusbar_drop(uid: u64) -> RefactorDroppedEntry {
    RefactorDroppedEntry {
        uid: uid.to_string(),
        span: format!("{uid}#s1"),
        title: "美化状态栏".to_owned(),
        content: "<Status_block>…</Status_block>".to_owned(),
        rule: 5,
        statusbar: true,
    }
}

/// characters 產物：淘汰狀態欄（退場桌）、可帶機制條目的規則與觸發、盤點 MIRRORS。
pub(super) fn outcome(
    uid: u64,
    rules: BTreeMap<String, FieldRule>,
    triggers: Vec<data::Trigger>,
    mirrors: &[(&str, &str)],
) -> RefactorOutcome {
    let entries = if rules.is_empty() && triggers.is_empty() {
        Vec::new()
    } else {
        vec![crate::refactor_ai::RefactorNewEntry {
            title: "驿站规则".to_owned(),
            kind: "mechanism".to_owned(),
            content: "说明".to_owned(),
            source_uids: Vec::new(),
            rules,
            triggers,
            meta: None,
        }]
    };
    RefactorOutcome {
        mode: Some("characters".to_owned()),
        characters: Vec::new(),
        interface: None,
        mechanisms: Vec::new(),
        entries,
        deletable_shared_uids: Vec::new(),
        dropped: vec![statusbar_drop(uid)],
        unabsorbed: Vec::new(),
        audit: Vec::new(),
        preserve_source_uids: Vec::new(),
        source_fingerprints: Default::default(),
        mirrors: mirrors
            .iter()
            .map(|(name, key)| ((*name).to_owned(), (*key).to_owned()))
            .collect(),
        stage_cards: Vec::new(),
        fields: Vec::new(),
    }
}

pub(super) fn selection(entries: bool) -> RefactorSelection {
    RefactorSelection {
        entry_indices: if entries { vec![0] } else { Vec::new() },
        ..no_player_selection(Vec::new())
    }
}

pub(super) fn path(text: &str) -> Vec<String> {
    text.split('.').map(str::to_owned).collect()
}

pub(super) fn leaf<'a>(state: &'a data::WorldState, at: &str) -> Option<&'a str> {
    match data::node_at(&state.state.tree, &path(at)) {
        Some(StateNode::Leaf(value)) => Some(value),
        _ => None,
    }
}

pub(super) fn first_uid(root: &Path, world_id: &str) -> u64 {
    data::read_worldbook(root, world_id).unwrap()[0].uid
}

fn apply_north(root: &Path, mirrors: &[(&str, &str)]) -> (String, data::WorldState) {
    let world_id = north_world(root);
    let uid = first_uid(root, &world_id);
    apply_recorded(
        root,
        &world_id,
        &outcome(uid, BTreeMap::new(), Vec::new(), mirrors),
        &selection(false),
    );
    let state = data::read_state(root, &world_id).unwrap();
    (world_id, state)
}

pub(super) fn gm_update(
    state: &mut data::WorldState,
    patch_json: &str,
) -> crate::mechanism::Outcome {
    let block = StateBlock {
        fields: Vec::new(),
        updates: vec![format!("<JSONPatch>{patch_json}</JSONPatch>")],
        display: String::new(),
        lists: Vec::new(),
        maps: Vec::new(),
        fenced: None,
    };
    crate::mechanism::apply_block(state, &block, "林远")
}

pub(super) fn fence(state: &mut data::WorldState, fields: &[(&str, &str)]) {
    let block = StateBlock {
        fields: fields
            .iter()
            .map(|(key, value)| (vec![(*key).to_owned()], (*value).to_owned()))
            .collect(),
        updates: Vec::new(),
        display: String::new(),
        lists: Vec::new(),
        maps: Vec::new(),
        fenced: None,
    };
    crate::mechanism::apply_block(state, &block, "林远");
}

/// 北境驛站退場桌：沒有規則也沒有觸發也開回報；每格都有每回合注入的規則；人物序列轉成以名字為鍵的集合、
/// 名字欄選中「名字」；行動選項登記成序列集合；前綴剝進顯示前綴表；巢狀地點收成鏡像。
#[test]
fn north_opening_cells_are_all_tracked() {
    let root = TestRoot::new("opening-north");
    let (_, state) = apply_north(root.path(), &[("状态栏.地点", "place")]);
    let mechanism = &state.mechanism;
    assert!(mechanism.incremental && mechanism.incremental_by_refactor);

    // 人物序列：以名字為鍵、名字欄不存、依原序補 `_序`
    let spec = &mechanism.collections[PEOPLE];
    assert!(spec.keyed);
    assert!(!spec.fields.contains_key("名字"));
    assert_eq!(leaf(&state, "状态栏.人物列表.林远.身份"), Some("新任驿丞"));
    assert_eq!(
        leaf(&state, "状态栏.人物列表.贺铮.体力"),
        Some("70 腿伤畏寒")
    );
    assert_eq!(leaf(&state, "状态栏.人物列表.周掌柜._序"), Some("2"));
    assert!(data::node_at(&state.state.tree, &path("状态栏.人物列表.0")).is_none());
    for field in ["身份", "行动", "装备", "体力", "心情", "内心"] {
        let rule = &mechanism.rules[&format!("{PEOPLE}.*.{field}")];
        assert_eq!(rule.inject, InjectLevel::Turn, "{field}");
    }

    // 行動選項：純量序列集合，整批換
    let options = &mechanism.collections["状态栏.行动选项.选项"];
    assert!(!options.keyed);
    assert!(mechanism.rules.contains_key("状态栏.行动选项.选项.*"));
    assert_eq!(leaf(&state, "状态栏.行动选项.名字"), Some("林远"));

    // 前綴：每項一樣存萬用，單格存精確
    let prefixes = &mechanism.display_prefixes;
    assert_eq!(prefixes[&format!("{PEOPLE}.*")], "👤 ");
    assert_eq!(prefixes[&format!("{PEOPLE}.*.身份")], "🪪 ");
    assert_eq!(prefixes["状态栏.天气"], "🌨\u{fe0f} ");
    assert_eq!(leaf(&state, "状态栏.天气"), Some("北风小雪 零下八度"));

    // 開場格子一律每回合注入：非對齊回合也送給 GM
    for cell in ["状态栏.天气", "状态栏.驿站物资.粮草", "状态栏.日期和时间"] {
        assert_eq!(mechanism.rules[cell].kind, FieldKind::Text, "{cell}");
        assert_eq!(mechanism.rules[cell].inject, InjectLevel::Turn, "{cell}");
        assert!(
            crate::mechanism::leaf_shown(mechanism, &path(cell), "x", false),
            "{cell}"
        );
    }
    assert!(crate::mechanism::leaf_shown(
        mechanism,
        &path("状态栏.人物列表.林远.心情"),
        "x",
        false
    ));

    // 巢狀鏡像：收進鏡像表、拿掉規則、不列給 GM
    assert_eq!(mechanism.mirrors["状态栏.地点"], "place");
    assert!(!mechanism.rules.contains_key("状态栏.地点"));
}

/// 集合更新走得通：新增人物（值帶前綴照剝）、刪除人物、改單一欄位、改名（精確前綴跟著搬）、行動選項整批換、
/// 整份重排；寫到不存在的編號項目被拒收並回饋現有項目名。
#[test]
fn collection_updates_add_remove_rename_reorder_and_replace_options() {
    let root = TestRoot::new("opening-collection");
    let (_, mut state) = apply_north(root.path(), &[]);
    state
        .mechanism
        .display_prefixes
        .insert(format!("{PEOPLE}.贺铮.行动"), "🚬 ".to_owned());

    gm_update(
        &mut state,
        r#"[
          {"op":"insert","path":"/状态栏/人物列表/秦岚","value":{"身份":"🪪 斥候队长","行动":"翻身下马","装备":"弓","体力":"90","心情":"警觉","内心":"马蹄印不对"}},
          {"op":"remove","path":"/状态栏/人物列表/周掌柜"},
          {"op":"replace","path":"/状态栏/人物列表/林远/行动","value":"📝 推开仓房的门"},
          {"op":"move","from":"/状态栏/人物列表/贺铮","path":"/状态栏/人物列表/老贺"},
          {"op":"replace","path":"/状态栏/行动选项/选项","value":["1. 清点粮草","2. 追问铁三"]}
        ]"#,
    );
    assert_eq!(leaf(&state, "状态栏.人物列表.秦岚.身份"), Some("斥候队长"));
    assert!(data::node_at(&state.state.tree, &path("状态栏.人物列表.周掌柜")).is_none());
    assert_eq!(
        leaf(&state, "状态栏.人物列表.林远.行动"),
        Some("推开仓房的门")
    );
    assert_eq!(leaf(&state, "状态栏.人物列表.老贺.身份"), Some("老驿长"));
    let prefixes = &state.mechanism.display_prefixes;
    assert_eq!(prefixes[&format!("{PEOPLE}.老贺.行动")], "🚬 ");
    assert!(!prefixes.contains_key(&format!("{PEOPLE}.贺铮.行动")));
    assert_eq!(leaf(&state, "状态栏.行动选项.选项.0"), Some("1. 清点粮草"));
    assert!(data::node_at(&state.state.tree, &path("状态栏.行动选项.选项.2")).is_none());

    // 整份重排：依 JSON 順序重編 `_序`
    gm_update(
        &mut state,
        r#"[{"op":"replace","path":"/状态栏/人物列表","value":{"老贺":{"身份":"老驿长"},"林远":{"身份":"新任驿丞"}}}]"#,
    );
    assert_eq!(leaf(&state, "状态栏.人物列表.老贺._序"), Some("0"));
    assert_eq!(leaf(&state, "状态栏.人物列表.林远._序"), Some("1"));

    // 舊的編號式路徑：項目不存在，拒收並回饋
    let outcome = gm_update(
        &mut state,
        r#"[{"op":"replace","path":"/状态栏/人物列表/0/行动","value":"x"}]"#,
    );
    assert!(data::node_at(&state.state.tree, &path("状态栏.人物列表.0")).is_none());
    assert!(
        outcome.notes.iter().any(|note| note.contains("林远")),
        "{:?}",
        outcome.notes
    );
}

/// 防雙重前綴：GM 回報與鏡像同步寫進樹時都剝掉開頭相同的前綴（圍欄 place 帶 emoji 時鏡像欄不帶）。
#[test]
fn writes_strip_prefixes_for_reports_and_mirror_sync() {
    let root = TestRoot::new("opening-prefix-write");
    let (_, mut state) = apply_north(root.path(), &[("状态栏.地点", "place")]);
    assert_eq!(state.mechanism.display_prefixes["状态栏.地点"], "📍 ");
    gm_update(
        &mut state,
        r#"[{"op":"replace","path":"/状态栏/天气","value":"🌨️ 雪停了"}]"#,
    );
    assert_eq!(leaf(&state, "状态栏.天气"), Some("雪停了"));
    fence(&mut state, &[("place", "📍 北境驿站 仓房")]);
    assert_eq!(leaf(&state, "状态栏.地点"), Some("北境驿站 仓房"));
    crate::mechanism::sync_mirrors_from_edit(
        &mut state,
        &[("place".to_owned(), "📍 马厩".to_owned())]
            .into_iter()
            .collect(),
    );
    assert_eq!(leaf(&state, "状态栏.地点"), Some("马厩"));
}

/// 被拒的鏡像候選照普通格子追蹤（time 不是鏡像來源），GM 回報照樣寫得進去。
#[test]
fn rejected_mirror_candidate_keeps_its_turn_rule() {
    let root = TestRoot::new("opening-mirror-rejected");
    let (_, mut state) = apply_north(root.path(), &[("状态栏.日期和时间", "time")]);
    assert!(state.mechanism.mirrors.is_empty());
    let rule = &state.mechanism.rules["状态栏.日期和时间"];
    assert!(rule.same_as.is_none());
    assert_eq!(rule.inject, InjectLevel::Turn);
    gm_update(
        &mut state,
        r#"[{"op":"replace","path":"/状态栏/日期和时间","value":"大雍永和三年十月初八 卯时"}]"#,
    );
    assert_eq!(
        leaf(&state, "状态栏.日期和时间"),
        Some("大雍永和三年十月初八 卯时")
    );
}

/// 非退場桌（沒淘汰狀態欄）也能標巢狀鏡像（鏡像資格放寬到點分路徑，所有收圍欄的桌都適用）。
#[test]
fn nested_mirrors_work_on_tables_that_kept_their_statusbar() {
    let root = TestRoot::new("opening-mirror-kept");
    let world_id = north_world(root.path());
    let uid = first_uid(root.path(), &world_id);
    let mut product = outcome(
        uid,
        [(
            "状态栏.天气".to_owned(),
            FieldRule::for_kind(FieldKind::Text),
        )]
        .into(),
        Vec::new(),
        &[("状态栏.地点", "place")],
    );
    product.dropped.clear();
    apply(root.path(), &world_id, &product, &selection(true)).unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(!state.statusbar_retired);
    assert_eq!(state.mechanism.mirrors["状态栏.地点"], "place");
    // 非退場桌不做開場格子補齊
    assert!(state.mechanism.collections.is_empty());
    assert!(state.mechanism.display_prefixes.is_empty());
}

/// 平欄型（灰岩要塞 `<details>`）：只有開場格子、沒有規則與觸發的退場桌也開回報；格子都搬進樹、每回合注入，
/// 明確數字給數字欄並記顯示模板；盤點寫繁體「當前環境」也對得上鏡像；MIRRORS 歧義只略過那一行。
#[test]
fn flat_opening_without_rules_opens_reporting() {
    let root = TestRoot::new("opening-flat");
    let world_id = data::create_world(root.path(), "要塞").unwrap();
    let uid = seed_entry(root.path(), &world_id, "状态栏", "模板");
    let mut state = data::read_state(root.path(), &world_id).unwrap();
    for (key, value) in [
        ("驻守天数", "第 1 天"),
        ("当前环境", "灰岩要塞城门内侧"),
        ("驻留/交涉角色", "托伦 (Toren)"),
        ("剧情阶段/要塞事件", "初始阶段。"),
        ("开门", "是"),
        ("開門", "否"),
    ] {
        state.state.table.insert(key.to_owned(), value.to_owned());
    }
    data::write_state(root.path(), &world_id, &state).unwrap();
    let product = outcome(
        uid,
        BTreeMap::new(),
        Vec::new(),
        &[("當前環境", "place"), ("开門", "present")],
    );
    apply(root.path(), &world_id, &product, &selection(false)).unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(state.mechanism.incremental);
    assert!(state
        .state
        .table
        .keys()
        .all(|key| ["time", "place", "present"].contains(&key.as_str())));
    assert_eq!(leaf(&state, "驻守天数"), Some("1"));
    assert_eq!(state.mechanism.display_formats["驻守天数"], "第 {} 天");
    assert_eq!(state.mechanism.rules["驻守天数"].kind, FieldKind::Number);
    for cell in [
        "驻守天数",
        "驻留/交涉角色",
        "剧情阶段/要塞事件",
        "开门",
        "開門",
    ] {
        assert_eq!(
            state.mechanism.rules[cell].inject,
            InjectLevel::Turn,
            "{cell}"
        );
    }
    assert_eq!(
        state.mechanism.mirrors,
        [("当前环境".to_owned(), "place".to_owned())]
            .into_iter()
            .collect(),
        "「开門」同時折到「开门」「開門」，歧義略過，其餘照套"
    );
}

/// 已由重構開過增量的非 MVU 退場桌再套用：缺規則的開場格子補齊；沒有開場值但有規則的非退場桌行為不變。
#[test]
fn reapply_fills_missing_rules_and_plain_rule_tables_are_unchanged() {
    let root = TestRoot::new("opening-reapply");
    let (world_id, mut state) = apply_north(root.path(), &[]);
    state
        .state
        .tree
        .entry("新格".to_owned())
        .or_insert_with(|| StateNode::Leaf("🔥 篝火".to_owned()));
    data::write_state(root.path(), &world_id, &state).unwrap();
    let uid = first_uid(root.path(), &world_id);
    apply(
        root.path(),
        &world_id,
        &outcome(uid, BTreeMap::new(), Vec::new(), &[]),
        &selection(false),
    )
    .unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(state.mechanism.rules["新格"].inject, InjectLevel::Turn);
    assert_eq!(leaf(&state, "新格"), Some("篝火"));

    // 沒有開場值、有規則的 characters 桌（沒淘汰狀態欄）：照原條件開協定，其餘不動
    let plain = data::create_world(root.path(), "空桌").unwrap();
    let uid = seed_entry(root.path(), &plain, "规则", "原文");
    let mut product = outcome(
        uid,
        [("好感度".to_owned(), FieldRule::for_kind(FieldKind::Number))].into(),
        Vec::new(),
        &[],
    );
    product.dropped.clear();
    apply(root.path(), &plain, &product, &selection(true)).unwrap();
    let state = data::read_state(root.path(), &plain).unwrap();
    assert!(state.mechanism.incremental);
    assert_eq!(state.mechanism.rules.len(), 1);
    assert!(state.mechanism.display_prefixes.is_empty());
}

/// 既有引用跟著改寫：規則（萬用）、觸發條件與 `{{state:}}` 從編號路徑換成名字路徑。
#[test]
fn references_into_the_sequence_are_remapped() {
    let root = TestRoot::new("opening-remap");
    let world_id = north_world(root.path());
    let uid = first_uid(root.path(), &world_id);
    let trigger: data::Trigger = serde_json::from_value(json!({
        "id": "hezheng", "title": "贺铮", "mode": "range",
        "cases": [{ "when": [{ "kind": "contains", "path": "状态栏.人物列表.1.人物.心情", "any": ["怒"] }],
                    "text": "贺铮动怒（{{state:状态栏.人物列表.1.人物.行动}}）" }]
    }))
    .unwrap();
    let product = outcome(
        uid,
        [(
            "状态栏.人物列表.*.人物.体力".to_owned(),
            FieldRule::for_kind(FieldKind::Text),
        )]
        .into(),
        vec![trigger],
        &[],
    );
    apply(root.path(), &world_id, &product, &selection(true)).unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(state.mechanism.collections.contains_key(PEOPLE));
    assert!(state.mechanism.rules.contains_key("状态栏.人物列表.*.体力"));
    assert!(!state
        .mechanism
        .rules
        .contains_key("状态栏.人物列表.*.人物.体力"));
    let trigger = &state.mechanism.triggers[0];
    match &trigger.cases[0].when[0] {
        data::Condition::Contains { path, .. } => assert_eq!(path, "状态栏.人物列表.贺铮.心情"),
        other => panic!("{other:?}"),
    }
    assert!(trigger.cases[0]
        .text
        .contains("{{state:状态栏.人物列表.贺铮.行动}}"));
}

/// 保全不了的引用（指到名字欄、條目內文的 `{{state:}}`）：整個序列不轉、不收，原有機制保留。
#[test]
fn unmappable_references_keep_the_sequence_untouched() {
    let root = TestRoot::new("opening-remap-refused");
    let world_id = north_world(root.path());
    let uid = first_uid(root.path(), &world_id);
    let product = outcome(
        uid,
        [(
            "状态栏.人物列表.0.人物.名字".to_owned(),
            FieldRule::for_kind(FieldKind::Text),
        )]
        .into(),
        Vec::new(),
        &[],
    );
    apply(root.path(), &world_id, &product, &selection(true)).unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(!state.mechanism.collections.contains_key(PEOPLE));
    assert_eq!(leaf(&state, "状态栏.人物列表.0.人物.名字"), Some("👤 林远"));
    assert!(state
        .mechanism
        .rules
        .contains_key("状态栏.人物列表.0.人物.名字"));
    assert!(!crate::mechanism::has_rule(
        &state.mechanism,
        &path("状态栏.人物列表.0.人物.行动")
    ));

    let other = north_world(root.path());
    data::upsert_worldbook_entry(
        root.path(),
        &other,
        data::WorldbookEntry {
            uid: u64::MAX,
            title: "引用".to_owned(),
            keys: Vec::new(),
            content: "现在 {{state:状态栏.人物列表.0.人物.行动}}".to_owned(),
            constant: true,
            order: 1,
            disabled: false,
            visibility: data::Visibility::Gm,
            is_person: false,
            locked: false,
        },
    )
    .unwrap();
    let uid = first_uid(root.path(), &other);
    apply(
        root.path(),
        &other,
        &outcome(uid, BTreeMap::new(), Vec::new(), &[]),
        &selection(false),
    )
    .unwrap();
    let state = data::read_state(root.path(), &other).unwrap();
    assert!(!state.mechanism.collections.contains_key(PEOPLE));
}

/// 名字欄有歧義（兩個常見名字鍵）或字形折疊後重名：不轉、不收。
#[test]
fn ambiguous_or_duplicate_names_leave_the_sequence_alone() {
    for (label, people) in [
        (
            "ambiguous",
            "    - 名字: \"甲\"\n      姓名: \"乙\"\n    - 名字: \"丙\"\n      姓名: \"丁\"",
        ),
        (
            "duplicate",
            "    - 名字: \"林远\"\n      心情: \"a\"\n    - 名字: \"林遠\"\n      心情: \"b\"",
        ),
    ] {
        let root = TestRoot::new(&format!("opening-names-{label}"));
        let world_id = data::create_world(root.path(), "桌").unwrap();
        let uid = seed_entry(root.path(), &world_id, "状态栏", "模板");
        post_opening(
            root.path(),
            &world_id,
            &format!("正文\n<Status_block>\n状态栏:\n  人物:\n{people}\n</Status_block>"),
            None,
        );
        apply(
            root.path(),
            &world_id,
            &outcome(uid, BTreeMap::new(), Vec::new(), &[]),
            &selection(false),
        )
        .unwrap();
        let state = data::read_state(root.path(), &world_id).unwrap();
        assert!(state.mechanism.collections.is_empty(), "{label}");
        assert!(
            data::node_at(&state.state.tree, &path("状态栏.人物.0")).is_some(),
            "{label}"
        );
        assert!(
            !crate::mechanism::has_rule(&state.mechanism, &path("状态栏.人物.0.心情")),
            "{label}"
        );
    }
}

/// 掛 MVU 腳本、世界書沒有變數定義的桌（incremental 是假）照 MVU 處理：不做開場格子補齊。
#[test]
fn mvu_script_cards_without_initvar_are_left_alone() {
    const MVU_SCRIPT: &str =
        "import'https://testingcf.jsdelivr.net/gh/MagicalAstrogy/MagVarUpdate/artifact/bundle.js';";
    let root = TestRoot::new("opening-mvu");
    let world_id = data::create_world(root.path(), "腳本卡").unwrap();
    let raw = card(json!({
        "tavern_helper": {"scripts": [{"name":"MVU","enabled":true,"type":"script","content": MVU_SCRIPT}]},
        "regex_scripts": []
    }));
    import::import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff", "zh-TW").unwrap();
    post_opening(root.path(), &world_id, NORTH_OPENING, None);
    let before = data::read_state(root.path(), &world_id).unwrap();
    assert!(!before.mechanism.incremental);
    let uid = first_uid(root.path(), &world_id);
    apply(
        root.path(),
        &world_id,
        &outcome(uid, BTreeMap::new(), Vec::new(), &[]),
        &selection(false),
    )
    .unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(state.mechanism.collections.is_empty());
    assert!(state.mechanism.display_prefixes.is_empty());
    assert!(!state.mechanism.incremental);
}

/// 模型建的文字規則照舊 snapshot；開場格子補的才是每回合注入。
#[test]
fn model_text_rules_keep_snapshot() {
    let root = TestRoot::new("opening-snapshot");
    let world_id = north_world(root.path());
    let uid = first_uid(root.path(), &world_id);
    let product = outcome(
        uid,
        [(
            "状态栏.传闻".to_owned(),
            FieldRule::for_kind(FieldKind::Text),
        )]
        .into(),
        Vec::new(),
        &[],
    );
    apply(root.path(), &world_id, &product, &selection(true)).unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(
        state.mechanism.rules["状态栏.传闻"].inject,
        InjectLevel::Snapshot
    );
    assert_eq!(
        state.mechanism.rules["状态栏.天气"].inject,
        InjectLevel::Turn
    );
}

/// 撤銷：編號分支、規則、集合表、顯示前綴表、鏡像都回到套用前。
#[test]
fn undo_restores_the_sequence_and_all_tables() {
    let root = TestRoot::new("opening-undo");
    let world_id = north_world(root.path());
    let before = data::read_state(root.path(), &world_id).unwrap();
    let uid = first_uid(root.path(), &world_id);
    apply_recorded(
        root.path(),
        &world_id,
        &outcome(
            uid,
            BTreeMap::new(),
            Vec::new(),
            &[("状态栏.地点", "place")],
        ),
        &selection(false),
    );
    receipts::undo_last_import(root.path(), &world_id, &data::test_exclusive(&world_id)).unwrap();
    let after = data::read_state(root.path(), &world_id).unwrap();
    assert_eq!(after.state.tree, before.state.tree);
    assert_eq!(after.state.table, before.state.table);
    assert!(after.mechanism.rules.is_empty());
    assert!(after.mechanism.collections.is_empty());
    assert!(after.mechanism.display_prefixes.is_empty());
    assert!(after.mechanism.opening.is_empty());
    assert!(after.mechanism.mirrors.is_empty());
    assert!(!after.mechanism.incremental);
}

/// 既有缺口（非本案引入）：撤銷重構只還原狀態檔，整幕事件快照仍是套用時同步進去的轉換後樹；
/// 撤銷後再收回上一句，樹被打回轉換後的樣子，集合表與規則卻已退掉，兩邊不一致。此測試記錄現況。
#[test]
fn undo_then_retract_rewinds_to_the_converted_tree_known_gap() {
    let root = TestRoot::new("opening-undo-retract");
    let world_id = north_world(root.path());
    let uid = first_uid(root.path(), &world_id);
    apply_recorded(
        root.path(),
        &world_id,
        &outcome(
            uid,
            BTreeMap::new(),
            Vec::new(),
            &[("状态栏.地点", "place")],
        ),
        &selection(false),
    );
    super::takeover_flat_keys::narration(root.path(), &world_id, "第二則");
    receipts::undo_last_import(root.path(), &world_id, &data::test_exclusive(&world_id)).unwrap();
    let undone = data::read_state(root.path(), &world_id).unwrap();
    assert!(data::node_at(&undone.state.tree, &path("状态栏.人物列表.0")).is_some());

    assert!(data::pop_transcript(root.path(), &world_id, 0).unwrap());
    let state = data::read_state(root.path(), &world_id).unwrap();
    assert!(
        data::node_at(&state.state.tree, &path("状态栏.人物列表.林远")).is_some(),
        "收回後樹回到轉換後的鍵名（缺口現況）"
    );
    assert!(state.mechanism.collections.is_empty(), "集合表已隨撤銷退掉");
}

/// 欄位取所有項目的聯集：第二人多一欄「伤势」也轉進集合，第一人不補空值；前綴只看有這欄的項目。
#[test]
fn sequence_fields_are_the_union_of_all_items() {
    let root = TestRoot::new("opening-union");
    let world_id = data::create_world(root.path(), "桌").unwrap();
    let uid = seed_entry(root.path(), &world_id, "状态栏", "模板");
    post_opening(
        root.path(),
        &world_id,
        "正文\n<Status_block>\n状态栏:\n  人物:\n    - 名字: \"甲\"\n      心情: \"🙂 平静\"\n    - 名字: \"乙\"\n      心情: \"🙂 紧张\"\n      伤势: \"🩸 左臂擦伤\"\n</Status_block>",
        None,
    );
    apply(
        root.path(),
        &world_id,
        &outcome(uid, BTreeMap::new(), Vec::new(), &[]),
        &selection(false),
    )
    .unwrap();
    let state = data::read_state(root.path(), &world_id).unwrap();
    let spec = &state.mechanism.collections["状态栏.人物"];
    assert!(spec.fields.contains_key("伤势") && spec.fields.contains_key("心情"));
    assert_eq!(leaf(&state, "状态栏.人物.乙.伤势"), Some("左臂擦伤"));
    assert_eq!(leaf(&state, "状态栏.人物.甲.伤势"), None, "沒有的欄位不補");
    assert!(state.mechanism.rules.contains_key("状态栏.人物.*.伤势"));
    assert_eq!(
        state.mechanism.display_prefixes["状态栏.人物.*.伤势"],
        "🩸 "
    );
    assert!(!spec.present_scoped, "模板沒寫只顯示在場人物");
}

pub(super) fn north_with_template(root: &Path, template: &str) -> data::WorldState {
    let world_id = north_world(root);
    let uid = first_uid(root, &world_id);
    let mut product = outcome(uid, BTreeMap::new(), Vec::new(), &[]);
    product.dropped[0].content = template.to_owned();
    apply(root, &world_id, &product, &selection(false)).unwrap();
    data::read_state(root, &world_id).unwrap()
}

/// 原卡模板寫明只顯示在場人物（簡繁都認）才把人物集合標成在場名單；沒寫的不標。
#[test]
fn present_only_templates_mark_the_person_collection() {
    for (label, template, expected) in [
        ("rule", "- 只显示在场人物的数据，不在场的不显示。", true),
        ("inline", "     {在对话现场的人物才显示}", true),
        ("traditional", "只顯示在場人物", true),
        (
            "plain",
            "- 状态栏数据用于显示角色当前状态与所处环境。",
            false,
        ),
    ] {
        let root = TestRoot::new(&format!("opening-present-{label}"));
        let state = north_with_template(root.path(), template);
        assert_eq!(
            state.mechanism.collections[PEOPLE].present_scoped, expected,
            "{label}"
        );
        assert!(
            !state.mechanism.collections["状态栏.行动选项.选项"].present_scoped,
            "{label}"
        );
    }
}
