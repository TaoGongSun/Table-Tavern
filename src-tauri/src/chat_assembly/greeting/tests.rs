//! 開場白政策：開場送原文、換幕往回找範例、範例不進世界書掃描、接管／退場例外、API 與 lane 一致。
use super::*;
use crate::chat_assembly::{gm_instruction, gm_lane_parts, gm_materials, gm_messages, gm_scope};
use crate::data::{self, TranscriptEvent, TranscriptKind, Visibility, WorldbookEntry};
use crate::lanes::{self, Lane};

struct Root(std::path::PathBuf);

impl Root {
    fn new(label: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "table-tavern-greeting-{label}-{}-{id}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const LANG: &str = "zh-TW";
/// 只出現在開場白狀態欄裡的關鍵字：顯示文字（text）剝掉了它
const SECRET: &str = "城門密令";
const ENTRY_BODY: &str = "密令條目的內文";
const OPENING: &str = "風雪拍打著城門。\n\n<details>\n<summary>状态栏</summary>\n- 驻守天数：第 1 天\n- 线索：城門密令\n</details>";

fn card() -> data::CharacterCard {
    data::CharacterCard {
        id: "01ARZ3NDEKTSV4RRFFQ69G5FAW".to_owned(),
        name: "灰岩要塞".to_owned(),
        color: "#336699".to_owned(),
        avatar: "🏰".to_owned(),
        tier: data::Tier::Balanced,
        show_image: true,
        archived: false,
        gen_prompt: String::new(),
        public_md: "### 簡介\n北境要塞\n\n### 開場白\n說明頁".to_owned(),
        private_md: format!("### 備用開場白 1\n{OPENING}"),
    }
}

fn table(label: &str) -> (Root, String) {
    let root = Root::new(label);
    let world_id = data::create_world(&root.0, "要塞").unwrap();
    data::write_character(&root.0, &world_id, &card()).unwrap();
    data::upsert_worldbook_entry(
        &root.0,
        &world_id,
        WorldbookEntry {
            uid: u64::MAX,
            title: "密令".to_owned(),
            keys: vec![SECRET.to_owned()],
            content: ENTRY_BODY.to_owned(),
            constant: false,
            order: 1,
            disabled: false,
            visibility: Visibility::Gm,
            is_person: false,
            locked: false,
        },
    )
    .unwrap();
    (root, world_id)
}

fn post_opening(root: &std::path::Path, world_id: &str, scene: u64, raw: &str) {
    data::append_opening(
        root,
        world_id,
        scene,
        "2026-10-28T00:00:00.000Z",
        raw,
        &crate::transport::extract_state_block(raw),
        "阿濤",
    )
    .unwrap();
}

fn player(text: &str) -> TranscriptEvent {
    TranscriptEvent {
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
        raw: None,
        ts: "2026-10-28T00:01:00.000Z".to_owned(),
        speaker_id: String::new(),
        speaker_name: "阿濤".to_owned(),
        kind: TranscriptKind::Player,
        text: text.to_owned(),
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
    }
}

/// GM 一輪實送：(API 訊息全文, API system, lane system, lane prompt)
fn gm_turn(root: &std::path::Path, world_id: &str) -> (String, String, String, String) {
    let materials = gm_materials(root, world_id).unwrap();
    let scan = crate::chat_assembly::test_gm_scan(root, world_id, &materials, LANG);
    let (scope, _) = gm_scope(&materials);
    let (instruction, closing) = gm_instruction(root, world_id, &materials, &scan, LANG);
    let joined = format!("{}\n{closing}", instruction.content);
    let api = gm_messages(&materials, &scan, &scope, instruction, LANG);
    let (lane_system, turn) = gm_lane_parts(&materials, &scan, &scope, &joined, LANG);
    let side = lanes::lane_side(Lane::Gm, materials.opening.raw_history);
    let prompt = lanes::build_prompt_as(&materials.events, 0, &turn.tail, true, side, LANG);
    let all = api
        .iter()
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    (all, api[0].content.clone(), lane_system, prompt)
}

#[test]
fn gm_sees_raw_opening_and_cards_lose_greetings() {
    let (root, world_id) = table("raw");
    post_opening(&root.0, &world_id, 0, OPENING);
    let (all, api_system, lane_system, prompt) = gm_turn(&root.0, &world_id);
    // 卡片開場白段不進 system，API 與 lane 同一份 system
    assert_eq!(api_system, lane_system);
    assert!(!api_system.contains("說明頁") && !api_system.contains("備用開場白"));
    assert!(api_system.contains("北境要塞"));
    // 本幕開場送原文（狀態欄回來了），API 與 lane 一致
    assert!(all.contains("<summary>状态栏</summary>") && all.contains(SECRET));
    assert!(prompt.contains("<summary>状态栏</summary>") && prompt.contains(SECRET));
    // 世界書掃描照 text：只在狀態欄裡的關鍵字不觸發
    assert!(!all.contains(ENTRY_BODY) && !prompt.contains(ENTRY_BODY));
    // 角色線照 text：狀態欄不回來
    let side = lanes::lane_side(Lane::Chars, false);
    let events = data::read_transcript(&root.0, &world_id, 0).unwrap();
    let chars = lanes::build_prompt_as(&events, 0, "尾段", true, side, LANG);
    assert!(chars.contains("風雪拍打著城門") && !chars.contains("<details>"));
    // 對照：關鍵字真的出現在對話裡就觸發，證明上面不是條目壞了
    data::append_transcript(&root.0, &world_id, 0, &player(&format!("我問起{SECRET}。"))).unwrap();
    let (all, ..) = gm_turn(&root.0, &world_id);
    assert!(all.contains(ENTRY_BODY));
}

#[test]
fn opening_without_raw_falls_back_to_text() {
    let (root, world_id) = table("no-raw");
    post_opening(&root.0, &world_id, 0, "只有旁白。");
    let events = data::read_transcript(&root.0, &world_id, 0).unwrap();
    assert!(events[0].opening && events[0].raw.is_none());
    let (all, _, _, prompt) = gm_turn(&root.0, &world_id);
    assert!(all.contains("只有旁白。") && prompt.contains("只有旁白。"));
}

#[test]
fn next_scene_gets_branch_opening_as_example_outside_scan() {
    let (root, world_id) = table("next-scene");
    post_opening(&root.0, &world_id, 0, OPENING);
    data::begin_next_scene(&root.0, &world_id, "第一幕摘要", None).unwrap();
    data::begin_next_scene(&root.0, &world_id, "第二幕摘要", None).unwrap();
    let (all, api_system, lane_system, prompt) = gm_turn(&root.0, &world_id);
    assert_eq!(api_system, lane_system);
    assert!(
        api_system.contains("僅供格式範例") && api_system.contains("<summary>状态栏</summary>")
    );
    assert!(!api_system.contains("說明頁") && !api_system.contains("備用開場白"));
    // 範例只在 system 一次，歷史裡沒有開場；掃描看不到範例
    assert_eq!(all.matches("風雪拍打著城門").count(), 1);
    assert!(!prompt.contains("風雪拍打著城門"));
    assert!(!all.contains(ENTRY_BODY));
}

#[test]
fn branch_walk_covers_fork_copy_and_retracted_opening() {
    let (root, world_id) = table("fork");
    post_opening(&root.0, &world_id, 0, OPENING);
    data::begin_next_scene(&root.0, &world_id, "第一幕摘要", None).unwrap();
    // 從第 0 幕分岔：開場事件跟著複製，本幕就有 → 送原文、沒有範例
    let forked = data::fork_scene(&root.0, &world_id, 0).unwrap();
    let materials = gm_materials(&root.0, &world_id).unwrap();
    assert!(materials.opening.raw_history && materials.opening.example.is_none());
    // 分岔幕的開場收回、再換幕：往上一層退（parent＝分岔當下所在的第 1 幕 → 第 0 幕）
    data::remove_transcript_event(&root.0, &world_id, forked, "2026-10-28T00:00:00.000Z").unwrap();
    data::begin_next_scene(&root.0, &world_id, "分岔幕摘要", None).unwrap();
    let materials = gm_materials(&root.0, &world_id).unwrap();
    assert!(!materials.opening.raw_history);
    assert_eq!(materials.opening.example.as_deref(), Some(OPENING));
}

#[test]
fn never_posted_table_keeps_card_greetings() {
    let (root, world_id) = table("never");
    data::append_transcript(&root.0, &world_id, 0, &player("先不要開場。")).unwrap();
    let materials = gm_materials(&root.0, &world_id).unwrap();
    assert_eq!(materials.opening, crate::transport::GmOpening::default());
    assert!(materials.cards[0].public_md.contains("說明頁"));
    // 換幕後一樣：往回找不到開場
    data::begin_next_scene(&root.0, &world_id, "摘要", None).unwrap();
    let materials = gm_materials(&root.0, &world_id).unwrap();
    assert_eq!(materials.opening, crate::transport::GmOpening::default());
    assert!(!strip_card_greetings(&root.0, &world_id));
}

#[test]
fn takeover_and_retired_only_strip_cards() {
    for retired in [false, true] {
        let (root, world_id) = table("exception");
        post_opening(&root.0, &world_id, 0, OPENING);
        if retired {
            let mut state = data::read_state(&root.0, &world_id).unwrap();
            state.statusbar_retired = true;
            data::write_state(&root.0, &world_id, &state).unwrap();
        } else {
            data::write_interface_shell(&root.0, &world_id, "<UI>{{本回合.正文}}</UI>").unwrap();
        }
        let materials = gm_materials(&root.0, &world_id).unwrap();
        assert_eq!(
            materials.opening,
            crate::transport::GmOpening::default(),
            "{retired}"
        );
        assert!(
            !materials.cards[0].public_md.contains("說明頁"),
            "{retired}"
        );
        assert!(strip_card_greetings(&root.0, &world_id), "{retired}");
        // 換幕後也不恢復範例
        data::begin_next_scene(&root.0, &world_id, "摘要", None).unwrap();
        let materials = gm_materials(&root.0, &world_id).unwrap();
        assert_eq!(
            materials.opening,
            crate::transport::GmOpening::default(),
            "{retired}"
        );
    }
}

/// API 實送的訊息逐字：system＝lane 那份、第 0 則＝開場原文（一般桌）或顯示文字（接管、退場桌）。
#[test]
fn api_request_carries_opening_verbatim_by_table_kind() {
    for kind in ["plain", "takeover", "retired"] {
        let (root, world_id) = table(kind);
        post_opening(&root.0, &world_id, 0, OPENING);
        match kind {
            "takeover" => {
                data::write_interface_shell(&root.0, &world_id, "<UI>{{本回合.正文}}</UI>").unwrap()
            }
            "retired" => {
                let mut state = data::read_state(&root.0, &world_id).unwrap();
                state.statusbar_retired = true;
                data::write_state(&root.0, &world_id, &state).unwrap();
            }
            _ => {}
        }
        let materials = gm_materials(&root.0, &world_id).unwrap();
        let scan = crate::chat_assembly::test_gm_scan(&root.0, &world_id, &materials, LANG);
        let (scope, _) = gm_scope(&materials);
        let (instruction, closing) = gm_instruction(&root.0, &world_id, &materials, &scan, LANG);
        let joined = format!("{}\n{closing}", instruction.content);
        let api = gm_messages(&materials, &scan, &scope, instruction, LANG);
        let (lane_system, _) = gm_lane_parts(&materials, &scan, &scope, &joined, LANG);
        let text = data::read_transcript(&root.0, &world_id, 0).unwrap()[0]
            .text
            .clone();
        let expected = match kind {
            "plain" => OPENING.to_owned(),
            _ => text,
        };
        assert_eq!(api[0].role, "system", "{kind}");
        assert_eq!(api[0].content, lane_system, "{kind}");
        assert_eq!(
            api[1],
            crate::transport::ChatMessage {
                role: "assistant".to_owned(),
                content: expected,
            },
            "{kind}"
        );
        if kind != "plain" {
            assert!(
                api.iter()
                    .all(|message| !message.content.contains("<details>")),
                "{kind}"
            );
        }
    }
}
