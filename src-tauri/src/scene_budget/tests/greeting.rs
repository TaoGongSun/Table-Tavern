//! greeting-context-st-parity：量測與實送同一份開場白政策（本幕開場送原文、換幕後的格式範例）。
use super::{gm_path, text_of, world_with};
use crate::data;
use crate::lanes::{self, Lane};
use crate::scene_budget::measure;

const OPENING: &str =
    "風雪。\n<details>\n<summary>状态栏</summary>\n- 驻守天数：第 1 天\n</details>";

fn post_opening(root: &std::path::Path, world: &str) {
    data::append_opening(
        root,
        world,
        0,
        "2026-10-28T00:00:00.000Z",
        OPENING,
        &crate::transport::extract_state_block(OPENING),
        "阿濤",
    )
    .unwrap();
}

/// lane（claude、codex）量的 system 與 prompt＝實送 `gm_lane_parts`＋`build_prompt_as`。
fn assert_measure_matches(root: &std::path::Path, world: &str) {
    let lang = "zh-TW";
    let materials = crate::chat_assembly::gm_materials(root, world).unwrap();
    let scan = crate::chat_assembly::test_gm_scan(root, world, &materials, lang);
    let (scope, _) = crate::chat_assembly::gm_scope(&materials);
    let (instruction, closing) =
        crate::chat_assembly::gm_instruction(root, world, &materials, &scan, lang);
    let joined = format!("{}\n{closing}", instruction.content);
    let (system, turn) =
        crate::chat_assembly::gm_lane_parts(&materials, &scan, &scope, &joined, lang);
    let side = lanes::lane_side(Lane::Gm, materials.opening.raw_history);
    let prompt = lanes::build_prompt_as(&materials.events, 0, &turn.tail, true, side, lang);
    match gm_path(root, world, "claude") {
        measure::Request::Cli {
            system: measured_system,
            prompt: measured_prompt,
        } => {
            assert_eq!(measured_system, system);
            assert_eq!(measured_prompt, prompt);
        }
        measure::Request::Api(_) => panic!("claude 走 lane"),
    }
    match gm_path(root, world, "codex") {
        measure::Request::Cli {
            system: measured_system,
            prompt: measured_prompt,
        } => {
            assert_eq!(measured_system, system);
            assert_eq!(measured_prompt, prompt);
        }
        measure::Request::Api(_) => panic!("codex 走 lane"),
    }
}

#[test]
fn gm_measure_follows_greeting_policy() {
    let (root, world) = world_with(&[], "greeting");
    post_opening(&root, &world);
    assert_measure_matches(&root, &world);
    for transport in ["claude", "codex", "api"] {
        let measured = text_of(&gm_path(&root, &world, transport));
        assert!(
            measured.contains("<summary>状态栏</summary>"),
            "{transport}"
        );
    }
    // 換幕後：範例段在 system，三種後端都量得到
    data::begin_next_scene(&root, &world, "摘要", None).unwrap();
    assert_measure_matches(&root, &world);
    for transport in ["claude", "codex", "api"] {
        let measured = text_of(&gm_path(&root, &world, transport));
        assert!(measured.contains("僅供格式範例"), "{transport}");
        assert!(
            measured.contains("<summary>状态栏</summary>"),
            "{transport}"
        );
    }
    std::fs::remove_dir_all(&root).unwrap();
}
