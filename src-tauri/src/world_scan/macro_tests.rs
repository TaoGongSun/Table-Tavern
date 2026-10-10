//! 巨集接線的單元測試（方案三之 3、三之 7、三之 8、四之 2–3）：穩定定義的巨集條件、動態公開設定移出共用快照、
//! 代換結果的機密分流、中性模式不落地、注入段落只代換一次、上一輪 outlet、變數落地與衝突重放、重設。
use super::tests::{card, event, raw, run, TempRoot, LANG};
use super::*;
use crate::data::card_vars::{self, Layer, LayerWrite};
use crate::data::state_commit::with_commit;
use crate::data::world_info_store::{self as store, Perspective, Report, Stage};
use crate::data::{self, message_vars, TranscriptKind, Visibility};
use crate::st_macros::variables::{ScopeKind, VarOpKind, VarScope, Variables};
use crate::transport::{self, Hoist};
use serde_json::json;

fn inputs(local: &str, outlets: &[(&str, SourceText)]) -> MacroInputs {
    MacroInputs {
        variables: Variables::new(
            VarScope::from_json_text(local).unwrap(),
            VarScope::default(),
        ),
        prev_outlets: outlets
            .iter()
            .map(|(name, text)| ((*name).to_owned(), text.clone()))
            .collect(),
        ..MacroInputs::empty()
    }
}

fn prepared_with(
    book: &TableBook,
    viewer: Viewer<'_>,
    cards: &[CharacterCard],
    events: &[TranscriptEvent],
    inputs: MacroInputs,
) -> WorldScan {
    let session = MacroSession::new(
        inputs,
        &viewer,
        "",
        book.world_card_name(),
        None,
        events,
        LANG,
    );
    let mut scanned = scan(ScanRequest {
        book,
        viewer,
        sole_card: None,
        player: None,
        events,
        lang: LANG,
        timed: Default::default(),
        budget: None,
        random: Randomness::Measure,
        session: &session,
    });
    prepare(&mut scanned, &session, book, &viewer, cards, None);
    scanned
}

pub(super) fn layer(root: &TempRoot, world: &str, layer: Layer) -> card_vars::LayerDoc {
    card_vars::read_layer(root.path(), world, layer, None).unwrap()
}

pub(super) fn write(
    root: &TempRoot,
    world: &str,
    which: Layer,
    rev: Option<&str>,
    vars: &str,
) -> String {
    let generation = with_commit(root.path(), world, message_vars::generation);
    match card_vars::write_layer(root.path(), world, which, None, generation, rev, vars).unwrap() {
        LayerWrite::LayerOk { rev } => rev,
        other => panic!("{other:?}"),
    }
}

pub(super) fn set_op(
    scope: ScopeKind,
    name: &str,
    value: &str,
) -> crate::st_macros::variables::VarOp {
    crate::st_macros::variables::VarOp {
        scope,
        name: name.to_owned(),
        kind: VarOpKind::Set {
            value: crate::st_macros::js_value::JsValue::str(value),
            index: None,
        },
    }
}

/// 穩定定義：內文與標題只含靜態巨集（`{{user}}`、`{{newline}}`、`{{trim}}`、`{{noop}}`、註解）才進共用快照；
/// `{{char}}`、`{{random}}`、變數類、帶參數的都不算。
#[test]
fn stable_requires_static_macros() {
    let constant = |uid, title: &str, content: &str| {
        raw(
            uid,
            title,
            Visibility::Public,
            json!({"constant": true, "content": content}),
        )
    };
    let book = TableBook::from_raw(vec![
        constant(
            1,
            "靜態",
            "{{user}}來過{{newline}}{{// 註解 {{random::a}} }}{{noop}}",
        ),
        constant(2, "說話者", "{{char}}在場"),
        constant(3, "擲骰", "{{random::甲::乙}}"),
        constant(4, "{{random::標::題}}", "內文"),
        constant(5, "變數", "{{getvar::x}}"),
        constant(6, "換行數", "{{newline::2}}"),
        constant(7, "跳脫", "\\{\\{random\\}\\}"),
    ]);
    assert_eq!(
        book.snapshot()
            .iter()
            .map(|entry| entry.uid)
            .collect::<Vec<_>>(),
        [1]
    );
    let fox = card("fox", "狐狸", "", "");
    let scanned = run(
        &book,
        Viewer::Character(&fox),
        std::slice::from_ref(&fox),
        &[],
    );
    let unstable: Vec<u64> = scanned
        .placed
        .iter()
        .filter(|entry| !entry.stable)
        .map(|entry| entry.uid)
        .collect();
    assert_eq!(unstable.len(), 6, "{unstable:?}");
}

/// 公開設定含動態巨集的卡移出共用快照（只留名字），改在每位角色的回合尾（機密段）以該角色視角代換；
/// 零溢出時切換角色，共用 system 逐字不變。不抹尾段的線（`Hoist::All`）進 system 並算進重開指紋。
#[test]
fn dynamic_public_profiles_leave_the_shared_snapshot() {
    let book = TableBook::from_raw(vec![]);
    let knight = card("knight", "騎士", "我是 {{char}}，{{user}} 的朋友。", "");
    let fox = card("fox", "狐狸", "{{char}} 今天 {{random::笑::哭}}。", "");
    let cards = [knight.clone(), fox.clone()];
    let mut systems = Vec::new();
    for speaker in [&knight, &fox, &knight] {
        let scanned = run(&book, Viewer::Character(speaker), &cards, &[]);
        let system = transport::chars_lane_system(&cards, None, &scanned, LANG);
        assert!(system.contains("### 騎士\n我是 騎士，"));
        assert!(system.contains("### 狐狸\n"));
        assert!(!system.contains("狐狸 今天"));
        systems.push(system);
        let turn = transport::chars_lane_turn(
            speaker,
            &cards,
            None,
            &scanned,
            &Default::default(),
            &Default::default(),
            None,
            LANG,
            Hoist::None,
        );
        let confidential = turn.confidential.unwrap();
        assert!(
            confidential.contains("## 登場角色（公開設定，本輪）\n### 狐狸\n狐狸 今天 笑。"),
            "{confidential}"
        );
        let all = transport::chars_lane_turn(
            speaker,
            &cards,
            None,
            &scanned,
            &Default::default(),
            &Default::default(),
            None,
            LANG,
            Hoist::All,
        );
        assert!(all.hoisted_private.unwrap().contains("狐狸 今天 笑。"));
        assert!(all.hoisted_worldbook.unwrap().contains("狐狸 今天 笑。"));
        assert!(!all.tail.contains("狐狸 今天"));
    }
    assert!(systems.windows(2).all(|pair| pair[0] == pair[1]));
}

/// 代換結果的機密分流：`{{description}}` 讀到自己的私設 → 該條目改放機密段；別人的卡欄位巨集只讀得到公開設定。
#[test]
fn macro_reading_private_text_goes_to_the_confidential_block() {
    let book = TableBook::from_raw(vec![raw(
        1,
        "自述",
        Visibility::Public,
        json!({"constant": true, "content": "記得：{{description}}"}),
    )]);
    let fox = card("fox", "狐狸", "狡猾。", "其實是龍。");
    let knight = card("knight", "騎士", "{{description}}！", "其實是王子。");
    let cards = [fox.clone(), knight.clone()];
    let scanned = run(&book, Viewer::Character(&fox), &cards, &[]);
    let entry = &scanned.placed[0];
    assert!(entry.confidential() && entry.content.contains("其實是龍。"));
    let turn = transport::chars_lane_turn(
        &fox,
        &cards,
        None,
        &scanned,
        &Default::default(),
        &Default::default(),
        None,
        LANG,
        Hoist::None,
    );
    assert!(!turn.tail.contains("## 你知道的世界情報\n### 自述"));
    let confidential = turn.confidential.unwrap();
    assert!(confidential.contains("### 自述\n記得：狡猾。\n其實是龍。"));
    // 騎士的公開設定在狐狸視角是中性代換、只讀得到騎士的公開設定
    // 騎士的 `{{description}}` 只讀到騎士自己的公開設定（第一輪不含卡欄位巨集，所以是「！」）
    assert!(confidential.contains("### 騎士\n！！\n"), "{confidential}");
    assert!(!format!("{}{confidential}", turn.tail).contains("其實是王子"));
}

/// 上一輪 outlet：掃描時的代換讀上一輪的值（私密 outlet 讓條目變私密）；掃完換成本輪值；限定條目組成的
/// outlet 是私密的。
#[test]
fn previous_outlets_feed_the_scan_and_this_round_replaces_them() {
    let book = TableBook::from_raw(vec![
        raw(
            1,
            "引用",
            Visibility::Public,
            json!({"constant": true, "content": "前情：{{outlet::秘}}"}),
        ),
        raw(
            2,
            "秘密出口",
            Visibility::Characters(vec!["fox".to_owned()]),
            json!({"constant": true, "position": 7, "outletName": "秘", "content": "密道"}),
        ),
    ]);
    let fox = card("fox", "狐狸", "", "");
    let scanned = prepared_with(
        &book,
        Viewer::Character(&fox),
        std::slice::from_ref(&fox),
        &[],
        inputs("{}", &[("秘", SourceText::private("舊密道"))]),
    );
    let quoting = scanned.placed.iter().find(|entry| entry.uid == 1).unwrap();
    assert_eq!(quoting.content, "前情：舊密道");
    assert!(quoting.confidential(), "讀到私密的上一輪 outlet");
    assert_eq!(scanned.outlets["秘"], SourceText::private("密道"));
    // 沒有上一輪 outlet：讀到空字串，條目仍公開
    let fresh = run(
        &book,
        Viewer::Character(&fox),
        std::slice::from_ref(&fox),
        &[],
    );
    let quoting = fresh.placed.iter().find(|entry| entry.uid == 1).unwrap();
    assert_eq!(quoting.content, "前情：");
    assert!(!quoting.confidential());
}

/// 格式條目判定：outlet 條目只有在 `{{outlet}}` 真的被代入時才算進「本輪實際送入全文」。
#[test]
fn outlet_entries_count_as_sent_only_when_quoted() {
    let book = TableBook::from_raw(vec![raw(
        1,
        "格式",
        Visibility::Gm,
        json!({"constant": true, "position": 7, "outletName": "格式", "content": "<格式>"}),
    )]);
    let quoting = card("fox", "狐狸", "照這個寫：{{outlet::格式}}", "");
    let silent = card("fox", "狐狸", "沒有引用", "");
    let titles = |cards: &[CharacterCard]| {
        let scanned = run(&book, Viewer::Gm, cards, &[]);
        transport::gm_prompt_full_entries(&scanned, &[])
            .iter()
            .map(|entry| entry.title.to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(titles(std::slice::from_ref(&quoting)), ["格式"]);
    assert!(titles(std::slice::from_ref(&silent)).is_empty());
    let scanned = run(&book, Viewer::Gm, std::slice::from_ref(&quoting), &[]);
    assert_eq!(scanned.texts.cards["fox"].public, "照這個寫：<格式>");
}

/// 中性模式：別人的卡的 `{{setvar}}` 輸出照算、不進操作序列；自己的卡與本視角條目照完整模式記下。
/// 注入段落（同深度同角色）每條內文的副作用只執行一次。
#[test]
fn neutral_texts_do_not_land_and_injections_run_once() {
    let book = TableBook::from_raw(vec![
        raw(
            1,
            "深一",
            Visibility::Public,
            json!({"constant": true, "position": 4, "depth": 2, "content": "{{incvar::n}}一"}),
        ),
        raw(
            2,
            "深二",
            Visibility::Public,
            json!({"constant": true, "position": 4, "depth": 2, "content": "{{incvar::n}}二"}),
        ),
    ]);
    let fox = card("fox", "狐狸", "{{setvar::mine::1}}{{getvar::mine}}", "");
    let knight = card(
        "knight",
        "騎士",
        "{{setvar::theirs::1}}{{getvar::theirs}}",
        "",
    );
    let cards = [fox.clone(), knight.clone()];
    let scanned = run(&book, Viewer::Character(&fox), &cards, &[]);
    assert_eq!(scanned.texts.cards["knight"].public, "1");
    assert_eq!(scanned.texts.cards["fox"].public, "1");
    let names: Vec<&str> = scanned.var_ops.iter().map(|op| op.name.as_str()).collect();
    assert_eq!(names, ["mine", "n", "n"]);
    let turn = transport::chars_lane_turn(
        &fox,
        &cards,
        None,
        &scanned,
        &Default::default(),
        &Default::default(),
        None,
        LANG,
        Hoist::None,
    );
    assert!(turn.tail.contains("2二\n1一"), "{}", turn.tail);
}

/// 落地：在最新的表上重放操作序列（卡片介面在求值與落地之間寫的值也在）；成功記下本輪 outlet。
#[test]
fn landing_replays_ops_on_the_latest_table() {
    let root = TempRoot::new("macro-land");
    let world = data::create_world(root.path(), "落地").unwrap();
    let rev = write(&root, &world, Layer::Chat, None, r#"{"a":1}"#);
    // 介面在掃描之後、落地之前又寫了一次
    write(&root, &world, Layer::Chat, Some(&rev), r#"{"a":2}"#);
    let scanned = WorldScan {
        var_ops: vec![
            set_op(ScopeKind::Local, "x", "甲"),
            set_op(ScopeKind::Global, "g", "乙"),
        ],
        outlets: [("門".to_owned(), SourceText::public("後門"))].into(),
        ..WorldScan::default()
    };
    let gm = Perspective::Gm;
    landing::land(root.path(), &world, 0, "t1", &gm, &scanned).unwrap();
    assert_eq!(
        layer(&root, &world, Layer::Chat).vars,
        r#"{"a":2,"x":"甲"}"#
    );
    assert_eq!(layer(&root, &world, Layer::Global).vars, r#"{"g":"乙"}"#);
    let pending = store::read_scene(root.path(), &world, 0)
        .unwrap()
        .pending
        .unwrap();
    assert_eq!(pending.stage, Stage::Sent);
    assert_eq!(pending.vars.len(), 2);
    assert!(pending.vars.iter().all(|intent| intent.after_rev.is_some()));
    let next = MacroInputs::load(
        root.path(),
        &world,
        &gm,
        String::new(),
        Default::default(),
        false,
    );
    assert_eq!(next.prev_outlets["門"].text, "後門");
    assert_eq!(next.variables.local.raw("x").unwrap().normalize(), "甲");
    macros::forget_outlets(&world);
    let cleared = MacroInputs::load(
        root.path(),
        &world,
        &gm,
        String::new(),
        Default::default(),
        false,
    );
    assert!(cleared.prev_outlets.is_empty());
    // 已送出之後才失敗：計時回滾、變數副作用保留
    landing::fail(root.path(), &world, 0, "t1", Report::Inline);
    assert_eq!(
        layer(&root, &world, Layer::Chat).vars,
        r#"{"a":2,"x":"甲"}"#
    );
}

/// 部分失敗：chat 寫成之後 global 層壞掉 → 不送出、回錯、chat 已還原、pending 清掉、outlet 不更新。
#[test]
fn a_failed_layer_undoes_the_ones_already_written() {
    let root = TempRoot::new("macro-land-fail");
    let world = data::create_world(root.path(), "失敗").unwrap();
    write(&root, &world, Layer::Chat, None, r#"{"a":1}"#);
    let global = root.path().join("card-vars").join("global.json");
    std::fs::create_dir_all(global.parent().unwrap()).unwrap();
    std::fs::write(&global, "{壞掉").unwrap();
    let scanned = WorldScan {
        var_ops: vec![
            set_op(ScopeKind::Local, "x", "甲"),
            set_op(ScopeKind::Global, "g", "乙"),
        ],
        outlets: [("門".to_owned(), SourceText::public("後門"))].into(),
        ..WorldScan::default()
    };
    let gm = Perspective::Gm;
    assert!(landing::land(root.path(), &world, 0, "t1", &gm, &scanned).is_err());
    assert_eq!(layer(&root, &world, Layer::Chat).vars, r#"{"a":1}"#);
    assert!(store::read_scene(root.path(), &world, 0)
        .unwrap()
        .pending
        .is_none());
    assert!(MacroInputs::load(
        root.path(),
        &world,
        &gm,
        String::new(),
        Default::default(),
        false
    )
    .prev_outlets
    .is_empty());
}

/// 撤回時層已被別人寫過：保留別人的值，連同原錯回報（`world_info_vars_kept`）。
#[test]
fn undo_keeps_writes_made_by_others_and_reports_them() {
    let root = TempRoot::new("macro-land-kept");
    let world = data::create_world(root.path(), "保留").unwrap();
    let gm = Perspective::Gm;
    store::begin_landing(root.path(), &world, 0, "t1", &gm, &Default::default()).unwrap();
    let rev = write(&root, &world, Layer::Chat, None, r#"{"x":"甲"}"#);
    store::push_var_intent(
        root.path(),
        &world,
        0,
        "t1",
        store::VarIntent {
            layer: Layer::Chat,
            id: None,
            expected_rev: None,
            before: "{}".to_owned(),
            ops: json!([]),
            after_rev: Some(rev.clone()),
            restore_rev: None,
        },
    )
    .unwrap();
    write(&root, &world, Layer::Chat, Some(&rev), r#"{"x":"介面"}"#);
    let kept = landing::fail(root.path(), &world, 0, "t1", Report::Inline);
    assert_eq!(kept.len(), 1);
    let message = landing::with_kept("原錯".to_owned(), &kept);
    assert!(message.contains("world_info_vars_kept") && message.contains("原錯"));
    assert_eq!(layer(&root, &world, Layer::Chat).vars, r#"{"x":"介面"}"#);
    assert!(store::read_notices(root.path(), &world).unwrap().is_empty());
}

/// 重設前先撤回寫入中的變數意圖（撤得回就還原，撤不回寫進待回報檔），再把幕檔移去備份。
#[test]
fn reset_releases_writing_intents_before_moving_the_file() {
    let root = TempRoot::new("macro-reset");
    let world = data::create_world(root.path(), "重設").unwrap();
    let gm = Perspective::Gm;
    store::begin_landing(root.path(), &world, 0, "t1", &gm, &Default::default()).unwrap();
    let chat_rev = write(&root, &world, Layer::Chat, None, r#"{"x":1}"#);
    let global_rev = write(&root, &world, Layer::Global, None, r#"{"g":1}"#);
    for (layer, rev) in [(Layer::Chat, &chat_rev), (Layer::Global, &global_rev)] {
        store::push_var_intent(
            root.path(),
            &world,
            0,
            "t1",
            store::VarIntent {
                layer,
                id: None,
                expected_rev: None,
                before: "{}".to_owned(),
                ops: json!([]),
                after_rev: Some(rev.clone()),
                restore_rev: None,
            },
        )
        .unwrap();
    }
    // global 在之後被別人寫過
    write(
        &root,
        &world,
        Layer::Global,
        Some(&global_rev),
        r#"{"g":2}"#,
    );
    let moved = store::reset_scene(root.path(), &world, 0).unwrap();
    assert_eq!(moved.len(), 1);
    assert_eq!(
        layer(&root, &world, Layer::Chat).rev,
        None,
        "chat 撤回成沒有檔"
    );
    assert_eq!(layer(&root, &world, Layer::Global).vars, r#"{"g":2}"#);
    let notices = store::read_notices(root.path(), &world).unwrap();
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].conflict.layer, Layer::Global);
}

/// 重設時待回報檔寫不成：回錯、不移檔，pending 留著（日誌不得整筆丟掉）；之後寫得成就照常重設。
#[test]
fn reset_keeps_the_log_when_notices_cannot_be_written() {
    let root = TempRoot::new("macro-reset-notices");
    let world = data::create_world(root.path(), "重設失敗").unwrap();
    store::begin_landing(
        root.path(),
        &world,
        0,
        "t1",
        &Perspective::Gm,
        &Default::default(),
    )
    .unwrap();
    let rev = write(&root, &world, Layer::Chat, None, r#"{"x":1}"#);
    store::push_var_intent(
        root.path(),
        &world,
        0,
        "t1",
        store::VarIntent {
            layer: Layer::Chat,
            id: None,
            expected_rev: None,
            before: "{}".to_owned(),
            ops: json!([]),
            after_rev: Some(rev.clone()),
            restore_rev: None,
        },
    )
    .unwrap();
    // 之後被別人寫過：撤不回、要寫待回報檔
    write(&root, &world, Layer::Chat, Some(&rev), r#"{"x":2}"#);
    {
        let _guard = crate::data::RenameFailGuard::fail_ending("notices.json", 1);
        assert!(store::reset_scene(root.path(), &world, 0).is_err());
    }
    let scene = store::read_scene(root.path(), &world, 0).unwrap();
    assert_eq!(
        scene.pending.map(|pending| pending.turn_key).as_deref(),
        Some("t1")
    );
    assert!(store::read_notices(root.path(), &world).unwrap().is_empty());
    let moved = store::reset_scene(root.path(), &world, 0).unwrap();
    assert_eq!(moved.len(), 1);
    assert_eq!(store::read_notices(root.path(), &world).unwrap().len(), 1);
    assert_eq!(layer(&root, &world, Layer::Chat).vars, r#"{"x":2}"#);
}

/// 信任邊界：狀態值（模型輸出）裡的巨集不接引擎，`{{setvar}}` 原樣留著、不進操作序列。
#[test]
fn state_values_stay_outside_the_engine() {
    let mut state = data::TableState::default();
    state.table.insert(
        "place".to_owned(),
        "{{setvar::x::1}}{{user}}的房間".to_owned(),
    );
    let messages = transport::test_support::legacy::assemble_gm_messages(
        "",
        &[],
        Some(&card("p", "阿濤", "", "")),
        &[event(TranscriptKind::Player, "阿濤", "嗨")],
        &[],
        &state,
        &Default::default(),
        &Default::default(),
        LANG,
    );
    let tail = &messages.last().unwrap().content;
    assert!(tail.contains("{{setvar::x::1}}{{user}}的房間"), "{tail}");
}

/// `{{persona}}`：玩家卡的公開設定。
#[test]
fn persona_reads_the_player_card() {
    let book = TableBook::from_raw(vec![raw(
        1,
        "人設",
        Visibility::Public,
        json!({"constant": true, "content": "玩家是：{{persona}}"}),
    )]);
    let player = card("p", "阿濤", "愛冒險的旅人", "");
    let fox = card("fox", "狐狸", "", "");
    let session = MacroSession::new(
        MacroInputs::empty(),
        &Viewer::Character(&fox),
        "",
        None,
        Some(&player),
        &[],
        LANG,
    );
    assert_eq!(session.own("{{persona}}").text, "愛冒險的旅人");
    let scanned = transport::test_support::legacy::prepared_book(
        &book,
        Viewer::Character(&fox),
        "",
        std::slice::from_ref(&fox),
        Some(&player),
        &[],
        LANG,
    );
    assert_eq!(scanned.placed[0].content, "玩家是：愛冒險的旅人");
}

/// 公開事件（回歸、公開人物登場、開場白）用公開環境：卡欄位巨集只讀公開設定、讀不到 GM outlet；
/// 只給 GM 看的事件才讀得到私設與 outlet。
#[test]
fn public_events_never_read_private_text_or_gm_outlets() {
    let fox = card(
        "fox",
        "狐狸",
        "{{description}}|{{outlet::秘}}",
        "其實是龍。",
    );
    let session = MacroSession::new(
        inputs("{}", &[("秘", SourceText::public("GM 的密道"))]),
        &Viewer::Gm,
        "",
        None,
        None,
        &[],
        LANG,
    );
    let gm = session.fill(&fox.public_md, Some(&fox), true);
    assert!(
        gm.contains("其實是龍。") && gm.ends_with("|GM 的密道"),
        "{gm}"
    );
    let public = session.publicized(&[], LANG);
    let text = public.fill(&fox.public_md, Some(&fox), false);
    assert!(
        !text.contains("其實是龍") && !text.contains("密道"),
        "{text}"
    );
}

/// 掃描、代換一次（帶玩家卡）。
fn prepared_with_player(
    book: &TableBook,
    me: &CharacterCard,
    player: &CharacterCard,
    inputs: MacroInputs,
) -> WorldScan {
    let viewer = Viewer::Character(me);
    let session = MacroSession::new(inputs, &viewer, "", None, Some(player), &[], LANG);
    let mut scanned = scan(ScanRequest {
        book,
        viewer,
        sole_card: None,
        player: Some(player),
        events: &[],
        lang: LANG,
        timed: Default::default(),
        budget: None,
        random: Randomness::Measure,
        session: &session,
    });
    prepare(
        &mut scanned,
        &session,
        book,
        &viewer,
        std::slice::from_ref(me),
        Some(player),
    );
    scanned
}

/// 全域掃描的私密來源：公開設定讀到私密的上一輪 outlet、玩家人設也是——只靠它們觸發的 Public 條目照私密觸發
/// 分流（機密段）；同樣的 outlet 是公開的就照常公開。
#[test]
fn global_fields_reading_private_sources_trigger_privately() {
    let book = TableBook::from_raw(vec![
        raw(
            1,
            "描述觸發",
            Visibility::Public,
            json!({"key": ["密道"], "matchCharacterDescription": true, "content": "地圖"}),
        ),
        raw(
            2,
            "人設觸發",
            Visibility::Public,
            json!({"key": ["暗門"], "matchPersonaDescription": true, "content": "鑰匙"}),
        ),
    ]);
    let fox = card("fox", "狐狸", "前情：{{outlet::秘}}", "");
    let player = card("p", "阿濤", "知道{{outlet::暗}}", "");
    let run_with = |private: bool| {
        let source = |text: &str| match private {
            true => SourceText::private(text),
            false => SourceText::public(text),
        };
        prepared_with_player(
            &book,
            &fox,
            &player,
            inputs("{}", &[("秘", source("密道")), ("暗", source("暗門"))]),
        )
    };
    let confidential = |scanned: &WorldScan| {
        let mut uids: Vec<(u64, bool)> = scanned
            .placed
            .iter()
            .map(|entry| (entry.uid, entry.confidential()))
            .collect();
        uids.sort();
        uids
    };
    assert_eq!(confidential(&run_with(true)), [(1, true), (2, true)]);
    assert_eq!(confidential(&run_with(false)), [(1, false), (2, false)]);
}

/// 卡欄位巨集回傳第一輪已代換的文字：公開設定的 `{{incvar}}` 不論被一條還是兩條條目以卡欄位巨集引用，
/// 都只加 1。
#[test]
fn card_field_macros_do_not_rerun_side_effects() {
    let quoting = |uid, content: &str| {
        raw(
            uid,
            &format!("引用{uid}"),
            Visibility::Public,
            json!({"constant": true, "content": content}),
        )
    };
    let fox = card("fox", "狐狸", "第{{incvar::n}}次", "");
    for book in [
        TableBook::from_raw(vec![quoting(1, "記得：{{description}}")]),
        TableBook::from_raw(vec![
            quoting(1, "記得：{{description}}"),
            quoting(2, "{{personality}}／{{charDepthPrompt}}"),
        ]),
    ] {
        let scanned = run(
            &book,
            Viewer::Character(&fox),
            std::slice::from_ref(&fox),
            &[],
        );
        let n: Vec<_> = scanned.var_ops.iter().filter(|op| op.name == "n").collect();
        assert_eq!(n.len(), 1, "{:?}", scanned.var_ops);
        assert!(scanned
            .placed
            .iter()
            .all(|entry| entry.content.contains("第1次")));
        assert_eq!(scanned.texts.cards["fox"].public, "第1次");
    }
}

/// 沒被 `{{outlet}}` 引用的 outlet 條目不跑第二輪：第一輪產物裡的 `{{incvar}}` 不執行。
#[test]
fn unquoted_outlets_skip_the_second_round() {
    let book = TableBook::from_raw(vec![raw(
        1,
        "出口",
        Visibility::Public,
        json!({"constant": true, "position": 7, "outletName": "x", "content": "{{getvar::tpl}}"}),
    )]);
    let fox = card("fox", "狐狸", "沒有引用", "");
    let scanned = prepared_with(
        &book,
        Viewer::Character(&fox),
        std::slice::from_ref(&fox),
        &[],
        inputs(r#"{"tpl":"{{incvar::n}}"}"#, &[]),
    );
    assert_eq!(scanned.outlets["x"].text, "{{incvar::n}}");
    assert!(scanned.var_ops.is_empty(), "{:?}", scanned.var_ops);
}
