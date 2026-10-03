//! 中文骨架逐字基準：zh-TW／zh-CN／zh-HK 送 AI 的 system、動態塊、事件行、CLI 包裝、補丁與收尾句
//! 必須與 `scaffold_baseline/<lang>.txt` 逐字相同。基準在骨架語系改版前產出；
//! 要重產（只在刻意改中文骨架時）：`SCAFFOLD_BASELINE_WRITE=1 cargo test scaffold_baseline`。

use std::collections::BTreeMap;

use super::snapshot_patch::render_patch;
use super::{
    build_prompt, events_fingerprint, plan_turn, Lane, LaneProvider, LaneState, ReopenReason,
    ReplyEcho, TurnInput, TurnPlan, CACHE_TTL_SECS,
};
use crate::cli::flatten_messages;
use crate::data::{
    CharacterCard, EventMarker, FieldKind, FieldRule, Mechanism, StateNode, TableState, Tier,
    TranscriptEvent, TranscriptKind, Trigger, TriggerMode, Visibility, WorldbookEntry,
};
use crate::transport::{
    self, assemble_gm_messages, assemble_shared_messages, chars_lane_system, chars_lane_turn,
    gm_lane_system, gm_lane_turn, lane_event_line, ChatMessage, GmTurnFormat, Side, StateScope,
};

pub(super) fn card(id: &str, name: &str, public_md: &str, private_md: &str) -> CharacterCard {
    CharacterCard {
        id: id.to_owned(),
        name: name.to_owned(),
        color: "#336699".to_owned(),
        avatar: "🦊".to_owned(),
        tier: Tier::Balanced,
        show_image: true,
        archived: false,
        gen_prompt: String::new(),
        public_md: public_md.to_owned(),
        private_md: private_md.to_owned(),
    }
}

fn event(kind: TranscriptKind, id: &str, name: &str, text: &str) -> TranscriptEvent {
    TranscriptEvent {
        raw: None,
        ts: "2026-10-03T12:00:00+08:00".to_owned(),
        speaker_id: id.to_owned(),
        speaker_name: name.to_owned(),
        kind,
        text: text.to_owned(),
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
    }
}

fn marked(mut event: TranscriptEvent, marker: EventMarker, gm_only: bool) -> TranscriptEvent {
    event.marker = Some(marker);
    event.gm_only = gm_only;
    event
}

fn entry(
    uid: u64,
    title: &str,
    keys: &[&str],
    constant: bool,
    visibility: Visibility,
) -> WorldbookEntry {
    WorldbookEntry {
        uid,
        title: title.to_owned(),
        keys: keys.iter().map(|key| (*key).to_owned()).collect(),
        content: format!("{title}內容，{{{{user}}}}也知道"),
        constant,
        order: uid as i64,
        disabled: false,
        visibility,
        is_person: false,
        locked: false,
    }
}

pub(super) struct Fixture {
    pub cards: Vec<CharacterCard>,
    pub player: CharacterCard,
    pub worldbook: Vec<WorldbookEntry>,
    pub events: Vec<TranscriptEvent>,
    pub state: TableState,
    pub incremental: Mechanism,
}

pub(super) fn fixture() -> Fixture {
    let knight = card("knight", "騎士", "高大的騎士。", "其實是王子。");
    let fox = card("fox", "狐狸", "狡猾的狐狸。", "");
    let player = card("player", "阿濤", "旅人，{{user}}本人。", "");
    let mut person = entry(5, "店主", &[], true, Visibility::Public);
    person.is_person = true;
    let worldbook = vec![
        entry(1, "王國", &[], true, Visibility::Public),
        entry(2, "陰謀", &[], true, Visibility::Gm),
        entry(3, "酒館", &["酒館"], false, Visibility::Public),
        entry(
            4,
            "密道",
            &["酒館"],
            false,
            Visibility::Characters(vec!["knight".to_owned()]),
        ),
        person,
    ];
    let events = vec![
        event(TranscriptKind::Narration, "gm", "GM", "夜晚的酒館很熱鬧。"),
        event(TranscriptKind::Dialogue, "knight", "騎士", "「誰在那裡？」"),
        event(TranscriptKind::Player, "player", "阿濤", "我舉起手。"),
        event(TranscriptKind::Player, "player", "", "沒名字的玩家說話。"),
        event(TranscriptKind::System, "system", "系統", "時間流逝。"),
        marked(
            event(TranscriptKind::System, "system", "系統", "狡猾的狐狸。"),
            EventMarker::CardArrival {
                name: "狐狸".to_owned(),
            },
            false,
        ),
        marked(
            event(TranscriptKind::System, "system", "系統", "其實是王子。"),
            EventMarker::CardPrivate {
                name: "騎士".to_owned(),
            },
            true,
        ),
        marked(
            event(
                TranscriptKind::System,
                "system",
                "系統",
                "Heroes.騎士.Mood：緊張",
            ),
            EventMarker::StateUpdate,
            true,
        ),
        marked(
            event(TranscriptKind::System, "system", "系統", ""),
            EventMarker::GmCall {
                name: String::new(),
            },
            false,
        ),
        marked(
            event(TranscriptKind::System, "system", "系統", "店主走進來。"),
            EventMarker::PersonArrival {
                title: "店主".to_owned(),
            },
            false,
        ),
        marked(
            event(
                TranscriptKind::Narration,
                "gm",
                "GM",
                "前情：大家在酒館集合。",
            ),
            EventMarker::SceneSummary,
            false,
        ),
    ];
    let leaf = |value: &str| StateNode::Leaf(value.to_owned());
    let branch = |pairs: Vec<(&str, StateNode)>| {
        StateNode::Branch(pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
    };
    let state = TableState {
        table: BTreeMap::from([
            ("time".to_owned(), "深夜".to_owned()),
            ("place".to_owned(), "酒館".to_owned()),
            ("present".to_owned(), "騎士、狐狸".to_owned()),
            ("weather".to_owned(), "雨".to_owned()),
        ]),
        tree: BTreeMap::from([
            (
                "Heroes".to_owned(),
                branch(vec![
                    (
                        "騎士",
                        branch(vec![("Affection", leaf("40")), ("Mood", leaf("緊張"))]),
                    ),
                    ("狐狸", branch(vec![("Affection", leaf("10"))])),
                ]),
            ),
            (
                "World".to_owned(),
                branch(vec![("Location", leaf("{{user}}的酒館"))]),
            ),
        ]),
        notes: vec!["Heroes.騎士.Affection 請用增減量。".to_owned()],
        changes: BTreeMap::from([
            ("Heroes.騎士.Affection".to_owned(), "+5".to_owned()),
            ("Heroes.騎士.Mood".to_owned(), "更新".to_owned()),
        ]),
        triggers: BTreeMap::from([("mood".to_owned(), "空氣很緊繃。".to_owned())]),
        jumps: BTreeMap::new(),
    };
    let incremental = Mechanism {
        incremental: true,
        guide: "Affection 每回合必報。".to_owned(),
        rules: BTreeMap::from([
            (
                "Heroes.*.Affection".to_owned(),
                FieldRule::for_kind(FieldKind::Number),
            ),
            (
                "Heroes.*.Mood".to_owned(),
                FieldRule::for_kind(FieldKind::Text),
            ),
        ]),
        triggers: vec![Trigger {
            id: "mood".to_owned(),
            title: "氣氛".to_owned(),
            mode: TriggerMode::Range,
            cases: Vec::new(),
            preamble: String::new(),
            scope: Vec::new(),
            flag: None,
        }],
        ..Mechanism::default()
    };
    Fixture {
        cards: vec![knight, fox],
        player,
        worldbook,
        events,
        state,
        incremental,
    }
}

fn dump(out: &mut String, title: &str, body: &str) {
    out.push_str(&format!("===== {title} =====\n{body}\n"));
}

fn dump_messages(out: &mut String, title: &str, messages: &[ChatMessage]) {
    let body: Vec<String> = messages
        .iter()
        .map(|message| format!("--- {} ---\n{}", message.role, message.content))
        .collect();
    dump(out, title, &body.join("\n"));
}

/// 這語系送 AI 的骨架全部攤開成一份文字。
pub(super) fn render_all(lang: &str) -> String {
    let f = fixture();
    let plain = Mechanism::default();
    let roster: Vec<String> = f.cards.iter().map(|card| card.name.clone()).collect();
    let knight_branch = vec!["Heroes".to_owned(), "騎士".to_owned()];
    let hidden = StateScope {
        hidden: vec![vec!["Heroes".to_owned(), "狐狸".to_owned()]],
        align: false,
    };
    let align = StateScope {
        hidden: Vec::new(),
        align: true,
    };
    let mut out = String::new();

    for (title, mechanism, player) in [
        (
            "gm_lane_system incremental+player",
            &f.incremental,
            Some(&f.player),
        ),
        ("gm_lane_system plain no-player", &plain, None),
    ] {
        dump(
            &mut out,
            title,
            &gm_lane_system(
                "王國的歷史。{{user}}",
                &f.cards,
                player,
                &f.worldbook,
                mechanism,
                lang,
            ),
        );
    }
    dump(
        &mut out,
        "chars_lane_system player",
        &chars_lane_system(&f.cards, Some(&f.player), &f.worldbook, lang),
    );
    dump(
        &mut out,
        "chars_lane_system no-player",
        &chars_lane_system(&f.cards, None, &f.worldbook, lang),
    );

    let narrate = transport::narrate_instruction(lang, &roster, Some("阿濤")).content;
    for (title, mechanism, scope) in [
        ("gm_lane_turn incremental hidden", &f.incremental, &hidden),
        ("gm_lane_turn incremental align", &f.incremental, &align),
        ("gm_lane_turn plain", &plain, &StateScope::default()),
    ] {
        let turn = gm_lane_turn(
            &f.events,
            &f.worldbook,
            Some(&f.player),
            &f.state,
            mechanism,
            scope,
            &narrate,
            lang,
        );
        dump(&mut out, title, &turn.tail);
    }
    for hoist in [false, true] {
        let turn = chars_lane_turn(
            &f.cards[0],
            None,
            &f.events,
            &f.worldbook,
            &f.state,
            &f.incremental,
            Some(&knight_branch),
            lang,
            hoist,
        );
        dump(
            &mut out,
            &format!("chars_lane_turn hoist={hoist} tail"),
            &turn.tail,
        );
        dump(
            &mut out,
            &format!("chars_lane_turn hoist={hoist} confidential"),
            turn.confidential.as_deref().unwrap_or("<none>"),
        );
        dump(
            &mut out,
            &format!("chars_lane_turn hoist={hoist} hoisted"),
            turn.hoisted_private.as_deref().unwrap_or("<none>"),
        );
    }

    let gm_messages = assemble_gm_messages(
        "王國的歷史。",
        &f.cards,
        Some(&f.player),
        &f.events,
        &f.worldbook,
        &f.state,
        &f.incremental,
        &hidden,
        lang,
    );
    dump_messages(&mut out, "assemble_gm_messages", &gm_messages);
    for cards in [&f.cards[..1], &f.cards[..]] {
        let shared = assemble_shared_messages(
            &cards[0],
            cards,
            Some(&f.player),
            &f.events,
            &f.worldbook,
            &f.state,
            &f.incremental,
            Some(&knight_branch),
            lang,
        );
        dump_messages(
            &mut out,
            &format!("assemble_shared_messages cards={}", cards.len()),
            &shared,
        );
    }
    dump_messages(
        &mut out,
        "summary_messages",
        &transport::summary_messages(&f.events, lang),
    );

    for (title, message) in [
        (
            "narrate roster+player",
            transport::narrate_instruction(lang, &roster, Some("阿濤")),
        ),
        (
            "narrate roster",
            transport::narrate_instruction(lang, &roster, None),
        ),
        (
            "narrate empty",
            transport::narrate_instruction(lang, &[], None),
        ),
        (
            "takeover roster+player",
            transport::takeover_instruction(lang, &roster, Some("阿濤")),
        ),
        (
            "takeover empty",
            transport::takeover_instruction(lang, &[], None),
        ),
        (
            "card_format title",
            transport::card_format_instruction(lang, Some("輸出格式")),
        ),
        (
            "card_format none",
            transport::card_format_instruction(lang, None),
        ),
    ] {
        dump(&mut out, title, &message.content);
    }
    for format in [
        GmTurnFormat::CardFormat,
        GmTurnFormat::InterfaceTakeover,
        GmTurnFormat::Narration,
    ] {
        for has_roster in [false, true] {
            dump(
                &mut out,
                &format!("gm_closing {format:?} {has_roster}"),
                transport::gm_closing(format, has_roster, lang),
            );
        }
    }
    dump(
        &mut out,
        "summary_closing",
        transport::summary_closing(lang),
    );

    for side in [Side::Gm, Side::Character] {
        let lines: Vec<String> = f
            .events
            .iter()
            .map(|event| lane_event_line(event, lang, side).unwrap_or_else(|| "<skip>".to_owned()))
            .collect();
        dump(
            &mut out,
            &format!("lane_event_line {side:?}"),
            &lines.join("\n"),
        );
    }

    let flat_input = vec![
        transport_message("system", "系統本體"),
        transport_message("user", "使用者說話"),
        transport_message("assistant", "GM 的旁白"),
    ];
    let (system, prompt) = flatten_messages("GM", "收尾句", &flat_input, lang);
    dump(&mut out, "flatten labelled", &format!("{system}\n{prompt}"));
    let (system, prompt) = flatten_messages("", "", &flat_input, lang);
    dump(&mut out, "flatten bare", &format!("{system}\n{prompt}"));

    for (lane, opening) in [(Lane::Gm, true), (Lane::Gm, false), (Lane::Chars, true)] {
        dump(
            &mut out,
            &format!("build_prompt {lane:?} opening={opening}"),
            &build_prompt(&f.events, 0, "回合尾段", opening, lane, lang),
        );
    }

    let applied = "開頭指示\n## 世界設定\n舊的\n### 酒館\nA\n### 酒館\nB\n## 被刪掉\n內容\n";
    let current = "新的開頭指示\n## 世界設定\n新的\n### 酒館\nA\n### 酒館\nB2\n## 新增段\n內容\n";
    dump(
        &mut out,
        "render_patch",
        &render_patch(applied, current, lang).unwrap_or_default(),
    );
    out
}

fn transport_message(role: &str, content: &str) -> ChatMessage {
    ChatMessage {
        role: role.to_owned(),
        content: content.to_owned(),
    }
}

fn baseline_path(lang: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/lanes/scaffold_baseline")
        .join(format!("{lang}.txt"))
}

#[test]
fn scaffold_baseline_chinese_is_byte_identical() {
    for lang in ["zh-TW", "zh-CN", "zh-HK"] {
        let actual = render_all(lang);
        let path = baseline_path(lang);
        if std::env::var_os("SCAFFOLD_BASELINE_WRITE").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &actual).unwrap();
            continue;
        }
        let expected = std::fs::read_to_string(&path).unwrap();
        assert!(
            actual == expected,
            "{lang} 骨架與基準不同：{}",
            path.display()
        );
    }
}

/// 中文骨架才出現的字：非中文語系的整份輸出一個都不該有。
const CHINESE_SCAFFOLD: &[&str] = &[
    "你是這場",
    "（導演指示）",
    "## 世界設定（",
    "## 世界書",
    "## 登場角色",
    "## 玩家角色",
    "公開設定：",
    "私有設定（",
    "這桌還有這些人",
    "## 狀態更新協定",
    "## 介面由誰畫",
    "## 目前狀態",
    "## 當前情境",
    "## 上一輪被系統擋下",
    "## 你知道的世界情報",
    "的私有設定",
    "知道的世界情報",
    "目前的狀態",
    "現在你是",
    "現在請",
    "（旁白）",
    "（系統）",
    "以下是到目前為止的對話紀錄",
    "## 設定更新",
    "（開頭指示更新）",
    "（已移除段落",
    "標題：",
    "下一位",
    "（新的值）",
    "時間：",
    "地點：",
    "在場人物：",
];

/// 語言規則句與玩家稱呼換成佔位，剩下的就是骨架本身。
fn normalized(lang: &str) -> String {
    render_all(lang)
        .replace(transport::language_rule(lang), "<RULE>")
        .replace(crate::data::player_fallback_name(lang), "<PLAYER>")
}

#[test]
fn non_chinese_languages_share_one_english_scaffold() {
    let ja = normalized("ja");
    for lang in ["ko", "es", "pt-BR", "de", "fr", "ru"] {
        assert!(normalized(lang) == ja, "{lang} 的骨架與 ja 不同");
    }
    for marker in CHINESE_SCAFFOLD {
        assert!(!ja.contains(marker), "英文骨架殘留中文骨架字：{marker}");
    }
    for marker in [
        "You are the GM",
        "## World setting",
        "## Characters (public profiles)",
        "## Current state",
        "Here is the conversation so far:",
        "(Director instruction)",
        "## Settings update",
        "Title: <act name",
    ] {
        assert!(ja.contains(marker), "英文骨架缺：{marker}");
    }
    // 英文與未知語系同一份骨架（玩家稱呼 Player 會撞到骨架字，只比對語言規則以外的部分）
    let unknown = render_all("it");
    assert_eq!(unknown, render_all("en"));
    assert!(!unknown.contains("in English"), "摘要不得寫死英文輸出");
}

#[test]
fn output_language_rule_stays_native_under_english_scaffold() {
    for lang in ["ja", "fr", "ru"] {
        let rule = transport::language_rule(lang);
        let f = fixture();
        let system = gm_lane_system(
            "",
            &f.cards,
            None,
            &f.worldbook,
            &Mechanism::default(),
            lang,
        );
        assert!(system.starts_with("You are the GM") && system.contains(rule));
        assert!(chars_lane_system(&f.cards, None, &f.worldbook, lang).contains(rule));
        let summary = &transport::summary_messages(&f.events, lang)[0].content;
        assert!(summary.contains("Title: <act name") && summary.ends_with(rule));
        assert!(!summary.contains("in English"));
    }
    // 中文骨架下 zh-CN 仍是簡中規則（逐字見基準檔）
    assert!(render_all("zh-CN").contains(transport::language_rule("zh-CN")));
}

/// 共線換角色：英文骨架照樣是除了尾端那則以外逐字相同；單角色私設上提進 system，
/// 多角色時機密段整段落在 tail 裡（回合後才抹得乾淨）。
#[test]
fn english_shared_lane_prefix_is_stable_and_secrets_stay_in_place() {
    let f = fixture();
    let knight_branch = vec!["Heroes".to_owned(), "騎士".to_owned()];
    let fox_branch = vec!["Heroes".to_owned(), "狐狸".to_owned()];
    let shared = |index: usize, branch: &[String]| {
        assemble_shared_messages(
            &f.cards[index],
            &f.cards,
            Some(&f.player),
            &f.events,
            &f.worldbook,
            &f.state,
            &f.incremental,
            Some(branch),
            "fr",
        )
    };
    let knight = shared(0, &knight_branch);
    let fox = shared(1, &fox_branch);
    assert_eq!(knight[..knight.len() - 1], fox[..fox.len() - 1]);
    assert!(knight[1..]
        .iter()
        .any(|m| m.role == "assistant" && m.content.starts_with("騎士: ")));

    let single = assemble_shared_messages(
        &f.cards[0],
        &f.cards[..1],
        None,
        &f.events,
        &f.worldbook,
        &f.state,
        &f.incremental,
        Some(&knight_branch),
        "fr",
    );
    assert!(single[0].content.contains("## 騎士's private profile"));
    assert!(!single.last().unwrap().content.contains("其實是王子"));

    let turn = chars_lane_turn(
        &f.cards[0],
        None,
        &f.events,
        &f.worldbook,
        &f.state,
        &f.incremental,
        Some(&knight_branch),
        "fr",
        false,
    );
    let confidential = turn.confidential.unwrap();
    assert!(turn.tail.contains(&confidential));
    let scrubbed = turn.tail.replacen(&confidential, "", 1);
    assert!(!scrubbed.contains("其實是王子") && !scrubbed.contains("密道"));
    assert!(
        scrubbed.ends_with("Do not add a name prefix or any explanation outside the character.")
    );
}

#[test]
fn english_patch_lists_added_removed_and_duplicate_sections() {
    let patch = render_patch(
        "intro\n## A\nold\n### Card\none\n### Card\ntwo\n## Gone\nx\n",
        "intro2\n## A\nold\n### Card\none\n### Card\nTWO\n## New\ny\n",
        "de",
    )
    .unwrap();
    assert!(patch.starts_with("## Settings update\n"));
    assert!(patch.contains("(Opening instructions updated)\nintro2"));
    assert!(patch.contains("### Card\nTWO\n") && patch.matches("### Card").count() == 1);
    assert!(patch.contains("## New\ny\n"));
    assert!(patch.contains("(Removed sections: \"## Gone\")"));
    assert!(!patch.contains("## A"));
}

fn lane_input<'a>(events: &'a [TranscriptEvent], system: &str) -> TurnInput<'a> {
    TurnInput {
        lane: Lane::Gm,
        scene: 0,
        events,
        lang: "ja",
        frozen_system: system.to_owned(),
        tail: "tail".to_owned(),
        confidential: None,
        prefix: None,
        echo: ReplyEcho::Narration,
        scope: None,
    }
}

fn lane_state(events: &[TranscriptEvent], system: &str, provider: LaneProvider) -> LaneState {
    LaneState {
        session_id: "sid".to_owned(),
        scene: 0,
        sent_events: events.len(),
        sent_hash: events_fingerprint(events),
        snapshot: system.to_owned(),
        applied: system.to_owned(),
        pending_rewrite: None,
        expected_reply: None,
        provider: provider.as_str().to_owned(),
        model: "m".to_owned(),
        last_call_epoch: 1_000,
        last_prompt_tokens: 0,
        agy_usage: None,
    }
}

/// 既有非中文桌升級：舊（繁中）骨架的線遇到新（英文）骨架。Claude 快取活著走補丁、過期整份追平，
/// Grok／Agy 因 system 改變重開；追上之後第二輪照常續聊。已送段指紋只看原事件，渲染前綴改字不影響。
#[test]
fn upgrading_scaffold_patches_or_rebases_claude_and_reopens_grok_agy() {
    let f = fixture();
    let old = gm_lane_system(
        "",
        &f.cards,
        None,
        &f.worldbook,
        &Mechanism::default(),
        "zh-TW",
    );
    let new = gm_lane_system(
        "",
        &f.cards,
        None,
        &f.worldbook,
        &Mechanism::default(),
        "ja",
    );
    let events = &f.events[..3];
    let input = lane_input(events, &new);
    let live = 1_001;
    let expired = 1_000 + CACHE_TTL_SECS + 1;

    let claude = lane_state(events, &old, LaneProvider::Claude);
    match plan_turn(Some(&claude), &input, live, LaneProvider::Claude) {
        TurnPlan::Resume {
            system,
            patch,
            rebased,
            base,
            ..
        } => {
            assert_eq!(system, old);
            assert!(patch.unwrap().starts_with("## Settings update"));
            assert!(!rebased);
            assert_eq!(base, events.len());
        }
        TurnPlan::Reopen { .. } => panic!("claude 快取活著走補丁"),
    }
    match plan_turn(Some(&claude), &input, expired, LaneProvider::Claude) {
        TurnPlan::Resume {
            system,
            patch,
            rebased,
            ..
        } => {
            assert_eq!(system, new);
            assert!(patch.is_none() && rebased);
        }
        TurnPlan::Reopen { .. } => panic!("claude 過期整份追平"),
    }
    for provider in [LaneProvider::Grok, LaneProvider::Agy] {
        let state = lane_state(events, &old, provider);
        assert!(matches!(
            plan_turn(Some(&state), &input, live, provider),
            TurnPlan::Reopen {
                reason: ReopenReason::SystemChanged
            }
        ));
        let caught_up = lane_state(events, &new, provider);
        assert!(matches!(
            plan_turn(Some(&caught_up), &input, live, provider),
            TurnPlan::Resume { patch: None, .. }
        ));
    }
    let caught_up = lane_state(events, &new, LaneProvider::Claude);
    assert!(matches!(
        plan_turn(Some(&caught_up), &input, live, LaneProvider::Claude),
        TurnPlan::Resume {
            patch: None,
            rebased: false,
            ..
        }
    ));
}
